//! **Exportar la nota a Word** (H12, 1-oct): lo que comparten la flecha
//! del titulo del editor («Exportar a Word…») y la hoja de compartir de la
//! aplicacion (formato «Word»). El `.docx` lo escribe
//! `pixpin_docs::exportar_docx` (puro y probado); aqui solo se le dan las
//! fotos del disco, la letra de la vista y los comentarios.

use std::path::{Path, PathBuf};

use pixpin_docs::exportar_docx::{self, Letra, Nota, imagen};
use pixpin_docs::md_comentarios::Comentarios;

use crate::vista::Vista;

/// Los bytes de una foto como los quiere Word: tal cual si es PNG, JPEG,
/// GIF o BMP (lo mira por los bytes); si es otra cosa que Windows sabe
/// leer (WebP), pasada a PNG. Nada si no se lee.
pub fn bytes_de_foto(ruta: &Path) -> Option<Vec<u8>> {
    let bytes = std::fs::read(ruta).ok()?;
    if imagen::leer(&bytes).is_some() {
        return Some(bytes);
    }
    let rgba = pixpin_codec::cargar(ruta).ok()?;
    pixpin_codec::codificar_png(&rgba).ok()
}

/// El `.docx` de la nota y el nombre con que se ofrece guardarlo (sin la
/// extension, que la pone el dialogo).
pub fn exportar(
    markdown: &str,
    nueva: &str,
    vista: &Vista,
    comentarios: Option<&Comentarios>,
    resolver: &dyn Fn(&str) -> Option<PathBuf>,
    autor: &str,
    ahora_ms: i64,
) -> Option<(Vec<u8>, String)> {
    let titulo = match pixpin_docs::md_vivo::titulo(markdown) {
        t if t.trim().is_empty() => nueva.to_string(),
        t => t,
    };
    let fotos = |ruta: &str| resolver(ruta).and_then(|p| bytes_de_foto(&p));
    let bytes = exportar_docx::exportar(&Nota {
        markdown,
        titulo: &titulo,
        letra: Letra {
            cuerpo: vista.cuerpo.clone(),
            titulos: vista.titulos.clone(),
            px: vista.px,
        },
        comentarios,
        imagen: &fotos,
        autor,
        ahora_ms,
    })
    .map_err(|e| tracing::warn!(?e, "no se pudo hacer el .docx de la nota"))
    .ok()?;
    let nombre = exportar_docx::nombre_de_fichero(&titulo);
    let nombre = nombre.strip_suffix(".docx").unwrap_or(&nombre).to_string();
    Some((bytes, nombre))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_nota_sale_en_word_con_su_titulo_por_nombre_y_sus_fotos_del_disco() {
        let dir = std::env::temp_dir().join(format!("pixpin-word-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Una foto de verdad, guardada como PNG por el mismo codec.
        let foto = dir.join("planta.png");
        let rgba = pixpin_codec::ImagenRgba {
            ancho: 4,
            alto: 2,
            pixeles: vec![200; 32],
        };
        std::fs::write(&foto, pixpin_codec::codificar_png(&rgba).unwrap()).unwrap();
        let f = foto.clone();
        let resolver = move |r: &str| (r == "notas/planta.png").then(|| f.clone());
        let (bytes, nombre) = exportar(
            "# Obra: fase 1\n\n![Planta](notas/planta.png)\n![Falta](notas/no.png)",
            "Nota nueva",
            &Vista::default(),
            None,
            &resolver,
            "PC",
            0,
        )
        .unwrap();
        assert_eq!(nombre, "Obra_ fase 1");
        let mut p = pixpin_docs::Paquete::de_bytes(&bytes).unwrap();
        let d = pixpin_docs::docx::de_paquete(&mut p, "x").unwrap();
        assert_eq!(d.imagenes.len(), 1, "la que esta, y la otra como aviso");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sin_titulo_se_llama_como_una_nota_nueva_y_una_foto_rota_no_se_mete() {
        let dir = std::env::temp_dir().join(format!("pixpin-word-rota-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rota = dir.join("rota.png");
        std::fs::write(&rota, b"esto no es una imagen").unwrap();
        assert!(bytes_de_foto(&rota).is_none());
        assert!(bytes_de_foto(&dir.join("no-existe.png")).is_none());
        let (_, nombre) = exportar("\n\n", "Nota nueva", &Vista::default(), None, &|_| None, "", 0).unwrap();
        assert_eq!(nombre, "Nota nueva");
        std::fs::remove_dir_all(&dir).ok();
    }
}
