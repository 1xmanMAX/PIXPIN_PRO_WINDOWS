//! **Las pegatinas de comentario en el lector de PDF y Office**
//! (`crate::comentarios`): el ancla es la pagina y un punto en sus unidades
//! (los puntos del PDF; en un Word o un PowerPoint, las medidas de su hoja),
//! con y hacia abajo desde la esquina de arriba a la izquierda. Asi cae en
//! el mismo sitio con cualquier aumento, con los espacios para anotar y en
//! otro aparato.

use pindesk_comentarios::{Ancla, Sitio};
use pindesk_docs::vista;

use super::{Estado, Marco, alto_de, px};
use crate::comentarios::Ubicar;

/// Las anclas del lector con la vista de ahora.
pub(super) struct UbicarPdf<'a> {
    pub(super) e: &'a Estado,
    pub(super) m: Marco,
}

impl UbicarPdf<'_> {
    /// Lo que mide la hoja `i` en sus unidades (sin medidas, las del lector).
    fn unidades(&self, i: usize) -> (f64, f64) {
        match self.e.medidas.get(i) {
            Some(&(w, h)) if w > 0.0 && h > 0.0 => (w as f64, h as f64),
            _ => (
                vista::ANCHO_HOJA as f64,
                alto_de(&self.e.hojas, i).max(1.0) as f64,
            ),
        }
    }
}

impl Ubicar for UbicarPdf<'_> {
    fn a_pantalla(&self, a: &Ancla) -> Option<(f32, f32)> {
        let Sitio::Pagina { pagina, x, y } = a.sitio() else {
            return None;
        };
        let i = pagina as usize;
        let arriba = *self.e.hojas.arriba.get(i)?;
        let (uw, uh) = self.unidades(i);
        let rx = (x / uw) as f32 * vista::ANCHO_HOJA;
        let ry = arriba + (y / uh) as f32 * alto_de(&self.e.hojas, i);
        let s = px(self.e, self.m);
        Some(((rx - self.e.x) * s, (ry - self.e.y) * s))
    }

    fn de_pantalla(&self, p: (f32, f32)) -> Option<Ancla> {
        if self.e.hojas.cuantas() == 0 {
            return None;
        }
        let s = px(self.e, self.m);
        let (rx, ry) = (self.e.x + p.0 / s, self.e.y + p.1 / s);
        let i = self.e.hojas.en(ry);
        let arriba = self.e.hojas.arriba[i];
        let alto = alto_de(&self.e.hojas, i).max(1e-3);
        // Fuera de la hoja (en los espacios o entre dos hojas) no se pega.
        if !(0.0..=vista::ANCHO_HOJA).contains(&rx) || ry < arriba || ry > arriba + alto {
            return None;
        }
        let (uw, uh) = self.unidades(i);
        Some(Ancla::pagina(
            i as u32,
            (rx / vista::ANCHO_HOJA) as f64 * uw,
            ((ry - arriba) / alto) as f64 * uh,
        ))
    }
}

/// **Va a la pegatina** (de la lista): el aumento de ahora, con su punto en
/// el centro de la ventana.
pub(super) fn ir_a(e: &mut Estado, m: Marco, a: &Ancla) {
    let Sitio::Pagina { pagina, x, y } = a.sitio() else {
        return;
    };
    let i = pagina as usize;
    let Some(&arriba) = e.hojas.arriba.get(i) else {
        return;
    };
    let u = UbicarPdf { e, m }.unidades(i);
    let rx = (x / u.0) as f32 * vista::ANCHO_HOJA;
    let ry = arriba + (y / u.1) as f32 * alto_de(&e.hojas, i);
    let s = px(e, m);
    e.x = rx - m.ancho / (2.0 * s);
    e.y = ry - m.alto / (2.0 * s);
    super::acotar(e, m);
}

#[cfg(test)]
mod pruebas {
    use super::super::pruebas::{estado, marco};
    use super::*;

    /// El ancla va en puntos de la pagina: la misma con otro aumento y
    /// otra vista, y vuelve al mismo sitio de la pantalla.
    #[test]
    fn el_ancla_de_pagina_no_depende_del_aumento() {
        let mut e = estado(3);
        let m = marco();
        e.zoom = 1.0;
        e.x = 0.0;
        e.y = 0.0;
        let u = UbicarPdf { e: &e, m };
        let p = u.a_pantalla(&Ancla::pagina(1, 297.5, 421.0)).unwrap();
        let a = u.de_pantalla(p).unwrap();
        let Sitio::Pagina { pagina, x, y } = a.sitio() else {
            panic!()
        };
        assert_eq!(pagina, 1);
        assert!(
            (x - 297.5).abs() < 0.05 && (y - 421.0).abs() < 0.05,
            "{x} {y}"
        );
        // Con el doble de aumento, el mismo punto del papel cae en otro sitio
        // de la pantalla, pero sigue siendo el mismo punto.
        e.zoom = 2.0;
        let u = UbicarPdf { e: &e, m };
        let q = u.a_pantalla(&a).unwrap();
        assert_ne!(p, q);
        let b = u.de_pantalla(q).unwrap();
        let Sitio::Pagina { x: x2, y: y2, .. } = b.sitio() else {
            panic!()
        };
        assert!((x2 - x).abs() < 0.05 && (y2 - y).abs() < 0.05);
        // Caso negativo: fuera de la hoja (a su izquierda) no hay ancla.
        assert!(u.de_pantalla((-50.0, q.1)).is_none());
        // E ir a ella la deja en el centro de la ventana.
        ir_a(&mut e, m, &b);
        let u = UbicarPdf { e: &e, m };
        let c = u.a_pantalla(&b).unwrap();
        assert!(
            (c.0 - m.ancho / 2.0).abs() < 1.0 && (c.1 - m.alto / 2.0).abs() < 1.0,
            "{c:?}"
        );
    }
}
