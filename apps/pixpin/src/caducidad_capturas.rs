//! **Las capturas se van solas a la semana** (el usuario, 3-oct): «que
//! todas tengan una fecha de autodestruccion, por ejemplo 1 semana; cada
//! captura se va borrando a la semana a menos que se le ponga que no se
//! borre y se conserve ahi en el chat».
//!
//! Es la misma idea que el buzon del chat (`DIAS_DEL_BUZON`, que viene del
//! movil): lo que no se guarda a proposito se va solo a los siete dias.
//! Tres decisiones, tomadas con el usuario antes de escribir esto:
//!
//! - **Lo que ya habia no se borra de golpe.** Al estrenar la caducidad
//!   habia capturas de hace meses; con la regla a secas se irian todas el
//!   primer dia. Por eso el registro apunta `desde` (la primera vez que se
//!   miro) y una captura caduca a los siete dias de lo MAS TARDE entre su
//!   fecha y `desde`.
//! - **Irse es ir a la papelera de reciclaje de Windows**
//!   (`pixpin_shell::papelera`), no borrar: si se fue algo que hacia falta,
//!   se restaura desde ahi.
//! - **Conservar es mandarla al chat** («Mensajes guardados», como una foto
//!   mas) y apuntarla en [`Registro::conservadas`]: deja de caducar.
//!
//! El registro vive en `<datos>/capturas-caducidad.json`, fuera de la
//! carpeta de las capturas: dentro, cada escritura cambiaria la fecha de la
//! carpeta y la galeria la releeria sin motivo.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::galeria_capturas::{self, Entrada};

/// Lo que aguanta una captura antes de irse sola, de fabrica. Lo que vale
/// de verdad es `[capturas] dias_caducidad` (ventana de ajustes), que se
/// pasa con [`fijar_dias`].
pub const DIAS: i64 = 7;
const DIA_MS: i64 = 86_400_000;

/// Los dias que valen ahora. Cero: no se va ninguna.
static DIAS_ACTUALES: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(DIAS);

/// Lo que diga `[capturas] dias_caducidad`: al arrancar y al cerrar los
/// ajustes. Vale desde el siguiente barrido y el siguiente pintado de la
/// galeria.
pub fn fijar_dias(dias: u32) {
    DIAS_ACTUALES.store(dias as i64, std::sync::atomic::Ordering::Relaxed);
}

fn dias() -> i64 {
    DIAS_ACTUALES.load(std::sync::atomic::Ordering::Relaxed)
}
/// Cada cuanto se barre con la aplicacion abierta.
const CADA: Duration = Duration::from_secs(60 * 60);
const FICHERO: &str = "capturas-caducidad.json";

/// Leer, cambiar y escribir el registro, de uno en uno: lo tocan la galeria
/// (al conservar) y el barrendero, cada uno en su hilo.
static CERROJO: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registro {
    /// La primera vez que se miro la caducidad, en ms UTC. Nada caduca
    /// antes de `desde + DIAS`.
    pub desde: i64,
    /// Los nombres de fichero de las capturas que no se van.
    #[serde(default)]
    pub conservadas: BTreeSet<String>,
}

fn ruta_del_registro(raiz: &Path) -> PathBuf {
    raiz.join(FICHERO)
}

/// Lee el registro; si no esta (la primera vez) lo crea con `desde = ahora`
/// y lo escribe en el acto, para que la cuenta no vuelva a empezar.
pub fn leer(raiz: &Path, ahora: i64) -> Registro {
    let _c = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
    leer_sin_cerrojo(raiz, ahora)
}

fn leer_sin_cerrojo(raiz: &Path, ahora: i64) -> Registro {
    let ruta = ruta_del_registro(raiz);
    if let Ok(texto) = std::fs::read_to_string(&ruta)
        && let Ok(r) = serde_json::from_str::<Registro>(&texto)
    {
        return r;
    }
    let nuevo = Registro {
        desde: ahora,
        conservadas: BTreeSet::new(),
    };
    if ruta.exists() {
        // Estaba pero no se entiende: se aparta, no se pisa a ciegas.
        let _ = std::fs::rename(&ruta, raiz.join(format!("{FICHERO}.roto-{ahora}")));
    }
    if let Err(e) = escribir(raiz, &nuevo) {
        tracing::warn!(?e, "no se pudo escribir el registro de caducidad de las capturas");
    }
    nuevo
}

fn escribir(raiz: &Path, r: &Registro) -> std::io::Result<()> {
    let ruta = ruta_del_registro(raiz);
    let tmp = ruta.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(r).map_err(std::io::Error::other)?)?;
    std::fs::rename(&tmp, &ruta)
}

/// Cambia el registro con el cerrojo cogido y lo escribe.
pub fn cambiar(raiz: &Path, ahora: i64, f: impl FnOnce(&mut Registro)) -> std::io::Result<Registro> {
    let _c = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = leer_sin_cerrojo(raiz, ahora);
    f(&mut r);
    escribir(raiz, &r)?;
    Ok(r)
}

/// El nombre con que se apunta una captura.
pub fn nombre(ruta: &Path) -> String {
    ruta.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

pub fn ms_de(t: SystemTime) -> i64 {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Cuando se va una captura hecha en `cuando` (ms UTC). `None`: no se va.
pub fn se_va_el(r: &Registro, e: &Entrada) -> Option<i64> {
    se_va_el_con(r, e, dias())
}

/// [`se_va_el`] con un plazo dado. Con cero o menos no se va ninguna.
pub fn se_va_el_con(r: &Registro, e: &Entrada, dias: i64) -> Option<i64> {
    if dias <= 0 || r.conservadas.contains(&nombre(&e.ruta)) {
        return None;
    }
    Some(ms_de(e.cuando).max(r.desde) + dias * DIA_MS)
}

/// Las que ya tocan.
pub fn caducadas<'a>(r: &Registro, lista: &'a [Entrada], ahora: i64) -> Vec<&'a Entrada> {
    lista
        .iter()
        .filter(|e| se_va_el(r, e).is_some_and(|t| t <= ahora))
        .collect()
}

/// **Barre**: lo caducado a la papelera de Windows. Devuelve cuantas se
/// fueron. Tambien olvida las conservadas cuyo fichero ya no esta (el
/// numero de `captura-NNNN` se reutiliza, y una nueva con el nombre de una
/// conservada borrada a mano no tiene por que quedarse para siempre).
pub fn barrer(raiz: &Path, ahora: i64) -> usize {
    let lista = galeria_capturas::listar(&galeria_capturas::carpeta_en(raiz));
    let r = leer(raiz, ahora);
    let mut idas = 0;
    for e in caducadas(&r, &lista, ahora) {
        match pixpin_shell::papelera::a_la_papelera_de_reciclaje(&e.ruta) {
            Ok(()) => {
                idas += 1;
                tracing::info!(ruta = %e.ruta.display(), "captura caducada, a la papelera de Windows");
            }
            Err(err) => tracing::warn!(?err, ruta = %e.ruta.display(), "no se pudo llevar a la papelera"),
        }
    }
    let presentes: BTreeSet<String> = lista.iter().map(|e| nombre(&e.ruta)).collect();
    if r.conservadas.iter().any(|n| !presentes.contains(n)) {
        let _ = cambiar(raiz, ahora, |r| r.conservadas.retain(|n| presentes.contains(n)));
    }
    idas
}

/// **El barrendero**: barre al arrancar y luego cada hora, en su hilo.
pub fn vigilar(raiz: PathBuf) {
    let hecho = std::thread::Builder::new()
        .name("caducidad-capturas".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            loop {
                let idas = barrer(&raiz, pixpin_shell::entorno::ahora_utc_ms());
                if idas > 0 {
                    tracing::info!(idas, "capturas caducadas barridas");
                }
                std::thread::sleep(CADA);
            }
        });
    if let Err(e) = hecho {
        tracing::warn!(?e, "no se pudo lanzar el barrendero de capturas");
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn entrada(nombre: &str, ms: i64) -> Entrada {
        Entrada {
            ruta: PathBuf::from(format!(r"C:\d\capturas\{nombre}")),
            cuando: SystemTime::UNIX_EPOCH + Duration::from_millis(ms as u64),
            bytes: 1,
        }
    }

    fn registro(desde: i64) -> Registro {
        Registro {
            desde,
            conservadas: BTreeSet::new(),
        }
    }

    #[test]
    fn una_captura_nueva_se_va_a_los_siete_dias_de_hacerse() {
        let r = registro(0);
        let e = entrada("a.png", 10 * DIA_MS);
        assert_eq!(se_va_el(&r, &e), Some(17 * DIA_MS));
        assert!(caducadas(&r, std::slice::from_ref(&e), 17 * DIA_MS - 1).is_empty());
        assert_eq!(caducadas(&r, std::slice::from_ref(&e), 17 * DIA_MS).len(), 1);
    }

    #[test]
    fn las_que_ya_habia_cuentan_la_semana_desde_el_estreno_y_no_se_van_de_golpe() {
        // Estreno el dia 100; una captura de hace meses (dia 3).
        let r = registro(100 * DIA_MS);
        let vieja = entrada("vieja.png", 3 * DIA_MS);
        assert!(caducadas(&r, std::slice::from_ref(&vieja), 100 * DIA_MS).is_empty(), "el primer dia no");
        assert_eq!(se_va_el(&r, &vieja), Some(107 * DIA_MS));
    }

    #[test]
    fn el_plazo_lo_pone_el_ajuste_y_cero_es_nunca() {
        let r = registro(0);
        let e = entrada("a.png", 10 * DIA_MS);
        assert_eq!(se_va_el_con(&r, &e, 30), Some(40 * DIA_MS));
        assert_eq!(se_va_el_con(&r, &e, 1), Some(11 * DIA_MS));
        // Caso negativo: con cero no se va, por vieja que sea.
        assert_eq!(se_va_el_con(&r, &e, 0), None);
    }

    #[test]
    fn la_conservada_no_se_va_nunca() {
        let mut r = registro(0);
        r.conservadas.insert("a.png".into());
        let e = entrada("a.png", 0);
        assert_eq!(se_va_el(&r, &e), None);
        assert!(caducadas(&r, &[e], i64::MAX / 2).is_empty());
    }

    #[test]
    fn el_registro_se_crea_una_vez_y_conserva_su_estreno() {
        let raiz = std::env::temp_dir().join(format!("pixpin-caducidad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        let a = leer(&raiz, 1000);
        assert_eq!(a.desde, 1000);
        // Leido otro dia, el estreno es el de la primera vez.
        assert_eq!(leer(&raiz, 9_999_999).desde, 1000);
        cambiar(&raiz, 5, |r| {
            r.conservadas.insert("x.png".into());
        })
        .unwrap();
        assert!(leer(&raiz, 5).conservadas.contains("x.png"));
        // Un registro roto se aparta y se empieza de nuevo, sin panico.
        std::fs::write(raiz.join(FICHERO), b"{roto").unwrap();
        assert_eq!(leer(&raiz, 77).desde, 77);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn barrer_lleva_lo_caducado_a_la_papelera_y_deja_lo_demas() {
        let raiz = std::env::temp_dir().join(format!("pixpin-caducidad-barrer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let dir = galeria_capturas::carpeta_en(&raiz);
        std::fs::create_dir_all(&dir).unwrap();
        for n in ["vieja.png", "guardada.png", "nueva.png"] {
            std::fs::write(dir.join(n), b"x").unwrap();
        }
        let ahora = ms_de(SystemTime::now());
        // Estreno hace ocho dias: todo lo de antes ya toco.
        cambiar(&raiz, ahora - 8 * DIA_MS, |r| {
            r.desde = ahora - 8 * DIA_MS;
            r.conservadas.insert("guardada.png".into());
        })
        .unwrap();
        // «nueva» se hizo hoy: la de hace ocho dias es «vieja».
        let hace_ocho = SystemTime::now() - Duration::from_secs(8 * 86_400);
        std::fs::File::options()
            .write(true)
            .open(dir.join("vieja.png"))
            .unwrap()
            .set_modified(hace_ocho)
            .unwrap();
        std::fs::File::options()
            .write(true)
            .open(dir.join("guardada.png"))
            .unwrap()
            .set_modified(hace_ocho)
            .unwrap();
        assert_eq!(barrer(&raiz, ahora), 1);
        assert!(!dir.join("vieja.png").exists());
        assert!(dir.join("guardada.png").exists(), "la conservada se queda");
        assert!(dir.join("nueva.png").exists(), "la de hoy se queda");
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
