//! **El riel de los visores web y del pin de Office** (19-oct-2026): el
//! marcador, el lapiz y el marco, los mismos del PDF y del Word.
//!
//! El usuario: «el lapiz, el marcador y el frame sean universales: que
//! funcionen en todos los formatos y aparezcan en un riel lateral como en el
//! Word y el PDF; con el frame puedo imprimir rapido lo que necesito», «los
//! mismos iconos en todos los visores».
//!
//! # Por que en ventanas propias
//!
//! Un WebView2 (o la vista previa de Office) es una ventana de Windows
//! metida dentro del pin: tapa su sitio y Direct2D no puede pintar encima
//! dentro de la misma ventana. Asi que el riel va en **dos ventanas de
//! DirectComposition**, poseidas por la del visor (siempre encima de ella,
//! se esconden con ella):
//!
//! - **La del riel**: solo lo que ocupa el riel (la isla y los puntos), sin
//!   tapar la pagina. Se pinta con [`crate::lector::pintar_riel_con`], la
//!   misma funcion que el PDF: mismos iconos, mismo tamano, mismo encendido.
//!   Al pasar por un punto crece hacia la izquierda (el punto se adelanta,
//!   como en el PDF) y con la tira de emoticonos abierta cubre la vista
//!   entera. No se activa al pulsarla: el teclado se queda en la pagina.
//! - **La de la tinta**: la vista entera. Con el lapiz pulsado (o poniendo
//!   marcos) recoge el raton y la tinta es la del lienzo ([`Tinta`], la
//!   barra y el panel de siempre); suelto, deja pasar los clics a la pagina
//!   y solo ensena lo dibujado y los marcos. Sin nada, ni se ve.
//!
//! # Los marcos numerados (19-oct-2026)
//!
//! El boton del marco ya no congela la ventana para imprimir un recuadro:
//! son los **marcos del plano** ([`crate::marcos_doc`]), con su misma
//! logica. Pulsado (azul) o con la I, cada arrastre deja un marco con su
//! numero; Supr quita el ultimo, Ctrl+P los imprime todos en grupo, una hoja
//! por marco, y la I o Esc salen. Van en pixeles CSS del documento, como la
//! tinta: se quedan en su sitio al desplazar la pagina, y se guardan en el
//! `.pixpin-marcos` de junto al fichero. Lapiz y marcos no van a la vez.
//!
//! Imprimir, como en el plano: la vista se lleva a cada marco, se pinta sin
//! el riel ni las rayas azules (la tinta si sale) y se fotografia. Un marco
//! mas grande que la vista se fotografia por trozos y se cose. No se cambia
//! el zoom de la pagina para llenar la ventana: en una pagina web el zoom la
//! recoloca (el texto se reparte de nuevo) y el marco ya no casaria con lo
//! que encierra; sale con los pixeles de como se ve.
//!
//! # La pagina de debajo
//!
//! Con una pagina ([`Pagina`]: el visor de HTML y el lector de notas) el
//! riel le pregunta por donde va con un guion ([`guion`]): las marcas son la
//! fraccion del desplazamiento (como las del Word, para el movil y las
//! versiones de antes) y, ademas, su vista exacta (el desplazamiento en
//! pixeles CSS y el zoom de Ctrl+rueda) en el `.pixpin-vistas`
//! ([`crate::marcos_doc::Vistas`]): volver a una marca es volver justo ahi y
//! con ese zoom. La tinta y los marcos van en pixeles CSS del documento, con
//! el zoom de la pagina y el de pellizco, asi que se mueven con ella. Con el
//! lapiz o los marcos pulsados la rueda sigue moviendo la pagina.
//!
//! **Sin retraso al desplazar** (19-oct-2026): con una pagina, lo dibujado
//! y los marcos van DENTRO de ella ([`super::capa_web`]) y el navegador los
//! mueve con el texto en el mismo fotograma; la ventana de la tinta solo
//! pinta el gesto a medias (y las fichas, la pista y lo de encima de la
//! tinta). Si la pagina no llega a tener la capa, se sigue como antes:
//! preguntando por donde va cada 40 ms.
//!
//! La vista previa de Office no dice por donde va (el previsualizador de
//! Office no cuenta su desplazamiento ni su zoom): alli no hay marcas (el
//! boton sale apagado) y la tinta y los marcos van pegados a la ventana,
//! solo mientras el pin esta abierto (en otro sitio de la hoja no casarian).
//!
//! Las marcas se guardan como las del Word, en el `.pixpin-lectura` de
//! junto al documento (`pixpin_docs::lectura`), y la tinta en su
//! `.pixpin-anotado/capa.excalidraw` (`lector_tinta::ruta_de_capa`).
//!
//! **Las de un adjunto del chat (o una nota de un proyecto) viajan**
//! (20-oct-2026): las marcas van ademas en `anot-<uid>.marcas` de su
//! mensaje (`id:fraccion:emoji` separadas por `|`, como las del Word: se
//! leen y escriben por `anotado_del_adjunto::leer`/`escribir`), la tinta en
//! `anot-<uid>` (`anotado_del_adjunto::tinta_de_la_pagina`, en pixeles CSS,
//! sin marco), los marcos en `anot-<uid>.marcos.json` y las vistas exactas
//! (desplazamiento y zoom) en `anot-<uid>.vistas.json`. Lo de antes pasa la
//! primera vez.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use pixpin_codec::imagen::ImagenRgba;
use pixpin_docs::lectura::Marcador;
use pixpin_geom::{Punto, Rect};
use pixpin_motor2d::camara::Camara;
use pixpin_render::{Color, MotorRender, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, FormaCursorWin, VentanaOverlay};
use pixpin_store::Catalogo;
use pixpin_ui::riel_marcas::{DestinoRiel, PUNTO_ELEGIDO, Riel, SALTO};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

use crate::lector;
use crate::lector_tinta::{self, Capa, Tinta};
use crate::marcos_doc::{self, Marcos, Vistas};

use super::capa_web::{self, DelGuion, Deseado};

const VK_ESCAPE: u32 = 0x1B;
const VK_F11: u32 = 0x7A;
const VK_Z: u32 = 0x5A;
const VK_Y: u32 = 0x59;
const VK_A: u32 = 0x41;
const VK_M: u32 = 0x4D;
const VK_G: u32 = 0x47;
/// Cada cuanto se mira si la pagina sigue teniendo el guion del riel (al
/// recargarse se pierde), con la capa ya puesta; sin ella, mas a menudo.
const LATIDO: Duration = Duration::from_millis(1000);
const LATIDO_SIN_CAPA: Duration = Duration::from_millis(250);
/// Lo que se espera para volver a mandar la capa si cayo en la pagina
/// vacia del principio.
const REINTENTO: Duration = Duration::from_millis(200);
/// Lo que se deja a la izquierda del riel para el globo de un boton
/// (pixeles logicos): cabe «Marcos para imprimir (I)» y su traduccion.
const GLOBO_ANCHO: f32 = 240.0;
/// Lo que se espera sin tocar la tinta antes de guardarla.
const PARA_GUARDAR: Duration = Duration::from_millis(1500);
/// Cada cuanto se pregunta a la pagina por donde va, con tinta a la vista.
const MIRAR_SITIO: Duration = Duration::from_millis(40);
/// Una pregunta sin respuesta en este tiempo se da por perdida.
const SIN_RESPUESTA: Duration = Duration::from_millis(600);
/// Pixeles CSS por muesca de rueda con el lapiz pulsado (los de Edge).
const CSS_POR_MUESCA: f32 = 100.0;
/// Lo mas grande que sale la foto de un marco (pixeles por lado): un marco
/// de todo un documento largo no se come la memoria.
const FOTO_MAXIMA: f64 = 16_384.0;
/// Lo que se espera a que la pagina se pinte en su sitio nuevo.
const PARA_PINTARSE: Duration = Duration::from_millis(120);
/// Lo que se espera a que la pagina se repinte en claro antes de las fotos.
const PARA_ACLARARSE: Duration = Duration::from_millis(350);
/// Lo que se espera a que la pagina conteste a un guion al imprimir.
const ESPERA_GUION: Duration = Duration::from_millis(1500);

/// **Con quien se habla debajo**: una pagina que corre guiones y tiene su
/// zoom (el de Ctrl+rueda).
pub trait Pagina {
    fn ejecutar(&self, js: &str, al_volver: Box<dyn FnOnce(String) + 'static>);
    /// El `ZoomFactor` del WebView2 (1 es el 100 %).
    fn zoom(&self) -> f64 {
        1.0
    }
    fn poner_zoom(&self, _zoom: f64) {}
    /// El esquema de color de la pagina (la luna/sol): al imprimir se
    /// fotografia en claro, con sus colores de verdad.
    fn poner_oscuro(&self, _oscuro: bool) {}
    /// Si Ctrl+P ya lo coge quien lleva la pagina (el visor de HTML, por las
    /// teclas del WebView2): entonces el guion no lo manda otra vez.
    fn ctrl_p_propio(&self) -> bool {
        false
    }
}

impl Pagina for pixpin_web::VisorHtml {
    fn ctrl_p_propio(&self) -> bool {
        true
    }
    fn ejecutar(&self, js: &str, al_volver: Box<dyn FnOnce(String) + 'static>) {
        pixpin_web::VisorHtml::ejecutar(self, js, al_volver);
    }
    fn zoom(&self) -> f64 {
        pixpin_web::VisorHtml::zoom(self)
    }
    fn poner_zoom(&self, zoom: f64) {
        pixpin_web::VisorHtml::poner_zoom(self, zoom);
    }
    fn poner_oscuro(&self, oscuro: bool) {
        pixpin_web::VisorHtml::poner_oscuro(self, oscuro);
    }
}

impl Pagina for pixpin_web::pagina::Pagina {
    fn ejecutar(&self, js: &str, al_volver: Box<dyn FnOnce(String) + 'static>) {
        pixpin_web::pagina::Pagina::ejecutar(self, js, al_volver);
    }
    fn zoom(&self) -> f64 {
        pixpin_web::pagina::Pagina::zoom(self)
    }
    fn poner_zoom(&self, zoom: f64) {
        pixpin_web::pagina::Pagina::poner_zoom(self, zoom);
    }
    fn poner_oscuro(&self, oscuro: bool) {
        pixpin_web::pagina::Pagina::poner_oscuro(self, oscuro, crate::notas_md::lector_web::fondo(oscuro));
    }
}

/// **Por donde va la pagina**, como lo cuenta el guion: la esquina de
/// arriba a la izquierda de la vista en pixeles CSS del documento (`x`,
/// `y`), lo desplazado (`arriba`) y lo que se puede desplazar (`maximo`),
/// y los pixeles de pantalla por pixel CSS (`dpr`; ya lleva el zoom de
/// Ctrl+rueda, que en un WebView2 cambia el `devicePixelRatio`). Ademas, el
/// zoom de pellizco del panel tactil (`visualViewport`): lo que la vista se
/// adelanta dentro de la pagina (`vx`, `vy`, pixeles CSS) y cuanto acerca
/// (`escala`). Sin el, la tinta se iria de su sitio al pellizcar. Y lo
/// que de verdad ensena el documento dentro de la vista (`ax`, `ay`, `aw`,
/// `ah`, pixeles CSS de la vista): en el lector de notas, la caja que se
/// desplaza, sin su barra de arriba ni la de desplazamiento. `aw` 0: toda.
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
pub struct Sitio {
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub arriba: f32,
    #[serde(default)]
    pub maximo: f32,
    #[serde(default = "uno")]
    pub dpr: f32,
    #[serde(default)]
    pub vx: f32,
    #[serde(default)]
    pub vy: f32,
    #[serde(default = "uno")]
    pub escala: f32,
    #[serde(default)]
    pub ax: f32,
    #[serde(default)]
    pub ay: f32,
    #[serde(default)]
    pub aw: f32,
    #[serde(default)]
    pub ah: f32,
    /// El guion del riel se acababa de poner en la pagina (era nueva o se
    /// recargo): hay que mandarle la capa de la tinta otra vez.
    #[serde(default)]
    pub nuevo: bool,
    /// La medida de la columna de ahora, si la pagina es una nota
    /// ([`capa_web::Maqueta`]).
    #[serde(default)]
    pub maq: Option<capa_web::Maqueta>,
}

fn uno() -> f32 {
    1.0
}

impl Sitio {
    /// La esquina de arriba a la izquierda de lo que ensena el documento,
    /// en el documento (lo que guarda una marca y lo que pide `Colocar`).
    pub fn esquina(&self) -> [f32; 2] {
        [self.x + self.ax, self.y + self.ay]
    }

    /// Lo que ensena el documento, en el documento: `None` si la pagina no
    /// lo dijo (entonces, la vista entera).
    pub fn area_en_documento(&self) -> Option<[f64; 4]> {
        (self.aw > 0.5 && self.ah > 0.5).then(|| {
            let [x, y] = self.esquina();
            [x as f64, y as f64, (x + self.aw) as f64, (y + self.ah) as f64]
        })
    }

    /// Lo leido, de 0 a 1 (una pagina que cabe entera esta siempre en 0).
    pub fn fraccion(&self) -> f32 {
        if self.maximo > 0.5 { (self.arriba / self.maximo).clamp(0.0, 1.0) } else { 0.0 }
    }

    /// La camara de la tinta y de los marcos: documento (pixeles CSS) a
    /// pixeles de la vista, con el zoom de Ctrl+rueda y el de pellizco.
    pub fn camara(&self) -> Camara {
        Camara { x: self.x + self.vx, y: self.y + self.vy, zoom: (self.dpr * self.escala.max(0.01)).max(0.1) }
    }
}

/// Lo que se le pide a la pagina.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Orden {
    /// Solo por donde va.
    Mirar,
    /// A esa fraccion de lo que se desplaza (una marca sin vista guardada).
    Ir(f32),
    /// Tantos pixeles CSS mas abajo (la rueda con el lapiz pulsado).
    Bajar(f32),
    /// Que la esquina de arriba a la izquierda de lo que ensena el documento
    /// (la de [`Sitio::esquina`]) caiga en ese punto del documento
    /// (pixeles CSS): una marca con su vista, o un marco que se fotografia.
    Colocar(f32, f32),
}

/// **El guion que se corre en la pagina**: busca lo que se desplaza (el
/// documento o, como en el lector de notas, una caja de dentro: la que
/// tenga el centro de la vista), hace la orden y devuelve el [`Sitio`].
/// `colocar` se repite al pintar y un poco despues: si a la vez cambio el
/// zoom, la pagina se recoloca y el primer intento se quedaria corto.
pub fn guion(orden: Orden) -> String {
    let fin = |v: f32| if v.is_finite() { v } else { 0.0 };
    let (modo, a, b) = match orden {
        Orden::Mirar => ("mirar", 0.0, 0.0),
        Orden::Ir(f) => ("ir", fin(f).clamp(0.0, 1.0), 0.0),
        Orden::Bajar(d) => ("bajar", fin(d), 0.0),
        Orden::Colocar(x, y) => ("colocar", fin(x), fin(y)),
    };
    format!(
        "{}
(function(modo,a,b){{var R=window.__pixpinRiel;var n=R.nuevo;R.nuevo=false;R.colgar();R.ir(modo,a,b);var r=R.sitio();r.nuevo=n;return r;}})({modo:?},{a},{b})",
        super::capa_web::INSTALAR
    )
}

/// El resultado del guion (JSON) a [`Sitio`]; `None` si no se entiende.
pub fn sitio_de(json: &str) -> Option<Sitio> {
    serde_json::from_str::<Sitio>(json).ok().filter(sitio_valido)
}

/// Si los numeros de un sitio son numeros (un `NaN` no mueve la tinta).
pub fn sitio_valido(s: &Sitio) -> bool {
    [s.x, s.y, s.dpr, s.vx, s.vy, s.escala, s.ax, s.ay, s.aw, s.ah].iter().all(|v| v.is_finite())
}

/// **La vista de una marca** (lo que va al `.pixpin-vistas`): el zoom de la
/// pagina y la esquina de la vista en el documento.
pub fn vista_de_marca(zoom: f64, s: &Sitio) -> String {
    let [x, y] = s.esquina();
    marcos_doc::zoom_a_texto(zoom, x as f64, y as f64)
}

/// **Los trozos de un marco que no cabe en la vista**: las esquinas (en el
/// documento) donde poner la vista, de `ancho` × `alto` pixeles CSS, para
/// fotografiarlo entero por partes. Un marco que cabe es un solo trozo.
pub fn trozos(caja: [f64; 4], ancho: f64, alto: f64) -> Vec<[f64; 2]> {
    let (ancho, alto) = (ancho.max(1.0), alto.max(1.0));
    let mut v = Vec::new();
    let mut y = caja[1];
    loop {
        let mut x = caja[0];
        loop {
            v.push([x, y]);
            x += ancho;
            if x >= caja[2] - 0.5 {
                break;
            }
        }
        y += alto;
        if y >= caja[3] - 0.5 || v.len() > 400 {
            break;
        }
    }
    v
}

/// La interseccion de dos cajas `[x0, y0, x1, y1]`, si tiene area.
fn cortar(a: [f64; 4], b: [f64; 4]) -> Option<[f64; 4]> {
    let c = [a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])];
    (c[2] - c[0] >= 1.0 && c[3] - c[1] >= 1.0).then_some(c)
}

/// Pega `trozo` en `hoja` con su esquina en `(dx, dy)`, recortando lo que
/// se salga.
fn pegar(hoja: &mut ImagenRgba, trozo: &ImagenRgba, dx: i64, dy: i64) {
    for fila in 0..trozo.alto as i64 {
        let y = dy + fila;
        if y < 0 || y >= hoja.alto as i64 {
            continue;
        }
        let x0 = dx.max(0);
        let x1 = (dx + trozo.ancho as i64).min(hoja.ancho as i64);
        if x1 <= x0 {
            continue;
        }
        let n = ((x1 - x0) * 4) as usize;
        let de = ((fila * trozo.ancho as i64 + (x0 - dx)) * 4) as usize;
        let a = ((y * hoja.ancho as i64 + x0) * 4) as usize;
        if de + n <= trozo.pixeles.len() && a + n <= hoja.pixeles.len() {
            hoja.pixeles[a..a + n].copy_from_slice(&trozo.pixeles[de..de + n]);
        }
    }
}

/// Una hoja en blanco de `ancho` × `alto`.
fn hoja_blanca(ancho: u32, alto: u32) -> ImagenRgba {
    ImagenRgba { ancho, alto, pixeles: vec![255; ancho as usize * alto as usize * 4] }
}

/// Como se abre el riel.
pub struct Opciones {
    /// El documento: de el cuelgan las marcas, la tinta y los marcos
    /// guardados. `None`: solo mientras este abierto.
    pub documento: Option<PathBuf>,
    /// Si lo de debajo dice por donde va (una pagina): sin eso no hay
    /// marcas y la tinta y los marcos no se guardan.
    pub con_pagina: bool,
    /// El nombre para el dialogo de imprimir.
    pub titulo: String,
    /// Si el visor tiene menu ⋮ (la G lo abre: llega como [`Pide::Tecla`]).
    pub menu: bool,
    /// Si la tinta va a la medida de la columna con que se anoto (las notas:
    /// [`capa_web::Maqueta`]).
    pub maqueta: bool,
}

/// El marcador especial de la maqueta: pulsarlo pone la columna a la medida
/// con que se anoto y la tinta aparece en su sitio.
pub const MARCA_MAQUETA: &str = "✏️";

/// Lo que el visor tiene que hacer despues de un evento del riel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pide {
    Nada,
    /// Devolverle el teclado a la pagina (se dejo de anotar o de marcar).
    Teclado,
    /// Una tecla que es del visor (Esc sin nada que soltar no: eso deja de
    /// anotar; F11).
    Tecla(u32),
}

/// Una de las dos ventanas.
struct Capita {
    ventana: VentanaOverlay,
    superficie: Superficie,
    /// Donde esta, en pixeles de pantalla.
    area: Rect,
    visible: bool,
}

impl Capita {
    fn nueva(motor: &MotorRender, d3d: &ID3D11Device, dueno: HWND, area: Rect) -> Option<Capita> {
        let ventana = VentanaOverlay::nueva(area).map_err(|e| tracing::warn!(?e, "riel: sin ventana")).ok()?;
        ventana.poner_dueno(dueno);
        ventana.poner_siempre_encima(false);
        let superficie = Superficie::nueva(motor, d3d, ventana.handle(), area.ancho.max(1), area.alto.max(1))
            .map_err(|e| tracing::warn!(?e, "riel: sin superficie"))
            .ok()?;
        Some(Capita { ventana, superficie, area, visible: false })
    }

    /// A su sitio; `true` si cambio de tamano (hay que repintar).
    fn colocar(&mut self, area: Rect) -> bool {
        if area == self.area {
            return false;
        }
        let tamano = area.ancho != self.area.ancho || area.alto != self.area.alto;
        self.area = area;
        self.ventana.mover(area);
        if tamano {
            let _ = self.superficie.redimensionar(area.ancho.max(1), area.alto.max(1));
        }
        true
    }

    fn mostrar(&mut self, si: bool) {
        if si != self.visible {
            self.visible = si;
            if si { self.ventana.mostrar() } else { self.ventana.ocultar() }
        }
    }
}

/// **El riel de un visor** con su tinta y sus marcos encima.
pub struct RielDeLaVista {
    motor: Rc<MotorRender>,
    riel: Capita,
    tinta_v: Capita,
    /// La ventana del visor (la duena del dialogo de imprimir).
    visor: isize,
    /// La vista (la pagina) en pixeles de pantalla.
    vista: Rect,
    escala_por_cien: u32,
    opciones: Opciones,
    marcas: Vec<Marcador>,
    /// La vista exacta de cada marca (desplazamiento y zoom).
    vistas: Vistas,
    poniendo: bool,
    /// El lapiz pulsado.
    pub anotando: bool,
    /// Los marcos numerados para imprimir (el boton del marco).
    marcos: Marcos,
    /// Su pista («Arrastra para marcar…»).
    pista: String,
    /// Fotografiando los marcos: sin riel, sin rayas azules ni velo.
    imprimiendo: bool,
    encima: DestinoRiel,
    /// Si el riel esta crecido (el raton sobre sus puntos).
    crecido: bool,
    tinta: Tinta,
    capa: Capa,
    ultimo_trazo: Instant,
    trazando: bool,
    /// Lo ultimo que dijo la pagina.
    sitio: Sitio,
    buzon: Rc<RefCell<Vec<Sitio>>>,
    preguntado: Option<Instant>,
    /// La ultima vez que se pregunto por seguirla (no mas de una cada
    /// [`MIRAR_SITIO`]: cada respuesta despierta al bucle, y sin tope se
    /// preguntaria sin parar).
    mirado: Instant,
    /// La marca que espera a saber el sitio para ponerse (el emoticono).
    marca_pendiente: Option<String>,
    /// La medida de la columna con que se anoto esta nota
    /// ([`capa_web::Maqueta`]; solo con [`Opciones::maqueta`]).
    maqueta: Option<capa_web::Maqueta>,
    /// La columna puesta a esa medida (por el lapiz o el marcador ✏️).
    fija: bool,
    /// La tinta sobre papel oscuro (la luna/sol).
    oscuro: bool,
    /// «Elige un emoticono», bajo la tira.
    elige: String,
    fijado: bool,
    repintar_riel: bool,
    repintar_tinta: bool,
    /// Pruebas: cuando imprimir los marcos solo
    /// (`PIXPIN_IMPRIMIR_MARCOS_AL_ABRIR`, a los 2 s de abrir; con
    /// `PIXPIN_IMPRIMIR_A_PDF`, a un fichero sin dialogo).
    imprimir_al_abrir: Option<Instant>,
    /// Pruebas (solo en debug, `PIXPIN_RIEL_MARCOS_PRUEBA`): dos marcos
    /// arrastrados con mensajes a la ventana propia y luego la pagina
    /// desplazada, para ver que se quedan en su sitio.
    #[cfg(debug_assertions)]
    prueba: Option<(Instant, u8)>,
    /// Pruebas (solo en debug, `PIXPIN_RIEL_TINTA_PRUEBA`): un trazo con
    /// mensajes a la ventana propia y luego la pagina desplazada, a tirones
    /// y suave, para fotografiar que la tinta va pegada al texto.
    #[cfg(debug_assertions)]
    prueba_tinta: Option<(Instant, u8)>,
    /// Pruebas (solo en debug, `PIXPIN_RIEL_TECLA_PRUEBA`): teclas de mentira
    /// dentro de la pagina (un `keydown` sintetico): la I a los 2 s (los
    /// marcos), la A a los 4 s (el lapiz) y una I dentro de un campo de texto
    /// a los 6 s (que no hace nada).
    #[cfg(debug_assertions)]
    prueba_tecla: Option<(Instant, u8)>,
    /// **La tinta y los marcos dentro de la pagina** ([`capa_web`]).
    web: Web,
    /// El marco que se esta arrastrando (sus dos esquinas en el documento):
    /// lo pinta la ventana, los ya puestos van en la pagina.
    arrastre: Option<([f64; 2], [f64; 2])>,
    /// Pruebas: el raton de mentira esta sobre el riel (no se mira el de
    /// verdad para saber si se fue).
    globo_de_prueba: bool,
}

/// Lo que la pagina tiene de la capa del riel ([`capa_web`]).
struct Web {
    /// Lo ultimo que se le mando, y su numero.
    enviado: Option<Deseado>,
    numero: u64,
    enviado_en: Instant,
    /// Lo que contesto la pagina: el numero de lo que ya tiene puesto y si
    /// lo puso en la pagina de verdad (y no en la vacia del principio).
    puesto: Rc<std::cell::Cell<(u64, bool)>>,
    /// El numero confirmado que ya se vio (para repintar la ventana al
    /// llegar la confirmacion: entonces suelta la tinta).
    visto: u64,
    /// La tinta ya pasada a SVG: la escena y el papel de entonces.
    hecha: Option<(pixpin_motor2d::Escena, [u8; 3], String)>,
    /// La tinta lleva imagenes (no van en el SVG): la pinta la ventana.
    con_imagenes: bool,
    /// La ultima vez que se miro si la pagina sigue teniendo el guion (se
    /// pierde al recargarla).
    latido: Instant,
}

impl Web {
    fn nueva() -> Web {
        Web {
            enviado: None,
            numero: 0,
            enviado_en: Instant::now(),
            puesto: Rc::new(std::cell::Cell::new((0, false))),
            visto: 0,
            hecha: None,
            con_imagenes: false,
            latido: Instant::now(),
        }
    }

    /// Si la pagina tiene puesto lo ultimo que se le mando.
    fn al_dia(&self) -> bool {
        self.enviado.is_some() && self.puesto.get() == (self.numero, true)
    }
}

impl RielDeLaVista {
    /// El riel sobre `vista` (pixeles de pantalla) del visor `dueno`.
    /// `None` sin GPU o sin ventanas: el visor sigue sin riel.
    pub fn nuevo(motor: Rc<MotorRender>, d3d: &ID3D11Device, dueno: &VentanaOverlay, vista: Rect, escala_por_cien: u32, opciones: Opciones, textos: &Catalogo) -> Option<RielDeLaVista> {
        let tinta_v = Capita::nueva(&motor, d3d, dueno.handle(), vista)?;
        tinta_v.ventana.pedir_entrada_fina();
        // El riel, poseido por la de la tinta: siempre por encima de ella.
        let riel = Capita::nueva(&motor, d3d, tinta_v.ventana.handle(), vista)?;
        riel.ventana.poner_sin_activar();
        let guardado = opciones.documento.as_deref().filter(|_| opciones.con_pagina);
        let (marcas, capa) = match guardado {
            Some(d) => (crate::anotado_del_adjunto::leer(d).marcadores, Capa::leer(&crate::anotado_del_adjunto::tinta_de_la_pagina(d))),
            None => (Vec::new(), Capa::default()),
        };
        let vistas = Vistas::de_documento(guardado);
        let maqueta = if opciones.maqueta { vistas.de_marca(capa_web::CLAVE_MAQUETA).and_then(capa_web::Maqueta::de_texto) } else { None };
        let e = escala_por_cien as f32 / 100.0;
        let mut r = RielDeLaVista {
            motor,
            riel,
            tinta_v,
            visor: dueno.handle().0 as isize,
            vista,
            escala_por_cien,
            marcas: ordenadas(marcas),
            vistas,
            marcos: Marcos::de(guardado),
            pista: marcos_doc::pista(textos),
            imprimiendo: false,
            opciones,
            poniendo: false,
            anotando: false,
            encima: DestinoRiel::Fuera,
            crecido: false,
            tinta: Tinta::nueva(),
            capa,
            ultimo_trazo: Instant::now(),
            trazando: false,
            sitio: Sitio { x: 0.0, y: 0.0, arriba: 0.0, maximo: 0.0, dpr: e, vx: 0.0, vy: 0.0, escala: 1.0, ax: 0.0, ay: 0.0, aw: 0.0, ah: 0.0, nuevo: false, maq: None },
            buzon: Rc::new(RefCell::new(Vec::new())),
            preguntado: None,
            mirado: Instant::now(),
            marca_pendiente: None,
            maqueta,
            fija: false,
            oscuro: false,
            elige: textos.t("marca-elige"),
            fijado: false,
            repintar_riel: true,
            repintar_tinta: true,
            // (Con la prueba de los marcos, despues de ponerlos: a los 4,5 s.)
            imprimir_al_abrir: std::env::var_os("PIXPIN_IMPRIMIR_MARCOS_AL_ABRIR").map(|_| {
                let tras = if cfg!(debug_assertions) && std::env::var_os("PIXPIN_RIEL_MARCOS_PRUEBA").is_some() { 4500 } else { 2000 };
                Instant::now() + Duration::from_millis(tras)
            }),
            #[cfg(debug_assertions)]
            prueba: std::env::var_os("PIXPIN_RIEL_MARCOS_PRUEBA").map(|_| (Instant::now(), 0)),
            #[cfg(debug_assertions)]
            prueba_tinta: std::env::var_os("PIXPIN_RIEL_TINTA_PRUEBA").map(|_| (Instant::now(), 0)),
            #[cfg(debug_assertions)]
            prueba_tecla: std::env::var_os("PIXPIN_RIEL_TECLA_PRUEBA").map(|_| (Instant::now(), 0)),
            web: Web::nueva(),
            arrastre: None,
            globo_de_prueba: false,
        };
        r.recolocar();
        r.riel.mostrar(true);
        // Para las fotos de prueba (solo en debug): nacer con el lapiz o con
        // los marcos pulsados.
        #[cfg(debug_assertions)]
        {
            if std::env::var_os("PIXPIN_RIEL_LAPIZ").is_some() {
                r.alternar_anotar();
            } else if std::env::var_os("PIXPIN_RIEL_MARCOS").is_some() {
                r.alternar_marcos();
            }
        }
        Some(r)
    }

    /// Si `h` es una de sus ventanas (sus eventos van a [`Self::evento`]).
    pub fn es_suya(&self, h: HWND) -> bool {
        h == self.riel.ventana.handle() || h == self.tinta_v.ventana.handle()
    }

    /// La ventana de la tinta y los marcos (para las pruebas con mensajes).
    pub fn ventana_de_la_tinta(&self) -> isize {
        self.tinta_v.ventana.handle().0 as isize
    }

    /// Si hay marcos puestos (Ctrl+P es suyo).
    pub fn hay_marcos(&self) -> bool {
        !self.marcos.lista.is_empty()
    }

    fn e(&self) -> f32 {
        self.escala_por_cien as f32 / 100.0
    }

    /// Si la ventana de la tinta se queda con el raton (lapiz o marcos).
    fn toma_el_raton(&self) -> bool {
        self.anotando || self.marcos.activo
    }

    /// Si lo dibujado va ya dentro de la pagina ([`capa_web`]): la pagina lo
    /// tiene puesto tal como esta ahora.
    fn en_la_pagina(&self) -> bool {
        self.opciones.con_pagina && self.web.al_dia()
    }

    /// **Un gesto de tinta a medias** (el trazo que se dibuja, lo que se
    /// arrastra, el texto que se escribe): mientras, la tinta la pinta la
    /// ventana de encima, con [`Tinta`], y la de la pagina se esconde.
    fn vivo(&self) -> bool {
        self.anotando && (self.trazando || self.tinta.trazando() || self.tinta.gesto.esta_escribiendo())
    }

    /// Si la ventana de encima pinta la tinta (sin pagina que la lleve, o
    /// con un gesto a medias).
    fn tinta_en_la_ventana(&self) -> bool {
        !self.tinta_escondida() && (!self.en_la_pagina() || self.vivo() || self.web.con_imagenes)
    }

    /// **La tinta, escondida**: la nota tiene maqueta y el texto cae ahora
    /// con otra medida (otra ventana, otro zoom, otro equipo), asi que la
    /// tinta no caeria sobre lo anotado. Sale al pulsar el marcador ✏️ o el
    /// lapiz, que ponen la columna a su medida.
    fn tinta_escondida(&self) -> bool {
        match (self.maqueta, self.sitio.maq) {
            (Some(m), Some(ahora)) => !self.fija && !m.coincide(&ahora),
            _ => false,
        }
    }

    /// Si hay que seguir preguntandole a la pagina por donde va (cada
    /// [`MIRAR_SITIO`]): solo si lo dibujado no va dentro de ella.
    fn sigue_preguntando(&self) -> bool {
        !self.en_la_pagina() || (self.web.con_imagenes && !self.capa.vacia())
    }

    /// Si hay que ensenar la ventana de la tinta.
    fn tinta_a_la_vista(&self) -> bool {
        self.toma_el_raton()
            || (self.tinta_en_la_ventana() && !self.capa.vacia())
            || (!self.en_la_pagina() && !self.marcos.lista.is_empty())
    }

    /// **Lo que tiene que haber en la pagina** ahora: la tinta en SVG (se
    /// rehace solo si cambio la escena o el papel), escondida durante un
    /// gesto; los marcos (no al fotografiarlos) y lo que mandan las teclas.
    fn deseado(&mut self, pagina: &dyn Pagina) -> Deseado {
        let papel = lector_tinta::papel(if self.oscuro { pixpin_render::oscuro::PAPEL } else { Color::BLANCO });
        let clave = [(papel.r * 255.0) as u8, (papel.g * 255.0) as u8, (papel.b * 255.0) as u8];
        let vivo = self.vivo();
        if !vivo {
            let fresca = matches!(&self.web.hecha, Some((esc, k, _)) if *k == clave && *esc == self.capa.escena);
            if !fresca {
                let svg = capa_web::svg_de_la_tinta(&self.capa, papel);
                self.web.con_imagenes = self.capa.escena.elementos.iter().any(|e| !e.borrado && matches!(e.figura, pixpin_motor2d::Figura::Imagen { .. }));
                // Una imagen pegada en la tinta no va en el SVG (su bitmap es del
                // lector): entonces la tinta entera sigue en la ventana de encima.
                let svg = if self.web.con_imagenes { String::new() } else { svg };
                self.web.hecha = Some((self.capa.escena.clone(), clave, svg));
            }
        }
        let marcos = if self.imprimiendo { String::new() } else { capa_web::svg_de_los_marcos(&self.marcos.lista, self.camara().zoom as f64, self.e() as f64) };
        Deseado {
            tinta: self.web.hecha.as_ref().map(|h| h.2.clone()).unwrap_or_default(),
            ver_tinta: !vivo && !self.tinta_escondida(),
            marcos,
            ctrl_p: !self.marcos.lista.is_empty() && !pagina.ctrl_p_propio(),
            menu: self.opciones.menu,
            maqueta: self.maqueta.map(|m| m.a_texto()),
            fija: self.fija,
        }
    }

    /// **Deja en la pagina lo que tiene que haber** (si cambio) y mira si
    /// ya lo confirmo: entonces la ventana de encima suelta la tinta.
    fn sincronizar(&mut self, pagina: Option<&dyn Pagina>) {
        let Some(pg) = pagina.filter(|_| self.opciones.con_pagina) else { return };
        let (n, ok) = self.web.puesto.get();
        if n == self.web.numero && n != self.web.visto {
            self.web.visto = n;
            self.repintar_tinta = true;
            if !ok {
                // Aun era la pagina vacia del principio: otra vez en un rato.
                self.web.enviado = None;
                self.web.enviado_en = Instant::now();
            }
        }
        let d = self.deseado(pg);
        if self.web.enviado.as_ref() == Some(&d) {
            return;
        }
        if self.web.enviado.is_none() && self.web.numero > 0 && self.web.enviado_en.elapsed() < REINTENTO {
            return;
        }
        self.web.numero += 1;
        let numero = self.web.numero;
        tracing::debug!(numero, tinta = d.tinta.len(), ver = d.ver_tinta, marcos = d.marcos.len(), elementos = self.capa.escena.elementos.len(), "riel: capa a la pagina");
        let puesto = Rc::clone(&self.web.puesto);
        pg.ejecutar(
            &capa_web::guion_poner(&d),
            Box::new(move |r| {
                tracing::debug!(numero, %r, "riel: capa puesta");
                if numero >= puesto.get().0 {
                    puesto.set((numero, r.trim() == "1"));
                }
            }),
        );
        self.web.enviado = Some(d);
        self.web.enviado_en = Instant::now();
        self.repintar_tinta = true;
    }

    /// Al imprimir: espera a que la pagina tenga puesta la capa de ahora
    /// (sin las rayas de los marcos) antes de las fotos.
    fn esperar_capa(&mut self, pagina: Option<&dyn Pagina>) {
        if !self.opciones.con_pagina || pagina.is_none() {
            return;
        }
        self.sincronizar(pagina);
        let t = Instant::now();
        while !self.web.al_dia() && t.elapsed() < ESPERA_GUION {
            pixpin_shell::overlay::bombear_pendientes();
            pixpin_shell::overlay::esperar_eventos(Some(10));
        }
        self.sincronizar(pagina);
    }

    /// **Lo que manda la pagina por su cuenta** (`chrome.webview.postMessage`):
    /// por donde va al desplazarse y las teclas de los visores pulsadas en
    /// ella. `None`: el mensaje no es del riel (es de quien lleva la pagina).
    pub fn mensaje(&mut self, m: &str, pagina: Option<&dyn Pagina>) -> Option<Pide> {
        match capa_web::mensaje_de(m)? {
            DelGuion::Sitio(s) => {
                self.buzon.borrow_mut().push(s);
                Some(Pide::Nada)
            }
            DelGuion::Tecla { vk, ctrl } => {
                tracing::debug!(vk, ctrl, "riel: tecla desde la pagina");
                Some(self.tecla(vk, ctrl, pagina).unwrap_or(Pide::Nada))
            }
        }
    }

    /// **Las teclas de los visores, las mismas en todos** (19-oct-2026):
    /// I, los marcos; A, el lapiz; M o Ctrl+M, la tira de los marcadores;
    /// Ctrl+P con marcos, imprimirlos; G, el menu ⋮ si lo hay. Las que llegan
    /// a la ventana del visor o, con [`Self::mensaje`], desde la pagina; las
    /// de la ventana de la tinta (con el lapiz o los marcos) van por
    /// [`Self::evento`]. `None`: no es suya.
    pub fn tecla(&mut self, vk: u32, ctrl: bool, pagina: Option<&dyn Pagina>) -> Option<Pide> {
        match (vk, ctrl) {
            (marcos_doc::VK_I, false) => Some(self.alternar_marcos()),
            (VK_A, false) => Some(self.alternar_anotar()),
            (VK_M, _) => {
                self.alternar_tira();
                Some(Pide::Nada)
            }
            (marcos_doc::VK_P, true) if self.hay_marcos() => {
                self.imprimir_marcos(pagina);
                Some(Pide::Nada)
            }
            (VK_G, false) if self.opciones.menu => Some(Pide::Tecla(VK_G)),
            _ => None,
        }
    }

    /// Esc con la tira de los marcadores abierta la cierra (y no cierra el
    /// visor). `true` si la cerro.
    pub fn soltar_esc(&mut self) -> bool {
        if !self.poniendo {
            return false;
        }
        self.alternar_tira();
        true
    }

    /// La tira de los emoticonos para poner un marcador (solo con pagina).
    fn alternar_tira(&mut self) {
        if self.opciones.con_pagina {
            self.poniendo = !self.poniendo;
            self.recolocar();
            self.repintar_riel = true;
        }
    }

    /// **Pulsar la ficha de un marco**: la vista va a el, centrado (o, si no
    /// cabe, con su esquina de arriba a la izquierda arriba del todo).
    fn ir_a_marco(&mut self, i: usize, pagina: Option<&dyn Pagina>) {
        let Some(m) = self.marcos.lista.get(i) else { return };
        let v = self.vista_en_documento();
        let (vw, vh) = (v[2] - v[0], v[3] - v[1]);
        let [cx, cy] = m.centro();
        let aire = 12.0;
        let x = if m.ancho() + 2.0 * aire <= vw { cx - vw / 2.0 } else { m.caja[0] - aire };
        let y = if m.alto() + 2.0 * aire <= vh { cy - vh / 2.0 } else { m.caja[1] - aire };
        self.pedir(pagina, Orden::Colocar(x as f32, y as f32));
    }

    /// El riel colocado en la vista (coordenadas de la vista).
    fn colocado(&self) -> Riel {
        lector::riel(self.vista.ancho as f32, self.vista.alto as f32, self.escala_por_cien, self.marcas.len(), self.poniendo)
    }

    /// **Lo que ocupa la ventana del riel**, en coordenadas de la vista: la
    /// isla y los puntos; crecido, tambien a la izquierda de los puntos (el
    /// que tiene el raton se adelanta y lleva su atajo); con la tira, toda
    /// la vista.
    fn area_del_riel(&self, riel: &Riel) -> Rect {
        if riel.tira.is_some() {
            return Rect { x: 0, y: 0, ancho: self.vista.ancho, alto: self.vista.alto };
        }
        let e = self.e();
        let aire = (6.0 * e) as i32;
        let mut x0 = riel.isla.x - aire;
        if self.crecido && !riel.puntos.is_empty() {
            x0 -= ((SALTO + PUNTO_ELEGIDO / 2) as f32 * e + 70.0 * e) as i32;
        }
        // Con el raton sobre un boton, su globo (el nombre y la tecla) sale
        // a la izquierda: la ventana crece para que no se corte.
        if matches!(self.encima, DestinoRiel::PonerMarca | DestinoRiel::Hojita | DestinoRiel::Imprimir) {
            x0 = x0.min(riel.isla.x - aire - (GLOBO_ANCHO * e) as i32);
        }
        let abajo = riel.isla.abajo().max(riel.columna.abajo()) + aire;
        let x0 = x0.max(0);
        // Por la derecha, sin aire: alli esta la barra de desplazamiento.
        let x1 = (riel.isla.x + riel.isla.ancho as i32).min(self.vista.ancho as i32);
        let y0 = (riel.isla.y - aire).max(0);
        let y1 = abajo.min(self.vista.alto as i32);
        Rect { x: x0, y: y0, ancho: (x1 - x0).max(1) as u32, alto: (y1 - y0).max(1) as u32 }
    }

    fn recolocar(&mut self) {
        let riel = self.colocado();
        let a = self.area_del_riel(&riel);
        let en_pantalla = Rect { x: self.vista.x + a.x, y: self.vista.y + a.y, ..a };
        if self.riel.colocar(en_pantalla) {
            self.repintar_riel = true;
        }
        if self.tinta_v.colocar(self.vista) {
            self.repintar_tinta = true;
        }
    }

    /// **La vista se movio o cambio** (el pin se arrastro, se estiro o fue a
    /// pantalla completa), el pin se fijo o cambio la luna/sol.
    pub fn seguir(&mut self, vista: Rect, escala_por_cien: u32, fijado: bool, oscuro: bool) {
        if vista != self.vista || escala_por_cien != self.escala_por_cien {
            self.vista = vista;
            self.escala_por_cien = escala_por_cien;
            self.repintar_riel = true;
            self.repintar_tinta = true;
            self.recolocar();
        }
        if fijado != self.fijado {
            self.fijado = fijado;
            self.tinta_v.ventana.poner_siempre_encima(fijado);
            self.riel.ventana.poner_siempre_encima(fijado);
        }
        if oscuro != self.oscuro {
            self.oscuro = oscuro;
            self.repintar_tinta = true;
        }
    }

    /// El punto del raton (pantalla) en coordenadas de la vista.
    fn local(&self, p: Punto) -> Punto {
        Punto { x: p.x - self.vista.x, y: p.y - self.vista.y }
    }

    /// El punto del raton (pantalla) en el documento.
    fn en_documento(&self, p: Punto) -> [f64; 2] {
        let c = self.camara();
        let l = self.local(p);
        [(l.x as f32 / c.zoom + c.x) as f64, (l.y as f32 / c.zoom + c.y) as f64]
    }

    fn pedir(&mut self, pagina: Option<&dyn Pagina>, orden: Orden) {
        let Some(pg) = pagina.filter(|_| self.opciones.con_pagina) else { return };
        let buzon = Rc::clone(&self.buzon);
        self.preguntado = Some(Instant::now());
        pg.ejecutar(
            &guion(orden),
            Box::new(move |json| {
                if let Some(s) = sitio_de(&json) {
                    buzon.borrow_mut().push(s);
                }
            }),
        );
    }

    /// **Un evento de una de sus ventanas.**
    pub fn evento(&mut self, h: HWND, ev: &EventoOverlay, pagina: Option<&dyn Pagina>, textos: &Catalogo) -> Pide {
        if h == self.riel.ventana.handle() {
            return self.evento_del_riel(ev, pagina, textos);
        }
        if self.marcos.activo {
            return self.evento_de_los_marcos(ev, pagina);
        }
        self.evento_de_la_tinta(ev, pagina)
    }

    fn evento_del_riel(&mut self, ev: &EventoOverlay, pagina: Option<&dyn Pagina>, textos: &Catalogo) -> Pide {
        match *ev {
            EventoOverlay::RatonMovido(p) => {
                self.raton_en(p);
                Pide::Nada
            }
            EventoOverlay::BotonPulsado(p) => {
                let destino = self.colocado().destino(self.local(p));
                self.pulsar(destino, pagina, textos)
            }
            // El derecho sobre un punto quita esa marca, como en el PDF.
            EventoOverlay::BotonDerechoPulsado(p) => {
                if let DestinoRiel::Marca(i) = self.colocado().destino(self.local(p))
                    && i < self.marcas.len()
                {
                    let m = self.marcas.remove(i);
                    self.vistas.quitar(&m.id.to_string());
                    self.guardar_marcas();
                    self.recolocar();
                    self.repintar_riel = true;
                }
                Pide::Nada
            }
            EventoOverlay::Tecla { vk: VK_ESCAPE, .. } if self.poniendo => {
                self.poniendo = false;
                self.recolocar();
                self.repintar_riel = true;
                Pide::Nada
            }
            EventoOverlay::Pintar | EventoOverlay::CambioDpi => {
                self.repintar_riel = true;
                Pide::Nada
            }
            _ => Pide::Nada,
        }
    }

    /// El raton paso por `p` (pantalla): lo de debajo se resalta y el riel
    /// crece sobre sus puntos.
    fn raton_en(&mut self, p: Punto) {
        let riel = self.colocado();
        let local = self.local(p);
        let encima = riel.destino(local);
        if encima != self.encima {
            self.encima = encima;
            // (Crece o mengua con el globo del boton.)
            self.recolocar();
            self.repintar_riel = true;
        }
        let a = self.area_del_riel(&riel);
        let dentro = local.x >= a.x && local.y >= a.y && local.x < a.x + a.ancho as i32 && local.y < a.y + a.alto as i32;
        let sobre_los_puntos = matches!(encima, DestinoRiel::Marca(_));
        let crecido = if self.crecido { dentro } else { sobre_los_puntos };
        if crecido != self.crecido {
            self.crecido = crecido;
            self.recolocar();
            self.repintar_riel = true;
        }
    }

    fn pulsar(&mut self, destino: DestinoRiel, pagina: Option<&dyn Pagina>, _textos: &Catalogo) -> Pide {
        let mut pide = Pide::Nada;
        match destino {
            DestinoRiel::PonerMarca if self.opciones.con_pagina => {
                self.poniendo = !self.poniendo;
            }
            DestinoRiel::PonerMarca => {}
            DestinoRiel::Hojita => pide = self.alternar_anotar(),
            // El marco: los marcos numerados, como en el plano.
            DestinoRiel::Imprimir => pide = self.alternar_marcos(),
            DestinoRiel::Marca(i) => {
                if let Some(m) = self.marcas.get(i).cloned() {
                    self.ir_a_marca(&m, pagina);
                }
            }
            DestinoRiel::Emoji(i) => {
                self.poniendo = false;
                self.marca_pendiente = Some(lector::emoji_de_la_tira(i).to_string());
                self.pedir(pagina, Orden::Mirar);
            }
            DestinoRiel::CerrarTira => self.poniendo = false,
            // Con la tira abierta, un clic fuera de ella la cierra.
            DestinoRiel::Hueco | DestinoRiel::Fuera => self.poniendo = false,
        }
        self.recolocar();
        self.repintar_riel = true;
        pide
    }

    /// **Volver a una marca**: con su vista guardada, a ese zoom y a ese
    /// desplazamiento exactos; sin ella (una de antes o del movil), a su
    /// fraccion.
    fn ir_a_marca(&mut self, m: &Marcador, pagina: Option<&dyn Pagina>) {
        // El marcador ✏️ de una nota: antes de ir, la columna a la medida
        // con que se anoto (en la pagina YA: los guiones van en orden, asi
        // que el desplazamiento de despues cae con el texto recolocado).
        if m.emoji == MARCA_MAQUETA
            && let Some(mq) = self.maqueta
            && !self.fija
        {
            self.fija = true;
            self.repintar_tinta = true;
            if let Some(pg) = pagina {
                pg.ejecutar(&format!("if(window.__pixpinMaqueta)window.__pixpinMaqueta.poner({},true)", mq.a_texto()), Box::new(|_| {}));
            }
        }
        let vista = self.vistas.de_marca(&m.id.to_string()).and_then(marcos_doc::zoom_de_texto);
        match (vista, pagina) {
            (Some((zoom, x, y)), Some(pg)) => {
                if (pg.zoom() - zoom).abs() > 1e-3 {
                    pg.poner_zoom(zoom);
                }
                self.pedir(pagina, Orden::Colocar(x as f32, y as f32));
            }
            _ => self.pedir(pagina, Orden::Ir(m.fraccion)),
        }
    }

    /// Que la ventana de la tinta recoja el raton y el teclado, o los
    /// suelte a la pagina.
    fn tomar_el_raton(&mut self, si: bool) {
        if si {
            self.tinta_v.ventana.poner_pasante(false);
            self.tinta_v.mostrar(true);
            self.riel.ventana.traer_encima();
            self.tinta_v.ventana.enfocar();
        } else {
            self.tinta_v.ventana.soltar_raton();
            self.tinta_v.ventana.poner_pasante(true);
        }
    }

    /// **Al coger el lapiz en una nota**: la columna, a la medida de su
    /// maqueta (se anota siempre a la misma). La primera vez que se anota
    /// la nota no la tiene: es la medida de ahora, y se pone el marcador ✏️
    /// en este sitio para volver a ella desde otra ventana u otro equipo.
    fn fijar_maqueta(&mut self) {
        if !self.opciones.maqueta {
            return;
        }
        if self.maqueta.is_none() {
            let Some(ahora) = self.sitio.maq else { return };
            self.maqueta = Some(ahora);
            self.vistas.poner(capa_web::CLAVE_MAQUETA, ahora.a_texto());
            if !self.marcas.iter().any(|m| m.emoji == MARCA_MAQUETA) {
                self.marca_pendiente = Some(MARCA_MAQUETA.to_string());
            }
            tracing::info!(ancho = ahora.ancho, tam = ahora.tam, "nota: maqueta de la tinta guardada");
        }
        self.fija = true;
    }

    /// **El lapiz**: la ventana de la tinta recoge el raton (y el teclado)
    /// o los suelta a la pagina, guardando lo dibujado. Sale de los marcos.
    pub fn alternar_anotar(&mut self) -> Pide {
        self.anotando = !self.anotando;
        self.poniendo = false;
        self.repintar_riel = true;
        self.repintar_tinta = true;
        if self.anotando {
            if self.marcos.activo {
                self.marcos.alternar();
            }
            self.fijar_maqueta();
            self.tomar_el_raton(true);
            Pide::Nada
        } else {
            self.trazando = false;
            self.guardar_tinta();
            self.tomar_el_raton(false);
            Pide::Teclado
        }
    }

    /// **El boton del marco** (o la I): entrar o salir de poner marcos.
    /// Como en el plano, marcar y anotar no van a la vez.
    pub fn alternar_marcos(&mut self) -> Pide {
        if self.anotando {
            self.anotando = false;
            self.trazando = false;
            self.guardar_tinta();
        }
        self.marcos.alternar();
        self.poniendo = false;
        self.repintar_riel = true;
        self.repintar_tinta = true;
        if self.marcos.activo {
            self.tomar_el_raton(true);
            self.tinta_v.ventana.poner_cursor(FormaCursorWin::Cruz);
            Pide::Nada
        } else {
            self.tomar_el_raton(false);
            Pide::Teclado
        }
    }

    /// Lo que llega a la ventana de la tinta con los marcos pulsados.
    fn evento_de_los_marcos(&mut self, ev: &EventoOverlay, pagina: Option<&dyn Pagina>) -> Pide {
        match *ev {
            EventoOverlay::Pintar | EventoOverlay::CambioDpi => self.repintar_tinta = true,
            EventoOverlay::Rueda(d) => self.pedir(pagina, Orden::Bajar(-(d as f32) / 120.0 * CSS_POR_MUESCA)),
            EventoOverlay::BotonPulsado(p) => {
                // Las fichas 1, 2, 3… de abajo: ir a ese marco.
                let l = self.local(p);
                if let Some(i) = self.marcos.ficha_en((l.x as f32, l.y as f32), self.vista.alto as f32, self.e()) {
                    self.ir_a_marco(i, pagina);
                    return Pide::Nada;
                }
                let d = self.en_documento(p);
                if self.marcos.pulsar(d, 0) {
                    self.arrastre = Some((d, d));
                    self.tinta_v.ventana.capturar_raton();
                    self.repintar_tinta = true;
                }
            }
            EventoOverlay::RatonMovido(p) => {
                self.tinta_v.ventana.poner_cursor(FormaCursorWin::Cruz);
                let d = self.en_documento(p);
                if self.marcos.mover(d) {
                    if let Some((_, b)) = self.arrastre.as_mut() {
                        *b = d;
                    }
                    self.repintar_tinta = true;
                }
            }
            EventoOverlay::BotonSoltado(p) => {
                let d = self.en_documento(p);
                let z = self.camara().zoom as f64;
                self.marcos.soltar(d, z);
                self.arrastre = None;
                self.tinta_v.ventana.soltar_raton();
                self.repintar_tinta = true;
            }
            EventoOverlay::Tecla { vk: VK_F11, .. } => return Pide::Tecla(VK_F11),
            EventoOverlay::Tecla { vk: VK_ESCAPE, .. } if self.poniendo => self.alternar_tira(),
            EventoOverlay::Tecla { vk: VK_ESCAPE, .. } => return self.alternar_marcos(),
            // Las de todos los visores: A, el lapiz; M o Ctrl+M, la tira.
            EventoOverlay::Tecla { vk: VK_A, ctrl: false, .. } => return self.alternar_anotar(),
            EventoOverlay::Tecla { vk: VK_M, .. } => self.alternar_tira(),
            EventoOverlay::Tecla { vk, ctrl, .. } => match self.marcos.tecla(vk, ctrl) {
                Some(marcos_doc::Pedido::Imprimir) => self.imprimir_marcos(pagina),
                Some(marcos_doc::Pedido::Alternado) => {
                    // `tecla` ya lo alterno: se deshace y se pasa por el
                    // camino de siempre (soltar el raton, el teclado...).
                    self.marcos.alternar();
                    return self.alternar_marcos();
                }
                Some(marcos_doc::Pedido::Cambiado) => self.repintar_tinta = true,
                None => {}
            },
            _ => {}
        }
        Pide::Nada
    }

    fn evento_de_la_tinta(&mut self, ev: &EventoOverlay, pagina: Option<&dyn Pagina>) -> Pide {
        if !self.anotando {
            return Pide::Nada;
        }
        match *ev {
            EventoOverlay::Pintar | EventoOverlay::CambioDpi => {
                self.repintar_tinta = true;
                return Pide::Nada;
            }
            // La rueda mueve la pagina, aunque se este anotando.
            EventoOverlay::Rueda(d) => {
                self.pedir(pagina, Orden::Bajar(-(d as f32) / 120.0 * CSS_POR_MUESCA));
                return Pide::Nada;
            }
            EventoOverlay::Tecla { vk: VK_F11, .. } => return Pide::Tecla(VK_F11),
            EventoOverlay::Tecla { vk: VK_ESCAPE, .. } if self.poniendo => {
                self.alternar_tira();
                return Pide::Nada;
            }
            EventoOverlay::Tecla { vk: VK_ESCAPE, .. } if !self.tinta.quiere_escape() => return self.alternar_anotar(),
            // Ctrl+M, la tira de los marcadores, como en todos los visores
            // (la M sola es aqui una herramienta de la tinta).
            EventoOverlay::Tecla { vk: VK_M, ctrl: true, .. } if !self.tinta.gesto.esta_escribiendo() => {
                self.alternar_tira();
                return Pide::Nada;
            }
            EventoOverlay::Tecla { vk: VK_Z, ctrl: true, shift, .. } if !self.tinta.gesto.esta_escribiendo() => {
                if shift { self.capa.rehacer() } else { self.capa.deshacer() };
                self.ultimo_trazo = Instant::now();
                self.repintar_tinta = true;
                return Pide::Nada;
            }
            EventoOverlay::Tecla { vk: VK_Y, ctrl: true, .. } if !self.tinta.gesto.esta_escribiendo() => {
                self.capa.rehacer();
                self.ultimo_trazo = Instant::now();
                self.repintar_tinta = true;
                return Pide::Nada;
            }
            // Ctrl+P con el lapiz: los marcos, como en el plano.
            EventoOverlay::Tecla { vk: marcos_doc::VK_P, ctrl: true, .. } => {
                self.imprimir_marcos(pagina);
                return Pide::Nada;
            }
            _ => {}
        }
        let camara = self.camara();
        let h = self.tinta.evento(ev, &mut self.capa, &camara, self.vista, self.escala_por_cien);
        if let Some(c) = h.cursor {
            self.tinta_v.ventana.poner_cursor(crate::dibujo::teclas::forma_de(c));
        }
        if h.cambio {
            self.ultimo_trazo = Instant::now();
        }
        if h.arrastra {
            self.trazando = true;
            self.tinta_v.ventana.capturar_raton();
        }
        if matches!(ev, EventoOverlay::BotonSoltado(_)) && self.trazando {
            self.trazando = false;
            self.tinta_v.ventana.soltar_raton();
        }
        self.repintar_tinta = true;
        if h.salir {
            return self.alternar_anotar();
        }
        Pide::Nada
    }

    /// La camara de la tinta y los marcos: con pagina, la que dice ella;
    /// sin, la vista quieta en pixeles logicos.
    fn camara(&self) -> Camara {
        if self.opciones.con_pagina { self.sitio.camara() } else { Camara { x: 0.0, y: 0.0, zoom: self.e() } }
    }

    /// **Cada vuelta del bucle del visor**: lo que conto la pagina, si el
    /// raton se fue del riel, la forma rapida, guardar la tinta y pintar.
    pub fn tic(&mut self, pagina: Option<&dyn Pagina>) {
        let llegados: Vec<Sitio> = self.buzon.borrow_mut().drain(..).collect();
        // (La pagina se recargo y perdio el guion: la capa, otra vez.)
        if llegados.iter().any(|s| s.nuevo) {
            self.web.enviado = None;
        }
        if let Some(s) = llegados.last().copied() {
            self.preguntado = None;
            let s = Sitio { nuevo: false, ..s };
            if s != self.sitio {
                self.sitio = s;
                self.repintar_tinta = true;
            }
            if let Some(emoji) = self.marca_pendiente.take() {
                let zoom = pagina.map_or(1.0, |p| p.zoom());
                self.poner_marca(&s, zoom, &emoji);
            }
        }
        if self.preguntado.is_some_and(|t| t.elapsed() > SIN_RESPUESTA) {
            self.preguntado = None;
        }
        // Con la tinta o los marcos en la ventana de encima y sin la capa en
        // la pagina (aun no llego, o la pagina no deja), la pagina se sigue
        // preguntando: van con ella. Con la capa puesta, la pagina avisa
        // sola al desplazarse y solo se mira de vez en cuando si sigue
        // teniendo el guion (al recargarse lo pierde).
        if self.opciones.con_pagina && self.preguntado.is_none() {
            if self.sigue_preguntando() && self.tinta_a_la_vista() && self.mirado.elapsed() >= MIRAR_SITIO {
                self.mirado = Instant::now();
                self.web.latido = Instant::now();
                self.pedir(pagina, Orden::Mirar);
            } else if self.web.latido.elapsed() >= if self.en_la_pagina() { LATIDO } else { LATIDO_SIN_CAPA } {
                self.web.latido = Instant::now();
                self.pedir(pagina, Orden::Mirar);
            }
        }
        // El raton se fue sin pasar por el borde (no hay WM_MOUSELEAVE).
        if (self.crecido || self.encima != DestinoRiel::Fuera) && !self.globo_de_prueba {
            let p = pixpin_shell::overlay::raton_en_pantalla();
            let r = self.riel.area;
            if !(p.x >= r.x && p.y >= r.y && p.x < r.x + r.ancho as i32 && p.y < r.y + r.alto as i32) {
                self.encima = DestinoRiel::Fuera;
                self.crecido = false;
                self.recolocar();
                self.repintar_riel = true;
            }
        }
        let zoom = self.camara().zoom;
        if self.anotando && self.tinta.forma_rapida(&mut self.capa, zoom) {
            self.repintar_tinta = true;
        }
        if self.capa.sucia && !self.tinta.trazando() && self.ultimo_trazo.elapsed() > PARA_GUARDAR {
            self.guardar_tinta();
        }
        self.sincronizar(pagina);
        self.pintar();
        #[cfg(debug_assertions)]
        {
            self.paso_de_prueba(pagina);
            self.paso_de_prueba_tinta(pagina);
            self.paso_de_prueba_tecla(pagina);
        }
        if self.imprimir_al_abrir.is_some_and(|t| Instant::now() >= t) {
            self.imprimir_al_abrir = None;
            self.imprimir_marcos(pagina);
        }
    }

    /// Si quiere que el bucle vuelva pronto (siguiendo la pagina o con un
    /// gesto a medias).
    pub fn quiere_reloj(&self) -> bool {
        self.toma_el_raton()
            || self.crecido
            || self.marca_pendiente.is_some()
            || (self.opciones.con_pagina && self.sigue_preguntando() && self.tinta_a_la_vista())
    }

    /// Cuanto puede dormir el bucle como mucho.
    pub fn tope_ms(&self) -> Option<u32> {
        if !self.quiere_reloj() {
            // Con pagina, solo el latido (si sigue teniendo el guion).
            return self.opciones.con_pagina.then(|| if self.en_la_pagina() { LATIDO } else { LATIDO_SIN_CAPA }.as_millis() as u32);
        }
        let t = MIRAR_SITIO.as_millis() as u32;
        Some(self.tinta.mano.tope_forma_ms().map_or(t, |f| f.min(t)))
    }

    fn poner_marca(&mut self, s: &Sitio, zoom: f64, emoji: &str) {
        let ahora = lector::ahora_ms();
        let mut id = ahora;
        while self.marcas.iter().any(|m| m.id == id) {
            id += 1;
        }
        self.marcas.push(Marcador { id, fraccion: s.fraccion(), emoji: emoji.to_string() });
        self.vistas.poner(&id.to_string(), vista_de_marca(zoom, s));
        // Como en el PDF: 24 como mucho, se va la mas vieja.
        while self.marcas.len() > pixpin_motor2d::marcas::MAXIMO {
            if let Some((i, _)) = self.marcas.iter().enumerate().min_by_key(|(_, m)| m.id) {
                let m = self.marcas.remove(i);
                self.vistas.quitar(&m.id.to_string());
            }
        }
        self.marcas = ordenadas(std::mem::take(&mut self.marcas));
        self.guardar_marcas();
        self.recolocar();
        self.repintar_riel = true;
    }

    fn guardar_marcas(&self) {
        let Some(d) = self.opciones.documento.as_deref().filter(|_| self.opciones.con_pagina) else { return };
        guardar_marcas_en(d, &self.marcas);
    }

    fn guardar_tinta(&mut self) {
        let Some(d) = self.opciones.documento.as_deref().filter(|_| self.opciones.con_pagina) else {
            self.capa.sucia = false;
            return;
        };
        if let Err(e) = self.capa.guardar(&crate::anotado_del_adjunto::tinta_de_la_pagina(d)) {
            tracing::warn!(?e, "riel: no se pudo guardar la tinta");
        }
    }

    /// Pinta lo que haga falta.
    fn pintar(&mut self) {
        // La tinta y los marcos: solo si hay algo que ensenar.
        let a_la_vista = self.tinta_a_la_vista();
        if !a_la_vista {
            self.tinta_v.mostrar(false);
        }
        if a_la_vista && self.repintar_tinta {
            self.repintar_tinta = false;
            self.pintar_tinta();
            if !self.tinta_v.visible {
                self.tinta_v.ventana.poner_pasante(!self.toma_el_raton());
                self.tinta_v.mostrar(true);
                self.riel.ventana.traer_encima();
            }
        }
        if self.repintar_riel {
            self.repintar_riel = false;
            self.pintar_riel();
        }
    }

    fn pintar_tinta(&mut self) {
        let (w, h) = (self.tinta_v.area.ancho as f32, self.tinta_v.area.alto as f32);
        let camara = self.camara();
        let papel = lector_tinta::papel(if self.oscuro { pixpin_render::oscuro::PAPEL } else { Color::BLANCO });
        let e = self.e();
        // Con la capa en la pagina, lo dibujado va alli (sin retraso al
        // desplazar): aqui solo el gesto a medias, lo de encima de la tinta
        // (lo elegido, el lazo), las fichas y la pista, y el marco que se
        // arrastra.
        let (tinta_aqui, marcos_aqui) = (self.tinta_en_la_ventana(), !self.en_la_pagina());
        let arrastre = self.arrastre;
        let Ok(destino) = self.tinta_v.superficie.empezar(&self.motor) else { return };
        let (tinta, capa, vista, escala) = (&mut self.tinta, &self.capa, self.vista, self.escala_por_cien);
        let (marcos, pista, imprimiendo) = (&self.marcos, self.pista.as_str(), self.imprimiendo);
        let anotando = self.anotando && !imprimiendo;
        let velo = (self.anotando || self.marcos.activo) && !imprimiendo;
        let _ = self.motor.dibujar(&destino, |p| {
            p.limpiar_transparente();
            // Con el lapiz o los marcos, un velo muy leve: se ve que la
            // pagina esta debajo y no se la puede tocar.
            if velo {
                p.rellenar(RectF { x: 0.0, y: 0.0, ancho: w, alto: h }, Color { r: 0.0, g: 0.0, b: 0.0, a: 0.04 });
            }
            let z = camara.zoom;
            p.poner_vista((0.0, 0.0), z, (-camara.x * z, -camara.y * z));
            if tinta_aqui {
                tinta.pintar_capa(p, 0, capa, camara.ventana(w, h), z, papel, anotando);
            } else if anotando {
                tinta.pintar_capa_parte(p, 0, capa, camara.ventana(w, h), z, papel, true, lector_tinta::Parte::Encima);
            }
            p.desplazar(0.0, 0.0);
            if !imprimiendo {
                let a_pantalla = |q: [f64; 2]| (((q[0] as f32) - camara.x) * z, ((q[1] as f32) - camara.y) * z);
                if marcos_aqui {
                    marcos.pintar(p, 0, &a_pantalla, e, h, pista);
                } else {
                    // (Ninguno es de esa parte: solo las fichas y la pista.)
                    marcos.pintar(p, u32::MAX, &a_pantalla, e, h, pista);
                    if let Some((a, b)) = arrastre {
                        let caja = [a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1])];
                        p.trazar(marcos_doc::caja_en_pantalla(caja, &a_pantalla), 1.5 * e, marcos_doc::AZUL);
                    }
                }
            }
            if anotando {
                tinta.pintar_interfaz(p, Some(capa), vista, escala);
            }
        });
        let _ = self.tinta_v.superficie.presentar();
    }

    fn pintar_riel(&mut self) {
        let riel = self.colocado();
        let a = self.area_del_riel(&riel);
        let emojis: Vec<&str> = self.marcas.iter().map(|m| m.emoji.as_str()).collect();
        let (encima, poniendo, anotando, con_marcas, elige) = (self.encima, self.poniendo, self.anotando, self.opciones.con_pagina, self.elige.as_str());
        let enmarcando = self.marcos.activo;
        let e = self.e();
        let Ok(destino) = self.riel.superficie.empezar(&self.motor) else { return };
        let _ = self.motor.dibujar(&destino, |p| {
            p.limpiar_transparente();
            p.desplazar(-(a.x as f32), -(a.y as f32));
            lector::pintar_riel_con(p, &riel, &emojis, encima, poniendo, anotando, enmarcando, elige);
            // Sin pagina que diga por donde va (la vista previa de Office)
            // no hay marcas: el boton, apagado.
            if !con_marcas {
                let r = lector::rf(riel.poner_marca);
                p.rellenar_redondeado(r, 6.0 * e, lector::con_alfa(lector::CRISTAL, 0.75));
            }
            p.desplazar(0.0, 0.0);
        });
        let _ = self.riel.superficie.presentar();
    }

    /// Las pruebas con mensajes: a los 2 s, los marcos y dos arrastres
    /// (pixeles de la vista, sin mover el raton de verdad); luego, la
    /// pagina 150 px CSS mas abajo (a los 2,8 s).
    #[cfg(debug_assertions)]
    fn paso_de_prueba(&mut self, pagina: Option<&dyn Pagina>) {
        let Some((t, paso)) = self.prueba else { return };
        let ms = t.elapsed().as_millis();
        let h = self.ventana_de_la_tinta();
        let e = self.e();
        let px = |v: f32| (v * e) as i32;
        if paso == 0 && ms > 2000 {
            if !self.marcos.activo {
                self.alternar_marcos();
            }
            for (a, b) in [((40.0, 40.0), (300.0, 190.0)), ((60.0, 230.0), (420.0, 400.0))] {
                pixpin_shell::overlay::simular_raton(h, 0, px(a.0), px(a.1));
                pixpin_shell::overlay::simular_raton(h, 1, px((a.0 + b.0) / 2.0), px((a.1 + b.1) / 2.0));
                pixpin_shell::overlay::simular_raton(h, 1, px(b.0), px(b.1));
                pixpin_shell::overlay::simular_raton(h, 2, px(b.0), px(b.1));
            }
            self.prueba = Some((t, 1));
        } else if paso == 1 && ms > 2800 {
            self.pedir(pagina, Orden::Bajar(150.0));
            // Con `PIXPIN_RIEL_FICHA_PRUEBA`: lejos (a los 3,8 s) y luego la
            // ficha del 2 (a los 5 s), que tiene que volver a el.
            self.prueba = std::env::var_os("PIXPIN_RIEL_FICHA_PRUEBA").map(|_| (t, 2));
        } else if paso == 2 && ms > 3800 {
            self.pedir(pagina, Orden::Bajar(2000.0));
            self.prueba = Some((t, 3));
        } else if paso == 3 && ms > 5000 {
            if let Some(r) = self.marcos.fichas(self.vista.alto as f32, e).get(1) {
                let (x, y) = ((r.x + r.ancho / 2.0) as i32, (r.y + r.alto / 2.0) as i32);
                pixpin_shell::overlay::simular_raton(h, 0, x, y);
                pixpin_shell::overlay::simular_raton(h, 2, x, y);
            }
            self.prueba = None;
        }
    }

    /// La prueba de las teclas dentro de la pagina (ver `prueba_tecla`).
    #[cfg(debug_assertions)]
    fn paso_de_prueba_tecla(&mut self, pagina: Option<&dyn Pagina>) {
        let Some((t, paso)) = self.prueba_tecla else { return };
        let Some(pg) = pagina else { return };
        let ms = t.elapsed().as_millis();
        let tecla = |k: &str| format!("document.body.dispatchEvent(new KeyboardEvent('keydown',{{key:'{k}',bubbles:true,cancelable:true}}))");
        let (js, siguiente) = match paso {
            0 if ms > 2000 => (tecla("i"), Some((t, 1))),
            1 if ms > 4000 => (tecla("a"), Some((t, 2))),
            2 if ms > 6000 => (
                "(function(){var c=document.createElement('textarea');document.body.appendChild(c);c.focus();c.dispatchEvent(new KeyboardEvent('keydown',{key:'i',bubbles:true,cancelable:true}));})()".to_string(),
                None,
            ),
            _ => return,
        };
        pg.ejecutar(&js, Box::new(|_| {}));
        self.prueba_tecla = siguiente;
    }

    /// La prueba de la tinta pegada al texto (`PIXPIN_RIEL_TINTA_PRUEBA`):
    /// a los 2 s, el lapiz y un trazo en zigzag (mensajes a la ventana
    /// propia); a los 2,8 s se suelta el lapiz; a los 5 s la pagina baja 200
    /// px CSS de golpe y a los 7 s 900 mas, poco a poco durante 2,5 s (lo que
    /// se fotografia a medio camino tiene que tener la tinta en su sitio). Con
    /// `PIXPIN_RIEL_GLOBO_PRUEBA`, a los 10,5 s el raton (de mentira) pasa por
    /// el boton del marco: su globo.
    #[cfg(debug_assertions)]
    fn paso_de_prueba_tinta(&mut self, pagina: Option<&dyn Pagina>) {
        let Some((t, paso)) = self.prueba_tinta else { return };
        let ms = t.elapsed().as_millis();
        let e = self.e();
        let px = |v: f32| (v * e) as i32;
        let siguiente = |n: u8| Some((t, n));
        match paso {
            0 if ms > 2000 => {
                if !self.anotando {
                    self.alternar_anotar();
                }
                let h = self.ventana_de_la_tinta();
                pixpin_shell::overlay::simular_raton(h, 0, px(300.0), px(420.0));
                for i in 1..=24 {
                    let x = 300.0 + i as f32 * 14.0;
                    let y = if i % 2 == 0 { 420.0 } else { 470.0 };
                    pixpin_shell::overlay::simular_raton(h, 1, px(x), px(y));
                }
                pixpin_shell::overlay::simular_raton(h, 2, px(636.0), px(420.0));
                self.prueba_tinta = siguiente(1);
            }
            1 if ms > 2800 => {
                if self.anotando {
                    self.alternar_anotar();
                }
                self.prueba_tinta = siguiente(2);
            }
            2 if ms > 5000 => {
                self.pedir(pagina, Orden::Bajar(200.0));
                self.prueba_tinta = siguiente(3);
            }
            3 if ms > 7000 => {
                if let Some(pg) = pagina {
                    pg.ejecutar("(function(){var R=window.__pixpinRiel,e=R&&R.donde(),w=e||window,y0=e?e.scrollTop:scrollY,t0=performance.now();function f(t){var k=Math.min(1,(t-t0)/2500);w.scrollTo(0,y0+900*k);if(k<1)requestAnimationFrame(f);}requestAnimationFrame(f);})()", Box::new(|_| {}));
                }
                self.prueba_tinta = siguiente(4);
            }
            4 if ms > 10500 => {
                if std::env::var_os("PIXPIN_RIEL_GLOBO_PRUEBA").is_some() {
                    let riel = self.colocado();
                    let c = riel.imprimir;
                    let (x, y) = (c.x + c.ancho as i32 / 2, c.y + c.alto as i32 / 2);
                    // Primero se le dice que el raton esta sobre el boton
                    // (crece) y luego, ya crecida, otra vez en su sitio nuevo.
                    let h = self.riel.ventana.handle().0 as isize;
                    pixpin_shell::overlay::simular_raton(h, 1, x - self.riel.area.x + self.vista.x, y - self.riel.area.y + self.vista.y);
                    self.globo_de_prueba = true;
                }
                self.prueba_tinta = None;
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------
    // Imprimir los marcos

    /// Corre `orden` en la pagina y espera a que conteste, bombeando los
    /// mensajes del hilo (lo que llegue para las ventanas se queda en su
    /// cola para la vuelta siguiente del visor).
    fn esperar_sitio(&mut self, pagina: &dyn Pagina, orden: Orden) -> Option<Sitio> {
        let celda: Rc<RefCell<Option<Sitio>>> = Rc::new(RefCell::new(None));
        let c = Rc::clone(&celda);
        pagina.ejecutar(&guion(orden), Box::new(move |json| *c.borrow_mut() = sitio_de(&json)));
        let t = Instant::now();
        while t.elapsed() < ESPERA_GUION {
            pixpin_shell::overlay::bombear_pendientes();
            if let Some(s) = celda.borrow_mut().take() {
                return Some(s);
            }
            pixpin_shell::overlay::esperar_eventos(Some(10));
        }
        tracing::warn!("riel: la pagina no contesto al imprimir");
        None
    }

    /// Deja que la pagina y la tinta se pinten y lleguen a la pantalla.
    fn dejar_pintarse(&mut self) {
        self.pintar_tinta();
        let t = Instant::now();
        while t.elapsed() < PARA_PINTARSE {
            pixpin_shell::overlay::bombear_pendientes();
            pixpin_shell::overlay::esperar_eventos(Some(15));
        }
        pixpin_shell::overlay::esperar_composicion();
    }

    /// Lo que se ve del documento con la camara de ahora: la vista entera
    /// o, si la pagina lo dijo, solo lo que ensena el documento (sin la
    /// barra del lector de notas).
    fn vista_en_documento(&self) -> [f64; 4] {
        let c = self.camara();
        let (w, h) = (self.vista.ancho as f64 / c.zoom as f64, self.vista.alto as f64 / c.zoom as f64);
        let toda = [c.x as f64, c.y as f64, c.x as f64 + w, c.y as f64 + h];
        match self.sitio.area_en_documento().filter(|_| self.opciones.con_pagina) {
            Some(a) => cortar(toda, a).unwrap_or(toda),
            None => toda,
        }
    }

    /// **La foto de un trozo del documento** que esta a la vista (`caja`,
    /// en el documento), en pixeles de pantalla.
    fn foto_del_trozo(&self, caja: [f64; 4]) -> Option<ImagenRgba> {
        let c = self.camara();
        let z = c.zoom as f64;
        let x0 = self.vista.x + ((caja[0] - c.x as f64) * z).round() as i32;
        let y0 = self.vista.y + ((caja[1] - c.y as f64) * z).round() as i32;
        let x1 = self.vista.x + ((caja[2] - c.x as f64) * z).round() as i32;
        let y1 = self.vista.y + ((caja[3] - c.y as f64) * z).round() as i32;
        let r = Rect { x: x0, y: y0, ancho: (x1 - x0).max(1) as u32, alto: (y1 - y0).max(1) as u32 };
        crate::marco_imprimir::foto_de_pantalla(r.interseccion(self.vista)?)
    }

    /// **La foto de un marco**: con pagina, la vista se lleva a cada trozo
    /// del marco y se cosen; sin (la vista previa de Office), lo que se ve.
    fn foto_del_marco(&mut self, caja: [f64; 4], pagina: Option<&dyn Pagina>) -> Option<ImagenRgba> {
        let Some(pg) = pagina.filter(|_| self.opciones.con_pagina) else {
            let c = cortar(caja, self.vista_en_documento())?;
            return self.foto_del_trozo(c);
        };
        let z = self.camara().zoom as f64;
        let ancho = ((caja[2] - caja[0]) * z).round().clamp(1.0, FOTO_MAXIMA) as u32;
        let alto = ((caja[3] - caja[1]) * z).round().clamp(1.0, FOTO_MAXIMA) as u32;
        let mut hoja = hoja_blanca(ancho, alto);
        let v = self.vista_en_documento();
        let (vw, vh) = (v[2] - v[0], v[3] - v[1]);
        for esquina in trozos(caja, vw, vh) {
            if let Some(s) = self.esperar_sitio(pg, Orden::Colocar(esquina[0] as f32, esquina[1] as f32)) {
                self.sitio = s;
            }
            self.dejar_pintarse();
            // Lo que toca de este trozo y de verdad se ve (al final de la
            // pagina la vista no baja mas que el documento).
            let trozo = [esquina[0], esquina[1], esquina[0] + vw, esquina[1] + vh];
            let Some(c) = cortar(caja, trozo).and_then(|t| cortar(t, self.vista_en_documento())) else { continue };
            if let Some(foto) = self.foto_del_trozo(c) {
                let z = self.camara().zoom as f64;
                pegar(&mut hoja, &foto, ((c[0] - caja[0]) * z).round() as i64, ((c[1] - caja[1]) * z).round() as i64);
            }
        }
        Some(hoja)
    }

    /// **Ctrl+P: imprime todos los marcos en grupo**, una hoja por marco y
    /// en su orden, como las laminas del plano. Sin marcos, lo que se ve.
    /// La vista vuelve a donde estaba.
    pub fn imprimir_marcos(&mut self, pagina: Option<&dyn Pagina>) {
        let antes = match pagina.filter(|_| self.opciones.con_pagina) {
            Some(pg) => self.esperar_sitio(pg, Orden::Mirar),
            None => None,
        };
        if let Some(s) = antes {
            self.sitio = s;
        }
        self.imprimiendo = true;
        self.riel.mostrar(false);
        // Las fotos, con los colores de verdad: la pagina en claro (y la
        // tinta sobre papel claro). Fotografiada en oscuro y aclarada luego
        // salia apagada y con los colores perdidos.
        let oscuro = self.oscuro;
        if oscuro {
            self.oscuro = false;
            if let Some(pg) = pagina.filter(|_| self.opciones.con_pagina) {
                pg.poner_oscuro(false);
            }
            let t = Instant::now();
            while t.elapsed() < PARA_ACLARARSE {
                pixpin_shell::overlay::bombear_pendientes();
                pixpin_shell::overlay::esperar_eventos(Some(15));
            }
        }
        // La capa de la pagina, sin las rayas de los marcos (y en claro).
        self.esperar_capa(pagina);
        self.dejar_pintarse();
        let mut fotos = Vec::new();
        let cajas: Vec<[f64; 4]> = if self.marcos.lista.is_empty() {
            vec![self.vista_en_documento()]
        } else {
            self.marcos.lista.iter().map(|m| m.caja).collect()
        };
        for caja in cajas {
            match self.foto_del_marco(caja, pagina) {
                Some(f) => fotos.push(f),
                None => tracing::warn!(?caja, "riel: un marco fuera de la vista no se pudo fotografiar"),
            }
        }
        if let (Some(s), Some(pg)) = (antes, pagina) {
            let [x, y] = s.esquina();
            if let Some(ahora) = self.esperar_sitio(pg, Orden::Colocar(x, y)) {
                self.sitio = ahora;
            }
        }
        if oscuro {
            self.oscuro = true;
            if let Some(pg) = pagina.filter(|_| self.opciones.con_pagina) {
                pg.poner_oscuro(true);
            }
        }
        self.imprimiendo = false;
        self.riel.mostrar(true);
        self.repintar_riel = true;
        self.repintar_tinta = true;
        self.sincronizar(pagina);
        self.pintar();
        tracing::info!(hojas = fotos.len(), "riel: marcos a imprimir");
        if let Err(e) = crate::marco_imprimir::imprimir_fotos(self.visor, fotos, &self.opciones.titulo) {
            tracing::warn!(?e, "riel: no se pudieron imprimir los marcos");
        }
    }

    /// Al cerrar el visor: lo dibujado, al disco.
    pub fn cerrar(mut self) {
        self.guardar_tinta();
        self.riel.ventana.ocultar();
        self.tinta_v.ventana.ocultar();
    }
}

/// **La vista en pixeles de pantalla**: la ventana del pin (`ventana`) y la
/// vista dentro de su zona de cliente (`vista`, la de
/// `PinLector::area_de_la_vista`; la ventana no tiene marco).
pub fn en_pantalla(ventana: Rect, vista: Rect) -> Rect {
    Rect { x: ventana.x + vista.x, y: ventana.y + vista.y, ..vista }
}

/// Las marcas en el orden del riel: de arriba abajo.
fn ordenadas(mut v: Vec<Marcador>) -> Vec<Marcador> {
    v.sort_by(|a, b| a.fraccion.partial_cmp(&b.fraccion).unwrap_or(std::cmp::Ordering::Equal));
    v
}

/// Las marcas en el `.pixpin-lectura` del documento, sin tocar lo demas
/// que guarde alli el lector de Word, y en el `anot-<uid>.marcas` de su
/// mensaje si es un adjunto del chat (vacias, vacio: no se borra).
fn guardar_marcas_en(documento: &Path, marcas: &[Marcador]) {
    let mut a = crate::anotado_del_adjunto::leer(documento);
    if a.marcadores == marcas {
        return;
    }
    a.marcadores = marcas.to_vec();
    if let Some(c) = documento.parent() {
        let _ = std::fs::create_dir_all(c);
    }
    if let Err(e) = crate::anotado_del_adjunto::escribir(documento, &a) {
        tracing::warn!(?e, "riel: no se pudieron guardar las marcas");
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_docs::lectura;

    #[test]
    fn el_sitio_de_la_pagina_se_entiende_y_da_la_fraccion() {
        let s = sitio_de(r#"{"x":0,"y":300,"arriba":300,"maximo":1200,"dpr":1.5}"#).expect("sitio");
        assert_eq!(s.fraccion(), 0.25);
        let c = s.camara();
        assert_eq!((c.x, c.y, c.zoom), (0.0, 300.0, 1.5));
        // Una pagina que cabe entera: siempre arriba.
        let corta = sitio_de(r#"{"x":0,"y":0,"arriba":0,"maximo":0,"dpr":1}"#).expect("sitio");
        assert_eq!(corta.fraccion(), 0.0);
        // Casos negativos: lo que no es un sitio no se toma por uno.
        assert!(sitio_de("null").is_none());
        assert!(sitio_de("\"hola\"").is_none());
    }

    #[test]
    fn la_tinta_sigue_el_zoom_de_la_pagina_y_el_pellizco() {
        // Ctrl+rueda al 150 % en una pantalla al 100 %: el dpr ya lo lleva.
        let s = sitio_de(r#"{"x":10,"y":300,"arriba":300,"maximo":900,"dpr":1.5}"#).unwrap();
        assert_eq!(s.camara().zoom, 1.5);
        // Pellizco al doble, adelantado 40 × 25 px CSS dentro de la vista.
        let p = sitio_de(r#"{"x":10,"y":300,"arriba":300,"maximo":900,"dpr":1.5,"vx":40,"vy":25,"escala":2}"#).unwrap();
        let c = p.camara();
        assert_eq!((c.x, c.y, c.zoom), (50.0, 325.0, 3.0));
        // Caso negativo: una escala rota no deja la tinta sin tamano.
        let roto = Sitio { escala: 0.0, ..p };
        assert!(roto.camara().zoom >= 0.1);
    }

    #[test]
    fn el_guion_lleva_su_orden_y_acota_la_fraccion() {
        assert!(guion(Orden::Mirar).ends_with(r#"("mirar",0,0)"#));
        assert!(guion(Orden::Ir(0.5)).ends_with(r#"("ir",0.5,0)"#));
        assert!(guion(Orden::Bajar(-100.0)).ends_with(r#"("bajar",-100,0)"#));
        assert!(guion(Orden::Colocar(12.5, 3400.0)).ends_with(r#"("colocar",12.5,3400)"#));
        // Caso negativo: una fraccion fuera de 0..1 no saca la pagina de si,
        // y un numero roto no llega al guion.
        assert!(guion(Orden::Ir(3.0)).ends_with(r#"("ir",1,0)"#));
        assert!(guion(Orden::Ir(-1.0)).ends_with(r#"("ir",0,0)"#));
        assert!(guion(Orden::Colocar(f32::NAN, f32::INFINITY)).ends_with(r#"("colocar",0,0)"#));
    }

    #[test]
    fn la_vista_de_una_marca_guarda_zoom_y_desplazamiento() {
        let s = Sitio { x: 0.0, y: 1834.5, arriba: 1834.5, maximo: 5000.0, dpr: 1.25, vx: 0.0, vy: 0.0, escala: 1.0, ax: 0.0, ay: 0.0, aw: 0.0, ah: 0.0, nuevo: false, maq: None };
        let t = vista_de_marca(1.25, &s);
        assert_eq!(marcos_doc::zoom_de_texto(&t), Some((1.25, 0.0, 1834.5)));
        // En el lector de notas lo que se desplaza empieza bajo su barra: la
        // marca guarda lo que se ve arriba del todo de esa caja.
        let n = Sitio { x: -10.0, y: 900.0, ax: 10.0, ay: 52.0, aw: 800.0, ah: 600.0, ..s };
        assert_eq!(n.esquina(), [0.0, 952.0]);
        assert_eq!(n.area_en_documento(), Some([0.0, 952.0, 800.0, 1552.0]));
        assert_eq!(s.area_en_documento(), None, "sin area dicha, la vista entera");
        // Y se guarda junto al documento con el id de la marca.
        let dir = std::env::temp_dir().join(format!("pixpin-riel-vistas-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let doc = dir.join("informe.html");
        let mut v = Vistas::de_documento(Some(&doc));
        v.poner("17", t);
        let otra = Vistas::de_documento(Some(&doc));
        assert_eq!(otra.de_marca("17").and_then(marcos_doc::zoom_de_texto), Some((1.25, 0.0, 1834.5)));
        // Caso negativo: una marca sin vista (de antes o del movil) no tiene.
        assert!(otra.de_marca("18").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_marco_que_no_cabe_se_fotografia_por_trozos() {
        // Cabe: un solo trozo, en su esquina.
        assert_eq!(trozos([10.0, 20.0, 300.0, 200.0], 800.0, 600.0), vec![[10.0, 20.0]]);
        // El doble de alto que la vista: dos trozos, uno debajo del otro.
        assert_eq!(trozos([0.0, 0.0, 500.0, 1100.0], 800.0, 600.0), vec![[0.0, 0.0], [0.0, 600.0]]);
        // Mas ancho y mas alto: cuatro.
        assert_eq!(trozos([0.0, 0.0, 1000.0, 700.0], 800.0, 600.0).len(), 4);
        // Caso negativo: justo del tamano de la vista no pide un trozo vacio.
        assert_eq!(trozos([0.0, 0.0, 800.0, 600.0], 800.0, 600.0).len(), 1);
    }

    #[test]
    fn los_trozos_se_cosen_en_su_sitio_sin_salirse() {
        let mut hoja = hoja_blanca(4, 4);
        let rojo = ImagenRgba { ancho: 2, alto: 2, pixeles: [255, 0, 0, 255].repeat(4) };
        pegar(&mut hoja, &rojo, 2, 2);
        let px = |h: &ImagenRgba, x: usize, y: usize| h.pixeles[(y * 4 + x) * 4];
        assert_eq!(px(&hoja, 3, 3), 255);
        assert_eq!(hoja.pixeles[(3 * 4 + 3) * 4 + 1], 0, "abajo a la derecha, rojo");
        assert_eq!(hoja.pixeles[1], 255, "arriba a la izquierda sigue en blanco");
        // Caso negativo: un trozo que se sale por fuera no rompe nada.
        pegar(&mut hoja, &rojo, 3, -1);
        pegar(&mut hoja, &rojo, 10, 10);
        assert_eq!(hoja.pixeles.len(), 64);
        assert_eq!(cortar([0.0, 0.0, 10.0, 10.0], [20.0, 0.0, 30.0, 10.0]), None);
    }

    #[test]
    fn las_marcas_van_de_arriba_abajo_y_se_guardan_junto_al_documento() {
        let v = ordenadas(vec![
            Marcador { id: 1, fraccion: 0.8, emoji: "⭐".into() },
            Marcador { id: 2, fraccion: 0.1, emoji: "🔖".into() },
        ]);
        assert_eq!(v.iter().map(|m| m.id).collect::<Vec<_>>(), vec![2, 1]);
        let dir = std::env::temp_dir().join(format!("pixpin-riel-web-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let doc = dir.join("informe.html");
        guardar_marcas_en(&doc, &v);
        assert_eq!(lectura::leer(&doc).marcadores, v);
        // Caso negativo: sin marcas nuevas no se reescribe lo que habia.
        guardar_marcas_en(&doc, &v);
        assert_eq!(lectura::leer(&doc).marcadores.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn las_marcas_y_la_tinta_de_una_pagina_del_chat_van_a_su_mensaje() {
        use crate::anotado_del_adjunto::pruebas::{con_adjunto, raiz};
        let r = raiz("riel-web-adjunto");
        let doc = con_adjunto(&r, "informe.html", b"<p>hola</p>");
        let b = crate::anotado_del_adjunto::base_de(&doc).expect("adjunto");
        // Lo de antes (junto a la pagina) pasa la primera vez.
        let vieja = lector_tinta::ruta_de_capa(&doc);
        std::fs::create_dir_all(vieja.parent().unwrap()).unwrap();
        std::fs::write(&vieja, r#"{"type":"excalidraw","version":2,"elements":[]}"#).unwrap();
        let tinta = crate::anotado_del_adjunto::tinta_de_la_pagina(&doc);
        assert_eq!(tinta, b.tinta());
        assert!(tinta.is_file(), "copiada al mensaje");
        assert!(tinta.to_string_lossy().replace('\\', "/").contains("/lienzos/anot-"));
        // Las marcas: en `anot-<uid>.marcas`, como las del Word.
        let v = vec![Marcador { id: 5, fraccion: 0.5, emoji: "⭐".into() }];
        guardar_marcas_en(&doc, &v);
        assert_eq!(std::fs::read_to_string(b.fichero(".marcas")).unwrap(), "5:0.5:⭐");
        assert_eq!(crate::anotado_del_adjunto::leer(&doc).marcadores, v);
        // Quitadas: vacio, no borrado.
        guardar_marcas_en(&doc, &[]);
        assert_eq!(std::fs::read_to_string(b.fichero(".marcas")).unwrap(), "");
        assert!(crate::anotado_del_adjunto::leer(&doc).marcadores.is_empty());
        let _ = std::fs::remove_dir_all(&r);
    }
}
