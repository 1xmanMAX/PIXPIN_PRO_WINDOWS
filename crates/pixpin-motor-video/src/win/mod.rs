//! Lo que habla con Windows: la eleccion de la grafica, la captura, el
//! procesador de video, el escritor del MP4, los caminos de la cadena y el
//! hilo del motor. Es el backend de Windows de los rasgos de
//! `crate::plataforma`.

mod captura;
mod com;
mod conversor;
mod escritor;
mod grafica;
mod motor;
mod tubo;

pub use motor::MotorVideo;

use windows::Win32::Media::MediaFoundation::{MFVideoFormat_H264, MFVideoFormat_HEVC};

use crate::Fuente;

/// **Si hay un codificador H.264 de la grafica** registrado (Quick Sync,
/// NVENC, VCE...). Que lo haya no asegura que arranque: el motor lo prueba
/// y, si no, cae al siguiente.
pub fn hay_h264_por_hardware() -> bool {
    let _com = com::ComDelHilo::nuevo();
    com::arrancar_media_foundation();
    !grafica::codificadores_de_hardware(MFVideoFormat_H264).is_empty()
}

/// **Si la grafica codifica HEVC.** Solo entonces tiene sentido ofrecerlo
/// (`Opciones::hevc`): por software no lo hay en todos los Windows, y H.264
/// sigue siendo lo que reproduce cualquier movil.
pub fn hay_hevc_por_hardware() -> bool {
    let _com = com::ComDelHilo::nuevo();
    com::arrancar_media_foundation();
    !grafica::codificadores_de_hardware(MFVideoFormat_HEVC).is_empty()
}

/// El monitor principal (el del punto 0,0).
pub fn monitor_principal() -> Fuente {
    Fuente::Monitor(grafica::principal().0 as isize)
}
