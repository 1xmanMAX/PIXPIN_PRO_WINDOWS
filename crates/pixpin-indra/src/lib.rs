//! pixpin-indra — el lector de texto **Indra OCR** (Datalab,
//! `github.com/datalab-to/indra`, pesos `datalab-to/indra-ocr` v1.1.0) sin
//! el runtime: todo lo que hay alrededor de los cuatro modelos ONNX, en Rust
//! puro y probado en cualquier sistema.
//!
//! Es el puerto del de Kotlin del escaner (Escaner Pro Max, `ocr/indra/`),
//! que lo es a su vez de `indra_ocr` en Python, con su paridad medida alli
//! (CER 1,3 % frente a Python). Mismos ficheros y mismos numeros, para que
//! el movil y el PC lean igual:
//!
//! - [`imagen`] y [`gris`]: la pagina en gris, la inclinacion, la entrada
//!   del detector y los recortes de cada renglon (lo de `cv2`, a mano).
//! - [`prep`]: el fondo a blanco y un enfoque suave antes de leer.
//! - [`deteccion`]: de la salida de `det.onnx` a cajas de renglon.
//! - [`lm`]: el modelo de lenguaje n-grama y la busqueda en haz CTC.
//! - [`orden`]: orden de lectura y bloques (`order.onnx` y el XY-cut).
//! - [`dudas`], [`lexico`] y [`corrector`]: el «afinado» del escaner —
//!   lineas dudosas, diccionario espanol hunspell y «Mi vocabulario».
//! - [`canal`]: la pagina entera, con los modelos detras del rasgo
//!   [`canal::Modelos`] (en Windows los corre `pixpin-ocr` con el ONNX
//!   Runtime del sistema).
//! - [`catalogo`]: que se baja, de donde, cuanto pesa y su sha256.
//!
//! **Licencia de los pesos**: OpenRAIL-M modificada de Datalab — gratis
//! para uso personal, investigacion y empresas de menos de 20 M$ de
//! ingresos o financiacion que no compitan con su API; el resto necesita
//! licencia de Datalab. Por eso los pesos no van con PixPin: los baja el
//! usuario, si quiere, desde Ajustes (ver `docs/ocr.md`). El codigo de
//! Indra es Apache-2.0.
#![forbid(unsafe_code)]

pub mod canal;
pub mod catalogo;
pub mod corrector;
pub mod deteccion;
pub mod dudas;
pub mod gris;
pub mod imagen;
pub mod lexico;
pub mod lm;
pub mod orden;
pub mod prep;
pub mod vocab;

pub use canal::{Indra, Linea, Modelos, Pagina, Palabra};
pub use gris::Gris;
