//! **El asistente desde Flow**: `p ia <pedido>` (o `p ? <pedido>`).
//!
//! El usuario, 13-oct: «yo le pido algo y lo hace directamente». Intro
//! manda el pedido `asistente` y la app lo hace con el motor de Ajustes sin
//! abrir el chat; lo que conteste sale en su aviso flotante con «Abrir».
//!
//! - Con texto: «Pedir al asistente: …» arriba (con `@proyecto`, sobre ese
//!   chat; con Ctrl+V, la imagen copiada va con el pedido, como en `p s`).
//! - Sin texto: como pedirle algo, abrir el chat «Asistente» y las
//!   automatizaciones de la lista, para correrlas ya (pedido `asistente`
//!   con `automatizacion`).
//!
//! Las automatizaciones se leen del disco como todo lo demas del plugin: el
//! JSON que la app guarda en la carpeta del chat «Asistente»
//! (`android/guardados/asistente/automatizaciones.json`).

use crate::datos::Proyecto;
use crate::resultados::{Accion, Contexto, Resultado, glifo, pedido};
use serde_json::{Value, json};

/// Donde guarda la app las automatizaciones, dentro de la carpeta del chat.
pub const FICHERO: &str = "android/guardados/asistente/automatizaciones.json";

/// Lo que el plugin ensena de una automatizacion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Automatizacion {
    pub id: String,
    pub nombre: String,
    pub activa: bool,
    /// `al_capturar`, `a_la_hora`, `al_sincronizar` o `a_mano`.
    pub disparador: String,
    pub hora: Option<String>,
}

/// Las automatizaciones guardadas (las del chat que tenga el fichero). Lo
/// que no se entiende se salta.
pub fn automatizaciones(proyectos: &[Proyecto]) -> Vec<Automatizacion> {
    for p in proyectos {
        let Ok(texto) = std::fs::read_to_string(p.carpeta.join(FICHERO)) else {
            continue;
        };
        return de_json(&texto);
    }
    Vec::new()
}

pub fn de_json(texto: &str) -> Vec<Automatizacion> {
    let Ok(v) = serde_json::from_str::<Value>(texto) else {
        return Vec::new();
    };
    v.get("automatizaciones")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|a| {
            let id = a.get("id")?.as_str()?.to_string();
            let nombre = a.get("nombre")?.as_str()?.trim().to_string();
            let d = a.get("disparador")?;
            let disparador = d.get("tipo")?.as_str()?.to_string();
            if id.is_empty() || nombre.is_empty() {
                return None;
            }
            Some(Automatizacion {
                id,
                nombre,
                activa: a.get("activa").and_then(Value::as_bool).unwrap_or(true),
                hora: d.get("hora").and_then(Value::as_str).map(str::to_string),
                disparador,
            })
        })
        .collect()
}

fn cuando(a: &Automatizacion) -> String {
    match (a.disparador.as_str(), &a.hora) {
        ("al_capturar", _) => "al capturar".into(),
        ("a_la_hora", Some(h)) => format!("cada día a las {h}"),
        ("al_sincronizar", _) => "al sincronizar".into(),
        ("a_mano", _) => "a mano".into(),
        (otro, _) => otro.replace('_', " "),
    }
}

/// **Las filas de `p ia …`**. `destino_id`/`destino`: el chat de la
/// `@arroba` (o `null` y «en Mensajes guardados»).
pub fn resultados(proyectos: &[Proyecto], resto: &str, destino_id: Value, destino: &str, ctx: &Contexto) -> Vec<Resultado> {
    let mut v = Vec::new();
    let abrir_chat = Resultado::nuevo(
        "Abrir el chat «Asistente»",
        "La conversación con el asistente, en PixPin",
        glifo::CHAT,
        Accion::Pedido(pedido("asistente", json!({}))),
    );
    if resto.is_empty() {
        let mut r = Resultado::nuevo(
            "Pídele algo al asistente…",
            "Escribe detrás de «ia» y pulsa Intro: lo hace sin abrir el chat · Ctrl+V pega una imagen · @proyecto",
            glifo::ASISTENTE,
            Accion::Consulta(ctx.consulta("ia ")),
        );
        r.autocompletar = Some(ctx.consulta("ia "));
        v.push(r);
        v.push(abrir_chat);
        for a in automatizaciones(proyectos) {
            let estado = if a.activa { "" } else { " · apagada" };
            let mut r = Resultado::nuevo(
                format!("▶ {}", a.nombre),
                format!("Automatización ({}){estado} · Intro: correrla ya", cuando(&a)),
                glifo::AUTOMATIZACION,
                Accion::Pedido(pedido("asistente", json!({ "automatizacion": a.id }))),
            );
            r.clave = Some(format!("automatizacion/{}", a.id));
            v.push(r);
        }
        return v;
    }
    let sobre = if destino_id.is_null() {
        String::new()
    } else {
        format!(" · sobre el chat {}", destino.trim_start_matches("en "))
    };
    let mut r = Resultado::nuevo(
        format!("Pedir al asistente: {resto}"),
        format!("Intro: lo hace sin abrir el chat (el motor de Ajustes){sobre} · la respuesta sale en un aviso"),
        glifo::ASISTENTE,
        Accion::Pedido(pedido("asistente", json!({ "texto": resto, "proyecto": destino_id }))),
    );
    r.ayuda_titulo = Some(resto.to_string());
    // Con `[img NN]`, las imagenes van con el pedido (el modelo las ve).
    crate::imagenes::con_imagenes(&mut r, resto, ctx);
    v.push(r);
    v.push(abrir_chat);
    v
}
