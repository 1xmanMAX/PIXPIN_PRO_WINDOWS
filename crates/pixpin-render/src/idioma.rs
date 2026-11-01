//! **El idioma de la interfaz, para DirectWrite.**
//!
//! Dos cosas dependen de el al pintar un rotulo:
//!
//! - El *locale* del formato de texto (`CreateTextFormat`). Con el, la
//!   reserva de letras del sistema (la que DirectWrite usa sola en cada
//!   `IDWriteTextLayout` desde Windows 8.1) elige la letra de cada
//!   escritura que Segoe UI no tiene: Microsoft YaHei para el chino, Yu
//!   Gothic para el japones, Malgun Gothic para el coreano, Nirmala UI para
//!   el devanagari. Los ideogramas Han son los mismos puntos de Unicode en
//!   chino y en japones, pero se dibujan distinto: sin el locale, un chino
//!   leeria sus caracteres con formas japonesas. El cirilico y el arabe
//!   estan en Segoe UI.
//! - La direccion de lectura: un rotulo en arabe va de derecha a izquierda
//!   (ver [`de_derecha_a_izquierda`]).
//!
//! El ejecutable lo fija una vez al arrancar ([`fijar`]), con el idioma ya
//! resuelto; cambiar de idioma pide reiniciar, como siempre. Sin fijar,
//! `es-ES` y de izquierda a derecha: lo de antes.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::core::HSTRING;

static LOCALE: OnceLock<HSTRING> = OnceLock::new();
static INTERFAZ_RTL: AtomicBool = AtomicBool::new(false);

/// Fija el idioma de la interfaz: su etiqueta BCP 47 (`ja-JP`, `ar-SA`) y
/// si se escribe de derecha a izquierda. Solo vale la primera vez.
pub fn fijar(etiqueta: &str, rtl: bool) {
    let _ = LOCALE.set(HSTRING::from(etiqueta));
    INTERFAZ_RTL.store(rtl, Ordering::Relaxed);
}

/// El locale para `CreateTextFormat`.
pub(crate) fn locale() -> &'static HSTRING {
    LOCALE.get_or_init(|| HSTRING::from("es-ES"))
}

/// Si la interfaz es de derecha a izquierda (arabe).
pub fn interfaz_rtl() -> bool {
    INTERFAZ_RTL.load(Ordering::Relaxed)
}

/// Si un caracter es de una escritura de derecha a izquierda (hebreo,
/// arabe, siriaco, thaana, n'ko y sus formas de presentacion).
fn es_rtl(c: char) -> bool {
    matches!(c as u32,
        0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFE | 0x1E800..=0x1EFFF)
}

/// **La direccion de un rotulo**: la de su primera letra con direccion
/// (la regla P2 del algoritmo bidireccional de Unicode, el `dir=auto` de
/// HTML), y con la interfaz en arabe, de derecha a izquierda en cuanto el
/// texto lleve algo en arabe: «PixPin Max مرحبا» tambien se lee desde la
/// derecha. Un texto todo latino (un nombre de fichero) sigue de izquierda
/// a derecha aunque la interfaz sea arabe.
pub fn de_derecha_a_izquierda(texto: &str, interfaz_rtl: bool) -> bool {
    // Las letras arabes tambien son `is_alphabetic`: se miran antes.
    let primera = texto
        .chars()
        .find(|c| es_rtl(*c) || c.is_alphabetic())
        .map(es_rtl);
    match primera {
        Some(true) => true,
        _ => interfaz_rtl && texto.chars().any(es_rtl),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_direccion_la_decide_la_primera_letra_y_en_arabe_cualquiera() {
        assert!(!de_derecha_a_izquierda("Hola", false));
        assert!(!de_derecha_a_izquierda("", true));
        assert!(de_derecha_a_izquierda("مرحبا", false));
        assert!(de_derecha_a_izquierda("12 مرحبا", false));
        // Empieza en latino: con la interfaz en español, de izquierda a
        // derecha; en arabe, de derecha a izquierda.
        assert!(!de_derecha_a_izquierda("PixPin مرحبا", false));
        assert!(de_derecha_a_izquierda("PixPin مرحبا", true));
        // Todo latino sigue igual aunque la interfaz sea arabe.
        assert!(!de_derecha_a_izquierda("captura.png", true));
        assert!(de_derecha_a_izquierda("שלום", false));
    }
}
