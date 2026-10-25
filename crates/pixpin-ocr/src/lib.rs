//! pixpin-ocr — reconocer el texto de una imagen, con el motor que elija el
//! usuario.
//!
//! Este crate habla con el sistema operativo o con librerias C. El `unsafe`
//! esta permitido, pero cada bloque lleva su comentario `// SAFETY:`.
//!
//! **Dos motores detras del rasgo [`Ocr`]** (ver `docs/ocr.md`):
//!
//! - [`windows_ocr`]: `Windows.Media.Ocr`, el de fabrica. Viene con Windows,
//!   no se baja nada y lee bien el texto de pantalla.
//! - [`indra`]: Indra OCR de Datalab (`pixpin-indra` + el ONNX Runtime de
//!   Windows). Lee mucho mejor fotos, escaneos, paginas torcidas, tablas y
//!   letra pequena; trae orden de lectura y bloques. Se baja aparte (26 MB)
//!   y solo si el usuario lo pide en Ajustes: la licencia de sus pesos no
//!   deja que PixPin los reparta.
//!
//! **Toda la app pasa por aqui**: [`reconocer`] y [`disponible`] miran el
//! motor elegido con [`configurar`] y, si Indra no esta listo o falla, usan
//! el de Windows. Asi la captura de texto, «Calca esto», los pines, la
//! galeria, los PDF escaneados y las herramientas del asistente leen con el
//! mismo motor sin saber cual es. El resultado tiene siempre la misma forma:
//! lineas con su caja y sus palabras, en pixeles de la imagen.
//!
//! Aqui solo se reconoce. Agrupar las lineas en parrafos y columnas es
//! geometria pura y vive en `pixpin-geom::parrafos`.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod indra;
pub mod windows_ocr;

use std::path::PathBuf;
use std::sync::RwLock;

use pixpin_geom::Rect;

#[derive(Debug, thiserror::Error)]
pub enum ErrorOcr {
    /// No hay ningun idioma de reconocimiento instalado. Se arregla desde
    /// los ajustes de Windows, anadiendo el paquete de idioma.
    #[error("este Windows no tiene ningun idioma de reconocimiento instalado")]
    SinIdiomas,
    #[error("la imagen no es coherente con sus medidas")]
    ImagenInvalida,
    #[error("el reconocimiento fallo: {0}")]
    Sistema(#[source] windows::core::Error),
    /// El lector Indra no esta bajado o no se pudo cargar.
    #[error("el lector Indra no esta listo: {0}")]
    IndraNoListo(String),
    /// El lector Indra fallo leyendo (se cae al de Windows).
    #[error("el lector Indra fallo: {0}")]
    Indra(String),
    /// Bajar el lector fallo o se corto.
    #[error("no se pudo bajar {que}: {detalle}")]
    Descarga { que: String, detalle: String },
    #[error("la descarga se cancelo")]
    Cancelada,
}

impl From<windows::core::Error> for ErrorOcr {
    fn from(e: windows::core::Error) -> Self {
        ErrorOcr::Sistema(e)
    }
}

/// Una linea reconocida: su texto y la caja que ocupa, en pixeles de la
/// imagen que se paso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Linea {
    pub caja: Rect,
    pub texto: String,
    /// Las palabras con su recuadro, en el orden de lectura.
    ///
    /// Es lo que hace posible seleccionar texto con el raton sobre un pin
    /// (P4.4): los dos motores entregan palabras, no caracteres, asi que la
    /// seleccion mas fina que se puede hacer BIEN es por palabra.
    pub palabras: Vec<Palabra>,
}

/// Una palabra reconocida, con el recuadro que ocupa en la imagen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palabra {
    pub caja: Rect,
    pub texto: String,
}

/// **Un motor de OCR.** Los dos dan lo mismo: lineas con caja y palabras.
pub trait Ocr: Send + Sync {
    /// Para el registro: «windows» o «indra».
    fn nombre(&self) -> &'static str;
    /// Si puede leer ya, sin bajar nada.
    fn disponible(&self) -> bool;
    /// Lee una imagen RGBA de `ancho` x `alto`.
    fn reconocer(&self, ancho: u32, alto: u32, rgba: &[u8]) -> Result<Vec<Linea>, ErrorOcr>;
}

/// `Windows.Media.Ocr`.
pub struct MotorWindows;

impl Ocr for MotorWindows {
    fn nombre(&self) -> &'static str {
        "windows"
    }
    fn disponible(&self) -> bool {
        windows_ocr::disponible()
    }
    fn reconocer(&self, ancho: u32, alto: u32, rgba: &[u8]) -> Result<Vec<Linea>, ErrorOcr> {
        windows_ocr::reconocer(ancho, alto, rgba)
    }
}

/// Que motor quiere el usuario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Motor {
    #[default]
    Windows,
    Indra,
}

/// Lo que la app le dice al OCR (de `[ocr]` en los ajustes).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Config {
    pub motor: Motor,
    /// La carpeta de datos de PixPin: Indra vive en `<datos>/indra/` y el
    /// ONNX Runtime bajado, en `<datos>/whisper/` (el sitio que ya mira
    /// Whisper).
    pub raiz: Option<PathBuf>,
    /// La carpeta del ejecutable, otro sitio donde puede estar el runtime.
    pub junto_al_exe: Option<PathBuf>,
    /// Corregir con el diccionario espanol.
    pub corregir: bool,
    /// «Mi vocabulario», como lo escribio el usuario.
    pub vocabulario: String,
    /// Hilos para el ONNX Runtime y la imagen (2 en el modo cuidadoso).
    pub hilos: usize,
}

static CONFIG: RwLock<Option<Config>> = RwLock::new(None);

/// **Elige el motor** (al arrancar y cada vez que cambian los ajustes).
/// Cambiar de motor o de vocabulario suelta a Indra si estaba cargado: se
/// vuelve a cargar al leer, con lo nuevo.
pub fn configurar(c: Config) {
    let cambia = config().is_none_or(|v| v != c);
    if cambia {
        indra::soltar();
    }
    if let Ok(mut g) = CONFIG.write() {
        *g = Some(c);
    }
}

/// Lo configurado (o `None` si la app no dijo nada: el de Windows).
pub fn config() -> Option<Config> {
    CONFIG.read().ok().and_then(|g| g.clone())
}

/// El motor que se usaria ahora mismo: Indra si esta elegido y bajado; si
/// no, el de Windows.
pub fn motor_en_uso() -> Motor {
    match config() {
        Some(c) if c.motor == Motor::Indra && c.raiz.as_deref().is_some_and(indra::listo) => {
            Motor::Indra
        }
        _ => Motor::Windows,
    }
}

/// Si el equipo puede reconocer texto. Se pregunta antes de ofrecerlo, para
/// no abrir una captura que luego no va a servir de nada.
pub fn disponible() -> bool {
    motor_en_uso() == Motor::Indra || windows_ocr::disponible()
}

/// **Reconoce el texto de una imagen RGBA** de `ancho` x `alto` con el
/// motor elegido.
///
/// Devuelve una linea por renglon, con su caja. Con Indra, en orden de
/// lectura y sin las lineas dudosas (las que lee con menos de 0,5 de
/// confianza: no van al texto, igual que en el escaner); con Windows, en el
/// orden que de el sistema (`pixpin_geom::parrafos::agrupar` lo pone en
/// orden de lectura). Si Indra falla, lee el de Windows y lo apunta.
pub fn reconocer(ancho: u32, alto: u32, pixeles_rgba: &[u8]) -> Result<Vec<Linea>, ErrorOcr> {
    if !medidas_validas(ancho, alto, pixeles_rgba) {
        return Err(ErrorOcr::ImagenInvalida);
    }
    if let Some(c) = config()
        && c.motor == Motor::Indra
        && c.raiz.as_deref().is_some_and(indra::listo)
    {
        match indra::reconocer(&c, ancho, alto, pixeles_rgba) {
            Ok(l) => return Ok(l),
            Err(e) => {
                tracing::warn!(error = %e, "Indra fallo; se lee con el OCR de Windows");
            }
        }
    }
    windows_ocr::reconocer(ancho, alto, pixeles_rgba)
}

/// **Deja el motor listo para leer**, sin leer nada: con Indra elegido y
/// bajado, carga sus modelos si no lo estaban ([`indra::precargar`]); con el
/// de Windows no hace nada (no tiene nada caro que cargar). Devuelve si
/// cargo algo. Bloquea lo que tarde la carga: se llama desde un hilo.
pub fn precargar() -> bool {
    let Some(c) = config() else {
        return false;
    };
    if c.motor != Motor::Indra || !c.raiz.as_deref().is_some_and(indra::listo) {
        return false;
    }
    match indra::precargar(&c) {
        Ok(cargo) => cargo,
        Err(e) => {
            // No pasa nada: la primera lectura lo volvera a intentar y, si
            // falla otra vez, leera el de Windows.
            tracing::warn!(error = %e, "no se pudo precargar Indra");
            false
        }
    }
}

/// Si el buffer cuadra con las medidas (y las medidas no desbordan).
pub(crate) fn medidas_validas(ancho: u32, alto: u32, pixeles_rgba: &[u8]) -> bool {
    let necesarios = (ancho as usize)
        .checked_mul(alto as usize)
        .and_then(|n| n.checked_mul(4));
    matches!(necesarios, Some(n) if ancho > 0 && alto > 0 && pixeles_rgba.len() >= n)
}

pub(crate) fn envolvente(a: Rect, b: Rect) -> Rect {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    let derecha = (a.x + a.ancho as i32).max(b.x + b.ancho as i32);
    let abajo = (a.y + a.alto as i32).max(b.y + b.alto as i32);
    Rect {
        x,
        y,
        ancho: (derecha - x).max(0) as u32,
        alto: (abajo - y).max(0) as u32,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_envolvente_cubre_las_dos_cajas() {
        let a = Rect {
            x: 10,
            y: 10,
            ancho: 20,
            alto: 5,
        };
        let b = Rect {
            x: 40,
            y: 8,
            ancho: 10,
            alto: 9,
        };
        assert_eq!(
            envolvente(a, b),
            Rect {
                x: 10,
                y: 8,
                ancho: 40,
                alto: 9
            }
        );
    }

    #[test]
    fn sin_configurar_o_sin_indra_bajado_manda_el_de_windows() {
        assert_eq!(motor_en_uso(), Motor::Windows);
        let dir = std::env::temp_dir().join(format!("pixpin-ocr-motor-{}", std::process::id()));
        configurar(Config {
            motor: Motor::Indra,
            raiz: Some(dir.clone()),
            ..Config::default()
        });
        // Elegido pero sin bajar: el de Windows, y no hay nada que precargar.
        assert_eq!(motor_en_uso(), Motor::Windows);
        assert!(!precargar());
        // Una imagen incoherente se rechaza antes de tocar ningun motor.
        assert!(matches!(
            reconocer(10, 10, &[0u8; 8]),
            Err(ErrorOcr::ImagenInvalida)
        ));
        configurar(Config::default());
    }
}
