//! **Escuchar un PDF** en el propio lector de hojas.
//!
//! El movil lo hace pasando el PDF a texto y abriendolo en el lector de Word
//! («Escuchar este PDF», `VisorHtmlActivity.abrirPdfComoTexto`), que es
//! quien tiene la voz, y empieza por la hoja del **marcador verde** del PDF.
//! El PC no tiene ese paso a texto, pero si el texto de cada hoja con donde
//! cae cada letra (`pixpin_pdf::texto`, el de buscar): asi que lee aqui
//! mismo, **resaltando en la hoja** lo que suena, y el verde (🟢 en las
//! marcas del PDF, `anot-<uid>.marcas`, las mismas que usa el movil para
//! saber desde que hoja leer) se mueve con lo que se va oyendo.
//!
//! Cada «parrafo» de la voz es un trozo de frases de una hoja
//! (`voz_alta::cortes`): el texto de un PDF llega en renglones sin
//! parrafos, y por frases se resalta un trozo que se puede seguir con la
//! vista.

use std::ops::Range;

use pixpin_docs::voz_alta;

use super::*;
use crate::leer_en_voz::{self, BotonVoz, LeerEnVoz, Suceso};

/// Lo mas largo de cada trozo que se resalta y se dice de una vez.
const TOPE_DEL_TROZO: usize = 320;

pub(super) struct VozDelPdf {
    lector: LeerEnVoz,
    /// De que hoja es cada trozo y que letras de su texto.
    trozos: Vec<(usize, Range<usize>)>,
}

/// Los trozos que se leen de todas las hojas, en orden.
pub(super) fn trozos_de(paginas: &[pixpin_pdf::texto::PaginaDeTexto]) -> Vec<(usize, Range<usize>)> {
    paginas
        .iter()
        .enumerate()
        .flat_map(|(i, p)| voz_alta::cortes(&p.texto, TOPE_DEL_TROZO).into_iter().map(move |r| (i, r)))
        .collect()
}

/// A que altura de su hoja (de 0 a 1) empieza un trozo: la de su primera letra con caja.
fn altura_del_trozo(paginas: &[pixpin_pdf::texto::PaginaDeTexto], t: &(usize, Range<usize>)) -> f32 {
    paginas
        .get(t.0)
        .and_then(|p| p.cajas_de(t.1.start, t.1.end - t.1.start).first().map(|c| c[1]))
        .unwrap_or(0.0)
}

/// El primer trozo en la hoja `pagina` a partir de la altura `dentro` (o el
/// primero de las hojas siguientes).
pub(super) fn trozo_desde(
    paginas: &[pixpin_pdf::texto::PaginaDeTexto],
    trozos: &[(usize, Range<usize>)],
    pagina: usize,
    dentro: f32,
) -> usize {
    trozos
        .iter()
        .position(|t| t.0 > pagina || (t.0 == pagina && altura_del_trozo(paginas, t) >= dentro - 0.01))
        .unwrap_or(0)
}

/// Pide el texto del PDF a su hilo, si aun no se tiene ni se esta leyendo.
pub(super) fn pedir_el_texto(e: &mut Estado, ruta: &Path) {
    if e.hallar.texto.is_some() || e.hallar.leyendo.is_some() {
        return;
    }
    let (tx, rx) = mpsc::channel();
    let r = ruta.to_path_buf();
    let lanzado = std::thread::Builder::new()
        .name("lector-pdf-texto".into())
        .spawn(move || {
            let _ = tx.send(pixpin_pdf::texto::de_fichero(&r));
        });
    match lanzado {
        Ok(_) => e.hallar.leyendo = Some(rx),
        Err(err) => tracing::warn!(?err, "sin hilo para leer el texto del PDF"),
    }
}

fn el_verde(e: &Estado) -> Option<&Marca> {
    e.marcas.iter().find(|m| m.emoji == voz_alta::EMOJI_DE_VOZ)
}

/// **Escuchar** (el boton de la pastilla y la L): se empieza por el marcador
/// verde, y si no hay, por lo que se esta viendo. Con la barra abierta,
/// play/pausa. Si el texto aun no esta, se pide y se empieza al llegar.
pub(super) fn escuchar(e: &mut Estado, textos: &Catalogo, ruta: &Path, m: Marco) {
    if let Some(v) = e.voz.as_mut() {
        v.lector.alternar();
        return;
    }
    let paginas = match &e.hallar.texto {
        Some(Ok(p)) => p,
        Some(Err(_)) => {
            e.escuchar_al_llegar = false;
            e.aviso = Some((textos.t("lector-sin-texto"), ahora_ms() + 3000));
            return;
        }
        None => {
            pedir_el_texto(e, ruta);
            e.escuchar_al_llegar = true;
            e.aviso = Some((textos.t("lector-leyendo-texto"), ahora_ms() + 3000));
            return;
        }
    };
    e.escuchar_al_llegar = false;
    let trozos = trozos_de(paginas);
    if trozos.is_empty() {
        e.aviso = Some((textos.t("lector-sin-texto"), ahora_ms() + 3000));
        return;
    }
    let (pagina, dentro) = match el_verde(e) {
        Some(v) => (marcas::pagina_de(v) as usize, marcas::alto_en_la_pagina(v) as f32),
        None => {
            let s = sitio(e);
            (s.floor().max(0.0) as usize, (s - s.floor()) as f32)
        }
    };
    let desde = trozo_desde(paginas, &trozos, pagina, dentro);
    let letras = |t: &(usize, Range<usize>)| -> String {
        paginas[t.0].texto.chars().skip(t.1.start).take(t.1.end - t.1.start).collect()
    };
    let parrafos: Vec<String> = trozos.iter().map(|t| voz_alta::juntar_blancos(&letras(t))).collect();
    let muestra: String = parrafos.iter().skip(desde).take(40).cloned().collect::<Vec<_>>().join(" ").chars().take(4000).collect();
    let de_la_app = leer_en_voz::idioma_de_la_app(textos);
    let idioma = voz_alta::idioma_para_leer(&muestra, "", &de_la_app);
    let Some(mut lector) = LeerEnVoz::arrancar(&idioma, &de_la_app, parrafos, 1.0) else {
        e.aviso = Some((leer_en_voz::sin_voz(textos, &idioma), ahora_ms() + 6000));
        return;
    };
    lector.leer(desde);
    e.voz = Some(VozDelPdf { lector, trozos });
    e.poniendo_marca = false;
    e.panel = false;
    al_sonar(e, desde, ruta, m, true);
}

/// Una vuelta del bucle: la voz sigue, y si llego el texto que se esperaba
/// para empezar, se empieza. `true` si hay que repintar.
pub(super) fn vuelta(e: &mut Estado, textos: &Catalogo, ruta: &Path, m: Marco) -> bool {
    if e.escuchar_al_llegar && e.hallar.texto.is_some() {
        escuchar(e, textos, ruta, m);
        return true;
    }
    let Some(suceso) = e.voz.as_mut().and_then(|v| v.lector.vuelta()) else {
        return false;
    };
    match suceso {
        Suceso::Parrafo(i) => al_sonar(e, i, ruta, m, false),
        // Acabado: fuera el verde, la proxima vez desde lo que se vea.
        Suceso::Acabado => {
            e.marcas.retain(|x| x.emoji != voz_alta::EMOJI_DE_VOZ);
            guardar_ajustes(e, ruta);
        }
    }
    true
}

/// La hoja y la altura de la hoja (0..1) donde empieza el trozo `i`.
fn donde(e: &Estado, i: usize) -> Option<(usize, f32)> {
    let (Some(v), Some(Ok(paginas))) = (&e.voz, &e.hallar.texto) else {
        return None;
    };
    let t = v.trozos.get(i)?;
    Some((t.0, altura_del_trozo(paginas, t)))
}

/// **Suena otro trozo**: la vista lo sigue si el de antes se veia (anotando
/// no se mueve) y el verde se va a el.
fn al_sonar(e: &mut Estado, i: usize, ruta: &Path, m: Marco, primero: bool) {
    let Some((pagina, alto)) = donde(e, i) else {
        return;
    };
    let y_de = |e: &Estado, (p, a): (usize, f32)| -> Option<f32> {
        Some(e.hojas.arriba.get(p)? + a * e.hojas.altos.get(p)?)
    };
    if let Some(y) = y_de(e, (pagina, alto)) {
        let visto = m.alto / px(e, m);
        let se_veia = primero
            || i.checked_sub(1)
                .and_then(|j| donde(e, j))
                .and_then(|d| y_de(e, d))
                .is_none_or(|ya| ya > e.y && ya < e.y + visto);
        if !e.anotando && se_veia && (y < e.y + visto * 0.08 || y > e.y + visto * 0.85) {
            e.y = y - visto * 0.25;
            acotar(e, m);
        }
    }
    let (x, y) = marcas::en_la_pagina(pagina as i32, f64::from(alto), 0.5);
    let sin_verde: Vec<Marca> = e.marcas.iter().filter(|x| x.emoji != voz_alta::EMOJI_DE_VOZ).cloned().collect();
    e.marcas = marcas::con(&sin_verde, x, y, voz_alta::EMOJI_DE_VOZ, ahora_ms() as i64);
    guardar_ajustes(e, ruta);
}

/// **El verde, aqui** (la ultima celda de la tira): uno solo, se mueve a
/// donde se esta leyendo. Escuchando, la voz salta ahi ya.
pub(super) fn verde_aqui(e: &mut Estado, ruta: &Path) {
    let s = sitio(e);
    let pagina = s.floor() as i32;
    let (x, y) = marcas::en_la_pagina(pagina, s - pagina as f64, 0.5);
    let sin_verde: Vec<Marca> = e.marcas.iter().filter(|x| x.emoji != voz_alta::EMOJI_DE_VOZ).cloned().collect();
    e.marcas = marcas::con(&sin_verde, x, y, voz_alta::EMOJI_DE_VOZ, ahora_ms() as i64);
    e.poniendo_marca = false;
    guardar_ajustes(e, ruta);
    if let (Some(v), Some(Ok(paginas))) = (e.voz.as_mut(), &e.hallar.texto) {
        let i = trozo_desde(paginas, &v.trozos, pagina.max(0) as usize, (s - s.floor()) as f32);
        if v.lector.leyendo {
            v.lector.leer(i);
        } else {
            let actual = v.lector.actual as isize;
            v.lector.saltar(i as isize - actual);
        }
    }
}

/// Un boton de la barra de escuchar.
pub(super) fn boton(e: &mut Estado, b: BotonVoz, ruta: &Path, m: Marco) {
    let Some(v) = e.voz.as_mut() else {
        return;
    };
    match b {
        BotonVoz::Alternar => v.lector.alternar(),
        BotonVoz::Anterior | BotonVoz::Siguiente => {
            let i = v.lector.saltar(if b == BotonVoz::Anterior { -1 } else { 1 });
            al_sonar(e, i, ruta, m, true);
        }
        BotonVoz::Velocidad => {
            v.lector.otra_velocidad();
        }
        BotonVoz::Cerrar => e.voz = None,
    }
}

/// Lo que suena, en ambar sobre la hoja `i` (con su transformada puesta).
pub(super) fn pintar_lo_que_suena(e: &Estado, p: &Pintor, i: usize) {
    let (Some(v), Some(Ok(paginas)), Some(alto)) = (&e.voz, &e.hallar.texto, e.hojas.altos.get(i)) else {
        return;
    };
    let Some(t) = v.trozos.get(v.lector.actual).filter(|t| t.0 == i) else {
        return;
    };
    let Some(pagina) = paginas.get(i) else {
        return;
    };
    for c in pagina.cajas_de(t.1.start, t.1.end - t.1.start) {
        let r = RectF {
            x: c[0] * vista::ANCHO_HOJA - 3.0,
            y: c[1] * alto - 3.0,
            ancho: (c[2] - c[0]) * vista::ANCHO_HOJA + 6.0,
            alto: (c[3] - c[1]) * alto + 6.0,
        };
        p.rellenar_redondeado(r, 3.0, leer_en_voz::AMBAR_LEYENDO);
    }
}

/// La barra de escuchar, abajo.
pub(super) fn pintar_barra(e: &Estado, p: &Pintor, m: Marco) -> Vec<(RectF, BotonVoz)> {
    match &e.voz {
        Some(v) => leer_en_voz::pintar_barra(p, m.ancho, m.alto, m.e, &v.lector),
        None => Vec::new(),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_pdf::texto::PaginaDeTexto;

    fn hoja(texto: &str, y: f32) -> PaginaDeTexto {
        PaginaDeTexto {
            texto: texto.into(),
            cajas: texto.chars().map(|_| Some([0.1, y, 0.2, y + 0.01])).collect(),
        }
    }

    #[test]
    fn cada_hoja_se_lee_en_trozos_de_frases_y_se_empieza_por_la_del_verde() {
        let paginas = vec![hoja("Una. Dos.", 0.1), hoja("", 0.0), hoja("Tres frases aqui.", 0.5)];
        let t = trozos_de(&paginas);
        assert_eq!(t.len(), 2, "{t:?}");
        assert_eq!(t[0].0, 0);
        assert_eq!(t[1], (2, 0..17));
        // El verde en la hoja 2 (desde 0), arriba: su primer trozo.
        assert_eq!(trozo_desde(&paginas, &t, 2, 0.0), 1);
        // En la hoja vacia: el de la siguiente con texto.
        assert_eq!(trozo_desde(&paginas, &t, 1, 0.3), 1);
        // Mas abajo que todo: vuelta al principio.
        assert_eq!(trozo_desde(&paginas, &t, 9, 0.0), 0);
    }
}
