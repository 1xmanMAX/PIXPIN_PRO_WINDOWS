//! **El permiso de red de PinDesk** (3-nov-2026), lo de la ventana.
//!
//! El usuario: «cuando escribo el codigo NO aparece el aparato». Una de las
//! causas es que el cortafuegos de Windows no deje entrar a `pindesk.exe`
//! (exe nuevo desde el cambio de nombre: la regla de `pixpinmax.exe` no le
//! vale, y si su aviso se cerro sin permitir, Windows lo bloquea). Sin
//! permiso, el movil no puede llamar al PC ni el PC oir las balizas de los
//! demas, y no se ve nada.
//!
//! Aqui: mirar si hay permiso (TCP del puerto de la presencia y UDP de la
//! baliza, `pindesk_shell::cortafuegos::permiso`) y, si el usuario lo pide,
//! ponerlo como administrador. Lo pinta y lo explica la portada de
//! Sincronizar («PinDesk no tiene permiso de red» y su boton).

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use pindesk_shell::cortafuegos::{self, Pedido, Permiso};

/// Lo ultimo que se supo, y cuando: mirar cuesta poco, pero no en cada
/// repintado.
static VISTO: Mutex<Option<(Instant, Option<bool>)>> = Mutex::new(None);
const VALE: Duration = Duration::from_secs(30);

fn exe() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

/// **Si los otros aparatos pueden llegar a PinDesk**: `Some(true)` si el
/// cortafuegos deja entrar en el puerto de la sincronizacion y en el de la
/// baliza, `Some(false)` si bloquea alguno, `None` si no se sabe (Windows no
/// contesto, o la presencia esta apagada y no hay puerto). Recordado 30 s.
pub(crate) fn permiso_de_red() -> Option<bool> {
    if let Ok(v) = VISTO.lock()
        && let Some((cuando, que)) = *v
        && cuando.elapsed() < VALE
    {
        return que;
    }
    let que = mirar();
    if let Ok(mut v) = VISTO.lock() {
        *v = Some((Instant::now(), que));
    }
    que
}

fn mirar() -> Option<bool> {
    let exe = exe()?;
    let puerto = super::presencia::puerto()?;
    let tcp = cortafuegos::permiso(&exe, puerto, false);
    let udp = cortafuegos::permiso(&exe, pindesk_sincro::baliza::PUERTO_BALIZA, true);
    let que = juntar(tcp, udp);
    tracing::info!(?tcp, ?udp, "cortafuegos: permiso de red de PinDesk");
    que
}

/// Los dos puertos en uno: basta un bloqueo para que no se vea.
fn juntar(tcp: Permiso, udp: Permiso) -> Option<bool> {
    match (tcp, udp) {
        (Permiso::Bloqueado, _) | (_, Permiso::Bloqueado) => Some(false),
        (Permiso::Permitido, Permiso::Permitido) => Some(true),
        _ => None,
    }
}

/// Como acabo «Dar permiso de red», para decirlo en la ventana.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Resultado {
    /// Puesto y ya entra.
    Hecho,
    /// Puesto, pero sigue sin entrar: la red esta marcada como «Pública»
    /// (las reglas son para redes privadas y de dominio, a proposito).
    RedPublica,
    /// El usuario dijo «No» al aviso de administrador.
    Cancelado,
    /// Windows no dejo (directiva de empresa, sin servicio).
    Fallo,
}

/// **Pone el permiso** como administrador y lo vuelve a mirar. Bloquea unos
/// segundos (el aviso de Windows y PowerShell): desde un hilo.
pub(crate) fn dar_permiso() -> Resultado {
    let Some(exe) = exe() else {
        return Resultado::Fallo;
    };
    let pedido = cortafuegos::pedir_permiso(&exe);
    if let Ok(mut v) = VISTO.lock() {
        *v = None;
    }
    resultado(pedido, permiso_de_red())
}

fn resultado(pedido: Pedido, ahora: Option<bool>) -> Resultado {
    match (pedido, ahora) {
        (Pedido::Cancelado, _) => Resultado::Cancelado,
        (Pedido::Fallo, _) => Resultado::Fallo,
        (Pedido::Hecho, Some(false)) => Resultado::RedPublica,
        (Pedido::Hecho, _) => Resultado::Hecho,
    }
}

/// La clave del texto de cada resultado.
pub(crate) fn clave_del_texto(r: Resultado) -> &'static str {
    match r {
        Resultado::Hecho => "sinc-red-hecho",
        Resultado::RedPublica => "sinc-red-publica",
        Resultado::Cancelado => "sinc-red-cancelado",
        Resultado::Fallo => "sinc-red-fallo",
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn basta_un_puerto_bloqueado_para_que_no_se_vea() {
        assert_eq!(juntar(Permiso::Permitido, Permiso::Permitido), Some(true));
        assert_eq!(juntar(Permiso::Bloqueado, Permiso::Permitido), Some(false));
        assert_eq!(juntar(Permiso::Permitido, Permiso::Bloqueado), Some(false));
        assert_eq!(juntar(Permiso::NoSe, Permiso::Bloqueado), Some(false));
        // Caso negativo: sin saberlo no se molesta al usuario.
        assert_eq!(juntar(Permiso::NoSe, Permiso::Permitido), None);
    }

    #[test]
    fn puesto_y_aun_bloqueado_es_una_red_publica() {
        assert_eq!(resultado(Pedido::Hecho, Some(true)), Resultado::Hecho);
        assert_eq!(resultado(Pedido::Hecho, None), Resultado::Hecho);
        assert_eq!(resultado(Pedido::Hecho, Some(false)), Resultado::RedPublica);
        assert_eq!(
            resultado(Pedido::Cancelado, Some(false)),
            Resultado::Cancelado
        );
        assert_eq!(resultado(Pedido::Fallo, None), Resultado::Fallo);
    }
}
