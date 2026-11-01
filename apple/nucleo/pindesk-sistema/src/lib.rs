//! pindesk-sistema — lo poco que los crates de logica tienen que preguntarle
//! al sistema operativo, en cualquier sistema operativo.
//!
//! Antes vivia en `pindesk-shell`, y por eso `pindesk-store` y
//! `pindesk-proyecto` arrastraban Win32 entero (y no compilaban fuera de
//! Windows) solo por saber la hora local, la carpeta de datos o crear una
//! union de directorio. Aqui cada funcion tiene su version de Windows, que
//! es la de siempre, y una portable para Mac, iPhone/iPad y el Linux de la
//! CI. Ver `docs/investigacion/2026-10-15-mac-ipad-iphone.md`, fase 0.
//!
//! `pindesk-shell` reexporta estos modulos con el mismo nombre
//! (`pindesk_shell::entorno`, `pindesk_shell::union`, `pindesk_shell::Atajo`),
//! asi que quien ya los usaba no cambia.
//!
//! Capa 0: no depende de ningun crate de PinDesk.

pub mod archivos;
pub mod atajo;
pub mod entorno;
pub mod union;

pub use atajo::{Atajo, ErrorAtajo, Modificadores, Tecla};
