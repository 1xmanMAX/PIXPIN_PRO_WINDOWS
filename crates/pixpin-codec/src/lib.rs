//! pixpin-codec — codificacion y decodificacion de imagen.
//!
//! Este crate habla con librerias C. El `unsafe` esta permitido, pero cada
//! bloque lleva su comentario `// SAFETY:`.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod imagen;
pub mod portapapeles;

pub use imagen::{ErrorCodec, FormatoImagen, ImagenRgba, guardar};
pub use portapapeles::copiar_imagen;
