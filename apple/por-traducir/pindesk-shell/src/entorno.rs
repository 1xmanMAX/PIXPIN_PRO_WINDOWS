//! Lo que hay que preguntarle a Windows antes de arrancar.
//!
//! Casi todo vive ahora en `pindesk_sistema::entorno`, que tiene version de
//! Windows y portable (para que `pindesk-store` y `pindesk-proyecto` compilen
//! tambien en Mac). Se reexporta aqui con el nombre de siempre. Solo se
//! queda lo que es del escritorio de Windows.

pub use pindesk_sistema::entorno::*;

/// Donde esta el puntero, en pixeles fisicos del escritorio virtual.
///
/// Decide en que monitor nace un pin del portapapeles: donde estan los ojos
/// del usuario, no en el principal por defecto.
pub fn posicion_del_cursor() -> pindesk_geom::Punto {
    let mut p = windows::Win32::Foundation::POINT::default();
    // SAFETY: escribe en una variable local propia; sin precondiciones.
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut p);
    }
    pindesk_geom::Punto { x: p.x, y: p.y }
}
