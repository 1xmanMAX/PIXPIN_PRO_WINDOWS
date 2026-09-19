//! El fondo de estrellas (D219): baldosas de 512 px con puntos, repetidas en
//! mosaico y con paralaje.
//!
//! Se pintan en pixeles de pantalla y no escalan con el zoom: son un fondo,
//! no algo del mundo. Si escalaran, al alejarse hasta ver todas las galaxias
//! las estrellas se juntarian en una mancha gris.
//!
//! Cada capa se tesela UNA vez como triangulos (`pixpin_render::puntos`)
//! cubriendo la pantalla y una baldosa de sobra, y cada fotograma solo la
//! corre. Antes eran bitmaps de 512 px repetidos: casi todo transparente, y
//! aun asi una pasada de pantalla entera por capa, que a 3000 x 2000 en una
//! grafica integrada eran 2,5-3 ms por capa (medido, `sesion::medir`).

use pixpin_motor2d::Azar;
use pixpin_render::puntos::{Punto, Puntos};
use pixpin_render::{ErrorRender, MotorRender, Pintor};

/// Lado de la baldosa: el periodo con el que se repite el cielo.
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

/// Cuantas baldosas hacen falta en un lado de `px` pixeles para que, corrida
/// hasta una baldosa entera, la capa siga tapando la pantalla.
pub fn baldosas_para(px: f32) -> u32 {
    (px.max(0.0) / LADO as f32).ceil() as u32 + 1
}

/// Los puntos de `baldosas` repetida `nx` x `ny` veces. Un punto de 1 px, y
/// de 2 x 2 los mas brillantes, algo mas tenue: si no, a 100 % de escala
/// todas parecen la misma.
pub fn puntos_de_capa(baldosa: &[(f32, f32, f32)], nx: u32, ny: u32) -> Vec<Punto> {
    let lado = LADO as f32;
    let mut v = Vec::with_capacity(baldosa.len() * (nx * ny) as usize);
    for by in 0..ny {
        for bx in 0..nx {
            for &(x, y, brillo) in baldosa {
                let (lado_px, alfa) = if brillo > 0.55 {
                    (2.0, brillo * 0.7)
                } else {
                    (1.0, brillo)
                };
                v.push(Punto {
                    x: bx as f32 * lado + x.floor(),
                    y: by as f32 * lado + y.floor(),
                    lado: lado_px,
                    alfa,
                });
            }
        }
    }
    v
}

struct Capa {
    baldosa: Vec<(f32, f32, f32)>,
    factor: f32,
    /// Teselada para `(nx, ny)` baldosas.
    realizada: Option<((u32, u32), Puntos)>,
}

pub struct Estrellas {
    capas: Vec<Capa>,
}

impl Estrellas {
    /// Una capa en Ligero, sin paralaje (D219); tres en Completo; cero, un
    /// cielo liso. Solo los puntos, en CPU: se teselan con `preparar`, que
    /// tambien los vuelve a teselar si se pierde el dispositivo.
    pub fn nuevas(semilla: u64, capas: usize) -> Self {
        Self {
            capas: CAPAS
                .iter()
                .take(capas.min(CAPAS.len()))
                .enumerate()
                .map(|(i, &(cuantas, factor))| Capa {
                    baldosa: puntos(semilla + i as u64, LADO, cuantas),
                    factor: if capas == 1 { 0.0 } else { factor },
                    realizada: None,
                })
                .collect(),
        }
    }

    /// Tesela las capas que falten para una pantalla de `ancho` x `alto`.
    /// Fuera del fotograma, como todo lo que crea recursos.
    pub fn preparar(
        &mut self,
        motor: &MotorRender,
        ancho: f32,
        alto: f32,
    ) -> Result<(), ErrorRender> {
        let tam = (baldosas_para(ancho), baldosas_para(alto));
        for c in &mut self.capas {
            if c.realizada.as_ref().is_some_and(|(t, _)| *t == tam) {
                continue;
            }
            let p = motor.realizar_puntos(&puntos_de_capa(&c.baldosa, tam.0, tam.1))?;
            c.realizada = Some((tam, p));
        }
        Ok(())
    }

    /// Olvida lo teselado (dispositivo perdido). Los puntos se quedan.
    pub fn soltar(&mut self) {
        for c in &mut self.capas {
            c.realizada = None;
        }
    }

    /// Pinta las capas. `camara_px` es donde esta la camara en pixeles de
    /// pantalla (su `x` y su `y` por el zoom). `solo_lejana`: solo la
    /// primera, que es lo que se pinta mientras la camara se mueve.
    pub fn pintar(&self, p: &Pintor, camara_px: (f32, f32), solo_lejana: bool) {
        let lado = LADO as f32;
        for (i, c) in self.capas.iter().enumerate() {
            if i > 0 && solo_lejana {
                break;
            }
            let Some((_, r)) = &c.realizada else { continue };
            // En pixeles enteros: la estrella de un pixel no tiembla entre
            // dos al moverse despacio.
            let dx = -desfase(camara_px.0, c.factor, lado).floor();
            let dy = -desfase(camara_px.1, c.factor, lado).floor();
            p.puntos(r, (dx, dy));
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
    fn la_capa_cubre_la_pantalla_aunque_se_corra_una_baldosa_entera() {
        // 3000 px: seis baldosas no bastan si se corre casi 512 a la
        // izquierda; hacen falta siete.
        assert_eq!(baldosas_para(3000.0), 7);
        assert!(baldosas_para(3000.0) as f32 * 512.0 - 511.0 >= 3000.0);
        assert_eq!(baldosas_para(512.0), 2);
        // Caso negativo: una pantalla sin tamano sigue teniendo una.
        assert_eq!(baldosas_para(0.0), 1);
    }

    #[test]
    fn cada_estrella_sale_en_cada_baldosa_y_las_brillantes_son_de_dos_pixeles() {
        let baldosa = [(10.4, 20.9, 0.3), (100.0, 5.0, 0.6)];
        let v = puntos_de_capa(&baldosa, 2, 3);
        assert_eq!(v.len(), 2 * 6);
        assert!(
            v.iter()
                .any(|p| p.x == 512.0 + 10.0 && p.y == 1024.0 + 20.0)
        );
        assert!(
            v.iter()
                .filter(|p| p.lado == 2.0)
                .all(|p| (p.alfa - 0.42).abs() < 1e-6)
        );
        // Caso negativo: una baldosa vacia no da ningun punto.
        assert!(puntos_de_capa(&[], 4, 4).is_empty());
    }
}
