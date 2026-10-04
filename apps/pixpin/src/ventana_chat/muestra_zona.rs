//! **Muestra en PNG del chat con una zona recien mandada** (F8): el lienzo
//! «Planta» y, debajo, la foto de su zona con la tarjeta «Viene de», pintados
//! con las mismas funciones que la ventana y la GPU de verdad.
//!
//! ```text
//! cargo test -p pixpin --bin pixpinmax muestra_del_chat_con_la_zona -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Deja el PNG en `target/muestras-zona`.

use std::path::Path;

use pixpin_geom::Rect;
use pixpin_proyecto::almacen::{self, Ficha, Indice};
use pixpin_render::MotorRender;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_store::{Catalogo, Idioma, Ubicacion};

use super::{
    CLARO, Lista, Pinta, abrir_proyecto, disponer, fotos_del_historial, papel_de, pintar,
    pintar_historial, pintar_redaccion,
};

/// La foto de una zona: un trozo de plano con dos muros y una cota roja.
fn foto_de_ejemplo() -> pixpin_codec::ImagenRgba {
    let (w, h) = (480u32, 300u32);
    let mut px = vec![255u8; (w * h * 4) as usize];
    let mut raya = |x0: u32, y0: u32, x1: u32, y1: u32, c: [u8; 3]| {
        for y in y0..=y1.min(h - 1) {
            for x in x0..=x1.min(w - 1) {
                let i = ((y * w + x) * 4) as usize;
                px[i..i + 3].copy_from_slice(&c);
            }
        }
    };
    raya(30, 40, 450, 46, [40, 40, 40]);
    raya(30, 40, 36, 260, [40, 40, 40]);
    raya(240, 40, 244, 200, [90, 90, 90]);
    raya(60, 240, 420, 243, [210, 40, 40]);
    pixpin_codec::ImagenRgba {
        ancho: w,
        alto: h,
        pixeles: px,
    }
}

#[test]
#[ignore = "necesita GPU; genera PNG para mirarlos"]
fn muestra_del_chat_con_la_zona() {
    let raiz = std::env::temp_dir().join(format!("pixpin-muestra-zona-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(&raiz).unwrap();
    let u = Ubicacion::Portable { raiz: raiz.clone() };
    let ficha = Ficha {
        id: "casa".into(),
        nombre: "Casa Lima".into(),
        tocado: 1,
        ..Default::default()
    };
    Indice {
        proyectos: vec![ficha.clone()],
        ..Default::default()
    }
    .guardar(&raiz)
    .unwrap();
    // El lienzo de origen, como mensaje del chat.
    let carpeta = almacen::carpeta(&raiz, "casa");
    std::fs::create_dir_all(carpeta.join("lienzos")).unwrap();
    std::fs::write(
        almacen::lienzo(&raiz, "casa", "dib-planta"),
        pixpin_motor2d::excalidraw::escribir(&pixpin_motor2d::excalidraw::Lienzo::vacio()),
    )
    .unwrap();
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let mut planta = pixpin_proyecto::cuaderno::Mensaje::adjunto(
        pixpin_proyecto::cuaderno::Clase::Dibujo,
        "Planta",
        "lienzos/dib-planta.excalidraw",
        10,
        &pixpin_proyecto::cuaderno::Sello {
            cuando: ahora - 60_000,
            numero: 1,
            aparato: "K7Q2".into(),
            proyecto: "casa".into(),
        },
    );
    planta.referencia = Some("dib-planta".into());
    pixpin_proyecto::cuaderno::anadir(&carpeta, &planta).unwrap();
    // La zona, mandada como la manda el editor.
    let textos = Catalogo::nuevo(Idioma::Espanol);
    let h = crate::zona_al_chat::HojaAbierta::de(
        &raiz,
        "casa",
        &almacen::lienzo(&raiz, "casa", "dib-planta"),
        &planta,
    );
    let mut escena = pixpin_motor2d::Escena::nueva();
    let dicho = crate::zona_al_chat::mandar_y_marcar(
        &mut escena,
        &foto_de_ejemplo(),
        (0.0, 0.0, 480.0, 300.0),
        &h,
        &textos,
    );
    println!("  {dicho:?}");

    let a = abrir_proyecto(&u, &ficha);
    let d3 = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d3.d3d()).expect("motor");
    let (ancho, alto) = (1024u32, 720u32);
    let destino = FueraDePantalla::nuevo(&motor, d3.d3d(), ancho, alto).expect("destino");
    let escala = 100;
    let marco = Rect {
        x: 0,
        y: 0,
        ancho,
        alto,
    };
    let d = disponer(
        marco,
        escala,
        pixpin_ui::chat::ancho_inicial(ancho, escala),
        Some(&a),
    );
    let miniaturas = crate::miniaturas::Miniaturas::nuevo();
    let mut previas = crate::miniaturas::Miniaturas::con_lado(super::PREVIA_LADO);
    let fichas = vec![ficha.clone()];
    let orden = vec![0usize];
    let marcados = Default::default();
    // Unas vueltas: la primera coloca las burbujas, las siguientes suben la
    // foto (va de cuatro en cuatro) y la pintan.
    for _ in 0..4 {
        let rutas = fotos_del_historial(&a, &d, escala);
        if !rutas.is_empty() {
            previas.asegurar(&rutas, &motor);
        }
        let lista = Lista {
            fichas: &fichas,
            orden: &orden,
            scroll: 0,
            sobre: None,
            elegida: Some(0),
            ahora,
            textos: &textos,
            busqueda: "",
            marcados: &marcados,
        };
        motor
            .dibujar(&destino.destino, |p| {
                pintar(
                    p,
                    &d,
                    &CLARO,
                    papel_de(0, true),
                    escala,
                    &textos,
                    None,
                    &lista,
                );
                a.zonas.borrow_mut().clear();
                let c = Pinta {
                    tema: &CLARO,
                    escala,
                    textos: &textos,
                    ahora,
                    miniaturas: &miniaturas,
                    previas: &previas,
                };
                let alto_texto = (pixpin_ui::chat::REDACCION_TAM * 1.3).ceil() as u32;
                a.alto_caja.set(alto_texto);
                pintar_historial(p, &d, &c, &a);
                pintar_redaccion(p, &d, &c, &a, alto_texto);
            })
            .expect("fotograma");
        destino.esperar_gpu().expect("GPU");
    }
    let (w, hh, pixeles) = destino.leer_rgba().expect("leer");
    let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
        ancho: w,
        alto: hh,
        pixeles,
    })
    .expect("codificar");
    let salida = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/muestras-zona");
    std::fs::create_dir_all(&salida).unwrap();
    let ruta = salida.join("chat-con-la-zona.png");
    std::fs::write(&ruta, png).unwrap();
    println!("chat-con-la-zona: {}", ruta.display());
    let _ = std::fs::remove_dir_all(&raiz);
}
