//! Las puertas de rendimiento de S3-A (§6 de la spec).
//!
//! Se ejecutan en release, que es donde vive el usuario. En depuracion el
//! coma flotante de Rust va varias veces mas lento y los numeros no dicen
//! nada, asi que los topes se relajan segun el perfil: lo que se comprueba
//! siempre es que el orden de magnitud es el correcto, no que la maquina de
//! CI sea rapida.

use std::time::Instant;

use pixpin_motor2d::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use pixpin_motor2d::{Ajustes, Escena, Punto2, ordenes_de_escena, poligono};

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
