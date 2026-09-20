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

/// Las tres capas de estrellas, copiadas del movil (`Galaxia.kt:903-910`):
/// **muchas y tenues, pocas y brillantes**, y las tres con el MISMO paralaje
/// (0,35 alli), que es lo que hace que el cielo se sienta lejos sin que las
/// capas se descorrelacionen.
///
/// `(cuantas por baldosa, paralaje, lado en px logicos, color, alfa)`. Las
/// cuentas son las del movil (70 / 22 / 7 por baldosa de 360 dp) llevadas a
/// nuestra baldosa de 512: sale un cielo algo mas escaso que el de antes
/// (eran 120 / 60 / 30 todas blancas) y con los colores de alli.
const CAPAS: [(usize, f32, f32, u32, f32); 3] = [
    (70, 0.35, 1.1, 0xFFFFFF, 0.40),
    (22, 0.35, 2.2, 0xDCE6FF, 0.60),
    (7, 0.35, 3.3, 0xFFF4D6, 0.87),
];

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

/// `0xRRGGBB` a `(r, g, b)` de 0 a 1.
fn rgb_de(hex: u32) -> (f32, f32, f32) {
    (
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
    )
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

/// Los puntos de `baldosa` repetida `nx` x `ny` veces, todos del tamano y
/// del color de su capa (como en el movil: el tamano lo da la capa, no el
/// brillo de cada estrella). El brillo sorteado solo modula la opacidad, asi
/// que dentro de una capa unas se ven mas que otras y el cielo no es una
/// rejilla de puntos identicos.
pub fn puntos_de_capa(
    baldosa: &[(f32, f32, f32)],
    nx: u32,
    ny: u32,
    lado_px: f32,
    rgb: (f32, f32, f32),
    alfa_capa: f32,
) -> Vec<Punto> {
    let lado = LADO as f32;
    let mut v = Vec::with_capacity(baldosa.len() * (nx * ny) as usize);
    for by in 0..ny {
        for bx in 0..nx {
            for &(x, y, brillo) in baldosa {
                v.push(Punto {
                    x: bx as f32 * lado + x.floor(),
                    y: by as f32 * lado + y.floor(),
                    lado: lado_px.max(1.0).round(),
                    // El brillo va de 0,2 a 0,7: se estira a 0,55..1 para
                    // que ninguna estrella de la capa desaparezca.
                    alfa: alfa_capa * (0.55 + (brillo - 0.2) * 0.9),
                    rgb,
                });
            }
        }
    }
    v
}

struct Capa {
    baldosa: Vec<(f32, f32, f32)>,
    factor: f32,
    lado_px: f32,
    rgb: (f32, f32, f32),
    alfa: f32,
    /// Teselada para `(nx, ny)` baldosas.
    realizada: Option<((u32, u32, u32), Puntos)>,
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
                .map(|(i, &(cuantas, factor, lado_px, hex, alfa))| Capa {
                    baldosa: puntos(semilla + i as u64, LADO, cuantas),
                    factor: if capas == 1 { 0.0 } else { factor },
                    lado_px,
                    rgb: rgb_de(hex),
                    alfa,
                    realizada: None,
                })
                .collect(),
        }
    }

    /// A que fraccion de la camara se mueven. Es el mismo para las tres (lo
    /// que hace que el cielo se sienta lejos sin que las capas se
    /// descorrelacionen), y 0 en Ligero, que no tiene paralaje.
    ///
    /// Lo pregunta A3 fase 2: si las estrellas van en su propio visual, hay
    /// que saber cuanto correrlo, y es esto, ni 0,35 fijo ni lo que diga la
    /// constante -en Ligero la constante mentiria-.
    pub fn paralaje(&self) -> f32 {
        self.capas.first().map_or(0.0, |c| c.factor)
    }

    /// El paralaje que TENDRAN unas estrellas de `capas` capas, sin
    /// crearlas. Hace falta porque el visual de las estrellas se monta al
    /// abrir la ventana y las capas no existen hasta el primer `preparar`,
    /// que necesita un motor de dibujo.
    pub fn paralaje_de(capas: usize) -> f32 {
        if capas == 1 || capas == 0 {
            // Una sola capa va quieta (D219); ninguna, tampoco.
            return 0.0;
        }
        CAPAS.first().map_or(0.0, |c| c.1)
    }

    /// Tesela las capas que falten para una pantalla de `ancho` x `alto` a
    /// `escala` pixeles fisicos por logico. Fuera del fotograma, como todo lo
    /// que crea recursos.
    pub fn preparar(
        &mut self,
        motor: &MotorRender,
        ancho: f32,
        alto: f32,
        escala: f32,
    ) -> Result<(), ErrorRender> {
        // La escala entra en la clave: al cambiar de monitor las estrellas
        // tienen que volver a teselarse con su nuevo tamano en pixeles.
        let tam = (
            baldosas_para(ancho),
            baldosas_para(alto),
            (escala * 100.0).round() as u32,
        );
        for c in &mut self.capas {
            if c.realizada.as_ref().is_some_and(|(t, _)| *t == tam) {
                continue;
            }
            let p = motor.realizar_puntos(&puntos_de_capa(
                &c.baldosa,
                tam.0,
                tam.1,
                c.lado_px * escala,
                c.rgb,
                c.alfa,
            ))?;
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
    fn cada_estrella_sale_en_cada_baldosa_con_el_lado_y_el_color_de_su_capa() {
        let baldosa = [(10.4, 20.9, 0.2), (100.0, 5.0, 0.7)];
        let crema = (1.0, 0.956_862_75, 0.839_215_7);
        let v = puntos_de_capa(&baldosa, 2, 3, 3.3, crema, 0.87);
        assert_eq!(v.len(), 2 * 6);
        assert!(
            v.iter()
                .any(|p| p.x == 512.0 + 10.0 && p.y == 1024.0 + 20.0)
        );
        // El lado y el color son de la capa, no de cada estrella.
        assert!(v.iter().all(|p| p.lado == 3.0 && p.rgb == crema));
        // El brillo sorteado solo modula la opacidad, y ninguna se apaga.
        assert!(v.iter().all(|p| p.alfa > 0.0 && p.alfa <= 0.87));
        assert!(v.iter().any(|p| p.alfa < 0.87), "la tenue se ve menos");
        // Caso negativo: una baldosa vacia no da ningun punto.
        assert!(puntos_de_capa(&[], 4, 4, 1.0, crema, 1.0).is_empty());
    }

    #[test]
    fn las_tres_capas_van_al_mismo_paso_y_de_mas_finas_a_mas_gruesas() {
        let factores: Vec<f32> = CAPAS.iter().map(|c| c.1).collect();
        assert!(factores.windows(2).all(|p| p[0] == p[1]), "{factores:?}");
        assert!(CAPAS.windows(2).all(|p| p[0].0 > p[1].0), "menos cuantas");
        assert!(CAPAS.windows(2).all(|p| p[0].2 < p[1].2), "mas gordas");
    }

    #[test]
    fn el_paralaje_que_se_pide_para_mover_el_visual_es_el_que_de_verdad_se_pinta() {
        // Con las tres capas, el de la constante.
        assert_eq!(Estrellas::nuevas(1, 3).paralaje(), CAPAS[0].1);
        // Caso negativo: en Ligero hay una sola capa y NO tiene paralaje
        // (D219), asi que su visual no se mueve. Preguntarle a la constante
        // en vez de a las capas moveria un cielo que se pinta quieto.
        assert_eq!(Estrellas::nuevas(1, 1).paralaje(), 0.0);
        // Y sin capas, nada que mover.
        assert_eq!(Estrellas::nuevas(1, 0).paralaje(), 0.0);
    }
}
