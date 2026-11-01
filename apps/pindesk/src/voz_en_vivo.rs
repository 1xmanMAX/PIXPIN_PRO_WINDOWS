//! **Con que se escucha en vivo una reunion** (`[voz] en_vivo`, 1-nov-2026):
//! lo de siempre (el reconocedor de Windows o Whisper *base*, frase a frase)
//! o **Vosk**, que se baja aparte (unos 55 MB: la biblioteca y el modelo
//! pequeno del idioma; `pindesk_voz::vosk_instalar`) y da lo que se va oyendo
//! mientras se habla y la frase a las decimas de callarse
//! (`pindesk_voz::vosk_vivo`, medido en `docs/probar-ideas-sin-interfaz.md`).
//!
//! Como Qwen3-ASR (`voz_motor`): elegir Vosk en Ajustes empieza la descarga
//! en un hilo, con su avance en la propia fila; nunca se baja sola. Sin
//! bajar, en un idioma que no tiene (hoy castellano e ingles) o si falla al
//! cargarse, la escucha sigue con lo de siempre.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use fluent_bundle::FluentArgs;
use pindesk_store::ajustes::{Ajustes, MotorEnVivo};
use pindesk_store::{Catalogo, Idioma};
use pindesk_voz::vosk_instalar;

/// Vosk elegido en Ajustes (lo fija [`aplicar`]).
static ELEGIDO: AtomicBool = AtomicBool::new(false);
/// Milesimas de la descarga en curso (0-1000).
static AVANCE: AtomicU32 = AtomicU32::new(0);
static BAJANDO: AtomicBool = AtomicBool::new(false);
static CANCELAR: AtomicBool = AtomicBool::new(false);
static FALLO: Mutex<Option<String>> = Mutex::new(None);

/// **Aplica `[voz] en_vivo`**: al arrancar y al cerrar los ajustes.
pub fn aplicar(a: &Ajustes) {
    ELEGIDO.store(a.voz.en_vivo == MotorEnVivo::Vosk, Ordering::Relaxed);
}

/// Si la escucha en vivo va con Vosk en ese idioma: elegido y bajado.
pub fn con_vosk(raiz: &Path, idioma: Idioma) -> bool {
    ELEGIDO.load(Ordering::Relaxed) && vosk_instalar::listo(raiz, crate::voz::idioma_de_voz(idioma))
}

pub fn bajando() -> bool {
    BAJANDO.load(Ordering::Relaxed)
}

/// **Empieza a bajar Vosk** para ese idioma en un hilo (si no esta ya en
/// ello o no hay nada que bajar).
pub fn empezar_descarga(raiz: PathBuf, idioma: Idioma) {
    let voz = crate::voz::idioma_de_voz(idioma);
    if vosk_instalar::falta_bajar(&raiz, voz) == 0 || BAJANDO.swap(true, Ordering::SeqCst) {
        return;
    }
    CANCELAR.store(false, Ordering::SeqCst);
    AVANCE.store(0, Ordering::Relaxed);
    if let Ok(mut f) = FALLO.lock() {
        *f = None;
    }
    let lanzado = std::thread::Builder::new()
        .name("bajar-vosk".into())
        .spawn(move || {
            pindesk_shell::prioridad::hilo_por_debajo_de_lo_normal();
            let r = vosk_instalar::bajar(
                &raiz,
                voz,
                &mut |a| {
                    let nuevo = (a.clamp(0.0, 1.0) * 1000.0) as u32;
                    if AVANCE.swap(nuevo, Ordering::Relaxed) / 10 != nuevo / 10 {
                        crate::voz_motor::avisar();
                    }
                },
                &CANCELAR,
            );
            match r {
                Ok(()) => tracing::info!(idioma = voz, "Vosk bajado para escuchar en vivo"),
                Err(pindesk_voz::ErrorVoz::Cancelada) => {
                    tracing::info!("descarga de Vosk cancelada")
                }
                Err(e) => {
                    tracing::warn!(error = %e, "no se pudo bajar Vosk");
                    if let Ok(mut f) = FALLO.lock() {
                        *f = Some(e.to_string());
                    }
                }
            }
            BAJANDO.store(false, Ordering::SeqCst);
            crate::voz_motor::avisar();
        });
    if lanzado.is_err() {
        BAJANDO.store(false, Ordering::SeqCst);
    }
}

/// Corta la descarga en curso (volver a lo de siempre, cerrar la app).
pub fn cancelar_descarga() {
    CANCELAR.store(true, Ordering::SeqCst);
}

/// Si hay algo bajado que borrar.
pub fn algo_bajado(raiz: &Path) -> bool {
    vosk_instalar::algo_bajado(raiz)
}

/// Borra lo bajado. Si una escucha lo tiene cargado, Windows no deja borrar
/// la DLL: se dice en el registro y queda para la proxima vez.
pub fn borrar(raiz: &Path) {
    cancelar_descarga();
    if let Err(e) = vosk_instalar::borrar(raiz) {
        tracing::warn!(?e, "no se pudo borrar Vosk");
    }
    crate::voz_motor::avisar();
}

/// La linea de ayuda de la fila: que hay, que se esta bajando o que fallo.
pub fn estado(t: &Catalogo, raiz: &Path, elegido: MotorEnVivo, idioma: Idioma) -> String {
    if bajando() {
        let mut args = FluentArgs::new();
        args.set("por-ciento", (AVANCE.load(Ordering::Relaxed) / 10) as i64);
        return t.t_args("ajustes2-voz-en-vivo-bajando", &args);
    }
    if let Some(f) = FALLO.lock().ok().and_then(|f| f.clone()) {
        let mut args = FluentArgs::new();
        args.set("fallo", f);
        return t.t_args("ajustes2-voz-en-vivo-fallo", &args);
    }
    let voz = crate::voz::idioma_de_voz(idioma);
    if vosk_instalar::modelo_para(voz).is_none() {
        return t.t("ajustes2-ayuda-voz-en-vivo-sin-idioma");
    }
    let falta = vosk_instalar::falta_bajar(raiz, voz);
    match (elegido, falta) {
        (MotorEnVivo::Vosk, 0) => t.t("ajustes2-ayuda-voz-en-vivo-vosk-listo"),
        _ => {
            let mut args = FluentArgs::new();
            args.set("megas", (falta.div_ceil(1_000_000)) as i64);
            t.t_args("ajustes2-ayuda-voz-en-vivo", &args)
        }
    }
}
