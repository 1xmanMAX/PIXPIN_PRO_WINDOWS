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
//! **Las tareas, al instante** (10-oct-2026, el usuario: «implementa una
//! funcion para sincronizar las tareas de forma rapida a todos los
//! dispositivos»). Esperar [`QUIETO`] esta bien para un mensaje que se
//! escribe, no para una casilla: quien tacha en el PC mira el movil y la
//! quiere tachada ya. Asi que lo que toca tareas lo dice ([`avisar_tareas`],
//! desde `pedidos`, el panel del chat, el pin de la lista y el recordatorio)
//! y, tras [`CALMA_TAREAS`] sin mas toques (o [`TOPE_TAREAS`] desde el
//! primero, si no paran), sale una vuelta **solo con esos chats** con todos
//! los que contestan (`vuelta::solo_estos`: el cable es el de siempre). Si
//! ya hay una vuelta en marcha, espera a que acabe y sale detras; nunca a la
//! par. Sin globos: lo hecho queda en «Actividad». Quien no contesta lo
//! recibe con la vuelta entera al aparecer, como todo. La de siempre sigue
//! detras ([`QUIETO`]): si la rapida no llego a alguno, esa lo arregla.
//!
//! La decision es pura ([`Reloj::siguiente`]) y se prueba sin red.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, mpsc};
use std::time::Duration;

use super::Fase;
use super::en_fondo::Arranque;
use super::presencia::{self, Novedad};

/// Lo que se espera sin mas toques en las tareas antes de mandarlas: lo
/// justo para que tres casillas seguidas viajen en una vuelta.
pub const CALMA_TAREAS: i64 = 600;
/// Y lo mas que se espera desde el primer toque, aunque no paren: quien
/// tacha una lista entera ve llegar las primeras mientras sigue.
pub const TOPE_TAREAS: i64 = 1_500;
/// Cada cuanto se vuelve a mirar si se puede, con tareas esperando a que
/// acabe otra vuelta (la del movil que llamo, la de la ventana).
const REINTENTO: Duration = Duration::from_millis(250);

/// Lo que se espera sin cambios en los chats antes de mandarlos: escribir un
/// mensaje o dibujar un trazo son muchos cambios seguidos.
pub const QUIETO: i64 = 30_000;
/// Cada cuanto se sincroniza aunque aqui no cambie nada.
pub const PERIODO: i64 = 10 * 60_000;
/// A los cuantos fallos seguidos se avisa.
pub const FALLOS_PARA_AVISAR: u32 = 3;
/// Cada cuanto se mira si hay que hacer algo.
const LATIDO: Duration = Duration::from_secs(5);
/// Cada cuanto se recorre la carpeta de los chats (ms).
const CADA_FIRMA: i64 = 15_000;

/// Por que se sincroniza ahora.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motivo {
    Aparecio,
    Cambio,
    Periodo,
}

/// Lo que toca hacer ahora ([`Reloj::siguiente`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Paso {
    Nada,
    /// La vuelta rapida, con las fichas de los chats cuyas tareas cambiaron.
    Tareas(BTreeSet<String>),
    /// La vuelta entera de siempre.
    Entera(Motivo),
    /// Habia tareas esperando y no contesta nadie: se olvidan, que la vuelta
    /// entera de cuando aparezca alguno ya las lleva.
    Olvidar,
}

/// Lo que le llega al hilo de la automatica.
enum Entrada {
    Presencia(Novedad),
    /// Cambiaron las tareas del chat de esta ficha (`almacen::Ficha::id`).
    Tareas(String),
    /// Acabo una vuelta lanzada por la automatica (`rapida`: la de tareas).
    Fin {
        fin: Option<Fase>,
        rapida: bool,
    },
    /// Si una persona de un chat compartido contesto en su ultima direccion.
    RespondePersona(String, bool),
    /// Se acabo de mirar quien de ellas contesta (el sondeo va en otro hilo).
    SondeoAcabado,
    /// Acabo la vuelta con las personas: si fue bien, lo que trajo (para
    /// avisar) y el fallo, si lo hubo.
    FinPersonas {
        bien: bool,
        trajo: Vec<String>,
        fallo: Option<String>,
    },
}

/// **Cada cuanto se mira si las personas de los chats compartidos estan en
/// la red** (Android 13-oct-2026, `SincronizacionAutomatica.conPersonas`).
/// Las del grupo las sondea la presencia; a estas, la automatica, y sin
/// prisa: un minuto, lo que tarda la sonda de la presencia sin nadie
/// mirando. Solo si hay alguna persona con direccion conocida.
pub const SONDEO_PERSONAS: i64 = 60_000;

/// **Con quien de los chats compartidos se sincroniza solo**: las personas
/// que estan dentro de un chat donde tambien estoy yo dentro, sin mis propios
/// aparatos (con esos ya va la vuelta del grupo). Una vez cada una.
pub fn personas_a_llamar(
    lista: &[pixpin_sincro::compartidos::ChatCompartido],
    yo: &str,
    mios: &BTreeSet<String>,
) -> Vec<(String, String)> {
    let mut salida: Vec<(String, String)> = Vec::new();
    for c in lista
        .iter()
        .filter(|c| pixpin_sincro::compartidos::dentro(c, yo))
    {
        for p in c.personas.iter().filter(|p| p.dentro()) {
            if p.id != yo && !mios.contains(&p.id) && !salida.iter().any(|(i, _)| *i == p.id) {
                salida.push((p.id.clone(), p.nombre.clone()));
            }
        }
    }
    salida
}

/// Las personas a llamar con su ultima direccion conocida (`id`, nombre,
/// host y puerto). Sin direccion no se busca por la red: como el movil, se
/// espera a que llame ella o a que se sincronice a mano.
fn personas_con_direccion(raiz: &Path) -> Vec<(String, String, String, u16)> {
    let Ok(id) = super::leer_identidad(raiz) else {
        return Vec::new();
    };
    let lista = pixpin_sincro::compartidos::CompartidosEnDisco::nuevo(raiz).leer();
    if lista.is_empty() {
        return Vec::new();
    }
    let mut mios: BTreeSet<String> = id.miembros.iter().map(|m| m.id.clone()).collect();
    mios.insert(id.yo.id.clone());
    let dirs = super::leer_direcciones(raiz);
    personas_a_llamar(&lista, &id.yo.id, &mios)
        .into_iter()
        .filter_map(|(pid, nombre)| {
            let (_, h, p) = dirs.iter().find(|(i, _, _)| *i == pid)?;
            Some((pid, nombre, h.clone(), *p))
        })
        .collect()
}

/// Lo hecho con una persona, en una linea (`resumen` del movil).
fn resumen(h: &pixpin_sincro::protocolo::Hecho) -> String {
    let mut v = Vec::new();
    if h.traidos > 0 {
        v.push(format!("{} traídos", h.traidos));
    }
    if h.fusionados > 0 {
        v.push(format!("{} juntados", h.fusionados));
    }
    if h.borrados > 0 {
        v.push(format!("{} borrados", h.borrados));
    }
    v.join(" · ")
}

/// Por donde se avisa al hilo; sin automatica (apagada), no hay.
static AVISOS: OnceLock<mpsc::Sender<Entrada>> = OnceLock::new();

/// **Cambiaron las tareas del chat `ficha`** (`almacen::Ficha::id`, la
/// carpeta de `proyectos/`): que salgan ya hacia los aparatos que contestan.
/// Lo llama quien acaba de escribirlas, despues de escribir. Con la
/// automatica apagada no hace nada.
pub(crate) fn avisar_tareas(ficha: &str) {
    if let Some(tx) = AVISOS.get() {
        let _ = tx.send(Entrada::Tareas(ficha.to_string()));
    }
}

/// Como [`avisar_tareas`], con la carpeta del chat (`proyectos/<ficha>`).
pub(crate) fn avisar_tareas_en(carpeta: &Path) {
    if let Some(ficha) = carpeta.file_name().and_then(|n| n.to_str()) {
        avisar_tareas(ficha);
    }
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
    /// Las fichas con tareas cambiadas que aun no han salido.
    pub tareas: BTreeSet<String>,
    /// Cuando llego el primer aviso de tareas sin mandar, y el ultimo.
    pub tareas_desde: i64,
    pub tareas_ultimo: i64,
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

    /// Cambiaron las tareas del chat de `ficha`, `ahora`.
    pub fn avisan_tareas(&mut self, ficha: &str, ahora: i64) {
        if self.tareas.is_empty() {
            self.tareas_desde = ahora;
        }
        self.tareas.insert(ficha.to_string());
        self.tareas_ultimo = ahora;
    }

    /// Cuanto falta para mandar las tareas que esperan (0: ya), o `None` si
    /// no espera ninguna.
    pub fn espera_tareas(&self, ahora: i64) -> Option<i64> {
        if self.tareas.is_empty() {
            return None;
        }
        let calma = self.tareas_ultimo + CALMA_TAREAS;
        let tope = self.tareas_desde + TOPE_TAREAS;
        Some((calma.min(tope) - ahora).max(0))
    }

    /// **Pura**: lo que toca `ahora`. Con `ocupado` (una vuelta en marcha,
    /// nuestra o del movil que llamo) nada: lo que espera se queda y sale
    /// detras. Las tareas van antes que la vuelta entera, que tarda mas.
    pub fn siguiente(&self, ahora: i64, ocupado: bool) -> Paso {
        if ocupado {
            return Paso::Nada;
        }
        if self.espera_tareas(ahora) == Some(0) {
            return if self.con_quien().is_empty() {
                Paso::Olvidar
            } else {
                Paso::Tareas(self.tareas.clone())
            };
        }
        match self.toca(ahora) {
            Some(m) => Paso::Entera(m),
            None => Paso::Nada,
        }
    }

    /// Salieron (o se olvidan) las tareas que esperaban.
    pub fn tomar_tareas(&mut self) -> BTreeSet<String> {
        std::mem::take(&mut self.tareas)
    }

    /// Empezo una vuelta entera: lo que esperaba de tareas va dentro (se
    /// aviso despues de escribirlo, y la vuelta lee el disco de ahora).
    pub fn empezo(&mut self, ahora: i64) {
        self.tareas.clear();
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
    if AVISOS.set(tx.clone()).is_err() {
        return;
    }
    // Lo de la presencia entra por el mismo canal que los avisos de tareas y
    // los finales de vuelta: asi el hilo duerme en un solo sitio y despierta
    // con cualquiera de los tres.
    let (tx_presencia, rx_presencia) = mpsc::channel();
    presencia::oir(tx_presencia);
    let puente = std::thread::Builder::new()
        .name("sincro-automatica-oye".into())
        .spawn(move || {
            while let Ok(n) = rx_presencia.recv() {
                if tx.send(Entrada::Presencia(n)).is_err() {
                    return;
                }
            }
        });
    if let Err(e) = puente {
        tracing::warn!(?e, "no se pudo lanzar la sincronizacion automatica");
        return;
    }
    let lanzado = std::thread::Builder::new()
        .name("sincro-automatica".into())
        .spawn(move || bucle(&raiz, rx));
    match lanzado {
        Ok(_) => tracing::info!("sincronizacion automatica encendida"),
        Err(e) => tracing::warn!(?e, "no se pudo lanzar la sincronizacion automatica"),
    }
}

/// Cuanto dormir hasta la siguiente mirada.
fn cuanto_dormir(reloj: &Reloj, ahora: i64, corriendo: bool) -> Duration {
    if corriendo {
        // El final de la vuelta despierta por el canal.
        return LATIDO;
    }
    match reloj.espera_tareas(ahora) {
        // Toca ya y no se pudo (otra vuelta en marcha): se reintenta pronto.
        Some(0) => REINTENTO,
        Some(ms) => Duration::from_millis(ms as u64).min(LATIDO),
        None => LATIDO,
    }
}

fn bucle(raiz: &Path, rx: mpsc::Receiver<Entrada>) {
    // La carpeta con todos los chats: lo que viaja.
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, "x")
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| raiz.to_path_buf());
    let mut reloj = Reloj::default();
    let mut corriendo = false;
    let mut firmado: Option<i64> = None;
    // **Las personas de los chats compartidos llevan su propio reloj**:
    // aparecer, un cambio aqui o el periodo, por cada una (como el movil).
    let mut personas = Reloj::default();
    let mut sondeado: Option<i64> = None;
    let mut sondeando = false;
    loop {
        let espera = cuanto_dormir(&reloj, super::ahora_ms(), corriendo);
        let primera = match rx.recv_timeout(espera) {
            Ok(e) => Some(e),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        };
        // Y las demas que hayan llegado juntas.
        for e in primera
            .into_iter()
            .chain(std::iter::from_fn(|| rx.try_recv().ok()))
        {
            let ahora = super::ahora_ms();
            match e {
                Entrada::Presencia(Novedad::Responde(id, si)) => reloj.responde(&id, si),
                Entrada::Presencia(_) => {}
                Entrada::Tareas(ficha) => reloj.avisan_tareas(&ficha, ahora),
                Entrada::RespondePersona(id, si) => personas.responde(&id, si),
                Entrada::SondeoAcabado => sondeando = false,
                Entrada::FinPersonas { bien, trajo, fallo } => {
                    corriendo = false;
                    let avisar = personas.acabo(bien, firma_de(&carpeta), ahora);
                    for t in trajo {
                        presencia::difundir(Novedad::Registro(t.clone()));
                        super::al_movil::avisar(t);
                    }
                    if let Some(e) = fallo {
                        presencia::difundir(Novedad::Registro(e.clone()));
                        if avisar {
                            super::al_movil::avisar(e);
                        }
                    }
                }
                Entrada::Fin { fin, rapida } => {
                    corriendo = false;
                    if rapida {
                        acabo_rapida(fin);
                    } else {
                        acabo(&mut reloj, fin, firma_de(&carpeta), ahora);
                    }
                }
            }
        }
        let ahora = super::ahora_ms();
        if firmado.is_none_or(|f| ahora - f >= CADA_FIRMA) {
            firmado = Some(ahora);
            let f = firma_de(&carpeta);
            reloj.ver(f, ahora);
            personas.ver(f, ahora);
        }
        // Quien de las personas contesta, sin prisa y en otro hilo (una que
        // no esta tarda segundos en no contestar). Sin chats compartidos, ni
        // se mira: ni red ni disco de mas.
        if !sondeando && sondeado.is_none_or(|s| ahora - s >= SONDEO_PERSONAS) {
            sondeado = Some(ahora);
            let quienes = personas_con_direccion(raiz);
            personas
                .responden
                .retain(|id, _| quienes.iter().any(|(q, ..)| q == id));
            if !quienes.is_empty()
                && let Some(tx) = AVISOS.get().cloned()
            {
                sondeando = true;
                let lanzado = std::thread::Builder::new()
                    .name("sincro-personas-sonda".into())
                    .spawn(move || {
                        for (id, _, host, puerto) in quienes {
                            let si = super::sondear_uno(&host, puerto);
                            let _ = tx.send(Entrada::RespondePersona(id, si));
                        }
                        let _ = tx.send(Entrada::SondeoAcabado);
                    });
                if lanzado.is_err() {
                    sondeando = false;
                }
            }
        }
        match reloj.siguiente(ahora, corriendo || super::hay_vueltas()) {
            Paso::Nada => {}
            Paso::Olvidar => {
                reloj.tomar_tareas();
            }
            Paso::Tareas(fichas) => {
                let chats: BTreeSet<String> = fichas
                    .iter()
                    .filter_map(|f| pixpin_proyecto::vista::chat_de_ficha(raiz, f))
                    .collect();
                if chats.is_empty() {
                    reloj.tomar_tareas();
                    continue;
                }
                let ids = reloj.con_quien();
                let tx = AVISOS.get().cloned();
                let lanzada =
                    super::en_fondo::lanzar_tareas(raiz, &ids, chats.clone(), move |fin| {
                        if let Some(tx) = tx {
                            let _ = tx.send(Entrada::Fin { fin, rapida: true });
                        }
                    });
                match lanzada {
                    Arranque::Lanzada => {
                        reloj.tomar_tareas();
                        corriendo = true;
                        tracing::info!(?chats, aparatos = ids.len(), "tareas al instante");
                    }
                    // La de Flow (u otra): se reintenta en cuanto acabe.
                    Arranque::Ocupada => {}
                    Arranque::SinNadie => {
                        reloj.tomar_tareas();
                    }
                }
            }
            Paso::Entera(motivo) => {
                let ids = reloj.con_quien();
                let tx = AVISOS.get().cloned();
                let lanzada = super::en_fondo::lanzar_callada(raiz, &ids, move |fin| {
                    if let Some(tx) = tx {
                        let _ = tx.send(Entrada::Fin { fin, rapida: false });
                    }
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
        // **Con las personas de los chats compartidos** (`conPersonas` del
        // movil): las que contestan en su ultima direccion, por las mismas
        // tres razones que el grupo. Solo esos chats, una vuelta cada una, y
        // nunca a la par que otra vuelta.
        if !corriendo
            && !super::hay_vueltas()
            && personas.toca(ahora).is_some()
            && let Some(tx) = AVISOS.get().cloned()
        {
            let ids = personas.con_quien();
            personas.empezo(ahora);
            corriendo = true;
            let raiz = raiz.to_path_buf();
            let lanzado = std::thread::Builder::new()
                .name("sincro-personas".into())
                .spawn(move || {
                    let fin = con_personas(&raiz, &ids);
                    let _ = tx.send(fin);
                });
            if lanzado.is_err() {
                corriendo = false;
            }
        }
    }
}

/// Una vuelta con cada persona que contesto (`VueltaConPersona`), con la
/// direccion apuntada. Devuelve lo que contar al acabar.
fn con_personas(raiz: &Path, ids: &[String]) -> Entrada {
    // Mientras dura, ni la ventana ni el grupo arrancan otra.
    let _en_curso = super::EnCurso::nueva();
    let disco = pixpin_proyecto::vista::DiscoPc::nuevo(raiz).con_avisos(|c| {
        if c == pixpin_sincro::disco::Cambio::Identidad {
            presencia::difundir(Novedad::Identidad);
        } else {
            crate::ventana_chat::refrescar();
            crate::tareas::refrescar();
        }
    });
    let quienes = personas_con_direccion(raiz);
    let mi_puerto = presencia::puerto().map_or(0, u32::from);
    let (mut trajo, mut fallo, mut bien) = (Vec::new(), None, true);
    for (id, nombre, host, puerto) in quienes.into_iter().filter(|(i, ..)| ids.contains(i)) {
        let Some(n) = super::nonce() else { continue };
        let r = super::conectar(&host, puerto)
            .map_err(|e| e.to_string())
            .and_then(|flujo| {
                pixpin_sincro::invitacion::vuelta_con_persona(
                    flujo,
                    &disco,
                    Some(&id),
                    None,
                    &super::ahora_ms,
                    mi_puerto,
                    n,
                    &mut |_| {},
                )
                .map_err(|e| e.to_string())
            });
        match r {
            Ok(r) => {
                if r.puerto > 0 {
                    super::apuntar_direccion(raiz, &id, &host, r.puerto as u16);
                }
                if r.trajo_algo() {
                    trajo.push(format!(
                        "Chat compartido con {}: {}",
                        r.quien.nombre,
                        resumen(&r.hecho)
                    ));
                }
            }
            // Ocupado (otra vuelta en marcha alli) no es un fallo: la
            // proxima.
            Err(e) if e == pixpin_sincro::mensajes::OCUPADO => {}
            Err(e) => {
                bien = false;
                fallo = Some(format!("No se pudo sincronizar solo con {nombre} ({e})"));
            }
        }
    }
    Entrada::FinPersonas { bien, trajo, fallo }
}

/// Lo que se hace al acabar una vuelta de tareas: solo apuntarlo. Ni globo
/// si sale bien (quien tacha ya lo ve) ni si falla: la vuelta entera de
/// detras ([`QUIETO`]) lo vuelve a intentar, y esa si avisa si falla seguido.
/// Tampoco se toca la huella: lo demas que cambiara en los chats sigue
/// pendiente para esa vuelta.
fn acabo_rapida(fin: Option<Fase>) {
    match &fin {
        Some(Fase::Terminado { titulo, .. }) => {
            presencia::difundir(Novedad::Registro(format!("Tareas al instante: {titulo}")));
        }
        Some(Fase::Fallo(e)) => {
            tracing::info!(%e, "las tareas al instante no salieron");
            presencia::difundir(Novedad::Registro(format!(
                "Tareas al instante: no se pudo ({e})"
            )));
        }
        _ => {}
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
        assert!(
            !r.acabo(false, firma(1), 0),
            "caso negativo: el cuarto ya no"
        );
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

    fn con_tel() -> Reloj {
        let mut r = Reloj::default();
        r.responde("tel", true);
        r.ver(firma(1), 0);
        // Lo de aparecer ya se hizo: aqui se mira lo de las tareas.
        r.empezo(0);
        r.acabo(true, firma(1), 0);
        r
    }

    fn fichas(v: &[&str]) -> BTreeSet<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn una_tarea_tachada_sale_en_cuanto_se_calma_y_no_antes() {
        let mut r = con_tel();
        assert_eq!(r.siguiente(1_000, false), Paso::Nada);
        r.avisan_tareas("obra", 1_000);
        assert_eq!(r.espera_tareas(1_000), Some(CALMA_TAREAS));
        assert_eq!(
            r.siguiente(1_000 + CALMA_TAREAS - 1, false),
            Paso::Nada,
            "aun tachando"
        );
        assert_eq!(
            r.siguiente(1_000 + CALMA_TAREAS, false),
            Paso::Tareas(fichas(&["obra"]))
        );
        assert_eq!(r.tomar_tareas(), fichas(&["obra"]));
        assert_eq!(
            r.siguiente(1_000 + CALMA_TAREAS, false),
            Paso::Nada,
            "caso negativo: ya salio"
        );
        assert_eq!(r.espera_tareas(5_000), None);
    }

    #[test]
    fn varias_casillas_seguidas_van_en_una_vuelta_y_el_tope_no_deja_esperar_de_mas() {
        let mut r = con_tel();
        // Un clic cada 400 ms: la calma no llega nunca, el tope si.
        for (i, t) in [0, 400, 800, 1_200].into_iter().enumerate() {
            r.avisan_tareas(if i % 2 == 0 { "obra" } else { "guardados" }, 10_000 + t);
            if 10_000 + t < 10_000 + TOPE_TAREAS {
                assert_eq!(r.siguiente(10_000 + t, false), Paso::Nada);
            }
        }
        assert_eq!(r.siguiente(10_000 + TOPE_TAREAS - 1, false), Paso::Nada);
        assert_eq!(
            r.siguiente(10_000 + TOPE_TAREAS, false),
            Paso::Tareas(fichas(&["guardados", "obra"])),
            "dos chats en la misma vuelta"
        );
    }

    #[test]
    fn con_otra_vuelta_en_marcha_las_tareas_esperan_y_salen_detras() {
        let mut r = con_tel();
        r.avisan_tareas("obra", 0);
        assert_eq!(r.siguiente(5_000, true), Paso::Nada, "nunca a la par");
        // Mientras, otra tarea: se suma a la cola, no se pierde.
        r.avisan_tareas("guardados", 5_100);
        assert_eq!(r.siguiente(5_200, true), Paso::Nada);
        // Acabo la otra: salen las dos juntas, y van antes que la entera.
        assert_eq!(
            r.siguiente(PERIODO + 1, false),
            Paso::Tareas(fichas(&["guardados", "obra"]))
        );
        r.tomar_tareas();
        assert_eq!(
            r.siguiente(PERIODO + 1, false),
            Paso::Entera(Motivo::Periodo)
        );
    }

    #[test]
    fn caso_negativo_sin_nadie_que_conteste_las_tareas_se_olvidan_sin_salir() {
        let mut r = Reloj::default();
        r.avisan_tareas("obra", 0);
        assert_eq!(r.siguiente(CALMA_TAREAS, false), Paso::Olvidar);
        r.responde("tel", false);
        assert_eq!(r.siguiente(CALMA_TAREAS, false), Paso::Olvidar);
        r.tomar_tareas();
        assert_eq!(r.siguiente(CALMA_TAREAS, false), Paso::Nada);
    }

    #[test]
    fn la_vuelta_entera_se_lleva_lo_que_esperaba_de_tareas() {
        let mut r = con_tel();
        r.responde("tel", false);
        r.responde("tel", true);
        r.avisan_tareas("obra", 100);
        // Aun en calma: toca la entera (aparecio) y se lleva las tareas.
        assert_eq!(r.siguiente(200, false), Paso::Entera(Motivo::Aparecio));
        r.empezo(200);
        assert_eq!(r.espera_tareas(10_000), None);
    }

    #[test]
    fn el_hilo_duerme_lo_justo_para_mandar_las_tareas_a_tiempo() {
        let mut r = con_tel();
        assert_eq!(cuanto_dormir(&r, 0, false), LATIDO);
        r.avisan_tareas("obra", 0);
        assert_eq!(
            cuanto_dormir(&r, 100, false),
            Duration::from_millis((CALMA_TAREAS - 100) as u64)
        );
        assert_eq!(
            cuanto_dormir(&r, CALMA_TAREAS, false),
            REINTENTO,
            "ocupado: reintentar pronto"
        );
        assert_eq!(
            cuanto_dormir(&r, 0, true),
            LATIDO,
            "con una vuelta suya en marcha, la despierta su final"
        );
    }

    #[test]
    fn sin_automatica_avisar_tareas_no_hace_nada() {
        // Caso negativo: en las pruebas no hay hilo; avisar no puede romper.
        avisar_tareas("obra");
        avisar_tareas_en(Path::new("C:/x/proyectos/obra"));
    }

    fn persona(id: &str, nombre: &str) -> pixpin_sincro::compartidos::Yo {
        pixpin_sincro::compartidos::Yo {
            id: id.into(),
            nombre: nombre.into(),
            codigo: pixpin_sincro::grupo::codigo_de_aparato(id),
            llaves: pixpin_sincro::compartidos::Llaves::nuevas(),
        }
    }

    #[test]
    fn solo_se_llama_a_las_personas_de_chats_donde_estamos_dentro() {
        use pixpin_sincro::compartidos::{crear, invitar, quitar};
        let (yo, tab, ana, eva, luis) = (
            persona("id-pc", "Max"),
            persona("id-tab", "Max"),
            persona("id-ana", "Ana"),
            persona("id-eva", "Eva"),
            persona("id-luis", "Luis"),
        );
        let mut tesis = crear("p1", "Tesis", &yo, 1);
        tesis = invitar(&tesis, &yo, &tab.ficha(0), 2).unwrap();
        tesis = invitar(&tesis, &yo, &ana.ficha(0), 3).unwrap();
        tesis = invitar(&tesis, &yo, &eva.ficha(0), 4).unwrap();
        // Eva se fue: ya no se la llama.
        tesis = quitar(&tesis, &eva, "id-eva", 5).unwrap();
        // Un chat del que yo sali: Luis no se llama por el.
        let mut otro = crear("p2", "Otro", &yo, 1);
        otro = invitar(&otro, &yo, &luis.ficha(0), 2).unwrap();
        otro = invitar(&otro, &yo, &ana.ficha(0), 3).unwrap();
        otro = quitar(&otro, &yo, "id-pc", 4).unwrap();
        let mios: BTreeSet<String> = ["id-pc".to_string(), "id-tab".into()].into();
        let l = personas_a_llamar(&[tesis.clone(), otro.clone()], "id-pc", &mios);
        // Ana, una sola vez; ni yo, ni mi tableta (va por el grupo), ni Eva,
        // ni Luis.
        assert_eq!(l, [("id-ana".to_string(), "Ana".to_string())]);
        // Caso negativo: sin chats compartidos no hay a nadie que llamar, ni
        // red que gastar.
        assert!(personas_a_llamar(&[], "id-pc", &mios).is_empty());
        // Y quien no esta en ningun chat tampoco llama a nadie.
        assert!(personas_a_llamar(&[tesis, otro], "id-desconocido", &BTreeSet::new()).is_empty());
    }

    #[test]
    fn con_las_personas_el_reloj_es_el_del_grupo_y_sin_respuesta_no_se_llama() {
        // Las personas llevan su propio `Reloj`: aparecer dispara, y sin
        // ninguna que conteste no se intenta nunca (no se gasta red).
        let mut r = Reloj::default();
        assert_eq!(r.toca(0), None);
        r.responde("id-ana", false);
        assert_eq!(r.toca(10 * PERIODO), None);
        r.responde("id-ana", true);
        assert_eq!(r.toca(0), Some(Motivo::Aparecio));
        r.empezo(0);
        assert_eq!(r.toca(1_000), None);
        const { assert!(SONDEO_PERSONAS >= 60_000, "las personas se miran sin prisa") };
    }
}
