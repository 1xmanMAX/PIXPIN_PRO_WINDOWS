//! **El texto de un documento del disco y el OCR de Windows**, con los
//! crates de siempre: `pixpin-pdf` (capa de texto y pintar la hoja para el
//! OCR), `pixpin-docs` (Word, EPUB, HTML, Markdown, texto), `pixpin-office`
//! (PowerPoint), `pixpin-hojas` (Excel) y `pixpin-ocr` (`Windows.Media.Ocr`,
//! sin modelos que bajar). Se lee igual que el indice del asistente
//! (`apps/pixpin/src/asistente/documentos.rs`, `LectorDeLaApp`) y que su OCR
//! (`asistente/ocr.rs`): la capa de texto si la hay y, si no, la hoja
//! pintada a ~200 ppp y leida.
//!
//! Solo lee: ni el `.txt` del OCR se escribe al lado (eso lo hace el chat).

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::herramientas::{Contenido, Resultado};

/// Ancho al que se pinta una hoja de PDF para el OCR (~200 ppp en A4), el
/// mismo que el asistente.
pub const ANCHO_OCR: u32 = 1700;
/// Hojas como mucho por llamada con OCR (cada una son cientos de ms).
pub const MAX_OCR: usize = 30;
/// Letras por hoja a partir de las cuales la capa de texto del PDF vale.
const TEXTO_MINIMO: usize = 40;
/// Lo mas que se devuelve (letras): un libro entero no cabe en el contexto.
pub const MAX_LETRAS: usize = 120_000;
/// Celdas por hoja de Excel como mucho.
const MAX_CELDAS: usize = 20_000;

fn ruta_de(args: &Map<String, Value>) -> Result<PathBuf, String> {
    let r = args
        .get("ruta")
        .and_then(Value::as_str)
        .map(|s| {
            s.trim()
                .trim_matches(|c| c == '"' || c == '\'' || c == '«' || c == '»')
        })
        .filter(|s| !s.is_empty())
        .ok_or("falta «ruta» (ruta absoluta del fichero)")?;
    let ruta = PathBuf::from(r);
    if !ruta.is_absolute() {
        return Err(format!("«{r}» no es una ruta absoluta"));
    }
    if !ruta.is_file() {
        return Err(format!("no existe el fichero «{r}»"));
    }
    Ok(ruta)
}

/// Las paginas pedidas («3», «1-4, 7»), desde 1. Vacio: todas.
pub fn rangos(texto: &str) -> Result<Vec<(u32, u32)>, String> {
    let mut v = Vec::new();
    for trozo in texto
        .split([',', ';'])
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let (a, b) = match trozo.split_once(['-', '–']) {
            Some((a, b)) => (a.trim().parse::<u32>(), b.trim().parse::<u32>()),
            None => (trozo.parse::<u32>(), trozo.parse::<u32>()),
        };
        match (a, b) {
            (Ok(a), Ok(b)) if a >= 1 && b >= a => v.push((a, b)),
            _ => {
                return Err(format!(
                    "«{trozo}» no es una pagina ni un rango (p. ej. «3» o «1-4»)"
                ));
            }
        }
    }
    Ok(v)
}

/// Si la pagina `n` (desde 1) entra en los rangos.
fn entra(rangos: &[(u32, u32)], n: u32) -> bool {
    rangos.is_empty() || rangos.iter().any(|&(a, b)| (a..=b).contains(&n))
}

fn es_imagen(ext: &str) -> bool {
    matches!(
        ext,
        "png" | "jpg" | "jpeg" | "bmp" | "gif" | "webp" | "tif" | "tiff" | "heic" | "jfif"
    )
}

/// El OCR de una imagen ya cargada, en orden de lectura (lo mismo que
/// `texto_de_imagen` de la app: lineas → parrafos).
fn ocr(imagen: &pixpin_codec::ImagenRgba) -> Result<String, String> {
    let lineas = pixpin_ocr::reconocer(imagen.ancho, imagen.alto, &imagen.pixeles)
        .map_err(|e| format!("el OCR fallo: {e}"))?;
    Ok(pixpin_geom::parrafos::a_texto(
        &pixpin_geom::parrafos::agrupar(
            lineas
                .into_iter()
                .map(|l| pixpin_geom::parrafos::LineaTexto {
                    caja: l.caja,
                    texto: l.texto,
                })
                .collect(),
        ),
    ))
}

fn sin_ocr() -> String {
    "este Windows no tiene ningun idioma de OCR instalado (Configuracion > Hora e idioma > Idioma)"
        .into()
}

/// Un PDF: la capa de texto de cada hoja y, si no la tiene, OCR.
fn leer_pdf(ruta: &Path, rangos: &[(u32, u32)], forzar_ocr: bool) -> Result<String, String> {
    let doc =
        pixpin_pdf::Documento::abrir(ruta).map_err(|e| format!("el PDF no se deja abrir: {e}"))?;
    let total = doc.paginas();
    let capa = if forzar_ocr {
        Vec::new()
    } else {
        pixpin_pdf::texto::de_fichero(ruta).unwrap_or_default()
    };
    let hojas: Vec<u32> = (1..=total).filter(|n| entra(rangos, *n)).collect();
    if hojas.is_empty() {
        return Err(format!("el PDF tiene {total} paginas; esas no existen"));
    }
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let mut s = String::new();
    let mut con_ocr = 0;
    let mut cortado = false;
    for n in &hojas {
        let de_capa = capa
            .get(*n as usize - 1)
            .map(|p| p.texto.trim().to_string())
            .filter(|t| t.chars().filter(|c| !c.is_whitespace()).count() >= TEXTO_MINIMO);
        let texto = match de_capa {
            Some(t) => t,
            None => {
                if con_ocr >= MAX_OCR {
                    cortado = true;
                    break;
                }
                if con_ocr == 0 && !pixpin_ocr::disponible() {
                    return Err(sin_ocr());
                }
                con_ocr += 1;
                let img = doc
                    .renderizar(n - 1, ANCHO_OCR)
                    .map_err(|e| e.to_string())?;
                ocr(&img)?
            }
        };
        if hojas.len() > 1 {
            s.push_str(&format!("--- Pagina {n} ---\n"));
        }
        s.push_str(texto.trim());
        s.push_str("\n\n");
        if s.len() > MAX_LETRAS {
            cortado = true;
            break;
        }
    }
    if cortado {
        s.push_str("[cortado: pide menos paginas con «paginas»]\n");
    }
    if con_ocr > 0 {
        s.push_str(&format!("[{con_ocr} paginas leidas con OCR]\n"));
    }
    Ok(s)
}

fn leer_word_y_demas(ruta: &Path) -> Result<String, String> {
    let doc = pixpin_docs::abrir(ruta).map_err(|e| e.to_string())?;
    let mut s = String::new();
    for b in &doc.bloques {
        let t = b
            .texto()
            .replace(pixpin_docs::documento::SEPARADOR_DE_CELDA, " | ");
        if t.trim().is_empty() || t.trim() == pixpin_docs::documento::MARCA_IMAGEN {
            continue;
        }
        if matches!(
            b.clase,
            pixpin_docs::documento::Clase::Titulo(_) | pixpin_docs::documento::Clase::Capitulo
        ) {
            s.push_str("\n## ");
        }
        s.push_str(t.trim_end());
        s.push('\n');
    }
    Ok(s)
}

fn leer_powerpoint(ruta: &Path, rangos: &[(u32, u32)]) -> Result<String, String> {
    let doc = pixpin_office::Documento::abrir(ruta)
        .map_err(|e| format!("el PowerPoint no se deja leer: {e}"))?;
    let pixpin_office::Documento::PowerPoint(mut p) = doc else {
        return Err("no es un PowerPoint".into());
    };
    let medidor = pixpin_office::MedidorFijo::default();
    let mut s = String::new();
    for n in 0..p.cuantas() {
        if !entra(rangos, n as u32 + 1) {
            continue;
        }
        let Some(pagina) = p.diapositiva(n, &medidor) else {
            continue;
        };
        let texto: Vec<String> = pagina
            .textos()
            .iter()
            .map(|(t, _)| t.texto.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
        s.push_str(&format!(
            "--- Diapositiva {} ---\n{}\n\n",
            n + 1,
            texto.join("\n")
        ));
    }
    Ok(s)
}

fn leer_excel(ruta: &Path, rangos: &[(u32, u32)]) -> Result<String, String> {
    let libro = pixpin_hojas::Libro::abrir(ruta, pixpin_hojas::Regional::default())
        .map_err(|e| format!("el Excel no se deja leer: {e}"))?;
    let mut s = String::new();
    let mut numero = 0;
    for (i, info) in libro.hojas.iter().enumerate() {
        if info.tipo != pixpin_hojas::TipoHoja::Hoja {
            continue;
        }
        numero += 1;
        if !entra(rangos, numero) {
            continue;
        }
        let mut hoja = libro.abrir_hoja(i);
        hoja.cargar_todo();
        s.push_str(&format!("--- Hoja «{}» ---\n", info.nombre));
        let filas: Vec<u32> = {
            let f = hoja.filas_entre(0, u32::MAX);
            if f.is_empty() {
                (0..=hoja.max_fila().min(MAX_CELDAS as u32)).collect()
            } else {
                f.iter().map(|f| f.indice).collect()
            }
        };
        let mut celdas = 0;
        for i in filas {
            let Some(fila) = hoja.fila(i) else { continue };
            let linea: Vec<String> = hoja
                .celdas_de(fila)
                .iter()
                .map(|c| libro.texto_crudo(&hoja, c))
                .collect();
            if linea.iter().all(|t| t.trim().is_empty()) {
                continue;
            }
            celdas += linea.len();
            s.push_str(&format!("{}: {}\n", i + 1, linea.join(" | ")));
            if celdas >= MAX_CELDAS {
                s.push_str("[hoja cortada]\n");
                break;
            }
        }
        s.push('\n');
    }
    Ok(s)
}

fn recortar(mut s: String) -> String {
    if s.len() > MAX_LETRAS {
        let mut corte = MAX_LETRAS;
        while !s.is_char_boundary(corte) {
            corte -= 1;
        }
        s.truncate(corte);
        s.push_str("\n[cortado: el documento es mas largo; pide menos paginas]");
    }
    s
}

/// **`leer_documento`**.
pub fn leer_documento(args: &Map<String, Value>) -> Resultado {
    let ruta = ruta_de(args)?;
    let rangos = rangos(args.get("paginas").and_then(Value::as_str).unwrap_or(""))?;
    let ext = pixpin_docs::extension(&pixpin_docs::nombre(&ruta));
    let texto = match ext.as_str() {
        "pdf" => leer_pdf(&ruta, &rangos, false)?,
        "pptx" | "pptm" | "ppsx" => leer_powerpoint(&ruta, &rangos)?,
        "xlsx" | "xlsm" | "xls" | "ods" => leer_excel(&ruta, &rangos)?,
        e if es_imagen(e) => return ocr_imagen(args),
        _ if pixpin_docs::formato_de(&pixpin_docs::nombre(&ruta)).is_some() => {
            leer_word_y_demas(&ruta)?
        }
        _ => return Err(format!("PixPin no sabe leer el texto de un «.{ext}»")),
    };
    if texto.trim().is_empty() {
        return Err("el documento no tiene texto".into());
    }
    Ok(vec![Contenido::Texto(recortar(
        texto.trim_end().to_string(),
    ))])
}

/// **`ocr_imagen`**: una imagen, o un PDF leido entero con OCR (aunque
/// tenga capa de texto: se pidio OCR).
pub fn ocr_imagen(args: &Map<String, Value>) -> Resultado {
    let ruta = ruta_de(args)?;
    let ext = pixpin_docs::extension(&pixpin_docs::nombre(&ruta));
    if ext == "pdf" {
        let rangos = rangos(args.get("paginas").and_then(Value::as_str).unwrap_or(""))?;
        return Ok(vec![Contenido::Texto(recortar(leer_pdf(
            &ruta, &rangos, true,
        )?))]);
    }
    if !es_imagen(&ext) {
        return Err("solo se puede hacer OCR de una imagen o un PDF".into());
    }
    let _com = pixpin_shell::ComDelHilo::iniciar();
    if !pixpin_ocr::disponible() {
        return Err(sin_ocr());
    }
    let img = pixpin_codec::imagen::cargar(&ruta)
        .map_err(|e| format!("no se pudo abrir la imagen: {e}"))?;
    let texto = ocr(&img)?;
    if texto.trim().is_empty() {
        return Err("no se encontro texto en la imagen".into());
    }
    Ok(vec![Contenido::Texto(texto)])
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_rangos_de_paginas() {
        assert_eq!(rangos("").unwrap(), vec![]);
        assert_eq!(rangos("3").unwrap(), vec![(3, 3)]);
        assert_eq!(rangos("1-4, 7").unwrap(), vec![(1, 4), (7, 7)]);
        assert!(entra(&[(1, 4)], 4) && !entra(&[(1, 4)], 5) && entra(&[], 99));
        // Casos negativos.
        assert!(rangos("0").is_err());
        assert!(rangos("5-2").is_err());
        assert!(rangos("dos").is_err());
    }

    #[test]
    fn sin_ruta_buena_no_se_lee_nada() {
        let a = |s: &str| serde_json::from_str::<Map<String, Value>>(s).unwrap();
        assert!(leer_documento(&a("{}")).unwrap_err().contains("«ruta»"));
        assert!(
            leer_documento(&a(r#"{"ruta":"relativa.pdf"}"#))
                .unwrap_err()
                .contains("absoluta")
        );
        assert!(
            leer_documento(&a(r#"{"ruta":"C:\\no\\existe\\nada.pdf"}"#))
                .unwrap_err()
                .contains("no existe")
        );
    }

    #[test]
    fn un_texto_y_un_markdown_se_leen() {
        let dir = std::env::temp_dir().join(format!("pixpin-mcp-doc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let md = dir.join("nota.md");
        std::fs::write(&md, "# Obra\n\nRevisar las cunetas del km 3.\n").unwrap();
        let mut a = Map::new();
        a.insert("ruta".into(), Value::String(md.display().to_string()));
        let r = leer_documento(&a).unwrap();
        let Contenido::Texto(t) = &r[0] else { panic!() };
        assert!(t.contains("cunetas del km 3"), "{t}");
        let raro = dir.join("x.qqq");
        std::fs::write(&raro, "x").unwrap();
        a.insert("ruta".into(), Value::String(raro.display().to_string()));
        assert!(leer_documento(&a).unwrap_err().contains(".qqq"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
