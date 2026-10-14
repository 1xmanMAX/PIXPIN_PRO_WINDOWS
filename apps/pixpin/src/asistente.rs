//! **El asistente local en el chat** (encargos 1-3 de
//! `docs/investigacion/2026-10-13-ia-local.md` §7).
//!
//! El cerebro esta en `crates/pixpin-ia` (motor llama.cpp en proceso aparte,
//! protocolo de herramientas, bucle `agente::turno`). Aqui va lo que solo
//! sabe la app:
//!
//! - [`motor`]: **un motor por proceso** de PixPin, del modelo elegido en
//!   los ajustes (2B o 0.8B), creado al primer uso. El propio motor cierra
//!   el modelo a los 5 min sin uso y devuelve la memoria entera.
//! - [`descarga`]: bajar el motor y el modelo en un hilo, con progreso y
//!   cancelar, para la tarjeta de la primera vez.
//! - [`turno`]: un turno del agente **en un hilo de fondo**. La UI no espera
//!   nunca: mira el canal una vez por vuelta de su bucle, como hace con la
//!   voz o la transcripcion, y pinta lo que llegue (texto en vivo, «dibujando…»,
//!   el final). «Parar» corta en el siguiente trozo.
//! - [`ejecutor`]: hace cada herramienta. Lo del lienzo se le pide a la UI
//!   (el lienzo vive en su hilo) y se espera su respuesta; el OCR corre en el
//!   propio hilo del turno ([`ocr`]).
//! - [`al_lienzo`]: meter lo dibujado en un lienzo de verdad, centrado en la
//!   vista y en **un solo paso de deshacer**.
//! - [`foto`]: reducir la foto adjunta a ~768 px antes de mandarla.
//! - [`pensado`]: lo que penso cada respuesta, su peticion mejorada y la
//!   hoja de trabajo del chat, guardados con la nota (13-oct).
//! - [`nube`]: contestar con Claude Haiku 5.5 o DeepSeek V4.1 Flash (la
//!   clave cifrada con DPAPI) y, si fallan, seguir con el local.
//! - [`documentos`]: `buscar_en_mis_cosas`: el indice local de los PDF,
//!   Office, notas y chats del usuario, al dia en segundo plano, y el
//!   permiso de mandar trozos a la nube.
//!
//! Android todavia no tiene asistente (comprobado el 13-oct con `gh`): aqui
//! se abre camino y el protocolo (`catalogo_json`, Excalidraw) es el que
//! portara el movil.

pub mod acciones;
pub mod al_lienzo;
pub mod chat;
pub mod descarga;
pub mod documentos;
pub mod documentos_acciones;
pub mod ejecutor;
pub mod foto;
pub mod motor;
pub mod nube;
pub mod ocr;
pub mod permiso;
pub mod recetas;
pub mod pensado;
pub mod rapido;
pub mod turno;

#[cfg(test)]
mod pruebas;
#[cfg(test)]
mod pruebas_pensado;
