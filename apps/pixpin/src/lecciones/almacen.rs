//! **Donde viven las lecciones en el PC** (`LeccionesStore.kt`): un archivo
//! por leccion y un mensaje del chat que lo senala.
//!
//! Igual que en el movil, para que viajen con la sincronizacion **sin tocar
//! el protocolo**:
//!
//! - El archivo es `guardados/lecciones/<id>.leccion` visto desde el movil.
//!   Aqui, como todo lo que llega del movil a un chat, vive en
//!   `proyectos/<chat>/android/guardados/lecciones/<id>.leccion`, y el
//!   mensaje lo nombra con la ruta portatil `pixpin:files/guardados/
//!   lecciones/<id>.leccion`. Una leccion hecha aqui se guarda en el MISMO
//!   sitio y con la MISMA ruta que una hecha en el telefono: asi, cuando el
//!   telefono la edita, encuentra su archivo (`LeccionesStore.guardar` busca
//!   el mensaje por esa ruta) y no crea otro.
//! - El mensaje es de clase `ARCHIVO`, con id `lec-<id>`, de nombre
//!   «💡 titulo» y con el resumen legible en su texto
//!   ([`pixpin_lecciones::Leccion::resumen`], identico al de Kotlin).
//! - Se escribe con las funciones del chat (`cuaderno::anadir`,
//!   `cuaderno::cambiar`, el cerrojo `cuaderno::cerrojo`) y el sello de este
//!   equipo, y se borra como borra el chat: la marca de borrado
//!   (`vista::anotar_borrados`) para que no vuelva del otro aparato, y el
//!   archivo fuera.
//!
//! El archivo es la verdad; el mensaje, su escaparate. Las lecciones no se
//! ensenan en el historial del chat (el movil, 4-oct: «no se mezclan con el
//! chat»), ver [`es_de_leccion`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use pixpin_lecciones::Leccion;
use pixpin_lecciones::leccion::{EXTENSION, PREFIJO};
use pixpin_proyecto::cuaderno::{self, Clase, Mensaje};
use pixpin_proyecto::{almacen, vista};

/// Donde guarda el movil sus lecciones, relativo a su carpeta `files`.
pub const CARPETA: &str = "guardados/lecciones";
const PORTATIL: &str = "pixpin:files/";

/// Una leccion con el mensaje que la lleva y el chat donde esta.
#[derive(Debug, Clone)]
pub struct Entrada {
    pub leccion: Leccion,
    pub mensaje: Mensaje,
    /// La ficha (carpeta) del chat: `proyectos/<ficha>`.
    pub ficha: String,
    pub archivo: PathBuf,
}

/// Si un mensaje lleva una leccion (`LeccionesStore.esLeccion`).
pub fn es_leccion(m: &Mensaje) -> bool {
    m.clase == Some(Clase::Archivo)
        && m.ruta
            .as_deref()
            .is_some_and(pixpin_lecciones::leccion::es_ruta_de_leccion)
}

/// Si un mensaje es una leccion o una foto o audio suyo (los que la
/// responden): lo que el chat no ensena (`LeccionesStore.sinLecciones`). Las
/// suyas se reconocen por el id de lo que responden, que empieza por `lec-`.
pub fn es_de_leccion(m: &Mensaje) -> bool {
    es_leccion(m)
        || m.responde_a
            .as_deref()
            .is_some_and(|r| r.starts_with(PREFIJO))
}

/// La ruta con que el mensaje nombra la leccion `id`.
pub fn ruta_portatil(id: &str) -> String {
    format!("{PORTATIL}{CARPETA}/{id}{EXTENSION}")
}

/// Donde esta en este equipo lo que nombra la `ruta` de un mensaje de
/// leccion. Lo normal es la ruta portatil (en `android/` del chat); una
/// relativa es de la carpeta del chat; una absoluta del movil (de un
/// `.pixpin` viejo) se busca por lo que va detras de `files/`.
pub fn archivo_de(raiz: &Path, ficha: &str, ruta: &str) -> Option<PathBuf> {
    let carpeta = almacen::carpeta(raiz, ficha);
    let rel = if let Some(rel) = ruta.strip_prefix(PORTATIL) {
        rel.to_string()
    } else if let Some((_, rel)) = ruta.split_once("/files/") {
        rel.to_string()
    } else {
        return vista::ruta_real(raiz, ficha, ruta);
    };
    // Lo nacido en otro chat de este equipo o un dibujo: que lo resuelva la
    // vista de sincronizar, que sabe de esos casos.
    if rel.starts_with("guardados/pc/") || rel.starts_with("pins/") {
        return vista::ruta_real(raiz, ficha, &format!("{PORTATIL}{rel}"));
    }
    let mut p = carpeta.join("android");
    for trozo in rel.split('/') {
        match trozo {
            "" | "." => {}
            ".." => return None,
            t => p.push(t),
        }
    }
    Some(p)
}

/// Lo ya leido, por archivo y fecha: abrir la lista no vuelve a leer las
/// lecciones que no cambiaron.
fn cache() -> &'static Mutex<HashMap<PathBuf, (SystemTime, Leccion)>> {
    static C: std::sync::OnceLock<Mutex<HashMap<PathBuf, (SystemTime, Leccion)>>> =
        std::sync::OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

fn leer_archivo(archivo: &Path) -> Option<Leccion> {
    let cuando = std::fs::metadata(archivo).and_then(|m| m.modified()).ok()?;
    let mut c = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some((t, l)) = c.get(archivo)
        && *t == cuando
    {
        return Some(l.clone());
    }
    let l = Leccion::leer(&std::fs::read_to_string(archivo).ok()?)?;
    c.insert(archivo.to_path_buf(), (cuando, l.clone()));
    Some(l)
}

/// **Todas las lecciones**, de todos los chats, de la mas tocada a la
/// menos. Una cuyo archivo aun no llego (el mensaje viajo y el archivo no)
/// o no se lee, no sale.
pub fn listar(raiz: &Path) -> Vec<Entrada> {
    let indice = almacen::Indice::leer(raiz);
    let mut salida = Vec::new();
    for f in &indice.proyectos {
        let Ok(c) = cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, &f.id)) else {
            continue;
        };
        for m in c.mensajes.into_iter().filter(es_leccion) {
            let Some(archivo) = m.ruta.as_deref().and_then(|r| archivo_de(raiz, &f.id, r)) else {
                continue;
            };
            let Some(leccion) = leer_archivo(&archivo) else {
                continue;
            };
            // Su mensaje dos veces (dos guardados a la vez en el movil,
            // v0.103.1): la leccion sale una sola vez.
            if salida.iter().any(|e: &Entrada| e.leccion.id == leccion.id) {
                continue;
            }
            salida.push(Entrada {
                leccion,
                mensaje: m,
                ficha: f.id.clone(),
                archivo,
            });
        }
    }
    salida.sort_by_key(|x| std::cmp::Reverse(x.leccion.tocada));
    salida
}

/// Lo que dice si algo cambio desde la ultima vez: la fecha de cada
/// cuaderno y la de cada leccion. Mirarlo cuesta unas pocas lecturas de
/// fecha, no leer nada.
pub fn firma(raiz: &Path, entradas: &[Entrada]) -> Vec<Option<SystemTime>> {
    let fecha = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let mut v: Vec<Option<SystemTime>> = almacen::Indice::leer(raiz)
        .proyectos
        .iter()
        .map(|f| fecha(&almacen::carpeta(raiz, &f.id).join("guardados.jsonl")))
        .collect();
    v.extend(entradas.iter().map(|e| fecha(&e.archivo)));
    v
}

/// Donde se guarda una leccion.
#[derive(Debug, Clone)]
pub enum Donde {
    /// Una nueva, en el chat de esa ficha.
    Nueva { ficha: String },
    /// Una que ya existe: su archivo y su mensaje se ponen al dia.
    Existente {
        ficha: String,
        mensaje: String,
        archivo: PathBuf,
    },
}

impl Donde {
    pub fn de(e: &Entrada) -> Donde {
        Donde::Existente {
            ficha: e.ficha.clone(),
            mensaje: e.mensaje.id.clone(),
            archivo: e.archivo.clone(),
        }
    }
}

/// Escribe el archivo entero al lado y lo pone en su sitio de un tiron.
fn escribir_archivo(archivo: &Path, l: &Leccion) -> std::io::Result<u64> {
    if let Some(p) = archivo.parent() {
        std::fs::create_dir_all(p)?;
    }
    let texto = l.escribir();
    let tmp = archivo.with_extension("leccion.tmp");
    std::fs::write(&tmp, &texto)?;
    std::fs::rename(&tmp, archivo)?;
    Ok(texto.len() as u64)
}

/// **Guarda una leccion** (`LeccionesStore.guardar`): la primera vez crea
/// su mensaje en el chat de la ficha; las siguientes reescriben el archivo y
/// ponen al dia el resumen del mensaje. `aparato` es el codigo de este
/// equipo, el del sello.
pub fn guardar(raiz: &Path, l: &Leccion, donde: &Donde, aparato: &str) -> std::io::Result<Leccion> {
    let (ficha, archivo, mensaje) = match donde {
        Donde::Nueva { ficha } => (ficha.clone(), archivo_nuevo(raiz, ficha, &l.id), None),
        Donde::Existente {
            ficha,
            mensaje,
            archivo,
        } => (ficha.clone(), archivo.clone(), Some(mensaje.clone())),
    };
    let bytes = escribir_archivo(&archivo, l)? as i64;
    let carpeta = almacen::carpeta(raiz, &ficha);
    let puesto = match &mensaje {
        Some(id) => cuaderno::cambiar(&carpeta, id, |m| {
            m.texto = l.resumen();
            m.nombre = l.nombre();
            m.bytes = bytes;
            true
        })?
        .is_some(),
        None => false,
    };
    if !puesto {
        // Nueva, o su mensaje se borro mientras tanto: uno nuevo.
        let numero = crate::ventana_chat::siguiente_numero_en(raiz, &ficha)?;
        let ahora = pixpin_shell::entorno::ahora_utc_ms();
        let sello = cuaderno::Sello {
            cuando: ahora,
            numero,
            aparato: aparato.to_string(),
            proyecto: ficha.clone(),
        };
        let ruta = match &mensaje {
            // El de antes se fue, pero el archivo es el mismo.
            Some(_) => {
                vista::portatil_de_ruta(raiz, &archivo).unwrap_or_else(|| ruta_portatil(&l.id))
            }
            None => ruta_portatil(&l.id),
        };
        let mut m = Mensaje::adjunto(Clase::Archivo, &l.nombre(), &ruta, bytes, &sello);
        m.id = format!("{PREFIJO}{}", l.id);
        m.cuando = l.creada;
        m.texto = l.resumen();
        cuaderno::anadir(&carpeta, &m)?;
        crate::ventana_chat::subir_en_la_lista(raiz, &ficha, ahora, &m.nombre)?;
    }
    crate::ventana_chat::refrescar();
    avisar_cambio();
    Ok(l.clone())
}

/// Una foto que va con una leccion: el nombre de su fichero y sus bytes.
pub type Foto = (String, Vec<u8>);

/// Como se llama en el chat el mensaje de una foto de leccion (el del movil,
/// `AdjuntosDeLaLeccion.kt`).
pub const NOMBRE_FOTO: &str = "Foto de la lección";

/// **Guarda una leccion con fotos nuevas** (`LeccionesStore.guardar` con
/// `nuevos`): la leccion primero, para que su mensaje exista; despues cada
/// foto en `archivos/` del chat con su mensaje de clase `IMAGEN` que
/// **responde** al de la leccion (asi el chat no la ensena y el movil la
/// reconoce como suya), y por ultimo la leccion otra vez con los ids de esos
/// mensajes en `adjuntos`. Sin fotos es [`guardar`].
// Hoy solo la usan las pruebas: la ficha guarda con `guardar_con_cambios`.
#[cfg(test)]
pub fn guardar_con_fotos(
    raiz: &Path,
    l: &Leccion,
    donde: &Donde,
    aparato: &str,
    fotos: &[Foto],
) -> std::io::Result<Leccion> {
    guardar_con_adjuntos(raiz, l, donde, aparato, fotos, None)
}

/// Una nota de voz que va con una leccion: el nombre de su fichero, sus
/// bytes y lo que dura.
pub type Voz = (String, Vec<u8>, i64);

/// Como se llama en el chat el mensaje de una nota de voz de leccion.
pub const NOMBRE_VOZ: &str = "Nota de voz de la lección";

/// Como [`guardar_con_fotos`], y ademas una nota de voz (5-oct-2026: el
/// timeline hace lecciones de sus momentos con un boton, y un momento
/// dictado lleva su audio). La voz va como las fotos: un mensaje de clase
/// `VOZ` que responde al de la leccion, con su duracion.
pub fn guardar_con_adjuntos(
    raiz: &Path,
    l: &Leccion,
    donde: &Donde,
    aparato: &str,
    fotos: &[Foto],
    voz: Option<&Voz>,
) -> std::io::Result<Leccion> {
    let voces: Vec<(&Voz, &str)> = voz.map(|v| (v, NOMBRE_VOZ)).into_iter().collect();
    guardar_con_todo(raiz, l, donde, aparato, fotos, &voces)
}

/// Como se llama en el chat el mensaje de un audio grabado en la ficha
/// (`AdjuntosDeLaLeccion.kt`: «Audio de la lección»).
pub const NOMBRE_AUDIO: &str = "Audio de la lección";

/// Fotos y voces (cada una con su rotulo) que responden a la leccion.
// `numero` sale de `?` antes del bucle; el contador a mano se lee mejor.
#[allow(clippy::explicit_counter_loop)]
fn guardar_con_todo(
    raiz: &Path,
    l: &Leccion,
    donde: &Donde,
    aparato: &str,
    fotos: &[Foto],
    voces: &[(&Voz, &str)],
) -> std::io::Result<Leccion> {
    if fotos.is_empty() && voces.is_empty() {
        return guardar(raiz, l, donde, aparato);
    }
    let (ficha, mensaje, archivo) = match donde {
        Donde::Nueva { ficha } => (
            ficha.clone(),
            format!("{PREFIJO}{}", l.id),
            archivo_nuevo(raiz, ficha, &l.id),
        ),
        Donde::Existente {
            ficha,
            mensaje,
            archivo,
        } => (ficha.clone(), mensaje.clone(), archivo.clone()),
    };
    guardar(raiz, l, donde, aparato)?;
    let carpeta = almacen::carpeta(raiz, &ficha);
    let mut numero = crate::ventana_chat::siguiente_numero_en(raiz, &ficha)?;
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let mut ids = Vec::with_capacity(fotos.len());
    let todos = fotos
        .iter()
        .map(|(n, b)| (n, b, Clase::Imagen, NOMBRE_FOTO, 0))
        .chain(
            voces
                .iter()
                .map(|((n, b, d), rotulo)| (n, b, Clase::Voz, *rotulo, *d)),
        );
    for (i, (nombre, bytes, clase, rotulo, duracion)) in todos.enumerate() {
        let ruta = almacen::guardar_adjunto(raiz, &ficha, nombre, bytes)?;
        // Un milisegundo cada una: el id del mensaje sale de la hora, y dos
        // fotos no pueden compartirlo.
        let sello = cuaderno::Sello {
            cuando: ahora + i as i64,
            numero,
            aparato: aparato.to_string(),
            proyecto: ficha.clone(),
        };
        let mut m = Mensaje::adjunto(clase, rotulo, &ruta, bytes.len() as i64, &sello);
        m.duracion_ms = duracion;
        m.responde_a = Some(mensaje.clone());
        cuaderno::anadir(&carpeta, &m)?;
        ids.push(m.id);
        numero += 1;
    }
    let mut adjuntos = l.adjuntos.clone();
    adjuntos.extend(ids);
    let con_fotos = Leccion {
        adjuntos,
        ..l.clone()
    };
    guardar(
        raiz,
        &con_fotos,
        &Donde::Existente {
            ficha,
            mensaje,
            archivo,
        },
        aparato,
    )
}

/// **Guarda una leccion con sus cambios de fotos y audios**
/// (`LeccionesStore.guardar` con `nuevos` y `quitados`, v0.102): lo nuevo
/// entra en el chat **respondiendo** a la leccion, lo quitado sale de su
/// lista y del chat con su marca de borrado (asi el movil tambien lo quita
/// y no vuelve) y su archivo se va. Vale para una nueva y para una vieja.
pub fn guardar_con_cambios(
    raiz: &Path,
    l: &Leccion,
    donde: &Donde,
    aparato: &str,
    fotos: &[Foto],
    audios: &[Voz],
    quitados: &[String],
) -> std::io::Result<Leccion> {
    let sin: Leccion = Leccion {
        adjuntos: l
            .adjuntos
            .iter()
            .filter(|a| !quitados.contains(a))
            .cloned()
            .collect(),
        ..l.clone()
    };
    let voces: Vec<(&Voz, &str)> = audios.iter().map(|v| (v, NOMBRE_AUDIO)).collect();
    let hecha = guardar_con_todo(raiz, &sin, donde, aparato, fotos, &voces)?;
    if let Donde::Existente { ficha, .. } = donde
        && !quitados.is_empty()
    {
        quitar_adjuntos(raiz, ficha, quitados)?;
    }
    Ok(hecha)
}

/// **Las fotos y audios de una leccion** (`LeccionesStore.adjuntosDe`): los
/// de su lista, en su orden, y **tambien los que le responden** (si dos
/// aparatos le pusieron una foto cada uno, la lista del archivo trae la de
/// uno, pero las dos le responden).
pub fn adjuntos_de(raiz: &Path, e: &Entrada) -> Vec<Mensaje> {
    let Ok(c) = cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, &e.ficha)) else {
        return Vec::new();
    };
    let mut v: Vec<Mensaje> = e
        .leccion
        .adjuntos
        .iter()
        .filter_map(|id| c.mensajes.iter().find(|m| m.id == *id).cloned())
        .collect();
    for m in &c.mensajes {
        if m.responde_a.as_deref() == Some(e.mensaje.id.as_str())
            && es_adjunto(m)
            && !v.iter().any(|x| x.id == m.id)
        {
            v.push(m.clone());
        }
    }
    v
}

/// Una foto o un audio (no del buzon): lo que puede ir con una leccion.
fn es_adjunto(m: &Mensaje) -> bool {
    matches!(m.clase, Some(Clase::Imagen) | Some(Clase::Voz)) && !m.en_buzon
}

/// Quita del chat `ficha` los mensajes `ids` con su marca de borrado (como
/// borra el chat) y sus archivos del disco.
pub fn quitar_adjuntos(raiz: &Path, ficha: &str, ids: &[String]) -> std::io::Result<()> {
    let ids: std::collections::BTreeSet<String> = ids.iter().cloned().collect();
    let quitados = cuaderno::quitar(&almacen::carpeta(raiz, ficha), &ids)?;
    vista::anotar_borrados(
        raiz,
        ficha,
        &quitados,
        pixpin_shell::entorno::ahora_utc_ms(),
    )?;
    for m in &quitados {
        if let Some(r) = m
            .ruta
            .as_deref()
            .and_then(|r| vista::ruta_real(raiz, ficha, r))
            && r.is_file()
        {
            let _ = std::fs::remove_file(r);
        }
    }
    Ok(())
}

/// Donde se escribe el archivo de una leccion nueva del chat `ficha`.
fn archivo_nuevo(raiz: &Path, ficha: &str, id: &str) -> PathBuf {
    almacen::carpeta(raiz, ficha)
        .join("android")
        .join(CARPETA)
        .join(format!("{id}{EXTENSION}"))
}

/// **Borra la leccion** (`LeccionesStore.borrar`): su mensaje y los de sus
/// fotos y audios salen del cuaderno con su marca de borrado, como borra el
/// chat, y sus archivos se van.
pub fn borrar(raiz: &Path, e: &Entrada) -> std::io::Result<()> {
    let carpeta = almacen::carpeta(raiz, &e.ficha);
    let mut ids: std::collections::BTreeSet<String> = e.leccion.adjuntos.iter().cloned().collect();
    ids.insert(e.mensaje.id.clone());
    let quitados = {
        let _cerrojo = cuaderno::cerrojo();
        let fichero = carpeta.join("guardados.jsonl");
        let texto = std::fs::read_to_string(&fichero)?;
        let mut salida = String::with_capacity(texto.len());
        let mut quitados = Vec::new();
        for linea in texto.lines() {
            match serde_json::from_str::<Mensaje>(linea) {
                Ok(m) if ids.contains(&m.id) => quitados.push(m),
                // Lo que no se entiende se copia tal cual, como el chat.
                _ => {
                    salida.push_str(linea);
                    salida.push('\n');
                }
            }
        }
        if !quitados.is_empty() {
            let temporal = fichero.with_extension("jsonl.tmp");
            std::fs::write(&temporal, salida)?;
            std::fs::rename(&temporal, &fichero)?;
        }
        quitados
    };
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    if let Err(err) = vista::anotar_borrados(raiz, &e.ficha, &quitados, ahora) {
        tracing::warn!(
            ?err,
            "no se pudo apuntar la leccion borrada para sincronizar"
        );
    }
    for m in quitados.iter().filter(|m| m.id != e.mensaje.id) {
        if let Some(r) = m
            .ruta
            .as_deref()
            .and_then(|r| vista::ruta_real(raiz, &e.ficha, r))
            && r.is_file()
        {
            let _ = std::fs::remove_file(r);
        }
    }
    if e.archivo.is_file() {
        std::fs::remove_file(&e.archivo)?;
    }
    crate::ventana_chat::refrescar();
    avisar_cambio();
    Ok(())
}

/// Las ventanas de lecciones abiertas, para que se enteren al momento de lo
/// que se guarda o borra en otra (la ficha guarda, la lista se pone al dia).
static CAMBIOS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static ABIERTAS: Mutex<Vec<isize>> = Mutex::new(Vec::new());

pub fn cambios() -> u64 {
    CAMBIOS.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn avisar_cambio() {
    CAMBIOS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    for h in ABIERTAS.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        pixpin_shell::overlay::despertar(*h);
    }
}

pub fn apuntar_ventana(hwnd: isize) {
    ABIERTAS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(hwnd);
}

pub fn quitar_ventana(hwnd: isize) {
    ABIERTAS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|h| *h != hwnd);
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn raiz(nombre: &str) -> PathBuf {
        let r =
            std::env::temp_dir().join(format!("pixpin-lecciones-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    #[test]
    fn la_ruta_es_la_del_movil_y_vive_donde_lo_que_llega_del_movil() {
        assert_eq!(
            ruta_portatil("abc"),
            "pixpin:files/guardados/lecciones/abc.leccion"
        );
        let r = Path::new("C:/datos");
        let a = archivo_de(r, "p1", "pixpin:files/guardados/lecciones/abc.leccion").unwrap();
        assert!(a.ends_with("proyectos/p1/android/guardados/lecciones/abc.leccion"));
        // La absoluta de un movil viejo se busca por lo de detras de files/.
        let b = archivo_de(
            r,
            "p1",
            "/data/user/0/com.forge.pixpin/files/guardados/lecciones/abc.leccion",
        )
        .unwrap();
        assert_eq!(a, b);
        // Caso negativo: no se sale de la carpeta.
        assert!(archivo_de(r, "p1", "pixpin:files/guardados/../../x.leccion").is_none());
    }

    #[test]
    fn el_chat_no_ensena_ni_la_leccion_ni_lo_que_la_responde() {
        let mut l = Mensaje {
            id: "lec-1".into(),
            clase: Some(Clase::Archivo),
            ruta: Some(ruta_portatil("1")),
            ..Default::default()
        };
        assert!(es_leccion(&l) && es_de_leccion(&l));
        let foto = Mensaje {
            clase: Some(Clase::Imagen),
            responde_a: Some("lec-1".into()),
            ..Default::default()
        };
        assert!(es_de_leccion(&foto));
        l.ruta = Some("archivos/informe.pdf".into());
        assert!(!es_de_leccion(&l), "un archivo cualquiera se ve");
    }

    #[test]
    fn guardar_crea_el_mensaje_y_volver_a_guardar_lo_pone_al_dia_y_borrar_lo_quita() {
        let r = raiz("guardar");
        let mut indice = almacen::Indice::default();
        let ficha = almacen::Ficha::nueva("Obra", 1, "K7Q2");
        indice.proyectos.push(ficha.clone());
        indice.guardar(&r).unwrap();
        let l = Leccion::nueva("k1", 1000, "Revisar puntales");
        guardar(
            &r,
            &l,
            &Donde::Nueva {
                ficha: ficha.id.clone(),
            },
            "K7Q2",
        )
        .unwrap();
        let todas = listar(&r);
        assert_eq!(todas.len(), 1);
        let e = &todas[0];
        assert_eq!(e.mensaje.id, "lec-k1");
        assert_eq!(e.mensaje.nombre, "💡 Revisar puntales");
        assert_eq!(e.mensaje.texto, "💡 Lección: Revisar puntales");
        assert_eq!(
            e.mensaje.ruta.as_deref(),
            Some("pixpin:files/guardados/lecciones/k1.leccion")
        );
        assert_eq!(e.mensaje.cuando, 1000);
        // Editarla no crea otro mensaje.
        let cambiada = Leccion {
            titulo: "Revisar puntales siempre".into(),
            ..l
        };
        guardar(&r, &cambiada, &Donde::de(e), "K7Q2").unwrap();
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&r, &ficha.id)).unwrap();
        assert_eq!(c.mensajes.len(), 1);
        assert_eq!(c.mensajes[0].texto, "💡 Lección: Revisar puntales siempre");
        // Borrar: fuera el mensaje y el archivo.
        let e = listar(&r).remove(0);
        borrar(&r, &e).unwrap();
        assert!(listar(&r).is_empty());
        assert!(!e.archivo.exists());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn las_fotos_van_en_archivos_con_su_mensaje_que_responde_a_la_leccion() {
        let r = raiz("fotos");
        let mut indice = almacen::Indice::default();
        let ficha = almacen::Ficha::nueva("Obra", 1, "K7Q2");
        indice.proyectos.push(ficha.clone());
        indice.guardar(&r).unwrap();
        let l = Leccion::nueva("f1", 1000, "Mirar el encofrado");
        let fotos = vec![
            ("a.png".to_string(), b"uno".to_vec()),
            ("b.jpg".to_string(), b"dos".to_vec()),
        ];
        let guardada = guardar_con_fotos(
            &r,
            &l,
            &Donde::Nueva {
                ficha: ficha.id.clone(),
            },
            "K7Q2",
            &fotos,
        )
        .unwrap();
        assert_eq!(guardada.adjuntos.len(), 2);
        let todas = listar(&r);
        assert_eq!(todas.len(), 1, "una sola leccion, no una por guardado");
        assert_eq!(todas[0].leccion.adjuntos, guardada.adjuntos);
        assert!(todas[0].mensaje.texto.contains("📎 2 adjuntos"));
        let carpeta = almacen::carpeta(&r, &ficha.id);
        let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
        let fotos_del_chat: Vec<&Mensaje> = c
            .mensajes
            .iter()
            .filter(|m| m.clase == Some(Clase::Imagen))
            .collect();
        assert_eq!(fotos_del_chat.len(), 2);
        for (m, id) in fotos_del_chat.iter().zip(&guardada.adjuntos) {
            assert_eq!(&m.id, id);
            assert_eq!(m.responde_a.as_deref(), Some("lec-f1"));
            assert_eq!(m.nombre, NOMBRE_FOTO);
            assert!(es_de_leccion(m), "el chat no la ensena");
            assert!(carpeta.join(m.ruta.as_deref().unwrap()).is_file());
        }
        assert_ne!(fotos_del_chat[0].id, fotos_del_chat[1].id);
        // Caso negativo: sin fotos no se anade ningun mensaje de foto.
        let otra = Leccion::nueva("f2", 2000, "Sin fotos");
        let g = guardar_con_fotos(
            &r,
            &otra,
            &Donde::Nueva {
                ficha: ficha.id.clone(),
            },
            "K7Q2",
            &[],
        )
        .unwrap();
        assert!(g.adjuntos.is_empty());
        let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
        assert_eq!(
            c.mensajes
                .iter()
                .filter(|m| m.clase == Some(Clase::Imagen))
                .count(),
            2
        );
        let _ = std::fs::remove_dir_all(&r);
    }
    /// **Fotos y audios en una leccion ya guardada** (v0.102): lo nuevo le
    /// responde, lo quitado sale del chat y del disco, y la lista de la
    /// leccion queda con lo que de verdad lleva.
    #[test]
    fn en_una_vieja_se_anade_y_se_quita_y_la_lista_queda_al_dia() {
        let r = raiz("cambios");
        let mut indice = almacen::Indice::default();
        let ficha = almacen::Ficha::nueva("Obra", 1, "K7Q2");
        indice.proyectos.push(ficha.clone());
        indice.guardar(&r).unwrap();
        let l = Leccion::nueva("c1", 1000, "Curar el concreto");
        let fotos = vec![("vieja.jpg".to_string(), b"vieja".to_vec())];
        guardar_con_fotos(
            &r,
            &l,
            &Donde::Nueva {
                ficha: ficha.id.clone(),
            },
            "K7Q2",
            &fotos,
        )
        .unwrap();
        let e = listar(&r).remove(0);
        let viejos = adjuntos_de(&r, &e);
        assert_eq!(viejos.len(), 1);
        let vieja = viejos[0].clone();
        let carpeta = almacen::carpeta(&r, &ficha.id);
        let fichero_viejo = carpeta.join(vieja.ruta.as_deref().unwrap());
        assert!(fichero_viejo.is_file());

        // En la ficha: una foto y un audio nuevos, y la vieja fuera.
        let nuevas = vec![("nueva.png".to_string(), b"nueva".to_vec())];
        let audios = vec![("voz_1.m4a".to_string(), b"m4a".to_vec(), 4_200)];
        let g = guardar_con_cambios(
            &r,
            &e.leccion,
            &Donde::de(&e),
            "K7Q2",
            &nuevas,
            &audios,
            std::slice::from_ref(&vieja.id),
        )
        .unwrap();
        assert_eq!(g.adjuntos.len(), 2);
        assert!(!g.adjuntos.contains(&vieja.id));
        let e = listar(&r).remove(0);
        assert_eq!(e.leccion.adjuntos, g.adjuntos, "lo guardado en el archivo");
        let ahora = adjuntos_de(&r, &e);
        assert_eq!(
            ahora.iter().map(|m| m.clase.clone()).collect::<Vec<_>>(),
            vec![Some(Clase::Imagen), Some(Clase::Voz)]
        );
        for m in &ahora {
            assert_eq!(
                m.responde_a.as_deref(),
                Some("lec-c1"),
                "le responde, como en el movil"
            );
            assert!(carpeta.join(m.ruta.as_deref().unwrap()).is_file());
        }
        assert_eq!(ahora[1].duracion_ms, 4_200);
        assert_eq!(ahora[1].nombre, NOMBRE_AUDIO);
        let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
        assert!(
            !c.mensajes.iter().any(|m| m.id == vieja.id),
            "la quitada sale del chat"
        );
        assert!(!fichero_viejo.exists(), "y su archivo del disco");
        assert_eq!(c.mensajes.len(), 3, "la leccion y sus dos adjuntos");

        // Caso negativo: sin cambios no se toca ningun adjunto ni se quita nada.
        guardar_con_cambios(&r, &e.leccion, &Donde::de(&e), "K7Q2", &[], &[], &[]).unwrap();
        let e = listar(&r).remove(0);
        assert_eq!(adjuntos_de(&r, &e).len(), 2);
        // Un id que no esta no se lleva nada por delante.
        guardar_con_cambios(
            &r,
            &e.leccion,
            &Donde::de(&e),
            "K7Q2",
            &[],
            &[],
            &["no-esta".to_string()],
        )
        .unwrap();
        assert_eq!(
            cuaderno::Cuaderno::leer_de(&carpeta)
                .unwrap()
                .mensajes
                .len(),
            3
        );
        let _ = std::fs::remove_dir_all(&r);
    }

    /// Una foto que le puso otro aparato (le responde pero no esta en su
    /// lista) tambien es de la leccion.
    #[test]
    fn lo_que_responde_a_la_leccion_es_suyo_aunque_no_este_en_su_lista() {
        let r = raiz("responde");
        let mut indice = almacen::Indice::default();
        let ficha = almacen::Ficha::nueva("Obra", 1, "K7Q2");
        indice.proyectos.push(ficha.clone());
        indice.guardar(&r).unwrap();
        guardar(
            &r,
            &Leccion::nueva("d1", 1000, "Uno"),
            &Donde::Nueva {
                ficha: ficha.id.clone(),
            },
            "K7Q2",
        )
        .unwrap();
        let carpeta = almacen::carpeta(&r, &ficha.id);
        let de_otro = Mensaje {
            id: "otro-1".into(),
            clase: Some(Clase::Imagen),
            responde_a: Some("lec-d1".into()),
            ..Default::default()
        };
        let suelta = Mensaje {
            id: "suelta".into(),
            clase: Some(Clase::Imagen),
            ..Default::default()
        };
        let en_buzon = Mensaje {
            id: "buzon".into(),
            clase: Some(Clase::Imagen),
            responde_a: Some("lec-d1".into()),
            en_buzon: true,
            ..Default::default()
        };
        for m in [&de_otro, &suelta, &en_buzon] {
            cuaderno::anadir(&carpeta, m).unwrap();
        }
        let e = listar(&r).remove(0);
        let ids: Vec<String> = adjuntos_de(&r, &e).into_iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["otro-1".to_string()]);
        let _ = std::fs::remove_dir_all(&r);
    }
}
