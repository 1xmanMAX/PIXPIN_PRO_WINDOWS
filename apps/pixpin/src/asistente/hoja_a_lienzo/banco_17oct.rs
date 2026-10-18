//! **Banco de calcar con las hojas del usuario** (17-oct-2026): capturas y
//! laminas reales («calca esto»: comentarios de TikTok, Cuadrilateros II,
//! problemas resueltos, practica dirigida, la circunferencia). Por el mismo
//! camino que la app (`lamina_de_calcar` → `pasar` con texto rico, o el
//! calco de siempre si no es lamina). Por cada hoja deja en `<salida>`:
//!
//! - `<hoja>-zonas.png`: el seccionado, cada zona con su recuadro (color por
//!   tipo) para ver si se parte bien;
//! - `<hoja>-comparar.png`: original | resultado pintado como el lienzo;
//! - y en la salida estandar las zonas, lo hecho de cada una y los tiempos.
//!
//! ```text
//! PIXPIN_CALCAR_DIR=<carpeta con las hojas>   (E:\pixpin-pruebas\calcar-17oct)
//! PIXPIN_CALCAR_SALIDA=<carpeta>              (<dir>\salida)
//! PIXPIN_CALCAR_SOLO=tiktok                   (solo las que contengan eso)
//! PIXPIN_CALCAR_SIN_NUBE=1                    (sin modelo: seccionar y montar)
//! cargo test -p pixpin --bin pixpinmax -- --ignored --nocapture --test-threads=1 banco_17oct
//! ```

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use pixpin_ia::hoja::{Opciones, Tipo};
use pixpin_motor2d::Elemento;

use super::banco::motores;
use super::banco_diagramas::{a_ancho, guardar, lado_a_lado, pintar_gpu};
use super::pruebas::EnEscena;
use super::*;

fn carpeta() -> PathBuf {
    std::env::var_os("PIXPIN_CALCAR_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"E:\pixpin-pruebas\calcar-17oct"))
}

fn salida() -> PathBuf {
    std::env::var_os("PIXPIN_CALCAR_SALIDA").map(PathBuf::from).unwrap_or_else(|| carpeta().join("salida"))
}

/// Las hojas: los PNG de la carpeta (no las comparaciones ni el ideal).
pub(super) fn hojas() -> Vec<(String, PathBuf)> {
    let solo = std::env::var("PIXPIN_CALCAR_SOLO").unwrap_or_default();
    let mut v: Vec<(String, PathBuf)> = std::fs::read_dir(carpeta())
        .map(|d| d.flatten().map(|e| e.path()).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|p: &PathBuf| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")))
        .filter_map(|p| Some((p.file_stem()?.to_string_lossy().to_string(), p)))
        .filter(|(n, _)| !n.contains("ideal") && !n.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .filter(|(n, _)| solo.is_empty() || solo.split(',').any(|s| n.contains(s)))
        .collect();
    v.sort();
    v
}

/// El color del recuadro de cada tipo de zona.
fn color(t: Tipo) -> [u8; 3] {
    match t {
        Tipo::Encabezado => [200, 0, 200],
        Tipo::Parrafo => [0, 90, 230],
        Tipo::Tabla => [0, 170, 170],
        Tipo::Formula => [230, 120, 0],
        Tipo::Foto => [0, 170, 0],
        Tipo::Grafica => [150, 90, 0],
        Tipo::Dibujo | Tipo::Diagrama => [230, 0, 0],
        Tipo::Linea => [120, 120, 120],
        Tipo::Dudosa => [240, 200, 0],
        Tipo::Recuadro => [0, 0, 0],
    }
}

/// La hoja con el recuadro de cada zona (2 px, color por tipo).
pub(super) fn con_recuadros(p: &Pagina, cajas: &[(Caja, [u8; 3])]) -> pixpin_codec::ImagenRgba {
    let mut px = p.rgba.clone();
    let (w, h) = (p.ancho as i64, p.alto as i64);
    let mut poner = |x: i64, y: i64, c: [u8; 3]| {
        if x >= 0 && y >= 0 && x < w && y < h {
            let i = ((y * w + x) * 4) as usize;
            px[i..i + 3].copy_from_slice(&c);
        }
    };
    for (caja, c) in cajas {
        let (x0, y0, x1, y1) = (caja.x as i64, caja.y as i64, caja.x1() as i64, caja.y1() as i64);
        for g in 0..2 {
            for x in x0..=x1 {
                poner(x, y0 + g, *c);
                poner(x, y1 - g, *c);
            }
            for y in y0..=y1 {
                poner(x0 + g, y, *c);
                poner(x1 - g, y, *c);
            }
        }
    }
    pixpin_codec::ImagenRgba { ancho: p.ancho, alto: p.alto, pixeles: px }
}

#[test]
#[ignore = "con las hojas del usuario y, si hay clave, la nube de verdad"]
fn banco_17oct() {
    pixpin_motor2d::texto::instalar_medidor(crate::dibujo::pintar::medir_para_el_motor);
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let sin_nube = std::env::var_os("PIXPIN_CALCAR_SIN_NUBE").is_some();
    let motor = if sin_nube { None } else { motores().into_iter().find(|(n, _)| *n == "claude").map(|(_, m)| m) };
    println!("motor: {}", if motor.is_some() { "claude (Haiku)" } else { "ninguno" });
    let base = salida();
    for (nombre, ruta) in hojas() {
        let p = match leer(&ruta, None) {
            Ok(p) => p,
            Err(e) => {
                println!("{nombre}: no se pudo leer: {e}");
                continue;
            }
        };
        let lamina = es_lamina(&p);
        println!("\n== {nombre} {}x{} {} palabras, lamina={lamina}, leer {} ms", p.ancho, p.alto, p.palabras.len(), p.ms_leer);
        let s = pixpin_ia::hoja::paneles::seccionar(&p.rgba, p.ancho, p.alto, p.palabras.clone(), true);
        for z in &s.zonas {
            let t: String = z.texto().chars().take(60).collect::<String>().replace('\n', " | ");
            println!("  zona {:>2} {:<10} {:.2} [{:.0},{:.0} {:.0}x{:.0}] {t} {:?}", z.id, z.tipo.nombre(), z.confianza, z.caja.x, z.caja.y, z.caja.w, z.caja.h, z.renglones.iter().map(|r| r.color).collect::<Vec<_>>());
            if std::env::var_os("PIXPIN_CALCAR_RENGLONES").is_some() {
                for r in &z.renglones {
                    println!("      renglon [{:.0},{:.0} {:.0}x{:.0}] {}", r.caja.x, r.caja.y, r.caja.w, r.caja.h, r.texto);
                }
            }
        }
        let cajas: Vec<(Caja, [u8; 3])> = s.zonas.iter().map(|z| (z.caja, color(z.tipo))).collect();
        guardar(&con_recuadros(&p, &cajas), &base.join(format!("{nombre}-zonas.png")));
        let mut d = EnEscena::nueva();
        let o = Opciones { ancho: p.ancho as f64, rico: true, ..Default::default() };
        let t0 = std::time::Instant::now();
        let inf = match pasar(&p, motor.clone(), &o, false, 17_000 + p.ancho as u64, &AtomicBool::new(false), &mut d, &mut |_, _| {}) {
            Ok(i) => i,
            Err(e) => {
                println!("{nombre}: fallo: {e}");
                continue;
            }
        };
        for h in &inf.hechos {
            println!("  hecho {:>2} {:<10} {:?} {}", h.zona, h.tipo.nombre(), h.via, h.nota);
        }
        for f in &inf.fallos {
            println!("  FALLO {f}");
        }
        println!(
            "{nombre}: {} zonas, {} peticiones ({} a la vez), 1o visible {:?} ms, total {} ms, rehechas {}",
            inf.seccionado.zonas.len(),
            inf.peticiones,
            inf.a_la_vez,
            inf.ms_primera,
            t0.elapsed().as_millis(),
            inf.rehechas
        );
        let els: Vec<Elemento> = d.escena.visibles().cloned().collect();
        let original = pixpin_codec::ImagenRgba { ancho: p.ancho, alto: p.alto, pixeles: p.rgba.clone() };
        match pintar_gpu(els, &d.imagenes, p.ancho.max(900)) {
            Some(res) => guardar(&lado_a_lado(&a_ancho(&original, res.ancho), &res), &base.join(format!("{nombre}-comparar.png"))),
            None => println!("{nombre}: no se pudo pintar el resultado"),
        }
    }
}

/// La rejilla que mide el lector de tablas en la tabla KPI (oscura, leida en
/// negativo): para ver por que parte en filas una celda de cinco renglones.
#[test]
#[ignore = "lee una foto de E: con el OCR de Windows; a mano"]
fn rejilla_de_la_tabla_kpi() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let im0 = pixpin_codec::imagen::cargar(std::path::Path::new("E:/pixpin-pruebas/calcar-17oct/tabla-kpi.png")).unwrap();
    let c = pixpin_ia::hoja::Caja::nueva(2.0, 350.0, 964.0, 336.0);
    let (mut px, w, h) = pixpin_ia::hoja::imagen::recortar(&im0.pixeles, im0.ancho, im0.alto, &c);
    for p in px.chunks_exact_mut(4) {
        p[0] = 255 - p[0];
        p[1] = 255 - p[1];
        p[2] = 255 - p[2];
    }
    let im = pixpin_codec::ImagenRgba { ancho: w, alto: h, pixeles: px };
    let ruta = std::env::temp_dir().join("pixpin-kpi-negativo.png");
    std::fs::write(&ruta, pixpin_codec::imagen::codificar_png(&im).unwrap()).unwrap();
    let (m, _) = crate::asistente::documentos_acciones::aspecto_de_tabla(&ruta, None).unwrap();
    println!("columnas {:?}\nfilas {:?}\nborde {:?}", m.columnas, m.filas, m.borde);
    for f in &m.celdas {
        println!("  {:?}", f);
    }
}
