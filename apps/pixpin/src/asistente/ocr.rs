//! **El OCR del asistente** (encargo 3): el texto de un PDF escaneado o de
//! una imagen adjunta, con lo que ya trae Windows (`pixpin-ocr`,
//! `Windows.Media.Ocr`).
//!
//! - Un PDF: primero se mira si ya trae texto (`pixpin_pdf::texto`, exacto e
//!   instantaneo); las hojas sin texto se pintan con `pixpin-pdf` a
//!   [`ANCHO_OCR`] pixeles (unos 200 ppp en un A4) y se leen.
//! - Una imagen: se carga y se lee, en orden de lectura
//!   (`crate::texto_de_lineas`, el mismo de «copiar texto» del pin).
//!
//! El idioma lo pone Windows (los del perfil del usuario): `pixpin-ocr` no
//! deja elegir otro, asi que el `idioma` de la llamada se ignora.
//!
//! **Guardar el PDF con capa de texto**: en el repo no hay forma de
//! escribirla (`con_anotaciones` pega tinta visible, `de_imagenes` solo
//! fotos; lo «buscable» de compartir es la pagina web). Se ofrece solo el
//! texto, que el chat guarda como `.txt`.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use super::turno::Adjunto;

/// Ancho al que se pinta una hoja para leerla.
pub const ANCHO_OCR: u32 = 1700;

/// Tope de hojas por llamada: un PDF de 300 hojas son minutos de OCR.
pub const MAX_HOJAS: usize = 40;

/// Letras por hoja a partir de las cuales se da por buena la capa de texto
/// que ya trae el PDF (menos suele ser un sello o un numero de pagina sobre
/// una foto escaneada).
const TEXTO_MINIMO: usize = 40;

/// El adjunto que se pide: por nombre (sin mayusculas, vale un trozo) o, sin
/// nombre, el ultimo.
pub fn elegir<'a>(adjuntos: &'a [Adjunto], nombre: Option<&str>) -> Result<&'a Adjunto, String> {
    if adjuntos.is_empty() {
        return Err("no hay ningun PDF ni imagen adjunto en esta conversacion; pide al usuario que lo adjunte".into());
    }
    let Some(nombre) = nombre.map(str::trim).filter(|n| !n.is_empty()) else {
        return Ok(adjuntos.last().expect("no esta vacia"));
    };
    let buscado = nombre.to_lowercase();
    adjuntos
        .iter()
        .rev()
        .find(|a| a.nombre.to_lowercase() == buscado)
        .or_else(|| {
            adjuntos
                .iter()
                .rev()
                .find(|a| a.nombre.to_lowercase().contains(&buscado))
        })
        .ok_or_else(|| {
            let hay: Vec<&str> = adjuntos.iter().map(|a| a.nombre.as_str()).collect();
            format!(
                "no hay ningun adjunto llamado «{nombre}»; hay: {}",
                hay.join(", ")
            )
        })
}

pub fn es_pdf(ruta: &Path) -> bool {
    ruta.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

pub fn es_imagen(ruta: &Path) -> bool {
    ruta.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .is_some_and(|e| {
            matches!(
                e.as_str(),
                "png" | "jpg" | "jpeg" | "bmp" | "gif" | "webp" | "tif" | "tiff" | "heic"
            )
        })
}

/// Las hojas pedidas (desde 0), dentro de `total`. `rangos` vacio: todas.
pub fn hojas(rangos: &[(u32, u32)], total: u32) -> Vec<u32> {
    let mut v: Vec<u32> = if rangos.is_empty() {
        (0..total).collect()
    } else {
        rangos
            .iter()
            .flat_map(|&(a, b)| a.max(1)..=b.min(total))
            .map(|n| n - 1)
            .collect()
    };
    v.sort_unstable();
    v.dedup();
    v
}

/// **Lee el texto** de un PDF o una imagen.
pub fn leer(ruta: &Path, paginas: &[(u32, u32)], parar: &AtomicBool) -> Result<String, String> {
    if !ruta.is_file() {
        return Err(format!("el fichero «{}» ya no esta", ruta.display()));
    }
    if es_pdf(ruta) {
        leer_pdf(ruta, paginas, parar)
    } else if es_imagen(ruta) {
        if !pixpin_ocr::disponible() {
            return Err("este Windows no tiene ningun idioma de OCR instalado".into());
        }
        let imagen = pixpin_codec::imagen::cargar(ruta)
            .map_err(|e| format!("no se pudo abrir la imagen: {e}"))?;
        let texto = crate::texto_de_imagen(&imagen).map_err(|e| format!("{e:#}"))?;
        if texto.trim().is_empty() {
            return Err("no se encontro texto en la imagen".into());
        }
        Ok(texto)
    } else {
        Err("solo se puede leer el texto de un PDF o de una imagen".into())
    }
}

fn leer_pdf(ruta: &Path, paginas: &[(u32, u32)], parar: &AtomicBool) -> Result<String, String> {
    let doc = pixpin_pdf::Documento::abrir(ruta).map_err(|e| e.to_string())?;
    let total = doc.paginas();
    let elegidas = hojas(paginas, total);
    if elegidas.is_empty() {
        return Err(format!("el PDF tiene {total} hojas; esas no existen"));
    }
    let recortado = elegidas.len() > MAX_HOJAS;
    // La capa de texto que ya traiga (un PDF «de verdad» o uno ya pasado por
    // OCR): exacta y gratis.
    let capa = pixpin_pdf::texto::de_fichero(ruta).unwrap_or_default();
    let mut salida = String::new();
    let mut con_ocr = 0;
    for &i in elegidas.iter().take(MAX_HOJAS) {
        if parar.load(Ordering::Relaxed) {
            return Err("el usuario lo paro".into());
        }
        let de_capa = capa
            .get(i as usize)
            .map(|p| p.texto.trim().to_string())
            .filter(|t| t.chars().filter(|c| !c.is_whitespace()).count() >= TEXTO_MINIMO);
        let texto = match de_capa {
            Some(t) => t,
            None => {
                if con_ocr == 0 && !pixpin_ocr::disponible() {
                    return Err("este Windows no tiene ningun idioma de OCR instalado".into());
                }
                con_ocr += 1;
                let imagen = doc.renderizar(i, ANCHO_OCR).map_err(|e| e.to_string())?;
                crate::texto_de_imagen(&imagen).map_err(|e| format!("{e:#}"))?
            }
        };
        if elegidas.len() > 1 {
            salida.push_str(&format!("--- Hoja {} ---\n", i + 1));
        }
        salida.push_str(texto.trim());
        salida.push_str("\n\n");
    }
    if recortado {
        salida.push_str(&format!(
            "[solo las {MAX_HOJAS} primeras hojas de las pedidas]\n"
        ));
    }
    let salida = salida.trim_end().to_string();
    if salida
        .lines()
        .all(|l| l.trim().is_empty() || l.starts_with("--- Hoja"))
    {
        return Err("no se encontro texto en el PDF".into());
    }
    Ok(salida)
}
