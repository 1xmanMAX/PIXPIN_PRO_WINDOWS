//! pixpin-ui — la interaccion como logica pura.
//!
//! Nada de este crate llama a Win32. Dibuja a traves de `pixpin-render` y
//! recibe eventos ya traducidos, de modo que el comportamiento del overlay
//! completo se prueba en milisegundos y sin escritorio. Si algun dia este
//! crate necesitase Win32, la frontera se ha roto: arreglar el diseno, no
//! relajar la regla.
#![forbid(unsafe_code)]

pub mod barra;
pub mod lupa;
pub mod overlay;

pub use barra::{AccionBarra, Barra};
pub use lupa::{FormatoColorLupa, Lupa, texto_color};
pub use overlay::{Efecto, EstadoOverlay, EventoEntrada, Fase, FormaCursor, TeclaOverlay};
