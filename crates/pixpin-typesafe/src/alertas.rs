//! **Avisos que interrumpen o esperan** (29-oct-2026, prototipo
//! `avisos_listos`, ver `docs/probar-ideas-sin-interfaz.md`, «Más casos»):
//! llega algo (lo sincronizado de otro aparato, un mensaje de un chat, un
//! aviso de una app) y el usuario esta en una reunion, grabando o
//! presentando. Lo que hace el usuario lo sabe el codigo ([`Actividad`]);
//! Jev solo juzga el aviso (si es automatico, si pide algo ya, si esperar
//! una hora haria dano y, en una reunion, si es de esa reunion).
//!
//! **Tabla** ([`decidir`]), por orden:
//!
//! | condicion | libre | en reunion o grabando | presentando |
//! |---|---|---|---|
//! | `grave` ≥ SI | interrumpir | interrumpir | interrumpir |
//! | `rutina` ≥ SI | a la bandeja | a la bandeja | a la bandeja |
//! | `de_la_reunion` ≥ SI | — | discreto | discreto |
//! | `pide_ya` ≥ SI | mostrar | discreto | para despues |
//! | lo demas | mostrar | para despues | para despues |
//!
//! «Interrumpir» suena y se ve; «discreto», un aviso pequeno sin sonido (y,
//! grabando, nunca suena: se oiria en la grabacion); «para despues», al
//! resumen de cuando acabe; «a la bandeja», sin aviso. Lo dudoso (entre NO
//! y SI) no sube de fila. Sin respuesta de Jev, lo de siempre: mostrar.

use serde_json::{Map, Value, json};

use crate::preguntas::SI;
use crate::preguntas::alertas as q;
use crate::{Cliente, Error, Peticion};

/// Lo que esta haciendo el usuario (lo sabe la app, no Jev).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Actividad {
    Libre,
    /// En una reunion (su titulo, si se sabe).
    EnReunion(String),
    /// Grabando una clase o una nota de voz.
    Grabando,
    /// Compartiendo la pantalla.
    Presentando,
}

/// Lo que llega.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Aviso {
    /// La app o el origen («Chat Obra», «Sincronizar», «Correo»).
    pub app: String,
    /// Quien lo manda, si es una persona.
    pub de: String,
    pub texto: String,
}

pub fn estado(a: &Aviso, act: &Actividad) -> Value {
    let mut m = Map::new();
    if !a.app.trim().is_empty() {
        m.insert("app".into(), json!(crate::recortar(&a.app, 80)));
    }
    if !a.de.trim().is_empty() {
        m.insert("de".into(), json!(crate::recortar(&a.de, 80)));
    }
    m.insert("texto".into(), json!(crate::recortar(&a.texto, 600)));
    let mut e = json!({ "aviso": m });
    if let Actividad::EnReunion(t) = act
        && !t.trim().is_empty()
    {
        e["reunion"] = json!(crate::recortar(t, 120));
    }
    e
}

pub fn peticion(a: &Aviso, act: &Actividad) -> Peticion {
    let e = estado(a, act);
    let con_reunion = e.get("reunion").is_some();
    let mut p = Peticion::nueva(e)
        .con(q::RUTINA, q::rutina())
        .con(q::PIDE_YA, q::pide_ya())
        .con(q::GRAVE, q::grave());
    if con_reunion {
        p = p.con(q::DE_LA_REUNION, q::de_la_reunion());
    }
    p
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Senales {
    pub rutina: f64,
    pub pide_ya: f64,
    pub grave: f64,
    /// `None` si no habia reunion (no se pregunto).
    pub de_la_reunion: Option<f64>,
}

impl Senales {
    pub fn de(r: &crate::Respuestas, con_reunion: bool) -> Result<Senales, Error> {
        Ok(Senales {
            rutina: r.noul(q::RUTINA)?,
            pide_ya: r.noul(q::PIDE_YA)?,
            grave: r.noul(q::GRAVE)?,
            de_la_reunion: if con_reunion {
                Some(r.noul(q::DE_LA_REUNION)?)
            } else {
                None
            },
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Que {
    /// Suena (salvo grabando) y se ve.
    Interrumpir { sonido: bool },
    /// Un aviso normal.
    Mostrar,
    /// Un aviso pequeno, sin sonido.
    Discreto,
    /// Al resumen de cuando acabe.
    ParaDespues,
    /// Sin aviso.
    Bandeja,
}

impl Que {
    pub fn id(self) -> &'static str {
        match self {
            Que::Interrumpir { .. } => "interrumpir",
            Que::Mostrar => "mostrar",
            Que::Discreto => "discreto",
            Que::ParaDespues => "para despues",
            Que::Bandeja => "bandeja",
        }
    }
}

/// **La tabla.**
pub fn decidir(act: &Actividad, s: &Senales) -> Que {
    let ocupado = !matches!(act, Actividad::Libre);
    if s.grave >= SI {
        return Que::Interrumpir {
            sonido: !matches!(act, Actividad::Grabando),
        };
    }
    if s.rutina >= SI {
        return Que::Bandeja;
    }
    if !ocupado {
        return Que::Mostrar;
    }
    if s.de_la_reunion.is_some_and(|p| p >= SI) {
        return Que::Discreto;
    }
    if s.pide_ya >= SI && !matches!(act, Actividad::Presentando) {
        return Que::Discreto;
    }
    Que::ParaDespues
}

/// **Pregunta y decide.** `Err`: quien llama lo muestra, como siempre.
pub fn decidir_aviso(
    cliente: &Cliente,
    a: &Aviso,
    act: &Actividad,
) -> Result<(Que, Senales), Error> {
    let p = peticion(a, act);
    let con_reunion = p.questions.contains_key(q::DE_LA_REUNION);
    let s = Senales::de(&cliente.preguntar(&p)?, con_reunion)?;
    Ok((decidir(act, &s), s))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn s(rutina: f64, pide_ya: f64, grave: f64, reunion: Option<f64>) -> Senales {
        Senales {
            rutina,
            pide_ya,
            grave,
            de_la_reunion: reunion,
        }
    }

    #[test]
    fn la_tabla_de_los_avisos() {
        let reunion = Actividad::EnReunion("Obra".into());
        // Lo grave interrumpe siempre; grabando, sin sonido.
        assert_eq!(
            decidir(&reunion, &s(0.1, 0.9, 0.9, Some(0.1))),
            Que::Interrumpir { sonido: true }
        );
        assert_eq!(
            decidir(&Actividad::Grabando, &s(0.1, 0.9, 0.9, None)),
            Que::Interrumpir { sonido: false }
        );
        // Lo automatico, a la bandeja aunque este libre.
        assert_eq!(
            decidir(&Actividad::Libre, &s(0.9, 0.1, 0.1, None)),
            Que::Bandeja
        );
        assert_eq!(
            decidir(&Actividad::Libre, &s(0.1, 0.1, 0.1, None)),
            Que::Mostrar
        );
        // Ocupado: lo de la reunion y lo que pide algo ya, discreto.
        assert_eq!(
            decidir(&reunion, &s(0.1, 0.1, 0.1, Some(0.9))),
            Que::Discreto
        );
        assert_eq!(
            decidir(&reunion, &s(0.1, 0.8, 0.2, Some(0.1))),
            Que::Discreto
        );
        assert_eq!(
            decidir(&reunion, &s(0.1, 0.5, 0.5, Some(0.5))),
            Que::ParaDespues
        );
        // Presentando, lo que pide algo ya espera (se veria en la pantalla).
        assert_eq!(
            decidir(&Actividad::Presentando, &s(0.1, 0.9, 0.2, None)),
            Que::ParaDespues
        );
    }

    #[test]
    fn la_reunion_solo_va_si_hay() {
        let a = Aviso {
            app: "Chat".into(),
            de: "Ana".into(),
            texto: "llego tarde".into(),
        };
        let p = peticion(&a, &Actividad::EnReunion("Obra".into()));
        assert!(p.questions.contains_key(q::DE_LA_REUNION));
        assert_eq!(p.state["reunion"], "Obra");
        let p = peticion(&a, &Actividad::Grabando);
        assert!(!p.questions.contains_key(q::DE_LA_REUNION));
        assert!(p.state.get("reunion").is_none());
        let sin_de = estado(
            &Aviso {
                app: "Sincronizar".into(),
                de: String::new(),
                texto: "listo".into(),
            },
            &Actividad::Libre,
        );
        assert!(sin_de["aviso"].get("de").is_none());
    }
}
