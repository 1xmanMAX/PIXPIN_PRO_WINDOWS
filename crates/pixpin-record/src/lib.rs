//! pixpin-record — ver docs/superpowers/specs/2026-08-09-pixpin-pc-master-design.md
//!
//! Este crate habla con el sistema operativo o con librerias C. El `unsafe`
//! esta permitido, pero cada bloque lleva su comentario `// SAFETY:`.
//!
//! - [`escala`]: las cuentas del video en vivo (tamano, encoger, tiempos),
//!   puras y probadas en cualquier maquina.
//! - `mp4` y `mp4_vivo`: lo que toca Media Foundation, solo en Windows.
//!   `mp4` es la exportacion de GIF a MP4; `mp4_vivo` (BGRA por la CPU y
//!   codificador por software) es el ultimo recurso del motor de video
//!   (`pixpin-motor-video`, ver docs/motor-de-video.md).
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod escala;
#[cfg(windows)]
mod mp4;
#[cfg(windows)]
mod mp4_vivo;

#[cfg(windows)]
pub use mp4::{ErrorMp4, OpcionesMp4, codificar_mp4};
#[cfg(windows)]
pub use mp4_vivo::{Codificador, OpcionesVivo};
