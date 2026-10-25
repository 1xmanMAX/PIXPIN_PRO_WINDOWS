//! **La pagina preparada para Indra** (`IndraPrep.kt` del escaner): el fondo
//! normalizado —papel a blanco, sin sombras ni degradados— y un enfoque
//! suave. Es lo que ve el OCR, no el usuario.
//!
//! Indra lee «tinta» = 1 − gris/255: un papel gris o una sombra son tinta
//! tenue para el detector y el reconocedor. En el banco del escaner (54
//! imagenes con verdad) llevar el papel a blanco subio el espanol impreso
//! de 73,9 a 91,9 % de palabras. CLAHE, gamma, umbrales suaves o reescalar
//! a una altura de renglon se probaron alli y salieron iguales o peores.
//!
//! En una captura de pantalla el papel ya es liso: la cuenta deja el fondo
//! igual y solo enfoca un poco (lo que tampoco le hace dano a un texto
//! nitido).

use crate::gris::{Gris, Interpolacion, a_byte, cerrar, desenfocar, mediana5, redimensionar};

/// Lado al que se estima el papel.
pub const LADO_FONDO: usize = 512;
/// Diametro del cierre que borra la tinta al estimar el papel (fraccion del
/// lado reducido).
pub const NUCLEO_FONDO: f64 = 1.0 / 30.0;
pub const ENFOQUE: f32 = 0.6;
pub const SIGMA_ENFOQUE: f64 = 1.0;

/// Si conviene preparar la pagina: no en las oscuras (modo oscuro, una
/// terminal). Ahi el «papel» es el fondo negro y dividir por el
/// convierte el texto claro en blanco sobre blanco; el banco de PixPin lo
/// vio con una captura de codigo (CER 0 → 1,95 % preparandola).
pub fn conviene(g: &Gris) -> bool {
    let mut hist = [0usize; 256];
    for &v in &g.px {
        hist[v as usize] += 1;
    }
    let mitad = g.px.len() / 2;
    let mut acc = 0;
    for (i, n) in hist.iter().enumerate() {
        acc += n;
        if acc > mitad {
            return i >= 128;
        }
    }
    true
}

/// La pagina preparada (nueva; `g` no cambia).
pub fn preparar(g: &Gris) -> Gris {
    enfocar(&normalizar_fondo(g), ENFOQUE, SIGMA_ENFOQUE)
}

/// gris / papel · 255, con el papel = cierre + mediana + suavizado a 512 px.
pub fn normalizar_fondo(g: &Gris) -> Gris {
    let (w, h) = (g.ancho, g.alto);
    let s = LADO_FONDO as f64 / w.max(h) as f64;
    let pequena = if s < 1.0 {
        redimensionar(
            g,
            ((w as f64 * s).round() as usize).max(1),
            ((h as f64 * s).round() as usize).max(1),
            Interpolacion::Area,
        )
    } else {
        g.clone()
    };
    let k = ((pequena.ancho.max(pequena.alto) as f64 * NUCLEO_FONDO) as usize | 1).max(5);
    let fondo = mediana5(&cerrar(&pequena, k));
    let suave = desenfocar(&fondo, k as f64 / 3.0);
    let fondo = Gris {
        ancho: fondo.ancho,
        alto: fondo.alto,
        px: suave.into_iter().map(a_byte).collect(),
    };
    let fondo = if s < 1.0 {
        redimensionar(&fondo, w, h, Interpolacion::Lineal)
    } else {
        fondo
    };
    let px =
        g.px.iter()
            .zip(&fondo.px)
            .map(|(&v, &p)| a_byte(v as f32 * 255.0 / (p as f32).max(1.0)))
            .collect();
    Gris {
        ancho: w,
        alto: h,
        px,
    }
}

/// Mascara de enfoque: `(1 + a)·g − a·desenfoque(g)`.
pub fn enfocar(g: &Gris, a: f32, sigma: f64) -> Gris {
    let b = desenfocar(g, sigma);
    let px =
        g.px.iter()
            .zip(b)
            .map(|(&v, d)| a_byte((1.0 + a) * v as f32 - a * d))
            .collect();
    Gris {
        ancho: g.ancho,
        alto: g.alto,
        px,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_preparacion_blanquea_la_sombra_y_conserva_la_tinta() {
        // IndraAfinadoTest.preparacionBlanqueaSombraYConservaTinta: papel de
        // 230 a 120 en degradado y renglones negros encima.
        let (w, h) = (800, 600);
        let mut g = Gris::lisa(w, h, 0);
        for y in 0..h {
            for x in 0..w {
                g.px[y * w + x] = (230.0 - 110.0 * x as f64 / w as f64) as u8;
            }
        }
        for i in 0..5 {
            let y0 = 65 + 100 * i;
            for y in y0..y0 + 18 {
                for x in (30..600).filter(|x| (x / 6) % 3 != 2) {
                    g.px[y * w + x] = 25;
                }
            }
        }
        let p = preparar(&g);
        assert_eq!((p.ancho, p.alto), (w, h));
        assert!(p.en(20, 40) > 235, "papel claro {}", p.en(20, 40));
        assert!(
            p.en(780, 40) > 235 && g.en(780, 40) < 130,
            "papel en sombra {} (antes {})",
            p.en(780, 40),
            g.en(780, 40)
        );
        let tinta = (30..600).map(|x| p.en(x, 75)).min().unwrap();
        assert!(tinta < 80, "tinta {tinta}");
    }

    #[test]
    fn una_pagina_oscura_no_se_prepara() {
        assert!(!conviene(&Gris::lisa(10, 10, 30)));
        assert!(conviene(&Gris::lisa(10, 10, 240)));
    }

    #[test]
    fn una_captura_lisa_queda_igual() {
        let g = Gris::lisa(300, 200, 250);
        let p = preparar(&g);
        assert!(p.px.iter().all(|&v| v >= 254), "{:?}", &p.px[..4]);
    }
}
