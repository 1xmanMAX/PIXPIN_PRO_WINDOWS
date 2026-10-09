//! Prueba a mano de `pagina`: abre una ventana con una pagina y cuenta lo
//! que pasa. `cargo run -p pixpin-web --example pagina`.
use pixpin_web::pagina::{OpcionesPagina, Pagina, Suceso};

fn main() {
    // SAFETY: COM de este hilo, una vez.
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
    }
    eprintln!("creando");
    let html: &'static [u8] = b"<!doctype html><body style='background:#222;color:#fff'><h1>Hola</h1><script>chrome.webview.postMessage('listo')</script>";
    let p = Pagina::nueva(OpcionesPagina {
        titulo: "Prueba".into(),
        area: None,
        maximizada: false,
        fondo: (0x13, 0x14, 0x15),
        pagina: html,
        servir: Box::new(|_| None),
    });
    let p = match p {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return;
        }
    };
    eprintln!("creada hwnd={}", p.hwnd());
    let t = std::time::Instant::now();
    while t.elapsed().as_secs() < 6 {
        for s in p.esperar(200) {
            eprintln!("suceso {s:?}");
            if s == Suceso::PideCerrar {
                return;
            }
        }
    }
    eprintln!("fin");
}
