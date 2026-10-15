//! Utilidades SOLO para tests de este crate.
//!
//! WGC y Desktop Duplication no entregan fotogramas de un escritorio
//! quieto; sin esto, los tests dependerian de que el usuario tuviera un
//! video abierto (paso, y costo una tarde de depuracion).
#![cfg(test)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Genera movimiento real en pantalla mientras dura la medicion: una
/// ventana STATIC de 64 px en la esquina que se repinta y vaivenea 1 px.
/// Dibujar al DC de pantalla no pasa por DWM y ni WGC ni DDA lo ven: hace
/// falta una ventana real que fuerce la recomposicion.
pub(crate) fn con_movimiento<T>(dur: Duration, medir: impl FnOnce() -> T) -> T {
    let seguir = Arc::new(AtomicBool::new(true));
    let seguir_hilo = Arc::clone(&seguir);
    let hilo = std::thread::spawn(move || {
        use windows::Win32::Graphics::Gdi::{
            CreateSolidBrush, DeleteObject, FillRect, GetDC, ReleaseDC,
        };
        use windows::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos,
            WINDOW_EX_STYLE, WS_POPUP, WS_VISIBLE,
        };
        use windows::core::w;
        // SAFETY: ventana y brochas propias del hilo, destruidas al final;
        // FillRect sobre el DC de cliente de la propia ventana.
        unsafe {
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!(""),
                WS_POPUP | WS_VISIBLE,
                0,
                0,
                64,
                64,
                None,
                None,
                Some(GetModuleHandleW(None).unwrap().into()),
                None,
            )
            .expect("ventana de movimiento");
            let a = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x0000FF));
            let b = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00FF00));
            let r = windows::Win32::Foundation::RECT {
                left: 0,
                top: 0,
                right: 64,
                bottom: 64,
            };
            let mut alterna = false;
            let fin = Instant::now() + dur;
            while seguir_hilo.load(Ordering::Relaxed) && Instant::now() < fin {
                let dc = GetDC(Some(hwnd));
                FillRect(dc, &r, if alterna { a } else { b });
                ReleaseDC(Some(hwnd), dc);
                let dx = if alterna { 1 } else { 0 };
                let _ = SetWindowPos(hwnd, None, dx, 0, 64, 64, SWP_NOZORDER | SWP_NOACTIVATE);
                alterna = !alterna;
                std::thread::sleep(Duration::from_millis(10));
            }
            let _ = DeleteObject(a.into());
            let _ = DeleteObject(b.into());
            let _ = DestroyWindow(hwnd);
        }
    });
    let resultado = medir();
    seguir.store(false, Ordering::Relaxed);
    let _ = hilo.join();
    resultado
}
