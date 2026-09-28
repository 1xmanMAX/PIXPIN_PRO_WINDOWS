//! **La muestra del grafito, pintada y guardada en PNG, y lo que cuesta.**
//!
//! El grafito se mira: un trazo a mano, el mismo muy de cerca (donde tienen
//! que verse las casillas de canto vivo), un repaso, y las figuras de grafito
//! —rectangulo con fondo, ovalo rayado, flecha—. Los PNG quedan en
//! `target/muestras-de-tintas/grafito-*.png`.
//!
//! Y se mide, porque es la condicion del encargo: un trazo de grafito no puede
//! hacer ir lento el lienzo en un i3 de tercera generacion. Se miden los dos
//! casos que importan: el trazo en curso —que se vuelve a cocer y a subir en
//! cada fotograma, porque cambia en cada uno— y un dibujo lleno de grafito ya
//! cocido, que es un `DrawBitmap` por trazo.
//!
//! Necesita GPU y sesion de escritorio:
//! `cargo test --release -p pixpin --test muestra_del_grafito -- --ignored
//! --nocapture --test-threads=1`.

use std::time::{Duration, Instant};

use pixpin_motor2d::elemento::{ColorRgba, Elemento, Figura};
use pixpin_motor2d::relleno::EstiloRelleno;
use pixpin_motor2d::tinta::{self, MaterialTinta, grafito};
use pixpin_motor2d::vector::Punto2;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_render::{CacheGrafito, Color, MapaGrafito, MotorRender, Pintor};

fn motor_y_dispositivo() -> (pixpin_capture::Dispositivo, MotorRender) {
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let m = MotorRender::nuevo(d.d3d()).expect("motor");
    (d, m)
}

const BLANCO: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

const GRAFITO: ColorRgba = ColorRgba::opaco(0.12, 0.12, 0.14);

/// Un garabato a pulso: una onda con algo de temblor, como el de la muestra
/// de tintas, para que se vean las curvas y las dos puntas.
fn onda(x0: f32, y0: f32, largo: f32, n: usize) -> Vec<Punto2> {
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            Punto2::nuevo(
                x0 + t * largo,
                y0 + (t * std::f32::consts::TAU * 1.5).sin() * 22.0 + (i as f32 * 1.7).sin() * 0.6,
            )
        })
        .collect()
}

fn trazo(id: u64, puntos: Vec<Punto2>, grosor: f32) -> Elemento {
    Elemento {
        id,
        figura: Figura::Lapiz {
            puntos,
            presiones: Vec::new(),
            opciones: Some(tinta::OpcionesTinta::default()),
        },
        grosor,
        trazo: GRAFITO,
        material: MaterialTinta::Cuadritos,
        ..Default::default()
    }
}

fn mapa(c: &grafito::Cocido) -> MapaGrafito<'_> {
    MapaGrafito {
        rgba: &c.rgba,
        ancho: c.ancho,
        alto: c.alto,
        caja: c.caja(),
        huella: c.huella,
        angulo: c.angulo,
        centro: (c.centro.x, c.centro.y),
        id: c.id,
        generacion: c.generacion,
        sucio: c.sucio.map(|(desde, r)| {
            (
                desde,
                (r.x as u32, r.y as u32, r.ancho as u32, r.alto as u32),
            )
        }),
    }
}

/// Pinta lo cocido de `elementos` con la vista `(escala, desplazamiento)` y
/// devuelve los pixeles.
fn pintar(
    fuera: &FueraDePantalla,
    motor: &MotorRender,
    elementos: &[Elemento],
    escala: f32,
    desplazamiento: (f32, f32),
    cache: &mut CacheGrafito,
) -> Vec<u8> {
    motor
        .dibujar(&fuera.destino, |p: &Pintor| {
            p.limpiar(BLANCO);
            p.poner_vista((0.0, 0.0), escala, desplazamiento);
            for e in elementos {
                let c = grafito::cocer(e).expect("se cuece");
                assert!(p.grafito(Some(cache), &mapa(&c), 1.0, escala));
            }
        })
        .expect("pintar");
    fuera.esperar_gpu().expect("esperar");
    fuera.leer_rgba().expect("leer").2
}

fn guardar(nombre: &str, ancho: u32, alto: u32, rgba: Vec<u8>) {
    let d =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/muestras-de-tintas");
    std::fs::create_dir_all(&d).expect("carpeta");
    let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
        ancho,
        alto,
        pixeles: rgba,
    })
    .expect("codificar");
    let ruta = d.join(format!("grafito-{nombre}.png"));
    std::fs::write(&ruta, png).expect("guardar");
    println!("-> {}", ruta.display());
}

fn tinta_derramada(rgba: &[u8]) -> u64 {
    rgba.chunks_exact(4).map(|p| (255 - p[0]) as u64).sum()
}

#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn la_muestra_del_grafito_de_lejos_de_cerca_y_en_figuras() {
    let (dispositivo, motor) = motor_y_dispositivo();
    let (ancho, alto) = (900u32, 420u32);
    let fuera = FueraDePantalla::nuevo(&motor, dispositivo.d3d(), ancho, alto).expect("fuera");
    let mut cache = CacheGrafito::nueva();

    // Los tres grosores del lapiz (1, 2 y 3 del teclado), una pasada; y el
    // medio repasado ida y vuelta, que tiene que salir mas cargado.
    let mut repaso = onda(40.0, 330.0, 820.0, 220);
    repaso.extend(onda(40.0, 330.0, 820.0, 220).into_iter().rev());
    let trazos = vec![
        trazo(1, onda(40.0, 60.0, 820.0, 220), tinta::GROSOR_FINO),
        trazo(2, onda(40.0, 150.0, 820.0, 220), tinta::GROSOR_MEDIO),
        trazo(3, onda(40.0, 240.0, 820.0, 220), tinta::GROSOR_GRUESO),
        trazo(4, repaso, tinta::GROSOR_MEDIO),
    ];
    let rgba = pintar(&fuera, &motor, &trazos, 1.0, (0.0, 0.0), &mut cache);
    let de_lejos = tinta_derramada(&rgba);
    assert!(de_lejos > 0, "no se pinto nada");
    guardar("trazos-al-100", ancho, alto, rgba);

    // **De cerca, cada casilla de canto vivo**: a 8x, un cuadrito son 8x8
    // pixeles de pantalla del mismo color. Se miran todas las filas y se
    // cuentan los saltos de color: tienen que caer en
    // multiplos de 8 (las juntas de la rejilla), no en cualquier sitio.
    let zoom = 8.0;
    let rgba = pintar(
        &fuera,
        &motor,
        &trazos[1..2],
        zoom,
        (-300.0 * zoom, -120.0 * zoom),
        &mut cache,
    );
    let mut saltos_fuera_de_junta = 0;
    let mut saltos = 0;
    for fila in 0..alto as usize {
        for x in 1..ancho as usize {
            let a = &rgba[(fila * ancho as usize + x - 1) * 4..][..3];
            let b = &rgba[(fila * ancho as usize + x) * 4..][..3];
            if a != b {
                saltos += 1;
                if x % zoom as usize != 0 {
                    saltos_fuera_de_junta += 1;
                }
            }
        }
    }
    println!("de cerca: {saltos} saltos, {saltos_fuera_de_junta} fuera de junta");
    assert!(saltos > 10, "de cerca no se ven casillas: {saltos} saltos");
    assert_eq!(saltos_fuera_de_junta, 0, "las casillas salen suavizadas");
    guardar("de-cerca-x8", ancho, alto, rgba);

    // **Como la simulacion del movil** (`docs/img/lapiz-cuadritos-simulacion.png`
    // de su repositorio): rojo sobre negro, tres ondas de tres grosores y la
    // de en medio muy de cerca, para ponerlas una al lado de la otra.
    let rojo = ColorRgba::opaco(1.0, 0.13, 0.08);
    let ondas: Vec<Elemento> = [
        tinta::GROSOR_FINO,
        tinta::GROSOR_MEDIO,
        tinta::GROSOR_GRUESO,
    ]
    .into_iter()
    .enumerate()
    .map(|(k, g)| Elemento {
        trazo: rojo,
        ..trazo(
            20 + k as u64,
            onda(10.0, 45.0 + k as f32 * 60.0, 300.0, 160),
            g,
        )
    })
    .collect();
    let negro = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    for (nombre, escala, desplazamiento) in [
        ("como-el-movil", 1.0f32, (0.0f32, 0.0f32)),
        ("como-el-movil-de-cerca", 5.0, (-40.0 * 5.0, -80.0 * 5.0)),
    ] {
        motor
            .dibujar(&fuera.destino, |p: &Pintor| {
                p.limpiar(negro);
                p.poner_vista((0.0, 0.0), escala, desplazamiento);
                for e in &ondas {
                    let c = grafito::cocer(e).expect("se cuece");
                    p.grafito(Some(&mut cache), &mapa(&c), 1.0, escala);
                }
            })
            .expect("pintar");
        fuera.esperar_gpu().expect("esperar");
        guardar(nombre, ancho, alto, fuera.leer_rgba().expect("leer").2);
    }

    // Las figuras de grafito: las del gesto de pararse y las de la barra con
    // el grafito en la mano.
    let caja = |id, figura, x, y, w, h, relleno: Option<ColorRgba>, estilo| Elemento {
        id,
        figura,
        x,
        y,
        ancho: w,
        alto: h,
        grosor: 2.0,
        rugosidad: 1.0,
        semilla: id as u32 * 7 + 1,
        trazo: GRAFITO,
        relleno,
        estilo_relleno: estilo,
        material: MaterialTinta::Cuadritos,
        ..Default::default()
    };
    let figuras = vec![
        caja(
            10,
            Figura::Rectangulo,
            40.0,
            40.0,
            220.0,
            150.0,
            Some(ColorRgba::opaco(0.2, 0.4, 0.85)),
            EstiloRelleno::Solido,
        ),
        caja(
            11,
            Figura::Elipse,
            320.0,
            40.0,
            240.0,
            150.0,
            Some(ColorRgba::opaco(0.85, 0.3, 0.2)),
            EstiloRelleno::Rayado,
        ),
        caja(
            12,
            Figura::Rombo,
            620.0,
            40.0,
            220.0,
            150.0,
            None,
            EstiloRelleno::Solido,
        ),
        Elemento {
            figura: Figura::Flecha {
                puntos: vec![Punto2::nuevo(60.0, 300.0), Punto2::nuevo(800.0, 340.0)],
                punta_inicio: pixpin_motor2d::formas::TipoPunta::Ninguna,
                punta_fin: pixpin_motor2d::formas::TipoPunta::Triangulo,
                codos: false,
            },
            x: 60.0,
            y: 300.0,
            ..caja(
                13,
                Figura::Rectangulo,
                0.0,
                0.0,
                0.0,
                0.0,
                None,
                EstiloRelleno::Solido,
            )
        },
    ];
    let rgba = pintar(&fuera, &motor, &figuras, 1.0, (0.0, 0.0), &mut cache);
    assert!(tinta_derramada(&rgba) > 0);
    guardar("figuras", ancho, alto, rgba);
}

#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn el_grafito_no_vuelve_lento_el_lienzo() {
    let (dispositivo, motor) = motor_y_dispositivo();
    let fuera = FueraDePantalla::nuevo(&motor, dispositivo.d3d(), 1920, 1080).expect("fuera");

    // **El trazo en curso**: un garabato largo de 1.500 muestras (unos diez
    // segundos de mano a 150 Hz) que se vuelve a cocer, subir y pintar cada
    // tres muestras, como hace la tinta viva en cada fotograma. Lo que cuenta
    // es el fotograma peor y el del final, que es el trazo entero.
    let todos: Vec<Punto2> = (0..1500)
        .map(|i| {
            let t = i as f32 / 1500.0;
            Punto2::nuevo(
                100.0 + t * 1700.0,
                540.0 + (t * std::f32::consts::TAU * 6.0).sin() * 380.0,
            )
        })
        .collect();
    // Lo que cuesta un fotograma sin nada (limpiar 1920x1080 y esperar a la
    // GPU): es el suelo del arnes, y lo que se mide es lo que el grafito pone
    // encima.
    let mut vacios = Vec::new();
    for _ in 0..60 {
        let t0 = Instant::now();
        motor
            .dibujar(&fuera.destino, |p| p.limpiar(BLANCO))
            .expect("pintar");
        fuera.esperar_gpu().expect("esperar");
        vacios.push(t0.elapsed());
    }
    let suelo = mediana(&mut vacios);
    let mut cocidos = Vec::new();
    let mut fotogramas = Vec::new();
    let mut e = trazo(99, Vec::new(), tinta::GROSOR_MEDIO);
    for fin in (3..=todos.len()).step_by(3) {
        if let Figura::Lapiz { puntos, .. } = &mut e.figura {
            *puntos = todos[..fin].to_vec();
        }
        e.tocar();
        let t0 = Instant::now();
        let c = grafito::cocer(&e).expect("se cuece");
        let cocer = t0.elapsed();
        motor
            .dibujar(&fuera.destino, |p| {
                p.limpiar(BLANCO);
                // Sin cache: es lo que hace el editor con el trazo en curso.
                p.grafito(None, &mapa(&c), 1.0, 1.0);
            })
            .expect("pintar");
        fuera.esperar_gpu().expect("esperar");
        fotogramas.push(t0.elapsed());
        cocidos.push(cocer);
    }
    let n = fotogramas.len();
    let (peor, peor_cocer) = (
        *fotogramas.iter().max().unwrap(),
        *cocidos.iter().max().unwrap(),
    );
    let (medio, medio_cocer) = (mediana(&mut fotogramas), mediana(&mut cocidos));
    let encima = medio.saturating_sub(suelo);
    println!(
        "trazo en curso ({n} fotogramas): mediana {medio:?} (suelo del arnes {suelo:?}, el grafito pone {encima:?}; cocer {medio_cocer:?}), peor {peor:?} (cocer peor {peor_cocer:?})"
    );

    // **Un dibujo lleno de grafito ya cocido**: 300 trazos, cada uno un
    // `DrawBitmap` desde la cache. Es lo que cuesta un paneo.
    let quietos: Vec<Elemento> = (0..300)
        .map(|i| {
            let (bx, by) = ((i % 20) as f32 * 95.0, (i / 20) as f32 * 70.0);
            trazo(
                1000 + i as u64,
                onda(bx + 10.0, by + 35.0, 80.0, 40),
                tinta::GROSOR_MEDIO,
            )
        })
        .collect();
    let mut cache = CacheGrafito::nueva();
    // El primer fotograma sube los 300 mapas.
    let t0 = Instant::now();
    pintar(&fuera, &motor, &quietos, 1.0, (0.0, 0.0), &mut cache);
    let primero = t0.elapsed();
    let mut paneos = Vec::new();
    for k in 0..30 {
        let t0 = Instant::now();
        motor
            .dibujar(&fuera.destino, |p| {
                p.limpiar(BLANCO);
                p.poner_vista((0.0, 0.0), 1.0, (-(k as f32) * 3.0, 0.0));
                for e in &quietos {
                    let c = grafito::cocer(e).expect("cocido");
                    p.grafito(Some(&mut cache), &mapa(&c), 1.0, 1.0);
                }
            })
            .expect("pintar");
        fuera.esperar_gpu().expect("esperar");
        paneos.push(t0.elapsed());
    }
    let paneo_peor = *paneos.iter().max().unwrap();
    let paneo = mediana(&mut paneos);
    println!(
        "300 trazos cocidos: primer fotograma {primero:?}, paneo mediana {paneo:?}, peor {paneo_peor:?}"
    );
    assert_eq!(cache.cuantos(), 300, "se subio algun mapa mas de una vez");

    // Los topes, con margen para un i3 de tercera generacion (este equipo es
    // tres o cuatro veces mas rapido): el trazo en curso y el paneo tienen
    // que caber en un fotograma de 60 Hz aqui con holgura para caber alli.
    // Medianas y no peores: otros procesos compilando a la vez meten picos
    // que no son del grafito.
    assert!(
        encima < Duration::from_millis(3),
        "el trazo en curso pone {encima:?} por fotograma"
    );
    assert!(
        paneo < Duration::from_millis(8),
        "pintar 300 trazos cocidos cuesta {paneo:?}"
    );
}

fn mediana(v: &mut [Duration]) -> Duration {
    v.sort();
    v[v.len() / 2]
}

#[test]
fn el_png_del_grafito_para_el_svg_se_lee_igual_que_el_mapa() {
    // El SVG lleva el mapa con un compresor propio (`grafito::png_del_trozo`):
    // si el deflate saliera mal, el navegador ensenaria un hueco. Se lee con
    // el lector de imagenes de la aplicacion y tiene que dar los mismos
    // pixeles.
    let c = grafito::cocer_sin_horno(&trazo(5, onda(10.0, 40.0, 300.0, 120), tinta::GROSOR_MEDIO))
        .expect("se cuece");
    let trozo = grafito::lo_pintado(&c).expect("tiene algo");
    let png = grafito::png_del_trozo(&c, trozo);
    let ruta = std::env::temp_dir().join(format!("grafito-png-{}.png", std::process::id()));
    std::fs::write(&ruta, &png).expect("escribir");
    let leida = pixpin_codec::imagen::cargar(&ruta).expect("el png se lee");
    let _ = std::fs::remove_file(&ruta);
    let (tx, ty, tw, th) = trozo;
    assert_eq!((leida.ancho, leida.alto), (tw, th));
    for y in 0..th {
        for x in 0..tw {
            let a = (((ty + y) * c.ancho + tx + x) * 4) as usize;
            let b = ((y * tw + x) * 4) as usize;
            // Alfa exacto; el color solo cuenta donde hay algo.
            assert_eq!(leida.pixeles[b + 3], c.rgba[a + 3], "({x},{y})");
            if c.rgba[a + 3] == 255 {
                assert_eq!(&leida.pixeles[b..b + 3], &c.rgba[a..a + 3]);
            }
        }
    }
    // Caso negativo: comprimido de verdad, no los bytes tal cual.
    assert!(png.len() < (tw * th * 4) as usize / 3, "{} bytes", png.len());
}

#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn la_muestra_del_bote_de_grafito_y_de_la_figura_mientras_se_arrastra() {
    let (dispositivo, motor) = motor_y_dispositivo();
    let (ancho, alto) = (900u32, 420u32);
    let fuera = FueraDePantalla::nuevo(&motor, dispositivo.d3d(), ancho, alto).expect("fuera");
    let mut cache = CacheGrafito::nueva();
    let anillo = |x0: f32, y0: f32, x1: f32, y1: f32| {
        vec![
            Punto2::nuevo(x0, y0),
            Punto2::nuevo(x1, y0),
            Punto2::nuevo(x1, y1),
            Punto2::nuevo(x0, y1),
        ]
    };
    // El bote con el grafito en la mano: una mancha de grafito con agujero,
    // y encima la raya del recinto, de grafito tambien.
    let bote = Elemento {
        id: 30,
        figura: Figura::Region {
            contorno: anillo(40.0, 40.0, 400.0, 380.0),
            huecos: vec![anillo(160.0, 150.0, 280.0, 270.0)],
        },
        x: 40.0,
        y: 40.0,
        ancho: 360.0,
        alto: 340.0,
        grosor: 2.0,
        trazo: GRAFITO,
        relleno: Some(ColorRgba::opaco(0.2, 0.45, 0.85)),
        material: MaterialTinta::Cuadritos,
        ..Default::default()
    };
    let recinto = Elemento {
        id: 31,
        figura: Figura::Rectangulo,
        x: 40.0,
        y: 40.0,
        ancho: 360.0,
        alto: 340.0,
        grosor: 2.0,
        semilla: 3,
        trazo: GRAFITO,
        material: MaterialTinta::Cuadritos,
        ..Default::default()
    };
    // La figura a medio arrastrar: la copia con la punta predicha, que es lo
    // que pinta el editor en ese fotograma.
    let en_curso = Elemento {
        id: 32,
        figura: Figura::Elipse,
        x: 480.0,
        y: 60.0,
        ancho: 200.0,
        alto: 120.0,
        grosor: 2.0,
        semilla: 5,
        trazo: GRAFITO,
        material: MaterialTinta::Cuadritos,
        ..Default::default()
    };
    let copia = pixpin_motor2d::tinta::prediccion::con_punta(
        &en_curso,
        Some(Punto2::nuevo(480.0, 60.0)),
        Punto2::nuevo(860.0, 360.0),
    )
    .expect("copia predicha");
    let rgba = pintar(&fuera, &motor, &[bote, recinto, copia], 1.0, (0.0, 0.0), &mut cache);
    // En el agujero, papel; en la mancha, grafito azul.
    let px = |x: usize, y: usize| &rgba[(y * ancho as usize + x) * 4..][..4];
    assert_eq!(px(220, 210)[..3], [255, 255, 255], "el agujero se relleno");
    let azules = (60..140)
        .flat_map(|x| (60..140).map(move |y| (x, y)))
        .filter(|&(x, y)| px(x, y)[2] > px(x, y)[0] + 20)
        .count();
    assert!(azules > 1000, "la mancha del bote no es de grafito azul: {azules}");
    guardar("bote-y-figura-en-curso", ancho, alto, rgba);
}
