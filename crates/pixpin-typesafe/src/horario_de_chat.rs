//! **Citas de los mensajes de un chat al horario** (29-oct-2026, prototipo
//! `citas_del_chat`, ver `docs/probar-ideas-sin-interfaz.md`, «Más
//! casos»): fuera de las reuniones, en un chat se propone en un mensaje
//! («¿te va el jueves a las 5?») y se acepta en otro («vale»). El codigo
//! busca las fechas ([`crate::fechas`]); Jev dice si el mensaje pone un
//! momento a una cita, cual de las expresiones es, si esta acordado o solo
//! propuesto y, si hay una propuesta de otra persona pendiente, si este
//! mensaje la acepta.
//!
//! **Tabla** ([`Horario::actualizar`]), por orden:
//!
//! | condicion | se hace |
//! |---|---|
//! | `cita` ≥ SI y `acordada` ≥ SI | evento (y se olvida lo pendiente) |
//! | `cita` ≥ SI y `acordada` < SI | queda pendiente (sustituye a la anterior) |
//! | hay pendiente de otro y `acepta` ≥ SI | evento con lo pendiente |
//! | `cita` entre NO y SI | nada, dudosa |
//! | lo demas | nada |
//!
//! Un evento en un momento ya puesto es «repetido». Sin fechas en el
//! mensaje ni nada pendiente de otro, no se pregunta.

use serde_json::{Map, Value, json};

use crate::fechas::{self, Expresion, Fecha, Momento};
use crate::preguntas::horario_de_chat as q;
use crate::preguntas::{NO, SI};
use crate::{Cliente, Error, Peticion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mensaje {
    pub de: String,
    pub texto: String,
}

/// Una propuesta que espera respuesta.
#[derive(Debug, Clone, PartialEq)]
pub struct Pendiente {
    pub de: String,
    pub texto: String,
    pub momento: Momento,
    pub expresion: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Evento {
    pub titulo: String,
    pub momento: Momento,
    pub expresion: String,
    /// `true`: acordado en otro mensaje (se acepto lo pendiente).
    pub aceptado: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Resultado {
    Evento(Evento),
    Pendiente(Pendiente),
    Repetido(Momento),
    Dudosa(f64),
    Nada,
}

/// Lo que va de los mensajes anteriores.
pub const ANTES: usize = 2;

/// La peticion preparada: la peticion (si hay algo que preguntar) y las
/// fechas del mensaje.
#[derive(Debug, Clone)]
pub struct Preparada {
    pub peticion: Option<Peticion>,
    pub fechas: Vec<Expresion>,
    pub claves: Vec<String>,
    /// Si se pregunta por lo pendiente.
    pub por_pendiente: bool,
}

pub fn estado(chat: &str, antes: &[Mensaje], m: &Mensaje) -> Value {
    let mut e = Map::new();
    if !chat.trim().is_empty() {
        e.insert("chat".into(), json!(crate::recortar(chat, 120)));
    }
    let antes: Vec<Value> = antes
        .iter()
        .skip(antes.len().saturating_sub(ANTES))
        .map(|a| json!(format!("{}: {}", a.de, crate::recortar(&a.texto, 300))))
        .collect();
    if !antes.is_empty() {
        e.insert("antes".into(), Value::Array(antes));
    }
    e.insert(
        "mensaje".into(),
        json!({"de": m.de, "texto": crate::recortar(&m.texto, 600)}),
    );
    Value::Object(e)
}

/// **Prepara la peticion** de un mensaje.
pub fn preparar(
    chat: &str,
    antes: &[Mensaje],
    m: &Mensaje,
    hoy: Fecha,
    pendiente: Option<&Pendiente>,
) -> Preparada {
    let fechas = fechas::buscar(&m.texto, hoy);
    let claves: Vec<String> =
        crate::claves_unicas(&fechas.iter().map(|f| f.texto.clone()).collect::<Vec<_>>())
            .into_iter()
            .map(|(k, _)| k)
            .collect();
    let mut p = Peticion::nueva(estado(chat, antes, m));
    if !fechas.is_empty() {
        p = p.con(q::CITA, q::cita()).con(q::ACORDADA, q::acordada());
        if fechas.len() > 1 {
            let ops: Vec<(String, String)> = claves
                .iter()
                .zip(&fechas)
                .map(|(k, f)| (k.clone(), f.texto.clone()))
                .collect();
            p = p.con(q::CUANDO, q::cuando(&ops));
        }
    }
    let por_pendiente = pendiente.is_some_and(|x| x.de != m.de);
    if let Some(x) = pendiente.filter(|_| por_pendiente) {
        p = p.con(q::ACEPTA, q::acepta(&crate::recortar(&x.texto, 200)));
    }
    Preparada {
        peticion: (!p.questions.is_empty()).then_some(p),
        fechas,
        claves,
        por_pendiente,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Senales {
    /// `None`: sin fechas, no se pregunto.
    pub cita: Option<f64>,
    pub acordada: f64,
    /// Cual de las fechas (indice).
    pub cual: usize,
    /// `None`: nada pendiente de otro.
    pub acepta: Option<f64>,
}

impl Senales {
    pub fn de(r: &crate::Respuestas, p: &Preparada) -> Result<Senales, Error> {
        let mut s = Senales::default();
        if !p.fechas.is_empty() {
            s.cita = Some(r.noul(q::CITA)?);
            s.acordada = r.noul(q::ACORDADA)?;
            if p.fechas.len() > 1 {
                let (elegida, _) = r.choice(q::CUANDO)?;
                s.cual = p.claves.iter().position(|k| k == elegida).unwrap_or(0);
            }
        }
        if p.por_pendiente {
            s.acepta = Some(r.noul(q::ACEPTA)?);
        }
        Ok(s)
    }
}

/// **Lo propuesto y acordado en un chat.**
#[derive(Debug, Clone, Default)]
pub struct Horario {
    pub pendiente: Option<Pendiente>,
    pub eventos: Vec<Evento>,
}

impl Horario {
    fn poner(&mut self, e: Evento) -> Resultado {
        self.pendiente = None;
        if self.eventos.iter().any(|x| x.momento == e.momento) {
            return Resultado::Repetido(e.momento);
        }
        self.eventos.push(e.clone());
        Resultado::Evento(e)
    }

    /// **La tabla.**
    pub fn actualizar(&mut self, m: &Mensaje, s: &Senales, fechas: &[Expresion]) -> Resultado {
        if let Some(c) = s.cita
            && c >= SI
            && let Some(f) = fechas.get(s.cual)
        {
            if s.acordada >= SI {
                return self.poner(Evento {
                    titulo: crate::recortar(&m.texto, 80),
                    momento: f.momento,
                    expresion: f.texto.clone(),
                    aceptado: false,
                });
            }
            let p = Pendiente {
                de: m.de.clone(),
                texto: m.texto.clone(),
                momento: f.momento,
                expresion: f.texto.clone(),
            };
            self.pendiente = Some(p.clone());
            return Resultado::Pendiente(p);
        }
        if let (Some(a), Some(p)) = (s.acepta, self.pendiente.clone())
            && a >= SI
            && p.de != m.de
        {
            return self.poner(Evento {
                titulo: crate::recortar(&p.texto, 80),
                momento: p.momento,
                expresion: p.expresion,
                aceptado: true,
            });
        }
        match s.cita {
            Some(c) if c > NO => Resultado::Dudosa(c),
            _ => Resultado::Nada,
        }
    }
}

/// **Pregunta por un mensaje** (o nada, si no hay que preguntar).
pub fn preguntar(cliente: &Cliente, p: &Preparada) -> Result<Option<Senales>, Error> {
    let Some(pet) = &p.peticion else {
        return Ok(None);
    };
    Senales::de(&cliente.preguntar(pet)?, p).map(Some)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn m(de: &str, texto: &str) -> Mensaje {
        Mensaje {
            de: de.into(),
            texto: texto.into(),
        }
    }

    fn hoy() -> Fecha {
        Fecha::nueva(2026, 10, 26).unwrap()
    }

    #[test]
    fn la_tabla_propuesta_y_aceptada_en_otro_mensaje() {
        let mut h = Horario::default();
        let a = m(
            "Ana",
            "¿Te va bien el jueves a las 5 para revisar los planos?",
        );
        let p = preparar("Obra", &[], &a, hoy(), None);
        assert!(p.peticion.is_some());
        let s = Senales {
            cita: Some(0.9),
            acordada: 0.2,
            cual: 0,
            acepta: None,
        };
        let Resultado::Pendiente(x) = h.actualizar(&a, &s, &p.fechas) else {
            panic!()
        };
        assert_eq!(x.momento.to_string(), "jue 2026-10-29 17:00");
        // «vale» de otra persona: sin fechas, solo se pregunta si acepta.
        let b = m("Luis", "Vale, allí estaré");
        let p = preparar("Obra", &[a.clone()], &b, hoy(), h.pendiente.as_ref());
        let pet = p.peticion.as_ref().unwrap();
        assert_eq!(pet.questions.len(), 1);
        assert!(pet.questions.contains_key(q::ACEPTA));
        let s = Senales {
            acepta: Some(0.95),
            ..Default::default()
        };
        let Resultado::Evento(e) = h.actualizar(&b, &s, &p.fechas) else {
            panic!()
        };
        assert!(e.aceptado);
        assert!(h.pendiente.is_none());
        // Lo mismo otra vez: repetido.
        let c = m("Ana", "Quedamos el jueves a las 5 entonces");
        let p = preparar("Obra", &[], &c, hoy(), None);
        let s = Senales {
            cita: Some(0.9),
            acordada: 0.9,
            ..Default::default()
        };
        assert!(matches!(
            h.actualizar(&c, &s, &p.fechas),
            Resultado::Repetido(_)
        ));
    }

    #[test]
    fn lo_que_no_es_cita_y_lo_propio() {
        let mut h = Horario::default();
        let a = m("Ana", "Mándame el informe el viernes");
        let p = preparar("Obra", &[], &a, hoy(), None);
        let s = Senales {
            cita: Some(0.1),
            ..Default::default()
        };
        assert_eq!(h.actualizar(&a, &s, &p.fechas), Resultado::Nada);
        let s = Senales {
            cita: Some(0.5),
            ..Default::default()
        };
        assert_eq!(h.actualizar(&a, &s, &p.fechas), Resultado::Dudosa(0.5));
        // Sin fechas ni nada pendiente, no se pregunta.
        assert!(
            preparar("Obra", &[], &m("Ana", "hola"), hoy(), None)
                .peticion
                .is_none()
        );
        // Lo pendiente de uno mismo no se pregunta (no se acepta solo).
        let pend = Pendiente {
            de: "Ana".into(),
            texto: "¿el martes?".into(),
            momento: p.fechas[0].momento,
            expresion: "el viernes".into(),
        };
        assert!(
            preparar(
                "Obra",
                &[],
                &m("Ana", "o cuando puedas"),
                hoy(),
                Some(&pend)
            )
            .peticion
            .is_none()
        );
    }

    #[test]
    fn el_estado_lleva_los_dos_anteriores() {
        let antes = vec![m("A", "uno"), m("B", "dos"), m("A", "tres")];
        let e = estado("Familia", &antes, &m("B", "cuatro"));
        assert_eq!(e["antes"], json!(["B: dos", "A: tres"]));
        assert_eq!(e["mensaje"]["de"], "B");
        assert!(estado("", &[], &m("B", "x")).get("antes").is_none());
    }
}
