//! Dialogos nativos de Win32 que no dependen de una ventana propia.
//!
//! Hoy solo hay uno: un cuadro de error para fallos que ocurren antes de
//! tener bandeja, ventana o consola con las que avisar al usuario (p. ej.
//! un fallo de arranque). Vive aqui y no en `apps/pixpin` a proposito: el
//! documento maestro reparte los crates entre los que tienen
//! `#![forbid(unsafe_code)]` y los que hablan con Win32 de forma auditada,
//! y el ejecutable no es ninguno de los dos -- cualquier `unsafe` suyo
//! quedaria fuera de esa contabilidad. Ademas, S1-B va a necesitar el mismo
//! tipo de dialogo, asi que de paso queda reutilizable.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
use windows::core::HSTRING;

/// Muestra un cuadro de error modal, sin ventana propietaria.
///
/// Pensado como ultimo recurso cuando no hay bandeja, ventana ni consola con
/// las que avisar al usuario de un fallo. No traduce `mensaje` ni `titulo`:
/// quien la llame es responsable de pasar texto ya traducido si tiene un
/// catalogo cargado, o texto sin traducir si el fallo ocurrio antes de
/// tenerlo (mejor un mensaje en el idioma equivocado que ningun mensaje).
pub fn mostrar_error_fatal(titulo: &str, mensaje: &str) {
    let mensaje = HSTRING::from(mensaje);
    let titulo = HSTRING::from(titulo);

    // SAFETY: `mensaje` y `titulo` son HSTRING propias, vivas durante toda la
    // llamada; None como ventana propietaria es un uso valido y documentado
    // de MessageBoxW (crea un cuadro sin padre).
    unsafe {
        let _ = MessageBoxW(None, &mensaje, &titulo, MB_OK | MB_ICONERROR);
    }
}

/// Pregunta si continuar con algo irreversible. `false` si el usuario dice
/// que no **y tambien** si cierra el cuadro: ante la duda, no se destruye.
///
/// El boton por defecto es «No» (`MB_DEFBUTTON2`) a proposito: quien pulsa
/// Intro por inercia sobre un cuadro que no leyo no debe borrar nada.
pub fn confirmar_destructivo(propietaria: HWND, titulo: &str, mensaje: &str) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{IDYES, MB_DEFBUTTON2, MB_ICONWARNING, MB_YESNO};

    let mensaje = HSTRING::from(mensaje);
    let titulo = HSTRING::from(titulo);
    // SAFETY: HSTRING propias vivas durante la llamada; la ventana
    // propietaria es del llamante y esta viva mientras el cuadro es modal.
    let r = unsafe {
        MessageBoxW(
            Some(propietaria),
            &mensaje,
            &titulo,
            MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
        )
    };
    r == IDYES
}

/// Pregunta de si/no con el icono de pregunta y Si como opcion por defecto:
/// para «¿guardar lo dibujado?» (D54), donde perder el trabajo es peor que
/// crear un pin de mas.
pub fn preguntar(propietaria: HWND, titulo: &str, mensaje: &str) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{IDYES, MB_ICONQUESTION, MB_YESNO};

    let mensaje = HSTRING::from(mensaje);
    let titulo = HSTRING::from(titulo);
    // SAFETY: HSTRING propias vivas durante la llamada; la ventana
    // propietaria es del llamante y esta viva mientras el cuadro es modal.
    unsafe {
        MessageBoxW(
            Some(propietaria),
            &mensaje,
            &titulo,
            MB_YESNO | MB_ICONQUESTION,
        ) == IDYES
    }
}
