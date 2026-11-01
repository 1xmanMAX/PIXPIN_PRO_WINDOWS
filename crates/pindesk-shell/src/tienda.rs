//! **Lo de Windows que solo usa la version de la Microsoft Store** (rama
//! `microsoft-store`, ver `docs/tienda.md` y `docs/ramas.md`).
//!
//! Dos cosas, las dos sin efecto fuera del paquete MSIX:
//!
//! - [`familia_del_paquete`]: el nombre de familia del paquete con que
//!   corre este proceso. Distingue el paquete de la Store
//!   ([`FAMILIA_TIENDA`]) del paquete de IDENTIDAD de siempre
//!   (`PinDesk`, `herramientas/paquete-identidad.ps1`), que
//!   tambien da identidad pero corre desde la carpeta instalada y SI puede
//!   escribir en el registro.
//! - La **tarea de inicio** del paquete (`desktop:StartupTask` del
//!   manifiesto, `tienda/AppxManifest.xml`), que en la Store sustituye a la
//!   entrada de `HKCU\...\Run` de `arranque.rs`: la certificacion no deja
//!   que una app empaquetada se apunte sola en `Run`, y el usuario la ve y
//!   la apaga en Configuracion › Aplicaciones › Inicio.

use windows::ApplicationModel::{StartupTask, StartupTaskState};
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS};
use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFamilyName;
use windows::core::{HSTRING, PWSTR};

/// La familia del paquete reservado en Partner Center (nombre `MingXuan.PinDesk`
/// y el resumen del editor).
pub const FAMILIA_TIENDA: &str = "MingXuan.PinDesk_mtqkn9ff51s0e";

/// El `TaskId` de `desktop:StartupTask` en `tienda/AppxManifest.xml`.
/// Tienen que ser iguales: si no, [`tarea_de_inicio`] no la encuentra.
pub const TAREA_DE_INICIO: &str = "PinDeskInicio";

/// El nombre de familia del paquete de este proceso, o `None` si corre sin
/// paquete (el ZIP de `main`, el modo portable).
pub fn familia_del_paquete() -> Option<String> {
    let mut largo = 0u32;
    // SAFETY: con buffer nulo solo escribe en `largo` cuantos u16 hacen
    // falta (con el cero final).
    let r = unsafe { GetCurrentPackageFamilyName(&mut largo, None) };
    if r != ERROR_INSUFFICIENT_BUFFER || largo == 0 {
        return None;
    }
    let mut buf = vec![0u16; largo as usize];
    // SAFETY: `buf` mide los `largo` u16 que pidio Windows y vive durante
    // la llamada; Windows no escribe mas alla de `largo`.
    let r = unsafe { GetCurrentPackageFamilyName(&mut largo, Some(PWSTR(buf.as_mut_ptr()))) };
    if r != ERROR_SUCCESS {
        return None;
    }
    let fin = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..fin]))
}

/// Si una familia es la del paquete de la Store (sin mirar mayusculas: el
/// resumen del editor viene en minusculas, el nombre no).
pub fn es_familia_tienda(familia: &str) -> bool {
    familia.eq_ignore_ascii_case(FAMILIA_TIENDA)
}

/// Si este proceso corre dentro del paquete de la Store. Se mira una vez.
pub fn dentro_del_paquete_tienda() -> bool {
    static DENTRO: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *DENTRO.get_or_init(|| familia_del_paquete().is_some_and(|f| es_familia_tienda(&f)))
}

/// Como esta la tarea de inicio, en palabras de PinDesk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoInicio {
    /// Arranca con Windows (la encendio el usuario o una directiva).
    Encendida,
    /// Apagada; se puede encender desde la app.
    Apagada,
    /// Apagada por el usuario en Configuracion › Aplicaciones › Inicio: la
    /// app ya no puede encenderla, solo el usuario desde ahi.
    ApagadaPorElUsuario,
    /// Apagada por una directiva de la empresa.
    ApagadaPorDirectiva,
}

impl EstadoInicio {
    fn de(s: StartupTaskState) -> EstadoInicio {
        match s {
            StartupTaskState::Enabled | StartupTaskState::EnabledByPolicy => {
                EstadoInicio::Encendida
            }
            StartupTaskState::DisabledByUser => EstadoInicio::ApagadaPorElUsuario,
            StartupTaskState::DisabledByPolicy => EstadoInicio::ApagadaPorDirectiva,
            _ => EstadoInicio::Apagada,
        }
    }

    /// Si con este estado la app arranca con Windows.
    pub fn encendida(self) -> bool {
        self == EstadoInicio::Encendida
    }
}

fn tarea_de_inicio() -> windows::core::Result<StartupTask> {
    StartupTask::GetAsync(&HSTRING::from(TAREA_DE_INICIO))?.join()
}

/// El estado de la tarea de inicio. `Err` fuera de un paquete (no hay
/// tarea que mirar) o si el manifiesto no la declara.
pub fn estado_inicio() -> windows::core::Result<EstadoInicio> {
    Ok(EstadoInicio::de(tarea_de_inicio()?.State()?))
}

/// Enciende o apaga la tarea de inicio y devuelve como quedo. Encenderla
/// no siempre se puede: si el usuario la apago en Configuracion, Windows
/// contesta [`EstadoInicio::ApagadaPorElUsuario`] y no cambia nada (lo
/// decide el usuario, no la app). Bloquea hasta que Windows contesta; para
/// una app de escritorio no sale ningun cuadro.
pub fn fijar_inicio(encender: bool) -> windows::core::Result<EstadoInicio> {
    let tarea = tarea_de_inicio()?;
    if encender {
        Ok(EstadoInicio::de(tarea.RequestEnableAsync()?.join()?))
    } else {
        tarea.Disable()?;
        Ok(EstadoInicio::de(tarea.State()?))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_familia_de_la_tienda_se_reconoce_sin_mirar_mayusculas() {
        assert!(es_familia_tienda("MingXuan.PinDesk_mtqkn9ff51s0e"));
        assert!(es_familia_tienda("mingxuan.pindesk_MTQKN9FF51S0E"));
        // El paquete de identidad de siempre NO es la tienda: ese si puede
        // escribir en el registro.
        assert!(!es_familia_tienda("PinDesk_abc123xyz0000"));
        // Ni el de antes del cambio de nombre.
        assert!(!es_familia_tienda("ForgePixPin.PixPinMax_abc123"));
        assert!(!es_familia_tienda(""));
    }

    #[test]
    fn las_pruebas_no_corren_dentro_del_paquete() {
        // `cargo test` nunca corre empaquetado: sin familia y sin tarea.
        assert_eq!(familia_del_paquete(), None);
        assert!(!dentro_del_paquete_tienda());
        assert!(estado_inicio().is_err());
    }

    #[test]
    fn solo_encendida_cuenta_como_arrancar_con_windows() {
        assert!(EstadoInicio::Encendida.encendida());
        assert!(!EstadoInicio::Apagada.encendida());
        assert!(!EstadoInicio::ApagadaPorElUsuario.encendida());
        assert!(!EstadoInicio::ApagadaPorDirectiva.encendida());
    }
}
