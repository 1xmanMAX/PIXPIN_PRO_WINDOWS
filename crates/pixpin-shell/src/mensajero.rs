//! Pasarle ficheros a la copia que ya esta corriendo.
//!
//! PixPin Max solo deja una instancia. Sin esto, abrir una imagen con
//! «Abrir con» no haria nada: la segunda copia veria el mutex tomado y se
//! iria en silencio, llevandose la ruta que le habian dado. El usuario
//! veria que no pasa NADA al abrir su fichero, que es lo peor que puede
//! hacer un programa.
//!
//! Asi que la segunda copia, antes de irse, busca la ventana de mensajes de
//! la primera y le manda las rutas con `WM_COPYDATA`.
//!
//! `WM_COPYDATA` y no un socket ni una tuberia: es lo unico que Windows
//! copia entre procesos por su cuenta, sin permisos, sin nombres que
//! chocar y sin nada que limpiar si el otro lado se cae.

use std::path::PathBuf;

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SendMessageW};
use windows::core::w;

/// El identificador de nuestro mensaje dentro de `WM_COPYDATA`.
///
/// Lo lleva `dwData`, y sirve para que otro programa que le mande
/// `WM_COPYDATA` a nuestra ventana por error no acabe pineando cosas.
pub const ABRIR_FICHEROS: usize = 0x5049_5850;

/// Las rutas, empaquetadas como texto UTF-16 separado por nulos.
///
/// Se separan por nulo y no por salto de linea porque un nombre de fichero
/// en Windows puede contener casi cualquier cosa MENOS un nulo: es el unico
/// separador que no puede aparecer dentro de una ruta y partirla en dos.
fn empaquetar(rutas: &[PathBuf]) -> Vec<u16> {
    let mut fuera = Vec::new();
    for ruta in rutas {
        fuera.extend(ruta.as_os_str().encode_wide());
        fuera.push(0);
    }
    fuera
}

/// Deshace lo que hizo `empaquetar`.
///
/// Aparte y pura para poder probarla: el desempaquetado es donde se cuelan
/// los fallos de uno de mas o de menos, y no necesita dos procesos para
/// comprobarse.
pub fn desempaquetar(unidades: &[u16]) -> Vec<PathBuf> {
    unidades
        .split(|u| *u == 0)
        .filter(|trozo| !trozo.is_empty())
        .map(|trozo| PathBuf::from(String::from_utf16_lossy(trozo)))
        .collect()
}

use std::os::windows::ffi::OsStrExt;

/// Manda las rutas a la instancia que ya corre. Devuelve si llegaron.
///
/// `false` significa que no hay nadie escuchando, y entonces quien llama
/// debe seguir arrancando con normalidad: puede que el mutex estuviera
/// tomado por una copia que se estaba cerrando justo en ese instante.
pub fn enviar_ficheros(rutas: &[PathBuf]) -> bool {
    if rutas.is_empty() {
        return false;
    }
    // SAFETY: la clase es un literal estatico terminado en cero. Devuelve
    // una ventana nula si no hay ninguna, que se descarta abajo.
    let destino = unsafe { FindWindowW(w!("PixPinMaxVentanaMensajes"), None) };
    let Ok(destino) = destino else { return false };
    if destino.0.is_null() {
        return false;
    }
    let datos = empaquetar(rutas);
    let paquete = COPYDATASTRUCT {
        dwData: ABRIR_FICHEROS,
        cbData: std::mem::size_of_val(datos.as_slice()) as u32,
        lpData: datos.as_ptr() as *mut _,
    };
    // SAFETY: SendMessageW es SINCRONO, asi que `datos` y `paquete` siguen
    // vivos durante toda la llamada. Con PostMessage esto seria memoria
    // liberada leida por el otro proceso: es el fallo clasico de
    // WM_COPYDATA y por eso Windows exige mandarlo con Send y no con Post.
    let respuesta = unsafe {
        SendMessageW(
            destino,
            windows::Win32::UI::WindowsAndMessaging::WM_COPYDATA,
            Some(WPARAM(0)),
            Some(LPARAM(&paquete as *const _ as isize)),
        )
    };
    respuesta.0 != 0
}

/// Lee las rutas de un `WM_COPYDATA` recibido.
///
/// Devuelve vacio si el mensaje no es nuestro. Otro programa puede mandarle
/// `WM_COPYDATA` a cualquier ventana, asi que se comprueba el
/// identificador antes de hacerle caso a nada.
///
/// # Safety
///
/// `lparam` tiene que ser el de un `WM_COPYDATA` recien recibido, con su
/// `COPYDATASTRUCT` todavia vivo. Windows lo garantiza durante el
/// procesamiento del mensaje y NO despues: copiar aqui, no guardar el
/// puntero.
pub unsafe fn ficheros_de_copydata(lparam: LPARAM) -> Vec<PathBuf> {
    if lparam.0 == 0 {
        return Vec::new();
    }
    // SAFETY: el llamante garantiza que viene de un WM_COPYDATA vivo.
    let paquete = unsafe { &*(lparam.0 as *const COPYDATASTRUCT) };
    if paquete.dwData != ABRIR_FICHEROS || paquete.lpData.is_null() {
        return Vec::new();
    }
    let cuantas = paquete.cbData as usize / 2;
    if cuantas == 0 {
        return Vec::new();
    }
    // SAFETY: el otro proceso escribio `cbData` bytes de UTF-16 en esa
    // direccion, y Windows los ha copiado a nuestro espacio para la
    // duracion del mensaje.
    let unidades = unsafe { std::slice::from_raw_parts(paquete.lpData as *const u16, cuantas) };
    desempaquetar(unidades)
}

/// Las rutas que vienen en la linea de mandatos, ya filtradas.
///
/// Solo las que existen: Windows pasa la ruta del propio ejecutable como
/// primer argumento, y ademas un acceso directo roto o un fichero borrado
/// entre el doble clic y el arranque no deben abrir un pin vacio.
pub fn rutas_de_los_argumentos() -> Vec<PathBuf> {
    std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .filter(|r| r.is_file())
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_lista_de_rutas_va_y_vuelve() {
        let rutas = vec![
            PathBuf::from(r"C:\Users\alguien\foto.png"),
            PathBuf::from(r"D:\videos\clip largo.mp4"),
        ];
        assert_eq!(desempaquetar(&empaquetar(&rutas)), rutas);
    }

    #[test]
    fn los_espacios_y_los_acentos_sobreviven() {
        // Van en UTF-16 justamente por esto: una ruta con acentos o con
        // caracteres de otro alfabeto tiene que llegar entera.
        let rutas = vec![PathBuf::from(r"C:\Fotos\año 2026\niño ñu.png")];
        assert_eq!(desempaquetar(&empaquetar(&rutas)), rutas);
    }

    #[test]
    fn una_lista_vacia_no_da_rutas() {
        // Caso negativo: sin esto, empaquetar nada y desempaquetarlo podria
        // dar una ruta vacia que luego se intentaria abrir.
        assert!(empaquetar(&[]).is_empty());
        assert!(desempaquetar(&[]).is_empty());
        assert!(desempaquetar(&[0, 0, 0]).is_empty());
    }

    #[test]
    fn los_nulos_de_mas_no_inventan_rutas() {
        // Caso negativo del separador: dos nulos seguidos no son una ruta
        // vacia entre medias, son el final de una y el relleno del paquete.
        let unidades: Vec<u16> = "C:\\a.png\0\0\0D:\\b.png\0".encode_utf16().collect();
        assert_eq!(
            desempaquetar(&unidades),
            vec![PathBuf::from(r"C:\a.png"), PathBuf::from(r"D:\b.png")]
        );
    }
}
