//! **Musica con su letra** (27-oct-2026): lo que el movil llama karaoke
//! (`guardados/MusicaActivity.kt`, `TarjetaDeMusica.kt`,
//! `CancionNuevaActivity.kt`), en el chat del PC.
//!
//! Una cancion **no tiene campos propios**: es una nota de voz con
//! `estadoDelTexto = "letra"` (`biblioteca_audio::es_musica`) y su letra en
//! `transcripcion`, con el formato de minutos de siempre (`[m:ss] linea`,
//! un renglon en blanco entre lineas; ver `pixpin_voz::letra_de_cancion`).
//! Asi viaja por la sincronizacion como cualquier nota, y lo que aqui se
//! sincroniza a toques se canta en el telefono con la linea resaltada.
//!
//! Lo que hay aqui:
//!
//! - **Adjuntar → «Musica»** ([`adjuntar`]): elegir audios, que entran ya
//!   marcados como musica; si al lado del fichero hay un `.lrc` o un `.txt`
//!   con su mismo nombre, esa es su letra.
//! - **La tarjeta** de la burbuja ([`tarjeta`], [`pintar_portada`]): la
//!   portada que trae el fichero (o un degradado con la nota), el titulo y
//!   el artista de sus etiquetas, en vez de la onda.
//! - **El karaoke** ([`pintar_karaoke`]): en la pantalla de la letra, la
//!   linea que suena grande y en color a un tercio de la altura, las
//!   cantadas apagadas y las que vienen un poco menos; un clic en una linea
//!   lleva la cancion ahi.
//! - **Sincronizar a toques** ([`Boton::Sincronizar`]): suena desde el
//!   principio y Espacio (o «Marcar») al empezar cada linea le pone su
//!   minuto; Retroceso deshace, Intro guarda y Escape lo deja.
//!
//! El auricular de llamadas del movil no tiene sitio aqui: es del aparato.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use pixpin_geom::Rect;
use pixpin_proyecto::cuaderno::{Clase, Mensaje};
use pixpin_render::{Color, Pintor, RectF};
use pixpin_store::{Catalogo, Ubicacion};
use pixpin_voz::letra_de_cancion as letra;

use super::{Abierto, Efecto, LetraAbierta, Pinta, Zona, con_alfa, hex, icono_centrado, mi, rf};

/// Lo que se pulsa de la musica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Boton {
    /// La pastilla de la burbuja de una cancion: abre su karaoke.
    Karaoke(usize),
    /// El reloj de la cabecera del karaoke.
    Sincronizar,
    Marcar,
    Deshacer,
    Guardar,
    Cancelar,
}

// --- La tarjeta ----------------------------------------------------------------

/// Lo que la tarjeta ensena de una cancion, leido una vez de sus etiquetas.
#[derive(Debug, Default)]
pub(super) struct Tarjeta {
    pub titulo: Option<String>,
    pub artista: Option<String>,
    pub album: Option<String>,
    /// La portada, sacada a un fichero para que la cargue el mismo cargador
    /// de miniaturas que las fotos del chat.
    pub portada: Option<PathBuf>,
}

thread_local! {
    /// Las tarjetas ya leidas, por ruta del audio. Leer las etiquetas es ir
    /// al disco: una vez por cancion y no en cada fotograma.
    static TARJETAS: RefCell<HashMap<PathBuf, Rc<Tarjeta>>> = RefCell::new(HashMap::new());
}

/// La tarjeta de la cancion `m` de la conversacion abierta, o `None` si no
/// es musica. Sin el fichero en este equipo (aun sincronizando) sale vacia,
/// sin guardarla: cuando llegue se leera.
pub(super) fn tarjeta(a: &Abierto, m: &Mensaje) -> Option<Rc<Tarjeta>> {
    if !crate::biblioteca_audio::es_musica(m) {
        return None;
    }
    let Some(ruta) = super::ruta_del_mensaje(&a.raiz, &a.ficha.id, m) else {
        return Some(Rc::default());
    };
    if let Some(t) = TARJETAS.with_borrow(|c| c.get(&ruta).cloned()) {
        return Some(t);
    }
    if !ruta.is_file() {
        return Some(Rc::default());
    }
    let t = Rc::new(leer_tarjeta(&ruta));
    TARJETAS.with_borrow_mut(|c| c.insert(ruta, t.clone()));
    Some(t)
}

fn leer_tarjeta(ruta: &Path) -> Tarjeta {
    let e = pixpin_voz::etiquetas::leer(ruta);
    let portada = e.portada.as_ref().and_then(|p| guardar_portada(ruta, p));
    Tarjeta {
        titulo: e.titulo,
        artista: e.artista,
        album: e.album,
        portada,
    }
}

/// Saca la portada a la carpeta temporal, con un nombre que cambia si
/// cambia el audio (ruta, tamano y fecha). Es cache: borrarla no pierde
/// nada, se vuelve a sacar.
fn guardar_portada(audio: &Path, p: &pixpin_voz::etiquetas::Portada) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let meta = std::fs::metadata(audio).ok()?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    audio.hash(&mut h);
    meta.len().hash(&mut h);
    meta.modified().ok().hash(&mut h);
    let carpeta = std::env::temp_dir().join("pixpin-portadas");
    let destino = carpeta.join(format!("{:016x}.{}", h.finish(), p.extension()));
    if destino.is_file() {
        return Some(destino);
    }
    std::fs::create_dir_all(&carpeta).ok()?;
    match std::fs::write(&destino, &p.datos) {
        Ok(()) => Some(destino),
        Err(e) => {
            tracing::warn!(?e, "no se pudo sacar la portada");
            None
        }
    }
}

/// El nombre que lleva la tarjeta: el titulo de sus etiquetas o el del
/// fichero sin su extension.
pub(super) fn titulo(t: &Tarjeta, m: &Mensaje) -> String {
    t.titulo
        .clone()
        .unwrap_or_else(|| crate::biblioteca_audio::titulo_de_audio(m))
}

/// El renglon de debajo: el artista y lo que dura (`tiempo`, ya escrito).
pub(super) fn detalle(t: &Tarjeta, tiempo: &str) -> String {
    match (t.artista.as_deref(), tiempo.is_empty()) {
        (Some(a), true) => a.to_string(),
        (Some(a), false) => format!("{a} · {tiempo}"),
        (None, _) => tiempo.to_string(),
    }
}

/// La portada de la cancion de la burbuja `i`, si la trae: lo que hay que
/// cargar antes de pintar.
pub(super) fn portada(a: &Abierto, i: usize) -> Option<PathBuf> {
    tarjeta(a, a.mensajes.get(i)?)?.portada.clone()
}

/// La de la cancion cuya letra esta abierta.
pub(super) fn portada_de_la_letra(a: &Abierto) -> Option<PathBuf> {
    portada(a, super::indice_de_la_letra(a)?)
}

/// Los dos colores del degradado de una cancion sin portada: siempre los
/// mismos para la misma cancion, y distintos entre canciones, para que una
/// lista de ellas no sean todas iguales.
fn degradado_de(semilla: &str) -> (Color, Color) {
    const PARES: [(u32, u32); 5] = [
        (0x8e44b8, 0x0a6cd6),
        (0xc93c35, 0xc77700),
        (0x1e7a4a, 0x0a6cd6),
        (0x4b49c8, 0x9a3fc4),
        (0xc77700, 0x8e44b8),
    ];
    let n = semilla
        .bytes()
        .fold(0u32, |n, b| n.wrapping_mul(31).wrapping_add(u32::from(b)));
    let (a, b) = PARES[n as usize % PARES.len()];
    (hex(a), hex(b))
}

/// **La portada** en `caja`, con las esquinas redondeadas: la imagen
/// recortada al centro o, sin ella, un degradado con la nota encima.
pub(super) fn pintar_portada(
    p: &Pintor,
    caja: RectF,
    radio: f32,
    t: &Tarjeta,
    semilla: &str,
    previas: &crate::miniaturas::Miniaturas,
) {
    let recortada = p.empujar_recorte_redondeado(caja, radio);
    match t.portada.as_deref().and_then(|r| previas.ya(r)) {
        Some((b, w, h)) => crate::miniaturas::pintar_recortado(p, b, caja, w, h),
        None => {
            let (a, b) = degradado_de(semilla);
            p.rect_degradado(
                caja,
                (caja.x, caja.y),
                (caja.x + caja.ancho, caja.y + caja.alto),
                a,
                b,
            );
            let lado = caja.ancho.min(caja.alto) * 0.55;
            p.icono(
                &mi::MUSIC_NOTE,
                RectF {
                    x: caja.x + (caja.ancho - lado) / 2.0,
                    y: caja.y + (caja.alto - lado) / 2.0,
                    ancho: lado,
                    alto: lado,
                },
                Color::BLANCO,
            );
        }
    }
    if recortada {
        p.soltar_recorte_redondeado();
    }
}

// --- Adjuntar → «Musica» -------------------------------------------------------

/// La letra que acompana a un audio en su carpeta: `cancion.lrc` o
/// `cancion.txt` al lado de `cancion.mp3`. Un LRC se convierte al formato
/// de la letra; un texto llano va tal cual.
fn letra_al_lado(audio: &Path) -> Option<String> {
    ["lrc", "LRC", "txt"].iter().find_map(|ext| {
        let texto = std::fs::read_to_string(audio.with_extension(ext)).ok()?;
        let texto = texto.trim_start_matches('\u{feff}').replace("\r\n", "\n");
        let guardar =
            letra::convertir_si_es_lrc(&texto).unwrap_or_else(|| texto.trim().to_string());
        (!guardar.trim().is_empty()).then_some(guardar)
    })
}

/// **Mete las canciones elegidas** en la conversacion abierta, ya como
/// musica. Devuelve el aviso que hay que ensenar (nada si no se eligio
/// nada). Con una sola, se abre su karaoke, como el movil al guardarla.
pub(super) fn adjuntar(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    aparato: &str,
    rutas: &[PathBuf],
    textos: &Catalogo,
) -> Option<String> {
    if rutas.is_empty() {
        return None;
    }
    let carpeta = pixpin_proyecto::almacen::carpeta(ubicacion.raiz(), &a.ficha.id);
    let (mut metidas, mut con_letra) = (0usize, false);
    let mut aviso = None;
    for ruta in rutas {
        let nombre = ruta
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if pixpin_proyecto::cuaderno::clase_de_nombre(&nombre) != Clase::Voz {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("nombre", nombre.clone());
            aviso = Some(textos.t_args("musica-no-es-audio", &args));
            continue;
        }
        let bytes = match std::fs::read(ruta) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(?e, ruta = %ruta.display(), "no se pudo leer la cancion");
                aviso = Some(textos.t("chat-no-se-pudo"));
                continue;
            }
        };
        let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
        let mut m = match super::adjuntar_en_proyecto(
            ubicacion.raiz(),
            &a.ficha.id,
            aparato,
            &nombre,
            &bytes,
            numero,
        ) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(?e, "no se pudo adjuntar la cancion");
                aviso = Some(textos.t("chat-no-se-pudo"));
                continue;
            }
        };
        // Lo que la hace musica, en los campos que ya leen los dos.
        m.resto.insert(
            "estadoDelTexto".into(),
            serde_json::Value::String(crate::biblioteca_audio::TEXTO_LETRA.into()),
        );
        if let Some(t) = letra_al_lado(ruta) {
            m.transcripcion = Some(t);
            con_letra = true;
        }
        if let Err(e) = pixpin_proyecto::cuaderno::reemplazar(&carpeta, &m) {
            tracing::warn!(?e, "la cancion quedo como nota de voz sin marcar");
        }
        a.vistas.push(None);
        a.ficha.tocado = m.cuando;
        a.ficha.resumen = m.resumen();
        a.mensajes.push(m);
        metidas += 1;
    }
    a.colocado.borrow_mut().ancho = 0;
    a.scroll = None;
    match metidas {
        0 => aviso,
        1 => {
            let _ = super::abrir_la_letra(a, a.mensajes.len() - 1);
            aviso.or_else(|| (!con_letra).then(|| textos.t("musica-anadida")))
        }
        n => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("n", n as i64);
            aviso.or_else(|| Some(textos.t_args("musica-anadidas", &args)))
        }
    }
}

// --- El karaoke ------------------------------------------------------------------

/// Si se esta sincronizando la letra abierta: entonces las teclas son suyas.
pub(super) fn sincronizando(a: &Abierto) -> bool {
    a.letra.as_ref().is_some_and(|l| l.sincronizando.is_some())
}

/// Por donde va la cancion `m`, si es la que esta cargada.
fn posicion_de(a: &Abierto, m: &Mensaje) -> Option<i64> {
    let ruta = super::ruta_del_mensaje(&a.raiz, &a.ficha.id, m)?;
    (crate::audio::cargado().as_deref() == Some(ruta.as_path()))
        .then(|| crate::audio::estado().posicion_ms)
}

/// Lo que hace cada boton de la musica.
pub(super) fn pulsar(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    boton: Boton,
    textos: &Catalogo,
) -> Efecto {
    if let Boton::Karaoke(i) = boton {
        return super::abrir_la_letra(a, i);
    }
    let Some(i) = super::indice_de_la_letra(a) else {
        return Efecto::Nada;
    };
    match boton {
        Boton::Karaoke(_) => Efecto::Nada,
        Boton::Sincronizar => {
            if sincronizando(a) {
                return Efecto::Nada;
            }
            let guardada = a.mensajes[i].transcripcion.clone().unwrap_or_default();
            let s = letra::Sincronizador::de(&letra::leer(&guardada));
            if s.textos().is_empty() {
                return Efecto::Aviso(textos.t("musica-sinc-primero-letra"));
            }
            if let Some(l) = a.letra.as_mut() {
                l.sincronizando = Some(s);
            }
            // Desde el principio, sonando: lo primero que se marca es la
            // primera linea.
            if let Err(e) = super::saltar_en_el_audio(a, i, 0) {
                tracing::warn!(?e, "no se pudo arrancar la cancion para sincronizar");
                if let Some(l) = a.letra.as_mut() {
                    l.sincronizando = None;
                }
                return Efecto::Aviso(textos.t("chat-voz-no-es-audio"));
            }
            Efecto::Aviso(textos.t("musica-sinc-empieza"))
        }
        Boton::Marcar => {
            let ms = posicion_de(a, &a.mensajes[i]).unwrap_or(0);
            if let Some(s) = a.letra.as_mut().and_then(|l| l.sincronizando.as_mut()) {
                s.marcar(ms);
            }
            Efecto::Nada
        }
        Boton::Deshacer => {
            let volver = a
                .letra
                .as_mut()
                .and_then(|l| l.sincronizando.as_mut())
                .and_then(|s| s.deshacer());
            if let Some(ms) = volver
                && let Err(e) = super::saltar_en_el_audio(a, i, ms)
            {
                tracing::warn!(?e, "no se pudo volver atras en la cancion");
            }
            Efecto::Nada
        }
        Boton::Guardar => {
            let Some(s) = a.letra.as_ref().and_then(|l| l.sincronizando.as_ref()) else {
                return Efecto::Nada;
            };
            if s.siguiente() == 0 {
                return Efecto::Nada;
            }
            let texto = letra::a_guardar(&s.letra());
            if let Some(l) = a.letra.as_mut() {
                l.sincronizando = None;
                l.seguido = None;
            }
            let mut m = a.mensajes[i].clone();
            if !crate::voz::escribir_letra(&mut m, &texto) {
                return Efecto::Aviso(textos.t("musica-sinc-guardada"));
            }
            match super::reescribir_mensaje(ubicacion, a, i, m, textos) {
                Efecto::Aviso(t) => Efecto::Aviso(t),
                _ => Efecto::Aviso(textos.t("musica-sinc-guardada")),
            }
        }
        Boton::Cancelar => {
            if let Some(l) = a.letra.as_mut() {
                l.sincronizando = None;
            }
            Efecto::Nada
        }
    }
}

/// Las teclas mientras se sincroniza: Espacio marca, Retroceso (o Ctrl+Z)
/// deshace, Intro guarda y Escape lo deja. Las demas no hacen nada: van
/// cantando y una tecla suelta no puede ir a parar a otro sitio.
pub(super) fn tecla(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    vk: u32,
    ctrl: bool,
    textos: &Catalogo,
) -> Efecto {
    const VK_BACK: u32 = 0x08;
    const VK_RETURN: u32 = 0x0D;
    const VK_ESCAPE: u32 = 0x1B;
    const VK_SPACE: u32 = 0x20;
    const VK_Z: u32 = 0x5A;
    let boton = match vk {
        VK_SPACE => Boton::Marcar,
        VK_BACK => Boton::Deshacer,
        VK_Z if ctrl => Boton::Deshacer,
        VK_RETURN => Boton::Guardar,
        VK_ESCAPE => Boton::Cancelar,
        _ => return Efecto::Nada,
    };
    pulsar(ubicacion, a, boton, textos)
}

/// Ctrl+C y el boton de copiar de una cancion: su letra en LRC, para
/// llevarla a otra aplicacion de karaoke (`Copiar como LRC` del movil).
/// `None` si la letra abierta no es de una cancion.
pub(super) fn copiar_lrc(a: &Abierto, textos: &Catalogo) -> Option<Efecto> {
    let m = a.mensajes.get(super::indice_de_la_letra(a)?)?;
    if !crate::biblioteca_audio::es_musica(m) {
        return None;
    }
    let guardada = crate::voz::transcripcion_de(m)?;
    let leida = letra::leer(guardada);
    if !leida.con_tiempos() {
        return None; // sin tiempos, el texto tal cual (lo de siempre)
    }
    let t = tarjeta(a, m).unwrap_or_default();
    let titulo = titulo(&t, m);
    let lrc = letra::a_lrc(&leida, Some(&titulo), t.artista.as_deref());
    Some(match pixpin_codec::portapapeles::copiar_texto(&lrc) {
        Ok(()) => Efecto::Aviso(textos.t("musica-lrc-copiada")),
        Err(e) => {
            tracing::warn!(?e, "no se pudo copiar el LRC");
            Efecto::Aviso(textos.t("chat-no-se-pudo"))
        }
    })
}

/// Lo alto de la cabecera del karaoke (portada, titulo y artista).
const CABECERA: f32 = 88.0;
/// Lo alto del panel de sincronizar, abajo.
const PANEL: f32 = 124.0;

/// **El karaoke** en `caja` (el sitio de los parrafos de la letra): la
/// portada con su titulo arriba y la letra centrada, siguiendo a la
/// cancion. Sincronizando, la lista de lineas con su minuto y el panel de
/// marcar.
pub(super) fn pintar_karaoke(
    p: &Pintor,
    caja: Rect,
    c: &Pinta,
    a: &Abierto,
    i: usize,
    l: &LetraAbierta,
) {
    let (tema, escala) = (c.tema, c.escala);
    let e = escala as f32 / 100.0;
    let Some(m) = a.mensajes.get(i) else {
        return;
    };
    let t = tarjeta(a, m).unwrap_or_default();

    // La cabecera: la portada de 64 y, al lado, el titulo y el artista.
    let alto_cab = (CABECERA * e).min(caja.alto as f32);
    let lado = 64.0 * e;
    let x0 = caja.x as f32 + 20.0 * e;
    let portada = RectF {
        x: x0,
        y: caja.y as f32 + (alto_cab - lado) / 2.0,
        ancho: lado,
        alto: lado,
    };
    pintar_portada(p, portada, 12.0 * e, &t, &m.id, c.previas);
    let tx = portada.x + lado + 14.0 * e;
    let ancho_t = (caja.x as f32 + caja.ancho as f32 - tx - 16.0 * e).max(1.0);
    let rotulo = titulo(&t, m);
    let (tam_t, tam_a) = (17.0 * e, 13.0 * e);
    let (_, alto_t) = p.medir_texto("Ag", tam_t);
    let debajo: Vec<&str> = [t.artista.as_deref(), t.album.as_deref()]
        .into_iter()
        .flatten()
        .collect();
    let (_, alto_a) = p.medir_texto("Ag", tam_a);
    let alto_todo = alto_t + if debajo.is_empty() { 0.0 } else { alto_a };
    let ty = caja.y as f32 + (alto_cab - alto_todo) / 2.0;
    p.texto_linea(&rotulo, tx, ty, tam_t, ancho_t, tema.texto_papel);
    if !debajo.is_empty() {
        p.texto_linea(
            &debajo.join(" · "),
            tx,
            ty + alto_t,
            tam_a,
            ancho_t,
            con_alfa(tema.texto_papel, 0.6),
        );
    }
    let lista = Rect {
        y: caja.y + alto_cab as i32,
        alto: caja.alto.saturating_sub(alto_cab as u32),
        ..caja
    };
    match l.sincronizando.as_ref() {
        Some(s) => pintar_sincronizar(p, lista, c, a, s, l.tam),
        None => pintar_lineas(p, lista, c, a, i, m, l, alto_cab as u32),
    }
}

/// La letra que se canta, centrada: la linea que suena grande y en color a
/// un tercio de la altura, las cantadas al 40 % y las que vienen al 75 %.
#[allow(clippy::too_many_arguments)] // el sitio, el pintado, la nota y la letra
fn pintar_lineas(
    p: &Pintor,
    lista: Rect,
    c: &Pinta,
    a: &Abierto,
    i: usize,
    m: &Mensaje,
    l: &LetraAbierta,
    alto_cabecera: u32,
) {
    use pixpin_ui::chat as ui;
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    let trozos = super::trozos_de_la_letra(m);
    if trozos.is_empty() {
        // Sin letra se dice, y se dice como ponerla.
        let tam = 15.0 * e;
        for (n, (clave, color)) in [
            ("musica-sin-letra", tema.texto_papel),
            ("musica-escribir-letra", tema.apagado),
        ]
        .into_iter()
        .enumerate()
        {
            let texto = textos.t(clave);
            let (w, h) = p.medir_texto_ajustado(&texto, tam, lista.ancho as f32 - 40.0 * e);
            p.texto_ajustado(
                &texto,
                lista.x as f32 + (lista.ancho as f32 - w) / 2.0,
                lista.y as f32 + lista.alto as f32 / 3.0 + n as f32 * (h + 8.0 * e),
                tam,
                w + 1.0,
                color,
            );
        }
        return;
    }
    let con_tiempos = trozos.iter().any(|t| t.ms.is_some());
    let actual = posicion_de(a, m).and_then(|ms| crate::voz::por_donde_va(&trozos, ms));
    let tam = l.tam as f32 * e;
    let tam_activa = tam * 1.15;
    let ancho = ui::ancho_de_la_letra(lista, escala) as f32;
    let textos_de: Vec<String> = trozos
        .iter()
        .map(|t| {
            if t.texto.is_empty() && t.ms.is_some() {
                "♪".to_string()
            } else {
                t.texto.clone()
            }
        })
        .collect();
    let altos: Vec<u32> = textos_de
        .iter()
        .enumerate()
        .map(|(n, t)| {
            let tam = if actual == Some(n) { tam_activa } else { tam };
            p.medir_texto_ajustado(t, tam, ancho.max(1.0)).1.ceil() as u32
        })
        .collect();
    // Para la rueda (`tope_de_la_letra`), lo que mide todo con la cabecera:
    // sin ella, el final de una letra larga sin tiempos no se alcanzaria.
    {
        let mut guardados = altos.clone();
        guardados.push(alto_cabecera);
        *l.altos.borrow_mut() = guardados;
    }
    // Con tiempos la vista la lleva la cancion: la linea que suena (o la
    // primera, antes de empezar) a un tercio de la altura. Sin tiempos, la
    // rueda.
    let scroll = if con_tiempos {
        let foco = actual.unwrap_or(0);
        let cajas = ui::parrafos_de_la_letra(lista, &altos, 0, escala);
        cajas.get(foco).map_or(0, |k| {
            k.y - lista.y - lista.alto as i32 / 3 + k.alto as i32 / 2
        })
    } else {
        l.scroll
    };
    let cajas = ui::parrafos_de_la_letra(lista, &altos, scroll, escala);
    let tinta = tema.texto_papel;
    let aire = ui::LETRA_AIRE_Y as f32 * e;
    p.empujar_recorte(rf(lista));
    for (n, (texto, k)) in textos_de.iter().zip(&cajas).enumerate() {
        if k.abajo() < lista.y || k.y > lista.abajo() {
            continue;
        }
        let activa = actual == Some(n);
        let color = match actual {
            _ if !con_tiempos => tinta,
            None => tinta,
            Some(_) if activa => tema.enviar,
            Some(donde) if n < donde => con_alfa(tinta, 0.40),
            Some(_) => con_alfa(tinta, 0.75),
        };
        let tam = if activa { tam_activa } else { tam };
        let (w, _) = p.medir_texto_ajustado(texto, tam, ancho.max(1.0));
        p.texto_ajustado(
            texto,
            k.x as f32 + (k.ancho as f32 - w) / 2.0,
            k.y as f32 + aire,
            tam,
            w + 1.0,
            color,
        );
        if let Some(ms) = trozos[n].ms {
            a.zonas.borrow_mut().push((*k, Zona::Trozo(i, ms)));
        }
    }
    p.soltar_recorte();
}

/// Sincronizando: cada linea con el minuto que se le puso, la que toca
/// resaltada a un tercio de la altura, y abajo el panel de marcar.
fn pintar_sincronizar(
    p: &Pintor,
    lista: Rect,
    c: &Pinta,
    a: &Abierto,
    s: &letra::Sincronizador,
    tam_letra: u32,
) {
    use pixpin_ui::chat as ui;
    let (tema, escala, textos) = (c.tema, c.escala, c.textos);
    let e = escala as f32 / 100.0;
    let alto_panel = ((PANEL * e) as u32).min(lista.alto);
    let arriba = Rect {
        alto: lista.alto - alto_panel,
        ..lista
    };
    let panel = Rect {
        y: arriba.abajo(),
        alto: alto_panel,
        ..lista
    };
    // Un clic en cualquier sitio de la lista tambien marca: es donde estan
    // mirando mientras cantan. Los botones del panel van encima.
    a.zonas
        .borrow_mut()
        .push((arriba, Zona::Musica(Boton::Marcar)));

    let tam = tam_letra as f32 * e * 0.8;
    let tam_minuto = 12.0 * e;
    let columna = 56.0 * e;
    let margen = ui::LETRA_MARGEN_X as f32 * e;
    let ancho = (arriba.ancho as f32 - 2.0 * margen - columna).max(1.0);
    let altos: Vec<u32> = s
        .textos()
        .iter()
        .map(|t| p.medir_texto_ajustado(t, tam, ancho).1.ceil() as u32)
        .collect();
    let foco = s.siguiente().min(altos.len().saturating_sub(1));
    let scroll = ui::parrafos_de_la_letra(arriba, &altos, 0, escala)
        .get(foco)
        .map_or(0, |k| k.y - arriba.y - arriba.alto as i32 / 3);
    let cajas = ui::parrafos_de_la_letra(arriba, &altos, scroll, escala);
    let aire = ui::LETRA_AIRE_Y as f32 * e;
    p.empujar_recorte(rf(arriba));
    for (n, (texto, k)) in s.textos().iter().zip(&cajas).enumerate() {
        if k.abajo() < arriba.y || k.y > arriba.abajo() {
            continue;
        }
        if n == s.siguiente() {
            p.rellenar(rf(*k), con_alfa(tema.enviar, 0.12));
        }
        let x = k.x as f32 + margen;
        let y = k.y as f32 + aire;
        if let Some(ms) = s.tiempo(n) {
            p.texto(&letra::marca(ms), x, y + 2.0 * e, tam_minuto, tema.enviar);
        }
        let color = if s.tiempo(n).is_some() {
            con_alfa(tema.texto_papel, 0.55)
        } else {
            tema.texto_papel
        };
        p.texto_ajustado(texto, x + columna, y, tam, ancho + 1.0, color);
    }
    p.soltar_recorte();

    // El panel: que toca, las teclas, y los cuatro botones.
    p.rellenar(rf(panel), tema.cabecera);
    p.rellenar(
        RectF {
            x: panel.x as f32,
            y: panel.y as f32,
            ancho: panel.ancho as f32,
            alto: (1.0 * e).max(1.0),
        },
        tema.separador,
    );
    let px = panel.x as f32 + 16.0 * e;
    let pancho = (panel.ancho as f32 - 32.0 * e).max(1.0);
    let que = match s.textos().get(s.siguiente()) {
        Some(linea) => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("linea", linea.replace('\n', " "));
            textos.t_args("musica-sinc-siguiente", &args)
        }
        None => textos.t("musica-sinc-todas"),
    };
    let tam_que = 14.0 * e;
    let (_, alto_que) = p.medir_texto("Ag", tam_que);
    let mut y = panel.y as f32 + 10.0 * e;
    p.texto_linea(&que, px, y, tam_que, pancho, tema.texto);
    y += alto_que + 2.0 * e;
    let tam_pista = 11.5 * e;
    let (_, alto_pista) = p.medir_texto("Ag", tam_pista);
    p.texto_linea(
        &textos.t("musica-sinc-teclas"),
        px,
        y,
        tam_pista,
        pancho,
        tema.apagado,
    );
    y += alto_pista + 8.0 * e;

    let alto_b = (panel.abajo() as f32 - y - 10.0 * e).clamp(1.0, 48.0 * e);
    let deshacer = RectF {
        x: px,
        y,
        ancho: alto_b,
        alto: alto_b,
    };
    let (tam_b, hueco) = (14.0 * e, 8.0 * e);
    let rotulo_cancelar = textos.t("musica-sinc-cancelar");
    let rotulo_guardar = textos.t("musica-sinc-guardar");
    let w_cancelar = p.medir_texto(&rotulo_cancelar, tam_b).0 + 24.0 * e;
    let w_guardar = p.medir_texto(&rotulo_guardar, tam_b).0 + 28.0 * e;
    let guardar = RectF {
        x: px + pancho - w_guardar,
        y,
        ancho: w_guardar,
        alto: alto_b,
    };
    let cancelar = RectF {
        x: guardar.x - hueco - w_cancelar,
        ..guardar
    };
    let cancelar = RectF {
        ancho: w_cancelar,
        ..cancelar
    };
    let marcar = RectF {
        x: deshacer.x + deshacer.ancho + hueco,
        y,
        ancho: (cancelar.x - hueco - (deshacer.x + deshacer.ancho + hueco)).max(1.0),
        alto: alto_b,
    };
    let hay_marcas = s.siguiente() > 0;
    let a_rect = |r: RectF| Rect {
        x: r.x as i32,
        y: r.y as i32,
        ancho: r.ancho as u32,
        alto: r.alto as u32,
    };
    let mut z = a.zonas.borrow_mut();
    // Deshacer.
    p.rellenar_redondeado(deshacer, alto_b / 2.0, con_alfa(tema.texto, 0.08));
    icono_centrado(
        p,
        &mi::UNDO,
        a_rect(deshacer),
        20.0 * e,
        if hay_marcas { tema.texto } else { tema.apagado },
    );
    if hay_marcas {
        z.push((a_rect(deshacer), Zona::Musica(Boton::Deshacer)));
    }
    // Marcar n/N: el grande, como el boton del movil.
    let terminado = s.terminado();
    let mut args = fluent_bundle::FluentArgs::new();
    args.set(
        "n",
        (s.siguiente() + usize::from(!terminado)).min(s.textos().len()) as i64,
    );
    args.set("total", s.textos().len() as i64);
    let rotulo_marcar = textos.t_args("musica-sinc-marcar", &args);
    p.rellenar_redondeado(
        marcar,
        10.0 * e,
        if terminado {
            con_alfa(tema.enviar, 0.35)
        } else {
            tema.enviar
        },
    );
    let tam_m = 17.0 * e;
    let (wm, hm) = p.medir_texto(&rotulo_marcar, tam_m);
    p.texto(
        &rotulo_marcar,
        marcar.x + (marcar.ancho - wm) / 2.0,
        marcar.y + (marcar.alto - hm) / 2.0,
        tam_m,
        Color::BLANCO,
    );
    if !terminado {
        z.push((a_rect(marcar), Zona::Musica(Boton::Marcar)));
    }
    // Cancelar y Guardar.
    for (r, rotulo, boton, activo) in [
        (cancelar, &rotulo_cancelar, Boton::Cancelar, true),
        (guardar, &rotulo_guardar, Boton::Guardar, hay_marcas),
    ] {
        if boton == Boton::Guardar {
            p.trazar(
                r,
                (1.2 * e).max(1.0),
                con_alfa(tema.enviar, if activo { 1.0 } else { 0.35 }),
            );
        }
        let (w, h) = p.medir_texto(rotulo, tam_b);
        p.texto(
            rotulo,
            r.x + (r.ancho - w) / 2.0,
            r.y + (r.alto - h) / 2.0,
            tam_b,
            if activo { tema.enviar } else { tema.apagado },
        );
        if activo {
            z.push((a_rect(r), Zona::Musica(boton)));
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_detalle_lleva_el_artista_y_lo_que_dura() {
        let t = Tarjeta {
            artista: Some("Consuelo".into()),
            ..Tarjeta::default()
        };
        assert_eq!(detalle(&t, "3:07"), "Consuelo · 3:07");
        assert_eq!(detalle(&t, ""), "Consuelo");
        assert_eq!(detalle(&Tarjeta::default(), "3:07"), "3:07");
    }

    #[test]
    fn el_degradado_de_una_cancion_es_siempre_el_mismo() {
        assert_eq!(degradado_de("abc"), degradado_de("abc"));
    }

    #[test]
    fn la_letra_de_al_lado_se_lee_y_un_lrc_se_convierte() {
        let dir = std::env::temp_dir().join(format!("pixpin-musica-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let audio = dir.join("cancion.mp3");
        std::fs::write(
            dir.join("cancion.lrc"),
            "\u{feff}[ti:X]\r\n[00:01.60]Hola\r\n",
        )
        .unwrap();
        assert_eq!(letra_al_lado(&audio).as_deref(), Some("[0:02] Hola"));
        let otro = dir.join("otra.mp3");
        std::fs::write(dir.join("otra.txt"), "  la la\nla  \n").unwrap();
        assert_eq!(letra_al_lado(&otro).as_deref(), Some("la la\nla"));
        assert_eq!(letra_al_lado(&dir.join("nada.mp3")), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
