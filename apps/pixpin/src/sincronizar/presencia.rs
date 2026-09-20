//! **Estar localizable para el grupo mientras PixPin esta abierto.**
//!
//! Puerto de `sincro/Presencia.kt` del movil. Alli la escucha y el anuncio
//! viven mientras haya alguna pantalla de PixPin a la vista, no mientras este
//! abierta la de Sincronizar; el usuario lo pidio con estas palabras: «tengo
//! que esperar un minuto o dos y recien me aparece para sincronizar». La
//! razon es que muchas redes Wi-Fi tardan uno o dos minutos en repartir un
//! anuncio mDNS nuevo, asi que un anuncio que se apaga y se enciende cada vez
//! que se abre la ventana no llega a tiempo nunca.
//!
//! Aqui pasa lo mismo: **la escucha, el anuncio y la sonda arrancan con la
//! aplicacion** (`main.rs`) y viven hasta que se cierra. La ventana de
//! Sincronizar ya no los monta: pregunta por el puerto y se apunta para que
//! le cuenten lo que pase (`suscribir`).
//!
//! Tres cosas que importan:
//!
//! - **El puerto 47474 lo tiene esto y nadie mas.** «Recibir» y «Enviar» de
//!   «Pasar algo a otra persona» piden un puerto cualquiera y lo meten en su
//!   QR, que es de donde lo lee el movil.
//! - **Se puede apagar** desde `ajustes.toml` (`[sincro] presencia = false`):
//!   quien no quiera que PixPin tenga una puerta abierta mientras vive, no la
//!   tiene. Sincronizar sigue pudiendo llamar; lo que no habra es a quien
//!   llamar aqui.
//! - **Buscar a los demas cuesta**, asi que la sonda va deprisa mientras
//!   alguien mira la ventana y despacio cuando no mira nadie. Escuchar un
//!   puerto por el que no habla nadie no gasta; salir a buscar por la red
//!   cada cuatro segundos, si.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};

/// Lo que el servicio le cuenta a quien este mirando.
#[derive(Debug, Clone)]
pub(crate) enum Novedad {
    /// Alguien cambio `identidad.json` (se unio un aparato, cambio un nombre,
    /// termino una vuelta): la ventana vuelve a leerla.
    Identidad,
    /// Si un aparato recordado contesto a la sonda.
    Responde(String, bool),
    /// Una linea para «Actividad».
    Registro(String),
}

struct Servicio {
    /// En que puerto se escucha, si se pudo abrir.
    puerto: Option<u16>,
    /// Quien quiere enterarse. Un canal cerrado se cae solo al difundir.
    oyentes: Mutex<Vec<mpsc::Sender<Novedad>>>,
    /// Cuantas ventanas estan mirando: con ninguna, la sonda va despacio.
    mirando: AtomicUsize,
}

static SERVICIO: OnceLock<Servicio> = OnceLock::new();

/// Cada cuanto se pregunta por los demas con alguien mirando y sin nadie.
/// Cuatro segundos es lo que hace el movil con Sincronizar abierta.
const MIRANDO_MS: u64 = 4_000;
const A_SOLAS_MS: u64 = 60_000;

/// Arranca la presencia. Se llama una vez, al abrir la aplicacion.
///
/// Con `encendida = false` no se abre ninguna puerta ni se anuncia nada: solo
/// se deja el servicio puesto para que la ventana pueda preguntar y decir que
/// esta apagado.
pub(crate) fn instalar(raiz: PathBuf, encendida: bool) {
    if SERVICIO.get().is_some() {
        return;
    }
    // `vivo` se queda en los hilos y nadie lo baja: la presencia vive lo que
    // la aplicacion, y cuando el proceso termina se van con el. Existe
    // porque es lo que esperan `escuchar`, `anunciarse` y `sondear`, que la
    // ventana usaba antes con una vida mas corta.
    let vivo = Arc::new(AtomicBool::new(true));
    // La puerta se abre AQUI y no en el hilo: el puerto hace falta ya, para
    // decirlo en la ventana y para meterlo en el anuncio.
    let puerto = if encendida {
        super::escuchar(raiz.clone(), vivo.clone())
    } else {
        None
    };
    let _ = SERVICIO.set(Servicio {
        puerto,
        oyentes: Mutex::new(Vec::new()),
        mirando: AtomicUsize::new(0),
    });
    if let Some(p) = puerto {
        super::anunciarse(raiz.clone(), p, vivo.clone());
        super::sondear(raiz, vivo);
        tracing::info!(puerto = p, "presencia encendida");
    } else if encendida {
        tracing::warn!(
            "no se pudo abrir la puerta de sincronizar: nadie podra encontrar este equipo"
        );
    } else {
        tracing::info!("presencia apagada por los ajustes ([sincro] presencia = false)");
    }
}

/// En que puerto se escucha, para «La de este aparato». `None` si esta
/// apagada o no se pudo abrir.
pub(crate) fn puerto() -> Option<u16> {
    SERVICIO.get().and_then(|s| s.puerto)
}

/// Si la presencia esta funcionando de verdad.
pub(crate) fn encendida() -> bool {
    puerto().is_some()
}

/// Cuenta a quien este mirando lo que acaba de pasar.
pub(crate) fn difundir(n: Novedad) {
    let Some(s) = SERVICIO.get() else {
        return;
    };
    let Ok(mut oyentes) = s.oyentes.lock() else {
        return;
    };
    // Quien cerro su ventana se cae solo: el `Sender` no llega a nadie.
    oyentes.retain(|tx| tx.send(n.clone()).is_ok());
}

/// Que se mire deprisa: mientras viva lo que devuelve, la sonda va al ritmo
/// del movil y lo que pase llega por `tx`.
///
/// Se devuelve un guardian y no dos funciones sueltas a proposito: una
/// ventana que se cierra sin «desapuntarse» dejaria la sonda corriendo
/// deprisa para siempre.
pub(crate) fn suscribir(tx: mpsc::Sender<Novedad>) -> Suscripcion {
    if let Some(s) = SERVICIO.get() {
        if let Ok(mut o) = s.oyentes.lock() {
            o.push(tx);
        }
        s.mirando.fetch_add(1, Ordering::SeqCst);
    }
    Suscripcion
}

pub(crate) struct Suscripcion;

impl Drop for Suscripcion {
    fn drop(&mut self) {
        if let Some(s) = SERVICIO.get() {
            // `fetch_update` y no un `-1` a secas: por debajo de cero, la
            // sonda se quedaria corriendo deprisa sin nadie delante.
            let _ = s
                .mirando
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| {
                    Some(v.saturating_sub(1))
                });
        }
    }
}

/// Cuanto espera la sonda antes de volver a preguntar.
pub(crate) fn espera_de_la_sonda() -> std::time::Duration {
    let mirando = SERVICIO
        .get()
        .is_some_and(|s| s.mirando.load(Ordering::SeqCst) > 0);
    std::time::Duration::from_millis(if mirando { MIRANDO_MS } else { A_SOLAS_MS })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn sin_instalar_no_hay_puerto_ni_se_rompe_nada_al_difundir() {
        // Caso negativo: la ventana puede preguntar antes de que la
        // aplicacion haya arrancado el servicio, y eso no puede reventar.
        difundir(Novedad::Identidad);
        let _ = puerto();
        let _ = encendida();
    }

    #[test]
    fn la_sonda_va_despacio_cuando_no_mira_nadie() {
        // Sin nadie delante no hay para quien buscar: salir a la red cada
        // cuatro segundos seria gastar por gastar.
        assert!(espera_de_la_sonda() >= std::time::Duration::from_millis(MIRANDO_MS));
    }

    #[test]
    fn soltar_la_suscripcion_no_deja_la_cuenta_bajo_cero() {
        // Dos ventanas que se cierran habiendo una sola apuntada dejarian la
        // cuenta negativa, y entonces «mirando» seria cierto para siempre.
        let (tx, _rx) = mpsc::channel();
        let a = suscribir(tx);
        drop(a);
        let (tx2, _rx2) = mpsc::channel();
        let b = suscribir(tx2);
        drop(b);
        if let Some(s) = SERVICIO.get() {
            assert_eq!(s.mirando.load(Ordering::SeqCst), 0);
        }
    }
}
