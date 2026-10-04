//! **Lo que se pinta encima del texto**: las fotos de la nota, y lo que en
//! un editor como el de Claude no son letras —la casilla de una tarea, la
//! raya de un separador, la barra de una cita— y la barra de desplazamiento
//! fina.
//!
//! # Las fotos
//!
//! En el Markdown una foto es un renglon `![texto](ruta)`, como la escribe
//! el movil (`Bloques.plantilla(IMAGEN)`), y el texto del control sigue
//! siendo ese renglon letra por letra: nada de objetos incrustados que
//! habria que volver a traducir al guardar.
//!
//! Para que se vea, el parrafo de ese renglon lleva **aire encima** del alto
//! de la foto (`dySpaceBefore`, puesto con el formato, sin tocar el texto
//! ni el deshacer) y la foto se pinta en ese hueco despues de que el
//! `RichEdit` pinte lo suyo. Fuera del renglon del cursor el texto del
//! renglon se esconde, como las demas marcas: queda la foto sola; con el
//! cursor en el, se ve debajo su pie, la foto lleva un borde de color y su
//! **asa** abajo a la derecha para cambiarle el ancho (el ancho va en el
//! texto, `![x|320](…)`, ver `pixpin_docs::md_imagen`).
//!
//! Una pagina viva cuya hoja ya no esta lleva un **aviso** encima: se sigue
//! viendo la ultima copia, que es lo que ve el movil.
//!
//! Las fotos se leen una vez, ya encogidas a lo que se ensenan (nada de
//! guardar en memoria una foto de 12 Mpx para ensenarla a 720 px).
//!
//! # Donde va cada cosa: del texto de AHORA
//!
//! El sitio de cada foto y de cada casilla se saca **al pintar**, del texto
//! que tiene el control en ese momento, no del que habia la ultima vez que
//! se puso el formato (60 ms despues de escribir): con una posicion vieja,
//! escribir un renglon por encima de una pagina viva la pintaba un renglon
//! mas arriba, encima del texto, y se quedaba ahi (lo vio el usuario el
//! 30-sep: «se pone sobre el texto»).
//!
//! # Sin parpadeo
//!
//! Antes de que el control pinte, los huecos de lo que va encima se le
//! quitan de lo que tiene que pintar (`reservar`), y luego cada cosa se
//! compone en memoria con el papel debajo y se copia de una vez: el control
//! no borra la foto para que se vuelva a pintar encima.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Controls::RichEdit::{EM_GETSCROLLPOS, EM_GETTEXTEX, EM_GETTEXTLENGTHEX};
use windows::Win32::UI::Controls::RichEdit::{
    GETTEXTEX, GETTEXTLENGTHEX, GT_RAWTEXT, GTL_NUMCHARS, GTL_PRECISE,
};
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, SendMessageW};
use windows::core::HSTRING;

use pixpin_docs::{md_edicion, md_vivo};

use crate::disposicion::Caja;
use crate::pintor::{Icono, Pintor};
use crate::tema::{Rgb, Tema, bgr};

/// Una foto lista para pintar.
pub struct Foto {
    pub ancho: i32,
    pub alto: i32,
    /// Lo que mide el fichero: con esto el asa calcula el tamano exacto
    /// que tendra al soltarla (sin rebote, 1-oct).
    pub original: (u32, u32),
    mapa: HBITMAP,
}

impl Drop for Foto {
    fn drop(&mut self) {
        // SAFETY: mapa de bits propio, creado en `cargar`.
        unsafe {
            let _ = DeleteObject(HGDIOBJ(self.mapa.0));
        }
    }
}

impl Foto {
    /// Una foto ya pintada en memoria (una tarjeta de un incrustado, ver
    /// `incrustados::pintar`): BGRA de 32 bits, opaco. Se queda con el mapa.
    pub(crate) fn de_mapa(mapa: HBITMAP, ancho: i32, alto: i32) -> Foto {
        Foto {
            ancho,
            alto,
            original: (ancho.max(0) as u32, alto.max(0) as u32),
            mapa,
        }
    }
}

/// El aire de arriba y de abajo de una foto, en pixeles a 96 ppp.
pub const AIRE_PX: i32 = 10;
/// Lo mas alta que se ensena una foto a su tamano, a 96 ppp.
pub const ALTO_MAXIMO_PX: i32 = 440;
/// Lo mas alta que se ensena una foto con el ancho puesto a mano: quien la
/// agranda quiere verla grande (una hoja de un lienzo alta, entera), pero
/// no de diez pantallas; y el aire de encima del renglon tiene su tope en
/// el control (unos 1580 puntos).
pub const ALTO_CON_ANCHO_PX: i32 = 2000;
/// El lado del asa de cambiar el tamano, a 96 ppp.
pub const ASA_PX: i32 = 12;
/// El ancho de la barra de desplazamiento fina (y el de cogerla).
pub const BARRA_PX: i32 = 6;
pub const BARRA_COGER_PX: i32 = 14;

/// Las fotos leidas, por la ruta tal como va en el Markdown y el tamano en
/// que caben. `None` si no se pudo (no esta, o no es una foto que se sepa
/// leer): ese renglon se queda a la vista como texto.
#[derive(Default)]
pub struct Fotos {
    leidas: HashMap<(String, (i32, i32), bool), Option<Rc<Foto>>>,
}

impl Fotos {
    /// La foto de `ruta` en lo que cabe en `max`. Con `llenar` (un ancho
    /// puesto a mano) se agranda hasta ese ancho si es pequena: el asa tiene
    /// que poder agrandar una hoja de un lienzo con poco dibujo.
    pub fn foto(
        &mut self,
        ruta: &str,
        resolver: &dyn Fn(&str) -> Option<PathBuf>,
        max: (i32, i32),
        llenar: bool,
    ) -> Option<Rc<Foto>> {
        self.leidas
            .entry((ruta.to_string(), max, llenar))
            .or_insert_with(|| {
                resolver(ruta)
                    .and_then(|r| cargar(&r, max, llenar))
                    .map(Rc::new)
            })
            .clone()
    }

    /// Se olvida de una ruta: su fichero cambio (una pagina viva que se
    /// volvio a pintar) o aparecio (una que aun no estaba).
    pub fn olvidar(&mut self, ruta: &str) {
        self.leidas.retain(|(r, _, _), _| r != ruta);
    }
}

/// El tamano en que cabe `(an, al)` dentro de `max` sin deformarla: sin
/// agrandarla, o con `llenar`, agrandandola hasta el ancho de `max` si cabe
/// de alto.
pub fn encajar_con(an: u32, al: u32, max: (i32, i32), llenar: bool) -> (i32, i32) {
    if an == 0 || al == 0 {
        return (0, 0);
    }
    let k = (max.0 as f64 / an as f64).min(max.1 as f64 / al as f64);
    let k = if llenar { k } else { k.min(1.0) };
    (
        ((an as f64 * k).round() as i32).max(1),
        ((al as f64 * k).round() as i32).max(1),
    )
}

/// Lo que puede ocupar una foto: la columna y 440 de alto, o el ancho que
/// le puso el usuario (en pixeles a 96 ppp) con mas alto. `esc` es la
/// escala de la pantalla.
pub fn caja_maxima(ancho: Option<u32>, columna_px: i32, esc: f32) -> (i32, i32) {
    match ancho {
        Some(a) => (
            ((a as i32).min(columna_px) as f32 * esc) as i32,
            (ALTO_CON_ANCHO_PX as f32 * esc) as i32,
        ),
        None => (
            (columna_px as f32 * esc) as i32,
            (ALTO_MAXIMO_PX as f32 * esc) as i32,
        ),
    }
}

fn cargar(ruta: &std::path::Path, max: (i32, i32), llenar: bool) -> Option<Foto> {
    let img = pixpin_codec::imagen::cargar(ruta).ok()?;
    let original = (img.ancho, img.alto);
    let (an, al) = encajar_con(img.ancho, img.alto, max, llenar);
    let img = if (an as u32, al as u32) != (img.ancho, img.alto) {
        pixpin_codec::imagen::redimensionar(img, an as u32, al as u32).ok()?
    } else {
        img
    };
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: an,
            biHeight: -al,
            biPlanes: 1,
            biBitCount: 32,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
    // SAFETY: el DIB se crea del tamano justo y se escribe dentro de el;
    // lo suelta `Drop`.
    unsafe {
        let mapa = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(mapa.0));
            return None;
        }
        let destino = std::slice::from_raw_parts_mut(bits as *mut u8, (an * al * 4) as usize);
        // RGBA recto a BGRA premultiplicado, que es lo que pide AlphaBlend.
        for (d, s) in destino.chunks_exact_mut(4).zip(img.pixeles.chunks_exact(4)) {
            let a = s[3] as u32;
            d[0] = (s[2] as u32 * a / 255) as u8;
            d[1] = (s[1] as u32 * a / 255) as u8;
            d[2] = (s[0] as u32 * a / 255) as u8;
            d[3] = s[3];
        }
        Some(Foto {
            ancho: an,
            alto: al,
            original,
            mapa,
        })
    }
}

/// Una foto colocada en la nota: donde empieza su renglon, lo que ocupa y
/// donde se pinto la ultima vez (para el raton). El renglon y la posicion
/// se vuelven a mirar en el texto cada vez que se pinta.
pub struct Puesta {
    pub pos: usize,
    pub linea: usize,
    pub ruta: String,
    pub foto: Rc<Foto>,
    /// El renglon tiene el cursor: borde y asa a la vista.
    pub activa: bool,
    /// Un aviso encima (una pagina viva sin su hoja).
    pub aviso: Option<String>,
    pub caja: Cell<RECT>,
}

/// Los colores de lo que se pinta encima de las fotos.
#[derive(Debug, Clone, Copy, Default)]
pub struct Colores {
    pub acento: COLORREF,
    pub aviso_fondo: COLORREF,
    pub aviso_texto: COLORREF,
}

/// Lo que hace falta para pintar lo que no son fotos: el pintor de formas,
/// los colores del tema y las medidas de la letra elegida.
#[derive(Clone)]
pub struct Adornos {
    pub pintor: Rc<Pintor>,
    pub tema: Tema,
    /// Donde empieza la columna de texto dentro del rectangulo de escribir.
    pub margen_px: i32,
    /// Lo alto de un renglon del cuerpo y lo que se sangra una lista.
    pub renglon_px: i32,
    pub sangria_px: i32,
}

thread_local! {
    /// Las fotos que hay que pintar encima del `RichEdit` (las pone el
    /// formato, las lee su pintado).
    pub static PUESTAS: RefCell<Vec<Puesta>> = const { RefCell::new(Vec::new()) };
    pub static PPP: Cell<i32> = const { Cell::new(96) };
    pub static COLORES: Cell<Colores> = Cell::new(Colores::default());
    /// Un cambio de tamano a medias: que foto y a que ancho y alto en
    /// pantalla (los que tendra al soltarla, ver `editor::fotos`).
    pub static ARRASTRE: Cell<Option<(usize, i32, i32)>> = const { Cell::new(None) };
    /// Lo de pintar casillas, rayas, citas y la barra (lo pone el editor).
    pub static ADORNOS: RefCell<Option<Adornos>> = const { RefCell::new(None) };
    /// Donde quedo la barra de desplazamiento (pista y asidero), para el
    /// raton, y si el raton esta encima (se ensancha).
    pub static BARRA: Cell<Option<(RECT, RECT)>> = const { Cell::new(None) };
    pub static BARRA_ENCIMA: Cell<bool> = const { Cell::new(false) };
}

const EM_POSFROMCHAR: u32 = 0x00D6;
const EM_CHARFROMPOS: u32 = 0x00D7;
const EM_GETRECT: u32 = 0x00B2;

fn escala(v: i32) -> i32 {
    v * PPP.with(|p| p.get()) / 96
}

fn enviar(h: HWND, m: u32, w: usize, l: isize) -> isize {
    // SAFETY: mensajes a una ventana viva de este hilo; los punteros los
    // presta quien llama y viven durante la llamada.
    unsafe { SendMessageW(h, m, Some(WPARAM(w)), Some(LPARAM(l))).0 }
}

/// El texto del control tal cual (con `\r` de salto: las posiciones son las
/// mismas que con `\n`).
pub fn texto_del_control(edit: HWND) -> String {
    let largo = GETTEXTLENGTHEX {
        flags: GTL_NUMCHARS | GTL_PRECISE,
        codepage: 1200,
    };
    let n = enviar(edit, EM_GETTEXTLENGTHEX, &largo as *const _ as usize, 0).max(0) as usize;
    let mut buf = vec![0u16; n + 1];
    let pedido = GETTEXTEX {
        cb: ((n + 1) * 2) as u32,
        flags: GT_RAWTEXT,
        codepage: 1200,
        ..Default::default()
    };
    let copiados = enviar(
        edit,
        EM_GETTEXTEX,
        &pedido as *const _ as usize,
        buf.as_mut_ptr() as isize,
    )
    .max(0) as usize;
    buf.truncate(copiados.min(n));
    String::from_utf16_lossy(&buf)
}

fn punto_de(edit: HWND, pos: usize) -> POINT {
    let mut p = POINT::default();
    enviar(
        edit,
        EM_POSFROMCHAR,
        &mut p as *mut _ as usize,
        pos as isize,
    );
    p
}

/// Una cosa que se pinta encima, ya colocada en el control.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Que {
    /// La foto `i` de [`PUESTAS`] en `(x, y)` de `an` x `al` (al cambiarle
    /// el tamano, a otro que el suyo).
    Foto {
        i: usize,
        x: i32,
        y: i32,
        an: i32,
        al: i32,
    },
    Casilla {
        hecha: bool,
        caja: Caja,
    },
    Raya {
        x0: i32,
        x1: i32,
        y: i32,
    },
    Cita {
        x: i32,
        y0: i32,
        y1: i32,
    },
    Barra {
        asidero: RECT,
    },
}

/// La pieza y el rectangulo que ocupa (lo que se compone de una vez).
type Pieza = (Que, RECT);

fn rect(x: i32, y: i32, an: i32, al: i32) -> RECT {
    RECT {
        left: x,
        top: y,
        right: x + an,
        bottom: y + al,
    }
}

fn caja_de(r: RECT) -> Caja {
    Caja {
        x: r.left,
        y: r.top,
        an: r.right - r.left,
        al: r.bottom - r.top,
    }
}

/// Lo que mide la nota entera (de alto, en pixeles, desde arriba del todo)
/// y lo que se ve: para la barra.
pub fn medidas(edit: HWND) -> (i32, i32) {
    let texto = texto_del_control(edit);
    medidas_con(edit, &texto)
}

fn medidas_con(edit: HWND, texto: &str) -> (i32, i32) {
    let mut dentro = RECT::default();
    let mut cliente = RECT::default();
    enviar(edit, EM_GETRECT, 0, &mut dentro as *mut _ as isize);
    // SAFETY: rectangulo de una ventana propia.
    unsafe {
        let _ = GetClientRect(edit, &mut cliente);
    }
    let mut scroll = POINT::default();
    enviar(edit, EM_GETSCROLLPOS, 0, &mut scroll as *mut _ as isize);
    let total = texto.encode_utf16().count();
    let renglon = ADORNOS
        .with(|a| a.borrow().as_ref().map(|a| a.renglon_px))
        .unwrap_or(escala(24));
    let alto_nota =
        punto_de(edit, total).y + scroll.y + renglon + (cliente.bottom - dentro.bottom).max(0);
    (cliente.bottom - cliente.top, alto_nota)
}

/// **Coloca todo lo que va encima** con el texto de ahora: el renglon de
/// cada foto, cada casilla, raya y cita, y la barra de desplazamiento.
fn colocar(edit: HWND) -> Vec<Pieza> {
    let texto = texto_del_control(edit);
    let ls = md_vivo::lineas(&texto);
    let mut dentro = RECT::default();
    let mut cliente = RECT::default();
    enviar(edit, EM_GETRECT, 0, &mut dentro as *mut _ as isize);
    // SAFETY: rectangulo de una ventana propia.
    unsafe {
        let _ = GetClientRect(edit, &mut cliente);
    }
    // Lo que se ve del papel empieza tras el hueco de las tablas anchas
    // (`tabla_ancha`), que esta a la izquierda, fuera de la ventana.
    let izq = dentro.left + crate::tabla_ancha::hueco();
    let mut sal = Vec::new();
    let aire = escala(AIRE_PX);
    let arrastre = ARRASTRE.with(|a| a.get());
    // Las fotos: cada puesta, en el renglon de su foto en el texto de ahora
    // (la primera con su ruta desde la anterior: dos copias de la misma
    // foto siguen cada una en lo suyo).
    // Con los incrustados, que se ponen encima igual (`incrustados`).
    let ahora = crate::incrustados::encima(&texto);
    PUESTAS.with(|p| {
        let mut p = p.borrow_mut();
        let mut desde = 0;
        for (i, pu) in p.iter_mut().enumerate() {
            let Some(k) = (desde..ahora.len()).find(|&k| ahora[k].1 == pu.ruta) else {
                pu.caja.set(RECT::default());
                continue;
            };
            desde = k + 1;
            let n = ahora[k].0;
            let Some(l) = ls.get(n) else { continue };
            pu.linea = n;
            pu.pos = l.desde;
            let pt = punto_de(edit, l.desde);
            let (an, al) = match arrastre {
                Some((j, a, b)) if j == i => (a, b),
                _ => (pu.foto.ancho, pu.foto.alto),
            };
            // La posicion de un renglon es la de lo alto de su parrafo, con
            // el aire de encima dentro: ahi va la foto.
            // Centrada en la columna de texto (corrida si las tarjetas de los
            // comentarios ocupan el margen).
            let ppp = PPP.with(|p| p.get());
            let (ci, cd) = (
                izq + crate::tabla_ancha::sobra_px(ppp),
                dentro.right - crate::tabla_ancha::sobra_der_px(ppp),
            );
            let (x, y) = ((ci + cd - an) / 2, pt.y + aire);
            pu.caja.set(rect(x, y, an, al));
            // Con el borde y el asa, que salen un poco por fuera.
            let g = escala(3);
            sal.push((
                Que::Foto { i, x, y, an, al },
                rect(x - g, y - g, an + 2 * g, al + 2 * g),
            ));
        }
    });
    let Some(ad) = ADORNOS.with(|a| a.borrow().clone()) else {
        return sal;
    };
    // La sangria de lo escrito ya lleva el hueco (no se suma otra vez); a la
    // derecha solo el margen de la columna.
    let col_izq = dentro.left + ad.margen_px;
    let col_der = dentro.right - crate::tabla_ancha::sobra_der_px(PPP.with(|p| p.get()));
    let tipos = md_edicion::renglones(&texto);
    let tramos = md_vivo::analizar(&texto);
    let lado = (ad.renglon_px * 11 / 20).max(escala(10));
    let mut cita: Option<(i32, i32)> = None;
    let x_cita = col_izq + ad.sangria_px / 3;
    for (n, l) in ls.iter().enumerate() {
        let es_cita = tramos
            .iter()
            .any(|t| t.linea == n && t.estilo == md_vivo::Estilo::Cita);
        if es_cita {
            let y = punto_de(edit, l.desde).y;
            cita = Some(match cita {
                Some((y0, _)) => (y0, y + ad.renglon_px),
                None => (y, y + ad.renglon_px),
            });
        } else if let Some((y0, y1)) = cita.take() {
            sal.push((
                Que::Cita { x: x_cita, y0, y1 },
                rect(x_cita, y0, escala(3), y1 - y0),
            ));
        }
        match tipos.get(n) {
            Some(md_edicion::Renglon::Texto(m)) => {
                if let Some(t) = tramos
                    .iter()
                    .find(|t| t.linea == n && matches!(t.estilo, md_vivo::Estilo::Casilla { .. }))
                {
                    let hecha = matches!(t.estilo, md_vivo::Estilo::Casilla { hecha: true });
                    let pt = punto_de(edit, (l.desde + m).min(l.hasta));
                    let caja = Caja {
                        x: pt.x - escala(8) - lado,
                        // A la altura de las letras, que van algo por encima del medio
                        // del renglon (el aire del interlineado va debajo).
                        y: pt.y + (ad.renglon_px - lado) / 2 - ad.renglon_px / 9,
                        an: lado,
                        al: lado,
                    };
                    let r = rect(caja.x - 1, caja.y - 1, caja.an + 2, caja.al + 2);
                    sal.push((Que::Casilla { hecha, caja }, r));
                }
            }
            Some(md_edicion::Renglon::Bloque)
                if tramos
                    .iter()
                    .any(|t| t.linea == n && t.estilo == md_vivo::Estilo::Regla) =>
            {
                let y = punto_de(edit, l.desde).y + ad.renglon_px / 2;
                sal.push((
                    Que::Raya {
                        x0: col_izq,
                        x1: col_der,
                        y,
                    },
                    rect(col_izq, y - 1, col_der - col_izq, 3),
                ));
            }
            _ => {}
        }
    }
    if let Some((y0, y1)) = cita {
        sal.push((
            Que::Cita { x: x_cita, y0, y1 },
            rect(x_cita, y0, escala(3), y1 - y0),
        ));
    }
    // La barra de desplazamiento fina, de lo que mide toda la nota.
    let mut scroll = POINT::default();
    enviar(edit, EM_GETSCROLLPOS, 0, &mut scroll as *mut _ as isize);
    let (visible, alto_nota) = medidas_con(edit, &texto);
    let b = barra(
        visible,
        alto_nota,
        scroll.y,
        cliente.right,
        BARRA_ENCIMA.with(|b| b.get()),
    );
    BARRA.with(|x| x.set(b));
    if let Some((pista, asidero)) = b {
        sal.push((Que::Barra { asidero }, pista));
    }
    sal
}

/// **La barra fina**: la pista (a la derecha, de arriba abajo) y el
/// asidero, proporcional a lo que se ve de la nota. `None` si cabe entera.
pub fn barra(
    visible: i32,
    alto_nota: i32,
    desplazado: i32,
    derecha: i32,
    encima: bool,
) -> Option<(RECT, RECT)> {
    if visible <= 0 || alto_nota <= visible {
        return None;
    }
    let ancho = escala(if encima { BARRA_PX + 4 } else { BARRA_PX });
    let coger = escala(BARRA_COGER_PX);
    let pista = rect(derecha - coger, 0, coger, visible);
    let margen = escala(3);
    let largo = visible - 2 * margen;
    let alto = (largo as i64 * visible as i64 / alto_nota as i64).max(escala(28) as i64) as i32;
    let alto = alto.min(largo);
    let recorrido = (largo - alto).max(0);
    let y = margen
        + (desplazado.max(0) as i64 * recorrido as i64 / (alto_nota - visible) as i64)
            .min(recorrido as i64) as i32;
    let asidero = rect(derecha - margen - ancho, y, ancho, alto);
    Some((pista, asidero))
}

/// A que altura de la nota lleva un asidero cuyo borde de arriba esta en
/// `y` (la cuenta de [`barra`] al reves): para arrastrarlo.
pub fn desplazado_de(visible: i32, alto_nota: i32, y_asidero: i32, alto_asidero: i32) -> i32 {
    let margen = escala(3);
    let recorrido = (visible - 2 * margen - alto_asidero).max(1);
    let y = (y_asidero - margen).clamp(0, recorrido);
    (y as i64 * (alto_nota - visible).max(0) as i64 / recorrido as i64) as i32
}

/// Pinta lo de encima en `hdc` (el de la ventana del control, o el de una
/// muestra): cada cosa compuesta en memoria con el papel debajo y copiada
/// de una vez.
pub fn pintar_encima(edit: HWND, hdc: HDC) {
    let piezas = colocar(edit);
    if piezas.is_empty() {
        return;
    }
    let mut cliente = RECT::default();
    // SAFETY: rectangulo de una ventana propia.
    unsafe {
        let _ = GetClientRect(edit, &mut cliente);
    }
    let ad = ADORNOS.with(|a| a.borrow().clone());
    let papel = ad.as_ref().map(|a| a.tema.papel);
    let colores = COLORES.with(|c| c.get());
    PUESTAS.with(|p| {
        let puestas = p.borrow();
        for (que, r) in &piezas {
            let r = RECT {
                left: r.left.max(cliente.left),
                top: r.top.max(cliente.top),
                right: r.right.min(cliente.right),
                bottom: r.bottom.min(cliente.bottom),
            };
            if r.right <= r.left || r.bottom <= r.top {
                continue;
            }
            componer(hdc, r, papel, |dc, dx, dy| match *que {
                Que::Foto { i, x, y, an, al } => {
                    if let Some(pu) = puestas.get(i) {
                        pintar_foto(dc, pu, (x + dx, y + dy, an, al), colores);
                    }
                }
                _ => {
                    if let Some(ad) = &ad {
                        pintar_adorno(dc, ad, que, dx, dy, (r.right - r.left, r.bottom - r.top));
                    }
                }
            });
        }
    });
}

/// Compone en memoria lo que va en `r` (con el papel debajo, si se sabe) y
/// lo copia a `hdc`. `dibujar` recibe el DC de memoria y lo que hay que
/// sumar a las coordenadas del control para caer en el.
fn componer(hdc: HDC, r: RECT, papel: Option<Rgb>, dibujar: impl FnOnce(HDC, i32, i32)) {
    let (an, al) = (r.right - r.left, r.bottom - r.top);
    // SAFETY: DC y mapa de bits propios, creados y soltados aqui; el de
    // destino es prestado.
    unsafe {
        let mem = CreateCompatibleDC(Some(hdc));
        let mapa = CreateCompatibleBitmap(hdc, an, al);
        let viejo = SelectObject(mem, HGDIOBJ(mapa.0));
        match papel {
            Some(c) => rellenar(mem, rect(0, 0, an, al), COLORREF(bgr(c))),
            // Sin tema (no deberia pasar): lo que ya hay debajo.
            None => {
                let _ = BitBlt(mem, 0, 0, an, al, Some(hdc), r.left, r.top, SRCCOPY);
            }
        }
        dibujar(mem, -r.left, -r.top);
        let _ = BitBlt(hdc, r.left, r.top, an, al, Some(mem), 0, 0, SRCCOPY);
        SelectObject(mem, viejo);
        let _ = DeleteObject(HGDIOBJ(mapa.0));
        let _ = DeleteDC(mem);
    }
}

fn pintar_foto(dc: HDC, pu: &Puesta, (x, y, an, al): (i32, i32, i32, i32), colores: Colores) {
    let f = &pu.foto;
    // SAFETY: DC de memoria propio y el mapa de la foto, vivo mientras ella.
    unsafe {
        let memoria = CreateCompatibleDC(Some(dc));
        let viejo = SelectObject(memoria, HGDIOBJ(f.mapa.0));
        let mezcla = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        // Al cambiarle el tamano se estira la que hay: al soltar se lee a
        // su tamano nuevo, nitida.
        let _ = AlphaBlend(dc, x, y, an, al, memoria, 0, 0, f.ancho, f.alto, mezcla);
        SelectObject(memoria, viejo);
        let _ = DeleteDC(memoria);
    }
    let caja = rect(x, y, an, al);
    if pu.activa {
        // Por fuera de la foto, que se vea sea del color que sea.
        let g = escala(3);
        let fuera = RECT {
            left: caja.left - g,
            top: caja.top - g,
            right: caja.right + g,
            bottom: caja.bottom + g,
        };
        marco(dc, fuera, colores.acento, g);
        let lado = escala(ASA_PX);
        let asa = RECT {
            left: fuera.right - lado,
            top: fuera.bottom - lado,
            right: fuera.right,
            bottom: fuera.bottom,
        };
        rellenar(dc, asa, colores.acento);
        marco(dc, asa, colores.aviso_texto, escala(1));
    }
    if let Some(t) = &pu.aviso {
        aviso(dc, caja, t, colores);
    }
}

/// Una casilla, una raya, la barra de una cita o la de desplazamiento.
fn pintar_adorno(dc: HDC, ad: &Adornos, que: &Que, dx: i32, dy: i32, (an, al): (i32, i32)) {
    let t = &ad.tema;
    let mueve = |c: Caja| Caja {
        x: c.x + dx,
        y: c.y + dy,
        ..c
    };
    ad.pintor.formas(dc, rect(0, 0, an, al), |f| match *que {
        Que::Casilla { hecha, caja } => {
            let c = mueve(caja);
            if hecha {
                f.redondo(c, 4.0, t.acento);
                f.icono(
                    Icono::Hecho,
                    c,
                    (c.an as f32 / ad.pintor.escala) * 0.8,
                    t.papel,
                );
            } else {
                f.borde(c, 4.0, 1.5 * ad.pintor.escala, t.tenue);
            }
        }
        Que::Raya { x0, x1, y } => f.raya(x0 + dx, y + dy, x1 + dx, y + dy, t.apagado),
        Que::Cita { x, y0, y1 } => f.redondo(
            Caja {
                x: x + dx,
                y: y0 + dy,
                an: escala(3),
                al: y1 - y0,
            },
            1.5,
            t.raya,
        ),
        Que::Barra { asidero } => f.redondo(
            mueve(caja_de(asidero)),
            3.0,
            if t.oscuro { 0x4a4a48 } else { 0xc4c4c0 },
        ),
        Que::Foto { .. } => {}
    });
}

/// Un rectangulo lleno.
fn rellenar(hdc: HDC, r: RECT, c: COLORREF) {
    // SAFETY: pincel propio, creado y soltado aqui, sobre un DC prestado.
    unsafe {
        let pincel = CreateSolidBrush(c);
        FillRect(hdc, &r, pincel);
        let _ = DeleteObject(HGDIOBJ(pincel.0));
    }
}

/// Un borde de `grueso` pixeles por dentro de `r`.
fn marco(hdc: HDC, r: RECT, c: COLORREF, grueso: i32) {
    let g = grueso.max(1);
    for b in [
        RECT {
            bottom: r.top + g,
            ..r
        },
        RECT {
            top: r.bottom - g,
            ..r
        },
        RECT {
            right: r.left + g,
            ..r
        },
        RECT {
            left: r.right - g,
            ..r
        },
    ] {
        rellenar(hdc, b, c);
    }
}

/// El aviso en una franja abajo de la foto.
fn aviso(hdc: HDC, caja: RECT, texto: &str, c: Colores) {
    let alto = escala(26);
    let franja = RECT {
        top: (caja.bottom - alto).max(caja.top),
        ..caja
    };
    rellenar(hdc, franja, c.aviso_fondo);
    // SAFETY: letra propia, creada y soltada aqui; el DC es prestado y se
    // deja como estaba.
    unsafe {
        let letra = CreateFontW(
            -escala(13),
            0,
            0,
            0,
            FW_SEMIBOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            &HSTRING::from("Segoe UI"),
        );
        let vieja = SelectObject(hdc, HGDIOBJ(letra.0));
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, c.aviso_texto);
        let mut r = RECT {
            left: franja.left + escala(10),
            right: franja.right - escala(10),
            ..franja
        };
        let mut t: Vec<u16> = texto.encode_utf16().collect();
        DrawTextW(
            hdc,
            &mut t,
            &mut r,
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
        SelectObject(hdc, vieja);
        let _ = DeleteObject(HGDIOBJ(letra.0));
    }
}

/// Que hay bajo un punto del control: la foto (por su posicion en
/// [`PUESTAS`]) y si es su asa.
pub fn tocar(x: i32, y: i32) -> Option<(usize, bool)> {
    let lado = escala(ASA_PX) + escala(4);
    PUESTAS.with(|p| {
        p.borrow().iter().enumerate().find_map(|(i, pu)| {
            let c = pu.caja.get();
            let dentro = x >= c.left && x < c.right && y >= c.top && y < c.bottom;
            // El asa solo con la foto elegida: sin borde no se ve, y
            // cogerla a ciegas cambiaria el tamano sin querer.
            let asa = pu.activa
                && x >= c.right - lado
                && x < c.right + escala(4)
                && y >= c.bottom - lado
                && y < c.bottom + escala(4);
            (dentro || asa).then_some((i, asa))
        })
    })
}

/// El renglon, la ruta y la caja de la foto `i`.
pub fn puesta(i: usize) -> Option<(usize, String, RECT, i32)> {
    PUESTAS.with(|p| {
        p.borrow()
            .get(i)
            .map(|pu| (pu.linea, pu.ruta.clone(), pu.caja.get(), pu.foto.ancho))
    })
}

/// La casilla bajo un punto del control, si hay una: su renglon (para
/// marcarla con un clic, como en el movil).
pub fn casilla_en(edit: HWND, x: i32, y: i32) -> Option<usize> {
    let texto = texto_del_control(edit);
    let ls = md_vivo::lineas(&texto);
    let holgura = escala(4);
    colocar(edit).into_iter().find_map(|(q, _)| match q {
        Que::Casilla { caja, .. }
            if x >= caja.x - holgura
                && x < caja.derecha() + holgura
                && y >= caja.y - holgura
                && y < caja.abajo() + holgura =>
        {
            let p = POINT {
                x: caja.derecha() + escala(12),
                y: caja.y + caja.al / 2,
            };
            let pos = enviar(edit, EM_CHARFROMPOS, 0, &p as *const _ as isize);
            Some(md_vivo::linea_de(&ls, pos.max(0) as usize))
        }
        _ => None,
    })
}

/// Pinta lo de encima ya, sobre la ventana: tras algo que el control
/// repinta por su cuenta sin `WM_PAINT` (escribir en el renglon de al lado).
pub fn repintar(edit: HWND) {
    let hay = PUESTAS.with(|p| !p.borrow().is_empty()) || ADORNOS.with(|a| a.borrow().is_some());
    if !hay {
        return;
    }
    // Solo lo de encima, directo: repintar la nota entera a cada tecla la
    // volvia lenta (el usuario, 1-oct: «el markdown esta super lagueado»).
    // SAFETY: DC de una ventana propia, pedido y devuelto aqui.
    unsafe {
        let dc = GetDC(Some(edit));
        pintar_encima(edit, dc);
        ReleaseDC(Some(edit), dc);
    }
}

/// **Antes de que el control pinte**: lo que va encima se le quita de lo
/// que tiene que pintar, para que no lo borre y se vuelva a pintar (el
/// parpadeo de las fotos al desplazarse o al escribir al lado).
pub fn reservar(edit: HWND) {
    for (_, r) in colocar(edit) {
        // SAFETY: rectangulo de una ventana propia.
        unsafe {
            let _ = ValidateRect(Some(edit), Some(&r));
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_foto_grande_se_encoge_sin_deformarse_y_una_chica_no_se_agranda() {
        assert_eq!(encajar_con(4000, 3000, (720, 440), false), (587, 440));
        assert_eq!(encajar_con(2000, 500, (720, 440), false), (720, 180));
        assert_eq!(encajar_con(100, 50, (720, 440), false), (100, 50));
        assert_eq!(encajar_con(0, 50, (720, 440), false), (0, 0));
    }

    #[test]
    fn con_el_ancho_puesto_a_mano_una_chica_se_agranda_hasta_el_sin_deformarse() {
        assert_eq!(encajar_con(100, 50, (400, 2000), true), (400, 200));
        // Caso negativo: sin pedirlo no se agranda, y nunca pasa del alto.
        assert_eq!(encajar_con(100, 50, (400, 2000), false), (100, 50));
        assert_eq!(encajar_con(100, 1000, (400, 2000), true), (200, 2000));
    }

    #[test]
    fn el_ancho_puesto_manda_y_no_pasa_de_la_columna() {
        assert_eq!(caja_maxima(None, 720, 1.0), (720, ALTO_MAXIMO_PX));
        assert_eq!(caja_maxima(Some(300), 720, 1.0), (300, ALTO_CON_ANCHO_PX));
        assert_eq!(
            caja_maxima(Some(300), 720, 2.0),
            (600, 2 * ALTO_CON_ANCHO_PX)
        );
        // Caso negativo: mas ancha que la columna no se sale.
        assert_eq!(caja_maxima(Some(5000), 720, 1.0).0, 720);
    }

    #[test]
    fn una_foto_que_no_esta_no_se_pinta_y_no_se_vuelve_a_buscar() {
        let mut f = Fotos::default();
        let veces = Cell::new(0);
        let resolver = |_: &str| {
            veces.set(veces.get() + 1);
            Some(PathBuf::from("C:\\no\\existe.png"))
        };
        assert!(f.foto("x.png", &resolver, (720, 440), false).is_none());
        assert!(f.foto("x.png", &resolver, (720, 440), false).is_none());
        assert_eq!(veces.get(), 1);
        // Olvidada, se vuelve a buscar (una pagina viva que ya se pinto).
        f.olvidar("x.png");
        assert!(f.foto("x.png", &resolver, (720, 440), false).is_none());
        assert_eq!(veces.get(), 2);
    }

    #[test]
    fn la_barra_fina_es_proporcional_y_no_sale_si_la_nota_cabe() {
        PPP.with(|p| p.set(96));
        assert_eq!(barra(600, 600, 0, 1000, false), None);
        assert_eq!(barra(600, 400, 0, 1000, false), None);
        let (pista, a) = barra(600, 1200, 0, 1000, false).unwrap();
        assert_eq!(pista.right, 1000);
        // Media nota a la vista: el asidero es media pista, arriba.
        assert_eq!(a.bottom - a.top, (600 - 6) / 2);
        assert_eq!(a.top, 3);
        // Al final de la nota, el asidero abajo del todo.
        let (_, a) = barra(600, 1200, 600, 1000, false).unwrap();
        assert_eq!(a.bottom, 600 - 3);
        // Y al reves: arrastrar el asidero hasta abajo es ir al final.
        assert_eq!(desplazado_de(600, 1200, a.top, a.bottom - a.top), 600);
        assert_eq!(desplazado_de(600, 1200, -50, a.bottom - a.top), 0);
        // Con una nota muy larga el asidero no se queda en una raya.
        let (_, a) = barra(600, 600_000, 0, 1000, false).unwrap();
        assert_eq!(a.bottom - a.top, 28);
    }
}
