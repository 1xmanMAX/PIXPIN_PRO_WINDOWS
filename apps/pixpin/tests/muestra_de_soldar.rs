//! **Soldar vertices y numerar puntos, pintados y guardados en PNG para
//! mirarlos** (`target/muestras-de-construir/soldar-*.png`).
//!
//! Lo que se comprueba a maquina es lo unico que se puede decir sin verlo: que
//! la cabeza roja del clavo esta donde se clavo, y que tras girar o llevarse el
//! clavo sigue en la junta. Necesita GPU y sesion de escritorio:
//! `cargo test -p pixpin --test muestra_de_soldar -- --ignored --nocapture
//! --test-threads=1`.

use pixpin_motor2d::elemento::{ColorRgba, Elemento, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::nudos;
use pixpin_motor2d::pintado::{self, Orden};
use pixpin_motor2d::puntos_etiquetados::{self, SerieDePunto};
use pixpin_motor2d::vector::Punto2;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_render::{Color, MotorRender};

const ANCHO: u32 = 300;
const ALTO: u32 = 300;

fn a_color(c: ColorRgba) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

fn raya(a: (f32, f32), b: (f32, f32)) -> Elemento {
    Elemento {
        figura: Figura::Linea {
            puntos: vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
        },
        x: a.0.min(b.0),
        y: a.1.min(b.1),
        ancho: (b.0 - a.0).abs(),
        alto: (b.1 - a.1).abs(),
        trazo: ColorRgba::opaco(0.12, 0.12, 0.14),
        grosor: 3.0,
        rugosidad: 0.0,
        ..Default::default()
    }
}

fn triangulo() -> Escena {
    let mut e = Escena::nueva();
    e.anadir(raya((50.0, 230.0), (250.0, 230.0)));
    e.anadir(raya((250.0, 230.0), (150.0, 60.0)));
    e.anadir(raya((150.0, 60.0), (50.0, 230.0)));
    e
}

/// La escena y, encima, las cabezas de los clavos: lo mismo que encadena el
/// lienzo (`dibujo::pintar::pintar_encima`).
fn ordenes(escena: &Escena) -> Vec<Orden> {
    let mut v: Vec<Orden> = escena.visibles().flat_map(pintado::ordenes).collect();
    v.extend(nudos::ordenes_de_clavos(escena, 1.0));
    v
}

fn pintar(fuera: &FueraDePantalla, motor: &MotorRender, ordenes: &[Orden]) -> Vec<u8> {
    motor
        .dibujar(&fuera.destino, |p| {
            p.limpiar(Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            });
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
    let (_, _, rgba) = fuera.leer_rgba().expect("leer");
    rgba
}

fn guardar(nombre: &str, rgba: &[u8]) {
    let d =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/muestras-de-construir");
    std::fs::create_dir_all(&d).expect("carpeta");
    let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
        ancho: ANCHO,
        alto: ALTO,
        pixeles: rgba.to_vec(),
    })
    .expect("codificar");
    let ruta = d.join(format!("soldar-{nombre}.png"));
    std::fs::write(&ruta, png).expect("guardar");
    println!("{nombre:>16} -> {}", ruta.display());
}

/// Si el pixel de `p` es el rojo del clavo.
fn es_rojo(rgba: &[u8], p: Punto2) -> bool {
    let i = ((p.y.round() as u32 * ANCHO + p.x.round() as u32) * 4) as usize;
    rgba[i] > 180 && rgba[i + 1] < 90 && rgba[i + 2] < 90
}

#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn soldar_vertices_y_numerar_puntos_se_ven_como_tienen_que_verse() {
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d.d3d()).expect("motor");
    let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), ANCHO, ALTO).expect("fuera de pantalla");

    // 1. El triangulo con sus tres esquinas soldadas.
    let mut escena = triangulo();
    for p in [(250.0, 230.0), (150.0, 60.0), (50.0, 230.0)] {
        assert!(nudos::soldar(&mut escena, Punto2::nuevo(p.0, p.1), 16.0));
    }
    let rgba = pintar(&fuera, &motor, &ordenes(&escena));
    guardar("1-tres-clavos", &rgba);
    assert!(
        es_rojo(&rgba, Punto2::nuevo(250.0, 230.0)),
        "no se ve el clavo"
    );
    assert!(
        !es_rojo(&rgba, Punto2::nuevo(150.0, 230.0)),
        "rojo donde no hay clavo"
    );

    // 2. Llevarse el clavo de arriba: el triangulo se deforma, no se abre.
    let i = nudos::clavo_en(&escena, Punto2::nuevo(150.0, 60.0), 16.0).unwrap();
    nudos::mover_clavo(&mut escena, i, Punto2::nuevo(90.0, 40.0));
    let rgba = pintar(&fuera, &motor, &ordenes(&escena));
    guardar("2-clavo-llevado", &rgba);
    assert!(
        es_rojo(&rgba, Punto2::nuevo(90.0, 40.0)),
        "el clavo no llego"
    );

    // 3. Una escuadra con un solo clavo: arrastrar la base la hace girar.
    let mut escena = Escena::nueva();
    let base = escena.anadir(raya((60.0, 200.0), (200.0, 200.0)));
    escena.anadir(raya((200.0, 200.0), (200.0, 60.0)));
    assert!(nudos::soldar(
        &mut escena,
        Punto2::nuevo(200.0, 200.0),
        16.0
    ));
    nudos::arrastrar(
        &mut escena,
        &[base],
        Punto2::nuevo(80.0, 200.0),
        Punto2::nuevo(90.0, 270.0),
    );
    let rgba = pintar(&fuera, &motor, &ordenes(&escena));
    guardar("3-gira-sobre-el-clavo", &rgba);
    assert!(
        es_rojo(&rgba, Punto2::nuevo(200.0, 200.0)),
        "el clavo se movio al girar"
    );

    // 4. Numerar puntos: la misma cruz con las tres series.
    let mut escena = Escena::nueva();
    escena.anadir(raya((20.0, 150.0), (280.0, 150.0)));
    for (x, serie) in [
        (70.0, SerieDePunto::Mayusculas),
        (150.0, SerieDePunto::Minusculas),
        (230.0, SerieDePunto::Numeros),
    ] {
        escena.anadir(raya((x, 60.0), (x, 240.0)));
        let molde = Elemento {
            trazo: ColorRgba::opaco(0.85, 0.2, 0.2),
            grosor: 2.0,
            ..Default::default()
        };
        // En el cruce y en la punta de arriba.
        for toque in [Punto2::nuevo(x + 3.0, 152.0), Punto2::nuevo(x - 2.0, 63.0)] {
            let otros: Vec<Elemento> = escena.elementos.clone();
            let donde =
                puntos_etiquetados::sitio_para_punto(&otros, toque, 30.0).expect("sin sitio");
            let punto = puntos_etiquetados::nuevo_punto(donde, &otros, serie, &molde);
            escena.anadir(punto);
        }
    }
    let letras: Vec<String> = escena
        .visibles()
        .filter_map(|e| match &e.figura {
            Figura::Punto { letra, .. } => Some(letra.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(letras, ["A", "B", "a", "b", "1", "2"]);
    let rgba = pintar(&fuera, &motor, &ordenes(&escena));
    guardar("4-puntos-numerados", &rgba);
}
