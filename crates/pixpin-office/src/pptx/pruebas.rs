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
            Elemento::Efecto { elementos, .. } => textos(elementos, dx, dy, v),
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

/// Una forma de texto en (0, 0) de `alto` puntos con `n` parrafos de 18 pt
/// y el `bodyPr` dado.
fn caja_de_texto(body_pr: &str, alto: f32, n: usize) -> String {
    let parrafos: String = (0..n).map(|i| format!(r#"<a:p><a:r><a:rPr lang="es" sz="1800"/><a:t>linea {i}</a:t></a:r></a:p>"#)).collect();
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="3" name="C"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="2540000" cy="{}"/></a:xfrm><a:prstGeom prst="rect"/></p:spPr><p:txBody>{body_pr}{parrafos}</p:txBody></p:sp>"#,
        (alto * 12700.0) as i64
    )
}

fn textos_de(forma: &str) -> Vec<crate::pagina::Texto> {
    let mut p = presentacion(forma);
    let d = p.diapositiva(0, &MedidorFijo::default()).unwrap();
    let mut v = Vec::new();
    textos(&d.elementos, 0.0, 0.0, &mut v);
    v
}

#[test]
fn el_autoajuste_con_fontscale_reduce_la_letra_y_el_interlineado() {
    let sin = textos_de(&caja_de_texto(r#"<a:bodyPr><a:normAutofit fontScale="50000"/></a:bodyPr>"#, 300.0, 3));
    let con = textos_de(&caja_de_texto(r#"<a:bodyPr><a:normAutofit fontScale="50000" lnSpcReduction="20000"/></a:bodyPr>"#, 300.0, 3));
    assert_eq!(sin.len(), 3);
    assert!(sin.iter().chain(&con).all(|t| (t.letra.tam - 9.0).abs() < 1e-4), "{:?}", sin.iter().map(|t| t.letra.tam).collect::<Vec<_>>());
    // El renglon de 9 pt mide 10,8; con un 20 % menos, 8,64.
    let paso_sin = sin[1].y - sin[0].y;
    let paso_con = con[1].y - con[0].y;
    assert!((paso_sin - 10.8).abs() < 0.01, "{paso_sin}");
    assert!((paso_con - 8.64).abs() < 0.01, "{paso_con}");
}

#[test]
fn el_autoajuste_sin_fontscale_reduce_hasta_que_quepa() {
    // Diez renglones de 18 pt (216 pt) en una caja de 100.
    let v = textos_de(&caja_de_texto(r#"<a:bodyPr><a:normAutofit/></a:bodyPr>"#, 100.0, 10));
    assert_eq!(v.len(), 10);
    let tam = v[0].letra.tam;
    assert!(tam < 18.0, "{tam}");
    let abajo = v.last().map(|t| t.y + t.letra.tam * 0.3).unwrap_or(0.0);
    assert!(abajo <= 100.0, "se sale: {abajo}");
    // Si cabe, no se toca.
    let v = textos_de(&caja_de_texto(r#"<a:bodyPr><a:normAutofit/></a:bodyPr>"#, 300.0, 3));
    assert!(v.iter().all(|t| t.letra.tam == 18.0));
}

#[test]
fn spautofit_y_noautofit_no_reducen() {
    for cuerpo in [r#"<a:bodyPr><a:spAutoFit/></a:bodyPr>"#, r#"<a:bodyPr><a:noAutofit/></a:bodyPr>"#, r#"<a:bodyPr/>"#] {
        let v = textos_de(&caja_de_texto(cuerpo, 100.0, 10));
        assert_eq!(v.len(), 10);
        assert!(v.iter().all(|t| t.letra.tam == 18.0), "{cuerpo}");
    }
}

#[test]
fn la_sombra_de_una_forma_envuelve_la_forma() {
    let forma = r#"<p:sp><p:nvSpPr><p:cNvPr id="3" name="S"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="127000" y="127000"/><a:ext cx="1270000" cy="635000"/></a:xfrm><a:prstGeom prst="rect"/><a:solidFill><a:srgbClr val="4472C4"/></a:solidFill><a:effectLst><a:outerShdw blurRad="50800" dist="38100" dir="5400000" rotWithShape="0"><a:srgbClr val="000000"><a:alpha val="50000"/></a:srgbClr></a:outerShdw></a:effectLst></p:spPr><p:txBody><a:bodyPr/><a:p><a:r><a:rPr lang="es"><a:effectLst><a:glow rad="63500"><a:srgbClr val="FFC000"/></a:glow></a:effectLst></a:rPr><a:t>Hola</a:t></a:r></a:p></p:txBody></p:sp>
    <p:sp><p:nvSpPr><p:cNvPr id="4" name="N"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="127000" cy="127000"/></a:xfrm><a:prstGeom prst="rect"/><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></p:spPr><p:style><a:lnRef idx="0"/><a:fillRef idx="1"/><a:effectRef idx="2"/><a:fontRef idx="minor"/></p:style></p:sp>"#;
    let mut p = presentacion(forma);
    let d = p.diapositiva(0, &MedidorFijo::default()).unwrap();
    let efectos: Vec<&Elemento> = d.elementos.iter().filter(|e| matches!(e, Elemento::Efecto { .. })).collect();
    assert_eq!(efectos.len(), 1, "solo la forma con effectLst; el effectRef sin efectos en el tema no envuelve nada");
    let Elemento::Efecto { efectos: lista, elementos } = efectos[0] else { unreachable!() };
    let [crate::pagina::Efecto::SombraExterior { dx, dy, desenfoque, .. }] = lista.as_slice() else { panic!("{lista:?}") };
    assert!(dx.abs() < 1e-4 && (dy - 3.0).abs() < 1e-4 && (desenfoque - 4.0).abs() < 1e-4);
    assert!(matches!(elementos.as_slice(), [Elemento::Grupo { .. }]));
    // El texto con brillo sigue encontrandose (buscar, seleccionar).
    assert_eq!(d.textos().iter().map(|(t, _)| t.texto.as_str()).collect::<Vec<_>>(), vec!["Hola"]);
    let mut v = Vec::new();
    textos(&d.elementos, 0.0, 0.0, &mut v);
    assert_eq!(v.len(), 1);
}
