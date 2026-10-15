//! **Tablas de verdad utiles** (H12): pegar una tabla de Excel, Sheets,
//! Word, LibreOffice o una pagina; combinar y separar celdas; y pintar el
//! fondo y la letra de celdas, filas o columnas.
//!
//! Lo que el texto del control no dice de una tabla (que celdas estan
//! combinadas, sus colores, como se alinea cada una) lo sabe el control, y
//! se le pregunta con `ITextRow` (TOM de Windows 8): sus banderas de
//! combinar y el fondo de la celda; el color de la letra va en la galleta
//! (`CFM_COOKIE`) de la marca de fin de cada celda (ver `poner_tabla` y
//! `tabla_rtf`). Cambiar algo es lo de siempre: el modelo cambia y la tabla
//! se vuelve a escribir entera, como un paso de deshacer.

use super::*;
use pixpin_docs::md_tabla::{Formato, TablaEnControl, Vertical};
use pixpin_docs::md_tabla_html;

use crate::tabla_rtf::{FONDOS, LETRAS, letra_sobre};

pub(super) const C_COMBINAR: u16 = 200;
pub(super) const C_SEPARAR: u16 = 201;
/// El fondo: `C_FONDO + objetivo * 5 + color` (objetivo 0 las celdas
/// elegidas, 1 sus filas enteras, 2 sus columnas; color 0 «sin color» y
/// 1-4 los de la paleta). La letra igual desde `C_LETRA`.
pub(super) const C_FONDO: u16 = 210;
pub(super) const C_LETRA: u16 = 230;

pub(super) fn es_suyo(c: u16) -> bool {
    matches!(c, C_COMBINAR | C_SEPARAR)
        || (C_FONDO..C_FONDO + 15).contains(&c)
        || (C_LETRA..C_LETRA + 15).contains(&c)
}

// Las banderas de `ITextRow::GetCellMergeFlags` (medidas: 5 la que manda
// de una combinada de 2 x 2, 9 su vecina, 6 la de debajo, 10 la otra).
const V_ARRIBA: i32 = 1;
const V_SIGUE: i32 = 2;
const H_INICIO: i32 = 4;
const H_SIGUE: i32 = 8;

/// Un `COLORREF` de TOM a `0xRRGGBB`; `None` el automatico (`tomAutoColor`,
/// negativo).
fn rgb(cr: i32) -> Option<Rgb> {
    (cr >= 0).then(|| bgr(cr as u32))
}

/// La fila de la tabla cuya marca de abrir esta en `pos`.
fn fila_de(doc: &ITextDocument, pos: usize) -> Option<ITextRow> {
    // SAFETY: rango del documento vivo del control.
    unsafe {
        doc.Range(pos as i32, pos as i32)
            .ok()?
            .cast::<ITextRange2>()
            .ok()?
            .GetRow()
            .ok()
    }
}

/// Una celda segun el control.
struct Leida {
    banderas: i32,
    fondo: Option<Rgb>,
    abajo: bool,
}

fn leer_fila(fila: &ITextRow, n: usize) -> Vec<Leida> {
    // SAFETY: la fila es del documento vivo; se recorre por su indice.
    unsafe {
        let cuantas = fila.GetCellCount().unwrap_or(0).max(0) as usize;
        (0..n.min(cuantas))
            .map(|c| {
                let _ = fila.SetCellIndex(c as i32);
                Leida {
                    banderas: fila.GetCellMergeFlags().unwrap_or(0),
                    fondo: fila.GetCellColorBack().ok().and_then(rgb),
                    abajo: fila.GetCellAlignment().unwrap_or(0) == 2,
                }
            })
            .collect()
    }
}

thread_local! {
    /// Si alguna tabla de este hilo tiene combinadas, colores o una
    /// alineacion de celda: solo entonces se le pregunta al control celda a
    /// celda (las notas de siempre no pagan nada; el formato solo entra por
    /// [`poner_tabla`]).
    static CON_FORMATO: Cell<bool> = const { Cell::new(false) };
}

/// La galleta de una letra con color de celda: la marca y el color.
const CON_LETRA: u32 = 0x0100_0000;

/// Donde esta la marca de fin de la celda que empieza en `desde`.
fn fin_de_celda(u: &[u16], desde: usize) -> usize {
    (desde..u.len())
        .find(|&i| u[i] == md_tabla::CELDA as u16)
        .unwrap_or(desde)
}

/// **El color de la letra de una celda**, guardado en la galleta de su
/// marca de fin (`CFM_COOKIE`, que no se ve y que el formato en vivo no
/// toca). Medido: la trama de la celda (`\clcfpat`) no vale, el control
/// pierde algunos colores segun los que haya en la tabla.
fn letra_de(doc: &ITextDocument, fin: usize) -> Option<Rgb> {
    // SAFETY: rango del documento vivo del control.
    let g = unsafe {
        doc.Range(fin as i32, fin as i32 + 1)
            .ok()
            .and_then(|r| r.cast::<ITextRange2>().ok())
            .and_then(|r| r.GetFont2().ok())
            .and_then(|f| f.GetCookie().ok())
            .unwrap_or(0) as u32
    };
    (g & CON_LETRA != 0).then_some(g & 0xff_ffff)
}

/// **Pone la tabla `t` en lo elegido** (RTF, con o sin deshacer) y guarda
/// el color de la letra de cada celda en su galleta; las demas, sin.
pub(super) fn poner_tabla(e: &Estado, t: &Tabla, deshacer: bool) {
    // Reescribir una tabla ancha no la devuelve a su principio.
    let (desde, _) = seleccion(e.edit);
    let desplazada = desplazar::desplazamiento_en(e, desde);
    poner_y_pintar(e, t, deshacer);
    desplazar::colocar_en(e, desde, desplazada);
}

/// Mete la tabla (con sus anchos medidos con la letra de la nota) y pone
/// las galletas de sus letras pintadas.
fn poner_y_pintar(e: &Estado, t: &Tabla, deshacer: bool) {
    let (desde, _) = seleccion(e.edit);
    poner_rtf(e.edit, &desplazar::rtf_de(e, t), deshacer);
    if !t.formato.is_empty() {
        CON_FORMATO.with(|x| x.set(true));
    }
    let Some(doc) = &e.doc else {
        return;
    };
    // Sin ninguna tabla con formato no hay galletas que poner ni quitar.
    if !CON_FORMATO.with(|x| x.get()) {
        return;
    }
    let texto = leer(e.edit);
    let u: Vec<u16> = texto.encode_utf16().collect();
    let Some(tc) = md_tabla::tablas_en_control(&texto)
        .into_iter()
        .find(|x| x.desde == desde)
    else {
        return;
    };
    // SAFETY: documento vivo; la galleta no se deshace aparte (va con el
    // texto que se acaba de meter).
    unsafe {
        let _ = doc.Undo(tomSuspend.0);
        for (f, inicios) in tc.celdas.iter().enumerate() {
            for (c, &p) in inicios.iter().enumerate() {
                let (af, ac) = t.ancla(f, c);
                let g = t.formato(af, ac).letra.map_or(0, |l| CON_LETRA | l);
                let fin = fin_de_celda(&u, p);
                if let Ok(r) = doc.Range(p as i32, fin as i32 + 1)
                    && let Ok(r2) = r.cast::<ITextRange2>()
                    && let Ok(fuente) = r2.GetFont2()
                {
                    let _ = fuente.SetCookie(g as i32);
                }
            }
        }
        let _ = doc.Undo(tomResume.0);
    }
}

fn alineacion_en(doc: &ITextDocument, p: usize) -> Alineacion {
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
}

/// **Pone en `t` lo que sabe el control de la tabla `tc`**: alineaciones,
/// combinadas, colores y cabeceras. Un fondo del gris de cabecera del tema
/// es cabecera, no un color.
// La rejilla se recorre por filas y columnas a la vez (banderas de las
// vecinas): los indices son lo claro aqui.
#[allow(clippy::needless_range_loop)]
pub(super) fn completar(e: &Estado, tc: &TablaEnControl, t: &mut Tabla) {
    let Some(doc) = &e.doc else {
        return;
    };
    if !CON_FORMATO.with(|x| x.get()) {
        // Lo de siempre: la alineacion de cada columna, la de su cabecera.
        t.alineaciones = tc
            .celdas
            .first()
            .map(|fila| fila.iter().map(|&p| alineacion_en(doc, p)).collect())
            .unwrap_or_default();
        return;
    }
    let n = t.columnas();
    let alto = tc.celdas.len().min(t.filas.len());
    let gris = e.estilos.tema.tabla_cabecera;
    let u: Vec<u16> = leer(e.edit).encode_utf16().collect();
    let mut leidas: Vec<Vec<Leida>> = Vec::with_capacity(alto);
    let mut formato = vec![vec![Formato::default(); n]; alto];
    for (f, inicios) in tc.celdas.iter().enumerate().take(alto) {
        let fila = inicios
            .first()
            .and_then(|p| fila_de(doc, p.saturating_sub(2)));
        leidas.push(fila.map(|x| leer_fila(&x, n)).unwrap_or_default());
        for (c, &p) in inicios.iter().enumerate().take(n) {
            formato[f][c].alineacion = Some(alineacion_en(doc, p));
            formato[f][c].letra = letra_de(doc, fin_de_celda(&u, p));
        }
    }
    let banderas = |f: usize, c: usize| {
        leidas
            .get(f)
            .and_then(|x| x.get(c))
            .map_or(0, |l| l.banderas)
    };
    for f in 0..alto {
        for c in 0..n {
            let b = banderas(f, c);
            let x = &mut formato[f][c];
            if b & (H_SIGUE | V_SIGUE) != 0 {
                *x = Formato {
                    tapada: true,
                    ..Formato::default()
                };
                continue;
            }
            if b & H_INICIO != 0 {
                x.columnas = 1
                    + (c + 1..n)
                        .take_while(|k| banderas(f, *k) & H_SIGUE != 0)
                        .count();
            }
            if b & V_ARRIBA != 0 {
                x.filas = 1
                    + (f + 1..alto)
                        .take_while(|k| banderas(*k, c) & V_SIGUE != 0)
                        .count();
            }
            let Some(l) = leidas.get(f).and_then(|x| x.get(c)) else {
                continue;
            };
            let cabecera = match l.fondo {
                Some(g) if g == gris => true,
                Some(otro) => {
                    x.fondo = Some(otro);
                    f == 0
                }
                None => false,
            };
            x.cabecera = (cabecera != (f == 0)).then_some(cabecera);
            if l.abajo {
                x.vertical = Vertical::Abajo;
            }
        }
    }
    for fila in &mut t.filas {
        fila.resize(n, String::new());
    }
    t.formato = formato;
    t.canonica();
}

/// La tabla `tc` entera, con lo que sabe el control.
pub(super) fn modelo(e: &Estado, tc: &TablaEnControl) -> Tabla {
    let mut t = tc.tabla.clone();
    completar(e, tc, &mut t);
    t
}

/// **El color de las letras de las celdas pintadas**: el formato en vivo
/// pone todo el texto del color del tema, y despues de eso aqui se vuelve
/// a poner el de las celdas que lo tienen (el guardado en su galleta, o el
/// que se lee sobre su fondo).
pub(super) fn colorear_letras(
    e: &Estado,
    texto: &str,
    ls: &[md_vivo::Linea],
    entra: &dyn Fn(usize) -> bool,
) {
    let Some(doc) = &e.doc else {
        return;
    };
    if !CON_FORMATO.with(|x| x.get()) || !texto.contains(md_tabla::FILA_ABRE) {
        return;
    }
    let u: Vec<u16> = texto.encode_utf16().collect();
    let gris = e.estilos.tema.tabla_cabecera;
    for tc in md_tabla::tablas_en_control(texto) {
        for inicios in &tc.celdas {
            let Some(&p0) = inicios.first() else { continue };
            if !entra(md_vivo::linea_de(ls, p0)) {
                continue;
            }
            let Some(fila) = fila_de(doc, p0.saturating_sub(2)) else {
                continue;
            };
            for (c, l) in leer_fila(&fila, inicios.len()).into_iter().enumerate() {
                let desde = inicios[c];
                let hasta = fin_de_celda(&u, desde);
                let fondo = l.fondo.filter(|x| *x != gris);
                let Some(letra) = letra_de(doc, hasta).or(fondo.map(letra_sobre)) else {
                    continue;
                };
                if hasta > desde {
                    let mut f = formato();
                    f.Base.dwMask = CFM_COLOR;
                    f.Base.crTextColor = color(letra);
                    elegir(e.edit, desde, hasta);
                    poner_formato(e.edit, &f);
                }
            }
        }
    }
}

thread_local! {
    /// La letra donde se pulso y donde va el raton al arrastrar: al elegir
    /// celdas de varias filas el control alarga lo elegido a filas enteras
    /// (medido), y asi se sabe que rectangulo se arrastro de verdad.
    static ARRASTRE: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
}

/// Apunta el arrastre del raton sobre la nota. Nunca se queda el mensaje.
pub(super) fn raton(e: &Estado, m: &MSG) -> bool {
    let p = letra_en(e.edit, m.lParam);
    match m.message {
        WM_LBUTTONDOWN => ARRASTRE.with(|a| a.set(Some((p, p)))),
        WM_MOUSEMOVE if m.wParam.0 & 0x0001 != 0 => ARRASTRE.with(|a| {
            if let Some((ini, _)) = a.get() {
                a.set(Some((ini, p)));
            }
        }),
        WM_LBUTTONUP => ARRASTRE.with(|a| {
            if let Some((ini, _)) = a.get() {
                a.set(Some((ini, p)));
            }
        }),
        _ => {}
    }
    false
}

/// Lo mismo que un arrastre de la letra `desde` a la letra `hasta`: las
/// pruebas y el clic en la barrita de una columna (`asas`), que la elige
/// entera para colorearla o combinarla.
pub(super) fn recordar_arrastre(desde: usize, hasta: usize) {
    ARRASTRE.with(|a| a.set(Some((desde, hasta))));
}

/// La tabla de lo elegido y sus dos esquinas (fila, columna).
type Eleccion = (TablaEnControl, (usize, usize), (usize, usize));

/// La tabla, y las dos esquinas (fila, columna) de lo elegido en ella; con
/// el cursor solo, las dos son su celda.
fn elegido(e: &Estado) -> Option<Eleccion> {
    let texto = leer(e.edit);
    if !texto.contains(md_tabla::FILA_ABRE) {
        return None;
    }
    let u: Vec<u16> = texto.encode_utf16().collect();
    let tablas = md_tabla::tablas_en_control(&texto);
    let (mut a, b) = seleccion(e.edit);
    // Si lo elegido es lo que se arrastro, el rectangulo del arrastre.
    if let Some((p1, p2)) = ARRASTRE.with(|x| x.get())
        && a < b
        && a <= p1.min(p2)
        && p1.max(p2) <= b
        && let (Some(x), Some(y)) = (
            md_tabla::celda_en(&tablas, p1),
            md_tabla::celda_en(&tablas, p2),
        )
        && x.0 == y.0
    {
        let tc = tablas.into_iter().nth(x.0)?;
        return Some((tc, (x.1, x.2), (y.1, y.2)));
    }
    // Una fila elegida entera empieza en su marca: su primera celda.
    if u.get(a) == Some(&(md_tabla::FILA_ABRE as u16)) {
        a += 2;
    }
    let (k, f1, c1) = md_tabla::celda_en(&tablas, a)?;
    let fin = if b > a { b - 1 } else { a };
    let (_, f2, c2) = md_tabla::celda_en(&tablas, fin)
        .filter(|x| x.0 == k)
        .unwrap_or((k, f1, c1));
    let tc = tablas.into_iter().nth(k)?;
    Some((tc, (f1, c1), (f2, c2)))
}

/// Vuelve a escribir la tabla `tc` con `t`, con el cursor en `(f, c)`.
fn reescribir(e: &mut Estado, tc: &TablaEnControl, t: &Tabla, destino: (usize, usize)) {
    elegir(e.edit, tc.desde, tc.hasta);
    poner_tabla(e, t, true);
    ir_a_celda(e, tc.desde, destino.0, destino.1);
    apuntar(Orden::Cambio);
}

/// Las ordenes de este modulo.
pub(super) fn comando(e: &mut Estado, c: u16) {
    let Some((tc, a, b)) = elegido(e) else {
        return;
    };
    let mut t = modelo(e, &tc);
    match c {
        C_COMBINAR => {
            let (f1, _, c1, _) = t.rectangulo(a, b);
            if t.combinar(a, b) {
                reescribir(e, &tc, &t, (f1, c1));
            }
        }
        C_SEPARAR => {
            if t.separar(b.0, b.1) {
                let ancla = t.ancla(b.0, b.1);
                reescribir(e, &tc, &t, ancla);
            }
        }
        _ => {
            let (base, paleta) = if c >= C_LETRA {
                (C_LETRA, LETRAS)
            } else {
                (C_FONDO, FONDOS)
            };
            let (objetivo, cual) = ((c - base) / 5, ((c - base) % 5) as usize);
            let color = (cual > 0).then(|| paleta[cual - 1]);
            let (alto, ancho) = (t.filas.len().max(1), t.columnas());
            let (de, hasta) = match objetivo {
                1 => ((a.0.min(b.0), 0), (a.0.max(b.0), ancho - 1)),
                2 => ((0, a.1.min(b.1)), (alto - 1, a.1.max(b.1))),
                _ => (a, b),
            };
            if base == C_LETRA {
                t.poner_letra(de, hasta, color);
            } else {
                t.poner_fondo(de, hasta, color);
            }
            reescribir(e, &tc, &t, a);
        }
    }
}

/// Si lo elegido son varias celdas (para combinar) y si la del cursor esta
/// combinada (para separar).
pub(super) fn se_puede(e: &Estado) -> (bool, bool) {
    let Some((tc, a, b)) = elegido(e) else {
        return (false, false);
    };
    let t = modelo(e, &tc);
    let combinable = t.clone().combinar(a, b);
    let (af, ac) = t.ancla(b.0, b.1);
    let x = t.formato(af, ac);
    (combinable, x.filas > 1 || x.columnas > 1)
}

/// El boton «Combinar» de la barra: combina lo elegido, y si es una sola
/// celda que ya esta combinada, la separa.
pub(super) fn combinar_o_separar(e: &mut Estado) {
    match se_puede(e) {
        (true, _) => comando(e, C_COMBINAR),
        (false, true) => comando(e, C_SEPARAR),
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Pegar

/// **Ctrl+V con una tabla en el portapapeles**: `true` si se pego como
/// tabla; si no (texto, una sola celda, media pagina), lo de siempre.
pub(super) fn pegar(e: &mut Estado) -> bool {
    let Some((fragmento, entero, texto)) =
        pixpin_codec::portapapeles::tabla::leer_tabla_con_estilos()
    else {
        return false;
    };
    let tabla = fragmento
        .as_deref()
        .and_then(|h| md_tabla_html::leer_pegado(h, entero.as_deref()))
        .or_else(|| texto.as_deref().and_then(md_tabla_html::leer_tsv));
    match tabla {
        Some(t) => {
            pegar_tabla(e, &t);
            true
        }
        None => false,
    }
}

/// Pega `nueva`: dentro de una tabla la rellena desde la celda del cursor
/// (como una hoja de calculo, creciendo si hace falta); fuera, es una tabla
/// nueva en su renglon.
pub(super) fn pegar_tabla(e: &mut Estado, nueva: &Tabla) {
    match celda_del_cursor(e) {
        Some((tc, f, c)) => {
            let mut t = modelo(e, &tc);
            let (f, c) = t.ancla(f, c);
            t.pegar_en(f, c, nueva);
            reescribir(e, &tc, &t, (f, c));
        }
        None => meter_tabla(e, nueva),
    }
}

// ---------------------------------------------------------------------------
// Los menus

fn nombres(lista: &str, defecto: &[&str; 5]) -> Vec<String> {
    let v: Vec<String> = lista.split('|').map(|s| s.trim().to_string()).collect();
    if v.len() == 5 && v.iter().all(|s| !s.is_empty()) {
        v
    } else {
        defecto.iter().map(|s| s.to_string()).collect()
    }
}

fn nombres_fondo(r: &Rotulos) -> Vec<String> {
    nombres(
        &r.nombres_fondo,
        &["Sin color", "Rojo", "Verde", "Azul", "Amarillo"],
    )
}

fn nombres_letra(r: &Rotulos) -> Vec<String> {
    nombres(
        &r.nombres_letra,
        &["Automático", "Rojo", "Verde", "Azul", "Naranja"],
    )
}

fn o(s: &str, defecto: &str) -> String {
    if s.is_empty() {
        defecto.to_string()
    } else {
        s.to_string()
    }
}

/// **La paleta de la barra** (boton «Color»): fondo y letra de las celdas
/// elegidas, con su muestra de color, en el menu propio del editor.
pub(super) fn entradas_de_color(e: &Estado) -> Vec<Entrada> {
    let r = &e.rotulos;
    let mut v = Vec::new();
    for (i, nombre) in nombres_fondo(r).into_iter().enumerate() {
        let c = (i > 0).then(|| FONDOS[i - 1]);
        let atajo = if i == 0 {
            o(&r.color_fondo, "Color de fondo")
        } else {
            String::new()
        };
        v.push(entrada(
            C_FONDO + i as u16,
            Dibujo::Fondo(c),
            &nombre,
            &atajo,
        ));
    }
    for (i, nombre) in nombres_letra(r).into_iter().enumerate() {
        let c = (i > 0).then(|| LETRAS[i - 1]);
        let atajo = if i == 0 {
            o(&r.color_letra, "Color de la letra")
        } else {
            String::new()
        };
        v.push(entrada(
            C_LETRA + i as u16,
            Dibujo::Tinta(c),
            &nombre,
            &atajo,
        ));
    }
    v
}

/// Los dos botones de la tabla en la barra.
pub(super) fn rotulo_combinar(r: &Rotulos) -> String {
    o(&r.boton_combinar, "Combinar")
}

pub(super) fn rotulo_color(r: &Rotulos) -> String {
    o(&r.boton_color, "Color")
}

/// La pista de debajo de la paleta.
pub(super) fn pista_de_color(e: &Estado) -> String {
    o(
        &e.rotulos.pista_color,
        "Filas y columnas enteras: clic derecho",
    )
}

/// Un cuadradito de color para una entrada del menu de Windows (con una
/// raya del color abajo si es de letra).
fn muestra_de(c: Option<Rgb>, tinta: bool, tema: &Tema, lado: i32) -> HBITMAP {
    // SAFETY: DC y mapa de bits propios; el mapa lo suelta `soltar`.
    unsafe {
        let pantalla = GetDC(None);
        let dc = CreateCompatibleDC(Some(pantalla));
        let mapa = CreateCompatibleBitmap(pantalla, lado, lado);
        ReleaseDC(None, pantalla);
        let viejo = SelectObject(dc, HGDIOBJ(mapa.0));
        let caja = |a: i32, b: i32, c: i32, d: i32| RECT {
            left: a,
            top: b,
            right: c,
            bottom: d,
        };
        let pintar_caja = |r: RECT, x: Rgb| {
            let p = CreateSolidBrush(color(x));
            let _ = FillRect(dc, &r, p);
            let _ = DeleteObject(HGDIOBJ(p.0));
        };
        pintar_caja(caja(0, 0, lado, lado), tema.raya);
        pintar_caja(
            caja(1, 1, lado - 1, lado - 1),
            if tinta {
                tema.menu
            } else {
                c.unwrap_or(tema.menu)
            },
        );
        if tinta {
            pintar_caja(
                caja(3, lado - 5, lado - 3, lado - 2),
                c.unwrap_or(tema.texto),
            );
        }
        SelectObject(dc, viejo);
        let _ = DeleteDC(dc);
        mapa
    }
}

/// Un submenu de colores en tres columnas: celdas elegidas, sus filas y
/// sus columnas, con un cuadradito de cada color.
fn submenu(
    e: &Estado,
    base: u16,
    nombres_color: &[String],
    tinta: bool,
    mapas: &mut Vec<HBITMAP>,
) -> Option<HMENU> {
    let mut objetivos: Vec<String> = e
        .rotulos
        .aplicar_a
        .split('|')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if objetivos.len() != 3 {
        objetivos = vec![
            "Celdas".into(),
            "Fila entera".into(),
            "Columna entera".into(),
        ];
    }
    let lado = 14 * super::PPP.with(|p| p.get()) / 96;
    let paleta = if tinta { LETRAS } else { FONDOS };
    // SAFETY: el menu y sus mapas viven hasta que quien llama los suelta;
    // las cadenas, durante cada llamada.
    unsafe {
        let sub = CreatePopupMenu().ok()?;
        for (k, objetivo) in objetivos.iter().take(3).enumerate() {
            let salto = if k > 0 {
                MF_MENUBARBREAK
            } else {
                MENU_ITEM_FLAGS(0)
            };
            let _ = AppendMenuW(
                sub,
                MF_STRING | MF_GRAYED | salto,
                0,
                &HSTRING::from(objetivo.as_str()),
            );
            for (i, nombre) in nombres_color.iter().enumerate() {
                let id = base + (k * 5 + i) as u16;
                let _ = AppendMenuW(sub, MF_STRING, id as usize, &HSTRING::from(nombre.as_str()));
                let mapa = muestra_de((i > 0).then(|| paleta[i - 1]), tinta, &e.estilos.tema, lado);
                mapas.push(mapa);
                let info = MENUITEMINFOW {
                    cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
                    fMask: MIIM_BITMAP,
                    hbmpItem: mapa,
                    ..Default::default()
                };
                let _ = SetMenuItemInfoW(sub, id as u32, false, &info);
            }
        }
        Some(sub)
    }
}

/// **Lo de la tabla en el clic derecho**: combinar, separar y los dos
/// submenus de color, metidos en `menu` desde la posicion `pos`. Devuelve
/// los mapas de bits de las muestras, que se sueltan al cerrar el menu.
pub(super) fn al_menu_contextual(e: &Estado, menu: HMENU, pos: u32) -> Vec<HBITMAP> {
    let r = &e.rotulos;
    let (combinar, separar) = se_puede(e);
    let mut mapas = Vec::new();
    let gris = |si: bool| if si { MF_ENABLED } else { MF_GRAYED };
    let fondo = submenu(e, C_FONDO, &nombres_fondo(r), false, &mut mapas);
    let letra = submenu(e, C_LETRA, &nombres_letra(r), true, &mut mapas);
    let mut p = pos;
    let mut meter = |flags: MENU_ITEM_FLAGS, id: usize, texto: &str| {
        // SAFETY: menu vivo de quien llama; la cadena vive en la llamada.
        unsafe {
            let _ = InsertMenuW(menu, p, MF_BYPOSITION | flags, id, &HSTRING::from(texto));
        }
        p += 1;
    };
    meter(MF_SEPARATOR, 0, "");
    meter(
        MF_STRING | gris(combinar),
        C_COMBINAR as usize,
        &o(&r.combinar_celdas, "Combinar celdas"),
    );
    meter(
        MF_STRING | gris(separar),
        C_SEPARAR as usize,
        &o(&r.separar_celdas, "Separar celdas"),
    );
    if let Some(s) = fondo {
        meter(
            MF_STRING | MF_POPUP,
            s.0 as usize,
            &o(&r.color_fondo, "Color de fondo"),
        );
    }
    if let Some(s) = letra {
        meter(
            MF_STRING | MF_POPUP,
            s.0 as usize,
            &o(&r.color_letra, "Color de la letra"),
        );
    }
    mapas
}

/// Suelta los mapas de las muestras (el menu ya se destruyo).
pub(super) fn soltar(mapas: Vec<HBITMAP>) {
    for m in mapas {
        // SAFETY: mapas propios, ya sin menu que los use.
        unsafe {
            let _ = DeleteObject(HGDIOBJ(m.0));
        }
    }
}

#[cfg(test)]
mod pruebas;

/// Cada tabla con su ancho y su propio desplazamiento de lado.
pub(super) mod desplazar;
