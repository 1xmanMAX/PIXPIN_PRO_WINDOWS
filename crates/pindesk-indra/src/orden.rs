//! Orden de lectura y bloques de maquetacion de Indra (`order_post.py`,
//! `layout_post.py`; `IndraOrder.kt`): los rasgos de cada linea para
//! `order.onnx`, el orden desde sus logits por pares, la agrupacion de
//! lineas en bloques (enlace medio) y el XY-cut de respaldo.

// Las cuentas de matrices van por indice, como en `order_post.py` y en el
// puerto de Kotlin: asi se pueden comparar linea a linea con el original.
#![allow(clippy::needless_range_loop)]

use crate::imagen::Caja;

pub const CLASES_DE_BLOQUE: [&str; 10] = [
    "Text",
    "Section-Header",
    "Page-Header",
    "Page-Footer",
    "List-Group",
    "Table",
    "Caption",
    "Footnote",
    "Table-Of-Contents",
    "Form",
];
/// Tabla, formulario e indice: dentro se lee por filas.
const REJILLA: [usize; 3] = [5, 9, 8];
pub const N_RASGOS: usize = 16;
const FIN_DE_FRASE: &str = ".!?:;。！？：；．";

pub fn mediana(v: &[f32]) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(f32::total_cmp);
    let n = s.len();
    if n % 2 == 1 {
        s[n / 2]
    } else {
        (s[n / 2 - 1] + s[n / 2]) / 2.0
    }
}

fn ancho_doble(cp: u32) -> bool {
    matches!(cp,
        0x1100..=0x115F | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF | 0xA000..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6 | 0x20000..=0x3FFFD)
}

/// Ancho en columnas (CJK cuenta dos).
pub fn largo_visible(t: &str) -> usize {
    t.chars()
        .map(|c| if ancho_doble(c as u32) { 2 } else { 1 })
        .sum()
}

/// Los 16 rasgos de cada linea (`line_features`), fila a fila.
pub fn rasgos(cajas: &[Caja], ancho: f32, alto: f32, textos: &[String]) -> Vec<f32> {
    let n = cajas.len();
    let mut o = vec![0f32; n * N_RASGOS];
    if n == 0 {
        return o;
    }
    let hs: Vec<f32> = cajas.iter().map(|b| (b[3] - b[1]).max(1e-3)).collect();
    let med = mediana(&hs);
    for (i, b) in cajas.iter().enumerate() {
        let w = (b[2] - b[0]).max(1e-3);
        let h = hs[i];
        let f = &mut o[i * N_RASGOS..(i + 1) * N_RASGOS];
        f[0] = b[0] / ancho;
        f[1] = b[1] / alto;
        f[2] = b[2] / ancho;
        f[3] = b[3] / alto;
        f[4] = (b[0] + b[2]) / (2.0 * ancho);
        f[5] = (b[1] + b[3]) / (2.0 * alto);
        f[6] = w / ancho;
        f[7] = (h / med).ln();
        f[8] = (w / h).ln() / 4.0;
        f[9] = med / alto * 20.0;
        let t = textos.get(i).map(|t| t.trim()).unwrap_or("");
        if t.is_empty() {
            continue;
        }
        let largo = t.chars().count() as f32;
        f[10] = ((largo_visible(t) as f64).ln_1p() / 5.0) as f32;
        f[11] = t.chars().filter(|c| c.is_numeric()).count() as f32 / largo;
        let cabeza: String = t.chars().take(3).collect();
        let cabeza = cabeza.trim_end_matches(['.', ')']);
        let primero = t.chars().next().unwrap_or(' ');
        let cabeza_numero = !cabeza.is_empty() && cabeza.chars().all(|c| c.is_numeric());
        f[12] = if "•-–▪◦*·→".contains(primero) || (cabeza_numero && t.chars().count() > 3)
        {
            1.0
        } else {
            0.0
        };
        f[13] = if t.ends_with('-') { 1.0 } else { 0.0 };
        f[14] = if t.chars().last().is_some_and(|c| FIN_DE_FRASE.contains(c)) {
            1.0
        } else {
            0.0
        };
        f[15] = if primero.is_uppercase() { 1.0 } else { 0.0 };
    }
    o
}

fn softplus(x: f64) -> f64 {
    if x > 0.0 {
        x + (-x).exp().ln_1p()
    } else {
        x.exp().ln_1p()
    }
}

/// Orden desde los logits por pares `a[i][j] > 0`: i antes que j
/// (`decode_order`): ordenacion por puntuacion y barridos de insercion.
pub fn decodificar_orden(a: &[Vec<f64>], barridos: usize) -> Vec<usize> {
    let n = a.len();
    if n <= 1 {
        return (0..n).collect();
    }
    let lp: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| -softplus(-a[i][j])).collect())
        .collect();
    let clave: Vec<f64> = (0..n)
        .map(|i| {
            let r: f64 = (0..n).map(|j| lp[i][j]).sum();
            let c: f64 = (0..n).map(|j| lp[j][i]).sum();
            -r + c
        })
        .collect();
    let mut orden: Vec<usize> = (0..n).collect();
    orden.sort_by(|&x, &y| clave[x].total_cmp(&clave[y]).then(x.cmp(&y)));
    for _ in 0..barridos {
        let mut mejoro = false;
        for x in orden.clone() {
            let resto: Vec<usize> = orden.iter().copied().filter(|&v| v != x).collect();
            let r = resto.len();
            let mut pre = vec![0.0; r + 1];
            let mut acc = 0.0;
            for k in 0..r {
                pre[k] = acc;
                acc += lp[resto[k]][x];
            }
            pre[r] = acc;
            let mut suf = vec![0.0; r + 1];
            let mut s = 0.0;
            for k in (0..r).rev() {
                s += lp[x][resto[k]];
                suf[k] = s;
            }
            let ganancia: Vec<f64> = (0..=r).map(|k| pre[k] + suf[k]).collect();
            let mut mejor = 0;
            for k in 0..=r {
                if ganancia[k] > ganancia[mejor] {
                    mejor = k;
                }
            }
            let actual = orden.iter().position(|&v| v == x).unwrap_or(0);
            if ganancia[mejor] > ganancia[actual] + 1e-6 {
                let mut o = Vec::with_capacity(r + 1);
                for k in 0..=r {
                    if k == mejor {
                        o.push(x);
                    }
                    if k < r {
                        o.push(resto[k]);
                    }
                }
                orden = o;
                mejoro = true;
            }
        }
        if !mejoro {
            return orden;
        }
    }
    orden
}

/// Filas de arriba abajo, de izquierda a derecha (`order_top_down`).
pub fn orden_por_filas(cajas: &[Caja]) -> Vec<usize> {
    if cajas.is_empty() {
        return Vec::new();
    }
    let alturas: Vec<f32> = cajas.iter().map(|b| b[3] - b[1]).collect();
    let q = (mediana(&alturas) * 0.5).max(1e-3) as f64;
    let mut idx: Vec<usize> = (0..cajas.len()).collect();
    let fila = |i: usize| (((cajas[i][1] + cajas[i][3]) / 2.0) as f64 / q).round_ties_even();
    idx.sort_by(|&x, &y| {
        fila(x)
            .total_cmp(&fila(y))
            .then(cajas[x][0].total_cmp(&cajas[y][0]))
    });
    idx
}

struct Corte {
    hueco: f32,
    k: Option<usize>,
    orden: Vec<usize>,
}

fn mejor_corte(b: &[Caja], eje: usize) -> Corte {
    let (lo, hi) = if eje == 1 { (1, 3) } else { (0, 2) };
    let mut o: Vec<usize> = (0..b.len()).collect();
    o.sort_by(|&x, &y| b[x][lo].total_cmp(&b[y][lo]).then(x.cmp(&y)));
    let mut fin = b[o[0]][hi];
    let mut hueco = 0f32;
    let mut k = None;
    for i in 1..o.len() {
        let g = b[o[i]][lo] - fin;
        if g > hueco {
            hueco = g;
            k = Some(i);
        }
        fin = fin.max(b[o[i]][hi]);
    }
    Corte { hueco, k, orden: o }
}

/// XY-cut recursivo (`order_xycut`).
pub fn orden_xy(cajas: &[Caja]) -> Vec<usize> {
    fn rec(cajas: &[Caja], ids: Vec<usize>) -> Vec<usize> {
        if ids.len() <= 1 {
            return ids;
        }
        let b: Vec<Caja> = ids.iter().map(|&i| cajas[i]).collect();
        let cy = mejor_corte(&b, 1);
        let cx = mejor_corte(&b, 0);
        if cy.k.is_none() && cx.k.is_none() {
            return orden_por_filas(&b).into_iter().map(|i| ids[i]).collect();
        }
        let c = if cy.hueco >= cx.hueco { cy } else { cx };
        let k = c.k.unwrap_or(0);
        let mut a = rec(cajas, c.orden[..k].iter().map(|&i| ids[i]).collect());
        a.extend(rec(cajas, c.orden[k..].iter().map(|&i| ids[i]).collect()));
        a
    }
    rec(cajas, (0..cajas.len()).collect())
}

fn argmax(v: &[f64]) -> usize {
    let mut k = 0;
    for i in 1..v.len() {
        if v[i] > v[k] {
            k = i;
        }
    }
    k
}

/// Agrupacion por enlace medio de los logits «mismo grupo» (`_cluster`).
pub fn agrupar(logits: &[Vec<f32>]) -> Vec<Vec<usize>> {
    let n = logits.len();
    if n == 0 {
        return Vec::new();
    }
    let mut s: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| if i == j { 0.0 } else { logits[i][j] as f64 })
                .collect()
        })
        .collect();
    let mut tam = vec![1.0f64; n];
    let mut vivo = vec![true; n];
    let mut miembros: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
    let mut m: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| if i == j { f64::NEG_INFINITY } else { s[i][j] })
                .collect()
        })
        .collect();
    let mut mejor: Vec<usize> = (0..n).map(|i| argmax(&m[i])).collect();
    let mut mejor_v: Vec<f64> = (0..n).map(|i| m[i][mejor[i]]).collect();
    for _ in 0..n.saturating_sub(1) {
        let a0 = argmax(&mejor_v);
        let b0 = mejor[a0];
        if mejor_v[a0] <= 0.0 {
            break;
        }
        let (a, b) = (a0.min(b0), a0.max(b0));
        for j in 0..n {
            s[a][j] += s[b][j];
        }
        for j in 0..n {
            s[j][a] += s[j][b];
        }
        s[a][a] = 0.0;
        tam[a] += tam[b];
        vivo[b] = false;
        let mb = std::mem::take(&mut miembros[b]);
        miembros[a].extend(mb);
        let fila: Vec<f64> = (0..n)
            .map(|j| {
                if !vivo[j] || j == a {
                    f64::NEG_INFINITY
                } else {
                    s[a][j] / (tam[a] * tam[j])
                }
            })
            .collect();
        for j in 0..n {
            m[a][j] = fila[j];
            m[j][a] = fila[j];
        }
        for j in 0..n {
            m[b][j] = f64::NEG_INFINITY;
            m[j][b] = f64::NEG_INFINITY;
        }
        mejor_v[b] = f64::NEG_INFINITY;
        mejor[a] = argmax(&fila);
        mejor_v[a] = fila[mejor[a]];
        for j in 0..n {
            if !vivo[j] || j == a {
                continue;
            }
            if mejor[j] == a || mejor[j] == b {
                mejor[j] = argmax(&m[j]);
                mejor_v[j] = m[j][mejor[j]];
            }
        }
        for j in 0..n {
            if !vivo[j] || j == a {
                continue;
            }
            if fila[j] > mejor_v[j] {
                mejor[j] = a;
                mejor_v[j] = fila[j];
            }
        }
    }
    (0..n)
        .filter(|&i| vivo[i])
        .map(|i| {
            let mut v = miembros[i].clone();
            v.sort_unstable();
            v
        })
        .collect()
}

/// La clase de un bloque: argmax de la suma de los logits de sus lineas.
pub fn clase_de_bloque(cls: &[Vec<f32>], bloque: &[usize]) -> usize {
    let c = cls.first().map_or(0, |v| v.len());
    let mut suma = vec![0.0f64; c];
    for &i in bloque {
        for k in 0..c {
            suma[k] += cls[i][k] as f64;
        }
    }
    argmax(&suma)
}

/// Orden con las cabezas de maquetacion: bloques y luego lineas dentro
/// (`decode_blocks_order`).
pub fn decodificar_orden_por_bloques(
    a: &[Vec<f32>],
    mismo: &[Vec<f32>],
    cajas: &[Caja],
    cls: Option<&[Vec<f32>]>,
) -> Vec<usize> {
    let n = a.len();
    if n <= 1 {
        return (0..n).collect();
    }
    let bloques = agrupar(mismo);
    let dentro: Vec<Vec<usize>> = bloques
        .iter()
        .map(|b| {
            let bx: Vec<Caja> = b.iter().map(|&i| cajas[i]).collect();
            let local = if cls.is_some_and(|c| REJILLA.contains(&clase_de_bloque(c, b))) {
                orden_por_filas(&bx)
            } else {
                orden_xy(&bx)
            };
            local.into_iter().map(|i| b[i]).collect()
        })
        .collect();
    let m = bloques.len();
    let mut ab = vec![vec![0.0f64; m]; m];
    for p in 0..m {
        for q in p + 1..m {
            let mut s = 0.0;
            for &i in &bloques[p] {
                for &j in &bloques[q] {
                    s += a[i][j] as f64;
                }
            }
            let v = (s / (bloques[p].len() as f64 * bloques[q].len() as f64)) as f32 as f64;
            ab[p][q] = v;
            ab[q][p] = -v;
        }
    }
    decodificar_orden(&ab, 4)
        .into_iter()
        .flat_map(|i| dentro[i].clone())
        .collect()
}

/// Tramos del orden de lectura con «mismo bloque» positivo entre vecinos.
pub fn partir_en_bloques(orden: &[usize], mismo: &[Vec<f32>]) -> Vec<Vec<usize>> {
    let mut o: Vec<Vec<usize>> = Vec::new();
    for &i in orden {
        if let Some(ultimo) = o.last_mut()
            && mismo[*ultimo.last().unwrap_or(&i)][i] > 0.0
        {
            ultimo.push(i);
            continue;
        }
        o.push(vec![i]);
    }
    o
}

/// Un bloque de maquetacion: etiqueta, caja union e indices de linea.
#[derive(Debug, Clone, PartialEq)]
pub struct Bloque {
    pub etiqueta: &'static str,
    pub caja: Caja,
    pub lineas: Vec<usize>,
}

fn union(cajas: &[Caja], ids: &[usize]) -> Caja {
    let mut u = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for &i in ids {
        let b = cajas[i];
        u[0] = u[0].min(b[0]);
        u[1] = u[1].min(b[1]);
        u[2] = u[2].max(b[2]);
        u[3] = u[3].max(b[3]);
    }
    u
}

/// Bloques en orden de lectura (`page_blocks`).
pub fn bloques_de_pagina(
    cajas: &[Caja],
    orden: &[usize],
    cls: &[Vec<f32>],
    mismo: &[Vec<f32>],
) -> Vec<Bloque> {
    partir_en_bloques(orden, mismo)
        .into_iter()
        .map(|b| Bloque {
            etiqueta: CLASES_DE_BLOQUE
                .get(clase_de_bloque(cls, &b))
                .copied()
                .unwrap_or("Text"),
            caja: union(cajas, &b),
            lineas: b,
        })
        .collect()
}

/// Bloques por XY-cut, sin modelo de orden: huecos de al menos `hueco_min`
/// alturas medianas (`xycut_blocks`).
pub fn bloques_xy(cajas: &[Caja], hueco_min: f32) -> Vec<Vec<usize>> {
    if cajas.is_empty() {
        return Vec::new();
    }
    let alturas: Vec<f32> = cajas.iter().map(|b| b[3] - b[1]).collect();
    let h = mediana(&alturas);
    fn rec(cajas: &[Caja], ids: Vec<usize>, h: f32, hueco_min: f32) -> Vec<Vec<usize>> {
        let b: Vec<Caja> = ids.iter().map(|&i| cajas[i]).collect();
        if ids.len() > 1 {
            let cy = mejor_corte(&b, 1);
            let cx = mejor_corte(&b, 0);
            let c = if cy.hueco >= cx.hueco { cy } else { cx };
            if let Some(k) = c.k
                && c.hueco >= hueco_min * h
            {
                let mut a = rec(
                    cajas,
                    c.orden[..k].iter().map(|&i| ids[i]).collect(),
                    h,
                    hueco_min,
                );
                a.extend(rec(
                    cajas,
                    c.orden[k..].iter().map(|&i| ids[i]).collect(),
                    h,
                    hueco_min,
                ));
                return a;
            }
        }
        vec![orden_xy(&b).into_iter().map(|i| ids[i]).collect()]
    }
    rec(cajas, (0..cajas.len()).collect(), h, hueco_min)
}

/// La caja union de un bloque (para el respaldo sin modelo de orden).
pub fn caja_de(cajas: &[Caja], ids: &[usize]) -> Caja {
    union(cajas, ids)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn orden_por_pares_y_agrupacion() {
        // IndraUnitTest.ordenPorParesYAgrupacion: orden real 2, 0, 1.
        let pos = [1, 2, 0];
        let a: Vec<Vec<f64>> = (0..3)
            .map(|i| {
                (0..3)
                    .map(|j| {
                        if i == j {
                            0.0
                        } else if pos[i] < pos[j] {
                            4.0
                        } else {
                            -4.0
                        }
                    })
                    .collect()
            })
            .collect();
        assert_eq!(decodificar_orden(&a, 4), [2, 0, 1]);
        let mismo: Vec<Vec<f32>> = (0..4)
            .map(|i| {
                (0..4)
                    .map(|j| if (i < 2) == (j < 2) { 3.0 } else { -3.0 })
                    .collect()
            })
            .collect();
        let mut g = agrupar(&mismo);
        g.sort();
        assert_eq!(g, vec![vec![0, 1], vec![2, 3]]);
    }

    #[test]
    fn xy_cut_con_dos_columnas() {
        let b = [
            [10.0, 10.0, 90.0, 20.0],
            [110.0, 10.0, 190.0, 20.0],
            [10.0, 30.0, 90.0, 40.0],
            [110.0, 30.0, 190.0, 40.0],
        ];
        assert_eq!(orden_xy(&b), [0, 2, 1, 3]);
        assert_eq!(orden_por_filas(&b), [0, 1, 2, 3]);
        // Con huecos de una altura se parte todo; con dos, solo las columnas.
        assert_eq!(bloques_xy(&b, 1.0).len(), 4);
        assert_eq!(bloques_xy(&b, 2.0), vec![vec![0, 2], vec![1, 3]]);
    }

    #[test]
    fn rasgos_de_linea() {
        let f = rasgos(
            &[[0.0, 0.0, 100.0, 10.0]],
            200.0,
            100.0,
            &["12) Hola.".into()],
        );
        assert_eq!(f.len(), 16);
        assert!((f[2] - 0.5).abs() < 1e-6);
        assert_eq!(f[12], 1.0); // empieza por enumerador
        assert_eq!(f[14], 1.0); // termina en punto
        assert_eq!(f[15], 0.0); // «1» no es mayuscula
    }

    #[test]
    fn bloques_de_pagina_con_su_etiqueta() {
        let cajas = [
            [0.0, 0.0, 10.0, 5.0],
            [0.0, 6.0, 10.0, 11.0],
            [0.0, 30.0, 10.0, 35.0],
        ];
        let mismo = vec![
            vec![0.0, 2.0, -2.0],
            vec![2.0, 0.0, -2.0],
            vec![-2.0, -2.0, 0.0],
        ];
        let mut cls = vec![vec![0.0f32; 10]; 3];
        cls[0][1] = 5.0;
        cls[1][1] = 5.0;
        cls[2][0] = 5.0;
        let b = bloques_de_pagina(&cajas, &[0, 1, 2], &cls, &mismo);
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].etiqueta, "Section-Header");
        assert_eq!(b[0].caja, [0.0, 0.0, 10.0, 11.0]);
        assert_eq!(b[1].lineas, [2]);
    }
}
