//! **Tu Google Calendar en PixPin** (18-oct-2026), solo para ver. Lo que
//! eligio el usuario de la integracion: los eventos salen en la rejilla del
//! horario semanal, avisan con la misma llamada que las clases y el
//! asistente contesta «¿que tengo hoy?».
//!
//! La red va en un hilo propio cada 15 minutos (y al conectar): lo leido se
//! guarda en `calendario-google.json` junto a los datos, asi el horario y
//! los avisos lo tienen al instante y sin internet. Quien lo lee
//! ([`eventos_entre`]) nunca espera a Google.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock};
use std::time::Duration;

use pixpin_ia::conexiones::calendario::{self, Evento, GOOGLE_CALENDAR};

/// Cada cuanto se pregunta a Google.
const CADA: Duration = Duration::from_secs(15 * 60);
/// Lo que se trae: de ayer a tres semanas.
const ATRAS_MS: i64 = 2 * 86_400_000;
const ADELANTE_MS: i64 = 21 * 86_400_000;

/// Lo leido la ultima vez (los eventos y cuando).
#[derive(Default)]
struct Cache {
    eventos: Vec<Evento>,
    leido_ms: i64,
    cargada: bool,
}

static CACHE: RwLock<Cache> = RwLock::new(Cache { eventos: Vec::new(), leido_ms: 0, cargada: false });
/// «Lee ya» (al conectar), sin esperar a los 15 minutos.
static YA: AtomicBool = AtomicBool::new(false);
static RAIZ: Mutex<Option<PathBuf>> = Mutex::new(None);

fn fichero(raiz: &Path) -> PathBuf {
    raiz.join("calendario-google.json")
}

/// Si esta conectado (hay permiso guardado).
pub fn conectado() -> bool {
    PRUEBA.load(Ordering::SeqCst) || crate::asistente::conexiones::conectado(&GOOGLE_CALENDAR)
}

/// Las pruebas ponen sus eventos sin cuenta de Google.
static PRUEBA: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
pub fn poner_para_pruebas(eventos: Vec<Evento>) {
    PRUEBA.store(true, Ordering::SeqCst);
    if let Ok(mut c) = CACHE.write() {
        c.eventos = eventos;
        c.leido_ms = pixpin_shell::entorno::ahora_utc_ms();
        c.cargada = true;
    }
}

fn cargar_si_hace_falta() {
    if CACHE.read().map(|c| c.cargada).unwrap_or(true) {
        return;
    }
    let Some(raiz) = RAIZ.lock().ok().and_then(|r| r.clone()) else { return };
    let leido = std::fs::read(fichero(&raiz)).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
    if let Ok(mut c) = CACHE.write() {
        c.cargada = true;
        if let Some(v) = leido {
            c.eventos = calendario::de_json(&v["eventos"]);
            c.leido_ms = v["leido"].as_i64().unwrap_or(0);
        }
    }
}

/// **Los eventos que tocan** el tramo `[desde, hasta)` (ms UTC), de lo
/// guardado. Vacio si no esta conectado o aun no se leyo nada.
pub fn eventos_entre(desde_utc: i64, hasta_utc: i64) -> Vec<Evento> {
    if !conectado() {
        return Vec::new();
    }
    cargar_si_hace_falta();
    CACHE
        .read()
        .map(|c| c.eventos.iter().filter(|e| e.inicio_ms < hasta_utc && e.fin_ms > desde_utc).cloned().collect())
        .unwrap_or_default()
}

/// Uno por su id (el aviso que suena).
pub fn evento(id: &str) -> Option<Evento> {
    cargar_si_hace_falta();
    CACHE.read().ok()?.eventos.iter().find(|e| e.id == id).cloned()
}

/// Cuando se leyo por ultima vez (ms UTC; 0: nunca).
pub fn leido_ms() -> i64 {
    cargar_si_hace_falta();
    CACHE.read().map(|c| c.leido_ms).unwrap_or(0)
}

/// Que se lea en cuanto se pueda (al conectar).
pub fn leer_ya() {
    YA.store(true, Ordering::SeqCst);
}

/// Una lectura: de Google a la cache y al disco.
fn leer(raiz: &Path) -> Result<usize, String> {
    let cx = pixpin_ia::conexiones::Conexion::abrir(&GOOGLE_CALENDAR)?;
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let eventos = calendario::eventos(&cx, ahora - ATRAS_MS, ahora + ADELANTE_MS)?;
    let n = eventos.len();
    let cambiaron = CACHE.read().map(|c| c.eventos != eventos).unwrap_or(true);
    let v = serde_json::json!({"leido": ahora, "eventos": calendario::a_json(&eventos)});
    let tmp = fichero(raiz).with_extension("json.parte");
    if std::fs::write(&tmp, v.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, fichero(raiz));
    }
    if let Ok(mut c) = CACHE.write() {
        c.eventos = eventos;
        c.leido_ms = ahora;
        c.cargada = true;
    }
    if cambiaron {
        crate::recordatorios::releer(raiz);
    }
    Ok(n)
}

/// **El hilo que lee el calendario**: al arrancar (tras unos segundos, para
/// no estorbar), cada 15 minutos y en cuanto se conecta. Una vez.
pub fn vigilar(raiz: &Path) {
    if let Ok(mut r) = RAIZ.lock() {
        *r = Some(raiz.to_path_buf());
    }
    let raiz = raiz.to_path_buf();
    let lanzado = std::thread::Builder::new().name("google-calendar".into()).spawn(move || {
        std::thread::sleep(Duration::from_secs(8));
        let mut siguiente = std::time::Instant::now();
        loop {
            if (std::time::Instant::now() >= siguiente || YA.swap(false, Ordering::SeqCst)) && conectado() {
                let t = std::time::Instant::now();
                match leer(&raiz) {
                    Ok(n) => tracing::info!(eventos = n, ms = t.elapsed().as_millis() as u64, "google calendar: leido"),
                    Err(e) => tracing::warn!(%e, "google calendar: no se pudo leer"),
                }
                siguiente = std::time::Instant::now() + CADA;
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de Google Calendar");
    }
}

/// `16:00–17:30` (o «todo el día») en hora local, para la llamada y el
/// asistente.
pub fn tramo_legible(e: &Evento) -> String {
    if e.todo_el_dia {
        return "todo el día".into();
    }
    let hm = |utc: i64| {
        let l = pixpin_shell::entorno::a_local(utc).rem_euclid(86_400_000) / 60_000;
        format!("{}:{:02}", l / 60, l % 60)
    };
    format!("{}–{}", hm(e.inicio_ms), hm(e.fin_ms))
}

/// **Lo que se le cuenta al asistente** de los eventos de `[desde, hasta)`
/// (ms locales): una linea por evento con su dia, su hora, su lugar y su
/// calendario.
pub fn para_el_modelo(desde_local: i64, hasta_local: i64) -> String {
    if !conectado() {
        return "Google Calendar no esta conectado: el usuario lo conecta en Ajustes → Asistente → Conexiones (Conectar Google Calendar).".into();
    }
    let desfase = pixpin_shell::entorno::ahora_local_ms() - pixpin_shell::entorno::ahora_utc_ms();
    let evs = eventos_entre(desde_local - desfase, hasta_local - desfase);
    let leido = leido_ms();
    let hace = if leido == 0 {
        "aun no se ha leido (se lee en unos segundos)".to_string()
    } else {
        format!("leido hace {} min", (pixpin_shell::entorno::ahora_utc_ms() - leido).max(0) / 60_000)
    };
    if evs.is_empty() {
        return format!("No hay eventos en Google Calendar en ese tramo ({hace}).");
    }
    let mut out = format!("{} eventos de Google Calendar ({hace}):\n", evs.len());
    for e in &evs {
        let dia = if e.todo_el_dia { e.inicio_ms } else { pixpin_shell::entorno::a_local(e.inicio_ms) }.div_euclid(86_400_000);
        let (a, m, d) = calendario::civil(dia);
        let semana = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"][(dia + 3).rem_euclid(7) as usize];
        out.push_str(&format!("- {semana} {d:02}/{m:02}/{a} {} «{}»", tramo_legible(e), e.titulo));
        if !e.lugar.is_empty() {
            out.push_str(&format!(" en {}", e.lugar));
        }
        out.push_str(&format!(" ({})\n", e.calendario));
    }
    out
}

#[cfg(test)]
mod pruebas {
    #[test]
    fn el_dia_de_la_semana_del_1970_es_jueves() {
        // (El indice de `para_el_modelo`: el dia 0 fue jueves.)
        assert_eq!(["lun", "mar", "mié", "jue", "vie", "sáb", "dom"][(0i64 + 3).rem_euclid(7) as usize], "jue");
    }
}
