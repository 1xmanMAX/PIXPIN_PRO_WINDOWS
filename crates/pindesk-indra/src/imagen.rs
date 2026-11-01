//! Pre y posproceso de imagen de Indra: puertos de `pipeline.py`,
//! `det_post.py` y `rec_preproc.py` (y de `IndraImage.kt` del escaner).
//! Mismos tamanos (redondeo de Python), mismas reglas de reducir con media y
//! ampliar con bilineal, mismos giros.

use crate::gris::{
    Borde, Gris, Interpolacion, en_paralelo, girar_afin, matriz_de_giro, redimensionar, redondeo_py,
};

pub const LADO_PREVIA: usize = 1600;
pub const LADO_ORIENTACION: usize = 768;
pub const INCLINACION_MAX: f64 = 45.0;
/// Inclinaciones menores no se enderezan (el paso de la busqueda es 0,1).
pub const INCLINACION_MIN: f64 = 0.3;
pub const MULTIPLO_ENTRADA: usize = 32;
pub const MARGEN_Y: f64 = 0.2;
pub const MARGEN_X: f64 = 0.3;
pub const MULTIPLO_ANCHO: usize = 8;

/// Caja de linea `[x0, y0, x1, y1]` en pixeles.
pub type Caja = [f32; 4];

fn reducir_a(g: &Gris, lado: usize, modo: Interpolacion) -> Gris {
    let s = lado as f64 / g.ancho.max(g.alto) as f64;
    redimensionar(
        g,
        redondeo_py(g.ancho as f64 * s).max(1) as usize,
        redondeo_py(g.alto as f64 * s).max(1) as usize,
        modo,
    )
}

/// La pagina reducida una vez a como mucho `lado` pixeles: la leen la
/// busqueda de la inclinacion y el modelo de orientacion.
pub fn previa(g: &Gris, lado: usize) -> Gris {
    if lado as f64 / g.ancho.max(g.alto) as f64 >= 1.0 {
        return g.clone();
    }
    reducir_a(g, lado, Interpolacion::Area)
}

/// Mapa de tinta (255 = tinta) reducido a `lado`.
fn miniatura_de_tinta(g: &Gris, lado: usize) -> Gris {
    let mut m = reducir_a(g, lado, Interpolacion::Area);
    for p in &mut m.px {
        *p = 255 - *p;
    }
    m
}

/// Lo bruscamente que cambia el perfil horizontal de tinta de fila a fila,
/// con la pagina girada `a` grados: maximo con los renglones derechos.
///
/// Se gira la miniatura entera, como el original. Proyectar cada pixel a
/// su fila girada seria diez veces mas barato, pero con papel gris (una
/// foto) o un fondo oscuro el muestreo irregular mete ruido que gana a los
/// renglones: en el banco elegia 45 grados en vez de 0 o de 1,8.
fn nitidez_de_filas(tinta: &Gris, a: f64) -> f64 {
    let m = matriz_de_giro(tinta.ancho as f64 / 2.0, tinta.alto as f64 / 2.0, a);
    let r = girar_afin(tinta, &m, Borde::Fijo(0));
    let mut previo: Option<i64> = None;
    let mut acc = 0.0;
    for fila in r.px.chunks_exact(r.ancho) {
        let s: i64 = fila.iter().map(|&v| v as i64).sum();
        if let Some(p) = previo {
            let d = (s - p) as f64;
            acc += d * d;
        }
        previo = Some(s);
    }
    acc
}

/// Elemento `i` de `np.arange(inicio, …, paso)`: numpy suma
/// `i · ((inicio + paso) − inicio)`.
fn np_arange(inicio: f64, paso: f64, i: usize) -> f64 {
    inicio + i as f64 * ((inicio + paso) - inicio)
}

/// Grados (antihorario) que enderezan los renglones (`estimate_skew`): de 2
/// en 2 sobre ±45 en la miniatura de la mitad, y luego afinando de 0,5 y de
/// 0,1 alrededor de los dos mejores y del 0.
pub fn estimar_inclinacion(g: &Gris, lado: usize) -> f64 {
    let fina = miniatura_de_tinta(g, lado);
    let media = fina.px.iter().map(|&v| v as f64).sum::<f64>() / fina.px.len().max(1) as f64;
    if media < 0.003 * 255.0 {
        return 0.0; // una pagina en blanco no tiene renglones
    }
    let gruesa = miniatura_de_tinta(g, lado / 2);
    let mut rejilla = Vec::new();
    let mut a = -INCLINACION_MAX;
    while a <= INCLINACION_MAX + 1e-6 {
        rejilla.push(a);
        a += 2.0;
    }
    let notas: Vec<f64> = en_paralelo(rejilla.len(), |i| nitidez_de_filas(&gruesa, rejilla[i]));
    // np.argsort(notas)[-2:] (estable: en empate, el indice mayor al final).
    let mut idx: Vec<usize> = (0..notas.len()).collect();
    idx.sort_by(|&x, &y| notas[x].total_cmp(&notas[y]).then(x.cmp(&y)));
    let mut salidas: Vec<f64> = vec![0.0];
    for &i in &idx[idx.len().saturating_sub(2)..] {
        if !salidas.contains(&rejilla[i]) {
            salidas.push(rejilla[i]);
        }
    }
    let mut cerca = Vec::new();
    for &a0 in &salidas {
        for k in 0..=8 {
            cerca.push(np_arange(a0 - 2.0, 0.5, k));
        }
    }
    let mut unicos: Vec<f64> = Vec::new();
    for &x in &cerca {
        if !unicos.contains(&x) {
            unicos.push(x);
        }
    }
    let notas_finas = en_paralelo(unicos.len(), |i| nitidez_de_filas(&fina, unicos[i]));
    let vistos: Vec<(f64, f64)> = unicos.into_iter().zip(notas_finas).collect();
    let nota = |x: f64| {
        vistos
            .iter()
            .find(|&&(v, _)| v == x)
            .map_or(0.0, |&(_, n)| n)
    };
    let mut a1 = cerca[0];
    let mut b1 = nota(a1);
    for &x in &cerca {
        let v = nota(x);
        if v > b1 {
            b1 = v;
            a1 = x;
        }
    }
    let mut mejor = a1 - 0.5;
    let mut bv = f64::NEG_INFINITY;
    let ultimos: Vec<f64> = (0..=10).map(|k| np_arange(a1 - 0.5, 0.1, k)).collect();
    let notas_ultimas = en_paralelo(ultimos.len(), |i| nitidez_de_filas(&fina, ultimos[i]));
    for (&x, &v) in ultimos.iter().zip(&notas_ultimas) {
        if v > bv {
            bv = v;
            mejor = x;
        }
    }
    mejor
}

/// La pagina girada `grados` (antihorario) sobre su centro, mismo tamano,
/// borde replicado (`rotate_page`).
pub fn girar_pagina(g: &Gris, grados: f64) -> Gris {
    let m = matriz_de_giro(g.ancho as f64 / 2.0, g.alto as f64 / 2.0, grados);
    girar_afin(g, &m, Borde::Replicar)
}

/// Una caja de la pagina que `girar_pagina(.., grados)` hizo, devuelta a la
/// pagina de antes: la envolvente de las cuatro esquinas, recortada
/// (`unrotate_box`).
pub fn desgirar_caja(b: &Caja, grados: f64, ancho: usize, alto: usize) -> Caja {
    let m = matriz_de_giro(ancho as f64 / 2.0, alto as f64 / 2.0, -grados);
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (px, py) in [(b[0], b[1]), (b[2], b[1]), (b[0], b[3]), (b[2], b[3])] {
        let (px, py) = (px as f64, py as f64);
        let x = m[0] * px + m[1] * py + m[2];
        let y = m[3] * px + m[4] * py + m[5];
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    let r1 = |v: f64, tope: usize| ((v.clamp(0.0, tope as f64) * 10.0).round() / 10.0) as f32;
    [r1(x0, ancho), r1(y0, alto), r1(x1, ancho), r1(y1, alto)]
}

/// Entrada de `orient.onnx`: `[lado, lado]` de tinta (1 = tinta), el lado
/// largo a `lado` y el resto relleno de papel (`orient_input`).
pub fn entrada_orientacion(g: &Gris, lado: usize) -> Vec<f32> {
    let p = reducir_a(g, lado, Interpolacion::Area);
    let mut v = vec![0f32; lado * lado];
    for y in 0..p.alto.min(lado) {
        for x in 0..p.ancho.min(lado) {
            v[y * lado + x] = 1.0 - p.en(x, y) as f32 / 255.0;
        }
    }
    v
}

/// Entrada del detector (`page_to_input`): el lado largo a `lado_largo` y
/// relleno hasta multiplos de 32.
#[derive(Debug, Clone)]
pub struct EntradaDetector {
    pub datos: Vec<f32>,
    pub ancho: usize,
    pub alto: usize,
    pub escala: f64,
}

pub fn entrada_detector(g: &Gris, lado_largo: usize) -> EntradaDetector {
    let escala = lado_largo as f64 / g.ancho.max(g.alto) as f64;
    let nh = redondeo_py(g.alto as f64 * escala).max(1) as usize;
    let nw = redondeo_py(g.ancho as f64 * escala).max(1) as usize;
    let r = redimensionar(
        g,
        nw,
        nh,
        if escala < 1.0 {
            Interpolacion::Area
        } else {
            Interpolacion::Lineal
        },
    );
    let ph = nh.div_ceil(MULTIPLO_ENTRADA) * MULTIPLO_ENTRADA;
    let pw = nw.div_ceil(MULTIPLO_ENTRADA) * MULTIPLO_ENTRADA;
    let mut datos = vec![0f32; ph * pw];
    for y in 0..nh {
        for x in 0..nw {
            datos[y * pw + x] = 1.0 - r.en(x, y) as f32 / 255.0;
        }
    }
    EntradaDetector {
        datos,
        ancho: pw,
        alto: ph,
        escala,
    }
}

/// `np.percentile(b, (2, 50, 98))` de un recorte, desde su histograma.
fn percentiles(g: &Gris, x0: usize, y0: usize, x1: usize, y1: usize) -> [f64; 3] {
    let mut hist = [0u64; 256];
    let mut n: u64 = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            hist[g.en(x, y) as usize] += 1;
            n += 1;
        }
    }
    let mut cum = [0u64; 256];
    let mut c = 0;
    for i in 0..256 {
        c += hist[i];
        cum[i] = c;
    }
    // np.searchsorted(cum, v, side="right")
    let derecha = |v: u64| -> f64 { cum.partition_point(|&x| x <= v) as f64 };
    let mut out = [0.0; 3];
    for (k, q) in [0.02, 0.5, 0.98].into_iter().enumerate() {
        let x = q * (n.saturating_sub(1)) as f64;
        let i = x.floor() as u64;
        let f = x - i as f64;
        let lo = derecha(i);
        let hi = derecha((i + 1).min(n.saturating_sub(1)));
        let d = hi - lo;
        out[k] = if f >= 0.5 {
            hi - d * (1.0 - f)
        } else {
            lo + d * f
        };
    }
    out
}

/// Texto claro sobre fondo oscuro (`is_dark_line`): el fondo (la mediana)
/// queda mas cerca del extremo oscuro que el texto.
pub fn linea_oscura(g: &Gris, b: &Caja) -> bool {
    let x0 = (b[0] as i64).max(0) as usize;
    let y0 = (b[1] as i64).max(0) as usize;
    let x1 = ((b[2] as i64) + 1).min(g.ancho as i64).max(0) as usize;
    let y1 = ((b[3] as i64) + 1).min(g.alto as i64).max(0) as usize;
    if x1 <= x0 || y1 <= y0 {
        return false;
    }
    let [lo, med, hi] = percentiles(g, x0, y0, x1, y1);
    hi - med > 2.0 * (med - lo) + 8.0
}

/// Un renglon listo para el reconocedor: mapa de tinta `alto` x `ancho`.
#[derive(Debug, Clone)]
pub struct Recorte {
    pub datos: Vec<f32>,
    pub ancho: usize,
    pub alto: usize,
}

/// Recorte de un renglon a `alto · ventana` pixeles (`crop_line`): la caja
/// con margen de 0,3 alturas a los lados y 0,2 arriba y abajo (o la pagina
/// alrededor con una ventana alta), escalada sin deformar.
pub fn recortar_linea(
    g: &Gris,
    b: &Caja,
    alto: usize,
    ventana: usize,
    oscura: Option<bool>,
    ancho_max: usize,
) -> Recorte {
    let margen_y = (ventana as f64 * (1.0 + 2.0 * MARGEN_Y) - 1.0) / 2.0;
    let hh = alto * ventana;
    let (x0, y0, x1, y1) = (b[0] as f64, b[1] as f64, b[2] as f64, b[3] as f64);
    let h = (y1 - y0).max(1.0);
    let ix0 = (x0 - MARGEN_X * h).floor() as i64;
    let iy0 = (y0 - margen_y * h).floor() as i64;
    let ix1 = (x1 + MARGEN_X * h).ceil() as i64;
    let iy1 = (y1 + margen_y * h).ceil() as i64;
    let cw = (ix1 - ix0).max(1) as usize;
    let ch = (iy1 - iy0).max(1) as usize;
    let sx0 = ix0.max(0);
    let sy0 = iy0.max(0);
    let sx1 = ix1.min(g.ancho as i64);
    let sy1 = iy1.min(g.alto as i64);
    let oscura = oscura.unwrap_or_else(|| linea_oscura(g, b));
    let mut c = Gris::lisa(cw, ch, if oscura { 0 } else { 255 });
    if sx1 > sx0 && sy1 > sy0 {
        let n = (sx1 - sx0) as usize;
        for y in sy0..sy1 {
            let o = (y as usize) * g.ancho + sx0 as usize;
            let d = (y - iy0) as usize * cw + (sx0 - ix0) as usize;
            c.px[d..d + n].copy_from_slice(&g.px[o..o + n]);
        }
    }
    let ancho = (redondeo_py(cw as f64 * hh as f64 / ch as f64) as usize)
        .clamp(MULTIPLO_ANCHO, ancho_max.max(MULTIPLO_ANCHO));
    let modo = if ch > hh {
        Interpolacion::Area
    } else {
        Interpolacion::Lineal
    };
    let r = redimensionar(&c, ancho, hh, modo);
    let datos =
        r.px.iter()
            .map(|&v| {
                let v = v as f32 / 255.0;
                if oscura { v } else { 1.0 - v }
            })
            .collect();
    Recorte {
        datos,
        ancho,
        alto: hh,
    }
}

/// Lotes de anchos parecidos (`width_batches`): del mas estrecho al mas
/// ancho; un lote se cierra a las `max_lote` lineas, cuando la siguiente
/// es 1,3 veces mas ancha que la primera, o cuando lineas x ancho pasarian
/// de 64 x 640 columnas (ONNX Runtime se queda con su mayor reserva).
pub fn lotes_por_ancho(anchos: &[usize], max_lote: usize) -> Vec<Vec<usize>> {
    const RAZON: f64 = 1.3;
    const ANCHO_MIN: usize = 64;
    const AREA_MAX: usize = 64 * 640;
    let mut orden: Vec<usize> = (0..anchos.len()).collect();
    orden.sort_by_key(|&i| (anchos[i], i));
    let mut lotes: Vec<Vec<usize>> = Vec::new();
    for i in orden {
        let w = anchos[i].max(ANCHO_MIN);
        if let Some(ultimo) = lotes.last_mut()
            && ultimo.len() < max_lote
            && w as f64 <= RAZON * anchos[ultimo[0]].max(ANCHO_MIN) as f64
            && (ultimo.len() + 1) * w <= AREA_MAX
        {
            ultimo.push(i);
            continue;
        }
        lotes.push(vec![i]);
    }
    lotes
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lotes_por_ancho_como_el_escaner() {
        // IndraUnitTest.lotesPorAncho
        let b = lotes_por_ancho(&[100, 30, 500, 110, 40], 64);
        assert_eq!(b, vec![vec![1, 4], vec![0, 3], vec![2]]);
    }

    #[test]
    fn la_entrada_del_detector_rellena_a_multiplos_de_32() {
        let g = Gris::lisa(100, 50, 255);
        let e = entrada_detector(&g, 1024);
        assert_eq!((e.ancho, e.alto), (1024, 512));
        assert!((e.escala - 10.24).abs() < 1e-9);
        // Papel blanco = sin tinta.
        assert!(e.datos.iter().all(|&v| v == 0.0));
        let g = Gris::lisa(1000, 333, 0);
        let e = entrada_detector(&g, 1024);
        // 333 · 1.024 = 340.99 → 341 → 352
        assert_eq!((e.ancho, e.alto), (1024, 352));
        assert_eq!(e.datos[0], 1.0);
        assert_eq!(e.datos[351 * 1024], 0.0);
    }

    #[test]
    fn el_recorte_tiene_la_altura_pedida_y_la_tinta_en_positivo() {
        let mut g = Gris::lisa(200, 60, 255);
        for y in 20..40 {
            for x in 20..180 {
                g.px[y * 200 + x] = 0;
            }
        }
        let caja = [20.0, 20.0, 180.0, 40.0];
        // Una mancha negra entera pareceria texto claro sobre oscuro: se dice.
        let r = recortar_linea(&g, &caja, 32, 1, Some(false), 4096);
        assert_eq!(r.alto, 32);
        // ancho: (160 + 2·0,3·20) · 32 / (20·1,4) = 172 · 32 / 28 ≈ 196,6 → 197
        assert_eq!(r.ancho, 197);
        let medio = r.datos[16 * r.ancho + r.ancho / 2];
        assert!(medio > 0.9, "{medio}");
        // Ventana alta: tres alturas.
        let t = recortar_linea(&g, &caja, 32, 3, Some(false), 4096);
        assert_eq!(t.alto, 96);
    }

    #[test]
    fn texto_claro_sobre_oscuro_se_detecta() {
        let mut g = Gris::lisa(100, 20, 30);
        for x in (10..90).step_by(4) {
            for y in 5..15 {
                g.px[y * 100 + x] = 240;
            }
        }
        assert!(linea_oscura(&g, &[0.0, 0.0, 99.0, 19.0]));
        let mut g = Gris::lisa(100, 20, 240);
        for x in (10..90).step_by(4) {
            for y in 5..15 {
                g.px[y * 100 + x] = 20;
            }
        }
        assert!(!linea_oscura(&g, &[0.0, 0.0, 99.0, 19.0]));
    }

    #[test]
    fn una_pagina_en_blanco_no_se_endereza_y_una_torcida_si() {
        assert_eq!(estimar_inclinacion(&Gris::lisa(400, 300, 255), 800), 0.0);
        // Renglones horizontales girados 4 grados.
        let mut g = Gris::lisa(600, 600, 255);
        for r in 0..12 {
            let y0 = 60 + r * 40;
            for y in y0..y0 + 10 {
                for x in 80..520 {
                    g.px[y * 600 + x] = 0;
                }
            }
        }
        let torcida = girar_pagina(&g, -4.0);
        let a = estimar_inclinacion(&torcida, 800);
        assert!((a - 4.0).abs() < 0.35, "{a}");
    }

    #[test]
    fn desgirar_una_caja_la_devuelve_a_su_sitio() {
        let b = [100.0, 100.0, 200.0, 120.0];
        let c = desgirar_caja(&b, 0.0, 400, 400);
        assert_eq!(c, b);
        // Girar 90 grados en una pagina cuadrada y volver.
        let c = desgirar_caja(&b, 90.0, 400, 400);
        assert!(
            (c[2] - c[0] - 20.0).abs() < 0.2 && (c[3] - c[1] - 100.0).abs() < 0.2,
            "{c:?}"
        );
    }
}
