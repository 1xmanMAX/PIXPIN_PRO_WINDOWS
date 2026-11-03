//! **Las pegatinas de comentario en el lector que reajusta el texto**
//! (Word en modo lectura, libros, Markdown, HTML y texto; `crate::comentarios`).
//!
//! Aqui el texto se recoloca con el ancho de la ventana (y del movil), asi
//! que una pegatina no puede guardar un punto: guarda **su cita**, como las
//! marcas de texto de las notas (`TextQuoteSelector` y
//! `TextPositionSelector`, posiciones UTF-16), sobre el texto del documento
//! tal como lo lee el lector: los parrafos colocados unidos con `\n`
//! (`sobre: "documento"`). Al pintar se vuelve a buscar y la pegatina va al
//! principio de su cita. Si la cita ya no esta, no se pinta (queda en la
//! lista) y no se borra.

use std::cell::RefCell;
use std::collections::HashMap;

use pindesk_comentarios::ancla::SOBRE_DOCUMENTO;
use pindesk_comentarios::{Ancla, Sitio};

use super::{Estado, Marco, marcar, px};
use crate::comentarios::Ubicar;

/// El texto del documento y donde empieza cada parrafo, con lo ya buscado.
#[derive(Default)]
pub(super) struct TextoDelVisor {
    utf16: Vec<u16>,
    inicios: Vec<usize>,
    firma: (usize, Option<(u32, u32, u8, u8)>),
    /// Donde se encontro cada cita (por su JSON): buscarla en cada evento
    /// costaria recorrer el documento entero.
    hallado: RefCell<HashMap<String, Option<usize>>>,
}

impl TextoDelVisor {
    /// Al dia con lo colocado (otra columna, otra letra, otro documento).
    pub(super) fn al_dia(&mut self, e: &Estado) {
        let firma = (e.colocados.len(), e.medido_con);
        if firma == self.firma && !self.inicios.is_empty() {
            return;
        }
        self.firma = firma;
        self.utf16.clear();
        self.inicios.clear();
        for c in &e.colocados {
            self.inicios.push(self.utf16.len());
            self.utf16.extend(c.texto.encode_utf16());
            self.utf16.push(u16::from(b'\n'));
        }
        self.hallado.borrow_mut().clear();
    }

    /// El parrafo y la letra (no UTF-16) de una posicion del texto.
    fn parrafo_y_letra(&self, e: &Estado, g: usize) -> Option<(usize, usize)> {
        let i = self.inicios.partition_point(|s| *s <= g).checked_sub(1)?;
        let local = g - self.inicios[i];
        let c = e.colocados.get(i)?;
        let mut u = 0;
        for (k, ch) in c.texto.chars().enumerate() {
            if u >= local {
                return Some((i, k));
            }
            u += ch.len_utf16();
        }
        Some((i, c.texto.chars().count()))
    }

    /// La posicion UTF-16 de la letra `k` del parrafo `i`.
    fn posicion(&self, e: &Estado, i: usize, k: usize) -> usize {
        let local: usize = e
            .colocados
            .get(i)
            .map(|c| c.texto.chars().take(k).map(char::len_utf16).sum())
            .unwrap_or(0);
        self.inicios.get(i).copied().unwrap_or(0) + local
    }
}

/// Las anclas del lector con la vista de ahora.
pub(super) struct UbicarVisor<'a> {
    pub(super) e: &'a Estado,
    pub(super) m: Marco,
    pub(super) t: &'a TextoDelVisor,
}

impl UbicarVisor<'_> {
    fn a_la_pantalla(&self, x: f32, y: f32) -> (f32, f32) {
        let s = px(self.e, self.m);
        ((x - self.e.x) * s, (y - self.e.y) * s)
    }

    /// Donde empieza la cita en el documento (unidades del documento).
    pub(super) fn en_el_documento(&self, a: &Ancla) -> Option<(f32, f32)> {
        let clave = serde_json::to_string(a).ok()?;
        let g = *self
            .t
            .hallado
            .borrow_mut()
            .entry(clave)
            .or_insert_with(|| a.ubicar(&self.t.utf16).map(|(i, _)| i));
        let (i, k) = self.t.parrafo_y_letra(self.e, g?)?;
        let c = self.e.colocados.get(i)?;
        Some(match marcar::caja_de_letra(self.e, i, k) {
            Some(caja) => (caja[0], caja[1]),
            // Un parrafo que aun no se ve: su esquina (basta para ir a el).
            None => (c.sangria, c.y),
        })
    }
}

impl Ubicar for UbicarVisor<'_> {
    fn a_pantalla(&self, a: &Ancla) -> Option<(f32, f32)> {
        if !matches!(a.sitio(), Sitio::Texto { .. }) {
            return None;
        }
        let (x, y) = self.en_el_documento(a)?;
        Some(self.a_la_pantalla(x, y))
    }

    fn de_pantalla(&self, p: (f32, f32)) -> Option<Ancla> {
        let s = px(self.e, self.m);
        let (x, y) = (self.e.x + p.0 / s, self.e.y + p.1 / s);
        let i = self
            .e
            .colocados
            .iter()
            .position(|c| y >= c.y && y <= c.y + c.alto && !c.texto.is_empty())?;
        let k = marcar::letra_en(self.e, i, x, y).unwrap_or(0);
        let g = self.t.posicion(self.e, i, k);
        Ancla::texto(SOBRE_DOCUMENTO, &self.t.utf16, g, g)
    }
}

/// **Va a la pegatina** (de la lista): su renglon arriba, a un tercio.
pub(super) fn ir_a(e: &mut Estado, m: Marco, t: &TextoDelVisor, a: &Ancla) {
    let Some((_, y)) = (UbicarVisor { e, m, t }).en_el_documento(a) else {
        return;
    };
    let s = px(e, m);
    e.y = y - m.alto / (3.0 * s);
    super::acotar(e, m);
}

#[cfg(test)]
mod pruebas {
    use super::super::pruebas::{colocado, estado_de_prueba, marco};
    use super::*;

    /// El ancla es la cita: aunque el texto se recoloque en otro sitio (otra
    /// columna, un parrafo nuevo delante), la pegatina va con su frase.
    #[test]
    fn la_pegatina_sigue_a_su_cita_aunque_el_texto_se_mueva() {
        let mut e = estado_de_prueba();
        e.colocados = vec![
            colocado("Primero el replanteo.", 0.0),
            colocado("Luego la losa del segundo piso.", 100.0),
        ];
        let mut t = TextoDelVisor::default();
        t.al_dia(&e);
        let m = marco();
        let g = t.posicion(&e, 1, 9); // «losa»
        let a = Ancla::texto(SOBRE_DOCUMENTO, &t.utf16, g, g).unwrap();
        let Sitio::Texto { cita, .. } = a.sitio() else {
            panic!()
        };
        assert_eq!(cita.exact, "losa");
        let u = UbicarVisor { e: &e, m, t: &t };
        assert_eq!(u.en_el_documento(&a), Some((0.0, 100.0)));
        // Un parrafo nuevo delante: la losa baja con su parrafo.
        e.colocados.insert(0, colocado("Nota nueva.", 0.0));
        e.colocados[1].y = 100.0;
        e.colocados[2].y = 200.0;
        t.al_dia(&e);
        let u = UbicarVisor { e: &e, m, t: &t };
        assert_eq!(u.en_el_documento(&a), Some((0.0, 200.0)));
        // Caso negativo: sin la frase no hay sitio (y no se pinta).
        e.colocados = vec![colocado("Otro documento.", 0.0)];
        t.al_dia(&e);
        let u = UbicarVisor { e: &e, m, t: &t };
        assert_eq!(u.a_pantalla(&a), None);
    }
}
