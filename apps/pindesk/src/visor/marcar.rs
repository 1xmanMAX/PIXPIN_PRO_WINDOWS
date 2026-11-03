//! **Elegir y marcar el texto** en el lector de Word, libros y paginas
//! (1-nov-2026): la misma herramienta que el lector de PDF
//! (`lector_pdf::marcar`) y que el texto reconocido de los pines.
//!
//! - **Elegir**: arrastrar sobre el texto (fuera de el se arrastra el
//!   documento, como siempre), doble clic la palabra, triple el renglon,
//!   Ctrl+C copia y Esc lo suelta. Solo leyendo: con el lapiz, arrastrar es
//!   dibujar.
//! - **La capa de texto** es la caja de cada letra preguntada a los MISMOS
//!   renglones que se pintan (`Pintor::letras_de_lectura`), en unidades del
//!   documento; se calcula al pintar para los parrafos que se ven y se
//!   olvida al volver a medir (otra columna u otra letra).
//! - **La barrita** (`lector::pintar_barrita_de_marcar`) sale encima de lo
//!   elegido; un clic en una marca la elige entera para cambiarla o
//!   quitarla; Ctrl+Z la deshace.
//!
//! **Donde van las marcas**: como la tinta, en la capa del documento
//! (`capa.excalidraw` junto a el, o la `anot-<uid>` de un adjunto), con el
//! grupo `marca-texto-<ms>` (`pindesk_motor2d::marcas_texto`). Un documento
//! que se reajusta al ancho no deja que una caja siga encima de su palabra,
//! asi que **la primera marca fija la columna y la letra**, igual que la
//! primera vez que se anota (`alternar_anotar`): desde ahi el texto ya no se
//! vuelve a partir, acercar es solo aumento y el movil coloca las mismas
//! palabras en el mismo sitio (`maqueta_movil`), asi que la marca cae en la
//! misma palabra en los dos aparatos.

use std::collections::HashMap;
use std::ops::Range;

use pindesk_motor2d::marcas_texto::{self as mt, Marca};
use pindesk_pdf::texto::PaginaDeTexto;
use pindesk_shell::overlay::FormaCursorWin;
use pindesk_ui::barra_marcar::BotonMarcar;

use crate::lector_pdf::elegir::{palabra_en, renglon_en};
use crate::lector_pdf::office::{Seleccion, letra_cercana, letra_debajo};

use super::*;

/// Dos clics seguidos cuentan como doble en este tiempo y a menos de estos
/// pixeles (los de `lector_pdf::elegir`).
const MS_DOBLE_CLIC: u64 = 500;
const PX_DOBLE_CLIC: f32 = 5.0;
/// El azul de elegir de Windows, como en el lector de PDF.
const ELEGIDO: Color = Color {
    r: 0.0,
    g: 0.47,
    b: 0.84,
    a: 0.30,
};

/// Un arrastre que empezo sobre texto.
#[derive(Debug, Clone, Copy)]
struct Pulsado {
    desde: (f32, f32),
    movido: bool,
    /// Doble o triple clic: lo elegido se queda al soltar.
    fijo: bool,
}

/// Lo de elegir y marcar del visor.
#[derive(Debug)]
pub(super) struct MarcarVisor {
    /// La capa de texto de cada parrafo colocado (por su indice en
    /// `colocados`), con las cajas en unidades del documento.
    textos: HashMap<usize, PaginaDeTexto>,
    /// Con que medida se calcularon (`Estado::medido_con`).
    medido: Option<(u32, u32, u8, u8)>,
    /// Lo elegido: (parrafo, letra) de la ancla a la punta.
    pub(super) seleccion: Option<Seleccion>,
    pulsado: Option<Pulsado>,
    clics: (u64, (f32, f32), u8),
    /// El color elegido (`marcas_texto::COLORES`).
    pub(super) color: u8,
    /// La marca elegida con un clic encima (su grupo).
    pub(super) de_marca: Option<String>,
    /// **Una marca que espera a que se vuelva a medir**: la primera fija la
    /// columna y el texto puede partirse en otros renglones, asi que se
    /// apunta POR LETRAS (parrafo y letra, el ancla) y se pone al pintar,
    /// con las cajas de la medida nueva.
    pendiente: Option<Pendiente>,
}

#[derive(Debug, Clone)]
struct Pendiente {
    seleccion: Option<Seleccion>,
    de_marca: Option<String>,
    que: Option<Marca>,
}

impl Default for MarcarVisor {
    fn default() -> Self {
        Self {
            textos: HashMap::new(),
            medido: None,
            seleccion: None,
            pulsado: None,
            clics: (0, (0.0, 0.0), 0),
            color: mt::COLOR_INICIAL,
            de_marca: None,
            pendiente: None,
        }
    }
}

/// **La capa de texto de un parrafo**: cada letra (no cada unidad UTF-16)
/// con su caja, en unidades del documento.
fn texto_del_colocado(p: &Pintor, c: &Colocado) -> PaginaDeTexto {
    let cajas16 = p.letras_de_lectura(
        &c.texto,
        c.tam,
        c.ancho,
        &c.tramos,
        &c.letra.para_pintar(),
        &c.renglones,
    );
    let mut t = PaginaDeTexto::default();
    let mut u = 0usize;
    for ch in c.texto.chars() {
        t.texto.push(ch);
        t.cajas
            .push(cajas16.get(u).copied().flatten().and_then(|r| {
                (r.ancho > 0.0 && !ch.is_whitespace()).then_some([
                    r.x + c.sangria,
                    r.y + c.y,
                    r.x + r.ancho + c.sangria,
                    r.y + r.alto + c.y,
                ])
            }));
        u += ch.len_utf16();
    }
    t
}

/// Si un parrafo tiene texto que elegir (no una raya, una imagen o una
/// celda vacia).
fn de_texto(c: &Colocado) -> bool {
    !c.texto.is_empty() && !c.recuadro && c.clase != Clase::Regla && c.clase != Clase::Capitulo
}

/// **Al pintar**: la capa de texto de los parrafos que se ven (los demas
/// no se calculan hasta que se vean). Se olvida todo al volver a medir.
pub(super) fn preparar(e: &mut Estado, p: &Pintor, vista: (f32, f32, f32, f32)) {
    if e.marcar.medido != e.medido_con {
        e.marcar.textos.clear();
        e.marcar.seleccion = None;
        e.marcar.medido = e.medido_con;
    }
    if e.anotando || e.fotografiando {
        return;
    }
    let desde = e.colocados.partition_point(|c| c.y + c.alto < vista.1);
    for i in desde..e.colocados.len() {
        let c = &e.colocados[i];
        if c.y > vista.3 {
            break;
        }
        if de_texto(c) && !e.marcar.textos.contains_key(&i) {
            let t = texto_del_colocado(p, c);
            e.marcar.textos.insert(i, t);
        }
    }
    // La marca que esperaba a la medida nueva: sus letras, aunque ya no se
    // vean, y a la capa.
    if let Some(pend) = e.marcar.pendiente.take() {
        if let Some(sel) = pend.seleccion {
            for i in parrafos_de(sel) {
                if let Some(c) = e.colocados.get(i)
                    && de_texto(c)
                    && !e.marcar.textos.contains_key(&i)
                {
                    let t = texto_del_colocado(p, c);
                    e.marcar.textos.insert(i, t);
                }
            }
        }
        e.marcar.seleccion = pend.seleccion;
        e.marcar.de_marca = pend.de_marca;
        aplicar(e, pend.que);
    }
}

/// El parrafo a la altura `y` (en unidades del documento), si su texto ya
/// se calculo: el que la contiene o, si cae entre dos, el mas cercano en
/// vertical de los que se ven.
fn parrafo_en(e: &Estado, y: f32) -> Option<usize> {
    e.marcar
        .textos
        .keys()
        .copied()
        .filter_map(|i| {
            let c = e.colocados.get(i)?;
            let d = if y < c.y {
                c.y - y
            } else if y > c.y + c.alto {
                y - c.y - c.alto
            } else {
                0.0
            };
            Some((d, i))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, i)| i)
}

/// La letra justo debajo de un punto, si hay una.
fn debajo(t: &PaginaDeTexto, x: f32, y: f32) -> Option<usize> {
    t.cajas
        .iter()
        .position(|c| c.is_some_and(|c| x >= c[0] && x <= c[2] && y >= c[1] && y <= c[3]))
}

/// Si se puede elegir ahora.
fn se_elige(e: &Estado) -> bool {
    !e.anotando && !e.fotografiando && !e.panel && !e.viendo_indice && !e.poniendo_marca
}

/// **Al pulsar leyendo**: sobre texto empieza a elegir (con doble clic la
/// palabra, con triple el renglon). `false` si no hay texto debajo: el
/// visor arrastra el documento, como siempre.
pub(super) fn pulsar(e: &mut Estado, m: Marco) -> bool {
    e.marcar.de_marca = None;
    if !se_elige(e) {
        return false;
    }
    let q = en_el_documento(e, m);
    let Some(i) = e
        .marcar
        .textos
        .iter()
        .find(|(_, t)| letra_debajo(t, q.x, q.y))
        .map(|(i, _)| *i)
    else {
        e.marcar.seleccion = None;
        return false;
    };
    let t = &e.marcar.textos[&i];
    let ahora = ahora_ms();
    let (antes, donde, cuantos) = e.marcar.clics;
    let seguido = ahora.saturating_sub(antes) <= MS_DOBLE_CLIC
        && (e.raton.0 - donde.0).abs().max((e.raton.1 - donde.1).abs()) <= PX_DOBLE_CLIC * m.e;
    let cuantos = if seguido { (cuantos % 3) + 1 } else { 1 };
    e.marcar.clics = (ahora, e.raton, cuantos);
    let letra = letra_cercana(t, q.x, q.y).unwrap_or(0);
    let k = debajo(t, q.x, q.y).unwrap_or(letra);
    let rango = match cuantos {
        2 => {
            let letras: Vec<char> = t.texto.chars().collect();
            palabra_en(&letras, k)
        }
        3 => renglon_en(t, k),
        _ => letra..letra,
    };
    e.marcar.seleccion = Some(Seleccion {
        ancla: (i, rango.start),
        punta: (i, rango.end),
    });
    e.marcar.pulsado = Some(Pulsado {
        desde: e.raton,
        movido: false,
        fijo: cuantos >= 2,
    });
    true
}

/// Arrastrando: la punta va a la letra mas cercana al raton (en el parrafo
/// que haya debajo, aunque sea otro).
pub(super) fn mover(e: &mut Estado, m: Marco) {
    let raton = e.raton;
    let Some(p) = e.marcar.pulsado.as_mut() else {
        return;
    };
    if !p.movido
        && (raton.0 - p.desde.0).abs().max((raton.1 - p.desde.1).abs()) > lector::UMBRAL_DE_ARRASTRE
    {
        p.movido = true;
    }
    if p.fijo || !p.movido {
        return;
    }
    let q = en_el_documento(e, m);
    let Some(i) = parrafo_en(e, q.y) else {
        return;
    };
    let Some(letra) = e
        .marcar
        .textos
        .get(&i)
        .and_then(|t| letra_cercana(t, q.x, q.y))
    else {
        return;
    };
    if let Some(s) = e.marcar.seleccion.as_mut() {
        s.punta = (i, letra);
    }
}

/// Al soltar: un clic sin moverse suelta lo elegido (salvo doble o
/// triple). `true` si fue un clic suelto: quien llama mira si cae en una
/// marca o saca la pastilla, como un toque en el papel.
pub(super) fn soltar(e: &mut Estado) -> bool {
    let Some(p) = e.marcar.pulsado.take() else {
        return false;
    };
    if p.fijo {
        return false;
    }
    if !p.movido || e.marcar.seleccion.is_some_and(|s| s.vacia()) {
        e.marcar.seleccion = None;
    }
    !p.movido
}

/// Las letras elegidas del parrafo `i`.
fn rango_de(e: &Estado, i: usize) -> Option<Range<usize>> {
    let sel = e.marcar.seleccion.filter(|s| !s.vacia())?;
    let t = e.marcar.textos.get(&i)?;
    sel.rango_en(i, t.cajas.len())
}

fn parrafos_de(s: Seleccion) -> std::ops::RangeInclusive<usize> {
    s.ancla.0.min(s.punta.0)..=s.ancla.0.max(s.punta.0)
}

/// Lo elegido, como texto: un salto entre parrafos.
pub(super) fn texto_elegido(e: &Estado) -> String {
    let Some(sel) = e.marcar.seleccion else {
        return String::new();
    };
    let mut salida = String::new();
    for i in parrafos_de(sel) {
        let (Some(t), Some(r)) = (e.marcar.textos.get(&i), rango_de(e, i)) else {
            continue;
        };
        if !salida.is_empty() && !salida.ends_with('\n') {
            salida.push('\n');
        }
        salida.extend(t.texto.chars().skip(r.start).take(r.end - r.start));
    }
    salida
}

/// Ctrl+C: lo elegido al portapapeles. `false` si no habia nada.
pub(super) fn copiar(e: &mut Estado, textos: &Catalogo) -> bool {
    let texto = texto_elegido(e);
    if texto.is_empty() {
        return false;
    }
    match pindesk_codec::portapapeles::copiar_texto(&texto) {
        Ok(()) => e.aviso = Some((textos.t("lector-copiado"), ahora_ms() + 1500)),
        Err(err) => tracing::warn!(?err, "visor: no se pudo copiar el texto elegido"),
    }
    true
}

/// Las cajas de lo elegido (en unidades del documento), por renglon.
fn cajas_elegidas(e: &Estado) -> Vec<[f32; 4]> {
    let Some(sel) = e.marcar.seleccion.filter(|s| !s.vacia()) else {
        return Vec::new();
    };
    parrafos_de(sel)
        .filter_map(|i| {
            let r = rango_de(e, i)?;
            Some(e.marcar.textos.get(&i)?.cajas_de(r.start, r.end - r.start))
        })
        .flatten()
        .collect()
}

/// Lo elegido, ya con la transformada del documento puesta.
pub(super) fn pintar_seleccion(e: &Estado, p: &Pintor) {
    for c in cajas_elegidas(e) {
        p.rellenar(
            RectF {
                x: c[0],
                y: c[1],
                ancho: c[2] - c[0],
                alto: c[3] - c[1],
            },
            ELEGIDO,
        );
    }
}

/// Si el raton esta sobre texto que se puede elegir (para poner la barra).
pub(super) fn sobre_texto(e: &Estado, m: Marco) -> bool {
    if !se_elige(e) {
        return false;
    }
    let q = en_el_documento(e, m);
    e.marcar.textos.values().any(|t| letra_debajo(t, q.x, q.y))
}

/// El cursor de texto, para `cursor_leyendo`.
pub(super) const CURSOR_DE_TEXTO: FormaCursorWin = FormaCursorWin::Texto;

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

/// Las cajas de lo de la marca `g`, en unidades del documento.
fn cajas_del_grupo(e: &Estado, g: &str) -> Vec<[f32; 4]> {
    e.capa
        .escena
        .visibles()
        .filter(|el| el.grupos.iter().any(|x| x == g))
        .map(|el| {
            let (x0, y0, x1, y1) = el.caja();
            [x0, y0, x1, y1]
        })
        .collect()
}

/// **Donde esta lo elegido en la ventana**, en pixeles.
fn zona_en_pantalla(e: &Estado, m: Marco) -> Option<[f32; 4]> {
    let s = px(e, m);
    let a_pantalla = |c: [f32; 4]| {
        [
            (c[0] - e.x) * s,
            (c[1] - e.y) * s,
            (c[2] - e.x) * s,
            (c[3] - e.y) * s,
        ]
    };
    let mut zona = cajas_elegidas(e)
        .into_iter()
        .map(a_pantalla)
        .fold(None, unir);
    if let Some(g) = &e.marcar.de_marca {
        zona = cajas_del_grupo(e, g)
            .into_iter()
            .map(a_pantalla)
            .fold(zona, unir);
    }
    let z = zona?;
    let z = [
        z[0].max(0.0),
        z[1].max(0.0),
        z[2].min(m.ancho),
        z[3].min(m.alto),
    ];
    (z[2] > z[0] && z[3] > z[1]).then_some(z)
}

fn a_la_vista(e: &Estado) -> bool {
    se_elige(e)
        && e.nombre.editando.is_none()
        && (e.marcar.seleccion.is_some_and(|s| !s.vacia()) || e.marcar.de_marca.is_some())
}

/// **Pinta la barrita** junto a lo elegido y apunta sus botones.
pub(super) fn pintar_barrita(
    e: &Estado,
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
    let (caja, de_la_barrita) = lector::pintar_barrita_de_marcar(
        p,
        zona,
        (m.ancho, m.alto),
        m.e,
        e.marcar.color,
        e.raton,
        textos,
    );
    botones.push((caja, Accion::FondoBarrita));
    botones.extend(
        de_la_barrita
            .into_iter()
            .map(|(r, b)| (r, Accion::Marcar(b))),
    );
}

/// **Marca lo elegido** con `que` (o, con `None`, quita las marcas que caen
/// en ello o la marca elegida), en un paso de deshacer. La primera marca
/// fija la columna y la letra, como anotar: entonces se pone al pintar, con
/// la medida nueva (`preparar`). `true` si hay algo que hacer.
pub(super) fn cambiar(e: &mut Estado, ruta: &Path, que: Option<Marca>, m: Marco) -> bool {
    if e.marcar.seleccion.is_none_or(|s| s.vacia()) && e.marcar.de_marca.is_none() {
        return false;
    }
    if !e.ajustes.letra_fijada() {
        e.ajustes.columna = columna_que_cabe(m).round() as u32;
        guardar_ajustes(e, ruta);
        e.marcar.pendiente = Some(Pendiente {
            seleccion: e.marcar.seleccion.take(),
            de_marca: e.marcar.de_marca.take(),
            que,
        });
        return true;
    }
    aplicar(e, que)
}

/// Lo que hace [`cambiar`] en la capa, con las cajas de la medida de ahora.
fn aplicar(e: &mut Estado, que: Option<Marca>) -> bool {
    let cajas = cajas_elegidas(e);
    let de_marca = e.marcar.de_marca.take();
    let quitar: Vec<u64> = match (&de_marca, que) {
        (Some(g), _) => e
            .capa
            .escena
            .visibles()
            .filter(|el| el.grupos.contains(g))
            .map(|el| el.id)
            .collect(),
        (None, None) => mt::las_que_tocan(&e.capa.escena, &cajas),
        (None, Some(_)) => Vec::new(),
    };
    let nuevos = match que {
        Some(mk) => mt::elementos_en(
            mk,
            mt::rgb(e.marcar.color),
            &cajas,
            &mt::grupo_nuevo(ahora_ms() as i64),
        ),
        None => Vec::new(),
    };
    e.marcar.seleccion = None;
    if quitar.is_empty() && nuevos.is_empty() {
        return false;
    }
    let escena = &mut e.capa.escena;
    escena.abrir_paso();
    for id in quitar {
        escena.borrar_apuntando(id);
    }
    for el in nuevos {
        escena.anadir(el);
    }
    escena.cerrar_paso();
    e.capa.sucia = true;
    e.ultimo_trazo = ahora_ms();
    tracing::info!(marca = ?que, "visor: texto marcado");
    true
}

/// **Un clic sobre una marca la elige entera**. `false` si no habia.
pub(super) fn elegir_la_marca(e: &mut Estado, m: Marco) -> bool {
    if !se_elige(e) || e.marcar.seleccion.is_some_and(|s| !s.vacia()) {
        return false;
    }
    let q = en_el_documento(e, m);
    let holgura = 3.0 * m.e / px(e, m);
    let Some(grupo) = mt::grupo_en(&e.capa.escena, q.x, q.y, holgura).and_then(|ids| {
        let el = e.capa.escena.buscar(*ids.first()?)?;
        el.grupos.iter().find(|g| g.starts_with(mt::GRUPO)).cloned()
    }) else {
        return false;
    };
    // Su texto: de la primera letra a la ultima que tapa, en los parrafos
    // que se ven.
    if let Some(z) = cajas_del_grupo(e, &grupo).into_iter().fold(None, unir) {
        let mut claves: Vec<usize> = e.marcar.textos.keys().copied().collect();
        claves.sort_unstable();
        let (mut ancla, mut punta) = (None, None);
        for i in claves {
            if let Some((a, b)) =
                pindesk_docs::capa_de_texto::letras_en(&e.marcar.textos[&i].cajas, z)
            {
                ancla.get_or_insert((i, a));
                punta = Some((i, b));
            }
        }
        if let (Some(ancla), Some(punta)) = (ancla, punta) {
            e.marcar.seleccion = Some(Seleccion { ancla, punta });
        }
    }
    e.marcar.de_marca = Some(grupo);
    true
}

/// Esc: suelta lo elegido y la marca elegida. `true` si habia algo.
pub(super) fn soltar_lo_elegido(e: &mut Estado) -> bool {
    let marca = e.marcar.de_marca.take().is_some();
    let sel = e.marcar.seleccion.take().is_some();
    marca || sel
}

/// Un boton de la barrita.
pub(super) fn boton(e: &mut Estado, b: BotonMarcar, ruta: &Path, textos: &Catalogo, m: Marco) {
    match b {
        BotonMarcar::Copiar => {
            copiar(e, textos);
        }
        BotonMarcar::Color(k) => e.marcar.color = k,
        BotonMarcar::Marcar(mk) => {
            cambiar(e, ruta, Some(mk), m);
        }
        BotonMarcar::Quitar => {
            cambiar(e, ruta, None, m);
        }
    }
}

/// La caja de la letra `k` (en letras, no en UTF-16) del parrafo `i`, si su
/// capa de texto ya se calculo (lo que se ve): donde va una pegatina de
/// comentario (`comentar`).
pub(super) fn caja_de_letra(e: &Estado, i: usize, k: usize) -> Option<[f32; 4]> {
    e.marcar.textos.get(&i)?.cajas.get(k).copied().flatten()
}

/// La letra del parrafo `i` bajo un punto del documento, si hay una.
pub(super) fn letra_en(e: &Estado, i: usize, x: f32, y: f32) -> Option<usize> {
    debajo(e.marcar.textos.get(&i)?, x, y)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un parrafo de letras de diez unidades de ancho a la altura `y`.
    fn parrafo(texto: &str, y: f32) -> PaginaDeTexto {
        let mut t = PaginaDeTexto::default();
        for (k, c) in texto.chars().enumerate() {
            t.texto.push(c);
            t.cajas.push((c != ' ').then(|| {
                let x = 10.0 + k as f32 * 10.0;
                [x, y, x + 10.0, y + 20.0]
            }));
        }
        t
    }

    #[test]
    fn lo_elegido_entre_dos_parrafos_se_copia_y_se_marca_por_renglones() {
        let mut m = MarcarVisor::default();
        m.textos.insert(0, parrafo("uno dos tres", 0.0));
        m.textos.insert(1, parrafo("cuatro cinco", 40.0));
        m.seleccion = Some(Seleccion {
            ancla: (0, 4),
            punta: (1, 6),
        });
        let r0 = m.seleccion.unwrap().rango_en(0, m.textos[&0].cajas.len());
        assert_eq!(r0, Some(4..12));
        // Las cajas: una por renglon, «dos tres» y «cuatro».
        let c0 = m.textos[&0].cajas_de(4, 8);
        assert_eq!(c0.len(), 1);
        assert!((c0[0][0] - 50.0).abs() < 1e-3 && (c0[0][2] - 130.0).abs() < 1e-3);
        let e = mt::elementos_en(Marca::Resaltar, mt::rgb(0), &c0, "marca-texto-1");
        assert_eq!(e.len(), 1);
        // Caso negativo: nada elegido no tiene cajas.
        m.seleccion = Some(Seleccion {
            ancla: (0, 3),
            punta: (0, 3),
        });
        assert!(m.seleccion.unwrap().vacia());
    }
}
