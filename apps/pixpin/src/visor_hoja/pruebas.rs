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
    let carpeta = std::env::var_os("PIXPIN_MUESTRAS").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let prefijo = std::env::var("PIXPIN_PREFIJO").unwrap_or_else(|_| "hoja".into());
    let e: f32 = std::env::var("PIXPIN_ESCALA").ok().and_then(|s| s.parse().ok()).unwrap_or(1.25);
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
        motor.dibujar(&fuera.destino, |p| pintar(p, est, &textos)).expect("pintar");
        fuera.esperar_gpu().expect("gpu");
        let dur = t.elapsed();
        let (a, b, pixeles) = fuera.leer_rgba().expect("leer");
        let png = pixpin_codec::imagen::codificar_png(&ImagenRgba { ancho: a, alto: b, pixeles }).expect("png");
        n += 1;
        let salida = carpeta.join(format!("{prefijo}-{n:02}-{que}.png"));
        std::fs::write(&salida, png).expect("guardar");
        println!("{que}: {:.1} ms -> {}", dur.as_secs_f64() * 1000.0, salida.display());
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
    println!("fotogramas lejos: {:.1} y {:.1} ms", d1.as_secs_f64() * 1000.0, d2.as_secs_f64() * 1000.0);
    est.mover(Mov::InicioHoja, false);
    est.mover(Mov::Derecha, true);
    est.mover(Mov::Abajo, true);
    est.mover(Mov::Abajo, true);
    est.raton = (400.0 * e, 300.0 * e);
    est.poner_zoom(1.6, Some(est.raton));
    foto(&mut est, "zoom");
    est.poner_zoom(1.0, None);

    // Cada pestana visible.
    let visibles: Vec<usize> = (0..est.libro.hojas.len()).filter(|&i| est.libro.hojas[i].visibilidad == Visibilidad::Visible).take(6).collect();
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
    assert_eq!(ids(botones_de_ver(true, true)), [BOTON_EDITAR, BOTON_ABRIR_CON], "sin Copiar: Ctrl+C copia");
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
            t.poner(pixpin_proyecto::tabla::Ref { columna: c, fila: f }, &format!("{}", f * 100 + c));
        }
    }
    let bytes = pixpin_proyecto::tabla_xlsx::a_xlsx(&[pixpin_proyecto::tabla_xlsx::HojaParaVer { nombre: "Gastos", tabla: &t, valores: None }], 0).unwrap();
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
    assert_eq!((est.abierta().x, est.abierta().y), (0.0, 0.0), "no se pasa del principio");
    // Hacia abajo y a la derecha: la hoja va 100 px por cada 100 del raton.
    est.raton = (300.0, 200.0);
    est.llevar_mano();
    assert_eq!((est.abierta().x, est.abierta().y), (100.0, 100.0));
    assert_eq!(est.abierta().sel, sel, "arrastrar con el central no elige celdas");
    // Al doble de zoom, la mitad de hoja por el mismo gesto.
    est.gesto = None;
    est.abierta_mut().zoom = 2.0;
    est.raton = (400.0, 300.0);
    est.empezar_mano();
    est.raton = (300.0, 300.0);
    est.llevar_mano();
    assert_eq!(est.abierta().x, 150.0);
    est.gesto = None;
    assert_ne!(est.cursor(), FormaCursorWin::Mover, "al soltar vuelve la flecha");
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
    let carpeta = std::env::var_os("PIXPIN_MUESTRAS").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
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
        t.poner(pixpin_proyecto::tabla::Ref { columna: 1, fila: f }, &format!("{}", f * 3));
    }
    t.estilos.insert("A1".into(), pixpin_proyecto::tabla::EstiloDeCelda { n: true, f: Some("#d0ebff".into()), ..Default::default() });
    let sello = pixpin_proyecto::cuaderno::Sello { cuando: 5, numero: 1, aparato: "PC".into(), proyecto: "p".into() };
    let m = pixpin_proyecto::cuaderno::Mensaje::miniapp("tabla", "Gastos", &t.escribir().unwrap(), &sello);
    let mut ed = crate::hoja_pin::Editable::nuevo(pixpin_store::Ubicacion::Portable { raiz: raiz.clone() }, "p".into(), crate::hoja_pin::Origen::Tabla { mensaje: m });
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
        motor.dibujar(&fuera.destino, |p| pintar(p, est, &textos)).expect("pintar");
        fuera.esperar_gpu().expect("gpu");
        let (a, b, pixeles) = fuera.leer_rgba().expect("leer");
        let png = pixpin_codec::imagen::codificar_png(&ImagenRgba { ancho: a, alto: b, pixeles: pixeles.clone() }).expect("png");
        let salida = carpeta.join(format!("hoja-ver-editar-{que}.png"));
        std::fs::write(&salida, png).expect("guardar");
        println!("{que}: {}", salida.display());
        pixeles
    };
    let ver = foto(&mut est, "1-ver");
    let (x0, y0) = (est.abierta().x, est.abierta().y);
    assert!(est.empezar_a_editar(&textos));
    let editar = foto(&mut est, "2-editar");
    assert_eq!((est.abierta().x, est.abierta().y), (x0, y0), "editar no mueve la hoja");
    // La hoja (sin la barra de formulas, que dice que se edita), igual.
    let z = est.zonas();
    let desde = (z.hoja.y as usize) * ancho as usize * 4;
    let hasta = (z.pestanas.y as usize).min(alto as usize) * ancho as usize * 4;
    let distintos = ver[desde..hasta].chunks(4).zip(editar[desde..hasta].chunks(4)).filter(|(a, b)| a != b).count();
    assert_eq!(distintos, 0, "ver y editar pintan la hoja igual");

    // Escribir en una celda: sale encima, en la letra de la hoja.
    est.abierta_mut().sel = Seleccion::en(4, 1);
    for c in "=B2*2".chars() {
        est.teclear(c);
    }
    foto(&mut est, "3-escribiendo");
    est.hacer(AccionEd::Confirmar, &textos, &motor);
    // Formato a un rango: cursiva, subrayado, resaltar, color y moneda.
    est.abierta_mut().sel = Seleccion { ancla: (1, 0), activa: (4, 1) };
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
    est.hacer(AccionEd::Formato(Formato::Alternar(Interruptor::Tachado)), &textos, &motor);
    assert_eq!((est.abierta().x, est.abierta().y), (x0, y0), "rehacer lo que se ve no mueve la hoja");
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
        est.guardadas.insert(0, Posicion { x: 0.0, y: 5000.0, zoom: 1.0, sel: Seleccion::en(200, 3) });
        est.abrir_hoja_con_digito(0);
        println!("tabla de 40 000 celdas: libro {ms_libro:.1} ms, con su primera pantalla {:.1} ms", t.elapsed().as_secs_f64() * 1000.0);
    }
}

/// El reparto de lo anterior: calcular, escribir el `.xlsx` y leerlo.
#[test]
#[ignore = "mide tiempos"]
fn mide_rehacer_por_partes() {
    let mut t = pixpin_proyecto::tabla::Tabla::default();
    for f in 0..1000 {
        for c in 0..40 {
            t.poner(pixpin_proyecto::tabla::Ref { columna: c, fila: f }, &format!("{}", f * 100 + c));
        }
    }
    let ms = |t: Instant| t.elapsed().as_secs_f64() * 1000.0;
    let t0 = Instant::now();
    let v = pixpin_proyecto::formula::evaluar_todo(&t);
    let calcular = ms(t0);
    let t1 = Instant::now();
    let bytes = pixpin_proyecto::tabla_xlsx::a_xlsx(&[pixpin_proyecto::tabla_xlsx::HojaParaVer { nombre: "G", tabla: &t, valores: Some(&v) }], 0).unwrap();
    let escribir = ms(t1);
    let t2 = Instant::now();
    let libro = Libro::desde_bytes(bytes, Regional::es_pe()).unwrap();
    let mut h = libro.abrir_hoja(0);
    h.cargar_tanda(TANDA_CELDAS);
    println!("calcular {calcular:.1} ms, escribir {escribir:.1} ms, leer {:.1} ms", ms(t2));
}

/// Los botones de la barra al editar, pintados como en la barra del pin
/// (iconos claros sobre oscuro; encendidos, dorados), para mirarlos.
///
/// `PIXPIN_MUESTRAS=<carpeta> cargo test -p pixpin --bin pixpinmax visor_hoja::pruebas::botones_de_editar_pintados -- --ignored`
#[test]
#[ignore = "necesita GPU"]
fn botones_de_editar_pintados() {
    let carpeta = std::env::var_os("PIXPIN_MUESTRAS").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor = MotorRender::nuevo(d.d3d()).expect("motor");
    let mut t = pixpin_proyecto::tabla::Tabla::default();
    t.estilos.insert("A1".into(), pixpin_proyecto::tabla::EstiloDeCelda { n: true, a: Some("c".into()), ..Default::default() });
    let ed = crate::hoja_pin::editor::Editor::nuevo(vec![crate::hoja_pin::editor::HojaEditable::nueva("x".into(), t, None)], 0).unwrap();
    let botones = crate::hoja_pin::botones_de_editar(&ed);
    let (lado, n) = (50u32, botones.len() as u32);
    let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), lado * n, lado).expect("superficie");
    motor
        .dibujar(&fuera.destino, |p| {
            p.limpiar(Color { r: 0.118, g: 0.118, b: 0.125, a: 1.0 });
            for (i, b) in botones.iter().enumerate() {
                let caja = RectF { x: i as f32 * lado as f32 + 10.0, y: 10.0, ancho: 30.0, alto: 30.0 };
                let color = if b.encendido { Color { r: 0.95, g: 0.77, b: 0.33, a: 1.0 } } else { Color { r: 0.96, g: 0.96, b: 0.97, a: 1.0 } };
                p.icono(b.icono, caja, color);
            }
        })
        .expect("pintar");
    fuera.esperar_gpu().expect("gpu");
    let (a, b, pixeles) = fuera.leer_rgba().expect("leer");
    let png = pixpin_codec::imagen::codificar_png(&ImagenRgba { ancho: a, alto: b, pixeles }).expect("png");
    let salida = carpeta.join("hoja-botones-de-editar.png");
    std::fs::write(&salida, png).expect("guardar");
    println!("{}", salida.display());
}
