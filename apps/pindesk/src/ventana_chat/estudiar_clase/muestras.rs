//! **Muestra en PNG de «Estudiar la clase»** (2-nov-2026), para las
//! capturas de la Store: una clase de 52 minutos con sus capturas de la
//! pizarra, la transcripcion con su minuto, marcadores y los temas de la
//! reunion (dos tratados y uno pendiente).
//!
//! ```text
//! cargo test -p pindesk --bin pindesk muestra_de_estudiar_la_clase -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Deja `estudiar-clase.png` en `PINDESK_MUESTRAS` (o la temporal). Sin
//! sonido de verdad (no hay nota que abrir) lo que suena es el principio:
//! se ve la primera captura en grande.

use std::path::Path;
use std::time::{Duration, Instant};

use pindesk_render::fuera_de_pantalla::FueraDePantalla;

use super::*;

const MIN: i64 = 60_000;

/// Una «diapositiva» de la clase: papel claro, una franja de titulo de
/// color y unas rayas de texto y una figura, distintas en cada una.
fn diapositiva(ruta: &Path, n: u32) {
    let (w, h) = (1280u32, 720u32);
    let colores = [[42, 122, 226], [229, 83, 75], [36, 138, 61], [232, 163, 61]];
    let color = colores[n as usize % colores.len()];
    let mut px = vec![255u8; (w * h * 4) as usize];
    let mut rect = |x0: u32, y0: u32, x1: u32, y1: u32, c: [u8; 3]| {
        for y in y0..y1.min(h) {
            for x in x0..x1.min(w) {
                let i = ((y * w + x) * 4) as usize;
                px[i..i + 3].copy_from_slice(&c);
            }
        }
    };
    rect(0, 0, w, h, [250, 250, 247]);
    rect(0, 0, w, 120, color);
    for k in 0..(4 + n % 3) {
        let largo = 520 + (k * 97 + n * 53) % 360;
        rect(80, 180 + k * 70, 80 + largo, 204 + k * 70, [70, 78, 92]);
    }
    // La figura: unos ejes y una curva hecha de puntos gruesos.
    rect(820, 220, 824, 620, [40, 40, 40]);
    rect(820, 616, 1200, 620, [40, 40, 40]);
    for x in 0..360u32 {
        let t = x as f32 / 360.0;
        let y = 600.0 - 360.0 * (t * t * (1.0 + n as f32 * 0.15)).min(1.0);
        rect(
            830 + x,
            y as u32,
            836 + x,
            y as u32 + 6,
            [color[0] / 2, color[1] / 2, color[2] / 2],
        );
    }
    let png = pindesk_codec::imagen::codificar_png(&ImagenRgba {
        ancho: w,
        alto: h,
        pixeles: px,
    })
    .unwrap();
    std::fs::write(ruta, png).unwrap();
}

fn clase_de_muestra(carpeta: &Path) -> Clase {
    let tiempos = [MIN + 20_000, 9 * MIN, 15 * MIN + 30_000, 27 * MIN, 41 * MIN];
    let capturas = tiempos
        .iter()
        .enumerate()
        .map(|(i, ms)| {
            let ruta = carpeta.join(format!("pizarra-{i}.png"));
            diapositiva(&ruta, i as u32);
            (*ms, ruta)
        })
        .collect::<Vec<_>>();
    let lineas = [
        (
            0,
            "Buenos días. Hoy terminamos la regla de la cadena y empezamos con las derivadas implícitas.",
        ),
        (
            MIN + 20_000,
            "Recordad: si y = f(g(x)), la derivada es f′(g(x)) por g′(x).",
        ),
        (
            4 * MIN,
            "Un ejemplo: la derivada de sen(x²) es cos(x²) por 2x.",
        ),
        (9 * MIN, "Ahora, ¿qué pasa cuando no podemos despejar y?"),
        (
            12 * MIN,
            "Derivamos los dos lados respecto de x, y cada vez que derivamos y multiplicamos por y′.",
        ),
        (
            15 * MIN + 30_000,
            "En la circunferencia x² + y² = 25 queda 2x + 2y·y′ = 0.",
        ),
        (
            17 * MIN,
            "Y de ahí y′ = −x / y. Fijaos en que la pendiente depende del punto.",
        ),
        (
            21 * MIN,
            "Esto entra en el parcial: es la pregunta tres de todos los años.",
        ),
        (27 * MIN, "Hacemos juntos el ejercicio 4.2 de la hoja."),
        (
            34 * MIN,
            "¿Dudas? La del fondo: sí, la recta tangente sale con la pendiente y el punto.",
        ),
        (
            41 * MIN,
            "Para el lunes: ejercicios 4.3 a 4.6, y repasad la regla del producto.",
        ),
    ];
    let temas = {
        let mut t: Vec<agenda::Tema> = [
            "Repaso de la regla de la cadena",
            "Derivadas implícitas",
            "Ejercicios 4.2 a 4.6",
            "Dudas del examen parcial",
        ]
        .iter()
        .enumerate()
        .map(|(i, x)| agenda::Tema::nuevo(&format!("t{i}"), x))
        .collect();
        t[0].hecho = true;
        t[0].ms = Some(MIN + 20_000);
        t[0].por_jev = true;
        t[1].hecho = true;
        t[1].ms = Some(9 * MIN);
        t[1].por_jev = true;
        t[2].hecho = true;
        t[2].ms = Some(27 * MIN);
        t[2].a_mano = true;
        t
    };
    let mut c = Clase {
        proyecto: "calculo".into(),
        id: "clase-muestra".into(),
        nombre: "Cálculo II · derivadas implícitas".into(),
        duracion_ms: 52 * MIN,
        audio: None,
        video: None,
        fotos: capturas
            .iter()
            .map(|_| cuaderno::Mensaje::default())
            .collect(),
        autores: capturas.iter().map(|_| (None, false)).collect(),
        capturas,
        clips: Vec::new(),
        lineas: lineas
            .iter()
            .map(|(ms, t)| (Some(*ms), (*t).to_string()))
            .collect(),
        importantes: None,
        temas,
        acta: acta_viva::Acta::default(),
        senales: Vec::new(),
        filas: Vec::new(),
        tiempos_filas: Vec::new(),
    };
    let marcador = |ms: i64, texto: &str| Senal {
        ms,
        texto: texto.into(),
        banderita: false,
        clip: None,
    };
    c.poner_senales(vec![
        marcador(16 * MIN, "Fórmula de y′ en la circunferencia"),
        marcador(21 * MIN + 10_000, "Entra en el parcial"),
        Senal {
            ms: 41 * MIN + 30_000,
            texto: String::new(),
            banderita: true,
            clip: None,
        },
    ]);
    c
}

#[test]
#[ignore = "necesita GPU; genera PNG para mirarlos"]
fn muestra_de_estudiar_la_clase() {
    let carpeta =
        std::env::temp_dir().join(format!("pixpin-muestra-estudiar-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&carpeta);
    std::fs::create_dir_all(&carpeta).unwrap();
    let textos = Catalogo::nuevo(Idioma::Espanol);
    let recursos = Recursos::nuevos().expect("GPU real");
    let motor = recursos.motor();
    let mut v = Vista::nueva(&recursos, carpeta.clone(), clase_de_muestra(&carpeta), 0);

    // Las miniaturas y la grande llegan de su hilo: se esperan un poco.
    let hasta = Instant::now() + Duration::from_secs(10);
    while Instant::now() < hasta {
        v.imagenes(&motor);
        v.al_dia_lo_visto();
        v.seguir_al_sonido();
        if v.miniaturas.iter().all(Option::is_some) && v.grande.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let (ancho, alto) = (ANCHO, ALTO);
    let e = 1.0;
    v.reparto.franja = v.clase.con_franja();
    v.disp = disponer(ancho as f32, alto as f32, e, v.reparto);
    let fuera = FueraDePantalla::nuevo(&motor, &recursos.d3d(), ancho, alto).expect("superficie");
    // Dos vueltas: la primera mide el texto y coloca las listas.
    for _ in 0..2 {
        motor
            .dibujar(&fuera.destino, |p| {
                pintar_todo(p, &mut v, ancho as f32, alto as f32, &CLARO, e, &textos)
            })
            .expect("pintar");
        fuera.esperar_gpu().expect("esperar");
    }
    let (_, _, pixeles) = fuera.leer_rgba().expect("leer");
    let png = pindesk_codec::imagen::codificar_png(&ImagenRgba {
        ancho,
        alto,
        pixeles,
    })
    .expect("codificar");
    let destino = std::env::var_os("PINDESK_MUESTRAS")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let ruta = destino.join("estudiar-clase.png");
    std::fs::write(&ruta, png).expect("guardar");
    println!("estudiar-clase: {}", ruta.display());
    drop(v);
    let _ = std::fs::remove_dir_all(&carpeta);
}
