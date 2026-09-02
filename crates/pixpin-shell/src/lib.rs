//! pixpin-shell — ver docs/superpowers/specs/2026-08-09-pixpin-pc-master-design.md
//!
//! Este crate habla con el sistema operativo o con librerias C. El `unsafe`
//! esta permitido, pero cada bloque lleva su comentario `// SAFETY:`.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod abrir;
pub mod arranque;
pub mod atajo;
pub mod atajos;
pub mod bandeja;
pub mod dialogo;
pub mod entorno;
pub mod guardar;
pub mod hechos;
pub mod instancia;
pub mod overlay;
pub mod uia;
pub mod ventana;

pub use abrir::{abrir, abrir_ubicacion};
pub use arranque::ErrorArranque;
pub use atajo::{Atajo, ErrorAtajo, Modificadores, Tecla};
pub use atajos::{
    AtajosRegistrados, ID_COPIAR, ID_CUENTAGOTAS, ID_PIN, ID_PORTAPAPELES, ID_REGION, ID_SCROLL,
    registrar,
};
pub use bandeja::{Bandeja, EtiquetasMenu};
pub use dialogo::{confirmar_destructivo, mostrar_error_fatal};
pub use entorno::{
    appdata, directorio_del_ejecutable, locale_del_sistema, posicion_del_cursor, tema_claro,
};
pub use instancia::{ErrorInstanciaUnica, InstanciaUnica, adquirir_instancia_unica};
pub use ventana::{
    Continuar, Evento, ID_MENU_AJUSTES, ID_MENU_CAPTURAR, ID_MENU_SALIR, VentanaMensajes,
    WM_BANDEJA, despertar,
};
