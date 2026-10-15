//! **Banco de `anotar_inteligente` con el modelo de verdad** (15-oct-2026).
//!
//! Corre sobre COPIAS de las muestras (`E:\pixpin-demo`, `E:\pixpin-demo-om`,
//! `E:\pixpin-cad\muestras`) en el scratchpad, con la clave guardada en
//! Ajustes leida EN MEMORIA por el codigo de la app (nunca se imprime), y
//! pinta cada documento CON lo anotado a PNG por Direct2D, con la misma
//! tinta que los visores (`Tinta::pintar_capa`):
//! `scratchpad\banco-anotar\<motor>\<caso>.png`.
//!
//! ```text
//! PIXPIN_IA_MOTOR=deepseek|claude   (por omision: los de la nube con clave guardada)
//! PIXPIN_IA_SOLO=<texto>                   (solo los casos que lo contienen)
//! cargo test -p pixpin -- --ignored --nocapture --test-threads=1 banco_de_anotar
//! ```

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use pixpin_ia::nube::{MotorNube, Proveedor};
use pixpin_motor2d::{ColorRgba, Elemento, Figura};
use pixpin_render::{Color, RectF};

use super::*;

const SCRATCHPAD: &str = r"C:\Users\MaxBook\AppData\Local\Temp\claude\F--THE-FORGE-PIXPIN-PC-VERSION-MAX\afe461c5-d9fa-4287-b8b5-3ab7961f6cb3\scratchpad";

fn base() -> PathBuf {
    std::env::var("PIXPIN_IA_SCRATCH").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(SCRATCHPAD)).join("banco-anotar")
}

/// Una copia de la muestra (nunca se abre la original), limpia de capas.
fn muestra(nombre: &str, caso: &str) -> PathBuf {
    let dir = base().join("muestras").join(caso);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for origen in [r"E:\pixpin-demo", r"E:\pixpin-demo-om", r"E:\pixpin-cad\muestras"] {
        let o = Path::new(origen).join(nombre);
        if o.is_file() {
            let r = dir.join(nombre);
            std::fs::copy(&o, &r).unwrap();
            return r;
        }
    }
    panic!("falta la muestra {nombre}");
}

// ---------------------------------------------------------------------------
// Pintar lo anotado a PNG

/// Lo de debajo: una imagen puesta en `caja` (unidades de la capa).
struct Fondo {
    img: pixpin_codec::ImagenRgba,
    caja: [f32; 4],
}

fn caja_de(els: &[Elemento]) -> Option<[f32; 4]> {
    pixpin_ia::anotacion::union(els.iter().map(|e| {
        let (x0, y0, x1, y1) = e.caja();
        [x0, y0, x1, y1]
    }))
}

/// **El documento con lo anotado**, pintado por Direct2D con la tinta de
/// los visores, a `salida` (unos 1600 px de ancho).
fn pintar(fondo: &Fondo, els: &[Elemento], salida: &Path) {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let todo = pixpin_ia::anotacion::union([fondo.caja].into_iter().chain(caja_de(els))).unwrap();
    let aire = (todo[2] - todo[0]) * 0.02;
    let todo = [todo[0] - aire, todo[1] - aire, todo[2] + aire, todo[3] + aire];
    let s = (1600.0 / (todo[2] - todo[0])).min(2600.0 / (todo[3] - todo[1]));
    let (w, h) = (((todo[2] - todo[0]) * s).ceil() as u32, ((todo[3] - todo[1]) * s).ceil() as u32);
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU");
    let motor = pixpin_render::MotorRender::nuevo(d.d3d()).expect("motor");
    let fuera = pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(&motor, d.d3d(), w, h).expect("superficie");
    let b = motor.bitmap_desde_pixeles(fondo.img.ancho, fondo.img.alto, &fondo.img.pixeles).expect("bitmap");
    let mut capa = crate::lector_tinta::Capa::default();
    for e in els {
        capa.escena.anadir(e.clone());
    }
    let mut tinta = crate::lector_tinta::Tinta::nueva();
    motor
        .dibujar(&fuera.destino, |p| {
            p.limpiar(Color { r: 0.93, g: 0.93, b: 0.94, a: 1.0 });
            let a = |x: f32, y: f32| ((x - todo[0]) * s, (y - todo[1]) * s);
            let (x0, y0) = a(fondo.caja[0], fondo.caja[1]);
            let (x1, y1) = a(fondo.caja[2], fondo.caja[3]);
            p.rellenar(RectF { x: x0, y: y0, ancho: x1 - x0, alto: y1 - y0 }, Color::BLANCO);
            p.bitmap(&b, RectF { x: x0, y: y0, ancho: x1 - x0, alto: y1 - y0 }, None, false);
            p.poner_vista((0.0, 0.0), s, (-todo[0] * s, -todo[1] * s));
            tinta.pintar_capa(p, 0, &capa, (todo[0], todo[1], todo[2], todo[3]), s, crate::lector_tinta::papel(Color::BLANCO), false);
            p.desplazar(0.0, 0.0);
        })
        .expect("pintar");
    fuera.esperar_gpu().unwrap();
    let (ancho, alto, pixeles) = fuera.leer_rgba().unwrap();
    std::fs::create_dir_all(salida.parent().unwrap()).unwrap();
    std::fs::write(salida, pixpin_codec::imagen::codificar_png(&pixpin_codec::ImagenRgba { ancho, alto, pixeles }).unwrap()).unwrap();
}

/// Lo del turno en una capa tocada.
fn del_turno(capa: &Path, grupo: &str, foto: bool) -> Vec<Elemento> {
    let escena = if foto { pixpin_motor2d::cargar(capa).unwrap() } else { crate::lector_tinta::Capa::leer(capa).escena };
    escena.visibles().filter(|e| e.grupos.iter().any(|g| g == grupo)).cloned().collect()
}

/// La hoja `i` de un PDF, a la capa de 1400 de ancho.
fn fondo_pdf(pdf: &Path, i: usize) -> Fondo {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let doc = pixpin_pdf::Documento::abrir(pdf).unwrap();
    let img = doc.renderizar(i as u32, 1400).unwrap();
    let alto = 1400.0 * img.alto as f32 / img.ancho as f32;
    Fondo { img, caja: [0.0, 0.0, 1400.0, alto] }
}

fn fondo_foto(foto: &Path) -> Fondo {
    let img = pixpin_codec::imagen::cargar(foto).unwrap();
    let caja = [0.0, 0.0, img.ancho as f32, img.alto as f32];
    Fondo { img, caja }
}

/// El plano entero (en sus unidades de capa) pintado por el pintor de
/// pruebas del visor.
fn fondo_plano(raiz: &Path, plano: &Path) -> Fondo {
    use crate::plano_cad::anotar;
    let m = crate::plano_cad::modelo(raiz, plano).unwrap();
    let rutas = anotar::rutas_de_la_capa(raiz, plano);
    let mut encaje = None;
    anotar::anadir_figuras(&rutas, &m, |e| {
        encaje = Some(*e);
        Vec::new()
    })
    .unwrap();
    let encaje = encaje.unwrap();
    let c = m.caja_util();
    let (ancho, alto) = ((c[2] - c[0]) as f64, (c[3] - c[1]) as f64);
    let w = 1600usize;
    let h = ((w as f64) * alto / ancho).round().max(1.0) as usize;
    let centro = [m.origen[0] + (c[0] + c[2]) as f64 / 2.0, m.origen[1] + (c[1] + c[3]) as f64 / 2.0];
    let mut l = pixpin_cad::pruebas_png::Lienzo::nuevo(&m, centro, ancho, w, h);
    l.pintar(&m);
    let px: Vec<u8> = l.px.iter().flatten().copied().collect();
    let a = encaje.a_capa([c[0] as f64, c[1] as f64 + alto]);
    let b = encaje.a_capa([c[2] as f64, c[1] as f64]);
    Fondo { img: pixpin_codec::ImagenRgba { ancho: w as u32, alto: h as u32, pixeles: px }, caja: [a[0] as f32, a[1] as f32, b[0] as f32, b[1] as f32] }
}

/// La hoja de calculo pintada por su pintor, a la rejilla de la capa.
fn fondo_libro(xlsx: &Path, ancho_capa: f32, alto_capa: f32) -> Fondo {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let libro = pixpin_hojas::Libro::abrir(xlsx, pixpin_hojas::Regional::es_pe()).unwrap();
    let mut hoja = libro.abrir_hoja(0);
    hoja.cargar_todo();
    let rejilla = rejilla_de_la_capa(&libro, &hoja);
    let k = 1.5f32;
    let (w, h) = ((ancho_capa * k).ceil() as u32, (alto_capa * k).ceil() as u32);
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU");
    let motor = pixpin_render::MotorRender::nuevo(d.d3d()).expect("motor");
    let fuera = pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(&motor, d.d3d(), w, h).expect("superficie");
    let mut cache = pixpin_render::hoja_office::CacheHoja::nueva();
    let v = pixpin_hojas::Vista { ancho: w as f64, alto: h as f64, escala: k as f64, x: 0.0, y: 0.0 };
    let op = pixpin_render::hoja_office::OpcionesHoja { encabezados: false, seleccion: None, activa: None, bajo_raton: None, oscuro: false };
    motor
        .dibujar(&fuera.destino, |p| {
            p.limpiar(Color::BLANCO);
            pixpin_render::hoja_office::pintar_hoja(p, &mut cache, &libro, &hoja, &rejilla, &v, RectF { x: 0.0, y: 0.0, ancho: w as f32, alto: h as f32 }, &op);
        })
        .unwrap();
    fuera.esperar_gpu().unwrap();
    let (ancho, alto, pixeles) = fuera.leer_rgba().unwrap();
    Fondo { img: pixpin_codec::ImagenRgba { ancho, alto, pixeles }, caja: [0.0, 0.0, ancho as f32 / k, alto as f32 / k] }
}

// ---------------------------------------------------------------------------
// Muestras que se hacen aqui

fn texto(t: &str, x: f32, y: f32, tam: f32) -> Elemento {
    Elemento {
        figura: Figura::Texto { texto: t.into(), tam, familia: "Segoe UI".into() },
        x,
        y,
        ancho: pixpin_motor2d::exportar::ancho_de_texto(t, tam),
        alto: tam * 1.33,
        trazo: ColorRgba::opaco(0.08, 0.08, 0.1),
        grosor: 1.0,
        rugosidad: 0.0,
        ..Default::default()
    }
}

/// Una hoja A4 con estos renglones (titulo en grande) a PDF.
fn pdf_de(ruta: &Path, renglones: &[&str]) {
    let mut escena = pixpin_motor2d::Escena::nueva();
    for (i, t) in renglones.iter().enumerate() {
        let tam = if i == 0 { 16.0 } else { 11.0 };
        escena.anadir(texto(t, 72.0, 80.0 + i as f32 * 21.0, tam));
    }
    let mut hojas = pixpin_motor2d::exportar::hojas(&escena, pixpin_motor2d::exportar::Alcance::Todo, &[], None);
    hojas.truncate(1);
    hojas[0].caja = (0.0, 0.0, 595.0, 842.0);
    let bytes = pixpin_pdf::escribir::de_hojas_a_medida(&hojas, &[(595.0, 842.0)], &|_| None, super::super::hoja_a_lienzo::letra_segoe()).unwrap();
    std::fs::write(ruta, bytes).unwrap();
}

/// Una «foto» de una hoja: gris y con grano (la lee el OCR de Windows).
fn foto_de(pdf: &Path, salida: &Path) {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let mut img = pixpin_pdf::Documento::abrir(pdf).unwrap().renderizar(0, 1500).unwrap();
    let mut s = 7u32;
    for px in img.pixeles.chunks_exact_mut(4) {
        s = s.wrapping_mul(1103515245).wrapping_add(12345);
        let ruido = ((s >> 16) % 30) as i32 - 15;
        for c in px.iter_mut().take(3) {
            *c = ((*c as i32) * 88 / 100 + 18 + ruido).clamp(0, 255) as u8;
        }
    }
    std::fs::write(salida, pixpin_codec::imagen::codificar_png(&img).unwrap()).unwrap();
}

const PASOS: &[&str] = &[
    "Como medir la humedad de una muestra de suelo",
    "Primero se pesa la tara vacia y se anota su peso en gramos.",
    "Despues se coloca la muestra humeda en la tara y se pesa el conjunto.",
    "La tara con la muestra se lleva al horno a 110 grados durante 24 horas.",
    "Al sacarla se deja enfriar en el desecador y se vuelve a pesar.",
    "Con los tres pesos se calcula la humedad: w = (Wh - Ws) / (Ws - Wt).",
    "Por ultimo se repite con otra muestra y se promedian los resultados.",
];

const CON_ERRORES: &[&str] = &[
    "Informe de laboratorio: densidad del agua",
    "El agua pura hierve a 50 grados Celsius a nivel del mar.",
    "Se midio una masa de 250 g en un volumen de 250 ml.",
    "Por lo tanto la densidad es 250 / 250 = 2 g/ml.",
    "La densidad del agua es mayor a 4 grados porque el hielo se hunde.",
    "El volumen se midio con una probeta graduada de 500 ml.",
    "Conclusion: el resultado coincide con el valor teorico.",
];

// ---------------------------------------------------------------------------
// Los casos

struct Caso {
    nombre: &'static str,
    /// La muestra (ya copiada o hecha) y los argumentos.
    preparar: fn(&str) -> (PathBuf, Value),
}

/// La primera hoja desde `desde` con mucho texto.
fn hoja_con_texto(pdf: &Path, desde: usize) -> usize {
    let t = pixpin_pdf::texto::de_fichero(pdf).unwrap_or_default();
    (desde..t.len())
        .find(|&i| t[i].texto.chars().filter(|c| c.is_alphabetic()).count() > 1800)
        .unwrap_or(desde.min(t.len().saturating_sub(1)))
}

fn carpeta_del_caso(c: &str) -> PathBuf {
    let dir = base().join("muestras").join(c);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn casos() -> Vec<Caso> {
    vec![
        Caso {
            nombre: "pdf-lo-importante",
            preparar: |c| {
                let r = muestra("Tesis.pdf", c);
                let p = hoja_con_texto(&r, 14);
                (r, json!({"pedido": "señálame lo importante de esta página", "pagina": p + 1}))
            },
        },
        Caso {
            nombre: "pdf-resume-secciones",
            preparar: |c| {
                let r = muestra("Tesis.pdf", c);
                let p = hoja_con_texto(&r, 20);
                (r, json!({"pedido": "resume cada sección al margen", "modo": "resumen", "pagina": p + 1}))
            },
        },
        Caso {
            nombre: "pdf-preguntas-repaso",
            preparar: |c| {
                let r = muestra("Tesis.pdf", c);
                let p = hoja_con_texto(&r, 26);
                (r, json!({"pedido": "hazme preguntas de repaso al margen", "modo": "preguntas", "pagina": p + 1}))
            },
        },
        Caso {
            nombre: "pdf-numera-pasos",
            preparar: |c| {
                let r = carpeta_del_caso(c).join("procedimiento.pdf");
                pdf_de(&r, PASOS);
                (r, json!({"pedido": "numera los pasos del procedimiento", "modo": "pasos"}))
            },
        },
        Caso {
            nombre: "word-explica-formulas",
            preparar: |c| (muestra("Losa plana FEA.docx", c), json!({"pedido": "explícame las fórmulas y lo técnico al costado", "modo": "explicar", "pagina": 3})),
        },
        Caso {
            nombre: "pptx-glosario",
            preparar: |c| (muestra("Charla.pptx", c), json!({"pedido": "haz un glosario de los términos técnicos", "modo": "glosario", "pagina": 2})),
        },
        Caso {
            nombre: "escaneado-marca-errores",
            preparar: |c| {
                let dir = carpeta_del_caso(c);
                let pdf = dir.join("informe.pdf");
                pdf_de(&pdf, CON_ERRORES);
                let foto = dir.join("informe-foto.png");
                foto_de(&pdf, &foto);
                let _ = std::fs::remove_file(&pdf);
                (foto, json!({"pedido": "marca los errores y pon la corrección", "modo": "errores"}))
            },
        },
        Caso {
            nombre: "plano-lo-importante",
            preparar: |c| (muestra("senal-vertical-tahuantinsuyo.dwg", c), json!({"pedido": "señala lo importante del plano y explícalo"})),
        },
        Caso {
            nombre: "excel-explica",
            preparar: |c| (muestra("Perfiles de acero.xlsx", c), json!({"pedido": "explícame qué significa cada columna y señala lo importante", "celdas": "A1:L30"})),
        },
    ]
}

// ---------------------------------------------------------------------------
// Los motores

fn motores() -> Vec<(String, Arc<dyn Motor>)> {
    let eleccion = std::env::var("PIXPIN_IA_MOTOR").ok();
    let mut v: Vec<(String, Arc<dyn Motor>)> = Vec::new();
    for p in Proveedor::TODOS {
        if eleccion.as_deref().is_some_and(|e| e != p.id()) {
            continue;
        }
        // La clave de Ajustes, en memoria (nunca se imprime).
        match pixpin_ia::nube::claves::leer(p) {
            Some(clave) => v.push((p.id().to_string(), Arc::new(MotorNube::nuevo(p, clave).ligero()))),
            None => println!("BANCO {}: sin clave guardada, se salta", p.id()),
        }
    }
    v
}

/// **El banco**: cada caso con cada motor, a PNG, con sus tiempos.
#[test]
#[ignore = "llama a la nube con la clave de Ajustes y pinta PNG"]
fn banco_de_anotar() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let solo = std::env::var("PIXPIN_IA_SOLO").ok();
    for (nombre_motor, motor) in motores() {
        let salida = base().join(&nombre_motor);
        std::fs::create_dir_all(&salida).unwrap();
        for caso in casos() {
            if solo.as_deref().is_some_and(|s| !caso.nombre.contains(s)) {
                continue;
            }
            let (ruta, args) = (caso.preparar)(caso.nombre);
            let raiz = base().join("raiz").join(caso.nombre);
            std::fs::create_dir_all(&raiz).unwrap();
            let parar = AtomicBool::new(false);
            let t = Instant::now();
            let mut primera_ms = None;
            let r = anotar_en(&raiz, &ruta, &args, motor.as_ref(), &parar, &mut |_, primera| {
                if primera {
                    primera_ms = Some(t.elapsed().as_millis());
                }
            });
            let h = match r {
                Ok(h) => h,
                Err(e) => {
                    println!("BANCO {nombre_motor} {}: ERROR {e}", caso.nombre);
                    continue;
                }
            };
            println!(
                "BANCO {nombre_motor} {}: {} | leer {} ms, primera a {:?} ms del primer trozo ({:?} ms del pedido), poner max {} ms (anclar max {} ms), total {} ms, ocr {}",
                caso.nombre, h.resumen, h.ms_leer, h.ms_a_la_primera, primera_ms, h.ms_poner_max, h.ms_anclar_max, h.ms_total, h.con_ocr
            );
            let formato = formato_de(&ruta).unwrap();
            for (pagina, capa) in &h.capas {
                let mut els = del_turno(capa, &h.grupo, formato == Formato::Foto);
                let fondo = match formato {
                    Formato::Pdf => fondo_pdf(&ruta, *pagina),
                    Formato::Office => {
                        // La hoja tal como la maqueta el lector: cada palabra
                        // en su caja (lo que el anotador uso para anclar),
                        // en negro sobre papel blanco, y lo anotado encima.
                        let l = leer(&raiz, &ruta, &json!({"pagina": pagina + 1}), &AtomicBool::new(false)).unwrap();
                        let p = &l.paginas[0];
                        let mut todo: Vec<Elemento> = p
                            .renglones
                            .iter()
                            .flat_map(|r| r.palabras.iter())
                            .map(|w| {
                                let tam = (w.caja[3] - w.caja[1]) * 0.78;
                                let mut e = texto(&w.texto, w.caja[0], w.caja[1], tam);
                                e.figura = Figura::Texto { texto: w.texto.clone(), tam, familia: "Nunito".into() };
                                e
                            })
                            .collect();
                        todo.extend(els.drain(..));
                        els = todo;
                        Fondo { img: pixpin_codec::ImagenRgba { ancho: 1, alto: 1, pixeles: vec![255; 4] }, caja: p.caja }
                    }
                    Formato::Foto => fondo_foto(&ruta),
                    Formato::Plano => fondo_plano(&raiz, &ruta),
                    Formato::Libro => {
                        let c = caja_de(&els).unwrap_or([0.0, 0.0, 800.0, 600.0]);
                        fondo_libro(&ruta, c[2].max(400.0), c[3].max(300.0))
                    }
                };
                let png = if h.capas.len() > 1 {
                    salida.join(format!("{}-p{}.png", caso.nombre, pagina + 1))
                } else {
                    salida.join(format!("{}.png", caso.nombre))
                };
                pintar(&fondo, &els, &png);
                println!("PNG {}", png.display());
            }
        }
    }
}
