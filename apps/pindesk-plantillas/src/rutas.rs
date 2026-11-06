//! **Donde vive cada cosa**: la carpeta de datos de PinDesk (la misma que
//! usa PinDesk: `%APPDATA%\PinDesk`, o junto al `.exe` en la portable), la
//! carpeta compartida de plantillas, el idioma y las letras para exportar.

use std::path::{Path, PathBuf};

use nucleo::exportar::letras::Letras;
use nucleo::tema::{CAVEAT, FRAUNCES, WORK_SANS};

/// La raiz de datos de PinDesk.
pub fn raiz() -> PathBuf {
    let dir_exe = pindesk_shell::directorio_del_ejecutable().unwrap_or_default();
    let appdata = pindesk_shell::appdata().unwrap_or_default();
    pindesk_store::resolver(&dir_exe, &appdata)
        .raiz()
        .to_path_buf()
}

/// Lo que dicen los ajustes de PinDesk.
pub fn ajustes() -> pindesk_store::Ajustes {
    let dir_exe = pindesk_shell::directorio_del_ejecutable().unwrap_or_default();
    let appdata = pindesk_shell::appdata().unwrap_or_default();
    pindesk_store::cargar(&pindesk_store::resolver(&dir_exe, &appdata)).unwrap_or_default()
}

/// La carpeta compartida con PinDesk (`<raiz>/plantillas`).
pub fn carpeta_compartida() -> PathBuf {
    nucleo::carpeta::en(&raiz())
}

/// **El idioma de la interfaz**: el de PinDesk (sus ajustes, o el de
/// Windows), para que las dos apps hablen igual.
pub fn idioma() -> String {
    let i = pindesk_store::resolver_idioma(&pindesk_shell::locale_del_sistema(), ajustes().idioma);
    i.etiqueta().to_string()
}

/// Lo que se recuerda de la app (por ahora, el dia o la noche).
fn fichero_preferencias() -> PathBuf {
    carpeta_compartida().join(".app.json")
}

/// `Some(true)` si se eligio la noche, `Some(false)` el dia; `None`, el de
/// Windows.
pub fn noche_elegida() -> Option<bool> {
    let t = std::fs::read_to_string(fichero_preferencias()).ok()?;
    let v: serde_json::Value = serde_json::from_str(&t).ok()?;
    v.get("noche").and_then(serde_json::Value::as_bool)
}

pub fn recordar_noche(noche: bool) {
    let r = fichero_preferencias();
    if let Some(p) = r.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let _ = std::fs::write(r, serde_json::json!({ "noche": noche }).to_string());
}

fn leer(p: &Path) -> Option<Vec<u8>> {
    std::fs::read(p).ok()
}

/// **Las letras para el PNG y el PDF**: las de PinDesk (Work Sans,
/// Fraunces y Caveat, desempaquetadas por DirectWrite) y de reserva las de
/// Windows para lo que no tengan (cirilico, arabe, ideogramas, coreano,
/// devanagari).
pub fn letras() -> Letras {
    let mut l = Letras::nuevas();
    for familia in [WORK_SANS, FRAUNCES, CAVEAT] {
        let caras = pindesk_render::letras::ficheros_llanos(familia);
        let n = caras.len();
        for (i, b) in caras.into_iter().enumerate() {
            // La primera es la normal; la ultima, la gruesa (Caveat trae 500 y
            // 700; Fraunces y Work Sans, una sola que vale para las dos).
            if i == 0 {
                l.poner(familia, false, b.clone());
            }
            if i + 1 == n {
                l.poner(familia, true, b);
            }
        }
    }
    let fuentes = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Windows"))
        .join("Fonts");
    if let Some(b) = leer(&fuentes.join("segoeuib.ttf")) {
        l.poner(WORK_SANS, true, b);
    }
    for f in [
        "segoeui.ttf",
        "Nirmala.ttf",
        "malgun.ttf",
        "simhei.ttf",
        "msyh.ttc",
        "YuGothR.ttc",
    ] {
        if let Some(b) = leer(&fuentes.join(f)) {
            l.reserva(b);
        }
    }
    l
}
