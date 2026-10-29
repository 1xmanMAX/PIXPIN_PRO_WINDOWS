//! **Que OCR lee una imagen** (29-oct-2026, prototipo `elegir_ocr`, ver
//! `docs/probar-ideas-sin-interfaz.md`, «Más casos» y `docs/ocr.md`):
//! Windows (rapido, sin bajar nada, bien con lo de pantalla) o Indra (en el
//! PC, mejor con tablas y manuscrito). Jev dice si hay texto y de que clase
//! es; si ya leyeron los dos, cual copia mejor el texto. Elegir el motor lo
//! hace el codigo.
//!
//! **Tabla del motor** ([`motor`]), `tipo` la opcion mas probable:
//!
//! | `hay_texto` | `tipo` (≥ [`CLARO`]) | Indra bajado | motor |
//! |---|---|---|---|
//! | ≤ NO | — | — | ninguno (no se lee) |
//! | — | manuscrito o tabla | si | Indra |
//! | — | manuscrito o tabla | no | Windows (y se sugiere bajar Indra) |
//! | — | captura de pantalla o impreso | — | Windows |
//! | — | < CLARO | — | el de los ajustes (lo de siempre) |
//!
//! **Tabla de la lectura** ([`lectura`]): con las dos lecturas, la mas
//! probable de `mejor_lectura` si pasa de [`CLARO`]; si no, la del motor
//! de la tabla de arriba.

use serde_json::json;

use crate::preguntas::NO;
use crate::preguntas::ocr as q;
use crate::{Cliente, Error, Imagen, Peticion};

/// Por encima, una Choice es clara.
pub const CLARO: f64 = 0.6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motor {
    Windows,
    Indra,
}

impl Motor {
    pub fn id(self) -> &'static str {
        match self {
            Motor::Windows => "windows",
            Motor::Indra => "indra",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Captura,
    Impreso,
    Tabla,
    Manuscrito,
}

impl Tipo {
    pub const TODOS: [Tipo; 4] = [Tipo::Captura, Tipo::Impreso, Tipo::Tabla, Tipo::Manuscrito];

    pub fn id(self) -> &'static str {
        match self {
            Tipo::Captura => q::CAPTURA,
            Tipo::Impreso => q::IMPRESO,
            Tipo::Tabla => q::TABLA,
            Tipo::Manuscrito => q::MANUSCRITO,
        }
    }
}

/// La peticion; `lecturas` (motor, texto) solo si ya leyeron los dos.
pub fn peticion(imagen: &Imagen, lecturas: &[(Motor, String)]) -> Peticion {
    let r = crate::mejor_foto::referencia(imagen);
    let mut p = Peticion::nueva(json!({ "imagen": r }))
        .con_imagen(&r, imagen.clone())
        .con(q::HAY_TEXTO, q::hay_texto())
        .con(q::TIPO, q::tipo());
    if lecturas.len() > 1 {
        let ops: Vec<(String, String)> = lecturas
            .iter()
            .map(|(m, t)| (m.id().to_string(), crate::recortar(t, 600)))
            .collect();
        p = p.con(q::MEJOR, q::mejor(&ops));
    }
    p
}

#[derive(Debug, Clone, PartialEq)]
pub struct Senales {
    pub hay_texto: f64,
    /// El tipo mas probable y su probabilidad.
    pub tipo: (Tipo, f64),
    /// La lectura mas probable y su probabilidad, si se pregunto.
    pub mejor: Option<(Motor, f64)>,
}

impl Senales {
    pub fn de(r: &crate::Respuestas, con_lecturas: bool) -> Result<Senales, Error> {
        let (_, probs) = r.choice(q::TIPO)?;
        let tipo = Tipo::TODOS
            .iter()
            .map(|t| (*t, probs.get(t.id()).copied().unwrap_or(0.0)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or((Tipo::Captura, 0.0));
        let mejor = if con_lecturas {
            let (_, probs) = r.choice(q::MEJOR)?;
            [Motor::Windows, Motor::Indra]
                .iter()
                .filter_map(|m| probs.get(m.id()).map(|p| (*m, *p)))
                .max_by(|a, b| a.1.total_cmp(&b.1))
        } else {
            None
        };
        Ok(Senales {
            hay_texto: r.noul(q::HAY_TEXTO)?,
            tipo,
            mejor,
        })
    }
}

/// Lo que dice la tabla del motor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eleccion {
    /// No hay texto: no se lee.
    Ninguno,
    /// Este motor; `bajar_indra`: Indra iria mejor pero no esta bajado.
    Este { motor: Motor, bajar_indra: bool },
    /// No esta claro: el de los ajustes.
    ElDeSiempre,
}

/// **La tabla del motor.**
pub fn motor(s: &Senales, indra_bajado: bool) -> Eleccion {
    if s.hay_texto <= NO {
        return Eleccion::Ninguno;
    }
    let (tipo, p) = s.tipo;
    if p < CLARO {
        return Eleccion::ElDeSiempre;
    }
    match tipo {
        Tipo::Manuscrito | Tipo::Tabla if indra_bajado => Eleccion::Este {
            motor: Motor::Indra,
            bajar_indra: false,
        },
        Tipo::Manuscrito | Tipo::Tabla => Eleccion::Este {
            motor: Motor::Windows,
            bajar_indra: true,
        },
        Tipo::Captura | Tipo::Impreso => Eleccion::Este {
            motor: Motor::Windows,
            bajar_indra: false,
        },
    }
}

/// **La tabla de la lectura**: con que lectura se queda (de dos).
/// `de_los_ajustes`: el motor elegido en Ajustes.
pub fn lectura(s: &Senales, de_los_ajustes: Motor) -> Motor {
    if let Some((m, p)) = s.mejor
        && p >= CLARO
    {
        return m;
    }
    match motor(s, true) {
        Eleccion::Este { motor, .. } => motor,
        _ => de_los_ajustes,
    }
}

/// **Pregunta.**
pub fn preguntar(
    cliente: &Cliente,
    imagen: &Imagen,
    lecturas: &[(Motor, String)],
) -> Result<Senales, Error> {
    let r = cliente.preguntar(&peticion(imagen, lecturas))?;
    Senales::de(&r, lecturas.len() > 1)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn s(hay: f64, tipo: Tipo, p: f64, mejor: Option<(Motor, f64)>) -> Senales {
        Senales {
            hay_texto: hay,
            tipo: (tipo, p),
            mejor,
        }
    }

    #[test]
    fn la_tabla_del_motor() {
        let indra = Eleccion::Este {
            motor: Motor::Indra,
            bajar_indra: false,
        };
        let windows = Eleccion::Este {
            motor: Motor::Windows,
            bajar_indra: false,
        };
        assert_eq!(motor(&s(0.9, Tipo::Manuscrito, 0.8, None), true), indra);
        assert_eq!(motor(&s(0.9, Tipo::Tabla, 0.8, None), true), indra);
        assert_eq!(
            motor(&s(0.9, Tipo::Tabla, 0.8, None), false),
            Eleccion::Este {
                motor: Motor::Windows,
                bajar_indra: true
            }
        );
        assert_eq!(motor(&s(0.9, Tipo::Captura, 0.9, None), true), windows);
        assert_eq!(motor(&s(0.9, Tipo::Impreso, 0.7, None), true), windows);
        assert_eq!(
            motor(&s(0.9, Tipo::Impreso, 0.5, None), true),
            Eleccion::ElDeSiempre
        );
        assert_eq!(
            motor(&s(0.2, Tipo::Manuscrito, 0.9, None), true),
            Eleccion::Ninguno
        );
    }

    #[test]
    fn la_tabla_de_la_lectura() {
        let x = s(0.9, Tipo::Captura, 0.9, Some((Motor::Indra, 0.8)));
        assert_eq!(lectura(&x, Motor::Windows), Motor::Indra);
        // Sin una clara, la del tipo; sin tipo claro, la de los ajustes.
        let x = s(0.9, Tipo::Manuscrito, 0.9, Some((Motor::Windows, 0.55)));
        assert_eq!(lectura(&x, Motor::Windows), Motor::Indra);
        let x = s(0.9, Tipo::Manuscrito, 0.4, Some((Motor::Windows, 0.55)));
        assert_eq!(lectura(&x, Motor::Indra), Motor::Indra);
    }

    #[test]
    fn las_lecturas_solo_van_si_hay_dos() {
        let img = Imagen {
            formato: "png".into(),
            bytes: vec![1, 2],
        };
        assert_eq!(peticion(&img, &[]).questions.len(), 2);
        let p = peticion(
            &img,
            &[
                (Motor::Windows, "Hola mundo".into()),
                (Motor::Indra, "Hola rnundo".into()),
            ],
        );
        let v: serde_json::Value = serde_json::from_slice(&p.cuerpo().unwrap()).unwrap();
        assert_eq!(
            v["questions"]["mejor_lectura"]["criteria"]["windows"],
            "«Hola mundo»"
        );
    }
}
