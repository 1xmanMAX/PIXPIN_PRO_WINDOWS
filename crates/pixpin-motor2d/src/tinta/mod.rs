//! La tinta de Excalidraw, portada (E1). Ver `docs/excalidraw/mapa.md`.
//!
//! `freehand` y `laser` son portes literales y trabajan en f64. Este modulo
//! es la envoltura de Excalidraw (`getFreedrawOutlinePoints` en
//! `packages/element/src/shape.ts` @afa3a65): elige pluma, aplica los
//! factores de tamano y pasa a `Punto2`.

pub mod freehand;
pub mod laser;
pub mod prediccion;

use serde::{Deserialize, Serialize};

use crate::vector::Punto2;

pub const STREAMLINE_RATON: f32 = 0.5;
pub const STREAMLINE_LAPIZ: f32 = 0.2;
/// `VARIABLE_WIDTH_FREEDRAW.SIZE_FACTOR`: afinado a ojo por Excalidraw.
pub const FACTOR_VARIABLE: f32 = 4.25;
/// `CONSTANT_WIDTH_FREEDRAW.SIZE_FACTOR`.
pub const FACTOR_CONSTANTE: f32 = 1.4;
/// `FREEDRAW_STROKE_WIDTH`: la mitad que el resto de figuras (esquema 2.0).
pub const GROSOR_FINO: f32 = 0.5;
pub const GROSOR_MEDIO: f32 = 1.0;
pub const GROSOR_GRUESO: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Variabilidad {
    #[default]
    #[serde(rename = "variable")]
    Variable,
    #[serde(rename = "constant")]
    Constante,
}

/// `strokeOptions` de Excalidraw, con sus mismos nombres en el fichero.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OpcionesTinta {
    #[serde(rename = "variability", default)]
    pub variabilidad: Variabilidad,
    #[serde(default = "streamline_raton")]
    pub streamline: f32,
}

fn streamline_raton() -> f32 {
    STREAMLINE_RATON
}

impl Default for OpcionesTinta {
    fn default() -> Self {
        Self {
            variabilidad: Variabilidad::Variable,
            streamline: STREAMLINE_RATON,
        }
    }
}

fn a_punto(v: [f64; 2]) -> Punto2 {
    Punto2::nuevo(v[0] as f32, v[1] as f32)
}

/// El contorno de un trazo de lapiz, como lo pinta Excalidraw.
///
/// `opciones == None` es un trazo de antes de E1: su `grosor` era el tamano
/// en pixeles y no un `strokeWidth`, asi que se divide por el factor para
/// que un dibujo viejo no se abra cuatro veces mas gordo.
pub fn contorno_de_lapiz(
    puntos: &[Punto2],
    presiones: &[f32],
    grosor: f32,
    opciones: Option<OpcionesTinta>,
) -> Vec<Punto2> {
    let (o, grosor) = match opciones {
        Some(o) => (o, grosor),
        None => (OpcionesTinta::default(), grosor / FACTOR_VARIABLE),
    };
    match o.variabilidad {
        Variabilidad::Constante => {
            let pts: Vec<[f64; 2]> = puntos.iter().map(|p| [p.x as f64, p.y as f64]).collect();
            laser::contorno(
                &pts,
                (grosor * FACTOR_CONSTANTE) as f64,
                o.streamline as f64,
            )
            .into_iter()
            .map(a_punto)
            .collect()
        }
        Variabilidad::Variable => {
            // Excalidraw guarda presiones solo si son reales; una lista de
            // otra longitud no puede venir de un lapiz y se trata como
            // simulada en vez de leer fuera de rango.
            let simular = presiones.len() != puntos.len();
            let entrada: Vec<freehand::Entrada> = puntos
                .iter()
                .enumerate()
                .map(|(i, p)| freehand::Entrada {
                    x: p.x as f64,
                    y: p.y as f64,
                    presion: if simular {
                        None
                    } else {
                        Some(presiones[i] as f64)
                    },
                })
                .collect();
            let op = freehand::Opciones {
                size: (grosor * FACTOR_VARIABLE) as f64,
                thinning: 0.6,
                smoothing: 0.5,
                streamline: o.streamline as f64,
                simular_presion: simular,
                last: true,
                easing: freehand::seno,
            };
            freehand::contorno(&entrada, &op)
                .into_iter()
                .map(a_punto)
                .collect()
        }
    }
}

/// El resaltador (D45): grueso, sin adelgazar y translucido. No existe en
/// Excalidraw; se hace con la misma maquina con `thinning = 0`, que da
/// grosor constante y tapas redondas. `grosor * 3` conserva su tamano de
/// antes de E1.
pub fn contorno_de_resaltador(puntos: &[Punto2], grosor: f32) -> Vec<Punto2> {
    let entrada: Vec<freehand::Entrada> = puntos
        .iter()
        .map(|p| freehand::Entrada {
            x: p.x as f64,
            y: p.y as f64,
            presion: None,
        })
        .collect();
    let op = freehand::Opciones {
        size: (grosor * 3.0) as f64,
        thinning: 0.0,
        smoothing: 0.5,
        streamline: STREAMLINE_RATON as f64,
        simular_presion: true,
        last: true,
        easing: freehand::seno,
    };
    freehand::contorno(&entrada, &op)
        .into_iter()
        .map(a_punto)
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn recta() -> Vec<Punto2> {
        (0..40)
            .map(|i| Punto2::nuevo(i as f32 * 5.0, 0.0))
            .collect()
    }

    fn alto(c: &[Punto2]) -> f32 {
        let (mn, mx) = c
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
        mx - mn
    }

    #[test]
    fn un_trazo_legado_no_engorda_al_abrirlo_con_la_tinta_nueva() {
        // Grosor 4,25 sin opciones = el mismo trazo que grosor 1 con opciones.
        let legado = contorno_de_lapiz(&recta(), &[], 4.25, None);
        let nuevo = contorno_de_lapiz(&recta(), &[], 1.0, Some(OpcionesTinta::default()));
        assert_eq!(legado, nuevo);
    }

    #[test]
    fn la_pluma_constante_da_otra_forma_que_la_variable() {
        let v = contorno_de_lapiz(&recta(), &[], 1.0, Some(OpcionesTinta::default()));
        let constante = OpcionesTinta {
            variabilidad: Variabilidad::Constante,
            ..Default::default()
        };
        let c = contorno_de_lapiz(&recta(), &[], 1.0, Some(constante));
        assert_ne!(v, c);
    }

    #[test]
    fn presiones_de_otra_longitud_se_tratan_como_simuladas() {
        let p = recta();
        let simulada = contorno_de_lapiz(&p, &[], 1.0, Some(OpcionesTinta::default()));
        let rota = contorno_de_lapiz(&p, &[0.9, 0.9], 1.0, Some(OpcionesTinta::default()));
        assert_eq!(simulada, rota);
    }

    #[test]
    fn mas_grosor_es_mas_ancho() {
        let o = Some(OpcionesTinta::default());
        assert!(
            alto(&contorno_de_lapiz(&recta(), &[], GROSOR_GRUESO, o))
                > alto(&contorno_de_lapiz(&recta(), &[], GROSOR_FINO, o))
        );
    }

    #[test]
    fn las_opciones_viajan_con_los_nombres_de_excalidraw() {
        let o = OpcionesTinta {
            variabilidad: Variabilidad::Constante,
            streamline: 0.2,
        };
        let json = serde_json::to_string(&o).unwrap();
        assert!(json.contains("\"variability\":\"constant\""), "{json}");
    }
}
