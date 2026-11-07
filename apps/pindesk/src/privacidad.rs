//! **Privacidad y datos** (1-nov-2026, lo que el movil hizo para Google Play
//! el 30-oct y vale igual para la Microsoft Store): la politica, **lo que
//! sale de este PC y adonde**, donde viven tus datos y **reportar lo que
//! escribe la IA**.
//!
//! Es lo que ensena Ajustes › «Privacidad y datos» y lo que tiene que casar
//! con `docs/privacidad.md` (la politica publicada). Si una funcion nueva
//! manda algo a internet, **se apunta aqui** (y en la politica): un destino
//! que no sale en esta lista es un destino que el usuario no sabe que
//! existe.
//!
//! La auditoria del 1-nov-2026 (lo que abre una conexion en `apps/` y
//! `crates/`): el asistente con la clave del usuario (Anthropic, DeepSeek),
//! Jev (TypeSafe), la web del asistente (DuckDuckGo, Brave y la pagina que
//! lea), las voces de Microsoft de los lectores (`pindesk_voz::edge`), las
//! conexiones de Google (Drive, Calendar), «compartir por enlace»
//! (litterbox, temp.sh), las descargas de modelos (Hugging Face, GitHub,
//! alphacephei.com) y la sincronizacion con tus aparatos por tu red. No hay
//! analiticas, ni anuncios, ni informes de fallos, ni servidores de PinDesk.
//!
//! **Reportar una respuesta** (la politica de contenido generado por IA de
//! las tiendas): «Reportar respuesta…» en el menu de una respuesta del
//! asistente, con el motivo; se guarda en `<datos>/reportes-ia.jsonl` (una
//! linea JSON por reporte, como el movil) y se abre el correo con todo
//! escrito para mandarlo: no hay servidor que lo recoja.

use std::path::{Path, PathBuf};

use pindesk_store::Catalogo;
use serde::{Deserialize, Serialize};

/// La politica publicada (la misma que se pone en la ficha de la tienda).
pub const URL_POLITICA: &str =
    "https://github.com/1xmanMAX/PinDesk/blob/main/docs/privacidad.md";

/// A quien se escribe: dudas de privacidad y los reportes de la IA (el del
/// movil, `Privacidad.CORREO_CONTACTO`).
pub const CORREO: &str = "max.antony.8.5.1@gmail.com";

/// El fichero de los reportes, en la carpeta de datos.
pub const ARCHIVO_REPORTES: &str = "reportes-ia.jsonl";

/// Lo mas largo del contenido que se guarda (una respuesta larga no infla
/// el fichero) y lo que cabe en el correo (los enlaces `mailto:` largos
/// los cortan algunos programas).
pub const LARGO_CONTENIDO: usize = 8_000;
pub const LARGO_EN_EL_CORREO: usize = 1_500;

/// **Un sitio adonde sale algo**: su clave (`privacidad-<clave>-que`,
/// `-adonde` y `-cuando` en los catalogos) y la seccion de Ajustes donde se
/// enciende o se apaga, si la hay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Destino {
    pub clave: &'static str,
    pub ajustes: Option<&'static str>,
}

/// **Lo que sale de este PC**, lo personal primero.
pub const DESTINOS: [Destino; 8] = [
    Destino {
        clave: "asistente",
        ajustes: Some("asistente"),
    },
    Destino {
        clave: "jev",
        ajustes: Some("asistente"),
    },
    Destino {
        clave: "web",
        ajustes: Some("asistente"),
    },
    Destino {
        clave: "google",
        ajustes: Some("asistente"),
    },
    Destino {
        clave: "voces",
        ajustes: None,
    },
    Destino {
        clave: "enlace",
        ajustes: None,
    },
    Destino {
        clave: "modelos",
        ajustes: Some("voz"),
    },
    Destino {
        clave: "sincro",
        ajustes: Some("sincro"),
    },
];

impl Destino {
    pub fn que(&self, t: &Catalogo) -> String {
        t.t(&format!("privacidad-{}-que", self.clave))
    }

    /// «→ Anthropic (Claude) o DeepSeek. Solo si pegas tu clave…».
    pub fn adonde_y_cuando(&self, t: &Catalogo) -> String {
        format!(
            "→ {}. {}",
            t.t(&format!("privacidad-{}-adonde", self.clave)),
            t.t(&format!("privacidad-{}-cuando", self.clave))
        )
    }
}

/// **Por que se reporta**. El `id` es lo que se guarda (no cambia aunque
/// cambie el rotulo); los mismos que el movil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motivo {
    Ofensivo,
    Peligroso,
    Falso,
    Otro,
}

impl Motivo {
    pub const TODOS: [Motivo; 4] = [
        Motivo::Ofensivo,
        Motivo::Peligroso,
        Motivo::Falso,
        Motivo::Otro,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Motivo::Ofensivo => "ofensivo",
            Motivo::Peligroso => "peligroso",
            Motivo::Falso => "falso",
            Motivo::Otro => "otro",
        }
    }

    pub fn de_id(id: &str) -> Motivo {
        Motivo::TODOS
            .into_iter()
            .find(|m| m.id() == id)
            .unwrap_or(Motivo::Otro)
    }

    pub fn rotulo(self, t: &Catalogo) -> String {
        t.t(&format!("reportar-ia-motivo-{}", self.id()))
    }
}

/// **Un reporte**, una linea de `reportes-ia.jsonl` (los campos del movil).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reporte {
    /// Milisegundos desde 1970 (UTC).
    pub cuando: i64,
    /// Donde salio: `asistente` (el chat), `jev-lo-importante`,
    /// `jev-reunion`, `jev-repaso` u `otra`.
    pub funcion: String,
    pub motivo: String,
    pub contenido: String,
    #[serde(default)]
    pub detalle: String,
    #[serde(default)]
    pub version: String,
}

impl Reporte {
    /// Arma el reporte, recortando lo largo.
    pub fn nuevo(funcion: &str, motivo: Motivo, contenido: &str, cuando: i64) -> Reporte {
        Reporte {
            cuando,
            funcion: funcion.to_string(),
            motivo: motivo.id().to_string(),
            contenido: contenido.trim().chars().take(LARGO_CONTENIDO).collect(),
            detalle: String::new(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// La linea del fichero: JSON en una sola linea.
    pub fn a_linea(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Al reves; `None` si la linea no se entiende (se salta sin llevarse
    /// las demas).
    pub fn de_linea(linea: &str) -> Option<Reporte> {
        serde_json::from_str(linea.trim()).ok()
    }

    /// El asunto del correo.
    pub fn asunto(&self, t: &Catalogo) -> String {
        format!(
            "PinDesk · {} · {}",
            t.t("reportar-ia-asunto"),
            Motivo::de_id(&self.motivo).rotulo(t)
        )
    }

    /// **El cuerpo del correo**, en texto plano (el contenido, recortado a
    /// lo que cabe en un enlace).
    pub fn cuerpo(&self, t: &Catalogo) -> String {
        let contenido: String = self.contenido.chars().take(LARGO_EN_EL_CORREO).collect();
        let cortado = if self.contenido.chars().count() > LARGO_EN_EL_CORREO {
            "…"
        } else {
            ""
        };
        format!(
            "{}: {}\n{}: {}\n{}: {}\n{}: {}\n\n{}\n\n{}:\n{contenido}{cortado}\n",
            t.t("reportar-ia-campo-motivo"),
            Motivo::de_id(&self.motivo).rotulo(t),
            t.t("reportar-ia-campo-funcion"),
            self.funcion,
            t.t("reportar-ia-campo-fecha"),
            fecha_utc(self.cuando),
            t.t("reportar-ia-campo-version"),
            self.version,
            t.t("reportar-ia-comentario"),
            t.t("reportar-ia-campo-contenido"),
        )
    }

    /// El enlace `mailto:` con todo escrito.
    pub fn correo(&self, t: &Catalogo) -> String {
        format!(
            "mailto:{CORREO}?subject={}&body={}",
            cifrar(&self.asunto(t)),
            cifrar(&self.cuerpo(t))
        )
    }
}

/// `%`-codifica para un enlace (todo lo que no es ASCII sin reservar).
fn cifrar(s: &str) -> String {
    let mut o = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

/// «2026-11-01 14:05 UTC» (sin el huso del equipo: el correo es igual
/// donde se lea).
pub fn fecha_utc(ms: i64) -> String {
    let dias = ms.div_euclid(86_400_000);
    let resto = ms.rem_euclid(86_400_000) / 60_000;
    // Civil desde dias (Howard Hinnant).
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02} UTC",
        resto / 60,
        resto % 60
    )
}

/// Donde se guardan los reportes.
pub fn fichero(raiz: &Path) -> PathBuf {
    raiz.join(ARCHIVO_REPORTES)
}

/// **Lo apunta** al final del fichero (lo crea si no existe).
pub fn guardar(raiz: &Path, r: &Reporte) -> std::io::Result<()> {
    use std::io::Write;
    std::fs::create_dir_all(raiz)?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(fichero(raiz))?;
    writeln!(f, "{}", r.a_linea())
}

/// Los reportes guardados, en orden.
pub fn leer(raiz: &Path) -> Vec<Reporte> {
    std::fs::read_to_string(fichero(raiz))
        .map(|s| s.lines().filter_map(Reporte::de_linea).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cada_destino_tiene_sus_tres_textos_en_castellano_y_en_ingles() {
        for idioma in [
            pindesk_store::Idioma::Espanol,
            pindesk_store::Idioma::Ingles,
        ] {
            let t = Catalogo::nuevo(idioma);
            for d in DESTINOS {
                for parte in ["que", "adonde", "cuando"] {
                    let k = format!("privacidad-{}-{parte}", d.clave);
                    assert_ne!(t.t(&k), k, "falta {k} en {idioma:?}");
                }
            }
            for m in Motivo::TODOS {
                assert_ne!(m.rotulo(&t), format!("reportar-ia-motivo-{}", m.id()));
            }
        }
    }

    #[test]
    fn un_reporte_va_y_vuelve_por_su_linea_y_lo_raro_se_salta() {
        let dir = std::env::temp_dir().join(format!("pixpin-reportes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let largo = "x".repeat(LARGO_CONTENIDO + 50);
        let r = Reporte::nuevo("asistente", Motivo::Falso, &largo, 1_793_000_000_000);
        assert_eq!(r.contenido.chars().count(), LARGO_CONTENIDO);
        assert!(!r.a_linea().contains('\n'));
        guardar(&dir, &r).unwrap();
        std::fs::write(
            fichero(&dir),
            format!(
                "{}\nesto no es json\n",
                std::fs::read_to_string(fichero(&dir)).unwrap()
            ),
        )
        .unwrap();
        let otro = Reporte::nuevo("jev-reunion", Motivo::Ofensivo, "linea 1\nlinea 2", 0);
        guardar(&dir, &otro).unwrap();
        assert_eq!(leer(&dir), vec![r, otro]);
        let _ = std::fs::remove_dir_all(&dir);
        // Caso negativo: un motivo desconocido es «otro».
        assert_eq!(Motivo::de_id("raro"), Motivo::Otro);
    }

    #[test]
    fn el_correo_lleva_el_motivo_la_fecha_y_el_contenido_cifrados() {
        let t = Catalogo::nuevo(pindesk_store::Idioma::Espanol);
        let r = Reporte::nuevo("asistente", Motivo::Peligroso, "Mezcla lejía & amoniaco", 0);
        let c = r.correo(&t);
        assert!(c.starts_with(&format!("mailto:{CORREO}?subject=")));
        assert!(!c.contains(' ') && !c.contains('\n'));
        assert!(c.contains("lej%C3%ADa%20%26%20amoniaco"), "{c}");
        assert!(r.cuerpo(&t).contains("1970-01-01 00:00 UTC"));
        assert_eq!(fecha_utc(1_793_000_000_000), "2026-10-26 07:33 UTC");
        // Lo largo se recorta en el correo, no en el fichero.
        let largo = Reporte::nuevo("asistente", Motivo::Otro, &"y".repeat(5_000), 0);
        assert!(largo.cuerpo(&t).contains('…'));
        assert_eq!(largo.contenido.len(), 5_000);
    }
}
