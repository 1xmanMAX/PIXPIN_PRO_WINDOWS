//! **Una pagina en gris y lo poco de OpenCV que Indra necesita.**
//!
//! El original (`indra_ocr`, y el puerto de Kotlin del escaner) usa `cv2`
//! para reducir, ampliar, girar, desenfocar y cerrar. Aqui no hay OpenCV:
//! cada operacion esta escrita con la misma regla que la de `cv2` (centros
//! de pixel en `+0.5`, bordes que replican o reflejan como alli, nucleos
//! gaussianos del mismo tamano), en coma flotante. Las sumas en punto fijo
//! de OpenCV dan, como mucho, un nivel de gris de diferencia: no cambia lo
//! que lee el reconocedor.

use std::sync::atomic::{AtomicUsize, Ordering};

/// Hilos para lo que se reparte bien (las inclinaciones de prueba, la
/// morfologia). 0 = los del equipo, hasta cuatro. Lo fija quien llama
/// (`pixpin-ocr`: dos en el modo cuidadoso de 4 GB).
static HILOS: AtomicUsize = AtomicUsize::new(0);

pub fn fijar_hilos(n: usize) {
    HILOS.store(n, Ordering::Relaxed);
}

pub fn hilos() -> usize {
    match HILOS.load(Ordering::Relaxed) {
        0 => std::thread::available_parallelism()
            .map_or(2, |n| n.get())
            .min(4),
        n => n,
    }
}

/// `f(0..n)` repartido entre [`hilos`], en orden.
pub fn en_paralelo<T: Send>(n: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let h = hilos().min(n).max(1);
    if h == 1 {
        return (0..n).map(f).collect();
    }
    let f = &f;
    std::thread::scope(|s| {
        let trozos: Vec<_> = (0..h)
            .map(|k| s.spawn(move || (k..n).step_by(h).map(|i| (i, f(i))).collect::<Vec<_>>()))
            .collect();
        let mut v: Vec<(usize, T)> = trozos
            .into_iter()
            .flat_map(|t| t.join().unwrap_or_default())
            .collect();
        v.sort_by_key(|x| x.0);
        v.into_iter().map(|x| x.1).collect()
    })
}

/// Pagina en gris de 8 bits, fila a fila (0 negro, 255 blanco).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gris {
    pub ancho: usize,
    pub alto: usize,
    pub px: Vec<u8>,
}

impl Gris {
    /// Una pagina de un solo color.
    pub fn lisa(ancho: usize, alto: usize, valor: u8) -> Gris {
        Gris {
            ancho,
            alto,
            px: vec![valor; ancho * alto],
        }
    }

    /// Envuelve un buffer; `None` si no cuadra con las medidas.
    pub fn de_buffer(ancho: usize, alto: usize, px: Vec<u8>) -> Option<Gris> {
        (ancho > 0 && alto > 0 && px.len() == ancho * alto).then_some(Gris { ancho, alto, px })
    }

    /// Desde RGBA (lo que dan las capturas): luminancia BT.601 en punto
    /// fijo, la misma cuenta que `cv2.COLOR_RGB2GRAY` y que `IndraEngine.luma`.
    pub fn de_rgba(ancho: usize, alto: usize, rgba: &[u8]) -> Option<Gris> {
        let n = ancho.checked_mul(alto)?;
        if ancho == 0 || alto == 0 || rgba.len() < n.checked_mul(4)? {
            return None;
        }
        let px = rgba[..n * 4]
            .chunks_exact(4)
            .map(|p| luma(p[0], p[1], p[2]))
            .collect();
        Some(Gris { ancho, alto, px })
    }

    #[inline]
    pub fn en(&self, x: usize, y: usize) -> u8 {
        self.px[y * self.ancho + x]
    }
}

/// `(r·4899 + g·9617 + b·1868 + 8192) >> 14`: BT.601 con los pesos de OpenCV.
#[inline]
pub fn luma(r: u8, g: u8, b: u8) -> u8 {
    ((r as u32 * 4899 + g as u32 * 9617 + b as u32 * 1868 + 8192) >> 14) as u8
}

/// `round()` de Python (a la par): los tamanos tienen que salir como alli.
pub fn redondeo_py(x: f64) -> i64 {
    x.round_ties_even() as i64
}

/// Satura a 8 bits como `saturate_cast<uchar>`. Redondea la mitad hacia
/// arriba y no a la par: `round_ties_even` sin SSE4.1 (el x86-64 de base,
/// el del i3 de referencia) es una llamada a libm por pixel, y la
/// diferencia es un nivel de gris en los empates.
#[inline]
pub fn a_byte(v: f32) -> u8 {
    (v.clamp(0.0, 255.0) + 0.5) as u8
}

/// Como reducir o ampliar: los dos de `cv2.resize` que usa Indra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolacion {
    /// `INTER_AREA`: media de lo que cae dentro (para reducir).
    Area,
    /// `INTER_LINEAR`: bilineal con centros en `+0.5` (para ampliar).
    Lineal,
}

/// Pesos de un eje: para cada destino, (origen, peso) que suman 1.
fn pesos_eje(origen: usize, destino: usize, modo: Interpolacion) -> Vec<Vec<(usize, f32)>> {
    let escala = origen as f64 / destino as f64;
    let mut v = Vec::with_capacity(destino);
    if modo == Interpolacion::Area && escala > 1.0 {
        // Cada destino cubre [d·escala, (d+1)·escala) del origen.
        for d in 0..destino {
            let a = d as f64 * escala;
            let b = ((d + 1) as f64 * escala).min(origen as f64);
            let mut p = Vec::new();
            let mut s = a.floor() as usize;
            while (s as f64) < b && s < origen {
                let ini = a.max(s as f64);
                let fin = b.min((s + 1) as f64);
                if fin > ini {
                    p.push((s, ((fin - ini) / (b - a)) as f32));
                }
                s += 1;
            }
            v.push(p);
        }
    } else {
        for d in 0..destino {
            let x = ((d as f64 + 0.5) * escala - 0.5).max(0.0);
            let x0 = (x.floor() as usize).min(origen - 1);
            let f = (x - x0 as f64) as f32;
            let x1 = (x0 + 1).min(origen - 1);
            if x1 == x0 || f <= 0.0 {
                v.push(vec![(x0, 1.0)]);
            } else {
                v.push(vec![(x0, 1.0 - f), (x1, f)]);
            }
        }
    }
    v
}

/// `cv2.resize` a `ancho` x `alto` (separable, en coma flotante).
pub fn redimensionar(g: &Gris, ancho: usize, alto: usize, modo: Interpolacion) -> Gris {
    let ancho = ancho.max(1);
    let alto = alto.max(1);
    if ancho == g.ancho && alto == g.alto {
        return g.clone();
    }
    let px_x = pesos_eje(g.ancho, ancho, modo);
    let px_y = pesos_eje(g.alto, alto, modo);
    // Primero en horizontal (fila a fila), luego en vertical.
    let mut medio = vec![0f32; ancho * g.alto];
    for y in 0..g.alto {
        let fila = &g.px[y * g.ancho..(y + 1) * g.ancho];
        let sal = &mut medio[y * ancho..(y + 1) * ancho];
        for (x, p) in px_x.iter().enumerate() {
            sal[x] = p.iter().map(|&(s, w)| fila[s] as f32 * w).sum();
        }
    }
    let mut px = vec![0u8; ancho * alto];
    for (y, p) in px_y.iter().enumerate() {
        let sal = &mut px[y * ancho..(y + 1) * ancho];
        for (x, v) in sal.iter_mut().enumerate() {
            *v = a_byte(p.iter().map(|&(s, w)| medio[s * ancho + x] * w).sum());
        }
    }
    Gris { ancho, alto, px }
}

/// `np.rot90(g, k)`: `k > 0` antihorario, `k < 0` horario.
pub fn rot90(g: &Gris, k: i32) -> Gris {
    let k = k.rem_euclid(4);
    let (w, h) = (g.ancho, g.alto);
    match k {
        0 => g.clone(),
        1 => {
            let mut o = vec![0u8; w * h];
            for yy in 0..w {
                for xx in 0..h {
                    o[yy * h + xx] = g.px[xx * w + (w - 1 - yy)];
                }
            }
            Gris {
                ancho: h,
                alto: w,
                px: o,
            }
        }
        2 => Gris {
            ancho: w,
            alto: h,
            px: g.px.iter().rev().copied().collect(),
        },
        _ => {
            let mut o = vec![0u8; w * h];
            for yy in 0..w {
                for xx in 0..h {
                    o[yy * h + xx] = g.px[(h - 1 - xx) * w + yy];
                }
            }
            Gris {
                ancho: h,
                alto: w,
                px: o,
            }
        }
    }
}

/// `cv2.getRotationMatrix2D(centro, grados, 1)`: `[[a, b, tx], [-b, a, ty]]`.
pub fn matriz_de_giro(cx: f64, cy: f64, grados: f64) -> [f64; 6] {
    let r = grados.to_radians();
    let (a, b) = (r.cos(), r.sin());
    [
        a,
        b,
        (1.0 - a) * cx - b * cy,
        -b,
        a,
        b * cx + (1.0 - a) * cy,
    ]
}

/// Que hay fuera de la pagina al girarla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Borde {
    /// `BORDER_CONSTANT` con este valor.
    Fijo(u8),
    /// `BORDER_REPLICATE`: el pixel de borde mas cercano.
    Replicar,
}

/// `cv2.warpAffine(g, m, mismo tamano)` con interpolacion bilineal.
pub fn girar_afin(g: &Gris, m: &[f64; 6], borde: Borde) -> Gris {
    // OpenCV invierte la matriz y recorre el destino.
    let det = m[0] * m[4] - m[1] * m[3];
    let det = if det.abs() < 1e-12 { 1.0 } else { det };
    let (a, b, c, d) = (m[4] / det, -m[1] / det, -m[3] / det, m[0] / det);
    let tx = -(a * m[2] + b * m[5]);
    let ty = -(c * m[2] + d * m[5]);
    let (w, h) = (g.ancho as i64, g.alto as i64);
    let lee = |x: i64, y: i64| -> f32 {
        if x >= 0 && y >= 0 && x < w && y < h {
            g.px[(y * w + x) as usize] as f32
        } else {
            match borde {
                Borde::Fijo(v) => v as f32,
                Borde::Replicar => {
                    g.px[(y.clamp(0, h - 1) * w + x.clamp(0, w - 1)) as usize] as f32
                }
            }
        }
    };
    let src = &g.px;
    let wu = g.ancho;
    let mut px = vec![0u8; g.px.len()];
    for y in 0..h {
        // A lo largo de la fila el origen avanza (a, c) por pixel.
        let mut sx = b * y as f64 + tx;
        let mut sy = d * y as f64 + ty;
        let fila = &mut px[(y * w) as usize..((y + 1) * w) as usize];
        for v in fila.iter_mut() {
            // `floor` sin SSE4.1 tambien es libm: se trunca y se corrige.
            let x0f = piso(sx);
            let y0f = piso(sy);
            let fx = (sx - x0f) as f32;
            let fy = (sy - y0f) as f32;
            let (x0, y0) = (x0f as i64, y0f as i64);
            let r = if x0 >= 0 && y0 >= 0 && x0 + 1 < w && y0 + 1 < h {
                // Camino rapido: los cuatro vecinos dentro.
                let i = y0 as usize * wu + x0 as usize;
                let (p00, p01) = (src[i] as f32, src[i + 1] as f32);
                let (p10, p11) = (src[i + wu] as f32, src[i + wu + 1] as f32);
                (p00 + (p01 - p00) * fx) * (1.0 - fy) + (p10 + (p11 - p10) * fx) * fy
            } else if let Borde::Fijo(c) = borde
                && (x0 < -1 || y0 < -1 || x0 >= w || y0 >= h)
            {
                c as f32
            } else {
                (lee(x0, y0) * (1.0 - fx) + lee(x0 + 1, y0) * fx) * (1.0 - fy)
                    + (lee(x0, y0 + 1) * (1.0 - fx) + lee(x0 + 1, y0 + 1) * fx) * fy
            };
            *v = a_byte(r);
            sx += a;
            sy += c;
        }
    }
    Gris {
        ancho: g.ancho,
        alto: g.alto,
        px,
    }
}

#[inline]
fn piso(v: f64) -> f64 {
    let t = v as i64 as f64;
    if t > v { t - 1.0 } else { t }
}

/// Nucleo de `cv2.getGaussianKernel` para `sigma` (tamano de 8 bits:
/// `round(sigma·6 + 1) | 1`).
pub fn nucleo_gaussiano(sigma: f64) -> Vec<f32> {
    let n = (((sigma * 6.0 + 1.0).round_ties_even() as i64) | 1).max(1) as usize;
    let c = (n as f64 - 1.0) / 2.0;
    let v: Vec<f64> = (0..n)
        .map(|i| (-((i as f64 - c).powi(2)) / (2.0 * sigma * sigma)).exp())
        .collect();
    let s: f64 = v.iter().sum();
    v.iter().map(|x| (x / s) as f32).collect()
}

/// `BORDER_REFLECT_101` (`gfedcb|abcdefgh|gfedcba`), el de los filtros.
#[inline]
fn refleja(i: i64, n: i64) -> usize {
    if n == 1 {
        return 0;
    }
    let mut i = i;
    loop {
        if i < 0 {
            i = -i;
        } else if i >= n {
            i = 2 * n - 2 - i;
        } else {
            return i as usize;
        }
    }
}

/// `cv2.GaussianBlur(g, (0, 0), sigma)`, separable.
pub fn desenfocar(g: &Gris, sigma: f64) -> Vec<f32> {
    let k = nucleo_gaussiano(sigma);
    let r = k.len() / 2;
    let (w, h) = (g.ancho, g.alto);
    // Horizontal: cada fila con su borde reflejado delante y detras.
    let mut medio = vec![0f32; w * h];
    let mut fila = vec![0f32; w + 2 * r];
    for y in 0..h {
        for (i, v) in fila.iter_mut().enumerate() {
            *v = g.px[y * w + refleja(i as i64 - r as i64, w as i64)] as f32;
        }
        let sal = &mut medio[y * w..(y + 1) * w];
        for (x, o) in sal.iter_mut().enumerate() {
            *o = fila[x..x + k.len()]
                .iter()
                .zip(&k)
                .map(|(a, b)| a * b)
                .sum();
        }
    }
    // Vertical: se suman filas enteras, que es lo que mejor va a la cache.
    let mut sal = vec![0f32; w * h];
    for y in 0..h {
        let o = &mut sal[y * w..(y + 1) * w];
        for (j, kv) in k.iter().enumerate() {
            let yy = refleja(y as i64 + j as i64 - r as i64, h as i64);
            let src = &medio[yy * w..(yy + 1) * w];
            for (d, s) in o.iter_mut().zip(src) {
                *d += s * kv;
            }
        }
    }
    sal
}

/// Lo mismo, de vuelta a 8 bits.
pub fn desenfocar_gris(g: &Gris, sigma: f64) -> Gris {
    Gris {
        ancho: g.ancho,
        alto: g.alto,
        px: desenfocar(g, sigma).into_iter().map(a_byte).collect(),
    }
}

/// `cv2.medianBlur(g, 5)` (borde replicado).
pub fn mediana5(g: &Gris) -> Gris {
    let (w, h) = (g.ancho as i64, g.alto as i64);
    let mut px = vec![0u8; g.px.len()];
    let mut ventana = [0u8; 25];
    for y in 0..h {
        for x in 0..w {
            let mut n = 0;
            for dy in -2..=2 {
                for dx in -2..=2 {
                    let xx = (x + dx).clamp(0, w - 1);
                    let yy = (y + dy).clamp(0, h - 1);
                    ventana[n] = g.px[(yy * w + xx) as usize];
                    n += 1;
                }
            }
            ventana.sort_unstable();
            px[(y * w + x) as usize] = ventana[12];
        }
    }
    Gris {
        ancho: g.ancho,
        alto: g.alto,
        px,
    }
}

/// Elipse de `cv2.getStructuringElement(MORPH_ELLIPSE, (k, k))`: por fila,
/// el tramo de columnas `[desde, hasta)` que cuenta.
pub fn elipse(k: usize) -> Vec<(usize, usize)> {
    let r = (k / 2) as f64;
    let c = (k / 2) as i64;
    let inv = if r > 0.0 { 1.0 / (r * r) } else { 0.0 };
    (0..k)
        .map(|i| {
            let dy = i as f64 - r;
            if dy.abs() <= r {
                let dx = (c as f64 * ((r * r - dy * dy) * inv).sqrt()).round_ties_even() as i64;
                (
                    (c - dx).max(0) as usize,
                    ((c + dx + 1).min(k as i64)) as usize,
                )
            } else {
                (0, 0)
            }
        })
        .collect()
}

/// Dilata (`max`) o erosiona (`min`) con el elemento; fuera de la imagen no
/// cuenta (es el borde por defecto de la morfologia de OpenCV).
fn morfologia(g: &Gris, el: &[(usize, usize)], dilatar: bool) -> Gris {
    let k = el.len() as i64;
    let c = k / 2;
    let (w, h) = (g.ancho as i64, g.alto as i64);
    let filas = en_paralelo(g.alto, |y| {
        let y = y as i64;
        let mut fila = vec![0u8; g.ancho];
        for (x, o) in fila.iter_mut().enumerate() {
            let x = x as i64;
            let mut v: u8 = if dilatar { 0 } else { 255 };
            for (i, &(a, b)) in el.iter().enumerate() {
                let yy = y + i as i64 - c;
                if yy < 0 || yy >= h || a >= b {
                    continue;
                }
                let x0 = (x + a as i64 - c).max(0);
                let x1 = (x + b as i64 - c).min(w);
                if x0 >= x1 {
                    continue;
                }
                let tramo = &g.px[(yy * w + x0) as usize..(yy * w + x1) as usize];
                v = if dilatar {
                    tramo.iter().fold(v, |m, &p| m.max(p))
                } else {
                    tramo.iter().fold(v, |m, &p| m.min(p))
                };
            }
            *o = v;
        }
        fila
    });
    Gris {
        ancho: g.ancho,
        alto: g.alto,
        px: filas.concat(),
    }
}

/// `cv2.morphologyEx(g, MORPH_CLOSE, elipse k)`: dilatar y luego erosionar.
pub fn cerrar(g: &Gris, k: usize) -> Gris {
    let el = elipse(k);
    morfologia(&morfologia(g, &el, true), &el, false)
}

/// Una mancha de `componentes` con lo que da `connectedComponentsWithStats`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mancha {
    pub izquierda: usize,
    pub arriba: usize,
    pub ancho: usize,
    pub alto: usize,
    pub area: usize,
}

/// Componentes conexas (vecindad 4) de `mascara` (`!= 0`): la etiqueta de
/// cada pixel (0 fondo, 1.. manchas, por orden de aparicion fila a fila,
/// como OpenCV) y las manchas (`manchas[e - 1]` es la de la etiqueta `e`).
pub fn componentes(mascara: &[u8], ancho: usize, alto: usize) -> (Vec<u32>, Vec<Mancha>) {
    let mut etiqueta = vec![0u32; mascara.len()];
    let mut manchas = Vec::new();
    let mut pila = Vec::new();
    for inicio in 0..mascara.len() {
        if mascara[inicio] == 0 || etiqueta[inicio] != 0 {
            continue;
        }
        let e = manchas.len() as u32 + 1;
        let (mut x0, mut y0, mut x1, mut y1, mut area) = (usize::MAX, usize::MAX, 0, 0, 0);
        etiqueta[inicio] = e;
        pila.push(inicio);
        while let Some(i) = pila.pop() {
            let (x, y) = (i % ancho, i / ancho);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            area += 1;
            let mut mira = |j: usize| {
                if mascara[j] != 0 && etiqueta[j] == 0 {
                    etiqueta[j] = e;
                    pila.push(j);
                }
            };
            if x > 0 {
                mira(i - 1);
            }
            if x + 1 < ancho {
                mira(i + 1);
            }
            if y > 0 {
                mira(i - ancho);
            }
            if y + 1 < alto {
                mira(i + ancho);
            }
        }
        manchas.push(Mancha {
            izquierda: x0,
            arriba: y0,
            ancho: x1 - x0 + 1,
            alto: y1 - y0 + 1,
            area,
        });
    }
    (etiqueta, manchas)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_luma_es_la_de_opencv() {
        assert_eq!(luma(255, 255, 255), 255);
        assert_eq!(luma(0, 0, 0), 0);
        // 0.299·255 = 76.2 → 76
        assert_eq!(luma(255, 0, 0), 76);
        let g = Gris::de_rgba(2, 1, &[255, 255, 255, 255, 0, 0, 0, 255]).unwrap();
        assert_eq!(g.px, [255, 0]);
        assert!(Gris::de_rgba(2, 2, &[0; 8]).is_none());
    }

    #[test]
    fn reducir_a_la_mitad_promedia_y_ampliar_interpola() {
        let g = Gris::de_buffer(4, 2, vec![0, 100, 200, 200, 0, 100, 200, 200]).unwrap();
        let r = redimensionar(&g, 2, 1, Interpolacion::Area);
        assert_eq!(r.px, [50, 200]);
        // Escala no entera: 3 → 2, cada destino cubre 1,5 pixeles.
        let g = Gris::de_buffer(3, 1, vec![0, 90, 180]).unwrap();
        let r = redimensionar(&g, 2, 1, Interpolacion::Area);
        assert_eq!(r.px, [30, 150]);
        let g = Gris::de_buffer(2, 1, vec![0, 100]).unwrap();
        let r = redimensionar(&g, 4, 1, Interpolacion::Lineal);
        // Centros: -0.25 (pega al 0), 0.25, 0.75, 1.25 (pega al 1).
        assert_eq!(r.px, [0, 25, 75, 100]);
    }

    #[test]
    fn rot90_vuelve_al_origen_y_gira_hacia_donde_toca() {
        let g = Gris::de_buffer(3, 2, vec![1, 2, 3, 4, 5, 6]).unwrap();
        let a = rot90(&g, 1);
        // np.rot90([[1,2,3],[4,5,6]]) = [[3,6],[2,5],[1,4]]
        assert_eq!((a.ancho, a.alto), (2, 3));
        assert_eq!(a.px, [3, 6, 2, 5, 1, 4]);
        let h = rot90(&g, -1);
        assert_eq!(h.px, [4, 1, 5, 2, 6, 3]);
        assert_eq!(rot90(&rot90(&g, 1), -1), g);
        assert_eq!(rot90(&rot90(&g, 2), 2), g);
    }

    #[test]
    fn girar_cero_grados_no_cambia_nada_y_noventa_mueve_el_punto() {
        let mut g = Gris::lisa(9, 9, 0);
        g.px[2 * 9 + 6] = 255; // (6, 2)
        let m = matriz_de_giro(4.0, 4.0, 0.0);
        assert_eq!(girar_afin(&g, &m, Borde::Fijo(0)), g);
        // 90 grados antihorario sobre el centro: (6, 2) → (2, 2).
        let r = girar_afin(&g, &matriz_de_giro(4.0, 4.0, 90.0), Borde::Fijo(0));
        assert_eq!(r.en(2, 2), 255);
    }

    #[test]
    fn el_nucleo_gaussiano_suma_uno_y_tiene_el_tamano_de_opencv() {
        let k = nucleo_gaussiano(1.0);
        assert_eq!(k.len(), 7);
        assert!((k.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        assert_eq!(nucleo_gaussiano(17.0 / 3.0).len(), 35);
        // Una imagen lisa sigue lisa.
        let g = Gris::lisa(10, 10, 77);
        assert!(desenfocar_gris(&g, 2.0).px.iter().all(|&v| v == 77));
    }

    #[test]
    fn la_elipse_es_la_de_opencv() {
        // getStructuringElement(MORPH_ELLIPSE, (5, 5))
        let e = elipse(5);
        assert_eq!(e, vec![(2, 3), (0, 5), (0, 5), (0, 5), (2, 3)]);
    }

    #[test]
    fn cerrar_borra_la_tinta_fina_y_la_mediana_el_punto_suelto() {
        let mut g = Gris::lisa(20, 20, 220);
        for y in 5..15 {
            g.px[y * 20 + 10] = 10; // un trazo vertical de 1 px
        }
        let c = cerrar(&g, 5);
        assert!(c.px.iter().all(|&v| v == 220));
        let mut g = Gris::lisa(9, 9, 200);
        g.px[4 * 9 + 4] = 0;
        assert!(mediana5(&g).px.iter().all(|&v| v == 200));
    }

    #[test]
    fn las_componentes_van_por_orden_de_lectura_con_sus_cajas() {
        #[rustfmt::skip]
        let m = [
            1, 1, 0, 0, 1,
            0, 1, 0, 0, 1,
            0, 0, 0, 1, 0,
        ];
        let (et, manchas) = componentes(&m, 5, 3);
        assert_eq!(manchas.len(), 3); // la diagonal no une (vecindad 4)
        assert_eq!(et[0], 1);
        assert_eq!(et[4], 2);
        assert_eq!(et[13], 3);
        assert_eq!(
            manchas[0],
            Mancha {
                izquierda: 0,
                arriba: 0,
                ancho: 2,
                alto: 2,
                area: 3
            }
        );
        assert_eq!(manchas[1].alto, 2);
    }
}
