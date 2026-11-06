//! **Exportar**: PDF de vectores, PNG, SVG y los formatos de PinDesk (la
//! figura de la biblioteca y el `.excalidraw` del lienzo). Todo puro: entra
//! un documento ya maquetado y salen bytes.

pub mod figura;
pub mod letras;
pub mod pdf;
pub mod png;
pub mod svg;

use crate::campos::{self, Contexto};
use crate::diseno;
use crate::fecha::Fecha;
use crate::modelo::Documento;
use crate::prim::PaginaHecha;

/// **Todas las paginas** de un documento (una por fecha de su repeticion).
pub fn paginas(doc: &Documento, hoy: Fecha) -> Vec<PaginaHecha> {
    contextos(doc, hoy)
        .iter()
        .map(|c| diseno::maquetar(doc, c))
        .collect()
}

/// Los contextos de sus paginas.
pub fn contextos(doc: &Documento, hoy: Fecha) -> Vec<Contexto> {
    campos::paginas(doc.repeticion.as_ref(), hoy, &doc.idioma)
}

/// Una sola pagina (la `n`, desde 0; fuera de rango, la ultima).
pub fn pagina(doc: &Documento, hoy: Fecha, n: usize) -> PaginaHecha {
    let c = contextos(doc, hoy);
    let ctx = c
        .get(n)
        .or(c.last())
        .copied()
        .unwrap_or_else(|| Contexto::nuevo(hoy, &doc.idioma));
    diseno::maquetar(doc, &ctx)
}
