//! **Pasar una nota de voz a texto, en este ordenador y sin red.**
//!
//! Es el puerto de `guardados/Transcriptor.kt` del movil, y tambien el del
//! telepronter (`guardados/TelepronterActivity.kt`), que es lo que el movil
//! llama «leer en voz alta». Las dos cosas viven juntas porque comparten lo
//! unico que de verdad tienen en comun: el cuerpo con marcas de tiempo
//! (`[1:23] lo que se dijo`) que acaba en `Mensaje.transcripcion` y viaja
//! por la sincronizacion.
//!
//! ## Dos correcciones que hay que tener presentes
//!
//! 1. **La transcripcion del movil NO es la nube.** Los tres motores corren
//!    en el aparato; la red solo baja modelos. Hasta el llamado «Google»
//!    usa `createOnDeviceSpeechRecognizer` con `EXTRA_PREFER_OFFLINE`
//!    (`MotorGoogle.kt:78, 170`).
//! 2. **«Leer en voz alta» no es voz sintetica.** En todo PixPin Android no
//!    hay ni un `TextToSpeech`. Significa que el usuario lee mientras el
//!    telefono graba: telepronter y pronunciar. Aqui no se sintetiza nada.
//!
//! ## Por que Vosk, y no lo que trae Windows ni Whisper
//!
//! Se miraron los tres caminos:
//!
//! - **`Windows.Media.SpeechRecognition`** (viene con el sistema, gratis).
//!   **Queda descartado por una razon tecnica, no de gusto: no sabe leer un
//!   fichero.** Su `SpeechRecognizer` oye el microfono por defecto y nada
//!   mas —ni `RecognizeAsync` ni `SpeechContinuousRecognitionSession`
//!   admiten una fuente de audio—, asi que transcribir un `.m4a` guardado
//!   obligaria a reproducirlo por los altavoces y volverlo a oir, o a
//!   montar un cable de audio virtual. Ademas exige que el usuario instale
//!   el paquete de voz del idioma desde los ajustes de Windows, y devuelve
//!   la frase sin el tiempo de cada palabra, que es justo lo que hace falta
//!   para escribir `[1:23]`.
//! - **sherpa-onnx o whisper.cpp** (Apache-2.0 / MIT). Dan buen texto y con
//!   puntuacion, pero: son C++ que hay que **enlazar al compilar**, o sea
//!   cmake, bindgen y onnxruntime en el arbol, y sin ese aparejo no
//!   compilaria ni `cargo test`; los modelos pesan de 104 a 375 MB frente a
//!   los 40 de Vosk; y Whisper **se inventa frases** cuando le llega
//!   silencio —el movil arrastra una lista negra entera contra eso
//!   (`Transcriptor.kt:284-324`)—.
//! - **Vosk** (Kaldi). Es **el motor por omision del movil**
//!   (`Transcriptor.kt:231`), con **los mismos modelos**
//!   (`vosk-model-small-es-0.42` y hermanos, `MotorVosk.kt:32-40`), asi que
//!   el mismo audio da **el mismo texto en los dos aparatos**, que era el
//!   objetivo. Da el tiempo de cada palabra de serie (`setWords`). Y trae
//!   un `libvosk.dll` ya compilado, de modo que aqui **se carga al vuelo
//!   con `LoadLibraryW`** en vez de enlazarlo: el repositorio no gana ni
//!   una dependencia de compilacion, `cargo build` y `cargo test` siguen
//!   funcionando en una maquina donde no haya nada de esto instalado, y no
//!   viaja un solo byte de modelo dentro del ejecutable.
//!
//! Licencias, que es la otra puerta: `vosk-api` es **Apache-2.0**, Kaldi es
//! **Apache-2.0** y los modelos *small* que usa el movil son **Apache-2.0**.
//! Nada de GPL ni AGPL entra por aqui, y como no hay dependencia de cargo
//! nueva, `cargo-deny` no tiene nada nuevo que mirar.
//!
//! Lo que se pierde con Vosk y conviene saber: los modelos pequenos
//! escriben **sin mayusculas ni puntuacion**. El movil vive con ello desde
//! el primer dia; el texto se lee igual y se busca mejor.
//!
//! ## Lo que hay que instalar (una vez, y lo hace el usuario)
//!
//! Ver [`motor`] y [`idiomas`]: un `libvosk.dll` y la carpeta del modelo
//! del idioma, las dos dentro de `<datos>/vosk/`. Este crate **no descarga
//! nada**: dice lo que falta y el enlace exacto, y quien lo baja es el
//! usuario.
//!
//! ## Reparto del crate
//!
//! - [`tiempos`], [`credibilidad`], [`idiomas`] y [`telepronter`]:
//!   aritmetica y texto, sin Windows y sin sonido. Es donde estan las
//!   pruebas de verdad.
//! - [`pcm`] y [`motor`]: lo que toca el sistema operativo y la libreria en
//!   C, con su `unsafe` acotado y comentado.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod credibilidad;
pub mod idiomas;
pub mod telepronter;
pub mod tiempos;

#[cfg(windows)]
pub mod motor;
#[cfg(windows)]
pub mod pcm;

pub use credibilidad::creible;
pub use idiomas::{Disponibilidad, IDIOMAS, carpeta_de_modelos, enlace_del_modelo, modelo_de};
pub use telepronter::{Desfile, parrafos_de};
pub use tiempos::{EstadoDelTexto, Segmento, con_tiempos, marca_de_tiempo, tiempo_de};

#[cfg(windows)]
pub use motor::{Transcripcion, transcribir};

/// A cuantas muestras por segundo oye el reconocedor.
///
/// No es negociable: los modelos de Vosk estan entrenados a 16 kHz
/// (`Transcriptor.HERCIOS`, `Transcriptor.kt:32`). Todo lo que entra se
/// remuestrea a esto antes de llegar al motor.
pub const HERCIOS: u32 = 16_000;

/// Bytes de PCM de 16 bits mono que ocupa un segundo de audio.
pub const BYTES_POR_SEGUNDO: usize = HERCIOS as usize * 2;

/// Lo que puede salir mal al pasar una nota a texto.
///
/// Cada variante dice **que hacer**, no solo que fallo, porque las dos
/// causas normales —falta el motor, falta el idioma— se arreglan con una
/// descarga y el usuario tiene que enterarse de cual. Misma regla que
/// `pixpin_audio::ErrorAudio`.
#[derive(Debug, thiserror::Error)]
pub enum ErrorVoz {
    #[error(
        "falta el reconocedor de voz: no se encontro `libvosk.dll` en {donde} \
         ni junto al ejecutable"
    )]
    SinMotor { donde: String },
    #[error(
        "no hay modelo de voz para «{idioma}»; hay que descomprimir {modelo} \
         dentro de {donde}"
    )]
    SinModelo {
        idioma: String,
        modelo: String,
        donde: String,
    },
    #[error("Vosk no tiene modelo de voz pequeno para «{idioma}»")]
    IdiomaSinModelo { idioma: String },
    #[error(
        "la carpeta del modelo {donde} lleva caracteres que el reconocedor no \
         sabe abrir; mueve la carpeta a una ruta sin tildes ni acentos"
    )]
    RutaImposible { donde: String },
    #[error("el modelo de {donde} esta incompleto o no se pudo cargar")]
    ModeloIlegible { donde: String },
    #[error("no se pudo leer {ruta}: no parece un audio que Windows sepa abrir")]
    NoEsAudio { ruta: String },
    #[error("el audio esta vacio: no hay nada que transcribir")]
    AudioVacio,
    /// El motor corrio entero y no saco ni una palabra creible. No es un
    /// fallo del programa: es una nota en la que no se entiende nada, y el
    /// movil tambien la distingue (`Transcriptor.NADA`).
    #[error("no se entendio nada de lo que se dice en el audio")]
    NoSeEntiendeNada,
    #[error("se cancelo la transcripcion")]
    Cancelada,
    #[error("Windows fallo al {paso}: {fuente}")]
    Windows {
        paso: &'static str,
        #[source]
        fuente: windows::core::Error,
    },
}

/// Envuelve un fallo de Windows diciendo en que paso ocurrio.
#[cfg(windows)]
pub(crate) fn en(paso: &'static str) -> impl Fn(windows::core::Error) -> ErrorVoz {
    move |fuente| ErrorVoz::Windows { paso, fuente }
}
