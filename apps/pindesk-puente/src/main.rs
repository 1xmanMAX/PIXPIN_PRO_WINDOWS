//! `pindesk-puente.exe`: ver la cabecera de `lib.rs`.
//!
//! Subsistema de Windows y no de consola: el navegador lo arranca con las
//! tuberias redirigidas, y una consola saldria parpadeando en cada clic.
#![windows_subsystem = "windows"]

use std::path::PathBuf;

fn main() {
    pindesk_sistema::nombre::aceptar_variables_de_antes();
    let dir_exe = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(PathBuf::from));
    // Solo para las pruebas: no manda nada y usa esa carpeta como raiz.
    let en_seco = std::env::var_os("PINDESK_PUENTE_EN_SECO").map(PathBuf::from);
    let Some(mut entorno) =
        pindesk_puente::Entorno::del_equipo(dir_exe.as_deref(), en_seco.clone())
    else {
        std::process::exit(1);
    };
    let mut entrada = std::io::stdin().lock();
    let mut salida = std::io::stdout().lock();
    let codigo = if en_seco.is_some() {
        entorno.ensenar_pedido = true;
        pindesk_puente::correr(
            &mut entrada,
            &mut salida,
            &entorno,
            &mut pindesk_puente::EnSeco,
        )
    } else {
        // Que lo que abra PinDesk (una nota, un pin) pueda pasar delante del
        // navegador, que tiene el foco y nos arranco.
        // SAFETY: sin punteros; si no se puede dar, no hace nada.
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(u32::MAX);
        }
        let mut cartero = pindesk_puente::AlaApp {
            exe: pindesk_puente::exe_de_pindesk(dir_exe.as_deref()),
        };
        pindesk_puente::correr(&mut entrada, &mut salida, &entorno, &mut cartero)
    };
    std::process::exit(codigo);
}
