//! El chat con el asistente de mentira, pensando (13-oct): lo pensado se
//! guarda con la respuesta y se vuelve a ver, la hoja de trabajo pasa al
//! turno siguiente, una hoja rota no rompe nada, y una conversacion larga se
//! reinicia y recupera lo viejo con `buscar_historial`.

use super::*;
use pixpin_ia::motor::{MotorGuionado, Respuesta};

fn chat(nombre: &str) -> (Ubicacion, Abierto, std::path::PathBuf) {
    olvidar_para_pruebas();
    crate::asistente::al_lienzo::vaciar_para_pruebas();
    let raiz = std::env::temp_dir().join(format!(
        "pixpin-chat-pensado-{nombre}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(&raiz).unwrap();
    let u = Ubicacion::Portable { raiz: raiz.clone() };
    let ficha = pixpin_proyecto::almacen::asegurar_asistente(&raiz, 0, "PC").unwrap();
    std::fs::create_dir_all(pixpin_proyecto::almacen::carpeta(&raiz, &ficha.id)).unwrap();
    let a = abrir_proyecto(&u, &ficha);
    (u, a, raiz)
}

fn escribir(u: &Ubicacion, a: &mut Abierto, texto: &str) {
    a.borrador = texto.into();
    guardar_nota(u, a, "K7Q2").unwrap();
}

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

fn ultima_respuesta(a: &Abierto) -> cuaderno::Mensaje {
    a.mensajes
        .iter()
        .rev()
        .find(|m| ia_chat::es_del_asistente(m))
        .cloned()
        .expect("respuesta")
}

fn pensando(p: &str, t: &str) -> Respuesta {
    Respuesta {
        pensamiento: p.into(),
        ..Respuesta::de_texto(t)
    }
}

const PLAN_FIESTA: &str = "ENTIENDO: Apuntar la fiesta de Lucia: sabado 18 a las 17:00 en el parque Kennedy.\nHARE: confirmarlo.\nHERRAMIENTAS: ninguna\nDATOS: sabado 18; 17:00; parque Kennedy";

#[test]
fn lo_pensado_se_guarda_con_la_respuesta_y_la_hoja_pasa_al_turno_siguiente() {
    let (u, mut a, raiz) = chat("guardar");
    let motor = Arc::new(MotorGuionado::nuevo(vec![
        pensando(
            "Hay que recordar la fecha, la hora y el sitio.",
            PLAN_FIESTA,
        ),
        Respuesta::de_texto("Apuntado."),
        Respuesta::de_texto("ENTIENDO: Decir la hora de la fiesta de Lucia.\nHARE: mirar la hoja."),
        Respuesta::de_texto("A las 17:00."),
    ]));
    escribir(
        &u,
        &mut a,
        "Apunta: fiesta de Lucia el sabado 18 a las 17:00 en el parque Kennedy",
    );
    lanzar_pensando(
        &u,
        &mut a,
        motor.clone(),
        "Apunta la fiesta".into(),
        true,
        8192,
    )
    .unwrap();
    hasta_el_final(&u, &mut a);
    let r = ultima_respuesta(&a);
    assert_eq!(r.texto, "Apuntado.");
    // En el disco, junto a la nota, con su codigo unico.
    let ruta = ia_pensado::ruta_del_registro(&raiz, &a.ficha.id, &r.codigo_unico());
    assert!(
        ruta.to_string_lossy().contains(".asistente.json"),
        "{}",
        ruta.display()
    );
    let reg = pixpin_ia::hoja_trabajo::Registro::leer(&ruta)
        .unwrap()
        .expect("registro guardado");
    assert_eq!(
        reg.pensamiento,
        "Hay que recordar la fecha, la hora y el sitio."
    );
    assert!(reg.hoja.objetivo.contains("parque Kennedy"));
    assert!(reg.hoja.hechos.iter().any(|h| h == "17:00"));
    // La seccion de la burbuja sale del registro (como al volver a abrir).
    ia_pensado::olvidar_para_pruebas();
    let s = ia_pensado::seccion_de(&raiz, &a.ficha.id, &r).expect("seccion");
    assert!(s.segundos.is_some() && !s.pensando);
    assert!(s.plan.unwrap().entiendo.contains("Kennedy"));
    // El sincronizador lo reconoce como lo anotado de esa nota.
    let nombre = ruta.file_name().unwrap().to_string_lossy().to_string();
    assert_eq!(
        pixpin_sincro::anotado::uid_del_nombre(&nombre),
        Some(r.codigo_unico().as_str())
    );

    // Turno siguiente: la vuelta de planear lee la hoja.
    escribir(&u, &mut a, "¿a que hora era?");
    lanzar_pensando(
        &u,
        &mut a,
        motor.clone(),
        "¿a que hora era?".into(),
        false,
        8192,
    )
    .unwrap();
    hasta_el_final(&u, &mut a);
    assert_eq!(ultima_respuesta(&a).texto, "A las 17:00.");
    let pedidas = motor.recibido.lock().unwrap();
    let planear = pedidas[2].mensajes[1].texto_entero();
    assert!(
        planear.contains("Objetivo: Apuntar la fiesta de Lucia"),
        "{planear}"
    );
    assert!(planear.contains("- 17:00"), "{planear}");
    assert!(!pedidas[2].pensar, "con pensar apagado no piensa");
    let actuar = pedidas[3].mensajes[0].texto_entero();
    assert!(
        actuar.contains("# Hoja de trabajo") && actuar.contains("Decir la hora"),
        "{actuar}"
    );
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn una_hoja_rota_no_rompe_el_chat_y_se_empieza_otra() {
    let (u, mut a, raiz) = chat("rota");
    let motor = Arc::new(MotorGuionado::nuevo(vec![
        Respuesta::de_texto(PLAN_FIESTA),
        Respuesta::de_texto("Apuntado."),
        Respuesta::de_texto("ENTIENDO: Saludar.\nHARE: contestar."),
        Respuesta::de_texto("Hola."),
    ]));
    escribir(&u, &mut a, "apunta la fiesta");
    lanzar_pensando(
        &u,
        &mut a,
        motor.clone(),
        "apunta la fiesta".into(),
        true,
        8192,
    )
    .unwrap();
    hasta_el_final(&u, &mut a);
    let r = ultima_respuesta(&a);
    let ruta = ia_pensado::ruta_del_registro(&raiz, &a.ficha.id, &r.codigo_unico());
    std::fs::write(&ruta, b"{\"version\":1,\"hoja\":{\"objetivo\":").unwrap();
    ia_pensado::olvidar_para_pruebas();
    // La burbuja vieja no ensena nada (ni falla).
    assert!(ia_pensado::seccion_de(&raiz, &a.ficha.id, &r).is_none());
    escribir(&u, &mut a, "hola");
    lanzar_pensando(&u, &mut a, motor.clone(), "hola".into(), true, 8192).unwrap();
    hasta_el_final(&u, &mut a);
    assert_eq!(ultima_respuesta(&a).texto, "Hola.");
    let pedidas = motor.recibido.lock().unwrap();
    assert!(
        !pedidas[2].mensajes[1]
            .texto_entero()
            .contains("Hoja de trabajo"),
        "hoja nueva"
    );
    let nueva = ultima_respuesta(&a);
    let reg = pixpin_ia::hoja_trabajo::Registro::leer(&ia_pensado::ruta_del_registro(
        &raiz,
        &a.ficha.id,
        &nueva.codigo_unico(),
    ))
    .unwrap()
    .unwrap();
    assert_eq!(reg.hoja.objetivo, "Saludar.");
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn una_conversacion_larga_se_reinicia_sin_perder_el_objetivo_y_busca_lo_viejo() {
    let (u, mut a, raiz) = chat("larga");
    // Una primera respuesta con su hoja (el objetivo), y mucho despues.
    let motor = Arc::new(MotorGuionado::nuevo(vec![
        Respuesta::de_texto(PLAN_FIESTA),
        Respuesta::de_texto("Apuntado."),
    ]));
    escribir(
        &u,
        &mut a,
        "Apunta: fiesta de Lucia el sabado 18 a las 17:00 en el parque Kennedy, 12 ninos",
    );
    lanzar_pensando(&u, &mut a, motor, "apunta".into(), false, 8192).unwrap();
    hasta_el_final(&u, &mut a);
    let relleno = "bla ".repeat(400);
    let raiz_s = raiz.clone();
    for i in 0..10 {
        let numero = a.mensajes.iter().map(|m| m.numero).max().unwrap_or(0) + 1;
        let id = a.ficha.id.clone();
        let quien = if i % 2 == 0 { "K7Q2" } else { ia_chat::APARATO };
        let n = escribir_nota(
            &raiz_s,
            &id,
            quien,
            &format!("charla {i} {relleno}"),
            1 + i,
            numero,
            None,
        )
        .unwrap();
        a.vistas.push(None);
        a.mensajes.push(n);
    }
    let motor = Arc::new(MotorGuionado::nuevo(vec![
        Respuesta::de_texto(
            "ENTIENDO: Decir cuantos ninos van a la fiesta de Lucia.\nHARE: buscarlo.",
        ),
        Respuesta::de_llamada(
            pixpin_ia::contexto::BUSCAR_HISTORIAL,
            r#"{"consulta":"ninos fiesta Lucia"}"#,
        ),
        Respuesta::de_texto("Van 12 ninos."),
    ]));
    escribir(&u, &mut a, "¿cuantos ninos venian?");
    lanzar_pensando(
        &u,
        &mut a,
        motor.clone(),
        "¿cuantos ninos venian?".into(),
        false,
        8192,
    )
    .unwrap();
    hasta_el_final(&u, &mut a);
    assert_eq!(ultima_respuesta(&a).texto, "Van 12 ninos.");
    let pedidas = motor.recibido.lock().unwrap();
    // La de actuar va reiniciada: sistema (con la hoja) + 2 + la pregunta.
    let actuar = &pedidas[1].mensajes;
    assert_eq!(actuar.len(), 4, "{}", actuar.len());
    let sistema = actuar[0].texto_entero();
    assert!(
        sistema.contains("Objetivo: Apuntar la fiesta de Lucia"),
        "el objetivo sigue: {sistema}"
    );
    assert!(sistema.contains("buscar_historial"));
    // buscar_historial encontro el primer mensaje, que ya no iba cargado.
    let buscado = pedidas[2].mensajes.last().unwrap().texto_entero();
    assert!(buscado.contains("12 ninos"), "{buscado}");
    // Y la hoja apunta desde donde se reinicio, para el turno siguiente.
    let r = ultima_respuesta(&a);
    let reg = pixpin_ia::hoja_trabajo::Registro::leer(&ia_pensado::ruta_del_registro(
        &raiz,
        &a.ficha.id,
        &r.codigo_unico(),
    ))
    .unwrap()
    .unwrap();
    assert!(reg.reinicio);
    assert_eq!(reg.hoja.reinicios, 1);
    assert!(reg.hoja.reinicio_desde.is_some());
    // La burbuja dice que se renovo el contexto.
    assert!(
        ia_pensado::seccion_de(&raiz, &a.ficha.id, &r)
            .unwrap()
            .reinicio
    );
    let _ = std::fs::remove_dir_all(&raiz);
}
