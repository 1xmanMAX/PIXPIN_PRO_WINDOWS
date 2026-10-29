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
//! **Dos modos** (25-oct-2026, «que tenga diferentes modos: grabar la
//! pantalla con su audio … o el audio y capturas»), elegidos arriba del
//! todo en Preparar y recordados para la proxima vez ([`Modo`]):
//!
//! - **Pantalla y sonido**: el video de la pantalla donde esta la ventana,
//!   con los sonidos que se elijan (el del equipo, el microfono o los dos) y
//!   su calidad. «Captura» sigue siendo la de una zona, que se repite.
//! - **Audio y capturas**: solo el sonido, y «Captura» fotografia **la
//!   pantalla entera donde esta la barra**, al instante y sin preguntar (la
//!   barra no sale). El boton de la zona sigue para cuando se quiere solo
//!   un trozo.
//!
//! Al acabar, «Abrir para estudiar» abre la clase en su ventana
//! ([`super::estudiar_clase`]): el video o la captura del minuto, el texto
//! y las capturas, que se siguen.
//!
//! Las fotos se hacen en su propio hilo ([`fotografo`]): la barra no se
//! congela mientras se captura una zona grande.
//!
//! **Reunion presencial** (27-oct-2026, «supon que estoy en una reunion
//! PRESENCIAL: que siga grabando aunque haya salido de la app, que aparezca
//! un boton flotante para tomar FOTOS … y todo se adjunte a la reunion; y a
//! resolucion COMPLETA»): el tercer modo. Graba el microfono (el sonido del
//! equipo, apagado de salida) y el boton grande de la barra es **«Foto»**:
//! una foto con la camara del equipo a su resolucion entera ([`camara`],
//! `MediaCapture`), que entra como una captura mas (`IMAGEN` con
//! `enGrabacion {de, ms}` y `origen: "camara"`). Al lado, **«Video»**
//! graba un clip corto con la camara (1080p30 como mucho, HEVC si la
//! grafica lo codifica, si no H.264), que entra con `enGrabacion {de, ms,
//! duracionMs}`. Con varias camaras se elige en Preparar (la de atras de una
//! tableta va primero: mira a la sala) y se recuerda. Lo puro, en
//! `pixpin_proyecto::presencial`.
//!
//! **Sigue grabando sin PixPin delante**: la barra vive en su hilo, siempre
//! encima, y no depende de la ventana del chat (cerrarla no corta nada). El
//! icono de la bandeja dice «Grabando…» mientras tanto ([`grabando`]), y
//! «Salir» de la bandeja pregunta antes y, si se termina, guarda todo
//! ([`terminar_para_salir`]).
//!
//! **Resolucion entera**: las capturas van en PNG al tamano de la pantalla
//! (sus pixeles de verdad, sin escalar) y las fotos de la camara en el JPEG
//! que da a su resolucion mayor. Al chat entran los bytes tal cual
//! (`almacen::guardar_adjunto`); solo las miniaturas se reducen.
//!
//! **Sin perder nada** (28-oct-2026, «si se perdiera algo seria malo, se
//! perderia una reunion»): la nota se graba con su respaldo WAV
//! (`Grabadora::empezar_a_prueba_de_cortes`), el video en MP4 fragmentado, y
//! cada captura, foto o clip se apunta en el disco en el acto, en el diario
//! de la grabacion ([`a_salvo`]). Si PixPin muere, se va la luz o el disco
//! falla, al volver a abrirlo se ofrece «Recuperar / Descartar»
//! ([`recuperar`]). Ver `docs/grabar-sin-perder-nada.md`.
//!
//! **Los temas de la reunion** (28-oct-2026, «la persona crea una lista de
//! TEMAS a tratar; durante la reunion se queda escuchando y transcribiendo;
//! eso se manda a JEV y Jev marca que temas ya se trataron»): en Preparar,
//! la fila «Temas de la reunion»; grabando, «Temas 2/5» en la barra con su
//! lista para marcar a mano, y la escucha en vivo que le pregunta a Jev
//! ([`temas`], [`escucha`]; `docs/jev.md`).
//!
//! **Lo demas de la reunion** (29-oct-2026): la misma escucha, con temas o
//! sin ellos, propone en la barra las tareas y los eventos dichos, aplica
//! (con «Deshacer») los cambios de urgencia de las tareas que ya existen,
//! avisa del tema olvidado cerca de la «Duración prevista» y apunta las
//! decisiones, las preguntas sin respuesta y los minutos de cada tema en la
//! nota ([`reunion`]).
//!
//! **Una sola grabacion** (29-oct-2026, lo que el usuario pidio en el movil
//! y se trae aqui: «solo deberia haber una opcion de grabar, no tres …
//! una opcion que funcione para los tres escenarios»; «el boton de play y
//! pausa sobra: en el tiempo, donde esta el puntito rojo, le damos clic y
//! pausa, y si lo mantengo presionado, que se termine»; «una opcion de
//! pantalla completa donde vea la lista de lo que voy a decir, arriba las
//! opciones y abajo la transcripcion, mas grande»). Ya no hay modos:
//!
//! - **Preparar**: los sonidos, **«Empezar grabando la pantalla»** (con su
//!   calidad), la camara para Foto y Clip, el chat y los temas.
//! - **La barra**: el reloj «● 12:34» (un clic pausa o sigue y el punto
//!   pasa a ❚❚; **mantenerlo termina** y guarda), **Foto** (la camara del
//!   equipo, que se abre a la primera), **Captura** (la pantalla entera al
//!   instante; **mantenerla** o el clic derecho, una zona), **Pantalla**
//!   (enciende o apaga el video de la pantalla mientras el audio sigue,
//!   [`pantalla`]), **Clip** (un clip con la camara), los temas, ⛶ la
//!   pantalla completa y ✕ tirar.
//! - **La pantalla completa** ([`completa`]): arriba el reloj grande,
//!   «Terminar» y las cuatro acciones; en medio lo que voy a decir (los
//!   temas, para marcarlos a mano) y abajo lo que se dice (la escucha en
//!   vivo, frase a frase con su minuto), con A− y A+ y «Descartar».
//!
//! El formato no cambia: la misma nota `grabacion: "clase"` con
//! `capturas`, `video` y `clips`. Ver `docs/grabacion-unica.md`.

#![forbid(unsafe_code)]

mod a_salvo;
mod camara;
mod cola_de_texto;
mod completa;
mod escucha;
mod fotografo;
mod pantalla;
mod recuperar;
mod reunion;
mod temas;
mod video;

pub(crate) use recuperar::{TextosRecuperar, al_arrancar as recuperar_al_arrancar};

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_codec::imagen::ImagenRgba;
use pixpin_geom::{Punto, Rect};
use pixpin_motor_video::{Codificador, Perfil};
use pixpin_proyecto::presencial::{self, FalloDeCamara};
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
/// Lo que hay que mantener el reloj (terminar) o «Captura» (una zona).
const MANTENER: Duration = Duration::from_millis(650);

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
    /// Lo que dice debajo del titulo: la grabacion es una sola.
    sub: String,
    /// «Empezar grabando la pantalla» de Preparar.
    pantalla_al_empezar: String,
    /// «Pantalla» y «Clip» de la barra, y lo que se dice al encender o
    /// apagar la pantalla.
    pantalla: String,
    clip: String,
    pantalla_encendida: String,
    pantalla_apagada: String,
    /// La pista de la barra: el reloj, «Captura» mantenida.
    pista_unica: String,
    /// «N imágenes y clips», con [`HUECO`] en el numero.
    imagenes: String,
    /// La pantalla completa ([`completa`]).
    completa_temas: String,
    completa_sin_temas: String,
    completa_dice: String,
    completa_sin_frases: String,
    completa_sin_escucha: String,
    descartar: String,
    descartar_otra: String,
    /// La fila de la camara en Preparar: «Cámara: …», con [`HUECO`].
    camara: String,
    buscando_camaras: String,
    sin_camaras: String,
    /// «Foto» de la barra.
    foto: String,
    foto_hecha: String,
    abriendo_camara: String,
    camara_lista: String,
    /// Los fallos de la camara, en el orden de `FalloDeCamara`.
    camara_sin: String,
    camara_en_uso: String,
    camara_sin_permiso: String,
    camara_fallo: String,
    clip_empezado: String,
    clip_guardado: String,
    clip_corto: String,
    estudiar: String,
    microfono: String,
    equipo: String,
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
    /// El disco se llena (menos de 2 GB) y, casi lleno, el video se cerro.
    disco_poco: String,
    disco_lleno: String,
    /// «El equipo se suspendió N min…», con [`HUECO`] en los minutos.
    suspendido: String,
    /// Los temas de la reunion ([`temas`]). Los que llevan dos cosas
    /// dentro traen [`HUECO`] y `temas::HUECO2`.
    temas_ninguno: String,
    temas_n: String,
    temas_titulo: String,
    temas_sub: String,
    temas_escribe: String,
    temas_pegar: String,
    temas_de_lista: String,
    temas_lista: String,
    temas_sin_listas: String,
    temas_pegados: String,
    temas_nada_que_pegar: String,
    temas_vaciar: String,
    temas_listo: String,
    temas_anteriores: String,
    temas_tratado: String,
    temas_escuchando: String,
    temas_sin_jev: String,
    temas_apagado: String,
    temas_sin_modelo: String,
    temas_jev_fallo: String,
    temas_cargado: String,
    /// «Temas · {nombre}»: el titulo de la lista de tareas de la reunion.
    temas_lista_de: String,
    /// Lo de las fichas de la reunion ([`reunion`]).
    reunion: reunion::Rotulos,
}

const HUECO: &str = "\u{1}";

impl Rotulos {
    pub(crate) fn de(t: &Catalogo) -> Rotulos {
        let con = |clave: &str, nombre: &str| {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set(nombre.to_string(), HUECO);
            t.t_args(clave, &args)
        };
        let con2 = |clave: &str, uno: &str, dos: &str| {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set(uno.to_string(), HUECO);
            args.set(dos.to_string(), temas::HUECO2);
            t.t_args(clave, &args)
        };
        Rotulos {
            titulo: t.t("clase-titulo"),
            sub: t.t("clase-unica-sub"),
            pantalla_al_empezar: t.t("clase-pantalla-al-empezar"),
            pantalla: t.t("clase-pantalla"),
            clip: t.t("clase-clip"),
            pantalla_encendida: t.t("clase-pantalla-encendida"),
            pantalla_apagada: t.t("clase-pantalla-apagada"),
            pista_unica: t.t("clase-pista-unica"),
            imagenes: con("clase-imagenes", "n"),
            completa_temas: t.t("clase-completa-temas"),
            completa_sin_temas: t.t("clase-completa-sin-temas"),
            completa_dice: t.t("clase-completa-dice"),
            completa_sin_frases: t.t("clase-completa-sin-frases"),
            completa_sin_escucha: t.t("clase-completa-sin-escucha"),
            descartar: t.t("clase-descartar"),
            descartar_otra: t.t("clase-descartar-otra"),
            camara: con("clase-camara", "camara"),
            buscando_camaras: t.t("clase-camara-buscando"),
            sin_camaras: t.t("clase-camara-ninguna"),
            foto: t.t("clase-foto"),
            foto_hecha: con("clase-foto-hecha", "minuto"),
            abriendo_camara: t.t("clase-camara-abriendo"),
            camara_lista: con("clase-camara-lista", "medida"),
            camara_sin: t.t("clase-camara-sin"),
            camara_en_uso: t.t("clase-camara-en-uso"),
            camara_sin_permiso: t.t("clase-camara-sin-permiso"),
            camara_fallo: t.t("clase-camara-fallo"),
            clip_empezado: t.t("clase-clip-empezado"),
            clip_guardado: con("clase-clip-guardado", "minuto"),
            clip_corto: t.t("clase-clip-corto"),
            estudiar: t.t("clase-abrir-para-estudiar"),
            microfono: t.t("clase-microfono"),
            equipo: t.t("clase-equipo"),
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
            disco_poco: t.t("clase-disco-poco"),
            disco_lleno: t.t("clase-disco-lleno"),
            suspendido: con("clase-suspendido", "minutos"),
            temas_ninguno: t.t("clase-temas-ninguno"),
            temas_n: con("clase-temas-n", "n"),
            temas_titulo: t.t("clase-temas-titulo"),
            temas_sub: t.t("clase-temas-sub"),
            temas_escribe: t.t("clase-temas-escribe"),
            temas_pegar: t.t("clase-temas-pegar"),
            temas_de_lista: t.t("clase-temas-de-lista"),
            temas_lista: con2("clase-temas-lista", "lista", "n"),
            temas_sin_listas: t.t("clase-temas-sin-listas"),
            temas_pegados: con("clase-temas-pegados", "n"),
            temas_nada_que_pegar: t.t("clase-temas-nada-que-pegar"),
            temas_vaciar: t.t("clase-temas-vaciar"),
            temas_listo: t.t("clase-temas-listo"),
            temas_anteriores: con("clase-temas-anteriores", "n"),
            temas_tratado: con2("clase-temas-tratado", "n", "minuto"),
            temas_escuchando: t.t("clase-temas-escuchando"),
            temas_sin_jev: t.t("clase-temas-sin-jev"),
            temas_apagado: t.t("clase-temas-apagado"),
            temas_sin_modelo: t.t("clase-temas-sin-modelo"),
            temas_jev_fallo: t.t("clase-temas-jev-fallo"),
            temas_cargado: t.t("clase-temas-cargado"),
            temas_lista_de: con("clase-temas-lista-de", "nombre"),
            reunion: reunion::Rotulos::de(t),
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

    /// Lo que se dice cuando la camara no puede.
    fn de_la_camara(&self, f: FalloDeCamara) -> String {
        match f {
            FalloDeCamara::SinCamara => self.camara_sin.clone(),
            FalloDeCamara::EnUso => self.camara_en_uso.clone(),
            FalloDeCamara::SinPermiso => self.camara_sin_permiso.clone(),
            FalloDeCamara::Otro => self.camara_fallo.clone(),
        }
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

/// **Los modos de antes** (lo puro en `pixpin_proyecto::presencial`): ya no
/// se eligen (29-oct-2026, una sola grabacion), pero su fichero dice si se
/// empieza grabando la pantalla («pantalla») y con que sonidos (en persona,
/// sin el del equipo), y su palabra va al diario.
pub(super) use pixpin_proyecto::presencial::Modo;

/// Donde se recuerda: al lado de las notas de voz, en este equipo (no
/// viaja: es como se graba aqui).
fn fichero_del_modo(u: &Ubicacion) -> PathBuf {
    crate::voz::carpeta_de_audios(u).join("modo-de-grabar.txt")
}

fn modo_recordado(u: &Ubicacion) -> Modo {
    std::fs::read_to_string(fichero_del_modo(u))
        .map(|t| Modo::de(&t))
        .unwrap_or(Modo::Capturas)
}

/// Donde se recuerda lo grande de la letra de la pantalla completa.
fn fichero_de_la_letra(u: &Ubicacion) -> PathBuf {
    crate::voz::carpeta_de_audios(u).join("letra-de-grabar.txt")
}

/// Donde se recuerda la camara elegida (su identificador de Windows), al
/// lado de lo demas.
fn fichero_de_la_camara(u: &Ubicacion) -> PathBuf {
    crate::voz::carpeta_de_audios(u).join("camara-de-grabar.txt")
}

fn recordar_camara(u: &Ubicacion, id: &str) {
    if let Err(e) = std::fs::write(fichero_de_la_camara(u), id) {
        tracing::debug!(?e, "no se pudo recordar la camara");
    }
}

fn recordar_modo(u: &Ubicacion, m: Modo) {
    let f = fichero_del_modo(u);
    if let Some(d) = f.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if let Err(e) = std::fs::write(&f, m.palabra()) {
        tracing::debug!(?e, "no se pudo recordar el modo de grabar");
    }
}

fn letra_recordada(u: &Ubicacion) -> f32 {
    std::fs::read_to_string(fichero_de_la_letra(u))
        .ok()
        .and_then(|t| t.trim().parse::<f32>().ok())
        .map_or(1.0, completa::letra_valida)
}

fn recordar_letra(u: &Ubicacion, letra: f32) {
    if let Err(e) = std::fs::write(fichero_de_la_letra(u), format!("{letra:.2}")) {
        tracing::debug!(?e, "no se pudo recordar la letra de grabar");
    }
}

/// Una sola a la vez: dos grabaciones se pelearian por el microfono, y las
/// capturas no sabrian a cual van.
static ABIERTA: AtomicBool = AtomicBool::new(false);

/// Hay algo grabandose (abriendo el audio, grabando o cerrandolo): lo mira
/// la bandeja para su «Grabando…» y «Salir» para preguntar antes.
static GRABANDO: AtomicBool = AtomicBool::new(false);

/// «Salir» de la bandeja pidio terminar y guardar: la barra termina sin
/// pasar a texto y se cierra.
static TERMINAR_YA: AtomicBool = AtomicBool::new(false);

/// **Si se esta grabando una clase o reunion** ahora mismo.
pub(crate) fn grabando() -> bool {
    GRABANDO.load(Ordering::SeqCst)
}

/// **Termina y guarda la grabacion para salir de PixPin**: lo que hace
/// «Terminar», sin pasar a texto (se pide luego en la nota), esperando como
/// mucho `tope` a que este guardada. Devuelve si dio tiempo.
pub(crate) fn terminar_para_salir(tope: Duration) -> bool {
    if !ABIERTA.load(Ordering::SeqCst) {
        return true;
    }
    TERMINAR_YA.store(true, Ordering::SeqCst);
    let hasta = Instant::now() + tope;
    while ABIERTA.load(Ordering::SeqCst) {
        if Instant::now() >= hasta {
            tracing::warn!("la clase no termino de guardarse antes de salir");
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    true
}

/// Apunta si se graba y, si cambio, se lo dice a la bandeja.
fn poner_grabando(si: bool) {
    if GRABANDO.swap(si, Ordering::SeqCst) != si
        && !pixpin_shell::mensajero::pedir_ventana_principal(crate::ID_CLASE_EN_MARCHA)
    {
        tracing::debug!("la bandeja no se entero de la grabacion");
    }
}

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
    TERMINAR_YA.store(false, Ordering::SeqCst);
    let (tx, rx) = channel();
    *ZONA.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
    let hecho = std::thread::Builder::new()
        .name("grabar-clase".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            // Los datos viven fuera de `con_recursos`: si el dispositivo de
            // dibujo se pierde y la ventana se reabre, la grabacion sigue.
            let mut estado = Estado::nuevo(&pedido, rotulos.nombre.clone())
                .con_los_de_la_proxima(&pedido, &rotulos);
            estado.titulo_temas = rotulos.temas_lista_de.clone();
            if let Err(e) = crate::dispositivo_perdido::con_recursos("grabar clase", |r| {
                flotar(r, &pedido, &rotulos, &rx, &mut estado)
            }) {
                tracing::warn!(?e, "no se pudo abrir la ventana de grabar clase");
            }
            estado.cerrar(&pedido);
            *ZONA.lock().unwrap_or_else(|e| e.into_inner()) = None;
            poner_grabando(false);
            TERMINAR_YA.store(false, Ordering::SeqCst);
            ABIERTA.store(false, Ordering::SeqCst);
        });
    if let Err(e) = hecho {
        *ZONA.lock().unwrap_or_else(|e| e.into_inner()) = None;
        ABIERTA.store(false, Ordering::SeqCst);
        tracing::warn!(?e, "no se pudo lanzar el hilo de grabar clase");
    }
}

// --- Lo puro ---------------------------------------------------------------

/// Una captura hecha grabando: en que milisegundo de la nota y sus bytes,
/// tal cual se guardan (el PNG de la pantalla o el JPEG de la camara, a su
/// resolucion entera).
#[derive(Debug, Clone)]
pub(super) struct Captura {
    pub ms: i64,
    pub bytes: Vec<u8>,
    /// Si es una foto de la camara (JPEG) y no de la pantalla (PNG).
    pub de_camara: bool,
    /// Ya a salvo en el disco, en el diario de la grabacion ([`a_salvo`]):
    /// entonces `bytes` va vacio y se lee de aqui al guardar.
    pub fichero: Option<PathBuf>,
}

impl Captura {
    fn de_pantalla(ms: i64, png: Vec<u8>) -> Captura {
        Captura {
            ms,
            bytes: png,
            de_camara: false,
            fichero: None,
        }
    }

    /// Sus bytes: los de memoria o, si ya estaba a salvo, los del disco.
    fn leer(&self) -> std::io::Result<std::borrow::Cow<'_, [u8]>> {
        match &self.fichero {
            Some(f) if self.bytes.is_empty() => Ok(std::fs::read(f)?.into()),
            _ => Ok(self.bytes.as_slice().into()),
        }
    }

    fn extension(&self) -> &'static str {
        if self.de_camara { "jpg" } else { "png" }
    }
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
    let tiempos: Vec<i64> = capturas.iter().map(|(ms, _)| *ms).collect();
    pixpin_proyecto::clase::ultimo_antes(&tiempos, posicion_ms)
}

// --- El estado ------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    /// «Empezar grabando la pantalla» de Preparar.
    PantallaAlEmpezar,
    /// La fila de la camara: un clic pasa a la siguiente.
    Camara,
    /// «Foto» de la barra: la camara del equipo.
    Foto,
    /// «Clip» de la barra: empieza o para un clip con la camara.
    Clip,
    /// «Pantalla» de la barra: enciende o apaga el video de la pantalla.
    Pantalla,
    /// El reloj de la barra: un clic pausa o sigue; mantenerlo termina.
    Reloj,
    /// ⛶: la pantalla completa, o volver a la barra.
    Completa,
    /// A− y A+ de la pantalla completa.
    LetraMenos,
    LetraMas,
    Microfono,
    Equipo,
    /// El perfil del video («Ligero», «Normal», «Nitido»): un clic pasa al
    /// siguiente.
    Calidad,
    Proyecto,
    Empezar,
    Cerrar,
    Captura,
    Zona,
    Terminar,
    Tirar,
    /// Cerrar mientras se pasa a texto: el texto sigue de fondo.
    DeFondo,
    Listo,
    /// «Abrir para estudiar» del final: la clase en su ventana.
    Estudiar,
    /// «📱 Cámara del móvil» de la barra: la enciende o la apaga
    /// (`sincronizar::camara`).
    CamaraMovil,
    /// La fila «Temas de la reunion» de Preparar: abre su pantalla.
    Temas,
    /// En esa pantalla: pegar, tomar de una lista, vaciar, quitar uno y
    /// volver.
    TemasPegar,
    TemasDeLista,
    TemasVaciar,
    QuitarTema(usize),
    TemasListo,
    /// «Temas 2/5» de la barra: abre o pliega la lista.
    PanelTemas,
    /// Un tema de esa lista: marcarlo o desmarcarlo a mano.
    Tema(usize),
    /// El ✓ (o «Deshacer») y el ✕ de una ficha de la reunion ([`reunion`]),
    /// por su id.
    FichaSi(u32),
    FichaNo(u32),
    /// «Duración prevista» de Preparar: la siguiente.
    Duracion,
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
    /// Empezar grabando la pantalla («Pantalla» encendida desde el
    /// principio: el video de la nota).
    pantalla_al_empezar: bool,
    /// Los sonidos con que se empezo a recordar (en persona, sin el del
    /// equipo): la palabra del diario.
    modo_recordado: Modo,
    /// Con que perfil (ver `pixpin_motor_video::Perfil`).
    perfil_video: Perfil,
    /// Ya se dijo con que codificador va el video (se dice una vez).
    codificador_avisado: bool,
    /// El reparto del sonido a los trozos del video ([`pantalla`]), desde
    /// que se abre el audio.
    reparto: Option<pantalla::Reparto>,
    /// La pantalla que se graba: la de la barra al encender «Pantalla».
    monitor_video: u32,
    /// Los trozos del video de la pantalla; el ultimo, si va, el de ahora.
    trozos: Vec<pantalla::Trozo>,
    /// La camara ya se pidio abrir (se abre a la primera foto o clip).
    camara_abierta: bool,
    /// **La pantalla completa** ([`completa`]), lo grande de su letra y lo
    /// que se va oyendo (`(ms, texto)`, las ultimas).
    completa: bool,
    letra: f32,
    frases: Vec<(i64, String)>,
    /// Desde cuando se mantiene el reloj: se va llenando de rojo hasta
    /// terminar (que se vea que va a terminar, y como).
    manteniendo: Option<Instant>,
    /// Ya se dijo que el video se corto (se dice una vez).
    video_avisado: bool,
    /// Si el video entro en el chat, para el «Guardada» del final.
    video_guardado: bool,
    /// El hilo de las fotos.
    fotografo: fotografo::Fotografo,
    /// El hilo de la camara (reunion presencial).
    camarografo: camara::Camarografo,
    /// Las camaras del equipo, una vez buscadas, y la elegida.
    camaras: Vec<pixpin_capture::camara::InfoCamara>,
    camara: usize,
    /// Buscando las camaras (la lista llega por aqui).
    buscando_camaras: Option<Receiver<Vec<pixpin_capture::camara::InfoCamara>>>,
    /// Los clips de la camara ya cerrados.
    clips: Vec<camara::ClipHecho>,
    /// Cuantos clips se han empezado (para su nombre temporal).
    clips_empezados: usize,
    /// Los clips guardados, para el «Guardada» del final.
    clips_guardados: usize,
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
    /// El ultimo destello: la foto salio (`true`, de la camara o del movil:
    /// brilla «Foto») o la captura (`false`: brilla «Captura»). Es lo que
    /// dice, sin mirar la linea de abajo, que se hizo (punto 52 del movil).
    destello: Option<(Instant, bool)>,
    /// Las capturas guardadas, para el «con N capturas» del final.
    guardadas: usize,
    /// El nombre de la nota (`Clase {fecha}`), con [`HUECO`] en la fecha.
    plantilla: String,
    /// La nota guardada, `(proyecto, id)`, para «Abrir para estudiar».
    para_estudiar: Option<(String, String)>,
    /// El diario de la grabacion en el disco ([`a_salvo`]).
    a_salvo: a_salvo::ASalvo,
    /// El disco esta casi lleno: el video no empieza (o se cerro).
    sin_video_por_disco: bool,
    /// El mensaje de la carpeta de la clase, una vez guardada.
    carpeta_id: Option<String>,
    /// **Los temas de la reunion** ([`temas`]), en el orden de la agenda.
    temas: Vec<pixpin_proyecto::temas::Tema>,
    /// La lista de tareas de la que salieron, `(proyecto, id)`.
    temas_lista: Option<(String, String)>,
    /// Salieron de lo que se paso a la proxima reunion (se olvida al
    /// empezar).
    temas_de_la_proxima: bool,
    /// Preparar ensena la pantalla de los temas, y lo que se va escribiendo.
    editando_temas: bool,
    escrito: String,
    /// Las listas de tareas de «De una lista», una vez leidas, y la
    /// siguiente.
    listas_de_temas: Option<Vec<crate::jev::temas::ListaDeTemas>>,
    lista_de_temas: usize,
    /// La lista de los temas abierta bajo la barra.
    panel_temas: bool,
    /// La escucha en vivo ([`escucha`]) y lo que dice.
    escucha: Option<escucha::Escucha>,
    estado_escucha: Option<String>,
    /// Lo recien marcado por Jev (para su brillo) y el destello del boton.
    recien_tratados: Vec<(String, Instant)>,
    destello_temas: Option<Instant>,
    /// «Temas · {nombre}» (con [`HUECO`]), el titulo de su lista de tareas.
    titulo_temas: String,
    /// Lo demas de la reunion: las fichas de la barra y lo que va a la nota
    /// ([`reunion`]).
    reunion: reunion::EnVivo,
    /// Lo que se piensa que dura la reunion, en minutos (para el tema
    /// olvidado).
    duracion_prevista: Option<u32>,
}

impl Estado {
    fn nuevo(pedido: &Pedido, plantilla: String) -> Estado {
        Estado::vacio(pedido, plantilla)
            .con_temporal(pedido)
            .con_sonidos_del_modo(&pedido.ubicacion)
    }

    /// Sin temporal ni camaras buscadas: lo que necesita la recuperacion
    /// para guardar una grabacion cortada ([`recuperar`]).
    fn vacio(pedido: &Pedido, plantilla: String) -> Estado {
        Estado {
            fase: Fase::Preparar,
            microfono: true,
            equipo: true,
            pantalla_al_empezar: false,
            modo_recordado: modo_recordado(&pedido.ubicacion),
            perfil_video: Perfil::Normal,
            codificador_avisado: false,
            reparto: None,
            monitor_video: 0,
            trozos: Vec::new(),
            camara_abierta: false,
            completa: false,
            letra: 1.0,
            frases: Vec::new(),
            manteniendo: None,
            video_avisado: false,
            video_guardado: false,
            fotografo: fotografo::Fotografo::default(),
            camarografo: camara::Camarografo::default(),
            camaras: Vec::new(),
            camara: 0,
            buscando_camaras: None,
            clips: Vec::new(),
            clips_empezados: 0,
            clips_guardados: 0,
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
            para_estudiar: None,
            a_salvo: a_salvo::ASalvo::default(),
            sin_video_por_disco: false,
            carpeta_id: None,
            temas: Vec::new(),
            temas_lista: None,
            temas_de_la_proxima: false,
            editando_temas: false,
            escrito: String::new(),
            listas_de_temas: None,
            lista_de_temas: 0,
            panel_temas: false,
            escucha: None,
            estado_escucha: None,
            recien_tratados: Vec::new(),
            destello_temas: None,
            titulo_temas: String::new(),
            reunion: reunion::EnVivo::default(),
            duracion_prevista: None,
        }
    }

    /// Lo recordado: los sonidos del modo de antes (en persona, solo el
    /// microfono), si se empieza grabando la pantalla y lo grande de la letra
    /// de la pantalla completa. Las camaras se buscan ya (Foto y Clip estan
    /// siempre en la barra).
    fn con_sonidos_del_modo(mut self, u: &Ubicacion) -> Estado {
        (self.microfono, self.equipo) = self.modo_recordado.sonidos();
        self.pantalla_al_empezar = self.modo_recordado.con_video();
        self.letra = letra_recordada(u);
        self.buscar_camaras();
        self
    }

    /// La palabra del diario: como empezo la grabacion.
    pub(super) fn modo_del_diario(&self) -> Modo {
        if self.pantalla_al_empezar {
            Modo::Pantalla
        } else if !self.equipo {
            Modo::Presencial
        } else {
            Modo::Capturas
        }
    }

    /// **Abre la camara** si aun no se pidio (la primera foto o clip): en
    /// una clase por pantalla no se enciende. Devuelve si hay camara.
    fn abrir_la_camara(&mut self, r: &Rotulos) -> bool {
        if self.camara_abierta {
            return true;
        }
        let Some(c) = self.camaras.get(self.camara) else {
            if self.buscando_camaras.is_some() {
                self.avisar(r.buscando_camaras.clone());
            } else {
                self.avisar(r.camara_sin.clone());
            }
            return false;
        };
        let id = c.id.clone();
        if self.camarografo.abrir(&id) {
            self.camara_abierta = true;
            self.avisar(r.abriendo_camara.clone());
            true
        } else {
            self.avisar(r.camara_fallo.clone());
            false
        }
    }

    fn buscar_camaras(&mut self) {
        if self.camaras.is_empty() && self.buscando_camaras.is_none() {
            self.buscando_camaras = Some(camara::buscar_camaras());
        }
    }

    /// Las camaras que llegaron de su hilo: se empieza con la recordada o
    /// con la que mejor mira a la sala.
    fn camaras_encontradas(&mut self, u: &Ubicacion) {
        let Some(rx) = &self.buscando_camaras else {
            return;
        };
        match rx.try_recv() {
            Ok(v) => {
                let recordada = std::fs::read_to_string(fichero_de_la_camara(u)).ok();
                let lados: Vec<(String, presencial::Lado)> = v
                    .iter()
                    .map(|c| {
                        let lado = match c.lado {
                            pixpin_capture::camara::LadoCamara::Delante => {
                                presencial::Lado::Delante
                            }
                            pixpin_capture::camara::LadoCamara::Detras => presencial::Lado::Detras,
                            pixpin_capture::camara::LadoCamara::Fuera => presencial::Lado::Fuera,
                        };
                        (c.id.clone(), lado)
                    })
                    .collect();
                self.camara = presencial::camara_elegida(&lados, recordada.as_deref()).unwrap_or(0);
                self.camaras = v;
                self.buscando_camaras = None;
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => self.buscando_camaras = None,
        }
    }

    /// Lo que vuelve del hilo de la camara.
    fn de_la_camara(&mut self, r: &Rotulos) {
        for v in self.camarografo.recoger() {
            match v {
                camara::Vuelta::Abierta(Ok((w, h))) => {
                    if w > 0 {
                        self.avisar(con(&r.camara_lista, &format!("{w}×{h}")));
                    }
                }
                camara::Vuelta::Abierta(Err(f)) => {
                    // La siguiente foto o clip lo vuelve a probar (la cerro
                    // otra aplicacion, se dio el permiso).
                    self.camara_abierta = false;
                    self.avisar(r.de_la_camara(f));
                }
                camara::Vuelta::ClipEmpezado(Err(f)) => self.avisar(r.de_la_camara(f)),
                camara::Vuelta::Foto(ms, Ok(jpg)) => {
                    self.anadir_captura(Captura {
                        ms,
                        bytes: jpg,
                        de_camara: true,
                        fichero: None,
                    });
                    self.destello = Some((Instant::now(), true));
                    self.avisar(con(&r.foto_hecha, &minuto(ms)));
                }
                camara::Vuelta::Foto(_, Err(f)) => self.avisar(r.de_la_camara(f)),
                camara::Vuelta::ClipEmpezado(Ok(_)) => self.avisar(r.clip_empezado.clone()),
                camara::Vuelta::ClipParado(Some(c)) => {
                    self.avisar(con(&r.clip_guardado, &minuto(c.ms)));
                    self.apuntar_clip(&c);
                    self.clips.push(c);
                }
                camara::Vuelta::ClipParado(None) => self.avisar(r.clip_corto.clone()),
            }
        }
    }

    /// La ruta del siguiente clip: al lado del temporal de la nota (en la
    /// carpeta del proyecto: guardarlo luego es renombrarlo).
    fn ruta_del_clip(&mut self) -> PathBuf {
        self.clips_empezados += 1;
        let tronco = self
            .temporal
            .file_stem()
            .map(|t| t.to_string_lossy().into_owned())
            .unwrap_or_else(|| "clase".into());
        self.temporal
            .with_file_name(format!("{tronco}-clip-{}.mp4", self.clips_empezados))
    }

    /// Recoge lo ultimo de la camara al terminar: las fotos que faltaban y
    /// el clip que se estuviera grabando.
    fn acabar_la_camara(&mut self) {
        for v in self.camarografo.acabar(Duration::from_secs(10)) {
            match v {
                camara::Vuelta::Foto(ms, Ok(jpg)) => self.anadir_captura(Captura {
                    ms,
                    bytes: jpg,
                    de_camara: true,
                    fichero: None,
                }),
                camara::Vuelta::ClipParado(Some(c)) => {
                    self.apuntar_clip(&c);
                    self.clips.push(c);
                }
                camara::Vuelta::Foto(ms, Err(f)) => {
                    tracing::warn!(?f, ms, "una foto de la reunion no se hizo");
                }
                _ => {}
            }
        }
    }

    /// Tira los clips (la grabacion se tira).
    fn tirar_clips(&mut self) {
        self.acabar_la_camara();
        for c in self.clips.drain(..) {
            let _ = std::fs::remove_file(&c.ruta);
        }
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
        self.acabar_escucha(None);
        let parado = match std::mem::replace(&mut self.fase, Fase::Preparar) {
            Fase::Grabando(g) => Some(g.parar()),
            Fase::Abriendo(rx) => {
                if let Ok(Ok(g)) = rx.recv_timeout(Duration::from_secs(5)) {
                    g.cancelar();
                }
                // No llego a grabarse nada.
                self.tirar_diario();
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
        if let Some(parado) = parado {
            let g = self.guardar_todo(pedido, parado);
            if let Some(e) = &g.fallo_audio {
                tracing::warn!(?e, "la nota de la clase no se cerro al cerrar la ventana");
            }
            if let Some(e) = &g.fallo_guardar {
                tracing::warn!(?e, "la clase no se guardo al cerrar la ventana");
            }
            self.cerrar_diario(&g);
        }
        // Lo que no llego a grabarse ya no tiene a quien ir.
        self.tirar_trozos();
        self.reparto = None;
        // El temporal, solo si no queda un diario que lo necesite para
        // recuperar la clase.
        if !self.temporal.as_os_str().is_empty() && !self.con_diario() {
            let _ = std::fs::remove_file(&self.temporal);
        }
    }

    /// **Guarda todo lo que haya** al parar: las fotos que aun estaban
    /// haciendose, el video y la nota. Si la nota fallo (el microfono se
    /// fue, el disco), las capturas y el video se guardan igual: una
    /// reunion sin su nota sigue valiendo por sus imagenes.
    fn guardar_todo(&mut self, pedido: &Pedido, parado: Parado) -> Guardado {
        self.acabar_escucha(None);
        self.acabar_la_camara();
        for (ms, hecha) in self.fotografo.acabar(Duration::from_secs(5)) {
            match hecha {
                Ok(png) => self.anadir_captura(Captura::de_pantalla(ms, png)),
                Err(e) => tracing::warn!(?e, ms, "una captura de la clase no se hizo"),
            }
        }
        // El video de la nota y los trozos de la pantalla (a los clips).
        let fin_ms = parado.as_ref().map_or(0, |g| g.duracion_ms);
        let video = self.recoger_trozos(fin_ms);
        self.reparto = None;
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
        if corta {
            for c in self.clips.drain(..) {
                let _ = std::fs::remove_file(&c.ruta);
            }
        }
        if grabacion.is_none()
            && self.capturas.is_empty()
            && video.is_none()
            && self.clips.is_empty()
        {
            let _ = std::fs::remove_file(&self.temporal);
            return Guardado {
                voz: None,
                fallo_audio,
                fallo_guardar: None,
            };
        }
        let hecho = guardar(pedido, self, grabacion.as_ref(), video.as_deref());
        // La nota ya esta copiada en el chat: el temporal sobra. Si no entro,
        // se queda para recuperarla (lo dice el diario).
        if hecho.as_ref().is_ok_and(|v| v.is_some()) {
            let _ = std::fs::remove_file(&self.temporal);
        }
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
                    let raiz = pedido.ubicacion.raiz();
                    match salida {
                        Ok(t) => match escribir_el_texto(&pedido, &proyecto, &id, &t) {
                            Ok(()) => cola_de_texto::quitar(raiz, &id),
                            Err(e) => tracing::warn!(?e, "el texto de la clase no se escribio"),
                        },
                        Err(e) => {
                            tracing::info!(?e, "la clase quedo sin texto");
                            cola_de_texto::fallo(raiz, &id);
                        }
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
    // `de_camara`: la foto o el clip salio de la camara (`origen`), y se
    // apunta aunque no haya nota con que enlazarlo.
    let enlazar = |m: &mut cuaderno::Mensaje, ms: i64, de_camara: bool| {
        if de_camara {
            presencial::marcar_de_camara(&mut m.resto);
        }
        if let Some(de) = &de {
            m.resto.insert(
                "enGrabacion".into(),
                serde_json::json!({ "de": de, "ms": ms }),
            );
        }
        if (de.is_some() || de_camara)
            && let Err(e) = cuaderno::reemplazar(&carpeta, m)
        {
            tracing::warn!(?e, "un adjunto de la clase quedo sin enlazar");
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
                enlazar(&mut m, 0, false);
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
        // Los bytes tal cual: el PNG de la pantalla o el JPEG de la camara,
        // a su resolucion entera (solo las miniaturas se reducen).
        let fichero = presencial::nombre_de_la_foto(
            c.de_camara,
            pixpin_shell::entorno::ahora_utc_ms(),
            i + 1,
            c.extension(),
        );
        let pie = format!("⏱ {} · {}", minuto(c.ms), nombre);
        let hecho = c.leer().and_then(|bytes| {
            super::adjuntar_con_pie(
                raiz,
                proyecto,
                &pedido.aparato,
                &fichero,
                &bytes,
                numero,
                &pie,
                None,
            )
        });
        match hecho {
            Ok(mut m) => {
                numero += 1;
                enlazar(&mut m, c.ms, c.de_camara);
                enlazadas.push((c.ms, m.id));
            }
            Err(e) => tracing::warn!(?e, ms = c.ms, "una captura de la clase no se guardo"),
        }
    }
    estado.guardadas = enlazadas.len();

    // Los clips de la camara y los trozos de la pantalla, como el video:
    // movidos (no leidos enteros) y enlazados en su minuto, con lo que
    // duran. Los de la camara llevan `fuente`; los de la pantalla no (como
    // las capturas), y cada uno cuenta aparte en su nombre.
    let mut clips = std::mem::take(&mut estado.clips);
    clips.sort_by_key(|c| c.ms);
    let mut clips_dentro: Vec<presencial::Clip> = Vec::new();
    let (mut de_camara, mut de_pantalla) = (0, 0);
    for c in clips {
        let fichero = if c.de_pantalla {
            de_pantalla += 1;
            format!("{nombre} · pantalla {de_pantalla}.mp4")
        } else {
            de_camara += 1;
            format!("{nombre} · clip {de_camara}.mp4")
        };
        let hecho = adjuntar_video(raiz, proyecto, &pedido.aparato, &fichero, &c.ruta, numero);
        match hecho {
            Ok(mut m) => {
                numero += 1;
                if !c.de_pantalla {
                    presencial::marcar_de_camara(&mut m.resto);
                }
                if let Some(de) = &de {
                    m.resto.insert(
                        "enGrabacion".into(),
                        presencial::enlace_del_clip(de, c.ms, c.duracion_ms),
                    );
                }
                if let Err(e) = cuaderno::reemplazar(&carpeta, &m) {
                    tracing::warn!(?e, "un clip de la clase quedo sin enlazar");
                }
                clips_dentro.push(presencial::Clip {
                    ms: c.ms,
                    duracion_ms: c.duracion_ms,
                    id: m.id,
                });
            }
            // Se queda en la carpeta del proyecto, con su nombre temporal.
            Err(e) => tracing::warn!(
                ?e,
                ruta = %c.ruta.display(),
                "un clip de la clase no entro en el chat (se queda en la carpeta)"
            ),
        }
    }
    estado.clips_guardados = clips_dentro.len();
    if let Some(voz) = voz.as_mut() {
        // Siempre, aunque no haya capturas: es lo que dice que la nota es
        // de una clase y no una nota de voz cualquiera.
        voz.resto
            .insert("grabacion".into(), serde_json::json!("clase"));
        voz.resto
            .insert("capturas".into(), capturas_en_json(&enlazadas));
        // Los temas de la reunion, con lo tratado (su lista, al final).
        pixpin_proyecto::temas::poner_temas(&mut voz.resto, &estado.temas);
        // Y lo demas que saco Jev mientras se grababa (sin nada, nada).
        pixpin_proyecto::acta_viva::poner(&mut voz.resto, &estado.reunion.acta());
        if let Some(id) = &video_id {
            voz.resto
                .insert("video".into(), serde_json::json!({ "id": id }));
        }
        presencial::poner_clips(&mut voz.resto, &clips_dentro);
        if let Err(e) = cuaderno::reemplazar(&carpeta, voz) {
            // La nota y sus adjuntos ya estan dentro; solo falta el enlace.
            tracing::warn!(?e, "la nota de la clase quedo sin sus enlaces");
        }
    }
    // Y todo dentro de su carpeta: en el chat se ve UNA tarjeta y no la nota,
    // el video y cada captura sueltos.
    let grabacion_en_carpeta = carpeta_de_la_grabacion(
        voz.as_ref().map(|v| v.id.as_str()),
        grabacion.map_or(0, |g| g.duracion_ms),
        video_id.as_deref(),
        &enlazadas,
        &clips_dentro,
    );
    let mut carpeta_id = None;
    if !grabacion_en_carpeta.is_empty() {
        let cuando = super::descripcion::hora_sin_repetir();
        let m = pixpin_proyecto::carpeta_de_paginas::mensaje_de_grabacion(
            &nombre,
            &grabacion_en_carpeta,
            &cuaderno::Sello {
                cuando,
                numero,
                aparato: pedido.aparato.clone(),
                proyecto: proyecto.clone(),
            },
        );
        match cuaderno::anadir(&carpeta, &m) {
            Ok(()) => {
                carpeta_id = Some(m.id.clone());
                estado.carpeta_id = Some(m.id.clone());
                let mut indice = almacen::Indice::leer(raiz);
                if let Some(f) = indice.proyectos.iter_mut().find(|f| &f.id == proyecto) {
                    f.tocado = cuando;
                    f.resumen = m.nombre.clone();
                    if let Err(e) = indice.guardar(raiz) {
                        tracing::warn!(?e, "el indice no se entero de la clase");
                    }
                }
            }
            // Lo de dentro ya esta en el chat, suelto: como antes.
            Err(e) => tracing::warn!(?e, "la clase quedo sin su carpeta"),
        }
    }
    // Los temas, en su lista de tareas (detras de la carpeta: los numeros
    // van seguidos), enlazada en la nota.
    if !estado.temas.is_empty() {
        guardar_los_temas(pedido, proyecto, &nombre, estado, voz.as_mut(), &carpeta);
    }
    // Lo que llego del movil grabando entra ahora, en su minuto.
    crate::sincronizar::camara::guardada(
        raiz,
        proyecto,
        voz.as_ref().map(|v| v.id.as_str()),
        carpeta_id.as_deref(),
        grabacion.map_or(0, |g| g.duracion_ms),
    );
    super::refrescar();
    tracing::info!(
        con_voz = voz.is_some(),
        con_video = video_id.is_some(),
        capturas = enlazadas.len(),
        clips = clips_dentro.len(),
        duracion_ms = grabacion.map(|g| g.duracion_ms),
        "clase guardada"
    );
    Ok(voz)
}

/// **Los temas de la reunion al guardar**: lo marcado mientras tanto en su
/// lista de tareas (en Tareas o en el movil) entra como hecho a mano; la
/// lista (la elegida al preparar o una nueva) queda con los temas nuevos y
/// lo tratado marcado, y la nota los lleva en `resto["temas"]` con
/// `resto["temasLista"]`. Un fallo con la lista no tira la clase.
fn guardar_los_temas(
    pedido: &Pedido,
    proyecto: &str,
    nombre: &str,
    estado: &mut Estado,
    voz: Option<&mut cuaderno::Mensaje>,
    carpeta: &Path,
) {
    use pixpin_proyecto::temas;
    let raiz = pedido.ubicacion.raiz();
    if let Some((p, c)) = &estado.temas_lista
        && let Ok(m) = crate::pedidos::mensaje_de(raiz, p, c)
    {
        temas::con_la_lista(&mut estado.temas, &m.texto);
    }
    let titulo = if estado.titulo_temas.is_empty() {
        nombre.to_string()
    } else {
        con(&estado.titulo_temas, nombre)
    };
    match crate::jev::temas::guardar_lista(
        raiz,
        proyecto,
        &pedido.aparato,
        &titulo,
        &estado.temas,
        estado.temas_lista.as_ref(),
    ) {
        Ok(l) => estado.temas_lista = Some(l),
        Err(e) => tracing::warn!(
            ?e,
            "los temas de la reunion quedaron sin su lista de tareas"
        ),
    }
    let Some(voz) = voz else {
        return;
    };
    temas::poner_temas(&mut voz.resto, &estado.temas);
    if let Some((p, c)) = &estado.temas_lista {
        temas::poner_lista(&mut voz.resto, p, c);
    }
    let resto = voz.resto.clone();
    let hecho = cuaderno::cambiar(carpeta, &voz.id, |m| {
        for k in [temas::TEMAS, temas::TEMAS_LISTA] {
            match resto.get(k) {
                Some(v) => m.resto.insert(k.into(), v.clone()),
                None => m.resto.remove(k),
            };
        }
        true
    });
    if let Err(e) = hecho {
        tracing::warn!(?e, "la nota de la clase quedo sin sus temas");
    }
}

/// **La carpeta de una clase**: la nota, el video, las capturas y los clips
/// de la camara en el orden de la grabacion, cada uno con su minuto (las
/// capturas con el suyo de titulo, «⏱ 01:05», y los clips «▶ 01:05»; la
/// nota y el video, con su nombre).
fn carpeta_de_la_grabacion(
    voz: Option<&str>,
    duracion_ms: i64,
    video: Option<&str>,
    capturas: &[(i64, String)],
    clips: &[presencial::Clip],
) -> pixpin_proyecto::carpeta_de_paginas::Carpeta {
    let mut c = pixpin_proyecto::carpeta_de_paginas::Carpeta::grabacion(
        voz.unwrap_or_default(),
        duracion_ms,
    );
    c.video = video.unwrap_or_default().to_string();
    for id in voz.into_iter().chain(video) {
        c.anadir_al_minuto(id, "", 0);
    }
    let mut en_orden: Vec<(i64, &str, String)> = capturas
        .iter()
        .map(|(ms, id)| (*ms, id.as_str(), format!("⏱ {}", minuto(*ms))))
        .chain(
            clips
                .iter()
                .map(|k| (k.ms, k.id.as_str(), format!("▶ {}", minuto(k.ms)))),
        )
        .collect();
    en_orden.sort_by_key(|(ms, _, _)| *ms);
    for (ms, id, titulo) in &en_orden {
        c.anadir_al_minuto(id, titulo, *ms);
    }
    c
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
    // Lo importante de la clase (Jev, en su hilo; sin clave, nada).
    crate::jev::reunion::lanzar(&pedido.ubicacion, proyecto, id, true);
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

/// Lo ancho de la barra, en logicos: 16 de margen, el reloj (96) y el
/// nivel (34), Foto (84), Captura (100), Pantalla (104) y Clip (76), ⛶ y ✕
/// (36 cada uno), los huecos de 8 y el margen.
const ANCHO_BARRA: u32 =
    16 + 96 + 8 + 34 + 8 + 84 + 8 + 100 + 8 + 104 + 8 + 76 + 8 + 36 + 8 + 36 + 16;

fn tamano(fase: &Fase) -> (u32, u32) {
    match fase {
        Fase::Preparar => (340, 464),
        // Lo que piden los botones de la barra (ver `pintar_barra`): el
        // reloj y el nivel, las cuatro acciones, ⛶ y ✕, y el margen.
        Fase::Abriendo(_) | Fase::Grabando(_) => (ANCHO_BARRA, 92),
        _ => (360, 170),
    }
}

/// [`tamano`] con los temas: la barra con temas lleva su boton y, abierta,
/// su lista debajo.
fn tamano_de(estado: &Estado) -> (u32, u32) {
    let (w, h) = tamano(&estado.fase);
    match &estado.fase {
        // La pantalla completa: mas que cualquier pantalla, que `marco_para`
        // recorta a su area de trabajo.
        Fase::Abriendo(_) | Fase::Grabando(_) if estado.completa => {
            (completa::TODA, completa::TODA)
        }
        Fase::Abriendo(_) | Fase::Grabando(_) if !estado.temas.is_empty() => (
            w + temas::ANCHO_BOTON,
            h + temas::alto_del_panel(estado) + reunion::alto(estado),
        ),
        Fase::Abriendo(_) | Fase::Grabando(_) => (w, h + reunion::alto(estado)),
        _ => (w, h),
    }
}

/// Preparar, en el centro del monitor; la barra, arriba en el centro (a la
/// vista y lejos de lo que se suele mirar); lo demas, donde quedo la barra.
/// `(lw, lh)`: su tamano en logicos ([`tamano_de`]).
fn marco_para(
    area: Rect,
    escala: u32,
    fase: &Fase,
    (lw, lh): (u32, u32),
    antes: Option<Rect>,
) -> Rect {
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
fn reencajar(area: Rect, escala: u32, (lw, lh): (u32, u32), antes: Rect) -> Rect {
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
    let mut logico = tamano_de(estado);
    let mut marco = marco_para(area, monitor.escala_por_cien, &estado.fase, logico, None);
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
    // Donde se pulso, el marco de entonces, sobre que, si ya es un
    // arrastre y cuando (mantener el reloj termina; «Captura», una zona).
    let mut pulsado: Option<(Punto, Rect, Option<Accion>, bool, Instant)> = None;
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
                    if let Some((desde, origen, _, arrastre, _)) = pulsado.as_mut() {
                        let (dx, dy) = (p.x - desde.x, p.y - desde.y);
                        // La pantalla completa no se arrastra: ocupa la pantalla.
                        if !*arrastre
                            && !estado.completa
                            && (dx.abs() > UMBRAL_ARRASTRE || dy.abs() > UMBRAL_ARRASTRE)
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
                    pulsado = Some((p, marco, botones.bajo_el_raton(), false, Instant::now()));
                }
                // El derecho sobre «Captura»: una zona (como mantenerla).
                EventoOverlay::BotonDerechoPulsado(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    if botones.bajo_el_raton() == Some(Accion::Captura) {
                        accion = Some(Accion::Zona);
                    }
                }
                EventoOverlay::BotonSoltado(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    match pulsado.take() {
                        Some((_, _, Some(a), false, _)) if botones.bajo_el_raton() == Some(a) => {
                            accion = Some(a);
                        }
                        Some((_, _, _, true, _)) => soltado_tras_arrastrar = true,
                        _ => {}
                    }
                }
                // La pantalla de los temas se lleva el teclado.
                EventoOverlay::Caracter(c)
                    if estado.editando_temas && matches!(estado.fase, Fase::Preparar) =>
                {
                    estado.escribir_tema(c);
                }
                EventoOverlay::Tecla { vk, ctrl, .. }
                    if estado.editando_temas && matches!(estado.fase, Fase::Preparar) =>
                {
                    if let Some(a) = estado.tecla_de_temas(vk, ctrl, r) {
                        accion = Some(a);
                    }
                }
                EventoOverlay::Tecla { vk, .. } => match (vk, &estado.fase) {
                    (VK_ESCAPE, Fase::Preparar) => accion = Some(Accion::Cerrar),
                    (VK_ENTRAR, Fase::Preparar) => accion = Some(Accion::Empezar),
                    // Escape sale de la pantalla completa (si tiene el teclado).
                    (VK_ESCAPE, Fase::Grabando(_)) if estado.completa => {
                        accion = Some(Accion::Completa)
                    }
                    (VK_ENTRAR, Fase::Lista { .. }) if estado.para_estudiar.is_some() => {
                        accion = Some(Accion::Estudiar)
                    }
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
        // **Mantener** el reloj termina y «Captura» pide una zona, sin soltar
        // (se ve en el acto); al soltar ya no hace nada mas.
        if let Some((desde, origen, Some(a @ (Accion::Reloj | Accion::Captura)), false, cuando)) =
            pulsado
            && cuando.elapsed() >= MANTENER
            && botones.bajo_el_raton() == Some(a)
        {
            accion = Some(if a == Accion::Reloj {
                Accion::Terminar
            } else {
                Accion::Zona
            });
            pulsado = Some((desde, origen, None, false, cuando));
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
            marco = reencajar(area, monitor.escala_por_cien, tamano_de(estado), marco);
            ventana.mover(marco);
            if let Err(e) = superficie.redimensionar(marco.ancho, marco.alto) {
                tracing::warn!(?e, "no se pudo redimensionar grabar clase");
            }
        }

        // «Salir» de la bandeja: terminar y guardar ya, sin pasar a texto.
        if TERMINAR_YA.load(Ordering::SeqCst) {
            match &estado.fase {
                Fase::Grabando(_) => accion = Some(Accion::Terminar),
                // Lo demas lo cierra y lo guarda `Estado::cerrar`.
                Fase::Parando { .. } => {}
                _ => break,
            }
        }

        // Las camaras encontradas y lo que vuelve de la camara.
        estado.camaras_encontradas(&pedido.ubicacion);
        estado.de_la_camara(r);

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
                    estado.anadir_captura(Captura::de_pantalla(ms, png));
                    estado.destello = Some((Instant::now(), false));
                    estado.avisar(con(&r.captura_hecha, &minuto(ms)));
                }
                Err(e) => {
                    tracing::warn!(?e, "no se pudo capturar la zona de la clase");
                    estado.avisar(r.sin_zona.clone());
                }
            }
        }

        // El diario de la grabacion: el latido y el disco ([`a_salvo`]).
        estado.latir(pedido, r);
        // Lo que Jev marco de los temas.
        estado.de_la_escucha(pedido, r);

        // El movil como camara (`sincronizar::camara`): sabe en que minuto va
        // la grabacion y avisa de lo que llega.
        let grabando = match &estado.fase {
            Fase::Grabando(g) => Some((g.llevado_ms(), g.en_pausa())),
            _ => None,
        };
        let titulo = con(
            &estado.plantilla,
            &fecha_del_nombre(pixpin_shell::entorno::ahora_local_ms()),
        );
        if let Some(aviso) =
            crate::sincronizar::camara::latido(crate::sincronizar::camara::Latido {
                ubicacion: &pedido.ubicacion,
                idioma: pedido.idioma,
                titulo: &titulo,
                grabando,
            })
        {
            estado.destello = Some((Instant::now(), true));
            estado.avisar(aviso);
        }

        // El video que se corto: se dice una vez; la nota sigue.
        if !estado.video_avisado && estado.video_en_marcha().is_some_and(|v| v.roto()) {
            estado.video_avisado = true;
            estado.avisar(r.video_roto.clone());
        }
        // Y con que se codifica, una vez, en cuanto el motor arranca: la
        // grafica, el procesador o el modo sencillo.
        if !estado.codificador_avisado
            && let Some(c) = estado.video_en_marcha().and_then(video::Video::codificador)
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
                    // Con temas, a escuchar (de la misma grabacion).
                    estado.empezar_escucha(&g, pedido, r);
                    // «Empezar grabando la pantalla»: el video de la nota
                    // ([`pantalla`]; sin sitio en el disco, ni empieza).
                    if estado.pantalla_al_empezar {
                        estado.encender_pantalla((0, g.muestreo(), false), true, r);
                    }
                    Fase::Grabando(g)
                }
                Ok(Err(e)) => {
                    tracing::warn!(?e, "no se pudo abrir el audio de la clase");
                    estado.reparto = None;
                    estado.tirar_diario();
                    Fase::Fallo {
                        texto: r.de_un_fallo(&e),
                        desde: Instant::now(),
                    }
                }
                Err(TryRecvError::Empty) => Fase::Abriendo(rx),
                Err(TryRecvError::Disconnected) => {
                    estado.reparto = None;
                    estado.tirar_diario();
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
                let video_cerrado = estado.trozos_acabados();
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
                    let raiz = pedido.ubicacion.raiz();
                    let con_texto = match salida {
                        Ok(t) => match escribir_el_texto(pedido, &proyecto, &id, &t) {
                            Ok(()) => {
                                cola_de_texto::quitar(raiz, &id);
                                true
                            }
                            Err(e) => {
                                tracing::warn!(?e, "el texto de la clase no se escribio");
                                false
                            }
                        },
                        Err(e) => {
                            tracing::info!(?e, "la clase quedo sin texto");
                            cola_de_texto::fallo(raiz, &id);
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
            pulsar(a, pedido, r, estado, (monitor.id, monitor.area), &mut vivo);
        }
        // La bandeja dice «Grabando…» mientras se graba (aunque PixPin no
        // este delante: la barra sigue en su hilo).
        poner_grabando(matches!(
            estado.fase,
            Fase::Abriendo(_) | Fase::Grabando(_) | Fase::Parando { .. }
        ));
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

        if tamano_de(estado) != logico {
            logico = tamano_de(estado);
            marco = marco_para(
                area,
                monitor.escala_por_cien,
                &estado.fase,
                logico,
                Some(marco),
            );
            ventana.mover(marco);
            if let Err(e) = superficie.redimensionar(marco.ancho, marco.alto) {
                tracing::warn!(?e, "no se pudo redimensionar grabar clase");
            }
            pulsado = None;
        }

        estado.manteniendo = match pulsado {
            Some((_, _, Some(Accion::Reloj), false, desde)) => Some(desde),
            _ => None,
        };
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
            // El cursor de la pantalla de los temas parpadea.
            Fase::Preparar if estado.editando_temas => 60,
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
    estado.cerrar_diario(&g);
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
    if let Some(voz) = &g.voz {
        estado.para_estudiar = Some((proyecto.clone(), voz.id.clone()));
        // A la cola del texto antes de empezar: si PixPin se cierra antes
        // de acabar (o se sale sin pasarla), se retoma al volver a abrirlo.
        cola_de_texto::apuntar(pedido.ubicacion.raiz(), &proyecto, &voz.id);
    }
    // Para salir de PixPin no se pasa a texto: se pide luego en la nota.
    if TERMINAR_YA.load(Ordering::SeqCst) {
        return lista(r, &nombre, estado, false);
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
    // Las fotos y los clips de la camara cuentan con las capturas.
    let n = (estado.guardadas + estado.clips_guardados).to_string();
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
    (monitor, pantalla): (u32, Rect),
    vivo: &mut bool,
) {
    if a != Accion::Tirar {
        estado.tirar_desde = None;
    }
    match (a, &estado.fase) {
        (Accion::Microfono, Fase::Preparar) => estado.microfono = !estado.microfono,
        (Accion::Equipo, Fase::Preparar) => estado.equipo = !estado.equipo,
        (Accion::PantallaAlEmpezar, Fase::Preparar) => {
            estado.pantalla_al_empezar = !estado.pantalla_al_empezar;
        }
        (Accion::Camara, Fase::Preparar) => {
            if estado.camaras.is_empty() {
                // Se enchufo una despues de mirar: se vuelve a mirar.
                estado.buscar_camaras();
            } else {
                estado.camara = (estado.camara + 1) % estado.camaras.len();
                recordar_camara(&pedido.ubicacion, &estado.camaras[estado.camara].id);
            }
        }
        (Accion::Calidad, Fase::Preparar) => estado.perfil_video = estado.perfil_video.siguiente(),
        // Los temas de la reunion ([`temas`]).
        (Accion::Temas, Fase::Preparar) => {
            estado.editando_temas = true;
            estado.aviso = None;
        }
        (Accion::TemasPegar, Fase::Preparar) => estado.pegar_temas(r),
        (Accion::TemasDeLista, Fase::Preparar) => estado.temas_de_una_lista(pedido, r),
        (Accion::TemasVaciar, Fase::Preparar) => estado.vaciar_temas(),
        (Accion::QuitarTema(i), Fase::Preparar) => {
            if let Some(id) = estado.temas.get(i).map(|t| t.id.clone()) {
                pixpin_proyecto::temas::quitar(&mut estado.temas, &id);
            }
        }
        (Accion::TemasListo, Fase::Preparar) => {
            // Lo que quedo escrito sin Intro tambien vale.
            let escrito = std::mem::take(&mut estado.escrito);
            pixpin_proyecto::temas::anadir(&mut estado.temas, &escrito);
            estado.editando_temas = false;
            estado.aviso = None;
        }
        (Accion::PanelTemas, Fase::Abriendo(_) | Fase::Grabando(_)) => {
            estado.panel_temas = !estado.panel_temas;
        }
        (Accion::Tema(i), Fase::Abriendo(_) | Fase::Grabando(_)) => estado.tocar_tema(i),
        // Lo demas de la reunion ([`reunion`]).
        (Accion::FichaSi(id), Fase::Grabando(_)) => estado.ficha_si(id, pedido, &r.reunion),
        (Accion::FichaNo(id), Fase::Grabando(_)) => estado.ficha_no(id),
        (Accion::Duracion, Fase::Preparar) => estado.otra_duracion(),
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
            // La grabadora le manda siempre una copia de lo que oye al
            // reparto: «Pantalla» se puede encender en cualquier momento
            // ([`pantalla`]), y la copia solo se pide al abrirla.
            let (reparto, copia) = pantalla::Reparto::nuevo();
            estado.reparto = Some(reparto);
            estado.monitor_video = monitor;
            let copia = Some(copia);
            // Como se empezo, para la proxima vez.
            recordar_modo(&pedido.ubicacion, estado.modo_del_diario());
            // A prueba de cortes: la nota lleva su respaldo WAV, y todo lo
            // que se haga grabando se apunta en el diario ([`a_salvo`]).
            estado.fase = Fase::Abriendo(crate::ventanita::abrir_reunion(
                estado.temporal.clone(),
                fuentes,
                copia,
            ));
            estado.abrir_diario(pedido);
            // Con temas, su lista abierta bajo la barra; los que esperaban
            // de la reunion anterior ya estan en esta.
            estado.panel_temas = !estado.temas.is_empty();
            if estado.temas_de_la_proxima {
                crate::jev::temas::olvidar_la_proxima(pedido.ubicacion.raiz());
            }
        }
        (Accion::Cerrar, Fase::Preparar) | (Accion::Listo, _) => *vivo = false,
        (Accion::Estudiar, Fase::Lista { .. }) => {
            if let Some((proyecto, id)) = estado.para_estudiar.clone() {
                super::estudiar_clase::abrir(
                    pedido.idioma,
                    pedido.ubicacion.clone(),
                    proyecto,
                    id,
                    0,
                );
            }
            *vivo = false;
        }
        // Cerrar mientras se pasa a texto: `Estado::cerrar` lo deja de fondo.
        (Accion::DeFondo, Fase::Texto { .. }) => *vivo = false,
        // «Foto» con la camara del equipo, en el minuto en que se pulsa (la
        // camara se abre a la primera: sus encargos van en orden).
        (Accion::Foto, Fase::Grabando(g)) => {
            let ms = presencial::ms_de_la_foto(g.llevado_ms(), None);
            if estado.abrir_la_camara(r) && !estado.camarografo.foto(ms) {
                estado.avisar(r.camara_fallo.clone());
            }
        }
        // «Clip»: empieza un clip con la camara o para el que va.
        (Accion::Clip, Fase::Grabando(g)) => {
            let ms = g.llevado_ms();
            if estado.camarografo.clip_llevado().is_some() {
                estado.camarografo.parar_clip();
            } else if estado.abrir_la_camara(r) {
                let ruta = estado.ruta_del_clip();
                if estado.camarografo.empezar_clip(ms, ruta.clone()) {
                    estado.apuntar_clip_empezado(ms, &ruta);
                } else {
                    estado.avisar(r.camara_fallo.clone());
                }
            }
        }
        // «Pantalla»: el video de la pantalla donde esta la barra, encendido
        // o apagado; el audio sigue igual.
        (Accion::Pantalla, Fase::Grabando(g)) => {
            let ahora = (g.llevado_ms(), g.muestreo(), g.en_pausa());
            if estado.pantalla_encendida() {
                estado.apagar_pantalla(ahora.0);
                estado.avisar(r.pantalla_apagada.clone());
            } else {
                estado.monitor_video = monitor;
                estado.encender_pantalla(ahora, false, r);
                if estado.pantalla_encendida() {
                    estado.avisar(r.pantalla_encendida.clone());
                }
            }
        }
        // «Captura»: la pantalla entera donde esta la barra, al instante: la
        // barra (y la pantalla completa) no salen (`excluir_de_capturas`) y
        // la foto se hace en su hilo. Mantenerla (o el derecho) es la zona.
        (Accion::Captura, Fase::Grabando(g)) => {
            if !estado.fotografo.zona(g.llevado_ms(), pantalla) {
                estado.avisar(r.sin_zona.clone());
            }
        }
        (Accion::Zona, Fase::Grabando(g)) => {
            let ms = g.llevado_ms();
            pedir_zona(estado, ms, r);
        }
        (Accion::Completa, Fase::Abriendo(_) | Fase::Grabando(_)) => {
            estado.completa = !estado.completa;
        }
        (Accion::LetraMenos | Accion::LetraMas, _) => {
            let paso = if a == Accion::LetraMas {
                completa::PASO_LETRA
            } else {
                -completa::PASO_LETRA
            };
            estado.letra = completa::letra_valida(estado.letra + paso);
            recordar_letra(&pedido.ubicacion, estado.letra);
        }
        // El reloj: un clic pausa o sigue (mantenerlo termina, en `flotar`).
        (Accion::Reloj, Fase::Grabando(g)) => {
            let pausa = !g.en_pausa();
            g.pausar(pausa);
            if let Some(v) = estado.video_en_marcha() {
                v.pausar(pausa);
            }
            // En pausa no se graba nada: tampoco el clip.
            if pausa {
                estado.camarografo.parar_clip();
            }
        }
        (Accion::Terminar, Fase::Grabando(_)) => {
            // La escucha se para ya (lo ultimo que marco Jev se recoge): la
            // transcripcion entera de despues es la de siempre.
            estado.acabar_escucha(Some((pedido, r)));
            estado.completa = false;
            if let Fase::Grabando(g) = std::mem::replace(&mut estado.fase, Fase::Preparar) {
                estado.fase = Fase::Parando {
                    audio: crate::ventanita::cerrar_microfono(g),
                    listo: None,
                };
            }
        }
        (Accion::CamaraMovil, _) => crate::sincronizar::camara::alternar(pedido.ubicacion.raiz()),
        (Accion::Tirar, Fase::Grabando(_)) => {
            if estado.tirar_desde.is_some() {
                estado.acabar_escucha(None);
                crate::sincronizar::camara::tirada(pedido.ubicacion.raiz());
                if let Fase::Grabando(g) = std::mem::replace(&mut estado.fase, Fase::Preparar) {
                    g.cancelar();
                }
                estado.tirar_trozos();
                estado.reparto = None;
                estado.completa = false;
                let _ = estado.fotografo.acabar(Duration::from_secs(2));
                estado.tirar_clips();
                estado.capturas.clear();
                // Tirar es lo unico que se lleva el diario: lo pidio el
                // usuario (✕ dos veces).
                estado.tirar_diario();
                tracing::info!("clase tirada sin guardar");
                *vivo = false;
            } else {
                estado.tirar_desde = Some(Instant::now());
            }
        }
        // Mientras se pasa a texto, «Sin texto» lo deja: la nota ya esta
        // guardada.
        (Accion::Tirar, Fase::Texto { id, en_marcha }) => {
            // El usuario no la quiere con texto: tampoco al volver a abrir.
            cola_de_texto::quitar(pedido.ubicacion.raiz(), id);
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
        Fase::Preparar if estado.editando_temas => {
            temas::pintar_editor(p, w, h, e, estado, r, llevado, botones)
        }
        Fase::Preparar => pintar_preparar(p, w, e, estado, pedido, r, botones),
        Fase::Abriendo(_) | Fase::Grabando(_) if estado.completa => {
            completa::pintar(p, w, h, e, estado, r, llevado, botones);
        }
        Fase::Abriendo(_) | Fase::Grabando(_) => {
            pintar_barra(p, w, e, estado, r, llevado, botones);
            temas::pintar_panel(p, w, e, estado, r, botones);
            reunion::pintar(p, w, e, estado, &r.reunion, botones);
        }
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
            estado.para_estudiar.is_some(),
            botones,
        ),
        Fase::Fallo { texto, .. } => pintar_final(
            p,
            w,
            e,
            &mi::MIC,
            ROJO_GRABAR,
            texto,
            None,
            r,
            false,
            botones,
        ),
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
    estudiar: bool,
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
    // Con la nota guardada, «Abrir para estudiar» delante de «Listo»: es lo
    // siguiente que se suele querer hacer con una clase.
    let y = 122.0 * e;
    let alto = 34.0 * e;
    if estudiar {
        let ancho_listo = 96.0 * e;
        let hueco = 8.0 * e;
        let ancho_estudiar = (w - 36.0 * e - ancho_listo - hueco).min(200.0 * e);
        let x0 = (w - ancho_estudiar - hueco - ancho_listo) / 2.0;
        boton(
            p,
            botones,
            RectF {
                x: x0,
                y,
                ancho: ancho_estudiar,
                alto,
            },
            Accion::Estudiar,
            &mi::MENU_BOOK,
            &r.estudiar,
            OSCURO.enviar,
            Color::BLANCO,
            e,
        );
        botones.boton(
            p,
            RectF {
                x: x0 + ancho_estudiar + hueco,
                y,
                ancho: ancho_listo,
                alto,
            },
            Accion::Listo,
            &r.listo,
            None,
            e,
        );
        return;
    }
    botones.boton(
        p,
        RectF {
            x: (w - 120.0 * e) / 2.0,
            y,
            ancho: 120.0 * e,
            alto,
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

/// **La ficha de la calidad del video**, en la fila de «Empezar grabando la
/// pantalla», a la izquierda de su pildora: tan ancha como su rotulo.
fn ficha_de_calidad(p: &Pintor, fila: RectF, rotulo: &str, e: f32) -> RectF {
    let (tw, _) = p.medir_texto(rotulo, 12.0 * e);
    let ancho = tw + 16.0 * e;
    let alto = 24.0 * e;
    // La pildora: 38 de ancho y 12 de margen; y un hueco de 8.
    RectF {
        x: fila.x + fila.ancho - (12.0 + 38.0 + 8.0) * e - ancho,
        y: fila.y + (fila.alto - alto) / 2.0,
        ancho,
        alto,
    }
}

/// Las filas de Preparar, en logicos desde arriba: «Empezar grabando la
/// pantalla», los dos sonidos, la camara, donde se guarda, los temas y el
/// boton grande.
const Y_MODO: f32 = 68.0;
const Y_MICROFONO: f32 = 118.0;
const Y_EQUIPO: f32 = 170.0;
const Y_TERCERA: f32 = 222.0;
const Y_PROYECTO: f32 = 274.0;
const Y_TEMAS: f32 = 326.0;
const Y_EMPEZAR: f32 = 386.0;
const Y_AVISO: f32 = 444.0;

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

/// Una fila de Preparar sin pildora: icono y rotulo y, si se puede pulsar,
/// su zona. `reservado`: lo que el rotulo deja libre a la derecha.
#[allow(clippy::too_many_arguments)]
fn fila_sin_pildora(
    p: &Pintor,
    botones: &mut Botones<Accion>,
    caja: RectF,
    que: Option<Accion>,
    icono: &pixpin_render::icono::Icono,
    rotulo: &str,
    reservado: f32,
    e: f32,
) {
    let tema = &OSCURO;
    let fondo = if que.is_some() && dentro(caja, botones.raton) {
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
        tema.texto,
    );
    let tam = 14.0 * e;
    let (_, th) = p.medir_texto(rotulo, tam);
    p.texto_linea(
        rotulo,
        caja.x + 42.0 * e,
        caja.y + (caja.alto - th) / 2.0,
        tam,
        caja.ancho - 54.0 * e - reservado,
        tema.texto,
    );
    if let Some(a) = que {
        botones.zona(caja, a);
    }
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
    let fila = |y: f32| filas_de_preparar(w, e, y);
    // Con el raton sobre la ficha de la calidad, la linea de abajo del
    // titulo dice que es cada una (lo que pesa una hora): es su pista. Si
    // no, que la grabacion es una sola, con todo en la barra.
    let (calidad, pista_calidad) = r.calidad(estado.perfil_video);
    let ficha = estado
        .pantalla_al_empezar
        .then(|| ficha_de_calidad(p, fila(Y_MODO), calidad, e));
    let sobre_la_calidad = ficha.is_some_and(|f| dentro(f, botones.raton));
    p.texto_linea(
        if sobre_la_calidad {
            pista_calidad
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

    // «Empezar grabando la pantalla», con su calidad (un clic en la ficha
    // pasa a la siguiente); apagado, «Pantalla» de la barra lo enciende
    // cuando haga falta.
    interruptor(
        p,
        botones,
        fila(Y_MODO),
        Accion::PantallaAlEmpezar,
        &mi::SCREEN_SHARE,
        &r.pantalla_al_empezar,
        estado.pantalla_al_empezar,
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
        botones.zona(f, Accion::Calidad);
    }

    // Los sonidos, en los dos modos: se elige cuales.
    interruptor(
        p,
        botones,
        fila(Y_MICROFONO),
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
        fila(Y_EQUIPO),
        Accion::Equipo,
        &mi::VOLUME_UP,
        &r.equipo,
        estado.equipo,
        0.0,
        e,
    );
    // La camara de «Foto» y «Clip»: un clic pasa a la siguiente.
    let rotulo = match estado.camaras.get(estado.camara) {
        Some(c) => con(&r.camara, &c.nombre),
        None if estado.buscando_camaras.is_some() => r.buscando_camaras.clone(),
        None => r.sin_camaras.clone(),
    };
    let varias = estado.camaras.len() > 1;
    fila_sin_pildora(
        p,
        botones,
        fila(Y_TERCERA),
        Some(Accion::Camara),
        &mi::PHOTO_CAMERA,
        &rotulo,
        if varias { 28.0 * e } else { 0.0 },
        e,
    );
    if varias {
        let caja = fila(Y_TERCERA);
        let lado = 20.0 * e;
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
    // Donde se guarda: un clic pasa al siguiente proyecto.
    let caja = fila(Y_PROYECTO);
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
    // Los temas de la reunion: un clic abre su pantalla.
    temas::pintar_fila(p, fila(Y_TEMAS), e, estado, r, botones);
    // Y a su derecha, lo que se piensa que dura (el tema olvidado).
    reunion::pintar_duracion(p, fila(Y_TEMAS), e, estado, &r.reunion, botones);

    // El boton grande.
    let caja = RectF {
        x: m,
        y: Y_EMPEZAR * e,
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
            Y_AVISO * e,
            12.0 * e,
            ROJO_GRABAR,
        );
    }
}

/// **El reloj de la barra** «● 12:34» (y el de la pantalla completa, mas
/// grande): un boton. El punto rojo late grabando y pasa a ❚❚ en pausa; un
/// clic pausa o sigue y mantenerlo termina (`flotar`).
#[allow(clippy::too_many_arguments)]
fn pintar_reloj(
    p: &Pintor,
    botones: &mut Botones<Accion>,
    caja: RectF,
    g: Option<&pixpin_audio::Grabadora>,
    (llevado, mantenido): (Duration, Option<Instant>),
    tam: f32,
    e: f32,
) {
    let tema = &OSCURO;
    let pausa = g.is_some_and(|g| g.en_pausa());
    let fondo = if g.is_some() && dentro(caja, botones.raton) {
        mas_claro(tema.campo)
    } else {
        tema.campo
    };
    p.rellenar_redondeado(caja, 10.0 * e, fondo);
    // Mantenido, se llena de rojo de izquierda a derecha hasta terminar.
    if let Some(desde) = mantenido {
        let parte = (desde.elapsed().as_secs_f32() / MANTENER.as_secs_f32()).clamp(0.0, 1.0);
        p.rellenar_redondeado(
            RectF {
                ancho: (caja.ancho * parte).max(10.0 * e),
                ..caja
            },
            10.0 * e,
            con_alfa(ROJO_GRABAR, 0.55),
        );
    }
    let tiempo = g
        .map(|g| pixpin_audio::duracion_legible(g.llevado_ms()))
        .unwrap_or_else(|| "0:00".into());
    let (tw, th) = p.medir_texto(&tiempo, tam);
    let lado = tam * 0.62;
    let hueco = tam * 0.4;
    let x = caja.x + (caja.ancho - lado - hueco - tw) / 2.0;
    let cy = caja.y + caja.alto / 2.0;
    let latido = (llevado.as_millis() % 1000) as f32 / 1000.0;
    if pausa {
        p.icono(
            &mi::PAUSE,
            RectF {
                x: x - lado * 0.2,
                y: cy - lado * 0.7,
                ancho: lado * 1.4,
                alto: lado * 1.4,
            },
            tema.texto,
        );
    } else if g.is_some() && latido < 0.6 {
        p.circulo((x + lado / 2.0, cy), lado / 2.0, ROJO_GRABAR);
    }
    p.texto(&tiempo, x + lado + hueco, cy - th / 2.0, tam, tema.texto);
    if g.is_some() {
        botones.zona(caja, Accion::Reloj);
    }
}

/// Las cuatro acciones de la grabacion, en el orden de la barra y de la
/// pantalla completa: `(accion, icono, rotulo, fondo, tinta)`.
fn acciones(
    estado: &Estado,
    r: &Rotulos,
) -> [(
    Accion,
    &'static pixpin_render::icono::Icono,
    String,
    Color,
    Color,
); 4] {
    let tema = &OSCURO;
    let destello = |de_camara: bool| {
        estado
            .destello
            .is_some_and(|(d, c)| c == de_camara && d.elapsed() < Duration::from_millis(600))
    };
    // La foto que se esta haciendo ya se ve en su boton («Foto…»).
    let foto = if estado.camarografo.pendientes() > 0 {
        format!("{}…", r.foto)
    } else {
        r.foto.clone()
    };
    // Grabando, en rojo: la pantalla (si no se corto) y el clip, con lo que
    // lleva.
    let pantalla = estado.video_en_marcha();
    let pantalla_roja = pantalla.is_some_and(|v| !v.roto());
    let clip = estado.camarografo.clip_llevado();
    let rojo = (ROJO_GRABAR, Color::BLANCO);
    let normal = (tema.campo, tema.texto);
    let (fondo_pantalla, tinta_pantalla) = if pantalla_roja { rojo } else { normal };
    let (fondo_clip, tinta_clip) = if clip.is_some() { rojo } else { normal };
    [
        (
            Accion::Foto,
            &mi::PHOTO_CAMERA,
            foto,
            if destello(true) { VERDE } else { tema.enviar },
            Color::BLANCO,
        ),
        (
            Accion::Captura,
            &mi::IMAGE,
            r.captura.clone(),
            if destello(false) { VERDE } else { tema.enviar },
            Color::BLANCO,
        ),
        (
            Accion::Pantalla,
            &mi::SCREEN_SHARE,
            r.pantalla.clone(),
            fondo_pantalla,
            tinta_pantalla,
        ),
        (
            Accion::Clip,
            if clip.is_some() {
                &mi::STOP
            } else {
                &mi::VIDEOCAM
            },
            match clip {
                Some(d) => minuto(d.as_millis() as i64),
                None => r.clip.clone(),
            },
            fondo_clip,
            tinta_clip,
        ),
    ]
}

/// Las imagenes y clips hechos (las que aun se estan haciendo cuentan ya:
/// el clic se vio).
fn hechas(estado: &Estado) -> usize {
    estado.capturas.len()
        + estado.fotografo.pendientes()
        + estado.camarografo.pendientes()
        + estado.clips.len()
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
    let alto = 44.0 * e;
    let y = 10.0 * e;
    // ● 12:34: el reloj es el boton de la pausa (y de terminar).
    pintar_reloj(
        p,
        botones,
        RectF {
            x: 16.0 * e,
            y,
            ancho: 96.0 * e,
            alto,
        },
        g,
        (llevado, estado.manteniendo),
        18.0 * e,
        e,
    );
    // El nivel: cinco barras que dicen que de verdad entra sonido.
    let nivel = g
        .map(|g| (g.nivel() as f32 / pixpin_audio::PICO_MAXIMO as f32).clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let x_nivel = 124.0 * e;
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

    // Las acciones, cada una con su nombre, de izquierda a derecha.
    let mut x = 162.0 * e;
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
    for ((accion, icono, rotulo, fondo, tinta), ancho) in acciones(estado, r)
        .into_iter()
        .zip([84.0, 100.0, 104.0, 76.0])
    {
        boton_si(
            p,
            botones,
            sitio(ancho),
            accion,
            icono,
            &rotulo,
            fondo,
            tinta,
            e,
            activo,
        );
    }
    // Con temas, «Temas 2/5»: abre o pliega su lista.
    if !estado.temas.is_empty() {
        temas::pintar_boton(
            p,
            sitio(temas::ANCHO_BOTON as f32 - 8.0),
            e,
            estado,
            activo,
            botones,
        );
    }
    // ⛶ la pantalla completa y ✕ tirar.
    boton_si(
        p,
        botones,
        sitio(36.0),
        Accion::Completa,
        &mi::FULLSCREEN,
        "",
        tema.campo,
        tema.texto,
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
    let hechas = hechas(estado);
    let (linea, color) = if g.is_none() {
        (r.abriendo.clone(), tema.campo_apagado)
    } else if tirar {
        (r.tirar.clone(), ROJO_GRABAR)
    } else if let Some((a, _)) = &estado.aviso {
        (a.clone(), tema.texto)
    } else if pausa {
        (r.en_pausa.clone(), tema.campo_apagado)
    } else if hechas == 0 {
        (r.pista_unica.clone(), tema.campo_apagado)
    } else {
        (
            format!(
                "{} · {}",
                con(&r.imagenes, &hechas.to_string()),
                r.pista_unica
            ),
            tema.campo_apagado,
        )
    };
    let mut ancho_linea = w - 32.0 * e;
    // El movil como camara, a la derecha: un clic la apaga o la enciende.
    if g.is_some()
        && let Some((texto, encendida)) = crate::sincronizar::camara::indicador_actual()
    {
        let tam = 12.0 * e;
        let (tw, th) = p.medir_texto(&texto, tam);
        let x = 16.0 * e + ancho_linea - tw;
        let caja = RectF {
            x: x - 4.0 * e,
            y: 58.0 * e,
            ancho: tw + 8.0 * e,
            alto: th + 8.0 * e,
        };
        if dentro(caja, botones.raton) {
            p.rellenar_redondeado(caja, 6.0 * e, tema.campo);
        }
        let tinta = if encendida {
            tema.texto
        } else {
            tema.campo_apagado
        };
        p.texto(&texto, x, 62.0 * e, tam, tinta);
        botones.zona(caja, Accion::CamaraMovil);
        ancho_linea -= tw + 16.0 * e;
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
    fn las_filas_de_preparar_caben_en_su_ventana() {
        let (_, alto) = tamano(&Fase::Preparar);
        // El boton grande y el aviso de debajo.
        const { assert!(Y_EMPEZAR + 50.0 <= Y_AVISO) };
        assert!(Y_AVISO + 16.0 <= alto as f32, "{alto}");
        // Las filas no se pisan (44 cada una).
        for (a, b) in [
            (Y_MODO + 44.0, Y_MICROFONO),
            (Y_MICROFONO + 44.0, Y_EQUIPO),
            (Y_EQUIPO + 44.0, Y_TERCERA),
            (Y_TERCERA + 44.0, Y_PROYECTO),
            (Y_PROYECTO + 44.0, Y_TEMAS),
            (Y_TEMAS + 44.0, Y_EMPEZAR),
        ] {
            assert!(a <= b, "{a} > {b}");
        }
    }

    #[test]
    fn la_barra_es_tan_ancha_como_sus_botones() {
        // El fallo: la barra medía 520 y sus botones pedían 538; el ✕ de
        // tirar salía cortado. 162 de reloj y nivel, las cuatro acciones,
        // ⛶ y ✕ (ver `pintar_barra`) y un margen.
        let barra = 162 + (84 + 8) + (100 + 8) + (104 + 8) + (76 + 8) + (36 + 8) + 36;
        let (_, rx) = channel();
        let (w, _) = tamano(&Fase::Abriendo(rx));
        assert!(w >= barra + 16, "{w} < {barra}");
        // Con temas, su boton (72 y el hueco) cabe en lo que se suma.
        assert!(w + temas::ANCHO_BOTON >= barra + 72 + 8 + 16);
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
            alto: 412,
        };
        let m = reencajar(area, 150, tamano(&Fase::Preparar), antes);
        assert_eq!((m.ancho, m.alto), (510, 696), "la escala de la nueva");
        assert!(m.x >= area.x && m.x + m.ancho as i32 <= area.x + area.ancho as i32);
        assert!(m.y >= area.y && m.y + m.alto as i32 <= area.y + area.alto as i32);
        // Caso negativo: dentro y a la misma escala no se mueve.
        let quieta = Rect {
            x: 2200,
            y: 100,
            ancho: 340,
            alto: 464,
        };
        assert_eq!(
            reencajar(area, 100, tamano(&Fase::Preparar), quieta),
            quieta
        );
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

    #[test]
    fn la_clase_entra_en_su_carpeta_en_el_orden_de_la_grabacion() {
        let c = carpeta_de_la_grabacion(
            Some("v"),
            90_000,
            Some("mp4"),
            &[(65_000, "f2".into()), (5_000, "f1".into())],
            &[presencial::Clip {
                ms: 30_000,
                duracion_ms: 8_000,
                id: "k".into(),
            }],
        );
        assert!(c.es_grabacion());
        assert_eq!((c.voz.as_str(), c.duracion_ms), ("v", 90_000));
        let ids: Vec<&str> = c.paginas.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["v", "mp4", "f1", "k", "f2"]);
        assert_eq!(c.paginas[4].titulo, "⏱ 1:05");
        assert_eq!(c.paginas[4].ms, Some(65_000));
        assert_eq!(c.paginas[3].titulo, "▶ 0:30", "el clip, en su minuto");
        assert_eq!(c.paginas[0].titulo, "", "la nota con su nombre");
        assert_eq!(c.video, "mp4");
        assert_eq!(c.capturas().count(), 3);
        // Caso negativo: sin nota (no se pudo cerrar), solo las capturas.
        let sola = carpeta_de_la_grabacion(None, 0, None, &[(1, "f".into())], &[]);
        assert_eq!(sola.voz, "");
        assert_eq!(sola.len(), 1);
        assert!(carpeta_de_la_grabacion(None, 0, None, &[], &[]).is_empty());
    }
}
