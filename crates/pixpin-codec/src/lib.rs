//! pixpin-codec — codificacion y decodificacion de imagen.
//!
//! Este crate habla con librerias C. El `unsafe` esta permitido, pero cada
//! bloque lleva su comentario `// SAFETY:`.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod cosido;
pub mod gif;
pub mod imagen;
pub mod portapapeles;

pub use cosido::{
    Cosedor, Orden, Plan, Resultado, SIN_ENCAJE, encontrar_desplazamiento, es_lisa, firmas,
    franjas_fijas,
};
pub use gif::{ErrorGif, OpcionesGif, codificar as codificar_gif};
pub use imagen::{ErrorCodec, FormatoImagen, ImagenRgba, cargar, codificar_png, guardar};
pub use portapapeles::{ContenidoPortapapeles, copiar_imagen, copiar_texto, leer};
