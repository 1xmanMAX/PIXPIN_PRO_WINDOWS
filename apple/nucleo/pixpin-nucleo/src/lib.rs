//! **El puente entre el nucleo de Rust y la app de Apple** (fase 1 del plan,
//! `docs/investigacion/2026-10-15-mac-ipad-iphone.md`).
//!
//! Una API pequena, estable y de grano grueso sobre los crates de
//! `apple/nucleo/`, expuesta a Swift con UniFFI (macros, sin `.udl`). No
//! inventa nada: el formato, el cuaderno y la sincronizacion son los mismos
//! crates que usa Windows, para que un Mac sea un aparato mas del grupo.
//!
//! # La regla de rendimiento (§4bis del plan)
//!
//! **Cruzar el puente pocas veces y con mucho dentro.** Cada llamada de Swift
//! a Rust copia sus argumentos y su resultado (UniFFI los serializa en un
//! `RustBuffer`), asi que:
//!
//! - Nada de un objeto por figura ni por mensaje. El lienzo cruza como **un
//!   solo bufer plano** de `f32` (ver [`lienzo`]): una llamada por escena o
//!   por fotograma, y Swift construye sus `CGPath` en una pasada.
//! - Un fichero entra al chat con [`Nucleo::adjuntar`] (se copia, no se
//!   lee entero en memoria) y su ruta en el disco sale de
//!   [`Nucleo::ruta_absoluta`]; las fotos de un lienzo, todas juntas, de
//!   [`Nucleo::imagenes_del_lienzo`] (ver [`adjuntos`]).
//! - Los chats se leen **por paginas** ([`Nucleo::mensajes`]): nunca el chat
//!   entero. El indice de lineas de cada cuaderno se guarda en una cache con
//!   tope ([`chats`]) y se vacia con el aviso de memoria del sistema
//!   ([`Nucleo::vaciar_caches`]).
//! - Lo que tarda (sincronizar, leer un lienzo grande) es **bloqueante y sin
//!   hilos propios**: Swift lo llama desde una `Task` con prioridad
//!   `.utility`, y el sistema lo manda a los nucleos de eficiencia. Rust no
//!   arranca hilos ni temporizadores que nadie pidio.
//!
//! # Panicos
//!
//! UniFFI envuelve cada llamada en `catch_unwind`: con `panic = "unwind"`
//! un panico de Rust llega a Swift como error y la app sigue viva. El
//! perfil `release` del espacio de trabajo usa `panic = "abort"` (el de
//! Windows, que es un ejecutable entero en Rust): ahi un panico cerraria la
//! app de Apple sin decir nada. Por eso el xcframework se compila con el
//! perfil `ffi` (`apple/Cargo.toml`), que hereda de `release` y solo cambia
//! eso. Ver `herramientas/xcframework.sh`.

uniffi::setup_scaffolding!();

pub mod adjuntos;
pub mod chats;
pub mod editor;
pub mod lienzo;
pub mod sincro;
pub mod voz;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub use chats::{FichaChat, MensajeChat, PaginaMensajes};
pub use editor::{EditorLienzo, FasePuntero};
pub use lienzo::{CajaLienzo, ImagenDelLienzo, Lienzo, LienzoPlano};
pub use sincro::{AnuncioBonjour, Atendido, Escucha, ParTxt, ResumenSincro};
pub use voz::SegmentoVoz;

/// Lo que puede salir mal, tal como lo ve Swift (un `enum` que `throws`).
///
/// Pocas variantes y con el detalle en texto: Swift decide que ensenar por
/// la variante, y el texto es para el registro.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ErrorNucleo {
    /// Leer o escribir en el disco.
    #[error("disco: {detalle}")]
    Disco { detalle: String },
    /// Lo pedido no esta (un chat, un lienzo).
    #[error("no existe: {que}")]
    NoExiste { que: String },
    /// Un `.excalidraw` que no se entiende.
    #[error("lienzo: {detalle}")]
    Lienzo { detalle: String },
    /// La red o el protocolo de sincronizar.
    #[error("sincronizar: {detalle}")]
    Sincro { detalle: String },
    /// Un argumento que no vale (un codigo de grupo mal escrito...).
    #[error("argumento: {detalle}")]
    Argumento { detalle: String },
}

impl From<std::io::Error> for ErrorNucleo {
    fn from(e: std::io::Error) -> Self {
        ErrorNucleo::Disco {
            detalle: e.to_string(),
        }
    }
}

impl From<pixpin_sincro::protocolo::ErrorSincro> for ErrorNucleo {
    fn from(e: pixpin_sincro::protocolo::ErrorSincro) -> Self {
        ErrorNucleo::Sincro {
            detalle: e.to_string(),
        }
    }
}

/// Quien es este aparato.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct InfoAparato {
    /// El id al azar, fijo para siempre. No se ensena.
    pub id: String,
    pub nombre: String,
    /// Los cuatro signos fijos (`K7Q2`) que lleva cada mensaje nacido aqui.
    pub codigo: String,
    /// La letra en el grupo, si esta en uno.
    pub letra: Option<String>,
    /// El codigo del grupo, si esta en uno.
    pub grupo: Option<String>,
}

/// **Una carpeta de datos de PixPin abierta**: el objeto con el que trabaja
/// Swift. Es barato de crear y se puede usar desde varios hilos.
#[derive(uniffi::Object)]
pub struct Nucleo {
    raiz: PathBuf,
    /// El nombre del aparato si hay que crear la identidad (lo pone Swift:
    /// `Host.current().localizedName` o `UIDevice.current.name`).
    nombre: String,
    pub(crate) cache: Mutex<chats::Cache>,
}

#[uniffi::export]
impl Nucleo {
    /// Abre (o crea) la carpeta de datos `raiz`. Si no tiene identidad, nace
    /// una con `nombre_del_aparato`: el formato es el de Android y Windows
    /// (`sincro/identidad.json`), asi que una carpeta copiada de Windows se
    /// abre tal cual y conserva su identidad.
    #[uniffi::constructor]
    pub fn abrir(raiz: String, nombre_del_aparato: String) -> Result<Arc<Nucleo>, ErrorNucleo> {
        let raiz = PathBuf::from(raiz);
        std::fs::create_dir_all(&raiz)?;
        pixpin_proyecto::identidad::Identidad::leer_o_crear(&raiz, &nombre_del_aparato)?;
        Ok(Arc::new(Nucleo {
            raiz,
            nombre: nombre_del_aparato,
            cache: Mutex::new(chats::Cache::default()),
        }))
    }

    /// La carpeta de datos.
    pub fn raiz(&self) -> String {
        self.raiz.display().to_string()
    }

    /// Quien es este aparato y en que grupo esta.
    pub fn aparato(&self) -> Result<InfoAparato, ErrorNucleo> {
        let i = self.identidad()?;
        Ok(InfoAparato {
            codigo: i.yo.codigo(),
            id: i.yo.id,
            nombre: i.yo.nombre,
            letra: i.yo.letra,
            grupo: i.codigo,
        })
    }

    /// Suelta las caches (indices de los cuadernos). Para el aviso de
    /// memoria del sistema (`didReceiveMemoryWarning`,
    /// `DispatchSource.makeMemoryPressureSource`): se rehacen solas al
    /// volver a leer.
    pub fn vaciar_caches(&self) {
        self.cache_tomada().vaciar();
    }
}

impl Nucleo {
    /// Cuantos chats tienen su indice en la cache (para comprobar el tope).
    pub fn chats_en_cache(&self) -> usize {
        self.cache_tomada().cuantos()
    }

    pub(crate) fn ruta(&self) -> &Path {
        &self.raiz
    }

    pub(crate) fn identidad(&self) -> Result<pixpin_proyecto::identidad::Identidad, ErrorNucleo> {
        Ok(pixpin_proyecto::identidad::Identidad::leer_o_crear(
            &self.raiz,
            &self.nombre,
        )?)
    }

    /// La cache, aunque un hilo muriera con ella tomada: es solo una cache.
    pub(crate) fn cache_tomada(&self) -> std::sync::MutexGuard<'_, chats::Cache> {
        self.cache.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// La hora de ahora en milisegundos desde 1970, como la de todo el formato.
pub(crate) fn ahora_ms() -> i64 {
    pixpin_sistema::entorno::ahora_utc_ms()
}
