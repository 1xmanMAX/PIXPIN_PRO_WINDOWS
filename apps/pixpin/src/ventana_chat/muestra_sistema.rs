//! **Los mensajes de sistema no ensucian el chat «Asistente»** (14-oct):
//! las letras, las figuras y las automatizaciones (`auto-*`) siguen en el
//! cuaderno y viajan, pero no son burbujas: van plegadas en una franja
//! arriba que al pulsarla las lista. Y la muestra en PNG:
//!
//! ```text
//! cargo test -p pixpin --bin pixpinmax muestra_de_la_franja_de_sistema -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Deja los PNG en `%TEMP%/pixpin-muestras-sistema` (o en `PIXPIN_MUESTRAS`).

use pixpin_geom::Rect;
use pixpin_proyecto::almacen::{self, Ficha};
use pixpin_proyecto::cuaderno::{self, Clase, Mensaje, Sello};
use pixpin_render::MotorRender;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_store::{Catalogo, Idioma, Ubicacion};

use super::{
    Abierto, Accion, CLARO, Lista, OSCURO, Pinta, Tema, abrir_proyecto, disponer, escribir_nota,
    hay_barra, indices_visibles, linea_de_sistema, menu_de_sistema, papel_de, pintar,
    pintar_historial, pintar_redaccion,
};

/// Un chat «Asistente» con una letra (de verdad: `letras_que_viajan`), las
/// figuras, las automatizaciones y dos notas normales, una que empieza por
/// «auto-».
fn montar(nombre: &str) -> (std::path::PathBuf, Ubicacion, Ficha) {
    let raiz = std::env::temp_dir().join(format!("pixpin-sistema-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(&raiz).unwrap();
    let u = Ubicacion::Portable { raiz: raiz.clone() };
    let ficha = almacen::asegurar_asistente(&raiz, 1, "PC01").unwrap();
    let carpeta = almacen::carpeta(&raiz, &ficha.id);
    std::fs::create_dir_all(&carpeta).unwrap();
    let cuando = pixpin_shell::entorno::ahora_utc_ms() - 60_000;
    escribir_nota(
        &raiz,
        &ficha.id,
        "PC01",
        "Dibuja el plano de la obra",
        cuando,
        1,
        None,
    )
    .unwrap();
    let letras = crate::letras_que_viajan::carpeta_en(&raiz);
    std::fs::create_dir_all(&letras).unwrap();
    std::fs::write(
        letras.join("romans.shx"),
        include_bytes!("../../../../crates/pixpin-cad/fuentes-shx/romans.shx"),
    )
    .unwrap();
    std::fs::write(
        letras.join("greeks.shx"),
        include_bytes!("../../../../crates/pixpin-cad/fuentes-shx/greeks.shx"),
    )
    .unwrap();
    assert_eq!(
        crate::letras_que_viajan::senalar(&raiz, "PC01", cuando + 1000).unwrap(),
        2
    );
    for (n, (id, fichero, ruta)) in [
        (
            crate::biblioteca_figuras::ID_MENSAJE,
            "figuras.json",
            crate::biblioteca_figuras::RUTA_PORTATIL,
        ),
        (
            crate::automatizaciones::ID_MENSAJE,
            "automatizaciones.json",
            crate::automatizaciones::RUTA_PORTATIL,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut m = Mensaje::adjunto(
            Clase::Archivo,
            fichero,
            ruta,
            10,
            &Sello {
                cuando: cuando + 2000 + n as i64,
                numero: 10 + n as i64,
                aparato: "PC01".into(),
                proyecto: ficha.id.clone(),
            },
        );
        m.id = id.into();
        cuaderno::anadir(&carpeta, &m).unwrap();
    }
    escribir_nota(
        &raiz,
        &ficha.id,
        "PC01",
        "auto-guardado: la nota del usuario se ve",
        cuando + 5000,
        20,
        None,
    )
    .unwrap();
    (raiz, u, ficha)
}

#[test]
fn los_mensajes_de_sistema_van_en_la_franja_y_no_como_burbujas() {
    let (raiz, u, ficha) = montar("chat");
    let textos = Catalogo::nuevo(Idioma::Espanol);
    let a = abrir_proyecto(&u, &ficha);
    assert_eq!(a.mensajes.len(), 6, "siguen en el cuaderno");
    assert_eq!(
        crate::pdf_en_chat::cuenta_de_la_cabecera(&a.mensajes).0,
        2,
        "la cabecera no los cuenta"
    );
    let vistos: Vec<String> = indices_visibles(&a)
        .into_iter()
        .map(|i| a.mensajes[i].texto.clone())
        .collect();
    assert_eq!(vistos.len(), 2, "{vistos:?}");
    // Caso negativo: una nota cuyo TEXTO empieza por «auto-» si se ve.
    assert!(vistos.iter().any(|t| t.starts_with("auto-guardado")));
    let linea = linea_de_sistema(&a, &textos).expect("franja");
    assert!(
        linea.contains("Letras de planos: 2")
            && linea.contains("Figuras")
            && linea.contains("Automatizaciones"),
        "{linea}"
    );
    assert!(hay_barra(&a));
    // La lista: quitar cada letra borra su mensaje (como la burbuja antes);
    // las figuras y las automatizaciones dicen donde se usan.
    let menu = menu_de_sistema(&a, &textos);
    assert_eq!(menu.len(), 4);
    let quitar: Vec<_> = menu.iter().filter(|e| e.peligro).collect();
    assert_eq!(quitar.len(), 2);
    for e in &quitar {
        let Accion::Borrar(v) = &e.accion else {
            panic!("{:?}", e.accion)
        };
        assert!(a.mensajes[v[0]].id.starts_with("auto-letra-"));
        assert!(e.texto.contains(".shx"));
    }
    assert!(menu.iter().any(|e| e.accion == Accion::Ajustes));
    // En otro chat no hay franja aunque hubiera un `auto-` (solo nacen en el
    // del asistente), y sin ninguno tampoco.
    let otra = almacen::Ficha::nueva("Obra", 2, "PC01");
    let mut ind = almacen::Indice::leer(&raiz);
    ind.proyectos.push(otra.clone());
    ind.guardar(&raiz).unwrap();
    escribir_nota(&raiz, &otra.id, "PC01", "hola", 5, 1, None).unwrap();
    let b = abrir_proyecto(&u, &otra);
    assert!(linea_de_sistema(&b, &textos).is_none());
    let _ = std::fs::remove_dir_all(&raiz);
}

/// Pinta el chat abierto `a` a PNG.
fn a_png(a: &Abierto, ficha: &Ficha, tema: &Tema, claro: bool, nombre: &str) -> std::path::PathBuf {
    let textos = Catalogo::nuevo(Idioma::Espanol);
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let d3 = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d3.d3d()).expect("motor");
    let (ancho, alto) = (900u32, 560u32);
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
        .unwrap_or_else(|| std::env::temp_dir().join("pixpin-muestras-sistema"));
    std::fs::create_dir_all(&salida).unwrap();
    let ruta = salida.join(format!("{nombre}.png"));
    std::fs::write(&ruta, png).unwrap();
    println!("{nombre}: {}", ruta.display());
    ruta
}

#[test]
#[ignore = "necesita GPU; genera PNG para mirarlos"]
fn muestra_de_la_franja_de_sistema() {
    let (raiz, u, ficha) = montar("png");
    let a = abrir_proyecto(&u, &ficha);
    assert!(a_png(&a, &ficha, &CLARO, true, "asistente-franja-sistema").is_file());
    a_png(&a, &ficha, &OSCURO, false, "asistente-franja-sistema-noche");
    let _ = std::fs::remove_dir_all(&raiz);
}
