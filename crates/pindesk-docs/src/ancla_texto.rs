//! **Las marcas de texto de las paginas que se reajustan** (2-nov-2026):
//! resaltar, raya ondulada, subrayar, tachar y tapar lo elegido leyendo una
//! nota Markdown (el editor de notas) o una pagina web (el visor de HTML).
//!
//! En el PDF y en el Word las marcas son dibujo de la capa de su hoja
//! (`pindesk_motor2d::marcas_texto`): alli el texto no se mueve. Una nota o
//! una pagina se recolocan con el ancho de la ventana (y del movil), asi que
//! una caja no sirve: cada marca guarda **su cita**, como las anotaciones de
//! la W3C (*Web Annotation Data Model*, selectores `TextQuoteSelector` y
//! `TextPositionSelector`), y al abrir se vuelve a buscar en el texto. Las
//! cuentas de buscar estan en la pagina
//! (`herramientas/lector-md/src/marcas-texto.mjs`); aqui, el fichero y como
//! se juntan dos versiones.
//!
//! # El fichero
//!
//! `anot-<uid>.marcas-texto.json` del mensaje (viaja,
//! `pindesk_sincro::anotado::MARCAS_TEXTO`) o, si el documento no es de un
//! chat, `<doc>.pixpin-marcas-texto.json` a su lado:
//!
//! ```json
//! {"version":1,"sobre":"markdown","marcas":[
//!   {"id":"marca-texto-1793582613265","marca":"ondulada","color":"#5BA4F5",
//!    "cuando":1793582613265,
//!    "cita":{"type":"TextQuoteSelector","exact":"el curado empieza",
//!            "prefix":"… y ","suffix":" mañana."},
//!    "posicion":{"type":"TextPositionSelector","start":44,"end":61}}]}
//! ```
//!
//! - `sobre`: el texto en que se cuentan las posiciones. `markdown`: el
//!   `.md` tal cual (las mismas posiciones que los comentarios,
//!   [`crate::md_comentarios`]). `html`: el texto de la pagina, el
//!   `textContent` de `<body>` sin `<script>`, `<style>`, `<noscript>`,
//!   `<template>` ni lo que pone PinDesk.
//! - Las posiciones son unidades UTF-16 (las de JavaScript y Kotlin).
//! - `prefix` y `suffix`: hasta 32 unidades de cada lado.
//! - `marca`: `resaltar`, `ondulada`, `subrayar`, `tachar` o `tapar`
//!   (`pindesk_motor2d::marcas_texto::Marca::nombre`); `color`, uno de los
//!   cinco de la barra.
//! - Lo que no se conoce (de una version mas nueva) se conserva.
//!
//! Una marca cuya cita ya no se encuentra no se pinta pero no se borra.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// La version del formato.
pub const VERSION: u32 = 1;

/// Lo que va delante de los ids (el mismo grupo que las marcas del PDF).
pub const GRUPO: &str = "marca-texto";

/// Las marcas, con los nombres de `pindesk_motor2d::marcas_texto`.
pub const MARCAS: [&str; 5] = ["resaltar", "ondulada", "subrayar", "tachar", "tapar"];

/// Sobre que texto se cuentan las posiciones.
pub const SOBRE_MARKDOWN: &str = "markdown";
pub const SOBRE_HTML: &str = "html";

/// La cita: lo marcado y un poco de antes y de despues (`TextQuoteSelector`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Cita {
    #[serde(rename = "type")]
    pub tipo: String,
    pub exact: String,
    pub prefix: String,
    pub suffix: String,
    #[serde(flatten)]
    pub resto: Map<String, Value>,
}

/// Donde estaba (`TextPositionSelector`): solo para desempatar.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Posicion {
    #[serde(rename = "type")]
    pub tipo: String,
    pub start: usize,
    pub end: usize,
    #[serde(flatten)]
    pub resto: Map<String, Value>,
}

/// Una marca.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MarcaDeTexto {
    pub id: String,
    pub marca: String,
    pub color: String,
    /// Milisegundos UTC de cuando se hizo.
    pub cuando: i64,
    pub cita: Cita,
    pub posicion: Posicion,
    #[serde(flatten)]
    pub resto: Map<String, Value>,
}

impl MarcaDeTexto {
    /// Si se puede usar: con id y con algo citado.
    pub fn valida(&self) -> bool {
        !self.id.is_empty() && !self.cita.exact.is_empty()
    }
}

/// El fichero entero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MarcasDeTexto {
    pub version: u32,
    pub sobre: String,
    pub marcas: Vec<MarcaDeTexto>,
    #[serde(flatten)]
    pub resto: Map<String, Value>,
}

impl Default for MarcasDeTexto {
    fn default() -> Self {
        MarcasDeTexto {
            version: VERSION,
            sobre: String::new(),
            marcas: Vec::new(),
            resto: Map::new(),
        }
    }
}

/// Lee el fichero. Vacio es «sin marcas»; lo que no es JSON, un error
/// (quien lee no debe escribir encima: seria perderlo).
pub fn leer(t: &str) -> Result<MarcasDeTexto, String> {
    if t.trim().is_empty() {
        return Ok(MarcasDeTexto::default());
    }
    let mut m: MarcasDeTexto = serde_json::from_str(t).map_err(|e| e.to_string())?;
    m.marcas.retain(MarcaDeTexto::valida);
    Ok(m)
}

/// El texto del fichero (con sangria: se lee bien en un diff).
pub fn escribir(m: &MarcasDeTexto) -> String {
    serde_json::to_string_pretty(m).unwrap_or_default()
}

/// La lista de la pagina (lo que manda: un JSON con las marcas) a marcas;
/// las que no se entienden o no tienen cita se quedan fuera.
pub fn de_la_pagina(v: &Value) -> Vec<MarcaDeTexto> {
    v.as_array()
        .map(|l| {
            l.iter()
                .filter_map(|m| serde_json::from_value::<MarcaDeTexto>(m.clone()).ok())
                .filter(MarcaDeTexto::valida)
                .collect()
        })
        .unwrap_or_default()
}

/// **Lo de aqui y lo que llego mientras tanto, juntos** (la pagina estaba
/// abierta y la sincronizacion trajo las del movil): lo nuevo de cada lado
/// entra, lo quitado en un lado se va si el otro no lo cambio, y si los dos
/// cambiaron la misma marca gana la de aqui.
pub fn fusionar(base: &MarcasDeTexto, mio: &MarcasDeTexto, disco: &MarcasDeTexto) -> MarcasDeTexto {
    let marcas = crate::md_comentarios::juntar(
        &base.marcas,
        &mio.marcas,
        &disco.marcas,
        |m| m.id.as_str(),
        |_, m, _| m.clone(),
    );
    let mut resto = disco.resto.clone();
    resto.extend(mio.resto.clone());
    MarcasDeTexto {
        version: mio.version.max(disco.version),
        sobre: if mio.sobre.is_empty() {
            disco.sobre.clone()
        } else {
            mio.sobre.clone()
        },
        marcas,
        resto,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn marca(id: &str, exact: &str, color: &str) -> MarcaDeTexto {
        MarcaDeTexto {
            id: id.into(),
            marca: "resaltar".into(),
            color: color.into(),
            cuando: 1,
            cita: Cita {
                tipo: "TextQuoteSelector".into(),
                exact: exact.into(),
                ..Cita::default()
            },
            posicion: Posicion {
                tipo: "TextPositionSelector".into(),
                start: 3,
                end: 3 + exact.len(),
                ..Posicion::default()
            },
            resto: Map::new(),
        }
    }

    fn lista(v: &[MarcaDeTexto]) -> MarcasDeTexto {
        MarcasDeTexto {
            sobre: SOBRE_MARKDOWN.into(),
            marcas: v.to_vec(),
            ..MarcasDeTexto::default()
        }
    }

    /// Lo que escribe la pagina (`marcas-texto.mjs`) se lee igual y vuelve
    /// igual, con lo que no se conoce dentro.
    #[test]
    fn el_fichero_de_la_pagina_se_lee_y_se_escribe_igual() {
        let t = r##"{"version":1,"sobre":"html","nuevo":true,"marcas":[{"id":"marca-texto-5","marca":"ondulada","color":"#5BA4F5","cuando":5,"otra":7,
            "cita":{"type":"TextQuoteSelector","exact":"el curado","prefix":"y ","suffix":" empieza"},
            "posicion":{"type":"TextPositionSelector","start":44,"end":53}}]}"##;
        let m = leer(t).unwrap();
        assert_eq!(m.sobre, SOBRE_HTML);
        assert_eq!(m.marcas.len(), 1);
        let k = &m.marcas[0];
        assert_eq!(
            (k.marca.as_str(), k.posicion.start, k.posicion.end),
            ("ondulada", 44, 53)
        );
        assert_eq!(k.cita.prefix, "y ");
        assert_eq!(k.resto["otra"], 7);
        let otra = leer(&escribir(&m)).unwrap();
        assert_eq!(otra, m);
        assert!(escribir(&m).contains("\"nuevo\": true"));
        assert!(escribir(&m).contains("\"type\": \"TextQuoteSelector\""));
    }

    #[test]
    fn caso_negativo_lo_roto_es_error_y_lo_vacio_no() {
        assert!(leer("{no es json").is_err());
        assert_eq!(leer("").unwrap().marcas.len(), 0);
        // Una marca sin cita no sirve: no se lee.
        let m =
            leer(r#"{"marcas":[{"id":"a","cita":{"exact":""}},{"id":"b","cita":{"exact":"x"}}]}"#)
                .unwrap();
        assert_eq!(m.marcas.len(), 1);
        assert_eq!(m.marcas[0].id, "b");
        let v: Value =
            serde_json::from_str(r#"[{"id":"a","cita":{"exact":"x"}},{"id":""},7]"#).unwrap();
        assert_eq!(de_la_pagina(&v).len(), 1);
    }

    #[test]
    fn juntar_conserva_lo_nuevo_de_los_dos_lados_y_lo_quitado() {
        let a = marca("marca-texto-1", "losa", "#F5C218");
        let b = marca("marca-texto-2", "curado", "#F5C218");
        let base = lista(&[a.clone(), b.clone()]);
        // Aqui: se quita `a`, `b` pasa a azul y se pone `c`.
        let c = marca("marca-texto-3", "obra", "#F47286");
        let mut b_azul = b.clone();
        b_azul.color = "#5BA4F5".into();
        let mio = lista(&[b_azul.clone(), c.clone()]);
        // En disco (del movil): sigue `a`, `b` igual y llego `d`.
        let d = marca("marca-texto-4", "piso", "#3FC88C");
        let disco = lista(&[a.clone(), b.clone(), d.clone()]);
        let j = fusionar(&base, &mio, &disco);
        let ids: Vec<&str> = j.marcas.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["marca-texto-2", "marca-texto-3", "marca-texto-4"]);
        assert_eq!(j.marcas[0].color, "#5BA4F5");
        // Quitada alli y sin tocar aqui: fuera.
        let j = fusionar(&base, &base, &lista(std::slice::from_ref(&b)));
        assert_eq!(j.marcas, vec![b.clone()]);
        // Sin cambios en ningun lado, nada cambia.
        assert_eq!(fusionar(&base, &base, &base), base);
    }
}
