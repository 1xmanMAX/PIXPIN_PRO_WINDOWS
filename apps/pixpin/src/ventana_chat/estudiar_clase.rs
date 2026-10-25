//! **Estudiar una clase grabada** (25-oct-2026, el usuario: «una ventana
//! especial, siguiendo la misma interfaz de las demas ventanas, donde se vea
//! la grabacion del video con su transcripcion al lado; al otro lado las
//! capturas, los marcadores, etc., todo integrado; y que las capturas y el
//! texto se puedan desplazar de forma que uno siga al otro»).
//!
//! Una ventana normal, en su hilo, como la de las figuras:
//!
//! - **A la izquierda, lo que se ve**: el video de la clase (con su sonido)
//!   o, si no se grabo video, la captura del minuto que suena en grande y
//!   la nota de voz sonando. Debajo, la barra del tiempo con una raya por
//!   captura y una banderita por marcador (un clic cerca se pega a ellas),
//!   los mandos (atras, reproducir, adelante, la velocidad, «Marcador» y
//!   «Seguir») y la lista de marcadores.
//! - **En el centro, el texto**, con su minuto delante de cada trozo y los
//!   marcadores metidos en su sitio. Un clic lleva ahi; lo que suena se
//!   resalta.
//! - **A la derecha, las capturas** en una tira, con su minuto. Un clic
//!   lleva ahi y la ensena en grande.
//!
//! **Que una siga a la otra** (`pixpin_proyecto::clase`): con «Seguir»
//! encendido, mientras suena el texto y la tira van solos a lo que suena; y
//! al mover una con la rueda, la otra se pone en el mismo minuto. Mover una
//! a mano deja de seguir al sonido un rato, para poder leer.
//!
//! **Los marcadores** se guardan en la nota de voz, en `resto["marcadores"]`
//! (`[{ms, texto}]`), que viaja al movil como lo demas de `resto`. Las
//! banderitas del movil (`marcas`, sin nota) se ensenan con ellos.
//!
//! Teclado: espacio o K reproduce y para, ←/→ cinco segundos, J/L diez,
//! ↑/↓ el trozo anterior y el siguiente, M pone un marcador (se escribe su
//! nota e Intro lo guarda), S enciende y apaga «Seguir», Escape cierra.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_codec::imagen::ImagenRgba;
use pixpin_geom::{Punto, Rect};
use pixpin_proyecto::clase::{self, Fila, Marcador};
use pixpin_proyecto::{almacen, cuaderno};
use pixpin_render::{Color, MotorRender, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Idioma, Ubicacion};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use super::{CLARO, OSCURO, Tema, con_alfa, mi};
use crate::caja_dibujo::hex;
use crate::overlay::Recursos;
use crate::ventanita::dentro;

/// Lo que mide la ventana (logicos) y sus columnas.
const ANCHO: u32 = 1360;
const ALTO: u32 = 840;
const BARRA: f32 = 48.0;
const MARGEN: f32 = 16.0;
const ANCHO_FOTOS: f32 = 236.0;
const CABECERA: f32 = 36.0;
/// El color de los marcadores: ambar, que se distingue del azul de las
/// capturas en la barra del tiempo.
const AMBAR: Color = hex(0xf2a33a);
/// Lo que se deja de seguir al sonido despues de mover una lista a mano.
const A_MANO: Duration = Duration::from_secs(5);
/// Lo que dura un aviso abajo.
const AVISO_DURA: Duration = Duration::from_millis(2200);
/// Los saltos del teclado y de los botones.
const SALTO_CORTO: i64 = 5_000;
const SALTO_LARGO: i64 = 10_000;
/// Un clic en la barra del tiempo a menos de esto (logicos) de una captura
/// o un marcador se queda en el.
const PEGAR: f32 = 6.0;
/// Las miniaturas de la tira se bajan a este ancho (pixeles): sobra para
/// una columna de 236 a 200 %, y treinta capturas a pantalla entera no
/// caben en la memoria del equipo de referencia.
const ANCHO_MINIATURA: u32 = 480;

const VK_ESPACIO: u32 = 0x20;
const VK_ENTRAR: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_RETROCESO: u32 = 0x08;
const VK_IZQUIERDA: u32 = 0x25;
const VK_ARRIBA: u32 = 0x26;
const VK_DERECHA: u32 = 0x27;
const VK_ABAJO: u32 = 0x28;
const VK_J: u32 = 0x4A;
const VK_K: u32 = 0x4B;
const VK_L: u32 = 0x4C;
const VK_M: u32 = 0x4D;
const VK_S: u32 = 0x53;

/// La ventana abierta (su HWND), -1 mientras nace, 0 sin ventana.
static ABIERTA: AtomicIsize = AtomicIsize::new(0);

/// Que clase abrir: el proyecto, el id de la nota de voz (o de una captura
/// o del video de la clase: se busca su nota) y el minuto donde empezar.
#[derive(Debug, Clone)]
struct Pedido {
    ubicacion: Ubicacion,
    proyecto: String,
    id: String,
    ms: i64,
}

/// Otra clase pedida con la ventana ya abierta: la recoge su bucle.
static OTRA: Mutex<Option<Pedido>> = Mutex::new(None);

/// **Abre la clase en la ventana de estudiar**, o la cambia y la trae
/// delante si ya estaba abierta. `id` puede ser la nota de voz o cualquier
/// cosa de la clase (una captura, el video): se busca su nota.
pub(crate) fn abrir(idioma: Idioma, ubicacion: Ubicacion, proyecto: String, id: String, ms: i64) {
    let pedido = Pedido {
        ubicacion,
        proyecto,
        id,
        ms,
    };
    let ya = ABIERTA.load(Ordering::SeqCst);
    if ya != 0 {
        if let Ok(mut g) = OTRA.lock() {
            *g = Some(pedido);
        }
        if ya > 0 {
            VentanaOverlay::restaurar_de_hwnd(windows::Win32::Foundation::HWND(ya as *mut _));
            pixpin_shell::overlay::despertar(ya);
        }
        return;
    }
    ABIERTA.store(-1, Ordering::SeqCst);
    let lanzado = std::thread::Builder::new()
        .name("estudiar-clase".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(idioma);
            // Si el dispositivo grafico se pierde, la ventana se reabre en
            // el mismo minuto.
            let mut pedido = pedido;
            let hecho = crate::dispositivo_perdido::con_recursos("estudiar clase", |r| {
                bucle(r, &textos, &mut pedido)
            });
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo abrir la ventana de estudiar la clase");
            }
            if let Ok(mut g) = OTRA.lock() {
                *g = None;
            }
            ABIERTA.store(0, Ordering::SeqCst);
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de estudiar la clase");
        ABIERTA.store(0, Ordering::SeqCst);
    }
}

/// **Si un mensaje es de una clase grabada**: `(id de su nota de voz, minuto)`.
/// La nota misma (`grabacion: "clase"`) va al principio; una captura o el
/// video de la clase (`enGrabacion`), a su minuto.
pub(super) fn de_una_clase(m: &cuaderno::Mensaje) -> Option<(String, i64)> {
    if m.resto.get("grabacion").and_then(|v| v.as_str()) == Some("clase") {
        return Some((m.id.clone(), 0));
    }
    let en = m.resto.get("enGrabacion")?;
    let de = en.get("de")?.as_str()?.to_string();
    Some((de, en.get("ms").and_then(|v| v.as_i64()).unwrap_or(0)))
}

// --- La clase leida ------------------------------------------------------------

/// Una senal de la barra y de la lista: un marcador (con su nota) o una
/// banderita del movil (sin ella).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Senal {
    ms: i64,
    texto: String,
    banderita: bool,
}

/// Todo lo de una clase, leido del cuaderno.
struct Clase {
    proyecto: String,
    id: String,
    nombre: String,
    duracion_ms: i64,
    audio: Option<PathBuf>,
    video: Option<PathBuf>,
    /// `(ms, ruta)` en orden; las que ya no estan en el disco, fuera.
    capturas: Vec<(i64, PathBuf)>,
    /// Los trozos del texto: su minuto (si lo lleva) y lo que dice.
    lineas: Vec<(Option<i64>, String)>,
    senales: Vec<Senal>,
    /// El texto con las senales en su minuto, y el minuto de cada fila.
    filas: Vec<Fila>,
    tiempos_filas: Vec<i64>,
}

impl Clase {
    fn leer(raiz: &Path, proyecto: &str, id: &str) -> Result<Clase> {
        let carpeta = almacen::carpeta(raiz, proyecto);
        let mensajes = cuaderno::Cuaderno::leer_de(&carpeta)
            .context("sin cuaderno")?
            .mensajes;
        let m = mensajes
            .iter()
            .find(|m| m.id == id)
            .context("la clase ya no esta en el chat")?;
        // Una captura o el video: su nota.
        let voz = match de_una_clase(m) {
            Some((de, _)) if de != m.id => mensajes
                .iter()
                .find(|v| v.id == de)
                .context("la nota de la clase ya no esta")?,
            _ => m,
        };
        let ruta_de = |id: &str| {
            mensajes
                .iter()
                .find(|c| c.id == id)
                .and_then(|c| super::ruta_del_mensaje(raiz, proyecto, c))
                .filter(|r| r.is_file())
        };
        let capturas = super::grabar_clase::capturas_de(voz)
            .into_iter()
            .filter_map(|(ms, id)| Some((ms, ruta_de(&id)?)))
            .collect();
        let video = voz
            .resto
            .get("video")
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            .and_then(ruta_de);
        let audio = super::ruta_del_mensaje(raiz, proyecto, voz).filter(|r| r.is_file());
        let lineas = super::trozos_de_la_letra(voz)
            .into_iter()
            .map(|t| (t.ms, t.texto))
            .collect();
        let mut c = Clase {
            proyecto: proyecto.to_string(),
            id: voz.id.clone(),
            nombre: if voz.nombre.trim().is_empty() {
                voz.texto.clone()
            } else {
                voz.nombre.trim_end_matches(".m4a").to_string()
            },
            duracion_ms: voz.duracion_ms.max(0),
            audio,
            video,
            capturas,
            lineas,
            senales: Vec::new(),
            filas: Vec::new(),
            tiempos_filas: Vec::new(),
        };
        c.poner_senales(senales_de(voz));
        Ok(c)
    }

    fn poner_senales(&mut self, senales: Vec<Senal>) {
        self.senales = senales;
        let lineas: Vec<Option<i64>> = self.lineas.iter().map(|(ms, _)| *ms).collect();
        let ms: Vec<i64> = self.senales.iter().map(|s| s.ms).collect();
        self.filas = clase::mezclar(&lineas, &ms);
        self.tiempos_filas = clase::tiempos_de_filas(&self.filas, &lineas, &ms);
    }

    fn tiempos_capturas(&self) -> Vec<i64> {
        self.capturas.iter().map(|(ms, _)| *ms).collect()
    }
}

/// Los marcadores y las banderitas de una nota, en orden.
fn senales_de(m: &cuaderno::Mensaje) -> Vec<Senal> {
    let mut v: Vec<Senal> = clase::marcadores_de(&m.resto)
        .into_iter()
        .map(|Marcador { ms, texto }| Senal {
            ms,
            texto,
            banderita: false,
        })
        .collect();
    for ms in crate::voz::marcas_de(m) {
        if !v.iter().any(|s| s.ms == ms) {
            v.push(Senal {
                ms,
                texto: String::new(),
                banderita: true,
            });
        }
    }
    v.sort_by_key(|s| s.ms);
    v
}

/// Cambia la nota en el cuaderno (relee: el chat o el movil pudieron
/// tocarla) y avisa al chat. Devuelve como quedo.
fn cambiar_la_nota(
    raiz: &Path,
    proyecto: &str,
    id: &str,
    cambio: impl FnOnce(&mut cuaderno::Mensaje) -> bool,
) -> Result<cuaderno::Mensaje> {
    let carpeta = almacen::carpeta(raiz, proyecto);
    let m = cuaderno::cambiar(&carpeta, id, cambio)?.context("la nota ya no esta")?;
    super::refrescar();
    Ok(m)
}

// --- Lo que suena ----------------------------------------------------------------

/// El video (con su sonido) o la nota de voz, con las mismas ordenes.
enum Medio {
    Video {
        r: pixpin_pin::Reproductor,
        bitmap: Option<ID2D1Bitmap1>,
        /// A donde ir en cuanto se sepa el tamano (la carga es asincrona).
        ir: Option<i64>,
        velocidad: f32,
    },
    Audio {
        s: pixpin_audio::salida::Salida,
        ir: Option<i64>,
        velocidad: f32,
        sonando: bool,
    },
    Nada,
}

impl Medio {
    fn abrir(
        d3d: &windows::Win32::Graphics::Direct3D11::ID3D11Device,
        c: &Clase,
        ms: i64,
    ) -> Medio {
        if let Some(v) = &c.video {
            match pixpin_pin::Reproductor::para_ver(d3d, v) {
                Ok(r) => {
                    return Medio::Video {
                        r,
                        bitmap: None,
                        ir: Some(ms),
                        velocidad: 1.0,
                    };
                }
                Err(e) => tracing::warn!(?e, "el video de la clase no se pudo abrir; va el audio"),
            }
        }
        Medio::audio(c, ms)
    }

    fn audio(c: &Clase, ms: i64) -> Medio {
        match c.audio.as_deref().map(pixpin_audio::salida::Salida::abrir) {
            Some(Ok(s)) => Medio::Audio {
                s,
                ir: Some(ms),
                velocidad: 1.0,
                sonando: false,
            },
            Some(Err(e)) => {
                tracing::warn!(?e, "la nota de la clase no se pudo abrir");
                Medio::Nada
            }
            None => Medio::Nada,
        }
    }

    fn es_video(&self) -> bool {
        matches!(self, Medio::Video { .. })
    }

    fn fallo(&self) -> bool {
        match self {
            Medio::Video { r, .. } => r.fallo(),
            Medio::Audio { s, .. } => s.fallo(),
            Medio::Nada => false,
        }
    }

    fn posicion_ms(&self) -> i64 {
        match self {
            Medio::Video { r, ir, .. } => {
                ir.unwrap_or_else(|| (r.posicion().0 * 1000.0).round() as i64)
            }
            Medio::Audio { s, ir, .. } => ir.unwrap_or_else(|| s.posicion_ms()),
            Medio::Nada => 0,
        }
    }

    fn duracion_ms(&self) -> i64 {
        match self {
            Medio::Video { r, .. } => r.posicion().1.map_or(0, |d| (d * 1000.0).round() as i64),
            Medio::Audio { s, .. } => s.duracion_ms(),
            Medio::Nada => 0,
        }
    }

    fn sonando(&self) -> bool {
        match self {
            Medio::Video { r, .. } => r.reproduciendo() && !r.acabado(),
            Medio::Audio { sonando, .. } => *sonando,
            Medio::Nada => false,
        }
    }

    fn velocidad(&self) -> f32 {
        match self {
            Medio::Video { velocidad, .. } | Medio::Audio { velocidad, .. } => *velocidad,
            Medio::Nada => 1.0,
        }
    }

    fn alternar(&mut self) {
        match self {
            Medio::Video { r, .. } => {
                // Al final, otra vez desde el principio: buscar arranca
                // solo si el motor no se quedo en pausa.
                if r.acabado() {
                    r.buscar(0.0, false);
                    if !r.reproduciendo() {
                        r.alternar_pausa();
                    }
                } else {
                    r.alternar_pausa();
                }
            }
            Medio::Audio {
                s,
                velocidad,
                sonando,
                ..
            } => {
                if *sonando {
                    s.pausar();
                    *sonando = false;
                } else {
                    if s.lista() && s.duracion_ms() > 0 && s.posicion_ms() >= s.duracion_ms() {
                        s.ir_a_ms(0);
                    }
                    match s.tocar(*velocidad) {
                        Ok(()) => *sonando = true,
                        Err(e) => tracing::warn!(?e, "la nota de la clase no arranca"),
                    }
                }
            }
            Medio::Nada => {}
        }
    }

    /// Va a `ms`. `aproximado` mientras se arrastra la barra: el fotograma
    /// clave mas cercano, que es instantaneo.
    fn ir_a(&mut self, ms: i64, aproximado: bool) {
        let ms = ms.max(0);
        match self {
            Medio::Video { r, ir, .. } => {
                if ir.is_some() {
                    *ir = Some(ms);
                } else {
                    r.buscar(ms as f64 / 1000.0, aproximado);
                }
            }
            Medio::Audio { s, ir, .. } => {
                if s.lista() {
                    s.ir_a_ms(ms);
                    *ir = None;
                } else {
                    *ir = Some(ms);
                }
            }
            Medio::Nada => {}
        }
    }

    fn otra_velocidad(&mut self) {
        match self {
            Medio::Video { r, velocidad, .. } => {
                *velocidad = pixpin_audio::reloj::siguiente_velocidad(*velocidad);
                r.poner_velocidad(*velocidad as f64);
            }
            Medio::Audio { s, velocidad, .. } => {
                *velocidad = pixpin_audio::reloj::siguiente_velocidad(*velocidad);
                s.poner_velocidad(*velocidad);
            }
            Medio::Nada => {}
        }
    }

    /// Lo de cada vuelta: el fotograma nuevo del video, el salto pendiente
    /// en cuanto se puede y el final de la nota. Devuelve si hay que
    /// repintar.
    fn latido(&mut self, motor: &MotorRender) -> bool {
        match self {
            Medio::Video { r, bitmap, ir, .. } => {
                if let Some(ms) = *ir
                    && r.dimensiones().is_some()
                {
                    r.buscar(ms as f64 / 1000.0, false);
                    *ir = None;
                }
                // Clonar la interfaz suelta el prestamo del reproductor.
                let textura = r.tick().cloned();
                match textura {
                    Some(t) => {
                        if bitmap.is_none() {
                            *bitmap = motor.bitmap_desde_textura(&t).ok();
                        }
                        true
                    }
                    None => false,
                }
            }
            Medio::Audio { s, ir, sonando, .. } => {
                if let Some(ms) = *ir
                    && s.lista()
                {
                    s.ir_a_ms(ms);
                    *ir = None;
                }
                if s.termino() {
                    s.pausar();
                    *sonando = false;
                    return true;
                }
                *sonando
            }
            Medio::Nada => false,
        }
    }
}

// --- Las imagenes, en su hilo ---------------------------------------------------

/// Lo que vuelve del hilo de las imagenes.
enum Cargada {
    Miniatura(usize, ImagenRgba),
    Grande(usize, ImagenRgba),
}

/// Lee las capturas fuera del hilo de la ventana: las miniaturas de todas,
/// una detras de otra, y la grande que se pida (esa primero).
struct Cargador {
    pedir: Sender<usize>,
    llegan: Receiver<Cargada>,
}

impl Cargador {
    fn lanzar(rutas: Vec<PathBuf>, lado_maximo: u32) -> Option<Cargador> {
        let (pedir, pedidas) = channel::<usize>();
        let (envia, llegan) = channel::<Cargada>();
        let lanzado = std::thread::Builder::new()
            .name("estudiar-imagenes".into())
            .spawn(move || {
                let grande = |i: usize| -> Option<Cargada> {
                    let img = pixpin_codec::imagen::cargar(rutas.get(i)?).ok()?;
                    Some(Cargada::Grande(i, reducir(img, lado_maximo)))
                };
                for (i, ruta) in rutas.iter().enumerate() {
                    // Lo pedido va antes que la tira: es lo que se esta
                    // mirando.
                    while let Ok(g) = pedidas.try_recv() {
                        if let Some(c) = grande(g)
                            && envia.send(c).is_err()
                        {
                            return;
                        }
                    }
                    if let Ok(img) = pixpin_codec::imagen::cargar(ruta) {
                        let ancho = img.ancho.min(ANCHO_MINIATURA);
                        let m = reducir(img, ancho.max(1));
                        if envia.send(Cargada::Miniatura(i, m)).is_err() {
                            return;
                        }
                    }
                }
                for g in pedidas {
                    if let Some(c) = grande(g)
                        && envia.send(c).is_err()
                    {
                        return;
                    }
                }
            });
        match lanzado {
            Ok(_) => Some(Cargador { pedir, llegan }),
            Err(e) => {
                tracing::warn!(?e, "no se pudo lanzar el hilo de las imagenes de la clase");
                None
            }
        }
    }
}

/// Baja una imagen para que su lado mayor no pase de `lado` (o su ancho, si
/// es una miniatura), sin deformarla.
fn reducir(img: ImagenRgba, lado: u32) -> ImagenRgba {
    let mayor = img.ancho.max(img.alto);
    if mayor <= lado || mayor == 0 {
        return img;
    }
    let k = lado as f32 / mayor as f32;
    let (w, h) = (
        ((img.ancho as f32 * k).round() as u32).max(1),
        ((img.alto as f32 * k).round() as u32).max(1),
    );
    pixpin_codec::imagen::redimensionar(img, w, h).unwrap_or_else(|e| {
        tracing::debug!(?e, "una captura de la clase no se pudo reducir");
        ImagenRgba {
            ancho: 1,
            alto: 1,
            pixeles: vec![0; 4],
        }
    })
}

/// Un bitmap con su tamano.
type Imagen = (ID2D1Bitmap1, u32, u32);

fn subir(motor: &MotorRender, img: &ImagenRgba) -> Option<Imagen> {
    motor
        .bitmap_desde_pixeles(img.ancho, img.alto, &img.pixeles)
        .ok()
        .map(|b| (b, img.ancho, img.alto))
}

// --- La ventana ------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Zona {
    Barra,
    Cerrar,
    Tocar,
    Atras,
    Adelante,
    Velocidad,
    Marcar,
    Seguir,
    /// Cambiar entre el video y la captura del minuto (con video).
    CambiarVista,
    /// La barra del tiempo entera.
    Tiempo,
    /// Lo que se ve: un clic reproduce o para.
    Medio,
    /// Un trozo del texto o una senal: ir a ese milisegundo.
    Ir(i64),
    /// Quitar la senal de ese milisegundo.
    Quitar(i64),
    /// Una captura de la tira: ir a su minuto y ensenarla.
    Captura(usize),
    /// Las listas, para la rueda.
    ListaTexto,
    ListaFotos,
}

/// Donde va cada cosa, en pixeles de la ventana.
#[derive(Debug, Clone, Copy)]
struct Disposicion {
    medio: RectF,
    tiempo: RectF,
    mandos: RectF,
    senales: RectF,
    cabecera_texto: RectF,
    texto: RectF,
    cabecera_fotos: RectF,
    fotos: RectF,
    /// Lo que mide una fila de la tira.
    fila_foto: f32,
}

fn disponer(ancho: f32, alto: f32, e: f32) -> Disposicion {
    let m = MARGEN * e;
    let arriba = BARRA * e + m;
    let abajo = alto - m;
    let ancho_fotos = ANCHO_FOTOS * e;
    let x_fotos = ancho - m - ancho_fotos;
    let ancho_texto = (ancho * 0.30).clamp(320.0 * e, 460.0 * e);
    let x_texto = x_fotos - m - ancho_texto;
    let ancho_medio = (x_texto - 2.0 * m).max(100.0 * e);
    let alto_columna = abajo - arriba;
    let alto_medio = (ancho_medio * 9.0 / 16.0)
        .min(alto_columna - 230.0 * e)
        .max(120.0 * e);
    let medio = RectF {
        x: m,
        y: arriba,
        ancho: ancho_medio,
        alto: alto_medio,
    };
    let tiempo = RectF {
        x: m,
        y: medio.y + medio.alto + 8.0 * e,
        ancho: ancho_medio,
        alto: 28.0 * e,
    };
    let mandos = RectF {
        x: m,
        y: tiempo.y + tiempo.alto + 4.0 * e,
        ancho: ancho_medio,
        alto: 44.0 * e,
    };
    let senales = RectF {
        x: m,
        y: mandos.y + mandos.alto + 12.0 * e,
        ancho: ancho_medio,
        alto: (abajo - (mandos.y + mandos.alto + 12.0 * e)).max(0.0),
    };
    let cab = |x: f32, w: f32| RectF {
        x,
        y: arriba,
        ancho: w,
        alto: CABECERA * e,
    };
    let lista = |x: f32, w: f32| RectF {
        x,
        y: arriba + CABECERA * e,
        ancho: w,
        alto: (alto_columna - CABECERA * e).max(0.0),
    };
    let alto_imagen = (ancho_fotos - 16.0 * e) * 9.0 / 16.0;
    Disposicion {
        medio,
        tiempo,
        mandos,
        senales,
        cabecera_texto: cab(x_texto, ancho_texto),
        texto: lista(x_texto, ancho_texto),
        cabecera_fotos: cab(x_fotos, ancho_fotos),
        fotos: lista(x_fotos, ancho_fotos),
        fila_foto: 8.0 * e + alto_imagen + 24.0 * e,
    }
}

/// Todo lo de la ventana abierta.
struct Vista {
    raiz: PathBuf,
    clase: Clase,
    medio: Medio,
    seguir: bool,
    /// Ensenar la captura del minuto aunque haya video.
    ver_capturas: bool,
    scroll_texto: f32,
    scroll_fotos: f32,
    /// Hasta cuando no se sigue al sonido: se movio una lista a mano.
    a_mano_hasta: Option<Instant>,
    /// Ir a lo que suena en la siguiente vuelta, aunque este parado (tras
    /// un salto).
    ir_ya: bool,
    /// El marcador que se esta escribiendo: su minuto y su nota.
    escribiendo: Option<(i64, String)>,
    /// La M que abrio el marcador llega tambien como caracter: se traga.
    tragar: Option<char>,
    arrastrando_tiempo: bool,
    raton: (f32, f32),
    zonas: Vec<(RectF, Zona)>,
    disp: Disposicion,
    /// Lo alto de cada fila del texto, para el ancho con que se midio.
    altos_texto: RefCell<(u32, Vec<f32>)>,
    miniaturas: Vec<Option<Imagen>>,
    grande: Option<(usize, Imagen)>,
    pedida: Option<usize>,
    cargador: Option<Cargador>,
    aviso: Option<(String, Instant)>,
    /// La ultima vez que se miro si el cuaderno cambio, y como estaba.
    mirado: Instant,
    leido: Option<std::time::SystemTime>,
}

impl Vista {
    fn nueva(recursos: &Recursos, raiz: PathBuf, clase: Clase, ms: i64) -> Vista {
        let medio = Medio::abrir(&recursos.d3d(), &clase, ms);
        let lado = recursos.motor().lado_maximo_bitmap().min(2560);
        let cargador = Cargador::lanzar(
            clase.capturas.iter().map(|(_, r)| r.clone()).collect(),
            lado,
        );
        let leido = mtime(&raiz, &clase.proyecto);
        Vista {
            raiz,
            miniaturas: (0..clase.capturas.len()).map(|_| None).collect(),
            clase,
            medio,
            seguir: true,
            ver_capturas: false,
            scroll_texto: 0.0,
            scroll_fotos: 0.0,
            a_mano_hasta: None,
            ir_ya: true,
            escribiendo: None,
            tragar: None,
            arrastrando_tiempo: false,
            raton: (-1.0, -1.0),
            zonas: Vec::new(),
            disp: disponer(ANCHO as f32, ALTO as f32, 1.0),
            altos_texto: RefCell::new((0, Vec::new())),
            grande: None,
            pedida: None,
            cargador,
            aviso: None,
            mirado: Instant::now(),
            leido,
        }
    }

    fn duracion(&self) -> i64 {
        let d = self.medio.duracion_ms();
        if d > 0 { d } else { self.clase.duracion_ms }
    }

    fn avisar(&mut self, t: String) {
        self.aviso = Some((t, Instant::now()));
    }

    /// Ir a `ms`, y que el texto y la tira vayan con el.
    fn ir_a(&mut self, ms: i64, aproximado: bool) {
        self.medio
            .ir_a(clase::saltar(ms, 0, self.duracion()), aproximado);
        self.a_mano_hasta = None;
        self.ir_ya = true;
    }

    fn saltar(&mut self, delta: i64) {
        let ms = clase::saltar(self.medio.posicion_ms(), delta, self.duracion());
        self.ir_a(ms, false);
    }

    /// El trozo anterior o el siguiente al que suena.
    fn otro_trozo(&mut self, adelante: bool) {
        let ms = self.medio.posicion_ms();
        let tiempos: Vec<i64> = self.clase.lineas.iter().filter_map(|(t, _)| *t).collect();
        let destino = if adelante {
            tiempos.iter().find(|t| **t > ms + 200).copied()
        } else {
            // El de antes del que suena: si se acaba de empezar uno, el
            // anterior; si no, el principio de este.
            let actual = tiempos.iter().rposition(|t| *t <= ms);
            match actual {
                Some(i) if ms - tiempos[i] < 1_500 && i > 0 => Some(tiempos[i - 1]),
                Some(i) => Some(tiempos[i]),
                None => None,
            }
        };
        if let Some(t) = destino {
            self.ir_a(t, false);
        }
    }

    fn alto_vista_texto(&self) -> f32 {
        self.disp.texto.alto
    }

    fn altos_fotos(&self) -> Vec<f32> {
        vec![self.disp.fila_foto; self.clase.capturas.len()]
    }

    /// Movio una lista con la rueda: la otra la sigue (con «Seguir») y se
    /// deja de seguir al sonido un rato.
    fn rueda(&mut self, en_texto: bool, delta: i32, e: f32) {
        let paso = delta as f32 / 120.0 * 72.0 * e;
        let altos_texto = self.altos_texto.borrow().1.clone();
        let altos_fotos = self.altos_fotos();
        let (vt, vf) = (self.alto_vista_texto(), self.disp.fotos.alto);
        if en_texto {
            self.scroll_texto =
                (self.scroll_texto - paso).clamp(0.0, clase::tope(&altos_texto, vt));
        } else {
            self.scroll_fotos =
                (self.scroll_fotos - paso).clamp(0.0, clase::tope(&altos_fotos, vf));
        }
        self.a_mano_hasta = Some(Instant::now() + A_MANO);
        if !self.seguir {
            return;
        }
        let tf = self.clase.tiempos_capturas();
        if en_texto {
            if let Some(s) = clase::seguir(
                &self.clase.tiempos_filas,
                &altos_texto,
                vt,
                self.scroll_texto,
                &tf,
                &altos_fotos,
                vf,
            ) {
                self.scroll_fotos = s;
            }
        } else if let Some(s) = clase::seguir(
            &tf,
            &altos_fotos,
            vf,
            self.scroll_fotos,
            &self.clase.tiempos_filas,
            &altos_texto,
            vt,
        ) {
            self.scroll_texto = s;
        }
    }

    /// Lo de cada vuelta con «Seguir»: el texto y la tira se deslizan hasta
    /// lo que suena (mientras suena, o justo despues de un salto).
    fn seguir_al_sonido(&mut self) -> bool {
        if self.a_mano_hasta.is_some_and(|h| Instant::now() < h) {
            return false;
        }
        self.a_mano_hasta = None;
        if !self.seguir {
            self.ir_ya = false;
            return false;
        }
        if !self.ir_ya && !self.medio.sonando() {
            return false;
        }
        let ms = self.medio.posicion_ms();
        let altos_texto = self.altos_texto.borrow().1.clone();
        let altos_fotos = self.altos_fotos();
        let dt = clase::scroll_para(
            &self.clase.tiempos_filas,
            &altos_texto,
            self.alto_vista_texto(),
            ms,
            false,
        );
        let df = clase::scroll_para(
            &self.clase.tiempos_capturas(),
            &altos_fotos,
            self.disp.fotos.alto,
            ms,
            false,
        );
        let (t, f) = (
            clase::acercar(self.scroll_texto, dt, 0.25),
            clase::acercar(self.scroll_fotos, df, 0.25),
        );
        let cambio = t != self.scroll_texto || f != self.scroll_fotos;
        self.scroll_texto = t;
        self.scroll_fotos = f;
        if !cambio {
            self.ir_ya = false;
        }
        cambio
    }

    fn empezar_marcador(&mut self) {
        let ms = self.medio.posicion_ms();
        // Al escribir, el sonido se para: se esta tomando una nota.
        if self.medio.sonando() {
            self.medio.alternar();
        }
        let ya = self
            .clase
            .senales
            .iter()
            .find(|s| s.ms == ms && !s.banderita)
            .map(|s| s.texto.clone())
            .unwrap_or_default();
        self.escribiendo = Some((ms, ya));
    }

    fn guardar_marcador(&mut self, textos: &Catalogo) {
        let Some((ms, texto)) = self.escribiendo.take() else {
            return;
        };
        let hecho = cambiar_la_nota(&self.raiz, &self.clase.proyecto, &self.clase.id, |m| {
            clase::poner_marcador(&mut m.resto, ms, &texto);
            true
        });
        match hecho {
            Ok(m) => {
                self.clase.poner_senales(senales_de(&m));
                self.leido = mtime(&self.raiz, &self.clase.proyecto);
                self.avisar(textos.t("estudiar-marcador-guardado"));
            }
            Err(e) => {
                tracing::warn!(?e, "el marcador de la clase no se guardo");
                self.avisar(textos.t("estudiar-no-se-guardo"));
            }
        }
    }

    fn quitar_senal(&mut self, ms: i64, textos: &Catalogo) {
        let banderita = self.clase.senales.iter().any(|s| s.ms == ms && s.banderita);
        let hecho = cambiar_la_nota(&self.raiz, &self.clase.proyecto, &self.clase.id, |m| {
            if banderita {
                crate::voz::quitar_marca(m, ms)
            } else {
                clase::quitar_marcador(&mut m.resto, ms)
            }
        });
        match hecho {
            Ok(m) => {
                self.clase.poner_senales(senales_de(&m));
                self.leido = mtime(&self.raiz, &self.clase.proyecto);
            }
            Err(e) => {
                tracing::warn!(?e, "la senal de la clase no se quito");
                self.avisar(textos.t("estudiar-no-se-guardo"));
            }
        }
    }

    /// Si el cuaderno cambio por fuera (llego el texto, un marcador del
    /// movil), se relee lo que no suena: el texto, las senales y las
    /// capturas nuevas.
    fn releer_si_cambio(&mut self, motor: &MotorRender) -> bool {
        if self.mirado.elapsed() < Duration::from_millis(1500) || self.escribiendo.is_some() {
            return false;
        }
        self.mirado = Instant::now();
        let ahora = mtime(&self.raiz, &self.clase.proyecto);
        if ahora == self.leido {
            return false;
        }
        self.leido = ahora;
        let Ok(nueva) = Clase::leer(&self.raiz, &self.clase.proyecto, &self.clase.id) else {
            return false;
        };
        let otras_capturas = nueva.capturas != self.clase.capturas;
        self.clase.lineas = nueva.lineas;
        self.clase.nombre = nueva.nombre;
        self.clase.poner_senales(nueva.senales);
        if otras_capturas {
            self.clase.capturas = nueva.capturas;
            self.miniaturas = (0..self.clase.capturas.len()).map(|_| None).collect();
            self.grande = None;
            self.pedida = None;
            self.cargador = Cargador::lanzar(
                self.clase.capturas.iter().map(|(_, r)| r.clone()).collect(),
                motor.lado_maximo_bitmap().min(2560),
            );
        }
        *self.altos_texto.borrow_mut() = (0, Vec::new());
        true
    }

    /// Las imagenes que llegaron de su hilo, y pedir la grande que toca.
    fn imagenes(&mut self, motor: &MotorRender) -> bool {
        let mut cambio = false;
        if let Some(c) = &self.cargador {
            for llegada in c.llegan.try_iter() {
                match llegada {
                    Cargada::Miniatura(i, img) => {
                        if let Some(sitio) = self.miniaturas.get_mut(i) {
                            *sitio = subir(motor, &img);
                        }
                    }
                    Cargada::Grande(i, img) => {
                        if let Some(b) = subir(motor, &img) {
                            self.grande = Some((i, b));
                        }
                        if self.pedida == Some(i) {
                            self.pedida = None;
                        }
                    }
                }
                cambio = true;
            }
        }
        if let Some(i) = self.captura_a_la_vista()
            && self.grande.as_ref().is_none_or(|(g, _)| *g != i)
            && self.pedida != Some(i)
            && let Some(c) = &self.cargador
            && c.pedir.send(i).is_ok()
        {
            self.pedida = Some(i);
        }
        cambio
    }

    /// La captura que se ensena en grande, si se ensena alguna.
    fn captura_a_la_vista(&self) -> Option<usize> {
        if self.medio.es_video() && !self.ver_capturas {
            return None;
        }
        clase::ultimo_antes(&self.clase.tiempos_capturas(), self.medio.posicion_ms())
    }

    fn zona_en(&self, p: (f32, f32)) -> Option<Zona> {
        self.zonas
            .iter()
            .rev()
            .find(|(r, _)| dentro(*r, p))
            .map(|(_, z)| *z)
    }
}

fn mtime(raiz: &Path, proyecto: &str) -> Option<std::time::SystemTime> {
    std::fs::metadata(almacen::carpeta(raiz, proyecto).join("guardados.jsonl"))
        .and_then(|m| m.modified())
        .ok()
}

fn bucle(recursos: &Recursos, textos: &Catalogo, pedido: &mut Pedido) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .monitor_en(pixpin_shell::posicion_del_cursor())
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien;
    let e = escala as f32 / 100.0;
    let raiz = pedido.ubicacion.raiz().to_path_buf();
    let clase = Clase::leer(&raiz, &pedido.proyecto, &pedido.id)?;
    let mut v = Vista::nueva(recursos, raiz, clase, pedido.ms);
    let mut marco = crate::ventanita::centrado(monitor.area_trabajo, ANCHO, ALTO, escala);
    let mut ventana = VentanaOverlay::nueva_normal(marco, &textos.t("estudiar-titulo"))
        .context("no se pudo abrir la ventana de estudiar la clase")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para estudiar la clase")?;
    ventana.mostrar();
    ventana.enfocar();
    ABIERTA.store(ventana.handle().0 as isize, Ordering::SeqCst);
    tracing::info!(
        capturas = v.clase.capturas.len(),
        trozos = v.clase.lineas.len(),
        con_video = v.medio.es_video(),
        "ventana de estudiar la clase abierta"
    );
    v.disp = disponer(marco.ancho as f32, marco.alto as f32, e);

    let mut moviendo: Option<(Punto, Rect)> = None;
    let mut vivo = true;
    let mut pintar = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        // Otra clase pedida con la ventana abierta.
        let otra = OTRA.lock().ok().and_then(|mut g| g.take());
        if let Some(o) = otra {
            match Clase::leer(&v.raiz, &o.proyecto, &o.id) {
                Ok(c) => {
                    let disp = v.disp;
                    v = Vista::nueva(recursos, v.raiz.clone(), c, o.ms);
                    v.disp = disp;
                    *pedido = o;
                }
                Err(e) => tracing::warn!(?e, "la otra clase no se pudo leer"),
            }
            pintar = true;
        }
        for (hw, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hw != ventana.handle() {
                continue;
            }
            let local = |p: Punto, m: Rect| ((p.x - m.x) as f32, (p.y - m.y) as f32);
            match ev {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::RatonMovido(p) => {
                    if let Some((desde, origen)) = moviendo {
                        marco = Rect {
                            x: origen.x + (p.x - desde.x),
                            y: origen.y + (p.y - desde.y),
                            ..origen
                        };
                        ventana.mover(marco);
                    }
                    let l = local(p, marco);
                    if v.arrastrando_tiempo {
                        let t = v.disp.tiempo;
                        let ms = clase::ms_de_la_barra(l.0 - t.x, t.ancho, v.duracion());
                        v.ir_a(ms, true);
                    }
                    if v.zona_en(l) != v.zona_en(v.raton) {
                        pintar = true;
                    }
                    v.raton = l;
                }
                EventoOverlay::BotonPulsado(p) => {
                    let l = local(p, marco);
                    v.raton = l;
                    pintar = true;
                    match v.zona_en(l) {
                        Some(Zona::Barra) => {
                            moviendo = Some((p, marco));
                            ventana.capturar_raton();
                        }
                        Some(Zona::Tiempo) => {
                            let t = v.disp.tiempo;
                            let senales: Vec<i64> = v
                                .clase
                                .capturas
                                .iter()
                                .map(|(ms, _)| *ms)
                                .chain(v.clase.senales.iter().map(|s| s.ms))
                                .collect();
                            let ms = clase::pegar_a_senal(
                                l.0 - t.x,
                                t.ancho,
                                v.duracion(),
                                &senales,
                                PEGAR * e,
                            );
                            v.ir_a(ms, false);
                            v.arrastrando_tiempo = true;
                            ventana.capturar_raton();
                        }
                        Some(z) => pulsar(&mut v, z, textos, &mut vivo),
                        None => {}
                    }
                }
                EventoOverlay::BotonSoltado(p) => {
                    if moviendo.take().is_some() {
                        ventana.soltar_raton();
                    }
                    if v.arrastrando_tiempo {
                        v.arrastrando_tiempo = false;
                        ventana.soltar_raton();
                        // Al soltar, el fotograma exacto.
                        let l = local(p, marco);
                        let t = v.disp.tiempo;
                        let ms = clase::ms_de_la_barra(l.0 - t.x, t.ancho, v.duracion());
                        v.ir_a(ms, false);
                    }
                }
                EventoOverlay::BotonDerechoPulsado(p) => {
                    // Clic derecho en una senal: quitarla (como mantener
                    // pulsada una banderita en el movil).
                    let l = local(p, marco);
                    if let Some(Zona::Ir(ms)) = v.zona_en(l)
                        && v.clase.senales.iter().any(|s| s.ms == ms)
                    {
                        v.quitar_senal(ms, textos);
                        pintar = true;
                    }
                }
                EventoOverlay::Rueda(delta) => match v.zona_en(v.raton) {
                    Some(Zona::ListaFotos | Zona::Captura(_)) => {
                        v.rueda(false, delta, e);
                        pintar = true;
                    }
                    Some(_) if dentro(v.disp.texto, v.raton) => {
                        v.rueda(true, delta, e);
                        pintar = true;
                    }
                    _ => {}
                },
                EventoOverlay::Tecla { vk, ctrl, .. } => {
                    pintar = true;
                    tecla(&mut v, vk, ctrl, textos, &mut vivo);
                }
                EventoOverlay::Caracter(c) => {
                    if v.tragar.take().is_some_and(|t| t.eq_ignore_ascii_case(&c)) {
                        continue;
                    }
                    if let Some((_, texto)) = v.escribiendo.as_mut()
                        && !c.is_control()
                        && texto.chars().count() < clase::NOTA_MAXIMA
                    {
                        texto.push(c);
                        pintar = true;
                    }
                }
                _ => {}
            }
        }
        if !vivo {
            break;
        }
        // El video que no se pudo decodificar: se oye la nota.
        if v.medio.es_video() && v.medio.fallo() {
            tracing::warn!("el video de la clase no se puede ver aqui; va el audio");
            let ms = v.medio.posicion_ms();
            v.medio = Medio::audio(&v.clase, ms);
            v.avisar(textos.t("estudiar-sin-video"));
            pintar = true;
        }
        if v.medio.latido(&motor) {
            pintar = true;
        }
        if v.imagenes(&motor) {
            pintar = true;
        }
        if v.releer_si_cambio(&motor) {
            pintar = true;
        }
        if v.seguir_al_sonido() {
            pintar = true;
        }
        if v.aviso
            .as_ref()
            .is_some_and(|(_, t)| t.elapsed() >= AVISO_DURA)
        {
            v.aviso = None;
            pintar = true;
        }
        if pintar {
            pintar = false;
            let tema = if pixpin_shell::entorno::tema_claro() {
                &CLARO
            } else {
                &OSCURO
            };
            v.disp = disponer(marco.ancho as f32, marco.alto as f32, e);
            if let Ok(d) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&d, |p: &Pintor| {
                    pintar_todo(
                        p,
                        &mut v,
                        marco.ancho as f32,
                        marco.alto as f32,
                        tema,
                        e,
                        textos,
                    )
                });
                let _ = superficie.presentar();
            }
        }
        pedido.ms = v.medio.posicion_ms();
        let latido = if v.medio.sonando() || v.ir_ya || v.arrastrando_tiempo {
            16
        } else if v.pedida.is_some() || v.aviso.is_some() {
            100
        } else {
            400
        };
        pixpin_shell::overlay::esperar_eventos(Some(latido));
    }
    ventana.ocultar();
    tracing::info!("ventana de estudiar la clase cerrada");
    Ok(())
}

fn pulsar(v: &mut Vista, z: Zona, textos: &Catalogo, vivo: &mut bool) {
    match z {
        Zona::Cerrar => *vivo = false,
        Zona::Tocar | Zona::Medio => v.medio.alternar(),
        Zona::Atras => v.saltar(-SALTO_LARGO),
        Zona::Adelante => v.saltar(SALTO_LARGO),
        Zona::Velocidad => v.medio.otra_velocidad(),
        Zona::Marcar => {
            if v.escribiendo.is_some() {
                v.guardar_marcador(textos);
            } else {
                v.empezar_marcador();
            }
        }
        Zona::Seguir => {
            v.seguir = !v.seguir;
            if v.seguir {
                v.a_mano_hasta = None;
                v.ir_ya = true;
            }
        }
        Zona::CambiarVista => v.ver_capturas = !v.ver_capturas,
        Zona::Ir(ms) => v.ir_a(ms, false),
        Zona::Quitar(ms) => v.quitar_senal(ms, textos),
        Zona::Captura(i) => {
            if let Some((ms, _)) = v.clase.capturas.get(i) {
                let ms = *ms;
                v.ver_capturas = true;
                v.ir_a(ms, false);
            }
        }
        Zona::Barra | Zona::Tiempo | Zona::ListaTexto | Zona::ListaFotos => {}
    }
}

fn tecla(v: &mut Vista, vk: u32, ctrl: bool, textos: &Catalogo, vivo: &mut bool) {
    // Escribiendo la nota de un marcador, las letras son letras.
    if v.escribiendo.is_some() {
        match vk {
            VK_ENTRAR => v.guardar_marcador(textos),
            VK_ESCAPE => v.escribiendo = None,
            VK_RETROCESO => {
                if let Some((_, texto)) = v.escribiendo.as_mut() {
                    texto.pop();
                }
            }
            _ => {}
        }
        return;
    }
    if ctrl {
        return;
    }
    match vk {
        VK_ESPACIO | VK_K => v.medio.alternar(),
        VK_IZQUIERDA => v.saltar(-SALTO_CORTO),
        VK_DERECHA => v.saltar(SALTO_CORTO),
        VK_J => v.saltar(-SALTO_LARGO),
        VK_L => v.saltar(SALTO_LARGO),
        VK_ARRIBA => v.otro_trozo(false),
        VK_ABAJO => v.otro_trozo(true),
        VK_M => {
            v.tragar = Some('m');
            v.empezar_marcador();
        }
        VK_S => {
            v.tragar = Some('s');
            pulsar(v, Zona::Seguir, textos, vivo);
        }
        VK_ESCAPE => *vivo = false,
        _ => {}
    }
    // Las letras de los atajos tambien llegan como caracter: sin marcador
    // abierto no hacen nada, pero la de la M no puede acabar en su nota.
    if !matches!(vk, VK_M | VK_S) {
        v.tragar = None;
    }
}

// --- Pintar ------------------------------------------------------------------------

fn pintar_todo(
    p: &Pintor,
    v: &mut Vista,
    ancho: f32,
    alto: f32,
    tema: &Tema,
    e: f32,
    textos: &Catalogo,
) {
    v.zonas.clear();
    p.rellenar(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho,
            alto,
        },
        tema.chat,
    );
    // La barra: el titulo y cerrar. Lo que no es boton la arrastra.
    let barra = RectF {
        x: 0.0,
        y: 0.0,
        ancho,
        alto: BARRA * e,
    };
    p.rellenar(barra, tema.barra);
    v.zonas.push((barra, Zona::Barra));
    let titulo = format!("{} · {}", textos.t("estudiar-titulo"), v.clase.nombre);
    let (_, th) = p.medir_texto(&titulo, 16.0 * e);
    p.icono(
        &mi::MENU_BOOK,
        RectF {
            x: MARGEN * e,
            y: (barra.alto - 20.0 * e) / 2.0,
            ancho: 20.0 * e,
            alto: 20.0 * e,
        },
        tema.enviar,
    );
    p.texto_linea(
        &titulo,
        MARGEN * e + 30.0 * e,
        (barra.alto - th) / 2.0,
        16.0 * e,
        ancho - 200.0 * e,
        tema.texto,
    );
    let cerrar = RectF {
        x: ancho - BARRA * e,
        y: 0.0,
        ancho: BARRA * e,
        alto: BARRA * e,
    };
    if dentro(cerrar, v.raton) {
        p.rellenar(cerrar, tema.cerrar_sobre);
    }
    p.icono(
        &mi::CLOSE,
        RectF {
            x: cerrar.x + (cerrar.ancho - 20.0 * e) / 2.0,
            y: (cerrar.alto - 20.0 * e) / 2.0,
            ancho: 20.0 * e,
            alto: 20.0 * e,
        },
        tema.texto,
    );
    v.zonas.push((cerrar, Zona::Cerrar));

    let posicion = v.medio.posicion_ms();
    pintar_medio(p, v, e, textos);
    pintar_tiempo(p, v, tema, e, posicion);
    pintar_mandos(p, v, tema, e, textos, posicion);
    pintar_senales(p, v, tema, e, textos, posicion);
    pintar_texto(p, v, tema, e, textos, posicion);
    pintar_fotos(p, v, tema, e, textos, posicion);

    if let Some((texto, _)) = &v.aviso {
        let tam = 14.0 * e;
        let (w, h) = p.medir_texto(texto, tam);
        let pastilla = RectF {
            x: (ancho - w - 36.0 * e) / 2.0,
            y: alto - h - 40.0 * e,
            ancho: w + 36.0 * e,
            alto: h + 18.0 * e,
        };
        p.rellenar_redondeado(pastilla, pastilla.alto / 2.0, hex(0x248a3d));
        p.texto(
            texto,
            pastilla.x + 18.0 * e,
            pastilla.y + 9.0 * e,
            tam,
            Color::BLANCO,
        );
    }
}

/// Encaja `w × h` dentro de `caja` sin deformarlo, centrado.
fn encajar(caja: RectF, w: u32, h: u32) -> RectF {
    if w == 0 || h == 0 {
        return caja;
    }
    let k = (caja.ancho / w as f32).min(caja.alto / h as f32);
    let (dw, dh) = (w as f32 * k, h as f32 * k);
    RectF {
        x: caja.x + (caja.ancho - dw) / 2.0,
        y: caja.y + (caja.alto - dh) / 2.0,
        ancho: dw,
        alto: dh,
    }
}

/// Lo que se ve: el fotograma del video, o la captura del minuto en grande
/// (la grande si ya llego, si no su miniatura), o el aviso de solo audio.
fn pintar_medio(p: &Pintor, v: &mut Vista, e: f32, textos: &Catalogo) {
    let caja = v.disp.medio;
    p.rellenar_redondeado(caja, 10.0 * e, hex(0x0b0f14));
    let recortado = p.empujar_recorte_redondeado(caja, 10.0 * e);
    let mut algo = false;
    if let Some(i) = v.captura_a_la_vista() {
        let img = match &v.grande {
            Some((g, img)) if *g == i => Some(img),
            _ => v.miniaturas.get(i).and_then(|m| m.as_ref()),
        };
        if let Some((b, w, h)) = img {
            p.bitmap(b, encajar(caja, *w, *h), None, false);
            algo = true;
        }
    } else if let Medio::Video {
        bitmap: Some(b), r, ..
    } = &mut v.medio
        && let Some((w, h)) = r.dimensiones()
    {
        p.bitmap(b, encajar(caja, w, h), None, false);
        algo = true;
    }
    if recortado {
        p.soltar_recorte_redondeado();
    }
    if !algo {
        let (icono, texto) = match (&v.medio, v.clase.capturas.is_empty()) {
            (Medio::Nada, _) => (&mi::MIC, textos.t("estudiar-sin-sonido")),
            (Medio::Video { .. }, _) => (&mi::VIDEOCAM, textos.t("estudiar-cargando")),
            (_, true) => (&mi::MIC, textos.t("estudiar-solo-audio")),
            (_, false) => (&mi::IMAGE, textos.t("estudiar-cargando")),
        };
        let lado = 40.0 * e;
        p.icono(
            icono,
            RectF {
                x: caja.x + (caja.ancho - lado) / 2.0,
                y: caja.y + caja.alto / 2.0 - lado,
                ancho: lado,
                alto: lado,
            },
            con_alfa(Color::BLANCO, 0.6),
        );
        let (w, _) = p.medir_texto(&texto, 14.0 * e);
        p.texto(
            &texto,
            caja.x + (caja.ancho - w) / 2.0,
            caja.y + caja.alto / 2.0 + 8.0 * e,
            14.0 * e,
            con_alfa(Color::BLANCO, 0.7),
        );
    }
    v.zonas.push((caja, Zona::Medio));
    // Con video y capturas, una ficha arriba a la derecha cambia entre los
    // dos: «Ver la captura» / «Ver el vídeo».
    if v.medio.es_video() && !v.clase.capturas.is_empty() {
        let (icono, rotulo) = if v.ver_capturas {
            (&mi::VIDEOCAM, textos.t("estudiar-ver-video"))
        } else {
            (&mi::IMAGE, textos.t("estudiar-ver-captura"))
        };
        let tam = 12.5 * e;
        let (tw, th) = p.medir_texto(&rotulo, tam);
        let ficha = RectF {
            x: caja.x + caja.ancho - tw - 52.0 * e,
            y: caja.y + 10.0 * e,
            ancho: tw + 42.0 * e,
            alto: 28.0 * e,
        };
        let fondo = if dentro(ficha, v.raton) { 0.85 } else { 0.65 };
        p.rellenar_redondeado(ficha, ficha.alto / 2.0, con_alfa(hex(0x000000), fondo));
        p.icono(
            icono,
            RectF {
                x: ficha.x + 10.0 * e,
                y: ficha.y + (ficha.alto - 16.0 * e) / 2.0,
                ancho: 16.0 * e,
                alto: 16.0 * e,
            },
            Color::BLANCO,
        );
        p.texto(
            &rotulo,
            ficha.x + 32.0 * e,
            ficha.y + (ficha.alto - th) / 2.0,
            tam,
            Color::BLANCO,
        );
        v.zonas.push((ficha, Zona::CambiarVista));
    }
}

/// La barra del tiempo: lo oido, una raya por captura, una banderita por
/// senal y el tirador.
fn pintar_tiempo(p: &Pintor, v: &mut Vista, tema: &Tema, e: f32, posicion: i64) {
    let caja = v.disp.tiempo;
    let duracion = v.duracion();
    let pista = RectF {
        x: caja.x,
        y: caja.y + caja.alto / 2.0 - 3.0 * e,
        ancho: caja.ancho,
        alto: 6.0 * e,
    };
    p.rellenar_redondeado(pista, 3.0 * e, con_alfa(tema.apagado, 0.35));
    let x_de = |ms: i64| caja.x + clase::x_en_la_barra(ms, caja.ancho, duracion);
    let xp = x_de(posicion);
    p.rellenar_redondeado(
        RectF {
            ancho: (xp - caja.x).max(0.0),
            ..pista
        },
        3.0 * e,
        tema.enviar,
    );
    for (ms, _) in &v.clase.capturas {
        let x = x_de(*ms);
        p.rellenar(
            RectF {
                x: x - 1.0 * e,
                y: caja.y + 4.0 * e,
                ancho: 2.0 * e,
                alto: caja.alto - 8.0 * e,
            },
            con_alfa(tema.texto, 0.7),
        );
    }
    for s in &v.clase.senales {
        let x = x_de(s.ms);
        p.rellenar(
            RectF {
                x: x - 1.0 * e,
                y: caja.y,
                ancho: 2.0 * e,
                alto: caja.alto / 2.0,
            },
            AMBAR,
        );
        p.circulo((x, caja.y + 3.0 * e), 4.0 * e, AMBAR);
    }
    let encima = dentro(caja, v.raton) || v.arrastrando_tiempo;
    p.circulo(
        (xp, caja.y + caja.alto / 2.0),
        (if encima { 8.0 } else { 6.0 }) * e,
        tema.enviar,
    );
    v.zonas.push((caja, Zona::Tiempo));
}

/// Un boton redondeado de los mandos: icono y, si lo hay, rotulo.
#[allow(clippy::too_many_arguments)]
fn boton(
    p: &Pintor,
    v: &mut Vista,
    caja: RectF,
    zona: Zona,
    icono: &pixpin_render::icono::Icono,
    rotulo: &str,
    encendido: bool,
    tema: &Tema,
    e: f32,
) {
    let fondo = if encendido {
        tema.enviar
    } else if dentro(caja, v.raton) {
        tema.fila_sobre
    } else {
        tema.lista
    };
    let tinta = if encendido { Color::BLANCO } else { tema.texto };
    p.rellenar_redondeado(caja, 10.0 * e, fondo);
    let lado = 20.0 * e;
    let tam = 13.5 * e;
    let (tw, th) = if rotulo.is_empty() {
        (0.0, 0.0)
    } else {
        p.medir_texto(rotulo, tam)
    };
    let hueco = if rotulo.is_empty() { 0.0 } else { 6.0 * e };
    let x = caja.x + (caja.ancho - lado - hueco - tw) / 2.0;
    p.icono(
        icono,
        RectF {
            x,
            y: caja.y + (caja.alto - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        tinta,
    );
    if !rotulo.is_empty() {
        p.texto(
            rotulo,
            x + lado + hueco,
            caja.y + (caja.alto - th) / 2.0,
            tam,
            tinta,
        );
    }
    v.zonas.push((caja, zona));
}

/// Lo que mide un boton con rotulo.
fn ancho_de(p: &Pintor, rotulo: &str, e: f32) -> f32 {
    p.medir_texto(rotulo, 13.5 * e).0 + 20.0 * e + 6.0 * e + 24.0 * e
}

fn pintar_mandos(p: &Pintor, v: &mut Vista, tema: &Tema, e: f32, textos: &Catalogo, posicion: i64) {
    let caja = v.disp.mandos;
    let alto = 40.0 * e;
    let y = caja.y + (caja.alto - alto) / 2.0;
    let mut x = caja.x;
    let mut sitio = |w: f32| {
        let r = RectF {
            x,
            y,
            ancho: w,
            alto,
        };
        x += w + 6.0 * e;
        r
    };
    let atras = sitio(44.0 * e);
    let tocar = sitio(52.0 * e);
    let adelante = sitio(44.0 * e);
    boton(p, v, atras, Zona::Atras, &mi::REPLAY_10, "", false, tema, e);
    let icono = if v.medio.sonando() {
        &mi::PAUSE
    } else {
        &mi::PLAY_ARROW
    };
    boton(p, v, tocar, Zona::Tocar, icono, "", true, tema, e);
    boton(
        p,
        v,
        adelante,
        Zona::Adelante,
        &mi::FORWARD_10,
        "",
        false,
        tema,
        e,
    );
    // 12:34 / 45:00
    let reloj = format!(
        "{} / {}",
        pixpin_voz::marca_de_tiempo(posicion),
        pixpin_voz::marca_de_tiempo(v.duracion())
    );
    let tam = 14.0 * e;
    let (_, th) = p.medir_texto(&reloj, tam);
    p.texto(
        &reloj,
        adelante.x + adelante.ancho + 14.0 * e,
        y + (alto - th) / 2.0,
        tam,
        tema.texto,
    );
    // A la derecha: la velocidad, «Marcador» y «Seguir».
    let velocidad = pixpin_audio::reloj::velocidad_legible(v.medio.velocidad());
    let marcar = textos.t("estudiar-marcador");
    let seguir = textos.t("estudiar-seguir");
    let w_vel = p.medir_texto(&velocidad, 13.5 * e).0 + 24.0 * e;
    let w_marcar = ancho_de(p, &marcar, e);
    let w_seguir = ancho_de(p, &seguir, e);
    let mut x = caja.x + caja.ancho - w_seguir;
    let c_seguir = RectF {
        x,
        y,
        ancho: w_seguir,
        alto,
    };
    x -= w_marcar + 6.0 * e;
    let c_marcar = RectF {
        x,
        y,
        ancho: w_marcar,
        alto,
    };
    x -= w_vel + 6.0 * e;
    let c_vel = RectF {
        x,
        y,
        ancho: w_vel,
        alto,
    };
    let fondo = if dentro(c_vel, v.raton) {
        tema.fila_sobre
    } else {
        tema.lista
    };
    p.rellenar_redondeado(c_vel, 10.0 * e, fondo);
    let (tw, th) = p.medir_texto(&velocidad, 13.5 * e);
    p.texto(
        &velocidad,
        c_vel.x + (c_vel.ancho - tw) / 2.0,
        y + (alto - th) / 2.0,
        13.5 * e,
        tema.texto,
    );
    v.zonas.push((c_vel, Zona::Velocidad));
    let escribiendo = v.escribiendo.is_some();
    boton(
        p,
        v,
        c_marcar,
        Zona::Marcar,
        &mi::BOOKMARK_ADD,
        &marcar,
        escribiendo,
        tema,
        e,
    );
    let encendido = v.seguir;
    boton(
        p,
        v,
        c_seguir,
        Zona::Seguir,
        &mi::LINK,
        &seguir,
        encendido,
        tema,
        e,
    );
}

/// Los marcadores: el que se esta escribiendo arriba y, debajo, los que
/// hay, cada uno con su minuto.
fn pintar_senales(
    p: &Pintor,
    v: &mut Vista,
    tema: &Tema,
    e: f32,
    textos: &Catalogo,
    posicion: i64,
) {
    let caja = v.disp.senales;
    if caja.alto < 40.0 * e {
        return;
    }
    p.rellenar_redondeado(caja, 10.0 * e, tema.lista);
    let m = 12.0 * e;
    let mut y = caja.y + 10.0 * e;
    let titulo = format!(
        "{} · {}",
        textos.t("estudiar-marcadores"),
        v.clase.senales.len()
    );
    p.texto(&titulo, caja.x + m, y, 14.0 * e, tema.texto);
    let pista = textos.t("estudiar-pista-teclas");
    let (pw, _) = p.medir_texto(&pista, 11.5 * e);
    let tw = p.medir_texto(&titulo, 14.0 * e).0;
    if tw + pw + 3.0 * m < caja.ancho {
        p.texto(
            &pista,
            caja.x + caja.ancho - m - pw,
            y + 2.0 * e,
            11.5 * e,
            tema.apagado,
        );
    }
    y += 28.0 * e;
    p.empujar_recorte(caja);
    // El que se esta escribiendo.
    if let Some((ms, texto)) = &v.escribiendo {
        let fila = RectF {
            x: caja.x + m,
            y,
            ancho: caja.ancho - 2.0 * m,
            alto: 36.0 * e,
        };
        p.rellenar_redondeado(fila, 8.0 * e, con_alfa(AMBAR, 0.18));
        let minuto = pixpin_voz::marca_de_tiempo(*ms);
        p.texto(
            &minuto,
            fila.x + 10.0 * e,
            fila.y + 9.0 * e,
            13.0 * e,
            AMBAR,
        );
        let (escrito, color) = if texto.is_empty() {
            (textos.t("estudiar-nota-del-marcador"), tema.apagado)
        } else {
            (format!("{texto}▏"), tema.texto)
        };
        p.texto_linea(
            &escrito,
            fila.x + 64.0 * e,
            fila.y + 9.0 * e,
            13.5 * e,
            fila.ancho - 74.0 * e,
            color,
        );
        y += 42.0 * e;
    }
    if v.clase.senales.is_empty() && v.escribiendo.is_none() {
        p.texto_ajustado(
            &textos.t("estudiar-sin-marcadores"),
            caja.x + m,
            y,
            13.0 * e,
            caja.ancho - 2.0 * m,
            tema.apagado,
        );
    }
    let actual = v.clase.senales.iter().rposition(|s| s.ms <= posicion);
    let senales = v.clase.senales.clone();
    for (k, s) in senales.iter().enumerate() {
        let fila = RectF {
            x: caja.x + m,
            y,
            ancho: caja.ancho - 2.0 * m,
            alto: 30.0 * e,
        };
        if fila.y > caja.y + caja.alto {
            break;
        }
        let encima = dentro(fila, v.raton);
        if actual == Some(k) || encima {
            p.rellenar_redondeado(
                fila,
                8.0 * e,
                if encima {
                    tema.fila_sobre
                } else {
                    con_alfa(AMBAR, 0.12)
                },
            );
        }
        p.icono(
            &mi::FLAG,
            RectF {
                x: fila.x + 8.0 * e,
                y: fila.y + 7.0 * e,
                ancho: 16.0 * e,
                alto: 16.0 * e,
            },
            AMBAR,
        );
        p.texto(
            &pixpin_voz::marca_de_tiempo(s.ms),
            fila.x + 30.0 * e,
            fila.y + 7.0 * e,
            12.5 * e,
            AMBAR,
        );
        let nota = if s.texto.is_empty() {
            textos.t("estudiar-marcador-sin-nota")
        } else {
            s.texto.clone()
        };
        p.texto_linea(
            &nota,
            fila.x + 84.0 * e,
            fila.y + 7.0 * e,
            13.0 * e,
            fila.ancho - 120.0 * e,
            if s.texto.is_empty() {
                tema.apagado
            } else {
                tema.texto
            },
        );
        v.zonas.push((fila, Zona::Ir(s.ms)));
        if encima {
            let x = RectF {
                x: fila.x + fila.ancho - 28.0 * e,
                y: fila.y + 3.0 * e,
                ancho: 24.0 * e,
                alto: 24.0 * e,
            };
            p.icono(
                &mi::CLOSE,
                RectF {
                    x: x.x + 4.0 * e,
                    y: x.y + 4.0 * e,
                    ancho: 16.0 * e,
                    alto: 16.0 * e,
                },
                if dentro(x, v.raton) {
                    tema.cerrar_sobre
                } else {
                    tema.apagado
                },
            );
            v.zonas.push((x, Zona::Quitar(s.ms)));
        }
        y += 32.0 * e;
    }
    p.soltar_recorte();
}

/// Lo que mide la columna del minuto en el texto.
const COLUMNA_MINUTO: f32 = 52.0;
const TAM_TEXTO: f32 = 14.5;

fn pintar_texto(p: &Pintor, v: &mut Vista, tema: &Tema, e: f32, textos: &Catalogo, posicion: i64) {
    let cab = v.disp.cabecera_texto;
    let caja = v.disp.texto;
    p.rellenar_redondeado(
        RectF {
            alto: cab.alto + caja.alto,
            ..cab
        },
        10.0 * e,
        tema.lista,
    );
    let titulo = textos.t("estudiar-transcripcion");
    let (_, th) = p.medir_texto(&titulo, 14.0 * e);
    p.icono(
        &mi::LYRICS,
        RectF {
            x: cab.x + 12.0 * e,
            y: cab.y + (cab.alto - 18.0 * e) / 2.0,
            ancho: 18.0 * e,
            alto: 18.0 * e,
        },
        tema.enviar,
    );
    p.texto(
        &titulo,
        cab.x + 38.0 * e,
        cab.y + (cab.alto - th) / 2.0,
        14.0 * e,
        tema.texto,
    );
    v.zonas.push((caja, Zona::ListaTexto));
    if v.clase.lineas.is_empty() && v.clase.senales.is_empty() {
        p.texto_ajustado(
            &textos.t("estudiar-sin-texto"),
            caja.x + 14.0 * e,
            caja.y + 12.0 * e,
            13.5 * e,
            caja.ancho - 28.0 * e,
            tema.apagado,
        );
        return;
    }
    // Lo alto de cada fila, medido una vez por ancho.
    let ancho_texto = caja.ancho - (COLUMNA_MINUTO + 24.0) * e;
    {
        let mut cache = v.altos_texto.borrow_mut();
        let clave = caja.ancho.round() as u32;
        if cache.0 != clave || cache.1.len() != v.clase.filas.len() {
            let altos = v
                .clase
                .filas
                .iter()
                .map(|f| match f {
                    Fila::Linea(i) => {
                        let t = v.clase.lineas.get(*i).map_or("", |(_, t)| t.as_str());
                        p.medir_texto_ajustado(t, TAM_TEXTO * e, ancho_texto).1 + 14.0 * e
                    }
                    Fila::Marcador(_) => 34.0 * e,
                })
                .collect();
            *cache = (clave, altos);
        }
    }
    let altos = v.altos_texto.borrow().1.clone();
    v.scroll_texto = v.scroll_texto.clamp(0.0, clase::tope(&altos, caja.alto));
    let lineas_ms: Vec<Option<i64>> = v.clase.lineas.iter().map(|(ms, _)| *ms).collect();
    let actual = clase::linea_en(&lineas_ms, posicion);
    p.empujar_recorte(caja);
    let mut y = caja.y - v.scroll_texto;
    let filas = v.clase.filas.clone();
    for (k, f) in filas.iter().enumerate() {
        let alto = altos.get(k).copied().unwrap_or(0.0);
        let fila = RectF {
            x: caja.x + 6.0 * e,
            y,
            ancho: caja.ancho - 12.0 * e,
            alto,
        };
        y += alto;
        if fila.y + fila.alto < caja.y || fila.y > caja.y + caja.alto {
            continue;
        }
        let encima = dentro(fila, v.raton) && dentro(caja, v.raton);
        match f {
            Fila::Linea(i) => {
                let Some((ms, texto)) = v.clase.lineas.get(*i) else {
                    continue;
                };
                let es_actual = actual == Some(*i);
                if es_actual {
                    p.rellenar_redondeado(fila, 8.0 * e, con_alfa(tema.enviar, 0.16));
                    p.rellenar_redondeado(
                        RectF {
                            ancho: 3.0 * e,
                            ..fila
                        },
                        1.5 * e,
                        tema.enviar,
                    );
                } else if encima && ms.is_some() {
                    p.rellenar_redondeado(fila, 8.0 * e, tema.fila_sobre);
                }
                if let Some(ms) = ms {
                    p.texto(
                        &pixpin_voz::marca_de_tiempo(*ms),
                        fila.x + 10.0 * e,
                        fila.y + 8.0 * e,
                        12.0 * e,
                        if es_actual { tema.enviar } else { tema.apagado },
                    );
                    v.zonas.push((fila, Zona::Ir(*ms)));
                }
                // Lo que aun no se ha oido, un poco apagado.
                let futuro = ms.is_some_and(|ms| ms > posicion) && v.medio.sonando();
                p.texto_ajustado(
                    texto,
                    fila.x + COLUMNA_MINUTO * e,
                    fila.y + 7.0 * e,
                    TAM_TEXTO * e,
                    ancho_texto,
                    if futuro {
                        con_alfa(tema.texto, 0.6)
                    } else {
                        tema.texto
                    },
                );
            }
            Fila::Marcador(j) => {
                let Some(s) = v.clase.senales.get(*j) else {
                    continue;
                };
                let marca = RectF {
                    y: fila.y + 3.0 * e,
                    alto: fila.alto - 6.0 * e,
                    ..fila
                };
                p.rellenar_redondeado(
                    marca,
                    8.0 * e,
                    con_alfa(AMBAR, if encima { 0.26 } else { 0.16 }),
                );
                p.icono(
                    &mi::FLAG,
                    RectF {
                        x: marca.x + 10.0 * e,
                        y: marca.y + (marca.alto - 16.0 * e) / 2.0,
                        ancho: 16.0 * e,
                        alto: 16.0 * e,
                    },
                    AMBAR,
                );
                let nota = if s.texto.is_empty() {
                    textos.t("estudiar-marcador-sin-nota")
                } else {
                    s.texto.clone()
                };
                let (_, nh) = p.medir_texto(&nota, 13.0 * e);
                p.texto_linea(
                    &format!("{} · {nota}", pixpin_voz::marca_de_tiempo(s.ms)),
                    marca.x + COLUMNA_MINUTO * e,
                    marca.y + (marca.alto - nh) / 2.0,
                    13.0 * e,
                    ancho_texto,
                    tema.texto,
                );
                v.zonas.push((marca, Zona::Ir(s.ms)));
            }
        }
    }
    p.soltar_recorte();
}

fn pintar_fotos(p: &Pintor, v: &mut Vista, tema: &Tema, e: f32, textos: &Catalogo, posicion: i64) {
    let cab = v.disp.cabecera_fotos;
    let caja = v.disp.fotos;
    p.rellenar_redondeado(
        RectF {
            alto: cab.alto + caja.alto,
            ..cab
        },
        10.0 * e,
        tema.lista,
    );
    let titulo = format!(
        "{} · {}",
        textos.t("estudiar-capturas"),
        v.clase.capturas.len()
    );
    let (_, th) = p.medir_texto(&titulo, 14.0 * e);
    p.icono(
        &mi::PHOTO_LIBRARY,
        RectF {
            x: cab.x + 12.0 * e,
            y: cab.y + (cab.alto - 18.0 * e) / 2.0,
            ancho: 18.0 * e,
            alto: 18.0 * e,
        },
        tema.enviar,
    );
    p.texto(
        &titulo,
        cab.x + 38.0 * e,
        cab.y + (cab.alto - th) / 2.0,
        14.0 * e,
        tema.texto,
    );
    v.zonas.push((caja, Zona::ListaFotos));
    if v.clase.capturas.is_empty() {
        p.texto_ajustado(
            &textos.t("estudiar-sin-capturas"),
            caja.x + 14.0 * e,
            caja.y + 12.0 * e,
            13.0 * e,
            caja.ancho - 28.0 * e,
            tema.apagado,
        );
        return;
    }
    let altos = v.altos_fotos();
    v.scroll_fotos = v.scroll_fotos.clamp(0.0, clase::tope(&altos, caja.alto));
    let actual = clase::ultimo_antes(&v.clase.tiempos_capturas(), posicion);
    p.empujar_recorte(caja);
    let paso = v.disp.fila_foto;
    for (i, (ms, _)) in v.clase.capturas.clone().iter().enumerate() {
        let y = caja.y - v.scroll_fotos + i as f32 * paso;
        if y + paso < caja.y || y > caja.y + caja.alto {
            continue;
        }
        let fila = RectF {
            x: caja.x + 6.0 * e,
            y,
            ancho: caja.ancho - 12.0 * e,
            alto: paso,
        };
        let marco = RectF {
            x: caja.x + 8.0 * e,
            y: y + 8.0 * e,
            ancho: caja.ancho - 16.0 * e,
            alto: paso - 32.0 * e,
        };
        let es_actual = actual == Some(i);
        let encima = dentro(fila, v.raton) && dentro(caja, v.raton);
        if es_actual {
            p.rellenar_redondeado(
                RectF {
                    x: marco.x - 3.0 * e,
                    y: marco.y - 3.0 * e,
                    ancho: marco.ancho + 6.0 * e,
                    alto: marco.alto + 6.0 * e,
                },
                9.0 * e,
                tema.enviar,
            );
        } else if encima {
            p.rellenar_redondeado(fila, 8.0 * e, tema.fila_sobre);
        }
        p.rellenar_redondeado(marco, 6.0 * e, hex(0x0b0f14));
        if let Some(Some((b, w, h))) = v.miniaturas.get(i) {
            p.bitmap(b, encajar(marco, *w, *h), None, false);
        }
        let rotulo = format!("⏱ {}", pixpin_voz::marca_de_tiempo(*ms));
        p.texto(
            &rotulo,
            marco.x + 2.0 * e,
            marco.y + marco.alto + 5.0 * e,
            12.5 * e,
            if es_actual { tema.enviar } else { tema.apagado },
        );
        v.zonas.push((fila, Zona::Captura(i)));
    }
    p.soltar_recorte();
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_nota_de_una_clase_y_lo_suyo_abren_la_clase() {
        let mut voz = cuaderno::Mensaje {
            id: "v".into(),
            ..Default::default()
        };
        assert_eq!(de_una_clase(&voz), None, "una nota de voz cualquiera no");
        voz.resto
            .insert("grabacion".into(), serde_json::json!("clase"));
        assert_eq!(de_una_clase(&voz), Some(("v".into(), 0)));
        let mut foto = cuaderno::Mensaje {
            id: "f".into(),
            ..Default::default()
        };
        foto.resto.insert(
            "enGrabacion".into(),
            serde_json::json!({ "de": "v", "ms": 65_000 }),
        );
        assert_eq!(de_una_clase(&foto), Some(("v".into(), 65_000)));
        // Caso negativo: un enlace roto (sin `de`) no es de ninguna clase.
        foto.resto
            .insert("enGrabacion".into(), serde_json::json!({ "ms": 1 }));
        assert_eq!(de_una_clase(&foto), None);
    }

    #[test]
    fn las_senales_juntan_marcadores_y_banderitas_sin_repetir() {
        let mut m = cuaderno::Mensaje::default();
        clase::poner_marcador(&mut m.resto, 5_000, "ojo");
        crate::voz::poner_marca(&mut m, 5_000);
        crate::voz::poner_marca(&mut m, 1_000);
        let s = senales_de(&m);
        assert_eq!(s.len(), 2, "la banderita del mismo minuto no se repite");
        assert!(s[0].banderita && s[0].ms == 1_000);
        assert!(!s[1].banderita && s[1].texto == "ojo");
    }

    #[test]
    fn las_columnas_caben_y_no_se_pisan() {
        for (w, h, e) in [
            (1360.0, 840.0, 1.0),
            (2040.0, 1260.0, 1.5),
            (1100.0, 700.0, 1.0),
        ] {
            let d = disponer(w, h, e);
            assert!(d.medio.x + d.medio.ancho <= d.texto.x, "{w}");
            assert!(d.texto.x + d.texto.ancho <= d.fotos.x, "{w}");
            assert!(d.fotos.x + d.fotos.ancho <= w, "{w}");
            assert!(d.mandos.y + d.mandos.alto <= h, "{w}");
            assert!(d.fila_foto > 0.0);
        }
    }

    #[test]
    fn encajar_no_deforma() {
        let c = RectF {
            x: 0.0,
            y: 0.0,
            ancho: 160.0,
            alto: 90.0,
        };
        let r = encajar(c, 100, 100);
        assert_eq!((r.ancho, r.alto), (90.0, 90.0));
        assert_eq!(r.x, 35.0);
        // Caso negativo: una imagen vacia se queda con la caja.
        assert_eq!(encajar(c, 0, 10).ancho, 160.0);
    }
}
