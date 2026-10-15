//! **Los adjuntos dentro de la pagina web compartida** (8-oct-2026).
//!
//! El usuario: «en toda la interfaz anade compartir como HTML, y en este HTML
//! tiene que aparecer tambien los audios de forma comprimida y otros archivos
//! adjuntos, de forma que sea la funcion universal de compartir estrella».
//!
//! La pagina ya llevaba las hojas (texto, fotos, tablas, lienzos). Ahora
//! lleva ademas, dentro del mismo fichero, lo que no es una hoja: las notas de
//! voz, que se oyen ahi mismo, y cualquier otro fichero, que se descarga con
//! su nombre. Todo en `data:`, como las fotos del timeline: se abre sin
//! internet, en el PC o en el movil.
//!
//! Los audios van **comprimidos** porque ya lo estan: PixPin graba en `.m4a`
//! (AAC) y el movil tambien; un `.mp3`, `.ogg` u `.opus` va tal cual. No se
//! vuelven a codificar: perderian calidad sin ganar casi nada.
//!
//! Un boton fijo abajo a la izquierda («📎 Adjuntos (n)») abre la lista. Va
//! encima de todo y no toca el visor de hojas, que ocupa la pagina entera.

use std::path::{Path, PathBuf};


/// Lo que mas pesa un fichero que se mete dentro de la pagina. Con mas, la
/// pagina se haria inmanejable (el navegador la carga entera en memoria):
/// sale en la lista con su nombre, sin descarga.
pub const TOPE_BYTES: u64 = 40 * 1024 * 1024;

/// El tipo de un fichero, por su extension.
fn tipo_de(ruta: &Path) -> &'static str {
    let ext = ruta
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "m4a" | "mp4" | "aac" => "audio/mp4",
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "opus" => "audio/ogg; codecs=opus",
        "wav" => "audio/wav",
        "webm" => "audio/webm",
        "flac" => "audio/flac",
        "pdf" => "application/pdf",
        "txt" | "md" | "csv" => "text/plain; charset=utf-8",
        "html" | "htm" => "text/html",
        "json" => "application/json",
        "zip" => "application/zip",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

/// Si es una nota de voz o cualquier sonido: se oye en la pagina.
pub fn es_audio(ruta: &Path) -> bool {
    tipo_de(ruta).starts_with("audio/")
}

/// Si es una foto: esas ya van como hojas, no hace falta repetirlas.
pub fn es_imagen(ruta: &Path) -> bool {
    tipo_de(ruta).starts_with("image/")
}

/// Lo que va como adjunto de unos originales: todo menos las fotos.
pub fn de_originales(rutas: &[PathBuf]) -> Vec<PathBuf> {
    rutas
        .iter()
        .filter(|r| !es_imagen(r) && r.is_file())
        .cloned()
        .collect()
}

fn escapar(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn peso(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{} KB", bytes.div_ceil(1024).max(1))
    }
}

/// **El trozo de HTML de los adjuntos**: el boton y su lista. Vacio si no
/// hay ninguno. `rotulo` es «Adjuntos» en el idioma de la app.
pub fn html(rutas: &[PathBuf], rotulo: &str) -> String {
    if rutas.is_empty() {
        return String::new();
    }
    let mut filas = String::new();
    for r in rutas {
        let nombre = r
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = std::fs::metadata(r).map(|m| m.len()).unwrap_or(0);
        let cabe = bytes > 0 && bytes <= TOPE_BYTES;
        let datos = if cabe {
            std::fs::read(r).ok().map(|b| {
                format!(
                    "data:{};base64,{}",
                    tipo_de(r),
                    pixpin_timeline::html::base64(&b)
                )
            })
        } else {
            None
        };
        let n = escapar(&nombre);
        let p = peso(bytes);
        let icono = if es_audio(r) { "🎵" } else { "📄" };
        filas.push_str("<div class=\"pp-adj-fila\">");
        match (&datos, es_audio(r)) {
            (Some(d), true) => filas.push_str(&format!(
                "<div class=\"pp-adj-nombre\">{icono} {n} <span>· {p}</span></div>\
                 <audio controls preload=\"none\" src=\"{d}\"></audio>\
                 <a class=\"pp-adj-bajar\" download=\"{n}\" href=\"{d}\">⬇</a>"
            )),
            (Some(d), false) => filas.push_str(&format!(
                "<a class=\"pp-adj-nombre\" download=\"{n}\" href=\"{d}\">{icono} {n} <span>· {p}</span></a>"
            )),
            (None, _) => filas.push_str(&format!(
                "<div class=\"pp-adj-nombre pp-adj-fuera\">{icono} {n} <span>· {p}</span></div>"
            )),
        }
        filas.push_str("</div>");
    }
    let rotulo = escapar(rotulo);
    let n = rutas.len();
    format!(
        "<style>\
#pp-adj-boton{{position:fixed;left:16px;bottom:16px;z-index:2147483000;background:#0060DF;color:#fff;border:0;border-radius:20px;padding:10px 16px;font:600 14px 'Segoe UI',system-ui,sans-serif;box-shadow:0 4px 16px rgba(0,0,0,.35);cursor:pointer}}\
#pp-adj{{position:fixed;left:16px;bottom:64px;z-index:2147483000;width:min(420px,calc(100vw - 32px));max-height:60vh;overflow:auto;background:#1C1C1E;color:#F5F5F7;border:1px solid rgba(255,255,255,.12);border-radius:14px;padding:8px;font:14px 'Segoe UI',system-ui,sans-serif;box-shadow:0 8px 32px rgba(0,0,0,.45)}}\
#pp-adj[hidden]{{display:none}}\
.pp-adj-fila{{display:flex;flex-wrap:wrap;align-items:center;gap:6px;padding:8px;border-radius:10px}}\
.pp-adj-fila:hover{{background:rgba(255,255,255,.06)}}\
.pp-adj-nombre{{flex:1 1 100%;color:#F5F5F7;text-decoration:none;overflow-wrap:anywhere}}\
.pp-adj-nombre span{{color:#98989D}}\
.pp-adj-fila audio{{flex:1;min-width:0;height:36px}}\
.pp-adj-bajar{{color:#F5F5F7;text-decoration:none;padding:6px 10px;border-radius:8px;background:rgba(255,255,255,.08)}}\
.pp-adj-fuera{{opacity:.6}}\
@media print{{#pp-adj-boton,#pp-adj{{display:none}}}}\
</style>\
<button id=\"pp-adj-boton\" type=\"button\" onclick=\"var a=document.getElementById('pp-adj');a.hidden=!a.hidden\">📎 {rotulo} ({n})</button>\
<div id=\"pp-adj\" hidden>{filas}</div>"
    )
}

/// Mete los adjuntos en una pagina ya hecha, justo antes de `</body>` (o al
/// final, si no lo tiene).
pub fn meter(pagina: String, trozo: &str) -> String {
    if trozo.is_empty() {
        return pagina;
    }
    match pagina.rfind("</body>") {
        Some(i) => {
            let mut s = pagina;
            s.insert_str(i, trozo);
            s
        }
        None => pagina + trozo,
    }
}

/// Una pagina solo con adjuntos: lo compartido no tiene hojas (una nota de
/// voz sola, unos ficheros sueltos).
pub fn pagina_sola(titulo: &str, trozo: &str) -> String {
    let t = escapar(titulo);
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{t}</title>\
<style>body{{margin:0;min-height:100vh;background:#111113;color:#F5F5F7;font:15px 'Segoe UI',system-ui,sans-serif}}\
h1{{font-size:22px;margin:24px 16px}}</style></head>\
<body><h1>{t}</h1>{trozo}<script>document.getElementById('pp-adj').hidden=false</script></body></html>"
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn fichero(dir: &Path, nombre: &str, bytes: &[u8]) -> PathBuf {
        let r = dir.join(nombre);
        std::fs::write(&r, bytes).unwrap();
        r
    }

    #[test]
    fn el_audio_se_oye_y_lo_demas_se_descarga_todo_dentro() {
        let dir = std::env::temp_dir().join(format!("pp-adj-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let voz = fichero(&dir, "nota.m4a", b"audio");
        let doc = fichero(&dir, "informe & co.pdf", b"%PDF");
        let h = html(&[voz, doc], "Adjuntos");
        assert!(h.contains("<audio controls"));
        assert!(h.contains("data:audio/mp4;base64,YXVkaW8="));
        assert!(h.contains("download=\"informe &amp; co.pdf\""));
        assert!(h.contains("data:application/pdf;base64,"));
        assert!(h.contains("Adjuntos (2)"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn caso_negativo_sin_adjuntos_no_se_toca_la_pagina() {
        assert_eq!(html(&[], "Adjuntos"), "");
        let p = "<html><body>x</body></html>".to_string();
        assert_eq!(meter(p.clone(), ""), p);
    }

    #[test]
    fn se_meten_antes_del_cierre_del_cuerpo_y_las_fotos_no_cuentan() {
        let p = meter("<body>x</body></html>".into(), "<i>a</i>");
        assert_eq!(p, "<body>x<i>a</i></body></html>");
        let dir = std::env::temp_dir().join(format!("pp-adj2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let foto = fichero(&dir, "foto.png", b"png");
        let voz = fichero(&dir, "voz.ogg", b"ogg");
        assert_eq!(de_originales(&[foto, voz.clone()]), vec![voz]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
