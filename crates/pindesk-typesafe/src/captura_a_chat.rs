//! **A que chat va una captura** (29-oct-2026, prototipo `captura_a_chat`,
//! ver `docs/probar-ideas-sin-interfaz.md`, «Más casos»): al hacer una
//! captura, con el texto que el OCR saca de ella y el titulo de la ventana,
//! a cual de los chats del usuario pertenece. Jev dice si es de alguno
//! (Noul) y de cual (Choice con el nombre y de que trata cada chat); lo que
//! se hace lo decide la tabla.
//!
//! **Tabla** ([`decidir`]), `p1` y `p2` las dos opciones mas probables:
//!
//! | `de_algun_chat` | `p1` | se hace |
//! |---|---|---|
//! | ≥ SI | ≥ [`SEGURO`] | se guarda en ese chat (con deshacer) |
//! | ≥ SI | < [`SEGURO`] | se sugieren los dos primeros |
//! | entre NO y SI | ≥ [`SEGURO`] | se sugiere el primero |
//! | entre NO y SI | < [`SEGURO`] | nada: queda en Capturas |
//! | ≤ NO | — | nada: queda en Capturas |
//!
//! Sin chats no se pregunta; con uno solo, basta el Noul (la Choice no va).

use serde_json::{Value, json};

use crate::preguntas::captura_a_chat as q;
use crate::preguntas::{NO, SI};
use crate::{Cliente, Error, Peticion};

/// Por encima, la Choice es clara.
pub const SEGURO: f64 = 0.6;

/// Lo que se sabe de la captura.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Captura {
    /// El titulo de la ventana capturada.
    pub ventana: String,
    /// Lo que leyo el OCR.
    pub texto: String,
}

/// Un chat del usuario: su nombre y, si lo hay, de que trata (su
/// descripcion o los titulos de lo ultimo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chat {
    pub nombre: String,
    pub trata: String,
}

impl Chat {
    fn rotulo(&self) -> String {
        if self.trata.trim().is_empty() {
            self.nombre.clone()
        } else {
            format!("{}: {}", self.nombre, self.trata.trim())
        }
    }
}

/// Lo que va del texto del OCR (una captura de una pagina entera es mucho).
const LARGO: usize = 1500;

pub fn estado(c: &Captura, chats: &[Chat]) -> Value {
    let mut captura = serde_json::Map::new();
    if !c.ventana.trim().is_empty() {
        captura.insert("ventana".into(), json!(crate::recortar(&c.ventana, 200)));
    }
    captura.insert("texto".into(), json!(crate::recortar(&c.texto, LARGO)));
    json!({
        "captura": captura,
        "chats": chats.iter().map(|c| crate::recortar(&c.rotulo(), 200)).collect::<Vec<_>>(),
    })
}

/// La peticion y las claves de los chats (en su orden).
pub fn peticion(c: &Captura, chats: &[Chat]) -> (Peticion, Vec<String>) {
    let opciones =
        crate::claves_unicas(&chats.iter().map(|c| c.nombre.clone()).collect::<Vec<_>>());
    let rotulos: Vec<(String, String)> = opciones
        .iter()
        .zip(chats)
        .map(|((k, _), c)| (k.clone(), c.rotulo()))
        .collect();
    let mut p = Peticion::nueva(estado(c, chats)).con(q::DE_ALGUNO, q::de_alguno());
    if chats.len() > 1 {
        p = p.con(q::CHAT, q::chat(&rotulos));
    }
    (p, opciones.into_iter().map(|(k, _)| k).collect())
}

/// Lo que dijo Jev: la probabilidad de «es de alguno» y la de cada chat, de
/// mas a menos probable (por indice de chat).
#[derive(Debug, Clone, PartialEq)]
pub struct Senales {
    pub de_alguno: f64,
    pub chats: Vec<(usize, f64)>,
}

impl Senales {
    pub fn de(r: &crate::Respuestas, claves: &[String]) -> Result<Senales, Error> {
        let de_alguno = r.noul(q::DE_ALGUNO)?;
        let mut chats: Vec<(usize, f64)> = if claves.len() > 1 {
            let (_, probs) = r.choice(q::CHAT)?;
            claves
                .iter()
                .enumerate()
                .map(|(i, k)| (i, probs.get(k).copied().unwrap_or(0.0)))
                .collect()
        } else {
            claves.iter().enumerate().map(|(i, _)| (i, 1.0)).collect()
        };
        chats.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        Ok(Senales { de_alguno, chats })
    }
}

/// Lo que se hace con la captura.
#[derive(Debug, Clone, PartialEq)]
pub enum Destino {
    /// Se guarda en este chat (por indice), con deshacer.
    Guardar(usize),
    /// Se le sugieren al usuario estos chats, del mas al menos probable.
    Sugerir(Vec<usize>),
    /// Queda en Capturas, sin chat.
    Ninguno,
}

/// **La tabla.**
pub fn decidir(s: &Senales) -> Destino {
    let Some(&(primero, p1)) = s.chats.first() else {
        return Destino::Ninguno;
    };
    if s.de_alguno <= NO {
        return Destino::Ninguno;
    }
    if s.de_alguno >= SI {
        if p1 >= SEGURO {
            Destino::Guardar(primero)
        } else {
            Destino::Sugerir(s.chats.iter().take(2).map(|(i, _)| *i).collect())
        }
    } else if p1 >= SEGURO {
        Destino::Sugerir(vec![primero])
    } else {
        Destino::Ninguno
    }
}

/// **Pregunta y decide.** Sin chats, `Ninguno` sin preguntar.
pub fn elegir(cliente: &Cliente, c: &Captura, chats: &[Chat]) -> Result<(Destino, Senales), Error> {
    if chats.is_empty() {
        return Ok((
            Destino::Ninguno,
            Senales {
                de_alguno: 0.0,
                chats: Vec::new(),
            },
        ));
    }
    let (p, claves) = peticion(c, chats);
    let s = Senales::de(&cliente.preguntar(&p)?, &claves)?;
    Ok((decidir(&s), s))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::grabadas;
    use crate::transporte::Grabado;
    use std::sync::Arc;

    fn s(de_alguno: f64, chats: &[f64]) -> Senales {
        let mut v: Vec<(usize, f64)> = chats.iter().copied().enumerate().collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1));
        Senales {
            de_alguno,
            chats: v,
        }
    }

    #[test]
    fn la_tabla_de_la_captura() {
        assert_eq!(decidir(&s(0.9, &[0.1, 0.8, 0.1])), Destino::Guardar(1));
        assert_eq!(
            decidir(&s(0.9, &[0.45, 0.4, 0.15])),
            Destino::Sugerir(vec![0, 1])
        );
        assert_eq!(
            decidir(&s(0.5, &[0.1, 0.1, 0.8])),
            Destino::Sugerir(vec![2])
        );
        assert_eq!(decidir(&s(0.5, &[0.4, 0.3, 0.3])), Destino::Ninguno);
        assert_eq!(decidir(&s(0.1, &[0.9, 0.05, 0.05])), Destino::Ninguno);
        // Los umbrales son inclusivos.
        assert_eq!(decidir(&s(0.7, &[0.6, 0.4])), Destino::Guardar(0));
        assert_eq!(decidir(&s(0.3, &[0.9, 0.1])), Destino::Ninguno);
    }

    fn chats() -> Vec<Chat> {
        vec![
            Chat {
                nombre: "Obra Calle Mayor".into(),
                trata: "reforma del piso: planos, presupuestos".into(),
            },
            Chat {
                nombre: "Cálculo I".into(),
                trata: String::new(),
            },
        ]
    }

    #[test]
    fn el_estado_y_las_opciones() {
        let c = Captura {
            ventana: "presupuesto.pdf - Visor".into(),
            texto: "Partida 3: alicatado del baño".into(),
        };
        let (p, claves) = peticion(&c, &chats());
        assert_eq!(claves, vec!["obra_calle_mayor", "calculo_i"]);
        let v: Value = serde_json::from_slice(&p.cuerpo().unwrap()).unwrap();
        assert_eq!(v["state"]["captura"]["ventana"], "presupuesto.pdf - Visor");
        assert_eq!(
            v["state"]["chats"][0],
            "Obra Calle Mayor: reforma del piso: planos, presupuestos"
        );
        assert_eq!(v["state"]["chats"][1], "Cálculo I");
        assert_eq!(v["questions"]["chat"]["type"], "choice");
        // Con un chat solo, basta el Noul.
        let (p, _) = peticion(&c, &chats()[..1]);
        assert_eq!(p.questions.len(), 1);
        // Sin ventana, el campo no va.
        let e = estado(
            &Captura {
                ventana: " ".into(),
                texto: "x".into(),
            },
            &chats(),
        );
        assert!(e["captura"].get("ventana").is_none());
    }

    #[test]
    fn elige_con_respuestas_grabadas() {
        let t = Arc::new(Grabado::nuevo(vec![Grabado::ok(&grabadas::respuesta(&[
            grabadas::noul(q::DE_ALGUNO, 0.93),
            grabadas::choice(q::CHAT, "obra_calle_mayor", &["calculo_i"]),
        ]))]));
        let c = Cliente::con_transporte("k", t).con_cache_propio();
        let (d, s) = elegir(&c, &Captura::default(), &chats()).unwrap();
        assert_eq!(d, Destino::Guardar(0));
        assert_eq!(s.chats[0], (0, 0.8));
        // Sin chats no se pregunta.
        let vacio = Cliente::con_transporte("k", Arc::new(Grabado::default()));
        assert_eq!(
            elegir(&vacio, &Captura::default(), &[]).unwrap().0,
            Destino::Ninguno
        );
    }
}
