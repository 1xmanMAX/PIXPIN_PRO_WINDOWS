//! **Los comentarios, en tarjetas flotantes** en el margen derecho del
//! papel, como en Google Docs o Word (H12, 1-oct-2026; el usuario: «no
//! tienen que estar todos en una caja sino como tarjetas flotantes en un
//! lateral, y solo se muestren cuando le doy al boton de comentarios»).
//! Sin caja ni cabecera: cada comentario es una tarjeta con sombra **a la
//! altura de su frase**, que se mueve con el texto; si no caben, las de
//! debajo bajan, y la elegida se adelanta hacia la izquierda, se resalta y
//! empuja a las demas. Los que perdieron su sitio en el texto van arriba,
//! aparte. Con la ventana estrecha no caben al lado del papel: van en un
//! **cajon** que se pone encima del texto por la derecha, con su cabecera
//! para cerrarlo.
//!
//! Aqui solo donde va cada cosa y como se pinta (Direct2D para las formas,
//! GDI para las letras, como el resto del marco, ver `pintor`). Que hace un
//! clic lo decide el editor (`editor::comentarios`). La colocacion es una
//! lista de piezas ya medidas: pintar no mide nada, y las pruebas miran la
//! colocacion sin pintar.

use std::cell::{Cell, RefCell};

use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Gdi::*;

use crate::disposicion::{Caja, Marco};
use crate::pintor::{Icono, Pintor};
use crate::tema::{Rgb, Tema, bgr};

/// El ancho del margen de las tarjetas a 96 ppp.
pub const ANCHO_PX: i32 = 320;
/// Lo que tiene que quedarle al papel para que las tarjetas vayan a su
/// lado; si no, van en el cajon, encima del texto.
const PAPEL_MINIMO_PX: i32 = 480;
/// El cajon deja a la vista esto del texto, a su izquierda.
const CAJON_DEJA_PX: i32 = 48;
/// Cuanto se adelanta hacia la izquierda la tarjeta elegida.
pub const ADELANTE_PX: i32 = 14;
const CABECERA_PX: i32 = 44;
const HUECO_PX: i32 = 10;
const RELLENO_PX: i32 = 12;
const COMPOSITOR_PX: i32 = 64;

/// Los textos del panel, ya traducidos.
#[derive(Debug, Clone, Default)]
pub struct RotulosComentarios {
    pub comentarios: String,
    pub comentar: String,
    pub sin_comentarios: String,
    pub pista: String,
    pub sin_ancla: String,
    pub responder_pista: String,
    pub responder: String,
    pub guardar: String,
    pub cancelar: String,
    pub editar: String,
    pub borrar: String,
    pub resuelto: String,
    pub ver_resueltos: String,
    pub ocultar_resueltos: String,
    pub editado: String,
    pub borrar_hilo: String,
}

/// Con que letra va un texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Letra {
    Normal,
    Chica,
    Negrita,
}

/// Lo que se puede pulsar en el panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Accion {
    CerrarPanel,
    Filtro,
    /// La tarjeta (fuera de sus botones): se elige y se salta a su texto.
    Tarjeta(String),
    /// Resolver o reabrir el hilo.
    Resolver(String),
    /// El «⋯» de un comentario o de una respuesta (su id).
    Mas(String),
    /// «Responder…» del hilo.
    Responder(String),
    Enviar,
    Cancelar,
}

/// Una pieza de lo que se pinta, en pixeles de la ventana.
#[derive(Debug, Clone, PartialEq)]
pub enum Pieza {
    Caja { caja: Caja, radio: f32, color: Rgb },
    Borde { caja: Caja, radio: f32, grosor: f32, color: Rgb },
    Raya { x0: i32, y0: i32, x1: i32, color: Rgb },
    Icono { icono: Icono, caja: Caja, lado: f32, color: Rgb },
    Texto { caja: Caja, texto: String, letra: Letra, color: Rgb, varias: bool, centrado: bool },
}

impl Pieza {
    fn mover(&mut self, dx: i32, dy: i32) {
        let m = |c: &mut Caja| {
            c.x += dx;
            c.y += dy;
        };
        match self {
            Pieza::Caja { caja, .. } | Pieza::Borde { caja, .. } | Pieza::Icono { caja, .. } | Pieza::Texto { caja, .. } => {
                m(caja)
            }
            Pieza::Raya { x0, y0, x1, .. } => {
                *x0 += dx;
                *x1 += dx;
                *y0 += dy;
            }
        }
    }
}

/// Una entrada de un hilo como se ensena: el comentario o una respuesta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    pub id: String,
    pub autor: String,
    pub aparato: String,
    /// Ya escrita: «30 sept, 14:05 · editado».
    pub fecha: String,
    pub texto: String,
}

/// Lo que se esta escribiendo en una tarjeta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compositor {
    Nuevo,
    Responder,
    /// Cambiar el texto de esa entrada.
    Editar(String),
}

/// Una tarjeta por colocar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tarjeta {
    pub id: String,
    /// Donde esta su frase (alto de la ventana); `None`: sin ancla.
    pub ancla_y: Option<i32>,
    pub cita: String,
    pub resuelto: bool,
    pub activa: bool,
    /// El comentario y sus respuestas; vacia en uno nuevo.
    pub entradas: Vec<Entrada>,
    pub compositor: Option<Compositor>,
    /// Quien escribe uno nuevo (nombre, aparato).
    pub autor_nuevo: (String, String),
}

/// Lo que mide quien pinta, con su letra.
pub trait Medir {
    fn ancho(&self, l: Letra, t: &str) -> i32;
    /// Lo alto de `t` partido en renglones de `ancho`.
    fn alto(&self, l: Letra, t: &str, ancho: i32) -> i32;
}

/// Todo lo que hace falta para colocar el panel.
pub struct Datos<'a> {
    pub tarjetas: Vec<Tarjeta>,
    /// Cuantos resueltos hay (se ven o no segun `ver_resueltos`).
    pub resueltos: usize,
    pub ver_resueltos: bool,
    pub rotulos: &'a RotulosComentarios,
    pub tema: Tema,
    pub escala: f32,
    /// En el cajon (ventana estrecha): encima del texto, con cabecera.
    pub cajon: bool,
}

/// El panel colocado: lo que se pinta y lo que se pulsa.
#[derive(Debug, Clone, Default)]
pub struct Vista {
    pub caja: Caja,
    /// Va en el cajon, encima del texto (ver [`recortar`]).
    pub cajon: bool,
    /// Donde van las tarjetas que siguen al texto (se recortan aqui).
    pub zona_movil: Caja,
    pub fijas: Vec<Pieza>,
    pub moviles: Vec<Pieza>,
    /// Lo que se pulsa; `true` si es de la zona movil (se recorta con ella).
    pub zonas: Vec<(Accion, Caja, bool)>,
    /// Donde va el cuadro de escribir, si se escribe; con su zona.
    pub compositor: Option<(Caja, bool)>,
    /// Donde quedo cada tarjeta (id, caja), para saltar y para las pruebas.
    pub tarjetas: Vec<(String, Caja)>,
}

thread_local! {
    /// El panel tal como se pinta ahora (lo pone el bucle; lo lee WM_PAINT).
    pub static VISTA: RefCell<Option<Vista>> = const { RefCell::new(None) };
    /// Si el panel esta abierto: lo mira `recortar` al colocar el marco.
    pub static ABIERTO: Cell<bool> = const { Cell::new(false) };
    /// Los comentarios abiertos, para el numero del boton de la cabecera.
    pub static CONTADOR: Cell<usize> = const { Cell::new(0) };
    /// Donde va el panel ahora (lo pone `recortar`); `None`, cerrado.
    pub static CAJA: Cell<Option<Caja>> = const { Cell::new(None) };
    /// Si va en el cajon (encima del texto) y no al lado del papel.
    pub static CAJON: Cell<bool> = const { Cell::new(false) };
    /// Un clic en el panel que espera al bucle.
    static CLIC: Cell<Option<(i32, i32)>> = const { Cell::new(None) };
}

/// Los colores propios de los comentarios, sacados del tema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colores {
    /// El fondo del texto comentado y el del comentario elegido.
    pub resaltado: Rgb,
    pub resaltado_activo: Rgb,
    pub tarjeta: Rgb,
    pub fondo: Rgb,
}

pub fn colores(t: &Tema) -> Colores {
    if t.oscuro {
        Colores {
            resaltado: 0x3d3317,
            resaltado_activo: 0x6b5414,
            tarjeta: 0x1a1a19,
            fondo: 0x101010,
        }
    } else {
        Colores {
            resaltado: 0xfcefc2,
            resaltado_activo: 0xf6d66a,
            tarjeta: 0xffffff,
            fondo: 0xf7f6f2,
        }
    }
}

/// El color del redondel de cada aparato: siempre el mismo para el mismo.
fn color_de(aparato: &str) -> Rgb {
    const PALETA: [Rgb; 6] = [0x5b8def, 0xd9822b, 0x3aa776, 0xb45fc9, 0xd2555a, 0x2f9fb3];
    let h = aparato.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193));
    PALETA[(h % PALETA.len() as u32) as usize]
}

fn inicial(autor: &str) -> String {
    autor.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into())
}

/// **El sitio de las tarjetas**, si estan a la vista: con sitio, encima del
/// margen derecho del papel, sin quitarle nada (el papel sigue siendo uno;
/// el usuario, 1-oct: «los comentarios no estan flotantes sino que siguen
/// en su caja»): la columna de texto se corre a la izquierda lo justo para
/// dejarles su sitio ([`reserva`]) y la barra de desplazamiento de la nota
/// sigue en el borde. En una ventana estrecha, el cajon encima del texto
/// por la derecha. Devuelve su caja, o `None`.
pub fn recortar(m: &mut Marco, escala: f32) -> Option<Caja> {
    let (caja, cajon) = recorte(m, escala).map_or((None, false), |(c, k)| (Some(c), k));
    CAJA.with(|c| c.set(caja));
    CAJON.with(|c| c.set(cajon));
    caja
}

/// Lo que la columna de texto tiene que dejar libre a la derecha del papel
/// para las tarjetas (0 sin ellas o en el cajon), en pixeles.
pub fn reserva() -> i32 {
    if CAJON.with(Cell::get) {
        return 0;
    }
    CAJA.with(Cell::get).map_or(0, |c| c.an)
}

fn recorte(m: &mut Marco, escala: f32) -> Option<(Caja, bool)> {
    if !ABIERTO.with(Cell::get) {
        return None;
    }
    let e = |v: i32| (v as f32 * escala).round() as i32;
    let an = e(ANCHO_PX);
    // La barra fina de la nota va en el borde derecho del papel: se deja.
    let barra = e(crate::imagenes::BARRA_COGER_PX);
    if m.cuerpo.an - an - barra >= e(PAPEL_MINIMO_PX) {
        let caja = Caja { x: m.cuerpo.derecha() - barra - an, y: m.cuerpo.y, an, al: m.cuerpo.al };
        return Some((caja, false));
    }
    let an = an.min(m.cuerpo.an - e(CAJON_DEJA_PX));
    if an <= 0 {
        return None;
    }
    let caja = Caja { x: m.cuerpo.derecha() - an, y: m.cuerpo.y, an, al: m.cuerpo.al };
    Some((caja, true))
}

/// Un color entre `a` y `b` (`t` de 0 a 255 hacia `b`): las sombras sin
/// transparencia, sobre el fondo que tienen debajo.
fn mezcla(a: Rgb, b: Rgb, t: u32) -> Rgb {
    let c = |s: u32| {
        let (x, y) = ((a >> s) & 0xff, (b >> s) & 0xff);
        ((x * (255 - t) + y * t) / 255) << s
    };
    c(16) | c(8) | c(0)
}

/// La sombra de una tarjeta: unas capas cada vez mas anchas y mas claras,
/// algo caidas hacia abajo, sobre el fondo `debajo`.
fn sombra(caja: Caja, debajo: Rgb, escala: f32) -> Vec<Pieza> {
    let e = |v: i32| (v as f32 * escala).round().max(1.0) as i32;
    [(4, 18u32), (3, 30), (2, 44), (1, 60)]
        .iter()
        .map(|&(k, t)| Pieza::Caja {
            caja: Caja { x: caja.x - e(k), y: caja.y - e(k) + e(2), an: caja.an + 2 * e(k), al: caja.al + 2 * e(k) },
            radio: 8.0 + k as f32,
            color: mezcla(debajo, 0x000000, t),
        })
        .collect()
}

/// **Donde van las tarjetas que siguen al texto**: cada una a la altura de
/// su frase sin montarse en la de encima; si hay una elegida, esa va justo
/// en su frase y las demas se apartan hacia arriba y hacia abajo. Entran
/// ordenadas por su sitio en el texto: `(alto de la frase, alto de la
/// tarjeta)`. Pueden quedar fuera de la vista: se recortan.
pub fn apilar(tarjetas: &[(i32, i32)], activa: Option<usize>, hueco: i32) -> Vec<i32> {
    let n = tarjetas.len();
    let mut y = vec![0; n];
    if n == 0 {
        return y;
    }
    let desde = activa.filter(|&a| a < n).unwrap_or(0);
    y[desde] = tarjetas[desde].0;
    for i in desde + 1..n {
        y[i] = tarjetas[i].0.max(y[i - 1] + tarjetas[i - 1].1 + hueco);
    }
    for i in (0..desde).rev() {
        y[i] = tarjetas[i].0.min(y[i + 1] - hueco - tarjetas[i].1);
    }
    y
}

/// «30 sept, 14:05», o con el ano si no es este. `cuando` es UTC; `desfase`,
/// lo que va la hora local por delante de la UTC.
pub fn fecha_corta(cuando: i64, desfase: i64, ahora: i64, meses: &[String]) -> String {
    let local = cuando + desfase;
    let (d, m, a) = pixpin_docs::md_comandos::dia_de(local);
    let (_, _, este) = pixpin_docs::md_comandos::dia_de(ahora + desfase);
    let mes = meses.get(m.saturating_sub(1) as usize).cloned().unwrap_or_else(|| format!("{m:02}"));
    if a != este {
        return format!("{d} {mes} {a}");
    }
    let minutos = local.div_euclid(60_000).rem_euclid(24 * 60);
    format!("{d} {mes}, {:02}:{:02}", minutos / 60, minutos % 60)
}

/// Lo que va dentro de una tarjeta, colocado desde su esquina (0, 0).
struct Hecha {
    piezas: Vec<Pieza>,
    zonas: Vec<(Accion, Caja)>,
    compositor: Option<Caja>,
    alto: i32,
}

fn tarjeta(t: &Tarjeta, an: i32, d: &Datos, medir: &dyn Medir) -> Hecha {
    let e = |v: i32| (v as f32 * d.escala).round() as i32;
    let tm = &d.tema;
    let co = colores(tm);
    let r = d.rotulos;
    let p = e(RELLENO_PX);
    let w = an - 2 * p;
    let mut piezas = Vec::new();
    let mut zonas = vec![(Accion::Tarjeta(t.id.clone()), Caja { x: 0, y: 0, an, al: 0 })];
    let mut compositor = None;
    let mut y = p;
    let tenue = if t.resuelto { tm.apagado } else { tm.tenue };
    let texto_c = if t.resuelto { tm.tenue } else { tm.texto };
    let redondel = |piezas: &mut Vec<Pieza>, x: i32, y: i32, lado: i32, autor: &str, aparato: &str| {
        let c = Caja { x, y, an: lado, al: lado };
        piezas.push(Pieza::Caja {
            caja: c,
            radio: lado as f32 / 2.0 / d.escala,
            color: color_de(aparato),
        });
        piezas.push(Pieza::Texto {
            caja: c,
            texto: inicial(autor),
            letra: if lado >= e(24) { Letra::Negrita } else { Letra::Chica },
            color: 0xffffff,
            varias: false,
            centrado: true,
        });
    };
    // El cuadro de escribir con sus dos botones debajo.
    let escribir = |piezas: &mut Vec<Pieza>, zonas: &mut Vec<(Accion, Caja)>, y: &mut i32, enviar: &str| -> Caja {
        let caja = Caja { x: p, y: *y, an: w, al: e(COMPOSITOR_PX) };
        piezas.push(Pieza::Caja { caja, radio: 6.0, color: tm.pastilla });
        piezas.push(Pieza::Borde { caja, radio: 6.0, grosor: 1.0, color: tm.acento });
        *y += caja.al + e(8);
        let an_enviar = medir.ancho(Letra::Negrita, enviar) + e(24);
        let b_enviar = Caja { x: p + w - an_enviar, y: *y, an: an_enviar, al: e(28) };
        piezas.push(Pieza::Caja { caja: b_enviar, radio: 7.0, color: tm.acento });
        piezas.push(Pieza::Texto {
            caja: b_enviar,
            texto: enviar.to_string(),
            letra: Letra::Negrita,
            color: 0xffffff,
            varias: false,
            centrado: true,
        });
        let an_cancelar = medir.ancho(Letra::Normal, &r.cancelar) + e(20);
        let b_cancelar = Caja { x: b_enviar.x - e(6) - an_cancelar, y: *y, an: an_cancelar, al: e(28) };
        piezas.push(Pieza::Texto {
            caja: b_cancelar,
            texto: r.cancelar.clone(),
            letra: Letra::Normal,
            color: tm.tenue,
            varias: false,
            centrado: true,
        });
        zonas.push((Accion::Enviar, b_enviar));
        zonas.push((Accion::Cancelar, b_cancelar));
        *y += e(28);
        // El control de escribir va dentro del cuadro, con su margen.
        Caja { x: caja.x + e(8), y: caja.y + e(6), an: caja.an - e(16), al: caja.al - e(12) }
    };
    // Cabecera: redondel, quien y cuando; con la tarjeta elegida, resolver y «⋯».
    let (autor, aparato, fecha) = match t.entradas.first() {
        Some(x) => (x.autor.clone(), x.aparato.clone(), x.fecha.clone()),
        None => (t.autor_nuevo.0.clone(), t.autor_nuevo.1.clone(), String::new()),
    };
    redondel(&mut piezas, p, y, e(26), &autor, &aparato);
    let botones = if t.activa && !t.entradas.is_empty() { e(26) * 2 + e(4) } else { 0 };
    let x_texto = p + e(34);
    piezas.push(Pieza::Texto {
        caja: Caja { x: x_texto, y, an: w - e(34) - botones, al: e(15) },
        texto: autor,
        letra: Letra::Negrita,
        color: texto_c,
        varias: false,
        centrado: false,
    });
    let linea2 = if t.resuelto { format!("{fecha} · {}", r.resuelto) } else { fecha };
    piezas.push(Pieza::Texto {
        caja: Caja { x: x_texto, y: y + e(16), an: w - e(34) - botones, al: e(14) },
        texto: linea2,
        letra: Letra::Chica,
        color: tenue,
        varias: false,
        centrado: false,
    });
    if let (true, Some(raiz)) = (t.activa, t.entradas.first()) {
        let mas = Caja { x: p + w - e(26), y, an: e(26), al: e(26) };
        let hecho = Caja { x: mas.x - e(4) - e(26), ..mas };
        if t.resuelto {
            piezas.push(Pieza::Caja { caja: hecho, radio: 6.0, color: tm.pastilla });
        }
        piezas.push(Pieza::Icono {
            icono: Icono::Hecho,
            caja: hecho,
            lado: 15.0,
            color: if t.resuelto { tm.hecha } else { tm.tenue },
        });
        piezas.push(Pieza::Icono { icono: Icono::Puntos, caja: mas, lado: 15.0, color: tm.tenue });
        zonas.push((Accion::Resolver(t.id.clone()), hecho));
        zonas.push((Accion::Mas(raiz.id.clone()), mas));
    }
    y += e(34);
    // La cita, en un renglon con su raya.
    if !t.cita.trim().is_empty() {
        piezas.push(Pieza::Caja {
            caja: Caja { x: p, y: y + e(2), an: e(3), al: e(15) },
            radio: 1.0,
            color: co.resaltado_activo,
        });
        let cita = t.cita.split_whitespace().collect::<Vec<_>>().join(" ");
        piezas.push(Pieza::Texto {
            caja: Caja { x: p + e(10), y, an: w - e(10), al: e(19) },
            texto: cita,
            letra: Letra::Chica,
            color: tenue,
            varias: false,
            centrado: false,
        });
        y += e(25);
    }
    let editando = |id: &str| matches!(&t.compositor, Some(Compositor::Editar(x)) if x == id);
    for (i, en) in t.entradas.iter().enumerate() {
        if i > 0 {
            y += e(12);
            redondel(&mut piezas, p, y, e(20), &en.autor, &en.aparato);
            let an_autor = medir.ancho(Letra::Negrita, &en.autor).min(w - e(60));
            piezas.push(Pieza::Texto {
                caja: Caja { x: p + e(28), y, an: an_autor, al: e(20) },
                texto: en.autor.clone(),
                letra: Letra::Negrita,
                color: texto_c,
                varias: false,
                centrado: false,
            });
            let x_fecha = p + e(28) + an_autor + e(6);
            let tope = if t.activa { e(28) } else { 0 };
            piezas.push(Pieza::Texto {
                caja: Caja { x: x_fecha, y, an: (p + w - tope - x_fecha).max(0), al: e(20) },
                texto: en.fecha.clone(),
                letra: Letra::Chica,
                color: tenue,
                varias: false,
                centrado: false,
            });
            if t.activa {
                let mas = Caja { x: p + w - e(24), y: y - e(2), an: e(24), al: e(24) };
                piezas.push(Pieza::Icono { icono: Icono::Puntos, caja: mas, lado: 14.0, color: tm.tenue });
                zonas.push((Accion::Mas(en.id.clone()), mas));
            }
            y += e(26);
        }
        if editando(&en.id) {
            compositor = Some(escribir(&mut piezas, &mut zonas, &mut y, &r.guardar));
        } else {
            let al = medir.alto(Letra::Normal, &en.texto, w).max(e(18));
            piezas.push(Pieza::Texto {
                caja: Caja { x: p, y, an: w, al },
                texto: en.texto.clone(),
                letra: Letra::Normal,
                color: texto_c,
                varias: true,
                centrado: false,
            });
            y += al;
        }
    }
    match &t.compositor {
        Some(Compositor::Nuevo) => {
            compositor = Some(escribir(&mut piezas, &mut zonas, &mut y, &r.comentar));
        }
        Some(Compositor::Responder) => {
            y += e(12);
            compositor = Some(escribir(&mut piezas, &mut zonas, &mut y, &r.responder));
        }
        Some(Compositor::Editar(_)) => {}
        None if t.activa && !t.resuelto => {
            y += e(12);
            let pastilla = Caja { x: p, y, an: w, al: e(30) };
            piezas.push(Pieza::Borde { caja: pastilla, radio: 15.0, grosor: 1.0, color: tm.raya });
            piezas.push(Pieza::Texto {
                caja: Caja { x: p + e(12), an: w - e(24), ..pastilla },
                texto: r.responder_pista.clone(),
                letra: Letra::Normal,
                color: tm.tenue,
                varias: false,
                centrado: false,
            });
            zonas.push((Accion::Responder(t.id.clone()), pastilla));
            y += e(30);
        }
        None => {}
    }
    y += p;
    // El fondo y el borde, debajo de todo.
    let caja = Caja { x: 0, y: 0, an, al: y };
    let (borde, grosor) = if t.activa { (tm.acento, 1.5) } else { (tm.raya, 1.0) };
    piezas.insert(0, Pieza::Borde { caja, radio: 8.0, grosor, color: borde });
    piezas.insert(0, Pieza::Caja { caja, radio: 8.0, color: co.tarjeta });
    zonas[0].1.al = y;
    Hecha { piezas, zonas, compositor, alto: y }
}

/// **Coloca las tarjetas** en `panel` (pixeles de la ventana).
pub fn disponer(d: &Datos, panel: Caja, medir: &dyn Medir) -> Vista {
    let e = |v: i32| (v as f32 * d.escala).round() as i32;
    let tm = &d.tema;
    let co = colores(tm);
    let r = d.rotulos;
    let mut v = Vista { caja: panel, cajon: d.cajon, ..Default::default() };
    // Sin caja: en el margen las tarjetas flotan sobre el papel; el cajon,
    // que va encima del texto, tiene su fondo, su sombra y su cabecera.
    let debajo = if d.cajon { co.fondo } else { tm.papel };
    v.fijas.push(Pieza::Caja { caja: panel, radio: 0.0, color: debajo });
    let mut y = panel.y + e(14);
    if d.cajon {
        for k in 0..6 {
            v.fijas.push(Pieza::Caja {
                caja: Caja { x: panel.x + k * e(1).max(1), y: panel.y, an: e(1).max(1), al: panel.al },
                radio: 0.0,
                color: mezcla(debajo, 0x000000, (90 - 15 * k) as u32),
            });
        }
        let cab = Caja { x: panel.x, y: panel.y, an: panel.an, al: e(CABECERA_PX) };
        let cerrar = Caja { x: cab.derecha() - e(10) - e(28), y: cab.y + (cab.al - e(28)) / 2, an: e(28), al: e(28) };
        let filtro = Caja { x: cerrar.x - e(4) - e(28), ..cerrar };
        v.fijas.push(Pieza::Texto {
            caja: Caja { x: cab.x + e(20), an: filtro.x - cab.x - e(24), ..cab },
            texto: r.comentarios.clone(),
            letra: Letra::Negrita,
            color: tm.texto,
            varias: false,
            centrado: false,
        });
        if d.ver_resueltos {
            v.fijas.push(Pieza::Caja { caja: filtro, radio: 6.0, color: tm.pastilla });
        }
        v.fijas.push(Pieza::Icono {
            icono: Icono::Filtro,
            caja: filtro,
            lado: 15.0,
            color: if d.ver_resueltos { tm.acento } else { tm.tenue },
        });
        v.fijas.push(Pieza::Icono { icono: Icono::Cerrar, caja: cerrar, lado: 14.0, color: tm.tenue });
        v.zonas.push((Accion::Filtro, filtro, false));
        v.zonas.push((Accion::CerrarPanel, cerrar, false));
        v.fijas.push(Pieza::Caja { caja: Caja { x: panel.x, y: cab.abajo() - 1, an: panel.an, al: 1 }, radio: 0.0, color: tm.raya });
        y = cab.abajo() + e(12);
    }
    // Sitio a la izquierda para que la elegida se adelante, y para la sombra.
    let x = panel.x + e(ADELANTE_PX) + e(10);
    let an = panel.an - e(ADELANTE_PX) - e(26);
    // Los resueltos: cuantos hay y verlos u ocultarlos (un enlace, sin caja).
    if d.resueltos > 0 {
        let t = if d.ver_resueltos { r.ocultar_resueltos.clone() } else { format!("{} ({})", r.ver_resueltos, d.resueltos) };
        let w = medir.ancho(Letra::Chica, &t) + e(8);
        let c = Caja { x: x + an - w, y, an: w, al: e(20) };
        v.fijas.push(Pieza::Texto { caja: c, texto: t, letra: Letra::Chica, color: tm.acento, varias: false, centrado: true });
        v.zonas.push((Accion::Filtro, c, false));
        y += e(28);
    }
    if d.tarjetas.is_empty() {
        // Una tarjeta que dice que no hay y como poner uno.
        let p = e(RELLENO_PX);
        let al_pista = medir.alto(Letra::Chica, &r.pista, an - 2 * p);
        let caja = Caja { x, y: y + e(4), an, al: p + e(18) + e(6) + al_pista + p };
        v.fijas.extend(sombra(caja, debajo, d.escala));
        v.fijas.push(Pieza::Caja { caja, radio: 8.0, color: co.tarjeta });
        v.fijas.push(Pieza::Borde { caja, radio: 8.0, grosor: 1.0, color: tm.raya });
        let c = Caja { x: x + p, y: caja.y + p, an: an - 2 * p, al: e(18) };
        v.fijas.push(Pieza::Texto {
            caja: c,
            texto: r.sin_comentarios.clone(),
            letra: Letra::Negrita,
            color: tm.texto,
            varias: false,
            centrado: false,
        });
        v.fijas.push(Pieza::Texto {
            caja: Caja { x: x + p, y: c.abajo() + e(6), an: an - 2 * p, al: al_pista },
            texto: r.pista.clone(),
            letra: Letra::Chica,
            color: tm.tenue,
            varias: true,
            centrado: false,
        });
        v.zona_movil = Caja { x: panel.x, y: panel.abajo(), an: panel.an, al: 0 };
        return v;
    }
    // Los que perdieron su sitio, arriba y quietos.
    let (sueltas, ancladas): (Vec<&Tarjeta>, Vec<&Tarjeta>) = d.tarjetas.iter().partition(|t| t.ancla_y.is_none());
    if !sueltas.is_empty() {
        v.fijas.push(Pieza::Texto {
            caja: Caja { x: x + e(4), y, an, al: e(18) },
            texto: r.sin_ancla.clone(),
            letra: Letra::Chica,
            color: tm.tenue,
            varias: false,
            centrado: false,
        });
        y += e(22);
        for t in sueltas {
            let mut h = tarjeta(t, an, d, medir);
            let tx = if t.activa { x - e(ADELANTE_PX) } else { x };
            for pz in &mut h.piezas {
                pz.mover(tx, y);
            }
            v.fijas.extend(sombra(Caja { x: tx, y, an, al: h.alto }, debajo, d.escala));
            v.fijas.extend(h.piezas);
            for (a, mut c) in h.zonas {
                c.x += tx;
                c.y += y;
                v.zonas.push((a, c, false));
            }
            if let Some(mut c) = h.compositor {
                c.x += tx;
                c.y += y;
                v.compositor = Some((c, false));
            }
            v.tarjetas.push((t.id.clone(), Caja { x: tx, y, an, al: h.alto }));
            y += h.alto + e(HUECO_PX);
        }
        y += e(6);
    }
    v.zona_movil = Caja { x: panel.x, y, an: panel.an, al: (panel.abajo() - y).max(0) };
    // Los que siguen al texto, cada uno a la altura de su frase.
    let hechas: Vec<Hecha> = ancladas.iter().map(|t| tarjeta(t, an, d, medir)).collect();
    // Una frase a la vista no deja su tarjeta debajo de los sin ancla: la
    // tarjeta baja hasta donde empieza su zona.
    let tope = v.zona_movil.y;
    let pedidos: Vec<(i32, i32)> = ancladas
        .iter()
        .zip(&hechas)
        .map(|(t, h)| {
            let y = t.ancla_y.unwrap_or(0);
            (if y >= panel.y { y.max(tope) } else { y }, h.alto)
        })
        .collect();
    let activa = ancladas.iter().position(|t| t.activa || t.compositor.is_some());
    let ys = apilar(&pedidos, activa, e(HUECO_PX));
    for ((t, h), ty) in ancladas.iter().zip(hechas).zip(ys) {
        let tx = if t.activa || t.compositor.is_some() { x - e(ADELANTE_PX) } else { x };
        let mut piezas = sombra(Caja { x: tx, y: ty, an, al: h.alto }, debajo, d.escala);
        let mut propias = h.piezas;
        for pz in &mut propias {
            pz.mover(tx, ty);
        }
        piezas.extend(propias);
        v.moviles.extend(piezas);
        for (a, mut c) in h.zonas {
            c.x += tx;
            c.y += ty;
            v.zonas.push((a, c, true));
        }
        if let Some(mut c) = h.compositor {
            c.x += tx;
            c.y += ty;
            v.compositor = Some((c, true));
        }
        v.tarjetas.push((t.id.clone(), Caja { x: tx, y: ty, an, al: h.alto }));
    }
    v
}

impl Vista {
    /// Lo que hay bajo un punto de la ventana (lo de encima gana).
    pub fn accion_en(&self, x: i32, y: i32) -> Option<Accion> {
        if !self.caja.contiene(x, y) {
            return None;
        }
        self.zonas
            .iter()
            .rev()
            .find(|(_, c, movil)| c.contiene(x, y) && (!movil || self.zona_movil.contiene(x, y)))
            .map(|(a, _, _)| a.clone())
    }

    /// Si el cuadro de escribir se ve (su zona no lo recorta entero).
    pub fn compositor_visible(&self) -> Option<Caja> {
        let (c, movil) = self.compositor?;
        if !movil {
            return Some(c);
        }
        let z = self.zona_movil;
        (c.y >= z.y && c.abajo() <= z.abajo()).then_some(c)
    }
}

/// Apunta un clic en el panel para el bucle. `true` si el punto es del panel.
pub fn clic(x: i32, y: i32) -> bool {
    let dentro = VISTA.with(|v| v.borrow().as_ref().is_some_and(|v| v.caja.contiene(x, y)));
    if dentro {
        CLIC.with(|c| c.set(Some((x, y))));
    }
    dentro
}

/// El clic que esperaba, si lo hay.
pub fn tomar_clic() -> Option<(i32, i32)> {
    CLIC.with(Cell::take)
}

// ---------------------------------------------------------------------------
// Medir y pintar con GDI

/// Mide con las letras del pintor en un `HDC`.
pub struct MedirGdi<'a> {
    pub hdc: HDC,
    pub pintor: &'a Pintor,
}

fn letra_de(p: &Pintor, l: Letra) -> HFONT {
    match l {
        Letra::Normal => p.letra,
        Letra::Chica => p.letra_chica,
        Letra::Negrita => p.letra_negrita,
    }
}

const PARTIR: DRAW_TEXT_FORMAT = DRAW_TEXT_FORMAT(DT_WORDBREAK.0 | DT_NOPREFIX.0 | DT_EDITCONTROL.0);

impl Medir for MedirGdi<'_> {
    fn ancho(&self, l: Letra, t: &str) -> i32 {
        self.pintor.medir(self.hdc, letra_de(self.pintor, l), t)
    }

    fn alto(&self, l: Letra, t: &str, ancho: i32) -> i32 {
        let mut b: Vec<u16> = if t.is_empty() { vec![b' ' as u16] } else { t.encode_utf16().collect() };
        let mut r = RECT { left: 0, top: 0, right: ancho.max(1), bottom: 0 };
        // SAFETY: HDC y letra vivos; el texto es un bufer propio; se devuelve
        // la letra que habia.
        unsafe {
            let vieja = SelectObject(self.hdc, HGDIOBJ(letra_de(self.pintor, l).0));
            DrawTextW(self.hdc, &mut b, &mut r, PARTIR | DT_CALCRECT);
            SelectObject(self.hdc, vieja);
        }
        r.bottom - r.top
    }
}

fn texto_partido(hdc: HDC, p: &Pintor, l: Letra, t: &str, caja: Caja, tinta: Rgb, centrado: bool) {
    if t.is_empty() {
        return;
    }
    let mut b: Vec<u16> = t.encode_utf16().collect();
    let mut r = RECT { left: caja.x, top: caja.y, right: caja.derecha(), bottom: caja.abajo() };
    let f = if centrado { PARTIR | DT_CENTER } else { PARTIR };
    // SAFETY: como en `alto`.
    unsafe {
        let vieja = SelectObject(hdc, HGDIOBJ(letra_de(p, l).0));
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, COLORREF(bgr(tinta)));
        DrawTextW(hdc, &mut b, &mut r, f);
        SelectObject(hdc, vieja);
    }
}

fn pintar_piezas(hdc: HDC, p: &Pintor, piezas: &[Pieza], zona: Caja) {
    if zona.an <= 0 || zona.al <= 0 {
        return;
    }
    let r = RECT { left: zona.x, top: zona.y, right: zona.derecha(), bottom: zona.abajo() };
    p.formas(hdc, r, |f| {
        for pz in piezas {
            match pz {
                Pieza::Caja { caja, radio, color } => f.redondo(*caja, *radio, *color),
                Pieza::Borde { caja, radio, grosor, color } => f.borde(*caja, *radio, *grosor, *color),
                Pieza::Raya { x0, y0, x1, color } => f.raya(*x0, *y0, *x1, *y0, *color),
                Pieza::Icono { icono, caja, lado, color } => f.icono(*icono, *caja, *lado, *color),
                Pieza::Texto { .. } => {}
            }
        }
    });
    // SAFETY: el recorte se guarda y se devuelve en el mismo HDC.
    unsafe {
        let guardado = SaveDC(hdc);
        IntersectClipRect(hdc, zona.x, zona.y, zona.derecha(), zona.abajo());
        for pz in piezas {
            if let Pieza::Texto { caja, texto, letra, color, varias, centrado } = pz {
                if *varias {
                    texto_partido(hdc, p, *letra, texto, *caja, *color, *centrado);
                } else {
                    p.texto(hdc, letra_de(p, *letra), texto, *caja, *color, *centrado);
                }
            }
        }
        let _ = RestoreDC(hdc, guardado);
    }
}

/// Pinta el panel (si esta abierto) en el `HDC` de la ventana.
pub fn pintar(hdc: HDC, p: &Pintor) {
    pintar_en(hdc, p, (0, 0));
}

/// Pinta las tarjetas con el origen en `origen` (pixeles de la ventana): la
/// ventana de las tarjetas pinta lo suyo con su esquina en (0, 0).
pub fn pintar_en(hdc: HDC, p: &Pintor, (ox, oy): (i32, i32)) {
    VISTA.with(|v| {
        if !ABIERTO.with(Cell::get) {
            return;
        }
        if let Some(v) = v.borrow().as_ref() {
            let mover = |ps: &[Pieza]| -> Vec<Pieza> {
                ps.iter()
                    .cloned()
                    .map(|mut x| {
                        x.mover(-ox, -oy);
                        x
                    })
                    .collect()
            };
            let caja = |c: Caja| Caja { x: c.x - ox, y: c.y - oy, ..c };
            pintar_piezas(hdc, p, &mover(&v.fijas), caja(v.caja));
            pintar_piezas(hdc, p, &mover(&v.moviles), caja(v.zona_movil));
        }
    });
}

/// El boton de la cabecera: el globo, con fondo si el panel esta abierto, y
/// el numero de comentarios abiertos en su esquina.
pub fn pintar_boton(hdc: HDC, p: &Pintor, caja: Caja, hover: bool, tema: &Tema) {
    let abierto = ABIERTO.with(Cell::get);
    let n = CONTADOR.with(Cell::get);
    let e = |v: i32| (v as f32 * p.escala).round() as i32;
    let insignia = Caja { x: caja.derecha() - e(13), y: caja.y - e(2), an: e(15), al: e(15) };
    let zona = RECT { left: caja.x, top: caja.y - e(4), right: caja.derecha() + e(4), bottom: caja.abajo() };
    p.formas(hdc, zona, |f| {
        if abierto || hover {
            f.redondo(caja, 6.0, tema.pastilla);
        }
        f.icono(Icono::Comentario, caja, 15.0, if abierto { tema.acento } else { tema.texto });
        if n > 0 {
            f.redondo(insignia, 7.5, tema.acento);
        }
    });
    if n > 0 {
        let t = if n > 99 { "99".to_string() } else { n.to_string() };
        p.texto(hdc, p.letra_chica, &t, insignia, 0xffffff, true);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Letras de mentira: 7 px por letra, renglones de 18.
    struct Falso;
    impl Medir for Falso {
        fn ancho(&self, _: Letra, t: &str) -> i32 {
            t.chars().count() as i32 * 7
        }
        fn alto(&self, _: Letra, t: &str, ancho: i32) -> i32 {
            let por_renglon = (ancho / 7).max(1) as usize;
            t.split('\n').map(|r| r.chars().count().div_ceil(por_renglon).max(1)).sum::<usize>() as i32 * 18
        }
    }

    fn rotulos() -> RotulosComentarios {
        RotulosComentarios {
            comentarios: "Comentarios".into(),
            comentar: "Comentar".into(),
            sin_comentarios: "No hay comentarios".into(),
            pista: "Elige un trozo y pulsa Comentar".into(),
            sin_ancla: "Sin sitio en el texto".into(),
            responder_pista: "Responder…".into(),
            responder: "Responder".into(),
            guardar: "Guardar".into(),
            cancelar: "Cancelar".into(),
            ver_resueltos: "Ver resueltos".into(),
            ocultar_resueltos: "Ocultar resueltos".into(),
            ..Default::default()
        }
    }

    fn t(id: &str, y: Option<i32>, texto: &str) -> Tarjeta {
        Tarjeta {
            id: id.into(),
            ancla_y: y,
            cita: "la losa".into(),
            resuelto: false,
            activa: false,
            entradas: vec![Entrada {
                id: id.into(),
                autor: "Portátil".into(),
                aparato: "K7Q2".into(),
                fecha: "30 sept, 14:05".into(),
                texto: texto.into(),
            }],
            compositor: None,
            autor_nuevo: ("Portátil".into(), "K7Q2".into()),
        }
    }

    const PANEL: Caja = Caja { x: 700, y: 82, an: 300, al: 700 };

    fn colocar(tarjetas: Vec<Tarjeta>, resueltos: usize) -> Vista {
        colocar_en(tarjetas, resueltos, false)
    }

    fn colocar_en(tarjetas: Vec<Tarjeta>, resueltos: usize, cajon: bool) -> Vista {
        let r = rotulos();
        let d = Datos {
            tarjetas,
            resueltos,
            ver_resueltos: false,
            rotulos: &r,
            tema: Tema::oscuro(),
            escala: 1.0,
            cajon,
        };
        disponer(&d, PANEL, &Falso)
    }

    fn textos(ps: &[Pieza]) -> Vec<String> {
        ps.iter()
            .filter_map(|p| match p {
                Pieza::Texto { texto, .. } => Some(texto.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn en_el_margen_las_tarjetas_flotan_sin_caja_con_sombra_y_la_elegida_se_adelanta() {
        let mut a = t("a", Some(200), "uno");
        a.activa = true;
        let v = colocar(vec![a, t("b", Some(400), "dos")], 0);
        // Sin cabecera ni fondo propio: lo de debajo es el papel.
        assert!(!textos(&v.fijas).contains(&"Comentarios".to_string()));
        assert!(matches!(v.fijas[0], Pieza::Caja { color, .. } if color == Tema::oscuro().papel));
        assert!(!v.zonas.iter().any(|(a, _, _)| *a == Accion::CerrarPanel));
        let caja = |id: &str| v.tarjetas.iter().find(|(x, _)| x == id).unwrap().1;
        let (a, b) = (caja("a"), caja("b"));
        assert_eq!(b.x - a.x, ADELANTE_PX, "la elegida, hacia la izquierda");
        assert!(a.x >= PANEL.x && b.derecha() <= PANEL.derecha());
        // Cada tarjeta lleva su sombra debajo: algo mas grande que ella y de
        // otro color que la tarjeta y que el papel.
        let co = colores(&Tema::oscuro());
        let sombras = v
            .moviles
            .iter()
            .filter(|p| matches!(p, Pieza::Caja { caja, color, .. }
                if caja.an > b.an && *color != co.tarjeta && *color != Tema::oscuro().papel))
            .count();
        assert!(sombras >= 2 * 3, "{sombras}");
    }

    #[test]
    fn muchas_tarjetas_juntas_se_apilan_sin_solaparse() {
        let mut v_t: Vec<Tarjeta> = (0..5).map(|i| t(&format!("c{i}"), Some(150 + i * 8), "un comentario")).collect();
        v_t[2].activa = true;
        let v = colocar(v_t, 0);
        let mut cajas: Vec<Caja> = v.tarjetas.iter().map(|(_, c)| *c).collect();
        cajas.sort_by_key(|c| c.y);
        for par in cajas.windows(2) {
            assert!(par[0].abajo() <= par[1].y, "{:?} pisa {:?}", par[0], par[1]);
        }
        // La elegida, justo en su frase.
        assert_eq!(v.tarjetas.iter().find(|(x, _)| x == "c2").unwrap().1.y, 150 + 16);
    }

    #[test]
    fn en_el_cajon_hay_cabecera_para_cerrarlo_y_fondo_propio() {
        let v = colocar_en(vec![t("a", Some(200), "uno")], 0, true);
        assert!(textos(&v.fijas).contains(&"Comentarios".to_string()));
        let cerrar = v.zonas.iter().find(|(a, _, _)| *a == Accion::CerrarPanel).unwrap().1;
        assert_eq!(v.accion_en(cerrar.x + 2, cerrar.y + 2), Some(Accion::CerrarPanel));
        assert!(matches!(v.fijas[0], Pieza::Caja { color, .. } if color == colores(&Tema::oscuro()).fondo));
    }

    #[test]
    fn cada_tarjeta_va_a_la_altura_de_su_frase_sin_montarse() {
        let ys = apilar(&[(100, 50), (120, 40), (400, 30)], None, 10);
        assert_eq!(ys, [100, 160, 400]);
    }

    #[test]
    fn la_elegida_va_en_su_frase_y_aparta_a_las_demas() {
        // La segunda elegida: en su sitio; la primera sube y la tercera baja.
        let ys = apilar(&[(100, 50), (120, 40), (130, 30)], Some(1), 10);
        assert_eq!(ys, [60, 120, 170]);
        // Una elegida que no existe es como ninguna.
        assert_eq!(apilar(&[(5, 5)], Some(9), 10), [5]);
        assert!(apilar(&[], Some(0), 10).is_empty());
    }

    #[test]
    fn el_panel_vacio_dice_que_no_hay_comentarios_y_como_poner_uno() {
        let v = colocar(Vec::new(), 0);
        let textos = textos(&v.fijas);
        assert!(textos.contains(&"No hay comentarios".to_string()));
        assert!(textos.iter().any(|t| t.contains("Comentar")));
        // En una tarjeta como las demas, dentro del margen.
        assert!(v.fijas.iter().any(|p| matches!(p, Pieza::Borde { caja, .. } if caja.derecha() <= PANEL.derecha())));
        // En el cajon, ademas, su cruz para cerrarlo.
        let v = colocar_en(Vec::new(), 0, true);
        let cerrar = v.zonas.iter().find(|(a, _, _)| *a == Accion::CerrarPanel).unwrap().1;
        assert!(cerrar.derecha() <= PANEL.derecha());
    }

    #[test]
    fn los_sin_ancla_van_arriba_y_los_demas_debajo_siguiendo_al_texto() {
        let v = colocar(vec![t("a", Some(300), "en su frase"), t("s", None, "perdido")], 0);
        let caja = |id: &str| v.tarjetas.iter().find(|(x, _)| x == id).unwrap().1;
        let (a, s) = (caja("a"), caja("s"));
        assert!(s.y < v.zona_movil.y, "el suelto va en la parte quieta");
        assert_eq!(a.y, 300, "el anclado, a la altura de su frase");
        assert!(v.zona_movil.y > PANEL.y + CABECERA_PX);
    }

    #[test]
    fn una_frase_a_la_vista_no_deja_su_tarjeta_tapada_por_los_sin_ancla() {
        // La frase esta justo bajo la cabecera, donde van los sin ancla.
        let v = colocar(vec![t("a", Some(PANEL.y + 20), "uno"), t("s", None, "perdido")], 0);
        let a = v.tarjetas.iter().find(|(x, _)| x == "a").unwrap().1;
        assert!(a.y >= v.zona_movil.y, "entera a la vista");
    }

    #[test]
    fn cada_aparato_tiene_su_color() {
        assert_ne!(color_de("MOVI"), color_de("K7Q2"));
        assert_eq!(color_de("MOVI"), color_de("MOVI"));
    }

    #[test]
    fn una_tarjeta_fuera_de_la_vista_no_se_pulsa() {
        // Su frase se fue por arriba al bajar la nota: la tarjeta tambien.
        let v = colocar(vec![t("a", Some(-400), "arriba del todo")], 0);
        let a = v.tarjetas[0].1;
        assert!(a.abajo() < v.zona_movil.y);
        assert_eq!(v.accion_en(a.x + 5, v.zona_movil.y + 5), None);
    }

    #[test]
    fn la_tarjeta_elegida_tiene_resolver_mas_y_responder_y_las_otras_no() {
        let mut a = t("a", Some(200), "uno");
        a.activa = true;
        let v = colocar(vec![a, t("b", Some(260), "dos")], 0);
        let acciones: Vec<&Accion> = v.zonas.iter().map(|(a, _, _)| a).collect();
        assert!(acciones.contains(&&Accion::Resolver("a".into())));
        assert!(acciones.contains(&&Accion::Mas("a".into())));
        assert!(acciones.contains(&&Accion::Responder("a".into())));
        assert!(!acciones.contains(&&Accion::Resolver("b".into())));
        // Un clic en el cuerpo de la otra la elige.
        let b = v.tarjetas.iter().find(|(x, _)| x == "b").unwrap().1;
        assert_eq!(v.accion_en(b.x + 5, b.y + 5), Some(Accion::Tarjeta("b".into())));
        // Y la elegida no se monta en la otra.
        let a = v.tarjetas.iter().find(|(x, _)| x == "a").unwrap().1;
        assert!(a.abajo() <= b.y);
    }

    #[test]
    fn escribir_pone_el_cuadro_y_sus_botones_dentro_de_la_tarjeta() {
        let mut a = t("a", Some(200), "uno");
        a.activa = true;
        a.compositor = Some(Compositor::Responder);
        let v = colocar(vec![a], 0);
        let tarjeta = v.tarjetas[0].1;
        let c = v.compositor_visible().unwrap();
        assert!(c.y > tarjeta.y && c.abajo() < tarjeta.abajo());
        let enviar = v.zonas.iter().find(|(a, _, _)| *a == Accion::Enviar).unwrap().1;
        assert_eq!(v.accion_en(enviar.x + 3, enviar.y + 3), Some(Accion::Enviar));
        assert!(v.zonas.iter().any(|(a, _, _)| *a == Accion::Cancelar));
        // Sin la pastilla de responder: ya se esta respondiendo.
        assert!(!v.zonas.iter().any(|(a, _, _)| matches!(a, Accion::Responder(_))));
    }

    #[test]
    fn los_resueltos_ocultos_se_cuentan_y_se_ven_con_un_clic() {
        let v = colocar(vec![t("a", Some(200), "uno")], 2);
        let texto = v.fijas.iter().find_map(|p| match p {
            Pieza::Texto { texto, .. } if texto.starts_with("Ver resueltos") => Some(texto.clone()),
            _ => None,
        });
        assert_eq!(texto.as_deref(), Some("Ver resueltos (2)"));
        // En el margen, solo el enlace; en el cajon, tambien el filtro de
        // su cabecera.
        assert_eq!(v.zonas.iter().filter(|(a, _, _)| *a == Accion::Filtro).count(), 1);
        let v = colocar_en(vec![t("a", Some(200), "uno")], 2, true);
        assert_eq!(v.zonas.iter().filter(|(a, _, _)| *a == Accion::Filtro).count(), 2);
        // Caso negativo: sin resueltos no hay enlace.
        let v = colocar(vec![t("a", Some(200), "uno")], 0);
        assert!(!v.zonas.iter().any(|(a, _, _)| *a == Accion::Filtro));
    }

    #[test]
    fn el_panel_va_encima_del_margen_del_papel_solo_si_esta_abierto() {
        let mut m = Marco { cuerpo: Caja { x: 0, y: 82, an: 1000, al: 600 }, ..Default::default() };
        ABIERTO.with(|a| a.set(false));
        assert_eq!(recortar(&mut m, 1.0), None);
        assert_eq!(m.cuerpo.an, 1000);
        ABIERTO.with(|a| a.set(true));
        let p = recortar(&mut m, 1.0).unwrap();
        // Encima del margen del papel, sin quitarle nada, y antes de la barra.
        assert_eq!((p.x, p.an, m.cuerpo.an), (666, 320, 1000));
        assert!(!CAJON.with(Cell::get));
        assert_eq!(reserva(), 320);
        // Estrecha: las tarjetas van en el cajon, encima del texto, y el
        // papel no pierde nada.
        let mut m = Marco { cuerpo: Caja { x: 0, y: 82, an: 600, al: 600 }, ..Default::default() };
        let p = recortar(&mut m, 1.0).unwrap();
        assert_eq!((p.x, p.an, m.cuerpo.an), (280, 320, 600));
        assert!(CAJON.with(Cell::get));
        // Muy estrecha: el cajon deja ver un poco de texto a su izquierda.
        let mut m = Marco { cuerpo: Caja { x: 0, y: 82, an: 300, al: 600 }, ..Default::default() };
        let p = recortar(&mut m, 1.0).unwrap();
        assert_eq!((p.x, p.an), (48, 252));
        ABIERTO.with(|a| a.set(false));
        let mut m = Marco { cuerpo: Caja { x: 0, y: 82, an: 600, al: 600 }, ..Default::default() };
        assert_eq!(recortar(&mut m, 1.0), None);
        assert!(!CAJON.with(Cell::get));
    }

    #[test]
    fn la_fecha_es_corta_y_lleva_el_ano_solo_si_no_es_este() {
        let meses: Vec<String> = "ene feb mar abr may jun jul ago sept oct nov dic".split(' ').map(String::from).collect();
        // 2026-09-30 19:05 UTC; Lima va 5 h por detras.
        let cuando = 1_790_795_100_000;
        let lima = -5 * 3_600_000;
        assert_eq!(fecha_corta(cuando, lima, cuando, &meses), "30 sept, 14:05");
        assert_eq!(fecha_corta(cuando - 365 * 86_400_000, lima, cuando, &meses), "30 sept 2025");
    }
}
