//! **El asistente sigue aunque se cierre el chat** (24-oct-2026). El
//! usuario: «cuando cierro la aplicación por accidente, el asistente como
//! que se olvida de la tarea … y al final no hace nada. A pesar de que
//! cierre la ventana, debería seguir trabajando y completando la tarea».
//!
//! **Por que se olvidaba.** Los turnos en marcha (`Sesion`) vivian en un
//! `thread_local` del hilo del chat, y al cerrar la ventana ese hilo acababa
//! y se los llevaba. El hilo del turno seguia, pero hablando solo: nadie
//! recogia lo que escribia (la nota se quedaba en el disco como «💭
//! Pensando…» o a medio escribir), nadie atendia lo que pedia meter en el
//! lienzo del chat (esperaba 60 s y «la ventana del chat no contesto»), y
//! lo que preguntaba caducaba en «no» al momento. Y si se cerraba PixPin
//! entera (Salir, o se caia), no quedaba nada apuntado en ningun sitio.
//!
//! **Ahora**:
//!
//! - Al cerrar la ventana, sus turnos pasan a **un hilo de fondo**
//!   (`asistente-de-fondo`) que hace lo mismo que el bucle del chat sin
//!   conversacion abierta (`latido(…, None)`, el camino de las
//!   automatizaciones): escribe la respuesta en el cuaderno del proyecto,
//!   atiende el lienzo del chat en su fichero y deja caducar las preguntas
//!   a su hora (5 min), no al cerrar. Una tarjeta flotante dice que sigue,
//!   avisa si pregunta algo y, al acabar, ensena la respuesta con «Abrir».
//!   Al reabrir el chat, los turnos vuelven a su hilo y la burbuja sigue en
//!   vivo donde iba.
//! - Cada turno se **apunta en el disco** al empezar ([`FICHERO`]) y se
//!   borra al acabar. Lo que siga apuntado al arrancar se corto a medias:
//!   si aun no habia hecho nada con sus herramientas, **se vuelve a lanzar
//!   solo**; si ya habia hecho algo, no se repite (apuntaria o dibujaria
//!   dos veces: la misma regla que la cola de encargos,
//!   `automatizaciones::cola`) y la burbuja cortada lo dice con claridad,
//!   con como seguir. Un pedido que ya se retomo una vez no se retoma otra
//!   (un pedido que tumba PixPin no la tumba en cada arranque).
//!
//! Lo que no es de la ventana (la descarga del modelo, la tarjeta de
//! bajarlo) se queda como estaba.

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use pixpin_proyecto::cuaderno;
use serde::{Deserialize, Serialize};

use super::{SESIONES, Sesion};
use crate::asistente::chat as ia_chat;
use crate::ventana_chat::sin_ventana::{Abrir, Aviso, Chat, Mando, avisar};

/// El fichero, en la raiz de los datos (uno por equipo, no viaja con la
/// sincro: lo que esta a medias es de este PC).
pub(crate) const FICHERO: &str = "asistente-en-marcha.json";
const VERSION: u32 = 1;
/// Mas viejo que esto, no se retoma solo: el usuario ya no lo espera.
pub(crate) const RETOMAR_HASTA_MS: i64 = 24 * 60 * 60 * 1000;
/// Cada cuanto mira el hilo de fondo (lo mismo que el chat con un turno en
/// marcha, `asistente::espera`).
const VUELTA: Duration = Duration::from_millis(80);

/// **Un turno en marcha, tal como se apunta.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EnMarcha {
    pub proyecto: String,
    /// La nota de la respuesta.
    pub nota: String,
    /// La nota de la pregunta (de ahi sale el pedido al retomarlo).
    pub pregunta: String,
    /// Si es el chat «Asistente» (todo lo escrito es para el) o otro, donde
    /// hay que nombrarlo con `@asistente`.
    #[serde(default)]
    pub asistente: bool,
    /// Cuando empezo (ms UTC).
    pub desde: i64,
    /// Alguna herramienta ya hizo algo.
    #[serde(default)]
    pub actuo: bool,
    /// Las veces que ya se retomo.
    #[serde(default)]
    pub intentos: u32,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Apuntados {
    version: u32,
    #[serde(default)]
    turnos: Vec<EnMarcha>,
}

/// El fichero lo tocan el hilo del chat, el de fondo y el de retomar.
static CANDADO: Mutex<()> = Mutex::new(());

fn leer(raiz: &Path) -> Vec<EnMarcha> {
    match std::fs::read_to_string(raiz.join(FICHERO)) {
        Ok(t) => match serde_json::from_str::<Apuntados>(&t) {
            Ok(a) => a.turnos,
            Err(e) => {
                tracing::warn!(%e, "asistente: el fichero de turnos en marcha esta roto, se ignora");
                Vec::new()
            }
        },
        Err(_) => Vec::new(),
    }
}

/// Lo escribe de una vez (al lado y renombrando); sin nada, lo borra.
fn escribir(raiz: &Path, turnos: &[EnMarcha]) -> std::io::Result<()> {
    let ruta = raiz.join(FICHERO);
    if turnos.is_empty() {
        return match std::fs::remove_file(&ruta) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    let a = Apuntados {
        version: VERSION,
        turnos: turnos.to_vec(),
    };
    let texto = serde_json::to_string_pretty(&a).map_err(std::io::Error::other)?;
    let parte = ruta.with_extension("parte");
    std::fs::write(&parte, texto)?;
    std::fs::rename(&parte, &ruta)
}

fn cambiar(raiz: &Path, f: impl FnOnce(&mut Vec<EnMarcha>)) {
    let _g = CANDADO.lock().unwrap_or_else(|e| e.into_inner());
    let mut v = leer(raiz);
    f(&mut v);
    if let Err(e) = escribir(raiz, &v) {
        tracing::warn!(?e, "asistente: no se pudo apuntar el turno en marcha");
    }
}

/// Apunta un turno que empieza (uno anterior de la misma nota se cambia).
pub(crate) fn apuntar(raiz: &Path, e: EnMarcha) {
    cambiar(raiz, |v| {
        v.retain(|x| x.nota != e.nota);
        v.push(e);
    });
}

/// El turno acabo (bien, mal o parado): ya no esta a medias.
pub(crate) fn quitar(raiz: &Path, nota: &str) {
    cambiar(raiz, |v| v.retain(|x| x.nota != nota));
}

/// Una herramienta del turno ya hizo algo.
pub(crate) fn marcar_que_actuo(raiz: &Path, nota: &str) {
    cambiar(raiz, |v| {
        v.iter_mut()
            .filter(|x| x.nota == nota)
            .for_each(|x| x.actuo = true);
    });
}

/// Lo apuntado, vaciando el fichero: lo coge quien lo va a retomar.
fn tomar_apuntados(raiz: &Path) -> Vec<EnMarcha> {
    let _g = CANDADO.lock().unwrap_or_else(|e| e.into_inner());
    let v = leer(raiz);
    if let Err(e) = escribir(raiz, &[]) {
        tracing::warn!(
            ?e,
            "asistente: no se pudo vaciar el fichero de turnos en marcha"
        );
    }
    v
}

// ---------------------------------------------------------------------------
// Que hacer con lo cortado (puro)

/// Por que no se retoma solo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Motivo {
    /// Ya hizo algo con sus herramientas: repetirlo lo haria dos veces.
    YaActuo,
    /// Ya se retomo una vez y volvio a cortarse.
    YaSeRetomo,
    /// Es de hace demasiado.
    Viejo,
    /// No se encuentra la pregunta (se borro).
    SinPregunta,
    /// No se pudo volver a lanzar (sin modelo, sin clave…): el aviso.
    NoSePudo(String),
}

/// Lo que se hace con un turno que quedo a medias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Retomar {
    /// Se vuelve a lanzar, desde el principio.
    OtraVez,
    /// Solo se dice en la burbuja (y como seguir).
    Avisar(Motivo),
}

/// **La regla**: se retoma solo lo que no hizo nada todavia, no se habia
/// retomado ya, es reciente y conserva su pregunta.
pub(crate) fn que_hacer(e: &EnMarcha, ahora: i64, hay_pregunta: bool) -> Retomar {
    if !hay_pregunta {
        Retomar::Avisar(Motivo::SinPregunta)
    } else if e.actuo {
        Retomar::Avisar(Motivo::YaActuo)
    } else if e.intentos > 0 {
        Retomar::Avisar(Motivo::YaSeRetomo)
    } else if ahora.saturating_sub(e.desde) > RETOMAR_HASTA_MS {
        Retomar::Avisar(Motivo::Viejo)
    } else {
        Retomar::OtraVez
    }
}

/// Lo que quedo escrito, sin lo que solo tiene sentido en vivo: el cursor,
/// el «💭 Pensando…» de antes de llegar nada y la linea de estado de una
/// herramienta («🎨 Dibujando…»), que ya no esta pasando.
pub(crate) fn sin_lo_vivo(texto: &str) -> String {
    let t = texto.trim_end();
    let t = t.strip_suffix('▍').unwrap_or(t).trim_end();
    let mut lineas: Vec<&str> = t.lines().collect();
    if let Some(ultima) = lineas.last().map(|l| l.trim()) {
        let estado = ultima == super::ESTADO_INICIAL
            || (ultima.ends_with('…')
                && ultima.chars().count() <= 60
                && ultima.chars().next().is_some_and(|c| !c.is_alphanumeric()));
        if estado {
            lineas.pop();
        }
    }
    lineas.join("\n").trim().to_string()
}

/// **El texto de la burbuja cortada**: lo que ya habia escrito y, debajo,
/// lo que paso y como seguir.
pub(crate) fn texto_cortado(parcial: &str, r: &Retomar, asistente: bool) -> String {
    let sigue = if asistente {
        "«continúa»"
    } else {
        "«@asistente continúa»"
    };
    let aviso = match r {
        Retomar::OtraVez => "↻ PixPin se cerró antes de que acabara. Lo retomo desde el principio en la respuesta de abajo.".to_string(),
        Retomar::Avisar(m) => {
            let porque = match m {
                Motivo::YaActuo => format!(
                    "Algunos pasos ya estaban hechos, así que no lo repito solo (se harían dos veces). Escribe {sigue} y sigo desde aquí."
                ),
                Motivo::YaSeRetomo => {
                    format!("Ya lo había retomado una vez y volvió a cortarse, así que no insisto. Escribe {sigue} si quieres que lo intente otra vez.")
                }
                Motivo::Viejo => format!("Ha pasado demasiado para retomarlo solo. Escribe {sigue} si aún lo quieres."),
                Motivo::SinPregunta => "No encuentro tu pedido (¿se borró?), así que no puedo retomarlo.".to_string(),
                Motivo::NoSePudo(aviso) => format!("No he podido retomarlo: {aviso}"),
            };
            format!("⚠️ PixPin se cerró antes de que acabara esto. {porque}")
        }
    };
    let parcial = sin_lo_vivo(parcial);
    if parcial.is_empty() {
        aviso
    } else {
        format!("{parcial}\n\n{aviso}")
    }
}

/// Lo que dice la tarjeta flotante al acabar: el principio de la respuesta.
fn resumen_para_la_tarjeta(texto: &str) -> String {
    let t = sin_lo_vivo(texto);
    let t: String = t.chars().take(400).collect();
    if t.is_empty() { "✅ Hecho.".into() } else { t }
}

// ---------------------------------------------------------------------------
// El hilo de fondo

/// La tarjeta flotante de un turno que sigue con el chat cerrado.
pub(crate) struct Fuera {
    mando: Mando,
    chat: Chat,
}

/// Los turnos de camino entre el hilo del chat y el de fondo (en los dos
/// sentidos): quien los coge los mete en su `SESIONES`.
static TRASPASO: Mutex<Vec<Sesion>> = Mutex::new(Vec::new());
/// Si el hilo de fondo esta en pie.
static EN_PIE: AtomicBool = AtomicBool::new(false);

fn con_traspaso<R>(f: impl FnOnce(&mut Vec<Sesion>) -> R) -> R {
    f(&mut TRASPASO.lock().unwrap_or_else(|e| e.into_inner()))
}

/// El chat: `> 0` abierto, `< 0` naciendo, `0` cerrado.
fn estado_del_chat() -> isize {
    super::super::ABIERTA.load(Ordering::SeqCst)
}

/// **Al cerrar la ventana del chat**, desde su hilo y antes de que acabe:
/// lo que siga en marcha (y lo que estuviera de camino) se va al hilo de
/// fondo en vez de morir con el hilo.
pub(crate) fn al_cerrar_el_chat(chat: Chat) {
    let quedan = SESIONES.with_borrow_mut(std::mem::take);
    if !quedan.is_empty() {
        tracing::info!(
            cuantos = quedan.len(),
            "asistente: el chat se cerro con turnos en marcha; siguen de fondo"
        );
    }
    let hay = con_traspaso(|t| {
        t.extend(quedan);
        !t.is_empty()
    });
    if hay {
        poner_en_pie(chat);
    }
}

/// **El bucle del chat, en cada vuelta**: lo que devolvio el hilo de fondo
/// (o lo retomado al arrancar) vuelve a la ventana, que lo pinta en vivo.
pub(crate) fn recoger() {
    let mut vuelven = con_traspaso(std::mem::take);
    if vuelven.is_empty() {
        return;
    }
    for s in &mut vuelven {
        // Su tarjeta ya no hace falta: se ve en el chat.
        if let Some(f) = s.fuera.take() {
            f.mando.poner("💬 Lo sigo en el chat.".into(), None);
        }
    }
    SESIONES.with_borrow_mut(|s| s.extend(vuelven));
}

fn poner_en_pie(chat: Chat) {
    if EN_PIE.swap(true, Ordering::SeqCst) {
        return;
    }
    let lanzado = std::thread::Builder::new()
        .name("asistente-de-fondo".into())
        .spawn(move || servicio(chat));
    if let Err(e) = lanzado {
        // Sin hilo, el turno sigue en el traspaso: lo recoge el chat al
        // abrirse o lo retoma el arranque siguiente (sigue apuntado).
        tracing::warn!(?e, "asistente: no se pudo lanzar el hilo de fondo");
        EN_PIE.store(false, Ordering::SeqCst);
    }
}

/// **El hilo de fondo**: el `latido` del chat sin conversacion abierta,
/// hasta que no quede nada o se abra el chat (entonces se lo devuelve).
fn servicio(chat: Chat) {
    // El OCR y los PDF de lo que pida el turno son WinRT, como en el chat.
    let _com = pixpin_shell::ComDelHilo::iniciar();
    loop {
        let llegan = con_traspaso(std::mem::take);
        SESIONES.with_borrow_mut(|s| s.extend(llegan));
        if estado_del_chat() > 0 {
            // El chat abierto: que lo pinte el en vivo.
            let mias = SESIONES.with_borrow_mut(std::mem::take);
            con_traspaso(|t| t.extend(mias));
            pixpin_shell::overlay::despertar(estado_del_chat());
            break;
        }
        // Con el chat cerrado del todo (no naciendo), la tarjeta que dice
        // que sigue.
        if estado_del_chat() == 0 {
            SESIONES.with_borrow_mut(|v| {
                for s in v.iter_mut().filter(|s| s.fuera.is_none()) {
                    s.fuera = Some(tarjeta(&chat, s));
                }
            });
        }
        let _ = super::latido(&chat.ubicacion, None);
        if SESIONES.with_borrow(Vec::is_empty) {
            break;
        }
        std::thread::sleep(VUELTA);
    }
    EN_PIE.store(false, Ordering::SeqCst);
    // Lo que llego entre la ultima mirada y bajar la bandera no se queda
    // sin nadie (si el chat esta abierto, lo recoge el).
    if estado_del_chat() <= 0 && con_traspaso(|t| !t.is_empty()) {
        poner_en_pie(chat);
    }
}

fn abrir_de(chat: &Chat, s: &Sesion) -> Abrir {
    Abrir {
        idioma: chat.idioma,
        ubicacion: chat.ubicacion.clone(),
        opciones: chat.opciones,
        proyecto: s.proyecto.clone(),
        codigo: s.codigo.clone(),
    }
}

fn tarjeta(chat: &Chat, s: &Sesion) -> Fuera {
    let mando = avisar(Aviso {
        titulo: "🤖 Asistente".into(),
        texto: "⏳ Sigo con tu pedido aunque se cerró el chat. Te aviso al acabar.".into(),
        abrir: Some(abrir_de(chat, s)),
        acabado: false,
        chat: Some(chat.clone()),
    });
    Fuera {
        mando,
        chat: chat.clone(),
    }
}

/// El turno pregunta algo: con el chat cerrado, la tarjeta lo dice (se
/// contesta en la burbuja, con «Abrir»).
pub(super) fn al_preguntar(s: &Sesion, pregunta: &str) {
    if let Some(f) = &s.fuera {
        f.mando.cambiar_texto(format!(
            "❓ Necesito que me contestes en el chat: {}\n\nPulsa «Abrir». Si no contestas, sigo con lo más seguro.",
            pregunta.trim()
        ));
    }
}

/// El turno acabo: fuera del disco y, si seguia con el chat cerrado, la
/// respuesta en su tarjeta con «Abrir».
pub(super) fn al_acabar(s: &mut Sesion, texto: &str) {
    quitar(&s.raiz, &s.nota);
    if let Some(f) = s.fuera.take() {
        let a = abrir_de(&f.chat, s);
        f.mando.poner(
            resumen_para_la_tarjeta(texto),
            Some((a.idioma, a.ubicacion, a.opciones, a.proyecto, a.codigo)),
        );
    }
}

// ---------------------------------------------------------------------------
// Al arrancar

/// **Lo que quedo a medias la ultima vez**, en su hilo (el arranque no
/// espera): cada burbuja cortada dice que paso y, lo que se puede, se
/// vuelve a lanzar y sigue en el chat o en el hilo de fondo.
pub(crate) fn retomar_al_arrancar(chat: Chat) {
    let lanzado = std::thread::Builder::new().name("asistente-retomar".into()).spawn(move || {
        let _com = pixpin_shell::ComDelHilo::iniciar();
        let cortados = tomar_apuntados(chat.ubicacion.raiz());
        if cortados.is_empty() {
            return;
        }
        tracing::info!(cuantos = cortados.len(), "asistente: turnos que quedaron a medias al cerrarse PixPin");
        let textos = pixpin_store::Catalogo::nuevo(chat.idioma);
        let ahora = pixpin_shell::entorno::ahora_utc_ms();
        let mut otra_vez = Vec::new();
        for e in &cortados {
            match retomar_uno(&chat, &textos, e, ahora) {
                Ok(Some(s)) => otra_vez.push(s),
                Ok(None) => {}
                Err(err) => tracing::warn!(?err, proyecto = %e.proyecto, "asistente: no se pudo retomar un turno cortado"),
            }
        }
        // Las burbujas cambiaron por debajo del chat (si esta abierto).
        crate::ventana_chat::refrescar();
        if !otra_vez.is_empty() {
            con_traspaso(|t| t.extend(otra_vez));
            if estado_del_chat() > 0 {
                pixpin_shell::overlay::despertar(estado_del_chat());
            } else {
                poner_en_pie(chat);
            }
        }
    });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "asistente: no se pudo lanzar el hilo de retomar");
    }
}

/// Uno: reescribe su burbuja y, si toca, lo lanza otra vez (la `Sesion`).
fn retomar_uno(
    chat: &Chat,
    textos: &pixpin_store::Catalogo,
    e: &EnMarcha,
    ahora: i64,
) -> std::io::Result<Option<Sesion>> {
    let raiz = chat.ubicacion.raiz();
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &e.proyecto);
    let mut mensajes = cuaderno::Cuaderno::leer_de(&carpeta)?.mensajes;
    mensajes.sort_by_key(|m| m.cuando);
    // Sin su burbuja (el usuario la borro) no hay nada que retomar.
    let Some(parcial) = mensajes
        .iter()
        .find(|m| m.id == e.nota)
        .map(|m| m.texto.clone())
    else {
        return Ok(None);
    };
    let donde = mensajes.iter().position(|m| m.id == e.pregunta);
    let pregunta = donde.and_then(|i| ia_chat::pregunta_de(&mensajes[i], e.asistente));
    let mut decision = que_hacer(e, ahora, pregunta.is_some());
    let mut lanzado = None;
    if let (Retomar::OtraVez, Some(i), Some(pregunta)) = (&decision, donde, pregunta) {
        // Lo de hasta la pregunta: lo que vino despues (la burbuja cortada,
        // lo que escribiera el usuario) no es parte de este pedido.
        let contexto = &mensajes[..=i];
        let numero = mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
        match super::motor_para(&chat.ubicacion, textos, contexto, &pregunta) {
            Ok((motor, pensar, ventana)) => {
                // La burbuja cortada, antes: la nueva nace debajo.
                escribir_cortado(&carpeta, e, &parcial, &decision);
                match super::lanzar_en(
                    raiz,
                    &e.proyecto,
                    e.asistente,
                    contexto,
                    numero,
                    motor,
                    pregunta,
                    Some((pensar, ventana)),
                    e.intentos + 1,
                ) {
                    Ok((_, s)) => {
                        tracing::info!(proyecto = %e.proyecto, "asistente: turno cortado, lanzado otra vez");
                        lanzado = Some(s);
                    }
                    Err(err) => {
                        tracing::warn!(
                            ?err,
                            "asistente: no se pudo lanzar otra vez el turno cortado"
                        );
                        decision = Retomar::Avisar(Motivo::NoSePudo(
                            "no pude empezar a contestar.".into(),
                        ));
                    }
                }
            }
            Err(aviso) => decision = Retomar::Avisar(Motivo::NoSePudo(aviso)),
        }
    }
    if lanzado.is_none() {
        escribir_cortado(&carpeta, e, &parcial, &decision);
        // Con el chat cerrado (PixPin arranco sin el), que se sepa igual.
        if estado_del_chat() == 0 {
            avisar(Aviso {
                titulo: "🤖 Asistente".into(),
                texto: texto_cortado("", &decision, e.asistente),
                abrir: Some(Abrir {
                    idioma: chat.idioma,
                    ubicacion: chat.ubicacion.clone(),
                    opciones: chat.opciones,
                    proyecto: e.proyecto.clone(),
                    codigo: e.nota.clone(),
                }),
                acabado: true,
                chat: Some(chat.clone()),
            });
        }
    }
    Ok(lanzado)
}

fn escribir_cortado(carpeta: &Path, e: &EnMarcha, parcial: &str, r: &Retomar) {
    let texto = texto_cortado(parcial, r, e.asistente);
    if let Err(err) = cuaderno::cambiar(carpeta, &e.nota, |m| {
        m.texto = texto.clone();
        true
    }) {
        tracing::warn!(?err, "asistente: no se pudo reescribir la burbuja cortada");
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn en_marcha(nota: &str) -> EnMarcha {
        EnMarcha {
            proyecto: "p1".into(),
            nota: nota.into(),
            pregunta: "q1".into(),
            asistente: true,
            desde: 1_000,
            actuo: false,
            intentos: 0,
        }
    }

    /// Se apunta al empezar, se marca al actuar y se borra al acabar; lo que
    /// queda se coge una vez (el fichero se vacia) y sin nada no hay fichero.
    #[test]
    fn el_fichero_apunta_marca_y_se_vacia() {
        let raiz = std::env::temp_dir().join(format!("pixpin-de-fondo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        apuntar(&raiz, en_marcha("a"));
        apuntar(&raiz, en_marcha("b"));
        // La misma nota otra vez no se duplica.
        apuntar(&raiz, en_marcha("b"));
        marcar_que_actuo(&raiz, "b");
        let v = leer(&raiz);
        assert_eq!(v.len(), 2);
        assert!(!v[0].actuo && v[1].actuo, "{v:?}");
        quitar(&raiz, "a");
        assert_eq!(
            leer(&raiz)
                .iter()
                .map(|e| e.nota.as_str())
                .collect::<Vec<_>>(),
            ["b"]
        );
        assert_eq!(tomar_apuntados(&raiz).len(), 1);
        assert!(tomar_apuntados(&raiz).is_empty());
        assert!(
            !raiz.join(FICHERO).exists(),
            "sin nada a medias no queda fichero"
        );
        // Uno roto no tumba nada: se ignora.
        std::fs::write(raiz.join(FICHERO), "{roto").unwrap();
        assert!(leer(&raiz).is_empty());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// Solo se repite solo lo que no hizo nada, no se habia retomado, es
    /// reciente y conserva su pregunta.
    #[test]
    fn la_regla_de_retomar() {
        let e = en_marcha("a");
        assert_eq!(que_hacer(&e, 2_000, true), Retomar::OtraVez);
        assert_eq!(
            que_hacer(&e, 2_000, false),
            Retomar::Avisar(Motivo::SinPregunta)
        );
        assert_eq!(
            que_hacer(
                &EnMarcha {
                    actuo: true,
                    ..e.clone()
                },
                2_000,
                true
            ),
            Retomar::Avisar(Motivo::YaActuo)
        );
        assert_eq!(
            que_hacer(
                &EnMarcha {
                    intentos: 1,
                    ..e.clone()
                },
                2_000,
                true
            ),
            Retomar::Avisar(Motivo::YaSeRetomo)
        );
        assert_eq!(
            que_hacer(&e, 1_000 + RETOMAR_HASTA_MS + 1, true),
            Retomar::Avisar(Motivo::Viejo)
        );
    }

    /// La burbuja cortada guarda lo escrito sin lo de en vivo y dice que
    /// paso y como seguir (con `@asistente` fuera de su chat).
    #[test]
    fn la_burbuja_cortada_dice_que_paso() {
        assert_eq!(sin_lo_vivo("💭 Pensando…"), "");
        assert_eq!(sin_lo_vivo("Hola, voy a ▍"), "Hola, voy a");
        assert_eq!(sin_lo_vivo("Primero esto.\n🎨 Dibujando…"), "Primero esto.");
        // Un texto suyo que acaba en puntos suspensivos se queda.
        assert_eq!(sin_lo_vivo("Y entonces…"), "Y entonces…");

        let t = texto_cortado("💭 Pensando…", &Retomar::OtraVez, true);
        assert!(t.starts_with("↻ PixPin se cerró"), "{t}");
        let t = texto_cortado(
            "Ya apunte dos tareas ▍",
            &Retomar::Avisar(Motivo::YaActuo),
            true,
        );
        assert!(
            t.starts_with("Ya apunte dos tareas\n\n⚠️ PixPin se cerró"),
            "{t}"
        );
        assert!(t.contains("«continúa»") && !t.contains("@asistente"), "{t}");
        let t = texto_cortado("", &Retomar::Avisar(Motivo::Viejo), false);
        assert!(t.contains("«@asistente continúa»"), "{t}");
        let t = texto_cortado(
            "",
            &Retomar::Avisar(Motivo::NoSePudo("falta la clave".into())),
            true,
        );
        assert!(t.ends_with("No he podido retomarlo: falta la clave"), "{t}");
    }
}
