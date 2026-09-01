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
pub mod instantanea;
pub mod mapa;
pub mod monitores;

pub use dispositivo::{Dispositivo, ErrorCaptura};
pub use instantanea::{Instantanea, capturar_monitor};
pub use mapa::a_imagen;
pub use monitores::{enumerar_monitores, handle_de_monitor};
