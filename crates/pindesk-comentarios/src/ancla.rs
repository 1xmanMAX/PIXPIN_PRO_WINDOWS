//! **Donde esta pegada una pegatina**, en las unidades de cada visor.
//!
//! Tres clases de sitio, porque los documentos se mueven de tres maneras:
//!
//! - **`pagina`**: un punto de una hoja que no se recoloca. `pagina` desde 0
//!   y `x`, `y` en las unidades de esa hoja, con y hacia abajo desde su
//!   esquina de arriba a la izquierda:
//!   - PDF y Office paginado (Word, PowerPoint, el PDF que sale de ellos):
//!     puntos de la pagina (1/72 de pulgada), los de su `MediaBox` girado
//!     como se ve;
//!   - hoja de calculo: pixeles de la hoja al 100 % (96 ppp), con `pagina` =
//!     hoja del libro (desde 0), como los marcos;
//!   - imagen (adjunto o pin): pixeles de la imagen guardada, sin girar,
//!     `pagina` 0;
//!   - plano CAD: unidades del dibujo (y hacia **arriba**, las del plano),
//!     `pagina` 0.
//! - **`escena`**: un punto del lienzo, en coordenadas del mundo (las de los
//!   elementos Excalidraw, y hacia abajo).
//! - **`texto`**: un sitio de un texto que se recoloca con el ancho (Word en
//!   modo lectura, libros, Markdown, HTML): la cita, como las marcas de texto
//!   (`TextQuoteSelector` y `TextPositionSelector` de la W3C, posiciones en
//!   unidades UTF-16). La pegatina va al principio de la cita. `sobre` dice
//!   en que texto se cuentan: `markdown`, `html` o `documento` (el texto del
//!   lector de Word y libros, parrafos unidos con `\n`).
//!
//! Lo que no se conoce se conserva al reescribir (`resto`).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const PAGINA: &str = "pagina";
pub const ESCENA: &str = "escena";
pub const TEXTO: &str = "texto";

/// En que texto se cuentan las posiciones de un ancla de texto.
pub const SOBRE_MARKDOWN: &str = "markdown";
pub const SOBRE_HTML: &str = "html";
pub const SOBRE_DOCUMENTO: &str = "documento";

/// La cita (`TextQuoteSelector`), el mismo JSON que las marcas de texto.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Cita {
    #[serde(rename = "type")]
    pub tipo: String,
    pub exact: String,
    pub prefix: String,
    pub suffix: String,
    #[serde(flatten)]
    pub resto: Map<String, Value>,
}

/// Donde estaba la cita (`TextPositionSelector`): para desempatar.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Posicion {
    #[serde(rename = "type")]
    pub tipo: String,
    pub start: usize,
    pub end: usize,
    #[serde(flatten)]
    pub resto: Map<String, Value>,
}

/// El ancla tal cual va en el fichero.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Ancla {
    /// `pagina`, `escena` o `texto`.
    pub sitio: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagina: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub sobre: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cita: Option<Cita>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posicion: Option<Posicion>,
    #[serde(flatten)]
    pub resto: Map<String, Value>,
}

/// El ancla ya entendida.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sitio<'a> {
    Pagina {
        pagina: u32,
        x: f64,
        y: f64,
    },
    Escena {
        x: f64,
        y: f64,
    },
    Texto {
        sobre: &'a str,
        cita: &'a Cita,
        posicion: Option<&'a Posicion>,
    },
    /// De una version mas nueva, o rota: no se pinta, pero se conserva.
    Desconocido,
}

/// Cuantas unidades de contexto se guardan a cada lado de la cita.
pub const CONTEXTO: usize = 32;

fn redondo(v: f64) -> f64 {
    // A diezmilesimas: el fichero se lee a ojo y no cambia por ruido.
    (v * 10_000.0).round() / 10_000.0
}

impl Ancla {
    pub fn pagina(pagina: u32, x: f64, y: f64) -> Ancla {
        Ancla {
            sitio: PAGINA.into(),
            pagina: Some(pagina),
            x: Some(redondo(x)),
            y: Some(redondo(y)),
            ..Ancla::default()
        }
    }

    pub fn escena(x: f64, y: f64) -> Ancla {
        Ancla {
            sitio: ESCENA.into(),
            x: Some(redondo(x)),
            y: Some(redondo(y)),
            ..Ancla::default()
        }
    }

    /// Un ancla de texto sobre `texto` (unidades UTF-16) desde `desde` hasta
    /// `hasta`. Sin nada citado (un clic sin elegir), se cita la palabra que
    /// hay en `desde`; `None` si el texto esta vacio.
    pub fn texto(sobre: &str, texto: &[u16], desde: usize, hasta: usize) -> Option<Ancla> {
        let n = texto.len();
        let (mut a, mut b) = (desde.min(n), hasta.min(n));
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        if a == b {
            (a, b) = palabra_en(texto, a)?;
        }
        // Sin blancos en los bordes.
        while a < b && es_blanco(texto[a]) {
            a += 1;
        }
        while b > a && es_blanco(texto[b - 1]) {
            b -= 1;
        }
        if a == b {
            return None;
        }
        let ini = sin_partir(texto, a.saturating_sub(CONTEXTO), true);
        let fin = sin_partir(texto, (b + CONTEXTO).min(n), false);
        let s = |r: std::ops::Range<usize>| String::from_utf16_lossy(&texto[r]);
        Some(Ancla {
            sitio: TEXTO.into(),
            sobre: sobre.into(),
            cita: Some(Cita {
                tipo: "TextQuoteSelector".into(),
                exact: s(a..b),
                prefix: s(ini..a),
                suffix: s(b..fin),
                resto: Map::new(),
            }),
            posicion: Some(Posicion {
                tipo: "TextPositionSelector".into(),
                start: a,
                end: b,
                resto: Map::new(),
            }),
            ..Ancla::default()
        })
    }

    pub fn sitio(&self) -> Sitio<'_> {
        match self.sitio.as_str() {
            PAGINA => match (self.x, self.y) {
                (Some(x), Some(y)) if x.is_finite() && y.is_finite() => Sitio::Pagina {
                    pagina: self.pagina.unwrap_or(0),
                    x,
                    y,
                },
                _ => Sitio::Desconocido,
            },
            ESCENA => match (self.x, self.y) {
                (Some(x), Some(y)) if x.is_finite() && y.is_finite() => Sitio::Escena { x, y },
                _ => Sitio::Desconocido,
            },
            TEXTO => match &self.cita {
                Some(c) if !c.exact.is_empty() => Sitio::Texto {
                    sobre: &self.sobre,
                    cita: c,
                    posicion: self.posicion.as_ref(),
                },
                _ => Sitio::Desconocido,
            },
            _ => Sitio::Desconocido,
        }
    }

    /// Lo mismo, movido a otro punto del mismo tipo de sitio (arrastrar la
    /// pegatina): conserva lo que no se conoce.
    pub fn movida(&self, x: f64, y: f64, pagina: Option<u32>) -> Ancla {
        let mut a = self.clone();
        a.x = Some(redondo(x));
        a.y = Some(redondo(y));
        if pagina.is_some() {
            a.pagina = pagina;
        }
        a
    }

    /// **Donde se busca en un texto** (UTF-16): la cita en su sitio si sigue
    /// ahi; si no, la aparicion con mas contexto igual y, a igualdad, la mas
    /// cercana a donde estaba; si la cita ya no esta, lo que hay entre su
    /// contexto. `None` si no se encuentra: la pegatina queda **sin sitio**
    /// (se ve en la lista, no en el texto) y no se borra.
    pub fn ubicar(&self, texto: &[u16]) -> Option<(usize, usize)> {
        let Sitio::Texto { cita, posicion, .. } = self.sitio() else {
            return None;
        };
        ubicar_cita(texto, cita, posicion.map(|p| p.start).unwrap_or(0))
    }
}

fn es_blanco(u: u16) -> bool {
    char::from_u32(u as u32).is_some_and(char::is_whitespace)
}

fn es_de_palabra(u: u16) -> bool {
    char::from_u32(u as u32).is_some_and(char::is_alphanumeric) || (0xD800..0xE000).contains(&u)
}

/// La palabra que hay en `pos` (o justo antes).
fn palabra_en(texto: &[u16], pos: usize) -> Option<(usize, usize)> {
    let n = texto.len();
    if n == 0 {
        return None;
    }
    let mut p = pos.min(n);
    if p == n || !es_de_palabra(texto[p]) {
        if p > 0 && es_de_palabra(texto[p - 1]) {
            p -= 1;
        } else {
            // Sin palabra: la letra que haya (sin blancos).
            let q = (p..n).find(|&i| !es_blanco(texto[i]))?;
            return Some((q, q + 1));
        }
    }
    let mut a = p;
    while a > 0 && es_de_palabra(texto[a - 1]) {
        a -= 1;
    }
    let mut b = p;
    while b < n && es_de_palabra(texto[b]) {
        b += 1;
    }
    Some((a, b))
}

/// Un borde que no parte un par sustituto.
fn sin_partir(texto: &[u16], i: usize, hacia_dentro_desde_la_izquierda: bool) -> usize {
    let baja = |u: u16| (0xDC00..0xE000).contains(&u);
    if i > 0 && i < texto.len() && baja(texto[i]) {
        if hacia_dentro_desde_la_izquierda {
            i + 1
        } else {
            i - 1
        }
    } else {
        i
    }
}

fn apariciones(texto: &[u16], aguja: &[u16]) -> Vec<usize> {
    if aguja.is_empty() || aguja.len() > texto.len() {
        return Vec::new();
    }
    (0..=texto.len() - aguja.len())
        .filter(|&i| &texto[i..i + aguja.len()] == aguja)
        .collect()
}

/// Cuantas unidades del contexto coinciden alrededor de `en`.
fn contexto_igual(texto: &[u16], en: usize, largo: usize, antes: &[u16], despues: &[u16]) -> usize {
    let a = antes
        .iter()
        .rev()
        .zip(texto[..en].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let d = despues
        .iter()
        .zip(texto[en + largo..].iter())
        .take_while(|(x, y)| x == y)
        .count();
    a + d
}

fn ubicar_cita(texto: &[u16], cita: &Cita, pos: usize) -> Option<(usize, usize)> {
    let exacta: Vec<u16> = cita.exact.encode_utf16().collect();
    let antes: Vec<u16> = cita.prefix.encode_utf16().collect();
    let despues: Vec<u16> = cita.suffix.encode_utf16().collect();
    let todas = apariciones(texto, &exacta);
    if let Some(&mejor) = todas.iter().max_by(|&&a, &&b| {
        let ca = contexto_igual(texto, a, exacta.len(), &antes, &despues);
        let cb = contexto_igual(texto, b, exacta.len(), &antes, &despues);
        ca.cmp(&cb)
            .then_with(|| b.abs_diff(pos).cmp(&a.abs_diff(pos)))
    }) {
        return Some((mejor, mejor + exacta.len()));
    }
    // Entre su contexto, si los dos lados siguen ahi y no queda muy lejos.
    if antes.is_empty() || despues.is_empty() {
        return None;
    }
    let tope = exacta.len() * 2 + 64;
    apariciones(texto, &antes)
        .into_iter()
        .filter_map(|i| {
            let ini = i + antes.len();
            let resto = &texto[ini..];
            let j = apariciones(resto, &despues).into_iter().next()?;
            (j > 0 && j <= tope).then_some((ini, ini + j))
        })
        .min_by_key(|(a, _)| a.abs_diff(pos))
}

/// **Una hoja en la pantalla**: el rectangulo en que se pinta (pixeles de la
/// ventana) y lo que mide en sus unidades. Con ella el ancla de pagina va a
/// la pantalla y vuelve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HojaEnPantalla {
    pub pagina: u32,
    /// x, y, ancho, alto en pixeles de la ventana.
    pub caja: [f32; 4],
    /// Ancho y alto de la hoja en sus unidades (puntos, pixeles...).
    pub unidades: (f64, f64),
}

impl HojaEnPantalla {
    /// Donde cae en la pantalla un punto de esta hoja.
    pub fn a_pantalla(&self, x: f64, y: f64) -> (f32, f32) {
        let (uw, uh) = (self.unidades.0.max(1e-9), self.unidades.1.max(1e-9));
        (
            self.caja[0] + (x / uw) as f32 * self.caja[2],
            self.caja[1] + (y / uh) as f32 * self.caja[3],
        )
    }

    /// El punto de la hoja bajo un punto de la pantalla (puede caer fuera
    /// de la hoja: quien pregunta decide si vale).
    pub fn de_pantalla(&self, px: f32, py: f32) -> (f64, f64) {
        let (w, h) = (self.caja[2].max(1e-6), self.caja[3].max(1e-6));
        (
            ((px - self.caja[0]) / w) as f64 * self.unidades.0,
            ((py - self.caja[1]) / h) as f64 * self.unidades.1,
        )
    }

    pub fn contiene(&self, px: f32, py: f32) -> bool {
        px >= self.caja[0]
            && py >= self.caja[1]
            && px <= self.caja[0] + self.caja[2]
            && py <= self.caja[1] + self.caja[3]
    }
}

/// Donde cae en la pantalla el ancla de pagina `a`, entre las hojas que se
/// ven. `None` si no es de pagina o su hoja no se ve.
pub fn pagina_a_pantalla(a: &Ancla, hojas: &[HojaEnPantalla]) -> Option<(f32, f32)> {
    let Sitio::Pagina { pagina, x, y } = a.sitio() else {
        return None;
    };
    hojas
        .iter()
        .find(|h| h.pagina == pagina)
        .map(|h| h.a_pantalla(x, y))
}

/// El ancla de pagina bajo un punto de la pantalla: la hoja que lo
/// contiene. `None` si cae entre hojas.
pub fn pantalla_a_pagina(hojas: &[HojaEnPantalla], px: f32, py: f32) -> Option<Ancla> {
    let h = hojas.iter().find(|h| h.contiene(px, py))?;
    let (x, y) = h.de_pantalla(px, py);
    Some(Ancla::pagina(h.pagina, x, y))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn u(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn cada_sitio_se_lee_y_se_escribe_igual() {
        for a in [
            Ancla::pagina(3, 120.5, 700.25),
            Ancla::escena(-40.0, 1e4),
            Ancla::texto(SOBRE_HTML, &u("la losa ya esta"), 3, 7).unwrap(),
        ] {
            let t = serde_json::to_string(&a).unwrap();
            let b: Ancla = serde_json::from_str(&t).unwrap();
            assert_eq!(a, b, "{t}");
        }
        let a = Ancla::pagina(1, 2.0, 3.0);
        assert_eq!(
            serde_json::to_value(&a).unwrap(),
            serde_json::json!({"sitio":"pagina","pagina":1,"x":2.0,"y":3.0})
        );
    }

    #[test]
    fn lo_que_no_se_conoce_se_conserva_y_no_se_pinta() {
        let a: Ancla = serde_json::from_str(r#"{"sitio":"capitulo","n":4}"#).unwrap();
        assert_eq!(a.sitio(), Sitio::Desconocido);
        assert_eq!(serde_json::to_value(&a).unwrap()["n"], 4);
        // Caso negativo: una pagina sin punto no es un sitio.
        let b: Ancla = serde_json::from_str(r#"{"sitio":"pagina","pagina":2}"#).unwrap();
        assert_eq!(b.sitio(), Sitio::Desconocido);
    }

    #[test]
    fn el_ancla_de_texto_cita_lo_elegido_o_la_palabra_del_clic() {
        let t = u("El hormigon de la losa ya esta curado.");
        let a = Ancla::texto(SOBRE_DOCUMENTO, &t, 18, 22).unwrap();
        let Sitio::Texto { cita, posicion, .. } = a.sitio() else {
            panic!()
        };
        assert_eq!(cita.exact, "losa");
        assert_eq!(cita.prefix, "El hormigon de la ");
        assert_eq!(posicion.unwrap().start, 18);
        // Un clic dentro de «curado»: la palabra entera.
        let b = Ancla::texto(SOBRE_DOCUMENTO, &t, 33, 33).unwrap();
        assert_eq!(b.cita.unwrap().exact, "curado");
        // Caso negativo: sin texto no hay ancla.
        assert!(Ancla::texto(SOBRE_DOCUMENTO, &[], 0, 0).is_none());
        assert!(Ancla::texto(SOBRE_DOCUMENTO, &u("   "), 1, 2).is_none());
    }

    #[test]
    fn el_ancla_de_texto_se_vuelve_a_encontrar_aunque_el_texto_cambie() {
        let t = u("losa uno. La losa dos. Otra losa tres.");
        let a = Ancla::texto(SOBRE_HTML, &t, 13, 17).unwrap();
        assert_eq!(a.ubicar(&t), Some((13, 17)));
        // Se escribe delante: sigue en «losa dos» por su contexto.
        let t2 = u("Nuevo. losa uno. La losa dos. Otra losa tres.");
        assert_eq!(a.ubicar(&t2), Some((20, 24)));
        // Se cambia la cita: lo que hay entre su contexto.
        let b = Ancla::texto(SOBRE_HTML, &t, 18, 21).unwrap(); // «dos»
        let t3 = u("losa uno. La losa DOS!. Otra losa tres.");
        let (i, j) = b.ubicar(&t3).unwrap();
        assert_eq!(String::from_utf16_lossy(&t3[i..j]), "DOS!");
        // Caso negativo: sin rastro, sin sitio.
        assert_eq!(b.ubicar(&u("nada que ver")), None);
    }

    #[test]
    fn la_pagina_va_a_la_pantalla_y_vuelve() {
        let hojas = [
            HojaEnPantalla {
                pagina: 0,
                caja: [10.0, 10.0, 306.0, 396.0],
                unidades: (612.0, 792.0),
            },
            HojaEnPantalla {
                pagina: 1,
                caja: [10.0, 420.0, 306.0, 396.0],
                unidades: (612.0, 792.0),
            },
        ];
        let a = Ancla::pagina(1, 306.0, 396.0);
        let (px, py) = pagina_a_pantalla(&a, &hojas).unwrap();
        assert_eq!((px, py), (163.0, 618.0));
        let b = pantalla_a_pagina(&hojas, px, py).unwrap();
        assert_eq!(b, a);
        // Entre hojas no hay ancla; una hoja que no se ve no se pinta.
        assert!(pantalla_a_pagina(&hojas, 50.0, 412.0).is_none());
        assert!(pagina_a_pantalla(&Ancla::pagina(7, 1.0, 1.0), &hojas).is_none());
        assert!(pagina_a_pantalla(&Ancla::escena(1.0, 1.0), &hojas).is_none());
    }

    #[test]
    fn mover_conserva_el_sitio_y_lo_desconocido() {
        let mut a = Ancla::pagina(2, 1.0, 1.0);
        a.resto.insert("giro".into(), Value::from(90));
        let b = a.movida(5.123456, 6.0, None);
        assert_eq!(
            b.sitio(),
            Sitio::Pagina {
                pagina: 2,
                x: 5.1235,
                y: 6.0
            }
        );
        assert_eq!(b.resto["giro"], 90);
        let c = a.movida(1.0, 1.0, Some(4));
        assert_eq!(c.pagina, Some(4));
    }
}
