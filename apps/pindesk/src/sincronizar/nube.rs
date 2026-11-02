//! **La nube de los chats compartidos, en el PC** (1-nov-2026).
//!
//! Lo de dentro —el protocolo, los deltas de trazos, juntar, «Sin
//! conexión»— esta en `pindesk_sincro::nube`, sin ventanas y probado alli
//! (tambien contra la nube de referencia). Aqui:
//!
//! - **Los ajustes**: la direccion de la web (`sincro/nube/ajustes.json`,
//!   que no viaja) y la llave `pxn_…` del aparato, cifrada con DPAPI en
//!   `%LOCALAPPDATA%\PinDesk\claves\nube.clave` como las de la IA.
//! - **El hilo de la nube**, que vive lo que la aplicacion: mira cada pocos
//!   segundos si cambio algo en la carpeta de un chat de la nube y, cuando
//!   lleva unos segundos quieto, sube (solo lo que cambio); otro hilo espera
//!   las novedades con la conexion abierta (`novedades?espera=25`) y baja en
//!   cuanto otro sube. Sin red, espera creciente (`reintentos`).
//! - **Lo que ve la ventana**: el estado de cada chat (subiendo, al dia,
//!   sin red) para la chapa de la lista, y el modo (Local o Nube) de cada
//!   chat compartido.
//! - **«En línea / Sin conexión»** de los visores ([`documento`]).
//!
//! Los chats propios siguen solo por la red local.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use pindesk_proyecto::vista::DiscoPc;
use pindesk_sincro::compartidos::CompartidosEnDisco;
use pindesk_sincro::nube::cliente::{Contestado, FalloNube, Nube, Transporte};
use pindesk_sincro::nube::estado::{self, ChatsEnLaNube, EstadoDeChat, Modo};
use pindesk_sincro::nube::sin_conexion::SinConexion;
use pindesk_sincro::nube::vuelta::{self, Opciones};
use pindesk_sincro::reintentos;
use serde::{Deserialize, Serialize};

/// El fichero de la llave, junto a las de la IA.
const FICHERO_LLAVE: &str = "nube.clave";
/// Cada cuanto se mira si cambio algo en un chat de la nube.
const MIRAR_CADA: Duration = Duration::from_secs(3);
/// Cuanto tiene que llevar quieto un chat para subirlo (juntar cambios).
const QUIETO: Duration = Duration::from_secs(4);
/// Cada cuanto se baja aunque no llegue ninguna novedad.
const BAJAR_CADA: Duration = Duration::from_secs(10 * 60);

// ------------------------------------------------------------------ ajustes

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Ajustes {
    direccion: String,
}

fn sincro(raiz: &Path) -> PathBuf {
    raiz.join("sincro")
}

fn ruta_ajustes(raiz: &Path) -> PathBuf {
    estado::carpeta(&sincro(raiz)).join("ajustes.json")
}

fn leer_ajustes(raiz: &Path) -> Ajustes {
    std::fs::read_to_string(ruta_ajustes(raiz))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// La direccion de la web de la nube (sin barra final), o vacia.
pub(crate) fn direccion(raiz: &Path) -> String {
    leer_ajustes(raiz).direccion
}

/// Si una direccion se puede usar: `https://`, o `http://` hacia este
/// equipo (una nube de pruebas).
pub(crate) fn direccion_valida(d: &str) -> bool {
    pindesk_ia::nube::http::partir(d.trim()).is_some()
}

fn llave() -> Option<String> {
    let f = pindesk_ia::nube::claves::carpeta()?.join(FICHERO_LLAVE);
    let claro = pindesk_ia::nube::claves::descifrar(&std::fs::read(f).ok()?).ok()?;
    String::from_utf8(claro)
        .ok()
        .filter(|l| !l.trim().is_empty())
}

/// La llave enmascarada, para ensenarla (`pxn…a1b2`).
pub(crate) fn llave_a_la_vista() -> Option<String> {
    llave().map(|l| pindesk_ia::nube::claves::enmascarar(&l))
}

/// **Guarda los ajustes**: la direccion y, si viene, la llave (cifrada).
pub(crate) fn guardar(raiz: &Path, direccion: &str, llave: Option<&str>) -> Result<(), String> {
    let direccion = direccion.trim().trim_end_matches('/').to_string();
    if !direccion.is_empty() && !direccion_valida(&direccion) {
        return Err("direccion".into());
    }
    if let Some(l) = llave.map(str::trim).filter(|l| !l.is_empty()) {
        if !pindesk_sincro::nube::llave_valida(l) {
            return Err("llave".into());
        }
        let carpeta = pindesk_ia::nube::claves::carpeta().ok_or("LOCALAPPDATA")?;
        std::fs::create_dir_all(&carpeta).map_err(|e| e.to_string())?;
        let cifrado = pindesk_ia::nube::claves::cifrar(l.as_bytes())?;
        pindesk_sincro::disco::escribir_atomico(&carpeta.join(FICHERO_LLAVE), &cifrado)
            .map_err(|e| e.to_string())?;
    }
    let t = serde_json::to_vec_pretty(&Ajustes { direccion }).map_err(|e| e.to_string())?;
    pindesk_sincro::disco::escribir_atomico(&ruta_ajustes(raiz), &t).map_err(|e| e.to_string())?;
    despertar();
    Ok(())
}

/// Olvida la llave de este aparato.
pub(crate) fn olvidar_llave() {
    if let Some(c) = pindesk_ia::nube::claves::carpeta() {
        let _ = std::fs::remove_file(c.join(FICHERO_LLAVE));
    }
    despertar();
}

fn conexion(raiz: &Path) -> Option<(String, String)> {
    let d = direccion(raiz);
    if d.is_empty() {
        return None;
    }
    Some((d, llave()?))
}

/// Si hay direccion y llave.
pub(crate) fn configurada(raiz: &Path) -> bool {
    conexion(raiz).is_some()
}

// ---------------------------------------------------------------- transporte

/// **WinHTTP**, el de la IA (`pindesk_ia::nube::http::pedir`): el TLS y el
/// proxy del sistema.
struct WinHttp;

/// Lo mas que se baja de una vez (los topes de la nube: 512 MB un objeto).
const LIMITE: usize = 600 * 1024 * 1024;

impl Transporte for WinHttp {
    fn enviar(
        &self,
        metodo: &str,
        url: &str,
        cabeceras: &[(&'static str, String)],
        cuerpo: &[u8],
        _tiempo: Duration,
    ) -> Result<Contestado, String> {
        use pindesk_ia::nube::http::{Fallo, pedir};
        match pedir(metodo, url, cabeceras, cuerpo, LIMITE) {
            Ok(r) => Ok(Contestado {
                codigo: r.codigo as u16,
                cuerpo: r.cuerpo,
            }),
            Err(Fallo::Http { codigo, cuerpo, .. }) => Ok(Contestado {
                codigo: codigo as u16,
                cuerpo: cuerpo.into_bytes(),
            }),
            Err(Fallo::Red(m)) => Err(m),
        }
    }
}

// ------------------------------------------------------------------ estado

/// Como va un chat de la nube, para la chapa de la lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EstadoNube {
    Subiendo,
    AlDia,
    SinRed,
    /// La llave no vale o no deja: hay que mirar los ajustes.
    Llave,
}

#[derive(Default)]
struct Compartido {
    /// Por chat.
    estados: HashMap<String, EstadoNube>,
    /// Chats con una novedad que bajar.
    pendientes: BTreeSet<String>,
    /// Algo pidio una vuelta ya (ajustes, modo, volver a en linea).
    despierto: bool,
    /// La ultima prueba de la conexion (para Ajustes).
    ultimo: Option<Result<String, String>>,
}

fn compartido() -> &'static (Mutex<Compartido>, Condvar) {
    static C: OnceLock<(Mutex<Compartido>, Condvar)> = OnceLock::new();
    C.get_or_init(|| (Mutex::new(Compartido::default()), Condvar::new()))
}

fn con<T>(f: impl FnOnce(&mut Compartido) -> T) -> T {
    let mut g = compartido().0.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut g)
}

/// Que el hilo de la nube de una vuelta ya.
pub(crate) fn despertar() {
    con(|c| c.despierto = true);
    compartido().1.notify_all();
}

fn poner_estado(chat: &str, e: EstadoNube) {
    let cambio = con(|c| c.estados.insert(chat.to_string(), e) != Some(e));
    if cambio {
        crate::ventana_chat::refrescar();
    }
}

/// El estado del chat de la nube de esta ficha, o `None` si no va por la nube.
pub(crate) fn estado_de_ficha(raiz: &Path, ficha: &str) -> Option<EstadoNube> {
    let chat = pindesk_proyecto::vista::chat_de_ficha(raiz, ficha)?;
    if modo(raiz, &chat) != Modo::Nube {
        return None;
    }
    Some(
        con(|c| c.estados.get(&chat).copied()).unwrap_or(if configurada(raiz) {
            EstadoNube::Subiendo
        } else {
            EstadoNube::SinRed
        }),
    )
}

/// Lo que dijo la ultima vez la nube al probarla (Ajustes).
pub(crate) fn ultimo_saludo() -> Option<Result<String, String>> {
    con(|c| c.ultimo.clone())
}

// -------------------------------------------------------------------- modos

/// Por donde va un chat compartido.
pub(crate) fn modo(raiz: &Path, chat: &str) -> Modo {
    if raiz.as_os_str().is_empty() {
        return Modo::Local;
    }
    ChatsEnLaNube::leer(&sincro(raiz)).modo(chat)
}

/// Cambia por donde va un chat compartido.
pub(crate) fn poner_modo(raiz: &Path, chat: &str, m: Modo) -> std::io::Result<()> {
    estado::cambiar_chats(&sincro(raiz), |c| c.poner(chat, m))?;
    if m == Modo::Local {
        con(|c| c.estados.remove(chat));
    }
    crate::ventana_chat::refrescar();
    despertar();
    Ok(())
}

// --------------------------------------------------------------- el hilo

/// La firma de la carpeta de un chat: cuantos ficheros, el mayor tamano y
/// la ultima fecha. Barata: no lee nada, solo mira.
fn firma_de(dir: &Path) -> (u64, u64, u128) {
    let mut f = (0u64, 0u64, 0u128);
    let mut pila = vec![dir.to_path_buf()];
    while let Some(d) = pila.pop() {
        let Ok(l) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in l.flatten() {
            let Ok(m) = e.metadata() else { continue };
            if m.is_dir() {
                pila.push(e.path());
            } else {
                f.0 += 1;
                f.1 = f.1.wrapping_add(m.len());
                let t = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_millis());
                f.2 = f.2.max(t);
            }
        }
    }
    f
}

fn carpeta_del_chat(d: &DiscoPc, raiz: &Path, chat: &str) -> Option<PathBuf> {
    d.ficha_de(chat)
        .map(|f| pindesk_proyecto::almacen::carpeta(raiz, &f.id))
}

fn disco(raiz: &Path) -> DiscoPc {
    DiscoPc::nuevo(raiz).con_avisos(|c| {
        if c != pindesk_sincro::disco::Cambio::Identidad {
            crate::ventana_chat::refrescar();
            crate::tareas::refrescar();
        }
    })
}

/// **Arranca la nube** (al arrancar la aplicacion, con la presencia).
pub(crate) fn instalar(raiz: PathBuf) {
    static UNA: OnceLock<()> = OnceLock::new();
    if UNA.set(()).is_err() {
        return;
    }
    let r = raiz.clone();
    let _ = std::thread::Builder::new()
        .name("nube".into())
        .spawn(move || bucle(r));
    let _ = std::thread::Builder::new()
        .name("nube-novedades".into())
        .spawn(move || novedades(raiz));
}

/// Presenta la persona de este aparato si la nube aun no la conoce.
fn presentarse(raiz: &Path, n: &Nube<'_>, h: &pindesk_sincro::nube::tipos::Hola) {
    let d = DiscoPc::nuevo(raiz);
    let Ok(yo) = CompartidosEnDisco::nuevo(raiz).yo_de(&d) else {
        return;
    };
    if h.aparato.persona.as_deref() == Some(yo.id.as_str()) {
        return;
    }
    let firma = pindesk_sincro::nube::firma_de_yo(&yo.llaves, &h.aparato.id, &yo.id);
    if let Err(e) = n.presentar(&yo.id, &yo.clave(), &firma) {
        tracing::warn!(%e, "la nube no acepto la persona de este aparato");
    }
}

/// Los compartidos que otros pusieron en la nube y donde estoy: entran solos.
fn adoptar(raiz: &Path, n: &Nube<'_>) -> Result<(), FalloNube> {
    let s = sincro(raiz);
    for c in n.chats()? {
        if c.compartido != Some(true) {
            continue;
        }
        vuelta::conocer_espacio(&s, &c.chat, &c.espacio)?;
        let mut nuevo = false;
        estado::cambiar_chats(&s, |l| nuevo = l.adoptar(&c.chat))?;
        if nuevo {
            con(|x| x.pendientes.insert(c.chat.clone()));
        }
    }
    Ok(())
}

/// Prueba la conexion (el boton «Probar» de Ajustes): `Ok` con el espacio y
/// la persona, o el error para ensenarlo.
pub(crate) fn probar(raiz: &Path) -> Result<(String, Option<String>), String> {
    let (direccion, llave) = conexion(raiz).ok_or_else(|| "sin-configurar".to_string())?;
    let t = WinHttp;
    let n = Nube {
        direccion,
        llave,
        t: &t,
    };
    let r = n.hola();
    let dicho = r
        .as_ref()
        .map(|h| h.aparato.espacio.clone())
        .map_err(|e| e.to_string());
    con(|c| c.ultimo = Some(dicho));
    match r {
        Ok(h) => {
            presentarse(raiz, &n, &h);
            despertar();
            Ok((h.aparato.espacio, h.aparato.persona))
        }
        Err(e) => Err(e.to_string()),
    }
}

fn bucle(raiz: PathBuf) {
    let t = WinHttp;
    let mut saludo: Option<(String, bool)> = None;
    let mut firmas: HashMap<String, ((u64, u64, u128), Instant)> = HashMap::new();
    let mut subidas: HashMap<String, (u64, u64, u128)> = HashMap::new();
    let mut bajado = Instant::now();
    let mut fallos = 0u32;
    loop {
        let espera = if fallos > 0 {
            Duration::from_millis(reintentos::espera_tras_fallo(
                fallos,
                reintentos::azar_del_sistema(),
            ) as u64)
        } else {
            MIRAR_CADA
        };
        {
            let (m, cv) = compartido();
            let g = m.lock().unwrap_or_else(|e| e.into_inner());
            let (mut g, _) = cv
                .wait_timeout_while(g, espera, |c| !c.despierto && c.pendientes.is_empty())
                .unwrap_or_else(|e| e.into_inner());
            if g.despierto {
                g.despierto = false;
                fallos = 0;
            }
        }
        let Some((direccion, llave)) = conexion(&raiz) else {
            saludo = None;
            continue;
        };
        let chats = ChatsEnLaNube::leer(&sincro(&raiz)).chats;
        let n = Nube {
            direccion: direccion.clone(),
            llave,
            t: &t,
        };
        if saludo.as_ref().is_none_or(|(d, _)| *d != direccion) {
            match n.hola() {
                Ok(h) => {
                    presentarse(&raiz, &n, &h);
                    if let Err(e) = adoptar(&raiz, &n) {
                        tracing::info!(%e, "nube: no se pudieron mirar los chats compartidos");
                    }
                    saludo = Some((direccion.clone(), h.sabe(pindesk_sincro::nube::DELTAS)));
                }
                Err(e) => {
                    fallos += 1;
                    let est = if matches!(e, FalloNube::Llave(_)) {
                        EstadoNube::Llave
                    } else {
                        EstadoNube::SinRed
                    };
                    for c in &chats {
                        poner_estado(c, est);
                    }
                    continue;
                }
            }
        }
        let deltas = saludo.as_ref().is_some_and(|s| s.1);
        let d = disco(&raiz);
        let sc = SinConexion::nuevo(&sincro(&raiz));
        let toca_bajar = bajado.elapsed() > BAJAR_CADA;
        if toca_bajar {
            bajado = Instant::now();
        }
        let mut fallo = false;
        for chat in &chats {
            let pendiente = con(|c| c.pendientes.remove(chat));
            let Some(dir) = carpeta_del_chat(&d, &raiz, chat) else {
                // Un compartido que aun no esta aqui: se baja entero.
                if pendiente || EstadoDeChat::leer(&sincro(&raiz), chat).revision == 0 {
                    fallo |= una_vuelta(&d, &n, chat, deltas, &sc, true);
                }
                continue;
            };
            let f = firma_de(&dir);
            let quieto = match firmas.get(chat) {
                Some((g, desde)) if *g == f => desde.elapsed() >= QUIETO,
                _ => {
                    firmas.insert(chat.clone(), (f, Instant::now()));
                    false
                }
            };
            let cambio = subidas.get(chat) != Some(&f) && quieto;
            if pendiente || toca_bajar || cambio {
                fallo |= una_vuelta(&d, &n, chat, deltas, &sc, pendiente || toca_bajar);
                if !fallo {
                    // Lo que acaba de escribir la vuelta tambien es «visto».
                    let f = firma_de(&dir);
                    subidas.insert(chat.clone(), f);
                    firmas.insert(chat.clone(), (f, Instant::now()));
                }
            }
        }
        fallos = if fallo { fallos + 1 } else { 0 };
    }
}

/// Una vuelta de un chat; dice si fallo por la red.
fn una_vuelta(
    d: &DiscoPc,
    n: &Nube<'_>,
    chat: &str,
    deltas: bool,
    sc: &SinConexion,
    bajar: bool,
) -> bool {
    poner_estado(chat, EstadoNube::Subiendo);
    let op = Opciones {
        deltas,
        ahora: super::ahora_ms(),
        retencion: sc,
        bajar,
    };
    match vuelta::vuelta_de_chat(d, n, chat, &op) {
        Ok(i) => {
            if i.subido || i.bajados > 0 {
                tracing::info!(
                    chat,
                    subido = i.subido,
                    bajados = i.bajados,
                    objetos = i.objetos,
                    bytes = i.bytes_de_objetos + i.bytes_de_parches,
                    "nube: vuelta"
                );
            }
            poner_estado(chat, EstadoNube::AlDia);
            false
        }
        Err(e) => {
            tracing::info!(chat, %e, "nube: la vuelta no salio");
            poner_estado(
                chat,
                if matches!(e, FalloNube::Llave(_)) {
                    EstadoNube::Llave
                } else {
                    EstadoNube::SinRed
                },
            );
            e.pasajero()
        }
    }
}

/// **Las novedades, con la conexion abierta**: en cuanto otro sube un chat
/// de la nube, se apunta y se despierta al hilo de la nube.
fn novedades(raiz: PathBuf) {
    let t = WinHttp;
    let mut fallos = 0u32;
    loop {
        if fallos > 0 {
            std::thread::sleep(Duration::from_millis(reintentos::espera_tras_fallo(
                fallos,
                reintentos::azar_del_sistema(),
            ) as u64));
        }
        let Some((direccion, llave)) = conexion(&raiz) else {
            std::thread::sleep(Duration::from_secs(10));
            continue;
        };
        let n = Nube {
            direccion,
            llave,
            t: &t,
        };
        let s = sincro(&raiz);
        let desde = ChatsEnLaNube::leer(&s).seq;
        match n.novedades(desde, 25) {
            Ok(nv) => {
                fallos = 0;
                let chats = ChatsEnLaNube::leer(&s);
                let mut hay = nv.reiniciar;
                for c in &nv.cambios {
                    if c.borrado == Some(true) {
                        continue;
                    }
                    if c.compartido == Some(true) && !chats.locales.contains(&c.chat) {
                        let _ = vuelta::conocer_espacio(&s, &c.chat, &c.espacio);
                        let _ = estado::cambiar_chats(&s, |l| {
                            l.adoptar(&c.chat);
                        });
                    }
                    if ChatsEnLaNube::leer(&s).modo(&c.chat) == Modo::Nube
                        && EstadoDeChat::leer(&s, &c.chat).revision < c.revision
                    {
                        con(|x| x.pendientes.insert(c.chat.clone()));
                        hay = true;
                    }
                }
                let _ = estado::cambiar_chats(&s, |l| l.seq = nv.seq);
                if nv.reiniciar {
                    let _ = adoptar(&raiz, &n);
                    con(|x| x.pendientes.extend(chats.chats.iter().cloned()));
                }
                if hay {
                    compartido().1.notify_all();
                }
            }
            Err(_) => fallos += 1,
        }
    }
}

// ------------------------------------------------- «En línea / Sin conexión»

/// Lo anotado de un visor en un chat de la nube: la carpeta de datos, el
/// chat y la clave de lo anotado (el codigo del adjunto, `anot-<uid>…`, o la
/// referencia de un lienzo, `pins/draw/<referencia>.excalidraw.gz`).
pub(crate) type Sitio = (PathBuf, String, String);

/// El documento de un visor, si es un adjunto de un chat de la nube.
pub(crate) fn documento(doc: &Path) -> Option<Sitio> {
    let raiz = pindesk_proyecto::anotado::raiz_de(doc)?;
    let x = pindesk_proyecto::anotado::adjunto_de(&raiz, doc)?;
    (modo(&raiz, &x.chat) == Modo::Nube).then_some((raiz, x.chat, x.uid))
}

/// El lienzo `referencia` de la ficha `ficha` (una hoja DIBUJO o PAGINA, o
/// el lienzo de una foto), si su chat va por la nube.
pub(crate) fn lienzo(raiz: &Path, ficha: &str, referencia: &str) -> Option<Sitio> {
    if referencia.is_empty() || raiz.as_os_str().is_empty() {
        return None;
    }
    let chat = pindesk_proyecto::vista::chat_de_ficha(raiz, ficha)?;
    (modo(raiz, &chat) == Modo::Nube).then(|| (raiz.to_path_buf(), chat, referencia.to_string()))
}

/// Si lo anotado de `sitio` esta «Sin conexión».
pub(crate) fn sin_conexion((raiz, chat, clave): &Sitio) -> bool {
    SinConexion::nuevo(&sincro(raiz)).esta(chat, clave)
}

/// **Pasa lo anotado a sin conexion o lo vuelve a en linea** (lo de aqui
/// sube en la vuelta siguiente, solo los trazos nuevos).
pub(crate) fn poner_sin_conexion((raiz, chat, clave): &Sitio, sin: bool) {
    let sc = SinConexion::nuevo(&sincro(raiz));
    let r = if sin {
        sc.retener(&DiscoPc::nuevo(raiz), chat, clave, super::ahora_ms())
    } else {
        sc.volver(chat, clave)
    };
    if let Err(e) = r {
        tracing::warn!(?e, "no se pudo cambiar «Sin conexión»");
    }
    despertar();
}

/// **Al salir de un documento sin conexion** (despues de que el visor
/// guarde lo suyo): se pregunta. «Sí» (lo de por omision, como pidio el
/// usuario) descarta lo anotado sin conexion; «No» vuelve a en linea y lo
/// sube; «Cancelar» lo deja sin conexion para la proxima vez que se abra.
pub(crate) fn al_cerrar((raiz, chat, clave): &Sitio, textos: &pindesk_store::Catalogo) {
    let sc = SinConexion::nuevo(&sincro(raiz));
    if !sc.esta(chat, clave) {
        return;
    }
    use pindesk_shell::dialogo::{Eleccion, elegir_al_frente};
    match elegir_al_frente(
        &textos.t("nube-cerrar-titulo"),
        &textos.t("nube-cerrar-pregunta"),
    ) {
        Eleccion::Si => {
            if let Err(e) = sc.descartar(&disco(raiz), chat, clave) {
                tracing::warn!(?e, "no se pudo descartar lo anotado sin conexion");
            }
        }
        Eleccion::No => {
            let _ = sc.volver(chat, clave);
            despertar();
        }
        Eleccion::Luego => {}
    }
}
