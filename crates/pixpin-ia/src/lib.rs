//! **Un modelo de lenguaje pequeno, en este ordenador y sin red.**
//!
//! Pedido del usuario (13-oct-2026): un asistente local que viva en el chat y
//! que, ademas de contestar, use **herramientas** de PixPin: dibujar en el
//! lienzo lo que se le pide o lo que ve en una foto, hacer graficas, pasar
//! por OCR un PDF escaneado... Aceptando alguna alucinacion pequena, pero sin
//! nada pesado. El analisis completo, con las medidas en este PC, esta en
//! `docs/investigacion/2026-10-13-ia-local.md`.
//!
//! ## Lo que se eligio y por que (resumen)
//!
//! - **Modelo: Qwen3.5** (Apache-2.0), en GGUF cuantizado a 4 bits. El 2B
//!   por omision (ve imagenes y llama herramientas bien); el 0.8B como
//!   «ligero» para equipos flojos o moviles. Los dos tienen vision.
//! - **Motor: llama.cpp** (MIT), su `llama-server` **en un proceso aparte**.
//!   El mismo GGUF corre en Android con llama.cpp, que es la condicion para
//!   que el PC y el movil se porten igual. Ni un byte en el ejecutable: el
//!   motor (unos 30 MB) y el modelo se bajan cuando el usuario lo pide.
//!
//! ## Las piezas
//!
//! - [`modelos`]: que se baja, de donde, cuanto pesa y su huella.
//! - [`descarga`]: bajar y verificar (SHA-256) a
//!   `%LOCALAPPDATA%\PixPinMax\modelos`, nunca en el instalador.
//! - [`motor`]: el trait [`Motor`] —lo unico que ve el resto— y los tipos de
//!   la conversacion. Detras puede ir llama.cpp ([`servidor`]) o un motor
//!   falso para las pruebas ([`motor::MotorGuionado`]).
//! - [`servidor`]: lanzar `llama-server`, hablarle por HTTP local con
//!   streaming, y matarlo al rato de no usarlo para devolver la memoria.
//! - [`herramientas`]: el **protocolo comun PC/Android** (nombre,
//!   descripcion, parametros en JSON Schema) y la validacion estricta de lo
//!   que devuelve el modelo.
//! - [`lienzo`]: de una llamada validada a elementos de **Excalidraw**, el
//!   formato que ya comparten el lienzo del PC y el de Android.
//! - [`agente`]: el bucle de un turno: pedir, validar, ejecutar, devolver el
//!   resultado al modelo, y reintentar si el modelo se equivoco.
//!
//! ## Puntos de enganche para la UI (otros agentes)
//!
//! - El chat implementa [`agente::Ejecutor`] (que hace cada herramienta de
//!   verdad: anadir al lienzo, OCR de Windows...) y escucha los
//!   [`agente::Evento`] para pintar la respuesta en vivo.
//! - El lienzo recibe un `.excalidraw` ya hecho ([`lienzo::documento`]) y lo
//!   lee con `pixpin_motor2d::excalidraw::leer`, como un fichero del movil.

pub mod agente;
pub mod base64;
pub mod descarga;
pub mod herramientas;
pub mod lienzo;
pub mod modelos;
pub mod motor;
pub mod servidor;
pub mod sse;

pub use agente::{Ejecutor, Evento, turno};
pub use herramientas::{Herramienta, Llamada, catalogo};
pub use motor::{Mensaje, Motor, Parte, Peticion, Respuesta, Rol};

/// Lo que puede salir mal. Los textos van en espanol porque acaban en la
/// burbuja del chat.
#[derive(Debug, thiserror::Error)]
pub enum ErrorIa {
    #[error("no se pudo bajar {que}: {detalle}")]
    Descarga { que: String, detalle: String },
    #[error("el fichero {que} esta danado (su huella no coincide); se borra y hay que bajarlo otra vez")]
    HuellaDistinta { que: String },
    #[error("falta {0}: hay que bajar el asistente primero")]
    Falta(String),
    #[error("no se pudo arrancar el modelo: {0}")]
    Arranque(String),
    #[error("el modelo dejo de responder: {0}")]
    Conexion(String),
    #[error("el modelo contesto algo que no se entiende: {0}")]
    Protocolo(String),
    #[error("cancelado")]
    Cancelado,
    #[error("{0}")]
    Io(#[from] std::io::Error),
}
