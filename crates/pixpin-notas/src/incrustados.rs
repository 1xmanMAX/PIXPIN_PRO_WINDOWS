//! **Lo que se mete en una nota sin ser texto ni foto** (H12, 1-oct):
//! documentos, mensajes del chat, hojas enlazadas y audios con su
//! transcripcion. Aqui las cuentas puras sobre el Markdown (que renglon es
//! que, como se escribe, las marcas de tiempo) y lo que la aplicacion le da
//! al editor para pintarlos ([`Ficha`], [`Medios`]); el pintado esta en
//! [`pintar`] y el raton y el teclado en `editor/incrustados.rs`.
//!
//! # Como va en el `.md`, para que el movil no lo rompa
//!
//! Se escribe lo que el `Markdown.kt` del movil ya sabe leer, nada nuevo:
//!
//! - **Un documento o un audio**: `![Plano.pdf](pixpin:files/…/notas/…)`
//!   en un renglon suyo. Es el `MEDIO` del movil (`Markdown.medioDe`): un
//!   `![alt](ruta)` solo en su renglon es un medio y **la extension decide
//!   la clase** (`claseDeMedio`): imagen, video, audio o archivo. El movil
//!   pinta un archivo como tarjeta con su nombre y su extension
//!   (`MarkdownText.MedioUi`) y un audio con su boton de tocar; la
//!   sincronizacion lleva el fichero porque la ruta es `pixpin:files/`
//!   (`Disco.enTexto`, que corta en `"` y `)`: por eso el nombre de la copia
//!   no lleva parentesis, `adjuntos::nombre_limpio`).
//! - **La transcripcion de un audio**: parrafos `[1:23] lo que se dijo`
//!   debajo, separados por un renglon en blanco. Es lo que escribe el movil
//!   al transcribir (`Transcriptor.conTiempos`, `apuntarTranscripcion`:
//!   `![audio](voz.m4a)` y el texto debajo) y lo que su editor ya entiende:
//!   un parrafo que empieza por su minuto salta el audio de la nota a ese
//!   punto (`MarkdownText.minutoDelParrafo`, `AudioDeLaNota.saltar`).
//! - **Un mensaje del chat**: `[lo que dice](pixpin:mensaje=<proyecto>/<codigo>)`
//!   en un renglon suyo, con los codigos unicos del proyecto y del mensaje
//!   (los mismos en el movil). En el movil es un enlace de los de siempre
//!   (`Markdown.parseInline`, `SpanKind.LINK`): se lee su texto y no se
//!   pierde nada; aqui se pinta como la burbuja del chat.
//! - **Una hoja enlazada**: el `[Planta](pixpin:hoja=…)` de siempre
//!   (`md_imagen`), solo en su renglon, se pinta como su burbuja.
//!
//! Un renglon asi, con el cursor fuera, se ve solo pintado (el texto va
//! escondido, como el de una foto); con el cursor dentro, debajo asoma su
//! nombre para poder editarlo.

use std::path::PathBuf;

use pixpin_docs::{md_imagen, md_vivo};

pub(crate) mod pintar;

/// El esquema de los enlaces a un mensaje del chat.
pub const ESQUEMA_MENSAJE: &str = "pixpin:mensaje=";

/// Las extensiones de audio, las de `Markdown.claseDeMedio` del movil (y
/// de `AudioLigero.EXTENSIONES_DE_AUDIO`): si alli se anade una, aqui
/// tambien.
pub const EXTENSIONES_DE_AUDIO: [&str; 8] = ["mp3", "ogg", "oga", "m4a", "wav", "flac", "opus", "aac"];

/// Que clase de cosa es un renglon incrustado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clase {
    /// Un fichero que no es foto ni audio (PDF, Word, plano...).
    Archivo,
    /// Un audio: lleva su reproductor y, debajo, su transcripcion.
    Audio,
    /// Un mensaje del chat, como su burbuja.
    Mensaje,
    /// Una hoja de un proyecto enlazada, como su burbuja.
    Hoja,
}

/// Un renglon de la nota que se pinta como incrustado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renglon {
    /// El renglon, desde 0.
    pub linea: usize,
    pub clase: Clase,
    /// El texto del enlace (el nombre que se ve).
    pub nombre: String,
    /// La ruta o la direccion, tal cual va en el Markdown.
    pub ruta: String,
}

/// La extension de una ruta, en minusculas y sin `?…`.
fn extension(ruta: &str) -> String {
    let ultimo = ruta.rsplit(['/', '\\']).next().unwrap_or(ruta);
    match ultimo.rsplit_once('.') {
        Some((_, e)) => e.split('?').next().unwrap_or("").to_ascii_lowercase(),
        None => String::new(),
    }
}

/// Si una ruta es de un audio (por su extension, como el movil).
pub fn es_audio(ruta: &str) -> bool {
    EXTENSIONES_DE_AUDIO.contains(&extension(ruta).as_str())
}

/// `[texto](url)` o `![texto](url)` que ocupa el renglon entero. El texto
/// no lleva corchetes y la url no lleva parentesis (el movil corta ahi).
fn enlace_solo(renglon: &str) -> Option<(bool, String, String)> {
    let t = renglon.trim();
    let (medio, resto) = match t.strip_prefix("![") {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('[')?),
    };
    let (texto, resto) = resto.split_once("](")?;
    let url = resto.strip_suffix(')')?;
    if texto.contains(['[', ']']) || url.contains(['(', ')']) || url.trim().is_empty() {
        return None;
    }
    Some((medio, texto.trim().to_string(), url.trim().to_string()))
}

/// Si un renglon es un incrustado, y cual. Una foto no lo es (eso es de
/// `md_imagen`), ni un enlace a una pagina web.
pub fn de_renglon(renglon: &str) -> Option<(Clase, String, String)> {
    let (medio, nombre, ruta) = enlace_solo(renglon)?;
    if medio {
        if md_vivo::imagen_de(renglon).is_some() {
            return None;
        }
        let clase = if es_audio(&ruta) { Clase::Audio } else { Clase::Archivo };
        return Some((clase, nombre, ruta));
    }
    if mensaje_del_enlace(&ruta).is_some() {
        return Some((Clase::Mensaje, nombre, ruta));
    }
    if md_imagen::hoja_del_enlace(&ruta).is_some() {
        return Some((Clase::Hoja, nombre, ruta));
    }
    None
}

/// Si una linea abre o cierra un bloque de codigo.
fn es_valla(renglon: &str) -> bool {
    let t = renglon.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

/// Los renglones incrustados de la nota, en orden. Los de dentro de un
/// bloque de codigo no cuentan: ahi el Markdown se ensena tal cual.
pub fn renglones(texto: &str) -> Vec<Renglon> {
    let mut en_codigo = false;
    let mut v = Vec::new();
    for (linea, r) in texto.split(['\n', '\r']).enumerate() {
        if es_valla(r) {
            en_codigo = !en_codigo;
            continue;
        }
        if en_codigo {
            continue;
        }
        if let Some((clase, nombre, ruta)) = de_renglon(r) {
            v.push(Renglon {
                linea,
                clase,
                nombre,
                ruta,
            });
        }
    }
    v
}

/// Las fotos y los incrustados de la nota, por renglon: lo que se pinta
/// encima del texto (`imagenes::colocar` busca aqui el sitio de cada uno).
pub fn encima(texto: &str) -> Vec<(usize, String)> {
    let mut v: Vec<(usize, String)> = md_imagen::fotos(texto).into_iter().map(|(n, f)| (n, f.ruta)).collect();
    v.extend(renglones(texto).into_iter().map(|r| (r.linea, r.ruta)));
    v.sort_by_key(|(n, _)| *n);
    v
}

// ---------------------------------------------------------------------------
// Escribir

/// Sin lo que romperia el `[…]`: corchetes y saltos de renglon.
fn texto_limpio(s: &str) -> String {
    s.chars()
        .map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c })
        .filter(|c| !matches!(c, '[' | ']'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// El renglon de un documento (o de un audio) ya junto a la nota.
pub fn renglon_de_archivo(nombre: &str, ruta: &str) -> String {
    let nombre = match texto_limpio(nombre) {
        n if n.is_empty() => ruta.rsplit(['/', '\\']).next().unwrap_or(ruta).to_string(),
        n => n,
    };
    format!("![{nombre}]({})", ruta.trim())
}

/// La direccion de un mensaje del chat.
pub fn direccion_de_mensaje(proyecto: &str, mensaje: &str) -> String {
    format!("{ESQUEMA_MENSAJE}{proyecto}/{mensaje}")
}

/// El proyecto y el mensaje de un enlace `pixpin:mensaje=<proyecto>/<codigo>`.
pub fn mensaje_del_enlace(url: &str) -> Option<(String, String)> {
    let resto = url.trim().strip_prefix(ESQUEMA_MENSAJE)?;
    let (p, m) = resto.split_once('/')?;
    let bien = |s: &str| !s.is_empty() && !s.contains(['/', '\\', ' ', ')', '(', '"']);
    (bien(p) && bien(m)).then(|| (p.to_string(), m.to_string()))
}

/// Lo mas largo que va el texto del enlace de un mensaje: lo que el movil
/// ensena de el (una frase), no el mensaje entero.
const LARGO_DEL_TEXTO: usize = 80;

/// El renglon de un mensaje del chat: su primera frase como texto del
/// enlace (lo que lee el movil) y sus codigos en la direccion.
pub fn renglon_de_mensaje(texto: &str, proyecto: &str, mensaje: &str) -> String {
    let primera = texto.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let mut t: String = texto_limpio(primera).chars().take(LARGO_DEL_TEXTO).collect();
    if texto_limpio(primera).chars().count() > LARGO_DEL_TEXTO {
        t.push('…');
    }
    if t.is_empty() {
        t = mensaje.to_string();
    }
    format!("[{t}]({})", direccion_de_mensaje(proyecto, mensaje))
}

/// **El bloque de un audio**: su renglon y, si la hay, su transcripcion
/// debajo tal como la escribe el movil (`[m:ss] texto`, un renglon en
/// blanco entre parrafos). Sin transcripcion, el renglon solo: el
/// reproductor ofrece «Pasar a texto».
pub fn bloque_de_audio(nombre: &str, ruta: &str, letra: Option<&str>) -> String {
    let renglon = renglon_de_archivo(nombre, ruta);
    match letra.map(normalizar_letra).filter(|l| !l.is_empty()) {
        Some(l) => format!("{renglon}\n\n{l}"),
        None => renglon,
    }
}

/// La transcripcion con sus parrafos separados por un renglon en blanco y
/// sin blancos de mas: lo que entra en la nota.
fn normalizar_letra(letra: &str) -> String {
    let mut parrafos: Vec<String> = Vec::new();
    let mut actual: Vec<&str> = Vec::new();
    let limpio = letra.replace("\r\n", "\n");
    for l in limpio.split('\n') {
        if l.trim().is_empty() {
            if !actual.is_empty() {
                parrafos.push(actual.join(" "));
                actual.clear();
            }
        } else {
            actual.push(l.trim());
        }
    }
    if !actual.is_empty() {
        parrafos.push(actual.join(" "));
    }
    parrafos.join("\n\n")
}

// ---------------------------------------------------------------------------
// Las marcas de tiempo

/// `1:23`, `12:05`, `1:02:09` (`Transcriptor.marcaDeTiempo` del movil).
pub fn marca_de_tiempo(ms: i64) -> String {
    let s = ms.max(0) / 1000;
    let (h, m, seg) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{seg:02}") } else { format!("{m}:{seg:02}") }
}

/// La velocidad como la escribe el reproductor del chat: `1×`, `1,5×`
/// (`pixpin_audio::reloj::velocidad_legible`; aqui sin arrastrar el crate
/// del audio, que la nota no toca).
pub fn velocidad_legible(v: f32) -> String {
    if (v - v.trunc()).abs() < 1e-4 {
        return format!("{}\u{d7}", v.trunc() as i64);
    }
    let mut t = format!("{v}").replace('.', ",");
    while t.ends_with('0') {
        t.pop();
    }
    format!("{t}\u{d7}")
}

/// La marca de tiempo con que empieza un renglon: el milisegundo y donde
/// estan `[` y `]` (posiciones UTF-16 dentro del renglon, la del `]`
/// incluida). `None` si no empieza por una.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Marca {
    pub ms: i64,
    /// El `[`.
    pub abre: usize,
    /// El `]`.
    pub cierra: usize,
}

pub fn marca(renglon: &str) -> Option<Marca> {
    let blancos = renglon.chars().take_while(|c| c.is_whitespace()).map(char::len_utf16).sum::<usize>();
    let t = renglon.trim_start();
    let resto = t.strip_prefix('[')?;
    let (dentro, _) = resto.split_once(']')?;
    let partes: Vec<&str> = dentro.split(':').collect();
    let num = |s: &str| (!s.is_empty() && s.len() <= 3 && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse::<i64>().ok()).flatten();
    let ms = match partes.as_slice() {
        [m, s] if s.len() == 2 => (num(m)? * 60 + num(s)?) * 1000,
        [h, m, s] if s.len() == 2 && m.len() == 2 => (num(h)? * 3600 + num(m)? * 60 + num(s)?) * 1000,
        _ => return None,
    };
    Some(Marca {
        ms,
        abre: blancos,
        cierra: blancos + 1 + dentro.encode_utf16().count(),
    })
}

/// Un audio de la nota y los parrafos de su transcripcion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Letra {
    /// El renglon del audio.
    pub linea: usize,
    pub ruta: String,
    /// Cada parrafo con marca: su renglon y su milisegundo.
    pub parrafos: Vec<(usize, i64)>,
}

/// **Cada audio con su transcripcion**: los parrafos con marca de tiempo
/// son del audio que tienen mas cerca por encima (el movil los manda todos
/// al primero de la nota; con uno solo es lo mismo, y con varios asi cada
/// transcripcion salta en el suyo). Los de antes de cualquier audio no son
/// de nadie.
pub fn letras(texto: &str) -> Vec<Letra> {
    let mut v: Vec<Letra> = Vec::new();
    let mut en_codigo = false;
    for (n, r) in texto.split(['\n', '\r']).enumerate() {
        if es_valla(r) {
            en_codigo = !en_codigo;
            continue;
        }
        if en_codigo {
            continue;
        }
        if let Some((Clase::Audio, _, ruta)) = de_renglon(r) {
            v.push(Letra {
                linea: n,
                ruta,
                parrafos: Vec::new(),
            });
        } else if let (Some(m), Some(l)) = (marca(r), v.last_mut()) {
            l.parrafos.push((n, m.ms));
        }
    }
    v
}

/// El parrafo por el que va el audio: el ultimo cuyo minuto ya paso
/// (`voz::por_donde_va`). `None` antes del primero.
pub fn parrafo_que_suena(parrafos: &[(usize, i64)], posicion_ms: i64) -> Option<usize> {
    parrafos.iter().rev().find(|(_, ms)| *ms <= posicion_ms).map(|(n, _)| *n)
}

// ---------------------------------------------------------------------------
// Lo que da la aplicacion

/// Un documento, como se pinta: su nombre y lo que se dice debajo.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Archivo {
    pub nombre: String,
    /// «1.2 MB · PDF»; o por que no se puede abrir.
    pub detalle: String,
    /// El fichero no esta en este equipo (aun no llego, o se borro).
    pub falta: bool,
}

/// Lo de dentro de una burbuja del chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Contenido {
    Texto(String),
    /// Una foto (su fichero en este equipo) con su texto, si lo lleva.
    Foto { ruta: PathBuf, pie: String },
    Archivo(Archivo),
    /// Una hoja: su miniatura si ya la hay, su nombre y su clase corta
    /// («2D», «MD», «PDF»), como la tira de Proyectos.
    Hoja { nombre: String, miniatura: Option<PathBuf>, clase: String },
    /// Una nota de voz en una burbuja: su duracion y lo que dice.
    Voz { duracion: String, texto: String },
}

/// Una burbuja del chat para pintar en la nota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Burbuja {
    /// La hora como la ensena el chat (`etiqueta_hora`).
    pub hora: String,
    /// La chapa del codigo (`#47·K7Q2`), si el mensaje la tiene.
    pub codigo: Option<String>,
    pub contenido: Contenido,
}

/// Un audio, como se pinta su reproductor.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Audio {
    pub nombre: String,
    /// Lo que dura, si se sabe (el mensaje lo apunta al grabarse).
    pub duracion_ms: i64,
    pub falta: bool,
}

/// **Lo que la aplicacion dice de un renglon incrustado** para pintarlo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ficha {
    Archivo(Archivo),
    Burbuja(Burbuja),
    Audio(Audio),
    /// El mensaje o la hoja ya no estan: se dice, con lo que queda (el
    /// texto del enlace).
    Borrado(String),
}

/// Un mensaje del chat que se puede meter: su clave y como sale en el menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MensajeElegible {
    pub clave: String,
    pub rotulo: String,
}

/// Lo que se le pide al reproductor (el de la aplicacion: uno por hilo,
/// `audio.rs`). La ruta es la del Markdown.
#[derive(Debug, Clone, PartialEq)]
pub enum OrdenAudio {
    /// Solo preguntar como va (y que el reproductor de su latido).
    Mirar,
    Alternar(String),
    /// Ir a ese milisegundo (cargandolo y tocando si no era el que sonaba).
    Saltar(String, i64),
    /// Ir a ese punto de la barra, de 0 a 1.
    IrA(String, f32),
    /// La siguiente velocidad.
    Velocidad,
}

/// Como va lo que suena.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EstadoAudio {
    /// La ruta (del Markdown) de lo cargado.
    pub ruta: String,
    pub sonando: bool,
    pub posicion_ms: i64,
    pub duracion_ms: i64,
    pub velocidad: f32,
}

/// Una transcripcion en marcha o recien acabada.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Transcribiendo {
    /// El audio (ruta del Markdown).
    pub ruta: String,
    /// De 0 a 1.
    pub avance: f32,
    /// Acabada: el texto con sus marcas, o por que no salio (ya redactado).
    pub hecho: Option<Result<String, String>>,
}

/// **Lo que la aplicacion pone para los incrustados.** Todo tiene un
/// «no hago nada» por defecto: sin aplicacion (las pruebas) los renglones
/// se quedan como texto y el menu no ofrece lo que no hay.
pub trait Medios {
    /// Lo que se pinta del renglon con esa ruta o direccion. `None` si no
    /// se sabe que es (se queda como texto).
    fn ficha(&mut self, ruta: &str) -> Option<Ficha>;
    /// Los mensajes del chat del proyecto de la nota, del mas nuevo al mas
    /// viejo, para el selector «Del chat».
    fn mensajes(&mut self) -> Vec<MensajeElegible> {
        Vec::new()
    }
    /// Lo que se escribe para el mensaje `clave` (su burbuja, o su audio
    /// con la transcripcion).
    fn insertar_mensaje(&mut self, _clave: &str) -> Option<String> {
        None
    }
    fn audio(&mut self, _orden: OrdenAudio) -> Option<EstadoAudio> {
        None
    }
    /// Empieza a pasar a texto el audio de esa ruta. `Err` con el aviso ya
    /// redactado si no se puede.
    fn pasar_a_texto(&mut self, _ruta: &str) -> Result<(), String> {
        Err(String::new())
    }
    /// La transcripcion en marcha, si hay una.
    fn transcribiendo(&mut self) -> Option<Transcribiendo> {
        None
    }
}

/// Los textos de los incrustados.
#[derive(Debug, Clone, Default)]
pub struct RotulosIncrustados {
    /// «Documento», «Del chat», «Audio»: las entradas de los menus.
    pub documento: String,
    pub del_chat: String,
    pub audio: String,
    pub abrir: String,
    pub quitar: String,
    pub ir_al_mensaje: String,
    /// «Mensaje borrado»: el de un enlace cuyo mensaje ya no esta.
    pub borrado: String,
    /// «No está en este equipo».
    pub falta: String,
    pub pasar_a_texto: String,
    /// «Pasando a texto { $pct } %» con `{pct}` dentro.
    pub pasando: String,
    /// El aviso de que el chat no tiene mensajes que meter.
    pub sin_mensajes: String,
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const PDF: &str = "pixpin:files/guardados/pc/c1/notas/17-Plano.pdf";
    const VOZ: &str = "pixpin:files/guardados/pc/c1/voz/voz-9.m4a";

    #[test]
    fn un_documento_solo_en_su_renglon_es_un_archivo_y_una_foto_no() {
        assert_eq!(
            de_renglon(&format!("![Plano.pdf]({PDF})")),
            Some((Clase::Archivo, "Plano.pdf".into(), PDF.into()))
        );
        assert_eq!(de_renglon(&format!("  ![nota]({VOZ})  ")).map(|x| x.0), Some(Clase::Audio));
        // Casos negativos: una foto es de `md_imagen`, y un documento en
        // mitad de una frase sigue siendo texto.
        assert_eq!(de_renglon("![obra](pixpin:files/a/obra.jpg)"), None);
        assert_eq!(de_renglon(&format!("mira ![Plano.pdf]({PDF})")), None);
        assert_eq!(de_renglon("![x]()"), None);
        assert_eq!(de_renglon("![x](a(b).pdf)"), None);
    }

    #[test]
    fn un_mensaje_y_una_hoja_enlazados_solos_en_su_renglon_son_incrustados() {
        let m = format!("[Llamar al aparejador]({})", direccion_de_mensaje("p1", "K7Q2ABCDEF"));
        assert_eq!(de_renglon(&m).map(|x| x.0), Some(Clase::Mensaje));
        let h = md_imagen::enlace_a_hoja("Planta", "p1", "H1");
        assert_eq!(de_renglon(&h).map(|x| x.0), Some(Clase::Hoja));
        // Casos negativos: un enlace a una pagina web, uno roto y uno en una
        // frase no se pintan como burbuja.
        assert_eq!(de_renglon("[web](https://x.es)"), None);
        assert_eq!(de_renglon("[x](pixpin:mensaje=p1)"), None);
        assert_eq!(de_renglon(&format!("ver {m}")), None);
    }

    #[test]
    fn los_renglones_dentro_de_un_bloque_de_codigo_no_se_pintan() {
        let t = format!("# Obra\n![Plano.pdf]({PDF})\n```\n![Otro.pdf]({PDF})\n```\n[x]({})", direccion_de_mensaje("p", "m"));
        let r = renglones(&t);
        assert_eq!(r.iter().map(|r| (r.linea, r.clase)).collect::<Vec<_>>(), vec![(1, Clase::Archivo), (5, Clase::Mensaje)]);
    }

    #[test]
    fn lo_que_va_encima_junta_fotos_e_incrustados_por_renglon() {
        let t = format!("![Plano.pdf]({PDF})\n![obra](obra.png)\ntexto");
        assert_eq!(encima(&t), vec![(0, PDF.to_string()), (1, "obra.png".to_string())]);
        assert!(encima("solo texto").is_empty());
    }

    #[test]
    fn el_enlace_de_un_mensaje_va_y_vuelve_con_sus_codigos() {
        let d = direccion_de_mensaje("PRJ1234567", "MSG7654321");
        assert_eq!(mensaje_del_enlace(&d), Some(("PRJ1234567".into(), "MSG7654321".into())));
        // Casos negativos: sin mensaje, con un espacio o con un parentesis
        // (el movil cortaria la direccion ahi).
        assert_eq!(mensaje_del_enlace("pixpin:mensaje=PRJ/"), None);
        assert_eq!(mensaje_del_enlace("pixpin:mensaje=P R/M"), None);
        assert_eq!(mensaje_del_enlace("pixpin:mensaje=P/M)"), None);
        assert_eq!(mensaje_del_enlace("pixpin:hoja=P/M"), None);
    }

    #[test]
    fn el_texto_de_un_mensaje_enlazado_es_su_primera_frase_sin_corchetes() {
        let r = renglon_de_mensaje("\n  Pedir [urgente] la grua\nsegunda linea", "p", "m");
        assert_eq!(r, "[Pedir urgente la grua](pixpin:mensaje=p/m)");
        assert_eq!(de_renglon(&r).map(|x| x.1), Some("Pedir urgente la grua".to_string()));
        // Un mensaje sin texto (una foto suelta) se nombra por su codigo.
        assert_eq!(renglon_de_mensaje("", "p", "m"), "[m](pixpin:mensaje=p/m)");
        let largo = renglon_de_mensaje(&"a".repeat(200), "p", "m");
        assert!(largo.starts_with(&format!("[{}…]", "a".repeat(80))));
    }

    #[test]
    fn el_bloque_de_un_audio_lleva_su_transcripcion_como_la_escribe_el_movil() {
        let b = bloque_de_audio("Nota de voz", VOZ, Some("[0:00] llamar al aparejador\n\n\n[0:21] y pedir\nel presupuesto\n"));
        assert_eq!(b, format!("![Nota de voz]({VOZ})\n\n[0:00] llamar al aparejador\n\n[0:21] y pedir el presupuesto"));
        // Y vuelve: el audio y sus dos parrafos con su minuto.
        let l = letras(&b);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].parrafos, vec![(2, 0), (4, 21_000)]);
        // Sin transcripcion, el renglon solo.
        assert_eq!(bloque_de_audio("Nota", VOZ, Some("  \n ")), format!("![Nota]({VOZ})"));
        assert_eq!(bloque_de_audio("Nota", VOZ, None), format!("![Nota]({VOZ})"));
    }

    #[test]
    fn la_marca_de_tiempo_se_lee_con_sus_posiciones_y_se_escribe_igual() {
        assert_eq!(marca("[1:23] hola"), Some(Marca { ms: 83_000, abre: 0, cierra: 5 }));
        assert_eq!(marca("  [1:02:09] x").map(|m| (m.ms, m.abre, m.cierra)), Some((3_729_000, 2, 10)));
        assert_eq!(marca_de_tiempo(83_000), "1:23");
        assert_eq!(marca_de_tiempo(3_729_000), "1:02:09");
        // Casos negativos: una casilla, un enlace y un numero suelto no son
        // marcas.
        assert_eq!(marca("[ ] tarea"), None);
        assert_eq!(marca("[x](y)"), None);
        assert_eq!(marca("[12] nota"), None);
        assert_eq!(marca("[1:2] corto"), None);
        assert_eq!(marca("texto [1:23]"), None);
    }

    #[test]
    fn cada_transcripcion_salta_en_el_audio_que_tiene_encima() {
        let t = format!("[0:05] antes de todo\n![a]({VOZ})\n[0:00] uno\n\n[0:10] dos\n![b](b.mp3)\n[0:03] tres");
        let l = letras(&t);
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].linea, l[0].parrafos.clone()), (1, vec![(2, 0), (4, 10_000)]));
        assert_eq!((l[1].ruta.as_str(), l[1].parrafos.clone()), ("b.mp3", vec![(6, 3_000)]));
        // El parrafo que suena: el ultimo cuyo minuto ya paso.
        assert_eq!(parrafo_que_suena(&l[0].parrafos, 12_000), Some(4));
        assert_eq!(parrafo_que_suena(&l[0].parrafos, 9_999), Some(2));
        // Caso negativo: antes del primer minuto no suena ninguno.
        assert_eq!(parrafo_que_suena(&[(3, 5_000)], 1_000), None);
    }

    #[test]
    fn el_teclado_trata_como_un_bloque_lo_mismo_que_se_pinta() {
        use pixpin_docs::md_edicion::{self, Renglon};
        let h = md_imagen::enlace_a_hoja("Planta", "p1", "H1");
        let m = format!("[x]({})", direccion_de_mensaje("p", "m"));
        let pdf = format!("![Plano.pdf]({PDF})");
        let voz = format!("![a]({VOZ})");
        for r in [pdf.as_str(), voz.as_str(), m.as_str(), h.as_str(), "[web](https://x.es)", "texto", "![x]()", "mira ![a](b.pdf)"] {
            assert_eq!(md_edicion::es_incrustado(r), de_renglon(r).is_some(), "{r}");
        }
        // Escribir en su renglon abre uno nuevo y Retroceso detras lo quita
        // entero, como con una foto.
        assert_eq!(md_edicion::renglones(&format!("{pdf}\ntexto")), vec![Renglon::Bloque, Renglon::Texto(0)]);
    }

    #[test]
    fn la_extension_decide_si_es_audio_como_en_el_movil() {
        for r in ["a.mp3", "x/y.M4A", "voz.ogg?v=2", "z.opus"] {
            assert!(es_audio(r), "{r}");
        }
        for r in ["a.pdf", "a.mp4", "sin-extension", "carpeta.m4a/x"] {
            assert!(!es_audio(r), "{r}");
        }
    }
}
