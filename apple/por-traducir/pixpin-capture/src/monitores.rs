//! Enumeracion real de monitores.
//!
//! Este modulo es la frontera: consulta Win32 y devuelve la
//! [`DisposicionMonitores`] de `pixpin-geom`, que es dato puro. Toda la
//! aritmetica posterior trabaja sobre ese dato y se prueba sin hardware.
//!
//! El identificador que se asigna a cada monitor es el indice de enumeracion,
//! estable **dentro de una misma llamada**. No sobrevive a un cambio de
//! configuracion de pantallas, asi que hay que volver a enumerar cuando eso
//! ocurra, no guardarlo indefinidamente.

use std::cell::RefCell;

use pixpin_geom::{DisposicionMonitores, Monitor, Rect};
use windows::Win32::Foundation::{LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
// En windows 0.62 esta constante vive en WindowsAndMessaging, no en Gdi.
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;
use windows::core::BOOL;

use crate::dispositivo::ErrorCaptura;

/// DPI de referencia de Windows: 96 puntos por pulgada es el 100%.
const DPI_BASE: u32 = 96;

thread_local! {
    /// Handles de la ultima enumeracion, por indice.
    ///
    /// Se guardan para poder crear despues un `GraphicsCaptureItem` del
    /// monitor elegido. Es `thread_local` porque los handles y la enumeracion
    /// pertenecen al hilo de interfaz.
    static HANDLES: RefCell<Vec<HMONITOR>> = const { RefCell::new(Vec::new()) };
}

/// Enumera los monitores conectados con su area, area de trabajo y escalado.
pub fn enumerar_monitores() -> Result<DisposicionMonitores, ErrorCaptura> {
    let mut recogidos: Vec<HMONITOR> = Vec::new();

    // SAFETY: se pasa un puntero al `Vec` local como `LPARAM`, y el callback
    // lo reconstruye como `&mut Vec<HMONITOR>`. El `Vec` sigue vivo durante
    // toda la llamada, que es sincrona: `EnumDisplayMonitors` no retiene el
    // puntero tras devolver.
    unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(acumular),
            LPARAM(&mut recogidos as *mut Vec<HMONITOR> as isize),
        )
        .ok()?;
    }

    let mut monitores = Vec::with_capacity(recogidos.len());
    for (indice, handle) in recogidos.iter().enumerate() {
        if let Some(m) = describir(*handle, indice as u32) {
            monitores.push(m);
        }
    }

    HANDLES.with(|h| *h.borrow_mut() = recogidos);
    Ok(DisposicionMonitores::nueva(monitores))
}

/// Handle del monitor con ese identificador, de la ultima enumeracion.
pub fn handle_de_monitor(id: u32) -> Option<HMONITOR> {
    HANDLES.with(|h| h.borrow().get(id as usize).copied())
}

extern "system" fn acumular(handle: HMONITOR, _hdc: HDC, _rc: *mut RECT, datos: LPARAM) -> BOOL {
    // SAFETY: `datos` es el puntero que pasamos nosotros en
    // `EnumDisplayMonitors`, apunta a un `Vec<HMONITOR>` vivo durante toda la
    // enumeracion, y este callback solo se invoca desde esa llamada.
    let destino = unsafe { &mut *(datos.0 as *mut Vec<HMONITOR>) };
    destino.push(handle);
    TRUE
}

fn describir(handle: HMONITOR, id: u32) -> Option<Monitor> {
    let mut info = MONITORINFOEXW {
        monitorInfo: MONITORINFO {
            cbSize: size_of::<MONITORINFOEXW>() as u32,
            ..Default::default()
        },
        ..Default::default()
    };

    // SAFETY: `info` esta inicializada con el `cbSize` que exige la API, y se
    // pasa como `MONITORINFO` porque `MONITORINFOEXW` empieza exactamente con
    // esa estructura — es el patron documentado por Microsoft.
    let ok = unsafe {
        GetMonitorInfoW(handle, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool()
    };
    if !ok {
        return None;
    }

    let mut dpi_x = DPI_BASE;
    let mut dpi_y = DPI_BASE;
    // SAFETY: `handle` viene de la enumeracion y sigue siendo valido; los dos
    // punteros de salida son variables locales.
    // Si la llamada falla se conserva el valor por defecto de 96 (100%), que
    // es preferible a rechazar el monitor entero.
    let _ = unsafe { GetDpiForMonitor(handle, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };

    Some(Monitor {
        id,
        area: desde_rect(info.monitorInfo.rcMonitor),
        area_trabajo: desde_rect(info.monitorInfo.rcWork),
        escala_por_cien: dpi_x * 100 / DPI_BASE,
        principal: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
    })
}

fn desde_rect(r: RECT) -> Rect {
    Rect {
        x: r.left,
        y: r.top,
        ancho: (r.right - r.left).max(0) as u32,
        alto: (r.bottom - r.top).max(0) as u32,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Necesita una sesion de escritorio real. Ejecutar con `--ignored`.
    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn enumera_al_menos_un_monitor_y_uno_es_el_principal() {
        let d = enumerar_monitores().expect("deberia enumerar monitores");
        assert!(
            !d.monitores().is_empty(),
            "toda sesion tiene al menos un monitor"
        );
        assert_eq!(
            d.monitores().iter().filter(|m| m.principal).count(),
            1,
            "Windows garantiza exactamente un monitor principal"
        );
    }

    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn los_datos_de_cada_monitor_son_coherentes() {
        let d = enumerar_monitores().unwrap();
        for m in d.monitores() {
            assert!(!m.area.esta_vacio(), "monitor {} con area vacia", m.id);
            assert!(
                (100..=500).contains(&m.escala_por_cien),
                "escalado fuera de rango en el monitor {}: {}",
                m.id,
                m.escala_por_cien
            );
            // El area de trabajo es parte del area total, nunca al reves.
            assert_eq!(
                m.area.interseccion(m.area_trabajo),
                Some(m.area_trabajo),
                "el area de trabajo del monitor {} se sale del area total",
                m.id
            );
        }
    }

    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn ningun_par_de_monitores_se_solapa() {
        // Caso negativo importante: si dos areas se solaparan, `monitor_en`
        // devolveria uno u otro segun el orden y el overlay se dibujaria dos
        // veces sobre la misma franja.
        let d = enumerar_monitores().unwrap();
        let ms = d.monitores();
        for (i, a) in ms.iter().enumerate() {
            for b in &ms[i + 1..] {
                assert_eq!(
                    a.area.interseccion(b.area),
                    None,
                    "los monitores {} y {} se solapan",
                    a.id,
                    b.id
                );
            }
        }
    }

    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn cada_id_enumerado_tiene_su_handle() {
        let d = enumerar_monitores().unwrap();
        for m in d.monitores() {
            assert!(
                handle_de_monitor(m.id).is_some(),
                "el monitor {} no tiene handle recuperable",
                m.id
            );
        }
        assert!(
            handle_de_monitor(9999).is_none(),
            "un id inventado no debe resolver"
        );
    }
}
