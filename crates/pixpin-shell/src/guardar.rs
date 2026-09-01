//! El dialogo "Guardar como" del sistema, con los filtros de imagen.
//!
//! Unica pieza de S1-B2 sin test automatico: un dialogo modal espera a un
//! humano. La comprobacion es manual (Task 12) y esta declarada, no
//! disimulada.

use std::path::PathBuf;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{FileSaveDialog, IFileSaveDialog, SIGDN_FILESYSPATH};
use windows::core::{HSTRING, PCWSTR, w};

/// Abre el dialogo y devuelve la ruta elegida, o None si se cancela o el
/// dialogo falla: perder el dialogo no puede costar la captura, que el
/// llamante conserva.
pub fn pedir_ruta_guardado(hwnd_padre: HWND, nombre_sugerido: &str) -> Option<PathBuf> {
    // SAFETY: COM ya esta inicializado en el hilo de interfaz (lo hace la
    // bandeja de S1-A); el dialogo es un objeto local que muere al salir.
    unsafe {
        let dialogo: IFileSaveDialog =
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let filtros = [
            COMDLG_FILTERSPEC {
                pszName: w!("PNG"),
                pszSpec: w!("*.png"),
            },
            COMDLG_FILTERSPEC {
                pszName: w!("JPEG"),
                pszSpec: w!("*.jpg;*.jpeg"),
            },
            COMDLG_FILTERSPEC {
                pszName: w!("WebP"),
                pszSpec: w!("*.webp"),
            },
        ];
        dialogo.SetFileTypes(&filtros).ok()?;
        dialogo.SetDefaultExtension(w!("png")).ok()?;
        let nombre = HSTRING::from(nombre_sugerido);
        dialogo.SetFileName(PCWSTR(nombre.as_ptr())).ok()?;
        // Show devuelve Err al cancelar: es el camino normal, no un fallo.
        dialogo.Show(Some(hwnd_padre)).ok()?;
        let item = dialogo.GetResult().ok()?;
        let ruta = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let texto = ruta.to_string().ok();
        // La memoria de GetDisplayName es COM (CoTaskMemAlloc) y se libera
        // AQUI, despues de copiar a String y ANTES de cualquier `?`: la fuga
        // de appdata() en S1-A fue exactamente un `?` entre la reserva y la
        // liberacion.
        CoTaskMemFree(Some(ruta.as_ptr() as *const _));
        texto.map(PathBuf::from)
    }
}
