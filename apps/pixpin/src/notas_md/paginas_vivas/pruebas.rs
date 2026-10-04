//! Las paginas vivas con un proyecto de verdad en una carpeta temporal: un
//! lienzo, una tabla y una nota en su `proyecto.json` y su cuaderno.
//!
//! Muestras a mano (la nota con una pagina viva de un lienzo y una captura
//! pegada, oscura y clara):
//! `PIXPIN_MUESTRA=<carpeta> cargo test -p pixpin --bin pixpin muestra_pagina_viva -- --ignored`

use super::*;
use pixpin_motor2d::{ColorRgba, Elemento, Escena, Figura};

struct Carpeta(PathBuf);
impl Drop for Carpeta {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn carpeta(nombre: &str) -> Carpeta {
    let d = std::env::temp_dir().join(format!("pixpin-vivas-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    Carpeta(d)
}

fn textos() -> Catalogo {
    Catalogo::nuevo(pixpin_store::Idioma::Espanol)
}

fn rectangulo(x: f32, y: f32, w: f32, h: f32, c: ColorRgba) -> Elemento {
    Elemento {
        figura: Figura::Rectangulo,
        x,
        y,
        ancho: w,
        alto: h,
        trazo: c,
        grosor: 4.0,
        ..Elemento::default()
    }
}

fn excalidraw_de(escena: &Escena) -> String {
    pixpin_motor2d::excalidraw::escribir(&pixpin_motor2d::excalidraw::con_escena(
        &pixpin_motor2d::excalidraw::Lienzo::vacio(),
        escena,
    ))
}

/// La planta: una casa con dos cuartos.
fn planta(cuartos: usize) -> Escena {
    let mut e = Escena::nueva();
    e.anadir(rectangulo(
        0.0,
        0.0,
        600.0,
        360.0,
        ColorRgba::opaco(0.1, 0.1, 0.12),
    ));
    for i in 0..cuartos {
        e.anadir(rectangulo(
            20.0 + i as f32 * 190.0,
            20.0,
            170.0,
            150.0,
            ColorRgba::opaco(0.85, 0.2, 0.2),
        ));
    }
    e
}

const LIENZO: &str = "UIDLIENZO2";
const TABLA: &str = "UIDTABLA22";
const NOTA: &str = "UIDNOTA222";

/// Un proyecto «Casa Lima» con un lienzo, una tabla, una nota y un mensaje
/// del chat que no es hoja. Devuelve su ficha.
fn proyecto(raiz: &Path) -> Ficha {
    let ficha = Ficha::nueva("Casa Lima", 0, "PC");
    Indice {
        proyectos: vec![ficha.clone()],
        ..Indice::default()
    }
    .guardar(raiz)
    .unwrap();
    let c = almacen::carpeta(raiz, &ficha.id);
    std::fs::create_dir_all(c.join("lienzos")).unwrap();
    std::fs::write(
        almacen::lienzo(raiz, &ficha.id, "d1"),
        excalidraw_de(&planta(2)),
    )
    .unwrap();
    let mut tabla = pixpin_proyecto::tabla::Tabla {
        nombre: "Gastos".into(),
        ..Default::default()
    };
    tabla.celdas.insert("A1".into(), "Cemento".into());
    tabla.celdas.insert("B1".into(), "12".into());
    let mensajes = vec![
        Mensaje {
            id: "m-lienzo".into(),
            uid: Some(LIENZO.into()),
            cuando: 1,
            clase: Some(Clase::Dibujo),
            referencia: Some("d1".into()),
            nombre: "Planta baja".into(),
            ..Mensaje::default()
        },
        Mensaje {
            id: "m-tabla".into(),
            uid: Some(TABLA.into()),
            cuando: 2,
            clase: Some(Clase::MiniApp),
            miniapp: Some(pixpin_proyecto::tabla::MINIAPP.into()),
            texto: tabla.escribir().unwrap(),
            ..Mensaje::default()
        },
        Mensaje {
            id: "m-nota".into(),
            uid: Some(NOTA.into()),
            cuando: 3,
            clase: Some(Clase::Nota),
            texto: "# Pendientes\nLlamar al fontanero.".into(),
            ..Mensaje::default()
        },
        Mensaje {
            id: "m-chat".into(),
            uid: Some("UIDCHAT222".into()),
            cuando: 4,
            clase: Some(Clase::Nota),
            texto: "hola".into(),
            ..Mensaje::default()
        },
    ];
    escribir_cuaderno(&c, &mensajes);
    let hoja = |uid: &str, nombre: &str| pixpin_proyecto::Hoja {
        id: format!("h-{uid}"),
        nombre: nombre.into(),
        uid: Some(uid.into()),
        ..Default::default()
    };
    let p = pixpin_proyecto::Proyecto {
        id: ficha.id.clone(),
        nombre: "Casa Lima".into(),
        hojas: vec![hoja(LIENZO, ""), hoja(TABLA, ""), hoja(NOTA, "")],
        ..Default::default()
    };
    std::fs::write(c.join("proyecto.json"), serde_json::to_string(&p).unwrap()).unwrap();
    ficha
}

fn escribir_cuaderno(c: &Path, mensajes: &[Mensaje]) {
    let lineas: Vec<String> = mensajes
        .iter()
        .map(|m| serde_json::to_string(m).unwrap())
        .collect();
    std::fs::write(c.join("guardados.jsonl"), lineas.join("\n") + "\n").unwrap();
}

fn nota_de(f: &Ficha) -> Destino {
    Destino::Nueva {
        proyecto: f.id.clone(),
    }
}

#[test]
fn las_hojas_del_proyecto_salen_en_su_orden_y_los_mensajes_del_chat_no() {
    let d = carpeta("hojas");
    let f = proyecto(&d.0);
    let h = hojas_de(&d.0, &f);
    let nombres: Vec<&str> = h.iter().map(|h| h.nombre.as_str()).collect();
    assert_eq!(nombres, vec!["Planta baja", "Gastos", "Pendientes"]);
    assert_eq!(h[0].codigo, LIENZO);
    // El selector: el proyecto de la nota arriba; la nota que se edita no sale.
    let g = grupos(
        &d.0,
        &Destino::Mensaje {
            proyecto: f.id.clone(),
            codigo: NOTA.into(),
        },
    );
    assert_eq!(g.len(), 1);
    assert!(g[0].propio);
    assert_eq!(g[0].hojas.len(), 2);
    assert!(g[0].hojas.iter().all(|x| !x.clave.ends_with(NOTA)));
}

#[test]
fn meter_una_hoja_da_su_pagina_viva_portatil_o_su_enlace() {
    let d = carpeta("meter");
    let f = proyecto(&d.0);
    let nota = nota_de(&f);
    let clave = format!("{}{SEP}{LIENZO}", f.id);
    let r = insertar(&d.0, &nota, &clave, false).unwrap();
    let foto = md_imagen::leer(&r).unwrap();
    assert_eq!(foto.alt, "Planta baja");
    assert!(foto.ruta.starts_with("pixpin:files/"), "{r}");
    assert!(
        foto.ruta.ends_with(&format!("/notas/vivo-{LIENZO}.png")),
        "{r}"
    );
    assert_eq!(md_imagen::hoja_de_viva(&foto.ruta).as_deref(), Some(LIENZO));
    // Y se encuentra en el disco donde la pintara el vigia.
    let fichero = adjuntos::resolver(&d.0, &nota, &foto.ruta).unwrap();
    assert_eq!(
        fichero,
        almacen::carpeta(&d.0, &f.id)
            .join("notas")
            .join(format!("vivo-{LIENZO}.png"))
    );
    // El enlace, con el codigo del proyecto (el mismo en el movil).
    let e = insertar(&d.0, &nota, &clave, true).unwrap();
    assert_eq!(
        e,
        format!(
            "[Planta baja](pixpin:hoja={}/{LIENZO})",
            codigo_de_proyecto(&f)
        )
    );
    // Casos negativos: una hoja que no esta, o una clave sin separador.
    assert_eq!(
        insertar(&d.0, &nota, &format!("{}{SEP}NOESTA", f.id), false),
        None
    );
    assert_eq!(insertar(&d.0, &nota, "basura", false), None);
}

#[test]
fn el_vigia_pinta_la_pagina_viva_y_la_repinta_solo_cuando_cambia_la_hoja() {
    let d = carpeta("vigia");
    let f = proyecto(&d.0);
    let nota = nota_de(&f);
    let r = insertar(&d.0, &nota, &format!("{}{SEP}{LIENZO}", f.id), false).unwrap();
    let ruta = md_imagen::leer(&r).unwrap().ruta;
    let png = adjuntos::resolver(&d.0, &nota, &ruta).unwrap();
    let v = Vigia::nuevo(d.0.clone(), pixpin_store::Idioma::Espanol);
    // Al abrir: no estaba, se pinta.
    assert_eq!(
        v.mirar_y_esperar(&nota, std::slice::from_ref(&ruta)),
        vec![(ruta.clone(), Viva::Renovada)]
    );
    let primera = pixpin_codec::imagen::cargar(&png).unwrap();
    assert!(primera.ancho > 100 && primera.ancho <= LADO_VIVA);
    // Sin cambios, nada.
    assert!(
        v.mirar_y_esperar(&nota, std::slice::from_ref(&ruta))
            .is_empty()
    );
    // La hoja cambia (un cuarto mas): se repinta y la imagen es otra.
    std::thread::sleep(std::time::Duration::from_millis(30));
    std::fs::write(
        almacen::lienzo(&d.0, &f.id, "d1"),
        excalidraw_de(&planta(3)),
    )
    .unwrap();
    assert_eq!(
        v.mirar_y_esperar(&nota, std::slice::from_ref(&ruta)),
        vec![(ruta.clone(), Viva::Renovada)]
    );
    let segunda = pixpin_codec::imagen::cargar(&png).unwrap();
    assert_ne!(primera.pixeles, segunda.pixeles);
    // Caso negativo: un mensaje del chat que no es de la hoja no repinta una
    // tabla (el cuaderno cambia, su texto no).
    let rt =
        md_imagen::leer(&insertar(&d.0, &nota, &format!("{}{SEP}{TABLA}", f.id), false).unwrap())
            .unwrap()
            .ruta;
    assert_eq!(v.mirar_y_esperar(&nota, std::slice::from_ref(&rt)).len(), 1);
    std::thread::sleep(std::time::Duration::from_millis(30));
    let c = almacen::carpeta(&d.0, &f.id);
    let mut ms = Cuaderno::leer_de(&c).unwrap().mensajes;
    ms.push(Mensaje {
        id: "m-otro".into(),
        cuando: 9,
        clase: Some(Clase::Nota),
        texto: "otro".into(),
        ..Mensaje::default()
    });
    escribir_cuaderno(&c, &ms);
    assert!(
        v.mirar_y_esperar(&nota, std::slice::from_ref(&rt))
            .is_empty()
    );
}

#[test]
fn una_hoja_borrada_deja_la_ultima_copia_y_avisa_una_vez() {
    let d = carpeta("borrada");
    let f = proyecto(&d.0);
    let nota = nota_de(&f);
    let ruta =
        md_imagen::leer(&insertar(&d.0, &nota, &format!("{}{SEP}{LIENZO}", f.id), false).unwrap())
            .unwrap()
            .ruta;
    let v = Vigia::nuevo(d.0.clone(), pixpin_store::Idioma::Espanol);
    v.mirar_y_esperar(&nota, std::slice::from_ref(&ruta));
    let png = adjuntos::resolver(&d.0, &nota, &ruta).unwrap();
    assert!(png.is_file());
    // Se borra la hoja del cuaderno.
    let c = almacen::carpeta(&d.0, &f.id);
    let ms: Vec<Mensaje> = Cuaderno::leer_de(&c)
        .unwrap()
        .mensajes
        .into_iter()
        .filter(|m| m.id != "m-lienzo")
        .collect();
    // Y de su proyecto, como al borrarla en el chat.
    let mut p = leer_proyecto_json(&d.0, &f.id);
    p.hojas.retain(|h| h.uid.as_deref() != Some(LIENZO));
    std::fs::write(c.join("proyecto.json"), serde_json::to_string(&p).unwrap()).unwrap();
    escribir_cuaderno(&c, &ms);
    assert_eq!(
        v.mirar_y_esperar(&nota, std::slice::from_ref(&ruta)),
        vec![(ruta.clone(), Viva::SinHoja)]
    );
    assert!(
        v.mirar_y_esperar(&nota, std::slice::from_ref(&ruta))
            .is_empty(),
        "se avisa una vez"
    );
    assert!(png.is_file(), "la ultima copia se queda");
    // Al abrirla se abre esa copia.
    assert_eq!(que_abre(&d.0, &nota, &ruta), Apertura::Fichero(png));
}

#[test]
fn abrir_desde_la_nota_lleva_a_la_hoja_del_enlace_o_de_la_pagina_viva() {
    let d = carpeta("abrir");
    let f = proyecto(&d.0);
    let nota = nota_de(&f);
    let hoja = Apertura::Hoja {
        proyecto: f.id.clone(),
        codigo: LIENZO.into(),
    };
    let enlace = md_imagen::direccion_de_hoja(&codigo_de_proyecto(&f), LIENZO);
    assert_eq!(que_abre(&d.0, &nota, &enlace), hoja);
    // Con el id de aqui tambien vale.
    assert_eq!(
        que_abre(&d.0, &nota, &md_imagen::direccion_de_hoja(&f.id, LIENZO)),
        hoja
    );
    let viva =
        md_imagen::leer(&insertar(&d.0, &nota, &format!("{}{SEP}{LIENZO}", f.id), false).unwrap())
            .unwrap()
            .ruta;
    assert_eq!(que_abre(&d.0, &nota, &viva), hoja);
    // Casos negativos: una hoja que no esta y una foto que no esta.
    assert_eq!(
        que_abre(&d.0, &nota, &md_imagen::direccion_de_hoja("X", "NOESTA")),
        Apertura::Nada
    );
    assert_eq!(que_abre(&d.0, &nota, "notas/1-no-esta.png"), Apertura::Nada);
}

#[test]
fn lo_mandado_a_una_nota_lo_recoge_esa_nota_y_ninguna_otra() {
    let a = Destino::Nueva {
        proyecto: "pA".into(),
    };
    let b = Destino::Nueva {
        proyecto: "pB".into(),
    };
    let a_guardada = Destino::Mensaje {
        proyecto: "pA".into(),
        codigo: "N1".into(),
    };
    PENDIENTES.lock().unwrap().push((a.clone(), "uno".into()));
    PENDIENTES.lock().unwrap().push((b.clone(), "dos".into()));
    // La nota nueva ya guardada sigue recogiendo lo mandado a la nueva.
    assert_eq!(recoger(&a_guardada, &a), vec!["uno".to_string()]);
    assert!(recoger(&a_guardada, &a).is_empty());
    assert_eq!(recoger(&b, &b), vec!["dos".to_string()]);
}

#[test]
fn en_el_chat_se_ofrece_insertar_las_hojas_que_se_pintan_y_no_las_notas() {
    let lienzo = Mensaje {
        clase: Some(Clase::Dibujo),
        referencia: Some("d1".into()),
        ..Default::default()
    };
    // Un dibujo sin lienzo detras no tiene nada que pintar.
    assert!(!se_puede_insertar(&Mensaje {
        clase: Some(Clase::Dibujo),
        ..Default::default()
    }));
    let tabla = Mensaje {
        clase: Some(Clase::MiniApp),
        miniapp: Some(pixpin_proyecto::tabla::MINIAPP.into()),
        ..Default::default()
    };
    let tareas = Mensaje {
        clase: Some(Clase::MiniApp),
        miniapp: Some("tareas".into()),
        ..Default::default()
    };
    let nota = Mensaje {
        clase: Some(Clase::Nota),
        ..Default::default()
    };
    assert!(se_puede_insertar(&lienzo) && se_puede_insertar(&tabla));
    assert!(!se_puede_insertar(&tareas) && !se_puede_insertar(&nota));
}

/// La muestra: una nota con una captura pegada y la pagina viva del lienzo
/// ya pintada por el vigia, en oscuro y en claro.
#[test]
#[ignore]
fn muestra_pagina_viva() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let d = carpeta("muestra");
    let f = proyecto(&d.0);
    let nota = nota_de(&f);
    let viva = insertar(&d.0, &nota, &format!("{}{SEP}{LIENZO}", f.id), false).unwrap();
    let ruta = md_imagen::leer(&viva).unwrap().ruta;
    Vigia::nuevo(d.0.clone(), pixpin_store::Idioma::Espanol).mirar_y_esperar(&nota, &[ruta]);
    // La captura: un degradado con una banda, como una ventana capturada.
    let (an, al) = (900u32, 380u32);
    let mut px = Vec::with_capacity((an * al * 4) as usize);
    for y in 0..al {
        for x in 0..an {
            let barra = y < 40;
            let (r, g, b) = if barra {
                (40, 44, 52)
            } else {
                (230 - (x * 40 / an) as u8, 236, 245 - (y * 50 / al) as u8)
            };
            px.extend_from_slice(&[r, g, b, 255]);
        }
    }
    let captura = pixpin_notas::integracion::png_temporal(
        &ImagenRgba {
            ancho: an,
            alto: al,
            pixeles: px,
        },
        "Captura",
    )
    .unwrap();
    let pegada = adjuntos::adjuntar(&d.0, &nota, &captura, 1_790_000_000_000).unwrap();
    let texto = format!(
        "# Visita a Casa Lima\nLa captura del presupuesto, pegada con Ctrl+V:\n![Captura|420]({pegada})\nY la planta, que se actualiza sola al cambiar el lienzo:\n{viva}\nFalta medir el baño."
    );
    let salida = std::env::var("PIXPIN_MUESTRA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    for (claro, nombre) in [
        (false, "nota-md-pagina-viva-oscura.png"),
        (true, "nota-md-pagina-viva-clara.png"),
    ] {
        let (r, n) = (d.0.clone(), nota.clone());
        let img = pixpin_notas::editor::pintar_en_memoria(
            pixpin_notas::Pedido {
                texto: texto.clone(),
                rotulos: super::super::rotulos(&textos()),
                nombre_de_fichero: None,
                colocacion: None,
                resolver: Box::new(move |s: &str| adjuntos::resolver(&r, &n, s)),
                adjuntar: Box::new(|_: &Path| None),
                compartir: None,
                integracion: Default::default(),
                comentarios: Default::default(),
            },
            claro,
            (980, 1500),
        )
        .unwrap();
        std::fs::write(
            salida.join(nombre),
            pixpin_codec::imagen::codificar_png(&img).unwrap(),
        )
        .unwrap();
    }
}
