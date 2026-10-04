//! **Muestras de la captura v2 para mirarlas a ojo** (`#[ignore]`: GPU).
//!
//! Pintan con las MISMAS funciones que el overlay (`pintar`, `anotar`,
//! `disposicion`) sobre un destino fuera de pantalla y guardan el PNG: asi
//! se ve la interfaz sin abrir el overlay sobre el escritorio del usuario.
//!
//! `cargo test -p pixpin --bin pixpinmax captura2::muestras -- --ignored --nocapture`
//! (`PIXPIN_MUESTRAS` = carpeta de salida; si no, la temporal).

use pixpin_codec::ImagenRgba;
use pixpin_geom::{Punto, Rect};
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use pixpin_render::{Color, MotorRender, RectF};
use pixpin_shell::ventanas_visibles::VentanaVisible;
use pixpin_store::{Catalogo, Idioma};
use pixpin_ui::FormatoColorLupa;
use std::path::PathBuf;

use super::disposicion::{self as d2, BarraModos, Modo, PanelLupa, Selector, Util};
use super::pintar as p2;
use super::{Contexto, EstadoSelector, Sesion, Textos};
use crate::overlay::ModoConfirmacion;

const ANCHO: u32 = 1440;
const ALTO: u32 = 900;

/// Un «escritorio» falso: degradado, unas ventanas claras con su barra y
/// una hoja con rejilla, para que la lupa y el mosaico tengan algo que
/// ensenar.
fn escritorio() -> ImagenRgba {
    let mut px = vec![0u8; (ANCHO * ALTO * 4) as usize];
    let ventanas = [
        (40, 90, 560, 520, [0xF2, 0xF2, 0xF4]),
        (960, 70, 430, 300, [0x3A, 0x4A, 0x5E]),
        (470, 200, 620, 400, [0xFF, 0xFF, 0xFF]),
    ];
    for y in 0..ALTO {
        for x in 0..ANCHO {
            let t = (x + y) as f32 / (ANCHO + ALTO) as f32;
            let mut c = [
                (0x2A as f32 * (1.0 - t) + 0x3B as f32 * t) as u8,
                (0x3B as f32 * (1.0 - t) + 0x2A as f32 * t) as u8,
                (0x55 as f32 * (1.0 - t) + 0x44 as f32 * t) as u8,
            ];
            for (vx, vy, vw, vh, color) in ventanas {
                if x >= vx && x < vx + vw && y >= vy && y < vy + vh {
                    c = if y < vy + 34 {
                        [0x2C, 0x2C, 0x2E]
                    } else if color == [0xFF, 0xFF, 0xFF] && (y % 28 == 0 || x % 120 == 0) {
                        [0xE3, 0xE3, 0xE8]
                    } else {
                        color
                    };
                    // Unas «barras» de un grafico en la hoja.
                    if color == [0xFF, 0xFF, 0xFF] && y > vy + 200 && y < vy + 380 {
                        let col = (x - vx) / 90;
                        let alto = [120, 160, 80, 175, 100, 140][(col as usize) % 6];
                        if (x - vx) % 90 > 20 && (x - vx) % 90 < 70 && y > vy + 380 - alto {
                            c = if col == 3 {
                                [0xE8, 0x83, 0x3A]
                            } else {
                                [0x5B, 0x8D, 0xEF]
                            };
                        }
                    }
                }
            }
            let i = ((y * ANCHO + x) * 4) as usize;
            px[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    ImagenRgba {
        ancho: ANCHO,
        alto: ALTO,
        pixeles: px,
    }
}

fn recortar(img: &ImagenRgba, r: Rect) -> ImagenRgba {
    let mut px = Vec::with_capacity((r.ancho * r.alto * 4) as usize);
    for y in r.y..r.y + r.alto as i32 {
        let i = ((y as u32 * img.ancho + r.x as u32) * 4) as usize;
        px.extend_from_slice(&img.pixeles[i..i + (r.ancho * 4) as usize]);
    }
    ImagenRgba {
        ancho: r.ancho,
        alto: r.alto,
        pixeles: px,
    }
}

fn guardar(nombre: &str, w: u32, h: u32, pixeles: Vec<u8>) -> PathBuf {
    let png = pixpin_codec::imagen::codificar_png(&ImagenRgba {
        ancho: w,
        alto: h,
        pixeles,
    })
    .unwrap();
    let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let ruta = carpeta.join(nombre);
    std::fs::write(&ruta, png).unwrap();
    println!("{nombre}: {}", ruta.display());
    ruta
}

fn velo_con_hueco(p: &pixpin_render::Pintor, s: RectF) {
    let velo = Color::oscurecido();
    let (w, h) = (ANCHO as f32, ALTO as f32);
    for r in [
        RectF {
            x: 0.0,
            y: 0.0,
            ancho: w,
            alto: s.y,
        },
        RectF {
            x: 0.0,
            y: s.y + s.alto,
            ancho: w,
            alto: h - s.y - s.alto,
        },
        RectF {
            x: 0.0,
            y: s.y,
            ancho: s.x,
            alto: s.alto,
        },
        RectF {
            x: s.x + s.ancho,
            y: s.y,
            ancho: w - s.x - s.ancho,
            alto: s.alto,
        },
    ] {
        p.rellenar(r, velo);
    }
}

fn contexto(t: &Catalogo) -> Contexto {
    pixpin_motor2d::texto::instalar_medidor(crate::dibujo::pintar::medir_para_el_motor);
    Contexto {
        textos: Textos::de(t),
        ultima_region: Some(Rect {
            x: 100,
            y: 100,
            ancho: 300,
            alto: 200,
        }),
        raiz: None,
        aparato: String::new(),
        gestos: true,
    }
}

#[test]
#[ignore = "necesita GPU; genera PNG para mirarlos"]
fn muestra_captura2_elegir() {
    let t = Catalogo::nuevo(Idioma::Espanol);
    let fondo_img = escritorio();
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d.d3d()).expect("motor");
    let fondo = motor
        .bitmap_desde_pixeles(ANCHO, ALTO, &fondo_img.pixeles)
        .unwrap();
    let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), ANCHO, ALTO).expect("superficie");
    let ventana = Rect {
        x: 470,
        y: 200,
        ancho: 620,
        alto: 400,
    };
    let mut s = Sesion::nueva(
        contexto(&t),
        ModoConfirmacion::ConBarra,
        false,
        FormatoColorLupa::Hex,
        vec![VentanaVisible {
            rect: ventana,
            titulo: "Excel · Presupuesto Obra Miraflores.xlsx".into(),
        }],
    );
    s.modo = Modo::Ventana;
    let pantalla = Rect {
        x: 0,
        y: 0,
        ancho: ANCHO,
        alto: ALTO,
    };
    let trabajo = pantalla;
    let cursor = Punto { x: 812, y: 432 };
    let l = p2::Local {
        ox: 0,
        oy: 0,
        e: 1.0,
    };
    let i = ((cursor.y as u32 * ANCHO + cursor.x as u32) * 4) as usize;
    let color = [
        fondo_img.pixeles[i],
        fondo_img.pixeles[i + 1],
        fondo_img.pixeles[i + 2],
        255,
    ];
    motor
        .dibujar(&fuera.destino, |p| {
            let todo = RectF {
                x: 0.0,
                y: 0.0,
                ancho: ANCHO as f32,
                alto: ALTO as f32,
            };
            p.bitmap(&fondo, todo, None, false);
            let r = l.r(ventana);
            velo_con_hueco(p, r);
            p.rellenar(
                r,
                Color {
                    r: 0.04,
                    g: 0.52,
                    b: 1.0,
                    a: 0.08,
                },
            );
            p.trazar(r, 3.0, p2::AZUL);
            p2::pintar_pistas(p, l, d2::fila_de_pistas(trabajo, 100), &s.contexto.textos);
            let titulo = s.ventana_en(cursor).unwrap().titulo.clone();
            let pista = s.contexto.textos.pista_ventana.clone();
            let w = p2::ancho_etiqueta(p, 1.0, &titulo, (620, 400), &pista);
            let et = d2::etiqueta_de_ventana(ventana, w, pantalla, 100);
            p2::pintar_etiqueta(p, l, et, &titulo, (620, 400), &pista);
            let b = BarraModos::colocar(trabajo, 100, true);
            p2::pintar_modos(
                p,
                l,
                &b,
                &s,
                &s.contexto.textos,
                Punto { x: 0, y: 0 },
                (620, 400),
            );
            let lupa = PanelLupa::colocar(cursor, pantalla, 100);
            let region = lupa.region(cursor, pantalla);
            p2::pintar_lupa(
                p,
                l,
                &lupa,
                &fondo,
                l.r(region),
                cursor,
                color,
                &s,
                &s.contexto.textos,
            );
        })
        .expect("pintar");
    fuera.esperar_gpu().unwrap();
    let (_, _, px) = fuera.leer_rgba().unwrap();
    let ruta = guardar("captura2-elegir.png", ANCHO, ALTO, px);
    assert!(ruta.exists());
}

#[test]
#[ignore = "necesita GPU; genera PNG para mirarlos"]
fn muestra_captura2_despues() {
    let t = Catalogo::nuevo(Idioma::Espanol);
    let fondo_img = escritorio();
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d.d3d()).expect("motor");
    let fondo = motor
        .bitmap_desde_pixeles(ANCHO, ALTO, &fondo_img.pixeles)
        .unwrap();
    let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), ANCHO, ALTO).expect("superficie");
    let sel = Rect {
        x: 470,
        y: 234,
        ancho: 620,
        alto: 366,
    };
    let mut s = Sesion::nueva(
        contexto(&t),
        ModoConfirmacion::ConBarra,
        false,
        FormatoColorLupa::Hex,
        vec![],
    );
    // Lo anotado: mosaico sobre la barra de arriba, un rectangulo rojo, un
    // texto y una flecha (como la maqueta).
    s.tomar(Util::Mosaico, sel, 100);
    {
        let a = s.anotacion_en(sel, 100);
        a.pulsar(Punto { x: 500, y: 250 });
        a.mover(Punto { x: 700, y: 290 });
        a.soltar(Punto { x: 700, y: 290 });
    }
    s.tomar(Util::Rectangulo, sel, 100);
    s.poner_grosor(2, sel, 100);
    {
        let a = s.anotacion_en(sel, 100);
        a.pulsar(Punto { x: 735, y: 420 });
        a.mover(Punto { x: 845, y: 590 });
        a.soltar(Punto { x: 845, y: 590 });
    }
    s.tomar(Util::Texto, sel, 100);
    {
        let a = s.anotacion_en(sel, 100);
        a.pulsar(Punto { x: 520, y: 330 });
        a.soltar(Punto { x: 520, y: 330 });
        for c in "Revisar este monto".chars() {
            a.caracter(c);
        }
        a.cerrar_texto();
    }
    s.tomar(Util::Flecha, sel, 100);
    {
        let a = s.anotacion_en(sel, 100);
        a.pulsar(Punto { x: 640, y: 360 });
        a.mover(Punto { x: 725, y: 410 });
        a.soltar(Punto { x: 725, y: 410 });
    }
    assert_eq!(s.anotacion.as_ref().unwrap().cuantos(), 4);
    s.proyectos = vec![
        ("g".into(), "Mensajes guardados".into()),
        ("t".into(), "Tesis".into()),
        ("o".into(), "Obra Miraflores".into()),
        ("c".into(), "Compras temu".into()),
        ("e".into(), "Examen IEN 2024".into()),
    ];
    s.selector = Some(EstadoSelector {
        elegida: 2,
        ..Default::default()
    });
    let foto = recortar(&fondo_img, sel);
    let foto_horno = foto.clone();
    s.anotacion
        .as_mut()
        .unwrap()
        .preparar(&motor, move || Some(foto));
    let pantalla = Rect {
        x: 0,
        y: 0,
        ancho: ANCHO,
        alto: ALTO,
    };
    let l = p2::Local {
        ox: 0,
        oy: 0,
        e: 1.0,
    };
    let (a, b) = d2::colocar_despues(sel, pantalla, 100);
    let selector = Selector::colocar(
        b.boton(d2::Accion::AlChat).unwrap(),
        pantalla,
        s.selector.as_ref().unwrap().visibles(&s.proyectos).len(),
        100,
    );
    let raton = Punto {
        x: b.botones[1].1.x + 10,
        y: b.botones[1].1.y + 10,
    };
    motor
        .dibujar(&fuera.destino, |p| {
            let todo = RectF {
                x: 0.0,
                y: 0.0,
                ancho: ANCHO as f32,
                alto: ALTO as f32,
            };
            p.bitmap(&fondo, todo, None, false);
            let r = l.r(sel);
            velo_con_hueco(p, r);
            s.anotacion.as_mut().unwrap().pintar(p, r);
            p.trazar(r, 2.0, Color::ACENTO);
            p2::pintar_despues(
                p,
                l,
                &a,
                &b,
                &s,
                &s.contexto.textos,
                raton,
                (sel.ancho, sel.alto),
            );
            p2::pintar_selector(
                p,
                l,
                &selector,
                s.selector.as_ref().unwrap(),
                &s.proyectos,
                Some("t"),
                &s.contexto.textos,
                raton,
            );
        })
        .expect("pintar");
    fuera.esperar_gpu().unwrap();
    let (_, _, px) = fuera.leer_rgba().unwrap();
    guardar("captura2-despues.png", ANCHO, ALTO, px);

    // Y lo que sale al copiar: la captura con lo anotado dentro.
    let mut horneada = foto_horno.clone();
    s.anotacion
        .as_mut()
        .unwrap()
        .hornear(&mut horneada, &motor, d.d3d())
        .unwrap();
    assert_ne!(
        horneada.pixeles, foto_horno.pixeles,
        "lo anotado tiene que verse"
    );
    guardar(
        "captura2-horneada.png",
        horneada.ancho,
        horneada.alto,
        horneada.pixeles,
    );
}
