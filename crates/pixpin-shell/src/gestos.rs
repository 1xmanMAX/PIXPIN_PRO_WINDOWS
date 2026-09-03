//! Gestos de raton con Alt en cualquier punto de la pantalla (D81).
//!
//! `Alt + boton izquierdo` y arrastrar selecciona y copia; `Alt + boton
//! derecho` y arrastrar selecciona y pinea. Los pidio el usuario en lugar de
//! los atajos de teclado `Ctrl+Alt+X` y `Ctrl+Alt+D`.
//!
//! Un gancho de raton de bajo nivel (`WH_MOUSE_LL`) ve la pulsacion antes
//! que la ventana de debajo. Si Alt esta pulsado (y ningun otro modificador),
//! se traga la pulsacion y se avisa a la ventana de mensajes con `WM_GESTO`;
//! el bucle principal abre entonces el overlay ya con el arrastre en marcha.
//! La soltada NO se traga: para entonces el overlay esta delante con la
//! captura del raton y es quien tiene que recibirla.
//!
//! Mientras un overlay esta abierto el gancho se suspende: un Alt+clic
//! dentro del overlay es del overlay, no un gesto nuevo.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HC_ACTION, HHOOK, MSLLHOOKSTRUCT, PostMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_MOUSE_LL, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_RBUTTONDOWN, WM_RBUTTONUP,
};

use crate::ventana::WM_GESTO;

/// La ventana que recibe `WM_GESTO`, como entero para poder vivir en un
/// estatico (el gancho es una funcion libre sin estado propio).
static DESTINO: AtomicIsize = AtomicIsize::new(0);
/// Con el gancho suspendido las pulsaciones pasan sin tocarse.
static SUSPENDIDO: AtomicBool = AtomicBool::new(false);
/// Hay un boton tragado por el gancho y todavia sin soltar. Se lleva aqui
/// y no con `GetAsyncKeyState`: una pulsacion que el gancho se traga nunca
/// llega al estado del teclado del sistema, asi que para Windows el boton
/// jamas estuvo abajo.
static EN_CURSO: AtomicBool = AtomicBool::new(false);

/// Si el boton del ultimo gesto sigue pulsado. El overlay lo consulta al
/// abrirse para arrancar ya arrastrando.
pub fn gesto_en_curso() -> bool {
    EN_CURSO.load(Ordering::SeqCst)
}

/// El gancho instalado. Al soltarlo se desinstala.
pub struct GanchoRaton {
    gancho: HHOOK,
}

impl GanchoRaton {
    /// Instala el gancho; los gestos llegan a `destino` como `WM_GESTO`.
    /// Debe llamarse desde el hilo que bombea mensajes (el principal): los
    /// ganchos de bajo nivel se ejecutan en el hilo que los instalo.
    pub fn instalar(destino: HWND) -> windows::core::Result<Self> {
        DESTINO.store(destino.0 as isize, Ordering::SeqCst);
        SUSPENDIDO.store(false, Ordering::SeqCst);
        // SAFETY: gancho global de bajo nivel con un procedimiento de este
        // modulo; sin hmodule ni hilo porque corre en el hilo llamante.
        let gancho = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(procedimiento), None, 0)? };
        Ok(Self { gancho })
    }

    /// Deja pasar (o vuelve a interceptar) las pulsaciones con Alt.
    pub fn suspender(&self, suspendido: bool) {
        SUSPENDIDO.store(suspendido, Ordering::SeqCst);
    }
}

impl Drop for GanchoRaton {
    fn drop(&mut self) {
        DESTINO.store(0, Ordering::SeqCst);
        // SAFETY: desinstala el gancho que instalo este mismo valor.
        unsafe {
            let _ = UnhookWindowsHookEx(self.gancho);
        }
    }
}

fn pulsada(tecla: VIRTUAL_KEY) -> bool {
    // SAFETY: consulta del estado del teclado, sin precondiciones.
    unsafe { (GetAsyncKeyState(tecla.0 as i32) as u16 & 0x8000) != 0 }
}

/// Si la combinacion de modificadores es exactamente Alt.
fn solo_alt() -> bool {
    pulsada(VK_MENU)
        && !pulsada(VK_CONTROL)
        && !pulsada(VK_SHIFT)
        && !pulsada(VK_LWIN)
        && !pulsada(VK_RWIN)
}

/// Empaqueta un punto en un `LPARAM` como hacen `MAKELPARAM` y los mensajes
/// de raton: x en la palabra baja, y en la alta, ambas con signo.
pub fn empaquetar_punto(x: i32, y: i32) -> LPARAM {
    LPARAM(((x & 0xFFFF) | ((y & 0xFFFF) << 16)) as isize)
}

extern "system" fn procedimiento(codigo: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let mensaje = wparam.0 as u32;
    // La soltada siempre pasa (el overlay la necesita) y cierra el gesto,
    // este suspendido el gancho o no.
    if codigo == HC_ACTION as i32 && (mensaje == WM_LBUTTONUP || mensaje == WM_RBUTTONUP) {
        EN_CURSO.store(false, Ordering::SeqCst);
    }
    if codigo == HC_ACTION as i32 && !SUSPENDIDO.load(Ordering::SeqCst) {
        let boton = match mensaje {
            WM_LBUTTONDOWN => Some(0usize),
            WM_RBUTTONDOWN => Some(1usize),
            _ => None,
        };
        let destino = DESTINO.load(Ordering::SeqCst);
        if let Some(boton) = boton
            && destino != 0
            && solo_alt()
        {
            // SAFETY: con HC_ACTION, lParam apunta a un MSLLHOOKSTRUCT valido
            // durante la llamada.
            let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
            // SAFETY: publicar un mensaje propio en una ventana propia.
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(destino as *mut _)),
                    WM_GESTO,
                    WPARAM(boton),
                    empaquetar_punto(info.pt.x, info.pt.y),
                );
            }
            // Tragar la pulsacion: la ventana de debajo no debe reaccionar.
            EN_CURSO.store(true, Ordering::SeqCst);
            return LRESULT(1);
        }
    }
    // SAFETY: pasar el evento al siguiente gancho de la cadena.
    unsafe { CallNextHookEx(None, codigo, wparam, lparam) }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_punto_empaquetado_se_desempaqueta_con_signo() {
        // Un monitor a la izquierda del principal tiene x negativa; la
        // ventana de mensajes lo desempaqueta igual que aqui.
        let l = empaquetar_punto(-120, 45);
        let x = (l.0 & 0xFFFF) as u16 as i16 as i32;
        let y = ((l.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
        assert_eq!((x, y), (-120, 45));

        let l = empaquetar_punto(2999, 1999);
        let x = (l.0 & 0xFFFF) as u16 as i16 as i32;
        let y = ((l.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
        assert_eq!((x, y), (2999, 1999));
    }
}
