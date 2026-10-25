//! **El motor de OCR de toda la app** (`[ocr]` de los ajustes).
//!
//! Todo lo que lee texto de una imagen —la captura de texto, «Calca esto»,
//! los pines, la galeria, los PDF escaneados, las herramientas del
//! asistente— llama a `pixpin_ocr::reconocer`, que mira lo que se le dice
//! aqui. Con «Indra» elegido y bajado lee Indra; si no esta bajado o falla,
//! el de Windows. Nadie mas tiene que saber cual es.
//!
//! La descarga es opt-in, como pide la licencia de los pesos (ver
//! `docs/ocr.md`): elegir Indra en Ajustes la empieza en un hilo, con su
//! avance en la propia fila; nunca se baja sola.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use fluent_bundle::FluentArgs;
use pixpin_store::ajustes::{Ajustes, MotorOcr};
use pixpin_store::{Catalogo, Ubicacion};

/// Milesimas de la descarga en curso (0-1000).
static AVANCE: AtomicU32 = AtomicU32::new(0);
static BAJANDO: AtomicBool = AtomicBool::new(false);
static CANCELAR: AtomicBool = AtomicBool::new(false);
/// El ultimo fallo de la descarga, para ensenarlo en la fila.
static FALLO: Mutex<Option<String>> = Mutex::new(None);
/// La raiz de datos, para lo que no recibe la `Ubicacion`.
static RAIZ: Mutex<Option<PathBuf>> = Mutex::new(None);
/// Sube con cada cambio que la ventana de ajustes tiene que pintar (cada
/// punto del avance, el final, un fallo).
static NOVEDADES: AtomicU64 = AtomicU64::new(0);
type Despertar = Arc<dyn Fn() + Send + Sync>;
/// Quien quiere enterarse (la ventana de ajustes abierta).
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

fn junto_al_exe() -> Option<PathBuf> {
    pixpin_shell::entorno::directorio_del_ejecutable().ok()
}

/// Hilos para Indra: dos en el modo cuidadoso (4 GB o nivel Ligero), hasta
/// cuatro si no. El i3 de referencia tiene cuatro hilos y lo demas de
/// PixPin sigue funcionando mientras lee.
fn hilos() -> usize {
    if crate::turno_pesado::cuidadoso() {
        2
    } else {
        std::thread::available_parallelism()
            .map_or(2, |n| n.get())
            .clamp(1, 4)
    }
}

/// Lo que se le dice a `pixpin-ocr` con estos ajustes (pura, para probarla).
pub fn config_de(
    a: &Ajustes,
    raiz: &Path,
    exe: Option<PathBuf>,
    hilos: usize,
) -> pixpin_ocr::Config {
    pixpin_ocr::Config {
        motor: match a.ocr.motor {
            MotorOcr::Windows => pixpin_ocr::Motor::Windows,
            MotorOcr::Indra => pixpin_ocr::Motor::Indra,
        },
        raiz: Some(raiz.to_path_buf()),
        junto_al_exe: exe,
        corregir: a.ocr.corregir,
        vocabulario: a.ocr.vocabulario.clone(),
        hilos,
    }
}

/// **Aplica `[ocr]`**: al arrancar y al cerrar los ajustes con cambios.
pub fn aplicar(ubicacion: &Ubicacion, a: &Ajustes) {
    if let Ok(mut r) = RAIZ.lock() {
        *r = Some(ubicacion.raiz().to_path_buf());
    }
    let c = config_de(a, ubicacion.raiz(), junto_al_exe(), hilos());
    tracing::info!(
        motor = ?c.motor,
        bajado = pixpin_ocr::indra::listo(ubicacion.raiz()),
        hilos = c.hilos,
        "motor de OCR"
    );
    pixpin_ocr::configurar(c);
}

/// Si el lector Indra esta bajado.
pub fn indra_listo(raiz: &Path) -> bool {
    pixpin_ocr::indra::listo(raiz)
}

pub fn bajando() -> bool {
    BAJANDO.load(Ordering::Relaxed)
}

/// **Empieza a bajar Indra** en un hilo (si no esta ya en ello). Al acabar
/// vuelve a aplicar lo configurado, para que la siguiente lectura ya sea con
/// Indra sin reiniciar.
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
        .name("bajar-indra".into())
        .spawn(move || {
            let t = std::time::Instant::now();
            let exe = junto_al_exe();
            let r = pixpin_ocr::indra::bajar(
                &raiz,
                exe.as_deref(),
                &mut |a| {
                    let nuevo = (a.clamp(0.0, 1.0) * 1000.0) as u32;
                    // Un aviso por punto entero, no por cada trozo de 64 KB.
                    if AVANCE.swap(nuevo, Ordering::Relaxed) / 10 != nuevo / 10 {
                        avisar();
                    }
                },
                &CANCELAR,
            );
            match r {
                Ok(()) => tracing::info!(segundos = t.elapsed().as_secs(), "lector Indra bajado"),
                Err(e) => {
                    tracing::warn!(error = %e, "no se pudo bajar el lector Indra");
                    if let Ok(mut f) = FALLO.lock() {
                        *f = Some(e.to_string());
                    }
                }
            }
            // Lo configurado sigue igual: `motor_en_uso` ya ve los ficheros
            // y la siguiente lectura es con Indra, sin reiniciar.
            BAJANDO.store(false, Ordering::SeqCst);
            avisar();
        });
    if lanzado.is_err() {
        BAJANDO.store(false, Ordering::SeqCst);
    }
}

/// Corta la descarga en curso (cerrar la app, volver a «Windows»).
pub fn cancelar_descarga() {
    CANCELAR.store(true, Ordering::SeqCst);
}

/// Borra el lector bajado (los pesos; el runtime se queda para Whisper).
pub fn borrar(raiz: &Path) {
    cancelar_descarga();
    if let Err(e) = pixpin_ocr::indra::borrar(raiz) {
        tracing::warn!(?e, "no se pudo borrar el lector Indra");
    }
    avisar();
}

/// La linea de ayuda de la fila del motor: que hay, que se esta bajando o
/// que fallo.
pub fn estado(t: &Catalogo, raiz: &Path, elegido: MotorOcr) -> String {
    if bajando() {
        let mut args = FluentArgs::new();
        args.set("por-ciento", (AVANCE.load(Ordering::Relaxed) / 10) as i64);
        return t.t_args("ajustes2-ocr-bajando", &args);
    }
    if let Some(f) = FALLO.lock().ok().and_then(|f| f.clone()) {
        let mut args = FluentArgs::new();
        args.set("fallo", f);
        return t.t_args("ajustes2-ocr-fallo", &args);
    }
    match (elegido, indra_listo(raiz)) {
        (MotorOcr::Indra, true) => t.t("ajustes2-ayuda-ocr-indra-listo"),
        (_, listo) => {
            let mut args = FluentArgs::new();
            let exe = junto_al_exe();
            args.set(
                "megas",
                pixpin_ocr::indra::megas_a_bajar(raiz, exe.as_deref()) as i64,
            );
            if listo {
                t.t("ajustes2-ayuda-ocr-windows-con-indra")
            } else {
                t.t_args("ajustes2-ayuda-ocr", &args)
            }
        }
    }
}

/// Hay un calentamiento en marcha (para no lanzar dos hilos a la vez).
static CALENTANDO: AtomicBool = AtomicBool::new(false);

/// Si toca calentar: solo con Indra elegido y bajado, si no esta ya cargado
/// ni cargandose. Con el de Windows, nunca: su comportamiento no cambia.
pub fn debe_calentar(indra_en_uso: bool, cargado: bool, calentando: bool) -> bool {
    indra_en_uso && !cargado && !calentando
}

/// **Calienta el OCR**: con Indra en uso, carga sus modelos en un hilo por
/// debajo de lo normal para que la primera lectura no congele nada.
///
/// Se llama cuando el usuario empieza algo que suele acabar leyendo texto
/// (abrir una captura, pinear una foto), no al arrancar: cargar al arrancar
/// un lector que nadie usa solo para que el vigilante lo suelte a los tres
/// minutos seria gastar 150-250 MB de un equipo de 4 GB para nada. Y no le
/// alarga la vida al lector cargado (`pixpin_ocr::indra::precargar`).
pub fn calentar() {
    let indra = pixpin_ocr::motor_en_uso() == pixpin_ocr::Motor::Indra;
    if !debe_calentar(
        indra,
        indra && pixpin_ocr::indra::cargado(),
        CALENTANDO.load(Ordering::SeqCst),
    ) || CALENTANDO.swap(true, Ordering::SeqCst)
    {
        return;
    }
    let lanzado = std::thread::Builder::new()
        .name("calentar-ocr".into())
        .spawn(|| {
            pixpin_shell::prioridad::hilo_por_debajo_de_lo_normal();
            let t = std::time::Instant::now();
            if pixpin_ocr::precargar() {
                tracing::info!(
                    ms = t.elapsed().as_millis() as u64,
                    "lector Indra precargado"
                );
            }
            CALENTANDO.store(false, Ordering::SeqCst);
        });
    if lanzado.is_err() {
        CALENTANDO.store(false, Ordering::SeqCst);
    }
}

/// La raiz de datos que se aplico (para quien no tiene la `Ubicacion`).
pub fn raiz() -> Option<PathBuf> {
    RAIZ.lock().ok().and_then(|r| r.clone())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn solo_se_calienta_indra_y_una_vez() {
        assert!(debe_calentar(true, false, false));
        assert!(!debe_calentar(false, false, false), "con Windows, nada");
        assert!(!debe_calentar(true, true, false), "ya cargado");
        assert!(!debe_calentar(true, false, true), "ya cargandose");
    }

    #[test]
    fn los_ajustes_pasan_tal_cual_al_ocr() {
        let mut a = Ajustes::default();
        let c = config_de(&a, Path::new("C:/datos"), None, 2);
        assert_eq!(
            c.motor,
            pixpin_ocr::Motor::Windows,
            "de fabrica, el de Windows"
        );
        assert!(c.corregir);
        a.ocr.motor = MotorOcr::Indra;
        a.ocr.vocabulario = "E030".into();
        a.ocr.corregir = false;
        let c = config_de(&a, Path::new("C:/datos"), Some(PathBuf::from("C:/app")), 4);
        assert_eq!(c.motor, pixpin_ocr::Motor::Indra);
        assert_eq!(c.raiz.as_deref(), Some(Path::new("C:/datos")));
        assert_eq!(c.vocabulario, "E030");
        assert!(!c.corregir);
        assert_eq!(c.hilos, 4);
    }
}
