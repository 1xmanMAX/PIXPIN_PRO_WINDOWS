//! Las pruebas de la hoja de compartir: **cada cosa en cada formato da un
//! fichero que se abre**, el peso que se dice es el de verdad, y lo que no
//! tiene nada que compartir no ofrece nada.
//!
//! Con `PIXPIN_MUESTRAS_COMPARTIR` puesta (una carpeta), las muestras se
//! quedan ahi para mirarlas; sin ella van a la temporal y se borran.

use super::*;
use pixpin_motor2d::{Elemento, Figura};
use pixpin_proyecto::almacen::{Ficha, Indice};
use pixpin_store::Idioma;
use pixpin_ui::hoja_compartir::Estado;

fn textos() -> Catalogo {
    Catalogo::nuevo(Idioma::Espanol)
}

/// Una carpeta propia para cada prueba.
fn carpeta(nombre: &str) -> (PathBuf, bool) {
    match std::env::var_os("PIXPIN_MUESTRAS_COMPARTIR") {
        Some(d) => {
            let c = PathBuf::from(d).join(nombre);
            let _ = std::fs::remove_dir_all(&c);
            std::fs::create_dir_all(&c).unwrap();
            (c, false)
        }
        None => {
            let c = std::env::temp_dir().join(format!("pixpin-compartir-{nombre}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&c);
            std::fs::create_dir_all(&c).unwrap();
            (c, true)
        }
    }
}

fn rectangulo(x: f32, y: f32, w: f32, h: f32) -> Elemento {
    Elemento {
        figura: Figura::Rectangulo,
        x,
        y,
        ancho: w,
        alto: h,
        ..Elemento::default()
    }
}

fn marco(nombre: &str, y: f32) -> Elemento {
    Elemento {
        figura: Figura::Marco { nombre: nombre.into() },
        x: 0.0,
        y,
        ancho: 300.0,
        alto: 200.0,
        ..Elemento::default()
    }
}

fn trazo(x: f32, y: f32) -> Elemento {
    Elemento {
        figura: Figura::Lapiz {
            puntos: (0..20)
                .map(|i| Punto2::nuevo(x + i as f32 * 6.0, y + (i % 3) as f32 * 4.0))
                .collect(),
            presiones: Vec::new(),
            opciones: Some(Default::default()),
        },
        grosor: 3.0,
        trazo: ColorRgba::opaco(0.9, 0.1, 0.1),
        ..Elemento::default()
    }
}

fn foto(ancho: u32, alto: u32) -> ImagenRgba {
    let mut pixeles = Vec::with_capacity((ancho * alto * 4) as usize);
    for y in 0..alto {
        for x in 0..ancho {
            pixeles.extend_from_slice(&[(x * 255 / ancho) as u8, (y * 255 / alto) as u8, 160, 255]);
        }
    }
    ImagenRgba { ancho, alto, pixeles }
}

/// El lienzo del editor: dos marcos con algo dentro y una foto.
fn lienzo_suelto() -> LienzoSuelto {
    let mut escena = Escena::nueva();
    escena.anadir(marco("Planta", 0.0));
    escena.anadir(rectangulo(20.0, 20.0, 100.0, 80.0));
    escena.anadir(marco("Alzado", 400.0));
    escena.anadir(rectangulo(30.0, 430.0, 120.0, 60.0));
    escena.anadir(Elemento {
        figura: Figura::Imagen { id_objeto: 7 },
        x: 160.0,
        y: 40.0,
        ancho: 100.0,
        alto: 80.0,
        ..Elemento::default()
    });
    let mut fotos = HashMap::new();
    fotos.insert(7, foto(100, 80));
    LienzoSuelto {
        escena,
        papel: None,
        fotos,
        nombre: "Casa".into(),
    }
}

/// Las paginas que se piden para un formato: las que la hoja dejaria al
/// elegirlo, y todas si con eso no hay ninguna.
fn elegidas_para(c: &Compartible, i: usize) -> Vec<String> {
    let mut e = Estado::nuevo(c);
    e.elegir_formato(c, i);
    if !e.hay_que_preparar(c) {
        e.todas(c);
        e.elegir_formato(c, i);
    }
    e.elegidas(c)
}

/// **Lo que hace que un fichero sea valido**, segun su formato.
fn comprobar(salida: &Salida, formato: &str, paginas: usize) {
    assert!(!salida.ficheros.is_empty(), "{formato}: sin ficheros");
    let suma: u64 = salida
        .ficheros
        .iter()
        .map(|r| std::fs::metadata(r).expect("existe").len())
        .sum();
    assert_eq!(salida.bytes, suma, "{formato}: el peso que se dice es el de verdad");
    assert!(salida.bytes > 0, "{formato}: vacio");
    let r = &salida.ficheros[0];
    let bytes = std::fs::read(r).unwrap();
    match formato {
        WEB => {
            let s = String::from_utf8(bytes).expect("utf8");
            assert!(s.starts_with("<!DOCTYPE html>"), "web sin cabecera");
            assert!(s.trim_end().ends_with("</html>"), "web cortada");
            // Un documento que se lee va entero en una hoja, como en el
            // lector: sus paginas son una columna, no hojas sueltas.
            let hojas = if s.contains("class=\"doc-caja\"") { 1 } else { paginas };
            assert_eq!(s.matches("class=\"hoja\"").count(), hojas, "una hoja por pagina");
            assert!(s.contains("id=\"lapiz\""), "con los mandos de dibujar");
        }
        PDF => {
            assert!(bytes.starts_with(b"%PDF"));
            let doc = pixpin_pdf::Documento::abrir(r).expect("el PDF abre");
            assert_eq!(doc.paginas() as usize, paginas, "una pagina por pagina marcada");
        }
        PNG => {
            assert!(bytes.starts_with(b"\x89PNG"));
            let img = pixpin_codec::cargar(r).expect("png legible");
            assert!(img.ancho > 10 && img.alto > 10);
            assert!(salida.imagen.is_some(), "copiar la pone como imagen");
        }
        JPG => {
            assert!(bytes.starts_with(&[0xFF, 0xD8, 0xFF]));
            pixpin_codec::cargar(r).expect("jpg legible");
        }
        SVG => {
            let s = String::from_utf8(bytes).expect("utf8");
            assert!(s.starts_with("<?xml") && s.trim_end().ends_with("</svg>"));
        }
        "excalidraw" => {
            let s = String::from_utf8(bytes).expect("utf8");
            pixpin_motor2d::excalidraw::leer(&s).expect("se vuelve a leer");
        }
        "pixpin" => assert!(bytes.starts_with(b"PK"), "un .pixpin es un zip"),
        "texto" | "csv" => assert!(String::from_utf8(bytes).is_ok()),
        // Un Word que el lector de Word de PixPin vuelve a abrir.
        "word" => {
            assert!(r.extension().is_some_and(|e| e == "docx"), "{}", r.display());
            pixpin_docs::abrir(r).expect("el .docx se vuelve a leer");
        }
        _ => {}
    }
}

/// Hace cada formato de lo preparado y lo comprueba. Devuelve los ids.
fn todos_los_formatos(p: &Preparado, dir: &Path) -> Vec<String> {
    let c = compartible(p, &textos());
    assert!(!c.formatos.is_empty());
    let mut hechos = Vec::new();
    for (i, f) in c.formatos.iter().enumerate() {
        let claves = elegidas_para(&c, i);
        let salida = generar(p, &f.id, &claves, &dir.join(&f.id))
            .unwrap_or_else(|e| panic!("{} no se pudo hacer: {e:#}", f.id));
        let paginas = match f.cuantas {
            Cuantas::Varias => claves.len(),
            _ => 1,
        };
        comprobar(&salida, &f.id, paginas);
        hechos.push(f.id.clone());
    }
    hechos
}

#[test]
fn el_lienzo_del_editor_sale_en_todos_los_formatos_y_con_sus_marcos() {
    let (dir, borrar) = carpeta("lienzo");
    let p = preparar(Cosa::Lienzo(lienzo_suelto()), &textos()).unwrap();
    // El lienzo entero y un marco por hoja, sangrados; al abrir, los marcos.
    let nombres: Vec<&str> = p.piezas.iter().map(|x| x.pagina.nombre.as_str()).collect();
    assert_eq!(nombres, ["Lienzo completo", "Planta", "Alzado"]);
    assert_eq!(p.piezas[1].pagina.nivel, 1);
    let marcadas = p.marcadas.clone().unwrap();
    assert!(!marcadas.contains("lienzo") && marcadas.len() == 2, "{marcadas:?}");
    let hechos = todos_los_formatos(&p, &dir);
    assert_eq!(hechos, [WEB, PDF, PNG, JPG, SVG, "excalidraw"]);
    // Caso negativo: el PDF de un lienzo no tiene interruptor «Con anotaciones».
    assert!(compartible(&p, &textos()).formatos.iter().all(|f| f.interruptor.is_none()));
    // La pagina web lleva el lienzo dentro para volver a editarlo.
    let web = std::fs::read_to_string(dir.join(WEB).join("Casa.html")).unwrap();
    assert!(web.contains("class=\"excalidraw\""));
    if borrar {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn con_varios_lienzos_cada_papel_y_cada_foto_tiene_su_propio_numero() {
    // Dos lienzos con su papel: los dos lo llaman `ID_PAPEL`, y juntos en un
    // PDF el segundo pintaria el papel del primero.
    let mut p = Preparado::nuevo("dos");
    let t = textos();
    let escena = Escena::nueva();
    let a = Arc::new(foto(10, 10));
    let b = Arc::new(foto(20, 20));
    anadir_lienzo(&mut p, "a", "", &escena, Some((FuenteImagen::Cargada(a), 10.0, 10.0)), &|_| None, &t);
    anadir_lienzo(&mut p, "b", "", &escena, Some((FuenteImagen::Cargada(b), 20.0, 20.0)), &|_| None, &t);
    let ids: Vec<u64> = p
        .piezas
        .iter()
        .flat_map(|x| x.hoja.ordenes.iter())
        .filter_map(|o| match o {
            Orden::Imagen { id_objeto, .. } => Some(*id_objeto),
            _ => None,
        })
        .collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    assert!(!ids.contains(&ID_PAPEL) && !ids.contains(&0), "ni el del papel ni el cero");
    // Caso negativo: un lienzo vacio y sin papel no pone ninguna pagina.
    assert!(!anadir_lienzo(&mut p, "c", "", &escena, None, &|_| None, &t));
    assert_eq!(p.piezas.len(), 2);
}

/// Un proyecto de verdad en disco: un lienzo, una foto con lo dibujado
/// encima, una nota, una tabla y una lista de tareas.
fn proyecto_en_disco(raiz: &Path) -> (Ficha, Vec<Mensaje>) {
    let ficha = Ficha::nueva("Obra", 0, "PC");
    Indice {
        proyectos: vec![ficha.clone()],
        ..Indice::default()
    }
    .guardar(raiz)
    .unwrap();
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &ficha.id);
    std::fs::create_dir_all(carpeta.join("lienzos")).unwrap();
    // El lienzo.
    let mut escena = Escena::nueva();
    escena.anadir(rectangulo(0.0, 0.0, 200.0, 120.0));
    escena.anadir(trazo(20.0, 60.0));
    std::fs::write(
        pixpin_proyecto::almacen::lienzo(raiz, &ficha.id, "d1"),
        excalidraw_de(&escena),
    )
    .unwrap();
    // La foto, con un trazo encima en su `.pixpin2d`.
    let ruta_foto = carpeta.join("fachada.png");
    std::fs::write(&ruta_foto, pixpin_codec::codificar_png(&foto(160, 120)).unwrap()).unwrap();
    let mut encima = Escena::nueva();
    encima.anadir(trazo(10.0, 50.0));
    pixpin_motor2d::guardar(&dibujo_de_foto(&ruta_foto), &encima).unwrap();
    // La tabla.
    let mut tabla = pixpin_proyecto::tabla::Tabla {
        nombre: "Gastos".into(),
        ..Default::default()
    };
    tabla.celdas.insert("A1".into(), "Cemento".into());
    tabla.celdas.insert("B1".into(), "12".into());
    tabla.celdas.insert("A2".into(), "Arena; fina".into());
    tabla.celdas.insert("B2".into(), "=B1*2".into());
    let mensajes = vec![
        Mensaje {
            id: "m-lienzo".into(),
            cuando: 1,
            clase: Some(Clase::Dibujo),
            referencia: Some("d1".into()),
            nombre: "Planta baja".into(),
            ..Mensaje::default()
        },
        Mensaje {
            id: "m-foto".into(),
            cuando: 2,
            clase: Some(Clase::Imagen),
            ruta: Some("fachada.png".into()),
            nombre: "fachada.png".into(),
            ..Mensaje::default()
        },
        Mensaje {
            id: "m-nota".into(),
            cuando: 3,
            clase: Some(Clase::Nota),
            texto: "Llamar al fontanero el lunes.\nLlevar el metro.".into(),
            ..Mensaje::default()
        },
        Mensaje {
            id: "m-tabla".into(),
            cuando: 4,
            clase: Some(Clase::MiniApp),
            miniapp: Some(pixpin_proyecto::tabla::MINIAPP.into()),
            texto: tabla.escribir().unwrap(),
            ..Mensaje::default()
        },
        Mensaje {
            id: "m-tareas".into(),
            cuando: 5,
            clase: Some(Clase::MiniApp),
            miniapp: Some("tareas".into()),
            texto: "# Compras\n\n- [x] Cemento\n- [ ] Arena".into(),
            ..Mensaje::default()
        },
    ];
    let lineas: Vec<String> = mensajes.iter().map(|m| serde_json::to_string(m).unwrap()).collect();
    std::fs::write(carpeta.join("guardados.jsonl"), lineas.join("\n") + "\n").unwrap();
    (ficha, mensajes)
}

#[test]
fn un_proyecto_entero_sale_en_todos_los_formatos_y_como_pixpin() {
    let (dir, borrar) = carpeta("proyecto");
    let raiz = dir.join("datos");
    let (ficha, _) = proyecto_en_disco(&raiz);
    let p = preparar(
        Cosa::Proyectos {
            raiz: raiz.clone(),
            ids: vec![ficha.id.clone()],
        },
        &textos(),
    )
    .unwrap();
    let claves: Vec<&str> = p.piezas.iter().map(|x| x.pagina.clave.as_str()).collect();
    assert_eq!(claves, ["m-lienzo", "m-foto", "m-nota", "m-tabla", "m-tareas"]);
    let hechos = todos_los_formatos(&p, &dir.join("salida"));
    assert_eq!(hechos, [WEB, PDF, PNG, JPG, SVG, "pixpin"]);
    // La foto sale con lo dibujado encima: el trazo rojo.
    let c = compartible(&p, &textos());
    let png = generar(&p, PNG, &["m-foto".to_string()], &dir.join("foto")).unwrap();
    let img = png.imagen.unwrap();
    assert_eq!((img.ancho, img.alto), (320, 240), "a doble tamano");
    assert!(
        img.pixeles.chunks_exact(4).any(|q| q[0] > 180 && q[1] < 80 && q[2] < 80),
        "el trazo rojo de encima"
    );
    assert_eq!(c.titulo, "Obra");
    if borrar {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn una_tabla_compartida_como_pagina_web_sigue_calculando() {
    // J3: la «Pagina web» de una tabla no es el dibujo de su rejilla sino la
    // tabla del movil, con su motor de formulas y lo escrito dentro.
    let (dir, borrar) = carpeta("tabla-web");
    let raiz = dir.join("datos");
    let (ficha, mensajes) = proyecto_en_disco(&raiz);
    let elegidos: Vec<Mensaje> = mensajes
        .iter()
        .filter(|m| m.id == "m-tabla" || m.id == "m-nota")
        .cloned()
        .collect();
    let p = preparar(
        Cosa::Mensajes {
            raiz: raiz.clone(),
            proyecto: ficha.id.clone(),
            titulo: "Obra".into(),
            mensajes: elegidos,
        },
        &textos(),
    )
    .unwrap();
    let salida = generar(&p, WEB, &["m-nota".to_string(), "m-tabla".to_string()], &dir.join("web")).unwrap();
    let html = std::fs::read_to_string(&salida.ficheros[0]).unwrap();
    assert_eq!(html.matches("data-tipo=\"tabla\"").count(), 1);
    assert_eq!(html.matches("data-tipo=\"dibujo\"").count(), 1, "la nota sigue siendo un dibujo");
    assert!(html.contains("function crearTabla(d,api){"));
    assert!(html.contains("\"B2\":\"=B1*2\""), "lo escrito viaja, con la formula");
    assert!(html.contains("<td class=\"d f\">24</td>"), "y ya calculada para leerse sin guion");
    // Caso negativo: sin tablas en lo elegido no se carga su motor.
    let solo_nota = generar(&p, WEB, &["m-nota".to_string()], &dir.join("nota")).unwrap();
    let html = std::fs::read_to_string(&solo_nota.ficheros[0]).unwrap();
    assert!(!html.contains("function crearTabla(d,api){"));
    if borrar {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn cada_mensaje_solo_ofrece_sus_formatos_y_su_original() {
    let (dir, borrar) = carpeta("mensajes");
    let raiz = dir.join("datos");
    let (ficha, mensajes) = proyecto_en_disco(&raiz);
    let t = textos();
    let esperado: [(&str, &[&str]); 5] = [
        ("m-lienzo", &[WEB, PDF, PNG, JPG, SVG, "excalidraw"]),
        ("m-foto", &["original", WEB, PDF, PNG, JPG, SVG]),
        // Una nota Markdown sola tambien sale como Word (`word`).
        ("m-nota", &[WEB, PDF, PNG, JPG, SVG, "texto", "word"]),
        ("m-tabla", &[WEB, PDF, PNG, JPG, SVG, "csv"]),
        ("m-tareas", &[WEB, PDF, PNG, JPG, SVG, "texto"]),
    ];
    for (id, formatos) in esperado {
        let m = mensajes.iter().find(|m| m.id == id).unwrap().clone();
        let p = preparar(
            Cosa::Mensajes {
                raiz: raiz.clone(),
                proyecto: ficha.id.clone(),
                titulo: "Obra".into(),
                mensajes: vec![m],
            },
            &t,
        )
        .unwrap();
        let hechos = todos_los_formatos(&p, &dir.join(id));
        assert_eq!(hechos, formatos, "{id}");
    }
    // La tabla va con su formula ya hecha, y el punto y coma entre comillas.
    let csv = std::fs::read_dir(dir.join("m-tabla").join("csv"))
        .unwrap()
        .flatten()
        .next()
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .unwrap();
    assert_eq!(csv, "Cemento;12\r\n\"Arena; fina\";24\r\n");
    // Varios a la vez: todas las paginas juntas, y los originales.
    let p = preparar(
        Cosa::Mensajes {
            raiz: raiz.clone(),
            proyecto: ficha.id.clone(),
            titulo: "Obra".into(),
            mensajes: mensajes.clone(),
        },
        &t,
    )
    .unwrap();
    let c = compartible(&p, &t);
    let ids: Vec<&str> = c.formatos.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(ids, [WEB, PDF, PNG, JPG, SVG, "original"], "un solo fichero detras");
    let web = generar(&p, WEB, &elegidas_para(&c, 0), &dir.join("varios")).unwrap();
    comprobar(&web, WEB, 5);
    if borrar {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn lo_que_no_tiene_nada_que_compartir_no_ofrece_nada() {
    let (dir, borrar) = carpeta("nada");
    let t = textos();
    // Un acceso a un proyecto y una nota en blanco: nada.
    let vacios = vec![
        Mensaje {
            id: "p".into(),
            clase: Some(Clase::Proyecto),
            ..Mensaje::default()
        },
        Mensaje {
            id: "n".into(),
            clase: Some(Clase::Nota),
            texto: "   ".into(),
            ..Mensaje::default()
        },
        // Un adjunto que no llego a este equipo.
        Mensaje {
            id: "a".into(),
            clase: Some(Clase::Archivo),
            ruta: Some("no-esta.pdf".into()),
            ..Mensaje::default()
        },
    ];
    let p = preparar(
        Cosa::Mensajes {
            raiz: dir.clone(),
            proyecto: "x".into(),
            titulo: "x".into(),
            mensajes: vacios,
        },
        &t,
    )
    .unwrap();
    assert!(compartible(&p, &t).formatos.is_empty());
    // Pedir paginas que no hay, o un formato que no existe, es un error y
    // no un fichero vacio.
    assert!(generar(&p, WEB, &[], &dir.join("w")).is_err());
    assert!(generar(&p, PNG, &["no".into()], &dir.join("i")).is_err());
    assert!(generar(&p, "zip", &[], &dir.join("z")).is_err());
    // Un proyecto que no esta, tampoco.
    assert!(preparar(
        Cosa::Proyectos {
            raiz: dir.clone(),
            ids: vec!["no-existe".into()]
        },
        &t
    )
    .is_err());
    if borrar {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn un_pdf_anotado_sale_con_su_tinta_y_solo_las_paginas_elegidas() {
    let (dir, borrar) = carpeta("pdf");
    // Un PDF de tres paginas, hecho con el escritor de siempre.
    let texto = "Parrafo de prueba. ".repeat(900);
    let hojas = hojas_de_texto(Some("Memoria"), &texto);
    assert!(hojas.len() >= 3, "el texto largo pasa de pagina: {}", hojas.len());
    let bytes = pixpin_pdf::escribir::de_hojas(&hojas[..3], Some(BLANCO), &|_| None).unwrap();
    let pdf = dir.join("memoria.pdf");
    std::fs::write(&pdf, bytes).unwrap();
    // Lo anotado en la segunda hoja, donde lo guarda el lector.
    let mut capa = Escena::nueva();
    capa.anadir(trazo(200.0, 300.0));
    let ruta_capa = crate::lector_tinta::ruta_de_hoja(&pdf, 1);
    std::fs::create_dir_all(ruta_capa.parent().unwrap()).unwrap();
    std::fs::write(&ruta_capa, excalidraw_de(&capa)).unwrap();

    let t = textos();
    let p = preparar(Cosa::Documento(pdf.clone()), &t).unwrap();
    assert_eq!(p.piezas.len(), 3);
    assert_eq!(p.piezas[1].pagina.detalle, "Con lo anotado");
    assert!(p.piezas[0].pagina.detalle.is_empty());
    let c = compartible(&p, &t);
    assert_eq!(c.formatos[0].id, "original", "un PDF se suele mandar tal cual");
    let hechos = todos_los_formatos(&p, &dir.join("salida"));
    assert_eq!(hechos, ["original", WEB, PDF, PNG, JPG, SVG]);
    // Solo la segunda: un PDF de una pagina, con la tinta.
    let una = generar(&p, PDF, &[p.piezas[1].pagina.clave.clone()], &dir.join("una")).unwrap();
    comprobar(&una, PDF, 1);
    // **La pagina sigue siendo la del PDF** (su texto se lee) y la tinta va
    // encima dentro de su contenido y en su capa (Android v0.98.2), no como
    // foto de la hoja ni como anotacion que el visor del movil no pinta.
    let una = std::fs::read(&una.ficheros[0]).unwrap();
    let hoja = pixpin_pdf::plano::de_bytes(&una, 0).unwrap();
    assert!(hoja.textos.iter().any(|x| x.texto.contains("Parrafo de prueba")), "sin su texto");
    let texto = String::from_utf8_lossy(&una);
    assert!(texto.contains(" BDC\n/PxT") && !texto.contains("/Subtype /Stamp"), "sin la tinta en el contenido");
    // Todas: el original entero al principio y la revision detras.
    let original_bytes = std::fs::read(&pdf).unwrap();
    let todas: Vec<String> = p.piezas.iter().map(|x| x.pagina.clave.clone()).collect();
    let entero = generar(&p, PDF, &todas, &dir.join("todas")).unwrap();
    let entero = std::fs::read(&entero.ficheros[0]).unwrap();
    assert_eq!(&entero[..original_bytes.len()], &original_bytes[..], "el original no va entero");
    assert!(entero.len() - original_bytes.len() < 20_000, "pesa la tinta, no las hojas");
    // El interruptor quitado: el PDF limpio, tal cual.
    let pdf_formato = c.formatos.iter().find(|f| f.id == PDF).unwrap();
    assert_eq!(pdf_formato.interruptor.as_ref().map(|i| i.id_apagado.as_str()), Some(PDF_LIMPIO));
    let limpio = generar(&p, PDF_LIMPIO, &todas, &dir.join("limpio")).unwrap();
    assert_eq!(std::fs::read(&limpio.ficheros[0]).unwrap(), original_bytes);
    // **La pagina web, con «Texto buscable»** (Android v0.98.1): puesto, las
    // hojas en lineas; quitado, como imagen con el texto invisible encima.
    let web_formato = c.formatos.iter().find(|f| f.id == WEB).unwrap();
    assert_eq!(web_formato.interruptor.as_ref().map(|i| i.id_apagado.as_str()), Some(WEB_IMAGEN));
    let en_lineas = generar(&p, WEB, &todas, &dir.join("web")).unwrap();
    let en_lineas = std::fs::read_to_string(&en_lineas.ficheros[0]).unwrap();
    assert!(en_lineas.contains("class=\"plano-hoja\"") && !en_lineas.contains("class=\"texto-pdf\""));
    let como_imagen = generar(&p, WEB_IMAGEN, &todas, &dir.join("web-imagen")).unwrap();
    let como_imagen = std::fs::read_to_string(&como_imagen.ficheros[0]).unwrap();
    assert!(!como_imagen.contains("class=\"plano-hoja\"") && como_imagen.contains("class=\"texto-pdf\""));
    assert!(como_imagen.contains("Parrafo de prueba."), "el texto se sigue buscando");
    let png = generar(&p, PNG, &[p.piezas[1].pagina.clave.clone()], &dir.join("png2")).unwrap();
    let img = png.imagen.unwrap();
    assert!(
        img.pixeles.chunks_exact(4).any(|q| q[0] > 180 && q[1] < 80 && q[2] < 80),
        "la tinta roja de la segunda hoja"
    );
    // El original es el mismo fichero, sin tocar.
    let original = generar(&p, "original", &[], &dir.join("o")).unwrap();
    assert_eq!(original.ficheros, vec![pdf.clone()]);
    if borrar {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn un_documento_anotado_sale_como_en_el_lector_partido_en_paginas() {
    let (dir, borrar) = carpeta("documento");
    let doc = dir.join("apuntes.md");
    let mut md = String::from("# Apuntes de obra\n\n");
    for i in 0..60 {
        md.push_str(&format!("Parrafo {i}: la zapata corrida va a ochenta centimetros de fondo y se hormigona de una vez.\n\n"));
    }
    std::fs::write(&doc, md).unwrap();
    // Anotado: la columna fija de 500 y un trazo arriba y otro muy abajo.
    let ajustes = pixpin_docs::lectura::Ajustes {
        columna: 500,
        ..Default::default()
    };
    pixpin_docs::lectura::escribir(&doc, &ajustes).unwrap();
    let mut capa = Escena::nueva();
    capa.anadir(trazo(40.0, 120.0));
    capa.anadir(trazo(40.0, 2600.0));
    let ruta_capa = crate::lector_tinta::ruta_de_capa(&doc);
    std::fs::create_dir_all(ruta_capa.parent().unwrap()).unwrap();
    std::fs::write(&ruta_capa, excalidraw_de(&capa)).unwrap();

    let t = textos();
    let p = preparar(Cosa::Documento(doc.clone()), &t).unwrap();
    assert!(p.piezas.len() >= 2, "un documento largo va en varias paginas");
    // Todas las paginas con el papel oscuro del lector y la columna fijada
    // mas sus margenes para anotar.
    let margen = pixpin_docs::vista::margen_de(500.0);
    for x in &p.piezas {
        assert_eq!(x.fondo, a_rgba(crate::lector::FONDO));
        assert_eq!(x.hoja.caja.0, -margen);
        assert_eq!(x.hoja.caja.2, 500.0 + margen);
    }
    // Las paginas se tocan sin hueco ni solape.
    for par in p.piezas.windows(2) {
        assert_eq!(par[0].hoja.caja.3, par[1].hoja.caja.1);
    }
    // Cada trazo va en la pagina donde cae, y no en las demas.
    let con_tinta = |x: &Pieza| x.hoja.ordenes.iter().filter(|o| matches!(o, Orden::Tinta { .. } | Orden::Poligono { .. })).count();
    assert!(con_tinta(&p.piezas[0]) > 0, "el trazo de arriba en la primera");
    let donde = p
        .piezas
        .iter()
        .position(|x| x.hoja.caja.1 <= 2600.0 && x.hoja.caja.3 > 2600.0)
        .unwrap();
    assert!(donde > 0 && con_tinta(&p.piezas[donde]) > 0, "el de abajo en la suya");
    let hechos = todos_los_formatos(&p, &dir.join("salida"));
    assert_eq!(hechos, ["original", WEB, PDF, PNG, JPG, SVG]);
    if borrar {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn un_texto_largo_pasa_de_pagina_y_uno_corto_cabe_en_una() {
    let una = hojas_de_texto(Some("Nota"), "corta");
    assert_eq!(una.len(), 1);
    assert!(matches!(&una[0].ordenes[0], Orden::Texto { texto, .. } if texto == "Nota"));
    let largo = "linea\n".repeat(200);
    let varias = hojas_de_texto(None, &largo);
    assert!(varias.len() >= 3, "{}", varias.len());
    // Ninguna linea se sale del papel.
    for h in &varias {
        for o in &h.ordenes {
            if let Orden::Texto { y, tam, .. } = o {
                assert!(*y + tam * ex::INTERLINEA <= ALTO_A4 - MARGEN_A4 + 0.5);
            }
        }
    }
    // Caso negativo: un texto vacio da una pagina en blanco, no ninguna.
    assert_eq!(hojas_de_texto(None, "").len(), 1);
}

#[test]
fn las_tareas_se_leen_como_casillas_y_la_tabla_vacia_no_da_hoja() {
    assert_eq!(texto_de_miniapp("# Compras\n\n- [x] Cemento\n- [ ] Arena"), "☑ Cemento\n☐ Arena");
    assert!(hoja_de_tabla(&pixpin_proyecto::tabla::Tabla::default()).is_none());
}

#[test]
fn los_ficheros_viejos_se_borran_y_los_nuevos_no() {
    let (dir, _) = carpeta("limpiar");
    let vieja = dir.join("vieja");
    std::fs::create_dir_all(&vieja).unwrap();
    limpiar_viejos(&dir, std::time::Duration::from_secs(3600));
    assert!(vieja.exists(), "recien hecha: se queda");
    limpiar_viejos(&dir, std::time::Duration::ZERO);
    assert!(!vieja.exists(), "pasada su edad: fuera");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn lo_preparado_se_puede_llevar_al_hilo_de_los_ficheros() {
    fn es_send<T: Send + Sync>() {}
    es_send::<Preparado>();
    es_send::<Salida>();
}
