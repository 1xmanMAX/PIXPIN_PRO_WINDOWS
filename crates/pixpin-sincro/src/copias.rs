//! Copias de seguridad de un chat antes de que nada lo pise.
//!
//! Puerto de `Copias.hacer` de `sincro/Copias.kt`: antes de sincronizar se
//! guarda como estaba el proyecto, sus mensajes y sus archivos, cada archivo
//! una vez por su contenido. Si nada cambio desde la ultima copia no se hace
//! otra, y se quedan las [POR_PROYECTO] mas nuevas. Viven en `copias/`, fuera
//! de lo que viaja: son de este aparato.
//!
//! Aqui solo se hacen. Volver a una copia es de la pantalla de copias, que en
//! el PC todavia no existe; los ficheros quedan con el mismo formato que en
//! el movil para que se pueda hacer despues sin perder las de ahora.

use std::collections::HashSet;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::disco::{Disco, escribir_atomico, limpio, sha256_de};
use crate::kotlin;

pub const CARPETA: &str = "copias";
pub const POR_PROYECTO: usize = 30;
/// Un PDF de cientos de megas no se copia: no lo pisa nadie y llenaria el
/// disco.
pub const TOPE_POR_ARCHIVO: u64 = 40 * 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Copia {
    pub id: String,
    pub chat: String,
    pub cuando: i64,
    pub motivo: String,
    pub nombre: String,
    pub hojas: usize,
    pub proyecto: Option<String>,
    pub mensajes: Vec<String>,
    pub archivos: std::collections::BTreeMap<String, String>,
    pub sin_copiar: Vec<String>,
}

fn raiz<D: Disco + ?Sized>(d: &D) -> PathBuf {
    d.raiz().join(CARPETA)
}

fn carpeta_de<D: Disco + ?Sized>(d: &D, chat: &str) -> PathBuf {
    raiz(d).join("proyectos").join(limpio(chat))
}

/// Las copias de un chat, la mas nueva primero.
pub fn lista<D: Disco + ?Sized>(d: &D, chat: &str) -> Vec<Copia> {
    let mut v: Vec<Copia> = std::fs::read_dir(carpeta_de(d, chat))
        .map(|l| {
            l.flatten()
                .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
                .filter_map(|e| std::fs::read_to_string(e.path()).ok())
                .filter_map(|t| serde_json::from_str(&t).ok())
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|c| std::cmp::Reverse(c.cuando));
    v
}

/// Hace la copia. `Ok(false)` si no habia nada o era igual que la ultima.
pub fn hacer<D: Disco + ?Sized>(d: &D, chat: &str, motivo: &str, ahora: i64) -> io::Result<bool> {
    let p = d.proyecto_portatil(chat)?;
    let mensajes: Vec<String> = d.mensajes(chat)?.iter().map(|m| m.a_texto()).collect();
    if p.is_none() && mensajes.is_empty() {
        return Ok(false);
    }
    let objetos = raiz(d).join("objetos");
    std::fs::create_dir_all(&objetos)?;
    let mut archivos = std::collections::BTreeMap::new();
    let mut grandes = Vec::new();
    for (rel, _) in d.alcance(chat)? {
        let f = d.ruta(chat, &rel);
        let Ok(meta) = std::fs::metadata(&f) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        if meta.len() > TOPE_POR_ARCHIVO {
            grandes.push(rel);
            continue;
        }
        let resumen = sha256_de(&mut std::fs::File::open(&f)?)?;
        let destino = objetos.join(&resumen);
        if !destino.exists() {
            escribir_atomico(&destino, &std::fs::read(&f)?)?;
        }
        archivos.insert(rel, resumen);
    }
    let copia = Copia {
        id: ahora.to_string(),
        chat: chat.into(),
        cuando: ahora,
        motivo: motivo.into(),
        nombre: p
            .as_ref()
            .and_then(|p| kotlin::cadena(p, "nombre"))
            .unwrap_or("Conversación")
            .into(),
        hojas: p
            .as_ref()
            .and_then(|p| {
                p.como_objeto()?
                    .obtener("hojas")?
                    .como_lista()
                    .map(<[_]>::len)
            })
            .unwrap_or(0),
        proyecto: p.as_ref().map(|p| p.a_texto()),
        mensajes,
        archivos,
        sin_copiar: grandes,
    };
    if let Some(u) = lista(d, chat).first()
        && u.proyecto == copia.proyecto
        && u.mensajes == copia.mensajes
        && u.archivos == copia.archivos
    {
        return Ok(false);
    }
    let carpeta = carpeta_de(d, chat);
    std::fs::create_dir_all(&carpeta)?;
    let mut nombre = format!("{}.json", copia.id);
    let mut n = 1;
    while carpeta.join(&nombre).exists() {
        nombre = format!("{}-{n}.json", copia.id);
        n += 1;
    }
    let puesta = Copia {
        id: nombre.trim_end_matches(".json").into(),
        ..copia
    };
    escribir_atomico(
        &carpeta.join(&nombre),
        serde_json::to_string(&puesta)
            .map_err(io::Error::other)?
            .as_bytes(),
    )?;
    podar(d, chat);
    Ok(true)
}

fn podar<D: Disco + ?Sized>(d: &D, chat: &str) {
    let sobran: Vec<Copia> = lista(d, chat).into_iter().skip(POR_PROYECTO).collect();
    if sobran.is_empty() {
        return;
    }
    for c in &sobran {
        let _ = std::fs::remove_file(carpeta_de(d, chat).join(format!("{}.json", c.id)));
    }
    // Los objetos que ya no senala ninguna copia de ningun chat.
    let mut vivos = HashSet::new();
    if let Ok(l) = std::fs::read_dir(raiz(d).join("proyectos")) {
        for e in l.flatten() {
            let chat = e.file_name().to_string_lossy().to_string();
            for c in lista(d, &chat) {
                vivos.extend(c.archivos.into_values());
            }
        }
    }
    if let Ok(l) = std::fs::read_dir(raiz(d).join("objetos")) {
        for e in l.flatten() {
            if !vivos.contains(&e.file_name().to_string_lossy().to_string()) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}
