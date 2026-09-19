//! El universo: los archivos del chat en un lienzo infinito.
//!
//! Diseno: `docs/superpowers/specs/2026-09-18-universo-design.md`.
//!
//! Es puro a proposito: ni Windows, ni Direct2D, ni el chat. Recibe los
//! archivos ya traducidos (`FichaLuna`) y devuelve que se ve y donde. Asi las
//! reglas de verdad —que puede ir dentro de que, que se pinta a cada zoom—
//! se prueban en milisegundos y sin ventana.

#![forbid(unsafe_code)]

pub mod astro;
pub mod buscar;
pub mod detalle;
pub mod ficha;
pub mod galaxias;
pub mod herramienta;
pub mod historia;
pub mod jerarquia;
pub mod nebulosa;
pub mod operar;
pub mod universo;
pub mod vista;

pub use astro::{
    Astro, Clase, Conexion, IdAstro, RADIO_GALAXIA, RADIO_LUNA, RADIO_PLANETA_L, RADIO_PLANETA_M,
    RADIO_PLANETA_S, TipoConexion,
};
pub use buscar::{Hallazgo, IndiceBusqueda, TOPE_RESULTADOS, normalizar};
pub use detalle::{
    HISTERESIS, Nivel, Tipo, ZOOM_MINIMO_UNIVERSO, abre_hijos, nivel, nivel_con_memoria, tipo_de,
};
pub use ficha::{ClaseLuna, EXTRACTO, FichaLuna, es_colocable, extracto};
pub use galaxias::{DISTANCIA_MINIMA, Informe, SEPARACION, siguiente_hueco, sincronizar};
pub use herramienta::HerramientaUniverso;
pub use jerarquia::{Aterriza, Rechazo};
pub use operar::Arrastre;
pub use universo::{Encuadre, Universo, VERSION};
pub use vista::{
    RejillaAstros, TOPE_DETALLE, Visto, astro_en, conexiones_visibles, extremos, visibles,
};
