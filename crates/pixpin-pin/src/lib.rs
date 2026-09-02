//! pixpin-pin — los pines flotantes: el alma de PixPin (spec S2).
//!
//! Este crate habla con Win32; `unsafe` permitido con `// SAFETY:` por
//! bloque. La interaccion vive en `estado`, que es puro y se prueba sin
//! escritorio. La regla de capas prohibe depender de pixpin-store y de
//! pixpin-capture (misma capa L2): el almacen lo toca el ejecutable via
//! el callback CambioPin, y el dispositivo llega como &ID3D11Device.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod estado;
pub mod icono;
pub mod ventana;

pub use estado::{EfectoPin, EstadoPin, EventoPin, MINIMO_LOGICO, ZONA_ESQUINA_LOGICA};
pub use icono::{LADO_ICONO, icono_de};
pub use ventana::{
    CambioPin, ErrorPin, MARGEN_SOMBRA_LOGICO, Pin, contenido_desde_ventana, rect_ventana,
};
