//! Pruebas de las automatizaciones y del asistente sin ventana: el formato,
//! las recetas rotas, los bucles, los disparadores con un reloj de mentira y
//! los pasos encadenados (con un hacedor de mentira y con el motor de
//! mentira de `pixpin-ia`, por el camino de verdad del encargo).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pixpin_ia::acciones::AccionApp;
use pixpin_ia::motor::{MotorGuionado, Respuesta};
use pixpin_proyecto::{almacen, cuaderno};
use serde_json::{Value, json};

use super::motor::{Disparo, Guardia, elegir, toca_a_la_hora};
use super::*;

fn raiz_de_prueba(nombre: &str) -> PathBuf {
    let raiz = std::env::temp_dir().join(format!("pixpin-auto-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    let obra = almacen::Ficha::nueva("Obra Miraflores", 1, "PC01");
    almacen::Indice {
        proyectos: vec![obra],
        ..Default::default()
    }
    .guardar(&raiz)
    .unwrap();
    raiz
}

fn id_de(raiz: &Path, nombre: &str) -> String {
    almacen::Indice::leer(raiz)
        .proyectos
        .into_iter()
        .find(|f| f.nombre == nombre)
        .unwrap()
        .id
}

const EJEMPLO: &str = r#"{"version":1,"deMas":"se guarda","automatizaciones":[
  {"id":"c1","nombre":"Capturas a la obra","activa":true,"aparato":"PC01",
   "disparador":{"tipo":"al_capturar"},
   "pasos":[{"tipo":"accion","herramienta":"escribir_en_chat","args":{"texto":"Captura {hora}"},"proyecto":"Obra","imagenes":["{captura}"]},
            {"tipo":"asistente","pedido":"Hazle OCR"}],
   "loDelMovil":42},
  {"id":"h1","nombre":"Resumen","activa":true,"aparato":"PC01",
   "disparador":{"tipo":"a_la_hora","hora":"08:00"},
   "pasos":[{"tipo":"asistente","pedido":"Resume mis tareas pendientes"}]},
  {"id":"s1","nombre":"Sincro","activa":false,"aparato":"PC01",
   "disparador":{"tipo":"al_sincronizar"},
   "pasos":[{"tipo":"accion","herramienta":"anadir_tarea","args":{"texto":"Mirar {sincro}"}}]},
  {"id":"m1","nombre":"A mano","aparato":"TEL9",
   "disparador":{"tipo":"a_mano"},
   "pasos":[{"tipo":"asistente","pedido":"Hola"}]},
  {"id":"x1","nombre":"Futura","disparador":{"tipo":"al_llegar_correo"},"pasos":[]}
]}"#;

#[test]
fn el_formato_va_y_vuelve_sin_perder_lo_que_no_entiende() {
    let l = Lista::de_json(EJEMPLO).unwrap();
    assert_eq!(l.entradas.len(), 5);
    assert_eq!(l.buenas().count(), 4);
    let c1 = l.buscar("c1").unwrap();
    assert_eq!(c1.disparador, Disparador::AlCapturar);
    assert_eq!(c1.resto.get("loDelMovil"), Some(&json!(42)));
    assert!(
        matches!(&l.entradas[4], Entrada::Rota { motivo, .. } if motivo == "«al_llegar_correo» es de una versión más nueva")
    );
    // Ida y vuelta: lo de mas y la rota siguen ahi, tal cual.
    let otra = Lista::de_json(&l.a_json()).unwrap();
    assert_eq!(otra, l);
    let v: Value = serde_json::from_str(&l.a_json()).unwrap();
    assert_eq!(v["deMas"], "se guarda");
    assert_eq!(v["version"], 1);
    assert_eq!(
        v["automatizaciones"][4]["disparador"]["tipo"],
        "al_llegar_correo"
    );
    assert_eq!(v["automatizaciones"][0]["pasos"][0]["tipo"], "accion");
    // Caso negativo: lo que no es JSON no se pisa.
    assert!(Lista::de_json("no es json").is_err());
    assert!(Lista::de_json(r#"{"automatizaciones":3}"#).is_err());
}

#[test]
fn una_receta_rota_o_con_un_paso_desconocido_no_entra() {
    let crear = |r: Value| de_receta(&r, "a".into(), "PC01", 1);
    let bien = crear(
        json!({"nombre":"Resumen","disparador":{"tipo":"a_la_hora","hora":"8:00"},
                            "pasos":"[\"Resume mis tareas pendientes\"]"}),
    )
    .unwrap();
    assert_eq!(bien.pasos.len(), 1);
    assert_eq!(bien.aparato.as_deref(), Some("PC01"));
    // Un texto suelto es un pedido al asistente.
    let suelto = crear(json!({"nombre":"x","disparador":"a_mano","pasos":"dime la hora"})).unwrap();
    assert!(matches!(&suelto.pasos[0], Paso::Asistente { pedido, .. } if pedido == "dime la hora"));
    // Objetos con y sin `tipo`.
    let mixto = crear(
        json!({"nombre":"x","disparador":{"tipo":"al_capturar"},"pasos":[
        {"herramienta":"escribir_en_chat","args":{"texto":"{captura}"},"proyecto":"Obra"},
        {"asistente":"hazle OCR","imagenes":["{captura}"]},
        {"tipo":"accion","herramienta":"anadir_tarea","args":{"texto":"{resultado}"}}]}),
    )
    .unwrap();
    assert_eq!(mixto.pasos.len(), 3);

    let mal = |r: Value| crear(r).unwrap_err();
    assert!(
        mal(
            json!({"nombre":"x","disparador":"a_mano","pasos":[{"herramienta":"volar","args":{}}]})
        )
        .contains("no hay ninguna acción «volar»")
    );
    assert!(
        mal(json!({"nombre":"x","disparador":"a_mano","pasos":[{"herramienta":"anadir_tarea"}]}))
            .contains("texto")
    );
    assert!(
        mal(json!({"nombre":"x","disparador":{"tipo":"a_la_hora","hora":"25:00"},"pasos":"hola"}))
            .contains("hora")
    );
    assert!(
        mal(json!({"nombre":"x","disparador":"cada_luna","pasos":"hola"})).contains("disparador")
    );
    assert!(mal(json!({"nombre":"x","disparador":"a_mano","pasos":[]})).contains("no tiene pasos"));
    assert!(mal(json!({"nombre":"","disparador":"a_mano","pasos":"hola"})).contains("nombre"));
    assert!(mal(json!({"nombre":"x","disparador":"a_mano","pasos":[3]})).contains("paso 1"));
    assert!(
        mal(json!({"nombre":"x","disparador":"a_mano","pasos":[{"herramienta":"capturar_pantalla","imagenes":["{captura}"]}]}))
            .contains("no lleva imágenes")
    );
    let muchos: Vec<Value> = (0..11).map(|i| json!(format!("paso {i}"))).collect();
    assert!(mal(json!({"nombre":"x","disparador":"a_mano","pasos":muchos})).contains("como mucho"));
    // Lo mismo antes de mandar el pedido (para que el modelo lo arregle).
    let pedido = json!({"pixpin":1,"accion":"automatizacion","operacion":"crear",
                        "receta":{"nombre":"x","disparador":{"tipo":"a_mano"},"pasos":"[{\"herramienta\":\"volar\"}]"}});
    assert!(revisar_pedido(&pedido).unwrap_err().contains("volar"));
    assert!(revisar_pedido(&json!({"pixpin":1,"accion":"capturar"})).is_ok());
}

#[test]
fn un_bucle_de_disparadores_se_rechaza_al_crearla_y_se_bloquea_al_correr() {
    let crear = |d: &str, h: &str| {
        de_receta(
            &json!({"nombre":"x","disparador":{"tipo":d},"pasos":[{"herramienta":h}]}),
            "a".into(),
            "PC01",
            1,
        )
    };
    assert!(
        crear("al_capturar", "capturar_pantalla")
            .unwrap_err()
            .contains("bucle")
    );
    assert!(
        crear("al_sincronizar", "sincronizar_movil")
            .unwrap_err()
            .contains("bucle")
    );
    // Capturar a mano o sincronizar cada dia si vale.
    assert!(crear("a_mano", "capturar_pantalla").is_ok());
    assert!(crear("a_mano", "sincronizar_movil").is_ok());
    // Ninguna crea automatizaciones (una que crea otra que crea otra…).
    let anidada = de_receta(
        &json!({"nombre":"x","disparador":"a_mano","pasos":[{"herramienta":"crear_automatizacion",
            "args":{"nombre":"y","disparador":"a_mano","pasos":"hola"}}]}),
        "a".into(),
        "PC01",
        1,
    );
    assert!(anidada.unwrap_err().contains("no puede crear"));

    // Al correr, tambien dentro de un paso del asistente: el pedido no sale.
    let salidos = Arc::new(Mutex::new(Vec::<String>::new()));
    let s = salidos.clone();
    let dentro: crate::asistente::acciones::Mandar = Box::new(move |j| {
        s.lock().unwrap().push(j.to_string());
        1
    });
    let mandar = encargo::envolver(dentro, Some("pr-obra".into()), Some(Tipo::AlCapturar));
    assert_eq!(
        mandar(r#"{"pixpin":1,"accion":"capturar","modo":"zona"}"#),
        3
    );
    assert_eq!(
        mandar(r#"{"pixpin":1,"accion":"automatizacion","operacion":"borrar","id":"a"}"#),
        3
    );
    assert!(
        salidos.lock().unwrap().is_empty(),
        "salio algo que hacia bucle"
    );
    // Lo demas sale, con el chat del encargo si no dice otro.
    assert_eq!(
        mandar(r#"{"pixpin":1,"accion":"anadir_tarea","texto":"pan"}"#),
        1
    );
    assert_eq!(
        mandar(r#"{"pixpin":1,"accion":"anadir_tarea","texto":"pan","codigo":"m1"}"#),
        1
    );
    assert_eq!(
        mandar(r#"{"pixpin":1,"accion":"sincronizar","aparato":"todos"}"#),
        1
    );
    let v: Vec<Value> = salidos
        .lock()
        .unwrap()
        .iter()
        .map(|j| serde_json::from_str(j).unwrap())
        .collect();
    assert_eq!(v[0]["proyecto"], "pr-obra");
    assert!(
        v[1].get("proyecto").is_none(),
        "una lista concreta no se cambia"
    );
    assert!(
        v[2].get("proyecto").is_none(),
        "sincronizar no va a un chat"
    );
}

/// 13-oct-2026 (un martes), a las `h:m` hora local, en ms.
fn a_las(dia: i64, h: i64, m: i64) -> i64 {
    // 2026-10-13 son 20 739 dias desde 1970.
    (20_739 + dia) * 86_400_000 + (h * 60 + m) * 60_000
}

#[test]
fn a_la_hora_con_un_reloj_de_mentira() {
    assert_eq!(
        dia_de_la_semana(a_las(0, 12, 0)),
        2,
        "el 13-oct-2026 es martes"
    );
    assert_eq!(Vars::default().con_hora(a_las(0, 8, 5)).fecha, "2026-10-13");
    assert_eq!(Vars::default().con_hora(a_las(0, 8, 5)).hora, "08:05");
    let ocho = Disparador::ALaHora {
        hora: "08:00".into(),
        dias: vec![],
    };
    assert!(!toca_a_la_hora(&ocho, a_las(0, 7, 59), None));
    assert!(toca_a_la_hora(&ocho, a_las(0, 8, 0), None));
    // Ya corrio hoy: no otra vez; manana si.
    assert!(!toca_a_la_hora(
        &ocho,
        a_las(0, 8, 30),
        Some(a_las(0, 8, 0))
    ));
    assert!(toca_a_la_hora(&ocho, a_las(1, 8, 0), Some(a_las(0, 8, 0))));
    // Con el PC apagado a las 8: a las 9 se recupera; a las 15, ya no.
    assert!(toca_a_la_hora(&ocho, a_las(0, 9, 0), Some(a_las(-1, 8, 0))));
    assert!(!toca_a_la_hora(
        &ocho,
        a_las(0, 15, 0),
        Some(a_las(-1, 8, 0))
    ));
    // Solo los lunes: el martes no.
    let lunes = Disparador::ALaHora {
        hora: "08:00".into(),
        dias: vec![1],
    };
    assert!(!toca_a_la_hora(&lunes, a_las(0, 8, 0), None));
    assert!(toca_a_la_hora(&lunes, a_las(6, 8, 0), None));
    // Caso negativo: otro disparador u hora rota, nunca.
    assert!(!toca_a_la_hora(&Disparador::AMano, a_las(0, 8, 0), None));
    let rota = Disparador::ALaHora {
        hora: "ocho".into(),
        dias: vec![],
    };
    assert!(!toca_a_la_hora(&rota, a_las(0, 8, 0), None));
    assert_eq!(minuto_del_dia("8:05"), Some(485));
    assert_eq!(minuto_del_dia("24:00"), None);
}

#[test]
fn cada_disparo_elige_las_suyas() {
    let l = Lista::de_json(EJEMPLO).unwrap();
    let ids = |d: &Disparo,
               ahora: i64,
               ultimas: &std::collections::BTreeMap<String, i64>|
     -> Vec<String> {
        elegir(&l, d, "PC01", ahora, ultimas)
            .into_iter()
            .map(|(a, _)| a.id)
            .collect()
    };
    let nada = Default::default();
    let captura = Disparo::Captura(PathBuf::from(r"C:\c\captura-1.png"));
    assert_eq!(ids(&captura, a_las(0, 12, 0), &nada), ["c1"]);
    let (_, vars) = elegir(&l, &captura, "PC01", a_las(0, 12, 0), &nada).remove(0);
    assert_eq!(
        vars.captura.as_deref(),
        Some(Path::new(r"C:\c\captura-1.png"))
    );
    assert_eq!(vars.hora, "12:00");
    // La de la sincro esta apagada.
    assert!(ids(&Disparo::Sincro("Al día".into()), a_las(0, 12, 0), &nada).is_empty());
    assert_eq!(ids(&Disparo::Reloj, a_las(0, 8, 1), &nada), ["h1"]);
    assert!(ids(&Disparo::Reloj, a_las(0, 7, 0), &nada).is_empty());
    let hecha = [("h1".to_string(), a_las(0, 8, 1))].into_iter().collect();
    assert!(ids(&Disparo::Reloj, a_las(0, 8, 2), &hecha).is_empty());
    // A mano: la pedida, aunque este apagada o sea de otro aparato.
    assert_eq!(ids(&Disparo::AMano("s1".into()), 0, &nada), ["s1"]);
    assert_eq!(ids(&Disparo::AMano("m1".into()), 0, &nada), ["m1"]);
    assert!(ids(&Disparo::AMano("no-esta".into()), 0, &nada).is_empty());
    // Caso negativo: desde otro aparato, la de captura de PC01 no corre.
    assert!(elegir(&l, &captura, "TEL9", 0, &nada).is_empty());
}

#[test]
fn la_guardia_corta_un_bucle_que_no_se_vio() {
    let mut g = Guardia::default();
    assert!(g.puede("a", 0).is_ok());
    assert!(g.puede("a", 1_000).is_err(), "dos veces en un segundo");
    assert!(g.puede("b", 1_000).is_ok(), "otra si puede");
    let mut t = 10_000;
    for _ in 1..motor::POR_HORA {
        assert!(g.puede("a", t).is_ok());
        t += 10_000;
    }
    assert!(g.puede("a", t).unwrap_err().contains("veces"));
    // Pasada la hora, vuelve a poder.
    assert!(g.puede("a", 3_700_000 + t).is_ok());
}

/// Un hacedor de mentira: apunta lo que se le pide.
#[derive(Default)]
struct Espia {
    acciones: Vec<Value>,
    preguntas: Vec<(String, Option<String>, Vec<PathBuf>, Tipo)>,
    respuesta: Option<Result<String, String>>,
}

impl Hacedor for Espia {
    fn accion(&mut self, a: &AccionApp) -> Result<String, String> {
        self.acciones.push(a.pedido.clone());
        Ok(format!("hecho: {}", a.herramienta))
    }
    fn asistente(
        &mut self,
        pedido: &str,
        proyecto: Option<&str>,
        imagenes: &[PathBuf],
        bloquear: Tipo,
    ) -> Result<String, String> {
        self.preguntas.push((
            pedido.to_string(),
            proyecto.map(str::to_string),
            imagenes.to_vec(),
            bloquear,
        ));
        self.respuesta
            .clone()
            .unwrap_or_else(|| Ok("Texto leído: CIMIENTOS".into()))
    }
    fn proyecto(&self, nombre: &str) -> Option<String> {
        (acciones_normalizar(nombre) == "obra").then(|| "pr-obra".into())
    }
}

fn acciones_normalizar(s: &str) -> String {
    pixpin_ia::acciones::normalizar(s.trim())
}

#[test]
fn los_pasos_se_encadenan_y_el_que_falla_para_los_demas() {
    let a = de_receta(
        &json!({"nombre":"Capturas a la obra","disparador":"al_capturar","pasos":[
            {"herramienta":"escribir_en_chat","args":{"texto":"Captura de las {hora}"},"proyecto":"Obra","imagenes":["{captura}"]},
            {"asistente":"Hazle OCR"},
            {"herramienta":"escribir_en_chat","args":{"texto":"{resultado}"},"proyecto":"Obra"}]}),
        "a".into(),
        "PC01",
        1,
    )
    .unwrap();
    let vars = Vars {
        captura: Some(PathBuf::from(r"C:\c\captura-7.png")),
        ..Vars::default().con_hora(a_las(0, 9, 30))
    };
    let mut h = Espia::default();
    let inf = ejecutar(&a, vars.clone(), &mut h);
    assert!(inf.bien(), "{inf:?}");
    assert_eq!(h.acciones.len(), 2);
    assert_eq!(h.acciones[0]["accion"], "chat");
    assert_eq!(h.acciones[0]["texto"], "Captura de las 09:30");
    assert_eq!(h.acciones[0]["proyecto"], "pr-obra");
    assert_eq!(h.acciones[0]["imagenes"], json!([r"C:\c\captura-7.png"]));
    // Al capturar, la captura va al asistente aunque el paso no la nombre.
    assert_eq!(h.preguntas[0].0, "Hazle OCR");
    assert_eq!(h.preguntas[0].2, [PathBuf::from(r"C:\c\captura-7.png")]);
    assert_eq!(h.preguntas[0].3, Tipo::AlCapturar);
    // Lo que contesto el asistente es el texto del paso siguiente.
    assert_eq!(h.acciones[1]["texto"], "Texto leído: CIMIENTOS");
    assert_eq!(inf.resumen(3), "hecho: escribir_en_chat");

    // Caso negativo: si el asistente falla, el tercer paso no se hace.
    let mut h = Espia {
        respuesta: Some(Err("sin modelo".into())),
        ..Default::default()
    };
    let inf = ejecutar(&a, vars.clone(), &mut h);
    assert!(!inf.bien());
    assert_eq!(inf.pasos.len(), 2);
    assert_eq!(h.acciones.len(), 1);
    assert!(
        inf.resumen(3).contains("Paso 2 de 3: sin modelo"),
        "{}",
        inf.resumen(3)
    );
    // Y un chat que no existe para en ese paso.
    let mut otra = a.clone();
    if let Paso::Accion { proyecto, .. } = &mut otra.pasos[0] {
        *proyecto = Some("Casa".into());
    }
    let mut h = Espia::default();
    let inf = ejecutar(&otra, vars, &mut h);
    assert_eq!(inf.pasos.len(), 1);
    assert!(inf.pasos[0].as_ref().unwrap_err().contains("«Casa»"));
    assert!(h.acciones.is_empty());
}

#[test]
fn guardar_crea_el_fichero_y_un_solo_mensaje_que_lo_hace_viajar() {
    let raiz = raiz_de_prueba("guardar");
    let a = de_receta(
        &json!({"nombre":"Resumen","disparador":"a_mano","pasos":"hola"}),
        "auto-1".into(),
        "PC01",
        5,
    )
    .unwrap();
    cambiar(&raiz, "PC01", 5, |l| l.anadir(a.clone())).unwrap();
    // Repetir el id no entra.
    assert!(
        cambiar(&raiz, "PC01", 6, |l| l.anadir(a.clone()))
            .unwrap_err()
            .contains("ya hay")
    );
    let b = de_receta(
        &json!({"nombre":"Otra","disparador":"a_mano","pasos":"adios"}),
        "auto-2".into(),
        "PC01",
        7,
    )
    .unwrap();
    cambiar(&raiz, "PC01", 7, |l| l.anadir(b)).unwrap();
    let (asistente, carpeta) = chat_del_asistente(&raiz, "PC01", 8).unwrap();
    assert!(fichero_en(&carpeta).ends_with(r"android\guardados\asistente\automatizaciones.json"));
    let l = leer(&raiz, "PC01").unwrap();
    assert_eq!(
        l.buenas().map(|a| a.id.as_str()).collect::<Vec<_>>(),
        ["auto-1", "auto-2"]
    );
    let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
    let senal: Vec<_> = c.mensajes.iter().filter(|m| m.id == ID_MENSAJE).collect();
    assert_eq!(senal.len(), 1, "un solo mensaje aunque se guarde dos veces");
    assert_eq!(senal[0].ruta.as_deref(), Some(RUTA_PORTATIL));
    assert_eq!(senal[0].clase, Some(cuaderno::Clase::Archivo));
    // La ruta portatil cae en el fichero de verdad (asi viaja).
    let real = pixpin_proyecto::vista::ruta_real(&raiz, &asistente, RUTA_PORTATIL).unwrap();
    assert_eq!(real, fichero_en(&carpeta));
    // Apagar y borrar.
    cambiar(&raiz, "PC01", 9, |l| Ok(l.activar("auto-1", false, 9))).unwrap();
    assert!(
        !leer(&raiz, "PC01")
            .unwrap()
            .buscar("auto-1")
            .unwrap()
            .activa
    );
    assert!(cambiar(&raiz, "PC01", 10, |l| Ok(l.borrar("auto-1"))).unwrap());
    assert!(!cambiar(&raiz, "PC01", 11, |l| Ok(l.borrar("auto-1"))).unwrap());
    assert_eq!(leer(&raiz, "PC01").unwrap().buenas().count(), 1);
    // Caso negativo: un fichero roto no se pisa.
    std::fs::write(fichero_en(&carpeta), "{roto").unwrap();
    assert!(cambiar(&raiz, "PC01", 12, |l| Ok(l.borrar("auto-2"))).is_err());
    assert_eq!(
        std::fs::read_to_string(fichero_en(&carpeta)).unwrap(),
        "{roto"
    );
    let _ = std::fs::remove_dir_all(&raiz);
}

fn contexto_de(
    ubicacion: &pixpin_store::Ubicacion,
    textos: &pixpin_store::Catalogo,
) -> crate::pedidos::Contexto<'static> {
    // Las pruebas viven lo que el proceso: se sueltan a proposito.
    let u: &'static pixpin_store::Ubicacion = Box::leak(Box::new(ubicacion.clone()));
    let t: &'static pixpin_store::Catalogo = Box::leak(Box::new(pixpin_store::Catalogo::nuevo(
        pixpin_store::Idioma::Espanol,
    )));
    let _ = textos;
    crate::pedidos::Contexto {
        idioma: pixpin_store::Idioma::Espanol,
        ubicacion: u,
        opciones: crate::ventana_chat::OpcionesLienzo {
            enganche: Default::default(),
            nivel: pixpin_nivel::Nivel::Completo,
            medir_fotogramas: false,
        },
        aparato: "PC01",
        textos: t,
    }
}

#[test]
fn el_pedido_automatizacion_crea_apaga_y_borra() {
    let raiz = raiz_de_prueba("pedido");
    let u = pixpin_store::Ubicacion::Instalado { raiz: raiz.clone() };
    let t = pixpin_store::Catalogo::nuevo(pixpin_store::Idioma::Espanol);
    let cx = contexto_de(&u, &t);
    // Lo que arma la fila del asistente, tal cual.
    let fila = pixpin_ia::acciones::por_nombre("crear_automatizacion").unwrap();
    let args: serde_json::Map<String, Value> = serde_json::from_value(json!({
        "nombre": "Resumen de pendientes", "disparador": "a_la_hora", "hora": "08:00",
        "pasos": "[\"Resume mis tareas pendientes\"]"}))
    .unwrap();
    let hecha = pixpin_ia::acciones::validar(fila, &args).unwrap();
    assert!(
        hecha
            .confirmar
            .as_deref()
            .unwrap()
            .contains("«Resumen de pendientes»")
    );
    let h = crate::pedidos::atender(&hecha.pedido.to_string(), &cx);
    assert_eq!(
        h.aviso.as_deref(),
        Some("Automatización «Resumen de pendientes» creada.")
    );
    let l = leer(&raiz, "PC01").unwrap();
    let a = l.buenas().next().unwrap().clone();
    assert_eq!(
        a.disparador,
        Disparador::ALaHora {
            hora: "08:00".into(),
            dias: vec![]
        }
    );
    let pedir = |op: &str| {
        crate::pedidos::atender(
            &json!({"pixpin":1,"accion":"automatizacion","operacion":op,"id":a.id}).to_string(),
            &cx,
        )
        .aviso
        .unwrap()
    };
    assert!(pedir("desactivar").contains("apagada"));
    assert!(!leer(&raiz, "PC01").unwrap().buscar(&a.id).unwrap().activa);
    assert!(pedir("activar").contains("encendida"));
    assert!(pedir("borrar").contains("borrada"));
    assert!(
        pedir("borrar").contains("No hay ninguna"),
        "la segunda vez ya no esta"
    );
    // Caso negativo: una receta rota no se guarda y se dice por que.
    let rota = json!({"pixpin":1,"accion":"automatizacion","operacion":"crear",
                      "receta":{"nombre":"x","disparador":{"tipo":"al_capturar"},"pasos":"[{\"herramienta\":\"capturar_pantalla\"}]"}});
    let h = crate::pedidos::atender(&rota.to_string(), &cx);
    assert!(h.aviso.unwrap().contains("bucle"));
    assert_eq!(leer(&raiz, "PC01").unwrap().entradas.len(), 0);
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn el_pedido_asistente_se_lee_y_lo_que_falta_se_dice() {
    use crate::pedidos::{Pedido, leer as leer_pedido};
    let p = leer_pedido(r#"{"pixpin":1,"accion":"asistente","texto":"hola [img 01]","imagenes":["C:\\a\\x.png"],"proyecto":"pr-1"}"#).unwrap();
    assert_eq!(
        p,
        Pedido::Asistente {
            texto: "hola [img 01]".into(),
            proyecto: Some("pr-1".into()),
            imagenes: vec![PathBuf::from(r"C:\a\x.png")],
            automatizacion: None,
            encargo: None,
        }
    );
    assert!(
        leer_pedido(r#"{"pixpin":1,"accion":"asistente"}"#).is_ok(),
        "sin nada: abrir el chat"
    );
    assert!(leer_pedido(r#"{"pixpin":1,"accion":"automatizacion","operacion":"volar"}"#).is_err());
    let raiz = raiz_de_prueba("pedido-asistente");
    let u = pixpin_store::Ubicacion::Instalado { raiz: raiz.clone() };
    let t = pixpin_store::Catalogo::nuevo(pixpin_store::Idioma::Espanol);
    let cx = contexto_de(&u, &t);
    // Un chat que no existe o una imagen que no esta: no se lanza nada.
    let h = crate::pedidos::atender(
        r#"{"pixpin":1,"accion":"asistente","texto":"hola","proyecto":"pr-no"}"#,
        &cx,
    );
    assert!(h.aviso.unwrap().contains("ya no está"));
    let h = crate::pedidos::atender(
        r#"{"pixpin":1,"accion":"asistente","texto":"hola","imagenes":["C:\\no\\esta.png"]}"#,
        &cx,
    );
    assert!(h.aviso.is_some());
    let h = crate::pedidos::atender(
        r#"{"pixpin":1,"accion":"asistente","automatizacion":"no-esta"}"#,
        &cx,
    );
    assert!(h.aviso.unwrap().contains("No hay ninguna"));
    let _ = std::fs::remove_dir_all(&raiz);
}

/// **Los pasos encadenados con el motor de mentira**, por el camino de
/// verdad del encargo: la pregunta y la respuesta quedan en el chat
/// «Asistente», el modelo apunta una tarea en el chat del encargo y lo que
/// contesto pasa al paso siguiente.
#[test]
fn los_pasos_encadenados_con_el_motor_de_mentira() {
    let raiz = raiz_de_prueba("motor");
    let obra = id_de(&raiz, "Obra Miraflores");
    let ent = encargo::Entorno {
        ubicacion: pixpin_store::Ubicacion::Instalado { raiz: raiz.clone() },
        aparato: "PC01".into(),
    };
    struct ConMotor {
        ent: encargo::Entorno,
        motor: Arc<MotorGuionado>,
        salidos: Arc<Mutex<Vec<String>>>,
        acciones: Vec<Value>,
    }
    impl Hacedor for ConMotor {
        fn accion(&mut self, a: &AccionApp) -> Result<String, String> {
            self.acciones.push(a.pedido.clone());
            Ok("hecho".into())
        }
        fn asistente(
            &mut self,
            pedido: &str,
            proyecto: Option<&str>,
            imagenes: &[PathBuf],
            bloquear: Tipo,
        ) -> Result<String, String> {
            let s = self.salidos.clone();
            let mandar: crate::asistente::acciones::Mandar = Box::new(move |j| {
                s.lock().unwrap().push(j.to_string());
                1
            });
            let r = encargo::hacer(
                &self.ent,
                self.motor.clone(),
                &encargo::Encargo {
                    texto: pedido.into(),
                    proyecto: proyecto.map(str::to_string),
                    imagenes: imagenes.to_vec(),
                    origen: Some("Resumen".into()),
                    bloquear: Some(bloquear),
                },
                encargo::Permiso::Fijo(Box::new(|_| false)),
                mandar,
            )?;
            if r.bien { Ok(r.texto) } else { Err(r.texto) }
        }
        fn proyecto(&self, nombre: &str) -> Option<String> {
            motor::proyecto_por_nombre(self.ent.ubicacion.raiz(), nombre)
        }
    }
    let motor = Arc::new(MotorGuionado::nuevo(vec![
        Respuesta::de_llamada("anadir_tarea", r#"{"texto":"Revisar el encofrado"}"#),
        Respuesta::de_texto("Apuntada. Tienes 2 tareas pendientes."),
    ]));
    let a = de_receta(
        &json!({"nombre":"Resumen","disparador":{"tipo":"a_la_hora","hora":"08:00"},"pasos":[
            {"asistente":"Apunta revisar el encofrado y dime cuantas tareas tengo","proyecto":"obra miraflores"},
            {"herramienta":"escribir_en_chat","args":{"texto":"Resumen {fecha}: {resultado}"},"proyecto":"Obra"}]}),
        "a".into(),
        "PC01",
        1,
    )
    .unwrap();
    let salidos = Arc::new(Mutex::new(Vec::new()));
    let mut h = ConMotor {
        ent: ent.clone(),
        motor: motor.clone(),
        salidos: salidos.clone(),
        acciones: Vec::new(),
    };
    let inf = ejecutar(&a, Vars::default().con_hora(a_las(0, 8, 0)), &mut h);
    assert!(inf.bien(), "{inf:?}");
    // El modelo apunto la tarea en el chat del encargo (la obra).
    let salidos = salidos.lock().unwrap().clone();
    assert_eq!(salidos.len(), 1, "{salidos:?}");
    let tarea: Value = serde_json::from_str(&salidos[0]).unwrap();
    assert_eq!(tarea["accion"], "anadir_tarea");
    assert_eq!(tarea["proyecto"], json!(obra));
    // Lo que contesto paso al paso siguiente.
    assert_eq!(
        h.acciones[0]["texto"],
        "Resumen 2026-10-13: Apuntada. Tienes 2 tareas pendientes."
    );
    assert_eq!(h.acciones[0]["proyecto"], json!(obra));
    // Y la conversacion quedo en el chat «Asistente».
    let (_, carpeta) = chat_del_asistente(&raiz, "PC01", 1).unwrap();
    let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
    let pregunta = c
        .mensajes
        .iter()
        .find(|m| m.texto.starts_with("⚙️ Resumen:"))
        .expect("la pregunta");
    assert!(
        pregunta.texto.ends_with("@Obra Miraflores"),
        "{}",
        pregunta.texto
    );
    let respuesta = c
        .mensajes
        .iter()
        .find(|m| m.aparato.as_deref() == Some(crate::asistente::chat::APARATO))
        .expect("la respuesta");
    assert_eq!(respuesta.texto, "Apuntada. Tienes 2 tareas pendientes.");
    // El modelo supo sobre que chat se le pedia.
    let pedido_al_modelo = motor.recibido.lock().unwrap()[0]
        .mensajes
        .last()
        .unwrap()
        .texto_entero();
    assert!(
        pedido_al_modelo.contains("«Obra Miraflores»"),
        "{pedido_al_modelo}"
    );
    let _ = std::fs::remove_dir_all(&raiz);
}

/// Caso negativo: el modelo, dentro de una automatizacion «al capturar»,
/// intenta capturar: no sale, el modelo se entera y el paso falla.
#[test]
fn dentro_de_un_paso_el_asistente_tampoco_puede_hacer_bucle() {
    let raiz = raiz_de_prueba("motor-bucle");
    let ent = encargo::Entorno {
        ubicacion: pixpin_store::Ubicacion::Instalado { raiz: raiz.clone() },
        aparato: "PC01".into(),
    };
    let motor = Arc::new(MotorGuionado::nuevo(vec![
        Respuesta::de_llamada("capturar_pantalla", "{}"),
        Respuesta::de_texto("No pude capturar."),
    ]));
    let salidos = Arc::new(Mutex::new(Vec::<String>::new()));
    let s = salidos.clone();
    let r = encargo::hacer(
        &ent,
        motor,
        &encargo::Encargo {
            texto: "captura la pantalla otra vez".into(),
            bloquear: Some(Tipo::AlCapturar),
            origen: Some("Capturas".into()),
            ..Default::default()
        },
        encargo::Permiso::Fijo(Box::new(|_| false)),
        Box::new(move |j| {
            s.lock().unwrap().push(j.to_string());
            1
        }),
    )
    .unwrap();
    assert!(salidos.lock().unwrap().is_empty(), "salio la captura");
    assert!(!r.bien, "{r:?}");
    assert!(r.texto.contains("No pude capturar."), "{}", r.texto);
    let _ = std::fs::remove_dir_all(&raiz);
}

/// **Flow y las automatizaciones preguntan en la burbuja** (13-oct-2026, ya
/// no con un cuadro de Windows): la respuesta del asistente espera con la
/// pregunta apuntada en `asistente::permiso`, la tarjeta contesta, y con
/// «Sí» el pedido sale y la nota lo cuenta. Con «No», nada sale.
#[test]
fn el_encargo_sin_ventana_pregunta_en_su_burbuja() {
    for (si, nombre) in [(true, "permiso-si"), (false, "permiso-no")] {
        let raiz = raiz_de_prueba(nombre);
        let ent = encargo::Entorno {
            ubicacion: pixpin_store::Ubicacion::Instalado { raiz: raiz.clone() },
            aparato: "PC01".into(),
        };
        let motor = Arc::new(MotorGuionado::nuevo(vec![
            Respuesta::de_llamada("borrar_lista", r#"{"lista":"m-flow"}"#),
            Respuesta::de_texto("Listo."),
        ]));
        let salidos = Arc::new(Mutex::new(Vec::<String>::new()));
        let s = salidos.clone();
        // El chat «Asistente» antes del hilo (si no, los dos lo crearian a la vez).
        let (proyecto, carpeta) = chat_del_asistente(&raiz, "PC01", 1).unwrap();
        // La lista que se pide borrar, por su nombre (14-oct).
        let lista = crate::pedidos::nueva_lista(&raiz, &proyecto, "PC01", "m-flow")
            .unwrap()
            .id;
        let hilo = std::thread::spawn({
            let ent = ent.clone();
            move || {
                encargo::hacer(
                    &ent,
                    motor,
                    &encargo::Encargo {
                        texto: "borra la lista m-flow".into(),
                        ..Default::default()
                    },
                    encargo::Permiso::EnLaBurbuja,
                    Box::new(move |j| {
                        s.lock().unwrap().push(j.to_string());
                        1
                    }),
                )
            }
        });
        // Se busca la burbuja que pregunta, como lo haria la ventana.
        let hasta = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let nota = loop {
            if hilo.is_finished() {
                panic!("acabo sin preguntar: {:?}", hilo.join().unwrap());
            }
            let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap_or_default();
            assert!(
                std::time::Instant::now() < hasta,
                "no pregunto: {:?}",
                c.mensajes
                    .iter()
                    .map(|m| (m.id.clone(), m.texto.clone()))
                    .collect::<Vec<_>>()
            );
            if let Some(m) = c
                .mensajes
                .iter()
                .find(|m| crate::asistente::permiso::pregunta(&proyecto, &m.id).is_some())
                // Y la burbuja ya dice que espera (la nota se escribe antes).
                .filter(|m| m.texto.contains("esperando tu respuesta"))
            {
                break m.id.clone();
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        assert!(
            salidos.lock().unwrap().is_empty(),
            "salio antes de contestar"
        );
        assert!(crate::asistente::permiso::contestar(&proyecto, &nota, si));
        let r = hilo.join().unwrap().unwrap();
        let sale = salidos.lock().unwrap().iter().any(|j| j.contains(&lista));
        assert_eq!(sale, si, "{nombre}");
        let marca = if si { "→ ✅ Sí" } else { "→ ✖️ No" };
        assert!(r.texto.contains(marca), "{}", r.texto);
        let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
        assert!(
            c.mensajes
                .iter()
                .any(|m| m.id == nota && m.texto.contains(marca))
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }
}

/// **El pedido `asistente` (Flow, automatizaciones) va por el camino
/// rapido, como el chat**: «¿que tengo pendiente?» se contesta sin tocar el
/// motor y al momento; algo que no es simple si va al modelo.
#[test]
fn el_encargo_con_una_orden_rapida_no_toca_el_modelo() {
    let raiz = raiz_de_prueba("encargo-rapido");
    crate::tareas::apuntar(&raiz, "PC01", "comprar pan").unwrap();
    let ent = encargo::Entorno {
        ubicacion: pixpin_store::Ubicacion::Instalado { raiz: raiz.clone() },
        aparato: "PC01".into(),
    };
    let pedir = |texto: &str, motor: Arc<MotorGuionado>| {
        let t0 = std::time::Instant::now();
        let r = encargo::hacer(
            &ent,
            motor,
            &encargo::Encargo {
                texto: texto.into(),
                ..Default::default()
            },
            encargo::Permiso::Fijo(Box::new(|_| false)),
            Box::new(|_| 1),
        )
        .unwrap();
        (r, t0.elapsed())
    };
    // Sin guion: si se le pidiera algo al motor, fallaria.
    let vacio = Arc::new(MotorGuionado::nuevo(vec![]));
    let (r, t) = pedir("¿Qué tengo pendiente?", vacio.clone());
    println!("encargo rapido: {t:?}");
    assert!(r.bien && r.texto.contains("comprar pan"), "{}", r.texto);
    assert!(
        vacio.recibido.lock().unwrap().is_empty(),
        "no se toca el modelo"
    );
    assert!(t < std::time::Duration::from_millis(500), "{t:?}");
    // Y sin ningun motor (sin bajar, sin clave) la orden rapida se hace igual.
    let sin = crate::asistente::rapido::motor_o_sin(
        Err("falta el modelo".into()),
        "¿Qué tengo pendiente?",
    );
    assert!(sin.is_ok());
    assert!(
        crate::asistente::rapido::motor_o_sin(Err("falta el modelo".into()), "dibuja una casa")
            .is_err()
    );
    // Negativo: lo que no es simple si va al modelo.
    let motor = Arc::new(MotorGuionado::nuevo(vec![Respuesta::de_texto(
        "Una casa con tejado.",
    )]));
    let (r, _) = pedir("Describe una casa bonita", motor.clone());
    assert_eq!(motor.recibido.lock().unwrap().len(), 1);
    assert!(r.texto.contains("Una casa"), "{}", r.texto);
    let _ = std::fs::remove_dir_all(&raiz);
}

// ---------------------------------------------------------------------------
// «Calca esto» con una lamina, por el camino de la cola (15-oct)

/// Un modelo con vision que no contesta (sin red): la lamina pasa igual
/// como hoja, con lo que no necesita modelo.
struct VisionSinRed;

impl pixpin_ia::motor::Motor for VisionSinRed {
    fn completar(
        &self,
        _: &pixpin_ia::motor::Peticion,
        _: &mut dyn FnMut(&pixpin_ia::motor::Trozo) -> pixpin_ia::motor::Seguir,
    ) -> Result<pixpin_ia::motor::Respuesta, pixpin_ia::ErrorIa> {
        Err(pixpin_ia::ErrorIa::Nube("sin red en la prueba".into()))
    }
    fn ve_imagenes(&self) -> bool {
        true
    }
    fn a_la_vez(&self) -> usize {
        8
    }
}

/// La lamina sintetica del teorema de Tales, en un PNG de la prueba.
fn png_de_lamina(raiz: &Path) -> PathBuf {
    let p = crate::asistente::hoja_a_lienzo::banco_diagramas::lamina_tales();
    let ruta = raiz.join("lamina-prueba.png");
    std::fs::create_dir_all(raiz).unwrap();
    let img = pixpin_codec::ImagenRgba {
        ancho: p.ancho,
        alto: p.alto,
        pixeles: p.rgba,
    };
    std::fs::write(&ruta, pixpin_codec::imagen::codificar_png(&img).unwrap()).unwrap();
    ruta
}

/// El encargo de un pedido `asistente`, como lo arma la cola.
fn encargo_del_pedido(json: &str) -> encargo::Encargo {
    use crate::pedidos::{Pedido, leer as leer_pedido};
    let Pedido::Asistente {
        texto,
        proyecto,
        imagenes,
        ..
    } = leer_pedido(json).unwrap()
    else {
        panic!("no es un pedido asistente")
    };
    let e = encargo::Encargo {
        texto,
        proyecto,
        imagenes,
        origen: None,
        bloquear: None,
    };
    super::cola::Registro::nuevo("prueba".into(), &e, 1).encargo()
}

/// **El usuario, por la cola** (Flow, `pixpinmax --pedido`): «[img 01]
/// calca esto en el canvas» con una lamina. El modelo llama a
/// `calcar_imagen` y el EJECUTOR la pasa como hoja (figuras exactas y
/// texto), no la calca. Caso negativo: sin modelo con vision (el local),
/// se calca como siempre.
#[test]
fn calca_esto_con_una_lamina_por_la_cola_va_como_hoja() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    for (con_vision, nombre) in [(true, "calca-lamina"), (false, "calca-lamina-local")] {
        let raiz = raiz_de_prueba(nombre);
        let ruta = png_de_lamina(&raiz);
        let json = json!({"pixpin":1,"accion":"asistente","texto":"[img 01] calca esto en el canvas","imagenes":[ruta.to_string_lossy()]}).to_string();
        let e = encargo_del_pedido(&json);
        let ent = encargo::Entorno {
            ubicacion: pixpin_store::Ubicacion::Instalado { raiz: raiz.clone() },
            aparato: "PC01".into(),
        };
        let motor = Arc::new(MotorGuionado::nuevo(vec![
            Respuesta::de_llamada("calcar_imagen", "{}"),
            Respuesta::de_texto("Listo."),
        ]));
        let vision: Option<Arc<dyn pixpin_ia::motor::Motor>> = if con_vision {
            Some(Arc::new(VisionSinRed))
        } else {
            None
        };
        let r = encargo::hacer_con_vision(
            &ent,
            motor.clone(),
            vision,
            &e,
            encargo::Permiso::Fijo(Box::new(|_| false)),
            Box::new(|_| 1),
        )
        .unwrap();
        // Lo que le volvio al modelo de la herramienta.
        let recibido = motor.recibido.lock().unwrap();
        let ultimo = recibido
            .last()
            .unwrap()
            .mensajes
            .iter()
            .map(|m| m.texto_entero())
            .collect::<Vec<_>>()
            .join("\n");
        eprintln!(
            "{nombre}: {}",
            ultimo
                .chars()
                .rev()
                .take(400)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>()
        );
        if con_vision {
            assert!(
                ultimo.contains("era una lamina"),
                "con vision la lamina va como hoja: {ultimo}"
            );
            assert!(
                ultimo.contains("figura(s) con rectas"),
                "con su figura exacta: {ultimo}"
            );
        } else {
            assert!(
                !ultimo.contains("era una lamina"),
                "sin vision se calca: {ultimo}"
            );
        }
        assert!(
            r.lienzo.is_some(),
            "dibujo en el lienzo del asistente: {r:?}"
        );
        drop(recibido);
        let _ = std::fs::remove_dir_all(&raiz);
    }
}

/// **El mismo pedido, con Claude de verdad** (la clave de Ajustes, leida en
/// memoria): exactamente `{"accion":"asistente","texto":"[img 01] calca esto
/// en el canvas","imagenes":["E:/pixpin-demo/lamina-prueba.png"]}` (copiada
/// a una carpeta de la prueba: no se toca el original).
///
/// ```text
/// cargo test -p pixpin -- --ignored --nocapture pedido_calca_esto_con_claude
/// ```
#[test]
#[ignore = "con la nube de verdad"]
fn pedido_calca_esto_con_claude() {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let raiz = raiz_de_prueba("calca-claude");
    std::fs::create_dir_all(&raiz).unwrap();
    let ruta = raiz.join("lamina-prueba.png");
    std::fs::copy(r"E:\pixpin-demo\lamina-prueba.png", &ruta).expect("la lamina de E:/pixpin-demo");
    let json = json!({"pixpin":1,"accion":"asistente","texto":"[img 01] calca esto en el canvas","imagenes":[ruta.to_string_lossy()]}).to_string();
    let e = encargo_del_pedido(&json);
    let ent = encargo::Entorno {
        ubicacion: pixpin_store::Ubicacion::Instalado { raiz: raiz.clone() },
        aparato: "PC01".into(),
    };
    let p = pixpin_ia::nube::Proveedor::Claude;
    let motor =
        crate::asistente::nube::motor(p, None, false).expect("la clave de Claude en Ajustes");
    let clave = pixpin_ia::nube::claves::leer(p).unwrap();
    let vision: Arc<dyn pixpin_ia::motor::Motor> =
        Arc::new(pixpin_ia::nube::MotorNube::nuevo(p, clave).ligero());
    let t = std::time::Instant::now();
    let r = encargo::hacer_con_vision(
        &ent,
        motor,
        Some(vision),
        &e,
        encargo::Permiso::Fijo(Box::new(|_| false)),
        Box::new(|_| 1),
    )
    .unwrap();
    println!(
        "{:.1} s, bien {}, lienzo {:?}\n{}",
        t.elapsed().as_secs_f32(),
        r.bien,
        r.lienzo,
        r.texto
    );
    assert!(r.lienzo.is_some(), "{r:?}");
    let _ = std::fs::remove_dir_all(&raiz);
}
