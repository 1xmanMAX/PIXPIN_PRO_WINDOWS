//! **Muestra en PNG de «Pensando…» en la burbuja del asistente** (13-oct):
//! la respuesta mientras piensa (abierto, escribiendose), ya pensada
//! («Penso 12 s», plegado) y abierta con un clic, con la peticion mejorada
//! encima del texto. Pintado con las mismas funciones que la ventana y la
//! GPU de verdad.
//!
//! ```text
//! cargo test -p pixpin --bin pixpinmax muestra_de_pensando -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Deja los PNG en `%TEMP%/pixpin-muestras-pensado` (o en `PIXPIN_MUESTRAS`).

use pixpin_geom::Rect;
use pixpin_ia::hoja_trabajo::Registro;
use pixpin_ia::pensar::Plan;
use pixpin_proyecto::almacen::Ficha;
use pixpin_render::MotorRender;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_store::{Catalogo, Idioma, Ubicacion};

use super::{
    Abierto, CLARO, Lista, OSCURO, Pinta, Tema, abrir_proyecto, disponer, escribir_nota, papel_de,
    pintar, pintar_historial, pintar_redaccion,
};
use crate::asistente::pensado::{self, Seccion};

const PENSADO: &str = "El usuario quiere una casa roja con tres ventanas. En la hoja de trabajo ya hay una casa dibujada \
     en el turno anterior, asi que «ponle» se refiere a esa casa. Tengo que anadir tres rectangulos pequenos dentro \
     del cuerpo de la casa, repartidos a lo ancho, y mantener el rojo que decidio. Usare dibujar_en_lienzo con tres \
     rectangulos de 60x50 en y=330.";

fn plan() -> Plan {
    Plan {
        entiendo: "Anadir tres ventanas a la casa roja que ya esta en el lienzo.".into(),
        hare: "dibujar tres rectangulos dentro de la casa, repartidos a lo ancho.".into(),
        herramientas: vec!["dibujar_en_lienzo".into()],
        datos: vec!["3 ventanas".into(), "color rojo".into()],
        decidido: Vec::new(),
        del_modelo: true,
    }
}

/// Pinta el chat abierto `a` a PNG.
fn a_png(a: &Abierto, ficha: &Ficha, tema: &Tema, claro: bool, nombre: &str) -> std::path::PathBuf {
    let textos = Catalogo::nuevo(Idioma::Espanol);
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let d3 = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d3.d3d()).expect("motor");
    let (ancho, alto) = (900u32, 760u32);
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
        Some(a),
    );
    let miniaturas = crate::miniaturas::Miniaturas::nuevo();
    let previas = crate::miniaturas::Miniaturas::con_lado(super::PREVIA_LADO);
    let fichas = vec![ficha.clone()];
    let orden = vec![0usize];
    let marcados = Default::default();
    a.colocado.borrow_mut().ancho = 0;
    for _ in 0..2 {
        let lista = Lista {
            fichas: &fichas,
            orden: &orden,
            scroll: 0,
            sobre: None,
            elegida: Some(0),
            ahora,
            textos: &textos,
            busqueda: &crate::caja_texto::CajaLinea::nueva(),
            buscando: false,
            marcados: &marcados,
            raiz: std::path::Path::new(""),
            giro_sincro: 0.0,
            sincro_fallo: false,
            en_chats: None,
            sobre_mensaje: None,
        };
        motor
            .dibujar(&destino.destino, |p| {
                pintar(
                    p,
                    &d,
                    tema,
                    papel_de(0, claro),
                    escala,
                    &textos,
                    None,
                    &lista,
                );
                a.zonas.borrow_mut().clear();
                let c = Pinta {
                    tema,
                    escala,
                    textos: &textos,
                    ahora,
                    miniaturas: &miniaturas,
                    previas: &previas,
                };
                let alto_texto = (pixpin_ui::chat::REDACCION_TAM * 1.3).ceil() as u32;
                a.alto_caja.set(alto_texto);
                pintar_historial(p, &d, &c, a);
                pintar_redaccion(p, &d, &c, a, alto_texto);
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
    let salida = std::env::var_os("PIXPIN_MUESTRAS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("pixpin-muestras-pensado"));
    std::fs::create_dir_all(&salida).unwrap();
    let ruta = salida.join(format!("{nombre}.png"));
    std::fs::write(&ruta, png).unwrap();
    println!("{nombre}: {}", ruta.display());
    ruta
}

#[test]
#[ignore = "necesita GPU; genera PNG para mirarlos"]
fn muestra_de_pensando() {
    pensado::olvidar_para_pruebas();
    let raiz = std::env::temp_dir().join(format!("pixpin-muestra-pensado-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(&raiz).unwrap();
    let u = Ubicacion::Portable { raiz: raiz.clone() };
    let ficha = pixpin_proyecto::almacen::asegurar_asistente(&raiz, 1, "PC").unwrap();
    std::fs::create_dir_all(pixpin_proyecto::almacen::carpeta(&raiz, &ficha.id)).unwrap();
    let cuando = pixpin_shell::entorno::ahora_utc_ms() - 60_000;
    let id = ficha.id.clone();
    escribir_nota(&raiz, &id, "K7Q2", "Dibuja una casa roja", cuando, 1, None).unwrap();
    let r1 = escribir_nota(
        &raiz,
        &id,
        "IA",
        "Listo: una casa roja en el lienzo «Asistente».",
        cuando + 1000,
        2,
        None,
    )
    .unwrap();
    escribir_nota(
        &raiz,
        &id,
        "K7Q2",
        "Ahora ponle 3 ventanas",
        cuando + 20_000,
        3,
        None,
    )
    .unwrap();
    let r2 = escribir_nota(&raiz, &id, "IA", "💭 …", cuando + 21_000, 4, None).unwrap();
    // La primera ya pensada y guardada con su nota.
    pensado::guardar(
        &raiz,
        &id,
        &r1.id,
        &r1.codigo_unico(),
        Registro {
            pensamiento:
                "Quiere una casa. Un rectangulo rojo para el cuerpo y un triangulo para el tejado."
                    .into(),
            segundos: Some(4.2),
            plan: Some(Plan {
                entiendo: "Dibujar una casa roja en el lienzo.".into(),
                ..plan()
            }),
            ..Default::default()
        },
    );
    // La segunda, pensando ahora mismo.
    pensado::poner_en_vivo(
        &r2.id,
        Seccion {
            pensamiento: PENSADO.into(),
            pensando: true,
            ..Default::default()
        },
    );
    let mut a = abrir_proyecto(&u, &ficha);
    let abierto = a_png(&a, &ficha, &CLARO, true, "pensando-abierto");
    assert!(abierto.is_file());

    // Acabo: se pliega solo («Penso 12 s») y arriba va la peticion mejorada.
    pensado::guardar(
        &raiz,
        &id,
        &r2.id,
        &r2.codigo_unico(),
        Registro {
            pensamiento: PENSADO.into(),
            segundos: Some(12.4),
            plan: Some(plan()),
            reinicio: true,
            ..Default::default()
        },
    );
    if let Some(m) = a.mensajes.iter_mut().find(|m| m.id == r2.id) {
        m.texto = "Hecho: tres ventanas en la casa.".into();
    }
    a_png(&a, &ficha, &CLARO, true, "pensando-plegado");

    // Un clic en «Penso 12 s» y otro en la peticion: abiertos, de noche.
    super::pensado::alternar(&mut a.desplegados, &r2.id, true);
    super::pensado::alternar(&mut a.desplegados, &r2.id, false);
    a_png(&a, &ficha, &OSCURO, false, "pensando-desplegado-noche");
    let _ = std::fs::remove_dir_all(&raiz);
}
