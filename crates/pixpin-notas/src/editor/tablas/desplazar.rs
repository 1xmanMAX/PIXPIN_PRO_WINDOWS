//! **Cada tabla ancha con su propio desplazamiento de lado** y su barra
//! fina debajo, como en Claude (H12). Por que con sangrias de fila y un
//! hueco a la izquierda del papel, y no con una ventana por tabla: ver
//! `crate::tabla_ancha`.
//!
//! Lo que esta desplazada cada tabla no se apunta en ningun sitio: es su
//! sangria (`ITextRow::GetIndent`), asi que deshacer, rehacer y Tab en la
//! ultima celda (la fila nueva copia la de encima) lo conservan solos. Para
//! pintar la barra y atender el raton sin el estado del bucle, lo de cada
//! tabla ancha se apunta en [`VISTAS`] cada vez que se colocan.

use std::cell::RefCell;

use windows::Win32::Foundation::SIZE;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetCapture, ReleaseCapture, SetCapture};

use super::*;
use crate::tabla_ancha::{self, Colocacion};

/// Lo que se sabe de una tabla ancha para pintar su barra.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::editor) struct Vista {
    pub desde: usize,
    pub hasta: usize,
    /// Su ancho en pixeles de la pantalla.
    pub ancho: i32,
    pub c: Colocacion,
    /// Lo que se ve del papel, en x del control.
    pub vis_izq: i32,
    pub vis_der: i32,
}

thread_local! {
    pub(in crate::editor) static VISTAS: RefCell<Vec<Vista>> = const { RefCell::new(Vec::new()) };
    /// El papel y el pulgar de la barra.
    static COLORES: Cell<(Rgb, Rgb)> = const { Cell::new((0x0b0b0b, 0x5c5b58)) };
    /// Arrastrando el pulgar: la tabla y donde se agarro (x del raton menos
    /// la del pulgar).
    static ARRASTRE: Cell<Option<(usize, i32)>> = const { Cell::new(None) };
}

/// Alto del pulgar, su separacion de la tabla y lo que se puede pulsar de
/// alto, a 96 ppp.
const GRUESO_PX: i32 = 6;
const SEPARACION_PX: i32 = 5;
const ZONA_PX: i32 = 16;
/// El pulgar nunca mas corto que esto.
const PULGAR_MINIMO_PX: i32 = 32;
/// Lo que se corre por cada paso de la rueda.
const PASO_PX: i32 = 60;
/// El aire que se deja a cada lado del cursor al seguirlo.
const AIRE_CURSOR_PX: i32 = 40;

fn esc(e: &Estado, v: i32) -> i32 {
    v * e.ppp / 96
}

// ---------------------------------------------------------------------------
// Medir el texto

/// **La letra del cuerpo, para medir** las celdas al calcular los anchos
/// (GDI, a 16 px: 12 pt a 96 ppp, la letra de la nota).
pub(in crate::editor) struct Medidor {
    dc: HDC,
    letra: HFONT,
    vieja: HGDIOBJ,
}

impl Medidor {
    pub fn nuevo(cara: &str) -> Medidor {
        // SAFETY: DC de memoria y letra propios, soltados en `drop`.
        unsafe {
            let dc = CreateCompatibleDC(None);
            let letra = CreateFontW(
                -16,
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                0,
                &HSTRING::from(cara),
            );
            let vieja = SelectObject(dc, HGDIOBJ(letra.0));
            Medidor { dc, letra, vieja }
        }
    }

    /// Lo que mide `s` en una linea, en pixeles a 96 ppp.
    pub fn medir(&self, s: &str) -> i32 {
        let u: Vec<u16> = s.encode_utf16().collect();
        if u.is_empty() {
            return 0;
        }
        let mut t = SIZE::default();
        // SAFETY: DC propio con la letra puesta; el texto vive en la llamada.
        unsafe {
            let _ = GetTextExtentPoint32W(self.dc, &u, &mut t);
        }
        t.cx
    }
}

impl Drop for Medidor {
    fn drop(&mut self) {
        // SAFETY: lo creado en `nuevo`.
        unsafe {
            SelectObject(self.dc, self.vieja);
            let _ = DeleteObject(HGDIOBJ(self.letra.0));
            let _ = DeleteDC(self.dc);
        }
    }
}

/// La tabla en RTF con los anchos medidos con la letra de la nota.
pub(in crate::editor) fn rtf_de(e: &Estado, t: &Tabla) -> String {
    let m = Medidor::nuevo(&e.estilos.letras.cuerpo);
    tabla_rtf::rtf_con(t, &estilo_tabla(e), &|s| m.medir(s))
}

// ---------------------------------------------------------------------------
// Colocar

/// Lo que se ve de una tabla ancha, en x del control: el papel entero (no
/// la columna de texto: el usuario, 1-oct, «no quiero que este limitado»).
/// Lo que limita es hasta donde se corre (ver [`columna`]).
pub(in crate::editor) fn visible(edit: HWND) -> (i32, i32) {
    let mut r = RECT::default();
    let mut c = RECT::default();
    enviar(edit, 0x00B2, 0, &mut r as *mut _ as isize);
    // SAFETY: ventana propia; estructura local.
    unsafe {
        let _ = GetClientRect(edit, &mut c);
    }
    (r.left + tabla_ancha::hueco(), r.right.min(c.right))
}

/// Lo ancho de la columna de texto, en pixeles: una tabla mas ancha se corre
/// de lado, desde empezar donde el texto hasta acabar donde acaba el texto
/// (`tabla_ancha::colocar_en_columna`), asi nunca se va entera a un lado.
pub(in crate::editor) fn columna(edit: HWND) -> i32 {
    let (izq, der) = visible(edit);
    let ppp = ppp_del_control();
    (der - izq - tabla_ancha::sobra_px(ppp) - tabla_ancha::sobra_der_px(ppp)).max(1)
}

/// La sangria (veinteavos), si va a la izquierda, y el ancho (veinteavos)
/// de la fila que empieza en `pos`.
fn leer_fila_entera(doc: &ITextDocument, pos: usize) -> Option<(i32, bool, i32)> {
    let fila = fila_de(doc, pos)?;
    // SAFETY: fila del documento vivo.
    unsafe {
        let n = fila.GetCellCount().ok()?.max(0);
        let mut ancho = 0;
        for c in 0..n {
            let _ = fila.SetCellIndex(c);
            ancho += fila.GetCellWidth().unwrap_or(0);
        }
        Some((fila.GetIndent().unwrap_or(0), fila.GetAlignment().unwrap_or(1) == 0, ancho))
    }
}

/// Pone a todas las filas de `tc` la sangria `twips`, a la izquierda o
/// centradas. Sin deshacer ni avisos: es colocar, no escribir.
fn sangrar(e: &Estado, tc: &TablaEnControl, izquierda: bool, twips: i32) {
    let Some(doc) = &e.doc else {
        return;
    };
    let mascara = enviar(e.edit, EM_GETEVENTMASK, 0, 0);
    enviar(e.edit, EM_SETEVENTMASK, 0, ENM_NONE as isize);
    // SAFETY: documento vivo; cada fila se carga, se cambia y se aplica
    // sola (aplicar varias de golpe copiaria las celdas de la primera, y
    // con ellas sus combinadas y colores).
    unsafe {
        let _ = doc.Undo(tomSuspend.0);
        // Congelado, el control no repinta fila a fila.
        let _ = doc.Freeze();
        for inicios in &tc.celdas {
            let Some(&p) = inicios.first() else { continue };
            if let Some(fila) = fila_de(doc, p.saturating_sub(2)) {
                let _ = fila.SetAlignment(if izquierda { 0 } else { 1 });
                let _ = fila.SetIndent(twips);
                let _ = fila.Apply(1, tomConstants(0));
            }
        }
        let _ = doc.Unfreeze();
        let _ = doc.Undo(tomResume.0);
    }
    enviar(e.edit, EM_SETEVENTMASK, 0, mascara);
}

/// Lo desplazada que esta una tabla segun su sangria.
fn desplazamiento_de(e: &Estado, sangria_tw: i32, izquierda: bool) -> i32 {
    if !izquierda || tabla_ancha::hueco_lleno() == 0 {
        return 0;
    }
    tabla_ancha::hueco_lleno() - sangria_tw * e.ppp / 1440
}

/// Lo desplazada que esta la tabla que empieza en `desde` (para conservarlo
/// al reescribirla).
pub(in crate::editor) fn desplazamiento_en(e: &Estado, desde: usize) -> i32 {
    let Some(doc) = &e.doc else {
        return 0;
    };
    // Solo si ahi empieza una tabla: en otro renglon el control da la fila
    // de la tabla mas cercana (medido), y una tabla nueva heredaria su sitio.
    if leer(e.edit).encode_utf16().nth(desde) != Some(md_tabla::FILA_ABRE as u16) {
        return 0;
    }
    match leer_fila_entera(doc, desde) {
        Some((s, izq, _)) => desplazamiento_de(e, s, izq).max(0),
        None => 0,
    }
}

/// **Coloca todas las tablas**: si alguna no cabe, el hueco; cada una en su
/// sitio con lo que estaba desplazada (`pedido` cambia el de una: orden de
/// la tabla y desplazamiento nuevo). Apunta las anchas para la barra.
pub(in crate::editor) fn colocar_todas(e: &Estado, pedido: Option<(usize, i32)>) {
    COLORES.with(|c| c.set(colores_de(&e.estilos.tema)));
    let texto = leer(e.edit);
    if !texto.contains(md_tabla::FILA_ABRE) {
        let habia = VISTAS.with(|v| !std::mem::take(&mut *v.borrow_mut()).is_empty());
        if habia {
            invalidar(e.edit);
        }
        return;
    }
    let Some(doc) = &e.doc else {
        return;
    };
    let tablas = md_tabla::tablas_en_control(&texto);
    let (vis_izq, vis_der) = visible(e.edit);
    // Lo que manda es la columna de texto: hasta donde se corre la tabla.
    let visible_px = columna(e.edit);
    let medidas: Vec<Option<(i32, bool, i32)>> = tablas.iter().map(|tc| leer_fila_entera(doc, tc.desde)).collect();
    let px = |tw: i32| tw * e.ppp / 1440;
    // Una tabla que no cabe y aun no hay hueco: se abre, y el texto se
    // vuelve a sangrar con el (ver `margen_de`).
    if tabla_ancha::hueco_lleno() == 0 && medidas.iter().flatten().any(|m| px(m.2) > visible_px) {
        tabla_ancha::poner_hueco(true, e.ppp);
        colocar_edit(e.marco);
        apuntar(Orden::Tamano);
        return colocar_todas(e, pedido);
    }
    let (lleno, oculto) = (tabla_ancha::hueco_lleno(), tabla_ancha::hueco());
    let mut vistas = Vec::new();
    for (k, (tc, m)) in tablas.iter().zip(&medidas).enumerate() {
        let Some((sangria, izquierda, ancho_tw)) = *m else {
            continue;
        };
        let ancho = px(ancho_tw);
        let ahora = desplazamiento_de(e, sangria, izquierda);
        let quiere = pedido.filter(|p| p.0 == k).map_or(ahora, |p| p.1);
        let c = tabla_ancha::colocar_en_columna(ancho, visible_px, lleno, oculto, quiere);
        // Ancha, a la izquierda; si cabe, centrada (ver `Colocacion`).
        let tw = if c.ancha() { c.sangria * 1440 / e.ppp } else { tabla_ancha::hueco_twips() };
        if izquierda != c.ancha() || (tw - sangria).abs() > 20 {
            sangrar(e, tc, c.ancha(), tw);
        }
        if c.ancha() {
            vistas.push(Vista {
                desde: tc.desde,
                hasta: tc.hasta,
                ancho,
                c,
                vis_izq,
                vis_der,
            });
        }
    }
    let cambio = VISTAS.with(|v| {
        let mut v = v.borrow_mut();
        let cambio = *v != vistas;
        *v = vistas;
        cambio
    });
    if cambio {
        invalidar(e.edit);
    }
}

/// Tras el `WM_PAINT` del control.
pub(in crate::editor) fn repintar(edit: HWND) {
    if VISTAS.with(|v| v.borrow().is_empty()) {
        return;
    }
    // SAFETY: DC de una ventana propia, pedido y devuelto aqui.
    unsafe {
        let dc = GetDC(Some(edit));
        pintar_encima(edit, dc);
        ReleaseDC(Some(edit), dc);
    }
}

/// Al cerrar la ventana: sin hueco ni tablas apuntadas para la siguiente.
pub(in crate::editor) fn olvidar() {
    tabla_ancha::poner_hueco(false, 96);
    VISTAS.with(|v| v.borrow_mut().clear());
    ARRASTRE.with(|a| a.set(None));
}

fn invalidar(edit: HWND) {
    // SAFETY: ventana propia.
    unsafe {
        let _ = InvalidateRect(Some(edit), None, false);
    }
}

/// **Tras cada tecla o clic**: coloca las tablas y, si el cursor esta en
/// una tabla ancha fuera de lo que se ve, la desplaza lo justo para verlo.
pub(in crate::editor) fn seguir(e: &Estado) {
    colocar_todas(e, None);
    let (a, b) = seleccion(e.edit);
    let Some(v) = VISTAS.with(|v| v.borrow().iter().copied().find(|x| x.desde <= b && b < x.hasta)) else {
        return;
    };
    // Arrastrando una eleccion que sale de la tabla, no se persigue.
    if a != b && (a < v.desde || a >= v.hasta) {
        return;
    }
    let mut p = POINT::default();
    enviar(e.edit, EM_POSFROMCHAR, &mut p as *mut _ as usize, b as isize);
    let aire = esc(e, AIRE_CURSOR_PX);
    let d = tabla_ancha::para_ver(&v.c, p.x - aire, p.x + aire, v.vis_izq, v.vis_der);
    if d != v.c.desplazamiento {
        desplazar(e, v.desde, d);
    }
}

/// Desplaza la tabla que empieza en `desde` a `d` (y coloca las demas).
pub(in crate::editor) fn desplazar(e: &Estado, desde: usize, d: i32) {
    let texto = leer(e.edit);
    let k = md_tabla::tablas_en_control(&texto).iter().position(|t| t.desde == desde);
    colocar_todas(e, k.map(|k| (k, d)));
}

/// Tras meter una tabla en `desde`: coloca todas, y esa con lo que estaba
/// desplazada la que habia.
pub(in crate::editor) fn colocar_en(e: &Estado, desde: usize, d: i32) {
    desplazar(e, desde, d);
}

fn colores_de(t: &Tema) -> (Rgb, Rgb) {
    (t.papel, if t.oscuro { 0x5c5b58 } else { 0xc4c2bb })
}

// ---------------------------------------------------------------------------
// La barra

/// Donde va la barra de una tabla: lo alto de la tabla y la caja de la barra
/// (x, y, ancho) en el control.
fn caja_de(edit: HWND, v: &Vista, ppp: i32) -> (i32, i32, i32, i32) {
    let mut arriba = POINT::default();
    let mut abajo = POINT::default();
    enviar(edit, EM_POSFROMCHAR, &mut arriba as *mut _ as usize, v.desde as isize);
    enviar(edit, EM_POSFROMCHAR, &mut abajo as *mut _ as usize, v.hasta as isize);
    let y = abajo.y + SEPARACION_PX * ppp / 96;
    (arriba.y, abajo.y, y, v.vis_der - v.vis_izq)
}

fn ppp_del_control() -> i32 {
    crate::imagenes::PPP.with(|p| p.get()).max(96)
}

/// **Pinta encima de lo del control** lo de las tablas anchas: tapa con el
/// papel lo que asoma por los margenes (la tabla se corta en el borde del
/// papel, como en Claude) y pinta la barra fina con su pulgar.
pub(in crate::editor) fn pintar_encima(edit: HWND, hdc: HDC) {
    let vistas = VISTAS.with(|v| v.borrow().clone());
    if vistas.is_empty() {
        return;
    }
    let ppp = ppp_del_control();
    let (papel, pulgar_color) = COLORES.with(|c| c.get());
    let mut cliente = RECT::default();
    // SAFETY: ventana y DC de quien llama; pinceles propios, soltados aqui.
    unsafe {
        let _ = GetClientRect(edit, &mut cliente);
        let fondo = CreateSolidBrush(color(papel));
        let tinta = CreateSolidBrush(color(pulgar_color));
        let pluma = GetStockObject(NULL_PEN);
        let vieja_pluma = SelectObject(hdc, pluma);
        let viejo_pincel = SelectObject(hdc, HGDIOBJ(tinta.0));
        for v in &vistas {
            let (arriba, abajo, y, an) = caja_de(edit, v, ppp);
            if abajo < cliente.top || arriba > cliente.bottom {
                continue;
            }
            // La letra de la primera fila va centrada en alto: la raya de
            // arriba queda por encima. En los margenes no hay mas que tabla.
            let arriba = arriba - tabla_rtf::ALTO_FILA_PX * ppp / 96;
            let hueco = tabla_ancha::hueco();
            let izq = RECT {
                left: hueco.min(v.vis_izq),
                top: arriba,
                right: v.vis_izq,
                bottom: abajo,
            };
            let der = RECT {
                left: v.vis_der + 1,
                top: arriba,
                right: cliente.right,
                bottom: abajo,
            };
            let _ = FillRect(hdc, &izq, fondo);
            let _ = FillRect(hdc, &der, fondo);
            let grueso = GRUESO_PX * ppp / 96;
            let (px, largo) = tabla_ancha::pulgar(v.vis_izq, an, v.ancho, &v.c, PULGAR_MINIMO_PX * ppp / 96);
            let _ = RoundRect(hdc, px, y, px + largo + 1, y + grueso + 1, grueso, grueso);
        }
        SelectObject(hdc, viejo_pincel);
        SelectObject(hdc, vieja_pluma);
        let _ = DeleteObject(HGDIOBJ(fondo.0));
        let _ = DeleteObject(HGDIOBJ(tinta.0));
    }
}

/// La tabla ancha cuya barra esta bajo `(x, y)` (del control).
fn barra_en(edit: HWND, x: i32, y: i32) -> Option<Vista> {
    let ppp = ppp_del_control();
    VISTAS.with(|v| {
        v.borrow().iter().copied().find(|v| {
            let (_, abajo, _, _) = caja_de(edit, v, ppp);
            x >= v.vis_izq && x < v.vis_der && y >= abajo && y < abajo + ZONA_PX * ppp / 96
        })
    })
}

/// La tabla ancha bajo `(x, y)` (del control), con su barra.
fn tabla_en(edit: HWND, x: i32, y: i32) -> Option<Vista> {
    let ppp = ppp_del_control();
    VISTAS.with(|v| {
        v.borrow().iter().copied().find(|v| {
            let (arriba, abajo, _, _) = caja_de(edit, v, ppp);
            x >= v.vis_izq && x < v.vis_der && y >= arriba && y < abajo + ZONA_PX * ppp / 96
        })
    })
}

/// Sobre la barra, la flecha (no la I de escribir).
pub(in crate::editor) fn cursor_de_la_barra(edit: HWND) -> bool {
    if VISTAS.with(|v| v.borrow().is_empty()) {
        return false;
    }
    let mut p = POINT::default();
    // SAFETY: posicion del raton y conversion a una ventana propia.
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut p);
        let _ = ScreenToClient(edit, &mut p);
    }
    if ARRASTRE.with(|a| a.get()).is_some() || barra_en(edit, p.x, p.y).is_some() {
        // SAFETY: cursor del sistema.
        unsafe {
            SetCursor(LoadCursorW(None, IDC_ARROW).ok());
        }
        return true;
    }
    false
}

/// **El raton**: Mayus+rueda o la rueda de lado sobre una tabla ancha la
/// desplazan; pulsar en su barra la arrastra (o salta adonde se pulso).
/// `true` si el mensaje era de esto (el control no lo ve).
pub(in crate::editor) fn raton(e: &Estado, m: &MSG) -> bool {
    if VISTAS.with(|v| v.borrow().is_empty()) && ARRASTRE.with(|a| a.get()).is_none() {
        return false;
    }
    let (x, y) = (m.lParam.0 as i16 as i32, (m.lParam.0 >> 16) as i16 as i32);
    match m.message {
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            let de_lado = m.message == WM_MOUSEHWHEEL;
            if !de_lado && !pulsada(VK_SHIFT.0) {
                return false;
            }
            // La rueda trae la posicion en la pantalla.
            let mut p = POINT { x, y };
            // SAFETY: conversion a una ventana propia.
            unsafe {
                let _ = ScreenToClient(e.edit, &mut p);
            }
            let Some(v) = tabla_en(e.edit, p.x, p.y) else {
                return false;
            };
            let giro = (m.wParam.0 >> 16) as i16 as i32;
            // Rueda abajo (negativa) con Mayus: a la derecha, como en el
            // navegador; la de lado, positiva a la derecha.
            let paso = giro * esc(e, PASO_PX) / 120;
            let d = if de_lado { v.c.desplazamiento + paso } else { v.c.desplazamiento - paso };
            desplazar(e, v.desde, d);
            true
        }
        WM_LBUTTONDOWN => {
            let Some(v) = barra_en(e.edit, x, y) else {
                return false;
            };
            let an = v.vis_der - v.vis_izq;
            let minimo = esc(e, PULGAR_MINIMO_PX);
            let (px, largo) = tabla_ancha::pulgar(v.vis_izq, an, v.ancho, &v.c, minimo);
            // Fuera del pulgar: salta para que quede centrado donde se pulso.
            let agarre = if x >= px && x < px + largo { x - px } else { largo / 2 };
            let d = tabla_ancha::desde_pulgar(x - agarre, v.vis_izq, an, v.ancho, &v.c, minimo);
            ARRASTRE.with(|a| a.set(Some((v.desde, agarre))));
            // SAFETY: ventana propia.
            unsafe {
                SetCapture(e.edit);
            }
            desplazar(e, v.desde, d);
            true
        }
        WM_MOUSEMOVE => {
            let Some((desde, agarre)) = ARRASTRE.with(|a| a.get()) else {
                return false;
            };
            let Some(v) = VISTAS.with(|v| v.borrow().iter().copied().find(|v| v.desde == desde)) else {
                return true;
            };
            let an = v.vis_der - v.vis_izq;
            let d = tabla_ancha::desde_pulgar(x - agarre, v.vis_izq, an, v.ancho, &v.c, esc(e, PULGAR_MINIMO_PX));
            if d != v.c.desplazamiento {
                desplazar(e, v.desde, d);
            }
            true
        }
        WM_LBUTTONUP => {
            if ARRASTRE.with(|a| a.take()).is_none() {
                return false;
            }
            // SAFETY: la captura es de esta ventana.
            unsafe {
                if GetCapture() == e.edit {
                    let _ = ReleaseCapture();
                }
            }
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod pruebas;
