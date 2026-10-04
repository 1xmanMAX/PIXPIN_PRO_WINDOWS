//! **Muestras en PNG de los menus v2** (v2-menus): el menu de un mensaje
//! con «Más» abierto, la hoja del «+» y el cuadro de enviar fotos con una
//! arrastrandose. Pintado con las mismas funciones que la ventana y la GPU
//! de verdad, sin abrir ninguna ventana.
//!
//! ```text
//! cargo test -p pixpin --bin pixpinmax muestras_de_los_menus_v2 -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Deja los PNG en la carpeta de `PIXPIN_MUESTRAS` o, si no, en
//! `%TEMP%/pixpin-muestras-menus`.

use pixpin_geom::{Punto, Rect};
use pixpin_proyecto::almacen::{Ficha, Indice};
use pixpin_render::MotorRender;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_store::{Catalogo, Idioma, Ubicacion};

use super::*;

fn foto(ruta: &std::path::Path, tono: [u8; 3]) {
    let (w, h) = (320u32, 220u32);
    let mut px = vec![255u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let k = (x + y) as f32 / (w + h) as f32;
            px[i] = (tono[0] as f32 * (0.6 + 0.4 * k)) as u8;
            px[i + 1] = (tono[1] as f32 * (0.6 + 0.4 * k)) as u8;
            px[i + 2] = (tono[2] as f32 * (0.6 + 0.4 * k)) as u8;
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
fn muestras_de_los_menus_v2() {
    let raiz = std::env::temp_dir().join(format!("pixpin-muestra-menus-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(&raiz).unwrap();
    let u = Ubicacion::Portable { raiz: raiz.clone() };
    let ficha = Ficha {
        id: "obra".into(),
        nombre: "Obra Miraflores".into(),
        tocado: 1,
        ..Default::default()
    };
    Indice {
        proyectos: vec![ficha.clone()],
        ..Default::default()
    }
    .guardar(&raiz)
    .unwrap();
    std::fs::create_dir_all(pixpin_proyecto::almacen::carpeta(&raiz, "obra")).unwrap();
    let fotos: Vec<std::path::PathBuf> = [[61u8, 92, 128], [122, 74, 42], [74, 90, 58]]
        .iter()
        .enumerate()
        .map(|(n, t)| {
            let r = raiz.join(format!("foto-{n}.png"));
            foto(&r, *t);
            r
        })
        .collect();
    let textos = Catalogo::nuevo(Idioma::Espanol);
    let mut a = abrir_proyecto(&u, &ficha);
    meter_ficheros(
        &u,
        &mut a,
        "PC",
        std::slice::from_ref(&fotos[0]),
        "Grieta en el muro norte",
    );
    let a = abrir_proyecto(&u, &ficha);

    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let d3 = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d3.d3d()).expect("motor");
    let (ancho, alto) = (1180u32, 860u32);
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
    let mut miniaturas = crate::miniaturas::Miniaturas::nuevo();
    let mut previas = crate::miniaturas::Miniaturas::con_lado(super::PREVIA_LADO);
    miniaturas.asegurar(&fotos, &motor);
    let fichas = vec![ficha.clone()];
    let orden = vec![0usize];
    let marcados = Default::default();
    let salida = std::env::var_os("PIXPIN_MUESTRAS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("pixpin-muestras-menus"));
    std::fs::create_dir_all(&salida).unwrap();

    // Tres escenas: (nombre, menu, pendientes).
    let mut menu_msg =
        MenuAbierto::nuevo(Punto { x: 520, y: 140 }, menu_de_mensaje(&a, 0, &textos))
            .con_etiquetas(0, Some(pixpin_ui::chat::ETIQUETAS[1].to_string()));
    menu_msg.mas_abierto = true;
    menu_msg.elegido = Some(menu_v2::Sitio::Mas(0));
    menu_msg.sobre = Some(menu_v2::Sitio::Principal(1));
    let alto_texto = (pixpin_ui::chat::REDACCION_TAM * 1.3).ceil() as u32;
    let clip = d.boton_adjuntar(alto_texto, escala);
    let mut hoja = hoja_adjuntar::Hoja::nueva(&textos);
    hoja.sobre = Some(2);
    let menu_hoja = MenuAbierto::de_hoja(
        Punto {
            x: clip.derecha(),
            y: clip.y,
        },
        hoja,
    );
    let mut hoja_buscando = hoja_adjuntar::Hoja::nueva(&textos);
    hoja_buscando.busqueda = "cro".into();
    let menu_hoja_buscando = MenuAbierto::de_hoja(
        Punto {
            x: clip.derecha(),
            y: clip.y,
        },
        hoja_buscando,
    );
    let mut pend = Pendientes::de(fotos.clone()).unwrap();
    pend.pie = "Grieta en el muro norte, revisar el lunes".into();
    let escenas: Vec<(&str, Option<&MenuAbierto>, Option<&Pendientes>)> = vec![
        ("menu-mensaje", Some(&menu_msg), None),
        ("hoja-mas", Some(&menu_hoja), None),
        ("hoja-mas-buscando", Some(&menu_hoja_buscando), None),
        ("enviar-fotos", None, Some(&pend)),
    ];
    for (nombre, menu, pendientes) in escenas {
        let destino = FueraDePantalla::nuevo(&motor, d3.d3d(), ancho, alto).expect("destino");
        // La foto arrastrada: se mide primero el cuadro para saber donde.
        let mut pend_local = pendientes.map(|p| Pendientes {
            rutas: p.rutas.clone(),
            pie: p.pie.clone(),
            tamanos: p.tamanos.clone(),
            arrastre: None,
            sobre: None,
            botones: std::cell::Cell::new(p.botones.get()),
        });
        for vuelta in 0..4 {
            let rutas = fotos_del_historial(&a, &d, escala);
            if !rutas.is_empty() {
                previas.asegurar(&rutas, &motor);
            }
            if vuelta == 2
                && let Some(p) = pend_local.as_mut()
            {
                let c = p.cuadro(marco, escala);
                let f = c.fotos[2];
                p.arrastre = Some(envio::Arrastre {
                    desde: 2,
                    agarre: Punto {
                        x: f.x + 40,
                        y: f.y + 40,
                    },
                    ahora: Punto {
                        x: c.fotos[1].x + 10,
                        y: f.y + 34,
                    },
                });
                p.sobre = Some(envio::Sitio::Enviar);
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
                        &OSCURO,
                        papel_de(0, false),
                        escala,
                        &textos,
                        None,
                        &lista,
                    );
                    a.zonas.borrow_mut().clear();
                    let c = Pinta {
                        tema: &OSCURO,
                        escala,
                        textos: &textos,
                        ahora,
                        miniaturas: &miniaturas,
                        previas: &previas,
                    };
                    a.alto_caja.set(alto_texto);
                    pintar_historial(p, &d, &c, &a);
                    pintar_redaccion(p, &d, &c, &a, alto_texto);
                    if let Some(pd) = pend_local.as_ref() {
                        envio::pintar(p, &c, pd, marco, &a.ficha.nombre);
                    }
                    if let Some(m) = menu {
                        pintar_menu_abierto(p, &c, m, marco);
                    }
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
        let ruta = salida.join(format!("{nombre}.png"));
        std::fs::write(&ruta, png).unwrap();
        println!("{nombre}: {}", ruta.display());
    }
    let _ = std::fs::remove_dir_all(&raiz);
}
