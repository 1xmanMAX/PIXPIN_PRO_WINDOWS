//! pixpin-capture — captura de pantalla por GPU.
//!
//! Este crate habla con el sistema operativo. El `unsafe` esta permitido,
//! pero cada bloque lleva su comentario `// SAFETY:`.
//!
//! **Principio que sostiene todo el presupuesto de rendimiento:** la imagen
//! vive en una textura de la GPU de principio a fin y solo baja a memoria de
//! sistema cuando el usuario guarda o copia. Una captura 4K son 33 MB; bajarla
//! en cada operacion es lo que hace lentas y glotonas a las herramientas de
//! captura corrientes.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod dispositivo;

pub use dispositivo::{Dispositivo, ErrorCaptura};
