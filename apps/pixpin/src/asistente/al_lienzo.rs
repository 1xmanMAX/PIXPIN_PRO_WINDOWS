//! **Lo que dibuja el asistente, al lienzo** (encargo 2).
//!
//! El turno corre en su hilo y el lienzo es de la ventana. Ademas, el
//! editor del lienzo es **modal sobre el hilo del chat**: mientras esta
//! abierto, el bucle del chat no da vueltas. Por eso el pedido no va por el
//! canal del chat sino por una cola de aqui, que vacia quien este vivo:
//!
//! - **Hay un lienzo abierto** ([`EditorAbierto`]): lo vacia el bucle del
//!   editor y lo dibujado cae **centrado en lo que se ve** ([`meter`]).
//! - **No hay**: lo vacia el chat, que lo mete sin abrirlo en el lienzo
//!   nombrado en la pregunta (`@lienzo:<nombre>`) o, si no, en el **lienzo
//!   propio del asistente** en esa conversacion (uno suyo, creado la primera
//!   vez); la respuesta lleva «Abrir lienzo». Como el editor encuadra lo
//!   dibujado al abrirse, lo nuevo se pone **a la derecha de lo que ya
//!   habia** ([`meter_al_lado`]) para no taparlo.
//!
//! Antes, sin lienzo abierto, iba al ultimo lienzo del chat, fuera de quien
//! fuera: el usuario no sabia donde buscarlo (13-oct).
//!
//! En los dos casos es `portapapeles::pegar`: **un solo paso de deshacer**,
//! y el lienzo se guarda por su camino de siempre (`guardar_hoja_dibujada`),
//! que es lo que mira la sincronizacion.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use pixpin_ia::edicion::{Anfitrion, Contexto, Edicion};
use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::Elemento;

/// Que se pide al lienzo (14-oct: ademas de dibujar, editar lo que hay y
/// resumirlo para el modelo; `pixpin_ia::edicion`).
#[derive(Debug, Clone, Default)]
pub enum Trabajo {
    /// Meter el `.excalidraw` de [`Pedido::documento`].
    #[default]
    Dibujar,
    /// Editar lo que ya hay, con lo que el turno sabe del lienzo y las
    /// imagenes adjuntas del chat (nombre y ruta) por si hay que pegar una.
    Editar {
        edicion: Box<Edicion>,
        ctx: Contexto,
        adjuntos: Vec<(String, std::path::PathBuf)>,
    },
    /// El resumen del lienzo destino, al empezar el turno.
    Resumir,
}

/// Lo que contesta el lienzo: el texto para el modelo, los ids que toco
/// (pasan a ser «lo ultimo») y, al resumir, el contexto del turno.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Respuesta {
    pub texto: String,
    pub ids: Vec<u64>,
    pub ctx: Option<Contexto>,
}

/// Un pedido al lienzo.
pub struct Pedido {
    /// La conversacion desde la que se pidio (adonde va si no hay lienzo
    /// abierto).
    pub proyecto: String,
    /// El lienzo de esa conversacion que se nombro con `@lienzo:` (su
    /// referencia). Sin el, el propio del asistente.
    pub destino: Option<String>,
    /// Un `.excalidraw` hecho por `pixpin_ia::lienzo::documento` (al
    /// dibujar).
    pub documento: String,
    pub trabajo: Trabajo,
    /// Desde donde mide el modelo en este turno (para listar lo dibujado
    /// con las coordenadas que el entiende).
    pub marco: Option<(f32, f32)>,
    responder: Sender<Result<Respuesta, String>>,
}

impl Pedido {
    pub fn nuevo(
        proyecto: String,
        destino: Option<String>,
        documento: String,
        responder: Sender<Result<Respuesta, String>>,
    ) -> Pedido {
        Pedido {
            proyecto,
            destino,
            documento,
            trabajo: Trabajo::Dibujar,
            marco: None,
            responder,
        }
    }

    /// Un pedido de editar o de resumir.
    pub fn de_trabajo(
        proyecto: String,
        destino: Option<String>,
        trabajo: Trabajo,
        responder: Sender<Result<Respuesta, String>>,
    ) -> Pedido {
        Pedido {
            trabajo,
            ..Pedido::nuevo(proyecto, destino, String::new(), responder)
        }
    }

    /// Contesta al turno (que esta esperando).
    pub fn responder(self, r: Result<String, String>) {
        self.responder_con(r.map(|texto| Respuesta {
            texto,
            ..Default::default()
        }));
    }

    pub fn responder_con(self, r: Result<Respuesta, String>) {
        let _ = self.responder.send(r);
    }
}

/// **Las imagenes que el asistente pega**: las de un almacen de imagenes del
/// lienzo (el del editor abierto, o uno de paso para una hoja cerrada), con
/// los pixeles de un adjunto del chat o del portapapeles.
pub struct ConImagenes<'a> {
    pub imagenes: &'a mut crate::imagenes_lienzo::ImagenesLienzo,
    pub adjuntos: &'a [(String, std::path::PathBuf)],
}

impl Anfitrion for ConImagenes<'_> {
    fn imagen_nueva(&mut self, fuente: &str) -> Result<Elemento, String> {
        let img = imagen_de(fuente, self.adjuntos)?;
        let (w, h) = (img.ancho as f32, img.alto as f32);
        let id = self.imagenes.guardar(img).ok_or("la imagen es demasiado grande para este equipo")?;
        Ok(crate::imagenes_lienzo::elemento_imagen(id, 0.0, 0.0, w, h))
    }
    fn tamano_de(&self, id_objeto: u64) -> Option<(u32, u32)> {
        self.imagenes.tamano(id_objeto)
    }
}

/// Los pixeles de lo que hay que pegar: «portapapeles», un adjunto por su
/// nombre, o (sin decir) la ultima imagen adjunta.
fn imagen_de(fuente: &str, adjuntos: &[(String, std::path::PathBuf)]) -> Result<pixpin_codec::ImagenRgba, String> {
    let f = pixpin_ia::acciones::normalizar(fuente.trim());
    if f.contains("portapapeles") || f.contains("clipboard") {
        return match pixpin_codec::portapapeles::leer() {
            Some(pixpin_codec::ContenidoPortapapeles::Imagen(img)) if img.ancho > 0 && img.alto > 0 => Ok(img),
            Some(pixpin_codec::ContenidoPortapapeles::Rutas(r)) => {
                crate::imagenes_lienzo::imagen_de_rutas(&r).ok_or_else(|| "lo copiado no es una imagen".to_string())
            }
            _ => Err("no hay ninguna imagen copiada en el portapapeles".into()),
        };
    }
    let imagenes: Vec<&(String, std::path::PathBuf)> = adjuntos
        .iter()
        .filter(|(_, r)| crate::imagenes_lienzo::imagen_de_rutas(std::slice::from_ref(r)).is_some() || es_imagen(r))
        .collect();
    let elegida = if f.is_empty() {
        imagenes.last().copied()
    } else {
        imagenes
            .iter()
            .rev()
            .find(|(n, _)| pixpin_ia::acciones::normalizar(n).contains(&f))
            .copied()
            .or(imagenes.last().copied())
    };
    let (_, ruta) = elegida.ok_or("no hay ninguna imagen adjunta en este chat")?;
    crate::imagenes_lienzo::imagen_de_rutas(std::slice::from_ref(ruta)).ok_or_else(|| "no se pudo leer la imagen".to_string())
}

fn es_imagen(r: &std::path::Path) -> bool {
    r.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ["png", "jpg", "jpeg", "bmp", "gif", "webp"].contains(&e.to_ascii_lowercase().as_str()))
}

/// **Hace un trabajo de editar o resumir sobre una escena** (la del editor
/// abierto o la de una hoja cerrada). `elegidos`: lo elegido en el editor;
/// `marco`: desde donde mide el modelo si es un resumen nuevo.
pub fn trabajar(
    escena: &mut Escena,
    trabajo: &Trabajo,
    elegidos: &[u64],
    marco: (f32, f32),
    imagenes: &mut crate::imagenes_lienzo::ImagenesLienzo,
) -> Result<Respuesta, String> {
    match trabajo {
        Trabajo::Resumir => {
            let ctx = Contexto {
                marco,
                elegidos: elegidos.to_vec(),
                ultimos: Vec::new(),
            };
            Ok(Respuesta {
                texto: pixpin_ia::edicion::resumen(escena, &ctx),
                ids: Vec::new(),
                ctx: Some(ctx),
            })
        }
        Trabajo::Editar { edicion, ctx, adjuntos } => {
            let mut ctx = ctx.clone();
            // Lo elegido es lo de ahora, no lo de cuando empezo el turno.
            ctx.elegidos = elegidos.to_vec();
            let mut an = ConImagenes { imagenes, adjuntos };
            let h = pixpin_ia::edicion::aplicar(escena, edicion, &ctx, &mut an)?;
            Ok(Respuesta {
                texto: h.texto,
                ids: h.ids,
                ctx: None,
            })
        }
        Trabajo::Dibujar => Err("eso no es un trabajo de editar".into()),
    }
}

static COLA: Mutex<Vec<Pedido>> = Mutex::new(Vec::new());
/// La ventana del editor abierto (0: ninguno).
static EDITOR: AtomicIsize = AtomicIsize::new(0);
/// La ventana del chat, para despertarla.
static CHAT: AtomicIsize = AtomicIsize::new(0);

/// El chat se apunta para que lo despierten.
pub fn avisar_al_chat(hwnd: isize) {
    CHAT.store(hwnd, Ordering::SeqCst);
}

/// Mientras vive, hay un lienzo abierto y es el quien recibe lo dibujado.
pub struct EditorAbierto(isize);

impl EditorAbierto {
    pub fn nuevo(hwnd: isize) -> EditorAbierto {
        EDITOR.store(hwnd, Ordering::SeqCst);
        EditorAbierto(hwnd)
    }
}

impl Drop for EditorAbierto {
    fn drop(&mut self) {
        let _ = EDITOR.compare_exchange(self.0, 0, Ordering::SeqCst, Ordering::SeqCst);
        // Lo que quedara en la cola es ya del chat: que se entere.
        if hay_pedidos() {
            despertar(CHAT.load(Ordering::SeqCst));
        }
    }
}

/// Si alguien vaciaria la cola ahora (un editor abierto o el chat): sin
/// nadie, preguntar por el resumen del lienzo solo haria esperar.
pub fn hay_quien_conteste() -> bool {
    EDITOR.load(Ordering::SeqCst) != 0 || CHAT.load(Ordering::SeqCst) != 0
}

pub fn hay_editor() -> bool {
    EDITOR.load(Ordering::SeqCst) != 0
}

fn despertar(hwnd: isize) {
    if hwnd != 0 {
        pixpin_shell::overlay::despertar(hwnd);
    }
}

/// Deja el pedido en la cola y despierta a quien lo va a atender.
pub fn pedir(p: Pedido) {
    if let Ok(mut c) = COLA.lock() {
        c.push(p);
    }
    let editor = EDITOR.load(Ordering::SeqCst);
    despertar(if editor != 0 { editor } else { CHAT.load(Ordering::SeqCst) });
}

pub fn hay_pedidos() -> bool {
    COLA.lock().is_ok_and(|c| !c.is_empty())
}

/// Los pedidos que haya, para atenderlos.
pub fn tomar() -> Vec<Pedido> {
    COLA.lock().map(|mut c| std::mem::take(&mut *c)).unwrap_or_default()
}

/// Los elementos de un documento del asistente.
pub fn elementos_de(documento: &str) -> Result<Vec<Elemento>, String> {
    let lienzo = pixpin_motor2d::excalidraw::leer(documento).map_err(|e| format!("el lienzo no lo entiende: {e}"))?;
    let els = lienzo.elementos();
    if els.is_empty() {
        return Err("no hay nada que dibujar".into());
    }
    Ok(els)
}

/// La caja que ocupan unos elementos: (x0, y0, x1, y1).
pub fn caja(els: &[Elemento]) -> Option<(f32, f32, f32, f32)> {
    els.iter()
        .map(Elemento::caja)
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
        .filter(|c| [c.0, c.1, c.2, c.3].iter().all(|v| v.is_finite()))
}

/// **Mete lo dibujado con su centro en `centro`** (el de la vista), en un
/// solo paso de deshacer. Devuelve cuantos elementos entraron.
pub fn meter(escena: &mut Escena, documento: &str, centro: (f32, f32)) -> Result<usize, String> {
    meter_ids(escena, documento, centro).map(|v| v.len())
}

/// Lo mismo, devolviendo los ids de lo que entro (para listarlo al modelo).
pub fn meter_ids(escena: &mut Escena, documento: &str, centro: (f32, f32)) -> Result<Vec<u64>, String> {
    let els = elementos_de(documento)?;
    let c = caja(&els).ok_or("las figuras no tienen medida")?;
    let dx = centro.0 - (c.0 + c.2) / 2.0;
    let dy = centro.1 - (c.1 + c.3) / 2.0;
    Ok(pixpin_motor2d::portapapeles::pegar(escena, &els, dx, dy))
}

/// Hueco entre lo que habia y lo nuevo.
const HUECO: f32 = 80.0;

/// **Mete lo dibujado a la derecha de lo que ya hay** (con un lienzo
/// cerrado), alineado arriba; en un lienzo vacio, donde lo puso el modelo.
pub fn meter_al_lado(escena: &mut Escena, documento: &str) -> Result<usize, String> {
    meter_al_lado_ids(escena, documento).map(|v| v.len())
}

/// Lo mismo, devolviendo los ids de lo que entro.
pub fn meter_al_lado_ids(escena: &mut Escena, documento: &str) -> Result<Vec<u64>, String> {
    let els = elementos_de(documento)?;
    let nueva = caja(&els).ok_or("las figuras no tienen medida")?;
    let habia = escena
        .visibles()
        .map(Elemento::caja)
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
        .filter(|c| [c.0, c.1, c.2, c.3].iter().all(|v| v.is_finite()));
    let (dx, dy) = match habia {
        Some(h) => (h.2 + HUECO - nueva.0, h.1 - nueva.1),
        None => (0.0, 0.0),
    };
    Ok(pixpin_motor2d::portapapeles::pegar(escena, &els, dx, dy))
}

/// El resumen que se le devuelve al modelo.
pub fn resumen(n: usize) -> String {
    format!("hecho: {n} elementos anadidos al lienzo")
}

/// El «hecho» de lo dibujado con sus `#id` (en el marco del modelo, si el
/// turno tiene uno), para poder editarlo en la llamada siguiente.
pub fn resumen_con_ids(escena: &Escena, ids: Vec<u64>, marco: Option<(f32, f32)>) -> Respuesta {
    let ctx = Contexto {
        marco: marco.unwrap_or_else(|| pixpin_ia::edicion::marco_de(escena)),
        ..Default::default()
    };
    Respuesta {
        texto: pixpin_ia::edicion::resumen_de_lo_anadido(escena, &ids, &ctx),
        ids,
        ctx: None,
    }
}

/// Vacia la cola **en el editor abierto** sin imagenes ni seleccion (las
/// pruebas de antes). Ver [`atender_en_editor_con`].
pub fn atender_en_editor(escena: &mut Escena, centro: (f32, f32)) -> bool {
    let mut imagenes = crate::imagenes_lienzo::ImagenesLienzo::nuevo(16_384);
    let vista = (centro.0 - 400.0, centro.1 - 300.0, centro.0 + 400.0, centro.1 + 300.0);
    atender_en_editor_con(escena, vista, &[], &mut imagenes)
}

/// Vacia la cola **en el editor abierto**: lo llama su bucle en cada vuelta.
/// `vista` es lo que se ve `(x0, y0, x1, y1)`, en coordenadas del lienzo: lo
/// dibujado cae en su centro y el modelo mide desde su esquina. `elegidos`
/// es lo elegido ahora; `imagenes`, el almacen del editor (para pegar).
/// Devuelve si cambio algo (para repintar).
pub fn atender_en_editor_con(
    escena: &mut Escena,
    vista: (f32, f32, f32, f32),
    elegidos: &[u64],
    imagenes: &mut crate::imagenes_lienzo::ImagenesLienzo,
) -> bool {
    if !hay_pedidos() {
        return false;
    }
    let centro = ((vista.0 + vista.2) / 2.0, (vista.1 + vista.3) / 2.0);
    let mut algo = false;
    for p in tomar() {
        let r = match &p.trabajo {
            Trabajo::Dibujar => meter_ids(escena, &p.documento, centro).map(|ids| resumen_con_ids(escena, ids, p.marco)),
            t => {
                let antes = escena.pasos_cerrados();
                let r = trabajar(escena, t, elegidos, (vista.0.floor(), vista.1.floor()), imagenes);
                algo |= escena.pasos_cerrados() != antes;
                r
            }
        };
        algo |= r.is_ok() && matches!(p.trabajo, Trabajo::Dibujar);
        p.responder_con(r);
    }
    algo
}

// ---------------------------------------------------------------------------
// «Guarda esto como figura»: lo elegido en el lienzo abierto
// ---------------------------------------------------------------------------

/// Quien espera lo elegido (el turno del asistente). `HAY_PREGUNTA` evita
/// mirar el candado en cada vuelta del editor.
static PREGUNTA: Mutex<Option<Sender<Vec<Elemento>>>> = Mutex::new(None);
static HAY_PREGUNTA: AtomicBool = AtomicBool::new(false);

/// **Pide al lienzo abierto lo que tiene elegido** (vacio si nada). Lo
/// contesta su bucle en la siguiente vuelta ([`contestar_lo_elegido`]).
pub fn pedir_lo_elegido() -> Receiver<Vec<Elemento>> {
    let (tx, rx) = channel();
    if let Ok(mut p) = PREGUNTA.lock() {
        *p = Some(tx);
        HAY_PREGUNTA.store(true, Ordering::SeqCst);
    }
    despertar(EDITOR.load(Ordering::SeqCst));
    rx
}

/// El bucle del editor contesta, si alguien pregunto, con copias de lo
/// elegido. Casi siempre no hay pregunta y no cuesta nada.
pub fn contestar_lo_elegido(escena: &Escena, elegidos: &[u64]) {
    if !HAY_PREGUNTA.swap(false, Ordering::SeqCst) {
        return;
    }
    let Some(tx) = PREGUNTA.lock().ok().and_then(|mut p| p.take()) else { return };
    let piezas: Vec<Elemento> = escena
        .visibles()
        .filter(|e| elegidos.contains(&e.id))
        .cloned()
        .collect();
    let _ = tx.send(piezas);
}

#[cfg(test)]
pub(crate) fn vaciar_para_pruebas() {
    let _ = tomar();
    EDITOR.store(0, Ordering::SeqCst);
}
