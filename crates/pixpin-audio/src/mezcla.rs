//! Las cuentas de muestreo y duracion de una nota.
//!
//! Lo que antes vivia aqui —bajar a mono, pasar a enteros y un
//! remuestreador lineal— lo hace ahora `pixpin-motor-audio`, con un filtro
//! de verdad y siguiendo el reloj de cada tarjeta. Queda lo que sigue
//! valiendo para cualquiera que mire una nota: que muestreos admite el
//! codificador y cuanto dura un numero de muestras.

/// Los dos unicos muestreos que traga el codificador AAC de Windows.
///
/// Documentado por Microsoft para el «AAC Encoder» (el MFT que usa el sink
/// writer): entrada PCM de 16 bits, 1 o 2 canales, y **44 100 o 48 000**
/// muestras por segundo. Los 22 050 Hz del movil (`Voz.kt:140`) no estan, y
/// esa es la unica diferencia de formato que no se puede evitar.
pub const MUESTREOS_QUE_ADMITE_AAC: [u32; 2] = [44_100, 48_000];

/// A que muestreo se graba, sabiendo a cual esta la tarjeta.
///
/// Se prefiere **no remuestrear**: si la tarjeta ya esta en uno de los dos
/// que admite el codificador, se usa ese y las muestras pasan intactas. En
/// cualquier otro caso se sube a 48 000, que es el de la inmensa mayoria de
/// los equipos y el que menos artefactos deja al interpolar.
pub fn muestreo_de_grabacion(el_de_la_tarjeta: u32) -> u32 {
    if MUESTREOS_QUE_ADMITE_AAC.contains(&el_de_la_tarjeta) {
        el_de_la_tarjeta
    } else {
        48_000
    }
}

/// Cuanto duran `muestras` muestras mono, en milisegundos.
///
/// Se redondea hacia abajo, que es lo que hace `MediaPlayer.duration`: una
/// nota de 6 999 ms se rotula `0:06` en los dos aparatos.
pub fn duracion_ms(muestras: usize, muestreo: u32) -> i64 {
    if muestreo == 0 {
        return 0;
    }
    (muestras as i64 * 1000) / muestreo as i64
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_graba_al_muestreo_de_la_tarjeta_si_el_codificador_lo_admite() {
        assert_eq!(muestreo_de_grabacion(48_000), 48_000);
        assert_eq!(muestreo_de_grabacion(44_100), 44_100);
    }

    #[test]
    fn un_muestreo_que_el_codificador_no_admite_sube_a_cuarenta_y_ocho_mil() {
        // Los 22 050 del movil son justo el caso: el AAC de Windows no los
        // acepta.
        assert_eq!(muestreo_de_grabacion(22_050), 48_000);
        assert_eq!(muestreo_de_grabacion(16_000), 48_000);
        assert_eq!(muestreo_de_grabacion(0), 48_000);
    }

    #[test]
    fn la_duracion_sale_de_las_muestras_y_no_del_reloj() {
        assert_eq!(duracion_ms(48_000, 48_000), 1_000);
        assert_eq!(duracion_ms(0, 48_000), 0);
        assert_eq!(duracion_ms(48_000, 0), 0);
    }
}
