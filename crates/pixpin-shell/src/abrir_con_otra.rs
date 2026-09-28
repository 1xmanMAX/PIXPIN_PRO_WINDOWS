//! «Abrir con otra app»: el dialogo «Abrir con» de Windows, el de verdad.
//!
//! Es lo que en el movil hace el selector del sistema (`ui/AbrirCon.kt`):
//! elegir CON QUE se abre este fichero, no abrirlo con lo de siempre. Antes
//! el chat llamaba a `abrir::abrir`, que lo abre con la aplicacion asociada;
//! para un PDF que uno quiere firmar con otro programa eso no servia.
//!
//! `SHOpenWithDialog` es el mismo cuadro que sale en el Explorador con el
//! boton derecho > «Abrir con» > «Elegir otra aplicacion»: lista lo que hay
//! instalado, deja buscar en la tienda y, marcado, recuerda la eleccion. No
//! pesa nada: es de shell32 y ya esta cargado.
//!
//! Sin prueba automatica, como `elegir` y `guardar`: el cuadro es modal y
//! espera a una persona. Se declara en vez de disimularlo.

use std::path::Path;

use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
use windows::Win32::UI::Shell::{
    OAIF_ALLOW_REGISTRATION, OAIF_EXEC, OAIF_REGISTER_EXT, OPENASINFO, SHOpenWithDialog,
};
use windows::core::{HRESULT, HSTRING, PCWSTR};

/// Pregunta con que abrir `ruta` y lo abre con lo elegido.
///
/// `Ok(true)` si se abrio, `Ok(false)` si el usuario cerro el cuadro sin
/// elegir (eso no es un fallo: es cambiar de idea), y el error de Windows en
/// otro caso.
///
/// Con `OAIF_EXEC` el propio cuadro abre el fichero; con los otros dos, la
/// casilla «Usar siempre esta aplicacion» funciona como en el Explorador.
pub fn abrir_con_otra(padre: HWND, ruta: &Path) -> windows::core::Result<bool> {
    let fichero = HSTRING::from(ruta.as_os_str());
    let info = OPENASINFO {
        pcszFile: PCWSTR(fichero.as_ptr()),
        pcszClass: PCWSTR::null(),
        oaifInFlags: OAIF_EXEC | OAIF_ALLOW_REGISTRATION | OAIF_REGISTER_EXT,
    };
    // SAFETY: `info` y la cadena a la que apunta viven durante toda la
    // llamada (es sincrona: vuelve cuando se cierra el cuadro) y el padre es
    // una ventana de este hilo o nula.
    match unsafe { SHOpenWithDialog(Some(padre), &info) } {
        Ok(()) => Ok(true),
        Err(e) if e.code() == HRESULT::from_win32(ERROR_CANCELLED.0) => Ok(false),
        Err(e) => Err(e),
    }
}
