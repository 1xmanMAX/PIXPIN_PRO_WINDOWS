//! La ventana de Tareas: arriba la caja para apuntar en «Mensajes
//! guardados», y debajo una tarjeta por lista con lo pendiente y, plegado,
//! lo hecho.
//!
//! Cada fila es como la del movil (`DeTareas` de `MiniActivity.kt`) y la del
//! panel del chat: la casilla, el texto (tachado y gris si esta hecha) y a la
//! derecha, pequeno y gris, cuantos dias lleva. **La casilla es lo unico que
//! tacha**, como alli. Lo que se acaba de tachar se queda unos segundos en su
//! sitio antes de irse a «Hechas»: un clic sin querer se deshace con otro en
//! el mismo sitio.
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

use super::{Fila, Lista};
use crate::caja_dibujo::hex;
use crate::overlay::Recursos;
use crate::ventanita::{APAGADO, Botones, CRISTAL, FONDO, TEXTO, centrado, dentro};

/// El acento: el azul del boton de enviar del chat (tema oscuro), el mismo
/// que lleva la casilla marcada del panel de tareas.
const ACENTO: Color = hex(0x5288c1);
/// Fondo de una fila o boton con el raton encima.
const ENCIMA: Color = hex(0x2e2e36);
/// El carril de la barra de avance.
const CARRIL: Color = hex(0x3a3a44);
/// La ficha `[img 01]` de una imagen pegada: letra azul clara sobre el
/// acento muy aguado, para que se vea que no es texto.
const FICHA_LETRA: Color = hex(0x9cc4f0);
const FICHA_FONDO: Color = Color { a: 0.24, ..ACENTO };
/// Lado de la miniatura de una imagen de tarea, en la fila.
const MINI: f32 = 28.0;
/// Lado mayor de la vista de una ficha con el raton encima.
const VISTA: f32 = 160.0;

const VK_RETROCESO: u32 = 0x08;
const VK_ENTRAR: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_FIN: u32 = 0x23;
const VK_INICIO: u32 = 0x24;
const VK_IZQUIERDA: u32 = 0x25;
const VK_DERECHA: u32 = 0x27;
const VK_SUPRIMIR: u32 = 0x2E;
const VK_V: u32 = 0x56;

/// Medidas, en pixeles logicos.
const CABECERA: f32 = 64.0;
const CAJA: f32 = 44.0;
const MARGEN: f32 = 16.0;
const RELLENO: f32 = 12.0;
const FILA: f32 = 36.0;
const PLIEGUE: f32 = 32.0;
const HUECO: f32 = 10.0;

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
            let hecho = Recursos::nuevos().and_then(|r| bucle(&r, &textos, &ubicacion, &aparato));
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
    /// La imagen `.2` de la tarea `.1` de la lista `.0`: sacarla a la
    /// pantalla como pin, como «Sacar a la pantalla» del chat.
    Imagen(usize, usize, usize),
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
    /// Los chats que se miraron, para la huella.
    proyectos: Vec<String>,
    firma: Vec<Option<(SystemTime, u64)>>,
    campo: Campo,
    /// Las listas con las hechas a la vista (por su clave). Empiezan todas
    /// plegadas.
    abiertas: HashSet<String>,
    /// Las recien tachadas (clave de la lista y texto crudo) y cuando: se
    /// quedan arriba hasta [`SE_QUEDA`].
    recien: HashMap<(String, String), Instant>,
    hoy: pixpin_proyecto::mini::Fecha,
    scroll: f32,
    alto_contenido: f32,
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
            proyectos: Vec::new(),
            firma: Vec::new(),
            campo: Campo::default(),
            abiertas: HashSet::new(),
            recien: HashMap::new(),
            hoy: super::hoy(),
            scroll: 0.0,
            alto_contenido: 0.0,
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
        self.listas = todas.into_iter().filter(|l| !l.filas.is_empty()).collect();
        // Releida, la tarea que se repartia puede estar en otro sitio.
        self.repartiendo = None;
        self.proyectos = proyectos;
        self.hoy = super::hoy();
    }

    fn decir(&mut self, t: String) {
        self.aviso = Some((t, Instant::now()));
    }

    fn sigue_arriba(&self, lista: &Lista, f: &Fila) -> bool {
        self.recien.contains_key(&(lista.clave(), f.crudo.clone()))
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
    let mut marco = centrado(monitor.area_trabajo, 620, 760, monitor.escala_por_cien);
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
                // Lo que se escribe va siempre a la caja de apuntar.
                EventoOverlay::Caracter(c) => {
                    e.campo.letra(c);
                }
                EventoOverlay::Tecla { vk: VK_ESCAPE, .. } if e.repartiendo.is_some() => {
                    e.repartiendo = None;
                }
                EventoOverlay::Tecla { vk: VK_ESCAPE, .. } => {
                    if e.campo.texto.is_empty() {
                        vivo = false;
                    } else {
                        e.campo.vaciar();
                    }
                }
                EventoOverlay::Tecla { vk: VK_ENTRAR, .. } => {
                    vivo = hacer(&mut e, Accion::Apuntar, textos, ubicacion, aparato);
                }
                EventoOverlay::Tecla { vk, ctrl, .. } => {
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
            let ruta = e
                .listas
                .get(li)
                .and_then(|l| Some((l, l.filas.get(fi)?)))
                .and_then(|(l, f)| e.ruta_de(l, f, k));
            match ruta {
                // Al `main`, como «Sacar a la pantalla» del chat: es quien
                // tiene los pines.
                Some(r) if pixpin_shell::mensajero::enviar_ficheros(std::slice::from_ref(&r)) => {
                    e.decir(textos.t("chat-pineado"));
                }
                Some(r) => {
                    tracing::warn!(ruta = %r.display(), "tareas: no contesta la ventana principal")
                }
                None => e.decir(textos.t("tareas-imagen-no-esta")),
            }
        }
        Accion::Repartir(li, fi) => e.repartiendo = Some((li, fi)),
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

fn pintar_todo(e: &mut Estado, p: &Pintor, marco: Rect, s: f32, textos: &Catalogo) {
    let (w, h) = (marco.ancho as f32, marco.alto as f32);
    p.limpiar(FONDO);
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
    let arriba = (CABECERA + CAJA + 12.0) * s;

    // El contenido, desplazable, ANTES que lo de arriba: asi la cabecera y
    // la caja se apuntan despues y le ganan a una tarjeta que asome por
    // debajo.
    let area = RectF {
        x: 0.0,
        y: arriba,
        ancho: w,
        alto: (h - arriba).max(0.0),
    };
    e.scroll = e.scroll.clamp(0.0, (e.alto_contenido - area.alto).max(0.0));
    let mut total = 0.0;
    p.con_recorte(area, |p| {
        let y0 = arriba - e.scroll;
        let fin = if e.listas.is_empty() {
            pintar_vacia(p, m, y0, ancho, s, textos)
        } else {
            pintar_listas(e, p, m, y0, ancho, area, s, textos)
        };
        total = fin - y0 + 20.0 * s;
    });
    e.alto_contenido = total;

    // Lo de arriba, tapando lo que se desplazo bajo ello.
    p.rellenar(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho: w,
            alto: arriba,
        },
        FONDO,
    );
    let cab = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: CABECERA * s,
    };
    e.botones.zona(cab, Accion::Mover);
    p.texto(&textos.t("tareas-titulo"), m, 10.0 * s, 20.0 * s, TEXTO);
    let pendientes: usize = e.listas.iter().map(Lista::cuantas_pendientes).sum();
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("pendientes", pendientes as i64);
    args.set("listas", e.listas.len() as i64);
    p.texto(
        &textos.t_args("tareas-resumen", &args),
        m,
        38.0 * s,
        12.5 * s,
        APAGADO,
    );
    let lado = 40.0 * s;
    let cerrar = RectF {
        x: w - m - lado,
        y: 12.0 * s,
        ancho: lado,
        alto: lado,
    };
    e.botones.boton(p, cerrar, Accion::Cerrar, "", None, s);
    p.icono(&mi::CLOSE, encoger(cerrar, 9.0 * s), TEXTO);

    pintar_caja(e, p, m, CABECERA * s, ancho, s, textos);

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

/// La caja para apuntar, siempre con el foco: lo que se escribe en esta
/// ventana va a ella, e Intro lo apunta.
fn pintar_caja(e: &mut Estado, p: &Pintor, x: f32, y: f32, ancho: f32, s: f32, textos: &Catalogo) {
    let caja = RectF {
        x,
        y,
        ancho,
        alto: CAJA * s,
    };
    p.rellenar_redondeado(caja, CAJA * s / 2.0, CRISTAL);
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
        ACENTO,
    );
    let tx = x + 16.0 * s + mw + 10.0 * s;
    // A la derecha, con algo escrito, el boton de apuntar para el raton.
    let mut fin = x + ancho - 18.0 * s;
    if !e.campo.texto.trim().is_empty() {
        let rotulo = textos.t("tareas-apuntar-boton");
        let (rw, _) = p.medir_texto(&rotulo, 13.5 * s);
        let b = RectF {
            x: x + ancho - rw - 28.0 * s - 6.0 * s,
            y: y + 6.0 * s,
            ancho: rw + 28.0 * s,
            alto: caja.alto - 12.0 * s,
        };
        let fondo = if dentro(b, e.botones.raton) {
            hex(0x6497cc)
        } else {
            ACENTO
        };
        p.rellenar_redondeado(b, b.alto / 2.0, fondo);
        let (_, rh) = p.medir_texto(&rotulo, 13.5 * s);
        p.texto(
            &rotulo,
            b.x + 14.0 * s,
            b.y + (b.alto - rh) / 2.0,
            13.5 * s,
            TEXTO,
        );
        e.botones.zona(b, Accion::Apuntar);
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
        p.texto_linea(&textos.t("tareas-apuntar"), tx, ty, tam, hueco, APAGADO);
        p.linea((tx, ty), (tx, ty + alto_linea), 1.5 * s, ACENTO);
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
                None => p.texto(&texto[r.clone()], x0, ty, tam, TEXTO),
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
        let c = tx - corrido + cx;
        p.linea((c, ty), (c, ty + alto_linea), 1.5 * s, ACENTO);
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
    p.rellenar_redondeado(marco, 10.0 * s, hex(0x24242c));
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
/// titulo y su chat).
fn pintar_eleccion(e: &mut Estado, p: &Pintor, w: f32, h: f32, s: f32, textos: &Catalogo) {
    let Some((li, fi)) = e.repartiendo else {
        return;
    };
    let tarea = e
        .listas
        .get(li)
        .and_then(|l| l.filas.get(fi))
        .map(|f| f.texto.clone())
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
    let cuantos = e.destinos.len().max(1);
    let alto = (cabeza + cuantos as f32 * renglon + pad).min(h - 80.0 * s);
    let caja = RectF {
        x: (w - ancho) / 2.0,
        y: ((h - alto) / 2.0).max(40.0 * s),
        ancho,
        alto,
    };
    p.rellenar_redondeado(caja, 14.0 * s, hex(0x24242c));
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
    if e.destinos.is_empty() {
        p.texto_ajustado(
            &textos.t("tareas-sin-grupos"),
            caja.x + pad,
            caja.y + cabeza,
            13.5 * s,
            caja.ancho - 2.0 * pad,
            APAGADO,
        );
        return;
    }
    let vista = RectF {
        y: caja.y + cabeza,
        alto: caja.alto - cabeza - pad / 2.0,
        ..caja
    };
    p.con_recorte(vista, |p| {
        for (di, d) in e.destinos.iter().enumerate() {
            let r = RectF {
                x: caja.x + 6.0 * s,
                y: vista.y + di as f32 * renglon,
                ancho: caja.ancho - 12.0 * s,
                alto: renglon,
            };
            if r.y > vista.y + vista.alto {
                break;
            }
            if dentro(r, e.botones.raton) {
                p.rellenar_redondeado(r, 8.0 * s, ENCIMA);
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
                APAGADO,
            );
        }
    });
    for di in 0..e.destinos.len() {
        let r = RectF {
            x: caja.x + 6.0 * s,
            y: vista.y + di as f32 * renglon,
            ancho: caja.ancho - 12.0 * s,
            alto: renglon,
        };
        if r.y + r.alto > vista.y + vista.alto + 1.0 {
            break;
        }
        e.botones.zona(r, Accion::Destino(di));
    }
}

fn pintar_vacia(p: &Pintor, x: f32, y: f32, ancho: f32, s: f32, textos: &Catalogo) -> f32 {
    let y = y + 60.0 * s;
    let t = textos.t("tareas-vacia");
    let anch = ancho.min(440.0 * s);
    let (_, th) = p.medir_texto_ajustado(&t, 15.0 * s, anch);
    p.texto_ajustado(&t, x + (ancho - anch) / 2.0, y, 15.0 * s, anch, APAGADO);
    y + th
}

/// Las tarjetas, una por lista. Solo se pinta lo que cae a la vista; lo
/// demas solo se cuenta, para saber hasta donde se desplaza.
#[allow(clippy::too_many_arguments)] // estado, pintor, sitio, ancho, vista, escala y textos
fn pintar_listas(
    e: &mut Estado,
    p: &Pintor,
    x: f32,
    y: f32,
    ancho: f32,
    vista: RectF,
    s: f32,
    textos: &Catalogo,
) -> f32 {
    let mut y = y;
    for li in 0..e.listas.len() {
        let lista = e.listas[li].clone();
        let (arriba, plegadas) = lista.a_la_vista(&|f| e.sigue_arriba(&lista, f));
        let abierta = e.abiertas.contains(&lista.clave());
        let cabecera = (RELLENO + 22.0 + 18.0 + 12.0) * s;
        let alto = cabecera
            + arriba.len() as f32 * FILA * s
            + if plegadas.is_empty() {
                0.0
            } else {
                PLIEGUE * s
            }
            + if abierta {
                plegadas.len() as f32 * FILA * s
            } else {
                0.0
            }
            + 8.0 * s;
        let caja = RectF { x, y, ancho, alto };
        if caja.y + caja.alto >= vista.y && caja.y <= vista.y + vista.alto {
            p.rellenar_redondeado(caja, 14.0 * s, CRISTAL);
            pintar_cabecera(p, &lista, caja, s, textos);
            let mut fy = y + cabecera;
            for f in &arriba {
                pintar_fila(e, p, li, f, x, fy, ancho, vista, s, textos);
                fy += FILA * s;
            }
            if !plegadas.is_empty() {
                let pliegue = RectF {
                    x: x + 4.0 * s,
                    y: fy,
                    ancho: ancho - 8.0 * s,
                    alto: PLIEGUE * s,
                };
                if dentro(pliegue, e.botones.raton) {
                    p.rellenar_redondeado(pliegue, 8.0 * s, ENCIMA);
                }
                let flecha = if abierta {
                    &mi::KEYBOARD_ARROW_UP
                } else {
                    &mi::KEYBOARD_ARROW_DOWN
                };
                p.icono(
                    flecha,
                    RectF {
                        x: x + RELLENO * s - 2.0 * s,
                        y: fy + (PLIEGUE * s - 22.0 * s) / 2.0,
                        ancho: 22.0 * s,
                        alto: 22.0 * s,
                    },
                    APAGADO,
                );
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("n", plegadas.len() as i64);
                let rotulo = textos.t_args("tareas-hechas", &args);
                let (_, rh) = p.medir_texto(&rotulo, 13.5 * s);
                p.texto(
                    &rotulo,
                    x + (RELLENO + 28.0) * s,
                    fy + (PLIEGUE * s - rh) / 2.0,
                    13.5 * s,
                    APAGADO,
                );
                e.botones.zona(pliegue, Accion::Plegar(li));
                fy += PLIEGUE * s;
                if abierta {
                    for f in &plegadas {
                        pintar_fila(e, p, li, f, x, fy, ancho, vista, s, textos);
                        fy += FILA * s;
                    }
                }
            }
        }
        y += alto + HUECO * s;
    }
    y
}

/// El titulo de la lista, su chat y cuantas van hechas, con su barra (como
/// la linea de avance del movil y del panel del chat).
fn pintar_cabecera(p: &Pintor, lista: &Lista, caja: RectF, s: f32, textos: &Catalogo) {
    let pad = RELLENO * s;
    let hechas = lista.cuantas_hechas();
    let de = lista.filas.len();
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("hechas", hechas as i64);
    args.set("de", de as i64);
    let avance = textos.t_args("mini-avance", &args);
    let (aw, _) = p.medir_texto(&avance, 12.5 * s);
    let y = caja.y + pad;
    p.texto(
        &avance,
        caja.x + caja.ancho - pad - aw,
        y + 3.0 * s,
        12.5 * s,
        APAGADO,
    );
    let interior = caja.ancho - 2.0 * pad - aw - 12.0 * s;
    p.texto_linea(&lista.titulo, caja.x + pad, y, 16.0 * s, interior, TEXTO);
    p.texto_linea(
        &lista.chat,
        caja.x + pad,
        y + 22.0 * s,
        12.5 * s,
        interior,
        APAGADO,
    );
    let barra = RectF {
        x: caja.x + pad,
        y: y + 44.0 * s,
        ancho: caja.ancho - 2.0 * pad,
        alto: 3.0 * s,
    };
    p.rellenar_redondeado(barra, 1.5 * s, CARRIL);
    if de > 0 && hechas > 0 {
        p.rellenar_redondeado(
            RectF {
                ancho: barra.ancho * hechas as f32 / de as f32,
                ..barra
            },
            1.5 * s,
            ACENTO,
        );
    }
}

/// Una tarea: la casilla (lo unico que tacha), su texto y cuantos dias
/// lleva.
#[allow(clippy::too_many_arguments)] // estado, pintor, lista, fila, sitio, vista, escala y textos
fn pintar_fila(
    e: &mut Estado,
    p: &Pintor,
    li: usize,
    f: &Fila,
    x: f32,
    y: f32,
    ancho: f32,
    vista: RectF,
    s: f32,
    textos: &Catalogo,
) {
    let fila = RectF {
        x,
        y,
        ancho,
        alto: FILA * s,
    };
    if fila.y + fila.alto < vista.y || fila.y > vista.y + vista.alto {
        return;
    }
    let pad = RELLENO * s;
    let lado = 20.0 * s;
    // La zona de la casilla, mas ancha que el dibujo: atinarle no puede
    // costar.
    let casilla = RectF {
        x: x + 4.0 * s,
        y,
        ancho: pad + lado + 6.0 * s,
        alto: fila.alto,
    };
    if dentro(casilla, e.botones.raton) {
        p.circulo(
            (x + pad + lado / 2.0, y + fila.alto / 2.0),
            lado * 0.8,
            ENCIMA,
        );
    }
    let icono = if f.hecha {
        &mi::CHECK_BOX
    } else {
        &mi::CHECK_BOX_OUTLINE_BLANK
    };
    p.icono(
        icono,
        RectF {
            x: x + pad,
            y: y + (fila.alto - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        if f.hecha { ACENTO } else { APAGADO },
    );
    // `filas` tiene todas las tareas en el orden del documento, asi que su
    // sitio en ella es su `indice`.
    e.botones.zona(casilla, Accion::Marcar(li, f.indice));

    let tam = 15.0 * s;
    let (_, alto_t) = p.medir_texto("Ag", tam);
    let ty = y + (fila.alto - alto_t) / 2.0;
    let mut derecha = x + ancho - pad;
    // En el Inbox, lo pendiente lleva su «Mover a…»: de ahi se reparte a
    // cada grupo (el usuario, 3-oct).
    let en_inbox = e.listas.get(li).is_some_and(super::es_inbox);
    if en_inbox && !f.hecha {
        let rotulo = textos.t("tareas-mover");
        let tam_b = 12.5 * s;
        let (rw, rh) = p.medir_texto(&rotulo, tam_b);
        let b = RectF {
            x: derecha - rw - 20.0 * s,
            y: y + (fila.alto - 26.0 * s) / 2.0,
            ancho: rw + 20.0 * s,
            alto: 26.0 * s,
        };
        let fondo = if dentro(b, e.botones.raton) {
            hex(0x6497cc)
        } else {
            ACENTO
        };
        p.rellenar_redondeado(b, b.alto / 2.0, fondo);
        p.texto(
            &rotulo,
            b.x + 10.0 * s,
            b.y + (b.alto - rh) / 2.0,
            tam_b,
            TEXTO,
        );
        e.botones.zona(b, Accion::Repartir(li, f.indice));
        derecha = b.x - 10.0 * s;
    }
    // Cuantos dias lleva, nunca la fecha (lo pidio el usuario): pequeno y
    // gris, que es un dato de contexto y no la tarea.
    if let Some(creada) = f.creada {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set(
            "dias",
            i64::from(pixpin_proyecto::mini::dias_desde(creada, e.hoy)),
        );
        let edad = textos.t_args("mini-tarea-edad", &args);
        let tam_e = 12.0 * s;
        let (ew, eh) = p.medir_texto(&edad, tam_e);
        p.texto(
            &edad,
            derecha - ew,
            y + (fila.alto - eh) / 2.0,
            tam_e,
            APAGADO,
        );
        derecha -= ew + 12.0 * s;
    }
    let tx = x + pad + lado + 12.0 * s;
    // Sus imagenes, en pequeno detras del texto: el texto cede su sitio.
    let mini = MINI * s;
    let paso = mini + 4.0 * s;
    let ancho_minis = if f.imagenes.is_empty() {
        0.0
    } else {
        f.imagenes.len() as f32 * paso + 6.0 * s
    };
    let hueco = (derecha - tx - ancho_minis).max(0.0);
    let color = if f.hecha { APAGADO } else { TEXTO };
    p.texto_linea(&f.texto, tx, ty, tam, hueco, color);
    let (tw, _) = if f.texto.is_empty() {
        (0.0, 0.0)
    } else {
        p.medir_texto(&f.texto, tam)
    };
    // Lo hecho se tacha, como en el movil (`TextDecoration.LineThrough`).
    if f.hecha && tw > 0.0 {
        let ly = ty + alto_t * 0.55;
        p.linea((tx, ly), (tx + tw.min(hueco), ly), 1.2 * s, APAGADO);
    }
    let Some(lista) = e.listas.get(li).cloned() else {
        return;
    };
    let mut mx = tx + tw.min(hueco) + if tw > 0.0 { 8.0 * s } else { 0.0 };
    for k in 0..f.imagenes.len() {
        let r = RectF {
            x: mx,
            y: y + (fila.alto - mini) / 2.0,
            ancho: mini,
            alto: mini,
        };
        match e
            .ruta_de(&lista, f, k)
            .as_deref()
            .and_then(|ruta| e.minis.ya(ruta))
        {
            Some((b, iw, ih)) => {
                crate::miniaturas::pintar_recortado(p, b, r, iw, ih);
                if f.hecha {
                    // Apagada como el texto tachado.
                    p.rellenar(r, Color { a: 0.45, ..FONDO });
                }
            }
            // Aun no llego del movil (o no se pudo leer): su hueco con el
            // dibujo de una imagen, para que se sepa que hay una.
            None => {
                p.rellenar_redondeado(r, 4.0 * s, ENCIMA);
                p.icono(&mi::IMAGE, encoger(r, 5.0 * s), APAGADO);
            }
        }
        if dentro(r, e.botones.raton) {
            p.trazar(r, 1.5 * s, ACENTO);
        }
        e.botones.zona(r, Accion::Imagen(li, f.indice, k));
        mx += paso;
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
                "- [ ] llamar al fontanero ➕ 2026-09-28\n- [ ] comprar pan ➕ 2026-10-03\n- [x] pagar la luz ➕ 2026-09-20\n- [ ] sin fecha",
            ),
            lista(
                "o",
                "Obra Miraflores",
                "Pendientes de obra",
                20,
                "- [ ] revisar puntales ➕ 2026-10-01\n- [x] pedir yeso\n- [x] pedir arena",
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
        e.destinos = vec![e.listas[1].clone(), lista("t", "Tesis", "Lecturas", 10, "")];
        e.abiertas.insert(e.listas[1].clave());
        e.campo.escribir("regar las plantas");
        e.campo
            .meter_imagen(foto("maceta.png", [60, 160, 90]), false, 0);
        e.campo.escribir("y abonar");
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

    #[test]
    #[ignore = "necesita GPU; ejecutar con --ignored y mirar el PNG"]
    fn muestra_de_la_ventana() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let mut e = muestra_estado();
        let marco = Rect {
            x: 0,
            y: 0,
            ancho: 620,
            alto: 760,
        };
        crate::ventanita::muestra("tareas", 620, 760, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        // Con el raton encima de la ficha pegada: su imagen debajo.
        let (chapa, _) = e.fichas_pintadas[0].clone();
        crate::ventanita::muestra("tareas-ficha", 620, 760, |p, motor| {
            cargar_minis(&mut e, motor);
            e.botones.raton = (chapa.x + chapa.ancho / 2.0, chapa.y + chapa.alto / 2.0);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        e.botones.raton = (0.0, 0.0);
        e.repartiendo = Some((0, 2));
        crate::ventanita::muestra("tareas-mover", 620, 760, |p, motor| {
            cargar_minis(&mut e, motor);
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
    }
}
