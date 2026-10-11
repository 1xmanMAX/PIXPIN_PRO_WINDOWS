//! **Las formas de DrawingML** (`a:prstGeom` y `a:custGeom`) como
//! trayectos. Se escriben a mano las predefinidas mas comunes a partir de
//! su definicion en ECMA-376 (parte 1, 20.1.10.56); las que faltan salen
//! como rectangulo, que es lo que menos engana.

use std::collections::HashMap;

use pixpin_docs::xml::Nodo;

use crate::pagina::{Figura, Tramo};

/// Constante de la Bezier que aproxima un cuarto de elipse.
const K: f32 = 0.552_284_8;

/// Los ajustes de una forma (`a:avLst/a:gd name="adj" fmla="val 25000"`).
pub fn ajustes(prst: Option<&Nodo>) -> HashMap<String, f32> {
    let mut m = HashMap::new();
    let Some(av) = prst.and_then(|p| p.hijo("avLst")) else {
        return m;
    };
    for gd in av.elementos().filter(|e| e.nombre == "gd") {
        let f = gd.atributo("fmla");
        if let Some(v) = f.strip_prefix("val ").and_then(|v| v.trim().parse::<f32>().ok()) {
            m.insert(gd.atributo("name").to_string(), v);
        }
    }
    m
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Figura {
    poligono(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)])
}

fn poligono(p: &[(f32, f32)]) -> Figura {
    Figura { inicio: p[0], tramos: p[1..].iter().map(|&(x, y)| Tramo::Linea(x, y)).collect(), cerrada: true, rellenar: true, trazar: true }
}

fn polilinea(p: &[(f32, f32)]) -> Figura {
    Figura { cerrada: false, rellenar: false, ..poligono(p) }
}

/// Una elipse inscrita en la caja.
pub fn elipse(x: f32, y: f32, w: f32, h: f32) -> Figura {
    let (rx, ry) = (w / 2.0, h / 2.0);
    let (cx, cy) = (x + rx, y + ry);
    let (kx, ky) = (rx * K, ry * K);
    Figura {
        inicio: (cx + rx, cy),
        tramos: vec![
            Tramo::Bezier((cx + rx, cy + ky), (cx + kx, cy + ry), (cx, cy + ry)),
            Tramo::Bezier((cx - kx, cy + ry), (cx - rx, cy + ky), (cx - rx, cy)),
            Tramo::Bezier((cx - rx, cy - ky), (cx - kx, cy - ry), (cx, cy - ry)),
            Tramo::Bezier((cx + kx, cy - ry), (cx + rx, cy - ky), (cx + rx, cy)),
        ],
        cerrada: true,
        rellenar: true,
        trazar: true,
    }
}

/// Rectangulo con esquinas redondeadas de radio `r` (las cuatro, o solo las
/// marcadas: arriba-izq, arriba-der, abajo-der, abajo-izq).
fn redondeado(w: f32, h: f32, r: [f32; 4]) -> Figura {
    let mut f = Figura { inicio: (r[0], 0.0), cerrada: true, rellenar: true, trazar: true, tramos: vec![] };
    let t = &mut f.tramos;
    t.push(Tramo::Linea(w - r[1], 0.0));
    if r[1] > 0.0 {
        t.push(Tramo::Bezier((w - r[1] + r[1] * K, 0.0), (w, r[1] - r[1] * K), (w, r[1])));
    }
    t.push(Tramo::Linea(w, h - r[2]));
    if r[2] > 0.0 {
        t.push(Tramo::Bezier((w, h - r[2] + r[2] * K), (w - r[2] + r[2] * K, h), (w - r[2], h)));
    }
    t.push(Tramo::Linea(r[3], h));
    if r[3] > 0.0 {
        t.push(Tramo::Bezier((r[3] - r[3] * K, h), (0.0, h - r[3] + r[3] * K), (0.0, h - r[3])));
    }
    t.push(Tramo::Linea(0.0, r[0]));
    if r[0] > 0.0 {
        t.push(Tramo::Bezier((0.0, r[0] - r[0] * K), (r[0] - r[0] * K, 0.0), (r[0], 0.0)));
    }
    f
}

/// Una forma predefinida de `w` x `h` puntos con origen en (0, 0).
/// Devuelve tambien si es una linea (sin relleno).
pub fn predefinida(nombre: &str, w: f32, h: f32, adj: &HashMap<String, f32>) -> Vec<Figura> {
    let a = |k: &str, def: f32| adj.get(k).copied().unwrap_or(def) / 100000.0;
    let ss = w.min(h);
    match nombre {
        "rect" | "flowChartProcess" | "flowChartPredefinedProcess" | "flowChartInternalStorage" | "frame" | "textBox" => {
            vec![rect(0.0, 0.0, w, h)]
        }
        "roundRect" | "flowChartAlternateProcess" => {
            let r = ss * a("adj", 16667.0);
            vec![redondeado(w, h, [r; 4])]
        }
        "round2SameRect" => {
            let r1 = ss * a("adj1", 16667.0);
            let r2 = ss * a("adj2", 0.0);
            vec![redondeado(w, h, [r1, r1, r2, r2])]
        }
        "round1Rect" => {
            let r = ss * a("adj", 16667.0);
            vec![redondeado(w, h, [0.0, r, 0.0, 0.0])]
        }
        "round2DiagRect" => {
            let r1 = ss * a("adj1", 16667.0);
            let r2 = ss * a("adj2", 0.0);
            vec![redondeado(w, h, [r1, r2, r1, r2])]
        }
        "flowChartTerminator" => vec![redondeado(w, h, [ss / 2.0; 4])],
        "ellipse" | "flowChartConnector" | "cloud" | "cloudCallout" => vec![elipse(0.0, 0.0, w, h)],
        "donut" => {
            let d = ss * a("adj", 25000.0);
            let mut i = elipse(d, d, w - 2.0 * d, h - 2.0 * d);
            // El agujero en sentido contrario para que la regla de relleno
            // (alterna) lo deje vacio.
            invertir(&mut i);
            vec![elipse(0.0, 0.0, w, h), i]
        }
        "line" | "straightConnector1" => vec![polilinea(&[(0.0, 0.0), (w, h)])],
        "bentConnector2" => vec![polilinea(&[(0.0, 0.0), (w, 0.0), (w, h)])],
        "bentConnector3" => {
            let x = w * a("adj1", 50000.0);
            vec![polilinea(&[(0.0, 0.0), (x, 0.0), (x, h), (w, h)])]
        }
        "bentConnector4" => {
            let x = w * a("adj1", 50000.0);
            let y = h * a("adj2", 50000.0);
            vec![polilinea(&[(0.0, 0.0), (x, 0.0), (x, y), (w, y), (w, h)])]
        }
        "curvedConnector3" => {
            let x = w * a("adj1", 50000.0);
            vec![Figura {
                inicio: (0.0, 0.0),
                tramos: vec![
                    Tramo::Bezier((x / 2.0, 0.0), (x, h / 4.0), (x, h / 2.0)),
                    Tramo::Bezier((x, 3.0 * h / 4.0), ((x + w) / 2.0, h), (w, h)),
                ],
                cerrada: false,
                rellenar: false,
                trazar: true,
            }]
        }
        "triangle" | "flowChartExtract" => {
            let x = w * a("adj", 50000.0);
            vec![poligono(&[(x, 0.0), (w, h), (0.0, h)])]
        }
        "flowChartDelay" => {
            let (rx, ry) = (w / 2.0, h / 2.0);
            vec![Figura {
                inicio: (0.0, 0.0),
                tramos: vec![
                    Tramo::Linea(rx, 0.0),
                    Tramo::Bezier((rx + rx * K, 0.0), (w, ry - ry * K), (w, ry)),
                    Tramo::Bezier((w, ry + ry * K), (rx + rx * K, h), (rx, h)),
                    Tramo::Linea(0.0, h),
                ],
                cerrada: true,
                rellenar: true,
                trazar: true,
            }]
        }
        "flowChartMerge" => vec![poligono(&[(0.0, 0.0), (w, 0.0), (w / 2.0, h)])],
        "rtTriangle" => vec![poligono(&[(0.0, 0.0), (w, h), (0.0, h)])],
        "diamond" | "flowChartDecision" => vec![poligono(&[(w / 2.0, 0.0), (w, h / 2.0), (w / 2.0, h), (0.0, h / 2.0)])],
        "parallelogram" | "flowChartInputOutput" => {
            let d = ss * a("adj", 25000.0);
            let d = if nombre == "flowChartInputOutput" { w / 5.0 } else { d };
            vec![poligono(&[(d, 0.0), (w, 0.0), (w - d, h), (0.0, h)])]
        }
        "trapezoid" | "flowChartManualOperation" => {
            let d = if nombre == "trapezoid" { ss * a("adj", 25000.0) } else { w / 5.0 };
            if nombre == "trapezoid" {
                vec![poligono(&[(d, 0.0), (w - d, 0.0), (w, h), (0.0, h)])]
            } else {
                vec![poligono(&[(0.0, 0.0), (w, 0.0), (w - d, h), (d, h)])]
            }
        }
        "pentagon" | "homePlate" => {
            let d = ss * a("adj", 50000.0);
            vec![poligono(&[(0.0, 0.0), (w - d, 0.0), (w, h / 2.0), (w - d, h), (0.0, h)])]
        }
        "chevron" => {
            let d = ss * a("adj", 50000.0);
            vec![poligono(&[(0.0, 0.0), (w - d, 0.0), (w, h / 2.0), (w - d, h), (0.0, h), (d, h / 2.0)])]
        }
        "hexagon" => {
            let d = ss * a("adj", 25000.0);
            vec![poligono(&[(d, 0.0), (w - d, 0.0), (w, h / 2.0), (w - d, h), (d, h), (0.0, h / 2.0)])]
        }
        "octagon" => {
            let d = ss * a("adj", 29289.0);
            vec![poligono(&[(d, 0.0), (w - d, 0.0), (w, d), (w, h - d), (w - d, h), (d, h), (0.0, h - d), (0.0, d)])]
        }
        "snip1Rect" => {
            let d = ss * a("adj", 16667.0);
            vec![poligono(&[(0.0, 0.0), (w - d, 0.0), (w, d), (w, h), (0.0, h)])]
        }
        "snip2SameRect" => {
            let d = ss * a("adj1", 16667.0);
            vec![poligono(&[(d, 0.0), (w - d, 0.0), (w, d), (w, h), (0.0, h), (0.0, d)])]
        }
        "plus" | "mathPlus" | "flowChartSummingJunction" => {
            let d = ss * a("adj", 25000.0);
            vec![poligono(&[
                (d, 0.0),
                (w - d, 0.0),
                (w - d, d),
                (w, d),
                (w, h - d),
                (w - d, h - d),
                (w - d, h),
                (d, h),
                (d, h - d),
                (0.0, h - d),
                (0.0, d),
                (d, d),
            ])]
        }
        "rightArrow" | "leftArrow" | "upArrow" | "downArrow" | "notchedRightArrow" => {
            // adj1: grosor del cuerpo (fraccion del alto), adj2: largo de la
            // punta (fraccion del lado menor).
            let horizontal = matches!(nombre, "rightArrow" | "leftArrow" | "notchedRightArrow");
            let (largo, ancho) = if horizontal { (w, h) } else { (h, w) };
            let cuerpo = ancho * a("adj1", 50000.0);
            let punta = (ss * a("adj2", 50000.0)).min(largo);
            let y0 = (ancho - cuerpo) / 2.0;
            let x1 = largo - punta;
            // En coordenadas «hacia la derecha»; luego se gira.
            let base = [(0.0, y0), (x1, y0), (x1, 0.0), (largo, ancho / 2.0), (x1, ancho), (x1, ancho - y0), (0.0, ancho - y0)];
            let p: Vec<(f32, f32)> = base
                .iter()
                .map(|&(x, y)| match nombre {
                    "leftArrow" => (largo - x, y),
                    "downArrow" => (y, x),
                    "upArrow" => (y, largo - x),
                    _ => (x, y),
                })
                .collect();
            vec![poligono(&p)]
        }
        "leftRightArrow" => {
            let cuerpo = h * a("adj1", 50000.0);
            let punta = (ss * a("adj2", 50000.0)).min(w / 2.0);
            let y0 = (h - cuerpo) / 2.0;
            vec![poligono(&[
                (0.0, h / 2.0),
                (punta, 0.0),
                (punta, y0),
                (w - punta, y0),
                (w - punta, 0.0),
                (w, h / 2.0),
                (w - punta, h),
                (w - punta, h - y0),
                (punta, h - y0),
                (punta, h),
            ])]
        }
        "upDownArrow" => {
            let cuerpo = w * a("adj1", 50000.0);
            let punta = (ss * a("adj2", 50000.0)).min(h / 2.0);
            let x0 = (w - cuerpo) / 2.0;
            vec![poligono(&[
                (w / 2.0, 0.0),
                (w, punta),
                (w - x0, punta),
                (w - x0, h - punta),
                (w, h - punta),
                (w / 2.0, h),
                (0.0, h - punta),
                (x0, h - punta),
                (x0, punta),
                (0.0, punta),
            ])]
        }
        "star5" | "star4" | "star6" | "star8" | "star10" | "star12" | "star16" | "star24" | "star32" => {
            let puntas: usize = nombre[4..].parse().unwrap_or(5);
            let interior = match puntas {
                5 => a("adj", 19098.0) * 2.0,
                4 => a("adj", 12500.0) * 2.0,
                6 => a("adj", 28868.0) * 2.0,
                _ => a("adj", 37500.0) * 2.0,
            };
            let (cx, cy) = (w / 2.0, h / 2.0);
            let mut p = Vec::new();
            for i in 0..puntas * 2 {
                let ang = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / puntas as f32;
                let r = if i % 2 == 0 { 1.0 } else { interior };
                p.push((cx + cx * r * ang.cos(), cy + cy * r * ang.sin()));
            }
            vec![poligono(&p)]
        }
        "heart" => vec![Figura {
            inicio: (w / 2.0, h / 4.0),
            tramos: vec![
                Tramo::Bezier((w / 2.0 + 3.0 * w / 8.0, -h / 4.0), (w + w / 4.0, h / 4.0), (w / 2.0, h)),
                Tramo::Bezier((-w / 4.0, h / 4.0), (w / 2.0 - 3.0 * w / 8.0, -h / 4.0), (w / 2.0, h / 4.0)),
            ],
            cerrada: true,
            rellenar: true,
            trazar: true,
        }],
        "can" | "flowChartMagneticDisk" => {
            let ry = (ss * a("adj", 25000.0)).min(h / 2.0) / 2.0;
            let mut cuerpo = Figura {
                inicio: (0.0, ry),
                tramos: vec![
                    Tramo::Linea(0.0, h - ry),
                    Tramo::Bezier((0.0, h - ry + ry * K), (w / 2.0 - w / 2.0 * K, h), (w / 2.0, h)),
                    Tramo::Bezier((w / 2.0 + w / 2.0 * K, h), (w, h - ry + ry * K), (w, h - ry)),
                    Tramo::Linea(w, ry),
                ],
                cerrada: true,
                rellenar: true,
                trazar: true,
            };
            cuerpo.tramos.push(Tramo::Bezier((w, ry + ry * K), (w / 2.0 + w / 2.0 * K, 2.0 * ry), (w / 2.0, 2.0 * ry)));
            cuerpo.tramos.push(Tramo::Bezier((w / 2.0 - w / 2.0 * K, 2.0 * ry), (0.0, ry + ry * K), (0.0, ry)));
            vec![cuerpo, elipse(0.0, 0.0, w, 2.0 * ry)]
        }
        "wedgeRectCallout" | "wedgeRoundRectCallout" | "wedgeEllipseCallout" => {
            // La caja y una cuna desde el borde mas cercano hasta la punta
            // (`adj1`, `adj2`: la punta respecto al centro, en fracciones).
            let tx = w / 2.0 + w * a("adj1", -20833.0);
            let ty = h / 2.0 + h * a("adj2", 62500.0);
            let mut v = match nombre {
                "wedgeRoundRectCallout" => vec![redondeado(w, h, [ss * a("adj3", 16667.0); 4])],
                "wedgeEllipseCallout" => vec![elipse(0.0, 0.0, w, h)],
                _ => vec![rect(0.0, 0.0, w, h)],
            };
            let (dx, dy) = ((tx - w / 2.0) / w.max(1.0), (ty - h / 2.0) / h.max(1.0));
            let cuna = if dy.abs() >= dx.abs() {
                let y = if dy > 0.0 { h } else { 0.0 };
                let c = tx.clamp(w * 0.2, w * 0.8);
                [(c - w / 12.0, y), (tx, ty), (c + w / 12.0, y)]
            } else {
                let x = if dx > 0.0 { w } else { 0.0 };
                let c = ty.clamp(h * 0.2, h * 0.8);
                [(x, c - h / 12.0), (tx, ty), (x, c + h / 12.0)]
            };
            if !(0.0..=w).contains(&tx) || !(0.0..=h).contains(&ty) {
                v.push(poligono(&cuna));
            }
            v
        }
        "flowChartDocument" | "flowChartMultidocument" | "foldedCorner" | "bevel" | "cube" | "plaque" => vec![rect(0.0, 0.0, w, h)],
        _ => vec![rect(0.0, 0.0, w, h)],
    }
}

fn invertir(f: &mut Figura) {
    // Recorre la figura al reves: reconstruye los puntos de cada tramo.
    let mut puntos = vec![f.inicio];
    for t in &f.tramos {
        puntos.push(match *t {
            Tramo::Linea(x, y) => (x, y),
            Tramo::Bezier(_, _, p) => p,
        });
    }
    let mut nuevos = Vec::new();
    for i in (0..f.tramos.len()).rev() {
        let destino = puntos[i];
        nuevos.push(match f.tramos[i] {
            Tramo::Linea(..) => Tramo::Linea(destino.0, destino.1),
            Tramo::Bezier(c1, c2, _) => Tramo::Bezier(c2, c1, destino),
        });
    }
    f.inicio = *puntos.last().unwrap_or(&f.inicio);
    f.tramos = nuevos;
}

/// Si la forma es de las que no tienen relleno (lineas y conectores).
pub fn es_linea(nombre: &str) -> bool {
    matches!(
        nombre,
        "line"
            | "straightConnector1"
            | "bentConnector2"
            | "bentConnector3"
            | "bentConnector4"
            | "curvedConnector2"
            | "curvedConnector3"
            | "curvedConnector4"
            | "arc"
    )
}

/// Una geometria propia (`a:custGeom/a:pathLst`) escalada a `w` x `h`.
/// Las guias de una geometria (`a:avLst` y `a:gdLst`) evaluadas para una
/// forma de `w` x `h` EMU, con las variables de siempre (`w`, `h`, `hc`,
/// `ss`, `cd4`...).
pub fn guias(cust: &Nodo, w: f64, h: f64) -> HashMap<String, f64> {
    let mut v: HashMap<String, f64> = HashMap::new();
    let ss = w.min(h);
    let ls = w.max(h);
    for (k, x) in [
        ("w", w),
        ("h", h),
        ("l", 0.0),
        ("t", 0.0),
        ("r", w),
        ("b", h),
        ("hc", w / 2.0),
        ("vc", h / 2.0),
        ("ss", ss),
        ("ls", ls),
        ("cd2", 10800000.0),
        ("cd4", 5400000.0),
        ("cd8", 2700000.0),
        ("3cd4", 16200000.0),
        ("3cd8", 8100000.0),
        ("5cd8", 13500000.0),
        ("7cd8", 18900000.0),
    ] {
        v.insert(k.into(), x);
    }
    for d in [2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0, 16.0, 32.0] {
        v.insert(format!("wd{d}"), w / d);
        v.insert(format!("hd{d}"), h / d);
        v.insert(format!("ssd{d}"), ss / d);
    }
    for lista in ["avLst", "gdLst"] {
        if let Some(l) = cust.hijo(lista) {
            for gd in l.elementos().filter(|e| e.nombre == "gd") {
                let x = evaluar(gd.atributo("fmla"), &v);
                v.insert(gd.atributo("name").to_string(), x);
            }
        }
    }
    v
}

/// Una formula de guia de DrawingML (`*/ a b c`, `+- a b c`, `sin r a`...).
fn evaluar(fmla: &str, v: &HashMap<String, f64>) -> f64 {
    let p: Vec<&str> = fmla.split_whitespace().collect();
    let a = |i: usize| -> f64 {
        let s = p.get(i).copied().unwrap_or("0");
        s.parse::<f64>().ok().or_else(|| v.get(s).copied()).unwrap_or(0.0)
    };
    let div = |x: f64, y: f64| if y.abs() < 1e-12 { 0.0 } else { x / y };
    let ang = |x: f64| (x / 60000.0).to_radians();
    let r = match p.first().copied().unwrap_or("") {
        "val" => a(1),
        "*/" => div(a(1) * a(2), a(3)),
        "+-" => a(1) + a(2) - a(3),
        "+/" => div(a(1) + a(2), a(3)),
        "?:" => {
            if a(1) > 0.0 {
                a(2)
            } else {
                a(3)
            }
        }
        "abs" => a(1).abs(),
        "at2" => a(2).atan2(a(1)).to_degrees() * 60000.0,
        "cat2" => a(1) * a(3).atan2(a(2)).cos(),
        "sat2" => a(1) * a(3).atan2(a(2)).sin(),
        "cos" => a(1) * ang(a(2)).cos(),
        "sin" => a(1) * ang(a(2)).sin(),
        "tan" => a(1) * ang(a(2)).tan(),
        "max" => a(1).max(a(2)),
        "min" => a(1).min(a(2)),
        "mod" => (a(1) * a(1) + a(2) * a(2) + a(3) * a(3)).sqrt(),
        "pin" => a(2).max(a(1)).min(a(3)),
        "sqrt" => a(1).max(0.0).sqrt(),
        _ => 0.0,
    };
    if r.is_finite() { r } else { 0.0 }
}

pub fn propia(cust: &Nodo, w: f32, h: f32) -> Vec<Figura> {
    let mut salida = Vec::new();
    let Some(lista) = cust.hijo("pathLst") else {
        return salida;
    };
    let g = guias(cust, w as f64 * 12700.0, h as f64 * 12700.0);
    let num = |n: &Nodo, k: &str| -> Option<f32> {
        let s = n.atributo(k).trim();
        if s.is_empty() {
            return None;
        }
        s.parse::<f64>().ok().or_else(|| g.get(s).copied()).map(|x| x as f32)
    };
    for path in lista.elementos().filter(|e| e.nombre == "path") {
        let pw = crate::paquete::attr_num(path, "w", 1.0).filter(|v| *v > 0.0);
        let ph = crate::paquete::attr_num(path, "h", 1.0).filter(|v| *v > 0.0);
        // Sin `w`/`h` el trayecto viene ya en EMU de la forma.
        let sx = pw.map(|pw| w / pw).unwrap_or(1.0 / 12700.0);
        let sy = ph.map(|ph| h / ph).unwrap_or(1.0 / 12700.0);
        let rellenar = path.atributo("fill") != "none";
        let trazar = path.atributo("stroke") != "0" && path.atributo("stroke") != "false";
        let punto = |pt: &Nodo| (num(pt, "x").unwrap_or(0.0) * sx, num(pt, "y").unwrap_or(0.0) * sy);
        let mut actual: Option<Figura> = None;
        let mut pos = (0.0f32, 0.0f32);
        for orden in path.elementos() {
            let pts: Vec<(f32, f32)> = orden.elementos().filter(|e| e.nombre == "pt").map(punto).collect();
            match orden.nombre.as_str() {
                "moveTo" => {
                    if let Some(f) = actual.take() {
                        salida.push(f);
                    }
                    pos = pts.first().copied().unwrap_or((0.0, 0.0));
                    actual = Some(Figura { inicio: pos, tramos: vec![], cerrada: false, rellenar, trazar });
                }
                "lnTo" => {
                    if let Some(&p) = pts.first() {
                        let f = actual.get_or_insert_with(|| Figura { inicio: pos, rellenar, trazar, ..Default::default() });
                        f.tramos.push(Tramo::Linea(p.0, p.1));
                        pos = p;
                    }
                }
                "cubicBezTo" if pts.len() >= 3 => {
                    let f = actual.get_or_insert_with(|| Figura { inicio: pos, rellenar, trazar, ..Default::default() });
                    f.tramos.push(Tramo::Bezier(pts[0], pts[1], pts[2]));
                    pos = pts[2];
                }
                "quadBezTo" if pts.len() >= 2 => {
                    let f = actual.get_or_insert_with(|| Figura { inicio: pos, rellenar, trazar, ..Default::default() });
                    let (c, p) = (pts[0], pts[1]);
                    let c1 = (pos.0 + 2.0 / 3.0 * (c.0 - pos.0), pos.1 + 2.0 / 3.0 * (c.1 - pos.1));
                    let c2 = (p.0 + 2.0 / 3.0 * (c.0 - p.0), p.1 + 2.0 / 3.0 * (c.1 - p.1));
                    f.tramos.push(Tramo::Bezier(c1, c2, p));
                    pos = p;
                }
                "arcTo" => {
                    let wr = num(orden, "wR").unwrap_or(0.0) * sx;
                    let hr = num(orden, "hR").unwrap_or(0.0) * sy;
                    let st = num(orden, "stAng").unwrap_or(0.0) / 60000.0;
                    let sw = num(orden, "swAng").unwrap_or(0.0) / 60000.0;
                    let f = actual.get_or_insert_with(|| Figura { inicio: pos, rellenar, trazar, ..Default::default() });
                    pos = arco(f, pos, wr, hr, st, sw);
                }
                "close" => {
                    if let Some(mut f) = actual.take() {
                        f.cerrada = true;
                        pos = f.inicio;
                        salida.push(f);
                    }
                }
                _ => {}
            }
        }
        if let Some(f) = actual.take() {
            salida.push(f);
        }
    }
    salida
}

/// Anade a `f` el arco de DrawingML que empieza en `pos`: elipse de radios
/// `wr`, `hr`, desde el angulo `st` barriendo `sw` grados. Devuelve el punto
/// final.
pub fn arco(f: &mut Figura, pos: (f32, f32), wr: f32, hr: f32, st: f32, sw: f32) -> (f32, f32) {
    if wr <= 0.0 || hr <= 0.0 || sw == 0.0 {
        return pos;
    }
    let rad = |g: f32| g.to_radians();
    // El centro: `pos` es el punto de la elipse en el angulo `st`.
    let (cx, cy) = (pos.0 - wr * rad(st).cos(), pos.1 - hr * rad(st).sin());
    let trozos = (sw.abs() / 90.0).ceil().max(1.0) as usize;
    let paso = sw / trozos as f32;
    let mut a0 = st;
    let mut fin = pos;
    for _ in 0..trozos {
        let a1 = a0 + paso;
        let k = 4.0 / 3.0 * (rad(paso) / 4.0).tan();
        let (c0, s0) = (rad(a0).cos(), rad(a0).sin());
        let (c1, s1) = (rad(a1).cos(), rad(a1).sin());
        let p0 = (cx + wr * c0, cy + hr * s0);
        let p3 = (cx + wr * c1, cy + hr * s1);
        let p1 = (p0.0 - k * wr * s0, p0.1 + k * hr * c0);
        let p2 = (p3.0 + k * wr * s1, p3.1 - k * hr * c1);
        f.tramos.push(Tramo::Bezier(p1, p2, p3));
        fin = p3;
        a0 = a1;
    }
    fin
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_docs::xml::leer;

    #[test]
    fn una_forma_desconocida_sale_como_rectangulo() {
        let f = predefinida("loQueSea", 10.0, 20.0, &HashMap::new());
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].tramos.len(), 3);
    }

    #[test]
    fn la_flecha_respeta_su_caja() {
        let f = predefinida("rightArrow", 100.0, 40.0, &HashMap::new());
        let mut xs = vec![f[0].inicio.0];
        for t in &f[0].tramos {
            if let Tramo::Linea(x, _) = t {
                xs.push(*x);
            }
        }
        assert!(xs.iter().all(|x| (0.0..=100.0).contains(x)));
        assert!(xs.contains(&100.0));
    }

    #[test]
    fn una_geometria_propia_se_escala() {
        let n = leer(r#"<a:custGeom><a:pathLst><a:path w="100" h="100"><a:moveTo><a:pt x="0" y="0"/></a:moveTo><a:lnTo><a:pt x="100" y="100"/></a:lnTo><a:close/></a:path></a:pathLst></a:custGeom>"#).unwrap();
        let f = propia(&n, 50.0, 20.0);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].tramos, vec![Tramo::Linea(50.0, 20.0)]);
        assert!(f[0].cerrada);
    }

    #[test]
    fn el_arco_de_un_cuarto_llega_a_su_sitio() {
        let mut f = Figura::default();
        let fin = arco(&mut f, (10.0, 0.0), 10.0, 10.0, 0.0, 90.0);
        assert!((fin.0 - 0.0).abs() < 1e-3 && (fin.1 - 10.0).abs() < 1e-3, "{fin:?}");
    }
}
