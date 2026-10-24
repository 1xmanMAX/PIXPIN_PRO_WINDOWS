//! Pruebas del indice de documentos con unos datos de PixPin inventados en
//! una carpeta temporal (nunca los del usuario): proyectos, chats, adjuntos,
//! notas, un chat compartido, un PDF roto y el `.txt` de un OCR. Los
//! formatos de verdad (PDF, Word, PowerPoint, Excel) salen de copias de
//! `E:\pixpin-demo-om` hechas en la carpeta temporal; si no estan, esa parte
//! se salta y lo dice.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use pixpin_ia::agente::Ejecutor;
use pixpin_ia::documentos::{Abrir, Alcance, Indice, Lector, PeticionBusqueda, Tipo};
use pixpin_ia::herramientas::Llamada;

use super::*;

/// Las pruebas tocan el servicio (uno por proceso): de una en una.
static SERIE: Mutex<()> = Mutex::new(());

const MUESTRAS: &str = r"E:\pixpin-demo-om";

fn carpeta(nombre: &str) -> PathBuf {
    let c = std::env::temp_dir().join(format!("pixpin-docs-ia-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&c);
    std::fs::create_dir_all(&c).unwrap();
    c
}

fn linea(v: serde_json::Value) -> String {
    format!("{v}\n")
}

/// Unos datos de PixPin: «Mensajes guardados», «Obra» (compartido no),
/// «Casa» (compartido con otra persona) y el chat «Asistente».
fn datos(raiz: &Path, con_muestras: bool) {
    let p = raiz.join("proyectos");
    std::fs::create_dir_all(&p).unwrap();
    std::fs::write(
        p.join("indice.json"),
        serde_json::json!({"proyectos": [
            {"id": "gua", "nombre": "Guardados", "guardados": true},
            {"id": "obra", "nombre": "Obra Carretera"},
            {"id": "casa", "nombre": "Casa"},
            {"id": "ia", "nombre": "Asistente", "asistente": true},
        ]})
        .to_string(),
    )
    .unwrap();
    // Obra: notas, voz, un plano mandado, un PDF roto y un escaneado con su OCR.
    let obra = p.join("obra");
    std::fs::create_dir_all(obra.join("archivos")).unwrap();
    std::fs::create_dir_all(obra.join("notas")).unwrap();
    std::fs::write(
        obra.join("archivos").join("roto.pdf"),
        b"%PDF-1.4 esto no es un pdf de verdad",
    )
    .unwrap();
    std::fs::write(
        obra.join("archivos").join("escaneado.pdf"),
        b"%PDF-1.4 (una foto)",
    )
    .unwrap();
    std::fs::write(
        obra.join("archivos").join("escaneado (texto).txt"),
        "--- Hoja 1 ---\nActa de entrega de obra.\n--- Hoja 2 ---\nEl contratista entrega las alcantarillas de concreto.\n",
    )
    .unwrap();
    std::fs::write(
        obra.join("archivos").join("plano cunetas km 12.dwg"),
        b"AC1032",
    )
    .unwrap();
    std::fs::write(
        obra.join("notas").join("ideas.md"),
        "# Ideas de la obra\n\nRevisar el presupuesto del asfalto con el ingeniero residente.\n",
    )
    .unwrap();
    let mut jsonl = String::new();
    jsonl += &linea(
        serde_json::json!({"id": "m1", "cuando": 1_790_985_600_000_i64, "clase": "NOTA",
        "texto": "Mañana llega el camión con el cemento portland", "proyecto": "obra"}),
    );
    jsonl += &linea(
        serde_json::json!({"id": "m2", "cuando": 1_790_985_700_000_i64, "clase": "ARCHIVO",
        "nombre": "plano cunetas km 12.dwg", "ruta": "archivos/plano cunetas km 12.dwg", "texto": "", "proyecto": "obra"}),
    );
    jsonl += &linea(
        serde_json::json!({"id": "m3", "cuando": 1, "clase": "VOZ", "texto": "",
        "transcripcion": "llamar al topografo por los niveles", "proyecto": "obra"}),
    );
    jsonl += &linea(
        serde_json::json!({"id": "m4", "cuando": 2, "clase": "ARCHIVO", "nombre": "roto.pdf",
        "ruta": "archivos/roto.pdf", "proyecto": "obra"}),
    );
    jsonl += &linea(
        serde_json::json!({"id": "m5", "cuando": 3, "clase": "ARCHIVO", "nombre": "escaneado.pdf",
        "ruta": "archivos/escaneado.pdf", "proyecto": "obra"}),
    );
    jsonl += &linea(
        serde_json::json!({"id": "m6", "cuando": 4, "clase": "ARCHIVO", "nombre": "escaneado (texto).txt",
        "ruta": "archivos/escaneado (texto).txt", "proyecto": "obra", "aparato": "IA"}),
    );
    // Lo que contesto el asistente no se indexa.
    jsonl += &linea(
        serde_json::json!({"id": "m7", "cuando": 5, "clase": "NOTA", "aparato": "IA",
        "texto": "Respuesta del asistente sobre cemento portland", "proyecto": "obra"}),
    );
    // Una linea rota no tumba nada.
    jsonl += "{esto no es json\n";
    if con_muestras {
        for (i, n) in ["Tesis.pdf", "muestra.docx", "Charla.pptx", "cf.xlsx"]
            .iter()
            .enumerate()
        {
            std::fs::copy(Path::new(MUESTRAS).join(n), obra.join("archivos").join(n)).unwrap();
            jsonl += &linea(
                serde_json::json!({"id": format!("d{i}"), "cuando": 10 + i, "clase": "ARCHIVO",
                "nombre": n, "ruta": format!("archivos/{n}"), "proyecto": "obra"}),
            );
        }
    }
    std::fs::write(obra.join("guardados.jsonl"), jsonl).unwrap();
    // Casa: compartido con otra persona.
    let casa = p.join("casa");
    std::fs::create_dir_all(&casa).unwrap();
    std::fs::write(
        casa.join("guardados.jsonl"),
        linea(serde_json::json!({"id": "c1", "cuando": 1, "clase": "NOTA",
            "texto": "Comprar pintura para la cocina y cemento para el patio", "proyecto": "casa"})),
    )
    .unwrap();
    std::fs::create_dir_all(raiz.join("sincro")).unwrap();
    std::fs::write(
        raiz.join("sincro").join("compartidos.json"),
        serde_json::json!([{"chat": "casa", "nombre": "Casa", "creador": "yo", "creado": 1, "personas": []}]).to_string(),
    )
    .unwrap();
    // Guardados y el chat del asistente.
    let gua = p.join("gua");
    std::fs::create_dir_all(&gua).unwrap();
    std::fs::write(
        gua.join("guardados.jsonl"),
        linea(serde_json::json!({"id": "g1", "cuando": 1, "clase": "NOTA", "texto": "La clave del wifi de la oficina"})),
    )
    .unwrap();
    let ia = p.join("ia");
    std::fs::create_dir_all(&ia).unwrap();
    std::fs::write(
        ia.join("guardados.jsonl"),
        linea(serde_json::json!({"id": "a1", "cuando": 1, "clase": "NOTA", "texto": "¿Que dice la tesis del cemento?"})),
    )
    .unwrap();
}

fn hay_muestras() -> bool {
    let hay = ["Tesis.pdf", "muestra.docx", "Charla.pptx", "cf.xlsx"]
        .iter()
        .all(|n| Path::new(MUESTRAS).join(n).is_file());
    if !hay {
        println!("(sin las muestras de {MUESTRAS}: se saltan los formatos de verdad)");
    }
    hay
}

fn ubicacion(raiz: &Path) -> Ubicacion {
    Ubicacion::Portable {
        raiz: raiz.to_path_buf(),
    }
}

#[test]
fn las_fuentes_son_las_que_tocan() {
    let _s = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    let raiz = carpeta("fuentes");
    datos(&raiz, false);
    let f = fuentes(&raiz);
    let claves: Vec<&str> = f.iter().map(|f| f.nombre.as_str()).collect();
    assert!(claves.contains(&"Chat de Obra Carretera"), "{claves:?}");
    assert!(claves.contains(&"Chat de Mensajes guardados"), "{claves:?}");
    assert!(claves.contains(&"Chat de Casa"), "{claves:?}");
    // El chat del asistente, no.
    assert!(
        !claves.iter().any(|c| c.contains("Asistente")),
        "{claves:?}"
    );
    // El plano .dwg no se lee (su nombre sale por el chat).
    assert!(!claves.iter().any(|c| c.ends_with(".dwg")), "{claves:?}");
    // El OCR cuenta como el PDF escaneado y lo abre a el.
    let ocr = f
        .iter()
        .find(|f| f.ruta.ends_with("escaneado (texto).txt"))
        .unwrap();
    assert_eq!(ocr.nombre, "escaneado.pdf");
    assert_eq!(ocr.tipo, Tipo::Pdf);
    assert!(matches!(&ocr.abrir, Abrir::Fichero { ruta, .. } if ruta.ends_with("escaneado.pdf")));
    // La nota Markdown, por su titulo.
    assert!(claves.contains(&"Nota «Ideas de la obra»"), "{claves:?}");
    // El chat de guardados abre con `proyecto: null`.
    let g = f
        .iter()
        .find(|f| f.nombre == "Chat de Mensajes guardados")
        .unwrap();
    assert!(matches!(&g.abrir, Abrir::Mensaje { proyecto: None, .. }));
    // Sin datos, nada (y sin panico).
    assert!(fuentes(&raiz.join("no-esta")).is_empty());
}

#[test]
fn se_indexa_y_se_encuentra_con_su_origen() {
    let _s = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    let raiz = carpeta("indexa");
    let muestras = hay_muestras();
    datos(&raiz, muestras);
    let mut ix = Indice::cargar(&carpeta_cache(&raiz));
    let inf = ix.poner_al_dia(&fuentes(&raiz), &LectorDeLaApp);
    // El PDF roto se apunta como roto; lo demas entra.
    assert!(inf.rotas.iter().any(|(n, _)| n == "roto.pdf"), "{inf:?}");
    let todo = Alcance::Todo;

    // Un mensaje: abre el chat en el.
    let r = ix.buscar("cemento portland", &todo, 5);
    assert_eq!(
        r[0].abrir,
        Abrir::Mensaje {
            proyecto: Some("obra".into()),
            codigo: "m1".into()
        },
        "{r:#?}"
    );
    // Lo que dijo el asistente no sale.
    assert!(
        r.iter()
            .all(|h| !h.fragmento.contains("Respuesta del asistente"))
    );
    // «¿Donde mande el plano de cunetas?»: el mensaje con el adjunto.
    let r = ix.buscar("donde mande el plano de cunetas", &todo, 5);
    assert_eq!(r[0].seccion.as_deref(), Some("adjunto"), "{r:#?}");
    assert!(matches!(&r[0].abrir, Abrir::Mensaje { codigo, .. } if codigo == "m2"));
    // La voz pasada a texto.
    assert!(!ix.buscar("topografo niveles", &todo, 5).is_empty());
    // El OCR de un escaneado: el PDF, en su pagina.
    let r = ix.buscar("alcantarillas", &todo, 5);
    assert_eq!(r[0].nombre, "escaneado.pdf");
    assert_eq!(r[0].pagina, Some(2));
    assert!(
        matches!(&r[0].abrir, Abrir::Fichero { ruta, pagina: Some(2) } if ruta.ends_with("escaneado.pdf"))
    );
    // La nota Markdown.
    let r = ix.buscar("busca en mis notas el presupuesto del asfalto", &todo, 5);
    assert_eq!(r[0].tipo, Tipo::Nota, "{r:#?}");
    // Mensajes guardados, con su `null`.
    let r = ix.buscar("wifi oficina", &todo, 5);
    assert!(matches!(&r[0].abrir, Abrir::Mensaje { proyecto: None, .. }));

    if muestras {
        // Un PDF de verdad, con su pagina.
        let r = ix.buscar("tesis", &todo, 5);
        assert!(
            r.iter()
                .any(|h| h.nombre == "Tesis.pdf" && h.pagina.is_some()),
            "{r:#?}"
        );
        let pdf = r.iter().find(|h| h.nombre == "Tesis.pdf").unwrap();
        println!("Tesis: pagina {:?}: {}", pdf.pagina, pdf.fragmento);
        // Y los demas formatos leen algo.
        let (n, trozos) = ix.tamano();
        println!("indice: {n} fuentes, {trozos} trozos");
        for nombre in ["muestra.docx", "Charla.pptx", "cf.xlsx"] {
            let f = fuentes(&raiz)
                .into_iter()
                .find(|f| f.nombre == nombre)
                .unwrap();
            let u = LectorDeLaApp
                .leer(&f)
                .unwrap_or_else(|e| panic!("{nombre}: {e}"));
            assert!(
                !u.is_empty() && u.iter().any(|u| !u.texto.trim().is_empty()),
                "{nombre}: vacio"
            );
            println!(
                "{nombre}: {} unidades; la primera: {:.120}",
                u.len(),
                u[0].texto.replace('\n', " ")
            );
            if nombre == "Charla.pptx" {
                assert!(
                    u.iter().all(|u| u.pagina.is_some()),
                    "las diapositivas llevan su numero"
                );
            }
        }
    }
}

#[test]
fn desde_un_chat_compartido_solo_se_ve_ese_chat() {
    let _s = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    let raiz = carpeta("compartido");
    datos(&raiz, false);
    assert_eq!(alcance_desde(&raiz, "casa"), Alcance::solo("casa"));
    assert_eq!(alcance_desde(&raiz, "obra"), Alcance::Todo);
    poner_ubicacion(ubicacion(&raiz));
    let p = |c: &str, proyecto: Option<&str>| PeticionBusqueda {
        consulta: c.into(),
        proyecto: proyecto.map(str::to_string),
    };
    // Desde «Casa»: el cemento de casa sí, el de la obra no.
    let (r, _) = buscar(&p("cemento", None), "casa").unwrap();
    assert!(!r.is_empty());
    assert!(r.iter().all(|h| h.proyecto == "casa"), "{r:#?}");
    // Ni pidiendo otro proyecto por su nombre (y sin decir si existe).
    let (r, aviso) = buscar(&p("cemento", Some("Obra")), "casa").unwrap();
    assert!(r.iter().all(|h| h.proyecto == "casa"), "{r:#?}");
    assert_eq!(
        aviso.as_deref(),
        Some("Desde este chat compartido solo se busca en este chat.")
    );
    // Desde «Obra» (no compartido): todo, y se puede acotar por nombre.
    let (r, _) = buscar(&p("cemento", None), "obra").unwrap();
    assert!(
        r.iter().any(|h| h.proyecto == "casa") && r.iter().any(|h| h.proyecto == "obra"),
        "{r:#?}"
    );
    let (r, aviso) = buscar(&p("cemento", Some("obra")), "obra").unwrap();
    assert!(aviso.is_none());
    assert!(r.iter().all(|h| h.proyecto == "obra"), "{r:#?}");
    let (_, aviso) = buscar(&p("cemento", Some("Marte")), "obra").unwrap();
    assert!(
        aviso
            .unwrap()
            .contains("No hay ningun proyecto llamado «Marte»")
    );
    vaciar_para_pruebas();
}

#[test]
fn el_ejecutor_busca_cita_y_respeta_la_nube() {
    let _s = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    let raiz = carpeta("ejecutor");
    datos(&raiz, false);
    poner_ubicacion(ubicacion(&raiz));
    let (tx, rx) = std::sync::mpsc::channel();
    let parar = Arc::new(AtomicBool::new(false));
    let llamada = Llamada::Buscar(PeticionBusqueda {
        consulta: "cemento portland".into(),
        proyecto: None,
    });
    // Local: busca (la primera vez construye el indice) y numera.
    let mut ej = super::super::ejecutor::EjecutorDelChat::nuevo(
        tx.clone(),
        "obra".into(),
        vec![],
        parar.clone(),
    );
    let texto = ej.ejecutar(&llamada).unwrap();
    assert!(
        texto.contains("[1]") && texto.contains("cemento"),
        "{texto}"
    );
    let citas = rx
        .try_iter()
        .find_map(|a| match a {
            super::super::turno::Aviso::Citas(c) => Some(c),
            _ => None,
        })
        .expect("manda las citas a la ventana");
    assert_eq!(citas[0].numero, 1);
    // La respuesta guarda la cita y el chat la lee para su tarjeta.
    let fuentes = super::super::chat::fuentes_para_la_nota(&citas, "Llega mañana [1].").unwrap();
    let mut nota = pixpin_proyecto::cuaderno::Mensaje {
        aparato: Some(super::super::chat::APARATO.into()),
        ..Default::default()
    };
    nota.resto
        .insert(super::super::chat::FUENTES_DE_LA_RESPUESTA.into(), fuentes);
    let f = super::super::chat::fuente_de_la_respuesta(&nota).unwrap();
    assert_eq!(f.pedido["que"]["tipo"], "mensaje");
    assert_eq!(f.pedido["que"]["codigo"], "m1");
    assert_eq!(f.proyecto, "Obra Carretera");

    // Con la nube y el ajuste en «no»: no sale nada.
    let mut a = pixpin_store::Ajustes::default();
    a.asistente.documentos_a_la_nube = "no".into();
    pixpin_store::ajustes::guardar(&ubicacion(&raiz), &a).unwrap();
    let mut ej = super::super::ejecutor::EjecutorDelChat::nuevo(
        tx.clone(),
        "obra".into(),
        vec![],
        parar.clone(),
    )
    .en_la_nube(true);
    let e = ej.ejecutar(&llamada).unwrap_err();
    assert!(e.contains("no permite"), "{e}");
    // Con «preguntar»: se pregunta (en las pruebas se dice que no) y se recuerda.
    a.asistente.documentos_a_la_nube = String::new();
    pixpin_store::ajustes::guardar(&ubicacion(&raiz), &a).unwrap();
    let mut ej = super::super::ejecutor::EjecutorDelChat::nuevo(
        tx.clone(),
        "obra".into(),
        vec![],
        parar.clone(),
    )
    .en_la_nube(true);
    assert!(ej.ejecutar(&llamada).is_err());
    let leido = pixpin_store::ajustes::cargar(&ubicacion(&raiz)).unwrap();
    assert_eq!(leido.asistente.documentos_a_la_nube, "no");
    // Con «si»: busca.
    a.asistente.documentos_a_la_nube = "si".into();
    pixpin_store::ajustes::guardar(&ubicacion(&raiz), &a).unwrap();
    let mut ej = super::super::ejecutor::EjecutorDelChat::nuevo(tx, "obra".into(), vec![], parar)
        .en_la_nube(true);
    assert!(ej.ejecutar(&llamada).unwrap().contains("[1]"));
    vaciar_para_pruebas();
}

#[test]
fn sin_datos_ni_consulta_no_rompe() {
    let _s = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    vaciar_para_pruebas();
    // Sin ubicacion: error claro, no panico.
    let p = PeticionBusqueda {
        consulta: "cemento".into(),
        proyecto: None,
    };
    assert!(buscar(&p, "obra").is_err());
    // Una carpeta vacia: nada que encontrar.
    let raiz = carpeta("vacia");
    poner_ubicacion(ubicacion(&raiz));
    let (r, _) = buscar(&p, "obra").unwrap();
    assert!(r.is_empty());
    // Consulta sin palabras: nada.
    let (r, _) = buscar(
        &PeticionBusqueda {
            consulta: "¿de que?".into(),
            proyecto: None,
        },
        "obra",
    )
    .unwrap();
    assert!(r.is_empty());
    vaciar_para_pruebas();
}

/// **De verdad, con Qwen3.5-2B**: un turno entero preguntando por la tesis
/// (una copia de `E:\pixpin-demo-om\Tesis.pdf` en unos datos inventados).
///
/// ```text
/// cargo test -p pixpin -- --ignored --nocapture asistente::documentos::pruebas::de_verdad
/// ```
#[test]
#[ignore = "necesita el motor y el modelo bajados, y la muestra Tesis.pdf"]
fn de_verdad() {
    use pixpin_ia::servidor::{Config, Dispositivo, MotorLlama};
    let _s = SERIE.lock().unwrap_or_else(|e| e.into_inner());
    let base = PathBuf::from(std::env::var("LOCALAPPDATA").unwrap())
        .join("PixPinMax")
        .join("modelos");
    let mut config = Config::nueva(
        base.join("llama.cpp")
            .join("b11580")
            .join("llama-server.exe"),
        base.join("qwen3.5-2b").join("Qwen3.5-2B-Q4_K_M.gguf"),
        None,
    );
    config.dispositivo = match std::env::var("PIXPIN_IA_DISPOSITIVO").as_deref() {
        Ok("cpu") | Err(_) => Dispositivo::Cpu,
        Ok(d) => Dispositivo::Gpu(d.into()),
    };
    let motor: Arc<dyn pixpin_ia::Motor> = Arc::new(MotorLlama::nuevo(config));
    let raiz = carpeta("de-verdad");
    datos(&raiz, true);
    poner_ubicacion(ubicacion(&raiz));
    let pregunta = std::env::var("PIXPIN_IA_PREGUNTA")
        .unwrap_or_else(|_| "¿Qué dice la tesis sobre el pavimento?".into());
    for pregunta in [pregunta.as_str(), "¿Dónde mandé el plano de cunetas?"] {
        let t0 = std::time::Instant::now();
        let historial = super::super::turno::historial_para_el_modelo(&[], pregunta, None);
        let mut t = super::super::turno::Turno::lanzar(
            motor.clone(),
            "obra".into(),
            historial,
            vec![],
            None,
        );
        let mut citas = Vec::new();
        let fin = loop {
            let mut fin = None;
            for a in t.recibir() {
                match a {
                    super::super::turno::Aviso::Citas(c) => citas = c,
                    super::super::turno::Aviso::Fin { resultado } => fin = Some(resultado),
                    super::super::turno::Aviso::Evento(
                        pixpin_ia::agente::Evento::Herramienta { llamada, .. },
                    ) => {
                        println!("  herramienta: {llamada:?}")
                    }
                    _ => {}
                }
            }
            if let Some(f) = fin {
                break f;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        };
        let fin = fin.expect("el turno acaba bien");
        println!(
            "«{pregunta}» ({:.1} s):\n{}\n",
            t0.elapsed().as_secs_f32(),
            fin.texto
        );
        for c in &citas {
            println!("  [{}] {} -> {}", c.numero, c.titulo(), c.pedido());
        }
        assert!(!citas.is_empty(), "tenia que buscar en los documentos");
    }
    motor.soltar();
    vaciar_para_pruebas();
}
