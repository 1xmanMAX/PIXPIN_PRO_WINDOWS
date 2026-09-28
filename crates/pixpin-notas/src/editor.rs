//! **La ventana del editor de notas**: un marco con el `RichEdit` de Windows
//! dentro, y el formato del Markdown puesto encima mientras se escribe.
//!
//! # El formato, encima del texto
//!
//! El texto del control es el Markdown tal cual (ver `pixpin_docs::md_vivo`).
//! Tras cada cambio —con un respiro de 60 ms para no rehacerlo en cada letra
//! de una rafaga— se analiza entero y se pinta: titulos grandes, negrita,
//! codigo en monoespaciada con su fondo, casillas en color… y las marcas
//! (`#`, `**`, `[`, `](url)`) **ocultas** (`CFE_HIDDEN`) en todos los
//! renglones menos en el del cursor, donde se ven apagadas: el que escribe
//! ve lo que borra. Al moverse de renglon solo se repintan los dos
//! renglones implicados.
//!
//! El formato no ensucia el deshacer: se pinta con el deshacer suspendido
//! (`ITextDocument::Undo(tomSuspend)`), asi Ctrl+Z deshace letras y no
//! colores.
//!
//! # Quien hace que
//!
//! El procedimiento de la ventana solo apunta lo que pasa en una cola; el
//! bucle de mensajes, que tiene a mano lo de guardar, lo atiende. Asi
//! guardar no tiene que vivir en una variable global del hilo.

use std::cell::{Cell, RefCell};

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateSolidBrush, DeleteObject, GetDC, GetDeviceCaps, HBRUSH, HGDIOBJ, LOGPIXELSY, ReleaseDC,
};
use windows::Win32::UI::Controls::RichEdit::*;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SetFocus, VK_APPS, VK_CONTROL, VK_F10, VK_MENU, VK_RETURN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, Interface, PCWSTR, w};

use pixpin_docs::md_vivo::{self, Continuar, Estilo};


use pixpin_shell::colocacion::Colocacion;

/// Los textos de la ventana, ya traducidos.
#[derive(Debug, Clone, Default)]
pub struct Rotulos {
    pub nueva: String,
    pub sufijo: String,
    pub negrita: String,
    pub cursiva: String,
    pub tachado: String,
    pub codigo: String,
    pub enlace: String,
    pub titulo1: String,
    pub titulo2: String,
    pub titulo3: String,
    pub lista: String,
    pub numerada: String,
    pub casilla: String,
    pub marcar: String,
    pub cita: String,
    pub bloque: String,
    pub raya: String,
    pub formula: String,
    pub cortar: String,
    pub copiar: String,
    pub pegar: String,
    pub guardar: String,
    pub copia_md: String,
    pub tipo_md: String,
    pub no_guardada: String,
    /// Como se llama la tecla de mayusculas en los atajos del menu.
    pub mayus: String,
    /// Y la de Intro.
    pub intro: String,
}


pub struct Pedido {
    pub texto: String,
    pub rotulos: Rotulos,
    /// El nombre del `.md` si la nota es un fichero: va en el titulo.
    pub nombre_de_fichero: Option<String>,
    /// Donde tiene que nacer, si viene de un grupo de ventanas (H9).
    pub colocacion: Option<Colocacion>,
}

// ---------------------------------------------------------------------------
// Colores y letra: los del lector (`lector.rs`), que son los del modo noche
// del movil. En 0x00BBGGRR, que es lo que quiere el RichEdit.

const fn rgb(c: u32) -> COLORREF {
    COLORREF(((c & 0xff) << 16) | (c & 0xff00) | ((c >> 16) & 0xff))
}
const FONDO: COLORREF = rgb(0x121316);
const TEXTO: COLORREF = rgb(0xe4e2e6);
const APAGADO: COLORREF = rgb(0x6e6e78);
const TENUE: COLORREF = rgb(0x9a9aa2);
const FONDO_CODIGO: COLORREF = rgb(0x24242a);
const DORADO: COLORREF = rgb(0xe8c06a);
const AZUL: COLORREF = rgb(0x8ab4f8);
const ACENTO: COLORREF = rgb(0x1e88e5);
const VERDE: COLORREF = rgb(0x66bb6a);
const MORADO: COLORREF = rgb(0xc3a6ff);

/// Tamanos en veinteavos de punto (lo que mide el RichEdit). La letra de
/// base, 12 pt, es la de los 16 sp del movil a la distancia de un monitor.
const BASE: i32 = 240;
fn tamano_de_titulo(n: u8) -> i32 {
    match n {
        1 => 440,
        2 => 360,
        3 => 300,
        _ => 260,
    }
}
const CODIGO: i32 = 210;
/// Cuanto se sangra una cita o una lista, en veinteavos de punto.
const SANGRIA: i32 = 360;
/// Lo mas ancho que se deja la columna de texto, en pixeles a 96 ppp: una
/// linea mas larga se lee mal, como en el lector.
const COLUMNA: i32 = 820;

// ---------------------------------------------------------------------------
// Las ordenes que el procedimiento deja para el bucle

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Orden {
    Cerrar,
    Comando(u16),
    Cambio,
    Repintar,
    Autoguardar,
}

thread_local! {
    static COLA: RefCell<Vec<Orden>> = const { RefCell::new(Vec::new()) };
    static EDIT: Cell<isize> = const { Cell::new(0) };
    static PPP: Cell<i32> = const { Cell::new(96) };
}

fn apuntar(o: Orden) {
    COLA.with(|c| c.borrow_mut().push(o));
}

// Los comandos: los del menu y los de las teclas, que son los mismos.
const C_NEGRITA: u16 = 10;
const C_CURSIVA: u16 = 11;
const C_TACHADO: u16 = 12;
const C_CODIGO: u16 = 13;
const C_ENLACE: u16 = 14;
const C_T1: u16 = 15;
const C_T2: u16 = 16;
const C_T3: u16 = 17;
const C_LISTA: u16 = 18;
const C_NUMERADA: u16 = 19;
const C_CASILLA: u16 = 20;
const C_MARCAR: u16 = 21;
const C_CITA: u16 = 22;
const C_BLOQUE: u16 = 23;
const C_RAYA: u16 = 24;
const C_FORMULA: u16 = 25;
const C_CORTAR: u16 = 26;
const C_COPIAR: u16 = 27;
const C_PEGAR: u16 = 28;
const C_GUARDAR: u16 = 29;
const C_COPIA_MD: u16 = 30;
const C_CERRAR: u16 = 31;

const T_REPINTAR: usize = 1;
const T_AUTOGUARDAR: usize = 2;
/// Cada cuanto se guarda solo si hay cambios: si Windows se apaga con la
/// nota abierta, se pierde como mucho esto.
const MS_AUTOGUARDAR: u32 = 30_000;
const MS_RESPIRO: u32 = 60;

// Mensajes del EDIT de siempre que el RichEdit tambien entiende.
const EM_REPLACESEL: u32 = 0x00C2;
const EM_CHARFROMPOS: u32 = 0x00D7;
const EM_EMPTYUNDOBUFFER: u32 = 0x00CD;
const EM_SETRECT: u32 = 0x00B3;
const EN_CHANGE: usize = 0x0300;
const CF_UNICODETEXT: usize = 13;

fn enviar(h: HWND, m: u32, w: usize, l: isize) -> isize {
    // SAFETY: mensajes a una ventana propia de este hilo; los punteros que
    // viajan en `l` los presta quien llama y viven durante la llamada.
    unsafe { SendMessageW(h, m, Some(WPARAM(w)), Some(LPARAM(l))).0 }
}

fn ancho_nulo(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ---------------------------------------------------------------------------
// Leer y escribir el texto del control

/// El texto, con `\n` donde el control tiene `\r`: las posiciones no cambian
/// (los dos son una unidad).
fn leer(edit: HWND) -> String {
    let largo = GETTEXTLENGTHEX {
        flags: GTL_NUMCHARS | GTL_PRECISE,
        codepage: 1200,
    };
    let n = enviar(edit, EM_GETTEXTLENGTHEX, &largo as *const _ as usize, 0).max(0) as usize;
    let mut buf = vec![0u16; n + 1];
    let pedido = GETTEXTEX {
        cb: ((n + 1) * 2) as u32,
        flags: GT_DEFAULT,
        codepage: 1200,
        ..Default::default()
    };
    let copiados = enviar(edit, EM_GETTEXTEX, &pedido as *const _ as usize, buf.as_mut_ptr() as isize).max(0) as usize;
    buf.truncate(copiados.min(n));
    String::from_utf16_lossy(&buf).replace('\r', "\n")
}

fn poner_todo(edit: HWND, texto: &str) {
    let t = ancho_nulo(&texto.replace("\r\n", "\n").replace('\n', "\r"));
    let st = SETTEXTEX {
        flags: ST_DEFAULT,
        codepage: 1200,
    };
    enviar(edit, EM_SETTEXTEX, &st as *const _ as usize, t.as_ptr() as isize);
    enviar(edit, EM_EMPTYUNDOBUFFER, 0, 0);
}

fn seleccion(edit: HWND) -> (usize, usize) {
    let mut r = CHARRANGE::default();
    enviar(edit, EM_EXGETSEL, 0, &mut r as *mut _ as isize);
    (r.cpMin.max(0) as usize, r.cpMax.max(0) as usize)
}

fn elegir(edit: HWND, a: usize, b: usize) {
    let r = CHARRANGE {
        cpMin: a as i32,
        cpMax: b as i32,
    };
    enviar(edit, EM_EXSETSEL, 0, &r as *const _ as isize);
}

/// Cambia el texto del control por `nuevo` tocando solo lo que difiere, y
/// como un paso de deshacer: rehacerlo entero borraria el historial y el
/// sitio por donde se iba.
fn aplicar_cambio(edit: HWND, viejo: &str, nuevo: &str) {
    let a: Vec<u16> = viejo.encode_utf16().collect();
    let b: Vec<u16> = nuevo.encode_utf16().collect();
    let (desde, hasta_a, hasta_b) = diferencia(&a, &b);
    elegir(edit, desde, hasta_a);
    let puesto: Vec<u16> = b[desde..hasta_b]
        .iter()
        .map(|&c| if c == b'\n' as u16 { b'\r' as u16 } else { c })
        .chain(std::iter::once(0))
        .collect();
    enviar(edit, EM_REPLACESEL, 1, puesto.as_ptr() as isize);
}

/// El tramo que cambia entre dos textos: lo comun por delante y por detras
/// se queda. Devuelve donde empieza y donde acaba en cada uno.
fn diferencia(a: &[u16], b: &[u16]) -> (usize, usize, usize) {
    let delante = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let max_detras = a.len().min(b.len()) - delante;
    let detras = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take(max_detras)
        .take_while(|(x, y)| x == y)
        .count();
    (delante, a.len() - detras, b.len() - detras)
}

// ---------------------------------------------------------------------------
// El formato

struct Estado {
    marco: HWND,
    edit: HWND,
    doc: Option<ITextDocument>,
    /// Lo ultimo guardado: si el texto es esto, cerrar no escribe nada.
    guardado: String,
    /// El renglon con las marcas a la vista.
    linea_activa: usize,
    /// Hay un repintado entero pendiente (el temporizador esta en marcha).
    pendiente: bool,
    rotulos: Rotulos,
    nombre_de_fichero: Option<String>,
}

fn formato() -> CHARFORMAT2W {
    let mut f = CHARFORMAT2W::default();
    f.Base.cbSize = std::mem::size_of::<CHARFORMAT2W>() as u32;
    f
}

fn cara(f: &mut CHARFORMAT2W, nombre: &str) {
    f.Base.dwMask |= CFM_FACE;
    for (i, c) in nombre.encode_utf16().take(31).enumerate() {
        f.Base.szFaceName[i] = c;
    }
}

fn poner_formato(edit: HWND, f: &CHARFORMAT2W) {
    enviar(edit, EM_SETCHARFORMAT, SCF_SELECTION as usize, f as *const _ as isize);
}

fn poner_parrafo(edit: HWND, p: &PARAFORMAT2) {
    enviar(edit, EM_SETPARAFORMAT, 0, p as *const _ as isize);
}

fn letra_base() -> CHARFORMAT2W {
    let mut f = formato();
    f.Base.dwMask = CFM_BOLD | CFM_ITALIC | CFM_UNDERLINE | CFM_STRIKEOUT | CFM_HIDDEN | CFM_SIZE | CFM_COLOR | CFM_BACKCOLOR;
    f.Base.dwEffects = CFE_AUTOBACKCOLOR;
    f.Base.yHeight = BASE;
    f.Base.crTextColor = TEXTO;
    cara(&mut f, "Segoe UI");
    f
}

fn parrafo_base() -> PARAFORMAT2 {
    let mut p = PARAFORMAT2::default();
    p.Base.cbSize = std::mem::size_of::<PARAFORMAT2>() as u32;
    p.Base.dwMask = PFM_NUMBERING | PFM_STARTINDENT | PFM_OFFSET | PFM_SPACEBEFORE | PFM_SPACEAFTER;
    p.dySpaceAfter = 60;
    p
}

/// El formato de un tramo. `activo` dice si su renglon tiene el cursor.
fn formato_de(estilo: Estilo, activo: bool) -> CHARFORMAT2W {
    let mut f = formato();
    let b = &mut f.Base;
    match estilo {
        Estilo::Marca if activo => {
            b.dwMask = CFM_COLOR;
            b.crTextColor = APAGADO;
        }
        Estilo::Marca => {
            b.dwMask = CFM_HIDDEN;
            b.dwEffects = CFE_HIDDEN;
        }
        Estilo::Titulo(n) => {
            b.dwMask = CFM_BOLD | CFM_SIZE;
            b.dwEffects = CFE_BOLD;
            b.yHeight = tamano_de_titulo(n);
        }
        Estilo::Negrita => {
            b.dwMask = CFM_BOLD;
            b.dwEffects = CFE_BOLD;
        }
        Estilo::Cursiva => {
            b.dwMask = CFM_ITALIC;
            b.dwEffects = CFE_ITALIC;
        }
        Estilo::Tachado => {
            b.dwMask = CFM_STRIKEOUT;
            b.dwEffects = CFE_STRIKEOUT;
        }
        Estilo::Codigo | Estilo::BloqueCodigo => {
            b.dwMask = CFM_COLOR | CFM_SIZE | CFM_BACKCOLOR;
            b.crTextColor = DORADO;
            b.yHeight = CODIGO;
            f.crBackColor = FONDO_CODIGO;
            cara(&mut f, "Consolas");
        }
        Estilo::Cita => {
            b.dwMask = CFM_COLOR | CFM_ITALIC;
            b.dwEffects = CFE_ITALIC;
            b.crTextColor = TENUE;
        }
        Estilo::Enlace => {
            b.dwMask = CFM_COLOR | CFM_UNDERLINE;
            b.dwEffects = CFE_UNDERLINE;
            b.crTextColor = AZUL;
        }
        Estilo::Casilla { hecha } => {
            b.dwMask = CFM_COLOR | CFM_BOLD;
            b.dwEffects = CFE_BOLD;
            b.crTextColor = if hecha { VERDE } else { ACENTO };
            cara(&mut f, "Consolas");
        }
        Estilo::Hecha => {
            b.dwMask = CFM_COLOR | CFM_STRIKEOUT;
            b.dwEffects = CFE_STRIKEOUT;
            b.crTextColor = APAGADO;
        }
        Estilo::Numero => {
            b.dwMask = CFM_COLOR;
            b.crTextColor = ACENTO;
        }
        Estilo::Formula => {
            b.dwMask = CFM_COLOR | CFM_ITALIC;
            b.dwEffects = CFE_ITALIC;
            b.crTextColor = MORADO;
            cara(&mut f, "Cambria Math");
        }
        Estilo::Regla => {
            b.dwMask = CFM_COLOR;
            b.crTextColor = APAGADO;
        }
        // La vineta es del parrafo, no de las letras.
        Estilo::Vineta => b.dwMask = CFM_MASK(0),
    }
    f
}

/// El parrafo de un renglon con ese estilo, si cambia algo.
fn parrafo_de(estilo: Estilo, activo: bool) -> Option<PARAFORMAT2> {
    let mut p = parrafo_base();
    match estilo {
        // Fuera del cursor el guion esta oculto y la vineta la pone Windows;
        // con el cursor se ve el guion, y una vineta mas seria doble.
        Estilo::Vineta if !activo => {
            p.Base.wNumbering = PFN_BULLET;
            p.Base.dxStartIndent = SANGRIA;
            p.Base.dxOffset = SANGRIA / 2;
        }
        Estilo::Vineta | Estilo::Cita => p.Base.dxStartIndent = SANGRIA,
        Estilo::BloqueCodigo => {
            p.Base.dxStartIndent = SANGRIA / 2;
            p.dySpaceAfter = 0;
        }
        _ => return None,
    }
    Some(p)
}

/// Pinta el formato. `solo` limita a unos renglones (al moverse el cursor);
/// `None` es todo (tras un cambio).
fn pintar(e: &mut Estado, solo: Option<&[usize]>) {
    let edit = e.edit;
    let texto = leer(edit);
    let ls = md_vivo::lineas(&texto);
    let (_, cursor) = seleccion(edit);
    let activa = md_vivo::linea_de(&ls, cursor);
    e.linea_activa = activa;
    let tramos = md_vivo::analizar(&texto);
    let entra = |n: usize| solo.is_none_or(|s| s.contains(&n));

    let mascara = enviar(edit, EM_GETEVENTMASK, 0, 0);
    enviar(edit, EM_SETEVENTMASK, 0, ENM_NONE as isize);
    // SAFETY: el documento es el del propio control, vivo mientras el.
    if let Some(d) = &e.doc {
        unsafe {
            let _ = d.Undo(tomSuspend.0);
            let _ = d.Freeze();
        }
    }
    let mut scroll = POINT::default();
    enviar(edit, EM_GETSCROLLPOS, 0, &mut scroll as *mut _ as isize);
    let sel = seleccion(edit);

    // Primero todo a letra y parrafo normales; luego cada tramo encima.
    let base = letra_base();
    let parrafo = parrafo_base();
    match solo {
        None => {
            elegir(edit, 0, usize::MAX >> 33);
            poner_formato(edit, &base);
            poner_parrafo(edit, &parrafo);
        }
        Some(ns) => {
            for l in ns.iter().filter_map(|n| ls.get(*n)) {
                elegir(edit, l.desde, l.hasta + 1);
                poner_formato(edit, &base);
                poner_parrafo(edit, &parrafo);
            }
        }
    }
    for t in tramos.iter().filter(|t| entra(t.linea)) {
        let activo = t.linea == activa;
        elegir(edit, t.desde, t.hasta);
        if let Some(p) = parrafo_de(t.estilo, activo) {
            poner_parrafo(edit, &p);
        }
        let f = formato_de(t.estilo, activo);
        if f.Base.dwMask.0 != 0 {
            poner_formato(edit, &f);
        }
    }

    elegir(edit, sel.0, sel.1);
    enviar(edit, EM_SETSCROLLPOS, 0, &scroll as *const _ as isize);
    if let Some(d) = &e.doc {
        // SAFETY: como arriba.
        unsafe {
            let _ = d.Unfreeze();
            let _ = d.Undo(tomResume.0);
        }
    }
    enviar(edit, EM_SETEVENTMASK, 0, mascara);
}

/// Si el cursor cambio de renglon, se ven las marcas del nuevo y se
/// esconden las del viejo.
fn seguir_al_cursor(e: &mut Estado) {
    if e.pendiente {
        return;
    }
    let texto = leer(e.edit);
    let ls = md_vivo::lineas(&texto);
    let (_, cursor) = seleccion(e.edit);
    let ahora = md_vivo::linea_de(&ls, cursor);
    if ahora != e.linea_activa {
        let antes = e.linea_activa;
        pintar(e, Some(&[antes, ahora]));
    }
}

// ---------------------------------------------------------------------------
// Las ordenes

fn titulo_de(e: &Estado, texto: &str) -> String {
    let nombre = e
        .nombre_de_fichero
        .clone()
        .unwrap_or_else(|| match md_vivo::titulo(texto) {
            t if t.is_empty() => e.rotulos.nueva.clone(),
            t => t,
        });
    format!("{nombre} — {}", e.rotulos.sufijo)
}

fn poner_titulo(e: &Estado, texto: &str) {
    // SAFETY: ventana propia; la cadena vive durante la llamada.
    unsafe {
        let _ = SetWindowTextW(e.marco, &HSTRING::from(titulo_de(e, texto)));
    }
}

fn guardar_ahora(e: &mut Estado, guardar: &mut dyn FnMut(&str) -> bool) -> bool {
    let texto = leer(e.edit);
    if texto == e.guardado {
        return true;
    }
    if guardar(&texto) {
        poner_titulo(e, &texto);
        e.guardado = texto;
        true
    } else {
        false
    }
}

/// Cambia el tipo del renglon del cursor y deja el cursor donde estaba
/// respecto al texto.
fn con_prefijo(e: &Estado, prefijo: &str) {
    let texto = leer(e.edit);
    let (a, b) = seleccion(e.edit);
    let ls = md_vivo::lineas(&texto);
    let n = md_vivo::linea_de(&ls, b);
    if let Some((nuevo, corrido)) = md_vivo::con_prefijo(&texto, n, prefijo) {
        aplicar_cambio(e.edit, &texto, &nuevo);
        let mover = |p: usize| (p as isize + corrido).max(ls[n].desde as isize) as usize;
        elegir(e.edit, mover(a), mover(b));
    }
}

fn envolver(e: &Estado, marca: &str) {
    let texto = leer(e.edit);
    let (a, b) = seleccion(e.edit);
    let (nuevo, x, y) = md_vivo::envolver(&texto, a, b, marca);
    aplicar_cambio(e.edit, &texto, &nuevo);
    elegir(e.edit, x, y);
}

fn con(e: &Estado, f: impl Fn(&str, usize, usize) -> (String, usize, usize)) {
    let texto = leer(e.edit);
    let (a, b) = seleccion(e.edit);
    let (nuevo, x, y) = f(&texto, a, b);
    aplicar_cambio(e.edit, &texto, &nuevo);
    elegir(e.edit, x, y);
}

/// La direccion del portapapeles, si lo que hay es una: es el boton de
/// pegar del dialogo de enlace del movil, sin dialogo.
fn url_del_portapapeles() -> String {
    match pixpin_codec::portapapeles::leer() {
        Some(pixpin_codec::portapapeles::ContenidoPortapapeles::Texto(t))
            if es_direccion(t.trim()) =>
        {
            t.trim().to_string()
        }
        _ => String::new(),
    }
}

/// Lo que se deja abrir con Ctrl+clic: paginas y correo. Un `file:` o un
/// ejecutable en una nota llegada de otro aparato no se lanza a ciegas.
fn es_direccion(s: &str) -> bool {
    let s = s.to_ascii_lowercase();
    (s.starts_with("https://") || s.starts_with("http://") || s.starts_with("mailto:")) && !s.contains(char::is_whitespace)
}

/// El titulo de la nota como nombre de fichero: sin las letras que Windows
/// no admite en un nombre.
fn sin_prohibidas(t: &str) -> String {
    let limpio: String = t
        .chars()
        .map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c })
        .collect();
    limpio.trim_end_matches(['.', ' ']).to_string()
}

fn alternar_casilla(e: &Estado, linea: usize) {
    let texto = leer(e.edit);
    if let Some(nuevo) = md_vivo::alternar_casilla(&texto, linea) {
        let sel = seleccion(e.edit);
        aplicar_cambio(e.edit, &texto, &nuevo);
        elegir(e.edit, sel.0, sel.1);
    }
}

fn atender(e: &mut Estado, o: Orden, guardar: &mut dyn FnMut(&str) -> bool) {
    match o {
        Orden::Cambio => {
            e.pendiente = true;
            // SAFETY: temporizador de una ventana propia.
            unsafe {
                SetTimer(Some(e.marco), T_REPINTAR, MS_RESPIRO, None);
            }
        }
        Orden::Repintar => {
            e.pendiente = false;
            pintar(e, None);
        }
        Orden::Autoguardar => {
            guardar_ahora(e, guardar);
        }
        Orden::Cerrar => {
            if guardar_ahora(e, guardar)
                || pixpin_shell::dialogo::preguntar(e.marco, &e.rotulos.sufijo, &e.rotulos.no_guardada)
            {
                // SAFETY: ventana propia; su WM_DESTROY acaba el bucle.
                unsafe {
                    let _ = DestroyWindow(e.marco);
                }
            }
        }
        Orden::Comando(c) => comando(e, c, guardar),
    }
}

fn comando(e: &mut Estado, c: u16, guardar: &mut dyn FnMut(&str) -> bool) {
    match c {
        C_NEGRITA => envolver(e, "**"),
        C_CURSIVA => envolver(e, "*"),
        C_TACHADO => envolver(e, "~~"),
        C_CODIGO => envolver(e, "`"),
        C_FORMULA => envolver(e, "$"),
        C_ENLACE => {
            let url = url_del_portapapeles();
            con(e, |t, a, b| md_vivo::poner_enlace(t, a, b, &url));
        }
        C_T1 => con_prefijo(e, "# "),
        C_T2 => con_prefijo(e, "## "),
        C_T3 => con_prefijo(e, "### "),
        C_LISTA => con_prefijo(e, "- "),
        C_NUMERADA => con_prefijo(e, "1. "),
        C_CASILLA => con_prefijo(e, "- [ ] "),
        C_CITA => con_prefijo(e, "> "),
        C_MARCAR => {
            let texto = leer(e.edit);
            let n = md_vivo::linea_de(&md_vivo::lineas(&texto), seleccion(e.edit).1);
            alternar_casilla(e, n);
        }
        C_BLOQUE => con(e, md_vivo::bloque_de_codigo),
        C_RAYA => {
            let (_, b) = seleccion(e.edit);
            elegir(e.edit, b, b);
            let t = ancho_nulo("\r---\r");
            enviar(e.edit, EM_REPLACESEL, 1, t.as_ptr() as isize);
        }
        C_CORTAR => {
            enviar(e.edit, WM_CUT, 0, 0);
        }
        C_COPIAR => {
            enviar(e.edit, WM_COPY, 0, 0);
        }
        // Se pega como texto: el Markdown es texto, y un formato pegado de
        // una pagina web no se guardaria de todos modos.
        C_PEGAR => {
            enviar(e.edit, EM_PASTESPECIAL, CF_UNICODETEXT, 0);
        }
        C_GUARDAR => {
            guardar_ahora(e, guardar);
        }
        C_COPIA_MD => {
            let texto = leer(e.edit);
            let nombre = match md_vivo::titulo(&texto) {
                t if t.is_empty() => e.rotulos.nueva.clone(),
                t => sin_prohibidas(&t),
            };
            if let Some(ruta) =
                pixpin_shell::guardar::pedir_ruta_para(e.marco, &nombre, &e.rotulos.tipo_md, "md")
                && let Err(err) = std::fs::write(&ruta, texto.as_bytes())
            {
                tracing::warn!(?err, ruta = %ruta.display(), "no se pudo guardar la copia .md");
            }
        }
        C_CERRAR => {
            // SAFETY: ventana propia; el WM_CLOSE pasa por guardar.
            unsafe {
                let _ = PostMessageW(Some(e.marco), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
        _ => {}
    }
}

/// Intro en una lista: el renglon nuevo sigue la lista, y en uno vacio se
/// sale de ella (`Vivo.partir`). `true` si se atendio aqui.
fn intro(e: &Estado) -> bool {
    let texto = leer(e.edit);
    let (a, b) = seleccion(e.edit);
    if a != b {
        return false;
    }
    let ls = md_vivo::lineas(&texto);
    let l = ls[md_vivo::linea_de(&ls, b)];
    let u: Vec<u16> = texto.encode_utf16().collect();
    let renglon = String::from_utf16_lossy(&u[l.desde..l.hasta]);
    match md_vivo::continuar(&renglon) {
        Continuar::Nada => false,
        Continuar::Cortar if b == l.hasta => {
            elegir(e.edit, l.desde, l.hasta);
            enviar(e.edit, EM_REPLACESEL, 1, ancho_nulo("").as_ptr() as isize);
            true
        }
        Continuar::Cortar => false,
        Continuar::Con(marca) => {
            // Con el cursor antes de la marca, un Intro normal: se abre un
            // renglon encima y la marca baja con su texto.
            let marca_hasta = l.desde + renglon.encode_utf16().count() - renglon.trim_start().encode_utf16().count() + 1;
            if b < marca_hasta {
                return false;
            }
            let t = ancho_nulo(&format!("\r{marca}"));
            enviar(e.edit, EM_REPLACESEL, 1, t.as_ptr() as isize);
            true
        }
    }
}

fn pulsada(vk: u16) -> bool {
    // SAFETY: consulta del estado del teclado de este hilo.
    unsafe { GetKeyState(vk as i32) < 0 }
}

/// La letra bajo un punto del control.
fn letra_en(edit: HWND, l: LPARAM) -> usize {
    let p = POINT {
        x: (l.0 & 0xffff) as i16 as i32,
        y: ((l.0 >> 16) & 0xffff) as i16 as i32,
    };
    enviar(edit, EM_CHARFROMPOS, 0, &p as *const _ as isize).max(0) as usize
}

/// Lo que se atiende antes de que el control vea el mensaje. `true` si ya
/// no hay que darselo.
fn interceptar(e: &mut Estado, m: &MSG) -> bool {
    match m.message {
        WM_KEYDOWN if m.wParam.0 == VK_RETURN.0 as usize
            && !pulsada(VK_CONTROL.0)
            && !pulsada(VK_SHIFT.0)
            && !pulsada(VK_MENU.0) =>
        {
            intro(e)
        }
        // Un clic en la casilla la marca, como en el movil. Con Ctrl, un
        // clic en un enlace lo abre (sin Ctrl se esta escribiendo en el).
        WM_LBUTTONDOWN => {
            let pos = letra_en(e.edit, m.lParam);
            let texto = leer(e.edit);
            if pulsada(VK_CONTROL.0)
                && let Some(url) = md_vivo::enlace_en(&texto, pos).filter(|u| es_direccion(u))
            {
                if let Err(err) = pixpin_shell::abrir(std::path::Path::new(&url)) {
                    tracing::warn!(?err, "no se pudo abrir el enlace");
                }
                return true;
            }
            match md_vivo::casilla_en(&texto, pos) {
                Some(n) => {
                    alternar_casilla(e, n);
                    true
                }
                None => false,
            }
        }
        WM_RBUTTONUP => {
            menu_contextual(e);
            true
        }
        WM_KEYDOWN if m.wParam.0 == VK_APPS.0 as usize || (m.wParam.0 == VK_F10.0 as usize && pulsada(VK_SHIFT.0)) => {
            menu_contextual(e);
            true
        }
        _ => false,
    }
}

/// El menu del clic derecho: el formato, que en el movil es la barra que
/// sale al elegir texto, con su tecla al lado.
fn menu_contextual(e: &mut Estado) {
    let r = &e.rotulos;
    let (m, i) = (&r.mayus, &r.intro);
    let entradas: Vec<Option<(u16, String)>> = vec![
        Some((C_NEGRITA, format!("{}\tCtrl+B", r.negrita))),
        Some((C_CURSIVA, format!("{}\tCtrl+I", r.cursiva))),
        Some((C_TACHADO, format!("{}\tCtrl+{m}+X", r.tachado))),
        Some((C_CODIGO, format!("{}\tCtrl+E", r.codigo))),
        Some((C_ENLACE, format!("{}\tCtrl+K", r.enlace))),
        Some((C_FORMULA, format!("{}\tCtrl+M", r.formula))),
        None,
        Some((C_T1, format!("{}\tCtrl+1", r.titulo1))),
        Some((C_T2, format!("{}\tCtrl+2", r.titulo2))),
        Some((C_T3, format!("{}\tCtrl+3", r.titulo3))),
        Some((C_LISTA, format!("{}\tCtrl+{m}+8", r.lista))),
        Some((C_NUMERADA, format!("{}\tCtrl+{m}+7", r.numerada))),
        Some((C_CASILLA, format!("{}\tCtrl+{m}+9", r.casilla))),
        Some((C_MARCAR, format!("{}\tCtrl+{i}", r.marcar))),
        Some((C_CITA, format!("{}\tCtrl+{m}+.", r.cita))),
        Some((C_BLOQUE, format!("{}\tCtrl+{m}+C", r.bloque))),
        Some((C_RAYA, r.raya.clone())),
        None,
        Some((C_CORTAR, format!("{}\tCtrl+X", r.cortar))),
        Some((C_COPIAR, format!("{}\tCtrl+C", r.copiar))),
        Some((C_PEGAR, format!("{}\tCtrl+V", r.pegar))),
        None,
        Some((C_GUARDAR, format!("{}\tCtrl+S", r.guardar))),
        Some((C_COPIA_MD, format!("{}\tCtrl+{m}+S", r.copia_md))),
    ];
    // SAFETY: el menu se crea y se destruye aqui; las cadenas viven durante
    // cada llamada; la ventana es propia.
    let elegido = unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        for en in &entradas {
            let _ = match en {
                Some((id, t)) => AppendMenuW(menu, MF_STRING, *id as usize, &HSTRING::from(t.as_str())),
                None => AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()),
            };
        }
        let mut p = POINT::default();
        let _ = GetCursorPos(&mut p);
        let r = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, p.x, p.y, None, e.marco, None);
        let _ = DestroyMenu(menu);
        r.0 as u16
    };
    if elegido != 0 {
        apuntar(Orden::Comando(elegido));
    }
}

fn teclas() -> Vec<ACCEL> {
    let c = FVIRTKEY | FCONTROL;
    let cs = FVIRTKEY | FCONTROL | FSHIFT;
    let a = |f: ACCEL_VIRT_FLAGS, k: u16, cmd: u16| ACCEL { fVirt: f, key: k, cmd };
    vec![
        a(c, b'B' as u16, C_NEGRITA),
        a(c, b'I' as u16, C_CURSIVA),
        a(cs, b'X' as u16, C_TACHADO),
        a(c, b'E' as u16, C_CODIGO),
        a(c, b'K' as u16, C_ENLACE),
        a(c, b'M' as u16, C_FORMULA),
        a(c, b'1' as u16, C_T1),
        a(c, b'2' as u16, C_T2),
        a(c, b'3' as u16, C_T3),
        a(cs, b'8' as u16, C_LISTA),
        a(cs, b'7' as u16, C_NUMERADA),
        a(cs, b'9' as u16, C_CASILLA),
        a(c, VK_RETURN.0, C_MARCAR),
        a(cs, 0xBE, C_CITA),
        a(cs, b'C' as u16, C_BLOQUE),
        a(c, b'V' as u16, C_PEGAR),
        a(FVIRTKEY | FSHIFT, 0x2D, C_PEGAR),
        a(c, b'S' as u16, C_GUARDAR),
        a(cs, b'S' as u16, C_COPIA_MD),
        a(c, b'W' as u16, C_CERRAR),
    ]
}

// ---------------------------------------------------------------------------
// La ventana

unsafe extern "system" fn procedimiento(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        WM_SIZE => {
            colocar_edit(h);
            LRESULT(0)
        }
        WM_SETFOCUS => {
            let edit = EDIT.with(|e| e.get());
            if edit != 0 {
                // SAFETY: el control es hijo de esta ventana y vive con ella.
                unsafe {
                    let _ = SetFocus(Some(HWND(edit as *mut _)));
                }
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            if l.0 != 0 {
                if (w.0 >> 16) & 0xffff == EN_CHANGE {
                    apuntar(Orden::Cambio);
                }
            } else {
                apuntar(Orden::Comando((w.0 & 0xffff) as u16));
            }
            LRESULT(0)
        }
        WM_TIMER => {
            if w.0 == T_REPINTAR {
                // SAFETY: temporizador propio.
                unsafe {
                    let _ = KillTimer(Some(h), T_REPINTAR);
                }
                apuntar(Orden::Repintar);
            } else if w.0 == T_AUTOGUARDAR {
                apuntar(Orden::Autoguardar);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            apuntar(Orden::Cerrar);
            LRESULT(0)
        }
        WM_DESTROY => {
            // SAFETY: acaba el bucle de este hilo.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        // SAFETY: lo demas, a Windows con los mismos argumentos.
        _ => unsafe { DefWindowProcW(h, m, w, l) },
    }
}

/// El control ocupa la ventana, con la columna de texto centrada y un
/// margen: una linea de lado a lado de un monitor ancho no se lee.
fn colocar_edit(marco: HWND) {
    let edit = EDIT.with(|e| e.get());
    if edit == 0 {
        return;
    }
    let edit = HWND(edit as *mut _);
    let mut r = RECT::default();
    // SAFETY: ventanas propias; el rectangulo es local.
    unsafe {
        let _ = GetClientRect(marco, &mut r);
        let _ = MoveWindow(edit, 0, 0, r.right, r.bottom, true);
    }
    let ppp = PPP.with(|p| p.get());
    let e = |v: i32| v * ppp / 96;
    let lado = e(28).max((r.right - e(COLUMNA)) / 2);
    let dentro = RECT {
        left: lado,
        top: e(20),
        right: (r.right - lado).max(lado + 1),
        bottom: (r.bottom - e(12)).max(e(21)),
    };
    enviar(edit, EM_SETRECT, 0, &dentro as *const _ as isize);
}

fn ppp_de_pantalla() -> i32 {
    // SAFETY: el DC de la pantalla se pide y se devuelve en el acto.
    unsafe {
        let dc = GetDC(None);
        let v = GetDeviceCaps(Some(dc), LOGPIXELSY);
        ReleaseDC(None, dc);
        v.max(96)
    }
}

/// Abre la ventana y no vuelve hasta que se cierra. `al_nacer` recibe la
/// ventana en cuanto existe; `guardar` escribe el texto y dice si pudo.
pub fn correr(
    p: Pedido,
    al_nacer: &mut dyn FnMut(isize),
    guardar: &mut dyn FnMut(&str) -> bool,
) -> windows::core::Result<()> {
    // SAFETY: todo lo que se crea aqui es de este hilo y muere con su bucle:
    // la clase (registrarla dos veces falla sin dano), las ventanas, el
    // pincel del fondo y la tabla de teclas, que se sueltan al final.
    unsafe {
        windows::Win32::System::LibraryLoader::LoadLibraryW(w!("msftedit.dll"))?;
        let instancia = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?;
        let pincel: HBRUSH = CreateSolidBrush(FONDO);
        let clase = WNDCLASSW {
            lpfnWndProc: Some(procedimiento),
            hInstance: instancia.into(),
            lpszClassName: w!("PixPinNotaMd"),
            hbrBackground: pincel,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&clase);
        let ppp = ppp_de_pantalla();
        PPP.with(|c| c.set(ppp));
        let e = |v: i32| v * ppp / 96;
        let marco = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("PixPinNotaMd"),
            w!(""),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            e(900),
            e(900),
            None,
            None,
            Some(instancia.into()),
            None,
        )?;
        // La barra del titulo, oscura como el resto (sin esto sale blanca
        // encima de un papel negro).
        let oscuro: i32 = 1;
        let _ = windows::Win32::Graphics::Dwm::DwmSetWindowAttribute(
            marco,
            windows::Win32::Graphics::Dwm::DWMWA_USE_IMMERSIVE_DARK_MODE,
            &oscuro as *const _ as *const _,
            4,
        );
        let edit = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            MSFTEDIT_CLASS,
            w!(""),
            WS_CHILD | WS_VISIBLE | WS_VSCROLL | WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL | ES_NOHIDESEL) as u32),
            0,
            0,
            10,
            10,
            Some(marco),
            None,
            Some(instancia.into()),
            None,
        )?;
        EDIT.with(|c| c.set(edit.0 as isize));

        // Sin el tope de 32 000 letras que el RichEdit trae de fabrica.
        enviar(edit, EM_EXLIMITTEXT, 0, 0x7FFF_FFFE);
        enviar(edit, EM_SETBKGNDCOLOR, 0, FONDO.0 as isize);
        // Ajustar a la ventana, sin barra horizontal.
        enviar(edit, EM_SETTARGETDEVICE, 0, 0);
        // El corrector ortografico de Windows (8 en adelante) y el IME.
        let opciones = enviar(edit, EM_GETLANGOPTIONS, 0, 0);
        enviar(edit, EM_SETLANGOPTIONS, 0, opciones | IMF_SPELLCHECKING as isize);
        let ctf = SES_USECTF | SES_CTFALLOWPROOFING | SES_CTFALLOWEMBED;
        enviar(edit, EM_SETEDITSTYLE, ctf as usize, ctf as isize);
        let mut f = letra_base();
        f.Base.dwMask = CFM_SIZE | CFM_COLOR | CFM_FACE;
        enviar(edit, EM_SETCHARFORMAT, SCF_ALL as usize, &f as *const _ as isize);
        poner_todo(edit, &p.texto);
        enviar(edit, EM_SETEVENTMASK, 0, ENM_CHANGE as isize);

        let doc: Option<ITextDocument> = {
            let mut ole: *mut core::ffi::c_void = std::ptr::null_mut();
            enviar(edit, EM_GETOLEINTERFACE, 0, &mut ole as *mut _ as isize);
            if ole.is_null() {
                None
            } else {
                windows::core::IUnknown::from_raw(ole).cast::<ITextDocument>().ok()
            }
        };

        let mut estado = Estado {
            marco,
            edit,
            doc,
            guardado: p.texto.replace("\r\n", "\n").replace('\r', "\n"),
            linea_activa: usize::MAX,
            pendiente: false,
            rotulos: p.rotulos,
            nombre_de_fichero: p.nombre_de_fichero,
        };
        poner_titulo(&estado, &estado.guardado.clone());
        pintar(&mut estado, None);
        // Una nota nueva empieza arriba; una que ya tiene texto, al final,
        // que es donde se sigue escribiendo (`TextRange(inicial.length)`).
        let fin = estado.guardado.encode_utf16().count();
        elegir(edit, fin, fin);
        seguir_al_cursor(&mut estado);

        al_nacer(marco.0 as isize);
        colocar_edit(marco);
        match p.colocacion {
            Some(c) => {
                pixpin_shell::colocacion::poner(marco.0 as isize, &c);
            }
            None => {
                let _ = ShowWindow(marco, SW_SHOWNORMAL);
            }
        }
        colocar_edit(marco);
        let _ = SetForegroundWindow(marco);
        let _ = SetFocus(Some(edit));
        SetTimer(Some(marco), T_AUTOGUARDAR, MS_AUTOGUARDAR, None);

        let tabla = CreateAcceleratorTableW(&teclas())?;
        let mut m = MSG::default();
        while GetMessageW(&mut m, None, 0, 0).0 > 0 {
            if m.hwnd == edit && interceptar(&mut estado, &m) {
            } else if TranslateAcceleratorW(marco, tabla, &m) == 0 {
                let _ = TranslateMessage(&m);
                DispatchMessageW(&m);
            }
            let ordenes: Vec<Orden> = COLA.with(|c| std::mem::take(&mut *c.borrow_mut()));
            for o in ordenes {
                atender(&mut estado, o, guardar);
            }
            if m.hwnd == edit
                && matches!(m.message, WM_KEYDOWN | WM_KEYUP | WM_LBUTTONUP | WM_LBUTTONDOWN)
            {
                seguir_al_cursor(&mut estado);
            }
        }
        let _ = DestroyAcceleratorTable(tabla);
        EDIT.with(|c| c.set(0));
        let _ = DeleteObject(HGDIOBJ(pincel.0));
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn u(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn la_diferencia_toca_solo_lo_que_cambia() {
        // Marcar una casilla es cambiar una letra.
        assert_eq!(diferencia(&u("- [ ] a"), &u("- [x] a")), (3, 4, 4));
        // Poner un titulo es meter dos al principio.
        assert_eq!(diferencia(&u("hola"), &u("# hola")), (0, 0, 2));
        // Quitar la negrita, cuatro menos.
        assert_eq!(diferencia(&u("x **y** z"), &u("x y z")), (2, 7, 3));
    }

    #[test]
    fn la_diferencia_de_textos_iguales_o_repetidos_no_se_sale() {
        assert_eq!(diferencia(&u("aaa"), &u("aaa")), (3, 3, 3));
        assert_eq!(diferencia(&u("aa"), &u("aaaa")), (2, 2, 4));
        assert_eq!(diferencia(&u(""), &u("x")), (0, 0, 1));
    }

    #[test]
    fn solo_se_abren_paginas_y_correo() {
        assert!(es_direccion("https://x.es/p"));
        assert!(es_direccion("mailto:a@b.es"));
        assert!(!es_direccion("file:///C:/Windows/calc.exe"));
        assert!(!es_direccion("C:\\a.exe"));
        assert!(!es_direccion("https://x.es con espacio"));
    }

    #[test]
    fn el_nombre_de_la_copia_no_lleva_letras_prohibidas() {
        assert_eq!(sin_prohibidas("Obra: planta 1/2?"), "Obra_ planta 1_2_");
        assert_eq!(sin_prohibidas("fin. "), "fin");
    }

    #[test]
    fn los_colores_van_al_reves_como_los_quiere_windows() {
        assert_eq!(rgb(0x112233).0, 0x00332211);
    }

    #[test]
    fn una_marca_se_esconde_salvo_en_el_renglon_del_cursor() {
        let fuera = formato_de(Estilo::Marca, false);
        assert_eq!(fuera.Base.dwEffects, CFE_HIDDEN);
        let dentro = formato_de(Estilo::Marca, true);
        assert_eq!(dentro.Base.dwMask, CFM_COLOR);
        assert_eq!(dentro.Base.dwEffects.0 & CFE_HIDDEN.0, 0);
    }

    #[test]
    fn la_vineta_la_pone_windows_solo_fuera_del_cursor() {
        assert_eq!(parrafo_de(Estilo::Vineta, false).unwrap().Base.wNumbering, PFN_BULLET);
        assert_eq!(parrafo_de(Estilo::Vineta, true).unwrap().Base.wNumbering.0, 0);
        assert!(parrafo_de(Estilo::Negrita, false).is_none());
    }

    /// Un RichEdit de verdad pero oculto (nunca se ensena), con el texto
    /// puesto y su documento de TOM, para probar el formato sin pantalla.
    fn edit_oculto(texto: &str, ancho: i32, alto: i32) -> Estado {
        // SAFETY: ventana de la prueba, que la destruye `soltar`.
        unsafe {
            windows::Win32::System::LibraryLoader::LoadLibraryW(w!("msftedit.dll")).unwrap();
            let edit = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                MSFTEDIT_CLASS,
                w!(""),
                WS_POPUP | WINDOW_STYLE(ES_MULTILINE as u32),
                0,
                0,
                ancho,
                alto,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            enviar(edit, EM_EXLIMITTEXT, 0, 0x7FFF_FFFE);
            enviar(edit, EM_SETBKGNDCOLOR, 0, FONDO.0 as isize);
            poner_todo(edit, texto);
            let doc = {
                let mut ole: *mut core::ffi::c_void = std::ptr::null_mut();
                enviar(edit, EM_GETOLEINTERFACE, 0, &mut ole as *mut _ as isize);
                windows::core::IUnknown::from_raw(ole).cast::<ITextDocument>().ok()
            };
            Estado {
                marco: edit,
                edit,
                doc,
                guardado: String::new(),
                linea_activa: usize::MAX,
                pendiente: false,
                rotulos: Rotulos::default(),
                nombre_de_fichero: None,
            }
        }
    }

    fn soltar(e: Estado) {
        // SAFETY: la ventana de `edit_oculto`.
        unsafe {
            let _ = DestroyWindow(e.edit);
        }
    }

    /// Los efectos de la letra en `pos` (lo que Windows dice de ella).
    /// Se pregunta con TOM y no eligiendo la letra: el RichEdit no deja
    /// elegir texto oculto (mueve la seleccion fuera), y se leeria otra.
    fn efectos(e: &Estado, pos: usize) -> u32 {
        // SAFETY: el documento del control de la prueba, vivo.
        unsafe {
            let f = e.doc.as_ref().unwrap().Range(pos as i32, pos as i32 + 1).unwrap().GetFont().unwrap();
            let si = |v: i32| v == tomTrue.0;
            let mut bits = 0;
            if si(f.GetHidden().unwrap()) {
                bits |= CFE_HIDDEN.0;
            }
            if si(f.GetBold().unwrap()) {
                bits |= CFE_BOLD.0;
            }
            bits
        }
    }

    const EM_CANUNDO: u32 = 0x00C6;
    const EM_UNDO: u32 = 0x00C7;

    #[test]
    fn las_marcas_se_ocultan_fuera_del_cursor_y_el_formato_no_se_deshace() {
        let mut e = edit_oculto("# Obra\n**cemento**\n", 600, 400);
        let fin = leer(e.edit).encode_utf16().count();
        elegir(e.edit, fin, fin);
        pintar(&mut e, None);
        // El cursor esta en el renglon 2 (vacio): las marcas de arriba, ocultas.
        assert_ne!(efectos(&e, 0) & CFE_HIDDEN.0, 0, "la almohadilla se ve");
        assert_ne!(efectos(&e, 7) & CFE_HIDDEN.0, 0, "los asteriscos se ven");
        assert_ne!(efectos(&e, 9) & CFE_BOLD.0, 0, "la negrita no es negrita");
        assert_eq!(efectos(&e, 2) & CFE_HIDDEN.0, 0, "el titulo quedo oculto");
        // Pintar no deja nada que deshacer: Ctrl+Z deshace letras, no colores.
        assert_eq!(enviar(e.edit, EM_CANUNDO, 0, 0), 0);
        // Al subir al titulo, su almohadilla se ve y los asteriscos siguen ocultos.
        elegir(e.edit, 3, 3);
        seguir_al_cursor(&mut e);
        assert_eq!(efectos(&e, 0) & CFE_HIDDEN.0, 0, "la almohadilla sigue oculta");
        assert_ne!(efectos(&e, 7) & CFE_HIDDEN.0, 0);
        // Y el texto es el Markdown, letra por letra.
        assert_eq!(leer(e.edit), "# Obra\n**cemento**\n");
        soltar(e);
    }

    #[test]
    fn un_cambio_minimo_se_puede_deshacer_y_deja_el_resto() {
        let e = edit_oculto("- [ ] a\n- [ ] b", 600, 400);
        alternar_casilla(&e, 1);
        assert_eq!(leer(e.edit), "- [ ] a\n- [x] b");
        enviar(e.edit, EM_UNDO, 0, 0);
        assert_eq!(leer(e.edit), "- [ ] a\n- [ ] b");
        soltar(e);
    }

    fn nota_larga() -> String {
        let mut nota = String::new();
        for i in 0..100 {
            nota.push_str(&format!(
                "## Parte {i}\nTexto con **negrita**, *cursiva* y `codigo` y [un enlace](https://x.es).\n- [ ] tarea {i}\n- [x] hecha {i}\n> una cita\n\n"
            ));
        }
        nota
    }

    /// Cuanto tarda el repintado de una nota larga. Se corre a mano:
    /// `cargo test --release -p pixpin-notas medir_repintado -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn medir_repintado() {
        let nota = nota_larga();
        let mut e = edit_oculto(&nota, 800, 800);
        let t = std::time::Instant::now();
        pintar(&mut e, None);
        let entero = t.elapsed();
        let t = std::time::Instant::now();
        pintar(&mut e, Some(&[3, 40]));
        let dos = t.elapsed();
        assert_eq!(leer(e.edit), nota);
        println!(
            "nota de {} letras y {} renglones: repintado entero {:?}, dos renglones {:?}",
            nota.chars().count(),
            nota.lines().count(),
            entero,
            dos
        );
        soltar(e);
    }

    /// Una muestra en PNG de como queda una nota, pintada fuera de pantalla
    /// con `EM_FORMATRANGE` (lo que usa el RichEdit para imprimir). Se corre a
    /// mano con `PIXPIN_MUESTRA=<carpeta>`:
    /// `cargo test -p pixpin-notas muestra_png -- --ignored`.
    #[test]
    #[ignore]
    fn muestra_png() {
        use windows::Win32::Graphics::Gdi::*;
        let _com = pixpin_shell::ComDelHilo::iniciar();
        let carpeta = std::env::var("PIXPIN_MUESTRA").unwrap_or_else(|_| ".".into());
        let texto = "# Obra de la calle Mayor\nLa **losa** se hormigona el *lunes*; ver [el plano](https://x.es/p).\n\n## Tareas\n- [x] Pedir la grua\n- [ ] Comprar `cemento 42,5`\n- [ ] Avisar al vecino\n\n> Sin permiso no se corta la calle.\n\n1. Encofrar\n2. Armar\n- Una vineta\n\nArea del pilar: $\\pi r^2$\n\n```\ncota = 3,20\n```\n---\nfin";
        let (ancho, alto) = (760, 820);
        let mut e = edit_oculto(texto, ancho, alto);
        // Con el cursor en «Comprar», para ver sus marcas y las demas ocultas.
        let resto = texto.split("Comprar").nth(1).unwrap().encode_utf16().count();
        let pos = texto.encode_utf16().count() - resto;
        elegir(e.edit, pos, pos);
        pintar(&mut e, None);
        // SAFETY: DC y mapa de bits propios, soltados al final.
        unsafe {
            let dc = CreateCompatibleDC(None);
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: ancho,
                    biHeight: -alto,
                    biPlanes: 1,
                    biBitCount: 32,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let mapa = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0).unwrap();
            let viejo = SelectObject(dc, HGDIOBJ(mapa.0));
            let pincel = CreateSolidBrush(FONDO);
            FillRect(
                dc,
                &RECT {
                    left: 0,
                    top: 0,
                    right: ancho,
                    bottom: alto,
                },
                pincel,
            );
            // En veinteavos de punto, a 96 ppp: 15 por pixel.
            let fr = FORMATRANGE {
                hdc: dc,
                hdcTarget: dc,
                rc: RECT {
                    left: 40 * 15,
                    top: 30 * 15,
                    right: (ancho - 40) * 15,
                    bottom: (alto - 20) * 15,
                },
                rcPage: RECT {
                    left: 0,
                    top: 0,
                    right: ancho * 15,
                    bottom: alto * 15,
                },
                chrg: CHARRANGE { cpMin: 0, cpMax: -1 },
            };
            enviar(e.edit, EM_FORMATRANGE, 1, &fr as *const _ as isize);
            enviar(e.edit, EM_FORMATRANGE, 0, 0);
            let crudo = std::slice::from_raw_parts(bits as *const u8, (ancho * alto * 4) as usize);
            let mut rgba = Vec::with_capacity(crudo.len());
            for p in crudo.chunks_exact(4) {
                rgba.extend_from_slice(&[p[2], p[1], p[0], 255]);
            }
            let img = pixpin_codec::ImagenRgba {
                ancho: ancho as u32,
                alto: alto as u32,
                pixeles: rgba,
            };
            let png = pixpin_codec::imagen::codificar_png(&img).unwrap();
            std::fs::write(std::path::Path::new(&carpeta).join("nota-md-muestra.png"), png).unwrap();
            SelectObject(dc, viejo);
            let _ = DeleteObject(HGDIOBJ(mapa.0));
            let _ = DeleteObject(HGDIOBJ(pincel.0));
            let _ = DeleteDC(dc);
        }
        soltar(e);
    }
}
