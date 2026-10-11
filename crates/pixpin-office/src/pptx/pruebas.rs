//! Pruebas con presentaciones fabricadas en la propia prueba.

use super::*;
use crate::docx::pruebas::zip;
use crate::medidor::MedidorFijo;

const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;
const RELS: &str = r#"xmlns="http://schemas.openxmlformats.org/package/2006/relationships""#;
const T: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn presentacion(diapositiva: &str) -> Presentacion {
    let raiz_rels =
        format!(r#"<Relationships {RELS}><Relationship Id="r1" Type="{T}/officeDocument" Target="ppt/presentation.xml"/></Relationships>"#);
    let pres = format!(
        r#"<p:presentation {NS}><p:sldIdLst><p:sldId id="256" r:id="rS"/></p:sldIdLst><p:sldSz cx="9144000" cy="5143500"/></p:presentation>"#
    );
    let pres_rels = format!(r#"<Relationships {RELS}><Relationship Id="rS" Type="{T}/slide" Target="slides/slide1.xml"/></Relationships>"#);
    let slide_rels =
        format!(r#"<Relationships {RELS}><Relationship Id="rL" Type="{T}/slideLayout" Target="../slideLayouts/slideLayout1.xml"/></Relationships>"#);
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
    let b = zip(&[
        ("_rels/.rels", raiz_rels.as_str()),
        ("ppt/presentation.xml", pres.as_str()),
        ("ppt/_rels/presentation.xml.rels", pres_rels.as_str()),
        ("ppt/slides/slide1.xml", slide.as_str()),
        ("ppt/slides/_rels/slide1.xml.rels", slide_rels.as_str()),
        ("ppt/slideLayouts/slideLayout1.xml", layout.as_str()),
        ("ppt/slideLayouts/_rels/slideLayout1.xml.rels", layout_rels.as_str()),
        ("ppt/slideMasters/slideMaster1.xml", master.as_str()),
    ]);
    Presentacion::de_paquete(Paquete::de_bytes(b).unwrap()).unwrap()
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
