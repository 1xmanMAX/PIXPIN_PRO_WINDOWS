//! **Grabar una clase o una reunion** (24-oct-2026, el usuario: «que
//! basicamente pueda grabar todo … y de paso lo pueda transcribir … y haya
//! una alternativa para que no ocupe mucho espacio que seria grabar el audio
//! y aparezca con un boton flotante … yo le doy clic y tomo una captura de
//! esa zona y la junta justo en ese tiempo»).
//!
//! Una ventanita siempre encima, en su hilo, como el microfono flotante:
//!
//! 1. **Preparar**: que se oye —el microfono, el sonido del equipo (lo que
//!    suena en Meet o Zoom) o los dos— y en que proyecto se guarda. Un boton
//!    grande para empezar.
//! 2. **Grabando**: una barra pequena con el tiempo, el nivel, **Captura**,
//!    cambiar la zona, pausa, **Terminar** y tirar. La primera captura pide
//!    la zona con la captura de siempre (`ModoConfirmacion::ZonaGrabacion`,
//!    que se abre en el hilo principal); las siguientes repiten la misma
//!    zona con un clic. La barra no sale en ninguna foto
//!    (`excluir_de_capturas`) y pulsarla no le quita el teclado a la
//!    reunion (`poner_sin_activar`).
//! 3. **Guardar**: la nota de voz entra en el chat por el mismo camino que la
//!    del boton del chat ([`super::guardar_la_voz`]) y cada captura detras,
//!    con su minuto en el pie. Se enlazan por `resto`, que es por donde
//!    viajan al movil los campos que solo conoce uno de los dos: la voz
//!    lleva `capturas` (`[{ms, id}]`) y cada imagen `enGrabacion`
//!    (`{de, ms}`).
//! 4. **Pasar a texto**, con su avance en la misma ventana, por el mismo
//!    camino que «Pasar a texto» del chat (`voz::pasar_a_texto`). Sin
//!    reconocedor, se guarda igual y se dice donde pedirlo luego. Cerrar la
//!    ventana a mitad no lo corta: sigue de fondo y escribe el texto al
//!    acabar.
//!
//! **Con video** (25-oct-2026, «si quiero grabar toda la reunion, que
//! tambien pueda grabar el VIDEO mas el audio del equipo y el microfono»):
//! el interruptor «Video de esta pantalla» graba ademas la pantalla donde
//! esta la ventana a un MP4 con el mismo sonido (ver [`video`]). Entra en el
//! chat detras de la nota de voz, enlazado igual que las capturas (la voz
//! lleva `video` (`{id}`) y el video `enGrabacion`), y la transcripcion sale
//! de la nota, como siempre.
//!
//! Las fotos se hacen en su propio hilo ([`fotografo`]): la barra no se
//! congela mientras se captura una zona grande.

#![forbid(unsafe_code)]

mod fotografo;
mod video;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_codec::imagen::ImagenRgba;
use pixpin_geom::{Punto, Rect};
use pixpin_motor_video::{Codificador, Perfil};
use pixpin_proyecto::{almacen, cuaderno};
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Idioma, Ubicacion};

use super::{OSCURO, mi};
use crate::caja_dibujo::hex;
use crate::overlay::Recursos;
use crate::ventanita::{Botones, VERDE, centrado_en, dentro};

/// El rojo de grabar del chat.
const ROJO_GRABAR: Color = hex(0xe5534b);
const UMBRAL_ARRASTRE: i32 = 4;
/// Lo que dura un aviso en la linea de abajo de la barra.
const AVISO_DURA: Duration = Duration::from_millis(2600);
/// Lo que espera el segundo clic en ✕ para tirar la grabacion.
const TIRAR_ESPERA: Duration = Duration::from_secs(4);
/// Lo que se espera a que se elija la zona antes de darla por perdida.
const ZONA_ESPERA: Duration = Duration::from_secs(180);
/// Lo que se queda «Guardada» en pantalla antes de irse sola.
const LISTA_DURA: Duration = Duration::from_secs(12);
const FALLO_DURA: Duration = Duration::from_secs(6);
const VK_ESCAPE: u32 = 0x1B;
const VK_ENTRAR: u32 = 0x0D;

/// Lo que «Grabar clase» necesita del hilo principal.
#[derive(Debug, Clone)]
pub(crate) struct Pedido {
    pub ubicacion: Ubicacion,
    pub idioma: Idioma,
    pub aparato: String,
    /// Donde se puede guardar, `(id, nombre)`: el ultimo usado primero.
    pub proyectos: Vec<(String, String)>,
}

/// Los rotulos, traducidos en el hilo principal (el `Catalogo` no cruza
/// hilos). Los que llevan algo dentro traen [`HUECO`] donde va.
#[derive(Debug, Clone)]
pub(crate) struct Rotulos {
    titulo: String,
    sub: String,
    microfono: String,
    equipo: String,
    video: String,
    video_corto: String,
    video_roto: String,
    video_guardado: String,
    /// Los perfiles del video, en el orden de `Perfil::TODOS`, con su pista.
    calidades: [String; 3],
    pistas_calidad: [String; 3],
    /// «Video con …», con [`HUECO`] donde va el codificador.
    video_con: String,
    /// La grafica, el procesador y el procesador en modo sencillo.
    codificadores: [String; 3],
    zona_perdida: String,
    salvado: String,
    de_fondo: String,
    guardar_en: String,
    empezar: String,
    nada: String,
    captura: String,
    terminar: String,
    capturas: String,
    pista: String,
    elige_zona: String,
    captura_hecha: String,
    zona_cambiada: String,
    en_pausa: String,
    tirar: String,
    sin_zona: String,
    abriendo: String,
    guardando: String,
    texto: String,
    bajando: String,
    lista: String,
    lista_con_texto: String,
    lista_sin_texto: String,
    sin_texto: String,
    listo: String,
    nombre: String,
    sin_equipo: String,
    sin_microfono: String,
    sin_codec: String,
    fallo: String,
    muy_corta: String,
}

const HUECO: &str = "\u{1}";

impl Rotulos {
    pub(crate) fn de(t: &Catalogo) -> Rotulos {
        let con = |clave: &str, nombre: &str| {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set(nombre.to_string(), HUECO);
            t.t_args(clave, &args)
        };
        Rotulos {
            titulo: t.t("clase-titulo"),
            sub: t.t("clase-sub"),
            microfono: t.t("clase-microfono"),
            equipo: t.t("clase-equipo"),
            video: t.t("clase-video"),
            video_corto: t.t("clase-video-corto"),
            video_roto: t.t("clase-video-roto"),
            video_guardado: t.t("clase-video-guardado"),
            calidades: [
                t.t("clase-calidad-ligero"),
                t.t("clase-calidad-normal"),
                t.t("clase-calidad-nitido"),
            ],
            pistas_calidad: [
                t.t("clase-calidad-ligero-pista"),
                t.t("clase-calidad-normal-pista"),
                t.t("clase-calidad-nitido-pista"),
            ],
            video_con: con("clase-video-con", "codificador"),
            codificadores: [
                t.t("clase-codificador-grafica"),
                t.t("clase-codificador-software"),
                t.t("clase-codificador-cpu"),
            ],
            zona_perdida: t.t("clase-zona-perdida"),
            salvado: con("clase-salvado", "n"),
            de_fondo: t.t("clase-texto-de-fondo"),
            guardar_en: con("clase-guardar-en", "proyecto"),
            empezar: t.t("clase-empezar"),
            nada: t.t("clase-nada"),
            captura: t.t("clase-captura"),
            terminar: t.t("clase-terminar"),
            capturas: con("clase-capturas", "n"),
            pista: t.t("clase-pista"),
            elige_zona: t.t("clase-elige-zona"),
            captura_hecha: con("clase-captura-hecha", "minuto"),
            zona_cambiada: t.t("clase-zona-cambiada"),
            en_pausa: t.t("clase-en-pausa"),
            tirar: t.t("clase-tirar"),
            sin_zona: t.t("clase-sin-zona"),
            abriendo: t.t("clase-abriendo"),
            guardando: t.t("clase-guardando"),
            texto: con("clase-texto", "avance"),
            bajando: con("clase-bajando", "avance"),
            lista: con("clase-lista", "proyecto"),
            lista_con_texto: con("clase-lista-con-texto", "n"),
            lista_sin_texto: con("clase-lista-sin-texto", "n"),
            sin_texto: t.t("clase-sin-texto"),
            listo: t.t("clase-listo"),
            nombre: con("clase-nombre", "fecha"),
            sin_equipo: t.t("clase-sin-sonido-equipo"),
            sin_microfono: t.t("chat-voz-sin-microfono"),
            sin_codec: t.t("chat-voz-sin-codec"),
            fallo: t.t("chat-voz-fallo"),
            muy_corta: t.t("chat-voz-muy-corta"),
        }
    }

    /// El nombre de un perfil del video y su pista.
    fn calidad(&self, p: Perfil) -> (&str, &str) {
        let i = match p {
            Perfil::Ligero => 0,
            Perfil::Normal => 1,
            Perfil::Nitido => 2,
        };
        (&self.calidades[i], &self.pistas_calidad[i])
    }

    /// «Video con la grafica», «… con el procesador».
    fn video_con(&self, c: Codificador) -> String {
        let i = match c {
            Codificador::Hardware => 0,
            Codificador::Software => 1,
            Codificador::Cpu => 2,
        };
        con(&self.video_con, &self.codificadores[i])
    }

    fn de_un_fallo(&self, e: &pixpin_audio::ErrorAudio) -> String {
        match e {
            pixpin_audio::ErrorAudio::SinSonidoDelEquipo => self.sin_equipo.clone(),
            pixpin_audio::ErrorAudio::DemasiadoCorta { .. } => self.muy_corta.clone(),
            otro => match super::rotulo_de_no_grabar(otro) {
                "chat-voz-sin-microfono" => self.sin_microfono.clone(),
                "chat-voz-sin-codec" => self.sin_codec.clone(),
                _ => self.fallo.clone(),
            },
        }
    }
}

/// Cambia [`HUECO`] por `que`.
fn con(plantilla: &str, que: &str) -> String {
    plantilla.replace(HUECO, que)
}

/// Una sola a la vez: dos grabaciones se pelearian por el microfono, y las
/// capturas no sabrian a cual van.
static ABIERTA: AtomicBool = AtomicBool::new(false);

/// Por donde llega la zona elegida en el hilo principal, con su primera
/// foto; `None` si se cancelo.
type Zona = Option<(Rect, ImagenRgba)>;
static ZONA: Mutex<Option<Sender<Zona>>> = Mutex::new(None);

/// **La zona de las capturas, elegida en la captura de siempre**: la manda
/// el hilo principal al cerrar el overlay abierto con
/// `ModoConfirmacion::ZonaGrabacion`. Sin grabacion abierta no hace nada.
pub(crate) fn zona_elegida(zona: Zona) {
    if let Some(tx) = ZONA.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        let _ = tx.send(zona);
    }
}

/// **Abre «Grabar clase»** en su propio hilo. Si ya hay una, no abre otra.
pub(crate) fn lanzar(pedido: Pedido, rotulos: Rotulos) {
    if ABIERTA.swap(true, Ordering::SeqCst) {
        tracing::info!("ya hay una clase grabandose; no se abre otra");
        return;
    }
    let (tx, rx) = channel();
    *ZONA.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
    let hecho = std::thread::Builder::new()
        .name("grabar-clase".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            // Los datos viven fuera de `con_recursos`: si el dispositivo de
            // dibujo se pierde y la ventana se reabre, la grabacion sigue.
            let mut estado = Estado::nuevo(&pedido, rotulos.nombre.clone());
            if let Err(e) = crate::dispositivo_perdido::con_recursos("grabar clase", |r| {
                flotar(r, &pedido, &rotulos, &rx, &mut estado)
            }) {
                tracing::warn!(?e, "no se pudo abrir la ventana de grabar clase");
            }
            estado.cerrar(&pedido);
            *ZONA.lock().unwrap_or_else(|e| e.into_inner()) = None;
            ABIERTA.store(false, Ordering::SeqCst);
        });
    if let Err(e) = hecho {
        *ZONA.lock().unwrap_or_else(|e| e.into_inner()) = None;
        ABIERTA.store(false, Ordering::SeqCst);
        tracing::warn!(?e, "no se pudo lanzar el hilo de grabar clase");
    }
}

// --- Lo puro ---------------------------------------------------------------

/// Una captura hecha grabando: en que milisegundo de la nota y el PNG.
#[derive(Debug, Clone)]
pub(super) struct Captura {
    pub ms: i64,
    pub png: Vec<u8>,
}

/// El minuto de una captura en el pie y en los avisos: `12:34` o `1:02:03`.
pub(super) fn minuto(ms: i64) -> String {
    let s = ms.max(0) / 1000;
    let (h, m, s) = (s / 3600, (s / 60) % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// La fecha del nombre de la nota, en hora local: `24-10 18h05` (sin `:`,
/// que Windows no admite en un nombre de fichero).
pub(super) fn fecha_del_nombre(ahora_local_ms: i64) -> String {
    let dias = ahora_local_ms.div_euclid(86_400_000);
    let (_, mes, dia) = crate::automatizaciones::fecha_civil(dias);
    let del_dia = ahora_local_ms.rem_euclid(86_400_000) / 60_000;
    format!("{dia:02}-{mes:02} {:02}h{:02}", del_dia / 60, del_dia % 60)
}

/// Lo que se apunta en la nota de voz: sus capturas en orden, `[{ms, id}]`.
pub(super) fn capturas_en_json(capturas: &[(i64, String)]) -> serde_json::Value {
    let mut v: Vec<&(i64, String)> = capturas.iter().collect();
    v.sort_by_key(|(ms, _)| *ms);
    serde_json::Value::Array(
        v.into_iter()
            .map(|(ms, id)| serde_json::json!({ "ms": ms, "id": id }))
            .collect(),
    )
}

/// Las capturas de una nota de voz grabada como clase, `(ms, id)` en orden.
/// Vacio si no es de una clase o si lo apuntado no se entiende.
pub(crate) fn capturas_de(m: &cuaderno::Mensaje) -> Vec<(i64, String)> {
    let mut v: Vec<(i64, String)> = m
        .resto
        .get("capturas")
        .and_then(|c| c.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|c| Some((c.get("ms")?.as_i64()?, c.get("id")?.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|(ms, _)| *ms);
    v
}

/// La captura que toca en `posicion_ms`: la ultima hecha antes (o en ese
/// mismo momento). Antes de la primera, la primera: algo hay que ensenar.
pub(crate) fn captura_en(capturas: &[(i64, String)], posicion_ms: i64) -> Option<usize> {
    if capturas.is_empty() {
        return None;
    }
    Some(
        capturas
            .iter()
            .rposition(|(ms, _)| *ms <= posicion_ms)
            .unwrap_or(0),
    )
}

// --- El estado ------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Microfono,
    Equipo,
    Video,
    /// El perfil del video («Ligero», «Normal», «Nitido»): un clic pasa al
    /// siguiente.
    Calidad,
    Proyecto,
    Empezar,
    Cerrar,
    Captura,
    Zona,
    Pausa,
    Terminar,
    Tirar,
    /// Cerrar mientras se pasa a texto: el texto sigue de fondo.
    DeFondo,
    Listo,
}

type Abriendo = Receiver<Result<pixpin_audio::Grabadora, pixpin_audio::ErrorAudio>>;
type Parado = Result<pixpin_audio::Grabacion, pixpin_audio::ErrorAudio>;
type Parando = Receiver<Parado>;

enum Fase {
    Preparar,
    Abriendo(Abriendo),
    Grabando(pixpin_audio::Grabadora),
    /// Cerrando la nota y, si lo hay, el video. `listo`: lo que dio la
    /// grabadora, a la espera de que el video termine de cerrarse.
    Parando {
        audio: Parando,
        listo: Option<Parado>,
    },
    /// Pasando a texto la nota ya guardada.
    Texto {
        id: String,
        en_marcha: crate::voz::EnMarcha,
    },
    Lista {
        texto: String,
        sub: String,
        desde: Instant,
    },
    Fallo {
        texto: String,
        desde: Instant,
    },
}

/// Todo lo que no es la ventana: sobrevive a que se reabra.
struct Estado {
    fase: Fase,
    microfono: bool,
    equipo: bool,
    /// Grabar tambien el video de la pantalla.
    video: bool,
    /// Con que perfil (ver `pixpin_motor_video::Perfil`).
    perfil_video: Perfil,
    /// Ya se dijo con que codificador va el video (se dice una vez).
    codificador_avisado: bool,
    /// El sonido para el video, mientras se abre el audio: el video empieza
    /// cuando la grabadora ya esta en marcha (sabe a que muestreo va).
    sonido_para_video: Option<Receiver<Vec<i16>>>,
    /// La pantalla que se graba: la de la ventana al pulsar «Empezar».
    monitor_video: u32,
    grabando_video: Option<video::Video>,
    /// Ya se dijo que el video se corto (se dice una vez).
    video_avisado: bool,
    /// Si el video entro en el chat, para el «Guardada» del final.
    video_guardado: bool,
    /// El hilo de las fotos.
    fotografo: fotografo::Fotografo,
    /// El proyecto elegido, en `Pedido::proyectos`.
    proyecto: usize,
    temporal: PathBuf,
    capturas: Vec<Captura>,
    /// La zona de las capturas, una vez elegida.
    zona: Option<Rect>,
    /// Esperando la zona: desde cuando y el minuto en que se pidio (la
    /// primera foto es de cuando se pulso, no de cuando se termino de
    /// elegir).
    pidiendo_zona: Option<(Instant, i64)>,
    tirar_desde: Option<Instant>,
    aviso: Option<(String, Instant)>,
    /// El ultimo destello de «Captura» (la foto salio).
    destello: Option<Instant>,
    /// Las capturas guardadas, para el «con N capturas» del final.
    guardadas: usize,
    /// El nombre de la nota (`Clase {fecha}`), con [`HUECO`] en la fecha.
    plantilla: String,
}

impl Estado {
    fn nuevo(pedido: &Pedido, plantilla: String) -> Estado {
        Estado {
            fase: Fase::Preparar,
            microfono: true,
            equipo: true,
            video: false,
            perfil_video: Perfil::Normal,
            codificador_avisado: false,
            sonido_para_video: None,
            monitor_video: 0,
            grabando_video: None,
            video_avisado: false,
            video_guardado: false,
            fotografo: fotografo::Fotografo::default(),
            proyecto: 0,
            temporal: PathBuf::new(),
            capturas: Vec::new(),
            zona: None,
            pidiendo_zona: None,
            tirar_desde: None,
            aviso: None,
            destello: None,
            guardadas: 0,
            plantilla,
        }
        .con_temporal(pedido)
    }

    fn con_temporal(mut self, pedido: &Pedido) -> Estado {
        if let Some((id, _)) = pedido.proyectos.get(self.proyecto) {
            self.temporal = super::temporal_de_la_voz(pedido.ubicacion.raiz(), id);
        }
        self
    }

    fn avisar(&mut self, texto: String) {
        self.aviso = Some((texto, Instant::now()));
    }

    /// Al cerrar la ventana a mitad: lo que se estaba grabando se guarda
    /// (cerrar no es tirar; tirar es ✕ dos veces) y el temporal se borra.
    /// Si se estaba pasando a texto, sigue de fondo.
    fn cerrar(&mut self, pedido: &Pedido) {
        let parado = match std::mem::replace(&mut self.fase, Fase::Preparar) {
            Fase::Grabando(g) => Some(g.parar()),
            Fase::Abriendo(rx) => {
                if let Ok(Ok(g)) = rx.recv_timeout(Duration::from_secs(5)) {
                    g.cancelar();
                }
                None
            }
            Fase::Parando { audio, listo } => Some(listo.unwrap_or_else(|| {
                audio
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap_or(Err(pixpin_audio::ErrorAudio::HiloCaido))
            })),
            Fase::Texto { id, en_marcha } => {
                let proyecto = pedido
                    .proyectos
                    .get(self.proyecto)
                    .map(|(id, _)| id.clone())
                    .unwrap_or_default();
                texto_de_fondo(pedido.clone(), proyecto, id, en_marcha);
                None
            }
            _ => None,
        };
        self.sonido_para_video = None;
        if let Some(parado) = parado {
            let g = self.guardar_todo(pedido, parado);
            if let Some(e) = &g.fallo_audio {
                tracing::warn!(?e, "la nota de la clase no se cerro al cerrar la ventana");
            }
            if let Some(e) = &g.fallo_guardar {
                tracing::warn!(?e, "la clase no se guardo al cerrar la ventana");
            }
        }
        // Lo que no llego a grabarse ya no tiene a quien ir.
        if let Some(v) = self.grabando_video.take() {
            v.tirar();
        }
        if !self.temporal.as_os_str().is_empty() {
            let _ = std::fs::remove_file(&self.temporal);
        }
    }

    /// **Guarda todo lo que haya** al parar: las fotos que aun estaban
    /// haciendose, el video y la nota. Si la nota fallo (el microfono se
    /// fue, el disco), las capturas y el video se guardan igual: una
    /// reunion sin su nota sigue valiendo por sus imagenes.
    fn guardar_todo(&mut self, pedido: &Pedido, parado: Parado) -> Guardado {
        for (ms, hecha) in self.fotografo.acabar(Duration::from_secs(5)) {
            match hecha {
                Ok(png) => self.capturas.push(Captura { ms, png }),
                Err(e) => tracing::warn!(?e, ms, "una captura de la clase no se hizo"),
            }
        }
        let video = self.grabando_video.take().and_then(|v| match v.recoger() {
            Ok(ruta) => Some(ruta),
            Err(e) => {
                tracing::warn!(?e, "el video de la clase no se pudo cerrar");
                None
            }
        });
        let (grabacion, fallo_audio) = match parado {
            Ok(g) => (Some(g), None),
            Err(e) => (None, Some(e)),
        };
        // Un resbalon en el boton (menos de un segundo) no deja un video.
        let corta = matches!(
            fallo_audio,
            Some(pixpin_audio::ErrorAudio::DemasiadoCorta { .. })
        );
        let video = match video {
            Some(v) if corta => {
                let _ = std::fs::remove_file(&v);
                None
            }
            otro => otro,
        };
        if grabacion.is_none() && self.capturas.is_empty() && video.is_none() {
            let _ = std::fs::remove_file(&self.temporal);
            return Guardado {
                voz: None,
                fallo_audio,
                fallo_guardar: None,
            };
        }
        let hecho = guardar(pedido, self, grabacion.as_ref(), video.as_deref());
        let _ = std::fs::remove_file(&self.temporal);
        if let Some(v) = video.as_ref().filter(|v| v.is_file()) {
            // No entro en el chat (fallo la nota, el disco) y no se borra:
            // es la reunion entera. Queda en la carpeta del proyecto, al
            // lado de su conversacion, con el nombre de la clase y no con
            // el del temporal.
            self.rescatar_video(pedido, v);
        }
        match hecho {
            Ok(voz) => Guardado {
                voz,
                fallo_audio,
                fallo_guardar: None,
            },
            Err(e) => Guardado {
                voz: None,
                fallo_audio,
                fallo_guardar: Some(e),
            },
        }
    }
}

impl Estado {
    /// Pone el video que no entro en el chat en la carpeta del proyecto
    /// como `Clase {fecha}.mp4` (sin pisar nada).
    fn rescatar_video(&self, pedido: &Pedido, video: &Path) {
        let Some((proyecto, _)) = pedido.proyectos.get(self.proyecto) else {
            tracing::warn!(ruta = %video.display(), "el video de la clase quedo sin entrar al chat");
            return;
        };
        let carpeta = almacen::carpeta(pedido.ubicacion.raiz(), proyecto);
        let nombre = con(
            &self.plantilla,
            &fecha_del_nombre(pixpin_shell::entorno::ahora_local_ms()),
        );
        let elegido = nombre_libre(&carpeta, &almacen::nombre_seguro(&format!("{nombre}.mp4")));
        let destino = carpeta.join(elegido);
        let ruta = if std::fs::rename(video, &destino).is_ok() {
            destino
        } else {
            video.to_path_buf()
        };
        tracing::warn!(ruta = %ruta.display(), "el video de la clase quedo sin entrar al chat");
    }
}

/// Lo que salio de [`Estado::guardar_todo`].
struct Guardado {
    /// La nota de voz, si entro.
    voz: Option<cuaderno::Mensaje>,
    /// Por que no se cerro la nota (lo demas se guardo).
    fallo_audio: Option<pixpin_audio::ErrorAudio>,
    /// Por que no se pudo guardar nada.
    fallo_guardar: Option<anyhow::Error>,
}

/// **Sigue pasando a texto sin ventana** y escribe el texto en la nota al
/// acabar. Es lo que pasa al cerrar la ventana a mitad de la transcripcion:
/// la nota ya esta guardada y no hay por que tirar el trabajo.
fn texto_de_fondo(
    pedido: Pedido,
    proyecto: String,
    id: String,
    mut en_marcha: crate::voz::EnMarcha,
) {
    let lanzado = std::thread::Builder::new()
        .name("clase-texto".into())
        .spawn(move || {
            loop {
                if let Some(salida) = en_marcha.recoger() {
                    match salida {
                        Ok(t) => {
                            if let Err(e) = escribir_el_texto(&pedido, &proyecto, &id, &t) {
                                tracing::warn!(?e, "el texto de la clase no se escribio");
                            }
                        }
                        Err(e) => tracing::info!(?e, "la clase quedo sin texto"),
                    }
                    return;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo seguir pasando a texto la clase");
    }
}

// --- Guardar --------------------------------------------------------------

/// **Mete la clase en el proyecto**: la nota de voz, detras el video (si
/// lo hay) y cada captura, enlazados. Devuelve el mensaje de la voz.
///
/// Sin `grabacion` (la nota no se pudo cerrar) entran igual el video y las
/// capturas, sueltos. Los numeros van seguidos: una captura que no entra no
/// deja un hueco.
fn guardar(
    pedido: &Pedido,
    estado: &mut Estado,
    grabacion: Option<&pixpin_audio::Grabacion>,
    video: Option<&Path>,
) -> Result<Option<cuaderno::Mensaje>> {
    let raiz = pedido.ubicacion.raiz();
    let (proyecto, _) = pedido
        .proyectos
        .get(estado.proyecto)
        .context("sin proyecto donde guardar")?;
    let nombre = con(
        &estado.plantilla,
        &fecha_del_nombre(pixpin_shell::entorno::ahora_local_ms()),
    );
    let mut numero = super::siguiente_numero_en(raiz, proyecto)?;
    let mut voz = match grabacion {
        Some(g) => {
            let v =
                super::guardar_la_voz(raiz, proyecto, &pedido.aparato, Some(&nombre), g, numero)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
            numero += 1;
            Some(v)
        }
        None => None,
    };
    let de = voz.as_ref().map(|v| v.id.clone());
    let carpeta = almacen::carpeta(raiz, proyecto);
    let enlazar = |m: &mut cuaderno::Mensaje, ms: i64| {
        if let Some(de) = &de {
            m.resto.insert(
                "enGrabacion".into(),
                serde_json::json!({ "de": de, "ms": ms }),
            );
            if let Err(e) = cuaderno::reemplazar(&carpeta, m) {
                tracing::warn!(?e, "un adjunto de la clase quedo sin enlazar");
            }
        }
    };

    // El video, justo detras de la nota: es la misma reunion.
    let mut video_id = None;
    if let Some(ruta) = video {
        match adjuntar_video(
            raiz,
            proyecto,
            &pedido.aparato,
            &format!("{nombre}.mp4"),
            ruta,
            numero,
        ) {
            Ok(mut m) => {
                numero += 1;
                enlazar(&mut m, 0);
                video_id = Some(m.id);
                estado.video_guardado = true;
            }
            Err(e) => tracing::warn!(
                ?e,
                ruta = %ruta.display(),
                "el video de la clase no entro en el chat (se queda en la carpeta)"
            ),
        }
    }

    let mut capturas = std::mem::take(&mut estado.capturas);
    capturas.sort_by_key(|c| c.ms);
    let mut enlazadas = Vec::new();
    for (i, c) in capturas.into_iter().enumerate() {
        let cuando = pixpin_shell::entorno::ahora_utc_ms();
        let fichero = format!("clase-{cuando}-{}.png", i + 1);
        let pie = format!("⏱ {} · {}", minuto(c.ms), nombre);
        let hecho = super::adjuntar_con_pie(
            raiz,
            proyecto,
            &pedido.aparato,
            &fichero,
            &c.png,
            numero,
            &pie,
            None,
        );
        match hecho {
            Ok(mut m) => {
                numero += 1;
                enlazar(&mut m, c.ms);
                enlazadas.push((c.ms, m.id));
            }
            Err(e) => tracing::warn!(?e, ms = c.ms, "una captura de la clase no se guardo"),
        }
    }
    estado.guardadas = enlazadas.len();
    if let Some(voz) = voz.as_mut() {
        // Siempre, aunque no haya capturas: es lo que dice que la nota es
        // de una clase y no una nota de voz cualquiera.
        voz.resto
            .insert("grabacion".into(), serde_json::json!("clase"));
        voz.resto
            .insert("capturas".into(), capturas_en_json(&enlazadas));
        if let Some(id) = &video_id {
            voz.resto
                .insert("video".into(), serde_json::json!({ "id": id }));
        }
        if let Err(e) = cuaderno::reemplazar(&carpeta, voz) {
            // La nota y sus adjuntos ya estan dentro; solo falta el enlace.
            tracing::warn!(?e, "la nota de la clase quedo sin sus enlaces");
        }
    }
    super::refrescar();
    tracing::info!(
        con_voz = voz.is_some(),
        con_video = video_id.is_some(),
        capturas = enlazadas.len(),
        duracion_ms = grabacion.map(|g| g.duracion_ms),
        "clase guardada"
    );
    Ok(voz)
}

/// Un nombre que no pise nada en `carpeta`: `nombre`, o `tronco (n).ext`.
fn nombre_libre(carpeta: &Path, nombre: &str) -> String {
    let (tronco, extension) = match nombre.rsplit_once('.') {
        Some((t, e)) if !t.is_empty() => (t.to_string(), format!(".{e}")),
        _ => (nombre.to_string(), String::new()),
    };
    let mut intento = nombre.to_string();
    let mut n = 1;
    while carpeta.join(&intento).exists() && n < 10_000 {
        intento = format!("{tronco} ({n}){extension}");
        n += 1;
    }
    intento
}

/// **El video entra en el chat como un adjunto** (como si se soltara el
/// `.mp4`), pero **moviendo** el fichero en vez de leerlo entero: una hora
/// de reunion son cientos de megas, y en el equipo de referencia (4 GB) no
/// caben en memoria. El temporal ya esta en la carpeta del proyecto, asi que
/// moverlo es un renombrado.
fn adjuntar_video(
    raiz: &Path,
    proyecto: &str,
    aparato: &str,
    nombre: &str,
    origen: &Path,
    numero: i64,
) -> std::io::Result<cuaderno::Mensaje> {
    let carpeta = almacen::carpeta(raiz, proyecto);
    let archivos = carpeta.join("archivos");
    std::fs::create_dir_all(&archivos)?;
    let elegido = nombre_libre(&archivos, &almacen::nombre_seguro(nombre));
    let destino = archivos.join(&elegido);
    if std::fs::rename(origen, &destino).is_err() {
        if let Err(e) = std::fs::copy(origen, &destino) {
            // Una copia a medias (disco lleno) no puede quedar como si
            // fuera el video: el bueno sigue en `origen`.
            let _ = std::fs::remove_file(&destino);
            return Err(e);
        }
        let _ = std::fs::remove_file(origen);
    }
    let bytes = std::fs::metadata(&destino).map(|m| m.len()).unwrap_or(0);
    let cuando = super::descripcion::hora_sin_repetir();
    let mensaje = cuaderno::Mensaje::adjunto(
        cuaderno::clase_de_nombre(nombre),
        nombre,
        &format!("archivos/{elegido}"),
        bytes as i64,
        &cuaderno::Sello {
            cuando,
            numero,
            aparato: aparato.to_string(),
            proyecto: proyecto.to_string(),
        },
    );
    cuaderno::anadir(&carpeta, &mensaje)?;
    let mut indice = almacen::Indice::leer(raiz);
    if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == proyecto) {
        f.tocado = cuando;
        f.resumen = mensaje.resumen();
        indice.guardar(raiz)?;
    }
    Ok(mensaje)
}

/// Empieza a pasar a texto la nota recien guardada, si se puede.
fn empezar_el_texto(pedido: &Pedido, proyecto: &str, voz: &cuaderno::Mensaje) -> Option<Fase> {
    let u = &pedido.ubicacion;
    // Lo mismo que mira «Pasar a texto» del chat: Whisper (el modelo se baja
    // solo), el reconocedor de Windows o, si no, que este todo para Vosk.
    let whisper = crate::voz::whisper_posible(u, pedido.idioma);
    if !whisper
        && !crate::voz::windows_sabe(pedido.idioma)
        && !matches!(
            crate::voz::disponibilidad(u, pedido.idioma),
            pixpin_voz::Disponibilidad::Listo(_)
        )
    {
        return None;
    }
    let ruta = super::ruta_del_mensaje(u.raiz(), proyecto, voz).filter(|r| r.is_file())?;
    Some(Fase::Texto {
        id: voz.id.clone(),
        en_marcha: crate::voz::pasar_a_texto(
            &voz.id,
            &ruta,
            u,
            pedido.idioma,
            crate::voz::turnos_de(voz),
            None,
        ),
    })
}

/// Escribe el texto en la nota (relee el cuaderno: el chat pudo tocarla).
fn escribir_el_texto(
    pedido: &Pedido,
    proyecto: &str,
    id: &str,
    t: &pixpin_voz::Transcripcion,
) -> Result<()> {
    let raiz = pedido.ubicacion.raiz();
    let carpeta = almacen::carpeta(raiz, proyecto);
    let mut m = cuaderno::Cuaderno::leer_de(&carpeta)?
        .mensajes
        .into_iter()
        .find(|m| m.id == id)
        .context("la nota de la clase ya no esta")?;
    crate::voz::aplicar(&mut m, t);
    cuaderno::reemplazar(&carpeta, &m)?;
    super::refrescar();
    Ok(())
}

// --- Capturar la zona -----------------------------------------------------

/// Pide la zona al hilo principal: abre la captura de siempre. Si ya se
/// esta eligiendo, no se pide otra vez: se perderia el minuto en que se
/// pulso la primera.
fn pedir_zona(estado: &mut Estado, ms: i64, r: &Rotulos) {
    if estado.pidiendo_zona.is_some() {
        estado.avisar(r.elige_zona.clone());
        return;
    }
    if pixpin_shell::mensajero::pedir_ventana_principal(crate::ID_ZONA_DE_LA_CLASE) {
        estado.pidiendo_zona = Some((Instant::now(), ms));
        estado.avisar(r.elige_zona.clone());
    } else {
        estado.avisar(r.sin_zona.clone());
    }
}

// --- La ventana -----------------------------------------------------------

fn tamano(fase: &Fase) -> (u32, u32) {
    match fase {
        Fase::Preparar => (340, 370),
        // Lo que piden los botones de la barra (ver `pintar_barra`): 146 de
        // tiempo y nivel, 392 de botones y el margen.
        Fase::Abriendo(_) | Fase::Grabando(_) => (556, 92),
        _ => (360, 170),
    }
}

/// Preparar, en el centro del monitor; la barra, arriba en el centro (a la
/// vista y lejos de lo que se suele mirar); lo demas, donde quedo la barra.
fn marco_para(area: Rect, escala: u32, fase: &Fase, antes: Option<Rect>) -> Rect {
    let (lw, lh) = tamano(fase);
    let e = |v: u32| v * escala / 100;
    let (w, h) = (e(lw).min(area.ancho), e(lh).min(area.alto));
    let (x, y) = match (fase, antes) {
        (Fase::Preparar, _) => {
            let c = crate::ventanita::centrado(area, lw, lh, escala);
            (c.x, c.y)
        }
        (Fase::Abriendo(_), _) | (_, None) => (
            area.x + (area.ancho as i32 - w as i32) / 2,
            area.y + e(16) as i32,
        ),
        (_, Some(a)) => (a.x + (a.ancho as i32 - w as i32) / 2, a.y),
    };
    Rect {
        x: x.clamp(area.x, area.x + area.ancho as i32 - w as i32),
        y: y.clamp(area.y, area.y + area.alto as i32 - h as i32),
        ancho: w,
        alto: h,
    }
}

/// **El marco tras arrastrar la ventana a otra pantalla**: el tamano de la
/// fase con la escala de esa pantalla, en el mismo sitio (centrado sobre
/// donde se solto) y sin salirse de ella.
fn reencajar(area: Rect, escala: u32, fase: &Fase, antes: Rect) -> Rect {
    let (lw, lh) = tamano(fase);
    let (w, h) = (
        (lw * escala / 100).min(area.ancho),
        (lh * escala / 100).min(area.alto),
    );
    let cx = antes.x + antes.ancho as i32 / 2;
    let cy = antes.y + antes.alto as i32 / 2;
    Rect {
        x: (cx - w as i32 / 2).clamp(area.x, area.x + area.ancho as i32 - w as i32),
        y: (cy - h as i32 / 2).clamp(area.y, area.y + area.alto as i32 - h as i32),
        ancho: w,
        alto: h,
    }
}

/// El centro de un marco.
fn centro(r: Rect) -> Punto {
    Punto {
        x: r.x + r.ancho as i32 / 2,
        y: r.y + r.alto as i32 / 2,
    }
}

fn flotar(
    recursos: &Recursos,
    pedido: &Pedido,
    r: &Rotulos,
    zonas: &Receiver<Zona>,
    estado: &mut Estado,
) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let mut monitor = monitores
        .monitor_en(pixpin_shell::posicion_del_cursor())
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let mut escala = monitor.escala_por_cien as f32 / 100.0;
    let mut area = monitor.area_trabajo;
    let mut logico = tamano(&estado.fase);
    let mut marco = marco_para(area, monitor.escala_por_cien, &estado.fase, None);
    let mut ventana = VentanaOverlay::nueva(marco).context("no se pudo abrir la ventana")?;
    ventana.excluir_de_capturas();
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para grabar clase")?;
    ventana.mostrar();
    ventana.traer_encima();
    if matches!(estado.fase, Fase::Preparar) {
        ventana.enfocar();
    } else {
        ventana.poner_sin_activar();
    }

    let empezo = Instant::now();
    let mut botones: Botones<Accion> = Botones::default();
    let mut pulsado: Option<(Punto, Rect, Option<Accion>, bool)> = None;
    let mut vivo = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        let mut accion: Option<Accion> = None;
        let mut soltado_tras_arrastrar = false;
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            match ev {
                // Cerrar no tira nada: lo grabado se guarda al salir.
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::RatonMovido(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    if let Some((desde, origen, _, arrastre)) = pulsado.as_mut() {
                        let (dx, dy) = (p.x - desde.x, p.y - desde.y);
                        if !*arrastre && (dx.abs() > UMBRAL_ARRASTRE || dy.abs() > UMBRAL_ARRASTRE)
                        {
                            *arrastre = true;
                        }
                        if *arrastre {
                            marco = Rect {
                                x: origen.x + dx,
                                y: origen.y + dy,
                                ..*origen
                            };
                            ventana.mover(marco);
                            botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                        }
                    }
                }
                EventoOverlay::BotonPulsado(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    pulsado = Some((p, marco, botones.bajo_el_raton(), false));
                }
                EventoOverlay::BotonSoltado(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    match pulsado.take() {
                        Some((_, _, Some(a), false)) if botones.bajo_el_raton() == Some(a) => {
                            accion = Some(a);
                        }
                        Some((_, _, _, true)) => soltado_tras_arrastrar = true,
                        _ => {}
                    }
                }
                EventoOverlay::Tecla { vk, .. } => match (vk, &estado.fase) {
                    (VK_ESCAPE, Fase::Preparar) => accion = Some(Accion::Cerrar),
                    (VK_ENTRAR, Fase::Preparar) => accion = Some(Accion::Empezar),
                    (VK_ESCAPE, Fase::Texto { .. }) => accion = Some(Accion::DeFondo),
                    (VK_ESCAPE | VK_ENTRAR, Fase::Lista { .. } | Fase::Fallo { .. }) => {
                        accion = Some(Accion::Listo)
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        if !vivo {
            break;
        }

        // Arrastrada a otra pantalla: la barra se queda en esa (su area y su
        // escala), y ahi es donde vuelve a colocarse al cambiar de fase.
        if soltado_tras_arrastrar
            && let Ok(ms) = pixpin_capture::enumerar_monitores()
            && let Some(m) = ms.monitor_en(centro(marco))
            && *m != monitor
        {
            monitor = m.to_owned();
            escala = monitor.escala_por_cien as f32 / 100.0;
            area = monitor.area_trabajo;
            marco = reencajar(area, monitor.escala_por_cien, &estado.fase, marco);
            ventana.mover(marco);
            if let Err(e) = superficie.redimensionar(marco.ancho, marco.alto) {
                tracing::warn!(?e, "no se pudo redimensionar grabar clase");
            }
        }

        // La zona elegida en el hilo principal.
        while let Ok(z) = zonas.try_recv() {
            let pedida = estado.pidiendo_zona.take();
            match z {
                Some((zona, imagen)) => {
                    estado.zona = Some(zona);
                    // Llegada tarde (despues de darla por perdida): vale
                    // igual, con el minuto de ahora.
                    let ms = pedida.map(|(_, ms)| ms).or(match &estado.fase {
                        Fase::Grabando(g) => Some(g.llevado_ms()),
                        _ => None,
                    });
                    match ms {
                        Some(ms) if estado.fotografo.imagen(ms, imagen) => {}
                        Some(_) => estado.avisar(r.sin_zona.clone()),
                        None => estado.avisar(r.zona_cambiada.clone()),
                    }
                }
                None => {
                    if pedida.is_some() {
                        estado.aviso = None;
                    }
                }
            }
        }
        if estado
            .pidiendo_zona
            .is_some_and(|(desde, _)| desde.elapsed() > ZONA_ESPERA)
        {
            estado.pidiendo_zona = None;
            estado.avisar(r.zona_perdida.clone());
        }

        // Las fotos que ya volvieron de su hilo.
        for (ms, hecha) in estado.fotografo.recoger() {
            match hecha {
                Ok(png) => {
                    estado.capturas.push(Captura { ms, png });
                    estado.destello = Some(Instant::now());
                    estado.avisar(con(&r.captura_hecha, &minuto(ms)));
                }
                Err(e) => {
                    tracing::warn!(?e, "no se pudo capturar la zona de la clase");
                    estado.avisar(r.sin_zona.clone());
                }
            }
        }

        // El video que se corto: se dice una vez; la nota sigue.
        if !estado.video_avisado && estado.grabando_video.as_ref().is_some_and(|v| v.roto()) {
            estado.video_avisado = true;
            estado.avisar(r.video_roto.clone());
        }
        // Y con que se codifica, una vez, en cuanto el motor arranca: la
        // grafica, el procesador o el modo sencillo.
        if !estado.codificador_avisado
            && let Some(c) = estado
                .grabando_video
                .as_ref()
                .and_then(video::Video::codificador)
        {
            estado.codificador_avisado = true;
            if !estado.video_avisado {
                estado.avisar(r.video_con(c));
            }
        }

        // Lo que llega de los hilos del audio y del texto.
        estado.fase = match std::mem::replace(&mut estado.fase, Fase::Preparar) {
            Fase::Abriendo(rx) => match rx.try_recv() {
                Ok(Ok(g)) => {
                    // A partir de aqui los clics en la barra no le quitan el
                    // teclado a la reunion.
                    ventana.poner_sin_activar();
                    if let Some(sonido) = estado.sonido_para_video.take() {
                        estado.grabando_video = Some(video::Video::empezar(
                            video::temporal_del_video(&estado.temporal),
                            estado.monitor_video,
                            g.muestreo(),
                            estado.perfil_video,
                            sonido,
                        ));
                    }
                    Fase::Grabando(g)
                }
                Ok(Err(e)) => {
                    tracing::warn!(?e, "no se pudo abrir el audio de la clase");
                    estado.sonido_para_video = None;
                    Fase::Fallo {
                        texto: r.de_un_fallo(&e),
                        desde: Instant::now(),
                    }
                }
                Err(TryRecvError::Empty) => Fase::Abriendo(rx),
                Err(TryRecvError::Disconnected) => {
                    estado.sonido_para_video = None;
                    Fase::Fallo {
                        texto: r.fallo.clone(),
                        desde: Instant::now(),
                    }
                }
            },
            Fase::Parando { audio, mut listo } => {
                if listo.is_none() {
                    listo = match audio.try_recv() {
                        Ok(p) => Some(p),
                        Err(TryRecvError::Empty) => None,
                        Err(TryRecvError::Disconnected) => {
                            Some(Err(pixpin_audio::ErrorAudio::HiloCaido))
                        }
                    };
                }
                // El video cierra su MP4 en cuanto se acaba el sonido; se le
                // espera aqui para no congelar la ventana en el `join`.
                let video_cerrado = estado
                    .grabando_video
                    .as_ref()
                    .is_none_or(video::Video::acabado);
                match listo {
                    Some(parado) if video_cerrado => tras_parar(pedido, r, estado, parado),
                    listo => Fase::Parando { audio, listo },
                }
            }
            Fase::Texto { id, mut en_marcha } => match en_marcha.recoger() {
                None => Fase::Texto { id, en_marcha },
                Some(salida) => {
                    let (proyecto, nombre) = pedido
                        .proyectos
                        .get(estado.proyecto)
                        .cloned()
                        .unwrap_or_default();
                    let con_texto = match salida {
                        Ok(t) => match escribir_el_texto(pedido, &proyecto, &id, &t) {
                            Ok(()) => true,
                            Err(e) => {
                                tracing::warn!(?e, "el texto de la clase no se escribio");
                                false
                            }
                        },
                        Err(e) => {
                            tracing::info!(?e, "la clase quedo sin texto");
                            false
                        }
                    };
                    lista(r, &nombre, estado, con_texto)
                }
            },
            otra => otra,
        };

        // Lo pulsado.
        if let Some(a) = accion {
            pulsar(a, pedido, r, estado, monitor.id, &mut vivo);
        }
        if !vivo {
            break;
        }
        if estado
            .tirar_desde
            .is_some_and(|d| d.elapsed() > TIRAR_ESPERA)
        {
            estado.tirar_desde = None;
        }
        if estado
            .aviso
            .as_ref()
            .is_some_and(|(_, d)| d.elapsed() > AVISO_DURA)
            && estado.pidiendo_zona.is_none()
        {
            estado.aviso = None;
        }
        let se_va = match &estado.fase {
            Fase::Lista { desde, .. } => desde.elapsed() >= LISTA_DURA,
            Fase::Fallo { desde, .. } => desde.elapsed() >= FALLO_DURA,
            _ => false,
        };
        if se_va {
            break;
        }

        if tamano(&estado.fase) != logico {
            logico = tamano(&estado.fase);
            marco = marco_para(area, monitor.escala_por_cien, &estado.fase, Some(marco));
            ventana.mover(marco);
            if let Err(e) = superficie.redimensionar(marco.ancho, marco.alto) {
                tracing::warn!(?e, "no se pudo redimensionar grabar clase");
            }
            pulsado = None;
        }

        if let Ok(d) = superficie.empezar(&motor) {
            let _ = motor.dibujar(&d, |p: &Pintor| {
                pintar(
                    p,
                    marco,
                    escala,
                    estado,
                    pedido,
                    r,
                    empezo.elapsed(),
                    &mut botones,
                )
            });
            let _ = superficie.presentar();
        }
        let latido = match &estado.fase {
            Fase::Preparar | Fase::Lista { .. } | Fase::Fallo { .. } => 120,
            _ => 33,
        };
        pixpin_shell::overlay::esperar_eventos(Some(latido));
    }
    ventana.ocultar();
    Ok(())
}

/// Con la nota (y el video) ya cerrados: guardar y seguir a pasar a texto,
/// o decir que se guardo, o que fallo y que se salvo.
fn tras_parar(pedido: &Pedido, r: &Rotulos, estado: &mut Estado, parado: Parado) -> Fase {
    let g = estado.guardar_todo(pedido, parado);
    let (proyecto, nombre) = pedido
        .proyectos
        .get(estado.proyecto)
        .cloned()
        .unwrap_or_default();
    if let Some(e) = &g.fallo_guardar {
        tracing::warn!(?e, "no se pudo guardar la clase");
        return Fase::Fallo {
            texto: r.fallo.clone(),
            desde: Instant::now(),
        };
    }
    match (g.voz, g.fallo_audio) {
        (Some(voz), _) => match empezar_el_texto(pedido, &proyecto, &voz) {
            Some(f) => f,
            None => lista(r, &nombre, estado, false),
        },
        (None, Some(e)) => {
            tracing::warn!(?e, "no se pudo cerrar el audio de la clase");
            let mut texto = r.de_un_fallo(&e);
            // Lo que si entro se dice: que nadie crea que lo perdio todo.
            if estado.guardadas > 0 || estado.video_guardado {
                texto = format!(
                    "{texto} · {}",
                    con(&r.salvado, &estado.guardadas.to_string())
                );
            }
            Fase::Fallo {
                texto,
                desde: Instant::now(),
            }
        }
        (None, None) => lista(r, &nombre, estado, false),
    }
}

fn lista(r: &Rotulos, proyecto: &str, estado: &Estado, con_texto: bool) -> Fase {
    let n = estado.guardadas.to_string();
    let sub = con(
        if con_texto {
            &r.lista_con_texto
        } else {
            &r.lista_sin_texto
        },
        &n,
    );
    Fase::Lista {
        texto: con(&r.lista, proyecto),
        sub: if estado.video_guardado {
            format!("{} {sub}", r.video_guardado)
        } else {
            sub
        },
        desde: Instant::now(),
    }
}

fn pulsar(
    a: Accion,
    pedido: &Pedido,
    r: &Rotulos,
    estado: &mut Estado,
    monitor: u32,
    vivo: &mut bool,
) {
    if a != Accion::Tirar {
        estado.tirar_desde = None;
    }
    match (a, &estado.fase) {
        (Accion::Microfono, Fase::Preparar) => estado.microfono = !estado.microfono,
        (Accion::Equipo, Fase::Preparar) => estado.equipo = !estado.equipo,
        (Accion::Video, Fase::Preparar) => estado.video = !estado.video,
        (Accion::Calidad, Fase::Preparar) => estado.perfil_video = estado.perfil_video.siguiente(),
        (Accion::Proyecto, Fase::Preparar) => {
            if !pedido.proyectos.is_empty() {
                estado.proyecto = (estado.proyecto + 1) % pedido.proyectos.len();
                estado.temporal = super::temporal_de_la_voz(
                    pedido.ubicacion.raiz(),
                    &pedido.proyectos[estado.proyecto].0,
                );
            }
        }
        (Accion::Empezar, Fase::Preparar) => {
            if !estado.microfono && !estado.equipo {
                estado.avisar(r.nada.clone());
                return;
            }
            let fuentes = pixpin_audio::Fuentes {
                microfono: estado.microfono,
                equipo: estado.equipo,
            };
            // Con video, la grabadora le manda una copia de lo que oye; el
            // video empieza cuando la grabadora ya esta abierta.
            let copia = if estado.video {
                let (tx, rx) = channel();
                estado.sonido_para_video = Some(rx);
                estado.monitor_video = monitor;
                Some(tx)
            } else {
                None
            };
            estado.fase = Fase::Abriendo(crate::ventanita::abrir_audio_con_copia(
                estado.temporal.clone(),
                fuentes,
                copia,
            ));
        }
        (Accion::Cerrar, Fase::Preparar) | (Accion::Listo, _) => *vivo = false,
        // Cerrar mientras se pasa a texto: `Estado::cerrar` lo deja de fondo.
        (Accion::DeFondo, Fase::Texto { .. }) => *vivo = false,
        (Accion::Captura, Fase::Grabando(g)) => {
            let ms = g.llevado_ms();
            match estado.zona {
                Some(zona) if estado.pidiendo_zona.is_none() => {
                    if !estado.fotografo.zona(ms, zona) {
                        estado.avisar(r.sin_zona.clone());
                    }
                }
                // Sin zona, o eligiendola todavia: `pedir_zona` sabe que no
                // hay que pedirla dos veces.
                _ => pedir_zona(estado, ms, r),
            }
        }
        (Accion::Zona, Fase::Grabando(g)) => {
            let ms = g.llevado_ms();
            pedir_zona(estado, ms, r);
        }
        (Accion::Pausa, Fase::Grabando(g)) => {
            let pausa = !g.en_pausa();
            g.pausar(pausa);
            if let Some(v) = &estado.grabando_video {
                v.pausar(pausa);
            }
        }
        (Accion::Terminar, Fase::Grabando(_)) => {
            if let Fase::Grabando(g) = std::mem::replace(&mut estado.fase, Fase::Preparar) {
                estado.fase = Fase::Parando {
                    audio: crate::ventanita::cerrar_microfono(g),
                    listo: None,
                };
            }
        }
        (Accion::Tirar, Fase::Grabando(_)) => {
            if estado.tirar_desde.is_some() {
                if let Fase::Grabando(g) = std::mem::replace(&mut estado.fase, Fase::Preparar) {
                    g.cancelar();
                }
                if let Some(v) = estado.grabando_video.take() {
                    v.tirar();
                }
                let _ = estado.fotografo.acabar(Duration::from_secs(2));
                estado.capturas.clear();
                tracing::info!("clase tirada sin guardar");
                *vivo = false;
            } else {
                estado.tirar_desde = Some(Instant::now());
            }
        }
        // Mientras se pasa a texto, «Sin texto» lo deja: la nota ya esta
        // guardada.
        (Accion::Tirar, Fase::Texto { en_marcha, .. }) => {
            en_marcha.cancelar();
            let nombre = pedido
                .proyectos
                .get(estado.proyecto)
                .map(|(_, n)| n.clone())
                .unwrap_or_default();
            estado.fase = lista(r, &nombre, estado, false);
        }
        _ => {}
    }
}

// --- Pintar ---------------------------------------------------------------

fn con_alfa(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

fn mas_claro(c: Color) -> Color {
    Color {
        r: (c.r + 0.08).min(1.0),
        g: (c.g + 0.08).min(1.0),
        b: (c.b + 0.08).min(1.0),
        a: c.a,
    }
}

/// Un boton redondeado con icono y, si cabe, rotulo.
#[allow(clippy::too_many_arguments)]
fn boton(
    p: &Pintor,
    botones: &mut Botones<Accion>,
    caja: RectF,
    que: Accion,
    icono: &pixpin_render::icono::Icono,
    rotulo: &str,
    fondo: Color,
    tinta: Color,
    e: f32,
) {
    boton_si(p, botones, caja, que, icono, rotulo, fondo, tinta, e, true);
}

/// [`boton`] que, sin `activo`, se pinta apagado y no se puede pulsar: un
/// boton que se ve y no hace nada parece roto (la barra mientras se abre el
/// audio).
#[allow(clippy::too_many_arguments)]
fn boton_si(
    p: &Pintor,
    botones: &mut Botones<Accion>,
    caja: RectF,
    que: Accion,
    icono: &pixpin_render::icono::Icono,
    rotulo: &str,
    fondo: Color,
    tinta: Color,
    e: f32,
    activo: bool,
) {
    let (fondo, tinta) = if !activo {
        (con_alfa(fondo, 0.35), con_alfa(tinta, 0.45))
    } else if dentro(caja, botones.raton) {
        (mas_claro(fondo), tinta)
    } else {
        (fondo, tinta)
    };
    p.rellenar_redondeado(caja, 10.0 * e, fondo);
    let lado = 20.0 * e;
    let tam = 14.0 * e;
    let (tw, th) = if rotulo.is_empty() {
        (0.0, 0.0)
    } else {
        p.medir_texto(rotulo, tam)
    };
    let hueco = if rotulo.is_empty() { 0.0 } else { 8.0 * e };
    let x = caja.x + (caja.ancho - lado - hueco - tw) / 2.0;
    p.icono(
        icono,
        RectF {
            x,
            y: caja.y + (caja.alto - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        tinta,
    );
    if !rotulo.is_empty() {
        p.texto(
            rotulo,
            x + lado + hueco,
            caja.y + (caja.alto - th) / 2.0,
            tam,
            tinta,
        );
    }
    if activo {
        botones.zona(caja, que);
    }
}

#[allow(clippy::too_many_arguments)]
fn pintar(
    p: &Pintor,
    marco: Rect,
    e: f32,
    estado: &Estado,
    pedido: &Pedido,
    r: &Rotulos,
    llevado: Duration,
    botones: &mut Botones<Accion>,
) {
    let tema = &OSCURO;
    let (w, h) = (marco.ancho as f32, marco.alto as f32);
    p.limpiar_transparente();
    p.rellenar_redondeado(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho: w,
            alto: h,
        },
        18.0 * e,
        tema.isla_borde,
    );
    p.rellenar_redondeado(
        RectF {
            x: 1.0,
            y: 1.0,
            ancho: w - 2.0,
            alto: h - 2.0,
        },
        17.0 * e,
        tema.isla,
    );
    botones.vaciar();
    let todo = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: h,
    };
    match &estado.fase {
        Fase::Preparar => pintar_preparar(p, w, e, estado, pedido, r, botones),
        Fase::Abriendo(_) | Fase::Grabando(_) => pintar_barra(p, w, e, estado, r, llevado, botones),
        Fase::Parando { .. } => {
            centrado_en(p, &r.guardando, todo, 70.0 * e, 16.0 * e, tema.texto);
        }
        Fase::Texto { en_marcha, .. } => {
            let avance = en_marcha.avance();
            let pct = ((avance * 100.0).round() as i64).to_string();
            let texto = con(
                if en_marcha.bajando() {
                    &r.bajando
                } else {
                    &r.texto
                },
                &pct,
            );
            p.icono(
                &mi::SUBTITLES,
                RectF {
                    x: (w - 30.0 * e) / 2.0,
                    y: 16.0 * e,
                    ancho: 30.0 * e,
                    alto: 30.0 * e,
                },
                tema.enviar,
            );
            centrado_en(p, &texto, todo, 56.0 * e, 15.0 * e, tema.texto);
            let barra = RectF {
                x: 24.0 * e,
                y: 88.0 * e,
                ancho: w - 48.0 * e,
                alto: 6.0 * e,
            };
            p.rellenar_redondeado(barra, 3.0 * e, tema.campo);
            p.rellenar_redondeado(
                RectF {
                    ancho: (barra.ancho * avance.clamp(0.0, 1.0)).max(6.0 * e),
                    ..barra
                },
                3.0 * e,
                tema.enviar,
            );
            // «Sin texto» lo deja; «Cerrar» se va y el texto sigue de fondo.
            let ancho = (w - 24.0 * e * 2.0 - 8.0 * e) / 2.0;
            boton(
                p,
                botones,
                RectF {
                    x: 24.0 * e,
                    y: 112.0 * e,
                    ancho,
                    alto: 38.0 * e,
                },
                Accion::Tirar,
                &mi::CLOSE,
                &r.sin_texto,
                tema.campo,
                tema.texto,
                e,
            );
            boton(
                p,
                botones,
                RectF {
                    x: 24.0 * e + ancho + 8.0 * e,
                    y: 112.0 * e,
                    ancho,
                    alto: 38.0 * e,
                },
                Accion::DeFondo,
                &mi::CHECK_CIRCLE,
                &r.de_fondo,
                tema.campo,
                tema.texto,
                e,
            );
        }
        Fase::Lista { texto, sub, .. } => pintar_final(
            p,
            w,
            e,
            &mi::CHECK_CIRCLE,
            VERDE,
            texto,
            Some(sub),
            r,
            botones,
        ),
        Fase::Fallo { texto, .. } => {
            pintar_final(p, w, e, &mi::MIC, ROJO_GRABAR, texto, None, r, botones)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn pintar_final(
    p: &Pintor,
    w: f32,
    e: f32,
    icono: &pixpin_render::icono::Icono,
    color: Color,
    texto: &str,
    sub: Option<&str>,
    r: &Rotulos,
    botones: &mut Botones<Accion>,
) {
    let tema = &OSCURO;
    let lado = 30.0 * e;
    p.icono(
        icono,
        RectF {
            x: (w - lado) / 2.0,
            y: 14.0 * e,
            ancho: lado,
            alto: lado,
        },
        color,
    );
    let ancho_max = w - 36.0 * e;
    let (tw, th) = p.medir_texto_ajustado(texto, 15.0 * e, ancho_max);
    p.texto_ajustado(
        texto,
        (w - tw) / 2.0,
        52.0 * e,
        15.0 * e,
        ancho_max,
        tema.texto,
    );
    if let Some(sub) = sub {
        let (sw, _) = p.medir_texto_ajustado(sub, 12.5 * e, ancho_max);
        p.texto_ajustado(
            sub,
            (w - sw) / 2.0,
            56.0 * e + th,
            12.5 * e,
            ancho_max,
            tema.campo_apagado,
        );
    }
    botones.boton(
        p,
        RectF {
            x: (w - 120.0 * e) / 2.0,
            y: 122.0 * e,
            ancho: 120.0 * e,
            alto: 34.0 * e,
        },
        Accion::Listo,
        &r.listo,
        None,
        e,
    );
}

/// Una fila de la fase de preparar, a `y` (logico) desde arriba.
fn filas_de_preparar(w: f32, e: f32, y: f32) -> RectF {
    let m = 18.0 * e;
    RectF {
        x: m,
        y: y * e,
        ancho: w - 2.0 * m,
        alto: 44.0 * e,
    }
}

/// **La ficha de la calidad del video**, dentro de su fila y a la izquierda
/// de la pildora: tan ancha como su rotulo.
fn ficha_de_calidad(p: &Pintor, fila: RectF, rotulo: &str, e: f32) -> RectF {
    let (tw, _) = p.medir_texto(rotulo, 12.0 * e);
    let ancho = tw + 16.0 * e;
    let alto = 24.0 * e;
    // La pildora mide 38 y va a 12 del borde (ver `interruptor`).
    RectF {
        x: fila.x + fila.ancho - 50.0 * e - 8.0 * e - ancho,
        y: fila.y + (fila.alto - alto) / 2.0,
        ancho,
        alto,
    }
}

/// Un interruptor de la fase de preparar: icono, rotulo y la pildora.
/// `reservado`: lo que el rotulo deja libre a la derecha, antes de la
/// pildora (la ficha de la calidad del video).
#[allow(clippy::too_many_arguments)]
fn interruptor(
    p: &Pintor,
    botones: &mut Botones<Accion>,
    caja: RectF,
    que: Accion,
    icono: &pixpin_render::icono::Icono,
    rotulo: &str,
    encendido: bool,
    reservado: f32,
    e: f32,
) {
    let tema = &OSCURO;
    let fondo = if dentro(caja, botones.raton) {
        mas_claro(tema.campo)
    } else {
        tema.campo
    };
    p.rellenar_redondeado(caja, 10.0 * e, fondo);
    let lado = 20.0 * e;
    p.icono(
        icono,
        RectF {
            x: caja.x + 12.0 * e,
            y: caja.y + (caja.alto - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        if encendido {
            tema.texto
        } else {
            tema.campo_apagado
        },
    );
    let tam = 14.0 * e;
    let (_, th) = p.medir_texto(rotulo, tam);
    p.texto_linea(
        rotulo,
        caja.x + 42.0 * e,
        caja.y + (caja.alto - th) / 2.0,
        tam,
        caja.ancho - 100.0 * e - reservado,
        tema.texto,
    );
    // La pildora: verde y a la derecha si esta encendido.
    let pw = 38.0 * e;
    let ph = 22.0 * e;
    let pild = RectF {
        x: caja.x + caja.ancho - pw - 12.0 * e,
        y: caja.y + (caja.alto - ph) / 2.0,
        ancho: pw,
        alto: ph,
    };
    p.rellenar_redondeado(
        pild,
        ph / 2.0,
        if encendido { VERDE } else { tema.campo_apagado },
    );
    let radio = ph / 2.0 - 3.0 * e;
    let cx = if encendido {
        pild.x + pw - ph / 2.0
    } else {
        pild.x + ph / 2.0
    };
    p.circulo((cx, pild.y + ph / 2.0), radio, Color::BLANCO);
    botones.zona(caja, que);
}

fn pintar_preparar(
    p: &Pintor,
    w: f32,
    e: f32,
    estado: &Estado,
    pedido: &Pedido,
    r: &Rotulos,
    botones: &mut Botones<Accion>,
) {
    let tema = &OSCURO;
    let m = 18.0 * e;
    p.texto_linea(
        &r.titulo,
        m,
        16.0 * e,
        17.0 * e,
        w - 2.0 * m - 30.0 * e,
        tema.texto,
    );
    // Con el raton sobre la ficha de la calidad, la linea de abajo del
    // titulo dice que es cada una (lo que pesa una hora): es su pista.
    let sobre_la_calidad = estado.video
        && dentro(
            ficha_de_calidad(
                p,
                filas_de_preparar(w, e, 176.0),
                r.calidad(estado.perfil_video).0,
                e,
            ),
            botones.raton,
        );
    p.texto_linea(
        if sobre_la_calidad {
            r.calidad(estado.perfil_video).1
        } else {
            &r.sub
        },
        m,
        42.0 * e,
        12.0 * e,
        w - 2.0 * m,
        tema.campo_apagado,
    );
    // ✕ arriba a la derecha.
    let x = RectF {
        x: w - 40.0 * e,
        y: 12.0 * e,
        ancho: 28.0 * e,
        alto: 28.0 * e,
    };
    if dentro(x, botones.raton) {
        p.rellenar_redondeado(x, 8.0 * e, tema.campo);
    }
    p.icono(
        &mi::CLOSE,
        RectF {
            x: x.x + 5.0 * e,
            y: x.y + 5.0 * e,
            ancho: 18.0 * e,
            alto: 18.0 * e,
        },
        tema.campo_apagado,
    );
    botones.zona(x, Accion::Cerrar);

    let fila = |y: f32| filas_de_preparar(w, e, y);
    interruptor(
        p,
        botones,
        fila(72.0),
        Accion::Microfono,
        &mi::MIC,
        &r.microfono,
        estado.microfono,
        0.0,
        e,
    );
    interruptor(
        p,
        botones,
        fila(124.0),
        Accion::Equipo,
        &mi::VOLUME_UP,
        &r.equipo,
        estado.equipo,
        0.0,
        e,
    );
    // Apagado de salida: el video ocupa mucho y casi siempre basta la nota
    // con sus capturas. Encendido, lleva a la derecha su calidad, que se
    // cambia con un clic (ver `ficha_de_calidad`).
    let caja_video = fila(176.0);
    let (calidad, _) = r.calidad(estado.perfil_video);
    let ficha = estado
        .video
        .then(|| ficha_de_calidad(p, caja_video, calidad, e));
    interruptor(
        p,
        botones,
        caja_video,
        Accion::Video,
        &mi::VIDEOCAM,
        &r.video,
        estado.video,
        ficha.map_or(0.0, |f| f.ancho + 8.0 * e),
        e,
    );
    if let Some(f) = ficha {
        // Dos tonos por encima de la fila (que ya se aclara uno con el
        // raton encima), y uno mas sobre la propia ficha.
        let fondo = mas_claro(mas_claro(tema.campo));
        let encima = dentro(f, botones.raton);
        p.rellenar_redondeado(
            f,
            f.alto / 2.0,
            if encima { mas_claro(fondo) } else { fondo },
        );
        let tam = 12.0 * e;
        let (tw, th) = p.medir_texto(calidad, tam);
        p.texto(
            calidad,
            f.x + (f.ancho - tw) / 2.0,
            f.y + (f.alto - th) / 2.0,
            tam,
            tema.texto,
        );
        // Encima del rotulo del video: la zona de la ficha va despues y
        // gana (`Botones::bajo_el_raton`).
        botones.zona(f, Accion::Calidad);
    }
    // Donde se guarda: un clic pasa al siguiente proyecto.
    let caja = fila(228.0);
    let fondo = if dentro(caja, botones.raton) {
        mas_claro(tema.campo)
    } else {
        tema.campo
    };
    p.rellenar_redondeado(caja, 10.0 * e, fondo);
    let lado = 20.0 * e;
    p.icono(
        &mi::FOLDER,
        RectF {
            x: caja.x + 12.0 * e,
            y: caja.y + (caja.alto - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        tema.carpeta,
    );
    let nombre = pedido
        .proyectos
        .get(estado.proyecto)
        .map(|(_, n)| n.as_str())
        .unwrap_or("");
    let rotulo = con(&r.guardar_en, nombre);
    let tam = 14.0 * e;
    let (_, th) = p.medir_texto(&rotulo, tam);
    p.texto_linea(
        &rotulo,
        caja.x + 42.0 * e,
        caja.y + (caja.alto - th) / 2.0,
        tam,
        caja.ancho - 80.0 * e,
        tema.texto,
    );
    if pedido.proyectos.len() > 1 {
        p.icono(
            &mi::CHEVRON_RIGHT,
            RectF {
                x: caja.x + caja.ancho - 32.0 * e,
                y: caja.y + (caja.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            tema.campo_apagado,
        );
    }
    botones.zona(caja, Accion::Proyecto);

    // El boton grande.
    let caja = RectF {
        x: m,
        y: 288.0 * e,
        ancho: w - 2.0 * m,
        alto: 50.0 * e,
    };
    let fondo = if dentro(caja, botones.raton) {
        mas_claro(ROJO_GRABAR)
    } else {
        ROJO_GRABAR
    };
    p.rellenar_redondeado(caja, 14.0 * e, fondo);
    let tam = 16.0 * e;
    let (tw, th) = p.medir_texto(&r.empezar, tam);
    let punto = 12.0 * e;
    let x0 = caja.x + (caja.ancho - tw - punto - 10.0 * e) / 2.0;
    p.circulo(
        (x0 + punto / 2.0, caja.y + caja.alto / 2.0),
        punto / 2.0,
        Color::BLANCO,
    );
    p.texto(
        &r.empezar,
        x0 + punto + 10.0 * e,
        caja.y + (caja.alto - th) / 2.0,
        tam,
        Color::BLANCO,
    );
    botones.zona(caja, Accion::Empezar);
    if let Some((aviso, _)) = &estado.aviso {
        centrado_en(
            p,
            aviso,
            RectF {
                x: 0.0,
                y: 0.0,
                ancho: w,
                alto: 1.0,
            },
            346.0 * e,
            12.0 * e,
            ROJO_GRABAR,
        );
    }
}

fn pintar_barra(
    p: &Pintor,
    w: f32,
    e: f32,
    estado: &Estado,
    r: &Rotulos,
    llevado: Duration,
    botones: &mut Botones<Accion>,
) {
    let tema = &OSCURO;
    let g = match &estado.fase {
        Fase::Grabando(g) => Some(g),
        _ => None,
    };
    let pausa = g.is_some_and(|g| g.en_pausa());
    // Mientras se abre el audio los botones se ven, pero apagados.
    let activo = g.is_some();
    let latido = (llevado.as_millis() % 1000) as f32 / 1000.0;
    let alto = 44.0 * e;
    let y = 10.0 * e;
    // ● 12:34
    let tiempo = g
        .map(|g| pixpin_audio::duracion_legible(g.llevado_ms()))
        .unwrap_or_else(|| "0:00".into());
    let punto = 10.0 * e;
    let tam = 18.0 * e;
    let (_, th) = p.medir_texto(&tiempo, tam);
    let x = 16.0 * e;
    if g.is_some() && (pausa || latido < 0.6) {
        p.circulo(
            (x + punto / 2.0, y + alto / 2.0),
            punto / 2.0,
            if pausa {
                tema.campo_apagado
            } else {
                ROJO_GRABAR
            },
        );
    }
    p.texto(
        &tiempo,
        x + punto + 8.0 * e,
        y + (alto - th) / 2.0,
        tam,
        tema.texto,
    );
    // El nivel: cinco barras que dicen que de verdad entra sonido.
    let nivel = g
        .map(|g| (g.nivel() as f32 / pixpin_audio::PICO_MAXIMO as f32).clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let x_nivel = 104.0 * e;
    for n in 0..5 {
        let forma = [0.5, 0.8, 1.0, 0.8, 0.5][n];
        let medio = (13.0 * e * nivel.sqrt() * forma).max(1.0 * e);
        p.rellenar_redondeado(
            RectF {
                x: x_nivel + n as f32 * 6.0 * e,
                y: y + alto / 2.0 - medio,
                ancho: 3.0 * e,
                alto: medio * 2.0,
            },
            1.5 * e,
            if pausa {
                tema.campo_apagado
            } else {
                tema.enviar
            },
        );
    }

    // Los botones, de izquierda a derecha.
    let mut x = 146.0 * e;
    let mut sitio = |ancho: f32| {
        let c = RectF {
            x,
            y,
            ancho: ancho * e,
            alto,
        };
        x += (ancho + 8.0) * e;
        c
    };
    let destello = estado
        .destello
        .is_some_and(|d| d.elapsed() < Duration::from_millis(600));
    // Las que aun se estan haciendo cuentan ya: el clic se vio.
    let hechas = estado.capturas.len() + estado.fotografo.pendientes();
    let rotulo_captura = if hechas == 0 {
        r.captura.clone()
    } else {
        format!("{} · {}", r.captura, hechas)
    };
    boton_si(
        p,
        botones,
        sitio(128.0),
        Accion::Captura,
        &mi::IMAGE,
        &rotulo_captura,
        if destello { VERDE } else { tema.enviar },
        Color::BLANCO,
        e,
        activo,
    );
    boton_si(
        p,
        botones,
        sitio(44.0),
        Accion::Zona,
        &mi::CROP_FREE,
        "",
        tema.campo,
        tema.texto,
        e,
        activo,
    );
    boton_si(
        p,
        botones,
        sitio(44.0),
        Accion::Pausa,
        if pausa { &mi::PLAY_ARROW } else { &mi::PAUSE },
        "",
        tema.campo,
        tema.texto,
        e,
        activo,
    );
    boton_si(
        p,
        botones,
        sitio(108.0),
        Accion::Terminar,
        &mi::STOP,
        &r.terminar,
        ROJO_GRABAR,
        Color::BLANCO,
        e,
        activo,
    );
    let tirar = estado.tirar_desde.is_some();
    boton_si(
        p,
        botones,
        sitio(36.0),
        Accion::Tirar,
        &mi::CLOSE,
        "",
        if tirar { ROJO_GRABAR } else { tema.campo },
        if tirar {
            Color::BLANCO
        } else {
            tema.campo_apagado
        },
        e,
        activo,
    );

    // La linea de abajo: lo que acaba de pasar, o la pista.
    let (linea, color) = if g.is_none() {
        (r.abriendo.clone(), tema.campo_apagado)
    } else if tirar {
        (r.tirar.clone(), ROJO_GRABAR)
    } else if let Some((a, _)) = &estado.aviso {
        (a.clone(), tema.texto)
    } else if pausa {
        (r.en_pausa.clone(), tema.campo_apagado)
    } else if hechas == 0 {
        (r.pista.clone(), tema.campo_apagado)
    } else {
        (con(&r.capturas, &hechas.to_string()), tema.campo_apagado)
    };
    // Con video, a la derecha de la linea de abajo: que se esta grabando
    // la pantalla tiene que verse siempre.
    let mut ancho_linea = w - 32.0 * e;
    if estado.grabando_video.is_some() {
        let roto = estado.grabando_video.as_ref().is_some_and(|v| v.roto());
        let tam = 12.0 * e;
        let (tw, th) = p.medir_texto(&r.video_corto, tam);
        let lado = 14.0 * e;
        let x = w - 16.0 * e - tw;
        let tinta = if roto || pausa {
            tema.campo_apagado
        } else {
            ROJO_GRABAR
        };
        p.icono(
            &mi::VIDEOCAM,
            RectF {
                x: x - lado - 4.0 * e,
                y: 62.0 * e + (th - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            tinta,
        );
        p.texto(&r.video_corto, x, 62.0 * e, tam, tinta);
        ancho_linea -= tw + lado + 16.0 * e;
    }
    p.texto_linea(
        &linea,
        16.0 * e,
        62.0 * e,
        12.0 * e,
        ancho_linea,
        con_alfa(color, 0.95),
    );
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_minuto_de_una_captura_se_lee_como_un_reloj() {
        assert_eq!(minuto(0), "0:00");
        assert_eq!(minuto(65_400), "1:05");
        assert_eq!(minuto(3_723_000), "1:02:03");
        // Caso negativo: un reloj que se fue atras no da minutos negativos.
        assert_eq!(minuto(-5), "0:00");
    }

    #[test]
    fn la_fecha_del_nombre_no_lleva_dos_puntos() {
        // 24-oct-2026 18:05 en hora local.
        let dias = 20_750; // 2026-10-24
        let ms = dias * 86_400_000 + (18 * 60 + 5) * 60_000;
        let f = fecha_del_nombre(ms);
        assert_eq!(f, "24-10 18h05");
        assert!(!f.contains(':'));
    }

    #[test]
    fn las_capturas_van_y_vuelven_en_orden() {
        let mut m = cuaderno::Mensaje::default();
        m.resto.insert(
            "capturas".into(),
            capturas_en_json(&[(9_000, "b".into()), (2_000, "a".into())]),
        );
        let v = capturas_de(&m);
        assert_eq!(v, vec![(2_000, "a".to_string()), (9_000, "b".to_string())]);
        // Caso negativo: una nota sin capturas no tiene ninguna, y una
        // entrada rota se salta sin llevarse las demas.
        assert!(capturas_de(&cuaderno::Mensaje::default()).is_empty());
        m.resto.insert(
            "capturas".into(),
            serde_json::json!([{ "ms": "x" }, { "ms": 5, "id": "c" }]),
        );
        assert_eq!(capturas_de(&m), vec![(5, "c".to_string())]);
    }

    #[test]
    fn la_barra_es_tan_ancha_como_sus_botones() {
        // El fallo: la barra medía 520 y sus botones pedían 538; el ✕ de
        // tirar salía cortado. 146 de tiempo y nivel, 392 de botones (ver
        // `pintar_barra`) y un margen.
        let barra = 146 + (128 + 8) + (44 + 8) + (44 + 8) + (108 + 8) + 36;
        let (_, rx) = channel();
        let (w, _) = tamano(&Fase::Abriendo(rx));
        assert!(w >= barra + 12, "{w} < {barra}");
    }

    #[test]
    fn al_arrastrar_a_otra_pantalla_se_reencaja_sin_salirse() {
        let area = Rect {
            x: 1920,
            y: 0,
            ancho: 1280,
            alto: 1000,
        };
        // Soltada medio fuera por la derecha, en una pantalla al 150 %.
        let antes = Rect {
            x: 3100,
            y: 500,
            ancho: 340,
            alto: 370,
        };
        let m = reencajar(area, 150, &Fase::Preparar, antes);
        assert_eq!((m.ancho, m.alto), (510, 555), "la escala de la nueva");
        assert!(m.x >= area.x && m.x + m.ancho as i32 <= area.x + area.ancho as i32);
        assert!(m.y >= area.y && m.y + m.alto as i32 <= area.y + area.alto as i32);
        // Caso negativo: dentro y a la misma escala no se mueve.
        let quieta = Rect {
            x: 2200,
            y: 100,
            ancho: 340,
            alto: 370,
        };
        assert_eq!(reencajar(area, 100, &Fase::Preparar, quieta), quieta);
    }

    #[test]
    fn el_video_no_pisa_otro_con_el_mismo_nombre() {
        let dir = std::env::temp_dir().join(format!(
            "pixpin-clase-nombre-{}",
            pixpin_shell::entorno::ahora_utc_ms()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(nombre_libre(&dir, "Clase 1.mp4"), "Clase 1.mp4");
        std::fs::write(dir.join("Clase 1.mp4"), b"x").unwrap();
        assert_eq!(nombre_libre(&dir, "Clase 1.mp4"), "Clase 1 (1).mp4");
        std::fs::write(dir.join("Clase 1 (1).mp4"), b"x").unwrap();
        assert_eq!(nombre_libre(&dir, "Clase 1.mp4"), "Clase 1 (2).mp4");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_captura_del_momento_es_la_ultima_hecha_antes() {
        let c = vec![(2_000, "a".to_string()), (9_000, "b".to_string())];
        assert_eq!(
            captura_en(&c, 500),
            Some(0),
            "antes de la primera, la primera"
        );
        assert_eq!(captura_en(&c, 2_000), Some(0));
        assert_eq!(captura_en(&c, 8_999), Some(0));
        assert_eq!(captura_en(&c, 9_000), Some(1));
        assert_eq!(captura_en(&c, 99_000), Some(1));
        assert_eq!(captura_en(&[], 1_000), None);
    }
}
