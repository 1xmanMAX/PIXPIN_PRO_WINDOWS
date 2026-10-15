//! **El lector y editor de notas Markdown** (8-oct-2026): el que se abre
//! ahora al tocar una nota o un `.md`.
//!
//! El usuario: el de antes era «lento, poco fluido, no confiable y
//! lagueado», y pidio rehacerlo con **md-reader** como guia. Es una pagina
//! (`herramientas/lector-md`, compilada a `recursos/lector-md.html` y metida
//! en el ejecutable) dentro de una ventana normal de Windows con WebView2
//! (`pixpin_web::pagina`):
//!
//! - **Leer** es md-reader: su motor (markdown-it con sus complementos),
//!   su tema, su indice lateral, codigo coloreado, formulas, avisos, notas
//!   al pie, fotos que se amplian.
//! - **Escribir** es CodeMirror: un editor de texto de verdad, con el
//!   formato a la vista, casillas que se marcan, fotos bajo su renglon,
//!   Ctrl+F, deshacer y el corrector de Windows.
//! - **Lado a lado**, las dos.
//!
//! Lo que se guarda es el texto tal cual sale del editor: nada se reescribe
//! por detras, que era lo que hacia poco fiable al de antes (un `RichEdit`
//! que se volvia a pasar a Markdown en cada guardado). Se guarda solo,
//! a los 0,7 s de dejar de escribir, en el mismo sitio que antes
//! ([`super::guardar`]); lo que llega de otro aparato aparece solo si aqui
//! no hay nada sin guardar.
//!
//! El editor de antes sigue a mano («⋯ → Abrir en el editor anterior») para
//! lo que solo hace el (los comentarios al margen, meter mensajes del
//! chat), y se usa solo si este equipo no tiene WebView2.
//!
//! # Lo que se dicen la pagina y PixPin
//!
//! Textos JSON con un `tipo`. De PixPin a la pagina: `abrir` (el texto, el
//! idioma, las preferencias), `guardado`, `adjuntado`, `texto_externo` y
//! `cerrando`. De la pagina a PixPin: [`DeLaPagina`].

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use serde::Deserialize;
use serde_json::json;

use pixpin_store::{Catalogo, Ubicacion};
use pixpin_web::pagina::{OpcionesPagina, Pagina, Recurso, Suceso};

use super::{Destino, adjuntos, guardar};

/// La pagina, construida con `npm run build` en `herramientas/lector-md`.
const PAGINA: &[u8] = include_bytes!("../../recursos/lector-md.html");

/// Cada cuanto se mira si la nota cambio por fuera.
const MIRAR_FUERA: Duration = Duration::from_secs(2);

/// Lo que puede mandar la pagina.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum DeLaPagina {
    Listo,
    Guardar { texto: String, version: u64 },
    /// La letra y el tamano (los de `vista-de-notas.txt`, como antes).
    Vista { cuerpo: String, titulos: String, px: u32, solo_esta: bool },
    Titulo { texto: String },
    AbrirEnlace { url: String },
    AbrirPixpin { ruta: String },
    Adjuntar { id: u64, nombre: String, datos: String },
    /// Elegir ficheros con el dialogo de Windows: `imagen`, `documento`
    /// o `audio`.
    Elegir { clase: String },
    /// La lista de hojas para «Pagina de un proyecto» / «Enlace a una hoja».
    ElegirHoja { viva: bool },
    InsertarHoja { clave: String, viva: bool },
    /// La lista de mensajes del chat para «Del chat».
    ElegirMensaje,
    InsertarMensaje { clave: String },
    GuardarCopia { texto: String, titulo: String },
    ExportarWord { texto: String, titulo: String },
    Ventana { accion: String, #[serde(default)] borde: String },
    /// Los comentarios: cada pedido trae el texto de ahora (para anclar) y
    /// se contesta con la lista entera.
    ComentariosPedir { texto: String },
    ComentarioNuevo { texto: String, desde: usize, hasta: usize, cuerpo: String },
    ComentarioResponder { texto: String, hilo: String, cuerpo: String },
    ComentarioEditar { texto: String, id: String, cuerpo: String },
    ComentarioBorrar { texto: String, id: String },
    ComentarioResolver { texto: String, hilo: String, si: bool },
    Compartir,
    Cerrar,
    CerrarListo,
    EditorAnterior,
}

/// Como acabo.
#[derive(Debug, PartialEq, Eq)]
pub enum Salida {
    Cerrada,
    /// Pidio el editor de antes: quien llama lo abre con la misma nota.
    EditorAnterior,
}

/// Lo que se le pasa a [`correr`] de la ventana de notas.
pub struct Contexto<'a> {
    pub textos: &'a Catalogo,
    pub idioma: pixpin_store::Idioma,
    pub ubicacion: &'a Ubicacion,
    pub aparato: &'a str,
    pub actual: Rc<RefCell<Destino>>,
    pub texto: String,
    pub colocacion: Option<pixpin_shell::colocacion::Colocacion>,
    /// La ventana existe (para `grupos_ventanas` y las notas abiertas).
    pub al_nacer: &'a mut dyn FnMut(isize),
    /// Una nota nueva paso a ser su mensaje al guardarse.
    pub al_cambiar_destino: &'a mut dyn FnMut(&Destino, &Destino),
}

/// Si una direccion se puede abrir fuera: solo paginas y correo, nunca un
/// programa ni un fichero.
pub fn es_enlace_de_fuera(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    (u.starts_with("https://") || u.starts_with("http://") || u.starts_with("mailto:"))
        && !u.starts_with(pixpin_web::pagina::ORIGEN)
}

/// El tipo de un fichero que se le sirve a la pagina.
pub fn tipo_de(ruta: &Path) -> &'static str {
    match ruta
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("svg") => "image/svg+xml",
        Some("avif") => "image/avif",
        Some("ico") => "image/x-icon",
        Some("mp3") => "audio/mpeg",
        Some("m4a" | "aac") => "audio/mp4",
        Some("ogg" | "oga" | "opus") => "audio/ogg",
        Some("wav") => "audio/wav",
        Some("flac") => "audio/flac",
        Some("mp4" | "m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mov") => "video/quicktime",
        _ => "application/octet-stream",
    }
}

/// El renglon que entra en la nota por un fichero adjuntado: `![nombre](ruta)`
/// para todo (el movil decide por la extension si es foto, audio, video o
/// documento, `Markdown.claseDeMedio`).
pub fn renglon_de(nombre: &str, ruta: &str) -> String {
    let alt: String = nombre
        .chars()
        .filter(|c| !matches!(c, '[' | ']' | '\n' | '\r'))
        .collect();
    format!("![{}]({ruta})", alt.trim())
}

/// La huella de donde vive la nota: si cambia, alguien la toco.
fn huella(raiz: &Path, destino: &Destino) -> Option<(SystemTime, u64)> {
    let fecha = |p: &Path| std::fs::metadata(p).ok().and_then(|m| m.modified().ok());
    match destino {
        Destino::Fichero { ruta } => {
            let m = std::fs::metadata(ruta).ok()?;
            Some((m.modified().ok()?, m.len()))
        }
        Destino::Mensaje { proyecto, .. } | Destino::Nueva { proyecto } => {
            let carpeta = pixpin_proyecto::almacen::carpeta(raiz, proyecto);
            let mut mas = fecha(&carpeta)?;
            let mut n = 0u64;
            for e in std::fs::read_dir(&carpeta).ok()?.flatten() {
                if let Ok(m) = e.metadata()
                    && m.is_file()
                {
                    n += m.len();
                    if let Ok(f) = m.modified() {
                        mas = mas.max(f);
                    }
                }
            }
            Some((mas, n))
        }
    }
}

/// La letra y el tamano de esta nota (la suya o la general).
fn leer_vista(raiz: &Path, clave: &str) -> (pixpin_notas::vista::Vista, bool) {
    let contenido = std::fs::read_to_string(raiz.join(super::FICHERO_VISTA)).unwrap_or_default();
    pixpin_notas::vista::leer(&contenido, Some(clave))
}

fn guardar_vista(raiz: &Path, clave: &str, v: &pixpin_notas::vista::Vista, solo_esta: bool) {
    let ruta = raiz.join(super::FICHERO_VISTA);
    let contenido = std::fs::read_to_string(&ruta).unwrap_or_default();
    let nuevo = if solo_esta {
        pixpin_notas::vista::escribir(&contenido, Some(clave), v)
    } else {
        // La general; y si esta nota tenia la suya, se olvida.
        let sin = pixpin_notas::vista::olvidar(&contenido, clave);
        pixpin_notas::vista::escribir(&sin, None, v)
    };
    let _ = std::fs::write(ruta, nuevo);
}

/// Un nombre de fichero a partir del titulo (sin `<>:"/\|?*`).
fn nombre_de_fichero(titulo: &str, reserva: &str) -> String {
    let n: String = titulo.chars().filter(|c| !"<>:\"/\\|?*".contains(*c) && !c.is_control()).collect();
    let n = n.trim().trim_end_matches('…').trim().to_string();
    if n.is_empty() { reserva.to_string() } else { n }
}

fn hwnd(h: isize) -> windows::Win32::Foundation::HWND {
    windows::Win32::Foundation::HWND(h as *mut _)
}

/// Abre la nota en el lector. `Err` si no se pudo (sin WebView2): quien
/// llama abre el editor de antes.
pub fn correr(cx: Contexto) -> anyhow::Result<Salida> {
    let raiz = cx.ubicacion.raiz().to_path_buf();
    let nombre = match &*cx.actual.borrow() {
        Destino::Fichero { ruta } => pixpin_docs::nombre(ruta),
        _ => String::new(),
    };
    let (r_servir, a_servir) = (raiz.clone(), cx.actual.clone());
    tracing::info!("abriendo el lector de notas");
    let pagina = Pagina::nueva(OpcionesPagina {
        titulo: titulo_de_ventana(&nombre, cx.textos),
        area: None,
        maximizada: false,
        fondo: (0x13, 0x14, 0x15),
        pagina: PAGINA,
        servir: Box::new(move |resto| servir(&r_servir, &a_servir.borrow(), resto)),
    })
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    tracing::info!(hwnd = pagina.hwnd(), "lector de notas abierto");
    // Donde estaba la ultima ventana de notas (`grupos_ventanas`).
    if let Some(c) = &cx.colocacion {
        pixpin_shell::colocacion::poner(pagina.hwnd(), c);
    }
    (cx.al_nacer)(pagina.hwnd());

    let mut ultimo_conocido = cx.texto.clone();
    let mut firma = huella(&raiz, &cx.actual.borrow());
    let mut mirado = Instant::now();
    let mut cerrando: Option<Instant> = None;
    let mut salida = Salida::Cerrada;
    let clave_vista = super::clave_de_vista(&cx.actual.borrow());
    let (vista, solo_esta) = leer_vista(&raiz, &clave_vista);
    let de_proyecto = !matches!(*cx.actual.borrow(), Destino::Fichero { .. });
    let abrir = json!({
        "tipo": "abrir",
        "idioma": if cx.idioma == pixpin_store::Idioma::Ingles { "en" } else { "es" },
        "texto": cx.texto,
        "nombre": nombre,
        "vista": {"cuerpo": vista.cuerpo, "titulos": vista.titulos, "px": vista.px, "soloEsta": solo_esta},
        "capacidades": {"hojas": de_proyecto, "medios": true},
    })
    .to_string();
    let mut medios: Option<super::incrustados::MediosDeLaNota> = None;
    let autor = pixpin_proyecto::identidad::Identidad::leer_o_crear(&raiz, "PC")
        .map(|i| i.yo.nombre)
        .unwrap_or_else(|_| "PC".into());
    let mut comentarios = super::comentarios_web::Comentarista::abrir(
        &raiz,
        &cx.actual.borrow(),
        pixpin_docs::md_comentarios::Quien {
            autor,
            aparato: cx.aparato.to_string(),
        },
    );

    'bucle: loop {
        for s in pagina.esperar(250) {
            match s {
                Suceso::Destruida => break 'bucle,
                Suceso::PideCerrar => {
                    if cerrando.is_none() {
                        cerrando = Some(Instant::now());
                        pagina.mandar(r#"{"tipo":"cerrando"}"#);
                    }
                }
                Suceso::Navegar(url) => {
                    if es_enlace_de_fuera(&url) {
                        let _ = pixpin_shell::abrir(Path::new(&url));
                    }
                }
                Suceso::Mensaje(m) => {
                    let Ok(pedido) = serde_json::from_str::<DeLaPagina>(&m) else {
                        tracing::warn!(%m, "mensaje del lector que no se entiende");
                        continue;
                    };
                    match pedido {
                        DeLaPagina::Listo => pagina.mandar(&abrir),
                        DeLaPagina::Guardar { texto, version } => {
                            let (ok, cambio) = guardar_texto(&raiz, &cx, &texto);
                            if let Some((antes, nuevo)) = cambio {
                                (cx.al_cambiar_destino)(&antes, &nuevo);
                                comentarios.mudar(&raiz, &nuevo);
                            }
                            if ok {
                                ultimo_conocido = texto;
                                firma = huella(&raiz, &cx.actual.borrow());
                            }
                            pagina.mandar(
                                &json!({"tipo": "guardado", "version": version, "ok": ok})
                                    .to_string(),
                            );
                        }
                        DeLaPagina::Vista { cuerpo, titulos, px, solo_esta } => {
                            let v = pixpin_notas::vista::Vista {
                                cuerpo,
                                titulos,
                                px: px.clamp(pixpin_notas::vista::TAMANO_MINIMO, pixpin_notas::vista::TAMANO_MAXIMO),
                            };
                            guardar_vista(&raiz, &super::clave_de_vista(&cx.actual.borrow()), &v, solo_esta);
                        }
                        DeLaPagina::Elegir { clase } => {
                            let h = hwnd(pagina.hwnd());
                            let rutas = if clase == "imagen" {
                                pixpin_shell::elegir::pedir_imagenes(h)
                            } else {
                                pixpin_shell::elegir::pedir_ficheros(h)
                            };
                            let destino = cx.actual.borrow().clone();
                            let mut renglones = Vec::new();
                            for r in rutas {
                                let ahora = pixpin_shell::entorno::ahora_utc_ms();
                                match adjuntos::adjuntar(&raiz, &destino, &r, ahora) {
                                    Ok(ruta) => renglones.push(renglon_de(&pixpin_docs::nombre(&r), &ruta)),
                                    Err(e) => tracing::warn!(?e, "no se pudo copiar junto a la nota"),
                                }
                            }
                            if !renglones.is_empty() {
                                pagina.mandar(&json!({"tipo": "insertar", "bloque": true, "texto": renglones.join("\n")}).to_string());
                            }
                        }
                        DeLaPagina::ElegirHoja { viva } => {
                            let grupos = super::paginas_vivas::grupos(&raiz, &cx.actual.borrow());
                            let lista: Vec<serde_json::Value> = grupos
                                .iter()
                                .map(|g| json!({"proyecto": g.proyecto, "propio": g.propio, "hojas": g.hojas.iter().map(|h| json!({"clave": h.clave, "nombre": h.nombre})).collect::<Vec<_>>()}))
                                .collect();
                            pagina.mandar(&json!({"tipo": "hojas", "viva": viva, "grupos": lista}).to_string());
                        }
                        DeLaPagina::InsertarHoja { clave, viva } => {
                            let destino = cx.actual.borrow().clone();
                            if let Some(t) = super::paginas_vivas::insertar(&raiz, &destino, &clave, !viva) {
                                pagina.mandar(&json!({"tipo": "insertar", "bloque": true, "texto": t}).to_string());
                            }
                        }
                        DeLaPagina::ElegirMensaje => {
                            use pixpin_notas::incrustados::Medios;
                            let m = medios.get_or_insert_with(|| super::incrustados::MediosDeLaNota::nuevo(cx.idioma, cx.ubicacion, &cx.actual));
                            let lista: Vec<serde_json::Value> = m.mensajes().into_iter().map(|e| json!({"clave": e.clave, "rotulo": e.rotulo})).collect();
                            pagina.mandar(&json!({"tipo": "mensajes", "lista": lista}).to_string());
                        }
                        DeLaPagina::InsertarMensaje { clave } => {
                            use pixpin_notas::incrustados::Medios;
                            let m = medios.get_or_insert_with(|| super::incrustados::MediosDeLaNota::nuevo(cx.idioma, cx.ubicacion, &cx.actual));
                            if let Some(t) = m.insertar_mensaje(&clave) {
                                pagina.mandar(&json!({"tipo": "insertar", "bloque": true, "texto": t}).to_string());
                            }
                        }
                        DeLaPagina::GuardarCopia { texto, titulo } => {
                            let nombre_f = nombre_de_fichero(&titulo, &cx.textos.t("nota-md-nueva"));
                            if let Some(r) = pixpin_shell::guardar::pedir_ruta_para(hwnd(pagina.hwnd()), &nombre_f, &cx.textos.t("nota-md-tipo"), "md") {
                                if let Err(e) = std::fs::write(&r, texto) {
                                    tracing::warn!(?e, "no se pudo guardar la copia .md");
                                }
                            }
                        }
                        DeLaPagina::ExportarWord { texto, titulo } => {
                            let (v, _) = leer_vista(&raiz, &super::clave_de_vista(&cx.actual.borrow()));
                            let destino = cx.actual.borrow().clone();
                            let r2 = raiz.clone();
                            let resolver = move |ruta: &str| adjuntos::resolver(&r2, &destino, ruta);
                            let ahora = pixpin_shell::entorno::ahora_utc_ms();
                            if let Some((bytes, sugerido)) = pixpin_notas::word::exportar(&texto, &cx.textos.t("nota-md-nueva"), &v, None, &resolver, cx.aparato, ahora) {
                                let nombre_f = nombre_de_fichero(if sugerido.is_empty() { &titulo } else { &sugerido }, "Nota");
                                if let Some(r) = pixpin_shell::guardar::pedir_ruta_para(hwnd(pagina.hwnd()), &nombre_f, &cx.textos.t("nota-md-tipo-word"), "docx") {
                                    if let Err(e) = std::fs::write(&r, bytes) {
                                        tracing::warn!(?e, "no se pudo guardar el Word");
                                    }
                                }
                            }
                        }
                        DeLaPagina::ComentariosPedir { texto } => {
                            pagina.mandar(&comentarios.json(&texto).to_string());
                        }
                        DeLaPagina::ComentarioNuevo { texto, desde, hasta, cuerpo } => {
                            let id = comentarios.nuevo(&texto, desde, hasta, &cuerpo);
                            let mut j = comentarios.json(&texto);
                            j["nuevo"] = json!(id);
                            pagina.mandar(&j.to_string());
                        }
                        DeLaPagina::ComentarioResponder { texto, hilo, cuerpo } => {
                            comentarios.responder(&hilo, &cuerpo);
                            pagina.mandar(&comentarios.json(&texto).to_string());
                        }
                        DeLaPagina::ComentarioEditar { texto, id, cuerpo } => {
                            comentarios.editar(&id, &cuerpo);
                            pagina.mandar(&comentarios.json(&texto).to_string());
                        }
                        DeLaPagina::ComentarioBorrar { texto, id } => {
                            comentarios.borrar(&id);
                            pagina.mandar(&comentarios.json(&texto).to_string());
                        }
                        DeLaPagina::ComentarioResolver { texto, hilo, si } => {
                            comentarios.resolver(&hilo, si);
                            pagina.mandar(&comentarios.json(&texto).to_string());
                        }
                        DeLaPagina::Ventana { accion, borde } => match accion.as_str() {
                            "arrastrar" => pagina.arrastrar(),
                            "estirar" => pagina.estirar(&borde),
                            "minimizar" => pagina.minimizar(),
                            "maximizar" => pagina.alternar_maximizar(),
                            "pantalla" => {
                                let si = pagina.alternar_pantalla();
                                pagina.mandar(&json!({"tipo": "pantalla", "si": si}).to_string());
                            }
                            _ => {}
                        },
                        DeLaPagina::Titulo { texto } => {
                            let t = if texto.trim().is_empty() {
                                nombre.clone()
                            } else {
                                texto
                            };
                            pagina.poner_titulo(&titulo_de_ventana(&t, cx.textos));
                        }
                        DeLaPagina::AbrirEnlace { url } => {
                            if es_enlace_de_fuera(&url) {
                                let _ = pixpin_shell::abrir(Path::new(&url));
                            }
                        }
                        DeLaPagina::AbrirPixpin { ruta } => {
                            let destino = cx.actual.borrow().clone();
                            super::paginas_vivas::abrir(cx.idioma, cx.ubicacion, &destino, &ruta);
                        }
                        DeLaPagina::Adjuntar { id, nombre, datos } => {
                            let md = adjuntar(&raiz, &cx.actual.borrow(), &nombre, &datos);
                            pagina.mandar(
                                &json!({"tipo": "adjuntado", "id": id, "md": md, "error": md.is_none()})
                                    .to_string(),
                            );
                        }
                        DeLaPagina::Compartir => {
                            let destino = cx.actual.borrow().clone();
                            super::compartir(cx.idioma, cx.ubicacion, &destino, cx.aparato);
                        }
                        DeLaPagina::Cerrar => {
                            if cerrando.is_none() {
                                cerrando = Some(Instant::now());
                                pagina.mandar(r#"{"tipo":"cerrando"}"#);
                            }
                        }
                        DeLaPagina::CerrarListo => break 'bucle,
                        DeLaPagina::EditorAnterior => {
                            salida = Salida::EditorAnterior;
                            cerrando = Some(Instant::now());
                            pagina.mandar(r#"{"tipo":"cerrando"}"#);
                        }
                    }
                }
            }
        }
        // Si la pagina no contesta al cerrar (colgada), se cierra igual: lo
        // ultimo se guardo hace como mucho 0,7 s.
        if cerrando.is_some_and(|c| c.elapsed() > Duration::from_secs(3)) {
            tracing::warn!("el lector no contesto al cerrar; se cierra igual");
            break;
        }
        if mirado.elapsed() >= MIRAR_FUERA {
            mirado = Instant::now();
            let ahora = huella(&raiz, &cx.actual.borrow());
            if ahora != firma {
                firma = ahora;
                if let Some(t) = guardar::leer_texto(&raiz, &cx.actual.borrow())
                    && t != ultimo_conocido
                {
                    ultimo_conocido = t.clone();
                    pagina.mandar(&json!({"tipo": "texto_externo", "texto": t}).to_string());
                }
            }
        }
    }
    drop(pagina);
    Ok(salida)
}

fn titulo_de_ventana(titulo: &str, t: &Catalogo) -> String {
    if titulo.trim().is_empty() {
        t.t("nota-md-nueva")
    } else {
        format!("{} — PixPin", titulo.trim())
    }
}

/// Guarda y dice si pudo, y si la nota cambio de destino (una nueva pasa a
/// ser su mensaje).
fn guardar_texto(raiz: &Path, cx: &Contexto, texto: &str) -> (bool, Option<(Destino, Destino)>) {
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let antes = cx.actual.borrow().clone();
    match guardar::guardar(raiz, &antes, texto, cx.aparato, ahora) {
        Ok(nuevo) => {
            let cambio = (nuevo != antes).then(|| (antes, nuevo.clone()));
            *cx.actual.borrow_mut() = nuevo;
            if !matches!(*cx.actual.borrow(), Destino::Fichero { .. }) {
                crate::ventana_chat::refrescar();
            }
            (true, cambio)
        }
        Err(e) => {
            tracing::error!(?e, "no se pudo guardar la nota");
            (false, None)
        }
    }
}

/// Lo que la pagina pide bajo `https://pixpin.nota/`: las fotos, audios y
/// videos de la nota, por la misma ruta con que van en el Markdown.
fn servir(raiz: &Path, destino: &Destino, resto: &str) -> Option<Recurso> {
    if !resto.starts_with("archivo?") {
        return None;
    }
    let ruta = pixpin_web::pagina::parametro(resto, "r")?;
    let fichero: PathBuf = adjuntos::resolver(raiz, destino, &ruta)?;
    let bytes = std::fs::read(&fichero).ok()?;
    Some(Recurso {
        tipo: tipo_de(&fichero).to_string(),
        bytes,
    })
}

/// Un fichero pegado o soltado en la nota: se copia junto a ella como las
/// fotos (`adjuntos::adjuntar`) y se devuelve su renglon.
fn adjuntar(raiz: &Path, destino: &Destino, nombre: &str, datos: &str) -> Option<String> {
    let bytes = pixpin_web::base64::descodificar(datos)?;
    let limpio = adjuntos::nombre_limpio(if nombre.trim().is_empty() {
        "imagen.png"
    } else {
        nombre
    });
    let carpeta = std::env::temp_dir().join(format!("pixpin-pegado-{}", std::process::id()));
    std::fs::create_dir_all(&carpeta).ok()?;
    let temporal = carpeta.join(&limpio);
    std::fs::write(&temporal, &bytes).ok()?;
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let r = adjuntos::adjuntar(raiz, destino, &temporal, ahora);
    let _ = std::fs::remove_file(&temporal);
    match r {
        Ok(ruta) => Some(renglon_de(nombre, &ruta)),
        Err(e) => {
            tracing::warn!(?e, "no se pudo copiar el fichero junto a la nota");
            None
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_mensajes_de_la_pagina_se_entienden() {
        let m: DeLaPagina =
            serde_json::from_str(r##"{"tipo":"guardar","texto":"# Hola","version":3}"##).unwrap();
        assert_eq!(
            m,
            DeLaPagina::Guardar {
                texto: "# Hola".into(),
                version: 3
            }
        );
        let v: DeLaPagina = serde_json::from_str(
            r#"{"tipo":"vista","cuerpo":"Nunito","titulos":"Fraunces","px":18,"solo_esta":true}"#,
        )
        .unwrap();
        assert!(matches!(v, DeLaPagina::Vista { px: 18, solo_esta: true, .. }));
        let w: DeLaPagina = serde_json::from_str(r#"{"tipo":"ventana","accion":"minimizar"}"#).unwrap();
        assert!(matches!(w, DeLaPagina::Ventana { ref borde, .. } if borde.is_empty()));
        assert_eq!(
            serde_json::from_str::<DeLaPagina>(r#"{"tipo":"cerrar_listo"}"#).unwrap(),
            DeLaPagina::CerrarListo
        );
        // Caso negativo: un tipo que no existe no se toma por otro.
        assert!(serde_json::from_str::<DeLaPagina>(r#"{"tipo":"borrar_todo"}"#).is_err());
    }

    #[test]
    fn solo_se_abren_fuera_paginas_y_correo() {
        assert!(es_enlace_de_fuera("https://example.com/a"));
        assert!(es_enlace_de_fuera("mailto:a@b.c"));
        // Casos negativos: un programa, un fichero o la propia pagina.
        assert!(!es_enlace_de_fuera("file:///C:/Windows/notepad.exe"));
        assert!(!es_enlace_de_fuera("C:\\Windows\\System32\\cmd.exe"));
        assert!(!es_enlace_de_fuera("https://pixpin.nota/index.html"));
    }

    #[test]
    fn el_renglon_adjuntado_es_un_medio_del_movil() {
        assert_eq!(
            renglon_de("plano [v2].pdf", "pixpin:files/x/notas/1-plano_v2_.pdf"),
            "![plano v2.pdf](pixpin:files/x/notas/1-plano_v2_.pdf)"
        );
        assert_eq!(tipo_de(Path::new("a/voz.M4A")), "audio/mp4");
        assert_eq!(tipo_de(Path::new("a/b.xyz")), "application/octet-stream");
    }

    #[test]
    fn la_pagina_va_dentro_y_entera() {
        let t = std::str::from_utf8(PAGINA).unwrap();
        assert!(t.starts_with("<!doctype html>"));
        assert!(t.contains("pastilla-titulo") && t.trim_end().ends_with("</html>"));
    }
}

#[cfg(test)]
mod a_mano {
    /// Abre de verdad el lector con un `.md` de prueba, como lo abre PixPin,
    /// y lo cierra a los pocos segundos. Abre una ventana: solo a mano.
    #[test]
    #[ignore = "abre una ventana"]
    fn abre_un_md_de_verdad() {
        let d = std::env::temp_dir().join(format!("pixpin-lector-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let md = d.join("prueba.md");
        std::fs::write(&md, "# Prueba\n\nHola **mundo**\n\n- [ ] una\n").unwrap();
        let ub = pixpin_store::Ubicacion::Portable { raiz: d.clone() };
        let _ = tracing_subscriber::fmt().with_writer(std::io::stderr).try_init();
        let hilo = std::thread::spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = pixpin_store::Catalogo::nuevo(pixpin_store::Idioma::Espanol);
            eprintln!("correr");
            super::super::correr(&textos, pixpin_store::Idioma::Espanol, &ub, super::Destino::Fichero { ruta: md });
            eprintln!("fin de correr");
        });
        std::thread::sleep(std::time::Duration::from_secs(8));
        eprintln!("hilo vivo: {}", !hilo.is_finished());
    }
}
