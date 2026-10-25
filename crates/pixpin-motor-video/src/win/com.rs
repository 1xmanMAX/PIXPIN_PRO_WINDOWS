//! COM, Media Foundation y la hora del sistema: lo que necesita el hilo del
//! motor antes de tocar nada.

use std::sync::Once;

use windows::Win32::Media::MediaFoundation::{MF_VERSION, MFSTARTUP_LITE, MFStartup};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

use crate::ErrorMotor;
use crate::reloj::contador_a_unidades;

/// Envuelve un fallo de Windows diciendo en que paso ocurrio: sin esto todo
/// es el mismo `0x80004005` sin contexto.
pub(crate) fn en(paso: &'static str) -> impl Fn(windows::core::Error) -> ErrorMotor {
    move |e| ErrorMotor::Sistema {
        paso,
        detalle: e.to_string(),
    }
}

/// Media Foundation se arranca **una vez por proceso** y no se apaga, como
/// en `pixpin-record` y en el reproductor del pin: `MFStartup` lleva su
/// propia cuenta, asi que los arranques conviven.
static ARRANQUE: Once = Once::new();

pub(crate) fn arrancar_media_foundation() {
    ARRANQUE.call_once(|| {
        // SAFETY: `MFStartup` solo pide la version de la cabecera con la
        // que se compilo (`MF_VERSION`). Si fallara, el fallo reaparece al
        // crear el escritor, que si se cuenta.
        unsafe {
            let _ = MFStartup(MF_VERSION, MFSTARTUP_LITE);
        }
    });
}

/// COM multihilo (MTA) en el hilo del motor, soltado al salir **solo si lo
/// subimos nosotros**: los codificadores de la grafica son asincronos y
/// Media Foundation los llama desde sus propios hilos, que es lo que pide el
/// apartamento multihilo.
pub(crate) struct ComDelHilo {
    nuestro: bool,
}

impl ComDelHilo {
    pub(crate) fn nuevo() -> ComDelHilo {
        // SAFETY: la forma normal de entrar en COM; sin precondiciones. Se
        // guarda si la cuenta subio para deshacerlo con exactitud.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        ComDelHilo {
            // `S_FALSE` (ya estaba en MTA) tambien sube la cuenta;
            // `RPC_E_CHANGED_MODE` (el hilo ya era de apartamento) no.
            nuestro: hr.is_ok(),
        }
    }
}

impl Drop for ComDelHilo {
    fn drop(&mut self) {
        if self.nuestro {
            // SAFETY: empareja el `CoInitializeEx` de arriba; el guardia se
            // declara el primero del hilo y se suelta el ultimo, cuando ya
            // no queda ningun objeto COM vivo.
            unsafe { CoUninitialize() };
        }
    }
}

/// **La hora del sistema en 100 ns**: el mismo contador que fecha los
/// fotogramas de Windows.Graphics.Capture (`SystemRelativeTime`).
pub(crate) fn ahora() -> i64 {
    let mut cuenta = 0i64;
    let mut frecuencia = 0i64;
    // SAFETY: las dos funciones solo escriben en el entero local que se les
    // pasa; desde Windows XP no fallan.
    unsafe {
        let _ = QueryPerformanceCounter(&mut cuenta);
        let _ = QueryPerformanceFrequency(&mut frecuencia);
    }
    contador_a_unidades(cuenta, frecuencia)
}
