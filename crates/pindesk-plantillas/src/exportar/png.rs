//! **A PNG**, sin GPU ni sistema (tiny-skia para las formas y ab_glyph para
//! las letras). Es lo que pintan las pruebas y las muestras de
//! `docs/plantillas/`, y lo que exporta la app.

use ab_glyph::{Font, point};
use tiny_skia::{
    FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, StrokeDash, Transform,
};

use super::letras::{Letras, Vista};
use crate::prim::{Caja, PaginaHecha, Prim, Raya};
use crate::tema::Color;
use crate::texto;

/// Lo mas grande que se pinta (lado, pixeles).
pub const LADO_MAXIMO: u32 = 8000;

/// **Pinta una pagina** a `ppp` puntos por pulgada (96 = pantalla, 150 o
/// 300 para imprimir). `None` si no cabe en memoria.
pub fn pintar(p: &PaginaHecha, ppp: f32, letras: &Letras) -> Option<Pixmap> {
    let ppmm = ppp / 25.4;
    let w = ((p.ancho * ppmm).round() as u32).clamp(1, LADO_MAXIMO);
    let h = ((p.alto * ppmm).round() as u32).clamp(1, LADO_MAXIMO);
    let mut px = Pixmap::new(w, h)?;
    let fondo = p.papel.f32s();
    px.fill(tiny_skia::Color::from_rgba(
        fondo[0], fondo[1], fondo[2], 1.0,
    )?);
    let tr = Transform::from_scale(ppmm, ppmm);
    let vista = letras.vista(&p.familias);
    for prim in &p.prims {
        dibujar(&mut px, prim, tr, ppmm, &vista);
    }
    Some(px)
}

/// Lo mismo, ya en bytes de PNG.
pub fn a_png(p: &PaginaHecha, ppp: f32, letras: &Letras) -> Option<Vec<u8>> {
    pintar(p, ppp, letras)?.encode_png().ok()
}

fn pintura(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c.r, c.g, c.b, c.a);
    p.anti_alias = true;
    p
}

/// Un rectangulo de esquinas redondas.
pub(crate) fn rect_redondo(pb: &mut PathBuilder, c: Caja, r: f32) {
    let r = r.min(c.w / 2.0).min(c.h / 2.0).max(0.0);
    if r <= 0.01 {
        if let Some(rect) = tiny_skia::Rect::from_xywh(c.x, c.y, c.w, c.h) {
            pb.push_rect(rect);
        }
        return;
    }
    // La constante de la circunferencia con cubicas.
    let k = r * 0.552_284_8;
    let (x0, y0, x1, y1) = (c.x, c.y, c.derecha(), c.abajo());
    pb.move_to(x0 + r, y0);
    pb.line_to(x1 - r, y0);
    pb.cubic_to(x1 - r + k, y0, x1, y0 + r - k, x1, y0 + r);
    pb.line_to(x1, y1 - r);
    pb.cubic_to(x1, y1 - r + k, x1 - r + k, y1, x1 - r, y1);
    pb.line_to(x0 + r, y1);
    pb.cubic_to(x0 + r - k, y1, x0, y1 - r + k, x0, y1 - r);
    pb.line_to(x0, y0 + r);
    pb.cubic_to(x0, y0 + r - k, x0 + r - k, y0, x0 + r, y0);
    pb.close();
}

fn trazo(grosor: f32, raya: Raya) -> Stroke {
    let mut s = Stroke {
        width: grosor.max(0.05),
        line_cap: LineCap::Butt,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };
    match raya {
        Raya::Continua => {}
        Raya::Discontinua => {
            s.dash = StrokeDash::new(vec![grosor * 6.0 + 0.8, grosor * 4.0 + 0.6], 0.0);
        }
        Raya::Punteada => {
            s.line_cap = LineCap::Round;
            s.dash = StrokeDash::new(vec![0.001, grosor * 3.0 + 0.4], 0.0);
        }
    }
    s
}

fn dibujar(px: &mut Pixmap, prim: &Prim, tr: Transform, ppmm: f32, letras: &Vista) {
    match prim {
        Prim::Rect {
            caja,
            relleno,
            trazo: borde,
            grosor,
            radio,
        } => {
            let mut pb = PathBuilder::new();
            rect_redondo(&mut pb, *caja, *radio);
            let Some(path) = pb.finish() else { return };
            if let Some(c) = relleno {
                px.fill_path(&path, &pintura(*c), FillRule::Winding, tr, None);
            }
            if let Some(c) = borde {
                px.stroke_path(
                    &path,
                    &pintura(*c),
                    &trazo(*grosor, Raya::Continua),
                    tr,
                    None,
                );
            }
        }
        Prim::Linea {
            a,
            b,
            color,
            grosor,
            raya,
        } => {
            let mut pb = PathBuilder::new();
            pb.move_to(a.0, a.1);
            pb.line_to(b.0, b.1);
            if let Some(path) = pb.finish() {
                px.stroke_path(&path, &pintura(*color), &trazo(*grosor, *raya), tr, None);
            }
        }
        Prim::Circulo {
            centro,
            radio,
            relleno,
            trazo: borde,
            grosor,
        } => {
            let Some(path) = PathBuilder::from_circle(centro.0, centro.1, radio.max(0.01)) else {
                return;
            };
            if let Some(c) = relleno {
                px.fill_path(&path, &pintura(*c), FillRule::Winding, tr, None);
            }
            if let Some(c) = borde {
                px.stroke_path(
                    &path,
                    &pintura(*c),
                    &trazo(*grosor, Raya::Continua),
                    tr,
                    None,
                );
            }
        }
        Prim::Poli {
            puntos,
            color,
            grosor,
        } => {
            let mut pb = PathBuilder::new();
            for (i, (x, y)) in puntos.iter().enumerate() {
                if i == 0 {
                    pb.move_to(*x, *y);
                } else {
                    pb.line_to(*x, *y);
                }
            }
            if let Some(path) = pb.finish() {
                let mut s = trazo(*grosor, Raya::Continua);
                s.line_cap = LineCap::Round;
                px.stroke_path(&path, &pintura(*color), &s, tr, None);
            }
        }
        Prim::Puntos {
            centros,
            radio,
            color,
        } => {
            let mut pb = PathBuilder::new();
            for (x, y) in centros {
                pb.push_circle(*x, *y, *radio);
            }
            if let Some(path) = pb.finish() {
                px.fill_path(&path, &pintura(*color), FillRule::Winding, tr, None);
            }
        }
        Prim::Texto { .. } => {
            for r in texto::renglones(prim, letras) {
                renglon(px, &r, ppmm, letras);
            }
        }
    }
}

/// Las letras de un renglon, mezcladas en el mapa con su cobertura.
fn renglon(px: &mut Pixmap, r: &texto::Renglon, ppmm: f32, letras: &Vista) {
    let em = r.tam * ppmm;
    let mut x = r.x * ppmm;
    let base = r.base * ppmm;
    let (w, h) = (px.width() as i32, px.height() as i32);
    let [cr, cg, cb, ca] = r.color.f32s();
    for t in letras.trozos(&r.texto, r.letra, r.negrita) {
        let escala = t.cara.escala(em);
        let mut antes = None;
        for (g, _) in &t.glifos {
            if let Some(a) = antes {
                x += t.cara.interletra(a, *g, em);
            }
            let glifo = g.with_scale_and_position(escala, point(x, base));
            if let Some(o) = t.cara.fuente.outline_glyph(glifo) {
                let caja = o.px_bounds();
                let datos = px.data_mut();
                o.draw(|gx, gy, cobertura| {
                    let xx = caja.min.x as i32 + gx as i32;
                    let yy = caja.min.y as i32 + gy as i32;
                    if xx < 0 || yy < 0 || xx >= w || yy >= h {
                        return;
                    }
                    let a = (cobertura.clamp(0.0, 1.0) * ca).min(1.0);
                    if a <= 0.0 {
                        return;
                    }
                    let i = ((yy * w + xx) * 4) as usize;
                    // Premultiplicado: fuente sobre lo que hay.
                    let mezcla =
                        |d: u8, s: f32| (s * 255.0 * a + f32::from(d) * (1.0 - a)).round() as u8;
                    datos[i] = mezcla(datos[i], cr);
                    datos[i + 1] = mezcla(datos[i + 1], cg);
                    datos[i + 2] = mezcla(datos[i + 2], cb);
                    datos[i + 3] = (255.0 * a + f32::from(datos[i + 3]) * (1.0 - a)).round() as u8;
                });
            }
            x += t.cara.avance(*g) * em;
            antes = Some(*g);
        }
    }
}
