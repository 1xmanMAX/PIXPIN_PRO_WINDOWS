//! **Hablarle al asistente y oir lo que contesta** (13-oct-2026).
//!
//! Dos cosas, las dos iguales con los tres motores (local, Claude,
//! DeepSeek), porque viven en el chat y no en el motor: entra texto y sale
//! texto, y la voz se queda a este lado.
//!
//! ## El microfono de la caja
//!
//! En el chat «Asistente» (y en otro con la caja vacia salvo `@asistente`)
//! el boton redondo de la caja es el **microfono del asistente**: se pulsa
//! y se habla, y se pulsa otra vez para acabar; o se **mantiene pulsado**
//! mientras se habla y se suelta ([`MANTENER`]). Graba con la misma
//! grabadora que las notas de voz (asi el nivel y el cronometro se ven
//! igual), pero lo grabado **no se guarda como nota**: se pasa a texto con
//! **Whisper** sobre el ONNX Runtime de Windows (`pixpin_voz::whisper`) y
//! se borra. El reconocedor de Windows (SAPI) entiende mal el habla
//! natural (medido con las notas del usuario, 22-sep): aqui no se usa, ni
//! de reserva. La primera vez se baja el modelo, con la misma barra.
//!
//! Lo oido se **envia solo**, como un mensaje escrito; con el ajuste
//! «Ver lo dicho antes de enviarlo» (`[asistente] revisar_lo_dicho`) se
//! queda en la caja para corregirlo.
//!
//! ## El altavoz de la respuesta
//!
//! Cada respuesta acabada lleva un **altavoz** junto a la hora: la lee en
//! voz alta el **lector de voz comun de la app** (`leer_en_voz`: las voces
//! de Microsoft si estan encendidas en el engranaje de los lectores, con
//! caida a la de Windows). **Nunca se lee sola al acabar**: el usuario
//! quito «Leer siempre las respuestas» (17-oct-2026); solo suena al pulsar
//! el altavoz. **No se lee** lo
//! que no es la respuesta: el pensamiento (`<think>…</think>`), el JSON de
//! las herramientas, los bloques de codigo, las lineas de estado
//! («✏️ dibujando…») ni los avisos (ver [`texto_para_leer`]). Hablarle al
//! microfono la calla.
//!
//! Todo vive en el hilo de la ventana (`thread_local`), como
//! `ventana_chat/asistente.rs`; lo unico que va en otro hilo es Whisper.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use pixpin_proyecto::cuaderno;
use pixpin_voz::ErrorVoz;

use super::*;
use crate::asistente::chat as ia_chat;
use crate::leer_en_voz::{self, LeerEnVoz};

/// Mantenido al menos esto, **soltar el boton acaba** de grabar («mantener
/// para hablar»). Un clic normal dura menos: entonces se sigue grabando
/// hasta el siguiente clic.
pub(super) const MANTENER: Duration = Duration::from_millis(450);

/// Lo transcrito que esta pasando a texto.
struct Pasando {
    proyecto: String,
    /// Lo que habia en la caja al empezar (`@asistente` en otro chat): va
    /// delante de lo oido.
    prefijo: String,
    avance: Arc<AtomicU32>,
    bajando: Arc<AtomicBool>,
    cancelado: Arc<AtomicBool>,
    resultado: Receiver<Result<String, ErrorVoz>>,
}

impl Drop for Pasando {
    fn drop(&mut self) {
        // Soltarlo (cancelar, cerrar la app) no deja a Whisper moliendo.
        self.cancelado.store(true, Ordering::Relaxed);
    }
}

/// La respuesta que suena.
struct Leyendo {
    proyecto: String,
    nota: String,
    lector: LeerEnVoz,
}

thread_local! {
    static PASANDO: RefCell<Option<Pasando>> = const { RefCell::new(None) };
    static LEYENDO: RefCell<Option<Leyendo>> = const { RefCell::new(None) };
}

// ------------------------------------------------------------ el microfono

/// Si en la caja no hay nada, o solo la mencion: `Some(false)` vacia,
/// `Some(true)` solo `@asistente`, `None` hay algo escrito.
fn solo_la_mencion(borrador: &str) -> Option<bool> {
    let t = borrador.trim();
    if t.is_empty() {
        Some(false)
    } else if t.eq_ignore_ascii_case(ia_chat::MENCION) {
        Some(true)
    } else {
        None
    }
}

/// **Si el boton redondo de la caja es ahora el microfono del asistente**:
/// en su chat con la caja vacia, o en cualquiera con solo `@asistente`
/// escrito; sin grabar ni dictar ni pasar nada a texto, y sin el asistente
/// contestando (entonces el boton es «parar»).
pub(super) fn microfono_del_asistente(a: &Abierto) -> bool {
    a.grabando.is_none()
        && a.dictado.is_none()
        && !pasando(&a.ficha.id)
        && !asistente::ocupado(&a.ficha.id)
        && match solo_la_mencion(&a.borrador) {
            Some(true) => true,
            Some(false) => a.ficha.es_asistente(),
            None => false,
        }
}

/// Si lo que se graba ahora es para el asistente (y no una nota de voz).
pub(super) fn grabando_para_el_asistente(a: &Abierto) -> bool {
    a.grabando
        .as_ref()
        .is_some_and(|g| g.para_el_asistente.is_some())
}

/// **Empieza a oir para el asistente.** Calla lo que se estuviera leyendo:
/// el usuario habla.
pub(super) fn empezar(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
) -> Result<(), pixpin_audio::ErrorAudio> {
    callar();
    empezar_a_grabar(ubicacion, a)?;
    if let Some(g) = a.grabando.as_mut() {
        g.para_el_asistente = Some(Instant::now());
    }
    Ok(())
}

/// Al soltar el boton del raton: si se mantuvo pulsado lo bastante
/// ([`MANTENER`]), se acabo de hablar.
pub(super) fn al_soltar(a: &Abierto) -> bool {
    a.grabando
        .as_ref()
        .and_then(|g| g.para_el_asistente)
        .is_some_and(|desde| desde.elapsed() >= MANTENER)
}

/// **Acaba de oir** y lanza Whisper en su hilo. Devuelve un aviso si no se
/// pudo (demasiado corto, sin Whisper en el equipo).
pub(super) fn terminar(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    idioma: pixpin_store::Idioma,
    textos: &Catalogo,
) -> Option<String> {
    let g = a.grabando.take()?;
    let temporal = g.temporal.clone();
    let grabacion = match g.grabadora.parar() {
        Ok(x) => x,
        Err(pixpin_audio::ErrorAudio::DemasiadoCorta { .. }) => {
            let _ = std::fs::remove_file(&temporal);
            return Some(textos.t("chat-voz-muy-corta"));
        }
        Err(e) => {
            tracing::warn!(?e, "asistente: no se pudo cerrar lo grabado");
            let _ = std::fs::remove_file(&temporal);
            return Some(textos.t("asistente-voz-fallo"));
        }
    };
    if grabacion.ruta != temporal {
        let _ = std::fs::remove_file(&temporal);
    }
    let prefijo = match solo_la_mencion(&a.borrador) {
        Some(true) => a.borrador.trim().to_string(),
        _ => String::new(),
    };
    lanzar(
        ubicacion,
        &a.ficha.id,
        prefijo,
        grabacion.ruta,
        idioma,
        textos,
    )
}

/// Lanza Whisper sobre `audio` (que se borra al acabar). Las pruebas lo
/// llaman con un WAV hecho a mano.
pub(super) fn lanzar(
    ubicacion: &Ubicacion,
    proyecto: &str,
    prefijo: String,
    audio: PathBuf,
    idioma: pixpin_store::Idioma,
    textos: &Catalogo,
) -> Option<String> {
    if !crate::voz::whisper_posible(ubicacion, idioma) {
        let _ = std::fs::remove_file(&audio);
        return Some(textos.t("asistente-voz-sin-whisper"));
    }
    let raiz = ubicacion.raiz().to_path_buf();
    let bajar = !pixpin_voz::whisper::modelo_listo(&raiz);
    let idiomas = crate::voz::idiomas_de(ubicacion, idioma);
    let avance = Arc::new(AtomicU32::new(0));
    let bajando = Arc::new(AtomicBool::new(bajar));
    let cancelado = Arc::new(AtomicBool::new(false));
    let (enviar, resultado) = channel();
    let hilo = {
        let (avance, bajando, cancelado) = (
            Arc::clone(&avance),
            Arc::clone(&bajando),
            Arc::clone(&cancelado),
        );
        std::thread::Builder::new()
            .name("asistente-oir".into())
            .spawn(move || {
                // Media Foundation decodifica el audio en este hilo.
                let _com = pixpin_shell::ComDelHilo::iniciar();
                let mut avisar =
                    |f: f32| avance.store((f.clamp(0.0, 1.0) * 1000.0) as u32, Ordering::Relaxed);
                let r = (|| {
                    if bajar {
                        crate::voz::bajar_modelo(&raiz, &mut avisar, &cancelado)?;
                        bajando.store(false, Ordering::Relaxed);
                        avisar(0.0);
                    }
                    let _turno = crate::turno_pesado::tomar("transcribir");
                    let exe = pixpin_shell::entorno::directorio_del_ejecutable().ok();
                    lo_dicho(
                        &audio,
                        &raiz,
                        exe.as_deref(),
                        &idiomas,
                        &mut avisar,
                        &cancelado,
                    )
                })();
                let _ = std::fs::remove_file(&audio);
                let _ = enviar.send(r);
            })
    };
    if let Err(e) = hilo {
        tracing::warn!(?e, "asistente: sin hilo para pasar la voz a texto");
        return Some(textos.t("asistente-voz-fallo"));
    }
    PASANDO.with_borrow_mut(|p| {
        *p = Some(Pasando {
            proyecto: proyecto.to_string(),
            prefijo,
            avance,
            bajando,
            cancelado,
            resultado,
        })
    });
    None
}

/// **Lo dicho en `audio`, en texto**, solo con Whisper. Sin nada creible
/// (silencio, ruido) es [`ErrorVoz::NoSeEntiendeNada`].
pub(super) fn lo_dicho(
    audio: &Path,
    raiz: &Path,
    exe: Option<&Path>,
    idiomas: &pixpin_voz::Idiomas,
    avance: &mut dyn FnMut(f32),
    cancelado: &AtomicBool,
) -> Result<String, ErrorVoz> {
    let t =
        pixpin_voz::whisper::transcribir_con(audio, raiz, exe, idiomas, &[], avance, cancelado)?;
    let texto = limpiar_lo_oido(t.segmentos.iter().map(|s| s.texto.as_str()));
    if texto.is_empty() {
        Err(ErrorVoz::NoSeEntiendeNada)
    } else {
        Ok(texto)
    }
}

/// Los trozos de Whisper en una frase: sin los rotulos que se inventa con
/// el silencio o la musica («[Música]», «(risas)») y con los blancos juntos.
pub(super) fn limpiar_lo_oido<'a>(trozos: impl Iterator<Item = &'a str>) -> String {
    let sueltos: Vec<&str> = trozos
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .filter(|t| {
            let entre = |a: char, b: char| t.starts_with(a) && t.ends_with(b);
            !(entre('[', ']') || entre('(', ')') || entre('*', '*') || entre('♪', '♪'))
        })
        .collect();
    pixpin_docs::voz_alta::juntar_blancos(&sueltos.join(" "))
}

/// Lo oido detras de lo que habia (`@asistente`).
pub(super) fn juntar(prefijo: &str, oido: &str) -> String {
    if prefijo.is_empty() {
        oido.to_string()
    } else {
        format!("{prefijo} {oido}")
    }
}

/// Si se esta pasando a texto lo dicho en este chat.
pub(super) fn pasando(proyecto: &str) -> bool {
    PASANDO.with_borrow(|p| p.as_ref().is_some_and(|p| p.proyecto == proyecto))
}

/// Cancela lo que se este pasando a texto (el boton mientras se pasa).
pub(super) fn cancelar() {
    PASANDO.with_borrow_mut(|p| {
        if let Some(p) = p.as_ref() {
            p.cancelado.store(true, Ordering::Relaxed);
        }
    });
}

/// «Pasando tu voz a texto… 40 %», en la caja mientras dura.
pub(super) fn rotulo_pasando(a: &Abierto, textos: &Catalogo) -> Option<String> {
    PASANDO.with_borrow(|p| {
        let p = p.as_ref().filter(|p| p.proyecto == a.ficha.id)?;
        let mut args = fluent_bundle::FluentArgs::new();
        args.set(
            "pct",
            (p.avance.load(Ordering::Relaxed) as f32 / 10.0).round() as i64,
        );
        let clave = if p.bajando.load(Ordering::Relaxed) {
            "asistente-voz-bajando"
        } else {
            "asistente-voz-pasando"
        };
        Some(textos.t_args(clave, &args))
    })
}

/// El aviso de un fallo de Whisper, en palabras del usuario.
pub(super) fn clave_del_fallo(e: &ErrorVoz) -> &'static str {
    match e {
        ErrorVoz::NoSeEntiendeNada | ErrorVoz::AudioVacio => "asistente-voz-nada",
        ErrorVoz::Cancelada => "asistente-voz-cancelado",
        ErrorVoz::SinMicrofono => "chat-voz-sin-microfono",
        _ => "asistente-voz-fallo",
    }
}

/// Lo que dejo una vuelta del latido.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Vuelta {
    pub cambio: bool,
    pub aviso: Option<String>,
    /// Lo oido ya esta en la caja y hay que enviarlo como si se pulsara
    /// enviar (lo hace el bucle: es quien sabe quien escribe).
    pub enviar: bool,
}

/// **Una vez por vuelta del bucle**: recoge lo que dio Whisper, lee las
/// respuestas recien acabadas si se pidio y sigue a lo que suena.
pub(super) fn latido(
    ubicacion: &Ubicacion,
    abierto: Option<&mut Abierto>,
    textos: &Catalogo,
) -> Vuelta {
    let mut v = Vuelta::default();

    // 1. Lo dicho.
    let llegado = PASANDO.with_borrow_mut(|p| {
        let r = match p.as_ref()?.resultado.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err(ErrorVoz::Cancelada),
        };
        let hecho = p.take()?;
        Some((hecho.proyecto.clone(), hecho.prefijo.clone(), r))
    });
    if PASANDO.with_borrow(Option::is_some) {
        // La barra de avance se mueve sola.
        v.cambio = true;
    }
    if let Some((proyecto, prefijo, r)) = llegado {
        v.cambio = true;
        match r {
            Ok(oido) => {
                if let Some(a) = abierto.filter(|a| a.ficha.id == proyecto) {
                    a.borrador = juntar(&prefijo, &oido);
                    a.cursor.poner(&a.borrador, a.borrador.len(), false);
                    let revisar = pixpin_store::cargar(ubicacion)
                        .map(|aj| aj.asistente.revisar_lo_dicho)
                        .unwrap_or(false);
                    v.enviar = !revisar;
                }
            }
            Err(e) => {
                tracing::info!(?e, "asistente: lo dicho no paso a texto");
                v.aviso = Some(textos.t(clave_del_fallo(&e)));
            }
        }
    }

    // 2. Lo que suena: el altavoz cambia al acabar o si otro lector se
    //    quedo con la voz.
    LEYENDO.with_borrow_mut(|l| {
        let Some(x) = l.as_mut() else { return };
        match x.lector.vuelta() {
            Some(leer_en_voz::Suceso::Acabado) => {
                if let Some(x) = l.take() {
                    x.lector.soltar();
                }
                v.cambio = true;
            }
            Some(leer_en_voz::Suceso::Fuera) => {
                *l = None;
                v.cambio = true;
            }
            Some(_) => v.cambio = true,
            None => {}
        }
    });
    v
}

/// Cada cuanto volver a mirar (ms) mientras hay algo de voz en marcha.
pub(super) fn espera() -> Option<u32> {
    let pasando = PASANDO.with_borrow(Option::is_some);
    let leyendo = LEYENDO.with_borrow(Option::is_some);
    if pasando {
        Some(250)
    } else {
        leyendo.then_some(300)
    }
}

// ------------------------------------------------------------ el altavoz

/// Si la burbuja `i` lleva altavoz: una respuesta del asistente, ya
/// acabada, con algo que leer.
pub(super) fn se_puede_leer(a: &Abierto, i: usize) -> bool {
    let Some(m) = a.mensajes.get(i) else {
        return false;
    };
    if !ia_chat::es_del_asistente(m) || !matches!(m.clase, None | Some(cuaderno::Clase::Nota)) {
        return false;
    }
    // La que se esta escribiendo todavia no.
    let escribiendose = i + 1 == a.mensajes.len() && asistente::ocupado(&a.ficha.id);
    !escribiendose && !texto_para_leer(&m.texto).is_empty()
}

/// Si suena la respuesta `nota` de este chat.
pub(super) fn leyendo(proyecto: &str, nota: &str) -> bool {
    LEYENDO.with_borrow(|l| {
        l.as_ref()
            .is_some_and(|l| l.proyecto == proyecto && l.nota == nota && l.lector.leyendo())
    })
}

/// **Calla** lo que se este leyendo del asistente.
pub(super) fn callar() {
    if let Some(l) = LEYENDO.with_borrow_mut(Option::take) {
        l.lector.soltar();
    }
}

/// El altavoz de la burbuja `i`: si suena, la calla; si no, la lee.
pub(super) fn alternar(
    ubicacion: &Ubicacion,
    a: &Abierto,
    i: usize,
    textos: &Catalogo,
) -> Option<String> {
    let m = a.mensajes.get(i)?;
    if leyendo(&a.ficha.id, &m.id) {
        callar();
        return None;
    }
    leer(ubicacion, a, i, textos)
}

/// De que es la voz, para el lector comun: no es un fichero, pero tiene
/// que ser una clave unica por respuesta.
fn clave(proyecto: &str, nota: &str) -> PathBuf {
    PathBuf::from(format!("pixpin-asistente/{proyecto}/{nota}"))
}

/// **Lee en voz alta la respuesta `i`** con el lector de la app. Devuelve
/// un aviso si no hay nada que leer o no hay voz.
fn leer(ubicacion: &Ubicacion, a: &Abierto, i: usize, textos: &Catalogo) -> Option<String> {
    let m = a.mensajes.get(i)?;
    let parrafos = parrafos_para_leer(&m.texto);
    if parrafos.is_empty() {
        return Some(textos.t("asistente-voz-nada-que-leer"));
    }
    callar();
    let muestra: String = parrafos.join(" ").chars().take(4000).collect();
    let de_la_app = leer_en_voz::idioma_de_la_app(textos);
    let idioma = pixpin_docs::voz_alta::idioma_para_leer(&muestra, "", &de_la_app);
    let doc = leer_en_voz::documento(
        textos,
        ubicacion.raiz(),
        &clave(&a.ficha.id, &m.id),
        a.ficha.nombre.clone(),
        parrafos,
        idioma.clone(),
        // Una respuesta no tiene verde que apuntar.
        Box::new(|_| {}),
    );
    let Some(mut lector) = LeerEnVoz::arrancar(doc) else {
        return Some(leer_en_voz::sin_voz(textos, &idioma));
    };
    lector.leer(0);
    LEYENDO.with_borrow_mut(|l| {
        *l = Some(Leyendo {
            proyecto: a.ficha.id.clone(),
            nota: m.id.clone(),
            lector,
        })
    });
    None
}

/// Pinta el altavoz de la burbuja en `(x, y)` (el renglon de la hora) y
/// devuelve lo pulsable, algo mas grande que el dibujo.
#[allow(clippy::too_many_arguments)] // donde, el alto del renglon, los dos colores y la escala
pub(super) fn pintar_altavoz(
    p: &Pintor,
    a: &Abierto,
    i: usize,
    x: f32,
    y: f32,
    alto_renglon: f32,
    apagado: Color,
    encendido: Color,
    e: f32,
) -> Rect {
    let lado = ALTAVOZ * e;
    let sonando = a
        .mensajes
        .get(i)
        .is_some_and(|m| leyendo(&a.ficha.id, &m.id));
    let caja = RectF {
        x,
        y: y + (alto_renglon - lado) / 2.0,
        ancho: lado,
        alto: lado,
    };
    p.icono(
        if sonando { &mi::STOP } else { &mi::VOLUME_UP },
        caja,
        if sonando { encendido } else { apagado },
    );
    let holgura = 5.0 * e;
    Rect {
        x: (caja.x - holgura) as i32,
        y: (caja.y - holgura) as i32,
        ancho: (caja.ancho + 2.0 * holgura) as u32,
        alto: (caja.alto + 2.0 * holgura) as u32,
    }
}

/// Lo que ocupa el altavoz en el renglon de la hora (sin la escala): el
/// icono y su aire.
pub(super) const ALTAVOZ: f32 = 15.0;
pub(super) const ALTAVOZ_AIRE: f32 = 6.0;

// ------------------------------------------------------- lo que se lee

/// Las etiquetas de pensamiento que traen los modelos locales (Qwen) y las
/// de la nube cuando lo escriben en el texto.
const PENSAR: [(&str, &str); 2] = [("<think>", "</think>"), ("<thinking>", "</thinking>")];

/// Los principios de las lineas de estado de la burbuja
/// (`asistente::chat::rotulo_de` y `EnVivo::mostrar`) y de los avisos.
const ESTADOS: [&str; 12] = [
    "💭", "✏️", "📊", "🔎", "🖼️", "📖", "⚙️", "🔁", "🗂️", "⏹", "⚠️", "📄",
];

/// **Lo que se lee en voz alta de una burbuja del asistente**: solo la
/// respuesta. Fuera el pensamiento, los bloques de codigo, las lineas que
/// son JSON (las llamadas a herramientas que algun modelo escribe en el
/// texto), las lineas de estado y de aviso, las marcas de Markdown, las
/// direcciones y los emojis. Los parrafos quedan separados por saltos de
/// linea.
pub(super) fn texto_para_leer(burbuja: &str) -> String {
    let sin_pensar = quitar_pensamiento(burbuja);
    let sin_codigo =
        quitar_bloques_de_codigo(&sin_pensar.replace("⏹ (parado)", "").replace('▍', ""));
    let mut lineas: Vec<String> = Vec::new();
    for l in sin_codigo.lines() {
        let t = l.trim();
        if t.is_empty()
            || ESTADOS.iter().any(|e| t.starts_with(e))
            || parece_json(t)
            || es_raya_de_tabla(t)
        {
            continue;
        }
        // Una fila de tabla se lee celda a celda: «Ventanas, 2».
        let fila;
        let t = if t.starts_with('|') {
            fila = t
                .split('|')
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .collect::<Vec<_>>()
                .join(", ");
            fila.as_str()
        } else {
            t
        };
        let limpia = sin_emojis(&sin_markdown(t));
        let limpia =
            sin_blanco_antes_de_puntuacion(&pixpin_docs::voz_alta::juntar_blancos(&limpia));
        if limpia.chars().any(char::is_alphanumeric) {
            lineas.push(limpia);
        }
    }
    lineas.join("\n")
}

/// Los parrafos para el lector: uno por renglon.
pub(super) fn parrafos_para_leer(burbuja: &str) -> Vec<String> {
    texto_para_leer(burbuja)
        .lines()
        .map(str::to_string)
        .collect()
}

fn quitar_pensamiento(texto: &str) -> String {
    let mut s = texto.to_string();
    for (abre, cierra) in PENSAR {
        while let Some(i) = s.find(abre) {
            match s[i..].find(cierra) {
                Some(j) => s.replace_range(i..i + j + cierra.len(), ""),
                // Sin cerrar: todo lo de detras es pensamiento.
                None => s.truncate(i),
            }
        }
        // Un cierre suelto (el principio se lo comio el motor): lo de antes
        // era pensamiento.
        if let Some(j) = s.rfind(cierra) {
            s.replace_range(..j + cierra.len(), "");
        }
    }
    s
}

fn quitar_bloques_de_codigo(texto: &str) -> String {
    // Los trozos impares estan dentro de ``` … ```; uno sin cerrar se come
    // hasta el final.
    texto.split("```").step_by(2).collect::<Vec<_>>().join("\n")
}

fn parece_json(t: &str) -> bool {
    (t.starts_with('{') && (t.ends_with('}') || t.contains("\":")))
        || (t.starts_with('[') && t.ends_with(']') && t.contains('{'))
        || t.starts_with("\"") && t.contains("\":")
}

fn es_raya_de_tabla(t: &str) -> bool {
    t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn sin_markdown(linea: &str) -> String {
    let mut t = linea.trim_start_matches(['#', '>', ' ']).to_string();
    // Viñetas: «- », «* », «+ » (los numeros se leen bien: «1. Abre…»).
    for v in ["- ", "* ", "+ ", "• "] {
        if let Some(r) = t.strip_prefix(v) {
            t = r.to_string();
            break;
        }
    }
    // Enlaces [texto](direccion) → texto.
    while let Some(i) = t.find("](") {
        let Some(a) = t[..i].rfind('[') else { break };
        let Some(c) = t[i..].find(')') else { break };
        let dentro = t[a + 1..i].to_string();
        t.replace_range(a..i + c + 1, &dentro);
    }
    let t = t
        .replace("**", "")
        .replace("__", "")
        .replace(['`', '*'], "");
    // Las direcciones sueltas no se leen letra a letra.
    t.split_whitespace()
        .filter(|w| {
            !(w.starts_with("http://") || w.starts_with("https://") || w.starts_with("www."))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// «ventanas .» (quitado el emoji) → «ventanas.».
fn sin_blanco_antes_de_puntuacion(t: &str) -> String {
    let mut s = String::with_capacity(t.len());
    for c in t.chars() {
        if matches!(c, '.' | ',' | ';' | ':' | '!' | '?') && s.ends_with(' ') {
            s.pop();
        }
        s.push(c);
    }
    s
}

fn sin_emojis(t: &str) -> String {
    t.chars()
        .filter(|c| {
            let n = *c as u32;
            !((0x1F000..=0x1FAFF).contains(&n)
                || (0x2600..=0x27BF).contains(&n)
                || (0x2B00..=0x2BFF).contains(&n)
                || n == 0xFE0F
                || n == 0x200D)
        })
        .collect()
}

/// Las pruebas ponen a mano lo que estaria pasando a texto y mandan el
/// resultado cuando quieren.
#[cfg(test)]
pub(super) fn pasando_para_pruebas(
    proyecto: &str,
    prefijo: &str,
    avance: f32,
) -> std::sync::mpsc::Sender<Result<String, ErrorVoz>> {
    let (tx, resultado) = channel();
    PASANDO.with_borrow_mut(|p| {
        *p = Some(Pasando {
            proyecto: proyecto.to_string(),
            prefijo: prefijo.to_string(),
            avance: Arc::new(AtomicU32::new((avance * 1000.0) as u32)),
            bajando: Arc::new(AtomicBool::new(false)),
            cancelado: Arc::new(AtomicBool::new(false)),
            resultado,
        })
    });
    tx
}

/// Que no quede nada de una prueba en la siguiente (mismo hilo).
#[cfg(test)]
pub(super) fn olvidar_para_pruebas() {
    PASANDO.with_borrow_mut(|p| *p = None);
    LEYENDO.with_borrow_mut(|l| *l = None);
}

#[cfg(test)]
mod pruebas;
