//! D239: lo que se puede medir sin ventana. Lo que necesita la GPU (el
//! fotograma entero en Ligero) lo mide el usuario con `medir_fotogramas`:
//! el agente no usa su PC.
//!
//! Los topes son los de la spec en `--release`. En la suite normal (sin
//! optimizar) se aflojan diez veces: ahi no se mide el presupuesto sino que
//! nadie haya metido algo cuadratico, que se pasaria de diez veces igual.

use std::collections::HashMap;
use std::time::Instant;

use pixpin_motor2d::Camara;
use pixpin_universo::vista::{RejillaAstros, visibles};
use pixpin_universo::{Astro, IdAstro, Universo};

/// Cuanto se afloja un tope sin optimizar.
const HOLGURA_DEBUG: f64 = 10.0;

fn tope(ms: f64) -> f64 {
    if cfg!(debug_assertions) {
        ms * HOLGURA_DEBUG
    } else {
        ms
    }
}

/// 20 galaxias y 5.000 lunas colocadas, 250 por galaxia en una rejilla.
fn sintetico() -> Universo {
    let mut u = Universo::nuevo();
    let proyectos: Vec<String> = (0..20).map(|i| format!("p{i}")).collect();
    pixpin_universo::galaxias::sincronizar(&mut u, &proyectos);
    let galaxias: Vec<(IdAstro, f32, f32, String)> = u
        .astros
        .iter()
        .map(|a| (a.id, a.x, a.y, a.proyecto().unwrap_or_default().to_string()))
        .collect();
    assert_eq!(galaxias.len(), 20, "una galaxia por proyecto");
    for i in 0..5000 {
        let (g, gx, gy, p) = &galaxias[i % 20];
        let id = u.nuevo_id();
        let k = (i / 20) as f32;
        let mut l = Astro::luna(
            id,
            &format!("m:{i}"),
            p,
            gx + (k % 15.0) * 100.0 - 700.0,
            gy + (k / 15.0).floor() * 100.0 - 700.0,
        );
        l.padre = Some(*g);
        u.astros.push(l);
    }
    u.marcar_cambio();
    u
}

#[test]
fn calcular_lo_visible_sobre_el_cosmos_cuesta_menos_de_dos_milisegundos() {
    let u = sintetico();
    let mut r = RejillaAstros::default();
    r.al_dia(&u);
    let camara = Camara {
        x: -40_000.0,
        y: -25_000.0,
        zoom: 0.02,
    };
    let mut memoria = HashMap::new();
    // Una vuelta de calentamiento: la primera llena la memoria de niveles.
    let vistos = visibles(&u, &r, &camara, 1920.0, 1080.0, &mut memoria);
    assert!(!vistos.is_empty(), "desde ahi se ve el cosmos");
    let t = Instant::now();
    for _ in 0..100 {
        let _ = visibles(&u, &r, &camara, 1920.0, 1080.0, &mut memoria);
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0 / 100.0;
    assert!(ms < tope(2.0), "{ms:.3} ms por fotograma");
}

#[test]
fn dentro_de_una_galaxia_llena_lo_visible_tampoco_pasa_de_dos_milisegundos() {
    // El otro caso de la medicion manual: cerca, con cientos de lunas a la
    // vista en vez de veinte discos.
    let u = sintetico();
    let mut r = RejillaAstros::default();
    r.al_dia(&u);
    let g = u.astros.first().expect("hay galaxias");
    let camara = Camara {
        x: g.x - 960.0,
        y: g.y - 540.0,
        zoom: 1.0,
    };
    let mut memoria = HashMap::new();
    let _ = visibles(&u, &r, &camara, 1920.0, 1080.0, &mut memoria);
    let t = Instant::now();
    for _ in 0..100 {
        let _ = visibles(&u, &r, &camara, 1920.0, 1080.0, &mut memoria);
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0 / 100.0;
    assert!(ms < tope(2.0), "{ms:.3} ms por fotograma");
}

#[test]
fn guardar_cinco_mil_lunas_ocupa_menos_de_un_mega_y_tarda_menos_de_cincuenta_ms() {
    let u = sintetico();
    let dir = std::env::temp_dir().join(format!("pixpin-universo-rend-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ruta = dir.join("universo.json");
    let t = Instant::now();
    pixpin_universo::formato::guardar(&ruta, &u, &pixpin_motor2d::Escena::nueva()).unwrap();
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    let bytes = std::fs::metadata(&ruta).unwrap().len();
    let _ = std::fs::remove_dir_all(&dir);
    // El tamano no depende de optimizar: ese tope no se afloja.
    assert!(bytes < 1_000_000, "{bytes} bytes");
    assert!(ms < tope(50.0), "{ms:.1} ms");
}
