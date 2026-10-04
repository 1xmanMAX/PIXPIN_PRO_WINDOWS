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
//! **Teclado**: Ctrl+F al buscador; Tab da la vuelta buscador, caja,
//! tarjetas; las flechas pasan de tarjeta en tarjeta, Espacio marca la del
//! foco e Intro abre su «Mover a…» (y en la eleccion, las flechas eligen e
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

use super::{Fila, Lista, tarjetas};
use crate::caja_dibujo::hex;
use crate::lecciones::ui;
use crate::overlay::Recursos;
use crate::ventanita::{Botones, TEXTO, centrado, dentro};

// Los colores del rediseno v2 (los de la galeria y las lecciones). Como las
// demas ventanitas, la de tareas es oscura: no tiene tema claro.
const FONDO_V: Color = hex(0x1C1C1E);
const TEXTO_V: Color = hex(0xF5F5F7);
const GRIS: Color = hex(0x98989D);
/// Lo elegido y el foco.
const AZUL_V: Color = hex(0x0A84FF);
/// Lo buscado, resaltado en el texto de la tarea.
const RESALTE: Color = Color {
    a: 0.38,
    ..hex(0xB38F00)
};
/// La ficha `[img 01]` de una imagen pegada: letra azul clara sobre el
/// azul muy aguado, para que se vea que no es texto.
const FICHA_LETRA: Color = hex(0x9cc4f0);
const FICHA_FONDO: Color = Color { a: 0.24, ..AZUL_V };
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
const VK_V: u32 = 0x56;

/// El tamano de la ventana, en pixeles logicos.
const ANCHO_VENTANA: u32 = 660;
const ALTO_VENTANA: u32 = 800;

/// Medidas, en pixeles logicos.
const CABECERA: f32 = 56.0;
const CAJA: f32 = 44.0;
const MARGEN: f32 = 16.0;
/// Lo que baja la rueda por muesca, en «filas» de este alto (x1,5).
const FILA: f32 = 36.0;
const ENCABEZADO: f32 = 40.0;
const PLIEGUE: f32 = 40.0;
const HUECO: f32 = 8.0;
const ENTRE_GRUPOS: f32 = 18.0;
/// El objetivo de la casilla (el dibujo, de 24, va en medio).
const CASILLA: f32 = 40.0;
const TARJETA_MIN: f32 = 56.0;
/// La letra del texto de una tarea.
const TAM_TAREA: f32 = 15.0;

/// Cuanto se queda arriba, tachada, una tarea recien marcada.
const SE_QUEDA: Duration = Duration::from_secs(5);

/// La ventana abierta (su HWND), -1 mientras nace, 0 sin ventana: una sola
/// a la vez, y pedirla otra vez la trae delante.
static ABIERTA: AtomicIsize = AtomicIsize::new(0);

pub(super) fn abrir(idioma: Idioma, ubicacion: Ubicacion, aparato: String) {
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
    /// Las imagenes pegadas, con su numero (el de su ficha).
    pegadas: Vec<Pegada>,
    /// Lo que hay que decir despues de una tecla (la clave del aviso): un
    /// Ctrl+V con algo que no se puede pegar.
    aviso: Option<&'static str>,
    /// La ficha que va en el aviso (`$ficha`): la de una imagen que ya estaba.
    aviso_ficha: Option<String>,
}

/// Una imagen pegada en la caja, esperando a Intro.
#[derive(Debug, Clone)]
struct Pegada {
    numero: u32,
    ruta: PathBuf,
    /// Un PNG que se escribio al pegar un mapa de bits: se borra al vaciar
    /// la caja. Un fichero copiado del Explorador no es nuestro y se deja.
    temporal: bool,
    /// La huella de su contenido (`pixpin_lanzador::imagenes::huella`):
    /// una imagen, un nombre.
    huella: u64,
}

impl Campo {
    fn vaciar(&mut self) {
        self.texto.clear();
        self.cursor = 0;
        for p in self.pegadas.drain(..) {
            if p.temporal {
                let _ = std::fs::remove_file(&p.ruta);
            }
        }
    }

    /// Donde esta cada ficha de una imagen pegada que sigue en el texto (la
    /// primera vez que sale), ordenadas.
    fn fichas(&self) -> Vec<(std::ops::Range<usize>, &Pegada)> {
        let mut v: Vec<_> = self
            .pegadas
            .iter()
            .filter_map(|p| {
                let f = mini::ficha_de_imagen(p.numero);
                self.texto.find(&f).map(|i| (i..i + f.len(), p))
            })
            .collect();
        v.sort_by_key(|(r, _)| r.start);
        v
    }

    /// La ficha que acaba justo en el cursor, o la que empieza en el.
    fn ficha_en(&self, antes: bool) -> Option<std::ops::Range<usize>> {
        self.fichas().into_iter().map(|(r, _)| r).find(|r| {
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
            .map(|(_, p)| (p.numero, p.ruta.clone()))
            .collect()
    }

    /// Lo que dice el aviso de la tarea apuntada: el texto sin las fichas.
    fn lo_que_se_lee(&self) -> String {
        let mut s = self.texto.clone();
        for (r, _) in self.fichas().into_iter().rev() {
            s.replace_range(r, " ");
        }
        let s = mini::saneado(&s);
        match self.fichas().first() {
            Some((_, p)) if s.is_empty() => mini::ficha_de_imagen(p.numero),
            _ => s,
        }
    }

    /// Mete la ficha de una imagen en el cursor, separada por blancos de lo
    /// que tenga alrededor.
    fn meter_imagen(&mut self, ruta: PathBuf, temporal: bool, huella: u64) {
        let numero = self.pegadas.iter().map(|p| p.numero).max().unwrap_or(0) + 1;
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
        self.pegadas.push(Pegada {
            numero,
            ruta,
            temporal,
            huella,
        });
    }

    /// **Una imagen, un nombre**: la ficha que sigue en el texto con una
    /// imagen de esa huella. Pegarla otra vez no mete nada; se avisa.
    fn ya_pegada(&mut self, huella: u64) -> bool {
        let Some(numero) = self
            .fichas()
            .into_iter()
            .find(|(_, p)| p.huella == huella)
            .map(|(_, p)| p.numero)
        else {
            return false;
        };
        self.aviso = Some("tareas-pegar-repetida");
        self.aviso_ficha = Some(mini::ficha_de_imagen(numero));
        true
    }

    /// Lo que hace Ctrl+V con lo que haya en el portapapeles.
    fn pegar(&mut self, contenido: Option<pixpin_codec::ContenidoPortapapeles>) {
        use pixpin_codec::ContenidoPortapapeles as C;
        use pixpin_lanzador::imagenes::huella;
        match contenido {
            Some(C::Texto(t)) => self.escribir(&t),
            Some(C::Imagen(img)) if img.ancho > 0 && img.alto > 0 => {
                // Los pixeles y el tamano: la misma imagen, la misma huella.
                let mut bytes = Vec::with_capacity(img.pixeles.len() + 8);
                bytes.extend_from_slice(&img.ancho.to_le_bytes());
                bytes.extend_from_slice(&img.alto.to_le_bytes());
                bytes.extend_from_slice(&img.pixeles);
                let h = huella(&bytes);
                if self.ya_pegada(h) {
                    return;
                }
                match png_temporal(&img) {
                    Ok(ruta) => self.meter_imagen(ruta, true, h),
                    Err(e) => {
                        tracing::warn!(?e, "tareas: la imagen pegada no se pudo guardar");
                        self.aviso = Some("tareas-pegar-no-imagen");
                    }
                }
            }
            Some(C::Rutas(rutas)) => {
                let imagenes: Vec<_> = rutas.into_iter().filter(|r| super::es_imagen(r)).collect();
                if imagenes.is_empty() {
                    self.aviso = Some("tareas-pegar-no-imagen");
                }
                for r in imagenes {
                    // Por su contenido; si no se puede leer, por su ruta.
                    let h = std::fs::read(&r)
                        .map(|b| huella(&b))
                        .unwrap_or_else(|_| huella(r.to_string_lossy().as_bytes()));
                    if !self.ya_pegada(h) {
                        self.meter_imagen(r, false, h);
                    }
                }
            }
            _ => {}
        }
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

/// Guarda un mapa de bits pegado en un PNG temporal, hasta Intro.
fn png_temporal(img: &pixpin_codec::ImagenRgba) -> Result<PathBuf> {
    static CUENTA: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let carpeta = std::env::temp_dir().join("pixpin-tareas");
    std::fs::create_dir_all(&carpeta)?;
    let n = CUENTA.fetch_add(1, Ordering::Relaxed);
    let ruta = carpeta.join(format!(
        "pegada-{}-{}-{n}.png",
        std::process::id(),
        pixpin_shell::entorno::ahora_utc_ms()
    ));
    pixpin_codec::guardar(img, &ruta, pixpin_codec::FormatoImagen::Png)?;
    Ok(ruta)
}

// --------------------------------------------------------------- la ventana

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Mover,
    Cerrar,
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
    aviso: Option<(String, Instant)>,
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

    fn decir(&mut self, t: String) {
        self.aviso = Some((t, Instant::now()));
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
    e.recargar(ubicacion);
    tracing::info!(listas = e.listas.len(), "ventana de tareas abierta");
    let mut mirado = Instant::now();
    let mut vivo = true;
    let mut pintar = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
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
                        Some(a) => vivo = hacer(&mut e, a, textos, ubicacion, aparato),
                        None => {}
                    }
                }
                EventoOverlay::BotonSoltado(_) => {
                    if e.moviendo.take().is_some() {
                        ventana.soltar_raton();
                    }
                }
                // La rueda llega en 120 por muesca (y en trozos desde un panel tactil).
                EventoOverlay::Rueda(m) => e.scroll -= m as f32 / 120.0 * FILA * 1.5 * escala,
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
        if e.aviso
            .as_ref()
            .is_some_and(|(_, t)| t.elapsed() > Duration::from_millis(2_500))
        {
            e.aviso = None;
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
        (VK_ENTRAR, Foco::Tarjeta(li, fi)) => {
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
        _ => {}
    }
    true
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
        Accion::Apuntar => {
            if e.campo.texto.trim().is_empty() {
                return true;
            }
            match super::apuntar_con(raiz, aparato, &e.campo.texto, &e.campo.imagenes()) {
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
    }
    true
}

// ---------------------------------------------------------------- pintar

fn blanco(a: f32) -> Color {
    Color { a, ..Color::BLANCO }
}

/// Un recuadro redondeado con borde: el borde es el mismo recuadro un poco
/// mayor por debajo (como en la galeria).
fn con_borde(p: &Pintor, r: RectF, radio: f32, fondo: Color, borde: Color, grosor: f32) {
    p.rellenar_redondeado(r, radio, borde);
    p.rellenar_redondeado(encoger(r, grosor), (radio - grosor).max(0.0), fondo);
}

/// Donde empieza el contenido desplazable y donde van el buscador y la
/// caja, ya con la escala: `(buscador, caja, contenido)`.
fn alturas(s: f32) -> (f32, f32, f32) {
    let buscador = CABECERA * s;
    let caja = buscador + CAJA * s + 10.0 * s;
    (buscador, caja, caja + CAJA * s + 16.0 * s)
}

fn pintar_todo(e: &mut Estado, p: &Pintor, marco: Rect, s: f32, textos: &Catalogo) {
    let (w, h) = (marco.ancho as f32, marco.alto as f32);
    p.limpiar(FONDO_V);
    e.botones.vaciar();
    e.botones.zona(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho: w,
            alto: h,
        },
        Accion::Fondo,
    );
    let m = MARGEN * s;
    let ancho = w - 2.0 * m;
    let (y_buscar, y_caja, arriba) = alturas(s);

    // Lo que sale y donde: agrupar, medir cada tarjeta y colocarlas.
    let palabras = tarjetas::palabras(&e.buscar.texto);
    let buscando = !palabras.is_empty();
    let grupos = {
        let recien = &e.recien;
        tarjetas::agrupar(&e.listas, &palabras, e.hoy, &|l, f| {
            recien.contains_key(&(l.clave(), f.crudo.clone()))
        })
    };
    let (colocadas, total) = {
        let abierta = |li: usize| {
            buscando
                || e.listas
                    .get(li)
                    .is_some_and(|l| e.abiertas.contains(&l.clave()))
        };
        let est: &Estado = e;
        tarjetas::disponer(
            &grupos,
            &abierta,
            &mut |li, fi| hechura(est, p, li, fi, ancho, s, buscando, textos).alto,
            tarjetas::Medidas {
                encabezado: ENCABEZADO * s,
                pliegue: PLIEGUE * s,
                hueco: HUECO * s,
                entre_grupos: ENTRE_GRUPOS * s,
            },
        )
    };
    e.colocadas = colocadas;
    e.alto_contenido = total + 24.0 * s;
    let area = RectF {
        x: 0.0,
        y: arriba,
        ancho: w,
        alto: (h - arriba).max(0.0),
    };
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
        tarjetas::pieza_en(&e.colocadas, e.botones.raton.1 - arriba + e.scroll)
    } else {
        None
    };

    // El contenido, desplazable, ANTES que lo de arriba: asi la cabecera y
    // las cajas se apuntan despues y le ganan a una tarjeta que asome por
    // debajo.
    p.con_recorte(area, |p| {
        let y0 = arriba - e.scroll;
        if e.listas.is_empty() {
            pintar_vacia(p, m, y0, ancho, s, textos);
        } else if grupos.is_empty() {
            pintar_sin_resultados(e, p, m, y0, ancho, s, textos);
        } else {
            let colocadas = e.colocadas.clone();
            for c in colocadas {
                let y = y0 + c.y;
                if y + c.alto < area.y || y > area.y + area.alto {
                    continue;
                }
                match c.pieza {
                    tarjetas::Pieza::Encabezado(li) => {
                        if let Some(g) = grupos.iter().find(|g| g.lista == li) {
                            pintar_encabezado(e, p, g, m, y, ancho, s, textos);
                        }
                    }
                    tarjetas::Pieza::Pliegue(li) => {
                        let n = grupos
                            .iter()
                            .find(|g| g.lista == li)
                            .map_or(0, |g| g.hechas.len());
                        pintar_pliegue(e, p, li, n, m, y, ancho, s, buscando, textos);
                    }
                    tarjetas::Pieza::Tarjeta(li, fi) => {
                        let r = RectF {
                            x: m,
                            y,
                            ancho,
                            alto: c.alto,
                        };
                        pintar_tarjeta(e, p, li, fi, r, s, &palabras, textos);
                    }
                }
            }
        }
    });

    // Lo de arriba, tapando lo que se desplazo bajo ello.
    p.rellenar(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho: w,
            alto: arriba,
        },
        FONDO_V,
    );
    let cab = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: CABECERA * s,
    };
    e.botones.zona(cab, Accion::Mover);
    let titulo = textos.t("tareas-titulo");
    ui::negrita(p, &titulo, m, 14.0 * s, 20.0 * s, 300.0 * s, TEXTO_V);
    let (tw, _) = ui::medir_negrita(p, &titulo, 20.0 * s, 300.0 * s);
    let resumen = if buscando {
        let n: usize = grupos.iter().map(|g| g.arriba.len() + g.hechas.len()).sum();
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("n", n as i64);
        textos.t_args("tareas3-encontradas", &args)
    } else {
        let pendientes: usize = e.listas.iter().map(Lista::cuantas_pendientes).sum();
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("pendientes", pendientes as i64);
        args.set("listas", e.listas.len() as i64);
        textos.t_args("tareas-resumen", &args)
    };
    let lado = 40.0 * s;
    p.texto_linea(
        &resumen,
        m + tw + 12.0 * s,
        21.0 * s,
        13.0 * s,
        (w - 2.0 * m - tw - 12.0 * s - lado - 8.0 * s).max(0.0),
        GRIS,
    );
    let cerrar = RectF {
        x: w - m - lado + 6.0 * s,
        y: 8.0 * s,
        ancho: lado,
        alto: lado,
    };
    if dentro(cerrar, e.botones.raton) {
        p.rellenar_redondeado(cerrar, 10.0 * s, blanco(0.08));
    }
    p.icono(&mi::CLOSE, encoger(cerrar, 10.0 * s), GRIS);
    e.botones.zona(cerrar, Accion::Cerrar);

    pintar_buscador(e, p, m, y_buscar, ancho, s, textos);
    pintar_caja(e, p, m, y_caja, ancho, s, textos);

    if e.repartiendo.is_some() {
        pintar_eleccion(e, p, w, h, s, textos);
    } else {
        pintar_vista_de_ficha(e, p, w, s);
    }

    if let Some((t, _)) = &e.aviso {
        let tam = 14.0 * s;
        let (tw, th) = p.medir_texto_ajustado(t, tam, w - 60.0 * s);
        let caja = RectF {
            x: (w - tw) / 2.0 - 14.0 * s,
            y: h - th - 34.0 * s,
            ancho: tw + 28.0 * s,
            alto: th + 16.0 * s,
        };
        p.rellenar_redondeado(
            caja,
            8.0 * s,
            Color {
                a: 0.92,
                ..Color::NEGRO
            },
        );
        p.texto_ajustado(t, caja.x + 14.0 * s, caja.y + 8.0 * s, tam, tw + 2.0, TEXTO);
    }
}

/// El buscador, arriba del todo: la lupa, lo escrito (o la pista) y a la
/// derecha la chapita «Ctrl F» o, con algo escrito, la ✕ que lo vacia.
fn pintar_buscador(
    e: &mut Estado,
    p: &Pintor,
    x: f32,
    y: f32,
    ancho: f32,
    s: f32,
    textos: &Catalogo,
) {
    let lado = CAJA * s;
    let caja = RectF {
        x,
        y,
        ancho,
        alto: lado,
    };
    let foco = e.foco == Foco::Buscar;
    let borde = if foco {
        AZUL_V
    } else if dentro(caja, e.botones.raton) {
        blanco(0.2)
    } else {
        blanco(0.1)
    };
    con_borde(p, caja, 12.0 * s, hex(0x2A2A2D), borde, 1.0 * s);
    e.botones.zona(caja, Accion::EnfocarBuscar);
    let li = 18.0 * s;
    p.icono(
        &mi::SEARCH,
        RectF {
            x: x + 14.0 * s,
            y: y + (lado - li) / 2.0,
            ancho: li,
            alto: li,
        },
        if foco { AZUL_V } else { GRIS },
    );
    let derecha = if e.buscar.texto.is_empty() {
        let cw = ui::ancho_de_chapa(p, "Ctrl F", s);
        ui::chapa(
            p,
            "Ctrl F",
            x + ancho - 12.0 * s - cw,
            y + lado / 2.0,
            hex(0xD1D1D6),
            hex(0x2E2E31),
            s,
        );
        cw + 20.0 * s
    } else {
        let l = 36.0 * s;
        let b = RectF {
            x: x + ancho - 4.0 * s - l,
            y: y + (lado - l) / 2.0,
            ancho: l,
            alto: l,
        };
        if dentro(b, e.botones.raton) {
            p.rellenar_redondeado(b, 9.0 * s, blanco(0.1));
        }
        p.icono(&mi::CLOSE, encoger(b, 9.0 * s), GRIS);
        e.botones.zona(b, Accion::VaciarBusqueda);
        l + 8.0 * s
    };
    let tx = x + 44.0 * s;
    let tam = 15.0 * s;
    let (_, th) = p.medir_texto("Ag", tam);
    let campo = RectF {
        x: tx,
        y,
        ancho: (x + ancho - derecha - tx).max(0.0),
        alto: lado,
    };
    let pista = textos.t("tareas3-buscar");
    let buscar = &e.buscar;
    p.con_recorte(campo, |p| {
        // Sin el cursor de las lecciones (amarillo): el azul del foco de
        // esta ventana, como en la caja de apuntar.
        let ty = y + (lado - th) / 2.0;
        buscar.pintar_texto(p, tx, ty, 100_000.0, tam, false, &pista, s);
        if foco {
            let mut con_marca = buscar.texto.clone();
            con_marca.insert(buscar.cursor, '\u{200B}');
            let i = buscar.texto[..buscar.cursor].encode_utf16().count() as u32;
            let cx = p
                .cajas_de_trozo(&con_marca, tam, 100_000.0, &[], i, 1)
                .first()
                .map_or(0.0, |b| b.x);
            p.linea((tx + cx, ty), (tx + cx, ty + th), 1.5 * s, AZUL_V);
        }
    });
}

/// La caja para apuntar: con el foco, lo que se escribe va a ella e Intro
/// lo apunta en el Inbox.
fn pintar_caja(e: &mut Estado, p: &Pintor, x: f32, y: f32, ancho: f32, s: f32, textos: &Catalogo) {
    let caja = RectF {
        x,
        y,
        ancho,
        alto: CAJA * s,
    };
    let foco = e.foco == Foco::Apuntar;
    let borde = if foco {
        AZUL_V
    } else if dentro(caja, e.botones.raton) {
        blanco(0.2)
    } else {
        blanco(0.1)
    };
    con_borde(p, caja, 12.0 * s, hex(0x2A2A2D), borde, 1.0 * s);
    e.botones.zona(caja, Accion::EnfocarApuntar);
    let tam = 15.0 * s;
    let (_, alto_linea) = p.medir_texto("Ag", tam);
    let ty = y + (caja.alto - alto_linea) / 2.0;
    let mas = "＋";
    let (mw, mh) = p.medir_texto(mas, 18.0 * s);
    p.texto_color(
        mas,
        x + 16.0 * s,
        y + (caja.alto - mh) / 2.0,
        18.0 * s,
        AZUL_V,
    );
    let tx = x + 16.0 * s + mw + 10.0 * s;
    // A la derecha, con algo escrito, el boton de apuntar con su «Intro».
    let mut fin = x + ancho - 18.0 * s;
    if !e.campo.texto.trim().is_empty() {
        let rotulo = textos.t("tareas-apuntar-boton");
        let bw = ui::ancho_de_boton(p, false, &rotulo, Some("Intro"), s);
        let b = RectF {
            x: x + ancho - bw - 5.0 * s,
            y: y + 5.0 * s,
            ancho: bw,
            alto: caja.alto - 10.0 * s,
        };
        ui::boton_v2(
            p,
            &mut e.botones,
            b,
            Accion::Apuntar,
            None,
            &rotulo,
            Some("Intro"),
            Some(ui::v2::AZUL),
            ui::v2::BLANCO,
            s,
        );
        fin = b.x - 8.0 * s;
    } else if foco {
        let cw = ui::ancho_de_chapa(p, "Intro", s);
        ui::chapa(
            p,
            "Intro",
            x + ancho - 12.0 * s - cw,
            y + caja.alto / 2.0,
            hex(0xD1D1D6),
            hex(0x2E2E31),
            s,
        );
        fin = x + ancho - 20.0 * s - cw;
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
        p.texto_linea(&textos.t("tareas-apuntar"), tx, ty, tam, hueco, GRIS);
        if foco {
            p.linea((tx, ty), (tx, ty + alto_linea), 1.5 * s, AZUL_V);
        }
        return;
    }
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
        .map(|(r, peg)| (r, peg.ruta.clone()))
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
                None => p.texto(&texto[r.clone()], x0, ty, tam, TEXTO_V),
                Some(ruta) => {
                    let chapa = RectF {
                        x: x0 - 3.0 * s,
                        y: ty - 2.0 * s,
                        ancho: b.ancho + 6.0 * s,
                        alto: alto_linea + 4.0 * s,
                    };
                    p.rellenar_redondeado(chapa, 5.0 * s, FICHA_FONDO);
                    p.texto(&texto[r.clone()], x0, ty, tam, FICHA_LETRA);
                    pintadas.push((chapa, ruta.clone()));
                }
            }
        }
        if foco {
            let c = tx - corrido + cx;
            p.linea((c, ty), (c, ty + alto_linea), 1.5 * s, AZUL_V);
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
    p.rellenar_redondeado(marco, 10.0 * s, hex(0x2C2C2E));
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
/// cierra al pulsarlo y, encima, una tarjeta con un renglon por grupo (su
/// titulo y su chat). Las flechas eligen e Intro mueve.
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
    p.rellenar(
        todo,
        Color {
            a: 0.55,
            ..Color::NEGRO
        },
    );
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
    con_borde(p, caja, 14.0 * s, hex(0x2C2C2E), blanco(0.1), 1.0 * s);
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
        TEXTO_V,
    );
    // Abajo, los atajos: «↑↓ Intro» elige, «Esc» cierra.
    let yc = caja.y + caja.alto - pie / 2.0;
    let mut xx = caja.x + pad;
    for (t, rotulo) in [
        ("↑↓", None),
        ("Intro", Some(textos.t("tareas3-mover-intro"))),
        ("Esc", Some(textos.t("tareas3-mover-esc"))),
    ] {
        xx += ui::chapa(p, t, xx, yc, hex(0xD1D1D6), hex(0x2E2E31), s) + 6.0 * s;
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
                p.rellenar_redondeado(r, 8.0 * s, Color { a: 0.22, ..AZUL_V });
            } else if dentro(r, e.botones.raton) {
                p.rellenar_redondeado(r, 8.0 * s, blanco(0.06));
            }
            p.texto_linea(
                &d.titulo,
                r.x + pad - 6.0 * s,
                r.y + 6.0 * s,
                14.5 * s,
                r.ancho - 2.0 * pad,
                TEXTO_V,
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

/// Sin ninguna tarea: que se ve y como se empieza.
fn pintar_vacia(p: &Pintor, x: f32, y: f32, ancho: f32, s: f32, textos: &Catalogo) {
    let lado = 44.0 * s;
    let y = y + 56.0 * s;
    p.icono(
        &mi::CHECKLIST,
        RectF {
            x: x + (ancho - lado) / 2.0,
            y,
            ancho: lado,
            alto: lado,
        },
        GRIS,
    );
    let t = textos.t("tareas-vacia");
    let anch = ancho.min(440.0 * s);
    let (tw, _) = p.medir_texto_ajustado(&t, 15.0 * s, anch);
    p.texto_ajustado(
        &t,
        x + (ancho - tw) / 2.0,
        y + lado + 16.0 * s,
        15.0 * s,
        anch,
        GRIS,
    );
}

/// Buscando sin nada que coincida: lo que se busco, como buscar mejor y un
/// boton para vaciar la busqueda (o Esc).
fn pintar_sin_resultados(
    e: &mut Estado,
    p: &Pintor,
    x: f32,
    y: f32,
    ancho: f32,
    s: f32,
    textos: &Catalogo,
) {
    let lado = 44.0 * s;
    let mut y = y + 48.0 * s;
    p.icono(
        &mi::SEARCH,
        RectF {
            x: x + (ancho - lado) / 2.0,
            y,
            ancho: lado,
            alto: lado,
        },
        GRIS,
    );
    y += lado + 16.0 * s;
    let anch = ancho.min(460.0 * s);
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("busqueda", ui::corto(e.buscar.texto.trim(), 40));
    let titulo = textos.t_args("tareas3-sin-resultados", &args);
    let (tw, th) = ui::medir_negrita(p, &titulo, 16.0 * s, anch);
    ui::negrita(
        p,
        &titulo,
        x + (ancho - tw) / 2.0,
        y,
        16.0 * s,
        anch,
        TEXTO_V,
    );
    y += th + 8.0 * s;
    let pista = textos.t("tareas3-sin-resultados-pista");
    let (pw, ph) = p.medir_texto_ajustado(&pista, 13.5 * s, anch);
    p.texto_ajustado(&pista, x + (ancho - pw) / 2.0, y, 13.5 * s, anch, GRIS);
    y += ph + 18.0 * s;
    let rotulo = textos.t("tareas3-vaciar-busqueda");
    let bw = ui::ancho_de_boton(p, false, &rotulo, Some("Esc"), s);
    let b = RectF {
        x: x + (ancho - bw) / 2.0,
        y,
        ancho: bw,
        alto: 40.0 * s,
    };
    ui::boton_v2(
        p,
        &mut e.botones,
        b,
        Accion::VaciarBusqueda,
        None,
        &rotulo,
        Some("Esc"),
        Some(hex(0x2C2C2E)),
        TEXTO_V,
        s,
    );
}

/// El encabezado de un grupo: el nombre de la lista en negrita, «· su
/// chat» en gris y a la derecha cuantas quedan pendientes.
#[allow(clippy::too_many_arguments)] // estado, pintor, grupo, sitio, escala y textos
fn pintar_encabezado(
    e: &Estado,
    p: &Pintor,
    g: &tarjetas::Grupo,
    x: f32,
    y: f32,
    ancho: f32,
    s: f32,
    textos: &Catalogo,
) {
    let Some(l) = e.listas.get(g.lista) else {
        return;
    };
    let tam = 15.0 * s;
    let (_, th) = p.medir_texto("Ag", tam);
    let ty = y + ENCABEZADO * s - th - 8.0 * s;
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("n", g.pendientes(l) as i64);
    let cuenta = textos.t_args("tareas3-pendientes", &args);
    let (cw, ch) = p.medir_texto(&cuenta, 12.5 * s);
    p.texto(
        &cuenta,
        x + ancho - cw - 4.0 * s,
        ty + (th - ch) / 2.0,
        12.5 * s,
        GRIS,
    );
    let libre = (ancho - cw - 20.0 * s).max(0.0);
    let (nw, _) = ui::medir_negrita(p, &l.titulo, tam, libre);
    let nw = nw.min(libre * 0.7);
    p.con_recorte(
        RectF {
            x,
            y,
            ancho: nw + 6.0 * s,
            alto: ENCABEZADO * s,
        },
        |p| ui::negrita(p, &l.titulo, x + 4.0 * s, ty, tam, 100_000.0, TEXTO_V),
    );
    p.texto_linea(
        &format!("·  {}", l.chat),
        x + 4.0 * s + nw + 8.0 * s,
        ty + 1.0 * s,
        13.5 * s,
        (libre - nw - 12.0 * s).max(0.0),
        GRIS,
    );
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
        p.rellenar_redondeado(r, 10.0 * s, blanco(0.05));
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
        };
    };
    let pad = 16.0 * s;
    let texto_x = 8.0 * s + CASILLA * s + 10.0 * s;
    let mut reserva = edad(f, e.hoy, textos).map_or(0.0, |t| p.medir_texto(&t, 12.0 * s).0);
    if e.se_reparte(li, fi) {
        let b = ui::ancho_de_boton(p, true, &textos.t("tareas-mover"), Some("Intro"), s);
        reserva = reserva.max(b);
    }
    if reserva > 0.0 {
        reserva += 12.0 * s;
    }
    let texto_ancho = (ancho - texto_x - pad - reserva).max(40.0 * s);
    let texto_alto = if f.texto.is_empty() {
        0.0
    } else {
        p.medir_texto_ajustado(&f.texto, TAM_TAREA * s, texto_ancho)
            .1
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
    }
}

/// Una tarea en su tarjeta: la casilla grande (lo unico que tacha), el
/// texto entero en los renglones que haga falta (lo buscado resaltado), sus
/// imagenes (un clic las pinea), buscando la chapita de su lista y su chat,
/// y arriba a la derecha cuantos dias lleva o, con el raton encima o el
/// foco, «Mover a…» si es del Inbox.
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
    let fondo = match (f.hecha, encima) {
        (false, false) => hex(0x2A2A2D),
        (false, true) => hex(0x313135),
        (true, false) => hex(0x222225),
        (true, true) => hex(0x29292C),
    };
    if foco {
        con_borde(p, r, 12.0 * s, fondo, AZUL_V, 1.5 * s);
    } else {
        con_borde(p, r, 12.0 * s, fondo, blanco(0.06), 1.0 * s);
    }
    // La tarjeta entera da el foco; lo de dentro se apunta despues y gana.
    e.botones.zona(r, Accion::Enfocar(li, fi));

    // La casilla: un objetivo de 40 px con el dibujo de 24 en medio.
    let objetivo = RectF {
        x: r.x + 8.0 * s,
        y: r.y + 8.0 * s,
        ancho: CASILLA * s,
        alto: CASILLA * s,
    };
    if dentro(objetivo, e.botones.raton) {
        p.circulo(
            (
                objetivo.x + objetivo.ancho / 2.0,
                objetivo.y + objetivo.alto / 2.0,
            ),
            18.0 * s,
            blanco(0.1),
        );
    }
    let icono = if f.hecha {
        &mi::CHECK_BOX
    } else {
        &mi::CHECK_BOX_OUTLINE_BLANK
    };
    p.icono(
        icono,
        encoger(objetivo, (CASILLA - 24.0) / 2.0 * s),
        if f.hecha { AZUL_V } else { hex(0xAEAEB2) },
    );
    e.botones.zona(objetivo, Accion::Marcar(li, fi));

    // El texto, partido en renglones; lo buscado con un fondo amarillo
    // debajo, en las mismas cajas que pinta DirectWrite.
    let tx = r.x + hc.texto_x;
    let ty = r.y + 18.0 * s;
    let tam = TAM_TAREA * s;
    if !f.texto.is_empty() {
        for palabra in palabras {
            for t in pixpin_ui::resaltado::coincidencias(&f.texto, palabra) {
                let (inicio, largo) = t.en_utf16(&f.texto);
                if largo == 0 {
                    continue;
                }
                for b in p.cajas_de_trozo(&f.texto, tam, hc.texto_ancho, &[], inicio, largo) {
                    p.rellenar_redondeado(
                        RectF {
                            x: tx + b.x - 1.0 * s,
                            y: ty + b.y,
                            ancho: b.ancho + 2.0 * s,
                            alto: b.alto,
                        },
                        3.0 * s,
                        RESALTE,
                    );
                }
            }
        }
        let color = if f.hecha { GRIS } else { TEXTO_V };
        p.texto_ajustado(&f.texto, tx, ty, tam, hc.texto_ancho, color);
        // Lo hecho se tacha, renglon a renglon, como en el movil.
        if f.hecha {
            let largo = f.texto.encode_utf16().count() as u32;
            for b in p.cajas_de_trozo(&f.texto, tam, hc.texto_ancho, &[], 0, largo) {
                let ly = ty + b.y + b.alto * 0.55;
                p.linea((tx + b.x, ly), (tx + b.x + b.ancho, ly), 1.2 * s, GRIS);
            }
        }
    }

    // Arriba a la derecha: la edad, o «Mover a…» con el raton o el foco.
    let derecha = r.x + r.ancho - 16.0 * s;
    if e.se_reparte(li, fi) && (encima || foco) && e.repartiendo.is_none() {
        let rotulo = textos.t("tareas-mover");
        let tecla = foco.then_some("Intro");
        let bw = ui::ancho_de_boton(p, true, &rotulo, tecla, s);
        let b = RectF {
            x: r.x + r.ancho - 8.0 * s - bw,
            y: r.y + 8.0 * s,
            ancho: bw,
            alto: 40.0 * s,
        };
        ui::boton_v2(
            p,
            &mut e.botones,
            b,
            Accion::Repartir(li, fi),
            Some(&mi::FORWARD),
            &rotulo,
            tecla,
            Some(ui::v2::AZUL),
            ui::v2::BLANCO,
            s,
        );
    } else if let Some(t) = edad(&f, e.hoy, textos) {
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
                        p.rellenar(c, Color { a: 0.45, ..fondo });
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
                p.trazar(c, 2.0 * s, AZUL_V);
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
            hex(0xC7C7CC),
        );
    }
}

fn encoger(r: RectF, m: f32) -> RectF {
    RectF {
        x: r.x + m,
        y: r.y + m,
        ancho: (r.ancho - 2.0 * m).max(0.0),
        alto: (r.alto - 2.0 * m).max(0.0),
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
        let temporal = c.pegadas[0].ruta.clone();
        assert!(
            c.pegadas[0].temporal && temporal.is_file(),
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
        crate::ventanita::muestra("tareas-v3-sin-buscar", w, h, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos);
            let c = tarjetas::sitio_de(&e.colocadas, 0, 1).expect("la grieta");
            let (_, _, arriba) = alturas(1.0);
            e.botones.raton = (300.0, arriba - e.scroll + c.y + 20.0);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        // Buscando «fontan»: el fontanero del Inbox y el libro de la tesis,
        // con su chapita.
        e.botones.raton = (0.0, 0.0);
        e.buscar.poner("fontan");
        e.foco = Foco::Buscar;
        crate::ventanita::muestra("tareas-v3-buscando", w, h, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        e.buscar.poner("ayer pedir");
        crate::ventanita::muestra("tareas-v3-buscando-ayer", w, h, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        e.buscar.poner("ventanas");
        crate::ventanita::muestra("tareas-v3-sin-resultados", w, h, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        // Teclado: el foco en la tarjeta de la grieta, y luego su «Mover a…».
        e.buscar.poner("");
        e.foco = Foco::Tarjeta(0, 1);
        e.campo.escribir("regar las plantas");
        crate::ventanita::muestra("tareas-v3-foco", w, h, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        e.repartiendo = Some((0, 1));
        crate::ventanita::muestra("tareas-v3-mover", w, h, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        e.campo.vaciar();
        // Sin ninguna tarea.
        let mut v = Estado::nuevo();
        crate::ventanita::muestra("tareas-v3-vacia", w, h, |p, _| {
            pintar_todo(&mut v, p, marco, 1.0, &textos)
        });
    }
}
