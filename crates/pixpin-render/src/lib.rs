//! pixpin-render — el backend de dibujo Direct2D + DirectWrite.
//!
//! Frontera unica de dibujo del proyecto: todo lo que se pinta pasa por
//! aqui. Este crate habla con el sistema; `unsafe` permitido con `// SAFETY:`
//! en cada bloque. El bucle de render es dirigido por eventos, nunca por
//! fotogramas: sin trabajo no se dibuja nada, y de ahi sale el 0% de CPU.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod motor;

pub use motor::{Color, ErrorRender, MotorRender};

pub mod superficie;

pub use superficie::Superficie;
