//! La ventana del timeline.
//!
//! Arriba, la **barra comun** de Galeria, Lecciones, Tareas y Timeline
//! (`crate::cabecera`): titulo, el buscador y, como botones propios, las
//! pestanas «Hoy», «Momentos» y «Estado» y «Exportar». El usuario lo pidio
//! asi el 5-oct («que todos en la parte superior mantengan la barra de
//! busqueda y las funciones unicas de cada parte») y, para lo de dentro,
//! mando tres capturas de «Mis publicaciones» de WeChat: «algo asi, mejora
//! la parte de timeline».
//!
//! - **Hoy**: la linea de tiempo vertical de las ultimas 24 horas (la hora,
//!   un punto en la linea y la tarjeta del momento) y abajo, como en un
//!   chat, la caja para apuntar: escribir y Intro, el microfono o una foto.
//! - **Momentos** (captura 2): el archivo por ano y mes; cada dia, su numero
//!   grande y en fila las miniaturas de lo que paso. Clic en una abre su
//!   detalle; en el numero, el dia entero.
//! - **Estado** (captura 1): una tarjeta por mes con sus dias al reves y un
//!   circulo con el emoticono (o la foto) de cada dia con algo. Clic abre el
//!   dia; Ctrl+clic lo elige para exportar varios juntos.
//! - **Un dia** del archivo, sobre la pestana desde la que se abrio.
//! - **El detalle** de un momento (captura 3): la foto entera sobre negro,
//!   la fecha arriba, el texto abajo y flechas para sus fotos.
//! - **La busqueda**: con algo escrito arriba, el contenido son los
//!   resultados de todo el timeline, por dias.
//!
//! **Exportar** saca lo que se ve (las ultimas 24 horas, el dia abierto, lo
//! encontrado o los dias elegidos) en HTML o PDF, con la misma linea de
//! tiempo.
//!
//! **Teclado**: Intro apunta, Mayus+Intro parte el renglon, Ctrl+M dicta,
//! Ctrl+V pega texto o una imagen, flecha arriba con la caja vacia corrige el
//! ultimo momento, Ctrl+Z deshace un borrado mientras dura su aviso, Ctrl+F
//! va al buscador, Ctrl+1/2/3 cambian de pestana. En el detalle, ←/→ pasan
//! de foto y ↑/↓ o RePag/AvPag de momento. Esc va por capas: cierra el
//! menu, el detalle, vacia la busqueda, deja de corregir, vacia la caja,
//! cierra el dia, vuelve a «Hoy» y, por ultimo, cierra.
//!
//! Una sola ventana, en su propio hilo, como la de tareas. Se pone al dia
//! sola mirando cada segundo la huella del fichero.

#![forbid(unsafe_code)]

mod archivo;
mod compartir;
mod historia;
mod estado_mes;
mod lecciones;
#[cfg(test)]
mod muestras;
#[cfg(test)]
mod pruebas;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use pixpin_geom::Rect;
use pixpin_render::icono::material as mi;
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Idioma, Ubicacion};
use pixpin_timeline::Momento;
use pixpin_timeline::almacen::Almacen;
use pixpin_timeline::dias::{self, Dia, Mes};
use pixpin_timeline::frases;
use pixpin_timeline::html::{Opciones, Seccion};

use super::dictar::{Dicho, Dictado, Salida};
use super::disposicion as dis;
use super::exportar::{self, Formato};
use crate::caja_dibujo::hex;
use crate::lecciones::ui;
use crate::overlay::Recursos;
use crate::ventanita::{Botones, TEXTO, centrado, dentro};

// Los colores del rediseno v2, como tareas y lecciones (ventana oscura).
const FONDO_V: Color = hex(0x1C1C1E);
const TARJETA: Color = hex(0x2A2A2D);
const TEXTO_V: Color = hex(0xF5F5F7);
const CUERPO: Color = hex(0xD1D1D6);
const GRIS: Color = hex(0x98989D);
/// El color del timeline: los puntos, la linea viva y lo elegido.
const ACENTO: Color = hex(0x0A84FF);
const LINEA: Color = hex(0x3A3A3D);
const ROJO: Color = hex(0xFF453A);

const VK_ESCAPE: u32 = 0x1B;
const VK_ENTRAR: u32 = 0x0D;
const VK_IZQUIERDA: u32 = 0x25;
const VK_ARRIBA: u32 = 0x26;
const VK_DERECHA: u32 = 0x27;
const VK_ABAJO: u32 = 0x28;
const VK_PAG_ARRIBA: u32 = 0x21;
const VK_PAG_ABAJO: u32 = 0x22;
const VK_1: u32 = 0x31;
const VK_2: u32 = 0x32;
const VK_3: u32 = 0x33;
const VK_4: u32 = 0x34;
const VK_F: u32 = 0x46;
const VK_M: u32 = 0x4D;
const VK_V: u32 = 0x56;
const VK_Z: u32 = 0x5A;

/// Mas ancha que antes (760): las diez columnas de dias de «Estado» y las
/// cinco miniaturas por renglon de «Momentos» tienen que caber.
const ANCHO_VENTANA: u32 = 900;
const ALTO_VENTANA: u32 = 900;

/// Medidas, en pixeles logicos.
const MARGEN: f32 = 16.0;
/// La columna de la hora.
const HORA: f32 = 48.0;
/// De la hora a la linea, y de la linea a la tarjeta.
const AL_RIEL: f32 = 18.0;
const PAD: f32 = 14.0;
const ENTRE: f32 = 12.0;
const SEPARADOR: f32 = 40.0;
const FOTO_W: f32 = 176.0;
const FOTO_H: f32 = 132.0;
const CHAPA_VOZ: f32 = 32.0;
const BOTON: f32 = 40.0;
/// La caja de escribir no crece mas que esto: lo largo se desplaza dentro.
const CAJA_MAX: f32 = 150.0;
const PEGADA: f32 = 52.0;
/// La barra de «volver» encima de un dia abierto.
const BARRA_DIA: f32 = 56.0;

const DURA_AVISO: Duration = Duration::from_millis(2_800);
const DURA_DESHACER: Duration = Duration::from_secs(6);

static ABIERTA: AtomicIsize = AtomicIsize::new(0);
/// La pestana que pidio otra entrada (las de lecciones) para la ventana ya
/// abierta: 0 ninguna, si no `Pestana as u8 + 1`. La ventana la mira en su
/// bucle.
static PEDIDA: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub(super) fn abrir(idioma: Idioma, ubicacion: Ubicacion) {
    abrir_en(idioma, ubicacion, Pestana::Hoy);
}

pub(super) fn abrir_en(idioma: Idioma, ubicacion: Ubicacion, pestana: Pestana) {
    PEDIDA.store(pestana as u8 + 1, Ordering::SeqCst);
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
        .name("timeline".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(idioma);
            let hecho = crate::dispositivo_perdido::con_recursos("timeline", |r| {
                bucle(r, &textos, &ubicacion, idioma)
            });
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo abrir la ventana del timeline");
            }
            ABIERTA.store(0, Ordering::SeqCst);
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo del timeline");
        ABIERTA.store(0, Ordering::SeqCst);
    }
}

/// Un momento o una leccion en una accion, por su id (un resumen de 64
/// bits, que la accion es `Copy`). No por su indice: tras releer el fichero
/// el indice puede ser otro, y se borraria o marcaria el equivocado (lo vio
/// un revisor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Clave(u64);

fn clave(id: &str) -> Clave {
    // FNV-1a de 64: estable y sin choques en la practica.
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in id.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01B3);
    }
    Clave(h)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Mover,
    Cerrar,
    Fondo,
    Enviar,
    Microfono,
    Imagen,
    QuitarPegada(usize),
    Pestana(Pestana),
    /// Cerrar el dia abierto y volver a la pestana.
    VolverDelDia,
    Exportar,
    Formato(Formato),
    SoltarMenu,
    EnfocarBusqueda,
    VaciarBusqueda,
    /// Abrir el dia (su numero, `Dia::numero`).
    AbrirDia(i64),
    /// Un dia de «Estado»: abrirlo, o con Ctrl elegirlo para exportar.
    DiaDelEstado(i64),
    /// Elegir o soltar el dia para exportar.
    ElegirDia(i64),
    QuitarEleccion,
    /// El momento `.0` de `Estado::momentos`.
    Borrar(Clave),
    Corregir(Clave),
    Oir(Clave),
    /// La foto `.1` del momento `.0`: pinearla.
    AbrirFoto(Clave, usize),
    /// Abrir el detalle del momento `.0`.
    AbrirDetalle(Clave),
    /// Marcar o desmarcar el momento `.0` como leccion aprendida.
    Leccion(Clave),
    /// Abrir el detalle de la leccion `.0` de `Estado::lecciones`.
    AbrirLeccion(Clave),
    /// «Me volvio a pasar» en la leccion `.0`.
    OtraVez(Clave),
    /// El menu de compartir como imagen de un momento o de una leccion.
    MenuCompartirMomento(Clave),
    MenuCompartirLeccion(Clave),
    Compartir(Destino),
    /// Lo mismo, con lo que ensena el detalle.
    OirDetalle,
    PinearDetalle,
    LeccionDetalle,
    GravedadDetalle,
    OtraVezDetalle,
    CompartirDetalle,
    /// Los chips de «Lecciones»: filtrar por gravedad y ordenar (`true` =
    /// las mas repetidas primero).
    FiltroGravedad(Option<i64>),
    OrdenLecciones(bool),
    /// Lo buscado tambien esta en lecciones: ir a verlas.
    VerLeccionesBuscadas,
    CerrarDetalle,
    FotoAnterior,
    FotoSiguiente,
    IrAFoto(usize),
    MomentoAnterior,
    MomentoSiguiente,
    Deshacer,
    AbrirExportado,
    DejarDeCorregir,
}

/// Las pestanas de la barra de arriba.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pestana {
    Hoy,
    Momentos,
    Estado,
    /// Las lecciones aprendidas, en tarjetas (5-oct-2026).
    Lecciones,
}

impl Pestana {
    fn de_numero(n: u8) -> Option<Pestana> {
        match n {
            1 => Some(Pestana::Hoy),
            2 => Some(Pestana::Momentos),
            3 => Some(Pestana::Estado),
            4 => Some(Pestana::Lecciones),
            _ => None,
        }
    }
}

/// Compartir una tarjeta como imagen: al portapapeles o a un fichero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Destino {
    Copiar,
    Guardar,
}

/// Algo que se abre en el detalle o se comparte: un momento o una leccion,
/// por su id (los indices cambian al releer).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Item {
    Momento(String),
    Leccion(String),
}

/// Lo que ocupa el contenido ahora, por orden de prioridad: lo buscado tapa
/// el dia abierto, y el dia abierto tapa la pestana.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modo {
    Hoy,
    Momentos,
    Estado,
    Lecciones,
    Dia(Dia),
    Buscar,
}

/// El detalle abierto: la lista desde la que se abrio (sus ids, para que las
/// flechas de arriba y abajo sigan su orden), cual es y que foto se ve.
#[derive(Debug, Clone)]
struct Detalle {
    lista: Vec<Item>,
    pos: usize,
    foto: usize,
}

/// Una imagen esperando en la caja a que se apunte.
#[derive(Debug, Clone)]
struct Pegada {
    ruta: PathBuf,
    /// Un PNG escrito al pegar un mapa de bits: se borra al soltarla.
    temporal: bool,
}

/// Lo que va en la lista: un rotulo de dia o un momento (su indice).
#[derive(Debug, Clone, Copy)]
enum Pieza {
    Separador(Dia),
    Momento(usize),
}

struct Estado {
    almacen: Almacen,
    momentos: Vec<Momento>,
    huella: Option<(SystemTime, u64)>,
    pestana: Pestana,
    /// El dia abierto encima de la pestana.
    dia: Option<Dia>,
    detalle: Option<Detalle>,
    busqueda: ui::Campo,
    /// El foco en el buscador: las letras van a el.
    buscando: bool,
    /// Si Ctrl estaba pulsado en el ultimo clic (elegir dias en «Estado»).
    ctrl: bool,
    elegidos: BTreeSet<Dia>,
    campo: ui::Campo,
    pegadas: Vec<Pegada>,
    /// Lo dictado mientras habia algo escrito: su texto se metio en la caja
    /// y su audio espera aqui a Intro.
    audio_pendiente: Option<Dicho>,
    /// El id del momento que se esta corrigiendo.
    corrigiendo: Option<String>,
    dictado: Dictado,
    scroll: f32,
    alto_contenido: f32,
    /// Pegada al final (lo nuevo): asi sigue al apuntar. Se suelta al subir.
    abajo: bool,
    botones: Botones<Accion>,
    aviso: Option<(String, Instant)>,
    deshacer: Option<Momento>,
    exportado: Option<PathBuf>,
    /// El menu de exportar abierto.
    menu: bool,
    /// El menu de compartir como imagen: de que y donde.
    menu_compartir: Option<(Item, (f32, f32))>,
    /// Lo pedido en ese menu, para el bucle (que tiene el motor de dibujo).
    compartir: Option<(Item, Destino)>,
    /// Las lecciones aprendidas, para su pestana y para marcar momentos.
    lecciones: Vec<lecciones::TarjetaLeccion>,
    /// La cuenta de cambios de lecciones con la que se leyeron, y su firma.
    lecciones_cuenta: u64,
    lecciones_firma: Vec<Option<SystemTime>>,
    raiz: PathBuf,
    /// El codigo de este equipo: el sello de las lecciones que se hacen aqui.
    aparato: String,
    moviendo: Option<(pixpin_geom::Punto, Rect)>,
    minis: crate::miniaturas::Miniaturas,
    /// La foto del detalle a tamano de ventana. Una sola a la vez: se tira
    /// y se vuelve a hacer al cambiar de foto, porque una foto grande son
    /// decenas de megas y el detalle solo ensena una.
    grande: crate::miniaturas::Miniaturas,
    grande_ruta: Option<PathBuf>,
    /// El filtro de gravedad de «Lecciones» (`None` = todas) y su orden.
    filtro_gravedad: Option<i64>,
    orden_lecciones: super::tarjetas::Orden,
    /// La disposicion de «Momentos», hecha una vez por cambio.
    cache_archivo: std::cell::RefCell<Option<std::rc::Rc<archivo::CacheArchivo>>>,
    hwnd: Option<windows::Win32::Foundation::HWND>,
    desfase: i64,
    cuenta: u32,
    pdf: Option<Receiver<Result<PathBuf, String>>>,
    ingles: bool,
}

impl Estado {
    fn nuevo(ubicacion: &Ubicacion, idioma: Idioma) -> Estado {
        let raiz = ubicacion.raiz().to_path_buf();
        // El mismo codigo de equipo que usan el chat y las lecciones.
        let aparato = pixpin_proyecto::identidad::Identidad::leer_o_crear(&raiz, "PC")
            .map(|i| i.yo.codigo())
            .unwrap_or_default();
        let mut e = Self::con_almacen(Almacen::en(&raiz), idioma, raiz, aparato);
        e.releer_lecciones();
        e
    }

    fn con_almacen(almacen: Almacen, idioma: Idioma, raiz: PathBuf, aparato: String) -> Estado {
        let desfase = pixpin_shell::entorno::desfase_local_ms();
        Estado {
            momentos: almacen.leer(),
            huella: almacen.huella(),
            almacen,
            pestana: Pestana::Hoy,
            dia: None,
            detalle: None,
            busqueda: ui::Campo::default(),
            buscando: false,
            ctrl: false,
            elegidos: BTreeSet::new(),
            campo: ui::Campo::default(),
            pegadas: Vec::new(),
            audio_pendiente: None,
            corrigiendo: None,
            dictado: Dictado::Nada,
            scroll: 0.0,
            alto_contenido: 0.0,
            abajo: true,
            botones: Botones::default(),
            aviso: None,
            deshacer: None,
            exportado: None,
            menu: false,
            menu_compartir: None,
            compartir: None,
            lecciones: Vec::new(),
            lecciones_cuenta: u64::MAX,
            lecciones_firma: Vec::new(),
            raiz,
            aparato,
            moviendo: None,
            minis: crate::miniaturas::Miniaturas::con_lado(FOTO_W as u32 * 2),
            grande: crate::miniaturas::Miniaturas::con_lado(1),
            grande_ruta: None,
            cache_archivo: std::cell::RefCell::new(None),
            filtro_gravedad: None,
            orden_lecciones: super::tarjetas::Orden::Recientes,
            hwnd: None,
            desfase,
            cuenta: 0,
            pdf: None,
            ingles: idioma == Idioma::Ingles,
        }
    }

    fn decir(&mut self, t: String) {
        self.aviso = Some((t, Instant::now()));
        self.deshacer = None;
        self.exportado = None;
    }

    fn dura_el_aviso(&self) -> Duration {
        if self.deshacer.is_some() || self.exportado.is_some() {
            DURA_DESHACER
        } else {
            DURA_AVISO
        }
    }

    fn ahora() -> i64 {
        pixpin_shell::entorno::ahora_utc_ms()
    }

    fn hoy(&self) -> Dia {
        Dia::de_instante(Self::ahora(), self.desfase)
    }

    fn modo(&self) -> Modo {
        let buscando = !self.busqueda.texto.trim().is_empty();
        // En «Lecciones», lo buscado filtra sus tarjetas; en lo demas, busca
        // en todos los momentos.
        if self.pestana == Pestana::Lecciones {
            return Modo::Lecciones;
        }
        if buscando {
            return Modo::Buscar;
        }
        if let Some(d) = self.dia {
            return Modo::Dia(d);
        }
        match self.pestana {
            Pestana::Hoy => Modo::Hoy,
            Pestana::Momentos => Modo::Momentos,
            Pestana::Estado => Modo::Estado,
            Pestana::Lecciones => Modo::Lecciones,
        }
    }

    /// Lo que se ve en la linea de tiempo, en orden (vacio en las pestanas
    /// que se pintan a su manera).
    fn visibles(&self) -> Vec<usize> {
        let elegidos: Vec<&Momento> = match self.modo() {
            Modo::Hoy => dias::ultimas_24h(&self.momentos, Self::ahora()),
            Modo::Dia(d) => dias::del_dia(&self.momentos, d, self.desfase),
            Modo::Buscar => pixpin_timeline::buscar::buscar(&self.momentos, &self.busqueda.texto),
            Modo::Momentos | Modo::Estado | Modo::Lecciones => Vec::new(),
        };
        self.indices(&elegidos)
    }

    /// Los indices en `momentos` de `v`, con un mapa id → indice (con
    /// `position` en bucle era cuadratico, y se llama cada fotograma).
    fn indices(&self, v: &[&Momento]) -> Vec<usize> {
        let mapa: std::collections::HashMap<&str, usize> = self
            .momentos
            .iter()
            .enumerate()
            .map(|(i, m)| (m.id.as_str(), i))
            .collect();
        v.iter().filter_map(|m| mapa.get(m.id.as_str()).copied()).collect()
    }

    /// El momento de una clave, si sigue ahi.
    fn momento_de(&self, c: Clave) -> Option<usize> {
        self.momentos.iter().position(|m| clave(&m.id) == c)
    }

    /// La leccion de una clave, si sigue ahi.
    fn leccion_de(&self, c: Clave) -> Option<usize> {
        self.lecciones
            .iter()
            .position(|t| clave(&t.entrada.leccion.id) == c)
    }

    /// Lo que se ve ahora, en orden: lo que recorre el detalle.
    fn orden_actual(&self) -> Vec<Item> {
        let idx = match self.modo() {
            Modo::Momentos => {
                let a = pixpin_timeline::archivo::agrupar(&self.momentos, self.desfase);
                self.indices(&pixpin_timeline::archivo::en_orden(&a))
            }
            Modo::Lecciones => {
                return lecciones::visibles(self)
                    .into_iter()
                    .map(|k| Item::Leccion(self.lecciones[k].entrada.leccion.id.clone()))
                    .collect();
            }
            _ => self.visibles(),
        };
        idx.into_iter()
            .map(|i| Item::Momento(self.momentos[i].id.clone()))
            .collect()
    }

    /// Lo que ensena el detalle abierto.
    fn item_en_detalle(&self) -> Option<&Item> {
        let d = self.detalle.as_ref()?;
        d.lista.get(d.pos)
    }

    /// El momento del detalle abierto (su indice en `momentos`).
    fn en_detalle(&self) -> Option<usize> {
        match self.item_en_detalle()? {
            Item::Momento(id) => self.momentos.iter().position(|m| &m.id == id),
            Item::Leccion(_) => None,
        }
    }

    /// La leccion del detalle abierto (su indice en `lecciones`).
    fn leccion_en_detalle(&self) -> Option<usize> {
        match self.item_en_detalle()? {
            Item::Leccion(id) => self.indice_de_leccion(id),
            Item::Momento(_) => None,
        }
    }

    fn indice_de_leccion(&self, id: &str) -> Option<usize> {
        self.lecciones
            .iter()
            .position(|t| t.entrada.leccion.id == id)
    }

    fn abrir_item(&mut self, item: Item) {
        let mut lista = self.orden_actual();
        let pos = match lista.iter().position(|x| *x == item) {
            Some(p) => p,
            None => {
                lista = vec![item];
                0
            }
        };
        self.detalle = Some(Detalle {
            lista,
            pos,
            foto: 0,
        });
        self.menu = false;
        self.menu_compartir = None;
    }

    fn abrir_detalle(&mut self, i: usize) {
        if let Some(id) = self.momentos.get(i).map(|m| m.id.clone()) {
            self.abrir_item(Item::Momento(id));
        }
    }

    fn abrir_leccion(&mut self, k: usize) {
        if let Some(t) = self.lecciones.get(k) {
            let id = t.entrada.leccion.id.clone();
            self.abrir_item(Item::Leccion(id));
        }
    }

    /// Pasa al anterior (`-1`) o siguiente (`+1`) de la lista del detalle.
    /// En los bordes se queda donde esta.
    fn mover_detalle(&mut self, paso: isize) {
        if let Some(d) = self.detalle.as_mut() {
            let n = d.pos as isize + paso;
            if n >= 0 && (n as usize) < d.lista.len() {
                d.pos = n as usize;
                d.foto = 0;
            }
        }
    }

    /// Las fotos de lo que ensena el detalle.
    fn fotos_del_detalle(&self) -> Vec<PathBuf> {
        if let Some(i) = self.en_detalle() {
            return self.momentos[i]
                .fotos
                .iter()
                .map(|f| self.almacen.ruta(f))
                .collect();
        }
        if let Some(k) = self.leccion_en_detalle() {
            return self.lecciones[k].fotos.clone();
        }
        Vec::new()
    }

    /// **Pasar** en la historia, como en Instagram: a la foto siguiente (o
    /// anterior) y, al acabarse las fotos, al momento siguiente (o al
    /// anterior, en su ultima foto). En los bordes de la lista se queda.
    fn avanzar(&mut self, paso: isize) {
        let fotos = self.fotos_del_detalle().len();
        let Some(d) = self.detalle.as_mut() else {
            return;
        };
        if paso > 0 {
            if d.foto + 1 < fotos {
                d.foto += 1;
            } else {
                self.mover_detalle(1);
            }
        } else if d.foto > 0 {
            d.foto -= 1;
        } else if d.pos > 0 {
            self.mover_detalle(-1);
            let n = self.fotos_del_detalle().len();
            if let Some(d) = self.detalle.as_mut() {
                d.foto = n.saturating_sub(1);
            }
        }
    }

    /// La ruta de la foto que ensena el detalle.
    fn foto_del_detalle(&self) -> Option<PathBuf> {
        let k = self.detalle.as_ref()?.foto;
        self.fotos_del_detalle().get(k).cloned()
    }

    /// Lee otra vez las lecciones, si algo cambio: otra ventana o el movil
    /// guardaron, o se marco un momento aqui.
    fn releer_lecciones(&mut self) {
        let cuenta = crate::lecciones::almacen::cambios();
        self.lecciones = lecciones::cargar(&self.raiz);
        self.lecciones_cuenta = cuenta;
        let entradas: Vec<_> = self.lecciones.iter().map(|t| t.entrada.clone()).collect();
        self.lecciones_firma = crate::lecciones::almacen::firma(&self.raiz, &entradas);
    }

    fn cambiar_pestana(&mut self, p: Pestana) {
        self.pestana = p;
        self.dia = None;
        self.scroll = 0.0;
        self.abajo = p == Pestana::Hoy;
        self.menu = false;
        // Los dias elegidos solo valen en «Estado», donde se ven: fuera de
        // ella se olvidan, y exportar no se lleva una eleccion invisible.
        if p != Pestana::Estado {
            self.elegidos.clear();
        }
    }

    fn vaciar_busqueda(&mut self) {
        self.busqueda.poner("");
        self.buscando = false;
        self.scroll = 0.0;
        self.abajo = self.modo() == Modo::Hoy;
    }

    fn releer(&mut self) {
        self.momentos = self.almacen.leer();
        self.huella = self.almacen.huella();
    }

    /// Escribe todo tras borrar o corregir.
    fn guardar_todos(&mut self, textos: &Catalogo) {
        self.momentos.sort_by_key(|m| m.cuando);
        if let Err(e) = self.almacen.guardar_todos(&self.momentos) {
            tracing::warn!(?e, "timeline: no se pudo guardar");
            self.decir(textos.t("timeline-no-guardado"));
        }
        self.huella = self.almacen.huella();
    }

    fn soltar_pegadas(&mut self) {
        for p in self.pegadas.drain(..) {
            if p.temporal {
                let _ = std::fs::remove_file(&p.ruta);
            }
        }
    }

    fn vaciar_caja(&mut self) {
        self.campo.poner("");
        self.soltar_pegadas();
        if let Some(d) = self.audio_pendiente.take() {
            let _ = std::fs::remove_file(d.audio);
        }
        self.corrigiendo = None;
    }

    /// Vuelve a «Hoy» tal cual, para apuntar: sin dia abierto, sin
    /// busqueda y sin detalle.
    fn ir_a_hoy(&mut self) {
        if self.modo() != Modo::Hoy {
            self.busqueda.poner("");
            self.buscando = false;
            self.detalle = None;
            self.cambiar_pestana(Pestana::Hoy);
        }
    }

    /// **Apunta un momento nuevo** con lo que diga `texto`, las imagenes de
    /// la caja y, si lo hay, el audio. Devuelve si se apunto.
    fn apuntar(&mut self, texto: &str, audio: Option<Dicho>, textos: &Catalogo) -> bool {
        let cuando = audio.as_ref().map_or_else(Self::ahora, |d| d.desde);
        let (mut titulo, descripcion) = frases::partir(texto);
        if titulo.is_empty() {
            titulo = if audio.is_some() {
                textos.t("timeline-sin-titulo-voz")
            } else if !self.pegadas.is_empty() {
                textos.t("timeline-sin-titulo-foto")
            } else {
                return false;
            };
        }
        self.cuenta += 1;
        let id = pixpin_timeline::momento::id_nuevo(cuando, self.cuenta);
        let mut m = Momento::nuevo(id.clone(), cuando, titulo, descripcion);
        for (k, p) in std::mem::take(&mut self.pegadas).into_iter().enumerate() {
            let base = format!("{id}-{}", k + 1);
            let hecho = if p.temporal {
                self.almacen.mover_medio(&p.ruta, &base)
            } else {
                self.almacen.copiar_medio(&p.ruta, &base)
            };
            match hecho {
                Ok(n) => m.fotos.push(n),
                Err(e) => tracing::warn!(?e, ruta = %p.ruta.display(), "timeline: foto sin copiar"),
            }
        }
        if let Some(d) = audio {
            match self.almacen.mover_medio(&d.audio, &id) {
                Ok(n) => {
                    m.audio = Some(n);
                    m.duracion_ms = d.duracion_ms;
                }
                Err(e) => tracing::warn!(?e, "timeline: el audio no se pudo guardar"),
            }
            if !d.texto.is_empty() {
                m.dicho = Some(d.texto);
            }
        }
        if let Err(e) = self.almacen.anadir(&m) {
            tracing::warn!(?e, "timeline: no se pudo apuntar");
            self.decir(textos.t("timeline-no-guardado"));
            return false;
        }
        self.huella = self.almacen.huella();
        self.momentos.push(m);
        self.momentos.sort_by_key(|m| m.cuando);
        self.ir_a_hoy();
        self.abajo = true;
        true
    }

    /// Intro: apunta lo de la caja, o guarda la correccion.
    fn enviar(&mut self, textos: &Catalogo) {
        let texto = self.campo.texto.trim().to_string();
        if let Some(id) = self.corrigiendo.clone() {
            let (titulo, descripcion) = frases::partir(&texto);
            let Some(m) = self.momentos.iter_mut().find(|m| m.id == id) else {
                self.vaciar_caja();
                return;
            };
            if titulo.is_empty() && m.fotos.is_empty() && m.audio.is_none() {
                self.decir(textos.t("timeline-escribe-algo"));
                return;
            }
            if !titulo.is_empty() {
                m.titulo = titulo;
            }
            m.descripcion = descripcion;
            self.guardar_todos(textos);
            self.campo.poner("");
            self.corrigiendo = None;
            self.decir(textos.t("timeline-corregido"));
            return;
        }
        let audio = self.audio_pendiente.take();
        if texto.is_empty() && self.pegadas.is_empty() && audio.is_none() {
            return;
        }
        if self.apuntar(&texto, audio, textos) {
            self.campo.poner("");
        }
    }

    /// Lo que llega del microfono.
    fn dictado_listo(&mut self, d: Dicho, textos: &Catalogo) {
        // Con algo escrito o corrigiendo, lo dicho se suma a la caja y espera
        // a Intro; con la caja vacia, se apunta ya: es lo rapido.
        if !self.campo.texto.trim().is_empty() || self.corrigiendo.is_some() {
            let mut t = d.texto.clone();
            if !self.campo.texto.is_empty() && !self.campo.texto.ends_with([' ', '\n']) {
                t.insert(0, ' ');
            }
            self.campo.cursor = self.campo.texto.len();
            self.campo.escribir(&t);
            if self.corrigiendo.is_none() {
                if let Some(viejo) = self.audio_pendiente.replace(d) {
                    let _ = std::fs::remove_file(viejo.audio);
                }
            } else {
                let _ = std::fs::remove_file(d.audio);
            }
            return;
        }
        let texto = d.texto.clone();
        if self.apuntar(&texto, Some(d), textos) {
            self.decir(textos.t("timeline-apuntado"));
        }
    }
}

fn bucle(
    recursos: &Recursos,
    textos: &Catalogo,
    ubicacion: &Ubicacion,
    idioma: Idioma,
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
    let mut ventana = VentanaOverlay::nueva_normal(marco, &textos.t("timeline-titulo"))
        .context("no se pudo abrir la ventana del timeline")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para el timeline")?;
    ventana.mostrar();
    ventana.enfocar();
    ABIERTA.store(ventana.handle().0 as isize, Ordering::SeqCst);

    let mut e = Estado::nuevo(ubicacion, idioma);
    e.hwnd = Some(ventana.handle());
    tracing::info!(momentos = e.momentos.len(), "ventana del timeline abierta");
    let mut mirado = Instant::now();
    let mut vivo = true;
    let mut pintar = true;
    let mut firma_mirada = Instant::now();
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        // Otra entrada (las de lecciones) pidio una pestana.
        if let Some(p) = Pestana::de_numero(PEDIDA.swap(0, Ordering::SeqCst)) {
            e.detalle = None;
            e.busqueda.poner("");
            e.cambiar_pestana(p);
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
                            e.menu = false;
                            e.moviendo = Some((p, marco));
                            ventana.capturar_raton();
                        }
                        Some(a) => {
                            e.ctrl = pixpin_shell::entrada::modificadores_pulsados().ctrl;
                            vivo = hacer(&mut e, a, textos, ubicacion, idioma);
                        }
                        None => e.menu = false,
                    }
                }
                EventoOverlay::BotonDerechoPulsado(p) => {
                    e.botones.raton = local(p, marco);
                    let r = e.botones.raton;
                    // Clic derecho en una tarjeta: compartirla como imagen.
                    let a = match e.botones.bajo_el_raton() {
                        Some(Accion::AbrirDetalle(i)) => Some(Accion::MenuCompartirMomento(i)),
                        Some(Accion::AbrirLeccion(k)) => Some(Accion::MenuCompartirLeccion(k)),
                        _ => None,
                    };
                    if let Some(a) = a {
                        vivo = hacer(&mut e, a, textos, ubicacion, idioma);
                        if let Some((_, pos)) = e.menu_compartir.as_mut() {
                            *pos = r;
                        }
                    }
                }
                EventoOverlay::BotonSoltado(_) => {
                    if e.moviendo.take().is_some() {
                        ventana.soltar_raton();
                    }
                }
                // En la historia, la rueda pasa de momento, como arriba/abajo.
                EventoOverlay::Rueda(m) if e.detalle.is_some() => {
                    e.mover_detalle(if m > 0 { -1 } else { 1 });
                }
                EventoOverlay::Rueda(m) => {
                    e.scroll -= m as f32 / 120.0 * 60.0 * escala;
                    if m > 0 {
                        e.abajo = false;
                    }
                }
                EventoOverlay::Caracter(c) => letra(&mut e, c),
                EventoOverlay::Tecla {
                    vk, ctrl, shift, ..
                } => vivo = tecla(&mut e, vk, ctrl, shift, textos, ubicacion, idioma),
                _ => {}
            }
        }
        if !vivo {
            break;
        }

        match e.dictado.avanzar(ubicacion, idioma, textos) {
            Salida::Nada => {}
            Salida::Listo(d) => {
                e.dictado_listo(d, textos);
                pintar = true;
            }
            Salida::SinTexto(d) => {
                e.decir(textos.t("timeline-mic-sin-texto"));
                e.dictado_listo(d, textos);
                pintar = true;
            }
            Salida::Fallo(t) => {
                e.decir(t);
                pintar = true;
            }
        }
        if let Some(rx) = &e.pdf {
            if let Ok(r) = rx.try_recv() {
                e.pdf = None;
                match r {
                    Ok(ruta) => exportado(&mut e, ruta, textos),
                    Err(motivo) => {
                        let clave = if motivo == "sin-edge" {
                            "timeline-pdf-sin-edge"
                        } else {
                            "timeline-pdf-fallo"
                        };
                        e.decir(textos.t(clave));
                    }
                }
                pintar = true;
            }
        }

        if let Some((item, destino)) = e.compartir.take() {
            compartir::hacer(&mut e, &item, destino, &motor, &recursos.d3d(), textos);
            pintar = true;
        }
        if mirado.elapsed() >= Duration::from_secs(1) {
            mirado = Instant::now();
            // Las lecciones: al momento si cambiaron en este proceso; las
            // del movil o de otro equipo, mirando su firma cada pocos
            // segundos (leerla cuesta unas fechas, no los ficheros).
            let mirar_firma = firma_mirada.elapsed() >= Duration::from_secs(5);
            if mirar_firma {
                firma_mirada = Instant::now();
            }
            let entradas: Vec<_> = if mirar_firma {
                e.lecciones.iter().map(|t| t.entrada.clone()).collect()
            } else {
                Vec::new()
            };
            if crate::lecciones::almacen::cambios() != e.lecciones_cuenta
                || (mirar_firma
                    && crate::lecciones::almacen::firma(&e.raiz, &entradas) != e.lecciones_firma)
            {
                e.releer_lecciones();
                pintar = true;
            }
            if e.almacen.huella() != e.huella {
                e.releer();
                pintar = true;
            }
            // Las ultimas 24 horas corren solas: lo que cae fuera se va.
            if e.modo() == Modo::Hoy {
                pintar = true;
            }
        }
        if e.aviso
            .as_ref()
            .is_some_and(|(_, t)| t.elapsed() > e.dura_el_aviso())
        {
            e.aviso = None;
            e.deshacer = None;
            e.exportado = None;
            pintar = true;
        }

        let mut faltan_minis = false;
        if pintar {
            let rutas = imagenes_a_cargar(&e, marco.ancho as f32, marco.alto as f32, escala);
            if !rutas.is_empty() {
                faltan_minis = e.minis.asegurar(&rutas, &motor);
            }
            faltan_minis |= cargar_grande(&mut e, marco, &motor);
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
        } else if e.dictado.activo() || e.pdf.is_some() {
            100
        } else if e.aviso.is_some() {
            250
        } else {
            1_000
        };
        pixpin_shell::overlay::esperar_eventos(Some(espera));
    }
    e.soltar_pegadas();
    tracing::info!("ventana del timeline cerrada");
    Ok(())
}

/// Las fotos que hacen falta para el proximo fotograma, en miniatura. En
/// «Momentos» y «Estado» solo las que caen cerca de lo que se ve: un ano de
/// fotos no puede cargarse entero al abrir la pestana.
fn imagenes_a_cargar(e: &Estado, w: f32, h: f32, s: f32) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = e.pegadas.iter().map(|p| p.ruta.clone()).collect();
    // Lo que se ve, con una pantalla de margen arriba y abajo.
    let (desde, hasta) = (e.scroll - h, e.scroll + 2.0 * h);
    match e.modo() {
        Modo::Momentos => {
            for (i, r) in archivo::disposicion(e, w, s).minis.iter() {
                if r.y + r.alto >= desde && r.y <= hasta {
                    if let Some(f) = e.momentos[*i].fotos.first() {
                        v.push(e.almacen.ruta(f));
                    }
                }
            }
        }
        Modo::Lecciones => {
            for (y, ruta) in lecciones::portadas(e, w, s) {
                if y >= desde && y <= hasta {
                    v.push(ruta);
                }
            }
        }
        Modo::Estado => {
            for (y, ruta) in estado_mes::fotos_de_los_circulos(e, w, s) {
                if y >= desde && y <= hasta {
                    v.push(ruta);
                }
            }
        }
        _ => {
            for i in e.visibles() {
                for f in &e.momentos[i].fotos {
                    v.push(e.almacen.ruta(f));
                }
            }
        }
    }
    // La del detalle, en pequeno, mientras llega la grande.
    if let Some(r) = e.foto_del_detalle() {
        v.push(r);
    }
    v
}

/// Carga la foto del detalle a tamano de ventana. Se hace aqui, en el bucle
/// y antes de pintar, nunca dentro del dibujo. Devuelve si aun falta.
fn cargar_grande(e: &mut Estado, marco: Rect, motor: &pixpin_render::MotorRender) -> bool {
    let Some(ruta) = e.foto_del_detalle() else {
        if e.grande_ruta.take().is_some() {
            e.grande = crate::miniaturas::Miniaturas::con_lado(1);
        }
        return false;
    };
    if e.grande_ruta.as_ref() != Some(&ruta) {
        // Otra foto: la anterior se tira entera (ver `Estado::grande`).
        let lado = marco.ancho.max(marco.alto).clamp(256, 3840);
        e.grande = crate::miniaturas::Miniaturas::con_lado(lado);
        e.grande_ruta = Some(ruta.clone());
    }
    e.grande.asegurar(std::slice::from_ref(&ruta), motor)
}

/// Una letra escrita: al buscador si tiene el foco; en «Hoy», a la caja de
/// apuntar; en «Momentos» y «Estado», que no tienen caja, empieza a buscar.
fn letra(e: &mut Estado, c: char) {
    if e.detalle.is_some() {
        return;
    }
    if e.buscando {
        if e.busqueda.letra(c) {
            e.scroll = 0.0;
        }
        return;
    }
    match e.modo() {
        Modo::Hoy => {
            e.campo.letra(c);
        }
        Modo::Momentos | Modo::Estado | Modo::Lecciones
            if !c.is_control() && !c.is_whitespace() =>
        {
            e.buscando = true;
            e.busqueda.letra(c);
            e.scroll = 0.0;
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)] // estado, tecla, modificadores, textos y donde
fn tecla(
    e: &mut Estado,
    vk: u32,
    ctrl: bool,
    shift: bool,
    textos: &Catalogo,
    ubicacion: &Ubicacion,
    idioma: Idioma,
) -> bool {
    // El detalle, encima de todo, se queda las teclas.
    if e.detalle.is_some() {
        let a = match vk {
            VK_ESCAPE if e.menu_compartir.take().is_some() => return true,
            VK_ESCAPE => Accion::CerrarDetalle,
            VK_IZQUIERDA => Accion::FotoAnterior,
            VK_DERECHA => Accion::FotoSiguiente,
            VK_ARRIBA | VK_PAG_ARRIBA => Accion::MomentoAnterior,
            VK_ABAJO | VK_PAG_ABAJO => Accion::MomentoSiguiente,
            _ => return true,
        };
        return hacer(e, a, textos, ubicacion, idioma);
    }
    if ctrl {
        let pestana = match vk {
            VK_1 => Some(Pestana::Hoy),
            VK_2 => Some(Pestana::Momentos),
            VK_3 => Some(Pestana::Estado),
            VK_4 => Some(Pestana::Lecciones),
            _ => None,
        };
        if let Some(p) = pestana {
            return hacer(e, Accion::Pestana(p), textos, ubicacion, idioma);
        }
        if vk == VK_F {
            return hacer(e, Accion::EnfocarBusqueda, textos, ubicacion, idioma);
        }
    }
    if e.buscando {
        match vk {
            VK_ESCAPE => e.vaciar_busqueda(),
            // Intro deja el foco: los resultados ya estan a la vista.
            VK_ENTRAR => e.buscando = false,
            _ => {
                let antes = e.busqueda.texto.clone();
                e.busqueda.tecla(vk, ctrl, shift, false);
                if e.busqueda.texto != antes {
                    e.scroll = 0.0;
                }
            }
        }
        return true;
    }
    match vk {
        VK_ESCAPE => {
            if std::mem::take(&mut e.menu) || e.menu_compartir.take().is_some() {
            } else if !e.busqueda.texto.is_empty() {
                e.vaciar_busqueda();
            } else if e.corrigiendo.is_some() {
                e.vaciar_caja();
            } else if !e.campo.texto.is_empty() || !e.pegadas.is_empty() {
                e.vaciar_caja();
            } else if e.dia.is_some() {
                return hacer(e, Accion::VolverDelDia, textos, ubicacion, idioma);
            } else if e.pestana != Pestana::Hoy {
                return hacer(e, Accion::Pestana(Pestana::Hoy), textos, ubicacion, idioma);
            } else {
                return false;
            }
        }
        VK_Z if ctrl && e.deshacer.is_some() => {
            return hacer(e, Accion::Deshacer, textos, ubicacion, idioma);
        }
        VK_M if ctrl => return hacer(e, Accion::Microfono, textos, ubicacion, idioma),
        VK_PAG_ARRIBA => {
            e.scroll -= 400.0;
            e.abajo = false;
        }
        VK_PAG_ABAJO => e.scroll += 400.0,
        _ if e.modo() != Modo::Hoy => {}
        VK_ENTRAR if !shift => return hacer(e, Accion::Enviar, textos, ubicacion, idioma),
        VK_V if ctrl => pegar(e, textos),
        // Como en Telegram: flecha arriba con la caja vacia corrige el ultimo.
        VK_ARRIBA if e.campo.texto.is_empty() && e.corrigiendo.is_none() => {
            if let Some(&i) = e.visibles().last() {
                let c = clave(&e.momentos[i].id);
                return hacer(e, Accion::Corregir(c), textos, ubicacion, idioma);
            }
        }
        _ => {
            e.campo.tecla(vk, ctrl, shift, true);
        }
    }
    true
}

/// Ctrl+V: una imagen va a la caja como foto; el texto, a lo escrito.
fn pegar(e: &mut Estado, textos: &Catalogo) {
    use pixpin_codec::ContenidoPortapapeles as C;
    match pixpin_codec::portapapeles::leer() {
        Some(C::Texto(t)) => e.campo.escribir(&t.replace("\r\n", "\n")),
        Some(C::Imagen(img)) if img.ancho > 0 && img.alto > 0 => match png_temporal(&img) {
            Ok(ruta) => e.pegadas.push(Pegada {
                ruta,
                temporal: true,
            }),
            Err(err) => {
                tracing::warn!(?err, "timeline: la imagen pegada no se pudo guardar");
                e.decir(textos.t("timeline-pegar-no-imagen"));
            }
        },
        Some(C::Rutas(rutas)) => {
            let antes = e.pegadas.len();
            for r in rutas.into_iter().filter(|r| crate::tareas::es_imagen(r)) {
                e.pegadas.push(Pegada {
                    ruta: r,
                    temporal: false,
                });
            }
            if e.pegadas.len() == antes {
                e.decir(textos.t("timeline-pegar-no-imagen"));
            }
        }
        _ => {}
    }
}

fn png_temporal(img: &pixpin_codec::ImagenRgba) -> Result<PathBuf> {
    static CUENTA: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let carpeta = std::env::temp_dir().join("pixpin-timeline");
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

/// Lo que hace un clic (o su tecla). Devuelve si la ventana sigue.
fn hacer(
    e: &mut Estado,
    a: Accion,
    textos: &Catalogo,
    ubicacion: &Ubicacion,
    idioma: Idioma,
) -> bool {
    if !matches!(a, Accion::Formato(_) | Accion::Exportar) {
        e.menu = false;
    }
    if !matches!(a, Accion::Compartir(_) | Accion::Fondo) {
        e.menu_compartir = None;
    }
    // Pulsar cualquier otra cosa le quita el foco al buscador (lo escrito
    // se queda: los resultados siguen a la vista).
    if !matches!(
        a,
        Accion::EnfocarBusqueda | Accion::VaciarBusqueda | Accion::Fondo
    ) {
        e.buscando = false;
    }
    match a {
        Accion::Mover | Accion::Fondo | Accion::SoltarMenu => {}
        Accion::Cerrar => return false,
        Accion::Enviar => e.enviar(textos),
        Accion::Microfono => {
            e.ir_a_hoy();
            if let Some(t) = e.dictado.pulsar(ubicacion, idioma, textos) {
                e.decir(t);
            }
        }
        Accion::Imagen => {
            if let Some(h) = e.hwnd {
                for r in pixpin_shell::elegir::pedir_imagenes(h)
                    .into_iter()
                    .filter(|r| crate::tareas::es_imagen(r))
                {
                    e.pegadas.push(Pegada {
                        ruta: r,
                        temporal: false,
                    });
                }
            }
        }
        Accion::QuitarPegada(k) => {
            if k < e.pegadas.len() {
                let p = e.pegadas.remove(k);
                if p.temporal {
                    let _ = std::fs::remove_file(p.ruta);
                }
            }
        }
        Accion::Pestana(p) => {
            // Pulsar una pestana es ir a ella tal cual: sin la busqueda ni
            // el dia que tapaban lo de debajo.
            e.busqueda.poner("");
            e.detalle = None;
            e.cambiar_pestana(p);
        }
        Accion::VolverDelDia => {
            e.dia = None;
            e.scroll = 0.0;
            e.abajo = e.modo() == Modo::Hoy;
        }
        Accion::Exportar => e.menu = !e.menu,
        Accion::Formato(f) => {
            e.menu = false;
            exportar_lo_que_se_ve(e, f, textos);
        }
        Accion::EnfocarBusqueda => e.buscando = true,
        Accion::VaciarBusqueda => e.vaciar_busqueda(),
        Accion::AbrirDia(n) => {
            e.busqueda.poner("");
            e.dia = Some(Dia::de_numero(n));
            e.scroll = 0.0;
            e.abajo = false;
        }
        Accion::DiaDelEstado(n) => {
            if e.ctrl {
                return hacer(e, Accion::ElegirDia(n), textos, ubicacion, idioma);
            }
            return hacer(e, Accion::AbrirDia(n), textos, ubicacion, idioma);
        }
        Accion::ElegirDia(n) => {
            let d = Dia::de_numero(n);
            if !e.elegidos.remove(&d) {
                e.elegidos.insert(d);
            }
        }
        Accion::QuitarEleccion => e.elegidos.clear(),
        Accion::Borrar(c) => {
            if let Some(i) = e.momento_de(c) {
                let m = e.momentos.remove(i);
                if e.corrigiendo.as_deref() == Some(m.id.as_str()) {
                    e.vaciar_caja();
                }
                e.guardar_todos(textos);
                e.decir(textos.t("timeline-borrado"));
                e.deshacer = Some(m);
            }
        }
        Accion::Deshacer => {
            if let Some(m) = e.deshacer.take() {
                if !e.momentos.iter().any(|x| x.id == m.id) {
                    e.momentos.push(m);
                    e.guardar_todos(textos);
                }
                e.aviso = None;
            }
        }
        Accion::Corregir(c) => {
            if let Some(m) = e.momento_de(c).map(|i| &e.momentos[i]) {
                let texto = if m.descripcion.is_empty() {
                    m.titulo.clone()
                } else {
                    format!("{}\n{}", m.titulo, m.descripcion)
                };
                let id = m.id.clone();
                e.soltar_pegadas();
                e.campo.poner(&texto);
                e.corrigiendo = Some(id);
                // Se corrige en la caja de «Hoy», que es la unica que hay.
                e.ir_a_hoy();
            }
        }
        Accion::DejarDeCorregir => e.vaciar_caja(),
        Accion::Oir(c) => {
            if let Some(m) = e.momento_de(c).map(|i| &e.momentos[i])
                && let Some(a) = &m.audio
            {
                crate::ventana_chat::reproducir_flotante(
                    textos,
                    e.almacen.ruta(a),
                    m.titulo.clone(),
                );
            }
        }
        // Clic en una foto de una tarjeta: la historia, en esa foto (pinearla
        // se hace desde ahi).
        Accion::AbrirFoto(c, k) => {
            if let Some(i) = e.momento_de(c) {
                e.abrir_detalle(i);
                if let Some(d) = e.detalle.as_mut() {
                    d.foto = k;
                }
            }
        }
        Accion::AbrirDetalle(c) => {
            if let Some(i) = e.momento_de(c) {
                e.abrir_detalle(i);
            }
        }
        Accion::CerrarDetalle => e.detalle = None,
        Accion::FotoAnterior => e.avanzar(-1),
        Accion::FotoSiguiente => e.avanzar(1),
        Accion::IrAFoto(k) => {
            if let Some(d) = e.detalle.as_mut() {
                d.foto = k;
            }
        }
        Accion::MomentoAnterior => e.mover_detalle(-1),
        Accion::MomentoSiguiente => e.mover_detalle(1),
        Accion::Leccion(c) => {
            if let Some(i) = e.momento_de(c) {
                lecciones::alternar(e, i, textos);
            }
        }
        Accion::AbrirLeccion(c) => {
            if let Some(k) = e.leccion_de(c) {
                e.abrir_leccion(k);
            }
        }
        Accion::OtraVez(c) => {
            if let Some(k) = e.leccion_de(c) {
                lecciones::otra_vez(e, k, textos);
            }
        }
        Accion::MenuCompartirMomento(c) => {
            if let Some(m) = e.momento_de(c).map(|i| &e.momentos[i]) {
                e.menu_compartir = Some((Item::Momento(m.id.clone()), e.botones.raton));
            }
        }
        Accion::MenuCompartirLeccion(c) => {
            if let Some(t) = e.leccion_de(c).map(|k| &e.lecciones[k]) {
                let id = t.entrada.leccion.id.clone();
                e.menu_compartir = Some((Item::Leccion(id), e.botones.raton));
            }
        }
        Accion::Compartir(d) => {
            if let Some((item, _)) = e.menu_compartir.take() {
                e.compartir = Some((item, d));
            }
        }
        Accion::OirDetalle => {
            if let Some(c) = compartir::contenido_del_detalle(e, textos)
                && let Some((ruta, _)) = c.audio
            {
                crate::ventana_chat::reproducir_flotante(textos, ruta, c.titulo);
            }
        }
        Accion::PinearDetalle => {
            if let Some(r) = e.foto_del_detalle()
                && r.is_file()
                && pixpin_shell::mensajero::enviar_ficheros(std::slice::from_ref(&r))
            {
                e.decir(textos.t("chat-pineado"));
            }
        }
        Accion::LeccionDetalle => {
            if let Some(i) = e.en_detalle() {
                lecciones::alternar(e, i, textos);
            }
        }
        Accion::OtraVezDetalle => {
            if let Some(k) = e.leccion_en_detalle() {
                lecciones::otra_vez(e, k, textos);
            }
        }
        Accion::GravedadDetalle => {
            if let Some(k) = e.leccion_en_detalle() {
                lecciones::cambiar_gravedad(e, k, textos);
            }
        }
        Accion::CompartirDetalle => {
            if let Some(item) = e.item_en_detalle().cloned() {
                e.menu_compartir = Some((item, e.botones.raton));
            }
        }
        Accion::FiltroGravedad(g) => {
            e.filtro_gravedad = g;
            e.scroll = 0.0;
        }
        Accion::OrdenLecciones(repetidas) => {
            e.orden_lecciones = if repetidas {
                super::tarjetas::Orden::MasRepetidas
            } else {
                super::tarjetas::Orden::Recientes
            };
            e.scroll = 0.0;
        }
        Accion::VerLeccionesBuscadas => {
            let q = e.busqueda.texto.clone();
            e.cambiar_pestana(Pestana::Lecciones);
            e.busqueda.poner(&q);
        }
        Accion::AbrirExportado => {
            if let Some(r) = e.exportado.take() {
                let _ = pixpin_shell::abrir::abrir(&r);
            }
            e.aviso = None;
        }
    }
    true
}

/// Los dias que exporta «Momentos» o «Estado»: los elegidos o, sin
/// eleccion, los del mes de hoy que tienen algo.
fn dias_a_exportar(e: &Estado) -> Vec<Dia> {
    if e.pestana == Pestana::Estado && !e.elegidos.is_empty() {
        return e.elegidos.iter().copied().collect();
    }
    let con = dias::dias_con(&e.momentos, e.desfase);
    let mes = Mes::de(e.hoy());
    con.keys().copied().filter(|d| Mes::de(*d) == mes).collect()
}

/// Lo que sale al exportar segun lo que se ve: las ultimas 24 horas, el dia
/// abierto, lo encontrado o, en «Momentos» y «Estado», los dias elegidos (o
/// sin eleccion, los del mes de hoy).
fn exportar_lo_que_se_ve(e: &mut Estado, f: Formato, textos: &Catalogo) {
    let hoy = e.hoy();
    let (nombre, subtitulo, dias_sueltos): (String, String, Vec<Dia>) = match e.modo() {
        Modo::Lecciones => {
            exportar_lecciones(e, f, textos);
            return;
        }
        Modo::Hoy => (
            format!("timeline-24h-{}", hoy.iso()),
            textos.t("timeline-ultimas-24h"),
            Vec::new(),
        ),
        Modo::Buscar => {
            let mut a = fluent_bundle::FluentArgs::new();
            a.set("que", e.busqueda.texto.trim().to_string());
            (
                format!("timeline-busqueda-{}", hoy.iso()),
                textos.t_args("timeline-exportar-busqueda", &a),
                Vec::new(),
            )
        }
        Modo::Dia(d) => (
            format!("timeline-{}", d.iso()),
            dias::nombre_del_dia(d, e.ingles),
            vec![d],
        ),
        Modo::Momentos | Modo::Estado => {
            let v = dias_a_exportar(e);
            let (Some(a), Some(b)) = (v.first().copied(), v.last().copied()) else {
                e.decir(textos.t("timeline-nada-que-exportar"));
                return;
            };
            let sub = if a == b {
                dias::nombre_del_dia(a, e.ingles)
            } else {
                format!(
                    "{} – {}",
                    dias::nombre_del_dia(a, e.ingles),
                    dias::nombre_del_dia(b, e.ingles)
                )
            };
            let nombre = if a == b {
                format!("timeline-{}", a.iso())
            } else {
                format!("timeline-{}_{}", a.iso(), b.iso())
            };
            (nombre, sub, v)
        }
    };
    let secciones: Vec<Seccion<'_>> = if dias_sueltos.is_empty() {
        let mut v = if e.modo() == Modo::Buscar {
            pixpin_timeline::buscar::buscar(&e.momentos, &e.busqueda.texto)
        } else {
            dias::ultimas_24h(&e.momentos, Estado::ahora())
        };
        // En la pagina, siempre de lo viejo a lo nuevo, como una linea de
        // tiempo (lo encontrado llega al reves).
        v.sort_by_key(|m| (m.cuando, m.id.clone()));
        partir_por_dias(e, v)
    } else {
        dias_sueltos
            .iter()
            .map(|d| Seccion {
                rotulo: dias::nombre_del_dia(*d, e.ingles),
                momentos: dias::del_dia(&e.momentos, *d, e.desfase),
            })
            .filter(|s| !s.momentos.is_empty())
            .collect()
    };
    if secciones.is_empty() {
        e.decir(textos.t("timeline-nada-que-exportar"));
        return;
    }
    let op = opciones(e, textos, textos.t("timeline-titulo"), subtitulo);
    let pagina = exportar::pagina(&e.almacen, &op, &secciones);
    guardar_pagina(e, f, &nombre, pagina, textos);
}

/// Las opciones de la pagina exportada.
fn opciones(e: &Estado, textos: &Catalogo, titulo: String, subtitulo: String) -> Opciones {
    let mut pie = fluent_bundle::FluentArgs::new();
    pie.set("fecha", dias::nombre_del_dia(e.hoy(), e.ingles));
    Opciones {
        titulo,
        subtitulo,
        pie: textos.t_args("timeline-exportado-pie", &pie),
        nota_de_voz: textos.t("timeline-nota-de-voz"),
        desfase: e.desfase,
        ingles: e.ingles,
        pestanas: [
            textos.t("timeline-pagina-linea"),
            textos.t("timeline-pestana-momentos"),
            textos.t("timeline-pestana-estado"),
        ],
        sobre_todo: textos.t("timeline-estado-sobre-todo"),
    }
}

/// **Exportar las lecciones** que se ven, con la misma pagina: cada una
/// como un momento (su gravedad delante del titulo, los tres campos en el
/// texto) con sus fotos y su nota de voz dentro.
fn exportar_lecciones(e: &mut Estado, f: Formato, textos: &Catalogo) {
    let visibles = lecciones::visibles(e);
    if visibles.is_empty() {
        e.decir(textos.t("timeline-nada-que-exportar"));
        return;
    }
    let como_momentos: Vec<Momento> = visibles
        .iter()
        .map(|&k| {
            let t = &e.lecciones[k];
            let l = &t.entrada.leccion;
            let gr = super::tarjetas::gravedad(l.gravedad);
            let mut texto = l.que_paso.trim().to_string();
            for (clave, campo) in [("timeline-leccion-por-que", &l.por_que), ("timeline-leccion-proxima", &l.proxima)] {
                if !campo.trim().is_empty() {
                    if !texto.is_empty() {
                        texto.push_str("

");
                    }
                    texto.push_str(&format!("{}: {}", textos.t(clave), campo.trim()));
                }
            }
            let mut m = Momento::nuevo(l.id.clone(), l.creada, format!("{} {}", gr.emoticono, l.titulo), texto);
            m.fotos = t.fotos.iter().map(|p| p.to_string_lossy().into_owned()).collect();
            if let Some((a, d)) = &t.voz {
                m.audio = Some(a.to_string_lossy().into_owned());
                m.duracion_ms = *d;
            }
            m
        })
        .collect();
    let mut refs: Vec<&Momento> = como_momentos.iter().collect();
    refs.sort_by_key(|m| m.cuando);
    let secciones = partir_por_dias(e, refs);
    let op = opciones(
        e,
        textos,
        textos.t("timeline-pestana-lecciones"),
        con_n(textos, "timeline-sub-lecciones", visibles.len()),
    );
    // Sus ficheros van por su ruta entera (no estan en el almacen del
    // timeline, sino en el chat de cada leccion).
    let dentro = |ruta: &str| {
        let bytes = std::fs::read(ruta).ok()?;
        Some(pixpin_timeline::html::data_uri(ruta, &bytes))
    };
    let pagina = pixpin_timeline::html::pagina(&op, &secciones, &dentro, &dentro);
    let nombre = format!("lecciones-{}", e.hoy().iso());
    guardar_pagina(e, f, &nombre, pagina, textos);
}

/// Pide donde y guarda la pagina en HTML, o la imprime a PDF en un hilo.
fn guardar_pagina(e: &mut Estado, f: Formato, nombre: &str, pagina: String, textos: &Catalogo) {
    let pesa = pagina.len();
    let Some(h) = e.hwnd else {
        return;
    };
    let tipo = match f {
        Formato::Html => textos.t("timeline-tipo-html"),
        Formato::Pdf => textos.t("timeline-tipo-pdf"),
    };
    let sugerido = format!("{nombre}.{}", f.extension());
    let Some(destino) = pixpin_shell::guardar::pedir_ruta_para(h, &sugerido, &tipo, f.extension())
    else {
        return;
    };
    match f {
        Formato::Html => match std::fs::write(&destino, pagina) {
            Ok(()) => {
                exportado(e, destino, textos);
                // Fotos y audios van dentro: si pesa mucho, que se sepa
                // antes de intentar mandarlo.
                if pesa > exportar::PESA_MUCHO
                    && let Some((t, _)) = e.aviso.as_mut()
                {
                    let mut a = fluent_bundle::FluentArgs::new();
                    a.set("mb", (pesa / (1024 * 1024)) as i64);
                    t.push_str(" · ");
                    t.push_str(&textos.t_args("timeline-exportado-pesa", &a));
                }
            }
            Err(err) => {
                tracing::warn!(?err, "timeline: no se pudo exportar");
                e.decir(textos.t("timeline-no-exportado"));
            }
        },
        Formato::Pdf => {
            let (tx, rx) = std::sync::mpsc::channel();
            let lanzado = std::thread::Builder::new()
                .name("timeline-pdf".into())
                .spawn(move || {
                    let r = exportar::a_pdf(&pagina, &destino).map(|()| destino);
                    if let Err(m) = &r {
                        tracing::warn!(motivo = %m, "timeline: el PDF no salio");
                    }
                    let _ = tx.send(r);
                });
            if lanzado.is_ok() {
                e.pdf = Some(rx);
                e.decir(textos.t("timeline-pdf-haciendo"));
            }
        }
    }
}

/// Lo ya exportado: el aviso con «Abrir».
fn exportado(e: &mut Estado, ruta: PathBuf, textos: &Catalogo) {
    let mut a = fluent_bundle::FluentArgs::new();
    a.set(
        "nombre",
        ruta.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    );
    e.decir(textos.t_args("timeline-exportado", &a));
    e.exportado = Some(ruta);
}

/// Las ultimas 24 horas cortadas por dias, cada uno con su rotulo.
fn partir_por_dias<'a>(e: &Estado, v: Vec<&'a Momento>) -> Vec<Seccion<'a>> {
    let mut out: Vec<Seccion<'a>> = Vec::new();
    let mut actual: Option<Dia> = None;
    for m in v {
        let d = Dia::de_instante(m.cuando, e.desfase);
        if actual != Some(d) {
            actual = Some(d);
            out.push(Seccion {
                rotulo: dias::nombre_del_dia(d, e.ingles),
                momentos: Vec::new(),
            });
        }
        if let Some(s) = out.last_mut() {
            s.momentos.push(m);
        }
    }
    out
}

// ------------------------------------------------------------- el pintado

fn blanco(a: f32) -> Color {
    Color { a, ..Color::BLANCO }
}

fn encoger(r: RectF, m: f32) -> RectF {
    RectF {
        x: r.x + m,
        y: r.y + m,
        ancho: (r.ancho - 2.0 * m).max(0.0),
        alto: (r.alto - 2.0 * m).max(0.0),
    }
}

/// Un boton redondo de icono. `encendido` lo pinta con ese fondo.
#[allow(clippy::too_many_arguments)] // pintor, botones, caja, que, icono, color, fondo y escala
fn boton_icono(
    p: &Pintor,
    botones: &mut Botones<Accion>,
    caja: RectF,
    que: Accion,
    icono: &pixpin_render::icono::Icono,
    tinta: Color,
    fondo: Option<Color>,
    s: f32,
) {
    let encima = dentro(caja, botones.raton);
    match (fondo, encima) {
        (Some(c), true) => p.rellenar_redondeado(caja, caja.alto / 2.0, ui::aclarar(c, 0.08)),
        (Some(c), false) => p.rellenar_redondeado(caja, caja.alto / 2.0, c),
        (None, true) => p.rellenar_redondeado(caja, caja.alto / 2.0, blanco(0.09)),
        (None, false) => {}
    }
    p.icono(icono, encoger(caja, 10.0 * s), tinta);
    botones.zona(caja, que);
}

/// Lo que mide cada trozo de una tarjeta.
struct Medida {
    titulo: f32,
    desc: f32,
    fotos_por_fila: usize,
    filas_de_fotos: usize,
    alto: f32,
}

/// El titulo de una tarjeta en renglones, con los emoticonos en color (la
/// negrita los pintaba en monocromo): partido igual al medir y al pintar.
fn renglones_del_titulo(p: &Pintor, t: &str, ancho: f32, s: f32) -> Vec<String> {
    dis::renglones(t, ancho, &|x| historia::medir_trozos(p, x, 15.5 * s, true))
}

fn alto_del_titulo(p: &Pintor, t: &str, ancho: f32, s: f32) -> f32 {
    renglones_del_titulo(p, t, ancho, s).len().max(1) as f32 * 15.5 * s * 1.33
}

fn titulo_en_color(p: &Pintor, t: &str, x: f32, y: f32, ancho: f32, s: f32) {
    for (k, l) in renglones_del_titulo(p, t, ancho, s).iter().enumerate() {
        historia::pintar_trozos(p, l, x, y + k as f32 * 15.5 * s * 1.33, 15.5 * s, true, TEXTO_V, false);
    }
}

fn medir(p: &Pintor, m: &Momento, ancho: f32, s: f32) -> Medida {
    let dentro_w = (ancho - 2.0 * PAD * s).max(20.0);
    let titulo = alto_del_titulo(p, &m.titulo, dentro_w, s);
    let desc = if m.descripcion.trim().is_empty() {
        0.0
    } else {
        p.medir_texto_ajustado(&m.descripcion, 14.0 * s, dentro_w).1
    };
    let fotos_por_fila = (((dentro_w + 8.0 * s) / ((FOTO_W + 8.0) * s)).floor() as usize).max(1);
    let filas_de_fotos = m.fotos.len().div_ceil(fotos_por_fila);
    let mut alto = PAD * s + titulo;
    if desc > 0.0 {
        alto += 4.0 * s + desc;
    }
    if filas_de_fotos > 0 {
        alto += 10.0 * s + filas_de_fotos as f32 * (FOTO_H + 8.0) * s - 8.0 * s;
    }
    if m.audio.is_some() {
        alto += 10.0 * s + CHAPA_VOZ * s;
    }
    alto += PAD * s;
    Medida {
        titulo,
        desc,
        fotos_por_fila,
        filas_de_fotos,
        alto,
    }
}

fn con_n(textos: &Catalogo, clave: &str, n: usize) -> String {
    let mut a = fluent_bundle::FluentArgs::new();
    a.set("n", n as i64);
    textos.t_args(clave, &a)
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
    let arriba = crate::cabecera::ALTO * s;
    let modo = e.modo();

    // Abajo: la caja de apuntar (solo en «Hoy»).
    let alto_caja = if modo == Modo::Hoy {
        alto_de_la_caja(e, p, w, s)
    } else {
        0.0
    };
    let area = RectF {
        x: 0.0,
        y: arriba,
        ancho: w,
        alto: (h - arriba - alto_caja).max(0.0),
    };

    match modo {
        Modo::Momentos => archivo::pintar(e, p, area, s, textos),
        Modo::Estado => estado_mes::pintar(e, p, area, s, textos),
        Modo::Dia(d) => {
            let barra = BARRA_DIA * s;
            let debajo = RectF {
                y: area.y + barra,
                alto: (area.alto - barra).max(0.0),
                ..area
            };
            pintar_linea(e, p, debajo, s, textos);
            barra_del_dia(
                e,
                p,
                RectF {
                    alto: barra,
                    ..area
                },
                d,
                s,
                textos,
            );
        }
        Modo::Lecciones => lecciones::pintar(e, p, area, s, textos),
        Modo::Buscar => {
            // Si lo buscado tambien esta en lecciones, una franja lo dice.
            let n = lecciones::cuantas_buscadas(e);
            if n > 0 {
                let franja = 52.0 * s;
                let debajo = RectF {
                    y: area.y + franja,
                    alto: (area.alto - franja).max(0.0),
                    ..area
                };
                pintar_linea(e, p, debajo, s, textos);
                lecciones::franja_buscadas(
                    e,
                    p,
                    RectF {
                        alto: franja,
                        ..area
                    },
                    n,
                    s,
                    textos,
                );
            } else {
                pintar_linea(e, p, area, s, textos);
            }
        }
        Modo::Hoy => pintar_linea(e, p, area, s, textos),
    }

    // La barra comun, encima de lo que se desplazo bajo ella.
    let subtitulo = match modo {
        Modo::Hoy => con_n(textos, "timeline-sub-ultimas", e.visibles().len()),
        Modo::Dia(d) => {
            let mut a = fluent_bundle::FluentArgs::new();
            a.set("dia", dias::nombre_del_dia(d, e.ingles));
            a.set("n", e.visibles().len() as i64);
            textos.t_args("timeline-sub-dia", &a)
        }
        Modo::Buscar => con_n(textos, "timeline-sub-buscar", e.visibles().len()),
        Modo::Momentos => con_n(textos, "timeline-sub-momentos", e.momentos.len()),
        Modo::Estado => con_n(textos, "timeline-sub-estado-dias", dias::dias_con(&e.momentos, e.desfase).len()),
        Modo::Lecciones => con_n(
            textos,
            "timeline-sub-lecciones",
            lecciones::visibles(e).len(),
        ),
    };
    let titulo = textos.t("timeline-titulo");
    let pista = textos.t("timeline-buscar-pista");
    let rotulos = [
        (Pestana::Hoy, textos.t("timeline-pestana-hoy")),
        (Pestana::Momentos, textos.t("timeline-pestana-momentos")),
        (Pestana::Estado, textos.t("timeline-pestana-estado")),
        (Pestana::Lecciones, textos.t("timeline-pestana-lecciones")),
    ];
    // Las pestanas, como control segmentado. Buscando en todo el timeline
    // no hay ninguna activa: lo que se ve no es de ninguna.
    let segmentos: Vec<crate::cabecera::Segmento<'_, Accion>> = rotulos
        .iter()
        .map(|(pe, r)| crate::cabecera::Segmento {
            rotulo: r.as_str(),
            activo: e.pestana == *pe && modo != Modo::Buscar,
            accion: Accion::Pestana(*pe),
        })
        .collect();
    // «Exportar», aparte y solo de icono (descargar, que no es compartir):
    // `dis::caja_exportar` cuenta con que es el ultimo para colgar su menu.
    let botones = vec![crate::cabecera::Boton {
        icono: Some(&mi::FILE_DOWNLOAD),
        rotulo: "",
        chapa: None,
        activo: e.menu,
        accion: Accion::Exportar,
    }];
    crate::cabecera::pintar_con_segmentos(
        p,
        &mut e.botones,
        w,
        s,
        &crate::cabecera::Cabecera {
            titulo: &titulo,
            subtitulo: &subtitulo,
            buscador: Some(crate::cabecera::Buscador {
                texto: &e.busqueda.texto,
                pista: &pista,
                foco: e.buscando,
                cursor: Some(e.busqueda.cursor),
                enfocar: Accion::EnfocarBusqueda,
                vaciar: Accion::VaciarBusqueda,
            }),
            botones,
            mover: Accion::Mover,
            cerrar: Accion::Cerrar,
        },
        &segmentos,
    );

    if modo == Modo::Hoy {
        pintar_caja(
            e,
            p,
            RectF {
                x: 0.0,
                y: h - alto_caja,
                ancho: w,
                alto: alto_caja,
            },
            s,
            textos,
        );
    }

    if e.menu {
        pintar_menu(e, p, dis::caja_exportar(w, s), w, h, s, textos);
    }
    if e.detalle.is_some() {
        historia::pintar(e, p, w, h, s, textos);
    }
    if e.menu_compartir.is_some() {
        compartir::pintar_menu(e, p, w, h, s, textos);
    }
    pintar_aviso(e, p, w, h - alto_caja, s, textos);
}

/// La barra encima de un dia abierto: volver (Esc) y que dia es.
fn barra_del_dia(e: &mut Estado, p: &Pintor, zona: RectF, d: Dia, s: f32, textos: &Catalogo) {
    p.rellenar(zona, FONDO_V);
    p.linea(
        (0.0, zona.y + zona.alto),
        (zona.ancho, zona.y + zona.alto),
        1.0 * s,
        blanco(0.06),
    );
    let m = MARGEN * s;
    let rot = textos.t("timeline-volver");
    let bw = ui::ancho_de_boton(p, true, &rot, Some("Esc"), s);
    let b = RectF {
        x: m,
        y: zona.y + (zona.alto - BOTON * s) / 2.0,
        ancho: bw,
        alto: BOTON * s,
    };
    ui::boton_v2(
        p,
        &mut e.botones,
        b,
        Accion::VolverDelDia,
        Some(&mi::ARROW_BACK),
        &rot,
        Some("Esc"),
        Some(hex(0x2C2C2E)),
        TEXTO_V,
        s,
    );
    let nombre = dias::nombre_del_dia(d, e.ingles);
    let x = b.x + b.ancho + 16.0 * s;
    let (_, nh) = ui::medir_negrita(p, &nombre, 16.0 * s, zona.ancho - x - m);
    ui::negrita(
        p,
        &nombre,
        x,
        zona.y + (zona.alto - nh) / 2.0,
        16.0 * s,
        zona.ancho - x - m,
        TEXTO_V,
    );
}

/// La linea de tiempo de las ultimas 24 horas o de un dia.
fn pintar_linea(e: &mut Estado, p: &Pintor, area: RectF, s: f32, textos: &Catalogo) {
    let m = MARGEN * s;
    let riel = m + HORA * s + AL_RIEL * s;
    let x_tarjeta = riel + AL_RIEL * s;
    let ancho_tarjeta = (area.ancho - x_tarjeta - m).max(80.0 * s);
    let visibles = e.visibles();
    let modo = e.modo();
    let con_rotulos = matches!(modo, Modo::Hoy | Modo::Buscar);

    // Las piezas: en las ultimas 24 horas y en lo buscado, un rotulo cada
    // vez que cambia el dia.
    let mut piezas: Vec<(Pieza, f32, f32)> = Vec::new();
    let mut y = 16.0 * s;
    let mut dia_actual: Option<Dia> = None;
    let mut medidas = Vec::with_capacity(visibles.len());
    for &i in &visibles {
        let mo = &e.momentos[i];
        if con_rotulos {
            let d = Dia::de_instante(mo.cuando, e.desfase);
            if dia_actual != Some(d) {
                dia_actual = Some(d);
                piezas.push((Pieza::Separador(d), y, SEPARADOR * s));
                y += SEPARADOR * s;
            }
        }
        let md = medir(p, mo, ancho_tarjeta, s);
        piezas.push((Pieza::Momento(i), y, md.alto));
        y += md.alto + ENTRE * s;
        medidas.push((i, md));
    }
    e.alto_contenido = y + 8.0 * s;
    let tope = (e.alto_contenido - area.alto).max(0.0);
    // Solo «Hoy» se pega al final (lo nuevo, junto a la caja); un dia o lo
    // buscado se leen de arriba abajo.
    if modo == Modo::Hoy && e.abajo {
        e.scroll = tope;
    }
    e.scroll = e.scroll.clamp(0.0, tope);
    if modo == Modo::Hoy && e.scroll >= tope - 1.0 {
        e.abajo = true;
    }

    if visibles.is_empty() {
        let clave = match modo {
            Modo::Dia(_) => "timeline-vacio-dia",
            Modo::Buscar => "timeline-sin-resultados",
            _ => "timeline-vacio",
        };
        let t = textos.t(clave);
        let ancho = (area.ancho - 2.0 * m * 3.0).max(100.0);
        let (tw, th) = p.medir_texto_ajustado(&t, 15.0 * s, ancho);
        p.icono(
            &mi::TIMELINE,
            RectF {
                x: area.x + (area.ancho - 48.0 * s) / 2.0,
                y: area.y + area.alto / 2.0 - th - 64.0 * s,
                ancho: 48.0 * s,
                alto: 48.0 * s,
            },
            blanco(0.25),
        );
        p.texto_ajustado(
            &t,
            area.x + (area.ancho - tw) / 2.0,
            area.y + area.alto / 2.0 - th,
            15.0 * s,
            tw + 2.0,
            GRIS,
        );
        return;
    }

    let raton = e.botones.raton;
    let y0 = area.y - e.scroll;
    let hoy = e.hoy();
    let ahora = Estado::ahora();
    let mut zonas: Vec<(RectF, Accion)> = Vec::new();
    p.con_recorte(area, |p| {
        // La linea, de punta a punta de lo que hay.
        if let (Some(a), Some(b)) = (piezas.first(), piezas.last()) {
            p.linea(
                (riel, (y0 + a.1 + 8.0 * s).max(area.y)),
                (riel, (y0 + b.1 + 22.0 * s).min(area.y + area.alto)),
                2.0 * s,
                LINEA,
            );
        }
        for (pieza, py, alto) in &piezas {
            let y = y0 + py;
            if y + alto < area.y || y > area.y + area.alto {
                continue;
            }
            match *pieza {
                Pieza::Separador(d) => {
                    let rot = if d == hoy {
                        textos.t("timeline-hoy")
                    } else if d.numero() == hoy.numero() - 1 {
                        textos.t("timeline-ayer")
                    } else {
                        dias::nombre_del_dia(d, e.ingles)
                    };
                    let (tw, th) = p.medir_texto(&rot, 12.5 * s);
                    let pill = RectF {
                        x: (riel - (tw + 24.0 * s) / 2.0).max(m),
                        y: y + 6.0 * s,
                        ancho: tw + 24.0 * s,
                        alto: th + 10.0 * s,
                    };
                    p.rellenar_redondeado(pill, pill.alto / 2.0, hex(0x2C2C2E));
                    p.texto(&rot, pill.x + 12.0 * s, pill.y + 5.0 * s, 12.5 * s, CUERPO);
                }
                Pieza::Momento(i) => {
                    let Some((_, md)) = medidas.iter().find(|(k, _)| *k == i) else {
                        continue;
                    };
                    let mo = &e.momentos[i];
                    let cm = clave(&mo.id);
                    let tarjeta = RectF {
                        x: x_tarjeta,
                        y,
                        ancho: ancho_tarjeta,
                        alto: *alto,
                    };
                    // Clic en la tarjeta: su detalle. Va primero para que
                    // los botones de dentro, apuntados despues, le ganen.
                    zonas.push((tarjeta, Accion::AbrirDetalle(cm)));
                    let encima = dentro(tarjeta, raton) && dentro(area, raton);
                    let corrigiendo = e.corrigiendo.as_deref() == Some(mo.id.as_str());
                    // La hora, a la izquierda de su punto.
                    let hora = dias::hora(mo.cuando, e.desfase);
                    let (hw, _) = p.medir_texto(&hora, 13.5 * s);
                    p.texto(
                        &hora,
                        riel - AL_RIEL * s - hw,
                        y + PAD * s,
                        13.5 * s,
                        CUERPO,
                    );
                    // El punto: el mas reciente de la ultima hora, lleno; los demas, aro.
                    let cy = y + PAD * s + 9.0 * s;
                    p.circulo((riel, cy), 8.0 * s, FONDO_V);
                    let reciente = ahora - mo.cuando < 3_600_000 && visibles.last() == Some(&i);
                    if reciente || corrigiendo {
                        p.circulo((riel, cy), 6.0 * s, ACENTO);
                    } else {
                        p.anillo((riel, cy), 5.0 * s, 2.5 * s, ACENTO);
                    }
                    // La tarjeta.
                    let fondo = if corrigiendo {
                        hex(0x23324A)
                    } else if encima {
                        hex(0x313134)
                    } else {
                        TARJETA
                    };
                    p.rellenar_redondeado(tarjeta, 12.0 * s, fondo);
                    let tx = tarjeta.x + PAD * s;
                    let dw = (tarjeta.ancho - 2.0 * PAD * s).max(20.0);
                    let mut ty = y + PAD * s;
                    titulo_en_color(p, &mo.titulo, tx, ty, dw, s);
                    ty += md.titulo;
                    if md.desc > 0.0 {
                        ty += 4.0 * s;
                        p.texto_ajustado(&mo.descripcion, tx, ty, 14.0 * s, dw, CUERPO);
                        ty += md.desc;
                    }
                    if md.filas_de_fotos > 0 {
                        ty += 10.0 * s;
                        for (k, f) in mo.fotos.iter().enumerate() {
                            let fila = k / md.fotos_por_fila;
                            let col = k % md.fotos_por_fila;
                            let r = RectF {
                                x: tx + col as f32 * (FOTO_W + 8.0) * s,
                                y: ty + fila as f32 * (FOTO_H + 8.0) * s,
                                ancho: FOTO_W * s,
                                alto: FOTO_H * s,
                            };
                            pintar_foto(p, &e.minis, &e.almacen.ruta(f), r, s);
                            zonas.push((r, Accion::AbrirFoto(cm, k)));
                        }
                        ty += md.filas_de_fotos as f32 * (FOTO_H + 8.0) * s - 8.0 * s;
                    }
                    if mo.audio.is_some() {
                        ty += 10.0 * s;
                        let chapa = pintar_chapa_voz(p, mo.duracion_ms, tx, ty, raton, s, textos);
                        zonas.push((chapa, Accion::Oir(cm)));
                    }
                    // Con el raton encima, en la esquina: leccion, compartir
                    // como imagen y (fuera de la busqueda) corregir y borrar.
                    // Marcado como leccion, la bombilla se ve siempre.
                    let marcado = mo.leccion.is_some();
                    if encima {
                        let l = 32.0 * s;
                        let mut acciones: Vec<(
                            Accion,
                            &pixpin_render::icono::Icono,
                            Color,
                            Option<Color>,
                        )> = vec![
                            (
                                Accion::Leccion(cm),
                                &mi::LIGHTBULB,
                                if marcado { ui::v2::AMARILLO } else { CUERPO },
                                marcado.then_some(Color {
                                    a: 0.22,
                                    ..ui::v2::AMARILLO
                                }),
                            ),
                            (
                                Accion::MenuCompartirMomento(cm),
                                &mi::IOS_SHARE,
                                CUERPO,
                                None,
                            ),
                        ];
                        if modo != Modo::Buscar {
                            acciones.push((Accion::Corregir(cm), &mi::EDIT, CUERPO, None));
                            acciones.push((Accion::Borrar(cm), &mi::DELETE, hex(0xFF6961), None));
                        }
                        let n = acciones.len() as f32;
                        let fondo_b = RectF {
                            x: tarjeta.x + tarjeta.ancho - n * l - 10.0 * s,
                            y: tarjeta.y + 6.0 * s,
                            ancho: n * l + 4.0 * s,
                            alto: l,
                        };
                        p.rellenar_redondeado(fondo_b, l / 2.0, hex(0x3A3A3D));
                        for (k, (a, icono, tinta, fondo)) in acciones.into_iter().enumerate() {
                            let b = RectF {
                                x: fondo_b.x + 2.0 * s + k as f32 * l,
                                y: fondo_b.y,
                                ancho: l,
                                alto: l,
                            };
                            if let Some(f) = fondo {
                                p.rellenar_redondeado(b, l / 2.0, f);
                            }
                            if dentro(b, raton) {
                                let c = if matches!(a, Accion::Borrar(_)) {
                                    Color { a: 0.25, ..ROJO }
                                } else {
                                    blanco(0.1)
                                };
                                p.rellenar_redondeado(b, l / 2.0, c);
                            }
                            p.icono(icono, encoger(b, 8.0 * s), tinta);
                            zonas.push((b, a));
                        }
                    } else if marcado {
                        let c = (tarjeta.x + tarjeta.ancho - 22.0 * s, tarjeta.y + 22.0 * s);
                        archivo::marca_de_leccion(p, e, mo, c, s);
                    }
                }
            }
        }
    });
    // Solo lo que se ve dentro del area responde.
    for (r, a) in zonas {
        if dentro(area, (r.x + 1.0, r.y.max(area.y) + 1.0)) && r.y + r.alto > area.y {
            let recortada = RectF {
                y: r.y.max(area.y),
                alto: (r.y + r.alto).min(area.y + area.alto) - r.y.max(area.y),
                ..r
            };
            if recortada.alto > 0.0 {
                e.botones.zona(recortada, a);
            }
        }
    }
}

fn pintar_foto(
    p: &Pintor,
    minis: &crate::miniaturas::Miniaturas,
    ruta: &std::path::Path,
    r: RectF,
    s: f32,
) {
    p.rellenar_redondeado(r, 10.0 * s, hex(0x3A3A3D));
    if let Some((b, w, h)) = minis.ya(ruta) {
        if p.empujar_recorte_redondeado(r, 10.0 * s) {
            crate::miniaturas::pintar_recortado(p, b, r, w, h);
            p.soltar_recorte_redondeado();
        } else {
            crate::miniaturas::pintar_recortado(p, b, r, w, h);
        }
    } else {
        p.icono(
            &mi::IMAGE,
            RectF {
                x: r.x + r.ancho / 2.0 - 14.0 * s,
                y: r.y + r.alto / 2.0 - 14.0 * s,
                ancho: 28.0 * s,
                alto: 28.0 * s,
            },
            GRIS,
        );
    }
}

/// Lo que mide la zona de abajo: la fila de fotos o del estado, y la caja.
fn alto_de_la_caja(e: &Estado, p: &Pintor, w: f32, s: f32) -> f32 {
    let m = MARGEN * s;
    let ancho_texto = w - 2.0 * m - 3.0 * (BOTON * s + 4.0 * s) - 10.0 * s;
    let caja = e
        .campo
        .alto(p, ancho_texto, 15.0 * s, 1, 0.0, s)
        .clamp(52.0 * s, CAJA_MAX * s);
    let mut alto = caja + 2.0 * 12.0 * s;
    if !e.pegadas.is_empty() {
        alto += PEGADA * s + 8.0 * s;
    }
    if e.dictado.activo() || e.corrigiendo.is_some() || e.audio_pendiente.is_some() {
        alto += 26.0 * s;
    }
    alto
}

fn pintar_caja(e: &mut Estado, p: &Pintor, zona: RectF, s: f32, textos: &Catalogo) {
    let m = MARGEN * s;
    p.rellenar(zona, FONDO_V);
    p.linea((0.0, zona.y), (zona.ancho, zona.y), 1.0 * s, blanco(0.06));
    let mut y = zona.y + 12.0 * s;
    // La fila de lo que espera: el estado del microfono o la correccion.
    let estado = e
        .dictado
        .estado(textos)
        .or_else(|| {
            e.corrigiendo.as_ref().map(|id| {
                let hora = e
                    .momentos
                    .iter()
                    .find(|x| &x.id == id)
                    .map(|x| dias::hora(x.cuando, e.desfase))
                    .unwrap_or_default();
                let mut a = fluent_bundle::FluentArgs::new();
                a.set("hora", hora);
                textos.t_args("timeline-corrigiendo", &a)
            })
        })
        .or_else(|| {
            e.audio_pendiente
                .as_ref()
                .map(|_| textos.t("timeline-voz-esperando"))
        });
    if let Some(t) = estado {
        let color = if e.dictado.grabando() {
            ROJO
        } else {
            ui::v2::CIAN
        };
        if e.dictado.grabando() {
            p.circulo((m + 5.0 * s, y + 9.0 * s), 5.0 * s, ROJO);
            p.texto_linea(&t, m + 16.0 * s, y, 13.5 * s, zona.ancho - 2.0 * m, color);
        } else {
            p.texto_linea(&t, m, y, 13.5 * s, zona.ancho - 2.0 * m, color);
        }
        y += 26.0 * s;
    }
    if !e.pegadas.is_empty() {
        let lado = PEGADA * s;
        for (k, pg) in e.pegadas.iter().enumerate() {
            let r = RectF {
                x: m + k as f32 * (lado + 8.0 * s),
                y,
                ancho: lado,
                alto: lado,
            };
            pintar_foto(p, &e.minis, &pg.ruta, r, s);
            let x = RectF {
                x: r.x + r.ancho - 20.0 * s,
                y: r.y - 4.0 * s,
                ancho: 24.0 * s,
                alto: 24.0 * s,
            };
            p.circulo((x.x + 12.0 * s, x.y + 12.0 * s), 11.0 * s, hex(0x48484A));
            p.icono(&mi::CLOSE, encoger(x, 6.0 * s), TEXTO_V);
            e.botones.zona(x, Accion::QuitarPegada(k));
        }
        y += lado + 8.0 * s;
    }
    let lado = BOTON * s;
    let botones_w = 3.0 * (lado + 4.0 * s);
    let ancho_texto = zona.ancho - 2.0 * m - botones_w - 10.0 * s;
    let alto = e
        .campo
        .alto(p, ancho_texto, 15.0 * s, 1, 0.0, s)
        .clamp(52.0 * s, CAJA_MAX * s);
    let caja = RectF {
        x: m,
        y,
        ancho: zona.ancho - 2.0 * m,
        alto,
    };
    p.rellenar_redondeado(caja, 14.0 * s, ACENTO);
    p.rellenar_redondeado(encoger(caja, 1.5 * s), 13.0 * s, hex(0x2A2A2D));
    let pista = textos.t("timeline-pista");
    let texto = RectF {
        x: caja.x + 4.0 * s,
        y: caja.y,
        ancho: ancho_texto,
        alto,
    };
    let campo = &e.campo;
    p.con_recorte(texto, |p| {
        // Lo largo se desplaza para que el cursor (al final) se vea.
        let lleno = campo.alto(p, ancho_texto, 15.0 * s, 1, 0.0, s);
        let ty = if lleno <= alto {
            texto.y + (alto - lleno) / 2.0 + 10.0 * s
        } else {
            texto.y + 10.0 * s - (lleno - alto)
        };
        campo.pintar_texto(
            p,
            texto.x + 10.0 * s,
            ty,
            ancho_texto - 20.0 * s,
            15.0 * s,
            true,
            &pista,
            s,
        );
    });
    // Los botones: foto, microfono y apuntar.
    let by = caja.y + caja.alto - lado - 6.0 * s;
    let mut bx = caja.x + caja.ancho - 6.0 * s - lado;
    let hay =
        !e.campo.texto.trim().is_empty() || !e.pegadas.is_empty() || e.audio_pendiente.is_some();
    boton_icono(
        p,
        &mut e.botones,
        RectF {
            x: bx,
            y: by,
            ancho: lado,
            alto: lado,
        },
        Accion::Enviar,
        &mi::SEND,
        if hay { ui::v2::BLANCO } else { GRIS },
        hay.then_some(ui::v2::AZUL),
        s,
    );
    bx -= lado + 4.0 * s;
    let grabando = e.dictado.grabando();
    boton_icono(
        p,
        &mut e.botones,
        RectF {
            x: bx,
            y: by,
            ancho: lado,
            alto: lado,
        },
        Accion::Microfono,
        if grabando { &mi::STOP } else { &mi::MIC },
        if grabando { ui::v2::BLANCO } else { CUERPO },
        grabando.then_some(ROJO),
        s,
    );
    bx -= lado + 4.0 * s;
    boton_icono(
        p,
        &mut e.botones,
        RectF {
            x: bx,
            y: by,
            ancho: lado,
            alto: lado,
        },
        Accion::Imagen,
        &mi::IMAGE,
        CUERPO,
        None,
        s,
    );
    if e.corrigiendo.is_some() {
        // La ✕ de dejar de corregir, en la fila del estado.
        let l = 24.0 * s;
        let r = RectF {
            x: zona.ancho - m - l,
            y: zona.y + 10.0 * s,
            ancho: l,
            alto: l,
        };
        p.icono(&mi::CLOSE, encoger(r, 3.0 * s), GRIS);
        e.botones.zona(r, Accion::DejarDeCorregir);
    }
}

/// El menu de exportar: HTML o PDF, y que se va a exportar.
#[allow(clippy::too_many_arguments)] // estado, pintor, ancla, tamano, escala y textos
fn pintar_menu(
    e: &mut Estado,
    p: &Pintor,
    ancla: RectF,
    w: f32,
    h: f32,
    s: f32,
    textos: &Catalogo,
) {
    // Todo lo de debajo cierra el menu.
    e.botones.zona(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho: w,
            alto: h,
        },
        Accion::SoltarMenu,
    );
    let que = match e.modo() {
        Modo::Hoy => textos.t("timeline-ultimas-24h"),
        Modo::Dia(d) => dias::nombre_del_dia(d, e.ingles),
        Modo::Buscar => con_n(textos, "timeline-sub-buscar", e.visibles().len()),
        Modo::Momentos | Modo::Estado if !e.elegidos.is_empty() => {
            con_n(textos, "timeline-cal-elegidos", e.elegidos.len())
        }
        Modo::Momentos | Modo::Estado => dias::nombre_del_mes(Mes::de(e.hoy()), e.ingles),
        Modo::Lecciones => String::new(),
    };
    let ancho = 260.0 * s;
    let fila = 44.0 * s;
    let caja = RectF {
        x: (ancla.x + ancla.ancho - ancho).max(8.0 * s),
        y: ancla.y + ancla.alto + 6.0 * s,
        ancho,
        alto: 30.0 * s + 2.0 * fila + 8.0 * s,
    };
    p.rellenar_redondeado(caja, 12.0 * s, blanco(0.12));
    p.rellenar_redondeado(encoger(caja, 1.0 * s), 11.0 * s, hex(0x2C2C2E));
    e.botones.zona(caja, Accion::Fondo);
    p.texto_linea(
        &que,
        caja.x + 14.0 * s,
        caja.y + 9.0 * s,
        12.5 * s,
        ancho - 28.0 * s,
        GRIS,
    );
    for (k, (f, clave)) in [
        (Formato::Html, "timeline-exportar-html"),
        (Formato::Pdf, "timeline-exportar-pdf"),
    ]
    .into_iter()
    .enumerate()
    {
        let r = RectF {
            x: caja.x + 4.0 * s,
            y: caja.y + 30.0 * s + k as f32 * fila,
            ancho: ancho - 8.0 * s,
            alto: fila,
        };
        if dentro(r, e.botones.raton) {
            p.rellenar_redondeado(r, 8.0 * s, blanco(0.08));
        }
        p.icono(
            if f == Formato::Html {
                &mi::PUBLIC
            } else {
                &mi::DESCRIPTION
            },
            RectF {
                x: r.x + 10.0 * s,
                y: r.y + (fila - 20.0 * s) / 2.0,
                ancho: 20.0 * s,
                alto: 20.0 * s,
            },
            ui::v2::CIAN,
        );
        let t = textos.t(clave);
        let (_, th) = p.medir_texto(&t, 14.5 * s);
        p.texto(
            &t,
            r.x + 42.0 * s,
            r.y + (fila - th) / 2.0,
            14.5 * s,
            TEXTO_V,
        );
        e.botones.zona(r, Accion::Formato(f));
    }
}

fn pintar_aviso(e: &mut Estado, p: &Pintor, w: f32, base: f32, s: f32, textos: &Catalogo) {
    let Some((t, _)) = e.aviso.clone() else {
        return;
    };
    let tam = 14.0 * s;
    let accion = if e.deshacer.is_some() {
        Some((
            Accion::Deshacer,
            textos.t("timeline-deshacer"),
            Some("Ctrl Z"),
            &mi::UNDO,
        ))
    } else if e.exportado.is_some() {
        Some((
            Accion::AbrirExportado,
            textos.t("timeline-abrir"),
            None,
            &mi::OPEN_IN_NEW,
        ))
    } else {
        None
    };
    let extra = accion.as_ref().map_or(0.0, |(_, r, k, _)| {
        ui::ancho_de_boton(p, true, r, *k, s) + 8.0 * s
    });
    let (tw, th) = p.medir_texto_ajustado(&t, tam, (w - 60.0 * s - extra).max(40.0 * s));
    let alto = if accion.is_some() {
        (th + 16.0 * s).max(48.0 * s)
    } else {
        th + 16.0 * s
    };
    let caja = RectF {
        x: (w - tw - extra) / 2.0 - 14.0 * s,
        y: base - alto - 14.0 * s,
        ancho: tw + extra + 28.0 * s,
        alto,
    };
    p.rellenar_redondeado(
        caja,
        8.0 * s,
        Color {
            a: 0.92,
            ..Color::NEGRO
        },
    );
    e.botones.zona(caja, Accion::Fondo);
    p.texto_ajustado(
        &t,
        caja.x + 14.0 * s,
        caja.y + (alto - th) / 2.0,
        tam,
        tw + 2.0,
        TEXTO,
    );
    if let Some((a, rot, k, icono)) = accion {
        let bw = ui::ancho_de_boton(p, true, &rot, k, s);
        let b = RectF {
            x: caja.x + caja.ancho - 4.0 * s - bw,
            y: caja.y + (alto - 40.0 * s) / 2.0,
            ancho: bw,
            alto: 40.0 * s,
        };
        ui::boton_v2(
            p,
            &mut e.botones,
            b,
            a,
            Some(icono),
            &rot,
            k,
            None,
            hex(0x64D2FF),
            s,
        );
    }
}

/// La chapita «▶ Nota de voz · 0:42» de un momento con audio, con su
/// esquina de arriba a la izquierda en `(x, y)`. Devuelve su caja (la zona
/// de oirla), igual en la tarjeta y en el detalle.
#[allow(clippy::too_many_arguments)] // pintor, momento, donde, raton, escala y textos
fn pintar_chapa_voz(
    p: &Pintor,
    duracion_ms: i64,
    x: f32,
    y: f32,
    raton: (f32, f32),
    s: f32,
    textos: &Catalogo,
) -> RectF {
    let mut rot = textos.t("timeline-nota-de-voz");
    if duracion_ms > 0 {
        let seg = (duracion_ms + 500) / 1000;
        rot.push_str(&format!(" · {}:{:02}", seg / 60, seg % 60));
    }
    let (rw, rh) = p.medir_texto(&rot, 13.0 * s);
    let chapa = RectF {
        x,
        y,
        ancho: rw + 46.0 * s,
        alto: CHAPA_VOZ * s,
    };
    let sobre = dentro(chapa, raton);
    p.rellenar_redondeado(
        chapa,
        chapa.alto / 2.0,
        if sobre { hex(0x1F3B63) } else { hex(0x182B47) },
    );
    p.icono(
        &mi::PLAY_ARROW,
        RectF {
            x: chapa.x + 8.0 * s,
            y: chapa.y + (chapa.alto - 20.0 * s) / 2.0,
            ancho: 20.0 * s,
            alto: 20.0 * s,
        },
        ui::v2::CIAN,
    );
    p.texto(
        &rot,
        chapa.x + 34.0 * s,
        chapa.y + (chapa.alto - rh) / 2.0,
        13.0 * s,
        ui::v2::CIAN,
    );
    chapa
}

/// Apunta la zona `r` recortada a lo que se ve de `area`: lo que se ha ido
/// con el desplazamiento no debe responder bajo la barra de arriba.
fn zona_en(botones: &mut Botones<Accion>, area: RectF, r: RectF, a: Accion) {
    let y0 = r.y.max(area.y);
    let y1 = (r.y + r.alto).min(area.y + area.alto);
    if y1 > y0 {
        botones.zona(
            RectF {
                y: y0,
                alto: y1 - y0,
                ..r
            },
            a,
        );
    }
}
