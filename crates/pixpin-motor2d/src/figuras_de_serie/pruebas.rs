use super::*;
use crate::biblioteca::{self, MAX_PIEZAS};
use std::collections::HashSet;

#[test]
fn el_catalogo_tiene_ids_fijos_unicos_y_cada_figura_su_categoria() {
    let c = catalogo();
    assert!(c.len() >= 70, "{}", c.len());
    let mut ids = HashSet::new();
    let mut nombres = HashSet::new();
    for f in &c {
        assert!(es_de_serie(f), "{}", f.id);
        assert!(ids.insert(f.id.clone()), "id repetido: {}", f.id);
        assert!(nombres.insert(f.nombre.clone()), "nombre repetido: {}", f.nombre);
        let (cat, _) = categoria_de(&f.nombre).unwrap_or_else(|| panic!("sin categoria: {}", f.nombre));
        assert!(CATEGORIAS.contains(&cat), "{cat}");
        assert!(!f.elementos.is_empty() && f.elementos.len() < MAX_PIEZAS, "{} {}", f.nombre, f.elementos.len());
    }
    for cat in CATEGORIAS {
        assert!(c.iter().any(|f| categoria_de(&f.nombre).is_some_and(|(k, _)| k == cat)), "{cat} vacia");
    }
}

#[test]
fn todo_es_finito_y_esta_puesto_en_el_origen() {
    for f in catalogo() {
        let (x0, y0, x1, y1) = biblioteca::caja_de(&f.elementos).unwrap();
        assert!(x0.abs() < 0.01 && y0.abs() < 0.01, "{} ({x0}, {y0})", f.nombre);
        assert!(x1.is_finite() && y1.is_finite() && x1 > 20.0 && y1 > 20.0, "{}", f.nombre);
        for e in &f.elementos {
            assert!(e.x.is_finite() && e.y.is_finite() && e.ancho >= 0.0 && e.alto >= 0.0, "{}", f.nombre);
        }
    }
}

#[test]
fn viajan_en_el_formato_del_movil_sin_perder_piezas() {
    let c = catalogo();
    let leidas = biblioteca::desde_json(&biblioteca::a_json(&c));
    assert_eq!(leidas.len(), c.len());
    for (a, b) in c.iter().zip(&leidas) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.nombre, b.nombre);
        assert_eq!(a.elementos.len(), b.elementos.len(), "{}", a.nombre);
    }
}

#[test]
fn instalar_sustituye_las_de_serie_y_respeta_las_del_usuario() {
    let propia = FiguraGuardada { id: "abc".into(), nombre: "Mi flecha".into(), elementos: catalogo()[0].elementos.clone() };
    let tapa = FiguraGuardada { id: "xyz".into(), nombre: "Storyboard · Claqueta".into(), elementos: propia.elementos.clone() };
    let vieja = FiguraGuardada { id: format!("{PREFIJO_ID}ya-no-existe"), nombre: "Blocs · Vieja".into(), elementos: propia.elementos.clone() };
    let mut lista = vec![propia.clone(), tapa.clone(), vieja];
    instalar_en(&mut lista);
    assert_eq!(lista[0], propia);
    assert_eq!(lista[1], tapa);
    assert!(!lista.iter().any(|f| f.id.ends_with("ya-no-existe")));
    assert_eq!(lista.iter().filter(|f| f.nombre == "Storyboard · Claqueta").count(), 1);
    let n = lista.len();
    instalar_en(&mut lista);
    assert_eq!(lista.len(), n, "instalar dos veces no duplica");
}

#[test]
fn la_categoria_sale_del_nombre() {
    assert_eq!(categoria_de("Diario · Gratitud"), Some(("Diario", "Gratitud")));
    assert_eq!(categoria_de("arbol"), None);
    assert_eq!(categoria_de(" · x"), None);
}
