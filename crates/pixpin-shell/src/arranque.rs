//! Arranque con Windows.
//!
//! **En modo portable esto no se toca nunca.** La promesa del modo portable es
//! cero rastro en el equipo, y una entrada en `HKCU\...\Run` la romperia. Ademas
//! apuntaria a una ruta de USB que mañana puede no estar, dejando un error de
//! arranque al usuario cada vez que enciende el ordenador.

use std::path::Path;

use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ, RegCloseKey, RegDeleteValueW,
    RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::{HSTRING, w};

const VALOR: windows::core::PCWSTR = w!("PixPinMax");

#[derive(Debug, thiserror::Error)]
pub enum ErrorArranque {
    #[error("en modo portable no se escribe en el registro, por diseño")]
    ModoPortable,
    #[error("error del registro de Windows: {0}")]
    Registro(#[from] windows::core::Error),
}

/// Si se puede ofrecer la opcion al usuario.
pub fn permitido(es_portable: bool) -> bool {
    !es_portable
}

fn abrir(acceso: windows::Win32::System::Registry::REG_SAM_FLAGS) -> windows::core::Result<HKEY> {
    let mut clave = HKEY::default();
    // SAFETY: la ruta es un literal estatico terminado en cero y `clave` es una
    // variable propia que se rellena en caso de exito.
    unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Run"),
            Some(0),
            acceso,
            &mut clave,
        )
        .ok()?;
    }
    Ok(clave)
}

/// Activa o desactiva el arranque automatico.
pub fn establecer(activo: bool, es_portable: bool, ruta_exe: &Path) -> Result<(), ErrorArranque> {
    if es_portable {
        // Desactivar es una operacion vacia, no un fallo: asi quien aplica los
        // ajustes no necesita ramificar por modo.
        return if activo {
            Err(ErrorArranque::ModoPortable)
        } else {
            Ok(())
        };
    }

    let clave = abrir(KEY_WRITE)?;

    let resultado = if activo {
        // Las comillas son obligatorias: sin ellas una ruta con espacios
        // (C:\Program Files\...) se interpreta como varios argumentos.
        let linea = HSTRING::from(format!("\"{}\"", ruta_exe.display()));
        // SAFETY: `linea.as_ptr()` apunta al buffer UTF-16 propio de `linea`,
        // que sigue vivo hasta el final de este bloque `if`. `linea.len()`
        // cuenta las unidades UTF-16 de contenido (sin el cero final) y
        // `HSTRING` garantiza un cero adicional justo despues, asi que
        // `(linea.len() + 1) * size_of::<u16>()` bytes caen exactamente
        // dentro del buffer inicializado. El cast de `*const u16` a
        // `*const u8` solo relaja la alineacion (de 2 a 1 byte), lo cual es
        // siempre valido.
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                linea.as_ptr() as *const u8,
                (linea.len() + 1) * size_of::<u16>(),
            )
        };
        // SAFETY: `clave` es un handle valido abierto justo arriba con
        // acceso de escritura, `VALOR` es un literal UTF-16 terminado en
        // cero, y `bytes` describe un buffer REG_SZ valido: el contenido
        // UTF-16 de `linea` mas su cero final, como exige REG_SZ.
        unsafe { RegSetValueExW(clave, VALOR, Some(0), REG_SZ, Some(bytes)).ok() }
    } else {
        // SAFETY: `clave` es un handle valido abierto justo arriba.
        unsafe { RegDeleteValueW(clave, VALOR).ok() }
    };

    // SAFETY: `clave` viene de RegOpenKeyExW y no se ha cerrado antes.
    unsafe {
        let _ = RegCloseKey(clave);
    }

    // Borrar un valor que no existe no es un fallo desde el punto de vista del
    // usuario: el resultado que pedia (que no arranque solo) ya se cumple.
    match resultado {
        Ok(()) => Ok(()),
        Err(_) if !activo => Ok(()),
        Err(e) => Err(ErrorArranque::Registro(e)),
    }
}

/// Si ahora mismo hay una entrada de arranque automatico.
pub fn esta_activo() -> bool {
    let Ok(clave) = abrir(KEY_READ) else {
        return false;
    };
    // SAFETY: `clave` es valida; se piden solo los metadatos del valor pasando
    // None como buffer, que es el uso documentado para comprobar existencia.
    let existe = unsafe { RegQueryValueExW(clave, VALOR, None, None, None, None).is_ok() };
    // SAFETY: `clave` viene de RegOpenKeyExW y no se ha cerrado antes.
    unsafe {
        let _ = RegCloseKey(clave);
    }
    existe
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::path::Path;

    #[test]
    fn el_modo_portable_no_permite_tocar_el_registro() {
        // La promesa del modo portable es "cero rastro en el equipo". Escribir
        // en HKCU\...\Run la romperia, y ademas dejaria una entrada apuntando
        // a un USB que manaña no esta.
        assert!(!permitido(true));
        assert!(permitido(false));
    }

    #[test]
    fn establecer_en_modo_portable_da_error_claro() {
        let e = establecer(true, true, Path::new(r"C:\x\pixpinmax.exe")).unwrap_err();
        assert!(matches!(e, ErrorArranque::ModoPortable));
    }

    #[test]
    fn desactivar_en_modo_portable_no_es_error() {
        // Desactivar algo que no puede estar activo es una operacion vacia,
        // no un fallo: asi el codigo que aplica ajustes no necesita ramas.
        assert!(establecer(false, true, Path::new(r"C:\x\pixpinmax.exe")).is_ok());
    }

    #[test]
    fn en_modo_instalado_activar_y_desactivar_es_reversible() {
        let exe = Path::new(r"C:\NoExiste\pixpinmax.exe");

        establecer(true, false, exe).unwrap();
        assert!(esta_activo(), "tras activar deberia estar activo");

        establecer(false, false, exe).unwrap();
        assert!(!esta_activo(), "tras desactivar no deberia estar activo");
    }
}
