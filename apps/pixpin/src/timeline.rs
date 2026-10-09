//! **El timeline** (5-oct-2026): apuntar lo que pasa, minuto a minuto, en
//! el momento en que pasa — con la voz, escribiendo o con una foto— y verlo
//! como una linea de tiempo vertical.
//!
//! Lo pidio el usuario: «como un chat que se llena super rapido, con una UI
//! diferente al chat; solo voz, texto e imagenes; que se muestren las
//! ultimas 24 horas, lo demas se archive y se pueda buscar viendo el
//! calendario; y exportar las ultimas 24 horas o los dias que elija en HTML
//! o PDF con el mismo formato de timeline». Al dictar, «con esto paso…» abre
//! el titulo y «y asi te lo cuento…» la descripcion
//! (`pixpin_timeline::frases`).
//!
//! En el movil no existe (buscado en el Kotlin actual el 5-oct), asi que no
//! hay logica de Android que copiar. Vive aparte del chat
//! (`<datos>/timeline/`) para no llenar el cuaderno que se sincroniza con
//! notas del dia; el formato es JSON de una linea por momento, facil de
//! llevar al movil cuando alli exista.
//!
//! - [`ventana`]: la ventana, en su propio hilo.
//! - [`dictar`]: el microfono, que aqui guarda el audio.
//! - [`exportar`]: HTML y PDF.
//!
//! El formato, el fichero, las frases, los dias y la pagina estan en el crate
//! `pixpin-timeline`, con sus pruebas.

mod dictar;
mod disposicion;
pub(crate) mod exportar;
pub(crate) mod tarjetas;
mod ventana;

use pixpin_store::{Idioma, Ubicacion};

/// Abre el timeline, o lo trae delante si ya estaba. Vuelve enseguida.
pub fn abrir(idioma: Idioma, ubicacion: Ubicacion) {
    ventana::abrir(idioma, ubicacion);
}

/// Abre el timeline en la pestana «Lecciones» (o la pone si ya estaba
/// abierto). Es la entrada de las lecciones desde el 5-oct-2026: el usuario
/// pidio juntarlas con el timeline, «que todo se muestre en el mismo lugar».
pub fn abrir_lecciones(idioma: Idioma, ubicacion: Ubicacion) {
    ventana::abrir_en(idioma, ubicacion, ventana::Pestana::Lecciones);
}
