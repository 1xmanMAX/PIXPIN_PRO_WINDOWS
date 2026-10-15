//! **Banco de `hoja_a_lienzo` con la nube de verdad.** Hojas sinteticas
//! realistas (dos columnas con raya en medio, titulos, parrafos, tabla,
//! formula, foto, una grafica; y una «escaneada» con papel gris, grano y
//! una tabla sin rayas) pintadas a PNG por el camino del PDF (letra de
//! verdad, que lee el OCR de Windows), y una pagina real de muestra
//! (`Tesis.pdf`, copiada al scratchpad). Por cada motor: tiempo total, a la
//! primera zona en el lienzo, peticiones, y el parecido de cada zona.
//!
//! ```text
//! DEEPSEEK_API_KEY=...  ANTHROPIC_API_KEY=...   (sin clave, ese motor se salta)
//! PIXPIN_IA_IMAGENES=<carpeta>  (ahi deja <motor>\hoja-*-{entrada,resultado}.png)
//! PIXPIN_HOJA_MUESTRA=<un PDF real>  PIXPIN_HOJA_PAGINA=<desde 1>
//! cargo test -p pixpin -- --ignored --nocapture --test-threads=1 hoja_a_lienzo::banco
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use pixpin_ia::hoja::{Opciones, Tipo};
use pixpin_ia::nube::{MotorNube, Proveedor};
use pixpin_motor2d::Elemento;
use pixpin_motor2d::ecuacion::{Estilo, elemento_linea, elemento_texto};
use pixpin_motor2d::elemento::ColorRgba;
use pixpin_motor2d::vector::Punto2;

use super::pruebas::EnEscena;
use super::*;

/// La hoja sintetica mide esto (como una carta) y se pinta al doble.
const W: f32 = 850.0;
const H: f32 = 1100.0;

fn negro() -> ColorRgba {
    ColorRgba::opaco(0.08, 0.08, 0.08)
}

fn estilo(tam: f32) -> Estilo {
    Estilo { color: negro(), tam, opacidad: 1.0, familia: "Work Sans".into() }
}

fn medir(t: &str, tam: f32) -> (f32, f32) {
    (pixpin_motor2d::exportar::ancho_de_texto(t, tam), tam * 1.25)
}

fn texto(t: &str, x: f32, y: f32, tam: f32, negrita: bool) -> Elemento {
    let mut e = elemento_texto(t, x, y, medir(t, tam), tam, false, &estilo(tam));
    e.extras.negrita = negrita;
    e
}

fn centrado(t: &str, cx: f32, y: f32, tam: f32, negrita: bool) -> Elemento {
    let w = medir(t, tam).0;
    texto(t, cx - w / 2.0, y, tam, negrita)
}

/// Un parrafo partido en renglones de `ancho`; devuelve donde acaba.
fn parrafo(v: &mut Vec<Elemento>, t: &str, x: f32, y: f32, ancho: f32, tam: f32) -> f32 {
    let mut renglon = String::new();
    let mut yy = y;
    for p in t.split_whitespace() {
        let prueba = if renglon.is_empty() { p.to_string() } else { format!("{renglon} {p}") };
        if medir(&prueba, tam).0 > ancho && !renglon.is_empty() {
            v.push(texto(&renglon, x, yy, tam, false));
            yy += tam * 1.45;
            renglon = p.to_string();
        } else {
            renglon = prueba;
        }
    }
    if !renglon.is_empty() {
        v.push(texto(&renglon, x, yy, tam, false));
        yy += tam * 1.45;
    }
    yy
}

fn raya(x0: f32, y0: f32, x1: f32, y1: f32, g: f32) -> Elemento {
    elemento_linea(vec![Punto2::nuevo(x0, y0), Punto2::nuevo(x1, y1)], g, &estilo(10.0))
}

fn tabla(v: &mut Vec<Elemento>, filas: &[&[&str]], x: f32, y: f32, tam: f32, rayas: bool) -> f32 {
    let f: Vec<Vec<String>> = filas.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect();
    let els = pixpin_motor2d::tabla_dibujada::elementos_de_tabla(&f, &estilo(tam), Punto2::nuevo(x, y), &medir, true);
    let fin = els.iter().map(|e| e.caja().3).fold(y, f32::max);
    v.extend(els.into_iter().filter(|e| rayas || matches!(e.figura, pixpin_motor2d::Figura::Texto { .. })));
    fin
}

fn formula(v: &mut Vec<Elemento>, latex: &str, cx: f32, y: f32, tam: f32) -> f32 {
    let mut args = serde_json::Map::new();
    args.insert("ecuacion".into(), serde_json::json!(latex));
    let mut e = pixpin_ia::ecuacion::validar(&args, &mut Vec::new()).unwrap();
    e.tam = tam as f64;
    e.color = "#141414".into();
    e.donde = Some((0.0, 0.0));
    let (_, (w, h)) = pixpin_ia::ecuacion::piezas(&e);
    e.donde = Some(((cx - w / 2.0) as f64, y as f64));
    v.extend(pixpin_ia::ecuacion::piezas(&e).0);
    y + h
}

/// Una «foto» de paisaje: cielo, sol, colinas y grano.
fn paisaje(w: u32, h: u32, semilla: u32) -> pixpin_codec::ImagenRgba {
    let mut px = vec![0u8; (w * h * 4) as usize];
    let mut s = semilla;
    for y in 0..h {
        for x in 0..w {
            s = s.wrapping_mul(1103515245).wrapping_add(12345);
            let ruido = ((s >> 16) % 24) as f32 - 12.0;
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            let colina = 0.62 + 0.12 * (fx * 7.0).sin() + 0.05 * (fx * 17.0).cos();
            let sol = ((fx - 0.75).powi(2) + (fy - 0.25).powi(2)).sqrt() < 0.09;
            let (r, g, b) = if sol {
                (250.0, 220.0, 120.0)
            } else if fy > colina {
                (60.0 + 40.0 * fy, 120.0 + 30.0 * fx, 50.0)
            } else {
                (110.0 + 80.0 * fy, 160.0 + 60.0 * fy, 230.0)
            };
            let i = ((y * w + x) * 4) as usize;
            px[i] = (r + ruido).clamp(0.0, 255.0) as u8;
            px[i + 1] = (g + ruido).clamp(0.0, 255.0) as u8;
            px[i + 2] = (b + ruido).clamp(0.0, 255.0) as u8;
            px[i + 3] = 255;
        }
    }
    pixpin_codec::ImagenRgba { ancho: w, alto: h, pixeles: px }
}

/// Los elementos (y fotos) pintados a `ancho` px por el camino del PDF.
fn pintar_hoja(els: Vec<Elemento>, fotos: &HashMap<u64, pixpin_codec::ImagenRgba>, ancho: u32) -> pixpin_codec::ImagenRgba {
    let mut escena = pixpin_motor2d::Escena::nueva();
    for e in els {
        escena.anadir(e);
    }
    let mut hojas = pixpin_motor2d::exportar::hojas(&escena, pixpin_motor2d::exportar::Alcance::Todo, &[], None);
    hojas.truncate(1);
    hojas[0].caja = (0.0, 0.0, W, H);
    let resolver = |id: u64| fotos.get(&id).map(|i| pixpin_pdf::escribir::Pixeles { ancho: i.ancho, alto: i.alto, rgba: i.pixeles.clone() });
    let pdf = pixpin_pdf::escribir::de_hojas_a_medida(&hojas, &[(W, H)], &resolver, None).unwrap();
    let ruta = std::env::temp_dir().join(format!("pixpin-banco-hoja-{}.pdf", std::process::id()));
    std::fs::write(&ruta, pdf).unwrap();
    let img = pixpin_pdf::Documento::abrir(&ruta).unwrap().renderizar(0, ancho).unwrap();
    let _ = std::fs::remove_file(&ruta);
    img
}

const P1: &str = "El terreno destinado a la vivienda se encuentra en una ladera suave con pendiente menor al ocho por ciento. Se realizaron tres calicatas hasta dos metros de profundidad y se tomaron muestras alteradas e inalteradas para los ensayos de laboratorio.";
const P2: &str = "Los ensayos siguieron las normas vigentes. La granulometria por tamizado y los limites de Atterberg permiten clasificar el suelo como una arcilla de baja plasticidad con algo de arena fina.";
const P3: &str = "La capacidad portante admisible se obtiene con la expresion clasica de la resistencia a compresion simple, donde c es la cohesion y phi el angulo de friccion interna del suelo ensayado.";
const P4: &str = "Con un factor de seguridad de tres, la presion admisible resulta adecuada para zapatas aisladas de concreto armado de un metro y medio de lado.";
const P5: &str = "Se recomienda drenar las aguas superficiales y compactar el relleno en capas de veinte centimetros antes de vaciar las cimentaciones.";

/// La hoja A: titulo, dos columnas con raya en medio, tabla, formula y foto.
fn hoja_a(con_rayas: bool, raya_central: bool) -> (Vec<Elemento>, HashMap<u64, pixpin_codec::ImagenRgba>) {
    let mut v = Vec::new();
    let mut fotos = HashMap::new();
    v.push(centrado("Estudio de suelos para la vivienda", W / 2.0, 50.0, 26.0, true));
    v.push(centrado("Informe tecnico - octubre de 2026", W / 2.0, 92.0, 13.0, false));
    v.push(raya(50.0, 125.0, 800.0, 125.0, 1.5));
    if raya_central {
        v.push(raya(425.0, 150.0, 425.0, 1040.0, 1.0));
    }
    // Izquierda.
    let (x, a) = (50.0, 355.0);
    v.push(texto("1. Introduccion", x, 150.0, 16.0, true));
    let y = parrafo(&mut v, P1, x, 185.0, a, 11.0);
    v.push(texto("2. Ensayos", x, y + 18.0, 16.0, true));
    let y = parrafo(&mut v, P2, x, y + 52.0, a, 11.0);
    let filas: &[&[&str]] = &[&["Ensayo", "Valor", "Unidad"], &["Humedad", "18.4", "%"], &["Limite liquido", "34", "%"], &["Cohesion", "0.25", "kg/cm2"], &["Friccion", "22", "grados"]];
    tabla(&mut v, filas, x + 10.0, y + 20.0, 11.0, con_rayas);
    // Derecha.
    let (x, a) = (445.0, 355.0);
    v.push(texto("3. Resultados", x, 150.0, 16.0, true));
    let y = parrafo(&mut v, P3, x, 185.0, a, 11.0);
    let y = formula(&mut v, "q_u = \\frac{2c \\cos\\phi}{1 - \\sin\\phi}", x + a / 2.0, y + 14.0, 20.0);
    let y = parrafo(&mut v, P4, x, y + 18.0, a, 11.0);
    let id = 77u64;
    fotos.insert(id, paisaje(680, 400, 3));
    v.push(crate::imagenes_lienzo::elemento_imagen(id, x, y + 16.0, a, 210.0));
    v.push(texto("Figura 1. Vista del terreno", x, y + 236.0, 10.0, false));
    parrafo(&mut v, P5, x, y + 270.0, a, 11.0);
    (v, fotos)
}

/// La hoja B: una columna, formula con raiz, tabla ancha, una grafica de
/// y = x^2 y una foto.
fn hoja_b() -> (Vec<Elemento>, HashMap<u64, pixpin_codec::ImagenRgba>) {
    let mut v = Vec::new();
    let mut fotos = HashMap::new();
    v.push(texto("Practica 4: ecuacion cuadratica", 70.0, 50.0, 24.0, true));
    let y = parrafo(&mut v, "Las raices de una ecuacion de segundo grado ax2 + bx + c = 0 se calculan con la formula general. El signo del discriminante dice si las raices son reales y distintas, reales e iguales o complejas.", 70.0, 100.0, 710.0, 12.0);
    let y = formula(&mut v, "x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}", W / 2.0, y + 16.0, 24.0);
    let y = parrafo(&mut v, "La tabla resume tres casos resueltos en clase, con su discriminante y el tipo de raices que resultan.", 70.0, y + 22.0, 710.0, 12.0);
    let filas: &[&[&str]] = &[&["a", "b", "c", "Discriminante", "Raices"], &["1", "-3", "2", "1", "reales"], &["1", "2", "1", "0", "doble"], &["2", "1", "3", "-23", "complejas"]];
    let y = tabla(&mut v, filas, 120.0, y + 20.0, 12.0, true);
    // La grafica: ejes y la parabola.
    let (gx, gy, gw, gh) = (90.0, y + 40.0, 300.0, 240.0);
    v.push(raya(gx, gy + gh, gx + gw, gy + gh, 1.5));
    v.push(raya(gx + gw / 2.0, gy, gx + gw / 2.0, gy + gh, 1.5));
    let puntos: Vec<Punto2> = (0..=60)
        .map(|i| {
            let x = -3.0 + i as f32 * 0.1;
            Punto2::nuevo(gx + gw / 2.0 + x * gw / 6.0, gy + gh - x * x * gh / 9.5)
        })
        .collect();
    v.push(elemento_linea(puntos, 2.0, &estilo(10.0)));
    v.push(texto("y = x^2", gx + gw / 2.0 + 70.0, gy + 20.0, 12.0, false));
    let id = 78u64;
    fotos.insert(id, paisaje(600, 480, 9));
    v.push(crate::imagenes_lienzo::elemento_imagen(id, 450.0, gy, 320.0, 240.0));
    parrafo(&mut v, "La grafica muestra la parabola con su vertice en el origen; la foto es la pizarra del aula al final de la clase.", 70.0, gy + gh + 30.0, 710.0, 12.0);
    (v, fotos)
}

/// Papel gris, grano y un poco de desenfoque: como un escaneo.
fn escanear(img: &mut pixpin_codec::ImagenRgba) {
    let (w, h) = (img.ancho as usize, img.alto as usize);
    let orig = img.pixeles.clone();
    let mut s: u32 = 99;
    for y in 0..h {
        for x in 0..w {
            for c in 0..3 {
                let mut suma = 0u32;
                let mut n = 0u32;
                for yy in y.saturating_sub(1)..(y + 2).min(h) {
                    for xx in x.saturating_sub(1)..(x + 2).min(w) {
                        suma += orig[(yy * w + xx) * 4 + c] as u32;
                        n += 1;
                    }
                }
                s = s.wrapping_mul(1103515245).wrapping_add(12345);
                let ruido = ((s >> 16) % 20) as f32 - 10.0;
                let v = (suma / n) as f32 * 0.9 + ruido + 5.0;
                img.pixeles[(y * w + x) * 4 + c] = v.clamp(0.0, 255.0) as u8;
            }
        }
    }
}

fn salida() -> PathBuf {
    std::env::var_os("PIXPIN_IA_IMAGENES").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("banco-imagenes"))
}

fn motores() -> Vec<(&'static str, Arc<dyn pixpin_ia::Motor>)> {
    let mut v: Vec<(&'static str, Arc<dyn pixpin_ia::Motor>)> = Vec::new();
    let clave = |var: &str, p: Proveedor| std::env::var(var).ok().filter(|k| !k.trim().is_empty()).or_else(|| pixpin_ia::nube::claves::leer(p));
    let solo = std::env::var("PIXPIN_HOJA_MOTOR").unwrap_or_default();
    if let Some(k) = clave("DEEPSEEK_API_KEY", Proveedor::DeepSeek).filter(|_| solo.is_empty() || solo.contains("deepseek")) {
        v.push(("deepseek", Arc::new(MotorNube::nuevo(Proveedor::DeepSeek, k).ligero())));
    }
    if let Some(k) = clave("ANTHROPIC_API_KEY", Proveedor::Claude).filter(|_| solo.is_empty() || solo.contains("claude")) {
        v.push(("claude", Arc::new(MotorNube::nuevo(Proveedor::Claude, k).ligero())));
    }
    v
}

#[test]
#[ignore = "banco con la nube de verdad"]
fn banco() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let base = salida();
    // Las entradas: tres sinteticas y, si hay, una real.
    let mut entradas: Vec<(String, PathBuf)> = Vec::new();
    std::fs::create_dir_all(&base).unwrap();
    let hojas: Vec<(&str, (Vec<Elemento>, HashMap<u64, pixpin_codec::ImagenRgba>), bool)> = vec![
        ("hoja-1", hoja_a(true, true), false),
        ("hoja-2", hoja_b(), false),
        ("hoja-3", hoja_a(false, false), true),
    ];
    for (nombre, (els, fotos), escaneo) in hojas {
        let mut img = pintar_hoja(els, &fotos, 1700);
        if escaneo {
            escanear(&mut img);
        }
        let ruta = base.join(format!("{nombre}-sintetica.png"));
        std::fs::write(&ruta, pixpin_codec::imagen::codificar_png(&img).unwrap()).unwrap();
        entradas.push((nombre.to_string(), ruta));
    }
    let muestra = std::env::var_os("PIXPIN_HOJA_MUESTRA").map(PathBuf::from).filter(|r| r.is_file());
    let pagina_muestra = std::env::var("PIXPIN_HOJA_PAGINA").ok().and_then(|p| p.parse::<usize>().ok()).unwrap_or(1).max(1) - 1;
    if let Some(m) = &muestra {
        entradas.push(("hoja-4-real".into(), m.clone()));
    }
    let mut filas = Vec::new();
    for (motor_nombre, motor) in motores() {
        let carpeta = base.join(motor_nombre);
        std::fs::create_dir_all(&carpeta).unwrap();
        for (nombre, ruta) in &entradas {
            if let Ok(h) = std::env::var("PIXPIN_HOJA_SOLO")
                && !nombre.contains(h.as_str())
            {
                continue;
            }
            let pagina = if super::super::ocr::es_pdf(ruta) { Some(pagina_muestra) } else { None };
            let p = match leer(ruta, pagina) {
                Ok(p) => p,
                Err(e) => {
                    println!("{motor_nombre} {nombre}: no se pudo leer: {e}");
                    continue;
                }
            };
            let entrada = pixpin_codec::ImagenRgba { ancho: p.ancho, alto: p.alto, pixeles: p.rgba.clone() };
            std::fs::write(carpeta.join(format!("{nombre}-entrada.png")), pixpin_codec::imagen::codificar_png(&entrada).unwrap()).unwrap();
            let mut d = EnEscena::nueva();
            let o = Opciones::default();
            let r = pasar(&p, Some(motor.clone()), &o, false, 5000 + filas.len() as u64, &AtomicBool::new(false), &mut d, &mut |_, _| {});
            let inf = match r {
                Ok(i) => i,
                Err(e) => {
                    println!("{motor_nombre} {nombre}: fallo: {e}");
                    continue;
                }
            };
            let s = &inf.seccionado;
            let (hw, hh) = pixpin_ia::hoja::Mapa::para(s, &o).hoja(s);
            let pintado = pintar(&inf.hechos, (hw as f32, hh as f32), p.ancho).unwrap();
            let res = pixpin_codec::ImagenRgba { ancho: pintado.1, alto: pintado.2, pixeles: pintado.0 };
            std::fs::write(carpeta.join(format!("{nombre}-resultado.png")), pixpin_codec::imagen::codificar_png(&res).unwrap()).unwrap();
            let pars: Vec<f64> = inf.parecidos.iter().map(|x| x.1).collect();
            let media = pars.iter().sum::<f64>() / pars.len().max(1) as f64;
            let minimo = pars.iter().copied().fold(1.0, f64::min);
            let tipos: Vec<String> = [Tipo::Encabezado, Tipo::Parrafo, Tipo::Tabla, Tipo::Formula, Tipo::Foto, Tipo::Grafica, Tipo::Dibujo, Tipo::Linea, Tipo::Dudosa]
                .iter()
                .filter(|t| s.cuantas(**t) > 0)
                .map(|t| format!("{}{}", s.cuantas(*t), &t.nombre()[..3]))
                .collect();
            for h in &inf.hechos {
                let par = inf.parecidos.iter().find(|x| x.0 == h.zona).map_or(-1.0, |x| x.1);
                let z = &s.zonas[h.zona];
                let t: String = z.texto().chars().take(40).collect::<String>().replace('\n', " | ");
                println!("    {motor_nombre} {nombre} zona {:>2} {:<10} {:.2} {} [{:.0},{:.0} {:.0}x{:.0}] {t}", h.zona, h.tipo.nombre(), par, h.nota, z.caja.x, z.caja.y, z.caja.w, z.caja.h);
            }
            for f in &inf.fallos {
                println!("    FALLO {f}");
            }
            let fila = format!(
                "| {motor_nombre} | {nombre} | {} | {} | {} ms | {} ms | {} ms | {} ms | {} | {} | {:.2} / {:.2} | {} |",
                s.zonas.len(),
                tipos.join(" "),
                p.ms_leer,
                inf.ms_seccionar,
                inf.ms_primera.unwrap_or(0) + p.ms_leer,
                inf.ms_total + p.ms_leer,
                inf.peticiones,
                inf.fallos.len(),
                media,
                minimo,
                inf.rehechas,
            );
            println!("{fila}");
            filas.push(fila);
        }
    }
    println!("\n| motor | hoja | zonas | tipos | leer (OCR) | seccionar | 1a zona | total | peticiones | fallos | parecido medio/min | rehechas |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|");
    for f in &filas {
        println!("{f}");
    }
    println!("imagenes en {}", base.display());
}
