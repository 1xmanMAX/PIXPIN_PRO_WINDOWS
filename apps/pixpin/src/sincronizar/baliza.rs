//! **Descubrir rapido y prioridad al grabar** (28-oct-2026), lo de este
//! equipo. El cable y las cuentas son `pixpin_sincro::baliza`; la guia para
//! Android, `docs/movil-como-camara.md` («Descubrir rápido y prioridad al
//! grabar»).
//!
//! El usuario: «el MOVIL tarda demasiado en saber si el PC esta grabando y
//! no puedo hacer fotos rapido… cuando el PC grabe, modo PRIORITARIO para la
//! grabacion y sus fotos… perder algo seria perder una reunion».
//!
//! Dos hilos que viven lo que la aplicacion (los arranca la presencia):
//!
//! - **`sincro-baliza`** manda la baliza: cada segundo grabando, cada 15 s
//!   sin grabar, y al momento al empezar, pausar, seguir o acabar
//!   ([`cambio_la_grabacion`], desde el aviso a los moviles). Por difusion y
//!   multidifusion en cada red de este equipo, directa a la ultima direccion
//!   de cada aparato del grupo y, grabando sin oir a nadie, a la red entera.
//!   Al arrancar (y al cambiar de red) pregunta: quien la oye contesta con la
//!   suya, directa, y se sabe de el sin esperar a su latido.
//! - **`sincro-baliza-oye`** oye las de los demas: apunta su direccion (la
//!   del datagrama, que es la de verdad aunque el DHCP la haya cambiado), le
//!   dice a la automatica que esta ([`super::presencia::Novedad::Responde`])
//!   y, si este equipo graba, le manda el anuncio de la camara al momento.
//!
//! **El modo prioritario** ([`grabando_alguien`]): mientras este equipo o
//! uno del grupo graba, la sincronizacion automatica no arranca nada (ni la
//! vuelta entera, ni las tareas, ni las personas: `automatica`), la sonda va
//! deprisa y la escucha atiende cada llamada en 20 ms. Y **este equipo se
//! da por ocupado** ([`pixpin_sincro::protocolo::ocupar`]) mientras graba:
//! quien llame para sincronizar oye «ocupado» y vuelve luego, pero las fotos
//! y clips del movil y el `camara_remota` se atienden igual (`suelto` y la
//! camara no tocan lo sincronizado). Al acabar, el chat de la grabacion sale
//! primero (`automatica::grabacion_guardada`) y lo demas detras.
//!
//! **El cortafuegos**: la baliza escucha en UDP [`b::PUERTO_BALIZA`] el mismo
//! programa que ya escucha en TCP 47474; la regla que crea Windows al pulsar
//! «Permitir» en su aviso vale para los dos protocolos. En una red marcada
//! como «Pública» no entra nada: lo explica la guia y
//! `herramientas/cortafuegos-sincro.ps1` pone las reglas a mano.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

use pixpin_proyecto::vista::DiscoPc;
use pixpin_sincro::baliza::{self as b, Baliza, Diagnostico, Emisor, Interfaz, Oidas, Oyente};
use pixpin_store::Catalogo;

use super::presencia::{self, Novedad};

/// La grabacion de este equipo, como la cuenta la barra de «Grabar clase»
/// (`camara::para_la_baliza`).
pub(crate) struct Local {
    /// El id de la grabacion (`camara_remota.grabacion`).
    pub id: String,
    pub en_pausa: bool,
    /// En que milisegundo va.
    pub ms: i64,
    /// Si la camara del movil esta encendida en la barra.
    pub camara: bool,
}

/// Lo que se sabe del grupo, para el que oye: lo pone el que manda.
#[derive(Clone)]
struct Grupo {
    clave: [u8; 32],
    yo: String,
    miembros: Vec<String>,
}

/// Lo oido de cada aparato del grupo.
struct Oido {
    cuando: Instant,
    baliza: Baliza,
}

static GRUPO: Mutex<Option<Grupo>> = Mutex::new(None);
static OIDOS: Mutex<Option<HashMap<String, Oido>>> = Mutex::new(None);
/// Si este equipo esta grabando ahora (lo apunta el que manda).
static GRABANDO_AQUI: AtomicBool = AtomicBool::new(false);
/// Cambio la grabacion de este equipo: la baliza sale al momento.
static CAMBIO: AtomicBool = AtomicBool::new(false);
/// Cambiaron las redes de este equipo: el oyente se vuelve a apuntar.
static REDES_NUEVAS: AtomicBool = AtomicBool::new(false);

/// La baliza sale ya, sin esperar a su latido: empezo, se pauso, siguio o
/// acabo la grabacion (lo llama el aviso a los moviles, `camara`).
pub(crate) fn cambio_la_grabacion() {
    CAMBIO.store(true, Ordering::SeqCst);
}

/// Cambio la red (lo dice la automatica): se vuelven a leer las redes y se
/// pregunta otra vez quien esta.
pub(crate) fn cambio_la_red() {
    REDES_NUEVAS.store(true, Ordering::SeqCst);
}

/// Si este equipo graba ahora.
pub(crate) fn grabando_aqui() -> bool {
    GRABANDO_AQUI.load(Ordering::SeqCst)
}

/// **El modo prioritario**: si este equipo, o alguno del grupo que se ha
/// oido hace menos de [`b::VIGENCIA_GRABANDO_MS`], esta grabando.
pub(crate) fn grabando_alguien() -> bool {
    grabando_aqui() || !otros_grabando().is_empty()
}

/// Los aparatos del grupo que graban ahora, segun su baliza.
pub(crate) fn otros_grabando() -> Vec<String> {
    let vigencia = Duration::from_millis(b::VIGENCIA_GRABANDO_MS as u64);
    OIDOS
        .lock()
        .ok()
        .and_then(|o| {
            o.as_ref().map(|o| {
                o.iter()
                    .filter(|(_, x)| x.baliza.graba() && x.cuando.elapsed() < vigencia)
                    .map(|(id, _)| id.clone())
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// Hace cuanto se oyo a alguien del grupo (ms), o `None` si a nadie.
fn oido_hace() -> Option<i64> {
    OIDOS.lock().ok().and_then(|o| {
        o.as_ref()?
            .values()
            .map(|x| x.cuando.elapsed().as_millis() as i64)
            .min()
    })
}

/// Las redes de este equipo; sin ninguna (Windows no contesto), la de la
/// ruta por defecto como si fuera una /24.
fn redes() -> Vec<Interfaz> {
    let mut v: Vec<Interfaz> = pixpin_shell::red::interfaces_ipv4()
        .into_iter()
        .map(|(ip, prefijo)| Interfaz { ip, prefijo })
        .collect();
    if v.is_empty()
        && let Some(ip) = super::ip_local().and_then(|t| t.parse::<Ipv4Addr>().ok())
    {
        v.push(Interfaz { ip, prefijo: 24 });
    }
    v
}

/// Las ultimas direcciones conocidas de los demas del grupo.
fn conocidas(raiz: &Path, g: &Grupo) -> Vec<Ipv4Addr> {
    super::leer_direcciones(raiz)
        .into_iter()
        .filter(|(id, ..)| *id != g.yo && g.miembros.contains(id))
        .filter_map(|(_, h, _)| h.parse().ok())
        .collect()
}

/// Arranca los dos hilos. Lo llama la presencia, con la puerta ya abierta.
pub(crate) fn instalar(raiz: PathBuf, puerto_tcp: u16) {
    let (tx, rx) = mpsc::channel::<Ipv4Addr>();
    let r = raiz.clone();
    let mandar = std::thread::Builder::new()
        .name("sincro-baliza".into())
        .spawn(move || mandar(&r, puerto_tcp, rx));
    let oir = std::thread::Builder::new()
        .name("sincro-baliza-oye".into())
        .spawn(move || oir(&raiz, tx));
    if mandar.is_err() || oir.is_err() {
        tracing::warn!("no se pudo lanzar la baliza: se descubre solo por mDNS y la sonda");
    }
}

/// Lo de la grabacion de este equipo en cada vuelta del que manda.
struct Grabacion {
    desde: Instant,
    /// Si se oyo a alguien del grupo desde que empezo.
    oido: bool,
    /// Si ya se dijo al usuario que no aparece nadie.
    diagnosticada: bool,
    /// Si este equipo se dio por ocupado (y hay que soltarlo al acabar).
    ocupado: bool,
}

fn mandar(raiz: &Path, puerto_tcp: u16, contestar: mpsc::Receiver<Ipv4Addr>) {
    let mut ritmo = b::Ritmo::default();
    let mut secuencia: u32 = 0;
    let mut leido: Option<Instant> = None;
    let mut grupo: Option<Grupo> = None;
    let mut redes_leidas: Option<Instant> = None;
    let mut emisor: Option<Emisor> = None;
    let mut preguntar = true;
    let mut grabacion: Option<Grabacion> = None;
    let disco = DiscoPc::nuevo(raiz);
    // Lo que dice la sonda TCP: si alguno contesta en su ultima direccion,
    // no hace falta decir nada aunque su PixPin no mande balizas (uno de
    // antes).
    let (tx, novedades) = mpsc::channel();
    presencia::oir(tx);
    let mut contesto: Option<Instant> = None;
    loop {
        std::thread::sleep(Duration::from_millis(50));
        for n in novedades.try_iter() {
            if let Novedad::Responde(_, true) = n {
                contesto = Some(Instant::now());
            }
        }
        // El grupo, cada dos segundos: sin grupo no hay a quien llamar.
        if leido.is_none_or(|l| l.elapsed() > Duration::from_secs(2)) {
            leido = Some(Instant::now());
            grupo = super::leer_identidad(raiz).ok().and_then(|id| {
                let codigo = id.codigo.filter(|c| !c.trim().is_empty())?;
                Some(Grupo {
                    clave: pixpin_sincro::codigo::clave_de_grupo(&codigo),
                    yo: id.yo.id.clone(),
                    miembros: id.miembros.iter().map(|m| m.id.clone()).collect(),
                })
            });
            if let Ok(mut g) = GRUPO.lock() {
                *g = grupo.clone();
            }
        }
        // Las redes, cada medio minuto o al cambiar.
        let nuevas = REDES_NUEVAS.swap(false, Ordering::SeqCst);
        if nuevas || redes_leidas.is_none_or(|l| l.elapsed() > Duration::from_secs(30)) {
            redes_leidas = Some(Instant::now());
            let ahora = redes();
            if nuevas || emisor.as_ref().is_none_or(|e| e.redes() != ahora) {
                emisor = Emisor::nuevo(b::PUERTO_BALIZA, &ahora).ok();
                // Red nueva: se pregunta otra vez quien esta, y el oyente se
                // apunta a la multidifusion en las nuevas.
                preguntar = true;
                OYENTE_DE_NUEVO.store(true, Ordering::SeqCst);
                tracing::info!(redes = ?ahora, "baliza: redes de este equipo");
            }
        }
        let local = super::camara::para_la_baliza(super::ahora_ms());
        let graba = local.is_some();
        let cambio = CAMBIO.swap(false, Ordering::SeqCst) | (graba != grabando_aqui());
        GRABANDO_AQUI.store(graba, Ordering::SeqCst);
        // **Ocupado mientras se graba**: quien llame para sincronizar, que
        // vuelva luego; las fotos y la camara se atienden igual.
        match (&mut grabacion, graba) {
            (None, true) => {
                tracing::info!("baliza: grabando, modo prioritario");
                grabacion = Some(Grabacion {
                    desde: Instant::now(),
                    oido: false,
                    diagnosticada: false,
                    ocupado: false,
                });
            }
            (Some(g), false) => {
                if g.ocupado {
                    pixpin_sincro::protocolo::soltar(&disco);
                }
                grabacion = None;
                tracing::info!("baliza: acabo la grabacion, fin del modo prioritario");
            }
            _ => {}
        }
        if let Some(g) = grabacion.as_mut()
            && !g.ocupado
        {
            // Si habia una vuelta en marcha, se reintenta hasta que acabe.
            g.ocupado = pixpin_sincro::protocolo::ocupar(&disco);
        }
        let (Some(grupo), Some(emisor)) = (grupo.as_ref(), emisor.as_ref()) else {
            continue;
        };
        let ahora = super::ahora_ms();
        let paquete = |pregunta: bool, secuencia: u32| {
            let mut banderas = 0;
            if pregunta {
                banderas |= b::PREGUNTA;
            }
            let (grabacion, ms_actual) = match &local {
                Some(l) => {
                    banderas |= b::GRABANDO;
                    if l.en_pausa {
                        banderas |= b::EN_PAUSA;
                    }
                    if l.camara {
                        banderas |= b::CAMARA;
                    }
                    (l.id.clone(), l.ms)
                }
                None => (String::new(), 0),
            };
            Baliza {
                etiqueta: b::etiqueta_cruda(&grupo.clave),
                id: grupo.yo.clone(),
                puerto: puerto_tcp,
                reloj: ahora,
                secuencia,
                banderas,
                grabacion,
                ms_actual,
            }
            .escribir(&grupo.clave)
        };
        // A quien pregunto, directa.
        let a_quien: Vec<Ipv4Addr> = contestar.try_iter().collect();
        if !a_quien.is_empty() {
            secuencia = secuencia.wrapping_add(1);
            emisor.a(&paquete(false, secuencia), &a_quien);
        }
        if ritmo.toca(ahora, graba, cambio || preguntar) {
            secuencia = secuencia.wrapping_add(1);
            let p = paquete(preguntar, secuencia);
            preguntar = false;
            emisor.difundir(&p);
            emisor.a(&p, &conocidas(raiz, grupo));
        }
        let hace = oido_hace();
        if ritmo.toca_barrer(ahora, graba, hace) {
            let conocidas = conocidas(raiz, grupo);
            secuencia = secuencia.wrapping_add(1);
            let p = paquete(true, secuencia);
            let mut n = 0;
            for red in emisor.redes() {
                n += emisor.a(&p, &b::barrido(&red, &conocidas));
            }
            tracing::info!(n, "baliza: grabando sin oir a nadie, se barre la red");
        }
        if let Some(g) = grabacion.as_mut() {
            g.oido |= hace.is_some_and(|h| h < g.desde.elapsed().as_millis() as i64)
                || contesto.is_some_and(|c| c >= g.desde);
            if !g.diagnosticada {
                let d = b::diagnosticar(
                    g.desde.elapsed().as_millis() as i64,
                    g.oido,
                    &emisor.redes(),
                    &conocidas(raiz, grupo),
                );
                if d != Diagnostico::Bien {
                    g.diagnosticada = true;
                    decir(d);
                }
            }
        }
    }
}

/// Se le dice al usuario, una vez por grabacion, que no aparece el movil.
fn decir(d: Diagnostico) {
    let clave = match d {
        Diagnostico::Bien => return,
        Diagnostico::OtraRed => "baliza-movil-otra-red",
        Diagnostico::Aislados => "baliza-movil-aislado",
    };
    let texto = Catalogo::nuevo(crate::salida::idioma_actual()).t(clave);
    tracing::warn!(
        ?d,
        "baliza: grabando y sin ningun aparato del grupo a la vista"
    );
    presencia::difundir(Novedad::Registro(texto.clone()));
    super::al_movil::avisar(texto);
}

/// El oyente se vuelve a abrir (redes nuevas).
static OYENTE_DE_NUEVO: AtomicBool = AtomicBool::new(false);

fn oir(raiz: &Path, contestar: mpsc::Sender<Ipv4Addr>) {
    let mut oidas = Oidas::default();
    let mut buf = [0u8; b::TOPE];
    // A quien ya se contesto hace poco: una pregunta llega por tres caminos.
    let mut contestados: HashMap<Ipv4Addr, Instant> = HashMap::new();
    // Quien ya tiene el anuncio de esta grabacion, en que direccion.
    let mut avisados: HashMap<String, IpAddr> = HashMap::new();
    let mut oyente: Option<Oyente> = None;
    let mut fallo: Option<Instant> = None;
    loop {
        if OYENTE_DE_NUEVO.swap(false, Ordering::SeqCst) {
            oyente = None;
        }
        if oyente.is_none() {
            if fallo.is_some_and(|f| f.elapsed() < Duration::from_secs(30)) {
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }
            match Oyente::abrir(b::PUERTO_BALIZA, &redes()) {
                Ok(o) => {
                    oyente = Some(o);
                    fallo = None;
                }
                Err(e) => {
                    // Otro programa en el puerto (otro PixPin abierto): sin
                    // oir, se sigue mandando y quedan mDNS y la sonda.
                    tracing::warn!(?e, puerto = b::PUERTO_BALIZA, "baliza: no se puede oir");
                    fallo = Some(Instant::now());
                    continue;
                }
            }
        }
        let Some(o) = oyente.as_ref() else {
            continue;
        };
        let (n, de) = match o.oir(&mut buf) {
            Ok(Some(x)) => x,
            Ok(None) => continue,
            Err(e) => {
                tracing::info!(?e, "baliza: el oyente fallo, se vuelve a abrir");
                oyente = None;
                fallo = Some(Instant::now());
                continue;
            }
        };
        let Some(g) = GRUPO.lock().ok().and_then(|g| g.clone()) else {
            continue;
        };
        let Ok(baliza) = Baliza::leer(&buf[..n], &g.clave) else {
            continue;
        };
        if baliza.id == g.yo || !oidas.admitir(&baliza, super::ahora_ms()) {
            continue;
        }
        let IpAddr::V4(ip) = de.ip() else {
            continue;
        };
        if baliza.pregunta()
            && contestados
                .get(&ip)
                .is_none_or(|t| t.elapsed().as_secs() >= 1)
        {
            contestados.insert(ip, Instant::now());
            let _ = contestar.send(ip);
        }
        // Firmada con la clave del grupo pero de alguien que aun no esta en
        // la lista (se acaba de unir en otro aparato): que lo vea la sonda.
        if !g.miembros.contains(&baliza.id) {
            presencia::sondear_ya();
            continue;
        }
        oido(raiz, &baliza, ip, &mut avisados);
    }
}

/// Lo que se hace con la baliza de uno del grupo.
fn oido(raiz: &Path, baliza: &Baliza, ip: Ipv4Addr, avisados: &mut HashMap<String, IpAddr>) {
    let host = ip.to_string();
    let puerto = if baliza.puerto == 0 {
        pixpin_sincro::PUERTO
    } else {
        baliza.puerto
    };
    let antes = OIDOS.lock().ok().and_then(|mut o| {
        o.get_or_insert_with(HashMap::new).insert(
            baliza.id.clone(),
            Oido {
                cuando: Instant::now(),
                baliza: baliza.clone(),
            },
        )
    });
    // Su direccion de verdad, aunque el DHCP le haya dado otra.
    let ya = super::leer_direcciones(raiz)
        .into_iter()
        .any(|(i, h, p)| i == baliza.id && h == host && p == puerto);
    if !ya {
        tracing::info!(id = %baliza.id, %host, puerto, "baliza: direccion nueva de un aparato del grupo");
        super::apuntar_direccion(raiz, &baliza.id, &host, puerto);
        presencia::difundir(Novedad::Identidad);
    }
    // Para la automatica: esta, sin esperar a la sonda. Una vez por
    // aparicion (o si cambio de sitio), no a cada latido.
    let reaparece = antes
        .as_ref()
        .is_none_or(|a| a.cuando.elapsed() > Duration::from_secs(30));
    if reaparece || !ya {
        presencia::difundir(Novedad::Responde(baliza.id.clone(), true));
    }
    let empezo_a_grabar = baliza.graba() && antes.as_ref().is_none_or(|a| !a.baliza.graba());
    if empezo_a_grabar {
        tracing::info!(id = %baliza.id, grabacion = %baliza.grabacion, "baliza: otro aparato del grupo graba");
        presencia::difundir(Novedad::Registro(format!(
            "Grabando en otro aparato del grupo ({})",
            baliza.grabacion
        )));
    }
    // **Este equipo graba**: quien aparece (o cambia de sitio) recibe el
    // anuncio de la camara al momento, sin esperar a que pregunte.
    if grabando_aqui() {
        if avisados.get(&baliza.id) != Some(&IpAddr::V4(ip)) {
            avisados.insert(baliza.id.clone(), IpAddr::V4(ip));
            super::camara::avisar_ya(raiz);
        }
    } else {
        avisados.clear();
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn sin_oir_a_nadie_no_graba_nadie() {
        // Caso negativo: sin balizas, el modo prioritario depende solo de
        // este equipo.
        if let Ok(mut o) = OIDOS.lock() {
            *o = None;
        }
        assert!(otros_grabando().is_empty());
        assert_eq!(grabando_alguien(), grabando_aqui());
    }

    #[test]
    fn una_baliza_de_grabando_caduca_sola() {
        let viejo = Instant::now()
            .checked_sub(Duration::from_millis(b::VIGENCIA_GRABANDO_MS as u64 + 500))
            .unwrap();
        let grabando = Baliza {
            id: "tel".into(),
            banderas: b::GRABANDO,
            ..Default::default()
        };
        if let Ok(mut o) = OIDOS.lock() {
            let m = o.get_or_insert_with(HashMap::new);
            m.insert(
                "tel".into(),
                Oido {
                    cuando: Instant::now(),
                    baliza: grabando.clone(),
                },
            );
        }
        assert_eq!(otros_grabando(), ["tel"]);
        if let Ok(mut o) = OIDOS.lock() {
            o.get_or_insert_with(HashMap::new).insert(
                "tel".into(),
                Oido {
                    cuando: viejo,
                    baliza: grabando,
                },
            );
        }
        // El movil se apago grabando: a los cuatro segundos ya no cuenta.
        assert!(otros_grabando().is_empty());
        if let Ok(mut o) = OIDOS.lock() {
            *o = None;
        }
    }
}
