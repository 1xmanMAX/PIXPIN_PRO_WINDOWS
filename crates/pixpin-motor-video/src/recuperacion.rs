//! **Cuando la grafica se pierde a mitad de grabacion** (un TDR, el
//! controlador que se actualiza solo, la grafica de alto rendimiento que se
//! apaga): que se reconoce como perdida y cuantas veces se intenta volver.
//!
//! Lo que hace OBS (`libobs-d3d11/d3d11-rebuild.cpp`, `RebuildDevice`): al
//! ver `DXGI_ERROR_DEVICE_REMOVED` o `_RESET`, suelta todo lo que vive en la
//! grafica, crea un dispositivo nuevo en el mismo adaptador y lo vuelve a
//! montar todo, avisando a cada fuente (`winrt_capture_device_loss_release`
//! / `_rebuild`, que recrean la sesion de captura entera). Aqui se hace lo
//! mismo con un tope: un controlador que se cae cada pocos segundos no va
//! a mejorar, y entonces se cierra el MP4 con lo que haya.
//!
//! Es puro: el backend dice si un fallo es una perdida ([`es_perdida`]) y
//! pregunta al [`Presupuesto`] si vuelve a intentarlo.

use crate::reloj::UNIDADES_POR_SEGUNDO;

/// `DXGI_ERROR_DEVICE_REMOVED`: la grafica ya no esta (TDR, controlador
/// actualizado, grafica apagada).
pub const DISPOSITIVO_QUITADO: i32 = 0x887A_0005_u32 as i32;
/// `DXGI_ERROR_DEVICE_HUNG`.
pub const DISPOSITIVO_COLGADO: i32 = 0x887A_0006_u32 as i32;
/// `DXGI_ERROR_DEVICE_RESET`.
pub const DISPOSITIVO_REINICIADO: i32 = 0x887A_0007_u32 as i32;
/// `DXGI_ERROR_DRIVER_INTERNAL_ERROR`.
pub const ERROR_DEL_CONTROLADOR: i32 = 0x887A_0020_u32 as i32;
/// `MF_E_DXGI_NEW_VIDEO_DEVICE`: el gestor de Media Foundation ya tiene otro
/// dispositivo.
pub const DISPOSITIVO_NUEVO_EN_MF: i32 = 0x8004_1001_u32 as i32;
/// `MF_E_DXGI_DEVICE_NOT_INITIALIZED`.
pub const DISPOSITIVO_SIN_INICIAR_EN_MF: i32 = 0x8004_1000_u32 as i32;

/// **Si un codigo de error es la grafica que se fue** (y no un fallo de
/// este fotograma).
pub fn es_perdida(codigo: i32) -> bool {
    matches!(
        codigo,
        DISPOSITIVO_QUITADO
            | DISPOSITIVO_COLGADO
            | DISPOSITIVO_REINICIADO
            | ERROR_DEL_CONTROLADOR
            | DISPOSITIVO_NUEVO_EN_MF
            | DISPOSITIVO_SIN_INICIAR_EN_MF
    )
}

/// Vueltas que se permiten en una [`VENTANA`].
pub const VUELTAS: usize = 3;
/// La ventana en la que se cuentan las vueltas: diez minutos.
pub const VENTANA: i64 = 600 * UNIDADES_POR_SEGUNDO;

/// Cuantas veces se ha vuelto a montar la grafica hace poco.
#[derive(Debug, Clone, Default)]
pub struct Presupuesto {
    vueltas: Vec<i64>,
}

impl Presupuesto {
    /// **Si se puede intentar otra vez** a la hora `ahora`; si se puede, la
    /// cuenta. Las de hace mas de [`VENTANA`] ya no cuentan: una perdida al
    /// dia en una grabacion de varias horas siempre se intenta.
    pub fn intentar(&mut self, ahora: i64) -> bool {
        self.vueltas.retain(|&t| ahora - t < VENTANA);
        if self.vueltas.len() >= VUELTAS {
            return false;
        }
        self.vueltas.push(ahora);
        true
    }

    /// Las vueltas que llevan cuenta ahora mismo.
    pub fn usadas(&self) -> usize {
        self.vueltas.len()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const S: i64 = UNIDADES_POR_SEGUNDO;

    #[test]
    fn las_perdidas_de_la_grafica_se_reconocen() {
        assert!(es_perdida(DISPOSITIVO_QUITADO));
        assert!(es_perdida(DISPOSITIVO_REINICIADO));
        assert!(es_perdida(DISPOSITIVO_NUEVO_EN_MF));
        // Caso negativo: un fallo cualquiera (E_FAIL, E_INVALIDARG) no lo es.
        assert!(!es_perdida(0x8000_4005_u32 as i32));
        assert!(!es_perdida(0x8007_0057_u32 as i32));
        assert!(!es_perdida(0));
    }

    #[test]
    fn se_vuelve_tres_veces_y_luego_se_cierra() {
        let mut p = Presupuesto::default();
        assert!(p.intentar(0));
        assert!(p.intentar(10 * S));
        assert!(p.intentar(20 * S));
        // Caso negativo: la cuarta en diez minutos, no.
        assert!(!p.intentar(30 * S));
        assert_eq!(p.usadas(), 3);
        // Pasados diez minutos de la primera, vuelve a haber sitio.
        assert!(p.intentar(VENTANA + S));
    }
}
