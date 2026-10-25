//! **El motor con el que se pasan a texto las notas y las grabaciones**
//! (`[voz] motor` de los ajustes): Whisper *base*, el de siempre, o
//! Qwen3-ASR-0.6B, que se baja aparte (ver `docs/voz-qwen3.md`).
//!
//! «Pasar a texto» llama a `pixpin_voz::transcribir_con`, que mira lo que se
//! le dice aqui: con Qwen3-ASR elegido y bajado transcribe Qwen3-ASR; si no
//! esta bajado o falla, Whisper. Nadie mas tiene que saber cual es.
//!
//! La descarga es opt-in y se avisa antes de lo que pesa, la memoria que
//! pide y su licencia (Apache-2.0): elegir Qwen3-ASR en Ajustes la empieza en
//! un hilo, con su avance en la propia fila; nunca se baja sola. Solo se
//! ofrece en equipos con ~4 GB o mas ([`pixpin_voz::qwen3::se_ofrece`]).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use fluent_bundle::FluentArgs;
use pixpin_store::ajustes::{Ajustes, MotorDeVoz};
use pixpin_store::{Catalogo, Ubicacion};
use pixpin_voz::qwen3::{self, catalogo};

/// Milesimas de la descarga en curso (0-1000).
static AVANCE: AtomicU32 = AtomicU32::new(0);
static BAJANDO: AtomicBool = AtomicBool::new(false);
static CANCELAR: AtomicBool = AtomicBool::new(false);
/// El ultimo fallo de la descarga, para ensenarlo en la fila.
static FALLO: Mutex<Option<String>> = Mutex::new(None);
/// La memoria fisica del equipo en MB (la fija `turno_pesado` al arrancar).
static MEMORIA_MB: AtomicU64 = AtomicU64::new(0);
/// Sube con cada cambio que la ventana de ajustes tiene que pintar.
static NOVEDADES: AtomicU64 = AtomicU64::new(0);
type Despertar = Arc<dyn Fn() + Send + Sync>;
static DESPERTAR: Mutex<Option<Despertar>> = Mutex::new(None);

/// Para la ventana de ajustes: cambia cuando hay algo nuevo que pintar.
pub fn novedades() -> u64 {
    NOVEDADES.load(Ordering::SeqCst)
}

/// La ventana de ajustes se apunta para que la despierten.
pub fn al_cambiar(f: Despertar) {
    if let Ok(mut d) = DESPERTAR.lock() {
        *d = Some(f);
    }
}

fn avisar() {
    NOVEDADES.fetch_add(1, Ordering::SeqCst);
    if let Some(f) = DESPERTAR.lock().ok().and_then(|d| d.clone()) {
        f();
    }
}

/// La memoria del equipo, para decidir si se ofrece (la apunta
/// `turno_pesado::fijar_equipo`).
pub fn fijar_memoria(ram_fisica_bytes: u64) {
    MEMORIA_MB.store(ram_fisica_bytes / (1024 * 1024), Ordering::Relaxed);
}

/// Si se ofrece Qwen3-ASR en este equipo. Sin saber la memoria (0), si: el
/// usuario lo elige a sabiendas y la fila dice lo que pide.
pub fn se_ofrece() -> bool {
    let mb = MEMORIA_MB.load(Ordering::Relaxed);
    mb == 0 || qwen3::se_ofrece(mb)
}

/// Lo que se le dice a `pixpin-voz` con estos ajustes (pura, para probarla).
pub fn motor_de(a: &Ajustes, se_ofrece: bool) -> pixpin_voz::MotorDeNotas {
    match a.voz.motor {
        MotorDeVoz::Qwen3 if se_ofrece => pixpin_voz::MotorDeNotas::Qwen3,
        _ => pixpin_voz::MotorDeNotas::Whisper,
    }
}

/// **Aplica `[voz] motor`**: al arrancar y al cerrar los ajustes con cambios.
pub fn aplicar(ubicacion: &Ubicacion, a: &Ajustes) {
    let m = motor_de(a, se_ofrece());
    tracing::info!(
        motor = ?m,
        qwen3_bajado = qwen3::modelo_listo(ubicacion.raiz()),
        "motor de pasar a texto"
    );
    pixpin_voz::fijar_motor_de_notas(m);
}

/// Si Qwen3-ASR esta bajado entero.
pub fn qwen3_listo(raiz: &Path) -> bool {
    qwen3::modelo_listo(raiz)
}

/// Si hay algo que borrar (entero o a medias).
pub fn qwen3_bajado_algo(raiz: &Path) -> bool {
    qwen3::algo_bajado(raiz)
}

pub fn bajando() -> bool {
    BAJANDO.load(Ordering::Relaxed)
}

/// **Empieza a bajar Qwen3-ASR** en un hilo (si no esta ya en ello). Lo
/// configurado no cambia: en cuanto estan los ficheros, la siguiente nota
/// ya va con Qwen3-ASR, sin reiniciar.
pub fn empezar_descarga(raiz: PathBuf) {
    if BAJANDO.swap(true, Ordering::SeqCst) {
        return;
    }
    CANCELAR.store(false, Ordering::SeqCst);
    AVANCE.store(0, Ordering::Relaxed);
    if let Ok(mut f) = FALLO.lock() {
        *f = None;
    }
    let lanzado = std::thread::Builder::new()
        .name("bajar-qwen3-asr".into())
        .spawn(move || {
            pixpin_shell::prioridad::hilo_por_debajo_de_lo_normal();
            let t = std::time::Instant::now();
            let r = qwen3::motor::bajar_modelo(
                &raiz,
                &mut |a| {
                    let nuevo = (a.clamp(0.0, 1.0) * 1000.0) as u32;
                    if AVANCE.swap(nuevo, Ordering::Relaxed) / 10 != nuevo / 10 {
                        avisar();
                    }
                },
                &CANCELAR,
            );
            match r {
                Ok(()) => tracing::info!(segundos = t.elapsed().as_secs(), "Qwen3-ASR bajado"),
                Err(pixpin_voz::ErrorVoz::Cancelada) => {
                    tracing::info!("descarga de Qwen3-ASR cancelada")
                }
                Err(e) => {
                    tracing::warn!(error = %e, "no se pudo bajar Qwen3-ASR");
                    if let Ok(mut f) = FALLO.lock() {
                        *f = Some(e.to_string());
                    }
                }
            }
            BAJANDO.store(false, Ordering::SeqCst);
            avisar();
        });
    if lanzado.is_err() {
        BAJANDO.store(false, Ordering::SeqCst);
    }
}

/// Corta la descarga en curso (volver a Whisper, cerrar la app).
pub fn cancelar_descarga() {
    CANCELAR.store(true, Ordering::SeqCst);
}

/// Borra lo bajado (el runtime se queda: lo usan Whisper e Indra).
pub fn borrar(raiz: &Path) {
    cancelar_descarga();
    if let Err(e) = qwen3::borrar(raiz) {
        tracing::warn!(?e, "no se pudo borrar Qwen3-ASR");
    }
    avisar();
}

/// Los numeros que se ensenan en la fila: lo que falta por bajar, la
/// memoria que pide y la licencia.
fn args_de(raiz: &Path) -> FluentArgs<'static> {
    let mut args = FluentArgs::new();
    args.set("megas", qwen3::megas_a_bajar(raiz) as i64);
    args.set("total", catalogo::megas() as i64);
    args.set(
        "ram",
        format!("{:.1}", catalogo::MEGAS_DE_RAM as f32 / 1000.0).replace('.', ","),
    );
    args.set("licencia", catalogo::LICENCIA);
    args
}

/// La linea de ayuda de la fila del motor: que hay, que se esta bajando o
/// que fallo.
pub fn estado(t: &Catalogo, raiz: &Path, elegido: MotorDeVoz) -> String {
    if bajando() {
        let mut args = FluentArgs::new();
        args.set("por-ciento", (AVANCE.load(Ordering::Relaxed) / 10) as i64);
        return t.t_args("ajustes2-voz-motor-bajando", &args);
    }
    if let Some(f) = FALLO.lock().ok().and_then(|f| f.clone()) {
        let mut args = FluentArgs::new();
        args.set("fallo", f);
        return t.t_args("ajustes2-voz-motor-fallo", &args);
    }
    if !se_ofrece() {
        return t.t("ajustes2-ayuda-voz-motor-sin-memoria");
    }
    match (elegido, qwen3_listo(raiz)) {
        (MotorDeVoz::Qwen3, true) => {
            t.t_args("ajustes2-ayuda-voz-motor-qwen3-listo", &args_de(raiz))
        }
        (_, true) => t.t("ajustes2-ayuda-voz-motor-whisper-con-qwen3"),
        (_, false) => t.t_args("ajustes2-ayuda-voz-motor", &args_de(raiz)),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn de_fabrica_whisper_y_qwen3_solo_si_se_eligio_y_se_ofrece() {
        let mut a = Ajustes::default();
        assert_eq!(motor_de(&a, true), pixpin_voz::MotorDeNotas::Whisper);
        a.voz.motor = MotorDeVoz::Qwen3;
        assert_eq!(motor_de(&a, true), pixpin_voz::MotorDeNotas::Qwen3);
        assert_eq!(
            motor_de(&a, false),
            pixpin_voz::MotorDeNotas::Whisper,
            "en un equipo sin memoria, Whisper aunque el TOML diga otra cosa"
        );
    }
}
