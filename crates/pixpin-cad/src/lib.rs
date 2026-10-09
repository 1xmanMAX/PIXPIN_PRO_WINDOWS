//! pixpin-cad — ver planos DWG y DXF en una ventana como un pin, fluido
//! aunque el plano sea grande y el equipo modesto.
//!
//! - [`leer`]: abrir el fichero con `opencadcodec` (MPL-2.0).
//! - [`convertir`]: del plano a rayas y triangulos ([`modelo::Modelo`]).
//! - [`modelo`]: lo que va a la tarjeta grafica, ordenado para dibujar solo
//!   lo que se ve, y su cache en disco.
pub mod convertir;
pub mod convertir3d;
pub mod gpu;
pub mod gpu3d;
pub mod leer;
pub mod modelo;
pub mod modelo3d;
pub mod proxy;
pub mod regla;
pub mod anotado;
pub mod ventana3d;
pub mod relleno;
pub mod shx;
pub mod teselar;
pub mod texto;
pub mod ventana;
