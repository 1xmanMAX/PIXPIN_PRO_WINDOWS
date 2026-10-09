//! **Sincronizar solo, con la aplicacion abierta** (8-oct-2026).
//!
//! El usuario: «mejora la sincronizacion para que funcione aun cuando no
//! estamos en la pestana de sincronizar, para que se sincronice todo con solo
//! tener la app abierta». Android no lo hace (alli se sincroniza a mano o
//! cuando llama el otro), asi que la regla es nuestra y la aprobo el usuario:
//!
//! - **Con quien**: los aparatos del grupo que contestan a la sonda de
//!   [`super::presencia`] (que ya corre con la aplicacion). Con los demas no
//!   se intenta: cada intento con uno apagado cuesta agotar la conexion.
//! - **Cuando**: al aparecer uno en la red (el movil entra en el Wi-Fi o abre
//!   PixPin); al cambiar algo en los chats de este equipo, tras
//!   [`QUIETO`] sin mas cambios; y cada [`PERIODO`] mientras haya alguno, que
//!   es como llega lo hecho en el movil (el movil no avisa de sus cambios).
//! - **Como**: igual que «Sincronizar con todos», con lo elegido la ultima vez
//!   con cada aparato ([`super::en_fondo::lanzar_callada`]). Si ya hay una
//!   vuelta en marcha —de la ventana, de Flow o del movil que llamo—, espera.
//! - **Sin ruido**: sin globos cuando sale bien y no trae nada. Avisa si llega
//!   algo del otro lado, o si falla [`FALLOS_PARA_AVISAR`] veces seguidas (una
//!   vez, no en cada intento). Lo hecho queda en «Actividad» de la ventana.
//! - Se apaga con `[sincro] automatica = false` en `ajustes.toml`.
//!
//! La decision es pura ([`Reloj::toca`]) y se prueba sin red.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use super::Fase;
use super::presencia::{self, Novedad};

/// Lo que se espera sin cambios en los chats antes de mandarlos: escribir un
/// mensaje o dibujar un trazo son muchos cambios seguidos.
pub const QUIETO: i64 = 30_000;
/// Cada cuanto se sincroniza aunque aqui no cambie nada.
pub const PERIODO: i64 = 10 * 60_000;
/// A los cuantos fallos seguidos se avisa.
pub const FALLOS_PARA_AVISAR: u32 = 3;
/// Cada cuanto se mira si hay que hacer algo.
const LATIDO: Duration = Duration::from_secs(5);
/// Cada cuantos latidos se recorre la carpeta de los chats.
const LATIDOS_POR_FIRMA: u32 = 3;

/// Por que se sincroniza ahora.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motivo {
    Aparecio,
    Cambio,
    Periodo,
}

/// **La huella de los chats**: cuantos ficheros, cuanto pesan y la suma de
/// cuando se tocaron. Cambia con cualquier cosa que se escriba.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Firma {
    pub ficheros: u64,
    pub bytes: u64,
    pub tocados: u128,
}

pub fn firma_de(carpeta: &Path) -> Firma {
    let mut f = Firma::default();
    let mut pendientes = vec![carpeta.to_path_buf()];
    while let Some(d) = pendientes.pop() {
        let Ok(leer) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in leer.flatten() {
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                pendientes.push(e.path());
            } else {
                f.ficheros += 1;
                f.bytes += meta.len();
                f.tocados += meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_millis());
            }
        }
    }
    f
}

/// Lo que recuerda la automatica entre latido y latido.
#[derive(Debug, Default)]
pub struct Reloj {
    /// Quien contesto la ultima vez.
    pub responden: BTreeMap<String, bool>,
    /// Los que pasaron a contestar desde la ultima vuelta.
    pub aparecidos: BTreeSet<String>,
    /// La huella tras la ultima vuelta (o al empezar).
    pub base: Option<Firma>,
    /// La ultima huella vista y desde cuando esta asi.
    pub vista: Option<(Firma, i64)>,
    /// Cuando empezo la ultima vuelta automatica.
    pub ultima: Option<i64>,
    pub fallos_seguidos: u32,
}

impl Reloj {
    /// Lo que dijo la sonda de un aparato.
    pub fn responde(&mut self, id: &str, si: bool) {
        let antes = self.responden.insert(id.to_string(), si).unwrap_or(false);
        if si && !antes {
            self.aparecidos.insert(id.to_string());
        }
        if !si {
            self.aparecidos.remove(id);
        }
    }

    /// Una huella nueva de los chats, vista `ahora`.
    pub fn ver(&mut self, firma: Firma, ahora: i64) {
        if self.base.is_none() {
            self.base = Some(firma);
        }
        if self.vista.is_none_or(|(f, _)| f != firma) {
            self.vista = Some((firma, ahora));
        }
    }

    /// Con quien se puede sincronizar ahora.
    pub fn con_quien(&self) -> Vec<String> {
        self.responden
            .iter()
            .filter(|(_, si)| **si)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// **Pura**: si toca sincronizar `ahora`, y por que.
    pub fn toca(&self, ahora: i64) -> Option<Motivo> {
        if self.con_quien().is_empty() {
            return None;
        }
        if !self.aparecidos.is_empty() {
            return Some(Motivo::Aparecio);
        }
        if let (Some(base), Some((vista, desde))) = (self.base, self.vista)
            && vista != base
            && ahora - desde >= QUIETO
        {
            return Some(Motivo::Cambio);
        }
        if self.ultima.is_none_or(|u| ahora - u >= PERIODO) {
            return Some(Motivo::Periodo);
        }
        None
    }

    /// Empezo una vuelta.
    pub fn empezo(&mut self, ahora: i64) {
        self.ultima = Some(ahora);
        self.aparecidos.clear();
    }

    /// Acabo una vuelta, con la huella de despues (lo que escribio la propia
    /// vuelta no cuenta como cambio). Devuelve si hay que avisar del fallo.
    pub fn acabo(&mut self, bien: bool, firma: Firma, ahora: i64) -> bool {
        self.base = Some(firma);
        self.vista = Some((firma, ahora));
        if bien {
            self.fallos_seguidos = 0;
            false
        } else {
            self.fallos_seguidos += 1;
            self.fallos_seguidos == FALLOS_PARA_AVISAR
        }
    }
}

/// Arranca la automatica, si toca. Una vez, al abrir la aplicacion, despues
/// de la presencia (sin su sonda no se sabe quien contesta).
pub(crate) fn instalar(raiz: PathBuf, encendida: bool) {
    if !encendida || !presencia::encendida() {
        tracing::info!(encendida, "sincronizacion automatica apagada");
        return;
    }
    let (tx, rx) = mpsc::channel();
    presencia::oir(tx);
    let lanzado = std::thread::Builder::new()
        .name("sincro-automatica".into())
        .spawn(move || bucle(&raiz, rx));
    match lanzado {
        Ok(_) => tracing::info!("sincronizacion automatica encendida"),
        Err(e) => tracing::warn!(?e, "no se pudo lanzar la sincronizacion automatica"),
    }
}

fn bucle(raiz: &Path, rx: mpsc::Receiver<Novedad>) {
    // La carpeta con todos los chats: lo que viaja.
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, "x")
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| raiz.to_path_buf());
    let mut reloj = Reloj::default();
    let (tx_fin, rx_fin) = mpsc::channel::<Option<Fase>>();
    let mut corriendo = false;
    let mut latidos = 0u32;
    loop {
        match rx.recv_timeout(LATIDO) {
            Ok(Novedad::Responde(id, si)) => reloj.responde(&id, si),
            Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        // Las demas novedades que hayan llegado juntas.
        while let Ok(n) = rx.try_recv() {
            if let Novedad::Responde(id, si) = n {
                reloj.responde(&id, si);
            }
        }
        let ahora = super::ahora_ms();
        if let Ok(fin) = rx_fin.try_recv() {
            corriendo = false;
            acabo(&mut reloj, fin, firma_de(&carpeta), ahora);
        }
        latidos = latidos.wrapping_add(1);
        if latidos % LATIDOS_POR_FIRMA == 0 || reloj.base.is_none() {
            reloj.ver(firma_de(&carpeta), ahora);
        }
        if corriendo || super::hay_vueltas() {
            continue;
        }
        let Some(motivo) = reloj.toca(ahora) else {
            continue;
        };
        let ids = reloj.con_quien();
        let tx = tx_fin.clone();
        let lanzada = super::en_fondo::lanzar_callada(raiz, &ids, move |fin| {
            let _ = tx.send(fin);
        });
        // Si no se pudo (ya habia una de fondo, la de Flow), se cuenta como
        // hecha: se vuelve a mirar en el siguiente periodo.
        reloj.empezo(ahora);
        if lanzada {
            tracing::info!(?motivo, aparatos = ids.len(), "sincronizacion automatica");
            corriendo = true;
        }
    }
}

/// Lo que se hace al acabar una vuelta automatica.
fn acabo(reloj: &mut Reloj, fin: Option<Fase>, firma: Firma, ahora: i64) {
    let bien = matches!(fin, Some(Fase::Terminado { .. }));
    let avisar_fallo = reloj.acabo(bien, firma, ahora);
    match &fin {
        Some(f @ Fase::Terminado { titulo, trajo, .. }) => {
            presencia::difundir(Novedad::Registro(format!("Automática: {titulo}")));
            if *trajo && let Some(t) = super::en_fondo::contar(f) {
                super::al_movil::avisar(t);
            }
        }
        Some(Fase::Fallo(e)) => {
            tracing::info!(%e, fallos = reloj.fallos_seguidos, "la sincronizacion automatica fallo");
            presencia::difundir(Novedad::Registro(format!("Automática: no se pudo ({e})")));
            if avisar_fallo {
                super::al_movil::avisar(format!(
                    "La sincronización automática falla desde hace un rato: {e}"
                ));
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn firma(n: u64) -> Firma {
        Firma {
            ficheros: n,
            bytes: n,
            tocados: n as u128,
        }
    }

    #[test]
    fn sin_nadie_que_conteste_no_se_sincroniza_nunca() {
        let mut r = Reloj::default();
        r.ver(firma(1), 0);
        r.ver(firma(2), 0);
        assert_eq!(r.toca(PERIODO * 5), None);
        r.responde("tel", false);
        assert_eq!(r.toca(PERIODO * 5), None, "caso negativo: contesto que no");
    }

    #[test]
    fn al_aparecer_un_aparato_se_sincroniza_enseguida_y_solo_una_vez() {
        let mut r = Reloj::default();
        r.ver(firma(1), 0);
        r.empezo(0);
        r.responde("tel", true);
        assert_eq!(r.toca(1_000), Some(Motivo::Aparecio));
        r.empezo(1_000);
        r.acabo(true, firma(1), 2_000);
        // Sigue contestando: no vuelve a «aparecer».
        r.responde("tel", true);
        assert_eq!(r.toca(3_000), None);
        // Se va y vuelve: otra vez.
        r.responde("tel", false);
        r.responde("tel", true);
        assert_eq!(r.toca(4_000), Some(Motivo::Aparecio));
    }

    #[test]
    fn un_cambio_aqui_espera_treinta_segundos_quieto() {
        let mut r = Reloj::default();
        r.responde("tel", true);
        r.ver(firma(1), 0);
        r.empezo(0);
        r.acabo(true, firma(1), 0);
        r.ver(firma(2), 1_000);
        assert_eq!(r.toca(1_000 + QUIETO - 1), None, "aun escribiendo");
        // Sigue cambiando: el reloj vuelve a empezar.
        r.ver(firma(3), 20_000);
        assert_eq!(r.toca(1_000 + QUIETO + 1), None);
        assert_eq!(r.toca(20_000 + QUIETO), Some(Motivo::Cambio));
        // Lo que escribe la propia vuelta no es un cambio.
        r.empezo(50_000);
        r.acabo(true, firma(9), 60_000);
        assert_eq!(r.toca(60_000 + QUIETO * 2), None);
    }

    #[test]
    fn cada_diez_minutos_aunque_no_cambie_nada() {
        let mut r = Reloj::default();
        r.responde("tel", true);
        r.ver(firma(1), 0);
        assert_eq!(r.toca(0), Some(Motivo::Aparecio));
        r.empezo(0);
        r.acabo(true, firma(1), 0);
        assert_eq!(r.toca(PERIODO - 1), None);
        assert_eq!(r.toca(PERIODO), Some(Motivo::Periodo));
    }

    #[test]
    fn se_avisa_del_fallo_una_vez_al_tercero_seguido_y_un_acierto_lo_borra() {
        let mut r = Reloj::default();
        assert!(!r.acabo(false, firma(1), 0));
        assert!(!r.acabo(false, firma(1), 0));
        assert!(r.acabo(false, firma(1), 0), "el tercero avisa");
        assert!(!r.acabo(false, firma(1), 0), "caso negativo: el cuarto ya no");
        assert!(!r.acabo(true, firma(1), 0));
        assert_eq!(r.fallos_seguidos, 0);
    }

    #[test]
    fn la_firma_cambia_al_escribir_un_fichero() {
        let d = std::env::temp_dir().join(format!("pp-firma-{}", std::process::id()));
        std::fs::create_dir_all(d.join("chat")).unwrap();
        let a = firma_de(&d);
        std::fs::write(d.join("chat").join("x.txt"), b"hola").unwrap();
        let b = firma_de(&d);
        assert_ne!(a, b);
        assert_eq!(b.ficheros, a.ficheros + 1);
        let _ = std::fs::remove_dir_all(&d);
    }
}
