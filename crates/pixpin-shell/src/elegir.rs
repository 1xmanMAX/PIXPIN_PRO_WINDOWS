//! El dialogo "Abrir" del sistema, con seleccion multiple.
//!
//! Es lo que en PixPin Android hace el SAF al adjuntar: elegir ficheros para
//! copiarlos dentro de la aplicacion. Aqui la copia la hace quien llama;
//! esto solo devuelve las rutas.
//!
//! Va aparte de `abrir`, que es lo contrario: aquel abre un fichero CON el
//! sistema, este se lo pide AL sistema.
//!
//! Sin test automatico, como el de guardar: un dialogo modal espera a un
//! humano. Se declara en vez de disimularlo.

use std::path::PathBuf;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::Win32::UI::Shell::{
    FOS_ALLOWMULTISELECT, FOS_FILEMUSTEXIST, FileOpenDialog, IFileOpenDialog, SIGDN_FILESYSPATH,
};

/// Pide ficheros al usuario. Lista vacia si cancela o si el dialogo falla:
/// no poder abrirlo no puede tumbar la ventana que lo pidio.
pub fn pedir_ficheros(hwnd_padre: HWND) -> Vec<PathBuf> {
    // SAFETY: COM ya esta inicializado en el hilo de interfaz (lo hace la
    // bandeja); el dialogo es un objeto local que muere al salir de aqui, y
    // cada cadena que devuelve se libera con CoTaskMemFree.
    unsafe {
        let Ok(dialogo) =
            CoCreateInstance::<_, IFileOpenDialog>(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
        else {
            return Vec::new();
        };
        // Varios a la vez, y solo ficheros que existan: adjuntar algo que no
        // esta no tiene sentido.
        let opciones = dialogo.GetOptions().unwrap_or_default();
        let _ = dialogo.SetOptions(opciones | FOS_ALLOWMULTISELECT | FOS_FILEMUSTEXIST);
        if dialogo.Show(Some(hwnd_padre)).is_err() {
            // Cancelar llega como error; no es un fallo que anotar.
            return Vec::new();
        }
        let Ok(elegidos) = dialogo.GetResults() else {
            return Vec::new();
        };
        let cuantos = elegidos.GetCount().unwrap_or(0);
        let mut rutas = Vec::with_capacity(cuantos as usize);
        for i in 0..cuantos {
            // Uno que falle no puede llevarse los demas que se eligieron.
            let Ok(elemento) = elegidos.GetItemAt(i) else {
                continue;
            };
            let Ok(ancha) = elemento.GetDisplayName(SIGDN_FILESYSPATH) else {
                continue;
            };
            if ancha.is_null() {
                continue;
            }
            if let Ok(texto) = ancha.to_string() {
                rutas.push(PathBuf::from(texto));
            }
            CoTaskMemFree(Some(ancha.0 as *const _));
        }
        rutas
    }
}
