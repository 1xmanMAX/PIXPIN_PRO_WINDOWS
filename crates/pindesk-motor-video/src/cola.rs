//! **Las colas, todas con tope**: que el codificador se atrase no puede
//! llenar la memoria del equipo de referencia (4 GB) a mitad de reunion.
//!
//! - **Los mensajes al hilo del motor** (sonido, avisos de fotograma,
//!   pausas) van por un canal con [`CAPACIDAD_MENSAJES`]: el sonido que no
//!   cabe **espera** a que haya sitio (nunca se tira); el aviso de un
//!   fotograma nuevo que no cabe se tira, porque el fotograma sigue retenido
//!   y el motor lo ve igual en su siguiente vuelta.
//! - **Los fotogramas en la grafica**: el repartidor de muestras de Media
//!   Foundation tiene [`MUESTRAS_GPU`] texturas NV12 como mucho. Si estan
//!   todas dentro del codificador, el fotograma se salta.
//! - **Los fotogramas por software**: se mira cuantos esperan al codificador
//!   y por encima de [`TOPE_SOFTWARE`] se salta ([`admitir_en_software`]).
//! - **El atasco**: si el codificador no suelta nada en [`ATASCO`], no va a
//!   volver; se corta el video y se guarda lo que habia ([`Atasco`]).

use crate::reloj::UNIDADES_POR_SEGUNDO;

/// Mensajes que caben en el canal del motor. Los bloques de sonido llegan
/// cada 10-20 ms: 256 son varios segundos de holgura.
pub const CAPACIDAD_MENSAJES: usize = 256;

/// Texturas NV12 del repartidor de la grafica: las que se dan al empezar y
/// las que puede haber como mucho. A 1080p son 3 MB cada una.
pub const MUESTRAS_GPU_INICIALES: u32 = 3;
pub const MUESTRAS_GPU: u32 = 8;

/// Fotogramas que pueden esperar al codificador por software. Cada uno es
/// un NV12 en memoria (3 MB a 1080p): seis son 18 MB.
pub const TOPE_SOFTWARE: u64 = 6;

/// Lo que se aguanta sin poder dar ningun fotograma al codificador.
pub const ATASCO: i64 = 10 * UNIDADES_POR_SEGUNDO;

/// Si un fotograma entra en el codificador por software con `en_cola`
/// esperando. El primero siempre: sin el no hay video.
pub fn admitir_en_software(escritos: u64, en_cola: u64) -> bool {
    escritos == 0 || en_cola < TOPE_SOFTWARE
}

/// Cuenta cuanto lleva el codificador sin aceptar fotogramas.
#[derive(Debug, Clone, Default)]
pub struct Atasco {
    desde: Option<i64>,
}

impl Atasco {
    /// Un fotograma entro: no hay atasco.
    pub fn entro(&mut self) {
        self.desde = None;
    }

    /// Se vuelve a contar de cero: tras una pausa o una vuelta de la grafica
    /// (`recuperacion`). Sin esto, un fotograma que no entro antes de una
    /// pausa larga haria que el primero que no entre al seguir pareciera un
    /// atasco de minutos y cortara el video: el fallo que Cap arreglo con
    /// `notify_input_stall_recovered` (`enc-mediafoundation/src/video/h264.rs`).
    pub fn reiniciar(&mut self) {
        self.desde = None;
    }

    /// Un fotograma no entro a la hora `ahora`. Devuelve si ya es un atasco
    /// de los que no se pasan.
    pub fn no_entro(&mut self, ahora: i64) -> bool {
        let desde = *self.desde.get_or_insert(ahora);
        ahora - desde >= ATASCO
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_software_admite_hasta_su_tope() {
        assert!(admitir_en_software(0, 1_000), "el primero siempre");
        assert!(admitir_en_software(10, 0));
        assert!(admitir_en_software(10, TOPE_SOFTWARE - 1));
        // Caso negativo: con la cola llena se salta.
        assert!(!admitir_en_software(10, TOPE_SOFTWARE));
    }

    #[test]
    fn un_atasco_tiene_que_durar_para_contar() {
        let mut a = Atasco::default();
        assert!(!a.no_entro(0));
        assert!(!a.no_entro(ATASCO - 1));
        assert!(a.no_entro(ATASCO));
        // Caso negativo: con que entre uno, se vuelve a contar de cero.
        a.entro();
        assert!(!a.no_entro(ATASCO * 3));
    }

    #[test]
    fn una_pausa_larga_no_parece_un_atasco() {
        let mut a = Atasco::default();
        // Uno no entro y luego se pauso diez minutos.
        assert!(!a.no_entro(0));
        a.reiniciar();
        // Caso negativo: el primero que no entra al seguir no es un atasco.
        assert!(!a.no_entro(600 * crate::reloj::UNIDADES_POR_SEGUNDO));
    }

    #[test]
    fn la_memoria_de_las_colas_tiene_un_techo() {
        // 1080p NV12: 3 MB por fotograma. Las dos colas de fotogramas
        // juntas no pasan de ~45 MB aunque el codificador no de abasto.
        let fotograma = 1920u64 * 1080 * 3 / 2;
        let techo = (u64::from(MUESTRAS_GPU) + TOPE_SOFTWARE) * fotograma;
        assert!(techo < 45 * 1024 * 1024, "{techo}");
        const { assert!(MUESTRAS_GPU_INICIALES <= MUESTRAS_GPU) };
    }
}
