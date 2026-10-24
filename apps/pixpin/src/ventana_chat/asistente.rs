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
//! - [`pregunta_pendiente`]/[`pintar_confirmacion`]/[`contestar`]: lo que
//!   borra o envia pide «Sí»/«No» en una tarjeta DENTRO de la burbuja
//!   (13-oct-2026); el turno espera en su hilo y la ventana sigue suelta.
//! - [`recetas`]: los pedidos guardados, como botones encima de la caja.
//! - La tarjeta de bajar tambien ofrece el modelo de mejor vista (Gemma 4
//!   E2B) si esta encendido en los ajustes y falta.
//!
//! Todo vive en el hilo de la ventana (`thread_local`): el bucle del chat es
//! uno y no hay que compartir nada con otros hilos salvo por los canales.
//! **Salvo al cerrarla** (24-oct-2026): los turnos que sigan en marcha se
//! van a un hilo de fondo que hace lo mismo sin ventana, y vuelven al
//! reabrirla; y cada turno queda apuntado en el disco por si PixPin se
//! cierra a medias ([`de_fondo`]).

use std::cell::RefCell;
use std::sync::Arc;
use std::time::{Duration, Instant};

use pixpin_ia::motor::Motor;
use pixpin_proyecto::cuaderno;

use super::*;
use crate::asistente::chat::{self as ia_chat, EnVivo};
use crate::asistente::descarga::{self as ia_descarga, AvisoDescarga, Descarga};
use crate::asistente::turno::{Aviso, Turno};
use crate::asistente::{
    al_lienzo, motor as ia_motor, nube as ia_nube, pensado as ia_pensado, turno as ia_turno,
};
use pixpin_ia::modelos::GEMMA4_E2B;
use pixpin_ia::preguntar::{Contestada, Pregunta};

/// Cada cuanto se escribe en el disco lo que va contestando (en memoria se
/// ve al momento). Un turno de minuto y medio no son cien escrituras.
const GUARDAR_CADA: Duration = Duration::from_secs(2);

/// Lo que se ve en la burbuja de la respuesta desde el primer instante,
/// antes de que llegue nada del modelo (regla del usuario: algo visible en
/// menos de medio segundo).
pub(super) use crate::asistente::rapido::ESTADO_INICIAL;

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
    /// Lo citado de los documentos del usuario: la respuesta lo apunta
    /// para su «Abrir fuente».
    citas: Vec<pixpin_ia::documentos::Cita>,
    /// Donde estan los datos (para dejar escrita una pregunta que caduca al
    /// cerrar el chat).
    raiz: std::path::PathBuf,
    /// La pregunta de confirmar que espera respuesta: por donde contestar y
    /// desde cuando espera.
    responder: Option<(std::sync::mpsc::Sender<bool>, Instant)>,
    /// La duda con opciones que espera (`preguntar_usuario`): la pregunta,
    /// por donde contestar, desde cuando y si se pulso «Otro…» (lo
    /// siguiente que se escriba es la respuesta).
    elegir: Option<Eleccion>,
    /// Alguna herramienta ya hizo algo (lo apunta tambien el disco): si
    /// PixPin se cierra a medias, no se repite solo ([`de_fondo`]).
    actuo: bool,
    /// La tarjeta flotante que lo cuenta mientras sigue con el chat cerrado.
    fuera: Option<de_fondo::Fuera>,
}

/// Una duda con opciones en el aire.
struct Eleccion {
    pregunta: Pregunta,
    responder: std::sync::mpsc::Sender<Contestada>,
    desde: Instant,
    otro: bool,
}

impl Sesion {
    /// **Contesta la duda con opciones** (al turno y en la burbuja). `false`
    /// si no habia ninguna.
    fn elegir(&mut self, c: Contestada) -> bool {
        let Some(e) = self.elegir.take() else {
            return false;
        };
        let recomendada = e
            .pregunta
            .opciones
            .first()
            .map(|o| o.etiqueta.as_str())
            .unwrap_or("");
        let respuesta = match &c {
            Contestada::Opcion(n) => e.pregunta.opciones.get(*n).map(|o| o.etiqueta.clone()),
            Contestada::Texto(t) => Some(t.clone()),
            Contestada::SinRespuesta => None,
        };
        self.vivo
            .contestada_con_opciones(&e.pregunta.texto, respuesta.as_deref(), recomendada);
        let _ = e.responder.send(c);
        true
    }

    /// Contesta la pregunta pendiente (al turno y en la burbuja).
    fn contestar(&mut self, c: ia_chat::Contestacion) -> bool {
        if let Some((tx, _)) = self.responder.take() {
            let _ = tx.send(c == ia_chat::Contestacion::Si);
        }
        self.vivo.contestar(c)
    }
}

/// **Si el chat se cierra con una pregunta en el aire, caduca**: el turno
/// recibe un no (su `Sender` muere aqui) y la nota del disco lo cuenta, para
/// que el historial no se quede con una pregunta sin contestar.
impl Drop for Sesion {
    fn drop(&mut self) {
        let duda = self.elegir(Contestada::SinRespuesta);
        if self.contestar(ia_chat::Contestacion::Caducada) || duda {
            let texto = self.vivo.mostrar();
            let carpeta = pixpin_proyecto::almacen::carpeta(&self.raiz, &self.proyecto);
            let _ = cuaderno::cambiar(&carpeta, &self.nota, |m| {
                m.texto = texto.clone();
                true
            });
        }
    }
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
    /// Si cada modelo esta listo, mirado hace poco (mirar el disco en cada
    /// fotograma no).
    listo: Vec<(Instant, &'static str, bool)>,
    /// El modelo de los ajustes y si se quiere la mejor vista, leidos hace
    /// poco (el TOML no se lee en cada fotograma).
    ajuste: Option<(Instant, String, bool)>,
    /// La tarjeta es la del modelo de mejor vista (el de siempre ya esta):
    /// encendida en los ajustes y sin bajar.
    vista: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Accion {
    Elegir {
        ligero: bool,
    },
    Bajar,
    Confirmar,
    Cancelar,
    /// «Ahora no» en la tarjeta de la mejor vista: apaga el ajuste.
    AhoraNo,
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
    SESIONES.with_borrow(|s| {
        s.iter()
            .any(|x| x.proyecto == proyecto && !x.turno.acabado())
    })
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
    (turnos || al_lienzo::hay_pedidos())
        .then_some(80)
        .or(bajando.then_some(250))
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
        Some((proyecto, cuando)) => {
            *proyecto != a.ficha.id || (menciona() && cuando.elapsed() >= REPRECARGAR)
        }
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
    let modelo =
        ia_motor::modelo_elegido(Some(aj.asistente.modelo.as_str()).filter(|m| !m.is_empty()));
    if let Some(m) = ia_motor::raiz().and_then(|raiz| ia_motor::del_proceso(&raiz, &modelo).ok()) {
        ia_motor::precargar(m);
    }
}

/// El motor elegido en Ajustes → Asistente: el local (si esta bajado) o la
/// nube con su clave, con el local de respaldo si se quiere y esta bajado.
/// El local de ESTE turno puede ser el de mejor vista si se pide mirar una
/// foto (`ia_motor::modelo_para_el_turno`). `pensar`: si la nube piensa
/// este turno (`ia_pensado::pensar`); el local lo recibe en la peticion.
pub(super) fn motor_de_los_ajustes(
    ubicacion: &Ubicacion,
    textos: &Catalogo,
    pregunta: &str,
    con_foto: bool,
    pensar: bool,
) -> Result<Arc<dyn Motor>, String> {
    let aj = pixpin_store::cargar(ubicacion).unwrap_or_default();
    let local = || {
        ia_motor::raiz().and_then(|raiz| {
            let modelo = ia_motor::modelo_para_el_turno(&aj.asistente, &raiz, pregunta, con_foto);
            ia_motor::del_proceso(&raiz, &modelo).ok()
        })
    };
    match ia_nube::eleccion(&aj.asistente) {
        ia_nube::Eleccion::Nube(p) => ia_nube::motor(
            p,
            if aj.asistente.respaldo_local {
                local()
            } else {
                None
            },
            pensar,
        ),
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
pub(super) fn al_enviar(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    textos: &Catalogo,
) -> Option<String> {
    let nueva = a.mensajes.last()?;
    let para_el_asistente = ia_chat::pregunta_de(nueva, a.ficha.es_asistente());
    // Si espera una duda con opciones, lo escrito es la respuesta.
    let escrito = para_el_asistente
        .clone()
        .unwrap_or_else(|| nueva.texto.clone());
    if contestar_duda_escribiendo(a, &escrito, para_el_asistente.is_some()) {
        return None;
    }
    let pregunta = para_el_asistente?;
    // **Escribirle mientras trabaja** (25-oct-2026, «como en Claude Code»):
    // lo escrito va al turno en marcha, que lo lee antes de su siguiente paso
    // y corrige, anade o para. El mensaje ya esta en el chat.
    if ocupado(&a.ficha.id) {
        let llego = SESIONES.with_borrow(|s| {
            s.iter()
                .find(|x| x.proyecto == a.ficha.id && !x.turno.acabado())
                .map(|x| x.turno.anadir(pregunta.clone()))
                .is_some()
        });
        return (!llego).then(|| textos.t("asistente-ocupado"));
    }
    let (motor, pensar, ventana) = match motor_para(ubicacion, textos, &a.mensajes, &pregunta) {
        Ok(m) => m,
        Err(aviso) => return Some(aviso),
    };
    match lanzar_pensando(ubicacion, a, motor, pregunta, pensar, ventana) {
        Ok(()) => None,
        Err(e) => {
            tracing::warn!(?e, "asistente: no se pudo empezar a contestar");
            Some(textos.t("asistente-fallo"))
        }
    }
}

/// **El motor de un pedido y como pensarlo**: el de los ajustes, si piensa
/// y la ventana del modelo. Lo comparten el chat abierto y lo que se retoma
/// al arrancar ([`de_fondo`]); `Err` es el aviso para el usuario.
fn motor_para(
    ubicacion: &Ubicacion,
    textos: &Catalogo,
    mensajes: &[cuaderno::Mensaje],
    pregunta: &str,
) -> Result<(Arc<dyn Motor>, bool, u32), String> {
    let con_foto = ia_chat::foto_reciente(ia_chat::desde_el_corte(mensajes)).is_some();
    // Pensar (15-oct): «Siempre», «Nunca» o, en automatico, solo si la
    // tarea es compleja, con el mismo criterio para el local y la nube
    // (`pixpin_ia::complejidad`). La ventana: el local segun su modelo; la
    // nube tiene de sobra, pero la hoja y el reinicio valen igual.
    let aj = pixpin_store::cargar(ubicacion)
        .unwrap_or_default()
        .asistente;
    let pensar = ia_pensado::pensar(aj.pensar, pregunta);
    let ventana = match ia_nube::eleccion(&aj) {
        ia_nube::Eleccion::Nube(_) => ia_pensado::VENTANA_NUBE,
        ia_nube::Eleccion::Local => ia_pensado::ventana_local(),
    };
    // Sin modelo (o sin clave) las ordenes simples igual se hacen: van por
    // el camino rapido (`asistente::rapido`, el mismo que Flow Launcher y las
    // automatizaciones). Lo demas dira el aviso.
    let motor = crate::asistente::rapido::motor_o_sin(
        motor_de_los_ajustes(ubicacion, textos, pregunta, con_foto, pensar),
        pregunta,
    )?;
    Ok((motor, pensar, ventana))
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
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let (nota, sesion) = lanzar_en(
        ubicacion.raiz(),
        &a.ficha.id,
        a.ficha.es_asistente(),
        &a.mensajes,
        numero,
        motor,
        pregunta,
        pensando,
        0,
    )?;
    a.vistas.push(None);
    a.mensajes.push(nota);
    a.colocado.borrow_mut().ancho = 0;
    SESIONES.with_borrow_mut(|s| s.push(sesion));
    Ok(())
}

/// **El turno sin la ventana**: lo de [`lanzar_con`] que no toca la
/// conversacion abierta. `mensajes` acaba en la pregunta; `numero` es el de
/// la nota de la respuesta. Escribe la nota, apunta el turno en el disco
/// ([`de_fondo::apuntar`]) y lo lanza; quien llama guarda la `Sesion` (el
/// chat en su hilo, o el hilo de fondo al retomar lo cortado al arrancar).
/// `intentos`: las veces que ya se retomo este pedido.
#[allow(clippy::too_many_arguments)]
fn lanzar_en(
    raiz: &std::path::Path,
    proyecto: &str,
    es_asistente: bool,
    mensajes: &[cuaderno::Mensaje],
    numero: i64,
    motor: Arc<dyn Motor>,
    pregunta: String,
    pensando: Option<(bool, u32)>,
    intentos: u32,
) -> std::io::Result<(cuaderno::Mensaje, Sesion)> {
    let raiz = raiz.to_path_buf();
    let proyecto = proyecto.to_string();
    let n = mensajes.len();
    let (anteriores, nueva) = mensajes.split_at(n.saturating_sub(1));
    let pregunta_id = nueva.first().map(|m| m.id.clone());
    // Tras «Conversación nueva», lo de antes no se le manda (ni sus fotos,
    // ni sus adjuntos, ni su hoja de trabajo).
    let anteriores = ia_chat::desde_el_corte(anteriores);
    let vivos = ia_chat::desde_el_corte(mensajes);
    // La foto puede ser la misma que se acaba de mandar.
    let foto = ia_chat::foto_reciente(vivos).and_then(|m| ruta_del_mensaje(&raiz, &proyecto, m));
    let adjuntos = ia_chat::adjuntos(vivos, |m| {
        ruta_del_mensaje(&raiz, &proyecto, m).filter(|r| r.is_file())
    });
    // `@lienzo:<nombre>`: donde dibujar si no hay un lienzo abierto.
    let (para_el_modelo, destino) = ia_chat::resolver_lienzo(&pregunta, mensajes);
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
            (
                ia_turno::historial_para_el_modelo(&conversacion, &para_el_modelo, None),
                None,
            )
        }
    };

    // La burbuja de la respuesta nace ya, con el cursor: el usuario ve que
    // se le esta contestando aunque el modelo tarde en cargar.
    let vivo = EnVivo {
        estado: Some(ESTADO_INICIAL.into()),
        ..Default::default()
    };
    // En otro chat la respuesta cuelga de la pregunta, para que se sepa a
    // que contesta entre los mensajes de los demas.
    let responde_a = if es_asistente {
        None
    } else {
        pregunta_id.clone()
    };
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
    // Apuntado ANTES de lanzarlo: si PixPin se cierra en mitad del turno,
    // al volver a abrirla se sabe que quedo a medias.
    de_fondo::apuntar(
        &raiz,
        de_fondo::EnMarcha {
            proyecto: proyecto.clone(),
            nota: id.clone(),
            pregunta: pregunta_id.unwrap_or_default(),
            asistente: es_asistente,
            desde: pixpin_shell::entorno::ahora_utc_ms(),
            actuo: false,
            intentos,
        },
    );

    let turno = Turno::lanzar_con_memoria(
        motor,
        proyecto.clone(),
        historial,
        adjuntos,
        foto,
        destino,
        herramientas,
        memoria,
        Some(raiz.clone()),
    );
    let sesion = Sesion {
        proyecto,
        nota: id,
        codigo,
        turno,
        vivo,
        guardado: Instant::now(),
        registro: None,
        lienzo: None,
        abrir,
        raiz,
        responder: None,
        elegir: None,
        citas: Vec::new(),
        actuo: false,
        fuera: None,
    };
    Ok((nota, sesion))
}

/// **«Conversación nueva»** (17-oct-2026, el usuario: «que pueda borrar el
/// chat y crear uno nuevo de forma que no tenga el contexto de todo el chat
/// cada vez»): una nota del asistente que marca el corte
/// ([`ia_chat::NUEVA_CONVERSACION`]). Lo de arriba sigue a la vista, pero el
/// siguiente turno empieza de cero: sin historial, sin la hoja de trabajo y
/// sin las fotos ni los adjuntos de antes. Con un turno en marcha no se
/// corta (devuelve el aviso); si ya se acaba de cortar, no se repite.
pub(super) fn nueva_conversacion(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    textos: &Catalogo,
) -> Option<String> {
    if ocupado(&a.ficha.id) {
        return Some(textos.t("asistente-ocupado"));
    }
    // Dos cortes seguidos no dicen nada nuevo.
    if a.mensajes
        .iter()
        .rev()
        .find(|m| !cuaderno::es_de_sistema(m))
        .is_some_and(ia_chat::es_corte)
    {
        return None;
    }
    let raiz = ubicacion.raiz().to_path_buf();
    // Lo duradero de la que se cierra, a la memoria de largo plazo (con la
    // nube y su clave; sin ellas no se hace nada), en segundo plano.
    let cerrada: Vec<(bool, String)> = ia_chat::desde_el_corte(&a.mensajes)
        .iter()
        .filter(|m| !cuaderno::es_de_sistema(m) && !m.texto.trim().is_empty())
        .map(|m| (!ia_chat::es_del_asistente(m), m.texto.clone()))
        .collect();
    if !cerrada.is_empty() {
        crate::asistente::memoria::al_cerrar(&raiz, cerrada);
    }
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let hecho = escribir_nota(
        &raiz,
        &a.ficha.id,
        ia_chat::APARATO,
        ia_chat::TEXTO_NUEVA_CONVERSACION,
        descripcion::hora_sin_repetir(),
        numero,
        None,
    )
    .and_then(|mut nota| {
        nota.resto.insert(
            ia_chat::NUEVA_CONVERSACION.into(),
            serde_json::Value::Bool(true),
        );
        cuaderno::reemplazar(
            &pixpin_proyecto::almacen::carpeta(&raiz, &a.ficha.id),
            &nota,
        )?;
        Ok(nota)
    });
    match hecho {
        Ok(nota) => {
            a.vistas.push(None);
            a.mensajes.push(nota);
            a.colocado.borrow_mut().ancho = 0;
            a.scroll = None;
            None
        }
        Err(e) => {
            tracing::warn!(?e, "asistente: no se pudo empezar una conversacion nueva");
            Some(textos.t("chat-no-se-pudo"))
        }
    }
}

/// **El lienzo que pidio abrir al terminar** («dibuja… y ábrelo»), por su
/// posicion en la conversacion abierta. Se entrega una vez.
pub(super) fn lienzo_por_abrir(a: &Abierto) -> Option<usize> {
    let (proyecto, referencia) = POR_ABRIR.with_borrow(Clone::clone)?;
    if proyecto != a.ficha.id {
        return None;
    }
    POR_ABRIR.with_borrow_mut(|p| *p = None);
    a.mensajes.iter().rposition(|m| {
        ia_chat::es_lienzo(m) && m.referencia.as_deref() == Some(referencia.as_str())
    })
}

/// El lienzo de «Abrir lienzo» de la respuesta `i`, si lo lleva.
pub(super) fn lienzo_de(a: &Abierto, i: usize) -> Option<usize> {
    ia_chat::lienzo_de_la_respuesta(a.mensajes.get(i)?, &a.mensajes)
}

/// La fuente de «Abrir fuente» de la respuesta `i`, si cito alguna.
pub(super) fn fuente_de(a: &Abierto, i: usize) -> Option<ia_chat::FuenteCitada> {
    ia_chat::fuente_de_la_respuesta(a.mensajes.get(i)?)
}

/// **Abre una fuente citada**: su pedido `abrir`, a esta misma PixPin, como
/// si viniera de Flow Launcher (lo atiende `pedidos.rs` en el bucle
/// principal). En su hilo: la ventana del chat no espera a nadie.
pub(super) fn abrir_fuente(f: &ia_chat::FuenteCitada) {
    let json = f.pedido.to_string();
    let _ = std::thread::Builder::new()
        .name("abrir-fuente".into())
        .spawn(move || {
            let r = pixpin_shell::mensajero::enviar_pedido(&json);
            if r != 1 {
                tracing::warn!(
                    respuesta = r,
                    "asistente: no se pudo abrir la fuente citada"
                );
            }
        });
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

// ---------------------------------------------------------------------------
// La tarjeta de confirmar, dentro de la burbuja (13-oct-2026)

/// La pregunta que espera respuesta en la burbuja del mensaje `i`, si la hay.
pub(super) fn pregunta_pendiente(a: &Abierto, i: usize) -> Option<String> {
    let m = a.mensajes.get(i)?;
    SESIONES
        .with_borrow(|s| {
            s.iter()
                .find(|s| s.proyecto == a.ficha.id && s.nota == m.id && s.responder.is_some())
                .and_then(|s| s.vivo.pendiente.clone())
        })
        // O la de un turno sin ventana (Flow, una automatizacion), que escribe
        // en este mismo chat desde su hilo.
        .or_else(|| crate::asistente::permiso::pregunta(&a.ficha.id, &m.id))
}

/// **«Sí» o «No» en la tarjeta** de la burbuja `i`. La pregunta contestada
/// queda escrita en la burbuja; el turno sigue en su hilo.
pub(super) fn contestar(a: &mut Abierto, i: usize, si: bool) -> bool {
    let Some(id) = a.mensajes.get(i).map(|m| m.id.clone()) else {
        return false;
    };
    let texto = SESIONES.with_borrow_mut(|s| {
        let s = s
            .iter_mut()
            .find(|s| s.proyecto == a.ficha.id && s.nota == id)?;
        let c = if si {
            ia_chat::Contestacion::Si
        } else {
            ia_chat::Contestacion::No
        };
        s.contestar(c).then(|| s.vivo.mostrar())
    });
    let Some(texto) = texto else {
        // La de un turno sin ventana: contesta alli, y su hilo reescribe la
        // nota y avisa al chat para que la relea.
        return crate::asistente::permiso::contestar(&a.ficha.id, &id, si);
    };
    if let Some(m) = a.mensajes.get_mut(i) {
        m.texto = texto;
    }
    a.colocado.borrow_mut().ancho = 0;
    true
}

/// **La duda con opciones** que espera en la burbuja `i`, si la hay.
pub(super) fn eleccion_pendiente(a: &Abierto, i: usize) -> Option<(Pregunta, bool)> {
    let m = a.mensajes.get(i)?;
    SESIONES.with_borrow(|s| {
        s.iter()
            .find(|s| s.proyecto == a.ficha.id && s.nota == m.id)
            .and_then(|s| s.elegir.as_ref().map(|e| (e.pregunta.clone(), e.otro)))
    })
}

/// **Un boton de la tarjeta de opciones** de la burbuja `i`: `Some(n)` la
/// opcion `n`; `None`, «Otro…» (lo siguiente que escriba es la respuesta;
/// devuelve el aviso que se lo dice).
pub(super) fn elegir(
    a: &mut Abierto,
    i: usize,
    opcion: Option<usize>,
    textos: &Catalogo,
) -> Option<String> {
    let id = a.mensajes.get(i).map(|m| m.id.clone())?;
    let texto = SESIONES.with_borrow_mut(|s| {
        let s = s
            .iter_mut()
            .find(|s| s.proyecto == a.ficha.id && s.nota == id)?;
        match opcion {
            Some(n) => s.elegir(Contestada::Opcion(n)).then(|| s.vivo.mostrar()),
            None => {
                if let Some(e) = s.elegir.as_mut() {
                    e.otro = true;
                }
                None
            }
        }
    });
    match texto {
        Some(texto) => {
            if let Some(m) = a.mensajes.get_mut(i) {
                m.texto = texto;
            }
            a.colocado.borrow_mut().ancho = 0;
            None
        }
        None if opcion.is_none() => Some(textos.t("asistente-otro-escribe")),
        None => None,
    }
}

/// **Lo escrito contesta la duda** (al enviar): si este chat tiene una duda
/// con opciones esperando y lo enviado es para el asistente (o se pulso
/// «Otro…»), es la respuesta y no un pedido nuevo. Devuelve si la contesto.
fn contestar_duda_escribiendo(a: &mut Abierto, texto: &str, para_el_asistente: bool) -> bool {
    let proyecto = a.ficha.id.clone();
    let hecho = SESIONES.with_borrow_mut(|s| {
        let s = s.iter_mut().find(|s| {
            s.proyecto == proyecto
                && s.elegir
                    .as_ref()
                    .is_some_and(|e| e.otro || para_el_asistente)
        })?;
        s.elegir(Contestada::Texto(texto.trim().to_string()));
        Some((s.nota.clone(), s.vivo.mostrar()))
    });
    let Some((nota, mostrar)) = hecho else {
        return false;
    };
    if let Some(m) = a.mensajes.iter_mut().find(|m| m.id == nota) {
        m.texto = mostrar;
    }
    a.colocado.borrow_mut().ancho = 0;
    true
}

/// Medidas de la tarjeta de opciones (al 100 %).
const ELEGIR_TAM: f32 = 13.0;
const ELEGIR_DESC_TAM: f32 = 11.5;
const ELEGIR_AIRE: f32 = 10.0;
const ELEGIR_HUECO: f32 = 6.0;
const ELEGIR_RELLENO: f32 = 8.0;
const ELEGIR_OTRO: f32 = 28.0;

/// Lo que mide cada opcion (boton con su etiqueta y la descripcion debajo).
fn alto_opcion(p: &Pintor, etiqueta: &str, descripcion: &str, ancho: f32, e: f32) -> f32 {
    let dentro = ancho - 2.0 * (ELEGIR_AIRE + ELEGIR_RELLENO) * e;
    let (_, h1) = p.medir_texto_ajustado(etiqueta, ELEGIR_TAM * e, dentro);
    let h2 = if descripcion.trim().is_empty() {
        0.0
    } else {
        2.0 * e
            + p.medir_texto_ajustado(descripcion, ELEGIR_DESC_TAM * e, dentro)
                .1
    };
    2.0 * ELEGIR_RELLENO * e + h1 + h2
}

/// Lo que mide la tarjeta de opciones con ese ancho.
pub(super) fn alto_eleccion(p: &Pintor, pregunta: &Pregunta, ancho: f32, e: f32) -> f32 {
    let (_, alto_texto) = p.medir_texto_ajustado(
        &format!("❓ {}", pregunta.texto),
        ELEGIR_TAM * e,
        ancho - 2.0 * ELEGIR_AIRE * e,
    );
    let opciones: f32 = pregunta
        .opciones
        .iter()
        .map(|o| alto_opcion(p, &o.etiqueta, &o.descripcion, ancho, e) + ELEGIR_HUECO * e)
        .sum();
    ELEGIR_AIRE * e + alto_texto + 8.0 * e + opciones + ELEGIR_OTRO * e + ELEGIR_AIRE * e
}

/// **Pinta la tarjeta de opciones**: la pregunta, un boton por opcion (la
/// primera, la recomendada, rellena) y «Otro…». Apunta las zonas:
/// `Some(n)` cada opcion, `None` «Otro…».
pub(super) fn pintar_eleccion(
    p: &Pintor,
    caja: RectF,
    pregunta: &Pregunta,
    otro: bool,
    tema: &Tema,
    textos: &Catalogo,
    e: f32,
) -> Vec<(Rect, Option<usize>)> {
    let azul = tema.enviar;
    p.rellenar_redondeado(caja, 10.0 * e, con_alfa(azul, 0.08));
    let aire = ELEGIR_AIRE * e;
    let texto = format!("❓ {}", pregunta.texto);
    let (_, alto_texto) = p.medir_texto_ajustado(&texto, ELEGIR_TAM * e, caja.ancho - 2.0 * aire);
    p.texto_ajustado(
        &texto,
        caja.x + aire,
        caja.y + aire,
        ELEGIR_TAM * e,
        caja.ancho - 2.0 * aire,
        tema.texto,
    );
    let mut y = caja.y + aire + alto_texto + 8.0 * e;
    let mut zonas = Vec::new();
    let relleno = ELEGIR_RELLENO * e;
    for (n, o) in pregunta.opciones.iter().enumerate() {
        let alto = alto_opcion(p, &o.etiqueta, &o.descripcion, caja.ancho, e);
        let b = RectF {
            x: caja.x + aire,
            y,
            ancho: caja.ancho - 2.0 * aire,
            alto,
        };
        let dentro = b.ancho - 2.0 * relleno;
        let (color_texto, color_desc) = if n == 0 {
            p.rellenar_redondeado(b, 8.0 * e, azul);
            (hex(0xffffff), con_alfa(hex(0xffffff), 0.85))
        } else {
            p.rellenar_redondeado(b, 8.0 * e, azul);
            let filete = (1.5 * e).max(1.0);
            p.rellenar_redondeado(encoger(b, filete), 8.0 * e - filete, tema.burbuja_otra);
            (azul, tema.texto)
        };
        let etiqueta = if n == 0 {
            format!("{} · {}", o.etiqueta, textos.t("asistente-recomendada"))
        } else {
            o.etiqueta.clone()
        };
        let (_, h1) = p.medir_texto_ajustado(&etiqueta, ELEGIR_TAM * e, dentro);
        p.texto_ajustado(
            &etiqueta,
            b.x + relleno,
            b.y + relleno,
            ELEGIR_TAM * e,
            dentro,
            color_texto,
        );
        if !o.descripcion.trim().is_empty() {
            p.texto_ajustado(
                &o.descripcion,
                b.x + relleno,
                b.y + relleno + h1 + 2.0 * e,
                ELEGIR_DESC_TAM * e,
                dentro,
                color_desc,
            );
        }
        zonas.push((a_rect(b), Some(n)));
        y += alto + ELEGIR_HUECO * e;
    }
    // «Otro…»: pulsado, dice que se escriba abajo.
    let rotulo = if otro {
        textos.t("asistente-otro-escribe")
    } else {
        textos.t("asistente-otro")
    };
    let tam = ELEGIR_TAM * e;
    let (w, h) = p.medir_texto(&rotulo, tam);
    let b = RectF {
        x: caja.x + aire,
        y,
        ancho: (w + 24.0 * e).min(caja.ancho - 2.0 * aire),
        alto: ELEGIR_OTRO * e,
    };
    p.texto(&rotulo, b.x + 12.0 * e, b.y + (b.alto - h) / 2.0, tam, azul);
    zonas.push((a_rect(b), None));
    zonas
}

/// Medidas de la tarjeta (al 100 %).
const CONFIRMAR_TAM: f32 = 13.0;
const CONFIRMAR_AIRE: f32 = 10.0;
const CONFIRMAR_BOTON: f32 = 30.0;
/// Lo menos ancha que se hace, para que quepan los dos botones y se lea.
pub(super) const CONFIRMAR_ANCHO_MIN: f32 = 260.0;

/// Lo que mide la tarjeta de confirmar con ese ancho.
pub(super) fn alto_confirmacion(p: &Pintor, pregunta: &str, ancho: f32, e: f32) -> f32 {
    let (_, alto_texto) = p.medir_texto_ajustado(
        &format!("⚠️ {pregunta}"),
        CONFIRMAR_TAM * e,
        ancho - 2.0 * CONFIRMAR_AIRE * e,
    );
    CONFIRMAR_AIRE * e + alto_texto + 8.0 * e + CONFIRMAR_BOTON * e + CONFIRMAR_AIRE * e
}

/// **Pinta la tarjeta**: lo que va a hacer y, debajo a la derecha, «No»
/// (relleno: es lo de por omision, como en el cuadro de antes) y «Sí» (con
/// solo el borde, en rojo: borra o envia). Apunta sus dos zonas.
pub(super) fn pintar_confirmacion(
    p: &Pintor,
    caja: RectF,
    pregunta: &str,
    tema: &Tema,
    textos: &Catalogo,
    e: f32,
) -> Vec<(Rect, bool)> {
    let rojo = hex(0xe5534b);
    p.rellenar_redondeado(caja, 10.0 * e, con_alfa(rojo, 0.10));
    let aire = CONFIRMAR_AIRE * e;
    let texto = format!("⚠️ {pregunta}");
    p.texto_ajustado(
        &texto,
        caja.x + aire,
        caja.y + aire,
        CONFIRMAR_TAM * e,
        caja.ancho - 2.0 * aire,
        tema.texto,
    );
    let tam = CONFIRMAR_TAM * e;
    let alto_b = CONFIRMAR_BOTON * e;
    let y = caja.y + caja.alto - aire - alto_b;
    let no = textos.t("asistente-confirmar-no");
    let si = textos.t("asistente-confirmar-si");
    let (wno, hno) = p.medir_texto(&no, tam);
    let (wsi, _) = p.medir_texto(&si, tam);
    let b_no = RectF {
        x: caja.x + caja.ancho - aire - wno - 32.0 * e,
        y,
        ancho: wno + 32.0 * e,
        alto: alto_b,
    };
    let b_si = RectF {
        x: b_no.x - 8.0 * e - wsi - 32.0 * e,
        y,
        ancho: wsi + 32.0 * e,
        alto: alto_b,
    };
    p.rellenar_redondeado(b_no, alto_b / 2.0, tema.enviar);
    p.texto(
        &no,
        b_no.x + 16.0 * e,
        y + (alto_b - hno) / 2.0,
        tam,
        hex(0xffffff),
    );
    // El borde rojo redondeado: el rojo entero y, encima, el fondo un poco mas chico.
    p.rellenar_redondeado(b_si, alto_b / 2.0, rojo);
    let filete = (1.5 * e).max(1.0);
    p.rellenar_redondeado(
        encoger(b_si, filete),
        alto_b / 2.0 - filete,
        tema.burbuja_otra,
    );
    p.texto(&si, b_si.x + 16.0 * e, y + (alto_b - hno) / 2.0, tam, rojo);
    vec![(a_rect(b_si), true), (a_rect(b_no), false)]
}

/// **Una vez por vuelta del bucle.** Devuelve si hay que repintar y un
/// aviso, si lo hay.
pub(super) fn latido(
    ubicacion: &Ubicacion,
    mut abierto: Option<&mut Abierto>,
) -> (bool, Option<String>) {
    let mut cambio = false;
    let mut aviso = None;

    // 0. Precargar el modelo local al abrir el chat «Asistente» o al escribir
    //    `@asistente` en otro: la primera pregunta ya no espera la carga.
    if let Some(a) = abierto.as_deref() {
        precargar_si_toca(ubicacion, a);
    }

    // 1. Lo que hay que meter en un lienzo, si no hay ninguno abierto (si lo
    //    hay, lo atiende su bucle) o si es para otro lienzo del chat que no
    //    es el abierto (17-oct, `al_lienzo::es_para_el_editor`).
    {
        for p in al_lienzo::tomar_para_el_chat() {
            // Dibujar, editar o resumir (`pixpin_ia::edicion`), sin abrirlo.
            let r = trabajar_en_el_lienzo_del_chat(
                ubicacion,
                abierto.as_deref_mut(),
                &p.proyecto,
                p.destino.as_deref(),
                &p.trabajo,
                &p.documento,
                p.marco,
                p.hoja.as_ref(),
                &p.imagenes,
                p.junto_a,
            );
            cambio = true;
            let r = match r {
                Ok((respuesta, referencia)) => {
                    if let Some(referencia) = referencia {
                        SESIONES.with_borrow_mut(|s| {
                            if let Some(s) = s.iter_mut().find(|s| s.proyecto == p.proyecto) {
                                s.lienzo = Some(referencia);
                            }
                        });
                    }
                    Ok(respuesta)
                }
                Err(e) => Err(e),
            };
            p.responder_con(r);
        }
    }

    // 2. Los turnos en marcha.
    let raiz = ubicacion.raiz().to_path_buf();
    let mut leidos: Vec<(String, ia_turno::Adjunto, String)> = Vec::new();
    SESIONES.with_borrow_mut(|sesiones| {
        for s in sesiones.iter_mut() {
            let mut avisos = s.turno.recibir();
            // Una pregunta sin contestar a tiempo caduca: es un no.
            let caduca = s
                .responder
                .as_ref()
                .is_some_and(|(_, desde)| desde.elapsed() >= ia_chat::CADUCA_LA_PREGUNTA);
            let mut contestada = false;
            if caduca {
                contestada = s.contestar(ia_chat::Contestacion::Caducada);
            }
            // La duda sin contestar a tiempo: sigue con la recomendada.
            if s.elegir
                .as_ref()
                .is_some_and(|e| e.desde.elapsed() >= ia_chat::CADUCA_LA_PREGUNTA)
            {
                contestada |= s.elegir(Contestada::SinRespuesta);
            }
            if avisos.is_empty() && !contestada {
                continue;
            }
            let mut acabo = false;
            for av in avisos.drain(..) {
                match av {
                    Aviso::Evento(e) => {
                        // La primera herramienta que hace algo: ya no se
                        // repite solo si PixPin se cierra a medias.
                        let actua =
                            matches!(e, pixpin_ia::agente::Evento::Resultado { ok: true, .. });
                        if actua && !s.actuo {
                            s.actuo = true;
                            de_fondo::marcar_que_actuo(&s.raiz, &s.nota);
                        }
                        s.vivo.aplicar(&e)
                    }
                    Aviso::TextoLeido { de, texto } => leidos.push((s.proyecto.clone(), de, texto)),
                    Aviso::Confirmar {
                        pregunta,
                        responder,
                    } => {
                        s.vivo.preguntar(&pregunta);
                        de_fondo::al_preguntar(s, &pregunta);
                        s.responder = Some((responder, Instant::now()));
                        // Que se vea ya en el disco tambien.
                        s.guardado = Instant::now() - GUARDAR_CADA;
                    }
                    Aviso::Preguntar {
                        pregunta,
                        responder,
                    } => {
                        // Una anterior sin contestar (no deberia: el turno
                        // espera) se da por no contestada.
                        s.elegir(Contestada::SinRespuesta);
                        s.vivo.preguntar_con_opciones();
                        de_fondo::al_preguntar(s, &pregunta.texto);
                        s.elegir = Some(Eleccion {
                            pregunta,
                            responder,
                            desde: Instant::now(),
                            otro: false,
                        });
                        s.guardado = Instant::now() - GUARDAR_CADA;
                    }
                    Aviso::Citas(c) => s.citas = c,
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
            // sale su «Abrir lienzo». Y lo que cito de los documentos: su
            // «Abrir fuente».
            let lienzo = s.lienzo.clone().filter(|_| acabo);
            let fuentes = if acabo {
                ia_chat::fuentes_para_la_nota(&s.citas, &texto)
            } else {
                None
            };
            let apuntar = |m: &mut cuaderno::Mensaje| {
                if let Some(r) = &lienzo {
                    m.resto.insert(
                        ia_chat::LIENZO_DE_LA_RESPUESTA.into(),
                        serde_json::Value::String(r.clone()),
                    );
                }
                if let Some(f) = &fuentes {
                    m.resto
                        .insert(ia_chat::FUENTES_DE_LA_RESPUESTA.into(), f.clone());
                }
            };
            if let Some(a) = abierto.as_deref_mut().filter(|a| a.ficha.id == s.proyecto)
                && let Some(m) = a.mensajes.iter_mut().find(|m| m.id == s.nota)
                && (m.texto != texto || lienzo.is_some() || fuentes.is_some())
            {
                m.texto = texto.clone();
                apuntar(m);
                a.colocado.borrow_mut().ancho = 0;
                cambio = true;
            }
            if acabo
                && s.abrir
                && let Some(r) = &lienzo
            {
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
                    // Ya no se lee sola al acabar (pedido del usuario,
                    // 17-oct): queda el altavoz de la burbuja.
                    let resumen: String = texto.chars().take(120).collect();
                    let _ = subir_en_la_lista(
                        &raiz,
                        &s.proyecto,
                        pixpin_shell::entorno::ahora_utc_ms(),
                        &resumen,
                    );
                    // Ya no esta a medias; y si seguia con el chat cerrado,
                    // su tarjeta lo cuenta con «Abrir».
                    de_fondo::al_acabar(s, &texto);
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
            t.listo.clear();
            t.confirmando = false;
            t.noticia = match &fin {
                AvisoDescarga::Cancelada => Some("asistente-cancelada".into()),
                AvisoDescarga::Fallo(m) => Some(m.clone()),
                _ => None,
            };
        });
        if fin == AvisoDescarga::Lista && modelo.id == GEMMA4_E2B.id {
            // El de mejor vista NO se precarga: echaria al de siempre de la
            // memoria para nada. Se carga con la primera foto que mirar.
            aviso = Some("asistente-vista-lista".to_string());
        } else if fin == AvisoDescarga::Lista
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
pub(super) fn dejar_texto_leido(
    raiz: &std::path::Path,
    abierto: Option<&mut Abierto>,
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
    if let Some(a) = abierto.filter(|a| a.ficha.id == proyecto) {
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
    abierto: Option<&mut Abierto>,
    proyecto: &str,
    destino: Option<&str>,
    documento: &str,
) -> Result<(String, String), String> {
    let (r, referencia) = trabajar_en_el_lienzo_del_chat(
        ubicacion,
        abierto,
        proyecto,
        destino,
        &al_lienzo::Trabajo::Dibujar,
        documento,
        None,
        None,
        &[],
        None,
    )?;
    Ok((r.texto, referencia.unwrap_or_default()))
}

/// **Lo que pide el asistente a un lienzo del chat que no esta abierto**:
/// dibujar (`documento`), editar lo que hay o resumirlo (`pixpin_ia::edicion`).
/// El mismo lienzo que [`meter_en_el_lienzo_del_chat`]; resumir no lo crea si
/// aun no existe. Devuelve la respuesta y la referencia del lienzo (si hay).
pub(super) fn trabajar_en_el_lienzo_del_chat(
    ubicacion: &Ubicacion,
    abierto: Option<&mut Abierto>,
    proyecto: &str,
    destino: Option<&str>,
    trabajo: &al_lienzo::Trabajo,
    documento: &str,
    marco: Option<(f32, f32)>,
    // Una zona de una hoja (`hoja_a_lienzo`): su sitio en la hoja y sus
    // fotos (a `imagenes/` del chat, como una imagen pegada).
    en_hoja: Option<&al_lienzo::EnHoja>,
    fotos_de_hoja: &[(u64, pixpin_codec::ImagenRgba)],
    // La imagen del lienzo de la que sale lo dibujado: va a su lado.
    junto_a: Option<(f32, f32, f32, f32)>,
) -> Result<(al_lienzo::Respuesta, Option<String>), String> {
    let raiz = ubicacion.raiz();
    let es_el_abierto = abierto.as_deref().is_some_and(|a| a.ficha.id == proyecto);
    let del_disco = || {
        cuaderno::Cuaderno::leer_de(&pixpin_proyecto::almacen::carpeta(raiz, proyecto))
            .map(|c| c.mensajes)
            .unwrap_or_default()
    };
    let mut mensajes: Vec<cuaderno::Mensaje> = match abierto.as_deref() {
        Some(a) if es_el_abierto => a.mensajes.clone(),
        _ => del_disco(),
    };
    // Un lienzo que el asistente acaba de crear en este turno (`crear_lienzo`,
    // 17-oct) ya esta en el cuaderno pero aun no en la conversacion abierta:
    // se busca en el disco, y al acabar entra en la conversacion (`indice`
    // `None`, mas abajo).
    let mut del_disco_solo = false;
    if let Some(r) = destino
        && es_el_abierto
        && !mensajes
            .iter()
            .any(|m| ia_chat::es_lienzo(m) && m.referencia.as_deref() == Some(r))
    {
        let leidos = del_disco();
        if leidos
            .iter()
            .any(|m| ia_chat::es_lienzo(m) && m.referencia.as_deref() == Some(r))
        {
            mensajes = leidos;
            del_disco_solo = true;
        }
    }
    let elegido = destino
        .and_then(|r| {
            mensajes
                .iter()
                .rposition(|m| ia_chat::es_lienzo(m) && m.referencia.as_deref() == Some(r))
        })
        .or_else(|| ia_chat::lienzo_propio(&mensajes));
    let (indice, mensaje, nuevo) = match elegido {
        Some(i) if del_disco_solo => (None, mensajes[i].clone(), false),
        Some(i) => (Some(i), mensajes[i].clone(), false),
        // Resumir o editar no estrenan un lienzo: si aun no hay, esta vacio.
        None if !matches!(
            trabajo,
            al_lienzo::Trabajo::Dibujar | al_lienzo::Trabajo::Plantilla { .. }
        ) =>
        {
            return match trabajo {
                al_lienzo::Trabajo::Resumir => Ok((
                    al_lienzo::Respuesta {
                        texto:
                            "Lienzo destino: vacio (aun no hay lienzo del asistente en este chat)."
                                .into(),
                        ids: Vec::new(),
                        ctx: Some(Default::default()),
                        ..Default::default()
                    },
                    None,
                )),
                al_lienzo::Trabajo::Juntar { .. } => Ok((al_lienzo::Respuesta::default(), None)),
                _ => Err("todavia no hay ningun lienzo en este chat: dibuja algo primero".into()),
            };
        }
        None => {
            let numero = mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
            let m = escribir_lienzo(
                raiz,
                proyecto,
                ia_chat::APARATO,
                numero,
                Some(NOMBRE_LIENZO_PROPIO),
            )
            .map_err(|e| {
                tracing::warn!(?e, "asistente: no se pudo crear su lienzo");
                "no se pudo crear un lienzo".to_string()
            })?;
            (None, m, true)
        }
    };
    let referencia = mensaje.referencia.clone().unwrap_or_default();
    let hoja = preparar_hoja(
        raiz,
        proyecto,
        &mensaje,
        pixpin_shell::entorno::ahora_utc_ms(),
    )
    .ok_or("no se pudo abrir el lienzo")?;
    let mut escena = hoja.escena;
    let mut lienzo = hoja.lienzo;
    // Mirarlo (`asistente::ojos`): se copia la zona, con sus fotos por su
    // fichero; pintar va en el hilo de los ojos, no en el del chat. Juntar
    // pasos no aplica: cerrado, el lienzo no tiene deshacer.
    match trabajo {
        al_lienzo::Trabajo::Ver { mirada, ctx } => {
            let fotos = &hoja.fotos;
            let enc = crate::asistente::ojos::encargo_de(&escena, mirada, ctx, |id| {
                fotos
                    .iter()
                    .find(|(i, _)| *i == id)
                    .map(|(_, r)| crate::asistente::ojos::Pixeles::Ruta(r.clone()))
            })?;
            return Ok((
                al_lienzo::Respuesta {
                    vista: Some(Box::new(enc)),
                    ..Default::default()
                },
                Some(referencia),
            ));
        }
        al_lienzo::Trabajo::Juntar { .. } => {
            return Ok((al_lienzo::Respuesta::default(), Some(referencia)));
        }
        // La imagen del lienzo (la ultima pegada) para calcarla: de su fichero.
        al_lienzo::Trabajo::Imagen => {
            let (id, caja) = al_lienzo::imagen_de_escena(&escena, &[])
                .ok_or("no hay ninguna imagen en el lienzo")?;
            let ruta = hoja
                .fotos
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(_, r)| r.clone())
                .ok_or("la imagen del lienzo no esta en su carpeta")?;
            let img = crate::imagenes_lienzo::imagen_de_rutas(std::slice::from_ref(&ruta))
                .ok_or("no se pudo leer la imagen del lienzo")?;
            let imagen = al_lienzo::ImagenDelLienzo {
                rgba: img.pixeles,
                ancho: img.ancho,
                alto: img.alto,
                caja,
            };
            return Ok((
                al_lienzo::Respuesta {
                    imagen: Some(Box::new(imagen)),
                    ..Default::default()
                },
                Some(referencia),
            ));
        }
        _ => {}
    }
    let respuesta = match trabajo {
        al_lienzo::Trabajo::Dibujar if en_hoja.is_some() => {
            let n = al_lienzo::meter_de_hoja(
                &mut escena,
                documento,
                en_hoja.expect("se acaba de mirar"),
                None,
            )?;
            if documento.is_empty() {
                // Solo cerraba la hoja: no hay nada que guardar.
                return Ok((
                    al_lienzo::Respuesta {
                        texto: al_lienzo::resumen(0),
                        ..Default::default()
                    },
                    Some(referencia),
                ));
            }
            if !fotos_de_hoja.is_empty() {
                lienzo = crate::imagenes_lienzo::guardar_pegadas_en_hoja(
                    &pixpin_proyecto::almacen::carpeta(raiz, proyecto),
                    &lienzo,
                    &escena,
                    fotos_de_hoja,
                    pixpin_shell::entorno::ahora_utc_ms(),
                )
                .map_err(|e| format!("no se pudieron guardar las fotos: {e}"))?;
            }
            al_lienzo::Respuesta {
                texto: al_lienzo::resumen(n),
                ..Default::default()
            }
        }
        al_lienzo::Trabajo::Dibujar => {
            let ids = al_lienzo::meter_en_sitio(&mut escena, documento, junto_a, None)?;
            al_lienzo::resumen_con_ids(&escena, ids, marco)
        }
        otro => {
            // Un almacen de paso para lo que se pegue; las fotos de la hoja
            // se cargan solo si hace falta su tamano (recortar).
            let mut imagenes = crate::imagenes_lienzo::ImagenesLienzo::nuevo(16_384);
            if matches!(otro, al_lienzo::Trabajo::Editar { edicion, .. } if edicion.herramienta == pixpin_ia::edicion::IMAGEN)
            {
                for (id, ruta) in &hoja.fotos {
                    if let Some(img) =
                        crate::imagenes_lienzo::imagen_de_rutas(std::slice::from_ref(ruta))
                    {
                        imagenes.guardar_con_id(*id, img);
                    }
                }
            }
            let antes = escena.pasos_cerrados();
            let marco = pixpin_ia::edicion::marco_de(&escena);
            let r = al_lienzo::trabajar(&mut escena, otro, &[], marco, &mut imagenes)?;
            if escena.pasos_cerrados() == antes {
                // Elegir o resumir: no se toca el fichero.
                return Ok((r, Some(referencia)));
            }
            let nuevas = imagenes.tomar_nuevas();
            if !nuevas.is_empty() {
                lienzo = crate::imagenes_lienzo::guardar_pegadas_en_hoja(
                    &pixpin_proyecto::almacen::carpeta(raiz, proyecto),
                    &lienzo,
                    &escena,
                    &nuevas,
                    pixpin_shell::entorno::ahora_utc_ms(),
                )
                .map_err(|e| format!("no se pudo guardar la imagen: {e}"))?;
            }
            r
        }
    };
    if !guardar_hoja_dibujada(&hoja.ruta, &lienzo, &escena) {
        return Err("no se pudo guardar el lienzo".into());
    }
    if let Some(a) = abierto.filter(|_| es_el_abierto) {
        let vista = leer_vista(ubicacion, proyecto, &mensaje);
        // Si ya entro por otro camino (la ventana releyo el cuaderno), se
        // cambia en su sitio en vez de salir dos veces.
        let indice = indice.or_else(|| {
            a.mensajes
                .iter()
                .position(|m| m.referencia.is_some() && m.referencia == mensaje.referencia)
        });
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
    let mut respuesta = respuesta;
    // «hecho: ...» -> «hecho: ... «Lienzo del asistente»», con la lista
    // de ids detras.
    let (primera, resto) = respuesta
        .texto
        .split_once('\n')
        .unwrap_or((respuesta.texto.as_str(), ""));
    respuesta.texto = format!(
        "{primera} «{nombre}»{}{}{resto}",
        if nuevo {
            " (lienzo nuevo en este chat)"
        } else {
            ""
        },
        if resto.is_empty() { "" } else { "\n" }
    );
    Ok((respuesta, Some(referencia)))
}

// ---------------------------------------------------------------------------
// La tarjeta de la primera vez

/// Si la tarjeta se ensena: en el chat «Asistente» mientras falte el
/// modelo elegido (o se este bajando).
fn hace_falta_tarjeta(a: &Abierto) -> bool {
    if !a.ficha.es_asistente() {
        return false;
    }
    if let Some(bajando) = DESCARGA.with_borrow(|d| d.as_ref().map(|d| d.modelo)) {
        TARJETA.with_borrow_mut(|t| t.vista = bajando.id == GEMMA4_E2B.id);
        return true;
    }
    // Con Claude o DeepSeek no hace falta bajar nada.
    if ia_nube::eleccion_reciente(&a.raiz) != ia_nube::Eleccion::Local {
        return false;
    }
    let Some(raiz) = ia_motor::raiz() else {
        return false;
    };
    TARJETA.with_borrow_mut(|t| {
        t.vista = false;
        let modelo = elegido(t, &a.raiz);
        if !mirar_listo(t, &raiz, &modelo) {
            return true;
        }
        // El de siempre ya esta: ¿falta el de mejor vista, encendido en los
        // ajustes?
        let quiere = t.ajuste.as_ref().is_some_and(|(_, _, v)| *v);
        if quiere && !mirar_listo(t, &raiz, &GEMMA4_E2B) {
            t.vista = true;
            return true;
        }
        false
    })
}

/// Si el modelo esta bajado, mirado como mucho cada 5 s.
fn mirar_listo(t: &mut Tarjeta, raiz: &std::path::Path, m: &pixpin_ia::modelos::Modelo) -> bool {
    if let Some((_, _, listo)) = t
        .listo
        .iter()
        .find(|(cuando, id, _)| *id == m.id && cuando.elapsed() < Duration::from_secs(5))
    {
        return *listo;
    }
    let listo = ia_descarga::listo(raiz, m);
    t.listo.retain(|(_, id, _)| *id != m.id);
    t.listo.push((Instant::now(), m.id, listo));
    listo
}

/// El modelo de la tarjeta: el de mejor vista si la tarjeta es la suya; si
/// no, lo elegido en ella o lo de los ajustes; sin nada, el ligero si el
/// equipo tiene poca memoria.
fn elegido(t: &mut Tarjeta, raiz_datos: &std::path::Path) -> pixpin_ia::modelos::Modelo {
    use pixpin_ia::modelos::{QWEN35_2B, QWEN35_08B};
    let fresco = t
        .ajuste
        .as_ref()
        .is_some_and(|(c, _, _)| c.elapsed() < Duration::from_secs(5));
    if !fresco {
        let u = Ubicacion::Instalado {
            raiz: raiz_datos.to_path_buf(),
        };
        let aj = pixpin_store::cargar(&u).unwrap_or_default();
        t.ajuste = Some((
            Instant::now(),
            aj.asistente.modelo,
            aj.asistente.mejor_vista,
        ));
    }
    if t.vista {
        return GEMMA4_E2B;
    }
    match t.ligero {
        Some(true) => QWEN35_08B,
        Some(false) => QWEN35_2B,
        None => {
            let id = t
                .ajuste
                .as_ref()
                .map(|(_, m, _)| m.as_str())
                .unwrap_or_default();
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
    let bajando = DESCARGA.with_borrow(|d| {
        d.as_ref()
            .map(|d| (d.fraccion(), d.ultimo.clone(), d.modelo))
    });

    TARJETA.with_borrow_mut(|t| {
        t.botones.clear();
        let modelo = elegido(t, &a.raiz);
        // La del modelo de mejor vista: su titulo, su explicacion y una sola
        // fila (no hay nada que elegir).
        let (titulo, explica) = if t.vista {
            (
                textos.t("asistente-vista-titulo"),
                textos.t("asistente-vista-explica"),
            )
        } else {
            (textos.t("asistente-titulo"), textos.t("asistente-explica"))
        };
        let filas: Vec<(Option<bool>, pixpin_ia::modelos::Modelo)> = if t.vista {
            vec![(None, GEMMA4_E2B)]
        } else {
            vec![
                (Some(false), pixpin_ia::modelos::QWEN35_2B),
                (Some(true), pixpin_ia::modelos::QWEN35_08B),
            ]
        };
        let (_, alto_explica) = p.medir_texto_ajustado(&explica, 13.0 * e, dentro);
        let fila = 48.0 * e;
        let alto = pad
            + 22.0 * e
            + alto_explica
            + 12.0 * e
            + filas.len() as f32 * (fila + 8.0 * e)
            + 30.0 * e
            + 36.0 * e
            + pad;
        let caja = RectF {
            x,
            y: y0,
            ancho,
            alto,
        };
        p.rellenar_redondeado(caja, 16.0 * e, tema.burbuja_otra);
        p.trazar(caja, (1.0 * e).max(1.0), tema.isla_borde);

        let mut y = y0 + pad;
        p.texto(
            &format!("{} {titulo}", if t.vista { "🔭" } else { "🤖" }),
            x + pad,
            y,
            16.0 * e,
            tema.texto,
        );
        y += 22.0 * e;
        p.texto_ajustado(&explica, x + pad, y, 13.0 * e, dentro, tema.apagado);
        y += alto_explica + 12.0 * e;

        // Los modelos, para elegir (el de mejor vista va solo).
        for (ligero, m) in filas {
            let r = RectF {
                x: x + pad,
                y,
                ancho: dentro,
                alto: fila,
            };
            let es = m.id == modelo.id;
            p.rellenar_redondeado(
                r,
                10.0 * e,
                if es {
                    con_alfa(tema.enviar, 0.18)
                } else {
                    tema.campo
                },
            );
            if es {
                p.trazar(r, (1.5 * e).max(1.0), tema.enviar);
            }
            let centro = (r.x + 16.0 * e, r.y + fila / 2.0);
            p.anillo(centro, 7.0 * e, (1.5 * e).max(1.0), tema.enviar);
            if es {
                p.circulo(centro, 4.0 * e, tema.enviar);
            }
            let falta = raiz_m
                .as_deref()
                .map_or(m.bytes(), |rz| ia_descarga::falta(rz, &m));
            let (clave, detalle) = match ligero {
                Some(true) => ("asistente-modelo-ligero", "asistente-detalle-ligero"),
                Some(false) => ("asistente-modelo-normal", "asistente-detalle-normal"),
                None => ("asistente-modelo-vista", "asistente-detalle-vista"),
            };
            p.texto_linea(
                &format!("{} · {}", textos.t(clave), ia_descarga::tamano(falta)),
                r.x + 32.0 * e,
                r.y + 7.0 * e,
                14.0 * e,
                dentro - 40.0 * e,
                tema.texto,
            );
            p.texto_linea(
                &textos.t(detalle),
                r.x + 32.0 * e,
                r.y + 27.0 * e,
                12.0 * e,
                dentro - 40.0 * e,
                tema.apagado,
            );
            if bajando.is_none()
                && let Some(ligero) = ligero
            {
                t.botones.push((a_rect(r), Accion::Elegir { ligero }));
            }
            y += fila + 8.0 * e;
        }

        // La linea de estado: progreso, la pregunta de confirmar, o la
        // ultima noticia.
        let falta = raiz_m
            .as_deref()
            .map_or(modelo.bytes(), |rz| ia_descarga::falta(rz, &modelo));
        let estado = match &bajando {
            Some((fr, Some(pr), _)) if pr.verificando => {
                format!(
                    "{} {} ({:.0} %)",
                    textos.t("asistente-comprobando"),
                    pr.fichero,
                    fr * 100.0
                )
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
                raiz_m
                    .as_deref()
                    .map(|r| r.display().to_string())
                    .unwrap_or_default()
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
            let barra = RectF {
                x: x + pad,
                y,
                ancho: dentro,
                alto: 6.0 * e,
            };
            p.rellenar_redondeado(barra, 3.0 * e, tema.campo);
            p.rellenar_redondeado(
                RectF {
                    ancho: dentro * fr.clamp(0.0, 1.0),
                    ..barra
                },
                3.0 * e,
                tema.enviar,
            );
        }
        y += 12.0 * e;

        // Los botones, a la derecha.
        let (principal, accion_p, secundario): (String, Accion, Option<(String, Accion)>) =
            if bajando.is_some() {
                (textos.t("asistente-cancelar"), Accion::Cancelar, None)
            } else if t.confirmando {
                (
                    format!(
                        "{} ({})",
                        textos.t("asistente-si-bajar"),
                        ia_descarga::tamano(falta)
                    ),
                    Accion::Confirmar,
                    Some((textos.t("asistente-cancelar"), Accion::Cancelar)),
                )
            } else {
                (
                    format!(
                        "{} ({})",
                        textos.t("asistente-bajar"),
                        ia_descarga::tamano(falta)
                    ),
                    Accion::Bajar,
                    // La mejor vista es opcional: se puede decir que no.
                    t.vista
                        .then(|| (textos.t("asistente-ahora-no"), Accion::AhoraNo)),
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
        p.texto(
            &principal,
            boton.x + 14.0 * e,
            boton.y + 8.0 * e,
            tam,
            hex(0xffffff),
        );
        t.botones.push((a_rect(boton), accion_p));
        if let Some((rotulo, acc)) = secundario {
            let (w2, _) = p.medir_texto(&rotulo, tam);
            let otro = RectF {
                x: boton.x - w2 - 36.0 * e,
                ancho: w2 + 28.0 * e,
                ..boton
            };
            p.trazar(otro, (1.0 * e).max(1.0), tema.isla_borde);
            p.texto(
                &rotulo,
                otro.x + 14.0 * e,
                otro.y + 8.0 * e,
                tam,
                tema.texto,
            );
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
    let Some(accion) = TARJETA.with_borrow(|t| {
        t.botones
            .iter()
            .find(|(r, _)| r.contiene(l))
            .map(|(_, a)| *a)
    }) else {
        return false;
    };
    cumplir(ubicacion, a, accion);
    true
}

fn cumplir(ubicacion: &Ubicacion, a: &Abierto, accion: Accion) {
    match accion {
        Accion::Elegir { ligero } => {
            let id = if ligero {
                pixpin_ia::modelos::QWEN35_08B.id
            } else {
                pixpin_ia::modelos::QWEN35_2B.id
            };
            TARJETA.with_borrow_mut(|t| {
                t.ligero = Some(ligero);
                t.confirmando = false;
                t.noticia = None;
                t.listo.clear();
                t.ajuste = None;
            });
            guardar_modelo(ubicacion, id);
        }
        Accion::Bajar => TARJETA.with_borrow_mut(|t| {
            t.confirmando = true;
            t.noticia = None;
        }),
        Accion::AhoraNo => {
            let mut aj = pixpin_store::cargar(ubicacion).unwrap_or_default();
            aj.asistente.mejor_vista = false;
            if let Err(e) = pixpin_store::ajustes::guardar_conservando(ubicacion, &aj) {
                tracing::warn!(?e, "asistente: no se pudo apagar la mejor vista");
            }
            TARJETA.with_borrow_mut(|t| {
                t.ajuste = None;
                t.confirmando = false;
                t.vista = false;
            });
        }
        Accion::Confirmar => {
            let modelo = TARJETA.with_borrow_mut(|t| elegido(t, &a.raiz));
            // El de mejor vista no es «el de siempre»: no se guarda como tal.
            if modelo.id != GEMMA4_E2B.id {
                guardar_modelo(ubicacion, modelo.id);
            }
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
    let Some(o) = opciones.get(cual) else {
        return false;
    };
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
    let cual = MENCIONES.with_borrow(|m| {
        m.botones
            .iter()
            .find(|(r, _)| r.contiene(l))
            .map(|(_, i)| *i)
    });
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
            p.texto_linea(
                &o.rotulo,
                r.x + 8.0 * e,
                r.y + (fila - h) / 2.0,
                14.0 * e,
                r.ancho - 16.0 * e,
                tema.texto,
            );
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

pub(super) mod recetas;

/// Los turnos que siguen con el chat cerrado y lo que quedo a medias al
/// cerrarse PixPin (24-oct-2026).
pub(super) mod de_fondo;

#[cfg(test)]
mod pruebas;
#[cfg(test)]
mod pruebas_pensado;
