//! **Una nota como Word** en la hoja de compartir (H12, 1-oct): el formato
//! «Word» de una nota Markdown sola, con sus fotos (resueltas como en el
//! editor), sus comentarios y la letra con que se lee. El `.docx` lo hace
//! `pixpin_notas::word` (el mismo que el «Exportar a Word…» del editor).

use std::path::Path;

use pixpin_proyecto::cuaderno::{Clase, Mensaje};

use crate::notas_md::{self, Destino};

/// Si el mensaje es una nota que se exporta a Word: una nota escrita, no
/// una mini-app (una tabla o una lista tienen su propio formato).
pub(crate) fn es_nota(m: &Mensaje) -> bool {
    m.clase == Some(Clase::Nota) && m.miniapp.is_none() && !m.texto.trim().is_empty()
}

/// El `.docx` de la nota y su nombre de fichero (con `.docx`).
pub(crate) fn de_nota(raiz: &Path, proyecto: &str, m: &Mensaje, nueva: &str) -> Option<(Vec<u8>, String)> {
    let destino = Destino::Mensaje {
        proyecto: proyecto.to_string(),
        codigo: m.codigo_unico(),
    };
    let comentarios = notas_md::comentarios::ruta(raiz, &destino)
        .and_then(|f| pixpin_proyecto::comentarios_de_notas::leer(&f))
        .and_then(|t| pixpin_docs::md_comentarios::leer(&t).ok());
    let ajustes = std::fs::read_to_string(raiz.join(notas_md::FICHERO_VISTA)).unwrap_or_default();
    let (vista, _) = pixpin_notas::vista::leer(&ajustes, Some(&notas_md::clave_de_vista(&destino)));
    let resolver = |r: &str| notas_md::adjuntos::resolver(raiz, &destino, r);
    let (bytes, nombre) = pixpin_notas::word::exportar(
        &m.texto,
        nueva,
        &vista,
        comentarios.as_ref(),
        &resolver,
        "",
        pixpin_shell::entorno::ahora_utc_ms(),
    )?;
    Some((bytes, format!("{nombre}.docx")))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn solo_las_notas_escritas_se_exportan_a_word() {
        let nota = Mensaje {
            clase: Some(Clase::Nota),
            texto: "# Hola\n\n| a | b |\n|---|---|\n| 1 | 2 |".into(),
            ..Default::default()
        };
        assert!(es_nota(&nota));
        // Casos negativos: una mini-app, una nota vacia y una foto.
        let mini = Mensaje {
            miniapp: Some("tabla".into()),
            ..nota.clone()
        };
        assert!(!es_nota(&mini));
        assert!(!es_nota(&Mensaje {
            texto: "  ".into(),
            ..nota.clone()
        }));
        assert!(!es_nota(&Mensaje {
            clase: Some(Clase::Imagen),
            ..nota.clone()
        }));
        // Y el Word sale aunque el proyecto no exista en este equipo.
        let raiz = std::env::temp_dir().join(format!("pixpin-compartir-word-{}", std::process::id()));
        let (bytes, nombre) = de_nota(&raiz, "no-esta", &nota, "Nota nueva").unwrap();
        assert_eq!(nombre, "Hola.docx");
        let mut p = pixpin_docs::Paquete::de_bytes(&bytes).unwrap();
        let d = pixpin_docs::docx::de_paquete(&mut p, "x").unwrap();
        assert!(d.bloques.iter().any(|b| b.clase == pixpin_docs::Clase::Fila));
    }
}
