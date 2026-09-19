//! El universo: los archivos del chat en un lienzo infinito.
//!
//! Diseno: `docs/superpowers/specs/2026-09-18-universo-design.md`.
//!
//! Es puro a proposito: ni Windows, ni Direct2D, ni el chat. Recibe los
//! archivos ya traducidos (`FichaLuna`) y devuelve que se ve y donde. Asi las
//! reglas de verdad —que puede ir dentro de que, que se pinta a cada zoom—
//! se prueban en milisegundos y sin ventana.

#![forbid(unsafe_code)]

pub mod astro;
pub mod detalle;
pub mod herramienta;
mod historia;
pub mod jerarquia;
pub mod universo;

pub use astro::{
    Astro, Clase, Conexion, IdAstro, RADIO_GALAXIA, RADIO_LUNA, RADIO_PLANETA_L, RADIO_PLANETA_M,
    RADIO_PLANETA_S, TipoConexion,
};
pub use detalle::{
    HISTERESIS, Nivel, Tipo, ZOOM_MINIMO_UNIVERSO, abre_hijos, nivel, nivel_con_memoria, tipo_de,
};
pub use herramienta::HerramientaUniverso;
pub use jerarquia::{Aterriza, Rechazo};
pub use universo::{Encuadre, Universo, VERSION};
