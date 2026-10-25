//! **Las graficas del equipo y sus codificadores**: en cual se crea el
//! dispositivo (`crate::adaptador`), que codificadores de hardware hay y de
//! que fabricante, si el dispositivo se perdio, y los monitores conectados
//! (para volver a abrir uno que se desconecto, `crate::reapertura`).

use windows::Win32::Foundation::{HWND, LPARAM, LUID, POINT, RECT};
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIAdapter, IDXGIDevice, IDXGIFactory1,
};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST,
    MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MONITORINFOEXW, MonitorFromPoint, MonitorFromWindow,
};
use windows::Win32::Media::MediaFoundation::{
    IMFActivate, MFMediaType_Video, MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG,
    MFT_ENUM_FLAG_HARDWARE, MFT_ENUM_FLAG_SORTANDFILTER, MFT_ENUM_HARDWARE_VENDOR_ID_Attribute,
    MFT_FRIENDLY_NAME_Attribute, MFT_REGISTER_TYPE_INFO, MFTEnumEx, MFVideoFormat_NV12,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::core::{BOOL, GUID, Interface};

use crate::adaptador::{self, Candidato, Fabricante};
use crate::{ErrorMotor, Fuente};

/// El monitor en el que esta la fuente.
pub(crate) fn monitor_de(fuente: Fuente) -> HMONITOR {
    match fuente {
        Fuente::Monitor(h) => HMONITOR(h as *mut _),
        // SAFETY: consulta de solo lectura; con `DEFAULTTONEAREST` siempre
        // da un monitor, aunque la ventana ya no exista.
        Fuente::Ventana(h) => unsafe {
            MonitorFromWindow(HWND(h as *mut _), MONITOR_DEFAULTTONEAREST)
        },
    }
}

/// El monitor principal (el del punto 0,0).
pub(crate) fn principal() -> HMONITOR {
    // SAFETY: consulta de solo lectura; con `DEFAULTTOPRIMARY` siempre da
    // un monitor.
    unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) }
}

/// El nombre de dispositivo de un monitor (`\\.\DISPLAY1`), o vacio.
pub(crate) fn nombre_del_monitor(h: HMONITOR) -> String {
    let mut info = MONITORINFOEXW {
        monitorInfo: MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFOEXW>() as u32,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: `info` es local y su `cbSize` dice que es la version
    // extendida, que es la que la funcion rellena.
    let hecho = unsafe { GetMonitorInfoW(h, &mut info.monitorInfo) };
    if !hecho.as_bool() {
        return String::new();
    }
    let fin = info
        .szDevice
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(info.szDevice.len());
    String::from_utf16_lossy(&info.szDevice[..fin])
}

/// Los monitores conectados, con su manejador y su nombre.
pub(crate) fn monitores() -> Vec<(isize, String)> {
    unsafe extern "system" fn uno(h: HMONITOR, _: HDC, _: *mut RECT, datos: LPARAM) -> BOOL {
        // SAFETY: `datos` es el puntero al vector local que se paso a
        // `EnumDisplayMonitors`, vivo durante toda la enumeracion, que es
        // sincrona y en este mismo hilo.
        let v = unsafe { &mut *(datos.0 as *mut Vec<isize>) };
        v.push(h.0 as isize);
        true.into()
    }
    let mut v: Vec<isize> = Vec::new();
    // SAFETY: la funcion de vuelta solo escribe en `v` (ver arriba).
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(uno), LPARAM(&mut v as *mut _ as isize));
    }
    v.into_iter()
        .map(|h| (h, nombre_del_monitor(HMONITOR(h as *mut _))))
        .collect()
}

/// Lo que se sabe de la grafica de un dispositivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Identidad {
    pub fabricante: u32,
    pub dispositivo: u32,
    pub luid: (u32, i32),
}

pub(crate) fn identidad(d3d: &ID3D11Device) -> Option<Identidad> {
    // SAFETY: consultas de solo lectura sobre un dispositivo vivo.
    unsafe {
        let a = d3d.cast::<IDXGIDevice>().ok()?.GetAdapter().ok()?;
        let d = a.GetDesc().ok()?;
        Some(Identidad {
            fabricante: d.VendorId,
            dispositivo: d.DeviceId,
            luid: luid(d.AdapterLuid),
        })
    }
}

fn luid(l: LUID) -> (u32, i32) {
    (l.LowPart, l.HighPart)
}

/// **Si el dispositivo se perdio** (`GetDeviceRemovedReason` distinto de
/// `S_OK`): el codigo, para el registro y para `recuperacion::es_perdida`.
pub(crate) fn perdido(d3d: &ID3D11Device) -> Option<i32> {
    // SAFETY: consulta de solo lectura sobre un dispositivo vivo (o muerto:
    // es justo para lo que esta).
    match unsafe { d3d.GetDeviceRemovedReason() } {
        Ok(()) => None,
        Err(e) => Some(e.code().0),
    }
}

/// **El dispositivo para grabar `fuente`**: en la grafica que pinta su
/// monitor, si es de verdad (`crate::adaptador::elegir`); si no, lo de
/// siempre de `pixpin-capture`.
pub(crate) fn dispositivo_para(fuente: Fuente) -> Result<pixpin_capture::Dispositivo, ErrorMotor> {
    let monitor = monitor_de(fuente);
    let lista = adaptadores(monitor);
    let candidatos: Vec<Candidato> = lista.iter().map(|(c, _)| c.clone()).collect();
    let elegido = adaptador::elegir(&candidatos);
    if let Some(i) = elegido {
        tracing::info!(
            grafica = %candidatos[i].descripcion,
            con_el_monitor = candidatos[i].tiene_el_monitor,
            de = candidatos.len(),
            "grafica elegida para grabar"
        );
    }
    let preferido = elegido.map(|i| lista[i].1.clone());
    pixpin_capture::Dispositivo::sobre_adaptador(preferido)
        .map_err(|e| ErrorMotor::Fuente(e.to_string()))
}

/// Las graficas del equipo y si tienen `monitor`.
fn adaptadores(monitor: HMONITOR) -> Vec<(Candidato, IDXGIAdapter)> {
    let mut v = Vec::new();
    // SAFETY: consultas de solo lectura a DXGI; las interfaces devueltas
    // traen su propia cuenta de referencias.
    unsafe {
        let Ok(fabrica) = CreateDXGIFactory1::<IDXGIFactory1>() else {
            return v;
        };
        let mut i = 0;
        while let Ok(a) = fabrica.EnumAdapters1(i) {
            i += 1;
            let Ok(d) = a.GetDesc1() else {
                continue;
            };
            let mut tiene = false;
            let mut j = 0;
            while let Ok(salida) = a.EnumOutputs(j) {
                j += 1;
                if salida.GetDesc().is_ok_and(|s| s.Monitor == monitor) {
                    tiene = true;
                }
            }
            let fin = d
                .Description
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(d.Description.len());
            let candidato = Candidato {
                descripcion: String::from_utf16_lossy(&d.Description[..fin]),
                fabricante: d.VendorId,
                dispositivo: d.DeviceId,
                software: d.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0,
                tiene_el_monitor: tiene,
            };
            if let Ok(a) = a.cast::<IDXGIAdapter>() {
                v.push((candidato, a));
            }
        }
    }
    v
}

/// Un codificador de hardware registrado.
#[derive(Debug, Clone)]
pub(crate) struct Codificador {
    pub nombre: String,
    pub fabricante: Option<Fabricante>,
}

/// **Los codificadores de hardware** de NV12 a `subtipo` (H.264, HEVC),
/// con su nombre y su fabricante, como los enumera Cap
/// (`enc-mediafoundation/src/mft.rs`, `EncoderDevice::enumerate`).
pub(crate) fn codificadores_de_hardware(subtipo: GUID) -> Vec<Codificador> {
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
    let mut v = Vec::new();
    if !lista.is_null() {
        // SAFETY: `MFTEnumEx` deja `cuantos` activadores con su referencia
        // en un bloque de `CoTaskMemAlloc`: se lee cada uno (sacandolo del
        // bloque, con su referencia) y luego se suelta el bloque.
        unsafe {
            for i in 0..cuantos as usize {
                if let Some(a) = std::ptr::read(lista.add(i)) {
                    v.push(Codificador {
                        nombre: cadena(&a, &MFT_FRIENDLY_NAME_Attribute).unwrap_or_default(),
                        fabricante: cadena(&a, &MFT_ENUM_HARDWARE_VENDOR_ID_Attribute)
                            .and_then(|c| Fabricante::de_cadena_mft(&c)),
                    });
                }
            }
            CoTaskMemFree(Some(lista as *const _));
        }
    }
    if hecho.is_err() {
        v.clear();
    }
    v
}

/// Un atributo de texto de un activador, si lo tiene.
fn cadena(a: &IMFActivate, clave: &GUID) -> Option<String> {
    // SAFETY: activador vivo; se pide la longitud y luego se lee en un
    // buffer de ese tamano mas el cero final.
    unsafe {
        let n = a.GetStringLength(clave).ok()?;
        let mut buf = vec![0u16; n as usize + 1];
        let mut leido = 0u32;
        a.GetString(clave, &mut buf, Some(&mut leido)).ok()?;
        buf.truncate(leido as usize);
        Some(String::from_utf16_lossy(&buf))
    }
}
