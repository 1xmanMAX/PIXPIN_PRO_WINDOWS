//! **Las herramientas nuevas del asistente, del lado de la app**: calcar
//! una imagen adjunta al lienzo, hacer una accion de la app (un pedido del
//! protocolo) y contestar preguntas sobre PixPin.
//!
//! Las acciones de la app van por **el mismo camino que el plugin de Flow
//! Launcher**: el JSON del pedido a la ventana de mensajes de PixPin
//! (`pixpin_shell::mensajero::enviar_pedido`), y el bucle principal lo hace
//! con `pedidos::atender`, identico a si lo hubiera pedido el usuario. Asi
//! el asistente no tiene un segundo camino para escribir en los cuadernos.
//!
//! Lo que borra o saca algo del PC (`AccionApp::confirmar`) **pregunta
//! antes**, en una tarjeta dentro de la burbuja del asistente con «Sí» y
//! «No» (13-oct-2026; antes era un cuadro de Windows que tapaba la
//! ventana): [`confirmar_en_la_burbuja`]. El turno espera en su hilo; la
//! ventana sigue suelta. Sin respuesta (chat cerrado, «parar», o
//! [`super::chat::CADUCA_LA_PREGUNTA`]) es un no.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::time::Duration;

use pixpin_ia::acciones::AccionApp;
use pixpin_ia::fechas::Momento;
use pixpin_ia::tareas::{Chat, Fijas, FuenteTareas, VistaLista, VistaTarea};
use pixpin_ia::calcar::{self, Parametros};
use pixpin_ia::lienzo;
use serde_json::Value;

use super::turno::{Adjunto, Aviso};

/// Pregunta al usuario; `true` si dice que si.
pub type Confirmar = Box<dyn Fn(&str) -> bool + Send>;
/// Manda el JSON de un pedido a PixPin; devuelve la respuesta del protocolo
/// (`1` aceptado).
pub type Mandar = Box<dyn Fn(&str) -> isize + Send>;

/// Sin nadie a quien preguntar: no.
pub fn decir_que_no() -> Confirmar {
    Box::new(|_| false)
}

/// **La pregunta va a la burbuja** por el canal del turno y se espera el
/// «Sí»/«No» de la tarjeta. Cualquier otra cosa es un no: el chat se cerro
/// (el `Sender` de la respuesta murio con su sesion), el usuario pulso
/// parar, o el canal del turno ya no tiene a nadie escuchando.
pub fn confirmar_en_la_burbuja(tx: Sender<Aviso>, parar: Arc<AtomicBool>) -> Confirmar {
    Box::new(move |pregunta| {
        let (responder, rx) = channel();
        if tx
            .send(Aviso::Confirmar {
                pregunta: pregunta.to_string(),
                responder,
            })
            .is_err()
        {
            return false;
        }
        loop {
            if parar.load(Ordering::Relaxed) {
                return false;
            }
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(si) => return si,
                Err(RecvTimeoutError::Disconnected) => return false,
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    })
}

/// Lo que las pruebas mandaron a «PixPin» (nunca sale de verdad).
#[cfg(test)]
pub static MANDADOS_EN_PRUEBAS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// A la PixPin que corre (esta misma), como el plugin.
pub fn mandar_a_pixpin() -> Mandar {
    // En las pruebas, nada sale: la PixPin del usuario puede estar abierta y
    // un pedido le haria cosas de verdad. Se apunta para comprobarlo.
    #[cfg(test)]
    return Box::new(|j| {
        MANDADOS_EN_PRUEBAS.lock().unwrap_or_else(|e| e.into_inner()).push(j.to_string());
        0
    });
    #[cfg(not(test))]
    Box::new(pixpin_shell::mensajero::enviar_pedido)
}

fn ultimo_pedido(historial: &[pixpin_ia::Mensaje]) -> String {
    historial
        .iter()
        .rev()
        .find(|m| m.rol == pixpin_ia::Rol::Usuario)
        .map(pixpin_ia::Mensaje::texto_entero)
        .unwrap_or_default()
}

fn hay_imagen(adjuntos: &[Adjunto], con_foto: bool) -> bool {
    con_foto || adjuntos.iter().any(|a| super::ocr::es_imagen(&a.ruta))
}

/// **Si este turno es «copia/dibuja esto» con una imagen**: entonces solo
/// se ofrece `calcar_imagen`, al modelo local y a los de la nube (medido
/// 14-oct: con mas herramientas a mano, el 2B dibujaba el perrito adjunto
/// con 4 triangulos).
pub fn es_para_calcar(historial: &[pixpin_ia::Mensaje], adjuntos: &[Adjunto], con_foto: bool) -> bool {
    let q = pixpin_ia::acciones::normalizar(&ultimo_pedido(historial));
    let palabras: Vec<&str> = q.split(|c: char| !c.is_alphanumeric()).filter(|p| !p.is_empty()).collect();
    hay_imagen(adjuntos, con_foto) && pixpin_ia::seleccion::pide_la_imagen(&palabras) && !pixpin_ia::seleccion::pide_una_tabla(&palabras)
}

/// **Las herramientas de este turno**: las que vienen a cuento del ultimo
/// mensaje del usuario (`pixpin_ia::seleccion`), y el OCR si hay un PDF o
/// una imagen adjunta (preguntar «¿que dice?» no lleva palabras clave).
pub fn herramientas_para(
    historial: &[pixpin_ia::Mensaje],
    adjuntos: &[Adjunto],
    con_foto: bool,
) -> Vec<pixpin_ia::Herramienta> {
    let pedido = ultimo_pedido(historial);
    let mut h = pixpin_ia::seleccion::para_el_pedido(&pedido, hay_imagen(adjuntos, con_foto));
    if es_para_calcar(historial, adjuntos, con_foto) {
        return h;
    }
    if !adjuntos.is_empty() && !h.iter().any(|x| x.nombre == pixpin_ia::herramientas::OCR) {
        if let Some(ocr) = pixpin_ia::herramientas::catalogo_base()
            .into_iter()
            .find(|x| x.nombre == pixpin_ia::herramientas::OCR)
        {
            h.push(ocr);
        }
    }
    h
}

/// **Las tareas del usuario, para el asistente** (`pixpin_ia::tareas`): las
/// listas de todos los chats como las ve la ventana de tareas
/// (`tareas::reunir`), sus chats y la hora local.
pub struct FuenteDeLaApp {
    raiz: PathBuf,
}

impl FuenteDeLaApp {
    pub fn nueva(raiz: &Path) -> FuenteDeLaApp {
        FuenteDeLaApp { raiz: raiz.to_path_buf() }
    }
}

/// Una lista de la ventana de tareas como la ve el asistente.
pub fn vista_de(l: &crate::tareas::Lista) -> VistaLista {
    VistaLista {
        proyecto: l.proyecto.clone(),
        chat: l.chat.clone(),
        guardados: l.guardados,
        codigo: l.codigo.clone(),
        titulo: l.titulo.clone(),
        cuando: l.cuando,
        tareas: l
            .filas
            .iter()
            .map(|f| VistaTarea {
                indice: f.indice,
                texto: crate::tareas::sin_hora(&f.texto).trim().to_string(),
                hecha: f.hecha,
                creada: f.creada.map(|d| d.dias()),
                vence: Momento::de_marca(&f.texto),
            })
            .collect(),
    }
}

impl FuenteTareas for FuenteDeLaApp {
    fn listas(&self) -> Vec<VistaLista> {
        crate::tareas::reunir(&self.raiz).0.iter().map(vista_de).collect()
    }

    fn chats(&self) -> Vec<Chat> {
        pixpin_proyecto::almacen::Indice::leer(&self.raiz)
            .proyectos
            .iter()
            .map(|f| Chat {
                id: f.id.clone(),
                nombre: f.nombre.clone(),
                guardados: f.es_guardados(),
            })
            .collect()
    }

    fn ahora(&self) -> Momento {
        Momento::de_ms_locales(pixpin_shell::entorno::ahora_local_ms())
    }
}

/// **Hace una accion de la app**: con `raiz`, las tareas por su nombre se
/// resuelven con las listas de verdad (`pixpin_ia::tareas::hacer`); pregunta
/// antes si hace falta (con los nombres de verdad) y manda los pedidos en
/// orden. `Ok`: la frase corta de «hecho» (o la lista, si era una consulta).
pub fn hacer(a: &AccionApp, raiz: Option<&Path>, confirmar: &Confirmar, mandar: &Mandar) -> Result<String, String> {
    let sin_raiz = Fijas::default();
    let app = raiz.map(FuenteDeLaApp::nueva);
    let fuente: &dyn FuenteTareas = match &app {
        Some(f) => f,
        None if a.es_consulta() || !a.por_resolver.is_empty() => {
            return Err("no puedo leer las listas de tareas desde aqui".into());
        }
        None => &sin_raiz,
    };
    hacer_con(a, fuente, raiz, confirmar, mandar)
}

/// [`hacer`] con otra fuente de tareas (las pruebas).
pub fn hacer_con(
    a: &AccionApp,
    fuente: &dyn FuenteTareas,
    raiz: Option<&Path>,
    confirmar: &Confirmar,
    mandar: &Mandar,
) -> Result<String, String> {
    // Una automatizacion mal hecha se le dice al modelo ya, para que la
    // arregle, y no despues en un globo (el pedido solo contesta «aceptado»).
    crate::automatizaciones::revisar_pedido(&a.pedido)?;
    let mut con_hora = false;
    let r = pixpin_ia::tareas::hacer(a, fuente, &|q| confirmar(q), &mut |p| {
        con_hora |= p.get("texto").and_then(Value::as_str).is_some_and(|t| t.contains('⏰'));
        match mandar(&p.to_string()) {
            pixpin_shell::mensajero::respuesta::ACEPTADO => Ok(()),
            pixpin_shell::mensajero::respuesta::NO_SE_ENTIENDE => Err("PixPin no entendio el pedido".into()),
            pixpin_shell::mensajero::respuesta::VERSION_NO => Err("PixPin no conoce esta version del pedido".into()),
            _ => Err("PixPin no contesto".into()),
        }
    });
    // Una tarea con fecha: que el vigia de recordatorios la tenga ya.
    if con_hora && r.is_ok() && let Some(raiz) = raiz {
        crate::recordatorios::releer(raiz);
    }
    r
}

/// La imagen que se calca: la nombrada, o la ultima imagen adjunta.
pub fn imagen_para_calcar<'a>(adjuntos: &'a [Adjunto], nombre: Option<&str>) -> Result<&'a Adjunto, String> {
    let imagenes: Vec<Adjunto> = adjuntos
        .iter()
        .filter(|a| super::ocr::es_imagen(&a.ruta))
        .cloned()
        .collect();
    if imagenes.is_empty() {
        return Err("no hay ninguna imagen adjunta que calcar; pide al usuario que adjunte la foto".into());
    }
    // Un nombre que no esta (el 2B pidio «perro» con «perrito.png» adjunto,
    // 14-oct) no es motivo para no calcar: la ultima imagen.
    let elegido = super::ocr::elegir(&imagenes, nombre).or_else(|_| super::ocr::elegir(&imagenes, None))?;
    adjuntos
        .iter()
        .find(|a| a.ruta == elegido.ruta)
        .ok_or_else(|| "la imagen ya no esta".into())
}

/// **Calca** una imagen del disco: los elementos de Excalidraw, listos para
/// el lienzo, y cuantos son.
pub fn calcar_fichero(ruta: &Path, p: &Parametros, semilla: u64) -> Result<Vec<Value>, String> {
    let img = pixpin_codec::imagen::cargar(ruta).map_err(|e| format!("no se pudo abrir la imagen: {e}"))?;
    let c = calcar::calcar(&img.pixeles, img.ancho, img.alto, p);
    let els = lienzo::elementos_de_calcado(&c, (0.0, 0.0), semilla);
    if els.is_empty() {
        return Err("la imagen no tiene formas que calcar (esta en blanco o casi)".into());
    }
    Ok(els)
}

#[cfg(test)]
mod pruebas {
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use pixpin_ia::acciones::{ACCIONES, Tipo, por_nombre, validar};
    use serde_json::{Map, json};

    use super::*;

    /// Unos argumentos de ejemplo para cada fila de la tabla.
    fn ejemplo(a: &pixpin_ia::acciones::Accion) -> Map<String, Value> {
        let mut m = Map::new();
        for p in a.params {
            let v = match p.tipo {
                Tipo::Texto if p.nombre == "fecha" => json!("mañana"),
                Tipo::Texto => json!("algo"),
                Tipo::Ruta => json!(r"C:\x\y.png"),
                Tipo::Rutas => json!([r"C:\x\y.png"]),
                Tipo::Textos => json!(["algo"]),
                Tipo::Entero => json!(1),
                Tipo::SiNo => json!(true),
                Tipo::Opciones(o) => json!(o[0]),
                Tipo::Numeros => json!([0.1, 0.2, 0.3, 0.4]),
            };
            m.insert(p.nombre.into(), v);
        }
        m
    }

    /// Una lista «algo» con la tarea «algo» en el chat «algo», y otra
    /// «destino» adonde mover.
    fn fuente() -> Fijas {
        let lista = |codigo: &str, titulo: &str| VistaLista {
            proyecto: "p".into(),
            chat: "algo".into(),
            guardados: false,
            codigo: codigo.into(),
            titulo: titulo.into(),
            cuando: 0,
            tareas: vec![VistaTarea { indice: 0, texto: "algo".into(), hecha: false, creada: None, vence: None }],
        };
        Fijas {
            listas: vec![lista("1", "algo"), lista("2", "destino")],
            chats: vec![Chat { id: "p".into(), nombre: "algo".into(), guardados: false }],
            ahora: Some(Momento { dia: 20_000, minuto: 600 }),
        }
    }

    /// **Lo que genera la tabla del asistente lo entiende la app**: cada
    /// pedido (ya resuelto, con sus ids) pasa el vistazo de la ventana
    /// (version y accion) y lo lee `pedidos::leer`, el mismo que atiende al
    /// plugin.
    #[test]
    fn cada_accion_del_asistente_es_un_pedido_que_la_app_entiende() {
        for a in ACCIONES {
            let mut args = ejemplo(a);
            if args.contains_key("a_lista") {
                args.insert("a_lista".into(), json!("destino"));
            }
            let hecha = validar(a, &args).unwrap_or_else(|e| panic!("{}: {e}", a.herramienta));
            if hecha.es_consulta() {
                assert!(pixpin_ia::tareas::consultar(&hecha.por_resolver, &fuente()).is_ok());
                continue;
            }
            assert!(
                pixpin_shell::mensajero::ACCIONES.contains(&a.accion),
                "{}: la ventana no conoce «{}»",
                a.herramienta,
                a.accion
            );
            let pasos = pixpin_ia::tareas::resolver(&hecha, &fuente()).unwrap_or_else(|e| panic!("{}: {e}", a.herramienta));
            for mut p in pasos.pedidos {
                if let Some(o) = p.as_object_mut()
                    && o.remove("__en_lista_nueva").is_some()
                {
                    o.insert("codigo".into(), json!("3"));
                }
                let json = p.to_string();
                crate::pedidos::leer(&json).unwrap_or_else(|e| panic!("{}: {json}: {e:?}", a.herramienta));
            }
        }
    }

    #[test]
    fn lo_que_pregunta_no_se_manda_si_dice_que_no() {
        let mandados = Arc::new(Mutex::new(Vec::<String>::new()));
        let m = mandados.clone();
        let mandar: Mandar = Box::new(move |j| {
            m.lock().unwrap().push(j.to_string());
            1
        });
        let no: Confirmar = Box::new(|_| false);
        let si: Confirmar = Box::new(|_| true);
        let f = fuente();
        let borrar = validar(por_nombre("borrar_lista").unwrap(), &ejemplo(por_nombre("borrar_lista").unwrap())).unwrap();
        assert!(hacer_con(&borrar, &f, None, &no, &mandar).unwrap_err().contains("dijo que no"));
        assert!(mandados.lock().unwrap().is_empty(), "salio sin permiso");
        assert!(hacer_con(&borrar, &f, None, &si, &mandar).is_ok());
        assert_eq!(mandados.lock().unwrap().len(), 1);
        // Lo que no borra no pregunta.
        let preguntado = Arc::new(Mutex::new(false));
        let p = preguntado.clone();
        let espia: Confirmar = Box::new(move |_| {
            *p.lock().unwrap() = true;
            false
        });
        let tarea = validar(por_nombre("anadir_tarea").unwrap(), &ejemplo(por_nombre("anadir_tarea").unwrap())).unwrap();
        let r = hacer_con(&tarea, &f, None, &espia, &mandar).unwrap();
        assert!(r.starts_with("Apuntada «algo»"), "{r}");
        assert!(!*preguntado.lock().unwrap());
        // Si PixPin no contesta, se dice.
        let sordo: Mandar = Box::new(|_| 0);
        assert!(hacer_con(&tarea, &f, None, &si, &sordo).unwrap_err().contains("no contesto"));
        // Sin donde leer las tareas, lo que las nombra no se hace a ciegas.
        assert!(hacer(&tarea, None, &si, &mandar).unwrap_err().contains("no puedo leer"));
    }

    /// **De punta a punta con las tareas de verdad**: un almacen de prueba,
    /// las ordenes por el camino rapido (sin modelo), los pedidos atendidos
    /// por `pedidos::atender` como los del plugin, y lo que queda en disco.
    #[test]
    fn las_tareas_desde_el_asistente_de_punta_a_punta() {
        let raiz = std::env::temp_dir().join(format!("pixpin-asistente-tareas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        crate::tareas::apuntar(&raiz, "PC01", "comprar pan").unwrap();
        let r2 = raiz.clone();
        let mandar: Mandar = Box::new(move |json| {
            let u = pixpin_store::Ubicacion::Portable { raiz: r2.clone() };
            let textos = pixpin_store::Catalogo::nuevo(pixpin_store::Idioma::Espanol);
            let cx = crate::pedidos::Contexto {
                idioma: pixpin_store::Idioma::Espanol,
                ubicacion: &u,
                opciones: crate::ventana_chat::OpcionesLienzo {
                    enganche: Default::default(),
                    nivel: pixpin_nivel::Nivel::Ligero,
                    medir_fotogramas: false,
                },
                aparato: "PC01",
                textos: &textos,
            };
            let h = crate::pedidos::atender(json, &cx);
            println!("pedido {json} -> {:?}", h.aviso);
            1
        });
        let si: Confirmar = Box::new(|_| true);
        let f = FuenteDeLaApp::nueva(&raiz);
        let ahora = f.ahora();
        let orden = |t: &str| {
            let o = pixpin_ia::rapido::interpretar(t, ahora).unwrap_or_else(|| panic!("«{t}» no va por el camino rapido"));
            let cruda = pixpin_ia::motor::LlamadaCruda {
                id: "x".into(),
                nombre: o.herramienta.into(),
                argumentos: Value::Object(o.args).to_string(),
            };
            match pixpin_ia::herramientas::validar(&cruda, &pixpin_ia::herramientas::catalogo()).unwrap().llamada {
                pixpin_ia::Llamada::App(a) => a,
                otra => panic!("{otra:?}"),
            }
        };
        let t0 = std::time::Instant::now();
        let r = hacer_con(&orden("apunta revisar planos mañana a las 10"), &f, Some(&raiz), &si, &mandar).unwrap();
        println!("anadir por el camino rapido: {:?}", t0.elapsed());
        assert_eq!(r, "Apuntada «revisar planos» para mañana 10:00.");
        let r = hacer_con(&orden("marca como hecha la tarea comprar pan"), &f, Some(&raiz), &si, &mandar).unwrap();
        assert_eq!(r, "«comprar pan» marcada como hecha.");
        let editar = validar(
            por_nombre("editar_tarea").unwrap(),
            &serde_json::from_str(r#"{"tarea":"revisar planos","texto":"revisar planos y cotas"}"#).unwrap(),
        )
        .unwrap();
        hacer_con(&editar, &f, Some(&raiz), &si, &mandar).unwrap();
        let inbox = f.listas().into_iter().find(|l| l.es_inbox()).unwrap();
        assert_eq!(inbox.tareas.len(), 2);
        assert!(inbox.tareas[0].hecha && inbox.tareas[0].texto == "comprar pan", "{inbox:?}");
        assert_eq!(inbox.tareas[1].texto, "revisar planos y cotas");
        assert_eq!(inbox.tareas[1].vence.map(|v| v.dia), Some(ahora.dia + 1), "la fecha sigue tras editar");
        let t0 = std::time::Instant::now();
        let lista = hacer_con(&orden("¿qué tengo pendiente?"), &f, Some(&raiz), &si, &mandar).unwrap();
        println!("consulta por el camino rapido: {:?}", t0.elapsed());
        assert_eq!(lista, "Tareas pendientes (1):\nInbox:\n  ☐ revisar planos y cotas — mañana 10:00");
        // Negativo: una tarea que no esta no toca nada.
        let e = hacer_con(&orden("tacha pintar la luna"), &f, Some(&raiz), &si, &mandar).unwrap_err();
        assert!(e.contains("No encuentro"), "{e}");
        // Mover a una lista nueva y quitar (con su pregunta).
        hacer_con(&orden("crea una lista llamada Obra"), &f, Some(&raiz), &si, &mandar).unwrap();
        let mover = validar(
            por_nombre("mover_tarea").unwrap(),
            &serde_json::from_str(r#"{"tarea":"revisar planos","a_lista":"Obra"}"#).unwrap(),
        )
        .unwrap();
        assert_eq!(hacer_con(&mover, &f, Some(&raiz), &si, &mandar).unwrap(), "«revisar planos y cotas» movida a «Obra».");
        let quitar =
            validar(por_nombre("quitar_tarea").unwrap(), &serde_json::from_str(r#"{"tarea":"comprar pan"}"#).unwrap()).unwrap();
        hacer_con(&quitar, &f, Some(&raiz), &si, &mandar).unwrap();
        let listas = f.listas();
        assert!(listas.iter().find(|l| l.es_inbox()).unwrap().tareas.is_empty());
        assert_eq!(listas.iter().find(|l| l.titulo == "Obra").unwrap().tareas.len(), 1);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_perrito_adjunto_solo_ofrece_calcar_y_un_circulo_no() {
        let perrito = vec![Adjunto {
            nombre: "perrito.png".into(),
            ruta: PathBuf::from(r"C:\x\perrito.png"),
        }];
        let h = |t: &str| vec![pixpin_ia::Mensaje::usuario(t)];
        let pedido = h("Dibuja este perrito de la imagen adjunta dentro del lienzo.");
        assert!(es_para_calcar(&pedido, &perrito, false));
        let n: Vec<String> = herramientas_para(&pedido, &perrito, false).into_iter().map(|x| x.nombre).collect();
        assert_eq!(n, vec!["calcar_imagen"]);
        // Con la foto pegada al mensaje, igual.
        assert!(es_para_calcar(&pedido, &[], true));
        // Negativo: dibujar otra cosa con la imagen ahi no calca.
        let circulo = h("Dibuja un círculo");
        assert!(!es_para_calcar(&circulo, &perrito, false));
        let n: Vec<String> = herramientas_para(&circulo, &perrito, false).into_iter().map(|x| x.nombre).collect();
        assert!(n.contains(&"dibujar_en_lienzo".to_string()) && !n.contains(&"calcar_imagen".to_string()), "{n:?}");
        // Y sin imagen no hay calcado aunque lo pida.
        assert!(!es_para_calcar(&pedido, &[], false));
    }

    #[test]
    fn se_calca_la_imagen_adjunta_y_no_un_pdf() {
        let adj = |n: &str| Adjunto {
            nombre: n.into(),
            ruta: PathBuf::from(format!(r"C:\x\{n}")),
        };
        let lista = vec![adj("foto.png"), adj("tesis.pdf")];
        assert_eq!(imagen_para_calcar(&lista, None).unwrap().nombre, "foto.png");
        assert!(imagen_para_calcar(&[adj("tesis.pdf")], None).is_err());
        assert_eq!(imagen_para_calcar(&lista, Some("perro")).unwrap().nombre, "foto.png");
    }

    /// **Un calcado de verdad, pintado a PNG** por el camino del PDF
    /// (exportar → PDF de vectores → pixeles), para mirarlo:
    /// `PIXPIN_IA_SALIDA=<carpeta> cargo test -p pixpin -- --ignored calcado_a_png`.
    #[test]
    #[ignore = "pinta PNG para mirarlos"]
    fn calcado_a_png() {
        let salida = PathBuf::from(std::env::var("PIXPIN_IA_SALIDA").unwrap_or_else(|_| std::env::temp_dir().display().to_string()));
        // `PIXPIN_IA_ENTRADA`: otra imagen (p. ej. un emoji con degradados).
        let entrada = std::env::var("PIXPIN_IA_ENTRADA").map(PathBuf::from).unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/pixpin-ia/medir/prueba.png")
        });
        for (nombre, p) in [
            ("calcado-colores", Parametros::default()),
            ("calcado-contornos", Parametros { solo_contornos: true, ..Default::default() }),
            ("calcado-alto", Parametros { detalle: calcar::Detalle::Alto, ..Default::default() }),
        ] {
            let t0 = std::time::Instant::now();
            let els = calcar_fichero(&entrada, &p, 3).unwrap();
            println!("{nombre}: {} elementos en {:?}", els.len(), t0.elapsed());
            let lienzo = pixpin_motor2d::excalidraw::leer(&lienzo::documento(els)).unwrap();
            let mut escena = pixpin_motor2d::Escena::nueva();
            for e in lienzo.elementos() {
                escena.anadir(e);
            }
            super::super::pruebas::pintar_a_png(&escena, &salida.join(format!("{nombre}.png")));
        }
    }
}
