//! Pruebas de pensar, la peticion mejorada, la hoja de trabajo y el reinicio
//! del contexto en el turno de la app (13-oct). Con el motor de mentira; la
//! de verdad, `#[ignore]`, es [`de_verdad_conversacion_larga_con_reinicios`].

use std::sync::Arc;
use std::time::{Duration, Instant};

use pixpin_ia::agente::{Evento, Final};
use pixpin_ia::herramientas::catalogo_base as catalogo_entero;

/// Las propias sin las del lienzo (SVG, funciones, ecuacion, cronograma,
/// figuras): las cuentas de abajo se midieron con este tamano de catalogo.
fn catalogo_base() -> Vec<pixpin_ia::Herramienta> {
    use pixpin_ia::herramientas::{CRONOGRAMA, ECUACION, FIGURAS, FUNCION, SVG};
    catalogo_entero()
        .into_iter()
        .filter(|h| ![SVG, FUNCION, ECUACION, CRONOGRAMA, FIGURAS].contains(&h.nombre.as_str()))
        .collect()
}
use pixpin_ia::hoja_trabajo::{HojaDeTrabajo, Registro};
use pixpin_ia::motor::{MotorGuionado, Respuesta};
use pixpin_ia::pensar::Plan;
use pixpin_proyecto::cuaderno::{Mensaje as Nota, Sello};

use super::chat::EnVivo;
use super::pensado::{self, Seccion};
use super::turno::{Aviso, Memoria, Turno};

const PLAN: &str = "ENTIENDO: Dibujar una casa roja.\nHARE: usar dibujar_en_lienzo.\nHERRAMIENTAS: dibujar_en_lienzo\nDATOS: color rojo";

fn con_pensamiento(p: &str, t: &str) -> Respuesta {
    Respuesta {
        pensamiento: p.into(),
        ..Respuesta::de_texto(t)
    }
}

fn hasta_el_final(t: &mut Turno) -> (EnVivo, Vec<Aviso>) {
    let mut vivo = EnVivo::default();
    let mut todos = Vec::new();
    let hasta = Instant::now() + Duration::from_secs(20);
    while Instant::now() < hasta {
        for a in t.recibir() {
            match &a {
                Aviso::Evento(e) => vivo.aplicar(e),
                Aviso::Fin { resultado } => vivo.acabar(resultado),
                _ => {}
            }
            todos.push(a);
        }
        if t.acabado() {
            return (vivo, todos);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("el turno no acabo");
}

fn nota(n: i64, de_ia: bool, texto: &str) -> Nota {
    Nota::nota(
        texto,
        &Sello {
            cuando: 1_790_000_000_000 + n * 1000,
            numero: n,
            aparato: if de_ia { "IA".into() } else { "K7Q2".into() },
            proyecto: "p".into(),
        },
    )
}

#[test]
fn la_burbuja_piensa_en_vivo_se_pliega_y_guarda_la_peticion() {
    let mut v = EnVivo::default();
    v.aplicar(&Evento::Pensando("Quiere ".into()));
    v.aplicar(&Evento::Pensando("una casa.".into()));
    assert!(v.pensando);
    let s = Seccion::de_vivo(&v);
    assert!(s.pensando && s.segundos.is_none() && s.algo());
    v.aplicar(&Evento::PensoListo { segundos: 12.4 });
    v.aplicar(&Evento::Peticion(Plan::por_omision("casa")));
    v.aplicar(&Evento::Texto("Hecho.".into()));
    assert!(!v.pensando);
    assert_eq!(v.pensamiento, "Quiere una casa.");
    assert_eq!(v.segundos, Some(12.4));
    assert_eq!(v.plan.as_ref().unwrap().entiendo, "casa");
    // El texto de la burbuja es solo la respuesta.
    assert_eq!(v.mostrar(), "Hecho. ▍");
}

#[test]
fn un_cierre_de_pensar_colado_en_el_texto_se_quita_al_acabar() {
    let mut v = EnVivo::default();
    v.aplicar(&Evento::Texto("Pienso que si.".into()));
    v.aplicar(&Evento::Texto("</think>Es azul.".into()));
    v.acabar(&Ok(Final {
        texto: "Es azul.".into(),
        herramientas_hechas: 0,
        reintentos: 0,
        cortado: false,
    }));
    assert_eq!(v.mostrar(), "Es azul.");
    assert_eq!(v.pensamiento, "Pienso que si.");
}

#[test]
fn sin_pensamiento_ni_plan_no_hay_seccion() {
    let v = EnVivo::default();
    assert!(!Seccion::de_vivo(&v).algo());
    let mut v = EnVivo::default();
    v.aplicar(&Evento::Pensando("   ".into()));
    // Un pensamiento en blanco no se ensena (el agente ni lo manda; y si
    // llega, la seccion no tiene nada que decir).
    assert!(!Seccion::de_vivo(&v).penso());
}

#[test]
fn el_turno_con_memoria_reescribe_la_hoja_y_manda_el_registro_antes_del_fin() {
    let motor = Arc::new(MotorGuionado::nuevo(vec![
        con_pensamiento("El usuario quiere una casa. Sera roja.", PLAN),
        Respuesta::de_texto("Listo, una casa roja."),
    ]));
    let memoria = Memoria {
        pensar: true,
        planificar: true,
        ventana: 8192,
        ..Default::default()
    };
    let historial = vec![
        pixpin_ia::Mensaje::sistema(pixpin_ia::agente::instrucciones()),
        pixpin_ia::Mensaje::usuario("dibuja una casa roja"),
    ];
    let mut t = Turno::lanzar_con_memoria(
        motor.clone(),
        "p".into(),
        historial,
        vec![],
        None,
        None,
        vec![],
        Some(memoria),
        None,
    );
    let (vivo, avisos) = hasta_el_final(&mut t);
    assert_eq!(vivo.mostrar(), "Listo, una casa roja.");
    let i_reg = avisos
        .iter()
        .position(|a| matches!(a, Aviso::Registro(_)))
        .expect("registro");
    let i_fin = avisos
        .iter()
        .position(|a| matches!(a, Aviso::Fin { .. }))
        .unwrap();
    assert!(i_reg < i_fin);
    let Aviso::Registro(r) = &avisos[i_reg] else {
        unreachable!()
    };
    assert_eq!(r.pensamiento, "El usuario quiere una casa. Sera roja.");
    assert!(r.segundos.is_some());
    assert_eq!(r.plan.as_ref().unwrap().entiendo, "Dibujar una casa roja.");
    assert_eq!(r.hoja.objetivo, "Dibujar una casa roja.");
    assert_eq!(r.hoja.hechos, vec!["color rojo"]);
    assert_eq!(r.hoja.turnos.len(), 1);
    assert!(r.hoja.turnos[0].pensado.contains("casa"));
    assert!(!r.reinicio);
    // Planear pidio pensar; actuar no.
    let pedidas = motor.recibido.lock().unwrap();
    assert!(pedidas[0].pensar && !pedidas[1].pensar);
}

#[test]
fn sin_memoria_el_turno_es_el_de_siempre() {
    let motor = Arc::new(MotorGuionado::nuevo(vec![Respuesta::de_texto("Hola.")]));
    let historial = vec![pixpin_ia::Mensaje::usuario("hola")];
    let mut t = Turno::lanzar(motor.clone(), "p".into(), historial, vec![], None);
    let (_, avisos) = hasta_el_final(&mut t);
    assert!(!avisos.iter().any(|a| matches!(a, Aviso::Registro(_))));
    assert_eq!(
        motor.recibido.lock().unwrap().len(),
        1,
        "sin vuelta de planear"
    );
}

#[test]
fn un_turno_que_falla_deja_lo_que_falta_en_la_hoja() {
    // El guion se acaba tras planear: la vuelta de actuar falla.
    let motor = Arc::new(MotorGuionado::nuevo(vec![Respuesta::de_texto(PLAN)]));
    let memoria = Memoria {
        planificar: true,
        ..Default::default()
    };
    let mut t = Turno::lanzar_con_memoria(
        motor,
        "p".into(),
        vec![pixpin_ia::Mensaje::usuario("dibuja una casa roja")],
        vec![],
        None,
        None,
        vec![],
        Some(memoria),
        None,
    );
    let (vivo, avisos) = hasta_el_final(&mut t);
    assert!(vivo.acabado, "{}", vivo.mostrar());
    let r = avisos
        .iter()
        .find_map(|a| {
            if let Aviso::Registro(r) = a {
                Some(r.clone())
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(r.hoja.falta, vec!["usar dibujar_en_lienzo."]);
    assert!(r.hoja.hecho[0].ends_with("(sin acabar)"));
}

#[test]
fn preparar_el_turno_reinicia_al_70_y_apunta_desde_donde() {
    let largo = |i: i64| format!("mensaje {i}: {}", "palabra ".repeat(300));
    let notas: Vec<Nota> = (1..=20).map(|i| nota(i, i % 2 == 0, &largo(i))).collect();
    let mut hoja = HojaDeTrabajo {
        objetivo: "Planear la fiesta de Lucia".into(),
        ..Default::default()
    };
    let (h, reiniciado) =
        pensado::preparar_con_hoja(&mut hoja, &notas, "¿y la hora?", 8192, &catalogo_base());
    assert!(reiniciado);
    assert_eq!(h.len(), 4, "sistema + 2 + el nuevo");
    assert!(
        h[0].texto_entero()
            .contains("Objetivo: Planear la fiesta de Lucia")
    );
    assert_eq!(hoja.reinicio_desde.as_deref(), Some(notas[18].id.as_str()));
    assert_eq!(hoja.reinicios, 1);
    // El turno siguiente empieza desde ahi: con dos notas mas cabe sin
    // volver a reiniciar.
    let mut mas = notas.clone();
    mas.push(nota(21, false, "¿y la hora?"));
    mas.push(nota(22, true, "A las 17:00."));
    let (h, reiniciado) =
        pensado::preparar_con_hoja(&mut hoja, &mas, "gracias", 8192, &catalogo_base());
    assert!(!reiniciado);
    assert_eq!(h.len(), 6, "sistema + las 4 desde el reinicio + el nuevo");
    assert!(h[0].texto_entero().contains("buscar_historial"));
    assert_eq!(hoja.reinicios, 1);
    // Con una ventana grande no hace falta.
    let mut otra = HojaDeTrabajo::default();
    let (h, reiniciado) =
        pensado::preparar_con_hoja(&mut otra, &notas[..4], "x", 32768, &catalogo_base());
    assert!(!reiniciado && h.len() == 6 && otra.reinicio_desde.is_none());
}

#[test]
fn las_fuentes_de_buscar_historial_son_las_notas_y_lo_pensado() {
    let notas = vec![
        nota(1, false, "La fiesta es en el parque Kennedy"),
        nota(2, true, "Apuntado."),
    ];
    let mut hoja = HojaDeTrabajo::default();
    hoja.turnos.push(pixpin_ia::hoja_trabajo::TurnoResumido {
        n: 1,
        peticion: "Apuntar el sitio".into(),
        pensado: "Es en Miraflores.".into(),
        resultado: String::new(),
    });
    let f = pensado::fuentes(&notas, &hoja);
    assert_eq!(f.len(), 3);
    assert_eq!(f[0].etiqueta, "#1 usuario");
    assert_eq!(f[1].etiqueta, "#2 asistente");
    let r = pixpin_ia::contexto::buscar(&f, "parque", 2);
    assert!(r.contains("Kennedy"), "{r}");
    assert!(pixpin_ia::contexto::buscar(&f, "miraflores", 2).starts_with("[T1 pensado]"));
}

fn pensar_en_local_es_solo_si_se_pide() {
    assert!(!pensado::pensar_local(None));
    assert!(pensado::pensar_local(Some(true)));
    assert!(!pensado::pensar_local(Some(false)));
    assert_eq!(pensado::ventana_local(), 8192);
}

// ---------------------------------------------------------------------------
// De verdad

/// **Una conversacion larga con el 2B de verdad**, con la ventana forzada a
/// pequena (4096 en vez de 8192; `PIXPIN_IA_VENTANA`) para que se reinicie: al final se pregunta por
/// datos del primer mensaje, que ya no estan cargados, y la respuesta tiene
/// que traerlos (de la hoja de trabajo o con `buscar_historial`).
///
/// ```text
/// PIXPIN_IA_MOTOR=<carpeta con llama-server.exe>  PIXPIN_IA_MODELO=<Qwen3.5-2B-Q4_K_M.gguf>
/// PIXPIN_IA_DISPOSITIVO=cpu|Vulkan1  PIXPIN_IA_PENSAR=1 (por omision)
/// cargo test -p pixpin --bin pixpinmax de_verdad_conversacion_larga -- --ignored --nocapture --test-threads=1
/// ```
#[test]
#[ignore = "necesita el motor y el modelo bajados (ver la cabecera)"]
fn de_verdad_conversacion_larga_con_reinicios() {
    use pixpin_ia::servidor::{Config, Dispositivo, MotorLlama};
    let motor_dir =
        std::path::PathBuf::from(std::env::var("PIXPIN_IA_MOTOR").expect("PIXPIN_IA_MOTOR"));
    let modelo =
        std::path::PathBuf::from(std::env::var("PIXPIN_IA_MODELO").expect("PIXPIN_IA_MODELO"));
    let mut config = Config::nueva(motor_dir.join("llama-server.exe"), modelo, None);
    config.dispositivo = match std::env::var("PIXPIN_IA_DISPOSITIVO").as_deref() {
        Ok("cpu") | Err(_) => Dispositivo::Cpu,
        Ok(d) => Dispositivo::Gpu(d.into()),
    };
    let pensar = std::env::var("PIXPIN_IA_PENSAR").map_or(true, |v| v != "0");
    let motor: Arc<dyn pixpin_ia::Motor> = Arc::new(MotorLlama::nuevo(config));
    // 4096 por omision: el pedido sin pensar y sin herramientas es corto, y
    // con 6144 no llegaba al 70 % (medido, 14-oct). `PIXPIN_IA_VENTANA` la cambia.
    let ventana: u32 = std::env::var("PIXPIN_IA_VENTANA").ok().and_then(|v| v.parse().ok()).unwrap_or(4096);
    let pedidos = [
        "Vamos a organizar el cumpleaños de Lucía: es el sábado 18 a las 17:00 en el parque Kennedy y vienen 12 niños. Apúntalo, que luego te lo preguntaré.",
        "Dame cinco ideas de juegos para niños de 7 años, explicando cada uno en dos frases.",
        "Escribe una lista de compras para la merienda de los niños, con cantidades.",
        "Propón un horario de la tarde, de principio a fin, con los juegos de antes.",
        "Redacta un mensaje corto para invitar a los padres.",
        "Sin mirar arriba: ¿a qué hora, dónde y para cuántos niños es la fiesta?",
    ];
    let mut notas: Vec<Nota> = Vec::new();
    let mut hoja = HojaDeTrabajo::default();
    let mut reinicios = 0;
    let mut ultima = String::new();
    let t0 = Instant::now();
    for (i, pedido) in pedidos.iter().enumerate() {
        let n = notas.len() as i64;
        // Sin herramientas del chat (aqui no hay ventana que las haga): solo
        // `buscar_historial`, que es lo que se prueba.
        let herramientas: Vec<pixpin_ia::Herramienta> = Vec::new();
        let (historial, reiniciado) =
            pensado::preparar_con_hoja(&mut hoja, &notas, pedido, ventana, &herramientas);
        let memoria = Memoria {
            fuentes: pensado::fuentes(&notas, &hoja),
            hoja: hoja.clone(),
            pensar,
            planificar: true,
            ventana,
            reiniciado,
            ampliar: false,
            ..Default::default()
        };
        let mut t = Turno::lanzar_con_memoria(
            motor.clone(),
            "p".into(),
            historial,
            vec![],
            None,
            None,
            herramientas,
            Some(memoria),
            None,
        );
        let (vivo, avisos) = hasta_el_final_largo(&mut t);
        let registro: Registro = avisos
            .iter()
            .find_map(|a| {
                if let Aviso::Registro(r) = a {
                    Some(r.clone())
                } else {
                    None
                }
            })
            .expect("registro");
        reinicios += usize::from(registro.reinicio);
        println!(
            "\n=== T{} ({:.0} s){}\n> {pedido}\n· penso {:?} s: {}\n· peticion: {}\n< {}",
            i + 1,
            t0.elapsed().as_secs_f32(),
            if registro.reinicio {
                " [contexto renovado]"
            } else {
                ""
            },
            registro.segundos.map(|s| s.round()),
            pixpin_ia::pensar::resumir(&registro.pensamiento, 200),
            registro
                .plan
                .as_ref()
                .map(|p| p.lineas().replace('\n', " | "))
                .unwrap_or_default(),
            vivo.mostrar()
        );
        hoja = registro.hoja.clone();
        println!("--- hoja:\n{}", hoja.a_texto());
        notas.push(nota(n + 1, false, pedido));
        notas.push(nota(n + 2, true, &vivo.mostrar()));
        ultima = vivo.mostrar();
    }
    motor.soltar();
    let bajo = ultima.to_lowercase();
    let aciertos = [
        bajo.contains("17"),
        bajo.contains("kennedy"),
        bajo.contains("12") || bajo.contains("doce"),
    ];
    println!("\naciertos (hora, sitio, niños): {aciertos:?}; reinicios: {reinicios}");
    assert!(reinicios > 0, "con {ventana} tokens tenia que reiniciarse alguna vez");
    assert!(
        aciertos.iter().filter(|x| **x).count() >= 2,
        "perdio el objetivo tras reiniciar: {ultima}"
    );
}

fn hasta_el_final_largo(t: &mut Turno) -> (EnVivo, Vec<Aviso>) {
    let mut vivo = EnVivo::default();
    let mut todos = Vec::new();
    let t0 = Instant::now();
    let hasta = t0 + Duration::from_secs(900);
    while Instant::now() < hasta {
        for a in t.recibir() {
            match &a {
                Aviso::Evento(e) => {
                    match e {
                        Evento::Texto(_) | Evento::Pensando(_) => {}
                        otro => println!("  [{:>5.1} s] {otro:?}", t0.elapsed().as_secs_f32()),
                    }
                    vivo.aplicar(e)
                }
                Aviso::Fin { resultado } => vivo.acabar(resultado),
                _ => {}
            }
            todos.push(a);
        }
        if t.acabado() {
            return (vivo, todos);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    t.parar();
    panic!("el turno no acabo en 15 min: {}", vivo.mostrar());
}
