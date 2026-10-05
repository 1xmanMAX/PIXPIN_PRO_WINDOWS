//! **La tarjeta de Proyectos dentro de la ventana del chat**, la de PixPin
//! Android (`ui/Proyectos.kt`), y el interruptor de la barra de arriba que
//! cambia entre ella y el chat de siempre.
//!
//! El usuario lo pidio en dos tiempos. Primero: «con solo darle click al
//! chat me abra una visualizacion de proyectos en vez de chat, el mismo que
//! de PixPin Android», con un interruptor en la barra de arriba. Despues,
//! al verla a pantalla entera: «no se esta adaptando a la interfaz de una
//! PC». Asi que ahora cada proyecto tiene **su vista de chat y su vista de
//! proyecto**, y la ventana es la del chat de siempre (maestro-detalle): la
//! lista de proyectos a la izquierda, con su buscador, sus botones, su
//! sincronizar y su «+», y en el panel derecho, en vez de la conversacion
//! del elegido, **su tarjeta**. Estrecha, un solo panel: la lista, o la
//! tarjeta con el circulo de volver arriba a la izquierda (el mismo del
//! chat) que lleva de vuelta a la lista. El interruptor cambia lo que ocupa
//! el panel derecho y el proyecto elegido se queda: de la tarjeta de «Casa
//! Lima» al chat de «Casa Lima» y al reves. Recuerda donde se quedo
//! (`vista-del-chat.txt` al lado del almacen) y sin nada guardado se abre en
//! Proyectos, que es lo que se pidio.
//!
//! De proyecto se cambia desde la lista (clic, o ↑↓ con el teclado); la
//! rueda sobre la tarjeta la recorre si no cabe entera. Lo que se copia del
//! movil, pieza a pieza:
//!
//! - **La tarjeta**: nombre grande (normal si esta archivado), «N hojas» o
//!   «a de N hojas anotadas» si sale de un PDF, el boton de rejilla / una
//!   pagina y los tres puntos (cambiar el nombre, archivar, compartir, lo
//!   borrado y borrar, con aviso).
//! - **La portada** de la hoja elegida, grande y con su nombre en la esquina;
//!   un clic la abre. **La tira**: un clic sube la hoja a la portada, doble
//!   clic la abre; marco del color de su lienzo, «MD»/«2D» en la esquina, el
//!   punto de las paginas anotadas y el contador de sus sublienzos plegados.
//! - **La barra de cristal** del proyecto (de pie a un lado con el panel
//!   apaisado): Chat, Hoja, Nota y Tabla.
//!
//! Abrir una hoja hace lo mismo que en el chat (`abrir_hojas`, el editor de
//! notas, el lienzo de la foto); lo que en el PC solo vive dentro del chat
//! (una tabla) pasa al chat con ella abierta.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pixpin_geom::{Punto, Rect};
use pixpin_proyecto::almacen::Ficha;
use pixpin_proyecto::cuaderno::Clase;
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Ubicacion};

use super::{
    Abierto, OpcionesLienzo, VK_ABAJO, VK_ARRIBA, VK_DERECHA, VK_ENTRAR, VK_ESCAPE, VK_IZQUIERDA,
};

mod cargar;
#[cfg(test)]
mod muestra;
mod pintar;

pub(super) use pintar::{pintar, pintar_interruptor};

/// Donde se recuerda la ultima vista: al lado del almacen y sin viajar,
/// como `fondo-del-chat.txt`. Es de este equipo, no del proyecto.
const FICHERO_VISTA: &str = "vista-del-chat.txt";
const VK_REPAG: u32 = 0x21;
const VK_AVPAG: u32 = 0x22;
const VK_FIN: u32 = 0x23;
const VK_INICIO: u32 = 0x24;

/// Cuanto espera un segundo clic para contar como doble: el de Windows de
/// fabrica. Se mide aqui porque el overlay no manda el doble clic.
const DOBLE_CLIC: Duration = Duration::from_millis(500);
/// Lo que da una muesca de la rueda (`WHEEL_DELTA`).
const MUESCA: i32 = 120;
/// Cuantos proyectos leidos se guardan: el que se ve y los ultimos que se
/// vieron, para volver a ellos sin esperar. Mas es memoria para proyectos
/// que nadie mira.
const EN_MEMORIA: usize = 6;
/// Cada cuanto, como mucho, se vuelven a pedir las paginas que aun se
/// estan pintando.
const REINTENTO: Duration = Duration::from_millis(250);
/// El lado mayor de la portada guardada: el ultimo escalon de nitidez.
const LADO_GRANDE: u32 = 2000;

/// Las acciones de la barra de cristal, en su orden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AccionBarra {
    Chat,
    Hoja,
    Nota,
    Tabla,
}

pub(super) const ACCIONES: [AccionBarra; 4] = [
    AccionBarra::Chat,
    AccionBarra::Hoja,
    AccionBarra::Nota,
    AccionBarra::Tabla,
];

/// Lo que se puede pulsar, apuntado al pintar: lo que no se ve no se pulsa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Zona {
    /// El circulo de volver: a la lista, como el del chat.
    Volver,
    /// La tarjeta entera: ahi la rueda la recorre.
    Tarjeta(String),
    /// Su nombre: el boton derecho lo cambia.
    Nombre(String),
    Rejilla(String),
    Menu(String),
    Portada(String),
    /// El hueco de la tira o de la rejilla: ahi la rueda desplaza de lado.
    Tira(String),
    /// Una miniatura: el proyecto y su posicion en las hojas.
    Hoja(String, usize),
    /// El contador de sublienzos de una hoja: el proyecto y la hoja padre.
    Plegado(String, String),
    Boton(AccionBarra),
}

/// Lo que cada tarjeta recuerda mientras la ventana vive (`remember(p.id)`).
#[derive(Debug, Default, Clone)]
pub(super) struct EstadoTarjeta {
    /// La hoja que ensena la portada, por su posicion en las hojas.
    pub en_foco: usize,
    pub rejilla: bool,
    /// Lo corrida que esta la tira (o la rejilla), en pixeles.
    pub corrida: i32,
    /// Las hojas con los sublienzos desplegados.
    pub abiertos: BTreeSet<String>,
    /// La hoja (por su puesto entre las que se ven) que la tira tiene que
    /// ensenar: la corre lo justo quien pinta, que es quien sabe cuanto mide.
    pub ver: Option<usize>,
    /// Las hojas marcadas (por su `id`), el `marcado` de `ui/Proyectos.kt`:
    /// con dos o mas paginas del PDF, el menu ofrece fusionarlas (E9). Se
    /// marca aqui y no en el chat porque las paginas ya no salen en el chat.
    pub marcadas: BTreeSet<String>,
}

/// Lo que la tarjeta pide al bucle del chat, que es quien tiene el proyecto
/// abierto, la lista y la ventana.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Pedido {
    Nada,
    Pintar,
    /// Ir al chat de ese proyecto (y al mensaje, si hay).
    AlChat {
        proyecto: String,
        codigo: Option<String>,
    },
    /// Abrir la hoja `indice` del proyecto.
    AbrirHoja {
        proyecto: String,
        indice: usize,
    },
    Accion(AccionBarra, String),
    /// El menu de los tres puntos del proyecto.
    Menu(String),
    /// Pedir el nombre nuevo del proyecto.
    PedirNombre(String),
    Renombrar {
        proyecto: String,
        nombre: String,
    },
    /// Soltar el proyecto elegido y volver a la lista, como el volver del chat.
    Volver,
    /// El proyecto de al lado en la lista (↑ es -1, ↓ es +1).
    Vecino(i32),
    Cerrar,
}

/// La tarjeta del panel derecho: si se ve, de que proyecto es y lo que ya
/// esta leido.
pub(super) struct VistaProyectos {
    activa: bool,
    /// El proyecto de la tarjeta, por `id`: el que el chat tiene abierto.
    /// Lo pone el bucle en cada vuelta (`seguir`).
    actual: Option<String>,
    /// Los ultimos que se vieron, el mas reciente delante: son los que se
    /// quedan leidos.
    recientes: VecDeque<String>,
    datos: HashMap<String, cargar::Hojas>,
    pedidos: HashSet<String>,
    resumenes: HashMap<String, cargar::Resumen>,
    /// Con que fichas se pidieron los resumenes (ids y `tocado`): si cambian,
    /// se piden otra vez.
    firma: u64,
    pub(super) tarjetas: RefCell<HashMap<String, EstadoTarjeta>>,
    /// Lo bajada que esta la tarjeta cuando no cabe, y hasta donde se puede
    /// bajar (lo apunta quien pinta, que es quien sabe lo que mide).
    desplazamiento: i32,
    pub(super) tope: Cell<i32>,
    /// Lo alto que se ve de la tarjeta: lo que baja Av Pag.
    pub(super) alto_hueco: Cell<i32>,
    /// Donde esta el raton, para saber a que va la rueda.
    raton: Punto,
    pub(super) zonas: RefCell<Vec<(Rect, Zona)>>,
    ultimo_clic: Option<(Zona, Instant)>,
    /// Las vistas previas que se ven (paginas del PDF, fotos), apuntadas al
    /// pintar para subirlas antes del fotograma siguiente.
    pub(super) fondos: RefCell<Vec<PathBuf>>,
    /// La portada a su nitidez, y la que se quiso pintar en el ultimo
    /// fotograma. Una sola: once megas por pagina son muchos para guardar
    /// las de todos.
    pub(super) grande_pedida: RefCell<Option<PathBuf>>,
    pub(super) grande: Option<(PathBuf, crate::miniaturas::Miniaturas)>,
    reintento: Instant,
    /// El nombre escrito en la caja de texto de Windows, que vive en su hilo.
    nombre_escrito: Arc<Mutex<Option<(String, String)>>>,
    /// Donde quedo el interruptor en el ultimo fotograma: depende de lo que
    /// mide el nombre del programa, y eso solo lo sabe quien pinta.
    pub(super) interruptor: Cell<Option<pixpin_ui::proyectos::Interruptor>>,
}

fn firma_de(fichas: &[Ficha]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for f in fichas {
        f.id.hash(&mut h);
        f.tocado.hash(&mut h);
    }
    h.finish()
}

/// Las teclas que, con Ctrl, cambian la escala de la interfaz.
fn es_de_la_escala(vk: u32) -> bool {
    use super::{VK_CERO, VK_CERO_NUM, VK_MAS, VK_MAS_NUM, VK_MENOS, VK_MENOS_NUM};
    matches!(
        vk,
        VK_MAS | VK_MENOS | VK_MAS_NUM | VK_MENOS_NUM | VK_CERO | VK_CERO_NUM
    )
}

/// Lo que dice el fichero de la vista: `true` si hay que abrir en Proyectos.
/// Sin fichero, Proyectos: es lo que el usuario pidio al darle al chat.
pub(super) fn abre_en_proyectos(guardado: Option<&str>) -> bool {
    guardado.map(str::trim) != Some("chat")
}

/// El puesto de la lista al que llevan ↑ o ↓ (`paso`) desde el proyecto
/// elegido, en la lista tal como se ve (`orden`, ya filtrada por el
/// buscador). Sin ninguno elegido, ↓ va al primero y ↑ al ultimo; en los
/// extremos se queda donde esta, como la lista de Telegram.
pub(super) fn vecino(
    orden: &[usize],
    fichas: &[Ficha],
    actual: Option<&str>,
    paso: i32,
) -> Option<usize> {
    if orden.is_empty() {
        return None;
    }
    let ultimo = orden.len() as i32 - 1;
    let puesto = actual.and_then(|id| {
        orden
            .iter()
            .position(|&i| fichas.get(i).is_some_and(|f| f.id == id))
    });
    let nuevo = match puesto {
        Some(p) => (p as i32 + paso).clamp(0, ultimo),
        None if paso < 0 => ultimo,
        None => 0,
    };
    orden.get(nuevo as usize).copied()
}

/// Las hojas que se ven de un proyecto, por su posicion: los sublienzos
/// solo si su padre (y los de encima) estan desplegados, como `seVe` del
/// movil, con el mismo tope de ocho para que dos hojas que se apunten una a
/// otra no den vueltas.
pub(super) fn hojas_visibles(hojas: &[cargar::Hoja], abiertos: &BTreeSet<String>) -> Vec<usize> {
    let padre_de: HashMap<&str, Option<&str>> = hojas
        .iter()
        .map(|h| (h.id.as_str(), h.padre.as_deref()))
        .collect();
    fn se_ve<'a>(
        padre_de: &HashMap<&'a str, Option<&'a str>>,
        abiertos: &BTreeSet<String>,
        mut padre: Option<&'a str>,
    ) -> bool {
        for _ in 0..8 {
            match padre {
                None => return true,
                Some(p) if abiertos.contains(p) => padre = padre_de.get(p).copied().flatten(),
                Some(_) => return false,
            }
        }
        false
    }
    (0..hojas.len())
        .filter(|&i| se_ve(&padre_de, abiertos, hojas[i].padre.as_deref()))
        .collect()
}

/// Cuantos sublienzos cuelgan de cada hoja.
pub(super) fn hijos_de(hojas: &[cargar::Hoja]) -> HashMap<&str, usize> {
    let mut n = HashMap::new();
    for h in hojas {
        if let Some(p) = h.padre.as_deref() {
            *n.entry(p).or_insert(0) += 1;
        }
    }
    n
}

/// La etiqueta de que clase de hoja es (`deQueEs`), o nada: las paginas de
/// un PDF no la llevan porque en un proyecto de PDF son casi todas.
pub(super) fn de_que_es(m: &pixpin_proyecto::cuaderno::Mensaje) -> Option<&'static str> {
    if m.pagina.is_some() {
        return None;
    }
    match m.clase {
        Some(Clase::Nota) => Some("MD"),
        Some(Clase::MiniApp) if m.miniapp.as_deref() == Some(pixpin_proyecto::tabla::MINIAPP) => {
            Some("fx")
        }
        Some(Clase::Dibujo) => Some("2D"),
        _ => None,
    }
}

impl VistaProyectos {
    pub(super) fn nueva(ubicacion: &Ubicacion) -> VistaProyectos {
        let guardado = std::fs::read_to_string(ubicacion.raiz().join(FICHERO_VISTA)).ok();
        VistaProyectos {
            activa: abre_en_proyectos(guardado.as_deref()),
            actual: None,
            recientes: VecDeque::new(),
            datos: HashMap::new(),
            pedidos: HashSet::new(),
            resumenes: HashMap::new(),
            firma: 0,
            tarjetas: RefCell::new(HashMap::new()),
            desplazamiento: 0,
            tope: Cell::new(0),
            alto_hueco: Cell::new(0),
            raton: Punto { x: 0, y: 0 },
            zonas: RefCell::new(Vec::new()),
            ultimo_clic: None,
            fondos: RefCell::new(Vec::new()),
            grande_pedida: RefCell::new(None),
            grande: None,
            reintento: Instant::now(),
            nombre_escrito: Arc::new(Mutex::new(None)),
            interruptor: Cell::new(None),
        }
    }

    /// Que mitad del interruptor hay bajo el punto: `Some(true)` Proyectos,
    /// `Some(false)` Chat. Pulsar una mitad la elige, como un selector de
    /// dos: pulsar «Chat» estando en el chat no cambia nada.
    pub(super) fn interruptor_en(&self, l: Punto) -> Option<bool> {
        let i = self.interruptor.get()?;
        if i.proyectos.contiene(l) {
            Some(true)
        } else if i.caja.contiene(l) {
            Some(false)
        } else {
            None
        }
    }

    /// El proyecto de la tarjeta: el que el chat tiene abierto. Al cambiar,
    /// la tarjeta nueva sale desde arriba y la anterior se queda leida por
    /// si se vuelve a ella.
    pub(super) fn seguir(&mut self, id: Option<&str>) {
        if self.actual.as_deref() == id {
            return;
        }
        self.actual = id.map(String::from);
        self.desplazamiento = 0;
        self.ultimo_clic = None;
        if let Some(id) = id {
            self.recientes.retain(|r| r != id);
            self.recientes.push_front(id.to_string());
            self.recientes.truncate(EN_MEMORIA);
        }
    }

    /// Sube lo que se va a ver ANTES del fotograma: las vistas previas de las
    /// miniaturas (a `previas`, las del chat) y la portada a su nitidez.
    /// Devuelve si quedaron por subir.
    pub(super) fn preparar(
        &mut self,
        previas: &mut crate::miniaturas::Miniaturas,
        motor: &pixpin_render::MotorRender,
    ) -> bool {
        let rutas = self.fondos.borrow().clone();
        let faltan = !rutas.is_empty() && previas.asegurar(&rutas, motor);
        let pedida = self.grande_pedida.borrow().clone();
        if let Some(r) = pedida {
            if self.grande.as_ref().is_none_or(|(g, _)| *g != r) {
                self.grande = Some((
                    r.clone(),
                    crate::miniaturas::Miniaturas::con_lado(LADO_GRANDE),
                ));
            }
            if let Some((_, m)) = self.grande.as_mut() {
                m.asegurar(&[r], motor);
            }
        }
        faltan
    }

    /// Si lo pintado se quedo esperando una imagen que aun no se subio: otra
    /// vuelta, o no saldria hasta mover el raton.
    pub(super) fn falta_algo(&self, previas: &crate::miniaturas::Miniaturas) -> bool {
        if !self.activa || self.actual.is_none() {
            return false;
        }
        let grande = self.grande_pedida.borrow().as_ref().is_some_and(|r| {
            self.grande
                .as_ref()
                .is_none_or(|(g, m)| g != r || m.pendiente(r))
        });
        grande || self.fondos.borrow().iter().any(|r| previas.pendiente(r))
    }

    /// Suelta los bitmaps de la portada (dispositivo perdido).
    pub(super) fn soltar(&mut self) {
        if let Some((_, m)) = self.grande.as_mut() {
            m.soltar();
        }
    }

    pub(super) fn activa(&self) -> bool {
        self.activa
    }

    /// Cambia de vista con el interruptor, y lo apunta para la proxima vez.
    pub(super) fn poner(&mut self, ubicacion: &Ubicacion, proyectos: bool) {
        self.cambiar(proyectos);
        let texto = if proyectos { "proyectos" } else { "chat" };
        if let Err(e) = std::fs::write(ubicacion.raiz().join(FICHERO_VISTA), texto) {
            tracing::warn!(?e, "no se pudo recordar la vista del chat");
        }
    }

    fn cambiar(&mut self, proyectos: bool) {
        self.activa = proyectos;
        if !proyectos {
            // Lo grande se suelta al salir: son megas que el chat no usa.
            self.grande = None;
            self.zonas.borrow_mut().clear();
        }
    }

    /// Al chat porque se pidio un proyecto concreto en el (su boton Chat, el
    /// universo, una tabla). **No se apunta**: es ir y volver, como abrir el
    /// chat de un proyecto en el movil, y lo que manda al abrir la ventana es
    /// lo que se eligio con el interruptor.
    pub(super) fn ver_chat(&mut self) {
        self.cambiar(false);
    }

    /// A Proyectos desde el menu del chat (⋮ → Proyectos): tampoco se apunta.
    pub(super) fn ver_proyectos(&mut self) {
        self.cambiar(true);
    }

    pub(super) fn tarjeta(&self, id: &str) -> EstadoTarjeta {
        self.tarjetas.borrow().get(id).cloned().unwrap_or_default()
    }

    /// Cambia lo que recuerda una tarjeta. Con `&self` porque quien pinta
    /// tambien lo ajusta (lo corrida que puede estar la tira).
    pub(super) fn con_tarjeta<R>(&self, id: &str, f: impl FnOnce(&mut EstadoTarjeta) -> R) -> R {
        f(self
            .tarjetas
            .borrow_mut()
            .entry(id.to_string())
            .or_default())
    }

    pub(super) fn datos(&self, id: &str) -> Option<&cargar::Hojas> {
        self.datos.get(id)
    }

    pub(super) fn resumen(&self, id: &str) -> Option<&cargar::Resumen> {
        self.resumenes.get(id)
    }

    /// Lo bajada que esta la tarjeta, ya sujeta a lo que se puede bajar.
    pub(super) fn desplazamiento(&self) -> i32 {
        self.desplazamiento.clamp(0, self.tope.get().max(0))
    }

    fn bajar(&mut self, cuanto: i32) -> Pedido {
        let antes = self.desplazamiento();
        self.desplazamiento = (antes + cuanto).clamp(0, self.tope.get().max(0));
        if self.desplazamiento == antes {
            Pedido::Nada
        } else {
            Pedido::Pintar
        }
    }

    /// Para las muestras: lo leido, sin pasar por el hilo.
    #[cfg(test)]
    pub(super) fn meter_datos(&mut self, h: cargar::Hojas) {
        self.datos.insert(h.id.clone(), h);
    }

    #[cfg(test)]
    pub(super) fn meter_resumenes(&mut self, r: HashMap<String, cargar::Resumen>) {
        self.resumenes = r;
    }

    /// Olvida lo leido de un proyecto: la proxima vuelta lo vuelve a pedir.
    pub(super) fn olvidar(&mut self, id: &str) {
        self.datos.remove(id);
        self.pedidos.remove(id);
    }

    /// Lo que hay que hacer en cada vuelta del bucle: recoger lo leido, pedir
    /// lo que falte del proyecto de la tarjeta, reintentar las paginas que se
    /// estaban pintando y aplicar un nombre recien escrito. Devuelve si hay
    /// que pintar y, si toca, un pedido para el bucle.
    pub(super) fn latido(
        &mut self,
        ubicacion: &Ubicacion,
        fichas: &[Ficha],
    ) -> (bool, Option<Pedido>) {
        let mut pintar = false;
        for hecho in cargar::recoger() {
            match hecho {
                cargar::Hecho::Hojas(h) => {
                    self.pedidos.remove(&h.id);
                    self.datos.insert(h.id.clone(), h);
                    pintar = true;
                }
                cargar::Hecho::Resumenes(r) => {
                    self.resumenes = r;
                    pintar = true;
                }
            }
        }
        let escrito = self.nombre_escrito.lock().ok().and_then(|mut g| g.take());
        let pedido = escrito.map(|(proyecto, nombre)| Pedido::Renombrar { proyecto, nombre });
        if !self.activa {
            return (false, pedido);
        }
        let firma = firma_de(fichas);
        if firma != self.firma {
            self.firma = firma;
            cargar::pedir_resumenes(ubicacion, fichas);
        }
        let Some(f) = self
            .actual
            .as_deref()
            .and_then(|id| fichas.iter().find(|f| f.id == id))
        else {
            return (pintar, pedido);
        };
        // Lo leido de una ficha que se toco despues ya no vale.
        if self.datos.get(&f.id).is_some_and(|d| d.tocado != f.tocado) {
            self.datos.remove(&f.id);
        }
        if !self.datos.contains_key(&f.id) && self.pedidos.insert(f.id.clone()) {
            cargar::pedir_hojas(ubicacion, f, true);
        }
        // Los que no se han visto hace rato se sueltan.
        if self.datos.len() > EN_MEMORIA {
            let recientes = &self.recientes;
            self.datos.retain(|id, _| recientes.contains(id));
        }
        if self.reintento.elapsed() >= REINTENTO {
            self.reintento = Instant::now();
            let id = f.id.clone();
            pintar |= self.pedir_paginas(ubicacion, &id);
        }
        (pintar, pedido)
    }

    /// Las paginas del documento que se ven y aun no tienen vista: se piden
    /// (o se recogen si ya estan pintadas). Solo las del proyecto de la
    /// tarjeta y en la ventana de lo que se ve, como el `PreparadorDeMiniaturas`.
    fn pedir_paginas(&mut self, ubicacion: &Ubicacion, id: &str) -> bool {
        let vistas: Vec<usize> = self
            .zonas
            .borrow()
            .iter()
            .filter_map(|(_, z)| match z {
                Zona::Hoja(p, n) if p == id => Some(*n),
                _ => None,
            })
            .chain(std::iter::once(self.tarjeta(id).en_foco))
            .collect();
        let Some(d) = self.datos.get_mut(id) else {
            return false;
        };
        let mut hubo = false;
        for n in vistas {
            let Some(h) = d.hojas.get_mut(n) else {
                continue;
            };
            let sin_fondo = match &h.vista {
                None => h.por_pedir,
                Some(super::Ojeada::Lienzo(l)) => h.mensaje.pagina.is_some() && l.fondo.is_none(),
                _ => false,
            };
            if !sin_fondo {
                continue;
            }
            if let Some(v) = super::leer_vista(ubicacion, id, &h.mensaje) {
                h.vista = Some(v);
                h.por_pedir = false;
                hubo = true;
            }
        }
        hubo
    }

    fn zona_en(&self, l: Punto) -> Option<Zona> {
        self.zonas
            .borrow()
            .iter()
            .rev()
            .find(|(r, _)| r.contiene(l))
            .map(|(_, z)| z.clone())
    }

    /// Donde esta el raton, para la rueda.
    pub(super) fn mover(&mut self, l: Punto) {
        self.raton = l;
    }

    /// El clic izquierdo.
    pub(super) fn pulsar(&mut self, l: Punto) -> Pedido {
        self.raton = l;
        let Some(zona) = self.zona_en(l) else {
            return Pedido::Nada;
        };
        let doble = self
            .ultimo_clic
            .as_ref()
            .is_some_and(|(z, cuando)| *z == zona && cuando.elapsed() <= DOBLE_CLIC);
        self.ultimo_clic = if doble {
            None
        } else {
            Some((zona.clone(), Instant::now()))
        };
        match zona {
            Zona::Volver => Pedido::Volver,
            // El nombre no hace nada con un clic (`onClick = {}`): se cambia
            // con el derecho, que es el mantener pulsado del movil.
            Zona::Tarjeta(_) | Zona::Tira(_) | Zona::Nombre(_) => Pedido::Nada,
            Zona::Rejilla(id) => {
                self.con_tarjeta(&id, |t| {
                    t.rejilla = !t.rejilla;
                    t.corrida = 0;
                });
                Pedido::Pintar
            }
            Zona::Menu(id) => Pedido::Menu(id),
            Zona::Portada(id) => {
                let indice = self.tarjeta(&id).en_foco;
                Pedido::AbrirHoja {
                    proyecto: id,
                    indice,
                }
            }
            // «Un toque la sube a la portada; dos, la abren». En la rejilla
            // no hay portada que subir: el primero no hace nada y el doble
            // abre, como `RejillaDeHojas` (su `onElegir` es el vacio).
            Zona::Hoja(id, n) => {
                let rejilla = self.tarjeta(&id).rejilla;
                if doble {
                    Pedido::AbrirHoja {
                        proyecto: id,
                        indice: n,
                    }
                } else if rejilla {
                    Pedido::Nada
                } else {
                    self.con_tarjeta(&id, |t| t.en_foco = n);
                    Pedido::Pintar
                }
            }
            Zona::Plegado(id, hoja) => {
                self.con_tarjeta(&id, |t| {
                    if !t.abiertos.remove(&hoja) {
                        t.abiertos.insert(hoja);
                    }
                });
                Pedido::Pintar
            }
            Zona::Boton(a) => match self.actual.clone() {
                Some(id) => Pedido::Accion(a, id),
                None => Pedido::Nada,
            },
        }
    }

    /// El clic derecho, que es el «mantener pulsado» del movil: sobre el
    /// nombre, cambiarlo («se renombra manteniendolo pulsado»); sobre una
    /// hoja, marcarla o desmarcarla (`marcado` de `ui/Proyectos.kt`). Con dos
    /// o mas paginas del PDF marcadas, los tres puntos ofrecen fusionarlas.
    pub(super) fn pulsar_derecho(&mut self, l: Punto) -> Pedido {
        match self.zona_en(l) {
            Some(Zona::Nombre(id)) => Pedido::PedirNombre(id),
            Some(Zona::Hoja(id, n)) => {
                let Some(hoja) = self
                    .datos
                    .get(&id)
                    .and_then(|d| d.hojas.get(n))
                    .map(|h| h.id.clone())
                else {
                    return Pedido::Nada;
                };
                self.con_tarjeta(&id, |t| {
                    if !t.marcadas.remove(&hoja) {
                        t.marcadas.insert(hoja);
                    }
                });
                Pedido::Pintar
            }
            _ => Pedido::Nada,
        }
    }

    /// Si lo marcado en la tarjeta de `id` son dos o mas paginas de su PDF,
    /// lo que hace falta para fusionarlas (`FusionarPaginas.de` del movil).
    pub(super) fn peticion_de_fusion(
        &self,
        raiz: &std::path::Path,
        id: &str,
    ) -> Option<crate::fusionar_paginas::Peticion> {
        let marcadas = self.tarjeta(id).marcadas;
        if marcadas.len() < 2 {
            return None;
        }
        let d = self.datos.get(id)?;
        let elegidas: Vec<&pixpin_proyecto::cuaderno::Mensaje> = d
            .hojas
            .iter()
            .filter(|h| marcadas.contains(&h.id))
            .map(|h| &h.mensaje)
            .collect();
        crate::fusionar_paginas::de(crate::pdf_en_chat::documento_de(raiz, id), id, &elegidas)
    }

    pub(super) fn actual_id(&self) -> Option<String> {
        self.actual.clone()
    }

    /// Mueve la hoja en foco `paso` posiciones entre las que se ven.
    fn mover_foco(&mut self, paso: i32) -> bool {
        let Some(id) = self.actual.clone() else {
            return false;
        };
        let Some(d) = self.datos.get(&id) else {
            return false;
        };
        let t = self.tarjeta(&id);
        let visibles = hojas_visibles(&d.hojas, &t.abiertos);
        if visibles.is_empty() {
            return false;
        }
        let ahora = visibles.iter().position(|&n| n == t.en_foco).unwrap_or(0) as i32;
        let nueva = (ahora + paso).clamp(0, visibles.len() as i32 - 1) as usize;
        // Y la tira se corre lo justo para que se vea: la ajusta el pintado,
        // que es quien sabe cuanto mide.
        self.con_tarjeta(&id, |t| {
            t.en_foco = visibles[nueva];
            t.ver = Some(nueva);
        });
        true
    }

    /// Las teclas, la rueda y lo que se escribe. Devuelve `None` si el evento
    /// no es de la tarjeta y tiene que seguir su camino (la escala de la
    /// interfaz con Ctrl, por ejemplo).
    pub(super) fn evento(&mut self, ev: EventoOverlay) -> Option<Pedido> {
        Some(match ev {
            EventoOverlay::Rueda(delta) => {
                // Sobre la tira, la rueda la corre de lado; en lo demas de la
                // tarjeta la recorre de arriba abajo si no cabe. De proyecto
                // no cambia: eso es cosa de la lista.
                if let Some(Zona::Tira(id)) = self
                    .zonas
                    .borrow()
                    .iter()
                    .rev()
                    .find(|(r, z)| r.contiene(self.raton) && matches!(z, Zona::Tira(_)))
                    .map(|(_, z)| z.clone())
                {
                    // Dos columnas por muesca; lo que se pase lo sujeta el
                    // pintado, que sabe cuanto mide la tira.
                    self.con_tarjeta(&id, |t| {
                        t.corrida -= delta * pixpin_ui::proyectos::ANCHO_COLUMNA as i32 / 60;
                    });
                    return Some(Pedido::Pintar);
                }
                // Una fila de miniaturas por muesca, y los trocitos de un
                // panel tactil en proporcion.
                let fila = pixpin_ui::proyectos::ALTO_FILA as i32;
                self.bajar(-delta * fila / MUESCA)
            }
            EventoOverlay::RuedaHorizontal(delta) => {
                let Some(id) = self.actual.clone() else {
                    return Some(Pedido::Nada);
                };
                self.con_tarjeta(&id, |t| {
                    t.corrida += delta * pixpin_ui::proyectos::ANCHO_COLUMNA as i32 / 60;
                });
                Pedido::Pintar
            }
            EventoOverlay::Tecla { vk, ctrl, .. } => match vk {
                // La escala de la interfaz (Ctrl + «+», «-», «0») sigue
                // siendo del bucle. Los demas atajos del chat no: Ctrl+V
                // pegaria en un chat que no se ve.
                _ if ctrl && es_de_la_escala(vk) => return None,
                _ if ctrl => Pedido::Nada,
                // ↑↓ cambian de proyecto en la lista, que es de donde se elige.
                VK_ABAJO => Pedido::Vecino(1),
                VK_ARRIBA => Pedido::Vecino(-1),
                VK_AVPAG => {
                    let alto = self.alto_hueco.get();
                    self.bajar(alto * 9 / 10)
                }
                VK_REPAG => {
                    let alto = self.alto_hueco.get();
                    self.bajar(-alto * 9 / 10)
                }
                VK_INICIO => self.bajar(i32::MIN / 2),
                VK_FIN => self.bajar(i32::MAX / 2),
                VK_IZQUIERDA => {
                    self.mover_foco(-1);
                    Pedido::Pintar
                }
                VK_DERECHA => {
                    self.mover_foco(1);
                    Pedido::Pintar
                }
                VK_ENTRAR => match self.actual.clone() {
                    Some(id) => {
                        let indice = self.tarjeta(&id).en_foco;
                        Pedido::AbrirHoja {
                            proyecto: id,
                            indice,
                        }
                    }
                    None => Pedido::Nada,
                },
                // Esc cierra, como en el chat.
                VK_ESCAPE => Pedido::Cerrar,
                _ => Pedido::Nada,
            },
            // Aqui no se escribe nada: sin esto las letras irian al borrador
            // del chat que hay detras, sin verse.
            EventoOverlay::Caracter(_) => Pedido::Nada,
            _ => return None,
        })
    }

    /// Pide el nombre nuevo en la caja de texto de Windows, en su hilo; al
    /// escribirlo, `latido` lo entrega como `Pedido::Renombrar`.
    fn pedir_nombre(&self, id: &str, actual: &str, textos: &Catalogo) {
        let caja = pixpin_shell::caja_de_texto::abrir(pixpin_shell::caja_de_texto::Pedido {
            titulo: textos.t("proyectos-renombrar-titulo"),
            texto: actual.to_string(),
            guardar: textos.t("proyectos-guardar"),
            cancelar: textos.t("chat-cancelar-caja"),
        });
        let destino = Arc::clone(&self.nombre_escrito);
        let id = id.to_string();
        let lanzado = std::thread::Builder::new()
            .name("proyectos-nombre".into())
            .spawn(move || {
                let Ok(Some(nombre)) = caja.recv() else {
                    return;
                };
                if let Ok(mut g) = destino.lock() {
                    *g = Some((id, nombre));
                }
                let h = super::ABIERTA.load(std::sync::atomic::Ordering::SeqCst);
                if h != 0 {
                    pixpin_shell::overlay::despertar(h);
                }
            });
        if let Err(e) = lanzado {
            tracing::warn!(?e, "no se pudo esperar el nombre del proyecto");
        }
    }
}

/// Todo lo del bucle del chat que un pedido puede necesitar tocar.
pub(super) struct Bucle<'a> {
    pub ubicacion: &'a Ubicacion,
    pub textos: &'a Catalogo,
    pub idioma: pixpin_store::Idioma,
    pub lienzo: OpcionesLienzo,
    pub identidad: &'a str,
    pub ventana: &'a VentanaOverlay,
    pub fichas: &'a [Ficha],
    /// La lista tal como se ve (indices de `fichas`, ya filtrada por el
    /// buscador): de ahi sale el proyecto de al lado para ↑↓.
    pub orden: &'a [usize],
    pub abierto: &'a mut Option<Abierto>,
    pub elegida: &'a mut Option<usize>,
    pub borradores: &'a mut HashMap<String, String>,
    pub aviso: &'a mut Option<(String, Instant)>,
    pub cerrar: &'a mut bool,
}

impl Bucle<'_> {
    fn avisar(&mut self, t: String) {
        *self.aviso = Some((t, Instant::now()));
    }

    fn ficha(&self, id: &str) -> Option<&Ficha> {
        self.fichas.iter().find(|f| f.id == id)
    }

    /// Deja `nuevo` como proyecto abierto del chat, guardando antes lo que el
    /// que habia tuviera a medias (lo mismo que hace `ir_a`).
    fn poner_abierto(&mut self, nuevo: Abierto) {
        if self
            .abierto
            .as_ref()
            .is_some_and(|a| a.ficha.id != nuevo.ficha.id)
        {
            if let Some(a) = self.abierto.as_mut() {
                super::cerrar_panel(self.ubicacion, a);
                super::apagar_lienzo(self.ubicacion, a);
            }
            if let Some(a) = self.abierto.take() {
                self.borradores.insert(a.ficha.id.clone(), a.borrador);
            }
        }
        *self.elegida = self.fichas.iter().position(|f| f.id == nuevo.ficha.id);
        *self.abierto = Some(nuevo);
    }

    /// El proyecto `id` abierto como en el chat: el que ya lo esta, o uno
    /// recien leido (que solo se queda si luego hace falta).
    fn tomar_abierto(&mut self, id: &str) -> Option<Abierto> {
        if self.abierto.as_ref().is_some_and(|a| a.ficha.id == id) {
            return self.abierto.take();
        }
        let f = self.ficha(id)?.clone();
        let mut a = super::abrir_proyecto(self.ubicacion, &f);
        a.borrador = self.borradores.remove(id).unwrap_or_default();
        Some(a)
    }

    /// Suelta el proyecto elegido y deja la lista delante, lo mismo que el
    /// volver del chat: lo que estuviera a medias se guarda antes.
    fn cerrar_abierto(&mut self) {
        if let Some(a) = self.abierto.as_mut() {
            super::cerrar_panel(self.ubicacion, a);
            super::apagar_lienzo(self.ubicacion, a);
        }
        if let Some(a) = self.abierto.take() {
            self.borradores.insert(a.ficha.id.clone(), a.borrador);
        }
        *self.elegida = None;
    }

    /// Devuelve al sitio un `Abierto` tomado con `tomar_abierto`: si era el
    /// del chat vuelve a serlo; si no, su borrador se guarda y se suelta.
    fn devolver_abierto(&mut self, a: Abierto) {
        if self.abierto.is_none()
            && self
                .elegida
                .and_then(|i| self.fichas.get(i))
                .is_some_and(|f| f.id == a.ficha.id)
        {
            *self.abierto = Some(a);
        } else {
            self.borradores.insert(a.ficha.id.clone(), a.borrador);
        }
    }
}

/// Soltar ficheros encima de la tarjeta los lleva al chat de su proyecto,
/// que es donde se pregunta antes de meterlos. Es cosa de Windows (en el
/// movil no se suelta nada encima de una pantalla).
pub(super) fn abrir_el_que_se_ve(v: &mut VistaProyectos, b: &mut Bucle) {
    let Some(id) = v.actual_id() else {
        return;
    };
    if let Some(a) = b.tomar_abierto(&id) {
        b.poner_abierto(a);
    }
    v.ver_chat();
}

/// Hace lo que la pantalla pidio. Devuelve si hay que pintar.
pub(super) fn cumplir(pedido: Pedido, v: &mut VistaProyectos, b: &mut Bucle) -> bool {
    match pedido {
        Pedido::Nada => false,
        Pedido::Pintar => true,
        Pedido::Cerrar => {
            *b.cerrar = true;
            false
        }
        Pedido::AlChat { proyecto, codigo } => {
            if let Ok(mut g) = super::IR_A.lock() {
                *g = Some((proyecto, codigo));
            }
            v.ver_chat();
            true
        }
        Pedido::Volver => {
            b.cerrar_abierto();
            v.seguir(None);
            true
        }
        Pedido::Vecino(paso) => {
            let actual = v.actual_id();
            let Some(i) = vecino(b.orden, b.fichas, actual.as_deref(), paso) else {
                return false;
            };
            let f = b.fichas[i].clone();
            if actual.as_deref() == Some(f.id.as_str()) {
                return false;
            }
            // Como el clic en la fila: un proyecto en un disco que no esta
            // se avisa, en vez de abrirlo vacio.
            if let Err(e) = pixpin_proyecto::ubicacion::preparar(b.ubicacion.raiz(), &f) {
                b.avisar(super::donde_vive::texto_de_error(b.textos, &e));
                return true;
            }
            if let Some(a) = b.tomar_abierto(&f.id) {
                b.poner_abierto(a);
            }
            v.seguir(Some(&f.id));
            true
        }
        Pedido::Renombrar { proyecto, nombre } => {
            renombrar(b, &proyecto, &nombre);
            v.olvidar(&proyecto);
            true
        }
        Pedido::Menu(id) => menu_del_proyecto(v, b, &id),
        Pedido::PedirNombre(id) => {
            // «Mensajes guardados» no se renombra: se llama siempre asi.
            if let Some(f) = b.ficha(&id).filter(|f| !f.es_guardados()) {
                v.pedir_nombre(&id, &f.nombre, b.textos);
            }
            false
        }
        Pedido::AbrirHoja { proyecto, indice } => abrir_hoja(v, b, &proyecto, indice),
        Pedido::Accion(a, id) => accion(v, b, a, &id),
    }
}

/// Lo que hace cada boton de la barra de cristal (`BarraDeAcciones`).
fn accion(v: &mut VistaProyectos, b: &mut Bucle, a: AccionBarra, id: &str) -> bool {
    match a {
        AccionBarra::Chat => cumplir(
            Pedido::AlChat {
                proyecto: id.to_string(),
                codigo: None,
            },
            v,
            b,
        ),
        // Una hoja de lienzo en blanco, que entra en las hojas del proyecto.
        // Como en el movil, no se abre sola: aparece al final de la tira.
        AccionBarra::Hoja => {
            let Some(mut a) = b.tomar_abierto(id) else {
                return false;
            };
            let hecho = super::crear_lienzo(b.ubicacion, &mut a, b.identidad);
            match hecho {
                Ok(()) => {
                    let i = a.mensajes.len() - 1;
                    let _ = super::unir_al_proyecto(b.ubicacion, &mut a, i, b.textos);
                    let mut args = fluent_bundle::FluentArgs::new();
                    args.set("nombre", a.ficha.nombre.clone());
                    b.avisar(b.textos.t_args("proyectos-hoja-hecha", &args));
                }
                Err(e) => {
                    tracing::warn!(?e, "no se pudo crear la hoja");
                    b.avisar(b.textos.t("chat-no-se-pudo"));
                }
            }
            b.devolver_abierto(a);
            v.olvidar(id);
            true
        }
        // La nota nace vacia y abre el editor: lo que se escriba vuelve a la
        // hoja al guardar (`notas_md`, H12).
        AccionBarra::Nota => {
            crate::notas_md::abrir(
                b.idioma,
                b.ubicacion.clone(),
                crate::notas_md::Destino::Nueva {
                    proyecto: id.to_string(),
                },
            );
            v.olvidar(id);
            false
        }
        // Una tabla con formulas. En el PC la hoja de calculo vive dentro del
        // chat, asi que se abre alli, ya puesta.
        AccionBarra::Tabla => {
            let Some(mut a) = b.tomar_abierto(id) else {
                return false;
            };
            match super::crear_tabla(b.ubicacion, &mut a, b.identidad, b.textos) {
                Ok(()) => {
                    let i = a.mensajes.len() - 1;
                    super::abrir_hoja(&mut a, i);
                    b.poner_abierto(a);
                    v.ver_chat();
                }
                Err(e) => {
                    tracing::warn!(?e, "no se pudo crear la tabla");
                    b.avisar(b.textos.t("chat-no-se-pudo"));
                    b.devolver_abierto(a);
                }
            }
            true
        }
    }
}

/// Abre una hoja como la abre `abrirHoja` del movil y el chat de aqui: el
/// lienzo para un dibujo o una pagina, el editor para una nota, la foto en
/// el lienzo; lo demas, por el chat.
fn abrir_hoja(v: &mut VistaProyectos, b: &mut Bucle, id: &str, indice: usize) -> bool {
    let Some(d) = v.datos(id) else {
        return false;
    };
    let Some(h) = d.hojas.get(indice) else {
        return false;
    };
    let m = h.mensaje.clone();
    let codigo = m.codigo_unico();
    let es_dibujo = matches!(m.clase, Some(Clase::Dibujo) | Some(Clase::Pagina))
        && (m.referencia.as_deref().is_some_and(|r| !r.is_empty())
            || crate::pdf_en_chat::es_pagina_sin_dibujo(&m));
    if m.clase == Some(Clase::Nota) {
        crate::notas_md::abrir(
            b.idioma,
            b.ubicacion.clone(),
            crate::notas_md::Destino::Mensaje {
                proyecto: id.to_string(),
                codigo,
            },
        );
        v.olvidar(id);
        return false;
    }
    if es_dibujo {
        let mensajes: Vec<_> = d.hojas.iter().map(|h| h.mensaje.clone()).collect();
        // Modal: vuelve al cerrar el lienzo, con lo dibujado ya guardado.
        super::abrir_hojas(b.ubicacion.raiz(), id, &mensajes, indice, b.lienzo);
        v.olvidar(id);
        if let Some(a) = b.abierto.as_mut().filter(|a| a.ficha.id == id) {
            super::releer_lo_abierto(b.ubicacion, a);
        }
        return true;
    }
    // Una foto, en SU lienzo como al tocar su burbuja (K11): la foto
    // bloqueada debajo y lo anotado encima editable (una pantalla anotada,
    // una foto anotada en el movil). Abrirla plana con `abrir_foto_en_lienzo`
    // ensenaba la captura sin la tinta guardada en su lienzo.
    if m.clase == Some(Clase::Imagen) {
        let mut mensajes: Vec<_> = d.hojas.iter().map(|h| h.mensaje.clone()).collect();
        if crate::foto_anotada::abrir_en_su_lienzo(
            b.ubicacion.raiz(),
            id,
            &mut mensajes,
            indice,
            b.lienzo,
        ) {
            v.olvidar(id);
            if let Some(a) = b.abierto.as_mut().filter(|a| a.ficha.id == id) {
                super::releer_lo_abierto(b.ubicacion, a);
            }
            return true;
        }
    }
    if m.clase == Some(Clase::Imagen)
        && let Some(ruta) = super::ruta_del_mensaje(b.ubicacion.raiz(), id, &m)
    {
        super::abrir_foto_en_lienzo(&ruta, b.lienzo);
        v.olvidar(id);
        return true;
    }
    // Lo demas (un PDF, un documento) se abre como al tocar su burbuja.
    let Some(a) = b.tomar_abierto(id) else {
        return false;
    };
    let Some(i) = super::indice_de_codigo(&a.mensajes, &codigo) else {
        b.devolver_abierto(a);
        return false;
    };
    let mut elegida = b.fichas.iter().position(|f| f.id == id);
    let mut tmp = Some(a);
    super::tocar_la_burbuja(
        i,
        b.ubicacion,
        &mut tmp,
        b.fichas,
        &mut elegida,
        b.borradores,
        b.textos,
        b.lienzo,
        b.idioma,
    );
    let Some(a) = tmp else {
        return true;
    };
    // Si se abrio dentro del chat (una hoja de calculo, una mini-app, un
    // libro de Excel), el chat pasa delante con ella.
    if a.hoja.is_some() || a.mini.is_some() || a.biblioteca.is_some() {
        b.poner_abierto(a);
        v.ver_chat();
    } else {
        b.devolver_abierto(a);
    }
    true
}

/// Los tres puntos de la tarjeta: lo de una vez, detras de un toque, en el
/// orden del movil (cambiar el nombre, archivar, compartir, copias de
/// seguridad y borrar). Sus «Copias de seguridad» son aqui la pantalla de
/// copias de Sincronizar, que es donde se recupera lo borrado.
fn menu_del_proyecto(v: &mut VistaProyectos, b: &mut Bucle, id: &str) -> bool {
    const RENOMBRAR: u32 = 1;
    const ARCHIVAR: u32 = 2;
    const COMPARTIR: u32 = 3;
    const PAPELERA: u32 = 4;
    const BORRAR: u32 = 5;
    const FUSIONAR: u32 = 6;
    const PONER_LOGO: u32 = 7;
    const QUITAR_LOGO: u32 = 8;
    let Some(f) = b.ficha(id).cloned() else {
        return false;
    };
    let archivado = v.resumen(id).is_some_and(|r| r.archivado)
        || f.resto
            .get(cargar::MARCA_ARCHIVADO)
            .and_then(|x| x.as_bool())
            == Some(true);
    // Fusionar va delante y solo con dos o mas paginas marcadas, como la
    // `CajaDeAcciones` del movil, que solo lo ofrece cuando tiene sentido.
    let fusion = v.peticion_de_fusion(b.ubicacion.raiz(), id);
    let mut entradas: Vec<(u32, String)> = Vec::new();
    if fusion.is_some() {
        entradas.push((FUSIONAR, b.textos.t("fusionar-paginas")));
    }
    if !f.es_guardados() {
        entradas.push((RENOMBRAR, b.textos.t("proyectos-renombrar")));
    }
    // El logo (solo del PC: el movil no tiene), junto al nombre, que es lo
    // otro que se ve del proyecto en la lista.
    entradas.push((PONER_LOGO, b.textos.t("proyecto-logo-poner")));
    if super::logo::tiene(b.ubicacion.raiz(), id) {
        entradas.push((QUITAR_LOGO, b.textos.t("proyecto-logo-quitar")));
    }
    entradas.extend([
        (
            ARCHIVAR,
            b.textos.t(if archivado {
                "proyectos-desarchivar"
            } else {
                "proyectos-archivar"
            }),
        ),
        (COMPARTIR, b.textos.t("proyectos-compartir")),
        (PAPELERA, b.textos.t("proyectos-copias")),
        (BORRAR, b.textos.t("proyectos-borrar")),
    ]);
    match pixpin_shell::menu_llano(b.ventana.handle(), &entradas) {
        Some(FUSIONAR) => {
            // En su hilo; el chat recoge la hoja nueva y dice como acabo.
            if fusion.is_some_and(|p| crate::fusionar_paginas::empezar(b.ubicacion.raiz(), p)) {
                v.con_tarjeta(id, |t| t.marcadas.clear());
            }
            true
        }
        Some(RENOMBRAR) => {
            v.pedir_nombre(id, &f.nombre, b.textos);
            false
        }
        Some(c @ (PONER_LOGO | QUITAR_LOGO)) => {
            let raiz = b.ubicacion.raiz();
            let hecho = if c == PONER_LOGO {
                super::logo::elegir_y_poner(b.ventana.handle(), raiz, id).map(|_| ())
            } else {
                super::logo::quitar(raiz, id).map_err(anyhow::Error::from)
            };
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo cambiar el logo del proyecto");
                b.avisar(b.textos.t("proyecto-logo-error"));
            }
            true
        }
        Some(ARCHIVAR) => {
            archivar(b.ubicacion, &f, !archivado);
            if let Some(r) = v.resumenes.get_mut(id) {
                r.archivado = !archivado;
            }
            // Los resumenes se vuelven a pedir: el orden cambio.
            v.firma = 0;
            true
        }
        Some(COMPARTIR) => {
            if let Some(a) = b.abierto.as_mut().filter(|a| a.ficha.id == id) {
                super::apagar_lienzo(b.ubicacion, a);
            }
            crate::compartir::ventana::abrir(
                b.idioma,
                b.ubicacion.clone(),
                crate::compartir::Cosa::Proyectos {
                    raiz: b.ubicacion.raiz().to_path_buf(),
                    ids: vec![id.to_string()],
                },
            );
            false
        }
        Some(PAPELERA) => {
            crate::sincronizar::abrir_papelera(b.idioma, b.ubicacion.clone());
            false
        }
        Some(BORRAR) => {
            let titulo = {
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("nombre", f.nombre.clone());
                b.textos.t_args("proyectos-borrar-titulo", &args)
            };
            if !pixpin_shell::confirmar_destructivo(
                b.ventana.handle(),
                &titulo,
                &b.textos.t("proyectos-borrar-aviso"),
            ) {
                return false;
            }
            if let Some(a) = b.abierto.as_mut().filter(|a| a.ficha.id == id) {
                super::cerrar_panel(b.ubicacion, a);
                super::apagar_lienzo(b.ubicacion, a);
            }
            if b.abierto.as_ref().is_some_and(|a| a.ficha.id == id) {
                *b.abierto = None;
                *b.elegida = None;
            }
            let cuando = pixpin_shell::entorno::ahora_utc_ms();
            match pixpin_proyecto::almacen::borrar_proyectos(
                b.ubicacion.raiz(),
                &[id.to_string()],
                cuando,
            ) {
                Ok((quitados, sin_mover)) => {
                    tracing::info!(quitados, "proyecto a la papelera desde Proyectos");
                    for ruta in sin_mover {
                        tracing::warn!(ruta = %ruta.display(), "carpeta que no se pudo llevar a la papelera");
                    }
                }
                Err(e) => tracing::error!(?e, "no se pudo borrar el proyecto"),
            }
            b.borradores.remove(id);
            v.olvidar(id);
            v.seguir(None);
            true
        }
        _ => false,
    }
}

/// Cambia el nombre (`Proyectos.renombrado`): recortado, en blanco no cambia
/// nada, y cuenta como tocarlo (sube arriba). Va al indice, que es lo que
/// ensena la lista, y al `proyecto.json` si lo tiene, que es lo que viaja.
fn renombrar(b: &mut Bucle, id: &str, nombre: &str) {
    if b.ficha(id).is_some_and(|f| f.es_guardados()) {
        return;
    }
    let Some(limpio) = nombre_limpio(nombre) else {
        return;
    };
    let limpio = limpio.as_str();
    let raiz = b.ubicacion.raiz();
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let mut indice = pixpin_proyecto::almacen::Indice::leer(raiz);
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == id) {
        f.nombre = limpio.to_string();
        f.tocado = ahora;
        if let Err(e) = indice.guardar(raiz) {
            tracing::warn!(?e, "no se pudo guardar el nombre del proyecto");
        }
    }
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, id);
    if carpeta.join("proyecto.json").is_file() {
        let mut p = super::leer_proyecto_json(&carpeta);
        p.nombre = limpio.to_string();
        p.tocado = ahora;
        if let Err(e) = super::guardar_proyecto_json(&carpeta, &p) {
            tracing::warn!(?e, "no se pudo poner el nombre en proyecto.json");
        }
    }
    if let Some(a) = b.abierto.as_mut().filter(|a| a.ficha.id == id) {
        a.ficha.nombre = limpio.to_string();
    }
}

/// Lo que queda de un nombre escrito: su primera linea recortada, o nada si
/// esta en blanco (`Proyectos.renombrado` no deja un proyecto sin nombre).
pub(super) fn nombre_limpio(escrito: &str) -> Option<String> {
    let limpio = escrito.lines().next().unwrap_or("").trim();
    (!limpio.is_empty()).then(|| limpio.to_string())
}

/// Archiva o desarchiva (`Proyectos.archivado`, que tambien lo da por
/// tocado): en su `proyecto.json` si lo tiene (viaja al movil, que lo lee
/// igual), y si no con una marca en su ficha del indice.
fn archivar(ubicacion: &Ubicacion, f: &Ficha, archivar: bool) {
    let raiz = ubicacion.raiz();
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &f.id);
    let con_json = carpeta.join("proyecto.json").is_file();
    if con_json {
        let mut p = super::leer_proyecto_json(&carpeta);
        p.archivado = archivar;
        p.tocado = ahora;
        if let Err(e) = super::guardar_proyecto_json(&carpeta, &p) {
            tracing::warn!(?e, "no se pudo archivar el proyecto");
        }
    }
    let mut indice = pixpin_proyecto::almacen::Indice::leer(raiz);
    if let Some(g) = indice.proyectos.iter_mut().find(|g| g.id == f.id) {
        g.tocado = ahora;
        if archivar && !con_json {
            g.resto.insert(
                cargar::MARCA_ARCHIVADO.into(),
                serde_json::Value::Bool(true),
            );
        } else {
            g.resto.remove(cargar::MARCA_ARCHIVADO);
        }
        if let Err(e) = indice.guardar(raiz) {
            tracing::warn!(?e, "no se pudo archivar el proyecto");
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn ficha(id: &str, nombre: &str, tocado: i64) -> Ficha {
        Ficha {
            id: id.into(),
            nombre: nombre.into(),
            tocado,
            ..Default::default()
        }
    }

    fn hoja(id: &str, padre: Option<&str>) -> cargar::Hoja {
        cargar::Hoja {
            id: id.into(),
            padre: padre.map(String::from),
            nombre: String::new(),
            mensaje: Default::default(),
            vista: None,
            por_pedir: false,
            anotada: false,
        }
    }

    fn tecla(vk: u32, ctrl: bool) -> EventoOverlay {
        EventoOverlay::Tecla {
            vk,
            shift: false,
            ctrl,
            alt: false,
        }
    }

    fn vista() -> VistaProyectos {
        let u = Ubicacion::Portable {
            raiz: std::env::temp_dir().join("pixpin-proyectos-sin-nada"),
        };
        VistaProyectos::nueva(&u)
    }

    #[test]
    fn sin_nada_guardado_se_abre_en_proyectos_y_con_chat_en_el_chat() {
        assert!(abre_en_proyectos(None));
        assert!(abre_en_proyectos(Some("proyectos")));
        assert!(!abre_en_proyectos(Some("chat\n")));
        // Lo que no se entiende no deja al usuario sin lo que pidio.
        assert!(abre_en_proyectos(Some("")));
    }

    #[test]
    fn el_interruptor_se_recuerda_y_el_boton_chat_no_lo_cambia() {
        let raiz =
            std::env::temp_dir().join(format!("pixpin-proyectos-vista-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        let u = Ubicacion::Portable { raiz: raiz.clone() };
        let mut v = VistaProyectos::nueva(&u);
        assert!(v.activa(), "de fabrica, Proyectos");
        v.poner(&u, false);
        assert!(!VistaProyectos::nueva(&u).activa(), "se apunta el chat");
        v.poner(&u, true);
        // Ir al chat de un proyecto es ir y volver: lo apuntado sigue siendo Proyectos.
        v.ver_chat();
        assert!(!v.activa());
        assert!(VistaProyectos::nueva(&u).activa());
    }

    #[test]
    fn la_tarjeta_es_la_del_proyecto_que_el_chat_tiene_abierto_y_cambiar_de_modo_no_lo_suelta() {
        let mut v = vista();
        assert_eq!(v.actual_id(), None, "sin proyecto elegido no hay tarjeta");
        v.seguir(Some("obra"));
        assert_eq!(v.actual_id().as_deref(), Some("obra"));
        // El interruptor cambia lo que ocupa el panel, no el proyecto.
        v.ver_chat();
        v.ver_proyectos();
        assert_eq!(v.actual_id().as_deref(), Some("obra"));
        v.seguir(None);
        assert_eq!(v.actual_id(), None);
    }

    #[test]
    fn otra_tarjeta_sale_desde_arriba() {
        let mut v = vista();
        v.seguir(Some("obra"));
        v.tope.set(300);
        v.evento(EventoOverlay::Rueda(-120));
        assert!(v.desplazamiento() > 0);
        // Seguir en el mismo proyecto no la mueve.
        v.seguir(Some("obra"));
        assert!(v.desplazamiento() > 0);
        v.seguir(Some("oficina"));
        assert_eq!(v.desplazamiento(), 0);
    }

    #[test]
    fn flechas_arriba_y_abajo_piden_el_proyecto_de_al_lado_en_la_lista() {
        let mut v = vista();
        assert_eq!(v.evento(tecla(VK_ABAJO, false)), Some(Pedido::Vecino(1)));
        assert_eq!(v.evento(tecla(VK_ARRIBA, false)), Some(Pedido::Vecino(-1)));
    }

    #[test]
    fn el_de_al_lado_sigue_la_lista_que_se_ve_y_no_se_sale_de_ella() {
        let fichas = vec![ficha("a", "A", 3), ficha("b", "B", 2), ficha("c", "C", 1)];
        // La lista filtrada por el buscador: solo «c» y «a», en ese orden.
        let orden = [2, 0];
        assert_eq!(vecino(&orden, &fichas, Some("c"), 1), Some(0));
        assert_eq!(
            vecino(&orden, &fichas, Some("a"), 1),
            Some(0),
            "en el ultimo se queda"
        );
        assert_eq!(
            vecino(&orden, &fichas, Some("c"), -1),
            Some(2),
            "en el primero se queda"
        );
        // Sin ninguno elegido (o uno que el buscador esconde), abajo al primero y arriba al ultimo.
        assert_eq!(vecino(&orden, &fichas, None, 1), Some(2));
        assert_eq!(vecino(&orden, &fichas, Some("b"), -1), Some(0));
        assert_eq!(
            vecino(&[], &fichas, Some("a"), 1),
            None,
            "lista vacia, nada"
        );
    }

    #[test]
    fn la_rueda_recorre_la_tarjeta_si_no_cabe_y_si_cabe_no_hace_nada() {
        let mut v = vista();
        v.seguir(Some("a"));
        // Cabe entera: nada que bajar, y no se repinta por nada.
        assert_eq!(v.evento(EventoOverlay::Rueda(-120)), Some(Pedido::Nada));
        assert_eq!(v.desplazamiento(), 0);
        v.tope.set(200);
        assert_eq!(v.evento(EventoOverlay::Rueda(-120)), Some(Pedido::Pintar));
        assert_eq!(v.desplazamiento(), pixpin_ui::proyectos::ALTO_FILA as i32);
        v.evento(EventoOverlay::Rueda(-600));
        assert_eq!(v.desplazamiento(), 200, "no pasa del tope");
        v.evento(tecla(VK_INICIO, false));
        assert_eq!(v.desplazamiento(), 0);
        v.evento(tecla(VK_FIN, false));
        assert_eq!(v.desplazamiento(), 200);
    }

    #[test]
    fn sobre_la_tira_la_rueda_la_corre_de_lado_y_no_baja_la_tarjeta() {
        let mut v = vista();
        v.seguir(Some("a"));
        v.tope.set(200);
        let r = Rect {
            x: 0,
            y: 0,
            ancho: 300,
            alto: 128,
        };
        v.zonas.borrow_mut().push((r, Zona::Tira("a".into())));
        v.mover(Punto { x: 10, y: 10 });
        assert_eq!(v.evento(EventoOverlay::Rueda(-120)), Some(Pedido::Pintar));
        assert!(v.tarjeta("a").corrida > 0);
        assert_eq!(v.desplazamiento(), 0);
    }

    #[test]
    fn las_letras_no_hacen_nada_y_escape_cierra_como_en_el_chat() {
        let mut v = vista();
        v.seguir(Some("a"));
        assert_eq!(v.evento(EventoOverlay::Caracter('o')), Some(Pedido::Nada));
        assert_eq!(v.evento(tecla(VK_ESCAPE, false)), Some(Pedido::Cerrar));
    }

    #[test]
    fn ctrl_con_otra_tecla_sigue_su_camino() {
        let mut v = vista();
        v.seguir(Some("a"));
        assert_eq!(v.evento(tecla(super::super::VK_MAS, true)), None);
        assert_eq!(
            v.evento(tecla(super::super::VK_U, true)),
            Some(Pedido::Nada),
            "Ctrl+U ya no abre nada"
        );
        assert_eq!(
            v.evento(EventoOverlay::Pintar),
            None,
            "lo que no es suyo sigue"
        );
        // Ctrl+V no llega al chat de detras, que no se ve.
        assert_eq!(
            v.evento(tecla(super::super::VK_V, true)),
            Some(Pedido::Nada)
        );
    }

    #[test]
    fn el_circulo_de_volver_devuelve_a_la_lista() {
        let mut v = vista();
        v.seguir(Some("a"));
        let r = Rect {
            x: 400,
            y: 30,
            ancho: 46,
            alto: 46,
        };
        v.zonas.borrow_mut().push((r, Zona::Volver));
        assert_eq!(v.pulsar(Punto { x: 410, y: 40 }), Pedido::Volver);
        assert_eq!(
            v.pulsar(Punto { x: 10, y: 10 }),
            Pedido::Nada,
            "fuera de todo, nada"
        );
    }

    #[test]
    fn los_botones_de_la_barra_son_del_proyecto_de_la_tarjeta_y_sin_el_no_hacen_nada() {
        let mut v = vista();
        let r = Rect {
            x: 0,
            y: 0,
            ancho: 66,
            alto: 55,
        };
        v.zonas
            .borrow_mut()
            .push((r, Zona::Boton(AccionBarra::Chat)));
        let p = Punto { x: 5, y: 5 };
        assert_eq!(v.pulsar(p), Pedido::Nada);
        v.seguir(Some("obra"));
        assert_eq!(
            v.pulsar(p),
            Pedido::Accion(AccionBarra::Chat, "obra".into())
        );
    }

    #[test]
    fn un_clic_en_la_tira_sube_la_hoja_a_la_portada_y_dos_la_abren() {
        let mut v = vista();
        v.seguir(Some("a"));
        let caja = Rect {
            x: 10,
            y: 10,
            ancho: 76,
            alto: 104,
        };
        v.zonas.borrow_mut().push((caja, Zona::Hoja("a".into(), 4)));
        let dentro = Punto { x: 20, y: 20 };
        assert_eq!(v.pulsar(dentro), Pedido::Pintar);
        assert_eq!(v.tarjeta("a").en_foco, 4);
        assert_eq!(
            v.pulsar(dentro),
            Pedido::AbrirHoja {
                proyecto: "a".into(),
                indice: 4
            }
        );
    }

    #[test]
    fn en_la_rejilla_un_clic_no_hace_nada_y_el_doble_abre() {
        let mut v = vista();
        v.con_tarjeta("a", |t| t.rejilla = true);
        let caja = Rect {
            x: 0,
            y: 0,
            ancho: 50,
            alto: 50,
        };
        v.zonas.borrow_mut().push((caja, Zona::Hoja("a".into(), 2)));
        let p = Punto { x: 5, y: 5 };
        assert_eq!(v.pulsar(p), Pedido::Nada);
        assert_eq!(v.tarjeta("a").en_foco, 0, "no hay portada que cambiar");
        assert_eq!(
            v.pulsar(p),
            Pedido::AbrirHoja {
                proyecto: "a".into(),
                indice: 2
            }
        );
    }

    #[test]
    fn el_boton_derecho_sobre_el_nombre_lo_cambia_y_en_una_hoja_sin_leer_no_hace_nada() {
        let mut v = vista();
        let r = Rect {
            x: 0,
            y: 0,
            ancho: 100,
            alto: 40,
        };
        v.zonas.borrow_mut().push((r, Zona::Nombre("a".into())));
        v.zonas
            .borrow_mut()
            .push((Rect { y: 50, ..r }, Zona::Hoja("a".into(), 0)));
        assert_eq!(
            v.pulsar_derecho(Punto { x: 5, y: 5 }),
            Pedido::PedirNombre("a".into())
        );
        // Una hoja que aun no se ha leido no se puede marcar.
        assert_eq!(v.pulsar_derecho(Punto { x: 5, y: 55 }), Pedido::Nada);
        assert!(v.tarjeta("a").marcadas.is_empty());
    }

    #[test]
    fn marcar_dos_paginas_con_el_derecho_ofrece_fusionarlas_y_una_sola_no() {
        // E9 vive aqui: las paginas del PDF ya no salen en el chat.
        let raiz =
            std::env::temp_dir().join(format!("pixpin-proyectos-fusion-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let f = Ficha::nueva("Plano", 5, "PC01");
        std::fs::create_dir_all(pixpin_proyecto::almacen::carpeta(&raiz, &f.id)).unwrap();
        let imgs: Vec<_> = (0..3)
            .map(|_| pixpin_codec::imagen::ImagenRgba {
                ancho: 20,
                alto: 30,
                pixeles: [10, 90, 200, 255].repeat(600),
            })
            .collect();
        let doc = raiz.join("plano.pdf");
        std::fs::write(&doc, pixpin_pdf::union::de_imagenes(&imgs).unwrap()).unwrap();
        crate::pdf_en_chat::unir(&raiz, &f, &doc, "m-pdf", "plano", 1000, &|_, _| {}).unwrap();
        let u = Ubicacion::Portable { raiz: raiz.clone() };
        let mut v = VistaProyectos::nueva(&u);
        v.datos.insert(f.id.clone(), cargar::hojas_de(&u, &f));
        let celda = |y| Rect {
            x: 0,
            y,
            ancho: 40,
            alto: 40,
        };
        for n in 0..3 {
            v.zonas
                .borrow_mut()
                .push((celda(50 * n as i32), Zona::Hoja(f.id.clone(), n)));
        }
        let en = |n: i32| Punto {
            x: 5,
            y: 50 * n + 5,
        };

        assert_eq!(v.pulsar_derecho(en(0)), Pedido::Pintar);
        assert!(
            v.peticion_de_fusion(&raiz, &f.id).is_none(),
            "una sola pagina no se fusiona"
        );
        assert_eq!(v.pulsar_derecho(en(2)), Pedido::Pintar);
        let p = v
            .peticion_de_fusion(&raiz, &f.id)
            .expect("dos paginas marcadas");
        assert_eq!(p.paginas, vec![0, 2]);
        // Otra vez el derecho la desmarca.
        v.pulsar_derecho(en(2));
        assert_eq!(v.tarjeta(&f.id).marcadas.len(), 1);
        assert!(v.peticion_de_fusion(&raiz, &f.id).is_none());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn un_nombre_en_blanco_no_cambia_nada_y_se_queda_la_primera_linea() {
        assert_eq!(
            nombre_limpio("  Obra nueva \nsegunda"),
            Some("Obra nueva".into())
        );
        assert_eq!(nombre_limpio("   "), None);
        assert_eq!(nombre_limpio(""), None);
    }

    #[test]
    fn los_sublienzos_se_ven_solo_con_su_padre_desplegado() {
        let hojas = vec![
            hoja("p1", None),
            hoja("s1", Some("p1")),
            hoja("s2", Some("s1")),
            hoja("p2", None),
        ];
        let mut abiertos = BTreeSet::new();
        assert_eq!(hojas_visibles(&hojas, &abiertos), [0, 3]);
        abiertos.insert("p1".to_string());
        assert_eq!(
            hojas_visibles(&hojas, &abiertos),
            [0, 1, 3],
            "el nieto sigue plegado"
        );
        abiertos.insert("s1".to_string());
        assert_eq!(hojas_visibles(&hojas, &abiertos), [0, 1, 2, 3]);
        assert_eq!(hijos_de(&hojas).get("p1"), Some(&1));
        assert_eq!(hijos_de(&hojas).get("p2"), None);
    }

    #[test]
    fn dos_hojas_que_se_apuntan_una_a_otra_no_cuelgan_la_pantalla() {
        let hojas = vec![hoja("a", Some("b")), hoja("b", Some("a"))];
        let abiertos: BTreeSet<String> = ["a".to_string(), "b".to_string()].into();
        assert!(hojas_visibles(&hojas, &abiertos).is_empty());
    }

    #[test]
    fn la_etiqueta_de_la_hoja_dice_su_clase_y_las_paginas_no_llevan() {
        use pixpin_proyecto::cuaderno::Mensaje;
        let mut m = Mensaje {
            clase: Some(Clase::Nota),
            ..Default::default()
        };
        assert_eq!(de_que_es(&m), Some("MD"));
        m.clase = Some(Clase::Dibujo);
        assert_eq!(de_que_es(&m), Some("2D"));
        m.pagina = Some(3);
        assert_eq!(
            de_que_es(&m),
            None,
            "una pagina dibujada sigue siendo pagina"
        );
        m.pagina = None;
        m.clase = Some(Clase::Imagen);
        assert_eq!(de_que_es(&m), None);
    }

    #[test]
    fn el_interruptor_elige_la_mitad_que_se_pulsa() {
        let v = vista();
        assert_eq!(
            v.interruptor_en(Punto { x: 1, y: 1 }),
            None,
            "sin pintar no hay interruptor"
        );
        let i = pixpin_ui::proyectos::interruptor(
            Rect {
                x: 0,
                y: 0,
                ancho: 1000,
                alto: 24,
            },
            80,
            900,
            100,
        );
        v.interruptor.set(Some(i));
        let centro = |r: Rect| Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        };
        assert_eq!(v.interruptor_en(centro(i.chat)), Some(false));
        assert_eq!(v.interruptor_en(centro(i.proyectos)), Some(true));
        assert_eq!(v.interruptor_en(Punto { x: 5, y: 5 }), None);
    }
}
