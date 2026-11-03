//! **Muestras en PNG de la ventana de las figuras del chat** (2-nov-2026,
//! como la «Figuras» del iPhone): la cabecera con «Cerrar», el titulo con su
//! cuenta y el ☀/☾ redondo, las categorias en pastillas, los grupos con su
//! cuenta y el buscador flotante. De dia, de noche (tema oscuro y plantillas
//! de noche) y eligiendo para un chat, con la GPU de verdad y sin abrir
//! ninguna ventana.
//!
//! Desde el 3-nov-2026, tambien la figura en grande al lado con sus
//! parametros (de dia y de noche, y acercada) y, con la ventana estrecha, la
//! figura sola con «‹ Figuras», sus parametros y «Adjuntar».
//!
//! ```text
//! cargo test -p pindesk --bin pindesk muestras_de_las_figuras -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Deja los PNG en la carpeta de `PINDESK_MUESTRAS` o, si no, en
//! `%TEMP%/pixpin-muestras-figuras`.

use pindesk_render::MotorRender;
use pindesk_render::fuera_de_pantalla::FueraDePantalla;
use pindesk_store::{Catalogo, Idioma};

use super::*;

#[test]
#[ignore = "necesita GPU; genera PNG para mirarlos"]
fn muestras_de_las_figuras() {
    let textos = Catalogo::nuevo(Idioma::Espanol);
    let serie = pindesk_motor2d::figuras_de_serie::catalogo();
    let d3 = pindesk_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d3.d3d()).expect("motor");
    let (ancho, alto) = (1320u32, 840u32);
    let (estrecha, alto_estrecha) = (720u32, 900u32);
    let escala = 100;
    let salida = std::env::var_os("PINDESK_MUESTRAS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("pixpin-muestras-figuras"));
    std::fs::create_dir_all(&salida).unwrap();

    let galeria = |noche: bool, destino: Option<Destino>| {
        let mut g = Galeria {
            noche,
            boton: pindesk_ui::dia_noche::DiaNoche::nuevo(noche),
            destino,
            ..Galeria::default()
        };
        g.poner_figuras(&serie, None);
        g.preparar();
        g
    };
    let dia = galeria(false, None);
    let mut noche = galeria(true, None);
    // De noche, con una categoria elegida y algo buscado.
    noche.filtro = noche.grupos().get(1).map(|(g, _)| g.clone());
    let mut eligiendo = galeria(
        false,
        Some(Destino {
            proyecto: "obra".into(),
            chat: "Casa Lima".into(),
        }),
    );
    eligiendo.consulta = "diario".into();
    // El raton encima del ☀/☾.
    eligiendo.raton = Punto {
        x: ancho as i32 - 30,
        y: 32,
    };
    // Una plantilla elegida con sus parametros: del 1 al 40, de 10 en 10.
    let con_parametros = |noche: bool, id: &str, cambiar: &dyn Fn(&mut Parametros)| {
        let mut g = galeria(
            noche,
            Some(Destino {
                proyecto: "obra".into(),
                chat: "Casa Lima".into(),
            }),
        );
        g.elegida = g.figuras.iter().position(|f| f.id == id);
        assert!(g.elegida.is_some(), "{id}");
        g.preparar();
        if let Some(p) = g.parametros.as_mut() {
            cambiar(p);
        }
        g.preparar();
        g
    };
    let lista = |p: &mut Parametros| {
        p.poner(Parametro::Hasta, 40);
        p.poner(Parametro::PorHoja, 10);
    };
    let al_lado = con_parametros(false, "serie-lista-de-tareas", &lista);
    let al_lado_noche = con_parametros(true, "serie-lista-de-tareas-noche", &lista);
    let mut acercada = con_parametros(false, "serie-cronograma", &|p| {
        p.poner(Parametro::Hasta, 24);
        p.poner(Parametro::SemanasHasta, 16);
    });
    acercada.encuadre.aumento = 2.5;
    // Estrecha: la figura sola, como un chat abierto.
    let sola = |noche: bool, id: &str| {
        let mut g = con_parametros(noche, id, &|p| {
            p.paso(Parametro::Mes, 11);
            p.poner(Parametro::Meses, 3);
        });
        g.viendo = true;
        g
    };
    let sola_dia = sola(false, "serie-habitos");
    let sola_noche = sola(true, "serie-planificador-mensual-noche");
    let escenas: Vec<(&str, &Tema, &Galeria, (u32, u32))> = vec![
        ("figuras-dia", &CLARO, &dia, (ancho, alto)),
        ("figuras-noche", &OSCURO, &noche, (ancho, alto)),
        ("figuras-eligiendo", &CLARO, &eligiendo, (ancho, alto)),
        ("figuras-parametros-dia", &CLARO, &al_lado, (ancho, alto)),
        (
            "figuras-parametros-noche",
            &OSCURO,
            &al_lado_noche,
            (ancho, alto),
        ),
        ("figuras-acercada", &CLARO, &acercada, (ancho, alto)),
        (
            "figuras-estrecha-dia",
            &CLARO,
            &sola_dia,
            (estrecha, alto_estrecha),
        ),
        (
            "figuras-estrecha-noche",
            &OSCURO,
            &sola_noche,
            (estrecha, alto_estrecha),
        ),
    ];
    for (nombre, tema, g, (ancho, alto)) in escenas {
        let destino = FueraDePantalla::nuevo(&motor, d3.d3d(), ancho, alto).expect("destino");
        motor
            .dibujar(&destino.destino, |p| {
                pintar_todo(p, g, ancho, alto, tema, escala, &textos);
            })
            .expect("fotograma");
        destino.esperar_gpu().expect("GPU");
        let (w, h, pixeles) = destino.leer_rgba().expect("leer");
        let png = pindesk_codec::imagen::codificar_png(&pindesk_codec::imagen::ImagenRgba {
            ancho: w,
            alto: h,
            pixeles,
        })
        .expect("codificar");
        let ruta = salida.join(format!("{nombre}.png"));
        std::fs::write(&ruta, png).unwrap();
        println!("{nombre}: {}", ruta.display());
    }
}
