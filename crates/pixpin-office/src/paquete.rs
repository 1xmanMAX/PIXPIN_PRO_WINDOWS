//! **El paquete OPC**: el ZIP de un `.docx` o un `.pptx`, sus partes, sus
//! relaciones (`_rels`) y los tipos de contenido. Y las unidades de Office.
//!
//! El fichero se lee entero a memoria una vez (un `.docx` de treinta paginas
//! pesa cientos de KB) y de ahi se sacan solo las partes que hacen falta,
//! cuando hacen falta.

use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::sync::Arc;

use pixpin_docs::xml::Nodo;

/// Por que no se abre un documento.
#[derive(Debug)]
pub enum ErrorOffice {
    /// No es un ZIP (un `.doc` antiguo, un fichero roto...).
    NoEsZip,
    /// Es un ZIP pero le falta la parte principal.
    SinParte(&'static str),
    Disco(std::io::Error),
}

impl std::fmt::Display for ErrorOffice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorOffice::NoEsZip => write!(f, "el archivo no es un documento de Office (.docx o .pptx)"),
            ErrorOffice::SinParte(p) => write!(f, "al documento le falta {p}"),
            ErrorOffice::Disco(e) => write!(f, "no se pudo leer el archivo: {e}"),
        }
    }
}

impl std::error::Error for ErrorOffice {}

impl From<std::io::Error> for ErrorOffice {
    fn from(e: std::io::Error) -> Self {
        ErrorOffice::Disco(e)
    }
}

/// Una parte no puede pesar mas que esto descomprimida: una bomba de ZIP no
/// se lleva la memoria por delante.
const TOPE_PARTE: u64 = 256 * 1024 * 1024;

/// Una relacion de `_rels`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relacion {
    /// La ruta de la parte destino ya resuelta (sin `/` inicial), o la URL
    /// tal cual si es externa.
    pub destino: String,
    pub externa: bool,
    /// El tipo, solo el final (`image`, `slideLayout`, `header`...).
    pub tipo: String,
}

/// id → relacion de una parte.
pub type Relaciones = HashMap<String, Relacion>;

pub struct Paquete {
    zip: zip::ZipArchive<Cursor<Arc<[u8]>>>,
    /// Nombre en minusculas → nombre real: Office no distingue mayusculas en
    /// las rutas y hay generadores que las escriben como quieren.
    nombres: HashMap<String, String>,
}

impl Paquete {
    pub fn de_bytes(bytes: Arc<[u8]>) -> Result<Paquete, ErrorOffice> {
        let zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| ErrorOffice::NoEsZip)?;
        let nombres = zip.file_names().map(|n| (n.to_ascii_lowercase(), n.to_string())).collect();
        Ok(Paquete { zip, nombres })
    }

    pub fn abrir(ruta: &std::path::Path) -> Result<Paquete, ErrorOffice> {
        let bytes = std::fs::read(ruta)?;
        Paquete::de_bytes(Arc::from(bytes))
    }

    pub fn existe(&self, parte: &str) -> bool {
        self.nombres.contains_key(&parte.trim_start_matches('/').to_ascii_lowercase())
    }

    /// Los bytes de una parte, descomprimidos.
    pub fn bytes(&mut self, parte: &str) -> Option<Vec<u8>> {
        let real = self.nombres.get(&parte.trim_start_matches('/').to_ascii_lowercase())?.clone();
        let f = self.zip.by_name(&real).ok()?;
        let tam = f.size();
        if tam > TOPE_PARTE {
            return None;
        }
        let mut v = Vec::with_capacity(tam as usize);
        f.take(TOPE_PARTE).read_to_end(&mut v).ok()?;
        Some(v)
    }

    /// Una parte como texto (para leerla por trozos).
    pub fn texto(&mut self, parte: &str) -> Option<String> {
        let b = self.bytes(parte)?;
        Some(texto_de(&b).into_owned())
    }

    /// Una parte XML leida como arbol.
    pub fn xml(&mut self, parte: &str) -> Option<Nodo> {
        let b = self.bytes(parte)?;
        crate::xml::leer(&texto_de(&b))
    }

    /// Las relaciones de una parte (`word/document.xml` →
    /// `word/_rels/document.xml.rels`), con los destinos ya resueltos.
    pub fn relaciones(&mut self, parte: &str) -> Relaciones {
        let parte = parte.trim_start_matches('/');
        let (carpeta, nombre) = match parte.rfind('/') {
            Some(i) => (&parte[..i], &parte[i + 1..]),
            None => ("", parte),
        };
        let ruta_rels = if carpeta.is_empty() { format!("_rels/{nombre}.rels") } else { format!("{carpeta}/_rels/{nombre}.rels") };
        let mut m = HashMap::new();
        let Some(rels) = self.xml(&ruta_rels) else {
            return m;
        };
        for r in rels.elementos() {
            if r.nombre != "Relationship" {
                continue;
            }
            let externa = r.atributo("TargetMode").eq_ignore_ascii_case("External");
            let destino = r.atributo("Target");
            let destino = if externa { destino.to_string() } else { resolver(carpeta, destino) };
            let tipo = r.atributo("Type").rsplit('/').next().unwrap_or("").to_string();
            m.insert(r.atributo("Id").to_string(), Relacion { destino, externa, tipo });
        }
        m
    }

    /// La parte principal segun `_rels/.rels` (la de tipo `officeDocument`).
    pub fn principal(&mut self) -> Option<String> {
        let r = self.relaciones("");
        r.values().find(|r| r.tipo == "officeDocument" && !r.externa).map(|r| r.destino.clone())
    }
}

/// Une una ruta relativa de `_rels` con la carpeta de la parte que la
/// pide, quitando los `..` y `.`: `word` + `../media/a.png` → `media/a.png`.
pub fn resolver(carpeta: &str, destino: &str) -> String {
    let destino = destino.replace('\\', "/");
    let mut partes: Vec<&str> = Vec::new();
    if !destino.starts_with('/') {
        partes.extend(carpeta.split('/').filter(|s| !s.is_empty()));
    }
    for p in destino.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                partes.pop();
            }
            otro => partes.push(otro),
        }
    }
    // `%20` y demas: los nombres de parte van codificados como en una URL.
    descodificar_url(&partes.join("/"))
}

fn descodificar_url(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let b = s.as_bytes();
    let mut v = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(n) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""), 16)
        {
            v.push(n);
            i += 3;
            continue;
        }
        v.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&v).into_owned()
}

/// Bytes a texto: UTF-8 (con o sin BOM) o UTF-16 con BOM.
pub fn texto_de(b: &[u8]) -> std::borrow::Cow<'_, str> {
    if b.len() >= 2 && (b[0] == 0xFF && b[1] == 0xFE || b[0] == 0xFE && b[1] == 0xFF) {
        let le = b[0] == 0xFF;
        let u: Vec<u16> =
            b[2..].chunks_exact(2).map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }).collect();
        return std::borrow::Cow::Owned(String::from_utf16_lossy(&u));
    }
    let b = b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(b);
    String::from_utf8_lossy(b)
}

// ---------------------------------------------------------------------
// Unidades
// ---------------------------------------------------------------------

/// Veintavos de punto (twips, `dxa`) a puntos.
pub fn twips(v: f32) -> f32 {
    v / 20.0
}

/// EMU (914400 por pulgada) a puntos.
pub fn emu(v: f32) -> f32 {
    v / 12700.0
}

/// Lee un numero de un atributo; `None` si no lo es. Acepta las unidades
/// que el estandar estricto permite (`12pt`, `1in`, `2.5cm`, `10mm`, `3pc`)
/// convirtiendolas a la unidad nativa que se diga (`por_punto`: cuantas
/// unidades nativas hay en un punto).
pub fn numero(s: &str, por_punto: f32) -> Option<f32> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(v) = s.parse::<f32>() {
        return v.is_finite().then_some(v);
    }
    let (num, unidad) = s.split_at(s.find(|c: char| c.is_ascii_alphabetic() || c == '%').unwrap_or(s.len()));
    let v: f32 = num.trim().parse().ok()?;
    let pt = match unidad {
        "pt" => v,
        "in" => v * 72.0,
        "cm" => v * 72.0 / 2.54,
        "mm" => v * 72.0 / 25.4,
        "pc" | "pi" => v * 12.0,
        "%" => return Some(v * 1000.0), // porcentajes «50%» → 50000 (milesimas)
        _ => return None,
    };
    Some(pt * por_punto)
}

/// Un atributo como numero (en la unidad nativa); `None` si falta.
pub fn attr_num(n: &Nodo, nombre: &str, por_punto: f32) -> Option<f32> {
    numero(n.atributo(nombre), por_punto)
}

/// Un interruptor de Word (`<w:b/>`, `<w:b w:val="0"/>`, `true`, `off`...).
pub fn interruptor(n: &Nodo) -> bool {
    !matches!(n.valor(), "0" | "false" | "off" | "none")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_rutas_relativas_se_resuelven() {
        assert_eq!(resolver("word", "media/a.png"), "word/media/a.png");
        assert_eq!(resolver("ppt/slides", "../slideLayouts/slideLayout1.xml"), "ppt/slideLayouts/slideLayout1.xml");
        assert_eq!(resolver("word", "/customXml/item1.xml"), "customXml/item1.xml");
        assert_eq!(resolver("word", "media/una%20foto.png"), "word/media/una foto.png");
        assert_eq!(resolver("", "word/document.xml"), "word/document.xml");
    }

    #[test]
    fn los_numeros_con_unidad_se_entienden() {
        assert_eq!(numero("240", 20.0), Some(240.0));
        assert_eq!(numero("12pt", 20.0), Some(240.0));
        assert_eq!(numero("1in", 1.0), Some(72.0));
        assert_eq!(numero("", 1.0), None);
        assert_eq!(numero("abc", 1.0), None);
    }

    #[test]
    fn un_zip_roto_no_abre() {
        assert!(matches!(Paquete::de_bytes(Arc::from(&b"no soy un zip"[..])), Err(ErrorOffice::NoEsZip)));
    }
}
