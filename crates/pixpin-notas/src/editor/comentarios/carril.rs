//! **La ventana de las tarjetas** (H12, 1-oct): una hija del marco donde se
//! pintan las tarjetas de los comentarios. Al lado del papel parece papel
//! (las tarjetas flotan en su margen); en una ventana estrecha es el cajon y
//! va **encima** del texto, por eso es una ventana y no algo pintado en el
//! marco (el marco no puede pintar encima del `RichEdit`, que es otra
//! ventana). Lo que se pinta y donde se pulsa es de `panel_comentarios`;
//! aqui solo se pinta en memoria y se copia (sin parpadeo al desplazar), y
//! el clic y la rueda se le pasan al bucle y a la nota.

use std::cell::Cell;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::disposicion::Caja;
use crate::panel_comentarios as panel;

thread_local! {
    /// Donde esta la ventana dentro del marco: lo que hay que sumar a lo
    /// suyo para hablar en pixeles del marco (los del panel).
    static ORIGEN: Cell<(i32, i32)> = const { Cell::new((0, 0)) };
}

/// Crea la ventana (escondida) dentro del marco.
pub(super) fn crear(marco: HWND) -> Option<HWND> {
    // SAFETY: clase y ventana de este hilo; registrar la clase dos veces
    // falla sin dano; la ventana la destruye `desmontar`.
    unsafe {
        let instancia = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).ok()?;
        let clase = WNDCLASSW {
            lpfnWndProc: Some(procedimiento),
            hInstance: instancia.into(),
            lpszClassName: w!("PixPinTarjetasNota"),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&clase);
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("PixPinTarjetasNota"),
            w!(""),
            // Sin pintar encima del cuadro de escribir, que va sobre ella.
            WS_CHILD | WS_CLIPSIBLINGS,
            0,
            0,
            1,
            1,
            Some(marco),
            None,
            Some(instancia.into()),
            None,
        )
        .ok()
    }
}

/// La pone en `caja` (pixeles del marco), encima de la nota, y la repinta.
pub(super) fn colocar(h: HWND, caja: Caja) {
    ORIGEN.with(|o| o.set((caja.x, caja.y)));
    // SAFETY: ventana propia.
    unsafe {
        let _ = SetWindowPos(h, Some(HWND_TOP), caja.x, caja.y, caja.an.max(1), caja.al.max(1), SWP_NOACTIVATE | SWP_SHOWWINDOW);
        let _ = InvalidateRect(Some(h), None, false);
    }
}

pub(super) fn esconder(h: HWND) {
    // SAFETY: ventana propia.
    unsafe {
        let _ = ShowWindow(h, SW_HIDE);
    }
}

fn punto(l: LPARAM) -> (i32, i32) {
    ((l.0 & 0xffff) as i16 as i32, ((l.0 >> 16) & 0xffff) as i16 as i32)
}

unsafe extern "system" fn procedimiento(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            // SAFETY: pintado de una ventana propia entre Begin y End; el DC y
            // el mapa de memoria se crean y se sueltan aqui.
            unsafe {
                let dc = BeginPaint(h, &mut ps);
                let mut r = RECT::default();
                let _ = GetClientRect(h, &mut r);
                let (an, al) = (r.right.max(1), r.bottom.max(1));
                let mem = CreateCompatibleDC(Some(dc));
                let mapa = CreateCompatibleBitmap(dc, an, al);
                let viejo = SelectObject(mem, HGDIOBJ(mapa.0));
                let pintor = super::super::VISTA.with(|v| v.borrow().as_ref().map(|v| v.pintor.clone()));
                if let Some(p) = pintor {
                    panel::pintar_en(mem, &p, ORIGEN.with(Cell::get));
                }
                let _ = BitBlt(dc, 0, 0, an, al, Some(mem), 0, 0, SRCCOPY);
                SelectObject(mem, viejo);
                let _ = DeleteObject(HGDIOBJ(mapa.0));
                let _ = DeleteDC(mem);
                let _ = EndPaint(h, &ps);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let (x, y) = punto(l);
            let (ox, oy) = ORIGEN.with(Cell::get);
            // En pixeles del marco, que es como habla el panel; lo atiende el
            // bucle (`comentarios::clic`).
            if panel::clic(x + ox, y + oy) {
                super::super::apuntar(super::super::Orden::ClicFuera);
            }
            LRESULT(0)
        }
        // La rueda encima de las tarjetas mueve la nota (y ellas la siguen).
        WM_MOUSEWHEEL => {
            let edit = super::super::EDIT.with(|e| e.get());
            if edit != 0 {
                // SAFETY: el control es hermano de esta ventana y vive con ella.
                unsafe {
                    let _ = PostMessageW(Some(HWND(edit as *mut _)), m, w, l);
                }
            }
            LRESULT(0)
        }
        // SAFETY: lo demas, a Windows con los mismos argumentos.
        _ => unsafe { DefWindowProcW(h, m, w, l) },
    }
}
