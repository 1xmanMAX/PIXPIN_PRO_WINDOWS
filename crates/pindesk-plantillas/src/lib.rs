//! **PinDesk Plantillas: el nucleo portable.**
//!
//! Diseñar plantillas (diarios, planificadores, calendarios, listas, notas
//! Cornell, actas…) por bloques y sacarlas a PDF, PNG o a PinDesk. Este
//! crate es todo lo que no es ventana: el modelo, las fechas en catorce
//! idiomas, la repeticion (una pagina por dia de 2027), la maquetacion como
//! funcion pura, la edicion con deshacer, la galeria de ejemplos, la IA con
//! JSON validado y los exportadores. Sin Windows: Android y Apple lo pueden
//! usar tal cual. La app de Windows esta en `apps/pindesk-plantillas`.
//!
//! El documento completo, el formato y como se separa a su repositorio:
//! `docs/plantillas.md`.

pub mod campos;
pub mod carpeta;
pub mod diseno;
pub mod edicion;
pub mod enlace;
pub mod exportar;
pub mod fecha;
pub mod formato;
pub mod galeria;
pub mod ia;
pub mod locale;
pub mod modelo;
pub mod paleta;
pub mod prim;
pub mod tema;
pub mod texto;
pub mod textos;

/// **El nombre del producto**, en un solo sitio (el nombre definitivo aun no
/// esta decidido: cambiarlo es cambiar esto y `plt-producto` en `i18n/`).
pub const NOMBRE_PRODUCTO: &str = "PinDesk Plantillas";
/// El nombre del ejecutable (sin `.exe`).
pub const EJECUTABLE: &str = "pindesk-plantillas";

pub use fecha::Fecha;
pub use modelo::{Bloque, Documento, Tipo};
