//! El universo dentro de la app: traduce el chat a fichas, lee los
//! cuadernos sin parar la ventana y engancha el nucleo puro
//! (`pixpin-universo`) al editor.
//!
//! La puerta de entrada es `lanzar`: abre el universo en su propio hilo, o
//! le deja el pedido al que ya esta abierto.

pub mod abrir;
pub mod cargador;
pub mod cielo;
pub mod estrellas;
pub mod fichas;
pub mod mapa;
pub mod pintar;
pub mod sesion;

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering};

use pixpin_store::Ubicacion;

use crate::ventana_chat::OpcionesLienzo;

/// Que ensenar al abrir (D210-D214).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pedido {
    /// Todas las galaxias.
    Cosmos,
    /// La galaxia de un proyecto, por su id del indice.
    Galaxia(String),
    /// Un archivo: su luna si esta colocada, o su galaxia con la nebulosa en
    /// la pagina donde esta.
    Luna { proyecto: String, codigo: String },
}

/// El HWND del universo abierto (> 0), -1 mientras nace, 0 si no hay. Un
/// entero y no un `HWND` porque lo miran dos hilos.
pub(crate) static ABIERTO: AtomicIsize = AtomicIsize::new(0);

/// Lo que se pidio con el universo ya abierto: lo recoge su hilo al
/// despertar. Solo cabe uno: si se piden dos cosas seguidas, gana la ultima,
/// que es la que el usuario esta mirando.
static PEDIDO: Mutex<Option<Pedido>> = Mutex::new(None);

pub(crate) fn dejar_pedido(p: Pedido) {
    if let Ok(mut g) = PEDIDO.lock() {
        *g = Some(p);
    }
}

pub(crate) fn tomar_pedido() -> Option<Pedido> {
    PEDIDO.lock().ok().and_then(|mut g| g.take())
}

/// Abre el universo en SU PROPIO HILO, o, si ya esta abierto, lo trae al
/// frente y le pasa el pedido.
///
/// En su propio hilo, como el chat: pasa mucho tiempo abierto y su bucle no
/// puede quedarse con los mensajes de la ventana principal.
pub fn lanzar(
    idioma: pixpin_store::Idioma,
    ubicacion: Ubicacion,
    opciones: OpcionesLienzo,
    pedido: Pedido,
) {
    let ya = ABIERTO.load(Ordering::SeqCst);
    if ya != 0 {
        dejar_pedido(pedido);
        if ya > 0 {
            pixpin_shell::overlay::VentanaOverlay::restaurar_de_hwnd(
                windows::Win32::Foundation::HWND(ya as *mut _),
            );
            pixpin_shell::overlay::despertar(ya);
        }
        // Naciendo (-1): el hilo recoge el pedido en cuanto abre la ventana.
        return;
    }
    ABIERTO.store(-1, Ordering::SeqCst);
    let lanzado = std::thread::Builder::new()
        .name("universo".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            if let Err(e) = abrir_en_este_hilo(idioma, &ubicacion, opciones, pedido) {
                tracing::warn!(?e, "no se pudo abrir el universo");
            }
            ABIERTO.store(0, Ordering::SeqCst);
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo del universo");
        ABIERTO.store(0, Ordering::SeqCst);
    }
}

/// Cuantas veces se sale a una hoja y se vuelve antes de parar.
const SALTOS_MAXIMOS: usize = 32;

/// Todo el universo, de principio a fin, en el hilo que llama: leer el
/// indice y `universo.json`, ponerlo al dia, abrir el editor y guardar.
fn abrir_en_este_hilo(
    idioma: pixpin_store::Idioma,
    ubicacion: &Ubicacion,
    opciones: OpcionesLienzo,
    pedido: Pedido,
) -> anyhow::Result<()> {
    let raiz = ubicacion.raiz().to_path_buf();
    let indice = pixpin_proyecto::almacen::Indice::leer(&raiz);
    // D229: la galaxia del origen es la del primer proyecto de la lista.
    let fichas = indice.ordenadas();
    let ids: Vec<String> = fichas.iter().map(|f| f.id.clone()).collect();
    let nombres: HashMap<String, String> = fichas
        .iter()
        .map(|f| (f.id.clone(), f.nombre.clone()))
        .collect();
    let ahora_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let cargado =
        pixpin_universo::formato::cargar(&pixpin_universo::formato::ruta(&raiz), ahora_ms)?;
    let mut u = cargado.universo;
    let informe = pixpin_universo::galaxias::sincronizar(&mut u, &ids);
    tracing::info!(?informe, "universo al dia con el indice");
    // Mientras el universo esta abierto, la escena es del editor.
    let mut escena = std::mem::take(&mut u.anotaciones);
    let textos = pixpin_store::Catalogo::nuevo(idioma);
    let roto = cargado.roto.is_some();
    let mut sesion = sesion::Sesion::nueva(
        raiz.clone(),
        u,
        nombres,
        opciones.nivel,
        textos,
        Some(pedido),
    );
    sesion.al_chat = Some(abrir::AlChat {
        idioma,
        ubicacion: ubicacion.clone(),
        opciones,
    });
    if roto {
        sesion.avisar_roto();
    }
    for _ in 0..SALTOS_MAXIMOS {
        escena = crate::ventana_editor::abrir_universo(
            escena,
            opciones.enganche,
            opciones.nivel,
            opciones.medir_fotogramas,
            &mut sesion,
        )?;
        match sesion.hoja_pedida.take() {
            Some((proyecto, referencia)) => {
                abrir::abrir_hoja(&raiz, &proyecto, &referencia, opciones);
            }
            None => break,
        }
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_segundo_lanzar_con_el_universo_abierto_solo_deja_el_pedido() {
        PEDIDO.lock().unwrap().take();
        dejar_pedido(Pedido::Galaxia("p1".into()));
        assert_eq!(tomar_pedido(), Some(Pedido::Galaxia("p1".into())));
        assert_eq!(tomar_pedido(), None);
    }

    #[test]
    fn el_ultimo_pedido_gana() {
        PEDIDO.lock().unwrap().take();
        dejar_pedido(Pedido::Cosmos);
        dejar_pedido(Pedido::Galaxia("p2".into()));
        assert_eq!(tomar_pedido(), Some(Pedido::Galaxia("p2".into())));
    }
}
