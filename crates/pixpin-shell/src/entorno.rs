//! Lo que hay que preguntarle a Windows antes de arrancar.
//!
//! Este modulo existe para que los crates con `forbid(unsafe_code)` no tengan
//! que llamar a Win32: reciben estos valores ya resueltos como parametros.

use std::path::PathBuf;

use windows::Win32::Globalization::GetUserDefaultLocaleName;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath};
use windows::core::PWSTR;

/// Directorio donde vive `pixpinmax.exe`.
pub fn directorio_del_ejecutable() -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    exe.parent()
        .map(PathBuf::from)
        .ok_or_else(|| std::io::Error::other("el ejecutable no tiene directorio padre"))
}

/// `%APPDATA%` (la carpeta itinerante del usuario).
///
/// Se usa `SHGetKnownFolderPath` y no la variable de entorno porque la
/// variable se puede manipular y no siempre esta presente en sesiones de
/// servicio.
pub fn appdata() -> std::io::Result<PathBuf> {
    // SAFETY: SHGetKnownFolderPath devuelve un puntero a cadena UTF-16
    // terminada en cero que hay que liberar con CoTaskMemFree exactamente una
    // vez. Por eso la conversion a String se hace primero y se guarda en una
    // variable local (sin propagar el error todavia): CoTaskMemFree se llama
    // despues, en todos los caminos, tanto si la conversion salio bien como
    // si no. Solo entonces se decide si propagar el error de la conversion.
    // Tras liberar no queda ninguna referencia viva al puntero.
    unsafe {
        let ruta: PWSTR = SHGetKnownFolderPath(&FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, None)
            .map_err(|e| std::io::Error::other(format!("SHGetKnownFolderPath fallo: {e}")))?;
        let convertida = ruta.to_string();
        CoTaskMemFree(Some(ruta.0 as *const _));
        let texto = convertida.map_err(std::io::Error::other)?;
        Ok(PathBuf::from(texto))
    }
}

/// Etiqueta de idioma del usuario, por ejemplo `es-ES`.
///
/// Si Windows no la devuelve se asume `en-US`: es preferible una interfaz en
/// ingles a no arrancar.
pub fn locale_del_sistema() -> String {
    const MAX: usize = 85; // LOCALE_NAME_MAX_LENGTH
    let mut buffer = [0u16; MAX];

    // SAFETY: se pasa un buffer propio de tamaño conocido y la funcion
    // devuelve cuantos u16 escribio, incluido el cero final.
    let escritos = unsafe { GetUserDefaultLocaleName(&mut buffer) };

    if escritos <= 1 {
        return "en-US".to_string();
    }
    String::from_utf16_lossy(&buffer[..(escritos as usize - 1)])
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_directorio_del_ejecutable_existe() {
        let dir = directorio_del_ejecutable().unwrap();
        assert!(dir.is_dir(), "{dir:?} deberia ser un directorio");
    }

    #[test]
    fn appdata_existe() {
        let dir = appdata().unwrap();
        assert!(dir.is_dir(), "{dir:?} deberia ser un directorio");
    }

    #[test]
    fn el_locale_tiene_forma_de_etiqueta_de_idioma() {
        let l = locale_del_sistema();
        assert!(!l.is_empty(), "el locale no puede venir vacio");
        assert!(
            l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "locale con forma rara: {l}"
        );
    }
}
