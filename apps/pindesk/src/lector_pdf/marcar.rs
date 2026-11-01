//! **Marcar el texto leyendo** (1-nov-2026): resaltar, raya ondulada,
//! subrayar, tachar y tapar lo elegido en un PDF, un Word o un PowerPoint,
//! con la misma herramienta que el texto reconocido de los pines.
//!
//! El usuario: «en las imagenes, con la T se reconoce el texto y puedo
//! seleccionar y subrayar, tachar, resaltar u ocultar; quiero la MISMA
//! herramienta en el visor de PDF, solo leyendo (no en el modo del lapiz)».
//!
//! - **Elegir** es lo de siempre (`elegir` en un PDF, `office` en un Word):
//!   arrastrar, doble clic la palabra, triple el renglon, Ctrl+C. En una hoja
//!   **escaneada** el texto lo pone el OCR (el mismo de los pines, Windows o
//!   Indra) la primera vez que se pasa el raton por encima, en el hilo de las
//!   hojas; **T** lo pide a la fuerza para la hoja bajo el raton (o la del
//!   centro), por si la capa de texto del PDF esta mal. Sobre lo reconocido
//!   se elige por palabras enteras, como en el pin
//!   (`pindesk_docs::capa_de_texto`).
//! - **La barrita** (`pindesk_ui::barra_marcar`) sale encima de lo elegido:
//!   copiar, los cinco colores, las cinco marcas y el circulo tachado, con
//!   los dibujitos de la barra del pin (`pindesk_pin::barra_marcas`).
//! - **Un clic en una marca** la elige entera (aunque pase a otra hoja):
//!   otra marca la cambia y el circulo tachado la quita. Con texto elegido a
//!   mano, las marcas se apilan como en el pin (resaltar y subrayar a la vez)
//!   y el circulo quita las que caen en lo elegido.
//! - **Ctrl+Z / Ctrl+Y**: la ultima marca, entera (en todas sus hojas).
//!
//! **Donde van**: son dibujo normal de la capa de cada hoja (la del lapiz,
//! `hoja-<n>.excalidraw` o la `anot-<uid>-p<n>` de un PDF de proyecto), en
//! las unidades de la hoja y con el grupo `marca-texto-<ms>`
//! (`pindesk_motor2d::marcas_texto`). Asi se ven leyendo y anotando, salen al
//! exportar, compartir e imprimir con lo anotado, ni acercar ni pasar hojas
//! las separan del texto y viajan al movil como cualquier trazo. El formato
//! esta en `docs/sincronizar-marcos-vistas-y-tinta.md`.

use std::ops::{Range, RangeInclusive};

use pindesk_docs::capa_de_texto;
use pindesk_motor2d::marcas_texto::{self as mt, HojaEnPantalla, Marca};
use pindesk_pdf::texto::PaginaDeTexto;
use pindesk_ui::barra_marcar::{self, BotonMarcar};

use super::office::Seleccion;
use super::*;

/// El ancho al que se dibuja una hoja para el OCR: lo del asistente
/// (`documentos_acciones`), que lee bien la letra de un libro escaneado.
pub(super) const ANCHO_OCR: u32 = 1700;

/// Lo de marcar del lector.
#[derive(Debug)]
pub(super) struct MarcarTexto {
    /// El color elegido (`marcas_texto::COLORES`).
    pub(super) color: u8,
    /// La marca elegida con un clic encima (su grupo): otra marca la cambia
    /// y el circulo tachado la quita.
    pub(super) de_marca: Option<String>,
    /// La hoja cuyo texto se pidio reconocer con la T, hasta que llega.
    pub(super) reconociendo: Option<usize>,
}

impl Default for MarcarTexto {
    fn default() -> Self {
        Self {
            color: mt::COLOR_INICIAL,
            de_marca: None,
            reconociendo: None,
        }
    }
}

/// Lo elegido ahora: lo de un Word o lo de un PDF.
fn seleccion(e: &Estado) -> Option<Seleccion> {
    match e.office.as_ref() {
        Some(o) => o.seleccion,
        None => e.elegir.seleccion,
    }
    .filter(|s| !s.vacia())
}

fn poner_seleccion(e: &mut Estado, s: Option<Seleccion>) {
    match e.office.as_mut() {
        Some(o) => o.seleccion = s,
        None => e.elegir.seleccion = s,
    }
}

fn hojas_de(s: Seleccion) -> RangeInclusive<usize> {
    s.ancla.0.min(s.punta.0)..=s.ancla.0.max(s.punta.0)
}

/// **Las letras elegidas de la hoja `i`**: las de la seleccion y, en una
/// hoja reconocida por el OCR, llevadas a palabras enteras.
pub(super) fn rango_en_hoja(
    sel: Seleccion,
    i: usize,
    t: &PaginaDeTexto,
    reconocida: bool,
) -> Option<Range<usize>> {
    let r = sel.rango_en(i, t.cajas.len())?;
    if !reconocida {
        return Some(r);
    }
    let letras: Vec<char> = t.texto.chars().collect();
    let (a, b) = capa_de_texto::a_palabras_enteras(&letras, r.start, r.end);
    (b > a).then_some(a..b)
}

/// El texto de la hoja `i` (y si lo puso el OCR), si ya esta.
fn con_texto<R>(e: &mut Estado, i: usize, f: impl FnOnce(&PaginaDeTexto, bool) -> R) -> Option<R> {
    match e.office.as_mut() {
        Some(o) => o.texto(i).map(|t| f(t, false)),
        None => {
            let h = e.elegir.hojas.get(&i)?;
            Some(f(&h.texto, h.reconocida))
        }
    }
}

/// **Las cajas de lo elegido, hoja a hoja**, en fracciones de cada hoja.
fn elegido_por_hoja(e: &mut Estado) -> Vec<(usize, Vec<[f32; 4]>)> {
    let Some(sel) = seleccion(e) else {
        return Vec::new();
    };
    hojas_de(sel)
        .filter_map(|i| {
            let cajas = con_texto(e, i, |t, reconocida| {
                rango_en_hoja(sel, i, t, reconocida)
                    .map(|r| t.cajas_de(r.start, r.end - r.start))
                    .unwrap_or_default()
            })?;
            (!cajas.is_empty()).then_some((i, cajas))
        })
        .collect()
}

/// La hoja `i` en la ventana.
fn en_pantalla(e: &Estado, i: usize, m: Marco) -> Option<HojaEnPantalla> {
    let s = px(e, m);
    Some(HojaEnPantalla {
        x: -e.x * s,
        y: (e.hojas.arriba.get(i)? - e.y) * s,
        escala: s,
        giro: 0,
        ancho: vista::ANCHO_HOJA,
        alto: *e.hojas.altos.get(i)?,
    })
}

fn unir(a: Option<[f32; 4]>, c: [f32; 4]) -> Option<[f32; 4]> {
    Some(match a {
        None => c,
        Some(u) => [
            u[0].min(c[0]),
            u[1].min(c[1]),
            u[2].max(c[2]),
            u[3].max(c[3]),
        ],
    })
}

/// Las cajas (en unidades de la hoja) de lo del grupo `g` en la hoja `i`.
fn cajas_del_grupo(e: &Estado, i: usize, g: &str) -> Vec<[f32; 4]> {
    e.capas
        .get(&i)
        .map(|c| {
            c.escena
                .visibles()
                .filter(|el| el.grupos.iter().any(|x| x == g))
                .map(|el| {
                    let (x0, y0, x1, y1) = el.caja();
                    [x0, y0, x1, y1]
                })
                .collect()
        })
        .unwrap_or_default()
}

/// **Donde esta lo elegido en la ventana** (lo que se ve de ello): la caja
/// que abarca su texto y la marca elegida. `None` si no hay nada o no se ve.
fn zona_en_pantalla(e: &mut Estado, m: Marco) -> Option<[f32; 4]> {
    let vis = visibles(e, m);
    let mut zona = None;
    for (i, cajas) in elegido_por_hoja(e) {
        if !vis.contains(&i) {
            continue;
        }
        let (Some(h), Some(&alto)) = (en_pantalla(e, i, m), e.hojas.altos.get(i)) else {
            continue;
        };
        for c in mt::de_fracciones(&cajas, vista::ANCHO_HOJA, alto) {
            zona = unir(zona, h.caja_a_pantalla(c));
        }
    }
    if let Some(g) = e.marcar.de_marca.clone() {
        for i in vis {
            let Some(h) = en_pantalla(e, i, m) else {
                continue;
            };
            for c in cajas_del_grupo(e, i, &g) {
                zona = unir(zona, h.caja_a_pantalla(c));
            }
        }
    }
    let z = zona?;
    // Lo que se sale de la ventana no cuenta (la barrita va junto a lo visto).
    let z = [
        z[0].max(0.0),
        z[1].max(0.0),
        z[2].min(m.ancho),
        z[3].min(m.alto),
    ];
    (z[2] > z[0] && z[3] > z[1]).then_some(z)
}

/// Si la barrita tiene que salir: leyendo, sin la caja del nombre ni las
/// fotos, con algo elegido.
fn a_la_vista(e: &Estado) -> bool {
    !e.anotando
        && !e.fotografiando
        && e.error.is_none()
        && e.renombre.editando.is_none()
        && (seleccion(e).is_some() || e.marcar.de_marca.is_some())
}

/// El nombre de un boton (para el aviso al pasar por encima): los del pin.
fn nombre(b: BotonMarcar, textos: &Catalogo) -> Option<String> {
    let clave = match b {
        BotonMarcar::Copiar => "pin-v2-marca-copiar",
        BotonMarcar::Color(_) => return None,
        BotonMarcar::Marcar(Marca::Resaltar) => "pin-v2-marca-resaltar",
        BotonMarcar::Marcar(Marca::Ondulada) => "pin-v2-marca-ondulada",
        BotonMarcar::Marcar(Marca::Subrayar) => "pin-v2-marca-subrayar",
        BotonMarcar::Marcar(Marca::Tachar) => "pin-v2-marca-tachar",
        BotonMarcar::Marcar(Marca::Tapar) => "pin-v2-marca-tapar",
        BotonMarcar::Quitar => "pin-v2-marca-quitar",
    };
    Some(textos.t(clave))
}

fn caja_f(c: barra_marcar::Caja) -> RectF {
    RectF {
        x: c.x,
        y: c.y,
        ancho: c.ancho,
        alto: c.alto,
    }
}

/// **Pinta la barrita** junto a lo elegido y apunta sus botones.
pub(super) fn pintar_barrita(
    e: &mut Estado,
    p: &Pintor,
    m: Marco,
    textos: &Catalogo,
    botones: &mut Vec<(RectF, Accion)>,
) {
    if !a_la_vista(e) {
        return;
    }
    let Some(zona) = zona_en_pantalla(e, m) else {
        return;
    };
    let b = barra_marcar::disponer(zona, (m.ancho, m.alto), m.e);
    let caja = caja_f(b.caja);
    // La sombra y el cristal de las barras del lector.
    p.rellenar_redondeado(
        RectF {
            y: caja.y + 2.0 * m.e,
            ..caja
        },
        10.0 * m.e,
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.30,
        },
    );
    p.rellenar_redondeado(caja, 10.0 * m.e, con_alfa(CRISTAL, 0.97));
    // Un clic en el cristal, entre dos botones, no es del documento.
    botones.push((caja, Accion::FondoBarrita));
    p.rellenar(
        RectF {
            x: b.raya,
            y: caja.y + caja.alto * 0.25,
            ancho: 1.0 * m.e,
            alto: caja.alto * 0.5,
        },
        con_alfa(RAYA, 0.9),
    );
    let elegido = pindesk_pin::barra_marcas::color(e.marcar.color);
    let encima = b.boton_en(e.raton);
    for (c, boton) in &b.botones {
        let r = caja_f(*c);
        if encima == Some(*boton) && !matches!(boton, BotonMarcar::Color(_)) {
            p.rellenar_redondeado(r, 7.0 * m.e, con_alfa(TEXTO, 0.12));
        }
        match *boton {
            BotonMarcar::Copiar => {
                let l = 19.0 * m.e;
                p.icono(
                    &material::CONTENT_COPY,
                    RectF {
                        x: r.x + (r.ancho - l) / 2.0,
                        y: r.y + (r.alto - l) / 2.0,
                        ancho: l,
                        alto: l,
                    },
                    TEXTO,
                );
            }
            BotonMarcar::Color(k) => {
                let (cx, cy) = (r.x + r.ancho / 2.0, r.y + r.alto / 2.0);
                if k == e.marcar.color {
                    p.circulo((cx, cy), 8.5 * m.e, TEXTO);
                    p.circulo((cx, cy), 7.0 * m.e, CRISTAL);
                }
                p.circulo((cx, cy), 5.5 * m.e, pindesk_pin::barra_marcas::color(k));
            }
            BotonMarcar::Marcar(mk) => {
                pindesk_pin::barra_marcas::pintar_icono(p, Some(mk), r, elegido, TEXTO, m.e)
            }
            BotonMarcar::Quitar => {
                pindesk_pin::barra_marcas::pintar_icono(p, None, r, elegido, TEXTO, m.e)
            }
        }
        botones.push((r, Accion::Marcar(*boton)));
    }
    // El nombre del boton bajo el raton, debajo de la barrita (o encima si
    // abajo no cabe).
    if let Some(nombre) = encima.and_then(|x| nombre(x, textos)) {
        let tam = 13.0 * m.e;
        let (w, h) = p.medir_texto(&nombre, tam);
        let (cx, _) = b
            .botones
            .iter()
            .find(|(_, x)| Some(*x) == encima)
            .map(|(c, _)| (c.x + c.ancho / 2.0, c.y))
            .unwrap_or((caja.x, caja.y));
        let alto = h + 10.0 * m.e;
        let y = if caja.y + caja.alto + 6.0 * m.e + alto <= m.alto {
            caja.y + caja.alto + 6.0 * m.e
        } else {
            caja.y - 6.0 * m.e - alto
        };
        let aviso = RectF {
            x: (cx - (w + 16.0 * m.e) / 2.0).clamp(0.0, (m.ancho - w - 16.0 * m.e).max(0.0)),
            y,
            ancho: w + 16.0 * m.e,
            alto,
        };
        p.rellenar_redondeado(aviso, 6.0 * m.e, con_alfa(CRISTAL, 0.97));
        p.texto(
            &nombre,
            aviso.x + 8.0 * m.e,
            aviso.y + 5.0 * m.e,
            tam,
            TEXTO,
        );
    }
}

/// La capa de la hoja `i`, leida si aun no lo estaba (lo elegido puede
/// llegar a una hoja que ya no se ve).
fn asegurar_capa(e: &mut Estado, ruta: &Path, i: usize) {
    if e.capas.contains_key(&i) {
        return;
    }
    let (espacios, alto) = (e.ajustes.espacios, alto_de(&e.hojas, i));
    let capa = e.donde.leer_capa(ruta, i, espacios, alto);
    e.capas.insert(i, capa);
}

/// **Marca lo elegido** con `que` (o, con `None`, quita las marcas que caen
/// en ello o la marca elegida). Cada hoja tocada es un paso de deshacer; si
/// son varias, un solo Ctrl+Z las deshace todas. `true` si cambio algo.
pub(super) fn cambiar(e: &mut Estado, ruta: &Path, que: Option<Marca>) -> bool {
    let por_hoja = elegido_por_hoja(e);
    let de_marca = e.marcar.de_marca.take();
    // Las hojas con algo de la marca elegida (de las leidas).
    let mut hojas: Vec<usize> = por_hoja.iter().map(|(i, _)| *i).collect();
    if let Some(g) = &de_marca {
        hojas.extend(
            e.capas
                .iter()
                .filter(|(_, c)| c.escena.visibles().any(|el| el.grupos.contains(g)))
                .map(|(i, _)| *i),
        );
    }
    hojas.sort_unstable();
    hojas.dedup();
    if hojas.is_empty() {
        return false;
    }
    let grupo = mt::grupo_nuevo(ahora_ms() as i64);
    let rgb = mt::rgb(e.marcar.color);
    let mut cambiadas = Vec::new();
    for i in hojas {
        asegurar_capa(e, ruta, i);
        let alto = alto_de(&e.hojas, i);
        let cajas = por_hoja
            .iter()
            .find(|(k, _)| *k == i)
            .map(|(_, c)| mt::de_fracciones(c, vista::ANCHO_HOJA, alto))
            .unwrap_or_default();
        let Some(capa) = e.capas.get_mut(&i) else {
            continue;
        };
        let antes = capa.escena.pasos_cerrados();
        // Lo que se quita: la marca elegida entera o, con el circulo sobre
        // texto elegido a mano, las marcas que caen en el.
        let quitar: Vec<u64> = match (&de_marca, que) {
            (Some(g), _) => capa
                .escena
                .visibles()
                .filter(|el| el.grupos.contains(g))
                .map(|el| el.id)
                .collect(),
            (None, None) => mt::las_que_tocan(&capa.escena, &cajas),
            (None, Some(_)) => Vec::new(),
        };
        capa.escena.abrir_paso();
        for id in quitar {
            capa.escena.borrar_apuntando(id);
        }
        if let Some(m) = que {
            for el in mt::elementos_en(m, rgb, &cajas, &grupo) {
                capa.escena.anadir(el);
            }
        }
        capa.escena.cerrar_paso();
        if capa.escena.pasos_cerrados() != antes {
            capa.sucia = true;
            cambiadas.push(i);
        }
    }
    if cambiadas.len() > 1 {
        for i in &cambiadas {
            if let Some(c) = e.capas.get_mut(i) {
                c.turno = Some(lector_tinta::turnos::Marcado::de_ahora(&grupo, &c.escena));
            }
        }
    }
    tracing::info!(marca = ?que, hojas = cambiadas.len(), "lector: texto marcado");
    e.hechos.extend(cambiadas.iter().copied());
    if !cambiadas.is_empty() {
        e.deshechos.clear();
    }
    e.ultimo_trazo = ahora_ms();
    poner_seleccion(e, None);
    !cambiadas.is_empty()
}

/// **Un clic sobre una marca la elige entera**: su texto (en todas las
/// hojas por las que pasa) queda elegido y la barrita sale para cambiarla o
/// quitarla. `false` si no habia marca debajo.
pub(super) fn elegir_la_marca(e: &mut Estado, m: Marco) -> bool {
    if e.anotando || seleccion(e).is_some() {
        return false;
    }
    let Some((i, punto)) = en_la_hoja(e, m) else {
        return false;
    };
    let holgura = 3.0 * m.e / px(e, m);
    let Some(grupo) = e.capas.get(&i).and_then(|c| {
        let ids = mt::grupo_en(&c.escena, punto.x, punto.y, holgura)?;
        let el = c.escena.buscar(*ids.first()?)?;
        el.grupos.iter().find(|g| g.starts_with(mt::GRUPO)).cloned()
    }) else {
        return false;
    };
    // El texto de la marca, de la primera hoja a la ultima por las que pasa.
    let mut hojas: Vec<usize> = e
        .capas
        .iter()
        .filter(|(_, c)| c.escena.visibles().any(|el| el.grupos.contains(&grupo)))
        .map(|(k, _)| *k)
        .collect();
    hojas.sort_unstable();
    let mut ancla = None;
    let mut punta = None;
    for k in hojas {
        let alto = alto_de(&e.hojas, k);
        let Some(z) = cajas_del_grupo(e, k, &grupo)
            .into_iter()
            .fold(None, unir)
            .map(|z| {
                [
                    z[0] / vista::ANCHO_HOJA,
                    z[1] / alto,
                    z[2] / vista::ANCHO_HOJA,
                    z[3] / alto,
                ]
            })
        else {
            continue;
        };
        if let Some(Some((a, b))) = con_texto(e, k, |t, _| capa_de_texto::letras_en(&t.cajas, z)) {
            ancla.get_or_insert((k, a));
            punta = Some((k, b));
        }
    }
    if let (Some(ancla), Some(punta)) = (ancla, punta) {
        poner_seleccion(e, Some(Seleccion { ancla, punta }));
    }
    e.marcar.de_marca = Some(grupo);
    true
}

/// Esc: quita lo elegido y la marca elegida. `true` si habia algo.
pub(super) fn soltar_lo_elegido(e: &mut Estado) -> bool {
    let marca = e.marcar.de_marca.take().is_some();
    let office = e.office.as_mut().and_then(|o| o.seleccion.take()).is_some();
    let pdf = e.elegir.seleccion.take().is_some();
    marca || office || pdf
}

/// Un boton de la barrita.
pub(super) fn boton(e: &mut Estado, b: BotonMarcar, ruta: &Path, textos: &Catalogo) {
    match b {
        BotonMarcar::Copiar => {
            if e.office.is_some() {
                office::copiar(e, Some(textos));
            } else {
                elegir::copiar(e, textos);
            }
        }
        BotonMarcar::Color(k) => e.marcar.color = k,
        BotonMarcar::Marcar(m) => {
            cambiar(e, ruta, Some(m));
        }
        BotonMarcar::Quitar => {
            cambiar(e, ruta, None);
        }
    }
}

/// **T: reconocer el texto** de la hoja bajo el raton (o la del centro) con
/// el OCR, aunque el PDF diga que tiene. `false` si no hay que hacer nada
/// (un Word, que siempre tiene su texto).
pub(super) fn reconocer(
    e: &mut Estado,
    tx: &mpsc::Sender<Pedido>,
    textos: &Catalogo,
    m: Marco,
) -> bool {
    if e.office.is_some() || e.anotando || e.hojas.cuantas() == 0 {
        return false;
    }
    let hoja = en_la_hoja(e, m)
        .map(|(i, _)| i)
        .unwrap_or_else(|| e.hojas.en(e.y + m.alto / 2.0 / px(e, m)));
    if tx.send(Pedido::Reconocer { hoja }).is_err() {
        return false;
    }
    e.marcar.reconociendo = Some(hoja);
    e.aviso = Some((textos.t("lector-pdf-reconociendo"), ahora_ms() + 30_000));
    true
}

/// Llego el texto de una hoja: si era la que se pidio con la T, se dice.
pub(super) fn al_llegar_el_texto(
    e: &mut Estado,
    hoja: usize,
    h: &elegir::HojaPdf,
    textos: &Catalogo,
) {
    if e.marcar.reconociendo != Some(hoja) || !h.reconocida {
        return;
    }
    e.marcar.reconociendo = None;
    let clave = if h.texto.texto.trim().is_empty() {
        "lector-pdf-sin-texto"
    } else {
        "lector-pdf-reconocido"
    };
    e.aviso = Some((textos.t(clave), ahora_ms() + 3000));
}

/// **El texto de una hoja por el OCR**, dibujada a [`ANCHO_OCR`]. En el hilo
/// de las hojas.
pub(super) fn reconocer_hoja(doc: &pindesk_pdf::Documento, hoja: usize) -> Option<PaginaDeTexto> {
    let img = doc
        .renderizar(hoja as u32, ANCHO_OCR)
        .map_err(|e| tracing::warn!(?e, hoja, "pdf: hoja que no se pudo dibujar para el OCR"))
        .ok()?;
    let lineas = pindesk_ocr::reconocer(img.ancho, img.alto, &img.pixeles)
        .map_err(|e| tracing::warn!(?e, hoja, "pdf: el OCR de la hoja fallo"))
        .ok()?;
    let renglones: Vec<Vec<capa_de_texto::PalabraLeida>> = lineas
        .iter()
        .map(|l| {
            l.palabras
                .iter()
                .map(|p| {
                    let (x, y) = (p.caja.x as f32, p.caja.y as f32);
                    (
                        [x, y, x + p.caja.ancho as f32, y + p.caja.alto as f32],
                        p.texto.clone(),
                    )
                })
                .collect()
        })
        .collect();
    let (texto, cajas) = capa_de_texto::de_palabras(&renglones, img.ancho as f32, img.alto as f32);
    tracing::info!(
        hoja,
        letras = cajas.len(),
        "pdf: texto de la hoja reconocido"
    );
    Some(PaginaDeTexto { texto, cajas })
}

/// Si una hoja no trae texto que elegir (una foto escaneada).
pub(super) fn sin_texto(t: &PaginaDeTexto) -> bool {
    t.texto.chars().filter(|c| !c.is_whitespace()).count() < 3
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::lector_pdf::pruebas::{estado, marco};
    use std::sync::Arc;

    /// Un renglon de letras de una centesima de ancho a la altura `y`.
    fn hoja(texto: &str, y: f32) -> PaginaDeTexto {
        let mut t = PaginaDeTexto::default();
        for (k, c) in texto.chars().enumerate() {
            t.texto.push(c);
            t.cajas.push(if c == ' ' {
                None
            } else {
                Some([0.1 + k as f32 * 0.01, y, 0.11 + k as f32 * 0.01, y + 0.02])
            });
        }
        t
    }

    fn con_texto_pdf(reconocida: bool) -> (Estado, Marco, PathBuf) {
        let mut e = estado(3);
        let m = marco();
        acotar(&mut e, m);
        let h = elegir::HojaPdf {
            texto: hoja("Hola mundo, adios", 0.1),
            reconocida,
            ..Default::default()
        };
        e.elegir.hojas.insert(0, Arc::new(h));
        e.elegir.hojas.insert(
            1,
            Arc::new(elegir::HojaPdf {
                texto: hoja("otra hoja", 0.5),
                ..Default::default()
            }),
        );
        // Las capas en una carpeta de usar y tirar: marcar escribe ahi.
        let d = std::env::temp_dir().join(format!(
            "pindesk-marcar-{}-{}",
            std::process::id(),
            ahora_ms()
        ));
        let _ = std::fs::create_dir_all(&d);
        for i in 0..3 {
            e.capas.insert(i, Capa::default());
        }
        (e, m, d.join("libro.pdf"))
    }

    #[test]
    fn en_lo_reconocido_se_elige_por_palabras_enteras() {
        let t = hoja("Hola mundo, adios", 0.1);
        let sel = Seleccion {
            ancla: (0, 7),
            punta: (0, 13),
        };
        assert_eq!(rango_en_hoja(sel, 0, &t, false), Some(7..13));
        assert_eq!(rango_en_hoja(sel, 0, &t, true), Some(5..17), "mundo, adios");
        // Caso negativo: una hoja fuera de lo elegido no tiene nada.
        assert_eq!(rango_en_hoja(sel, 1, &t, true), None);
    }

    #[test]
    fn marcar_pone_la_marca_en_la_capa_y_ctrl_z_la_quita() {
        let (mut e, m, ruta) = con_texto_pdf(false);
        e.elegir.seleccion = Some(Seleccion {
            ancla: (0, 5),
            punta: (0, 10),
        });
        assert!(cambiar(&mut e, &ruta, Some(Marca::Subrayar)));
        let capa = &e.capas[&0];
        let marcas: Vec<_> = capa.escena.visibles().filter(|x| mt::es_marca(x)).collect();
        assert_eq!(marcas.len(), 1, "un renglon, una raya");
        // En unidades de la hoja: debajo de «mundo» (x de 0.15 a 0.20).
        let (x0, y0, x1, _) = marcas[0].caja();
        assert!((x0 - 0.15 * vista::ANCHO_HOJA).abs() < 1.0);
        assert!((x1 - 0.20 * vista::ANCHO_HOJA).abs() < 1.0);
        assert!(y0 > 0.12 * e.hojas.altos[0], "debajo de las letras");
        assert!(capa.sucia);
        assert!(e.elegir.seleccion.is_none(), "lo elegido se suelta");
        assert_eq!(e.hechos, vec![0]);
        deshacer(&mut e);
        assert_eq!(e.capas[&0].escena.visibles().count(), 0);
        rehacer(&mut e);
        assert_eq!(e.capas[&0].escena.visibles().count(), 1);
        // Caso negativo: sin nada elegido no se marca nada.
        assert!(!cambiar(&mut e, &ruta, Some(Marca::Tachar)));
        let _ = m;
    }

    #[test]
    fn una_marca_entre_dos_hojas_se_deshace_de_una_vez() {
        let (mut e, _m, ruta) = con_texto_pdf(false);
        e.elegir.seleccion = Some(Seleccion {
            ancla: (0, 11),
            punta: (1, 4),
        });
        assert!(cambiar(&mut e, &ruta, Some(Marca::Resaltar)));
        assert_eq!(e.capas[&0].escena.visibles().count(), 1);
        assert_eq!(e.capas[&1].escena.visibles().count(), 1);
        deshacer(&mut e);
        assert_eq!(e.capas[&0].escena.visibles().count(), 0);
        assert_eq!(e.capas[&1].escena.visibles().count(), 0, "las dos hojas");
    }

    #[test]
    fn un_clic_en_la_marca_la_elige_y_otra_marca_la_cambia() {
        let (mut e, m, ruta) = con_texto_pdf(false);
        e.elegir.seleccion = Some(Seleccion {
            ancla: (0, 5),
            punta: (0, 10),
        });
        cambiar(&mut e, &ruta, Some(Marca::Resaltar));
        // El raton sobre «mundo».
        let s = px(&e, m);
        let alto = e.hojas.altos[0];
        e.raton = (
            (0.175 * vista::ANCHO_HOJA - e.x) * s,
            (0.11 * alto + e.hojas.arriba[0] - e.y) * s,
        );
        assert!(elegir_la_marca(&mut e, m));
        assert!(e.marcar.de_marca.is_some());
        assert_eq!(elegir::texto_elegido(&e), "mundo");
        // Tachar en vez de resaltar: la de antes se va, queda una.
        assert!(cambiar(&mut e, &ruta, Some(Marca::Tachar)));
        let marcas: Vec<_> = e.capas[&0].escena.visibles().collect();
        assert_eq!(marcas.len(), 1);
        assert!(matches!(
            marcas[0].figura,
            pindesk_motor2d::Figura::Linea { .. }
        ));
        // Y el circulo tachado la quita.
        assert!(elegir_la_marca(&mut e, m));
        assert!(cambiar(&mut e, &ruta, None));
        assert_eq!(e.capas[&0].escena.visibles().count(), 0);
        // Caso negativo: en blanco no hay marca que elegir.
        e.raton = (2.0, 2.0);
        assert!(!elegir_la_marca(&mut e, m));
    }

    #[test]
    fn la_barrita_sale_encima_de_lo_elegido_y_no_anotando() {
        let (mut e, m, _) = con_texto_pdf(false);
        e.elegir.seleccion = Some(Seleccion {
            ancla: (0, 0),
            punta: (0, 4),
        });
        let z = zona_en_pantalla(&mut e, m).expect("se ve");
        let h = en_pantalla(&e, 0, m).unwrap();
        let hola = h.caja_a_pantalla([
            0.1 * vista::ANCHO_HOJA,
            0.1 * e.hojas.altos[0],
            0.14 * vista::ANCHO_HOJA,
            0.12 * e.hojas.altos[0],
        ]);
        assert!((z[0] - hola[0]).abs() < 1.0 && (z[3] - hola[3]).abs() < 1.0);
        assert!(a_la_vista(&e));
        e.anotando = true;
        assert!(!a_la_vista(&e), "con el lapiz no hay barrita");
    }
}
