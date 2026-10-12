//! **Las tablas del chat como un `.xlsx` minimo**, para verlas en el visor
//! de hojas (`visor_hoja`, que pinta `pixpin-hojas`) con la misma calidad
//! que un Excel de verdad (el usuario, 11-oct-2026: «que todo siempre se
//! abra en version de pin, en modo visualizacion»).
//!
//! No es un exportador para Excel: es el camino mas corto para que el visor
//! entienda una [`Tabla`]. Lleva lo que la tabla tiene —los VALORES ya
//! calculados (el visor no calcula: ensena el ultimo resultado, como Excel
//! al abrir), la negrita, la alineacion, el fondo y los anchos— y nada mas.
//! Va sin comprimir (`Stored`): se hace y se lee en memoria al momento, y
//! comprimir solo costaria tiempo.
//!
//! Escrito desde ECMA-376 (SpreadsheetML): `workbook.xml`, una
//! `worksheets/sheetN.xml` por tabla, `styles.xml` y sus relaciones.

use std::collections::BTreeMap;
use std::io::Write;

use crate::formula::{self, Valor};
use crate::tabla::{EstiloDeCelda, Ref, Tabla, ref_a, ref_de};

/// El ancho de una columna sin ancho propio, en pixeles (el de la rejilla
/// del chat, `pixpin_ui::tabla::COLUMNA_ANCHO`, y el del editor del pin).
pub const ANCHO_COLUMNA: u32 = 88;
/// El alto de una fila, en pixeles al 100 %.
pub const ALTO_FILA: u32 = 22;
/// El ancho del digito de Calibri 11 a 96 ppp (`mdw` de ECMA-376 18.3.1.13):
/// un ancho guardado de `px / 7` caracteres vuelve a ser `px` pixeles.
const DIGITO: f64 = 7.0;

/// Una hoja del libro: su nombre (el de la pestana) y su tabla.
pub struct HojaParaVer<'a> {
    pub nombre: &'a str,
    pub tabla: &'a Tabla,
    /// Lo que vale cada celda, si ya se calculo (el editor lo tiene): asi
    /// no se vuelve a calcular.
    pub valores: Option<&'a BTreeMap<String, Valor>>,
}

/// **El libro en bytes** (un ZIP), con `activa` como la hoja que se ve al
/// abrir. Un libro sin hojas sale con una vacia: el visor necesita una.
pub fn a_xlsx(hojas: &[HojaParaVer], activa: usize) -> std::io::Result<Vec<u8>> {
    let vacia = Tabla::default();
    let sola = [HojaParaVer { nombre: "Hoja1", tabla: &vacia, valores: None }];
    let hojas = if hojas.is_empty() { &sola[..] } else { hojas };
    let activa = activa.min(hojas.len() - 1);
    let mut estilos = Estilos::default();
    let partes: Vec<String> = hojas.iter().map(|h| hoja_xml(h.tabla, h.valores, &mut estilos)).collect();

    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let op = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    let mut parte = |nombre: &str, datos: &[u8]| -> std::io::Result<()> {
        zip.start_file(nombre, op).map_err(std::io::Error::other)?;
        zip.write_all(datos)
    };
    let mut tipos = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>"#,
    );
    for i in 1..=hojas.len() {
        tipos.push_str(&format!(
            r#"<Override PartName="/xl/worksheets/sheet{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#
        ));
    }
    tipos.push_str("</Types>");
    parte("[Content_Types].xml", tipos.as_bytes())?;
    parte(
        "_rels/.rels",
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
    )?;
    let mut libro = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><bookViews><workbookView activeTab="{activa}"/></bookViews><sheets>"#
    );
    let mut rels = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for (i, h) in hojas.iter().enumerate() {
        let n = i + 1;
        let nombre = if h.nombre.trim().is_empty() { format!("Hoja{n}") } else { h.nombre.to_string() };
        libro.push_str(&format!(r#"<sheet name="{}" sheetId="{n}" r:id="rId{n}"/>"#, escapar(&nombre)));
        rels.push_str(&format!(
            r#"<Relationship Id="rId{n}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{n}.xml"/>"#
        ));
    }
    libro.push_str("</sheets></workbook>");
    let n = hojas.len() + 1;
    rels.push_str(&format!(
        r#"<Relationship Id="rId{n}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#
    ));
    parte("xl/workbook.xml", libro.as_bytes())?;
    parte("xl/_rels/workbook.xml.rels", rels.as_bytes())?;
    parte("xl/styles.xml", estilos.xml().as_bytes())?;
    for (i, x) in partes.iter().enumerate() {
        parte(&format!("xl/worksheets/sheet{}.xml", i + 1), x.as_bytes())?;
    }
    Ok(zip.finish().map_err(std::io::Error::other)?.into_inner())
}

/// Los formatos de celda que van saliendo (negrita, alineacion y fondo),
/// cada uno una vez: el indice es su `s` en las celdas.
#[derive(Default)]
struct Estilos {
    xfs: Vec<(bool, Option<char>, Option<String>)>,
}

impl Estilos {
    /// El `s` de una celda con este estilo (0, el de siempre, si no tiene).
    fn de(&mut self, e: Option<&EstiloDeCelda>) -> usize {
        let Some(e) = e else { return 0 };
        let alinea = e.a.as_deref().and_then(|a| a.chars().next()).filter(|c| matches!(c, 'i' | 'c' | 'd'));
        let fondo = e.f.as_deref().and_then(color_rgb);
        if !e.n && alinea.is_none() && fondo.is_none() {
            return 0;
        }
        let clave = (e.n, alinea, fondo);
        match self.xfs.iter().position(|x| *x == clave) {
            Some(i) => i + 1,
            None => {
                self.xfs.push(clave);
                self.xfs.len()
            }
        }
    }

    fn xml(&self) -> String {
        let fondos: Vec<&String> = {
            let mut v: Vec<&String> = self.xfs.iter().filter_map(|x| x.2.as_ref()).collect();
            v.sort();
            v.dedup();
            v
        };
        let mut s = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><name val="Calibri"/></font></fonts>"#,
        );
        s.push_str(&format!(
            r#"<fills count="{}"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill>"#,
            fondos.len() + 2
        ));
        for f in &fondos {
            s.push_str(&format!(r#"<fill><patternFill patternType="solid"><fgColor rgb="FF{f}"/></patternFill></fill>"#));
        }
        s.push_str(r#"</fills><borders count="1"><border/></borders>"#);
        s.push_str(&format!(r#"<cellXfs count="{}"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/>"#, self.xfs.len() + 1));
        for (negrita, alinea, fondo) in &self.xfs {
            let fill = fondo.as_ref().and_then(|f| fondos.iter().position(|x| *x == f)).map_or(0, |i| i + 2);
            s.push_str(&format!(
                r#"<xf numFmtId="0" fontId="{}" fillId="{fill}" borderId="0""#,
                if *negrita { 1 } else { 0 }
            ));
            match alinea {
                Some(a) => {
                    let h = match a {
                        'c' => "center",
                        'd' => "right",
                        _ => "left",
                    };
                    s.push_str(&format!(r#" applyAlignment="1"><alignment horizontal="{h}"/></xf>"#));
                }
                None => s.push_str("/>"),
            }
        }
        s.push_str("</cellXfs></styleSheet>");
        s
    }
}

/// `#rrggbb` (el `f` del movil) a `RRGGBB`. Lo que no tiene esa forma no
/// se pinta.
fn color_rgb(f: &str) -> Option<String> {
    let h = f.trim().strip_prefix('#')?;
    (h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit())).then(|| h.to_ascii_uppercase())
}

/// Lo que se puede escribir en XML: sin los caracteres de control que el
/// XML 1.0 no admite, y con `& < > "` como entidades.
fn escapar(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\t' | '\n' | '\r' => o.push(c),
            c if (c as u32) < 0x20 => {}
            c => o.push(c),
        }
    }
    o
}

/// Un numero como lo guarda Excel: el mas corto que vuelve al mismo valor,
/// sin notacion cientifica.
fn numero(n: f64) -> String {
    if n.is_finite() { format!("{n}") } else { "0".into() }
}

/// La hoja de una tabla: sus anchos y sus celdas, fila a fila.
fn hoja_xml(t: &Tabla, hechos: Option<&BTreeMap<String, Valor>>, estilos: &mut Estilos) -> String {
    let calculados;
    let valores = match hechos {
        Some(v) => v,
        None => {
            calculados = formula::evaluar_todo(t);
            &calculados
        }
    };
    // Por filas y, dentro, por columnas: es el orden que pide SpreadsheetML.
    let mut celdas: Vec<(Ref, &String)> = t.celdas.keys().filter_map(|k| Some((ref_de(k)?, k))).collect();
    celdas.sort_by_key(|(r, _)| (r.fila, r.columna));
    let (columnas, filas) = t.tamano();
    let mut s = String::with_capacity(64 * celdas.len() + 512);
    s.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#);
    if columnas > 0 && filas > 0 {
        s.push_str(&format!(r#"<dimension ref="A1:{}"/>"#, ref_a(Ref { columna: columnas - 1, fila: filas - 1 })));
    }
    s.push_str(&format!(
        r#"<sheetFormatPr defaultColWidth="{}" defaultRowHeight="{}" customHeight="1"/>"#,
        ANCHO_COLUMNA as f64 / DIGITO,
        ALTO_FILA as f64 * 0.75
    ));
    // Los anchos puestos a mano, por columna (`B` -> 180 px).
    let anchos: BTreeMap<u32, u32> = t
        .anchos
        .iter()
        .filter_map(|(letra, px)| Some((ref_de(&format!("{letra}1"))?.columna, *px)))
        .filter(|(_, px)| *px > 0)
        .collect();
    if !anchos.is_empty() {
        s.push_str("<cols>");
        for (c, px) in &anchos {
            let n = c + 1;
            s.push_str(&format!(r#"<col min="{n}" max="{n}" width="{}" customWidth="1"/>"#, *px as f64 / DIGITO));
        }
        s.push_str("</cols>");
    }
    s.push_str("<sheetData>");
    let mut fila_abierta: Option<u32> = None;
    for (r, clave) in celdas {
        if fila_abierta != Some(r.fila) {
            if fila_abierta.is_some() {
                s.push_str("</row>");
            }
            s.push_str(&format!(r#"<row r="{}">"#, r.fila + 1));
            fila_abierta = Some(r.fila);
        }
        let estilo = estilos.de(t.estilos.get(clave));
        let st = if estilo > 0 { format!(r#" s="{estilo}""#) } else { String::new() };
        match valores.get(clave) {
            Some(Valor::Numero(n)) => s.push_str(&format!(r#"<c r="{clave}"{st}><v>{}</v></c>"#, numero(*n))),
            Some(Valor::Error(e)) => s.push_str(&format!(r#"<c r="{clave}"{st} t="e"><v>{}</v></c>"#, escapar(&e.to_string()))),
            Some(Valor::Texto(texto)) if !texto.is_empty() => {
                s.push_str(&format!(r#"<c r="{clave}"{st} t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#, escapar(texto)))
            }
            // Vacia pero con estilo: el fondo se ve igual.
            _ if estilo > 0 => s.push_str(&format!(r#"<c r="{clave}"{st}/>"#)),
            _ => {}
        }
    }
    if fila_abierta.is_some() {
        s.push_str("</row>");
    }
    s.push_str("</sheetData></worksheet>");
    s
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::io::Read;

    fn parte(bytes: &[u8], nombre: &str) -> String {
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut s = String::new();
        z.by_name(nombre).unwrap().read_to_string(&mut s).unwrap();
        s
    }

    fn tabla() -> Tabla {
        let mut t = Tabla::default();
        for (k, v) in [("A1", "Concepto"), ("B1", "Importe"), ("A2", "Pan & <vino>"), ("B2", "12,5"), ("B3", "7"), ("B4", "=SUMA(B2:B3)"), ("C1", "=1/0")] {
            t.poner(ref_de(k).unwrap(), v);
        }
        t.estilos.insert("A1".into(), EstiloDeCelda { n: true, f: Some("#fff2cc".into()), ..Default::default() });
        t.estilos.insert("B1".into(), EstiloDeCelda { a: Some("d".into()), ..Default::default() });
        t.anchos.insert("A".into(), 180);
        t
    }

    #[test]
    fn la_tabla_sale_con_sus_valores_calculados_y_su_formato() {
        let t = tabla();
        let x = a_xlsx(&[HojaParaVer { nombre: "Gastos", tabla: &t, valores: None }], 0).unwrap();
        let hoja = parte(&x, "xl/worksheets/sheet1.xml");
        // La formula va con su RESULTADO: el visor no calcula.
        let total = formula::evaluar(&t, ref_de("B4").unwrap());
        assert!(matches!(total, Valor::Numero(_)), "{total:?}");
        assert!(hoja.contains(&format!(r#"<c r="B4"><v>{}</v></c>"#, numero(match total { Valor::Numero(n) => n, _ => 0.0 }))), "{hoja}");
        assert!(hoja.contains("Pan &amp; &lt;vino&gt;"), "el texto se escapa");
        assert!(hoja.contains(r#"<c r="C1" t="e">"#), "un error es un error, no un texto");
        assert!(hoja.contains(r#"<col min="1" max="1" width=""#), "el ancho de la A");
        // Las filas en orden y cada una una vez.
        assert!(hoja.find(r#"<row r="1">"#).unwrap() < hoja.find(r#"<row r="2">"#).unwrap());
        assert_eq!(hoja.matches(r#"<row r="1">"#).count(), 1);
        let estilos = parte(&x, "xl/styles.xml");
        assert!(estilos.contains(r#"<fgColor rgb="FFFFF2CC"/>"#), "{estilos}");
        assert!(estilos.contains(r#"horizontal="right""#));
        let libro = parte(&x, "xl/workbook.xml");
        assert!(libro.contains(r#"name="Gastos""#));
    }

    #[test]
    fn varias_hojas_con_la_activa_y_sin_hojas_una_vacia() {
        let t = tabla();
        let otra = Tabla::default();
        let x = a_xlsx(&[HojaParaVer { nombre: "A", tabla: &t, valores: None }, HojaParaVer { nombre: "", tabla: &otra, valores: None }], 7).unwrap();
        let libro = parte(&x, "xl/workbook.xml");
        assert!(libro.contains(r#"activeTab="1""#), "la activa se acota a las que hay");
        assert!(libro.contains(r#"name="Hoja2""#), "una hoja sin nombre se llama como en Excel");
        parte(&x, "xl/worksheets/sheet2.xml");
        // Caso negativo: sin hojas no sale un libro roto, sale una vacia.
        let x = a_xlsx(&[], 0).unwrap();
        assert!(parte(&x, "xl/worksheets/sheet1.xml").contains("<sheetData></sheetData>"));
    }

    #[test]
    fn lo_que_no_es_un_color_o_un_control_no_se_escribe() {
        assert_eq!(color_rgb("#a1B2c3").as_deref(), Some("A1B2C3"));
        for malo in ["", "a1b2c3", "#12345", "#gggggg", "#1234567"] {
            assert!(color_rgb(malo).is_none(), "{malo}");
        }
        assert_eq!(escapar("a\u{1}b\tc\"d"), "ab\tc&quot;d");
        let mut e = Estilos::default();
        // Un estilo que no cambia nada que se vea no gasta un formato.
        assert_eq!(e.de(Some(&EstiloDeCelda { e: true, ..Default::default() })), 0);
        assert_eq!(e.de(None), 0);
        let b = EstiloDeCelda { n: true, ..Default::default() };
        assert_eq!(e.de(Some(&b)), 1);
        assert_eq!(e.de(Some(&b)), 1, "el mismo formato se reutiliza");
    }
}
