//! Lo que la sincronizacion le pregunta al sistema de un fichero o de un
//! disco (27-oct-2026, «sincronizar por diario»).
//!
//! - [`nodo_de`]: **quien es** un fichero, no solo como se llama. Dos
//!   escrituras con el mismo tamano en el mismo tic del reloj dejan la misma
//!   fecha; si el fichero se sustituyo por otro (escribir a un temporal y
//!   renombrar encima), el nodo cambia. En Unix es el inodo con su
//!   dispositivo, como `fileKey()` de Java en Android; en Windows, el numero
//!   de volumen y el **indice de archivo de NTFS** (`nFileIndex`), que es lo
//!   que la guia de Android pide en su lugar.
//! - [`espacio_libre`]: cuanto cabe todavia en el disco de una carpeta, para
//!   no empezar a recibir lo que no cabe.
//!
//! Los dos devuelven `None` si el sistema no lo sabe decir: quien pregunta
//! sigue sin el (un nodo vacio, o intentarlo sin saber el sitio).

use std::path::Path;

/// El identificador estable de `ruta` en su disco, como texto, o `None` si no
/// existe o no se puede abrir.
#[cfg(windows)]
pub fn nodo_de(ruta: &Path) -> Option<String> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let f = std::fs::File::open(ruta).ok()?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: el handle es de `f`, que vive hasta el final de la funcion; la
    // estructura es nuestra y del tamano que espera la llamada, que no
    // retiene ningun puntero.
    let hecho = unsafe { GetFileInformationByHandle(HANDLE(f.as_raw_handle()), &mut info) };
    hecho.ok()?;
    let indice = (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow);
    Some(format!(
        "(vol={:x},idx={indice:x})",
        info.dwVolumeSerialNumber
    ))
}

/// En Unix (Mac, iPhone/iPad, Linux): el dispositivo y el inodo, como los
/// escribe Java en Android (`(dev=fd01,ino=3141)`).
#[cfg(unix)]
pub fn nodo_de(ruta: &Path) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    let m = std::fs::metadata(ruta).ok()?;
    Some(format!("(dev={:x},ino={})", m.dev(), m.ino()))
}

#[cfg(not(any(windows, unix)))]
pub fn nodo_de(_ruta: &Path) -> Option<String> {
    None
}

/// Los bytes que todavia caben en el disco de `carpeta` (para este usuario),
/// o `None` si no se sabe.
#[cfg(windows)]
pub fn espacio_libre(carpeta: &Path) -> Option<u64> {
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    use windows::core::HSTRING;

    let texto = HSTRING::from(carpeta.as_os_str());
    let mut libres = 0u64;
    // SAFETY: la cadena y el entero viven durante la llamada, que no los
    // retiene; los otros dos punteros van vacios, como permite la API.
    let hecho = unsafe { GetDiskFreeSpaceExW(&texto, Some(&mut libres), None, None) };
    hecho.ok()?;
    Some(libres)
}

#[cfg(unix)]
pub fn espacio_libre(carpeta: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(carpeta.as_os_str().as_bytes()).ok()?;
    let mut s: libc::statvfs = statvfs_a_cero();
    // SAFETY: `c` es una cadena C valida que vive durante la llamada y `s` es
    // una estructura nuestra del tipo que espera `statvfs`, que no retiene
    // ningun puntero.
    let r = unsafe { libc::statvfs(c.as_ptr(), &mut s) };
    if r != 0 {
        return None;
    }
    #[allow(clippy::unnecessary_cast)]
    Some((s.f_bavail as u64).saturating_mul(s.f_frsize as u64))
}

/// Una `statvfs` a cero, que es un valor valido de esa estructura (solo
/// enteros).
#[cfg(unix)]
fn statvfs_a_cero() -> libc::statvfs {
    // SAFETY: `statvfs` es una estructura de C solo con enteros: todos sus
    // bits a cero son un valor valido.
    unsafe { std::mem::zeroed() }
}

#[cfg(not(any(windows, unix)))]
pub fn espacio_libre(_carpeta: &Path) -> Option<u64> {
    None
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_nodo_sigue_al_fichero_y_cambia_si_se_sustituye() {
        let dir = std::env::temp_dir().join(format!("pixpin-nodo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.txt");
        std::fs::write(&a, b"uno").unwrap();
        let antes = nodo_de(&a).expect("un fichero que existe tiene nodo");
        std::fs::write(&a, b"dos").unwrap();
        assert_eq!(
            nodo_de(&a).as_deref(),
            Some(antes.as_str()),
            "reescrito en su sitio"
        );
        // Escrito a un temporal y renombrado encima: es otro fichero.
        let tmp = dir.join("a.txt.tmp");
        std::fs::write(&tmp, b"tres").unwrap();
        let del_tmp = nodo_de(&tmp).unwrap();
        std::fs::rename(&tmp, &a).unwrap();
        assert_eq!(nodo_de(&a).as_deref(), Some(del_tmp.as_str()));
        assert_ne!(del_tmp, antes);
        assert!(nodo_de(&dir.join("no-esta")).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_espacio_libre_de_la_carpeta_temporal_se_sabe() {
        let libre = espacio_libre(&std::env::temp_dir());
        assert!(libre.is_some_and(|b| b > 0), "{libre:?}");
    }
}
