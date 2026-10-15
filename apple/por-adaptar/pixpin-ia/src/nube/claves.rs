//! **Las claves de las API, cifradas con DPAPI.**
//!
//! Pedido del usuario: que solo se le pida la clave de cada proveedor. Una
//! clave es dinero: no puede quedar en claro en ningun sitio. Por eso:
//!
//! - Se cifra con `CryptProtectData` en el **ambito del usuario** de Windows
//!   (solo esta cuenta en este equipo la descifra; otra cuenta, otro PC o
//!   una copia del disco no), con una entropia propia de PixPin para que otro
//!   programa que use DPAPI sin ella no la saque por descuido.
//! - Vive en su **fichero propio** en `%LOCALAPPDATA%\PixPinMax\claves`:
//!   nunca en el TOML de ajustes (que el usuario abre, copia y comparte),
//!   nunca en la carpeta de datos que se sincroniza o se exporta, y
//!   `LOCALAPPDATA` no viaja con el perfil movil de Windows.
//! - Ni la clave ni un trozo de ella pasan por `tracing`: para ensenarla se
//!   usa [`enmascarar`], que deja ver solo el final.
//!
//! Este modulo habla con el sistema operativo: cada `unsafe` lleva su
//! `// SAFETY:`.

use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
use windows::core::w;

use super::Proveedor;

/// Mezclada con la clave al cifrar: sin ella no se descifra.
const ENTROPIA: &[u8] = b"PixPinMax/asistente/nube/v1";

/// `%LOCALAPPDATA%\PixPinMax\claves`.
pub fn carpeta() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(local).join("PixPinMax").join("claves"))
}

/// El fichero de la clave de un proveedor.
pub fn fichero(carpeta: &Path, p: Proveedor) -> PathBuf {
    carpeta.join(format!("{}.clave", p.id()))
}

/// Lo que se ensena de una clave: el principio fijo del proveedor y los
/// cuatro ultimos caracteres. `sk-ant-api03-…AbCd`.
pub fn enmascarar(clave: &str) -> String {
    let clave = clave.trim();
    let n = clave.chars().count();
    if n <= 8 {
        return "•".repeat(n.max(4));
    }
    let fin: String = clave.chars().skip(n - 4).collect();
    let prefijo: String = clave.chars().take_while(|c| *c != '-').take(6).collect();
    if clave.starts_with("sk-") {
        let cabeza: String = clave.chars().take(if clave.starts_with("sk-ant-") { 7 } else { 3 }).collect();
        format!("{cabeza}…{fin}")
    } else {
        format!("{prefijo}…{fin}")
    }
}

/// Si lo pegado tiene pinta de clave (sin espacios dentro, largo razonable).
/// No se comprueba el prefijo: los proveedores lo cambian.
pub fn parece_clave(clave: &str) -> bool {
    let c = clave.trim();
    (16..=400).contains(&c.len()) && c.chars().all(|ch| ch.is_ascii_graphic())
}

/// **Cifra** con DPAPI del usuario.
pub fn cifrar(claro: &[u8]) -> Result<Vec<u8>, String> {
    let entrada = blob(claro);
    let entropia = blob(ENTROPIA);
    let mut salida = CRYPT_INTEGER_BLOB::default();
    // SAFETY: los blobs de entrada apuntan a bufferes vivos durante la
    // llamada; la salida la reserva Windows con `LocalAlloc` y se libera
    // abajo con `LocalFree` tras copiarla.
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

/// **Descifra** lo cifrado por [`cifrar`] en esta misma cuenta.
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
    // SAFETY: DPAPI devolvio `cbData` bytes validos en `pbData`; se copian
    // y despues se borran y liberan una sola vez.
    unsafe {
        let v = std::slice::from_raw_parts(b.pbData, b.cbData as usize).to_vec();
        std::ptr::write_bytes(b.pbData, 0, b.cbData as usize);
        let _ = LocalFree(Some(HLOCAL(b.pbData.cast())));
        v
    }
}

/// **Guarda** la clave de un proveedor (cifrada). Una clave vacia la borra.
pub fn guardar_en(carpeta: &Path, p: Proveedor, clave: &str) -> Result<(), String> {
    let clave = clave.trim();
    let ruta = fichero(carpeta, p);
    if clave.is_empty() {
        return borrar_en(carpeta, p);
    }
    let cifrado = cifrar(clave.as_bytes())?;
    std::fs::create_dir_all(carpeta).map_err(|e| format!("no se pudo crear la carpeta de claves: {e}"))?;
    // Primero a un fichero aparte y luego se cambia de nombre: un corte a
    // medias no deja una clave rota en lugar de la buena.
    let tmp = ruta.with_extension("clave.parte");
    std::fs::write(&tmp, &cifrado).map_err(|e| format!("no se pudo guardar la clave: {e}"))?;
    std::fs::rename(&tmp, &ruta).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("no se pudo guardar la clave: {e}")
    })
}

/// **Lee** la clave guardada. `None` si no hay o si no se puede descifrar
/// (otra cuenta, fichero danado): se pide otra vez.
pub fn leer_de(carpeta: &Path, p: Proveedor) -> Option<String> {
    let cifrado = std::fs::read(fichero(carpeta, p)).ok()?;
    let claro = descifrar(&cifrado).ok()?;
    String::from_utf8(claro).ok().filter(|c| !c.trim().is_empty())
}

pub fn borrar_en(carpeta: &Path, p: Proveedor) -> Result<(), String> {
    match std::fs::remove_file(fichero(carpeta, p)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("no se pudo borrar la clave: {e}")),
    }
}

/// Atajos sobre la carpeta de siempre.
pub fn guardar(p: Proveedor, clave: &str) -> Result<(), String> {
    guardar_en(&carpeta().ok_or("no se encuentra LOCALAPPDATA")?, p, clave)
}

pub fn leer(p: Proveedor) -> Option<String> {
    leer_de(&carpeta()?, p)
}

pub fn hay(p: Proveedor) -> bool {
    carpeta().is_some_and(|c| fichero(&c, p).is_file())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn carpeta_de_prueba(nombre: &str) -> PathBuf {
        let c = std::env::temp_dir().join(format!("pixpin-claves-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&c);
        c
    }

    const CLAVE: &str = "sk-ant-api03-PRUEBAxyzzy0123456789abcdefghijklmnopqrstuvwxyzQWERTY";

    #[test]
    fn se_guarda_cifrada_y_se_lee_igual() {
        let c = carpeta_de_prueba("ida");
        guardar_en(&c, Proveedor::Claude, CLAVE).unwrap();
        assert_eq!(leer_de(&c, Proveedor::Claude).as_deref(), Some(CLAVE));
        // La de otro proveedor no existe.
        assert!(leer_de(&c, Proveedor::DeepSeek).is_none());
        let _ = std::fs::remove_dir_all(&c);
    }

    #[test]
    fn la_clave_no_aparece_en_claro_en_ningun_fichero() {
        let c = carpeta_de_prueba("claro");
        guardar_en(&c, Proveedor::Claude, CLAVE).unwrap();
        guardar_en(&c, Proveedor::DeepSeek, "clave-de-prueba-deepseek-0123456789abcdef").unwrap();
        let mut ficheros = 0;
        for e in std::fs::read_dir(&c).unwrap() {
            let bytes = std::fs::read(e.unwrap().path()).unwrap();
            ficheros += 1;
            for buscada in [CLAVE.as_bytes(), b"PRUEBAxyzzy", b"0123456789abcdef"] {
                assert!(!bytes.windows(buscada.len()).any(|w| w == buscada), "clave en claro en el disco");
            }
            // Ni en UTF-16.
            let ancha: Vec<u8> = "PRUEBAxyzzy".encode_utf16().flat_map(u16::to_le_bytes).collect();
            assert!(!bytes.windows(ancha.len()).any(|w| w == ancha.as_slice()));
        }
        assert_eq!(ficheros, 2, "sin ficheros temporales olvidados");
        let _ = std::fs::remove_dir_all(&c);
    }

    #[test]
    fn un_fichero_danado_no_da_clave_y_vacia_borra() {
        let c = carpeta_de_prueba("danado");
        std::fs::create_dir_all(&c).unwrap();
        std::fs::write(fichero(&c, Proveedor::DeepSeek), b"esto no es dpapi").unwrap();
        assert!(leer_de(&c, Proveedor::DeepSeek).is_none());
        guardar_en(&c, Proveedor::DeepSeek, "   ").unwrap();
        assert!(!fichero(&c, Proveedor::DeepSeek).exists());
        // Borrar lo que no esta no es un error.
        borrar_en(&c, Proveedor::DeepSeek).unwrap();
        let _ = std::fs::remove_dir_all(&c);
    }

    #[test]
    fn sin_la_entropia_de_pixpin_no_se_descifra() {
        let cifrado = cifrar(b"secreto").unwrap();
        let entrada = blob(&cifrado);
        let mut salida = CRYPT_INTEGER_BLOB::default();
        // SAFETY: igual que en `descifrar`, pero sin entropia.
        let r = unsafe {
            CryptUnprotectData(&entrada, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut salida)
        };
        assert!(r.is_err());
        assert_eq!(descifrar(&cifrado).unwrap(), b"secreto");
    }

    #[test]
    fn enmascarar_deja_ver_solo_el_final() {
        let m = enmascarar(CLAVE);
        assert_eq!(m, "sk-ant-…RTY".replace("RTY", "ERTY"));
        assert!(!m.contains("PRUEBA"));
        assert_eq!(enmascarar("sk-0123456789abcdef"), "sk-…cdef");
        assert_eq!(enmascarar("abc"), "••••");
        assert!(parece_clave(CLAVE));
        assert!(!parece_clave("hola mundo con espacios dentro"));
        assert!(!parece_clave("corta"));
    }
}
