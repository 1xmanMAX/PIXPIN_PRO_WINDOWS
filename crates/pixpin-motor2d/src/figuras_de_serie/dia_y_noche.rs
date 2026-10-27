//! **El interruptor de dia y noche de los paneles de figuras** (27-oct-2026,
//! puerto de `FigurasDeSerie.segunElModo` del movil, dd1af9e2). El usuario:
//! «en la zona de figuras las figuras se ven en modo noche por defecto; no
//! debe ser asi», y pidio un interruptor ☀/☾.
//!
//! El catalogo trae cada plantilla de pagina dos veces, la de dia y su
//! gemela «(noche)» (`noche.rs`), y los paneles las ensenaban mezcladas: en
//! Planificacion, Academico, Creativo y Diario la mitad de las miniaturas
//! eran hojas azul noche, y buscando «diario» salian las dos. Ahora:
//!
//! - **De dia**, ninguna gemela de noche: lo de siempre.
//! - **De noche**, cada una que tiene gemela se cambia **en su sitio** por
//!   la gemela; las que no la tienen (los simbolos, los muebles, las
//!   basicas) siguen como son, que no tienen version oscura y hacen falta
//!   igual.
//!
//! Una gemela de noche sin su de dia (no pasa en el catalogo) sale solo de
//! noche, al final.
//!
//! Recibe pares `(id, nombre)` y devuelve posiciones, para que cada panel lo
//! use con su propia lista (la del lienzo guarda `FiguraGuardada`; la del
//! chat, su vista ya preparada con el nombre sin la categoria).

use std::collections::HashMap;

use super::{PREFIJO_ID, SUFIJO_NOCHE};

/// El final del nombre de la gemela de noche («Planificación · Cronograma
/// (noche)»), como lo pone `noche::gemela`.
pub const MARCA_NOCHE: &str = " (noche)";

/// **Si es la gemela de noche de otra.** Por el id en las de serie («Mesita
/// de noche redonda» es un mueble, no una plantilla de noche) y por el final
/// del nombre en las demas (una propia que alguien guardo asi).
pub fn es_gemela_de_noche(id: &str, nombre: &str) -> bool {
    if id.starts_with(PREFIJO_ID) {
        id.ends_with(SUFIJO_NOCHE)
    } else {
        nombre.ends_with(MARCA_NOCHE)
    }
}

/// **Lo que ensena el panel en cada modo**: las posiciones de `figuras`
/// (pares `(id, nombre)`) que se ven, en el orden en que se ven.
pub fn segun_el_modo(figuras: &[(&str, &str)], noche: bool) -> Vec<usize> {
    let es: Vec<bool> = figuras
        .iter()
        .map(|(id, nombre)| es_gemela_de_noche(id, nombre))
        .collect();
    let de_dia = (0..figuras.len()).filter(|i| !es[*i]);
    if !noche {
        return de_dia.collect();
    }
    let mut por_id: HashMap<&str, usize> = HashMap::new();
    let mut por_nombre: HashMap<&str, usize> = HashMap::new();
    for (i, (id, nombre)) in figuras.iter().enumerate().filter(|(i, _)| es[*i]) {
        por_id.entry(id).or_insert(i);
        por_nombre.entry(nombre).or_insert(i);
    }
    let mut usadas = vec![false; figuras.len()];
    let mut v: Vec<usize> = de_dia
        .map(|i| {
            let (id, nombre) = figuras[i];
            let gemela = por_id
                .get(format!("{id}{SUFIJO_NOCHE}").as_str())
                .or_else(|| por_nombre.get(format!("{nombre}{MARCA_NOCHE}").as_str()))
                .copied()
                .filter(|j| !usadas[*j]);
            match gemela {
                Some(j) => {
                    usadas[j] = true;
                    j
                }
                None => i,
            }
        })
        .collect();
    v.extend((0..figuras.len()).filter(|i| es[*i] && !usadas[*i]));
    v
}

/// **La pareja de `i` en el otro modo**, para que al cambiar el interruptor
/// la elegida siga siendo «la misma» (el diario de dia pasa a su gemela de
/// noche y al reves). `None` si no tiene.
pub fn pareja(figuras: &[(&str, &str)], i: usize) -> Option<usize> {
    let (id, nombre) = *figuras.get(i)?;
    if es_gemela_de_noche(id, nombre) {
        let dia_id = id
            .strip_suffix(SUFIJO_NOCHE)
            .filter(|_| id.starts_with(PREFIJO_ID));
        let dia_nombre = nombre.strip_suffix(MARCA_NOCHE);
        figuras.iter().position(|(o, n)| {
            !es_gemela_de_noche(o, n)
                && (dia_id.is_some_and(|d| d == *o)
                    || (dia_id.is_none() && dia_nombre.is_some_and(|d| d == *n)))
        })
    } else {
        let por_id = format!("{id}{SUFIJO_NOCHE}");
        let por_nombre = format!("{nombre}{MARCA_NOCHE}");
        figuras.iter().position(|(o, _)| *o == por_id).or_else(|| {
            figuras
                .iter()
                .position(|(o, n)| *n == por_nombre && es_gemela_de_noche(o, n))
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::super::{CATEGORIAS, catalogo, categoria_de};
    use super::*;
    use crate::biblioteca::FiguraGuardada;

    fn pares(v: &[FiguraGuardada]) -> Vec<(&str, &str)> {
        v.iter()
            .map(|f| (f.id.as_str(), f.nombre.as_str()))
            .collect()
    }

    fn ids<'a>(v: &'a [FiguraGuardada], l: &[usize]) -> Vec<&'a str> {
        l.iter().map(|i| v[*i].id.as_str()).collect()
    }

    /// Sin tildes ni mayusculas, como buscan los paneles.
    fn plegar(s: &str) -> String {
        s.to_lowercase()
            .replace('á', "a")
            .replace('é', "e")
            .replace('í', "i")
            .replace('ó', "o")
            .replace('ú', "u")
    }

    fn buscar(v: &[FiguraGuardada], l: &[usize], q: &str) -> Vec<usize> {
        let palabras: Vec<String> = plegar(q).split_whitespace().map(String::from).collect();
        l.iter()
            .copied()
            .filter(|i| {
                let n = plegar(&v[*i].nombre);
                palabras.iter().all(|p| n.contains(p.as_str()))
            })
            .collect()
    }

    #[test]
    fn cada_gemela_de_noche_del_catalogo_tiene_su_de_dia() {
        let c = catalogo();
        let n = c
            .iter()
            .filter(|f| es_gemela_de_noche(&f.id, &f.nombre))
            .count();
        assert!(n >= 25, "{n}");
        for f in c.iter().filter(|f| es_gemela_de_noche(&f.id, &f.nombre)) {
            assert!(f.nombre.ends_with(MARCA_NOCHE), "{}", f.nombre);
            let dia = f.id.strip_suffix(SUFIJO_NOCHE).unwrap();
            assert!(c.iter().any(|d| d.id == dia), "{}", f.id);
        }
    }

    #[test]
    fn una_mesita_de_noche_no_es_una_plantilla_de_noche() {
        // Caso negativo: el nombre lleva «noche», pero es un mueble.
        let c = catalogo();
        for f in c.iter().filter(|f| plegar(&f.nombre).contains("noche")) {
            if !f.nombre.ends_with(MARCA_NOCHE) {
                assert!(!es_gemela_de_noche(&f.id, &f.nombre), "{}", f.nombre);
            }
        }
        assert!(!es_gemela_de_noche(
            "serie-x-noche-redonda",
            "Mueble · Mesita de noche"
        ));
    }

    #[test]
    fn de_dia_no_sale_ninguna_de_noche() {
        let c = catalogo();
        let p = pares(&c);
        let dia = segun_el_modo(&p, false);
        let gemelas = p.iter().filter(|(i, n)| es_gemela_de_noche(i, n)).count();
        assert_eq!(dia.len(), c.len() - gemelas);
        assert!(dia.iter().all(|i| !es_gemela_de_noche(p[*i].0, p[*i].1)));
    }

    #[test]
    fn de_noche_cada_gemela_va_en_el_sitio_de_su_de_dia_y_lo_demas_se_queda() {
        let c = catalogo();
        let p = pares(&c);
        let dia = segun_el_modo(&p, false);
        let noche = segun_el_modo(&p, true);
        assert_eq!(dia.len(), noche.len());
        let mut cambiadas = 0;
        for (d, n) in dia.iter().zip(&noche) {
            if d != n {
                assert_eq!(format!("{}{SUFIJO_NOCHE}", c[*d].id), c[*n].id);
                cambiadas += 1;
            }
        }
        let gemelas = p.iter().filter(|(i, n)| es_gemela_de_noche(i, n)).count();
        assert_eq!(cambiadas, gemelas);
        // Un simbolo de electronica es el mismo de dia y de noche.
        let elec = |l: &[usize]| -> Vec<usize> {
            l.iter()
                .copied()
                .filter(|i| c[*i].nombre.starts_with("Electrónica"))
                .collect()
        };
        assert!(!elec(&dia).is_empty());
        assert_eq!(elec(&dia), elec(&noche));
        let diario = c.iter().position(|f| f.id == "serie-diario").unwrap();
        let diario_noche = pareja(&p, diario).unwrap();
        assert_eq!(c[diario_noche].id, "serie-diario-noche");
        assert_eq!(pareja(&p, diario_noche), Some(diario));
    }

    #[test]
    fn las_categorias_son_las_mismas_en_los_dos_modos_y_cada_una_con_las_mismas_cuantas() {
        let c = catalogo();
        let p = pares(&c);
        let cuenta = |noche: bool| -> Vec<(&str, usize)> {
            CATEGORIAS
                .iter()
                .map(|cat| {
                    let n = segun_el_modo(&p, noche)
                        .into_iter()
                        .filter(|i| categoria_de(&c[*i].nombre).is_some_and(|(x, _)| x == *cat))
                        .count();
                    (*cat, n)
                })
                .collect()
        };
        assert_eq!(cuenta(false), cuenta(true));
    }

    #[test]
    fn buscando_se_respeta_el_modo() {
        let c = catalogo();
        let p = pares(&c);
        let dia = buscar(&c, &segun_el_modo(&p, false), "habitos");
        assert_eq!(ids(&c, &dia), ["serie-habitos"]);
        let noche = buscar(&c, &segun_el_modo(&p, true), "HÁBITOS");
        assert_eq!(ids(&c, &noche), ["serie-habitos-noche"]);
    }

    fn f(id: &str, nombre: &str) -> FiguraGuardada {
        FiguraGuardada {
            id: id.into(),
            nombre: nombre.into(),
            elementos: Vec::new(),
        }
    }

    #[test]
    fn una_propia_con_su_gemela_por_el_nombre_tambien_cambia() {
        let mias = [f("a", "Mi hoja"), f("b", "Mi hoja (noche)"), f("c", "Otra")];
        let p = pares(&mias);
        assert_eq!(ids(&mias, &segun_el_modo(&p, false)), ["a", "c"]);
        assert_eq!(ids(&mias, &segun_el_modo(&p, true)), ["b", "c"]);
        assert_eq!(pareja(&p, 0), Some(1));
        assert_eq!(pareja(&p, 1), Some(0));
        assert_eq!(pareja(&p, 2), None);
    }

    #[test]
    fn una_de_noche_sin_su_de_dia_sale_solo_de_noche() {
        let l = [
            f("serie-x-noche", "Diario · X (noche)"),
            f("serie-y", "Diario · Y"),
        ];
        let p = pares(&l);
        assert_eq!(ids(&l, &segun_el_modo(&p, false)), ["serie-y"]);
        assert_eq!(
            ids(&l, &segun_el_modo(&p, true)),
            ["serie-y", "serie-x-noche"]
        );
        assert_eq!(pareja(&p, 0), None);
    }
}
