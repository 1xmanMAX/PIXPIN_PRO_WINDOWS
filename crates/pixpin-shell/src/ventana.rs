//! La ventana invisible que recibe todo y el bucle que duerme.
//!
//! Es una ventana `HWND_MESSAGE`: no se dibuja, no sale en la barra de tareas
//! y no aparece con Alt+Tab, pero recibe mensajes. Es donde llegan `WM_HOTKEY`
//! y las notificaciones de la bandeja.
//!
//! **El bucle usa `GetMessageW`, que bloquea el hilo hasta que llega algo.**
//! Esa eleccion es la que cumple el objetivo de 0% de CPU en reposo del
//! presupuesto de rendimiento. Un bucle con `PeekMessageW` giraria sin parar y
//! se comeria la bateria de un portatil sin hacer nada.

use std::cell::RefCell;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, HWND_MESSAGE,
    MSG, PostQuitMessage, RegisterClassExW, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_APP, WM_HOTKEY, WNDCLASSEXW,
};
use windows::core::{Result as WinResult, w};

/// Mensaje propio para las notificaciones del icono de bandeja.
pub const WM_BANDEJA: u32 = WM_APP + 1;

/// Identificadores de los elementos del menu de la bandeja.
pub const ID_MENU_CAPTURAR: u32 = 1;
pub const ID_MENU_AJUSTES: u32 = 2;
pub const ID_MENU_SALIR: u32 = 3;

/// Lo que le puede pasar a la aplicacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evento {
    /// Se pulso un atajo global. El numero es el identificador con el que se
    /// registro (ver `atajos.rs`).
    Atajo(u32),
    MenuCapturar,
    MenuAjustes,
    MenuSalir,
    /// Clic izquierdo en el icono de la bandeja.
    IconoPulsado,
}

/// Que hacer despues de atender un evento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Continuar {
    Si,
    No,
}

thread_local! {
    /// Cola de eventos traducidos por el `WndProc`.
    ///
    /// Se usa un `thread_local` en vez de guardar un puntero al callback en
    /// los datos de la ventana porque el `WndProc` es una funcion `extern
    /// "system"` que no puede capturar entorno, y porque toda la interaccion
    /// con ventanas ocurre en el hilo de interfaz por exigencia de Win32.
    static PENDIENTES: RefCell<Vec<Evento>> = const { RefCell::new(Vec::new()) };
}

pub struct VentanaMensajes {
    hwnd: HWND,
}

impl VentanaMensajes {
    pub fn nueva() -> WinResult<Self> {
        const CLASE: windows::core::PCWSTR = w!("PixPinMaxVentanaMensajes");

        // SAFETY: GetModuleHandleW(None) devuelve el modulo del proceso
        // actual, que siempre existe mientras el proceso vive.
        let instancia = unsafe { GetModuleHandleW(None)? };

        let clase = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(procedimiento),
            hInstance: instancia.into(),
            lpszClassName: CLASE,
            ..Default::default()
        };

        // SAFETY: `clase` esta completamente inicializada y su `lpszClassName`
        // apunta a un literal estatico. Registrar la misma clase dos veces
        // devuelve 0 y pone ERROR_CLASS_ALREADY_EXISTS, que aqui es benigno:
        // significa que otra VentanaMensajes ya la registro. Si el fallo
        // fuera por otra razon, CreateWindowExW fallara a continuacion (la
        // clase no habria quedado registrada) y ese `?` es el que reporta el
        // error real; ningun fallo se pierde en silencio.
        unsafe {
            let _ = RegisterClassExW(&clase);
        }

        // SAFETY: la clase esta registrada (por esta llamada o por una
        // anterior) y HWND_MESSAGE es el padre valido para una ventana
        // solo-mensajes.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                CLASE,
                w!("PixPin Max"),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instancia.into()),
                None,
            )?
        };

        Ok(Self { hwnd })
    }

    pub fn handle(&self) -> HWND {
        self.hwnd
    }

    /// Bucle principal. Bloquea el hilo hasta que `al_recibir` devuelve
    /// `Continuar::No` o llega `WM_QUIT`.
    pub fn ejecutar(self, mut al_recibir: impl FnMut(Evento) -> Continuar) {
        let mut mensaje = MSG::default();
        loop {
            // SAFETY: `mensaje` es una estructura propia y valida.
            // GetMessageW devuelve >0 si hay mensaje, 0 al recibir WM_QUIT y
            // -1 si hay error; los dos ultimos casos se tratan igual aqui
            // porque `ejecutar` no devuelve Result y no hay nada mas que
            // hacer salvo dejar de bloquear el hilo.
            let resultado = unsafe { GetMessageW(&mut mensaje, None, 0, 0) };
            if resultado.0 <= 0 {
                break;
            }

            // SAFETY: `mensaje` viene de GetMessageW y es valido.
            unsafe {
                let _ = TranslateMessage(&mensaje);
                DispatchMessageW(&mensaje);
            }

            let eventos: Vec<Evento> = PENDIENTES.with(|p| p.borrow_mut().drain(..).collect());
            for evento in eventos {
                if al_recibir(evento) == Continuar::No {
                    return;
                }
            }
        }
    }
}

impl Drop for VentanaMensajes {
    fn drop(&mut self) {
        // SAFETY: `hwnd` viene de CreateWindowExW y no se ha destruido antes;
        // este tipo no es Clone ni Copy, asi que se destruye exactamente una
        // vez.
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// Traduce mensajes de Win32 a `Evento` y los deja en la cola del hilo.
///
/// No hace trabajo real: cuanto antes vuelva, mas fluido va todo lo demas.
extern "system" fn procedimiento(
    hwnd: HWND,
    mensaje: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{WM_COMMAND, WM_DESTROY, WM_LBUTTONUP};

    let evento = match mensaje {
        WM_HOTKEY => Some(Evento::Atajo(wparam.0 as u32)),
        WM_COMMAND => match (wparam.0 & 0xFFFF) as u32 {
            ID_MENU_CAPTURAR => Some(Evento::MenuCapturar),
            ID_MENU_AJUSTES => Some(Evento::MenuAjustes),
            ID_MENU_SALIR => Some(Evento::MenuSalir),
            _ => None,
        },
        WM_BANDEJA if (lparam.0 as u32) == WM_LBUTTONUP => Some(Evento::IconoPulsado),
        WM_DESTROY => {
            // SAFETY: llamada sin argumentos que solo encola WM_QUIT en la
            // cola de mensajes de este hilo; no toca memoria ajena.
            unsafe { PostQuitMessage(0) };
            None
        }
        _ => None,
    };

    if let Some(evento) = evento {
        PENDIENTES.with(|p| p.borrow_mut().push(evento));
        return LRESULT(0);
    }

    // SAFETY: delegar en el procedimiento por defecto siempre es valido para
    // los mensajes que no tratamos; hwnd, mensaje, wparam y lparam son los
    // que Windows paso a este WndProc.
    unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_crea_y_se_destruye_sin_fugas() {
        let v = VentanaMensajes::nueva().expect("deberia poder crearse");
        assert!(!v.handle().is_invalid(), "el handle no puede ser invalido");
        drop(v);

        // Crear una segunda tras destruir la primera comprueba que la clase de
        // ventana se registra de forma reentrante y que no queda basura.
        let otra = VentanaMensajes::nueva().expect("la segunda tambien");
        assert!(!otra.handle().is_invalid());
    }
}
