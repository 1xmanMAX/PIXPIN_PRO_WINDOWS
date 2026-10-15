//! **El rectangulo, el rombo y la elipse a mano, pintados de verdad.**
//!
//! El usuario (2-oct-2026): «en el canvas mejora las figuras que se dibujan;
//! se ven muy angulosas y poco realistas —el circulo, el cuadrado y el
//! rombo— en el modo dibujado a mano». Las ordenes del motor son puntos y no
//! dicen a simple vista si una figura se ve bien, asi que se pintan con el
//! mismo pintor del editor (Direct2D, punta redonda) y se guardan en PNG
//! para mirarlas: una por figura y rugosidad (0 arquitecto, 1 artista, 2
//! dibujante), con cuatro paneles —lisa, redondeada, rayada, redondeada y
//! rayada— por la tuberia entera (`pintado::ordenes`).
//!
//! `cargo test -p pixpin --test muestra_de_figuras_a_mano -- --ignored
//! --nocapture --test-threads=1`. Deja los PNG en `PIXPIN_MUESTRA` si esta
//! puesta, o en `target/muestras-de-figuras`.

use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use pixpin_motor2d::pintado::{self, Orden};
use pixpin_motor2d::relleno::EstiloRelleno;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_render::{Color, MotorRender, Pintor};

const PANEL: (u32, u32) = (230, 180);
const PANELES: u32 = 4;

fn a_color(c: ColorRgba) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

fn carpeta() -> std::path::PathBuf {
    let d = std::env::var_os("PIXPIN_MUESTRA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/muestras-de-figuras")
        });
    std::fs::create_dir_all(&d).expect("crear la carpeta de muestras");
    d
}

fn guardar(nombre: &str, ancho: u32, alto: u32, pixeles: Vec<u8>) {
    let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
        ancho,
        alto,
        pixeles,
    })
    .expect("codificar");
    let ruta = carpeta().join(format!("{nombre}.png"));
    std::fs::write(&ruta, png).expect("guardar");
    println!("{nombre}: {}", ruta.display());
}

/// La figura del panel `i`, con los valores de fabrica del movil (trazo 2,
/// semilla fija) y relleno rosa cuando toca.
pub fn figura(figura: Figura, i: u32, redondo: bool, rayado: bool, rugosidad: f32) -> Elemento {
    let (w, h) = match figura {
        Figura::Elipse => (150.0, 130.0),
        Figura::Rombo => (170.0, 140.0),
        _ => (180.0, 120.0),
    };
    Elemento {
        id: 1,
        figura,
        x: i as f32 * PANEL.0 as f32 + (PANEL.0 as f32 - w) / 2.0,
        y: (PANEL.1 as f32 - h) / 2.0,
        ancho: w,
        alto: h,
        trazo: ColorRgba::opaco(0.12, 0.12, 0.14),
        relleno: rayado.then(|| ColorRgba::opaco(0.93, 0.36, 0.42)),
        estilo_relleno: EstiloRelleno::Rayado,
        grosor: 2.0,
        estilo: EstiloTrazo::Solido,
        rugosidad,
        redondo,
        semilla: 1_234_567,
        ..Default::default()
    }
}

/// Los cuatro paneles de una figura: lisa, redondeada, rayada y las dos.
pub fn paneles(f: Figura, rugosidad: f32) -> Vec<Elemento> {
    [(false, false), (true, false), (false, true), (true, true)]
        .into_iter()
        .enumerate()
        .map(|(i, (redondo, rayado))| figura(f.clone(), i as u32, redondo, rayado, rugosidad))
        .collect()
}

pub fn pintar_ordenes(p: &Pintor, ordenes: &[Orden]) {
    for orden in ordenes {
        match orden {
            Orden::Relleno { puntos, color } | Orden::Poligono { puntos, color } => {
                let v: Vec<(f32, f32)> = puntos.iter().map(|q| (q.x, q.y)).collect();
                p.poligono(&v, a_color(*color));
            }
            Orden::Polilinea {
                puntos,
                color,
                grosor,
                ..
            } => {
                let v: Vec<(f32, f32)> = puntos.iter().map(|q| (q.x, q.y)).collect();
                p.polilinea(&v, *grosor, a_color(*color));
            }
            _ => {}
        }
    }
}

pub fn pintar_hoja(nombre: &str, ordenes: &[Orden]) -> Vec<u8> {
    let (w, h) = (PANEL.0 * PANELES, PANEL.1);
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d.d3d()).expect("motor");
    let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), w, h).expect("fuera de pantalla");
    motor
        .dibujar(&fuera.destino, |p| {
            p.limpiar(Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            });
            pintar_ordenes(p, ordenes);
        })
        .expect("pintar");
    fuera.esperar_gpu().expect("esperar");
    let rgba = fuera.leer_rgba().expect("leer").2;
    guardar(nombre, w, h, rgba.clone());
    rgba
}

pub const FIGURAS: [(&str, Figura); 3] = [
    ("rect", Figura::Rectangulo),
    ("rombo", Figura::Rombo),
    ("elipse", Figura::Elipse),
];

#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn las_figuras_a_mano_pintadas_por_la_tuberia_entera() {
    for (nombre, f) in FIGURAS {
        for rugosidad in [0.0f32, 1.0, 2.0] {
            let ordenes: Vec<Orden> = paneles(f.clone(), rugosidad)
                .iter()
                .flat_map(pintado::ordenes)
                .collect();
            let rgba = pintar_hoja(&format!("despues-{nombre}-r{rugosidad}"), &ordenes);
            // Algo se pinto en cada panel: ni uno en blanco.
            for i in 0..PANELES {
                let x0 = i * PANEL.0;
                let tinta = (0..PANEL.1)
                    .flat_map(|y| (x0..x0 + PANEL.0).map(move |x| (x, y)))
                    .filter(|&(x, y)| rgba[((y * PANEL.0 * PANELES + x) * 4) as usize] < 128)
                    .count();
                assert!(tinta > 200, "{nombre} r{rugosidad} panel {i}: {tinta}");
            }
        }
    }
}
