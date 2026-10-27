//! **Lo que quedo bajado del asistente local** (27-oct-2026).
//!
//! El asistente contesta solo con la nube desde ese dia (como en Android):
//! el motor llama.cpp y los modelos (Qwen3.5 2B/0.8B, Gemma 4 E2B) ya no se
//! usan, pero quien los bajo los tiene en
//! `%LOCALAPPDATA%\PixPinMax\modelos`: de 1 a 7 GB. Android los borra solo
//! al arrancar; aqui, por si acaso, **se pregunta una vez**: la tarjeta del
//! chat «Asistente» dice cuanto ocupan y ofrece liberarlo
//! ([`por_avisar`], [`borrar`], [`no_avisar`]), y Ajustes → Asistente tiene
//! el boton mientras sigan ahi ([`ocupan`]).
//!
//! Esa carpeta era SOLO del asistente local: Whisper, Qwen3-ASR, Vosk e
//! Indra viven en la carpeta de datos (`<datos>/whisper`, `<datos>/indra`…)
//! y no se tocan.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// El fichero que apunta «no me lo vuelvas a decir», dentro de la propia
/// carpeta: si se borra la carpeta, se va con ella.
const NO_AVISAR: &str = "no-avisar-pixpin.txt";

/// `%LOCALAPPDATA%\PixPinMax\modelos`, donde la app bajaba el asistente
/// local. Las pruebas la apuntan a otra con [`carpeta_de_prueba`].
pub fn carpeta() -> Option<PathBuf> {
    #[cfg(test)]
    if let Some(r) = DE_PRUEBA.with_borrow(Clone::clone) {
        return Some(r);
    }
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(local).join("PixPinMax").join("modelos"))
}

/// Lo que ocupa todo lo que hay debajo de `c` (sin seguir enlaces).
fn tamano_de(c: &Path) -> u64 {
    let Ok(entradas) = std::fs::read_dir(c) else {
        return 0;
    };
    entradas
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => tamano_de(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map_or(0, |m| m.len()),
            _ => 0,
        })
        .sum()
}

/// Lo ultimo medido: cuando, de que carpeta y cuanto (mirar el disco en
/// cada fotograma no).
static MEDIDO: Mutex<Option<(Instant, PathBuf, u64)>> = Mutex::new(None);

/// **Cuanto ocupan** los modelos viejos, o `None` si no queda nada. Se mide
/// como mucho cada 30 s.
pub fn ocupan() -> Option<u64> {
    let c = carpeta()?;
    let mut g = MEDIDO.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((t, de, b)) = g.as_ref()
        && *de == c
        && t.elapsed() < Duration::from_secs(30)
    {
        return (*b > 0).then_some(*b);
    }
    let b = if c.is_dir() { tamano_de(&c) } else { 0 };
    *g = Some((Instant::now(), c, b));
    (b > 0).then_some(b)
}

/// Lo que ocupan si aun no se dijo «Dejarlos»: lo que ensena la tarjeta.
pub fn por_avisar() -> Option<u64> {
    let b = ocupan()?;
    let dicho = carpeta().is_some_and(|c| c.join(NO_AVISAR).is_file());
    (!dicho).then_some(b)
}

/// «Dejarlos»: la tarjeta no vuelve a salir (el boton de Ajustes sigue).
pub fn no_avisar() {
    let Some(c) = carpeta() else { return };
    let texto = "PixPin Max ya no usa esta carpeta: el asistente contesta con la nube.\r\n\
                 Se puede borrar entera (Ajustes > Asistente > Liberar espacio).\r\n";
    if let Err(e) = std::fs::write(c.join(NO_AVISAR), texto) {
        tracing::warn!(?e, "asistente: no se pudo apuntar que no se avise");
    }
}

/// **Borra la carpeta entera** y devuelve lo liberado. Solo esa carpeta:
/// es la que la app creo para el asistente local.
pub fn borrar() -> std::io::Result<u64> {
    let Some(c) = carpeta() else {
        return Ok(0);
    };
    let b = if c.is_dir() { tamano_de(&c) } else { 0 };
    match std::fs::remove_dir_all(&c) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            // Lo que se pudo borrar ya no cuenta: que se vuelva a medir.
            olvidar();
            return Err(e);
        }
    }
    olvidar();
    tracing::info!(bytes = b, "asistente: modelos locales borrados");
    Ok(b)
}

/// Que la siguiente pregunta mire el disco otra vez.
fn olvidar() {
    if let Ok(mut g) = MEDIDO.lock() {
        *g = None;
    }
}

/// «1,9 GB», «740 MB»: como lo dicen la tarjeta y los ajustes.
pub fn tamano(bytes: u64) -> String {
    let mb = bytes as f64 / 1_000_000.0;
    if mb >= 1000.0 {
        format!("{:.1} GB", mb / 1000.0).replace('.', ",")
    } else {
        format!("{:.0} MB", mb.max(1.0))
    }
}

#[cfg(test)]
thread_local! {
    static DE_PRUEBA: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Las pruebas apuntan la carpeta a otra sin tocar el entorno.
#[cfg(test)]
pub fn carpeta_de_prueba(r: Option<PathBuf>) {
    olvidar();
    DE_PRUEBA.with_borrow_mut(|x| *x = r);
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_avisa_una_vez_y_borrar_libera_el_espacio() {
        let c = std::env::temp_dir().join(format!("pixpin-modelos-viejos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&c);
        carpeta_de_prueba(Some(c.clone()));
        // Sin carpeta, nada que decir.
        assert_eq!(ocupan(), None);
        assert_eq!(por_avisar(), None);
        assert_eq!(borrar().unwrap(), 0);
        // Lo que dejaba la app: el motor y un modelo.
        std::fs::create_dir_all(c.join("llama.cpp").join("b11580")).unwrap();
        std::fs::create_dir_all(c.join("qwen3.5-2b")).unwrap();
        std::fs::write(c.join("llama.cpp/b11580/llama-server.exe"), [0u8; 300]).unwrap();
        std::fs::write(c.join("qwen3.5-2b/Qwen3.5-2B-Q4_K_M.gguf"), [0u8; 700]).unwrap();
        olvidar();
        assert_eq!(ocupan(), Some(1000));
        assert_eq!(por_avisar(), Some(1000));
        // «Dejarlos»: la tarjeta se calla, el boton de Ajustes sigue.
        no_avisar();
        olvidar();
        assert_eq!(por_avisar(), None);
        assert!(ocupan().is_some_and(|b| b > 1000));
        // «Liberar espacio»: la carpeta entera fuera.
        assert!(borrar().unwrap() > 1000);
        assert!(!c.exists());
        assert_eq!(ocupan(), None);
        carpeta_de_prueba(None);
    }

    #[test]
    fn el_tamano_como_lo_dice_la_tarjeta() {
        assert_eq!(tamano(1_981_062_350), "2,0 GB");
        assert_eq!(tamano(737_504_352), "738 MB");
        assert_eq!(tamano(10), "1 MB");
    }
}
