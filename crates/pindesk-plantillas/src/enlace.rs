//! **Como se abre la app**: la linea de ordenes y los enlaces
//! `pindesk://plantilla?…` (PinDesk los recibe por su protocolo y se los
//! pasa a `pindesk-plantillas.exe`).
//!
//! | Enlace | Hace |
//! |---|---|
//! | `pindesk://plantilla` | Abre la app (con la galeria). |
//! | `pindesk://plantilla?abrir=C%3A%5C…%5Cx.pdplantilla` | Abre ese fichero. |
//! | `pindesk://plantilla?figura=plantilla-mi-diario` | Abre la plantilla de esa figura de PinDesk. |
//! | `pindesk://plantilla?galeria=semanal` | Empieza con una de la galeria. |
//! | `pindesk://plantilla?describir=planner%20semanal` | Abre «Descríbelo» con ese texto (sin mandarlo: lo manda el usuario). |
//!
//! En la linea de ordenes: `pindesk-plantillas [fichero.pdplantilla]`,
//! `--galeria <clave>`, `--describir "<texto>"`, `--url "<enlace>"` y,
//! sin ventana, `--exportar <fichero.pdplantilla> <salida.pdf|png|svg|excalidraw>`.
//! Por enlace solo se abren ficheros `.pdplantilla` locales (nunca una ruta
//! de red) y nunca se exporta nada.

use std::path::PathBuf;

/// Lo que se pide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Accion {
    /// Abrir sin nada (la galeria).
    Nada,
    Abrir(PathBuf),
    /// Abrir la plantilla de una figura de PinDesk por su id.
    Figura(String),
    Galeria(String),
    Describir(String),
    /// Sin ventana: exportar y salir.
    Exportar {
        entrada: PathBuf,
        salida: PathBuf,
    },
    /// Lo que no se entiende (se abre sin nada y se avisa).
    NoSeEntiende(String),
}

/// Los dos nombres del protocolo (el de antes de PinDesk tambien).
const PREFIJOS: [&str; 2] = ["pindesk://plantilla", "pixpin://plantilla"];

/// Si un texto es un enlace de Plantillas.
pub fn es_enlace(s: &str) -> bool {
    let s = s.trim().to_ascii_lowercase();
    PREFIJOS.iter().any(|p| {
        s.strip_prefix(p)
            .is_some_and(|r| r.is_empty() || r.starts_with('?') || r.starts_with('/'))
    })
}

/// Deshace el `%xx` y el `+` de un enlace.
pub fn descodificar(s: &str) -> String {
    let b = s.as_bytes();
    let mut sal = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16)
                {
                    Ok(x) => {
                        sal.push(x);
                        i += 3;
                        continue;
                    }
                    Err(_) => sal.push(b'%'),
                }
            }
            b'+' => sal.push(b' '),
            x => sal.push(x),
        }
        i += 1;
    }
    String::from_utf8_lossy(&sal).into_owned()
}

/// Lo contrario, para hacer enlaces.
pub fn codificar(s: &str) -> String {
    let mut sal = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            sal.push(b as char);
        } else {
            sal.push_str(&format!("%{b:02X}"));
        }
    }
    sal
}

/// **El enlace que abre un fichero.**
pub fn para_abrir(ruta: &std::path::Path) -> String {
    format!(
        "pindesk://plantilla?abrir={}",
        codificar(&ruta.to_string_lossy())
    )
}

/// **La accion de un enlace.** `None` si no es de Plantillas.
pub fn de_enlace(url: &str) -> Option<Accion> {
    if !es_enlace(url) || url.len() > 16 * 1024 {
        return None;
    }
    let consulta = url.split_once('?').map_or("", |(_, q)| q);
    if let Some(par) = consulta.split('&').find(|p| !p.is_empty()) {
        let (k, v) = par.split_once('=').unwrap_or((par, ""));
        let v = descodificar(v);
        match k {
            "abrir" | "open" => {
                return Some(
                    ruta_segura(&v).map_or_else(|| Accion::NoSeEntiende(v.clone()), Accion::Abrir),
                );
            }
            "figura" | "figure" if crate::carpeta::es_de_plantillas(&v) => {
                return Some(Accion::Figura(v));
            }
            "galeria" | "gallery" if crate::galeria::CLAVES.contains(&v.as_str()) => {
                return Some(Accion::Galeria(v));
            }
            "describir" | "describe" => {
                return Some(Accion::Describir(v.chars().take(500).collect()));
            }
            _ => return Some(Accion::NoSeEntiende(par.to_string())),
        }
    }
    Some(Accion::Nada)
}

/// Una ruta que un enlace puede abrir: un `.pdplantilla` local.
fn ruta_segura(s: &str) -> Option<PathBuf> {
    let s = s.trim();
    let red = s.starts_with("\\\\") || s.starts_with("//") || s.contains("://");
    let bien = s
        .to_ascii_lowercase()
        .ends_with(&format!(".{}", crate::formato::EXTENSION));
    (!red && bien && !s.contains('\0')).then(|| PathBuf::from(s))
}

/// **La accion de la linea de ordenes** (sin el nombre del programa).
pub fn de_argumentos(args: &[String]) -> Accion {
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let siguiente = args.get(i + 1).cloned();
        match a {
            "--url" => {
                return siguiente
                    .as_deref()
                    .and_then(de_enlace)
                    .unwrap_or(Accion::NoSeEntiende(a.into()));
            }
            "--galeria" => {
                return match siguiente {
                    Some(c) if crate::galeria::CLAVES.contains(&c.as_str()) => Accion::Galeria(c),
                    otro => Accion::NoSeEntiende(otro.unwrap_or_default()),
                };
            }
            "--describir" => {
                return siguiente.map_or(Accion::NoSeEntiende(a.into()), Accion::Describir);
            }
            "--exportar" => {
                return match (siguiente, args.get(i + 2)) {
                    (Some(e), Some(s)) => Accion::Exportar {
                        entrada: e.into(),
                        salida: s.into(),
                    },
                    _ => Accion::NoSeEntiende(a.into()),
                };
            }
            _ if es_enlace(a) => return de_enlace(a).unwrap_or(Accion::Nada),
            _ if !a.starts_with("--") => return Accion::Abrir(PathBuf::from(a)),
            _ => {}
        }
        i += 1;
    }
    Accion::Nada
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn los_enlaces_de_plantillas() {
        assert_eq!(de_enlace("pindesk://plantilla"), Some(Accion::Nada));
        assert_eq!(
            de_enlace("pindesk://plantilla?galeria=semanal"),
            Some(Accion::Galeria("semanal".into()))
        );
        assert_eq!(
            de_enlace("pindesk://plantilla?describir=planner+semanal%20con%20h%C3%A1bitos"),
            Some(Accion::Describir("planner semanal con hábitos".into()))
        );
        let r = PathBuf::from(r"C:\Users\Max\Mi diario.pdplantilla");
        assert_eq!(de_enlace(&para_abrir(&r)), Some(Accion::Abrir(r)));
        assert_eq!(
            de_enlace("pindesk://plantilla?figura=plantilla-mi-diario"),
            Some(Accion::Figura("plantilla-mi-diario".into()))
        );
        // Casos negativos: otro verbo de PinDesk, una ruta de red, otro fichero.
        assert_eq!(de_enlace("pindesk://tarea?texto=x"), None);
        assert_eq!(de_enlace("pindesk://plantillas"), None);
        assert!(matches!(
            de_enlace(r"pindesk://plantilla?abrir=%5C%5Cservidor%5Cx.pdplantilla"),
            Some(Accion::NoSeEntiende(_))
        ));
        assert!(matches!(
            de_enlace("pindesk://plantilla?abrir=C:/x/virus.exe"),
            Some(Accion::NoSeEntiende(_))
        ));
        assert!(matches!(
            de_enlace("pindesk://plantilla?galeria=cohete"),
            Some(Accion::NoSeEntiende(_))
        ));
        assert!(matches!(
            de_enlace("pindesk://plantilla?figura=serie-semanal"),
            Some(Accion::NoSeEntiende(_))
        ));
        assert!(es_enlace("PinDesk://Plantilla?x") && es_enlace("pixpin://plantilla"));
    }

    #[test]
    fn la_linea_de_ordenes() {
        assert_eq!(de_argumentos(&[]), Accion::Nada);
        assert_eq!(
            de_argumentos(&v(&["x.pdplantilla"])),
            Accion::Abrir("x.pdplantilla".into())
        );
        assert_eq!(
            de_argumentos(&v(&["--galeria", "diario"])),
            Accion::Galeria("diario".into())
        );
        assert_eq!(
            de_argumentos(&v(&["--exportar", "a.pdplantilla", "a.pdf"])),
            Accion::Exportar {
                entrada: "a.pdplantilla".into(),
                salida: "a.pdf".into()
            }
        );
        assert_eq!(
            de_argumentos(&v(&["--url", "pindesk://plantilla?galeria=dia"])),
            Accion::Galeria("dia".into())
        );
        assert_eq!(
            de_argumentos(&v(&["pindesk://plantilla?galeria=dia"])),
            Accion::Galeria("dia".into())
        );
        assert!(matches!(
            de_argumentos(&v(&["--exportar", "a"])),
            Accion::NoSeEntiende(_)
        ));
        assert_eq!(descodificar("%zz%4"), "%zz%4");
    }
}
