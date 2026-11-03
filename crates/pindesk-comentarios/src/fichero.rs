//! **El fichero**: las pegatinas van en el `.comentarios.json` de siempre,
//! en la clave `pegatinas`, al lado de los comentarios de las notas
//! (`comentarios`, `pindesk_docs::md_comentarios`). Un solo fichero de
//! comentarios por documento y un solo formato:
//!
//! ```json
//! { "version": 1,
//!   "comentarios": [ … los de la nota, si es una nota … ],
//!   "pegatinas": [
//!     { "id": "pc1-18f2a-0", "tipo": "urgente",
//!       "ancla": {"sitio":"pagina","pagina":2,"x":120.5,"y":700},
//!       "autor": "PC de Max", "aparato": "pc1",
//!       "creado": 1793582613265, "cambiado": 1793582613265,
//!       "resuelto": false,
//!       "hilo": [ {"id":"pc1-18f2b-0","autor":"PC de Max","cuando":…,
//!                  "cambiado":…,"texto":"Revisar","adjuntos":[…]} ] } ] }
//! ```
//!
//! Donde va: de un adjunto del chat o de una nota,
//! `pins/draw/anot-<uid>.comentarios.json` (viaja con su mensaje,
//! terminacion `COMENTARIOS` de `pindesk_sincro::anotado`); de un lienzo,
//! `pins/draw/<dibujo>.comentarios.json` junto a su dibujo; de un fichero
//! suelto, `<fichero>.pixpin-comentarios.json` a su lado (no viaja).
//!
//! Leer **no pierde nada**: lo que no son pegatinas se deja como estaba y
//! una version mas nueva se lee igual (lo que no se entiende se conserva).

use serde_json::{Map, Value};

use crate::modelo::{Comentario, Pegatinas};

/// La clave de las pegatinas dentro del fichero.
pub const CLAVE: &str = "pegatinas";

/// La version del formato de las pegatinas.
pub const VERSION: u32 = 1;

/// La terminacion del fichero (la misma de los comentarios de las notas).
pub const TERMINACION: &str = ".comentarios.json";

/// La terminacion del fichero junto a un documento suelto.
pub const TERMINACION_SUELTA: &str = ".pixpin-comentarios.json";

fn objeto(t: &str) -> Result<Map<String, Value>, String> {
    let t = t.trim_start_matches('\u{feff}');
    if t.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str::<Value>(t).map_err(|e| e.to_string())? {
        Value::Object(o) => Ok(o),
        _ => Err("el fichero de comentarios no es un objeto".into()),
    }
}

/// **Lee las pegatinas.** Vacio es «ninguna»; un JSON roto es un error y
/// quien lee **no debe escribir encima** (se perderia lo que hubiera).
/// Una pegatina que no se entiende (sin id) se salta.
pub fn leer(t: &str) -> Result<Pegatinas, String> {
    let o = objeto(t)?;
    let lista = match o.get(CLAVE) {
        Some(Value::Array(v)) => v
            .iter()
            .filter_map(|c| serde_json::from_value::<Comentario>(c.clone()).ok())
            .filter(|c| !c.id.is_empty())
            .collect(),
        _ => Vec::new(),
    };
    Ok(Pegatinas { lista })
}

/// **El texto nuevo del fichero** con estas pegatinas, a partir del que
/// habia (`antes`, vacio si no habia): lo demas queda igual. Error si lo
/// de antes estaba roto.
pub fn escribir(antes: &str, p: &Pegatinas) -> Result<String, String> {
    let mut o = objeto(antes)?;
    if !o.contains_key("version") {
        o.insert("version".into(), Value::from(VERSION));
    }
    if !o.contains_key("comentarios") {
        o.insert("comentarios".into(), Value::Array(Vec::new()));
    }
    o.insert(
        CLAVE.into(),
        serde_json::to_value(&p.lista).map_err(|e| e.to_string())?,
    );
    Ok(texto(&Value::Object(o)))
}

/// El JSON con sangria y un salto al final, como los comentarios de las
/// notas (se lee a ojo y en un diff).
pub fn texto(v: &Value) -> String {
    let mut s = serde_json::to_string_pretty(v).unwrap_or_else(|_| "{}".into());
    s.push('\n');
    s
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::ancla::Ancla;
    use crate::modelo::{Adjunto, Quien, Tipo};

    fn quien() -> Quien {
        Quien {
            autor: "Movil".into(),
            aparato: "and".into(),
        }
    }

    #[test]
    fn ida_y_vuelta_sin_perder_los_comentarios_de_la_nota() {
        let nota = r#"{"version":1,"comentarios":[{"id":"h1","texto":"de la nota","ancla":{"cita":"x"}}],"otro":true}"#;
        let mut p = leer(nota).unwrap();
        assert!(p.lista.is_empty());
        let id = p.poner(Tipo::Alta, Ancla::pagina(1, 3.0, 4.0), &quien(), 7);
        p.escribir(
            &id,
            &quien(),
            8,
            "mira",
            vec![Adjunto::mensaje("c", "ABCDE23456", "hola")],
        );
        let t = escribir(nota, &p).unwrap();
        let v: Value = serde_json::from_str(&t).unwrap();
        assert_eq!(v["comentarios"][0]["texto"], "de la nota");
        assert_eq!(v["otro"], true);
        assert_eq!(v["pegatinas"][0]["tipo"], "alta");
        assert_eq!(
            v["pegatinas"][0]["hilo"][0]["adjuntos"][0]["mensaje"],
            "ABCDE23456"
        );
        assert_eq!(leer(&t).unwrap(), p);
        assert!(t.ends_with("}\n"));
    }

    #[test]
    fn de_la_nada_sale_un_fichero_que_entiende_la_nota() {
        let mut p = Pegatinas::default();
        p.poner(Tipo::Nota, Ancla::escena(1.0, 2.0), &quien(), 1);
        let t = escribir("", &p).unwrap();
        let v: Value = serde_json::from_str(&t).unwrap();
        assert_eq!(v["version"], 1);
        assert_eq!(v["comentarios"], serde_json::json!([]));
        assert_eq!(leer(&t).unwrap(), p);
    }

    #[test]
    fn caso_negativo_lo_roto_es_error_y_no_se_escribe_encima() {
        assert!(leer("{roto").is_err());
        assert!(leer("[1,2]").is_err());
        assert!(escribir("{roto", &Pegatinas::default()).is_err());
        assert!(leer("").unwrap().lista.is_empty());
        assert!(leer("\u{feff}  ").unwrap().lista.is_empty());
        // Una pegatina sin id, o que no es un objeto, se salta; las demas no.
        let p = leer(r#"{"pegatinas":[{"tipo":"urgente"},7,{"id":"a","tipo":"media","nuevo":3}]}"#)
            .unwrap();
        assert_eq!(p.lista.len(), 1);
        assert_eq!(p.lista[0].tipo, Tipo::Media);
        // Y lo que no conoce de ella vuelve al escribir.
        let t = escribir("", &p).unwrap();
        assert!(t.contains("\"nuevo\": 3"));
    }
}
