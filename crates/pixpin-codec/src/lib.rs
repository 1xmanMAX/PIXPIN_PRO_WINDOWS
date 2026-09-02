//! pixpin-codec — codificacion y decodificacion de imagen.
//!
//! Este crate habla con librerias C. El `unsafe` esta permitido, pero cada
//! bloque lleva su comentario `// SAFETY:`.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod imagen;
pub mod portapapeles;

pub use imagen::{ErrorCodec, FormatoImagen, ImagenRgba, cargar, codificar_png, guardar};
pub use portapapeles::{ContenidoPortapapeles, copiar_imagen, copiar_texto, leer};
