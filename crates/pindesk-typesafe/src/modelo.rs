//! **Que modelo contesta al asistente**: antes de cada pedido, lo dificil
//! que es (Score), si es una orden simple para la app, si pide razonar en
//! varios pasos y si un error tendria consecuencias (Nouls). El codigo elige
//! el modelo rapido (Claude Haiku, DeepSeek Flash) o el fuerte (Claude
//! Sonnet, DeepSeek V4 Pro).
//!
//! Antes de preguntar, el codigo: las ordenes del camino rapido de la app
//! (`asistente::rapido`) se hacen en este PC, sin modelo y sin Jev.
//!
//! **Tabla de decision** ([`decidir`]), por orden; `dificil` = probabilidad
//! de nivel 2 o 3, `muy_dificil` = de nivel 3:
//!
//! | condicion | modelo | motivo |
//! |---|---|---|
//! | `orden_simple` ≥ 0.7 y `dificil` ≤ 0.3 | rapido | orden |
//! | `razonar` ≥ 0.7 | fuerte | razonar |
//! | `muy_dificil` ≥ 0.5 | fuerte | dificil |
//! | `importa` ≥ 0.7 y `dificil` ≥ 0.5 | fuerte | importante |
//! | `dificil` ≤ 0.3 | rapido | facil |
//! | lo demas (sin decidir) | rapido | sin decidir (lo de siempre) |
//!
//! Sin respuesta de Jev, el de siempre: el rapido.

use serde_json::{Map, Value, json};

use crate::preguntas::modelo as q;
use crate::preguntas::{NO, SI};
use crate::{Cliente, Error, Escala, Peticion};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ruta {
    /// El modelo de siempre: Haiku o DeepSeek Flash.
    Rapido,
    /// El fuerte del mismo proveedor: Sonnet o DeepSeek V4 Pro.
    Fuerte,
}

/// Por que (para el registro y la pastilla).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motivo {
    Orden,
    Razonar,
    Dificil,
    Importante,
    Facil,
    SinDecidir,
}

/// Lo que hace falta para preguntar: el pedido y, si hay, los ultimos
/// mensajes (`(quien, texto)`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Contexto {
    pub pedido: String,
    pub recientes: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Senales {
    pub dificultad: Escala,
    pub orden: f64,
    pub razonar: f64,
    pub importa: f64,
}

impl Senales {
    pub fn de(r: &crate::Respuestas) -> Result<Senales, Error> {
        Ok(Senales {
            dificultad: r.score(q::DIFICULTAD, q::NIVELES.len())?,
            orden: r.noul(q::ORDEN)?,
            razonar: r.noul(q::RAZONAR)?,
            importa: r.noul(q::IMPORTA)?,
        })
    }
}

/// **La tabla.**
pub fn decidir(s: &Senales) -> (Ruta, Motivo) {
    let dificil = s.dificultad.al_menos(2);
    let muy_dificil = s.dificultad.al_menos(3);
    if s.orden >= SI && dificil <= NO {
        (Ruta::Rapido, Motivo::Orden)
    } else if s.razonar >= SI {
        (Ruta::Fuerte, Motivo::Razonar)
    } else if muy_dificil >= 0.5 {
        (Ruta::Fuerte, Motivo::Dificil)
    } else if s.importa >= SI && dificil >= 0.5 {
        (Ruta::Fuerte, Motivo::Importante)
    } else if dificil <= NO {
        (Ruta::Rapido, Motivo::Facil)
    } else {
        (Ruta::Rapido, Motivo::SinDecidir)
    }
}

/// Los mensajes de antes que se mandan como mucho.
pub const RECIENTES: usize = 3;

pub fn estado(c: &Contexto) -> Value {
    let mut m = Map::new();
    m.insert("pedido".into(), json!(crate::recortar(&c.pedido, 2000)));
    let n = c.recientes.len();
    let recientes: Vec<Value> = c
        .recientes
        .iter()
        .skip(n.saturating_sub(RECIENTES))
        .map(|(quien, texto)| json!({ "de": quien, "texto": crate::recortar(texto, 400) }))
        .collect();
    if !recientes.is_empty() {
        m.insert("conversacion_reciente".into(), Value::Array(recientes));
    }
    Value::Object(m)
}

pub fn peticion(c: &Contexto) -> Peticion {
    q::preguntas()
        .into_iter()
        .fold(Peticion::nueva(estado(c)), |p, (n, pr)| p.con(n, pr))
}

/// **Pregunta y decide.** `Err`: quien llama usa el de siempre.
pub fn elegir(cliente: &Cliente, c: &Contexto) -> Result<(Ruta, Motivo), Error> {
    let r = cliente.preguntar(&peticion(c))?;
    Ok(decidir(&Senales::de(&r)?))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::grabadas;
    use crate::transporte::Grabado;
    use std::sync::Arc;

    fn s(niveles: [f64; 4], orden: f64, razonar: f64, importa: f64) -> Senales {
        Senales {
            dificultad: Escala {
                valor: niveles.iter().enumerate().map(|(i, p)| i as f64 * p).sum(),
                confianza: 0.6,
                niveles: niveles.to_vec(),
            },
            orden,
            razonar,
            importa,
        }
    }

    #[test]
    fn la_tabla_del_modelo() {
        let facil = [0.6, 0.35, 0.05, 0.0];
        let medio = [0.05, 0.3, 0.55, 0.1];
        let dificil = [0.0, 0.05, 0.3, 0.65];
        assert_eq!(
            decidir(&s(facil, 0.95, 0.0, 0.0)),
            (Ruta::Rapido, Motivo::Orden)
        );
        assert_eq!(
            decidir(&s(medio, 0.1, 0.85, 0.2)),
            (Ruta::Fuerte, Motivo::Razonar)
        );
        assert_eq!(
            decidir(&s(dificil, 0.0, 0.5, 0.1)),
            (Ruta::Fuerte, Motivo::Dificil)
        );
        assert_eq!(
            decidir(&s(medio, 0.1, 0.4, 0.9)),
            (Ruta::Fuerte, Motivo::Importante)
        );
        assert_eq!(
            decidir(&s(facil, 0.2, 0.1, 0.9)),
            (Ruta::Rapido, Motivo::Facil)
        );
        assert_eq!(
            decidir(&s(medio, 0.2, 0.4, 0.4)),
            (Ruta::Rapido, Motivo::SinDecidir)
        );
        // Una «orden» que parece dificil no va por la fila de la orden.
        assert_eq!(
            decidir(&s(dificil, 0.9, 0.1, 0.1)),
            (Ruta::Fuerte, Motivo::Dificil)
        );
    }

    #[test]
    fn el_estado_lleva_solo_lo_ultimo() {
        let c = Contexto {
            pedido: "resuelve el problema 3".into(),
            recientes: (1..=5)
                .map(|i| ("usuario".to_string(), format!("m{i}")))
                .collect(),
        };
        let e = estado(&c);
        let r = e["conversacion_reciente"].as_array().unwrap();
        assert_eq!(r.len(), RECIENTES);
        assert_eq!(r[0]["texto"], "m3");
        assert!(
            estado(&Contexto::default())
                .get("conversacion_reciente")
                .is_none()
        );
    }

    #[test]
    fn elige_con_una_respuesta_grabada_y_falla_como_error() {
        let r = grabadas::respuesta(&[
            grabadas::score(q::DIFICULTAD, &[0.0, 0.1, 0.3, 0.6]),
            grabadas::noul(q::ORDEN, 0.02),
            grabadas::noul(q::RAZONAR, 0.9),
            grabadas::noul(q::IMPORTA, 0.4),
        ]);
        let t = Arc::new(Grabado::nuevo(vec![Grabado::ok(&r)]));
        let c = Cliente::con_transporte("k", t).con_cache_propio();
        let ctx = Contexto {
            pedido: "demuestra que la raíz de 2 es irracional".into(),
            recientes: vec![],
        };
        assert_eq!(elegir(&c, &ctx).unwrap(), (Ruta::Fuerte, Motivo::Razonar));
        // Ya no queda respuesta grabada, pero la misma peticion sale del
        // cache; otra distinta falla como red caida (no como «rapido»).
        assert!(elegir(&c, &ctx).is_ok());
        let otra = Contexto {
            pedido: "hola".into(),
            recientes: vec![],
        };
        assert!(matches!(elegir(&c, &otra), Err(Error::Red(_))));
    }
}
