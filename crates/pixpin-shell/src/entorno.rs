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
    // si no. Tras liberar no queda ninguna referencia viva al puntero. Solo
    // estas tres llamadas son realmente inseguras; decidir si propagar el
    // error de la conversion es logica normal y vive fuera del bloque.
    let convertida = unsafe {
        let ruta: PWSTR = SHGetKnownFolderPath(&FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, None)
            .map_err(|e| std::io::Error::other(format!("SHGetKnownFolderPath fallo: {e}")))?;
        let convertida = ruta.to_string();
        CoTaskMemFree(Some(ruta.0 as *const _));
        convertida
    };
    let texto = convertida.map_err(std::io::Error::other)?;
    Ok(PathBuf::from(texto))
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

/// Donde esta el puntero, en pixeles fisicos del escritorio virtual.
///
/// Decide en que monitor nace un pin del portapapeles: donde estan los ojos
/// del usuario, no en el principal por defecto.
pub fn posicion_del_cursor() -> pixpin_geom::Punto {
    let mut p = windows::Win32::Foundation::POINT::default();
    // SAFETY: escribe en una variable local propia; sin precondiciones.
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut p);
    }
    pixpin_geom::Punto { x: p.x, y: p.y }
}

/// `true` si Windows esta en tema claro para las aplicaciones.
///
/// Decide el lienzo de las notas y las fichas (D30). Si la clave no existe
/// —Windows anteriores a la opcion, o una politica que la borro— se asume
/// claro, que es el aspecto por defecto del sistema.
pub fn tema_claro() -> bool {
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    use windows::core::w;

    let mut valor: u32 = 1;
    let mut tam = size_of::<u32>() as u32;
    // SAFETY: rutas constantes terminadas en cero; se pide un DWORD y se
    // entrega un u32 propio con su tamano exacto. RegGetValueW no guarda
    // ninguno de los dos punteros.
    let estado = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut valor as *mut u32 as *mut _),
            Some(&mut tam),
        )
    };
    if estado != ERROR_SUCCESS {
        return true;
    }
    valor != 0
}

/// La hora local ahora mismo, en milisegundos desde 1970 **ya corridos al
/// huso del usuario**. Asi lo de arriba puede comparar dias sin saber nada
/// de husos: dos instantes caen el mismo dia si dividen igual entre 86400000.
pub fn ahora_local_ms() -> i64 {
    use windows::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows::Win32::System::SystemInformation::GetLocalTime;
    use windows::Win32::System::Time::SystemTimeToFileTime;

    // SAFETY: `GetLocalTime` solo escribe el SYSTEMTIME que devuelve, y
    // `SystemTimeToFileTime` lee ese y escribe el FILETIME que se le pasa.
    let ft = unsafe {
        let t: SYSTEMTIME = GetLocalTime();
        let mut ft = FILETIME::default();
        if SystemTimeToFileTime(&t, &mut ft).is_err() {
            return 0;
        }
        ft
    };
    // FILETIME cuenta de cien en cien nanosegundos desde 1601.
    const A_1970: i64 = 116_444_736_000_000_000;
    let cien_ns = ((ft.dwHighDateTime as i64) << 32) | ft.dwLowDateTime as i64;
    (cien_ns - A_1970) / 10_000
}

/// Ahora, en milisegundos desde 1970 en UTC: la hora que se GUARDA.
///
/// Es la de `System.currentTimeMillis()` del movil. Lo que se guarda con la
/// hora local viaja al movil corrido por el huso (unas cinco horas en Peru),
/// se coloca en el chat donde no toca y, al decidir si un proyecto borrado
/// alli se toco aqui despues, puede parecer anterior de lo que fue.
pub fn ahora_utc_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// Cuanto hay que sumarle a una hora UTC para pintarla en la del usuario.
///
/// Se redondea al cuarto de hora: los husos van de cuarto en cuarto y asi
/// los pocos milisegundos entre las dos lecturas del reloj no se cuelan.
/// Se recalcula a cada llamada porque es barato y el cambio de horario de
/// verano tiene que notarse sin reiniciar la aplicacion.
pub fn desfase_local_ms() -> i64 {
    const CUARTO: i64 = 15 * 60_000;
    let d = ahora_local_ms() - ahora_utc_ms();
    (d as f64 / CUARTO as f64).round() as i64 * CUARTO
}

/// Una hora guardada (UTC) vista en la hora del usuario, para pintarla.
pub fn a_local(utc_ms: i64) -> i64 {
    utc_ms + desfase_local_ms()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_desfase_del_huso_va_en_cuartos_de_hora_y_a_local_lo_suma() {
        let d = desfase_local_ms();
        assert_eq!(d % (15 * 60_000), 0, "desfase {d}");
        assert!(d.abs() <= 14 * 3_600_000, "ningun huso pasa de 14 h");
        assert_eq!(a_local(1_000), 1_000 + d);
        // Caso negativo: la hora UTC no es la local corrida, salvo en UTC.
        let (utc, local) = (ahora_utc_ms(), ahora_local_ms());
        assert!((local - utc - d).abs() < 60_000);
    }

    #[test]
    fn el_tema_se_responde_sin_reventar() {
        // No se puede afirmar claro u oscuro (depende del equipo), pero si
        // que la consulta al registro no entra en panico ni cuelga, que es
        // lo unico que puede romper el arranque de un pin.
        let _ = tema_claro();
    }

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
