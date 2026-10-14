//! **Un PDF leido como un Word** («Ver como texto», Android v0.92 `800db67`
//! y v0.93 `a6153f8`: `VisorHtmlActivity.abrirPdfComoTexto`).
//!
//! El texto lo saca `pixpin_pdf::texto::trozos_de_bytes`, lo junta en lineas
//! y parrafos `pixpin_docs::pdf_a_texto`, y la pagina que sale (la misma del
//! movil) se abre en este lector como cualquier `.html`: letra, voz, pagina
//! que sube sola, anotar.
//!
//! Como el movil, la pagina se guarda **por archivo, fecha y peso**: la
//! segunda vez abre al momento. Va a `cache/pdf-como-texto/<huella de la
//! ruta>/<nombre>.html` y no junto al PDF: el lector apunta a su lado lo
//! leido y lo anotado (`.pixpin-lectura`, `.pixpin-anotado`), y eso es de
//! esta lectura como texto, no del PDF de hojas (en el movil tambien van
//! aparte, `anot-<uid>-texto`). Como la carpeta depende solo de la ruta del
//! PDF, lo anotado sigue ahi aunque el PDF cambie y se vuelva a convertir.

use std::path::{Path, PathBuf};

use pixpin_docs::pdf_a_texto::{self, Trozo};
use pixpin_pdf::texto::{SinTexto, TrozoDeTexto};

/// Por que un PDF no se puede leer como texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NoSale {
    NoSeLee,
    Cifrado,
    /// No tiene texto: un escaneo.
    Escaneo,
}

impl NoSale {
    /// La clave del aviso que se le ensena al usuario.
    pub(crate) fn clave(self) -> &'static str {
        match self {
            NoSale::NoSeLee => "lector-texto-no-se-lee",
            NoSale::Cifrado => "lector-texto-cifrado",
            NoSale::Escaneo => "lector-texto-escaneo",
        }
    }
}

const CARPETA: &str = "pdf-como-texto";
const HUELLA: &str = "huella";
const ORIGEN: &str = "origen";

fn a_trozo(t: TrozoDeTexto) -> Trozo {
    Trozo {
        x: t.x as f64,
        y: t.y as f64,
        fin: t.fin as f64,
        alto: t.alto as f64,
        texto: t.texto,
        negrita: t.negrita,
        cursiva: t.cursiva,
    }
}

/// **Del PDF a la pagina**, sin tocar el disco. `del_aparato` es el idioma
/// si el texto no lo deja claro; `hoja`, la palabra de la marca de cada hoja.
pub(crate) fn convertir(bytes: &[u8], titulo: &str, del_aparato: &str, hoja: &str) -> Result<String, NoSale> {
    let por_hoja: Vec<Vec<Trozo>> = pixpin_pdf::texto::trozos_de_bytes(bytes)
        .map_err(|e| match e {
            SinTexto::NoEsPdf => NoSale::NoSeLee,
            SinTexto::Cifrado => NoSale::Cifrado,
        })?
        .into_iter()
        .map(|h| h.into_iter().map(a_trozo).collect())
        .collect();
    let parrafos = pdf_a_texto::parrafos_de_hojas(&por_hoja);
    if parrafos.is_empty() {
        return Err(NoSale::Escaneo);
    }
    let idioma = pdf_a_texto::idioma_de(&parrafos, del_aparato);
    Ok(pdf_a_texto::html(titulo, &parrafos, &idioma, hoja))
}

/// FNV-1a de 64 bits: una huella estable de la ruta (la de la biblioteca
/// estandar puede cambiar de una version a otra).
fn huella(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

/// La carpeta de la lectura como texto de ese PDF.
pub(crate) fn carpeta_de(raiz: &Path, pdf: &Path) -> PathBuf {
    let ruta = std::fs::canonicalize(pdf).unwrap_or_else(|_| pdf.to_path_buf());
    let clave = ruta.to_string_lossy().to_lowercase();
    raiz.join("cache").join(CARPETA).join(format!("{:016x}", huella(&clave)))
}

/// Peso y fecha del PDF: si cambian, se vuelve a convertir.
fn sello(pdf: &Path) -> Option<String> {
    let m = std::fs::metadata(pdf).ok()?;
    let fecha = m
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();
    Some(format!("{}|{fecha}", m.len()))
}

/// **La pagina de ese PDF, lista para abrir**: la de la cache si el PDF no
/// cambio, o una nueva. Devuelve la ruta del `.html`.
pub(crate) fn preparar(raiz: &Path, pdf: &Path, del_aparato: &str, hoja: &str) -> Result<PathBuf, NoSale> {
    let carpeta = carpeta_de(raiz, pdf);
    let titulo = pixpin_docs::sin_extension(&pixpin_docs::nombre(pdf));
    let pagina = carpeta.join(format!("{titulo}.html"));
    let sello = sello(pdf).ok_or(NoSale::NoSeLee)?;
    let ya = std::fs::read_to_string(carpeta.join(HUELLA)).ok();
    if ya.as_deref() == Some(sello.as_str()) && std::fs::metadata(&pagina).is_ok_and(|m| m.len() > 0) {
        return Ok(pagina);
    }
    let bytes = std::fs::read(pdf).map_err(|_| NoSale::NoSeLee)?;
    let html = convertir(&bytes, &titulo, del_aparato, hoja)?;
    let escrito = std::fs::create_dir_all(&carpeta)
        .and_then(|_| std::fs::write(&pagina, html))
        .and_then(|_| std::fs::write(carpeta.join(ORIGEN), pdf.to_string_lossy().as_bytes()))
        .and_then(|_| std::fs::write(carpeta.join(HUELLA), &sello));
    match escrito {
        Ok(()) => Ok(pagina),
        Err(e) => {
            tracing::warn!(?e, "no se pudo guardar el PDF pasado a texto");
            Err(NoSale::NoSeLee)
        }
    }
}

/// Si `ruta` es la pagina de un PDF leido como texto, el PDF del que sale
/// (para «Ver el PDF original, en hojas»).
pub(crate) fn pdf_de_origen(raiz: &Path, ruta: &Path) -> Option<PathBuf> {
    let carpeta = ruta.parent()?;
    if carpeta.parent()? != raiz.join("cache").join(CARPETA) {
        return None;
    }
    let origen = PathBuf::from(std::fs::read_to_string(carpeta.join(ORIGEN)).ok()?);
    origen.is_file().then_some(origen)
}

/// **El PDF del que sale una pagina leida como texto**, sin saber donde
/// vive la app: la pagina esta en `…/cache/pdf-como-texto/<huella>/` con su
/// `origen` al lado. Lo usa `anotado_del_adjunto` para que lo anotado viaje
/// como `anot-<uid>-texto`. `None` si no es una de esas paginas.
pub(crate) fn origen_de_la_pagina(ruta: &Path) -> Option<PathBuf> {
    let carpeta = ruta.parent()?;
    let cache = carpeta.parent()?;
    if cache.file_name()? != CARPETA || cache.parent()?.file_name()? != "cache" {
        return None;
    }
    let origen = PathBuf::from(std::fs::read_to_string(carpeta.join(ORIGEN)).ok()?.trim());
    origen
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
        .then_some(origen)
}

/// Si un bloque es la marca «Hoja N» de un PDF leido como texto (en los dos
/// idiomas de la app): no se lee en voz alta.
pub(crate) fn es_marca(texto: &str) -> bool {
    pdf_a_texto::es_marca_de_hoja(texto, "Hoja") || pdf_a_texto::es_marca_de_hoja(texto, "Page")
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_docs::documento::Clase;

    /// Un PDF de verdad con su indice, escrito como en `PdfAHtmlTest.kt`.
    pub(crate) fn pdf_de(hojas: &[String], fuentes: &str) -> Vec<u8> {
        let n = hojas.len();
        let mut objetos = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            format!(
                "<< /Type /Pages /Kids [{}] /Count {n} >>",
                (0..n).map(|i| format!("{} 0 R", 3 + 2 * i)).collect::<Vec<_>>().join(" ")
            ),
        ];
        let fuente = 3 + 2 * n;
        for h in hojas {
            objetos.push(format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents {} 0 R /Resources << /Font << /F1 {fuente} 0 R /F2 {} 0 R >> >> >>",
                objetos.len() + 2,
                fuente + 1
            ));
            objetos.push(format!("<< /Length {} >>\nstream\n{h}\nendstream", h.len()));
        }
        objetos.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".into());
        objetos.push(fuentes.to_string());
        let mut s: Vec<u8> = b"%PDF-1.4\n".to_vec();
        let mut sitios = Vec::new();
        for (i, o) in objetos.iter().enumerate() {
            sitios.push(s.len());
            s.extend(format!("{} 0 obj\n", i + 1).bytes());
            // WinAnsi: las letras acentuadas en un byte, como el ISO_8859_1 del movil.
            s.extend(o.chars().map(|c| c as u32 as u8));
            s.extend(b"\nendobj\n");
        }
        let xref = s.len();
        s.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objetos.len() + 1).bytes());
        for d in sitios {
            s.extend(format!("{d:010} 00000 n \n").bytes());
        }
        s.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objetos.len() + 1).bytes());
        s
    }

    fn esc(s: &str) -> String {
        s.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)")
    }

    fn texto_en(f: &str, tam: u32, x: u32, y: u32, t: &str) -> String {
        format!("BT /{f} {tam} Tf {x} {y} Td ({}) Tj ET\n", esc(t))
    }

    /// Las dos hojas de `PdfAHtmlTest.pdf()`.
    fn apuntes() -> Vec<u8> {
        let hoja1 = texto_en("F1", 22, 72, 760, "Capítulo uno")
            + &texto_en("F1", 11, 72, 720, "La ingeniería de los materiales estudia cómo se")
            + &texto_en("F1", 11, 72, 706, "comportan las cosas cuando se cargan y por qué se rompen.")
            + &texto_en("F1", 11, 72, 670, "Es la base de todo lo que se construye, y en esta hoja lo vemos.")
            + &texto_en("F1", 9, 297, 40, "1");
        let hoja2 = texto_en("F1", 11, 72, 760, "Segunda hoja con más texto para que se lea bien el idioma de la página y de los que la leen.")
            + &texto_en("F1", 9, 297, 40, "2");
        pdf_de(&[hoja1, hoja2], "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>")
    }

    #[test]
    fn un_pdf_de_verdad_sale_como_pagina_con_titulos_parrafos_hojas_e_idioma() {
        let html = convertir(&apuntes(), "Apuntes", "es-PE", "Hoja").unwrap();
        assert!(html.contains("<html lang=\"es\">"), "{html}");
        assert!(
            html.contains("<h1 data-hoja=\"1\">Capítulo uno</h1>") || html.contains("<h2 data-hoja=\"1\">Capítulo uno</h2>"),
            "{html}"
        );
        assert!(html.contains("La ingeniería de los materiales estudia cómo se comportan las cosas cuando se cargan"), "{html}");
        assert!(html.contains("id=\"hoja-2\""));
        assert!(html.contains("Segunda hoja con más texto"));
        // El numero de pagina no es texto.
        assert!(!html.contains(">1<") && !html.contains(">2<"), "{html}");
        // Y el lector de Word la abre: marca de hoja en gris, titulo y parrafos.
        let doc = pixpin_docs::html::pagina(&html, "x");
        assert_eq!(doc.titulo, "Apuntes");
        assert_eq!(doc.bloques[0].clase, Clase::Nota);
        assert!(es_marca(&doc.bloques[0].texto()));
        assert!(matches!(doc.bloques[1].clase, Clase::Titulo(1 | 2)));
        assert!(!es_marca(&doc.bloques[2].texto()));
    }

    /// Una hoja de articulo a dos columnas, con titulo de ancho entero, un
    /// subtitulo en negrita y el numero de pagina: lo que sale de LaTeX o de
    /// una revista.
    fn articulo() -> Vec<u8> {
        let mut h = texto_en("F1", 18, 100, 780, "Un estudio a dos columnas sobre el hormigón");
        let izq = [
            "El hormigón armado se usa en casi todas las obras",
            "porque resiste bien a compresión y el acero le da",
            "la resistencia a tracción que le falta. Esta es la",
            "primera columna y se tiene que leer entera antes.",
        ];
        let der = [
            "La segunda columna empieza aquí arriba, a la misma",
            "altura que la primera, y solo se lee después de",
            "acabar la de la izquierda, como en cualquier revista",
            "que se imprime a dos columnas con su pasillo.",
        ];
        h += &format!("BT /F2 11 Tf 56 746 Td ({}) Tj ET\n", esc("Introducción"));
        for (k, (a, b)) in izq.iter().zip(der).enumerate() {
            let y = 720 - 14 * k as u32;
            h += &texto_en("F1", 10, 56, y, a);
            h += &texto_en("F1", 10, 330, y, b);
        }
        pdf_de(&[h], "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>")
    }

    #[test]
    fn un_pdf_a_dos_columnas_se_lee_columna_a_columna() {
        let html = convertir(&articulo(), "Artículo", "es", "Hoja").unwrap();
        let doc = pixpin_docs::html::pagina(&html, "x");
        let textos: Vec<String> = doc.bloques.iter().map(|b| b.texto()).collect();
        let todo = textos.join("\n");
        let primera = todo.find("primera columna").expect(&todo);
        let segunda = todo.find("La segunda columna").expect(&todo);
        assert!(primera < segunda, "la izquierda entera antes: {todo}");
        assert!(todo.contains("El hormigón armado se usa en casi todas las obras porque resiste"), "{todo}");
        assert!(!todo.contains("obras La segunda"), "no se intercalan: {todo}");
        // El subtitulo en negrita y corto es un titulo de nivel 3.
        assert!(doc.bloques.iter().any(|b| b.clase == Clase::Titulo(3) && b.texto() == "Introducción"), "{todo}");
    }

    #[test]
    fn un_escaneo_un_cifrado_y_lo_que_no_es_pdf_dicen_que_no() {
        let foto = pdf_de(&["0 0 1 rg 10 10 500 700 re f".into()], "<< /Type /Font /Subtype /Type1 /BaseFont /X >>");
        assert_eq!(convertir(&foto, "x", "es", "Hoja"), Err(NoSale::Escaneo));
        assert_eq!(convertir(b"esto no es un pdf", "x", "es", "Hoja"), Err(NoSale::NoSeLee));
        assert_eq!(NoSale::Cifrado.clave(), "lector-texto-cifrado");
    }

    fn temporal(etiqueta: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-como-texto-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn la_pagina_se_guarda_aparte_y_se_rehace_si_el_pdf_cambia() {
        let d = temporal("cache");
        let raiz = d.join("raiz");
        let pdf = d.join("Apuntes.pdf");
        std::fs::write(&pdf, apuntes()).unwrap();
        let pagina = preparar(&raiz, &pdf, "es", "Hoja").unwrap();
        assert!(pagina.starts_with(raiz.join("cache").join(CARPETA)));
        assert_eq!(pagina.file_name().unwrap(), "Apuntes.html");
        assert!(!d.join("Apuntes.html").exists(), "nada junto al PDF");
        assert_eq!(pdf_de_origen(&raiz, &pagina).as_deref(), Some(pdf.as_path()));
        // La segunda vez no se convierte: se toca la pagina y sigue asi.
        std::fs::write(&pagina, "<p>tocada</p>").unwrap();
        assert_eq!(preparar(&raiz, &pdf, "es", "Hoja").unwrap(), pagina);
        assert_eq!(std::fs::read_to_string(&pagina).unwrap(), "<p>tocada</p>");
        // El PDF cambia (otro peso): se rehace en la misma carpeta.
        std::fs::write(&pdf, articulo()).unwrap();
        assert_eq!(preparar(&raiz, &pdf, "es", "Hoja").unwrap(), pagina);
        assert!(std::fs::read_to_string(&pagina).unwrap().contains("hormigón"));
        // Un documento cualquiera no viene de un PDF.
        assert_eq!(pdf_de_origen(&raiz, &pdf), None);
        // Un escaneo no deja nada a medias.
        let foto = d.join("Foto.pdf");
        std::fs::write(&foto, pdf_de(&["".into()], "<< >>")).unwrap();
        assert_eq!(preparar(&raiz, &foto, "es", "Hoja"), Err(NoSale::Escaneo));
        assert!(!carpeta_de(&raiz, &foto).exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// **El PDF de dos columnas real**: uno hecho con LaTeX
    /// (`\documentclass[twocolumn]{article}`), en la ruta de
    /// `PIXPIN_PDF_DOS_COLUMNAS`. Escribe lo sacado al lado para mirarlo.
    #[test]
    #[ignore]
    fn un_pdf_real_a_dos_columnas() {
        let ruta = std::env::var("PIXPIN_PDF_DOS_COLUMNAS").expect("PIXPIN_PDF_DOS_COLUMNAS");
        let bytes = std::fs::read(&ruta).unwrap();
        let html = convertir(&bytes, "real", "es", "Hoja").unwrap();
        std::fs::write(format!("{ruta}.html"), &html).unwrap();
        let doc = pixpin_docs::html::pagina(&html, "x");
        let todo: Vec<String> = doc.bloques.iter().map(|b| b.texto()).collect();
        std::fs::write(format!("{ruta}.txt"), todo.join("\n\n")).unwrap();
        let todo = todo.join("\n");
        let a = todo.find("FINIZQUIERDA").expect("fin de la izquierda");
        let b = todo.find("INICIODERECHA").expect("inicio de la derecha");
        assert!(a < b, "la columna izquierda entera antes que la derecha");
    }
}
