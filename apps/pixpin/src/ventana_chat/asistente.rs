//! **El asistente local en la ventana del chat** (encargo 1): lo que engancha
//! `crate::asistente` con la conversacion abierta.
//!
//! El chat solo llama a cinco cosas, en sitios contados:
//!
//! - [`al_enviar`], justo despues de guardar una nota: si era para el
//!   asistente (el chat «Asistente», o `@asistente` en otro), nace su burbuja
//!   y el turno se lanza en su hilo.
//! - [`latido`], una vez por vuelta del bucle: recoge lo que haya escrito el
//!   modelo (la burbuja crece en vivo), lo que haya que meter en un lienzo
//!   y la descarga.
//! - [`espera`]: cada cuanto volver a mirar mientras hay algo en marcha.
//! - [`ocupado`]/[`parar`]: el boton de enviar es «parar» mientras contesta.
//! - [`pintar_tarjeta`]/[`tocar`]: la tarjeta de bajar el modelo, la
//!   primera vez, en el chat «Asistente»; y la lista de `@` mientras se
//!   escribe ([`pintar_menciones`], [`tecla_de_menciones`]).
//! - [`lienzo_por_abrir`]: el lienzo que pidio abrir al terminar («…y
//!   ábrelo»); [`lienzo_de`]: el de «Abrir lienzo» de una respuesta.
//!
//! Todo vive en el hilo de la ventana (`thread_local`): el bucle del chat es
//! uno y no hay que compartir nada con otros hilos salvo por los canales.

use std::cell::RefCell;
use std::sync::Arc;
use std::time::{Duration, Instant};

use pixpin_ia::motor::Motor;
use pixpin_proyecto::cuaderno;

use super::*;
use crate::asistente::chat::{self as ia_chat, EnVivo};
use crate::asistente::descarga::{self as ia_descarga, AvisoDescarga, Descarga};
use crate::asistente::turno::{Aviso, Turno};
use crate::asistente::{al_lienzo, motor as ia_motor, nube as ia_nube, pensado as ia_pensado, turno as ia_turno};

/// Cada cuanto se escribe en el disco lo que va contestando (en memoria se
/// ve al momento). Un turno de minuto y medio no son cien escrituras.
const GUARDAR_CADA: Duration = Duration::from_secs(2);

/// Lo que se ve en la burbuja de la respuesta desde el primer instante,
/// antes de que llegue nada del modelo (regla del usuario: algo visible en
/// menos de medio segundo).
pub(super) const ESTADO_INICIAL: &str = "💭 Pensando…";

/// Un turno en marcha y su burbuja.
struct Sesion {
    proyecto: String,
    /// El id de la nota de la respuesta.
    nota: String,
    /// Su codigo unico: con el se nombra lo que penso (`pensado`).
    codigo: String,
    turno: Turno,
    vivo: EnVivo,
    guardado: Instant,
    /// Lo que penso y la hoja reescrita, para guardar al acabar.
    registro: Option<pixpin_ia::hoja_trabajo::Registro>,
    /// El lienzo del chat donde dibujo (sin uno abierto), por su
    /// referencia: la respuesta lo apunta para su «Abrir lienzo».
    lienzo: Option<String>,
    /// Se pidio abrirlo al terminar («…y ábrelo»).
    abrir: bool,
}

/// La tarjeta de la primera vez.
#[derive(Default)]
struct Tarjeta {
    /// Lo elegido en la tarjeta (`None`: lo de los ajustes).
    ligero: Option<bool>,
    /// Se pulso «Bajar» y se espera el «Si».
    confirmando: bool,
    /// La ultima noticia (cancelada, fallo).
    noticia: Option<String>,
    /// Lo pulsable, como se pinto.
    botones: Vec<(Rect, Accion)>,
    /// Si el modelo esta listo, mirado hace poco (mirar el disco en cada
    /// fotograma no).
    listo: Option<(Instant, &'static str, bool)>,
    /// El modelo de los ajustes, leido hace poco (el TOML no se lee en cada
    /// fotograma).
    ajuste: Option<(Instant, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Accion {
    Elegir { ligero: bool },
    Bajar,
    Confirmar,
    Cancelar,
}

thread_local! {
    static SESIONES: RefCell<Vec<Sesion>> = const { RefCell::new(Vec::new()) };
    static DESCARGA: RefCell<Option<Descarga>> = const { RefCell::new(None) };
    static TARJETA: RefCell<Tarjeta> = RefCell::new(Tarjeta::default());
    /// El lienzo que se pidio abrir al terminar: (proyecto, referencia).
    static POR_ABRIR: RefCell<Option<(String, String)>> = const { RefCell::new(None) };
    /// Donde y cuando se precargo el modelo por ultima vez.
    static PRECARGADO: RefCell<Option<(String, Instant)>> = const { RefCell::new(None) };
    /// La lista de `@`: la opcion marcada, lo pulsable como se pinto, y si
    /// se cerro con Escape para ese `@` (su posicion).
    static MENCIONES: RefCell<Menciones> = RefCell::new(Menciones::default());
}

#[derive(Default)]
struct Menciones {
    marcada: usize,
    botones: Vec<(Rect, usize)>,
    cerrada_en: Option<(String, usize)>,
}

/// Si el asistente esta contestando en este chat.
pub(super) fn ocupado(proyecto: &str) -> bool {
    SESIONES.with_borrow(|s| s.iter().any(|x| x.proyecto == proyecto && !x.turno.acabado()))
}

/// «Parar»: corta en el siguiente trozo.
pub(super) fn parar(proyecto: &str) {
    SESIONES.with_borrow(|s| {
        s.iter()
            .filter(|x| x.proyecto == proyecto)
            .for_each(|x| x.turno.parar())
    });
}

/// Cada cuanto volver a mirar (ms) mientras hay algo en marcha.
pub(super) fn espera() -> Option<u32> {
    let turnos = SESIONES.with_borrow(|s| !s.is_empty());
    let bajando = DESCARGA.with_borrow(|d| d.is_some());
    (turnos || al_lienzo::hay_pedidos()).then_some(80).or(bajando.then_some(250))
}


/// Si el mensaje es una orden simple que el camino rapido hace sin modelo
/// (`pixpin_ia::rapido`).
fn es_orden_rapida(pregunta: &str) -> bool {
    let ahora = pixpin_ia::fechas::Momento::de_ms_locales(pixpin_shell::entorno::ahora_local_ms());
    pixpin_ia::rapido::interpretar(pregunta, ahora).is_some()
}

/// El «motor» cuando no hay ninguno (sin bajar, sin clave): el camino
/// rapido no lo necesita, y si hiciera falta, dice por que no hay.
struct SinMotor(String);

impl Motor for SinMotor {
    fn completar(
        &self,
        _: &pixpin_ia::motor::Peticion,
        _: &mut dyn FnMut(&pixpin_ia::motor::Trozo) -> pixpin_ia::motor::Seguir,
    ) -> Result<pixpin_ia::motor::Respuesta, pixpin_ia::ErrorIa> {
        Err(pixpin_ia::ErrorIa::Nube(self.0.clone()))
    }
}

/// Cada cuanto, como mucho, se vuelve a precargar por escribir `@asistente`
/// en el mismo chat (menos que el reposo de 5 min del motor: si se sigue
/// escribiendo, sigue cargado).
const REPRECARGAR: Duration = Duration::from_secs(4 * 60);

/// **La precarga** (14-oct): al abrir el chat «Asistente», o al escribir
/// `@asistente` en otro, se arranca `llama-server` en segundo plano si el
/// motor es el local. Nunca bloquea: leer los ajustes es un fichero pequeño
/// y la carga va en su hilo (`motor::precargar`). Una vez por chat abierto
/// (y otra si pasan [`REPRECARGAR`] escribiendo): tener el chat abierto sin
/// usarlo no retiene la memoria mas que el reposo de siempre.
fn precargar_si_toca(ubicacion: &Ubicacion, a: &Abierto) {
    let menciona = || pixpin_ia::acciones::normalizar(&a.borrador).contains("@asistente");
    if !(a.ficha.es_asistente() || menciona()) {
        return;
    }
    let toca = PRECARGADO.with_borrow(|p| match p {
        Some((proyecto, cuando)) => *proyecto != a.ficha.id || (menciona() && cuando.elapsed() >= REPRECARGAR),
        None => true,
    });
    if !toca {
        return;
    }
    PRECARGADO.with_borrow_mut(|p| *p = Some((a.ficha.id.clone(), Instant::now())));
    let aj = pixpin_store::cargar(ubicacion).unwrap_or_default();
    // Solo el local tiene algo que cargar; la nube contesta en frio.
    if !matches!(ia_nube::eleccion(&aj.asistente), ia_nube::Eleccion::Local) {
        return;
    }
    let modelo = ia_motor::modelo_elegido(Some(aj.asistente.modelo.as_str()).filter(|m| !m.is_empty()));
    if let Some(m) = ia_motor::raiz().and_then(|raiz| ia_motor::del_proceso(&raiz, &modelo).ok()) {
        ia_motor::precargar(m);
    }
}

/// El motor elegido en Ajustes → Asistente: el local (si esta bajado) o la
/// nube con su clave, con el local de respaldo si se quiere y esta bajado.
fn motor_de_los_ajustes(ubicacion: &Ubicacion, textos: &Catalogo) -> Result<Arc<dyn Motor>, String> {
    let aj = pixpin_store::cargar(ubicacion).unwrap_or_default();
    let modelo = ia_motor::modelo_elegido(Some(aj.asistente.modelo.as_str()).filter(|m| !m.is_empty()));
    let local = || ia_motor::raiz().and_then(|raiz| ia_motor::del_proceso(&raiz, &modelo).ok());
    match ia_nube::eleccion(&aj.asistente) {
        ia_nube::Eleccion::Nube(p) => ia_nube::motor(p, if aj.asistente.respaldo_local { local() } else { None }),
        ia_nube::Eleccion::Local => local().ok_or_else(|| textos.t("asistente-sin-modelo")),
    }
}

/// **Despues de guardar una nota o adjuntar una foto**: si era para el
/// asistente, lanza el turno. Devuelve un aviso para el usuario si no se
/// pudo.
///
/// Una foto tambien: en el chat «Asistente» una foto sola se contesta
/// (antes no se llamaba a nadie al adjuntarla y el usuario se quedaba
/// esperando una respuesta que no iba a llegar, 13-oct).
pub(super) fn al_enviar(ubicacion: &Ubicacion, a: &mut Abierto, textos: &Catalogo) -> Option<String> {
    let nueva = a.mensajes.last()?;
    let pregunta = ia_chat::pregunta_de(nueva, a.ficha.es_asistente())?;
    if ocupado(&a.ficha.id) {
        return Some(textos.t("asistente-ocupado"));
    }
    let motor = match motor_de_los_ajustes(ubicacion, textos) {
        Ok(m) => m,
        // Sin modelo (o sin clave) las ordenes simples igual se hacen: van
        // por el camino rapido y no lo necesitan. Lo demas dira el aviso.
        Err(aviso) if es_orden_rapida(&pregunta) => Arc::new(SinMotor(aviso)),
        Err(aviso) => return Some(aviso),
    };
    // Pensar y la ventana: el local segun su modelo; la nube piensa por su
    // cuenta (Claude resumido, DeepSeek `reasoning_content`) y tiene
    // ventana de sobra, pero la hoja y el reinicio valen igual.
    let aj = pixpin_store::cargar(ubicacion).unwrap_or_default().asistente;
    let (pensar, ventana) = match ia_nube::eleccion(&aj) {
        ia_nube::Eleccion::Nube(_) => (aj.pensar.unwrap_or(true), ia_pensado::VENTANA_NUBE),
        ia_nube::Eleccion::Local => (ia_pensado::pensar_local(aj.pensar), ia_pensado::ventana_local()),
    };
    match lanzar_pensando(ubicacion, a, motor, pregunta, pensar, ventana) {
        Ok(()) => None,
        Err(e) => {
            tracing::warn!(?e, "asistente: no se pudo empezar a contestar");
            Some(textos.t("asistente-fallo"))
        }
    }
}

/// Lanza el turno con este motor (las pruebas le pasan uno de mentira), sin
/// pensar ni hoja de trabajo: el turno de antes del 13-oct.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn lanzar(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    motor: Arc<dyn Motor>,
    pregunta: String,
) -> std::io::Result<()> {
    lanzar_con(ubicacion, a, motor, pregunta, None)
}

/// **Lanza el turno con memoria** (13-oct): piensa si `pensar`, planea la
/// peticion mejorada, lleva la hoja de trabajo del chat y reinicia el
/// contexto al pasar del 70 % de `ventana`.
pub(super) fn lanzar_pensando(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    motor: Arc<dyn Motor>,
    pregunta: String,
    pensar: bool,
    ventana: u32,
) -> std::io::Result<()> {
    lanzar_con(ubicacion, a, motor, pregunta, Some((pensar, ventana)))
}

fn lanzar_con(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    motor: Arc<dyn Motor>,
    pregunta: String,
    pensando: Option<(bool, u32)>,
) -> std::io::Result<()> {
    let raiz = ubicacion.raiz().to_path_buf();
    let proyecto = a.ficha.id.clone();
    let n = a.mensajes.len();
    let (anteriores, nueva) = a.mensajes.split_at(n.saturating_sub(1));
    let pregunta_id = nueva.first().map(|m| m.id.clone());
    // La foto puede ser la misma que se acaba de mandar.
    let foto = ia_chat::foto_reciente(&a.mensajes).and_then(|m| ruta_del_mensaje(&raiz, &proyecto, m));
    let adjuntos = ia_chat::adjuntos(&a.mensajes, |m| ruta_del_mensaje(&raiz, &proyecto, m).filter(|r| r.is_file()));
    // `@lienzo:<nombre>`: donde dibujar si no hay un lienzo abierto.
    let (para_el_modelo, destino) = ia_chat::resolver_lienzo(&pregunta, &a.mensajes);
    let abrir = ia_chat::pide_abrir(&pregunta);
    // Las herramientas de este turno (`acciones::herramientas_para`): con la
    // memoria, la vuelta de planear puede anadir las que nombre.
    // (Claude y DeepSeek, el catalogo entero, como en `Turno::lanzar_hacia`.)
    let herramientas = if motor.catalogo_completo() {
        pixpin_ia::herramientas::catalogo()
    } else {
        crate::asistente::acciones::herramientas_para(
            &[pixpin_ia::Mensaje::usuario(para_el_modelo.clone())],
            &adjuntos,
            foto.is_some(),
        )
    };
    let (historial, memoria) = match pensando {
        Some((pensar, ventana)) => {
            let (h, m) = ia_pensado::preparar_turno(
                &raiz,
                &proyecto,
                anteriores,
                &para_el_modelo,
                pensar,
                ventana,
                &herramientas,
            );
            (h, Some(m))
        }
        None => {
            let conversacion = ia_chat::conversacion(anteriores);
            (ia_turno::historial_para_el_modelo(&conversacion, &para_el_modelo, None), None)
        }
    };

    // La burbuja de la respuesta nace ya, con el cursor: el usuario ve que
    // se le esta contestando aunque el modelo tarde en cargar.
    let mut vivo = EnVivo::default();
    vivo.estado = Some(ESTADO_INICIAL.into());
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    // En otro chat la respuesta cuelga de la pregunta, para que se sepa a
    // que contesta entre los mensajes de los demas.
    let responde_a = if a.ficha.es_asistente() { None } else { pregunta_id };
    let nota = escribir_nota(
        &raiz,
        &proyecto,
        ia_chat::APARATO,
        &vivo.mostrar(),
        descripcion::hora_sin_repetir(),
        numero,
        responde_a,
    )?;
    let id = nota.id.clone();
    let codigo = nota.codigo_unico();
    a.vistas.push(None);
    a.mensajes.push(nota);
    a.colocado.borrow_mut().ancho = 0;

    let turno = Turno::lanzar_con_memoria(
        motor,
        proyecto.clone(),
        historial,
        adjuntos,
        foto,
        destino,
        herramientas,
        memoria,
    );
    SESIONES.with_borrow_mut(|s| {
        s.push(Sesion {
            proyecto,
            nota: id,
            codigo,
            turno,
            vivo,
            guardado: Instant::now(),
            registro: None,
            lienzo: None,
            abrir,
        })
    });
    Ok(())
}

/// **El lienzo que pidio abrir al terminar** («dibuja… y ábrelo»), por su
/// posicion en la conversacion abierta. Se entrega una vez.
pub(super) fn lienzo_por_abrir(a: &Abierto) -> Option<usize> {
    let (proyecto, referencia) = POR_ABRIR.with_borrow(Clone::clone)?;
    if proyecto != a.ficha.id {
        return None;
    }
    POR_ABRIR.with_borrow_mut(|p| *p = None);
    a.mensajes
        .iter()
        .rposition(|m| ia_chat::es_lienzo(m) && m.referencia.as_deref() == Some(referencia.as_str()))
}

/// El lienzo de «Abrir lienzo» de la respuesta `i`, si lo lleva.
pub(super) fn lienzo_de(a: &Abierto, i: usize) -> Option<usize> {
    ia_chat::lienzo_de_la_respuesta(a.mensajes.get(i)?, &a.mensajes)
}

/// A que lado va un mensaje: en una conversacion con el asistente (su chat,
/// o uno donde ya contesto), lo del usuario a la DERECHA y lo del asistente
/// a la izquierda, como en cualquier mensajeria. Lo demas, todo a la
/// izquierda como siempre (es un cuaderno propio, como en el movil).
pub(super) fn a_la_derecha(con_ia: bool, m: &cuaderno::Mensaje) -> bool {
    con_ia && !ia_chat::es_del_asistente(m)
}

/// Si en esta conversacion se habla con el asistente.
pub(super) fn conversacion_con_ia(a: &Abierto) -> bool {
    a.ficha.es_asistente() || a.mensajes.iter().any(ia_chat::es_del_asistente)
}

/// **Una vez por vuelta del bucle.** Devuelve si hay que repintar y un
/// aviso, si lo hay.
pub(super) fn latido(ubicacion: &Ubicacion, mut abierto: Option<&mut Abierto>) -> (bool, Option<String>) {
    let mut cambio = false;
    let mut aviso = None;

    // 0. Precargar el modelo local al abrir el chat «Asistente» o al escribir
    //    `@asistente` en otro: la primera pregunta ya no espera la carga.
    if let Some(a) = abierto.as_deref() {
        precargar_si_toca(ubicacion, a);
    }

    // 1. Lo que hay que meter en un lienzo, si no hay ninguno abierto (si lo
    //    hay, lo atiende su bucle).
    if !al_lienzo::hay_editor() {
        for p in al_lienzo::tomar() {
            let r = meter_en_el_lienzo_del_chat(
                ubicacion,
                abierto.as_deref_mut(),
                &p.proyecto,
                p.destino.as_deref(),
                &p.documento,
            );
            cambio = true;
            let r = match r {
                Ok((resumen, referencia)) => {
                    SESIONES.with_borrow_mut(|s| {
                        if let Some(s) = s.iter_mut().find(|s| s.proyecto == p.proyecto) {
                            s.lienzo = Some(referencia);
                        }
                    });
                    Ok(resumen)
                }
                Err(e) => Err(e),
            };
            p.responder(r);
        }
    }

    // 2. Los turnos en marcha.
    let raiz = ubicacion.raiz().to_path_buf();
    let mut leidos: Vec<(String, ia_turno::Adjunto, String)> = Vec::new();
    SESIONES.with_borrow_mut(|sesiones| {
        for s in sesiones.iter_mut() {
            let avisos = s.turno.recibir();
            if avisos.is_empty() {
                continue;
            }
            let mut acabo = false;
            for av in avisos {
                match av {
                    Aviso::Evento(e) => s.vivo.aplicar(&e),
                    Aviso::TextoLeido { de, texto } => leidos.push((s.proyecto.clone(), de, texto)),
                    Aviso::Registro(r) => s.registro = Some(r),
                    Aviso::Fin { resultado } => {
                        s.vivo.acabar(&resultado);
                        acabo = true;
                    }
                }
            }
            // Lo que piensa y la peticion mejorada, encima de la burbuja: si
            // cambio, la burbuja cambia de alto y hay que volver a medir.
            if ia_pensado::poner_en_vivo(&s.nota, ia_pensado::Seccion::de_vivo(&s.vivo))
                && let Some(a) = abierto.as_deref_mut().filter(|a| a.ficha.id == s.proyecto)
            {
                a.colocado.borrow_mut().ancho = 0;
                cambio = true;
            }
            if acabo {
                match s.registro.take() {
                    Some(r) => ia_pensado::guardar(&raiz, &s.proyecto, &s.nota, &s.codigo, r),
                    None => ia_pensado::olvidar_en_vivo(&s.nota),
                }
            }
            let texto = s.vivo.mostrar();
            // Al acabar, la respuesta apunta el lienzo donde dibujo: de ahi
            // sale su «Abrir lienzo».
            let lienzo = s.lienzo.clone().filter(|_| acabo);
            let apuntar = |m: &mut cuaderno::Mensaje| {
                if let Some(r) = &lienzo {
                    m.resto
                        .insert(ia_chat::LIENZO_DE_LA_RESPUESTA.into(), serde_json::Value::String(r.clone()));
                }
            };
            if let Some(a) = abierto.as_deref_mut().filter(|a| a.ficha.id == s.proyecto)
                && let Some(m) = a.mensajes.iter_mut().find(|m| m.id == s.nota)
                && (m.texto != texto || lienzo.is_some())
            {
                m.texto = texto.clone();
                apuntar(m);
                a.colocado.borrow_mut().ancho = 0;
                cambio = true;
            }
            if acabo && s.abrir && let Some(r) = &lienzo {
                POR_ABRIR.with_borrow_mut(|p| *p = Some((s.proyecto.clone(), r.clone())));
            }
            if acabo || s.guardado.elapsed() >= GUARDAR_CADA {
                s.guardado = Instant::now();
                let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, &s.proyecto);
                if let Err(e) = cuaderno::cambiar(&carpeta, &s.nota, |m| {
                    m.texto = texto.clone();
                    apuntar(m);
                    true
                }) {
                    tracing::warn!(?e, "asistente: no se pudo guardar la respuesta");
                }
                if acabo {
                    // «Leer siempre las respuestas» (`voz_del_asistente`).
                    super::voz_del_asistente::al_acabar(&s.proyecto, &s.nota);
                    let resumen: String = texto.chars().take(120).collect();
                    let _ = subir_en_la_lista(&raiz, &s.proyecto, pixpin_shell::entorno::ahora_utc_ms(), &resumen);
                }
            }
        }
        sesiones.retain(|s| !s.vivo.acabado);
    });

    // 3. El texto entero de un OCR: al chat, como nota y como .txt.
    for (proyecto, de, texto) in leidos {
        match dejar_texto_leido(&raiz, abierto.as_deref_mut(), &proyecto, &de, &texto) {
            Ok(()) => cambio = true,
            Err(e) => tracing::warn!(?e, "asistente: no se pudo dejar el texto leido"),
        }
    }

    // 4. La descarga.
    let fin = DESCARGA.with_borrow_mut(|d| {
        let des = d.as_mut()?;
        if des.mirar() {
            cambio = true;
        }
        let fin = des.acabada.clone()?;
        let modelo = des.modelo;
        *d = None;
        Some((fin, modelo))
    });
    if let Some((fin, modelo)) = fin {
        TARJETA.with_borrow_mut(|t| {
            t.listo = None;
            t.confirmando = false;
            t.noticia = match &fin {
                AvisoDescarga::Cancelada => Some("asistente-cancelada".into()),
                AvisoDescarga::Fallo(m) => Some(m.clone()),
                _ => None,
            };
        });
        if fin == AvisoDescarga::Lista
            && let Some(raiz_m) = ia_motor::raiz()
            && let Ok(m) = ia_motor::del_proceso(&raiz_m, &modelo)
        {
            // Que la primera pregunta no espere los 20 s de la carga.
            ia_motor::precargar(m);
            aviso = Some("asistente-listo".to_string());
        }
        cambio = true;
    }
    (cambio, aviso)
}

/// El OCR entero en el chat: una nota con el texto (se lee y se copia ahi
/// mismo) y el mismo texto como `.txt` adjunto (se guarda o se envia como
/// cualquier fichero). Un PDF con capa de texto no se puede escribir con lo
/// que hay en el repo (ver `asistente::ocr`).
fn dejar_texto_leido(
    raiz: &std::path::Path,
    mut abierto: Option<&mut Abierto>,
    proyecto: &str,
    de: &ia_turno::Adjunto,
    texto: &str,
) -> std::io::Result<()> {
    let base = std::path::Path::new(&de.nombre)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "texto".into());
    let numero = siguiente_numero_en(raiz, proyecto)?;
    let nota = escribir_nota(
        raiz,
        proyecto,
        ia_chat::APARATO,
        &format!("📄 Texto de «{}»:\n\n{}", de.nombre, texto),
        descripcion::hora_sin_repetir(),
        numero,
        None,
    )?;
    let fichero = adjuntar_con_pie(
        raiz,
        proyecto,
        ia_chat::APARATO,
        &format!("{base} (texto).txt"),
        texto.as_bytes(),
        numero + 1,
        "",
        None,
    )?;
    if let Some(a) = abierto.as_deref_mut().filter(|a| a.ficha.id == proyecto) {
        for m in [nota, fichero] {
            a.vistas.push(None);
            a.mensajes.push(m);
        }
        a.colocado.borrow_mut().ancho = 0;
    }
    Ok(())
}

/// El nombre del lienzo propio del asistente, al crearlo.
pub(super) const NOMBRE_LIENZO_PROPIO: &str = "Lienzo del asistente";

/// **Lo dibujado, a un lienzo del chat**, sin abrirlo: el mencionado
/// (`destino`) o, si no, el propio del asistente (creandolo la primera vez).
/// Leerlo, pegar en un paso y guardarlo por el camino de siempre. Devuelve
/// el resumen para el modelo y la referencia del lienzo.
pub(super) fn meter_en_el_lienzo_del_chat(
    ubicacion: &Ubicacion,
    mut abierto: Option<&mut Abierto>,
    proyecto: &str,
    destino: Option<&str>,
    documento: &str,
) -> Result<(String, String), String> {
    let raiz = ubicacion.raiz();
    let es_el_abierto = abierto.as_deref().is_some_and(|a| a.ficha.id == proyecto);
    let mensajes: Vec<cuaderno::Mensaje> = match abierto.as_deref() {
        Some(a) if es_el_abierto => a.mensajes.clone(),
        _ => cuaderno::Cuaderno::leer_de(&pixpin_proyecto::almacen::carpeta(raiz, proyecto))
            .map(|c| c.mensajes)
            .unwrap_or_default(),
    };
    let elegido = destino
        .and_then(|r| {
            mensajes
                .iter()
                .rposition(|m| ia_chat::es_lienzo(m) && m.referencia.as_deref() == Some(r))
        })
        .or_else(|| ia_chat::lienzo_propio(&mensajes));
    let (indice, mensaje, nuevo) = match elegido {
        Some(i) => (Some(i), mensajes[i].clone(), false),
        None => {
            let numero = mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
            let m = escribir_lienzo(raiz, proyecto, ia_chat::APARATO, numero, Some(NOMBRE_LIENZO_PROPIO))
                .map_err(|e| {
                    tracing::warn!(?e, "asistente: no se pudo crear su lienzo");
                    "no se pudo crear un lienzo".to_string()
                })?;
            (None, m, true)
        }
    };
    let referencia = mensaje.referencia.clone().unwrap_or_default();
    let hoja = preparar_hoja(raiz, proyecto, &mensaje, pixpin_shell::entorno::ahora_utc_ms())
        .ok_or("no se pudo abrir el lienzo")?;
    let mut escena = hoja.escena;
    let n = al_lienzo::meter_al_lado(&mut escena, documento)?;
    if !guardar_hoja_dibujada(&hoja.ruta, &hoja.lienzo, &escena) {
        return Err("no se pudo guardar el lienzo".into());
    }
    if let Some(a) = abierto.as_deref_mut().filter(|_| es_el_abierto) {
        let vista = leer_vista(ubicacion, proyecto, &mensaje);
        match indice {
            Some(i) if i < a.vistas.len() => a.vistas[i] = vista,
            _ => {
                a.vistas.push(vista);
                a.mensajes.push(mensaje.clone());
            }
        }
        a.colocado.borrow_mut().ancho = 0;
    }
    let nombre = ia_chat::nombre_de_lienzo(&mensaje);
    Ok((
        format!(
            "{} «{nombre}»{}",
            al_lienzo::resumen(n),
            if nuevo { " (lienzo nuevo en este chat)" } else { "" }
        ),
        referencia,
    ))
}

// ---------------------------------------------------------------------------
// La tarjeta de la primera vez

/// Si la tarjeta se ensena: en el chat «Asistente» mientras falte el
/// modelo elegido (o se este bajando).
fn hace_falta_tarjeta(a: &Abierto) -> bool {
    if !a.ficha.es_asistente() {
        return false;
    }
    if DESCARGA.with_borrow(|d| d.is_some()) {
        return true;
    }
    // Con Claude o DeepSeek no hace falta bajar nada.
    if ia_nube::eleccion_reciente(&a.raiz) != ia_nube::Eleccion::Local {
        return false;
    }
    let Some(raiz) = ia_motor::raiz() else { return false };
    TARJETA.with_borrow_mut(|t| {
        let modelo = elegido(t, &a.raiz);
        match t.listo {
            Some((cuando, id, listo)) if id == modelo.id && cuando.elapsed() < Duration::from_secs(5) => !listo,
            _ => {
                let listo = ia_descarga::listo(&raiz, &modelo);
                t.listo = Some((Instant::now(), modelo.id, listo));
                !listo
            }
        }
    })
}

/// El modelo de la tarjeta: lo elegido en ella o, si no, lo de los ajustes;
/// sin nada, el ligero si el equipo tiene poca memoria.
fn elegido(t: &mut Tarjeta, raiz_datos: &std::path::Path) -> pixpin_ia::modelos::Modelo {
    use pixpin_ia::modelos::{QWEN35_08B, QWEN35_2B};
    match t.ligero {
        Some(true) => QWEN35_08B,
        Some(false) => QWEN35_2B,
        None => {
            let fresco = t.ajuste.as_ref().is_some_and(|(c, _)| c.elapsed() < Duration::from_secs(5));
            if !fresco {
                let u = Ubicacion::Instalado { raiz: raiz_datos.to_path_buf() };
                let aj = pixpin_store::cargar(&u).unwrap_or_default();
                t.ajuste = Some((Instant::now(), aj.asistente.modelo));
            }
            let id = t.ajuste.as_ref().map(|(_, m)| m.as_str()).unwrap_or_default();
            if !id.is_empty() {
                ia_motor::modelo_elegido(Some(id))
            } else if poca_memoria() {
                QWEN35_08B
            } else {
                QWEN35_2B
            }
        }
    }
}

fn poca_memoria() -> bool {
    let ram = pixpin_shell::hechos::recolectar().ram_fisica_bytes;
    ram > 0 && ram / (1024 * 1024) < ia_motor::RAM_PARA_EL_2B_MB
}

/// **Pinta la tarjeta** encima del historial del chat «Asistente».
pub(super) fn pintar_tarjeta(p: &Pintor, d: &Disposicion, c: &Pinta, a: &Abierto) {
    if !hace_falta_tarjeta(a) {
        TARJETA.with_borrow_mut(|t| t.botones.clear());
        return;
    }
    let (tema, textos) = (c.tema, c.textos);
    let e = c.escala as f32 / 100.0;
    let area = area_del_historial(d, a, c.escala);
    let ancho = (460.0 * e).min(area.ancho as f32 - 32.0 * e).max(120.0);
    let x = area.x as f32 + (area.ancho as f32 - ancho) / 2.0;
    let y0 = area.y as f32 + 16.0 * e;
    let pad = 16.0 * e;
    let dentro = ancho - 2.0 * pad;
    let raiz_m = ia_motor::raiz();
    let bajando = DESCARGA.with_borrow(|d| d.as_ref().map(|d| (d.fraccion(), d.ultimo.clone(), d.modelo)));

    TARJETA.with_borrow_mut(|t| {
        t.botones.clear();
        let modelo = elegido(t, &a.raiz);
        let explica = textos.t("asistente-explica");
        let (_, alto_explica) = p.medir_texto_ajustado(&explica, 13.0 * e, dentro);
        let fila = 48.0 * e;
        let alto = pad + 22.0 * e + alto_explica + 12.0 * e + 2.0 * (fila + 8.0 * e) + 30.0 * e + 36.0 * e + pad;
        let caja = RectF { x, y: y0, ancho, alto };
        p.rellenar_redondeado(caja, 16.0 * e, tema.burbuja_otra);
        p.trazar(caja, (1.0 * e).max(1.0), tema.isla_borde);

        let mut y = y0 + pad;
        p.texto(&format!("🤖 {}", textos.t("asistente-titulo")), x + pad, y, 16.0 * e, tema.texto);
        y += 22.0 * e;
        p.texto_ajustado(&explica, x + pad, y, 13.0 * e, dentro, tema.apagado);
        y += alto_explica + 12.0 * e;

        // Los dos modelos, para elegir.
        for (ligero, m) in [(false, pixpin_ia::modelos::QWEN35_2B), (true, pixpin_ia::modelos::QWEN35_08B)] {
            let r = RectF { x: x + pad, y, ancho: dentro, alto: fila };
            let es = m.id == modelo.id;
            p.rellenar_redondeado(r, 10.0 * e, if es { con_alfa(tema.enviar, 0.18) } else { tema.campo });
            if es {
                p.trazar(r, (1.5 * e).max(1.0), tema.enviar);
            }
            let centro = (r.x + 16.0 * e, r.y + fila / 2.0);
            p.anillo(centro, 7.0 * e, (1.5 * e).max(1.0), tema.enviar);
            if es {
                p.circulo(centro, 4.0 * e, tema.enviar);
            }
            let falta = raiz_m.as_deref().map_or(m.bytes(), |rz| ia_descarga::falta(rz, &m));
            let clave = if ligero { "asistente-modelo-ligero" } else { "asistente-modelo-normal" };
            p.texto_linea(
                &format!("{} · {}", textos.t(clave), ia_descarga::tamano(falta)),
                r.x + 32.0 * e,
                r.y + 7.0 * e,
                14.0 * e,
                dentro - 40.0 * e,
                tema.texto,
            );
            let detalle = if ligero { "asistente-detalle-ligero" } else { "asistente-detalle-normal" };
            p.texto_linea(&textos.t(detalle), r.x + 32.0 * e, r.y + 27.0 * e, 12.0 * e, dentro - 40.0 * e, tema.apagado);
            if bajando.is_none() {
                t.botones.push((a_rect(r), Accion::Elegir { ligero }));
            }
            y += fila + 8.0 * e;
        }

        // La linea de estado: progreso, la pregunta de confirmar, o la
        // ultima noticia.
        let falta = raiz_m.as_deref().map_or(modelo.bytes(), |rz| ia_descarga::falta(rz, &modelo));
        let estado = match &bajando {
            Some((fr, Some(pr), _)) if pr.verificando => {
                format!("{} {} ({:.0} %)", textos.t("asistente-comprobando"), pr.fichero, fr * 100.0)
            }
            Some((fr, pr, _)) => format!(
                "{} {} / {} ({:.0} %)",
                textos.t("asistente-bajando"),
                ia_descarga::tamano(pr.as_ref().map_or(0, |p| p.hecho)),
                ia_descarga::tamano(pr.as_ref().map_or(falta, |p| p.total)),
                fr * 100.0
            ),
            None if t.confirmando => format!(
                "{} {} → {}",
                textos.t("asistente-pregunta"),
                ia_descarga::tamano(falta),
                raiz_m.as_deref().map(|r| r.display().to_string()).unwrap_or_default()
            ),
            None => match &t.noticia {
                Some(n) if n.starts_with("asistente-") => textos.t(n),
                Some(n) => format!("⚠️ {n}"),
                None if poca_memoria() => textos.t("asistente-poca-memoria"),
                None => String::new(),
            },
        };
        p.texto_linea(&estado, x + pad, y, 12.0 * e, dentro, tema.apagado);
        y += 18.0 * e;
        if let Some((fr, _, _)) = &bajando {
            let barra = RectF { x: x + pad, y, ancho: dentro, alto: 6.0 * e };
            p.rellenar_redondeado(barra, 3.0 * e, tema.campo);
            p.rellenar_redondeado(RectF { ancho: dentro * fr.clamp(0.0, 1.0), ..barra }, 3.0 * e, tema.enviar);
        }
        y += 12.0 * e;

        // Los botones, a la derecha.
        let (principal, accion_p, secundario): (String, Accion, Option<(String, Accion)>) = if bajando.is_some() {
            (textos.t("asistente-cancelar"), Accion::Cancelar, None)
        } else if t.confirmando {
            (
                format!("{} ({})", textos.t("asistente-si-bajar"), ia_descarga::tamano(falta)),
                Accion::Confirmar,
                Some((textos.t("asistente-cancelar"), Accion::Cancelar)),
            )
        } else {
            (
                format!("{} ({})", textos.t("asistente-bajar"), ia_descarga::tamano(falta)),
                Accion::Bajar,
                None,
            )
        };
        let tam = 13.0 * e;
        let (w, _) = p.medir_texto(&principal, tam);
        let boton = RectF {
            x: x + ancho - pad - w - 28.0 * e,
            y,
            ancho: w + 28.0 * e,
            alto: 32.0 * e,
        };
        p.rellenar_redondeado(boton, 16.0 * e, tema.enviar);
        p.texto(&principal, boton.x + 14.0 * e, boton.y + 8.0 * e, tam, hex(0xffffff));
        t.botones.push((a_rect(boton), accion_p));
        if let Some((rotulo, acc)) = secundario {
            let (w2, _) = p.medir_texto(&rotulo, tam);
            let otro = RectF {
                x: boton.x - w2 - 36.0 * e,
                ancho: w2 + 28.0 * e,
                ..boton
            };
            p.trazar(otro, (1.0 * e).max(1.0), tema.isla_borde);
            p.texto(&rotulo, otro.x + 14.0 * e, otro.y + 8.0 * e, tam, tema.texto);
            t.botones.push((a_rect(otro), acc));
        }
    });
}

fn a_rect(r: RectF) -> Rect {
    Rect {
        x: r.x.round() as i32,
        y: r.y.round() as i32,
        ancho: r.ancho.max(0.0).round() as u32,
        alto: r.alto.max(0.0).round() as u32,
    }
}

/// **Un clic en la tarjeta o en la lista de `@`.** `true` si era suyo.
pub(super) fn tocar(ubicacion: &Ubicacion, a: &mut Abierto, l: Punto) -> bool {
    if tocar_lista(a, l) {
        return true;
    }
    if !a.ficha.es_asistente() {
        return false;
    }
    let Some(accion) = TARJETA.with_borrow(|t| t.botones.iter().find(|(r, _)| r.contiene(l)).map(|(_, a)| *a)) else {
        return false;
    };
    cumplir(ubicacion, a, accion);
    true
}

fn cumplir(ubicacion: &Ubicacion, a: &Abierto, accion: Accion) {
    match accion {
        Accion::Elegir { ligero } => {
            let id = if ligero { pixpin_ia::modelos::QWEN35_08B.id } else { pixpin_ia::modelos::QWEN35_2B.id };
            TARJETA.with_borrow_mut(|t| {
                t.ligero = Some(ligero);
                t.confirmando = false;
                t.noticia = None;
                t.listo = None;
                t.ajuste = None;
            });
            guardar_modelo(ubicacion, id);
        }
        Accion::Bajar => TARJETA.with_borrow_mut(|t| {
            t.confirmando = true;
            t.noticia = None;
        }),
        Accion::Confirmar => {
            let modelo = TARJETA.with_borrow_mut(|t| elegido(t, &a.raiz));
            guardar_modelo(ubicacion, modelo.id);
            if let Some(raiz) = ia_motor::raiz() {
                DESCARGA.with_borrow_mut(|d| {
                    if d.is_none() {
                        *d = Some(Descarga::lanzar(raiz, modelo));
                    }
                });
            }
            TARJETA.with_borrow_mut(|t| t.confirmando = false);
        }
        Accion::Cancelar => {
            DESCARGA.with_borrow(|d| {
                if let Some(d) = d {
                    d.cancelar();
                }
            });
            TARJETA.with_borrow_mut(|t| t.confirmando = false);
        }
    }
}

/// El modelo elegido, a los ajustes (`[asistente] modelo`). Si cambia, el
/// motor del otro se suelta.
fn guardar_modelo(ubicacion: &Ubicacion, id: &str) {
    let mut aj = pixpin_store::cargar(ubicacion).unwrap_or_default();
    if aj.asistente.modelo == id {
        return;
    }
    aj.asistente.modelo = id.to_string();
    if let Err(e) = pixpin_store::ajustes::guardar_conservando(ubicacion, &aj) {
        tracing::warn!(?e, "asistente: no se pudo guardar el modelo elegido");
    }
    ia_motor::soltar_todo();
}

// ---------------------------------------------------------------------------
// La lista de `@` mientras se escribe

const VK_TAB: u32 = 0x09;

/// La lista que toca ahora, si toca: donde esta el `@`, la rayita y las
/// opciones (el asistente fuera de su chat, y los lienzos y hojas).
fn menciones_ahora(a: &Abierto) -> Option<(usize, usize, Vec<ia_chat::Opcion>)> {
    let cursor = a.cursor.pos(&a.borrador);
    let (arroba, escrito) = ia_chat::consulta_de_mencion(&a.borrador, cursor)?;
    let cerrada = MENCIONES.with_borrow(|m| {
        m.cerrada_en
            .as_ref()
            .is_some_and(|(p, i)| *p == a.ficha.id && *i == arroba)
    });
    if cerrada {
        return None;
    }
    let opciones = ia_chat::opciones_de_mencion(&a.mensajes, &escrito, a.ficha.es_asistente());
    (!opciones.is_empty()).then_some((arroba, cursor, opciones))
}

/// Si la lista de `@` esta a la vista (y por tanto las flechas, Entrar,
/// Tab y Escape son suyas).
pub(super) fn menciones_abiertas(a: &Abierto) -> bool {
    menciones_ahora(a).is_some()
}

/// Escribe la opcion `cual` en la caja.
fn elegir_mencion(a: &mut Abierto, cual: usize) -> bool {
    let Some((arroba, cursor, opciones)) = menciones_ahora(a) else {
        return false;
    };
    let Some(o) = opciones.get(cual) else { return false };
    let fin = ia_chat::poner_mencion(&mut a.borrador, arroba, cursor, o);
    a.cursor.poner(&a.borrador, fin, false);
    MENCIONES.with_borrow_mut(|m| m.marcada = 0);
    a.colocado.borrow_mut().ancho = 0;
    true
}

/// Las teclas que se lleva la lista mientras se ve.
pub(super) fn tecla_de_la_lista(vk: u32) -> bool {
    matches!(vk, VK_ARRIBA | VK_ABAJO | VK_ENTRAR | VK_TAB | VK_ESCAPE)
}

/// **Una tecla con la lista abierta.** `true` si era suya: las flechas
/// mueven la marca, Entrar o Tab eligen (Entrar NO envia mientras se elige
/// un lienzo), Escape la cierra para ese `@`.
pub(super) fn tecla_de_menciones(a: &mut Abierto, vk: u32) -> bool {
    let Some((arroba, _, opciones)) = menciones_ahora(a) else {
        return false;
    };
    let n = opciones.len();
    match vk {
        VK_ARRIBA => MENCIONES.with_borrow_mut(|m| m.marcada = (m.marcada + n - 1) % n),
        VK_ABAJO => MENCIONES.with_borrow_mut(|m| m.marcada = (m.marcada + 1) % n),
        VK_ENTRAR | VK_TAB => {
            let cual = MENCIONES.with_borrow(|m| m.marcada.min(n - 1));
            elegir_mencion(a, cual);
        }
        VK_ESCAPE => MENCIONES.with_borrow_mut(|m| {
            m.cerrada_en = Some((a.ficha.id.clone(), arroba));
            m.marcada = 0;
        }),
        _ => return false,
    }
    true
}

/// **Un clic en la lista de `@`.** `true` si era suyo.
pub(super) fn tocar_lista(a: &mut Abierto, l: Punto) -> bool {
    if !menciones_abiertas(a) {
        return false;
    }
    let cual = MENCIONES.with_borrow(|m| m.botones.iter().find(|(r, _)| r.contiene(l)).map(|(_, i)| *i));
    match cual {
        Some(i) => elegir_mencion(a, i),
        None => false,
    }
}

/// **Pinta la lista de `@`** encima de la caja de escribir, pegada abajo a
/// la izquierda del historial, como la de cualquier mensajeria.
pub(super) fn pintar_menciones(p: &Pintor, d: &Disposicion, c: &Pinta, a: &Abierto) {
    let Some((_, _, opciones)) = menciones_ahora(a) else {
        MENCIONES.with_borrow_mut(|m| {
            m.botones.clear();
            m.marcada = 0;
        });
        return;
    };
    let (tema, e) = (c.tema, c.escala as f32 / 100.0);
    let area = area_del_historial(d, a, c.escala);
    let fila = 34.0 * e;
    let pad = 6.0 * e;
    let ancho = (300.0 * e).min(area.ancho as f32 - 24.0 * e).max(120.0);
    let alto = fila * opciones.len() as f32 + 2.0 * pad + 22.0 * e;
    let caja = RectF {
        x: area.x as f32 + 12.0 * e,
        y: area.abajo() as f32 - alto - 6.0 * e,
        ancho,
        alto,
    };
    MENCIONES.with_borrow_mut(|m| {
        m.marcada = m.marcada.min(opciones.len() - 1);
        m.botones.clear();
        p.rellenar_redondeado(caja, 12.0 * e, tema.burbuja_otra);
        p.trazar(caja, (1.0 * e).max(1.0), tema.isla_borde);
        p.texto_linea(
            &c.textos.t("asistente-mencionar"),
            caja.x + 12.0 * e,
            caja.y + pad,
            11.0 * e,
            ancho - 24.0 * e,
            tema.apagado,
        );
        let mut y = caja.y + pad + 22.0 * e;
        for (i, o) in opciones.iter().enumerate() {
            let r = RectF {
                x: caja.x + pad,
                y,
                ancho: ancho - 2.0 * pad,
                alto: fila,
            };
            if i == m.marcada {
                p.rellenar_redondeado(r, 8.0 * e, con_alfa(tema.enviar, 0.16));
            }
            let (_, h) = p.medir_texto(&o.rotulo, 14.0 * e);
            p.texto_linea(&o.rotulo, r.x + 8.0 * e, r.y + (fila - h) / 2.0, 14.0 * e, r.ancho - 16.0 * e, tema.texto);
            m.botones.push((a_rect(r), i));
            y += fila;
        }
    });
}

#[cfg(test)]
pub(super) fn lanzar_descarga_para_pruebas(d: Descarga) {
    DESCARGA.with_borrow_mut(|x| *x = Some(d));
}

#[cfg(test)]
pub(super) fn olvidar_para_pruebas() {
    SESIONES.with_borrow_mut(Vec::clear);
    DESCARGA.with_borrow_mut(|d| *d = None);
    TARJETA.with_borrow_mut(|t| *t = Tarjeta::default());
    ia_pensado::olvidar_para_pruebas();
}

#[cfg(test)]
pub(super) fn noticia_para_pruebas() -> Option<String> {
    TARJETA.with_borrow(|t| t.noticia.clone())
}

#[cfg(test)]
mod pruebas;
#[cfg(test)]
mod pruebas_pensado;
