//! **La ventana del editor de notas**, con la pinta y la comodidad del
//! editor de documentos de Claude que mando el usuario: cabecera con el
//! titulo en su pastilla (clic y se cambia), «Compartir», pantalla completa
//! y cerrar; una barra con insertar tabla, imagen, casillas y el «+» de
//! listas, casillas y fecha; el papel con la columna de texto centrada,
//! titulos en serif y cuerpo sans; y el menu de la barra `/` al principio de
//! un renglon. Oscuro o claro segun Windows.
//!
//! # Por que el `RichEdit` y no un editor propio
//!
//! El texto lo edita el `RichEdit` de Windows (`msftedit.dll`, viene con
//! Windows): trae el IME y los acentos, el corrector, deshacer, el zoom, la
//! accesibilidad y **tablas de verdad** (`EM_INSERTTABLE`, RTF `\trowd`):
//! Tab de celda en celda, una fila nueva con Tab en la ultima, y la barra de
//! abajo si la tabla es mas ancha que la ventana. Un editor propio sobre el
//! pintor tendria que rehacer todo eso (el IME a mano, el corrector no) para
//! una maquina de 4 GB con una HD 4000. Lo propio es solo el marco (cabecera,
//! barra y menus, pintados con Direct2D sobre su `HDC`, ver `pintor`).
//!
//! # El formato, encima del texto
//!
//! El texto del control es el Markdown tal cual (ver `pixpin_docs::md_vivo`)
//! salvo las tablas, que son tablas del control y se traducen al leer y al
//! meter (`pixpin_docs::md_tabla`). Tras cada cambio —con un respiro de
//! 60 ms— se analiza y se pinta: titulos grandes en Fraunces, negrita,
//! codigo con su fondo, casillas en color… y las marcas (`#`, `**`, `](url)`)
//! **ocultas** (`CFE_HIDDEN`) siempre, tambien en el renglon del cursor: se
//! edita como en Claude, sin ver el Markdown (ver `wysiwyg`).
//! Una foto (`![x](ruta)`) se pinta en el aire de encima de su renglon (ver
//! `imagenes`). El formato se pinta con el deshacer suspendido: Ctrl+Z
//! deshace letras, no colores.
//!
//! # Quien hace que
//!
//! Los procedimientos de ventana solo apuntan lo que pasa en una cola; el
//! bucle, que tiene el estado y lo de guardar, lo atiende. Lo que hace falta
//! para pintar el marco vive aparte ([`VistaMarco`]) para que pintar no
//! dependa del estado del bucle.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Controls::RichEdit::*;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SetFocus, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_APPS, VK_CONTROL, VK_DOWN, VK_ESCAPE,
    VK_F10, VK_F11, VK_MENU, VK_RETURN, VK_SHIFT, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, Interface, PCWSTR, w};

use pixpin_docs::md_comandos::{self, Bloque};
use pixpin_docs::md_tabla::{self, Alineacion, Tabla};
use pixpin_docs::md_vivo::{self, Continuar, Estilo};

use pixpin_shell::colocacion::Colocacion;

use crate::disposicion::{self, Boton, Caja, Marco, Medidas, Zona};
use crate::imagenes::{self, Fotos};
use crate::letras::{self, Letras};
use crate::menu::{self, Dibujo, Entrada, Menu};
use crate::pintor::{Icono, Pintor};
use crate::tabla_rtf::{self, EstiloTabla};
use crate::tema::{Rgb, Tema, bgr};

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
    // Lo de la ventana nueva (cabecera, barra y menus).
    pub compartir: String,
    pub tabla: String,
    pub imagen: String,
    pub fecha: String,
    /// La pista de abajo del menu «+»: que hay mas escribiendo `/`.
    pub pista_barra: String,
    pub cambiar_titulo: String,
    pub fila_encima: String,
    pub fila_debajo: String,
    pub quitar_fila: String,
    pub columna_izquierda: String,
    pub columna_derecha: String,
    pub quitar_columna: String,
    pub quitar_tabla: String,
    /// Los cuatro botones de la tabla en la barra: «+ Fila», «− Fila»…
    pub boton_fila_mas: String,
    pub boton_fila_menos: String,
    pub boton_columna_mas: String,
    pub boton_columna_menos: String,
    // Combinar y colorear celdas (ver `tablas`). Vacios, en espanol.
    pub boton_combinar: String,
    pub boton_color: String,
    pub combinar_celdas: String,
    pub separar_celdas: String,
    pub color_fondo: String,
    pub color_letra: String,
    /// «Celdas|Fila entera|Columna entera»: las columnas de los submenus.
    pub aplicar_a: String,
    /// Los cinco nombres de la paleta, separados por `|`: sin color y los
    /// cuatro fondos; automatico y las cuatro letras.
    pub nombres_fondo: String,
    pub nombres_letra: String,
    pub pista_color: String,
    /// Las doce abreviaturas de los meses, separadas por espacios.
    pub meses: String,
    pub imagen_no_copiada: String,
    /// Lo de las fotos y las paginas vivas (ver `integracion`).
    pub fotos: crate::integracion::RotulosFotos,
    pub pagina_viva: String,
    pub enlace_hoja: String,
    /// Lo del panel de comentarios (ver `comentarios`).
    pub comentarios: crate::panel_comentarios::RotulosComentarios,
    /// Lo de la barra flotante, la letra y el tamano (ver `wysiwyg`).
    pub barra: crate::barra_flotante::RotulosBarra,
    /// Lo de los documentos, mensajes del chat y audios (ver `incrustados`).
    pub incrustados: crate::incrustados::RotulosIncrustados,
    /// «Exportar a Word…» y el nombre del tipo en el «Guardar como».
    pub exportar_word: String,
    pub tipo_word: String,
}

/// Lo que necesita la ventana para abrirse.
pub struct Pedido {
    pub texto: String,
    pub rotulos: Rotulos,
    /// El nombre del `.md` si la nota es un fichero: va en el titulo.
    pub nombre_de_fichero: Option<String>,
    /// Donde tiene que nacer, si viene de un grupo de ventanas (H9).
    pub colocacion: Option<Colocacion>,
    /// Donde esta en este equipo la foto de una ruta del Markdown.
    pub resolver: Box<dyn Fn(&str) -> Option<PathBuf>>,
    /// Copia una foto junto a la nota (para que viaje con ella) y da la
    /// ruta que se escribe en el Markdown.
    pub adjuntar: Box<dyn FnMut(&Path) -> Option<String>>,
    /// Abre la hoja de compartir de la aplicacion con la nota ya guardada.
    pub compartir: Option<Box<dyn FnMut(isize)>>,
    /// Las paginas vivas, los enlaces a hojas y lo demas que pone la
    /// aplicacion (ver `integracion`; vacio, no hace nada).
    pub integracion: crate::integracion::Integracion,
    /// Quien comenta y donde estan los comentarios (ver `comentarios`;
    /// vacio, se comenta pero no se guarda).
    pub comentarios: comentarios::DeComentarios,
}

// ---------------------------------------------------------------------------
// Tamanos

/// Tamanos en veinteavos de punto (lo que mide el RichEdit). La letra de
/// base, 12 pt, son los 16 px del cuerpo de la captura.
const BASE: i32 = 240;
fn tamano_de_titulo(n: u8) -> i32 {
    match n {
        1 => 520,
        2 => 400,
        3 => 320,
        _ => 280,
    }
}
const CODIGO: i32 = 210;
/// Cuanto se sangra una cita o una lista, en veinteavos de punto.
const SANGRIA: i32 = 360;
/// El margen del papel a cada lado de lo que se escribe, a 96 ppp.
const MARGEN_PX: i32 = 24;
/// El alto de un renglon del cuerpo, para colocar el menu `/` debajo.
const RENGLON_PX: i32 = 24;

// ---------------------------------------------------------------------------
// Las ordenes que los procedimientos dejan para el bucle

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Orden {
    Cerrar,
    Comando(u16),
    Cambio,
    Repintar,
    Autoguardar,
    Clic(Boton),
    ClicFuera,
    Tamano,
    TituloHecho,
    Tema,
    /// Mirar las paginas vivas y lo mandado a esta nota (ver `fotos`).
    Vivas,
    /// Unos ficheros soltados encima (en `fotos::SOLTADOS`).
    Soltar,
    /// Un paso del desplazamiento suave (ver `wysiwyg`).
    Deslizar,
    /// Un latido del reproductor de un audio de la nota (`incrustados`).
    Audio,
}

thread_local! {
    static COLA: RefCell<Vec<Orden>> = const { RefCell::new(Vec::new()) };
    static EDIT: Cell<isize> = const { Cell::new(0) };
    static PPP: Cell<i32> = const { Cell::new(96) };
    static ORIGINAL: Cell<isize> = const { Cell::new(0) };
    static VISTA: RefCell<Option<VistaMarco>> = const { RefCell::new(None) };
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
const C_TABLA: u16 = 40;
const C_IMAGEN: u16 = 41;
const C_FECHA: u16 = 42;
const C_FILA_ENCIMA: u16 = 43;
const C_FILA_DEBAJO: u16 = 44;
const C_QUITAR_FILA: u16 = 45;
const C_COL_IZQ: u16 = 46;
const C_COL_DER: u16 = 47;
const C_QUITAR_COL: u16 = 48;
const C_QUITAR_TABLA: u16 = 49;
const C_COMPLETA: u16 = 50;
const C_TITULO: u16 = 51;
const C_COMPARTIR: u16 = 52;
/// Las entradas del menu `/` van de aqui en adelante, en el orden de
/// `md_comandos::CATALOGO`.
const C_BARRA: u16 = 60;
// Las fotos y las hojas de los proyectos (ver `fotos`), despues de la barra.
const C_PAGINA_VIVA: u16 = 100;
const C_ENLACE_HOJA: u16 = 101;
const C_VER_FOTO: u16 = 102;
const C_FOTO_PEQUENA: u16 = 103;
const C_FOTO_MEDIANA: u16 = 104;
const C_FOTO_GRANDE: u16 = 105;
const C_FOTO_COLUMNA: u16 = 106;
const C_QUITAR_FOTO: u16 = 107;
/// Comentar lo elegido y abrir o cerrar el panel (ver `comentarios`).
const C_COMENTAR: u16 = 120;
const C_PANEL_COMENTARIOS: u16 = 121;

const T_REPINTAR: usize = 1;
const T_AUTOGUARDAR: usize = 2;
/// Cada cuanto se guarda solo si hay cambios: si Windows se apaga con la
/// nota abierta, se pierde como mucho esto.
const MS_AUTOGUARDAR: u32 = 30_000;
const MS_RESPIRO: u32 = 60;

// Mensajes del EDIT de siempre que el RichEdit tambien entiende.
const EM_REPLACESEL: u32 = 0x00C2;
const EM_CHARFROMPOS: u32 = 0x00D7;
const EM_POSFROMCHAR: u32 = 0x00D6;
const EM_EMPTYUNDOBUFFER: u32 = 0x00CD;
const EM_SETRECT: u32 = 0x00B3;
const EN_CHANGE: usize = 0x0300;
const EN_KILLFOCUS: usize = 0x0200;
const WM_MOUSELEAVE: u32 = 0x02A3;
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

/// El texto crudo (con las marcas de las tablas), con `\n` donde el control
/// tiene `\r`: las posiciones no cambian (los dos son una unidad).
fn leer(edit: HWND) -> String {
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
}

/// Mete RTF en lo elegido (una tabla). `deshacer`: como un paso que Ctrl+Z
/// quita.
fn poner_rtf(edit: HWND, rtf: &str, deshacer: bool) {
    let st = SETTEXTEX {
        flags: ST_SELECTION | if deshacer { ST_KEEPUNDO } else { ST_DEFAULT },
        codepage: 65001,
    };
    let b: Vec<u8> = rtf.bytes().chain(std::iter::once(0)).collect();
    enviar(edit, EM_SETTEXTEX, &st as *const _ as usize, b.as_ptr() as isize);
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
    let puesto: Vec<u16> = b[desde..hasta_b]
        .iter()
        .map(|&c| if c == b'\n' as u16 { b'\r' as u16 } else { c })
        .collect();
    // Por un rango del documento (TOM), no por lo elegido: con las marcas
    // escondidas (ver `wysiwyg`), el control corre los bordes de lo elegido
    // fuera del texto oculto y cambiaria otra cosa (medido: marcar la casilla
    // `- [ ]` reemplazaba `- [` entero). Sigue siendo un paso de deshacer.
    let mut ole: *mut core::ffi::c_void = std::ptr::null_mut();
    enviar(edit, EM_GETOLEINTERFACE, 0, &mut ole as *mut _ as isize);
    if !ole.is_null() {
        // SAFETY: la interfaz la da el propio control con una referencia de
        // mas, que suelta el `IUnknown` al salir; el rango es de su texto.
        let hecho = unsafe {
            windows::core::IUnknown::from_raw(ole)
                .cast::<ITextDocument>()
                .and_then(|d| d.Range(desde as i32, hasta_a as i32))
                .and_then(|r| r.SetText(&windows::core::BSTR::from_wide(&puesto)))
                .is_ok()
        };
        if hecho {
            elegir(edit, desde + puesto.len(), desde + puesto.len());
            // Un rango del documento no pasa por los mensajes: el espia de
            // las pruebas mira aqui (ver `congelar`).
            congelar::fotograma(edit);
            return;
        }
    }
    elegir(edit, desde, hasta_a);
    let puesto: Vec<u16> = puesto.into_iter().chain(std::iter::once(0)).collect();
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
// El estado

/// Lo que se abre al pulsar: el menu del «+», el de la barra `/` (con
/// donde esta la barra) o el de la flecha del titulo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Abierto {
    Mas,
    Barra(usize),
    Titulo,
    /// La paleta de las celdas (boton «Color» de la tabla).
    Color,
    /// Lo que abre la barra flotante («Aa ▾», listas, emoji) o la letra y
    /// el tamano de la flecha del titulo (ver `wysiwyg`).
    Vivo,
}

/// Lo que el formato necesita saber.
struct Estilos {
    tema: Tema,
    letras: Letras,
    /// La sangria de cada lado que centra la columna de texto, en twips.
    margen: i32,
    /// El tamano de la letra del cuerpo, en veinteavos de punto (`BASE` es
    /// el de siempre): la preferencia de vista de `vista`.
    tamano: i32,
}

struct Estado {
    marco: HWND,
    edit: HWND,
    menu: HWND,
    titulo_edit: Option<HWND>,
    doc: Option<ITextDocument>,
    /// Lo ultimo guardado (Markdown): si el texto es esto, cerrar no
    /// escribe nada.
    guardado: String,
    /// El renglon con las marcas a la vista.
    linea_activa: usize,
    /// Hay un repintado entero pendiente (el temporizador esta en marcha).
    pendiente: bool,
    rotulos: Rotulos,
    nombre_de_fichero: Option<String>,
    estilos: Estilos,
    pintor: Rc<Pintor>,
    ppp: i32,
    abierto: Option<Abierto>,
    /// Una barra `/` cuyo menu se cerro con Esc: no se vuelve a abrir
    /// mientras se siga escribiendo ese comando.
    descartada: Option<usize>,
    /// Donde esta el menu abierto, respecto al marco (para las muestras).
    menu_caja: Caja,
    fotos: Fotos,
    resolver: Box<dyn Fn(&str) -> Option<PathBuf>>,
    adjuntar: Box<dyn FnMut(&Path) -> Option<String>>,
    compartir: Option<Box<dyn FnMut(isize)>>,
    /// A pantalla completa: el estilo y el sitio de antes, para volver.
    completa: Option<(isize, WINDOWPLACEMENT)>,
    /// Sin ensenar nada en pantalla (las pruebas y las muestras).
    oculto: bool,
    en_tabla: bool,
    integracion: crate::integracion::Integracion,
    /// Las paginas vivas cuya hoja ya no esta, por su ruta.
    sin_hoja: std::collections::HashSet<String>,
    /// El renglon de la foto del ultimo clic derecho (su menu).
    foto_del_menu: Option<usize>,
    /// Los comentarios y su panel (ver `comentarios`).
    comentarios: comentarios::Comentarios,
    /// Editar sin marcas, la barra flotante, la letra y el desplazamiento
    /// suave (ver `wysiwyg`).
    vivo: wysiwyg::Vivo,
}

/// Lo que hace falta para pintar el marco y saber que hay bajo el raton.
struct VistaMarco {
    tema: Tema,
    pintor: Rc<Pintor>,
    titulo: String,
    compartir: String,
    de_tabla: [String; 6],
    en_tabla: bool,
    hover: Option<Boton>,
    disp: Marco,
    /// El pincel del fondo del titulo mientras se edita.
    pincel_titulo: HBRUSH,
}

fn formato() -> CHARFORMAT2W {
    let mut f = CHARFORMAT2W::default();
    f.Base.cbSize = std::mem::size_of::<CHARFORMAT2W>() as u32;
    f
}

fn cara(f: &mut CHARFORMAT2W, nombre: &str) {
    f.Base.dwMask |= CFM_FACE;
    f.Base.szFaceName = [0; 32];
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

fn color(c: Rgb) -> COLORREF {
    COLORREF(bgr(c))
}

/// Un tamano de los de la letra de siempre, llevado al tamano elegido (ver
/// `vista`): titulos, codigo, sangrias y aires crecen con la letra, y la
/// nota se ve igual de proporcionada en pequena que en muy grande.
fn a_escala(s: &Estilos, v: i32) -> i32 {
    v * s.tamano / BASE
}

fn letra_base(s: &Estilos) -> CHARFORMAT2W {
    let mut f = formato();
    f.Base.dwMask = CFM_BOLD | CFM_ITALIC | CFM_UNDERLINE | CFM_STRIKEOUT | CFM_HIDDEN | CFM_SIZE | CFM_COLOR | CFM_BACKCOLOR;
    f.Base.dwEffects = CFE_AUTOBACKCOLOR;
    f.Base.yHeight = s.tamano;
    f.Base.crTextColor = color(s.tema.texto);
    cara(&mut f, &s.letras.cuerpo);
    f
}

fn parrafo_base(s: &Estilos) -> PARAFORMAT2 {
    let mut p = PARAFORMAT2::default();
    p.Base.cbSize = std::mem::size_of::<PARAFORMAT2>() as u32;
    p.Base.dwMask = PFM_NUMBERING
        | PFM_STARTINDENT
        | PFM_RIGHTINDENT
        | PFM_OFFSET
        | PFM_SPACEBEFORE
        | PFM_SPACEAFTER
        | PFM_LINESPACING
        | PFM_NUMBERINGSTART
        | PFM_NUMBERINGSTYLE
        | PFM_NUMBERINGTAB;
    p.Base.dxStartIndent = s.margen;
    // El hueco de las tablas anchas va solo a la izquierda; a la derecha,
    // el margen de la columna (mas ancho con las tarjetas, `margen_de`).
    p.Base.dxRightIndent = crate::tabla_ancha::sobra_der_twips();
    p.dySpaceAfter = a_escala(s, 100);
    // Renglones a 1,4 (en veinteavos de renglon): el aire de la captura.
    // Es proporcional: crece solo con la letra.
    p.bLineSpacingRule = 5;
    p.dyLineSpacing = 28;
    p
}

/// El formato de un tramo. `activo` dice si su renglon tiene el cursor.
///
/// Las marcas no se ven **nunca**, tampoco en el renglon del cursor: se
/// edita como en Claude, sin `**` ni `#` (ver `wysiwyg`). El numero de una
/// lista, la casilla y la raya tampoco son letras a la vista: los pinta el
/// parrafo (numeracion de Windows) o el editor encima (`imagenes`).
fn formato_de(estilo: Estilo, activo: bool, s: &Estilos) -> CHARFORMAT2W {
    let t = &s.tema;
    let mut f = formato();
    let b = &mut f.Base;
    match estilo {
        Estilo::Marca | Estilo::Numero | Estilo::Casilla { .. } | Estilo::Regla => {
            b.dwMask = CFM_HIDDEN;
            b.dwEffects = CFE_HIDDEN;
        }
        Estilo::Titulo(n) => {
            b.dwMask = CFM_SIZE | CFM_BOLD;
            b.yHeight = a_escala(s, tamano_de_titulo(n));
            cara(&mut f, &s.letras.titulos);
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
            b.crTextColor = color(t.codigo);
            b.yHeight = a_escala(s, CODIGO);
            f.crBackColor = color(t.fondo_codigo);
            cara(&mut f, "Consolas");
        }
        Estilo::Cita => {
            b.dwMask = CFM_COLOR | CFM_ITALIC;
            b.dwEffects = CFE_ITALIC;
            b.crTextColor = color(t.tenue);
        }
        Estilo::Enlace => {
            b.dwMask = CFM_COLOR | CFM_UNDERLINE;
            b.dwEffects = CFE_UNDERLINE;
            b.crTextColor = color(t.enlace);
        }
        Estilo::Hecha => {
            b.dwMask = CFM_COLOR | CFM_STRIKEOUT;
            b.dwEffects = CFE_STRIKEOUT;
            b.crTextColor = color(t.apagado);
        }
        Estilo::Formula => {
            b.dwMask = CFM_COLOR | CFM_ITALIC;
            b.dwEffects = CFE_ITALIC;
            b.crTextColor = color(t.formula);
            cara(&mut f, "Cambria Math");
        }
        // El texto de una foto que no se puede ensenar: apagado, a la vista;
        // con la foto, fuera (queda la foto sola).
        Estilo::Imagen if activo => {
            b.dwMask = CFM_COLOR;
            b.crTextColor = color(t.tenue);
        }
        Estilo::Imagen => {
            b.dwMask = CFM_HIDDEN;
            b.dwEffects = CFE_HIDDEN;
        }
        Estilo::Cabecera => {
            b.dwMask = CFM_COLOR;
            b.crTextColor = color(t.tenue);
        }
        // La vineta es del parrafo, no de las letras.
        Estilo::Vineta => b.dwMask = CFM_MASK(0),
    }
    f
}

/// El parrafo de un renglon con ese estilo, si cambia algo. El guion de
/// una lista no se ve nunca: la vineta la pone siempre Windows.
fn parrafo_de(estilo: Estilo, _activo: bool, s: &Estilos) -> Option<PARAFORMAT2> {
    let mut p = parrafo_base(s);
    let sangria = a_escala(s, SANGRIA);
    match estilo {
        // La vineta en media sangria y el texto en la sangria entera: alineado
        // con el de las casillas, las citas y los numeros.
        Estilo::Vineta => {
            p.Base.wNumbering = PFN_BULLET;
            p.Base.dxStartIndent = s.margen + sangria / 2;
            p.Base.dxOffset = sangria / 2;
        }
        // La casilla la pinta el editor en la sangria (ver `imagenes`).
        Estilo::Casilla { .. } | Estilo::Cita => p.Base.dxStartIndent = s.margen + sangria,
        Estilo::BloqueCodigo => {
            p.Base.dxStartIndent = s.margen + sangria / 2;
            p.dySpaceAfter = 0;
            p.dyLineSpacing = 24;
        }
        Estilo::Titulo(n) => {
            p.dySpaceBefore = a_escala(s, if n <= 2 { 360 } else { 240 });
            p.dySpaceAfter = a_escala(s, 120);
            p.dyLineSpacing = 22;
        }
        _ => return None,
    }
    Some(p)
}

/// Cada renglon del texto del control: si es de una tabla.
fn renglones_de_tabla(texto: &str) -> Vec<bool> {
    texto
        .split('\n')
        .map(|r| r.chars().any(|c| matches!(c, md_tabla::FILA_ABRE | md_tabla::FILA_CIERRA | md_tabla::CELDA)))
        .collect()
}

/// Pinta el formato. `solo` limita a unos renglones (al moverse el cursor);
/// `None` es todo (tras un cambio).
fn pintar(e: &mut Estado, solo: Option<&[usize]>) {
    let edit = e.edit;
    let texto = leer(edit);
    let ls = md_vivo::lineas(&texto);
    let de_tabla = renglones_de_tabla(&texto);
    let (_, cursor) = seleccion(edit);
    let activa = md_vivo::linea_de(&ls, cursor);
    e.linea_activa = activa;
    let tramos = md_vivo::analizar(&texto);
    let entra = |n: usize| solo.is_none_or(|s| s.contains(&n));

    // Las fotos: cuales se pueden ensenar y lo que miden (con su ancho).
    let (con_foto, sin_foto) = fotos::medir(e, &texto);

    let mascara = enviar(edit, EM_GETEVENTMASK, 0, 0);
    enviar(edit, EM_SETEVENTMASK, 0, ENM_NONE as isize);
    congelar::HONDO.with(|h| h.set(h.get() + 1));
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

    // Primero todo a letra y parrafo normales; luego cada tramo encima. El
    // parrafo no se toca en las tablas: su estructura va en el.
    let base = letra_base(&e.estilos);
    let parrafo = parrafo_base(&e.estilos);
    let mut tramos_sin_tabla: Vec<(usize, usize)> = Vec::new();
    for (n, l) in ls.iter().enumerate() {
        if !entra(n) || de_tabla.get(n).copied().unwrap_or(false) {
            continue;
        }
        match tramos_sin_tabla.last_mut() {
            Some((_, hasta)) if *hasta + 1 == l.desde || *hasta == l.desde => *hasta = l.hasta,
            _ => tramos_sin_tabla.push((l.desde, l.hasta)),
        }
    }
    match solo {
        None => {
            elegir(edit, 0, usize::MAX >> 33);
            poner_formato(edit, &base);
        }
        Some(ns) => {
            for l in ns.iter().filter_map(|n| ls.get(*n)) {
                elegir(edit, l.desde, l.hasta + 1);
                poner_formato(edit, &base);
            }
        }
    }
    for (a, b) in &tramos_sin_tabla {
        elegir(edit, *a, *b + 1);
        poner_parrafo(edit, &parrafo);
    }
    // Un renglon vacio separa parrafos en Markdown; a tamano entero seria un
    // hueco de mas entre cada dos (en la captura no se ve). Fuera del cursor
    // se queda en poco alto; con el cursor vuelve a su tamano para escribir.
    let mut en_codigo = false;
    let u: Vec<u16> = texto.encode_utf16().collect();
    for (n, l) in ls.iter().enumerate() {
        let renglon = String::from_utf16_lossy(&u[l.desde..l.hasta.min(u.len())]);
        if renglon.trim_start().starts_with("```") || renglon.trim_start().starts_with("~~~") {
            en_codigo = !en_codigo;
        }
        if entra(n) && l.desde == l.hasta && n != activa && !en_codigo && !de_tabla.get(n).copied().unwrap_or(false) {
            let mut f = formato();
            f.Base.dwMask = CFM_SIZE;
            f.Base.yHeight = a_escala(&e.estilos, 110);
            elegir(edit, l.desde, l.desde + 1);
            poner_formato(edit, &f);
        }
    }
    // Aire entre una tabla y lo que le sigue (la tabla no tiene «despues»).
    for (n, l) in ls.iter().enumerate().skip(1) {
        if entra(n) && de_tabla[n - 1] && !de_tabla.get(n).copied().unwrap_or(false) {
            let mut p = parrafo_base(&e.estilos);
            p.Base.dwMask = PFM_SPACEBEFORE;
            p.dySpaceBefore = 240;
            elegir(edit, l.desde, l.hasta);
            poner_parrafo(edit, &p);
        }
    }
    for t in tramos.iter().filter(|t| entra(t.linea)) {
        // El texto de una foto que se ve no sale nunca, tampoco con el
        // cursor (como en Word: «que no aparece nada de eso», 1-oct); solo
        // si la foto no se puede ensenar queda su texto a la vista.
        let activo = sin_foto.contains(&t.linea) || (t.linea == activa && t.estilo != Estilo::Imagen);
        elegir(edit, t.desde, t.hasta);
        if !de_tabla.get(t.linea).copied().unwrap_or(false)
            && let Some(p) = parrafo_de(t.estilo, activo, &e.estilos)
        {
            poner_parrafo(edit, &p);
        }
        let f = formato_de(t.estilo, activo, &e.estilos);
        if f.Base.dwMask.0 != 0 {
            poner_formato(edit, &f);
        }
    }
    // El color de las celdas pintadas, que lo de arriba acaba de quitar
    // (la cabecera, las marcas): el de la celda manda.
    tablas::colorear_letras(e, &texto, &ls, &entra);
    // Lo que en Claude no son letras: el titulo entero (tambien sus marcas
    // escondidas, que dan el alto del cursor), los numeros y la sangria de
    // las listas, y las casillas, rayas y citas que se pintan encima.
    wysiwyg::pintar_bloques(e, &texto, &ls, &tramos, &entra, solo.is_none());
    // El aire de encima de cada foto, del alto de la foto.
    let aire = imagenes::AIRE_PX * e.ppp / 96;
    for (n, _, f) in &con_foto {
        if !entra(*n) {
            continue;
        }
        let Some(l) = ls.get(*n) else { continue };
        let mut p = parrafo_base(&e.estilos);
        p.Base.dwMask = PFM_SPACEBEFORE;
        p.dySpaceBefore = (f.alto + 2 * aire) * 1440 / e.ppp;
        elegir(edit, l.desde, l.hasta);
        poner_parrafo(edit, &p);
    }
    // Los documentos, mensajes y audios: su renglon escondido y las marcas
    // de tiempo de las transcripciones (`incrustados`).
    incrustados::formatear(e, &texto, &ls, &con_foto, activa, &entra);
    fotos::poner_puestas(e, &ls, &con_foto, activa, solo.is_none());
    // Lo comentado, resaltado encima de todo lo demas.
    comentarios::resaltar(e, &texto, solo.is_none());

    elegir(edit, sel.0, sel.1);
    enviar(edit, EM_SETSCROLLPOS, 0, &scroll as *const _ as isize);
    // El formato entero, solo (fuera de un `congelado`, que hace lo suyo):
    // al descongelar el control pintaria lo cambiado y luego lo de encima,
    // un parpadeo a los 60 ms de poner una negrita. Se descongela sin pintar
    // y se pinta una vez. Al mover el cursor (unos renglones) no: es a cada
    // tecla y el control ya pinta solo lo suyo.
    let solo_el = solo.is_none() && congelar::HONDO.with(|h| h.get()) == 1 && !e.oculto;
    if solo_el {
        enviar(edit, WM_SETREDRAW, 0, 0);
    }
    if let Some(d) = &e.doc {
        // SAFETY: como arriba.
        unsafe {
            let _ = d.Unfreeze();
            let _ = d.Undo(tomResume.0);
        }
    }
    if solo_el {
        enviar(edit, WM_SETREDRAW, 1, 0);
        // SAFETY: ventana propia.
        unsafe {
            let _ = RedrawWindow(Some(edit), None, None, RDW_INVALIDATE | RDW_UPDATENOW);
        }
    }
    enviar(edit, EM_SETEVENTMASK, 0, mascara);
    congelar::HONDO.with(|h| h.set(h.get() - 1));
    // Las tarjetas de los comentarios, con el texto ya medido (dentro de un
    // `congelado` lo hace el al soltar).
    if congelar::HONDO.with(|h| h.get()) == 0 {
        comentarios::componer(e);
    }
    congelar::fotograma(edit);
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
        imagenes::repintar(e.edit);
    }
}

// ---------------------------------------------------------------------------
// Tablas

/// Las alineaciones de una tabla del control: las de los parrafos de su
/// primera fila (el texto no las lleva).
fn alineaciones_de(e: &Estado, t: &md_tabla::TablaEnControl) -> Vec<Alineacion> {
    let Some(doc) = &e.doc else {
        return Vec::new();
    };
    t.celdas
        .first()
        .map(|fila| {
            fila.iter()
                .map(|&p| {
                    // SAFETY: rango del documento vivo del control.
                    let a = unsafe {
                        doc.Range(p as i32, p as i32)
                            .and_then(|r| r.GetPara())
                            .and_then(|pa| pa.GetAlignment())
                            .unwrap_or(0)
                    };
                    match a {
                        1 => Alineacion::Centro,
                        2 => Alineacion::Derecha,
                        _ => Alineacion::Izquierda,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// El Markdown de lo que hay en el control: lo que se guarda.
fn markdown(e: &Estado) -> String {
    let texto = leer(e.edit);
    let tablas = md_tabla::tablas_en_control(&texto);
    md_tabla::de_control_con(&texto, &mut |k, t| tablas::completar(e, &tablas[k], t))
}

fn estilo_tabla(e: &Estado) -> EstiloTabla<'_> {
    EstiloTabla {
        raya: e.estilos.tema.tabla_raya,
        cabecera: e.estilos.tema.tabla_cabecera,
        texto: e.estilos.tema.texto,
        letra: &e.estilos.letras.cuerpo,
    }
}

/// Mete el Markdown en el control: el texto con un renglon vacio donde va
/// cada tabla, y cada tabla en su renglon. Sin deshacer: es el principio.
fn cargar(e: &mut Estado, md: &str) {
    let pc = md_tabla::para_control(&md.replace("\r\n", "\n"));
    let mascara = enviar(e.edit, EM_GETEVENTMASK, 0, 0);
    enviar(e.edit, EM_SETEVENTMASK, 0, ENM_NONE as isize);
    poner_todo(e.edit, &pc.texto);
    for (pos, t) in pc.tablas.iter().rev() {
        elegir(e.edit, *pos, *pos + 1);
        tablas::poner_tabla(e, t, false);
    }
    // Una tabla ancha pudo abrir el hueco de la izquierda (`tabla_ancha`).
    e.estilos.margen = margen_de(e);
    enviar(e.edit, EM_EMPTYUNDOBUFFER, 0, 0);
    enviar(e.edit, EM_SETEVENTMASK, 0, mascara);
}

/// La tabla y la celda del cursor, si esta en una.
fn celda_del_cursor(e: &Estado) -> Option<(md_tabla::TablaEnControl, usize, usize)> {
    let texto = leer(e.edit);
    if !texto.contains(md_tabla::FILA_ABRE) {
        return None;
    }
    let tablas = md_tabla::tablas_en_control(&texto);
    let (_, cursor) = seleccion(e.edit);
    let (k, f, c) = md_tabla::celda_en(&tablas, cursor)?;
    let mut t = tablas.into_iter().nth(k)?;
    t.tabla.alineaciones = alineaciones_de(e, &t);
    Some((t, f, c))
}

/// Pone el cursor al principio de la celda `(f, c)` de la tabla que empieza
/// en `desde`.
fn ir_a_celda(e: &Estado, desde: usize, f: usize, c: usize) {
    let texto = leer(e.edit);
    if let Some(t) = md_tabla::tablas_en_control(&texto).into_iter().find(|t| t.desde == desde)
        && let Some(p) = t.celdas.get(f).and_then(|fila| fila.get(c).or(fila.last()))
    {
        elegir(e.edit, *p, *p);
    }
}

/// **Los «+» de una tabla** (`asas`): una fila mas debajo de la ultima o
/// una columna mas a la derecha de la ultima, con el cursor en su primera
/// celda nueva.
fn anadir_al_final(e: &mut Estado, desde: usize, fila: bool) {
    let texto = leer(e.edit);
    let Some(t) = md_tabla::tablas_en_control(&texto).into_iter().find(|t| t.desde == desde) else {
        return;
    };
    let f = t.celdas.len().saturating_sub(1);
    let c = t.celdas.first().map_or(0, |fila| fila.len().saturating_sub(1));
    ir_a_celda(e, desde, if fila { f } else { 0 }, c);
    operar_tabla(e, if fila { OpTabla::FilaDebajo } else { OpTabla::ColDerecha });
    if fila {
        ir_a_celda(e, desde, f + 1, 0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpTabla {
    FilaEncima,
    FilaDebajo,
    QuitarFila,
    ColIzquierda,
    ColDerecha,
    QuitarCol,
    QuitarTabla,
}

/// Cambia la tabla del cursor: se cambia el modelo y se vuelve a escribir
/// entera en su sitio, como un paso de deshacer.
fn operar_tabla(e: &mut Estado, op: OpTabla) {
    let Some((tc, f, c)) = celda_del_cursor(e) else {
        return;
    };
    let mut t: Tabla = tablas::modelo(e, &tc);
    if op == OpTabla::QuitarTabla {
        elegir(e.edit, tc.desde, tc.hasta);
        enviar(e.edit, EM_REPLACESEL, 1, ancho_nulo("").as_ptr() as isize);
        return;
    }
    let destino = match op {
        OpTabla::FilaEncima => {
            t.insertar_fila(f);
            (f, c)
        }
        OpTabla::FilaDebajo => {
            t.insertar_fila(f + 1);
            (f + 1, c)
        }
        OpTabla::QuitarFila => {
            if !t.quitar_fila(f) {
                return;
            }
            (f.min(t.filas.len() - 1), c)
        }
        OpTabla::ColIzquierda => {
            t.insertar_columna(c);
            (f, c)
        }
        OpTabla::ColDerecha => {
            t.insertar_columna(c + 1);
            (f, c + 1)
        }
        OpTabla::QuitarCol => {
            if !t.quitar_columna(c) {
                return;
            }
            (f, c.min(t.columnas() - 1))
        }
        OpTabla::QuitarTabla => unreachable!(),
    };
    elegir(e.edit, tc.desde, tc.hasta);
    tablas::poner_tabla(e, &t, true);
    ir_a_celda(e, tc.desde, destino.0, destino.1);
    apuntar(Orden::Cambio);
}

/// Mete una tabla nueva (la del movil: cabecera y dos filas, dos columnas)
/// en un renglon propio, con el cursor en su primera celda.
fn insertar_tabla(e: &mut Estado) {
    meter_tabla(e, &Tabla::nueva(3, 2));
}

/// Mete la tabla `t` en un renglon propio (o detras de la tabla del
/// cursor), con el cursor en su primera celda: la nueva y la pegada.
fn meter_tabla(e: &mut Estado, t: &Tabla) {
    let texto = leer(e.edit);
    let (_, b) = seleccion(e.edit);
    let pos = if let Some((t, _, _)) = celda_del_cursor(e) {
        t.hasta
    } else {
        let ls = md_vivo::lineas(&texto);
        let l = ls[md_vivo::linea_de(&ls, b)];
        if l.desde == l.hasta {
            l.desde
        } else {
            elegir(e.edit, l.hasta, l.hasta);
            enviar(e.edit, EM_REPLACESEL, 1, ancho_nulo("\r").as_ptr() as isize);
            l.hasta + 1
        }
    };
    elegir(e.edit, pos, pos);
    tablas::poner_tabla(e, t, true);
    ir_a_celda(e, pos, 0, 0);
    apuntar(Orden::Cambio);
}

// ---------------------------------------------------------------------------
// Las ordenes

fn titulo_de(e: &Estado, md: &str) -> String {
    match md_vivo::titulo(md) {
        t if t.is_empty() => e.nombre_de_fichero.clone().unwrap_or_else(|| e.rotulos.nueva.clone()),
        t => t,
    }
}

/// El titulo en la cabecera y en la barra de tareas.
fn poner_titulo(e: &Estado, md: &str) {
    let t = titulo_de(e, md);
    let en_barra = match &e.nombre_de_fichero {
        Some(f) => format!("{f} — {}", e.rotulos.sufijo),
        None => format!("{t} — {}", e.rotulos.sufijo),
    };
    // SAFETY: ventana propia; la cadena vive durante la llamada.
    unsafe {
        let _ = SetWindowTextW(e.marco, &HSTRING::from(en_barra));
    }
    let cambio = VISTA.with(|v| {
        let mut v = v.borrow_mut();
        let v = v.as_mut()?;
        (v.titulo != t).then(|| v.titulo = t)
    });
    if cambio.is_some() {
        recolocar(e.marco);
    }
}

fn guardar_ahora(e: &mut Estado, guardar: &mut dyn FnMut(&str) -> bool) -> bool {
    let texto = markdown(e);
    if texto == e.guardado {
        // Los comentarios van aparte: pueden cambiar sin tocar el texto.
        comentarios::guardar(e, &texto);
        return true;
    }
    if guardar(&texto) {
        poner_titulo(e, &texto);
        // Despues de la nota: una nota nueva ya tiene sitio para ellos.
        comentarios::guardar(e, &texto);
        e.guardado = texto;
        true
    } else {
        false
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
        Some(pixpin_codec::portapapeles::ContenidoPortapapeles::Texto(t)) if es_direccion(t.trim()) => {
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

/// Mete un renglon propio en el sitio del cursor (una foto): en el renglon
/// si esta vacio, o en uno nuevo debajo. Fuera de una tabla.
fn insertar_renglon(e: &Estado, renglon: &str) {
    let texto = leer(e.edit);
    let (_, b) = seleccion(e.edit);
    if let Some((t, _, _)) = celda_del_cursor(e) {
        elegir(e.edit, t.hasta, t.hasta);
        let r = ancho_nulo(&format!("{renglon}\r"));
        enviar(e.edit, EM_REPLACESEL, 1, r.as_ptr() as isize);
        elegir(e.edit, t.hasta + renglon.encode_utf16().count(), t.hasta + renglon.encode_utf16().count());
        return;
    }
    let ls = md_vivo::lineas(&texto);
    let l = ls[md_vivo::linea_de(&ls, b)];
    let (pos, puesto) = if l.desde == l.hasta {
        (l.desde, renglon.to_string())
    } else {
        (l.hasta, format!("\r{renglon}"))
    };
    elegir(e.edit, pos, pos);
    enviar(e.edit, EM_REPLACESEL, 1, ancho_nulo(&puesto).as_ptr() as isize);
}

fn fecha_de_hoy(e: &Estado) -> String {
    let meses: Vec<String> = e.rotulos.meses.split_whitespace().map(String::from).collect();
    let (d, m, a) = md_comandos::dia_de(pixpin_shell::entorno::ahora_local_ms());
    md_comandos::fecha(d, m, a, &meses)
}

fn nombre_de_bloque(r: &Rotulos, b: Bloque) -> String {
    match b {
        Bloque::Titulo1 => r.titulo1.clone(),
        Bloque::Titulo2 => r.titulo2.clone(),
        Bloque::Titulo3 => r.titulo3.clone(),
        Bloque::Lista => r.lista.clone(),
        Bloque::Numerada => r.numerada.clone(),
        Bloque::Casillas => r.casilla.clone(),
        Bloque::Cita => r.cita.clone(),
        Bloque::Codigo => r.bloque.clone(),
        Bloque::Tabla => r.tabla.clone(),
        Bloque::Imagen => r.imagen.clone(),
        Bloque::Fecha => r.fecha.clone(),
        Bloque::Separador => r.raya.clone(),
        Bloque::Pagina => r.pagina_viva.clone(),
        Bloque::EnlaceHoja => r.enlace_hoja.clone(),
        Bloque::Documento => r.incrustados.documento.clone(),
        Bloque::DelChat => r.incrustados.del_chat.clone(),
        Bloque::Audio => r.incrustados.audio.clone(),
    }
}

fn dibujo_de_bloque(b: Bloque) -> (Dibujo, &'static str) {
    match b {
        Bloque::Titulo1 => (Dibujo::Letras("H1"), "#"),
        Bloque::Titulo2 => (Dibujo::Letras("H2"), "##"),
        Bloque::Titulo3 => (Dibujo::Letras("H3"), "###"),
        Bloque::Lista => (Dibujo::Icono(Icono::Lista), "-"),
        Bloque::Numerada => (Dibujo::Icono(Icono::Numerada), "1."),
        Bloque::Casillas => (Dibujo::Icono(Icono::Casillas), "[ ]"),
        Bloque::Cita => (Dibujo::Icono(Icono::Cita), ">"),
        Bloque::Codigo => (Dibujo::Icono(Icono::Codigo), "```"),
        Bloque::Tabla => (Dibujo::Icono(Icono::Tabla), ""),
        Bloque::Imagen => (Dibujo::Icono(Icono::Imagen), ""),
        Bloque::Fecha => (Dibujo::Icono(Icono::Fecha), ""),
        Bloque::Separador => (Dibujo::Icono(Icono::Separador), "---"),
        Bloque::Pagina => (Dibujo::Icono(Icono::Pagina), ""),
        Bloque::EnlaceHoja => (Dibujo::Icono(Icono::Enlace), ""),
        Bloque::Documento => (Dibujo::Icono(Icono::Documento), ""),
        Bloque::DelChat => (Dibujo::Icono(Icono::Chat), ""),
        Bloque::Audio => (Dibujo::Icono(Icono::Audio), ""),
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
            actualizar_barra(e);
        }
        Orden::Repintar => {
            e.pendiente = false;
            pintar(e, None);
            let md = markdown(e);
            poner_titulo(e, &md);
            imagenes::repintar(e.edit);
        }
        Orden::Autoguardar => {
            guardar_ahora(e, guardar);
        }
        Orden::Cerrar => {
            cerrar_menu(e);
            if guardar_ahora(e, guardar)
                || pixpin_shell::dialogo::preguntar(e.marco, &e.rotulos.sufijo, &e.rotulos.no_guardada)
            {
                // SAFETY: ventana propia; su WM_DESTROY acaba el bucle.
                unsafe {
                    let _ = DestroyWindow(e.marco);
                }
            }
        }
        Orden::Comando(c) => {
            comando(e, c, guardar);
            actualizar_en_tabla(e);
        }
        Orden::Clic(b) => {
            clic(e, b, guardar);
            actualizar_en_tabla(e);
        }
        Orden::ClicFuera => {
            cerrar_menu(e);
            wysiwyg::esconder_barra(e);
        }
        Orden::Deslizar => wysiwyg::paso(e),
        Orden::Audio => incrustados::latido(e),
        Orden::Tamano => {
            cerrar_menu(e);
            let margen = margen_de(e);
            if margen != e.estilos.margen {
                e.estilos.margen = margen;
                apuntar(Orden::Cambio);
            }
            // Lo que cabe cambia con la ventana: cada tabla a su sitio.
            tablas::desplazar::colocar_todas(e, None);
            comentarios::componer(e);
        }
        Orden::TituloHecho => acabar_titulo(e, true),
        Orden::Tema => {
            let nuevo = Tema::del_sistema();
            if nuevo != e.estilos.tema {
                cambiar_tema(e, nuevo);
            }
        }
        Orden::Vivas => {
            fotos::vigilar(e);
            incrustados::vigilar(e);
        }
        Orden::Soltar => fotos::soltar_lo_soltado(e),
    }
}

/// Las ordenes que abren un cuadro de Windows (elegir fichero, guardar,
/// el selector de hojas) o mueven la ventana: no se congelan enteras (la
/// nota se quedaria sin pintar detras); lo que meten se congela dentro.
fn es_modal(c: u16) -> bool {
    let de_la_barra = (C_BARRA..C_BARRA + md_comandos::CATALOGO.len() as u16).contains(&c)
        && matches!(
            md_comandos::CATALOGO[(c - C_BARRA) as usize].0,
            Bloque::Imagen | Bloque::Pagina | Bloque::EnlaceHoja | Bloque::Documento | Bloque::DelChat | Bloque::Audio
        );
    de_la_barra
        || matches!(
            c,
            C_IMAGEN | C_PAGINA_VIVA | C_ENLACE_HOJA | C_COPIA_MD | C_COMPARTIR | C_CERRAR | C_COMPLETA | C_GUARDAR | C_VER_FOTO
                | incrustados::C_DOCUMENTO | incrustados::C_DEL_CHAT | incrustados::C_AUDIO | exportar::C_EXPORTAR_WORD
        )
}

/// Una orden (del menu, de una tecla, de la barra flotante): con el pintado
/// congelado y el formato puesto antes de ensenar nada (ver `congelar`).
fn comando(e: &mut Estado, c: u16, guardar: &mut dyn FnMut(&str) -> bool) {
    if es_modal(c) {
        comando_suelto(e, c, guardar);
    } else {
        congelar::congelado(e, congelar::Pintado::Entero, |e| comando_suelto(e, c, guardar));
    }
}

fn comando_suelto(e: &mut Estado, c: u16, guardar: &mut dyn FnMut(&str) -> bool) {
    match c {
        // Con las marcas escondidas (ver `wysiwyg`): se ponen y se quitan sin
        // que se vean, y un enlace pide su direccion en un cuadro.
        C_NEGRITA => wysiwyg::formato(e, "**"),
        C_CURSIVA => wysiwyg::formato(e, "*"),
        C_TACHADO => wysiwyg::formato(e, "~~"),
        C_CODIGO => wysiwyg::formato(e, "`"),
        C_FORMULA => wysiwyg::formato(e, "$"),
        C_ENLACE => wysiwyg::empezar_enlace(e),
        C_T1 => wysiwyg::bloque(e, "# "),
        C_T2 => wysiwyg::bloque(e, "## "),
        C_T3 => wysiwyg::bloque(e, "### "),
        C_LISTA => wysiwyg::bloque(e, "- "),
        C_NUMERADA => wysiwyg::bloque(e, "1. "),
        C_CASILLA => wysiwyg::bloque(e, "- [ ] "),
        C_CITA => wysiwyg::bloque(e, "> "),
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
        C_CORTAR => wysiwyg::cortar(e),
        C_COPIAR => {
            enviar(e.edit, WM_COPY, 0, 0);
        }
        // Se pega como texto: el Markdown es texto, y un formato pegado de
        // una pagina web no se guardaria de todos modos.
        // Una imagen (una captura) o fotos copiadas en el Explorador entran
        // como fotos de la nota (`fotos::pegar`), y una tabla de una hoja de
        // calculo, Word o una pagina, como tabla (`tablas::pegar`).
        C_PEGAR => {
            if !tablas::pegar(e) && !fotos::pegar(e) {
                // El texto lo mete el editor (como un paso de deshacer) y no
                // el control: asi entra ya con sus marcas escondidas.
                match pixpin_codec::portapapeles::leer() {
                    Some(pixpin_codec::portapapeles::ContenidoPortapapeles::Texto(t)) if celda_del_cursor(e).is_none() => {
                        wysiwyg::pegar_texto(e, &t)
                    }
                    _ => {
                        wysiwyg::antes_de_pegar(e);
                        enviar(e.edit, EM_PASTESPECIAL, CF_UNICODETEXT, 0);
                    }
                }
            }
        }
        C_GUARDAR => {
            guardar_ahora(e, guardar);
        }
        C_COPIA_MD => {
            let texto = markdown(e);
            let nombre = match md_vivo::titulo(&texto) {
                t if t.is_empty() => e.rotulos.nueva.clone(),
                t => sin_prohibidas(&t),
            };
            if let Some(ruta) = pixpin_shell::guardar::pedir_ruta_para(e.marco, &nombre, &e.rotulos.tipo_md, "md")
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
        C_TABLA => insertar_tabla(e),
        C_IMAGEN => {
            let rutas = pixpin_shell::elegir::pedir_imagenes(e.marco);
            fotos::meter(e, &rutas);
        }
        C_PAGINA_VIVA => fotos::hoja_de_un_proyecto(e, false),
        C_ENLACE_HOJA => fotos::hoja_de_un_proyecto(e, true),
        C_VER_FOTO | C_FOTO_PEQUENA | C_FOTO_MEDIANA | C_FOTO_GRANDE | C_FOTO_COLUMNA | C_QUITAR_FOTO => {
            fotos::orden_de_foto(e, c)
        }
        C_FECHA => {
            let f = ancho_nulo(&fecha_de_hoy(e));
            enviar(e.edit, EM_REPLACESEL, 1, f.as_ptr() as isize);
        }
        C_FILA_ENCIMA => operar_tabla(e, OpTabla::FilaEncima),
        C_FILA_DEBAJO => operar_tabla(e, OpTabla::FilaDebajo),
        C_QUITAR_FILA => operar_tabla(e, OpTabla::QuitarFila),
        C_COL_IZQ => operar_tabla(e, OpTabla::ColIzquierda),
        C_COL_DER => operar_tabla(e, OpTabla::ColDerecha),
        C_QUITAR_COL => operar_tabla(e, OpTabla::QuitarCol),
        C_QUITAR_TABLA => operar_tabla(e, OpTabla::QuitarTabla),
        C_COMPLETA => alternar_completa(e),
        C_TITULO => empezar_titulo(e),
        C_COMPARTIR => {
            guardar_ahora(e, guardar);
            let marco = e.marco.0 as isize;
            if let Some(c) = e.compartir.as_mut() {
                c(marco);
            }
        }
        c if (C_BARRA..C_BARRA + md_comandos::CATALOGO.len() as u16).contains(&c) => {
            elegir_de_la_barra(e, md_comandos::CATALOGO[(c - C_BARRA) as usize].0, guardar);
        }
        // Combinar, separar y los colores de las celdas.
        c if tablas::es_suyo(c) => tablas::comando(e, c),
        c if wysiwyg::es_suyo(c) => wysiwyg::comando(e, c),
        // Documentos, mensajes del chat y audios (`incrustados`).
        c if incrustados::es_suyo(c) => incrustados::comando(e, c),
        exportar::C_EXPORTAR_WORD => exportar::a_word(e),
        C_COMENTAR => comentarios::comentar(e),
        C_PANEL_COMENTARIOS => comentarios::alternar_panel(e),
        _ => {}
    }
    // Ni mientras se escribe el titulo ni un comentario.
    if e.titulo_edit.is_none() && e.comentarios.compositor.is_none() && e.vivo.enlace.is_none() {
        // SAFETY: foco a un hijo propio.
        unsafe {
            let _ = SetFocus(Some(e.edit));
        }
    }
}

/// Cambia la barra `/` y lo tecleado por el bloque elegido.
fn elegir_de_la_barra(e: &mut Estado, b: Bloque, guardar: &mut dyn FnMut(&str) -> bool) {
    let texto = leer(e.edit);
    let (_, cursor) = seleccion(e.edit);
    let Some((barra, _)) = md_comandos::consulta(&texto, cursor) else {
        return;
    };
    cerrar_menu(e);
    let (antes, despues) = md_comandos::plantilla(b).unwrap_or(("", ""));
    let (nuevo, pos) = md_comandos::poner(&texto, barra, cursor, antes, despues);
    aplicar_cambio(e.edit, &texto, &nuevo);
    elegir(e.edit, pos, pos);
    match b {
        Bloque::Tabla => comando(e, C_TABLA, guardar),
        Bloque::Imagen => comando(e, C_IMAGEN, guardar),
        Bloque::Fecha => comando(e, C_FECHA, guardar),
        Bloque::Pagina => comando(e, C_PAGINA_VIVA, guardar),
        Bloque::EnlaceHoja => comando(e, C_ENLACE_HOJA, guardar),
        Bloque::Documento => comando(e, incrustados::C_DOCUMENTO, guardar),
        Bloque::DelChat => comando(e, incrustados::C_DEL_CHAT, guardar),
        Bloque::Audio => comando(e, incrustados::C_AUDIO, guardar),
        _ => {}
    }
}

fn clic(e: &mut Estado, b: Boton, guardar: &mut dyn FnMut(&str) -> bool) {
    let habia = e.abierto;
    cerrar_menu(e);
    match b {
        Boton::Titulo => empezar_titulo(e),
        Boton::TituloMenu if habia != Some(Abierto::Titulo) => {
            let r = &e.rotulos;
            let m = &r.mayus;
            let entradas = vec![
                entrada(C_TITULO, Dibujo::Icono(Icono::Lapiz), &r.cambiar_titulo, ""),
                entrada(C_GUARDAR, Dibujo::Icono(Icono::Disco), &r.guardar, "Ctrl+S"),
                entrada(C_COPIA_MD, Dibujo::Icono(Icono::Copia), &r.copia_md, &format!("Ctrl+{m}+S")),
            ];
            // La letra y el tamano de la vista (no van en el texto).
            // Y exportar a Word (`exportar`), junto a la copia .md.
            let entradas: Vec<Entrada> = entradas
                .into_iter()
                .chain([exportar::entrada(e)])
                .chain(wysiwyg::entradas_de_vista(e))
                .collect();
            abrir_menu_en_boton(e, Abierto::Titulo, Boton::Titulo, entradas, None);
        }
        Boton::Mas if habia != Some(Abierto::Mas) => {
            let r = &e.rotulos;
            // Los de la captura, sin el desplegable (ver H12).
            let entradas = vec![
                entrada(C_LISTA, Dibujo::Icono(Icono::Lista), &r.lista, ""),
                entrada(C_CASILLA, Dibujo::Icono(Icono::Casillas), &r.casilla, ""),
                entrada(C_FECHA, Dibujo::Icono(Icono::Fecha), &r.fecha, ""),
                entrada(C_NUMERADA, Dibujo::Icono(Icono::Numerada), &r.numerada, ""),
                // Las hojas de los proyectos, si la aplicacion las da.
                entrada(C_PAGINA_VIVA, Dibujo::Icono(Icono::Pagina), &r.pagina_viva, ""),
                entrada(C_ENLACE_HOJA, Dibujo::Icono(Icono::Enlace), &r.enlace_hoja, ""),
            ];
            let entradas = if e.integracion.hojas.is_some() {
                entradas
            } else {
                entradas.into_iter().filter(|x| x.id != C_PAGINA_VIVA && x.id != C_ENLACE_HOJA).collect()
            };
            // Documento, del chat y audio, si la aplicacion los da.
            let entradas: Vec<Entrada> = entradas.into_iter().chain(incrustados::entradas_del_mas(e)).collect();
            let r = &e.rotulos;
            let pista = Some(r.pista_barra.clone());
            abrir_menu_en_boton(e, Abierto::Mas, Boton::Mas, entradas, pista);
        }
        Boton::TituloMenu | Boton::Mas => {}
        Boton::Compartir => comando(e, C_COMPARTIR, guardar),
        Boton::Minimizar => {
            // SAFETY: ventana propia.
            unsafe {
                let _ = ShowWindow(e.marco, SW_MINIMIZE);
            }
        }
        Boton::PantallaCompleta => alternar_completa(e),
        Boton::Cerrar => apuntar(Orden::Cerrar),
        Boton::Tabla => comando(e, C_TABLA, guardar),
        Boton::Imagen => comando(e, C_IMAGEN, guardar),
        Boton::Casillas => comando(e, C_CASILLA, guardar),
        Boton::Comentar => comando(e, C_COMENTAR, guardar),
        Boton::Comentarios => comando(e, C_PANEL_COMENTARIOS, guardar),
        Boton::FilaMas => comando(e, C_FILA_DEBAJO, guardar),
        Boton::FilaMenos => comando(e, C_QUITAR_FILA, guardar),
        Boton::ColumnaMas => comando(e, C_COL_DER, guardar),
        Boton::ColumnaMenos => comando(e, C_QUITAR_COL, guardar),
        Boton::Combinar => tablas::combinar_o_separar(e),
        Boton::Color if habia != Some(Abierto::Color) => {
            let entradas = tablas::entradas_de_color(e);
            let pista = Some(tablas::pista_de_color(e));
            abrir_menu_en_boton(e, Abierto::Color, Boton::Color, entradas, pista);
        }
        Boton::Color => {}
    }
}

fn entrada(id: u16, dibujo: Dibujo, texto: &str, atajo: &str) -> Entrada {
    Entrada {
        id,
        dibujo,
        texto: texto.to_string(),
        atajo: atajo.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Los menus que se despliegan

fn abrir_menu_en_boton(e: &mut Estado, que: Abierto, b: Boton, entradas: Vec<Entrada>, pista: Option<String>) {
    let Some(c) = VISTA.with(|v| v.borrow().as_ref().and_then(|v| v.disp.caja(b))) else {
        return;
    };
    let mut p = POINT { x: c.x, y: c.y };
    // SAFETY: conversion de coordenadas de una ventana propia.
    unsafe {
        let _ = ClientToScreen(e.marco, &mut p);
    }
    abrir_menu(e, que, entradas, pista, p, c.al + 4 * e.ppp / 96);
}

fn abrir_menu(e: &mut Estado, que: Abierto, entradas: Vec<Entrada>, pista: Option<String>, ancla: POINT, alto_ancla: i32) {
    let esc = e.pintor.escala;
    let mut m = Menu {
        entradas: Vec::new(),
        pista,
        elegida: 0,
    };
    // El mismo menu que se filtra conserva la entrada elegida.
    let viejo = menu::VISTA.with(|v| v.borrow().as_ref().map(|v| v.menu.clone()));
    if let (Some(Abierto::Barra(a)), Abierto::Barra(b), Some(vm)) = (e.abierto, que, viejo)
        && a == b
    {
        m = vm;
    }
    m.cambiar(entradas);
    // SAFETY: DC de la pantalla, pedido y devuelto aqui, para medir.
    let (an, al) = unsafe {
        let dc = GetDC(None);
        let pintor = e.pintor.clone();
        let r = m.tamano(esc, &|s| pintor.medir(dc, pintor.letra, s));
        ReleaseDC(None, dc);
        r
    };
    let (x, y) = menu::colocar((ancla.x, ancla.y), alto_ancla, (an, al), menu::pantalla_de(ancla));
    let mut dentro = POINT { x, y };
    // SAFETY: conversion de coordenadas de una ventana propia.
    unsafe {
        let _ = ScreenToClient(e.marco, &mut dentro);
    }
    e.menu_caja = Caja {
        x: dentro.x,
        y: dentro.y,
        an,
        al,
    };
    menu::VISTA.with(|v| {
        *v.borrow_mut() = Some(menu::Vista {
            menu: m,
            tema: e.estilos.tema,
            pintor: e.pintor.clone(),
            ancho: an,
        })
    });
    e.abierto = Some(que);
    if !e.oculto {
        menu::ensenar(e.menu, x, y, an, al);
    }
}

fn cerrar_menu(e: &mut Estado) {
    if e.abierto.take().is_some() {
        menu::esconder(e.menu);
        menu::VISTA.with(|v| *v.borrow_mut() = None);
    }
}

fn elegir_del_menu(e: &mut Estado, guardar: &mut dyn FnMut(&str) -> bool) {
    let id = menu::VISTA.with(|v| v.borrow().as_ref().and_then(|v| v.menu.elegida()));
    let que = e.abierto;
    if !matches!(que, Some(Abierto::Barra(_))) {
        cerrar_menu(e);
    }
    if let Some(id) = id {
        comando(e, id, guardar);
    }
}

/// Abre, filtra o cierra el menu `/` segun lo que haya tecleado detras de
/// una barra al principio del renglon del cursor.
fn actualizar_barra(e: &mut Estado) {
    let texto = leer(e.edit);
    let (a, b) = seleccion(e.edit);
    let consulta = if a == b { md_comandos::consulta(&texto, b) } else { None };
    let Some((barra, q)) = consulta else {
        e.descartada = None;
        if matches!(e.abierto, Some(Abierto::Barra(_))) {
            cerrar_menu(e);
        }
        return;
    };
    if e.descartada == Some(barra) {
        return;
    }
    let r = e.rotulos.clone();
    let nombre = |b: Bloque| nombre_de_bloque(&r, b);
    let bloques = md_comandos::buscar(&q, &nombre);
    if bloques.is_empty() {
        cerrar_menu(e);
        return;
    }
    let entradas: Vec<Entrada> = bloques
        .iter()
        .map(|b| {
            let i = md_comandos::CATALOGO.iter().position(|(x, _)| x == b).unwrap_or(0);
            let (dibujo, atajo) = dibujo_de_bloque(*b);
            entrada(C_BARRA + i as u16, dibujo, &nombre(*b), atajo)
        })
        .collect();
    let mut p = POINT::default();
    enviar(e.edit, EM_POSFROMCHAR, &mut p as *mut _ as usize, barra as isize);
    // SAFETY: conversion de coordenadas de una ventana propia.
    unsafe {
        let _ = ClientToScreen(e.edit, &mut p);
    }
    abrir_menu(e, Abierto::Barra(barra), entradas, None, p, RENGLON_PX * e.ppp / 96);
}

// ---------------------------------------------------------------------------
// El titulo en la cabecera

fn empezar_titulo(e: &mut Estado) {
    if e.titulo_edit.is_some() {
        return;
    }
    let Some((c, titulo, letra)) = VISTA.with(|v| {
        v.borrow()
            .as_ref()
            .and_then(|v| Some((v.disp.caja(Boton::Titulo)?, v.titulo.clone(), v.pintor.letra)))
    }) else {
        return;
    };
    let esc = e.pintor.escala;
    let lado = (8.0 * esc) as i32;
    // SAFETY: control hijo propio; se destruye en `acabar_titulo`.
    unsafe {
        let Ok(h) = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("EDIT"),
            &HSTRING::from(titulo.as_str()),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            c.x + lado,
            c.y + (c.al - (18.0 * esc) as i32) / 2,
            (c.an + (22.0 * esc) as i32 - lado).max(lado * 8),
            (18.0 * esc) as i32,
            Some(e.marco),
            None,
            None,
            None,
        ) else {
            return;
        };
        SendMessageW(h, WM_SETFONT, Some(WPARAM(letra.0 as usize)), Some(LPARAM(1)));
        SendMessageW(h, 0x00B1, Some(WPARAM(0)), Some(LPARAM(-1)));
        let _ = SetFocus(Some(h));
        e.titulo_edit = Some(h);
    }
}

fn acabar_titulo(e: &mut Estado, poner: bool) {
    let Some(h) = e.titulo_edit.take() else {
        return;
    };
    // SAFETY: control propio; el bufer es local.
    let nuevo = unsafe {
        let n = GetWindowTextLengthW(h).max(0) as usize;
        let mut b = vec![0u16; n + 1];
        let copiados = GetWindowTextW(h, &mut b).max(0) as usize;
        let _ = DestroyWindow(h);
        String::from_utf16_lossy(&b[..copiados])
    };
    if poner {
        let texto = leer(e.edit);
        if let Some(n) = md_vivo::con_titulo(&texto, &nuevo) {
            let sel = seleccion(e.edit);
            aplicar_cambio(e.edit, &texto, &n);
            let corrido = n.encode_utf16().count() as isize - texto.encode_utf16().count() as isize;
            let mover = |p: usize| (p as isize + corrido).max(0) as usize;
            elegir(e.edit, mover(sel.0), mover(sel.1));
        }
    }
    // SAFETY: foco a un hijo propio.
    unsafe {
        let _ = SetFocus(Some(e.edit));
        let _ = InvalidateRect(Some(e.marco), None, false);
    }
}

// ---------------------------------------------------------------------------
// Pantalla completa y tema

fn alternar_completa(e: &mut Estado) {
    // SAFETY: estilo y sitio de una ventana propia.
    unsafe {
        match e.completa.take() {
            Some((estilo, sitio)) => {
                SetWindowLongPtrW(e.marco, GWL_STYLE, estilo);
                let _ = SetWindowPlacement(e.marco, &sitio);
                let _ = SetWindowPos(
                    e.marco,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
                );
            }
            None => {
                let mut sitio = WINDOWPLACEMENT {
                    length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                    ..Default::default()
                };
                let _ = GetWindowPlacement(e.marco, &mut sitio);
                let estilo = GetWindowLongPtrW(e.marco, GWL_STYLE);
                let mut info = MONITORINFO {
                    cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                let _ = GetMonitorInfoW(MonitorFromWindow(e.marco, MONITOR_DEFAULTTONEAREST), &mut info);
                SetWindowLongPtrW(e.marco, GWL_STYLE, (WS_POPUP | WS_VISIBLE | WS_CLIPCHILDREN).0 as isize);
                let r = info.rcMonitor;
                let _ = SetWindowPos(
                    e.marco,
                    Some(HWND_TOP),
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_FRAMECHANGED,
                );
                e.completa = Some((estilo, sitio));
            }
        }
    }
}

fn cambiar_tema(e: &mut Estado, tema: Tema) {
    let md = markdown(e);
    let sel = seleccion(e.edit);
    e.estilos.tema = tema;
    VISTA.with(|v| {
        if let Some(v) = v.borrow_mut().as_mut() {
            v.tema = tema;
            // SAFETY: pincel propio, cambiado por otro del color nuevo.
            unsafe {
                let _ = DeleteObject(HGDIOBJ(v.pincel_titulo.0));
                v.pincel_titulo = CreateSolidBrush(color(tema.pastilla));
            }
        }
    });
    // Volver a meter la nota ensenaria el Markdown un momento: congelado.
    congelar::congelado(e, congelar::Pintado::Entero, |e| {
        preparar_edit(e.edit, e.marco, &tema, &e.estilos);
        cargar(e, &md);
        elegir(e.edit, sel.0, sel.1);
        pintar(e, None);
    });
    // SAFETY: ventanas propias.
    unsafe {
        let _ = InvalidateRect(Some(e.marco), None, true);
    }
}

/// Colores de fondo y barras de desplazamiento del control segun el tema.
fn preparar_edit(edit: HWND, marco: HWND, tema: &Tema, s: &Estilos) {
    enviar(edit, EM_SETBKGNDCOLOR, 0, bgr(tema.papel) as isize);
    let oscuro: i32 = tema.oscuro as i32;
    // SAFETY: atributos de ventanas propias; valores locales.
    unsafe {
        let _ = windows::Win32::Graphics::Dwm::DwmSetWindowAttribute(
            marco,
            windows::Win32::Graphics::Dwm::DWMWA_USE_IMMERSIVE_DARK_MODE,
            &oscuro as *const _ as *const _,
            4,
        );
        // Las barras de desplazamiento oscuras de Windows 10 y 11.
        let _ = windows::Win32::UI::Controls::SetWindowTheme(
            edit,
            if tema.oscuro { w!("DarkMode_Explorer") } else { w!("Explorer") },
            PCWSTR::null(),
        );
    }
    let mut f = letra_base(s);
    f.Base.dwMask = CFM_SIZE | CFM_COLOR | CFM_FACE;
    enviar(edit, EM_SETCHARFORMAT, SCF_ALL as usize, &f as *const _ as isize);
}

// ---------------------------------------------------------------------------
// Teclado y raton

/// Intro en una lista: el renglon nuevo sigue la lista, y en uno vacio se
/// sale de ella (`Vivo.partir`). `true` si se atendio aqui.
fn intro(e: &mut Estado) -> bool {
    // La marca del renglon nuevo (`- `) entra ya escondida.
    congelar::congelado(e, congelar::Pintado::Tocado, |e| intro_suelto(e))
}

fn intro_suelto(e: &Estado) -> bool {
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

/// Intro en una celda: a la de debajo (con una fila nueva si era la
/// ultima). Un salto dentro de una celda no cabe en Markdown.
fn intro_en_tabla(e: &mut Estado) -> bool {
    let Some((t, f, c)) = celda_del_cursor(e) else {
        return false;
    };
    if f + 1 < t.celdas.len() {
        ir_a_celda(e, t.desde, f + 1, c);
    } else {
        operar_tabla(e, OpTabla::FilaDebajo);
    }
    true
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
fn interceptar(e: &mut Estado, m: &MSG, guardar: &mut dyn FnMut(&str) -> bool) -> bool {
    let sin_mods = !pulsada(VK_CONTROL.0) && !pulsada(VK_SHIFT.0) && !pulsada(VK_MENU.0);
    if m.message == WM_KEYDOWN && e.abierto.is_some() {
        let k = m.wParam.0 as u16;
        let mover = |f: fn(&mut Menu)| {
            menu::VISTA.with(|v| {
                if let Some(v) = v.borrow_mut().as_mut() {
                    f(&mut v.menu);
                }
            })
        };
        match k {
            k if k == VK_DOWN.0 => mover(Menu::bajar),
            k if k == VK_UP.0 => mover(Menu::subir),
            k if k == VK_RETURN.0 || k == VK_TAB.0 => {
                elegir_del_menu(e, guardar);
                return true;
            }
            k if k == VK_ESCAPE.0 => {
                if let Some(Abierto::Barra(b)) = e.abierto {
                    e.descartada = Some(b);
                }
                cerrar_menu(e);
                return true;
            }
            _ => return false,
        }
        // SAFETY: ventana propia.
        unsafe {
            let _ = InvalidateRect(Some(e.menu), None, false);
        }
        return true;
    }
    let solo_ctrl = pulsada(VK_CONTROL.0) && !pulsada(VK_MENU.0) && !pulsada(VK_SHIFT.0);
    match m.message {
        // Ctrl + y Ctrl − cambian la letra; Ctrl 0 deja la pagina sin zoom
        // (el zoom es del pellizco y de Ctrl+rueda, ver `wysiwyg::acercar`).
        WM_KEYDOWN if solo_ctrl && matches!(m.wParam.0, 0xBB | 0x6B) => {
            wysiwyg::comando(e, wysiwyg::C_LETRA_MAS);
            true
        }
        WM_KEYDOWN if solo_ctrl && matches!(m.wParam.0, 0xBD | 0x6D) => {
            wysiwyg::comando(e, wysiwyg::C_LETRA_MENOS);
            true
        }
        WM_KEYDOWN if solo_ctrl && matches!(m.wParam.0, 0x30 | 0x60) => {
            wysiwyg::poner_zoom(e, 1000);
            true
        }
        // Borrar, cortar y escribir sin romper las marcas escondidas (`wysiwyg`).
        WM_KEYDOWN | WM_CHAR if wysiwyg::tecla(e, m) => true,
        // Deshacer y rehacer congelados: lo que vuelve, vuelve escondido.
        WM_KEYDOWN if congelar::es_deshacer(m, pulsada(VK_CONTROL.0), pulsada(VK_MENU.0)) => {
            let rehacer = m.wParam.0 == b'Y' as usize || pulsada(VK_SHIFT.0);
            congelar::deshacer(e, rehacer);
            true
        }
        WM_KEYDOWN if m.wParam.0 == VK_RETURN.0 as usize && sin_mods => intro_en_tabla(e) || intro(e),
        // Esc a mitad de mover un bloque, una fila o una columna (`asas`).
        WM_KEYDOWN if m.wParam.0 == VK_ESCAPE.0 as usize && asas::cancelar(e) => true,
        WM_KEYDOWN if m.wParam.0 == VK_ESCAPE.0 as usize && e.completa.is_some() => {
            alternar_completa(e);
            true
        }
        // Un clic en la casilla la marca, como en el movil. Con Ctrl, un
        // clic en un enlace lo abre (sin Ctrl se esta escribiendo en el).
        // Las fotos: su asa, abrirlas, elegirlas (`fotos::raton`).
        // Apunta las celdas que se arrastran (no se queda con nada).
        // La rueda de lado y la barra de una tabla ancha (`tabla_ancha`).
        // Las asas de mover bloques, filas y columnas (`asas`), antes que nada.
        WM_LBUTTONDOWN | WM_MOUSEMOVE | WM_LBUTTONUP | WM_MOUSELEAVE if asas::raton(e, m) => true,
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL | WM_LBUTTONDOWN | WM_MOUSEMOVE | WM_LBUTTONUP if tablas::desplazar::raton(e, m) => true,
        // La rueda por pixeles y suave, y la barra fina de la nota (`wysiwyg`).
        WM_MOUSEWHEEL if wysiwyg::rueda(e, m) => true,
        WM_LBUTTONDOWN | WM_MOUSEMOVE | WM_LBUTTONUP if wysiwyg::raton_barra(e, m) => true,
        WM_LBUTTONDOWN | WM_MOUSEMOVE | WM_LBUTTONUP if tablas::raton(e, m) => true,
        // Los documentos, mensajes y audios: abrir, tocar, la barra, las
        // marcas de tiempo (`incrustados`); antes que las fotos, que tienen asa.
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK if incrustados::raton(e, m) => true,
        WM_LBUTTONDOWN | WM_MOUSEMOVE | WM_LBUTTONUP | WM_LBUTTONDBLCLK if fotos::raton(e, m) => true,
        WM_DROPFILES => {
            fotos::soltar_hdrop(e, m.wParam.0, true);
            true
        }
        WM_LBUTTONDOWN => {
            cerrar_menu(e);
            let pos = letra_en(e.edit, m.lParam);
            let texto = leer(e.edit);
            // Un enlace a una hoja de un proyecto la abre en su editor.
            if pulsada(VK_CONTROL.0)
                && let Some(url) = md_vivo::enlace_en(&texto, pos).filter(|u| fotos::es_enlace_a_hoja(u))
            {
                fotos::abrir(e, &url);
                return true;
            }
            if pulsada(VK_CONTROL.0)
                && let Some(url) = md_vivo::enlace_en(&texto, pos).filter(|u| es_direccion(u))
            {
                if let Err(err) = pixpin_shell::abrir(std::path::Path::new(&url)) {
                    tracing::warn!(?err, "no se pudo abrir el enlace");
                }
                return true;
            }
            // La casilla se pinta en la sangria (su texto esta escondido): el
            // clic se mira por donde se pinto.
            let (x, y) = ((m.lParam.0 & 0xffff) as i16 as i32, ((m.lParam.0 >> 16) & 0xffff) as i16 as i32);
            if let Some(n) = imagenes::casilla_en(e.edit, x, y) {
                alternar_casilla(e, n);
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
/// sale al elegir texto, con su tecla al lado; y dentro de una tabla, sus
/// filas y columnas.
fn menu_contextual(e: &mut Estado) {
    cerrar_menu(e);
    // Sobre una foto, lo de la foto primero (verla, su tamano, quitarla).
    let mut de_foto = fotos::entradas_del_menu(e);
    // Sobre un documento, un mensaje o un audio: abrirlo y quitarlo.
    de_foto.extend(incrustados::entradas_del_menu(e));
    let con_hojas = e.integracion.hojas.is_some();
    let r = &e.rotulos;
    let (m, i) = (&r.mayus, &r.intro);
    let mut entradas: Vec<Option<(u16, String)>> = de_foto;
    // Comentar lo elegido, arriba como en Google Docs.
    entradas.push(Some((C_COMENTAR, format!("{}\tCtrl+Alt+M", r.comentarios.comentar))));
    entradas.push(None);
    let en_tabla = celda_del_cursor(e).is_some();
    // Detras de quitar tabla van combinar, separar y los colores.
    let pos_tabla = entradas.len() as u32 + 7;
    if en_tabla {
        entradas.extend([
            Some((C_FILA_ENCIMA, r.fila_encima.clone())),
            Some((C_FILA_DEBAJO, r.fila_debajo.clone())),
            Some((C_COL_IZQ, r.columna_izquierda.clone())),
            Some((C_COL_DER, r.columna_derecha.clone())),
            Some((C_QUITAR_FILA, r.quitar_fila.clone())),
            Some((C_QUITAR_COL, r.quitar_columna.clone())),
            Some((C_QUITAR_TABLA, r.quitar_tabla.clone())),
            None,
        ]);
    }
    entradas.extend([
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
        Some((C_TABLA, format!("{}\tCtrl+{m}+T", r.tabla))),
        Some((C_IMAGEN, r.imagen.clone())),
        Some((C_PAGINA_VIVA, r.pagina_viva.clone())),
        Some((C_ENLACE_HOJA, r.enlace_hoja.clone())),
        Some((C_FECHA, format!("{}\tCtrl+{m}+D", r.fecha))),
        None,
        Some((C_CORTAR, format!("{}\tCtrl+X", r.cortar))),
        Some((C_COPIAR, format!("{}\tCtrl+C", r.copiar))),
        Some((C_PEGAR, format!("{}\tCtrl+V", r.pegar))),
        None,
        Some((C_GUARDAR, format!("{}\tCtrl+S", r.guardar))),
        Some((C_COPIA_MD, format!("{}\tCtrl+{m}+S", r.copia_md))),
    ]);
    // Sin aplicacion detras no hay hojas que meter.
    entradas.retain(|x| con_hojas || !matches!(x, Some((C_PAGINA_VIVA | C_ENLACE_HOJA, _))));
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
        let muestras = if en_tabla {
            tablas::al_menu_contextual(e, menu, pos_tabla)
        } else {
            Vec::new()
        };
        let mut p = POINT::default();
        let _ = GetCursorPos(&mut p);
        let r = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, p.x, p.y, None, e.marco, None);
        let _ = DestroyMenu(menu);
        tablas::soltar(muestras);
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
        a(FVIRTKEY | FCONTROL | FALT, b'M' as u16, C_COMENTAR),
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
        a(cs, b'T' as u16, C_TABLA),
        a(cs, b'D' as u16, C_FECHA),
        a(c, b'V' as u16, C_PEGAR),
        a(FVIRTKEY | FSHIFT, 0x2D, C_PEGAR),
        a(c, b'S' as u16, C_GUARDAR),
        a(cs, b'S' as u16, C_COPIA_MD),
        a(c, b'W' as u16, C_CERRAR),
        a(FVIRTKEY, VK_F11.0, C_COMPLETA),
    ]
}

// ---------------------------------------------------------------------------
// El marco

/// Vuelve a medir y colocar el marco (tras cambiar de tamano o de titulo).
fn recolocar(marco: HWND) {
    let mut r = RECT::default();
    // SAFETY: ventana propia; el DC se pide y se devuelve aqui.
    unsafe {
        let _ = GetClientRect(marco, &mut r);
        let dc = GetDC(Some(marco));
        VISTA.with(|v| {
            if let Some(v) = v.borrow_mut().as_mut() {
                let p = &v.pintor;
                let medir = |s: &str| p.medir(dc, p.letra, s);
                let m = Medidas {
                    ancho: r.right,
                    alto: r.bottom,
                    escala: p.escala,
                    ancho_titulo: medir(&v.titulo),
                    ancho_compartir: medir(&v.compartir),
                    en_tabla: v.en_tabla,
                    anchos_tabla: [
                        medir(&v.de_tabla[0]),
                        medir(&v.de_tabla[1]),
                        medir(&v.de_tabla[2]),
                        medir(&v.de_tabla[3]),
                        medir(&v.de_tabla[4]),
                        medir(&v.de_tabla[5]),
                    ],
                };
                v.disp = disposicion::disponer(&m);
                // El panel de comentarios, si esta abierto, a la derecha.
                crate::panel_comentarios::recortar(&mut v.disp, p.escala);
            }
        });
        ReleaseDC(Some(marco), dc);
        let _ = InvalidateRect(Some(marco), None, false);
    }
}

/// Pinta la cabecera y la barra en `hdc`.
fn pintar_marco(hdc: HDC, v: &VistaMarco) {
    let t = &v.tema;
    let d = &v.disp;
    let p = &v.pintor;
    let e = |x: i32| (x as f32 * p.escala).round() as i32;
    let zona = RECT {
        left: 0,
        top: 0,
        right: d.cabecera.an,
        bottom: d.barra.abajo(),
    };
    let caja = |b: Boton| d.caja(b).unwrap_or_default();
    let titulo = caja(Boton::Titulo);
    let flecha = caja(Boton::TituloMenu);
    p.formas(hdc, zona, |f| {
        f.rect(d.cabecera, t.cabecera);
        f.rect(d.barra, t.papel);
        f.raya(0, d.barra.abajo() - 1, d.barra.an, d.barra.abajo() - 1, t.raya);
        // La pastilla del titulo con su flecha.
        let pastilla = Caja {
            an: flecha.derecha() - titulo.x,
            ..titulo
        };
        f.redondo(pastilla, 6.0, t.pastilla);
        if matches!(v.hover, Some(Boton::Titulo | Boton::TituloMenu)) {
            f.borde(pastilla, 6.0, 1.0, t.apagado);
        }
        let fl = Caja {
            x: flecha.x - e(4),
            ..flecha
        };
        f.icono(Icono::Flecha, fl, 12.0, t.tenue);
        // Compartir va al reves: claro sobre la cabecera oscura.
        let c = caja(Boton::Compartir);
        f.redondo(c, 7.0, t.boton_fondo);
        let ic = Caja {
            x: c.x + e(8),
            y: c.y,
            an: e(16),
            al: c.al,
        };
        f.icono(Icono::Compartir, ic, 14.0, t.boton_texto);
        for (b, icono) in [
            (Boton::Minimizar, Icono::Minimizar),
            (Boton::PantallaCompleta, Icono::PantallaCompleta),
            (Boton::Cerrar, Icono::Cerrar),
            (Boton::Tabla, Icono::Tabla),
            (Boton::Imagen, Icono::Imagen),
            (Boton::Casillas, Icono::Casillas),
            (Boton::Comentar, Icono::Comentar),
        ] {
            let c = caja(b);
            if v.hover == Some(b) {
                f.redondo(c, 6.0, t.pastilla);
            }
            let tinta = if matches!(b, Boton::Minimizar | Boton::PantallaCompleta | Boton::Cerrar) { t.tenue } else { t.texto };
            f.icono(icono, c, 15.0, tinta);
        }
        let mas = caja(Boton::Mas);
        if v.hover == Some(Boton::Mas) {
            f.redondo(mas, 6.0, t.pastilla);
        }
        let izq = Caja {
            x: mas.x + e(2),
            an: e(22),
            ..mas
        };
        f.icono(Icono::Mas, izq, 15.0, t.texto);
        let der = Caja {
            x: mas.x + e(22),
            an: e(16),
            ..mas
        };
        f.icono(Icono::Flecha, der, 11.0, t.tenue);
        for s in &d.separadores {
            f.rect(*s, t.raya);
        }
        for b in disposicion::DE_TABLA {
            if let Some(c) = d.caja(b) {
                if v.hover == Some(b) {
                    f.redondo(c, 6.0, t.pastilla);
                } else {
                    f.borde(c, 6.0, 1.0, t.raya);
                }
            }
        }
    });
    let texto_titulo = Caja {
        x: titulo.x + e(10),
        an: titulo.an - e(10),
        ..titulo
    };
    p.texto(hdc, p.letra, &v.titulo, texto_titulo, t.texto, false);
    let c = caja(Boton::Compartir);
    let et = Caja {
        x: c.x + e(28),
        an: c.an - e(34),
        ..c
    };
    p.texto(hdc, p.letra_negrita, &v.compartir, et, t.boton_texto, false);
    for (i, b) in disposicion::DE_TABLA.iter().enumerate() {
        if let Some(c) = d.caja(*b) {
            p.texto(hdc, p.letra_chica, &v.de_tabla[i], c, t.texto, true);
        }
    }
    // El globo de los comentarios (las tarjetas van en su ventana, ver
    // `comentarios::carril`).
    let g = caja(Boton::Comentarios);
    crate::panel_comentarios::pintar_boton(hdc, p, g, v.hover == Some(Boton::Comentarios), t);
}

/// **Los margenes de la columna** (izquierda, derecha) en `visible`
/// pixeles: centrada; con las tarjetas de los comentarios en el margen
/// derecho (`reserva`), corrida a la izquierda lo justo para no ir debajo
/// de ellas, y si ni asi cabe, mas estrecha (con `minimo` a la izquierda).
fn margenes(visible: i32, columna: i32, reserva: i32, minimo: i32) -> (i32, i32) {
    let sobra = (visible - columna).max(0) / 2;
    if reserva <= 0 || sobra >= reserva {
        return (sobra, sobra);
    }
    let izq = (visible - columna - reserva).max(minimo.min(sobra));
    let der = (visible - izq - columna).max(reserva);
    (izq, der)
}

/// El margen de la izquierda que deja la columna de texto, en twips (el de
/// la derecha queda en `tabla_ancha::sobra_der_twips`).
fn margen_de(e: &Estado) -> i32 {
    let mut r = RECT::default();
    enviar(e.edit, 0x00B2, 0, &mut r as *mut _ as isize);
    let columna = tabla_rtf::COLUMNA_PX * e.ppp / 96;
    // Con tablas anchas lo escrito empieza en un hueco que no se ve (ver
    // `crate::tabla_ancha`): se centra en lo que se ve y se sangra el hueco.
    // El hueco que no se ve es el de siempre menos este margen: lo escrito
    // queda a la sangria entera del hueco y una tabla ancha empieza donde el
    // texto (`tabla_ancha::poner_sobra`). Si el margen cambio, el control se
    // recoloca con el hueco nuevo.
    let visible = r.right - r.left - crate::tabla_ancha::hueco();
    // Las tarjetas tapan desde su borde hasta el del papel (la barra fina
    // incluida); lo escrito acaba antes, en el margen del control.
    let ui = PPP.with(|p| p.get());
    let reserva = match crate::panel_comentarios::reserva() {
        0 => 0,
        r => r + (imagenes::BARRA_COGER_PX + 16 - MARGEN_PX) * ui / 96,
    };
    let (sobra, derecha) = margenes(visible, columna, reserva, MARGEN_PX * e.ppp / 96);
    crate::tabla_ancha::poner_sobra_der(derecha * 1440 / e.ppp);
    let sobra_tw = sobra * 1440 / e.ppp;
    if crate::tabla_ancha::poner_sobra(sobra_tw) {
        colocar_edit(e.marco);
    }
    sobra_tw + crate::tabla_ancha::hueco_twips()
}

/// El control ocupa el papel, con un margen a cada lado.
fn colocar_edit(marco: HWND) {
    let edit = EDIT.with(|e| e.get());
    if edit == 0 {
        return;
    }
    let edit = HWND(edit as *mut _);
    let Some(cuerpo) = VISTA.with(|v| v.borrow().as_ref().map(|v| v.disp.cuerpo)) else {
        return;
    };
    let ppp = PPP.with(|p| p.get());
    let e = |v: i32| v * ppp / 96;
    // Con tablas anchas, el control se sale del papel por la izquierda lo
    // que mide el hueco (ver `crate::tabla_ancha`).
    let hueco = crate::tabla_ancha::hueco();
    // SAFETY: ventanas propias.
    unsafe {
        let _ = MoveWindow(edit, cuerpo.x - hueco, cuerpo.y, cuerpo.an + hueco, cuerpo.al.max(1), true);
    }
    let _ = marco;
    let dentro = RECT {
        left: e(MARGEN_PX),
        top: e(18),
        right: (cuerpo.an + hueco - e(MARGEN_PX)).max(e(MARGEN_PX) + 1),
        bottom: (cuerpo.al - e(8)).max(e(19)),
    };
    enviar(edit, EM_SETRECT, 0, &dentro as *const _ as isize);
}

fn hover(h: HWND, b: Option<Boton>) {
    let cambio = VISTA.with(|v| {
        let mut v = v.borrow_mut();
        let v = v.as_mut()?;
        (v.hover != b).then(|| v.hover = b)
    });
    if cambio.is_some() {
        // SAFETY: ventana propia.
        unsafe {
            let _ = InvalidateRect(Some(h), None, false);
        }
    }
}

fn punto_de(l: LPARAM) -> (i32, i32) {
    ((l.0 & 0xffff) as i16 as i32, ((l.0 >> 16) & 0xffff) as i16 as i32)
}

unsafe extern "system" fn procedimiento(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        // Sin barra de titulo de Windows: la cabecera propia es la barra
        // (se arrastra, doble clic maximiza); los bordes siguen siendo de
        // Windows para cambiar el tamano.
        WM_NCCALCSIZE if w.0 != 0 => {
            // SAFETY: Windows pasa aqui un NCCALCSIZE_PARAMS valido.
            unsafe {
                let p = l.0 as *mut NCCALCSIZE_PARAMS;
                let arriba = (*p).rgrc[0].top;
                let r = DefWindowProcW(h, m, w, l);
                (*p).rgrc[0].top = arriba;
                if IsZoomed(h).as_bool() {
                    (*p).rgrc[0].top += GetSystemMetrics(SM_CYFRAME) + GetSystemMetrics(SM_CXPADDEDBORDER);
                }
                r
            }
        }
        WM_NCHITTEST => {
            // SAFETY: lo de Windows primero (los bordes).
            let r = unsafe { DefWindowProcW(h, m, w, l) };
            if r.0 != HTCLIENT as isize {
                return r;
            }
            let mut p = POINT {
                x: (l.0 & 0xffff) as i16 as i32,
                y: ((l.0 >> 16) & 0xffff) as i16 as i32,
            };
            // SAFETY: conversion de coordenadas de una ventana propia.
            let maxima = unsafe {
                let _ = ScreenToClient(h, &mut p);
                IsZoomed(h).as_bool()
            };
            let borde = 5 * PPP.with(|x| x.get()) / 96;
            if p.y < borde && !maxima {
                return LRESULT(HTTOP as isize);
            }
            let zona = VISTA.with(|v| {
                v.borrow().as_ref().map(|v| (v.disp.zona(p.x, p.y), v.disp.cabecera.contiene(p.x, p.y)))
            });
            match zona {
                Some((Zona::Arrastre, true)) => LRESULT(HTCAPTION as isize),
                _ => LRESULT(HTCLIENT as isize),
            }
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            // SAFETY: pintado de una ventana propia, entre Begin y End.
            unsafe {
                let dc = BeginPaint(h, &mut ps);
                VISTA.with(|v| {
                    if let Some(v) = v.borrow().as_ref() {
                        pintar_marco(dc, v);
                    }
                });
                let _ = EndPaint(h, &ps);
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let (x, y) = punto_de(l);
            let b = VISTA.with(|v| {
                v.borrow().as_ref().and_then(|v| match v.disp.zona(x, y) {
                    Zona::Boton(b) => Some(b),
                    _ => None,
                })
            });
            hover(h, b);
            let mut t = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: h,
                dwHoverTime: 0,
            };
            // SAFETY: estructura local; ventana propia.
            unsafe {
                let _ = TrackMouseEvent(&mut t);
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            hover(h, None);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let (x, y) = punto_de(l);
            // El panel de comentarios lo atiende el bucle (`comentarios::clic`).
            if crate::panel_comentarios::clic(x, y) {
                apuntar(Orden::ClicFuera);
                return LRESULT(0);
            }
            let z = VISTA.with(|v| v.borrow().as_ref().map(|v| v.disp.zona(x, y)));
            match z {
                Some(Zona::Boton(b)) => apuntar(Orden::Clic(b)),
                _ => apuntar(Orden::ClicFuera),
            }
            LRESULT(0)
        }
        WM_SIZE => {
            recolocar(h);
            colocar_edit(h);
            apuntar(Orden::Tamano);
            LRESULT(0)
        }
        // La rueda sobre el panel de comentarios mueve la nota (y el panel
        // la sigue).
        WM_MOUSEWHEEL => {
            let edit = EDIT.with(|e| e.get());
            if edit != 0 {
                // SAFETY: el control es hijo de esta ventana y vive con ella.
                unsafe {
                    // Por la cola: asi pasa por el bucle, que la desliza suave
                    // (`wysiwyg::rueda`), como la que cae en el texto.
                    let _ = PostMessageW(Some(HWND(edit as *mut _)), m, w, l);
                }
            }
            LRESULT(0)
        }
        WM_MOVE => {
            apuntar(Orden::ClicFuera);
            LRESULT(0)
        }
        WM_GETMINMAXINFO => {
            let ppp = PPP.with(|p| p.get());
            // SAFETY: Windows pasa aqui un MINMAXINFO valido.
            unsafe {
                let mm = l.0 as *mut MINMAXINFO;
                (*mm).ptMinTrackSize = POINT {
                    x: 440 * ppp / 96,
                    y: 320 * ppp / 96,
                };
            }
            LRESULT(0)
        }
        WM_ACTIVATE => {
            if (w.0 & 0xffff) as u32 == WA_INACTIVE {
                apuntar(Orden::ClicFuera);
            }
            // SAFETY: lo demas de la activacion, a Windows.
            unsafe { DefWindowProcW(h, m, w, l) }
        }
        WM_SETTINGCHANGE => {
            // El cambio de tema claro/oscuro llega como «ImmersiveColorSet».
            if l.0 != 0 {
                // SAFETY: Windows manda una cadena terminada en cero.
                let s = unsafe { PCWSTR(l.0 as *const u16).to_string().unwrap_or_default() };
                if s == "ImmersiveColorSet" {
                    apuntar(Orden::Tema);
                }
            }
            LRESULT(0)
        }
        WM_CTLCOLOREDIT => {
            // El titulo mientras se edita: sobre su pastilla.
            let r = VISTA.with(|v| {
                v.borrow().as_ref().map(|v| {
                    let dc = HDC(w.0 as *mut _);
                    // SAFETY: el DC que presta Windows para este pintado.
                    unsafe {
                        SetTextColor(dc, color(v.tema.texto));
                        SetBkColor(dc, color(v.tema.pastilla));
                    }
                    v.pincel_titulo.0 as isize
                })
            });
            LRESULT(r.unwrap_or(0))
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
        // Ficheros soltados en el texto (`EN_DROPFILES`) o en el marco, y el
        // toque de la aplicacion para que mire las paginas vivas.
        WM_NOTIFY | WM_DROPFILES | fotos::WM_VIVAS => fotos::mensaje_del_marco(m, w, l),
        WM_TIMER if w.0 == fotos::T_VIVAS => {
            apuntar(Orden::Vivas);
            LRESULT(0)
        }
        WM_COMMAND => {
            let aviso = (w.0 >> 16) & 0xffff;
            if l.0 != 0 {
                if l.0 == EDIT.with(|e| e.get()) {
                    if aviso == EN_CHANGE {
                        apuntar(Orden::Cambio);
                    }
                } else if aviso == EN_KILLFOCUS {
                    apuntar(Orden::TituloHecho);
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
            } else if w.0 == incrustados::T_AUDIO {
                apuntar(Orden::Audio);
            } else if w.0 == wysiwyg::T_DESLIZAR {
                apuntar(Orden::Deslizar);
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

/// El procedimiento del `RichEdit`, envuelto: tras pintar lo suyo, las
/// fotos encima.
unsafe extern "system" fn procedimiento_edit(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    let original = ORIGINAL.with(|o| o.get());
    // SAFETY: `original` es el procedimiento que tenia el control, guardado
    // al envolverlo; el DC de WM_PRINTCLIENT es el de quien lo pide.
    unsafe {
        let anterior: WNDPROC = std::mem::transmute::<isize, WNDPROC>(original);
        // Sobre el asa de una foto, la flecha de cambiar el tamano.
        if m == WM_SETCURSOR
            && (asas::cursor(h) || fotos::cursor_del_asa(h) || tablas::desplazar::cursor_de_la_barra(h) || wysiwyg::cursor_de_la_barra(h))
        {
            return LRESULT(1);
        }
        // Lo que va encima (fotos, casillas, la barra) no lo pinta el control:
        // asi no lo borra para que se vuelva a pintar (parpadeo).
        if m == WM_PAINT {
            wysiwyg::antes_de_pintar(h);
        }
        let r = CallWindowProcW(anterior, h, m, w, l);
        // Lo que el control acaba de pintar por su cuenta (para el espia de
        // las pruebas; sin espia no hace nada).
        if congelar::cambia_lo_que_se_ve(m) {
            congelar::fotograma(h);
        }
        match m {
            WM_PAINT => {
                imagenes::repintar(h);
                tablas::desplazar::repintar(h);
                asas::repintar(h);
            }
            WM_PRINTCLIENT => {
                imagenes::pintar_encima(h, HDC(w.0 as *mut _));
                tablas::desplazar::pintar_encima(h, HDC(w.0 as *mut _));
                asas::pintar_encima(HDC(w.0 as *mut _));
            }
            _ => {}
        }
        r
    }
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

/// Como montar la ventana: las pruebas y las muestras la quieren oculta, de
/// un tamano dado y con un tema fijo.
#[derive(Debug, Clone, Copy, Default)]
struct Opciones {
    oculto: bool,
    tamano: Option<(i32, i32)>,
    claro: Option<bool>,
}

/// Crea las ventanas y mete la nota. No ensena nada.
fn montar(p: Pedido, op: Opciones) -> windows::core::Result<Estado> {
    // SAFETY: todo lo que se crea aqui es de este hilo y lo suelta
    // `desmontar`: la clase (registrarla dos veces falla sin dano), las
    // ventanas y los pinceles.
    unsafe {
        windows::Win32::System::LibraryLoader::LoadLibraryW(w!("msftedit.dll"))?;
        let instancia = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?;
        let clase = WNDCLASSW {
            lpfnWndProc: Some(procedimiento),
            hInstance: instancia.into(),
            lpszClassName: w!("PixPinNotaMd"),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&clase);
        let ppp = ppp_de_pantalla();
        PPP.with(|c| c.set(ppp));
        imagenes::PPP.with(|c| c.set(ppp));
        let escala = ppp as f32 / 96.0;
        let tema = match op.claro {
            Some(true) => Tema::claro(),
            Some(false) => Tema::oscuro(),
            None => Tema::del_sistema(),
        };
        let letras = letras::registrar();
        let pintor = Rc::new(
            Pintor::nuevo(escala, &letras.cuerpo).ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))?,
        );
        let e = |v: i32| v * ppp / 96;
        let (an, al) = op.tamano.unwrap_or((e(980), e(900)));
        VISTA.with(|v| {
            *v.borrow_mut() = Some(VistaMarco {
                tema,
                pintor: pintor.clone(),
                titulo: String::new(),
                compartir: p.rotulos.compartir.clone(),
                de_tabla: [
                    p.rotulos.boton_fila_mas.clone(),
                    p.rotulos.boton_fila_menos.clone(),
                    p.rotulos.boton_columna_mas.clone(),
                    p.rotulos.boton_columna_menos.clone(),
                    tablas::rotulo_combinar(&p.rotulos),
                    tablas::rotulo_color(&p.rotulos),
                ],
                en_tabla: false,
                hover: None,
                disp: Marco::default(),
                pincel_titulo: CreateSolidBrush(color(tema.pastilla)),
            })
        });
        let marco = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("PixPinNotaMd"),
            w!(""),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            an,
            al,
            None,
            None,
            Some(instancia.into()),
            None,
        )?;
        let edit = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            MSFTEDIT_CLASS,
            w!(""),
            // Sin barra de abajo: cada tabla ancha lleva la suya (`tabla_ancha`).
            // Sin pintar encima de sus hermanas: el cajon de los comentarios
            // va sobre ella (`comentarios::carril`).
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL | ES_NOHIDESEL) as u32),
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
        let original = SetWindowLongPtrW(edit, GWLP_WNDPROC, procedimiento_edit as *const () as isize);
        ORIGINAL.with(|o| o.set(original));
        let menu = menu::crear(marco)?;

        // Sin el tope de 32 000 letras que el RichEdit trae de fabrica.
        enviar(edit, EM_EXLIMITTEXT, 0, 0x7FFF_FFFE);
        // Ajustar a la ventana, y que el control no se corra de lado para
        // ensenar el cursor: lo hace la tabla ancha suya (`tabla_ancha`).
        enviar(edit, EM_SETOPTIONS, ECOOP_AND as usize, !(ECO_AUTOHSCROLL as isize));
        enviar(edit, EM_SETTARGETDEVICE, 0, 0);
        // El corrector ortografico de Windows (8 en adelante) y el IME.
        let opciones = enviar(edit, EM_GETLANGOPTIONS, 0, 0);
        enviar(edit, EM_SETLANGOPTIONS, 0, opciones | IMF_SPELLCHECKING as isize);
        let ctf = SES_USECTF | SES_CTFALLOWPROOFING | SES_CTFALLOWEMBED;
        enviar(edit, EM_SETEDITSTYLE, ctf as usize, ctf as isize);

        let doc: Option<ITextDocument> = {
            let mut ole: *mut core::ffi::c_void = std::ptr::null_mut();
            enviar(edit, EM_GETOLEINTERFACE, 0, &mut ole as *mut _ as isize);
            if ole.is_null() {
                None
            } else {
                windows::core::IUnknown::from_raw(ole).cast::<ITextDocument>().ok()
            }
        };
        // La letra y el tamano elegidos para leer (ver `vista`): no van en el
        // texto, los guarda la aplicacion aparte.
        let vivo = wysiwyg::Vivo::nuevo(&p.integracion);
        let estilos = Estilos {
            tema,
            letras: letras::de_vista(&vivo.vista),
            margen: 0,
            tamano: wysiwyg::tamano_de(&vivo.vista),
        };
        preparar_edit(edit, marco, &tema, &estilos);
        let mut estado = Estado {
            marco,
            edit,
            menu,
            titulo_edit: None,
            doc,
            guardado: String::new(),
            linea_activa: usize::MAX,
            pendiente: false,
            rotulos: p.rotulos,
            nombre_de_fichero: p.nombre_de_fichero,
            estilos,
            pintor,
            ppp,
            abierto: None,
            descartada: None,
            menu_caja: Caja::default(),
            fotos: Fotos::default(),
            resolver: p.resolver,
            adjuntar: p.adjuntar,
            compartir: p.compartir,
            completa: None,
            oculto: op.oculto,
            en_tabla: false,
            integracion: p.integracion,
            sin_hoja: Default::default(),
            foto_del_menu: None,
            // Antes de colocar: con comentarios abiertos, el panel nace abierto.
            comentarios: comentarios::Comentarios::nuevo(p.comentarios),
            vivo,
        };
        recolocar(marco);
        colocar_edit(marco);
        estado.estilos.margen = margen_de(&estado);
        cargar(&mut estado, &p.texto);
        // Con EN_DROPFILES: soltar fotos del Explorador en el texto.
        enviar(edit, EM_SETEVENTMASK, 0, (ENM_CHANGE | ENM_DROPFILES) as isize);
        fotos::preparar(&estado);
        // Lo guardado es lo que se leeria ahora: abrir y cerrar no reescribe
        // una tabla que venia escrita de otra forma.
        estado.guardado = markdown(&estado);
        poner_titulo(&estado, &estado.guardado.clone());
        // Una nota nueva empieza arriba; una que ya tiene texto, al final,
        // que es donde se sigue escribiendo (`TextRange(inicial.length)`).
        let fin = leer(edit).encode_utf16().count();
        elegir(edit, fin, fin);
        pintar(&mut estado, None);
        Ok(estado)
    }
}

fn desmontar(mut e: Estado) {
    wysiwyg::desmontar(&mut e);
    asas::olvidar();
    incrustados::olvidar();
    comentarios::desmontar(&e);
    EDIT.with(|c| c.set(0));
    imagenes::PUESTAS.with(|p| p.borrow_mut().clear());
    tablas::desplazar::olvidar();
    menu::VISTA.with(|v| *v.borrow_mut() = None);
    VISTA.with(|v| {
        if let Some(v) = v.borrow_mut().take() {
            // SAFETY: pincel propio.
            unsafe {
                let _ = DeleteObject(HGDIOBJ(v.pincel_titulo.0));
            }
        }
    });
    // SAFETY: ventanas propias; si ya se destruyeron, falla sin dano.
    unsafe {
        let _ = DestroyWindow(e.menu);
        let _ = DestroyWindow(e.marco);
    }
}

/// Mira si el cursor entro o salio de una tabla (salen o se van los
/// botones de la tabla en la barra).
fn actualizar_en_tabla(e: &mut Estado) {
    let dentro = celda_del_cursor(e).is_some();
    if dentro != e.en_tabla {
        e.en_tabla = dentro;
        VISTA.with(|v| {
            if let Some(v) = v.borrow_mut().as_mut() {
                v.en_tabla = dentro;
            }
        });
        recolocar(e.marco);
    }
}

/// Abre la ventana y no vuelve hasta que se cierra. `al_nacer` recibe la
/// ventana en cuanto existe; `guardar` escribe el texto y dice si pudo.
pub fn correr(
    p: Pedido,
    al_nacer: &mut dyn FnMut(isize),
    guardar: &mut dyn FnMut(&str) -> bool,
) -> windows::core::Result<()> {
    let colocacion = p.colocacion.clone();
    let mut estado = montar(p, Opciones::default())?;
    let (marco, edit) = (estado.marco, estado.edit);
    al_nacer(marco.0 as isize);
    // SAFETY: ventanas propias de este hilo; la tabla de teclas se suelta
    // al final.
    unsafe {
        match colocacion {
            Some(c) => {
                pixpin_shell::colocacion::poner(marco.0 as isize, &c);
            }
            None => {
                let _ = ShowWindow(marco, SW_SHOWNORMAL);
            }
        }
        // Que Windows recalcule el marco sin su barra de titulo.
        let _ = SetWindowPos(marco, None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED);
        let _ = SetForegroundWindow(marco);
        let _ = SetFocus(Some(edit));
        SetTimer(Some(marco), T_AUTOGUARDAR, MS_AUTOGUARDAR, None);
        // Las paginas vivas se miran al abrir y luego cada poco.
        fotos::arrancar(&mut estado);

        let tabla = CreateAcceleratorTableW(&teclas())?;
        let mut m = MSG::default();
        while GetMessageW(&mut m, None, 0, 0).0 > 0 {
            // Un clic en la nota no la mueve (ver `quedarse_en`).
            let antes_del_clic = (m.hwnd == edit && matches!(m.message, WM_LBUTTONDOWN | WM_LBUTTONUP)).then(|| {
                let mut p = POINT::default();
                enviar(edit, EM_GETSCROLLPOS, 0, &mut p as *mut _ as isize);
                p.y
            });
            let del_titulo = estado.titulo_edit.is_some_and(|t| t == m.hwnd);
            // Escribiendo un comentario las teclas son suyas (Intro envia).
            let del_comentario = estado.comentarios.compositor == Some(m.hwnd);
            // Y en el cuadro del enlace de la barra flotante (Intro lo pone).
            let del_enlace = wysiwyg::es_del_enlace(&estado, m.hwnd);
            if del_titulo && m.message == WM_KEYDOWN && (m.wParam.0 == VK_RETURN.0 as usize || m.wParam.0 == VK_ESCAPE.0 as usize) {
                acabar_titulo(&mut estado, m.wParam.0 == VK_RETURN.0 as usize);
            } else if del_comentario && comentarios::tecla(&mut estado, &m) {
            } else if del_enlace && wysiwyg::tecla_del_enlace(&mut estado, &m) {
            } else if m.hwnd == edit && interceptar(&mut estado, &m, guardar) {
            } else if del_titulo || del_comentario || del_enlace || TranslateAcceleratorW(marco, tabla, &m) == 0 {
                let _ = TranslateMessage(&m);
                DispatchMessageW(&m);
            }
            if menu::CLIC.with(|c| c.borrow_mut().take()).is_some() {
                elegir_del_menu(&mut estado, guardar);
            }
            if let Some(p) = crate::panel_comentarios::tomar_clic() {
                comentarios::clic(&mut estado, p);
            }
            if let Some(b) = crate::barra_flotante::tomar_clic() {
                wysiwyg::clic_barra(&mut estado, b, guardar);
            }
            let ordenes: Vec<Orden> = COLA.with(|c| std::mem::take(&mut *c.borrow_mut()));
            for o in ordenes {
                atender(&mut estado, o, guardar);
            }
            if m.hwnd == edit && matches!(m.message, WM_KEYDOWN | WM_KEYUP | WM_LBUTTONUP | WM_LBUTTONDOWN) {
                seguir_al_cursor(&mut estado);
                tablas::desplazar::seguir(&estado);
                actualizar_en_tabla(&mut estado);
                if matches!(m.message, WM_KEYUP | WM_LBUTTONUP) {
                    actualizar_barra(&mut estado);
                }
            }
            // El cursor fuera de las marcas, la conversion al vuelo y la barra
            // flotante (`wysiwyg`).
            wysiwyg::despues(&mut estado, &m);
            if m.hwnd == edit && matches!(m.message, WM_KEYDOWN | WM_CHAR | WM_MOUSEWHEEL | WM_VSCROLL | WM_LBUTTONUP) {
                imagenes::repintar(edit);
            }
            // El panel sigue a la nota y el cursor elige su comentario.
            comentarios::seguir(&mut estado, &m);
            if let Some(y) = antes_del_clic {
                quedarse_en(&estado, y);
            }
        }
        let _ = DestroyAcceleratorTable(tabla);
    }
    desmontar(estado);
    Ok(())
}

/// **Un clic no teletransporta** (el usuario, 1-oct: «cuando le doy click
/// a la tabla me teletransporta»): lo pulsado ya estaba a la vista, asi que
/// si tras atender el clic la nota se movio (el control enseñando el
/// cursor al entrar en una tabla, colocar sus filas, elegir una fila o una
/// columna entera desde su barrita), vuelve a donde estaba. Arrastrando
/// para elegir, el control si corre la nota: eso no se toca.
fn quedarse_en(e: &Estado, y: i32) {
    let mut p = POINT::default();
    enviar(e.edit, EM_GETSCROLLPOS, 0, &mut p as *mut _ as isize);
    // SAFETY: consulta de la captura del raton de este hilo.
    let arrastrando = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetCapture() } == e.edit;
    if p.y == y || arrastrando {
        return;
    }
    let (a, b) = seleccion(e.edit);
    if a != b && !asas::eligio_entera() {
        return;
    }
    let q = POINT { x: p.x, y };
    if e.oculto {
        enviar(e.edit, EM_SETSCROLLPOS, 0, &q as *const _ as isize);
        return;
    }
    enviar(e.edit, WM_SETREDRAW, 0, 0);
    enviar(e.edit, EM_SETSCROLLPOS, 0, &q as *const _ as isize);
    enviar(e.edit, WM_SETREDRAW, 1, 0);
    // SAFETY: ventana propia.
    unsafe {
        let _ = RedrawWindow(Some(e.edit), None, None, RDW_INVALIDATE | RDW_UPDATENOW);
    }
}

// ---------------------------------------------------------------------------
// Muestras

/// **La nota pintada en memoria**, sin ensenar nada: la ventana oculta de
/// `tamano`, desde arriba, con el cursor al principio. Para las muestras de
/// la aplicacion (una nota con sus paginas vivas ya pintadas).
pub fn pintar_en_memoria(p: Pedido, claro: bool, tamano: (i32, i32)) -> windows::core::Result<pixpin_codec::ImagenRgba> {
    let mut e = montar(
        p,
        Opciones {
            oculto: true,
            tamano: Some(tamano),
            claro: Some(claro),
        },
    )?;
    elegir(e.edit, 0, 0);
    pintar(&mut e, None);
    let arriba = POINT::default();
    enviar(e.edit, EM_SETSCROLLPOS, 0, &arriba as *const _ as isize);
    let img = muestra(&e);
    desmontar(e);
    Ok(img)
}

/// La ventana entera pintada en memoria, como se veria: el marco, la nota
/// (con `WM_PRINTCLIENT`, fotos incluidas) y el menu abierto si lo hay.
fn muestra(e: &Estado) -> pixpin_codec::ImagenRgba {
    let mut r = RECT::default();
    // SAFETY: ventanas propias; DC y mapa de bits propios, soltados al final.
    unsafe {
        let _ = GetClientRect(e.marco, &mut r);
        let (an, al) = (r.right.max(1), r.bottom.max(1));
        let dc = CreateCompatibleDC(None);
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
        let mapa = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0).unwrap_or_default();
        let viejo = SelectObject(dc, HGDIOBJ(mapa.0));
        VISTA.with(|v| {
            if let Some(v) = v.borrow().as_ref() {
                pintar_marco(dc, v);
            }
        });
        let cuerpo = VISTA.with(|v| v.borrow().as_ref().map(|v| v.disp.cuerpo)).unwrap_or_default();
        // Lo de la nota, solo en el papel (lo que esta por encima de lo
        // visible tambien se pinta y pisaria la barra).
        let guardado = SaveDC(dc);
        IntersectClipRect(dc, cuerpo.x, cuerpo.y, cuerpo.derecha(), cuerpo.abajo());
        let mut antes = POINT::default();
        let _ = SetViewportOrgEx(dc, cuerpo.x - crate::tabla_ancha::hueco(), cuerpo.y, Some(&mut antes));
        SendMessageW(
            e.edit,
            WM_PRINTCLIENT,
            Some(WPARAM(dc.0 as usize)),
            Some(LPARAM((PRF_CLIENT | PRF_ERASEBKGND) as isize)),
        );
        let _ = SetViewportOrgEx(dc, antes.x, antes.y, None);
        let _ = RestoreDC(dc, guardado);
        // Las tarjetas de los comentarios (su ventana va encima de la nota).
        crate::panel_comentarios::pintar(dc, &e.pintor);
        // La barra flotante sobre lo elegido (una ventana aparte).
        wysiwyg::pintar_en_muestra(e, dc);
        if e.abierto.is_some() {
            menu::VISTA.with(|v| {
                if let Some(v) = v.borrow().as_ref() {
                    menu::pintar(dc, v, (e.menu_caja.x, e.menu_caja.y), e.menu_caja.al);
                }
            });
        }
        // El cuadro de escribir un comentario es una ventana aparte.
        comentarios::pintar_compositor(e, dc);
        let crudo = std::slice::from_raw_parts(bits as *const u8, (an * al * 4) as usize);
        let mut rgba = Vec::with_capacity(crudo.len());
        for p in crudo.chunks_exact(4) {
            rgba.extend_from_slice(&[p[2], p[1], p[0], 255]);
        }
        SelectObject(dc, viejo);
        let _ = DeleteObject(HGDIOBJ(mapa.0));
        let _ = DeleteDC(dc);
        pixpin_codec::ImagenRgba {
            ancho: an as u32,
            alto: al as u32,
            pixeles: rgba,
        }
    }
}

/// Pegar tablas, combinar celdas y sus colores (H12, tablas utiles).
mod tablas;

/// Pegar y soltar fotos, su tamano, abrirlas y las paginas vivas (H12).
mod fotos;

/// Comentar en cualquier parte de la nota, con su panel (H12, 30-sep).
pub mod comentarios;

/// Editar sin ver las marcas: el cursor las salta, borrar no las rompe, la
/// barra flotante, la letra elegida y el desplazamiento suave (H12, 30-sep).
mod wysiwyg;

/// Documentos, mensajes del chat, hojas enlazadas y audios con su
/// transcripcion, pintados como en el chat (H12, 1-oct).
mod incrustados;

/// Cambiar el texto con el pintado congelado: ni un fotograma con las
/// marcas a la vista (H12, 1-oct).
mod congelar;

/// Las asas para mover bloques, filas y columnas (H12, 1-oct).
mod asas;

/// Exportar la nota a Word desde la flecha del titulo (H12, 1-oct).
mod exportar;

#[cfg(test)]
mod pruebas;
