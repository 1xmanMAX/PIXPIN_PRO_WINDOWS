//! Lo que aun no se ha colocado de una galaxia (D228). No se guarda: estar
//! en la nebulosa es no tener entrada en `universo.json`.

use std::collections::HashSet;
use std::ops::Range;

use crate::astro::Astro;
use crate::ficha::FichaLuna;

pub const POR_PAGINA: usize = 60;
pub const COLUMNAS: usize = 10;
pub const CELDA: f32 = 120.0;
/// Donde empieza la rejilla, en radios bajo el centro. Con 10 x 6 celdas
/// de 120 cabe entera en el circulo (lo comprueba una prueba).
const DESDE: f32 = 0.55;

pub fn sin_colocar<'a>(fichas: &'a [FichaLuna], colocadas: &HashSet<&str>) -> Vec<&'a FichaLuna> {
    let mut v: Vec<&FichaLuna> = fichas
        .iter()
        .filter(|f| !colocadas.contains(f.codigo.as_str()))
        .collect();
    v.sort_by_key(|f| std::cmp::Reverse(f.cuando));
    v
}

pub fn paginas(total: usize) -> usize {
    total.div_ceil(POR_PAGINA).max(1)
}

/// El trozo de la lista que ensena la pagina `n` (desde 0).
pub fn pagina(total: usize, n: usize) -> Range<usize> {
    let n = n.min(paginas(total) - 1);
    let desde = n * POR_PAGINA;
    desde.min(total)..(desde + POR_PAGINA).min(total)
}

/// La caja en mundo de la celda `i` (0..60) de una galaxia.
pub fn celda(g: &Astro, i: usize) -> (f32, f32, f32, f32) {
    let (col, fila) = ((i % COLUMNAS) as f32, (i / COLUMNAS) as f32);
    let x0 = g.x - CELDA * COLUMNAS as f32 / 2.0 + col * CELDA;
    let y0 = g.y + g.radio * DESDE + fila * CELDA;
    (x0, y0, x0 + CELDA, y0 + CELDA)
}

/// Que celda hay en un punto del mundo, si hay alguna.
pub fn luna_en(g: &Astro, x: f32, y: f32) -> Option<usize> {
    (0..POR_PAGINA).find(|i| {
        let (x0, y0, x1, y1) = celda(g, *i);
        x >= x0 && x < x1 && y >= y0 && y < y1
    })
}

/// El chip «+N mas», a la derecha de la ultima fila.
pub fn chip_mas(g: &Astro) -> (f32, f32, f32, f32) {
    let (_, y0, x1, y1) = celda(g, POR_PAGINA - 1);
    (x1 + 20.0, y0, x1 + 20.0 + CELDA * 1.5, y1)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::astro::{Astro, IdAstro};
    use crate::ficha::{ClaseLuna, FichaLuna};
    use std::collections::HashSet;

    fn ficha(codigo: &str, cuando: i64) -> FichaLuna {
        FichaLuna {
            codigo: codigo.into(),
            proyecto: "p".into(),
            clase: ClaseLuna::Archivo,
            cuando,
            ..Default::default()
        }
    }

    #[test]
    fn la_nebulosa_tiene_lo_no_colocado_de_la_mas_nueva_a_la_mas_vieja() {
        let fichas = vec![ficha("a", 1), ficha("b", 3), ficha("c", 2)];
        let colocadas: HashSet<&str> = ["c"].into_iter().collect();
        let s: Vec<&str> = sin_colocar(&fichas, &colocadas)
            .iter()
            .map(|f| f.codigo.as_str())
            .collect();
        assert_eq!(s, vec!["b", "a"]);
    }

    #[test]
    fn las_paginas_son_de_sesenta_y_la_ultima_lleva_lo_que_sobra() {
        assert_eq!(paginas(0), 1);
        assert_eq!(paginas(60), 1);
        assert_eq!(paginas(61), 2);
        assert_eq!(pagina(130, 2), 120..130);
        assert_eq!(
            pagina(130, 9),
            120..130,
            "una pagina de mas se queda en la ultima"
        );
    }

    #[test]
    fn toda_la_rejilla_cabe_dentro_de_la_galaxia() {
        let g = Astro::galaxia(IdAstro(1), "p", 0.0, 0.0);
        for i in 0..POR_PAGINA {
            let (x0, y0, x1, y1) = celda(&g, i);
            assert!(g.contiene_caja((x0, y0, x1, y1)), "la celda {i} se sale");
        }
    }

    #[test]
    fn luna_en_encuentra_la_celda_pulsada_y_nada_fuera() {
        let g = Astro::galaxia(IdAstro(1), "p", 0.0, 0.0);
        let (x0, y0, _, _) = celda(&g, 13);
        assert_eq!(luna_en(&g, x0 + 5.0, y0 + 5.0), Some(13));
        assert_eq!(luna_en(&g, 0.0, 0.0), None);
    }
}
