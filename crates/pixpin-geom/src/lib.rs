//! pixpin-geom — geometria pura, sin una sola llamada al sistema operativo.
//!
//! Aqui vive la aritmetica donde de verdad se cometen errores: el escritorio
//! virtual con DPI mixto, la normalizacion del arrastre, la resolucion del
//! ajuste automatico. Al no depender de Windows ni de la GPU, se prueba en
//! milisegundos y con disposiciones de monitores inventadas que seria
//! carisimo reproducir en hardware.
#![forbid(unsafe_code)]

pub mod punto;
pub mod rect;

pub use punto::Punto;
pub use rect::Rect;
