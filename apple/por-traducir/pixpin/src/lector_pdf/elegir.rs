//! **Elegir texto y seguir enlaces en un PDF**, leyendo (sin anotar).
//!
//! Lo mismo que ya hacia el lector con un Word o un PowerPoint (`office`),
//! ahora con las hojas de un PDF: el texto con la caja de cada letra lo saca
//! `pixpin_pdf::texto` y los enlaces `pixpin_pdf::enlaces`, **de la hoja
//! que hay bajo el raton y solo cuando se pasa por encima** (se pide al
//! hilo de las hojas, detras de lo que se ve): leer un PDF sin tocar el
//! texto no gasta nada.
//!
//! - **Arrastrar sobre el texto** lo elige en orden de lectura (aunque pase
//!   a otra hoja); fuera del texto se arrastra la hoja, como siempre.
//!   Anotando, arrastrar es dibujar: elegir es solo leyendo, asi que no hace
//!   falta un «modo texto» aparte.
//! - **Doble clic**: la palabra. **Triple clic**: el renglon.
//! - **Ctrl+C** copia lo elegido; Esc lo quita.
//! - **Un enlace** pone la mano; el clic (sin arrastrar) salta a su hoja y a
//!   su altura, o abre el navegador si es de la web o del correo.
//!
//! Partes traducidas de pdfcraft (https://github.com/storytold/pdfcraft), MIT OR Apache-2.0, Copyright (c) 2026 ArtCraft Team and the Pdfcraft contributors.
//! (`text.rs`: `word_at` y `line_rects`, la palabra y el renglon de una letra).

use std::ops::Range;
use std::sync::Arc;

use pixpin_pdf::enlaces::{Destino, Enlace};
use pixpin_pdf::texto::PaginaDeTexto;
use pixpin_shell::overlay::FormaCursorWin;

use super::office::{Seleccion, enlace_que_se_abre, letra_cercana, letra_debajo};
use super::*;

/// Dos clics seguidos cuentan como doble si van en este tiempo (el de
/// Windows de fabrica) y a menos de estos pixeles.
const MS_DOBLE_CLIC: u64 = 500;
const PX_DOBLE_CLIC: f32 = 5.0;
/// El color de lo elegido (el azul de elegir de Windows, como en `office`).
const ELEGIDO: Color = Color { r: 0.0, g: 0.47, b: 0.84, a: 0.30 };
/// Cuantas hojas alrededor de la que se ve guardan su texto.
const HOJAS_CON_TEXTO: usize = 8;

/// El texto y los enlaces de una hoja del PDF.
#[derive(Debug, Default)]
pub(super) struct HojaPdf {
    pub(super) texto: PaginaDeTexto,
    pub(super) enlaces: Vec<Enlace>,
}

/// Un clic o un arrastre que empezo sobre texto o sobre un enlace.
#[derive(Debug, Clone)]
pub(super) struct Pulsado {
    desde: (f32, f32),
    movido: bool,
    enlace: Option<Destino>,
    /// Doble o triple clic: lo elegido se queda al soltar.
    fijo: bool,
}

#[derive(Debug, Default)]
pub(super) struct ElegirPdf {
    /// El texto de las hojas que se pidieron (las que tuvieron el raton
    /// encima), mientras esten cerca.
    pub(super) hojas: HashMap<usize, Arc<HojaPdf>>,
    /// La hoja bajo el raton cuyo texto falta: se pide al hilo.
    pub(super) falta: Option<usize>,
    pub(super) seleccion: Option<Seleccion>,
    pulsado: Option<Pulsado>,
    cursor: Option<FormaCursorWin>,
    /// El ultimo clic: cuando, donde y cuantos seguidos lleva.
    clics: (u64, (f32, f32), u8),
}

/// **La palabra de la letra `k`**: las letras y cifras seguidas a su
/// alrededor; una letra suelta (un signo) es ella sola.
pub(super) fn palabra_en(texto: &[char], k: usize) -> Range<usize> {
    if k >= texto.len() {
        return texto.len()..texto.len();
    }
    let de_palabra = |c: char| c.is_alphanumeric() || c == '_';
    if !de_palabra(texto[k]) {
        return k..k + 1;
    }
    let mut a = k;
    while a > 0 && de_palabra(texto[a - 1]) {
        a -= 1;
    }
    let mut b = k + 1;
    while b < texto.len() && de_palabra(texto[b]) {
        b += 1;
    }
    a..b
}

/// **El renglon de la letra `k`**: las letras a su alrededor que caen a su
/// altura, sin pasar de un salto de renglon. Los blancos sin caja (los que
/// se anaden entre palabras) cuentan si los rodean letras del renglon.
pub(super) fn renglon_en(t: &PaginaDeTexto, k: usize) -> Range<usize> {
    let texto: Vec<char> = t.texto.chars().collect();
    let Some(Some(c)) = t.cajas.get(k) else {
        return k.min(texto.len())..(k + 1).min(texto.len());
    };
    let centro = (c[1] + c[3]) / 2.0;
    let en_el_renglon = |i: usize| -> Option<bool> {
        if texto.get(i) == Some(&'\n') {
            return Some(false);
        }
        match t.cajas.get(i)? {
            Some(o) => Some(centro >= o[1] && centro <= o[3]),
            // Un blanco: depende de lo que siga.
            None => None,
        }
    };
    let mut a = k;
    let mut i = k;
    while i > 0 {
        i -= 1;
        match en_el_renglon(i) {
            Some(true) => a = i,
            Some(false) => break,
            None => {}
        }
    }
    let mut b = k + 1;
    let mut i = k + 1;
    while i < texto.len() {
        match en_el_renglon(i) {
            Some(true) => b = i + 1,
            Some(false) => break,
            None => {}
        }
        i += 1;
    }
    a..b
}

/// El enlace bajo un punto de la hoja (en fracciones de ella).
fn enlace_en(h: &HojaPdf, fx: f32, fy: f32) -> Option<&Enlace> {
    h.enlaces
        .iter()
        .find(|l| fx >= l.caja[0] && fx <= l.caja[2] && fy >= l.caja[1] && fy <= l.caja[3])
}

/// Si un enlace se puede seguir desde el lector.
fn se_sigue(d: &Destino) -> bool {
    match d {
        Destino::Hoja { .. } => true,
        Destino::Web(u) => enlace_que_se_abre(u),
    }
}

/// La hoja bajo el raton, su texto (si ya llego) y el punto en fracciones de
/// ella. Apunta en `falta` la hoja cuyo texto no ha llegado.
fn bajo_el_raton(e: &mut Estado, m: Marco) -> Option<(usize, Option<Arc<HojaPdf>>, f32, f32)> {
    let (i, punto) = en_la_hoja(e, m)?;
    let alto = *e.hojas.altos.get(i)?;
    let (fx, fy) = (punto.x / vista::ANCHO_HOJA, punto.y / alto);
    if !(0.0..=1.0).contains(&fx) || !(0.0..=1.0).contains(&fy) {
        return Some((i, None, fx, fy));
    }
    let h = e.elegir.hojas.get(&i).cloned();
    if h.is_none() {
        e.elegir.falta = Some(i);
    }
    Some((i, h, fx, fy))
}

fn es_pdf_leyendo(e: &Estado) -> bool {
    e.office.is_none() && !e.anotando && e.error.is_none()
}

/// Al pulsar leyendo: sobre texto empieza a elegir (con doble clic la
/// palabra, con triple el renglon); sobre un enlace, se apunta para
/// seguirlo al soltar sin moverse. `false` si no hay ni texto ni enlace
/// debajo: entonces el lector arrastra la hoja, como siempre.
pub(super) fn pulsar(e: &mut Estado, m: Marco) -> bool {
    if !es_pdf_leyendo(e) {
        return false;
    }
    let Some((i, Some(h), fx, fy)) = bajo_el_raton(e, m) else {
        e.elegir.seleccion = None;
        return false;
    };
    let enlace = enlace_en(&h, fx, fy).map(|l| l.destino.clone()).filter(se_sigue);
    let sobre_letra = letra_debajo(&h.texto, fx, fy);
    if !sobre_letra && enlace.is_none() {
        e.elegir.seleccion = None;
        return false;
    }
    // Doble y triple clic.
    let ahora = ahora_ms();
    let (antes, donde, cuantos) = e.elegir.clics;
    let seguido = ahora.saturating_sub(antes) <= MS_DOBLE_CLIC
        && (e.raton.0 - donde.0).abs().max((e.raton.1 - donde.1).abs()) <= PX_DOBLE_CLIC * m.e;
    let cuantos = if seguido { (cuantos % 3) + 1 } else { 1 };
    e.elegir.clics = (ahora, e.raton, cuantos);
    let letra = letra_cercana(&h.texto, fx, fy).unwrap_or(0);
    let fijo = sobre_letra && cuantos >= 2;
    e.elegir.seleccion = if !sobre_letra {
        None
    } else if cuantos == 2 {
        let texto: Vec<char> = h.texto.texto.chars().collect();
        // La letra de debajo, no el hueco entre dos: la de la izquierda del cursor.
        let k = debajo(&h.texto, fx, fy).unwrap_or(letra);
        let r = palabra_en(&texto, k);
        Some(Seleccion { ancla: (i, r.start), punta: (i, r.end) })
    } else if cuantos == 3 {
        let k = debajo(&h.texto, fx, fy).unwrap_or(letra);
        let r = renglon_en(&h.texto, k);
        Some(Seleccion { ancla: (i, r.start), punta: (i, r.end) })
    } else {
        Some(Seleccion { ancla: (i, letra), punta: (i, letra) })
    };
    e.elegir.pulsado = Some(Pulsado { desde: e.raton, movido: false, enlace: if fijo { None } else { enlace }, fijo });
    true
}

/// La letra que hay justo debajo de un punto, si hay una.
fn debajo(t: &PaginaDeTexto, x: f32, y: f32) -> Option<usize> {
    t.cajas
        .iter()
        .position(|c| c.is_some_and(|c| x >= c[0] && x <= c[2] && y >= c[1] && y <= c[3]))
}

/// Arrastrando: la punta de lo elegido va al raton (a la hoja que haya
/// debajo, aunque sea otra, si su texto ya llego).
pub(super) fn mover(e: &mut Estado, m: Marco) {
    let raton = e.raton;
    let Some(p) = e.elegir.pulsado.as_mut() else {
        return;
    };
    if !p.movido && (raton.0 - p.desde.0).abs().max((raton.1 - p.desde.1).abs()) > lector::UMBRAL_DE_ARRASTRE {
        p.movido = true;
    }
    if p.fijo || !p.movido {
        return;
    }
    let Some((i, Some(h), fx, fy)) = bajo_el_raton(e, m) else { return };
    let Some(letra) = letra_cercana(&h.texto, fx.clamp(0.0, 1.0), fy.clamp(0.0, 1.0)) else { return };
    if let Some(s) = e.elegir.seleccion.as_mut() {
        s.punta = (i, letra);
    }
}

/// Al soltar: un clic sin moverse en un enlace lo sigue; sin moverse en
/// texto, se quita lo elegido (salvo doble o triple clic).
pub(super) fn soltar(e: &mut Estado, m: Marco) {
    let Some(p) = e.elegir.pulsado.take() else { return };
    if p.fijo {
        return;
    }
    if !p.movido {
        e.elegir.seleccion = None;
        if let Some(d) = p.enlace {
            seguir(e, &d, m);
        }
    } else if e.elegir.seleccion.is_some_and(|s| s.vacia()) {
        e.elegir.seleccion = None;
    }
}

/// **Sigue un enlace**: a su hoja y su altura (un poco por encima, que se
/// vea el titulo), o al navegador.
pub(super) fn seguir(e: &mut Estado, d: &Destino, m: Marco) {
    match d {
        Destino::Hoja { hoja, alto } => {
            let (Some(&arriba), Some(&alto_hoja)) = (e.hojas.arriba.get(*hoja), e.hojas.altos.get(*hoja)) else {
                return;
            };
            let s = px(e, m);
            e.y = arriba + alto.unwrap_or(0.0) * alto_hoja - 24.0 * m.e / s;
            e.x_meta = None;
            acotar(e, m);
        }
        Destino::Web(u) => {
            if !enlace_que_se_abre(u) {
                tracing::info!("pdf: enlace que no se abre desde el lector (no es web ni correo)");
                return;
            }
            if let Err(err) = pixpin_shell::abrir::abrir(Path::new(u.trim())) {
                tracing::warn!(?err, "pdf: no se pudo abrir el enlace");
            }
        }
    }
}

/// La forma del cursor leyendo un PDF: la mano en un enlace, la barra en el
/// texto, la flecha en lo demas. `None` si no cambia.
pub(super) fn cursor(e: &mut Estado, m: Marco) -> Option<FormaCursorWin> {
    if e.office.is_some() {
        return None;
    }
    let forma = if !es_pdf_leyendo(e) {
        FormaCursorWin::Flecha
    } else {
        match bajo_el_raton(e, m) {
            Some((_, Some(h), fx, fy)) => {
                if enlace_en(&h, fx, fy).is_some_and(|l| se_sigue(&l.destino)) {
                    FormaCursorWin::Giro
                } else if letra_debajo(&h.texto, fx, fy) {
                    FormaCursorWin::Texto
                } else {
                    FormaCursorWin::Flecha
                }
            }
            _ => FormaCursorWin::Flecha,
        }
    };
    if e.anotando {
        // La tinta pone el suyo: solo se olvida el nuestro.
        e.elegir.cursor = None;
        return None;
    }
    if e.elegir.cursor == Some(forma) {
        return None;
    }
    e.elegir.cursor = Some(forma);
    Some(forma)
}

/// Lo elegido, como texto: de la ancla a la punta, en orden de lectura, con
/// un salto entre hojas.
pub(super) fn texto_elegido(e: &Estado) -> String {
    let Some(sel) = e.elegir.seleccion else {
        return String::new();
    };
    let (a, b) = if sel.ancla <= sel.punta { (sel.ancla, sel.punta) } else { (sel.punta, sel.ancla) };
    let mut salida = String::new();
    for i in a.0..=b.0 {
        let Some(h) = e.elegir.hojas.get(&i) else { continue };
        let largo = h.texto.texto.chars().count();
        if let Some(r) = sel.rango_en(i, largo) {
            if !salida.is_empty() && !salida.ends_with('\n') {
                salida.push('\n');
            }
            salida.extend(h.texto.texto.chars().skip(r.start).take(r.end - r.start));
        }
    }
    salida
}

/// Ctrl+C: lo elegido al portapapeles. `false` si no habia nada elegido.
pub(super) fn copiar(e: &mut Estado, textos: &Catalogo) -> bool {
    let texto = texto_elegido(e);
    if texto.is_empty() {
        return false;
    }
    match pixpin_codec::portapapeles::copiar_texto(&texto) {
        Ok(()) => e.aviso = Some((textos.t("lector-copiado"), ahora_ms() + 1500)),
        Err(err) => tracing::warn!(?err, "pdf: no se pudo copiar el texto elegido"),
    }
    true
}

/// Lo elegido en la hoja `i`, ya con la transformada de la hoja puesta.
pub(super) fn pintar_seleccion(e: &Estado, p: &Pintor, i: usize) {
    let Some(sel) = e.elegir.seleccion else { return };
    if sel.vacia() {
        return;
    }
    let (Some(h), Some(&alto)) = (e.elegir.hojas.get(&i), e.hojas.altos.get(i)) else { return };
    let Some(r) = sel.rango_en(i, h.texto.cajas.len()) else { return };
    for c in h.texto.cajas_de(r.start, r.end - r.start) {
        p.rellenar(
            RectF {
                x: c[0] * vista::ANCHO_HOJA,
                y: c[1] * alto,
                ancho: (c[2] - c[0]) * vista::ANCHO_HOJA,
                alto: (c[3] - c[1]) * alto,
            },
            ELEGIDO,
        );
    }
}

/// Suelta el texto de las hojas lejanas (salvo las de lo elegido).
pub(super) fn olvidar_lejos(e: &mut Estado, vis: Range<usize>) {
    let cerca = vis.start.saturating_sub(HOJAS_CON_TEXTO)..vis.end + HOJAS_CON_TEXTO;
    let elegidas = e.elegir.seleccion.map(|s| {
        let (a, b) = if s.ancla <= s.punta { (s.ancla.0, s.punta.0) } else { (s.punta.0, s.ancla.0) };
        a..b + 1
    });
    e.elegir
        .hojas
        .retain(|i, _| cerca.contains(i) || elegidas.as_ref().is_some_and(|r| r.contains(i)));
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::lector_pdf::pruebas::{estado, marco};

    /// Un renglon por cada texto, a la altura que se diga, letra a letra.
    fn hoja(renglones: &[(&str, f32)]) -> PaginaDeTexto {
        let mut t = PaginaDeTexto::default();
        for (n, (r, y)) in renglones.iter().enumerate() {
            if n > 0 {
                t.texto.push('\n');
                t.cajas.push(None);
            }
            for (k, c) in r.chars().enumerate() {
                t.texto.push(c);
                t.cajas.push(if c == ' ' {
                    None
                } else {
                    Some([0.1 + k as f32 * 0.01, *y, 0.11 + k as f32 * 0.01, y + 0.02])
                });
            }
        }
        t
    }

    #[test]
    fn la_palabra_y_el_renglon_de_una_letra() {
        let t = hoja(&[("Hola mundo, adios", 0.1), ("otra linea", 0.2)]);
        let c: Vec<char> = t.texto.chars().collect();
        assert_eq!(palabra_en(&c, 6), 5..10, "mundo");
        assert_eq!(palabra_en(&c, 10), 10..11, "la coma sola");
        assert_eq!(palabra_en(&c, 0), 0..4, "Hola, al principio");
        // Caso negativo: fuera del texto no hay palabra.
        assert_eq!(palabra_en(&c, 99), c.len()..c.len());
        let r = renglon_en(&t, 6);
        assert_eq!(&t.texto[r.clone()], "Hola mundo, adios");
        let r = renglon_en(&t, 20);
        assert_eq!(t.texto.chars().skip(r.start).take(r.len()).collect::<String>(), "otra linea");
    }

    /// Un estado con una hoja de texto y un enlace cargados, y el raton
    /// sobre la letra `k` del primer renglon.
    fn con_texto() -> (Estado, Marco) {
        let mut e = estado(6);
        let m = marco();
        acotar(&mut e, m);
        let mut h = HojaPdf {
            texto: hoja(&[("Hola mundo, adios", 0.1), ("otra linea", 0.2)]),
            enlaces: Vec::new(),
        };
        h.enlaces.push(Enlace {
            caja: [0.1, 0.5, 0.3, 0.55],
            destino: Destino::Hoja { hoja: 2, alto: Some(0.5) },
        });
        e.elegir.hojas.insert(0, Arc::new(h));
        (e, m)
    }

    /// Lleva el raton a un punto de la hoja 0 en fracciones de ella.
    fn raton_en(e: &mut Estado, m: Marco, fx: f32, fy: f32) {
        let s = px(e, m);
        let alto = e.hojas.altos[0];
        e.raton = ((fx * vista::ANCHO_HOJA - e.x) * s, (fy * alto + e.hojas.arriba[0] - e.y) * s);
    }

    #[test]
    fn arrastrar_elige_doble_clic_la_palabra_triple_el_renglon_y_se_copia() {
        let (mut e, m) = con_texto();
        // Arrastrar de la «m» de mundo a la «d» de adios.
        raton_en(&mut e, m, 0.155, 0.11);
        assert!(pulsar(&mut e, m));
        raton_en(&mut e, m, 0.228, 0.11);
        mover(&mut e, m);
        soltar(&mut e, m);
        assert_eq!(texto_elegido(&e), "mundo, a");
        // Doble clic en «adios»: la palabra.
        e.elegir.clics = (0, (0.0, 0.0), 0);
        raton_en(&mut e, m, 0.245, 0.11);
        assert!(pulsar(&mut e, m));
        soltar(&mut e, m);
        assert!(pulsar(&mut e, m));
        soltar(&mut e, m);
        assert_eq!(texto_elegido(&e), "adios");
        // Y el tercero: el renglon.
        assert!(pulsar(&mut e, m));
        soltar(&mut e, m);
        assert_eq!(texto_elegido(&e), "Hola mundo, adios");
        // Caso negativo: un clic suelto en el texto lo quita todo.
        e.elegir.clics = (0, (0.0, 0.0), 0);
        assert!(pulsar(&mut e, m));
        soltar(&mut e, m);
        assert_eq!(texto_elegido(&e), "");
    }

    #[test]
    fn fuera_del_texto_se_arrastra_la_hoja_y_anotando_no_se_elige() {
        let (mut e, m) = con_texto();
        raton_en(&mut e, m, 0.8, 0.8);
        assert!(!pulsar(&mut e, m), "en blanco se arrastra la hoja");
        raton_en(&mut e, m, 0.155, 0.11);
        e.anotando = true;
        assert!(!pulsar(&mut e, m), "anotando, arrastrar es dibujar");
        e.anotando = false;
        // Sin el texto de la hoja aun: se pide y se arrastra.
        e.elegir.hojas.clear();
        assert!(!pulsar(&mut e, m));
        assert_eq!(e.elegir.falta, Some(0));
    }

    #[test]
    fn el_clic_en_un_enlace_lleva_a_su_hoja_y_pone_la_mano() {
        let (mut e, m) = con_texto();
        raton_en(&mut e, m, 0.2, 0.52);
        assert_eq!(cursor(&mut e, m), Some(FormaCursorWin::Giro));
        assert_eq!(cursor(&mut e, m), None, "la misma forma no se vuelve a poner");
        assert!(pulsar(&mut e, m));
        soltar(&mut e, m);
        let s = px(&e, m);
        let meta = e.hojas.arriba[2] + 0.5 * e.hojas.altos[2];
        assert!((e.y - (meta - 24.0 / s)).abs() < 1.0, "a la mitad de la hoja 3: {} {}", e.y, meta);
        // Caso negativo: arrastrar desde un enlace no lo sigue.
        let (mut e, m) = con_texto();
        raton_en(&mut e, m, 0.2, 0.52);
        let y0 = e.y;
        assert!(pulsar(&mut e, m));
        raton_en(&mut e, m, 0.3, 0.7);
        mover(&mut e, m);
        soltar(&mut e, m);
        assert_eq!(e.y, y0);
    }

    #[test]
    fn solo_se_abre_la_web_y_el_correo() {
        assert!(se_sigue(&Destino::Web("https://example.org".into())));
        assert!(se_sigue(&Destino::Web("mailto:a@b.c".into())));
        assert!(!se_sigue(&Destino::Web("file:///C:/Windows/calc.exe".into())));
        assert!(!se_sigue(&Destino::Web("javascript:alert(1)".into())));
    }
}
