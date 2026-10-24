//! Pruebas del visor de hojas con GPU y libros de verdad (ignoradas: piden
//! un `.xlsx` del usuario). La logica pura se prueba en `logica.rs`.

use super::*;
use pixpin_codec::ImagenRgba;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use std::path::PathBuf;

fn iniciar_registro() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
}

fn estado<'l>(libro: &'l mut Libro, e: f32, ancho: u32, alto: u32) -> Estado<'l> {
    let n = libro.hojas.len();
    let activa = libro.hoja_activa.min(n.saturating_sub(1));
    Estado {
        libro,
        ed: None,
        editando: false,
        ultimo_clic: None,
        sin_letras_hasta: None,
        generacion: 0,
        guardadas: Default::default(),
        hojas: (0..n).map(|_| None).collect(),
        activa,
        cache: CacheHoja::nueva(),
        digito: None,
        ppp: 96.0 * e as f64,
        e,
        ancho: ancho as f32,
        alto: alto as f32,
        pestanas: Vec::new(),
        desde_pestana: 0,
        gesto: None,
        raton: (0.0, 0.0),
        encima_de: None,
        bajo_raton: None,
        comentario_bajo: None,
        aviso: None,
        hwnd: 0,
        anotado: Default::default(),
        nombre: String::new(),
        area: pixpin_geom::Rect {
            x: 0,
            y: 0,
            ancho,
            alto,
        },
        anotando: false,
        poniendo_marca: false,
        encima: pixpin_ui::riel_marcas::DestinoRiel::Fuera,
        marcas: Vec::new(),
        recuerdo: super::riel::Recuerdo::de(None).0,
        tinta_cambio: false,
        trazo: false,
        marcos: Default::default(),
        vistas: Default::default(),
        sin_interfaz: false,
    }
}

/// **El visor pintado fuera de pantalla** con lo que hace el usuario:
/// abrir (primera pantalla), bajar con la rueda, esperar a que se lea
/// entera, Ctrl+Fin, Ctrl+rueda, y cada pestana. Deja PNG en
/// `PIXPIN_MUESTRAS` con el prefijo `PIXPIN_PREFIJO`.
///
/// `PIXPIN_XLSX=<libro> PIXPIN_MUESTRAS=<carpeta> cargo test --release -p pixpin --bin pixpinmax visor_hoja::pruebas::muestra_fuera_de_pantalla -- --ignored --nocapture`
#[test]
#[ignore = "necesita GPU y un libro de Excel"]
fn muestra_fuera_de_pantalla() {
    iniciar_registro();
    let ruta = PathBuf::from(std::env::var("PIXPIN_XLSX").expect("PIXPIN_XLSX"));
    let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let prefijo = std::env::var("PIXPIN_PREFIJO").unwrap_or_else(|_| "hoja".into());
    let e: f32 = std::env::var("PIXPIN_ESCALA")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.25);
    let (ancho, alto) = ((1100.0 * e) as u32, (760.0 * e) as u32);
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d.d3d()).expect("motor");
    let textos = Catalogo::nuevo(Idioma::Espanol);

    let t0 = Instant::now();
    let mut libro = Libro::abrir(&ruta, regional_del_usuario(Idioma::Espanol)).expect("abre");
    let t_libro = t0.elapsed();
    let mut est = estado(&mut libro, e, ancho, alto);
    let activa = est.activa;
    est.abrir_hoja(activa, &motor);
    let t_hoja = t0.elapsed();
    let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), ancho, alto).expect("superficie");
    let mut n = 0;
    let mut foto = |est: &mut Estado, que: &str| -> std::time::Duration {
        let t = Instant::now();
        preparar(est, &motor);
        motor
            .dibujar(&fuera.destino, |p| pintar(p, est, &textos))
            .expect("pintar");
        fuera.esperar_gpu().expect("gpu");
        let dur = t.elapsed();
        let (a, b, pixeles) = fuera.leer_rgba().expect("leer");
        let png = pixpin_codec::imagen::codificar_png(&ImagenRgba {
            ancho: a,
            alto: b,
            pixeles,
        })
        .expect("png");
        n += 1;
        let salida = carpeta.join(format!("{prefijo}-{n:02}-{que}.png"));
        std::fs::write(&salida, png).expect("guardar");
        println!(
            "{que}: {:.1} ms -> {}",
            dur.as_secs_f64() * 1000.0,
            salida.display()
        );
        dur
    };
    let t_pintar = foto(&mut est, "primera");
    println!(
        "libro {:.1} ms, primera pantalla leida {:.1} ms, pintada {:.1} ms: {:.1} ms desde abrir",
        t_libro.as_secs_f64() * 1000.0,
        (t_hoja - t_libro).as_secs_f64() * 1000.0,
        t_pintar.as_secs_f64() * 1000.0,
        (t_hoja + t_pintar).as_secs_f64() * 1000.0
    );

    // La rueda: 40 muescas abajo mientras se sigue leyendo.
    for _ in 0..40 {
        est.rueda(-120, false);
    }
    est.recibir();
    foto(&mut est, "rueda");

    // Esperar a que llegue todo.
    let t = Instant::now();
    while est.cargando() && t.elapsed() < Duration::from_secs(60) {
        est.recibir();
        std::thread::sleep(Duration::from_millis(5));
    }
    est.recibir();
    println!(
        "hoja entera en {:.0} ms desde abrirla ({} filas)",
        est.abierta().abierta_en.elapsed().as_secs_f64() * 1000.0,
        est.abierta().hoja.filas.len()
    );

    // Desplazarse en una hoja ya leida: lo que cuesta un fotograma lejos.
    est.mover(Mov::FinHoja, false);
    let d1 = foto(&mut est, "ctrl-fin");
    est.mover(Mov::RePag, false);
    let d2 = foto(&mut est, "repag");
    println!(
        "fotogramas lejos: {:.1} y {:.1} ms",
        d1.as_secs_f64() * 1000.0,
        d2.as_secs_f64() * 1000.0
    );
    est.mover(Mov::InicioHoja, false);
    est.mover(Mov::Derecha, true);
    est.mover(Mov::Abajo, true);
    est.mover(Mov::Abajo, true);
    est.raton = (400.0 * e, 300.0 * e);
    est.poner_zoom(1.6, Some(est.raton));
    foto(&mut est, "zoom");
    est.poner_zoom(1.0, None);

    // Cada pestana visible.
    let visibles: Vec<usize> = (0..est.libro.hojas.len())
        .filter(|&i| est.libro.hojas[i].visibilidad == Visibilidad::Visible)
        .take(6)
        .collect();
    for i in visibles {
        if i == activa {
            continue;
        }
        est.cambiar_hoja(i, &motor, &textos);
        foto(&mut est, &format!("pestana{i}"));
    }
}

/// **La ventana de verdad** (no la de la app instalada: esta corre en el
/// ejecutable de pruebas). Se queda abierta hasta cerrarla (Esc, la X de
/// su barra o WM_CLOSE).
///
/// `PIXPIN_XLSX=<libro> cargo test --release -p pixpin --bin pixpinmax visor_hoja::pruebas::ventana_de_verdad -- --ignored --nocapture`
#[test]
#[ignore = "abre una ventana y espera a que se cierre"]
fn ventana_de_verdad() {
    iniciar_registro();
    let ruta = PathBuf::from(std::env::var("PIXPIN_XLSX").expect("PIXPIN_XLSX"));
    ver(Idioma::Espanol, &ruta, Instant::now());
}

/// «Editar» solo sale en las hojas del chat (que es donde se guarda), y
/// «Abrir con su programa» solo si hay archivo.
#[test]
fn editar_solo_en_las_hojas_del_chat() {
    assert!(pide_editar(VK_F2, false, false));
    assert!(pide_editar(VK_E, false, false));
    // Casos negativos: Ctrl+E y Alt+E son otras cosas, y otra letra no.
    assert!(!pide_editar(VK_E, true, false));
    assert!(!pide_editar(VK_E, false, true));
    assert!(!pide_editar(0x41, false, false));
    let ids = |b: Vec<BotonLector>| b.iter().map(|x| x.id).collect::<Vec<_>>();
    assert_eq!(
        ids(botones_de_ver(true, true)),
        [BOTON_EDITAR, BOTON_ABRIR_CON],
        "sin Copiar: Ctrl+C copia"
    );
    assert_eq!(ids(botones_de_ver(false, true)), [BOTON_ABRIR_CON]);
    assert_eq!(ids(botones_de_ver(true, false)), [BOTON_EDITAR]);
    assert!(botones_de_ver(false, false).is_empty());
}

/// Un libro de tablas del chat (como el que se ve de una hoja del chat),
/// con `filas` x `columnas` numeros.
fn libro_de_tabla(filas: u32, columnas: u32) -> Libro {
    let mut t = pixpin_proyecto::tabla::Tabla::default();
    for f in 0..filas {
        for c in 0..columnas {
            t.poner(
                pixpin_proyecto::tabla::Ref {
                    columna: c,
                    fila: f,
                },
                &format!("{}", f * 100 + c),
            );
        }
    }
    let bytes = pixpin_proyecto::tabla_xlsx::a_xlsx(
        &[pixpin_proyecto::tabla_xlsx::HojaParaVer {
            nombre: "Gastos",
            tabla: &t,
            valores: None,
        }],
        0,
    )
    .unwrap();
    Libro::desde_bytes(bytes, Regional::es_pe()).unwrap()
}

/// Espera a que el hilo que lee termine las hojas abiertas.
fn leida_entera(est: &mut Estado) {
    let t = Instant::now();
    while est.cargando() && t.elapsed() < Duration::from_secs(20) {
        est.recibir();
        std::thread::sleep(Duration::from_millis(5));
    }
    est.recibir();
}

/// **Arrastrar con el boton central mueve la hoja** (como el lienzo): la
/// sigue al raton, al zoom que se ve, sin elegir celdas, con el cursor de
/// mover; y no se pasa del principio.
#[test]
fn el_boton_central_arrastra_la_hoja_sin_elegir() {
    let mut libro = libro_de_tabla(300, 40);
    let mut est = estado(&mut libro, 1.0, 900, 700);
    est.digito = Some(7.0);
    est.abrir_hoja_con_digito(0);
    leida_entera(&mut est);
    let sel = est.abierta().sel;
    assert_eq!((est.abierta().x, est.abierta().y), (0.0, 0.0));
    // Caso negativo: hacia arriba y a la izquierda desde el principio, nada.
    est.raton = (400.0, 300.0);
    est.empezar_mano();
    assert_eq!(est.cursor(), FormaCursorWin::Mover);
    est.raton = (460.0, 380.0);
    est.llevar_mano();
    assert_eq!(
        (est.abierta().x, est.abierta().y),
        (0.0, 0.0),
        "no se pasa del principio"
    );
    // Hacia abajo y a la derecha: la hoja va 100 px por cada 100 del raton.
    est.raton = (300.0, 200.0);
    est.llevar_mano();
    assert_eq!((est.abierta().x, est.abierta().y), (100.0, 100.0));
    assert_eq!(
        est.abierta().sel,
        sel,
        "arrastrar con el central no elige celdas"
    );
    // Al doble de zoom, la mitad de hoja por el mismo gesto.
    est.gesto = None;
    est.abierta_mut().zoom = 2.0;
    est.raton = (400.0, 300.0);
    est.empezar_mano();
    est.raton = (300.0, 300.0);
    est.llevar_mano();
    assert_eq!(est.abierta().x, 150.0);
    est.gesto = None;
    assert_ne!(
        est.cursor(),
        FormaCursorWin::Mover,
        "al soltar vuelve la flecha"
    );
}

/// **Ver y editar pintados del mismo libro** (una tabla del chat), para
/// compararlos: pasar a editar no mueve nada (la hoja se pinta igual pixel a
/// pixel), lo tecleado sale encima de su celda, el formato se ve y rehacer
/// lo que se ve no mueve la hoja. Deja PNG en `PIXPIN_MUESTRAS`.
///
/// `PIXPIN_MUESTRAS=<carpeta> cargo test -p pixpin --bin pixpinmax visor_hoja::pruebas::ver_y_editar_tienen_la_misma_cara -- --ignored --nocapture`
#[test]
#[ignore = "necesita GPU"]
fn ver_y_editar_tienen_la_misma_cara() {
    use crate::hoja_pin::AccionEd;
    use crate::hoja_pin::editor::{Formato, Interruptor};
    iniciar_registro();
    let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let e = 1.25f32;
    let (ancho, alto) = ((900.0 * e) as u32, (560.0 * e) as u32);
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d.d3d()).expect("motor");
    let textos = Catalogo::nuevo(Idioma::Espanol);

    // Una tabla del chat con cabecera, numeros y una formula.
    let raiz = std::env::temp_dir().join(format!("pixpin-ver-editar-{}", std::process::id()));
    let mut t = pixpin_proyecto::tabla::Tabla::default();
    let r = |s: &str| pixpin_proyecto::tabla::ref_de(s).unwrap();
    for (k, v) in [
        ("A1", "Concepto"),
        ("B1", "Importe"),
        ("C1", "Fecha"),
        ("A2", "Cemento"),
        ("B2", "120.5"),
        ("A3", "Arena"),
        ("B3", "80"),
        ("A4", "Total"),
        ("B4", "=SUMA(B2:B3)"),
        ("C2", "46000"),
        ("A6", "Un texto largo que se sale por las celdas vacias"),
    ] {
        t.poner(r(k), v);
    }
    for f in 7..60 {
        t.poner(
            pixpin_proyecto::tabla::Ref {
                columna: 1,
                fila: f,
            },
            &format!("{}", f * 3),
        );
    }
    t.estilos.insert(
        "A1".into(),
        pixpin_proyecto::tabla::EstiloDeCelda {
            n: true,
            f: Some("#d0ebff".into()),
            ..Default::default()
        },
    );
    let sello = pixpin_proyecto::cuaderno::Sello {
        cuando: 5,
        numero: 1,
        aparato: "PC".into(),
        proyecto: "p".into(),
    };
    let m = pixpin_proyecto::cuaderno::Mensaje::miniapp(
        "tabla",
        "Gastos",
        &t.escribir().unwrap(),
        &sello,
    );
    let mut ed = crate::hoja_pin::Editable::nuevo(
        pixpin_store::Ubicacion::Portable { raiz: raiz.clone() },
        "p".into(),
        crate::hoja_pin::Origen::Tabla { mensaje: m },
    );
    let mut libro = ed.libro_para_ver(Regional::es_pe()).expect("se ve");
    let mut est = estado(&mut libro, e, ancho, alto);
    est.ed = Some(&mut ed);
    est.abrir_hoja(0, &motor);
    leida_entera(&mut est);
    // Un zoom y una posicion cualquiera, y una celda elegida.
    est.raton = (300.0 * e, 200.0 * e);
    est.poner_zoom(1.3, Some(est.raton));
    est.abierta_mut().sel = Seleccion::en(1, 1);

    let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), ancho, alto).expect("superficie");
    let foto = |est: &mut Estado, que: &str| -> Vec<u8> {
        preparar(est, &motor);
        motor
            .dibujar(&fuera.destino, |p| pintar(p, est, &textos))
            .expect("pintar");
        fuera.esperar_gpu().expect("gpu");
        let (a, b, pixeles) = fuera.leer_rgba().expect("leer");
        let png = pixpin_codec::imagen::codificar_png(&ImagenRgba {
            ancho: a,
            alto: b,
            pixeles: pixeles.clone(),
        })
        .expect("png");
        let salida = carpeta.join(format!("hoja-ver-editar-{que}.png"));
        std::fs::write(&salida, png).expect("guardar");
        println!("{que}: {}", salida.display());
        pixeles
    };
    let ver = foto(&mut est, "1-ver");
    let (x0, y0) = (est.abierta().x, est.abierta().y);
    assert!(est.empezar_a_editar(&textos));
    let editar = foto(&mut est, "2-editar");
    assert_eq!(
        (est.abierta().x, est.abierta().y),
        (x0, y0),
        "editar no mueve la hoja"
    );
    // La hoja (sin la barra de formulas, que dice que se edita), igual.
    let z = est.zonas();
    let desde = (z.hoja.y as usize) * ancho as usize * 4;
    let hasta = (z.pestanas.y as usize).min(alto as usize) * ancho as usize * 4;
    let distintos = ver[desde..hasta]
        .chunks(4)
        .zip(editar[desde..hasta].chunks(4))
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(distintos, 0, "ver y editar pintan la hoja igual");

    // Escribir en una celda: sale encima, en la letra de la hoja.
    est.abierta_mut().sel = Seleccion::en(4, 1);
    for c in "=B2*2".chars() {
        est.teclear(c);
    }
    foto(&mut est, "3-escribiendo");
    est.hacer(AccionEd::Confirmar, &textos, &motor);
    // Formato a un rango: cursiva, subrayado, resaltar, color y moneda.
    est.abierta_mut().sel = Seleccion {
        ancla: (1, 0),
        activa: (4, 1),
    };
    for f in [
        Formato::Alternar(Interruptor::Cursiva),
        Formato::Alternar(Interruptor::Subrayado),
        Formato::Fondo(Some("#fff3bf".into())),
        Formato::Letra(Some("#c00000".into())),
        Formato::Numero(Some(pixpin_proyecto::tabla::FormatoNumero::Moneda)),
    ] {
        est.hacer(AccionEd::Formato(f), &textos, &motor);
    }
    est.abierta_mut().sel = Seleccion::en(5, 0);
    est.hacer(
        AccionEd::Formato(Formato::Alternar(Interruptor::Tachado)),
        &textos,
        &motor,
    );
    assert_eq!(
        (est.abierta().x, est.abierta().y),
        (x0, y0),
        "rehacer lo que se ve no mueve la hoja"
    );
    assert!((est.abierta().zoom - 1.3).abs() < 1e-9, "ni el zoom");
    foto(&mut est, "4-editar-con-formato");
    est.dejar_de_editar(&motor);
    foto(&mut est, "5-ver-con-formato");
    // Con el modo oscuro de los visores, editar se oscurece igual (en las
    // pruebas la preferencia no se guarda: no hay ubicacion).
    let antes = crate::oscuro::activo();
    if !antes {
        crate::oscuro::alternar();
    }
    assert!(est.empezar_a_editar(&textos));
    est.abierta_mut().sel = Seleccion::en(2, 2);
    for c in "Oscuro".chars() {
        est.teclear(c);
    }
    foto(&mut est, "6-editar-oscuro");
    est.hacer(AccionEd::Cancelar, &textos, &motor);
    if !antes {
        crate::oscuro::alternar();
    }
    let _ = std::fs::remove_dir_all(&raiz);
}

/// Lo que cuesta rehacer lo que se ve tras un cambio en una tabla grande
/// (40 000 celdas): pasarla a `.xlsx`, leerlo y abrir su primera pantalla.
///
/// `cargo test -p pixpin --bin pixpinmax visor_hoja::pruebas::mide_rehacer_una_tabla_grande -- --ignored --nocapture`
#[test]
#[ignore = "mide tiempos"]
fn mide_rehacer_una_tabla_grande() {
    for _ in 0..3 {
        let t = Instant::now();
        let mut libro = libro_de_tabla(1000, 40);
        let ms_libro = t.elapsed().as_secs_f64() * 1000.0;
        let mut est = estado(&mut libro, 1.0, 900, 700);
        est.digito = Some(7.0);
        est.guardadas.insert(
            0,
            Posicion {
                x: 0.0,
                y: 5000.0,
                zoom: 1.0,
                sel: Seleccion::en(200, 3),
            },
        );
        est.abrir_hoja_con_digito(0);
        println!(
            "tabla de 40 000 celdas: libro {ms_libro:.1} ms, con su primera pantalla {:.1} ms",
            t.elapsed().as_secs_f64() * 1000.0
        );
    }
}

/// El reparto de lo anterior: calcular, escribir el `.xlsx` y leerlo.
#[test]
#[ignore = "mide tiempos"]
fn mide_rehacer_por_partes() {
    let mut t = pixpin_proyecto::tabla::Tabla::default();
    for f in 0..1000 {
        for c in 0..40 {
            t.poner(
                pixpin_proyecto::tabla::Ref {
                    columna: c,
                    fila: f,
                },
                &format!("{}", f * 100 + c),
            );
        }
    }
    let ms = |t: Instant| t.elapsed().as_secs_f64() * 1000.0;
    let t0 = Instant::now();
    let v = pixpin_proyecto::formula::evaluar_todo(&t);
    let calcular = ms(t0);
    let t1 = Instant::now();
    let bytes = pixpin_proyecto::tabla_xlsx::a_xlsx(
        &[pixpin_proyecto::tabla_xlsx::HojaParaVer {
            nombre: "G",
            tabla: &t,
            valores: Some(&v),
        }],
        0,
    )
    .unwrap();
    let escribir = ms(t1);
    let t2 = Instant::now();
    let libro = Libro::desde_bytes(bytes, Regional::es_pe()).unwrap();
    let mut h = libro.abrir_hoja(0);
    h.cargar_tanda(TANDA_CELDAS);
    println!(
        "calcular {calcular:.1} ms, escribir {escribir:.1} ms, leer {:.1} ms",
        ms(t2)
    );
}

/// Los botones de la barra al editar, pintados como en la barra del pin
/// (iconos claros sobre oscuro; encendidos, dorados), para mirarlos.
///
/// `PIXPIN_MUESTRAS=<carpeta> cargo test -p pixpin --bin pixpinmax visor_hoja::pruebas::botones_de_editar_pintados -- --ignored`
#[test]
#[ignore = "necesita GPU"]
fn botones_de_editar_pintados() {
    let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d.d3d()).expect("motor");
    let mut t = pixpin_proyecto::tabla::Tabla::default();
    t.estilos.insert(
        "A1".into(),
        pixpin_proyecto::tabla::EstiloDeCelda {
            n: true,
            a: Some("c".into()),
            ..Default::default()
        },
    );
    let ed = crate::hoja_pin::editor::Editor::nuevo(
        vec![crate::hoja_pin::editor::HojaEditable::nueva(
            "x".into(),
            t,
            None,
        )],
        0,
    )
    .unwrap();
    let botones = crate::hoja_pin::botones_de_editar(&ed);
    let (lado, n) = (50u32, botones.len() as u32);
    let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), lado * n, lado).expect("superficie");
    motor
        .dibujar(&fuera.destino, |p| {
            p.limpiar(Color {
                r: 0.118,
                g: 0.118,
                b: 0.125,
                a: 1.0,
            });
            for (i, b) in botones.iter().enumerate() {
                let caja = RectF {
                    x: i as f32 * lado as f32 + 10.0,
                    y: 10.0,
                    ancho: 30.0,
                    alto: 30.0,
                };
                let color = if b.encendido {
                    Color {
                        r: 0.95,
                        g: 0.77,
                        b: 0.33,
                        a: 1.0,
                    }
                } else {
                    Color {
                        r: 0.96,
                        g: 0.96,
                        b: 0.97,
                        a: 1.0,
                    }
                };
                p.icono(b.icono, caja, color);
            }
        })
        .expect("pintar");
    fuera.esperar_gpu().expect("gpu");
    let (a, b, pixeles) = fuera.leer_rgba().expect("leer");
    let png = pixpin_codec::imagen::codificar_png(&ImagenRgba {
        ancho: a,
        alto: b,
        pixeles,
    })
    .expect("png");
    let salida = carpeta.join("hoja-botones-de-editar.png");
    std::fs::write(&salida, png).expect("guardar");
    println!("{}", salida.display());
}

// ---------------------------------------------------------------------------
// Los marcos numerados, las vistas de las marcas y la tinta en su celda

/// Un estado de 900 x 700 sin abrir hoja (para las pruebas de otros
/// modulos del visor).
pub(super) fn estado_de_prueba(libro: &mut Libro) -> Estado<'_> {
    estado(libro, 1.0, 900, 700)
}

/// Una hoja de tabla abierta y leida entera, de `ancho` x `alto`.
fn abierta_de_prueba(libro: &mut Libro, ancho: u32, alto: u32) -> Estado<'_> {
    let mut est = estado(libro, 1.0, ancho, alto);
    est.digito = Some(7.0);
    est.abrir_hoja_con_digito(0);
    leida_entera(&mut est);
    est
}

/// **Como en el plano**: con el modo puesto, arrastrar en las celdas pone un
/// marco en los pixeles de la hoja (no elige celdas), en su hoja; se mueve
/// con el desplazamiento y el zoom; un clic no es un marco.
#[test]
fn los_marcos_van_en_la_hoja_y_siguen_al_zoom_y_al_desplazamiento() {
    let mut libro = libro_de_tabla(300, 40);
    let mut est = abierta_de_prueba(&mut libro, 900, 700);
    let sel = est.abierta().sel;
    // Caso negativo: sin el modo, el clic no empieza un marco.
    est.raton = (300.0, 300.0);
    assert!(!est.pulsar_marco());
    est.marcos.alternar();
    let celdas = est.vista().1;
    let (x0, y0) = (celdas.x + 100.0, celdas.y + 50.0);
    est.raton = (x0, y0);
    assert!(est.pulsar_marco(), "en las celdas empieza un marco");
    assert!(matches!(est.gesto, Some(Gesto::Marco)));
    est.raton = (x0 + 200.0, y0 + 120.0);
    assert!(est.mover_marco());
    assert!(est.soltar_marco());
    est.gesto = None;
    assert_eq!(est.abierta().sel, sel, "poner un marco no elige celdas");
    let m = est.marcos.lista[0].clone();
    assert_eq!(m.parte, 0);
    assert!(
        (m.ancho() - 200.0).abs() < 1e-6 && (m.alto() - 120.0).abs() < 1e-6,
        "al 100 %, un px de hoja por px: {:?}",
        m.caja
    );
    // Un clic (menos de 8 px) no cuenta.
    est.raton = (x0 + 400.0, y0 + 300.0);
    est.pulsar_marco();
    est.raton = (x0 + 403.0, y0 + 302.0);
    assert!(!est.soltar_marco());
    assert_eq!(est.marcos.lista.len(), 1);
    // Bajar 60 px de hoja: el marco sube 60 en la ventana.
    let caja = est.zonas().hoja;
    let antes = marcos::Mapa::de_abierta(est.abierta(), caja).a_pantalla([m.caja[0], m.caja[1]]);
    est.abierta_mut().y += 60.0;
    let despues = marcos::Mapa::de_abierta(est.abierta(), caja).a_pantalla([m.caja[0], m.caja[1]]);
    assert!(
        (antes.1 - despues.1 - 60.0).abs() < 0.01,
        "{antes:?} {despues:?}"
    );
    // Al doble de zoom mide el doble en la ventana.
    est.abierta_mut().zoom = 2.0;
    let mapa = marcos::Mapa::de_abierta(est.abierta(), caja);
    let r = crate::marcos_doc::caja_en_pantalla(m.caja, &|q| mapa.a_pantalla(q));
    assert!(
        (r.ancho - 400.0).abs() < 0.01 && (r.alto - 240.0).abs() < 0.01,
        "{r:?}"
    );
}

/// Para la foto, el marco llena la zona de las celdas, centrado.
#[test]
fn para_imprimir_el_marco_llena_las_celdas() {
    let mut libro = libro_de_tabla(300, 40);
    let mut est = abierta_de_prueba(&mut libro, 900, 700);
    let caja = est.zonas().hoja;
    let m = crate::marcos_doc::Marco {
        caja: [300.0, 400.0, 500.0, 500.0],
        parte: 0,
        vista: String::new(),
    };
    let ab = est.abierta_mut();
    marcos::encuadrar(ab, caja, &m);
    let (v, celdas) = vista_de(ab, caja);
    let mapa = marcos::Mapa::de(&ab.rejilla, &v, celdas);
    let r = crate::marcos_doc::caja_en_pantalla(m.caja, &|q| mapa.a_pantalla(q));
    assert!(
        r.x >= celdas.x - 0.5 && r.y >= celdas.y - 0.5,
        "{r:?} {celdas:?}"
    );
    assert!(
        r.x + r.ancho <= celdas.x + celdas.ancho + 0.5
            && r.y + r.alto <= celdas.y + celdas.alto + 0.5
    );
    // El lado que aprieta (el ancho: 200 px en ~860) ocupa el 94 %.
    assert!(
        (r.ancho / celdas.ancho - 0.94).abs() < 0.015,
        "{} de {}",
        r.ancho,
        celdas.ancho
    );
    let centro = (r.x + r.ancho / 2.0, r.y + r.alto / 2.0);
    assert!(
        (centro.0 - (celdas.x + celdas.ancho / 2.0)).abs() < 0.5
            && (centro.1 - (celdas.y + celdas.alto / 2.0)).abs() < 0.5
    );
}

/// **Ir a un marco por su ficha**: las fichas salen solo con el modo puesto
/// (encima de las pestanas), y el marco llena el panel que se mueve,
/// centrado en el, tambien con filas y columnas inmovilizadas.
#[test]
fn la_ficha_lleva_al_marco_centrado_en_el_panel_que_se_mueve() {
    let mut libro = libro_de_tabla(300, 40);
    let mut est = abierta_de_prueba(&mut libro, 900, 700);
    est.marcos.lista.push(crate::marcos_doc::Marco {
        caja: [600.0, 900.0, 800.0, 1000.0],
        parte: 0,
        vista: String::new(),
    });
    est.marcos.lista.push(crate::marcos_doc::Marco {
        caja: [100.0, 100.0, 300.0, 300.0],
        parte: 0,
        vista: String::new(),
    });
    // Caso negativo: sin el modo no hay fichas.
    let alto = est.zonas().pestanas.y;
    est.raton = (12.0 + 14.0, alto - 34.0 + 4.0);
    assert_eq!(est.ficha_bajo_el_raton(), None);
    est.marcos.alternar();
    let fichas = est.marcos.fichas(alto, est.e);
    assert_eq!(fichas.len(), 2);
    assert!(
        fichas.iter().all(|r| r.y + r.alto < alto),
        "encima de las pestanas"
    );
    est.raton = (fichas[1].x + 5.0, fichas[1].y + 5.0);
    assert_eq!(est.ficha_bajo_el_raton(), Some(1));
    let caja = est.zonas().hoja;
    // (Un marco encima de lo inmovilizado no puede ir entero en el panel
    // que se mueve: el 2 se prueba sin paneles fijos.)
    for (fijas, m) in [
        (0, est.marcos.lista[0].clone()),
        (3, est.marcos.lista[0].clone()),
        (0, est.marcos.lista[1].clone()),
    ] {
        let ab = est.abierta_mut();
        ab.rejilla.filas_fijas = fijas;
        ab.rejilla.columnas_fijas = fijas;
        marcos::encuadrar_en_el_panel(ab, caja, &m);
        let (v, celdas) = vista_de(ab, caja);
        let mapa = marcos::Mapa::de(&ab.rejilla, &v, celdas);
        let r = crate::marcos_doc::caja_en_pantalla(m.caja, &|q| mapa.a_pantalla(q));
        let panel = (
            mapa.esquina(),
            (celdas.x + celdas.ancho, celdas.y + celdas.alto),
        );
        let centro = (r.x + r.ancho / 2.0, r.y + r.alto / 2.0);
        let medio = ((panel.0.0 + panel.1.0) / 2.0, (panel.0.1 + panel.1.1) / 2.0);
        // El marco 2 cae cerca del principio de la hoja: no se puede pasar
        // del borde, asi que solo se comprueba que se ve entero.
        if m.caja[0] > 500.0 {
            assert!(
                (centro.0 - medio.0).abs() < 1.0 && (centro.1 - medio.1).abs() < 1.0,
                "fijas {fijas}: {centro:?} {medio:?}"
            );
            assert!(ab.zoom > 1.5, "acercado: {}", ab.zoom);
        }
        assert!(
            r.x >= panel.0.0 - 0.5
                && r.y >= panel.0.1 - 0.5
                && r.x + r.ancho <= panel.1.0 + 0.5
                && r.y + r.alto <= panel.1.1 + 0.5,
            "fijas {fijas}: {r:?} {panel:?}"
        );
    }
}

/// **La marca vuelve con su zoom y su sitio exactos**; una de antes (sin
/// vista) sigue llevando su celda arriba a la izquierda, al zoom de ahora.
#[test]
fn la_marca_vuelve_con_su_zoom_y_su_sitio() {
    let dir = std::env::temp_dir().join(format!("pixpin-hoja-vistas-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let doc = dir.join("libro.xlsx");
    let mut libro = libro_de_tabla(300, 40);
    let mut est = abierta_de_prueba(&mut libro, 900, 700);
    est.vistas = crate::marcos_doc::Vistas::de_documento(Some(&doc));
    est.abierta_mut().sel = Seleccion::en(40, 3);
    {
        let ab = est.abierta_mut();
        (ab.zoom, ab.x, ab.y) = (1.75, 120.0, 610.0);
    }
    est.poner_marca(0);
    let id = est.marcas[0].id.to_string();
    let vista = est
        .vistas
        .de_marca(&id)
        .and_then(crate::marcos_doc::zoom_de_texto);
    assert_eq!(vista, Some((1.75, 120.0, 610.0)));
    // Lo guardado al lado del libro vuelve igual al reabrir.
    assert_eq!(
        crate::marcos_doc::Vistas::de_documento(Some(&doc))
            .de_marca(&id)
            .and_then(crate::marcos_doc::zoom_de_texto),
        vista
    );
    // Se va a otro sitio y otro zoom, y la marca devuelve lo de entonces.
    {
        let ab = est.abierta_mut();
        (ab.zoom, ab.x, ab.y) = (0.5, 0.0, 0.0);
    }
    let (_, celda) = riel::celda_de_marca(&est.marcas[0]);
    est.ver_la_marca(celda, vista);
    let ab = est.abierta();
    assert_eq!((ab.zoom, ab.x, ab.y), (1.75, 120.0, 610.0));
    assert_eq!(ab.sel, Seleccion::en(40, 3));
    // Caso negativo: una marca de antes (sin vista), a su celda y sin tocar el zoom.
    est.ver_la_marca((40, 3), None);
    let ab = est.abierta();
    assert_eq!(ab.zoom, 1.75);
    assert_eq!((ab.x, ab.y), (ab.rejilla.x(3), ab.rejilla.y(40)));
    // Quitar la marca quita su vista.
    est.raton = (-100.0, -100.0);
    est.vistas.quitar(&id);
    assert!(est.vistas.de_marca(&id).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// **La tinta sigue a su celda aunque una fila de arriba cambie de alto** (el
/// autoajuste de las filas con texto ajustado, que se mide al irlas viendo):
/// la capa se vuelve a llevar a la rejilla.
#[test]
fn la_tinta_se_rehace_si_una_fila_cambia_de_alto() {
    let mut libro = libro_de_tabla(300, 40);
    let mut est = abierta_de_prueba(&mut libro, 900, 700);
    let ab = est.abierta_mut();
    let antes = capa::clave_de(&ab.hoja, &ab.rejilla);
    let y_fila_10 = ab.rejilla.y(10);
    // Caso negativo: medir la misma altura no cambia nada.
    let alto_3 = ab.rejilla.alto(3);
    ab.rejilla.fijar_altos(&ab.hoja, &[3], &[(3, alto_3)]);
    assert_eq!(capa::clave_de(&ab.hoja, &ab.rejilla), antes);
    assert!(
        ab.rejilla
            .fijar_altos(&ab.hoja, &[4], &[(4, alto_3 + 60.0)])
    );
    assert_ne!(
        capa::clave_de(&ab.hoja, &ab.rejilla),
        antes,
        "la capa se rehace"
    );
    assert!(
        (ab.rejilla.y(10) - y_fila_10 - 60.0).abs() < 1e-6,
        "la fila 10 bajo 60 px y lo anotado en ella con ella"
    );
}

/// **Los marcos en la ventana de verdad** (la de este ejecutable de
/// pruebas, no la app instalada): se ponen con el modo de los marcos y el
/// raton de mentira (mensajes a la ventana propia), quedan en el
/// `.pixpin-marcos` y la impresion en grupo saca una foto por marco, en su
/// orden y sin el azul de los marcos. Deja las fotos en `PIXPIN_MUESTRAS`.
///
/// `PIXPIN_MUESTRAS=<carpeta> cargo test -p pixpin --bin pixpinmax visor_hoja::pruebas::los_marcos_en_la_ventana_de_verdad -- --ignored --nocapture --test-threads=1`
#[test]
#[ignore = "abre la ventana del visor en el escritorio y la fotografia"]
fn los_marcos_en_la_ventana_de_verdad() {
    use crate::hoja_pin::prueba;
    iniciar_registro();
    let dir = std::env::temp_dir().join(format!("pixpin-hoja-marcos-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ruta = dir.join("marcos.xlsx");
    let mut t = pixpin_proyecto::tabla::Tabla::default();
    for f in 0..200u32 {
        for c in 0..30u32 {
            t.poner(
                pixpin_proyecto::tabla::Ref {
                    columna: c,
                    fila: f,
                },
                &format!("{}", f * 100 + c),
            );
        }
    }
    let bytes = pixpin_proyecto::tabla_xlsx::a_xlsx(
        &[pixpin_proyecto::tabla_xlsx::HojaParaVer {
            nombre: "Gastos",
            tabla: &t,
            valores: None,
        }],
        0,
    )
    .unwrap();
    std::fs::write(&ruta, bytes).unwrap();
    let esperar = |que: &dyn Fn() -> bool| {
        let t = Instant::now();
        while !que() {
            assert!(t.elapsed() < Duration::from_secs(20), "no llego a tiempo");
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    prueba::olvidar();
    marcos::FOTOS.lock().unwrap().clear();
    lanzar(Idioma::Espanol, &ruta);
    esperar(&|| prueba::ventana().is_some());
    std::thread::sleep(Duration::from_millis(800));
    let (h, _, _) = prueba::ventana().unwrap();
    prueba::ordenar(prueba::MARCOS);
    std::thread::sleep(Duration::from_millis(300));
    let pausa = || std::thread::sleep(Duration::from_millis(60));
    for (x0, y0, x1, y1) in [
        (120, 120, 320, 220),
        (360, 140, 520, 300),
        (150, 300, 300, 420),
    ] {
        pixpin_shell::overlay::simular_raton(h, 0, x0, y0);
        pausa();
        pixpin_shell::overlay::simular_raton(h, 1, (x0 + x1) / 2, (y0 + y1) / 2);
        pausa();
        pixpin_shell::overlay::simular_raton(h, 1, x1, y1);
        pausa();
        pixpin_shell::overlay::simular_raton(h, 2, x1, y1);
        pausa();
    }
    std::thread::sleep(Duration::from_millis(500));
    let guardado = std::fs::read_to_string(crate::marcos_doc::ruta(&ruta))
        .expect("los marcos, junto al libro");
    let lista: Vec<crate::marcos_doc::Marco> = serde_json::from_str(&guardado).unwrap();
    println!("marcos: {guardado}");
    assert_eq!(lista.len(), 3, "tres marcos, en su orden");
    assert!(
        lista[0].caja[0] < lista[1].caja[0],
        "el 1 es el primero que se puso"
    );
    let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
        .map(PathBuf::from)
        .unwrap_or_else(|| dir.clone());
    if let Some(r) = crate::marco_imprimir::foto_de_pantalla(prueba::ventana().unwrap().1) {
        std::fs::write(
            carpeta.join("hoja-marcos-ventana.png"),
            pixpin_codec::imagen::codificar_png(&r).unwrap(),
        )
        .unwrap();
    }
    prueba::ordenar(prueba::IMPRIMIR);
    esperar(&|| marcos::FOTOS.lock().unwrap().len() == 3);
    let fotos = marcos::FOTOS.lock().unwrap().clone();
    for (i, f) in fotos.iter().enumerate() {
        println!("foto {}: {}x{}", i + 1, f.ancho, f.alto);
        std::fs::write(
            carpeta.join(format!("hoja-marco-{}.png", i + 1)),
            pixpin_codec::imagen::codificar_png(f).unwrap(),
        )
        .unwrap();
        // La forma del marco (sus lados en la hoja), con todos los pixeles que caben.
        let m = &lista[i];
        let (k_foto, k_marco) = (f.ancho as f64 / f.alto as f64, m.ancho() / m.alto());
        assert!(
            (k_foto / k_marco - 1.0).abs() < 0.05,
            "foto {} {k_foto} y marco {k_marco}",
            i + 1
        );
        assert!(
            f.ancho as f64 > m.ancho() * 1.5,
            "acercada para llenar la ventana"
        );
        // Sin la raya azul del marco en la foto.
        let azules = f
            .pixeles
            .chunks_exact(4)
            .filter(|p| p[2] > 180 && p[0] < 40 && p[1] > 90 && p[1] < 140)
            .count();
        assert!(
            azules * 200 < f.pixeles.len() / 4,
            "foto {}: {azules} pixeles del azul de los marcos",
            i + 1
        );
    }
    std::thread::sleep(Duration::from_millis(400));
    if let Some(r) = crate::marco_imprimir::foto_de_pantalla(prueba::ventana().unwrap().1) {
        std::fs::write(
            carpeta.join("hoja-marcos-despues.png"),
            pixpin_codec::imagen::codificar_png(&r).unwrap(),
        )
        .unwrap();
    }

    // Las fichas 1, 2, 3 de abajo: pulsar la 2 lleva al marco 2, acercado
    // hasta llenar las celdas (como en el modelo 3D y el PDF).
    let (fichas, zoom_antes, modos) = prueba::vista();
    assert_eq!(fichas.len(), 3, "una ficha por marco");
    assert!(modos[2], "con el modo de los marcos puesto");
    let (fx, fy) = fichas[1];
    pixpin_shell::overlay::simular_raton(h, 0, fx, fy);
    pausa();
    pixpin_shell::overlay::simular_raton(h, 2, fx, fy);
    esperar(&|| (prueba::vista().1 - zoom_antes).abs() > 0.2);
    std::thread::sleep(Duration::from_millis(400));
    let guardado = std::fs::read_to_string(crate::marcos_doc::ruta(&ruta)).unwrap();
    assert_eq!(
        serde_json::from_str::<Vec<crate::marcos_doc::Marco>>(&guardado)
            .unwrap()
            .len(),
        3,
        "pulsar una ficha no pone otro marco"
    );
    if let Some(r) = crate::marco_imprimir::foto_de_pantalla(prueba::ventana().unwrap().1) {
        std::fs::write(
            carpeta.join("hoja-marcos-ficha2.png"),
            pixpin_codec::imagen::codificar_png(&r).unwrap(),
        )
        .unwrap();
    }

    // Las teclas de todos los visores: M la tira del marcador, Esc la
    // cierra (Ctrl+M tambien la abre y M la cierra); A el lapiz, Esc sale;
    // I entra y sale de los marcos.
    let tecla = |vk: u32| prueba::teclear(vk, false);
    prueba::teclear(0x4D, true);
    esperar(&|| prueba::vista().2[0]);
    assert!(
        !prueba::vista().2[2],
        "la tira del marcador saca de los marcos"
    );
    tecla(0x4D);
    esperar(&|| !prueba::vista().2[0]);
    tecla(0x4D);
    esperar(&|| prueba::vista().2[0]);
    std::thread::sleep(Duration::from_millis(300));
    if let Some(r) = crate::marco_imprimir::foto_de_pantalla(prueba::ventana().unwrap().1) {
        std::fs::write(
            carpeta.join("hoja-tecla-m.png"),
            pixpin_codec::imagen::codificar_png(&r).unwrap(),
        )
        .unwrap();
    }
    tecla(0x1B);
    esperar(&|| !prueba::vista().2[0]);
    tecla(0x41);
    esperar(&|| prueba::vista().2[1]);
    std::thread::sleep(Duration::from_millis(300));
    if let Some(r) = crate::marco_imprimir::foto_de_pantalla(prueba::ventana().unwrap().1) {
        std::fs::write(
            carpeta.join("hoja-tecla-a.png"),
            pixpin_codec::imagen::codificar_png(&r).unwrap(),
        )
        .unwrap();
    }
    tecla(0x1B);
    esperar(&|| !prueba::vista().2[1]);
    tecla(0x49);
    esperar(&|| prueba::vista().2[2]);
    tecla(0x49);
    esperar(&|| !prueba::vista().2[2]);
    prueba::ordenar(prueba::CERRAR);
    std::thread::sleep(Duration::from_millis(600));
    let _ = std::fs::remove_dir_all(&dir);
}

/// **El globo del riel no se corta** (nombre y tecla del boton bajo el
/// raton, a su izquierda), con los marcos puestos y sus fichas abajo:
/// pintado fuera de pantalla, sin raton de verdad. Deja
/// `hoja-riel-globo-fuera.png` en `PIXPIN_MUESTRAS`.
#[test]
#[ignore = "necesita GPU"]
fn el_globo_del_riel_sale_entero() {
    use pixpin_ui::riel_marcas::DestinoRiel;
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d.d3d()).expect("motor");
    let textos = Catalogo::nuevo(Idioma::Espanol);
    let mut libro = libro_de_tabla(300, 40);
    let mut est = abierta_de_prueba(&mut libro, 900, 700);
    est.marcos.lista.push(crate::marcos_doc::Marco {
        caja: [100.0, 60.0, 300.0, 200.0],
        parte: 0,
        vista: String::new(),
    });
    est.marcos.alternar();
    let r = est.riel().imprimir;
    est.raton = (
        (r.x + r.ancho as i32 / 2) as f32,
        (r.y + r.alto as i32 / 2) as f32,
    );
    est.mirar_riel();
    assert_eq!(
        est.encima,
        DestinoRiel::Imprimir,
        "el raton, sobre el marco del riel"
    );
    let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), 900, 700).expect("superficie");
    preparar(&mut est, &motor);
    motor
        .dibujar(&fuera.destino, |p| pintar(p, &mut est, &textos))
        .expect("pintar");
    fuera.esperar_gpu().expect("gpu");
    let (a, b, pixeles) = fuera.leer_rgba().expect("leer");
    let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let salida = carpeta.join("hoja-riel-globo-fuera.png");
    std::fs::write(
        &salida,
        pixpin_codec::imagen::codificar_png(&ImagenRgba {
            ancho: a,
            alto: b,
            pixeles,
        })
        .expect("png"),
    )
    .expect("guardar");
    println!("{}", salida.display());
}
