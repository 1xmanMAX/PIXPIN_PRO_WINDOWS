//! **PinDesk Plantillas** (nombre provisional: `nucleo::NOMBRE_PRODUCTO`):
//! diseñar plantillas por bloques —diarios, planificadores, calendarios,
//! listas, notas— y sacarlas a PDF, PNG, al lienzo o a las Figuras de
//! PinDesk. Ver `docs/plantillas.md`.
//!
//! Este ejecutable es solo la ventana (Direct2D con los crates compartidos de
//! PinDesk); todo lo demas esta en `crates/pindesk-plantillas`. Sin ventana,
//! `--exportar <fichero.pdplantilla> <salida>` exporta y sale.

#![windows_subsystem = "windows"]

mod app;
mod nube;
mod pintar;
mod rutas;

use std::path::Path;

use nucleo::enlace::{self, Accion};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let accion = enlace::de_argumentos(&args);
    if let Accion::Exportar { entrada, salida } = &accion {
        std::process::exit(exportar_sin_ventana(entrada, salida));
    }
    let _com = pindesk_shell::ComDelHilo::iniciar();
    if let Err(e) = app::correr(accion) {
        pindesk_shell::mostrar_error_fatal(nucleo::NOMBRE_PRODUCTO, &e);
    }
}

/// **Exportar sin ventana**: el formato sale de la extension de `salida`
/// (`pdf`, `png`, `svg` o `excalidraw`). 0 si salio, 1 si no.
fn exportar_sin_ventana(entrada: &Path, salida: &Path) -> i32 {
    let Ok(texto) = std::fs::read_to_string(entrada) else {
        return 1;
    };
    let Ok((doc, _)) = nucleo::formato::leer(&texto) else {
        return 1;
    };
    let hoy = app::hoy();
    let ext = salida
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match app::exportar_a(&doc, hoy, 0, &ext, salida) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}
