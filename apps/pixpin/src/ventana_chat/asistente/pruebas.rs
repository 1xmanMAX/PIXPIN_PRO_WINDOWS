//! El chat con el asistente de mentira (`MotorGuionado`): la burbuja que
//! crece, el dibujo que acaba en un lienzo del chat, la mencion, parar, sin
//! modelo, la descarga cancelada y el texto leido.

use super::*;
use pixpin_ia::motor::{MotorGuionado, Respuesta};
use pixpin_store::idioma::Idioma;

fn chat(nombre: &str, del_asistente: bool) -> (Ubicacion, Abierto, std::path::PathBuf) {
    olvidar_para_pruebas();
    crate::asistente::al_lienzo::vaciar_para_pruebas();
    let raiz = std::env::temp_dir().join(format!("pixpin-chat-asistente-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(&raiz).unwrap();
    let u = Ubicacion::Portable { raiz: raiz.clone() };
    let ficha = if del_asistente {
        pixpin_proyecto::almacen::asegurar_asistente(&raiz, 0, "PC").unwrap()
    } else {
        let f = pixpin_proyecto::almacen::Ficha::nueva("obra", 0, "PC");
        let mut i = pixpin_proyecto::almacen::Indice::leer(&raiz);
        i.proyectos.push(f.clone());
        i.guardar(&raiz).unwrap();
        f
    };
    std::fs::create_dir_all(pixpin_proyecto::almacen::carpeta(&raiz, &ficha.id)).unwrap();
    let a = abrir_proyecto(&u, &ficha);
    (u, a, raiz)
}

fn escribir(u: &Ubicacion, a: &mut Abierto, texto: &str) {
    a.borrador = texto.into();
    guardar_nota(u, a, "K7Q2").unwrap();
}

/// Da vueltas al latido como el bucle del chat hasta que no quede nada.
fn hasta_el_final(u: &Ubicacion, a: &mut Abierto) {
    let hasta = Instant::now() + Duration::from_secs(20);
    while Instant::now() < hasta {
        latido(u, Some(a));
        if espera().is_none() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("el asistente no acabo");
}

fn respuesta(a: &Abierto) -> &cuaderno::Mensaje {
    a.mensajes
        .iter()
        .rev()
        .find(|m| ia_chat::es_del_asistente(m) && matches!(m.clase, None | Some(cuaderno::Clase::Nota)))
        .expect("respuesta")
}

#[test]
fn el_chat_asistente_existe_una_vez_y_va_detras_de_guardados() {
    let raiz = std::env::temp_dir().join(format!("pixpin-chat-asistente-fijo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(&raiz).unwrap();
    pixpin_proyecto::almacen::asegurar_guardados(&raiz, 0, "PC").unwrap();
    let a1 = pixpin_proyecto::almacen::asegurar_asistente(&raiz, 0, "PC").unwrap();
    let a2 = pixpin_proyecto::almacen::asegurar_asistente(&raiz, 5, "PC").unwrap();
    assert_eq!(a1.id, a2.id);
    assert!(a1.es_asistente() && !a1.es_guardados());
    let mut i = pixpin_proyecto::almacen::Indice::leer(&raiz);
    i.proyectos.push(pixpin_proyecto::almacen::Ficha::nueva("nuevo", 99, "PC"));
    let orden: Vec<String> = i.ordenadas().iter().map(|f| f.nombre.clone()).collect();
    assert_eq!(orden, vec!["Mensajes guardados", "Asistente", "nuevo"]);
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn un_dibujo_pedido_en_el_chat_acaba_en_un_lienzo_nuevo_y_luego_en_el_mismo() {
    let (u, mut a, raiz) = chat("dibujo", true);
    let dibujo = r#"{"elementos":[{"tipo":"elipse","x":10,"y":10,"ancho":80,"alto":80,"color":"rojo","relleno":"rojo"}]}"#;
    let motor = Arc::new(MotorGuionado::nuevo(vec![
        Respuesta::de_llamada("dibujar_en_lienzo", dibujo),
        Respuesta::de_texto("Hecho: un circulo rojo."),
        Respuesta::de_llamada("dibujar_en_lienzo", dibujo),
        Respuesta::de_texto("Otro mas."),
    ]));
    escribir(&u, &mut a, "dibuja un circulo rojo");
    lanzar(&u, &mut a, motor.clone(), "dibuja un circulo rojo".into()).unwrap();
    // Nada mas lanzar, la burbuja ya esta y el boton es «parar».
    assert!(ocupado(&a.ficha.id));
    assert_eq!(respuesta(&a).aparato.as_deref(), Some("IA"));
    hasta_el_final(&u, &mut a);
    assert!(!ocupado(&a.ficha.id));
    assert!(
        respuesta(&a).texto.starts_with("Hecho: un circulo rojo."),
        "{}",
        respuesta(&a).texto
    );
    // Un lienzo nuevo en el chat, con el circulo dentro y en rojo.
    let lienzos: Vec<_> = a
        .mensajes
        .iter()
        .filter(|m| m.clase == Some(cuaderno::Clase::Dibujo))
        .cloned()
        .collect();
    assert_eq!(lienzos.len(), 1);
    let ruta = fichero_del_mensaje(&raiz, &a.ficha.id, &lienzos[0]).unwrap();
    let json = std::fs::read_to_string(&ruta).unwrap();
    assert_eq!(pixpin_motor2d::excalidraw::leer(&json).unwrap().elementos().len(), 1);
    assert!(json.contains("#e03131"), "{json}");
    // Lo escrito esta en el disco, no solo en pantalla.
    let disco = cuaderno::Cuaderno::leer_de(&pixpin_proyecto::almacen::carpeta(&raiz, &a.ficha.id)).unwrap();
    assert!(disco.mensajes.iter().any(|m| m.texto.starts_with("Hecho: un circulo rojo.")));

    // La segunda vez, al mismo lienzo (a la derecha), sin crear otro.
    escribir(&u, &mut a, "otro");
    lanzar(&u, &mut a, motor, "otro".into()).unwrap();
    hasta_el_final(&u, &mut a);
    let n = a
        .mensajes
        .iter()
        .filter(|m| m.clase == Some(cuaderno::Clase::Dibujo))
        .count();
    assert_eq!(n, 1);
    let json = std::fs::read_to_string(&ruta).unwrap();
    assert_eq!(pixpin_motor2d::excalidraw::leer(&json).unwrap().elementos().len(), 2);
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn en_otro_chat_solo_contesta_con_la_mencion_y_cuelga_de_la_pregunta() {
    let (u, mut a, raiz) = chat("mencion", false);
    let textos = Catalogo::nuevo(Idioma::Espanol);
    escribir(&u, &mut a, "hola a todos");
    // Sin mencion: ni aviso ni turno.
    assert_eq!(al_enviar(&u, &mut a, &textos), None);
    assert!(!ocupado(&a.ficha.id));
    escribir(&u, &mut a, "@asistente resume esto");
    let pregunta = a.mensajes.last().unwrap().id.clone();
    let motor = Arc::new(MotorGuionado::nuevo(vec![Respuesta::de_texto("Dicen hola.")]));
    lanzar(&u, &mut a, motor.clone(), "resume esto".into()).unwrap();
    hasta_el_final(&u, &mut a);
    assert_eq!(respuesta(&a).texto, "Dicen hola.");
    assert_eq!(respuesta(&a).responde_a.as_deref(), Some(pregunta.as_str()));
    // Y el modelo vio lo de antes del chat.
    let pedidas = motor.recibido.lock().unwrap();
    assert!(pedidas[0].mensajes.iter().any(|m| m.texto_entero() == "hola a todos"));
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn sin_el_modelo_bajado_avisa_y_no_escribe_nada() {
    let (u, mut a, raiz) = chat("sin-modelo", true);
    let textos = Catalogo::nuevo(Idioma::Espanol);
    crate::asistente::motor::raiz_de_prueba(Some(raiz.join("modelos")));
    escribir(&u, &mut a, "hola");
    let n = a.mensajes.len();
    let aviso = al_enviar(&u, &mut a, &textos);
    crate::asistente::motor::raiz_de_prueba(None);
    assert_eq!(aviso, Some(textos.t("asistente-sin-modelo")));
    assert_eq!(a.mensajes.len(), n, "no nace ninguna burbuja");
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn parar_a_mitad_deja_lo_escrito_y_lo_dice() {
    let (u, mut a, raiz) = chat("parar", true);
    let largo: String = (0..400).map(|i| format!("p{i} ")).collect();
    let motor = Arc::new(MotorGuionado::nuevo(vec![Respuesta::de_texto(largo)]));
    escribir(&u, &mut a, "cuenta");
    lanzar(&u, &mut a, motor, "cuenta".into()).unwrap();
    parar(&a.ficha.id);
    hasta_el_final(&u, &mut a);
    let t = &respuesta(&a).texto;
    assert!(t.ends_with("⏹ (parado)"), "{t}");
    assert!(!t.contains("p399"), "no paro");
    assert!(!ocupado(&a.ficha.id));
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn la_descarga_cancelada_vuelve_a_la_tarjeta_con_el_aviso() {
    let (u, a, raiz) = chat("descarga", true);
    lanzar_descarga_para_pruebas(Descarga::lanzar_con(pixpin_ia::modelos::QWEN35_08B, |_, c| {
        while !c.load(std::sync::atomic::Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(5));
        }
        Err(pixpin_ia::ErrorIa::Cancelado)
    }));
    assert!(espera().is_some());
    cumplir(&u, &a, Accion::Cancelar);
    let hasta = Instant::now() + Duration::from_secs(10);
    while espera().is_some() && Instant::now() < hasta {
        latido(&u, None);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(noticia_para_pruebas().as_deref(), Some("asistente-cancelada"));
    // Elegir el ligero se guarda en los ajustes.
    cumplir(&u, &a, Accion::Elegir { ligero: true });
    assert_eq!(pixpin_store::cargar(&u).unwrap().asistente.modelo, "qwen3.5-0.8b");
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn el_texto_leido_queda_en_el_chat_como_nota_y_como_txt() {
    let (_u, mut a, raiz) = chat("leido", true);
    let de = crate::asistente::turno::Adjunto {
        nombre: "Contrato.pdf".into(),
        ruta: raiz.join("Contrato.pdf"),
    };
    let id = a.ficha.id.clone();
    dejar_texto_leido(&raiz, Some(&mut a), &id, &de, "Clausula primera").unwrap();
    let n = a.mensajes.len();
    assert!(a.mensajes[n - 2].texto.contains("Clausula primera"));
    let txt = &a.mensajes[n - 1];
    assert_eq!(txt.nombre, "Contrato (texto).txt");
    let ruta = ruta_del_mensaje(&raiz, &id, txt).unwrap();
    assert_eq!(std::fs::read_to_string(ruta).unwrap(), "Clausula primera");
    let _ = std::fs::remove_dir_all(&raiz);
}
