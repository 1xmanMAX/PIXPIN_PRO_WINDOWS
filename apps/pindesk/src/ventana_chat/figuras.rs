//! **La ventana de las figuras** (24-oct-2026, el usuario: «la
//! visualizacion de las figuras … que sea como una galeria donde yo pueda
//! ver las figuras asi realizadas agrupadas, de forma que sea todo intuitivo
//! y muy rapido de ver»; 25-oct: «las figuras deberian aparecer como una
//! ventana separada, y deberia haber como un previsualizador de las figuras
//! y de todo lo que tiene»).
//!
//! Se abre con su mosaico de la pestana Funciones del chat (hasta el
//! 25-oct-2026, la parte «Figuras» del interruptor de la barra), desde el
//! menu de la bandeja y desde «Figura» de la hoja del «+» de un chat. Una sola a la vez, en su hilo, como la galeria
//! de capturas:
//!
//! Desde el 2-nov-2026 va como la «Figuras» del iPhone (el usuario: «el
//! boton de dia/noche deberia ser mas como el del telefono, tambien en la
//! opcion Figuras»):
//!
//! - **La cabecera del movil**: «Cerrar» en una pastilla a la izquierda, el
//!   titulo y cuantas hay en medio y, a la derecha, el ☀/☾ redondo
//!   (`pindesk_ui::dia_noche`), cuyo icono gira al cambiar.
//! - **Debajo, las categorias en una fila de pastillas** («Todas», «Mis
//!   figuras» y las de serie; la categoria va en el nombre, «Diario ·
//!   Gratitud», ver `figuras_de_serie::categoria_de`), la elegida en azul.
//!   Si no caben, la rueda encima las corre de lado.
//! - **Cada grupo con su titulo y su cuenta** («Planificación 13») y sus
//!   figuras en rejilla, dibujadas de verdad (las mismas ordenes que la
//!   vista previa de un lienzo en el chat), con su nombre. Abajo, flotando,
//!   el buscador: se escribe y filtra.
//! - **A la derecha, el previsualizador** de la elegida: en grande, con su
//!   grupo, lo que mide, de que piezas esta hecha y los textos que lleva. Las
//!   del usuario se pueden borrar (dos clics).
//!
//! **Elegir para adjuntar** (25-oct-2026, el usuario: «adjuntar figura deberia
//! abrir directamente la ventana de figuras, yo selecciono, aparece el boton
//! de adjuntar y se adjunta al chat»): abierta desde el «+» de un chat, la
//! ventana sabe a que chat va (`abrir_para_adjuntar`). El previsualizador
//! lleva entonces, justo debajo del dibujo, el boton grande «Adjuntar a …»;
//! doble clic en una figura o Intro hacen lo mismo. La figura viaja al hilo
//! del chat por un buzon (`super::entregar_figura`), que la mete como un
//! lienzo con ella dentro, y la ventana se queda abierta con un
//! «Adjuntada ✓» por si se quiere otra.
//!
//! **De dia o de noche** (27-oct-2026, como el movil): el interruptor ☀/☾ de
//! la barra ensena las plantillas de dia o, en su mismo sitio, sus gemelas
//! «(noche)», nunca las dos mezcladas (`figuras_de_serie::segun_el_modo`).
//! Es el mismo que el de la ventana de figuras del lienzo y se recuerda
//! (`biblioteca_figuras::de_noche`).
//!
//! Teclado: escribir busca, las flechas recorren, Intro adjunta (eligiendo
//! para un chat), Escape borra lo buscado y, sin nada buscado, cierra.

use std::cell::{Cell, RefCell};
use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pindesk_geom::{Punto, Rect};
use pindesk_motor2d::biblioteca::{self, FiguraGuardada};
use pindesk_motor2d::figuras_de_serie::{
    CATEGORIAS, categoria_de, es_de_serie, pareja, segun_el_modo,
};
use pindesk_render::{Color, Pintor, RectF, Superficie};
use pindesk_shell::overlay::{EventoOverlay, VentanaOverlay};
use pindesk_store::{Catalogo, Idioma};

use super::{CLARO, LienzoVisto, OSCURO, Tema, con_alfa, encoger, mi, pintar_lienzo, rf};
use crate::overlay::Recursos;

/// Lo que mide una ficha de la rejilla (logicos): el dibujo y su nombre.
const CELDA_ANCHO: u32 = 156;
const CELDA_DIBUJO: u32 = 112;
const CELDA_NOMBRE: u32 = 30;
const HUECO: u32 = 12;
const MARGEN: u32 = 18;
/// La cabecera del movil: «Cerrar», el titulo con su cuenta y el ☀/☾.
const BARRA: u32 = 64;
/// Lo alto de «Cerrar» y de la fila de las categorias, y de cada una.
const PASTILLA: u32 = 32;
const FILA_CHIPS: u32 = 50;
const CHIP: u32 = 32;
/// El buscador flotante de abajo.
const BUSCADOR: u32 = 46;
const BUSCADOR_ANCHO: u32 = 440;
const PREVIA: u32 = 380;
const ALTO_TITULO: u32 = 36;
/// El grupo de lo que no lleva categoria: lo del usuario.
pub(super) const MIS_FIGURAS: &str = "";

/// Lo que mide el boton de adjuntar del previsualizador (logicos).
const ALTO_ADJUNTAR: u32 = 44;
/// Lo que se ve el «Adjuntada ✓» despues de adjuntar.
const DURA_AVISO: Duration = Duration::from_millis(1800);
/// Dos clics en la misma figura antes de esto son un doble clic.
const DOBLE_CLIC: Duration = Duration::from_millis(450);

const VK_ENTRAR: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_RETROCESO: u32 = 0x08;
const VK_IZQUIERDA: u32 = 0x25;
const VK_ARRIBA: u32 = 0x26;
const VK_DERECHA: u32 = 0x27;
const VK_ABAJO: u32 = 0x28;

/// La ventana abierta (su HWND), -1 mientras nace, 0 sin ventana.
static ABIERTA: AtomicIsize = AtomicIsize::new(0);

/// El chat al que se adjunta lo que se elija, si la ventana se abrio desde
/// el «+» de uno. Abierta desde la bandeja o el interruptor, ninguno.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Destino {
    /// El id del proyecto (el chat).
    pub proyecto: String,
    /// Su nombre, para el boton («Adjuntar a «Casa Lima»»).
    pub chat: String,
}

static DESTINO: Mutex<Option<Destino>> = Mutex::new(None);

fn destino() -> Option<Destino> {
    DESTINO.lock().ok().and_then(|g| g.clone())
}

fn poner_destino(d: Option<Destino>) {
    if let Ok(mut g) = DESTINO.lock() {
        *g = d;
    }
}

/// **Abre la ventana de las figuras**, o la trae delante si ya estaba, solo
/// para mirarlas: sin chat al que adjuntar.
pub(crate) fn abrir(idioma: Idioma) {
    poner_destino(None);
    lanzar(idioma);
}

/// **Abre la ventana de las figuras para adjuntar una al chat** `proyecto`
/// (el «+» → «Figura»). Si ya estaba abierta, cambia a ese chat y se trae
/// delante.
pub(super) fn abrir_para_adjuntar(idioma: Idioma, proyecto: String, chat: String) {
    poner_destino(Some(Destino { proyecto, chat }));
    lanzar(idioma);
}

fn lanzar(idioma: Idioma) {
    let ya = ABIERTA.load(Ordering::SeqCst);
    if ya > 0 {
        VentanaOverlay::restaurar_de_hwnd(windows::Win32::Foundation::HWND(ya as *mut _));
        // Despierta para que se pinte con su destino nuevo.
        pindesk_shell::overlay::despertar(ya);
        return;
    }
    if ya < 0 {
        return;
    }
    ABIERTA.store(-1, Ordering::SeqCst);
    let lanzado = std::thread::Builder::new()
        .name("figuras".into())
        .spawn(move || {
            let _com = pindesk_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(idioma);
            let noche = crate::biblioteca_figuras::de_noche();
            let mut g = Galeria {
                noche,
                boton: pindesk_ui::dia_noche::DiaNoche::nuevo(noche),
                ..Galeria::default()
            };
            g.leer();
            let hecho =
                crate::dispositivo_perdido::con_recursos("figuras", |r| bucle(r, &textos, &mut g));
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo abrir la ventana de las figuras");
            }
            ABIERTA.store(0, Ordering::SeqCst);
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de las figuras");
        ABIERTA.store(0, Ordering::SeqCst);
    }
}

// --- Lo de cada figura -------------------------------------------------------

/// De que esta hecha una figura, para el previsualizador.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Piezas {
    pub total: usize,
    pub trazos: usize,
    pub lineas: usize,
    pub formas: usize,
    pub textos: usize,
    pub otras: usize,
}

/// Una figura, ya lista para pintar.
struct Vista {
    id: String,
    /// El nombre sin la categoria delante.
    nombre: String,
    grupo: String,
    de_serie: bool,
    tamano: (f32, f32),
    piezas: Piezas,
    /// Los textos que lleva, en orden de lectura (arriba abajo).
    letras: Vec<String>,
    dibujo: Option<LienzoVisto>,
}

pub(super) fn piezas_de(f: &FiguraGuardada) -> Piezas {
    use pindesk_motor2d::Figura as F;
    let mut p = Piezas::default();
    for e in f.elementos.iter().filter(|e| !e.borrado) {
        p.total += 1;
        match &e.figura {
            F::Lapiz { .. } | F::Resaltador { .. } => p.trazos += 1,
            F::Linea { .. } | F::Flecha { .. } | F::Arco { .. } | F::Cota { .. } => p.lineas += 1,
            F::Rectangulo | F::Rombo | F::Elipse | F::Region { .. } => p.formas += 1,
            F::Texto { .. } => p.textos += 1,
            _ => p.otras += 1,
        }
    }
    p
}

pub(super) fn letras_de(f: &FiguraGuardada) -> Vec<String> {
    let mut v: Vec<(f32, f32, String)> = f
        .elementos
        .iter()
        .filter(|e| !e.borrado)
        .filter_map(|e| match &e.figura {
            pindesk_motor2d::Figura::Texto { texto, .. } if !texto.trim().is_empty() => {
                Some((e.y, e.x, texto.trim().replace('\n', " ")))
            }
            _ => None,
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    v.into_iter().map(|(_, _, t)| t).collect()
}

/// El orden de los grupos: lo del usuario, y las categorias de serie en el
/// orden del menu del lienzo; una categoria inventada por el usuario, detras.
fn orden_del_grupo(g: &str) -> usize {
    if g == MIS_FIGURAS {
        0
    } else {
        CATEGORIAS
            .iter()
            .position(|c| *c == g)
            .map_or(CATEGORIAS.len() + 1, |i| i + 1)
    }
}

fn vista_de(f: &FiguraGuardada) -> Vista {
    let (grupo, nombre) = match categoria_de(&f.nombre) {
        Some((c, n)) => (c.to_string(), n.to_string()),
        None => (MIS_FIGURAS.to_string(), f.nombre.clone()),
    };
    Vista {
        id: f.id.clone(),
        nombre,
        grupo,
        de_serie: es_de_serie(f),
        tamano: f.tamano(),
        piezas: piezas_de(f),
        letras: letras_de(f),
        dibujo: dibujo_de(f),
    }
}

/// La figura como la vista previa de un lienzo: sus ordenes y su caja.
fn dibujo_de(f: &FiguraGuardada) -> Option<LienzoVisto> {
    let mut escena = pindesk_motor2d::escena::Escena::nueva();
    for e in &f.elementos {
        escena.anadir(e.clone());
    }
    let caja = escena.caja()?;
    let lado = (caja.2 - caja.0).max(caja.3 - caja.1).max(1.0);
    let (ordenes, grafitos) = pindesk_motor2d::tinta::grafito::ordenes_con_grafito(
        &escena,
        (640.0 / lado).min(1.0),
        1 << 21,
    );
    let ordenes = crate::dibujo::tema::ordenes_como_en_el_lienzo(ordenes, escena.fondo);
    Some(LienzoVisto {
        ordenes,
        grafitos,
        caja,
        fondo: None,
        giro_del_fondo: 0.0,
        papel: None,
    })
}

/// El nombre de un grupo para ensenarlo.
fn rotulo_del_grupo(g: &str, textos: &Catalogo) -> String {
    if g == MIS_FIGURAS {
        textos.t("figuras-mias")
    } else {
        g.to_string()
    }
}

/// Si la figura casa con lo buscado (sin mayusculas ni tildes, en su
/// nombre, su grupo o sus textos).
fn casa(f: &Vista, consulta: &str) -> bool {
    let q = plegar(consulta.trim());
    q.is_empty()
        || plegar(&f.nombre).contains(&q)
        || plegar(&f.grupo).contains(&q)
        || f.letras.iter().any(|l| plegar(l).contains(&q))
}

pub(super) fn plegar(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            otra => otra,
        })
        .collect()
}

// --- El estado ---------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum Zona {
    Grupo(Option<String>),
    Figura(usize),
    Borrar,
    /// El boton grande «Adjuntar a …» (solo eligiendo para un chat).
    Adjuntar,
    /// El boton ☀/☾ redondo de la cabecera.
    DiaNoche,
    Cerrar,
    /// El buscador flotante: lo escrito va siempre a el, pulsarlo no hace
    /// nada (pero no deja pasar el clic a la figura de debajo).
    Buscador,
    Barra,
}

struct Galeria {
    figuras: Vec<Vista>,
    /// El interruptor ☀/☾: de noche, las gemelas «(noche)».
    noche: bool,
    /// El boton redondo que lo dice, con el giro de su icono al cambiar.
    boton: pindesk_ui::dia_noche::DiaNoche,
    /// Lo corrida que va la fila de las categorias (pixeles) y lo mas que
    /// se puede correr (lo mide el pintor).
    scroll_chips: i32,
    ancho_chips: Cell<i32>,
    /// Las del modo puesto, en su orden (`segun_el_modo`).
    modo: Vec<usize>,
    /// El grupo elegido a la izquierda; `None`, todas.
    filtro: Option<String>,
    consulta: String,
    scroll: i32,
    elegida: Option<usize>,
    borrar_pedido: bool,
    raton: Punto,
    zonas: RefCell<Vec<(Rect, Zona)>>,
    alto_total: Cell<i32>,
    /// Las que se ven, en el orden en que se pintan (para las flechas).
    visibles: RefCell<Vec<usize>>,
    columnas: Cell<usize>,
    /// El chat al que va lo que se adjunte (copia de `DESTINO` de esta vuelta).
    destino: Option<Destino>,
    /// El ultimo clic en una figura, para el doble clic.
    ultimo_clic: Option<(usize, Instant)>,
    /// «Adjuntada a … ✓», mientras se ve.
    aviso: Option<(String, Instant)>,
}

impl Default for Galeria {
    fn default() -> Self {
        Galeria {
            figuras: Vec::new(),
            noche: false,
            boton: pindesk_ui::dia_noche::DiaNoche::nuevo(false),
            scroll_chips: 0,
            ancho_chips: Cell::new(0),
            modo: Vec::new(),
            filtro: None,
            consulta: String::new(),
            scroll: 0,
            elegida: None,
            borrar_pedido: false,
            raton: Punto { x: -1, y: -1 },
            zonas: RefCell::new(Vec::new()),
            alto_total: Cell::new(0),
            visibles: RefCell::new(Vec::new()),
            columnas: Cell::new(1),
            destino: None,
            ultimo_clic: None,
            aviso: None,
        }
    }
}

impl Galeria {
    fn leer(&mut self) {
        let elegida = self
            .elegida
            .and_then(|i| self.figuras.get(i))
            .map(|f| f.id.clone());
        let figuras = biblioteca::cargar(&crate::biblioteca_figuras::ruta());
        self.poner_figuras(&figuras, elegida);
    }

    /// Las figuras que se ensenan (ya leidas), conservando la elegida si
    /// sigue.
    fn poner_figuras(&mut self, figuras: &[FiguraGuardada], elegida: Option<String>) {
        self.figuras = figuras.iter().map(vista_de).collect();
        // Las del usuario primero y despues las categorias en su orden.
        self.figuras.sort_by_key(|f| orden_del_grupo(&f.grupo));
        self.modo = segun_el_modo(&self.pares(), self.noche);
        if let Some(g) = &self.filtro
            && !self.modo.iter().any(|i| &self.figuras[*i].grupo == g)
        {
            self.filtro = None;
        }
        self.elegida = elegida
            .and_then(|id| self.figuras.iter().position(|f| f.id == id))
            .filter(|i| self.modo.contains(i))
            .or(self.modo.first().copied());
    }

    /// `(id, nombre)` de cada una, para `segun_el_modo` y `pareja`.
    fn pares(&self) -> Vec<(&str, &str)> {
        self.figuras
            .iter()
            .map(|f| (f.id.as_str(), f.nombre.as_str()))
            .collect()
    }

    /// **El interruptor ☀/☾**: cambia de modo y la elegida pasa a su pareja
    /// (el diario de dia, a su gemela de noche), si la tiene.
    fn poner_noche(&mut self, noche: bool) {
        if noche == self.noche {
            return;
        }
        self.noche = noche;
        self.boton.poner(noche, crate::boton_dia_noche::ahora_ms());
        self.modo = segun_el_modo(&self.pares(), noche);
        let pareja = self.elegida.and_then(|i| pareja(&self.pares(), i));
        self.elegida = pareja
            .or(self.elegida)
            .filter(|i| self.modo.contains(i))
            .or(self.modo.first().copied());
        if let Some(g) = &self.filtro
            && !self.modo.iter().any(|i| &self.figuras[*i].grupo == g)
        {
            self.filtro = None;
        }
        self.scroll = 0;
    }

    /// Los grupos que hay, en orden, con cuantas figuras del modo tiene cada
    /// uno.
    fn grupos(&self) -> Vec<(String, usize)> {
        let mut v: Vec<(String, usize)> = Vec::new();
        for f in self.modo.iter().map(|i| &self.figuras[*i]) {
            match v.iter_mut().find(|(g, _)| *g == f.grupo) {
                Some((_, n)) => *n += 1,
                None => v.push((f.grupo.clone(), 1)),
            }
        }
        v
    }

    fn zona_en(&self, l: Punto) -> Option<Zona> {
        self.zonas
            .borrow()
            .iter()
            .rev()
            .find(|(r, _)| r.contiene(l))
            .map(|(_, z)| z.clone())
    }

    /// Lo que hace un clic: elegir, y con un chat de destino, adjuntar (doble
    /// clic en una figura o el boton «Adjuntar»). Devuelve la zona pulsada.
    fn pulsar(&mut self, l: Punto, vivo: &mut bool, textos: &Catalogo) -> Option<Zona> {
        let zona = self.zona_en(l);
        if zona != Some(Zona::Borrar) {
            self.borrar_pedido = false;
        }
        match &zona {
            Some(Zona::Grupo(g)) => {
                self.filtro = g.clone();
                self.scroll = 0;
            }
            Some(Zona::Buscador) => {}
            Some(Zona::Figura(i)) => {
                self.elegida = Some(*i);
                let doble = self
                    .ultimo_clic
                    .is_some_and(|(j, t)| j == *i && t.elapsed() <= DOBLE_CLIC);
                self.ultimo_clic = Some((*i, Instant::now()));
                if doble {
                    self.ultimo_clic = None;
                    self.adjuntar(textos);
                }
            }
            Some(Zona::Adjuntar) => {
                self.adjuntar(textos);
            }
            Some(Zona::Borrar) => {
                if self.borrar_pedido {
                    self.borrar();
                } else {
                    self.borrar_pedido = true;
                }
            }
            Some(Zona::DiaNoche) => {
                let noche = !self.noche;
                self.poner_noche(noche);
                crate::biblioteca_figuras::poner_de_noche(noche);
            }
            Some(Zona::Cerrar) => *vivo = false,
            _ => {}
        }
        zona
    }

    /// **Adjunta la elegida** al chat del destino, si lo hay: va por el buzon
    /// del chat y aqui solo queda el aviso. Sin destino no hace nada (la
    /// ventana se abrio para mirar).
    fn adjuntar(&mut self, textos: &Catalogo) -> bool {
        let (Some(d), Some(f)) = (
            self.destino.as_ref(),
            self.elegida.and_then(|i| self.figuras.get(i)),
        ) else {
            return false;
        };
        super::entregar_figura(d.proyecto.clone(), f.id.clone());
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("chat", d.chat.clone());
        self.aviso = Some((textos.t_args("figuras-adjuntada", &args), Instant::now()));
        true
    }

    /// Las flechas: a la de al lado o a la de arriba/abajo en la rejilla.
    fn mover_eleccion(&mut self, dx: i32, dy: i32) {
        let v = self.visibles.borrow().clone();
        if v.is_empty() {
            return;
        }
        let actual = self
            .elegida
            .and_then(|e| v.iter().position(|i| *i == e))
            .unwrap_or(0) as i32;
        let paso = dx + dy * self.columnas.get().max(1) as i32;
        let nueva = (actual + paso).clamp(0, v.len() as i32 - 1);
        self.elegida = Some(v[nueva as usize]);
    }

    /// Quita de la biblioteca la elegida (solo las del usuario).
    fn borrar(&mut self) {
        self.borrar_pedido = false;
        let Some(f) = self
            .elegida
            .and_then(|i| self.figuras.get(i))
            .filter(|f| !f.de_serie)
        else {
            return;
        };
        let ruta = crate::biblioteca_figuras::ruta();
        let mut todas = biblioteca::cargar(&ruta);
        let antes = todas.len();
        todas.retain(|t| t.id != f.id);
        if todas.len() != antes && crate::biblioteca_figuras::guardar(&ruta, &todas) {
            tracing::info!(nombre = %f.nombre, "figura borrada desde la ventana de figuras");
        }
        self.elegida = None;
        self.leer();
    }
}

// --- La ventana ----------------------------------------------------------------

fn bucle(recursos: &Recursos, textos: &Catalogo, g: &mut Galeria) -> Result<()> {
    let monitores = pindesk_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .monitor_en(pindesk_shell::posicion_del_cursor())
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien;
    let mut marco =
        crate::ventanita::centrado(monitor.area_trabajo, 1320, 840, monitor.escala_por_cien);
    let mut ventana = VentanaOverlay::nueva_normal(marco, &textos.t("figuras-titulo"))
        .context("no se pudo abrir la ventana de las figuras")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para las figuras")?;
    ventana.mostrar();
    ventana.enfocar();
    ABIERTA.store(ventana.handle().0 as isize, Ordering::SeqCst);
    tracing::info!(figuras = g.figuras.len(), "ventana de las figuras abierta");

    let mut moviendo: Option<(Punto, Rect)> = None;
    let mut vivo = true;
    let mut pintar = true;
    let mut leida = std::fs::metadata(crate::biblioteca_figuras::ruta())
        .and_then(|m| m.modified())
        .ok();
    let mut mirado = std::time::Instant::now();
    while vivo {
        pindesk_shell::overlay::bombear_pendientes();
        // Donde esta la ventana AHORA: si otro la movio (AltSnap, Win+flecha),
        // los clics se reparten desde ahi y no desde donde se abrio
        // (docs/ventanas.md).
        ventana.seguir_sitio(&mut marco);
        // A que chat va lo que se adjunte: puede cambiar con la ventana
        // abierta (otro «+» → «Figura» desde otro chat).
        let d = destino();
        if d != g.destino {
            g.destino = d;
            pintar = true;
        }
        for (hw, ev) in pindesk_shell::overlay::tomar_eventos_pendientes() {
            if hw != ventana.handle() {
                continue;
            }
            let local = |p: Punto, m: Rect| Punto {
                x: p.x - m.x,
                y: p.y - m.y,
            };
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
                    if g.zona_en(l) != g.zona_en(g.raton) {
                        pintar = true;
                    }
                    g.raton = l;
                }
                EventoOverlay::BotonPulsado(p) => {
                    let l = local(p, marco);
                    g.raton = l;
                    if g.pulsar(l, &mut vivo, textos) == Some(Zona::Barra) {
                        moviendo = Some((p, marco));
                        ventana.capturar_raton();
                    }
                    pintar = true;
                }
                EventoOverlay::BotonSoltado(_) => {
                    if moviendo.take().is_some() {
                        ventana.soltar_raton();
                    }
                }
                // La rueda encima de las categorias las corre de lado.
                EventoOverlay::Rueda(delta)
                    if (BARRA * escala / 100) as i32 <= g.raton.y
                        && g.raton.y < ((BARRA + FILA_CHIPS) * escala / 100) as i32 =>
                {
                    let nuevo = (g.scroll_chips - delta * (CHIP * escala / 100) as i32 / 120 * 2)
                        .clamp(0, g.ancho_chips.get().max(0));
                    if nuevo != g.scroll_chips {
                        g.scroll_chips = nuevo;
                        pintar = true;
                    }
                }
                EventoOverlay::Rueda(delta) => {
                    let paso = (CELDA_DIBUJO * escala / 100) as i32;
                    let nuevo = (g.scroll - delta * paso / 120 * 6 / 10)
                        .clamp(0, g.alto_total.get().max(0));
                    if nuevo != g.scroll {
                        g.scroll = nuevo;
                        pintar = true;
                    }
                }
                EventoOverlay::Tecla { vk, .. } => {
                    pintar = true;
                    match vk {
                        VK_ENTRAR => {
                            g.adjuntar(textos);
                        }
                        VK_ESCAPE if !g.consulta.is_empty() => g.consulta.clear(),
                        VK_ESCAPE => vivo = false,
                        VK_RETROCESO => {
                            g.consulta.pop();
                        }
                        VK_IZQUIERDA => g.mover_eleccion(-1, 0),
                        VK_DERECHA => g.mover_eleccion(1, 0),
                        VK_ARRIBA => g.mover_eleccion(0, -1),
                        VK_ABAJO => g.mover_eleccion(0, 1),
                        _ => pintar = false,
                    }
                }
                EventoOverlay::Caracter(c) if !c.is_control() => {
                    if g.consulta.chars().count() < 40 {
                        g.consulta.push(c);
                        g.scroll = 0;
                    }
                    pintar = true;
                }
                _ => {}
            }
        }
        if !vivo {
            break;
        }
        // El «Adjuntada ✓» se va solo.
        if g.aviso
            .as_ref()
            .is_some_and(|(_, t)| t.elapsed() >= DURA_AVISO)
        {
            g.aviso = None;
            pintar = true;
        }
        // La biblioteca cambio por fuera (una figura guardada en un lienzo,
        // llegada del movil): se relee.
        if mirado.elapsed() >= std::time::Duration::from_secs(2) {
            mirado = std::time::Instant::now();
            let ahora = std::fs::metadata(crate::biblioteca_figuras::ruta())
                .and_then(|m| m.modified())
                .ok();
            if ahora != leida {
                leida = ahora;
                g.leer();
                pintar = true;
            }
            // El ☀/☾ se cambio en la ventana de figuras del lienzo.
            let noche = crate::biblioteca_figuras::de_noche();
            if noche != g.noche {
                g.poner_noche(noche);
                pintar = true;
            }
        }
        // Mientras gira el icono del ☀/☾, un fotograma tras otro.
        let girando = g.boton.animando(crate::boton_dia_noche::ahora_ms());
        if girando {
            pintar = true;
        }
        if pintar {
            pintar = false;
            let tema = if pindesk_shell::entorno::tema_claro() {
                &CLARO
            } else {
                &OSCURO
            };
            if let Ok(d) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&d, |p: &Pintor| {
                    pintar_todo(p, g, marco.ancho, marco.alto, tema, escala, textos)
                });
                let _ = superficie.presentar();
            }
        }
        // Con el aviso a la vista se despierta antes, para quitarlo a tiempo.
        let espera = if girando {
            16
        } else if g.aviso.is_some() {
            200
        } else {
            1_000
        };
        pindesk_shell::overlay::esperar_eventos(Some(espera));
    }
    poner_destino(None);
    ventana.ocultar();
    tracing::info!("ventana de las figuras cerrada");
    Ok(())
}

// --- Pintar ----------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn pintar_todo(
    p: &Pintor,
    g: &Galeria,
    ancho: u32,
    alto: u32,
    tema: &Tema,
    escala: u32,
    textos: &Catalogo,
) {
    let e = escala as f32 / 100.0;
    let ei = |x: u32| x * escala / 100;
    let mut zonas = g.zonas.borrow_mut();
    zonas.clear();
    let todo = Rect {
        x: 0,
        y: 0,
        ancho,
        alto,
    };
    let claro = es_claro(tema);
    p.rellenar(rf(todo), tema.lista);

    // --- La cabecera del movil: «Cerrar» a la izquierda, el titulo con su
    // cuenta en medio y el ☀/☾ redondo a la derecha. Lo que no es boton la
    // arrastra.
    let barra = Rect {
        alto: ei(BARRA),
        ..todo
    };
    zonas.push((barra, Zona::Barra));
    let tam = 13.5 * e;
    let gris = if claro {
        crate::caja_dibujo::hex(0xf2f2f7)
    } else {
        tema.buscador
    };
    let gris_sobre = if claro {
        crate::caja_dibujo::hex(0xe5e5ea)
    } else {
        tema.fila_sobre
    };
    let rotulo_cerrar = textos.t("pin-cerrar");
    let (cw, ch) = p.medir_texto(&rotulo_cerrar, tam);
    let alto_pastilla = ei(PASTILLA);
    let cerrar = Rect {
        x: ei(MARGEN) as i32,
        y: (barra.alto as i32 - alto_pastilla as i32) / 2,
        ancho: (cw + 32.0 * e) as u32,
        alto: alto_pastilla,
    };
    p.rellenar_redondeado(
        rf(cerrar),
        cerrar.alto as f32 / 2.0,
        if cerrar.contiene(g.raton) {
            gris_sobre
        } else {
            gris
        },
    );
    p.texto(
        &rotulo_cerrar,
        cerrar.x as f32 + (cerrar.ancho as f32 - cw) / 2.0,
        cerrar.y as f32 + (cerrar.alto as f32 - ch) / 2.0,
        tam,
        tema.texto,
    );
    zonas.push((cerrar, Zona::Cerrar));

    let centro_y = barra.alto as i32 / 2;
    let modo = pindesk_ui::dia_noche::caja(ancho as i32 - ei(MARGEN) as i32, centro_y, escala);
    crate::boton_dia_noche::pintar(
        p,
        rf(modo),
        g.boton.paso(crate::boton_dia_noche::ahora_ms()),
        modo.contiene(g.raton),
        &crate::boton_dia_noche::Colores::de(claro),
    );
    zonas.push((modo, Zona::DiaNoche));

    // Eligiendo para un chat, el titulo dice para cual.
    let titulo = match &g.destino {
        Some(d) => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("chat", d.chat.clone());
            textos.t_args("figuras-elegir-para", &args)
        }
        None => textos.t("figuras-titulo"),
    };
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("n", g.modo.len());
    let subtitulo = textos.t_args("figuras-ventana-subtitulo", &args);
    // El titulo va centrado en la ventana, sin pisar los botones.
    let libre = (ancho as f32 / 2.0 - cerrar.derecha() as f32 - 16.0 * e)
        .min(ancho as f32 / 2.0 - (ancho as i32 - modo.x) as f32 - 16.0 * e)
        .max(40.0 * e)
        * 2.0;
    let tam_t = 17.0 * e;
    let (tw, th) = crate::lecciones::ui::medir_negrita(p, &titulo, tam_t, libre);
    let tw = tw.min(libre);
    let (sw, sh) = p.medir_texto(&subtitulo, 12.0 * e);
    let sw = sw.min(libre);
    let y0 = (barra.alto as f32 - th - sh - 2.0 * e) / 2.0;
    crate::lecciones::ui::negrita(
        p,
        &titulo,
        (ancho as f32 - tw) / 2.0,
        y0,
        tam_t,
        libre,
        tema.texto,
    );
    p.texto_linea(
        &subtitulo,
        (ancho as f32 - sw) / 2.0,
        y0 + th + 2.0 * e,
        12.0 * e,
        libre,
        tema.apagado,
    );

    // --- Las categorias en una fila de pastillas que se corre con la rueda:
    // «Todas» y cada grupo; la elegida en azul lleno.
    let fila_chips = Rect {
        x: 0,
        y: barra.abajo(),
        ancho,
        alto: ei(FILA_CHIPS),
    };
    let grupos = g.grupos();
    let mut chips: Vec<(Option<String>, String)> = vec![(None, textos.t("figuras-todas"))];
    chips.extend(
        grupos
            .iter()
            .map(|(gr, _)| (Some(gr.clone()), rotulo_del_grupo(gr, textos))),
    );
    p.empujar_recorte(rf(fila_chips));
    let azul = crate::caja_dibujo::hex(0x0a84ff);
    let mut x = ei(MARGEN) as i32 - g.scroll_chips;
    let alto_chip = ei(CHIP);
    let y_chip = fila_chips.y + (fila_chips.alto as i32 - alto_chip as i32) / 2;
    for (gr, rotulo) in chips {
        let (w, h) = p.medir_texto(&rotulo, tam);
        let chip = Rect {
            x,
            y: y_chip,
            ancho: (w + 30.0 * e) as u32,
            alto: alto_chip,
        };
        let elegido = g.filtro == gr;
        let fondo = if elegido {
            azul
        } else if chip.contiene(g.raton) {
            gris_sobre
        } else {
            gris
        };
        p.rellenar_redondeado(rf(chip), chip.alto as f32 / 2.0, fondo);
        p.texto(
            &rotulo,
            chip.x as f32 + 15.0 * e,
            chip.y as f32 + (chip.alto as f32 - h) / 2.0,
            tam,
            if elegido { Color::BLANCO } else { tema.texto },
        );
        if let Some(vista) = chip.interseccion(fila_chips) {
            zonas.push((vista, Zona::Grupo(gr)));
        }
        x = chip.derecha() + ei(8) as i32;
    }
    p.soltar_recorte();
    g.ancho_chips
        .set((x + g.scroll_chips + ei(MARGEN) as i32 - ancho as i32).max(0));

    // --- La rejilla, por grupos, y el previsualizador a la derecha.
    let arriba = fila_chips.abajo();
    let cuerpo_alto = (alto as i32 - arriba).max(0) as u32;
    let previa_ancho = ei(PREVIA).min(ancho / 2);
    let rejilla = Rect {
        x: 0,
        y: arriba,
        ancho: ancho.saturating_sub(previa_ancho),
        alto: cuerpo_alto,
    };
    let margen = ei(MARGEN) as i32;
    let ancho_util = rejilla.ancho.saturating_sub(2 * ei(MARGEN));
    let paso = ei(CELDA_ANCHO + HUECO);
    let columnas = ((ancho_util + ei(HUECO)) / paso.max(1)).max(1) as usize;
    g.columnas.set(columnas);
    // Las fichas se estiran para llenar la fila, como en el movil.
    let ancho_celda = (ancho_util.saturating_sub((columnas as u32 - 1) * ei(HUECO))
        / columnas as u32)
        .max(1);
    let alto_dibujo = ancho_celda * CELDA_DIBUJO / CELDA_ANCHO;
    let alto_celda = alto_dibujo + ei(CELDA_NOMBRE);
    let mut visibles = Vec::new();
    p.empujar_recorte(rf(rejilla));
    let mut y = rejilla.y - g.scroll + ei(4) as i32;
    for (gr, _) in grupos
        .iter()
        .filter(|(gr, _)| g.filtro.as_ref().is_none_or(|f| f == gr))
    {
        let de_este: Vec<usize> = g
            .modo
            .iter()
            .copied()
            .filter(|i| &g.figuras[*i].grupo == gr && casa(&g.figuras[*i], &g.consulta))
            .collect();
        if de_este.is_empty() {
            continue;
        }
        // «Planificación 13»: el nombre en negrita y la cuenta en gris.
        y += ei(6) as i32;
        let nombre = rotulo_del_grupo(gr, textos);
        let tam_g = 17.0 * e;
        let (gw, gh) = crate::lecciones::ui::medir_negrita(p, &nombre, tam_g, ancho_util as f32);
        crate::lecciones::ui::negrita(
            p,
            &nombre,
            (rejilla.x + margen) as f32,
            y as f32 + 4.0 * e,
            tam_g,
            ancho_util as f32,
            tema.texto,
        );
        let cuenta = de_este.len().to_string();
        let (_, nh) = p.medir_texto(&cuenta, 14.0 * e);
        p.texto(
            &cuenta,
            (rejilla.x + margen) as f32 + gw + 8.0 * e,
            y as f32 + 4.0 * e + (gh - nh).max(0.0) / 2.0 + 1.0 * e,
            14.0 * e,
            tema.apagado,
        );
        y += ei(ALTO_TITULO) as i32;
        for (k, i) in de_este.iter().enumerate() {
            let (col, fila) = (k % columnas, k / columnas);
            let celda = Rect {
                x: rejilla.x + margen + (col as u32 * (ancho_celda + ei(HUECO))) as i32,
                y: y + (fila as u32 * (alto_celda + ei(HUECO))) as i32,
                ancho: ancho_celda,
                alto: alto_celda,
            };
            visibles.push(*i);
            if celda.abajo() < rejilla.y || celda.y > rejilla.abajo() {
                continue;
            }
            pintar_celda(
                p,
                &g.figuras[*i],
                celda,
                alto_dibujo,
                tema,
                e,
                celda.contiene(g.raton),
                g.elegida == Some(*i),
            );
            if let Some(vista) = celda.interseccion(rejilla) {
                zonas.push((vista, Zona::Figura(*i)));
            }
        }
        let filas = de_este.len().div_ceil(columnas) as u32;
        y += (filas * (alto_celda + ei(HUECO))) as i32;
    }
    if visibles.is_empty() {
        let vacio = if g.figuras.is_empty() {
            textos.t("figuras-vacia")
        } else {
            textos.t("figuras-sin-resultados")
        };
        let (w, _) = p.medir_texto_ajustado(&vacio, 14.0 * e, rejilla.ancho as f32 - 40.0 * e);
        p.texto_ajustado(
            &vacio,
            rejilla.x as f32 + (rejilla.ancho as f32 - w) / 2.0,
            rejilla.y as f32 + 60.0 * e,
            14.0 * e,
            rejilla.ancho as f32 - 40.0 * e,
            tema.apagado,
        );
    }
    p.soltar_recorte();
    *g.visibles.borrow_mut() = visibles;
    // Al final sobra lo que tapa el buscador flotante, para que la ultima
    // fila se pueda ver entera.
    g.alto_total.set(
        (y + g.scroll - rejilla.y - rejilla.alto as i32 + ei(MARGEN + BUSCADOR + 24) as i32)
            .max(0),
    );

    // --- El buscador flotante, abajo en medio de la rejilla (lo que se
    // escribe va siempre a el).
    let ancho_b = ei(BUSCADOR_ANCHO).min(rejilla.ancho.saturating_sub(2 * ei(MARGEN)));
    let buscador = Rect {
        x: rejilla.x + (rejilla.ancho as i32 - ancho_b as i32) / 2,
        y: rejilla.abajo() - ei(BUSCADOR + 18) as i32,
        ancho: ancho_b,
        alto: ei(BUSCADOR),
    };
    let b = rf(buscador);
    let radio = b.alto / 2.0;
    p.rellenar_redondeado(
        RectF {
            y: b.y + 5.0 * e,
            ..encoger(b, -3.0 * e)
        },
        radio + 3.0 * e,
        con_alfa(Color::NEGRO, if claro { 0.10 } else { 0.35 }),
    );
    p.rellenar_redondeado(b, radio, tema.pildora_borde);
    p.rellenar_redondeado(
        encoger(b, 1.0),
        radio - 1.0,
        if claro {
            tema.cabecera
        } else {
            tema.barra
        },
    );
    let lupa = 18.0 * e;
    p.icono(
        &mi::SEARCH,
        RectF {
            x: b.x + 16.0 * e,
            y: b.y + (b.alto - lupa) / 2.0,
            ancho: lupa,
            alto: lupa,
        },
        tema.apagado,
    );
    let (buscado, color) = if g.consulta.is_empty() {
        (textos.t("figuras-buscar"), tema.apagado)
    } else {
        (format!("{}▏", g.consulta), tema.texto)
    };
    let (_, bh) = p.medir_texto(&buscado, 14.0 * e);
    p.texto_linea(
        &buscado,
        b.x + 44.0 * e,
        b.y + (b.alto - bh) / 2.0,
        14.0 * e,
        b.ancho - 60.0 * e,
        color,
    );
    zonas.push((buscador, Zona::Buscador));

    // El previsualizador, a la derecha, con su raya.
    let previa = Rect {
        x: rejilla.derecha(),
        y: arriba,
        ancho: previa_ancho,
        alto: cuerpo_alto,
    };
    p.rellenar(rf(previa), tema.barra);
    p.rellenar(
        RectF {
            x: previa.x as f32,
            y: previa.y as f32,
            ancho: e.max(1.0),
            alto: previa.alto as f32,
        },
        con_alfa(tema.texto, 0.08),
    );
    // La raya de debajo de las categorias.
    p.rellenar(
        RectF {
            x: 0.0,
            y: arriba as f32,
            ancho: ancho as f32,
            alto: e.max(1.0),
        },
        con_alfa(tema.texto, 0.08),
    );
    if let Some(f) = g.elegida.and_then(|i| g.figuras.get(i)) {
        pintar_previa(p, f, previa, tema, e, textos, g, &mut zonas);
    }

    // «Adjuntada a … ✓», una pastilla encima del buscador.
    if let Some((texto, _)) = &g.aviso {
        let tam = 14.0 * e;
        let (w, h) = p.medir_texto(texto, tam);
        let pastilla = RectF {
            x: rejilla.x as f32 + (rejilla.ancho as f32 - w - 36.0 * e) / 2.0,
            y: buscador.y as f32 - h - 32.0 * e,
            ancho: w + 36.0 * e,
            alto: h + 18.0 * e,
        };
        p.rellenar_redondeado(
            pastilla,
            pastilla.alto / 2.0,
            crate::caja_dibujo::hex(0x248a3d),
        );
        p.texto(
            texto,
            pastilla.x + 18.0 * e,
            pastilla.y + 9.0 * e,
            tam,
            Color::BLANCO,
        );
    }
}

/// Si el tema es el de dia.
fn es_claro(tema: &Tema) -> bool {
    let c = tema.lista;
    c.r + c.g + c.b > 1.5
}

#[allow(clippy::too_many_arguments)]
fn pintar_previa(
    p: &Pintor,
    f: &Vista,
    caja: Rect,
    tema: &Tema,
    e: f32,
    textos: &Catalogo,
    g: &Galeria,
    zonas: &mut Vec<(Rect, Zona)>,
) {
    let m = 18.0 * e;
    let x = caja.x as f32 + m;
    let ancho = caja.ancho as f32 - 2.0 * m;
    let papel = RectF {
        x,
        y: caja.y as f32 + m,
        ancho,
        alto: ancho * 0.85,
    };
    p.rellenar_redondeado(papel, 12.0 * e, tema.papel);
    if let Some(d) = &f.dibujo {
        pintar_lienzo(
            p,
            d,
            RectF {
                x: papel.x + 14.0 * e,
                y: papel.y + 14.0 * e,
                ancho: papel.ancho - 28.0 * e,
                alto: papel.alto - 28.0 * e,
            },
            None,
        );
    }
    let mut y = papel.y + papel.alto + 16.0 * e;
    // Eligiendo para un chat, el boton grande va justo debajo del dibujo:
    // es lo primero que se busca al elegir una.
    if let Some(d) = &g.destino {
        let boton = Rect {
            x: x as i32,
            y: y as i32,
            ancho: ancho as u32,
            alto: (ALTO_ADJUNTAR as f32 * e) as u32,
        };
        let azul = crate::caja_dibujo::hex(0x0a6cd6);
        p.rellenar_redondeado(
            rf(boton),
            12.0 * e,
            if boton.contiene(g.raton) {
                azul
            } else {
                con_alfa(azul, 0.88)
            },
        );
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("chat", d.chat.clone());
        let rotulo = textos.t_args("figuras-adjuntar-a", &args);
        let lado = 20.0 * e;
        let (tw, th) = p.medir_texto(&rotulo, 15.0 * e);
        let tw = tw.min(boton.ancho as f32 - lado - 40.0 * e);
        let x0 = boton.x as f32 + (boton.ancho as f32 - lado - 8.0 * e - tw) / 2.0;
        p.icono(
            &mi::ATTACH_FILE,
            RectF {
                x: x0,
                y: boton.y as f32 + (boton.alto as f32 - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            Color::BLANCO,
        );
        p.texto_linea(
            &rotulo,
            x0 + lado + 8.0 * e,
            boton.y as f32 + (boton.alto as f32 - th) / 2.0,
            15.0 * e,
            tw,
            Color::BLANCO,
        );
        zonas.push((boton, Zona::Adjuntar));
        y += boton.alto as f32 + 6.0 * e;
        let pista = textos.t("figuras-adjuntar-pista");
        p.texto_linea(&pista, x, y, 11.5 * e, ancho, tema.apagado);
        y += 26.0 * e;
    }
    let (_, h) = p.medir_texto_ajustado(&f.nombre, 17.0 * e, ancho);
    p.texto_ajustado(&f.nombre, x, y, 17.0 * e, ancho, tema.texto);
    y += h + 4.0 * e;
    let grupo = rotulo_del_grupo(&f.grupo, textos);
    let origen = if f.de_serie {
        textos.t("figuras-de-serie")
    } else {
        textos.t("figuras-tuya")
    };
    p.texto_linea(
        &format!("{grupo} · {origen}"),
        x,
        y,
        12.5 * e,
        ancho,
        tema.apagado,
    );
    y += 26.0 * e;
    let datos = [
        (
            textos.t("figuras-mide"),
            format!("{:.0} × {:.0}", f.tamano.0, f.tamano.1),
        ),
        (
            textos.t("figuras-piezas"),
            resumen_de_piezas(&f.piezas, textos),
        ),
    ];
    for (rotulo, valor) in datos {
        p.texto(&rotulo, x, y, 12.5 * e, tema.apagado);
        let (_, vh) = p.medir_texto_ajustado(&valor, 12.5 * e, ancho - 90.0 * e);
        p.texto_ajustado(
            &valor,
            x + 90.0 * e,
            y,
            12.5 * e,
            ancho - 90.0 * e,
            tema.texto,
        );
        y += vh.max(16.0 * e) + 6.0 * e;
    }
    if !f.letras.is_empty() {
        y += 4.0 * e;
        p.texto(&textos.t("figuras-textos"), x, y, 12.5 * e, tema.apagado);
        y += 20.0 * e;
        let tope = caja.abajo() as f32 - 70.0 * e;
        for l in &f.letras {
            if y + 16.0 * e > tope {
                p.texto("…", x, y, 12.5 * e, tema.apagado);
                break;
            }
            p.texto_linea(&format!("• {l}"), x, y, 12.5 * e, ancho, tema.texto);
            y += 18.0 * e;
        }
    }
    if !f.de_serie {
        let boton = Rect {
            x: x as i32,
            y: (caja.abajo() as f32 - m - 38.0 * e) as i32,
            ancho: ancho as u32,
            alto: (38.0 * e) as u32,
        };
        let rojo = crate::caja_dibujo::hex(0xd9534f);
        p.rellenar_redondeado(
            rf(boton),
            10.0 * e,
            if g.borrar_pedido || boton.contiene(g.raton) {
                rojo
            } else {
                con_alfa(rojo, 0.75)
            },
        );
        let rotulo = if g.borrar_pedido {
            textos.t("figuras-borrar-seguro")
        } else {
            textos.t("figuras-borrar")
        };
        let (tw, th) = p.medir_texto(&rotulo, 14.0 * e);
        p.texto(
            &rotulo,
            boton.x as f32 + (boton.ancho as f32 - tw) / 2.0,
            boton.y as f32 + (boton.alto as f32 - th) / 2.0,
            14.0 * e,
            Color::BLANCO,
        );
        zonas.push((boton, Zona::Borrar));
    }
}

/// «23 · 8 líneas, 6 textos, 4 formas».
fn resumen_de_piezas(p: &Piezas, textos: &Catalogo) -> String {
    let mut partes = Vec::new();
    for (n, clave) in [
        (p.lineas, "figuras-lineas"),
        (p.formas, "figuras-formas"),
        (p.textos, "figuras-de-texto"),
        (p.trazos, "figuras-trazos"),
        (p.otras, "figuras-otras"),
    ] {
        if n > 0 {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("n", n as i64);
            partes.push(textos.t_args(clave, &args));
        }
    }
    if partes.is_empty() {
        p.total.to_string()
    } else {
        format!("{} · {}", p.total, partes.join(", "))
    }
}

/// Una ficha de la rejilla, como las del movil: la hoja con la figura en
/// una tarjeta redondeada con su filete y el nombre debajo. La elegida lleva
/// el anillo azul; con el raton encima, el filete mas marcado.
#[allow(clippy::too_many_arguments)]
fn pintar_celda(
    p: &Pintor,
    f: &Vista,
    celda: Rect,
    alto_dibujo: u32,
    tema: &Tema,
    e: f32,
    encima: bool,
    elegida: bool,
) {
    let dibujo = RectF {
        x: celda.x as f32,
        y: celda.y as f32,
        ancho: celda.ancho as f32,
        alto: alto_dibujo as f32,
    };
    let radio = 14.0 * e;
    if elegida {
        p.rellenar_redondeado(
            encoger(dibujo, -3.0 * e),
            radio + 3.0 * e,
            crate::caja_dibujo::hex(0x0a84ff),
        );
    }
    p.rellenar_redondeado(
        dibujo,
        radio,
        con_alfa(tema.texto, if encima { 0.22 } else { 0.10 }),
    );
    p.rellenar_redondeado(encoger(dibujo, 1.0), radio - 1.0, tema.papel);
    if let Some(d) = &f.dibujo {
        pintar_lienzo(
            p,
            d,
            RectF {
                x: dibujo.x + 12.0 * e,
                y: dibujo.y + 12.0 * e,
                ancho: dibujo.ancho - 24.0 * e,
                alto: dibujo.alto - 24.0 * e,
            },
            None,
        );
    }
    p.texto_linea(
        &f.nombre,
        dibujo.x + 4.0 * e,
        dibujo.y + dibujo.alto + 8.0 * e,
        13.0 * e,
        dibujo.ancho - 8.0 * e,
        tema.texto,
    );
}

// --- Adjuntar figuras al asistente (25-oct-2026) --------------------------------
//
// El usuario: «en la opcion de adjuntar del asistente … puedo adjuntar una
// figura para que la mejore, o decirle hazme una figura, y lo puedo anadir
// facilmente a la lista de figuras».

/// Las figuras de la biblioteca, en el orden de `biblioteca::cargar`.
pub(super) fn todas() -> Vec<FiguraGuardada> {
    biblioteca::cargar(&crate::biblioteca_figuras::ruta())
}

/// **La escena de un lienzo con la figura dentro**, para adjuntarla al chat.
pub(super) fn escena_con(f: &FiguraGuardada) -> pindesk_motor2d::Escena {
    let mut escena = pindesk_motor2d::Escena::nueva();
    let (w, h) = f.tamano();
    biblioteca::estampar_en_escena(
        &mut escena,
        f,
        pindesk_motor2d::vector::Punto2::nuevo(w / 2.0, h / 2.0),
    );
    escena
}

/// **Guarda un lienzo del chat como figura** con `nombre` (la del mismo
/// nombre se sustituye). Lo que se guarda es lo que se ve en el lienzo, con
/// sus piezas tal cual.
pub(super) fn guardar_lienzo(escena: &pindesk_motor2d::Escena, nombre: &str) -> Result<(), String> {
    let piezas: Vec<pindesk_motor2d::Elemento> = escena.visibles().cloned().collect();
    if piezas.is_empty() {
        return Err("vacio".into());
    }
    let figura = FiguraGuardada {
        id: biblioteca::id_nuevo(),
        nombre: nombre.trim().chars().take(80).collect(),
        elementos: biblioteca::normalizar(&piezas),
    };
    if crate::biblioteca_figuras::anadir(&crate::biblioteca_figuras::ruta(), figura) {
        Ok(())
    } else {
        Err("no se pudo escribir".into())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_figura_adjuntada_entra_entera_en_su_lienzo() {
        let catalogo = pindesk_motor2d::figuras_de_serie::catalogo();
        // La figura estampada es la misma, pieza a pieza.
        let e = escena_con(&catalogo[0]);
        assert_eq!(
            e.visibles().count(),
            catalogo[0].elementos.iter().filter(|x| !x.borrado).count()
        );
        let c = e.caja().unwrap();
        assert!(c.0 >= -0.5 && c.1 >= -0.5, "desde el origen: {c:?}");
    }

    #[test]
    fn sin_chat_de_destino_ni_el_doble_clic_ni_intro_adjuntan() {
        let t = Catalogo::nuevo(Idioma::Espanol);
        let mut g = Galeria {
            figuras: vec![vista_de(&FiguraGuardada {
                id: "mia".into(),
                nombre: "Mi árbol".into(),
                elementos: Vec::new(),
            })],
            elegida: Some(0),
            ..Galeria::default()
        };
        assert!(!g.adjuntar(&t), "abierta para mirar: no va a ningun chat");
        assert!(g.aviso.is_none());
        // Caso positivo: con destino va al buzon del chat y deja el aviso
        // con su nombre.
        g.destino = Some(Destino {
            proyecto: "p-prueba-figuras".into(),
            chat: "Casa Lima".into(),
        });
        assert!(g.adjuntar(&t));
        assert!(
            g.aviso
                .as_ref()
                .is_some_and(|(a, _)| a.contains("Casa Lima"))
        );
        assert_eq!(
            super::super::tomar_figura_pedida(),
            Some(("p-prueba-figuras".to_string(), "mia".to_string()))
        );
    }

    #[test]
    fn lo_del_usuario_va_primero_y_las_categorias_en_su_orden() {
        assert_eq!(orden_del_grupo(MIS_FIGURAS), 0);
        assert!(orden_del_grupo(CATEGORIAS[0]) < orden_del_grupo(CATEGORIAS[1]));
        // Caso negativo: una categoria inventada no se cuela delante de las
        // de serie.
        assert!(
            orden_del_grupo("Mi coleccion") > orden_del_grupo(CATEGORIAS[CATEGORIAS.len() - 1])
        );
    }

    #[test]
    fn el_grupo_sale_del_nombre_y_se_busca_sin_tildes() {
        let f = FiguraGuardada {
            id: "x".into(),
            nombre: "Diario · Gratitud".into(),
            elementos: Vec::new(),
        };
        let v = vista_de(&f);
        assert_eq!(
            (v.grupo.as_str(), v.nombre.as_str()),
            ("Diario", "Gratitud")
        );
        assert!(v.dibujo.is_none(), "sin piezas no hay nada que dibujar");
        assert!(casa(&v, "GRATI") && casa(&v, "diario") && casa(&v, ""));
        assert!(!casa(&v, "árbol"), "lo que no esta no casa");
        let mia = vista_de(&FiguraGuardada {
            nombre: "Mi árbol".into(),
            ..f
        });
        assert_eq!(mia.grupo, MIS_FIGURAS);
        assert!(!mia.de_serie);
        assert!(casa(&mia, "arbol"), "sin tildes tambien");
    }

    #[test]
    fn el_previsualizador_cuenta_las_piezas_y_lee_los_textos() {
        let catalogo = pindesk_motor2d::figuras_de_serie::catalogo();
        let con_textos = catalogo
            .iter()
            .find(|f| piezas_de(f).textos > 1)
            .expect("alguna de serie lleva textos");
        let p = piezas_de(con_textos);
        assert_eq!(
            p.total,
            p.trazos + p.lineas + p.formas + p.textos + p.otras,
            "cada pieza cuenta una vez"
        );
        assert!(!letras_de(con_textos).is_empty());
        // Caso negativo: una figura sin textos no lista ninguno.
        let sin = FiguraGuardada {
            id: "y".into(),
            nombre: "Nada".into(),
            elementos: Vec::new(),
        };
        assert!(letras_de(&sin).is_empty());
        assert_eq!(piezas_de(&sin), Piezas::default());
    }

    #[test]
    fn de_dia_sin_las_gemelas_y_de_noche_cada_una_en_su_sitio() {
        let f = |id: &str, nombre: &str| {
            vista_de(&FiguraGuardada {
                id: id.into(),
                nombre: nombre.into(),
                elementos: Vec::new(),
            })
        };
        let mut g = Galeria {
            figuras: vec![
                f("mia", "Mi flecha"),
                f("serie-diario", "Diario · Diario"),
                f("serie-gratitud", "Diario · Gratitud"),
                f("serie-diario-noche", "Diario · Diario (noche)"),
            ],
            ..Galeria::default()
        };
        g.modo = segun_el_modo(&g.pares(), false);
        g.elegida = Some(1);
        let ids = |g: &Galeria| -> Vec<String> {
            g.modo.iter().map(|i| g.figuras[*i].id.clone()).collect()
        };
        assert_eq!(ids(&g), ["mia", "serie-diario", "serie-gratitud"]);
        assert_eq!(
            g.grupos(),
            [(MIS_FIGURAS.to_string(), 1), ("Diario".into(), 2)]
        );
        g.poner_noche(true);
        assert_eq!(ids(&g), ["mia", "serie-diario-noche", "serie-gratitud"]);
        assert_eq!(g.figuras[g.elegida.unwrap()].id, "serie-diario-noche");
        // Caso negativo: lo que no tiene gemela sigue igual de noche.
        assert_eq!(
            g.grupos(),
            [(MIS_FIGURAS.to_string(), 1), ("Diario".into(), 2)]
        );
        g.poner_noche(false);
        assert_eq!(g.figuras[g.elegida.unwrap()].id, "serie-diario");
    }
}
