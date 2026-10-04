//! **La pagina, aunque el documento del proyecto este roto.** Puerto de
//! `PdfDelProyecto.paginaSana` del movil (v0.81: «la portada repara el PDF
//! roto»).
//!
//! Cada proyecto con PDF guarda dos copias: el documento (`pdfOrigen`) y
//! una **copia limpia** (`pdfLimpio`) que nace del mismo fichero y crece con
//! el cuando se le pegan paginas (`pdf_en_chat::unir`) o se aligera
//! (`aligerar`). En el movil el documento se reescribe a menudo, y antes del
//! candado de `PdfDelProyecto` dos escritores a la vez lo dejaban hecho de
//! trozos: no se abria, y lo que veia el usuario era su hoja anotada **sin
//! el fondo**. Ese documento roto llega aqui por la sincronizacion igual que
//! uno sano; y en el PC un corte de luz a mitad de un pegado puede dejar
//! otro igual.
//!
//! Lo que se hace es lo del movil: si el documento no se deja pintar, **se
//! rehace desde la copia limpia** (se copia encima, de un tiron) y se vuelve
//! a intentar; y si ni asi, se pinta la copia limpia, que lo anotado va
//! encima de todos modos. El fondo no desaparece nunca mientras exista la
//! copia. En el PC lo anotado no va dentro del documento (va en los lienzos
//! de sus hojas), asi que copiar la limpia encima no pierde nada: es lo que
//! el movil llama rehacer, sin capas que volver a escribir.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[derive(Default)]
struct Estado {
    /// Documento → su copia limpia, apuntado al buscarlo
    /// (`pdf_en_chat::documento_de`). Asi quien pinta solo con la ruta del
    /// documento sabe donde esta la copia (`paginaSanaDe` del movil).
    limpias: HashMap<PathBuf, PathBuf>,
    /// Los que ya se rehicieron en esta sesion: si vuelven a fallar, se
    /// pinta la copia y no se rehace otra vez (un bucle de copias no arregla
    /// nada y gasta disco).
    rehechos: HashSet<PathBuf>,
}

fn estado() -> &'static Mutex<Estado> {
    static E: OnceLock<Mutex<Estado>> = OnceLock::new();
    E.get_or_init(Default::default)
}

/// **La copia limpia de un proyecto**, si la tiene y esta en este equipo.
pub fn limpio_de(raiz: &Path, ficha: &str) -> Option<PathBuf> {
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, ficha);
    let p: pixpin_proyecto::Proyecto =
        serde_json::from_str(&std::fs::read_to_string(carpeta.join("proyecto.json")).ok()?).ok()?;
    pixpin_proyecto::vista::ruta_real(raiz, ficha, p.pdf_limpio.as_deref()?).filter(|r| r.is_file())
}

/// Apunta la copia limpia de un documento. Lo llama quien encuentra el
/// documento de un proyecto, que es quien sabe de que proyecto es.
pub fn apuntar(documento: &Path, limpio: Option<PathBuf>) {
    let Some(limpio) = limpio.filter(|l| l != documento) else {
        return;
    };
    if let Ok(mut e) = estado().lock() {
        e.limpias.insert(documento.to_path_buf(), limpio);
    }
}

/// La copia limpia apuntada de un documento o, si no se apunto, la de un
/// proyecto nacido aqui (`archivos/doc-<t>.pdf` junto a su `proyecto.json`).
fn limpio_por_ruta(documento: &Path) -> Option<PathBuf> {
    if let Some(l) = estado().lock().ok()?.limpias.get(documento).cloned() {
        return Some(l);
    }
    let carpeta = documento.parent()?.parent()?;
    let p: pixpin_proyecto::Proyecto =
        serde_json::from_str(&std::fs::read_to_string(carpeta.join("proyecto.json")).ok()?).ok()?;
    let origen = carpeta.join(p.pdf_origen.as_deref()?);
    if !mismo_fichero(&origen, documento) {
        return None;
    }
    Some(carpeta.join(p.pdf_limpio.as_deref()?)).filter(|l| l.is_file())
}

fn mismo_fichero(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// **Que pintar cuando el documento no se deja**: la primera vez se rehace
/// desde la copia limpia y se devuelve el propio documento, ya sano; si ya
/// se rehizo (o no se puede escribir), la copia limpia. `None` sin copia o
/// con la copia rota tambien: entonces no hay nada mejor que intentar.
pub fn reparar(documento: &Path) -> Option<PathBuf> {
    let limpio = limpio_por_ruta(documento).filter(|l| l.is_file())?;
    let bytes = std::fs::read(&limpio).ok()?;
    // La copia tiene que ser un PDF que se lee: pisar un documento roto con
    // otro roto solo cambia uno por otro.
    pixpin_pdf::union::contar_paginas(&bytes)?;
    let ya = estado()
        .lock()
        .ok()
        .is_some_and(|e| e.rehechos.contains(documento));
    if ya {
        return Some(limpio);
    }
    if let Ok(mut e) = estado().lock() {
        e.rehechos.insert(documento.to_path_buf());
    }
    // Al lado y luego el nombre: si algo se tuerce a mitad, el documento de
    // alguien no puede quedarse a medias.
    let temporal = documento.with_extension("pdf.sano");
    let hecho =
        std::fs::write(&temporal, &bytes).and_then(|_| std::fs::rename(&temporal, documento));
    match hecho {
        Ok(()) => {
            tracing::warn!(documento = %documento.display(), "documento del proyecto roto: rehecho desde su copia limpia");
            // Lo pintado del documento roto (o lo que fallo) ya no vale.
            Some(documento.to_path_buf())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&temporal);
            tracing::warn!(?e, documento = %documento.display(), "no se pudo rehacer el documento: se pinta su copia limpia");
            Some(limpio)
        }
    }
}

/// **El documento para el fondo del lienzo, sin lo que el movil cocio
/// dentro** (`pixpin_pdf::cocido`). Lo anotado sobre la pagina se pinta
/// encima, vivo, desde su hoja; si el fondo lo trae tambien, se ve doble (el
/// recuadro de una zona vinculada salia dos veces). Devuelve una copia del
/// documento de antes de la primera capa de PixPin, guardada en
/// `cache/sin-cocer/`, o el propio documento si no lleva nada cocido o si la
/// copia no tiene esa `pagina` (una hoja pegada despues de anotar).
///
/// Se recuerda por ruta, tamano y fecha: el documento se lee entero (el del
/// usuario, 11 MB) una vez, no en cada lienzo que se abre.
// La memoria interna ya usa el alias `Clave`; el resto es de un solo uso.
#[allow(clippy::type_complexity)]
pub fn sin_lo_cocido(raiz: &Path, documento: &Path, pagina: u32) -> PathBuf {
    type Clave = (PathBuf, u64, Option<std::time::SystemTime>);
    static RECUERDO: OnceLock<Mutex<HashMap<Clave, Option<(PathBuf, u32)>>>> = OnceLock::new();
    let Ok(meta) = std::fs::metadata(documento) else {
        return documento.to_path_buf();
    };
    let clave: Clave = (documento.to_path_buf(), meta.len(), meta.modified().ok());
    let recuerdo = RECUERDO.get_or_init(Default::default);
    let sabido = recuerdo.lock().ok().and_then(|r| r.get(&clave).cloned());
    let limpio = match sabido {
        Some(v) => v.filter(|(r, _)| r.is_file()),
        None => {
            let hecho = copia_sin_lo_cocido(raiz, documento, &clave);
            if let Ok(mut r) = recuerdo.lock() {
                r.insert(clave, hecho.clone());
            }
            hecho
        }
    };
    match limpio {
        Some((ruta, paginas)) if pagina < paginas => ruta,
        _ => documento.to_path_buf(),
    }
}

fn copia_sin_lo_cocido(
    raiz: &Path,
    documento: &Path,
    clave: &(PathBuf, u64, Option<std::time::SystemTime>),
) -> Option<(PathBuf, u32)> {
    use std::hash::{Hash, Hasher};
    let bytes = std::fs::read(documento).ok()?;
    let largo = pixpin_pdf::cocido::largo_sin_lo_cocido(&bytes)?;
    let antes = &bytes[..largo];
    // Tiene que ser un PDF que se lee entero: si no, mejor el documento con
    // lo cocido que un fondo en blanco.
    let paginas = pixpin_pdf::union::contar_paginas(antes)?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    clave.hash(&mut h);
    let destino = raiz
        .join("cache")
        .join("sin-cocer")
        .join(format!("{:016x}.pdf", h.finish()));
    if !destino.is_file() {
        std::fs::create_dir_all(destino.parent()?).ok()?;
        let temporal = destino.with_extension("pdf.tmp");
        std::fs::write(&temporal, antes)
            .and_then(|_| std::fs::rename(&temporal, &destino))
            .inspect_err(|e| tracing::warn!(?e, "no se pudo guardar el documento sin lo cocido"))
            .ok()?;
    }
    tracing::info!(
        documento = %documento.display(),
        de = bytes.len(),
        queda = largo,
        "fondo del lienzo sin lo anotado que el movil cocio en el PDF"
    );
    Some((destino, paginas))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Lo que hace el movil al anotar: una revision anadida al final con la
    /// capa `PixPin — pagina 1`.
    fn con_capa_del_movil(pdf: &[u8]) -> Vec<u8> {
        let mut v = pdf.to_vec();
        v.extend_from_slice(
            b"99 0 obj\n<</Type /OCG /Name <feff00500069007800500069006e>>>\nendobj\nstartxref\n0\n%%EOF\n",
        );
        v
    }

    #[test]
    fn el_fondo_del_lienzo_no_trae_lo_que_el_movil_cocio_en_el_pdf() {
        let raiz = std::env::temp_dir().join(format!("pixpin-sin-cocer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        let bueno = sano();
        let doc = raiz.join("limpio-1.pdf");
        std::fs::write(&doc, con_capa_del_movil(&bueno)).unwrap();
        let fondo = sin_lo_cocido(&raiz, &doc, 1);
        assert_ne!(fondo, doc);
        assert_eq!(
            std::fs::read(&fondo).unwrap(),
            bueno,
            "el PDF de antes de anotar"
        );
        assert_eq!(
            pixpin_pdf::Documento::abrir(&fondo).unwrap().paginas(),
            2,
            "y Windows lo pinta"
        );
        // Caso negativo: una pagina que la copia no tiene sale del documento.
        assert_eq!(sin_lo_cocido(&raiz, &doc, 5), doc);
        // Caso negativo: un documento sin nada cocido se usa tal cual.
        let limpio = raiz.join("otro.pdf");
        std::fs::write(&limpio, &bueno).unwrap();
        assert_eq!(sin_lo_cocido(&raiz, &limpio, 0), limpio);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    fn proyecto_con_documento(
        nombre: &str,
        documento: &[u8],
        limpio: Option<&[u8]>,
    ) -> (PathBuf, PathBuf) {
        let raiz =
            std::env::temp_dir().join(format!("pixpin-pdf-sano-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let carpeta = raiz.join("p1");
        std::fs::create_dir_all(carpeta.join("archivos")).unwrap();
        std::fs::write(carpeta.join("archivos/doc-1.pdf"), documento).unwrap();
        let mut p = pixpin_proyecto::Proyecto {
            pdf_origen: Some("archivos/doc-1.pdf".into()),
            ..Default::default()
        };
        if let Some(l) = limpio {
            std::fs::write(carpeta.join("archivos/limpio-1.pdf"), l).unwrap();
            p.pdf_limpio = Some("archivos/limpio-1.pdf".into());
        }
        std::fs::write(
            carpeta.join("proyecto.json"),
            serde_json::to_string(&p).unwrap(),
        )
        .unwrap();
        (raiz, carpeta.join("archivos/doc-1.pdf"))
    }

    fn sano() -> Vec<u8> {
        let hoja = pixpin_codec::imagen::ImagenRgba {
            ancho: 20,
            alto: 30,
            pixeles: [255u8, 255, 255, 255].repeat(600),
        };
        pixpin_pdf::union::de_imagenes(&[hoja.clone(), hoja]).unwrap()
    }

    #[test]
    fn un_documento_roto_se_rehace_desde_su_copia_limpia_y_la_segunda_vez_se_pinta_la_copia() {
        let bueno = sano();
        // Lo que dejaban dos escritores a la vez: trozos de dos ficheros.
        let roto = [&bueno[..bueno.len() / 2], &bueno[..bueno.len() / 3]].concat();
        let (raiz, doc) = proyecto_con_documento("roto", &roto, Some(&bueno));
        assert!(
            pixpin_pdf::Documento::abrir(&doc).is_err()
                || pixpin_pdf::union::contar_paginas(&roto).is_none()
        );
        assert_eq!(reparar(&doc), Some(doc.clone()));
        assert_eq!(
            std::fs::read(&doc).unwrap(),
            bueno,
            "el documento es otra vez la copia limpia"
        );
        assert_eq!(
            pixpin_pdf::Documento::abrir(&doc).unwrap().paginas(),
            2,
            "y Windows lo pinta"
        );
        // Si aun asi fallara, no se vuelve a copiar: se pinta la copia.
        assert_eq!(reparar(&doc), Some(raiz.join("p1/archivos/limpio-1.pdf")));
        assert!(
            !doc.with_extension("pdf.sano").exists(),
            "no queda el temporal"
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn sin_copia_limpia_o_con_ella_rota_no_se_toca_nada() {
        let (raiz, doc) = proyecto_con_documento("sin", b"%PDF-roto", None);
        assert_eq!(reparar(&doc), None);
        assert_eq!(std::fs::read(&doc).unwrap(), b"%PDF-roto");
        let _ = std::fs::remove_dir_all(&raiz);
        let (raiz, doc) = proyecto_con_documento("rota", b"%PDF-roto", Some(b"%PDF-tambien"));
        assert_eq!(
            reparar(&doc),
            None,
            "pisar un roto con otro roto no arregla nada"
        );
        assert_eq!(std::fs::read(&doc).unwrap(), b"%PDF-roto");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// Un documento que llego del movil (`pixpin:files/…`) no esta junto a
    /// su proyecto: su copia se sabe porque se apunto al buscarlo.
    #[test]
    fn la_copia_apuntada_vale_para_un_documento_que_no_esta_en_su_carpeta() {
        let dir =
            std::env::temp_dir().join(format!("pixpin-pdf-sano-apuntado-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let doc = dir.join("documento.pdf");
        let limpio = dir.join("limpio.pdf");
        std::fs::write(&doc, b"basura").unwrap();
        std::fs::write(&limpio, sano()).unwrap();
        assert_eq!(reparar(&doc), None, "sin apuntar no se sabe de quien es");
        apuntar(&doc, Some(limpio.clone()));
        assert_eq!(reparar(&doc), Some(doc.clone()));
        assert!(pixpin_pdf::union::contar_paginas(&std::fs::read(&doc).unwrap()).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
