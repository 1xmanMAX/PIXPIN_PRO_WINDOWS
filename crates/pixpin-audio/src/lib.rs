//! **Oir y grabar**: las notas de voz de PixPin en Windows.
//!
//! El chat del PC ya pintaba la onda de una nota de voz con las mismas
//! constantes que el movil —50 barras, paso 3— pero no sonaba nada, y no se
//! podia grabar. Este crate pone las dos mitades que faltaban.
//!
//! ## Por que Media Foundation para oir, y WASAPI para grabar
//!
//! Para **oir** se usa `IMFMediaEngine`, el mismo motor que ya reproduce el
//! video de un pin (`pixpin-pin/src/video.rs`), creado esta vez con
//! `MF_MEDIA_ENGINE_AUDIOONLY`. Es la eleccion barata y la correcta:
//!
//! - El `.m4a` del movil es AAC dentro de MPEG-4, y Media Foundation lo
//!   decodifica con el codec del sistema. La alternativa —`symphonia` para
//!   AAC mas `cpal` para sacarlo— son dos dependencias externas grandes,
//!   mas un hilo de mezcla propio, para hacer lo que Windows ya trae.
//! - Trae **de fabrica** las cuatro cosas que pide el movil y que un
//!   reproductor a mano cuesta escribir bien: `SetCurrentTime` (saltar),
//!   `GetCurrentTime` (por donde va), `GetDuration` y —la dificil—
//!   `SetPlaybackRate`, que **cambia la velocidad sin cambiar el tono**.
//!   Reproducir a `1,5x` remuestreando a pelo convierte una voz en un pato;
//!   el movil usa `PlaybackParams.setSpeed`, que tampoco cambia el tono, y
//!   esto es su equivalente exacto.
//! - Ya hay un precedente probado en la casa, con su arranque de MF y su
//!   `Shutdown`, y no se anaden dependencias al arbol.
//!
//! Para **grabar** no vale: `IMFMediaEngine` solo reproduce. Se captura con
//! **WASAPI** (`IAudioClient` en modo compartido) porque es lo unico que da
//! el PCM crudo, que es de donde salen los **picos** de la onda: el `.m4a`
//! ya escrito es AAC comprimido y sacarle las amplitudes obligaria a
//! descodificarlo entero, que es justo lo que el movil evita capturandolos
//! al grabar (`pin/Voz.kt:87-98`). Ese PCM se escribe a `.m4a` con
//! `IMFSinkWriter` y el codificador AAC del sistema, igual que
//! `pixpin-record` escribe el MP4 del video.
//!
//! ## Que queda igual que en el movil, y que no
//!
//! Igual: contenedor **MPEG-4** (`.m4a`), codec **AAC-LC**, **mono**, los
//! picos crudos de 0 a 32767 con tope 256 uno cada 50 ms, `duracionMs`
//! entero, el minimo de 700 ms y las cinco velocidades.
//!
//! Distinto, y no por gusto: el movil graba a **22 050 Hz y 32 000 bps**
//! (`Voz.kt:140-141`) y el codificador AAC de Windows **no admite ninguno de
//! los dos**: solo 44 100 o 48 000 Hz y 96/128/160/192 kbps. Se graba al
//! muestreo que ya tenga la tarjeta —48 000 casi siempre— y al bitrate mas
//! bajo que acepta. El fichero pesa mas que uno del movil; suena igual o
//! mejor, y **el movil lo reproduce sin enterarse**, porque lo que lee es el
//! contenedor y el codec, no los ajustes con que se escribio.
//!
//! ## Reparto del crate
//!
//! - [`reloj`] y [`picos`] y [`mezcla`]: aritmetica pura, sin Windows y sin
//!   sonido. Es donde estan las pruebas de verdad.
//! - [`salida`] y [`entrada`]: lo que toca el sistema operativo, con su
//!   `unsafe` acotado y comentado.

pub mod mezcla;
pub mod picos;
pub mod reloj;

#[cfg(windows)]
pub mod entrada;
#[cfg(windows)]
pub mod salida;

pub use picos::{MINIMO_MS, MS_ENTRE_PICOS, PICO_MAXIMO, PICOS_GUARDADOS, Picos, Reparto};
pub use reloj::{
    Estado, MS_ENTRE_LATIDOS, SALTO_MS, VELOCIDADES, destino_al_saltar, destino_de_fraccion,
    duracion_legible, fraccion, siguiente_velocidad, velocidad_legible,
};

#[cfg(windows)]
pub use entrada::{Grabacion, Grabadora};
#[cfg(windows)]
pub use salida::Salida;

/// Lo que puede salir mal al oir o al grabar.
///
/// Cada variante dice **que hacer**, no solo que fallo: los HRESULT de
/// Media Foundation son todos el mismo `0x80004005` sin contexto, y depurar
/// una tuberia de seis pasos con eso es perder la tarde. Es la misma regla
/// que sigue `pixpin-record::ErrorMp4`.
#[derive(Debug, thiserror::Error)]
pub enum ErrorAudio {
    #[error("no hay ningun microfono disponible (ni conectado, ni con permiso)")]
    SinMicrofono,
    #[error(
        "este Windows no trae el codificador AAC (suele ser una edicion N sin el \
         paquete de caracteristicas multimedia)"
    )]
    SinCodificadorAac,
    #[error(
        "el microfono entrega muestras de {bits} bits, que no se sabe convertir; \
         se esperaban 16 o 32"
    )]
    FormatoDesconocido { bits: u16 },
    #[error("no se pudo leer {ruta}: no parece un audio que Windows sepa abrir")]
    NoEsAudio { ruta: String },
    #[error("la grabacion duro {duro_ms} ms y hacen falta al menos {minimo_ms}")]
    DemasiadoCorta { duro_ms: i64, minimo_ms: i64 },
    #[error("la grabacion ya se habia parado")]
    YaParada,
    #[error("el hilo que grababa se cayo")]
    HiloCaido,
    #[error("Windows fallo al {paso}: {fuente}")]
    Windows {
        paso: &'static str,
        #[source]
        fuente: windows::core::Error,
    },
}

/// Envuelve un fallo de Windows diciendo en que paso ocurrio.
#[cfg(windows)]
pub(crate) fn en(paso: &'static str) -> impl Fn(windows::core::Error) -> ErrorAudio {
    move |fuente| ErrorAudio::Windows { paso, fuente }
}
