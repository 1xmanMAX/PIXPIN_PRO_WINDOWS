//! Lo que habla con Windows: la captura, el procesador de video, el
//! escritor del MP4, los tres caminos de la cadena y el hilo del motor.

mod captura;
mod com;
mod conversor;
mod escritor;
mod motor;
mod tubo;

pub use motor::MotorVideo;

use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTOPRIMARY, MonitorFromPoint};
use windows::Win32::Media::MediaFoundation::{
    IMFActivate, MFMediaType_Video, MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG,
    MFT_ENUM_FLAG_HARDWARE, MFT_ENUM_FLAG_SORTANDFILTER, MFT_REGISTER_TYPE_INFO, MFTEnumEx,
    MFVideoFormat_H264, MFVideoFormat_HEVC, MFVideoFormat_NV12,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::core::GUID;

use crate::Fuente;

/// **Si hay un codificador H.264 de la grafica** registrado (Quick Sync,
/// NVENC, VCE...). Que lo haya no asegura que arranque: el motor lo prueba
/// y, si no, cae al siguiente.
pub fn hay_h264_por_hardware() -> bool {
    hay_codificador_de_hardware(MFVideoFormat_H264)
}

/// **Si la grafica codifica HEVC.** Solo entonces tiene sentido ofrecerlo
/// (`Opciones::hevc`): por software no lo hay en todos los Windows, y H.264
/// sigue siendo lo que reproduce cualquier movil.
pub fn hay_hevc_por_hardware() -> bool {
    hay_codificador_de_hardware(MFVideoFormat_HEVC)
}

fn hay_codificador_de_hardware(subtipo: GUID) -> bool {
    let _com = com::ComDelHilo::nuevo();
    com::arrancar_media_foundation();
    let entrada = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_NV12,
    };
    let salida = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: subtipo,
    };
    let mut lista: *mut Option<IMFActivate> = std::ptr::null_mut();
    let mut cuantos = 0u32;
    let banderas = MFT_ENUM_FLAG(MFT_ENUM_FLAG_HARDWARE.0 | MFT_ENUM_FLAG_SORTANDFILTER.0);
    // SAFETY: salidas locales; los tipos viven durante la llamada.
    let hecho = unsafe {
        MFTEnumEx(
            MFT_CATEGORY_VIDEO_ENCODER,
            banderas,
            Some(&entrada),
            Some(&salida),
            &mut lista,
            &mut cuantos,
        )
    };
    if !lista.is_null() {
        // SAFETY: `MFTEnumEx` deja `cuantos` activadores con su referencia
        // en un bloque de `CoTaskMemAlloc`: se suelta cada uno (leyendolos
        // fuera del bloque) y luego el bloque.
        unsafe {
            for i in 0..cuantos as usize {
                drop(std::ptr::read(lista.add(i)));
            }
            CoTaskMemFree(Some(lista as *const _));
        }
    }
    hecho.is_ok() && cuantos > 0
}

/// El monitor principal (el del punto 0,0).
pub fn monitor_principal() -> Fuente {
    // SAFETY: consulta de solo lectura; con `DEFAULTTOPRIMARY` siempre da
    // un monitor.
    let h = unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) };
    Fuente::Monitor(h.0 as isize)
}
