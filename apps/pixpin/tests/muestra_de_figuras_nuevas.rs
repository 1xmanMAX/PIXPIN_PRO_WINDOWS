//! **Las figuras que abre la tanda cero, pintadas y guardadas en un PNG.**
//!
//! Una figura nueva no se comprueba con un `assert_eq`: se mira. El fallo que
//! esto viene a impedir no es que el codigo se caiga, es que produzca ordenes
//! que no dibujan nada —que es exactamente lo que le pasaba al mosaico y al
//! rombo antes de esta tanda: el elemento sobrevivia al guardar y en pantalla
//! no habia nada—.
//!
//! Lo que si se comprueba a maquina es lo unico que una prueba puede decir de
//! una imagen sin verla: que ninguna de las seis deja el lienzo en blanco, y
//! que un arco todavia sin repasar —la guia— deja MENOS tinta que el mismo
//! arco ya trazado, porque un ovalo punteado no puede pintar tanto como una
//! curva maciza.
//!
//! Necesita GPU y sesion de escritorio, como las demas puertas de pintado:
//! `cargo test -p pixpin --test muestra_de_figuras_nuevas -- --ignored
//! --nocapture --test-threads=1`.

use pixpin_motor2d::elemento::{ColorRgba, Elemento, Figura};
use pixpin_motor2d::pintado::{self, Orden};
use pixpin_motor2d::vector::Punto2;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_render::{Color, MotorRender};

const ANCHO: u32 = 260;
const ALTO: u32 = 200;

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

/// Una figura centrada en el lienzo de muestra, con trazo grueso para que se
/// vea lo que hace y no haya que adivinarlo.
fn muestra(figura: Figura, relleno: Option<ColorRgba>) -> Elemento {
    Elemento {
        id: 1,
        figura,
        x: 40.0,
        y: 40.0,
        ancho: 180.0,
        alto: 120.0,
        trazo: ColorRgba::opaco(0.12, 0.12, 0.14),
        relleno,
        grosor: 3.0,
        // Sin temblor: lo que se mira aqui es la FORMA, y la rugosidad la
        // ensucia sin anadir nada a lo que esta prueba comprueba.
        rugosidad: 0.0,
        ..Default::default()
    }
}

/// Pinta las ordenes de `e` sobre blanco y devuelve los pixeles RGBA.
///
/// Es la misma traduccion de `Orden` a `Pintor` que hace el editor
/// (`ventana_editor::dibujar_orden`), recortada a lo que estas seis figuras
/// producen: no hay imagenes ni velo que resolver.
fn pintar(fuera: &FueraDePantalla, motor: &MotorRender, e: &Elemento) -> Vec<u8> {
    motor
        .dibujar(&fuera.destino, |p| {
            p.limpiar(BLANCO);
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
                        estilo,
                    } => {
                        let v: Vec<(f32, f32)> = puntos.iter().map(|q| (q.x, q.y)).collect();
                        if matches!(estilo, pixpin_motor2d::elemento::EstiloTrazo::Solido) {
                            p.polilinea(&v, *grosor, a_color(*color));
                        } else {
                            p.polilinea_discontinua(&v, *grosor, a_color(*color));
                        }
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

/// Cuanta tinta hay en el papel: lo oscuro que sale el lienzo entero.
fn tinta_derramada(rgba: &[u8]) -> u64 {
    rgba.chunks_exact(4)
        .map(|p| (255 - p[0].min(p[1]).min(p[2])) as u64)
        .sum()
}

fn carpeta_de_muestras() -> std::path::PathBuf {
    let d =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/muestras-de-figuras");
    std::fs::create_dir_all(&d).expect("crear la carpeta de muestras");
    d
}

#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn las_figuras_nuevas_pintan_algo_y_no_dejan_el_lienzo_en_blanco() {
    let (dispositivo, motor) = motor_y_dispositivo();
    let fuera =
        FueraDePantalla::nuevo(&motor, dispositivo.d3d(), ANCHO, ALTO).expect("fuera de pantalla");
    let carpeta = carpeta_de_muestras();
    let amarillo = Some(ColorRgba {
        r: 1.0,
        g: 0.8,
        b: 0.2,
        a: 1.0,
    });

    let casos: Vec<(&str, Elemento)> = vec![
        ("rombo", muestra(Figura::Rombo, amarillo)),
        (
            "mosaico",
            muestra(Figura::Mosaico { desenfoque: true }, None),
        ),
        (
            "arco-guia",
            muestra(
                Figura::Arco {
                    inicio: 0.0,
                    barrido: None,
                },
                None,
            ),
        ),
        (
            "arco-trazado",
            muestra(
                Figura::Arco {
                    inicio: std::f32::consts::PI,
                    barrido: Some(std::f32::consts::PI),
                },
                None,
            ),
        ),
        ("serie", muestra(Figura::Serie { numero: 7 }, amarillo)),
        (
            "region",
            muestra(
                Figura::Region {
                    contorno: vec![
                        Punto2::nuevo(40.0, 40.0),
                        Punto2::nuevo(220.0, 40.0),
                        Punto2::nuevo(220.0, 160.0),
                        Punto2::nuevo(40.0, 160.0),
                    ],
                    huecos: vec![vec![
                        Punto2::nuevo(100.0, 80.0),
                        Punto2::nuevo(160.0, 80.0),
                        Punto2::nuevo(160.0, 120.0),
                        Punto2::nuevo(100.0, 120.0),
                    ]],
                },
                amarillo,
            ),
        ),
        (
            "punto",
            muestra(
                Figura::Punto {
                    letra: "A".into(),
                    angulo: -std::f32::consts::FRAC_PI_4,
                    radio: 22.0,
                },
                None,
            ),
        ),
    ];

    let mut huellas: Vec<(&str, u64)> = Vec::new();
    for (nombre, e) in &casos {
        let rgba = pintar(&fuera, &motor, e);
        let con_tinta = tinta_derramada(&rgba);
        assert!(
            con_tinta > 0,
            "«{nombre}» dejo el lienzo en blanco: produce ordenes que no dibujan nada, \
             que es justo el fallo que tenian el mosaico y el rombo antes de esta tanda"
        );
        let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
            ancho: ANCHO,
            alto: ALTO,
            pixeles: rgba,
        })
        .expect("codificar");
        let ruta = carpeta.join(format!("figura-{nombre}.png"));
        std::fs::write(&ruta, png).expect("guardar");
        println!(
            "{nombre:>13}: {con_tinta:>9} de tinta -> {}",
            ruta.display()
        );
        huellas.push((nombre, con_tinta));
    }

    let de = |n: &str| huellas.iter().find(|(k, _)| *k == n).unwrap().1;
    // La guia es un ovalo PUNTEADO y el arco trazado, media curva maciza: si
    // las dos dejaran la misma tinta, o la guia no se ve o no se distingue
    // del arco, y es su unica senal de «esto todavia no esta repasado».
    assert_ne!(
        de("arco-guia"),
        de("arco-trazado"),
        "la guia y el arco repasado se ven igual"
    );
    // El mosaico TAPA: es la figura que mas tinta deja de todas, porque es
    // una mancha opaca del tamano de su caja. Si dejara poca, estaria
    // dejando ver lo que viene a tapar.
    assert!(
        de("mosaico") > de("rombo"),
        "un mosaico que tapa menos que un rombo no esta tapando"
    );
}

/// **Las ocho puntas, la flecha de codos y las dos tramas nuevas, en una hoja
/// de contactos.**
///
/// Las ocho puntas no se comprueban con un `assert_eq`: lo que hay que ver es
/// que se DISTINGAN unas de otras, porque en un diagrama entidad-relacion la
/// punta dice la cardinalidad y dos puntas iguales son dos cosas distintas
/// que se leen igual. Lo que si se comprueba a maquina es lo unico que una
/// prueba puede decir sin mirar: que ninguna deja la flecha pelada, que las
/// macizas manchan mas que sus huecas, y que la de codos no traza la misma
/// raya que la recta.
///
/// `cargo test -p pixpin --test muestra_de_figuras_nuevas -- --ignored
/// --nocapture --test-threads=1`
#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn las_ocho_puntas_y_las_dos_tramas_se_distinguen_de_un_vistazo() {
    use pixpin_motor2d::TipoPunta;
    use pixpin_motor2d::relleno::EstiloRelleno;

    let (dispositivo, motor) = motor_y_dispositivo();
    let fuera =
        FueraDePantalla::nuevo(&motor, dispositivo.d3d(), ANCHO, ALTO).expect("fuera de pantalla");
    let carpeta = carpeta_de_muestras();

    // Una flecha larga y horizontal: la punta cae siempre en el mismo sitio,
    // que es lo que permite comparar las ocho mirando el mismo trozo de hoja.
    let flecha = |punta: TipoPunta, codos: bool| Elemento {
        figura: Figura::Flecha {
            puntos: vec![Punto2::nuevo(30.0, 100.0), Punto2::nuevo(230.0, 100.0)],
            punta_inicio: TipoPunta::Ninguna,
            punta_fin: punta,
            codos,
        },
        grosor: 3.0,
        ..muestra(Figura::Rectangulo, None)
    };

    let mut huellas: Vec<(String, u64)> = Vec::new();
    let mut guardar = |nombre: String, e: &Elemento| {
        let rgba = pintar(&fuera, &motor, e);
        let tinta = tinta_derramada(&rgba);
        let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
            ancho: ANCHO,
            alto: ALTO,
            pixeles: rgba,
        })
        .expect("codificar");
        let ruta = carpeta.join(format!("{nombre}.png"));
        std::fs::write(&ruta, png).expect("guardar");
        println!("{nombre:>24}: {tinta:>9} de tinta -> {}", ruta.display());
        huellas.push((nombre, tinta));
    };

    guardar("punta-ninguna".into(), &flecha(TipoPunta::Ninguna, false));
    for punta in pixpin_motor2d::PUNTAS {
        let palabra = punta.palabra().expect("las ocho tienen palabra");
        guardar(format!("punta-{palabra}"), &flecha(punta, false));
    }
    // La de codos, con los extremos en diagonal: en horizontal el conector
    // ortogonal y la recta son la misma raya y no habria nada que mirar.
    let mut codos = flecha(TipoPunta::Flecha, true);
    let mut recta = flecha(TipoPunta::Flecha, false);
    for e in [&mut codos, &mut recta] {
        if let Figura::Flecha { puntos, .. } = &mut e.figura {
            *puntos = vec![Punto2::nuevo(40.0, 40.0), Punto2::nuevo(220.0, 160.0)];
        }
    }
    guardar("flecha-de-codos".into(), &codos);
    guardar("flecha-recta".into(), &recta);

    let amarillo = Some(ColorRgba {
        r: 1.0,
        g: 0.8,
        b: 0.2,
        a: 1.0,
    });
    for (nombre, estilo) in [
        ("rayado", EstiloRelleno::Rayado),
        ("zigzag", EstiloRelleno::Zigzag),
        ("lineas-pixpin", EstiloRelleno::LineasPixpin),
    ] {
        let mut e = muestra(Figura::Rectangulo, amarillo);
        e.estilo_relleno = estilo;
        // Con rugosidad, que es donde se ve la diferencia entre el rayado
        // tembloroso y el tiralineas: sin ella los dos salen rectos.
        e.rugosidad = 1.0;
        guardar(format!("trama-{nombre}"), &e);
    }

    let de = |n: &str| huellas.iter().find(|(k, _)| k == n).unwrap().1;
    for punta in pixpin_motor2d::PUNTAS {
        let palabra = punta.palabra().unwrap();
        assert!(
            de(&format!("punta-{palabra}")) > de("punta-ninguna"),
            "la punta «{palabra}» no dibuja nada: la flecha sale pelada"
        );
    }
    // Maciza contra hueca: si mancharan lo mismo, una es la otra y el
    // diagrama pierde la distincion que la punta venia a decir.
    for (maciza, hueca) in [
        ("circle", "circle_outline"),
        ("triangle", "triangle_outline"),
        ("diamond", "diamond_outline"),
    ] {
        assert!(
            de(&format!("punta-{maciza}")) > de(&format!("punta-{hueca}")),
            "«{maciza}» no mancha mas que «{hueca}»: la maciza no se esta rellenando"
        );
    }
    assert_ne!(
        de("flecha-de-codos"),
        de("flecha-recta"),
        "la flecha de codos traza la misma raya que la recta"
    );
    // El tiralineas es el mismo barrido SIN temblor: por eso no puede dejar
    // exactamente la misma tinta que el rayado tembloroso.
    assert_ne!(
        de("trama-lineas-pixpin"),
        de("trama-rayado"),
        "el tiralineas sale igual que el rayado: no se esta repartiendo"
    );
    // El zigzag dibuja DOS ramas por raya: mancha mas que el rayado simple.
    assert!(
        de("trama-zigzag") > de("trama-rayado"),
        "el zigzag no dibuja sus dos ramas"
    );
}
