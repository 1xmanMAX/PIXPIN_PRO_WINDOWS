//! **Lo que viaja**: la peticion `POST /v1/systemone` y su respuesta, tal
//! como las describe el OpenAPI de TypeSafe (`https://api.typesafe.ai/openapi.json`,
//! version 0.2.0, leido el 28-oct-2026).
//!
//! - [`Peticion`]: `model`, `state` (JSON con campos con nombre), `questions`
//!   (cada una con el nombre que se elija) y, si hay imagenes, `assets`.
//! - [`Pregunta`]: los cuatro tipos. `noul` (la probabilidad de que algo sea
//!   verdad, con `criteria: {true, false}` opcional), `choice` (una opcion de
//!   `criteria: {nombre: descripcion}`), `score` (`criteria: [niveles]`, del 0
//!   en adelante, de 1 a 10) y `box` (donde esta algo en una imagen).
//! - [`Respuestas`]: `model` (el id con version que contesto), `answers` por
//!   el mismo nombre de la pregunta y `usage` (tokens de entrada, que son los
//!   que se pagan).
//!
//! Las opciones de una `choice` se mandan **en el orden en que se escriben**
//! (el orden pesa, ver la pagina de «jaggedness»): por eso la peticion se
//! escribe directamente a bytes y no pasa por un `serde_json::Value`, que las
//! ordenaria por nombre.

use std::collections::BTreeMap;

use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Error;

/// El modelo por omision: el alias estable de Jev. Cada respuesta dice el id
/// con version que contesto (`jev-1.14.0`), que es lo que se guarda.
pub const MODELO: &str = "jev-latest";

/// **Una pregunta**, de uno de los cuatro tipos.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Pregunta {
    /// ¿Es verdad? Contesta la probabilidad de «si».
    Noul {
        instructions: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        criteria: Option<CriteriosNoul>,
    },
    /// Una de varias opciones, con la probabilidad de cada una.
    Choice {
        instructions: String,
        #[serde(serialize_with = "en_orden")]
        criteria: Vec<(String, String)>,
    },
    /// Un nivel de una escala descrita nivel a nivel (del 0 en adelante).
    Score {
        instructions: String,
        criteria: Vec<String>,
    },
    /// Donde esta algo en una imagen (pide la cabecera de las imagenes).
    Box { instructions: String },
}

/// Lo que cuenta como «si» y como «no» en un Noul.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CriteriosNoul {
    #[serde(rename = "true")]
    pub si: String,
    #[serde(rename = "false")]
    pub no: String,
}

fn en_orden<S: Serializer>(v: &[(String, String)], s: S) -> Result<S::Ok, S::Error> {
    let mut m = s.serialize_map(Some(v.len()))?;
    for (k, d) in v {
        m.serialize_entry(k, d)?;
    }
    m.end()
}

impl Pregunta {
    /// Un Noul con sus dos criterios.
    pub fn noul(instrucciones: &str, si: &str, no: &str) -> Pregunta {
        Pregunta::Noul {
            instructions: instrucciones.to_string(),
            criteria: Some(CriteriosNoul {
                si: si.to_string(),
                no: no.to_string(),
            }),
        }
    }

    /// Un Score con sus niveles, del 0 al ultimo.
    pub fn score(instrucciones: &str, niveles: &[&str]) -> Pregunta {
        Pregunta::Score {
            instructions: instrucciones.to_string(),
            criteria: niveles.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Una Choice con sus opciones en este orden.
    pub fn choice(instrucciones: &str, opciones: &[(&str, &str)]) -> Pregunta {
        Pregunta::Choice {
            instructions: instrucciones.to_string(),
            criteria: opciones
                .iter()
                .map(|(k, d)| (k.to_string(), d.to_string()))
                .collect(),
        }
    }

    /// Un Box: donde esta lo que se dice.
    pub fn caja(instrucciones: &str) -> Pregunta {
        Pregunta::Box {
            instructions: instrucciones.to_string(),
        }
    }

    pub fn tipo(&self) -> &'static str {
        match self {
            Pregunta::Noul { .. } => "noul",
            Pregunta::Choice { .. } => "choice",
            Pregunta::Score { .. } => "score",
            Pregunta::Box { .. } => "box",
        }
    }
}

/// Una imagen de la peticion: va en `assets` con su referencia, y la
/// referencia se escribe donde va la imagen (en `state` o en una pregunta).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imagen {
    /// `png`, `jpg` o `jpeg`.
    pub formato: String,
    pub bytes: Vec<u8>,
}

impl Serialize for Imagen {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use base64::Engine as _;
        let mut m = s.serialize_map(Some(3))?;
        m.serialize_entry("type", "image")?;
        m.serialize_entry("format", &self.formato)?;
        m.serialize_entry(
            "data_base64",
            &base64::engine::general_purpose::STANDARD.encode(&self.bytes),
        )?;
        m.end()
    }
}

/// **Una peticion**: el estado y las preguntas que se le hacen.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Peticion {
    pub model: String,
    pub state: Value,
    pub questions: BTreeMap<String, Pregunta>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub assets: BTreeMap<String, Imagen>,
}

impl Peticion {
    /// Con el modelo por omision y sin preguntas todavia.
    pub fn nueva(estado: Value) -> Peticion {
        Peticion {
            model: MODELO.to_string(),
            state: estado,
            questions: BTreeMap::new(),
            assets: BTreeMap::new(),
        }
    }

    /// Anade una pregunta con su nombre (el de la respuesta).
    pub fn con(mut self, nombre: &str, p: Pregunta) -> Peticion {
        self.questions.insert(nombre.to_string(), p);
        self
    }

    /// Anade una imagen con su referencia.
    pub fn con_imagen(mut self, referencia: &str, i: Imagen) -> Peticion {
        self.assets.insert(referencia.to_string(), i);
        self
    }

    /// Si hay que mandar `TypeSafe-Danger-Zone-Images: true`: con imagenes o
    /// con alguna pregunta Box.
    pub fn con_imagenes(&self) -> bool {
        !self.assets.is_empty()
            || self
                .questions
                .values()
                .any(|p| matches!(p, Pregunta::Box { .. }))
    }

    /// El cuerpo exacto que se manda.
    pub fn cuerpo(&self) -> Result<Vec<u8>, Error> {
        if self.questions.is_empty() {
            return Err(Error::Formato("una peticion sin preguntas".into()));
        }
        serde_json::to_vec(self).map_err(|e| Error::Formato(e.to_string()))
    }

    /// El mismo cuerpo para el registro: las imagenes cambiadas por su
    /// tamano (el resto, tal cual se manda).
    pub fn para_el_registro(&self) -> String {
        let mut copia = self.clone();
        let mut tamanos = Vec::new();
        for (k, i) in &mut copia.assets {
            tamanos.push(format!("{k}: {} bytes {}", i.bytes.len(), i.formato));
            i.bytes.clear();
        }
        let cuerpo = serde_json::to_string(&copia).unwrap_or_default();
        if tamanos.is_empty() {
            cuerpo
        } else {
            format!("{cuerpo} [imagenes: {}]", tamanos.join(", "))
        }
    }
}

/// Una caja en una imagen: sus bordes en fracciones del alto y del ancho.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Caja {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

/// **Una respuesta**, del mismo tipo que su pregunta.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Respuesta {
    Noul {
        noul: f64,
    },
    Choice {
        choice: String,
        confidence: f64,
        probabilities: BTreeMap<String, f64>,
    },
    Score {
        score: f64,
        confidence: f64,
        #[serde(default)]
        legend: BTreeMap<String, Value>,
        probabilities: BTreeMap<String, f64>,
    },
    Box {
        #[serde(rename = "box")]
        caja: Caja,
        confidence: Option<f64>,
    },
}

/// Los tokens de la peticion. Se pagan los de entrada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub struct Uso {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// **Lo que contesto** una peticion entera.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Respuestas {
    /// El id con version que contesto (`jev-1.14.0`).
    pub model: String,
    pub answers: BTreeMap<String, Respuesta>,
    #[serde(default)]
    pub usage: Uso,
}

/// Un Score leido: el valor esperado y la probabilidad de cada nivel, por
/// su posicion (0, 1, 2...).
#[derive(Debug, Clone, PartialEq)]
pub struct Escala {
    pub valor: f64,
    pub confianza: f64,
    pub niveles: Vec<f64>,
}

impl Escala {
    /// La probabilidad de que el nivel sea `n` o mas.
    pub fn al_menos(&self, n: usize) -> f64 {
        self.niveles.iter().skip(n).sum()
    }

    /// El nivel mas probable.
    pub fn mas_probable(&self) -> usize {
        self.niveles
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map_or(0, |(i, _)| i)
    }
}

/// Una probabilidad que tiene sentido (0..=1). Un numero fuera de ahi es una
/// respuesta rota, no un «no».
fn probabilidad(nombre: &str, p: f64) -> Result<f64, Error> {
    if p.is_finite() && (0.0..=1.0).contains(&p) {
        Ok(p)
    } else {
        Err(Error::Formato(format!(
            "{nombre}: probabilidad fuera de rango ({p})"
        )))
    }
}

impl Respuestas {
    /// Lee el JSON de la respuesta.
    pub fn de_json(bytes: &[u8]) -> Result<Respuestas, Error> {
        serde_json::from_slice(bytes).map_err(|e| Error::Formato(e.to_string()))
    }

    fn una(&self, nombre: &str) -> Result<&Respuesta, Error> {
        self.answers
            .get(nombre)
            .ok_or_else(|| Error::FaltaRespuesta(nombre.to_string()))
    }

    /// La probabilidad de «si» del Noul `nombre`. Falta o es de otro tipo:
    /// error (nunca un «no» callado).
    pub fn noul(&self, nombre: &str) -> Result<f64, Error> {
        match self.una(nombre)? {
            Respuesta::Noul { noul } => probabilidad(nombre, *noul),
            otra => Err(Error::OtroTipo {
                nombre: nombre.to_string(),
                esperado: "noul",
                llego: tipo_de(otra),
            }),
        }
    }

    /// La opcion elegida de la Choice `nombre` y la probabilidad de cada una.
    pub fn choice(&self, nombre: &str) -> Result<(&str, &BTreeMap<String, f64>), Error> {
        match self.una(nombre)? {
            Respuesta::Choice {
                choice,
                probabilities,
                ..
            } => Ok((choice.as_str(), probabilities)),
            otra => Err(Error::OtroTipo {
                nombre: nombre.to_string(),
                esperado: "choice",
                llego: tipo_de(otra),
            }),
        }
    }

    /// El Score `nombre`, con `n` niveles (los que se pidieron). Un nivel
    /// que no llega cuenta 0; las claves que no son un nivel, error.
    pub fn score(&self, nombre: &str, n: usize) -> Result<Escala, Error> {
        match self.una(nombre)? {
            Respuesta::Score {
                score,
                confidence,
                probabilities,
                ..
            } => {
                let mut niveles = vec![0.0; n];
                for (k, p) in probabilities {
                    let i: usize = k
                        .parse()
                        .ok()
                        .filter(|i| *i < n)
                        .ok_or_else(|| Error::Formato(format!("{nombre}: nivel «{k}»")))?;
                    niveles[i] = probabilidad(nombre, *p)?;
                }
                if !score.is_finite() {
                    return Err(Error::Formato(format!("{nombre}: score no es un numero")));
                }
                Ok(Escala {
                    valor: *score,
                    confianza: *confidence,
                    niveles,
                })
            }
            otra => Err(Error::OtroTipo {
                nombre: nombre.to_string(),
                esperado: "score",
                llego: tipo_de(otra),
            }),
        }
    }

    /// La caja del Box `nombre`.
    pub fn caja(&self, nombre: &str) -> Result<Caja, Error> {
        match self.una(nombre)? {
            Respuesta::Box { caja, .. } => Ok(*caja),
            otra => Err(Error::OtroTipo {
                nombre: nombre.to_string(),
                esperado: "box",
                llego: tipo_de(otra),
            }),
        }
    }
}

fn tipo_de(r: &Respuesta) -> &'static str {
    match r {
        Respuesta::Noul { .. } => "noul",
        Respuesta::Choice { .. } => "choice",
        Respuesta::Score { .. } => "score",
        Respuesta::Box { .. } => "box",
    }
}

/// Un modelo de `GET /v1/models`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Modelo {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub release_date: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct ListaDeModelos {
    pub models: Vec<Modelo>,
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    #[test]
    fn la_peticion_sale_como_dice_el_openapi() {
        let p = Peticion::nueva(json!({"linea": "para el lunes el ejercicio 3"}))
            .con(
                "tarea",
                Pregunta::noul("Is it a task?", "Someone must do it.", "Nobody must."),
            )
            .con(
                "tono",
                Pregunta::choice("Tone?", &[("zeta", "z"), ("alfa", "a")]),
            )
            .con("nivel", Pregunta::score("How much?", &["Low", "High"]));
        let v: Value = serde_json::from_slice(&p.cuerpo().unwrap()).unwrap();
        assert_eq!(v["model"], "jev-latest");
        assert_eq!(v["state"]["linea"], "para el lunes el ejercicio 3");
        assert_eq!(v["questions"]["tarea"]["type"], "noul");
        assert_eq!(
            v["questions"]["tarea"]["criteria"]["true"],
            "Someone must do it."
        );
        assert_eq!(v["questions"]["tarea"]["criteria"]["false"], "Nobody must.");
        assert_eq!(v["questions"]["nivel"]["criteria"], json!(["Low", "High"]));
        assert!(v.get("assets").is_none());
        assert!(!p.con_imagenes());
        // Las opciones de la choice, en el orden escrito (no por nombre).
        let texto = String::from_utf8(p.cuerpo().unwrap()).unwrap();
        assert!(texto.find("\"zeta\"").unwrap() < texto.find("\"alfa\"").unwrap());
    }

    #[test]
    fn una_imagen_va_en_base64_y_no_al_registro() {
        let p = Peticion::nueva(json!({"foto": "img-1"}))
            .con("donde", Pregunta::caja("Locate the red rectangle."))
            .con_imagen(
                "img-1",
                Imagen {
                    formato: "png".into(),
                    bytes: vec![1, 2, 3],
                },
            );
        assert!(p.con_imagenes());
        let v: Value = serde_json::from_slice(&p.cuerpo().unwrap()).unwrap();
        assert_eq!(v["assets"]["img-1"]["data_base64"], "AQID");
        assert_eq!(v["assets"]["img-1"]["type"], "image");
        assert_eq!(v["questions"]["donde"]["type"], "box");
        let r = p.para_el_registro();
        assert!(!r.contains("AQID"));
        assert!(r.contains("3 bytes png"));
    }

    #[test]
    fn se_leen_las_respuestas_del_ejemplo_del_openapi() {
        let r = Respuestas::de_json(
            br#"{"model":"jev-1.14.0","answers":{
                "billing":{"type":"noul","noul":0.98},
                "tone":{"type":"choice","choice":"calm","confidence":0.7,"probabilities":{"angry":0.15,"calm":0.8,"excited":0.05}},
                "urgency":{"type":"score","score":1.7,"confidence":0.55,"legend":{"0":"Can wait","1":"This week","2":"Today"},"probabilities":{"0":0.1,"1":0.1,"2":0.8}},
                "red":{"type":"box","box":{"top":0.2,"bottom":0.66,"left":0.19,"right":0.68},"confidence":null}
            },"usage":{"input_tokens":120,"output_tokens":12}}"#,
        )
        .unwrap();
        assert_eq!(r.model, "jev-1.14.0");
        assert_eq!(r.usage.input_tokens, 120);
        assert_eq!(r.noul("billing").unwrap(), 0.98);
        assert_eq!(r.choice("tone").unwrap().0, "calm");
        let e = r.score("urgency", 3).unwrap();
        assert_eq!(e.mas_probable(), 2);
        assert!((e.al_menos(1) - 0.9).abs() < 1e-9);
        assert_eq!(r.caja("red").unwrap().left, 0.19);
        // Lo que falta o es de otro tipo es un error, no un «no».
        assert!(matches!(r.noul("nada"), Err(Error::FaltaRespuesta(_))));
        assert!(matches!(r.noul("tone"), Err(Error::OtroTipo { .. })));
        assert!(matches!(r.score("urgency", 2), Err(Error::Formato(_))));
    }

    #[test]
    fn una_probabilidad_rota_es_un_error() {
        let r = Respuestas::de_json(
            br#"{"model":"jev","answers":{"x":{"type":"noul","noul":1.5}},"usage":{"input_tokens":1,"output_tokens":1}}"#,
        )
        .unwrap();
        assert!(matches!(r.noul("x"), Err(Error::Formato(_))));
    }
}
