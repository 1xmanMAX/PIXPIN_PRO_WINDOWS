//! **El lector y editor de notas Markdown** (8-oct-2026): el que se abre
//! ahora al tocar una nota o un `.md`.
//!
//! El usuario: el de antes era «lento, poco fluido, no confiable y
//! lagueado», y pidio rehacerlo con **md-reader** como guia. Es una pagina
//! (`herramientas/lector-md`, compilada a `recursos/lector-md.html` y metida
//! en el ejecutable) con WebView2 (`pixpin_web::pagina`).
//!
//! **Como un pin, con la barra de los demas visores** (17-oct-2026, el
//! usuario: «modificar el visor de MD y de HTML para que tenga una UI igual a
//! la de los demas visores en formato de pin»). La ventana es la de los
//! lectores como pin (`lector_pin`, la del PDF, el Excel, el pin de Office):
//! sale en seguida con el fondo de la nota y la pagina llega dentro; se
//! estira por los bordes, la barra va fuera (asa, nombre con el recuadro
//! `md`, luna/sol, pantalla completa, pin, cerrar) y lo propio de una nota va
//! en sus botones ([`botones`]), que la pagina hace por `orden`. La cabecera
//! propia de la pagina se esconde (`enPin`).
//!
//! Lo que tiene la pagina:
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

use pixpin_render::icono::material;
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Ubicacion};
use pixpin_web::pagina::{OpcionesPagina, Pagina, Recurso, Suceso};

use crate::lector_pin::{AccionPin, BotonLector, PinLector};

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
    /// Lo que encienden los botones de la barra del pin: viendo la nota
    /// (sin escribir) y el panel de los comentarios abierto.
    Estado {
        leyendo: bool,
        comentarios: bool,
    },
    Guardar {
        texto: String,
        version: u64,
    },
    /// La letra y el tamano (los de `vista-de-notas.txt`, como antes).
    Vista {
        cuerpo: String,
        titulos: String,
        px: u32,
        solo_esta: bool,
    },
    Titulo {
        texto: String,
    },
    AbrirEnlace {
        url: String,
    },
    AbrirPixpin {
        ruta: String,
    },
    Adjuntar {
        id: u64,
        nombre: String,
        datos: String,
    },
    /// Elegir ficheros con el dialogo de Windows: `imagen`, `documento`
    /// o `audio`.
    Elegir {
        clase: String,
    },
    /// La lista de hojas para «Pagina de un proyecto» / «Enlace a una hoja».
    ElegirHoja {
        viva: bool,
    },
    InsertarHoja {
        clave: String,
        viva: bool,
    },
    /// La lista de mensajes del chat para «Del chat».
    ElegirMensaje,
    InsertarMensaje {
        clave: String,
    },
    GuardarCopia {
        texto: String,
        titulo: String,
    },
    ExportarWord {
        texto: String,
        titulo: String,
    },
    Ventana {
        accion: String,
        #[serde(default)]
        borde: String,
    },
    /// Los comentarios: cada pedido trae el texto de ahora (para anclar) y
    /// se contesta con la lista entera.
    ComentariosPedir {
        texto: String,
    },
    ComentarioNuevo {
        texto: String,
        desde: usize,
        hasta: usize,
        cuerpo: String,
    },
    ComentarioResponder {
        texto: String,
        hilo: String,
        cuerpo: String,
    },
    ComentarioEditar {
        texto: String,
        id: String,
        cuerpo: String,
    },
    ComentarioBorrar {
        texto: String,
        id: String,
    },
    ComentarioResolver {
        texto: String,
        hilo: String,
        si: bool,
    },
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
    let n: String = titulo
        .chars()
        .filter(|c| !"<>:\"/\\|?*".contains(*c) && !c.is_control())
        .collect();
    let n = n.trim().trim_end_matches('…').trim().to_string();
    if n.is_empty() { reserva.to_string() } else { n }
}

fn hwnd(h: isize) -> windows::Win32::Foundation::HWND {
    windows::Win32::Foundation::HWND(h as *mut _)
}

/// Abre la nota en el lector. `Err` si no se pudo (sin WebView2): quien
/// llama abre el editor de antes.
pub fn correr(cx: Contexto) -> anyhow::Result<Salida> {
    if !pixpin_web::VisorHtml::hay_runtime() {
        anyhow::bail!("este equipo no tiene el runtime de WebView2");
    }
    let inicio = Instant::now();
    let raiz = cx.ubicacion.raiz().to_path_buf();
    let nombre = match &*cx.actual.borrow() {
        Destino::Fichero { ruta } => pixpin_docs::nombre(ruta),
        _ => String::new(),
    };
    let mut en_la_barra = nombre_en_la_barra(
        &cx.actual.borrow(),
        &pixpin_docs::md_vivo::titulo(&cx.texto),
        cx.textos,
    );
    tracing::info!("abriendo el lector de notas");

    // **La ventana es la de los visores como pin** (`lector_pin`): sale ya,
    // con el fondo de la nota, y la pagina llega dentro en cuanto el
    // WebView2 arranca (antes la ventana esperaba a el, ~0,5-1,3 s).
    let e = pixpin_capture::enumerar_monitores()
        .ok()
        .and_then(|m| m.principal().map(|p| p.escala_por_cien as f32 / 100.0))
        .unwrap_or(1.0);
    let (area_pin, _) = crate::lector_pin::area_inicial();
    let mut pin = PinLector::nuevo(area_pin, e).con_extension("md");
    let mut ventana = VentanaOverlay::nueva_llana(pin.area, &titulo_de_ventana(&nombre, cx.textos))
        .map_err(|e| anyhow::anyhow!("no se pudo abrir la ventana de la nota: {e:?}"))?;
    let hwnd_pin = ventana.handle().0 as isize;
    pin.al_nacer(&ventana);
    ventana.mostrar();
    // Una nota se abre para escribir: el teclado, a ella.
    ventana.enfocar();
    // Donde estaba la ultima ventana de notas (`grupos_ventanas`).
    if let Some(c) = &cx.colocacion {
        pixpin_shell::colocacion::poner(hwnd_pin, c);
        pin.seguir_ventana(&mut ventana);
    }
    pixpin_web::pintar_fondo(hwnd_pin, fondo(pin.oscuro()));
    tracing::info!(
        ms = inicio.elapsed().as_millis() as u64,
        "lector de notas: a la vista"
    );
    (cx.al_nacer)(hwnd_pin);

    // La barra: sin GPU no la hay, pero la nota se edita igual.
    let recursos = crate::overlay::Recursos::nuevos()
        .map_err(|e| tracing::warn!(?e, "nota sin barra"))
        .ok();
    let motor = recursos.as_ref().map(|r| r.motor());
    if let (Some(r), Some(m)) = (&recursos, &motor) {
        pin.crear_barra(m, &r.d3d(), &ventana);
    }
    let (mut leyendo, mut con_comentarios) = (false, false);
    let mut propios = botones(leyendo, con_comentarios);
    pin.poner_pistas(pistas(cx.textos));
    // Pruebas (solo en debug): siempre encima, para fotografiar la pantalla
    // sin que otra ventana lo tape.
    #[cfg(debug_assertions)]
    if std::env::var_os("PIXPIN_PRUEBA_ENCIMA").is_some() {
        pin.alternar_fijado(&ventana);
    }
    if let Some(m) = &motor {
        pin.actualizar(m, &en_la_barra, &propios);
    }

    let (r_servir, a_servir) = (raiz.clone(), cx.actual.clone());
    let pagina = Pagina::nueva(OpcionesPagina {
        titulo: titulo_de_ventana(&nombre, cx.textos),
        area: Some(pin.area_de_la_vista()),
        maximizada: false,
        fondo: fondo(pin.oscuro()),
        pagina: PAGINA,
        servir: Box::new(move |resto| servir(&r_servir, &a_servir.borrow(), resto)),
        padre: Some(hwnd_pin),
        oscuro: Some(pin.oscuro()),
    })
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    tracing::info!(
        ms = inicio.elapsed().as_millis() as u64,
        "lector de notas: pagina pedida"
    );
    // **El riel de los visores** (el marcador, el lapiz de tinta y el marco),
    // en sus ventanas por encima de la pagina. El lapiz de la barra sigue
    // siendo el de escribir el texto; el del riel es el de la tinta.
    let escala = (e * 100.0).round() as u32;
    let mut riel = match (&recursos, &motor) {
        (Some(r), Some(m)) => crate::visor_html::riel::RielDeLaVista::nuevo(
            Rc::clone(m),
            &r.d3d(),
            &ventana,
            crate::visor_html::riel::en_pantalla(pin.area, pin.area_de_la_vista()),
            escala,
            crate::visor_html::riel::Opciones {
                documento: documento_del_riel(&raiz, &cx.actual.borrow()),
                con_pagina: true,
                titulo: en_la_barra.clone(),
                // La G abre el ⋮ de la barra, como en los demas visores.
                menu: true,
                // La tinta, a la medida de la columna con que se anoto
                // (`capa_web::Maqueta`): el texto de una nota se recoloca.
                maqueta: true,
            },
            cx.textos,
        ),
        _ => None,
    };
    let mut area = pin.area;

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
        // La cabecera propia no: la barra es la comun de los visores.
        "enPin": true,
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
        // Atiende la cola del hilo: la pagina y, por `pixpin_shell`, la
        // ventana del pin y su barra.
        let tope = if pin.quiere_reloj() { 50 } else { 250 };
        let tope = riel
            .as_ref()
            .and_then(|r| r.tope_ms())
            .map_or(tope, |t| t.min(tope));
        let sucesos = pagina.esperar(tope);
        let mut teclas: Vec<u32> = Vec::new();
        for (h, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if let Some(r) = riel.as_mut()
                && r.es_suya(h)
            {
                match r.evento(h, &evento, Some(&pagina), cx.textos) {
                    crate::visor_html::riel::Pide::Teclado => pagina.enfocar(),
                    crate::visor_html::riel::Pide::Tecla(vk) => teclas.push(vk),
                    crate::visor_html::riel::Pide::Nada => {}
                }
                continue;
            }
            if pin.es_de_la_barra(h) {
                let (accion, _) = pin.evento_barra(&evento, &mut ventana);
                match accion {
                    Some(AccionPin::Propio(id)) => {
                        if let Some(que) = orden_de(id) {
                            pagina.mandar(&json!({"tipo": "orden", "que": que}).to_string());
                            pagina.enfocar();
                        }
                    }
                    // El nombre: cambiar el titulo de la nota.
                    Some(AccionPin::Nombre) => {
                        pagina.mandar(r#"{"tipo":"orden","que":"titulo"}"#);
                        pagina.enfocar();
                    }
                    Some(AccionPin::PantallaCompleta) => {
                        alternar_pantalla(&mut pin, &mut ventana, &pagina)
                    }
                    Some(AccionPin::Fijar) => pin.alternar_fijado(&ventana),
                    Some(AccionPin::Cerrar) => pedir_cierre(&mut cerrando, &pagina),
                    None => {}
                }
                continue;
            }
            if h.0 as isize != hwnd_pin || pin.evento_ventana(&evento, &mut ventana).consumido {
                continue;
            }
            match evento {
                EventoOverlay::Cerrar => pedir_cierre(&mut cerrando, &pagina),
                // El teclado lo tenia la ventana (se toco su borde): la
                // tecla cuenta y la siguiente va ya a la pagina. Las de los
                // visores (I, A, M, Ctrl+M, G), al riel.
                EventoOverlay::Tecla {
                    vk,
                    ctrl,
                    alt: false,
                    ..
                } if riel.is_some() => {
                    match riel.as_mut().and_then(|r| r.tecla(vk, ctrl, Some(&pagina))) {
                        Some(crate::visor_html::riel::Pide::Tecla(v)) => teclas.push(v),
                        Some(crate::visor_html::riel::Pide::Teclado) => pagina.enfocar(),
                        Some(crate::visor_html::riel::Pide::Nada) => {}
                        None => {
                            teclas.push(vk);
                            pagina.enfocar();
                        }
                    }
                }
                EventoOverlay::Tecla { vk, .. } => {
                    teclas.push(vk);
                    pagina.enfocar();
                }
                EventoOverlay::Pintar | EventoOverlay::CambioDpi => {
                    pixpin_web::pintar_fondo(hwnd_pin, fondo(pin.oscuro()))
                }
                _ => {}
            }
        }
        // Esc y F11 con el teclado en la ventana, como en los demas visores.
        // (Dentro de la pagina Esc no cierra: es un editor; F11 y Esc a
        // pantalla completa llegan como `ventana`.)
        for vk in teclas {
            // G: el menu ⋮ de la barra (lo abre la pagina).
            if vk == VK_G {
                mandar_mas(&pagina);
                continue;
            }
            // Esc con la tira de los marcadores abierta: solo la cierra.
            if vk == 0x1B && riel.as_mut().is_some_and(|r| r.soltar_esc()) {
                continue;
            }
            match crate::lector_pin::tecla(vk, pin.pantalla_completa) {
                Some(true) => pedir_cierre(&mut cerrando, &pagina),
                Some(false) => alternar_pantalla(&mut pin, &mut ventana, &pagina),
                None => {}
            }
        }
        // Movida o estirada desde fuera (AltSnap, Win+flecha): el pin la sigue.
        pin.seguir_ventana(&mut ventana);
        if pin.area != area {
            area = pin.area;
            pagina.mover(pin.area_de_la_vista());
            pixpin_web::pintar_fondo(hwnd_pin, fondo(pin.oscuro()));
        }
        if let Some(m) = &motor
            && pin.actualizar(m, &en_la_barra, &propios)
        {
            // La luna/sol (la de esta barra o la de otro visor abierto).
            pagina.poner_oscuro(pin.oscuro(), fondo(pin.oscuro()));
            pixpin_web::pintar_fondo(hwnd_pin, fondo(pin.oscuro()));
        }
        if let Some(r) = riel.as_mut() {
            r.seguir(
                crate::visor_html::riel::en_pantalla(pin.area, pin.area_de_la_vista()),
                escala,
                pin.fijado,
                pin.oscuro(),
            );
            r.tic(Some(&pagina));
        }
        for s in sucesos {
            match s {
                Suceso::Destruida => break 'bucle,
                Suceso::PideCerrar => pedir_cierre(&mut cerrando, &pagina),
                Suceso::Navegar(url) => {
                    if es_enlace_de_fuera(&url) {
                        let _ = pixpin_shell::abrir(Path::new(&url));
                    }
                }
                Suceso::Mensaje(m) => {
                    // Lo del riel (por donde va al desplazarse, las teclas de
                    // los visores pulsadas en la pagina), a el.
                    if let Some(pide) = riel.as_mut().and_then(|r| r.mensaje(&m, Some(&pagina))) {
                        match pide {
                            crate::visor_html::riel::Pide::Teclado => pagina.enfocar(),
                            crate::visor_html::riel::Pide::Tecla(VK_G) => mandar_mas(&pagina),
                            _ => {}
                        }
                        continue;
                    }
                    let Ok(pedido) = serde_json::from_str::<DeLaPagina>(&m) else {
                        tracing::warn!(%m, "mensaje del lector que no se entiende");
                        continue;
                    };
                    match pedido {
                        DeLaPagina::Listo => {
                            tracing::info!(
                                ms = inicio.elapsed().as_millis() as u64,
                                "lector de notas: pagina lista"
                            );
                            pagina.mandar(&abrir);
                        }
                        // Lo que encienden los botones de la barra.
                        DeLaPagina::Estado {
                            leyendo: l,
                            comentarios: c,
                        } => {
                            (leyendo, con_comentarios) = (l, c);
                            propios = botones(leyendo, con_comentarios);
                        }
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
                        DeLaPagina::Vista {
                            cuerpo,
                            titulos,
                            px,
                            solo_esta,
                        } => {
                            let v = pixpin_notas::vista::Vista {
                                cuerpo,
                                titulos,
                                px: px.clamp(
                                    pixpin_notas::vista::TAMANO_MINIMO,
                                    pixpin_notas::vista::TAMANO_MAXIMO,
                                ),
                            };
                            guardar_vista(
                                &raiz,
                                &super::clave_de_vista(&cx.actual.borrow()),
                                &v,
                                solo_esta,
                            );
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
                                    Ok(ruta) => {
                                        renglones.push(renglon_de(&pixpin_docs::nombre(&r), &ruta))
                                    }
                                    Err(e) => {
                                        tracing::warn!(?e, "no se pudo copiar junto a la nota")
                                    }
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
                            pagina.mandar(
                                &json!({"tipo": "hojas", "viva": viva, "grupos": lista})
                                    .to_string(),
                            );
                        }
                        DeLaPagina::InsertarHoja { clave, viva } => {
                            let destino = cx.actual.borrow().clone();
                            if let Some(t) =
                                super::paginas_vivas::insertar(&raiz, &destino, &clave, !viva)
                            {
                                pagina.mandar(
                                    &json!({"tipo": "insertar", "bloque": true, "texto": t})
                                        .to_string(),
                                );
                            }
                        }
                        DeLaPagina::ElegirMensaje => {
                            use pixpin_notas::incrustados::Medios;
                            let m = medios.get_or_insert_with(|| {
                                super::incrustados::MediosDeLaNota::nuevo(
                                    cx.idioma,
                                    cx.ubicacion,
                                    &cx.actual,
                                )
                            });
                            let lista: Vec<serde_json::Value> = m
                                .mensajes()
                                .into_iter()
                                .map(|e| json!({"clave": e.clave, "rotulo": e.rotulo}))
                                .collect();
                            pagina.mandar(&json!({"tipo": "mensajes", "lista": lista}).to_string());
                        }
                        DeLaPagina::InsertarMensaje { clave } => {
                            use pixpin_notas::incrustados::Medios;
                            let m = medios.get_or_insert_with(|| {
                                super::incrustados::MediosDeLaNota::nuevo(
                                    cx.idioma,
                                    cx.ubicacion,
                                    &cx.actual,
                                )
                            });
                            if let Some(t) = m.insertar_mensaje(&clave) {
                                pagina.mandar(
                                    &json!({"tipo": "insertar", "bloque": true, "texto": t})
                                        .to_string(),
                                );
                            }
                        }
                        DeLaPagina::GuardarCopia { texto, titulo } => {
                            let nombre_f =
                                nombre_de_fichero(&titulo, &cx.textos.t("nota-md-nueva"));
                            if let Some(r) = pixpin_shell::guardar::pedir_ruta_para(
                                hwnd(pagina.hwnd()),
                                &nombre_f,
                                &cx.textos.t("nota-md-tipo"),
                                "md",
                            ) {
                                if let Err(e) = std::fs::write(&r, texto) {
                                    tracing::warn!(?e, "no se pudo guardar la copia .md");
                                }
                            }
                        }
                        DeLaPagina::ExportarWord { texto, titulo } => {
                            let (v, _) =
                                leer_vista(&raiz, &super::clave_de_vista(&cx.actual.borrow()));
                            let destino = cx.actual.borrow().clone();
                            let r2 = raiz.clone();
                            let resolver =
                                move |ruta: &str| adjuntos::resolver(&r2, &destino, ruta);
                            let ahora = pixpin_shell::entorno::ahora_utc_ms();
                            if let Some((bytes, sugerido)) = pixpin_notas::word::exportar(
                                &texto,
                                &cx.textos.t("nota-md-nueva"),
                                &v,
                                None,
                                &resolver,
                                cx.aparato,
                                ahora,
                            ) {
                                let nombre_f = nombre_de_fichero(
                                    if sugerido.is_empty() {
                                        &titulo
                                    } else {
                                        &sugerido
                                    },
                                    "Nota",
                                );
                                if let Some(r) = pixpin_shell::guardar::pedir_ruta_para(
                                    hwnd(pagina.hwnd()),
                                    &nombre_f,
                                    &cx.textos.t("nota-md-tipo-word"),
                                    "docx",
                                ) {
                                    if let Err(e) = std::fs::write(&r, bytes) {
                                        tracing::warn!(?e, "no se pudo guardar el Word");
                                    }
                                }
                            }
                        }
                        DeLaPagina::ComentariosPedir { texto } => {
                            pagina.mandar(&comentarios.json(&texto).to_string());
                        }
                        DeLaPagina::ComentarioNuevo {
                            texto,
                            desde,
                            hasta,
                            cuerpo,
                        } => {
                            let id = comentarios.nuevo(&texto, desde, hasta, &cuerpo);
                            let mut j = comentarios.json(&texto);
                            j["nuevo"] = json!(id);
                            pagina.mandar(&j.to_string());
                        }
                        DeLaPagina::ComentarioResponder {
                            texto,
                            hilo,
                            cuerpo,
                        } => {
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
                        // Como pin la ventana la mueve y la estira el pin; de
                        // la pagina solo llega F11 (o Esc a pantalla completa).
                        DeLaPagina::Ventana { accion, .. } => match accion.as_str() {
                            "pantalla" | "maximizar" => {
                                alternar_pantalla(&mut pin, &mut ventana, &pagina)
                            }
                            "minimizar" => ventana.minimizar(),
                            _ => {}
                        },
                        DeLaPagina::Titulo { texto } => {
                            en_la_barra =
                                nombre_en_la_barra(&cx.actual.borrow(), &texto, cx.textos);
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
                        DeLaPagina::Cerrar => pedir_cierre(&mut cerrando, &pagina),
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
    // Cerrar: se esconde ya (que se note al instante) y luego se suelta todo.
    if let Some(r) = riel.take() {
        r.cerrar();
    }
    drop(pin);
    ventana.ocultar();
    drop(pagina);
    drop(ventana);
    Ok(salida)
}

/// Los botones propios de la barra del pin.
pub const VER: u8 = 0;
pub const EDITAR: u8 = 1;
pub const DESHACER: u8 = 2;
pub const REHACER: u8 = 3;
pub const BUSCAR: u8 = 4;
pub const COMENTARIOS: u8 = 5;
pub const COMPARTIR: u8 = 6;
pub const MAS: u8 = 7;

/// **Lo que lleva la barra de una nota**, entre el nombre y la luna/sol, como
/// la barra del editor del movil (`MarkdownEditorActivity`): escribiendo, el
/// ojo para verla (deshacer y rehacer, con Ctrl+Z y Ctrl+Y: el usuario los
/// quito de la barra el 20-oct-2026); viendola, el lapiz para seguir
/// escribiendo (como el visor de Excel). Y siempre los comentarios
/// (encendido con su panel abierto), compartir y el ⋯ (titulo, guardar una
/// copia, Word, letra y tamano). Sin «Copiar» (Ctrl+C) ni lupa (Ctrl+F,
/// desde el 20-oct-2026).
pub fn botones(leyendo: bool, comentarios: bool) -> Vec<BotonLector> {
    let b = |id, icono, encendido| BotonLector {
        id,
        icono,
        encendido,
    };
    let mut v = Vec::with_capacity(7);
    if leyendo {
        v.push(b(EDITAR, &material::EDIT_NOTE, false));
    } else {
        v.push(b(VER, &material::VISIBILITY, false));
    }
    // Sin lupa (20-oct-2026, el usuario): buscar es Ctrl+F.
    v.push(b(COMENTARIOS, &material::FORUM, comentarios));
    v.push(b(COMPARTIR, &material::IOS_SHARE, false));
    v.push(b(MAS, &material::MORE_VERT, false));
    v
}

/// La G de los visores: el menu ⋮.
const VK_G: u32 = 0x47;

/// Abre el menu ⋮ de la barra (lo pinta la pagina), como su boton.
fn mandar_mas(pagina: &Pagina) {
    if let Some(que) = orden_de(MAS) {
        pagina.mandar(&json!({"tipo": "orden", "que": que}).to_string());
        pagina.enfocar();
    }
}

/// **El globo de cada boton de la barra**: su nombre y su tecla.
pub fn pistas(textos: &Catalogo) -> Vec<(u8, String)> {
    vec![
        (VER, textos.t("nota-pista-ver")),
        (EDITAR, textos.t("nota-pista-editar")),
        (DESHACER, textos.t("barra-pista-deshacer")),
        (REHACER, textos.t("barra-pista-rehacer")),
        (BUSCAR, textos.t("nota-pista-buscar")),
        (COMENTARIOS, textos.t("nota-pista-comentarios")),
        (COMPARTIR, textos.t("nota-pista-compartir")),
        (MAS, textos.t("lector-mas")),
    ]
}

/// La `orden` que se le manda a la pagina por cada boton: todo lo hace
/// ella (compartir tambien, que guarda antes lo ultimo escrito).
pub fn orden_de(id: u8) -> Option<&'static str> {
    Some(match id {
        VER => "leer",
        EDITAR => "editar",
        DESHACER => "deshacer",
        REHACER => "rehacer",
        BUSCAR => "buscar",
        COMENTARIOS => "comentarios",
        COMPARTIR => "compartir",
        MAS => "mas",
        _ => return None,
    })
}

/// El fondo mientras la pagina carga: su papel (`--papel` de `nota.css`).
pub fn fondo(oscuro: bool) -> (u8, u8, u8) {
    if oscuro {
        (0x0b, 0x0b, 0x0b)
    } else {
        (0xfd, 0xfc, 0xf9)
    }
}

/// El nombre en la barra del pin: el del fichero (sin extension ni el
/// numero de guardado del movil), como los demas visores; una nota de un
/// proyecto, su titulo.
fn nombre_en_la_barra(destino: &Destino, titulo: &str, t: &Catalogo) -> String {
    match destino {
        Destino::Fichero { ruta } => crate::lector_pin::nombre_limpio(&pixpin_docs::sin_extension(
            &pixpin_docs::nombre(ruta),
        )),
        _ if titulo.trim().is_empty() => t.t("nota-md-nueva"),
        _ => titulo.trim().to_string(),
    }
}

/// Que se cierre: la pagina guarda lo ultimo y contesta `cerrar_listo`.
fn pedir_cierre(cerrando: &mut Option<Instant>, pagina: &Pagina) {
    if cerrando.is_none() {
        *cerrando = Some(Instant::now());
        pagina.mandar(r#"{"tipo":"cerrando"}"#);
    }
}

/// Pantalla completa o de vuelta a pin, y que la pagina lo sepa (Esc vuelve).
fn alternar_pantalla(pin: &mut PinLector, ventana: &mut VentanaOverlay, pagina: &Pagina) {
    pin.alternar_pantalla_completa(ventana);
    pagina.mandar(&json!({"tipo": "pantalla", "si": pin.pantalla_completa}).to_string());
}
/// **De que cuelgan las marcas y la tinta del riel** de una nota: un `.md`
/// suelto, de el mismo (como un Word); una nota de un proyecto, de un
/// nombre fijo en la cache; una nota nueva, de nada (solo mientras este
/// abierta).
///
/// Ese nombre fijo (`cache/riel-notas/<ficha>-<codigo>.md`) no es solo de
/// este PC: `anotado_del_adjunto::base_de` lo entiende como la base
/// `anot-<codigo>` de la nota (la de sus comentarios), y sus marcas, su
/// tinta, sus marcos y sus vistas viajan con ella; lo de antes en la cache
/// pasa la primera vez.
pub fn documento_del_riel(raiz: &Path, destino: &Destino) -> Option<PathBuf> {
    let limpio = limpio_para_el_riel;
    match destino {
        Destino::Fichero { ruta } => Some(ruta.clone()),
        Destino::Mensaje { proyecto, codigo } => {
            Some(raiz.join("cache").join("riel-notas").join(format!(
                "{}-{}.md",
                limpio(proyecto),
                limpio(codigo)
            )))
        }
        Destino::Nueva { .. } => None,
    }
}

/// Lo que queda de un nombre en el de la cache del riel: letras, cifras y
/// `-`; lo demas, `_`.
pub fn limpio_para_el_riel(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
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
        assert!(matches!(
            v,
            DeLaPagina::Vista {
                px: 18,
                solo_esta: true,
                ..
            }
        ));
        let w: DeLaPagina =
            serde_json::from_str(r#"{"tipo":"ventana","accion":"minimizar"}"#).unwrap();
        assert!(matches!(w, DeLaPagina::Ventana { ref borde, .. } if borde.is_empty()));
        assert_eq!(
            serde_json::from_str::<DeLaPagina>(r#"{"tipo":"cerrar_listo"}"#).unwrap(),
            DeLaPagina::CerrarListo
        );
        // Caso negativo: un tipo que no existe no se toma por otro.
        assert!(serde_json::from_str::<DeLaPagina>(r#"{"tipo":"borrar_todo"}"#).is_err());
    }

    #[test]
    fn la_barra_de_una_nota_cambia_al_verla_y_al_escribirla() {
        let ids = |l, c| botones(l, c).iter().map(|b| b.id).collect::<Vec<_>>();
        // Escribiendo (como abre, igual que el movil): el ojo, deshacer y
        // rehacer; viendola, el lapiz. Y siempre buscar, comentarios,
        // compartir y el ⋯.
        // Sin deshacer ni rehacer (20-oct-2026): Ctrl+Z y Ctrl+Y.
        assert_eq!(ids(false, false), vec![VER, COMENTARIOS, COMPARTIR, MAS]);
        assert_eq!(ids(true, false), vec![EDITAR, COMENTARIOS, COMPARTIR, MAS]);
        // Caso negativo: viendola no se deshace (no se escribe), y el lapiz
        // no sale escribiendo.
        assert!(!ids(true, false).contains(&DESHACER) && !ids(false, false).contains(&EDITAR));
        // Los comentarios se encienden con su panel abierto, y nada mas.
        let encendidos = |l, c| {
            botones(l, c)
                .iter()
                .filter(|b| b.encendido)
                .map(|b| b.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(encendidos(false, true), vec![COMENTARIOS]);
        assert!(encendidos(true, false).is_empty());
        // La disposicion entera: la barra comun de los visores, con luna/sol.
        assert_eq!(
            crate::lector_pin::disposicion("Plan", "md", &botones(true, false), true),
            [
                "asa", "nombre", "propio:1", "propio:5", "propio:6", "propio:7", "oscuro",
                "pantalla", "fijar", "cerrar"
            ]
        );
    }

    #[test]
    fn cada_boton_de_la_barra_tiene_su_globo_en_los_dos_idiomas() {
        for idioma in [pixpin_store::Idioma::Espanol, pixpin_store::Idioma::Ingles] {
            let t = Catalogo::nuevo(idioma);
            let p = pistas(&t);
            for id in [
                VER,
                EDITAR,
                DESHACER,
                REHACER,
                BUSCAR,
                COMENTARIOS,
                COMPARTIR,
                MAS,
            ] {
                let s = p
                    .iter()
                    .find(|(i, _)| *i == id)
                    .map(|(_, s)| s.as_str())
                    .unwrap_or_else(|| panic!("{id} sin globo"));
                assert!(
                    !s.is_empty()
                        && !s.starts_with("nota-pista")
                        && !s.starts_with("barra-")
                        && !s.starts_with("lector-"),
                    "{s}"
                );
            }
        }
        // El ⋮ dice su tecla, la G, que tambien lo abre desde la pagina.
        let t = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        assert!(
            pistas(&t)
                .iter()
                .any(|(i, s)| *i == MAS && s.contains("(G)"))
        );
    }

    #[test]
    fn cada_boton_es_una_orden_que_la_pagina_entiende() {
        let pagina = std::str::from_utf8(PAGINA).unwrap();
        for id in [
            VER,
            EDITAR,
            DESHACER,
            REHACER,
            BUSCAR,
            COMENTARIOS,
            COMPARTIR,
            MAS,
        ] {
            let que = orden_de(id).unwrap_or_else(|| panic!("{id} sin orden"));
            // La pagina compilada tiene el `case` de esa orden.
            assert!(pagina.contains(&format!("case\"{que}\"")), "{que}");
        }
        assert!(
            pagina.contains("en-pin"),
            "la pagina sabe esconder su cabecera"
        );
        // Caso negativo: un boton que no existe no manda nada.
        assert_eq!(orden_de(99), None);
        // El estado que manda la pagina se entiende.
        let e: DeLaPagina =
            serde_json::from_str(r#"{"tipo":"estado","leyendo":true,"comentarios":false}"#)
                .unwrap();
        assert_eq!(
            e,
            DeLaPagina::Estado {
                leyendo: true,
                comentarios: false
            }
        );
    }

    #[test]
    fn en_la_barra_va_el_fichero_o_el_titulo_de_la_nota() {
        let t = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let f = Destino::Fichero {
            ruta: PathBuf::from(r"C:\notas\1791239986208_Plan de obra.md"),
        };
        assert_eq!(nombre_en_la_barra(&f, "Otra cosa", &t), "Plan de obra");
        let m = Destino::Mensaje {
            proyecto: "p".into(),
            codigo: "U1".into(),
        };
        assert_eq!(nombre_en_la_barra(&m, "Visita", &t), "Visita");
        // Caso negativo: una nota sin titulo no deja la barra sin nombre.
        assert_eq!(nombre_en_la_barra(&m, "  ", &t), t.t("nota-md-nueva"));
        assert_ne!(fondo(true), fondo(false));
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
        let _ = tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .try_init();
        let hilo = std::thread::spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = pixpin_store::Catalogo::nuevo(pixpin_store::Idioma::Espanol);
            eprintln!("correr");
            super::super::correr(
                &textos,
                pixpin_store::Idioma::Espanol,
                &ub,
                super::Destino::Fichero { ruta: md },
            );
            eprintln!("fin de correr");
        });
        std::thread::sleep(std::time::Duration::from_secs(8));
        eprintln!("hilo vivo: {}", !hilo.is_finished());
    }
}
