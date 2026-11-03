//! **El comentario avanzado** (3-nov-2026): una pegatina de comentario que
//! se pega en un punto de cualquier visor (el lienzo, un PDF, un Word, una
//! hoja de calculo, una imagen, una pagina web, una nota, un plano) y que,
//! una vez puesta, es **un minichat**: se escribe, se manda una imagen, un
//! fichero, un atajo a un elemento del lienzo o a un mensaje del chat, o un
//! lienzo propio en blanco; la imagen se abre en el lienzo para anotar
//! encima.
//!
//! Lo pidio el usuario: «un emoticono de comentario, cuatro tipos (tres de
//! urgencia y uno de comentario normal), que pego donde sea como una
//! pegatina; una vez puesto funciona como atajo a una foto, un texto o un
//! elemento como una figura que cree, como un MINICHAT: mando una imagen y
//! se abre en el lienzo, y tambien puedo anotar sobre su comentario».
//!
//! Este crate es solo cuentas y JSON, igual en todos los aparatos:
//!
//! - [`modelo`]: el comentario, su tipo, su ancla, su hilo y sus adjuntos,
//!   con lo que se les hace (poner, mover, escribir, resolver, quitar).
//! - [`ancla`]: de las unidades de cada visor a la pantalla y vuelta.
//! - [`fichero`]: el JSON, dentro del `.comentarios.json` de siempre (la
//!   clave `pegatinas`, junto a los comentarios de las notas).
//! - [`fusion`]: juntar lo de dos aparatos (por id; lo de la pegatina, gana
//!   el ultimo cambio; el hilo se suma entrada a entrada).
//! - [`espejo`]: la pegatina de un lienzo, copiada como elemento Excalidraw
//!   (`customData.comentario`) para que un movil sin esto vea el icono.
//! - [`lista`]: la lista «Comentarios» de un documento, por urgencia y fecha.
//!
//! El formato para Android, Mac y iPad: `docs/comentarios.md`.

#![forbid(unsafe_code)]

pub mod ancla;
pub mod espejo;
pub mod fichero;
pub mod fusion;
pub mod lista;
pub mod modelo;

pub use ancla::{Ancla, Cita, Posicion, Sitio};
pub use modelo::{Adjunto, Comentario, Entrada, Pegatinas, Quien, Tipo, TipoAdjunto};
