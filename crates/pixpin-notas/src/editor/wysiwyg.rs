//! **Editar como en Claude, sin ver el Markdown** (H12, 30-sep). El usuario:
//! «cuando le doy click al texto se muestran sus asteriscos o hashtags, que
//! no deberia ser asi»; «anadele la posibilidad de cambiar letras… tamano de
//! letra»; «su scroll es extrano».
//!
//! - **Las marcas no se ven nunca** (`formato_de`), tampoco en el renglon
//!   del cursor, pero siguen en el texto: el `.md` es el mismo de siempre y
//!   el movil lo lee igual. El cursor no se queda delante de la marca de un
//!   titulo o una lista (`md_edicion::cursor_valido`); Retroceso y Supr
//!   borran lo que se ve sin dejar `**` sueltos (`md_edicion::tecla_borrar`);
//!   escribir encima de lo elegido, cortar o pegar quitan solo lo visible.
//! - **Los numeros, las casillas, las rayas y las citas** los pone el
//!   parrafo o se pintan encima (`imagenes`), como en Claude.
//! - **La barra flotante** sobre lo elegido (`barra_flotante`) pone y quita
//!   las marcas escondidas: B, I, S, codigo, quitar formato, «Aa ▾»,
//!   listas, enlace (pide la direccion en un cuadro), emoji y comentar. Sin
//!   nada elegido, Ctrl+B deja la negrita «pendiente» para lo que se
//!   escriba.
//! - **La letra y el tamano** son de la vista (`vista`), no del texto: las
//!   ocho letras del lienzo, cuatro tamanos con nombre y Ctrl+rueda de uno
//!   en uno; generales o solo de esta nota. Todo crece en proporcion
//!   (titulos, sangrias, aires), ver `a_escala`.
//! - **El desplazamiento** va por pixeles y suave (la rueda del `RichEdit`
//!   salta de tres en tres renglones, y un renglon con una foto alta es un
//!   salto de una pantalla): `EM_SETSCROLLPOS` con una animacion corta, la
//!   barra fina propia (oscura o clara, la nativa no sigue el tema) y lo de
//!   encima (fotos, panel de comentarios) moviendose a la vez.

use std::path::PathBuf;

use pixpin_docs::md_edicion;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetFocus, ReleaseCapture, SetCapture, VK_BACK, VK_DELETE, VK_SHIFT};

use super::*;
use crate::barra_flotante::{self, BotonBarra};
use crate::vista::{self, Vista};

// Los comandos de aqui (los de las tablas van del 200 al 245).
pub(super) const C_QUITAR_FORMATO: u16 = 300;
pub(super) const C_TEXTO_NORMAL: u16 = 301;
const C_MENU_LETRA: u16 = 302;
const C_MENU_LETRA_TITULOS: u16 = 303;
const C_MENU_TAMANO: u16 = 304;
const C_SOLO_ESTA_NOTA: u16 = 305;
pub(super) const C_LETRA_MAS: u16 = 306;
pub(super) const C_LETRA_MENOS: u16 = 307;
const C_EMOJI: u16 = 320;
const C_LETRA: u16 = 340;
const C_LETRA_TITULOS: u16 = 350;
const C_TAMANO: u16 = 360;

pub(super) fn es_suyo(c: u16) -> bool {
    (C_QUITAR_FORMATO..C_TAMANO + vista::TAMANOS.len() as u16).contains(&c)
}

/// El temporizador del desplazamiento suave (en el marco) y su paso.
pub(super) const T_DESLIZAR: usize = 7;
const MS_DESLIZAR: u32 = 15;

/// Los emojis del selector: los de reaccionar y marcar de siempre.
const EMOJIS: [&str; 16] = [
    "😀", "😂", "😍", "🙂", "👍", "👎", "🙏", "👏", "🎉", "✅", "❌", "⚠️", "❤️", "🔥", "⭐", "💡",
];

/// Lo que el editor recuerda para editar sin marcas.
#[derive(Default)]
pub(super) struct Vivo {
    /// La letra y el tamano con que se ve la nota.
    pub vista: Vista,
    /// La vista es la de esta nota (no la general).
    pub de_la_nota: bool,
    ajustes: Option<PathBuf>,
    /// Un formato pedido sin nada elegido, y donde estaba el cursor: lo
    /// lleva lo proximo que se escriba ahi.
    pub pendiente: Option<(&'static str, usize)>,
    /// La barra flotante (se crea la primera vez que hace falta).
    barra: Option<HWND>,
    /// Donde se ve, respecto al marco (para las muestras).
    pub barra_caja: Option<Caja>,
    /// El cuadro del enlace y lo elegido al abrirlo.
    pub enlace: Option<(HWND, usize, usize)>,
    /// El desplazamiento suave: a donde va.
    objetivo: Option<i32>,
    /// Arrastrando el asidero de la barra: a que distancia de su borde de
    /// arriba se cogio.
    agarre: Option<i32>,
    /// El zoom de la pagina, en milesimas (0 es sin zoom): el pellizco y
    /// Ctrl+rueda, como en el navegador (ver [`acercar`]). No se guarda.
    pub zoom: i32,
}

impl Vivo {
    /// La vista guardada para esta nota (o la general).
    pub fn nuevo(integracion: &crate::integracion::Integracion) -> Vivo {
        let ajustes = integracion.ajustes_vista.clone();
        let clave = integracion.clave_vista.as_ref().map(|c| c());
        let contenido = ajustes.as_ref().and_then(|r| std::fs::read_to_string(r).ok()).unwrap_or_default();
        let (vista, de_la_nota) = vista::leer(&contenido, clave.as_deref());
        Vivo {
            vista,
            de_la_nota,
            ajustes,
            ..Default::default()
        }
    }
}

/// El tamano del cuerpo en veinteavos de punto (lo que mide el `RichEdit`):
/// 16 px a 96 ppp son 12 puntos.
pub(super) fn tamano_de(v: &Vista) -> i32 {
    v.px as i32 * 15
}

// ---------------------------------------------------------------------------
// El formato de lo que no son letras

/// Lo alto de un renglon del cuerpo, en pixeles de la pantalla: el de la
/// letra elegida a su tamano, con el interlineado de 1,4.
fn renglon_px(e: &Estado) -> i32 {
    let px = e.estilos.tamano * e.ppp / 1440;
    let letra = crate::pintor::letra_gdi(&e.estilos.letras.cuerpo, px.max(1), false);
    // SAFETY: DC de la pantalla y letra propia, pedidos y soltados aqui.
    let alto = unsafe {
        let dc = GetDC(None);
        let vieja = SelectObject(dc, HGDIOBJ(letra.0));
        let mut tm = TEXTMETRICW::default();
        let _ = GetTextMetricsW(dc, &mut tm);
        SelectObject(dc, vieja);
        ReleaseDC(None, dc);
        let _ = DeleteObject(HGDIOBJ(letra.0));
        tm.tmHeight
    };
    (if alto > 0 { alto } else { px * 6 / 5 }) * 14 / 10
}

/// Lo que en Claude no son letras: el titulo entero (tambien sus marcas
/// escondidas y el fin de parrafo, que dan el alto del cursor en un titulo
/// aun vacio), los numeros de Windows en las listas numeradas, la sangria
/// de las listas anidadas y lo que se pinta encima (`imagenes`).
pub(super) fn pintar_bloques(
    e: &mut Estado,
    texto: &str,
    ls: &[md_vivo::Linea],
    tramos: &[md_vivo::Tramo],
    entra: &dyn Fn(usize) -> bool,
    todo: bool,
) {
    let edit = e.edit;
    let u: Vec<u16> = texto.encode_utf16().collect();
    let total = u.len();
    let tipos = md_edicion::renglones(texto);
    let sangria = a_escala(&e.estilos, SANGRIA);
    let nivel = |l: &md_vivo::Linea| -> i32 {
        let blancos = u[l.desde..l.hasta].iter().take_while(|c| **c == b' ' as u16 || **c == b'\t' as u16);
        (blancos.map(|c| if *c == b'\t' as u16 { 2 } else { 1 }).sum::<i32>() / 2).min(6)
    };
    let mut racha: Option<(usize, i32, u16)> = None;
    for (n, l) in ls.iter().enumerate() {
        let Some(md_edicion::Renglon::Texto(m)) = tipos.get(n).copied() else {
            racha = None;
            continue;
        };
        if m == 0 {
            racha = None;
            continue;
        }
        let primera = u[l.desde..l.hasta].iter().find(|c| **c != b' ' as u16 && **c != b'\t' as u16).copied();
        // Titulo: el renglon entero, con su fin de parrafo.
        if u[l.desde] == b'#' as u16 && entra(n) {
            let almohadillas = u[l.desde..l.hasta].iter().take_while(|c| **c == b'#' as u16).count() as u8;
            let f = formato_de(Estilo::Titulo(almohadillas), false, &e.estilos);
            elegir(edit, l.desde, (l.hasta + 1).min(total.max(l.hasta)));
            poner_formato(edit, &f);
            if let Some(p) = parrafo_de(Estilo::Titulo(almohadillas), false, &e.estilos) {
                poner_parrafo(edit, &p);
            }
        }
        // Lista numerada: el numero lo pone Windows, contando desde el del
        // primer renglon de la racha (como lo pinta un lector de Markdown).
        let k = nivel(l);
        if let Some(t) = tramos.iter().find(|t| t.linea == n && t.estilo == Estilo::Numero) {
            let cifras = String::from_utf16_lossy(&u[t.desde..t.hasta.saturating_sub(1)]);
            let numero: u16 = cifras.parse().unwrap_or(1).clamp(0, 9999);
            let cierre = u.get(t.hasta.saturating_sub(1)).copied();
            let empieza = match racha {
                Some((previo, nv, inicio)) if previo + 1 == n && nv == k => inicio,
                _ => numero,
            };
            racha = Some((n, k, empieza));
            if entra(n) {
                let mut p = parrafo_base(&e.estilos);
                p.Base.wNumbering = PARAFORMAT_NUMBERING(2);
                p.wNumberingStart = empieza;
                p.wNumberingStyle = PARAFORMAT_NUMBERING_STYLE(if cierre == Some(b')' as u16) { 0 } else { 0x200 });
                // El numero en el primer cuarto de la sangria y el texto donde el
                // de las vinetas (`wNumberingTab` es el aire minimo tras el numero).
                p.wNumberingTab = (sangria / 6).clamp(0, u16::MAX as i32) as u16;
                p.Base.dxStartIndent = e.estilos.margen + sangria / 4 + sangria * k;
                p.Base.dxOffset = sangria * 3 / 4;
                elegir(edit, l.desde, l.hasta);
                poner_parrafo(edit, &p);
            }
            continue;
        }
        racha = None;
        // Vinetas y casillas anidadas: una sangria mas por nivel.
        if k > 0 && matches!(primera, Some(c) if c == b'-' as u16 || c == b'*' as u16 || c == b'+' as u16) && entra(n) {
            let estilo = if tramos.iter().any(|t| t.linea == n && matches!(t.estilo, Estilo::Casilla { .. })) {
                Estilo::Casilla { hecha: false }
            } else {
                Estilo::Vineta
            };
            if let Some(mut p) = parrafo_de(estilo, false, &e.estilos) {
                p.Base.dxStartIndent += sangria * k;
                elegir(edit, l.desde, l.hasta);
                poner_parrafo(edit, &p);
            }
        }
    }
    if todo {
        let ad = crate::imagenes::Adornos {
            pintor: e.pintor.clone(),
            tema: e.estilos.tema,
            margen_px: e.estilos.margen * e.ppp / 1440,
            renglon_px: renglon_px(e),
            sangria_px: sangria * e.ppp / 1440,
        };
        imagenes::ADORNOS.with(|a| *a.borrow_mut() = Some(ad));
    }
}

// ---------------------------------------------------------------------------
// El teclado

/// Cambia el texto por `nuevo` como un paso de deshacer, deja el cursor en
/// `cursor` y le pone ya el formato a los renglones tocados: que una marca
/// recien puesta no se vea ni un instante.
fn cambiar(e: &mut Estado, viejo: &str, nuevo: &str, a: usize, b: usize) {
    // Congelado: el control no pinta hasta que el formato esta puesto (ver
    // `congelar`); los renglones tocados se pintan dentro.
    congelar::congelado(e, congelar::Pintado::Tocado, |e| {
        aplicar_cambio(e.edit, viejo, nuevo);
        elegir(e.edit, a, b);
    });
}

/// Si lo que escriba el control en `pos` saldria escondido: detras de una
/// marca escondida (`# `, el `**` de cierre) hereda su formato.
fn hereda_escondido(e: &Estado, pos: usize) -> bool {
    if pos == 0 {
        return false;
    }
    let Some(d) = &e.doc else { return false };
    // SAFETY: rango del documento vivo del control.
    unsafe {
        d.Range(pos as i32 - 1, pos as i32).and_then(|r| r.GetFont()).and_then(|f| f.GetHidden()).unwrap_or(0) != 0
    }
}

/// **Una letra escrita que cambia lo escondido** (cierra una negrita, abre
/// un titulo o una lista, convierte `[] ` en casilla), o que caeria detras
/// de una marca escondida, o en el renglon de una foto: la mete el editor,
/// congelado y con el formato ya puesto. `true` si la metio.
fn letra_con_formato(e: &mut Estado, letra: char) -> bool {
    let (a, b) = seleccion(e.edit);
    if a != b || celda_del_cursor(e).is_some() {
        return false;
    }
    let texto = leer(e.edit);
    let u: Vec<u16> = texto.encode_utf16().collect();
    let ls = md_vivo::lineas(&texto);
    let n = md_vivo::linea_de(&ls, a);
    let l = ls[n];
    let mut buf = [0u16; 2];
    let puesta: Vec<u16> = letra.encode_utf16(&mut buf).to_vec();
    // En el renglon de una foto o de una raya no se escribe dentro (lo
    // romperia): la letra abre un renglon debajo (o encima, al principio).
    if matches!(md_edicion::renglones(&texto).get(n), Some(md_edicion::Renglon::Bloque)) {
        let (pos, v) = if a <= l.desde && l.hasta > l.desde {
            (l.desde, [puesta.clone(), vec![b'\n' as u16]].concat())
        } else {
            (l.hasta, [vec![b'\n' as u16], puesta.clone()].concat())
        };
        let mut nuevo = u[..pos].to_vec();
        nuevo.extend_from_slice(&v);
        nuevo.extend_from_slice(&u[pos..]);
        let cursor = if pos == l.desde && a <= l.desde { pos + puesta.len() } else { pos + v.len() };
        cambiar(e, &texto, &String::from_utf16_lossy(&nuevo), cursor, cursor);
        return true;
    }
    let mut nuevo = u[..a].to_vec();
    nuevo.extend_from_slice(&puesta);
    nuevo.extend_from_slice(&u[a..]);
    let nuevo = String::from_utf16_lossy(&nuevo);
    let cursor = a + puesta.len();
    // La casilla al vuelo, en el mismo paso que el espacio.
    if letra == ' '
        && let Some((t, c)) = md_edicion::convertir(&nuevo, cursor)
    {
        cambiar(e, &texto, &t, c, c);
        return true;
    }
    if md_edicion::letra_cambia_lo_escondido(&texto, a, letra) || hereda_escondido(e, a) {
        cambiar(e, &texto, &nuevo, cursor, cursor);
        return true;
    }
    false
}

/// **Pega texto** (Markdown o no) en el cursor: lo elegido se va como con
/// Supr, y lo pegado entra como un paso de deshacer y con sus marcas ya
/// escondidas (el pegar del control lo ensenaba tal cual hasta el formato).
pub(super) fn pegar_texto(e: &mut Estado, pegado: &str) {
    let pegado = pegado.replace("\r\n", "\n").replace('\r', "\n");
    if pegado.is_empty() {
        return;
    }
    congelar::congelado(e, congelar::Pintado::Entero, |e| {
        let texto = leer(e.edit);
        let (a, b) = seleccion(e.edit);
        let (base, c) = if a != b { md_edicion::borrar(&texto, a, b) } else { (texto.clone(), a) };
        let u: Vec<u16> = base.encode_utf16().collect();
        let c = c.min(u.len());
        let mut v = u[..c].to_vec();
        v.extend(pegado.encode_utf16());
        v.extend_from_slice(&u[c..]);
        let fin = c + pegado.encode_utf16().count();
        aplicar_cambio(e.edit, &texto, &String::from_utf16_lossy(&v));
        elegir(e.edit, fin, fin);
    });
}

fn pulsada_sola(vk: u16) -> bool {
    pulsada(vk)
}

/// **Lo que se atiende antes que el control**: Retroceso, Supr, Ctrl+X,
/// escribir sobre lo elegido y lo que se escribe con un formato pendiente.
/// `true` si ya no hay que darselo.
pub(super) fn tecla(e: &mut Estado, m: &MSG) -> bool {
    match m.message {
        WM_KEYDOWN => {
            let k = m.wParam.0 as u16;
            let (ctrl, mayus, alt) = (pulsada(VK_CONTROL.0), pulsada_sola(VK_SHIFT.0), pulsada(VK_MENU.0));
            if (k == VK_BACK.0 || k == VK_DELETE.0) && !alt && !ctrl {
                if celda_del_cursor(e).is_some() {
                    return false;
                }
                let texto = leer(e.edit);
                let (a, b) = seleccion(e.edit);
                let hecho = if a != b {
                    Some(md_edicion::borrar(&texto, a, b))
                } else {
                    md_edicion::tecla_borrar(&texto, a, k == VK_BACK.0)
                };
                return match hecho {
                    Some((t, c)) => {
                        e.vivo.pendiente = None;
                        if t != texto {
                            cambiar(e, &texto, &t, c, c);
                        } else {
                            elegir(e.edit, c, c);
                        }
                        true
                    }
                    None => false,
                };
            }
            if k == b'X' as u16 && ctrl && !mayus && !alt {
                cortar(e);
                return true;
            }
            false
        }
        WM_CHAR => {
            let c = m.wParam.0 as u32;
            let Some(letra) = char::from_u32(c).filter(|c| !c.is_control()) else {
                return false;
            };
            let (a, b) = seleccion(e.edit);
            if a != b && celda_del_cursor(e).is_none() {
                // Lo elegido se va como con Supr (sin romper marcas) y la
                // letra la escribe el control donde quedo el cursor.
                let texto = leer(e.edit);
                let (t, c) = md_edicion::borrar(&texto, a, b);
                cambiar(e, &texto, &t, c, c);
                e.vivo.pendiente = None;
                return letra_con_formato(e, letra);
            }
            if let Some((marca, donde)) = e.vivo.pendiente.take()
                && donde == a
                && a == b
            {
                let texto = leer(e.edit);
                let u: Vec<u16> = texto.encode_utf16().collect();
                let puesto = format!("{marca}{letra}{marca}");
                let mut v = u[..a].to_vec();
                v.extend(puesto.encode_utf16());
                v.extend_from_slice(&u[a..]);
                let nuevo = String::from_utf16_lossy(&v);
                let cursor = a + marca.len() + letra.len_utf16();
                cambiar(e, &texto, &nuevo, cursor, cursor);
                return true;
            }
            letra_con_formato(e, letra)
        }
        _ => false,
    }
}

/// **Lo que se mira despues de que el control atendio el mensaje**: el
/// cursor fuera de las marcas, la conversion al vuelo de `[ ] ` y la barra
/// flotante (sale al elegir, se va al escribir).
pub(super) fn despues(e: &mut Estado, m: &MSG) {
    // El cuadro del enlace se cierra si se va el foco (clic en la nota).
    if let Some((h, ..)) = e.vivo.enlace {
        // SAFETY: consulta del foco de este hilo.
        let foco = unsafe { GetFocus() };
        if foco != h && m.hwnd != h {
            acabar_enlace(e, false);
        }
    }
    if m.hwnd != e.edit {
        return;
    }
    match m.message {
        WM_KEYDOWN | WM_LBUTTONUP | WM_LBUTTONDOWN => {
            let (a, b) = seleccion(e.edit);
            if a == b && e.titulo_edit.is_none() && m.message != WM_LBUTTONDOWN {
                let texto = leer(e.edit);
                let c = md_edicion::cursor_valido(&texto, a);
                if c != a {
                    elegir(e.edit, c, c);
                }
            }
            if m.message == WM_KEYDOWN && !matches!(m.wParam.0 as u16, k if k == VK_SHIFT.0 || k == VK_CONTROL.0) {
                if !pulsada(VK_SHIFT.0) {
                    esconder_barra(e);
                }
            } else if m.message == WM_LBUTTONDOWN {
                esconder_barra(e);
            }
        }
        WM_CHAR => {
            esconder_barra(e);
            if m.wParam.0 == b' ' as usize {
                let texto = leer(e.edit);
                let (a, b) = seleccion(e.edit);
                if a == b
                    && let Some((t, c)) = md_edicion::convertir(&texto, a)
                {
                    cambiar(e, &texto, &t, c, c);
                }
            }
        }
        _ => {}
    }
    if matches!(m.message, WM_KEYUP | WM_LBUTTONUP) {
        actualizar_barra_flotante(e);
    }
}

/// Cortar sin dejar marcas sueltas: se copia lo elegido y se borra lo que
/// se ve de ello.
pub(super) fn cortar(e: &mut Estado) {
    let (a, b) = seleccion(e.edit);
    if a == b {
        return;
    }
    enviar(e.edit, WM_COPY, 0, 0);
    if celda_del_cursor(e).is_some() {
        enviar(e.edit, WM_CLEAR, 0, 0);
        return;
    }
    let texto = leer(e.edit);
    let (t, c) = md_edicion::borrar(&texto, a, b);
    cambiar(e, &texto, &t, c, c);
}

/// Antes de pegar encima de algo elegido: se quita como con Supr.
pub(super) fn antes_de_pegar(e: &mut Estado) {
    let (a, b) = seleccion(e.edit);
    if a != b && celda_del_cursor(e).is_none() {
        let texto = leer(e.edit);
        let (t, c) = md_edicion::borrar(&texto, a, b);
        cambiar(e, &texto, &t, c, c);
    }
}

// ---------------------------------------------------------------------------
// El formato desde los botones y las teclas

/// B, I, S, codigo y formula: con las marcas escondidas. Sin nada elegido
/// y fuera de un tramo con ese formato, queda pendiente para lo que se
/// escriba (como en Word o en Claude).
pub(super) fn formato(e: &mut Estado, marca: &'static str) {
    let texto = leer(e.edit);
    let (a, b) = seleccion(e.edit);
    match md_edicion::alternar(&texto, a, b, marca) {
        Some((t, x, y)) => {
            e.vivo.pendiente = None;
            cambiar(e, &texto, &t, x, y);
        }
        None if a == b => {
            e.vivo.pendiente = match e.vivo.pendiente {
                Some((m, p)) if m == marca && p == a => None,
                _ => Some((marca, a)),
            };
        }
        // Dentro de una celda de tabla: las marcas de siempre.
        None if celda_del_cursor(e).is_some() => envolver(e, marca),
        None => {}
    }
    actualizar_barra_flotante(e);
}

pub(super) fn quitar_formato(e: &mut Estado) {
    let texto = leer(e.edit);
    let (a, b) = seleccion(e.edit);
    if let Some((t, x, y)) = md_edicion::quitar_formato(&texto, a, b) {
        cambiar(e, &texto, &t, x, y);
    }
    e.vivo.pendiente = None;
    actualizar_barra_flotante(e);
}

/// Pone la marca de bloque `prefijo` (`"# "`, `"- "`, `"1. "`, `"- [ ] "`,
/// `"> "`, o `""` para texto normal) a todos los renglones elegidos; si ya
/// la tenian todos, se la quita. Una lista numerada se numera seguida.
pub(super) fn bloque(e: &mut Estado, prefijo: &str) {
    let texto = leer(e.edit);
    let (a, b) = seleccion(e.edit);
    let ls = md_vivo::lineas(&texto);
    let n0 = md_vivo::linea_de(&ls, a);
    let mut n1 = md_vivo::linea_de(&ls, b);
    if b > a && n1 > n0 && b == ls[n1].desde {
        n1 -= 1;
    }
    let tipos = md_edicion::renglones(&texto);
    let renglones: Vec<usize> = (n0..=n1).filter(|n| matches!(tipos.get(*n), Some(md_edicion::Renglon::Texto(_)))).collect();
    if renglones.is_empty() {
        return;
    }
    let numerada = prefijo == "1. ";
    let quiere = |i: usize| if numerada { format!("{}. ", i + 1) } else { prefijo.to_string() };
    let todos = !prefijo.is_empty()
        && renglones.iter().enumerate().all(|(i, n)| {
            let puesto = md_edicion::bloque_de(&texto, *n);
            if numerada { puesto.trim_start().chars().next().is_some_and(|c| c.is_ascii_digit()) } else { puesto == quiere(i) }
        });
    let mut t = texto.clone();
    for (i, n) in renglones.iter().enumerate().rev() {
        let puesto = md_edicion::bloque_de(&t, *n);
        let p = if todos { puesto.clone() } else { quiere(i) };
        if !todos && puesto == p {
            continue;
        }
        // `con_prefijo` quita la marca si es la misma (todos) y si no la pone.
        if let Some((nuevo, _)) = md_vivo::con_prefijo(&t, *n, if todos { &puesto } else { &p }) {
            t = nuevo;
        }
    }
    if t == texto {
        return;
    }
    // El cursor, a la misma distancia del final de su renglon.
    let nls = md_vivo::lineas(&t);
    let llevar = |p: usize| -> usize {
        let n = md_vivo::linea_de(&ls, p);
        let desde_el_final = ls[n].hasta.saturating_sub(p);
        let l = nls[n.min(nls.len() - 1)];
        let x = l.hasta.saturating_sub(desde_el_final).max(l.desde);
        md_edicion::cursor_valido(&t, x)
    };
    let (x, y) = (llevar(a), llevar(b));
    cambiar(e, &texto, &t, x, y);
}

// ---------------------------------------------------------------------------
// La barra flotante

fn ventana_barra(e: &mut Estado) -> Option<HWND> {
    if e.vivo.barra.is_none() {
        e.vivo.barra = barra_flotante::crear(e.marco).ok();
    }
    e.vivo.barra
}

/// Los formatos que ya tiene lo elegido (sus botones salen pulsados).
fn puestos(texto: &str, a: usize, b: usize) -> Vec<BotonBarra> {
    let marcas = md_edicion::marcas(texto);
    let u: Vec<u16> = texto.encode_utf16().collect();
    let (mut a, mut b) = (a.min(u.len()), b.min(u.len()));
    while a < b && (marcas[a] || u[a] == b' ' as u16) {
        a += 1;
    }
    while b > a && (marcas[b - 1] || u[b - 1] == b' ' as u16) {
        b -= 1;
    }
    if a >= b {
        return Vec::new();
    }
    md_edicion::envueltos(texto)
        .into_iter()
        .filter(|x| x.dentro.0 <= a && b <= x.dentro.1)
        .filter_map(|x| match x.estilo {
            Estilo::Negrita => Some(BotonBarra::Negrita),
            Estilo::Cursiva => Some(BotonBarra::Cursiva),
            Estilo::Tachado => Some(BotonBarra::Tachado),
            Estilo::Codigo => Some(BotonBarra::Codigo),
            Estilo::Enlace => Some(BotonBarra::Enlace),
            _ => None,
        })
        .collect()
}

/// Ensena la barra encima de lo elegido, o la esconde si no hay nada.
pub(super) fn actualizar_barra_flotante(e: &mut Estado) {
    let (a, b) = seleccion(e.edit);
    if a == b || e.vivo.enlace.is_some() || e.titulo_edit.is_some() {
        esconder_barra(e);
        return;
    }
    let texto = leer(e.edit);
    let disp = barra_flotante::disponer(e.pintor.escala);
    let mut pa = POINT::default();
    let mut pb = POINT::default();
    enviar(e.edit, EM_POSFROMCHAR, &mut pa as *mut _ as usize, a as isize);
    enviar(e.edit, EM_POSFROMCHAR, &mut pb as *mut _ as usize, b as isize);
    let mut cliente = RECT::default();
    // SAFETY: rectangulo y coordenadas de ventanas propias.
    let (pa, abajo, zona) = unsafe {
        let _ = GetClientRect(e.edit, &mut cliente);
        let mut esquina = POINT { x: cliente.left, y: cliente.top };
        let _ = ClientToScreen(e.edit, &mut esquina);
        let _ = ClientToScreen(e.edit, &mut pa);
        let _ = ClientToScreen(e.edit, &mut pb);
        (pa, pb.y + renglon_px(e), (esquina.x, esquina.y, cliente.right - cliente.left, cliente.bottom - cliente.top))
    };
    let (x, y) = barra_flotante::colocar((pa.x, pa.y), abajo, (disp.an, disp.al), zona, 8 * ppp_ui() / 96);
    let (an, al) = (disp.an, disp.al);
    let hechos = puestos(&texto, a, b);
    barra_flotante::VISTA.with(|v| {
        *v.borrow_mut() = Some(barra_flotante::Vista {
            disp,
            tema: e.estilos.tema,
            pintor: e.pintor.clone(),
            hover: None,
            puestos: hechos,
        })
    });
    let mut dentro = POINT { x, y };
    // SAFETY: conversion de coordenadas de una ventana propia.
    unsafe {
        let _ = ScreenToClient(e.marco, &mut dentro);
    }
    e.vivo.barra_caja = Some(Caja { x: dentro.x, y: dentro.y, an, al });
    if !e.oculto
        && let Some(h) = ventana_barra(e)
    {
        barra_flotante::ensenar(h, x, y, an, al);
    }
}

pub(super) fn esconder_barra(e: &mut Estado) {
    if e.vivo.barra_caja.take().is_some()
        && let Some(h) = e.vivo.barra
    {
        barra_flotante::esconder(h);
    }
}

/// Donde esta un boton de la barra, en la pantalla (para colgarle su menu).
fn ancla_de(e: &Estado, b: BotonBarra) -> Option<(POINT, i32)> {
    let caja = e.vivo.barra_caja?;
    let c = barra_flotante::VISTA.with(|v| v.borrow().as_ref().and_then(|v| v.disp.caja(b)))?;
    let mut p = POINT { x: caja.x + c.x, y: caja.y + c.y };
    // SAFETY: conversion de coordenadas de una ventana propia.
    unsafe {
        let _ = ClientToScreen(e.marco, &mut p);
    }
    Some((p, c.al + 6 * ppp_ui() / 96))
}

/// Un boton de la barra flotante.
pub(super) fn clic_barra(e: &mut Estado, b: BotonBarra, guardar: &mut dyn FnMut(&str) -> bool) {
    let habia = e.abierto;
    cerrar_menu(e);
    let r = e.rotulos.clone();
    let menu_de = |entradas: Vec<Entrada>, e: &mut Estado| {
        if let Some((p, alto)) = ancla_de(e, b) {
            abrir_menu(e, Abierto::Vivo, entradas, None, p, alto);
        }
    };
    match b {
        BotonBarra::Comentar => super::comando(e, C_COMENTAR, guardar),
        BotonBarra::Negrita => super::comando(e, C_NEGRITA, guardar),
        BotonBarra::Cursiva => super::comando(e, C_CURSIVA, guardar),
        BotonBarra::Tachado => super::comando(e, C_TACHADO, guardar),
        BotonBarra::Codigo => super::comando(e, C_CODIGO, guardar),
        BotonBarra::QuitarFormato => super::comando(e, C_QUITAR_FORMATO, guardar),
        BotonBarra::Enlace => super::comando(e, C_ENLACE, guardar),
        _ if habia == Some(Abierto::Vivo) => {}
        BotonBarra::Formato => {
            let entradas = vec![
                entrada(C_TEXTO_NORMAL, Dibujo::Letras("Aa"), &r.barra.texto_normal, ""),
                entrada(C_T1, Dibujo::Letras("H1"), &r.titulo1, "Ctrl+1"),
                entrada(C_T2, Dibujo::Letras("H2"), &r.titulo2, "Ctrl+2"),
                entrada(C_T3, Dibujo::Letras("H3"), &r.titulo3, "Ctrl+3"),
            ];
            menu_de(entradas, e);
        }
        BotonBarra::Listas => {
            let m = &r.mayus;
            let entradas = vec![
                entrada(C_LISTA, Dibujo::Icono(Icono::Lista), &r.lista, &format!("Ctrl+{m}+8")),
                entrada(C_NUMERADA, Dibujo::Icono(Icono::Numerada), &r.numerada, &format!("Ctrl+{m}+7")),
                entrada(C_CASILLA, Dibujo::Icono(Icono::Casillas), &r.casilla, &format!("Ctrl+{m}+9")),
            ];
            menu_de(entradas, e);
        }
        BotonBarra::Emoji => {
            let nombres: Vec<&str> = r.barra.emojis.split('|').collect();
            let entradas = EMOJIS
                .iter()
                .enumerate()
                .map(|(i, x)| entrada(C_EMOJI + i as u16, Dibujo::Letras(x), nombres.get(i).copied().unwrap_or(""), ""))
                .collect();
            menu_de(entradas, e);
        }
    }
}

/// Lo que se pinta de la barra en una muestra (la ventana es aparte).
pub(super) fn pintar_en_muestra(e: &Estado, dc: HDC) {
    if let Some(c) = e.vivo.barra_caja {
        barra_flotante::VISTA.with(|v| {
            if let Some(v) = v.borrow().as_ref() {
                barra_flotante::pintar(dc, v, (c.x, c.y));
            }
        });
    }
}

// ---------------------------------------------------------------------------
// El enlace: un cuadro donde pegar o escribir la direccion

pub(super) fn empezar_enlace(e: &mut Estado) {
    acabar_enlace(e, false);
    let (a, b) = seleccion(e.edit);
    let caja = e.vivo.barra_caja.unwrap_or_else(|| {
        let mut p = POINT::default();
        enviar(e.edit, EM_POSFROMCHAR, &mut p as *mut _ as usize, a as isize);
        let mut q = p;
        // SAFETY: conversion de coordenadas de ventanas propias.
        unsafe {
            let _ = ClientToScreen(e.edit, &mut q);
            let _ = ScreenToClient(e.marco, &mut q);
        }
        Caja { x: q.x, y: q.y - 40 * ppp_ui() / 96, an: 0, al: 34 * ppp_ui() / 96 }
    });
    esconder_barra(e);
    let ancho = 360 * ppp_ui() / 96;
    let alto = 28 * ppp_ui() / 96;
    // SAFETY: control hijo propio; se destruye en `acabar_enlace`.
    unsafe {
        let Ok(h) = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            &HSTRING::from(url_del_portapapeles().as_str()),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            caja.x.max(0),
            caja.y.max(0) + (caja.al - alto).max(0) / 2,
            ancho,
            alto,
            Some(e.marco),
            None,
            None,
            None,
        ) else {
            return;
        };
        SendMessageW(h, WM_SETFONT, Some(WPARAM(e.pintor.letra.0 as usize)), Some(LPARAM(1)));
        let pista: Vec<u16> = e.rotulos.barra.enlace_pista.encode_utf16().chain(std::iter::once(0)).collect();
        // EM_SETCUEBANNER: la pista gris mientras esta vacio.
        SendMessageW(h, 0x1501, Some(WPARAM(1)), Some(LPARAM(pista.as_ptr() as isize)));
        SendMessageW(h, 0x00B1, Some(WPARAM(0)), Some(LPARAM(-1)));
        let _ = SetFocus(Some(h));
        e.vivo.enlace = Some((h, a, b));
    }
}

/// Si el mensaje es del cuadro del enlace: Intro lo pone, Esc lo deja.
pub(super) fn tecla_del_enlace(e: &mut Estado, m: &MSG) -> bool {
    let Some((h, ..)) = e.vivo.enlace else { return false };
    if m.hwnd != h || m.message != WM_KEYDOWN {
        return false;
    }
    match m.wParam.0 as u16 {
        k if k == VK_RETURN.0 => acabar_enlace(e, true),
        k if k == VK_ESCAPE.0 => acabar_enlace(e, false),
        _ => return false,
    }
    true
}

pub(super) fn es_del_enlace(e: &Estado, h: HWND) -> bool {
    e.vivo.enlace.is_some_and(|(x, ..)| x == h)
}

/// Una direccion escrita sin `https://` (`www.x.es`) se completa: un
/// enlace sin esquema el movil lo abriria como fichero.
pub(super) fn completar_direccion(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() || s.contains("://") || s.to_ascii_lowercase().starts_with("mailto:") || s.starts_with("pixpin:") {
        s.to_string()
    } else if s.contains('@') && !s.contains('/') {
        format!("mailto:{s}")
    } else {
        format!("https://{s}")
    }
}

/// Pone el enlace `url` en lo que estaba elegido (`a..b`); sin nada
/// elegido, la direccion hace de texto.
pub(super) fn poner_enlace(e: &mut Estado, a: usize, b: usize, url: &str) {
    let url = completar_direccion(url);
    if url.is_empty() {
        return;
    }
    let texto = leer(e.edit);
    let (t, fin) = if a == b {
        let u: Vec<u16> = texto.encode_utf16().collect();
        let puesto = format!("[{url}]({url})");
        let mut v = u[..a.min(u.len())].to_vec();
        v.extend(puesto.encode_utf16());
        v.extend_from_slice(&u[a.min(u.len())..]);
        (String::from_utf16_lossy(&v), a + puesto.encode_utf16().count())
    } else {
        let (t, x, _) = md_vivo::poner_enlace(&texto, a, b, &url);
        (t, x)
    };
    cambiar(e, &texto, &t, fin, fin);
}

pub(super) fn acabar_enlace(e: &mut Estado, poner: bool) {
    let Some((h, a, b)) = e.vivo.enlace.take() else { return };
    // SAFETY: control propio; el bufer es local.
    let url = unsafe {
        let n = GetWindowTextLengthW(h).max(0) as usize;
        let mut buf = vec![0u16; n + 1];
        let copiados = GetWindowTextW(h, &mut buf).max(0) as usize;
        let _ = DestroyWindow(h);
        String::from_utf16_lossy(&buf[..copiados])
    };
    if poner {
        elegir(e.edit, a, b);
        poner_enlace(e, a, b, &url);
    }
    // SAFETY: foco a un hijo propio.
    unsafe {
        let _ = SetFocus(Some(e.edit));
    }
}

// ---------------------------------------------------------------------------
// La letra y el tamano (preferencia de vista)

/// Lo de la letra en el menu de la flecha del titulo.
pub(super) fn entradas_de_vista(e: &Estado) -> Vec<Entrada> {
    let r = &e.rotulos.barra;
    let v = &e.vivo.vista;
    vec![
        entrada(C_MENU_LETRA, Dibujo::Letras("Aa"), &r.letra_texto, &v.cuerpo),
        entrada(C_MENU_LETRA_TITULOS, Dibujo::Letras("H1"), &r.letra_titulos, &v.titulos),
        entrada(C_MENU_TAMANO, Dibujo::Letras("A+"), &r.tamano, &format!("{} px", v.px)),
    ]
}

fn marca_si(si: bool) -> &'static str {
    if si { "\u{2713}" } else { "" }
}

fn menu_de_letras(e: &mut Estado, titulos: bool) {
    let actual = if titulos { e.vivo.vista.titulos.clone() } else { e.vivo.vista.cuerpo.clone() };
    let base = if titulos { C_LETRA_TITULOS } else { C_LETRA };
    let entradas = vista::LETRAS
        .iter()
        .enumerate()
        .map(|(i, f)| entrada(base + i as u16, Dibujo::Letras("Aa"), f, marca_si(*f == actual)))
        .collect();
    abrir_menu_en_boton(e, Abierto::Vivo, Boton::Titulo, entradas, None);
}

fn menu_de_tamano(e: &mut Estado) {
    let r = e.rotulos.barra.clone();
    let nombres: Vec<&str> = r.tamanos.split('|').collect();
    let mut entradas: Vec<Entrada> = vista::TAMANOS
        .iter()
        .enumerate()
        .map(|(i, px)| {
            let nombre = nombres.get(i).copied().unwrap_or("");
            let atajo = if e.vivo.vista.px == *px { format!("{px} px \u{2713}") } else { format!("{px} px") };
            entrada(C_TAMANO + i as u16, Dibujo::Letras("A"), nombre, &atajo)
        })
        .collect();
    entradas.push(entrada(C_SOLO_ESTA_NOTA, Dibujo::Icono(Icono::Lapiz), &r.solo_esta_nota, marca_si(e.vivo.vista_de_la_nota())));
    let pista = Some(format!("{} \u{b7} {} px", r.pista_tamano, e.vivo.vista.px));
    abrir_menu_en_boton(e, Abierto::Vivo, Boton::Titulo, entradas, pista);
}

impl Vivo {
    fn vista_de_la_nota(&self) -> bool {
        self.de_la_nota
    }
}

/// Pone la vista `v`: letra y tamano nuevos en todo el texto (las tablas
/// tambien), el formato otra vez y se guarda.
pub(super) fn aplicar_vista(e: &mut Estado, v: Vista) {
    e.vivo.vista = v;
    e.estilos.letras = letras::de_vista(&e.vivo.vista);
    e.estilos.tamano = tamano_de(&e.vivo.vista);
    let tema = e.estilos.tema;
    // La letra nueva va a todo el texto antes que el formato: congelado,
    // que no se vean las marcas con la letra nueva un instante.
    congelar::congelado(e, congelar::Pintado::Entero, |e| {
        preparar_edit(e.edit, e.marco, &tema, &e.estilos);
        pintar(e, None);
    });
    imagenes::repintar(e.edit);
    guardar_vista(e);
    actualizar_barra_flotante(e);
}

fn guardar_vista(e: &Estado) {
    let Some(ruta) = &e.vivo.ajustes else { return };
    let contenido = std::fs::read_to_string(ruta).unwrap_or_default();
    let clave = if e.vivo.de_la_nota { e.integracion.clave_vista.as_ref().map(|c| c()) } else { None };
    let nuevo = vista::escribir(&contenido, clave.as_deref(), &e.vivo.vista);
    if let Err(err) = std::fs::write(ruta, nuevo) {
        tracing::warn!(?err, "no se pudo guardar la letra de las notas");
    }
}

/// «Solo en esta nota»: la vista de ahora pasa a ser la de esta nota, o
/// esta nota vuelve a la general.
fn alternar_de_la_nota(e: &mut Estado) {
    let clave = e.integracion.clave_vista.as_ref().map(|c| c());
    if e.vivo.de_la_nota {
        e.vivo.de_la_nota = false;
        let contenido = e.vivo.ajustes.as_ref().and_then(|r| std::fs::read_to_string(r).ok()).unwrap_or_default();
        let contenido = match (&clave, &e.vivo.ajustes) {
            (Some(c), Some(r)) => {
                let limpio = vista::olvidar(&contenido, c);
                if let Err(err) = std::fs::write(r, &limpio) {
                    tracing::warn!(?err, "no se pudo guardar la letra de las notas");
                }
                limpio
            }
            _ => contenido,
        };
        let (general, _) = vista::leer(&contenido, None);
        aplicar_vista(e, general);
    } else if clave.is_some() {
        e.vivo.de_la_nota = true;
        guardar_vista(e);
    }
}

pub(super) fn comando(e: &mut Estado, c: u16) {
    match c {
        C_QUITAR_FORMATO => quitar_formato(e),
        C_TEXTO_NORMAL => bloque(e, ""),
        C_MENU_LETRA => menu_de_letras(e, false),
        C_MENU_LETRA_TITULOS => menu_de_letras(e, true),
        C_MENU_TAMANO => menu_de_tamano(e),
        C_SOLO_ESTA_NOTA => alternar_de_la_nota(e),
        C_LETRA_MAS | C_LETRA_MENOS => {
            let px = e.vivo.vista.otro_tamano(c == C_LETRA_MAS);
            if px != e.vivo.vista.px {
                let v = Vista { px, ..e.vivo.vista.clone() };
                aplicar_vista(e, v);
            }
        }
        c if (C_EMOJI..C_EMOJI + EMOJIS.len() as u16).contains(&c) => {
            let (_, b) = seleccion(e.edit);
            elegir(e.edit, b, b);
            let t = ancho_nulo(EMOJIS[(c - C_EMOJI) as usize]);
            enviar(e.edit, EM_REPLACESEL, 1, t.as_ptr() as isize);
        }
        c if (C_LETRA..C_LETRA + vista::LETRAS.len() as u16).contains(&c) => {
            let v = Vista { cuerpo: vista::LETRAS[(c - C_LETRA) as usize].into(), ..e.vivo.vista.clone() };
            aplicar_vista(e, v);
        }
        c if (C_LETRA_TITULOS..C_LETRA_TITULOS + vista::LETRAS.len() as u16).contains(&c) => {
            let v = Vista { titulos: vista::LETRAS[(c - C_LETRA_TITULOS) as usize].into(), ..e.vivo.vista.clone() };
            aplicar_vista(e, v);
        }
        c if (C_TAMANO..C_TAMANO + vista::TAMANOS.len() as u16).contains(&c) => {
            let v = Vista { px: vista::TAMANOS[(c - C_TAMANO) as usize], ..e.vivo.vista.clone() };
            aplicar_vista(e, v);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// El desplazamiento suave

fn desplazado(e: &Estado) -> i32 {
    let mut p = POINT::default();
    enviar(e.edit, EM_GETSCROLLPOS, 0, &mut p as *mut _ as isize);
    p.y
}

/// Los ppp de la pantalla, sin zoom: para lo que no es de la pagina (la
/// barra flotante, el cuadro del enlace).
fn ppp_ui() -> i32 {
    super::PPP.with(|p| p.get())
}

/// Lo menos y lo mas que se acerca la pagina, en milesimas.
const ZOOM_MIN: i32 = 500;
const ZOOM_MAX: i32 = 3000;

/// El zoom de la pagina en milesimas (1000 sin zoom).
pub(super) fn zoom_de(v: &Vivo) -> i32 {
    if v.zoom == 0 { 1000 } else { v.zoom }
}

/// El zoom tras `pasos` de la rueda con Ctrl (o de pellizcar): un 10 % por
/// paso entero; el panel tactil manda trocitos de paso.
pub(super) fn zoom_tras(ahora: i32, pasos: f32) -> i32 {
    ((ahora as f32 * 1.1f32.powf(pasos)).round() as i32).clamp(ZOOM_MIN, ZOOM_MAX)
}

/// **Acercar o alejar la pagina**, como en el navegador (el usuario, 1-oct:
/// «cuando hago zoom con los dedos que se pueda hacer como en el navegador
/// y no se agrande el texto; el texto solo con Ctrl mas o menos»). El
/// pellizco del panel tactil llega como Ctrl+rueda.
pub(super) fn acercar(e: &mut Estado, pasos: f32) {
    let nuevo = zoom_tras(zoom_de(&e.vivo), pasos);
    poner_zoom(e, nuevo);
}

/// **Pone el zoom de la pagina**: todo crece junto (letras, tablas, aires,
/// fotos, paginas vivas, casillas) y la columna con ellas; si no cabe, el
/// texto se reparte en lo que se ve, como en el navegador. Se hace como si
/// la pantalla tuviera mas puntos por pulgada: el control con `EM_SETZOOM`
/// y lo que se pinta encima con los ppp de la pagina (`e.ppp`), asi cada
/// cuenta de veinteavos a pixeles sigue valiendo. Lo de arriba de la vista
/// se queda arriba.
pub(super) fn poner_zoom(e: &mut Estado, z: i32) {
    let z = z.clamp(ZOOM_MIN, ZOOM_MAX);
    if z == zoom_de(&e.vivo) {
        return;
    }
    let real = ppp_ui();
    let (izq, _) = super::tablas::desplazar::visible(e.edit);
    let arriba = POINT { x: izq + 1, y: 1 };
    let letra = enviar(e.edit, EM_CHARFROMPOS, 0, &arriba as *const _ as isize).max(0) as usize;
    let mut antes = POINT::default();
    enviar(e.edit, EM_POSFROMCHAR, &mut antes as *mut _ as usize, letra as isize);
    e.vivo.zoom = if z == 1000 { 0 } else { z };
    let ppp = (real * z + 500) / 1000;
    e.ppp = ppp;
    imagenes::PPP.with(|p| p.set(ppp));
    if crate::tabla_ancha::hueco() > 0 {
        crate::tabla_ancha::poner_hueco(true, ppp);
    }
    esconder_barra(e);
    congelar::congelado(e, congelar::Pintado::Entero, |e| {
        enviar(e.edit, EM_SETZOOM, ppp as usize, real as isize);
        super::colocar_edit(e.marco);
        e.estilos.margen = super::margen_de(e);
        super::pintar(e, None);
    });
    super::tablas::desplazar::colocar_todas(e, None);
    let mut despues = POINT::default();
    enviar(e.edit, EM_POSFROMCHAR, &mut despues as *mut _ as usize, letra as isize);
    let y = (desplazado(e) + despues.y - antes.y).clamp(0, tope(e));
    e.vivo.objetivo = None;
    ir_a(e, y);
    super::comentarios::componer(e);
}

/// El control corre lo pintado y solo pinta lo que entra (rapido); lo que
/// deja de rastro lo limpia `antes_de_pintar`.
fn ir_a(e: &Estado, y: i32) {
    let p = POINT { x: 0, y: y.max(0) };
    enviar(e.edit, EM_SETSCROLLPOS, 0, &p as *const _ as isize);
}

/// Lo mas abajo que se puede ir.
fn tope(e: &Estado) -> i32 {
    let (visible, alto) = imagenes::medidas(e.edit);
    (alto - visible).max(0)
}

/// Lo que baja un golpe de rueda: los renglones que dice Windows (3 de
/// fabrica; o una pantalla), a lo alto de un renglon de la letra elegida.
pub(super) fn paso_de_rueda(e: &Estado) -> i32 {
    let mut lineas: u32 = 3;
    // SAFETY: consulta de un ajuste del sistema a una variable local.
    unsafe {
        let _ = SystemParametersInfoW(SPI_GETWHEELSCROLLLINES, 0, Some(&mut lineas as *mut _ as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
    }
    if lineas == u32::MAX {
        imagenes::medidas(e.edit).0 * 9 / 10
    } else {
        renglon_px(e) * lineas.clamp(1, 20) as i32
    }
}

/// **La rueda del raton sobre la nota**: por pixeles y suave (los renglones
/// de la rueda de Windows, a lo alto de un renglon de la letra elegida); con
/// Ctrl, el tamano de la letra (no el zoom del control, que agrandaria el
/// texto y dejaria las fotos como estaban, encima del texto). Un panel
/// tactil manda pasos pequenos: van tal cual, sin animar.
pub(super) fn rueda(e: &mut Estado, m: &MSG) -> bool {
    if m.message != WM_MOUSEWHEEL {
        return false;
    }
    let delta = ((m.wParam.0 >> 16) & 0xffff) as i16 as i32;
    let teclas = m.wParam.0 & 0xffff;
    esconder_barra(e);
    if teclas & 0x0008 != 0 {
        acercar(e, delta as f32 / 120.0);
        return true;
    }
    let mover = -delta * paso_de_rueda(e) / 120;
    let desde = e.vivo.objetivo.unwrap_or_else(|| desplazado(e));
    let objetivo = (desde + mover).clamp(0, tope(e));
    if delta.abs() < 120 || e.oculto {
        e.vivo.objetivo = None;
        ir_a(e, objetivo);
        return true;
    }
    e.vivo.objetivo = Some(objetivo);
    // SAFETY: temporizador de una ventana propia.
    unsafe {
        SetTimer(Some(e.marco), T_DESLIZAR, MS_DESLIZAR, None);
    }
    true
}

/// **Lleva la nota hasta `y`** (pixeles desde arriba de la nota)
/// deslizando como la rueda: lo usa el clic en un comentario para ir a su
/// texto. En las pruebas (ventana oculta), de golpe.
pub(super) fn deslizar_a(e: &mut Estado, y: i32) {
    let objetivo = y.clamp(0, tope(e));
    if e.oculto {
        e.vivo.objetivo = None;
        ir_a(e, objetivo);
        return;
    }
    e.vivo.objetivo = Some(objetivo);
    // SAFETY: temporizador de una ventana propia.
    unsafe {
        SetTimer(Some(e.marco), T_DESLIZAR, MS_DESLIZAR, None);
    }
}

/// Un paso de la animacion: un tercio de lo que falta (rapido al principio,
/// suave al llegar), y al llegar se para el temporizador.
pub(super) fn paso(e: &mut Estado) {
    let Some(objetivo) = e.vivo.objetivo else {
        // SAFETY: temporizador propio.
        unsafe {
            let _ = KillTimer(Some(e.marco), T_DESLIZAR);
        }
        return;
    };
    let ahora = desplazado(e);
    let falta = objetivo - ahora;
    let siguiente = if falta.abs() <= 2 { objetivo } else { ahora + falta / 3 + falta.signum() };
    ir_a(e, siguiente);
    // Si el control no se movio (el final de la nota), se acaba aqui.
    if siguiente == objetivo || desplazado(e) == ahora {
        e.vivo.objetivo = None;
        // SAFETY: temporizador propio.
        unsafe {
            let _ = KillTimer(Some(e.marco), T_DESLIZAR);
        }
    }
}

fn en_la_pista(x: i32, y: i32) -> Option<(RECT, RECT)> {
    imagenes::BARRA
        .with(|b| b.get())
        .filter(|(p, _)| x >= p.left && x < p.right && y >= p.top && y < p.bottom)
}

/// **La barra fina con el raton**: coger el asidero y arrastrarlo, o un
/// clic en la pista para ir ahi (deslizando). `true` si era de la barra.
pub(super) fn raton_barra(e: &mut Estado, m: &MSG) -> bool {
    let (x, y) = ((m.lParam.0 & 0xffff) as i16 as i32, ((m.lParam.0 >> 16) & 0xffff) as i16 as i32);
    match m.message {
        WM_LBUTTONDOWN => {
            let Some((_, asidero)) = en_la_pista(x, y) else { return false };
            esconder_barra(e);
            if y >= asidero.top && y < asidero.bottom {
                e.vivo.agarre = Some(y - asidero.top);
                // SAFETY: captura del raton para una ventana propia.
                unsafe {
                    SetCapture(e.edit);
                }
            } else {
                // Un clic en la pista: una pantalla hacia alli.
                let (visible, _) = imagenes::medidas(e.edit);
                let hacia = if y < asidero.top { -visible * 9 / 10 } else { visible * 9 / 10 };
                let objetivo = (desplazado(e) + hacia).clamp(0, tope(e));
                if e.oculto {
                    ir_a(e, objetivo);
                } else {
                    e.vivo.objetivo = Some(objetivo);
                    // SAFETY: temporizador de una ventana propia.
                    unsafe {
                        SetTimer(Some(e.marco), T_DESLIZAR, MS_DESLIZAR, None);
                    }
                }
            }
            true
        }
        WM_MOUSEMOVE => {
            if let Some(agarre) = e.vivo.agarre {
                let Some((_, asidero)) = imagenes::BARRA.with(|b| b.get()) else { return true };
                let (visible, alto) = imagenes::medidas(e.edit);
                ir_a(e, imagenes::desplazado_de(visible, alto, y - agarre, asidero.bottom - asidero.top));
                imagenes::repintar(e.edit);
                return true;
            }
            // Encima, el asidero se ensancha (como el de Claude).
            let encima = en_la_pista(x, y).is_some();
            if imagenes::BARRA_ENCIMA.with(|b| b.replace(encima)) != encima {
                imagenes::repintar(e.edit);
            }
            false
        }
        WM_LBUTTONUP if e.vivo.agarre.take().is_some() => {
            // SAFETY: suelta la captura de este hilo.
            unsafe {
                let _ = ReleaseCapture();
            }
            true
        }
        _ => false,
    }
}

/// Sobre la barra, la flecha (no el cursor de escribir).
pub(super) fn cursor_de_la_barra(edit: HWND) -> bool {
    let mut p = POINT::default();
    // SAFETY: posicion del raton y conversion para una ventana propia.
    unsafe {
        let _ = GetCursorPos(&mut p);
        let _ = ScreenToClient(edit, &mut p);
    }
    if en_la_pista(p.x, p.y).is_none() {
        return false;
    }
    // SAFETY: cursor del sistema, compartido; no se suelta.
    unsafe {
        if let Ok(c) = LoadCursorW(None, IDC_ARROW) {
            SetCursor(Some(c));
        }
    }
    true
}

thread_local! {
    /// El desplazamiento con que se pinto la ultima vez.
    static PINTADO_EN: std::cell::Cell<i32> = const { std::cell::Cell::new(i32::MIN) };
}

/// **Antes de que el control pinte**: si la nota se movio, el control ya
/// corrio lo pintado (rapido: no se repinta la nota) con lo que no es de la
/// nota dentro: el asidero de la barra y las asas de una tabla, que se
/// quedarian a la vista donde las dejo (el rastro al desplazarse). Se
/// repinta solo eso: la pista de la barra entera y donde cayeron las asas,
/// que se van (siguen al raton, no al texto; vuelven al moverlo). Y lo de
/// encima se le quita al control para que no lo borre (sin parpadeo).
pub(super) fn antes_de_pintar(edit: HWND) {
    let mut p = POINT::default();
    enviar(edit, EM_GETSCROLLPOS, 0, &mut p as *mut _ as isize);
    let antes = PINTADO_EN.with(|x| x.replace(p.y));
    if antes != p.y && antes != i32::MIN {
        let dy = antes - p.y;
        let mut sucio: Vec<RECT> = super::asas::puestas()
            .into_iter()
            .flat_map(|r| [r, RECT { top: r.top + dy, bottom: r.bottom + dy, ..r }])
            .collect();
        sucio.extend(imagenes::BARRA.with(|b| b.get()).map(|(pista, _)| pista));
        super::asas::al_desplazar();
        for r in &sucio {
            let r = RECT { left: r.left - 2, top: r.top - 2, right: r.right + 2, bottom: r.bottom + 2 };
            // SAFETY: rectangulo de una ventana propia.
            unsafe {
                let _ = InvalidateRect(Some(edit), Some(&r), false);
            }
        }
    }
    imagenes::reservar(edit);
}

/// Al cerrar la ventana.
pub(super) fn desmontar(e: &mut Estado) {
    acabar_enlace(e, false);
    if let Some(h) = e.vivo.barra.take() {
        // SAFETY: ventana propia.
        unsafe {
            let _ = DestroyWindow(h);
        }
    }
    barra_flotante::VISTA.with(|v| *v.borrow_mut() = None);
    imagenes::ADORNOS.with(|a| *a.borrow_mut() = None);
    imagenes::BARRA.with(|b| b.set(None));
    PINTADO_EN.with(|x| x.set(i32::MIN));
}

#[cfg(test)]
mod pruebas;
