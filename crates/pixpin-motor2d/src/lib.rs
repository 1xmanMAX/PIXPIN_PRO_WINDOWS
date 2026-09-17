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

pub mod aligerar;
pub mod azar;
pub mod cache;
pub mod camara;
pub mod elemento;
pub mod enganche;
pub mod escalabarra;
pub mod escena;
pub mod estilo;
pub mod excalidraw;
pub mod forma_rapida;
pub mod formas;
pub mod formato;
pub mod gesto;
pub mod impacto;
pub mod indice;
pub mod medida;
pub mod organizar;
pub mod pintado;
pub mod portapapeles;
pub mod relleno;
pub mod seleccion;
pub mod texto;
pub mod tinta;
pub mod tiradores;
pub mod transformar;
pub mod vector;

pub use aligerar::aligerar;
pub use azar::Azar;
pub use camara::{Camara, cuantos_se_ven, recortar};
pub use elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
pub use escalabarra::Barra;
pub use escena::Escena;
pub use formas::{elipse, linea, punta_flecha, rectangulo};
pub use formato::{EXTENSION, ErrorFormato, cargar, guardar};
pub use impacto::{TOLERANCIA, dentro_de, elemento_en, elementos_en, esquinas_giradas, toca};
pub use medida::{
    Escala, longitud_de, medida_de, rotulo_del_reves, texto_de_cota, texto_de_medida,
};
pub use organizar::{
    Alineacion, Reparto, agrupar, al_fondo, al_frente, alinear, desagrupar, hermanos_de, repartir,
};
pub use pintado::{
    Orden, marco_de_seleccion, ordenes, ordenes_a_distancia, ordenes_de_escena,
    ordenes_de_escena_vista,
};
pub use relleno::{EstiloRelleno, lineas_de_rayado};
pub use seleccion::Seleccion;
pub use vector::{Punto2, distancia_a_segmento};
