//! Prueba a mano: abrir y cerrar paginas en hilos que luego se acaban, como
//! los pines de la app (cada visor en su hilo). Antes del 19-oct esto tiraba
//! la app dentro de `EmbeddedBrowserWebView.dll` al cerrar un pin.
//! Con la app abierta, otra carpeta de datos:
//! `PIXPIN_WEB_DATOS=%TEMP%\pixpin-web-prueba cargo run -p pixpin-web --example cerrar_hilos`.
//! `SIN_CALENTAR=1`: sin el navegador precalentado.
use pixpin_web::pagina::{OpcionesPagina, Pagina};

fn una(k: usize) {
    // SAFETY: COM de este hilo, se cierra al final como `ComDelHilo`.
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
    }
    eprintln!("{k}: creando");
    let html: &'static [u8] = b"<!doctype html><body><h1>Hola</h1>";
    let p = Pagina::nueva(OpcionesPagina {
        titulo: format!("cerrar-hilos-{k}"),
        // Fuera de la pantalla: no estorba a nadie.
        area: Some(pixpin_geom::Rect { x: -6000, y: -6000, ancho: 400, alto: 300 }),
        maximizada: false,
        fondo: (0x13, 0x14, 0x15),
        pagina: html,
        servir: Box::new(|_| None),
        padre: None,
        oscuro: None,
    });
    match p {
        Ok(p) => {
            let t = std::time::Instant::now();
            while t.elapsed().as_millis() < 1200 {
                let _ = p.esperar(100);
            }
            eprintln!("{k}: cerrando");
            drop(p);
            eprintln!("{k}: cerrada");
        }
        Err(e) => eprintln!("{k}: error {e}"),
    }
    // SAFETY: el CoInitializeEx de arriba.
    unsafe { windows::Win32::System::Com::CoUninitialize() };
    eprintln!("{k}: com cerrado, el hilo se acaba");
}

fn main() {
    if std::env::var_os("SIN_CALENTAR").is_none() {
        eprintln!("precalentando");
        pixpin_web::precalentar();
        std::thread::sleep(std::time::Duration::from_secs(2));
        eprintln!("precalentado");
    }
    for k in 0..8 {
        let t = std::time::Instant::now();
        std::thread::spawn(move || una(k)).join().expect("hilo");
        eprintln!("{k}: abierta y cerrada en {:?}", t.elapsed());
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    eprintln!("sigo vivo tras 8 cierres");
    std::thread::sleep(std::time::Duration::from_secs(5));
    eprintln!("fin sin caerse");
}
