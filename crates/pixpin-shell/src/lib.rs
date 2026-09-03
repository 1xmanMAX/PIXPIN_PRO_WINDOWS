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
pub mod entrada;
pub mod gestos;
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
    AtajosRegistrados, ID_ANOTAR, ID_ANOTAR_CONGELADA, ID_COPIAR, ID_CUENTAGOTAS, ID_PIN,
    ID_PORTAPAPELES, ID_REGION, ID_SCROLL, registrar,
};
pub use bandeja::{Bandeja, EtiquetasMenu};
pub use dialogo::{confirmar_destructivo, mostrar_error_fatal, preguntar};
pub use entorno::{
    appdata, directorio_del_ejecutable, locale_del_sistema, posicion_del_cursor, tema_claro,
};
pub use entrada::{escape_pulsado, modificadores, rueda_en};
pub use gestos::{GanchoRaton, gesto_en_curso};
pub use instancia::{ErrorInstanciaUnica, InstanciaUnica, adquirir_instancia_unica};
pub use overlay::esperar_composicion;
pub use ventana::{
    BotonGesto, Continuar, Evento, VentanaMensajes, WM_BANDEJA, WM_GESTO, despertar,
};
