//! Las puertas de rendimiento de S3-A (§6 de la spec).
//!
//! Se ejecutan en release, que es donde vive el usuario. En depuracion el
//! coma flotante de Rust va varias veces mas lento y los numeros no dicen
//! nada, asi que los topes se relajan segun el perfil: lo que se comprueba
//! siempre es que el orden de magnitud es el correcto, no que la maquina de
//! CI sea rapida.

use std::time::Instant;

use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use pixpin_motor2d::{Ajustes, Escena, Punto2, ordenes, ordenes_de_escena, poligono};

/// Los topes de la spec valen para release; en depuracion se multiplican.
const FACTOR: u32 = if cfg!(debug_assertions) { 20 } else { 1 };

fn trazo_largo(n: usize) -> Vec<Punto2> {
    (0..n)
        .map(|i| {
            let t = i as f32 * 0.1;
            Punto2::nuevo(t * 8.0, 300.0 + (t.sin() * 120.0))
        })
        .collect()
}

fn elemento(i: u64) -> Elemento {
    Elemento {
        id: i,
        figura: Figura::Rectangulo,
        x: (i % 40) as f32 * 30.0,
        y: (i / 40) as f32 * 30.0,
        ancho: 120.0,
        alto: 80.0,
        angulo: 0.0,
        trazo: ColorRgba::opaco(0.1, 0.1, 0.1),
        relleno: None,
        grosor: 2.0,
        estilo: EstiloTrazo::Solido,
        rugosidad: 1.0,
        opacidad: 1.0,
        semilla: (i as u32 * 7919) | 1,
        version: 0,
        borrado: false,
    }
}

#[test]
fn un_trazo_de_500_puntos_se_convierte_en_poligono_en_menos_de_2_ms() {
    let puntos = trazo_largo(500);
    let a = Ajustes::default();
    // Una pasada en frio para que las paginas de memoria ya esten tocadas:
    // se mide el algoritmo, no el primer fallo de pagina.
    let _ = poligono(&puntos, &a);

    let t = Instant::now();
    let repeticiones = 20;
    for _ in 0..repeticiones {
        let p = poligono(&puntos, &a);
        assert!(!p.is_empty());
    }
    let micros = t.elapsed().as_micros() / repeticiones;
    let tope = 2_000 * FACTOR as u128;
    assert!(
        micros <= tope,
        "el trazo de 500 puntos tarda {micros} us y el tope es {tope}"
    );
    println!("trazo de 500 puntos: {micros} us");
}

#[test]
fn mil_elementos_ocupan_menos_de_20_mb() {
    // Medida por estructura, no por asignador: lo que se quiere acotar es el
    // tamano del modelo, que es lo que crece con el documento.
    let mut e = Escena::nueva();
    for i in 0..1000 {
        e.anadir(elemento(i));
    }
    let por_elemento = size_of::<Elemento>();
    let total = por_elemento * e.elementos.len();
    assert!(
        total < 20 * 1024 * 1024,
        "mil elementos ocupan {total} bytes ({por_elemento} cada uno)"
    );
    println!("mil elementos: {total} bytes, {por_elemento} por elemento");
}

#[test]
fn generar_las_ordenes_de_100_elementos_cabe_en_un_fotograma() {
    // El caso real de un dibujo cargado: 100 figuras a mano. El cache que
    // pide la spec (D43) evita rehacerlo cada fotograma, pero incluso SIN
    // cache tiene que caber, porque es lo que pasa al cargar el documento.
    let mut e = Escena::nueva();
    for i in 0..100 {
        e.anadir(elemento(i));
    }
    let _ = ordenes_de_escena(&e);

    let t = Instant::now();
    let ordenes = ordenes_de_escena(&e);
    let micros = t.elapsed().as_micros();
    assert!(!ordenes.is_empty());
    // Un fotograma a 60 Hz son 16 666 us.
    let tope = 16_666 * FACTOR as u128;
    assert!(
        micros <= tope,
        "100 elementos tardan {micros} us y el tope es {tope}"
    );
    println!("100 elementos: {micros} us, {} ordenes", ordenes.len());
}

#[test]
fn abrir_un_dibujo_de_mil_elementos_tarda_menos_de_150_ms() {
    let dir = std::env::temp_dir().join("pixpin-motor2d-puerta-abrir");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let ruta = dir.join("grande.pixpin2d");

    let mut e = Escena::nueva();
    for i in 0..1000 {
        e.anadir(elemento(i));
    }
    pixpin_motor2d::guardar(&ruta, &e).unwrap();

    let t = Instant::now();
    let leida = pixpin_motor2d::cargar(&ruta).unwrap();
    let ms = t.elapsed().as_millis();

    assert_eq!(leida.elementos.len(), 1000);
    let tope = 150 * FACTOR as u128;
    assert!(ms <= tope, "abrir mil elementos tarda {ms} ms, tope {tope}");
    println!("abrir mil elementos: {ms} ms");
}

#[test]
fn el_dibujo_es_identico_al_reabrirlo() {
    // La puerta que de verdad importa: no es velocidad, es que el documento
    // del usuario se vea igual manana. Se guarda, se lee y se comparan las
    // ordenes de dibujo punto por punto.
    let dir = std::env::temp_dir().join("pixpin-motor2d-puerta-identico");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let ruta = dir.join("d.pixpin2d");

    let mut e = Escena::nueva();
    for i in 0..30 {
        e.anadir(elemento(i));
    }
    e.anadir(Elemento {
        figura: Figura::Lapiz {
            puntos: trazo_largo(80),
            presiones: vec![],
        },
        ..elemento(99)
    });

    let antes = ordenes_de_escena(&e);
    pixpin_motor2d::guardar(&ruta, &e).unwrap();
    let despues = ordenes_de_escena(&pixpin_motor2d::cargar(&ruta).unwrap());

    assert_eq!(
        antes, despues,
        "el dibujo cambia al reabrirlo: la semilla no esta haciendo su trabajo"
    );
}

/// Un dibujo con la forma del que el usuario trajo de su movil: 194
/// elementos, 8.003 puntos, extendido sobre 1013 x 1920.
///
/// No es un numero inventado para que la prueba salga bien; es lo que se
/// midio de `Proyecto 2.pixpin`, y por eso es la carga con la que tiene que
/// ir fluido.
fn dibujo_como_el_del_movil() -> Escena {
    let mut escena = Escena::nueva();
    for i in 0..194u64 {
        let fila = (i / 14) as f32;
        let col = (i % 14) as f32;
        let base = Punto2::nuevo(165.0 + col * 72.0, 228.0 + fila * 137.0);
        let puntos: Vec<Punto2> = (0..41)
            .map(|k| {
                let t = k as f32 * 0.25;
                Punto2::nuevo(base.x + t * 6.0, base.y + t.sin() * 18.0)
            })
            .collect();
        let mut e = elemento(i);
        e.x = base.x;
        e.y = base.y;
        e.figura = Figura::Lapiz {
            puntos,
            presiones: Vec::new(),
        };
        escena.anadir(e);
    }
    escena
}

/// Cuantos puntos de geometria hay que fabricar para pintar esto.
///
/// Es la medida que importa de verdad: cada punto se calcula en el procesador
/// antes de que el GPU vea nada, y es lo unico que crece con el dibujo.
fn puntos_de(v: &[pixpin_motor2d::Orden]) -> usize {
    v.iter()
        .map(|o| match o {
            pixpin_motor2d::Orden::Poligono { puntos, .. }
            | pixpin_motor2d::Orden::Polilinea { puntos, .. }
            | pixpin_motor2d::Orden::Relleno { puntos, .. } => puntos.len(),
            _ => 0,
        })
        .sum()
}

#[test]
fn de_lejos_el_dibujo_cuesta_una_decima_parte() {
    // La promesa del lienzo infinito en un equipo modesto. Los numeros con
    // los que se eligio la tecnica, medidos sobre este mismo dibujo: al 50 %
    // baja al 11,7 %, al 20 % al 7,4 % y al 5 % al 5,3 %. Se exige la quinta
    // parte, que deja sitio de sobra y sigue cazando una regresion de verdad.
    let escena = dibujo_como_el_del_movil();
    let entero = puntos_de(&ordenes_de_escena(&escena));
    for zoom in [0.5, 0.2, 0.05] {
        let c = pixpin_motor2d::Camara {
            x: 100.0,
            y: 150.0,
            zoom,
        };
        let lejos = puntos_de(&pixpin_motor2d::ordenes_de_escena_vista(
            &escena, &c, 1920.0, 1080.0,
        ));
        assert!(lejos > 0, "al {zoom} no se pinto nada");
        assert!(
            lejos * 5 < entero,
            "al {zoom} deberia costar mucho menos: {lejos} contra {entero}"
        );
    }
}

#[test]
fn de_cerca_se_dibuja_la_tinta_entera() {
    // La otra mitad del trato, y la que importa mas: a tamano natural no se
    // puede notar nada. Un trazo grande tiene que seguir saliendo como tinta
    // —un poligono relleno, con su afilado y sus tapas— y no como una raya.
    let escena = dibujo_como_el_del_movil();
    let e = escena.visibles().next().expect("el dibujo tiene elementos");
    let cerca = pixpin_motor2d::ordenes_a_distancia(e, 4.0);
    assert_eq!(
        cerca,
        ordenes(e),
        "de cerca el trazo tiene que salir identico a como se guardo"
    );
    assert!(
        matches!(cerca.first(), Some(pixpin_motor2d::Orden::Poligono { .. })),
        "de cerca un lapiz es tinta, no una linea: {:?}",
        cerca.first()
    );
}

#[test]
fn un_trazo_diminuto_se_dibuja_como_una_raya() {
    // El cambio que hace toda la economia: por debajo del umbral, la tinta se
    // sustituye por una linea del mismo grosor. A ese tamano el ojo no
    // distingue una de otra, y cuesta una fraccion.
    let escena = dibujo_como_el_del_movil();
    let e = escena.visibles().next().expect("el dibujo tiene elementos");
    let lejos = pixpin_motor2d::ordenes_a_distancia(e, 0.1);
    assert!(
        matches!(lejos.first(), Some(pixpin_motor2d::Orden::Polilinea { .. })),
        "de lejos deberia bastar una linea: {:?}",
        lejos.first()
    );
    assert!(
        puntos_de(&lejos) * 4 < puntos_de(&ordenes(e)),
        "la linea no salio mas barata: {} contra {}",
        puntos_de(&lejos),
        puntos_de(&ordenes(e))
    );
}

#[test]
fn lo_que_esta_fuera_de_la_pantalla_ni_se_calcula() {
    // El recorte es lo que hace que un lienzo infinito no se vuelva mas lento
    // cuanto mas se dibuja: mirando a un rincon vacio no cuesta nada, tenga
    // el dibujo lo que tenga.
    let escena = dibujo_como_el_del_movil();
    let vacio = pixpin_motor2d::Camara {
        x: 900_000.0,
        y: 900_000.0,
        zoom: 1.0,
    };
    let ahora = Instant::now();
    let salida = pixpin_motor2d::ordenes_de_escena_vista(&escena, &vacio, 1920.0, 1080.0);
    let tardo = ahora.elapsed();
    assert!(
        salida.is_empty(),
        "pinto {} cosas de un sitio vacio",
        salida.len()
    );
    assert!(
        tardo.as_millis() < (2 * FACTOR) as u128,
        "mirar a un sitio vacio tardo {tardo:?}"
    );
}

#[test]
fn sin_aumento_no_se_simplifica_nada() {
    // Caso negativo: exportar a un fichero pide el dibujo entero, y pedirlo
    // con un cero no puede devolver una version simplificada en silencio.
    let escena = dibujo_como_el_del_movil();
    let e = escena.visibles().next().expect("el dibujo tiene elementos");
    assert_eq!(pixpin_motor2d::ordenes_a_distancia(e, 0.0), ordenes(e));
    assert_eq!(pixpin_motor2d::ordenes_a_distancia(e, -1.0), ordenes(e));
}
