//! **Probar la vista previa de Office a mano** y medir lo que tarda.
//!
//! ```text
//! cargo run --release -p pixpin-shell --example vista_previa -- "C:\ruta\Informe.docx" [otro.xlsx ...]
//! ```
//!
//! Por cada fichero abre una ventana, pide la vista previa dos veces (la
//! primera en frio, la segunda con la fabrica ya caliente), escribe lo que
//! tardo cada una y se cierra sola: unos segundos en pantalla y nada mas.
//! No toca el teclado ni el raton de nadie.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use pixpin_geom::Rect;
use pixpin_shell::overlay::{self, VentanaOverlay};
use pixpin_shell::vista_previa::{self, Carteles, Estado, VistaPrevia};

/// Lo que se espera a que una vista este como mucho.
const TOPE: Duration = Duration::from_secs(12);
/// Lo que se deja vista cada una, para verla.
const A_LA_VISTA: Duration = Duration::from_millis(1200);

fn esperar(t: Duration) {
    let fin = Instant::now() + t;
    while Instant::now() < fin {
        overlay::bombear_pendientes();
        let _ = overlay::tomar_eventos_pendientes();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn una_vez(ruta: &std::path::Path, vuelta: &str) -> Option<Estado> {
    let area = Rect { x: 120, y: 80, ancho: 900, alto: 700 };
    let ventana = VentanaOverlay::nueva_llana(area, "PixPin - vista previa (prueba)").ok()?;
    ventana.poner_siempre_encima(true);
    ventana.mostrar();
    let carteles = Carteles { abriendo: "Abriendo…".into(), tarda: "Tarda mas de lo normal…".into(), fallo: "No se pudo previsualizar".into() };
    let t = Instant::now();
    let dentro = Rect { x: 7, y: 7, ancho: area.ancho - 14, alto: area.alto - 14 };
    let Some(vista) = VistaPrevia::abrir(ventana.handle(), ruta, dentro, carteles) else {
        println!("{vuelta}: sin previsualizador para {}", ruta.display());
        return None;
    };
    vista_previa::pintar_fondo(ventana.handle());
    let mut estado = vista.estado();
    while estado == Estado::Abriendo && t.elapsed() < TOPE {
        overlay::bombear_pendientes();
        let _ = overlay::tomar_eventos_pendientes();
        std::thread::sleep(Duration::from_millis(5));
        estado = vista.estado();
    }
    println!("{vuelta}: {:?} (visto por la app a los {} ms)", estado, t.elapsed().as_millis());
    esperar(A_LA_VISTA);
    // Probar el estirar: la vista sigue a la ventana.
    vista.mover(Rect { x: 7, y: 7, ancho: 700, alto: 500 });
    esperar(Duration::from_millis(300));
    let tc = Instant::now();
    let cerrada = vista.cerrar(Duration::from_secs(2));
    println!("{vuelta}: cerrada={cerrada} en {} ms", tc.elapsed().as_millis());
    if !cerrada {
        ventana.ocultar();
        std::mem::forget(ventana);
    }
    Some(estado)
}

fn main() {
    tracing_subscriber::fmt().with_max_level(tracing::Level::INFO).with_target(false).init();
    let rutas: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if rutas.is_empty() {
        eprintln!("uso: vista_previa <fichero> [fichero...]");
        return;
    }
    for ruta in &rutas {
        println!(
            "== {} (manejador {:?}, de internet: {})",
            ruta.display(),
            vista_previa::manejador_de(ruta),
            vista_previa::bloqueado_por_internet(ruta)
        );
        una_vez(ruta, "primera");
        // Sin ventana en pantalla: el tiempo de preparar un repuesto si su
        // servidor es de un solo uso (Excel).
        esperar(Duration::from_secs(8));
        una_vez(ruta, "segunda");
    }
    vista_previa::soltar_todo(Duration::from_secs(2));
}
