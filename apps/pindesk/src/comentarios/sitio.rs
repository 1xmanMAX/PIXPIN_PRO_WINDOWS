//! **Donde viven las pegatinas de un documento** y lo que se les adjunta.
//!
//! | Documento | Fichero | Adjuntos y lienzos |
//! |---|---|---|
//! | adjunto del chat, nota | `pins/draw/anot-<uid>.comentarios.json` | `anot-<uid>.a<huella>.adjunto`, `anot-<uid>.c<id>.excalidraw.gz` |
//! | lienzo del chat | `pins/draw/<d>.comentarios.json` | `<d>.a<huella>.adjunto`, `<d>.c<id>.excalidraw.gz` |
//! | suelto | `<doc>.pixpin-comentarios.json` | `<doc>.pixpin-anotado/comentarios/` |
//!
//! Lo del chat viaja (ver `pindesk_sincro::anotado`); en el JSON las rutas
//! van **portatiles** (`pins/draw/…`) y aqui se pasan al disco con
//! `DiscoPc`. Lo suelto no viaja y lleva rutas del disco.

use std::path::{Path, PathBuf};

use pindesk_comentarios::{Adjunto, fichero};
use pindesk_proyecto::anotado::Base;
use pindesk_proyecto::vista::DiscoPc;
use pindesk_sincro::anotado as a;
use pindesk_sincro::disco::Disco;

/// El sitio de las pegatinas de un documento.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Sitio {
    /// El fichero de comentarios.
    pub fichero: PathBuf,
    /// La base del chat si viaja (`anot-<uid>` o el dibujo).
    pub base: Option<Base>,
    /// Fuera de un chat: la carpeta de los adjuntos y lienzos.
    junto: PathBuf,
    /// El nombre que se ensena del documento (para la tarea).
    pub nombre: String,
}

/// `<doc><terminacion>` al lado de `doc`.
fn al_lado(doc: &Path, terminacion: &str) -> PathBuf {
    let mut n = doc.as_os_str().to_owned();
    n.push(terminacion);
    PathBuf::from(n)
}

fn nombre_de(doc: &Path) -> String {
    doc.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// La huella de unos bytes (FNV-1a de 64 bits): el mismo fichero mandado
/// dos veces, o desde dos aparatos, es el mismo adjunto.
pub(crate) fn huella(b: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for x in b {
        h ^= *x as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn extension(nombre: &str) -> String {
    Path::new(nombre)
        .extension()
        .map(|e| {
            e.to_string_lossy()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .take(8)
                .collect::<String>()
                .to_ascii_lowercase()
        })
        .filter(|e| !e.is_empty())
        .unwrap_or_else(|| "bin".into())
}

impl Sitio {
    /// De un documento de un visor (PDF, Word, hoja, imagen, pagina, nota).
    pub(crate) fn del_documento(doc: &Path) -> Sitio {
        let suelto = al_lado(doc, fichero::TERMINACION_SUELTA);
        let (fichero, viaja) =
            crate::anotado_del_adjunto::del_mensaje_o_junto(doc, a::COMENTARIOS, suelto);
        Sitio {
            fichero,
            base: if viaja {
                crate::anotado_del_adjunto::base_de(doc)
            } else {
                None
            },
            junto: al_lado(doc, ".pixpin-anotado").join("comentarios"),
            nombre: nombre_de(doc),
        }
    }

    /// De un lienzo (`…/lienzos/<d>.excalidraw` de un chat, o un dibujo
    /// suelto).
    pub(crate) fn del_lienzo(dibujo: &Path) -> Sitio {
        let base = pindesk_proyecto::anotado::base_del_lienzo(dibujo);
        let fichero = match &base {
            Some(b) => b.fichero(a::COMENTARIOS),
            None => al_lado(dibujo, fichero::TERMINACION_SUELTA),
        };
        Sitio {
            fichero,
            base,
            junto: al_lado(dibujo, ".pixpin-anotado").join("comentarios"),
            nombre: nombre_de(dibujo)
                .trim_end_matches(".excalidraw")
                .trim_end_matches(".pixpin2d")
                .to_string(),
        }
    }

    /// La raiz de los datos, si el documento es de un chat.
    pub(crate) fn raiz(&self) -> Option<&Path> {
        self.base.as_ref().map(|b| b.raiz.as_path())
    }

    /// **Un fichero del JSON, en el disco**: lo portatil por el chat; lo
    /// demas tal cual.
    pub(crate) fn ruta_real(&self, ruta: &str) -> PathBuf {
        match &self.base {
            Some(b) if ruta.starts_with(a::CARPETA) => DiscoPc::nuevo(&b.raiz).ruta(&b.chat, ruta),
            _ => PathBuf::from(ruta),
        }
    }

    /// **Adjunta unos bytes** con su nombre: los copia donde viajan (una
    /// vez: si ya estan, no se reescriben) y devuelve el adjunto.
    pub(crate) fn adjuntar_bytes(&self, bytes: &[u8], nombre: &str) -> std::io::Result<Adjunto> {
        let h = huella(bytes);
        let (ruta_json, disco) = match &self.base {
            Some(b) => {
                let rel = a::adjunto_de_comentario(&b.base, &h);
                let disco = DiscoPc::nuevo(&b.raiz).ruta(&b.chat, &rel);
                (rel, disco)
            }
            None => {
                let disco = self.junto.join(format!("{h}.{}", extension(nombre)));
                (disco.to_string_lossy().into_owned(), disco)
            }
        };
        if !disco.is_file() {
            if let Some(p) = disco.parent() {
                std::fs::create_dir_all(p)?;
            }
            pindesk_sincro::disco::escribir_atomico(&disco, bytes)?;
        }
        Ok(Adjunto::archivo(&ruta_json, nombre))
    }

    /// Adjunta un fichero del disco.
    pub(crate) fn adjuntar(&self, origen: &Path) -> std::io::Result<Adjunto> {
        let bytes = std::fs::read(origen)?;
        self.adjuntar_bytes(&bytes, &nombre_de(origen))
    }

    /// **La ruta del lienzo nuevo de una entrada** (una imagen anotada, un
    /// lienzo propio), como va en el JSON.
    pub(crate) fn lienzo_nuevo(&self, id: &str) -> String {
        match &self.base {
            Some(b) => a::lienzo_de_comentario(&b.base, id),
            None => {
                let limpio: String = id
                    .chars()
                    .map(|c| {
                        if c.is_ascii_alphanumeric() || c == '-' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();
                self.junto
                    .join(format!("c{limpio}.excalidraw"))
                    .to_string_lossy()
                    .into_owned()
            }
        }
    }

    /// El proyecto (ficha) del chat del documento, para los atajos a sus
    /// mensajes.
    pub(crate) fn ficha(&self) -> Option<String> {
        let b = self.base.as_ref()?;
        pindesk_proyecto::almacen::Indice::leer(&b.raiz)
            .proyectos
            .into_iter()
            .find(|f| {
                pindesk_proyecto::vista::chat_de_ficha(&b.raiz, &f.id).as_deref() == Some(&b.chat)
            })
            .map(|f| f.id)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_huella_es_la_del_contenido() {
        assert_eq!(huella(b""), "cbf29ce484222325");
        assert_eq!(huella(b"a"), huella(b"a"));
        assert_ne!(huella(b"a"), huella(b"b"));
        assert_eq!(extension("Foto.JPG"), "jpg");
        assert_eq!(extension("sin"), "bin");
    }

    #[test]
    fn lo_suelto_va_al_lado_y_no_viaja() {
        let d = std::env::temp_dir().join(format!("pindesk-comentarios-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let doc = d.join("plano.pdf");
        std::fs::write(&doc, b"%PDF").unwrap();
        let s = Sitio::del_documento(&doc);
        assert_eq!(s.fichero, d.join("plano.pdf.pixpin-comentarios.json"));
        assert!(s.base.is_none());
        let adj = s.adjuntar_bytes(b"png", "foto.png").unwrap();
        let ruta = adj.ruta.clone().unwrap();
        assert!(Path::new(&ruta).is_file());
        assert_eq!(s.ruta_real(&ruta), PathBuf::from(&ruta));
        // Otra vez lo mismo: el mismo fichero.
        assert_eq!(
            s.adjuntar_bytes(b"png", "otra.png").unwrap().ruta,
            Some(ruta)
        );
        assert!(s.lienzo_nuevo("pc-1").ends_with("cpc-1.excalidraw"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
