//! **Las fotos por su hora, en este equipo** (28-oct-2026). La logica esta
//! en `pixpin_proyecto::por_su_hora`; aqui, lo de la aplicacion:
//!
//! - **Los desfases medidos** ([`medido`]): en cada saludo de sincronizar
//!   (el `desfase` de la sesion), en cada aviso a la camara del movil y en
//!   cada `camara_remota` del otro, se apunta lo que va adelantado el reloj
//!   de ese aparato en `sincro/desfases.json`, por su codigo (`K7Q2`).
//! - **El repaso** ([`repasar`]): tras cada sincronizacion (con el grupo,
//!   con una persona, por Wi-Fi) y al guardarse una clase, en un hilo, cada
//!   foto con hora que cae en una grabacion de este equipo entra en su
//!   minuto. Una a la vez: si se pide mientras corre, se repite al acabar.
//! - **«Adjuntar a la grabación…»** a mano ([`a_mano`]).
//! - **Invitar a todos los del chat** a aportar fotos ([`invitar_a_todos`]).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use pixpin_proyecto::por_su_hora::{self, Desfases};
use pixpin_proyecto::{almacen, codigos, cuaderno};

/// Que solo un hilo escriba los desfases a la vez.
static DESFASES: Mutex<()> = Mutex::new(());
/// Si hay un repaso corriendo, y si se pidio otro mientras.
static CORRIENDO: AtomicBool = AtomicBool::new(false);
static OTRA_VEZ: AtomicBool = AtomicBool::new(false);

fn fichero(raiz: &Path) -> PathBuf {
    super::carpeta_sincro(raiz).join("desfases.json")
}

/// Los desfases medidos hasta ahora.
pub(crate) fn desfases(raiz: &Path) -> Desfases {
    let _g = DESFASES.lock().unwrap_or_else(|e| e.into_inner());
    Desfases::leer(&fichero(raiz))
}

/// **Apunta lo que va adelantado el reloj del aparato `id`** (su id, no su
/// codigo) medido ahora.
pub(crate) fn medido(raiz: &Path, id: &str, desfase: i64) {
    if id.trim().is_empty() {
        return;
    }
    let _g = DESFASES.lock().unwrap_or_else(|e| e.into_inner());
    let f = fichero(raiz);
    let mut d = Desfases::leer(&f);
    d.medir(&codigos::de_aparato(id), super::ahora_ms(), desfase);
    if let Err(e) = d.guardar(&f) {
        tracing::debug!(?e, "no se apunto el desfase");
    }
}

/// El codigo de este equipo y los de todos mis aparatos (los del grupo).
pub(crate) fn yo_y_mios(raiz: &Path) -> (String, HashSet<String>) {
    let yo = crate::letras_que_viajan::aparato(raiz);
    let mut mios: HashSet<String> = super::leer_identidad(raiz)
        .map(|i| {
            i.miembros
                .iter()
                .map(|m| codigos::de_aparato(&m.id))
                .chain(std::iter::once(codigos::de_aparato(&i.yo.id)))
                .collect()
        })
        .unwrap_or_default();
    mios.insert(yo.clone());
    (yo, mios)
}

/// **Repasa todos los chats en un hilo**: lo que haya llegado con hora
/// entra en su grabacion. Se puede llamar sin miedo tras cada cosa que
/// trae mensajes.
pub(crate) fn repasar(raiz: &Path) {
    if CORRIENDO.swap(true, Ordering::SeqCst) {
        OTRA_VEZ.store(true, Ordering::SeqCst);
        return;
    }
    let raiz = raiz.to_path_buf();
    let lanzado = std::thread::Builder::new()
        .name("fotos-por-su-hora".into())
        .spawn(move || {
            loop {
                OTRA_VEZ.store(false, Ordering::SeqCst);
                let (yo, mios) = yo_y_mios(&raiz);
                let hechas = por_su_hora::repasar_todos(&raiz, &yo, &mios, &desfases(&raiz));
                if !hechas.is_empty() {
                    tracing::info!(?hechas, "fotos puestas en su grabacion por su hora");
                    crate::ventana_chat::refrescar();
                }
                if !OTRA_VEZ.load(Ordering::SeqCst) {
                    break;
                }
            }
            CORRIENDO.store(false, Ordering::SeqCst);
        });
    if lanzado.is_err() {
        CORRIENDO.store(false, Ordering::SeqCst);
    }
}

/// **Las grabaciones de un chat** donde se puede adjuntar a mano la foto
/// `foto`: primero las que tienen su hora dentro, luego las demas de la mas
/// nueva a la mas vieja. El id de la nota y su nombre.
pub(crate) fn grabaciones_para(raiz: &Path, proyecto: &str, foto: &str) -> Vec<(String, String)> {
    let dir = almacen::carpeta(raiz, proyecto);
    let Ok(c) = cuaderno::Cuaderno::leer_de(&dir) else {
        return Vec::new();
    };
    let f = c
        .mensajes
        .iter()
        .find(|m| m.id == foto)
        .and_then(por_su_hora::foto_de);
    let desfases = desfases(raiz);
    let mut v: Vec<(bool, i64, String, String)> = c
        .mensajes
        .iter()
        .filter(|m| {
            m.clase == Some(cuaderno::Clase::Voz)
                && m.resto.get("grabacion").and_then(|g| g.as_str()) == Some("clase")
        })
        .map(|m| {
            let reloj = por_su_hora::RelojDeGrabacion::de(&m.resto);
            let dentro = match (&f, &reloj) {
                (Some(f), Some(r)) => {
                    let h = por_su_hora::hora_corregida(f, &r.aparato, &desfases);
                    por_su_hora::lugar(h.utc, r, None, por_su_hora::MARGEN_MS).is_some()
                }
                _ => false,
            };
            let nombre = if m.nombre.trim().is_empty() {
                m.texto.clone()
            } else {
                m.nombre.trim_end_matches(".m4a").to_string()
            };
            (dentro, m.cuando, m.id.clone(), nombre)
        })
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    v.into_iter().map(|(_, _, id, n)| (id, n)).collect()
}

/// **«Adjuntar a la grabación…»**: la foto en la grabacion mas a mano (la
/// que tiene su hora dentro o, si ninguna, la ultima del chat). Devuelve el
/// minuto donde quedo, o `None` si no hay grabaciones en el chat.
pub(crate) fn a_mano(raiz: &Path, proyecto: &str, foto: &str) -> std::io::Result<Option<i64>> {
    let Some((voz, _)) = grabaciones_para(raiz, proyecto, foto).into_iter().next() else {
        return Ok(None);
    };
    let (_, mios) = yo_y_mios(raiz);
    let dir = almacen::carpeta(raiz, proyecto);
    let ms = por_su_hora::a_mano(&dir, foto, &voz, None, &mios, &desfases(raiz))?;
    crate::ventana_chat::refrescar();
    Ok(Some(ms))
}

/// Si la nota de la grabacion `voz` deja aportar a todos los del chat.
pub(crate) fn abierta_a_todos(m: &cuaderno::Mensaje) -> bool {
    por_su_hora::colaboradores_de(&m.resto)
        .iter()
        .any(|c| c == por_su_hora::TODOS)
}

/// **Invita (o deja de invitar) a todos los del chat compartido** a poner
/// fotos en la grabacion de la nota `voz`.
pub(crate) fn invitar_a_todos(
    raiz: &Path,
    proyecto: &str,
    voz: &str,
    si: bool,
) -> std::io::Result<()> {
    let dir = almacen::carpeta(raiz, proyecto);
    cuaderno::cambiar(&dir, voz, |m| {
        por_su_hora::invitar(&mut m.resto, por_su_hora::TODOS, si)
    })?;
    crate::ventana_chat::refrescar();
    if si {
        repasar(raiz);
    }
    Ok(())
}
