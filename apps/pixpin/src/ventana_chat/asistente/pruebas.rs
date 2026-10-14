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

fn png_de_prueba(lado: u32) -> Vec<u8> {
    let img = pixpin_codec::ImagenRgba {
        ancho: lado,
        alto: lado,
        pixeles: vec![120; (lado * lado * 4) as usize],
    };
    pixpin_codec::imagen::codificar_png(&img).unwrap()
}

#[test]
fn una_foto_al_chat_del_asistente_lanza_el_turno_y_en_otro_chat_no() {
    // Sin modelo bajado, al_enviar avisa: es la prueba de que SI quiso
    // contestar a la foto (antes ni se le llamaba).
    let (u, mut a, raiz) = chat("foto-al-asistente", true);
    let textos = Catalogo::nuevo(Idioma::Espanol);
    crate::asistente::motor::raiz_de_prueba(Some(raiz.join("modelos")));
    adjuntar(&u, &mut a, "K7Q2", "foto.png", &png_de_prueba(32), "").unwrap();
    assert_eq!(al_enviar(&u, &mut a, &textos), Some(textos.t("asistente-sin-modelo")));
    let (u2, mut b, raiz2) = chat("foto-en-otro", false);
    adjuntar(&u2, &mut b, "K7Q2", "foto.png", &png_de_prueba(32), "").unwrap();
    assert_eq!(al_enviar(&u2, &mut b, &textos), None, "sin mencion no es para el");
    crate::asistente::motor::raiz_de_prueba(None);
    let _ = std::fs::remove_dir_all(&raiz);
    let _ = std::fs::remove_dir_all(&raiz2);
}

#[test]
fn la_foto_se_contesta_y_luego_dibuja_eso_la_sigue_viendo() {
    let (u, mut a, raiz) = chat("foto-y-dibuja", true);
    adjuntar(&u, &mut a, "K7Q2", "plano.png", &png_de_prueba(64), "").unwrap();
    let motor = Arc::new(MotorGuionado::nuevo(vec![
        Respuesta::de_texto("Veo un cuadrado gris. ¿Qué quieres que haga?"),
        Respuesta::de_llamada(
            "dibujar_en_lienzo",
            r#"{"elementos":[{"tipo":"rectangulo","x":10,"y":10,"ancho":80,"alto":80,"color":"gris"}]}"#,
        ),
        Respuesta::de_texto("Listo."),
    ]));
    let pregunta = ia_chat::pregunta_de(a.mensajes.last().unwrap(), true).unwrap();
    lanzar(&u, &mut a, motor.clone(), pregunta).unwrap();
    hasta_el_final(&u, &mut a);
    assert!(respuesta(&a).texto.starts_with("Veo un cuadrado"), "{}", respuesta(&a).texto);
    escribir(&u, &mut a, "dibuja eso");
    lanzar(&u, &mut a, motor.clone(), "dibuja eso".into()).unwrap();
    hasta_el_final(&u, &mut a);
    let pedidas = motor.recibido.lock().unwrap();
    assert!(pedidas[0].mensajes.last().unwrap().tiene_imagen(), "la foto sola llega");
    assert!(pedidas[1].mensajes.last().unwrap().tiene_imagen(), "y sigue al pedir dibujarla");
    assert_eq!(pedidas[1].mensajes.last().unwrap().texto_entero(), "dibuja eso");
    let _ = std::fs::remove_dir_all(&raiz);
}

/// Un lienzo del usuario con nombre, ya en el chat.
fn lienzo_del_usuario(u: &Ubicacion, a: &mut Abierto, nombre: &str) -> cuaderno::Mensaje {
    let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
    let m = escribir_lienzo(u.raiz(), &a.ficha.id, "K7Q2", numero, Some(nombre)).unwrap();
    a.vistas.push(None);
    a.mensajes.push(m.clone());
    m
}

fn elementos_de(raiz: &std::path::Path, a: &Abierto, m: &cuaderno::Mensaje) -> usize {
    let ruta = fichero_del_mensaje(raiz, &a.ficha.id, m).unwrap();
    pixpin_motor2d::excalidraw::leer(&std::fs::read_to_string(ruta).unwrap())
        .unwrap()
        .elementos()
        .len()
}

const CIRCULO: &str = r#"{"elementos":[{"tipo":"elipse","x":10,"y":10,"ancho":80,"alto":80,"color":"rojo"}]}"#;

#[test]
fn sin_mencion_dibuja_en_su_lienzo_propio_y_la_respuesta_lo_abre() {
    let (u, mut a, raiz) = chat("propio", true);
    let mio = lienzo_del_usuario(&u, &mut a, "Plano casa");
    let motor = Arc::new(MotorGuionado::nuevo(vec![
        Respuesta::de_llamada("dibujar_en_lienzo", CIRCULO),
        Respuesta::de_texto("Hecho."),
    ]));
    escribir(&u, &mut a, "dibuja un circulo");
    lanzar(&u, &mut a, motor, "dibuja un circulo".into()).unwrap();
    hasta_el_final(&u, &mut a);
    // El del usuario no se toca: va al propio, nuevo y suyo.
    assert_eq!(elementos_de(&raiz, &a, &mio), 0);
    let j = ia_chat::lienzo_propio(&a.mensajes).expect("su lienzo");
    assert_eq!(a.mensajes[j].nombre, NOMBRE_LIENZO_PROPIO);
    assert_eq!(elementos_de(&raiz, &a, &a.mensajes[j].clone()), 1);
    // La respuesta lleva su «Abrir lienzo», en pantalla y en el disco.
    let id = respuesta(&a).id.clone();
    let i = a.mensajes.iter().rposition(|m| m.id == id).unwrap();
    assert_eq!(lienzo_de(&a, i), Some(j));
    let disco = cuaderno::Cuaderno::leer_de(&pixpin_proyecto::almacen::carpeta(&raiz, &a.ficha.id)).unwrap();
    let r = disco.mensajes.iter().find(|m| m.id == id).unwrap();
    assert_eq!(
        r.resto.get(ia_chat::LIENZO_DE_LA_RESPUESTA).and_then(|v| v.as_str()),
        a.mensajes[j].referencia.as_deref()
    );
    // Sin pedirlo, no se abre solo.
    assert_eq!(lienzo_por_abrir(&a), None);
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn con_mencion_dibuja_en_ese_lienzo_y_si_se_pide_lo_abre() {
    let (u, mut a, raiz) = chat("mencionado", true);
    let plano = lienzo_del_usuario(&u, &mut a, "Plano casa");
    let otro = lienzo_del_usuario(&u, &mut a, "Fachada");
    let motor = Arc::new(MotorGuionado::nuevo(vec![
        Respuesta::de_llamada("dibujar_en_lienzo", CIRCULO),
        Respuesta::de_texto("Hecho en el plano."),
    ]));
    let pregunta = "dibuja un circulo en @lienzo:Plano_casa y ábrelo";
    escribir(&u, &mut a, pregunta);
    lanzar(&u, &mut a, motor.clone(), pregunta.into()).unwrap();
    hasta_el_final(&u, &mut a);
    assert_eq!(elementos_de(&raiz, &a, &plano), 1);
    assert_eq!(elementos_de(&raiz, &a, &otro), 0);
    assert!(ia_chat::lienzo_propio(&a.mensajes).is_none(), "no crea uno propio");
    // Al modelo le llega el nombre, no la mencion en crudo.
    let pedidas = motor.recibido.lock().unwrap();
    assert!(pedidas[0].mensajes.last().unwrap().texto_entero().contains("el lienzo «Plano casa»"));
    // «…y ábrelo»: queda para abrir, una vez.
    let j = a.mensajes.iter().position(|m| m.id == plano.id).unwrap();
    assert_eq!(lienzo_por_abrir(&a), Some(j));
    assert_eq!(lienzo_por_abrir(&a), None);
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn con_un_lienzo_abierto_va_al_abierto_y_no_al_chat() {
    let (u, mut a, raiz) = chat("abierto", true);
    let editor = crate::asistente::al_lienzo::EditorAbierto::nuevo(7);
    let motor = Arc::new(MotorGuionado::nuevo(vec![
        Respuesta::de_llamada("dibujar_en_lienzo", CIRCULO),
        Respuesta::de_texto("Hecho."),
    ]));
    escribir(&u, &mut a, "dibuja");
    lanzar(&u, &mut a, motor, "dibuja".into()).unwrap();
    // El chat no atiende la cola mientras hay editor: lo hace el editor.
    let mut escena = pixpin_motor2d::Escena::nueva();
    let hasta = Instant::now() + Duration::from_secs(20);
    while ocupado(&a.ficha.id) && Instant::now() < hasta {
        crate::asistente::al_lienzo::atender_en_editor(&mut escena, (0.0, 0.0));
        latido(&u, Some(&mut a));
        std::thread::sleep(Duration::from_millis(10));
    }
    latido(&u, Some(&mut a));
    drop(editor);
    assert_eq!(escena.cuantos_visibles(), 1);
    assert!(ia_chat::lienzo_propio(&a.mensajes).is_none());
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn la_lista_de_arroba_en_la_caja() {
    let (u, mut a, raiz) = chat("arroba", true);
    lienzo_del_usuario(&u, &mut a, "Plano casa");
    lienzo_del_usuario(&u, &mut a, "Fachada");
    a.borrador = "dibuja en @pla".into();
    a.cursor.poner(&a.borrador, a.borrador.len(), false);
    assert!(menciones_abiertas(&a));
    assert!(tecla_de_menciones(&mut a, VK_ENTRAR), "Entrar elige, no envia");
    assert_eq!(a.borrador, "dibuja en @lienzo:Plano_casa ");
    assert!(!menciones_abiertas(&a), "tras el espacio se cierra");
    // Las flechas mueven la marca; Escape la cierra para ese `@`.
    a.borrador = "@".into();
    a.cursor.poner(&a.borrador, 1, false);
    assert!(tecla_de_menciones(&mut a, VK_ABAJO));
    assert!(tecla_de_menciones(&mut a, VK_TAB));
    assert_eq!(a.borrador, "@lienzo:Plano_casa ", "la segunda: la mas vieja");
    a.borrador = "@".into();
    a.cursor.poner(&a.borrador, 1, false);
    assert!(tecla_de_menciones(&mut a, VK_ESCAPE));
    assert!(!menciones_abiertas(&a));
    // Una letra cualquiera no es suya.
    assert!(!tecla_de_la_lista(0x41));
    // Sin `@`, nada.
    a.borrador = "hola".into();
    a.cursor.poner(&a.borrador, 4, false);
    assert!(!menciones_abiertas(&a));
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn cada_uno_a_su_lado_solo_con_el_asistente() {
    let (u, mut a, raiz) = chat("lados", true);
    escribir(&u, &mut a, "hola");
    let motor = Arc::new(MotorGuionado::nuevo(vec![Respuesta::de_texto("Hola.")]));
    lanzar(&u, &mut a, motor, "hola".into()).unwrap();
    hasta_el_final(&u, &mut a);
    assert!(conversacion_con_ia(&a));
    let usuario = a.mensajes.iter().find(|m| m.texto == "hola").unwrap();
    assert!(a_la_derecha(true, usuario));
    assert!(!a_la_derecha(true, respuesta(&a)));
    // Un chat sin el asistente sigue todo a la izquierda, como en el movil.
    let (u2, mut b, raiz2) = chat("lados-otro", false);
    escribir(&u2, &mut b, "nota");
    assert!(!conversacion_con_ia(&b));
    assert!(!a_la_derecha(conversacion_con_ia(&b), b.mensajes.last().unwrap()));
    let _ = std::fs::remove_dir_all(&raiz);
    let _ = std::fs::remove_dir_all(&raiz2);
}

/// **Muestra en PNG del chat «Asistente»**: lo del usuario a la derecha y
/// lo del asistente a la izquierda, la foto que se le manda, la respuesta
/// con «Abrir lienzo», su lienzo propio y la lista de `@` abierta en la caja.
/// Pintado con las mismas funciones que la ventana y la GPU de verdad.
///
/// ```text
/// PIXPIN_MUESTRAS=<carpeta> cargo test -p pixpin -- --ignored --nocapture muestra_del_chat_con_el_asistente
/// ```
#[test]
#[ignore = "necesita GPU; genera un PNG para mirarlo"]
fn muestra_del_chat_con_el_asistente() {
    use pixpin_render::MotorRender;
    use pixpin_render::fuera_de_pantalla::FueraDePantalla;
    let (u, mut a, raiz) = chat("muestra", true);
    lienzo_del_usuario(&u, &mut a, "Plano casa");
    // Una foto: un plano de juguete.
    let (w, h) = (360u32, 240u32);
    let mut px = vec![255u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let muro = (30..36).contains(&y) || (20..26).contains(&x) || (200..206).contains(&y);
            let c = if muro { [40, 40, 40] } else { [230, 235, 245] };
            px[i..i + 3].copy_from_slice(&c);
        }
    }
    let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::ImagenRgba { ancho: w, alto: h, pixeles: px }).unwrap();
    adjuntar(&u, &mut a, "K7Q2", "plano.png", &png, "").unwrap();
    let motor = Arc::new(MotorGuionado::nuevo(vec![
        Respuesta::de_texto("Veo el plano de una habitación: dos muros horizontales y uno vertical. ¿Quieres que lo dibuje en un lienzo?"),
        Respuesta::de_llamada(
            "dibujar_en_lienzo",
            r#"{"elementos":[{"tipo":"rectangulo","x":20,"y":30,"ancho":340,"alto":170,"color":"negro"},
                             {"tipo":"linea","x":20,"y":30,"x2":20,"y2":200,"color":"negro"},
                             {"tipo":"texto","x":120,"y":100,"texto":"Habitación","color":"azul"}]}"#,
        ),
        Respuesta::de_texto("Listo: lo dibujé en mi lienzo."),
    ]));
    let pregunta = ia_chat::pregunta_de(a.mensajes.last().unwrap(), true).unwrap();
    lanzar(&u, &mut a, motor.clone(), pregunta).unwrap();
    hasta_el_final(&u, &mut a);
    escribir(&u, &mut a, "Sí, dibújalo");
    lanzar(&u, &mut a, motor, "Sí, dibújalo".into()).unwrap();
    hasta_el_final(&u, &mut a);
    // Y en la caja, a medio escribir una mencion.
    a.borrador = "ahora una puerta en @".into();
    a.cursor.poner(&a.borrador, a.borrador.len(), false);

    let textos = Catalogo::nuevo(Idioma::Espanol);
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let d3 = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor_r = MotorRender::nuevo(d3.d3d()).expect("motor");
    let (ancho, alto) = (1100u32, 980u32);
    let destino = FueraDePantalla::nuevo(&motor_r, d3.d3d(), ancho, alto).expect("destino");
    let escala = 100;
    let marco = Rect { x: 0, y: 0, ancho, alto };
    let d = disponer(marco, escala, pixpin_ui::chat::ancho_inicial(ancho, escala), Some(&a));
    let miniaturas = crate::miniaturas::Miniaturas::nuevo();
    let mut previas = crate::miniaturas::Miniaturas::con_lado(PREVIA_LADO);
    let fichas = vec![a.ficha.clone()];
    let orden = vec![0usize];
    let marcados = Default::default();
    for _ in 0..4 {
        let rutas = fotos_del_historial(&a, &d, escala);
        if !rutas.is_empty() {
            previas.asegurar(&rutas, &motor_r);
        }
        let lista = Lista {
            fichas: &fichas,
            orden: &orden,
            scroll: 0,
            sobre: None,
            elegida: Some(0),
            ahora,
            textos: &textos,
            busqueda: &crate::caja_texto::CajaLinea::nueva(),
            buscando: false,
            marcados: &marcados,
        };
        motor_r
            .dibujar(&destino.destino, |p| {
                pintar(p, &d, &CLARO, papel_de(0, true), escala, &textos, None, &lista);
                a.zonas.borrow_mut().clear();
                let c = Pinta {
                    tema: &CLARO,
                    escala,
                    textos: &textos,
                    ahora,
                    miniaturas: &miniaturas,
                    previas: &previas,
                };
                let alto_texto = (pixpin_ui::chat::REDACCION_TAM * 1.3).ceil() as u32;
                a.alto_caja.set(alto_texto);
                pintar_historial(p, &d, &c, &a);
                pintar_redaccion(p, &d, &c, &a, alto_texto);
                pintar_menciones(p, &d, &c, &a);
            })
            .expect("fotograma");
        destino.esperar_gpu().expect("GPU");
    }
    // Lo del usuario a la derecha y lo del asistente a la izquierda.
    let colocado = a.colocado.borrow();
    let centro = |i: usize| {
        let k = colocado.visibles.iter().position(|v| *v == i).unwrap();
        let b = colocado.puestos[k].burbuja;
        b.x + b.ancho as i32 / 2
    };
    let area = area_del_historial(&d, &a, escala);
    let mitad = area.x + area.ancho as i32 / 2;
    for (i, m) in a.mensajes.iter().enumerate() {
        if matches!(m.clase, None | Some(cuaderno::Clase::Nota)) {
            let derecha = centro(i) > mitad;
            assert_eq!(derecha, !ia_chat::es_del_asistente(m), "{:?}: {}", m.aparato, m.texto);
        }
    }
    drop(colocado);
    let (w, hh, pixeles) = destino.leer_rgba().expect("leer");
    let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::ImagenRgba { ancho: w, alto: hh, pixeles }).unwrap();
    let salida = std::env::var_os("PIXPIN_MUESTRAS").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&salida).unwrap();
    let ruta = salida.join("chat-asistente-burbujas.png");
    std::fs::write(&ruta, png).unwrap();
    println!("chat-asistente: {}", ruta.display());
    let _ = std::fs::remove_dir_all(&raiz);
}
