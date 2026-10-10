//! **tareas-v4** (5-oct): arriba, la barra comun de Galeria, Lecciones,
//! Tareas y Timeline ([`crate::cabecera`]) con el titulo, el buscador y el
//! conmutador Lista / Tarjetas. En «Tarjetas» cada tarea es un cuadrado
//! ([`super::rejilla`]) con su primera foto de fondo, sus emoticonos como
//! estados de WeChat ([`super::emoticonos`]) y, con varias fotos, la pila de
//! bordes detras. La vista elegida se recuerda en `tareas-vista.txt`.
//!
//! La ventana de Tareas (tareas-v3): arriba del todo el buscador, debajo la
//! caja para apuntar en el Inbox de «Mensajes guardados», y debajo las
//! tareas, **una tarjeta por tarea**, agrupadas por su lista con un
//! encabezado (la lista · su chat · cuantas quedan): lo pendiente primero y,
//! plegado en «Hechas (N)», lo hecho, atenuado y tachado.
//!
//! Cada tarjeta es como la fila del movil (`DeTareas` de `MiniActivity.kt`)
//! y la del panel del chat, en grande: la casilla, el texto entero en los
//! renglones que haga falta, sus imagenes y cuantos dias lleva. **La casilla
//! es lo unico que tacha**, como alli. Lo que se acaba de tachar se queda
//! unos segundos en su sitio antes de irse a «Hechas»: un clic sin querer se
//! deshace con otro en el mismo sitio.
//!
//! **Buscar** filtra al instante las tareas de todas las listas (ver
//! [`super::tarjetas`]); buscando, cada tarjeta lleva la chapita de su lista
//! y su chat, porque los grupos se mezclan, y lo hecho que coincide se ve.
//!
//! **Quitar y borrar** (el usuario, 4-oct: «añade la opcion de quitar tareas
//! y eliminar grupos de tareas facilmente»): con el raton encima, cada
//! tarjeta ensena su aspa, que la quita al momento (`Tareas.borrar` del
//! movil) con «Deshacer» en el aviso; y cada encabezado, su papelera, que
//! borra la lista entera tras preguntar, por el mismo camino que borrar el
//! mensaje en el chat (asi el movil tambien la quita).
//!
//! **Teclado**: Ctrl+F al buscador; Tab da la vuelta buscador, caja,
//! tarjetas; las flechas pasan de tarjeta en tarjeta, Espacio marca la del
//! foco, Supr la quita (y Ctrl+Z la devuelve mientras dura el aviso) e
//! Intro abre su «Mover a…» (y en la eleccion, las flechas eligen e
//! Intro mueve). Esc va por capas: cierra la eleccion, vacia lo escrito,
//! vacia la busqueda y, por ultimo, cierra la ventana.
//!
//! Una sola ventana, en su propio hilo, como la galeria de capturas. Se pone
//! al dia sola mirando cada segundo la huella de los cuadernos
//! ([`super::firma`]): lo que se tacha en el chat, en el movil o aqui sale
//! en todas partes.

#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use pixpin_geom::Rect;
use pixpin_proyecto::mini;
use pixpin_render::icono::material as mi;
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Idioma, Ubicacion};

use super::rejilla::{self, Vista};
use super::{Fila, Lista, emoticonos, tarjetas};
use crate::caja_dibujo::hex;
use crate::lecciones::ui;
use crate::overlay::Recursos;
use crate::v2::aviso::Aviso;
use crate::v2::color::{blanco, con_alfa, mezcla, negro};
use crate::v2::geom::encoger;
use crate::v2::pegar::{self, Pegada};
use crate::v2::{ACENTO, FONDO, GRIS, TEXTO};
use crate::ventanita::{Botones, centrado, dentro};

// Los colores, radios y piezas son los del sistema de diseno v2
// (`crate::v2`), los mismos de la galeria y el timeline. Aqui solo lo que es
// de las tareas.
/// Lo buscado, resaltado en el texto de la tarea.
const RESALTE: Color = Color {
    a: 0.38,
    ..hex(0xB38F00)
};
/// La ficha `[img 01]` de una imagen pegada: letra azul clara sobre el
/// azul muy aguado, para que se vea que no es texto.
const FICHA_LETRA: Color = hex(0x9cc4f0);
const FICHA_FONDO: Color = con_alfa(ACENTO, 0.24);
/// Lado de la miniatura de una imagen de tarea, en su tarjeta.
const MINI: f32 = 48.0;
/// Lado mayor de la vista de una ficha con el raton encima.
const VISTA: f32 = 160.0;

const VK_RETROCESO: u32 = 0x08;
const VK_TAB: u32 = 0x09;
const VK_ENTRAR: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_ESPACIO: u32 = 0x20;
const VK_FIN: u32 = 0x23;
const VK_INICIO: u32 = 0x24;
const VK_IZQUIERDA: u32 = 0x25;
const VK_ARRIBA: u32 = 0x26;
const VK_DERECHA: u32 = 0x27;
const VK_ABAJO: u32 = 0x28;
const VK_SUPRIMIR: u32 = 0x2E;
const VK_F: u32 = 0x46;
const VK_L: u32 = 0x4C;
const VK_T: u32 = 0x54;
const VK_V: u32 = 0x56;
const VK_Z: u32 = 0x5A;
const VK_F2: u32 = 0x71;

/// Dos clics en lo mismo antes de esto son un doble clic (el de la galeria).
const DOBLE_CLIC: Duration = Duration::from_millis(500);

/// El tamano de la ventana, en pixeles logicos.
const ANCHO_VENTANA: u32 = 660;
const ALTO_VENTANA: u32 = 800;

/// Medidas, en pixeles logicos.
/// El lado que se quiere para cada cuadrado de la vista en tarjetas, y el
/// hueco entre ellos (el de verdad lo reparte [`rejilla::columnas`]).
const LADO_CUADRADO: f32 = 200.0;
const HUECO_REJILLA: f32 = 12.0;
/// Encima de cada bloque de cuadrados: donde asoma la pila de los que
/// llevan varias fotos.
const RESPIRO: f32 = 12.0;
/// Una tarea hecha en la vista en cuadrados: una fila de este alto.
const FILA_HECHA: f32 = 44.0;
/// La letra de una tarjeta sin foto y la de una fila de las hechas: mas
/// grandes que antes (8-oct-2026, el usuario: «quiero que el texto sea mas
/// grande»).
const LETRA_CUADRO: f32 = 18.0;
const LETRA_FILA: f32 = 16.0;
/// Diametro de los circulos de emoticonos en la lista (en los cuadrados,
/// [`rejilla::CHAPA`]).
const ESTADO_LISTA: f32 = 34.0;
const CAJA: f32 = 44.0;
const MARGEN: f32 = 16.0;
/// Lo que baja la rueda por muesca en la lista, en «filas» de este alto
/// (x1,5).
const FILA: f32 = 36.0;
/// En cuadrados, la rueda baja media fila de la rejilla por muesca: con el
/// paso de la lista se tardaba una eternidad en pasar una fila de 212.
const MEDIA_FILA_REJILLA: f32 = (LADO_CUADRADO + HUECO_REJILLA) / 2.0;
const ENCABEZADO: f32 = 40.0;
const PLIEGUE: f32 = 40.0;
const HUECO: f32 = 8.0;
const ENTRE_GRUPOS: f32 = 18.0;
/// El objetivo de la casilla (el dibujo, de 24, va en medio).
const CASILLA: f32 = 40.0;
/// El objetivo del boton que quita una tarea, y de la papelera de una lista:
/// el boton de icono del v2.
const ASPA: f32 = crate::v2::BOTON;
const TARJETA_MIN: f32 = 56.0;
/// La letra del texto de una tarea.
const TAM_TAREA: f32 = 15.0;

/// Cuanto se queda arriba, tachada, una tarea recien marcada.
const SE_QUEDA: Duration = Duration::from_secs(5);

/// La ventana abierta (su HWND), -1 mientras nace, 0 sin ventana: una sola
/// a la vez, y pedirla otra vez la trae delante.
static ABIERTA: AtomicIsize = AtomicIsize::new(0);

/// La lista a la que hay que ir al abrir (su `Lista::clave`), si se pidio una:
/// al pulsar una lista de tareas en el chat se abre ESTA ventana en ella, y
/// no un panel distinto (el usuario, 8-oct-2026: «que la interfaz sea
/// unificada»).
/// Una cadena vacia es «abierta desde el boton»: la caja vuelve a apuntar en
/// el Inbox.
static PEDIDA: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub(super) fn abrir_en(idioma: Idioma, ubicacion: Ubicacion, aparato: String, lista: Option<String>) {
    *PEDIDA.lock().unwrap_or_else(|e| e.into_inner()) = Some(lista.unwrap_or_default());
    lanzar(idioma, ubicacion, aparato);
}

pub(super) fn abrir(idioma: Idioma, ubicacion: Ubicacion, aparato: String) {
    abrir_en(idioma, ubicacion, aparato, None);
}

fn lanzar(idioma: Idioma, ubicacion: Ubicacion, aparato: String) {
    let ya = ABIERTA.load(Ordering::SeqCst);
    if ya > 0 {
        VentanaOverlay::restaurar_de_hwnd(windows::Win32::Foundation::HWND(ya as *mut _));
        pixpin_shell::overlay::despertar(ya);
        return;
    }
    if ya < 0 {
        return;
    }
    ABIERTA.store(-1, Ordering::SeqCst);
    let lanzado = std::thread::Builder::new()
        .name("tareas".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(idioma);
            let hecho = crate::dispositivo_perdido::con_recursos("tareas", |r| {
                bucle(r, &textos, &ubicacion, &aparato)
            });
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo abrir la ventana de tareas");
            }
            ABIERTA.store(0, Ordering::SeqCst);
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de las tareas");
        ABIERTA.store(0, Ordering::SeqCst);
    }
}

// ------------------------------------------------------------ la caja

/// La caja de una linea para apuntar. Minima a proposito: escribir, borrar,
/// moverse con las flechas, Inicio/Fin y pegar con Ctrl+V; para una tarea no
/// hace falta mas.
///
/// **Ctrl+V con una imagen** (un mapa de bits, o un fichero de imagen
/// copiado en el Explorador) mete en el cursor una ficha `[img 01]`, como
/// pegar una imagen en la terminal de Claude Code: se pinta como una chapa
/// de otro color, Retroceso a su lado la borra entera y con el raton encima
/// ensena la imagen. Las imagenes se quedan aqui (las pegadas como mapa, en
/// un PNG temporal) hasta Intro, que las copia al chat del Inbox
/// (`tareas::apuntar_con`); la ficha que se borro del texto se lleva su
/// imagen.
#[derive(Debug, Clone, Default)]
struct Campo {
    texto: String,
    /// Donde esta el cursor, en bytes (siempre en el borde de una letra).
    cursor: usize,
    /// Las imagenes pegadas (`v2::pegar`, con su huella: una imagen, un
    /// nombre), cada una con el numero de su ficha.
    pegadas: Vec<(u32, Pegada)>,
    /// Lo que hay que decir despues de una tecla (la clave del aviso): un
    /// Ctrl+V con algo que no se puede pegar.
    aviso: Option<&'static str>,
    /// La ficha que va en el aviso (`$ficha`): la de una imagen que ya estaba.
    aviso_ficha: Option<String>,
}

/// Donde van los PNG de los mapas de bits pegados, hasta Intro.
fn carpeta_de_pegadas() -> PathBuf {
    std::env::temp_dir().join("pixpin-tareas")
}

impl Campo {
    fn vaciar(&mut self) {
        self.texto.clear();
        self.cursor = 0;
        for (_, p) in self.pegadas.drain(..) {
            p.soltar();
        }
    }

    /// Donde esta cada ficha de una imagen pegada que sigue en el texto (la
    /// primera vez que sale), ordenadas, con su numero y su imagen.
    fn fichas(&self) -> Vec<(std::ops::Range<usize>, u32, &Pegada)> {
        let mut v: Vec<_> = self
            .pegadas
            .iter()
            .filter_map(|(n, p)| {
                let f = mini::ficha_de_imagen(*n);
                self.texto.find(&f).map(|i| (i..i + f.len(), *n, p))
            })
            .collect();
        v.sort_by_key(|(r, _, _)| r.start);
        v
    }

    /// La ficha que acaba justo en el cursor, o la que empieza en el.
    fn ficha_en(&self, antes: bool) -> Option<std::ops::Range<usize>> {
        self.fichas().into_iter().map(|(r, _, _)| r).find(|r| {
            if antes {
                r.end == self.cursor
            } else {
                r.start == self.cursor
            }
        })
    }

    /// Las imagenes que van con la tarea: las que aun tienen su ficha.
    fn imagenes(&self) -> Vec<(u32, PathBuf)> {
        self.fichas()
            .into_iter()
            .map(|(_, n, p)| (n, p.ruta.clone()))
            .collect()
    }

    /// Lo que dice el aviso de la tarea apuntada: el texto sin las fichas.
    fn lo_que_se_lee(&self) -> String {
        let mut s = self.texto.clone();
        for (r, _, _) in self.fichas().into_iter().rev() {
            s.replace_range(r, " ");
        }
        let s = mini::saneado(&s);
        match self.fichas().first() {
            Some((_, n, _)) if s.is_empty() => mini::ficha_de_imagen(*n),
            _ => s,
        }
    }

    /// Mete la ficha de una imagen en el cursor, separada por blancos de lo
    /// que tenga alrededor.
    fn meter_imagen(&mut self, pegada: Pegada) {
        let numero = self.pegadas.iter().map(|(n, _)| *n).max().unwrap_or(0) + 1;
        let mut ficha = String::new();
        if self.texto[..self.cursor]
            .chars()
            .next_back()
            .is_some_and(|c| !c.is_whitespace())
        {
            ficha.push(' ');
        }
        ficha.push_str(&mini::ficha_de_imagen(numero));
        ficha.push(' ');
        self.texto.insert_str(self.cursor, &ficha);
        self.cursor += ficha.len();
        self.pegadas.push((numero, pegada));
    }

    /// **Una imagen, un nombre**: la ficha que sigue en el texto con una
    /// imagen de esa huella. Pegarla otra vez no mete nada; se avisa.
    fn ya_pegada(&mut self, huella: u64) -> bool {
        let Some(numero) = self
            .fichas()
            .into_iter()
            .find(|(_, _, p)| p.huella == huella)
            .map(|(_, n, _)| n)
        else {
            return false;
        };
        self.aviso = Some("tareas-pegar-repetida");
        self.aviso_ficha = Some(mini::ficha_de_imagen(numero));
        true
    }

    /// Mete las imagenes leidas del portapapeles o elegidas en el Explorador
    /// (las repetidas no: se avisa y su PNG temporal se borra).
    fn meter_imagenes(&mut self, nuevas: Vec<Pegada>) {
        for p in nuevas {
            if self.ya_pegada(p.huella) {
                p.soltar();
            } else {
                self.meter_imagen(p);
            }
        }
    }

    /// Lo que hace Ctrl+V con lo que haya en el portapapeles: el texto se
    /// escribe; las imagenes las lee `v2::pegar`, como en el timeline.
    fn pegar(&mut self, contenido: Option<pixpin_codec::ContenidoPortapapeles>) {
        use pixpin_codec::ContenidoPortapapeles as C;
        match contenido {
            Some(C::Texto(t)) => self.escribir(&t),
            Some(c @ (C::Imagen(_) | C::Rutas(_))) => {
                // Un mapa vacio no es nada que avisar: no habia imagen.
                let vacio = matches!(&c, C::Imagen(img) if img.ancho == 0 || img.alto == 0);
                let nuevas = pegar::pegadas_de(Some(c), &carpeta_de_pegadas());
                if nuevas.is_empty() && !vacio {
                    self.aviso = Some("tareas-pegar-no-imagen");
                }
                self.meter_imagenes(nuevas);
            }
            _ => {}
        }
    }

    /// Las imagenes elegidas con el icono de imagen de la caja: como si se
    /// hubieran copiado en el Explorador y pegado.
    fn elegidas(&mut self, rutas: Vec<PathBuf>) {
        if rutas.is_empty() {
            return;
        }
        self.pegar(Some(pixpin_codec::ContenidoPortapapeles::Rutas(rutas)));
    }

    fn escribir(&mut self, s: &str) {
        // Una tarea es una linea: lo pegado con saltos se junta.
        let s: String = s
            .chars()
            .map(|c| {
                if c == '\n' || c == '\r' || c == '\t' {
                    ' '
                } else {
                    c
                }
            })
            .filter(|c| !c.is_control())
            .collect();
        self.texto.insert_str(self.cursor, &s);
        self.cursor += s.len();
    }

    fn anterior(&self) -> usize {
        self.texto[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    fn siguiente(&self) -> usize {
        self.texto[self.cursor..]
            .chars()
            .next()
            .map_or(self.cursor, |c| self.cursor + c.len_utf8())
    }

    /// Una letra escrita (`WM_CHAR`). Las de control llegan por [`tecla`].
    fn letra(&mut self, c: char) -> bool {
        if c.is_control() {
            return false;
        }
        let mut b = [0u8; 4];
        self.escribir(c.encode_utf8(&mut b));
        true
    }

    /// Atiende una tecla. Devuelve si era suya.
    fn tecla(&mut self, vk: u32, ctrl: bool) -> bool {
        match vk {
            VK_RETROCESO => {
                if ctrl {
                    // La palabra de antes entera, como en cualquier caja.
                    let antes = self.texto[..self.cursor].trim_end();
                    let corte = antes.rfind(char::is_whitespace).map_or(0, |i| i + 1);
                    self.texto.replace_range(corte..self.cursor, "");
                    self.cursor = corte;
                } else if let Some(r) = self.ficha_en(true) {
                    // Una ficha se va entera: media ficha no es nada.
                    self.cursor = r.start;
                    self.texto.replace_range(r, "");
                } else if self.cursor > 0 {
                    let a = self.anterior();
                    self.texto.replace_range(a..self.cursor, "");
                    self.cursor = a;
                }
                true
            }
            VK_SUPRIMIR => {
                let s = self
                    .ficha_en(false)
                    .map_or_else(|| self.siguiente(), |r| r.end);
                self.texto.replace_range(self.cursor..s, "");
                true
            }
            // Las flechas saltan una ficha de una vez.
            VK_IZQUIERDA => {
                self.cursor = self
                    .ficha_en(true)
                    .map_or_else(|| self.anterior(), |r| r.start);
                true
            }
            VK_DERECHA => {
                self.cursor = self
                    .ficha_en(false)
                    .map_or_else(|| self.siguiente(), |r| r.end);
                true
            }
            VK_INICIO => {
                self.cursor = 0;
                true
            }
            VK_FIN => {
                self.cursor = self.texto.len();
                true
            }
            VK_V if ctrl => {
                self.pegar(pixpin_codec::portapapeles::leer());
                true
            }
            _ => false,
        }
    }
}

// --------------------------------------------------------------- la ventana

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Accion {
    Mover,
    Cerrar,
    /// Lo que no hace nada: el fondo, y la caja del aviso fuera de su boton.
    #[default]
    Fondo,
    Apuntar,
    /// Pulsar el buscador: el foco a el.
    EnfocarBuscar,
    /// Pulsar la caja de apuntar: el foco a ella.
    EnfocarApuntar,
    /// La ✕ del buscador (o el boton del estado vacio de la busqueda).
    VaciarBusqueda,
    /// Pulsar una tarjeta fuera de sus botones: el foco a ella.
    Enfocar(usize, usize),
    /// La casilla de la tarea `.1` (su `Fila::indice`, que es tambien su
    /// sitio en `Lista::filas`) de la lista `.0`.
    Marcar(usize, usize),
    /// Abrir o cerrar las hechas de la lista `.0`.
    Plegar(usize),
    /// «Mover a…» de la tarea `.1` del Inbox (lista `.0`): abre la eleccion
    /// del grupo.
    Repartir(usize, usize),
    /// El grupo `.0` de `Estado::destinos`, elegido para la tarea que se
    /// reparte.
    Destino(usize),
    /// Cerrar la eleccion del grupo sin mover nada.
    SoltarMenu,
    /// La imagen `.2` de la tarea `.1` de la lista `.0`: pinearla, como
    /// «Pinear» del chat.
    Imagen(usize, usize, usize),
    /// El aspa de la tarea `.1` de la lista `.0` (o Supr con su foco):
    /// quitarla, al momento y con «Deshacer» en el aviso.
    Quitar(usize, usize),
    /// La papelera del encabezado de la lista `.0`: borrarla entera, tras
    /// preguntar.
    BorrarLista(usize),
    /// Compartir la lista `.0` entera (8-oct-2026): la hoja de compartir de
    /// toda la app, con PDF, texto, HTML...
    CompartirLista(usize),
    /// «Deshacer» del aviso (o Ctrl+Z): devolver la ultima tarea quitada.
    Deshacer,
    /// El conmutador de la barra de arriba: lista o cuadrados.
    Vista(Vista),
    /// El icono de imagen de la caja de apuntar: elegir imagenes en el
    /// Explorador (lo mismo que pegarlas con Ctrl+V).
    ElegirImagen,
    /// La chapa «+N» de un cuadrado, o «Pinear imagenes» del menu: todas
    /// las imagenes de la tarea `.1` de la lista `.0` a la pantalla.
    PinearTodas(usize, usize),
    /// El circulo del emoticono `.2` de la tarea `.1` de la lista `.0`:
    /// buscar ese emoticono (son las etiquetas de las tareas).
    BuscarEmo(usize, usize, usize),
    /// Corregir el texto de la tarea `.1` de la lista `.0` (doble clic en
    /// su texto, F2 o «Editar» del menu): pasa a la caja de apuntar.
    Editar(usize, usize),
    /// «Copiar texto» del menu: el texto de la tarea al portapapeles.
    CopiarTexto(usize, usize),
    /// «Recordarmelo» de la tarea `.1` de la lista `.0`, a la hora local
    /// `.2` (8-oct-2026): la marca `⏰` va en su texto (`tareas::RELOJ`).
    Recordar(usize, usize, i64),
    /// Quitarle la hora a la tarea.
    OlvidarHora(usize, usize),
}

/// La ultima tarea quitada, para «Deshacer»: de que lista era, en que sitio
/// estaba y como estaba escrita (con su fecha y su estado).
#[derive(Debug, Clone)]
struct Quitada {
    proyecto: String,
    codigo: String,
    indice: usize,
    tarea: mini::Tarea,
}

/// La tarea de una accion, si es de una tarea: lo que hay bajo el raton
/// cuando se pulsa el boton derecho (su menu es el de esa tarea).
fn tarea_de(a: Accion) -> Option<(usize, usize)> {
    match a {
        Accion::Enfocar(l, f)
        | Accion::Marcar(l, f)
        | Accion::Repartir(l, f)
        | Accion::Imagen(l, f, _)
        | Accion::Quitar(l, f)
        | Accion::PinearTodas(l, f)
        | Accion::BuscarEmo(l, f, _)
        | Accion::Editar(l, f)
        | Accion::CopiarTexto(l, f)
        | Accion::Recordar(l, f, _)
        | Accion::OlvidarHora(l, f) => Some((l, f)),
        _ => None,
    }
}

/// Lo que hace un **doble clic** en una tarea fuera de sus botones (el
/// primer clic ya le dio el foco): en un cuadrado con foto, pinear la foto,
/// que es lo que se quiere de una foto; en lo demas, corregir el texto.
/// Lo que no es una tarea se queda como estaba.
fn al_doble_clic(a: Accion, vista: Vista, con_fotos: bool) -> Accion {
    match a {
        Accion::Enfocar(l, f) if vista == Vista::Tarjetas && con_fotos => Accion::Imagen(l, f, 0),
        Accion::Enfocar(l, f) => Accion::Editar(l, f),
        otra => otra,
    }
}

/// Las entradas del **menu del boton derecho** de una tarea, con su atajo
/// tras un tabulador (como el de la galeria) y la accion de cada una.
/// «Mover a…» solo en lo pendiente del Inbox y «Pinear» solo con imagenes:
/// lo que no se puede hacer no se ofrece.
fn menu_de_tarea(
    li: usize,
    fi: usize,
    hecha: bool,
    se_reparte: bool,
    imagenes: usize,
    con_hora: bool,
    textos: &Catalogo,
) -> Vec<(u32, String, Accion)> {
    let mut v = vec![(
        1,
        format!(
            "{}\tEspacio",
            textos.t(if hecha { "tareas5-desmarcar" } else { "tareas5-marcar" })
        ),
        Accion::Marcar(li, fi),
    )];
    v.push((2, format!("{}\tF2", textos.t("tareas5-editar")), Accion::Editar(li, fi)));
    if se_reparte {
        v.push((3, format!("{}\tIntro", textos.t("tareas-mover")), Accion::Repartir(li, fi)));
    }
    v.push((4, textos.t("tareas5-copiar-texto"), Accion::CopiarTexto(li, fi)));
    if imagenes > 0 {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("n", imagenes as i64);
        v.push((5, textos.t_args("tareas5-pinear-imagenes", &args), Accion::PinearTodas(li, fi)));
    }
    // La hora a la que recordarla: las mismas de «Recordarmelo» del chat.
    if !hecha {
        let ahora = pixpin_shell::entorno::ahora_local_ms();
        for (k, (clave, cuando)) in crate::recordatorios::atajos(ahora).into_iter().enumerate() {
            v.push((
                10 + k as u32,
                format!("⏰ {}", textos.t(clave)),
                Accion::Recordar(li, fi, cuando),
            ));
        }
    }
    if con_hora {
        v.push((20, textos.t("chat-recordatorio-quitar"), Accion::OlvidarHora(li, fi)));
    }
    v.push((6, format!("{}\tSupr", textos.t("tareas5-quitar")), Accion::Quitar(li, fi)));
    v
}


/// Donde va lo que se teclea.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Foco {
    Buscar,
    /// La caja de apuntar: el foco al abrir, como siempre.
    Apuntar,
    /// La tarjeta de la tarea `.1` de la lista `.0`: Espacio la marca, Intro
    /// abre «Mover a…».
    Tarjeta(usize, usize),
}

struct Estado {
    /// Las listas que se ensenan: las que tienen alguna tarea, el Inbox el
    /// primero.
    listas: Vec<Lista>,
    /// Adonde se puede mover lo del Inbox: todas las demas listas, tambien
    /// las vacias.
    destinos: Vec<Lista>,
    /// La tarea del Inbox que se esta repartiendo (lista y `Fila::indice`),
    /// con la eleccion del grupo abierta encima.
    repartiendo: Option<(usize, usize)>,
    /// El renglon de la eleccion del grupo que eligen las flechas.
    destino_elegido: usize,
    /// Los chats que se miraron, para la huella.
    proyectos: Vec<String>,
    firma: Vec<Option<(SystemTime, u64)>>,
    campo: Campo,
    /// Lo que se busca (una linea; Ctrl+V solo pega texto).
    buscar: ui::Campo,
    foco: Foco,
    /// Las listas con las hechas a la vista (por su clave). Empiezan todas
    /// plegadas; buscando se abren todas, para ver lo encontrado.
    abiertas: HashSet<String>,
    /// Las recien tachadas (clave de la lista y texto crudo) y cuando: se
    /// quedan arriba hasta [`SE_QUEDA`].
    recien: HashMap<(String, String), Instant>,
    hoy: pixpin_proyecto::mini::Fecha,
    scroll: f32,
    alto_contenido: f32,
    /// Lo colocado en el ultimo fotograma (desde lo alto del contenido): por
    /// donde van las flechas y que hay bajo el raton.
    colocadas: Vec<tarjetas::Colocada>,
    /// La pieza bajo el raton en el ultimo fotograma.
    encima: Option<tarjetas::Pieza>,
    /// Desplazar en el proximo fotograma lo justo para ver la tarjeta con
    /// el foco (se movio con las flechas).
    seguir_foco: bool,
    botones: Botones<Accion>,
    /// El aviso de abajo (`v2::aviso`, el mismo de la galeria y el
    /// timeline); con una tarea recien quitada lleva su «Deshacer».
    aviso: Option<Aviso<Accion>>,
    /// Arrastrando la ventana por su cabecera: donde se pulso, en pantalla,
    /// y donde estaba la ventana entonces.
    moviendo: Option<(pixpin_geom::Punto, Rect)>,
    /// Donde esta en este equipo cada imagen de una tarea, por su chat y su
    /// enlace. `None` es «aun no llego»: se vuelve a mirar al releer.
    rutas: HashMap<(String, String), Option<PathBuf>>,
    /// Las miniaturas de las imagenes (de las tareas y de lo pegado).
    minis: crate::miniaturas::Miniaturas,
    /// Donde quedo pintada cada ficha de la caja, y su imagen: con el raton
    /// encima se ensena.
    fichas_pintadas: Vec<(RectF, PathBuf)>,
    /// La tarea recien quitada, mientras su aviso ofrece «Deshacer».
    deshacer: Option<Quitada>,
    /// La ventana, duena del cuadro que pregunta antes de borrar una lista.
    hwnd: Option<windows::Win32::Foundation::HWND>,
    /// Lista o cuadrados: la elegida se recuerda en `tareas-vista.txt`.
    vista: Vista,
    /// En la vista de cuadrados, la caja de cada pieza del ultimo fotograma
    /// (desde el borde y lo alto del contenido): con ellas se pinta, se
    /// acierta el clic y se mueven las flechas. En la lista, vacia.
    celdas: Vec<rejilla::Celda>,
    /// La tarea que se esta corrigiendo en la caja de apuntar (como se
    /// leyo: al guardar se comprueba que sigue igual), o `None` si la caja
    /// apunta una nueva.
    corrigiendo: Option<(Lista, Fila)>,
    /// El ultimo clic y cuando: dos en lo mismo seguidos son un doble clic.
    ultimo_clic: Option<(Accion, Instant)>,
    /// La lista a la que llevar la vista en el proximo fotograma (ver
    /// [`PEDIDA`]).
    ir_a_lista: Option<String>,
    /// Abierta desde una lista del chat, lo apuntado va a ESA lista (como en
    /// el panel del chat que sustituye); desde el boton, al Inbox.
    apuntar_en: Option<Lista>,
}

impl Estado {
    fn nuevo() -> Estado {
        Estado {
            listas: Vec::new(),
            destinos: Vec::new(),
            repartiendo: None,
            destino_elegido: 0,
            proyectos: Vec::new(),
            firma: Vec::new(),
            campo: Campo::default(),
            buscar: ui::Campo::default(),
            foco: Foco::Apuntar,
            abiertas: HashSet::new(),
            recien: HashMap::new(),
            hoy: super::hoy(),
            scroll: 0.0,
            alto_contenido: 0.0,
            colocadas: Vec::new(),
            encima: None,
            seguir_foco: false,
            botones: Botones::default(),
            aviso: None,
            moviendo: None,
            rutas: HashMap::new(),
            minis: crate::miniaturas::Miniaturas::con_lado(VISTA as u32 * 2),
            fichas_pintadas: Vec::new(),
            deshacer: None,
            hwnd: None,
            vista: Vista::Lista,
            celdas: Vec::new(),
            corrigiendo: None,
            ultimo_clic: None,
            ir_a_lista: None,
            apuntar_en: None,
        }
    }

    fn recargar(&mut self, ubicacion: &Ubicacion) {
        let raiz = ubicacion.raiz();
        let (todas, proyectos) = super::reunir(raiz);
        self.firma = super::firma(raiz, &proyectos);
        // Las imagenes de las tareas, resueltas una vez por lectura y no en
        // cada fotograma (resolver un enlace lee el indice). Lo que ya se
        // encontro no se vuelve a buscar.
        let mut rutas = HashMap::new();
        for l in &todas {
            for f in &l.filas {
                for enlace in &f.imagenes {
                    let clave = (l.proyecto.clone(), enlace.clone());
                    let ya = self
                        .rutas
                        .get(&clave)
                        .cloned()
                        .flatten()
                        .filter(|r| r.is_file());
                    let ruta = ya.or_else(|| super::ruta_de_imagen(raiz, &l.proyecto, enlace));
                    rutas.insert(clave, ruta);
                }
            }
        }
        self.rutas = rutas;
        self.destinos = todas
            .iter()
            .filter(|l| !super::es_inbox(l))
            .cloned()
            .collect();
        // La tarea con el foco, por lo que es y no por su numero: releida,
        // puede estar en otro sitio.
        let con_foco = match self.foco {
            Foco::Tarjeta(li, fi) => self
                .listas
                .get(li)
                .and_then(|l| Some((l.clave(), l.filas.get(fi)?.crudo.clone()))),
            _ => None,
        };
        self.listas = todas.into_iter().filter(|l| !l.filas.is_empty()).collect();
        if let Some((clave, crudo)) = con_foco {
            self.foco = match tarjetas::reubicar(&self.listas, &clave, &crudo) {
                Some((li, fi)) => Foco::Tarjeta(li, fi),
                None => Foco::Apuntar,
            };
        }
        // Releida, la tarea que se repartia puede estar en otro sitio.
        self.repartiendo = None;
        self.proyectos = proyectos;
        self.hoy = super::hoy();
    }

    /// Un aviso nuevo tapa al anterior, y con el su «Deshacer»: lo que se
    /// ofrece deshacer es siempre lo que dice el aviso que se ve.
    fn decir(&mut self, t: String) {
        self.aviso = Some(Aviso::nuevo(t));
        self.deshacer = None;
    }


    fn buscando(&self) -> bool {
        !self.buscar.texto.trim().is_empty()
    }

    fn fila(&self, li: usize, fi: usize) -> Option<(&Lista, &Fila)> {
        let l = self.listas.get(li)?;
        Some((l, l.filas.get(fi)?))
    }

    /// Si la tarea tiene «Mover a…»: lo pendiente del Inbox.
    fn se_reparte(&self, li: usize, fi: usize) -> bool {
        self.fila(li, fi)
            .is_some_and(|(l, f)| super::es_inbox(l) && !f.hecha)
    }

    /// Donde esta la imagen `k` de la tarea `f` de la lista `l`.
    fn ruta_de(&self, l: &Lista, f: &Fila, k: usize) -> Option<PathBuf> {
        let enlace = f.imagenes.get(k)?;
        self.rutas
            .get(&(l.proyecto.clone(), enlace.clone()))
            .cloned()
            .flatten()
    }

    /// Las imagenes que se van a pintar: las de las tareas y las pegadas.
    fn imagenes_a_cargar(&self) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = self.campo.imagenes().into_iter().map(|(_, r)| r).collect();
        v.extend(self.rutas.values().flatten().cloned());
        v
    }

    fn vaciar_busqueda(&mut self) {
        self.buscar.poner("");
        self.scroll = 0.0;
        self.seguir_foco = true;
    }

    /// Si la tarea lleva alguna imagen (haya llegado o no a este equipo).
    fn con_fotos(&self, li: usize, fi: usize) -> bool {
        self.fila(li, fi).is_some_and(|(_, f)| !f.imagenes.is_empty())
    }

    /// Donde estan las imagenes de la tarea que ya estan en este equipo.
    fn rutas_de_todas(&self, li: usize, fi: usize) -> Vec<PathBuf> {
        let Some((l, f)) = self.fila(li, fi) else {
            return Vec::new();
        };
        (0..f.imagenes.len())
            .filter_map(|k| self.ruta_de(l, f, k))
            .collect()
    }

    /// Deja de corregir: la caja vuelve a apuntar tareas nuevas, vacia.
    fn soltar_correccion(&mut self) {
        if self.corrigiendo.take().is_some() {
            self.campo.vaciar();
        }
    }
}

/// El **menu del boton derecho** de una tarea, donde este el raton (el
/// mismo menu del sistema que el de la galeria). Devuelve si la ventana
/// sigue.
fn menu(
    e: &mut Estado,
    li: usize,
    fi: usize,
    textos: &Catalogo,
    ubicacion: &Ubicacion,
    aparato: &str,
) -> bool {
    let Some(hwnd) = e.hwnd else {
        return true;
    };
    let Some((_, f)) = e.fila(li, fi) else {
        return true;
    };
    let entradas = menu_de_tarea(
        li,
        fi,
        f.hecha,
        e.se_reparte(li, fi),
        f.imagenes.len(),
        super::hora_de_tarea(&f.texto).is_some(),
        textos,
    );
    e.foco = Foco::Tarjeta(li, fi);
    let lista: Vec<(u32, String)> = entradas.iter().map(|(id, t, _)| (*id, t.clone())).collect();
    match pixpin_shell::menu_llano(hwnd, &lista)
        .and_then(|id| entradas.iter().find(|(i, _, _)| *i == id))
    {
        Some(&(_, _, a)) => hacer(e, a, textos, ubicacion, aparato),
        None => true,
    }
}

fn bucle(
    recursos: &Recursos,
    textos: &Catalogo,
    ubicacion: &Ubicacion,
    aparato: &str,
) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = pixpin_shell::pantalla_de::monitor_de_la_ventana_activa()
        .and_then(|r| monitores.monitores().iter().find(|m| m.area == r))
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien as f32 / 100.0;
    let mut marco = centrado(
        monitor.area_trabajo,
        ANCHO_VENTANA,
        ALTO_VENTANA,
        monitor.escala_por_cien,
    );
    let mut ventana = VentanaOverlay::nueva_normal(marco, &textos.t("tareas-titulo"))
        .context("no se pudo abrir la ventana de tareas")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para las tareas")?;
    ventana.mostrar();
    ventana.enfocar();
    ABIERTA.store(ventana.handle().0 as isize, Ordering::SeqCst);

    let mut e = Estado::nuevo();
    e.hwnd = Some(ventana.handle());
    e.vista = Vista::leer(ubicacion.raiz());
    e.recargar(ubicacion);
    tracing::info!(listas = e.listas.len(), "ventana de tareas abierta");
    let mut mirado = Instant::now();
    let mut vivo = true;
    let mut pintar = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        if let Some(k) = PEDIDA.lock().unwrap_or_else(|e| e.into_inner()).take() {
            e.apuntar_en = e
                .listas
                .iter()
                .chain(&e.destinos)
                .find(|l| !k.is_empty() && l.clave() == k)
                .cloned();
            e.ir_a_lista = (!k.is_empty()).then_some(k);
            pintar = true;
        }
        for (h, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if h != ventana.handle() {
                continue;
            }
            pintar = true;
            let local = |p: pixpin_geom::Punto, m: Rect| ((p.x - m.x) as f32, (p.y - m.y) as f32);
            match ev {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::RatonMovido(p) => {
                    if let Some((desde, origen)) = e.moviendo {
                        marco = Rect {
                            x: origen.x + (p.x - desde.x),
                            y: origen.y + (p.y - desde.y),
                            ..origen
                        };
                        ventana.mover(marco);
                    }
                    e.botones.raton = local(p, marco);
                }
                EventoOverlay::BotonPulsado(p) => {
                    e.botones.raton = local(p, marco);
                    match e.botones.bajo_el_raton() {
                        Some(Accion::Mover) => {
                            e.moviendo = Some((p, marco));
                            ventana.capturar_raton();
                        }
                        Some(a) => {
                            let doble = e
                                .ultimo_clic
                                .is_some_and(|(b, t)| b == a && t.elapsed() < DOBLE_CLIC);
                            // Tras un doble clic se empieza de cero: un
                            // tercero no es otro doble.
                            e.ultimo_clic = (!doble).then(|| (a, Instant::now()));
                            let a = if doble {
                                let fotos = tarea_de(a).is_some_and(|(l, f)| e.con_fotos(l, f));
                                al_doble_clic(a, e.vista, fotos)
                            } else {
                                a
                            };
                            vivo = hacer(&mut e, a, textos, ubicacion, aparato);
                        }
                        None => {}
                    }
                }
                EventoOverlay::BotonDerechoPulsado(p) => {
                    e.botones.raton = local(p, marco);
                    if let Some((li, fi)) = e.botones.bajo_el_raton().and_then(tarea_de) {
                        vivo = menu(&mut e, li, fi, textos, ubicacion, aparato);
                    }
                }
                EventoOverlay::BotonSoltado(_) => {
                    if e.moviendo.take().is_some() {
                        ventana.soltar_raton();
                    }
                }
                // La rueda llega en 120 por muesca (y en trozos desde un
                // panel tactil). En cuadrados, media fila de la rejilla.
                EventoOverlay::Rueda(m) => {
                    let paso = match e.vista {
                        Vista::Lista => FILA * 1.5,
                        Vista::Tarjetas => MEDIA_FILA_REJILLA,
                    };
                    e.scroll -= m as f32 / 120.0 * paso * escala;
                }
                EventoOverlay::Caracter(c) => letra(&mut e, c),
                EventoOverlay::Tecla {
                    vk, ctrl, shift, ..
                } => vivo = tecla(&mut e, vk, ctrl, shift, textos, ubicacion, aparato),
                _ => {}
            }
        }
        if !vivo {
            // Los PNG temporales de lo pegado y sin apuntar, fuera.
            e.campo.vaciar();
            break;
        }

        // Lo que cambio fuera (el chat, el movil): la huella, cada segundo.
        if mirado.elapsed() >= Duration::from_secs(1) {
            mirado = Instant::now();
            if super::firma(ubicacion.raiz(), &e.proyectos) != e.firma {
                e.recargar(ubicacion);
                pintar = true;
            }
            // Pasada la medianoche, «hoy» cambia y con el la edad de todo.
            let hoy = super::hoy();
            if hoy != e.hoy {
                e.hoy = hoy;
                pintar = true;
            }
        }
        let antes = e.recien.len();
        e.recien.retain(|_, t| t.elapsed() < SE_QUEDA);
        if e.recien.len() != antes {
            pintar = true;
        }
        if e.aviso.as_ref().is_some_and(Aviso::caducado) {
            // Ido el aviso, ya no se ofrece deshacer: lo quitado, quitado.
            e.aviso = None;
            e.deshacer = None;
            pintar = true;
        }

        let mut faltan_minis = false;
        if pintar {
            // Las miniaturas, leidas y subidas ANTES del fotograma: crear
            // recursos de dibujo a medias no se puede. Las que no quepan en
            // este, en el siguiente.
            let rutas = e.imagenes_a_cargar();
            if !rutas.is_empty() {
                faltan_minis = e.minis.asegurar(&rutas, &motor);
            }
            if let Ok(d) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&d, |p: &Pintor| {
                    pintar_todo(&mut e, p, marco, escala, textos)
                });
                let _ = superficie.presentar();
            }
            pintar = faltan_minis;
        }
        let espera = if faltan_minis {
            16
        } else if e.aviso.is_some() || !e.recien.is_empty() {
            250
        } else {
            1_000
        };
        pixpin_shell::overlay::esperar_eventos(Some(espera));
    }
    tracing::info!("ventana de tareas cerrada");
    Ok(())
}

/// Una letra escrita (`WM_CHAR`): a la caja con el foco. Con el foco en una
/// tarjeta, el blanco es el de Espacio (ya marco) y se traga; lo demas va al
/// buscador si se esta buscando y si no a la caja de apuntar, como siempre.
fn letra(e: &mut Estado, c: char) {
    match e.foco {
        Foco::Buscar => {
            if e.buscar.letra(c) {
                e.scroll = 0.0;
            }
        }
        Foco::Apuntar => {
            e.campo.letra(c);
        }
        Foco::Tarjeta(..) if c == ' ' || c.is_control() => {}
        Foco::Tarjeta(..) if e.buscando() => {
            e.foco = Foco::Buscar;
            e.buscar.cursor = e.buscar.texto.len();
            e.buscar.letra(c);
            e.scroll = 0.0;
        }
        Foco::Tarjeta(..) => {
            e.foco = Foco::Apuntar;
            e.campo.letra(c);
        }
    }
}

/// Una tecla (`WM_KEYDOWN`). Devuelve si la ventana sigue.
#[allow(clippy::too_many_arguments)] // estado, tecla, modificadores, textos y donde escribir
fn tecla(
    e: &mut Estado,
    vk: u32,
    ctrl: bool,
    shift: bool,
    textos: &Catalogo,
    ubicacion: &Ubicacion,
    aparato: &str,
) -> bool {
    // Con la eleccion del grupo abierta, las teclas son suyas.
    if e.repartiendo.is_some() {
        match vk {
            VK_ESCAPE => e.repartiendo = None,
            VK_ARRIBA => e.destino_elegido = e.destino_elegido.saturating_sub(1),
            VK_ABAJO => {
                e.destino_elegido = (e.destino_elegido + 1).min(e.destinos.len().saturating_sub(1))
            }
            VK_ENTRAR if !e.destinos.is_empty() => {
                let d = e.destino_elegido.min(e.destinos.len() - 1);
                return hacer(e, Accion::Destino(d), textos, ubicacion, aparato);
            }
            _ => {}
        }
        return true;
    }
    let orden = tarjetas::en_orden(&e.colocadas);
    let buscando = e.buscando();
    let a_tarjeta = |e: &mut Estado, t: Option<(usize, usize)>| {
        if let Some((li, fi)) = t {
            e.foco = Foco::Tarjeta(li, fi);
            e.seguir_foco = true;
        }
    };
    match (vk, e.foco) {
        (VK_F, _) if ctrl => {
            e.foco = Foco::Buscar;
            e.buscar.cursor = e.buscar.texto.len();
        }
        // La vista, con el teclado: Ctrl+L la lista, Ctrl+T las tarjetas
        // (ni las cajas ni las tarjetas usan esas dos).
        (VK_L, _) if ctrl => {
            return hacer(e, Accion::Vista(Vista::Lista), textos, ubicacion, aparato);
        }
        (VK_T, _) if ctrl => {
            return hacer(e, Accion::Vista(Vista::Tarjetas), textos, ubicacion, aparato);
        }
        (VK_F2, Foco::Tarjeta(li, fi)) => {
            return hacer(e, Accion::Editar(li, fi), textos, ubicacion, aparato);
        }
        // Corrigiendo, Esc deja la tarea como estaba y la caja vuelve a
        // apuntar: antes que vaciar la busqueda o cerrar.
        (VK_ESCAPE, _) if e.corrigiendo.is_some() => {
            e.soltar_correccion();
            e.foco = Foco::Apuntar;
        }
        // Ctrl+Z, mientras el aviso lo ofrece, es su «Deshacer»: las cajas
        // de escribir no tienen deshacer propio que pisar.
        (VK_Z, _) if ctrl && e.deshacer.is_some() => {
            return hacer(e, Accion::Deshacer, textos, ubicacion, aparato);
        }
        // Supr sobre la tarjeta con el foco la quita, como su aspa.
        (VK_SUPRIMIR, Foco::Tarjeta(li, fi)) => {
            return hacer(e, Accion::Quitar(li, fi), textos, ubicacion, aparato);
        }
        (VK_ESCAPE, _) => {
            match tarjetas::escape(
                false,
                e.foco == Foco::Apuntar,
                e.campo.texto.is_empty(),
                e.buscar.texto.is_empty(),
            ) {
                tarjetas::Escape::Cerrar => return false,
                tarjetas::Escape::VaciarCaja => e.campo.vaciar(),
                tarjetas::Escape::VaciarBusqueda => e.vaciar_busqueda(),
                tarjetas::Escape::SoltarMenu => {}
            }
        }
        // Tab da la vuelta: buscador, caja, tarjetas.
        (VK_TAB, f) => {
            let primera = orden.first().copied();
            e.foco = match (f, shift) {
                (Foco::Buscar, false) => Foco::Apuntar,
                (Foco::Apuntar, false) => {
                    primera.map_or(Foco::Buscar, |(l, f)| Foco::Tarjeta(l, f))
                }
                (Foco::Tarjeta(..), false) => Foco::Buscar,
                (Foco::Buscar, true) => primera.map_or(Foco::Apuntar, |(l, f)| Foco::Tarjeta(l, f)),
                (Foco::Apuntar, true) => Foco::Buscar,
                (Foco::Tarjeta(..), true) => Foco::Apuntar,
            };
            e.seguir_foco = true;
        }
        (VK_ABAJO, Foco::Buscar) if buscando => a_tarjeta(e, orden.first().copied()),
        (VK_ABAJO, Foco::Buscar) => e.foco = Foco::Apuntar,
        (VK_ABAJO, Foco::Apuntar) => a_tarjeta(e, orden.first().copied()),
        (VK_ARRIBA, Foco::Apuntar) => e.foco = Foco::Buscar,
        // En cuadrados las cuatro flechas recorren la rejilla.
        (VK_ABAJO | VK_ARRIBA | VK_IZQUIERDA | VK_DERECHA, Foco::Tarjeta(li, fi))
            if e.vista == Vista::Tarjetas =>
        {
            let d = match vk {
                VK_ABAJO => rejilla::Direccion::Abajo,
                VK_ARRIBA => rejilla::Direccion::Arriba,
                VK_IZQUIERDA => rejilla::Direccion::Izquierda,
                _ => rejilla::Direccion::Derecha,
            };
            match rejilla::vecina(&e.celdas, Some((li, fi)), d) {
                Some(t) => a_tarjeta(e, Some(t)),
                None if buscando => e.foco = Foco::Buscar,
                None => e.foco = Foco::Apuntar,
            }
        }
        (VK_ABAJO | VK_ARRIBA, Foco::Tarjeta(li, fi)) => {
            let paso = if vk == VK_ABAJO { 1 } else { -1 };
            match tarjetas::vecina(&orden, Some((li, fi)), paso) {
                Some(t) => a_tarjeta(e, Some(t)),
                None if buscando => e.foco = Foco::Buscar,
                None => e.foco = Foco::Apuntar,
            }
        }
        (VK_INICIO, Foco::Tarjeta(..)) => a_tarjeta(e, orden.first().copied()),
        (VK_FIN, Foco::Tarjeta(..)) => a_tarjeta(e, orden.last().copied()),
        (VK_ESPACIO, Foco::Tarjeta(li, fi)) => {
            return hacer(e, Accion::Marcar(li, fi), textos, ubicacion, aparato);
        }
        // Intro en un cuadrado con foto la pinea, como su doble clic: en
        // cuadrados la foto es lo que se ve. Si no, «Mover a…» en el Inbox.
        (VK_ENTRAR, Foco::Tarjeta(li, fi)) => {
            if e.vista == Vista::Tarjetas && e.con_fotos(li, fi) {
                return hacer(e, Accion::Imagen(li, fi, 0), textos, ubicacion, aparato);
            }
            if e.se_reparte(li, fi) {
                return hacer(e, Accion::Repartir(li, fi), textos, ubicacion, aparato);
            }
        }
        // Intro en el buscador lleva a lo primero encontrado.
        (VK_ENTRAR, Foco::Buscar) => a_tarjeta(e, orden.first().copied()),
        (VK_ENTRAR, Foco::Apuntar) => {
            return hacer(e, Accion::Apuntar, textos, ubicacion, aparato);
        }
        (_, Foco::Buscar) => {
            let antes = e.buscar.texto.clone();
            e.buscar.tecla(vk, ctrl, shift, false);
            if e.buscar.texto != antes {
                e.scroll = 0.0;
            }
        }
        (_, Foco::Apuntar) => {
            e.campo.tecla(vk, ctrl);
            avisar_de_la_caja(e, textos);
        }
        _ => {}
    }
    true
}

/// Lo que la caja de apuntar dejo por decir (un Ctrl+V sin imagen, una
/// imagen que ya estaba), al aviso de abajo.
fn avisar_de_la_caja(e: &mut Estado, textos: &Catalogo) {
    if let Some(clave) = e.campo.aviso.take() {
        let aviso = match e.campo.aviso_ficha.take() {
            Some(ficha) => {
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("ficha", ficha);
                textos.t_args(clave, &args)
            }
            None => textos.t(clave),
        };
        e.decir(aviso);
    }
}

/// Lo que hace un clic (o Intro). Devuelve si la ventana sigue.
fn hacer(
    e: &mut Estado,
    a: Accion,
    textos: &Catalogo,
    ubicacion: &Ubicacion,
    aparato: &str,
) -> bool {
    let raiz = ubicacion.raiz();
    match a {
        Accion::Mover | Accion::Fondo => {}
        Accion::Cerrar => return false,
        Accion::Vista(v) => {
            if e.vista != v {
                e.vista = v;
                v.guardar(raiz);
                // La tarjeta con el foco sigue a la vista en su nuevo sitio.
                e.seguir_foco = true;
            }
        }
        Accion::EnfocarBuscar => e.foco = Foco::Buscar,
        Accion::EnfocarApuntar => e.foco = Foco::Apuntar,
        Accion::VaciarBusqueda => {
            e.vaciar_busqueda();
            e.foco = Foco::Buscar;
        }
        Accion::Enfocar(li, fi) => e.foco = Foco::Tarjeta(li, fi),
        Accion::Plegar(i) => {
            if let Some(l) = e.listas.get(i) {
                let clave = l.clave();
                if !e.abiertas.remove(&clave) {
                    e.abiertas.insert(clave);
                }
            }
        }
        // Corrigiendo, Intro guarda la tarea corregida en su sitio, por el
        // mismo camino que marcarla (`cuaderno::reemplazar` del mensaje).
        Accion::Apuntar if e.corrigiendo.is_some() => {
            let Some((lista, fila)) = e.corrigiendo.clone() else {
                return true;
            };
            match super::corregir(raiz, &lista, &fila, &e.campo.texto, &e.campo.imagenes()) {
                Ok(true) => {
                    e.decir(textos.t("tareas5-corregida"));
                    e.soltar_correccion();
                    crate::ventana_chat::refrescar();
                }
                Ok(false) => {
                    e.decir(textos.t("tareas-cambio"));
                    e.soltar_correccion();
                }
                Err(err) => {
                    tracing::warn!(?err, "tareas: no se pudo corregir");
                    e.decir(err.aviso(textos));
                }
            }
            e.recargar(ubicacion);
            // El foco, a la corregida: su texto cambio, pero no su sitio en
            // la lista (su numero es tambien su sitio en `Lista::filas`).
            let sitio = e
                .listas
                .iter()
                .position(|l| l.clave() == lista.clave())
                .filter(|&li| fila.indice < e.listas[li].filas.len());
            if let Some(li) = sitio {
                e.foco = Foco::Tarjeta(li, fila.indice);
                e.seguir_foco = true;
            }
        }
        Accion::Editar(li, fi) => {
            let Some((l, f)) = e.fila(li, fi).map(|(l, f)| (l.clone(), f.clone())) else {
                return true;
            };
            e.campo.vaciar();
            e.campo.escribir(&f.texto);
            e.corrigiendo = Some((l, f));
            e.repartiendo = None;
            e.foco = Foco::Apuntar;
        }
        Accion::ElegirImagen => {
            let Some(h) = e.hwnd else {
                return true;
            };
            let rutas = pixpin_shell::elegir::pedir_imagenes(h);
            e.foco = Foco::Apuntar;
            e.campo.cursor = e.campo.texto.len();
            e.campo.elegidas(rutas);
            avisar_de_la_caja(e, textos);
        }
        Accion::PinearTodas(li, fi) => {
            let rutas = e.rutas_de_todas(li, fi);
            if rutas.is_empty() {
                e.decir(textos.t("tareas-imagen-no-esta"));
            } else if pixpin_shell::mensajero::enviar_ficheros(&rutas) {
                e.decir(textos.t("chat-pineado"));
            } else {
                tracing::warn!("tareas: no contesta la ventana principal");
            }
        }
        Accion::BuscarEmo(li, fi, k) => {
            let emo = e
                .fila(li, fi)
                .and_then(|(_, f)| emoticonos::emoticonos_de(&f.texto).1.get(k).cloned());
            if let Some(emo) = emo {
                e.buscar.poner(&emo);
                e.buscar.cursor = e.buscar.texto.len();
                e.foco = Foco::Buscar;
                e.scroll = 0.0;
            }
        }
        Accion::Recordar(li, fi, _) | Accion::OlvidarHora(li, fi) => {
            let Some((l, f)) = e.fila(li, fi).map(|(l, f)| (l.clone(), f.clone())) else {
                return true;
            };
            let (texto, cuando) = match a {
                Accion::Recordar(_, _, t) => (super::con_hora(&f.texto, t), Some(t)),
                _ => (super::sin_hora(&f.texto), None),
            };
            match super::corregir(raiz, &l, &f, &texto, &[]) {
                Ok(true) => {
                    // Al vigia, para que suene (o deje de sonar).
                    crate::recordatorios::releer(raiz);
                    crate::ventana_chat::refrescar();
                    e.recargar(ubicacion);
                    match cuando {
                        Some(t) => {
                            let mut args = fluent_bundle::FluentArgs::new();
                            args.set(
                                "cuando",
                                crate::recordatorios::cuando_legible(
                                    t,
                                    pixpin_shell::entorno::ahora_local_ms(),
                                ),
                            );
                            e.decir(textos.t_args("tareas-recordare", &args));
                        }
                        None => e.decir(textos.t("tareas-sin-recordatorio")),
                    }
                }
                Ok(false) => e.recargar(ubicacion),
                Err(err) => {
                    tracing::warn!(?err, "tareas: no se pudo poner la hora");
                    e.decir(err.aviso(textos));
                }
            }
        }
        Accion::CopiarTexto(li, fi) => {
            let Some((_, f)) = e.fila(li, fi) else {
                return true;
            };
            match pixpin_codec::portapapeles::copiar_texto(&f.texto) {
                Ok(()) => e.decir(textos.t("tareas5-copiada")),
                Err(err) => tracing::warn!(?err, "tareas: no se pudo copiar el texto"),
            }
        }
        Accion::Apuntar => {
            if e.campo.texto.trim().is_empty() {
                return true;
            }
            let hecho = match &e.apuntar_en {
                Some(l) => super::apuntar_en(raiz, aparato, l, &e.campo.texto, &e.campo.imagenes()),
                None => super::apuntar_con(raiz, aparato, &e.campo.texto, &e.campo.imagenes()),
            };
            match hecho {
                Ok(lista) => {
                    let mut args = fluent_bundle::FluentArgs::new();
                    args.set("tarea", e.campo.lo_que_se_lee());
                    args.set("lista", crate::pedidos::nombre_de_lista(&lista));
                    e.decir(textos.t_args("pedido-tarea-anadida", &args));
                    e.campo.vaciar();
                    crate::ventana_chat::refrescar();
                    e.recargar(ubicacion);
                }
                Err(err) => {
                    tracing::warn!(?err, "tareas: no se pudo apuntar");
                    e.decir(err.aviso(textos));
                }
            }
        }
        Accion::Imagen(li, fi, k) => {
            let ruta = e.fila(li, fi).and_then(|(l, f)| e.ruta_de(l, f, k));
            match ruta {
                // Al `main`, como «Pinear» del chat: es quien tiene los pines.
                Some(r) if pixpin_shell::mensajero::enviar_ficheros(std::slice::from_ref(&r)) => {
                    e.decir(textos.t("chat-pineado"));
                }
                Some(r) => {
                    tracing::warn!(ruta = %r.display(), "tareas: no contesta la ventana principal")
                }
                None => e.decir(textos.t("tareas-imagen-no-esta")),
            }
        }
        Accion::Repartir(li, fi) => {
            e.foco = Foco::Tarjeta(li, fi);
            e.repartiendo = Some((li, fi));
            e.destino_elegido = 0;
        }
        Accion::SoltarMenu => e.repartiendo = None,
        Accion::Destino(di) => {
            let Some((li, fi)) = e.repartiendo.take() else {
                return true;
            };
            let (Some(desde), Some(hasta)) =
                (e.listas.get(li).cloned(), e.destinos.get(di).cloned())
            else {
                return true;
            };
            let Some(fila) = desde.filas.get(fi).cloned() else {
                return true;
            };
            match super::mover(raiz, &desde, &fila, &hasta) {
                Ok(true) => {
                    let mut args = fluent_bundle::FluentArgs::new();
                    args.set("lista", hasta.titulo.clone());
                    e.decir(textos.t_args("tareas-movida", &args));
                    crate::ventana_chat::refrescar();
                }
                Ok(false) => e.decir(textos.t("tareas-cambio")),
                Err(err) => {
                    tracing::warn!(?err, "tareas: no se pudo mover");
                    e.decir(err.aviso(textos));
                }
            }
            e.recargar(ubicacion);
        }
        Accion::Marcar(li, fi) => {
            let Some(lista) = e.listas.get(li).cloned() else {
                return true;
            };
            let Some(fila) = lista.filas.get(fi).cloned() else {
                return true;
            };
            let hecha = !fila.hecha;
            match super::marcar(raiz, &lista, &fila, hecha) {
                Ok(true) => {
                    let clave = (lista.clave(), fila.crudo.clone());
                    if hecha {
                        e.recien.insert(clave, Instant::now());
                    } else {
                        e.recien.remove(&clave);
                    }
                    crate::ventana_chat::refrescar();
                }
                Ok(false) => e.decir(textos.t("tareas-cambio")),
                Err(err) => {
                    tracing::warn!(?err, "tareas: no se pudo marcar");
                    e.decir(err.aviso(textos));
                }
            }
            // Se relee siempre: con lo escrito, o con lo que cambio fuera.
            e.recargar(ubicacion);
        }
        Accion::Quitar(li, fi) => {
            let Some((lista, fila)) = e.fila(li, fi).map(|(l, f)| (l.clone(), f.clone())) else {
                return true;
            };
            // Si el foco era suyo pasa a la de al lado (la de abajo, o la de
            // arriba si era la ultima): asi se quitan varias seguidas con Supr.
            let siguiente = if e.foco == Foco::Tarjeta(li, fi) {
                let orden = tarjetas::en_orden(&e.colocadas);
                orden
                    .iter()
                    .position(|&x| x == (li, fi))
                    .and_then(|p| {
                        orden
                            .get(p + 1)
                            .or_else(|| p.checked_sub(1).and_then(|q| orden.get(q)))
                    })
                    .and_then(|&(l, f)| e.fila(l, f))
                    .map(|(l, f)| (l.clave(), f.crudo.clone()))
            } else {
                None
            };
            match super::quitar(raiz, &lista, &fila) {
                Ok(Some(tarea)) => {
                    let mut args = fluent_bundle::FluentArgs::new();
                    let nombre = if fila.texto.is_empty() {
                        mini::ficha_de_imagen(1)
                    } else {
                        fila.texto.clone()
                    };
                    args.set("tarea", nombre);
                    e.decir(textos.t_args("tareas3-quitada", &args));
                    // Despues de `decir`, que suelta el «Deshacer» anterior:
                    // el aviso con su boton, y lo que deshace.
                    e.aviso = Some(Aviso::con_deshacer(
                        textos.t_args("tareas3-quitada", &args),
                        textos.t("tareas3-deshacer"),
                        Accion::Deshacer,
                    ));
                    e.deshacer = Some(Quitada {
                        proyecto: lista.proyecto.clone(),
                        codigo: lista.codigo.clone(),
                        indice: fila.indice,
                        tarea,
                    });
                    e.recien.remove(&(lista.clave(), fila.crudo.clone()));
                    crate::ventana_chat::refrescar();
                }
                Ok(None) => e.decir(textos.t("tareas3-cambio-quitar")),
                Err(err) => {
                    tracing::warn!(?err, "tareas: no se pudo quitar");
                    e.decir(err.aviso(textos));
                }
            }
            e.recargar(ubicacion);
            if let Some((l, f)) =
                siguiente.and_then(|(clave, crudo)| tarjetas::reubicar(&e.listas, &clave, &crudo))
            {
                e.foco = Foco::Tarjeta(l, f);
                e.seguir_foco = true;
            }
        }
        Accion::Deshacer => {
            let Some(q) = e.deshacer.take() else {
                return true;
            };
            let crudo = q.tarea.texto.clone();
            match super::reponer(raiz, &q.proyecto, &q.codigo, q.indice, q.tarea) {
                Ok(()) => {
                    e.decir(textos.t("tareas3-repuesta"));
                    crate::ventana_chat::refrescar();
                }
                Err(err) => {
                    tracing::warn!(?err, "tareas: no se pudo devolver la tarea");
                    e.decir(err.aviso(textos));
                }
            }
            e.recargar(ubicacion);
            // La devuelta, con el foco: se ve donde volvio.
            let clave = format!("{}/{}", q.proyecto, q.codigo);
            if let Some((l, f)) = tarjetas::reubicar(&e.listas, &clave, &crudo) {
                e.foco = Foco::Tarjeta(l, f);
                e.seguir_foco = true;
            }
        }
        Accion::CompartirLista(li) => {
            let Some(lista) = e.listas.get(li).cloned() else {
                return true;
            };
            match crate::pedidos::mensaje_de(raiz, &lista.proyecto, &lista.codigo) {
                Ok(m) => crate::compartir::ventana::abrir(
                    crate::compartir::idioma_de(ubicacion),
                    ubicacion.clone(),
                    crate::compartir::Cosa::Mensajes {
                        raiz: raiz.to_path_buf(),
                        proyecto: lista.proyecto.clone(),
                        titulo: lista.titulo.clone(),
                        mensajes: vec![m],
                    },
                ),
                Err(err) => {
                    tracing::warn!(?err, "tareas: no se pudo leer la lista para compartirla");
                    e.decir(err.aviso(textos));
                }
            }
        }
        Accion::BorrarLista(li) => {
            let Some(lista) = e.listas.get(li).cloned() else {
                return true;
            };
            // Se lleva todas sus tareas y no tiene «Deshacer»: se pregunta,
            // con «No» por defecto, como al borrar mensajes en el chat.
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("lista", lista.titulo.clone());
            args.set("chat", lista.chat.clone());
            args.set("n", lista.filas.len() as i64);
            let pregunta = textos.t_args("tareas3-borrar-lista-aviso", &args);
            let si = e.hwnd.is_some_and(|h| {
                pixpin_shell::confirmar_destructivo(h, &textos.t("tareas3-borrar-lista"), &pregunta)
            });
            if !si {
                return true;
            }
            match super::borrar_lista(raiz, &lista) {
                Ok(()) => {
                    e.decir(textos.t_args("tareas3-lista-borrada", &args));
                    e.abiertas.remove(&lista.clave());
                    crate::ventana_chat::refrescar();
                }
                Err(err) => {
                    tracing::warn!(?err, "tareas: no se pudo borrar la lista");
                    e.decir(err.aviso(textos));
                }
            }
            e.recargar(ubicacion);
        }
    }
    true
}

// ---------------------------------------------------------------- pintar

/// Donde va la caja de apuntar (debajo de la barra comun de arriba, que
/// lleva el buscador) y donde empieza el contenido desplazable, ya con la
/// escala: `(caja, contenido)`.
fn alturas(s: f32) -> (f32, f32) {
    let caja = crate::cabecera::ALTO * s + 12.0 * s;
    (caja, caja + CAJA * s + 16.0 * s)
}

/// Si una celda de la rejilla es una fila (una hecha, ver
/// `rejilla::disponer`) y no un cuadrado.
/// Si la caja es una fila de las hechas (a todo lo ancho) y no una tarjeta.
/// Por su ancho y no por su forma: una tarjeta de texto corto es mas ancha
/// que alta y se pintaba como fila, en un renglon y con «…» (lo vio el
/// usuario, 8-oct-2026).
fn es_fila(r: RectF, s: f32) -> bool {
    r.ancho > LADO_CUADRADO * s * 1.5
}

fn pintar_todo(e: &mut Estado, p: &Pintor, marco: Rect, s: f32, textos: &Catalogo) {
    let (w, h) = (marco.ancho as f32, marco.alto as f32);
    p.limpiar(FONDO);
    e.botones.vaciar();
    let todo = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: h,
    };
    e.botones.zona(todo, Accion::Fondo);
    let m = MARGEN * s;
    let ancho = w - 2.0 * m;
    let (y_caja, arriba) = alturas(s);

    // Lo que sale y donde: agrupar, medir cada tarjeta y colocarlas. Los
    // grupos son los mismos en las dos vistas; solo cambia como se colocan.
    let palabras = tarjetas::palabras(&e.buscar.texto);
    let buscando = !palabras.is_empty();
    let grupos = {
        let recien = &e.recien;
        tarjetas::agrupar(&e.listas, &palabras, e.hoy, &|l, f| {
            recien.contains_key(&(l.clave(), f.crudo.clone()))
        })
    };
    let (colocadas, celdas, total) = {
        let abierta = |li: usize| {
            buscando
                || e.listas
                    .get(li)
                    .is_some_and(|l| e.abiertas.contains(&l.clave()))
        };
        match e.vista {
            Vista::Lista => {
                let est: &Estado = e;
                let (c, total) = tarjetas::disponer(
                    &grupos,
                    &abierta,
                    &mut |li, fi| hechura(est, p, li, fi, ancho, s, buscando, textos).alto,
                    tarjetas::Medidas {
                        encabezado: ENCABEZADO * s,
                        pliegue: PLIEGUE * s,
                        hueco: HUECO * s,
                        entre_grupos: ENTRE_GRUPOS * s,
                    },
                );
                (c, Vec::new(), total)
            }
            Vista::Tarjetas => {
                let est: &Estado = e;
                let (c, total) = rejilla::disponer(
                    &grupos,
                    &abierta,
                    &mut |li, fi, lado| alto_de_cuadrado(est, p, li, fi, lado, s),
                    &rejilla::Medidas {
                        ancho,
                        lado: LADO_CUADRADO * s,
                        hueco: HUECO_REJILLA * s,
                        encabezado: ENCABEZADO * s,
                        pliegue: PLIEGUE * s,
                        entre_grupos: ENTRE_GRUPOS * s,
                        respiro: RESPIRO * s,
                        fila_hecha: FILA_HECHA * s,
                    },
                );
                (rejilla::colocadas(&c), c, total)
            }
        }
    };
    e.colocadas = colocadas;
    e.celdas = celdas;
    e.alto_contenido = total + 24.0 * s;
    let area = RectF {
        x: 0.0,
        y: arriba,
        ancho: w,
        alto: (h - arriba).max(0.0),
    };
    // Abierta desde una lista del chat: su encabezado arriba del todo.
    if let Some(k) = e.ir_a_lista.take()
        && let Some(li) = e.listas.iter().position(|l| l.clave() == k)
        && let Some(c) = e
            .colocadas
            .iter()
            .find(|c| c.pieza == tarjetas::Pieza::Encabezado(li))
    {
        e.scroll = (c.y - 8.0 * s).max(0.0);
    }
    if std::mem::take(&mut e.seguir_foco) {
        match e.foco {
            Foco::Tarjeta(li, fi) => {
                if let Some(c) = tarjetas::sitio_de(&e.colocadas, li, fi) {
                    e.scroll =
                        tarjetas::a_la_vista(c.y - 8.0 * s, c.alto + 16.0 * s, e.scroll, area.alto);
                }
            }
            _ => e.scroll = 0.0,
        }
    }
    e.scroll = e.scroll.clamp(0.0, (e.alto_contenido - area.alto).max(0.0));
    e.encima = if dentro(area, e.botones.raton) && e.repartiendo.is_none() {
        let y = e.botones.raton.1 - arriba + e.scroll;
        match e.vista {
            Vista::Lista => tarjetas::pieza_en(&e.colocadas, y),
            Vista::Tarjetas => rejilla::pieza_en(&e.celdas, e.botones.raton.0 - m, y),
        }
    } else {
        None
    };

    // El contenido, desplazable, ANTES que lo de arriba: asi la cabecera y
    // las cajas se apuntan despues y le ganan a una tarjeta que asome por
    // debajo.
    p.con_recorte(area, |p| {
        let y0 = arriba - e.scroll;
        if e.listas.is_empty() {
            let zona = RectF {
                x: m,
                y: area.y,
                ancho,
                alto: area.alto.min(320.0 * s),
            };
            crate::v2::vacio(p, zona, &mi::CHECKLIST, &textos.t("tareas-vacia"), s);
        } else if grupos.is_empty() {
            pintar_sin_resultados(e, p, m, area.y, ancho, s, textos);
        } else {
            // Cada pieza en su caja de ventana: la de la lista ocupa todo lo
            // ancho; la de la rejilla, la que le dio `rejilla::disponer`.
            let piezas: Vec<(tarjetas::Pieza, RectF)> = match e.vista {
                Vista::Lista => e
                    .colocadas
                    .iter()
                    .map(|c| {
                        let r = RectF {
                            x: m,
                            y: y0 + c.y,
                            ancho,
                            alto: c.alto,
                        };
                        (c.pieza, r)
                    })
                    .collect(),
                Vista::Tarjetas => e
                    .celdas
                    .iter()
                    .map(|c| {
                        let r = RectF {
                            x: m + c.caja.x,
                            y: y0 + c.caja.y,
                            ..c.caja
                        };
                        (c.pieza, r)
                    })
                    .collect(),
            };
            // La pila de un cuadrado asoma un poco por encima: se cuenta.
            let asoma = 2.0 * RESPIRO * s;
            for (pieza, r) in piezas {
                if r.y + r.alto < area.y || r.y - asoma > area.y + area.alto {
                    continue;
                }
                match pieza {
                    tarjetas::Pieza::Encabezado(li) => {
                        if let Some(g) = grupos.iter().find(|g| g.lista == li) {
                            pintar_encabezado(e, p, g, m, r.y, ancho, s, textos);
                        }
                    }
                    tarjetas::Pieza::Pliegue(li) => {
                        let n = grupos
                            .iter()
                            .find(|g| g.lista == li)
                            .map_or(0, |g| g.hechas.len());
                        pintar_pliegue(e, p, li, n, m, r.y, ancho, s, buscando, textos);
                    }
                    tarjetas::Pieza::Tarjeta(li, fi) => match e.vista {
                        Vista::Lista => pintar_tarjeta(e, p, li, fi, r, s, &palabras, textos),
                        Vista::Tarjetas if es_fila(r, s) => {
                            pintar_fila_hecha(e, p, li, fi, r, s, &palabras, textos)
                        }
                        Vista::Tarjetas => pintar_cuadrado(e, p, li, fi, r, s, &palabras, textos),
                    },
                }
            }
        }
    });

    // Lo de arriba, tapando lo que se desplazo bajo ello: el fondo de la
    // caja de apuntar, la caja, y encima de todo la barra comun.
    p.rellenar(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho: w,
            alto: arriba,
        },
        FONDO,
    );
    // El subtitulo dice cuantas listas hay (o, buscando, cuantas salen): lo
    // pendiente ya lo cuenta cada grupo en su encabezado, y repetirlo arriba
    // era decir dos veces lo mismo.
    let resumen = if buscando {
        let n: usize = grupos.iter().map(|g| g.arriba.len() + g.hechas.len()).sum();
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("n", n as i64);
        textos.t_args("tareas3-encontradas", &args)
    } else {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("listas", e.listas.len() as i64);
        textos.t_args("tareas5-subtitulo", &args)
    };
    pintar_caja(e, p, m, y_caja, ancho, s, textos);
    // La barra comun de Galeria, Lecciones, Tareas y Timeline: el titulo,
    // el buscador y el conmutador Lista / Tarjetas, una sola pastilla.
    let titulo = textos.t("tareas-titulo");
    let pista = textos.t("tareas4-buscar");
    let pista_lista = textos.t("tareas5-vista-lista");
    let pista_tarjetas = textos.t("tareas5-vista-tarjetas");
    let iconos = [
        crate::cabecera::SegmentoIcono {
            icono: &mi::LIST,
            activo: e.vista == Vista::Lista,
            accion: Accion::Vista(Vista::Lista),
            pista: &pista_lista,
        },
        crate::cabecera::SegmentoIcono {
            icono: &mi::GRID_VIEW,
            activo: e.vista == Vista::Tarjetas,
            accion: Accion::Vista(Vista::Tarjetas),
            pista: &pista_tarjetas,
        },
    ];
    let cab = crate::cabecera::Cabecera {
        titulo: &titulo,
        subtitulo: &resumen,
        buscador: Some(crate::cabecera::Buscador {
            texto: &e.buscar.texto,
            pista: &pista,
            foco: e.foco == Foco::Buscar,
            cursor: Some(e.buscar.cursor),
            enfocar: Accion::EnfocarBuscar,
            vaciar: Accion::VaciarBusqueda,
        }),
        botones: Vec::new(),
        mover: Accion::Mover,
        cerrar: Accion::Cerrar,
    };
    crate::cabecera::pintar_con_iconos(p, &mut e.botones, w, s, &cab, &iconos);

    if e.repartiendo.is_some() {
        pintar_eleccion(e, p, w, h, s, textos);
    } else {
        pintar_vista_de_ficha(e, p, w, s);
    }

    // El aviso de abajo, el de todas las ventanas v2 (con su «Deshacer»
    // dentro cuando se acaba de quitar una tarea).
    if let Some(a) = &e.aviso {
        crate::v2::aviso::pintar(a, p, &mut e.botones, todo, s);
    }
}

/// La caja para apuntar: con el foco, lo que se escribe va a ella e Intro
/// lo apunta en el Inbox (o, corrigiendo, guarda la tarea corregida).
///
/// Sencilla a proposito (tareas-v5): a la izquierda el «+» (o el lapiz al
/// corregir), la pista «Nueva tarea…» y a la derecha el icono de imagen en
/// gris, que recuerda que Ctrl+V pega imagenes y, pulsado, deja elegirlas.
/// Antes la pista explicaba todo eso en una frase que no cabia.
fn pintar_caja(e: &mut Estado, p: &Pintor, x: f32, y: f32, ancho: f32, s: f32, textos: &Catalogo) {
    let caja = RectF {
        x,
        y,
        ancho,
        alto: CAJA * s,
    };
    let foco = e.foco == Foco::Apuntar;
    let borde = if foco {
        con_alfa(ACENTO, 0.6)
    } else if dentro(caja, e.botones.raton) {
        blanco(0.2)
    } else {
        blanco(0.1)
    };
    let radio = crate::v2::RADIO_TARJETA * s;
    p.rellenar_redondeado(caja, radio, borde);
    p.rellenar_redondeado(encoger(caja, 1.0 * s), radio - 1.0 * s, crate::v2::CAJA);
    e.botones.zona(caja, Accion::EnfocarApuntar);
    let tam = 15.0 * s;
    let (_, alto_linea) = p.medir_texto("Ag", tam);
    let ty = y + (caja.alto - alto_linea) / 2.0;
    let li = 18.0 * s;
    let icono = if e.corrigiendo.is_some() {
        &mi::EDIT
    } else {
        &mi::ADD
    };
    p.icono(
        icono,
        RectF {
            x: x + 14.0 * s,
            y: y + (caja.alto - li) / 2.0,
            ancho: li,
            alto: li,
        },
        ACENTO,
    );
    let tx = x + 14.0 * s + li + 10.0 * s;

    // A la derecha del todo, el icono de imagen: 40 de objetivo, 18 de dibujo.
    let lado = crate::v2::BOTON * s;
    let imagen = RectF {
        x: x + ancho - 2.0 * s - lado,
        y: y + (caja.alto - lado) / 2.0,
        ancho: lado,
        alto: lado,
    };
    let sobre_imagen = dentro(imagen, e.botones.raton);
    p.icono(
        &mi::IMAGE,
        crate::v2::geom::centrado((imagen.x + lado / 2.0, imagen.y + lado / 2.0), li),
        if sobre_imagen { TEXTO } else { GRIS },
    );
    e.botones.zona(imagen, Accion::ElegirImagen);
    let mut fin = imagen.x - 4.0 * s;

    // Con algo escrito, el boton de apuntar (o guardar) con su «Intro».
    if !e.campo.texto.trim().is_empty() {
        let rotulo = textos.t(if e.corrigiendo.is_some() {
            "tareas5-guardar"
        } else {
            "tareas-apuntar-boton"
        });
        let bw = ui::ancho_de_boton(p, false, &rotulo, Some("Intro"), s);
        let b = RectF {
            x: fin - bw,
            y: y + 4.0 * s,
            ancho: bw,
            alto: caja.alto - 8.0 * s,
        };
        ui::boton_v2(
            p,
            &mut e.botones,
            b,
            Accion::Apuntar,
            None,
            &rotulo,
            Some("Intro"),
            Some(crate::v2::AZUL_LLENO),
            Color::BLANCO,
            s,
        );
        fin = b.x - 8.0 * s;
    }
    let hueco = (fin - tx).max(10.0);
    let recorte = RectF {
        x: tx,
        y,
        ancho: hueco,
        alto: caja.alto,
    };
    e.fichas_pintadas.clear();
    if e.campo.texto.is_empty() {
        // La pista, 3 px a la derecha del cursor: pegada a el parecia que
        // el cursor era su primera letra.
        let pista = match (&e.corrigiendo, &e.apuntar_en) {
            (Some(_), _) => textos.t("tareas5-corregir-pista"),
            (None, Some(l)) => {
                let mut a = fluent_bundle::FluentArgs::new();
                a.set("lista", l.titulo.clone());
                textos.t_args("tareas-apuntar-en-pista", &a)
            }
            (None, None) => textos.t("tareas5-apuntar-pista"),
        };
        p.texto_linea(&pista, tx + 3.0 * s, ty, tam, hueco - 3.0 * s, GRIS);
        if foco {
            p.linea((tx, ty), (tx, ty + alto_linea), 1.5 * s, ACENTO);
        }
    } else {
        pintar_texto_de_la_caja(e, p, tx, ty, alto_linea, hueco, recorte, foco, s);
    }
    if sobre_imagen {
        let limite = RectF {
            x: 0.0,
            y: 0.0,
            ancho: x + ancho + MARGEN * s,
            alto: 100_000.0,
        };
        crate::v2::pista::pintar(p, imagen, &textos.t("tareas5-imagen-pista"), limite, s);
    }
}

/// Lo escrito en la caja: el texto y cada ficha de imagen como una chapa, y
/// el cursor.
#[allow(clippy::too_many_arguments)] // estado, pintor, sitio, renglon, hueco, recorte, foco y escala
fn pintar_texto_de_la_caja(
    e: &mut Estado,
    p: &Pintor,
    tx: f32,
    ty: f32,
    alto_linea: f32,
    hueco: f32,
    recorte: RectF,
    foco: bool,
    s: f32,
) {
    let tam = 15.0 * s;
    // Donde va el cursor: una marca de ancho cero en su sitio, y la caja que
    // le da DirectWrite (medir el trozo de antes se comeria los blancos del
    // final).
    let mut con_marca = e.campo.texto.clone();
    con_marca.insert(e.campo.cursor, '\u{200B}');
    let i = e.campo.texto[..e.campo.cursor].encode_utf16().count() as u32;
    let cx = p
        .cajas_de_trozo(&con_marca, tam, 100_000.0, &[], i, 1)
        .first()
        .map_or(0.0, |b| b.x);
    // Si no cabe, se corre a la izquierda lo justo para ver el cursor.
    let corrido = (cx - (hueco - 4.0 * s)).max(0.0);
    // El texto a trozos: lo escrito en su color y cada ficha de imagen como
    // una chapa. Cada trozo se coloca donde lo pone la disposicion del texto
    // entero, asi no se mueve nada respecto a cuando era un solo texto.
    let texto = &e.campo.texto;
    let fichas: Vec<(std::ops::Range<usize>, PathBuf)> = e
        .campo
        .fichas()
        .into_iter()
        .map(|(r, _, peg)| (r, peg.ruta.clone()))
        .collect();
    let caja_de = |p: &Pintor, r: &std::ops::Range<usize>| {
        let inicio = texto[..r.start].encode_utf16().count() as u32;
        let largo = texto[r.clone()].encode_utf16().count() as u32;
        p.cajas_de_trozo(texto, tam, 100_000.0, &[], inicio, largo)
            .first()
            .copied()
    };
    let mut trozos: Vec<(std::ops::Range<usize>, Option<PathBuf>)> = Vec::new();
    let mut desde = 0;
    for (r, ruta) in fichas {
        trozos.push((desde..r.start, None));
        desde = r.end;
        trozos.push((r, Some(ruta)));
    }
    trozos.push((desde..texto.len(), None));
    let mut pintadas = Vec::new();
    p.con_recorte(recorte, |p| {
        for (r, ruta) in &trozos {
            if texto[r.clone()].trim().is_empty() {
                continue;
            }
            let Some(b) = caja_de(p, r) else {
                continue;
            };
            let x0 = tx - corrido + b.x;
            match ruta {
                None => p.texto(&texto[r.clone()], x0, ty, tam, TEXTO),
                Some(ruta) => {
                    let chapa = RectF {
                        x: x0 - 3.0 * s,
                        y: ty - 2.0 * s,
                        ancho: b.ancho + 6.0 * s,
                        alto: alto_linea + 4.0 * s,
                    };
                    p.rellenar_redondeado(chapa, crate::v2::RADIO_CHAPA * s, FICHA_FONDO);
                    p.texto(&texto[r.clone()], x0, ty, tam, FICHA_LETRA);
                    pintadas.push((chapa, ruta.clone()));
                }
            }
        }
        if foco {
            let c = tx - corrido + cx;
            p.linea((c, ty), (c, ty + alto_linea), 1.5 * s, ACENTO);
        }
    });
    e.fichas_pintadas = pintadas;
}

/// La imagen de la ficha que tiene el raton encima, en un recuadro debajo
/// de ella, entera y con su proporcion.
fn pintar_vista_de_ficha(e: &Estado, p: &Pintor, w: f32, s: f32) {
    let Some((chapa, ruta)) = e
        .fichas_pintadas
        .iter()
        .find(|(r, _)| dentro(*r, e.botones.raton))
    else {
        return;
    };
    let Some((b, iw, ih)) = e.minis.ya(ruta) else {
        return;
    };
    let lado = VISTA * s;
    let f = (lado / iw.max(1) as f32).min(lado / ih.max(1) as f32);
    let (dw, dh) = (iw as f32 * f, ih as f32 * f);
    let pad = 6.0 * s;
    let x = chapa.x.min(w - dw - 2.0 * pad - 8.0 * s).max(8.0 * s);
    let marco = RectF {
        x,
        y: chapa.y + chapa.alto + 8.0 * s,
        ancho: dw + 2.0 * pad,
        alto: dh + 2.0 * pad,
    };
    p.rellenar_redondeado(marco, crate::v2::RADIO_BOTON * s, crate::v2::CAJA);
    p.bitmap_con(
        b,
        RectF {
            x: marco.x + pad,
            y: marco.y + pad,
            ancho: dw,
            alto: dh,
        },
        None,
        pixpin_render::Interpolacion::Lineal,
    );
}

/// La eleccion del grupo adonde mover una tarea del Inbox: un velo que la
/// cierra al pulsarlo y, encima, una tarjeta flotante con un renglon por
/// grupo (su titulo y su chat). Las flechas eligen e Intro mueve.
fn pintar_eleccion(e: &mut Estado, p: &Pintor, w: f32, h: f32, s: f32, textos: &Catalogo) {
    let Some((li, fi)) = e.repartiendo else {
        return;
    };
    let tarea = e
        .fila(li, fi)
        .map(|(_, f)| f.texto.clone())
        .unwrap_or_default();
    let todo = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: h,
    };
    p.rellenar(todo, negro(0.55));
    e.botones.zona(todo, Accion::SoltarMenu);
    let renglon = 48.0 * s;
    let pad = 16.0 * s;
    let ancho = (w - 2.0 * MARGEN * s).min(460.0 * s);
    let cabeza = 52.0 * s;
    let pie = 40.0 * s;
    let cuantos = e.destinos.len().max(1);
    let alto = (cabeza + cuantos as f32 * renglon + pie).min(h - 80.0 * s);
    let caja = RectF {
        x: (w - ancho) / 2.0,
        y: ((h - alto) / 2.0).max(40.0 * s),
        ancho,
        alto,
    };
    // Lo flotante del v2: radio 14 y un canto de blanco al 10 %.
    let radio = crate::v2::RADIO_FLOTANTE * s;
    p.rellenar_redondeado(encoger(caja, -1.0 * s), radio + 1.0 * s, blanco(0.1));
    p.rellenar_redondeado(caja, radio, crate::v2::CAJA);
    // La tarjeta se traga sus clics: fuera de un renglon no cierra.
    e.botones.zona(caja, Accion::Fondo);
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("tarea", tarea);
    p.texto_linea(
        &textos.t_args("tareas-mover-titulo", &args),
        caja.x + pad,
        caja.y + 16.0 * s,
        15.0 * s,
        caja.ancho - 2.0 * pad,
        TEXTO,
    );
    // Abajo, los atajos: «↑↓ Intro» elige, «Esc» cierra.
    let yc = caja.y + caja.alto - pie / 2.0;
    let mut xx = caja.x + pad;
    for (t, rotulo) in [
        ("↑↓", None),
        ("Intro", Some(textos.t("tareas3-mover-intro"))),
        ("Esc", Some(textos.t("tareas3-mover-esc"))),
    ] {
        xx += ui::chapa(p, t, xx, yc, crate::v2::CUERPO, hex(0x2E2E31), s) + 6.0 * s;
        if let Some(r) = rotulo {
            let (rw, rh) = p.medir_texto(&r, 12.5 * s);
            p.texto(&r, xx, yc - rh / 2.0, 12.5 * s, GRIS);
            xx += rw + 14.0 * s;
        }
    }
    if e.destinos.is_empty() {
        p.texto_ajustado(
            &textos.t("tareas-sin-grupos"),
            caja.x + pad,
            caja.y + cabeza,
            13.5 * s,
            caja.ancho - 2.0 * pad,
            GRIS,
        );
        return;
    }
    let vista = RectF {
        y: caja.y + cabeza,
        alto: caja.alto - cabeza - pie,
        ..caja
    };
    let elegido = e.destino_elegido.min(e.destinos.len() - 1);
    // Lo elegido con las flechas siempre a la vista.
    let corrido = ((elegido + 1) as f32 * renglon - vista.alto).max(0.0);
    p.con_recorte(vista, |p| {
        for (di, d) in e.destinos.iter().enumerate() {
            let r = RectF {
                x: caja.x + 6.0 * s,
                y: vista.y + di as f32 * renglon - corrido,
                ancho: caja.ancho - 12.0 * s,
                alto: renglon,
            };
            if r.y > vista.y + vista.alto || r.y + r.alto < vista.y {
                continue;
            }
            if di == elegido {
                p.rellenar_redondeado(r, 8.0 * s, con_alfa(ACENTO, 0.22));
            } else if dentro(r, e.botones.raton) {
                p.rellenar_redondeado(r, 8.0 * s, blanco(0.06));
            }
            p.texto_linea(
                &d.titulo,
                r.x + pad - 6.0 * s,
                r.y + 6.0 * s,
                14.5 * s,
                r.ancho - 2.0 * pad,
                TEXTO,
            );
            p.texto_linea(
                &d.chat,
                r.x + pad - 6.0 * s,
                r.y + 26.0 * s,
                12.0 * s,
                r.ancho - 2.0 * pad,
                GRIS,
            );
        }
    });
    for di in 0..e.destinos.len() {
        let r = RectF {
            x: caja.x + 6.0 * s,
            y: vista.y + di as f32 * renglon - corrido,
            ancho: caja.ancho - 12.0 * s,
            alto: renglon,
        };
        if r.y < vista.y - 1.0 || r.y + r.alto > vista.y + vista.alto + 1.0 {
            continue;
        }
        e.botones.zona(r, Accion::Destino(di));
    }
}

/// Buscando sin nada que coincida: el estado vacio comun con lo que se
/// busco, como buscar mejor y un boton para vaciar la busqueda (o Esc).
fn pintar_sin_resultados(
    e: &mut Estado,
    p: &Pintor,
    x: f32,
    y: f32,
    ancho: f32,
    s: f32,
    textos: &Catalogo,
) {
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("busqueda", ui::corto(e.buscar.texto.trim(), 40));
    let titulo = textos.t_args("tareas3-sin-resultados", &args);
    let zona = RectF {
        x,
        y,
        ancho,
        alto: 220.0 * s,
    };
    crate::v2::vacio(p, zona, &mi::SEARCH, &titulo, s);
    let mut y = zona.y + zona.alto;
    let anch = ancho.min(460.0 * s);
    let pista = textos.t("tareas3-sin-resultados-pista");
    let tam = crate::v2::LETRA_SECUNDARIO * s;
    let (pw, ph) = p.medir_texto_ajustado(&pista, tam, anch);
    p.texto_ajustado(&pista, x + (ancho - pw) / 2.0, y, tam, anch, GRIS);
    y += ph + 18.0 * s;
    let rotulo = textos.t("tareas3-vaciar-busqueda");
    let bw = ui::ancho_de_boton(p, false, &rotulo, Some("Esc"), s);
    let b = RectF {
        x: x + (ancho - bw) / 2.0,
        y,
        ancho: bw,
        alto: crate::v2::BOTON * s,
    };
    ui::boton_v2(
        p,
        &mut e.botones,
        b,
        Accion::VaciarBusqueda,
        None,
        &rotulo,
        Some("Esc"),
        Some(crate::v2::CAJA),
        TEXTO,
        s,
    );
}

/// El encabezado de un grupo: el nombre de la lista en negrita, «· su
/// chat» en gris (no en el Inbox: que es de «Mensajes guardados» ya se
/// sabe) y, pegada al borde derecho, cuantas quedan pendientes. Con el raton
/// encima, la papelera ocupa el sitio de la cuenta: borra la lista entera
/// (pregunta antes).
#[allow(clippy::too_many_arguments)] // estado, pintor, grupo, sitio, escala y textos
fn pintar_encabezado(
    e: &mut Estado,
    p: &Pintor,
    g: &tarjetas::Grupo,
    x: f32,
    y: f32,
    ancho: f32,
    s: f32,
    textos: &Catalogo,
) {
    let Some(l) = e.listas.get(g.lista).cloned() else {
        return;
    };
    let tam = crate::v2::LETRA_TITULO_TARJETA * s;
    let (_, th) = p.medir_texto("Ag", tam);
    let ty = y + ENCABEZADO * s - th - 8.0 * s;
    let papelera = RectF {
        x: x + ancho - ASPA * s,
        y: ty + th / 2.0 - ASPA * s / 2.0,
        ancho: ASPA * s,
        alto: ASPA * s,
    };
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("n", g.pendientes(&l) as i64);
    let cuenta = textos.t_args("tareas3-pendientes", &args);
    let tam_c = crate::v2::LETRA_SECUNDARIO * s;
    let (cw, ch) = p.medir_texto(&cuenta, tam_c);
    if e.encima == Some(tarjetas::Pieza::Encabezado(g.lista)) {
        crate::v2::boton_icono(
            p,
            &mut e.botones,
            papelera,
            Accion::BorrarLista(g.lista),
            &mi::DELETE,
            false,
            false,
            s,
        );
        crate::v2::boton_icono(
            p,
            &mut e.botones,
            RectF {
                x: papelera.x - ASPA * s - 4.0 * s,
                ..papelera
            },
            Accion::CompartirLista(g.lista),
            &mi::IOS_SHARE,
            false,
            false,
            s,
        );
    } else {
        p.texto(&cuenta, x + ancho - cw, ty + (th - ch) / 2.0, tam_c, GRIS);
    }
    // Lo que queda para el nombre: lo mismo pase o no el raton, para que no
    // se recorte de otra forma al pasar.
    let derecha = cw.max(2.0 * ASPA * s + 4.0 * s);
    let libre = (ancho - derecha - 34.0 * s).max(0.0);
    let con_chat = !super::es_inbox(&l);
    let (nw, _) = ui::medir_negrita(p, &l.titulo, tam, libre);
    let nw = if con_chat { nw.min(libre * 0.7) } else { nw.min(libre) };
    // Su color, el de sus tarjetas: una marca redonda delante del nombre.
    let (color_grupo, _) = degradado_de_lista(e, g.lista);
    let marca = 6.0 * s;
    p.rellenar_redondeado(
        RectF {
            x: x + 4.0 * s,
            y: ty + th / 2.0 - th * 0.38,
            ancho: marca,
            alto: th * 0.76,
        },
        marca / 2.0,
        mezcla(color_grupo, Color::BLANCO, 0.25),
    );
    let x = x + marca + 8.0 * s;
    p.con_recorte(
        RectF {
            x,
            y,
            ancho: nw + 6.0 * s,
            alto: ENCABEZADO * s,
        },
        |p| ui::negrita(p, &l.titulo, x + 4.0 * s, ty, tam, 100_000.0, TEXTO),
    );
    if con_chat {
        p.texto_linea(
            &format!("·  {}", l.chat),
            x + 4.0 * s + nw + 8.0 * s,
            ty + 1.0 * s,
            13.5 * s,
            (libre - nw - 12.0 * s).max(0.0),
            GRIS,
        );
    }
}

/// «Hechas (N)» con su flecha: abre y cierra lo hecho de la lista. Buscando
/// esta abierto siempre (lo encontrado se ve), y es solo un rotulo.
#[allow(clippy::too_many_arguments)] // estado, pintor, lista, cuantas, sitio, escala, si se busca y textos
fn pintar_pliegue(
    e: &mut Estado,
    p: &Pintor,
    li: usize,
    n: usize,
    x: f32,
    y: f32,
    ancho: f32,
    s: f32,
    buscando: bool,
    textos: &Catalogo,
) {
    let r = RectF {
        x,
        y,
        ancho,
        alto: PLIEGUE * s,
    };
    let abierta = buscando
        || e.listas
            .get(li)
            .is_some_and(|l| e.abiertas.contains(&l.clave()));
    if !buscando && e.encima == Some(tarjetas::Pieza::Pliegue(li)) {
        p.rellenar_redondeado(r, crate::v2::RADIO_BOTON * s, blanco(0.05));
    }
    let mut tx = x + 8.0 * s;
    if !buscando {
        let flecha = if abierta {
            &mi::KEYBOARD_ARROW_UP
        } else {
            &mi::KEYBOARD_ARROW_DOWN
        };
        p.icono(
            flecha,
            RectF {
                x: tx,
                y: y + (r.alto - 22.0 * s) / 2.0,
                ancho: 22.0 * s,
                alto: 22.0 * s,
            },
            GRIS,
        );
        tx += 28.0 * s;
        e.botones.zona(r, Accion::Plegar(li));
    }
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("n", n as i64);
    let rotulo = textos.t_args("tareas-hechas", &args);
    let (_, rh) = p.medir_texto(&rotulo, 13.5 * s);
    p.texto(&rotulo, tx, y + (r.alto - rh) / 2.0, 13.5 * s, GRIS);
}

/// Lo que ocupa una tarjeta y donde va cada cosa dentro (desde su esquina).
/// La misma cuenta mide (para colocar) y pinta.
struct Hechura {
    alto: f32,
    /// Donde empieza el texto, desde el borde izquierdo de la tarjeta.
    texto_x: f32,
    texto_ancho: f32,
    /// Donde van las miniaturas, desde arriba; `None` si no tiene.
    minis_y: Option<f32>,
    /// Donde va la chapita de la lista y el chat (buscando).
    chapa_y: Option<f32>,
    /// Lo que se lee: el texto sin sus emoticonos, que van aparte.
    texto: String,
    /// Los emoticonos de la tarea: sus etiquetas, en circulos entre la
    /// casilla y el texto, como los estados de WeChat.
    emos: Vec<String>,
}

/// El rotulo de la edad de una tarea («hace 2 días»), si tiene fecha.
fn edad(f: &Fila, hoy: pixpin_proyecto::mini::Fecha, textos: &Catalogo) -> Option<String> {
    let creada = f.creada?;
    let mut args = fluent_bundle::FluentArgs::new();
    args.set(
        "dias",
        i64::from(pixpin_proyecto::mini::dias_desde(creada, hoy)),
    );
    Some(textos.t_args("mini-tarea-edad", &args))
}

#[allow(clippy::too_many_arguments)] // estado, pintor, tarea, ancho, escala, si se busca y textos
fn hechura(
    e: &Estado,
    p: &Pintor,
    li: usize,
    fi: usize,
    ancho: f32,
    s: f32,
    buscando: bool,
    textos: &Catalogo,
) -> Hechura {
    let Some((_, f)) = e.fila(li, fi) else {
        return Hechura {
            alto: 0.0,
            texto_x: 0.0,
            texto_ancho: 0.0,
            minis_y: None,
            chapa_y: None,
            texto: String::new(),
            emos: Vec::new(),
        };
    };
    let pad = 16.0 * s;
    let (texto, emos) = emoticonos::emoticonos_de(&f.texto);
    let estados = ancho_de_estados(p, &emos, ESTADO_LISTA * s, s);
    let texto_x = 8.0 * s + CASILLA * s + 10.0 * s + estados;
    // A la derecha va la edad o, con el raton encima, quitar (y «Mover a…»
    // en el Inbox): se reserva siempre el sitio del mas ancho, para que el
    // texto no salte de renglon al pasar el raton.
    let mut reserva = edad(f, e.hoy, textos)
        .map_or(0.0, |t| p.medir_texto(&t, 12.0 * s).0)
        .max(ASPA * s);
    if e.se_reparte(li, fi) {
        let b = ui::ancho_de_boton(p, true, &textos.t("tareas-mover"), Some("Intro"), s);
        reserva = reserva.max(b + ASPA * s + 4.0 * s);
    }
    if reserva > 0.0 {
        reserva += 12.0 * s;
    }
    let texto_ancho = (ancho - texto_x - pad - reserva).max(40.0 * s);
    let texto_alto = if texto.is_empty() {
        0.0
    } else {
        p.medir_texto_ajustado(&texto, TAM_TAREA * s, texto_ancho).1
    };
    let mut y = 18.0 * s + texto_alto;
    let minis_y = (!f.imagenes.is_empty()).then(|| {
        let en = if texto_alto > 0.0 {
            y + 10.0 * s
        } else {
            12.0 * s
        };
        y = en + MINI * s;
        en
    });
    let chapa_y = buscando.then(|| {
        let en = y + 10.0 * s;
        y = en + 22.0 * s;
        en
    });
    Hechura {
        alto: (y + 16.0 * s).max(TARJETA_MIN * s),
        texto_x,
        texto_ancho,
        minis_y,
        chapa_y,
        texto,
        emos,
    }
}

/// Lo que ocupa a lo ancho la pila de circulos de `emos` (con su «+N» y el
/// hueco hasta lo siguiente). Sin emoticonos, nada.
fn ancho_de_estados(p: &Pintor, emos: &[String], d: f32, s: f32) -> f32 {
    if emos.is_empty() {
        return 0.0;
    }
    let (_, mas) = emoticonos::pila_de_estados(0.0, d, emos.len());
    let extra = if mas > 0 {
        4.0 * s + ancho_de_mas(p, mas, s)
    } else {
        0.0
    };
    emoticonos::ancho_de_pila(d, emos.len()) + extra + 10.0 * s
}

/// La chapita «+N» de los emoticonos que no se ven.
fn ancho_de_mas(p: &Pintor, n: usize, s: f32) -> f32 {
    p.medir_texto(&format!("+{n}"), 12.5 * s).0 + 12.0 * s
}

// ------------------------------------------- piezas comunes de las tarjetas
//
// La tarjeta de la lista, el cuadrado y la fila de una hecha llevan las
// mismas cosas (el texto con lo buscado y el tachado, la casilla, los
// emoticonos, quitar): una sola funcion para cada una, asi no se separan.

/// Como se pinta el texto de una tarea.
#[derive(Clone, Copy)]
struct Letra {
    tam: f32,
    negrita: bool,
}

impl Letra {
    /// Los tramos de `texto` con esta letra: ninguno en la normal (la de
    /// `texto_ajustado`), uno entero en negrita.
    fn tramos(self, texto: &str) -> Vec<pixpin_render::Tramo> {
        if !self.negrita {
            return Vec::new();
        }
        vec![pixpin_render::Tramo {
            inicio: 0,
            longitud: texto.encode_utf16().count() as u32,
            estilo: pixpin_render::EstiloTexto {
                negrita: true,
                ..Default::default()
            },
        }]
    }

    /// Lo que ocupa `texto` partido a `ancho`, como lo pinta
    /// [`pintar_texto_tarea`].
    fn medir(self, p: &Pintor, texto: &str, ancho: f32) -> (f32, f32) {
        if self.negrita {
            p.medir_parrafo(texto, self.tam, ancho, &self.tramos(texto))
        } else {
            p.medir_texto_ajustado(texto, self.tam, ancho)
        }
    }
}

/// **El texto de una tarea** en `(x, y)`, partido a `ancho`: lo buscado con
/// un fondo amarillo debajo (en las mismas cajas que pinta DirectWrite), con
/// `sombra` una copia negra 1 px mas abajo (sobre una foto, para que el
/// blanco no se pierda en lo claro), y lo hecho tachado renglon a renglon,
/// como en el movil.
#[allow(clippy::too_many_arguments)] // pintor, texto, sitio, letra, palabras, color, estados y escala
fn pintar_texto_tarea(
    p: &Pintor,
    texto: &str,
    x: f32,
    y: f32,
    ancho: f32,
    letra: Letra,
    palabras: &[String],
    color: Color,
    hecha: bool,
    sombra: bool,
    s: f32,
) {
    if texto.is_empty() {
        return;
    }
    let tramos = letra.tramos(texto);
    let cajas = |inicio: u32, largo: u32| {
        p.cajas_de_trozo(texto, letra.tam, ancho, &tramos, inicio, largo)
    };
    for palabra in palabras {
        for t in pixpin_ui::resaltado::coincidencias(texto, palabra) {
            let (inicio, largo) = t.en_utf16(texto);
            if largo == 0 {
                continue;
            }
            for b in cajas(inicio, largo) {
                p.rellenar_redondeado(
                    RectF {
                        x: x + b.x - 1.0 * s,
                        y: y + b.y,
                        ancho: b.ancho + 2.0 * s,
                        alto: b.alto,
                    },
                    3.0 * s,
                    RESALTE,
                );
            }
        }
    }
    let pintar = |x: f32, y: f32, c: Color| {
        if letra.negrita {
            p.parrafo(texto, x, y, letra.tam, ancho, &tramos, c);
        } else {
            p.texto_ajustado(texto, x, y, letra.tam, ancho, c);
        }
    };
    if sombra {
        pintar(x, y + 1.0 * s, negro(0.7));
    }
    pintar(x, y, color);
    if hecha {
        let largo = texto.encode_utf16().count() as u32;
        for b in cajas(0, largo) {
            let ly = y + b.y + b.alto * 0.55;
            p.linea((x + b.x, ly), (x + b.x + b.ancho, ly), 1.2 * s, color);
        }
    }
}

/// **La casilla** de la tarea `(li, fi)` en `objetivo` (40 px, el dibujo de
/// 24 en medio): lo unico que la tacha. Sobre una foto lleva un circulo
/// oscuro detras para que se vea. `apagada`: la de una fila hecha, a media
/// opacidad como el resto de la fila.
#[allow(clippy::too_many_arguments)] // estado, pintor, sitio, tarea, estados y escala
fn pintar_casilla(
    e: &mut Estado,
    p: &Pintor,
    objetivo: RectF,
    (li, fi): (usize, usize),
    hecha: bool,
    sobre_foto: bool,
    apagada: bool,
    s: f32,
) {
    let centro = (
        objetivo.x + objetivo.ancho / 2.0,
        objetivo.y + objetivo.alto / 2.0,
    );
    if sobre_foto {
        p.circulo(centro, 15.0 * s, negro(0.45));
    }
    if dentro(objetivo, e.botones.raton) {
        p.circulo(centro, 18.0 * s, blanco(0.12));
    }
    let icono = if hecha {
        &mi::CHECK_BOX
    } else {
        &mi::CHECK_BOX_OUTLINE_BLANK
    };
    let tinta = match (hecha, sobre_foto) {
        (true, _) => ACENTO,
        (false, true) => Color::BLANCO,
        (false, false) => hex(0xAEAEB2),
    };
    let tinta = if apagada { con_alfa(tinta, 0.5) } else { tinta };
    p.icono(icono, encoger(objetivo, (CASILLA - 24.0) / 2.0 * s), tinta);
    e.botones.zona(objetivo, Accion::Marcar(li, fi));
}

/// **Los emoticonos como los estados de WeChat**: circulos de diametro `d`
/// con el emoticono dentro sobre su tono pastel (el mismo siempre para el
/// mismo emoticono), el primero delante a la izquierda y los siguientes
/// asomando detras, corridos a la derecha; a lo sumo tres, y el resto en
/// «+N». Cada circulo lleva un aro del color `aro` (lo que hay detras, o
/// blanco sobre una foto) que lo separa del de debajo. Cada circulo es un
/// boton: buscar su emoticono.
///
/// POR QUE el emoticono va encima limpio: antes se apagaba el de detras con
/// un velo negro pintado despues del emoticono, y el velo lo dejaba gris;
/// ahora solo se oscurece el fondo del circulo y el emoticono sale con sus
/// colores, al 60 % del diametro.
#[allow(clippy::too_many_arguments)] // estado, pintor, tarea, emoticonos, sitio, tamano, aro y escala
fn pintar_estados(
    e: &mut Estado,
    p: &Pintor,
    (li, fi): (usize, usize),
    emos: &[String],
    x: f32,
    cy: f32,
    d: f32,
    aro: Color,
    sobre_foto: bool,
    s: f32,
) {
    let (circulos, mas) = emoticonos::pila_de_estados(x, d, emos.len());
    // De atras adelante: el de delante tapa a los demas (y su zona, que se
    // apunta despues, gana).
    for (k, &(cx, sombra)) in circulos.iter().enumerate().rev() {
        let tono = mezcla(hex(emoticonos::tono_de(&emos[k])), Color::NEGRO, sombra);
        p.circulo((cx, cy), d / 2.0 + 2.0 * s, aro);
        p.circulo((cx, cy), d / 2.0, tono);
        let tam = d * 0.6;
        let (ew, eh) = p.medir_texto(&emos[k], tam);
        p.texto_color(&emos[k], cx - ew / 2.0, cy - eh / 2.0, tam, Color::BLANCO);
        e.botones
            .zona(crate::v2::geom::centrado((cx, cy), d), Accion::BuscarEmo(li, fi, k));
    }
    if mas > 0 {
        let t = format!("+{mas}");
        let tam = 12.5 * s;
        let (tw, th) = p.medir_texto(&t, tam);
        let cw = ancho_de_mas(p, mas, s);
        let chapa = RectF {
            x: x + emoticonos::ancho_de_pila(d, emos.len()) + 4.0 * s,
            y: cy - 11.0 * s,
            ancho: cw,
            alto: 22.0 * s,
        };
        let (fondo, letra) = if sobre_foto {
            (negro(0.55), Color::BLANCO)
        } else {
            (blanco(0.1), crate::v2::CUERPO)
        };
        p.rellenar_redondeado(chapa, 11.0 * s, fondo);
        p.texto(&t, chapa.x + (cw - tw) / 2.0, cy - th / 2.0, tam, letra);
    }
}

/// El boton de **quitar** la tarea (la papelera del v2, como en la
/// galeria): al momento, con «Deshacer» en el aviso.
fn boton_quitar(
    e: &mut Estado,
    p: &Pintor,
    caja: RectF,
    (li, fi): (usize, usize),
    sobre_foto: bool,
    s: f32,
) {
    crate::v2::boton_icono(
        p,
        &mut e.botones,
        caja,
        Accion::Quitar(li, fi),
        &mi::DELETE,
        false,
        sobre_foto,
        s,
    );
}

// ----------------------------------------------------------- las tarjetas

/// Una tarea en su tarjeta: la casilla grande (lo unico que tacha), el
/// texto entero en los renglones que haga falta (lo buscado resaltado), sus
/// imagenes (un clic las pinea), buscando la chapita de su lista y su chat,
/// y arriba a la derecha cuantos dias lleva o, con el raton encima o el
/// foco, quitar y «Mover a…» si es del Inbox.
#[allow(clippy::too_many_arguments)] // estado, pintor, tarea, sitio, escala, palabras y textos
fn pintar_tarjeta(
    e: &mut Estado,
    p: &Pintor,
    li: usize,
    fi: usize,
    r: RectF,
    s: f32,
    palabras: &[String],
    textos: &Catalogo,
) {
    let Some((lista, f)) = e.fila(li, fi).map(|(l, f)| (l.clone(), f.clone())) else {
        return;
    };
    let buscando = !palabras.is_empty();
    let hc = hechura(e, p, li, fi, r.ancho, s, buscando, textos);
    let encima = e.encima == Some(tarjetas::Pieza::Tarjeta(li, fi));
    let foco = e.foco == Foco::Tarjeta(li, fi);
    crate::v2::tarjeta::fondo(p, r, encima, foco, s);
    let mut fondo = if encima {
        crate::v2::TARJETA_ENCIMA
    } else {
        crate::v2::TARJETA
    };
    if f.hecha {
        // Lo hecho, hundido: mas oscuro que lo pendiente.
        fondo = crate::v2::TARJETA_HECHA;
        let g = 1.5 * s;
        p.rellenar_redondeado(encoger(r, g), crate::v2::RADIO_TARJETA * s - g, fondo);
    }
    // La tarjeta entera da el foco; lo de dentro se apunta despues y gana.
    e.botones.zona(r, Accion::Enfocar(li, fi));

    let objetivo = RectF {
        x: r.x + 8.0 * s,
        y: r.y + 8.0 * s,
        ancho: CASILLA * s,
        alto: CASILLA * s,
    };
    pintar_casilla(e, p, objetivo, (li, fi), f.hecha, false, false, s);

    // Sus emoticonos, en circulos entre la casilla y el texto, a la altura
    // de la casilla.
    if !hc.emos.is_empty() {
        let x = objetivo.x + objetivo.ancho + 8.0 * s;
        let cy = objetivo.y + objetivo.alto / 2.0;
        pintar_estados(e, p, (li, fi), &hc.emos, x, cy, ESTADO_LISTA * s, fondo, false, s);
    }

    let tx = r.x + hc.texto_x;
    pintar_texto_tarea(
        p,
        &hc.texto,
        tx,
        r.y + 18.0 * s,
        hc.texto_ancho,
        Letra {
            tam: TAM_TAREA * s,
            negrita: false,
        },
        palabras,
        if f.hecha { GRIS } else { TEXTO },
        f.hecha,
        false,
        s,
    );

    // Arriba a la derecha: la edad o, con el raton o el foco, quitar y, si
    // es del Inbox, «Mover a…» a su lado.
    let derecha = r.x + r.ancho - 16.0 * s;
    let a_mano = (encima || foco) && e.repartiendo.is_none();
    if a_mano {
        let aspa = RectF {
            x: r.x + r.ancho - 8.0 * s - ASPA * s,
            y: r.y + 8.0 * s,
            ancho: ASPA * s,
            alto: ASPA * s,
        };
        boton_quitar(e, p, aspa, (li, fi), false, s);
    }
    if e.se_reparte(li, fi) && a_mano {
        let rotulo = textos.t("tareas-mover");
        let tecla = foco.then_some("Intro");
        let bw = ui::ancho_de_boton(p, true, &rotulo, tecla, s);
        let b = RectF {
            x: r.x + r.ancho - 8.0 * s - ASPA * s - 4.0 * s - bw,
            y: r.y + 8.0 * s,
            ancho: bw,
            alto: crate::v2::BOTON * s,
        };
        ui::boton_v2(
            p,
            &mut e.botones,
            b,
            Accion::Repartir(li, fi),
            Some(&mi::FORWARD),
            &rotulo,
            tecla,
            Some(crate::v2::AZUL_LLENO),
            Color::BLANCO,
            s,
        );
    } else if !a_mano && let Some(t) = edad(&f, e.hoy, textos) {
        let (ew, eh) = p.medir_texto(&t, 12.0 * s);
        p.texto(&t, derecha - ew, r.y + 28.0 * s - eh / 2.0, 12.0 * s, GRIS);
    }

    // Sus imagenes, en una fila debajo del texto: un clic la pinea.
    if let Some(my) = hc.minis_y {
        let mini = MINI * s;
        let mut mx = tx;
        for k in 0..f.imagenes.len() {
            let c = RectF {
                x: mx,
                y: r.y + my,
                ancho: mini,
                alto: mini,
            };
            if c.x + c.ancho > r.x + r.ancho - 16.0 * s {
                break;
            }
            match e
                .ruta_de(&lista, &f, k)
                .as_deref()
                .and_then(|ruta| e.minis.ya(ruta))
            {
                Some((b, iw, ih)) => {
                    crate::miniaturas::pintar_recortado(p, b, c, iw, ih);
                    if f.hecha {
                        // Apagada como el texto tachado.
                        p.rellenar(c, con_alfa(fondo, 0.45));
                    }
                }
                // Aun no llego del movil (o no se pudo leer): su hueco con el
                // dibujo de una imagen, para que se sepa que hay una.
                None => {
                    p.rellenar_redondeado(c, 6.0 * s, blanco(0.06));
                    p.icono(&mi::IMAGE, encoger(c, 12.0 * s), GRIS);
                }
            }
            if dentro(c, e.botones.raton) {
                p.trazar(c, 2.0 * s, ACENTO);
            }
            e.botones.zona(c, Accion::Imagen(li, fi, k));
            mx += mini + 8.0 * s;
        }
    }

    // Buscando se mezclan grupos: de que lista y chat es cada una.
    if let Some(cy) = hc.chapa_y {
        let t = format!("{}  ·  {}", lista.titulo, lista.chat);
        let tam_c = 12.0 * s;
        let max = (r.ancho - hc.texto_x - 16.0 * s - 16.0 * s).max(0.0);
        let (cw, ch) = p.medir_texto(&t, tam_c);
        let cw = cw.min(max);
        let chapa = RectF {
            x: tx,
            y: r.y + cy,
            ancho: cw + 16.0 * s,
            alto: 22.0 * s,
        };
        p.rellenar_redondeado(chapa, 6.0 * s, blanco(0.07));
        p.texto_linea(
            &t,
            chapa.x + 8.0 * s,
            chapa.y + (chapa.alto - ch) / 2.0,
            tam_c,
            cw,
            crate::v2::CUERPO,
        );
    }
}

/// Redondea las esquinas de algo pintado en `r` sin ellas (una foto):
/// pinta, en cada esquina, lo que queda fuera del cuarto de circulo con el
/// color de lo que hay detras. Direct2D no recorta con forma redonda sin
/// capas, y para cuatro esquinas esto basta. Orden de `detras`: arriba
/// izquierda, arriba derecha, abajo derecha, abajo izquierda.
fn esquinas(p: &Pintor, r: RectF, radio: f32, detras: [Color; 4]) {
    const PASOS: usize = 8;
    let centros = [
        (r.x + radio, r.y + radio, std::f32::consts::PI),
        (r.x + r.ancho - radio, r.y + radio, 1.5 * std::f32::consts::PI),
        (r.x + r.ancho - radio, r.y + r.alto - radio, 0.0),
        (r.x + radio, r.y + r.alto - radio, 0.5 * std::f32::consts::PI),
    ];
    for (k, &(cx, cy, desde)) in centros.iter().enumerate() {
        // El cuadrado de la esquina y, dentro, el cuarto de circulo que se
        // respeta (el centro y su arco).
        let marco = RectF {
            x: if k == 0 || k == 3 { r.x } else { cx },
            y: if k < 2 { r.y } else { cy },
            ancho: radio,
            alto: radio,
        };
        let mut quesito = vec![(cx, cy)];
        for i in 0..=PASOS {
            let a = desde + std::f32::consts::FRAC_PI_2 * i as f32 / PASOS as f32;
            quesito.push((cx + radio * a.cos(), cy + radio * a.sin()));
        }
        p.velo(marco, &quesito, detras[k]);
    }
}

/// Donde empieza el velo oscuro de una foto, en fraccion de su alto: por
/// encima, la foto se ve limpia.
const VELO_DESDE: f32 = 0.45;
/// Donde el velo ya es el de en medio (0,65), en fraccion del alto.
const VELO_MEDIO: f32 = 0.72;

/// El velo de un cuadrado con foto, para leer encima el texto en blanco:
/// dos degradados de verdad, de nada a 0,65 y de 0,65 a 0,9 en el borde de
/// abajo, que es donde va el texto. Antes eran 16 capas apiladas al 10 %
/// desde el 36 % del alto: claro donde estaba la primera linea y con un
/// escalon visible en cada capa. Y uno suave arriba, para los emoticonos y
/// los botones.
fn velo_de_foto(p: &Pintor, r: RectF) {
    let y0 = r.y + r.alto * VELO_DESDE;
    let y1 = r.y + r.alto * VELO_MEDIO;
    let y2 = r.y + r.alto;
    p.rect_degradado(
        RectF {
            x: r.x,
            y: y0,
            ancho: r.ancho,
            alto: y1 - y0,
        },
        (r.x, y0),
        (r.x, y1),
        negro(0.0),
        negro(0.65),
    );
    p.rect_degradado(
        RectF {
            x: r.x,
            y: y1,
            ancho: r.ancho,
            alto: y2 - y1,
        },
        (r.x, y1),
        (r.x, y2),
        negro(0.65),
        negro(0.9),
    );
    let arriba = r.alto * 0.3;
    p.rect_degradado(
        RectF { alto: arriba, ..r },
        (r.x, r.y),
        (r.x, r.y + arriba),
        negro(0.35),
        negro(0.0),
    );
}

/// Los degradados de los grupos: doce tonos bien distintos entre si
/// (violeta, verde, rojo, naranja, azul, rosa, turquesa, oliva, marron,
/// anil, magenta y dorado), hondos para que el texto claro se lea encima.
const DEGRADADOS: [(u32, u32); 12] = [
    (0x5B2DA8, 0x3A1F7A),
    (0x0F7A4A, 0x0B5236),
    (0xA8323E, 0x6E1F33),
    (0xB8650F, 0x7A3D0C),
    (0x1767A8, 0x0F3F75),
    (0xA8306E, 0x6B1F52),
    (0x0E7C86, 0x0A4F5E),
    (0x6E7A12, 0x434D0C),
    (0x7A4A2A, 0x4D2C1A),
    (0x3A44B8, 0x232A7A),
    (0x8A2AA8, 0x561A70),
    (0x9C7A0E, 0x5E4808),
];

/// **El color de cada lista** (su sitio en [`DEGRADADOS`]), sin repetir
/// mientras haya colores (10-oct-2026, el usuario: «que cada grupo de tareas
/// tenga un color diferente, no el mismo color»). Antes salia solo de la
/// clave y dos listas podian caer en el mismo. Cada lista parte del color de
/// su clave y, si ya lo tiene otra, toma el siguiente libre; se reparten
/// por orden de clave (no por el de la ventana, que cambia al tachar), asi
/// una lista no cambia de color mientras no se creen o borren otras. Las
/// listas vacias no gastan color.
fn colores_de_listas(claves: &[(String, bool)]) -> Vec<usize> {
    let n = DEGRADADOS.len();
    // FNV-1a: estable entre versiones, a diferencia de `DefaultHasher`.
    let semilla = |c: &str| c.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193)) as usize % n;
    let mut orden: Vec<usize> = (0..claves.len()).filter(|&i| claves[i].1).collect();
    orden.sort_by(|a, b| claves[*a].0.cmp(&claves[*b].0));
    let mut usado = vec![false; n];
    let mut out: Vec<usize> = claves.iter().map(|(c, _)| semilla(c)).collect();
    for i in orden {
        let desde = semilla(&claves[i].0);
        let k = (0..n).map(|d| (desde + d) % n).find(|k| !usado[*k]).unwrap_or(desde);
        usado[k] = true;
        out[i] = k;
    }
    out
}

/// **El degradado de la lista `li`** de la ventana (ver [`colores_de_listas`]).
fn degradado_de_lista(e: &Estado, li: usize) -> (Color, Color) {
    let claves: Vec<(String, bool)> = e.listas.iter().map(|l| (l.clave(), !l.filas.is_empty())).collect();
    let k = colores_de_listas(&claves).get(li).copied().unwrap_or(0);
    let (a, b) = DEGRADADOS[k % DEGRADADOS.len()];
    (hex(a), hex(b))
}

/// **Lo que mide de alto la tarjeta** de una tarea en la vista de tarjetas:
/// con foto, el cuadrado entero (la foto es el fondo); sin foto, lo que
/// pide su texto (hasta cinco renglones) con los emoticonos de arriba y la
/// casilla de abajo. `rejilla::disponer` lo deja entre medio cuadrado y el
/// cuadrado.
fn alto_de_cuadrado(e: &Estado, p: &Pintor, li: usize, fi: usize, lado: f32, s: f32) -> f32 {
    let Some((_, f)) = e.fila(li, fi) else {
        return lado;
    };
    let (texto, emos) = emoticonos::emoticonos_de(&f.texto);
    // Una fila de las hechas (`lado` es entonces todo el ancho): su texto
    // entero entre la casilla y la edad, con su aire arriba y abajo.
    if lado > LADO_CUADRADO * s * 1.5 {
        let letra = Letra {
            tam: LETRA_FILA * s,
            negrita: false,
        };
        let ancho_t = (lado - CASILLA * s - 150.0 * s).max(40.0 * s);
        let t = if texto.is_empty() { emos.join(" ") } else { texto };
        return letra.medir(p, &t, ancho_t).1 + 24.0 * s;
    }
    if !f.imagenes.is_empty() {
        return lado;
    }
    let caja = RectF {
        x: 0.0,
        y: 0.0,
        ancho: lado,
        alto: lado,
    };
    let partes = rejilla::partes(caja, s, !emos.is_empty());
    let letra = Letra {
        tam: LETRA_CUADRO * s,
        negrita: false,
    };
    let (_, texto_alto) = letra.medir(p, &texto, partes.texto.ancho);
    // Lo de encima del texto, el texto, y lo de debajo (la casilla).
    partes.texto.y + texto_alto + (lado - partes.texto.y - partes.texto.alto)
}

/// Una tarea en su cuadrado (la vista en tarjetas). Con foto, la primera
/// llena el cuadrado, con un velo oscuro abajo para leer encima el texto en
/// blanco y negrita; sin foto, el fondo de tarjeta tenido del tono de su
/// primer emoticono y el texto arriba, mas grande. Arriba a la izquierda,
/// sus emoticonos como estados de WeChat; con varias fotos, la pila de
/// detras y «+N» abajo a la derecha (pinea todas). La casilla abajo a la
/// izquierda, y con el raton encima o el foco quitar (y «Mover a…» en el
/// Inbox) arriba a la derecha. Todo sale de [`rejilla::partes`].
///
/// Un clic da el foco, tambien con foto; el doble clic o Intro pinean la
/// foto. Antes el clic pineaba siempre y un cuadrado con foto no se podia
/// ni enfocar.
#[allow(clippy::too_many_arguments)] // estado, pintor, tarea, sitio, escala, palabras y textos
fn pintar_cuadrado(
    e: &mut Estado,
    p: &Pintor,
    li: usize,
    fi: usize,
    r: RectF,
    s: f32,
    palabras: &[String],
    textos: &Catalogo,
) {
    let Some((lista, f)) = e.fila(li, fi).map(|(l, f)| (l.clone(), f.clone())) else {
        return;
    };
    let (texto, emos) = emoticonos::emoticonos_de(&f.texto);
    let partes = rejilla::partes(r, s, !emos.is_empty());
    let encima = e.encima == Some(tarjetas::Pieza::Tarjeta(li, fi));
    let foco = e.foco == Foco::Tarjeta(li, fi);
    let radio = crate::v2::RADIO_TARJETA * s;

    // La pila de detras, la de todas las ventanas v2.
    crate::v2::tarjeta::pila(p, r, f.imagenes.len(), s);
    let hay_pila = crate::v2::tarjeta::cantos_de_pila(f.imagenes.len()) > 0;

    let foto = e
        .ruta_de(&lista, &f, 0)
        .and_then(|ruta| e.minis.ya(&ruta).map(|(b, iw, ih)| (b.clone(), iw, ih)));
    // Sin foto, el degradado de SU LISTA (8-oct-2026, el usuario: «un color
    // unico con degradado, que todo ese grupo de tareas tenga el mismo»):
    // las tareas de una lista se reconocen de lejos.
    let (desde, hasta) = degradado_de_lista(e, li);
    let (desde, hasta) = if encima {
        (mezcla(desde, Color::BLANCO, 0.08), mezcla(hasta, Color::BLANCO, 0.08))
    } else {
        (desde, hasta)
    };
    let fondo = desde;
    if let Some((b, iw, ih)) = &foto {
        if foco {
            p.rellenar_redondeado(encoger(r, -2.0 * s), radio + 2.0 * s, ACENTO);
        }
        crate::miniaturas::pintar_recortado(p, b, r, *iw, *ih);
        velo_de_foto(p, r);
        if f.hecha {
            p.rellenar(r, con_alfa(FONDO, 0.55));
        }
        if encima && !f.hecha {
            p.rellenar(r, blanco(0.05));
        }
        // El canto de la pila asoma por la esquina de arriba a la derecha:
        // ahi lo de detras es el tono de la tarjeta de justo debajo.
        let detras_arriba_dcha = if hay_pila { hex(0x48484E) } else { FONDO };
        let detras = if foco {
            [ACENTO; 4]
        } else {
            [FONDO, detras_arriba_dcha, FONDO, FONDO]
        };
        esquinas(p, r, radio, detras);
    } else {
        crate::v2::tarjeta::fondo(p, r, encima, foco, s);
        let g = if foco { 1.5 * s } else { 1.0 * s };
        let dentro_r = encoger(r, g);
        if p.empujar_recorte_redondeado(dentro_r, radio - g) {
            p.rect_degradado(
                dentro_r,
                (dentro_r.x, dentro_r.y),
                (dentro_r.x + dentro_r.ancho, dentro_r.y + dentro_r.alto),
                desde,
                hasta,
            );
            p.soltar_recorte_redondeado();
        }
        if !f.imagenes.is_empty() {
            // La foto aun no llego (o no se pudo leer): su dibujo en medio.
            p.icono(
                &mi::IMAGE,
                crate::v2::geom::centrado((r.x + r.ancho / 2.0, r.y + r.alto / 2.0), 48.0 * s),
                blanco(0.12),
            );
        }
    }
    let con_foto = foto.is_some();
    // El cuadrado entero da el foco; lo de dentro se apunta despues y gana.
    e.botones.zona(r, Accion::Enfocar(li, fi));

    // Los emoticonos, arriba a la izquierda; sobre una foto, con un aro
    // blanco que los despega de ella.
    if !emos.is_empty() {
        let aro = if con_foto { blanco(0.9) } else { fondo };
        pintar_estados(
            e,
            p,
            (li, fi),
            &emos,
            partes.chapas.x,
            partes.chapas.y + partes.chapas.alto / 2.0,
            rejilla::CHAPA * s,
            aro,
            con_foto,
            s,
        );
    }

    // El texto: con foto, pegado abajo, blanco, en negrita y con sombra,
    // hasta tres renglones (los que caben bajo el comienzo del velo). Sin
    // foto, arriba, grande y ENTERO: la tarjeta crece lo que haga falta
    // (`alto_de_cuadrado`; el usuario, 8-oct-2026: «que no se pierda el
    // texto»).
    let letra = Letra {
        tam: if con_foto { 15.0 * s } else { LETRA_CUADRO * s },
        negrita: con_foto,
    };
    let (_, renglon) = letra.medir(p, "Ag", 1000.0 * s);
    let caben = if con_foto {
        partes.texto.alto.min(renglon * 3.0 + 1.0)
    } else {
        partes.texto.alto + renglon
    };
    let ancho_t = partes.texto.ancho;
    let mostrado =
        rejilla::recortar_a_renglones(&texto, caben, &mut |t| letra.medir(p, t, ancho_t).1);
    if !mostrado.is_empty() {
        let (_, th) = letra.medir(p, &mostrado, ancho_t);
        let ty = if con_foto {
            partes.texto.y + partes.texto.alto - th
        } else {
            partes.texto.y
        };
        let color = match (con_foto, f.hecha) {
            (true, false) => Color::BLANCO,
            (true, true) => blanco(0.6),
            (false, false) => TEXTO,
            (false, true) => GRIS,
        };
        pintar_texto_tarea(
            p,
            &mostrado,
            partes.texto.x,
            ty,
            ancho_t,
            letra,
            palabras,
            color,
            f.hecha,
            con_foto,
            s,
        );
    }

    pintar_casilla(e, p, partes.casilla, (li, fi), f.hecha, con_foto, false, s);

    // Abajo a la derecha: «+N» con varias fotos (pulsarla las pinea todas);
    // si no, la edad.
    let (derecha, cy) = partes.pie;
    if f.imagenes.len() > 1 {
        let t = format!("+{}", f.imagenes.len() - 1);
        let tam_c = 12.5 * s;
        let (tw, th) = p.medir_texto(&t, tam_c);
        let li_ = 14.0 * s;
        let ancho_c = li_ + 4.0 * s + tw + 16.0 * s;
        let chapa = RectF {
            x: derecha - ancho_c,
            y: cy - 12.0 * s,
            ancho: ancho_c,
            alto: 24.0 * s,
        };
        // El objetivo, de 40 de alto aunque la chapa se vea de 24.
        let objetivo = RectF {
            y: cy - crate::v2::OBJETIVO_MINIMO * s / 2.0,
            alto: crate::v2::OBJETIVO_MINIMO * s,
            ..chapa
        };
        let sobre = dentro(objetivo, e.botones.raton);
        p.rellenar_redondeado(chapa, 12.0 * s, negro(if sobre { 0.8 } else { 0.6 }));
        p.icono(
            &mi::PHOTO_LIBRARY,
            RectF {
                x: chapa.x + 8.0 * s,
                y: cy - li_ / 2.0,
                ancho: li_,
                alto: li_,
            },
            Color::BLANCO,
        );
        p.texto(&t, chapa.x + 8.0 * s + li_ + 4.0 * s, cy - th / 2.0, tam_c, Color::BLANCO);
        e.botones.zona(objetivo, Accion::PinearTodas(li, fi));
    } else if let Some(t) = edad(&f, e.hoy, textos) {
        let tam_e = 12.0 * s;
        let (ew, eh) = p.medir_texto(&t, tam_e);
        let color = if con_foto { blanco(0.8) } else { GRIS };
        p.texto(&t, derecha - ew, cy - eh / 2.0, tam_e, color);
    }

    // Con el raton encima o el foco: quitar y, en el Inbox, «Mover a…»
    // (solo icono: en un cuadrado no cabe el rotulo; Intro sigue igual).
    let a_mano = (encima || foco) && e.repartiendo.is_none();
    if a_mano {
        boton_quitar(e, p, partes.aspa, (li, fi), con_foto, s);
        if e.se_reparte(li, fi) {
            crate::v2::boton_icono(
                p,
                &mut e.botones,
                partes.mover,
                Accion::Repartir(li, fi),
                &mi::FORWARD,
                false,
                con_foto,
                s,
            );
        }
    }
}

/// Una tarea **hecha en la vista en cuadrados**: una fila de 44 a media
/// opacidad, no un cuadrado entero (ver `rejilla::disponer`). La casilla
/// (para desmarcarla), el texto en un renglon y tachado, y a la derecha la
/// edad o, con el raton encima o el foco, quitar.
#[allow(clippy::too_many_arguments)] // estado, pintor, tarea, sitio, escala, palabras y textos
fn pintar_fila_hecha(
    e: &mut Estado,
    p: &Pintor,
    li: usize,
    fi: usize,
    r: RectF,
    s: f32,
    palabras: &[String],
    textos: &Catalogo,
) {
    let Some(f) = e.fila(li, fi).map(|(_, f)| f.clone()) else {
        return;
    };
    let encima = e.encima == Some(tarjetas::Pieza::Tarjeta(li, fi));
    let foco = e.foco == Foco::Tarjeta(li, fi);
    let radio = crate::v2::RADIO_BOTON * s;
    let fondo = if encima {
        crate::v2::TARJETA_ENCIMA
    } else {
        crate::v2::TARJETA
    };
    // La tarjeta a media opacidad sobre el fondo: «esto ya esta».
    if foco {
        p.rellenar_redondeado(r, radio, ACENTO);
        p.rellenar_redondeado(encoger(r, 1.5 * s), radio - 1.5 * s, mezcla(FONDO, fondo, 0.5));
    } else {
        p.rellenar_redondeado(r, radio, mezcla(FONDO, fondo, 0.5));
    }
    e.botones.zona(r, Accion::Enfocar(li, fi));
    let objetivo = RectF {
        x: r.x + 2.0 * s,
        y: r.y + (r.alto - CASILLA * s) / 2.0,
        ancho: CASILLA * s,
        alto: CASILLA * s,
    };
    pintar_casilla(e, p, objetivo, (li, fi), f.hecha, false, true, s);
    let (texto, emos) = emoticonos::emoticonos_de(&f.texto);
    // Sin texto (solo emoticonos), los emoticonos; solo una foto, la foto.
    let texto = if texto.is_empty() { emos.join(" ") } else { texto };
    let mut tx = objetivo.x + objetivo.ancho + 6.0 * s;
    // Su primera foto, pequena y apagada como el resto: con ella se
    // reconoce una tarea que era solo una foto.
    let foto = e.fila(li, fi).and_then(|(l, f)| e.ruta_de(l, f, 0));
    if let Some((b, iw, ih)) = foto.as_deref().and_then(|r| e.minis.ya(r)) {
        let lado = 28.0 * s;
        let c = RectF {
            x: tx,
            y: r.y + (r.alto - lado) / 2.0,
            ancho: lado,
            alto: lado,
        };
        crate::miniaturas::pintar_recortado(p, b, c, iw, ih);
        p.rellenar(c, con_alfa(mezcla(FONDO, fondo, 0.5), 0.5));
        tx += lado + 10.0 * s;
    }
    let a_mano = (encima || foco) && e.repartiendo.is_none();
    let derecha = r.x + r.ancho - 4.0 * s;
    let mut fin = derecha;
    if a_mano {
        let aspa = RectF {
            x: derecha - ASPA * s,
            y: r.y + (r.alto - ASPA * s) / 2.0,
            ancho: ASPA * s,
            alto: ASPA * s,
        };
        boton_quitar(e, p, aspa, (li, fi), false, s);
        fin = aspa.x - 8.0 * s;
    } else if let Some(t) = edad(&f, e.hoy, textos) {
        let tam_e = 12.0 * s;
        let (ew, eh) = p.medir_texto(&t, tam_e);
        let x = derecha - 12.0 * s - ew;
        p.texto(&t, x, r.y + (r.alto - eh) / 2.0, tam_e, con_alfa(GRIS, 0.6));
        fin = x - 12.0 * s;
    }
    let letra = Letra {
        tam: LETRA_FILA * s,
        negrita: false,
    };
    let ancho_t = (fin - tx).max(20.0 * s);
    // Entero, en los renglones que haga falta: la fila ya mide lo suyo
    // (`alto_de_cuadrado`). Uno solo, centrado; varios, desde arriba.
    let (_, th) = letra.medir(p, &texto, ancho_t);
    let ty = if th <= r.alto - 20.0 * s {
        r.y + (r.alto - th) / 2.0
    } else {
        r.y + 12.0 * s
    };
    pintar_texto_tarea(
        p,
        &texto,
        tx,
        ty,
        ancho_t,
        letra,
        palabras,
        con_alfa(TEXTO, 0.5),
        true,
        false,
        s,
    );
}

#[cfg(test)]
mod pruebas_colores {
    use super::*;

    #[test]
    fn cada_lista_tiene_su_color_sin_repetir() {
        let claves: Vec<(String, bool)> = (0..12).map(|i| (format!("lista {i}"), true)).collect();
        let c = colores_de_listas(&claves);
        let mut vistos = c.clone();
        vistos.sort();
        vistos.dedup();
        assert_eq!(vistos.len(), 12, "{c:?}");
        // El orden de la ventana no cambia el color de cada una.
        let mut al_reves = claves.clone();
        al_reves.reverse();
        let r = colores_de_listas(&al_reves);
        assert_eq!(r.iter().rev().copied().collect::<Vec<_>>(), c);
        // Caso negativo: una lista vacia no quita color a las demas.
        let mut con_vacia = claves[..11].to_vec();
        con_vacia.push(("vacia".into(), false));
        let v = colores_de_listas(&con_vacia);
        let mut d = v[..11].to_vec();
        d.sort();
        d.dedup();
        assert_eq!(d.len(), 11);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_caja_escribe_borra_y_se_mueve_por_letras_y_no_por_bytes() {
        let mut c = Campo::default();
        for l in "año".chars() {
            c.letra(l);
        }
        assert!(c.tecla(VK_IZQUIERDA, false));
        assert!(c.tecla(VK_IZQUIERDA, false));
        c.letra('x');
        assert_eq!(c.texto, "axño");
        c.tecla(VK_SUPRIMIR, false);
        assert_eq!(c.texto, "axo");
        c.tecla(VK_FIN, false);
        c.tecla(VK_RETROCESO, false);
        assert_eq!(c.texto, "ax");
        // Una tarea es una linea: lo pegado con saltos se junta.
        c.vaciar();
        c.escribir("comprar\r\npan");
        assert_eq!(c.texto, "comprar  pan");
        // Ctrl+Retroceso se lleva la palabra.
        c.tecla(VK_RETROCESO, true);
        assert_eq!(c.texto, "comprar  ");
        // Caso negativo: una letra de control no se escribe, e Intro no es
        // de la caja (es apuntar).
        assert!(!c.letra('\u{8}'));
        assert!(!c.tecla(VK_ENTRAR, false));
    }

    /// Una imagen de muestra, de colores para que se vea en el PNG.
    fn foto(nombre: &str, color: [u8; 3]) -> PathBuf {
        let carpeta = std::env::temp_dir().join("pixpin-tareas-pruebas");
        std::fs::create_dir_all(&carpeta).unwrap();
        let (w, h) = (64u32, 48u32);
        let mut pixeles = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let t = (x + y) as f32 / (w + h) as f32;
                pixeles.extend_from_slice(&[
                    (color[0] as f32 * (0.5 + t / 2.0)) as u8,
                    (color[1] as f32 * (0.5 + t / 2.0)) as u8,
                    (color[2] as f32 * (0.5 + t / 2.0)) as u8,
                    255,
                ]);
            }
        }
        let r = carpeta.join(nombre);
        let img = pixpin_codec::ImagenRgba {
            ancho: w,
            alto: h,
            pixeles,
        };
        pixpin_codec::guardar(&img, &r, pixpin_codec::FormatoImagen::Png).unwrap();
        r
    }

    #[test]
    fn pegar_una_imagen_mete_su_ficha_y_retroceso_la_borra_entera() {
        let mut c = Campo::default();
        c.escribir("comprar yeso");
        c.pegar(Some(pixpin_codec::ContenidoPortapapeles::Imagen(
            pixpin_codec::ImagenRgba {
                ancho: 1,
                alto: 1,
                pixeles: vec![1, 2, 3, 255],
            },
        )));
        assert_eq!(c.texto, "comprar yeso [img 01] ");
        let temporal = c.pegadas[0].1.ruta.clone();
        assert!(
            c.pegadas[0].1.temporal && temporal.is_file(),
            "el mapa va a un PNG temporal"
        );
        // Un fichero de imagen copiado del Explorador: su ficha, sin copia.
        let a = foto("explorador.png", [10, 200, 10]);
        c.pegar(Some(pixpin_codec::ContenidoPortapapeles::Rutas(vec![
            a.clone(),
        ])));
        assert_eq!(c.texto, "comprar yeso [img 01] [img 02] ");
        assert_eq!(c.imagenes(), vec![(1, temporal.clone()), (2, a.clone())]);
        // Retroceso tras el blanco borra el blanco; el siguiente, la ficha entera.
        c.tecla(VK_RETROCESO, false);
        c.tecla(VK_RETROCESO, false);
        assert_eq!(c.texto, "comprar yeso [img 01] ");
        assert_eq!(
            c.imagenes(),
            vec![(1, temporal.clone())],
            "la borrada se lleva su imagen"
        );
        // Las flechas la saltan de una vez, y Supr delante la borra.
        c.tecla(VK_IZQUIERDA, false);
        c.tecla(VK_IZQUIERDA, false);
        assert_eq!(c.cursor, "comprar yeso ".len());
        assert_eq!(c.ficha_en(false), Some(13..21));
        c.letra('x');
        assert_eq!(c.texto, "comprar yeso x[img 01] ");
        // Una imagen nueva no repite el numero de la borrada.
        c.tecla(VK_FIN, false);
        c.pegar(Some(pixpin_codec::ContenidoPortapapeles::Rutas(vec![
            a.clone(),
        ])));
        assert!(c.texto.ends_with("[img 03] "), "{}", c.texto);
        assert_eq!(c.lo_que_se_lee(), "comprar yeso x");
        // El texto pegado sigue entrando como texto.
        c.pegar(Some(pixpin_codec::ContenidoPortapapeles::Texto(
            "y arena".into(),
        )));
        assert!(c.texto.ends_with("[img 03] y arena"));
        assert!(c.aviso.is_none());
        // Vaciar borra el PNG temporal, pero no el fichero del Explorador.
        c.vaciar();
        assert!(!temporal.exists() && a.is_file());
        assert!(c.pegadas.is_empty());
    }

    #[test]
    fn una_imagen_un_nombre_pegarla_otra_vez_no_mete_otra_ficha() {
        let mapa = || {
            Some(pixpin_codec::ContenidoPortapapeles::Imagen(
                pixpin_codec::ImagenRgba {
                    ancho: 1,
                    alto: 1,
                    pixeles: vec![9, 8, 7, 255],
                },
            ))
        };
        let mut c = Campo::default();
        c.pegar(mapa());
        assert!(c.aviso.is_none());
        c.pegar(mapa());
        assert_eq!(c.texto, "[img 01] ", "la misma imagen no es [img 02]");
        assert_eq!(
            (c.aviso, c.aviso_ficha.as_deref()),
            (Some("tareas-pegar-repetida"), Some("[img 01]"))
        );
        // El mismo fichero del Explorador, igual (por su contenido).
        let a = foto("una-vez.png", [200, 10, 10]);
        c.aviso = None;
        c.pegar(Some(pixpin_codec::ContenidoPortapapeles::Rutas(vec![
            a.clone(),
        ])));
        c.pegar(Some(pixpin_codec::ContenidoPortapapeles::Rutas(vec![
            a.clone(),
        ])));
        assert_eq!(c.texto, "[img 01] [img 02] ");
        assert_eq!(c.aviso_ficha.as_deref(), Some("[img 02]"));
        // Caso negativo: otra imagen distinta si entra, y borrada la ficha
        // la misma imagen vuelve a poder pegarse.
        c.aviso = None;
        c.pegar(Some(pixpin_codec::ContenidoPortapapeles::Imagen(
            pixpin_codec::ImagenRgba {
                ancho: 1,
                alto: 1,
                pixeles: vec![1, 1, 1, 255],
            },
        )));
        assert!(
            c.texto.ends_with("[img 03] ") && c.aviso.is_none(),
            "{}",
            c.texto
        );
        c.vaciar();
        c.pegar(mapa());
        assert_eq!(c.texto, "[img 01] ");
        assert!(c.aviso.is_none());
        c.vaciar();
    }

    #[test]
    fn caso_negativo_lo_que_no_es_imagen_ni_texto_no_se_pega() {
        let mut c = Campo::default();
        let txt = std::env::temp_dir().join("pixpin-tareas-pruebas-nota.txt");
        std::fs::write(&txt, "x").unwrap();
        c.pegar(Some(pixpin_codec::ContenidoPortapapeles::Rutas(vec![txt])));
        assert_eq!(c.texto, "");
        assert_eq!(c.aviso, Some("tareas-pegar-no-imagen"));
        // Una ficha escrita a mano, sin imagen pegada, es texto.
        let mut c = Campo::default();
        c.escribir("[img 01] ");
        assert!(c.imagenes().is_empty());
        c.tecla(VK_RETROCESO, false);
        c.tecla(VK_RETROCESO, false);
        assert_eq!(c.texto, "[img 01", "se borra letra a letra");
        // Un mapa vacio no mete nada.
        c.pegar(Some(pixpin_codec::ContenidoPortapapeles::Imagen(
            pixpin_codec::ImagenRgba {
                ancho: 0,
                alto: 0,
                pixeles: vec![],
            },
        )));
        assert_eq!(c.texto, "[img 01");
    }

    fn muestra_estado() -> Estado {
        let lista = |proyecto: &str, chat: &str, titulo: &str, cuando: i64, doc: &str| Lista {
            proyecto: proyecto.into(),
            chat: chat.into(),
            guardados: false,
            codigo: format!("m{cuando}"),
            titulo: titulo.into(),
            cuando,
            filas: super::super::filas_de(doc),
        };
        let mut e = Estado::nuevo();
        e.hoy = pixpin_proyecto::mini::Fecha {
            anio: 2026,
            mes: 10,
            dia: 3,
        };
        e.listas = vec![
            lista(
                "g",
                "Mensajes guardados",
                super::super::INBOX,
                30,
                "- [ ] llamar al fontanero por la fuga del baño ➕ 2026-09-28\n- [ ] comprar pan ➕ 2026-10-03\n- [x] pagar la luz ➕ 2026-09-20\n- [ ] repasar el presupuesto de la cocina con Ana antes del viernes y mandarle las dudas sobre los muebles altos ➕ 2026-10-02\n- [ ] sin fecha",
            ),
            lista(
                "o",
                "Obra Miraflores",
                "Pendientes de obra",
                20,
                "- [ ] revisar puntales del segundo piso ➕ 2026-10-01\n- [x] pedir yeso ➕ 2026-10-02\n- [x] pedir arena",
            ),
            lista(
                "t",
                "Tesis",
                "Lecturas",
                10,
                "- [ ] leer el capítulo 3 de Fontana ➕ 2026-10-03",
            ),
        ];
        e.listas[0].guardados = true;
        // Una tarea con dos imagenes: una que esta y otra que aun no llego.
        let enlace = |n: &str| format!("pixpin:files/guardados/pc/general/archivos/{n}");
        e.listas[0].filas.insert(
            1,
            super::super::filas_de(&format!(
                "- [ ] grieta en el muro ![img 01]({}) ![img 02]({}) ➕ 2026-10-02",
                enlace("a.png"),
                enlace("b.png")
            ))
            .remove(0),
        );
        for (i, f) in e.listas[0].filas.iter_mut().enumerate() {
            f.indice = i;
        }
        e.rutas.insert(
            ("g".into(), enlace("a.png")),
            Some(foto("muro.png", [220, 120, 60])),
        );
        e.rutas.insert(("g".into(), enlace("b.png")), None);
        e.destinos = vec![e.listas[1].clone(), e.listas[2].clone()];
        e.abiertas.insert(e.listas[1].clave());
        e
    }

    /// Las miniaturas, todas antes de pintar (en la ventana se cargan de
    /// cuatro en cuatro, entre fotogramas).
    fn cargar_minis(e: &mut Estado, motor: &pixpin_render::MotorRender) {
        // Cada muestra trae su propio motor: los mapas del anterior no valen.
        e.minis.soltar();
        let rutas = e.imagenes_a_cargar();
        while e.minis.asegurar(&rutas, motor) {}
    }

    /// Las piezas colocadas como lo haria un fotograma, con tarjetas de alto
    /// fijo: para probar el teclado sin pintar.
    fn colocar(e: &mut Estado) {
        let palabras = tarjetas::palabras(&e.buscar.texto);
        let g = tarjetas::agrupar(&e.listas, &palabras, e.hoy, &|_, _| false);
        e.colocadas = tarjetas::disponer(
            &g,
            &|_| false,
            &mut |_, _| 60.0,
            tarjetas::Medidas {
                encabezado: 40.0,
                pliegue: 40.0,
                hueco: 8.0,
                entre_grupos: 18.0,
            },
        )
        .0;
    }

    #[test]
    fn el_teclado_lleva_el_foco_del_buscador_a_las_tarjetas_y_esc_va_por_capas() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let u = Ubicacion::Portable {
            raiz: std::env::temp_dir().join("pixpin-tareas-teclado-no-existe"),
        };
        let mut e = muestra_estado();
        colocar(&mut e);
        let t =
            |e: &mut Estado, vk: u32, ctrl: bool| tecla(e, vk, ctrl, false, &textos, &u, "PC01");
        assert_eq!(e.foco, Foco::Apuntar, "al abrir se apunta, como siempre");
        // Ctrl+F al buscador; lo escrito va a el y no a la caja.
        assert!(t(&mut e, VK_F, true));
        assert_eq!(e.foco, Foco::Buscar);
        for c in "PAN".chars() {
            letra(&mut e, c);
        }
        assert_eq!(
            (e.buscar.texto.as_str(), e.campo.texto.as_str()),
            ("PAN", "")
        );
        colocar(&mut e);
        // Abajo, a lo primero encontrado; Espacio no se escribe en ningun sitio.
        t(&mut e, VK_ABAJO, false);
        assert_eq!(e.foco, Foco::Tarjeta(0, 2), "comprar pan");
        letra(&mut e, ' ');
        assert_eq!(e.buscar.texto, "PAN");
        // Arriba de la primera vuelve al buscador (se esta buscando).
        t(&mut e, VK_ARRIBA, false);
        assert_eq!(e.foco, Foco::Buscar);
        // Esc: primero vacia la busqueda, luego cierra.
        assert!(t(&mut e, VK_ESCAPE, false));
        assert!(e.buscar.texto.is_empty());
        colocar(&mut e);
        // Sin buscar: la caja, y bajando las tarjetas en orden.
        t(&mut e, VK_TAB, false);
        assert_eq!(e.foco, Foco::Apuntar);
        t(&mut e, VK_ABAJO, false);
        let orden = tarjetas::en_orden(&e.colocadas);
        assert_eq!(e.foco, Foco::Tarjeta(orden[0].0, orden[0].1));
        t(&mut e, VK_ABAJO, false);
        assert_eq!(e.foco, Foco::Tarjeta(orden[1].0, orden[1].1));
        // Intro en una del Inbox abre «Mover a…» y las flechas eligen.
        assert!(e.se_reparte(orden[1].0, orden[1].1));
        t(&mut e, VK_ENTRAR, false);
        assert_eq!(e.repartiendo, Some(orden[1]));
        t(&mut e, VK_ABAJO, false);
        t(&mut e, VK_ABAJO, false);
        assert_eq!(e.destino_elegido, 1, "no se pasa del ultimo");
        assert!(t(&mut e, VK_ESCAPE, false));
        assert_eq!(e.repartiendo, None);
        // Caso negativo: Intro en una tarea de otra lista no abre nada, y
        // Esc sin nada escrito ni buscado cierra.
        e.foco = Foco::Tarjeta(1, 0);
        t(&mut e, VK_ENTRAR, false);
        assert_eq!(e.repartiendo, None);
        assert!(!t(&mut e, VK_ESCAPE, false));
    }

    /// Las capturas de la ventana, en `PIXPIN_MUESTRAS` (o en el temporal).
    #[test]
    #[ignore = "necesita GPU; ejecutar con --ignored y mirar el PNG"]
    fn muestra_de_la_ventana() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let (w, h) = (ANCHO_VENTANA, ALTO_VENTANA);
        let marco = Rect {
            x: 0,
            y: 0,
            ancho: w,
            alto: h,
        };
        let mut e = muestra_estado();
        // Sin buscar, con el raton encima de la tarea con imagenes del Inbox.
        crate::ventanita::muestra("tareas-v5-sin-buscar", w, h, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos);
            let c = tarjetas::sitio_de(&e.colocadas, 0, 1).expect("la grieta");
            let (_, arriba) = alturas(1.0);
            e.botones.raton = (300.0, arriba - e.scroll + c.y + 20.0);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        // Buscando «fontan»: el fontanero del Inbox y el libro de la tesis,
        // con su chapita.
        e.botones.raton = (0.0, 0.0);
        e.buscar.poner("fontan");
        e.foco = Foco::Buscar;
        crate::ventanita::muestra("tareas-v5-buscando", w, h, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        e.buscar.poner("ayer pedir");
        crate::ventanita::muestra("tareas-v5-buscando-ayer", w, h, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        e.buscar.poner("ventanas");
        crate::ventanita::muestra("tareas-v5-sin-resultados", w, h, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        // Teclado: el foco en la tarjeta de la grieta, y luego su «Mover a…».
        e.buscar.poner("");
        e.foco = Foco::Tarjeta(0, 1);
        e.campo.escribir("regar las plantas");
        // Con el aviso de una tarea recien quitada, con su «Deshacer».
        e.aviso = Some(Aviso::con_deshacer("«comprar pan» quitada", "Deshacer", Accion::Deshacer));
        crate::ventanita::muestra("tareas-v5-foco", w, h, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        e.repartiendo = Some((0, 1));
        e.aviso = None;
        crate::ventanita::muestra("tareas-v5-mover", w, h, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        e.campo.vaciar();
        // Sin ninguna tarea.
        let mut v = Estado::nuevo();
        crate::ventanita::muestra("tareas-v5-vacia", w, h, |p, _| {
            pintar_todo(&mut v, p, marco, 1.0, &textos)
        });

        // tareas-v4: los cuadrados, con fotos, pilas y emoticonos.
        e.repartiendo = None;
        con_fotos_y_emoticonos(&mut e);
        e.foco = Foco::Apuntar;
        crate::ventanita::muestra("tareas-v5-lista", w, h, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        e.vista = Vista::Tarjetas;
        crate::ventanita::muestra("tareas-v5-tarjetas", w, h, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos);
            // El raton encima de la grieta (dos fotos, del Inbox).
            let c = e
                .celdas
                .iter()
                .find(|c| c.pieza == tarjetas::Pieza::Tarjeta(0, 1))
                .expect("la grieta")
                .caja;
            let (_, arriba) = alturas(1.0);
            e.botones.raton = (MARGEN + c.x + 100.0, arriba - e.scroll + c.y + 100.0);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        e.botones.raton = (0.0, 0.0);
        e.scroll = 520.0;
        e.foco = Foco::Tarjeta(2, 1);
        crate::ventanita::muestra("tareas-v5-tarjetas-abajo", w, h, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        e.scroll = 0.0;
        e.foco = Foco::Buscar;
        e.buscar.poner("fontan");
        crate::ventanita::muestra("tareas-v5-tarjetas-buscando", w, h, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        // tareas-v5: las hechas en filas (abiertas las de la tesis), bajando
        // hasta ellas; y corrigiendo una tarea, con el raton en el icono de
        // imagen de la caja.
        e.buscar.poner("");
        e.abiertas.insert(e.listas[2].clave());
        e.foco = Foco::Apuntar;
        e.scroll = 100_000.0;
        crate::ventanita::muestra("tareas-v5-tarjetas-hechas", w, h, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        e.scroll = 0.0;
        e.corrigiendo = Some((e.listas[1].clone(), e.listas[1].filas[0].clone()));
        e.campo.escribir("revisar puntales del segundo piso");
        let (y_caja, _) = alturas(1.0);
        e.botones.raton = (w as f32 - MARGEN - 22.0, y_caja + CAJA / 2.0);
        crate::ventanita::muestra("tareas-v5-corrigiendo", w, h, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        e.corrigiendo = None;
        e.campo.vaciar();
        // El raton en el conmutador de la vista: su pista con el atajo.
        e.botones.raton = (w as f32 - 16.0 - 40.0 - 12.0 - 30.0, 32.0);
        crate::ventanita::muestra("tareas-v5-conmutador", w, h, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
    }

    /// La muestra con mas de todo para la vista en cuadrados: emoticonos en
    /// varias tareas, una con tres fotos, una solo foto ya hecha. Datos de
    /// ejemplo, nunca del usuario.
    fn con_fotos_y_emoticonos(e: &mut Estado) {
        let enlace = |n: &str| format!("pixpin:files/tesis/pc/general/archivos/{n}");
        let mas = super::super::filas_de(&format!(
            "- [ ] 🔥 ❤️ 🇵🇪 👨‍💻 planos de la casa nueva para la reunion del lunes ![img 01]({}) ![img 02]({}) ![img 03]({}) ➕ 2026-10-01\n- [ ] 📚 resumen del articulo de Ostrom 🧠\n- [x] ![img 01]({}) ➕ 2026-09-30\n- [ ] ✅ 🛒 comprar cartulinas y cola",
            enlace("p1.png"),
            enlace("p2.png"),
            enlace("p3.png"),
            enlace("hecha.png"),
        ));
        let l = &mut e.listas[2];
        l.filas.extend(mas);
        for (i, f) in l.filas.iter_mut().enumerate() {
            f.indice = i;
        }
        for (n, color) in [
            ("p1.png", [60, 120, 220]),
            ("p2.png", [60, 200, 120]),
            ("p3.png", [200, 60, 160]),
            ("hecha.png", [230, 200, 60]),
        ] {
            e.rutas.insert(("t".into(), enlace(n)), Some(foto(n, color)));
        }
        e.listas[0].filas[0].texto.push_str(" 🔧 🚿");
    }

    #[test]
    fn en_cuadrados_las_flechas_recorren_la_rejilla_y_la_vista_se_cambia() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let raiz = std::env::temp_dir().join(format!("pixpin-tareas-v4-{}", std::process::id()));
        std::fs::create_dir_all(&raiz).unwrap();
        let u = Ubicacion::Portable { raiz: raiz.clone() };
        let mut e = muestra_estado();
        assert!(hacer(&mut e, Accion::Vista(Vista::Tarjetas), &textos, &u, "PC01"));
        assert_eq!(Vista::leer(&raiz), Vista::Tarjetas, "se recuerda");
        let g = tarjetas::agrupar(&e.listas, &[], e.hoy, &|_, _| false);
        let (c, _) = rejilla::disponer(
            &g,
            &|_| false,
            &mut |_, _, l| l,
            &rejilla::Medidas {
                ancho: 628.0,
                lado: 200.0,
                hueco: 12.0,
                encabezado: 40.0,
                pliegue: 40.0,
                entre_grupos: 18.0,
                respiro: 8.0,
                fila_hecha: 44.0,
            },
        );
        e.colocadas = rejilla::colocadas(&c);
        e.celdas = c;
        let orden = tarjetas::en_orden(&e.colocadas);
        let t = |e: &mut Estado, vk: u32| tecla(e, vk, false, false, &textos, &u, "PC01");
        t(&mut e, VK_ABAJO);
        assert_eq!(e.foco, Foco::Tarjeta(orden[0].0, orden[0].1));
        t(&mut e, VK_DERECHA);
        assert_eq!(e.foco, Foco::Tarjeta(orden[1].0, orden[1].1));
        // El Inbox tiene 5 pendientes: abajo de la segunda, la quinta.
        t(&mut e, VK_ABAJO);
        assert_eq!(e.foco, Foco::Tarjeta(orden[4].0, orden[4].1));
        t(&mut e, VK_IZQUIERDA);
        assert_eq!(e.foco, Foco::Tarjeta(orden[3].0, orden[3].1));
        // Caso negativo: arriba desde la primera fila sale a la caja, y en
        // la lista izquierda y derecha no mueven el foco.
        t(&mut e, VK_ARRIBA);
        t(&mut e, VK_ARRIBA);
        assert_eq!(e.foco, Foco::Apuntar);
        hacer(&mut e, Accion::Vista(Vista::Lista), &textos, &u, "PC01");
        e.foco = Foco::Tarjeta(orden[1].0, orden[1].1);
        t(&mut e, VK_DERECHA);
        assert_eq!(e.foco, Foco::Tarjeta(orden[1].0, orden[1].1));
        assert_eq!(Vista::leer(&raiz), Vista::Lista);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_doble_clic_pinea_la_foto_de_un_cuadrado_y_si_no_corrige() {
        let a = Accion::Enfocar(0, 1);
        assert_eq!(al_doble_clic(a, Vista::Tarjetas, true), Accion::Imagen(0, 1, 0));
        assert_eq!(al_doble_clic(a, Vista::Tarjetas, false), Accion::Editar(0, 1));
        assert_eq!(al_doble_clic(a, Vista::Lista, true), Accion::Editar(0, 1));
        // Caso negativo: el doble clic en la casilla o en un boton sigue
        // siendo lo mismo (marcar dos veces es marcar y desmarcar).
        assert_eq!(
            al_doble_clic(Accion::Marcar(0, 1), Vista::Tarjetas, true),
            Accion::Marcar(0, 1)
        );
        assert_eq!(al_doble_clic(Accion::Fondo, Vista::Lista, false), Accion::Fondo);
    }

    #[test]
    fn el_boton_derecho_sabe_de_que_tarea_es_cada_zona() {
        assert_eq!(tarea_de(Accion::BuscarEmo(2, 3, 0)), Some((2, 3)));
        assert_eq!(tarea_de(Accion::PinearTodas(1, 0)), Some((1, 0)));
        assert_eq!(tarea_de(Accion::Imagen(0, 4, 1)), Some((0, 4)));
        // Caso negativo: el fondo, la caja o un encabezado no tienen menu.
        assert_eq!(tarea_de(Accion::Fondo), None);
        assert_eq!(tarea_de(Accion::EnfocarApuntar), None);
        assert_eq!(tarea_de(Accion::BorrarLista(0)), None);
    }

    #[test]
    fn el_menu_de_una_tarea_solo_ofrece_lo_que_se_puede_hacer() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let acciones = |hecha, reparte, imagenes| -> Vec<Accion> {
            menu_de_tarea(0, 1, hecha, reparte, imagenes, false, &textos)
                .into_iter()
                .map(|(_, _, a)| a)
                // Las horas de «Recordarmelo» se miran aparte: dependen de
                // cuando se pase la prueba.
                .filter(|a| !matches!(a, Accion::Recordar(..)))
                .collect()
        };
        assert_eq!(
            acciones(false, true, 2),
            [
                Accion::Marcar(0, 1),
                Accion::Editar(0, 1),
                Accion::Repartir(0, 1),
                Accion::CopiarTexto(0, 1),
                Accion::PinearTodas(0, 1),
                Accion::Quitar(0, 1),
            ]
        );
        let m = menu_de_tarea(0, 1, true, false, 2, false, &textos);
        assert!(m[0].1.starts_with("Marcar como pendiente"), "{}", m[0].1);
        assert!(m.iter().any(|(_, t, _)| t.contains("Pinear las 2")), "{m:?}");
        // Caso negativo: sin imagenes no hay «Pinear», fuera del Inbox no
        // hay «Mover a…», y los numeros no se repiten.
        let poco = acciones(false, false, 0);
        assert!(!poco.contains(&Accion::PinearTodas(0, 1)));
        assert!(!poco.contains(&Accion::Repartir(0, 1)));
        let ids: HashSet<u32> = menu_de_tarea(0, 1, false, true, 1, false, &textos)
            .iter()
            .map(|x| x.0)
            .collect();
        assert_eq!(ids.len(), 11, "seis y las cinco horas");
        // La hora: cinco para elegir en lo pendiente, ninguna en lo hecho, y
        // «Quitar el recordatorio» solo si tiene una.
        let horas = |hecha, con| {
            menu_de_tarea(0, 1, hecha, false, 0, con, &textos)
                .iter()
                .filter(|x| matches!(x.2, Accion::Recordar(..)))
                .count()
        };
        assert_eq!(horas(false, false), 5);
        assert_eq!(horas(true, false), 0);
        let quitar = |con| {
            menu_de_tarea(0, 1, false, false, 0, con, &textos)
                .iter()
                .any(|x| x.2 == Accion::OlvidarHora(0, 1))
        };
        assert!(quitar(true) && !quitar(false));
    }

    #[test]
    fn f2_pone_la_tarea_en_la_caja_y_esc_la_deja_como_estaba() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let u = Ubicacion::Portable {
            raiz: std::env::temp_dir().join("pixpin-tareas-f2-no-existe"),
        };
        let mut e = muestra_estado();
        e.foco = Foco::Tarjeta(1, 0);
        assert!(tecla(&mut e, VK_F2, false, false, &textos, &u, "PC01"));
        assert_eq!(e.foco, Foco::Apuntar);
        assert_eq!(e.campo.texto, "revisar puntales del segundo piso");
        assert_eq!(e.corrigiendo.as_ref().map(|(_, f)| f.indice), Some(0));
        // Esc suelta la correccion y vacia la caja, sin cerrar la ventana.
        assert!(tecla(&mut e, VK_ESCAPE, false, false, &textos, &u, "PC01"));
        assert!(e.corrigiendo.is_none() && e.campo.texto.is_empty());
        // Caso negativo: F2 sin una tarea con el foco no hace nada.
        tecla(&mut e, VK_F2, false, false, &textos, &u, "PC01");
        assert!(e.corrigiendo.is_none());
    }

    #[test]
    fn ctrl_l_y_ctrl_t_cambian_la_vista_y_un_emoticono_se_busca() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let raiz = std::env::temp_dir().join(format!("pixpin-tareas-v5-{}", std::process::id()));
        std::fs::create_dir_all(&raiz).unwrap();
        let u = Ubicacion::Portable { raiz: raiz.clone() };
        let mut e = muestra_estado();
        con_fotos_y_emoticonos(&mut e);
        tecla(&mut e, VK_T, true, false, &textos, &u, "PC01");
        assert_eq!(e.vista, Vista::Tarjetas);
        // Caso negativo: la L sin Ctrl no cambia la vista.
        tecla(&mut e, VK_L, false, false, &textos, &u, "PC01");
        assert_eq!(e.vista, Vista::Tarjetas);
        tecla(&mut e, VK_L, true, false, &textos, &u, "PC01");
        assert_eq!(e.vista, Vista::Lista);
        // El circulo del primer emoticono de «planos de la casa» busca 🔥.
        let fi = e.listas[2]
            .filas
            .iter()
            .position(|f| f.texto.contains("planos"))
            .unwrap();
        hacer(&mut e, Accion::BuscarEmo(2, fi, 0), &textos, &u, "PC01");
        assert_eq!((e.buscar.texto.as_str(), e.foco), ("🔥", Foco::Buscar));
        let g = tarjetas::agrupar(
            &e.listas,
            &tarjetas::palabras(&e.buscar.texto),
            e.hoy,
            &|_, _| false,
        );
        let salen: usize = g.iter().map(|g| g.arriba.len() + g.hechas.len()).sum();
        assert_eq!(salen, 1, "solo la que lleva 🔥");
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
