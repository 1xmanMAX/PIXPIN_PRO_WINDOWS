//! Pruebas del visor de hojas con GPU y libros de verdad (ignoradas: piden
//! un `.xlsx` del usuario). La logica pura se prueba en `logica.rs`.

use super::*;
use pixpin_codec::ImagenRgba;
use pixpin_render::fuera_de_pantalla::FueraDePantalla;
use std::path::PathBuf;

fn iniciar_registro() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
}

fn estado<'l>(libro: &'l Libro, e: f32, ancho: u32, alto: u32) -> Estado<'l> {
    Estado {
        libro,
        hojas: (0..libro.hojas.len()).map(|_| None).collect(),
        activa: libro.hoja_activa.min(libro.hojas.len().saturating_sub(1)),
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
    let libro = Libro::abrir(&ruta, regional_del_usuario(Idioma::Espanol)).expect("abre");
    let t_libro = t0.elapsed();
    let mut est = estado(&libro, e, ancho, alto);
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
    let visibles: Vec<usize> = (0..libro.hojas.len()).filter(|&i| libro.hojas[i].visibilidad == Visibilidad::Visible).take(6).collect();
    for i in visibles {
        if i == activa {
            continue;
        }
        est.cambiar_hoja(i, &motor);
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
