//! **«Exportar a Word…»** de la flecha del titulo (H12, 1-oct): la nota tal
//! como esta en el editor (con lo que aun no se haya guardado), con sus
//! fotos, la letra de la vista y los comentarios del panel, al `.docx` que
//! el usuario elija con el «Guardar como» de Windows.

use super::{Estado, markdown};
use crate::menu::{Dibujo, Entrada};
use crate::pintor::Icono;

pub(super) const C_EXPORTAR_WORD: u16 = 140;

/// La entrada del menu de la flecha del titulo.
pub(super) fn entrada(e: &Estado) -> Entrada {
    super::entrada(C_EXPORTAR_WORD, Dibujo::Icono(Icono::Documento), &e.rotulos.exportar_word, "")
}

pub(super) fn a_word(e: &mut Estado) {
    let texto = markdown(e);
    let resolver = |r: &str| (e.resolver)(r);
    let Some((bytes, nombre)) = crate::word::exportar(
        &texto,
        &e.rotulos.nueva,
        &e.vivo.vista,
        Some(&e.comentarios.datos),
        &resolver,
        "",
        pixpin_shell::entorno::ahora_utc_ms(),
    ) else {
        return;
    };
    let tipo = if e.rotulos.tipo_word.is_empty() { "Word" } else { e.rotulos.tipo_word.as_str() };
    if let Some(ruta) = pixpin_shell::guardar::pedir_ruta_para(e.marco, &nombre, tipo, "docx")
        && let Err(err) = std::fs::write(&ruta, &bytes)
    {
        tracing::warn!(?err, ruta = %ruta.display(), "no se pudo guardar el .docx");
    }
}
