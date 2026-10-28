//! **La clave de TypeSafe, cifrada con DPAPI**, igual que las de Claude y
//! DeepSeek (`pixpin_ia::nube::claves`):
//!
//! - cifrada con `CryptProtectData` en el ambito del usuario de Windows y una
//!   entropia propia, en `%LOCALAPPDATA%\PixPinMax\claves\typesafe.clave`;
//! - **nunca** en el TOML de ajustes ni en la carpeta de datos que se
//!   sincroniza o se exporta (`LOCALAPPDATA` tampoco viaja con el perfil);
//! - ni la clave ni un trozo pasan por `tracing`: se ensena con
//!   [`enmascarar`].
//!
//! Fuera de Windows no se guarda nada: solo se lee `TYPESAFE_API_KEY` (la de
//! las pruebas de verdad).
//!
//! Este modulo habla con el sistema operativo: cada `unsafe` lleva su
//! `// SAFETY:`.

use std::path::{Path, PathBuf};

/// El nombre del fichero de la clave.
pub const FICHERO: &str = "typesafe.clave";

/// `%LOCALAPPDATA%\PixPinMax\claves` (la de las demas claves).
pub fn carpeta() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(local).join("PixPinMax").join("claves"))
}

/// Lo que se ensena: el principio y los cuatro ultimos caracteres.
pub fn enmascarar(clave: &str) -> String {
    let c = clave.trim();
    let n = c.chars().count();
    if n <= 8 {
        return "•".repeat(n.max(4));
    }
    let cabeza: String = c.chars().take(3).collect();
    let fin: String = c.chars().skip(n - 4).collect();
    format!("{cabeza}…{fin}")
}

/// Si lo pegado tiene pinta de clave: sin espacios, de largo razonable.
pub fn parece_clave(clave: &str) -> bool {
    let c = clave.trim();
    (16..=400).contains(&c.len()) && c.chars().all(|ch| ch.is_ascii_graphic())
}

#[cfg(windows)]
mod dpapi {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
    };
    use windows::core::w;

    /// Mezclada con la clave al cifrar: sin ella no se descifra.
    const ENTROPIA: &[u8] = b"PixPinMax/typesafe/v1";

    fn blob(b: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: b.len() as u32,
            pbData: b.as_ptr() as *mut u8,
        }
    }

    /// # Safety
    /// `b` lo tiene que haber llenado DPAPI (memoria de `LocalAlloc`).
    unsafe fn copiar_y_soltar(b: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        if b.pbData.is_null() {
            return Vec::new();
        }
        // SAFETY: DPAPI devolvio `cbData` bytes validos en `pbData`; se
        // copian y despues se borran y liberan una sola vez.
        unsafe {
            let v = std::slice::from_raw_parts(b.pbData, b.cbData as usize).to_vec();
            std::ptr::write_bytes(b.pbData, 0, b.cbData as usize);
            let _ = LocalFree(Some(HLOCAL(b.pbData.cast())));
            v
        }
    }

    pub fn cifrar(claro: &[u8]) -> Result<Vec<u8>, String> {
        let entrada = blob(claro);
        let entropia = blob(ENTROPIA);
        let mut salida = CRYPT_INTEGER_BLOB::default();
        // SAFETY: los blobs de entrada apuntan a bufferes vivos durante la
        // llamada; la salida la reserva Windows y se libera tras copiarla.
        unsafe {
            CryptProtectData(
                &entrada,
                w!("PixPin Max"),
                Some(&entropia),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut salida,
            )
            .map_err(|e| format!("no se pudo cifrar la clave: {}", e.message()))?;
            Ok(copiar_y_soltar(salida))
        }
    }

    pub fn descifrar(cifrado: &[u8]) -> Result<Vec<u8>, String> {
        let entrada = blob(cifrado);
        let entropia = blob(ENTROPIA);
        let mut salida = CRYPT_INTEGER_BLOB::default();
        // SAFETY: como en `cifrar`.
        unsafe {
            CryptUnprotectData(
                &entrada,
                None,
                Some(&entropia),
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut salida,
            )
            .map_err(|e| format!("no se pudo descifrar la clave: {}", e.message()))?;
            Ok(copiar_y_soltar(salida))
        }
    }
}

/// **Guarda** la clave (cifrada) en `carpeta`. Vacia, la borra.
pub fn guardar_en(carpeta: &Path, clave: &str) -> Result<(), String> {
    let clave = clave.trim();
    if clave.is_empty() {
        return borrar_en(carpeta);
    }
    #[cfg(windows)]
    {
        let ruta = carpeta.join(FICHERO);
        let cifrado = dpapi::cifrar(clave.as_bytes())?;
        std::fs::create_dir_all(carpeta)
            .map_err(|e| format!("no se pudo crear la carpeta de claves: {e}"))?;
        // A un fichero aparte y luego se cambia el nombre: un corte a medias
        // no deja una clave rota en lugar de la buena.
        let tmp = ruta.with_extension("clave.parte");
        std::fs::write(&tmp, &cifrado).map_err(|e| format!("no se pudo guardar la clave: {e}"))?;
        std::fs::rename(&tmp, &ruta).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("no se pudo guardar la clave: {e}")
        })
    }
    #[cfg(not(windows))]
    {
        let _ = carpeta;
        Err("en este sistema la clave de TypeSafe no se guarda: usa TYPESAFE_API_KEY".into())
    }
}

/// **Lee** la clave de `carpeta`. `None` si no hay o no se descifra.
pub fn leer_de(carpeta: &Path) -> Option<String> {
    #[cfg(windows)]
    {
        let cifrado = std::fs::read(carpeta.join(FICHERO)).ok()?;
        let claro = dpapi::descifrar(&cifrado).ok()?;
        String::from_utf8(claro)
            .ok()
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
    }
    #[cfg(not(windows))]
    {
        let _ = carpeta;
        None
    }
}

pub fn borrar_en(carpeta: &Path) -> Result<(), String> {
    match std::fs::remove_file(carpeta.join(FICHERO)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("no se pudo borrar la clave: {e}")),
    }
}

/// La clave de siempre: la guardada en Ajustes y, si no hay, la de
/// `TYPESAFE_API_KEY` (las pruebas de verdad, otros sistemas).
pub fn leer() -> Option<String> {
    carpeta().and_then(|c| leer_de(&c)).or_else(|| {
        std::env::var("TYPESAFE_API_KEY")
            .ok()
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
    })
}

pub fn guardar(clave: &str) -> Result<(), String> {
    guardar_en(&carpeta().ok_or("no se encuentra LOCALAPPDATA")?, clave)
}

pub fn borrar() -> Result<(), String> {
    match carpeta() {
        Some(c) => borrar_en(&c),
        None => Ok(()),
    }
}

/// Si hay una clave guardada en Ajustes (sin descifrarla).
pub fn hay() -> bool {
    carpeta().is_some_and(|c| c.join(FICHERO).is_file())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn enmascarar_deja_ver_solo_el_final() {
        let m = enmascarar("ts_live_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123");
        assert_eq!(m, "ts_…0123");
        assert!(!m.contains("ABCDEF"));
        assert_eq!(enmascarar("abc"), "••••");
        assert!(parece_clave("ts_live_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123"));
        assert!(!parece_clave("con espacios dentro de la clave"));
        assert!(!parece_clave("corta"));
    }

    #[test]
    fn borrar_lo_que_no_esta_no_es_un_error() {
        let c = std::env::temp_dir().join(format!("pixpin-ts-claves-{}", std::process::id()));
        borrar_en(&c).unwrap();
        assert!(leer_de(&c).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn se_guarda_cifrada_y_se_lee_igual() {
        let c = std::env::temp_dir().join(format!("pixpin-ts-claves-w-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&c);
        let clave = "ts_live_PRUEBAxyzzy0123456789abcdef";
        guardar_en(&c, clave).unwrap();
        assert_eq!(leer_de(&c).as_deref(), Some(clave));
        let bytes = std::fs::read(c.join(FICHERO)).unwrap();
        assert!(!bytes.windows(11).any(|w| w == b"PRUEBAxyzzy"));
        guardar_en(&c, "  ").unwrap();
        assert!(!c.join(FICHERO).exists());
        let _ = std::fs::remove_dir_all(&c);
    }
}
