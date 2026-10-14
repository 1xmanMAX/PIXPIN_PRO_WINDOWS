//! **Las herramientas de documentos del asistente, del lado de la app**
//! (14-oct-2026, «integracion completa con la app»; la tabla de las
//! herramientas esta en `pixpin_ia::acciones_documentos`).
//!
//! - `tabla_de_imagen`: OCR con posiciones (`pixpin_ocr`, o la capa de
//!   texto del PDF si la tiene) → filas y columnas
//!   (`pixpin_ia::tabla_escaneada`) → el pedido `tabla_nueva` (la tabla del
//!   chat, que se abre en el pin de hoja y viaja al movil) o un `.xlsx`
//!   (`pixpin_proyecto::tabla_xlsx`) que entra al chat con el pedido `chat`.
//! - `anotar_documento`: busca el texto en las paginas (PDF, Word o
//!   PowerPoint maquetados como en el lector) o en los textos del plano, y
//!   escribe en **las mismas capas** que el lector (`DondeVa`, la
//!   `anot-<uid>-p<n>` con su `.hoja`) y el visor de planos (la
//!   `anot-<uid>`), con las figuras del motor comun (`pines::marcas_texto`
//!   para resaltar/subrayar/tachar, como las marcas de texto de los pines).
//!   Asi se ve al abrir y viaja al movil.
//! - `buscar_en_documento`: el pedido `abrir` con `buscar` (PDF/Office) o
//!   `zona` (plano), para que el visor lleve ahi.
//! - `abrir_por_nombre`: el archivo por su nombre o lo que trata, de los
//!   adjuntos de todos los chats; si hay varios, se le dicen al modelo para
//!   que pregunte con una lista corta. Nunca se le pide un codigo al usuario.
//! - `exportar_documento`: el componente «Exportar» comun
//!   (`compartir::preparar` + `generar`), y lo hecho entra al chat.
//!
//! **Latencia**: lo que se puede, sin red ni modelo; lo caro se guarda (los
//! textos de un plano, en la cache del plano). Todo corre en el hilo del
//! turno: la ventana del chat sigue suelta.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use pixpin_ia::acciones::{AccionApp, normalizar};
use pixpin_ia::acciones_documentos::{ABRIR, ANOTAR, BUSCAR_EN, EXPORTAR, TABLA};
use pixpin_ia::tabla_escaneada::{self, Palabra, TablaLeida};
use pixpin_motor2d::{ColorRgba, Elemento, Figura, Punto2};
use pixpin_pdf::texto::PaginaDeTexto;
use pixpin_proyecto::cuaderno::{Clase, Cuaderno};
use pixpin_proyecto::almacen;
use serde_json::{Value, json};

use super::acciones::Mandar;
use super::turno::Adjunto;

/// Lo que necesitan estas herramientas del chat que las pide.
pub struct Contexto<'a> {
    pub raiz: &'a Path,
    /// El chat desde el que se pide (su id); vacio: «Mensajes guardados».
    pub proyecto: &'a str,
    pub adjuntos: &'a [Adjunto],
    pub mandar: &'a Mandar,
    pub parar: &'a AtomicBool,
    pub idioma: pixpin_store::Idioma,
}

/// **Hace una herramienta de documentos.** `Ok` lleva lo que se le cuenta
/// al modelo; `Err`, el motivo (que el modelo dira o corregira).
pub fn hacer(a: &AccionApp, cx: &Contexto) -> Result<String, String> {
    let args = &a.pedido;
    match a.herramienta {
        TABLA => tabla(args, cx),
        ANOTAR => anotar(args, cx),
        BUSCAR_EN => buscar_en(args, cx),
        ABRIR => abrir(args, cx),
        EXPORTAR => exportar(args, cx),
        otra => Err(format!("la herramienta «{otra}» no es de documentos")),
    }
}

fn texto_de<'a>(args: &'a Value, campo: &str) -> Option<&'a str> {
    args.get(campo).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty())
}

fn numeros_de(args: &Value, campo: &str) -> Vec<f64> {
    args.get(campo)
        .and_then(Value::as_array)
        .map(|l| l.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default()
}

/// El `proyecto` de los pedidos: el chat de ahora (sin el, «Mensajes
/// guardados»).
fn proyecto_del_pedido(cx: &Contexto) -> Value {
    if cx.proyecto.trim().is_empty() { Value::Null } else { json!(cx.proyecto) }
}

fn mandar(cx: &Contexto, pedido: Value) -> Result<(), String> {
    match (cx.mandar)(&pedido.to_string()) {
        pixpin_shell::mensajero::respuesta::ACEPTADO => Ok(()),
        _ => Err("PixPin no acepto el pedido".into()),
    }
}

// ---------------------------------------------------------------------------
// Que archivo: por su nombre o lo que trata

/// Un archivo de un chat.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidato {
    pub nombre: String,
    pub ruta: PathBuf,
    pub proyecto: String,
    pub proyecto_nombre: String,
    pub cuando: i64,
}

/// Que clase de fichero sirve para una herramienta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sirve {
    /// PDF o imagen (tabla escaneada).
    Escaneado,
    /// PDF, Word, PowerPoint o plano (anotar, buscar dentro).
    Documento,
    /// Cualquiera (abrir, exportar).
    Cualquiera,
}

fn extension(nombre: &str) -> String {
    pixpin_docs::extension(nombre)
}

pub fn es_plano(nombre: &str) -> bool {
    crate::plano_cad::se_abre(nombre)
}

fn es_office_de_paginas(nombre: &str) -> bool {
    matches!(extension(nombre).as_str(), "docx" | "docm" | "dotx" | "dotm" | "pptx" | "pptm" | "ppsx" | "ppsm" | "potx" | "potm")
}

fn sirve(nombre: &str, s: Sirve) -> bool {
    let e = extension(nombre);
    match s {
        Sirve::Escaneado => e == "pdf" || super::ocr::es_imagen(Path::new(nombre)),
        Sirve::Documento => e == "pdf" || es_office_de_paginas(nombre) || es_plano(nombre),
        Sirve::Cualquiera => !e.is_empty(),
    }
}

/// **Los archivos de todos los chats** (los adjuntos que estan en este PC),
/// sin el chat del asistente.
pub fn candidatos(raiz: &Path) -> Vec<Candidato> {
    let mut v = Vec::new();
    for ficha in almacen::Indice::leer(raiz).proyectos {
        if ficha.es_asistente() {
            continue;
        }
        let pnombre = if ficha.es_guardados() { "Mensajes guardados".to_string() } else { ficha.nombre.clone() };
        let Ok(c) = Cuaderno::leer_de(&almacen::carpeta(raiz, &ficha.id)) else { continue };
        for m in c.mensajes.iter().filter(|m| m.clase == Some(Clase::Archivo) && !m.en_buzon) {
            let Some(ruta) = m.ruta.as_deref().and_then(|r| pixpin_proyecto::vista::ruta_real(raiz, &ficha.id, r)) else {
                continue;
            };
            let ruta: PathBuf = ruta.components().collect();
            if !ruta.is_file() {
                continue;
            }
            let nombre = if m.nombre.trim().is_empty() { pixpin_docs::nombre(&ruta) } else { m.nombre.trim().to_string() };
            v.push(Candidato { nombre, ruta, proyecto: ficha.id.clone(), proyecto_nombre: pnombre.clone(), cuando: m.cuando });
        }
    }
    v
}

/// Palabras que no dicen que archivo es.
const VACIAS: &[&str] = &[
    "de", "del", "la", "el", "los", "las", "en", "y", "mi", "mis", "que", "con", "por", "para", "un", "una", "abre",
    "abrir", "abreme", "archivo", "fichero", "documento", "muestrame", "ensename", "ese", "esa", "este", "esta", "al", "a", "lo",
];

fn palabras(s: &str) -> Vec<String> {
    normalizar(s)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

/// Lo que se parecen una palabra pedida y una del nombre: igual, o una
/// empieza por la otra («senal» y «senalizacion»).
fn parecido(q: &str, n: &str) -> f64 {
    if q == n {
        2.0
    } else if q.len().min(n.len()) >= 4 && (q.starts_with(n) || n.starts_with(q)) {
        1.5
    } else {
        0.0
    }
}

/// **Cuanto encaja un archivo con la descripcion.** Cada palabra suma lo
/// que mas se parezca en el nombre (o, a medias, en el nombre del chat); el
/// tipo dicho («plano», «pdf», «excel», «foto») suma si es el suyo.
pub fn puntuar(c: &Candidato, descripcion: &str) -> f64 {
    let nombre = palabras(&c.nombre);
    let chat = palabras(&c.proyecto_nombre);
    let ext = extension(&c.nombre);
    let mut s = 0.0;
    for q in palabras(descripcion) {
        let tipo = match q.as_str() {
            "plano" | "planos" | "dwg" | "dxf" | "cad" => Some(es_plano(&c.nombre)),
            "pdf" => Some(ext == "pdf"),
            "excel" | "xlsx" | "libro" => Some(matches!(ext.as_str(), "xlsx" | "xls" | "xlsm" | "csv")),
            "word" | "docx" => Some(matches!(ext.as_str(), "docx" | "doc")),
            "foto" | "imagen" | "captura" => Some(super::ocr::es_imagen(&c.ruta)),
            "presentacion" | "diapositivas" | "pptx" => Some(matches!(ext.as_str(), "pptx" | "ppt")),
            _ => None,
        };
        if let Some(es) = tipo {
            s += if es { 1.0 } else { -0.5 };
            // «plano» tambien puede estar escrito en el nombre.
        }
        if VACIAS.contains(&q.as_str()) {
            continue;
        }
        let en_nombre = nombre.iter().map(|n| parecido(&q, n)).fold(0.0, f64::max);
        let en_chat = chat.iter().map(|n| parecido(&q, n)).fold(0.0, f64::max) * 0.5;
        s += en_nombre.max(en_chat);
    }
    s
}

/// **Elige el archivo**: el nombrado o descrito; sin nada, el adjunto mas
/// reciente que sirva (del turno, luego del chat). Si hay varios que encajan
/// casi igual, el `Err` los lista para que el modelo pregunte cual.
pub fn elegir(cx: &Contexto, archivo: Option<&str>, s: Sirve) -> Result<Candidato, String> {
    let todos: Vec<Candidato> = candidatos(cx.raiz).into_iter().filter(|c| sirve(&c.nombre, s)).collect();
    let Some(descripcion) = archivo.map(str::trim).filter(|a| !a.is_empty()) else {
        // Lo adjunto en este turno, el ultimo.
        if let Some(a) = cx.adjuntos.iter().rev().find(|a| sirve(&a.nombre, s) && a.ruta.is_file()) {
            return Ok(todos.iter().find(|c| c.ruta == a.ruta).cloned().unwrap_or(Candidato {
                nombre: a.nombre.clone(),
                ruta: a.ruta.clone(),
                proyecto: cx.proyecto.to_string(),
                proyecto_nombre: String::new(),
                cuando: 0,
            }));
        }
        return todos
            .iter()
            .filter(|c| c.proyecto == cx.proyecto)
            .max_by_key(|c| c.cuando)
            .cloned()
            .ok_or_else(|| "en este chat no hay ningun archivo que sirva; pregunta al usuario cual (por su nombre)".into());
    };
    // Una ruta escrita tal cual.
    let como_ruta = PathBuf::from(descripcion.trim_matches(['"', '\'', '«', '»']));
    if como_ruta.is_absolute() && como_ruta.is_file() {
        return Ok(Candidato {
            nombre: pixpin_docs::nombre(&como_ruta),
            ruta: como_ruta,
            proyecto: String::new(),
            proyecto_nombre: String::new(),
            cuando: 0,
        });
    }
    // Lo adjunto en el turno cuenta como de este chat.
    let mut puntuados: Vec<(f64, Candidato)> = todos
        .into_iter()
        .map(|c| {
            let mut p = puntuar(&c, descripcion);
            if c.proyecto == cx.proyecto {
                p += 0.25;
            }
            (p, c)
        })
        .filter(|(p, _)| *p >= 1.5)
        .collect();
    // El mas nuevo antes a igual puntuacion; el mismo fichero, una vez.
    puntuados.sort_by(|a, b| b.0.total_cmp(&a.0).then(b.1.cuando.cmp(&a.1.cuando)));
    puntuados.dedup_by(|a, b| a.1.ruta == b.1.ruta);
    let Some((mejor, _)) = puntuados.first().cloned() else {
        return Err(format!(
            "no hay ningun archivo en los chats que se llame o trate de «{descripcion}»; pregunta al usuario el nombre"
        ));
    };
    let parecidos: Vec<&Candidato> = puntuados.iter().filter(|(p, _)| *p >= mejor - 0.3).map(|(_, c)| c).collect();
    if parecidos.len() > 1 {
        let lista: Vec<String> = parecidos
            .iter()
            .take(5)
            .enumerate()
            .map(|(i, c)| format!("{}) {} (chat {})", i + 1, c.nombre, c.proyecto_nombre))
            .collect();
        // La pregunta ya hecha: un modelo pequeno la copia tal cual (medido
        // 14-oct: con solo «pregunta cual», el 2B preguntaba sin la lista).
        return Err(format!(
            "no abras nada todavia; contesta al usuario EXACTAMENTE esto: «Hay {} que encajan: {}. ¿Cual?» y, cuando elija, vuelve a llamar con su nombre exacto",
            parecidos.len(),
            lista.join("; ")
        ));
    }
    Ok(puntuados.swap_remove(0).1)
}

// ---------------------------------------------------------------------------
// El texto de las paginas, con donde cae cada letra

/// El ancho de una hoja del lector (`pixpin_docs::vista::ANCHO_HOJA`): las
/// capas de un PDF o un Word se escriben en estas unidades.
const ANCHO_HOJA: f32 = pixpin_docs::vista::ANCHO_HOJA;

/// Las paginas de un PDF, Word o PowerPoint: su texto con la caja de cada
/// letra (fracciones de la hoja) y el alto de cada hoja en unidades del
/// lector. Un PDF escaneado pasa por el OCR (como mucho [`MAX_OCR`] hojas).
pub struct Paginas {
    pub textos: Vec<PaginaDeTexto>,
    pub altos: Vec<f32>,
}

const MAX_OCR: usize = 12;
/// El ancho al que se pinta una hoja para el OCR (~200 ppp en A4).
const ANCHO_OCR: u32 = 1700;

pub fn paginas(ruta: &Path, parar: &AtomicBool) -> Result<Paginas, String> {
    let nombre = pixpin_docs::nombre(ruta);
    if es_office_de_paginas(&nombre) {
        return paginas_de_office(ruta);
    }
    let doc = pixpin_pdf::Documento::abrir(ruta).map_err(|e| format!("no se pudo abrir el PDF: {e}"))?;
    let altos: Vec<f32> = doc.medidas().iter().map(|(w, h)| ANCHO_HOJA * h / w.max(1.0)).collect();
    let mut textos = pixpin_pdf::texto::de_fichero(ruta).unwrap_or_default();
    textos.resize_with(altos.len(), || PaginaDeTexto { texto: String::new(), cajas: Vec::new() });
    let mut leidas = 0;
    for (i, t) in textos.iter_mut().enumerate() {
        if t.texto.trim().chars().count() >= 20 || leidas >= MAX_OCR {
            continue;
        }
        if parar.load(Ordering::Relaxed) {
            return Err("el usuario lo paro".into());
        }
        leidas += 1;
        let img = doc.renderizar(i as u32, ANCHO_OCR).map_err(|e| e.to_string())?;
        *t = pagina_de_ocr(&img.pixeles, img.ancho, img.alto);
    }
    Ok(Paginas { textos, altos })
}

/// El texto de una imagen por el OCR, como una pagina: cada letra con la
/// caja de su palabra repartida a partes iguales.
fn pagina_de_ocr(rgba: &[u8], ancho: u32, alto: u32) -> PaginaDeTexto {
    let lineas = pixpin_ocr::reconocer(ancho, alto, rgba).unwrap_or_default();
    let (w, h) = (ancho.max(1) as f32, alto.max(1) as f32);
    let mut t = PaginaDeTexto { texto: String::new(), cajas: Vec::new() };
    for l in lineas {
        for p in &l.palabras {
            let n = p.texto.chars().count().max(1) as f32;
            let paso = p.caja.ancho as f32 / n;
            for (k, c) in p.texto.chars().enumerate() {
                let x0 = p.caja.x as f32 + paso * k as f32;
                t.texto.push(c);
                t.cajas.push(Some([x0 / w, p.caja.y as f32 / h, (x0 + paso) / w, (p.caja.y as f32 + p.caja.alto as f32) / h]));
            }
            t.texto.push(' ');
            t.cajas.push(None);
        }
        t.texto.push('\n');
        t.cajas.push(None);
    }
    t
}

/// Un Word o un PowerPoint maquetado como en el lector (mismas letras de
/// DirectWrite, mismas hojas).
fn paginas_de_office(ruta: &Path) -> Result<Paginas, String> {
    let mut doc = pixpin_office::Documento::abrir(ruta).map_err(|e| format!("no se pudo abrir: {e:?}"))?;
    let letras = pixpin_render::pagina_office::LetrasOffice::sin_motor().ok_or("sin DirectWrite")?;
    let mut textos = Vec::new();
    let mut altos = Vec::new();
    for i in 0..400 {
        let Some(p) = doc.pagina(i, &letras) else { break };
        altos.push(ANCHO_HOJA * p.alto / p.ancho.max(1.0));
        textos.push(crate::lector_pdf::office::texto_de_pagina(&p, &letras));
    }
    if textos.is_empty() {
        return Err("el documento no tiene paginas".into());
    }
    Ok(Paginas { textos, altos })
}

/// Un sitio encontrado: la hoja (desde 0) y las cajas de sus renglones en
/// unidades del lector (1400 de ancho).
#[derive(Debug, Clone, PartialEq)]
pub struct Sitio {
    pub pagina: usize,
    pub cajas: Vec<[f32; 4]>,
    /// Un trozo del texto de alrededor, para contarselo al modelo.
    pub contexto: String,
}

/// **Busca `consulta` en las paginas** (sin mirar mayusculas ni tildes,
/// como Ctrl+F). `pagina`: solo esa (desde 0).
pub fn buscar_en_paginas(p: &Paginas, consulta: &str, pagina: Option<usize>, max: usize) -> Vec<Sitio> {
    let mut v = Vec::new();
    for k in pixpin_docs::buscar::buscar(p.textos.iter().map(|t| t.texto.as_str()), consulta) {
        if pagina.is_some_and(|q| q != k.texto) {
            continue;
        }
        let t = &p.textos[k.texto];
        let alto = p.altos.get(k.texto).copied().unwrap_or(ANCHO_HOJA * 1.414);
        let cajas: Vec<[f32; 4]> = t
            .cajas_de(k.inicio, k.largo)
            .into_iter()
            .map(|c| [c[0] * ANCHO_HOJA, c[1] * alto, c[2] * ANCHO_HOJA, c[3] * alto])
            .collect();
        if cajas.is_empty() {
            continue;
        }
        let letras: Vec<char> = t.texto.chars().collect();
        let desde = k.inicio.saturating_sub(40);
        let hasta = (k.inicio + k.largo + 40).min(letras.len());
        let contexto: String = letras[desde..hasta].iter().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
        v.push(Sitio { pagina: k.texto, cajas, contexto });
        if v.len() >= max {
            break;
        }
    }
    v
}

// ---------------------------------------------------------------------------
// Las figuras: del motor comun

/// Como se anota.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComoAnotar {
    Resaltar,
    Subrayar,
    Tachar,
    Nota,
    Encerrar,
    Marco,
    Dibujar,
}

impl ComoAnotar {
    pub fn de(s: &str) -> Option<ComoAnotar> {
        Some(match s {
            "resaltar" => ComoAnotar::Resaltar,
            "subrayar" => ComoAnotar::Subrayar,
            "tachar" => ComoAnotar::Tachar,
            "nota" => ComoAnotar::Nota,
            "encerrar" => ComoAnotar::Encerrar,
            "marco" => ComoAnotar::Marco,
            "dibujar" => ComoAnotar::Dibujar,
            _ => return None,
        })
    }
    fn color_de_siempre(self) -> (u8, u8, u8) {
        match self {
            ComoAnotar::Resaltar => (0xff, 0xd4, 0x3b),
            ComoAnotar::Subrayar | ComoAnotar::Tachar | ComoAnotar::Encerrar | ComoAnotar::Dibujar => (0xe0, 0x31, 0x31),
            ComoAnotar::Marco => (0x3d, 0x8b, 0xff),
            ComoAnotar::Nota => (0x34, 0x3a, 0x40),
        }
    }
}

/// El grupo de lo que pone el asistente (para encontrarlo y borrarlo junto).
pub const GRUPO: &str = "asistente-anotado";

fn rgb(color: Option<&str>, como: ComoAnotar) -> (u8, u8, u8) {
    color
        .and_then(pixpin_ia::herramientas::color)
        .and_then(|c| {
            let h = c.0.trim_start_matches('#');
            let n = u32::from_str_radix(h.get(..6)?, 16).ok()?;
            Some(((n >> 16) as u8, (n >> 8) as u8, n as u8))
        })
        .unwrap_or_else(|| como.color_de_siempre())
}

fn color_motor((r, g, b): (u8, u8, u8)) -> ColorRgba {
    ColorRgba::opaco(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

/// La caja que abarca todas.
fn union(cajas: &[[f32; 4]]) -> [f32; 4] {
    cajas.iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |u, c| [u[0].min(c[0]), u[1].min(c[1]), u[2].max(c[2]), u[3].max(c[3])])
}

/// **Las figuras de una anotacion** sobre `cajas` (`[x0, y0, x1, y1]` en
/// las unidades de la capa, y hacia abajo): las mismas del lienzo y de las
/// marcas de texto de los pines. `letra`: el alto de letra de referencia
/// (el del texto marcado, o uno comodo para la hoja).
pub fn figuras(como: ComoAnotar, cajas: &[[f32; 4]], color: (u8, u8, u8), nota: Option<&str>, puntos: &[(f32, f32)], letra: f32) -> Vec<Elemento> {
    use pixpin_pin::Marca;
    let grupo = vec![GRUPO.to_string()];
    let c = color_motor(color);
    let grosor = (letra * 0.12).max(1.0);
    let marca = |m: Marca| {
        let rects: Vec<pixpin_geom::Rect> = cajas
            .iter()
            .map(|b| pixpin_geom::Rect {
                x: b[0].round() as i32,
                y: b[1].round() as i32,
                ancho: (b[2] - b[0]).round().max(1.0) as u32,
                alto: (b[3] - b[1]).round().max(1.0) as u32,
            })
            .collect();
        crate::pines::marcas_texto::elementos(m, color, &rects, GRUPO)
    };
    match como {
        ComoAnotar::Resaltar => marca(Marca::Resaltar),
        ComoAnotar::Subrayar => marca(Marca::Subrayar),
        ComoAnotar::Tachar => marca(Marca::Tachar),
        ComoAnotar::Encerrar | ComoAnotar::Marco => {
            if cajas.is_empty() {
                return Vec::new();
            }
            let u = union(cajas);
            let aire = letra * 0.6;
            vec![Elemento {
                figura: if como == ComoAnotar::Encerrar { Figura::Elipse } else { Figura::Rectangulo },
                x: u[0] - aire * 1.5,
                y: u[1] - aire,
                ancho: (u[2] - u[0]) + aire * 3.0,
                alto: (u[3] - u[1]) + aire * 2.0,
                trazo: c,
                grosor: grosor * 1.5,
                rugosidad: 0.0,
                grupos: grupo,
                ..Default::default()
            }]
        }
        ComoAnotar::Nota => {
            let texto = nota.unwrap_or("").trim();
            if texto.is_empty() {
                return Vec::new();
            }
            // A la derecha de lo marcado, o donde diga la zona.
            let u = if cajas.is_empty() { [0.0, 0.0, 0.0, letra] } else { union(cajas) };
            let tam = letra.max(1.0);
            let lineas = partir(texto, 28);
            let ancho = lineas.iter().map(|l| l.chars().count()).max().unwrap_or(1) as f32 * tam * 0.55 + tam;
            let alto = lineas.len() as f32 * tam * 1.3 + tam * 0.6;
            let (x, y) = (u[2] + tam * 0.8, u[1] - tam * 0.3);
            let papel = Elemento {
                figura: Figura::Rectangulo,
                x,
                y,
                ancho,
                alto,
                trazo: ColorRgba { a: 0.0, ..color_motor((0xe8, 0xc5, 0x47)) },
                relleno: Some(color_motor((0xff, 0xf3, 0xa3))),
                estilo_relleno: pixpin_motor2d::relleno::EstiloRelleno::Solido,
                grosor: 1.0,
                rugosidad: 0.0,
                grupos: grupo.clone(),
                ..Default::default()
            };
            let letras = Elemento {
                figura: Figura::Texto { texto: lineas.join("\n"), tam, familia: "sans".into() },
                x: x + tam * 0.5,
                y: y + tam * 0.3,
                ancho: ancho - tam,
                alto: alto - tam * 0.6,
                trazo: c,
                grosor: 1.0,
                rugosidad: 0.0,
                grupos: grupo,
                ..Default::default()
            };
            vec![papel, letras]
        }
        ComoAnotar::Dibujar => {
            if puntos.len() < 2 {
                return Vec::new();
            }
            let pts: Vec<Punto2> = puntos.iter().map(|(x, y)| Punto2::nuevo(*x, *y)).collect();
            let (x0, y0, x1, y1) = pts.iter().fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |a, p| (a.0.min(p.x), a.1.min(p.y), a.2.max(p.x), a.3.max(p.y)));
            vec![Elemento {
                figura: Figura::Linea { puntos: pts },
                x: x0,
                y: y0,
                ancho: x1 - x0,
                alto: y1 - y0,
                trazo: c,
                grosor: grosor * 1.5,
                rugosidad: 0.0,
                grupos: grupo,
                ..Default::default()
            }]
        }
    }
}

/// Parte un texto en renglones de unas `n` letras, por palabras.
fn partir(texto: &str, n: usize) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for parrafo in texto.lines() {
        let mut l = String::new();
        for w in parrafo.split_whitespace() {
            if !l.is_empty() && l.chars().count() + 1 + w.chars().count() > n {
                v.push(std::mem::take(&mut l));
            }
            if !l.is_empty() {
                l.push(' ');
            }
            l.push_str(w);
        }
        v.push(l);
    }
    v
}

/// Una zona `[x, y, ancho, alto]` en fracciones a una caja `[x0, y0, x1, y1]`
/// dentro de `marco` (las mismas unidades, y hacia abajo).
fn zona_en(zona: &[f64], marco: [f32; 4]) -> Option<[f32; 4]> {
    let [x, y, w, h] = <[f64; 4]>::try_from(zona.get(..4)?).ok()?;
    if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) || w <= 0.0 || h <= 0.0 {
        return None;
    }
    let (mw, mh) = (marco[2] - marco[0], marco[3] - marco[1]);
    Some([
        marco[0] + x as f32 * mw,
        marco[1] + y as f32 * mh,
        marco[0] + ((x + w).min(1.0)) as f32 * mw,
        marco[1] + ((y + h).min(1.0)) as f32 * mh,
    ])
}

fn puntos_en(puntos: &[f64], marco: [f32; 4]) -> Vec<(f32, f32)> {
    let (mw, mh) = (marco[2] - marco[0], marco[3] - marco[1]);
    puntos
        .chunks_exact(2)
        .map(|p| (marco[0] + p[0].clamp(0.0, 1.0) as f32 * mw, marco[1] + p[1].clamp(0.0, 1.0) as f32 * mh))
        .collect()
}

// ---------------------------------------------------------------------------
// 1. Tabla escaneada

/// Las palabras de una imagen o de la hoja `pagina` (desde 0; `None`: la
/// primera con texto) de un PDF: la capa de texto si la tiene, si no OCR.
pub fn palabras_de(ruta: &Path, pagina: Option<usize>) -> Result<(Vec<Palabra>, usize), String> {
    if super::ocr::es_imagen(ruta) {
        let img = pixpin_codec::imagen::cargar(ruta).map_err(|e| format!("no se pudo abrir la imagen: {e}"))?;
        return Ok((palabras_de_ocr(&img.pixeles, img.ancho, img.alto)?, 0));
    }
    // La capa de texto del PDF (exacta y al instante).
    if let Ok(trozos) = pixpin_pdf::texto::trozos_de_fichero(ruta) {
        let elegida = match pagina {
            Some(p) => Some(p),
            None => trozos.iter().position(|t| t.len() >= 4),
        };
        if let Some(p) = elegida
            && let Some(t) = trozos.get(p)
            && t.len() >= 4
        {
            let ps = t
                .iter()
                .map(|t| Palabra::nueva(&t.texto, t.x as f64, (t.y - t.alto * 0.8) as f64, (t.fin - t.x).max(0.0) as f64, t.alto as f64))
                .collect();
            return Ok((ps, p));
        }
    }
    // Escaneado: la hoja pintada, por el OCR.
    let doc = pixpin_pdf::Documento::abrir(ruta).map_err(|e| format!("no se pudo abrir el PDF: {e}"))?;
    let p = pagina.unwrap_or(0);
    if p as u32 >= doc.paginas() {
        return Err(format!("el PDF tiene {} paginas", doc.paginas()));
    }
    let img = doc.renderizar(p as u32, ANCHO_OCR).map_err(|e| e.to_string())?;
    Ok((palabras_de_ocr(&img.pixeles, img.ancho, img.alto)?, p))
}

fn palabras_de_ocr(rgba: &[u8], ancho: u32, alto: u32) -> Result<Vec<Palabra>, String> {
    if !pixpin_ocr::disponible() {
        return Err("el OCR de Windows no esta disponible (falta el idioma)".into());
    }
    let lineas = pixpin_ocr::reconocer(ancho, alto, rgba).map_err(|e| format!("el OCR fallo: {e:?}"))?;
    Ok(lineas
        .iter()
        .flat_map(|l| l.palabras.iter())
        .map(|p| Palabra::nueva(&p.texto, p.caja.x as f64, p.caja.y as f64, p.caja.ancho as f64, p.caja.alto as f64))
        .collect())
}

/// Lo que salio de una tabla, para el banco y para el modelo.
#[derive(Debug, Clone)]
pub struct TablaHecha {
    pub leida: TablaLeida,
    pub nombre: String,
    pub cabecera: bool,
    pub excel: Option<PathBuf>,
}

/// **Lee la tabla** del archivo (sin mandar nada).
pub fn leer_tabla(ruta: &Path, pagina: Option<usize>) -> Result<TablaLeida, String> {
    let (ps, p) = palabras_de(ruta, pagina)?;
    let t = tabla_escaneada::reconstruir(&ps);
    if t.filas.len() < 2 || t.columnas() < 2 {
        return Err(format!(
            "en la pagina {} no se ve una tabla (salieron {} filas y {} columnas)",
            p + 1,
            t.filas.len(),
            t.columnas()
        ));
    }
    Ok(t)
}

fn tabla(args: &Value, cx: &Contexto) -> Result<String, String> {
    let c = elegir(cx, texto_de(args, "archivo"), Sirve::Escaneado)?;
    let pagina = args.get("pagina").and_then(Value::as_u64).filter(|p| *p >= 1).map(|p| p as usize - 1);
    let leida = leer_tabla(&c.ruta, pagina)?;
    let cabecera = args.get("cabecera").and_then(Value::as_bool).unwrap_or_else(|| leida.parece_cabecera());
    let nombre = texto_de(args, "nombre").map(str::to_string).unwrap_or_else(|| pixpin_docs::sin_extension(&c.nombre));
    let excel = texto_de(args, "destino") == Some("excel");
    let hecha = if excel {
        let ruta = escribir_excel(&nombre, &leida.filas, cabecera)?;
        mandar(
            cx,
            json!({"pixpin": 1, "accion": "chat", "proyecto": proyecto_del_pedido(cx),
                   "texto": format!("[archivo 01] {nombre}.xlsx"), "archivos": [ruta]}),
        )?;
        TablaHecha { leida, nombre, cabecera, excel: Some(ruta) }
    } else {
        mandar(
            cx,
            json!({"pixpin": 1, "accion": "tabla_nueva", "proyecto": proyecto_del_pedido(cx), "nombre": nombre,
                   "filas": leida.filas, "cabecera": cabecera, "abrir": true}),
        )?;
        TablaHecha { leida, nombre, cabecera, excel: None }
    };
    Ok(resumen_de_tabla(&hecha, &c.nombre))
}

/// Lo que se le cuenta al modelo: el tamano y las primeras filas.
pub fn resumen_de_tabla(h: &TablaHecha, de: &str) -> String {
    let filas: Vec<String> = h.leida.filas.iter().take(4).map(|f| f.join(" | ")).collect();
    format!(
        "hecho: {} «{}» de «{}», {} filas x {} columnas{}{}. Primeras filas: {}",
        if h.excel.is_some() { "Excel" } else { "tabla del chat" },
        h.nombre,
        de,
        h.leida.filas.len(),
        h.leida.columnas(),
        if h.cabecera { ", con cabecera" } else { "" },
        if h.leida.combinadas.is_empty() {
            String::new()
        } else {
            format!(", {} celdas combinadas (aproximadas)", h.leida.combinadas.len())
        },
        filas.join(" / ")
    )
}

/// El `.xlsx` de la tabla, en una carpeta temporal (el pedido `chat` lo
/// copia al chat).
pub fn escribir_excel(nombre: &str, filas: &[Vec<String>], cabecera: bool) -> Result<PathBuf, String> {
    let t = crate::pedidos::tabla_de_filas(nombre, filas, cabecera, pixpin_shell::entorno::ahora_utc_ms());
    let bytes = pixpin_proyecto::tabla_xlsx::a_xlsx(&[pixpin_proyecto::tabla_xlsx::HojaParaVer { nombre, tabla: &t, valores: None }], 0)
        .map_err(|e| e.to_string())?;
    let carpeta = std::env::temp_dir().join("pixpin-asistente").join(format!("{}", pixpin_shell::entorno::ahora_utc_ms()));
    std::fs::create_dir_all(&carpeta).map_err(|e| e.to_string())?;
    let ruta = carpeta.join(format!("{}.xlsx", crate::compartir::nombre_de_fichero(nombre)));
    std::fs::write(&ruta, bytes).map_err(|e| e.to_string())?;
    Ok(ruta)
}

// ---------------------------------------------------------------------------
// 2. Anotar

/// Lo que quedo anotado (para el banco: donde y que).
#[derive(Debug, Clone, Default)]
pub struct Anotado {
    /// Hoja (desde 0) de un PDF/Word; `None` en un plano.
    pub pagina: Option<usize>,
    pub figuras: usize,
    /// Donde se marco y cuantas veces aparecia el texto en total.
    pub sitios: usize,
    pub hallados: usize,
    /// La vista del plano donde quedo (para abrirlo ahi).
    pub vista: Option<[f64; 3]>,
}

/// **Anota sin mandar nada** (el banco lo llama asi).
pub fn anotar_en(raiz: &Path, ruta: &Path, args: &Value, parar: &AtomicBool) -> Result<Anotado, String> {
    let como = texto_de(args, "tipo").and_then(ComoAnotar::de).ok_or("falta «tipo»")?;
    let texto = texto_de(args, "texto");
    let zona = numeros_de(args, "zona");
    let puntos = numeros_de(args, "puntos");
    let color = rgb(texto_de(args, "color"), como);
    let nota = texto_de(args, "nota").or(if como == ComoAnotar::Nota { texto } else { None });
    let pagina = args.get("pagina").and_then(Value::as_u64).filter(|p| *p >= 1).map(|p| p as usize - 1);
    if texto.is_none() && zona.len() < 4 && !(como == ComoAnotar::Dibujar && puntos.len() >= 4) && como != ComoAnotar::Nota {
        return Err("di sobre que texto del documento va («texto») o una «zona»".into());
    }
    if es_plano(&pixpin_docs::nombre(ruta)) {
        return anotar_plano(raiz, ruta, como, texto, &zona, &puntos, color, nota);
    }
    let p = paginas(ruta, parar)?;
    // Donde: el texto (todas sus apariciones en la hoja elegida o en la
    // primera que lo tenga), la zona, o arriba a la derecha para una nota.
    let (hoja, cajas, sitios, hallados) = match texto {
        Some(t) if como != ComoAnotar::Nota || zona.len() < 4 => {
            let s = buscar_en_paginas(&p, t, pagina, 40);
            let Some(primera) = s.first() else {
                return Err(format!("«{t}» no aparece en «{}»", pixpin_docs::nombre(ruta)));
            };
            let hoja = primera.pagina;
            let de_la_hoja: Vec<&Sitio> = s.iter().filter(|x| x.pagina == hoja).take(20).collect();
            // Encerrar o una nota van a lo primero; marcar el texto, a todo.
            let cajas: Vec<[f32; 4]> = match como {
                ComoAnotar::Resaltar | ComoAnotar::Subrayar | ComoAnotar::Tachar => de_la_hoja.iter().flat_map(|x| x.cajas.clone()).collect(),
                _ => primera.cajas.clone(),
            };
            let marcados = if matches!(como, ComoAnotar::Resaltar | ComoAnotar::Subrayar | ComoAnotar::Tachar) { de_la_hoja.len() } else { 1 };
            (hoja, cajas, marcados, s.len())
        }
        _ => {
            let hoja = pagina.unwrap_or(0).min(p.altos.len().saturating_sub(1));
            let marco = [0.0, 0.0, ANCHO_HOJA, p.altos.get(hoja).copied().unwrap_or(ANCHO_HOJA * 1.414)];
            let caja = zona_en(&zona, marco).unwrap_or([ANCHO_HOJA * 0.6, 60.0, ANCHO_HOJA * 0.62, 90.0]);
            (hoja, vec![caja], 1, 0)
        }
    };
    let alto = p.altos.get(hoja).copied().unwrap_or(ANCHO_HOJA * 1.414);
    let letra = cajas.first().map_or(18.0, |c| (c[3] - c[1]).clamp(10.0, 40.0));
    let pts = puntos_en(&puntos, [0.0, 0.0, ANCHO_HOJA, alto]);
    let nuevas = figuras(como, &cajas, color, nota, &pts, letra);
    if nuevas.is_empty() {
        return Err("no salio ninguna figura (¿una nota sin texto?)".into());
    }
    let n = nuevas.len();
    escribir_en_la_hoja(raiz, ruta, hoja, alto, nuevas)?;
    Ok(Anotado { pagina: Some(hoja), figuras: n, sitios, hallados, vista: None })
}

/// Escribe en la capa de la hoja `i` de un PDF o un Word: la MISMA que lee
/// y guarda el lector (`DondeVa`: la hoja de un PDF de proyecto, la
/// `anot-<uid>-p<i>` con su `.hoja` de un adjunto, o la de al lado).
pub fn escribir_en_la_hoja(raiz: &Path, ruta: &Path, i: usize, alto: f32, nuevas: Vec<Elemento>) -> Result<(), String> {
    let d = crate::lector_pdf_proyecto::DondeVa::de(raiz, ruta, pixpin_shell::entorno::ahora_utc_ms());
    let espacios = crate::anotado_del_adjunto::espacios_de_ahora(ruta);
    let mut capa = d.leer_capa(ruta, i, espacios, alto);
    for e in nuevas {
        capa.escena.anadir(e);
    }
    capa.sucia = true;
    d.guardar_capa(ruta, i, &mut capa, alto).map_err(|e| format!("no se pudo guardar lo anotado: {e}"))
}

#[allow(clippy::too_many_arguments)]
fn anotar_plano(
    raiz: &Path,
    ruta: &Path,
    como: ComoAnotar,
    texto: Option<&str>,
    zona: &[f64],
    puntos: &[f64],
    color: (u8, u8, u8),
    nota: Option<&str>,
) -> Result<Anotado, String> {
    let m = crate::plano_cad::modelo(raiz, ruta)?;
    let origen = m.origen;
    // Las cajas en coordenadas del plano (relativas a su origen, y arriba).
    let (cajas_plano, hallados, letra_plano): (Vec<[f64; 4]>, usize, f64) = match texto {
        Some(t) if como != ComoAnotar::Nota || zona.len() < 4 => {
            let ts = crate::plano_cad::textos::del_plano(raiz, ruta)?;
            let h = crate::plano_cad::textos::buscar(&ts, t, 10);
            if h.is_empty() {
                return Err(format!("«{t}» no aparece en el plano «{}»", pixpin_docs::nombre(ruta)));
            }
            let uno = matches!(como, ComoAnotar::Encerrar | ComoAnotar::Marco | ComoAnotar::Nota);
            let cajas = h
                .iter()
                .take(if uno { 1 } else { 10 })
                .map(|h| {
                    let largo = h.texto.chars().count().max(1) as f64 * h.alto * 0.75;
                    let (x, y) = (h.x - origen[0], h.y - origen[1]);
                    [x, y, x + largo, y + h.alto]
                })
                .collect();
            (cajas, h.len(), h[0].alto)
        }
        _ => {
            let c = m.caja;
            let (x0, y0, x1, y1) = (c[0] as f64, c[1] as f64, c[2] as f64, c[3] as f64);
            let caja = match zona.get(..4) {
                Some(&[x, y, w, h]) if w > 0.0 && h > 0.0 => {
                    let (ax, ay) = (x1 - x0, y1 - y0);
                    [x0 + x * ax, y1 - (y + h) * ay, x0 + (x + w) * ax, y1 - y * ay]
                }
                _ => [x1 - (x1 - x0) * 0.2, y1 - (y1 - y0) * 0.1, x1 - (x1 - x0) * 0.05, y1 - (y1 - y0) * 0.05],
            };
            let letra = ((caja[3] - caja[1]) * 0.2).max((y1 - y0) * 0.01);
            (vec![caja], 0, letra)
        }
    };
    let rutas = crate::plano_cad::anotar::rutas_de_la_capa(raiz, ruta);
    let caja_m = m.caja;
    let n = crate::plano_cad::anotar::anadir_figuras(&rutas, &m, |encaje| {
        if como == ComoAnotar::Marco {
            // El marco para imprimir de verdad, el del visor.
            let u = cajas_plano.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |u, c| [u[0].min(c[0]), u[1].min(c[1]), u[2].max(c[2]), u[3].max(c[3])]);
            let aire = letra_plano * 2.0;
            return vec![crate::plano_cad::anotar::marco_del_plano(&[u[0] - aire, u[1] - aire, u[2] + aire, u[3] + aire], encaje)];
        }
        // Del plano (y arriba) a la capa (y abajo): la esquina de arriba.
        let a_capa = |c: &[f64; 4]| {
            let a = encaje.a_capa([c[0], c[3]]);
            let b = encaje.a_capa([c[2], c[1]]);
            [a[0] as f32, a[1] as f32, b[0] as f32, b[1] as f32]
        };
        let cajas: Vec<[f32; 4]> = cajas_plano.iter().map(a_capa).collect();
        let letra = (letra_plano / encaje.u) as f32;
        let todo = a_capa(&[caja_m[0] as f64, caja_m[1] as f64, caja_m[2] as f64, caja_m[3] as f64]);
        let pts = puntos_en(puntos, todo);
        figuras(como, &cajas, color, nota, &pts, letra.max(1.0))
    })
    .map_err(|e| format!("no se pudo guardar lo anotado en el plano: {e}"))?;
    if n == 0 {
        return Err("no salio ninguna figura (¿una nota sin texto?)".into());
    }
    let c = cajas_plano[0];
    let ancho = ((c[2] - c[0]) * 6.0).max(letra_plano * 40.0);
    let vista = [(c[0] + c[2]) / 2.0 + origen[0], (c[1] + c[3]) / 2.0 + origen[1], ancho];
    Ok(Anotado { pagina: None, figuras: n, sitios: cajas_plano.len(), hallados, vista: Some(vista) })
}

fn anotar(args: &Value, cx: &Contexto) -> Result<String, String> {
    let c = elegir(cx, texto_de(args, "archivo"), Sirve::Documento)?;
    let hecho = anotar_en(cx.raiz, &c.ruta, args, cx.parar)?;
    // Se abre donde quedo, para que el usuario lo vea.
    let mut que = json!({"tipo": "fichero", "ruta": c.ruta});
    if let Some(p) = hecho.pagina {
        que["pagina"] = json!(p + 1);
    }
    if let Some(v) = hecho.vista {
        que["zona"] = json!(v);
    }
    mandar(cx, json!({"pixpin": 1, "accion": "abrir", "que": que}))?;
    Ok(format!(
        "hecho: {} en «{}»{}: marcado en {} sitio(s){} ({} figuras); ya esta abierto ahi y viaja al movil al sincronizar",
        texto_de(args, "tipo").unwrap_or("anotado"),
        c.nombre,
        hecho.pagina.map(|p| format!(", pagina {}", p + 1)).unwrap_or_default(),
        hecho.sitios,
        if hecho.hallados > hecho.sitios { format!(" (aparece {} veces en todo el documento; se marco lo de esta pagina o, al encerrar, lo primero)", hecho.hallados) } else { String::new() },
        hecho.figuras
    ))
}

// ---------------------------------------------------------------------------
// 3. Buscar dentro y llevar ahi

/// Lo encontrado, ya contado y con el pedido para abrir ahi.
#[derive(Debug, Clone)]
pub struct Encontrado {
    pub texto: String,
    pub pedido: Value,
    pub cuantos: usize,
}

/// **Busca sin mandar nada**: lo que se diria y el pedido `abrir`.
pub fn buscar_dentro(raiz: &Path, ruta: &Path, consulta: &str, parar: &AtomicBool) -> Result<Encontrado, String> {
    let nombre = pixpin_docs::nombre(ruta);
    if es_plano(&nombre) {
        let ts = crate::plano_cad::textos::del_plano(raiz, ruta)?;
        let h = crate::plano_cad::textos::buscar(&ts, consulta, 5);
        let Some(primero) = h.first() else {
            return Err(format!("«{consulta}» no aparece en el plano «{nombre}» ({} textos mirados)", ts.len()));
        };
        let vista = crate::plano_cad::textos::vista_de(primero);
        let lista: Vec<String> = h.iter().map(|h| format!("«{}» en ({:.1}, {:.1})", h.texto, h.x, h.y)).collect();
        return Ok(Encontrado {
            texto: format!("en «{nombre}»: {}", lista.join("; ")),
            pedido: json!({"pixpin": 1, "accion": "abrir", "que": {"tipo": "fichero", "ruta": ruta, "zona": vista}}),
            cuantos: h.len(),
        });
    }
    let p = paginas(ruta, parar)?;
    let s = buscar_en_paginas(&p, consulta, None, 8);
    let Some(primero) = s.first() else {
        return Err(format!("«{consulta}» no aparece en «{nombre}»"));
    };
    let lista: Vec<String> = s.iter().map(|x| format!("pagina {}: «…{}…»", x.pagina + 1, x.contexto)).collect();
    Ok(Encontrado {
        texto: format!("en «{nombre}»: {}", lista.join("; ")),
        pedido: json!({"pixpin": 1, "accion": "abrir", "que": {"tipo": "fichero", "ruta": ruta, "pagina": primero.pagina + 1, "buscar": consulta}}),
        cuantos: s.len(),
    })
}

fn buscar_en(args: &Value, cx: &Contexto) -> Result<String, String> {
    let consulta = texto_de(args, "texto").ok_or("falta «texto»")?;
    let c = elegir(cx, texto_de(args, "archivo"), Sirve::Documento)?;
    let e = buscar_dentro(cx.raiz, &c.ruta, consulta, cx.parar)?;
    let abrir = args.get("abrir").and_then(Value::as_bool).unwrap_or(true);
    if abrir {
        mandar(cx, e.pedido.clone())?;
    }
    Ok(format!("{} sitio(s) {}{}", e.cuantos, e.texto, if abrir { "; abierto en el primero" } else { "" }))
}

// ---------------------------------------------------------------------------
// 4. Abrir por su nombre

fn abrir(args: &Value, cx: &Contexto) -> Result<String, String> {
    let d = texto_de(args, "descripcion").ok_or("falta «descripcion»")?;
    let c = elegir(cx, Some(d), Sirve::Cualquiera)?;
    mandar(cx, json!({"pixpin": 1, "accion": "abrir", "que": {"tipo": "fichero", "ruta": c.ruta}}))?;
    Ok(format!("abierto «{}» (del chat {})", c.nombre, c.proyecto_nombre))
}

// ---------------------------------------------------------------------------
// 5. Exportar

/// Lo exportable de un fichero, por el componente comun.
fn cosa_de(raiz: &Path, ruta: &Path, idioma: pixpin_store::Idioma) -> Result<crate::compartir::Cosa, String> {
    use crate::compartir::{Cosa, exportable};
    let nombre = pixpin_docs::nombre(ruta);
    let titulo = pixpin_docs::sin_extension(&nombre);
    let e = extension(&nombre);
    Ok(if es_plano(&nombre) {
        let m = crate::plano_cad::modelo(raiz, ruta)?;
        let (cotas, marcos) = crate::plano_cad::anotar::cotas_y_marcos_de(&crate::plano_cad::anotar::rutas_de_la_capa(raiz, ruta), &m);
        let p = crate::plano_cad::exportar::Plano::nuevo(raiz, ruta, &m, &cotas, &marcos);
        Cosa::Visor(crate::plano_cad::exportar::exportable(p, idioma))
    } else if matches!(e.as_str(), "xlsx" | "xls" | "xlsm" | "csv" | "ods") {
        Cosa::Visor(exportable::de_libro(&titulo, ruta).map_err(|e| e.to_string())?)
    } else if super::ocr::es_imagen(ruta) {
        Cosa::Visor(exportable::de_imagen(&titulo, ruta, None).map_err(|e| e.to_string())?)
    } else {
        Cosa::Documento(ruta.to_path_buf())
    })
}

/// **Exporta sin mandar nada**: los ficheros hechos en `carpeta`.
pub fn exportar_a(raiz: &Path, ruta: &Path, formato: &str, carpeta: &Path, idioma: pixpin_store::Idioma) -> Result<Vec<PathBuf>, String> {
    let t = pixpin_store::Catalogo::nuevo(idioma);
    let p = crate::compartir::preparar(cosa_de(raiz, ruta, idioma)?, &t).map_err(|e| format!("no se pudo preparar: {e}"))?;
    let claves: Vec<String> = p.piezas.iter().map(|x| x.pagina.clave.clone()).collect();
    let salida = crate::compartir::generar(&p, formato, &claves, carpeta).map_err(|e| format!("no se pudo exportar a {formato}: {e}"))?;
    if salida.ficheros.is_empty() {
        return Err(format!("exportar a {formato} no dejo ningun fichero"));
    }
    Ok(salida.ficheros)
}

fn exportar(args: &Value, cx: &Contexto) -> Result<String, String> {
    let formato = texto_de(args, "formato").ok_or("falta «formato»")?;
    let c = elegir(cx, texto_de(args, "archivo"), Sirve::Cualquiera)?;
    let carpeta = std::env::temp_dir().join("pixpin-asistente").join(format!("exportar-{}", pixpin_shell::entorno::ahora_utc_ms()));
    let hechos = exportar_a(cx.raiz, &c.ruta, formato, &carpeta, cx.idioma)?;
    let fichas: String = (1..=hechos.len()).map(|i| format!("[archivo {i:02}]")).collect::<Vec<_>>().join(" ");
    mandar(
        cx,
        json!({"pixpin": 1, "accion": "chat", "proyecto": proyecto_del_pedido(cx),
               "texto": format!("{fichas} «{}» en {formato}", c.nombre), "archivos": hechos}),
    )?;
    let nombres: Vec<String> = hechos.iter().map(|r| pixpin_docs::nombre(r)).collect();
    Ok(format!("hecho: «{}» exportado a {formato} ({}), ya esta en el chat", c.nombre, nombres.join(", ")))
}

#[cfg(test)]
mod pruebas;

#[cfg(test)]
mod banco;
