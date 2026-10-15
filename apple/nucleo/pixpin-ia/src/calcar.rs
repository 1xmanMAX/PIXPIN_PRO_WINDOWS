//! **Calcar una imagen al lienzo**: de una foto o un dibujo, sus contornos y
//! sus manchas de color como elementos del lienzo.
//!
//! Pedido del usuario (13-oct-2026): «que pueda copiar lo de una imagen en un
//! lienzo». Un modelo de 2B no puede escribir los cientos de coordenadas que
//! hacen falta para copiar una forma (medido: describe 3-6 figuras sueltas y
//! alucina el resto), asi que el trabajo lo hace aqui un **algoritmo
//! clasico**, determinista y rapido; el modelo solo elige los parametros
//! (detalle, con o sin colores, solo contornos, una zona) con la herramienta
//! `calcar_imagen`.
//!
//! ## Dos caminos
//!
//! - **Manchas de color** (lo normal): k-medias en RGB sobre la imagen
//!   reducida → etiqueta por pixel, alisada con un filtro de mayoria → cada
//!   mancha conexa se recorre con *marching squares* (contornos y agujeros,
//!   sin casos raros) → Ramer-Douglas-Peucker → suavizado que respeta las
//!   esquinas. Sale como `pixpin-region` (el poligono relleno con agujeros
//!   del bote de pintura, que el lienzo del PC y el del movil ya pintan).
//!   El fondo (el color que mas toca el borde) no se dibuja.
//! - **Solo contornos**: Canny (gauss, Sobel, supresion de no maximos,
//!   histeresis con umbrales sacados de la propia imagen) → seguimiento de
//!   las cadenas de pixeles → RDP → suavizado. Sale como lineas (`line`),
//!   que en el lienzo se editan nodo a nodo.
//!
//! Todo con tope: la imagen se reduce a 160/256/384 px de lado segun el
//! detalle, y salen como mucho [`Detalle::tope`] elementos, los mas grandes.
//! Una foto de 768 px se calca en decimas de segundo (prueba abajo).

use std::collections::HashMap;

use crate::herramientas::{ALTO_LIENZO, ANCHO_LIENZO};

/// Cuanto detalle se quiere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detalle {
    Bajo,
    Medio,
    Alto,
}

impl Detalle {
    /// Lado mayor de la imagen con la que se trabaja.
    fn lado(self) -> u32 {
        match self {
            Detalle::Bajo => 160,
            Detalle::Medio => 256,
            Detalle::Alto => 384,
        }
    }
    /// Cuantos colores busca k-medias.
    fn colores(self) -> usize {
        match self {
            Detalle::Bajo => 5,
            Detalle::Medio => 8,
            Detalle::Alto => 12,
        }
    }
    /// Tolerancia de RDP, en pixeles de la imagen reducida.
    fn tolerancia(self) -> f64 {
        match self {
            Detalle::Bajo => 1.4,
            Detalle::Medio => 1.0,
            Detalle::Alto => 0.7,
        }
    }
    /// La mancha mas pequena que se dibuja, en fraccion de la imagen.
    fn area_minima(self) -> f64 {
        match self {
            Detalle::Bajo => 0.0015,
            Detalle::Medio => 0.0004,
            Detalle::Alto => 0.00015,
        }
    }
    /// Lo mas corto que se traza en «solo contornos», en pixeles.
    fn cadena_minima(self) -> usize {
        match self {
            Detalle::Bajo => 14,
            Detalle::Medio => 9,
            Detalle::Alto => 6,
        }
    }
    /// Cuantos elementos como mucho.
    pub fn tope(self) -> usize {
        match self {
            Detalle::Bajo => 40,
            Detalle::Medio => 90,
            Detalle::Alto => 160,
        }
    }
}

/// Lo que elige el modelo.
#[derive(Debug, Clone, PartialEq)]
pub struct Parametros {
    pub detalle: Detalle,
    /// Manchas de color (y lineas del color de la imagen); sin ello, todo en
    /// negro.
    pub colores: bool,
    /// Solo las lineas de los bordes, sin rellenos.
    pub solo_contornos: bool,
    /// Un trozo de la imagen, en fracciones `[x, y, ancho, alto]` de 0 a 1.
    pub zona: Option<[f64; 4]>,
    /// Que adjunto calcar; `None`: la ultima imagen de la conversacion.
    pub imagen: Option<String>,
}

/// **Lo que el modelo eligio sin que se lo pidieran, corregido con el
/// pedido.** Medido el 14-oct: a «dibuja este perrito de la imagen adjunta»
/// el 2B pide `solo_contornos` y sin colores, y el perrito sale en seis rayas
/// negras. Solo contornos si el pedido habla de contornos, bordes, lineas o
/// silueta; en blanco y negro solo si lo dice.
pub fn ajustar_al_pedido(p: &mut Parametros, pedido: &str) {
    let q = crate::acciones::normalizar(pedido);
    let dice = |raices: &[&str]| raices.iter().any(|r| q.contains(r));
    if p.solo_contornos && !dice(&["contorno", "borde", "linea", "silueta", "trazo", "perfil", "solo las rayas"]) {
        p.solo_contornos = false;
    }
    if !p.colores && !dice(&["blanco y negro", "sin color", "en negro", "monocrom", "gris"]) {
        p.colores = true;
    }
}

impl Default for Parametros {
    fn default() -> Self {
        Parametros {
            detalle: Detalle::Medio,
            colores: true,
            solo_contornos: false,
            zona: None,
            imagen: None,
        }
    }
}

/// Un elemento calcado, ya en coordenadas del lienzo de 800x600.
#[derive(Debug, Clone, PartialEq)]
pub enum Trazo {
    /// Una mancha rellena con sus agujeros.
    Mancha {
        contorno: Vec<(f64, f64)>,
        huecos: Vec<Vec<(f64, f64)>>,
        color: String,
    },
    /// Una linea (abierta o cerrada) del borde.
    Linea {
        puntos: Vec<(f64, f64)>,
        cerrada: bool,
        color: String,
    },
    /// El fondo, si no era blanco: un rectangulo lleno detras de todo.
    Fondo { caja: [f64; 4], color: String },
}

/// El resultado y como volver a la imagen original (para las pruebas y para
/// quien quiera colocarlo encima de la foto).
#[derive(Debug, Clone, PartialEq)]
pub struct Calcado {
    pub trazos: Vec<Trazo>,
    /// Lienzo = imagen original * `escala` + `desplazamiento`.
    pub escala: f64,
    pub desplazamiento: (f64, f64),
}

/// **Calca** una imagen RGBA (`ancho * alto * 4` bytes; lo transparente
/// cuenta como blanco). Nunca falla: una imagen vacia o rara da pocos o
/// ningun trazo.
pub fn calcar(rgba: &[u8], ancho: u32, alto: u32, p: &Parametros) -> Calcado {
    let vacio = Calcado {
        trazos: Vec::new(),
        escala: 1.0,
        desplazamiento: (0.0, 0.0),
    };
    if ancho == 0 || alto == 0 || rgba.len() < (ancho as usize * alto as usize * 4) {
        return vacio;
    }
    // 1. Zona y reduccion.
    let (zx, zy, zw, zh) = match p.zona {
        Some([x, y, w, h]) if w > 0.0 && h > 0.0 => {
            let x0 = (x.clamp(0.0, 1.0) * ancho as f64) as u32;
            let y0 = (y.clamp(0.0, 1.0) * alto as f64) as u32;
            let x1 = (((x + w).clamp(0.0, 1.0)) * ancho as f64).ceil() as u32;
            let y1 = (((y + h).clamp(0.0, 1.0)) * alto as f64).ceil() as u32;
            (x0, y0, x1.saturating_sub(x0).max(1), y1.saturating_sub(y0).max(1))
        }
        _ => (0, 0, ancho, alto),
    };
    let zw = zw.min(ancho - zx.min(ancho - 1));
    let zh = zh.min(alto - zy.min(alto - 1));
    let lado = p.detalle.lado();
    let f = (lado as f64 / zw.max(zh) as f64).min(1.0);
    let (w, h) = (((zw as f64 * f).round() as usize).max(1), ((zh as f64 * f).round() as usize).max(1));
    let img = reducir(rgba, ancho as usize, (zx as usize, zy as usize, zw as usize, zh as usize), w, h);

    // 2. A coordenadas del lienzo: cabe en 800x600 con margen y centrada.
    let margen = 20.0;
    let esc_red = ((ANCHO_LIENZO - 2.0 * margen) / w as f64).min((ALTO_LIENZO - 2.0 * margen) / h as f64);
    let dx = (ANCHO_LIENZO - w as f64 * esc_red) / 2.0;
    let dy = (ALTO_LIENZO - h as f64 * esc_red) / 2.0;
    let a_lienzo = |(x, y): (f64, f64)| (x * esc_red + dx, y * esc_red + dy);
    // De la imagen original al lienzo (zona y reduccion incluidas).
    let sx = w as f64 / zw as f64;
    let escala = sx * esc_red;
    let desplazamiento = (dx - zx as f64 * escala, dy - zy as f64 * escala);

    let mut trazos = if p.solo_contornos {
        contornos(&img, w, h, p)
    } else {
        manchas(&img, w, h, p)
    };
    // 3. Tope: los mas grandes primero (y el fondo, si lo hay, delante).
    let tope = p.detalle.tope();
    if trazos.len() > tope {
        let fondo = matches!(trazos.first(), Some(Trazo::Fondo { .. }));
        let resto = if fondo { trazos.split_off(1) } else { std::mem::take(&mut trazos) };
        let mut con_tam: Vec<(f64, Trazo)> = resto.into_iter().map(|t| (tamano(&t), t)).collect();
        con_tam.sort_by(|a, b| b.0.total_cmp(&a.0));
        con_tam.truncate(tope - usize::from(fondo));
        // Se conserva el orden de pintado (grande debajo).
        trazos.extend(con_tam.into_iter().map(|(_, t)| t));
    }
    for t in &mut trazos {
        match t {
            Trazo::Mancha { contorno, huecos, .. } => {
                contorno.iter_mut().for_each(|q| *q = a_lienzo(*q));
                huecos.iter_mut().flatten().for_each(|q| *q = a_lienzo(*q));
            }
            Trazo::Linea { puntos, .. } => puntos.iter_mut().for_each(|q| *q = a_lienzo(*q)),
            Trazo::Fondo { caja, .. } => {
                let (x, y) = a_lienzo((0.0, 0.0));
                *caja = [x, y, w as f64 * esc_red, h as f64 * esc_red];
            }
        }
    }
    Calcado {
        trazos,
        escala,
        desplazamiento,
    }
}

/// Lo que «pesa» un trazo, para quedarse con los importantes.
fn tamano(t: &Trazo) -> f64 {
    match t {
        Trazo::Mancha { contorno, .. } => area(contorno).abs(),
        Trazo::Linea { puntos, .. } => puntos.windows(2).map(|p| dist(p[0], p[1])).sum::<f64>() * 3.0,
        Trazo::Fondo { .. } => f64::MAX,
    }
}

/// Media de cajas de la zona a `w x h`, en RGB sobre blanco.
fn reducir(rgba: &[u8], ancho: usize, (zx, zy, zw, zh): (usize, usize, usize, usize), w: usize, h: usize) -> Vec<[f32; 3]> {
    let mut out = vec![[0f32; 3]; w * h];
    for y in 0..h {
        let y0 = zy + y * zh / h;
        let y1 = (zy + ((y + 1) * zh).div_ceil(h)).max(y0 + 1);
        for x in 0..w {
            let x0 = zx + x * zw / w;
            let x1 = (zx + ((x + 1) * zw).div_ceil(w)).max(x0 + 1);
            let mut s = [0f32; 3];
            let mut n = 0f32;
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let i = (yy * ancho + xx) * 4;
                    let Some(px) = rgba.get(i..i + 4) else { continue };
                    let a = px[3] as f32 / 255.0;
                    for c in 0..3 {
                        s[c] += px[c] as f32 * a + 255.0 * (1.0 - a);
                    }
                    n += 1.0;
                }
            }
            if n > 0.0 {
                out[y * w + x] = [s[0] / n, s[1] / n, s[2] / n];
            } else {
                out[y * w + x] = [255.0; 3];
            }
        }
    }
    out
}

fn hex(c: [f32; 3]) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        c[0].round().clamp(0.0, 255.0) as u8,
        c[1].round().clamp(0.0, 255.0) as u8,
        c[2].round().clamp(0.0, 255.0) as u8
    )
}

fn d2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

// ---------------------------------------------------------------------------
// Manchas de color

/// k-medias determinista: el primer centro es el color mas comun; los
/// siguientes, el pixel mas lejos de los que ya hay (asi un color pequeno
/// pero distinto, una puerta azul, tiene su centro).
fn k_medias(img: &[[f32; 3]], k: usize) -> (Vec<[f32; 3]>, Vec<u8>) {
    let paso = (img.len() / 20_000).max(1);
    let muestra: Vec<[f32; 3]> = img.iter().step_by(paso).copied().collect();
    // Histograma de 16 niveles por canal, con la media de cada cubo: los
    // centros salen de los cubos con bastantes pixeles. Asi un color
    // concentrado (una raya verde) gana a los de mezcla de los bordes, que
    // son muchos pero repartidos en cubos de pocos pixeles cada uno.
    let mut cubos: HashMap<[u8; 3], [f64; 4]> = HashMap::new();
    for c in &muestra {
        let e = cubos.entry([(c[0] / 16.0) as u8, (c[1] / 16.0) as u8, (c[2] / 16.0) as u8]).or_default();
        for j in 0..3 {
            e[j] += c[j] as f64;
        }
        e[3] += 1.0;
    }
    let minimo = (muestra.len() as f64 / 400.0).max(3.0);
    let mut candidatos: Vec<([f32; 3], f64)> = cubos
        .values()
        .map(|s| ([(s[0] / s[3]) as f32, (s[1] / s[3]) as f32, (s[2] / s[3]) as f32], s[3]))
        .collect();
    candidatos.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0[0].total_cmp(&b.0[0])));
    let mut centros = vec![candidatos.first().map(|c| c.0).unwrap_or([255.0; 3])];
    let fuertes: Vec<[f32; 3]> = candidatos.iter().filter(|c| c.1 >= minimo).map(|c| c.0).collect();
    while centros.len() < k {
        let Some((mejor, dist)) = fuertes
            .iter()
            .map(|c| (c, centros.iter().map(|z| d2(*c, *z)).fold(f32::MAX, f32::min)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
        else {
            break;
        };
        // Ya no hay colores distintos (menos de ~20 de distancia).
        if dist < 400.0 {
            break;
        }
        centros.push(*mejor);
    }
    for _ in 0..8 {
        let mut suma = vec![[0f64; 4]; centros.len()];
        for c in &muestra {
            let i = cercano(&centros, *c);
            for j in 0..3 {
                suma[i][j] += c[j] as f64;
            }
            suma[i][3] += 1.0;
        }
        for (z, s) in centros.iter_mut().zip(&suma) {
            if s[3] > 0.0 {
                *z = [(s[0] / s[3]) as f32, (s[1] / s[3]) as f32, (s[2] / s[3]) as f32];
            }
        }
    }
    let etiquetas = img.iter().map(|c| cercano(&centros, *c) as u8).collect();
    (centros, etiquetas)
}

fn cercano(centros: &[[f32; 3]], c: [f32; 3]) -> usize {
    let mut mejor = 0;
    let mut md = f32::MAX;
    for (i, z) in centros.iter().enumerate() {
        let d = d2(c, *z);
        if d < md {
            md = d;
            mejor = i;
        }
    }
    mejor
}

/// Filtro de mayoria 3x3: quita el «sal y pimienta» de los bordes
/// suavizados (antialias) sin mover los bordes de verdad.
fn mayoria(et: &[u8], w: usize, h: usize, k: usize) -> Vec<u8> {
    let mut out = et.to_vec();
    let mut cuenta = vec![0u8; k];
    for y in 0..h {
        for x in 0..w {
            cuenta.iter_mut().for_each(|c| *c = 0);
            for yy in y.saturating_sub(1)..(y + 2).min(h) {
                for xx in x.saturating_sub(1)..(x + 2).min(w) {
                    cuenta[et[yy * w + xx] as usize] += 1;
                }
            }
            let propio = et[y * w + x] as usize;
            let (mejor, n) = cuenta.iter().enumerate().max_by_key(|(_, n)| **n).map(|(i, n)| (i, *n)).unwrap();
            if n >= 6 && mejor != propio {
                out[y * w + x] = mejor as u8;
            }
        }
    }
    out
}

/// **Quita los colores de mezcla**: al reducir la imagen, el borde entre
/// rojo y blanco sale rosa, y k-medias le da su propio color; calcado, eso es
/// un anillo rosa alrededor de cada figura. Un grupo cuyo color esta entre
/// otros dos y que es casi todo borde (sus pixeles tocan otro color) se
/// reparte entre esos dos. Una mancha morada de verdad, maciza, no es borde
/// y se queda.
fn fusionar_mezclas(centros: &[[f32; 3]], et: &mut [u8], w: usize, h: usize) {
    let k = centros.len();
    for _ in 0..k {
        let mut cuenta = vec![0usize; k];
        let mut de_borde = vec![0usize; k];
        for y in 0..h {
            for x in 0..w {
                let e = et[y * w + x];
                cuenta[e as usize] += 1;
                let distinto = (x > 0 && et[y * w + x - 1] != e)
                    || (x + 1 < w && et[y * w + x + 1] != e)
                    || (y > 0 && et[(y - 1) * w + x] != e)
                    || (y + 1 < h && et[(y + 1) * w + x] != e);
                if distinto {
                    de_borde[e as usize] += 1;
                }
            }
        }
        let mut cambio = false;
        for c in 0..k {
            if cuenta[c] == 0 || (de_borde[c] as f64) < 0.5 * cuenta[c] as f64 {
                continue;
            }
            // Los dos colores entre los que esta.
            let mut mejor: Option<(usize, usize, f32)> = None;
            for a in 0..k {
                for b in a + 1..k {
                    if a == c || b == c || cuenta[a] == 0 || cuenta[b] == 0 {
                        continue;
                    }
                    let (pa, pb, pc) = (centros[a], centros[b], centros[c]);
                    let ab = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
                    let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
                    if l2 <= 0.0 {
                        continue;
                    }
                    let t = ((pc[0] - pa[0]) * ab[0] + (pc[1] - pa[1]) * ab[1] + (pc[2] - pa[2]) * ab[2]) / l2;
                    if !(0.05..=0.95).contains(&t) {
                        continue;
                    }
                    let q = [pa[0] + t * ab[0], pa[1] + t * ab[1], pa[2] + t * ab[2]];
                    let d = d2(pc, q);
                    if d < 45.0 * 45.0 && mejor.is_none_or(|m| d < m.2) {
                        mejor = Some((a, b, d));
                    }
                }
            }
            if let Some((a, b, _)) = mejor {
                // Cada pixel, al de los dos que mas tenga alrededor; si no
                // toca ninguno, al color mas cercano de los dos.
                let copia = et.to_vec();
                for y in 0..h {
                    for x in 0..w {
                        let i = y * w + x;
                        if copia[i] as usize != c {
                            continue;
                        }
                        let mut na = 0;
                        let mut nb = 0;
                        for yy in y.saturating_sub(1)..(y + 2).min(h) {
                            for xx in x.saturating_sub(1)..(x + 2).min(w) {
                                let v = copia[yy * w + xx] as usize;
                                na += usize::from(v == a);
                                nb += usize::from(v == b);
                            }
                        }
                        et[i] = if na > nb {
                            a as u8
                        } else if nb > na {
                            b as u8
                        } else if d2(centros[c], centros[a]) <= d2(centros[c], centros[b]) {
                            a as u8
                        } else {
                            b as u8
                        };
                    }
                }
                cambio = true;
                break;
            }
        }
        if !cambio {
            break;
        }
    }
}

/// Las manchas conexas (4-vecindad) de la etiqueta `l`: sus pixeles y su
/// caja `(x0, y0, x1, y1)` (x1, y1 incluidos).
#[allow(clippy::type_complexity)] // (pixeles, caja): solo se usa aqui al lado
fn componentes(et: &[u8], w: usize, h: usize, l: u8) -> Vec<(Vec<usize>, (usize, usize, usize, usize))> {
    let mut visto = vec![false; w * h];
    let mut salida = Vec::new();
    for inicio in 0..w * h {
        if visto[inicio] || et[inicio] != l {
            continue;
        }
        let mut pila = vec![inicio];
        visto[inicio] = true;
        let mut pixeles = Vec::new();
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
        while let Some(i) = pila.pop() {
            pixeles.push(i);
            let (x, y) = (i % w, i / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            let vecinos = [
                (x > 0).then(|| i - 1),
                (x + 1 < w).then(|| i + 1),
                (y > 0).then(|| i - w),
                (y + 1 < h).then(|| i + w),
            ];
            for j in vecinos.into_iter().flatten() {
                if !visto[j] && et[j] == l {
                    visto[j] = true;
                    pila.push(j);
                }
            }
        }
        salida.push((pixeles, (x0, y0, x1, y1)));
    }
    salida
}

fn manchas(img: &[[f32; 3]], w: usize, h: usize, p: &Parametros) -> Vec<Trazo> {
    let k = if p.colores { p.detalle.colores() } else { 2 };
    let (centros, et) = k_medias(img, k);
    let mut et = mayoria(&et, w, h, centros.len());
    fusionar_mezclas(&centros, &mut et, w, h);
    // El fondo: el color que mas toca el borde.
    let mut borde = vec![0usize; centros.len()];
    for x in 0..w {
        borde[et[x] as usize] += 1;
        borde[et[(h - 1) * w + x] as usize] += 1;
    }
    for y in 0..h {
        borde[et[y * w] as usize] += 1;
        borde[et[y * w + w - 1] as usize] += 1;
    }
    let fondo = borde.iter().enumerate().max_by_key(|(_, n)| **n).map(|(i, _)| i).unwrap_or(0);
    let mut trazos = Vec::new();
    let blanco = centros[fondo].iter().all(|c| *c > 225.0);
    if p.colores && !blanco {
        trazos.push(Trazo::Fondo {
            caja: [0.0; 4],
            color: hex(centros[fondo]),
        });
    }
    let area_min = (p.detalle.area_minima() * (w * h) as f64).max(4.0);
    let tol = p.detalle.tolerancia();
    let mut todas: Vec<(f64, Trazo)> = Vec::new();
    for (l, centro) in centros.iter().enumerate() {
        if l == fondo {
            continue;
        }
        let color = if p.colores {
            hex(*centro)
        } else {
            "#1e1e1e".to_string()
        };
        // Mancha a mancha, y solo las grandes: el ruido (miles de manchas
        // de un pixel) no llega a recorrerse.
        for (pixeles, (x0, y0, x1, y1)) in componentes(&et, w, h, l as u8) {
            if (pixeles.len() as f64) < area_min {
                continue;
            }
            let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
            let mut mascara = vec![false; bw * bh];
            for i in pixeles {
                mascara[(i / w - y0) * bw + (i % w - x0)] = true;
            }
            let mover = |v: &[(f64, f64)]| -> Vec<(f64, f64)> {
                v.iter().map(|&(x, y)| (x + x0 as f64, y + y0 as f64)).collect()
            };
            for (contorno, huecos) in regiones(&mascara, bw, bh, area_min) {
                let a = area(&contorno).abs();
                let contorno = suavizar(&simplificar_cerrado(&mover(&contorno), tol), true);
                if contorno.len() < 3 {
                    continue;
                }
                // Solo se cortan los agujeros por los que asoma el fondo: los
                // que tapa otra mancha (que se pinta encima, por ser menor)
                // dejaban rendijas blancas entre los dos colores.
                let huecos = huecos
                    .iter()
                    .filter(|hq| hueco_de_fondo(hq, &mascara, bw, bh, |x, y| et[(y + y0) * w + (x + x0)] as usize == fondo))
                    .map(|hq| suavizar(&simplificar_cerrado(&mover(hq), tol), true))
                    .filter(|hq| hq.len() >= 3)
                    .collect();
                todas.push((
                    a,
                    Trazo::Mancha {
                        contorno,
                        huecos,
                        color: color.clone(),
                    },
                ));
            }
        }
    }
    // Lo grande debajo.
    todas.sort_by(|a, b| b.0.total_cmp(&a.0));
    trazos.extend(todas.into_iter().map(|(_, t)| t));
    trazos
}

/// Los contornos de una mascara con *marching squares*: cada anillo es el
/// borde exterior de una mancha o el de uno de sus agujeros. Devuelve
/// (contorno, agujeros) de las manchas de al menos `area_min` pixeles, en
/// coordenadas continuas (el pixel (x, y) ocupa [x, x+1] x [y, y+1]).
#[allow(clippy::type_complexity)] // (contorno, agujeros), como lo pide el lienzo
pub fn regiones(mascara: &[bool], w: usize, h: usize, area_min: f64) -> Vec<(Vec<(f64, f64)>, Vec<Vec<(f64, f64)>>)> {
    let en = |x: i64, y: i64| x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h && mascara[y as usize * w + x as usize];
    // Puntos con coordenadas dobladas (enteras): el centro del pixel (x, y)
    // es (2x+1, 2y+1); el punto medio entre dos centros, entero.
    let mut vecinos: HashMap<(i64, i64), Vec<(i64, i64)>> = HashMap::new();
    let mut une = |a: (i64, i64), b: (i64, i64)| {
        vecinos.entry(a).or_default().push(b);
        vecinos.entry(b).or_default().push(a);
    };
    for j in -1..h as i64 {
        for i in -1..w as i64 {
            let tl = en(i, j);
            let tr = en(i + 1, j);
            let br = en(i + 1, j + 1);
            let bl = en(i, j + 1);
            let caso = (tl as u8) << 3 | (tr as u8) << 2 | (br as u8) << 1 | bl as u8;
            if caso == 0 || caso == 15 {
                continue;
            }
            // Centros de pixel en coordenadas dobladas: (2i+1, 2j+1).
            let (cx, cy) = (2 * i + 1, 2 * j + 1);
            let arriba = (cx + 1, cy);
            let derecha = (cx + 2, cy + 1);
            let abajo = (cx + 1, cy + 2);
            let izquierda = (cx, cy + 1);
            match caso {
                1 | 14 => une(izquierda, abajo),
                2 | 13 => une(abajo, derecha),
                3 | 12 => une(izquierda, derecha),
                4 | 11 => une(arriba, derecha),
                6 | 9 => une(arriba, abajo),
                7 | 8 => une(arriba, izquierda),
                // Sillas: lo lleno se une en diagonal (8-vecindad).
                5 => {
                    une(arriba, izquierda);
                    une(abajo, derecha);
                }
                10 => {
                    une(arriba, derecha);
                    une(izquierda, abajo);
                }
                _ => {}
            }
        }
    }
    // Anillos.
    let mut usados: HashMap<(i64, i64), bool> = HashMap::new();
    let mut claves: Vec<(i64, i64)> = vecinos.keys().copied().collect();
    claves.sort_unstable();
    let mut exteriores: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut agujeros: Vec<Vec<(f64, f64)>> = Vec::new();
    for inicio in claves {
        if usados.contains_key(&inicio) {
            continue;
        }
        let mut anillo = vec![inicio];
        usados.insert(inicio, true);
        let mut previo = inicio;
        let mut actual = vecinos[&inicio][0];
        while actual != inicio {
            usados.insert(actual, true);
            anillo.push(actual);
            let v = &vecinos[&actual];
            let siguiente = if v[0] != previo { v[0] } else { *v.get(1).unwrap_or(&v[0]) };
            if v.len() > 2 {
                // No deberia pasar con las sillas resueltas; por si acaso.
                break;
            }
            previo = actual;
            actual = siguiente;
            if anillo.len() > 4 * (w + 2) * (h + 2) {
                break;
            }
        }
        if anillo.len() < 3 {
            continue;
        }
        let puntos: Vec<(f64, f64)> = anillo.iter().map(|&(x, y)| (x as f64 / 2.0, y as f64 / 2.0)).collect();
        // ¿Exterior o agujero? Un punto medio esta entre un pixel lleno y uno
        // vacio: si el lleno queda dentro del anillo, es un borde exterior.
        let (mx, my) = anillo[0];
        let (lleno, _vacio) = if mx % 2 == 0 {
            // Entre (mx/2 - 1, ..) y (mx/2, ..) en horizontal.
            let y = (my - 1) / 2;
            let (a, b) = ((mx / 2 - 1, y), (mx / 2, y));
            if en(a.0, a.1) { (a, b) } else { (b, a) }
        } else {
            let x = (mx - 1) / 2;
            let (a, b) = ((x, my / 2 - 1), (x, my / 2));
            if en(a.0, a.1) { (a, b) } else { (b, a) }
        };
        let centro = (lleno.0 as f64 + 0.5, lleno.1 as f64 + 0.5);
        if dentro(&puntos, centro) {
            exteriores.push(puntos);
        } else {
            agujeros.push(puntos);
        }
    }
    let mut salida: Vec<(Vec<(f64, f64)>, Vec<Vec<(f64, f64)>>)> = exteriores
        .into_iter()
        .filter(|e| area(e).abs() >= area_min)
        .map(|e| (e, Vec::new()))
        .collect();
    for a in agujeros {
        if area(&a).abs() < area_min {
            continue;
        }
        // Al exterior mas pequeno que lo contiene.
        let p = a[0];
        if let Some((_, (_, hs))) = salida
            .iter_mut()
            .map(|r| (area(&r.0).abs(), r))
            .filter(|(_, r)| dentro(&r.0, p))
            .min_by(|x, y| x.0.total_cmp(&y.0))
        {
            hs.push(a);
        }
    }
    salida
}

/// Si por un agujero (en coordenadas de la caja de la mancha) asoma el
/// fondo: se mira la etiqueta de los pixeles de fuera de la mancha que
/// quedan dentro del agujero; con que la mitad sea fondo, se corta.
fn hueco_de_fondo(
    hueco: &[(f64, f64)],
    mascara: &[bool],
    bw: usize,
    bh: usize,
    es_fondo: impl Fn(usize, usize) -> bool,
) -> bool {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y) in hueco {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let (mut fondo, mut total) = (0usize, 0usize);
    for y in (y0.max(0.0) as usize)..(y1.ceil() as usize).min(bh) {
        for x in (x0.max(0.0) as usize)..(x1.ceil() as usize).min(bw) {
            if mascara[y * bw + x] || !dentro(hueco, (x as f64 + 0.5, y as f64 + 0.5)) {
                continue;
            }
            total += 1;
            fondo += usize::from(es_fondo(x, y));
        }
    }
    total > 0 && fondo * 2 >= total
}

/// Punto dentro de poligono (rayo hacia la derecha).
fn dentro(pol: &[(f64, f64)], (px, py): (f64, f64)) -> bool {
    let mut c = false;
    let n = pol.len();
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = pol[i];
        let (xj, yj) = pol[j];
        if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
            c = !c;
        }
        j = i;
    }
    c
}

/// Area con signo (formula del cordon).
pub fn area(pol: &[(f64, f64)]) -> f64 {
    let n = pol.len();
    if n < 3 {
        return 0.0;
    }
    let mut s = 0.0;
    for i in 0..n {
        let (x1, y1) = pol[i];
        let (x2, y2) = pol[(i + 1) % n];
        s += x1 * y2 - x2 * y1;
    }
    s / 2.0
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

// ---------------------------------------------------------------------------
// Simplificar y suavizar

/// Ramer-Douglas-Peucker de una polilinea abierta.
pub fn simplificar(p: &[(f64, f64)], tol: f64) -> Vec<(f64, f64)> {
    if p.len() < 3 {
        return p.to_vec();
    }
    let mut guardar = vec![false; p.len()];
    guardar[0] = true;
    guardar[p.len() - 1] = true;
    let mut pila = vec![(0, p.len() - 1)];
    while let Some((a, b)) = pila.pop() {
        if b <= a + 1 {
            continue;
        }
        let (mut peor, mut md) = (a, 0.0);
        for i in a + 1..b {
            let d = dist_a_segmento(p[i], p[a], p[b]);
            if d > md {
                md = d;
                peor = i;
            }
        }
        if md > tol {
            guardar[peor] = true;
            pila.push((a, peor));
            pila.push((peor, b));
        }
    }
    p.iter().zip(guardar).filter(|(_, g)| *g).map(|(q, _)| *q).collect()
}

/// RDP de un anillo: se parte por el punto mas lejano al primero.
fn simplificar_cerrado(p: &[(f64, f64)], tol: f64) -> Vec<(f64, f64)> {
    if p.len() < 4 {
        return p.to_vec();
    }
    let lejos = (0..p.len()).max_by(|&i, &j| dist(p[0], p[i]).total_cmp(&dist(p[0], p[j]))).unwrap_or(0);
    let mut a = simplificar(&p[..=lejos], tol);
    let mut resto = p[lejos..].to_vec();
    resto.push(p[0]);
    let b = simplificar(&resto, tol);
    a.pop();
    a.extend(b);
    a.pop();
    a
}

fn dist_a_segmento(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    if l2 == 0.0 {
        return dist(p, a);
    }
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0);
    dist(p, (a.0 + t * dx, a.1 + t * dy))
}

/// Suaviza (corte de esquinas de Chaikin) solo donde el giro es suave: un
/// circulo queda redondo y un cuadrado conserva sus esquinas.
pub fn suavizar(p: &[(f64, f64)], cerrado: bool) -> Vec<(f64, f64)> {
    let n = p.len();
    if n < 3 {
        return p.to_vec();
    }
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        let extremo = !cerrado && (i == 0 || i == n - 1);
        if extremo {
            out.push(p[i]);
            continue;
        }
        let a = p[(i + n - 1) % n];
        let b = p[i];
        let c = p[(i + 1) % n];
        let (u, v) = ((b.0 - a.0, b.1 - a.1), (c.0 - b.0, c.1 - b.1));
        let (lu, lv) = ((u.0 * u.0 + u.1 * u.1).sqrt(), (v.0 * v.0 + v.1 * v.1).sqrt());
        let coseno = if lu * lv > 0.0 { (u.0 * v.0 + u.1 * v.1) / (lu * lv) } else { 1.0 };
        // Giro de menos de ~40 grados: se redondea.
        if coseno > 0.76 {
            out.push((b.0 * 0.75 + a.0 * 0.25, b.1 * 0.75 + a.1 * 0.25));
            out.push((b.0 * 0.75 + c.0 * 0.25, b.1 * 0.75 + c.1 * 0.25));
        } else {
            out.push(b);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Solo contornos: Canny y cadenas

fn contornos(img: &[[f32; 3]], w: usize, h: usize, p: &Parametros) -> Vec<Trazo> {
    if w < 3 || h < 3 {
        return Vec::new();
    }
    let gris: Vec<f32> = img.iter().map(|c| 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]).collect();
    let suave = gauss(&gris, w, h);
    // Sobel.
    let mut mag = vec![0f32; w * h];
    let mut dir = vec![0u8; w * h];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let g = |dx: isize, dy: isize| suave[((y as isize + dy) as usize) * w + (x as isize + dx) as usize];
            let gx = g(1, -1) + 2.0 * g(1, 0) + g(1, 1) - g(-1, -1) - 2.0 * g(-1, 0) - g(-1, 1);
            let gy = g(-1, 1) + 2.0 * g(0, 1) + g(1, 1) - g(-1, -1) - 2.0 * g(0, -1) - g(1, -1);
            mag[y * w + x] = (gx * gx + gy * gy).sqrt();
            let a = gy.atan2(gx).to_degrees().rem_euclid(180.0);
            dir[y * w + x] = if !(22.5..157.5).contains(&a) {
                0
            } else if a < 67.5 {
                1
            } else if a < 112.5 {
                2
            } else {
                3
            };
        }
    }
    // Supresion de no maximos.
    let mut nms = vec![0f32; w * h];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            let (a, b) = match dir[i] {
                0 => (i - 1, i + 1),
                1 => (i - w - 1, i + w + 1),
                2 => (i - w, i + w),
                _ => (i - w + 1, i + w - 1),
            };
            if mag[i] >= mag[a] && mag[i] >= mag[b] {
                nms[i] = mag[i];
            }
        }
    }
    // Umbrales de la propia imagen.
    let maximo = nms.iter().copied().fold(0.0, f32::max);
    if maximo < 8.0 {
        return Vec::new();
    }
    let mut vivos: Vec<f32> = nms.iter().copied().filter(|m| *m > maximo * 0.02).collect();
    vivos.sort_by(|a, b| a.total_cmp(b));
    let percentil = match p.detalle {
        Detalle::Bajo => 0.85,
        Detalle::Medio => 0.7,
        Detalle::Alto => 0.55,
    };
    // Tope a media altura del maximo: en un dibujo limpio el percentil cae
    // en los bordes mas fuertes y dejaba fuera figuras enteras de menos
    // contraste (un circulo rojo junto a un cuadrado azul).
    let alto = vivos[((vivos.len() - 1) as f32 * percentil) as usize]
        .min(maximo * 0.45)
        .max(maximo * 0.1)
        .max(20.0);
    let bajo = alto * 0.4;
    // Histeresis.
    let mut borde = vec![false; w * h];
    let mut pila: Vec<usize> = (0..w * h).filter(|&i| nms[i] >= alto).collect();
    for &i in &pila {
        borde[i] = true;
    }
    while let Some(i) = pila.pop() {
        let (x, y) = (i % w, i / w);
        for yy in y.saturating_sub(1)..(y + 2).min(h) {
            for xx in x.saturating_sub(1)..(x + 2).min(w) {
                let j = yy * w + xx;
                if !borde[j] && nms[j] >= bajo {
                    borde[j] = true;
                    pila.push(j);
                }
            }
        }
    }
    // Fondo (para el color de las lineas): la media del borde de la imagen.
    let fondo = {
        let mut s = [0f32; 3];
        let mut n = 0.0;
        for x in 0..w {
            for c in [img[x], img[(h - 1) * w + x]] {
                (0..3).for_each(|k| s[k] += c[k]);
                n += 1.0;
            }
        }
        [s[0] / n, s[1] / n, s[2] / n]
    };
    let tol = p.detalle.tolerancia();
    let mut salida = Vec::new();
    for cadena in cadenas(&mut borde, w, h) {
        if cadena.len() < p.detalle.cadena_minima() {
            continue;
        }
        let cerrada = cadena.len() > 4 && {
            let (a, b) = (cadena[0], cadena[cadena.len() - 1]);
            (a.0 as i64 - b.0 as i64).abs() <= 1 && (a.1 as i64 - b.1 as i64).abs() <= 1
        };
        let color = if p.colores {
            // El lado de la linea que no es fondo, de media.
            let mut s = [0f32; 3];
            let mut n = 0.0;
            for &(x, y) in cadena.iter().step_by(2) {
                let mut mejor = img[y * w + x];
                let mut md = d2(mejor, fondo);
                for (dx, dy) in [(-2i64, 0i64), (2, 0), (0, -2), (0, 2)] {
                    let (xx, yy) = (x as i64 + dx, y as i64 + dy);
                    if xx >= 0 && yy >= 0 && (xx as usize) < w && (yy as usize) < h {
                        let c = img[yy as usize * w + xx as usize];
                        if d2(c, fondo) > md {
                            md = d2(c, fondo);
                            mejor = c;
                        }
                    }
                }
                (0..3).for_each(|k| s[k] += mejor[k]);
                n += 1.0;
            }
            hex([s[0] / n, s[1] / n, s[2] / n])
        } else {
            "#1e1e1e".to_string()
        };
        // Al centro del pixel.
        let puntos: Vec<(f64, f64)> = cadena.iter().map(|&(x, y)| (x as f64 + 0.5, y as f64 + 0.5)).collect();
        let mut puntos = if cerrada {
            simplificar_cerrado(&puntos, tol)
        } else {
            simplificar(&puntos, tol)
        };
        puntos = suavizar(&puntos, cerrada);
        if cerrada && puntos.len() >= 3 {
            puntos.push(puntos[0]);
        }
        if puntos.len() >= 2 {
            salida.push(Trazo::Linea { puntos, cerrada, color });
        }
    }
    salida
}

fn gauss(g: &[f32], w: usize, h: usize) -> Vec<f32> {
    const K: [f32; 5] = [1.0 / 16.0, 4.0 / 16.0, 6.0 / 16.0, 4.0 / 16.0, 1.0 / 16.0];
    let mut tmp = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut s = 0.0;
            for (k, kv) in K.iter().enumerate() {
                let xx = (x as isize + k as isize - 2).clamp(0, w as isize - 1) as usize;
                s += kv * g[y * w + xx];
            }
            tmp[y * w + x] = s;
        }
    }
    let mut out = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut s = 0.0;
            for (k, kv) in K.iter().enumerate() {
                let yy = (y as isize + k as isize - 2).clamp(0, h as isize - 1) as usize;
                s += kv * tmp[yy * w + x];
            }
            out[y * w + x] = s;
        }
    }
    out
}

/// Las cadenas de pixeles de borde: primero desde los extremos (las
/// abiertas), luego los lazos que quedan. Se consumen los pixeles.
fn cadenas(borde: &mut [bool], w: usize, h: usize) -> Vec<Vec<(usize, usize)>> {
    let vecinos = |borde: &[bool], x: usize, y: usize| -> Vec<(usize, usize)> {
        let mut v = Vec::with_capacity(8);
        // Primero los 4 de lado: la cadena sale mas fiel.
        for (dx, dy) in [(1i64, 0i64), (0, 1), (-1, 0), (0, -1), (1, 1), (-1, 1), (-1, -1), (1, -1)] {
            let (xx, yy) = (x as i64 + dx, y as i64 + dy);
            if xx >= 0 && yy >= 0 && (xx as usize) < w && (yy as usize) < h && borde[yy as usize * w + xx as usize] {
                v.push((xx as usize, yy as usize));
            }
        }
        v
    };
    let mut salida = Vec::new();
    for pasada in 0..2 {
        for y in 0..h {
            for x in 0..w {
                if !borde[y * w + x] {
                    continue;
                }
                if pasada == 0 && vecinos(borde, x, y).len() != 1 {
                    continue;
                }
                let mut cadena = vec![(x, y)];
                borde[y * w + x] = false;
                let (mut cx, mut cy) = (x, y);
                while let Some(&(nx, ny)) = vecinos(borde, cx, cy).first() {
                    borde[ny * w + nx] = false;
                    cadena.push((nx, ny));
                    (cx, cy) = (nx, ny);
                }
                salida.push(cadena);
            }
        }
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Lienzo blanco de `w x h` con un circulo rojo (centro, radio) y un
    /// cuadrado azul (x, y, lado).
    pub(super) fn circulo_y_cuadrado(w: u32, h: u32, (cx, cy, r): (f64, f64, f64), (qx, qy, ql): (f64, f64, f64)) -> Vec<u8> {
        let mut px = vec![255u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                if (fx - cx).powi(2) + (fy - cy).powi(2) <= r * r {
                    px[i..i + 3].copy_from_slice(&[220, 30, 30]);
                }
                if fx >= qx && fx <= qx + ql && fy >= qy && fy <= qy + ql {
                    px[i..i + 3].copy_from_slice(&[30, 60, 210]);
                }
            }
        }
        px
    }

    /// Del lienzo a la imagen original.
    fn a_imagen(c: &Calcado, (x, y): (f64, f64)) -> (f64, f64) {
        ((x - c.desplazamiento.0) / c.escala, (y - c.desplazamiento.1) / c.escala)
    }

    fn error_circulo(c: &Calcado, pts: &[(f64, f64)], (cx, cy, r): (f64, f64, f64)) -> f64 {
        pts.iter()
            .map(|p| {
                let (x, y) = a_imagen(c, *p);
                (((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - r).abs()
            })
            .sum::<f64>()
            / pts.len() as f64
    }

    #[test]
    fn circulo_y_cuadrado_salen_como_manchas_cerradas_y_fieles() {
        let (w, h) = (400, 300);
        let circ = (110.0, 150.0, 70.0);
        let px = circulo_y_cuadrado(w, h, circ, (230.0, 80.0, 120.0));
        let t0 = std::time::Instant::now();
        let c = calcar(&px, w, h, &Parametros::default());
        assert!(t0.elapsed().as_millis() < 2000);
        let manchas: Vec<_> = c
            .trazos
            .iter()
            .filter_map(|t| match t {
                Trazo::Mancha { contorno, color, .. } => Some((contorno, color)),
                _ => None,
            })
            .collect();
        assert_eq!(manchas.len(), 2, "{:?}", c.trazos.len());
        let rojo = manchas.iter().find(|(_, col)| col.starts_with("#d") || col.starts_with("#e")).expect("mancha roja");
        let azul = manchas.iter().find(|(_, col)| col.ends_with("d2") || col.ends_with("d1") || col.ends_with("d3")).expect("mancha azul");
        // El circulo: radio medio con error de menos de 1,5 px (de 70).
        let e = error_circulo(&c, rojo.0, circ);
        assert!(e < 1.5, "error del circulo {e}");
        assert!(rojo.0.len() >= 12, "un circulo de pocos puntos: {}", rojo.0.len());
        // El cuadrado: 4 esquinas bien puestas (se conservan, no se redondean).
        let pts: Vec<_> = azul.0.iter().map(|p| a_imagen(&c, *p)).collect();
        for esquina in [(230.0, 80.0), (350.0, 80.0), (350.0, 200.0), (230.0, 200.0)] {
            let d = pts.iter().map(|p| dist(*p, esquina)).fold(f64::MAX, f64::min);
            // La esquina sale achaflanada en un pixel de la imagen reducida.
            assert!(d < 5.0, "esquina {esquina:?} a {d}");
        }
        assert!(azul.0.len() <= 8, "un cuadrado con {} puntos", azul.0.len());
        // Y cada vertice, pegado al borde del cuadrado.
        for p in &pts {
            let al_borde = [(p.0 - 230.0).abs(), (p.0 - 350.0).abs(), (p.1 - 80.0).abs(), (p.1 - 200.0).abs()]
                .into_iter()
                .fold(f64::MAX, f64::min);
            assert!(al_borde < 2.0, "vertice {p:?} a {al_borde} del borde");
        }
    }

    #[test]
    fn solo_contornos_da_lineas_cerradas_fieles() {
        let (w, h) = (400, 300);
        let circ = (110.0, 150.0, 70.0);
        let px = circulo_y_cuadrado(w, h, circ, (230.0, 80.0, 120.0));
        let c = calcar(
            &px,
            w,
            h,
            &Parametros {
                solo_contornos: true,
                ..Default::default()
            },
        );
        let lineas: Vec<_> = c
            .trazos
            .iter()
            .filter_map(|t| match t {
                Trazo::Linea { puntos, cerrada, color } => Some((puntos, *cerrada, color)),
                _ => None,
            })
            .collect();
        assert!(lineas.len() >= 2 && lineas.len() <= 4, "{} lineas", lineas.len());
        let cerradas: Vec<_> = lineas.iter().filter(|l| l.1).collect();
        assert!(cerradas.len() >= 2, "contornos sin cerrar");
        // La del circulo: la de centro cerca de (110,150).
        let del_circulo = cerradas
            .iter()
            .find(|l| {
                let (sx, sy) = l.0.iter().fold((0.0, 0.0), |s, p| (s.0 + p.0, s.1 + p.1));
                let m = a_imagen(&c, (sx / l.0.len() as f64, sy / l.0.len() as f64));
                dist(m, (110.0, 150.0)) < 10.0
            })
            .expect("contorno del circulo");
        let e = error_circulo(&c, del_circulo.0, circ);
        assert!(e < 2.0, "error del contorno del circulo {e}");
        // Con colores: la linea del circulo es roja.
        assert!(del_circulo.2.starts_with("#d") || del_circulo.2.starts_with("#e"), "{}", del_circulo.2);
    }

    #[test]
    fn un_agujero_sale_como_hueco() {
        // Un anillo: mancha con un agujero.
        let (w, h) = (100usize, 100usize);
        let mascara: Vec<bool> = (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as f64 + 0.5, (i / w) as f64 + 0.5);
                let d = ((x - 50.0).powi(2) + (y - 50.0).powi(2)).sqrt();
                (20.0..=40.0).contains(&d)
            })
            .collect();
        let r = regiones(&mascara, w, h, 4.0);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].1.len(), 1, "falta el agujero");
    }

    #[test]
    fn imagen_vacia_o_rara_no_cuelga_ni_dibuja_de_mas() {
        // Blanca entera.
        let blanco = vec![255u8; 300 * 200 * 4];
        assert!(calcar(&blanco, 300, 200, &Parametros::default()).trazos.is_empty());
        assert!(
            calcar(&blanco, 300, 200, &Parametros { solo_contornos: true, ..Default::default() })
                .trazos
                .is_empty()
        );
        // Sin pixeles, de tamano cero o con menos bytes de los que dice.
        assert!(calcar(&[], 0, 0, &Parametros::default()).trazos.is_empty());
        assert!(calcar(&[1, 2, 3], 10, 10, &Parametros::default()).trazos.is_empty());
        // Un pixel.
        let _ = calcar(&[0, 0, 0, 255], 1, 1, &Parametros::default());
        let _ = calcar(&[0, 0, 0, 255], 1, 1, &Parametros { solo_contornos: true, ..Default::default() });
        // Ruido puro: termina rapido y respeta el tope.
        let mut s = 12345u32;
        let ruido: Vec<u8> = (0..768 * 576 * 4)
            .map(|_| {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
                (s >> 16) as u8
            })
            .collect();
        for p in [
            Parametros::default(),
            Parametros { solo_contornos: true, ..Default::default() },
            Parametros { detalle: Detalle::Alto, ..Default::default() },
        ] {
            let t0 = std::time::Instant::now();
            let c = calcar(&ruido, 768, 576, &p);
            assert!(c.trazos.len() <= p.detalle.tope(), "{}", c.trazos.len());
            // En depuracion; en la app (optimizada) es mucho menos.
            assert!(t0.elapsed().as_secs_f32() < 10.0, "{:?}", t0.elapsed());
        }
    }

    #[test]
    fn una_zona_calca_solo_ese_trozo() {
        let (w, h) = (400, 300);
        let px = circulo_y_cuadrado(w, h, (110.0, 150.0, 70.0), (230.0, 80.0, 120.0));
        // La mitad derecha: solo el cuadrado.
        let c = calcar(&px, w, h, &Parametros { zona: Some([0.5, 0.0, 0.5, 1.0]), ..Default::default() });
        assert_eq!(c.trazos.len(), 1, "{:?}", c.trazos);
        let Trazo::Mancha { contorno, .. } = &c.trazos[0] else { panic!() };
        let esquina = a_imagen(&c, contorno[0]);
        assert!(esquina.0 > 200.0, "{esquina:?}");
    }

    /// Un emoji de perrito de mentira, con degradados como los de verdad:
    /// cara con degradado radial, orejas oscuras, lengua rosa, fondo
    /// transparente.
    fn emoji_con_degradados(w: u32) -> Vec<u8> {
        let mut px = vec![0u8; (w * w * 4) as usize];
        let c = w as f64 / 2.0;
        for y in 0..w {
            for x in 0..w {
                let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
                let i = ((y * w + x) * 4) as usize;
                let d = ((fx - c).powi(2) + (fy - c).powi(2)).sqrt() / (w as f64 * 0.38);
                let mut col: Option<[f64; 3]> = None;
                if d <= 1.0 {
                    // Del melocoton claro del centro al marron del borde.
                    col = Some([250.0 - 50.0 * d, 200.0 - 70.0 * d, 160.0 - 70.0 * d]);
                }
                for ox in [0.2, 0.8] {
                    let e = ((fx - ox * w as f64) / (w as f64 * 0.1)).powi(2) + ((fy - 0.3 * w as f64) / (w as f64 * 0.18)).powi(2);
                    if e <= 1.0 {
                        col = Some([110.0 - 20.0 * e, 70.0 - 15.0 * e, 60.0 - 10.0 * e]);
                    }
                }
                let lengua = ((fx - c) / (w as f64 * 0.07)).powi(2) + ((fy - 0.75 * w as f64) / (w as f64 * 0.09)).powi(2);
                if lengua <= 1.0 {
                    col = Some([215.0, 50.0 + 20.0 * lengua, 120.0]);
                }
                if let Some(k) = col {
                    px[i..i + 4].copy_from_slice(&[k[0] as u8, k[1] as u8, k[2] as u8, 255]);
                }
            }
        }
        px
    }

    #[test]
    fn un_emoji_con_degradados_se_parece() {
        let w = 384;
        let px = emoji_con_degradados(w);
        for detalle in [Detalle::Medio, Detalle::Alto] {
            let c = calcar(&px, w, w, &Parametros { detalle, ..Default::default() });
            // El fondo transparente (blanco) no se dibuja.
            assert!(!c.trazos.iter().any(|t| matches!(t, Trazo::Fondo { .. })));
            let manchas: Vec<(&Vec<(f64, f64)>, &String)> = c
                .trazos
                .iter()
                .filter_map(|t| match t {
                    Trazo::Mancha { contorno, color, .. } => Some((contorno, color)),
                    _ => None,
                })
                .collect();
            // Unas pocas bandas de color, no cientos de manchitas.
            assert!(manchas.len() >= 4 && manchas.len() <= detalle.tope(), "{detalle:?}: {}", manchas.len());
            // Lo calcado ocupa lo mismo que el emoji: de la oreja izquierda a
            // la derecha y de arriba de la cara a abajo.
            let pts: Vec<(f64, f64)> = manchas
                .iter()
                .flat_map(|m| m.0.iter())
                .map(|p| ((p.0 - c.desplazamiento.0) / c.escala, (p.1 - c.desplazamiento.1) / c.escala))
                .collect();
            let caja = pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p.0), b[1].min(p.1), b[2].max(p.0), b[3].max(p.1)]);
            let esperada = [0.1, 0.12, 0.9, 0.88].map(|f| f * w as f64);
            for (a, b) in caja.iter().zip(esperada) {
                assert!((a - b).abs() < 0.04 * w as f64, "{detalle:?}: caja {caja:?} y no {esperada:?}");
            }
            let rgb = |h: &str| {
                let v = |i| u8::from_str_radix(&h[i..i + 2], 16).unwrap() as i32;
                (v(1), v(3), v(5))
            };
            assert!(manchas.iter().any(|m| { let (r, g, b) = rgb(m.1); r > 180 && g < 110 && b > 90 }), "sin la lengua rosa");
            assert!(manchas.iter().any(|m| { let (r, g, b) = rgb(m.1); r < 130 && g < 90 && b < 80 }), "sin las orejas oscuras");
            // Ningun agujero tapado por otra mancha (dejaria una rendija).
            for t in &c.trazos {
                if let Trazo::Mancha { huecos, .. } = t {
                    assert!(huecos.is_empty(), "{detalle:?}: un agujero sobre otra mancha");
                }
            }
        }
    }

    #[test]
    fn lo_que_no_se_pidio_se_corrige_con_el_pedido() {
        let mal = Parametros { solo_contornos: true, colores: false, ..Default::default() };
        let mut p = mal.clone();
        ajustar_al_pedido(&mut p, "Dibuja este perrito de la imagen adjunta dentro del lienzo.");
        assert!(!p.solo_contornos && p.colores);
        let mut p = mal.clone();
        ajustar_al_pedido(&mut p, "Calca solo los contornos, en blanco y negro");
        assert_eq!(p, mal, "lo pedido se respeta");
    }

    #[test]
    fn rdp_y_suavizado() {
        let linea: Vec<(f64, f64)> = (0..50).map(|i| (i as f64, (i as f64 * 0.01).sin() * 0.1)).collect();
        assert_eq!(simplificar(&linea, 0.5).len(), 2);
        let cuadrado = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        assert_eq!(suavizar(&cuadrado, true), cuadrado, "las esquinas se respetan");
        assert!((area(&cuadrado) - 100.0).abs() < 1e-9);
    }
}
