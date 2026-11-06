//! **A SVG**: la pagina como dibujo de vectores con el texto como texto (la
//! letra la pone quien lo abre: Fraunces, Work Sans y Caveat, con reserva
//! de serif o sans-serif).

use std::fmt::Write as _;

use crate::prim::{PaginaHecha, Prim, Raya};
use crate::tema::{Color, Letra, Tema};
use crate::texto::{self, Medidor};

fn color(c: Color) -> String {
    format!("rgb({},{},{})", c.r, c.g, c.b)
}

fn opacidad(c: Color) -> String {
    if c.a == 255 {
        String::new()
    } else {
        format!(" fill-opacity=\"{:.3}\"", f32::from(c.a) / 255.0)
    }
}

fn escapar(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn raya(grosor: f32, r: Raya) -> String {
    match r {
        Raya::Continua => String::new(),
        Raya::Discontinua => format!(
            " stroke-dasharray=\"{:.2} {:.2}\"",
            grosor * 6.0 + 0.8,
            grosor * 4.0 + 0.6
        ),
        Raya::Punteada => format!(
            " stroke-linecap=\"round\" stroke-dasharray=\"0.01 {:.2}\"",
            grosor * 3.0 + 0.4
        ),
    }
}

/// **El SVG de una pagina**, en milimetros.
pub fn a_svg(p: &PaginaHecha, tema: &Tema, m: &dyn Medidor) -> String {
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">\n",
        w = p.ancho,
        h = p.alto
    );
    for prim in &p.prims {
        match prim {
            Prim::Rect {
                caja,
                relleno,
                trazo,
                grosor,
                radio,
            } => {
                let _ = write!(
                    s,
                    "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" rx=\"{:.2}\"",
                    caja.x, caja.y, caja.w, caja.h, radio
                );
                match relleno {
                    Some(c) => {
                        let _ = write!(s, " fill=\"{}\"{}", color(*c), opacidad(*c));
                    }
                    None => s.push_str(" fill=\"none\""),
                }
                if let Some(c) = trazo {
                    let _ = write!(s, " stroke=\"{}\" stroke-width=\"{grosor:.3}\"", color(*c));
                }
                s.push_str("/>\n");
            }
            Prim::Linea {
                a,
                b,
                color: c,
                grosor,
                raya: r,
            } => {
                let _ = writeln!(
                    s,
                    "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{}\" stroke-width=\"{grosor:.3}\"{}/>",
                    a.0,
                    a.1,
                    b.0,
                    b.1,
                    color(*c),
                    raya(*grosor, *r)
                );
            }
            Prim::Circulo {
                centro,
                radio,
                relleno,
                trazo,
                grosor,
            } => {
                let _ = write!(
                    s,
                    "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"{:.2}\"",
                    centro.0, centro.1, radio
                );
                match relleno {
                    Some(c) => {
                        let _ = write!(s, " fill=\"{}\"{}", color(*c), opacidad(*c));
                    }
                    None => s.push_str(" fill=\"none\""),
                }
                if let Some(c) = trazo {
                    let _ = write!(s, " stroke=\"{}\" stroke-width=\"{grosor:.3}\"", color(*c));
                }
                s.push_str("/>\n");
            }
            Prim::Poli {
                puntos,
                color: c,
                grosor,
            } => {
                let pts: Vec<String> = puntos
                    .iter()
                    .map(|(x, y)| format!("{x:.2},{y:.2}"))
                    .collect();
                let _ = writeln!(
                    s,
                    "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{grosor:.3}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
                    pts.join(" "),
                    color(*c)
                );
            }
            Prim::Puntos {
                centros,
                radio,
                color: c,
            } => {
                let _ = writeln!(s, "<g fill=\"{}\">", color(*c));
                for (x, y) in centros {
                    let _ = writeln!(s, "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{radio:.2}\"/>");
                }
                s.push_str("</g>\n");
            }
            Prim::Texto { .. } => {
                for r in texto::renglones(prim, m) {
                    let familia = match r.letra {
                        Letra::Titulo => format!("{}, serif", tema.letra_titulo),
                        Letra::Cuerpo => format!("{}, sans-serif", tema.letra_cuerpo),
                        Letra::Mano => format!("{}, cursive", crate::tema::CAVEAT),
                    };
                    let _ = writeln!(
                        s,
                        "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"{}\" font-size=\"{:.2}\" fill=\"{}\"{}>{}</text>",
                        r.x,
                        r.base,
                        escapar(&familia),
                        r.tam,
                        color(r.color),
                        if r.negrita {
                            " font-weight=\"bold\""
                        } else {
                            ""
                        },
                        escapar(&r.texto)
                    );
                }
            }
        }
    }
    s.push_str("</svg>\n");
    s
}
