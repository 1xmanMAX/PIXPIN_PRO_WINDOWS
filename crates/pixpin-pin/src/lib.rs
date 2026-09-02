//! pixpin-pin — los pines flotantes: el alma de PixPin (spec S2).
//!
//! Este crate habla con Win32; `unsafe` permitido con `// SAFETY:` por
//! bloque. La interaccion vive en `estado`, que es puro y se prueba sin
//! escritorio. La regla de capas prohibe depender de pixpin-store y de
//! pixpin-capture (misma capa L2): el almacen lo toca el ejecutable via
//! el callback CambioPin, y el dispositivo llega como &ID3D11Device.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod contenido;
pub mod estado;
pub mod icono;
pub mod menu;
pub mod paleta;
pub mod ventana;

pub use contenido::{
    Contenido, DOCUMENTO_FRANJA_LOGICA, FICHA_ALTO_LOGICO, FICHA_ANCHO_LOGICO, Presentacion,
    presentacion_de, tamano_humano, tamano_natural,
};
pub use estado::{EfectoPin, EstadoPin, EventoPin, MINIMO_LOGICO, ZONA_ESQUINA_LOGICA};
pub use icono::{LADO_ICONO, icono_de};
pub use menu::{
    CMD_ABRIR_UBICACION, CMD_CERRAR, CMD_COLOR_BASE, CMD_COPIAR, CMD_ELIMINAR, CMD_GUARDAR_COMO,
    CMD_OCULTAR_GRUPO, CMD_REPRODUCIR, CMD_SIN_GRUPO, CMD_SONIDO, CMD_TAMANO_ORIGINAL, TextosPin,
};
pub use paleta::{Paleta, PintorPaleta};
pub use ventana::{
    CambioPin, ErrorPin, LupaPin, MARGEN_SOMBRA_LOGICO, Pin, contenido_desde_ventana, rect_ventana,
};
