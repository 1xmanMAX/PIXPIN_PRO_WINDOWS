//! **El motor de audio** de PixPin Max: el microfono y el sonido del equipo,
//! juntos, a la par durante horas y gastando lo minimo.
//!
//! Lo usan las notas de voz, el microfono flotante y la grabadora de clases
//! y reuniones (a traves de `pixpin-audio::Grabadora`), y el motor de video
//! de la reunion, que se suscribe a su PCM. La arquitectura entera esta en
//! `docs/motor-de-audio.md`; aqui, el reparto:
//!
//! - **Todo lo que no es del sistema, probado en Linux** (y que compila
//!   para Android sin tocar nada): [`formato`] (leer el `WAVEFORMATEX` y
//!   bajar a mono o estereo), [`anillo`] (la cola sin cerrojos entre captura
//!   y mezcla), [`reloj`] (rellenar los huecos de silencio por el reloj del
//!   sistema, y no rellenar las suspensiones), [`toma`] (colocar cada
//!   paquete por su fecha), [`deriva`] (seguir el reloj de cada tarjeta),
//!   [`remuestreo`] (seno enventanado polifasico), [`dsp`] (paso alto,
//!   puerta, realce, limitador), [`nucleo`] (la mezcla), [`pcm`] (el reparto
//!   a los suscriptores), [`medidas`], [`plataforma`] (lo que se le pide a
//!   cada sistema, el bucle de captura con sus reaperturas y el hueco del
//!   AAC), [`empuje`] (un aparato al que le empujan el sonido, para Android)
//!   y [`MotorAudio`], que lo junta todo con el [`plataforma::Sistema`] que
//!   se le de.
//! - **Windows** (`cfg(windows)`): el sistema de Windows — la captura WASAPI
//!   de cada fuente, los avisos de cambio de aparato, la captura por proceso
//!   y el `.m4a` con Media Foundation —, que es el de
//!   `MotorAudio::empezar`.

pub mod anillo;
pub mod deriva;
pub mod dsp;
pub mod empuje;
pub mod formato;
pub mod medidas;
pub mod nucleo;
pub mod opciones;
pub mod pcm;
pub mod plataforma;
pub mod reloj;
pub mod remuestreo;
pub mod respaldo;
pub mod toma;

mod motor;
#[cfg(windows)]
mod sistema;

pub use medidas::{Estadisticas, EstadisticasFuente, Nivel, Niveles};
pub use opciones::{Equipo, MUESTREOS, Microfono, Opciones, Salida};
pub use pcm::{BloquePcm, Receptor, SinBloque, Suscripcion, suscripcion};

pub use motor::{CAPACIDAD_AAC, MotorAudio, Resumen, SEGUNDOS_DE_ANILLO};
#[cfg(windows)]
pub use sistema::aparatos::{Aparato, microfonos};

/// **Vuelve a hacer el `.m4a` desde un respaldo WAV** ([`respaldo`]): lo
/// que queda de una grabacion cortada (la aplicacion murio, se fue la luz).
/// Escribe `destino` (AAC-LC en MPEG-4, como siempre) y devuelve lo que dura
/// en milisegundos. Lento lo justo: el codificador AAC de Windows va muy por
/// encima del tiempo real (una hora en unos pocos segundos).
///
/// Fuera de Windows no hay codificador: [`ErrorMotor::SinCodificadorAac`].
pub fn rehacer_m4a(wav: &std::path::Path, destino: &std::path::Path) -> Result<i64, ErrorMotor> {
    let w = respaldo::leer(wav).map_err(|_| ErrorMotor::Fichero {
        ruta: wav.display().to_string(),
    })?;
    if !MUESTREOS.contains(&w.muestreo) || !(1..=2).contains(&w.canales) {
        return Err(ErrorMotor::SalidaNoValida {
            canales: w.canales,
            muestreo: w.muestreo,
        });
    }
    #[cfg(windows)]
    {
        sistema::rehacer_m4a(wav, &w, destino)?;
        Ok(w.duracion_ms())
    }
    #[cfg(not(windows))]
    {
        let _ = destino;
        Err(ErrorMotor::SinCodificadorAac)
    }
}

/// Lo que puede salir mal.
///
/// Como en `pixpin-audio`, cada variante dice **que hacer**: «no hay
/// microfono» se le puede decir al usuario; un HRESULT suelto, no.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ErrorMotor {
    #[error("no se pidio ninguna fuente (ni microfono ni sonido del equipo)")]
    SinFuentes,
    #[error("no hay ningun microfono disponible (ni conectado, ni con permiso)")]
    SinMicrofono,
    #[error("no se puede oir lo que suena en el equipo (no hay altavoces ni cascos activos)")]
    SinSonidoDelEquipo,
    #[error(
        "este Windows no trae el codificador AAC (suele ser una edicion N sin el \
         paquete de caracteristicas multimedia)"
    )]
    SinCodificadorAac,
    #[error(
        "el aparato entrega un formato que no se sabe leer (etiqueta {etiqueta:#06x}, {bits} bits)"
    )]
    FormatoDesconocido { etiqueta: u16, bits: u16 },
    #[error(
        "la salida tiene que ser de 1 o 2 canales a 44 100 o 48 000 Hz (se pidio {canales} a {muestreo})"
    )]
    SalidaNoValida { canales: u16, muestreo: u32 },
    #[error("no se pudo crear {ruta}")]
    Fichero { ruta: String },
    #[error("un hilo del motor de audio se cayo")]
    HiloCaido,
    #[error("Windows fallo al {paso} (0x{codigo:08X})")]
    Windows { paso: &'static str, codigo: i32 },
}

/// Leer el respaldo WAV ([`respaldo::recorrer`]) es lo unico que da un
/// fallo de `std::io` al motor.
impl From<std::io::Error> for ErrorMotor {
    fn from(e: std::io::Error) -> ErrorMotor {
        ErrorMotor::Fichero {
            ruta: format!("el respaldo de la grabacion ({e})"),
        }
    }
}

impl From<formato::ErrorFormato> for ErrorMotor {
    fn from(e: formato::ErrorFormato) -> ErrorMotor {
        match e {
            formato::ErrorFormato::Desconocido { etiqueta, bits } => {
                ErrorMotor::FormatoDesconocido { etiqueta, bits }
            }
            formato::ErrorFormato::Desalineada { bits, .. } => {
                ErrorMotor::FormatoDesconocido { etiqueta: 0, bits }
            }
            formato::ErrorFormato::Corta | formato::ErrorFormato::Vacia => {
                ErrorMotor::FormatoDesconocido {
                    etiqueta: 0,
                    bits: 0,
                }
            }
        }
    }
}

/// Envuelve un fallo de Windows diciendo en que paso ocurrio.
#[cfg(windows)]
pub(crate) fn en(paso: &'static str) -> impl Fn(windows::core::Error) -> ErrorMotor {
    move |e| ErrorMotor::Windows {
        paso,
        codigo: e.code().0,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_codigo_de_windows_se_lee_en_hexadecimal() {
        let e = ErrorMotor::Windows {
            paso: "abrir el microfono",
            codigo: 0x8889_0004_u32 as i32,
        };
        assert_eq!(
            e.to_string(),
            "Windows fallo al abrir el microfono (0x88890004)"
        );
    }
}
