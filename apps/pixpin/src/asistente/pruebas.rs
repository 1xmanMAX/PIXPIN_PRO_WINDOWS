//! Pruebas del asistente sin ventana y sin modelo: el turno en su hilo con
//! el motor de mentira (`MotorGuionado`), el lienzo, el OCR de Windows sobre
//! imagenes y PDF fabricados aqui, la descarga y la foto.
//!
//! Las del chat abierto (burbuja en vivo, parar, tarjeta) estan en
//! `ventana_chat/asistente.rs`; la de extremo a extremo con el modelo de
//! verdad, `#[ignore]`, en [`de_verdad`].

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use pixpin_ia::ErrorIa;
use pixpin_ia::agente::Evento;
use pixpin_ia::descarga::Progreso;
use pixpin_ia::motor::{Mensaje, MotorGuionado, Respuesta};

use super::al_lienzo;
use super::chat::{self, EnVivo};
use super::descarga::{AvisoDescarga, Descarga};
use super::turno::{Adjunto, Aviso, Turno};

fn carpeta(nombre: &str) -> PathBuf {
    let c = std::env::temp_dir().join(format!("pixpin-asistente-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&c);
    std::fs::create_dir_all(&c).unwrap();
    c
}

/// Espera al final del turno atendiendo la cola del lienzo como lo haria la
/// ventana (aqui, en una escena suelta).
fn hasta_el_final(
    turno: &mut Turno,
    escena: Option<&mut pixpin_motor2d::Escena>,
) -> (EnVivo, Vec<Aviso>) {
    let mut vivo = EnVivo::default();
    let mut todos = Vec::new();
    let mut escena = escena;
    let hasta = Instant::now() + Duration::from_secs(20);
    while Instant::now() < hasta {
        if let Some(e) = escena.as_deref_mut() {
            al_lienzo::atender_en_editor(e, (1000.0, 1000.0));
        }
        for a in turno.recibir() {
            match &a {
                Aviso::Evento(e) => vivo.aplicar(e),
                Aviso::Fin { resultado } => vivo.acabar(resultado),
                // Sin nadie que conteste, lo que borra o envia no se hace.
                Aviso::Confirmar { responder, .. } => {
                    let _ = responder.send(false);
                }
                // Ni a las dudas: sigue con la recomendada.
                Aviso::Preguntar { responder, .. } => {
                    let _ = responder.send(pixpin_ia::preguntar::Contestada::SinRespuesta);
                }
                Aviso::TextoLeido { .. } | Aviso::Registro(_) | Aviso::Citas(_) => {}
            }
            todos.push(a);
        }
        if turno.acabado() {
            return (vivo, todos);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("el turno no acabo");
}

fn historial(pregunta: &str) -> Vec<Mensaje> {
    super::turno::historial_para_el_modelo(&[], pregunta, None)
}

#[test]
fn un_dibujo_llega_al_lienzo_abierto_centrado_y_en_un_paso() {
    al_lienzo::vaciar_para_pruebas();
    let _editor = al_lienzo::EditorAbierto::nuevo(1);
    let motor = Arc::new(MotorGuionado::nuevo(vec![
        Respuesta::de_llamada(
            "dibujar_en_lienzo",
            r#"{"elementos":[{"tipo":"elipse","x":0,"y":0,"ancho":100,"alto":100,"color":"rojo","relleno":"rojo"},
                             {"tipo":"rectangulo","x":200,"y":0,"ancho":100,"alto":100,"color":"azul"}]}"#,
        ),
        Respuesta::de_texto("Listo: un circulo rojo y un cuadrado azul."),
    ]));
    let mut escena = pixpin_motor2d::Escena::nueva();
    let mut t = Turno::lanzar(motor.clone(), "p".into(), historial("dibuja"), vec![], None);
    let (vivo, avisos) = hasta_el_final(&mut t, Some(&mut escena));
    assert!(vivo.texto.contains("Listo"), "{}", vivo.texto);
    assert!(vivo.fallos.is_empty(), "{:?}", vivo.fallos);
    assert_eq!(escena.cuantos_visibles(), 2);
    // Centrado en la vista (1000, 1000): la caja va de 850 a 1150.
    let caja = al_lienzo::caja(&escena.visibles().cloned().collect::<Vec<_>>()).unwrap();
    assert!(((caja.0 + caja.2) / 2.0 - 1000.0).abs() < 1.0, "{caja:?}");
    assert!(((caja.1 + caja.3) / 2.0 - 1000.0).abs() < 1.0, "{caja:?}");
    // Un solo paso de deshacer se lo lleva todo.
    assert!(escena.deshacer());
    assert_eq!(escena.cuantos_visibles(), 0);
    // El modelo recibio lo que paso de verdad.
    let pedidas = motor.recibido.lock().unwrap();
    assert!(
        pedidas[1]
            .mensajes
            .last()
            .unwrap()
            .texto_entero()
            .contains("2 elementos")
    );
    assert!(
        avisos
            .iter()
            .any(|a| matches!(a, Aviso::Evento(Evento::Resultado { ok: true, .. })))
    );
}

/// Dos rectas que se cruzan: una roja horizontal y una azul diagonal.
fn cruce() -> pixpin_motor2d::Escena {
    use pixpin_motor2d::{ColorRgba, Elemento, Punto2, elemento::Figura};
    let raya = |a: (f32, f32), b: (f32, f32), c: ColorRgba| Elemento {
        figura: Figura::Linea {
            puntos: vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
        },
        x: a.0.min(b.0),
        y: a.1.min(b.1),
        trazo: c,
        ..Default::default()
    };
    let mut s = pixpin_motor2d::Escena::nueva();
    s.anadir(raya(
        (900.0, 1000.0),
        (1200.0, 1000.0),
        ColorRgba::opaco(0.88, 0.19, 0.19),
    ));
    s.anadir(raya(
        (1000.0, 900.0),
        (1100.0, 1200.0),
        ColorRgba::opaco(0.10, 0.44, 0.76),
    ));
    s
}

#[test]
fn editar_el_lienzo_abierto_con_su_resumen_y_en_un_paso() {
    al_lienzo::vaciar_para_pruebas();
    let _editor = al_lienzo::EditorAbierto::nuevo(1);
    let motor = Arc::new(MotorGuionado::nuevo(vec![
        Respuesta::de_llamada(
            "cortar_linea",
            r#"{"que":"la recta azul","con":"la recta roja","quitar":"fin"}"#,
        ),
        Respuesta::de_texto("Cortada en el cruce."),
    ]));
    let mut escena = cruce();
    let pasos = escena.pasos_cerrados();
    let mut t = Turno::lanzar(
        motor.clone(),
        "p".into(),
        historial("corta la recta azul en el cruce"),
        vec![],
        None,
    );
    let (vivo, _) = hasta_el_final(&mut t, Some(&mut escena));
    assert!(vivo.fallos.is_empty(), "{:?}", vivo.fallos);
    // El modelo recibio el resumen del lienzo con el pedido.
    let pedidas = motor.recibido.lock().unwrap();
    let primero = pedidas[0].mensajes.last().unwrap().texto_entero();
    assert!(
        primero.contains("Lienzo destino: 2 elementos") && primero.contains("recta azul"),
        "{primero}"
    );
    // La azul acaba en la roja (y = 1000) y es un solo paso de deshacer.
    let azul = escena.visibles().nth(1).unwrap().puntos().unwrap().to_vec();
    assert!(azul.iter().all(|p| p.y <= 1000.5), "{azul:?}");
    assert_eq!(escena.pasos_cerrados(), pasos + 1);
    assert!(
        pedidas[1]
            .mensajes
            .last()
            .unwrap()
            .texto_entero()
            .contains("hecho: cortada")
    );
}

#[test]
fn una_edicion_imposible_vuelve_al_modelo_y_no_toca_el_lienzo() {
    // Casos negativos: un #id que no existe y cortar contra si misma.
    al_lienzo::vaciar_para_pruebas();
    let _editor = al_lienzo::EditorAbierto::nuevo(1);
    let motor = Arc::new(MotorGuionado::nuevo(vec![
        Respuesta::de_llamada(
            "transformar_elementos",
            r##"{"que":"#99","accion":"borrar"}"##,
        ),
        Respuesta::de_llamada(
            "cortar_linea",
            r#"{"que":"la recta roja","con":"la recta roja"}"#,
        ),
        Respuesta::de_texto("No pude."),
    ]));
    let mut escena = cruce();
    let antes = escena.clone();
    let mut t = Turno::lanzar(
        motor.clone(),
        "p".into(),
        historial("borra el 99 y corta la roja"),
        vec![],
        None,
    );
    let (_, avisos) = hasta_el_final(&mut t, Some(&mut escena));
    assert_eq!(escena.elementos, antes.elementos);
    let pedidas = motor.recibido.lock().unwrap();
    assert!(
        pedidas[1]
            .mensajes
            .last()
            .unwrap()
            .texto_entero()
            .contains("no existe #99")
    );
    assert!(
        pedidas[2]
            .mensajes
            .last()
            .unwrap()
            .texto_entero()
            .contains("misma linea")
    );
    assert!(
        avisos
            .iter()
            .any(|a| matches!(a, Aviso::Evento(Evento::Resultado { ok: false, .. })))
    );
}

#[test]
fn sin_nadie_que_atienda_el_lienzo_el_modelo_se_entera_del_fallo() {
    al_lienzo::vaciar_para_pruebas();
    // Ni editor ni chat atienden la cola: la espera se agota.
    let (tx, rx) = std::sync::mpsc::channel();
    let parar = Arc::new(AtomicBool::new(false));
    let mut ej = super::ejecutor::EjecutorDelChat::nuevo(tx, "p".into(), vec![], parar)
        .con_espera(Duration::from_millis(200));
    let llamada = pixpin_ia::herramientas::validar(
        &pixpin_ia::motor::LlamadaCruda {
            id: "1".into(),
            nombre: "grafica".into(),
            argumentos: r#"{"tipo":"barras","etiquetas":["a","b"],"valores":[1,2]}"#.into(),
        },
        &pixpin_ia::herramientas::catalogo(),
    )
    .unwrap()
    .llamada;
    let r = pixpin_ia::agente::Ejecutor::ejecutar(&mut ej, &llamada);
    assert!(r.unwrap_err().contains("no contesto a tiempo"));
    drop(rx);
    al_lienzo::vaciar_para_pruebas();
}

#[test]
fn el_ocr_sin_adjuntos_falla_y_la_burbuja_lo_dice() {
    let motor = Arc::new(MotorGuionado::nuevo(vec![
        Respuesta::de_llamada("ocr", "{}"),
        Respuesta::de_texto("No veo ningun documento adjunto."),
    ]));
    let mut t = Turno::lanzar(
        motor.clone(),
        "p".into(),
        historial("lee el pdf"),
        vec![],
        None,
    );
    let (vivo, _) = hasta_el_final(&mut t, None);
    assert!(vivo.mostrar().contains("No veo ningun documento"));
    assert!(vivo.mostrar().contains("⚠️"), "{}", vivo.mostrar());
    let pedidas = motor.recibido.lock().unwrap();
    assert!(
        pedidas[1]
            .mensajes
            .last()
            .unwrap()
            .texto_entero()
            .starts_with("ERROR: no hay ningun PDF")
    );
}

#[test]
fn un_motor_que_falla_deja_el_motivo_en_la_burbuja() {
    // El guion vacio: el motor contesta con un error, como uno sin modelo.
    let motor = Arc::new(MotorGuionado::nuevo(vec![]));
    let mut t = Turno::lanzar(motor, "p".into(), historial("hola"), vec![], None);
    let (vivo, _) = hasta_el_final(&mut t, None);
    assert!(vivo.acabado);
    // Un aviso en palabras; lo tecnico («el guion se acabo») va al registro.
    assert!(vivo.mostrar().starts_with("⚠️"), "{}", vivo.mostrar());
    assert!(!vivo.mostrar().contains("guion"), "{}", vivo.mostrar());
}

/// Un motor que escribe despacio, para parar a mitad.
struct Lento;

impl pixpin_ia::Motor for Lento {
    fn completar(
        &self,
        _p: &pixpin_ia::Peticion,
        al_trozo: &mut dyn FnMut(&pixpin_ia::motor::Trozo) -> pixpin_ia::motor::Seguir,
    ) -> Result<Respuesta, ErrorIa> {
        let mut texto = String::new();
        for i in 0..500 {
            let t = format!("palabra{i} ");
            texto.push_str(&t);
            if al_trozo(&pixpin_ia::motor::Trozo::Texto(t)) == pixpin_ia::motor::Seguir::Parar {
                return Ok(Respuesta {
                    fin: pixpin_ia::motor::Fin::Cortado,
                    ..Respuesta::de_texto(texto)
                });
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(Respuesta::de_texto(texto))
    }
}

#[test]
fn parar_a_mitad_corta_y_lo_dice() {
    let mut t = Turno::lanzar(
        Arc::new(Lento),
        "p".into(),
        historial("cuenta"),
        vec![],
        None,
    );
    std::thread::sleep(Duration::from_millis(60));
    t.parar();
    let (vivo, _) = hasta_el_final(&mut t, None);
    assert!(vivo.texto.contains("palabra0"));
    assert!(!vivo.texto.contains("palabra499"), "no paro");
    assert!(vivo.mostrar().ends_with("⏹ (parado)"), "{}", vivo.mostrar());
}

#[test]
fn la_foto_va_reducida_al_ultimo_mensaje() {
    let c = carpeta("foto");
    let grande = pixpin_codec::ImagenRgba {
        ancho: 2000,
        alto: 1000,
        pixeles: vec![200; 2000 * 1000 * 4],
    };
    let ruta = c.join("foto.png");
    std::fs::write(&ruta, pixpin_codec::imagen::codificar_png(&grande).unwrap()).unwrap();
    let motor = Arc::new(MotorGuionado::nuevo(vec![Respuesta::de_texto(
        "Veo un gris.",
    )]));
    let mut t = Turno::lanzar(
        motor.clone(),
        "p".into(),
        historial("que ves"),
        vec![],
        Some(ruta),
    );
    hasta_el_final(&mut t, None);
    let pedidas = motor.recibido.lock().unwrap();
    let ultimo = pedidas[0].mensajes.last().unwrap();
    assert!(ultimo.tiene_imagen());
    assert_eq!(ultimo.texto_entero(), "que ves");
    let pixpin_ia::Parte::Imagen { bytes, mime } = &ultimo.partes[0] else {
        panic!()
    };
    assert_eq!(mime, "image/jpeg");
    let tmp = c.join("mandada.jpg");
    std::fs::write(&tmp, bytes).unwrap();
    assert_eq!(pixpin_codec::imagen::medidas(&tmp).unwrap(), (768, 384));
    // Una foto que no se puede leer no impide contestar.
    let motor = Arc::new(MotorGuionado::nuevo(vec![Respuesta::de_texto("Sin foto.")]));
    let mut t = Turno::lanzar(
        motor.clone(),
        "p".into(),
        historial("y esta"),
        vec![],
        Some(c.join("no-esta.png")),
    );
    let (vivo, _) = hasta_el_final(&mut t, None);
    assert_eq!(vivo.texto, "Sin foto.");
    assert!(
        !motor.recibido.lock().unwrap()[0]
            .mensajes
            .last()
            .unwrap()
            .tiene_imagen()
    );
    let _ = std::fs::remove_dir_all(&c);
}

#[test]
fn medidas_reducidas_nunca_amplian() {
    use super::foto::medidas_reducidas;
    assert_eq!(medidas_reducidas(4000, 3000, 768), (768, 576));
    assert_eq!(medidas_reducidas(300, 200, 768), (300, 200));
    assert_eq!(medidas_reducidas(100, 5000, 768), (15, 768));
}

#[test]
fn la_mencion_se_reconoce_y_se_quita() {
    assert_eq!(
        chat::mencion("@asistente dibuja una casa").as_deref(),
        Some("dibuja una casa")
    );
    assert_eq!(
        chat::mencion("oye @Asistente, que es esto?").as_deref(),
        Some("oye , que es esto?")
    );
    assert_eq!(chat::mencion("@asistente: hola").as_deref(), Some("hola"));
    assert_eq!(chat::mencion("hola a todos"), None);
    assert_eq!(chat::mencion("@asistentes hola"), None);
    assert_eq!(chat::mencion("correo@asistente.com"), None);
}

#[test]
fn la_conversacion_separa_usuario_y_asistente_y_la_foto_es_la_reciente() {
    use pixpin_proyecto::cuaderno::{Clase, Mensaje as Nota};
    let nota = |t: &str, ap: &str| Nota {
        id: t.into(),
        texto: t.into(),
        aparato: Some(ap.into()),
        ..Nota::default()
    };
    let foto = Nota {
        id: "f".into(),
        clase: Some(Clase::Imagen),
        ruta: Some("imagenes/f.png".into()),
        ..Nota::default()
    };
    let ms = vec![
        nota("hola", "K7Q2"),
        nota("que tal", chat::APARATO),
        foto.clone(),
    ];
    let c = chat::conversacion(&ms);
    assert_eq!(
        c,
        vec![(true, "hola".to_string()), (false, "que tal".to_string())]
    );
    assert_eq!(chat::foto_reciente(&ms).map(|m| m.id.as_str()), Some("f"));
    // La foto de la que se hablo antes sigue a la vista: «dibuja eso» tras
    // la descripcion tiene que verla (13-oct).
    let ms2 = vec![
        foto.clone(),
        nota("es un plano", chat::APARATO),
        nota("dibuja eso", "K7Q2"),
    ];
    assert_eq!(chat::foto_reciente(&ms2).map(|m| m.id.as_str()), Some("f"));
    // Pero no una de hace mucho.
    let mut ms3 = vec![foto];
    ms3.extend((0..chat::FOTO_ATRAS).map(|i| nota(&format!("m{i}"), "K7Q2")));
    assert!(chat::foto_reciente(&ms3).is_none());
}

fn nota_de(texto: &str, aparato: &str) -> pixpin_proyecto::cuaderno::Mensaje {
    pixpin_proyecto::cuaderno::Mensaje {
        id: texto.into(),
        texto: texto.into(),
        aparato: Some(aparato.into()),
        ..Default::default()
    }
}

fn lienzo_de_prueba(
    nombre: &str,
    referencia: &str,
    aparato: &str,
) -> pixpin_proyecto::cuaderno::Mensaje {
    pixpin_proyecto::cuaderno::Mensaje {
        id: referencia.into(),
        clase: Some(pixpin_proyecto::cuaderno::Clase::Dibujo),
        nombre: nombre.into(),
        referencia: Some(referencia.into()),
        aparato: Some(aparato.into()),
        numero: 7,
        ..Default::default()
    }
}

#[test]
fn una_foto_sola_al_asistente_es_una_pregunta() {
    use pixpin_proyecto::cuaderno::Clase;
    let foto = pixpin_proyecto::cuaderno::Mensaje {
        clase: Some(Clase::Imagen),
        aparato: Some("K7Q2".into()),
        ..Default::default()
    };
    // En su chat, sin pie: se le pide que la describa y pregunte.
    assert_eq!(
        chat::pregunta_de(&foto, true).as_deref(),
        Some(chat::PREGUNTA_SOLO_FOTO)
    );
    // Con pie, el pie es la pregunta.
    let con_pie = pixpin_proyecto::cuaderno::Mensaje {
        texto: "¿qué es?".into(),
        ..foto.clone()
    };
    assert_eq!(
        chat::pregunta_de(&con_pie, true).as_deref(),
        Some("¿qué es?")
    );
    // En otro chat, solo si el pie lo menciona.
    assert_eq!(chat::pregunta_de(&foto, false), None);
    let mencion = pixpin_proyecto::cuaderno::Mensaje {
        texto: "@asistente ¿qué es?".into(),
        ..foto.clone()
    };
    assert_eq!(
        chat::pregunta_de(&mencion, false).as_deref(),
        Some("¿qué es?")
    );
    // Lo suyo no se contesta a si mismo, y un PDF suelto no es pregunta.
    assert_eq!(
        chat::pregunta_de(&nota_de("hola", chat::APARATO), true),
        None
    );
    let pdf = pixpin_proyecto::cuaderno::Mensaje {
        clase: Some(Clase::Archivo),
        ..foto
    };
    assert_eq!(chat::pregunta_de(&pdf, true), None);
}

#[test]
fn una_foto_sola_siempre_tiene_respuesta_aunque_el_modelo_calle() {
    let c = carpeta("foto-sola");
    let ruta = c.join("foto.png");
    let img = pixpin_codec::ImagenRgba {
        ancho: 64,
        alto: 64,
        pixeles: vec![90; 64 * 64 * 4],
    };
    std::fs::write(&ruta, pixpin_codec::imagen::codificar_png(&img).unwrap()).unwrap();
    // El modelo no dice nada: la burbuja no se queda vacia.
    let motor = Arc::new(MotorGuionado::nuevo(vec![Respuesta::de_texto("")]));
    let mut t = Turno::lanzar(
        motor.clone(),
        "p".into(),
        historial(chat::PREGUNTA_SOLO_FOTO),
        vec![],
        Some(ruta),
    );
    let (vivo, _) = hasta_el_final(&mut t, None);
    assert_eq!(vivo.mostrar(), pixpin_ia::agente::SIN_PALABRAS);
    // Y la foto le llego.
    assert!(
        motor.recibido.lock().unwrap()[0]
            .mensajes
            .last()
            .unwrap()
            .tiene_imagen()
    );
    let _ = std::fs::remove_dir_all(&c);
}

#[test]
fn la_mencion_de_un_lienzo_se_resuelve_por_su_nombre() {
    let ms = vec![
        lienzo_de_prueba("Plano casa", "ref-plano", "K7Q2"),
        lienzo_de_prueba("", "ref-sin", "K7Q2"),
        lienzo_de_prueba("Lienzo del asistente", "ref-ia", chat::APARATO),
    ];
    assert_eq!(chat::mencion_de_lienzo(&ms[0]), "@lienzo:Plano_casa");
    let (t, d) = chat::resolver_lienzo("dibuja una puerta en @lienzo:Plano_casa, por favor", &ms);
    assert_eq!(t, "dibuja una puerta en el lienzo «Plano casa», por favor");
    assert_eq!(d.as_deref(), Some("ref-plano"));
    // Sin tildes ni mayusculas.
    let (_, d) = chat::resolver_lienzo("en @LIENZO:plano_CASA", &ms);
    assert_eq!(d.as_deref(), Some("ref-plano"));
    // Un lienzo sin nombre se llama por su numero, no por su fichero.
    assert_eq!(
        chat::nombre_de_lienzo(&lienzo_de_prueba("x.excalidraw", "r", "K")),
        "Lienzo 7"
    );
    // Una mencion que no casa no cambia nada.
    let (t, d) = chat::resolver_lienzo("en @lienzo:Fachada ya", &ms);
    assert_eq!(t, "en @lienzo:Fachada ya");
    assert_eq!(d, None);
    // El propio del asistente es el suyo, no el ultimo.
    assert_eq!(chat::lienzo_propio(&ms), Some(2));
    assert_eq!(chat::lienzo_propio(&ms[..2]), None);
}

#[test]
fn pedir_que_lo_abra() {
    assert!(chat::pide_abrir("dibuja una casa y ábrelo"));
    assert!(chat::pide_abrir("Dibújalo y abre el lienzo al terminar"));
    assert!(!chat::pide_abrir("dibuja una casa"));
    assert!(!chat::pide_abrir("¿qué es esto?"));
}

#[test]
fn la_lista_de_arroba() {
    let ms = vec![
        lienzo_de_prueba("Plano casa", "ref-plano", "K7Q2"),
        lienzo_de_prueba("Fachada", "ref-fachada", "K7Q2"),
    ];
    // Se abre con un `@` que empieza palabra, hasta el primer espacio.
    let t = "hola @pla";
    assert_eq!(
        chat::consulta_de_mencion(t, t.len()),
        Some((5, "pla".into()))
    );
    assert_eq!(chat::consulta_de_mencion("correo@pla", 10), None);
    assert_eq!(chat::consulta_de_mencion("@pla casa", 9), None);
    assert_eq!(chat::consulta_de_mencion("sin nada", 8), None);
    // Fuera del chat del asistente sale tambien el asistente.
    let todas = chat::opciones_de_mencion(&ms, "", false);
    assert_eq!(todas.len(), 3);
    assert_eq!(todas[0].mencion, "@asistente");
    // Del mas nuevo al mas viejo, y filtradas por lo escrito.
    assert_eq!(todas[1].mencion, "@lienzo:Fachada");
    let pla = chat::opciones_de_mencion(&ms, "pla", true);
    assert_eq!(pla.len(), 1);
    assert_eq!(pla[0].mencion, "@lienzo:Plano_casa");
    assert!(
        chat::opciones_de_mencion(&ms, "lienzo:fa", true)[0]
            .mencion
            .ends_with("Fachada")
    );
    assert!(chat::opciones_de_mencion(&ms, "zzz", true).is_empty());
    // Elegir escribe la mencion y deja la rayita detras del espacio.
    let mut texto = "hola @pla y mas".to_string();
    let fin = chat::poner_mencion(&mut texto, 5, 9, &pla[0]);
    assert_eq!(texto, "hola @lienzo:Plano_casa  y mas");
    assert_eq!(&texto[..fin], "hola @lienzo:Plano_casa ");
}

#[test]
fn el_historial_se_recorta_a_lo_ultimo() {
    let muchos: Vec<(bool, String)> = (0..30).map(|i| (i % 2 == 0, format!("m{i}"))).collect();
    let h = super::turno::historial_para_el_modelo(&muchos, "nuevo", None);
    // Sistema + 12 de antes + el nuevo.
    assert_eq!(h.len(), 14);
    assert_eq!(h[1].texto_entero(), "m18");
    assert_eq!(h.last().unwrap().texto_entero(), "nuevo");
}

#[test]
fn la_burbuja_en_vivo() {
    let mut v = EnVivo::default();
    v.aplicar(&Evento::Texto("Voy ".into()));
    assert_eq!(v.mostrar(), "Voy ▍");
    v.aplicar(&Evento::Llamando("dibujar_en_lienzo".into()));
    assert!(v.mostrar().ends_with("dibujando en el lienzo…"));
    v.aplicar(&Evento::Resultado {
        nombre: "dibujar_en_lienzo".into(),
        ok: false,
        texto: "ERROR: el lienzo esta cerrado".into(),
    });
    v.aplicar(&Evento::Texto("No pude.".into()));
    v.acabar(&Ok(pixpin_ia::agente::Final {
        texto: "No pude.".into(),
        herramientas_hechas: 0,
        reintentos: 0,
        cortado: false,
    }));
    assert_eq!(v.mostrar(), "Voy No pude.\n⚠️ el lienzo esta cerrado");
}

// ---------------------------------------------------------------------------
// Lienzo cerrado

#[test]
fn con_el_lienzo_cerrado_lo_nuevo_va_a_la_derecha_de_lo_que_habia() {
    let doc = |x: f64| {
        pixpin_ia::lienzo::documento(vec![serde_json::json!({
            "id": format!("a{x}"), "type": "rectangle", "x": x, "y": 50.0, "width": 100.0, "height": 40.0,
            "strokeColor": "#e03131", "backgroundColor": "transparent", "version": 1, "isDeleted": false
        })])
    };
    let mut escena = pixpin_motor2d::Escena::nueva();
    // Vacio: donde lo puso el modelo.
    assert_eq!(
        al_lienzo::meter_al_lado(&mut escena, &doc(10.0)).unwrap(),
        1
    );
    let c1 = al_lienzo::caja(&escena.visibles().cloned().collect::<Vec<_>>()).unwrap();
    assert!((c1.0 - 10.0).abs() < 3.0, "{c1:?}");
    // Con algo: a la derecha, sin taparlo, alineado arriba.
    assert_eq!(
        al_lienzo::meter_al_lado(&mut escena, &doc(10.0)).unwrap(),
        1
    );
    let els: Vec<_> = escena.visibles().cloned().collect();
    let segundo = els[1].caja();
    assert!(segundo.0 > c1.2 + 40.0, "{segundo:?} {c1:?}");
    assert!((segundo.1 - c1.1).abs() < 1.0);
    // Un documento que no es del lienzo no entra.
    assert!(al_lienzo::meter_al_lado(&mut escena, "no es json").is_err());
    assert_eq!(escena.cuantos_visibles(), 2);
}

// ---------------------------------------------------------------------------
// OCR

#[test]
fn elegir_el_adjunto_por_nombre_o_el_ultimo() {
    use super::ocr::elegir;
    let a = |n: &str| Adjunto {
        nombre: n.into(),
        ruta: PathBuf::from(n),
    };
    let lista = vec![a("Contrato.pdf"), a("foto.jpg")];
    assert_eq!(elegir(&lista, None).unwrap().nombre, "foto.jpg");
    assert_eq!(
        elegir(&lista, Some("contrato")).unwrap().nombre,
        "Contrato.pdf"
    );
    assert!(
        elegir(&lista, Some("factura"))
            .unwrap_err()
            .contains("Contrato.pdf")
    );
    assert!(elegir(&[], None).unwrap_err().contains("no hay ningun PDF"));
}

#[test]
fn las_hojas_pedidas() {
    use super::ocr::hojas;
    assert_eq!(hojas(&[], 3), vec![0, 1, 2]);
    assert_eq!(hojas(&[(2, 3), (1, 1), (3, 9)], 4), vec![0, 1, 2, 3]);
    assert!(hojas(&[(7, 9)], 4).is_empty());
}

#[test]
fn el_ocr_no_lee_lo_que_no_es_pdf_ni_imagen() {
    let c = carpeta("ocr-no");
    let no = AtomicBool::new(false);
    let docx = c.join("a.docx");
    std::fs::write(&docx, b"PK").unwrap();
    assert!(
        super::ocr::leer(&docx, &[], &no)
            .unwrap_err()
            .contains("solo se puede")
    );
    assert!(
        super::ocr::leer(&c.join("falta.pdf"), &[], &no)
            .unwrap_err()
            .contains("ya no esta")
    );
    let _ = std::fs::remove_dir_all(&c);
}

/// Una pagina con texto, pintada a pixeles: lo que seria un escaneo.
fn pagina_con_texto(c: &std::path::Path) -> pixpin_codec::ImagenRgba {
    let llamada = pixpin_ia::herramientas::validar(
        &pixpin_ia::motor::LlamadaCruda {
            id: "1".into(),
            nombre: "dibujar_en_lienzo".into(),
            argumentos: r#"{"elementos":[{"tipo":"texto","x":20,"y":20,"texto":"HOLA PIXPIN 2026","tam_letra":64,"color":"negro"},
                                         {"tipo":"texto","x":20,"y":140,"texto":"Factura numero 4512","tam_letra":48,"color":"negro"},
                                         {"tipo":"texto","x":20,"y":240,"texto":"Total a pagar ciento veinte soles","tam_letra":40,"color":"negro"}]}"#
                .into(),
        },
        &pixpin_ia::herramientas::catalogo(),
    )
    .unwrap()
    .llamada;
    let doc = pixpin_ia::lienzo::documento(pixpin_ia::lienzo::elementos(&llamada, (0.0, 0.0), 1));
    let escena =
        pixpin_motor2d::excalidraw::a_escena(&pixpin_motor2d::excalidraw::leer(&doc).unwrap());
    let hojas = pixpin_motor2d::exportar::hojas(
        &escena,
        pixpin_motor2d::exportar::Alcance::Todo,
        &[],
        None,
    );
    let blanco = pixpin_motor2d::ColorRgba::opaco(1.0, 1.0, 1.0);
    let pdf = pixpin_pdf::escribir::de_hojas(&hojas, Some(blanco), &|_| None).unwrap();
    let ruta = c.join("vectorial.pdf");
    std::fs::write(&ruta, pdf).unwrap();
    pixpin_pdf::Documento::abrir(&ruta)
        .unwrap()
        .renderizar(0, 1400)
        .unwrap()
}

#[test]
fn el_ocr_lee_una_imagen_y_un_pdf_escaneado() {
    if !pixpin_ocr::disponible() {
        eprintln!("este Windows no tiene OCR: se salta");
        return;
    }
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let c = carpeta("ocr");
    let no = AtomicBool::new(false);
    let pagina = pagina_con_texto(&c);
    // La imagen.
    let png = c.join("escaneo.png");
    std::fs::write(&png, pixpin_codec::imagen::codificar_png(&pagina).unwrap()).unwrap();
    let texto = super::ocr::leer(&png, &[], &no).unwrap();
    assert!(texto.to_uppercase().contains("PIXPIN"), "{texto}");
    assert!(texto.contains("4512"), "{texto}");
    // El mismo escaneo como PDF sin capa de texto: se pinta y se lee.
    let pdf = pixpin_pdf::union::de_imagenes(&[pagina.clone(), pagina]).unwrap();
    let ruta = c.join("escaneado.pdf");
    std::fs::write(&ruta, pdf).unwrap();
    let texto = super::ocr::leer(&ruta, &[(2, 2)], &no).unwrap();
    assert!(texto.contains("4512"), "{texto}");
    // Una sola hoja pedida: sin el rotulo de hoja.
    assert!(!texto.contains("--- Hoja"));
    let texto = super::ocr::leer(&ruta, &[], &no).unwrap();
    assert!(texto.contains("--- Hoja 2 ---"), "{texto}");
    // Hojas que no existen.
    assert!(
        super::ocr::leer(&ruta, &[(5, 6)], &no)
            .unwrap_err()
            .contains("no existen")
    );
    // Parado antes de empezar.
    assert!(
        super::ocr::leer(&ruta, &[], &AtomicBool::new(true))
            .unwrap_err()
            .contains("paro")
    );
    let _ = std::fs::remove_dir_all(&c);
}

#[test]
fn un_pdf_con_capa_de_texto_se_lee_sin_ocr() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let c = carpeta("ocr-capa");
    let _ = pagina_con_texto(&c);
    // El PDF de vectores trae su texto: sale exacto aunque no haya OCR.
    let texto = super::ocr::leer(&c.join("vectorial.pdf"), &[], &AtomicBool::new(false)).unwrap();
    assert!(texto.contains("Factura numero 4512"), "{texto}");
    let _ = std::fs::remove_dir_all(&c);
}

#[test]
fn el_ocr_en_un_turno_manda_el_texto_entero_al_chat_y_recortado_al_modelo() {
    if !pixpin_ocr::disponible() {
        return;
    }
    let c = carpeta("ocr-turno");
    let pagina = {
        let _com = pixpin_shell::ComDelHilo::iniciar();
        pagina_con_texto(&c)
    };
    let png = c.join("recibo.png");
    std::fs::write(&png, pixpin_codec::imagen::codificar_png(&pagina).unwrap()).unwrap();
    let motor = Arc::new(MotorGuionado::nuevo(vec![
        Respuesta::de_llamada("ocr", r#"{"archivo":"recibo"}"#),
        Respuesta::de_texto("Dice HOLA PIXPIN 2026."),
    ]));
    let adj = vec![Adjunto {
        nombre: "recibo.png".into(),
        ruta: png,
    }];
    let mut t = Turno::lanzar(motor.clone(), "p".into(), historial("que dice"), adj, None);
    let (vivo, avisos) = hasta_el_final(&mut t, None);
    assert!(vivo.fallos.is_empty(), "{:?}", vivo.fallos);
    let leido = avisos.iter().find_map(|a| match a {
        Aviso::TextoLeido { de, texto } => Some((de.nombre.clone(), texto.clone())),
        _ => None,
    });
    let (nombre, texto) = leido.expect("el texto entero llega al chat");
    assert_eq!(nombre, "recibo.png");
    assert!(texto.contains("4512"));
    let pedidas = motor.recibido.lock().unwrap();
    assert!(
        pedidas[1]
            .mensajes
            .last()
            .unwrap()
            .texto_entero()
            .contains("4512")
    );
    let _ = std::fs::remove_dir_all(&c);
}

#[test]
fn recortar_dice_que_recorta() {
    let largo = "a".repeat(5000);
    let r = super::ejecutor::recortar(&largo, 4000);
    assert!(r.starts_with(&"a".repeat(4000)));
    assert!(r.contains("recortado"));
    assert_eq!(super::ejecutor::recortar("corto", 4000), "corto");
}

// ---------------------------------------------------------------------------
// Descarga

fn esperar_descarga(d: &mut Descarga) -> AvisoDescarga {
    let hasta = Instant::now() + Duration::from_secs(10);
    while Instant::now() < hasta {
        d.mirar();
        if let Some(f) = d.acabada.clone() {
            return f;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("la descarga no acabo");
}

#[test]
fn la_descarga_cancelada_se_dice_cancelada() {
    let mut d = Descarga::lanzar_con(pixpin_ia::modelos::QWEN35_08B, |avance, cancelado| {
        let mut hecho = 0;
        loop {
            if cancelado.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(ErrorIa::Cancelado);
            }
            hecho += 1000;
            avance(Progreso {
                hecho,
                total: 1_000_000,
                fichero: "x.gguf".into(),
                verificando: false,
            });
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    std::thread::sleep(Duration::from_millis(150));
    d.mirar();
    assert!(d.fraccion() > 0.0, "llega el progreso");
    d.cancelar();
    assert_eq!(esperar_descarga(&mut d), AvisoDescarga::Cancelada);
}

#[test]
fn la_descarga_que_falla_trae_el_motivo_y_la_buena_acaba_lista() {
    let mut d = Descarga::lanzar_con(pixpin_ia::modelos::QWEN35_2B, |_, _| {
        Err(ErrorIa::HuellaDistinta {
            que: "Qwen3.5-2B-Q4_K_M.gguf".into(),
        })
    });
    match esperar_descarga(&mut d) {
        AvisoDescarga::Fallo(m) => assert!(m.contains("huella"), "{m}"),
        otro => panic!("{otro:?}"),
    }
    let mut d = Descarga::lanzar_con(pixpin_ia::modelos::QWEN35_2B, |_, _| Ok(()));
    assert_eq!(esperar_descarga(&mut d), AvisoDescarga::Lista);
}

#[test]
fn sin_modelo_bajado_no_hay_motor_y_se_sabe_cuanto_falta() {
    let c = carpeta("sin-modelo");
    let m = pixpin_ia::modelos::QWEN35_2B;
    assert!(!super::descarga::listo(&c, &m));
    assert!(super::motor::del_proceso(&c, &m).is_err());
    assert_eq!(
        super::descarga::falta(&c, &m),
        m.bytes() + pixpin_ia::modelos::LLAMA_CPP.zip.bytes
    );
    assert_eq!(super::descarga::tamano(1_981_062_350), "2,0 GB");
    assert_eq!(super::descarga::tamano(737_504_352), "738 MB");
    let _ = std::fs::remove_dir_all(&c);
}

#[test]
fn el_modelo_de_los_ajustes() {
    use pixpin_ia::modelos::{QWEN35_2B, QWEN35_08B};
    assert_eq!(super::motor::modelo_elegido(None), QWEN35_2B);
    assert_eq!(
        super::motor::modelo_elegido(Some("qwen3.5-0.8b")),
        QWEN35_08B
    );
    assert_eq!(super::motor::modelo_elegido(Some("inventado")), QWEN35_2B);
}

// ---------------------------------------------------------------------------
// De verdad (modelo bajado)

/// **De extremo a extremo con el modelo de verdad**: pedir un dibujo, que
/// acabe en un lienzo de prueba y pintarlo a PNG; y el OCR de una imagen con
/// texto por el turno entero.
///
/// ```text
/// PIXPIN_IA_MOTOR=<carpeta con llama-server.exe>
/// PIXPIN_IA_MODELO=<Qwen3.5-2B-Q4_K_M.gguf>  PIXPIN_IA_SALIDA=<carpeta de los PNG>
/// cargo test -p pixpin -- --ignored --nocapture asistente::pruebas::de_verdad
/// ```
#[test]
#[ignore = "necesita el motor y el modelo bajados (ver la cabecera)"]
fn de_verdad() {
    use pixpin_ia::servidor::{Config, Dispositivo, MotorLlama};
    let motor_dir = PathBuf::from(std::env::var("PIXPIN_IA_MOTOR").expect("PIXPIN_IA_MOTOR"));
    let modelo = PathBuf::from(std::env::var("PIXPIN_IA_MODELO").expect("PIXPIN_IA_MODELO"));
    let salida = PathBuf::from(
        std::env::var("PIXPIN_IA_SALIDA")
            .unwrap_or_else(|_| std::env::temp_dir().display().to_string()),
    );
    let mut config = Config::nueva(motor_dir.join("llama-server.exe"), modelo, None);
    config.dispositivo = match std::env::var("PIXPIN_IA_DISPOSITIVO").as_deref() {
        Ok("cpu") | Err(_) => Dispositivo::Cpu,
        Ok(d) => Dispositivo::Gpu(d.into()),
    };
    let motor: Arc<dyn pixpin_ia::Motor> = Arc::new(MotorLlama::nuevo(config));

    // 1. Un dibujo, al lienzo «abierto» (una escena de prueba).
    al_lienzo::vaciar_para_pruebas();
    let editor = al_lienzo::EditorAbierto::nuevo(1);
    let mut escena = pixpin_motor2d::Escena::nueva();
    let t0 = Instant::now();
    let mut t = Turno::lanzar(
        motor.clone(),
        "p".into(),
        historial(
            "Dibuja un circulo rojo a la izquierda y un cuadrado azul a la derecha, con el titulo CASA arriba.",
        ),
        vec![],
        None,
    );
    let (vivo, _) = hasta_el_final_largo(&mut t, &mut escena);
    drop(editor);
    println!(
        "dibujo: {:.1} s, {} elementos\n{}",
        t0.elapsed().as_secs_f32(),
        escena.cuantos_visibles(),
        vivo.mostrar()
    );
    assert!(escena.cuantos_visibles() > 0, "{}", vivo.mostrar());
    pintar_a_png(&escena, &salida.join("asistente-dibujo.png"));

    // 2. Una grafica.
    al_lienzo::vaciar_para_pruebas();
    let editor = al_lienzo::EditorAbierto::nuevo(1);
    let mut escena = pixpin_motor2d::Escena::nueva();
    let mut t = Turno::lanzar(
        motor.clone(),
        "p".into(),
        historial("Haz una grafica de barras de ventas: enero 120, febrero 95, marzo 140."),
        vec![],
        None,
    );
    let (vivo, _) = hasta_el_final_largo(&mut t, &mut escena);
    drop(editor);
    println!(
        "grafica: {} elementos\n{}",
        escena.cuantos_visibles(),
        vivo.mostrar()
    );
    pintar_a_png(&escena, &salida.join("asistente-grafica.png"));

    // 3. OCR de una imagen con texto, por el turno entero.
    if pixpin_ocr::disponible() {
        let c = carpeta("de-verdad-ocr");
        let pagina = {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            pagina_con_texto(&c)
        };
        let png = c.join("recibo.png");
        std::fs::write(&png, pixpin_codec::imagen::codificar_png(&pagina).unwrap()).unwrap();
        let mut t = Turno::lanzar(
            motor.clone(),
            "p".into(),
            historial(
                "Lee el texto de la imagen adjunta recibo.png con ocr y dime que numero de factura tiene.",
            ),
            vec![Adjunto {
                nombre: "recibo.png".into(),
                ruta: png,
            }],
            None,
        );
        let mut nada = pixpin_motor2d::Escena::nueva();
        let (vivo, avisos) = hasta_el_final_largo(&mut t, &mut nada);
        println!("ocr:\n{}", vivo.mostrar());
        let leido = avisos
            .iter()
            .any(|a| matches!(a, Aviso::TextoLeido { texto, .. } if texto.contains("4512")));
        assert!(leido, "el OCR tiene que haber leido el recibo");
    }
    motor.soltar();
}

/// **Los tres fallos del 13-oct, con el modelo de verdad** (2B con su
/// vision, en la CPU para no pelear con la app abierta por la tarjeta):
///
/// 1. una foto sola tiene respuesta;
/// 2. «dibuja esto» con la foto deja figuras en el lienzo (PNG);
/// 3. un pedido que daria una llamada enorme no acaba en un error tecnico.
///
/// ```text
/// PIXPIN_IA_MOTOR=<carpeta con llama-server.exe>
/// PIXPIN_IA_MODELO=<Qwen3.5-2B-Q4_K_M.gguf>  PIXPIN_IA_VISION=<mmproj-F16.gguf>
/// PIXPIN_IA_SALIDA=<carpeta de los PNG>
/// PIXPIN_IA_PASOS=1,2,3 (los que se quieran; por omision, todos)
/// PIXPIN_IA_DISPOSITIVO=Vulkan1 (por omision, la CPU)
/// cargo test -p pixpin -- --ignored --nocapture asistente::pruebas::de_verdad_arreglos
/// ```
#[test]
#[ignore = "necesita el motor, el modelo y su vision bajados (ver la cabecera)"]
fn de_verdad_arreglos() {
    use pixpin_ia::servidor::{Config, MotorLlama};
    let motor_dir = PathBuf::from(std::env::var("PIXPIN_IA_MOTOR").expect("PIXPIN_IA_MOTOR"));
    let modelo = PathBuf::from(std::env::var("PIXPIN_IA_MODELO").expect("PIXPIN_IA_MODELO"));
    let vision = PathBuf::from(std::env::var("PIXPIN_IA_VISION").expect("PIXPIN_IA_VISION"));
    let salida = PathBuf::from(
        std::env::var("PIXPIN_IA_SALIDA")
            .unwrap_or_else(|_| std::env::temp_dir().display().to_string()),
    );
    std::fs::create_dir_all(&salida).unwrap();
    let mut config = Config::nueva(motor_dir.join("llama-server.exe"), modelo, Some(vision));
    config.registro = Some(salida.join("servidor-arreglos.log"));
    // En la CPU por omision; `PIXPIN_IA_DISPOSITIVO=Vulkan1` para la tarjeta.
    if let Ok(d) = std::env::var("PIXPIN_IA_DISPOSITIVO")
        && d != "cpu"
    {
        config.dispositivo = pixpin_ia::servidor::Dispositivo::Gpu(d);
    }
    let motor: Arc<dyn pixpin_ia::Motor> = Arc::new(MotorLlama::nuevo(config));

    // La foto: una casa sencilla (cuadrado azul, tejado rojo, puerta).
    let c = carpeta("de-verdad-arreglos");
    let foto = c.join("casa.png");
    {
        let (w, h) = (400u32, 400u32);
        let mut px = vec![255u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let (xf, yf) = (x as i32, y as i32);
                let tejado = (60..170).contains(&yf) && (xf - 200).abs() <= (yf - 60) * 140 / 110;
                let muro = (100..340).contains(&x) && (170..360).contains(&y);
                let puerta = (180..230).contains(&x) && (270..360).contains(&y);
                let col = if puerta {
                    [120, 70, 30]
                } else if muro {
                    [40, 90, 220]
                } else if tejado {
                    [220, 40, 40]
                } else {
                    [255, 255, 255]
                };
                px[i..i + 3].copy_from_slice(&col);
            }
        }
        let img = pixpin_codec::ImagenRgba {
            ancho: w,
            alto: h,
            pixeles: px,
        };
        std::fs::write(&foto, pixpin_codec::imagen::codificar_png(&img).unwrap()).unwrap();
    }

    let paso = |n: &str| {
        std::env::var("PIXPIN_IA_PASOS").map_or(true, |p| p.split(',').any(|x| x.trim() == n))
    };
    // 1. La foto sola.
    if paso("1") {
        let mut nada = pixpin_motor2d::Escena::nueva();
        let t0 = Instant::now();
        let mut t = Turno::lanzar(
            motor.clone(),
            "p".into(),
            historial(chat::PREGUNTA_SOLO_FOTO),
            vec![],
            Some(foto.clone()),
        );
        let (vivo, _) = hasta_el_final_largo(&mut t, &mut nada);
        println!(
            "1. foto sola ({:.1} s): {}",
            t0.elapsed().as_secs_f32(),
            vivo.mostrar()
        );
        assert!(!vivo.mostrar().trim().is_empty());
        assert!(!vivo.mostrar().starts_with("⚠️"), "{}", vivo.mostrar());
    }

    // 2. «Dibuja esto» con la foto, a un lienzo «abierto».
    if paso("2") {
        al_lienzo::vaciar_para_pruebas();
        let editor = al_lienzo::EditorAbierto::nuevo(1);
        let mut escena = pixpin_motor2d::Escena::nueva();
        let t0 = Instant::now();
        let mut t = Turno::lanzar(
            motor.clone(),
            "p".into(),
            historial("Dibuja esto en el lienzo."),
            vec![],
            Some(foto.clone()),
        );
        let (vivo, _) = hasta_el_final_largo(&mut t, &mut escena);
        drop(editor);
        println!(
            "2. dibuja esto ({:.1} s, {} elementos): {}",
            t0.elapsed().as_secs_f32(),
            escena.cuantos_visibles(),
            vivo.mostrar()
        );
        pintar_a_png(&escena, &salida.join("asistente-dibuja-esto.png"));
        assert!(escena.cuantos_visibles() > 0, "{}", vivo.mostrar());
    }

    // 3. Un pedido que pide una llamada enorme.
    if paso("3") {
        al_lienzo::vaciar_para_pruebas();
        let editor = al_lienzo::EditorAbierto::nuevo(1);
        let mut escena = pixpin_motor2d::Escena::nueva();
        let t0 = Instant::now();
        let mut t = Turno::lanzar(
            motor.clone(),
            "p".into(),
            historial(
                "Dibuja 150 lineas, cada una con su color distinto y un texto largo al lado, formando una espiral muy detallada.",
            ),
            vec![],
            None,
        );
        let (vivo, avisos) = hasta_el_final_largo(&mut t, &mut escena);
        drop(editor);
        println!(
            "3. llamada enorme ({:.1} s, {} elementos): {}",
            t0.elapsed().as_secs_f32(),
            escena.cuantos_visibles(),
            vivo.mostrar()
        );
        pintar_a_png(&escena, &salida.join("asistente-llamada-enorme.png"));
        let texto = vivo.mostrar();
        for tecnico in ["HTTP", "json", "JSON", "Failed", "parse", "{\""] {
            assert!(
                !texto.contains(tecnico),
                "se ve algo tecnico ({tecnico}): {texto}"
            );
        }
        assert!(
            avisos
                .iter()
                .any(|a| matches!(a, Aviso::Fin { resultado: Ok(_) })),
            "{texto}"
        );
        assert!(escena.cuantos_visibles() as u64 <= pixpin_ia::agente::TOPE_LISTAS * 4);
    }
    motor.soltar();
    let _ = std::fs::remove_dir_all(&c);
}

fn hasta_el_final_largo(
    t: &mut Turno,
    escena: &mut pixpin_motor2d::Escena,
) -> (EnVivo, Vec<Aviso>) {
    let mut vivo = EnVivo::default();
    let mut todos = Vec::new();
    let t0 = Instant::now();
    // Solo para no colgarse: con la CPU ocupada en otra cosa, un turno
    // puede ir a 0,3 tokens/s (medido con dos compilaciones a la vez).
    let espera = std::env::var("PIXPIN_IA_ESPERA_S")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1800);
    let hasta = t0 + Duration::from_secs(espera);
    while Instant::now() < hasta {
        al_lienzo::atender_en_editor(escena, (400.0, 300.0));
        for a in t.recibir() {
            match &a {
                Aviso::Evento(e) => {
                    if !matches!(e, Evento::Texto(_)) {
                        println!("  [{:>5.1} s] {e:?}", t0.elapsed().as_secs_f32());
                    }
                    vivo.aplicar(e)
                }
                Aviso::Fin { resultado } => vivo.acabar(resultado),
                // Sin nadie que conteste, lo que borra o envia no se hace.
                Aviso::Confirmar { responder, .. } => {
                    let _ = responder.send(false);
                }
                // Ni a las dudas: sigue con la recomendada.
                Aviso::Preguntar { responder, .. } => {
                    let _ = responder.send(pixpin_ia::preguntar::Contestada::SinRespuesta);
                }
                Aviso::TextoLeido { .. } | Aviso::Registro(_) | Aviso::Citas(_) => {}
            }
            todos.push(a);
        }
        if t.acabado() {
            return (vivo, todos);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    t.parar();
    panic!("el turno no acabo en {espera} s: {}", vivo.mostrar());
}

/// La escena pintada a PNG por el mismo camino que el PDF (exportar → PDF de
/// vectores → pixeles), para mirarla.
pub(super) fn pintar_a_png(escena: &pixpin_motor2d::Escena, ruta: &std::path::Path) {
    let hojas =
        pixpin_motor2d::exportar::hojas(escena, pixpin_motor2d::exportar::Alcance::Todo, &[], None);
    if hojas.is_empty() {
        return;
    }
    let blanco = pixpin_motor2d::ColorRgba::opaco(1.0, 1.0, 1.0);
    let pdf = pixpin_pdf::escribir::de_hojas(&hojas, Some(blanco), &|_| None).unwrap();
    let tmp = ruta.with_extension("pdf");
    std::fs::write(&tmp, pdf).unwrap();
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let img = pixpin_pdf::Documento::abrir(&tmp)
        .unwrap()
        .renderizar(0, 1000)
        .unwrap();
    std::fs::write(ruta, pixpin_codec::imagen::codificar_png(&img).unwrap()).unwrap();
    println!("PNG: {}", ruta.display());
}

/// **El perrito del usuario** (medido 14-oct): el emoji adjunto y «dibujalo
/// en el lienzo», con el modelo de verdad, pintado a PNG para mirarlo.
///
/// ```text
/// PIXPIN_IA_MODELOS=%LOCALAPPDATA%\PixPinMax\modelos  PIXPIN_IA_SALIDA=<carpeta con perrito.png>
/// PIXPIN_IA_DISPOSITIVO=cpu|Vulkan1   PIXPIN_IA_NOMBRE=perrito-en-el-lienzo-2.png
/// cargo test -p pixpin -- --ignored --nocapture asistente::pruebas::demo_perrito
/// ```
#[test]
#[ignore = "demostracion con el modelo real"]
fn demo_perrito() {
    use pixpin_ia::servidor::{Config, Dispositivo, MotorLlama};
    let m = PathBuf::from(std::env::var("PIXPIN_IA_MODELOS").unwrap());
    let salida = PathBuf::from(std::env::var("PIXPIN_IA_SALIDA").unwrap());
    let mut config = Config::nueva(
        m.join("llama.cpp").join("b11580").join("llama-server.exe"),
        m.join("qwen3.5-2b").join("Qwen3.5-2B-Q4_K_M.gguf"),
        Some(m.join("qwen3.5-2b").join("mmproj-F16.gguf")),
    );
    config.dispositivo = match std::env::var("PIXPIN_IA_DISPOSITIVO").as_deref() {
        Ok("cpu") | Err(_) => Dispositivo::Cpu,
        Ok(d) => Dispositivo::Gpu(d.into()),
    };
    let motor: Arc<dyn pixpin_ia::Motor> = Arc::new(MotorLlama::nuevo(config));
    al_lienzo::vaciar_para_pruebas();
    let editor = al_lienzo::EditorAbierto::nuevo(1);
    let mut escena = pixpin_motor2d::Escena::nueva();
    let t0 = Instant::now();
    let mut t = Turno::lanzar(
        motor.clone(),
        "p".into(),
        historial("Dibuja este perrito de la imagen adjunta dentro del lienzo."),
        vec![Adjunto {
            nombre: "perrito.png".into(),
            ruta: salida.join("perrito.png"),
        }],
        None,
    );
    let (vivo, _) = hasta_el_final_largo(&mut t, &mut escena);
    drop(editor);
    println!(
        "perrito: {:.1} s, {} elementos\n{}",
        t0.elapsed().as_secs_f32(),
        escena.cuantos_visibles(),
        vivo.mostrar()
    );
    let nombre =
        std::env::var("PIXPIN_IA_NOMBRE").unwrap_or_else(|_| "perrito-en-el-lienzo-2.png".into());
    pintar_a_png(&escena, &salida.join(nombre));
    motor.soltar();
    assert!(escena.cuantos_visibles() >= 4, "{}", vivo.mostrar());
}

/// «Guarda esto como figura»: el lienzo abierto contesta con lo elegido, y
/// solo si alguien pregunto (casos negativos: sin pregunta no contesta; sin
/// nada elegido, lista vacia).
#[test]
fn el_lienzo_abierto_da_lo_elegido_solo_si_se_lo_piden() {
    let mut escena = pixpin_motor2d::Escena::nueva();
    let a = escena.anadir(pixpin_motor2d::Elemento {
        ancho: 10.0,
        alto: 10.0,
        ..Default::default()
    });
    let _b = escena.anadir(pixpin_motor2d::Elemento {
        ancho: 5.0,
        alto: 5.0,
        ..Default::default()
    });
    // Sin pregunta: nada que hacer.
    al_lienzo::contestar_lo_elegido(&escena, &[a]);
    let rx = al_lienzo::pedir_lo_elegido();
    al_lienzo::contestar_lo_elegido(&escena, &[a]);
    let piezas = rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(piezas.len(), 1);
    assert_eq!(piezas[0].ancho, 10.0);
    let rx = al_lienzo::pedir_lo_elegido();
    al_lienzo::contestar_lo_elegido(&escena, &[]);
    assert!(rx.recv_timeout(Duration::from_secs(1)).unwrap().is_empty());
}

/// 17-oct: el asistente (nube) lista los lienzos del chat, crea uno con nombre
/// y pasa a trabajar en otro por su nombre: lo siguiente va a ese lienzo.
#[test]
fn el_asistente_lista_crea_y_usa_lienzos_del_chat_por_su_nombre() {
    use pixpin_ia::agente::Ejecutor;
    use pixpin_ia::herramientas::Llamada;
    let raiz = std::env::temp_dir().join(format!("pixpin-lienzos-ia-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(&raiz).unwrap();
    crate::ventana_chat::escribir_lienzo(&raiz, "obra", "PC", 1, Some("Fachada")).unwrap();
    let llamar = |ej: &mut super::ejecutor::EjecutorDelChat, h: &str, args: serde_json::Value| {
        let fila = pixpin_ia::acciones::por_nombre(h).expect("la fila");
        let a = pixpin_ia::acciones::validar(fila, args.as_object().unwrap()).expect("valida");
        ej.ejecutar(&Llamada::App(a))
    };
    let (tx, _rx) = std::sync::mpsc::channel();
    let parar = Arc::new(AtomicBool::new(false));
    let mut ej =
        super::ejecutor::EjecutorDelChat::nuevo(tx.clone(), "obra".into(), vec![], parar.clone())
            .en_la_nube(true)
            .con_raiz(Some(raiz.clone()))
            .con_espera(Duration::from_millis(200));
    let lista = llamar(&mut ej, "lienzos_del_chat", serde_json::json!({})).unwrap();
    assert!(lista.contains("«Fachada»: 0 elementos"), "{lista}");
    let hecho = llamar(
        &mut ej,
        "crear_lienzo",
        serde_json::json!({"nombre": "Planta baja"}),
    )
    .unwrap();
    assert!(hecho.contains("«Planta baja»"), "{hecho}");
    let lista = llamar(&mut ej, "lienzos_del_chat", serde_json::json!({})).unwrap();
    assert!(
        lista.contains("«Planta baja»: 0 elementos (trabajas en este)"),
        "{lista}"
    );
    // Usar otro por su nombre (sin tildes ni mayusculas): el destino cambia.
    let usado = llamar(
        &mut ej,
        "usar_lienzo",
        serde_json::json!({"nombre": "FACHADA"}),
    )
    .unwrap();
    assert!(usado.contains("«Fachada»"), "{usado}");
    let lista = llamar(&mut ej, "lienzos_del_chat", serde_json::json!({})).unwrap();
    assert!(
        lista.contains("«Fachada»: 0 elementos (trabajas en este)"),
        "{lista}"
    );
    // Casos negativos: un nombre que no esta dice los que hay; y con el
    // modelo local no se ofrece.
    let e = llamar(
        &mut ej,
        "usar_lienzo",
        serde_json::json!({"nombre": "Corte"}),
    )
    .unwrap_err();
    assert!(e.contains("Fachada") && e.contains("Planta baja"), "{e}");
    let mut local = super::ejecutor::EjecutorDelChat::nuevo(tx, "obra".into(), vec![], parar)
        .con_raiz(Some(raiz.clone()));
    assert!(llamar(&mut local, "lienzos_del_chat", serde_json::json!({})).is_err());
    al_lienzo::vaciar_para_pruebas();
    let _ = std::fs::remove_dir_all(&raiz);
}

/// 26-oct: el asistente (nube) mete un lienzo en una carpeta de paginas por
/// su nombre (creandola) y crea la pagina siguiente, que pasa a ser el
/// lienzo destino.
#[test]
fn el_asistente_mete_lienzos_en_una_carpeta_y_crea_la_pagina_siguiente() {
    use pixpin_ia::agente::Ejecutor;
    use pixpin_ia::herramientas::Llamada;
    use pixpin_proyecto::carpeta_de_paginas::Carpeta;
    let raiz = std::env::temp_dir().join(format!("pixpin-carpetas-ia-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(&raiz).unwrap();
    crate::ventana_chat::escribir_lienzo(&raiz, "obra", "PC", 1, Some("Día 1")).unwrap();
    let llamar = |ej: &mut super::ejecutor::EjecutorDelChat, h: &str, args: serde_json::Value| {
        let fila = pixpin_ia::acciones::por_nombre(h).expect("la fila");
        let a = pixpin_ia::acciones::validar(fila, args.as_object().unwrap()).expect("valida");
        ej.ejecutar(&Llamada::App(a))
    };
    let (tx, _rx) = std::sync::mpsc::channel();
    let parar = Arc::new(AtomicBool::new(false));
    let mut ej = super::ejecutor::EjecutorDelChat::nuevo(tx, "obra".into(), vec![], parar)
        .en_la_nube(true)
        .con_raiz(Some(raiz.clone()))
        .con_espera(Duration::from_millis(200));
    let hecho = llamar(
        &mut ej,
        "anadir_a_carpeta",
        serde_json::json!({"carpeta": "Diario", "lienzo": "dia 1"}),
    )
    .unwrap();
    assert!(
        hecho.contains("Carpeta creada") && hecho.contains("«Día 1»"),
        "{hecho}"
    );
    std::thread::sleep(Duration::from_millis(3));
    let hecho = llamar(
        &mut ej,
        "pagina_nueva_de_carpeta",
        serde_json::json!({"carpeta": "DIARIO"}),
    )
    .unwrap();
    assert!(hecho.contains("«Día 2»"), "{hecho}");
    let dir = pixpin_proyecto::almacen::carpeta(&raiz, "obra");
    let mensajes = pixpin_proyecto::cuaderno::Cuaderno::leer_de(&dir)
        .unwrap()
        .mensajes;
    let c = mensajes
        .iter()
        .find_map(Carpeta::de_mensaje)
        .expect("la carpeta");
    let titulos: Vec<&str> = c.paginas.iter().map(|p| p.titulo.as_str()).collect();
    assert_eq!(titulos, ["Día 1", "Día 2"]);
    // Casos negativos: una carpeta que no esta dice las que hay.
    let e = llamar(
        &mut ej,
        "pagina_nueva_de_carpeta",
        serde_json::json!({"carpeta": "Obra"}),
    )
    .unwrap_err();
    assert!(e.contains("Diario"), "{e}");
    al_lienzo::vaciar_para_pruebas();
    let _ = std::fs::remove_dir_all(&raiz);
}

/// 17-oct: con un lienzo abierto, lo pedido para OTRO lienzo del chat no se
/// lo lleva el editor: lo hace el chat en su fichero.
#[test]
fn lo_pedido_para_otro_lienzo_no_se_lo_lleva_el_editor_abierto() {
    al_lienzo::vaciar_para_pruebas();
    let editor = al_lienzo::EditorAbierto::nuevo(1).con_hoja(Some("dib-a".into()));
    let (tx, _rx) = std::sync::mpsc::channel();
    let para = |d: Option<&str>| {
        al_lienzo::Pedido::de_trabajo(
            "obra".into(),
            d.map(str::to_string),
            al_lienzo::Trabajo::Resumir,
            tx.clone(),
        )
    };
    al_lienzo::pedir(para(Some("dib-b")));
    al_lienzo::pedir(para(Some("dib-a")));
    al_lienzo::pedir(para(None));
    let del_editor = al_lienzo::tomar_si(al_lienzo::es_para_el_editor);
    assert_eq!(
        del_editor
            .iter()
            .map(|p| p.destino.clone())
            .collect::<Vec<_>>(),
        vec![Some("dib-a".into()), None]
    );
    let del_chat = al_lienzo::tomar_para_el_chat();
    assert_eq!(del_chat.len(), 1);
    assert_eq!(del_chat[0].destino.as_deref(), Some("dib-b"));
    drop(editor);
    // Sin decir su hoja, el editor se lo lleva todo (lo de antes).
    let _e = al_lienzo::EditorAbierto::nuevo(1);
    al_lienzo::pedir(para(Some("dib-b")));
    assert_eq!(al_lienzo::tomar_si(al_lienzo::es_para_el_editor).len(), 1);
    al_lienzo::vaciar_para_pruebas();
}

/// 23-oct: con dos lienzos abiertos a la vez, cada uno hace lo de su hoja; lo
/// que no dice hoja, el activo (el que estuvo delante el ultimo); lo de una
/// hoja cerrada, el chat. Al cerrar el activo, lo es el de antes.
#[test]
fn con_dos_lienzos_abiertos_cada_uno_hace_lo_suyo() {
    al_lienzo::vaciar_para_pruebas();
    al_lienzo::abrir_como("obra/m-a");
    let a = al_lienzo::EditorAbierto::nuevo(1).con_hoja(Some("dib-a".into()));
    al_lienzo::abrir_como("obra/m-b");
    let b = al_lienzo::EditorAbierto::nuevo(2).con_hoja(Some("dib-b".into()));
    assert!(al_lienzo::es_el_activo(2));
    // Abrir otra vez la misma hoja la trae al frente: se encuentra por su clave.
    assert_eq!(al_lienzo::editor_de_clave("obra/m-a"), Some(1));
    assert_eq!(al_lienzo::editor_de_clave("obra/m-c"), None);
    let (tx, _rx) = std::sync::mpsc::channel();
    let para = |d: Option<&str>| {
        al_lienzo::Pedido::de_trabajo(
            "obra".into(),
            d.map(str::to_string),
            al_lienzo::Trabajo::Resumir,
            tx.clone(),
        )
    };
    let destinos = |v: Vec<al_lienzo::Pedido>| v.into_iter().map(|p| p.destino).collect::<Vec<_>>();
    for d in [Some("dib-a"), Some("dib-b"), None, Some("dib-c")] {
        al_lienzo::pedir(para(d));
    }
    // Este hilo es el del lienzo 2 (el ultimo que se apunto).
    assert_eq!(
        destinos(al_lienzo::tomar_si(al_lienzo::es_para_el_editor)),
        vec![Some("dib-b".into()), None]
    );
    assert_eq!(
        destinos(al_lienzo::tomar_para_el_chat()),
        vec![Some("dib-c".into())]
    );
    // El 1 se pone delante: lo que no dice hoja es suyo, no del 2.
    al_lienzo::activar(1);
    assert!(al_lienzo::es_el_activo(1));
    al_lienzo::pedir(para(None));
    assert!(al_lienzo::tomar_si(al_lienzo::es_para_el_editor).is_empty());
    // Se cierra el 1 con lo suyo sin hacer: lo sin hoja pasa al 2 y lo de
    // «dib-a», que ya no esta abierta, al chat.
    drop(a);
    assert!(al_lienzo::es_el_activo(2));
    assert_eq!(
        destinos(al_lienzo::tomar_si(al_lienzo::es_para_el_editor)),
        vec![None]
    );
    assert_eq!(
        destinos(al_lienzo::tomar_para_el_chat()),
        vec![Some("dib-a".into())]
    );
    drop(b);
    assert!(!al_lienzo::hay_editor());
    assert_eq!(al_lienzo::editor_de_clave("obra/m-b"), None);
    al_lienzo::vaciar_para_pruebas();
}
