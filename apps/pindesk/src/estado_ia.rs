//! **El estado de las IA en la app** (1-nov-2026, `docs/ui-funciones-y-chats.md`):
//! lo que se sabe de Claude, DeepSeek y Jev para los puntitos de junto al
//! boton de sincronizar del chat (`ventana_chat::estado_ia`).
//!
//! - **De lo que se usa de verdad**: cada peticion del asistente, de «Probar»
//!   en Ajustes y de Jev avisa de como acabo (`pindesk_ia::nube::al_contestar`
//!   y `pindesk_typesafe::cliente::al_contestar`).
//! - **Una comprobacion que no gasta**, en su hilo («estado-ia»): la lista
//!   de modelos de cada API (Anthropic `GET /v1/models`, DeepSeek
//!   `GET /models`, TypeSafe `GET /v1/models`), a los 10 s de arrancar, al
//!   cambiar una clave, con «Comprobar ahora» y como mucho cada 30 min; sin
//!   red, antes y con espera creciente. Las reglas, con sus pruebas, en
//!   `pindesk_ui::estado_ia`. Nunca bloquea la interfaz.
//!
//! La clave solo se descifra en el hilo de la comprobacion, de camino a la
//! cabecera HTTP: nunca a `tracing`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use pindesk_ia::nube::{self, Proveedor};
use pindesk_ui::estado_ia::{Desenlace, Servicio, Tablero};

static TABLERO: Mutex<Option<Tablero>> = Mutex::new(None);
/// Despierta al hilo de las comprobaciones (clave nueva, «Comprobar ahora»).
static DESPERTADOR: Condvar = Condvar::new();
/// Sube cuando cambia algo que se ve: el chat repinta si lo ve distinto.
static NOVEDADES: AtomicU64 = AtomicU64::new(0);
static RELOJ: OnceLock<Instant> = OnceLock::new();

/// Lo mas que duerme el hilo sin mirar si hay claves nuevas en el disco.
const VISTAZO: Duration = Duration::from_secs(5 * 60);

/// Los ms del reloj del estado (desde que se miro por primera vez).
pub fn ahora_ms() -> u64 {
    RELOJ.get_or_init(Instant::now).elapsed().as_millis() as u64
}

fn tablero() -> MutexGuard<'static, Option<Tablero>> {
    TABLERO.lock().unwrap_or_else(|e| e.into_inner())
}

fn con<R>(f: impl FnOnce(&mut Tablero) -> R) -> R {
    f(tablero().get_or_insert_with(Tablero::nuevo))
}

/// Lo que se sabe ahora, para pintarlo.
pub fn foto() -> Tablero {
    con(|t| t.clone())
}

pub fn novedades() -> u64 {
    NOVEDADES.load(Ordering::SeqCst)
}

/// Algo de lo que se ve cambio: el chat lo repinta (el mismo despertar que
/// el boton de sincronizar).
fn cambio() {
    NOVEDADES.fetch_add(1, Ordering::SeqCst);
    crate::ventana_chat::avisar_de_la_sincronizacion();
}

fn de_proveedor(p: Proveedor) -> Servicio {
    match p {
        Proveedor::Claude => Servicio::Claude,
        Proveedor::DeepSeek => Servicio::DeepSeek,
    }
}

fn apuntar(s: Servicio, d: Desenlace) {
    let ahora = ahora_ms();
    let cambiado = con(|t| {
        let antes = t.estado(s);
        t.apuntar(s, d, ahora);
        t.estado(s) != antes
    });
    if cambiado {
        cambio();
    }
}

fn oir_nube(p: Proveedor, codigo: Option<u32>) {
    apuntar(de_proveedor(p), Desenlace::de_codigo(codigo));
}

fn oir_jev(codigo: Option<u16>) {
    apuntar(Servicio::Jev, Desenlace::de_codigo(codigo.map(u32::from)));
}

/// Si hay clave guardada (lo que ya saben Ajustes; no descifra nada).
fn hay_clave(s: Servicio) -> bool {
    match s {
        Servicio::Claude => crate::asistente::nube::hay_clave(Proveedor::Claude),
        Servicio::DeepSeek => crate::asistente::nube::hay_clave(Proveedor::DeepSeek),
        Servicio::Jev => crate::jev::hay_clave(),
    }
}

/// **Arranca** el estado: escucha lo que pasa en cada peticion y lanza el
/// hilo de las comprobaciones. Una vez, al arrancar la app. En las pruebas,
/// nada (nunca salen a la red).
pub fn arrancar() {
    if cfg!(test) {
        return;
    }
    let arranque = ahora_ms();
    nube::al_contestar(oir_nube);
    pindesk_typesafe::cliente::al_contestar(oir_jev);
    let lanzado = std::thread::Builder::new()
        .name("estado-ia".into())
        .spawn(move || bucle(arranque));
    if let Err(e) = lanzado {
        tracing::warn!(?e, "estado de las IA: no se pudo lanzar su hilo");
    }
}

/// **El usuario guardo o borro una clave** (Ajustes): se olvida lo de la
/// anterior y, si hay una nueva, se comprueba ya.
pub fn clave_cambiada(s: Servicio) {
    let hay = hay_clave(s);
    con(|t| t.clave_cambiada(s, hay));
    cambio();
    DESPERTADOR.notify_all();
}

pub fn clave_cambiada_nube(p: Proveedor) {
    clave_cambiada(de_proveedor(p));
}

/// **«Comprobar ahora»** (el menu de los puntitos).
pub fn comprobar_ahora() {
    con(|t| t.pedir_ahora());
    cambio();
    DESPERTADOR.notify_all();
}

/// El hilo: mira las claves, comprueba lo que toca y duerme hasta la
/// proxima (o hasta que lo despierten).
fn bucle(arranque: u64) {
    loop {
        // `hay_clave` puede descifrar la primera vez: fuera del cerrojo.
        let claves: Vec<(Servicio, bool)> = Servicio::TODOS
            .into_iter()
            .map(|s| (s, hay_clave(s)))
            .collect();
        let ahora = ahora_ms();
        let (toca, cambiado) = con(|t| {
            let mut cambiado = false;
            for (s, hay) in &claves {
                cambiado |= t.poner_clave(*s, *hay);
            }
            let toca = t.por_comprobar(ahora, arranque);
            for s in &toca {
                t.empezar(*s, ahora);
            }
            (toca, cambiado)
        });
        if cambiado || !toca.is_empty() {
            cambio();
        }
        for s in toca {
            comprobar(s);
            con(|t| t.acabar(s));
            cambio();
        }
        let g = tablero();
        let espera = g
            .as_ref()
            .and_then(|t| t.proxima(arranque))
            .map(|t| Duration::from_millis(t.saturating_sub(ahora_ms())))
            .unwrap_or(VISTAZO)
            .min(VISTAZO);
        if espera.is_zero() {
            continue;
        }
        let _ = DESPERTADOR.wait_timeout(g, espera);
    }
}

/// Una comprobacion que no gasta tokens. Lo que conteste llega por
/// `oir_nube`/`oir_jev`, como cualquier peticion.
fn comprobar(s: Servicio) {
    match s {
        Servicio::Claude | Servicio::DeepSeek => {
            let p = if s == Servicio::Claude {
                Proveedor::Claude
            } else {
                Proveedor::DeepSeek
            };
            if let Some(clave) = nube::claves::leer(p) {
                let _ = nube::comprobar(p, &clave);
            }
        }
        Servicio::Jev => {
            if let Some(clave) = pindesk_typesafe::claves::leer() {
                let _ = pindesk_typesafe::Cliente::nuevo(clave).modelos();
            }
        }
    }
}
