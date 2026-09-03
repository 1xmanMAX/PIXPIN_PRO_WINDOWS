//! Entrada sintetizada y sondeo de teclas para la captura con scroll (D75).
//!
//! Windows entrega la rueda a la ventana que hay bajo el cursor sin
//! activarla (ajuste por defecto desde Windows 10), asi que hacer scroll en
//! la ventana de abajo es mover el cursor a la region y enviar la rueda con
//! `SendInput`. Y como el overlay esta OCULTO mientras se hace scroll, el
//! Escape del usuario no llega por ningun WndProc: se sondea.

use pixpin_geom::Punto;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_WHEEL, MOUSEINPUT, SendInput,
    VK_ESCAPE,
};
use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;

/// Una muesca de rueda estandar.
const WHEEL_DELTA: i32 = 120;

/// Coloca el cursor en `p` (escritorio virtual) y envia `muescas` de rueda
/// hacia abajo (negativas: hacia arriba).
pub fn rueda_en(p: Punto, muescas: i32) {
    let entrada = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                // La palabra baja lleva el giro con signo; negativo = abajo.
                mouseData: (-(WHEEL_DELTA * muescas)) as u32,
                dwFlags: MOUSEEVENTF_WHEEL,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    // SAFETY: SetCursorPos no tiene precondiciones; SendInput recibe un
    // array propio de un elemento con el tamano correcto de la estructura.
    unsafe {
        let _ = SetCursorPos(p.x, p.y);
        let _ = SendInput(&[entrada], size_of::<INPUT>() as i32);
    }
}

/// Si Escape esta pulsado ahora mismo. Para el bucle de scroll, que no
/// tiene ventana con foco a la que le llegue la tecla (D76).
pub fn escape_pulsado() -> bool {
    // SAFETY: consulta pura del estado del teclado.
    unsafe { (GetAsyncKeyState(VK_ESCAPE.0 as i32) as u16 & 0x8000) != 0 }
}

/// Si algun boton principal del raton esta pulsado ahora mismo. El gancho
/// de gestos se traga la pulsacion, pero el estado asincrono del sistema si
/// la refleja, asi que sirve de segunda opinion cuando la bandera del
/// gancho no basta.
pub fn boton_del_raton_pulsado() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_LBUTTON, VK_RBUTTON};
    // SAFETY: consulta pura del estado de los botones.
    unsafe {
        (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0
            || (GetAsyncKeyState(VK_RBUTTON.0 as i32) as u16 & 0x8000) != 0
    }
}

/// Si estan pulsadas ahora mismo las mayusculas y Alt, en ese orden. Las
/// consulta el anotador antes de cada evento de puntero: mayusculas
/// restringe angulos y proporciones, y Alt duplica al arrastrar.
pub fn modificadores() -> (bool, bool) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_MENU, VK_SHIFT};
    // SAFETY: consulta pura del estado del teclado.
    unsafe {
        (
            (GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0,
            (GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0,
        )
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn sin_nadie_pulsando_escape_no_esta_pulsado() {
        // Caso negativo del sondeo: si esto diera true, la captura con
        // scroll se pararia sola en el primer paso.
        assert!(!escape_pulsado());
    }
}
