//! **Banco de diagramas** (15-oct-2026): la lamina de geometria del
//! usuario («calca esto») y diagramas sinteticos, con Claude y DeepSeek.
//! Guarda SOLO el resultado (>= 1200 px, con una franja: pedido, motor,
//! tiempos) en `<salida>\{claude,deepseek}\<caso>.png`, y para la lamina
//! una comparacion lado a lado original | resultado.
//!
//! ```text
//! PIXPIN_BANCO_DIAGRAMAS=<carpeta>  (por omision scratchpad\banco-diagramas)
//! cargo test -p pixpin -- --ignored --nocapture --test-threads=1 banco_diagramas
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use pixpin_ia::hoja::Opciones;
use pixpin_motor2d::Elemento;

use super::banco::{apilar, franja, motores, pintar_medida};
use super::pruebas::EnEscena;
use super::*;

const SCRATCH: &str = r"C:\Users\MaxBook\AppData\Local\Temp\claude\F--THE-FORGE-PIXPIN-PC-VERSION-MAX\afe461c5-d9fa-4287-b8b5-3ab7961f6cb3\scratchpad";

fn salida() -> PathBuf {
    std::env::var_os("PIXPIN_BANCO_DIAGRAMAS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(SCRATCH).join("banco-geometria"))
}

fn lamina() -> PathBuf {
    std::env::var_os("PIXPIN_LAMINA_RUTA")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(SCRATCH)
                .join("hoja-real")
                .join("circunferencia-geometria.png")
        })
}

pub(super) fn guardar(img: &pixpin_codec::ImagenRgba, ruta: &std::path::Path) {
    if let Some(d) = ruta.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    std::fs::write(ruta, pixpin_codec::imagen::codificar_png(img).unwrap()).unwrap();
}

/// Reescala (vecino mas proximo con media de area) a `ancho`.
pub(super) fn a_ancho(img: &pixpin_codec::ImagenRgba, ancho: u32) -> pixpin_codec::ImagenRgba {
    if img.ancho == ancho {
        return img.clone();
    }
    let alto = ((img.alto as f64 * ancho as f64 / img.ancho as f64).round() as u32).max(1);
    pixpin_codec::redimensionar(img.clone(), ancho, alto).unwrap()
}

/// Original | resultado, a la misma altura.
pub(super) fn lado_a_lado(
    a: &pixpin_codec::ImagenRgba,
    b: &pixpin_codec::ImagenRgba,
) -> pixpin_codec::ImagenRgba {
    let h = a.alto.max(b.alto);
    let b = if b.alto != h {
        pixpin_codec::redimensionar(
            b.clone(),
            ((b.ancho as f64 * h as f64 / b.alto as f64) as u32).max(1),
            h,
        )
        .unwrap()
    } else {
        b.clone()
    };
    let a = if a.alto != h {
        pixpin_codec::redimensionar(
            a.clone(),
            ((a.ancho as f64 * h as f64 / a.alto as f64) as u32).max(1),
            h,
        )
        .unwrap()
    } else {
        a.clone()
    };
    let hueco = 24;
    let w = a.ancho + hueco + b.ancho;
    let mut px = vec![255u8; (w * h * 4) as usize];
    for (img, ox) in [(&a, 0u32), (&b, a.ancho + hueco)] {
        for y in 0..img.alto {
            let d = ((y * w + ox) * 4) as usize;
            let o = (y * img.ancho * 4) as usize;
            px[d..d + (img.ancho * 4) as usize]
                .copy_from_slice(&img.pixeles[o..o + (img.ancho * 4) as usize]);
        }
    }
    // Una raya gris entre los dos.
    for y in 0..h {
        for x in a.ancho + hueco / 2 - 1..a.ancho + hueco / 2 + 1 {
            let i = ((y * w + x) * 4) as usize;
            px[i..i + 3].copy_from_slice(&[180, 180, 180]);
        }
    }
    pixpin_codec::ImagenRgba {
        ancho: w,
        alto: h,
        pixeles: px,
    }
}

/// **Sin red**: seccionar la lamina, vectorizar sus zonas de dibujo y
/// pintar lo que sale (para mirar y ajustar).
#[test]
#[ignore = "mira la lamina del usuario sin red"]
fn lamina_sin_red() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let base = salida().join("sin-red");
    let ruta = lamina();
    let p = leer(&ruta, None).expect("la lamina");
    println!(
        "lamina {}x{}, {} palabras, leer {} ms",
        p.ancho,
        p.alto,
        p.palabras.len(),
        p.ms_leer
    );
    for w in &p.palabras {
        println!(
            "  palabra {:?} [{:.0},{:.0} {:.0}x{:.0}]",
            w.texto, w.caja.x, w.caja.y, w.caja.w, w.caja.h
        );
    }
    let mut s = pixpin_ia::hoja::seccion::seccionar(&p.rgba, p.ancho, p.alto, p.palabras.clone());
    pixpin_ia::hoja::seccion::unir_dibujos(&mut s, &p.rgba);
    for z in &s.zonas {
        let t: String = z
            .texto()
            .chars()
            .take(50)
            .collect::<String>()
            .replace('\n', " | ");
        println!(
            "zona {:>2} {:<10} {:.2} [{:.0},{:.0} {:.0}x{:.0}] {t}",
            z.id,
            z.tipo.nombre(),
            z.confianza,
            z.caja.x,
            z.caja.y,
            z.caja.w,
            z.caja.h
        );
    }
    // La lamina entera como un diagrama (con las palabras del OCR como rotulos).
    let t = std::time::Instant::now();
    let leer_letras = super::lector_de_letras();
    let dir_hoja = base.clone();
    let lector = move |rgba: &[u8], w: u32, h: u32| {
        guardar(
            &pixpin_codec::ImagenRgba {
                ancho: w,
                alto: h,
                pixeles: rgba.to_vec(),
            },
            &dir_hoja.join("hoja-de-letras.png"),
        );
        let v = leer_letras(rgba, w, h);
        for p in &v {
            println!(
                "  ocr letras {:?} [{:.0},{:.0} {:.0}x{:.0}]",
                p.texto, p.caja.x, p.caja.y, p.caja.w, p.caja.h
            );
        }
        v
    };
    let o = pixpin_ia::diagrama::Opciones {
        alto_letra: s.alto_letra,
        ..Default::default()
    };
    let d =
        pixpin_ia::diagrama::vectorizar(&p.rgba, p.ancho, p.alto, &p.palabras, &o, Some(&lector));
    println!(
        "diagrama entero: {} en {} ms (total {} ms)",
        d.resumen(),
        d.ms,
        t.elapsed().as_millis()
    );
    for s in &d.segmentos {
        println!(
            "  seg ({:.0},{:.0})-({:.0},{:.0}) g {:.1} {:?} {:?} {}{}",
            s.a.0, s.a.1, s.b.0, s.b.1, s.grosor, s.color, s.estilo, s.flecha_a, s.flecha_b
        );
    }
    for a in &d.arcos {
        println!(
            "  arco c({:.0},{:.0}) r {:.0} {:.0}+{:.0} g {:.1} {:?}",
            a.c.0,
            a.c.1,
            a.r,
            a.inicio.to_degrees(),
            a.barrido.to_degrees(),
            a.grosor,
            a.color
        );
    }
    for g in &d.glifos {
        println!(
            "  glifo sin leer [{:.0},{:.0} {:.0}x{:.0}]",
            g.caja.x, g.caja.y, g.caja.w, g.caja.h
        );
    }
    for r in d.rotulos.iter().filter(|r| r.letra_suelta) {
        println!("  letra {:?} [{:.0},{:.0}]", r.texto, r.caja.x, r.caja.y);
    }
    let els = pixpin_ia::diagrama::a_elementos(&d, (0.0, 0.0), 1.0, 99, "Work Sans");
    let leido = pixpin_motor2d::excalidraw::leer(&pixpin_ia::lienzo::documento(els)).unwrap();
    let els: Vec<Elemento> = leido.elementos();
    let res = pintar_medida(
        els,
        &HashMap::new(),
        (p.ancho as f32, p.alto as f32),
        Some((0.0, 0.0, p.ancho as f32, p.alto as f32)),
        1400,
    )
    .unwrap();
    guardar(&res, &base.join("lamina-diagrama-entero.png"));
    let original = pixpin_codec::ImagenRgba {
        ancho: p.ancho,
        alto: p.alto,
        pixeles: p.rgba.clone(),
    };
    guardar(
        &lado_a_lado(&a_ancho(&original, 1400), &res),
        &base.join("lamina-comparar-entero.png"),
    );
    // Cada zona de dibujo, de cerca: el recorte | lo vectorizado (al doble).
    for z in s.zonas.iter().filter(|z| {
        matches!(
            z.tipo,
            pixpin_ia::hoja::Tipo::Dibujo | pixpin_ia::hoja::Tipo::Grafica
        )
    }) {
        // Como `montaje::diagrama`: con 12 px de aire.
        let zc = z.caja.crecer(12.0).dentro(p.ancho as f64, p.alto as f64);
        let (rgba, w, h) = pixpin_ia::hoja::imagen::recortar(&p.rgba, p.ancho, p.alto, &zc);
        let palabras: Vec<pixpin_ia::hoja::PalabraHoja> = Vec::new();
        let tapar: Vec<pixpin_ia::hoja::Caja> = p
            .palabras
            .iter()
            .filter(|q| q.caja.comun(&zc) > 0.0)
            .map(|q| {
                pixpin_ia::hoja::Caja::nueva(q.caja.x - zc.x, q.caja.y - zc.y, q.caja.w, q.caja.h)
            })
            .collect();
        let pistas = s
            .zonas
            .iter()
            .flat_map(|z| z.renglones.iter().map(|r| r.texto.clone()))
            .collect();
        let oz = pixpin_ia::diagrama::Opciones {
            tapar,
            pistas,
            ..o.clone()
        };
        let d = pixpin_ia::diagrama::vectorizar(&rgba, w, h, &palabras, &oz, Some(&lector));
        println!("zona {} de cerca: {} en {} ms", z.id, d.resumen(), d.ms);
        if std::env::var_os("PIXPIN_DIAGRAMA_LISTA").is_some() {
            for s in &d.segmentos {
                println!(
                    "  z seg ({:.0},{:.0})-({:.0},{:.0}) g {:.1} {:?} {:?} {}{}",
                    s.a.0, s.a.1, s.b.0, s.b.1, s.grosor, s.color, s.estilo, s.flecha_a, s.flecha_b
                );
            }
            for a in &d.arcos {
                println!(
                    "  z arco c({:.0},{:.0}) r {:.0} {:.0}+{:.0} g {:.1} {:?}",
                    a.c.0,
                    a.c.1,
                    a.r,
                    a.inicio.to_degrees(),
                    a.barrido.to_degrees(),
                    a.grosor,
                    a.color
                );
            }
            for p in &d.puntos {
                println!(
                    "  z punto ({:.0},{:.0}) r {:.1} {:?}",
                    p.c.0, p.c.1, p.r, p.color
                );
            }
            for g in &d.glifos {
                println!(
                    "  z glifo [{:.0},{:.0} {:.0}x{:.0}] g {:.1}",
                    g.caja.x, g.caja.y, g.caja.w, g.caja.h, g.grosor
                );
            }
            for r in &d.regiones {
                println!("  z region {:?} area {:.0}", r.color, r.area);
            }
        }
        let els = pixpin_ia::diagrama::a_elementos(&d, (0.0, 0.0), 1.0, 7, "Work Sans");
        let leido = pixpin_motor2d::excalidraw::leer(&pixpin_ia::lienzo::documento(els)).unwrap();
        if let Some(res) = pintar_medida(
            leido.elementos(),
            &HashMap::new(),
            (w as f32, h as f32),
            Some((0.0, 0.0, w as f32, h as f32)),
            w * 2,
        ) {
            let orig = a_ancho(
                &pixpin_codec::ImagenRgba {
                    ancho: w,
                    alto: h,
                    pixeles: rgba,
                },
                w * 2,
            );
            guardar(
                &lado_a_lado(&orig, &res),
                &base.join(format!("zona-{}-de-cerca.png", z.id)),
            );
        }
    }
    // Y por el camino de la hoja sin modelo.
    let mut dest = EnEscena::nueva();
    let inf = pasar(
        &p,
        None,
        &Opciones {
            rico: true,
            ..Default::default()
        },
        false,
        4242,
        &AtomicBool::new(false),
        &mut dest,
        &mut |_, _| {},
    )
    .unwrap();
    for h in &inf.hechos {
        println!(
            "hecho zona {:>2} {:<10} {}",
            h.zona,
            h.tipo.nombre(),
            h.nota
        );
    }
    let els: Vec<Elemento> = dest.escena.visibles().cloned().collect();
    if let Some(res) = pintar_medida(els, &dest.imagenes, (0.0, 0.0), None, 1400) {
        guardar(&res, &base.join("lamina-hoja-sin-modelo.png"));
    }
}

// ---------------------------------------------------------------------------
// Laminas sinteticas (con letra de verdad, que lee el OCR de Windows)

use pixpin_motor2d::ecuacion::{Estilo, elemento_linea, elemento_texto};
use pixpin_motor2d::elemento::ColorRgba;
use pixpin_motor2d::vector::Punto2;

fn estilo(c: ColorRgba, tam: f32) -> Estilo {
    Estilo {
        color: c,
        tam,
        opacidad: 1.0,
        familia: "Segoe UI".into(),
    }
}

const NEGRO: ColorRgba = ColorRgba::opaco(0.08, 0.08, 0.08);
const AZUL: ColorRgba = ColorRgba::opaco(0.15, 0.25, 0.8);
const ROJO: ColorRgba = ColorRgba::opaco(0.85, 0.15, 0.15);
const VERDE: ColorRgba = ColorRgba::opaco(0.15, 0.6, 0.3);

fn txt(t: &str, x: f32, y: f32, tam: f32, c: ColorRgba) -> Elemento {
    let w = pixpin_motor2d::exportar::ancho_de_texto(t, tam);
    elemento_texto(t, x, y, (w, tam * 1.25), tam, false, &estilo(c, tam))
}

fn raya(a: (f32, f32), b: (f32, f32), g: f32, c: ColorRgba) -> Elemento {
    elemento_linea(
        vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
        g,
        &estilo(c, 10.0),
    )
}

fn flecha(a: (f32, f32), b: (f32, f32), g: f32, c: ColorRgba) -> Elemento {
    let mut e = raya(a, b, g, c);
    e.figura = pixpin_motor2d::Figura::Flecha {
        puntos: vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
        punta_inicio: pixpin_motor2d::formas::TipoPunta::Ninguna,
        punta_fin: pixpin_motor2d::formas::TipoPunta::Triangulo,
        codos: false,
    };
    e
}

/// Un arco de centro `c` y radio `r`, de `desde` barriendo `barrido` (radianes de pantalla).
fn arco(c: (f32, f32), r: f32, desde: f32, barrido: f32, g: f32, col: ColorRgba) -> Elemento {
    let mut e = raya((0.0, 0.0), (1.0, 1.0), g, col);
    e.figura = if barrido >= std::f32::consts::TAU - 1e-3 {
        pixpin_motor2d::Figura::Elipse
    } else {
        pixpin_motor2d::Figura::Arco {
            inicio: desde,
            barrido: Some(barrido),
        }
    };
    e.x = c.0 - r;
    e.y = c.1 - r;
    e.ancho = 2.0 * r;
    e.alto = 2.0 * r;
    e
}

fn punto(p: (f32, f32), c: ColorRgba) -> Elemento {
    let mut e = arco(p, 4.0, 0.0, std::f32::consts::TAU, 1.0, c);
    e.relleno = Some(c);
    e.estilo_relleno = pixpin_motor2d::relleno::EstiloRelleno::Solido;
    e
}

/// Pinta elementos en una hoja de `tam` a `ancho` px (por el PDF).
pub(crate) fn pintar_a(
    els: Vec<Elemento>,
    tam: (f32, f32),
    ancho: u32,
) -> pixpin_codec::ImagenRgba {
    pintar_medida(
        els,
        &HashMap::new(),
        tam,
        Some((0.0, 0.0, tam.0, tam.1)),
        ancho,
    )
    .unwrap()
}

fn pagina_de(img: pixpin_codec::ImagenRgba) -> Pagina {
    let t = std::time::Instant::now();
    let palabras = ocr(&img.pixeles, img.ancho, img.alto).unwrap_or_default();
    Pagina {
        rgba: img.pixeles,
        ancho: img.ancho,
        alto: img.alto,
        palabras,
        ms_leer: t.elapsed().as_millis(),
    }
}

/// El teorema de Tales: una semicircunferencia con su triangulo y un texto.
pub(crate) fn lamina_tales() -> Pagina {
    let v = vec![
        txt(
            "Teorema de Tales: el angulo inscrito en una semicircunferencia es recto",
            30.0,
            20.0,
            18.0,
            NEGRO,
        ),
        raya((100.0, 400.0), (500.0, 400.0), 2.0, NEGRO),
        raya((100.0, 400.0), (380.0, 216.7), 2.0, AZUL),
        raya((380.0, 216.7), (500.0, 400.0), 2.0, AZUL),
        arco(
            (300.0, 400.0),
            200.0,
            std::f32::consts::PI,
            std::f32::consts::PI,
            2.0,
            NEGRO,
        ),
        punto((100.0, 400.0), NEGRO),
        punto((500.0, 400.0), NEGRO),
        punto((380.0, 216.7), NEGRO),
    ];
    pagina_de(pintar_a(v, (640.0, 460.0), 1280))
}

/// Un triangulo con su circunferencia circunscrita, sus puntos y letras.
fn lamina_triangulo() -> Pagina {
    let c = (420.0f32, 330.0f32);
    let r = 200.0f32;
    let ang = [200f32, 330.0, 95.0].map(|g| g.to_radians());
    let v3: Vec<(f32, f32)> = ang
        .iter()
        .map(|a| (c.0 + r * a.cos(), c.1 + r * a.sin()))
        .collect();
    let mut v = vec![
        txt(
            "Triangulo ABC inscrito en una circunferencia de centro O",
            40.0,
            24.0,
            20.0,
            NEGRO,
        ),
        arco(c, r, 0.0, std::f32::consts::TAU, 2.0, NEGRO),
        punto(c, NEGRO),
        txt("O", c.0 + 8.0, c.1 - 2.0, 18.0, NEGRO),
        raya(c, v3[1], 1.5, ROJO),
        txt(
            "R",
            (c.0 + v3[1].0) / 2.0,
            (c.1 + v3[1].1) / 2.0 - 26.0,
            18.0,
            ROJO,
        ),
    ];
    for i in 0..3 {
        v.push(raya(v3[i], v3[(i + 1) % 3], 2.5, AZUL));
        v.push(punto(v3[i], NEGRO));
    }
    v.push(txt("A", v3[0].0 - 30.0, v3[0].1 - 14.0, 20.0, NEGRO));
    v.push(txt("B", v3[1].0 + 10.0, v3[1].1 - 26.0, 20.0, NEGRO));
    v.push(txt("C", v3[2].0 - 6.0, v3[2].1 + 8.0, 20.0, NEGRO));
    v.push(txt("a + b + c = 180\u{b0}", 650.0, 560.0, 18.0, VERDE));
    pagina_de(pintar_a(v, (860.0, 620.0), 1400))
}

/// Una grafica de funciones (ejes con flechas, marcas, una parabola y una
/// recta) escaneada: papel gris, grano, desenfoque.
fn lamina_grafica_escaneada() -> Pagina {
    let (ox, oy, k) = (330.0f32, 380.0f32, 50.0f32);
    let mut v = vec![
        txt(
            "Grafica de f(x) = x^2 - 2 y g(x) = x + 1",
            40.0,
            20.0,
            20.0,
            NEGRO,
        ),
        flecha((60.0, oy), (620.0, oy), 2.0, NEGRO),
        flecha((ox, 560.0), (ox, 70.0), 2.0, NEGRO),
        txt("x", 610.0, oy + 6.0, 18.0, NEGRO),
        txt("y", ox + 10.0, 70.0, 18.0, NEGRO),
    ];
    for i in -5..=5 {
        if i == 0 {
            continue;
        }
        let x = ox + i as f32 * k;
        v.push(raya((x, oy - 5.0), (x, oy + 5.0), 1.5, NEGRO));
        let y = oy - i as f32 * k;
        if (70.0..560.0).contains(&y) {
            v.push(raya((ox - 5.0, y), (ox + 5.0, y), 1.5, NEGRO));
        }
    }
    let parabola: Vec<Punto2> = (0..=80)
        .map(|i| -2.6 + i as f32 * 0.065)
        .map(|x| Punto2::nuevo(ox + x * k, oy - (x * x - 2.0) * k))
        .filter(|p| p.y > 70.0)
        .collect();
    v.push(elemento_linea(parabola, 2.5, &estilo(AZUL, 10.0)));
    v.push(raya(
        (ox - 3.5 * k, oy + 2.5 * k),
        (ox + 3.5 * k, oy - 4.5 * k),
        2.5,
        ROJO,
    ));
    v.push(txt("f(x)", ox + 2.3 * k, oy - 4.6 * k, 18.0, AZUL));
    v.push(txt("g(x)", ox + 3.6 * k, oy - 4.2 * k, 18.0, ROJO));
    let mut img = pintar_a(v, (680.0, 600.0), 1360);
    super::banco::escanear(&mut img);
    pagina_de(img)
}

/// Un plano inclinado: el plano, el bloque, las fuerzas con flecha, el
/// angulo con su arco y su letra, y el suelo rayado.
fn lamina_plano_inclinado() -> Pagina {
    let th = 30f32.to_radians();
    let (ax, ay) = (80.0f32, 480.0f32);
    let base = 520.0f32;
    let (bx, by) = (ax + base, ay);
    let (cx, cy) = (ax + base, ay - base * th.tan());
    let mut v = vec![
        txt("Plano inclinado sin rozamiento", 40.0, 20.0, 22.0, NEGRO),
        raya((ax, ay), (bx, by), 2.5, NEGRO),
        raya((bx, by), (cx, cy), 2.5, NEGRO),
        raya((ax, ay), (cx, cy), 2.5, NEGRO),
        raya((40.0, ay + 2.0), (680.0, ay + 2.0), 1.5, NEGRO),
    ];
    for i in 0..20 {
        let x = 50.0 + i as f32 * 32.0;
        v.push(raya((x, ay + 3.0), (x - 14.0, ay + 18.0), 1.2, NEGRO));
    }
    // El bloque: un cuadrado apoyado en el plano, a media rampa.
    let t = 0.55f32;
    let (px, py) = (ax + (cx - ax) * t, ay + (cy - ay) * t);
    let (ux, uy) = (th.cos(), -th.sin());
    let (nx, ny) = (uy, -ux);
    let l = 70.0f32;
    let esq = [
        (px, py),
        (px + ux * l, py + uy * l),
        (px + ux * l + nx * l, py + uy * l + ny * l),
        (px + nx * l, py + ny * l),
    ];
    for i in 0..4 {
        v.push(raya(esq[i], esq[(i + 1) % 4], 2.5, AZUL));
    }
    let (gx, gy) = (
        px + ux * l / 2.0 + nx * l / 2.0,
        py + uy * l / 2.0 + ny * l / 2.0,
    );
    v.push(punto((gx, gy), NEGRO));
    v.push(flecha((gx, gy), (gx, gy + 150.0), 2.5, ROJO));
    v.push(txt("mg", gx + 8.0, gy + 120.0, 20.0, ROJO));
    v.push(flecha(
        (gx, gy),
        (gx + nx * 130.0, gy + ny * 130.0),
        2.5,
        VERDE,
    ));
    v.push(txt(
        "N",
        gx + nx * 130.0 + 6.0,
        gy + ny * 130.0 - 18.0,
        20.0,
        VERDE,
    ));
    v.push(arco((ax, ay), 70.0, -th, th, 2.0, NEGRO));
    v.push(txt("\u{3b8}", ax + 80.0, ay - 30.0, 20.0, NEGRO));
    pagina_de(pintar_a(v, (720.0, 560.0), 1440))
}

// ---------------------------------------------------------------------------
// El banco con la nube

/// **«Calca esto»** con la lamina del usuario y las sinteticas, como lo hace
/// la app con la nube (`ejecutor::lamina_al_lienzo`): es lamina -> hoja con
/// zonas de diagrama -> lienzo. Solo el resultado, con su franja.
#[test]
#[ignore = "banco con la nube de verdad"]
fn banco_diagramas() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    pixpin_motor2d::texto::instalar_medidor(crate::dibujo::pintar::medir_para_el_motor);
    let base = salida();
    let mut casos: Vec<(String, Pagina)> = Vec::new();
    match leer(&lamina(), None) {
        Ok(p) => casos.push(("lamina-circunferencia-usuario".into(), p)),
        Err(e) => println!("la lamina del usuario no se pudo leer: {e}"),
    }
    casos.push(("triangulo-circunscrito".into(), lamina_triangulo()));
    casos.push(("grafica-escaneada".into(), lamina_grafica_escaneada()));
    casos.push(("plano-inclinado".into(), lamina_plano_inclinado()));
    casos.push(("teorema-de-tales".into(), lamina_tales()));
    for (n, p) in &casos {
        let e = pixpin_codec::ImagenRgba {
            ancho: p.ancho,
            alto: p.alto,
            pixeles: p.rgba.clone(),
        };
        guardar(&e, &base.join("entradas").join(format!("{n}.png")));
    }
    let solo = std::env::var("PIXPIN_DIAGRAMA_SOLO").ok();
    let sin_nube = std::env::var_os("PIXPIN_DIAGRAMA_SIN_NUBE").is_some();
    let mut todos: Vec<(&'static str, Option<std::sync::Arc<dyn pixpin_ia::Motor>>)> = Vec::new();
    if sin_nube {
        todos.push(("sin-nube", None));
    } else {
        let solo_motor = std::env::var("PIXPIN_DIAGRAMA_MOTOR").ok();
        for (n, m) in motores()
            .into_iter()
            .filter(|(n, _)| solo_motor.as_deref().is_none_or(|s| *n == s))
        {
            todos.push((n, Some(m)));
        }
    }
    let mut filas = Vec::new();
    for (motor_nombre, motor) in todos {
        for (nombre, p) in &casos {
            if solo.as_ref().is_some_and(|s| !nombre.contains(s.as_str())) {
                continue;
            }
            let t0 = std::time::Instant::now();
            let lamina_si = es_lamina(p);
            let ms_decidir = t0.elapsed().as_millis();
            let mut d = EnEscena::nueva();
            let o = Opciones {
                ancho: p.ancho as f64,
                rico: true,
                ..Default::default()
            };
            let r = pasar(
                p,
                motor.clone(),
                &o,
                false,
                7000 + filas.len() as u64,
                &AtomicBool::new(false),
                &mut d,
                &mut |_, _| {},
            );
            let inf = match r {
                Ok(i) => i,
                Err(e) => {
                    println!("{motor_nombre} {nombre}: fallo: {e}");
                    continue;
                }
            };
            let primero = inf.ms_primera.unwrap_or(inf.ms_total) + p.ms_leer + ms_decidir;
            let total = inf.ms_total + p.ms_leer + ms_decidir;
            let diagramas = inf
                .hechos
                .iter()
                .filter(|h| h.tipo == pixpin_ia::hoja::Tipo::Diagrama)
                .count();
            for h in &inf.hechos {
                println!(
                    "    {motor_nombre} {nombre} zona {:>2} {:<10} {}",
                    h.zona,
                    h.tipo.nombre(),
                    h.nota
                );
            }
            for f in &inf.fallos {
                println!("    FALLO {f}");
            }
            for (z, v) in &inf.parecidos {
                println!("    parecido zona {z}: {v:.2}");
            }
            let els: Vec<Elemento> = d.escena.visibles().cloned().collect();
            let ancho = (p.ancho * 2).clamp(1200, 2400);
            let Some(res) = pintar_gpu(els, &d.imagenes, ancho) else {
                println!("{motor_nombre} {nombre}: el lienzo quedo vacio");
                continue;
            };
            let fr = franja(
                &[
                    format!(
                        "Pedido: \u{ab}Calca esto\u{bb} (imagen en el lienzo) \u{b7} caso: {nombre} \u{b7} lamina: {}",
                        if lamina_si { "si" } else { "no" }
                    ),
                    format!(
                        "Motor: {motor_nombre} \u{b7} 1\u{ba} visible {:.2} s \u{b7} total {:.1} s \u{b7} {} zonas ({diagramas} diagrama), {} peticiones ({} a la vez), {} fallos \u{b7} build de pruebas (debug)",
                        primero as f32 / 1000.0,
                        total as f32 / 1000.0,
                        inf.seccionado.zonas.len(),
                        inf.peticiones,
                        inf.a_la_vez,
                        inf.fallos.len()
                    ),
                ],
                res.ancho,
            );
            let img = apilar(&fr, &res);
            guardar(&img, &base.join(motor_nombre).join(format!("{nombre}.png")));
            if nombre.contains("usuario") {
                let original = pixpin_codec::ImagenRgba {
                    ancho: p.ancho,
                    alto: p.alto,
                    pixeles: p.rgba.clone(),
                };
                guardar(
                    &lado_a_lado(&a_ancho(&original, res.ancho), &res),
                    &base
                        .join(motor_nombre)
                        .join(format!("{nombre}-comparar.png")),
                );
            }
            let fila = format!(
                "| {motor_nombre} | {nombre} | {lamina_si} | {:.2} s | {:.1} s | {diagramas} | {} | {} |",
                primero as f32 / 1000.0,
                total as f32 / 1000.0,
                inf.peticiones,
                inf.fallos.len()
            );
            println!("{fila}");
            filas.push(fila);
        }
    }
    println!(
        "\n| motor | caso | lamina | 1\u{ba} visible | total | diagramas | peticiones | fallos |\n|---|---|---|---|---|---|---|---|"
    );
    for f in &filas {
        println!("{f}");
    }
}

#[test]
#[ignore = "depurar un caso sintetico"]
fn depurar_plano() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let p = lamina_plano_inclinado();
    let o = pixpin_ia::diagrama::Opciones {
        alto_letra: 24.0,
        ..Default::default()
    };
    let d = pixpin_ia::diagrama::vectorizar(&p.rgba, p.ancho, p.alto, &p.palabras, &o, None);
    println!("{}", d.resumen());
    for s in &d.segmentos {
        println!(
            "  seg ({:.0},{:.0})-({:.0},{:.0}) g {:.1} {:?} {:?}",
            s.a.0, s.a.1, s.b.0, s.b.1, s.grosor, s.color, s.estilo
        );
    }
}

#[test]
#[ignore = "depurar el recuadro de una formula"]
fn depurar_recuadro() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let p = leer(&lamina(), None).unwrap();
    for c in [
        pixpin_ia::hoja::Caja::nueva(92.0, 226.0, 95.0, 63.0),
        pixpin_ia::hoja::Caja::nueva(80.0, 500.0, 120.0, 80.0),
    ] {
        let (rgba, w, h) = pixpin_ia::hoja::imagen::recortar(&p.rgba, p.ancho, p.alto, &c);
        let tapar: Vec<pixpin_ia::hoja::Caja> = p
            .palabras
            .iter()
            .filter(|q| q.caja.comun(&c) > 0.0)
            .map(|q| {
                pixpin_ia::hoja::Caja::nueva(q.caja.x - c.x, q.caja.y - c.y, q.caja.w, q.caja.h)
            })
            .collect();
        println!("tapar {tapar:?}");
        let o = pixpin_ia::diagrama::Opciones {
            alto_letra: 12.0,
            exacta: false,
            tapar,
            ..Default::default()
        };
        let d = pixpin_ia::diagrama::vectorizar(&rgba, w, h, &[], &o, None);
        println!("{}", d.resumen());
        for s in &d.segmentos {
            println!(
                "  seg ({:.1},{:.1})-({:.1},{:.1}) g {:.1} {:?}",
                s.a.0, s.a.1, s.b.0, s.b.1, s.grosor, s.color
            );
        }
        for r in &d.recuadros {
            println!("  rec {r:?}");
        }
    }
}

// ---------------------------------------------------------------------------
// Pagina mixta (15-oct): foto + texto + formula. Lo que no se dibuja (la
// foto) va recortado de la hoja con el recorte del lienzo; lo demas, editable.

/// Una «foto» (un atardecer con montes y grano): muchos tonos y textura,
/// nada que se pueda dibujar con rectas y arcos.
fn pintar_foto(img: &mut pixpin_codec::ImagenRgba, x0: u32, y0: u32, w: u32, h: u32) {
    let mut s: u32 = 987_654;
    for y in y0..(y0 + h).min(img.alto) {
        for x in x0..(x0 + w).min(img.ancho) {
            let (u, v) = ((x - x0) as f32 / w as f32, (y - y0) as f32 / h as f32);
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let grano = ((s >> 16) % 30) as f32 - 15.0;
            let monte = 0.6 + 0.12 * (u * 9.0).sin() + 0.06 * (u * 23.0).cos();
            let (r, g, b) = if v > monte {
                (40.0 + 60.0 * u, 90.0 + 40.0 * (1.0 - v), 50.0)
            } else {
                (250.0 - 60.0 * v, 140.0 + 80.0 * v * u, 90.0 + 120.0 * v)
            };
            let i = ((y * img.ancho + x) * 4) as usize;
            img.pixeles[i] = (r + grano).clamp(0.0, 255.0) as u8;
            img.pixeles[i + 1] = (g + grano).clamp(0.0, 255.0) as u8;
            img.pixeles[i + 2] = (b + grano).clamp(0.0, 255.0) as u8;
            img.pixeles[i + 3] = 255;
        }
    }
}

pub(crate) fn pagina_mixta() -> Pagina {
    let mut v = vec![
        txt("Excursion al valle", 40.0, 24.0, 30.0, NEGRO),
        txt(
            "El grupo salio a las ocho y llego al mirador a mediodia.",
            40.0,
            90.0,
            18.0,
            NEGRO,
        ),
        txt(
            "La pendiente media del camino se calcula asi:",
            40.0,
            120.0,
            18.0,
            NEGRO,
        ),
    ];
    let ec = pixpin_ia::ecuacion::Ecuacion {
        lineas: vec![r"m = \frac{\Delta y}{\Delta x} = \frac{480 - 120}{6}".into()],
        donde: Some((60.0, 160.0)),
        tam: 26.0,
        color: "#1e1e1e".into(),
    };
    v.extend(pixpin_ia::ecuacion::piezas(&ec).0);
    v.push(txt("Foto del mirador:", 40.0, 270.0, 18.0, NEGRO));
    let mut img = pintar_a(v, (700.0, 620.0), 1400);
    // La foto, en pixeles de la hoja (al doble).
    pintar_foto(&mut img, 80, 600, 900, 560);
    pagina_de(img)
}

/// **De punta a punta, con Haiku**: la pagina mixta. La foto sale como el
/// recorte de la hoja (con su `crop`) en su sitio; el texto y la formula,
/// editables. Guarda el resultado y la comparacion en
/// `banco-geometria\recortes\`.
#[test]
#[ignore = "con la nube de verdad"]
fn mixta_con_haiku() {
    pixpin_motor2d::texto::instalar_medidor(crate::dibujo::pintar::medir_para_el_motor);
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let base = salida().join("recortes");
    let p = pagina_mixta();
    let original = pixpin_codec::ImagenRgba {
        ancho: p.ancho,
        alto: p.alto,
        pixeles: p.rgba.clone(),
    };
    guardar(&original, &base.join("mixta-entrada.png"));
    let motor = motores()
        .into_iter()
        .find(|(n, _)| *n == "claude")
        .map(|(_, m)| m)
        .expect("la clave de Claude");
    let mut d = EnEscena::nueva();
    let o = Opciones {
        ancho: p.ancho as f64,
        rico: true,
        ..Default::default()
    };
    let t0 = std::time::Instant::now();
    let inf = pasar(
        &p,
        Some(motor),
        &o,
        false,
        9100,
        &AtomicBool::new(false),
        &mut d,
        &mut |_, _| {},
    )
    .unwrap();
    for h in &inf.hechos {
        println!("  zona {:>2} {:<10} {}", h.zona, h.tipo.nombre(), h.nota);
    }
    println!(
        "{} zonas, {} peticiones ({} a la vez), 1o visible {:?} ms, total {} ms",
        inf.seccionado.zonas.len(),
        inf.peticiones,
        inf.a_la_vez,
        inf.ms_primera,
        t0.elapsed().as_millis()
    );
    let fotos: Vec<&pixpin_ia::hoja::montaje::Hecho> = inf
        .hechos
        .iter()
        .filter(|h| h.elementos.iter().any(|v| v["type"] == "image"))
        .collect();
    assert!(!fotos.is_empty(), "la foto, recortada");
    assert!(
        fotos.iter().all(|h| h
            .elementos
            .iter()
            .filter(|v| v["type"] == "image")
            .all(|v| v["crop"].is_object())),
        "con el recorte del lienzo"
    );
    // El texto y la formula, editables (no imagen).
    let textos = inf
        .hechos
        .iter()
        .filter(|h| {
            matches!(
                h.tipo,
                pixpin_ia::hoja::Tipo::Parrafo | pixpin_ia::hoja::Tipo::Encabezado
            )
        })
        .count();
    assert!(textos >= 2, "los parrafos, como texto");
    // La formula, compuesta: sola, o dentro del bloque de texto rico (`$...$`).
    let compuesta = |h: &&pixpin_ia::hoja::montaje::Hecho| {
        h.nota.starts_with("ecuacion")
            || h.elementos.iter().any(|v| {
                v["textoRico"]
                    .as_str()
                    .is_some_and(|t| t.contains(r"\frac"))
            })
    };
    assert!(
        inf.hechos.iter().any(|h| compuesta(&h)),
        "la formula, compuesta"
    );
    let els: Vec<Elemento> = d.escena.visibles().cloned().collect();
    let res = pintar_gpu(els, &d.imagenes, 1400).expect("el lienzo");
    guardar(&res, &base.join("mixta-resultado.png"));
    guardar(
        &lado_a_lado(&a_ancho(&original, res.ancho), &res),
        &base.join("mixta-comparar.png"),
    );
}

/// **Lo hecho, pintado como en el lienzo** (Direct2D, por el mismo camino
/// que «Exportar a PNG» del editor): el texto rico sale compuesto, con sus
/// colores y su cursiva (el camino del PDF del banco no los pinta).
pub(super) fn pintar_gpu(
    els: Vec<Elemento>,
    fotos: &HashMap<u64, pixpin_codec::ImagenRgba>,
    ancho: u32,
) -> Option<pixpin_codec::ImagenRgba> {
    let mut escena = pixpin_motor2d::Escena::nueva();
    escena.fondo = pixpin_motor2d::elemento::ColorRgba::opaco(1.0, 1.0, 1.0);
    for e in els {
        escena.anadir(e);
    }
    let hojas = pixpin_motor2d::exportar::hojas(
        &escena,
        pixpin_motor2d::exportar::Alcance::Todo,
        &[],
        None,
    );
    let hoja = hojas.first()?;
    let w = (hoja.caja.2 - hoja.caja.0).max(1.0);
    let ver = |id: u64| fotos.get(&id);
    let lienzo = crate::ventana_editor::exportar::Lienzo {
        escena: &escena,
        seleccion: &[],
        papel: None,
        fotos: &ver,
        nombre: "banco".into(),
    };
    crate::ventana_editor::exportar::a_imagen(hoja, ancho as f32 / w, Some(escena.fondo), &lienzo)
        .ok()
}

/// **Los bloques de texto rico**: los renglones de un parrafo y la formula de
/// debajo, en la misma columna, son UN bloque; el titulo (letra mayor) va
/// por su cuenta, y la foto no entra.
#[test]
fn el_parrafo_y_su_formula_son_un_bloque_y_el_titulo_no() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let p = pagina_mixta();
    let s = pixpin_ia::hoja::seccion::seccionar(&p.rgba, p.ancho, p.alto, p.palabras.clone());
    // Lo que vio el OCR de este equipo, para saber por que falla si falla.
    println!("alto de letra {:.1}", s.alto_letra);
    for z in &s.zonas {
        println!(
            "zona {} {:?} {:?}: {:?}",
            z.id,
            z.tipo,
            z.caja,
            z.renglones
                .iter()
                .map(|r| r.texto.clone())
                .collect::<Vec<_>>()
        );
    }
    let bloques = super::bloques_de_texto(&s);
    for b in &bloques {
        println!(
            "bloque {} {:?}: {:?}",
            b.0,
            b.1,
            b.2.renglones
                .iter()
                .map(|r| r.texto.clone())
                .collect::<Vec<_>>()
        );
    }
    let con = |t: &str| {
        bloques
            .iter()
            .find(|b| b.2.renglones.iter().any(|r| r.texto.contains(t)))
    };
    let parrafo = con("pendiente").expect("el bloque del parrafo");
    assert!(
        parrafo
            .2
            .renglones
            .iter()
            .any(|r| r.texto.contains("grupo")),
        "los dos renglones juntos"
    );
    assert!(parrafo.1.len() >= 2, "{:?}", parrafo.1);
    // El titulo no va con el parrafo.
    assert!(
        con("Excursion").is_none_or(|t| !t.2.renglones.iter().any(|r| r.texto.contains("grupo"))),
        "el titulo, solo"
    );
    // La foto no es texto.
    assert!(bloques.iter().all(|b| {
        b.1.iter()
            .all(|&k| s.zonas[k].tipo != pixpin_ia::hoja::Tipo::Foto)
    }));
}
