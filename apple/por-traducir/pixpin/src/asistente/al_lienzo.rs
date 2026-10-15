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

use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::Elemento;

/// Un pedido de dibujar.
pub struct Pedido {
    /// La conversacion desde la que se pidio (adonde va si no hay lienzo
    /// abierto).
    pub proyecto: String,
    /// El lienzo de esa conversacion que se nombro con `@lienzo:` (su
    /// referencia). Sin el, el propio del asistente.
    pub destino: Option<String>,
    /// Un `.excalidraw` hecho por `pixpin_ia::lienzo::documento`.
    pub documento: String,
    responder: Sender<Result<String, String>>,
}

impl Pedido {
    pub fn nuevo(
        proyecto: String,
        destino: Option<String>,
        documento: String,
        responder: Sender<Result<String, String>>,
    ) -> Pedido {
        Pedido {
            proyecto,
            destino,
            documento,
            responder,
        }
    }

    /// Contesta al turno (que esta esperando).
    pub fn responder(self, r: Result<String, String>) {
        let _ = self.responder.send(r);
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
    let els = elementos_de(documento)?;
    let c = caja(&els).ok_or("las figuras no tienen medida")?;
    let dx = centro.0 - (c.0 + c.2) / 2.0;
    let dy = centro.1 - (c.1 + c.3) / 2.0;
    Ok(pixpin_motor2d::portapapeles::pegar(escena, &els, dx, dy).len())
}

/// Hueco entre lo que habia y lo nuevo.
const HUECO: f32 = 80.0;

/// **Mete lo dibujado a la derecha de lo que ya hay** (con un lienzo
/// cerrado), alineado arriba; en un lienzo vacio, donde lo puso el modelo.
pub fn meter_al_lado(escena: &mut Escena, documento: &str) -> Result<usize, String> {
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
    Ok(pixpin_motor2d::portapapeles::pegar(escena, &els, dx, dy).len())
}

/// El resumen que se le devuelve al modelo.
pub fn resumen(n: usize) -> String {
    format!("hecho: {n} elementos anadidos al lienzo")
}

/// Vacia la cola **en el editor abierto**: lo llama su bucle en cada vuelta.
/// `centro` es el centro de lo que se ve, en coordenadas del lienzo.
/// Devuelve si entro algo (para repintar).
pub fn atender_en_editor(escena: &mut Escena, centro: (f32, f32)) -> bool {
    if !hay_pedidos() {
        return false;
    }
    let mut algo = false;
    for p in tomar() {
        let r = meter(escena, &p.documento, centro).map(resumen);
        algo |= r.is_ok();
        p.responder(r);
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
