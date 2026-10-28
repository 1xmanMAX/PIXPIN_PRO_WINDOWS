//! **La urgencia de una tarea segun su contexto**: lo que pasa si se hace
//! tarde (Score), si otro la espera, si el plazo lo puso otro y si en el
//! chat la piden ya (Nouls). Cuanto falta para su hora lo cuenta el codigo
//! (las fechas nunca se le preguntan a Jev).
//!
//! **Lo marcado por el usuario manda** (⏫ «muy urgente» es urgente; 🔼
//! «urgente», alta). Sin marca, unos puntos ([`puntos`]):
//!
//! | senal | puntos |
//! |---|---|
//! | consecuencia (valor esperado del Score, 0-3) | 0 a 3 |
//! | `bloquea` ≥ 0.7 | +1 |
//! | `plazo_ajeno` ≥ 0.7 | +1 |
//! | `piden_ya` ≥ 0.7 | +1 |
//! | su hora es hoy o ya paso | +2 |
//! | manana | +1.5 |
//! | en 2-3 dias | +1 |
//! | en 4-7 dias | +0.5 |
//!
//! | puntos | prioridad |
//! |---|---|
//! | ≥ 5 | urgente |
//! | ≥ 3 | alta |
//! | consecuencia «grave» (nivel 2-3) ≤ 0.3, nadie la espera, ningun plazo ajeno ni pedido, y sin hora en 7 dias | baja |
//! | lo demas | normal |
//!
//! Sin respuesta de Jev (sin clave, sin red), la prioridad es la marca del
//! usuario o «normal»: lo de siempre.

use serde_json::{Map, Value, json};

use crate::preguntas::urgencia as q;
use crate::preguntas::{NO, SI};
use crate::{Cliente, Error, Escala, Peticion};

/// La prioridad que se ensena y con la que se ordena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Prioridad {
    Urgente,
    Alta,
    Normal,
    Baja,
}

impl Prioridad {
    pub fn id(self) -> &'static str {
        match self {
            Prioridad::Urgente => "urgente",
            Prioridad::Alta => "alta",
            Prioridad::Normal => "normal",
            Prioridad::Baja => "baja",
        }
    }

    pub fn de_id(s: &str) -> Option<Prioridad> {
        [
            Prioridad::Urgente,
            Prioridad::Alta,
            Prioridad::Normal,
            Prioridad::Baja,
        ]
        .into_iter()
        .find(|p| p.id() == s)
    }
}

/// Lo que el usuario marco en la tarea.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Marca {
    #[default]
    Ninguna,
    Urgente,
    MuyUrgente,
}

/// Lo que hace falta para preguntar por una tarea.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Contexto {
    /// Su texto, sin las marcas ni la fecha.
    pub tarea: String,
    /// El titulo de su lista.
    pub lista: String,
    /// El nombre del chat.
    pub chat: String,
    /// Los ultimos mensajes del chat, del mas viejo al mas nuevo.
    pub recientes: Vec<String>,
}

/// Lo que Jev dijo de una tarea.
#[derive(Debug, Clone, PartialEq)]
pub struct Senales {
    pub consecuencia: Escala,
    pub bloquea: f64,
    pub plazo_ajeno: f64,
    /// `None` si no habia mensajes recientes (no se pregunto).
    pub piden_ya: Option<f64>,
}

impl Senales {
    pub fn de(r: &crate::Respuestas, con_mensajes: bool) -> Result<Senales, Error> {
        Ok(Senales {
            consecuencia: r.score(q::CONSECUENCIA, q::NIVELES.len())?,
            bloquea: r.noul(q::BLOQUEA)?,
            plazo_ajeno: r.noul(q::PLAZO_AJENO)?,
            piden_ya: if con_mensajes {
                Some(r.noul(q::PIDEN_YA)?)
            } else {
                None
            },
        })
    }
}

/// Los puntos de lo que falta para su hora (dias desde hoy; negativo, ya
/// paso).
pub fn puntos_del_plazo(dias: Option<i64>) -> f64 {
    match dias {
        None => 0.0,
        Some(d) if d <= 0 => 2.0,
        Some(1) => 1.5,
        Some(2..=3) => 1.0,
        Some(4..=7) => 0.5,
        Some(_) => 0.0,
    }
}

/// Los puntos de la tabla.
pub fn puntos(s: &Senales, dias: Option<i64>) -> f64 {
    let mas = |p: f64| if p >= SI { 1.0 } else { 0.0 };
    s.consecuencia.valor.clamp(0.0, 3.0)
        + mas(s.bloquea)
        + mas(s.plazo_ajeno)
        + mas(s.piden_ya.unwrap_or(0.0))
        + puntos_del_plazo(dias)
}

/// **La tabla.** `dias`: cuantos faltan para la hora de la tarea (`⏰`),
/// contados por el codigo.
pub fn decidir(marca: Marca, s: Option<&Senales>, dias: Option<i64>) -> Prioridad {
    match marca {
        Marca::MuyUrgente => return Prioridad::Urgente,
        Marca::Urgente => return Prioridad::Alta,
        Marca::Ninguna => {}
    }
    let Some(s) = s else {
        return Prioridad::Normal;
    };
    let n = puntos(s, dias);
    if n >= 5.0 {
        Prioridad::Urgente
    } else if n >= 3.0 {
        Prioridad::Alta
    } else if s.consecuencia.al_menos(2) <= NO
        && s.bloquea <= NO
        && s.plazo_ajeno <= NO
        && s.piden_ya.is_none_or(|p| p < SI)
        && dias.is_none_or(|d| d > 7)
    {
        Prioridad::Baja
    } else {
        Prioridad::Normal
    }
}

/// Los mensajes recientes que se mandan como mucho, y lo largo de cada uno.
pub const RECIENTES: usize = 5;
const LARGO_MENSAJE: usize = 300;

/// El estado de una tarea.
pub fn estado(c: &Contexto) -> Value {
    let mut m = Map::new();
    m.insert("tarea".into(), json!(crate::recortar(&c.tarea, 500)));
    if !c.lista.trim().is_empty() {
        m.insert("lista".into(), json!(crate::recortar(&c.lista, 120)));
    }
    if !c.chat.trim().is_empty() {
        m.insert("chat".into(), json!(crate::recortar(&c.chat, 120)));
    }
    let recientes: Vec<Value> = c
        .recientes
        .iter()
        .filter(|t| !t.trim().is_empty())
        .rev()
        .take(RECIENTES)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|t| json!(crate::recortar(t, LARGO_MENSAJE)))
        .collect();
    if !recientes.is_empty() {
        m.insert("mensajes_recientes".into(), Value::Array(recientes));
    }
    Value::Object(m)
}

pub fn peticion(c: &Contexto) -> (Peticion, bool) {
    let e = estado(c);
    let con_mensajes = e.get("mensajes_recientes").is_some();
    let p = q::preguntas(con_mensajes)
        .into_iter()
        .fold(Peticion::nueva(e), |p, (n, pr)| p.con(n, pr));
    (p, con_mensajes)
}

/// **Pregunta por una tarea.**
pub fn preguntar(cliente: &Cliente, c: &Contexto) -> Result<Senales, Error> {
    let (p, con_mensajes) = peticion(c);
    Senales::de(&cliente.preguntar(&p)?, con_mensajes)
}

/// **Pregunta por varias**, `hilos` a la vez, cada una con su resultado (un
/// fallo en una no se lleva las demas: su prioridad sera la de siempre).
pub fn preguntar_varias(
    cliente: &Cliente,
    tareas: &[Contexto],
    hilos: usize,
) -> Vec<Result<Senales, Error>> {
    crate::en_paralelo(tareas, hilos, &|c| Ok(preguntar(cliente, c)))
        .unwrap_or_else(|e| tareas.iter().map(|_| Err(e.clone())).collect())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::grabadas;
    use crate::transporte::Grabado;
    use std::sync::Arc;

    fn s(niveles: [f64; 4], bloquea: f64, plazo: f64, piden: Option<f64>) -> Senales {
        Senales {
            consecuencia: Escala {
                valor: niveles.iter().enumerate().map(|(i, p)| i as f64 * p).sum(),
                confianza: 0.6,
                niveles: niveles.to_vec(),
            },
            bloquea,
            plazo_ajeno: plazo,
            piden_ya: piden,
        }
    }

    const GRAVE: [f64; 4] = [0.0, 0.05, 0.15, 0.8];
    const MEDIA: [f64; 4] = [0.05, 0.2, 0.65, 0.1];
    const LEVE: [f64; 4] = [0.6, 0.35, 0.05, 0.0];

    #[test]
    fn la_marca_del_usuario_manda() {
        let leve = s(LEVE, 0.0, 0.0, None);
        assert_eq!(
            decidir(Marca::MuyUrgente, Some(&leve), None),
            Prioridad::Urgente
        );
        assert_eq!(decidir(Marca::Urgente, Some(&leve), None), Prioridad::Alta);
        assert_eq!(decidir(Marca::Urgente, None, None), Prioridad::Alta);
    }

    #[test]
    fn sin_jev_es_lo_de_siempre() {
        assert_eq!(decidir(Marca::Ninguna, None, Some(0)), Prioridad::Normal);
    }

    #[test]
    fn la_tabla_de_puntos() {
        // Grave (2.75) + plazo ajeno + para hoy: urgente.
        let pago = s(GRAVE, 0.1, 0.9, None);
        assert_eq!(
            decidir(Marca::Ninguna, Some(&pago), Some(0)),
            Prioridad::Urgente
        );
        // La misma sin fecha: 3.75, alta.
        assert_eq!(decidir(Marca::Ninguna, Some(&pago), None), Prioridad::Alta);
        // Media (1.8) que otro espera y que piden en el chat: 3.8, alta.
        let informe = s(MEDIA, 0.85, 0.2, Some(0.9));
        assert_eq!(
            decidir(Marca::Ninguna, Some(&informe), None),
            Prioridad::Alta
        );
        // Media sola: normal.
        let media = s(MEDIA, 0.1, 0.1, None);
        assert_eq!(
            decidir(Marca::Ninguna, Some(&media), None),
            Prioridad::Normal
        );
        // Leve, nadie la espera, sin fecha: baja.
        let leve = s(LEVE, 0.05, 0.1, Some(0.1));
        assert_eq!(decidir(Marca::Ninguna, Some(&leve), None), Prioridad::Baja);
        // La misma con hora manana ya no es baja.
        assert_eq!(
            decidir(Marca::Ninguna, Some(&leve), Some(1)),
            Prioridad::Normal
        );
        // Y una duda (bloquea 0.5) tampoco: no se baja lo que no esta claro.
        let duda = s(LEVE, 0.5, 0.1, None);
        assert_eq!(
            decidir(Marca::Ninguna, Some(&duda), None),
            Prioridad::Normal
        );
    }

    #[test]
    fn el_plazo_lo_cuenta_el_codigo() {
        assert_eq!(puntos_del_plazo(Some(-3)), 2.0);
        assert_eq!(puntos_del_plazo(Some(0)), 2.0);
        assert_eq!(puntos_del_plazo(Some(1)), 1.5);
        assert_eq!(puntos_del_plazo(Some(3)), 1.0);
        assert_eq!(puntos_del_plazo(Some(7)), 0.5);
        assert_eq!(puntos_del_plazo(Some(30)), 0.0);
        assert_eq!(puntos_del_plazo(None), 0.0);
    }

    #[test]
    fn el_estado_y_las_preguntas() {
        let c = Contexto {
            tarea: "Pagar la matrícula".into(),
            lista: "Universidad".into(),
            chat: "Mensajes guardados".into(),
            recientes: (1..=8).map(|i| format!("mensaje {i}")).collect(),
        };
        let (p, con) = peticion(&c);
        assert!(con);
        let r = p.state["mensajes_recientes"].as_array().unwrap();
        assert_eq!(r.len(), RECIENTES);
        assert_eq!(r[0], "mensaje 4");
        assert_eq!(r[4], "mensaje 8");
        assert_eq!(p.questions.len(), 4);
        let (p, con) = peticion(&Contexto {
            tarea: "x".into(),
            ..Default::default()
        });
        assert!(!con);
        assert!(p.state.get("mensajes_recientes").is_none());
        assert_eq!(p.questions.len(), 3);
    }

    #[test]
    fn pregunta_con_respuestas_grabadas() {
        let r = grabadas::respuesta(&[
            grabadas::score(q::CONSECUENCIA, &[0.0, 0.1, 0.2, 0.7]),
            grabadas::noul(q::BLOQUEA, 0.2),
            grabadas::noul(q::PLAZO_AJENO, 0.9),
        ]);
        let t = Arc::new(Grabado::nuevo(vec![Grabado::ok(&r), Err("caida".into())]));
        let c = Cliente::con_transporte("k", t).con_cache_propio();
        let tareas = vec![
            Contexto {
                tarea: "Pagar la matrícula".into(),
                ..Default::default()
            },
            Contexto {
                tarea: "Leer un libro".into(),
                ..Default::default()
            },
        ];
        let v = preguntar_varias(&c, &tareas, 1);
        let s = v[0].as_ref().unwrap();
        assert_eq!(s.piden_ya, None);
        // 2.6 de consecuencia + 1 del plazo ajeno: con hora en 2 dias, alta;
        // para hoy, urgente.
        assert_eq!(decidir(Marca::Ninguna, Some(s), Some(2)), Prioridad::Alta);
        assert_eq!(
            decidir(Marca::Ninguna, Some(s), Some(0)),
            Prioridad::Urgente
        );
        // La segunda fallo: es un error, no una prioridad baja.
        assert!(v[1].is_err());
    }
}
