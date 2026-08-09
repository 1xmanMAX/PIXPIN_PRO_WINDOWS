//! pixpin-shell — ver docs/superpowers/specs/2026-08-09-pixpin-pc-master-design.md
//!
//! Este crate habla con el sistema operativo o con librerias C. El `unsafe`
//! esta permitido, pero cada bloque lleva su comentario `// SAFETY:`.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod atajo;
pub mod entorno;
pub mod instancia;

pub use atajo::{Atajo, ErrorAtajo, Modificadores, Tecla};
pub use entorno::{appdata, directorio_del_ejecutable, locale_del_sistema};
pub use instancia::{ErrorInstanciaUnica, InstanciaUnica, adquirir_instancia_unica};
