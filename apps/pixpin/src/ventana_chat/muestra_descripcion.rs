//! **Muestra en PNG de una foto con su descripcion** (3-oct): la foto con el
//! texto debajo, dentro de su burbuja, y la barra de «Añadir descripción»
//! abierta sobre otra. Pintado con las mismas funciones que la ventana y la
//! GPU de verdad.
//!
//! ```text
//! cargo test -p pixpin --bin pixpinmax muestra_de_la_foto_con_descripcion -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Deja el PNG en `%TEMP%/pixpin-muestras-descripcion`.

use pixpin_geom::Rect;
use pixpin_proyecto::almacen::{Ficha, Indice};
use pixpin_render::MotorRender;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_store::{Catalogo, Idioma, Ubicacion};

use super::{
    CLARO, Lista, Pinta, abrir_proyecto, disponer, fotos_del_historial, meter_ficheros, papel_de, pintar,
    pintar_historial, pintar_redaccion,
};

/// Un plano de juguete: papel claro, dos muros y una cota roja.
fn foto(ruta: &std::path::Path, tono: u8) {
    let (w, h) = (480u32, 300u32);
    let mut px = vec![255u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let muro = (40..46).contains(&y) || (30..36).contains(&x);
            let cota = (240..243).contains(&y) && (60..420).contains(&x);
            let c = if muro {
                [40, 40, 40]
            } else if cota {
                [210, 40, 40]
            } else {
                [tono, 235, 245]
            };
            px[i..i + 3].copy_from_slice(&c);
        }
    }
    let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::ImagenRgba {
        ancho: w,
        alto: h,
        pixeles: px,
    })
    .unwrap();
    std::fs::write(ruta, png).unwrap();
}

#[test]
#[ignore = "necesita GPU; genera PNG para mirarlos"]
fn muestra_de_la_foto_con_descripcion() {
    let raiz = std::env::temp_dir().join(format!("pixpin-muestra-descripcion-{}", std::process::id()));
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
    std::fs::create_dir_all(pixpin_proyecto::almacen::carpeta(&raiz, "casa")).unwrap();
    let (uno, dos) = (raiz.join("planta.png"), raiz.join("alzado.png"));
    foto(&uno, 230);
    foto(&dos, 200);
    let textos = Catalogo::nuevo(Idioma::Espanol);
    let mut a = abrir_proyecto(&u, &ficha);
    // Como lo deja el cuadro de confirmar con algo escrito en la caja.
    meter_ficheros(
        &u,
        &mut a,
        "PC",
        std::slice::from_ref(&uno),
        "La pared norte se mueve 20 cm hacia el patio; la cota roja es la nueva.",
    );
    meter_ficheros(&u, &mut a, "PC", std::slice::from_ref(&dos), "");
    // Y la barra de describir abierta sobre la segunda, a medio escribir
    // (lo que deja `Accion::Describir`).
    a.renombrando_mensaje = Some((1, "Alzado sur".into()));
    a.describiendo = true;

    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let d3 = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d3.d3d()).expect("motor");
    let (ancho, alto) = (1024u32, 900u32);
    let destino = FueraDePantalla::nuevo(&motor, d3.d3d(), ancho, alto).expect("destino");
    let escala = 100;
    let marco = Rect { x: 0, y: 0, ancho, alto };
    let d = disponer(marco, escala, pixpin_ui::chat::ancho_inicial(ancho, escala), Some(&a));
    let miniaturas = crate::miniaturas::Miniaturas::nuevo();
    let mut previas = crate::miniaturas::Miniaturas::con_lado(super::PREVIA_LADO);
    let fichas = vec![ficha.clone()];
    let orden = vec![0usize];
    let marcados = Default::default();
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
                pintar(p, &d, &CLARO, papel_de(0, true), escala, &textos, None, &lista);
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
    let salida = std::env::temp_dir().join("pixpin-muestras-descripcion");
    std::fs::create_dir_all(&salida).unwrap();
    let ruta = salida.join("foto-con-descripcion.png");
    std::fs::write(&ruta, png).unwrap();
    println!("foto-con-descripcion: {}", ruta.display());
    let _ = std::fs::remove_dir_all(&raiz);
}
