//! **Las cuatro herramientas de construir, pintadas y guardadas en un PNG.**
//!
//! El bote de relleno, recortar, extender y el punto etiquetado no se
//! comprueban con un `assert_eq`: se miran. Las de abajo son pruebas de
//! verdad —fallan solas—, pero lo que de verdad contestan es la pregunta que
//! ninguna cifra contesta: *¿se ve lo que tenia que verse?*
//!
//! Lo que si se comprueba a maquina, que es lo unico que se puede decir de una
//! imagen sin verla:
//!
//! - que el bote sobre un recinto **cerrado** deja mas tinta que el mismo
//!   dibujo sin rellenar —esta pintando—, y que sobre el mismo recinto
//!   **abierto** deja exactamente la misma que sin rellenar: no pinta nada, que
//!   es la respuesta correcta y el fallo mas caro de esta herramienta;
//! - que recortar deja **menos** tinta de la que habia —se ha quitado algo— y
//!   extender, mas;
//! - y que ni los puntos ni los angulos dejan el lienzo en blanco, que es como
//!   se ve una herramienta que produce ordenes que no dibujan.
//!
//! Necesita GPU y sesion de escritorio, como las demas puertas de pintado:
//! `cargo test -p pixpin --test muestra_de_construir -- --ignored --nocapture
//! --test-threads=1`.

use pixpin_motor2d::angulos::{self, JUNTA};
use pixpin_motor2d::elemento::{ColorRgba, Elemento, Figura};
use pixpin_motor2d::pintado::{self, Orden};
use pixpin_motor2d::puntos_etiquetados::{self, RADIO_PARA_PUNTOS, SerieDePunto};
use pixpin_motor2d::recorte::{self, ALCANCE_EXTENDER};
use pixpin_motor2d::regiones::{self, AjustesRelleno};
use pixpin_motor2d::vector::Punto2;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_render::{Color, MotorRender};

const ANCHO: u32 = 300;
const ALTO: u32 = 300;

const BLANCO: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

const AMARILLO: ColorRgba = ColorRgba {
    r: 1.0,
    g: 0.8,
    b: 0.2,
    a: 1.0,
};

const AZUL: ColorRgba = ColorRgba {
    r: 0.2,
    g: 0.45,
    b: 0.9,
    a: 0.95,
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

/// Una raya de trazo grueso, para que se vea lo que hace.
fn raya(id: u64, a: (f32, f32), b: (f32, f32)) -> Elemento {
    Elemento {
        id,
        figura: Figura::Linea {
            puntos: vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
        },
        x: a.0.min(b.0),
        y: a.1.min(b.1),
        ancho: (b.0 - a.0).abs(),
        alto: (b.1 - a.1).abs(),
        trazo: ColorRgba::opaco(0.12, 0.12, 0.14),
        grosor: 3.0,
        // Sin temblor: lo que se mira aqui es la FORMA, y la rugosidad la
        // ensucia sin añadir nada.
        rugosidad: 0.0,
        ..Default::default()
    }
}

/// Un recinto de 200x200 hecho con **cuatro rayas sueltas**, que es el caso que
/// el bote viene a resolver: ese cuadrado no es de ninguna de las cuatro.
fn recinto_cerrado() -> Vec<Elemento> {
    vec![
        raya(1, (50.0, 50.0), (250.0, 50.0)),
        raya(2, (250.0, 50.0), (250.0, 250.0)),
        raya(3, (250.0, 250.0), (50.0, 250.0)),
        raya(4, (50.0, 250.0), (50.0, 50.0)),
    ]
}

/// El mismo, con un boquete de 60 px en el lado de arriba.
fn recinto_abierto() -> Vec<Elemento> {
    let mut v = recinto_cerrado();
    v[0] = raya(1, (50.0, 50.0), (120.0, 50.0));
    v.push(raya(5, (180.0, 50.0), (250.0, 50.0)));
    v
}

fn ordenes_de(elementos: &[Elemento]) -> Vec<Orden> {
    elementos.iter().flat_map(pintado::ordenes).collect()
}

/// Pinta las ordenes sobre blanco y devuelve los pixeles RGBA.
///
/// Es la misma traduccion de `Orden` a `Pintor` que hace el editor, recortada a
/// lo que estas escenas producen: no hay imagenes ni velo que resolver.
fn pintar(fuera: &FueraDePantalla, motor: &MotorRender, ordenes: &[Orden]) -> Vec<u8> {
    motor
        .dibujar(&fuera.destino, |p| {
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
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/muestras-de-construir");
    std::fs::create_dir_all(&d).expect("crear la carpeta de muestras");
    d
}

fn guardar(carpeta: &std::path::Path, nombre: &str, rgba: Vec<u8>) -> u64 {
    let tinta = tinta_derramada(&rgba);
    let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
        ancho: ANCHO,
        alto: ALTO,
        pixeles: rgba,
    })
    .expect("codificar");
    let ruta = carpeta.join(format!("construir-{nombre}.png"));
    std::fs::write(&ruta, png).expect("guardar");
    println!("{nombre:>18}: {tinta:>9} de tinta -> {}", ruta.display());
    tinta
}

#[test]
#[ignore = "necesita GPU y sesion de escritorio"]
fn las_cuatro_herramientas_de_construir_pintan_lo_que_tienen_que_pintar() {
    let (dispositivo, motor) = motor_y_dispositivo();
    let fuera =
        FueraDePantalla::nuevo(&motor, dispositivo.d3d(), ANCHO, ALTO).expect("fuera de pantalla");
    let carpeta = carpeta_de_muestras();

    // ---------------------------------------------------------------
    // 1. El bote sobre un recinto CERRADO, con un agujero dentro.
    // ---------------------------------------------------------------
    let mut cerrado = recinto_cerrado();
    cerrado.push(Elemento {
        id: 9,
        figura: Figura::Elipse,
        x: 120.0,
        y: 120.0,
        ancho: 60.0,
        alto: 60.0,
        trazo: ColorRgba::opaco(0.12, 0.12, 0.14),
        grosor: 3.0,
        rugosidad: 0.0,
        ..Default::default()
    });
    let sin_rellenar = guardar(
        &carpeta,
        "bote-antes",
        pintar(&fuera, &motor, &ordenes_de(&cerrado)),
    );

    let plantilla = Elemento {
        relleno: Some(AMARILLO),
        trazo: ColorRgba::opaco(0.12, 0.12, 0.14),
        grosor: 2.0,
        rugosidad: 0.0,
        ..Default::default()
    };
    let region = regiones::region_en(
        &cerrado,
        Punto2::nuevo(70.0, 70.0),
        &AjustesRelleno::default(),
    )
    .expect("el recinto esta cerrado");
    assert_eq!(
        region.huecos.len(),
        1,
        "el circulo de dentro tiene que salir como agujero"
    );
    let mancha = regiones::nueva_region(region, &plantilla);
    // El relleno va DEBAJO de las paredes, como en la escena de verdad.
    let mut con_bote = vec![mancha];
    con_bote.extend(cerrado.clone());
    let relleno = guardar(
        &carpeta,
        "bote-cerrado",
        pintar(&fuera, &motor, &ordenes_de(&con_bote)),
    );
    assert!(
        relleno > sin_rellenar,
        "el bote no pinto nada sobre un recinto cerrado"
    );

    // ---------------------------------------------------------------
    // 2. El mismo bote sobre el recinto ABIERTO: no pinta, y eso es la
    //    respuesta correcta.
    // ---------------------------------------------------------------
    let abierto = recinto_abierto();
    let sin_nada = guardar(
        &carpeta,
        "bote-abierto",
        pintar(&fuera, &motor, &ordenes_de(&abierto)),
    );
    assert!(
        regiones::region_en(
            &abierto,
            Punto2::nuevo(150.0, 150.0),
            &AjustesRelleno::default()
        )
        .is_none(),
        "el derrame se escapo por la rendija: lo que se pintaria seria una mancha \
         del tamaño del dibujo entero encima de todo lo demas"
    );
    assert!(sin_nada > 0, "las paredes del recinto abierto si se ven");

    // ---------------------------------------------------------------
    // 3. Recortar: la punta que sobra se va.
    // ---------------------------------------------------------------
    let larga = raya(1, (30.0, 150.0), (270.0, 150.0));
    let pared = raya(2, (180.0, 40.0), (180.0, 260.0));
    let antes_recorte = guardar(
        &carpeta,
        "recortar-antes",
        pintar(&fuera, &motor, &ordenes_de(&[larga.clone(), pared.clone()])),
    );
    let r = recorte::recortar_en(
        &larga,
        std::slice::from_ref(&pared),
        Punto2::nuevo(240.0, 150.0),
    )
    .expect("hay algo que recortar");
    let corta = r.queda.expect("queda el trozo de la izquierda");
    let despues_recorte = guardar(
        &carpeta,
        "recortar-despues",
        pintar(&fuera, &motor, &ordenes_de(&[corta.clone(), pared.clone()])),
    );
    assert!(
        despues_recorte < antes_recorte,
        "recortar tiene que dejar MENOS tinta: {despues_recorte} vs {antes_recorte}"
    );

    // ---------------------------------------------------------------
    // 4. Extender: la punta que se queda corta llega hasta la pared.
    // ---------------------------------------------------------------
    // Una raya que se queda corta: acaba en el aire, a 60 px de la pared.
    let quedada = raya(3, (30.0, 150.0), (120.0, 150.0));
    let antes_extender = guardar(
        &carpeta,
        "extender-antes",
        pintar(
            &fuera,
            &motor,
            &ordenes_de(&[quedada.clone(), pared.clone()]),
        ),
    );
    let estirada = recorte::extender_en(
        &quedada,
        std::slice::from_ref(&pared),
        Punto2::nuevo(120.0, 150.0),
        ALCANCE_EXTENDER,
    )
    .expect("la pared esta en su camino");
    let despues_extender = guardar(
        &carpeta,
        "extender-despues",
        pintar(&fuera, &motor, &ordenes_de(&[estirada, pared])),
    );
    assert!(
        despues_extender > antes_extender,
        "extender tiene que dejar MAS tinta: {despues_extender} vs {antes_extender}"
    );

    // ---------------------------------------------------------------
    // 5. Los puntos etiquetados: A, B y C en los vertices de un triangulo.
    // ---------------------------------------------------------------
    let mut croquis = vec![
        raya(1, (60.0, 240.0), (240.0, 240.0)),
        raya(2, (240.0, 240.0), (150.0, 70.0)),
        raya(3, (150.0, 70.0), (60.0, 240.0)),
    ];
    let molde = Elemento {
        trazo: ColorRgba::opaco(0.75, 0.1, 0.1),
        grosor: 3.0,
        rugosidad: 0.0,
        ..Default::default()
    };
    for (i, vertice) in [(60.0, 240.0), (240.0, 240.0), (150.0, 70.0)]
        .into_iter()
        .enumerate()
    {
        let p = Punto2::nuevo(vertice.0, vertice.1);
        let donde = puntos_etiquetados::sitio_para_punto(&croquis, p, RADIO_PARA_PUNTOS)
            .expect("un vertice del triangulo es un sitio notable");
        let mut punto =
            puntos_etiquetados::nuevo_punto(donde, &croquis, SerieDePunto::Mayusculas, &molde);
        punto.id = 100 + i as u64;
        croquis.push(punto);
    }
    let letras: Vec<String> = croquis
        .iter()
        .filter_map(|e| match &e.figura {
            Figura::Punto { letra, .. } => Some(letra.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(letras, vec!["A", "B", "C"], "las letras van en serie");
    let con_puntos = guardar(
        &carpeta,
        "puntos-etiquetados",
        pintar(&fuera, &motor, &ordenes_de(&croquis)),
    );
    assert!(con_puntos > 0);

    // ---------------------------------------------------------------
    // 6. Los angulos en vivo sobre una esquina que se esta moviendo.
    // ---------------------------------------------------------------
    let esquina = vec![
        raya(1, (60.0, 200.0), (240.0, 200.0)),
        raya(2, (60.0, 200.0), (160.0, 60.0)),
    ];
    let medidos = angulos::angulos_internos(&esquina, &[2], JUNTA);
    assert_eq!(medidos.len(), 1, "una esquina, un numero");
    let mut con_angulo = ordenes_de(&esquina);
    con_angulo.extend(angulos::ordenes_de_angulos(&medidos, 1.0, AZUL));
    let con_rotulo = guardar(
        &carpeta,
        "angulo-en-vivo",
        pintar(&fuera, &motor, &con_angulo),
    );
    let sin_rotulo = tinta_derramada(&pintar(&fuera, &motor, &ordenes_de(&esquina)));
    assert!(
        con_rotulo > sin_rotulo,
        "el rotulo del angulo no dibujo nada: {con_rotulo} vs {sin_rotulo}"
    );
}
