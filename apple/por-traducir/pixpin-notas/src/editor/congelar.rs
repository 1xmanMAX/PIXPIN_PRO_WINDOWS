//! **Ni un fotograma a medias** (H12, 1-oct-2026). El usuario: «cuando le
//! doy clic a poner negrita aparecen los asteriscos por un corto tiempo y
//! luego desaparecen… no tiene que haber esa migracion sino directamente el
//! resultado; lo mismo pasa con las imagenes: primero aparece su direccion».
//!
//! Por que pasaba: el texto del control es el Markdown, y el formato que
//! esconde las marcas se pintaba 60 ms despues de cada cambio (o en otro
//! paso). Entre medias el `RichEdit` ya habia pintado `**` (pinta lo que
//! cambia en el acto, sin esperar a `WM_PAINT`).
//!
//! Ahora todo lo que mete o cambia marcas pasa por [`congelado`]: el
//! control deja de pintar (`ITextDocument::Freeze`; y `WM_SETREDRAW` al
//! soltarlo, para que pinte una sola vez), no
//! avisa del cambio (`EM_SETEVENTMASK` sin `ENM_CHANGE`), se cambia el
//! texto, **se le pone el formato** (escondido, tamanos, el hueco de una
//! foto) y solo entonces se descongela y se pinta una vez: el primer
//! fotograma ya es el resultado.
//!
//! El [`ESPIA`] es para las pruebas: apunta lo que el control podria
//! ensenar en cada momento en que no esta congelado, y las pruebas miran
//! que en ninguno haya una marca a la vista.

use std::cell::{Cell, RefCell};

use pixpin_docs::md_edicion;
use windows::Win32::Graphics::Gdi::{RDW_INVALIDATE, RDW_UPDATENOW, RedrawWindow};

use super::*;

thread_local! {
    /// Cuantos congelados hay abiertos ahora (los de [`congelado`] y los de
    /// `pintar`): con alguno, el control no ensena nada.
    pub(super) static HONDO: Cell<u32> = const { Cell::new(0) };
    /// Dentro de un [`congelado`]: los de dentro no vuelven a congelar.
    static DENTRO: Cell<bool> = const { Cell::new(false) };
    /// Lo que vio el espia (solo en las pruebas; `None`, no mira).
    static ESPIA: RefCell<Option<Vec<Fotograma>>> = const { RefCell::new(None) };
}

/// Lo que el control podria ensenar en un momento: su texto y las marcas
/// que estan a la vista (deberian estar escondidas).
#[derive(Debug, Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct Fotograma {
    pub texto: String,
    pub marcas_a_la_vista: Vec<usize>,
}

/// Que formato pintar al acabar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pintado {
    /// Todo (una orden: puede haber metido una foto, una tabla, pegado…).
    Entero,
    /// Solo los renglones tocados (una tecla: a cada pulsacion, barato); el
    /// resto, con el respiro de siempre.
    Tocado,
}

/// **Hace `f` con el control congelado** y le pone el formato antes de
/// soltarlo. Si ya se esta dentro de uno, `f` sin mas: el de fuera pinta.
pub(super) fn congelado<R>(
    e: &mut Estado,
    pintado: Pintado,
    f: impl FnOnce(&mut Estado) -> R,
) -> R {
    if DENTRO.with(Cell::get) {
        return f(e);
    }
    DENTRO.with(|d| d.set(true));
    HONDO.with(|h| h.set(h.get() + 1));
    let edit = e.edit;
    let mascara = enviar(edit, EM_GETEVENTMASK, 0, 0);
    enviar(edit, EM_SETEVENTMASK, 0, mascara & !(ENM_CHANGE as isize));
    if let Some(d) = &e.doc {
        // SAFETY: el documento es el del propio control, vivo mientras el;
        // se descongela abajo.
        unsafe {
            let _ = d.Freeze();
        }
    }
    let antes = leer(edit);
    let r = f(e);
    let ahora = leer(edit);
    let cambio = antes != ahora;
    if cambio {
        match pintado {
            Pintado::Entero => pintar(e, None),
            Pintado::Tocado => {
                let renglones = tocados(&antes, &ahora, seleccion(edit));
                pintar(e, Some(&renglones));
            }
        }
    }
    // Al descongelar el control pintaria lo suyo por su cuenta, encima del
    // hueco de las fotos, y luego se repintarian ellas (un parpadeo): se
    // descongela sin pintar y se pinta todo junto en un `WM_PAINT`. Solo
    // aqui: con `WM_SETREDRAW` apagado mientras se cambia el texto, el
    // control no borra bien lo escondido (medido: «a **b** c» con
    // Retroceso dejaba «a **** c»).
    enviar(edit, WM_SETREDRAW, 0, 0);
    if let Some(d) = &e.doc {
        // SAFETY: como arriba.
        unsafe {
            let _ = d.Unfreeze();
        }
    }
    enviar(edit, WM_SETREDRAW, 1, 0);
    enviar(edit, EM_SETEVENTMASK, 0, mascara);
    HONDO.with(|h| h.set(h.get() - 1));
    DENTRO.with(|d| d.set(false));
    // Las tarjetas, con el texto ya medido otra vez.
    comentarios::componer(e);
    // Una sola vez, ya con todo puesto: el control y lo de encima (fotos,
    // casillas) en el mismo `WM_PAINT`.
    if !e.oculto {
        // SAFETY: ventana propia.
        unsafe {
            let _ = RedrawWindow(Some(edit), None, None, RDW_INVALIDATE | RDW_UPDATENOW);
        }
    }
    fotograma(e.edit);
    if cambio {
        // Lo de despues de un cambio (el titulo, el menu `/`, guardar, el
        // formato entero con su respiro): como si hubiera avisado el control.
        apuntar(Orden::Cambio);
    }
    r
}

/// Los renglones del texto nuevo que cambiaron, y los de lo elegido.
fn tocados(antes: &str, ahora: &str, sel: (usize, usize)) -> Vec<usize> {
    let a: Vec<u16> = antes.encode_utf16().collect();
    let b: Vec<u16> = ahora.encode_utf16().collect();
    let (desde, _, hasta) = diferencia(&a, &b);
    let ls = md_vivo::lineas(ahora);
    let n0 = md_vivo::linea_de(&ls, desde.min(sel.0));
    let n1 = md_vivo::linea_de(&ls, hasta.max(sel.1));
    (n0..=n1).collect()
}

// ---------------------------------------------------------------------------
// El espia de las pruebas

/// Empieza a mirar.
#[cfg(test)]
pub(super) fn espiar() {
    ESPIA.with(|s| *s.borrow_mut() = Some(Vec::new()));
}

/// Lo visto desde [`espiar`], y deja de mirar.
#[cfg(test)]
pub(super) fn fotogramas() -> Vec<Fotograma> {
    ESPIA.with(|s| s.borrow_mut().take()).unwrap_or_default()
}

/// Si el espia mira y el control no esta congelado: apunta lo que se veria.
pub(super) fn fotograma(edit: HWND) {
    if HONDO.with(Cell::get) > 0 || ESPIA.with(|s| s.borrow().is_none()) {
        return;
    }
    let mut ole: *mut core::ffi::c_void = std::ptr::null_mut();
    enviar(edit, EM_GETOLEINTERFACE, 0, &mut ole as *mut _ as isize);
    let doc: Option<ITextDocument> = if ole.is_null() {
        None
    } else {
        // SAFETY: la interfaz la da el control con una referencia de mas, que
        // suelta el `IUnknown` al salir.
        unsafe {
            windows::core::IUnknown::from_raw(ole)
                .cast::<ITextDocument>()
                .ok()
        }
    };
    let texto = leer(edit);
    let marcas = md_edicion::marcas(&texto);
    let a_la_vista = match &doc {
        Some(d) => marcas
            .iter()
            .enumerate()
            .filter(|(_, m)| **m)
            .map(|(p, _)| p)
            // SAFETY: rango del documento vivo del control.
            .filter(|p| unsafe {
                d.Range(*p as i32, *p as i32 + 1)
                    .and_then(|r| r.GetFont())
                    .and_then(|f| f.GetHidden())
                    .unwrap_or(0)
                    == 0
            })
            .collect(),
        None => Vec::new(),
    };
    ESPIA.with(|s| {
        if let Some(v) = s.borrow_mut().as_mut() {
            v.push(Fotograma {
                texto,
                marcas_a_la_vista: a_la_vista,
            });
        }
    });
}

/// Los mensajes tras los que el control puede haber pintado algo nuevo.
pub(super) fn cambia_lo_que_se_ve(m: u32) -> bool {
    matches!(
        m,
        EM_REPLACESEL
            | EM_SETTEXTEX
            | WM_SETTEXT
            | EM_PASTESPECIAL
            | WM_PASTE
            | WM_CHAR
            | WM_KEYDOWN
            | WM_CUT
            | WM_CLEAR
    ) || m == DESHACER_EM
        || m == REHACER_EM
        || m == WM_UNDO
}

/// Deshacer y rehacer del control (el del `EDIT` de siempre y el del
/// `RichEdit`).
pub(super) const DESHACER_EM: u32 = 0x00C7;
pub(super) const REHACER_EM: u32 = 0x0454;

/// **Ctrl+Z y Ctrl+Y (o Ctrl+Mayus+Z)**, si `m` es uno de los dos: se
/// hacen aqui, congelados, para que lo que vuelva (unos `**`) vuelva ya
/// escondido.
pub(super) fn es_deshacer(m: &MSG, ctrl: bool, alt: bool) -> bool {
    m.message == WM_KEYDOWN
        && ctrl
        && !alt
        && (m.wParam.0 == b'Z' as usize || m.wParam.0 == b'Y' as usize)
}

/// Deshace (o rehace) un paso. `true` si habia algo.
pub(super) fn deshacer(e: &mut Estado, rehacer: bool) -> bool {
    congelado(e, Pintado::Entero, |e| {
        let m = if rehacer { REHACER_EM } else { DESHACER_EM };
        enviar(e.edit, m, 0, 0) != 0
    })
}

#[cfg(test)]
mod pruebas;
