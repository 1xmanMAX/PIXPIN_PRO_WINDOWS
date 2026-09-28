//! **Las esquinas y el redondeo, pintados de verdad y guardados en PNG.**
//!
//! Dos fallos que el usuario vio en pantalla y que ninguna prueba sin GPU
//! podia ver, porque las ordenes del motor estaban bien:
//!
//! 1. **La muesca de la esquina.** Un rectangulo son cuatro lados sueltos; el
//!    pintor los trazaba con el extremo plano de Direct2D, asi que cada lado
//!    acababa justo en el vertice y en la esquina de fuera faltaba un
//!    cuadradito de medio grosor de lado. Se leia como «lineas juntas y no un
//!    cuadrado». Excalidraw pide a su canvas extremo y union redondos.
//! 2. **El redondeo que no se veia y el relleno que se salia.** El borde
//!    redondo solo se pintaba con rugosidad 0, y el relleno iba siempre por
//!    la caja recta: con el trazo «a mano» de fabrica el boton no hacia nada,
//!    y con rugosidad 0 el color asomaba por las cuatro esquinas.
//!
//! Cada caso se guarda **antes y despues**: el «antes» reproduce aqui lo que
//! hacia el codigo viejo (lados con `Pintor::linea`, que es el extremo plano
//! de siempre; relleno por la caja; redondeo solo sin temblor) para poder
//! mirar los dos juntos. Lo que se comprueba a maquina es lo unico que una
//! prueba puede decir de una imagen sin verla: que la esquina de fuera esta
//! pintada y que el fondo no pasa del contorno redondo.
//!
//! `cargo test -p pixpin --test muestra_de_esquinas -- --ignored --nocapture
//! --test-threads=1`. Deja los PNG en `target/muestras-de-esquinas`.

use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use pixpin_motor2d::pintado::{self, Orden};
use pixpin_motor2d::relleno::EstiloRelleno;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_render::{Color, MotorRender, Pintor};

const ANCHO: u32 = 240;
const ALTO: u32 = 180;
/// Cuanto se amplia la esquina en su muestra, pixel a pixel.
const LUPA: u32 = 8;

const BLANCO: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

fn a_color(c: ColorRgba) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

fn motor_y_dispositivo() -> (pixpin_capture::Dispositivo, MotorRender) {
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let m = MotorRender::nuevo(d.d3d()).expect("motor");
    (d, m)
}

fn carpeta() -> std::path::PathBuf {
    let d =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/muestras-de-esquinas");
    std::fs::create_dir_all(&d).expect("crear la carpeta de muestras");
    d
}

/// Un rectangulo (o rombo) de 160 x 110 en (40, 35), trazo gordo para que la
/// esquina se vea, con fondo rojo claro.
fn figura(
    figura: Figura,
    redondo: bool,
    rugosidad: f32,
    relleno: EstiloRelleno,
    grosor: f32,
) -> Elemento {
    Elemento {
        id: 1,
        figura,
        x: 40.0,
        y: 35.0,
        ancho: 160.0,
        alto: 110.0,
        trazo: ColorRgba::opaco(0.12, 0.12, 0.14),
        relleno: Some(ColorRgba::opaco(1.0, 0.79, 0.79)),
        estilo_relleno: relleno,
        grosor,
        rugosidad,
        redondo,
        semilla: 4,
        ..Default::default()
    }
}

/// Las ordenes que producia el codigo de antes: relleno por la caja recta, y
/// el redondeo solo en el rectangulo sin temblor.
fn ordenes_de_antes(e: &Elemento) -> Vec<Orden> {
    let recto = Elemento {
        redondo: e.redondo && e.rugosidad <= 0.0 && e.figura == Figura::Rectangulo,
        ..e.clone()
    };
    let mut o = pintado::ordenes(&Elemento {
        redondo: false,
        ..e.clone()
    });
    if recto.redondo {
        // El contorno liso redondo de entonces, en una pasada.
        o.retain(|x| !matches!(x, Orden::Polilinea { grosor, .. } if *grosor == e.grosor));
        o.push(Orden::Polilinea {
            puntos: pixpin_motor2d::formas::rectangulo_redondo(e.x, e.y, e.ancho, e.alto),
            color: e.trazo,
            grosor: e.grosor,
            estilo: EstiloTrazo::Solido,
        });
    }
    if e.redondo && e.figura == Figura::Rombo {
        // El rombo redondo ya salia liso siempre; solo el relleno iba en pico.
        o.retain(|x| !matches!(x, Orden::Polilinea { grosor, .. } if *grosor == e.grosor));
        o.push(Orden::Polilinea {
            puntos: pixpin_motor2d::formas::rombo_redondo(e.x, e.y, e.ancho, e.alto),
            color: e.trazo,
            grosor: e.grosor,
            estilo: EstiloTrazo::Solido,
        });
    }
    o
}

/// Pinta ordenes como el editor. `antes` traza cada polilinea tramo a tramo
/// con `Pintor::linea`, que es el trazo de extremo plano que usaba
/// `Pintor::polilinea` hasta ahora.
fn pintar(p: &Pintor, ordenes: &[Orden], antes: bool) {
    p.limpiar(BLANCO);
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
                if antes && puntos.len() == 2 {
                    p.linea(
                        (puntos[0].x, puntos[0].y),
                        (puntos[1].x, puntos[1].y),
                        *grosor,
                        a_color(*color),
                    );
                } else {
                    let v: Vec<(f32, f32)> = puntos.iter().map(|q| (q.x, q.y)).collect();
                    p.polilinea(&v, *grosor, a_color(*color));
                }
            }
            _ => {}
        }
    }
}

fn leer(fuera: &FueraDePantalla, motor: &MotorRender, ordenes: &[Orden], antes: bool) -> Vec<u8> {
    motor
        .dibujar(&fuera.destino, |p| pintar(p, ordenes, antes))
        .expect("pintar");
    fuera.esperar_gpu().expect("esperar");
    fuera.leer_rgba().expect("leer").2
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

/// La esquina de arriba a la izquierda (24 x 24 alrededor de (40, 35))
/// ampliada `LUPA` veces, pixel a pixel.
fn esquina_ampliada(rgba: &[u8]) -> (u32, u32, Vec<u8>) {
    let (x0, y0, lado) = (28u32, 23u32, 24u32);
    let n = lado * LUPA;
    let mut v = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let (sx, sy) = (x0 + x / LUPA, y0 + y / LUPA);
            let i = ((sy * ANCHO + sx) * 4) as usize;
            v.extend_from_slice(&rgba[i..i + 4]);
        }
    }
    (n, n, v)
}

fn pixel(rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * ANCHO + x) * 4) as usize;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}

fn oscuro(p: [u8; 4]) -> bool {
    p[0].max(p[1]).max(p[2]) < 128
}

#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn la_esquina_de_un_rectangulo_esta_entera_y_el_relleno_redondo_no_se_sale() {
    let (dispositivo, motor) = motor_y_dispositivo();
    let fuera =
        FueraDePantalla::nuevo(&motor, dispositivo.d3d(), ANCHO, ALTO).expect("fuera de pantalla");

    // 1. La muesca: rectangulo en pico, sin temblor, trazo de 8.
    let pico = figura(Figura::Rectangulo, false, 0.0, EstiloRelleno::Solido, 8.0);
    let nuevo = leer(&fuera, &motor, &pintado::ordenes(&pico), false);
    let viejo = leer(&fuera, &motor, &ordenes_de_antes(&pico), true);
    // Un punto de la esquina de FUERA, a 2,4 px del vertice por cada eje:
    // dentro del circulo del extremo redondo (radio 4), fuera de los dos
    // lados con extremo plano.
    let (ex, ey) = (40 - 3, 35 - 3);
    assert!(
        oscuro(pixel(&nuevo, ex, ey)),
        "la esquina de fuera sigue sin pintar: {:?}",
        pixel(&nuevo, ex, ey)
    );
    // El caso negativo que prueba que esto mira lo que dice mirar: con el
    // trazo de antes, ahi habia un hueco.
    assert!(
        !oscuro(pixel(&viejo, ex, ey)),
        "el «antes» no reproduce la muesca: la prueba no distingue nada"
    );
    for (nombre, rgba) in [("antes", viejo), ("despues", nuevo)] {
        let (w, h, v) = esquina_ampliada(&rgba);
        guardar(&format!("esquina-{nombre}"), w, h, v);
        guardar(&format!("rectangulo-pico-{nombre}"), ANCHO, ALTO, rgba);
    }

    // 2. El redondeo con cada rugosidad y cada relleno, antes y despues.
    for (clase, f) in [("rectangulo", Figura::Rectangulo), ("rombo", Figura::Rombo)] {
        for rugosidad in [0.0, 1.0, 2.0] {
            for (nombre_relleno, relleno) in [
                ("solido", EstiloRelleno::Solido),
                ("rayado", EstiloRelleno::Rayado),
                ("cruzado", EstiloRelleno::Cruzado),
            ] {
                let e = figura(f.clone(), true, rugosidad, relleno, 3.0);
                let nombre = format!("{clase}-redondo-r{rugosidad}-{nombre_relleno}");
                let despues = leer(&fuera, &motor, &pintado::ordenes(&e), false);
                let antes = leer(&fuera, &motor, &ordenes_de_antes(&e), true);
                if clase == "rectangulo" {
                    // A 3 px del vertice en diagonal, dentro de la caja pero
                    // fuera de la curva (radio 27,5): ni fondo ni trazo.
                    let p = pixel(&despues, 43, 38);
                    assert!(
                        p[0] > 240 && p[1] > 240 && p[2] > 240,
                        "{nombre}: la esquina de la caja no es blanca: {p:?}"
                    );
                }
                guardar(&format!("{nombre}-antes"), ANCHO, ALTO, antes);
                guardar(&format!("{nombre}-despues"), ANCHO, ALTO, despues);
            }
        }
    }
}
