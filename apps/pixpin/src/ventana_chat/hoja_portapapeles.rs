//! **Ctrl+V y Ctrl+C en la hoja de calculo del chat** (J2).
//!
//! Antes no habia ninguno de los dos: Ctrl+V llegaba como la letra 0x16 y
//! la hoja la tiraba. Ahora pegar trae las celdas de Excel, Sheets o
//! LibreOffice en su sitio —con las formulas de Sheets pasadas de R1C1 a
//! `A1`, los numeros exactos y la negrita— y copiar deja los valores como
//! texto con tabuladores y las formulas en la tabla HTML, como el movil
//! (`motor/PortapapelesDeTabla.kt`). Lo que se entiende esta en
//! `pixpin_proyecto::portapapeles_tabla`; el portapapeles de Windows, en
//! `pixpin_codec::portapapeles::tabla`.

use super::HojaAbierta;
use pixpin_proyecto::portapapeles_tabla;

/// Pega en la celda elegida. Escribiendo en una celda, pega el texto dentro
/// de lo que se escribe (en una linea: la barra es de una).
pub(super) fn pegar(h: &mut HojaAbierta) {
    let Some((html, texto)) = pixpin_codec::portapapeles::tabla::leer_tabla() else {
        return;
    };
    if let Some(edicion) = h.edicion.as_mut() {
        if let Some(t) = texto {
            edicion.push_str(&t.replace(['\r', '\n', '\t'], " ").trim_end().to_string());
        }
        return;
    }
    let Some(filas) = portapapeles_tabla::leer(html.as_deref(), texto.as_deref()) else {
        return;
    };
    let cambiadas = portapapeles_tabla::pegar(&mut h.tabla, h.sel, &filas);
    if cambiadas > 0 {
        h.tocada = true;
        h.valores = Default::default();
    }
    tracing::info!(filas = filas.len(), cambiadas, "pegado en la hoja");
}

/// Copia la celda elegida: su valor para pegarlo en un chat y su formula
/// para pegarla en otra hoja. Escribiendo, lo escrito.
pub(super) fn copiar(h: &HojaAbierta) {
    let hecho = match &h.edicion {
        Some(t) => pixpin_codec::portapapeles::copiar_texto(t),
        None => {
            let (tsv, html) = portapapeles_tabla::copiar(
                &h.tabla,
                h.sel,
                h.sel,
                pixpin_shell::entorno::separador_decimal(),
            );
            pixpin_codec::portapapeles::tabla::copiar_tabla(&tsv, &html)
        }
    };
    if let Err(e) = hecho {
        tracing::warn!(?e, "no se pudo copiar la celda");
    }
}
