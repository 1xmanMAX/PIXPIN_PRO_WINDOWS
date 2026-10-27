//! # Versiones vinculadas en los chats compartidos (27-oct-2026)
//!
//! Lo de `sincro/VersionesVinculadas.kt` de PixPin Android v0.145.0
//! (`docs/versiones-en-chats-compartidos.md` alli), con las mismas reglas,
//! los mismos campos y los mismos nombres de archivo.
//!
//! El usuario: «en los chats compartidos puede pasar que yo edito lo que
//! alguien me paso y esa persona edita lo que yo le pase, y al sincronizar
//! hay incongruencias. Mantener dos versiones, la no tocada y la tocada».
//!
//! **Por que no basta con fusionar.** Entre mis aparatos la fusion de tres
//! vias junta lo que hice en el movil y en el PC porque los dos cambios son
//! **mios**. Entre dos personas no: si Ana mueve la planta de su lienzo y yo
//! la borro en el mio, juntar los dos da una tercera cosa que nadie dibujo.
//!
//! ## La regla
//!
//! Solo en un **chat compartido** con otras personas; en uno mio, y entre
//! mis aparatos, todo sigue como siempre.
//!
//! 1. **Lo mio se edita en su sitio.** Mio es lo que escribio cualquiera de
//!    mis aparatos (`pixpin_ui::conversacion::Lados::quien`): [`es_ajeno`].
//! 2. **Lo ajeno no se toca nunca.** Al primer cambio sobre un mensaje de
//!    otra persona se hace una **copia mia** ([`nueva`]): id, codigo unico,
//!    rutas y dibujos nuevos, mi aparato, `versionDe` (el codigo unico del
//!    original) y `versionN` (1, 2…). Y **contesta al original**
//!    (`respondeA`): cualquier PixPin, tambien uno que no sepa de versiones,
//!    la ensena debajo, citandolo.
//! 3. **Lo que se siga cambiando va a la copia**, que ya es mia: regla 1.
//! 4. Si otra persona toca lo mio, es ella la que hace su copia: a mi me
//!    llega como un mensaje nuevo que contesta al mio. El mio no cambia.
//!
//! Volver a abrir el original y cambiarlo otra vez hace **otra** version: el
//! original siempre se abre tal cual es. Para seguir con la mia, se abre la
//! mia.
//!
//! ## Que se versiona ([`como`])
//!
//! - **Lienzos, paginas, fotos y documentos anotados**: **borrador**
//!   ([`Como::Borrador`]). Al abrir se copia todo a nombres nuevos
//!   ([`empezar`]) y el editor trabaja sobre la copia; el mensaje solo nace
//!   si al volver algo cambio ([`cerrar`]). Abrir para mirar no deja nada.
//! - **Mini-aplicaciones**: **provisional** en Android (la copia entra en el
//!   chat al abrir y se quita si no cambio). En el PC el panel de la mini-app
//!   vive dentro del chat y sabe cuando cambia algo, asi que la version nace
//!   al primer cambio, **al momento**, con el cambio ya hecho: lo mismo que
//!   ve quien mira el chat, sin la copia provisional entre medias.
//! - **Notas y cambios de un toque** (describir un archivo): **al momento**
//!   ([`Como::AlMomento`], [`al_momento`]).
//! - **No** se versionan las **listas de tareas** (tachar en una compartida
//!   es lo que se quiere), las **tablas del chat** ni lo que no se edita
//!   aqui (audios, videos, croquis, proyectos, lo que abre otra aplicacion).
//!
//! **Si cambio** se mide sobre la copia ([`huella`]), sin depender de como
//! reescriba cada editor. Un borrador sin cambios se guarda en
//! `<raiz>/sincro/versiones-en-curso.json` (no viaja: `sincro/` no es una
//! carpeta que se mande) por si su editor sigue abierto, y se tira a las
//! doce horas ([`CADUCA_MS`]).
//!
//! ## Los archivos
//!
//! Todo se nombra en **rutas portatiles** (las del movil, `pins/draw/…`,
//! `guardados/…`) y se pone donde `vista::DiscoPc::ruta` dice que esta cada
//! una en este equipo: asi los nombres son los de Android sin traducir nada,
//! y la sincronizacion los manda como cualquier otro archivo de un mensaje.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use pixpin_sincro::anotado as a;
use pixpin_sincro::disco::{Disco, PORTATIL};
use serde::{Deserialize, Serialize};

use crate::almacen;
use crate::cuaderno::{self, Clase, Mensaje};
use crate::vista::{self, DiscoPc};

/// Como se hace la version de un mensaje ajeno. Ver el modulo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Como {
    Borrador,
    Provisional,
    AlMomento,
}

/// Un borrador sin tocar se guarda doce horas por si su editor sigue
/// abierto (los lienzos y el lector se abren en ventanas aparte y se puede
/// volver al chat sin cerrarlos); despues se tira.
pub const CADUCA_MS: i64 = 12 * 60 * 60 * 1000;

/// Lo que se anota con el lector, el visor o las hojas, como en el movil:
/// el resto de archivos se abre fuera y no se edita aqui.
const DOCUMENTOS: [&str; 17] = [
    "pdf", "doc", "docx", "odt", "rtf", "ppt", "pptx", "odp", "epub", "html", "htm", "xls", "xlsx",
    "xlsm", "ods", "csv", "tsv",
];
const VIDEOS: [&str; 7] = ["mp4", "mkv", "mov", "webm", "3gp", "avi", "m4v"];

/// Lo que va al lado de un dibujo: el dibujo, sus marcadores y sus vistas.
const JUNTO_AL_DIBUJO: [&str; 3] = [".excalidraw.gz", ".marcas", a::VISTAS];

/// Donde se apuntan los borradores, en la carpeta que no viaja.
pub const REGISTRO: &str = "versiones-en-curso.json";

// ------------------------------------------------------------------ decidir

/// El codigo de aparato de un mensaje en su forma corta de cuatro signos
/// (`EntrePersonas.aparatoCorto`): uno largo (el id entero de un aparato
/// viejo) se acorta como en la burbuja.
pub fn aparato_corto(m: &Mensaje) -> Option<String> {
    let a = m.aparato.as_deref()?.trim();
    if a.chars().count() > 6 {
        Some(crate::codigos::de_aparato(a))
    } else {
        Some(a.to_string())
    }
}

/// Si un codigo de aparato (ya corto) es de otra persona de la lista.
pub type DeOtro<'a> = &'a dyn Fn(Option<&str>) -> bool;

/// **Si `m` es de otra persona** en un chat compartido. `de_otro` dice si un
/// codigo de aparato (ya corto) es de otra persona de la lista
/// (`Lados::quien(..) == Quien::Otro`); `None` en un chat que no es
/// compartido, y entonces nada es ajeno.
pub fn es_ajeno(m: &Mensaje, de_otro: Option<DeOtro>) -> bool {
    de_otro.is_some_and(|f| f(aparato_corto(m).as_deref()))
}

/// La extension de su ruta o, sin ruta, de su nombre, en minusculas.
fn extension(m: &Mensaje) -> String {
    let base = m
        .ruta
        .as_deref()
        .filter(|r| !r.is_empty())
        .unwrap_or(&m.nombre);
    let nombre = base.rsplit(['/', '\\']).next().unwrap_or(base);
    nombre
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .unwrap_or_default()
}

fn con_ruta(m: &Mensaje) -> bool {
    m.ruta.as_deref().is_some_and(|r| !r.is_empty())
}

/// **Como se versiona `m` si es ajeno**, o `None` si se abre y se cambia
/// como siempre.
pub fn como(m: &Mensaje) -> Option<Como> {
    match m.clase.as_ref()? {
        Clase::Dibujo | Clase::Pagina => m
            .referencia
            .as_deref()
            .is_some_and(|r| !r.is_empty())
            .then_some(Como::Borrador),
        Clase::Imagen => {
            (con_ruta(m) && !VIDEOS.contains(&extension(m).as_str())).then_some(Como::Borrador)
        }
        Clase::Archivo => {
            (con_ruta(m) && DOCUMENTOS.contains(&extension(m).as_str())).then_some(Como::Borrador)
        }
        Clase::MiniApp => match m.miniapp.as_deref() {
            Some(crate::mini::TAREAS) | Some(crate::tabla::MINIAPP) => None,
            _ => Some(Como::Provisional),
        },
        Clase::Nota => Some(Como::AlMomento),
        _ => None,
    }
}

/// Si tocar `m` hace una version en vez de cambiarlo: es ajeno y es de lo
/// que se versiona.
pub fn se_versiona(m: &Mensaje, ajeno: bool) -> bool {
    ajeno && como(m).is_some()
}

// ------------------------------------------------------------------ el vinculo

/// Las versiones de `original` que hay en `lista`, de la mas vieja a la mas
/// nueva.
pub fn versiones_de<'a>(lista: &'a [Mensaje], original: &Mensaje) -> Vec<&'a Mensaje> {
    let uid = original.codigo_unico();
    let mut v: Vec<&Mensaje> = lista
        .iter()
        .filter(|m| m.version_de.as_deref() == Some(uid.as_str()) && m.id != original.id)
        .collect();
    v.sort_by_key(|m| m.cuando);
    v
}

/// El original de la version `v`, si esta en `lista`.
pub fn original_de<'a>(lista: &'a [Mensaje], v: &Mensaje) -> Option<&'a Mensaje> {
    let uid = v.version_de.as_deref()?;
    lista.iter().find(|m| m.codigo_unico() == uid)
}

/// Lo que no se copia a una version: es de este aparato o de la
/// organizacion del chat (recordatorio, unido a las hojas, de donde vino, la
/// hoja del texto). Los demas (fijado, etiqueta, buzon, letra, origen) son
/// campos propios del mensaje.
const NO_SE_COPIAN: [&str; 4] = ["recuerdaEn", "unido", "recibidoDe", "hojaDelTexto"];

/// **La version de `original`, sin archivos**: otro mensaje, mio, que
/// contesta al original y dice de cual es. Codigos nuevos (con los del
/// original se tomaria por el al sincronizar y lo pisaria); el numero en el
/// chat lo pone quien la guarda ([`con_numero`]).
pub fn nueva(
    original: &Mensaje,
    id: &str,
    uid: &str,
    aparato: Option<&str>,
    ahora: i64,
    n: i64,
) -> Mensaje {
    let mut v = original.clone();
    v.id = id.to_string();
    v.uid = Some(uid.to_string());
    v.cuando = ahora;
    v.numero = 0;
    v.letra = None;
    v.aparato = aparato.map(str::to_string);
    v.origen = None;
    v.responde_a = Some(original.id.clone());
    v.version_de = Some(original.codigo_unico());
    v.version_n = Some(n);
    v.fijado = false;
    v.emoji = None;
    v.en_buzon = false;
    for k in NO_SE_COPIAN {
        v.resto.remove(k);
    }
    // Una nota que abre una hoja de proyecto: la copia ya no es esa hoja.
    if original.clase == Some(Clase::Nota) {
        v.referencia = None;
    }
    v
}

/// **Una version reenviada a otro chat deja de ser version**: su original
/// se queda en el chat de donde sale (`reenviado` del movil).
pub fn sin_vinculo(m: &mut Mensaje) {
    m.version_de = None;
    m.version_n = None;
}

/// El numero en su chat, como `MensajesStore.anadir` del movil: uno mas que
/// el mayor, nunca menos que cuantos hay.
pub fn con_numero(lista: &[Mensaje], mut v: Mensaje) -> Mensaje {
    if v.numero > 0 {
        return v;
    }
    let suyos: Vec<&Mensaje> = lista.iter().filter(|m| m.proyecto == v.proyecto).collect();
    let mayor = suyos.iter().map(|m| m.numero).max().unwrap_or(0);
    v.numero = mayor.max(suyos.len() as i64) + 1;
    v
}

/// Lo que hay que hacer en un chat al volver de los editores: anadir
/// versiones y quitar provisionales que no se tocaron.
#[derive(Debug, Clone, Default)]
pub struct Cierre {
    pub anadir: Vec<Mensaje>,
    pub quitar: BTreeSet<String>,
}

impl Cierre {
    pub fn nada(&self) -> bool {
        self.anadir.is_empty() && self.quitar.is_empty()
    }
}

/// Los mensajes de `lista` con los de `cierre`: sin los quitados y con los
/// nuevos al final, cada uno con su numero.
pub fn con_versiones(lista: &[Mensaje], cierre: &Cierre) -> Vec<Mensaje> {
    let mut salida: Vec<Mensaje> = lista
        .iter()
        .filter(|m| !cierre.quitar.contains(&m.id))
        .cloned()
        .collect();
    for v in &cierre.anadir {
        if !salida.iter().any(|m| m.id == v.id) {
            let v = con_numero(&salida, v.clone());
            salida.push(v);
        }
    }
    salida
}

/// **El nombre de la copia de un adjunto**: `<hora>_v<n>_<nombre>`, con el
/// nombre sin la hora de delante que le puso quien lo guardo
/// (`1761_plano.pdf` → `5000_v1_plano.pdf`).
pub fn nombre_de_la_copia(cuando: i64, n: i64, nombre: &str) -> String {
    format!("{cuando}_v{n}_{}", sin_hora_delante(nombre))
}

/// `1761_plano.pdf` → `plano.pdf` (`replace(Regex("^\\d+_"), "")`).
fn sin_hora_delante(nombre: &str) -> &str {
    match nombre.split_once('_') {
        Some((cifras, resto))
            if !cifras.is_empty() && cifras.chars().all(|c| c.is_ascii_digit()) =>
        {
            resto
        }
        _ => nombre,
    }
}

/// Si un nombre de fichero es el de una copia (`^\d+_v\d+_`).
fn es_nombre_de_copia(nombre: &str) -> bool {
    let Some((hora, resto)) = nombre.split_once('_') else {
        return false;
    };
    let Some(resto) = resto.strip_prefix('v') else {
        return false;
    };
    let Some((n, _)) = resto.split_once('_') else {
        return false;
    };
    let cifras = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    cifras(hora) && cifras(n)
}

// ------------------------------------------------------------------ los rotulos

/// De quien es el original, para el rotulo de encima de una version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeQuien {
    /// De otra persona, por su nombre: «Version de Ana».
    De(String),
    /// Mio: «Version de lo tuyo».
    Tuyo,
    /// El original no esta en este chat: «Version de otro mensaje».
    OtroMensaje,
    /// El chat ya no es compartido: «Version», sin nombrar a nadie.
    SinNombrar,
}

/// El rotulo de encima de una version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cabecera {
    /// El id del original en este chat, adonde lleva tocarlo; `None` si no
    /// esta.
    pub original: Option<String>,
    /// El numero, solo si es la 2 o mas («Version 2 de Ana»).
    pub n: Option<i64>,
    pub de: DeQuien,
}

/// Una version en la lista de su original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnLaLista {
    /// El id de la version, adonde lleva.
    pub id: String,
    /// «Version 1», «Version 2»…
    pub n: i64,
    /// De quien es: `Some(Some(nombre))` otra persona, `Some(None)` mia
    /// («tuya»), `None` en un chat que ya no es compartido.
    pub quien: Option<Option<String>>,
}

/// Los rotulos de un chat: el de cada version (por su id) y la lista de las
/// versiones de cada original (por el id del original).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rotulos {
    pub cabecera: std::collections::HashMap<String, Cabecera>,
    pub versiones: std::collections::HashMap<String, Vec<EnLaLista>>,
}

/// **Que rotulo lleva cada burbuja** (`VersionesEnElChat.delChat`). `autor`
/// da el nombre de la persona de un mensaje, o `None` si es mio.
/// `compartido`: si no lo es (un chat del que ya se salio), no se nombra a
/// nadie.
pub fn rotulos(
    mensajes: &[Mensaje],
    compartido: bool,
    autor: impl Fn(&Mensaje) -> Option<String>,
) -> Rotulos {
    if mensajes.iter().all(|m| m.version_de.is_none()) {
        return Rotulos::default();
    }
    let mut por_uid: std::collections::HashMap<String, &Mensaje> = std::collections::HashMap::new();
    for m in mensajes {
        por_uid.entry(m.codigo_unico()).or_insert(m);
    }
    let mut r = Rotulos::default();
    let mut listas: std::collections::HashMap<String, Vec<(i64, EnLaLista)>> =
        std::collections::HashMap::new();
    for v in mensajes {
        let Some(uid) = v.version_de.as_deref() else {
            continue;
        };
        let original = por_uid.get(uid).copied().filter(|o| o.id != v.id);
        let de = match original {
            _ if !compartido => DeQuien::SinNombrar,
            None => DeQuien::OtroMensaje,
            Some(o) => autor(o).map_or(DeQuien::Tuyo, DeQuien::De),
        };
        r.cabecera.insert(
            v.id.clone(),
            Cabecera {
                original: original.map(|o| o.id.clone()),
                n: v.version_n.filter(|n| *n > 1),
                de,
            },
        );
        if let Some(o) = original {
            listas.entry(o.id.clone()).or_default().push((
                v.cuando,
                EnLaLista {
                    id: v.id.clone(),
                    n: v.version_n.unwrap_or(1),
                    quien: compartido.then(|| autor(v)),
                },
            ));
        }
    }
    for (id, mut l) in listas {
        l.sort_by_key(|(c, _)| *c);
        r.versiones
            .insert(id, l.into_iter().map(|(_, e)| e).collect());
    }
    r
}

// ------------------------------------------------------------------ los archivos

/// El chat (como viaja) de una ficha.
fn chat_de(raiz: &Path, ficha: &str) -> String {
    vista::chat_de_ficha(raiz, ficha).unwrap_or_else(|| ficha.to_string())
}

fn dibujo_rel(id: &str, fin: &str) -> String {
    format!("{}/{id}{fin}", a::CARPETA)
}

/// El dibujo de un mensaje, si tiene: el de un lienzo o una pagina, o el
/// lienzo de una foto (`dibujoDeLaFoto`: su referencia o `foto-<id>`).
fn dibujo_de(m: &Mensaje) -> Option<String> {
    match m.clase.as_ref()? {
        Clase::Dibujo | Clase::Pagina => m.referencia.clone().filter(|r| !r.is_empty()),
        Clase::Imagen => Some(crate::lienzo_de_la_foto::id_del_lienzo(m)),
        _ => None,
    }
}

fn copiar(de: &Path, a: &Path) -> io::Result<()> {
    if let Some(p) = a.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::copy(de, a).map(|_| ())
}

/// El dibujo `id` y lo que va a su lado (marcadores, vistas), copiado a
/// `nuevo`.
fn copiar_dibujo(d: &DiscoPc, chat: &str, id: &str, nuevo: &str) {
    for fin in JUNTO_AL_DIBUJO {
        let f = d.ruta(chat, &dibujo_rel(id, fin));
        if f.is_file() {
            let _ = copiar(&f, &d.ruta(chat, &dibujo_rel(nuevo, fin)));
        }
    }
}

/// Lo anotado de cada codigo en el chat, en rutas portatiles.
fn anotado_por_uid(d: &DiscoPc, chat: &str) -> BTreeMap<String, Vec<String>> {
    let lista = d.anotado(chat);
    a::por_uid(lista.iter().map(String::as_str))
}

/// `pins/draw/anot-<de>…` → `pins/draw/anot-<a>…`, solo en el nombre.
fn anotado_con_otro_uid(rel: &str, de: &str, a_uid: &str) -> String {
    let (carpeta, nombre) = rel.rsplit_once('/').unwrap_or(("", rel));
    let nuevo = nombre.replacen(
        &format!("{}{de}", a::PREFIJO),
        &format!("{}{a_uid}", a::PREFIJO),
        1,
    );
    if carpeta.is_empty() {
        nuevo
    } else {
        format!("{carpeta}/{nuevo}")
    }
}

/// **La version `v` con sus propios archivos**: el adjunto copiado a otra
/// ruta (`guardados/<hora>_v<n>_<nombre>`), el dibujo a otro id
/// (`dib-<id>` o `foto-<id>`) y lo anotado con el codigo del original
/// copiado al suyo. Copias y no enlaces: si el original se borra, se lleva
/// sus archivos, y la version tiene que seguir entera.
pub fn con_archivos(raiz: &Path, ficha: &str, original: &Mensaje, v: Mensaje) -> Mensaje {
    let d = DiscoPc::nuevo(raiz);
    let chat = chat_de(raiz, ficha);
    let mut salida = v;
    if let Some(f) = original
        .ruta
        .as_deref()
        .filter(|r| !r.is_empty())
        .and_then(|r| vista::ruta_real(raiz, ficha, r))
        .filter(|f| f.is_file())
        && let Some(nombre) = f.file_name().map(|n| n.to_string_lossy().to_string())
    {
        let nombre = almacen::nombre_seguro(&nombre);
        let rel = format!(
            "guardados/{}",
            nombre_de_la_copia(salida.cuando, salida.version_n.unwrap_or(1), &nombre)
        );
        if pixpin_sincro::disco::permitida(&rel) && copiar(&f, &d.ruta(&chat, &rel)).is_ok() {
            salida.ruta = Some(format!("{PORTATIL}{rel}"));
        }
    }
    match original.clase {
        Some(Clase::Dibujo) | Some(Clase::Pagina) => {
            if let Some(r) = original.referencia.as_deref().filter(|r| !r.is_empty()) {
                let nuevo = format!("dib-{}", salida.id);
                copiar_dibujo(&d, &chat, r, &nuevo);
                salida.referencia = Some(nuevo);
            }
        }
        // La foto: su dibujo si ya lo tenia. Si no, el editor lo hara con el
        // nombre de la copia (`foto-<id>`).
        Some(Clase::Imagen) => {
            let suyo = crate::lienzo_de_la_foto::id_del_lienzo(original);
            if d.ruta(&chat, &dibujo_rel(&suyo, ".excalidraw.gz"))
                .is_file()
            {
                let nuevo = format!("foto-{}", salida.id);
                copiar_dibujo(&d, &chat, &suyo, &nuevo);
                salida.referencia = Some(nuevo);
            } else {
                salida.referencia = None;
            }
        }
        _ => {}
    }
    let de = original.codigo_unico();
    let a_uid = salida.codigo_unico();
    for rel in anotado_por_uid(&d, &chat).remove(&de).unwrap_or_default() {
        let f = d.ruta(&chat, &rel);
        let _ = copiar(&f, &d.ruta(&chat, &anotado_con_otro_uid(&rel, &de, &a_uid)));
    }
    salida
}

/// Todo lo que se copio para `v`: al tirar un borrador que no se toco.
pub fn borrar_archivos(raiz: &Path, ficha: &str, v: &Mensaje) {
    let d = DiscoPc::nuevo(raiz);
    let chat = chat_de(raiz, ficha);
    // Solo la copia (`guardados/<hora>_v<n>_…`): si copiar fallo, la ruta es
    // la del original.
    if let Some(rel) = v
        .ruta
        .as_deref()
        .and_then(|r| r.strip_prefix(PORTATIL))
        .and_then(|r| r.strip_prefix("guardados/"))
        .filter(|n| !n.contains('/') && es_nombre_de_copia(n))
    {
        let _ = std::fs::remove_file(d.ruta(&chat, &format!("guardados/{rel}")));
    }
    if let Some(r) = dibujo_de(v) {
        for fin in JUNTO_AL_DIBUJO {
            let _ = std::fs::remove_file(d.ruta(&chat, &dibujo_rel(&r, fin)));
        }
    }
    if let Some(u) = v.uid.as_deref() {
        for rel in anotado_por_uid(&d, &chat).remove(u).unwrap_or_default() {
            let _ = std::fs::remove_file(d.ruta(&chat, &rel));
        }
    }
}

// ------------------------------------------------------------------ si cambio

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn sha256_hex(b: &[u8]) -> String {
    hex(&crate::codigos::sha256(b))
}

/// **La huella de lo editable de `m`**: su texto y nombre, su dibujo y lo
/// anotado con su codigo. No depende de los nombres de los archivos —la del
/// borrador recien copiado es la del original— ni de como los vuelva a
/// escribir cada editor: de un dibujo cuentan los elementos vivos con su
/// `version`, que sube con cada cambio. Lo que es de leer (`.sitio`, `.voz`)
/// y el marco de la hoja (`.hoja`, que sale de la pagina) no cuentan.
///
/// **En el PC cuenta tambien el adjunto mismo** (su tamano y su fecha): el
/// visor de HTML y el pin de las hojas escriben en el fichero, no en lo
/// anotado, y sin esto lo hecho ahi no seria un cambio. La huella no viaja:
/// solo la compara este equipo consigo mismo.
pub fn huella(raiz: &Path, ficha: &str, m: &Mensaje, sin_fotos: bool) -> String {
    let d = DiscoPc::nuevo(raiz);
    let chat = chat_de(raiz, ficha);
    let mut partes: BTreeMap<String, String> = BTreeMap::new();
    partes.insert("texto".into(), m.texto.clone());
    partes.insert("nombre".into(), m.nombre.clone());
    if let Some(r) = dibujo_de(m) {
        partes.insert(
            "dibujo".into(),
            huella_de_dibujo(&d.ruta(&chat, &dibujo_rel(&r, ".excalidraw.gz")), sin_fotos),
        );
        for fin in [".marcas", a::VISTAS] {
            if let Ok(t) = std::fs::read_to_string(d.ruta(&chat, &dibujo_rel(&r, fin))) {
                partes.insert(format!("dibujo{fin}"), t.trim().to_string());
            }
        }
    }
    let uid = m.codigo_unico();
    for rel in anotado_por_uid(&d, &chat).remove(&uid).unwrap_or_default() {
        let nombre = rel.rsplit('/').next().unwrap_or(&rel);
        let fin = nombre
            .strip_prefix(&format!("{}{uid}", a::PREFIJO))
            .unwrap_or(nombre);
        if fin.ends_with(".sitio") || fin.ends_with(".voz") || fin.ends_with(a::HOJA) {
            continue;
        }
        let f = d.ruta(&chat, &rel);
        let valor = if fin.ends_with(".excalidraw.gz") {
            huella_de_dibujo(&f, false)
        } else {
            std::fs::read_to_string(&f)
                .map(|t| t.trim().to_string())
                .unwrap_or_default()
        };
        partes.insert(format!("anot{fin}"), valor);
    }
    if let Some(f) = m
        .ruta
        .as_deref()
        .filter(|r| !r.is_empty())
        .and_then(|r| vista::ruta_real(raiz, ficha, r))
        && let Ok(md) = std::fs::metadata(&f)
    {
        let fecha = md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_millis());
        partes.insert("archivo".into(), format!("{}:{fecha}", md.len()));
    }
    let junto: Vec<String> = partes.iter().map(|(k, v)| format!("{k}={v}")).collect();
    sha256_hex(junto.join("\u{0}").as_bytes())
}

/// Los elementos vivos de un dibujo, `id:version`, ordenados; vacio si no
/// hay dibujo. Con `sin_fotos`, sin las imagenes: la foto de fondo que pone
/// el editor al estrenar el lienzo de una foto no es un cambio. Lo que no es
/// un dibujo que se entienda cuenta entero.
pub fn huella_de_dibujo(f: &Path, sin_fotos: bool) -> String {
    let Ok(bytes) = std::fs::read(f) else {
        return String::new();
    };
    let Ok(valor) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return sha256_hex(&bytes);
    };
    let Some(elementos) = valor.get("elements").and_then(|e| e.as_array()) else {
        return sha256_hex(&bytes);
    };
    let texto = |v: Option<&serde_json::Value>| match v {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Null) | None => "null".to_string(),
        Some(otro) => otro.to_string(),
    };
    let mut v: Vec<String> = elementos
        .iter()
        .filter_map(|e| {
            let o = e.as_object()?;
            if o.get("isDeleted").and_then(|b| b.as_bool()) == Some(true) {
                return None;
            }
            if sin_fotos && o.get("type").and_then(|t| t.as_str()) == Some("image") {
                return None;
            }
            Some(format!(
                "{}:{}",
                texto(o.get("id")),
                texto(o.get("version"))
            ))
        })
        .collect();
    v.sort();
    v.join(",")
}

// ------------------------------------------------------------------ el apunte

/// **Un borrador (o una version provisional) que se esta editando**,
/// apuntado en disco para que sobreviva a cerrar la aplicacion con el
/// editor abierto. Las claves son las de `EnCurso` del movil, mas la ficha:
/// en el PC cada chat vive en su carpeta.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnCurso {
    /// La version preparada: con sus rutas, su dibujo y su codigo nuevos.
    pub version: Mensaje,
    /// El id del original.
    pub original: String,
    /// La [`huella`] de la version al abrir, que es la del original: si
    /// sigue igual, no se toco.
    pub huella: String,
    /// Si ya esta en el chat ([`Como::Provisional`]); si no, es un borrador.
    #[serde(default)]
    pub provisional: bool,
    /// Una foto que aun no tenia dibujo: la foto que pone el editor de
    /// fondo no es un cambio.
    #[serde(default)]
    pub sin_fotos: bool,
    /// Cuando se abrio.
    #[serde(default)]
    pub desde: i64,
    /// La ficha (el proyecto del PC) de su chat.
    #[serde(default)]
    pub ficha: String,
}

fn cerrojo() -> std::sync::MutexGuard<'static, ()> {
    static C: std::sync::Mutex<()> = std::sync::Mutex::new(());
    C.lock().unwrap_or_else(|e| e.into_inner())
}

/// Donde se apuntan: `<raiz>/sincro/versiones-en-curso.json`, que no viaja.
pub fn registro(raiz: &Path) -> PathBuf {
    raiz.join("sincro").join(REGISTRO)
}

/// Si hay algo apuntado (sin leerlo): lo que se pregunta a cada vuelta.
pub fn hay_en_curso(raiz: &Path) -> bool {
    registro(raiz).is_file()
}

/// Los borradores que se estan editando. Lo que no se entiende, como nada.
pub fn en_curso(raiz: &Path) -> Vec<EnCurso> {
    std::fs::read_to_string(registro(raiz))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn guardar_en_curso(raiz: &Path, lista: &[EnCurso]) -> io::Result<()> {
    let f = registro(raiz);
    if lista.is_empty() {
        return match std::fs::remove_file(&f) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    let texto = serde_json::to_vec(lista).map_err(io::Error::other)?;
    pixpin_sincro::disco::escribir_atomico(&f, &texto)
}

/// La version en el chat, si es provisional y sigue ahi; si no, la apuntada.
fn actual<'a>(e: &'a EnCurso, lista: &'a [Mensaje]) -> Option<&'a Mensaje> {
    if e.provisional {
        lista.iter().find(|m| m.id == e.version.id)
    } else {
        Some(&e.version)
    }
}

/// Un id nuevo para una version: la hora y un codigo al azar, que vale
/// tambien como nombre de fichero (`dib-<id>`).
pub fn id_nuevo(ahora: i64) -> String {
    format!("{ahora}-{}", crate::codigos::nuevo().to_lowercase())
}

/// **Empieza a editar `original` (de la ficha `ficha`) como version mia**:
/// prepara la copia con sus archivos y la apunta. Si ya habia un borrador
/// suyo sin tocar (se abrio para mirar), se sigue con ese en vez de llenar
/// el disco de copias. Quien llama pone en el chat la de
/// [`EnCurso::provisional`] y abre el editor sobre [`EnCurso::version`].
#[allow(clippy::too_many_arguments)]
pub fn empezar(
    raiz: &Path,
    ficha: &str,
    original: &Mensaje,
    lista: &[Mensaje],
    aparato: Option<&str>,
    ahora: i64,
    id: &str,
    uid: &str,
) -> io::Result<EnCurso> {
    let _c = cerrojo();
    let mut apuntados = en_curso(raiz);
    if let Some(e) = apuntados.iter().find(|e| {
        e.ficha == ficha
            && e.original == original.id
            && actual(e, lista).is_some_and(|v| huella(raiz, ficha, v, e.sin_fotos) == e.huella)
    }) {
        return Ok(e.clone());
    }
    let n = versiones_de(lista, original).len() as i64 + 1;
    let v = con_archivos(
        raiz,
        ficha,
        original,
        nueva(original, id, uid, aparato, ahora, n),
    );
    let sin_fotos = original.clase == Some(Clase::Imagen) && v.referencia.is_none();
    let e = EnCurso {
        huella: huella(raiz, ficha, &v, sin_fotos),
        version: v,
        original: original.id.clone(),
        provisional: como(original) == Some(Como::Provisional),
        sin_fotos,
        desde: ahora,
        ficha: ficha.to_string(),
    };
    apuntados.push(e.clone());
    guardar_en_curso(raiz, &apuntados)?;
    Ok(e)
}

/// **Una version de un toque** ([`Como::AlMomento`]): la copia con `cambio`
/// ya hecho, lista para anadir al chat. Nada queda apuntado.
#[allow(clippy::too_many_arguments)]
pub fn al_momento(
    raiz: &Path,
    ficha: &str,
    original: &Mensaje,
    lista: &[Mensaje],
    aparato: Option<&str>,
    ahora: i64,
    cambio: impl FnOnce(Mensaje) -> Mensaje,
    id: &str,
    uid: &str,
) -> Mensaje {
    let n = versiones_de(lista, original).len() as i64 + 1;
    let v = con_archivos(
        raiz,
        ficha,
        original,
        nueva(original, id, uid, aparato, ahora, n),
    );
    // Lo que identifica a la version no lo cambia el cambio.
    let mut c = cambio(v.clone());
    c.id = v.id;
    c.uid = v.uid;
    c.aparato = v.aparato;
    c.version_de = v.version_de;
    c.version_n = v.version_n;
    c.responde_a = v.responde_a;
    c.cuando = v.cuando;
    c.ruta = v.ruta;
    c.referencia = v.referencia;
    c
}

/// **Lo que toca hacer en el chat de `ficha` al volver de los editores**,
/// sin escribir en el cuaderno: lo que cambio se queda como version y lo que
/// no, se tira. Un borrador cambiado da un mensaje nuevo (a esta hora, para
/// que salga abajo, como una respuesta); una provisional sin cambios se
/// quita con sus archivos; un borrador sin cambios se guarda hasta
/// [`CADUCA_MS`] por si su editor sigue abierto. `lista` es el chat tal como
/// esta.
pub fn cerrar_en(raiz: &Path, ficha: &str, lista: &[Mensaje], ahora: i64) -> io::Result<Cierre> {
    let _c = cerrojo();
    let apuntados = en_curso(raiz);
    let mut cierre = Cierre::default();
    if apuntados.is_empty() {
        return Ok(cierre);
    }
    let mut quedan = Vec::new();
    for e in apuntados {
        if e.ficha != ficha {
            quedan.push(e);
            continue;
        }
        // Una provisional que ya no esta (se borro a mano): se olvida.
        let Some(v) = actual(&e, lista) else {
            continue;
        };
        let cambiada = huella(raiz, ficha, v, e.sin_fotos) != e.huella;
        if cambiada && !e.provisional {
            let mut v = e.version.clone();
            v.cuando = v.cuando.max(ahora);
            cierre.anadir.push(v);
        } else if cambiada {
            // Una provisional cambiada ya esta en el chat: se queda.
        } else if e.provisional {
            cierre.quitar.insert(e.version.id.clone());
            borrar_archivos(raiz, ficha, &e.version);
        } else if ahora - e.desde > CADUCA_MS {
            borrar_archivos(raiz, ficha, &e.version);
        } else {
            quedan.push(e);
        }
    }
    guardar_en_curso(raiz, &quedan)?;
    Ok(cierre)
}

/// **Al volver al chat**: [`cerrar_en`] en cada chat con algo apuntado, y
/// lo que sale, escrito en su cuaderno. Devuelve las versiones nuevas, con
/// su ficha. Sin nada apuntado, ni se lee ningun chat.
pub fn cerrar(raiz: &Path, ahora: i64) -> io::Result<Vec<(String, Mensaje)>> {
    if !hay_en_curso(raiz) {
        return Ok(Vec::new());
    }
    let fichas: BTreeSet<String> = en_curso(raiz).into_iter().map(|e| e.ficha).collect();
    let mut hechas = Vec::new();
    for ficha in fichas {
        let carpeta = almacen::carpeta(raiz, &ficha);
        let lista = cuaderno::Cuaderno::leer_de(&carpeta)
            .map(|c| c.mensajes)
            .unwrap_or_default();
        let c = cerrar_en(raiz, &ficha, &lista, ahora)?;
        if c.nada() {
            continue;
        }
        if !c.quitar.is_empty() {
            cuaderno::quitar(&carpeta, &c.quitar)?;
        }
        let mut chat: Vec<Mensaje> = lista
            .into_iter()
            .filter(|m| !c.quitar.contains(&m.id))
            .collect();
        for v in c.anadir {
            if chat.iter().any(|m| m.id == v.id) {
                continue;
            }
            let v = con_numero(&chat, v);
            cuaderno::anadir(&carpeta, &v)?;
            chat.push(v.clone());
            hechas.push((ficha.clone(), v));
        }
    }
    Ok(hechas)
}

/// **El codigo unico del borrador cuyo adjunto es `doc`**, y su chat: su
/// tinta va con el aunque aun no sea mensaje (`uidDeBorrador`). Lo pregunta
/// `anotado::adjunto_de` antes de buscar en los chats.
pub fn borrador_de(raiz: &Path, doc: &Path) -> Option<(String, String)> {
    if !hay_en_curso(raiz) {
        return None;
    }
    let canonica = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let doc = canonica(doc);
    en_curso(raiz).into_iter().find_map(|e| {
        if e.provisional {
            return None;
        }
        let r = e.version.ruta.as_deref().filter(|r| !r.is_empty())?;
        let real = vista::ruta_real(raiz, &e.ficha, r)?;
        (canonica(&real) == doc).then(|| (chat_de(raiz, &e.ficha), e.version.codigo_unico()))
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_nombre_de_la_copia_es_el_del_movil() {
        assert_eq!(
            nombre_de_la_copia(5000, 1, "1761_plano.pdf"),
            "5000_v1_plano.pdf"
        );
        assert_eq!(
            nombre_de_la_copia(7000, 2, "informe.docx"),
            "7000_v2_informe.docx"
        );
        // Solo la primera hora: la copia de una copia lleva su `v1_`.
        assert_eq!(
            nombre_de_la_copia(9000, 2, "5000_v1_plano.pdf"),
            "9000_v2_v1_plano.pdf"
        );
        // Casos negativos: lo que no empieza por cifras y `_` se queda.
        assert_eq!(nombre_de_la_copia(1, 1, "_x.pdf"), "1_v1__x.pdf");
        assert_eq!(nombre_de_la_copia(1, 1, "a1_x.pdf"), "1_v1_a1_x.pdf");
        assert!(es_nombre_de_copia("5000_v1_plano.pdf"));
        assert!(!es_nombre_de_copia("1761_plano.pdf"));
        assert!(!es_nombre_de_copia("5000_vx_plano.pdf"));
    }

    #[test]
    fn lo_anotado_cambia_de_codigo_solo_en_el_nombre() {
        assert_eq!(
            anotado_con_otro_uid(
                "pins/draw/anot-AAAAAAAAAA-p0.excalidraw.gz",
                "AAAAAAAAAA",
                "BBBBBBBBBB"
            ),
            "pins/draw/anot-BBBBBBBBBB-p0.excalidraw.gz"
        );
    }
}
