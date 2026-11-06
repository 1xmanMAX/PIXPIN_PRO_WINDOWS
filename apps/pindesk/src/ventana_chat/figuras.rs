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
//! - **A la derecha, la elegida en grande** (3-nov-2026, el usuario: «quiero
//!   una vista previa COMPLETA en el panel lateral … datos como lo que mide
//!   o el texto que contiene no son utiles en todo ese espacio»): el panel se
//!   lleva media ventana y la figura lo llena, con solo su nombre y su grupo
//!   en una linea. Se acerca y se lleva como un lienzo
//!   (`pindesk_ui::vista_figura`): dos dedos la llevan en cualquier
//!   direccion, pellizcar o Ctrl + rueda acercan hacia el raton, arrastrar
//!   la lleva, doble clic (o «0») la encaja y «+»/«−» acercan y alejan. Se
//!   vuelve a pintar a cada aumento, nitida. Las del usuario se pueden
//!   borrar (dos clics).
//! - **Con la ventana estrecha, una cosa cada vez**, como el chat estrecho:
//!   al elegir una figura la ventana entera pasa a ser esa figura, con «‹
//!   Figuras» para volver a la rejilla, sus parametros debajo y «Adjuntar».
//! - **Los parametros de las plantillas** (3-nov-2026, el usuario: «la
//!   opcion de poner PARAMETROS ya no esta: numero de elementos, columnas,
//!   filas, dias, actividades…»): las que se alargan llevan debajo del
//!   dibujo sus valores con − y + (y se pueden escribir): del cuanto al
//!   cuanto, cuantos por hoja, las semanas del cronograma, el mes del
//!   mensual y de los habitos (`figuras_de_serie::Parametros`). El dibujo
//!   cambia al momento y «Adjuntar» manda lo que se ve, con todas sus hojas
//!   en fila. Se recuerdan por plantilla.
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
//! para un chat), Escape borra lo buscado y, sin nada buscado, cierra (con
//! una figura sola, vuelve a la rejilla). Ctrl + «+», «−» y «0» acercan,
//! alejan y encajan la figura (sin Ctrl con la figura sola).

use std::cell::{Cell, RefCell};
use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pindesk_geom::{Punto, Rect};
use pindesk_motor2d::biblioteca::{self, FiguraGuardada};
use pindesk_motor2d::figuras_de_serie::{
    CATEGORIAS, Intervalo, Parametro, Parametros, alargar, categoria_de, clave_de, es_de_serie,
    es_la_de_noche, pareja, segun_el_modo, tiene_parametros,
};
use pindesk_render::{Color, Pintor, RectF, Superficie};
use pindesk_shell::overlay::{EventoOverlay, VentanaOverlay};
use pindesk_store::{Catalogo, Idioma};
use pindesk_ui::encuadre::Encuadre;
use pindesk_ui::vista_figura::{self as vf, Gesto, Reparto, TeclaZoom};

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
const ALTO_TITULO: u32 = 36;
/// El grupo de lo que no lleva categoria: lo del usuario.
pub(super) const MIS_FIGURAS: &str = "";

/// Lo que mide el boton de adjuntar del previsualizador (logicos).
const ALTO_ADJUNTAR: u32 = 44;
/// Cada fila de parametros y su − / +.
const ALTO_PARAMETRO: f32 = 34.0;
/// Lo que mide la pastilla del aumento, abajo a la derecha del dibujo.
const ALTO_ZOOM: f32 = 30.0;
/// Cuantas cifras se pueden escribir en un parametro.
const CIFRAS: usize = 4;
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
const VK_CONTROL: u32 = 0x11;

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
                recordar: true,
                ano_de_hoy: crate::tareas::hoy().anio,
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

/// Una figura, ya lista para pintar.
struct Vista {
    id: String,
    /// El nombre sin la categoria delante.
    nombre: String,
    grupo: String,
    de_serie: bool,
    /// La plantilla de serie que es, si lleva parametros (se alarga).
    clave: Option<String>,
    /// Los textos que lleva, en orden de lectura (arriba abajo): se buscan.
    letras: Vec<String>,
    dibujo: Option<LienzoVisto>,
    /// La figura tal cual, para adjuntarla.
    figura: FiguraGuardada,
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
    } else if g == pindesk_plantillas::exportar::figura::CATEGORIA {
        // Las de PinDesk Plantillas, justo detras de las del usuario.
        1
    } else {
        CATEGORIAS
            .iter()
            .position(|c| *c == g)
            .map_or(CATEGORIAS.len() + 2, |i| i + 2)
    }
}

fn vista_de(f: &FiguraGuardada) -> Vista {
    let (grupo, nombre) = match categoria_de(&f.nombre) {
        Some((c, n)) => (c.to_string(), n.to_string()),
        None => (MIS_FIGURAS.to_string(), f.nombre.clone()),
    };
    let clave = clave_de(f)
        .filter(|c| tiene_parametros(c))
        .map(str::to_string);
    Vista {
        id: f.id.clone(),
        nombre,
        grupo,
        de_serie: es_de_serie(f),
        clave,
        letras: letras_de(f),
        dibujo: dibujo_de(f),
        figura: f.clone(),
    }
}

/// La figura como la vista previa de un lienzo: sus ordenes y su caja.
fn dibujo_de(f: &FiguraGuardada) -> Option<LienzoVisto> {
    let mut escena = pindesk_motor2d::escena::Escena::nueva();
    for e in &f.elementos {
        escena.anadir(e.clone());
    }
    dibujo_de_escena(&escena, 640.0)
}

/// Las ordenes de una escena para pintarla en una caja de hasta `lado`
/// pixeles (la finura del grafito; lo demas es vectorial y sale nitido a
/// cualquier aumento).
fn dibujo_de_escena(escena: &pindesk_motor2d::Escena, lado: f32) -> Option<LienzoVisto> {
    let caja = escena.caja()?;
    let mide = (caja.2 - caja.0).max(caja.3 - caja.1).max(1.0);
    let (ordenes, grafitos) = pindesk_motor2d::tinta::grafito::ordenes_con_grafito(
        escena,
        (lado / mide).min(1.0),
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
    /// «‹ Figuras»: de la figura sola a la rejilla (ventana estrecha).
    Volver,
    /// El dibujo en grande: arrastrar lo lleva, doble clic lo encaja.
    Dibujo,
    ZoomMenos,
    ZoomMas,
    /// El tanto por ciento de la pastilla del aumento: encaja.
    ZoomEncajar,
    /// El − (`-1`) o el + (`1`) de un parametro.
    Paso(Parametro, i32),
    /// El numero de un parametro: se escribe.
    Valor(Parametro),
    /// Los parametros, como los de serie.
    Restablecer,
    /// «Abrir en Plantillas» (una figura que vino de PinDesk Plantillas).
    AbrirEnPlantillas,
}

/// **La elegida como se adjunta**: sus hojas (la figura tal cual, o la
/// plantilla con sus parametros) y su dibujo, hecho una vez por cambio.
struct Previa {
    /// Para que se hizo: la figura (su id) y lo que se le pidio.
    id: String,
    pedido: Option<Intervalo>,
    hojas: Vec<FiguraGuardada>,
    dibujo: Option<LienzoVisto>,
    /// Si lo pedido no se pudo hacer (sale la de serie y esto debajo).
    error: Option<String>,
}

/// La finura del grafito del dibujo en grande (los trazos son vectoriales).
const LADO_GRANDE: f32 = 2400.0;

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
    /// Los parametros de la elegida, si es una plantilla que los lleva.
    parametros: Option<Parametros>,
    /// Si se leen y se guardan los ultimos de cada plantilla (en la ventana
    /// de verdad; las pruebas y las muestras no tocan el estado).
    recordar: bool,
    ano_de_hoy: i32,
    /// El parametro en el que se escribe y lo escrito.
    foco: Option<Parametro>,
    tecleado: String,
    previa: Option<Previa>,
    /// Como se ve el dibujo en grande: el aumento y lo llevado.
    encuadre: Encuadre,
    /// Ventana estrecha y una figura elegida: se ve solo ella.
    viendo: bool,
    /// Lo que midio la ventana en el ultimo fotograma: estrecha (una cosa
    /// cada vez) o no, y donde quedo el dibujo en grande (vacio sin el).
    una_cada_vez: Cell<bool>,
    caja_dibujo: Cell<Rect>,
    /// Arrastrando el dibujo: donde iba el raton.
    arrastrando: Option<Punto>,
    /// El ultimo clic en el dibujo, para el doble clic.
    clic_dibujo: Option<Instant>,
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
            parametros: None,
            recordar: false,
            ano_de_hoy: 2026,
            foco: None,
            tecleado: String::new(),
            previa: None,
            encuadre: Encuadre::default(),
            viendo: false,
            una_cada_vez: Cell::new(false),
            caja_dibujo: Cell::new(Rect {
                x: 0,
                y: 0,
                ancho: 0,
                alto: 0,
            }),
            arrastrando: None,
            clic_dibujo: None,
        }
    }
}

impl Galeria {
    fn leer(&mut self) {
        let elegida = self
            .elegida
            .and_then(|i| self.figuras.get(i))
            .map(|f| f.id.clone());
        let mut figuras = biblioteca::cargar(&crate::biblioteca_figuras::ruta());
        // Y las de PinDesk Plantillas, de su carpeta (docs/plantillas.md).
        figuras.extend(crate::biblioteca_figuras::de_plantillas());
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
        // Lo escrito en un parametro vale al pulsar fuera de el.
        if self.foco.is_some() && !matches!(&zona, Some(Zona::Valor(p)) if self.foco == Some(*p)) {
            self.confirmar_tecleado();
        }
        match &zona {
            Some(Zona::Grupo(g)) => {
                self.filtro = g.clone();
                self.scroll = 0;
            }
            Some(Zona::Buscador) => {}
            Some(Zona::Figura(i)) => {
                self.elegida = Some(*i);
                // Estrecha, la figura elegida se ve sola, como un chat.
                if self.una_cada_vez.get() {
                    self.viendo = true;
                }
                let doble = self
                    .ultimo_clic
                    .is_some_and(|(j, t)| j == *i && t.elapsed() <= DOBLE_CLIC);
                self.ultimo_clic = Some((*i, Instant::now()));
                if doble {
                    self.ultimo_clic = None;
                    self.adjuntar(textos);
                }
            }
            Some(Zona::Volver) => {
                self.viendo = false;
                self.arrastrando = None;
            }
            Some(Zona::Dibujo) => {
                let doble = self.clic_dibujo.is_some_and(|t| t.elapsed() <= DOBLE_CLIC);
                if doble {
                    self.clic_dibujo = None;
                    self.encuadre.encajar();
                } else {
                    self.clic_dibujo = Some(Instant::now());
                    self.arrastrando = Some(l);
                }
            }
            Some(Zona::ZoomMenos) => {
                self.acercar(1.0 / pindesk_ui::encuadre::PASO, None);
            }
            Some(Zona::ZoomMas) => {
                self.acercar(pindesk_ui::encuadre::PASO, None);
            }
            Some(Zona::ZoomEncajar) => self.encuadre.encajar(),
            Some(Zona::Paso(p, pasos)) => self.paso_parametro(*p, *pasos),
            Some(Zona::Valor(p)) => {
                if self.foco != Some(*p) {
                    self.foco = Some(*p);
                    self.tecleado.clear();
                }
            }
            Some(Zona::Restablecer) => {
                if let Some(p) = &mut self.parametros {
                    p.restablecer();
                }
                self.guardar_parametros();
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
            Some(Zona::AbrirEnPlantillas) => {
                if let Some(f) = self.elegida.and_then(|i| self.figuras.get(i))
                    && !crate::biblioteca_figuras::abrir_en_plantillas(&f.id)
                {
                    tracing::warn!(id = %f.id, "figuras: no se pudo abrir en Plantillas");
                }
            }
            _ => {}
        }
        zona
    }

    /// Si se ve una figura sola (ventana estrecha y una elegida).
    fn sola(&self) -> bool {
        self.viendo && self.una_cada_vez.get() && self.elegida.is_some()
    }

    /// **Lo que se le pide a la elegida**: sus parametros, si no son los de
    /// serie (`None`: la figura tal cual). La gemela de noche sale de noche.
    fn pedido(&self) -> Option<Intervalo> {
        let f = self.elegida.and_then(|i| self.figuras.get(i))?;
        let p = self
            .parametros
            .as_ref()
            .filter(|p| f.clave.as_deref() == Some(p.clave()))?;
        let mut iv = p.intervalo()?;
        if es_la_de_noche(&f.figura) {
            iv.noche = Some(true);
        }
        Some(iv)
    }

    /// **Pone al dia lo de la elegida** antes de pintar: sus parametros (los
    /// ultimos de esa plantilla, o los de serie) y sus hojas con su dibujo
    /// en grande, si cambio algo.
    fn preparar(&mut self) {
        let Some(f) = self.elegida.and_then(|i| self.figuras.get(i)) else {
            self.previa = None;
            self.parametros = None;
            self.foco = None;
            return;
        };
        let (id, clave, figura) = (f.id.clone(), f.clave.clone(), f.figura.clone());
        // Los parametros siguen a la plantilla: la de dia y su gemela de
        // noche son la misma.
        if self.parametros.as_ref().map(|p| p.clave()) != clave.as_deref() {
            self.foco = None;
            self.parametros = clave.as_deref().and_then(|c| {
                let recordados = if self.recordar {
                    crate::biblioteca_figuras::parametros_de(c)
                } else {
                    None
                };
                recordados
                    .and_then(|t| Parametros::de_texto(c, &t, self.ano_de_hoy))
                    .or_else(|| Parametros::de_serie(c, self.ano_de_hoy))
            });
        }
        let pedido = self.pedido();
        if self
            .previa
            .as_ref()
            .is_some_and(|p| p.id == id && p.pedido == pedido)
        {
            return;
        }
        let (hojas, error) = match (&pedido, &clave) {
            (Some(iv), Some(c)) => match alargar(c, iv) {
                Ok(h) if !h.is_empty() => (h, None),
                Ok(_) => (vec![figura], None),
                Err(e) => (vec![figura], Some(e)),
            },
            _ => (vec![figura], None),
        };
        let dibujo = dibujo_de_escena(&escena_con_hojas(&hojas), LADO_GRANDE);
        self.encuadre.encajar();
        self.previa = Some(Previa {
            id,
            pedido,
            hojas,
            dibujo,
            error,
        });
    }

    /// Sube o baja un parametro (el − y el +, la rueda encima).
    fn paso_parametro(&mut self, p: Parametro, pasos: i32) {
        if let Some(ps) = &mut self.parametros {
            ps.paso(p, pasos);
        }
        self.guardar_parametros();
    }

    /// Lo escrito en el parametro con el foco, puesto (acotado); sin numero,
    /// se queda como estaba.
    fn confirmar_tecleado(&mut self) {
        let foco = self.foco.take();
        let tecleado = std::mem::take(&mut self.tecleado);
        if let (Some(p), Ok(v)) = (foco, tecleado.parse::<u32>()) {
            if let Some(ps) = &mut self.parametros {
                ps.poner(p, v);
            }
            self.guardar_parametros();
        }
    }

    fn guardar_parametros(&self) {
        if self.recordar
            && let Some(p) = &self.parametros
        {
            crate::biblioteca_figuras::recordar_parametros(p.clave(), &p.a_texto());
        }
    }

    /// La caja del dibujo en grande y lo que mide el dibujo, para el
    /// encuadre; `None` si no se esta viendo.
    fn encuadre_de(&self) -> Option<((f32, f32, f32, f32), f32, f32)> {
        let c = self.caja_dibujo.get();
        let d = self.previa.as_ref()?.dibujo.as_ref()?;
        (c.ancho > 0 && c.alto > 0).then_some((
            (c.x as f32, c.y as f32, c.ancho as f32, c.alto as f32),
            (d.caja.2 - d.caja.0).max(1.0),
            (d.caja.3 - d.caja.1).max(1.0),
        ))
    }

    /// Acerca (`factor` > 1) o aleja hacia `punto` (sin el, hacia el centro).
    /// Devuelve si cambio.
    fn acercar(&mut self, factor: f32, punto: Option<Punto>) -> bool {
        let Some((caja, w, h)) = self.encuadre_de() else {
            return false;
        };
        let punto = punto.map_or((caja.0 + caja.2 / 2.0, caja.1 + caja.3 / 2.0), |p| {
            (p.x as f32, p.y as f32)
        });
        let antes = self.encuadre;
        self.encuadre.acercar(caja, w, h, factor, punto);
        self.encuadre != antes
    }

    /// Lleva el dibujo en grande. Devuelve si cambio.
    fn mover_dibujo(&mut self, dx: f32, dy: f32) -> bool {
        let Some((caja, w, h)) = self.encuadre_de() else {
            return false;
        };
        let antes = self.encuadre;
        self.encuadre.mover(caja, w, h, dx, dy);
        self.encuadre != antes
    }

    /// Si el raton esta encima del dibujo en grande.
    fn sobre_el_dibujo(&self, l: Punto) -> bool {
        self.caja_dibujo.get().contiene(l)
    }

    /// **Adjunta la elegida** al chat del destino, si lo hay, como se ve:
    /// con sus parametros, todas sus hojas. Va por el buzon del chat y aqui
    /// solo queda el aviso. Sin destino no hace nada (la ventana se abrio
    /// para mirar).
    fn adjuntar(&mut self, textos: &Catalogo) -> bool {
        if self.destino.is_none() || self.elegida.is_none() {
            return false;
        }
        self.preparar();
        let (Some(d), Some(p)) = (self.destino.as_ref(), self.previa.as_ref()) else {
            return false;
        };
        super::entregar_figura(d.proyecto.clone(), p.hojas.clone());
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
    // La carpeta de PinDesk Plantillas: una plantilla mandada desde alli.
    let mut plantillas = crate::biblioteca_figuras::plantillas_cambiadas();
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
                    // Arrastrando el dibujo en grande: lo lleva.
                    if let Some(antes) = g.arrastrando {
                        g.arrastrando = Some(l);
                        if g.mover_dibujo((l.x - antes.x) as f32, (l.y - antes.y) as f32) {
                            pintar = true;
                        }
                    }
                    if g.zona_en(l) != g.zona_en(g.raton) {
                        pintar = true;
                    }
                    g.raton = l;
                }
                EventoOverlay::BotonPulsado(p) => {
                    let l = local(p, marco);
                    g.raton = l;
                    match g.pulsar(l, &mut vivo, textos) {
                        Some(Zona::Barra) => {
                            moviendo = Some((p, marco));
                            ventana.capturar_raton();
                        }
                        Some(Zona::Dibujo) if g.arrastrando.is_some() => ventana.capturar_raton(),
                        _ => {}
                    }
                    pintar = true;
                }
                EventoOverlay::BotonSoltado(_) => {
                    if moviendo.take().is_some() | g.arrastrando.take().is_some() {
                        ventana.soltar_raton();
                    }
                }
                // Encima del dibujo en grande, la rueda lo lleva (dos dedos,
                // en cualquier direccion) y con Ctrl —o pellizcando— acerca.
                EventoOverlay::Rueda(delta) | EventoOverlay::RuedaHorizontal(delta)
                    if g.sobre_el_dibujo(g.raton) =>
                {
                    let horizontal = matches!(ev, EventoOverlay::RuedaHorizontal(_));
                    let ctrl = pindesk_shell::entrada::tecla_pulsada_ahora(VK_CONTROL);
                    let cambio =
                        match vf::gesto_de_rueda(delta, horizontal, ctrl, escala as f32 / 100.0) {
                            Gesto::Acercar(f) => g.acercar(f, Some(g.raton)),
                            Gesto::Mover { dx, dy } => g.mover_dibujo(dx, dy),
                        };
                    pintar |= cambio;
                }
                // Encima de un parametro, la rueda lo sube o lo baja.
                EventoOverlay::Rueda(delta)
                    if matches!(g.zona_en(g.raton), Some(Zona::Paso(..) | Zona::Valor(_))) =>
                {
                    if let Some(Zona::Paso(p, _) | Zona::Valor(p)) = g.zona_en(g.raton) {
                        g.paso_parametro(p, delta.signum());
                        pintar = true;
                    }
                }
                // La rueda encima de las categorias las corre de lado.
                EventoOverlay::Rueda(delta) | EventoOverlay::RuedaHorizontal(delta)
                    if !g.sola()
                        && (BARRA * escala / 100) as i32 <= g.raton.y
                        && g.raton.y < ((BARRA + FILA_CHIPS) * escala / 100) as i32 =>
                {
                    let delta = if matches!(ev, EventoOverlay::RuedaHorizontal(_)) {
                        -delta
                    } else {
                        delta
                    };
                    let nuevo = (g.scroll_chips - delta * (CHIP * escala / 100) as i32 / 120 * 2)
                        .clamp(0, g.ancho_chips.get().max(0));
                    if nuevo != g.scroll_chips {
                        g.scroll_chips = nuevo;
                        pintar = true;
                    }
                }
                EventoOverlay::Rueda(delta) if !g.sola() => {
                    let paso = (CELDA_DIBUJO * escala / 100) as i32;
                    let nuevo = (g.scroll - delta * paso / 120 * 6 / 10)
                        .clamp(0, g.alto_total.get().max(0));
                    if nuevo != g.scroll {
                        g.scroll = nuevo;
                        pintar = true;
                    }
                }
                // Escribiendo en un parametro: Intro lo pone, Escape lo deja.
                EventoOverlay::Tecla { vk, .. } if g.foco.is_some() => {
                    pintar = true;
                    match vk {
                        VK_ENTRAR => g.confirmar_tecleado(),
                        VK_ESCAPE => {
                            g.foco = None;
                            g.tecleado.clear();
                        }
                        VK_RETROCESO => {
                            g.tecleado.pop();
                        }
                        _ => pintar = false,
                    }
                }
                // Ctrl + «+», «−» y «0» (sin Ctrl con la figura sola, que no
                // tiene buscador).
                EventoOverlay::Tecla { vk, ctrl, .. }
                    if (ctrl || g.sola()) && vf::tecla_de_zoom(vk).is_some() =>
                {
                    pintar = match vf::tecla_de_zoom(vk) {
                        Some(TeclaZoom::Acercar) => g.acercar(pindesk_ui::encuadre::PASO, None),
                        Some(TeclaZoom::Alejar) => {
                            g.acercar(1.0 / pindesk_ui::encuadre::PASO, None)
                        }
                        _ => {
                            g.encuadre.encajar();
                            true
                        }
                    };
                }
                EventoOverlay::Tecla { vk, .. } => {
                    pintar = true;
                    let sola = g.sola();
                    match vk {
                        VK_ENTRAR => {
                            g.adjuntar(textos);
                        }
                        VK_ESCAPE if sola => g.viendo = false,
                        VK_ESCAPE if !g.consulta.is_empty() => g.consulta.clear(),
                        VK_ESCAPE => vivo = false,
                        VK_RETROCESO if !sola => {
                            g.consulta.pop();
                        }
                        VK_IZQUIERDA => g.mover_eleccion(-1, 0),
                        VK_DERECHA => g.mover_eleccion(1, 0),
                        VK_ARRIBA => g.mover_eleccion(0, -1),
                        VK_ABAJO => g.mover_eleccion(0, 1),
                        _ => pintar = false,
                    }
                }
                EventoOverlay::Caracter(c) if g.foco.is_some() => {
                    if c.is_ascii_digit() && g.tecleado.len() < CIFRAS {
                        g.tecleado.push(c);
                        pintar = true;
                    }
                }
                // Con la figura sola no hay buscador (y el «0» la encaja).
                EventoOverlay::Caracter(c) if !c.is_control() && !g.sola() => {
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
            let ahora_plantillas = crate::biblioteca_figuras::plantillas_cambiadas();
            if ahora != leida || ahora_plantillas != plantillas {
                leida = ahora;
                plantillas = ahora_plantillas;
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
            g.preparar();
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
    g.caja_dibujo.set(Rect {
        x: 0,
        y: 0,
        ancho: 0,
        alto: 0,
    });
    let todo = Rect {
        x: 0,
        y: 0,
        ancho,
        alto,
    };
    let claro = es_claro(tema);
    p.rellenar(rf(todo), tema.lista);
    // Ancha, la rejilla y la figura al lado; estrecha, una cosa cada vez
    // (como el chat): elegida una, se ve sola.
    let reparto = vf::reparto(ancho as f32 / e);
    g.una_cada_vez.set(reparto == Reparto::UnaCadaVez);
    let sola = g.sola();
    let elegida = g.elegida.and_then(|i| g.figuras.get(i));

    // --- La cabecera del movil: «Cerrar» (o «‹ Figuras» con una figura
    // sola) a la izquierda, el titulo con su cuenta en medio y el ☀/☾
    // redondo a la derecha. Lo que no es boton la arrastra.
    let barra = Rect {
        alto: ei(BARRA),
        ..todo
    };
    zonas.push((barra, Zona::Barra));
    let tam = 13.5 * e;
    let (gris, gris_sobre) = grises(tema, claro);
    let (rotulo_izq, zona_izq) = if sola {
        (format!("‹ {}", textos.t("figuras-titulo")), Zona::Volver)
    } else {
        (textos.t("pin-cerrar"), Zona::Cerrar)
    };
    let (cw, ch) = p.medir_texto(&rotulo_izq, tam);
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
        &rotulo_izq,
        cerrar.x as f32 + (cerrar.ancho as f32 - cw) / 2.0,
        cerrar.y as f32 + (cerrar.alto as f32 - ch) / 2.0,
        tam,
        tema.texto,
    );
    zonas.push((cerrar, zona_izq));

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

    // Con una figura sola, su nombre y su grupo; eligiendo para un chat, el
    // titulo dice para cual.
    let (titulo, subtitulo) = match elegida.filter(|_| sola) {
        Some(f) => (f.nombre.clone(), rotulo_del_grupo(&f.grupo, textos)),
        None => {
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
            (titulo, textos.t_args("figuras-ventana-subtitulo", &args))
        }
    };
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

    // --- Una figura sola: la ventana entera es ella, con sus parametros y
    // «Adjuntar» debajo.
    if let Some(f) = elegida.filter(|_| sola) {
        let cuerpo = Rect {
            x: 0,
            y: barra.abajo(),
            ancho,
            alto: alto.saturating_sub(barra.alto),
        };
        raya(p, 0.0, cuerpo.y as f32, ancho as f32, tema, e);
        pintar_previa(p, f, cuerpo, tema, e, textos, g, &mut zonas, true);
        if let Some((texto, _)) = &g.aviso {
            pintar_aviso(
                p,
                texto,
                ancho as f32 / 2.0,
                cuerpo.abajo() as f32 - ei(ALTO_ADJUNTAR + MARGEN + 24) as f32,
                e,
            );
        }
        return;
    }

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

    // --- La rejilla, por grupos, y la figura en grande a la derecha.
    let arriba = fila_chips.abajo();
    let cuerpo_alto = (alto as i32 - arriba).max(0) as u32;
    let previa_ancho = match reparto {
        Reparto::AlLado { previa } => ((previa * e) as u32).min(ancho),
        Reparto::UnaCadaVez => 0,
    };
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
    let ancho_celda =
        (ancho_util.saturating_sub((columnas as u32 - 1) * ei(HUECO)) / columnas as u32).max(1);
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
        (y + g.scroll - rejilla.y - rejilla.alto as i32 + ei(MARGEN + BUSCADOR + 24) as i32).max(0),
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
        if claro { tema.cabecera } else { tema.barra },
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

    // La figura en grande, a la derecha, con su raya.
    if previa_ancho > 0 {
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
        if let Some(f) = elegida {
            pintar_previa(p, f, previa, tema, e, textos, g, &mut zonas, false);
        }
    }
    // La raya de debajo de las categorias.
    raya(p, 0.0, arriba as f32, ancho as f32, tema, e);

    // «Adjuntada a … ✓», una pastilla encima del buscador.
    if let Some((texto, _)) = &g.aviso {
        pintar_aviso(
            p,
            texto,
            rejilla.x as f32 + rejilla.ancho as f32 / 2.0,
            buscador.y as f32 - 14.0 * e,
            e,
        );
    }
}

/// Si el tema es el de dia.
fn es_claro(tema: &Tema) -> bool {
    let c = tema.lista;
    c.r + c.g + c.b > 1.5
}

/// El gris de las pastillas de la cabecera y de las categorias, y el de
/// con el raton encima (los del iPhone de dia; de noche, los del tema).
fn grises(tema: &Tema, claro: bool) -> (Color, Color) {
    if claro {
        (
            crate::caja_dibujo::hex(0xf2f2f7),
            crate::caja_dibujo::hex(0xe5e5ea),
        )
    } else {
        (tema.buscador, tema.fila_sobre)
    }
}

/// Una raya fina de lado a lado.
fn raya(p: &Pintor, x: f32, y: f32, ancho: f32, tema: &Tema, e: f32) {
    p.rellenar(
        RectF {
            x,
            y,
            ancho,
            alto: e.max(1.0),
        },
        con_alfa(tema.texto, 0.08),
    );
}

/// «Adjuntada a … ✓»: una pastilla verde centrada en `centro_x`, con su
/// borde de abajo en `abajo`.
fn pintar_aviso(p: &Pintor, texto: &str, centro_x: f32, abajo: f32, e: f32) {
    let tam = 14.0 * e;
    let (w, h) = p.medir_texto(texto, tam);
    let pastilla = RectF {
        x: centro_x - (w + 36.0 * e) / 2.0,
        y: abajo - h - 18.0 * e,
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

fn rect_de(r: RectF) -> Rect {
    Rect {
        x: r.x.round() as i32,
        y: r.y.round() as i32,
        ancho: r.ancho.max(0.0).round() as u32,
        alto: r.alto.max(0.0).round() as u32,
    }
}

/// **La figura en grande** en `caja`: arriba su nombre y su grupo en una
/// linea (con la figura sola ya lo dice la cabecera), el dibujo llenando lo
/// que queda con la pastilla del aumento, y debajo sus parametros y
/// «Adjuntar» (eligiendo para un chat) o «Borrar» (las del usuario).
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
    sola: bool,
) {
    let m = 16.0 * e;
    let x = caja.x as f32 + m;
    let ancho = (caja.ancho as f32 - 2.0 * m).max(1.0);
    let mut arriba = caja.y as f32 + m;
    let mut abajo = caja.abajo() as f32 - m;
    let claro = es_claro(tema);
    let (gris, gris_sobre) = grises(tema, claro);

    // El nombre en negrita y, detras, su grupo en gris.
    if !sola {
        let tam = 15.0 * e;
        let (nw, nh) = crate::lecciones::ui::medir_negrita(p, &f.nombre, tam, ancho * 0.75);
        let nw = nw.min(ancho * 0.75);
        crate::lecciones::ui::negrita(p, &f.nombre, x, arriba, tam, ancho * 0.75, tema.texto);
        let grupo = format!(" · {}", rotulo_del_grupo(&f.grupo, textos));
        let (_, gh) = p.medir_texto(&grupo, 13.0 * e);
        p.texto_linea(
            &grupo,
            x + nw,
            arriba + (nh - gh).max(0.0) / 2.0,
            13.0 * e,
            (ancho - nw).max(0.0),
            tema.apagado,
        );
        arriba += nh + 12.0 * e;
    }

    // Abajo del todo, «Adjuntar a …» y, si es del usuario, «Borrar» (o,
    // si vino de PinDesk Plantillas, «Abrir en Plantillas»: se borra alli).
    let de_plantillas = pindesk_plantillas::carpeta::es_de_plantillas(&f.id);
    let con_borrar = !f.de_serie;
    if g.destino.is_some() || con_borrar {
        let alto_boton = ALTO_ADJUNTAR as f32 * e;
        let y = abajo - alto_boton;
        let ancho_borrar = match (con_borrar, g.destino.is_some()) {
            (false, _) => 0.0,
            (true, true) => (ancho * 0.34).min(190.0 * e),
            (true, false) => ancho.min(260.0 * e),
        };
        if let Some(d) = &g.destino {
            let boton = rect_de(RectF {
                x,
                y,
                ancho: ancho - ancho_borrar - if con_borrar { 8.0 * e } else { 0.0 },
                alto: alto_boton,
            });
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
            let tw = tw.min(boton.ancho as f32 - lado - 40.0 * e).max(0.0);
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
        }
        if con_borrar {
            let boton = rect_de(RectF {
                x: x + ancho - ancho_borrar,
                y,
                ancho: ancho_borrar,
                alto: alto_boton,
            });
            let rojo = if de_plantillas {
                crate::caja_dibujo::hex(0x5b5bd6)
            } else {
                crate::caja_dibujo::hex(0xd9534f)
            };
            p.rellenar_redondeado(
                rf(boton),
                12.0 * e,
                if g.borrar_pedido || boton.contiene(g.raton) {
                    rojo
                } else {
                    con_alfa(rojo, 0.75)
                },
            );
            let rotulo = if de_plantillas {
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("app", pindesk_plantillas::NOMBRE_PRODUCTO);
                textos.t_args("figuras-abrir-en-plantillas", &args)
            } else if g.borrar_pedido {
                textos.t("figuras-borrar-seguro")
            } else {
                textos.t("figuras-borrar")
            };
            let (tw, th) = p.medir_texto(&rotulo, 13.5 * e);
            let tw = tw.min(boton.ancho as f32 - 16.0 * e).max(0.0);
            p.texto_linea(
                &rotulo,
                boton.x as f32 + (boton.ancho as f32 - tw) / 2.0,
                boton.y as f32 + (boton.alto as f32 - th) / 2.0,
                13.5 * e,
                tw,
                Color::BLANCO,
            );
            zonas.push((
                boton,
                if de_plantillas {
                    Zona::AbrirEnPlantillas
                } else {
                    Zona::Borrar
                },
            ));
        }
        abajo = y - 12.0 * e;
    }

    // Encima, los parametros de la plantilla, si los lleva.
    if let Some(ps) = g
        .parametros
        .as_ref()
        .filter(|ps| f.clave.as_deref() == Some(ps.clave()))
    {
        let lista = ps.visibles();
        let columnas = if ancho >= 520.0 * e { 2 } else { 1 };
        let filas = lista.len().div_ceil(columnas);
        let alto_fila = ALTO_PARAMETRO * e;
        let hueco = 8.0 * e;
        let error = g
            .previa
            .as_ref()
            .filter(|pv| pv.id == f.id)
            .and_then(|pv| pv.error.clone());
        let alto_titulo = 30.0 * e;
        let alto_bloque = alto_titulo
            + filas as f32 * (alto_fila + hueco)
            + if error.is_some() { 22.0 * e } else { 0.0 };
        let y0 = abajo - alto_bloque;
        // «Parámetros», cuantas hojas salen y, si se toco algo, «Restablecer».
        let tam_t = 14.5 * e;
        let titulo = textos.t("figuras-param-titulo");
        let (_, th) = crate::lecciones::ui::medir_negrita(p, &titulo, tam_t, ancho);
        crate::lecciones::ui::negrita(p, &titulo, x, y0, tam_t, ancho * 0.5, tema.texto);
        let mut derecha = x + ancho;
        if !ps.de_fabrica() {
            let rotulo = textos.t("figuras-param-restablecer");
            let (rw, rh) = p.medir_texto(&rotulo, 13.0 * e);
            let r = rect_de(RectF {
                x: derecha - rw - 4.0 * e,
                y: y0 + (th - rh) / 2.0 - 4.0 * e,
                ancho: rw + 8.0 * e,
                alto: rh + 8.0 * e,
            });
            let azul = crate::caja_dibujo::hex(0x0a84ff);
            p.texto(
                &rotulo,
                r.x as f32 + 4.0 * e,
                r.y as f32 + 4.0 * e,
                13.0 * e,
                if r.contiene(g.raton) {
                    azul
                } else {
                    con_alfa(azul, 0.85)
                },
            );
            zonas.push((r, Zona::Restablecer));
            derecha = r.x as f32 - 12.0 * e;
        }
        let hojas = match ps.hojas() {
            1 => textos.t("figuras-ventana-una-hoja"),
            n => {
                let mut a = fluent_bundle::FluentArgs::new();
                a.set("n", n);
                textos.t_args("figuras-ventana-hojas", &a)
            }
        };
        let (hw, hh) = p.medir_texto(&hojas, 13.0 * e);
        p.texto(
            &hojas,
            derecha - hw,
            y0 + (th - hh) / 2.0,
            13.0 * e,
            tema.apagado,
        );
        let ancho_col = (ancho - hueco * 2.0 * (columnas - 1) as f32) / columnas as f32;
        for (k, par) in lista.iter().enumerate() {
            let (col, fila) = (k % columnas, k / columnas);
            let celda = RectF {
                x: x + col as f32 * (ancho_col + hueco * 2.0),
                y: y0 + alto_titulo + fila as f32 * (alto_fila + hueco),
                ancho: ancho_col,
                alto: alto_fila,
            };
            pintar_parametro(
                p,
                ps,
                *par,
                celda,
                g,
                tema,
                e,
                textos,
                zonas,
                (gris, gris_sobre),
            );
        }
        if let Some(err) = error {
            p.texto_linea(
                &err,
                x,
                y0 + alto_bloque - 18.0 * e,
                12.0 * e,
                ancho,
                crate::caja_dibujo::hex(0xd9534f),
            );
        }
        abajo = y0 - 14.0 * e;
    }

    // --- El dibujo en grande, llenando lo que queda. Se pinta de nuevo a
    // cada aumento (son sus ordenes, no un mapa de bits): nitido siempre.
    let papel = RectF {
        x,
        y: arriba,
        ancho,
        alto: (abajo - arriba).max(60.0 * e),
    };
    let radio = 14.0 * e;
    p.rellenar_redondeado(papel, radio, con_alfa(tema.texto, 0.10));
    p.rellenar_redondeado(encoger(papel, 1.0), radio - 1.0, tema.papel);
    let interior = encoger(papel, 14.0 * e);
    let dibujo = g
        .previa
        .as_ref()
        .filter(|pv| pv.id == f.id)
        .and_then(|pv| pv.dibujo.as_ref())
        .or(f.dibujo.as_ref());
    zonas.push((rect_de(papel), Zona::Dibujo));
    if let Some(d) = dibujo {
        let ci = rect_de(interior);
        g.caja_dibujo.set(ci);
        let (w, h) = (
            (d.caja.2 - d.caja.0).max(1.0),
            (d.caja.3 - d.caja.1).max(1.0),
        );
        let (dx, dy, dw, dh) = g.encuadre.destino(
            (ci.x as f32, ci.y as f32, ci.ancho as f32, ci.alto as f32),
            w,
            h,
        );
        p.empujar_recorte(encoger(papel, 1.0));
        pintar_lienzo(
            p,
            d,
            RectF {
                x: dx,
                y: dy,
                ancho: dw,
                alto: dh,
            },
            None,
        );
        p.soltar_recorte();
    }

    // La pastilla del aumento, abajo a la derecha: −, el tanto por ciento
    // (pulsarlo encaja) y +. A su izquierda, mientras esta encajada, como se
    // acerca.
    let alto_z = ALTO_ZOOM * e;
    let (bw, pw) = (32.0 * e, 58.0 * e);
    let zoom = RectF {
        x: papel.x + papel.ancho - 10.0 * e - (2.0 * bw + pw),
        y: papel.y + papel.alto - 10.0 * e - alto_z,
        ancho: 2.0 * bw + pw,
        alto: alto_z,
    };
    p.rellenar_redondeado(
        encoger(zoom, -1.0),
        alto_z / 2.0 + 1.0,
        con_alfa(tema.texto, 0.12),
    );
    p.rellenar_redondeado(zoom, alto_z / 2.0, if claro { gris } else { tema.barra });
    let por_ciento = format!("{} %", (g.encuadre.aumento * 100.0).round() as u32);
    let partes = [
        ("−".to_string(), bw, Zona::ZoomMenos),
        (por_ciento, pw, Zona::ZoomEncajar),
        ("+".to_string(), bw, Zona::ZoomMas),
    ];
    let mut zx = zoom.x;
    for (rotulo, w, zona) in partes {
        let r = rect_de(RectF {
            x: zx,
            y: zoom.y,
            ancho: w,
            alto: alto_z,
        });
        if r.contiene(g.raton) {
            p.rellenar_redondeado(rf(r), alto_z / 2.0, gris_sobre);
        }
        let tam = if zona == Zona::ZoomEncajar {
            12.5 * e
        } else {
            17.0 * e
        };
        let (tw, th) = p.medir_texto(&rotulo, tam);
        p.texto(
            &rotulo,
            r.x as f32 + (r.ancho as f32 - tw) / 2.0,
            r.y as f32 + (r.alto as f32 - th) / 2.0,
            tam,
            tema.texto,
        );
        zonas.push((r, zona));
        zx += w;
    }
    if g.encuadre.encajado() {
        let pista = textos.t("figuras-zoom-pista");
        let libre = zoom.x - papel.x - 24.0 * e;
        if libre > 120.0 * e {
            let (_, ph) = p.medir_texto(&pista, 11.5 * e);
            p.texto_linea(
                &pista,
                papel.x + 14.0 * e,
                zoom.y + (alto_z - ph) / 2.0,
                11.5 * e,
                libre,
                tema.apagado,
            );
        }
    }
}

/// El rotulo de un parametro.
fn rotulo_de(par: Parametro, textos: &Catalogo) -> String {
    textos.t(match par {
        Parametro::Desde => "figuras-ventana-desde",
        Parametro::Hasta => "figuras-ventana-hasta",
        Parametro::PorHoja => "figuras-param-por-hoja",
        Parametro::SemanasDesde => "figuras-ventana-semanas-desde",
        Parametro::SemanasHasta => "figuras-ventana-semanas-hasta",
        Parametro::Mes => "figuras-ventana-mes",
        Parametro::Ano => "figuras-param-ano",
        Parametro::Meses => "figuras-ventana-meses",
    })
}

/// El valor como se ensena: «—» el mes sin poner y «todo en una hoja».
fn valor_de(ps: &Parametros, par: Parametro) -> String {
    match (par, ps.valor(par)) {
        (Parametro::Mes | Parametro::PorHoja, 0) => "—".to_string(),
        (_, v) => v.to_string(),
    }
}

/// **Un parametro**: su rotulo a la izquierda y, a la derecha, la
/// pastilla con −, el numero (pulsarlo deja escribirlo) y +.
#[allow(clippy::too_many_arguments)]
fn pintar_parametro(
    p: &Pintor,
    ps: &Parametros,
    par: Parametro,
    caja: RectF,
    g: &Galeria,
    tema: &Tema,
    e: f32,
    textos: &Catalogo,
    zonas: &mut Vec<(Rect, Zona)>,
    (gris, gris_sobre): (Color, Color),
) {
    let (bw, vw) = (34.0 * e, 64.0 * e);
    let pastilla = RectF {
        x: caja.x + caja.ancho - (2.0 * bw + vw),
        ..caja
    };
    let rotulo = rotulo_de(par, textos);
    let (_, rh) = p.medir_texto(&rotulo, 13.0 * e);
    p.texto_linea(
        &rotulo,
        caja.x,
        caja.y + (caja.alto - rh) / 2.0,
        13.0 * e,
        (pastilla.x - caja.x - 8.0 * e).max(0.0),
        tema.texto,
    );
    p.rellenar_redondeado(pastilla, pastilla.alto / 2.0, gris);
    let (lo, hi) = ps.rango(par);
    let v = ps.valor(par);
    let escribiendo = g.foco == Some(par);
    let partes = [
        (Zona::Paso(par, -1), bw, v > lo),
        (Zona::Valor(par), vw, true),
        (Zona::Paso(par, 1), bw, v < hi),
    ];
    let mut x = pastilla.x;
    for (zona, w, activo) in partes {
        let r = rect_de(RectF {
            x,
            y: pastilla.y,
            ancho: w,
            alto: pastilla.alto,
        });
        x += w;
        let (texto, tam) = match zona {
            Zona::Paso(_, -1) => ("−".to_string(), 18.0 * e),
            Zona::Paso(..) => ("+".to_string(), 18.0 * e),
            _ if escribiendo => (format!("{}▏", g.tecleado), 14.0 * e),
            _ => (valor_de(ps, par), 14.0 * e),
        };
        if escribiendo && matches!(zona, Zona::Valor(_)) {
            let azul = crate::caja_dibujo::hex(0x0a84ff);
            p.rellenar_redondeado(encoger(rf(r), 1.0 * e), 8.0 * e, azul);
            p.rellenar_redondeado(encoger(rf(r), 2.5 * e), 6.5 * e, tema.papel);
        } else if activo && r.contiene(g.raton) {
            p.rellenar_redondeado(rf(r), r.alto as f32 / 2.0, gris_sobre);
        }
        let (tw, th) = p.medir_texto(&texto, tam);
        p.texto(
            &texto,
            r.x as f32 + (r.ancho as f32 - tw) / 2.0,
            r.y as f32 + (r.alto as f32 - th) / 2.0,
            tam,
            if activo { tema.texto } else { tema.apagado },
        );
        zonas.push((r, zona));
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

/// **La escena de un lienzo con la figura dentro**, para adjuntarla al chat:
/// una hoja, o las de una plantilla con sus parametros en fila (como las
/// pone la ventana de figuras del lienzo), desde el origen.
pub(super) fn escena_con_hojas(hojas: &[FiguraGuardada]) -> pindesk_motor2d::Escena {
    let mut escena = pindesk_motor2d::Escena::nueva();
    if let [f] = hojas {
        let (w, h) = f.tamano();
        biblioteca::estampar_en_escena(
            &mut escena,
            f,
            pindesk_motor2d::vector::Punto2::nuevo(w / 2.0, h / 2.0),
        );
    } else if !hojas.is_empty() {
        let (w, h) = crate::asistente::al_lienzo::medida_de_las_hojas(hojas);
        crate::asistente::al_lienzo::colocar_hojas_en_medio(&mut escena, hojas, (w / 2.0, h / 2.0));
    }
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

/// Muestras en PNG de la ventana, de dia y de noche (2-nov-2026).
#[cfg(test)]
#[path = "muestra_figuras.rs"]
mod muestra;

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El buzon del chat es uno para todo el programa: las pruebas que
    /// adjuntan van de una en una.
    static BUZON: Mutex<()> = Mutex::new(());

    #[test]
    fn la_figura_adjuntada_entra_entera_en_su_lienzo() {
        let catalogo = pindesk_motor2d::figuras_de_serie::catalogo();
        // La figura estampada es la misma, pieza a pieza.
        let e = escena_con_hojas(&catalogo[..1]);
        assert_eq!(
            e.visibles().count(),
            catalogo[0].elementos.iter().filter(|x| !x.borrado).count()
        );
        let c = e.caja().unwrap();
        assert!(c.0 >= -0.5 && c.1 >= -0.5, "desde el origen: {c:?}");
        // Varias hojas van en fila, todas enteras.
        let hojas = alargar(
            "lista-de-tareas",
            &Intervalo {
                desde: Some(1),
                hasta: Some(40),
                de_a: Some(10),
                ..Intervalo::default()
            },
        )
        .unwrap();
        let e = escena_con_hojas(&hojas);
        let piezas: usize = hojas
            .iter()
            .map(|h| h.elementos.iter().filter(|x| !x.borrado).count())
            .sum();
        assert_eq!(e.visibles().count(), piezas);
        let c = e.caja().unwrap();
        assert!(
            c.0 >= -0.5 && c.2 - c.0 > 3.0 * hojas[0].tamano().0,
            "en fila: {c:?}"
        );
        // Caso negativo: sin hojas, un lienzo vacio.
        assert_eq!(escena_con_hojas(&[]).visibles().count(), 0);
    }

    #[test]
    fn sin_chat_de_destino_ni_el_doble_clic_ni_intro_adjuntan() {
        let _buzon = BUZON.lock().unwrap_or_else(|e| e.into_inner());
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
        let (proyecto, hojas) = super::super::tomar_figura_pedida().unwrap();
        assert_eq!(proyecto, "p-prueba-figuras");
        assert_eq!(
            hojas.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(),
            ["mia"]
        );
    }

    /// La galeria con el catalogo de serie y la plantilla `id` elegida.
    fn con_la_de_serie(id: &str) -> Galeria {
        let mut g = Galeria::default();
        g.poner_figuras(&pindesk_motor2d::figuras_de_serie::catalogo(), None);
        g.elegida = g.figuras.iter().position(|f| f.id == id);
        assert!(g.elegida.is_some(), "{id} es de serie");
        g.preparar();
        g
    }

    #[test]
    fn la_plantilla_lleva_sus_parametros_y_se_adjunta_como_se_ve() {
        let _buzon = BUZON.lock().unwrap_or_else(|e| e.into_inner());
        let t = Catalogo::nuevo(Idioma::Espanol);
        let mut g = con_la_de_serie("serie-lista-de-tareas");
        let ps = g.parametros.as_ref().expect("la lista de tareas se alarga");
        assert!(ps.de_fabrica());
        assert_eq!(g.previa.as_ref().unwrap().hojas.len(), 1);
        // Del 1 al 40 de 10 en 10: cuatro hojas, en el dibujo y al adjuntar.
        g.parametros.as_mut().unwrap().poner(Parametro::Hasta, 40);
        g.paso_parametro(Parametro::PorHoja, 1);
        g.parametros.as_mut().unwrap().poner(Parametro::PorHoja, 10);
        g.preparar();
        let previa = g.previa.as_ref().unwrap();
        assert_eq!(previa.hojas.len(), 4);
        assert!(previa.error.is_none());
        let d = previa.dibujo.as_ref().unwrap();
        assert!(d.caja.2 - d.caja.0 > 3.0 * (d.caja.3 - d.caja.1) * 0.5);
        g.destino = Some(Destino {
            proyecto: "p-prueba-parametros".into(),
            chat: "Obra".into(),
        });
        assert!(g.adjuntar(&t));
        let (_, hojas) = super::super::tomar_figura_pedida().unwrap();
        assert_eq!(hojas.len(), 4);
        // Escribir un numero lo pone, acotado.
        g.foco = Some(Parametro::Hasta);
        g.tecleado = "5000".into();
        g.confirmar_tecleado();
        assert_eq!(g.parametros.as_ref().unwrap().valor(Parametro::Hasta), 999);
        assert!(g.foco.is_none() && g.tecleado.is_empty());
        // Caso negativo: una figura que no se alarga no lleva parametros.
        let g = con_la_de_serie("serie-diario");
        assert!(g.parametros.is_none());
        assert_eq!(g.previa.as_ref().unwrap().hojas[0].id, "serie-diario");
    }

    #[test]
    fn estrecha_la_figura_se_ve_sola_y_ancha_al_lado() {
        let t = Catalogo::nuevo(Idioma::Espanol);
        let mut g = con_la_de_serie("serie-lista-de-tareas");
        let figura = Rect {
            x: 10,
            y: 120,
            ancho: 150,
            alto: 140,
        };
        g.zonas.borrow_mut().push((figura, Zona::Figura(0)));
        let mut vivo = true;
        // Ancha: elegir no cambia de pantalla.
        g.una_cada_vez.set(false);
        g.pulsar(Punto { x: 20, y: 130 }, &mut vivo, &t);
        assert!(!g.viendo && !g.sola());
        // Estrecha: la figura elegida ocupa la ventana y «‹» vuelve.
        g.una_cada_vez.set(true);
        g.ultimo_clic = None;
        g.pulsar(Punto { x: 20, y: 130 }, &mut vivo, &t);
        assert!(g.sola());
        g.zonas.borrow_mut().clear();
        let volver = Rect {
            x: 0,
            y: 0,
            ancho: 80,
            alto: 40,
        };
        g.zonas.borrow_mut().push((volver, Zona::Volver));
        g.pulsar(Punto { x: 5, y: 5 }, &mut vivo, &t);
        assert!(!g.sola() && vivo, "vuelve a la rejilla sin cerrar");
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
    fn se_buscan_los_textos_que_lleva() {
        let catalogo = pindesk_motor2d::figuras_de_serie::catalogo();
        let con_textos = catalogo
            .iter()
            .find(|f| letras_de(f).len() > 1)
            .expect("alguna de serie lleva textos");
        let v = vista_de(con_textos);
        let palabra = v.letras[0].split_whitespace().next().unwrap().to_string();
        assert!(casa(&v, &palabra));
        // Caso negativo: una figura sin textos no lista ninguno.
        let sin = FiguraGuardada {
            id: "y".into(),
            nombre: "Nada".into(),
            elementos: Vec::new(),
        };
        assert!(letras_de(&sin).is_empty());
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
