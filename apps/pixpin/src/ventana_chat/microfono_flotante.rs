//! **El microfono flotante** (pedido «grabar», el usuario, 2-oct): «cuando le
//! doy a grabar audio solo salga de forma flotante un microfono en la
//! pantalla y la senalizacion de que esta grabando, de forma que se detenga
//! cuando le doy clic … y una opcion para convertirlo en llamada de la app
//! con un selector de hora … y cuando se guarde desaparezca el boton, que
//! este en always on top».
//!
//! Es el `GrabadoraActivity.kt` del movil —un toque para empezar, otro para
//! parar, el toque se coge en toda la ventana porque acertarle a un boton sin
//! mirar es pedir demasiado— en una ventanita siempre encima, sin boton en la
//! barra de tareas y sin abrir el chat:
//!
//! 1. **Grabando**: el microfono grande en el rojo de grabar del chat, un aro
//!    que late, el tiempo y el nivel en barras como la isla del chat. Un clic
//!    en cualquier sitio la para y la guarda; Escape la tira sin preguntar.
//! 2. **Guardada**: «Convertir en llamada» o «Listo». Si nadie toca nada, se
//!    va sola: lo pedido era que desaparezca al guardarse.
//! 3. **La hora**: el `TimePicker` del movil (`HoraDelRecordatorioActivity`),
//!    que nace **una hora mas tarde y en punto** como alli, con atajos
//!    grandes, flechas, la rueda y la hora tecleada. Al ponerla, la nota es
//!    una llamada secreta exactamente como la del menu del chat
//!    (`Accion::RecordarEn`): la hora en el mensaje (`recuerdaEn`), el vigia
//!    avisado y «quien llama» apuntado.
//!
//! Se arrastra por cualquier sitio: pulsar y mover es llevarla; pulsar y
//! soltar sin moverse es un clic. La nota se mete en el proyecto por
//! [`super::guardar_la_voz`], el mismo camino que el boton del chat, con el
//! mismo sello y el mismo realce de la voz (`pixpin_audio::realce`).
//!
//! Va en su hilo, como la llamada secreta: el bucle principal sigue con sus
//! atajos y sus pines. El microfono se abre en otro hilo a la vez que nace la
//! ventana (`ventanita::abrir_microfono`): tarda medio segundo la primera
//! vez, y en ese medio segundo la ventana ya esta en pantalla diciendolo.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_geom::{Punto, Rect};
use pixpin_proyecto::cuaderno::Mensaje;
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::Catalogo;

use super::{OSCURO, mi};
use crate::caja_dibujo::hex;
use crate::overlay::Recursos;
use crate::ventanita::{Botones, VERDE, centrado_en};

/// El rojo de grabar del chat (el del boton de parar y el cronometro).
const ROJO_GRABAR: Color = hex(0xe5534b);
/// Lo que se mueve el raton antes de que un clic pase a ser un arrastre.
const UMBRAL_ARRASTRE: i32 = 4;
/// Lo que espera la ventana de «guardada» antes de irse sola.
const SE_VA_SOLA: Duration = Duration::from_secs(10);
/// Lo que se queda un fallo en pantalla antes de cerrarse.
const FALLO_DURA: Duration = Duration::from_secs(4);
/// Lo que se queda «Te llamara a las…» antes de cerrarse.
const PUESTA_DURA: Duration = Duration::from_millis(1600);
/// Los minutos que mueve cada flecha de los minutos.
const PASO_MINUTOS: i64 = 5;
const MINUTO: i64 = 60_000;
const HORA: i64 = 60 * MINUTO;
const DIA: i64 = 24 * HORA;

const VK_ESCAPE: u32 = 0x1B;
const VK_ENTRAR: u32 = 0x0D;
const VK_RETROCESO: u32 = 0x08;
const VK_ARRIBA: u32 = 0x26;
const VK_ABAJO: u32 = 0x28;

/// Que grabar y donde dejarlo.
#[derive(Debug, Clone)]
pub(crate) struct Pedido {
    pub raiz: PathBuf,
    pub proyecto: String,
    /// El codigo de este equipo, el del sello.
    pub aparato: String,
    pub nombre: String,
}

/// Los rotulos, ya traducidos en el hilo principal (el `Catalogo` no cruza
/// hilos). Los que llevan la hora dentro traen [`HUECO`] donde va.
#[derive(Debug, Clone)]
pub(crate) struct Rotulos {
    abriendo: String,
    guardando: String,
    parar: String,
    guardada: String,
    convertir: String,
    listo: String,
    cuando: String,
    en_15_min: String,
    en_1_hora: String,
    manana: String,
    poner: String,
    volver: String,
    sonara: String,
    puesta: String,
    teclas: String,
    hora_mal: String,
    sin_microfono: String,
    sin_codec: String,
    fallo: String,
    muy_corta: String,
    no_se_pudo: String,
}

/// Lo que se cambia por la hora en `sonara` y `puesta`.
const HUECO: &str = "\u{1}";

impl Rotulos {
    pub(crate) fn de(t: &Catalogo) -> Rotulos {
        let con_hueco = |clave: &str| {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("cuando", HUECO);
            t.t_args(clave, &args)
        };
        Rotulos {
            abriendo: t.t("microfono-abriendo"),
            guardando: t.t("microfono-guardando"),
            parar: t.t("microfono-parar"),
            guardada: t.t("microfono-guardada"),
            convertir: t.t("microfono-convertir"),
            listo: t.t("microfono-listo"),
            cuando: t.t("microfono-cuando"),
            en_15_min: t.t("microfono-en-15-min"),
            en_1_hora: t.t("microfono-en-1-hora"),
            manana: t.t("microfono-manana"),
            poner: t.t("microfono-poner"),
            volver: t.t("microfono-volver"),
            sonara: con_hueco("microfono-sonara"),
            puesta: con_hueco("microfono-puesta"),
            teclas: t.t("microfono-teclas"),
            hora_mal: t.t("chat-recordar-hora-mal"),
            sin_microfono: t.t("chat-voz-sin-microfono"),
            sin_codec: t.t("chat-voz-sin-codec"),
            fallo: t.t("chat-voz-fallo"),
            muy_corta: t.t("chat-voz-muy-corta"),
            no_se_pudo: t.t("chat-no-se-pudo"),
        }
    }

    fn de_un_fallo(&self, e: &pixpin_audio::ErrorAudio) -> String {
        match super::rotulo_de_no_grabar(e) {
            "chat-voz-sin-microfono" => self.sin_microfono.clone(),
            "chat-voz-sin-codec" => self.sin_codec.clone(),
            _ => self.fallo.clone(),
        }
    }
}

/// Si ya hay un microfono flotante en pantalla. Uno solo: dos grabadoras a
/// la vez se pelearian por el mismo microfono y la segunda no se sabria
/// para que es.
static ABIERTO: AtomicBool = AtomicBool::new(false);

/// **Saca el microfono flotante y empieza a grabar**, en su propio hilo.
pub(crate) fn lanzar(pedido: Pedido, rotulos: Rotulos) {
    if ABIERTO.swap(true, Ordering::SeqCst) {
        tracing::info!("ya hay un microfono flotante grabando; el pedido se ignora");
        return;
    }
    let hecho = std::thread::Builder::new()
        .name("microfono-flotante".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            if let Err(e) = Recursos::nuevos().and_then(|r| flotar(&r, &pedido, &rotulos)) {
                tracing::warn!(?e, "no se pudo abrir el microfono flotante");
            }
            ABIERTO.store(false, Ordering::SeqCst);
        });
    if let Err(e) = hecho {
        ABIERTO.store(false, Ordering::SeqCst);
        tracing::warn!(?e, "no se pudo lanzar el hilo del microfono flotante");
    }
}

// --- La hora de la llamada (puro) --------------------------------------

/// **El selector de la hora**, en hora local. Puro, para probarlo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Selector {
    /// La hora elegida, en milisegundos locales. Siempre en el futuro.
    pub cuando: i64,
    /// Lo tecleado a medias («18», «183», «18:3»).
    pub escrito: String,
}

impl Selector {
    /// Como el `TimePicker` del movil: **una hora mas tarde, en punto**
    /// (nadie pone una llamada para ahora mismo).
    pub fn nuevo(ahora: i64) -> Selector {
        let hora = (ahora.rem_euclid(DIA) / HORA + 1) % 24;
        Selector {
            cuando: crate::recordatorios::hora_escrita(&format!("{hora}:00"), ahora)
                .unwrap_or(ahora + HORA),
            escrito: String::new(),
        }
    }

    /// La hora y el minuto que se ensenan.
    pub fn reloj(&self) -> (i64, i64) {
        let del_dia = self.cuando.rem_euclid(DIA);
        (del_dia / HORA, (del_dia % HORA) / MINUTO)
    }

    /// Lleva la hora a `cuando` y, si quedo en el pasado, al dia siguiente.
    fn poner(&mut self, cuando: i64, ahora: i64) {
        let mut c = cuando - cuando.rem_euclid(MINUTO);
        while c <= ahora {
            c += DIA;
        }
        self.cuando = c;
        self.escrito.clear();
    }

    /// Un atajo («En 15 min», «Mañana 9:00»).
    pub fn atajo(&mut self, cuando: i64, ahora: i64) {
        self.poner(cuando, ahora);
    }

    /// Las flechas de la hora: una hora mas o menos, mismo minuto.
    pub fn mover_hora(&mut self, horas: i64, ahora: i64) {
        self.poner(self.cuando + horas * HORA, ahora);
    }

    /// Las flechas de los minutos: de cinco en cinco, y lo que venga de un
    /// atajo («18:47») se encaja primero en el cinco de al lado.
    pub fn mover_minutos(&mut self, pasos: i64, ahora: i64) {
        let (_, m) = self.reloj();
        let resto = m % PASO_MINUTOS;
        let delta = match (pasos.signum(), resto) {
            (1, 0) | (-1, 0) => pasos * PASO_MINUTOS,
            (1, r) => PASO_MINUTOS - r + (pasos - 1) * PASO_MINUTOS,
            (-1, r) => -r + (pasos + 1) * PASO_MINUTOS,
            _ => 0,
        };
        let mut c = self.cuando + delta * MINUTO;
        // Bajar no puede llevarla a mañana sin querer: si cae en el pasado se
        // queda en lo mas cercano que aun no paso.
        if c <= ahora && pasos < 0 {
            c = self.cuando;
        }
        self.poner(c, ahora);
    }

    /// Una letra tecleada: cifras y separadores, cinco como mucho. En cuanto
    /// lo escrito es una hora, esa es la elegida (la proxima vez que el
    /// reloj la marque, como «Elegir la hora…» del chat).
    pub fn teclear(&mut self, c: char, ahora: i64) {
        if !(c.is_ascii_digit() || c == ':' || c == '.') || self.escrito.len() >= 5 {
            return;
        }
        self.escrito.push(c);
        self.releer(ahora);
    }

    pub fn borrar(&mut self, ahora: i64) {
        self.escrito.pop();
        self.releer(ahora);
    }

    fn releer(&mut self, ahora: i64) {
        if let Some(c) = crate::recordatorios::hora_escrita(&self.escrito, ahora) {
            self.cuando = c;
        }
    }

    /// Si lo tecleado no es una hora (para decirlo en rojo).
    pub fn escrito_mal(&self, ahora: i64) -> bool {
        !self.escrito.is_empty()
            && crate::recordatorios::hora_escrita(&self.escrito, ahora).is_none()
    }
}

/// Los atajos de la hora, en hora local: dentro de 15 minutos, dentro de una
/// hora y mañana a las 9, como los del menu del chat
/// (`recordatorios::atajos`) pero los tres que se piden para una llamada.
pub(super) fn atajos(ahora: i64) -> [i64; 3] {
    let manana = (ahora.div_euclid(DIA) + 1) * DIA + 9 * HORA;
    [ahora + 15 * MINUTO, ahora + HORA, manana]
}

/// **Convierte la nota en llamada secreta** a `cuando_local`: lo mismo que
/// `Accion::RecordarEn` del chat sobre una nota de voz. La hora va en el
/// mensaje (`recuerdaEn`, lo que viaja al movil), al vigia se le avisa, y se
/// apunta quien llama (el nombre de la nota, como al elegir en el movil).
/// `Ok(false)` si la nota ya no esta.
pub(super) fn poner_la_llamada(
    raiz: &Path,
    proyecto: &str,
    id: &str,
    cuando_local: i64,
) -> std::io::Result<bool> {
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, proyecto);
    let cuando = crate::recordatorios::de_local_a_utc(cuando_local);
    if !crate::recordatorios::volver_a_poner(&carpeta, id, cuando)? {
        return Ok(false);
    }
    if let Some(m) = pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta)?
        .mensajes
        .iter()
        .find(|m| m.id == id)
    {
        super::quien_llama::al_poner_la_hora(raiz, &carpeta, m);
    }
    super::refrescar();
    Ok(true)
}

// --- La ventana ----------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Convertir,
    Listo,
    Atajo(usize),
    Hora(i64),
    Minutos(i64),
    Poner,
    Volver,
}

/// En que punto esta.
enum Fase {
    /// El microfono se esta abriendo en su hilo.
    Abriendo(Receiver<Result<pixpin_audio::Grabadora, pixpin_audio::ErrorAudio>>),
    Grabando(pixpin_audio::Grabadora),
    /// Parando y cerrando el `.m4a`, en su hilo (25-50 ms).
    Parando(Receiver<Result<pixpin_audio::Grabacion, pixpin_audio::ErrorAudio>>),
    Guardada { mensaje: Mensaje, desde: Instant },
    Hora { mensaje: Mensaje, selector: Selector },
    Puesta { texto: String, desde: Instant },
    Fallo { texto: String, desde: Instant },
}

/// El tamano logico de la ventana en cada fase.
fn tamano(fase: &Fase) -> (u32, u32) {
    match fase {
        Fase::Abriendo(_) | Fase::Grabando(_) | Fase::Parando(_) => (230, 248),
        Fase::Guardada { .. } => (300, 236),
        Fase::Hora { .. } => (320, 400),
        Fase::Puesta { .. } | Fase::Fallo { .. } => (300, 150),
    }
}

/// Abajo a la derecha del monitor del raton, como la llamada; si ya estaba
/// en pantalla, se queda donde la dejo el usuario y solo cambia de tamano,
/// creciendo hacia arriba para no salirse por abajo.
fn marco_para(area: Rect, escala_por_cien: u32, logico: (u32, u32), antes: Option<Rect>) -> Rect {
    let e = |v: u32| v * escala_por_cien / 100;
    let (w, h) = (e(logico.0).min(area.ancho), e(logico.1).min(area.alto));
    let (x, y) = match antes {
        Some(a) => (a.x + a.ancho as i32 - w as i32, a.y + a.alto as i32 - h as i32),
        None => (
            area.x + area.ancho as i32 - w as i32 - e(24) as i32,
            area.y + area.alto as i32 - h as i32 - e(24) as i32,
        ),
    };
    let x = x.clamp(area.x, area.x + area.ancho as i32 - w as i32);
    let y = y.clamp(area.y, area.y + area.alto as i32 - h as i32);
    Rect {
        x,
        y,
        ancho: w,
        alto: h,
    }
}

fn flotar(recursos: &Recursos, pedido: &Pedido, rotulos: &Rotulos) -> Result<()> {
    // El microfono primero: se abre mientras nace la ventana.
    let temporal = super::temporal_de_la_voz(&pedido.raiz, &pedido.proyecto);
    let mut fase = Fase::Abriendo(crate::ventanita::abrir_microfono(temporal.clone()));

    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .monitor_en(pixpin_shell::posicion_del_cursor())
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien as f32 / 100.0;
    let area = monitor.area_trabajo;
    let mut logico = tamano(&fase);
    let mut marco = marco_para(area, monitor.escala_por_cien, logico, None);
    let mut ventana = VentanaOverlay::nueva(marco).context("no se pudo abrir la ventana")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para el microfono")?;
    ventana.mostrar();
    ventana.traer_encima();
    // El foco, para que Escape llegue: quien pidio grabar (el lanzador) dio
    // permiso de pasar al frente con `AllowSetForegroundWindow`.
    ventana.enfocar();

    let empezo = Instant::now();
    let mut botones: Botones<Accion> = Botones::default();
    // El boton pulsado: donde, que habia debajo y si ya es un arrastre.
    let mut pulsado: Option<(Punto, Rect, Option<Accion>, bool)> = None;
    let mut cancelar = false;
    let mut vivo = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        let ahora = pixpin_shell::entorno::ahora_local_ms();
        let mut accion: Option<Accion> = None;
        let mut clic_suelto = false;
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            match ev {
                EventoOverlay::Cerrar => {
                    cancelar = true;
                    vivo = false;
                }
                EventoOverlay::RatonMovido(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    if let Some((desde, origen, _, arrastre)) = pulsado.as_mut() {
                        let (dx, dy) = (p.x - desde.x, p.y - desde.y);
                        if !*arrastre && (dx.abs() > UMBRAL_ARRASTRE || dy.abs() > UMBRAL_ARRASTRE) {
                            *arrastre = true;
                        }
                        if *arrastre {
                            marco = Rect {
                                x: origen.x + dx,
                                y: origen.y + dy,
                                ..*origen
                            };
                            ventana.mover(marco);
                            // El raton relativo a la ventana ya movida.
                            botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                        }
                    }
                    if let Fase::Guardada { desde, .. } = &mut fase {
                        // Mientras el raton anda por encima, no se va.
                        *desde = Instant::now();
                    }
                }
                EventoOverlay::BotonPulsado(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    pulsado = Some((p, marco, botones.bajo_el_raton(), false));
                }
                EventoOverlay::BotonSoltado(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    if let Some((_, _, debajo, arrastre)) = pulsado.take()
                        && !arrastre
                    {
                        // Solo vale si se suelta sobre el mismo boton.
                        match debajo {
                            Some(a) if botones.bajo_el_raton() == Some(a) => accion = Some(a),
                            Some(_) => {}
                            None => clic_suelto = true,
                        }
                    }
                }
                EventoOverlay::Rueda(delta) => {
                    if let Fase::Hora { selector, .. } = &mut fase {
                        let pasos = delta.signum() as i64;
                        // La rueda sobre la mitad izquierda mueve la hora; sobre
                        // la derecha, los minutos.
                        if botones.raton.0 < marco.ancho as f32 / 2.0 {
                            selector.mover_hora(pasos, ahora);
                        } else {
                            selector.mover_minutos(pasos, ahora);
                        }
                    }
                }
                EventoOverlay::Caracter(c) => {
                    if let Fase::Hora { selector, .. } = &mut fase {
                        selector.teclear(c, ahora);
                    }
                }
                EventoOverlay::Tecla { vk, .. } => match (vk, &mut fase) {
                    (VK_ESCAPE, Fase::Abriendo(_) | Fase::Grabando(_)) => {
                        cancelar = true;
                        vivo = false;
                    }
                    (VK_ESCAPE, _) => vivo = false,
                    (VK_ENTRAR, Fase::Hora { .. }) => accion = Some(Accion::Poner),
                    (VK_ENTRAR, Fase::Grabando(_)) => clic_suelto = true,
                    (VK_ENTRAR, Fase::Guardada { .. }) => accion = Some(Accion::Listo),
                    (VK_RETROCESO, Fase::Hora { selector, .. }) => selector.borrar(ahora),
                    (VK_ARRIBA, Fase::Hora { selector, .. }) => selector.mover_minutos(1, ahora),
                    (VK_ABAJO, Fase::Hora { selector, .. }) => selector.mover_minutos(-1, ahora),
                    _ => {}
                },
                _ => {}
            }
        }
        if !vivo {
            break;
        }

        // Lo que llega de los hilos del microfono.
        fase = match fase {
            Fase::Abriendo(rx) => match rx.try_recv() {
                Ok(Ok(g)) => Fase::Grabando(g),
                Ok(Err(e)) => {
                    tracing::warn!(?e, "no se pudo abrir el microfono para el pedido");
                    Fase::Fallo {
                        texto: rotulos.de_un_fallo(&e),
                        desde: Instant::now(),
                    }
                }
                Err(TryRecvError::Empty) => Fase::Abriendo(rx),
                Err(TryRecvError::Disconnected) => Fase::Fallo {
                    texto: rotulos.fallo.clone(),
                    desde: Instant::now(),
                },
            },
            Fase::Parando(rx) => match rx.try_recv() {
                Ok(Ok(grabacion)) => {
                    let hecho = super::siguiente_numero_en(&pedido.raiz, &pedido.proyecto)
                        .map_err(|_| pixpin_audio::ErrorAudio::NoEsAudio {
                            ruta: pedido.proyecto.clone(),
                        })
                        .and_then(|numero| {
                            super::guardar_la_voz(
                                &pedido.raiz,
                                &pedido.proyecto,
                                &pedido.aparato,
                                Some(&pedido.nombre),
                                &grabacion,
                                numero,
                            )
                        });
                    let _ = std::fs::remove_file(&temporal);
                    match hecho {
                        Ok(mensaje) => {
                            super::refrescar();
                            tracing::info!(nombre = %mensaje.nombre, "nota de voz del pedido guardada");
                            Fase::Guardada {
                                mensaje,
                                desde: Instant::now(),
                            }
                        }
                        Err(e) => {
                            tracing::warn!(?e, "no se pudo guardar la nota de voz del pedido");
                            Fase::Fallo {
                                texto: rotulos.fallo.clone(),
                                desde: Instant::now(),
                            }
                        }
                    }
                }
                Ok(Err(e)) => {
                    let _ = std::fs::remove_file(&temporal);
                    let texto = if matches!(e, pixpin_audio::ErrorAudio::DemasiadoCorta { .. }) {
                        rotulos.muy_corta.clone()
                    } else {
                        tracing::warn!(?e, "no se pudo cerrar la nota de voz del pedido");
                        rotulos.fallo.clone()
                    };
                    Fase::Fallo {
                        texto,
                        desde: Instant::now(),
                    }
                }
                Err(TryRecvError::Empty) => Fase::Parando(rx),
                Err(TryRecvError::Disconnected) => Fase::Fallo {
                    texto: rotulos.fallo.clone(),
                    desde: Instant::now(),
                },
            },
            otra => otra,
        };

        // Lo pulsado.
        fase = match (fase, accion, clic_suelto) {
            // Grabando, cualquier clic la para: toda la ventana es el boton.
            (Fase::Grabando(g), _, true) | (Fase::Grabando(g), Some(_), _) => {
                Fase::Parando(crate::ventanita::cerrar_microfono(g))
            }
            (Fase::Guardada { mensaje, .. }, Some(Accion::Convertir), _) => Fase::Hora {
                mensaje,
                selector: Selector::nuevo(ahora),
            },
            (guardada @ Fase::Guardada { .. }, Some(Accion::Listo), _) => {
                vivo = false;
                guardada
            }
            (Fase::Hora { mensaje, mut selector }, Some(a), _) => match a {
                Accion::Atajo(i) => {
                    selector.atajo(atajos(ahora)[i.min(2)], ahora);
                    Fase::Hora { mensaje, selector }
                }
                Accion::Hora(d) => {
                    selector.mover_hora(d, ahora);
                    Fase::Hora { mensaje, selector }
                }
                Accion::Minutos(d) => {
                    selector.mover_minutos(d, ahora);
                    Fase::Hora { mensaje, selector }
                }
                Accion::Volver => Fase::Guardada {
                    mensaje,
                    desde: Instant::now(),
                },
                Accion::Poner => {
                    match poner_la_llamada(&pedido.raiz, &pedido.proyecto, &mensaje.id, selector.cuando) {
                        Ok(true) => Fase::Puesta {
                            texto: rotulos.puesta.replace(
                                HUECO,
                                &crate::recordatorios::cuando_legible(selector.cuando, ahora),
                            ),
                            desde: Instant::now(),
                        },
                        otra => {
                            tracing::warn!(?otra, "no se pudo convertir la nota en llamada");
                            Fase::Fallo {
                                texto: rotulos.no_se_pudo.clone(),
                                desde: Instant::now(),
                            }
                        }
                    }
                }
                _ => Fase::Hora { mensaje, selector },
            },
            (otra, _, _) => otra,
        };

        if !vivo {
            break;
        }
        // Lo que se va solo.
        let se_va = match &fase {
            Fase::Guardada { desde, .. } => desde.elapsed() >= SE_VA_SOLA,
            Fase::Puesta { desde, .. } => desde.elapsed() >= PUESTA_DURA,
            Fase::Fallo { desde, .. } => desde.elapsed() >= FALLO_DURA,
            _ => false,
        };
        if se_va {
            break;
        }

        // Cada fase tiene su tamano: si cambio, la ventana tambien.
        if tamano(&fase) != logico {
            logico = tamano(&fase);
            marco = marco_para(area, monitor.escala_por_cien, logico, Some(marco));
            ventana.mover(marco);
            if let Err(e) = superficie.redimensionar(marco.ancho, marco.alto) {
                tracing::warn!(?e, "no se pudo redimensionar el microfono flotante");
            }
            pulsado = None;
        }

        if let Ok(d) = superficie.empezar(&motor) {
            let _ = motor.dibujar(&d, |p: &Pintor| {
                pintar(
                    p,
                    marco,
                    escala,
                    &fase,
                    pedido,
                    rotulos,
                    empezo.elapsed(),
                    ahora,
                    &mut botones,
                )
            });
            let _ = superficie.presentar();
        }
        // Grabando late (unos 30 fotogramas por segundo); lo demas solo
        // espera al raton o a que llegue el microfono.
        let latido = match &fase {
            Fase::Grabando(_) | Fase::Abriendo(_) | Fase::Parando(_) => 33,
            _ => 250,
        };
        pixpin_shell::overlay::esperar_eventos(Some(latido));
    }
    ventana.ocultar();
    // Lo que quede abierto se cierra: Escape grabando tira la nota sin
    // preguntar, y un microfono que llega tarde se suelta en el acto.
    match fase {
        Fase::Grabando(g) => g.cancelar(),
        Fase::Abriendo(rx) => {
            if let Ok(Ok(g)) = rx.recv_timeout(Duration::from_secs(5)) {
                g.cancelar();
            }
        }
        Fase::Parando(rx) => {
            // Ya estaba parando: la nota se guarda igual, no se pierde.
            if let Ok(Ok(grabacion)) = rx.recv_timeout(Duration::from_secs(5))
                && let Ok(numero) = super::siguiente_numero_en(&pedido.raiz, &pedido.proyecto)
                && super::guardar_la_voz(
                    &pedido.raiz,
                    &pedido.proyecto,
                    &pedido.aparato,
                    Some(&pedido.nombre),
                    &grabacion,
                    numero,
                )
                .is_ok()
            {
                super::refrescar();
            }
        }
        _ => {}
    }
    if cancelar {
        tracing::info!("grabacion del pedido cancelada");
    }
    let _ = std::fs::remove_file(&temporal);
    Ok(())
}

/// Un texto con su opacidad.
fn con_alfa(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

#[allow(clippy::too_many_arguments)]
fn pintar(
    p: &Pintor,
    marco: Rect,
    e: f32,
    fase: &Fase,
    pedido: &Pedido,
    r: &Rotulos,
    llevado: Duration,
    ahora: i64,
    botones: &mut Botones<Accion>,
) {
    let tema = &OSCURO;
    let (w, h) = (marco.ancho as f32, marco.alto as f32);
    let todo = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: h,
    };
    // La tarjeta redondeada sobre transparente: la isla del chat oscuro.
    p.limpiar_transparente();
    p.rellenar_redondeado(todo, 22.0 * e, tema.isla_borde);
    p.rellenar_redondeado(
        RectF {
            x: 1.0,
            y: 1.0,
            ancho: w - 2.0,
            alto: h - 2.0,
        },
        21.0 * e,
        tema.isla,
    );
    botones.vaciar();
    match fase {
        Fase::Abriendo(_) | Fase::Grabando(_) | Fase::Parando(_) => {
            pintar_grabando(p, w, e, fase, pedido, r, llevado, tema)
        }
        Fase::Guardada { mensaje, .. } => {
            let lado = 40.0 * e;
            p.icono(
                &mi::CHECK_CIRCLE,
                RectF {
                    x: (w - lado) / 2.0,
                    y: 18.0 * e,
                    ancho: lado,
                    alto: lado,
                },
                VERDE,
            );
            centrado_en(p, &r.guardada, todo, 66.0 * e, 17.0 * e, tema.texto);
            let nombre = crate::llamada::nombre_de_la_llamada(&mensaje.nombre);
            let (nw, _) = p.medir_texto(&nombre, 13.0 * e);
            let ancho_max = w - 32.0 * e;
            p.texto_linea(
                &nombre,
                ((w - nw.min(ancho_max)) / 2.0).max(16.0 * e),
                92.0 * e,
                13.0 * e,
                ancho_max,
                tema.campo_apagado,
            );
            let caja = RectF {
                x: 16.0 * e,
                y: 122.0 * e,
                ancho: w - 32.0 * e,
                alto: 46.0 * e,
            };
            boton_con_icono(p, botones, caja, Accion::Convertir, &mi::CALL, &r.convertir, tema.enviar, e);
            botones.boton(
                p,
                RectF {
                    y: 178.0 * e,
                    alto: 42.0 * e,
                    ..caja
                },
                Accion::Listo,
                &r.listo,
                None,
                e,
            );
        }
        Fase::Hora { selector, .. } => pintar_hora(p, w, e, selector, r, ahora, botones, tema),
        Fase::Puesta { texto, .. } | Fase::Fallo { texto, .. } => {
            let (icono, color) = if matches!(fase, Fase::Puesta { .. }) {
                (&mi::CALL, VERDE)
            } else {
                (&mi::MIC, ROJO_GRABAR)
            };
            let lado = 34.0 * e;
            p.icono(
                icono,
                RectF {
                    x: (w - lado) / 2.0,
                    y: 16.0 * e,
                    ancho: lado,
                    alto: lado,
                },
                color,
            );
            let ancho_max = w - 36.0 * e;
            let (tw, _) = p.medir_texto_ajustado(texto, 14.0 * e, ancho_max);
            p.texto_ajustado(
                texto,
                (w - tw) / 2.0,
                62.0 * e,
                14.0 * e,
                ancho_max,
                tema.texto,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn pintar_grabando(
    p: &Pintor,
    w: f32,
    e: f32,
    fase: &Fase,
    pedido: &Pedido,
    r: &Rotulos,
    llevado: Duration,
    tema: &super::Tema,
) {
    let todo = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: 1.0,
    };
    let grabando = matches!(fase, Fase::Grabando(_));
    // El nombre con que se guardara, arriba y apagado.
    let ancho_max = w - 32.0 * e;
    let (nw, _) = p.medir_texto(&pedido.nombre, 13.0 * e);
    p.texto_linea(
        &pedido.nombre,
        ((w - nw.min(ancho_max)) / 2.0).max(16.0 * e),
        14.0 * e,
        13.0 * e,
        ancho_max,
        tema.campo_apagado,
    );
    // El microfono grande con su aro que late (una vez por segundo).
    let centro = (w / 2.0, 100.0 * e);
    let radio = 50.0 * e;
    let fase_latido = (llevado.as_millis() % 1000) as f32 / 1000.0;
    if grabando {
        let aro = radio + 14.0 * e * fase_latido;
        p.circulo(centro, aro, con_alfa(ROJO_GRABAR, 0.35 * (1.0 - fase_latido)));
    }
    p.circulo(centro, radio, if grabando { ROJO_GRABAR } else { tema.campo });
    let lado = 46.0 * e;
    p.icono(
        &mi::MIC,
        RectF {
            x: centro.0 - lado / 2.0,
            y: centro.1 - lado / 2.0,
            ancho: lado,
            alto: lado,
        },
        if grabando { Color::BLANCO } else { tema.campo_apagado },
    );

    match fase {
        Fase::Grabando(g) => {
            // «● 0:07»: el punto rojo parpadea, como el de grabar de siempre.
            let tiempo = pixpin_audio::duracion_legible(g.llevado_ms());
            let tam = 24.0 * e;
            let (tw, th) = p.medir_texto(&tiempo, tam);
            let punto = 10.0 * e;
            let x = (w - tw - punto - 8.0 * e) / 2.0;
            let y = 164.0 * e;
            if fase_latido < 0.6 {
                p.circulo((x + punto / 2.0, y + th / 2.0), punto / 2.0, ROJO_GRABAR);
            }
            p.texto(&tiempo, x + punto + 8.0 * e, y, tam, tema.texto);
            // El nivel, en las barras de la isla del chat: lo que dice que
            // el microfono de verdad coge voz y no silencio.
            let nivel = (g.nivel() as f32 / pixpin_audio::PICO_MAXIMO as f32).clamp(0.0, 1.0);
            let paso = pixpin_ui::chat::ONDA_PASO as f32 * e;
            let grueso = (pixpin_ui::chat::ONDA_GRUESO as f32 * e).max(1.0);
            let ancho_barras = w - 64.0 * e;
            let cuantas = ((ancho_barras / paso).floor() as i32).max(1);
            let x0 = (w - cuantas as f32 * paso) / 2.0;
            let eje = 208.0 * e;
            for n in 0..cuantas {
                let medio_de = 1.0 - ((n as f32 / cuantas.max(1) as f32) - 0.5).abs() * 2.0;
                let medio = (9.0 * e * nivel * medio_de).max(0.5);
                p.rellenar_redondeado(
                    RectF {
                        x: x0 + n as f32 * paso,
                        y: eje - medio,
                        ancho: grueso,
                        alto: medio * 2.0,
                    },
                    grueso / 2.0,
                    tema.enviar,
                );
            }
            centrado_en(p, &r.parar, todo, 224.0 * e, 11.5 * e, tema.campo_apagado);
        }
        _ => {
            let texto = if matches!(fase, Fase::Parando(_)) {
                &r.guardando
            } else {
                &r.abriendo
            };
            centrado_en(p, texto, todo, 170.0 * e, 15.0 * e, tema.campo_apagado);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn boton_con_icono(
    p: &Pintor,
    botones: &mut Botones<Accion>,
    caja: RectF,
    que: Accion,
    icono: &pixpin_render::icono::Icono,
    rotulo: &str,
    color: Color,
    e: f32,
) {
    let fondo = if crate::ventanita::dentro(caja, botones.raton) {
        Color {
            r: (color.r + 0.08).min(1.0),
            g: (color.g + 0.08).min(1.0),
            b: (color.b + 0.08).min(1.0),
            a: color.a,
        }
    } else {
        color
    };
    p.rellenar_redondeado(caja, 12.0 * e, fondo);
    let tam = 15.0 * e;
    let lado = 22.0 * e;
    let (tw, th) = p.medir_texto(rotulo, tam);
    let x = caja.x + (caja.ancho - tw - lado - 10.0 * e) / 2.0;
    p.icono(
        icono,
        RectF {
            x,
            y: caja.y + (caja.alto - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        Color::BLANCO,
    );
    p.texto(rotulo, x + lado + 10.0 * e, caja.y + (caja.alto - th) / 2.0, tam, Color::BLANCO);
    botones.zona(caja, que);
}

/// Una flecha (triangulo) hacia arriba o hacia abajo, centrada en `caja`.
fn flecha(p: &Pintor, caja: RectF, arriba: bool, color: Color) {
    let (cx, cy) = (caja.x + caja.ancho / 2.0, caja.y + caja.alto / 2.0);
    let (mw, mh) = (caja.alto * 0.42, caja.alto * 0.24);
    let v = if arriba {
        [(cx - mw, cy + mh), (cx + mw, cy + mh), (cx, cy - mh)]
    } else {
        [(cx - mw, cy - mh), (cx + mw, cy - mh), (cx, cy + mh)]
    };
    p.poligono(&v, color);
}

#[allow(clippy::too_many_arguments)]
fn pintar_hora(
    p: &Pintor,
    w: f32,
    e: f32,
    selector: &Selector,
    r: &Rotulos,
    ahora: i64,
    botones: &mut Botones<Accion>,
    tema: &super::Tema,
) {
    let todo = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: 1.0,
    };
    centrado_en(p, &r.cuando, todo, 16.0 * e, 17.0 * e, tema.texto);

    // Los tres atajos, grandes y en una fila.
    let margen = 16.0 * e;
    let hueco = 8.0 * e;
    let ancho = (w - 2.0 * margen - 2.0 * hueco) / 3.0;
    for (i, rotulo) in [&r.en_15_min, &r.en_1_hora, &r.manana].into_iter().enumerate() {
        let caja = RectF {
            x: margen + i as f32 * (ancho + hueco),
            y: 50.0 * e,
            ancho,
            alto: 36.0 * e,
        };
        let fondo = if crate::ventanita::dentro(caja, botones.raton) {
            tema.boton_sobre
        } else {
            tema.campo
        };
        p.rellenar_redondeado(caja, caja.alto / 2.0, fondo);
        let tam = 13.0 * e;
        let (tw, th) = p.medir_texto(rotulo, tam);
        p.texto(
            rotulo,
            caja.x + (caja.ancho - tw) / 2.0,
            caja.y + (caja.alto - th) / 2.0,
            tam,
            tema.texto,
        );
        botones.zona(caja, Accion::Atajo(i));
    }

    // El reloj: hora y minutos con sus flechas, como una rueda del movil.
    let (hh, mm) = selector.reloj();
    let caja_cifras = 92.0 * e;
    let x_hora = w / 2.0 - 14.0 * e - caja_cifras;
    let x_min = w / 2.0 + 14.0 * e;
    let y_arriba = 100.0 * e;
    let y_cifras = 130.0 * e;
    let alto_cifras = 70.0 * e;
    let y_abajo = 204.0 * e;
    let alto_flecha = 28.0 * e;
    for (x, valor, mover) in [
        (x_hora, hh, Accion::Hora as fn(i64) -> Accion),
        (x_min, mm, Accion::Minutos as fn(i64) -> Accion),
    ] {
        let fondo = RectF {
            x,
            y: y_cifras,
            ancho: caja_cifras,
            alto: alto_cifras,
        };
        p.rellenar_redondeado(fondo, 14.0 * e, tema.campo);
        let texto = format!("{valor:02}");
        let tam = 46.0 * e;
        let (tw, th) = p.medir_texto(&texto, tam);
        p.texto(
            &texto,
            x + (caja_cifras - tw) / 2.0,
            y_cifras + (alto_cifras - th) / 2.0,
            tam,
            tema.texto,
        );
        for (y, arriba, d) in [(y_arriba, true, 1), (y_abajo, false, -1)] {
            let caja = RectF {
                x,
                y,
                ancho: caja_cifras,
                alto: alto_flecha,
            };
            let color = if crate::ventanita::dentro(caja, botones.raton) {
                tema.texto
            } else {
                tema.campo_apagado
            };
            flecha(p, caja, arriba, color);
            botones.zona(caja, mover(d));
        }
    }
    let tam = 40.0 * e;
    let (dw, dh) = p.medir_texto(":", tam);
    p.texto(
        ":",
        (w - dw) / 2.0,
        y_cifras + (alto_cifras - dh) / 2.0,
        tam,
        tema.texto,
    );

    // Cuando sonara, dicho entero («hoy 18:00» o «03/10 09:00»), y lo
    // tecleado si no es una hora.
    let cuando = crate::recordatorios::cuando_legible(selector.cuando, ahora);
    centrado_en(
        p,
        &r.sonara.replace(HUECO, &cuando),
        todo,
        244.0 * e,
        15.0 * e,
        crate::ventanita::DORADO,
    );
    let (pista, color) = if selector.escrito_mal(ahora) {
        (r.hora_mal.as_str(), ROJO_GRABAR)
    } else {
        (r.teclas.as_str(), tema.campo_apagado)
    };
    let ancho_max = w - 2.0 * margen;
    let (pw, _) = p.medir_texto(pista, 11.5 * e);
    p.texto_linea(
        pista,
        ((w - pw.min(ancho_max)) / 2.0).max(margen),
        270.0 * e,
        11.5 * e,
        ancho_max,
        color,
    );

    let caja = RectF {
        x: margen,
        y: 296.0 * e,
        ancho: w - 2.0 * margen,
        alto: 46.0 * e,
    };
    boton_con_icono(p, botones, caja, Accion::Poner, &mi::CALL, &r.poner, VERDE, e);
    botones.boton(
        p,
        RectF {
            y: 350.0 * e,
            alto: 38.0 * e,
            ..caja
        },
        Accion::Volver,
        &r.volver,
        None,
        e,
    );
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Las 14:20 de un dia cualquiera, en hora local.
    const AHORA: i64 = 20_000 * DIA + 14 * HORA + 20 * MINUTO;

    #[test]
    fn la_hora_nace_una_hora_mas_tarde_y_en_punto() {
        let s = Selector::nuevo(AHORA);
        assert_eq!(s.reloj(), (15, 0));
        assert_eq!(s.cuando, 20_000 * DIA + 15 * HORA);
        // A las 23:30, la de «una hora mas tarde» es mañana a las 0:00.
        let tarde = 20_000 * DIA + 23 * HORA + 30 * MINUTO;
        assert_eq!(Selector::nuevo(tarde).cuando, 20_001 * DIA);
    }

    #[test]
    fn los_atajos_son_15_min_una_hora_y_manana_a_las_9() {
        let [a, b, c] = atajos(AHORA);
        assert_eq!(a, AHORA + 15 * MINUTO);
        assert_eq!(b, AHORA + HORA);
        assert_eq!(c, 20_001 * DIA + 9 * HORA);
        // Pasada la medianoche, «mañana» sigue siendo el dia siguiente.
        let madrugada = 20_000 * DIA + HORA;
        assert_eq!(atajos(madrugada)[2], 20_001 * DIA + 9 * HORA);
    }

    #[test]
    fn las_flechas_mueven_y_nunca_dejan_la_hora_en_el_pasado() {
        let mut s = Selector::nuevo(AHORA);
        s.mover_hora(1, AHORA);
        assert_eq!(s.reloj(), (16, 0));
        s.mover_minutos(1, AHORA);
        assert_eq!(s.reloj(), (16, 5));
        s.mover_minutos(-2, AHORA);
        assert_eq!(s.reloj(), (15, 55));
        // Bajar la hora hasta antes de ahora la lleva a mañana, no al pasado.
        s.mover_hora(-2, AHORA);
        assert_eq!(s.reloj(), (13, 55));
        assert!(s.cuando > AHORA);
        assert_eq!(s.cuando, 20_001 * DIA + 13 * HORA + 55 * MINUTO);
        // Lo de un atajo («14:35») se encaja en el cinco de al lado.
        let mut t = Selector::nuevo(AHORA);
        t.atajo(AHORA + 13 * MINUTO, AHORA);
        assert_eq!(t.reloj(), (14, 33));
        t.mover_minutos(1, AHORA);
        assert_eq!(t.reloj(), (14, 35));
        t.atajo(AHORA + 13 * MINUTO, AHORA);
        t.mover_minutos(-1, AHORA);
        assert_eq!(t.reloj(), (14, 30));
        t.mover_minutos(-1, AHORA);
        assert_eq!(t.reloj(), (14, 25));
        // Caso negativo: bajar hasta ahora mismo (las 14:20) no se mueve.
        t.mover_minutos(-1, AHORA);
        assert_eq!(t.reloj(), (14, 25), "las 14:20 ya no es el futuro");
    }

    #[test]
    fn lo_tecleado_pone_la_hora_y_lo_que_no_es_hora_se_dice() {
        let mut s = Selector::nuevo(AHORA);
        for c in "18:30".chars() {
            s.teclear(c, AHORA);
        }
        assert_eq!(s.reloj(), (18, 30));
        assert!(!s.escrito_mal(AHORA));
        s.borrar(AHORA);
        s.borrar(AHORA);
        s.borrar(AHORA);
        s.borrar(AHORA);
        // Queda «1»: la una. Con «19», las siete de la tarde.
        s.teclear('9', AHORA);
        assert_eq!(s.reloj(), (19, 0));
        // «197» no es una hora: se dice y se queda la ultima buena.
        s.teclear('7', AHORA);
        assert!(s.escrito_mal(AHORA));
        assert_eq!(s.reloj(), (19, 0));
        // Las letras no entran.
        let antes = s.clone();
        s.teclear('x', AHORA);
        assert_eq!(s, antes);
    }

    fn proyecto_de_prueba(nombre: &str) -> (PathBuf, String) {
        let raiz = std::env::temp_dir()
            .join(format!("pixpin-microfono-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let f = pixpin_proyecto::almacen::Ficha::nueva("Casa", 1, "PC01");
        pixpin_proyecto::almacen::Indice {
            proyectos: vec![f.clone()],
            ..Default::default()
        }
        .guardar(&raiz)
        .unwrap();
        (raiz, f.id)
    }

    #[test]
    fn la_nota_se_guarda_con_su_nombre_y_se_convierte_en_llamada() {
        let (raiz, proyecto) = proyecto_de_prueba("llamada");
        // Una «grabacion» de mentira: el camino de guardar solo lee el fichero.
        let audio = raiz.join("prueba.m4a");
        std::fs::write(&audio, b"no es audio de verdad").unwrap();
        let grabacion = pixpin_audio::Grabacion {
            ruta: audio,
            duracion_ms: 4200,
            picos: vec![10, 2000, 30000],
        };
        let m = super::super::guardar_la_voz(&raiz, &proyecto, "PC01", Some("reunion"), &grabacion, 1)
            .unwrap();
        assert_eq!(m.nombre, "reunion.m4a");
        assert_eq!(m.clase, Some(pixpin_proyecto::cuaderno::Clase::Voz));
        assert_eq!(m.duracion_ms, 4200);
        assert_eq!(m.aparato.as_deref(), Some("PC01"));

        let cuando = AHORA + HORA;
        assert!(poner_la_llamada(&raiz, &proyecto, &m.id, cuando).unwrap());
        let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, &proyecto);
        let guardada = pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta)
            .unwrap()
            .mensajes
            .into_iter()
            .find(|x| x.id == m.id)
            .unwrap();
        assert_eq!(
            crate::recordatorios::hora_de(&guardada),
            Some(crate::recordatorios::de_local_a_utc(cuando))
        );
        assert_eq!(
            guardada.resto.get("picos"),
            Some(&serde_json::json!([10, 2000, 30000]))
        );
        // Quien llama queda apuntado, con el nombre de la nota.
        assert_eq!(
            crate::llamada::quien_llama(&raiz, &m.id).as_deref(),
            Some("reunion")
        );
        // Caso negativo: una nota que ya no esta no se inventa.
        assert!(!poner_la_llamada(&raiz, &proyecto, "no-esta", cuando).unwrap());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn la_ventana_crece_hacia_arriba_sin_salirse_del_monitor() {
        let area = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1040,
        };
        let m = marco_para(area, 100, (230, 248), None);
        assert_eq!((m.x, m.y, m.ancho, m.alto), (1920 - 230 - 24, 1040 - 248 - 24, 230, 248));
        let mayor = marco_para(area, 100, (320, 400), Some(m));
        // Mismo pie y mismo borde derecho: crece hacia arriba y a la izquierda.
        assert_eq!(mayor.x + mayor.ancho as i32, m.x + m.ancho as i32);
        assert_eq!(mayor.y + mayor.alto as i32, m.y + m.alto as i32);
        // Arrastrada contra la esquina de arriba, no se sale al crecer.
        let arriba = Rect { x: 0, y: 0, ..m };
        let crecida = marco_para(area, 100, (320, 400), Some(arriba));
        assert!(crecida.x >= 0 && crecida.y >= 0);
    }
}
