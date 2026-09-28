//! Pinta la tarjeta de Proyectos del panel derecho y el interruptor de la
//! barra.
//!
//! Los colores son los del movil y no los del chat de estilo Telegram: la
//! tarjeta es la de Android, con su tarjeta de Material 3 (`Card`), su
//! `surfaceVariant` detras de cada hoja y la barra de cristal
//! (`ui/theme/Cristal.kt`). Salen de `ui/theme/Theme.kt`; lo que alli no se
//! fija (el fondo y la tarjeta de dia) son los de fabrica de Material 3.

use std::path::{Path, PathBuf};

use pixpin_geom::Rect;
use pixpin_proyecto::almacen::Ficha;
use pixpin_proyecto::cuaderno::Clase;
use pixpin_render::icono::{Icono, Pintura, TrazoIcono, material as mi};
use pixpin_render::letras::Letra;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_ui::proyectos as ui;

use super::super::{
    Ojeada, Pinta, Tema, encoger, papel_de_vista, pintar_lienzo, pintar_ojeada_tabla,
    pildora, pintar_tarjeta_galeria, rf, ruta_del_mensaje,
};
use super::cargar::{Hoja, Hojas};
use super::{ACCIONES, AccionBarra, VistaProyectos, Zona, de_que_es, hijos_de, hojas_visibles};
use crate::caja_dibujo::hex;

/// Los colores de la pantalla, los del tema del movil.
pub(crate) struct Paleta {
    /// `surface`: el fondo de la pantalla.
    fondo: Color,
    /// `surfaceContainerHighest`: la tarjeta (`Card` rellena de Material 3).
    tarjeta: Color,
    /// `surfaceVariant`: detras de la portada y de cada hoja.
    variante: Color,
    /// `onSurface` y `onSurfaceVariant`.
    texto: Color,
    texto_suave: Color,
    primario: Color,
    sobre_primario: Color,
    /// La barra de cristal (`surfaceContainerHigh` al 94 %) y su filo
    /// (`outlineVariant`).
    cristal: Color,
    filo: Color,
}

const fn alfa(c: Color, a: f32) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a,
    }
}

/// De dia: `LightColors` de `Theme.kt` (primario `#29B8DB`, `surfaceVariant`
/// `#DCE7EB`, `onSurfaceVariant` `#40484C`) y lo de fabrica de Material 3.
pub(crate) const CLARA: Paleta = Paleta {
    fondo: hex(0xfef7ff),
    tarjeta: hex(0xe6e0e9),
    variante: hex(0xdce7eb),
    texto: hex(0x1d1b20),
    texto_suave: hex(0x40484c),
    primario: hex(0x29b8db),
    sobre_primario: hex(0x00323d),
    cristal: alfa(hex(0xece6f0), 0.94),
    filo: hex(0xcac4d0),
};

/// De noche: `DarkColors` de `Theme.kt`, el negro apagado del movil.
pub(crate) const OSCURA: Paleta = Paleta {
    fondo: hex(0x0e1416),
    tarjeta: hex(0x2b3639),
    variante: hex(0x212b2e),
    texto: hex(0xe2e9eb),
    texto_suave: hex(0xb7c3c7),
    primario: hex(0x5cd3f0),
    sobre_primario: hex(0x00363f),
    cristal: alfa(hex(0x212b2e), 0.94),
    filo: hex(0x344145),
};

/// El filete de un sublienzo, el azul de la marca de las zonas.
const FILETE_SUBLIENZO: Color = hex(0x1971c2);

// --- Los iconos de Material que el chat aun no tenia ------------------------
//
// Los `d` del `24px.svg` oficial de `google/material-design-icons`
// (Apache-2.0), igual que `pixpin_render::icono::material`.

const VISTA: (f32, f32, f32, f32) = (0.0, 0.0, 24.0, 24.0);

const fn relleno(d: &'static str) -> TrazoIcono {
    TrazoIcono {
        d,
        relleno: Pintura::Actual,
        trazo: Pintura::Nada,
        grosor: 0.0,
        extremo_redondo: false,
        union_redonda: false,
        opacidad: 1.0,
        par_impar: false,
        matriz: None,
        mascara: None,
    }
}

/// `chat` (`src/communication/chat`).
const CHAT: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M20 2H4c-1.1 0-1.99.9-1.99 2L2 22l4-4h14c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zM6 9h12v2H6V9zm8 5H6v-2h8v2zm4-6H6V6h12v2z",
    )],
};
/// `add` (`src/content/add`).
const ADD: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno("M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z")],
};
/// `notes` (`src/editor/notes`).
const NOTES: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno("M3 18h12v-2H3v2zM3 6v2h18V6H3zm0 7h18v-2H3v2z")],
};
/// `grid_view` (`src/action/grid_view`).
const GRID_VIEW: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno(
        "M3 3v8h8V3H3zm6 6H5V5h4v4zm-6 4v8h8v-8H3zm6 6H5v-4h4v4zm4-16v8h8V3h-8zm6 6h-4V5h4v4zm-6 4v8h8v-8h-8zm6 6h-4v-4h4v4z",
    )],
};
/// `view_carousel` (`src/action/view_carousel`).
const VIEW_CAROUSEL: Icono = Icono {
    vista: VISTA,
    trazos: &[relleno("M7 19h10V4H7v15zm-5-2h4V6H2v11zM18 6v11h4V6h-4z")],
};

fn icono_de(a: AccionBarra) -> &'static Icono {
    match a {
        AccionBarra::Chat => &CHAT,
        AccionBarra::Hoja => &ADD,
        AccionBarra::Nota => &NOTES,
        AccionBarra::Tabla => &mi::TABLE_CHART,
    }
}

fn rotulo_de(a: AccionBarra) -> &'static str {
    match a {
        AccionBarra::Chat => "proyectos-chat",
        AccionBarra::Hoja => "proyectos-hoja",
        AccionBarra::Nota => "proyectos-nota",
        AccionBarra::Tabla => "proyectos-tabla",
    }
}

fn icono_centrado(p: &Pintor, icono: &Icono, caja: Rect, lado: f32, color: Color) {
    p.icono(
        icono,
        RectF {
            x: caja.x as f32 + (caja.ancho as f32 - lado) / 2.0,
            y: caja.y as f32 + (caja.alto as f32 - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        color,
    );
}

/// Apunta una zona pulsable, recortada a lo que se ve.
fn zona(v: &VistaProyectos, r: Rect, dentro: Rect, z: Zona) {
    if let Some(r) = r.interseccion(dentro) {
        v.zonas.borrow_mut().push((r, z));
    }
}

/// Lo que se le pasa a todo lo que pinta dentro de la pantalla.
struct Cx<'a> {
    v: &'a VistaProyectos,
    c: &'a Pinta<'a>,
    pal: &'a Paleta,
    raiz: &'a Path,
    e: f32,
    /// Lo que se ve de la pantalla: las zonas no salen de aqui.
    visible: Rect,
}

/// El panel derecho del chat (`d.chat`) con la tarjeta del proyecto `f`: el
/// circulo de volver del chat arriba a la izquierda, la tarjeta y la barra
/// de cristal. `nombre` es el que se ve: el que se esta escribiendo, con el
/// cursor detras si `escribiendo` (un proyecto recien creado con el «+»).
#[allow(clippy::too_many_arguments)]
pub(crate) fn pintar(
    p: &Pintor,
    v: &VistaProyectos,
    d: &pixpin_ui::chat::Disposicion,
    c: &Pinta,
    f: &Ficha,
    nombre: &str,
    escribiendo: bool,
    claro: bool,
    raiz: &Path,
) {
    v.zonas.borrow_mut().clear();
    v.fondos.borrow_mut().clear();
    *v.grande_pedida.borrow_mut() = None;
    let area = d.chat;
    if area.ancho == 0 || area.alto == 0 {
        return;
    }
    let pal = if claro { &CLARA } else { &OSCURA };
    let e = c.escala as f32 / 100.0;
    p.rellenar(rf(area), pal.fondo);
    let panel = ui::panel(area, d.cabecera_chat.alto, c.escala, ACCIONES.len());
    // Volver: el mismo circulo del chat, en el mismo sitio y haciendo lo
    // mismo. Con la ventana estrecha es lo que lleva de vuelta a la lista.
    let volver = d.pildoras(c.escala).volver;
    pildora(p, c.tema, rf(volver), e);
    icono_centrado(p, &mi::ARROW_BACK, volver, 24.0 * e, c.tema.pildora_texto);
    zona(v, volver, area, Zona::Volver);
    // Lo que se puede bajar lo sabe quien reparte: se apunta para la rueda.
    v.tope.set(panel.tope());
    v.alto_hueco.set(panel.hueco.alto as i32);
    let cx = Cx {
        v,
        c,
        pal,
        raiz,
        e,
        visible: panel.hueco,
    };
    p.empujar_recorte(rf(panel.hueco));
    tarjeta(p, &cx, f, nombre, escribiendo, panel.tarjeta(v.desplazamiento()));
    p.soltar_recorte();
    barra_de_acciones(p, &Cx { visible: area, ..cx }, &panel);
}

/// Lo que dice bajo el nombre (`proyecto_hojas` o `proyecto_hojas_de`).
fn detalle(cx: &Cx, f: &Ficha, d: Option<&Hojas>) -> String {
    let textos = cx.c.textos;
    let resumen = cx.v.resumen(&f.id);
    let total = d
        .map(|d| d.total)
        .or(resumen.map(|r| r.hojas))
        .unwrap_or(f.hojas as usize);
    let con_pdf = d.map(|d| d.con_pdf).or(resumen.map(|r| r.con_pdf)).unwrap_or(false);
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("total", total);
    if con_pdf {
        args.set("anotadas", d.map(|d| d.anotadas).unwrap_or(0));
        textos.t_args("proyectos-hojas-de", &args)
    } else {
        textos.t_args("proyectos-hojas", &args)
    }
}

/// El nombre de una hoja: el suyo en el proyecto, o el de su mensaje.
fn nombre_de(cx: &Cx, h: &Hoja) -> String {
    if !h.nombre.trim().is_empty() {
        return h.nombre.clone();
    }
    super::super::nombre_de_la_fila(&h.mensaje, cx.c.textos)
}

/// Una tarjeta: nombre, detalle, botones y lo de las hojas. `nombre` es el
/// que se ve (el que se esta escribiendo, si se esta poniendo), con el
/// cursor detras si `escribiendo`.
fn tarjeta(p: &Pintor, cx: &Cx, f: &Ficha, nombre: &str, escribiendo: bool, r: Rect) {
    let (e, pal, v) = (cx.e, cx.pal, cx.v);
    p.rellenar_redondeado(rf(r), ui::RADIO_TARJETA as f32 * e, pal.tarjeta);
    zona(v, r, cx.visible, Zona::Tarjeta(f.id.clone()));
    let nombre = if escribiendo {
        format!("{nombre}|")
    } else {
        nombre.to_string()
    };
    let d = v.datos(&f.id);
    let est = v.tarjeta(&f.id);
    let visibles = d
        .map(|d| hojas_visibles(&d.hojas, &est.abiertos))
        .unwrap_or_default();
    let hay = !visibles.is_empty();
    let archivado = d
        .map(|d| d.archivado)
        .or(v.resumen(&f.id).map(|r| r.archivado))
        .unwrap_or(false);

    // El nombre grande, en dos lineas como mucho; normal y apagado si esta
    // archivado.
    let provisional = ui::tarjeta(r, cx.c.escala, 0, hay, est.rejilla);
    let letra = Letra::de(if archivado { "Segoe UI" } else { "Segoe UI Semibold" });
    let tam = ui::NOMBRE_TAM * e;
    let ancho = provisional.texto.ancho as f32;
    let (_, alto) = p.medir_con_letra(&nombre, tam, ancho.max(1.0), &letra);
    let (_, linea) = p.medir_con_letra("Ag", tam, f32::MAX, &letra);
    let alto_nombre = alto.min(2.0 * linea).max(linea);
    let det = detalle(cx, f, d);
    let (_, alto_det) = p.medir_texto(&det, ui::DETALLE_TAM * e);
    let t = ui::tarjeta(
        r,
        cx.c.escala,
        (alto_nombre + alto_det).ceil() as u32,
        hay,
        est.rejilla,
    );
    let texto = rf(t.texto);
    p.empujar_recorte(RectF {
        alto: alto_nombre,
        ..texto
    });
    p.texto_con_letra(
        &nombre,
        texto.x,
        texto.y,
        tam,
        texto.ancho.max(1.0),
        &letra,
        if archivado { pal.texto_suave } else { pal.texto },
    );
    p.soltar_recorte();
    p.texto_linea(
        &det,
        texto.x,
        texto.y + alto_nombre,
        ui::DETALLE_TAM * e,
        texto.ancho,
        pal.texto_suave,
    );
    zona(v, t.texto, cx.visible, Zona::Nombre(f.id.clone()));
    // La rejilla o una pagina, y los tres puntos: iconos de 20.
    if hay {
        let icono = if est.rejilla { &VIEW_CAROUSEL } else { &GRID_VIEW };
        icono_centrado(p, icono, t.rejilla, 20.0 * e, pal.texto);
        zona(v, t.rejilla, cx.visible, Zona::Rejilla(f.id.clone()));
    }
    icono_centrado(p, &mi::MORE_VERT, t.menu, 20.0 * e, pal.texto);
    zona(v, t.menu, cx.visible, Zona::Menu(f.id.clone()));

    let Some(d) = d else {
        // Aun leyendose: el hueco de la portada, sin nada encima.
        p.rellenar_redondeado(rf(t.hojas), ui::RADIO_PORTADA as f32 * e, pal.variante);
        return;
    };
    if !hay {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("total", 0);
        let vacio = cx.c.textos.t_args("proyectos-hojas", &args);
        let tam = ui::DETALLE_TAM * e;
        let (w, h) = p.medir_texto(&vacio, tam);
        p.texto(
            &vacio,
            t.hojas.x as f32 + (t.hojas.ancho as f32 - w) / 2.0,
            t.hojas.y as f32 + (t.hojas.alto as f32 - h) / 2.0,
            tam,
            pal.texto_suave,
        );
        return;
    }
    let hijos = hijos_de(&d.hojas);
    if est.rejilla {
        let m = ui::rejilla(t.hojas.alto, visibles.len(), cx.c.escala);
        tira(p, cx, &f.id, d, &visibles, &hijos, t.hojas, m, None);
        return;
    }
    // La portada sigue a la hoja elegida; si esa ya no se ve (se plego o se
    // borro), la primera.
    let en_foco = if visibles.contains(&est.en_foco) {
        est.en_foco
    } else {
        visibles[0]
    };
    portada(p, cx, &f.id, d, en_foco, t.hojas);
    let m = if t.apaisada {
        ui::tira_en_filas(t.tira.alto, visibles.len(), cx.c.escala)
    } else {
        ui::tira(visibles.len())
    };
    tira(p, cx, &f.id, d, &visibles, &hijos, t.tira, m, Some(en_foco));
}

/// La ruta de la pagina (o la foto) de la portada a la nitidez de su hueco,
/// si la hay ya pintada; si no, se pide y se ensena la de siempre mientras.
fn ruta_grande(cx: &Cx, proyecto: &str, h: &Hoja, ancho: u32) -> Option<PathBuf> {
    let m = &h.mensaje;
    if m.clase == Some(Clase::Imagen) {
        return ruta_del_mensaje(cx.raiz, proyecto, m);
    }
    let pagina = m.pagina?;
    let pdf = crate::pdf_en_chat::documento_de(cx.raiz, proyecto)?;
    crate::pdf_en_chat::pagina_pintada(cx.raiz, &pdf, pagina, ui::nitidez_para(ancho)).map(|(r, _, _)| r)
}

/// La hoja que se esta mirando, del tamano de mirarla, con su nombre en una
/// esquina. Un clic la abre.
fn portada(p: &Pintor, cx: &Cx, proyecto: &str, d: &Hojas, n: usize, caja: Rect) {
    let (e, pal) = (cx.e, cx.pal);
    p.rellenar_redondeado(rf(caja), ui::RADIO_PORTADA as f32 * e, pal.variante);
    let Some(h) = d.hojas.get(n) else { return };
    zona(cx.v, caja, cx.visible, Zona::Portada(proyecto.to_string()));
    p.empujar_recorte(rf(caja));
    let grande = ruta_grande(cx, proyecto, h, caja.ancho);
    contenido(p, cx, proyecto, h, caja, grande.as_deref(), ui::NOTA_PORTADA_TAM);
    let nombre = nombre_de(cx, h);
    let tam = ui::PORTADA_ETIQUETA_TAM * e;
    let (w, alto) = p.medir_texto(&nombre, tam);
    let w = w.min(caja.ancho as f32 - 32.0 * e).max(0.0);
    let etiqueta = RectF {
        x: caja.x as f32 + 8.0 * e,
        y: caja.abajo() as f32 - 8.0 * e - alto - 6.0 * e,
        ancho: w + 16.0 * e,
        alto: alto + 6.0 * e,
    };
    let fondo = if std::ptr::eq(pal, &CLARA) {
        alfa(CLARA.fondo, 0.85)
    } else {
        alfa(OSCURA.fondo, 0.85)
    };
    p.rellenar_redondeado(etiqueta, 6.0 * e, fondo);
    p.texto_linea(&nombre, etiqueta.x + 8.0 * e, etiqueta.y + 3.0 * e, tam, w + 1.0, pal.texto);
    p.soltar_recorte();
}

/// Lo de dentro de una hoja, en la portada o en una miniatura.
fn contenido(
    p: &Pintor,
    cx: &Cx,
    proyecto: &str,
    h: &Hoja,
    caja: Rect,
    grande: Option<&Path>,
    tam_nota: f32,
) {
    let (e, pal, c) = (cx.e, cx.pal, cx.c);
    let m = &h.mensaje;
    let r = rf(caja);
    if m.clase == Some(Clase::Nota) {
        // La nota compuesta en pequeno sobre el `surfaceVariant`, con 8 y 6 de
        // aire (`MiniaturaDeNota`), sin las marcas del Markdown a la vista.
        p.parrafo(
            &texto_de_nota(&m.texto),
            r.x + 8.0 * e,
            r.y + 6.0 * e,
            tam_nota * e,
            (r.ancho - 16.0 * e).max(1.0),
            &[],
            pal.texto,
        );
        return;
    }
    if let Some(g) = grande {
        *cx.v.grande_pedida.borrow_mut() = Some(g.to_path_buf());
    }
    let alta = grande.and_then(|g| {
        let (ruta, mini) = cx.v.grande.as_ref()?;
        (ruta.as_path() == g).then_some(())?;
        mini.ya(g)
    });
    match &h.vista {
        Some(Ojeada::Lienzo(l)) => {
            if l.fondo.is_none() {
                p.rellenar(r, papel_de_vista(l, c.tema.papel));
            }
            if let Some((ruta, _)) = &l.fondo {
                cx.v.fondos.borrow_mut().push(ruta.clone());
            }
            match (alta, &l.fondo) {
                // La pagina a su nitidez debajo y los trazos encima, con la
                // misma cuenta de encaje que `pintar_lienzo`.
                (Some((b, _, _)), Some((_, (fx0, fy0, fx1, fy1)))) => {
                    let (x0, y0, x1, y1) = l.caja;
                    let (w, hh) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
                    let s = (r.ancho / w).min(r.alto / hh);
                    let dx = r.x + (r.ancho - w * s) / 2.0;
                    let dy = r.y + (r.alto - hh * s) / 2.0;
                    let destino = RectF {
                        x: (fx0 - x0) * s + dx,
                        y: (fy0 - y0) * s + dy,
                        ancho: (fx1 - fx0) * s,
                        alto: (fy1 - fy0) * s,
                    };
                    p.bitmap_con(b, destino, None, pixpin_render::Interpolacion::Lineal);
                    pintar_lienzo(p, l, r, None);
                }
                _ => pintar_lienzo(p, l, r, Some(c.previas)),
            }
        }
        Some(Ojeada::Tabla(t)) => {
            p.rellenar(r, c.tema.papel);
            pintar_ojeada_tabla(p, c.tema, c.escala, t, encoger(r, 4.0 * e));
        }
        Some(Ojeada::Foto { .. }) => {
            let Some(ruta) = ruta_del_mensaje(cx.raiz, proyecto, m) else {
                return;
            };
            cx.v.fondos.borrow_mut().push(ruta.clone());
            // En la portada, entera; en la tira, llenando la celda.
            match alta.or_else(|| grande.and_then(|_| c.previas.ya(&ruta))) {
                Some((b, w, hh)) => {
                    let s = (r.ancho / w.max(1) as f32).min(r.alto / hh.max(1) as f32);
                    let (dw, dh) = (w as f32 * s, hh as f32 * s);
                    p.bitmap_con(
                        b,
                        RectF {
                            x: r.x + (r.ancho - dw) / 2.0,
                            y: r.y + (r.alto - dh) / 2.0,
                            ancho: dw,
                            alto: dh,
                        },
                        None,
                        pixpin_render::Interpolacion::Lineal,
                    );
                }
                None => {
                    if let Some((b, w, hh)) = c.previas.ya(&ruta) {
                        crate::miniaturas::pintar_recortado(p, b, r, w, hh);
                    }
                }
            }
        }
        // Una pagina que aun se esta pintando: el hueco, y llega sola.
        None if h.por_pedir || m.pagina.is_some() => {}
        None => pintar_tarjeta_galeria(p, c, m, caja),
    }
}

/// Cuantos bloques de una nota entran en una miniatura
/// (`BLOQUES_DE_LA_MINIATURA`): componer doscientos para recortarlos es
/// trabajo tirado en cada fotograma.
const BLOQUES_DE_LA_MINIATURA: usize = 24;

/// El texto de una nota como se lee, sin las marcas del Markdown: el movil
/// la compone con `MarkdownText` y ahi no se ven ni las almohadillas ni los
/// corchetes de las tareas. Aqui, en pequeno, basta con quitarlas y poner la
/// vineta o la casilla que dicen.
pub(crate) fn texto_de_nota(md: &str) -> String {
    md.lines()
        .take(BLOQUES_DE_LA_MINIATURA)
        .map(|l| {
            let sangria = &l[..l.len() - l.trim_start().len()];
            let t = l.trim_start();
            let cuerpo = if let Some(r) = t.strip_prefix("- [ ] ").or(t.strip_prefix("* [ ] ")) {
                format!("☐ {r}")
            } else if let Some(r) = ["- [x] ", "- [X] ", "* [x] ", "* [X] "]
                .iter()
                .find_map(|p| t.strip_prefix(p))
            {
                format!("☑ {r}")
            } else if let Some(r) = t.strip_prefix("- ").or(t.strip_prefix("* ")) {
                format!("• {r}")
            } else if t.starts_with('#') {
                t.trim_start_matches('#').trim_start().to_string()
            } else if let Some(r) = t.strip_prefix("> ") {
                r.to_string()
            } else {
                t.to_string()
            };
            format!("{sangria}{}", cuerpo.replace("**", "").replace("__", ""))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// La tira, las filas de debajo del nombre o la rejilla: miniaturas en su
/// hueco, que se corre de lado con la rueda.
#[allow(clippy::too_many_arguments)]
fn tira(
    p: &Pintor,
    cx: &Cx,
    proyecto: &str,
    d: &Hojas,
    visibles: &[usize],
    hijos: &std::collections::HashMap<&str, usize>,
    area: Rect,
    m: ui::Mosaico,
    en_foco: Option<usize>,
) {
    if area.ancho == 0 || area.alto == 0 {
        return;
    }
    let escala = cx.c.escala;
    let maximo = m.desplazamiento_maximo(area.ancho, escala).max(0);
    // Lo corrida que puede estar, y la hoja que haya que ensenar.
    let corrida = cx.v.con_tarjeta(proyecto, |t| {
        if let Some(pos) = t.ver.take() {
            t.corrida = m.para_ver(pos, area.ancho, t.corrida, escala);
        }
        t.corrida = t.corrida.clamp(0, maximo);
        t.corrida
    });
    let dentro = area.interseccion(cx.visible);
    if let Some(r) = dentro {
        cx.v.zonas.borrow_mut().push((r, Zona::Tira(proyecto.to_string())));
    }
    // La etiqueta de debajo va fuera de la celda: el recorte deja sitio.
    p.empujar_recorte(rf(area));
    for x in m.columnas_visibles(area.ancho, corrida, escala) {
        for fila in 0..m.filas {
            let Some(k) = m.indice(x, fila) else { continue };
            let n = visibles[k];
            let celda = m.celda(area, x, fila, corrida, escala);
            miniatura(p, cx, proyecto, d, n, celda, en_foco == Some(n), hijos);
            if let Some(dentro) = dentro {
                zona(cx.v, celda, dentro, Zona::Hoja(proyecto.to_string(), n));
            }
            // El contador de sublienzos va encima de la hoja: se apunta despues.
            let h = &d.hojas[n];
            let cuantos = hijos.get(h.id.as_str()).copied().unwrap_or(0);
            if let Some(dentro) = dentro
                && cuantos > 0
            {
                let abierto = cx.v.tarjeta(proyecto).abiertos.contains(&h.id);
                zona(
                    cx.v,
                    caja_del_plegado(p, cx, celda, cuantos, abierto),
                    dentro,
                    Zona::Plegado(proyecto.to_string(), h.id.clone()),
                );
            }
        }
    }
    p.soltar_recorte();
}

/// Donde va el contador de sublienzos de una hoja (y, si se pide, lo pinta).
fn caja_del_plegado(p: &Pintor, cx: &Cx, celda: Rect, cuantos: usize, abierto: bool) -> Rect {
    let e = cx.e;
    let texto = format!("{} {cuantos}", if abierto { "▾" } else { "▸" });
    let tam = ui::ETIQUETA_TAM * e;
    let (w, h) = p.medir_texto(&texto, tam);
    Rect {
        x: celda.x + (3.0 * e) as i32,
        y: celda.abajo() - (3.0 * e + h + 4.0 * e) as i32,
        ancho: (w + 12.0 * e) as u32,
        alto: (h + 4.0 * e) as u32,
    }
}

/// Una miniatura de la tira: su hoja, el marco del color de su lienzo (o del
/// primario si es la de la portada), la etiqueta de su clase, el punto si
/// esta anotada y su nombre debajo.
#[allow(clippy::too_many_arguments)]
fn miniatura(
    p: &Pintor,
    cx: &Cx,
    proyecto: &str,
    d: &Hojas,
    n: usize,
    celda: Rect,
    en_foco: bool,
    hijos: &std::collections::HashMap<&str, usize>,
) {
    let (e, pal) = (cx.e, cx.pal);
    let Some(h) = d.hojas.get(n) else { return };
    let m = &h.mensaje;
    let r = rf(celda);
    let radio = ui::RADIO_HOJA as f32 * e;
    p.rellenar_redondeado(r, radio, pal.variante);
    p.empujar_recorte(r);
    contenido(p, cx, proyecto, h, celda, None, ui::NOTA_TAM);
    // Un sublienzo lleva un filete a la izquierda.
    if h.padre.is_some() {
        p.rellenar(
            RectF {
                ancho: 4.0 * e,
                ..r
            },
            FILETE_SUBLIENZO,
        );
    }
    // Sus sublienzos, plegados bajo un contador que los despliega.
    let cuantos = hijos.get(h.id.as_str()).copied().unwrap_or(0);
    if cuantos > 0 {
        let abierto = cx.v.tarjeta(proyecto).abiertos.contains(&h.id);
        let caja = caja_del_plegado(p, cx, celda, cuantos, abierto);
        let texto = format!("{} {cuantos}", if abierto { "▾" } else { "▸" });
        p.rellenar_redondeado(rf(caja), 10.0 * e, pal.primario);
        p.texto_con_letra(
            &texto,
            caja.x as f32 + 6.0 * e,
            caja.y as f32 + 2.0 * e,
            ui::ETIQUETA_TAM * e,
            f32::MAX,
            &Letra::de("Segoe UI Semibold"),
            pal.sobre_primario,
        );
    }
    // De que es, en la esquina: MD, 2D, fx.
    if let Some(que) = de_que_es(m) {
        let tam = ui::ETIQUETA_TAM * e;
        let (w, alto) = p.medir_texto(que, tam);
        let chapa = RectF {
            x: r.x + r.ancho - 3.0 * e - w - 8.0 * e,
            y: r.y + 3.0 * e,
            ancho: w + 8.0 * e,
            alto: alto + 2.0 * e,
        };
        let fondo = if std::ptr::eq(pal, &CLARA) {
            alfa(CLARA.fondo, 0.82)
        } else {
            alfa(OSCURA.fondo, 0.82)
        };
        p.rellenar_redondeado(chapa, 4.0 * e, fondo);
        p.texto(que, chapa.x + 4.0 * e, chapa.y + 1.0 * e, tam, pal.texto_suave);
    }
    // El punto de una pagina anotada, solo si de verdad lleva algo.
    if m.pagina.is_some() && h.anotada {
        let centro = (r.x + r.ancho - 4.0 * e - 3.5 * e, r.y + r.alto - 4.0 * e - 3.5 * e);
        p.circulo(centro, 4.5 * e, if std::ptr::eq(pal, &CLARA) { CLARA.fondo } else { OSCURA.fondo });
        p.circulo(centro, 3.5 * e, pal.primario);
    }
    // Marcada (clic derecho): el velo del primario al 22 % y el circulo con
    // la palomita arriba a la derecha, como la miniatura del movil.
    if cx.v.tarjeta(proyecto).marcadas.contains(&h.id) {
        p.rellenar_redondeado(r, radio, alfa(pal.primario, 0.22));
        let lado = 16.0 * e;
        let caja = RectF {
            x: r.x + r.ancho - 3.0 * e - lado,
            y: r.y + 3.0 * e,
            ancho: lado,
            alto: lado,
        };
        p.circulo((caja.x + lado / 2.0, caja.y + lado / 2.0), lado * 0.42, pal.fondo);
        p.icono(&mi::CHECK_CIRCLE, caja, pal.primario);
    }
    p.soltar_recorte();
    // El marco: el foco manda sobre el color del lienzo. Las paginas y las
    // notas no llevan color (`HojasDelProyecto.colorDe`).
    let color = if en_foco {
        Some(pal.primario)
    } else if m.pagina.is_none() {
        m.referencia
            .as_deref()
            .and_then(pixpin_ui::chat::color_del_borde)
            .map(hex)
    } else {
        None
    };
    if let Some(color) = color {
        p.trazar(encoger(r, 1.0 * e), 2.0 * e, color);
    }
    // Su nombre debajo, en una linea.
    let nombre = nombre_de(cx, h);
    let tam = ui::ETIQUETA_TAM * e;
    let (w, _) = p.medir_texto(&nombre, tam);
    let ancho = r.ancho;
    p.texto_linea(
        &nombre,
        r.x + ((ancho - w) / 2.0).max(0.0),
        r.y + r.alto + 2.0 * e,
        tam,
        ancho,
        pal.texto,
    );
}

/// La barra de cristal con lo que se hace a diario, del proyecto que se ve.
fn barra_de_acciones(p: &Pintor, cx: &Cx, pan: &ui::Panel) {
    let (e, pal) = (cx.e, cx.pal);
    let b = pan.barra;
    if b.ancho == 0 {
        return;
    }
    let radio = (ui::RADIO_BARRA as f32 * e).min(b.ancho.min(b.alto) as f32 / 2.0);
    // El filo de un punto, y el cristal encima.
    p.rellenar_redondeado(rf(b), radio, pal.filo);
    p.rellenar_redondeado(encoger(rf(b), 1.0 * e), radio - 1.0 * e, pal.fondo);
    p.rellenar_redondeado(encoger(rf(b), 1.0 * e), radio - 1.0 * e, pal.cristal);
    for (n, a) in ACCIONES.iter().enumerate() {
        let caja = pan.boton(n);
        let lado = 24.0 * e;
        p.icono(
            icono_de(*a),
            RectF {
                x: caja.x as f32 + (caja.ancho as f32 - lado) / 2.0,
                y: caja.y as f32 + 6.0 * e,
                ancho: lado,
                alto: lado,
            },
            pal.texto,
        );
        let rotulo = cx.c.textos.t(rotulo_de(*a));
        let tam = ui::ETIQUETA_TAM * e;
        let (w, _) = p.medir_texto(&rotulo, tam);
        p.texto_linea(
            &rotulo,
            caja.x as f32 + ((caja.ancho as f32 - w) / 2.0).max(0.0),
            caja.y as f32 + 6.0 * e + lado + 3.0 * e,
            tam,
            caja.ancho as f32,
            pal.texto,
        );
        zona(cx.v, caja, cx.visible, Zona::Boton(*a));
    }
}

/// El interruptor Chat ↔ Proyectos, en la barra de titulo: un selector de
/// dos con la mitad elegida puesta, como los de Windows 11. Se pinta en las
/// dos vistas, siempre a la vista.
pub(crate) fn pintar_interruptor(
    p: &Pintor,
    v: &VistaProyectos,
    d: &pixpin_ui::chat::Disposicion,
    tema: &Tema,
    escala: u32,
    textos: &pixpin_store::Catalogo,
) {
    let e = escala as f32 / 100.0;
    let barra = d.barra;
    if barra.ancho == 0 {
        return;
    }
    // Desde donde acaba el nombre del programa hasta el primer boton de la
    // ventana (el de minimizar, el de mas a la izquierda).
    let (nombre, _) = p.medir_texto(&textos.t("app-nombre"), pixpin_ui::chat::TITULO_TAM * e);
    let desde = barra.x + (12.0 * e + nombre + 12.0 * e) as i32;
    let hasta = d
        .botones_barra(escala)
        .iter()
        .map(|(_, r)| r.x)
        .min()
        .unwrap_or(barra.derecha());
    let i = ui::interruptor(barra, desde, hasta, escala);
    v.interruptor.set(Some(i));
    let radio = i.caja.alto as f32 / 2.0;
    p.rellenar_redondeado(rf(i.caja), radio, tema.boton_sobre);
    let puesta = if v.activa() { i.proyectos } else { i.chat };
    p.rellenar_redondeado(encoger(rf(puesta), 2.0 * e), radio - 2.0 * e, tema.fila_elegida);
    for (mitad, clave, elegida) in [
        (i.chat, "proyectos-interruptor-chat", !v.activa()),
        (i.proyectos, "proyectos-interruptor-proyectos", v.activa()),
    ] {
        let t = textos.t(clave);
        let tam = ui::INTERRUPTOR_TAM * e;
        let (w, h) = p.medir_texto(&t, tam);
        p.texto(
            &t,
            mitad.x as f32 + (mitad.ancho as f32 - w) / 2.0,
            mitad.y as f32 + (mitad.alto as f32 - h) / 2.0,
            tam,
            if elegida { tema.texto_elegido } else { tema.apagado },
        );
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El contraste de dos colores (WCAG), para comprobar que la paleta se lee.
    fn contraste(a: Color, b: Color) -> f32 {
        let l = |c: Color| {
            let f = |v: f32| {
                if v <= 0.03928 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b)
        };
        let (x, y) = (l(a), l(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn el_texto_de_la_tarjeta_se_lee_de_dia_y_de_noche() {
        for pal in [&CLARA, &OSCURA] {
            assert!(contraste(pal.texto, pal.tarjeta) >= 7.0);
            assert!(contraste(pal.texto_suave, pal.tarjeta) >= 4.5);
            assert!(contraste(pal.sobre_primario, pal.primario) >= 4.5);
        }
    }

    #[test]
    fn la_tarjeta_se_despega_del_fondo_y_la_hoja_de_la_tarjeta() {
        for pal in [&CLARA, &OSCURA] {
            assert!(contraste(pal.tarjeta, pal.fondo) > 1.1);
            assert!(contraste(pal.variante, pal.tarjeta) > 1.02);
        }
    }

    #[test]
    fn la_nota_en_pequeno_se_lee_sin_las_marcas_del_markdown() {
        let md = "# Presupuesto\n\n**Acero**: 1 200\n- [ ] Pedir\n- [x] Medir\n  - suelta\n> cita";
        assert_eq!(
            texto_de_nota(md),
            "Presupuesto\n\nAcero: 1 200\n☐ Pedir\n☑ Medir\n  • suelta\ncita"
        );
        // Lo que no es Markdown se queda como estaba, almohadilla en medio incluida.
        assert_eq!(texto_de_nota("Piso #3"), "Piso #3");
        // Y una nota enorme no se compone entera.
        let larga = "linea\n".repeat(500);
        assert_eq!(texto_de_nota(&larga).lines().count(), BLOQUES_DE_LA_MINIATURA);
    }

    #[test]
    fn cada_accion_de_la_barra_tiene_su_icono_y_su_palabra() {
        for a in ACCIONES {
            assert!(!icono_de(a).trazos.is_empty());
            assert!(rotulo_de(a).starts_with("proyectos-"));
        }
    }
}
