//! El motor de edicion avanzada en 2D de PixPin Max.
//!
//! Dibujo vectorial a mano alzada, sin interfaz: lo usan el pin (anotar
//! dentro), la pantalla (anotar encima) y, mas adelante, el PDF. Por eso es un
//! crate propio y no codigo dentro del pin — tres consumidores muy distintos,
//! una sola verdad sobre que es un trazo.
//!
//! Diseno: `docs/superpowers/specs/2026-09-02-s3a-motor2d-design.md`.
//! Los algoritmos se estudiaron de Excalidraw (MIT) y estan documentados en
//! `docs/investigacion/2026-09-02-excalidraw-analisis.md`; la implementacion
//! es nueva.
//!
//! Casi todo es puro y se prueba sin escritorio: solo el modulo de dibujo
//! toca Direct2D.

#![forbid(unsafe_code)]

pub mod azar;
pub mod trazo;
pub mod vector;

pub use azar::Azar;
pub use trazo::{Ajustes, PuntoTrazo, contorno, linea_central, poligono};
pub use vector::{Punto2, distancia_a_segmento};
