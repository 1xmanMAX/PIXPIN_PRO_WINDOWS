//! **Las herramientas de la app que hace el ejecutor del chat** (15-oct-2026,
//! «¿que mas herramientas le podemos dar al asistente para que la gente
//! pueda hacer TODO con PixPin a traves de el?»).
//!
//! Son filas de la misma tabla que las demas (`pixpin_ia::acciones`, con
//! `accion: EN_EL_CHAT`): mismo esquema, misma validacion, mismas claves
//! para elegirlas. Lo distinto es que **le devuelven al modelo lo que
//! salio** (el texto de un audio, las celdas de un Excel, lo hecho hoy) y
//! que eligen el archivo entre los adjuntos del chat, como las de
//! documentos (`documentos_acciones`):
//!
//! - `leer_en_voz_alta`: un texto, o el de un documento, al pedido
//!   `leer_en_voz` (el lector de voz comun de la app).
//! - `comprimir_pdf`: una copia ligera con el compresor de siempre
//!   (`aligerar::copia_ligera`), que entra al chat con el pedido `chat`. El
//!   original no se toca.
//! - `transcribir_audio`: Whisper en este PC (`voz::pasar_a_texto`, el de
//!   «Pasar a texto» del chat). Una nota de voz ya pasada a texto se lee
//!   tal cual, al instante.
//! - `leer_excel`: las celdas de una hoja o un rango (`pixpin_hojas`), con
//!   su fila y su columna, para contestar, calcular o hacer una grafica.
//! - `resumen_de_actividad`: lo hecho hoy, esta semana o este mes en los
//!   chats, las capturas, el timeline y las tareas.
//!
//! Todo corre en el hilo del turno: la ventana sigue suelta.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::Duration;

use pixpin_ia::acciones::AccionApp;
use pixpin_proyecto::almacen;
use pixpin_proyecto::cuaderno::{Clase, Cuaderno};
use pixpin_store::Ubicacion;
use serde_json::{Value, json};

use super::acciones::Mandar;
use super::documentos_acciones::{Candidato, candidatos, puntuar};
use super::turno::{Adjunto, Aviso};

/// Lo que necesitan estas herramientas del chat que las pide.
pub struct Contexto<'a> {
    pub ubicacion: &'a Ubicacion,
    /// El chat desde el que se pide (su id); vacio: «Mensajes guardados».
    pub proyecto: &'a str,
    pub adjuntos: &'a [Adjunto],
    pub mandar: &'a Mandar,
    pub parar: &'a AtomicBool,
    pub idioma: pixpin_store::Idioma,
    /// Al chat: el texto entero de lo transcrito.
    pub tx: Option<&'a Sender<Aviso>>,
}

impl Contexto<'_> {
    fn raiz(&self) -> &Path {
        self.ubicacion.raiz()
    }
}

/// **Hace una herramienta de esta tabla.** `Ok` lleva lo que se le cuenta al
/// modelo; `Err`, el motivo.
pub fn hacer(a: &AccionApp, cx: &Contexto) -> Result<String, String> {
    let args = &a.pedido;
    match a.herramienta {
        "leer_en_voz_alta" => leer_en_voz_alta(args, cx),
        "comprimir_pdf" => comprimir_pdf(args, cx),
        "transcribir_audio" => transcribir_audio(args, cx),
        "leer_excel" => leer_excel(args, cx),
        "resumen_de_actividad" => resumen_de_actividad(args, cx),
        "leer_documento" => leer_documento(args, cx),
        otra => Err(format!("la herramienta «{otra}» no la hace el chat")),
    }
}

fn texto_de<'a>(args: &'a Value, campo: &str) -> Option<&'a str> {
    args.get(campo).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty())
}

fn mandar(cx: &Contexto, pedido: Value) -> Result<(), String> {
    match (cx.mandar)(&pedido.to_string()) {
        pixpin_shell::mensajero::respuesta::ACEPTADO => Ok(()),
        _ => Err("PixPin no acepto el pedido".into()),
    }
}

// ---------------------------------------------------------------------------
// Que archivo

/// Que clase de archivo sirve a cada herramienta (por su extension).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clase2 {
    /// Lo que tiene texto que leer en voz alta.
    ConTexto,
    Pdf,
    Audio,
    Excel,
    /// Lo que lee `leer_documento`: lo de [`Clase2::ConTexto`] y las fotos
    /// (por el OCR).
    Legible,
}

impl Clase2 {
    pub fn sirve(self, nombre: &str) -> bool {
        let e = pixpin_docs::extension(nombre);
        match self {
            Clase2::ConTexto => matches!(e.as_str(), "pdf" | "docx" | "pptx" | "xlsx" | "xlsm" | "md" | "txt" | "csv"),
            Clase2::Pdf => e == "pdf",
            Clase2::Audio => matches!(e.as_str(), "m4a" | "mp3" | "wav" | "ogg" | "opus" | "aac" | "flac" | "wma" | "amr"),
            Clase2::Excel => matches!(e.as_str(), "xlsx" | "xlsm" | "xls"),
            Clase2::Legible => Clase2::ConTexto.sirve(nombre) || super::ocr::es_imagen(Path::new(nombre)),
        }
    }

    fn que(self) -> &'static str {
        match self {
            Clase2::ConTexto => "un documento con texto (PDF, Word, PowerPoint, Excel o nota)",
            Clase2::Pdf => "un PDF",
            Clase2::Audio => "un audio",
            Clase2::Excel => "un Excel",
            Clase2::Legible => "un documento (PDF, Word, PowerPoint, Excel, nota o foto)",
        }
    }
}

/// Un archivo de un chat, con la transcripcion si es una nota de voz que ya
/// se paso a texto.
#[derive(Debug, Clone)]
struct Elegido {
    c: Candidato,
    transcripcion: Option<String>,
}

/// Los archivos de los chats que sirven (adjuntos y notas de voz), sin el
/// chat del asistente. Las notas de voz no son adjuntos para
/// `documentos_acciones::candidatos`; aqui si.
fn del_disco(raiz: &Path, clase: Clase2) -> Vec<Elegido> {
    if clase != Clase2::Audio {
        return candidatos(raiz)
            .into_iter()
            .filter(|c| clase.sirve(&c.nombre))
            .map(|c| Elegido { c, transcripcion: None })
            .collect();
    }
    let mut v = Vec::new();
    for ficha in almacen::Indice::leer(raiz).proyectos {
        if ficha.es_asistente() {
            continue;
        }
        let pnombre = if ficha.es_guardados() { "Mensajes guardados".to_string() } else { ficha.nombre.clone() };
        let Ok(c) = Cuaderno::leer_de(&almacen::carpeta(raiz, &ficha.id)) else { continue };
        for m in c.mensajes.iter().filter(|m| matches!(m.clase, Some(Clase::Voz) | Some(Clase::Archivo)) && !m.en_buzon) {
            let Some(ruta) = m.ruta.as_deref().and_then(|r| pixpin_proyecto::vista::ruta_real(raiz, &ficha.id, r)) else {
                continue;
            };
            let ruta: PathBuf = ruta.components().collect();
            let nombre = if m.nombre.trim().is_empty() { pixpin_docs::nombre(&ruta) } else { m.nombre.trim().to_string() };
            if !clase.sirve(&pixpin_docs::nombre(&ruta)) || !ruta.is_file() {
                continue;
            }
            let transcripcion = crate::voz::transcripcion_de(m).map(|t| t.to_string()).filter(|t| !t.trim().is_empty());
            v.push(Elegido {
                c: Candidato { nombre, ruta, proyecto: ficha.id.clone(), proyecto_nombre: pnombre.clone(), cuando: m.cuando },
                transcripcion,
            });
        }
    }
    v
}

/// **Elige el archivo**: el nombrado o descrito (si hay varios parecidos, el
/// `Err` los lista para que el modelo pregunte); sin nada, el adjunto mas
/// reciente de este turno y si no el mas reciente de este chat.
fn elegir(cx: &Contexto, archivo: Option<&str>, clase: Clase2) -> Result<Elegido, String> {
    let todos = del_disco(cx.raiz(), clase);
    let Some(descripcion) = archivo.map(str::trim).filter(|a| !a.is_empty()) else {
        if let Some(a) = cx.adjuntos.iter().rev().find(|a| clase.sirve(&a.nombre) && a.ruta.is_file()) {
            return Ok(todos.iter().find(|e| e.c.ruta == a.ruta).cloned().unwrap_or(Elegido {
                c: Candidato {
                    nombre: a.nombre.clone(),
                    ruta: a.ruta.clone(),
                    proyecto: cx.proyecto.to_string(),
                    proyecto_nombre: String::new(),
                    cuando: 0,
                },
                transcripcion: None,
            }));
        }
        return todos
            .into_iter()
            .filter(|e| e.c.proyecto == cx.proyecto)
            .max_by_key(|e| e.c.cuando)
            .ok_or_else(|| format!("en este chat no hay {}; pregunta al usuario cual (por su nombre)", clase.que()));
    };
    let como_ruta = PathBuf::from(descripcion.trim_matches(['"', '\'', '«', '»']));
    if como_ruta.is_absolute() && como_ruta.is_file() {
        let nombre = pixpin_docs::nombre(&como_ruta);
        if !clase.sirve(&nombre) {
            return Err(format!("«{nombre}» no es {}", clase.que()));
        }
        return Ok(Elegido {
            c: Candidato { nombre, ruta: como_ruta, proyecto: String::new(), proyecto_nombre: String::new(), cuando: 0 },
            transcripcion: None,
        });
    }
    let mut puntuados: Vec<(f64, Elegido)> = todos
        .into_iter()
        .map(|e| {
            let mut p = puntuar(&e.c, descripcion);
            if e.c.proyecto == cx.proyecto {
                p += 0.25;
            }
            (p, e)
        })
        .filter(|(p, _)| *p >= 1.5)
        .collect();
    puntuados.sort_by(|a, b| b.0.total_cmp(&a.0).then(b.1.c.cuando.cmp(&a.1.c.cuando)));
    puntuados.dedup_by(|a, b| a.1.c.ruta == b.1.c.ruta);
    let Some(mejor) = puntuados.first().map(|x| x.0) else {
        return Err(format!(
            "no hay {} en los chats que se llame o trate de «{descripcion}»; pregunta al usuario el nombre",
            clase.que()
        ));
    };
    let parecidos: Vec<&Elegido> = puntuados.iter().filter(|(p, _)| *p >= mejor - 0.3).map(|(_, e)| e).collect();
    if parecidos.len() > 1 {
        let lista: Vec<String> = parecidos
            .iter()
            .take(5)
            .enumerate()
            .map(|(i, e)| format!("{}) {} (chat {})", i + 1, e.c.nombre, e.c.proyecto_nombre))
            .collect();
        return Err(format!(
            "no hagas nada todavia; contesta al usuario EXACTAMENTE esto: «Hay {} que encajan: {}. ¿Cual?» y, cuando elija, vuelve a llamar con su nombre exacto",
            parecidos.len(),
            lista.join("; ")
        ));
    }
    Ok(puntuados.swap_remove(0).1)
}

// ---------------------------------------------------------------------------
// Leer en voz alta

fn leer_en_voz_alta(args: &Value, cx: &Contexto) -> Result<String, String> {
    if let Some(t) = texto_de(args, "texto") {
        mandar(cx, json!({"pixpin": 1, "accion": "leer_en_voz", "texto": t}))?;
        return Ok(format!("Leyendo en voz alta ({} palabras).", t.split_whitespace().count()));
    }
    let e = elegir(cx, texto_de(args, "archivo"), Clase2::ConTexto)?;
    mandar(
        cx,
        json!({"pixpin": 1, "accion": "leer_en_voz", "ruta": e.c.ruta, "titulo": pixpin_docs::sin_extension(&e.c.nombre)}),
    )?;
    Ok(format!("Leyendo en voz alta «{}». «Para de leer» la calla.", e.c.nombre))
}

// ---------------------------------------------------------------------------
// Comprimir un PDF

/// Donde se deja la copia ligera antes de que entre al chat.
fn carpeta_temporal() -> PathBuf {
    std::env::temp_dir().join("pixpin-asistente")
}

/// «12,3 MB», «850 KB».
pub fn tamano(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0)).replace('.', ",")
    } else {
        format!("{} KB", bytes.div_ceil(1024))
    }
}

fn comprimir_pdf(args: &Value, cx: &Contexto) -> Result<String, String> {
    let e = elegir(cx, texto_de(args, "archivo"), Clase2::Pdf)?;
    let dir = carpeta_temporal();
    std::fs::create_dir_all(&dir).map_err(|e| format!("sin sitio para la copia: {e}"))?;
    let base = pixpin_docs::sin_extension(&e.c.nombre);
    let salida = dir.join(format!("{base} (ligero).pdf"));
    let t = std::time::Instant::now();
    match crate::aligerar::copia_ligera(&e.c.ruta, &salida, cx.parar)? {
        crate::aligerar::Copia::SinMejora => Ok(format!(
            "«{}» ya esta ligero: comprimirlo no gana lo bastante, asi que no hice copia.",
            e.c.nombre
        )),
        crate::aligerar::Copia::Hecha { antes, despues } => {
            let menos = 100.0 - despues as f64 * 100.0 / antes.max(1) as f64;
            let frase = format!(
                "«{}» aligerado: de {} a {} ({:.0} % menos), en {:.1} s.",
                e.c.nombre,
                tamano(antes),
                tamano(despues),
                menos,
                t.elapsed().as_secs_f32()
            );
            let proyecto = if e.c.proyecto.is_empty() { cx.proyecto } else { &e.c.proyecto };
            let mut pedido = json!({"pixpin": 1, "accion": "chat", "texto": frase, "archivos": [salida]});
            if !proyecto.trim().is_empty() {
                pedido["proyecto"] = json!(proyecto);
            }
            mandar(cx, pedido)?;
            Ok(format!("{frase} La copia ligera esta en el chat; el original no se toco."))
        }
    }
}

// ---------------------------------------------------------------------------
// Pasar un audio a texto

/// Lo mas que se espera a Whisper (con la primera bajada del modelo).
const ESPERA_TRANSCRIBIR: Duration = Duration::from_secs(20 * 60);

fn transcribir_audio(args: &Value, cx: &Contexto) -> Result<String, String> {
    let e = elegir(cx, texto_de(args, "archivo"), Clase2::Audio)?;
    let de = Adjunto { nombre: e.c.nombre.clone(), ruta: e.c.ruta.clone() };
    // Ya pasada a texto en el chat: al instante.
    if let Some(t) = e.transcripcion {
        return Ok(format!(
            "«{}» (ya estaba pasada a texto):\n{}",
            e.c.nombre,
            super::ejecutor::recortar(&crate::voz::sin_marcas(&t), super::ejecutor::OCR_PARA_EL_MODELO)
        ));
    }
    let mut en_marcha = crate::voz::pasar_a_texto("asistente", &e.c.ruta, cx.ubicacion, cx.idioma, Vec::new(), None);
    let hasta = std::time::Instant::now() + ESPERA_TRANSCRIBIR;
    let hecha = loop {
        if cx.parar.load(Ordering::Relaxed) {
            en_marcha.cancelar();
            return Err("el usuario lo paro".into());
        }
        if let Some(r) = en_marcha.recoger() {
            break r.map_err(|e| format!("no se pudo pasar a texto: {e}"))?;
        }
        if std::time::Instant::now() >= hasta {
            en_marcha.cancelar();
            return Err("pasar a texto tardo demasiado".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let texto = crate::voz::sin_marcas(&hecha.texto);
    if texto.trim().is_empty() {
        return Ok(format!("En «{}» no se entiende nada que pasar a texto.", e.c.nombre));
    }
    if let Some(tx) = cx.tx {
        let _ = tx.send(Aviso::TextoLeido { de, texto: hecha.texto.clone() });
    }
    Ok(format!(
        "«{}» pasado a texto ({} s de audio):\n{}",
        e.c.nombre,
        hecha.duracion_ms / 1000,
        super::ejecutor::recortar(&texto, super::ejecutor::OCR_PARA_EL_MODELO)
    ))
}

// ---------------------------------------------------------------------------
// Leer un Excel

/// Lo mas que se le da al modelo de una hoja.
const FILAS_EXCEL: u32 = 300;
const COLUMNAS_EXCEL: u32 = 30;
const LETRAS_EXCEL: usize = 6000;

/// **Las celdas de una hoja** como tabla para el modelo: la primera linea,
/// las letras de las columnas; cada fila, su numero y sus valores separados
/// por tabuladores.
pub fn celdas_de_excel(ruta: &Path, hoja: Option<&str>, rango: Option<&str>) -> Result<String, String> {
    use pixpin_hojas::{Libro, Rango, Regional, TipoHoja, nombre_columna};
    let libro = Libro::abrir(ruta, Regional::default()).map_err(|e| format!("el Excel no se deja leer: {e}"))?;
    let hojas: Vec<(usize, &str)> = libro
        .hojas
        .iter()
        .enumerate()
        .filter(|(_, h)| h.tipo == TipoHoja::Hoja)
        .map(|(i, h)| (i, h.nombre.as_str()))
        .collect();
    let nombres = hojas.iter().map(|(_, n)| format!("«{n}»")).collect::<Vec<_>>().join(", ");
    let i = match hoja {
        Some(h) => {
            let q = pixpin_ia::acciones::normalizar(h.trim());
            hojas
                .iter()
                .find(|(_, n)| pixpin_ia::acciones::normalizar(n) == q)
                .or_else(|| hojas.iter().find(|(_, n)| pixpin_ia::acciones::normalizar(n).contains(&q)))
                .map(|(i, _)| *i)
                .ok_or_else(|| format!("no hay ninguna hoja «{h}»; las hojas son: {nombres}"))?
        }
        None => hojas
            .iter()
            .find(|(i, _)| *i == libro.hoja_activa)
            .or(hojas.first())
            .map(|(i, _)| *i)
            .ok_or("el Excel no tiene hojas de celdas")?,
    };
    let nombre_hoja = libro.hojas[i].nombre.clone();
    let mut h = libro.abrir_hoja(i);
    let pedido = match rango {
        Some(r) => Some(Rango::leer(r).ok_or_else(|| format!("«{r}» no es un rango (como A1:D20)"))?),
        None => None,
    };
    match pedido {
        Some(r) => {
            h.cargar_hasta_fila(r.fila1.saturating_add(1));
        }
        None => h.cargar_todo(),
    }
    // Lo usado: de la primera a la ultima fila y columna con algo.
    let filas: Vec<_> = h.filas_entre(0, u32::MAX).to_vec();
    let mut usado: Option<Rango> = None;
    for f in &filas {
        for c in h.celdas_de(f) {
            if libro.texto_crudo(&h, c).trim().is_empty() {
                continue;
            }
            let u = usado.get_or_insert(Rango { fila0: f.indice, col0: c.col, fila1: f.indice, col1: c.col });
            u.fila0 = u.fila0.min(f.indice);
            u.fila1 = u.fila1.max(f.indice);
            u.col0 = u.col0.min(c.col);
            u.col1 = u.col1.max(c.col);
        }
    }
    let Some(r) = pedido.or(usado) else {
        return Ok(format!("La hoja «{nombre_hoja}» esta vacia (hojas: {nombres})."));
    };
    let f1 = r.fila1.min(r.fila0 + FILAS_EXCEL - 1);
    let c1 = r.col1.min(r.col0 + COLUMNAS_EXCEL - 1);
    let mut out = format!(
        "Hoja «{nombre_hoja}» (hojas: {nombres}), {}{}:{}{}:\n",
        nombre_columna(r.col0),
        r.fila0 + 1,
        nombre_columna(r.col1),
        r.fila1 + 1
    );
    out.push_str("fila");
    for c in r.col0..=c1 {
        out.push('\t');
        out.push_str(&nombre_columna(c));
    }
    out.push('\n');
    let mut cortado = f1 < r.fila1 || c1 < r.col1;
    for f in filas.iter().filter(|f| f.indice >= r.fila0 && f.indice <= f1) {
        let mut linea = (f.indice + 1).to_string();
        let mut algo = false;
        let celdas = h.celdas_de(f);
        for c in r.col0..=c1 {
            linea.push('\t');
            if let Some(x) = celdas.iter().find(|x| x.col == c) {
                let t = libro.texto_crudo(&h, x);
                algo |= !t.trim().is_empty();
                linea.push_str(&t.replace(['\t', '\n', '\r'], " "));
            }
        }
        if !algo {
            continue;
        }
        if out.len() + linea.len() > LETRAS_EXCEL {
            cortado = true;
            break;
        }
        out.push_str(&linea);
        out.push('\n');
    }
    if cortado {
        out.push_str("[...hay mas: pide un rango mas pequeno para ver el resto]\n");
    }
    Ok(out)
}

fn leer_excel(args: &Value, cx: &Contexto) -> Result<String, String> {
    let e = elegir(cx, texto_de(args, "archivo"), Clase2::Excel)?;
    let tabla = celdas_de_excel(&e.c.ruta, texto_de(args, "hoja"), texto_de(args, "rango"))?;
    // Las cuentas, comprobadas aqui (16-oct): el modelo no las rehace, y si
    // el usuario pidio marcar, sabe que herramienta va.
    let cuentas = match pixpin_ia::revision_tabla::para_el_modelo(&filas_de_celdas(&tabla)) {
        Some(c) if c.contains("no las rehagas") => {
            format!("\n{c}\nSi el usuario pidio MARCAR o señalar los errores en el Excel, llama ahora a anotar_inteligente con modo «errores».")
        }
        Some(c) => format!("\n{c}"),
        None => String::new(),
    };
    Ok(format!("«{}» — {tabla}{cuentas}", e.c.nombre))
}

/// Las filas de lo que da [`celdas_de_excel`] (sin la linea de la hoja, la
/// de las letras ni el numero de fila).
pub fn filas_de_celdas(tabla: &str) -> Vec<Vec<String>> {
    tabla
        .lines()
        .skip(2)
        .filter(|l| !l.starts_with('['))
        .map(|l| l.split('\t').skip(1).map(str::to_string).collect())
        .collect()
}

// ---------------------------------------------------------------------------
// Leer un documento entero (banco de documentos, 15-oct)

/// Cuanto texto de un documento se le da al modelo de la nube (unas 6000
/// palabras: un informe o un capitulo enteros; lo demas, por paginas).
pub const LETRAS_DOCUMENTO: usize = 40_000;

/// Las paginas pedidas, «3», «2-5» o «1,3-4» (desde 1); vacio: todas.
pub fn rangos_de_paginas(t: Option<&str>) -> Vec<(u32, u32)> {
    let Some(t) = t else { return Vec::new() };
    t.split([',', ';', ' '])
        .filter_map(|p| {
            let p = p.trim();
            let (a, b) = p.split_once(['-', '–']).unwrap_or((p, p));
            let (a, b) = (a.trim().parse::<u32>().ok()?, b.trim().parse::<u32>().ok()?);
            (a >= 1 && b >= a).then_some((a, b))
        })
        .collect()
}

/// **El texto de un documento, pagina a pagina**: un PDF o una foto por el
/// OCR del asistente (la capa de texto si la hay, si no Windows.Media.Ocr);
/// lo demas con el lector de la busqueda y la voz
/// (`documentos::leer_fichero`): un solo lector por formato.
pub fn texto_del_documento(ruta: &Path, paginas: &[(u32, u32)], parar: &AtomicBool) -> Result<String, String> {
    if super::ocr::es_pdf(ruta) || super::ocr::es_imagen(ruta) {
        return super::ocr::leer(ruta, paginas, parar);
    }
    let unidades = super::documentos::leer_fichero(ruta)?;
    let dentro = |n: u32| paginas.is_empty() || paginas.iter().any(|&(a, b)| (a..=b).contains(&n));
    let que = match pixpin_docs::extension(&pixpin_docs::nombre(ruta)).as_str() {
        "pptx" | "pptm" | "ppsx" | "ppsm" | "potx" | "potm" => "Diapositiva",
        _ => "Hoja",
    };
    let mut s = String::new();
    for (i, u) in unidades.iter().enumerate() {
        let n = u.pagina.unwrap_or(i as u32 + 1);
        if !dentro(n) {
            continue;
        }
        match (&u.seccion, u.pagina) {
            // Un apartado del Word ya empieza por su titulo.
            (Some(sec), _) if u.texto.trim_start().starts_with(sec.as_str()) => {}
            (Some(sec), _) => s.push_str(&format!("--- {sec} ---\n")),
            (None, Some(p)) => s.push_str(&format!("--- {que} {p} ---\n")),
            (None, None) => {}
        }
        s.push_str(u.texto.trim());
        s.push_str("\n\n");
    }
    let s = s.trim_end().to_string();
    if s.is_empty() {
        return Err(if paginas.is_empty() { "el documento no tiene texto que leer".into() } else { "esas paginas no tienen texto (o no existen)".into() });
    }
    Ok(s)
}

fn leer_documento(args: &Value, cx: &Contexto) -> Result<String, String> {
    let e = elegir(cx, texto_de(args, "archivo"), Clase2::Legible)?;
    let paginas = rangos_de_paginas(texto_de(args, "paginas"));
    let texto = texto_del_documento(&e.c.ruta, &paginas, cx.parar)?;
    let letras = texto.chars().count();
    let mut s: String = texto.chars().take(LETRAS_DOCUMENTO).collect();
    if letras > LETRAS_DOCUMENTO {
        s.push_str("\n[...sigue: pide las paginas siguientes con «paginas»]");
    }
    Ok(format!("«{}» ({letras} letras):\n{s}", e.c.nombre))
}

/// **Los archivos de este chat, para el modelo de la nube** (banco de
/// documentos, 15-oct): sin esto no sabia que el usuario habia adjuntado el
/// comunicado («no veo ningun archivo, ¿me lo pegas?») o lanzaba una captura
/// para «pasa esta pagina al lienzo». Los mas recientes primero, como
/// mucho [`ARCHIVOS_EN_LA_NOTA`], cada uno con la fecha y hora en que se
/// subio, al minuto y en hora local (`desfase_ms`, 17-oct-2026: «que se le
/// pasen fechas de subida con alto detalle para que barra rapido»); `None`
/// si no hay ninguno.
pub fn nota_de_archivos(raiz: &Path, proyecto: &str, desfase_ms: i64) -> Option<String> {
    // Solo el cuaderno de este chat (un fichero): el turno no espera.
    let c = Cuaderno::leer_de(&almacen::carpeta(raiz, proyecto)).ok()?;
    let mut hay: Vec<(i64, String)> = c
        .mensajes
        .iter()
        .filter(|m| matches!(m.clase, Some(Clase::Archivo) | Some(Clase::Imagen)) && !m.en_buzon)
        .filter_map(|m| {
            let nombre = m.nombre.trim();
            let nombre = if nombre.is_empty() { m.ruta.as_deref()?.rsplit(['/', '\\']).next()?.to_string() } else { nombre.to_string() };
            Some((m.cuando, nombre))
        })
        .collect();
    if hay.is_empty() {
        return None;
    }
    hay.sort_by_key(|(cuando, _)| std::cmp::Reverse(*cuando));
    let nombres: Vec<String> = hay
        .iter()
        .take(ARCHIVOS_EN_LA_NOTA)
        .map(|(cuando, n)| match pixpin_ia::fechas::fecha_y_hora(*cuando, desfase_ms) {
            Some(f) => format!("«{n}» (subido el {f})"),
            None => format!("«{n}»"),
        })
        .collect();
    Some(format!(
        "\n\n(Archivos adjuntos en este chat, el mas reciente primero: {}. «Esto», «el documento» o «esta pagina» es el primero. \
         Para leerlo entero: leer_documento; un Excel: leer_excel; una pagina de PDF o una foto al lienzo: hoja_a_lienzo.)",
        nombres.join(", ")
    ))
}

/// Cuantos archivos nombra [`nota_de_archivos`]. Eran 6 (15-oct); con la
/// fecha cada uno ocupa ~15 tokens y la nube tiene sitio de sobra: 15 son
/// ~250 tokens, menos del 1 % de la ventana que se mide (`VENTANA_NUBE`).
pub const ARCHIVOS_EN_LA_NOTA: usize = 15;

// ---------------------------------------------------------------------------
// Lo hecho hoy

/// Lo contado en un periodo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Actividad {
    pub notas: usize,
    pub fotos: usize,
    pub archivos: usize,
    pub voz: usize,
    pub dibujos: usize,
    pub otros: usize,
    /// Los chats donde se escribio algo, del que mas al que menos.
    pub chats: Vec<(String, usize)>,
    pub capturas: usize,
    pub momentos: usize,
    pub tareas_nuevas: usize,
    pub tareas_nuevas_hechas: usize,
    pub pendientes: usize,
    pub vencidas: usize,
}

impl Actividad {
    pub fn mensajes(&self) -> usize {
        self.notas + self.fotos + self.archivos + self.voz + self.dibujos + self.otros
    }
}

/// El primer dia (local, en dias desde 1970) del periodo.
pub fn desde_el_dia(periodo: &str, hoy: i64) -> i64 {
    match periodo {
        // El 1-1-1970 fue jueves: lunes = 0.
        "semana" => hoy - (hoy + 3).rem_euclid(7),
        "mes" => {
            let (_, _, d) = pixpin_ia::fechas::fecha_de(hoy);
            hoy - (d - 1)
        }
        _ => hoy,
    }
}

/// **Cuenta lo hecho** desde `desde_utc_ms`: los mensajes de cada chat (sin
/// el del asistente), las capturas de la galeria, los momentos del timeline
/// y las tareas (`listas`, las de la ventana de tareas).
pub fn contar(raiz: &Path, desde_utc_ms: i64, desde_dia: i64, hoy: pixpin_ia::fechas::Momento, listas: &[pixpin_ia::tareas::VistaLista]) -> Actividad {
    let mut a = Actividad::default();
    for ficha in almacen::Indice::leer(raiz).proyectos {
        if ficha.es_asistente() {
            continue;
        }
        let Ok(c) = Cuaderno::leer_de(&almacen::carpeta(raiz, &ficha.id)) else { continue };
        let mut n = 0;
        for m in c.mensajes.iter().filter(|m| m.cuando >= desde_utc_ms) {
            n += 1;
            match &m.clase {
                Some(Clase::Nota) | None => a.notas += 1,
                Some(Clase::Imagen) => a.fotos += 1,
                Some(Clase::Archivo) => a.archivos += 1,
                Some(Clase::Voz) => a.voz += 1,
                Some(Clase::Dibujo) | Some(Clase::Pagina) => a.dibujos += 1,
                _ => a.otros += 1,
            }
        }
        if n > 0 {
            let nombre = if ficha.es_guardados() { "Mensajes guardados".to_string() } else { ficha.nombre.clone() };
            a.chats.push((nombre, n));
        }
    }
    a.chats.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
    let desde = std::time::UNIX_EPOCH + Duration::from_millis(desde_utc_ms.max(0) as u64);
    a.capturas = crate::galeria_capturas::listar(&crate::galeria_capturas::carpeta_en(raiz))
        .iter()
        .filter(|e| e.cuando >= desde)
        .count();
    a.momentos = pixpin_timeline::almacen::Almacen::en(raiz)
        .leer()
        .iter()
        .filter(|m| m.cuando >= desde_utc_ms)
        .count();
    for l in listas {
        for t in &l.tareas {
            if t.creada.is_some_and(|d| d >= desde_dia) {
                a.tareas_nuevas += 1;
                a.tareas_nuevas_hechas += usize::from(t.hecha);
            }
            if !t.hecha {
                a.pendientes += 1;
                a.vencidas += usize::from(t.vence.is_some_and(|v| v < hoy));
            }
        }
    }
    a
}

/// La frase para el modelo (y para la burbuja del camino rapido).
pub fn frase(periodo: &str, a: &Actividad) -> String {
    let cuando = match periodo {
        "semana" => "Esta semana",
        "mes" => "Este mes",
        _ => "Hoy",
    };
    let plural = |n: usize, uno: &str, varios: &str| format!("{n} {}", if n == 1 { uno } else { varios });
    let mut partes = Vec::new();
    if a.mensajes() == 0 {
        partes.push("ningun mensaje en los chats".to_string());
    } else {
        let mut de = Vec::new();
        for (n, uno, varios) in [
            (a.notas, "nota", "notas"),
            (a.fotos, "foto", "fotos"),
            (a.archivos, "archivo", "archivos"),
            (a.voz, "nota de voz", "notas de voz"),
            (a.dibujos, "lienzo u hoja", "lienzos u hojas"),
            (a.otros, "otro", "otros"),
        ] {
            if n > 0 {
                de.push(plural(n, uno, varios));
            }
        }
        let chats: Vec<String> = a.chats.iter().take(4).map(|(c, n)| format!("{c} {n}")).collect();
        partes.push(format!(
            "{} en {} ({}): {}",
            plural(a.mensajes(), "mensaje", "mensajes"),
            plural(a.chats.len(), "chat", "chats"),
            chats.join(", "),
            de.join(", ")
        ));
    }
    partes.push(plural(a.capturas, "captura", "capturas"));
    partes.push(plural(a.momentos, "momento en el timeline", "momentos en el timeline"));
    partes.push(format!(
        "{} ({} ya {}); quedan {} pendientes{}",
        plural(a.tareas_nuevas, "tarea nueva", "tareas nuevas"),
        a.tareas_nuevas_hechas,
        if a.tareas_nuevas_hechas == 1 { "hecha" } else { "hechas" },
        a.pendientes,
        if a.vencidas > 0 { format!(", {} vencidas", a.vencidas) } else { String::new() }
    ));
    format!("{cuando}: {}.", partes.join("; "))
}

fn resumen_de_actividad(args: &Value, cx: &Contexto) -> Result<String, String> {
    use pixpin_ia::tareas::FuenteTareas;
    let periodo = texto_de(args, "periodo").unwrap_or("hoy");
    let local = pixpin_shell::entorno::ahora_local_ms();
    let desfase = local - pixpin_shell::entorno::ahora_utc_ms();
    let hoy = pixpin_ia::fechas::Momento::de_ms_locales(local);
    let dia = desde_el_dia(periodo, hoy.dia);
    let desde_utc = dia * 86_400_000 - desfase;
    let listas = super::acciones::FuenteDeLaApp::nueva(cx.raiz()).listas();
    Ok(frase(periodo, &contar(cx.raiz(), desde_utc, dia, hoy, &listas)))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cada_clase_sirve_a_lo_suyo() {
        assert!(Clase2::Pdf.sirve("Tesis.PDF") && !Clase2::Pdf.sirve("tesis.docx"));
        assert!(Clase2::Audio.sirve("reunion.m4a") && !Clase2::Audio.sirve("reunion.pdf"));
        assert!(Clase2::Excel.sirve("costos.xlsx") && !Clase2::Excel.sirve("costos.csv"));
        assert!(Clase2::ConTexto.sirve("nota.md") && !Clase2::ConTexto.sirve("plano.dwg"));
    }

    #[test]
    fn el_periodo_empieza_donde_toca() {
        // 15-oct-2026 es jueves.
        let hoy = 20_741;
        assert_eq!(pixpin_ia::fechas::fecha_de(hoy), (2026, 10, 15));
        assert_eq!(desde_el_dia("hoy", hoy), hoy);
        assert_eq!(pixpin_ia::fechas::fecha_de(desde_el_dia("semana", hoy)), (2026, 10, 12), "el lunes");
        assert_eq!(pixpin_ia::fechas::fecha_de(desde_el_dia("mes", hoy)), (2026, 10, 1));
    }

    #[test]
    fn la_frase_dice_lo_contado_y_nada_de_mas() {
        let a = Actividad {
            notas: 3,
            fotos: 1,
            chats: vec![("Obra".into(), 3), ("Tesis".into(), 1)],
            capturas: 2,
            momentos: 1,
            tareas_nuevas: 2,
            tareas_nuevas_hechas: 1,
            pendientes: 5,
            vencidas: 1,
            ..Default::default()
        };
        assert_eq!(
            frase("hoy", &a),
            "Hoy: 4 mensajes en 2 chats (Obra 3, Tesis 1): 3 notas, 1 foto; 2 capturas; 1 momento en el timeline; 2 tareas nuevas (1 ya hecha); quedan 5 pendientes, 1 vencidas."
        );
        let nada = frase("semana", &Actividad::default());
        assert!(nada.starts_with("Esta semana: ningun mensaje"), "{nada}");
    }

    fn filas(v: &[&[&str]]) -> Vec<Vec<String>> {
        v.iter().map(|f| f.iter().map(|c| c.to_string()).collect()).collect()
    }

    /// Un Excel de verdad (escrito por el mismo camino que la tabla
    /// escaneada) leido entero y por rango, y los errores con su motivo.
    #[test]
    fn un_excel_se_lee_con_su_fila_y_su_columna() {
        let ruta = super::super::documentos_acciones::escribir_excel(
            "Costos",
            &filas(&[&["Partida", "Costo"], &["Cemento", "1250"], &["Fierro", "3400"], &["Arena", "600"]]),
            true,
        )
        .unwrap();
        let t = std::time::Instant::now();
        let todo = celdas_de_excel(&ruta, None, None).unwrap();
        println!("leer el Excel entero: {:?}\n{todo}", t.elapsed());
        assert!(todo.contains("A1:B4"), "{todo}");
        assert!(todo.contains("fila\tA\tB\n1\tPartida\tCosto\n2\tCemento\t1250\n"), "{todo}");
        let parte = celdas_de_excel(&ruta, Some("costos"), Some("B2:B3")).unwrap();
        assert!(parte.contains("2\t1250\n3\t3400\n") && !parte.contains("Arena"), "{parte}");
        // Negativos: una hoja que no esta (dice cuales hay) y un rango roto.
        let e = celdas_de_excel(&ruta, Some("Ventas"), None).unwrap_err();
        assert!(e.contains("«Costos»"), "{e}");
        assert!(celdas_de_excel(&ruta, None, Some("ZZ")).is_err());
    }

    /// Un almacen de prueba con un chat, sus mensajes de hoy y un Excel y un
    /// texto adjuntos en el turno.
    fn mundo(nombre: &str) -> (PathBuf, almacen::Ficha) {
        let raiz = std::env::temp_dir().join(format!("pixpin-en-el-chat-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        let f = almacen::Ficha::nueva("Obra", 1, "PC01");
        almacen::Indice { proyectos: vec![f.clone()], ..Default::default() }.guardar(&raiz).unwrap();
        (raiz, f)
    }

    #[test]
    fn leer_en_voz_y_leer_excel_desde_el_chat() {
        use pixpin_ia::acciones::{por_nombre, validar};
        let (raiz, f) = mundo("voz");
        let u = Ubicacion::Portable { raiz: raiz.clone() };
        let xlsx = super::super::documentos_acciones::escribir_excel("Presupuesto", &filas(&[&["Partida", "Costo"], &["Cemento", "1250"]]), true).unwrap();
        let nota = raiz.join("informe de obra.md");
        std::fs::write(&nota, "# Informe\n\nSe vacio la losa.\n\nFaltan 5 bolsas.").unwrap();
        let adjuntos = vec![
            Adjunto { nombre: "Presupuesto.xlsx".into(), ruta: xlsx },
            Adjunto { nombre: "informe de obra.md".into(), ruta: nota.clone() },
        ];
        let mandados = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
        let m2 = mandados.clone();
        let mandar: Mandar = Box::new(move |j| {
            m2.lock().unwrap().push(serde_json::from_str(j).unwrap());
            1
        });
        let parar = AtomicBool::new(false);
        let cx = Contexto { ubicacion: &u, proyecto: &f.id, adjuntos: &adjuntos, mandar: &mandar, parar: &parar, idioma: pixpin_store::Idioma::Espanol, tx: None };
        let llamar = |h: &str, args: &str| hacer(&validar(por_nombre(h).unwrap(), &serde_json::from_str(args).unwrap()).unwrap(), &cx);
        // Sin decir cual: el adjunto con texto mas reciente.
        let r = llamar("leer_en_voz_alta", "{}").unwrap();
        assert!(r.contains("informe de obra.md"), "{r}");
        assert_eq!(mandados.lock().unwrap()[0]["accion"], "leer_en_voz");
        assert_eq!(mandados.lock().unwrap()[0]["ruta"], json!(nota));
        let r = llamar("leer_en_voz_alta", r#"{"texto":"Hola Max"}"#).unwrap();
        assert!(r.contains("2 palabras"), "{r}");
        assert_eq!(mandados.lock().unwrap()[1]["texto"], "Hola Max");
        // El pedido que sale lo entiende la app.
        for p in mandados.lock().unwrap().iter() {
            crate::pedidos::leer(&p.to_string()).unwrap();
        }
        let r = llamar("leer_excel", r#"{"rango":"A1:B2"}"#).unwrap();
        assert!(r.contains("Cemento\t1250"), "{r}");
        // Negativos: no hay ningun PDF ni ningun audio en el chat.
        assert!(llamar("comprimir_pdf", "{}").unwrap_err().contains("un PDF"));
        assert!(llamar("transcribir_audio", "{}").unwrap_err().contains("un audio"));
        assert!(llamar("leer_excel", r#"{"archivo":"cronograma de vaciados"}"#).is_err());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn leer_documento_da_el_texto_entero_por_paginas_y_la_nota_dice_los_adjuntos() {
        use pixpin_ia::acciones::{por_nombre, validar};
        let (raiz, f) = mundo("leer-documento");
        let u = Ubicacion::Portable { raiz: raiz.clone() };
        // Un Word de verdad, por el escritor de la app, adjunto al chat.
        let md = "# Informe de drenaje\n\nEl caudal de diseno es 0,85 m3/s.\n\n## Costo\n\nLa obra cuesta S/ 184 500.";
        let docx = pixpin_docs::exportar_docx::exportar(&pixpin_docs::exportar_docx::Nota {
            markdown: md,
            titulo: "",
            letra: Default::default(),
            comentarios: None,
            imagen: &|_| None,
            autor: "",
            ahora_ms: 0,
        })
        .unwrap();
        let rel = almacen::guardar_adjunto(&raiz, &f.id, "informe.docx", &docx).unwrap();
        let sello = pixpin_proyecto::cuaderno::Sello { cuando: 1_000, numero: 1, aparato: "PC01".into(), proyecto: f.id.clone() };
        let m = pixpin_proyecto::cuaderno::Mensaje::adjunto(Clase::Archivo, "informe.docx", &rel, docx.len() as i64, &sello);
        pixpin_proyecto::cuaderno::anadir(&almacen::carpeta(&raiz, &f.id), &m).unwrap();
        let mandar: Mandar = Box::new(|_| 1);
        let parar = AtomicBool::new(false);
        let cx = Contexto { ubicacion: &u, proyecto: &f.id, adjuntos: &[], mandar: &mandar, parar: &parar, idioma: pixpin_store::Idioma::Espanol, tx: None };
        let llamar = |args: &str| hacer(&validar(por_nombre("leer_documento").unwrap(), &serde_json::from_str(args).unwrap()).unwrap(), &cx);
        // Sin decir cual: el del chat, entero (los dos apartados).
        let r = llamar("{}").unwrap();
        assert!(r.contains("informe.docx") && r.contains("0,85") && r.contains("184 500"), "{r}");
        // La nota para la nube nombra el adjunto.
        let nota = nota_de_archivos(&raiz, &f.id, 0).unwrap();
        assert!(nota.contains("«informe.docx»") && nota.contains("leer_documento"), "{nota}");
        assert_eq!(rangos_de_paginas(Some("2-4, 7")), vec![(2, 4), (7, 7)]);
        assert!(rangos_de_paginas(Some("cero")).is_empty());
        // Negativos: un archivo que no esta; un chat sin adjuntos no tiene nota.
        assert!(llamar(r#"{"archivo":"cronograma de vaciados"}"#).is_err());
        let otro = almacen::Ficha::nueva("Vacio", 2, "PC01");
        assert!(nota_de_archivos(&raiz, &otro.id, 0).is_none());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_resumen_cuenta_lo_de_hoy_y_no_lo_de_antes() {
        let (raiz, f) = mundo("resumen");
        let carpeta = almacen::carpeta(&raiz, &f.id);
        std::fs::create_dir_all(&carpeta).unwrap();
        let ahora = 1_760_000_000_000_i64;
        for (i, (clase, cuando)) in [(Clase::Nota, ahora), (Clase::Imagen, ahora), (Clase::Nota, ahora - 3 * 86_400_000)].into_iter().enumerate() {
            let m = pixpin_proyecto::cuaderno::Mensaje {
                id: format!("m{i}"),
                clase: Some(clase),
                cuando,
                texto: "x".into(),
                ..Default::default()
            };
            pixpin_proyecto::cuaderno::anadir(&carpeta, &m).unwrap();
        }
        crate::pedidos::apuntar_en_timeline(&raiz, "Llego el camion", ahora).unwrap();
        let a = contar(&raiz, ahora - 1000, 0, pixpin_ia::fechas::Momento { dia: 0, minuto: 0 }, &[]);
        assert_eq!((a.notas, a.fotos, a.momentos), (1, 1, 1), "{a:?}");
        assert_eq!(a.chats, vec![("Obra".to_string(), 2)]);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// **Comprimir una muestra de verdad** (copia de `E:\pixpin-demo-om\Tesis.pdf`
    /// en el temporal), con su tiempo: `cargo test -p pixpin -- --ignored
    /// comprimir_la_tesis`.
    #[test]
    #[ignore = "comprime una muestra de E: (tarda)"]
    fn comprimir_la_tesis() {
        let _com = pixpin_shell::ComDelHilo::iniciar();
        let dir = std::env::temp_dir().join("pixpin-en-el-chat-tesis");
        std::fs::create_dir_all(&dir).unwrap();
        let copia = dir.join("Tesis.pdf");
        std::fs::copy(r"E:\pixpin-demo-om\Tesis.pdf", &copia).unwrap();
        let salida = dir.join("Tesis (ligero).pdf");
        let t = std::time::Instant::now();
        let r = crate::aligerar::copia_ligera(&copia, &salida, &AtomicBool::new(false)).unwrap();
        println!("comprimir la Tesis: {r:?} en {:?}", t.elapsed());
        if let crate::aligerar::Copia::Hecha { antes, despues } = r {
            assert!(despues < antes && salida.is_file());
        }
    }

    #[test]
    fn los_tamanos_se_leen_bien() {
        assert_eq!(tamano(850 * 1024), "850 KB");
        assert_eq!(tamano(12_900_000), "12,3 MB");
    }
}
