//! **Lo que viaja con la nube**, en JSON: los cuerpos de `docs/pixpin-nube.md`
//! (§3, §4) y los de la capacidad `deltas1` (`PATCH`, `cambios`).

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::compartidos::ChatCompartido;

/// El aparato tal como lo ve la nube (`hola.aparato`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AparatoNube {
    pub id: String,
    pub nombre: String,
    pub espacio: String,
    pub persona: Option<String>,
}

/// La respuesta de `GET hola`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hola {
    pub protocolo: String,
    pub version: u32,
    pub aparato: AparatoNube,
    /// Lo que sabe ademas del protocolo 1 (`deltas1`). Sin la lista, nada.
    pub capacidades: Vec<String>,
    #[serde(rename = "topeObjeto")]
    pub tope_objeto: u64,
    #[serde(rename = "topeInventario")]
    pub tope_inventario: u64,
    /// El `seq` de la ultima novedad: desde donde mirar.
    pub novedades: i64,
}

impl Hola {
    /// Si la nube sabe `capacidad`.
    pub fn sabe(&self, capacidad: &str) -> bool {
        self.capacidades.iter().any(|c| c == capacidad)
    }
}

/// Un delta de un fichero (`deltas[i]`): el objeto con lo nuevo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delta {
    pub sha256: String,
    pub bytes: u64,
}

/// Una linea del inventario de ficheros.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Archivo {
    pub rel: String,
    pub sha256: String,
    pub bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tocado: Option<i64>,
    /// Con `deltas1`, los deltas encima del original (`[]`: ninguno).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deltas: Option<Vec<Delta>>,
}

impl Archivo {
    pub fn deltas(&self) -> &[Delta] {
        self.deltas.as_deref().unwrap_or(&[])
    }
}

/// Un campo que distingue «no viene» (`None`) de «viene `null`»
/// (`Some(Value::Null)`): el `proyecto` de un parche.
fn presente<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

/// **El inventario de un chat** (§4): lo que se sube con `PUT` y se baja con
/// `GET`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Inventario {
    pub nombre: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tocado: Option<i64>,
    pub mensajes: Vec<Value>,
    pub proyecto: Option<Value>,
    pub archivos: Vec<Archivo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compartido: Option<ChatCompartido>,
    /// Lo que pone la nube al bajarlo.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub espacio: Option<String>,
}

/// Lo que se pone y se quita de una lista en un parche.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Lista<T> {
    pub poner: Vec<T>,
    pub quitar: Vec<String>,
}

impl<T> Default for Lista<T> {
    fn default() -> Self {
        Lista {
            poner: Vec::new(),
            quitar: Vec::new(),
        }
    }
}

impl<T> Lista<T> {
    pub fn vacia(&self) -> bool {
        self.poner.is_empty() && self.quitar.is_empty()
    }
}

/// **Un parche** (`PATCH chats/<chat>`, `deltas1`): solo lo que cambio
/// desde la revision `base`. Es tambien cada entrada de `cambios`, con la
/// `revision` que dejo.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Parche {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub espacio: Option<String>,
    pub base: i64,
    pub tocado: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nombre: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compartido: Option<ChatCompartido>,
    /// Los mensajes enteros, en forma portatil; `quitar`, sus `id` (la
    /// nube pone y quita por `id`, §10.1).
    pub mensajes: Lista<Value>,
    /// Ausente: no cambio. `null`: ya no tiene proyecto.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "presente"
    )]
    pub proyecto: Option<Value>,
    pub archivos: Lista<Archivo>,
    /// En `cambios`: la revision que dejo este parche.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<i64>,
}

impl Parche {
    /// Si no trae nada que cambie el chat.
    pub fn vacio(&self) -> bool {
        self.nombre.is_none()
            && self.compartido.is_none()
            && self.mensajes.vacia()
            && self.proyecto.is_none()
            && self.archivos.vacia()
    }
}

/// Lo que contesta la nube a una subida (`PUT` o `PATCH`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Subido {
    pub ok: bool,
    pub espacio: String,
    pub revision: i64,
    pub faltan: Vec<String>,
}

/// `GET chats/<chat>/cambios`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cambios {
    pub revision: i64,
    pub reiniciar: bool,
    pub cambios: Vec<Parche>,
}

/// Un chat que cambio (`novedades.cambios[i]`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Novedad {
    pub espacio: String,
    pub chat: String,
    pub revision: i64,
    pub compartido: Option<bool>,
    pub borrado: Option<bool>,
}

/// `GET novedades`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Novedades {
    pub seq: i64,
    pub reiniciar: bool,
    pub cambios: Vec<Novedad>,
}

/// Un chat de la lista (`GET chats`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChatDeLaNube {
    pub espacio: String,
    pub chat: String,
    pub nombre: String,
    pub revision: i64,
    pub compartido: Option<bool>,
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    #[test]
    fn el_proyecto_de_un_parche_distingue_ausente_de_nulo() {
        let p: Parche = serde_json::from_value(json!({"base": 1, "tocado": 2})).unwrap();
        assert_eq!(p.proyecto, None);
        assert!(p.vacio());
        let p: Parche =
            serde_json::from_value(json!({"base": 1, "tocado": 2, "proyecto": null})).unwrap();
        assert_eq!(p.proyecto, Some(Value::Null));
        assert!(!p.vacio());
        let t = serde_json::to_value(&p).unwrap();
        assert_eq!(t["proyecto"], Value::Null);
        assert!(t.as_object().unwrap().contains_key("proyecto"));
        let sin = serde_json::to_value(Parche::default()).unwrap();
        assert!(!sin.as_object().unwrap().contains_key("proyecto"));
    }

    #[test]
    fn un_parche_se_escribe_como_dice_el_protocolo() {
        let p = Parche {
            base: 7,
            tocado: 9,
            mensajes: Lista {
                poner: vec![json!({"id": "1"})],
                quitar: vec!["ABCDEFGHJK".into()],
            },
            archivos: Lista {
                poner: vec![Archivo {
                    rel: "pins/draw/anot-X-p0.excalidraw.gz".into(),
                    sha256: "a".repeat(64),
                    bytes: 10,
                    tocado: None,
                    deltas: Some(vec![Delta {
                        sha256: "b".repeat(64),
                        bytes: 3,
                    }]),
                }],
                quitar: vec![],
            },
            ..Parche::default()
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["base"], 7);
        assert_eq!(v["mensajes"]["quitar"][0], "ABCDEFGHJK");
        assert_eq!(v["archivos"]["poner"][0]["deltas"][0]["bytes"], 3);
        assert!(v["archivos"]["poner"][0].get("tocado").is_none());
        assert!(v.get("revision").is_none());
    }

    #[test]
    fn hola_lee_las_capacidades_y_sin_ellas_no_sabe_nada() {
        let h: Hola = serde_json::from_value(json!({
            "protocolo": "pixpin-nube", "version": 1,
            "aparato": {"id": "a1", "nombre": "PC", "espacio": "max", "persona": null},
            "topeObjeto": 1, "topeInventario": 2, "novedades": 5,
            "capacidades": ["deltas1"]
        }))
        .unwrap();
        assert!(h.sabe("deltas1"));
        let h: Hola = serde_json::from_value(json!({"protocolo": "pixpin-nube"})).unwrap();
        assert!(!h.sabe("deltas1"));
    }
}
