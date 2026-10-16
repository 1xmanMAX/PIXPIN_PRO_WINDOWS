//! **El diagrama medido, a elementos del lienzo**: las figuras del motor
//! (`Linea`, `Flecha`, `Arco`, `Elipse`, `Rectangulo`, `Punto`, `Texto`) y
//! las regiones de color (`pixpin-region`), por `lienzo::Fabrica` como todo
//! lo que dibuja el asistente.

use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use pixpin_motor2d::formas::TipoPunta;
use pixpin_motor2d::vector::Punto2;

use super::{Diagrama, Estilo};
use crate::hoja::Alineacion;
use crate::lienzo::Fabrica;

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn color(c: [u8; 3]) -> ColorRgba {
    ColorRgba::opaco(c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0)
}

/// Lo casi negro, negro del lienzo (el antialias lo deja gris oscuro).
fn limpio(c: [u8; 3]) -> [u8; 3] {
    let max = *c.iter().max().unwrap_or(&0) as i32;
    let min = *c.iter().min().unwrap_or(&0) as i32;
    if max < 90 && max - min < 30 { [30, 30, 30] } else { c }
}

fn estilo(e: Estilo) -> EstiloTrazo {
    match e {
        Estilo::Solido => EstiloTrazo::Solido,
        Estilo::Discontinuo => EstiloTrazo::Discontinuo,
        Estilo::Punteado => EstiloTrazo::Punteado,
    }
}

/// El grosor en el lienzo: el medido a escala, de medio en medio pixel.
fn grosor(g: f64, escala: f64) -> f32 {
    (((g * escala).max(1.0) * 2.0).round() / 2.0) as f32
}

/// **Los elementos del diagrama** en la fabrica: el pixel `(x, y)` del
/// recorte cae en `origen + (x, y) * escala`. Debajo las regiones, luego
/// los trazos, los puntos y encima los rotulos.
pub(crate) fn elementos(d: &Diagrama, origen: (f64, f64), escala: f64, f: &mut Fabrica, familia: &str) {
    let a = |p: (f64, f64)| (origen.0 + p.0 * escala, origen.1 + p.1 * escala);
    let ap = |p: (f64, f64)| {
        let q = a(p);
        Punto2::nuevo(q.0 as f32, q.1 as f32)
    };
    for r in &d.regiones {
        let contorno: Vec<(f64, f64)> = r.contorno.iter().map(|&p| a(p)).collect();
        let huecos: Vec<Vec<(f64, f64)>> = r.huecos.iter().map(|h| h.iter().map(|&p| a(p)).collect()).collect();
        let c = hex(r.color);
        f.region(&contorno, &huecos, &c, &c);
        if let Some(v) = f.salida.last_mut() {
            v["roughness"] = serde_json::json!(0);
            v["strokeWidth"] = serde_json::json!(1);
        }
    }
    let mut piezas: Vec<Elemento> = Vec::new();
    let base = |c: [u8; 3], g: f64| Elemento { trazo: color(limpio(c)), grosor: grosor(g, escala), rugosidad: 0.0, ..Default::default() };
    for arco in &d.arcos {
        let (cx, cy) = a(arco.c);
        let r = (arco.r * escala) as f32;
        let mut e = base(arco.color, arco.grosor);
        e.x = cx as f32 - r;
        e.y = cy as f32 - r;
        e.ancho = 2.0 * r;
        e.alto = 2.0 * r;
        e.estilo = estilo(arco.estilo);
        e.figura = if arco.entera() { Figura::Elipse } else { Figura::Arco { inicio: arco.inicio as f32, barrido: Some(arco.barrido as f32) } };
        piezas.push(e);
    }
    for r in &d.recuadros {
        let (x0, y0) = a((r.x0, r.y0));
        let (x1, y1) = a((r.x1, r.y1));
        let mut e = base(r.color, r.grosor);
        e.figura = Figura::Rectangulo;
        e.x = x0 as f32;
        e.y = y0 as f32;
        e.ancho = (x1 - x0) as f32;
        e.alto = (y1 - y0) as f32;
        piezas.push(e);
    }
    for s in &d.segmentos {
        let puntos = vec![ap(s.a), ap(s.b)];
        let mut e = base(s.color, s.grosor);
        e.x = puntos[0].x;
        e.y = puntos[0].y;
        e.ancho = (puntos[0].x - puntos[1].x).abs();
        e.alto = (puntos[0].y - puntos[1].y).abs();
        e.estilo = estilo(s.estilo);
        e.figura = if s.flecha_a || s.flecha_b {
            Figura::Flecha {
                puntos,
                punta_inicio: if s.flecha_a { TipoPunta::Triangulo } else { TipoPunta::Ninguna },
                punta_fin: if s.flecha_b { TipoPunta::Triangulo } else { TipoPunta::Ninguna },
                codos: false,
            }
        } else {
            Figura::Linea { puntos }
        };
        piezas.push(e);
    }
    // Las letras que nadie leyo: sus trazos, alisados.
    for g in &d.glifos {
        for t in &g.trazos {
            let lisa = crate::calcar::curva(t, false, 0.5, 0.25);
            if lisa.len() < 2 {
                continue;
            }
            let mut e = base(g.color, g.grosor);
            let puntos: Vec<Punto2> = lisa.iter().map(|&p| ap(p)).collect();
            e.x = puntos[0].x;
            e.y = puntos[0].y;
            e.figura = Figura::Linea { puntos };
            piezas.push(e);
        }
    }
    for p in &d.puntos {
        let (x, y) = a(p.c);
        let mut e = base(p.color, (p.r / 1.6).max(1.0));
        e.grosor = ((p.r * escala / 1.6).max(1.0)) as f32;
        e.x = x as f32;
        e.y = y as f32;
        e.figura = Figura::Punto { letra: String::new(), angulo: 0.0, radio: 14.0 };
        piezas.push(e);
    }
    let k = pixpin_motor2d::texto::interlineado_de(familia).unwrap_or(pixpin_motor2d::texto::INTERLINEADO) as f64;
    for r in &d.rotulos {
        let c = crate::hoja::Caja::nueva(origen.0 + r.caja.x * escala, origen.1 + r.caja.y * escala, r.caja.w * escala, r.caja.h * escala);
        // Una letra suelta mide su alto de mayuscula (~0,7 de la letra); una
        // palabra del OCR, el renglon.
        let tam = if r.letra_suelta { c.h / 0.72 } else { c.h * 1.05 };
        let tam = ((tam * 2.0).round() / 2.0).max(6.0);
        piezas.push(crate::hoja::montaje::un_texto(&r.texto, c.x, c.cy() - tam * k / 2.0, tam, familia, r.negrita, Alineacion::Izquierda, r.color.map(limpio)));
    }
    f.de_motor(&piezas);
    for v in f.salida.iter_mut() {
        if v["type"] != "text" {
            v["roughness"] = serde_json::json!(0);
        }
    }
}

/// **Los elementos del diagrama**, en su propio grupo: `origen` y `escala`
/// llevan el recorte al lienzo.
pub fn a_elementos(d: &Diagrama, origen: (f64, f64), escala: f64, semilla: u64, familia: &str) -> Vec<serde_json::Value> {
    let mut f = Fabrica::nueva((0.0, 0.0), semilla);
    f.grupo = Some(f.nombre_de_grupo("diagrama"));
    elementos(d, origen, escala, &mut f, familia);
    f.salida
}
