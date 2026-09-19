//! El fondo de estrellas (D219): baldosas de 512 px con puntos, repetidas en
//! mosaico y con paralaje.
//!
//! Se pintan en pixeles de pantalla y no escalan con el zoom: son un fondo,
//! no algo del mundo. Si escalaran, al alejarse hasta ver todas las galaxias
//! las estrellas se juntarian en una mancha gris.

use pixpin_motor2d::Azar;
use pixpin_render::{ErrorRender, Interpolacion, MotorRender, Pintor, RectF};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

/// Lado de la baldosa. Potencia de dos y pequena: tres baldosas son 3 MB
/// de GPU, que caben en cualquier equipo modesto.
pub const LADO: u32 = 512;

/// Estrellas por capa y cuanto se mueve cada capa con la camara: la lejana
/// casi nada, la cercana lo mismo que el mundo.
const CAPAS: [(usize, f32); 3] = [(120, 0.2), (60, 0.5), (30, 1.0)];

/// Los puntos de una baldosa: `(x, y, brillo)` con el brillo entre 0,2 y
/// 0,7 (D217). Con la misma semilla salen siempre los mismos, asi el cielo
/// no cambia de una sesion a otra.
pub fn puntos(semilla: u64, lado: u32, cuantas: usize) -> Vec<(f32, f32, f32)> {
    let mut azar = Azar::nuevo((semilla as u32) ^ ((semilla >> 32) as u32) ^ 0x5bd1_e995);
    (0..cuantas)
        .map(|_| {
            let x = azar.siguiente() * lado as f32;
            let y = azar.siguiente() * lado as f32;
            let brillo = 0.2 + azar.siguiente() * 0.5;
            (x, y, brillo)
        })
        .collect()
}

/// Cuanto se corre una capa: lo que se movio la camara (en pixeles) por su
/// factor de paralaje, vuelto a la baldosa. `rem_euclid` y no `%`: con la
/// camara en negativo `%` da negativo y la baldosa dejaria un hueco.
pub fn desfase(camara: f32, factor: f32, lado: f32) -> f32 {
    (camara * factor).rem_euclid(lado)
}

/// Los pixeles RGBA de una baldosa. Un punto de 1 px, y de 2x2 los mas
/// brillantes: si no, a 100 % de escala todas parecen la misma.
fn pixeles(puntos: &[(f32, f32, f32)], lado: u32) -> Vec<u8> {
    let mut v = vec![0u8; (lado * lado * 4) as usize];
    let mut poner = |x: u32, y: u32, a: f32| {
        let i = ((y % lado) * lado + (x % lado)) as usize * 4;
        let alfa = (a * 255.0) as u8;
        if v[i + 3] < alfa {
            v[i..i + 4].copy_from_slice(&[255, 255, 255, alfa]);
        }
    };
    for &(x, y, brillo) in puntos {
        let (x, y) = (x as u32, y as u32);
        poner(x, y, brillo);
        if brillo > 0.55 {
            poner(x + 1, y, brillo * 0.6);
            poner(x, y + 1, brillo * 0.6);
            poner(x + 1, y + 1, brillo * 0.4);
        }
    }
    v
}

struct Capa {
    pixeles: Vec<u8>,
    factor: f32,
    bitmap: Option<ID2D1Bitmap1>,
}

pub struct Estrellas {
    capas: Vec<Capa>,
}

impl Estrellas {
    /// Una capa en Ligero, sin paralaje (D219); tres en Completo. Los
    /// pixeles se guardan tambien en CPU para volver a subirlos si se pierde
    /// el dispositivo, como hace `fondo_lienzo`.
    pub fn nuevas(motor: &MotorRender, semilla: u64, capas: usize) -> Result<Self, ErrorRender> {
        let mut s = Self {
            capas: CAPAS
                .iter()
                .take(capas.clamp(1, CAPAS.len()))
                .enumerate()
                .map(|(i, &(cuantas, factor))| Capa {
                    pixeles: pixeles(&puntos(semilla + i as u64, LADO, cuantas), LADO),
                    factor: if capas == 1 { 0.0 } else { factor },
                    bitmap: None,
                })
                .collect(),
        };
        s.volver_a_subir(motor)?;
        Ok(s)
    }

    /// Sube las capas que no tengan bitmap: al crearlas y tras un
    /// dispositivo perdido.
    pub fn volver_a_subir(&mut self, motor: &MotorRender) -> Result<(), ErrorRender> {
        for c in &mut self.capas {
            if c.bitmap.is_none() {
                c.bitmap =
                    Some(motor.bitmap_desde_pixeles_premultiplicado(LADO, LADO, &c.pixeles)?);
            }
        }
        Ok(())
    }

    /// Olvida los bitmaps (dispositivo perdido). Los pixeles se quedan.
    pub fn soltar(&mut self) {
        for c in &mut self.capas {
            c.bitmap = None;
        }
    }

    pub fn listas(&self) -> bool {
        self.capas.iter().all(|c| c.bitmap.is_some())
    }

    /// Pinta las capas en mosaico cubriendo `ancho` x `alto` pixeles.
    /// `camara_px` es donde esta la camara en pixeles de pantalla (su `x`
    /// y su `y` por el zoom).
    pub fn pintar(&self, p: &Pintor, camara_px: (f32, f32), ancho: f32, alto: f32) {
        let lado = LADO as f32;
        for c in &self.capas {
            let Some(b) = &c.bitmap else { continue };
            let x0 = -desfase(camara_px.0, c.factor, lado);
            let y0 = -desfase(camara_px.1, c.factor, lado);
            let mut y = y0;
            while y < alto {
                let mut x = x0;
                while x < ancho {
                    p.bitmap_con(
                        b,
                        RectF {
                            x,
                            y,
                            ancho: lado,
                            alto: lado,
                        },
                        None,
                        Interpolacion::Vecino,
                    );
                    x += lado;
                }
                y += lado;
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_brillos_quedan_entre_el_veinte_y_el_setenta_por_ciento_y_dentro_de_la_baldosa() {
        for (x, y, b) in puntos(3, 512, 500) {
            assert!((0.2..=0.7).contains(&b), "brillo {b}");
            assert!((0.0..512.0).contains(&x) && (0.0..512.0).contains(&y));
        }
    }

    #[test]
    fn con_la_camara_en_negativo_el_desfase_no_se_sale_de_la_baldosa() {
        let d = desfase(-100.0, 1.0, 512.0);
        assert_eq!(d, 412.0);
        assert!(desfase(-1.0e6, 0.5, 512.0) >= 0.0);
    }

    #[test]
    fn una_baldosa_tiene_tantos_pixeles_como_lado_por_lado_por_cuatro() {
        let v = pixeles(&puntos(1, 64, 10), 64);
        assert_eq!(v.len(), 64 * 64 * 4);
        assert!(v.chunks(4).any(|p| p[3] > 0), "alguna estrella se pinta");
        // Caso negativo: sin puntos, la baldosa es transparente.
        assert!(pixeles(&[], 64).iter().all(|b| *b == 0));
    }
}
