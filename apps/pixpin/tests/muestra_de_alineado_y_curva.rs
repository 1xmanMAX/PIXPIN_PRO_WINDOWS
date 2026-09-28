//! **El texto alineado y la flecha curva, pintados y guardados en PNG.**
//!
//! Lo que se tiene que ver no cabe en un `assert_eq`: que un parrafo
//! centrado se lea centrado y que una flecha curva sea una curva que pasa
//! por su vertice y no una quebrada. A maquina se comprueba lo unico que se
//! puede decir sin mirar: que alinear cambia donde cae la tinta y que la
//! curva no pinta lo mismo que la recta.
//!
//! Necesita GPU y sesion de escritorio: `cargo test -p pixpin --test
//! muestra_de_alineado_y_curva -- --ignored --nocapture --test-threads=1`.
//! Deja los PNG en `target/muestras-de-figuras`.

use pixpin_motor2d::TipoPunta;
use pixpin_motor2d::elemento::{ColorRgba, Elemento, Figura};
use pixpin_motor2d::pintado::{self, Orden};
use pixpin_motor2d::texto::AlineacionTexto;
use pixpin_motor2d::vector::Punto2;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_render::{Color, MotorRender};

const ANCHO: u32 = 260;
const ALTO: u32 = 200;

fn a_color(c: ColorRgba) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

/// La misma traduccion de `Orden` a `Pintor` que el editor, recortada a lo
/// que producen un texto y una flecha.
fn pintar(fuera: &FueraDePantalla, motor: &MotorRender, e: &Elemento) -> Vec<u8> {
    motor
        .dibujar(&fuera.destino, |p| {
            p.limpiar(Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            });
            for orden in pintado::ordenes(e) {
                match &orden {
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
                    Orden::Texto {
                        texto,
                        x,
                        y,
                        tam,
                        color,
                        ancho_max,
                        ..
                    } => p.texto_ajustado(texto, *x, *y, *tam, *ancho_max, a_color(*color)),
                    _ => {}
                }
            }
        })
        .expect("pintar");
    fuera.esperar_gpu().expect("esperar");
    fuera.leer_rgba().expect("leer").2
}

/// Donde cae, de media, la tinta en horizontal (0 = izquierda).
fn centro_de_la_tinta(rgba: &[u8]) -> f32 {
    let (mut suma, mut peso) = (0.0f64, 0.0f64);
    for (i, p) in rgba.chunks_exact(4).enumerate() {
        let t = (255 - p[0].min(p[1]).min(p[2])) as f64;
        suma += t * (i as u32 % ANCHO) as f64;
        peso += t;
    }
    (suma / peso.max(1.0)) as f32
}

fn guardar(nombre: &str, rgba: Vec<u8>) {
    let d =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/muestras-de-figuras");
    std::fs::create_dir_all(&d).expect("carpeta");
    let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
        ancho: ANCHO,
        alto: ALTO,
        pixeles: rgba,
    })
    .expect("codificar");
    let ruta = d.join(format!("{nombre}.png"));
    std::fs::write(&ruta, png).expect("guardar");
    println!("{nombre}: {}", ruta.display());
}

#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn el_texto_alineado_y_la_flecha_curva_se_ven_como_dicen() {
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d.d3d()).expect("motor");
    let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), ANCHO, ALTO).expect("fuera de pantalla");
    let tinta = ColorRgba::opaco(0.12, 0.12, 0.14);

    let texto = |a: Option<AlineacionTexto>| {
        let mut e = Elemento {
            figura: Figura::Texto {
                texto: "Un renglon bastante largo\ncorto\nmediano aqui".into(),
                tam: 20.0,
                familia: "Segoe UI".into(),
            },
            x: 10.0,
            y: 40.0,
            ancho: 240.0,
            alto: 90.0,
            trazo: tinta,
            ..Default::default()
        };
        e.extras.alineacion = a;
        e
    };
    let mut centros = Vec::new();
    for (nombre, a) in [
        ("texto-izquierda", Some(AlineacionTexto::Izquierda)),
        ("texto-centro", Some(AlineacionTexto::Centro)),
        ("texto-derecha", Some(AlineacionTexto::Derecha)),
    ] {
        let rgba = pintar(&fuera, &motor, &texto(a));
        centros.push(centro_de_la_tinta(&rgba));
        guardar(nombre, rgba);
    }
    // La tinta se corre a la derecha al centrar y mas al pegar a la derecha.
    assert!(
        centros[0] < centros[1] && centros[1] < centros[2],
        "la alineacion no mueve la tinta: {centros:?}"
    );

    let flecha = |redondo| Elemento {
        figura: Figura::Flecha {
            puntos: vec![
                Punto2::nuevo(20.0, 170.0),
                Punto2::nuevo(110.0, 30.0),
                Punto2::nuevo(240.0, 150.0),
            ],
            punta_inicio: TipoPunta::Ninguna,
            punta_fin: TipoPunta::Flecha,
            codos: false,
        },
        redondo,
        trazo: tinta,
        grosor: 3.0,
        rugosidad: 1.0,
        semilla: 7,
        ..Default::default()
    };
    let recta = pintar(&fuera, &motor, &flecha(false));
    let curva = pintar(&fuera, &motor, &flecha(true));
    assert_ne!(recta, curva, "la curva pinta lo mismo que la quebrada");
    guardar("flecha-afilada", recta);
    guardar("flecha-curva", curva);
}
