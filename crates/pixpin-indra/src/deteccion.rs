//! De la salida de `det.onnx` a cajas de renglon (`det_post.decode`).
//!
//! El detector da, a media resolucion, tres planos: el logit del «nucleo»
//! de cada renglon (una banda estrecha en su centro) y, en cada pixel del
//! nucleo, cuanto hay hasta el borde de arriba y hasta el de abajo. Cada
//! mancha de nucleo es un renglon: su alto sale de la media de esas dos
//! distancias y su ancho, de la mancha mas un poco a cada lado.

use crate::gris::componentes;
use crate::imagen::Caja;

/// La salida va a la mitad de la entrada.
pub const PASO_SALIDA: f64 = 2.0;
/// El nucleo es mas corto que el renglon esta fraccion de su alto por lado.
pub const ENCOGE_NUCLEO: f64 = 0.15;

/// Cajas `[x0, y0, x1, y1]` en pixeles de la ENTRADA del detector.
/// `salida` son los tres primeros planos `[3, alto, ancho]` (puede traer
/// mas: el de imagenes, que aqui no se usa).
pub fn decodificar(
    salida: &[f32],
    ancho: usize,
    alto: usize,
    umbral: f32,
    area_min: usize,
) -> Vec<Caja> {
    let plano = ancho * alto;
    if salida.len() < 3 * plano || plano == 0 {
        return Vec::new();
    }
    let mascara: Vec<u8> = salida[..plano]
        .iter()
        .map(|&l| u8::from(1.0 / (1.0 + (-l).exp()) > umbral))
        .collect();
    let (etiquetas, manchas) = componentes(&mascara, ancho, alto);
    if manchas.is_empty() {
        return Vec::new();
    }
    let n = manchas.len() + 1;
    let mut cuenta = vec![0f64; n];
    let mut arriba = vec![0f64; n];
    let mut abajo = vec![0f64; n];
    for (i, &e) in etiquetas.iter().enumerate() {
        if e == 0 {
            continue;
        }
        let e = e as usize;
        let ys = ((i / ancho) as f64 + 0.5) * PASO_SALIDA;
        cuenta[e] += 1.0;
        arriba[e] += ys - salida[plano + i] as f64;
        abajo[e] += ys + salida[2 * plano + i] as f64;
    }
    let mut cajas = Vec::new();
    for (k, m) in manchas.iter().enumerate() {
        let e = k + 1;
        if m.area < area_min {
            continue;
        }
        let c = cuenta[e].max(1.0);
        let (y0, y1) = (arriba[e] / c, abajo[e] / c);
        let lh = (y1 - y0).max(1.0);
        let ex = (ENCOGE_NUCLEO * lh).min(0.5 * lh);
        let x0 = m.izquierda as f64 * PASO_SALIDA - ex;
        let x1 = (m.izquierda + m.ancho) as f64 * PASO_SALIDA + ex;
        cajas.push([x0 as f32, y0 as f32, x1 as f32, y1 as f32]);
    }
    cajas
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_banda_de_nucleo_es_un_renglon_con_su_alto() {
        // Salida 20 x 10; nucleo en las filas 4-5, columnas 2-15.
        let (w, h) = (20, 10);
        let mut s = vec![-10.0f32; 3 * w * h];
        for y in 4..6 {
            for x in 2..16 {
                let i = y * w + x;
                s[i] = 10.0;
                // Borde a 6 px por arriba y a 6 por abajo del centro del pixel.
                s[w * h + i] = 6.0;
                s[2 * w * h + i] = 6.0;
            }
        }
        let c = decodificar(&s, w, h, 0.5, 2);
        assert_eq!(c.len(), 1);
        let [x0, y0, x1, y1] = c[0];
        // ys medio = (9 + 11) / 2 = 10 → 4..16 de alto 12; ex = 1,8.
        assert!((y0 - 4.0).abs() < 1e-4 && (y1 - 16.0).abs() < 1e-4);
        assert!(
            (x0 - 2.2).abs() < 1e-4 && (x1 - 33.8).abs() < 1e-4,
            "{x0} {x1}"
        );
        // Nada encima del umbral: ningun renglon.
        assert!(decodificar(&vec![-5.0; 3 * w * h], w, h, 0.5, 2).is_empty());
    }
}
