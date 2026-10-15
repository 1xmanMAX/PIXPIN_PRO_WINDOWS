//! Varias paginas en un lienzo (`FusionDePaginasTest` del movil): que no se
//! pisen, que la separacion se note, que el conjunto salga cuadrado y que
//! repartir los pixeles no deje una pagina con la mitad de detalle.

use super::*;

const A4: (f64, f64) = (595.0, 842.0);
const A4_APAISADO: (f64, f64) = (842.0, 595.0);

#[test]
fn las_paginas_verticales_van_en_fila_y_las_apaisadas_en_columna() {
    assert!(en_fila(&[A4; 3]));
    assert!(!en_fila(&[A4_APAISADO; 3]));
}

#[test]
fn una_pagina_distinta_no_manda_sobre_las_demas() {
    assert!(en_fila(&[A4_APAISADO, A4, A4, A4]));
}

#[test]
fn en_fila_las_paginas_se_separan_y_no_se_pisan() {
    let t = [(1000.0, 1400.0); 3];
    let hueco = separacion(&t);
    assert!(hueco > 20.0, "la separacion se tiene que notar");
    let s = sitios(&t, true);
    assert_eq!(s[1].x, 1000.0 + hueco);
    assert_eq!(s[2].x, 2.0 * (1000.0 + hueco));
    assert!(s.iter().all(|x| x.y == 0.0));
    assert!(
        s.windows(2).all(|w| w[0].x + w[0].ancho < w[1].x),
        "se pisan"
    );
}

#[test]
fn en_columna_se_apilan_con_el_mismo_hueco() {
    let t = [(1000.0, 700.0), (1000.0, 900.0)];
    let s = sitios(&t, false);
    assert_eq!(s[1].y, 700.0 + separacion(&t));
    assert!(s.iter().all(|x| x.x == 0.0));
}

#[test]
fn una_pagina_del_doble_de_ancha_se_lleva_el_doble_de_pixeles() {
    let a = anchos(&[(500.0, 800.0), (1000.0, 800.0)]);
    assert!((a[1] as f64 / a[0] as f64 - 2.0).abs() < 0.05);
}

#[test]
fn entre_todas_no_se_pasan_del_presupuesto() {
    let m = [(2384.0, 3370.0); 6];
    let a = anchos(&m);
    let pixeles: f64 = a
        .iter()
        .zip(&m)
        .map(|(w, p)| *w as f64 * (*w as f64 * p.1 / p.0))
        .sum();
    assert!(pixeles <= PIXELES_EN_TOTAL * 1.05, "son {pixeles}");
    assert!(a.iter().all(|w| *w as f64 >= ANCHO_MINIMO));
}

#[test]
fn dos_paginas_no_se_pintan_mas_grandes_de_lo_que_sirve() {
    assert_eq!(anchos(&[A4; 2])[0] as f64, ANCHO_MAXIMO);
    assert!(anchos(&[]).is_empty());
}

#[test]
fn el_nombre_dice_que_paginas_son() {
    assert_eq!(nombre(&[3, 4, 5]), "P\u{e1}ginas 4 a 6");
    assert_eq!(nombre(&[3, 5, 8]), "P\u{e1}ginas 4, 6 y 9");
    assert_eq!(nombre(&[1]), "P\u{e1}gina 2");
}

#[test]
fn cada_pagina_dice_de_que_fusion_viene() {
    assert_eq!(rotulo(&[3, 4, 5], 4), "4 a 6 \u{b7} p\u{e1}g. 5");
    assert_eq!(rotulo(&[3, 5, 8], 8), "4, 6 y 9 \u{b7} p\u{e1}g. 9");
}

fn pagina(n: u32) -> Mensaje {
    Mensaje {
        pagina: Some(n),
        ..Default::default()
    }
}

#[test]
fn solo_dos_o_mas_paginas_del_pdf_se_pueden_fusionar() {
    let dir = std::env::temp_dir().join(format!("pixpin-fusion-de-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pdf = dir.join("doc.pdf");
    std::fs::write(&pdf, b"%PDF").unwrap();
    let (a, b, c) = (pagina(5), pagina(3), pagina(5));
    let p = de(Some(pdf.clone()), "f", &[&a, &b, &c]).expect("dos paginas distintas");
    assert_eq!(p.paginas, vec![3, 5], "sin repetir y en orden");
    // Casos negativos: una sola pagina, algo que no es pagina, sin PDF.
    assert!(de(Some(pdf.clone()), "f", &[&a, &c]).is_none());
    let lienzo = Mensaje::default();
    assert!(de(Some(pdf.clone()), "f", &[&a, &b, &lienzo]).is_none());
    let mut nota = pagina(7);
    nota.resto
        .insert("nota".into(), serde_json::Value::String("x".into()));
    assert!(de(Some(pdf.clone()), "f", &[&a, &nota]).is_none());
    assert!(de(None, "f", &[&a, &b]).is_none());
    assert!(de(Some(dir.join("no-esta.pdf")), "f", &[&a, &b]).is_none());
    let muchas: Vec<Mensaje> = (0..13).map(pagina).collect();
    let refs: Vec<&Mensaje> = muchas.iter().collect();
    assert!(de(Some(pdf), "f", &refs).is_none(), "mas de doce no caben");
    let _ = std::fs::remove_dir_all(&dir);
}

/// El trabajo entero con un PDF de verdad: un lienzo nuevo con un marco y su
/// pagina por cada una, con candado, y una hoja mas en el proyecto.
#[test]
fn fusionar_deja_un_lienzo_con_un_marco_por_pagina_y_una_hoja_mas() {
    let raiz = std::env::temp_dir().join(format!("pixpin-fusion-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    let carpeta = almacen::carpeta(&raiz, "p1");
    std::fs::create_dir_all(carpeta.join("archivos")).unwrap();
    let hoja = |g: u8, w: u32, h: u32| pixpin_codec::imagen::ImagenRgba {
        ancho: w,
        alto: h,
        pixeles: [g, g, g, 255].repeat((w * h) as usize),
    };
    let pdf = carpeta.join("archivos/doc-1.pdf");
    std::fs::write(
        &pdf,
        pixpin_pdf::union::de_imagenes(&[hoja(10, 60, 80), hoja(90, 60, 80), hoja(200, 80, 60)])
            .unwrap(),
    )
    .unwrap();
    let p = Proyecto {
        id: "p1".into(),
        nombre: "Obra".into(),
        pdf_origen: Some("archivos/doc-1.pdf".into()),
        ..Default::default()
    };
    std::fs::write(
        carpeta.join("proyecto.json"),
        serde_json::to_string(&p).unwrap(),
    )
    .unwrap();
    let peticion = Peticion {
        ficha: "p1".into(),
        pdf,
        paginas: vec![0, 2],
    };
    let _com = pixpin_shell::ComDelHilo::iniciar();
    assert_eq!(
        en_un_lienzo(&raiz, &peticion, 77).unwrap(),
        "P\u{e1}ginas 1 y 3"
    );
    let p: Proyecto =
        serde_json::from_str(&std::fs::read_to_string(carpeta.join("proyecto.json")).unwrap())
            .unwrap();
    let h = p.hojas.last().unwrap();
    assert_eq!(h.dibujo.as_deref(), Some("dib-fus-77"));
    assert!(h.uid.is_some(), "con su codigo, para salir en el chat");
    let texto = std::fs::read_to_string(almacen::lienzo(&raiz, "p1", "dib-fus-77")).unwrap();
    let elementos = pixpin_motor2d::excalidraw::leer(&texto)
        .expect("el lienzo se lee")
        .elementos();
    let marcos = elementos
        .iter()
        .filter(|e| matches!(&e.figura, pixpin_motor2d::Figura::Marco { nombre } if nombre.contains("p\u{e1}g.")))
        .count();
    assert_eq!(marcos, 2);
    assert_eq!(elementos.len(), 4, "dos marcos y dos paginas");
    assert!(elementos.iter().all(|e| e.bloqueado), "todo con candado");
    assert!(carpeta.join("imagenes/fus77x0").is_file());
    let _ = std::fs::remove_dir_all(&raiz);
}
