//! **Las asas para mover** (H12, 1-oct-2026), como en el editor de
//! documentos de Claude (captura del usuario: los puntos `⋮⋮` a la
//! izquierda de la tabla y las barritas encima de cada columna). El usuario:
//! «esos puntos que tienen en la esquina sirven para mover el contenido… y
//! donde se mueve se proyecta una linea azul para saber donde se puede
//! poner; ademas las lineas de arriba que sirven para lo mismo pero para
//! mover la fila o columna: cada vez que mi mouse pasa por ahi aparecen».
//!
//! - **El asa de bloque** sale a la izquierda del bloque que tiene el raton
//!   (parrafo, titulo, lista con lo suyo, cita, codigo, foto o pagina viva,
//!   tabla entera; ver `pixpin_docs::md_bloques`). Arrastrarla ensena una
//!   **linea azul** entre bloques donde caera; al soltar se mueve entero, en
//!   un solo paso de deshacer; Esc lo deja.
//! - **En una tabla**, con el raton en una celda salen una barrita encima
//!   de su columna y otra a la izquierda de su fila: arrastrarlas reordena la
//!   columna o la fila (linea azul vertical u horizontal), con sus colores y
//!   sin partir combinadas (`Tabla::mover_fila`); un clic las elige enteras.
//!
//! Se pinta encima del control como las fotos (despues de su `WM_PAINT`),
//! en el margen y entre renglones: el texto no cambia hasta soltar.

use std::cell::{Cell, RefCell};

use pixpin_docs::md_bloques::{self, Bloque};
use pixpin_docs::md_tabla::TablaEnControl;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};

use super::*;

/// Lo que se coge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Asa {
    Bloque(Bloque),
    /// La fila `f` de la tabla que empieza en `desde`.
    Fila {
        desde: usize,
        f: usize,
    },
    /// La columna `c` de la tabla que empieza en `desde`.
    Columna {
        desde: usize,
        c: usize,
    },
    /// El «+» de debajo de la tabla: una fila mas al final.
    MasFila {
        desde: usize,
    },
    /// El «+» de su derecha: una columna mas al final.
    MasColumna {
        desde: usize,
    },
}

/// Un arrastre a medias: que se cogio, donde, si ya se movio el raton y a
/// donde caeria (renglon, fila o columna de delante).
#[derive(Debug, Clone, Copy)]
struct Arrastre {
    asa: Asa,
    x0: i32,
    y0: i32,
    movido: bool,
    destino: Option<usize>,
}

/// Lo que se pinta encima, en pixeles del control.
#[derive(Default)]
struct Pinta {
    asas: Vec<(Asa, RECT)>,
    /// La asa que tiene el raton encima (se resalta).
    encima: Option<Asa>,
    /// La linea azul de donde caera.
    linea: Option<RECT>,
    tenue: COLORREF,
    acento: COLORREF,
    pastilla: COLORREF,
    papel: COLORREF,
}

thread_local! {
    static PINTA: RefCell<Pinta> = RefCell::new(Pinta::default());
    static ARRASTRE: Cell<Option<Arrastre>> = const { Cell::new(None) };
    /// Un clic en una barrita acaba de elegir su fila o su columna entera.
    static ENTERA: Cell<bool> = const { Cell::new(false) };
}

/// Si el ultimo clic eligio una fila o una columna entera (y lo olvida): la
/// nota no se mueve por eso (`editor::quedarse_en`).
pub(super) fn eligio_entera() -> bool {
    ENTERA.with(Cell::take)
}

/// Cuanto hay que mover el raton para que un clic pase a ser arrastre.
const UMBRAL_PX: i32 = 4;

fn esc(e: &Estado, v: i32) -> i32 {
    v * e.ppp / 96
}

fn punto(l: LPARAM) -> (i32, i32) {
    (
        (l.0 & 0xffff) as i16 as i32,
        ((l.0 >> 16) & 0xffff) as i16 as i32,
    )
}

fn y_de(e: &Estado, pos: usize) -> i32 {
    let mut p = POINT::default();
    enviar(
        e.edit,
        EM_POSFROMCHAR,
        &mut p as *mut _ as usize,
        pos as isize,
    );
    p.y
}

fn x_de(e: &Estado, pos: usize) -> i32 {
    let mut p = POINT::default();
    enviar(
        e.edit,
        EM_POSFROMCHAR,
        &mut p as *mut _ as usize,
        pos as isize,
    );
    p.x
}

/// Donde empieza y acaba la columna de texto, en x del control.
fn columna(e: &Estado) -> (i32, i32) {
    let mut dentro = RECT::default();
    enviar(e.edit, 0x00B2, 0, &mut dentro as *mut _ as isize);
    let margen = e.estilos.margen * e.ppp / 1440;
    // La sangria ya lleva el hueco de las tablas anchas; a la derecha, solo
    // el margen de la columna.
    let derecha = crate::tabla_ancha::sobra_der_px(e.ppp);
    (dentro.left + margen, dentro.right - derecha)
}

fn renglon_px(e: &Estado) -> i32 {
    imagenes::ADORNOS
        .with(|a| a.borrow().as_ref().map(|a| a.renglon_px))
        .unwrap_or(esc(e, 24))
}

fn dentro(r: &RECT, x: i32, y: i32) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

// ---------------------------------------------------------------------------
// Donde va cada cosa

/// **La rejilla de una tabla** en el control: las x de los bordes de sus
/// columnas (una mas que columnas) y las y de los de sus filas.
struct Rejilla {
    xs: Vec<i32>,
    ys: Vec<i32>,
}

fn rejilla(e: &Estado, tc: &TablaEnControl) -> Option<Rejilla> {
    let doc = e.doc.as_ref()?;
    let primera = tc.celdas.first()?;
    let p0 = *primera.first()?;
    // SAFETY: rango y fila del documento vivo del control.
    let (anchos, margen) = unsafe {
        // La fila se pide por su marca de abrir (donde empieza la tabla).
        let fila = doc
            .Range(tc.desde as i32, tc.desde as i32)
            .ok()?
            .cast::<ITextRange2>()
            .ok()?
            .GetRow()
            .ok()?;
        let n = fila.GetCellCount().ok()?.max(0);
        let mut v = Vec::new();
        for c in 0..n {
            let _ = fila.SetCellIndex(c);
            v.push(fila.GetCellWidth().unwrap_or(0) * e.ppp / 1440);
        }
        (v, fila.GetCellMargin().unwrap_or(0) * e.ppp / 1440)
    };
    let mut xs = vec![x_de(e, p0) - margen];
    for a in anchos.iter().take(primera.len()) {
        let x = *xs.last()? + a;
        xs.push(x);
    }
    let mut ys: Vec<i32> = tc
        .celdas
        .iter()
        .filter_map(|f| f.first())
        .map(|p| y_de(e, *p) - esc(e, 3))
        .collect();
    let abajo = y_de(e, tc.hasta).max(*ys.last()? + renglon_px(e));
    ys.push(abajo);
    Some(Rejilla { xs, ys })
}

/// **Las asas que salen con el raton en `(x, y)`**: la del bloque de ese
/// alto y, en una tabla, las barritas de su fila y su columna.
fn asas_en(e: &Estado, x: i32, y: i32) -> Vec<(Asa, RECT)> {
    let texto = leer(e.edit);
    let ls = md_vivo::lineas(&texto);
    let (col_izq, col_der) = columna(e);
    let mut v = Vec::new();
    let p = POINT {
        x: col_izq + esc(e, 4),
        y,
    };
    let pos = enviar(e.edit, EM_CHARFROMPOS, 0, &p as *const _ as isize).max(0) as usize;
    let n = md_vivo::linea_de(&ls, pos);
    let bloques = md_bloques::bloques(&texto);
    if let Some(b) = md_bloques::bloque_en(&bloques, n) {
        let arriba = y_de(e, ls[b.desde].desde);
        let r = RECT {
            left: col_izq - esc(e, 30),
            top: arriba + esc(e, 2),
            right: col_izq - esc(e, 12),
            bottom: arriba + esc(e, 22),
        };
        v.push((Asa::Bloque(b), r));
    }
    // En una tabla: la barrita de la columna y la de la fila del raton.
    let tablas = md_tabla::tablas_en_control(&texto);
    let _ = col_der;
    for tc in &tablas {
        let Some(r) = rejilla(e, tc) else { continue };
        let (Some(&x0), Some(&x1), Some(&y0), Some(&y1)) =
            (r.xs.first(), r.xs.last(), r.ys.first(), r.ys.last())
        else {
            continue;
        };
        // Con aire a la derecha y debajo, para llegar a los «+» sin que se vayan.
        if x < x0 - esc(e, 30) || x > x1 + esc(e, 30) || y < y0 - esc(e, 16) || y > y1 + esc(e, 26)
        {
            continue;
        }
        // Los «+» de anadir una fila y una columna, como en Claude: debajo de
        // la tabla a su izquierda y a la derecha de su cabecera. Si la tabla
        // es ancha, en el borde de lo que se ve de ella.
        let (vis_izq, vis_der) = super::tablas::desplazar::visible(e.edit);
        let (xi, xd) = if x1 - x0 > vis_der - vis_izq {
            (vis_izq.max(x0), vis_der.min(x1))
        } else {
            (x0, x1)
        };
        let lado = esc(e, 20);
        v.push((
            Asa::MasFila { desde: tc.desde },
            RECT {
                left: xi - lado - esc(e, 4),
                top: y1 + esc(e, 3),
                right: xi - esc(e, 4),
                bottom: y1 + esc(e, 3) + lado,
            },
        ));
        v.push((
            Asa::MasColumna { desde: tc.desde },
            RECT {
                left: xd + esc(e, 4),
                top: y0 + esc(e, 2),
                right: xd + esc(e, 4) + lado,
                bottom: y0 + esc(e, 2) + lado,
            },
        ));
        if x > x1 || y > y1 {
            continue;
        }
        if let Some(c) = (0..r.xs.len() - 1).find(|&c| x >= r.xs[c] && x < r.xs[c + 1]) {
            let medio = (r.xs[c] + r.xs[c + 1]) / 2;
            let rc = RECT {
                left: medio - esc(e, 14),
                top: y0 - esc(e, 11),
                right: medio + esc(e, 14),
                bottom: y0 - esc(e, 5),
            };
            v.push((Asa::Columna { desde: tc.desde, c }, rc));
        }
        if let Some(f) = (0..r.ys.len() - 1).find(|&f| y >= r.ys[f] && y < r.ys[f + 1]) {
            let medio = (r.ys[f] + r.ys[f + 1]) / 2;
            let rf = RECT {
                left: x0 - esc(e, 11),
                top: medio - esc(e, 12),
                right: x0 - esc(e, 5),
                bottom: medio + esc(e, 12),
            };
            v.push((Asa::Fila { desde: tc.desde, f }, rf));
        }
    }
    v
}

/// **Donde caeria** lo cogido con el raton en `(x, y)`: el sitio y la linea
/// azul que lo ensena.
fn destino(e: &Estado, asa: Asa, x: i32, y: i32) -> Option<(usize, RECT)> {
    let texto = leer(e.edit);
    let ls = md_vivo::lineas(&texto);
    let (col_izq, col_der) = columna(e);
    let grueso = esc(e, 3).max(2);
    match asa {
        Asa::Bloque(b) => {
            let bloques = md_bloques::bloques(&texto);
            let sitios = md_bloques::sitios(&bloques, b);
            let alto_de = |k: usize| -> i32 {
                match ls.get(k) {
                    Some(l) => y_de(e, l.desde),
                    None => {
                        let u = ls.last().map(|l| l.hasta).unwrap_or(0);
                        y_de(e, u) + renglon_px(e)
                    }
                }
            };
            let (k, yk) = sitios
                .iter()
                .map(|&k| (k, alto_de(k)))
                .min_by_key(|(_, yk)| (yk - y).abs())?;
            // Tambien hacia los lados se ve: la linea cruza la columna entera.
            let r = RECT {
                left: col_izq - esc(e, 8),
                top: yk - grueso / 2 - esc(e, 1),
                right: col_der + esc(e, 8),
                bottom: yk + grueso / 2,
            };
            Some((k, r))
        }
        Asa::Fila { desde, f } => {
            let tc = md_tabla::tablas_en_control(&texto)
                .into_iter()
                .find(|t| t.desde == desde)?;
            let r = rejilla(e, &tc)?;
            let t = &tc.tabla;
            let (i, j) = t.banda_de_filas(f);
            let a = (0..r.ys.len())
                .filter(|&a| !(i..=j).contains(&a) && t.corte_de_filas(a))
                .min_by_key(|&a| (r.ys[a] - y).abs())?;
            let (x0, x1) = (*r.xs.first()?, *r.xs.last()?);
            Some((
                a,
                RECT {
                    left: x0 - esc(e, 4),
                    top: r.ys[a] - grueso / 2,
                    right: x1 + esc(e, 4),
                    bottom: r.ys[a] + grueso / 2 + 1,
                },
            ))
        }
        Asa::Columna { desde, c } => {
            let tc = md_tabla::tablas_en_control(&texto)
                .into_iter()
                .find(|t| t.desde == desde)?;
            let r = rejilla(e, &tc)?;
            let t = &tc.tabla;
            let (i, j) = t.banda_de_columnas(c);
            let a = (0..r.xs.len())
                .filter(|&a| !(i..=j).contains(&a) && t.corte_de_columnas(a))
                .min_by_key(|&a| (r.xs[a] - x).abs())?;
            let (y0, y1) = (*r.ys.first()?, *r.ys.last()?);
            Some((
                a,
                RECT {
                    left: r.xs[a] - grueso / 2,
                    top: y0 - esc(e, 4),
                    right: r.xs[a] + grueso / 2 + 1,
                    bottom: y1 + esc(e, 4),
                },
            ))
        }
        // Los «+» no se arrastran.
        Asa::MasFila { .. } | Asa::MasColumna { .. } => None,
    }
}

// ---------------------------------------------------------------------------
// Mover

/// Si entre los renglones `a` y `b` (los dos dentro) hay una tabla.
fn hay_tabla(texto: &str, a: usize, b: usize) -> bool {
    let tipos = pixpin_docs::md_edicion::renglones(texto);
    (a..=b).any(|n| tipos.get(n) == Some(&pixpin_docs::md_edicion::Renglon::Tabla))
}

/// **Mueve el bloque `b` delante del renglon `antes_de`**, como un paso de
/// deshacer. Sin tablas de por medio, el texto se cambia de una vez (con
/// lo que no cambia sin tocar); con una tabla, se copia con su formato (el
/// control no la entiende como texto) y se borra de donde estaba, juntando
/// los dos cambios en un solo paso de deshacer.
pub(super) fn mover_bloque(e: &mut Estado, b: Bloque, antes_de: usize) -> bool {
    congelar::congelado(e, congelar::Pintado::Entero, |e| {
        let texto = leer(e.edit);
        let (lo, hi) = (b.desde.min(antes_de), b.hasta.max(antes_de));
        if !hay_tabla(&texto, lo, hi) {
            let Some((nuevo, donde)) = md_bloques::mover(&texto, b, antes_de) else {
                return false;
            };
            aplicar_cambio(e.edit, &texto, &nuevo);
            let l = md_vivo::lineas(&nuevo)[donde];
            elegir(e.edit, l.desde, l.desde);
            return true;
        }
        mover_con_formato(e, b, antes_de)
    })
}

fn mover_con_formato(e: &mut Estado, b: Bloque, antes_de: usize) -> bool {
    let Some(doc) = e.doc.clone() else {
        return false;
    };
    let texto = leer(e.edit);
    let ls = md_vivo::lineas(&texto);
    if b.hasta >= ls.len() || antes_de > ls.len() || (b.desde..=b.hasta + 1).contains(&antes_de) {
        return false;
    }
    let junto = doc.cast::<ITextDocument2>().ok();
    // SAFETY: el documento del control, vivo mientras el; los rangos son
    // suyos y siguen al texto mientras se cambia.
    unsafe {
        if let Some(j) = &junto {
            let _ = j.BeginEditCollection();
        }
        // Lo cogido y el sitio tienen que acabar en un salto: si el bloque es
        // el ultimo o se suelta al final, se pone un renglon vacio al final y
        // luego se quita.
        let mut extra = false;
        if b.hasta + 1 >= ls.len() || antes_de >= ls.len() {
            let fin = texto.encode_utf16().count() as i32;
            if let Ok(r) = doc.Range(fin, fin) {
                let _ = r.SetText(&windows::core::BSTR::from("\r"));
                extra = true;
            }
        }
        let texto = leer(e.edit);
        let ls = md_vivo::lineas(&texto);
        let hecho = (|| -> windows::core::Result<()> {
            let a = ls[b.desde].desde as i32;
            let z = (ls[b.hasta].hasta + 1) as i32;
            let k = ls[antes_de.min(ls.len() - 1)].desde as i32;
            let origen = doc.Range(a, z)?;
            let copia = origen.GetFormattedText()?;
            let sitio = doc.Range(k, k)?;
            sitio.SetFormattedText(&copia)?;
            // El origen sigue a su texto: si se metio delante, ya corrio.
            origen.SetText(&windows::core::BSTR::new())?;
            Ok(())
        })()
        .is_ok();
        if extra {
            // El renglon de mas se quita, salvo que lo de antes sea una tabla:
            // detras de una tabla el control necesita un parrafo (quitarlo
            // se llevaria su ultima fila).
            let t = leer(e.edit);
            let n = t.encode_utf16().count() as i32;
            let tipos = pixpin_docs::md_edicion::renglones(&t);
            let antes_tabla = tipos.len() >= 2
                && tipos[tipos.len() - 2] == pixpin_docs::md_edicion::Renglon::Tabla;
            if t.ends_with('\n')
                && !antes_tabla
                && let Ok(r) = doc.Range(n - 1, n)
            {
                let _ = r.SetText(&windows::core::BSTR::new());
            }
        }
        if let Some(j) = &junto {
            let _ = j.EndEditCollection();
        }
        hecho
    }
}

/// Reordena la fila o la columna de una tabla: se cambia el modelo y se
/// vuelve a escribir la tabla entera en su sitio, como un paso de deshacer.
fn mover_en_tabla(e: &mut Estado, asa: Asa, a: usize) -> bool {
    congelar::congelado(e, congelar::Pintado::Entero, |e| {
        let texto = leer(e.edit);
        let desde = match asa {
            Asa::Fila { desde, .. } | Asa::Columna { desde, .. } => desde,
            _ => return false,
        };
        let Some(tc) = md_tabla::tablas_en_control(&texto)
            .into_iter()
            .find(|t| t.desde == desde)
        else {
            return false;
        };
        let mut t = tablas::modelo(e, &tc);
        let (hecho, celda) = match asa {
            Asa::Fila { f, .. } => {
                let (i, j) = t.banda_de_filas(f);
                let nueva = if a > j { a - (j - i) } else { a } + (f - i);
                (t.mover_fila(f, a), (nueva, 0))
            }
            Asa::Columna { c, .. } => {
                let (i, j) = t.banda_de_columnas(c);
                let nueva = if a > j { a - (j - i) } else { a } + (c - i);
                (t.mover_columna(c, a), (0, nueva))
            }
            _ => (false, (0, 0)),
        };
        if hecho {
            elegir(e.edit, tc.desde, tc.hasta);
            tablas::poner_tabla(e, &t, true);
            ir_a_celda(e, tc.desde, celda.0, celda.1);
        }
        hecho
    })
}

/// Un clic en una barrita (sin arrastrar): elige su fila o su columna
/// entera (para colorearla o combinarla).
fn elegir_en_tabla(e: &Estado, asa: Asa) {
    let texto = leer(e.edit);
    let desde = match asa {
        Asa::Fila { desde, .. } | Asa::Columna { desde, .. } => desde,
        _ => return,
    };
    let Some(tc) = md_tabla::tablas_en_control(&texto)
        .into_iter()
        .find(|t| t.desde == desde)
    else {
        return;
    };
    let (p1, p2) = match asa {
        Asa::Fila { f, .. } => match tc.celdas.get(f) {
            Some(fila) => (fila.first().copied(), fila.last().copied()),
            None => return,
        },
        Asa::Columna { c, .. } => (
            tc.celdas.first().and_then(|f| f.get(c)).copied(),
            tc.celdas.last().and_then(|f| f.get(c)).copied(),
        ),
        _ => return,
    };
    let (Some(p1), Some(p2)) = (p1, p2) else {
        return;
    };
    let u: Vec<u16> = texto.encode_utf16().collect();
    let fin = p2
        + u[p2.min(u.len())..]
            .iter()
            .take_while(|c| {
                **c != md_tabla::CELDA as u16 && **c != b'\r' as u16 && **c != b'\n' as u16
            })
            .count();
    tablas::recordar_arrastre(p1, p2);
    elegir(e.edit, p1, fin.max(p1 + 1));
    ENTERA.with(|x| x.set(true));
}

// ---------------------------------------------------------------------------
// El raton y el teclado

fn refrescar(edit: HWND, rects: &[RECT]) {
    for r in rects {
        let r = RECT {
            left: r.left - 2,
            top: r.top - 2,
            right: r.right + 2,
            bottom: r.bottom + 2,
        };
        // SAFETY: rectangulo de una ventana propia.
        unsafe {
            let _ = InvalidateRect(Some(edit), Some(&r), false);
        }
    }
}

fn poner(e: &Estado, asas: Vec<(Asa, RECT)>, encima: Option<Asa>, linea: Option<RECT>) {
    let t = &e.estilos.tema;
    let mut viejas = Vec::new();
    let cambio = PINTA.with(|p| {
        let mut p = p.borrow_mut();
        let igual = p.asas == asas && p.encima == encima && p.linea == linea;
        if !igual {
            viejas.extend(p.asas.iter().map(|(_, r)| *r));
            viejas.extend(p.linea);
            p.asas = asas.clone();
            p.encima = encima;
            p.linea = linea;
        }
        p.tenue = color(t.tenue);
        p.acento = color(t.acento);
        p.pastilla = color(t.pastilla);
        p.papel = color(t.papel);
        !igual
    });
    if cambio && !e.oculto {
        viejas.extend(asas.iter().map(|(_, r)| *r));
        viejas.extend(linea);
        refrescar(e.edit, &viejas);
    }
}

/// Se van las asas (el raton salio de la nota).
pub(super) fn esconder(e: &Estado) {
    if ARRASTRE.with(Cell::get).is_none() {
        poner(e, Vec::new(), None, None);
    }
}

/// **El raton sobre la nota**: ensena las asas, empieza, sigue y acaba un
/// arrastre. `true` si el mensaje era suyo.
pub(super) fn raton(e: &mut Estado, m: &MSG) -> bool {
    let (x, y) = punto(m.lParam);
    match m.message {
        WM_MOUSEMOVE => {
            if let Some(mut a) = ARRASTRE.with(Cell::get) {
                a.movido |=
                    (x - a.x0).abs() > esc(e, UMBRAL_PX) || (y - a.y0).abs() > esc(e, UMBRAL_PX);
                let d = if a.movido {
                    destino(e, a.asa, x, y)
                } else {
                    None
                };
                a.destino = d.map(|(k, _)| k);
                ARRASTRE.with(|x| x.set(Some(a)));
                let asas = PINTA.with(|p| p.borrow().asas.clone());
                poner(e, asas, Some(a.asa), d.map(|(_, r)| r));
                return true;
            }
            // Si el raton esta sobre una asa ya puesta, se queda (se va de la
            // tabla hacia su barrita y la barrita no se va).
            let puestas = PINTA.with(|p| p.borrow().asas.clone());
            if let Some((asa, _)) = puestas.iter().find(|(_, r)| dentro(r, x, y)) {
                let asa = *asa;
                poner(e, puestas, Some(asa), None);
                return false;
            }
            poner(e, asas_en(e, x, y), None, None);
            if !e.oculto {
                let mut t = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: e.edit,
                    dwHoverTime: 0,
                };
                // SAFETY: estructura local; ventana propia.
                unsafe {
                    let _ = TrackMouseEvent(&mut t);
                }
            }
            false
        }
        WM_LBUTTONDOWN => {
            let puestas = PINTA.with(|p| p.borrow().asas.clone());
            let Some((asa, _)) = puestas.iter().find(|(_, r)| dentro(r, x, y)).copied() else {
                return false;
            };
            cerrar_menu(e);
            ARRASTRE.with(|a| {
                a.set(Some(Arrastre {
                    asa,
                    x0: x,
                    y0: y,
                    movido: false,
                    destino: None,
                }))
            });
            if !e.oculto {
                // SAFETY: captura del raton para una ventana propia.
                unsafe {
                    SetCapture(e.edit);
                }
            }
            true
        }
        WM_LBUTTONUP => {
            let Some(a) = ARRASTRE.with(Cell::take) else {
                return false;
            };
            // SAFETY: suelta la captura de este hilo.
            unsafe {
                let _ = ReleaseCapture();
            }
            match (a.movido, a.destino, a.asa) {
                (true, Some(k), Asa::Bloque(b)) => {
                    mover_bloque(e, b, k);
                }
                (true, Some(k), asa) => {
                    mover_en_tabla(e, asa, k);
                }
                (false, _, asa @ (Asa::Fila { .. } | Asa::Columna { .. })) => {
                    elegir_en_tabla(e, asa)
                }
                (false, _, Asa::MasFila { desde }) => super::anadir_al_final(e, desde, true),
                (false, _, Asa::MasColumna { desde }) => super::anadir_al_final(e, desde, false),
                _ => {}
            }
            poner(e, Vec::new(), None, None);
            true
        }
        WM_MOUSELEAVE => {
            esconder(e);
            false
        }
        _ => false,
    }
}

/// Esc a mitad de un arrastre: se deja todo como estaba. `true` si habia
/// uno.
pub(super) fn cancelar(e: &Estado) -> bool {
    if ARRASTRE.with(Cell::take).is_none() {
        return false;
    }
    // SAFETY: suelta la captura de este hilo.
    unsafe {
        let _ = ReleaseCapture();
    }
    poner(e, Vec::new(), None, None);
    true
}

/// Sobre una asa o arrastrando: la mano (se coge algo).
pub(super) fn cursor(edit: HWND) -> bool {
    let mut p = POINT::default();
    // SAFETY: posicion del raton y conversion para una ventana propia.
    unsafe {
        let _ = GetCursorPos(&mut p);
        let _ = ScreenToClient(edit, &mut p);
    }
    let encima = ARRASTRE.with(Cell::get).is_some()
        || PINTA.with(|x| x.borrow().asas.iter().any(|(_, r)| dentro(r, p.x, p.y)));
    if encima {
        // SAFETY: cursor del sistema, compartido; no se suelta.
        unsafe {
            if let Ok(c) = LoadCursorW(None, IDC_SIZEALL) {
                SetCursor(Some(c));
            }
        }
    }
    encima
}

// ---------------------------------------------------------------------------
// Pintar

fn rellenar(hdc: HDC, r: RECT, c: COLORREF) {
    // SAFETY: pincel propio, creado y soltado aqui, sobre un DC prestado.
    unsafe {
        let pincel = CreateSolidBrush(c);
        FillRect(hdc, &r, pincel);
        let _ = DeleteObject(HGDIOBJ(pincel.0));
    }
}

fn redondo(hdc: HDC, r: RECT, c: COLORREF, radio: i32) {
    // SAFETY: pincel propio; la pluma es la de sistema (nula); el DC se deja
    // como estaba.
    unsafe {
        let pincel = CreateSolidBrush(c);
        let viejo = SelectObject(hdc, HGDIOBJ(pincel.0));
        let pluma = SelectObject(hdc, GetStockObject(NULL_PEN));
        let _ = RoundRect(hdc, r.left, r.top, r.right + 1, r.bottom + 1, radio, radio);
        SelectObject(hdc, pluma);
        SelectObject(hdc, viejo);
        let _ = DeleteObject(HGDIOBJ(pincel.0));
    }
}

/// Pinta las asas y la linea azul en `hdc` (el del control o una muestra).
pub(super) fn pintar_encima(hdc: HDC) {
    PINTA.with(|p| {
        let p = p.borrow();
        let ppp = imagenes::PPP.with(|x| x.get());
        let e = |v: i32| v * ppp / 96;
        for (asa, r) in &p.asas {
            let encima = p.encima == Some(*asa);
            match asa {
                Asa::Bloque(_) => {
                    if encima {
                        redondo(hdc, *r, p.pastilla, e(6));
                    }
                    // Seis puntos, dos columnas de tres: `⋮⋮`.
                    let lado = e(3).max(2);
                    let cx = (r.left + r.right) / 2;
                    let cy = (r.top + r.bottom) / 2;
                    for dx in [-e(3), e(3)] {
                        for dy in [-e(5), 0, e(5)] {
                            let (x, y) = (cx + dx - lado / 2, cy + dy - lado / 2);
                            redondo(
                                hdc,
                                RECT {
                                    left: x,
                                    top: y,
                                    right: x + lado,
                                    bottom: y + lado,
                                },
                                p.tenue,
                                lado,
                            );
                        }
                    }
                }
                Asa::MasFila { .. } | Asa::MasColumna { .. } => {
                    // Un circulo con su «+», como el de Claude.
                    redondo(
                        hdc,
                        *r,
                        if encima { p.acento } else { p.pastilla },
                        r.right - r.left,
                    );
                    let (cx, cy) = ((r.left + r.right) / 2, (r.top + r.bottom) / 2);
                    let (largo, grueso) = (e(5), e(1).max(1));
                    let tinta = if encima { p.papel } else { p.tenue };
                    rellenar(
                        hdc,
                        RECT {
                            left: cx - largo,
                            top: cy - grueso,
                            right: cx + largo + 1,
                            bottom: cy + grueso,
                        },
                        tinta,
                    );
                    rellenar(
                        hdc,
                        RECT {
                            left: cx - grueso,
                            top: cy - largo,
                            right: cx + grueso,
                            bottom: cy + largo + 1,
                        },
                        tinta,
                    );
                }
                _ => redondo(hdc, *r, if encima { p.acento } else { p.tenue }, e(6)),
            }
        }
        if let Some(l) = p.linea {
            rellenar(hdc, l, p.acento);
        }
    });
}

/// La nota se movio: las asas puestas ya no estan sobre su fila (siguen al
/// raton, que no se movio). Se quitan; el repintado entero ya va en camino.
/// Arrastrando, se quedan.
pub(super) fn al_desplazar() {
    if ARRASTRE.with(Cell::get).is_some() {
        return;
    }
    PINTA.with(|p| {
        let mut p = p.borrow_mut();
        p.asas.clear();
        p.encima = None;
    });
}

/// Tras un `WM_PAINT` del control: encima, ya.
pub(super) fn repintar(edit: HWND) {
    let hay = PINTA.with(|p| {
        let p = p.borrow();
        !p.asas.is_empty() || p.linea.is_some()
    });
    if !hay {
        return;
    }
    // SAFETY: DC de una ventana propia, pedido y devuelto aqui.
    unsafe {
        let dc = GetDC(Some(edit));
        pintar_encima(dc);
        ReleaseDC(Some(edit), dc);
    }
}

/// Lo que ocupan las asas puestas (para borrarlas al desplazarse).
pub(super) fn puestas() -> Vec<RECT> {
    PINTA.with(|p| {
        let p = p.borrow();
        p.asas.iter().map(|(_, r)| *r).chain(p.linea).collect()
    })
}

/// Al cerrar la ventana.
pub(super) fn olvidar() {
    PINTA.with(|p| *p.borrow_mut() = Pinta::default());
    ARRASTRE.with(|a| a.set(None));
}

#[cfg(test)]
pub(super) fn a_la_vista() -> Vec<(Asa, RECT)> {
    PINTA.with(|p| p.borrow().asas.clone())
}

#[cfg(test)]
pub(super) fn linea_azul() -> Option<RECT> {
    PINTA.with(|p| p.borrow().linea)
}

#[cfg(test)]
mod pruebas;
