//! **La vista previa de un lienzo se ve como el lienzo** (queja del
//! 27-sep-2026: «en el canvas se nota con la letra blanca y todo normal, en
//! la previsualizacion lo blanco pasa a ser negro haciendo que no se note
//! nada»).
//!
//! La causa: sobre papel de noche el editor pinta la tinta adaptada
//! (`dibujo::tema`: la tinta negra guardada se ve blanca) y la vista previa
//! la pintaba tal cual, negra sobre el papel negro. Y el blanco de fabrica se
//! cambiaba por el gris del tema oscuro. Ahora las dos salen de la misma
//! regla (`tema::ordenes_como_en_el_lienzo`) y del mismo papel
//! (`excalidraw::fondo`), como `DrawExport.aBitmap` del movil.
//!
//! Muestra lado a lado (editor | burbuja | tarjeta de Proyectos):
//!
//! ```text
//! cargo test -p pixpin --bin pixpinmax muestra_del_lienzo_y_su_vista_previa -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Deja los PNG en `target/muestras-papel-de-la-vista`.

use std::path::{Path, PathBuf};

use pixpin_motor2d::{ColorRgba, Elemento, Escena, Figura, Orden, Punto2};
use pixpin_proyecto::cuaderno::{Clase, Mensaje, Sello};
use pixpin_store::Ubicacion;

use super::{LienzoVisto, Ojeada, leer_vista};

fn hex(rgb: u32) -> ColorRgba {
    ColorRgba::opaco(
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        (rgb & 0xff) as f32 / 255.0,
    )
}

fn texto(t: &str, x: f32, y: f32, trazo: ColorRgba) -> Elemento {
    Elemento {
        figura: Figura::Texto {
            texto: t.into(),
            tam: 48.0,
            familia: "Virgil".into(),
        },
        x,
        y,
        ancho: 60.0 * t.chars().count() as f32 * 0.5,
        alto: 60.0,
        trazo,
        ..Default::default()
    }
}

fn raya(x: f32, y: f32, trazo: ColorRgba) -> Elemento {
    let puntos: Vec<Punto2> = (0..30)
        .map(|i| Punto2::nuevo(i as f32 * 10.0, (i as f32 * 0.4).sin() * 20.0))
        .collect();
    Elemento {
        figura: Figura::Lapiz {
            puntos,
            presiones: Vec::new(),
            opciones: None,
        },
        x,
        y,
        ancho: 290.0,
        alto: 40.0,
        grosor: 4.0,
        trazo,
        ..Default::default()
    }
}

/// Un lienzo como los del usuario en la pizarra: letra negra guardada (que
/// el editor ensena blanca), letra blanca y una raya negra a mano.
fn escena_de(papel: ColorRgba) -> Escena {
    let mut e = Escena::nueva();
    e.fondo = papel;
    e.anadir(texto("Cocina", 40.0, 40.0, hex(0x1e1e1e)));
    e.anadir(texto("Blanca", 40.0, 140.0, hex(0xffffff)));
    e.anadir(raya(40.0, 260.0, hex(0x1e1e1e)));
    e
}

/// Un almacen con un proyecto y el lienzo `escena` en un mensaje de dibujo.
fn almacen_con(etiqueta: &str, escena: &Escena) -> (Ubicacion, PathBuf, Mensaje) {
    let raiz = std::env::temp_dir().join(format!(
        "pixpin-papel-vista-{etiqueta}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(pixpin_proyecto::almacen::carpeta(&raiz, "p").join("lienzos")).unwrap();
    let l = pixpin_motor2d::excalidraw::con_escena(
        &pixpin_motor2d::excalidraw::Lienzo::vacio(),
        escena,
    );
    std::fs::write(
        pixpin_proyecto::almacen::lienzo(&raiz, "p", "dib-1"),
        pixpin_motor2d::excalidraw::escribir(&l),
    )
    .unwrap();
    let mut m = Mensaje::adjunto(
        Clase::Dibujo,
        "Planta",
        "lienzos/dib-1.excalidraw",
        10,
        &Sello {
            cuando: 1_790_000_000_000,
            numero: 1,
            aparato: "K7Q2".into(),
            proyecto: String::new(),
        },
    );
    m.referencia = Some("dib-1".into());
    (Ubicacion::Portable { raiz: raiz.clone() }, raiz, m)
}

fn vista_de(u: &Ubicacion, m: &Mensaje) -> LienzoVisto {
    match leer_vista(u, "p", m) {
        Some(Ojeada::Lienzo(l)) => l,
        _ => panic!("el lienzo no se leyo como lienzo"),
    }
}

/// Los colores de los textos, en su orden.
fn colores_de_texto(ordenes: &[Orden]) -> Vec<ColorRgba> {
    ordenes
        .iter()
        .filter_map(|o| match o {
            Orden::Texto { color, .. } => Some(*color),
            _ => None,
        })
        .collect()
}

fn claro(c: ColorRgba) -> bool {
    c.r > 0.8 && c.g > 0.8 && c.b > 0.8
}

#[test]
fn la_vista_previa_de_un_lienzo_de_pizarra_tiene_su_papel_negro_y_la_letra_clara_como_el_editor() {
    let (u, raiz, m) = almacen_con("pizarra", &escena_de(hex(0x000000)));
    let v = vista_de(&u, &m);
    let p = v.papel.expect("con su papel");
    assert!(
        p.r < 0.01 && p.g < 0.01 && p.b < 0.01,
        "el papel negro del lienzo: {p:?}"
    );
    let textos = colores_de_texto(&v.ordenes);
    assert_eq!(textos.len(), 2);
    // La negra guardada sale clara, la misma que pinta el editor con ese
    // papel; y la blanca sigue blanca (no pasa a negra).
    let en_el_editor = crate::dibujo::tema::con_papel(Some(hex(0x000000)), || {
        crate::dibujo::tema::tinta(hex(0x1e1e1e))
    });
    assert_eq!(textos[0], en_el_editor);
    assert!(claro(textos[0]) && claro(textos[1]), "{textos:?}");
    // Y la raya a mano, tambien.
    for o in &v.ordenes {
        if let Orden::Tinta { color, .. } | Orden::Poligono { color, .. } = o {
            assert!(claro(*color), "{color:?}");
        }
    }
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn la_vista_previa_de_un_lienzo_blanco_sigue_blanca_con_la_tinta_negra() {
    // Caso negativo: en papel blanco nada cambia, y el papel es el blanco
    // del lienzo, no el gris del tema oscuro.
    let (u, raiz, m) = almacen_con("blanco", &escena_de(hex(0xffffff)));
    let v = vista_de(&u, &m);
    let p = v.papel.expect("con su papel");
    assert!(p.r > 0.999 && p.g > 0.999 && p.b > 0.999, "{p:?}");
    assert_eq!(super::papel_de_vista(&v, super::OSCURO.papel), p);
    let textos = colores_de_texto(&v.ordenes);
    assert_eq!(textos, vec![hex(0x1e1e1e), hex(0xffffff)]);
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn en_un_papel_oscuro_de_color_la_tinta_que_ya_se_lee_no_se_toca() {
    // Caso negativo: el amarillo sobre «Azul noche» ya se lee; no se cambia.
    let mut e = Escena::nueva();
    e.fondo = hex(0x14213d);
    e.anadir(texto("Ojo", 0.0, 0.0, hex(0xffd43b)));
    let (u, raiz, m) = almacen_con("azul", &e);
    let v = vista_de(&u, &m);
    assert_eq!(colores_de_texto(&v.ordenes), vec![hex(0xffd43b)]);
    let _ = std::fs::remove_dir_all(&raiz);
}

// --- Muestra con la GPU ----------------------------------------------------

fn carpeta() -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/muestras-papel-de-la-vista");
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Cuantos pixeles claros (los tres canales por encima de 200) hay.
fn claros(px: &[u8]) -> usize {
    px.chunks_exact(4)
        .filter(|p| p[0] > 200 && p[1] > 200 && p[2] > 200)
        .count()
}

/// Pega varias imagenes en fila, con un filo gris entre ellas.
fn en_fila(imgs: &[&pixpin_codec::ImagenRgba]) -> pixpin_codec::ImagenRgba {
    let alto = imgs.iter().map(|i| i.alto).max().unwrap_or(1);
    let ancho: u32 = imgs.iter().map(|i| i.ancho + 8).sum();
    let mut px = [128u8, 128, 128, 255].repeat((ancho * alto) as usize);
    let mut x0 = 0;
    for i in imgs {
        for y in 0..i.alto {
            let o = ((y * i.ancho) * 4) as usize;
            let d = ((y * ancho + x0) * 4) as usize;
            px[d..d + (i.ancho * 4) as usize]
                .copy_from_slice(&i.pixeles[o..o + (i.ancho * 4) as usize]);
        }
        x0 += i.ancho + 8;
    }
    pixpin_codec::ImagenRgba {
        ancho,
        alto,
        pixeles: px,
    }
}

/// El lienzo tal como lo pinta el editor: su camino de pintado
/// (`exportar::a_imagen` → `dibujar_orden`) con el papel fijado como lo fija
/// su bucle (`tema::fijar_papel(escena.fondo)`).
fn como_el_editor(escena: &Escena) -> pixpin_codec::ImagenRgba {
    let caja = escena.caja().expect("con dibujo");
    let caja = (caja.0 - 20.0, caja.1 - 20.0, caja.2 + 20.0, caja.3 + 20.0);
    let hoja = pixpin_motor2d::exportar::de_una_zona(escena, caja, None).expect("hoja");
    let sin_fotos = |_: u64| None;
    let lienzo = crate::ventana_editor::exportar::Lienzo {
        escena,
        seleccion: &[],
        papel: None,
        fotos: &sin_fotos,
        nombre: String::new(),
    };
    crate::dibujo::tema::con_papel(Some(escena.fondo), || {
        crate::ventana_editor::exportar::a_imagen(&hoja, 1.0, Some(escena.fondo), &lienzo)
    })
    .expect("el editor pinta")
}

/// La vista previa como la pinta la burbuja del chat (y la tarjeta de
/// Proyectos: `papel_de_vista` + `pintar_lienzo`) con el tema oscuro.
fn como_la_vista_previa(v: &LienzoVisto, (ancho, alto): (u32, u32)) -> pixpin_codec::ImagenRgba {
    let dispositivo = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let d3d = dispositivo.d3d().clone();
    let motor = pixpin_render::MotorRender::nuevo(&d3d).expect("motor");
    let destino =
        pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(&motor, &d3d, ancho, alto)
            .expect("destino");
    let hoja = pixpin_render::RectF {
        x: 0.0,
        y: 0.0,
        ancho: ancho as f32,
        alto: alto as f32,
    };
    motor
        .dibujar(&destino.destino, |p| {
            p.limpiar(super::OSCURO.chat);
            p.rellenar_redondeado(hoja, 6.0, super::papel_de_vista(v, super::OSCURO.papel));
            super::pintar_lienzo(p, v, hoja, None);
        })
        .expect("pintar");
    let (ancho, alto, pixeles) = destino.leer_rgba().expect("leer");
    pixpin_codec::ImagenRgba {
        ancho,
        alto,
        pixeles,
    }
}

#[test]
#[ignore = "necesita GPU; genera PNG para mirarlos"]
fn muestra_del_lienzo_y_su_vista_previa() {
    for (nombre, papel) in [
        ("pizarra", 0x000000),
        ("azul-noche", 0x14213d),
        ("blanco", 0xffffff),
    ] {
        let escena = escena_de(hex(papel));
        let editor = como_el_editor(&escena);
        let (u, raiz, m) = almacen_con(nombre, &escena);
        let v = vista_de(&u, &m);
        let previa = como_la_vista_previa(&v, (editor.ancho / 2, editor.alto / 2));
        let fila = en_fila(&[&editor, &previa]);
        let ruta = carpeta().join(format!("{nombre}.png"));
        std::fs::write(&ruta, pixpin_codec::codificar_png(&fila).unwrap()).unwrap();
        let (ce, cp) = (
            claros(&editor.pixeles) as f32 / (editor.ancho * editor.alto) as f32,
            claros(&previa.pixeles) as f32 / (previa.ancho * previa.alto) as f32,
        );
        println!(
            "{nombre}: {} | claros editor {:.3} previa {:.3}",
            ruta.display(),
            ce,
            cp
        );
        // El papel del centro de una esquina: el del lienzo en los dos.
        let esquina = |i: &pixpin_codec::ImagenRgba| {
            let k = ((10 * i.ancho + 10) * 4) as usize;
            [i.pixeles[k], i.pixeles[k + 1], i.pixeles[k + 2]]
        };
        let (pe, pp) = (esquina(&editor), esquina(&previa));
        for c in 0..3 {
            assert!(
                (pe[c] as i32 - pp[c] as i32).abs() <= 3,
                "{nombre}: papel {pe:?} vs {pp:?}"
            );
        }
        // Lo claro ocupa lo mismo (proporcion del area) en los dos: con la
        // letra negra en la previa, en la pizarra saldria casi nada.
        if papel != 0xffffff {
            assert!(
                cp > 0.02 && (cp - ce).abs() < ce * 0.5,
                "{nombre}: editor {ce} previa {cp}"
            );
        }
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
