//! **La mejor foto de una rafaga** (29-oct-2026, prototipo `mejor_foto`,
//! ver `docs/probar-ideas-sin-interfaz.md`, «Más casos»): de varias fotos
//! seguidas de lo mismo (la camara del movil, la de PixPin), cual se
//! queda. Las fotos van en el estado una vez (`fotos.foto_1`...) y por cada
//! una cuatro Nouls (nitida, ojos cerrados, bien expuesta, torcida) mas
//! una Choice de cual se quedaria una persona. Una peticion por rafaga.
//!
//! **Tabla** ([`decidir`]):
//!
//! | por foto | se hace |
//! |---|---|
//! | `nitida` ≤ NO | se descarta: movida |
//! | `ojos_cerrados` ≥ SI | se descarta: ojos cerrados |
//! | `expuesta` ≤ NO | se descarta: oscura o quemada |
//! | `torcida` ≥ SI | se descarta: torcida |
//! | lo demas | candidata |
//!
//! Se elige la candidata con mas probabilidad en `mejor` (empate: la mas
//! nitida). Si todas se descartan, la mas probable de `mejor`, avisando de
//! que ninguna esta bien. Los motivos se dicen todos (una foto puede estar
//! movida y oscura). Lo dudoso (entre NO y SI) no descarta.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::preguntas::mejor_foto as q;
use crate::preguntas::{NO, SI};
use crate::{Cliente, Error, Imagen, Peticion};

/// La referencia de una imagen en `assets`: su huella (siempre la misma
/// para los mismos bytes; asi la peticion repetida sale del cache).
pub fn referencia(i: &Imagen) -> String {
    let h = Sha256::digest(&i.bytes);
    format!(
        "img-{}",
        h.iter()
            .take(8)
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

/// La peticion de una rafaga (las primeras [`q::COMO_MUCHO`] fotos).
pub fn peticion(fotos: &[Imagen]) -> Peticion {
    let fotos = &fotos[..fotos.len().min(q::COMO_MUCHO)];
    let mut estado = Map::new();
    let mut p = Peticion::nueva(Value::Null);
    for (i, f) in fotos.iter().enumerate() {
        let r = format!("{}-{}", referencia(f), i + 1);
        estado.insert(q::clave(i), json!(r));
        p = p
            .con_imagen(&r, f.clone())
            .con(&q::nitida(i), q::es_nitida(i))
            .con(&q::ojos(i), q::ojos_cerrados(i))
            .con(&q::expuesta(i), q::bien_expuesta(i))
            .con(&q::torcida(i), q::esta_torcida(i));
    }
    p.state = json!({ "fotos": estado });
    if fotos.len() > 1 {
        p = p.con(q::MEJOR, q::mejor(fotos.len()));
    }
    p
}

/// Lo que Jev dijo de una foto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeUna {
    pub nitida: f64,
    pub ojos_cerrados: f64,
    pub expuesta: f64,
    pub torcida: f64,
    /// Su probabilidad en `mejor` (1 si es la unica).
    pub mejor: f64,
}

pub fn senales(r: &crate::Respuestas, n: usize) -> Result<Vec<DeUna>, Error> {
    let n = n.min(q::COMO_MUCHO);
    let probs: BTreeMap<String, f64> = if n > 1 {
        r.choice(q::MEJOR)?.1.clone()
    } else {
        BTreeMap::new()
    };
    (0..n)
        .map(|i| {
            Ok(DeUna {
                nitida: r.noul(&q::nitida(i))?,
                ojos_cerrados: r.noul(&q::ojos(i))?,
                expuesta: r.noul(&q::expuesta(i))?,
                torcida: r.noul(&q::torcida(i))?,
                mejor: if n > 1 {
                    probs.get(&q::clave(i)).copied().unwrap_or(0.0)
                } else {
                    1.0
                },
            })
        })
        .collect()
}

/// Por que se descarta una foto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motivo {
    Movida,
    OjosCerrados,
    MalExpuesta,
    Torcida,
}

impl Motivo {
    pub fn id(self) -> &'static str {
        match self {
            Motivo::Movida => "movida",
            Motivo::OjosCerrados => "ojos cerrados",
            Motivo::MalExpuesta => "mal expuesta",
            Motivo::Torcida => "torcida",
        }
    }
}

/// Los motivos para descartar una foto (vacio: candidata).
pub fn motivos(s: &DeUna) -> Vec<Motivo> {
    let mut v = Vec::new();
    if s.nitida <= NO {
        v.push(Motivo::Movida);
    }
    if s.ojos_cerrados >= SI {
        v.push(Motivo::OjosCerrados);
    }
    if s.expuesta <= NO {
        v.push(Motivo::MalExpuesta);
    }
    if s.torcida >= SI {
        v.push(Motivo::Torcida);
    }
    v
}

/// Lo decidido de una rafaga.
#[derive(Debug, Clone, PartialEq)]
pub struct Eleccion {
    /// La que se queda (indice).
    pub elegida: usize,
    /// `false`: todas tenian algun fallo y se queda la menos mala.
    pub buena: bool,
    /// Las descartadas, con sus motivos.
    pub descartadas: Vec<(usize, Vec<Motivo>)>,
}

/// **La tabla.** `None` sin fotos.
pub fn decidir(s: &[DeUna]) -> Option<Eleccion> {
    let descartadas: Vec<(usize, Vec<Motivo>)> = s
        .iter()
        .enumerate()
        .map(|(i, x)| (i, motivos(x)))
        .filter(|(_, m)| !m.is_empty())
        .collect();
    let mejor_de = |idx: &mut dyn Iterator<Item = usize>| {
        idx.max_by(|a, b| {
            s[*a]
                .mejor
                .total_cmp(&s[*b].mejor)
                .then(s[*a].nitida.total_cmp(&s[*b].nitida))
                .then(b.cmp(a))
        })
    };
    let candidatas = (0..s.len()).filter(|i| !descartadas.iter().any(|(d, _)| d == i));
    if let Some(e) = mejor_de(&mut candidatas.into_iter()) {
        return Some(Eleccion {
            elegida: e,
            buena: true,
            descartadas,
        });
    }
    let e = mejor_de(&mut (0..s.len()))?;
    Some(Eleccion {
        elegida: e,
        buena: false,
        descartadas,
    })
}

/// **Pregunta y decide.**
pub fn elegir(
    cliente: &Cliente,
    fotos: &[Imagen],
) -> Result<(Option<Eleccion>, Vec<DeUna>), Error> {
    if fotos.is_empty() {
        return Ok((None, Vec::new()));
    }
    let r = cliente.preguntar(&peticion(fotos))?;
    let s = senales(&r, fotos.len())?;
    Ok((decidir(&s), s))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn u(nitida: f64, ojos: f64, expuesta: f64, torcida: f64, mejor: f64) -> DeUna {
        DeUna {
            nitida,
            ojos_cerrados: ojos,
            expuesta,
            torcida,
            mejor,
        }
    }

    #[test]
    fn la_tabla_de_la_rafaga() {
        // La segunda es la buena; la primera movida, la tercera con los ojos
        // cerrados.
        let e = decidir(&[
            u(0.1, 0.1, 0.9, 0.1, 0.5),
            u(0.9, 0.1, 0.9, 0.1, 0.3),
            u(0.9, 0.9, 0.9, 0.1, 0.2),
        ])
        .unwrap();
        assert_eq!(e.elegida, 1);
        assert!(e.buena);
        assert_eq!(
            e.descartadas,
            vec![(0, vec![Motivo::Movida]), (2, vec![Motivo::OjosCerrados])]
        );
        // Todas mal: la mas probable de `mejor`, avisando.
        let e = decidir(&[u(0.1, 0.1, 0.1, 0.1, 0.3), u(0.2, 0.1, 0.9, 0.8, 0.7)]).unwrap();
        assert_eq!(e.elegida, 1);
        assert!(!e.buena);
        assert_eq!(
            e.descartadas[0].1,
            vec![Motivo::Movida, Motivo::MalExpuesta]
        );
        // Lo dudoso no descarta; el empate en `mejor` lo gana la mas nitida.
        let e = decidir(&[u(0.5, 0.5, 0.5, 0.5, 0.5), u(0.9, 0.1, 0.9, 0.1, 0.5)]).unwrap();
        assert_eq!(e.elegida, 1);
        assert!(e.descartadas.is_empty());
        assert!(decidir(&[]).is_none());
    }

    #[test]
    fn las_fotos_van_una_vez_en_el_estado() {
        let f = |b: u8| Imagen {
            formato: "png".into(),
            bytes: vec![b; 4],
        };
        let p = peticion(&[f(1), f(2), f(1)]);
        assert_eq!(p.assets.len(), 3);
        assert_eq!(p.questions.len(), 3 * 4 + 1);
        let v: Value = serde_json::from_slice(&p.cuerpo().unwrap()).unwrap();
        let r1 = v["state"]["fotos"]["foto_1"].as_str().unwrap();
        let r3 = v["state"]["fotos"]["foto_3"].as_str().unwrap();
        // La misma imagen dos veces lleva dos referencias distintas.
        assert_ne!(r1, r3);
        assert!(v["assets"].get(r1).is_some());
        assert!(
            v["questions"]["nitida_2"]["instructions"]
                .as_str()
                .unwrap()
                .contains("`fotos.foto_2`")
        );
        assert!(p.con_imagenes());
        // Una sola foto: sin Choice.
        assert!(!peticion(&[f(1)]).questions.contains_key(q::MEJOR));
    }
}
