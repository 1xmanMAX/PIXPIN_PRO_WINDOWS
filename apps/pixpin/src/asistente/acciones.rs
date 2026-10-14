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
//! antes**. Por ahora con un cuadro de Windows (con «No» por defecto); el
//! enganche es [`Confirmar`]: la UI del chat puede cambiarlo por un «¿Sí?»
//! dentro de la burbuja con [`super::ejecutor::EjecutorDelChat::con_acciones`].

use std::path::Path;

use pixpin_ia::acciones::AccionApp;
use pixpin_ia::calcar::{self, Parametros};
use pixpin_ia::lienzo;
use serde_json::Value;

use super::turno::Adjunto;

/// Pregunta al usuario; `true` si dice que si.
pub type Confirmar = Box<dyn Fn(&str) -> bool + Send>;
/// Manda el JSON de un pedido a PixPin; devuelve la respuesta del protocolo
/// (`1` aceptado).
pub type Mandar = Box<dyn Fn(&str) -> isize + Send>;

/// El cuadro de Windows, con «No» por defecto.
pub fn confirmar_con_cuadro() -> Confirmar {
    // En las pruebas nunca se abre un cuadro: se dice que no.
    if cfg!(test) {
        return Box::new(|_| false);
    }
    Box::new(|pregunta| {
        pixpin_shell::dialogo::confirmar_destructivo(
            windows::Win32::Foundation::HWND::default(),
            "PixPin · Asistente",
            pregunta,
        )
    })
}

/// A la PixPin que corre (esta misma), como el plugin.
pub fn mandar_a_pixpin() -> Mandar {
    // En las pruebas, nada sale: la PixPin del usuario puede estar abierta y
    // un pedido le haria cosas de verdad.
    if cfg!(test) {
        return Box::new(|_| 0);
    }
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
    hay_imagen(adjuntos, con_foto) && pixpin_ia::seleccion::pide_la_imagen(&palabras)
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

/// **Hace una accion de la app**, preguntando antes si hace falta.
pub fn hacer(a: &AccionApp, confirmar: &Confirmar, mandar: &Mandar) -> Result<String, String> {
    // Una automatizacion mal hecha se le dice al modelo ya, para que la
    // arregle, y no despues en un globo (el pedido solo contesta «aceptado»).
    crate::automatizaciones::revisar_pedido(&a.pedido)?;
    if let Some(pregunta) = &a.confirmar
        && !confirmar(pregunta)
    {
        return Err("el usuario dijo que no; no se hizo nada".into());
    }
    match mandar(&a.pedido.to_string()) {
        pixpin_shell::mensajero::respuesta::ACEPTADO => Ok(format!("hecho: {}", resumen(&a.pedido))),
        pixpin_shell::mensajero::respuesta::NO_SE_ENTIENDE => Err("PixPin no entendio el pedido".into()),
        pixpin_shell::mensajero::respuesta::VERSION_NO => Err("PixPin no conoce esta version del pedido".into()),
        _ => Err("PixPin no contesto".into()),
    }
}

/// «accion (campo: valor, ...)», para que el modelo diga lo que hizo.
fn resumen(pedido: &Value) -> String {
    let accion = pedido["accion"].as_str().unwrap_or("?");
    let campos: Vec<String> = pedido
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(k, _)| *k != "pixpin" && *k != "accion")
        .map(|(k, v)| format!("{k}: {v}"))
        .collect();
    if campos.is_empty() {
        accion.to_string()
    } else {
        format!("{accion} ({})", campos.join(", "))
    }
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
                Tipo::Texto => json!("algo"),
                Tipo::Ruta => json!(r"C:\x\y.png"),
                Tipo::Rutas => json!([r"C:\x\y.png"]),
                Tipo::Entero => json!(1),
                Tipo::SiNo => json!(true),
                Tipo::Opciones(o) => json!(o[0]),
            };
            m.insert(p.nombre.into(), v);
        }
        m
    }

    /// **Lo que genera la tabla del asistente lo entiende la app**: cada
    /// pedido pasa el vistazo de la ventana (version y accion) y lo lee
    /// `pedidos::leer`, el mismo que atiende al plugin.
    #[test]
    fn cada_accion_del_asistente_es_un_pedido_que_la_app_entiende() {
        for a in ACCIONES {
            let hecha = validar(a, &ejemplo(a)).unwrap_or_else(|e| panic!("{}: {e}", a.herramienta));
            let json = hecha.pedido.to_string();
            assert!(
                pixpin_shell::mensajero::ACCIONES.contains(&a.accion),
                "{}: la ventana no conoce «{}»",
                a.herramienta,
                a.accion
            );
            crate::pedidos::leer(&json).unwrap_or_else(|e| panic!("{}: {json}: {e:?}", a.herramienta));
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
        let borrar = validar(por_nombre("borrar_lista").unwrap(), &ejemplo(por_nombre("borrar_lista").unwrap())).unwrap();
        assert!(hacer(&borrar, &no, &mandar).unwrap_err().contains("dijo que no"));
        assert!(mandados.lock().unwrap().is_empty(), "salio sin permiso");
        assert!(hacer(&borrar, &si, &mandar).is_ok());
        assert_eq!(mandados.lock().unwrap().len(), 1);
        // Lo que no borra no pregunta.
        let preguntado = Arc::new(Mutex::new(false));
        let p = preguntado.clone();
        let espia: Confirmar = Box::new(move |_| {
            *p.lock().unwrap() = true;
            false
        });
        let tarea = validar(por_nombre("anadir_tarea").unwrap(), &ejemplo(por_nombre("anadir_tarea").unwrap())).unwrap();
        let r = hacer(&tarea, &espia, &mandar).unwrap();
        assert!(r.starts_with("hecho: anadir_tarea"), "{r}");
        assert!(!*preguntado.lock().unwrap());
        // Si PixPin no contesta, se dice.
        let sordo: Mandar = Box::new(|_| 0);
        assert!(hacer(&tarea, &si, &sordo).unwrap_err().contains("no contesto"));
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
