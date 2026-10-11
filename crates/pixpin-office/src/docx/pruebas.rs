//! Pruebas con documentos fabricados en la propia prueba.

use std::io::Write;
use std::sync::Arc;

use super::*;
use crate::medidor::MedidorFijo;
use crate::pagina::Elemento;

/// Un ZIP con las partes dadas (ruta, contenido).
pub fn zip(partes: &[(&str, &str)]) -> Arc<[u8]> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let op = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (ruta, texto) in partes {
            z.start_file(*ruta, op).unwrap();
            z.write_all(texto.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }
    Arc::from(buf.into_inner())
}

const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;

fn documento(cuerpo: &str, extra: &[(&str, &str)]) -> DocumentoWord {
    let doc = format!(r#"<?xml version="1.0"?><w:document {W}><w:body>{cuerpo}</w:body></w:document>"#);
    let mut partes = vec![("word/document.xml", doc.as_str())];
    partes.extend_from_slice(extra);
    let p = Paquete::de_bytes(zip(&partes)).unwrap();
    DocumentoWord::de_paquete(p).unwrap()
}

fn textos(p: &Pagina) -> Vec<String> {
    p.elementos
        .iter()
        .filter_map(|e| match e {
            Elemento::Texto(t) => Some(t.texto.clone()),
            _ => None,
        })
        .collect()
}

const SECCION_PEQUENA: &str = r#"<w:sectPr><w:pgSz w:w="2880" w:h="2880"/><w:pgMar w:top="720" w:right="720" w:bottom="720" w:left="720" w:header="0" w:footer="0"/></w:sectPr>"#;

#[test]
fn un_documento_vacio_tiene_una_pagina_en_blanco() {
    let mut d = documento("", &[]);
    let p = d.pagina(0, &MedidorFijo::default()).unwrap();
    assert_eq!((p.ancho, p.alto), (612.0, 792.0));
    assert!(d.terminado());
    assert_eq!(d.paginas_listas(), 1);
}

#[test]
fn el_tamano_y_los_margenes_vienen_de_la_seccion() {
    let mut d = documento(&format!(r#"<w:p><w:r><w:t>Hola</w:t></w:r></w:p>{SECCION_PEQUENA}"#), &[]);
    let p = d.pagina(0, &MedidorFijo::default()).unwrap();
    assert_eq!((p.ancho, p.alto), (144.0, 144.0));
    let Some(Elemento::Texto(t)) = p.elementos.first() else { panic!("{:?}", p.elementos) };
    assert_eq!(t.texto, "Hola");
    assert_eq!(t.x, 36.0, "el texto empieza en el margen");
    // Letra por defecto 10 pt: sube 8.
    assert_eq!(t.y, 36.0 + 8.0);
}

#[test]
fn el_texto_que_no_cabe_pasa_a_la_pagina_siguiente() {
    // Caja de 72 x 72 puntos; renglones de 10 pt: caben 7.
    let parrafos: String = (0..20).map(|i| format!("<w:p><w:r><w:t>p{i}</w:t></w:r></w:p>")).collect();
    let mut d = documento(&format!("{parrafos}{SECCION_PEQUENA}"), &[]);
    let m = MedidorFijo::default();
    let p0 = d.pagina(0, &m).unwrap();
    assert_eq!(d.paginas_listas(), 1, "solo se maqueta lo que hace falta");
    assert!(!d.terminado());
    assert_eq!(textos(&p0).len(), 7);
    d.maquetar_todo(&m);
    assert_eq!(d.paginas_listas(), 3);
    assert_eq!(textos(&d.pagina(2, &m).unwrap()), vec!["p14", "p15", "p16", "p17", "p18", "p19"]);
}

#[test]
fn el_salto_de_pagina_empieza_otra() {
    let mut d = documento(&format!(r#"<w:p><w:r><w:t>a</w:t><w:br w:type="page"/><w:t>b</w:t></w:r></w:p>{SECCION_PEQUENA}"#), &[]);
    let m = MedidorFijo::default();
    d.maquetar_todo(&m);
    assert_eq!(d.paginas_listas(), 2);
    assert_eq!(textos(&d.pagina(1, &m).unwrap()), vec!["b"]);
}

#[test]
fn los_estilos_y_la_numeracion_se_aplican() {
    let estilos = format!(r#"<w:styles {W}><w:style w:type="paragraph" w:styleId="T1"><w:rPr><w:b/><w:sz w:val="40"/></w:rPr></w:style></w:styles>"#);
    let numeracion = format!(
        r#"<w:numbering {W}><w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/><w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr></w:lvl></w:abstractNum><w:num w:numId="3"><w:abstractNumId w:val="1"/></w:num></w:numbering>"#
    );
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="r2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/></Relationships>"#;
    let cuerpo = r#"<w:p><w:pPr><w:pStyle w:val="T1"/></w:pPr><w:r><w:t>Titulo</w:t></w:r></w:p>
        <w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="3"/></w:numPr></w:pPr><w:r><w:t>uno</w:t></w:r></w:p>
        <w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="3"/></w:numPr></w:pPr><w:r><w:t>dos</w:t></w:r></w:p>"#;
    let mut d = documento(
        cuerpo,
        &[("word/styles.xml", estilos.as_str()), ("word/numbering.xml", numeracion.as_str()), ("word/_rels/document.xml.rels", rels)],
    );
    let p = d.pagina(0, &MedidorFijo::default()).unwrap();
    let t: Vec<&crate::pagina::Texto> = p.elementos.iter().filter_map(|e| if let Elemento::Texto(t) = e { Some(t) } else { None }).collect();
    assert_eq!(t[0].texto, "Titulo");
    assert!(t[0].letra.negrita);
    assert_eq!(t[0].letra.tam, 20.0);
    assert_eq!(t[1].texto, "1.");
    assert_eq!(t[1].x, 72.0 + 18.0, "el numero en la sangria francesa");
    assert_eq!(t[2].texto, "uno");
    assert_eq!(t[2].x, 72.0 + 36.0, "el texto en el tope de la sangria");
    assert_eq!(t[3].texto, "2.");
}

#[test]
fn una_tabla_pone_cada_celda_en_su_columna() {
    let cuerpo = r#"<w:tbl><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="8"/><w:insideV w:val="single" w:sz="4"/></w:tblBorders></w:tblPr>
        <w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="3000"/></w:tblGrid>
        <w:tr><w:tc><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:shd w:fill="FF0000"/></w:tcPr><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc></w:tr>
        <w:tr><w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t>C</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#;
    let mut d = documento(cuerpo, &[]);
    let p = d.pagina(0, &MedidorFijo::default()).unwrap();
    let pos =
        |s: &str| p.elementos.iter().find_map(|e| if let Elemento::Texto(t) = e { (t.texto == s).then_some((t.x, t.y)) } else { None }).unwrap();
    // Sin modo de compatibilidad 15, el borde sale 5,4 pt hacia fuera y el
    // texto queda en el margen.
    assert_eq!(pos("A").0, 72.0);
    assert_eq!(pos("B").0, 72.0 + 100.0);
    assert_eq!(pos("C").0, 72.0);
    assert!(pos("C").1 > pos("A").1);
    assert!(
        p.elementos
            .iter()
            .any(|e| matches!(e, Elemento::Rect { relleno: Some(crate::pagina::Relleno::Solido(c)), .. } if *c == Color::rgb(255, 0, 0)))
    );
    assert!(p.elementos.iter().any(|e| matches!(e, Elemento::Linea { .. })));
}

#[test]
fn una_fila_larga_se_parte_entre_paginas() {
    let parrafos: String = (0..15).map(|i| format!("<w:p><w:r><w:t>c{i}</w:t></w:r></w:p>")).collect();
    let cuerpo = format!(r#"<w:tbl><w:tblGrid><w:gridCol w:w="1000"/></w:tblGrid><w:tr><w:tc>{parrafos}</w:tc></w:tr></w:tbl>{SECCION_PEQUENA}"#);
    let mut d = documento(&cuerpo, &[]);
    let m = MedidorFijo::default();
    d.maquetar_todo(&m);
    assert_eq!(d.paginas_listas(), 3);
    assert_eq!(textos(&d.pagina(0, &m).unwrap()).len(), 7);
}

#[test]
fn el_numero_de_pagina_del_pie_cuenta() {
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rP" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/></Relationships>"#;
    let pie = format!(
        r#"<w:ftr {W}><w:p><w:r><w:t>Pag </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> PAGE </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>9</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:ftr>"#
    );
    let cuerpo = r#"<w:p><w:r><w:t>a</w:t><w:br w:type="page"/><w:t>b</w:t></w:r></w:p>
        <w:sectPr><w:footerReference w:type="default" r:id="rP"/><w:pgSz w:w="2880" w:h="2880"/><w:pgMar w:top="720" w:right="720" w:bottom="720" w:left="720" w:header="0" w:footer="200"/></w:sectPr>"#;
    let mut d = documento(cuerpo, &[("word/_rels/document.xml.rels", rels), ("word/footer1.xml", pie.as_str())]);
    let m = MedidorFijo::default();
    let p1 = d.pagina(1, &m).unwrap();
    let t = textos(&p1);
    assert!(t.contains(&"2".to_string()), "{t:?}");
    assert!(!t.contains(&"9".to_string()), "el resultado guardado no se ensena");
}

#[test]
fn un_documento_roto_no_entra_en_panico() {
    let base = format!(
        r#"<w:p><w:pPr><w:numPr><w:numId w:val="7"/></w:numPr><w:spacing w:line="-5" w:lineRule="exact"/></w:pPr><w:r><w:t>a</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:tcPr><w:gridSpan w:val="99"/><w:vMerge/></w:tcPr><w:p/></w:tc></w:tr></w:tbl>{SECCION_PEQUENA}"#
    );
    let doc = format!(r#"<w:document {W}><w:body>{base}</w:body></w:document>"#);
    let m = MedidorFijo::default();
    for corte in (0..doc.len()).step_by(7) {
        let mut t = doc[..corte].to_string();
        while !t.is_char_boundary(t.len()) {
            t.pop();
        }
        let p = Paquete::de_bytes(zip(&[("word/document.xml", t.as_str())])).unwrap();
        if let Ok(mut d) = DocumentoWord::de_paquete(p) {
            d.maquetar_todo(&m);
        }
    }
}
