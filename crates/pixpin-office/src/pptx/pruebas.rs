//! Pruebas con presentaciones fabricadas en la propia prueba.

use super::*;
use crate::docx::pruebas::zip;
use crate::medidor::MedidorFijo;

const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;
const RELS: &str = r#"xmlns="http://schemas.openxmlformats.org/package/2006/relationships""#;
const T: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn presentacion(diapositiva: &str) -> Presentacion {
    presentacion_con(diapositiva, "", &[])
}

/// Una presentacion con relaciones de mas en la diapositiva y partes de mas.
fn presentacion_con(diapositiva: &str, rels_diapositiva: &str, extra: &[(&str, &str)]) -> Presentacion {
    let raiz_rels =
        format!(r#"<Relationships {RELS}><Relationship Id="r1" Type="{T}/officeDocument" Target="ppt/presentation.xml"/></Relationships>"#);
    let pres = format!(
        r#"<p:presentation {NS}><p:sldIdLst><p:sldId id="256" r:id="rS"/></p:sldIdLst><p:sldSz cx="9144000" cy="5143500"/></p:presentation>"#
    );
    let pres_rels = format!(r#"<Relationships {RELS}><Relationship Id="rS" Type="{T}/slide" Target="slides/slide1.xml"/></Relationships>"#);
    let slide_rels =
        format!(r#"<Relationships {RELS}><Relationship Id="rL" Type="{T}/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>{rels_diapositiva}</Relationships>"#);
    let layout = format!(
        r#"<p:sldLayout {NS}><p:cSld><p:spTree>
        <p:sp><p:nvSpPr><p:cNvPr id="2" name="T"/><p:cNvSpPr/><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="1270000" y="635000"/><a:ext cx="2540000" cy="1270000"/></a:xfrm></p:spPr><p:txBody><a:bodyPr/><a:p><a:r><a:t>Titulo de muestra</a:t></a:r></a:p></p:txBody></p:sp>
        </p:spTree></p:cSld></p:sldLayout>"#
    );
    let layout_rels =
        format!(r#"<Relationships {RELS}><Relationship Id="rM" Type="{T}/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#);
    let master = format!(
        r#"<p:sldMaster {NS}><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="102030"/></a:solidFill></p:bgPr></p:bg><p:spTree>
        <p:sp><p:nvSpPr><p:cNvPr id="9" name="Logo"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="127000" cy="127000"/></a:xfrm><a:prstGeom prst="ellipse"/><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></p:spPr></p:sp>
        </p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
        <p:txStyles><p:titleStyle><a:lvl1pPr algn="ctr"><a:defRPr sz="4400"><a:solidFill><a:srgbClr val="00FF00"/></a:solidFill></a:defRPr></a:lvl1pPr></p:titleStyle></p:txStyles></p:sldMaster>"#
    );
    let slide = format!(r#"<p:sld {NS}><p:cSld><p:spTree>{diapositiva}</p:spTree></p:cSld></p:sld>"#);
    let mut partes = vec![
        ("_rels/.rels", raiz_rels.as_str()),
        ("ppt/presentation.xml", pres.as_str()),
        ("ppt/_rels/presentation.xml.rels", pres_rels.as_str()),
        ("ppt/slides/slide1.xml", slide.as_str()),
        ("ppt/slides/_rels/slide1.xml.rels", slide_rels.as_str()),
        ("ppt/slideLayouts/slideLayout1.xml", layout.as_str()),
        ("ppt/slideLayouts/_rels/slideLayout1.xml.rels", layout_rels.as_str()),
        ("ppt/slideMasters/slideMaster1.xml", master.as_str()),
    ];
    partes.extend_from_slice(extra);
    let b = zip(&partes);
    Presentacion::de_paquete(Paquete::de_bytes(b).unwrap()).unwrap()
}

/// Un grafico de columnas minimo (dos categorias, una serie).
const GRAFICO: &str = r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><c:chart><c:autoTitleDeleted val="1"/><c:plotArea><c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:ser><c:idx val="0"/><c:order val="0"/><c:cat><c:strRef><c:strCache><c:ptCount val="2"/><c:pt idx="0"><c:v>Uvas</c:v></c:pt><c:pt idx="1"><c:v>Peras</c:v></c:pt></c:strCache></c:strRef></c:cat><c:val><c:numRef><c:numCache><c:ptCount val="2"/><c:pt idx="0"><c:v>3</c:v></c:pt><c:pt idx="1"><c:v>5</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser><c:axId val="1"/><c:axId val="2"/></c:barChart><c:catAx><c:axId val="1"/><c:axPos val="b"/><c:crossAx val="2"/></c:catAx><c:valAx><c:axId val="2"/><c:axPos val="l"/><c:crossAx val="1"/></c:valAx></c:plotArea></c:chart></c:chartSpace>"#;

fn marco(uri: &str, rid: &str) -> String {
    format!(
        r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="4" name="G"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="1270000" y="1270000"/><a:ext cx="5080000" cy="3810000"/></p:xfrm><a:graphic><a:graphicData uri="{uri}"><c:chart xmlns:c="x" r:id="{rid}"/></a:graphicData></a:graphic></p:graphicFrame>"#
    )
}

#[test]
fn un_grafico_se_dibuja_con_su_cache() {
    let rel = format!(r#"<Relationship Id="rC" Type="{T}/chart" Target="../charts/chart1.xml"/>"#);
    let mut p = presentacion_con(&marco("http://schemas.openxmlformats.org/drawingml/2006/chart", "rC"), &rel, &[("ppt/charts/chart1.xml", GRAFICO)]);
    let d = p.diapositiva(0, &MedidorFijo::default()).unwrap();
    let mut v = Vec::new();
    textos(&d.elementos, 0.0, 0.0, &mut v);
    let t: Vec<&str> = v.iter().map(|t| t.texto.as_str()).collect();
    assert!(t.contains(&"Uvas") && t.contains(&"Peras"), "{t:?}");
    // Dentro del marco (de 100 a 500 pt de ancho y de 100 a 400 de alto).
    for x in &v {
        assert!(x.x >= 100.0 && x.x + x.ancho <= 500.0 && x.y >= 100.0 && x.y <= 400.0, "{x:?}");
    }
}

#[test]
fn un_grafico_que_falta_o_es_nuevo_sale_con_aviso() {
    // La relacion apunta a una parte que no existe.
    let rel = format!(r#"<Relationship Id="rC" Type="{T}/chart" Target="../charts/no.xml"/>"#);
    let mut p = presentacion_con(&marco("http://schemas.openxmlformats.org/drawingml/2006/chart", "rC"), &rel, &[]);
    let d = p.diapositiva(0, &MedidorFijo::default()).unwrap();
    let mut v = Vec::new();
    textos(&d.elementos, 0.0, 0.0, &mut v);
    assert_eq!(v.len(), 1);
    assert!(v[0].texto.starts_with("Gráfico"), "{:?}", v[0].texto);
    // Un cx:chart suelto, sin respaldo.
    let mut p = presentacion(&marco("http://schemas.microsoft.com/office/drawing/2014/chartex", "rX"));
    let d = p.diapositiva(0, &MedidorFijo::default()).unwrap();
    let mut v = Vec::new();
    textos(&d.elementos, 0.0, 0.0, &mut v);
    assert!(v.iter().any(|t| t.texto.contains("sin vista previa")), "{v:?}");
}

fn textos(els: &[Elemento], dx: f32, dy: f32, v: &mut Vec<crate::pagina::Texto>) {
    for e in els {
        match e {
            Elemento::Texto(t) => {
                let mut t = t.clone();
                t.x += dx;
                t.y += dy;
                v.push(t);
            }
            Elemento::Grupo { transformada, elementos, .. } => textos(elementos, dx + transformada[4], dy + transformada[5], v),
            _ => {}
        }
    }
}

#[test]
fn el_titulo_hereda_posicion_y_estilo_del_diseno_y_el_patron() {
    let mut p = presentacion(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="T"/><p:cNvSpPr/><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:p><a:r><a:rPr lang="es"/><a:t>Hola</a:t></a:r></a:p></p:txBody></p:sp>"#,
    );
    assert_eq!(p.cuantas(), 1);
    let d = p.diapositiva(0, &MedidorFijo::default()).unwrap();
    assert_eq!((d.ancho, d.alto), (720.0, 405.0));
    assert_eq!(d.fondo, Some(Relleno::Solido(Color::rgb(0x10, 0x20, 0x30))));
    let mut v = Vec::new();
    textos(&d.elementos, 0.0, 0.0, &mut v);
    assert_eq!(v.len(), 1, "el texto de muestra del diseno no se pinta");
    let t = &v[0];
    assert_eq!(t.texto, "Hola");
    assert_eq!(t.letra.tam, 44.0);
    assert_eq!(t.color, Color::rgb(0, 255, 0));
    // Centrado en la caja del diseno: x de 100 a 300 pt.
    let centro = t.x + t.ancho / 2.0;
    assert!((centro - 200.0).abs() < 1.0, "{centro}");
    // La forma del patron (no marcador) se pinta.
    assert!(
        d.elementos
            .iter()
            .any(|e| matches!(e, Elemento::Grupo { elementos, .. } if elementos.iter().any(|x| matches!(x, Elemento::Trayecto { .. }))))
    );
}

#[test]
fn una_presentacion_rota_no_entra_en_panico() {
    let mut p = presentacion(
        r#"<p:sp><p:spPr><a:xfrm><a:off x="a"/></a:xfrm><a:prstGeom prst="star5"/></p:spPr><p:txBody><a:p><a:pPr lvl="99"/><a:r><a:t>x"#,
    );
    let _ = p.diapositiva(0, &MedidorFijo::default());
    assert!(p.diapositiva(5, &MedidorFijo::default()).is_none());
}
