//! **Pronunciar: hablar, oirse y repetir, con una guia delante** (B9).
//!
//! Es `guardados/PronunciarActivity.kt`. El bucle que pidio el usuario es
//! corto: **se mantiene pulsado, se habla, se suelta y se oye al momento**.
//! Cada toma pisa la anterior, que era un ensayo; la que convenza se guarda
//! en el chat con «Guardar» y alli se pasa a texto **en el idioma que se
//! practica** (no en el de la interfaz), para ver que entendio Whisper.
//!
//! En el PC se mantiene pulsada la **barra espaciadora** (o el boton del
//! microfono con el raton): el pulgar del movil es aqui la tecla mas grande.
//!
//! **La guia** es lo que se tiene delante mientras se habla: un texto
//! escrito aqui (en la caja de texto de Windows), una nota `.md`/`.txt`, una
//! imagen o una pagina de un PDF, del equipo o de lo que hay en el chat.
//! Imagenes y PDF se abren en un hilo aparte: una foto de doce megapixeles o
//! una pagina de PDF tardan mas de 50 ms y la ventana no puede pararse.
//!
//! **Extra que el movil no tiene**: «Oir el texto» lee la guia escrita con
//! la voz de Windows (`ISpVoice`, ver `pixpin_voz::sapi::Lector`), para oir
//! como suena antes de decirlo. No puntua la pronunciacion: el movil
//! tampoco, y la transcripcion de la toma guardada ya dice que se entendio.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use anyhow::{Context, Result};
use pixpin_audio::picos::MINIMO_MS;
use pixpin_audio::{Grabadora, Salida, reloj};
use pixpin_geom::Rect;
use pixpin_proyecto::cuaderno::{Clase, Mensaje};
use pixpin_render::{Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Idioma};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use crate::overlay::Recursos;
use crate::ventanita::{
    APAGADO, Botones, CRISTAL, DORADO, FONDO, ROJO, TEXTO, centrado, centrado_en, dentro,
};

const VK_ESCAPE: u32 = 0x1B;
const VK_ESPACIO: u32 = 0x20;
const VK_IZQUIERDA: u32 = 0x25;
const VK_DERECHA: u32 = 0x27;
const VK_R: u32 = 0x52;
const VK_G: u32 = 0x47;
const VK_MAS: u32 = 0xBB;
const VK_MENOS: u32 = 0xBD;

/// Los idiomas que se ofrecen para practicar, los del movil
/// (`PronunciarActivity.IDIOMAS`). Vacio = el de siempre. Cada uno con su
/// nombre en su lengua, que es como se reconoce en una lista.
pub const IDIOMAS: [(&str, &str); 8] = [
    ("", ""),
    ("es", "Español"),
    ("en", "English"),
    ("pt", "Português"),
    ("fr", "Français"),
    ("de", "Deutsch"),
    ("it", "Italiano"),
    ("ca", "Català"),
];

/// Una toma que el usuario guardo: el `.m4a`, lo que dura, su onda y en que
/// idioma se practicaba (`None` = el de siempre).
#[derive(Debug, Clone, PartialEq)]
pub struct Toma {
    pub ruta: PathBuf,
    pub duracion_ms: i64,
    pub picos: Vec<i32>,
    pub idioma: Option<String>,
}

/// **Escribe la toma en un mensaje de voz**: clase, duracion y onda, como
/// `Mensaje(clase = VOZ, duracionMs = tomaMs, ...)` del movil. El texto no:
/// ese lo pone la transcripcion que el chat arranca al meterla.
pub fn aplicar(m: &mut Mensaje, t: &Toma) {
    m.clase = Some(Clase::Voz);
    m.duracion_ms = t.duracion_ms;
    m.resto.insert(
        "picos".into(),
        serde_json::Value::Array(
            t.picos
                .iter()
                .map(|p| serde_json::Value::from(*p))
                .collect(),
        ),
    );
}

/// Una guia que se puede traer del chat: como se llama y que es.
#[derive(Debug, Clone, PartialEq)]
pub struct DelChat {
    pub titulo: String,
    pub fuente: Fuente,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Fuente {
    /// Una nota escrita.
    Texto(String),
    /// Un fichero: imagen, PDF o `.md`/`.txt`.
    Fichero(PathBuf),
}

/// Que clase de guia es un fichero, por su extension. `None` si no sirve.
pub fn clase_de(ruta: &Path) -> Option<ClaseDeGuia> {
    let ext = ruta
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "pdf" => Some(ClaseDeGuia::Pdf),
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "heic" | "tif" | "tiff" => {
            Some(ClaseDeGuia::Imagen)
        }
        "md" | "txt" | "markdown" => Some(ClaseDeGuia::Texto),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaseDeGuia {
    Texto,
    Imagen,
    Pdf,
}

/// **Lo del chat que sirve de guia**: notas, imagenes, PDF y `.md`/`.txt`,
/// lo ultimo primero (`opcionesDelChat`). `ruta` resuelve donde esta el
/// adjunto de un mensaje en este equipo.
pub fn guias_del_chat(
    mensajes: &[Mensaje],
    ruta: impl Fn(&Mensaje) -> Option<PathBuf>,
) -> Vec<DelChat> {
    /// Un menu de cien lineas no se lee.
    const CUANTAS: usize = 12;
    let mut v = Vec::new();
    for m in mensajes.iter().rev() {
        if v.len() >= CUANTAS {
            break;
        }
        if m.ruta.is_none() {
            let Some(linea) = m.texto.lines().find(|l| !l.trim().is_empty()) else {
                continue;
            };
            let titulo: String = linea.trim().chars().take(60).collect();
            v.push(DelChat {
                titulo: format!("✎ {titulo}"),
                fuente: Fuente::Texto(m.texto.clone()),
            });
            continue;
        }
        let Some(r) = ruta(m).filter(|r| r.is_file()) else {
            continue;
        };
        let icono = match clase_de(&r) {
            Some(ClaseDeGuia::Imagen) => "🖼",
            Some(ClaseDeGuia::Pdf) | Some(ClaseDeGuia::Texto) => "📄",
            None => continue,
        };
        let nombre = if m.nombre.is_empty() {
            r.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            m.nombre.clone()
        };
        v.push(DelChat {
            titulo: format!("{icono} {nombre}"),
            fuente: Fuente::Fichero(r),
        });
    }
    v
}

/// **Abre Pronunciar en su propio hilo.** Cada toma guardada llega por el
/// canal; el canal se cierra al cerrar la ventana. `destino` es la carpeta
/// de los audios (`crate::voz::carpeta_de_audios`).
pub fn lanzar(idioma: Idioma, del_chat: Vec<DelChat>, destino: PathBuf) -> Receiver<Toma> {
    let (enviar, recibir) = channel();
    let lanzado = std::thread::Builder::new()
        .name("pronunciar".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let hecho = crate::dispositivo_perdido::con_recursos("pronunciar", |r| {
                abrir(r, idioma, &del_chat, &destino, &enviar)
            });
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo abrir Pronunciar");
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de Pronunciar");
    }
    recibir
}

/// Lo que se tiene delante.
enum Guia {
    Ninguna,
    Texto(String),
    /// Una imagen o una pagina de PDF ya subida a la tarjeta.
    Lamina {
        mapa: Option<ID2D1Bitmap1>,
        ancho: u32,
        alto: u32,
        /// Del PDF: su ruta, la pagina y cuantas tiene.
        pdf: Option<(PathBuf, u32, u32)>,
        cargando: bool,
    },
}

/// Lo que devuelve el hilo que abre imagenes y PDF.
struct Cargada {
    imagen: Option<pixpin_codec::imagen::ImagenRgba>,
    paginas: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Cerrar,
    Guia,
    OirTexto,
    Repetir,
    Guardar,
    Microfono,
    Idioma(usize),
    Pagina(i32),
    MasLetra,
    MenosLetra,
}

struct Estado {
    guia: Guia,
    tam: f32,
    scroll: f32,
    idioma: usize,
    grabadora: Option<Grabadora>,
    /// El microfono abriendose en otro hilo, y si ya se solto la tecla.
    abriendo: Option<Receiver<Result<Grabadora, pixpin_audio::ErrorAudio>>>,
    soltado: bool,
    /// La toma cerrandose en otro hilo.
    cerrando: Option<Receiver<Result<pixpin_audio::Grabacion, pixpin_audio::ErrorAudio>>>,
    desde: u64,
    /// La ultima toma: fichero, lo que dura, su onda y si ya se guardo.
    toma: Option<(PathBuf, i64, Vec<i32>)>,
    guardada: bool,
    salida: Option<Salida>,
    botones: Botones<Accion>,
    aviso: Option<(String, u64)>,
    cargando: Option<Receiver<Cargada>>,
    lector: Option<pixpin_voz::sapi::Lector>,
    /// El microfono pulsado con el raton (y no con Espacio): se suelta al
    /// soltar el boton.
    con_el_raton: bool,
    /// El idioma de siempre, en la etiqueta de la voz (`es`).
    lengua: &'static str,
}

fn abrir(
    recursos: &Recursos,
    idioma: Idioma,
    del_chat: &[DelChat],
    destino: &Path,
    enviar: &Sender<Toma>,
) -> Result<()> {
    let textos = Catalogo::nuevo(idioma);
    let textos = &textos;
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = pixpin_shell::pantalla_de::monitor_de_la_ventana_activa()
        .and_then(|r| monitores.monitores().iter().find(|m| m.area == r))
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien as f32 / 100.0;
    let marco = centrado(monitor.area_trabajo, 1000, 780, monitor.escala_por_cien);
    let ventana = VentanaOverlay::nueva_normal(marco, &textos.t("pronunciar-titulo"))
        .context("no se pudo abrir Pronunciar")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para Pronunciar")?;
    ventana.mostrar();
    ventana.enfocar();
    let carpeta = destino.join("pronunciar");
    let _ = std::fs::create_dir_all(&carpeta);

    let mut e = Estado {
        guia: Guia::Ninguna,
        tam: 24.0,
        scroll: 0.0,
        idioma: 0,
        grabadora: None,
        abriendo: None,
        soltado: false,
        cerrando: None,
        desde: 0,
        toma: None,
        guardada: false,
        salida: motor_de_sonido_listo(),
        botones: Botones::default(),
        aviso: None,
        cargando: None,
        lector: None,
        con_el_raton: false,
        lengua: crate::voz::idioma_de_voz(idioma),
    };
    let mut vivo = true;
    let mut pintar = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        let ahora = pixpin_shell::entorno::ahora_utc_ms() as u64;
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            pintar = true;
            match ev {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::RatonMovido(p) => {
                    e.botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                }
                EventoOverlay::BotonPulsado(p) => {
                    e.botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    match e.botones.bajo_el_raton() {
                        Some(Accion::Microfono) => {
                            e.con_el_raton = true;
                            empezar(&mut e, &carpeta, ahora);
                        }
                        Some(a) => hacer(
                            &mut e, a, textos, &ventana, del_chat, enviar, &carpeta, &mut vivo,
                        ),
                        None => {}
                    }
                }
                EventoOverlay::BotonSoltado(_) if e.con_el_raton => {
                    e.con_el_raton = false;
                    soltar(&mut e);
                }
                EventoOverlay::Rueda(m) => {
                    e.scroll = (e.scroll - m as f32 * e.tam * 3.0 * escala).max(0.0);
                }
                // Mantener Espacio: la tecla repite mientras esta abajo, y
                // solo la primera cuenta.
                EventoOverlay::Tecla { vk: VK_ESPACIO, .. } => {
                    if e.grabadora.is_none() && e.abriendo.is_none() {
                        empezar(&mut e, &carpeta, ahora);
                    }
                }
                EventoOverlay::TeclaSoltada(VK_ESPACIO) => soltar(&mut e),
                EventoOverlay::Tecla { vk, ctrl, .. } => {
                    let a = match vk {
                        VK_ESCAPE => Some(Accion::Cerrar),
                        VK_R => Some(Accion::Repetir),
                        VK_G if ctrl => Some(Accion::Guardar),
                        VK_IZQUIERDA => Some(Accion::Pagina(-1)),
                        VK_DERECHA => Some(Accion::Pagina(1)),
                        VK_MAS => Some(Accion::MasLetra),
                        VK_MENOS => Some(Accion::MenosLetra),
                        _ => None,
                    };
                    if let Some(a) = a {
                        hacer(
                            &mut e, a, textos, &ventana, del_chat, enviar, &carpeta, &mut vivo,
                        );
                    }
                }
                _ => {}
            }
        }
        if !vivo {
            break;
        }
        // Lo que abrio el hilo de las laminas, subido a la tarjeta aqui (el
        // motor es de este hilo).
        if recoger_microfono(&mut e, textos, ahora) | recoger_toma(&mut e, textos, ahora) {
            pintar = true;
        }
        if let Some(r) = &e.cargando
            && let Ok(c) = r.try_recv()
        {
            e.cargando = None;
            recibir_lamina(&mut e, c, &motor);
            pintar = true;
        }
        if e.aviso.as_ref().is_some_and(|(_, hasta)| *hasta < ahora) {
            e.aviso = None;
            pintar = true;
        }
        if pintar || e.grabadora.is_some() {
            if let Ok(d) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&d, |p: &Pintor| {
                    pintar_todo(&mut e, p, marco, escala, textos, ahora)
                });
                let _ = superficie.presentar();
            }
            pintar = false;
        }
        // Grabando, el reloj corre: diez veces por segundo. Quieto, se
        // despierta solo por eventos o para mirar el hilo de las laminas.
        let espera = if e.grabadora.is_some() || e.abriendo.is_some() || e.cerrando.is_some() {
            100
        } else if e.cargando.is_some() {
            50
        } else {
            500
        };
        pixpin_shell::overlay::esperar_eventos(Some(espera));
    }
    if let Some(g) = e.grabadora.take() {
        g.cancelar();
    }
    if let Some(l) = &e.lector {
        l.callar();
    }
    e.salida = None;
    // Las tomas eran ensayos: fuera. Lo guardado ya se copio aparte.
    let _ = std::fs::remove_dir_all(&carpeta);
    Ok(())
}

/// Empieza una toma. Lo que sonaba se calla: si no, se grabaria encima.
fn empezar(e: &mut Estado, carpeta: &Path, ahora: u64) {
    if e.grabadora.is_some() || e.abriendo.is_some() {
        return;
    }
    if let Some(s) = &e.salida {
        s.pausar();
    }
    if let Some(l) = &e.lector {
        l.callar();
    }
    // El microfono se abre en otro hilo (medio segundo la primera vez): la
    // ventana ya dice «suelta para oirte» y recoge la grabadora al llegar.
    let fichero = carpeta.join(format!("toma-{ahora}.m4a"));
    e.abriendo = Some(crate::ventanita::abrir_microfono(fichero));
    e.soltado = false;
    e.desde = ahora;
}

/// Lo que llega del hilo del microfono. Si ya se solto, la toma era un
/// toque y se tira.
fn recoger_microfono(e: &mut Estado, textos: &Catalogo, ahora: u64) -> bool {
    let Some(r) = &e.abriendo else {
        return false;
    };
    let llegado = match r.try_recv() {
        Ok(g) => g,
        Err(std::sync::mpsc::TryRecvError::Empty) => return false,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            Err(pixpin_audio::ErrorAudio::HiloCaido)
        }
    };
    e.abriendo = None;
    match llegado {
        Ok(g) if e.soltado => {
            g.cancelar();
            e.aviso = Some((textos.t("chat-voz-muy-corta"), ahora + 3_000));
        }
        Ok(g) => e.grabadora = Some(g),
        Err(err) => {
            tracing::info!(?err, "no se pudo grabar la toma");
            e.aviso = Some((textos.t("telepronter-sin-micro"), ahora + 4_000));
        }
    }
    true
}

/// Suelta: la toma se cierra y **se oye al momento**. Una muy corta es un
/// toque sin querer y no pisa la anterior.
fn soltar(e: &mut Estado) {
    if e.abriendo.is_some() {
        e.soltado = true;
        return;
    }
    let Some(g) = e.grabadora.take() else {
        return;
    };
    // Cerrar el `.m4a` tarda 25-50 ms (medido): en otro hilo, y la toma
    // suena en cuanto llega (`recoger_toma`).
    e.cerrando = Some(crate::ventanita::cerrar_microfono(g));
}

/// La toma cerrada, cuando llega: si es muy corta era un toque y no pisa la
/// anterior; si no, **se oye al momento**.
fn recoger_toma(e: &mut Estado, textos: &Catalogo, ahora: u64) -> bool {
    let Some(r) = &e.cerrando else {
        return false;
    };
    let llegado = match r.try_recv() {
        Ok(g) => g,
        Err(std::sync::mpsc::TryRecvError::Empty) => return false,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            Err(pixpin_audio::ErrorAudio::HiloCaido)
        }
    };
    e.cerrando = None;
    let grabacion = match llegado {
        Ok(g) => g,
        Err(err) => {
            tracing::info!(?err, "la toma no se pudo cerrar");
            e.aviso = Some((textos.t("chat-voz-muy-corta"), ahora + 3_000));
            return true;
        }
    };
    if grabacion.duracion_ms < MINIMO_MS {
        let _ = std::fs::remove_file(&grabacion.ruta);
        e.aviso = Some((textos.t("chat-voz-muy-corta"), ahora + 3_000));
        return true;
    }
    if let Some((vieja, _, _)) = e.toma.take() {
        let _ = std::fs::remove_file(vieja);
    }
    e.toma = Some((grabacion.ruta, grabacion.duracion_ms, grabacion.picos));
    e.guardada = false;
    oir(e);
    true
}

fn oir(e: &mut Estado) {
    let Some((ruta, _, _)) = &e.toma else {
        return;
    };
    // El mismo motor para todas las tomas: crear uno cuesta ~55 ms y la
    // toma tiene que sonar al soltar la tecla, no un rato despues.
    let hecho = match &e.salida {
        Some(s) => s.cambiar_fuente(ruta).map(|_| ()),
        None => Salida::abrir(ruta).map(|s| e.salida = Some(s)),
    };
    if let Err(err) = hecho {
        tracing::info!(?err, "no se pudo abrir la toma");
        return;
    }
    if let Some(s) = &e.salida
        && let Err(err) = s.tocar(1.0)
    {
        tracing::info!(?err, "no se pudo oir la toma");
    }
}

/// El motor de sonido listo desde que se abre la ventana, con un sonido
/// cualquiera de Windows como fuente (no suena: sin `tocar`). Asi la
/// primera toma no paga los ~100 ms de arrancar Media Foundation.
fn motor_de_sonido_listo() -> Option<Salida> {
    let windir = std::env::var_os("WINDIR").map(PathBuf::from)?;
    let sonido = ["Windows Ding.wav", "ding.wav", "chimes.wav"]
        .iter()
        .map(|n| windir.join("Media").join(n))
        .find(|p| p.is_file())?;
    Salida::abrir(&sonido).ok()
}

#[allow(clippy::too_many_arguments)]
fn hacer(
    e: &mut Estado,
    a: Accion,
    textos: &Catalogo,
    ventana: &VentanaOverlay,
    del_chat: &[DelChat],
    enviar: &Sender<Toma>,
    carpeta: &Path,
    vivo: &mut bool,
) {
    let ahora = pixpin_shell::entorno::ahora_utc_ms() as u64;
    match a {
        Accion::Cerrar => *vivo = false,
        Accion::Repetir => oir(e),
        Accion::Guardar => {
            let Some((ruta, ms, picos)) = &e.toma else {
                return;
            };
            if e.guardada {
                return;
            }
            // Se manda una copia: el chat se queda el fichero (lo mueve al
            // proyecto) y esta toma se tiene que poder seguir oyendo.
            let copia = carpeta.join(format!("pronunciar-{ahora}.m4a"));
            if std::fs::copy(ruta, &copia).is_err() {
                e.aviso = Some((textos.t("chat-no-se-pudo"), ahora + 3_000));
                return;
            }
            let idioma = IDIOMAS[e.idioma].0;
            let toma = Toma {
                ruta: copia,
                duracion_ms: *ms,
                picos: picos.clone(),
                idioma: (!idioma.is_empty()).then(|| idioma.to_string()),
            };
            if enviar.send(toma).is_ok() {
                e.guardada = true;
                e.aviso = Some((textos.t("pronunciar-guardado"), ahora + 4_000));
            }
        }
        Accion::Idioma(i) => {
            e.idioma = i.min(IDIOMAS.len() - 1);
            e.lector = None;
        }
        Accion::MasLetra => e.tam = (e.tam + 2.0).min(44.0),
        Accion::MenosLetra => e.tam = (e.tam - 2.0).max(14.0),
        Accion::Pagina(d) => {
            if let Guia::Lamina {
                pdf: Some((ruta, pagina, paginas)),
                cargando,
                ..
            } = &mut e.guia
            {
                let nueva = (*pagina as i32 + d).clamp(0, *paginas as i32 - 1) as u32;
                if nueva != *pagina && !*cargando {
                    *pagina = nueva;
                    *cargando = true;
                    e.cargando = Some(cargar_lamina(ruta.clone(), Some(nueva)));
                }
            }
        }
        Accion::OirTexto => {
            let Guia::Texto(t) = &e.guia else {
                return;
            };
            if e.lector.is_none() {
                let idioma = match IDIOMAS[e.idioma].0 {
                    "" => e.lengua,
                    i => i,
                };
                e.lector = pixpin_voz::sapi::Lector::nuevo(idioma);
            }
            match &e.lector {
                Some(l) => l.leer(&pixpin_voz::parrafos_de(t).join("\n")),
                None => e.aviso = Some((textos.t("pronunciar-sin-voz"), ahora + 4_000)),
            }
        }
        Accion::Microfono => {}
        Accion::Guia => elegir_guia(e, textos, ventana, del_chat),
    }
}

/// «Guia»: escribir un texto, traer algo del equipo o del chat
/// (`ModalBottomSheet` del movil; aqui un menu de Windows).
fn elegir_guia(e: &mut Estado, textos: &Catalogo, ventana: &VentanaOverlay, del_chat: &[DelChat]) {
    let mut entradas = vec![
        (1, textos.t("pronunciar-guia-texto")),
        (2, textos.t("pronunciar-guia-fichero")),
    ];
    for (i, d) in del_chat.iter().enumerate() {
        entradas.push((100 + i as u32, d.titulo.clone()));
    }
    match pixpin_shell::menu_llano(ventana.handle(), &entradas) {
        Some(1) => {
            let actual = match &e.guia {
                Guia::Texto(t) => t.clone(),
                _ => String::new(),
            };
            // La caja de texto va en su hilo; aqui se espera su respuesta
            // bombeando, para que esta ventana siga pintandose detras.
            let r = pixpin_shell::caja_de_texto::abrir(pixpin_shell::caja_de_texto::Pedido {
                titulo: textos.t("pronunciar-guia-texto"),
                texto: actual,
                guardar: textos.t("chat-letra-guardar"),
                cancelar: textos.t("chat-cancelar-caja"),
            });
            loop {
                match r.try_recv() {
                    Ok(Some(t)) => {
                        poner_texto(e, t);
                        break;
                    }
                    Ok(None) | Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        pixpin_shell::overlay::bombear_pendientes();
                        let _ = pixpin_shell::overlay::tomar_eventos_pendientes();
                        pixpin_shell::overlay::esperar_eventos(Some(50));
                    }
                }
            }
        }
        Some(2) => {
            if let Some(f) = pixpin_shell::elegir::pedir_ficheros(ventana.handle())
                .into_iter()
                .next()
            {
                poner_fichero(e, textos, f);
            }
        }
        Some(n) if n >= 100 => match del_chat.get((n - 100) as usize).map(|d| &d.fuente) {
            Some(Fuente::Texto(t)) => poner_texto(e, t.clone()),
            Some(Fuente::Fichero(f)) => poner_fichero(e, textos, f.clone()),
            None => {}
        },
        _ => {}
    }
}

fn poner_texto(e: &mut Estado, t: String) {
    e.guia = if t.trim().is_empty() {
        Guia::Ninguna
    } else {
        Guia::Texto(t)
    };
    e.scroll = 0.0;
}

fn poner_fichero(e: &mut Estado, textos: &Catalogo, f: PathBuf) {
    let ahora = pixpin_shell::entorno::ahora_utc_ms() as u64;
    match clase_de(&f) {
        Some(ClaseDeGuia::Texto) => match std::fs::read(&f) {
            // Con perdidas y no exigiendo UTF-8: un `.txt` viejo de Windows
            // viene en la pagina de codigos de siempre.
            Ok(b) => poner_texto(e, String::from_utf8_lossy(&b).into_owned()),
            Err(_) => e.aviso = Some((textos.t("pronunciar-no-se-pudo"), ahora + 3_000)),
        },
        Some(ClaseDeGuia::Imagen) => {
            e.guia = Guia::Lamina {
                mapa: None,
                ancho: 0,
                alto: 0,
                pdf: None,
                cargando: true,
            };
            e.cargando = Some(cargar_lamina(f, None));
        }
        Some(ClaseDeGuia::Pdf) => {
            e.guia = Guia::Lamina {
                mapa: None,
                ancho: 0,
                alto: 0,
                pdf: Some((f.clone(), 0, 0)),
                cargando: true,
            };
            e.cargando = Some(cargar_lamina(f, Some(0)));
        }
        None => e.aviso = Some((textos.t("pronunciar-no-se-pudo"), ahora + 3_000)),
    }
}

/// El lado mayor de una lamina: de sobra para una ventana de mil y pico, y
/// por debajo del tope de textura de la HD 4000.
const LADO_DE_LAMINA: u32 = 1600;

/// Abre una imagen o una pagina de PDF en un hilo aparte.
fn cargar_lamina(ruta: PathBuf, pagina: Option<u32>) -> Receiver<Cargada> {
    let (tx, rx) = channel();
    let _ = std::thread::Builder::new()
        .name("pronunciar-lamina".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let c = match pagina {
                None => Cargada {
                    imagen: pixpin_codec::imagen::cargar(&ruta).ok().map(reducir),
                    paginas: 0,
                },
                Some(n) => match pixpin_pdf::Documento::abrir(&ruta) {
                    Ok(d) => Cargada {
                        imagen: d.renderizar(n, LADO_DE_LAMINA).ok(),
                        paginas: d.paginas(),
                    },
                    Err(_) => Cargada {
                        imagen: None,
                        paginas: 0,
                    },
                },
            };
            let _ = tx.send(c);
        });
    rx
}

fn reducir(i: pixpin_codec::imagen::ImagenRgba) -> pixpin_codec::imagen::ImagenRgba {
    let lado = i.ancho.max(i.alto);
    if lado <= LADO_DE_LAMINA {
        return i;
    }
    let (w, h) = (
        (i.ancho as u64 * LADO_DE_LAMINA as u64 / lado as u64).max(1) as u32,
        (i.alto as u64 * LADO_DE_LAMINA as u64 / lado as u64).max(1) as u32,
    );
    let copia = i.clone();
    pixpin_codec::imagen::redimensionar(i, w, h).unwrap_or(copia)
}

fn recibir_lamina(e: &mut Estado, c: Cargada, motor: &pixpin_render::MotorRender) {
    let Guia::Lamina {
        mapa,
        ancho,
        alto,
        pdf,
        cargando,
    } = &mut e.guia
    else {
        return;
    };
    *cargando = false;
    if let Some((_, _, paginas)) = pdf {
        *paginas = c.paginas.max(1);
    }
    let Some(i) = c.imagen else {
        e.guia = Guia::Ninguna;
        return;
    };
    *mapa = motor
        .bitmap_desde_pixeles_premultiplicado(i.ancho, i.alto, &i.pixeles)
        .ok();
    *ancho = i.ancho;
    *alto = i.alto;
}

fn pintar_todo(
    e: &mut Estado,
    p: &Pintor,
    marco: Rect,
    escala: f32,
    textos: &Catalogo,
    ahora: u64,
) {
    let (w, h) = (marco.ancho as f32, marco.alto as f32);
    p.limpiar(FONDO);
    e.botones.vaciar();

    // Arriba: volver, el titulo, A-/A+ y «Oir el texto» con texto, y Guia.
    let barra = 52.0 * escala;
    p.rellenar(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho: w,
            alto: barra,
        },
        CRISTAL,
    );
    let alto_b = 36.0 * escala;
    let yb = (barra - alto_b) / 2.0;
    let caja = |x: f32, ancho: f32| RectF {
        x,
        y: yb,
        ancho: ancho * escala,
        alto: alto_b,
    };
    e.botones.boton(
        p,
        caja(10.0 * escala, 44.0),
        Accion::Cerrar,
        "←",
        None,
        escala,
    );
    p.texto(
        &textos.t("pronunciar-titulo"),
        64.0 * escala,
        yb + 8.0 * escala,
        17.0 * escala,
        TEXTO,
    );
    let mut x = w - 10.0 * escala;
    let mut a_la_izquierda = |ancho: f32| {
        x -= ancho * escala;
        let c = caja(x, ancho);
        x -= 6.0 * escala;
        c
    };
    let c = a_la_izquierda(110.0);
    e.botones.boton(
        p,
        c,
        Accion::Guia,
        &textos.t("pronunciar-guia"),
        None,
        escala,
    );
    if matches!(e.guia, Guia::Texto(_)) {
        let c = a_la_izquierda(130.0);
        e.botones.boton(
            p,
            c,
            Accion::OirTexto,
            &textos.t("pronunciar-oir-texto"),
            None,
            escala,
        );
        let c = a_la_izquierda(44.0);
        e.botones.boton(p, c, Accion::MasLetra, "A+", None, escala);
        let c = a_la_izquierda(44.0);
        e.botones
            .boton(p, c, Accion::MenosLetra, "A-", None, escala);
    }

    // El idioma que se practica: manda en la transcripcion de lo guardado.
    let y_idiomas = barra + 8.0 * escala;
    p.texto(
        &textos.t("pronunciar-idioma"),
        14.0 * escala,
        y_idiomas,
        12.0 * escala,
        APAGADO,
    );
    let mut xi = 14.0 * escala;
    let yi = y_idiomas + 18.0 * escala;
    for (i, (_, nombre)) in IDIOMAS.iter().enumerate() {
        let rotulo = if nombre.is_empty() {
            textos.t("pronunciar-idioma-ajustes")
        } else {
            nombre.to_string()
        };
        let ancho = p.medir_texto(&rotulo, 13.0 * escala).0 + 24.0 * escala;
        let c = RectF {
            x: xi,
            y: yi,
            ancho,
            alto: 28.0 * escala,
        };
        let elegido = i == e.idioma;
        p.rellenar_redondeado(
            c,
            14.0 * escala,
            if elegido {
                crate::ventanita::AZUL
            } else {
                CRISTAL
            },
        );
        p.texto(
            &rotulo,
            c.x + 12.0 * escala,
            c.y + 6.0 * escala,
            13.0 * escala,
            TEXTO,
        );
        e.botones.zona(c, Accion::Idioma(i));
        xi += ancho + 6.0 * escala;
    }

    // La guia, con lo que quede entre los idiomas y el pie.
    let pie = 230.0 * escala;
    let guia = RectF {
        x: 16.0 * escala,
        y: yi + 40.0 * escala,
        ancho: w - 32.0 * escala,
        alto: (h - pie - (yi + 40.0 * escala)).max(1.0),
    };
    pintar_guia(e, p, guia, escala, textos);

    // Abajo: la toma y el microfono, con lo que va a pasar dicho claro.
    let y0 = h - pie;
    let grabando = e.grabadora.is_some() || e.abriendo.is_some();
    let medio = RectF {
        x: 0.0,
        y: y0,
        ancho: w,
        alto: pie,
    };
    let mut y = y0 + 8.0 * escala;
    if let Some((_, ms, _)) = &e.toma
        && !grabando
    {
        let ancho_b = 190.0 * escala;
        let c1 = RectF {
            x: w / 2.0 - ancho_b - 6.0 * escala,
            y,
            ancho: ancho_b,
            alto: 40.0 * escala,
        };
        let c2 = RectF {
            x: w / 2.0 + 6.0 * escala,
            ..c1
        };
        e.botones.boton(
            p,
            c1,
            Accion::Repetir,
            &textos.t("pronunciar-repetir"),
            Some(crate::ventanita::AZUL),
            escala,
        );
        let guardar = if e.guardada {
            textos.t("pronunciar-ya-guardada")
        } else {
            textos.t("pronunciar-guardar")
        };
        e.botones
            .boton(p, c2, Accion::Guardar, &guardar, None, escala);
        y += 46.0 * escala;
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("duracion", reloj::duracion_legible(*ms));
        centrado_en(
            p,
            &textos.t_args("pronunciar-toma", &args),
            medio,
            y,
            12.0 * escala,
            APAGADO,
        );
    }
    let estado = if grabando {
        textos.t("pronunciar-suelta")
    } else if e.toma.is_some() {
        textos.t("pronunciar-otra-vez")
    } else {
        textos.t("pronunciar-manten")
    };
    centrado_en(
        p,
        &estado,
        medio,
        y0 + 76.0 * escala,
        15.0 * escala,
        if grabando { ROJO } else { APAGADO },
    );
    // El reloj de lo hablado: dice que el microfono esta cogiendo algo.
    if grabando {
        let llevado = e.grabadora.as_ref().map(|g| g.llevado_ms()).unwrap_or(0);
        let _ = ahora;
        centrado_en(
            p,
            &reloj::duracion_legible(llevado),
            medio,
            y0 + 98.0 * escala,
            20.0 * escala,
            ROJO,
        );
    }
    let radio = if grabando { 54.0 } else { 46.0 } * escala;
    let centro = (w / 2.0, h - 16.0 * escala - 46.0 * escala);
    p.circulo(
        centro,
        radio,
        if grabando {
            ROJO
        } else {
            crate::ventanita::AZUL
        },
    );
    let icono = 40.0 * escala;
    p.icono(
        &pixpin_render::icono::material::MIC,
        RectF {
            x: centro.0 - icono / 2.0,
            y: centro.1 - icono / 2.0,
            ancho: icono,
            alto: icono,
        },
        TEXTO,
    );
    e.botones.zona(
        RectF {
            x: centro.0 - radio,
            y: centro.1 - radio,
            ancho: radio * 2.0,
            alto: radio * 2.0,
        },
        Accion::Microfono,
    );
    if let Some((aviso, _)) = &e.aviso {
        p.texto(
            aviso,
            16.0 * escala,
            y0 - 24.0 * escala,
            14.0 * escala,
            DORADO,
        );
    }
}

fn pintar_guia(e: &mut Estado, p: &Pintor, r: RectF, escala: f32, textos: &Catalogo) {
    match &e.guia {
        Guia::Ninguna => {
            centrado_en(
                p,
                &textos.t("pronunciar-sin-guia"),
                r,
                r.y + r.alto / 2.0 - 20.0 * escala,
                15.0 * escala,
                APAGADO,
            );
            let ancho = 160.0 * escala;
            let c = RectF {
                x: r.x + (r.ancho - ancho) / 2.0,
                y: r.y + r.alto / 2.0 + 8.0 * escala,
                ancho,
                alto: 36.0 * escala,
            };
            e.botones.boton(
                p,
                c,
                Accion::Guia,
                &textos.t("pronunciar-guia"),
                None,
                escala,
            );
        }
        Guia::Texto(t) => {
            let tam = e.tam * escala;
            let (_, alto) = p.medir_texto_ajustado(t, tam, r.ancho);
            e.scroll = e.scroll.min((alto - r.alto).max(0.0));
            let y = r.y - e.scroll;
            p.con_recorte(r, |p| p.texto_ajustado(t, r.x, y, tam, r.ancho, TEXTO));
        }
        Guia::Lamina {
            mapa,
            ancho,
            alto,
            pdf,
            cargando,
        } => {
            let abajo = if pdf.is_some() { 40.0 * escala } else { 0.0 };
            let sitio = RectF {
                alto: (r.alto - abajo).max(1.0),
                ..r
            };
            if let Some(m) = mapa
                && *ancho > 0
                && *alto > 0
            {
                // A lo ancho, como el `ContentScale.Fit` del movil.
                let k = (sitio.ancho / *ancho as f32).min(sitio.alto / *alto as f32);
                let (dw, dh) = (*ancho as f32 * k, *alto as f32 * k);
                p.bitmap(
                    m,
                    RectF {
                        x: sitio.x + (sitio.ancho - dw) / 2.0,
                        y: sitio.y + (sitio.alto - dh) / 2.0,
                        ancho: dw,
                        alto: dh,
                    },
                    None,
                    false,
                );
            } else if *cargando {
                centrado_en(
                    p,
                    "…",
                    sitio,
                    sitio.y + sitio.alto / 2.0,
                    24.0 * escala,
                    APAGADO,
                );
            }
            if let Some((_, pagina, paginas)) = pdf {
                let y = r.y + r.alto - abajo + 4.0 * escala;
                let lado = 36.0 * escala;
                let medio = r.x + r.ancho / 2.0;
                let (pagina, paginas) = (*pagina, *paginas);
                e.botones.boton(
                    p,
                    RectF {
                        x: medio - 110.0 * escala,
                        y,
                        ancho: lado,
                        alto: lado,
                    },
                    Accion::Pagina(-1),
                    "‹",
                    None,
                    escala,
                );
                e.botones.boton(
                    p,
                    RectF {
                        x: medio + 110.0 * escala - lado,
                        y,
                        ancho: lado,
                        alto: lado,
                    },
                    Accion::Pagina(1),
                    "›",
                    None,
                    escala,
                );
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("pagina", pagina as i64 + 1);
                args.set("paginas", paginas.max(1) as i64);
                centrado_en(
                    p,
                    &textos.t_args("pronunciar-pagina", &args),
                    r,
                    y + 9.0 * escala,
                    13.0 * escala,
                    APAGADO,
                );
            }
        }
    }
    let _ = dentro;
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_toma_guardada_se_escribe_como_nota_de_voz_con_su_onda() {
        let mut m = Mensaje::default();
        aplicar(
            &mut m,
            &Toma {
                ruta: PathBuf::from("C:/x/pronunciar-1.m4a"),
                duracion_ms: 4_200,
                picos: vec![1, 2, 3],
                idioma: Some("en".into()),
            },
        );
        assert_eq!(m.clase, Some(Clase::Voz));
        assert_eq!(m.duracion_ms, 4_200);
        assert_eq!(m.resto.get("picos"), Some(&serde_json::json!([1, 2, 3])));
        assert!(
            m.transcripcion.is_none(),
            "el texto lo pone la transcripcion"
        );
    }

    #[test]
    fn la_clase_de_guia_sale_de_la_extension_y_lo_demas_no_sirve() {
        assert_eq!(clase_de(Path::new("a/libro.PDF")), Some(ClaseDeGuia::Pdf));
        assert_eq!(clase_de(Path::new("foto.jpeg")), Some(ClaseDeGuia::Imagen));
        assert_eq!(clase_de(Path::new("frases.md")), Some(ClaseDeGuia::Texto));
        assert_eq!(clase_de(Path::new("hoja.xlsx")), None);
        assert_eq!(clase_de(Path::new("sin_extension")), None);
    }

    #[test]
    fn del_chat_salen_notas_y_adjuntos_que_sirven_lo_ultimo_primero() {
        let dir = std::env::temp_dir().join(format!("pixpin-pronunciar-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pdf = dir.join("lecturas.pdf");
        std::fs::write(&pdf, b"%PDF").unwrap();
        let hoja = dir.join("cuentas.xlsx");
        std::fs::write(&hoja, b"x").unwrap();
        let mensajes = vec![
            Mensaje {
                texto: "\n  Frase para practicar\notra linea".into(),
                ..Mensaje::default()
            },
            Mensaje {
                ruta: Some("lecturas.pdf".into()),
                nombre: "lecturas.pdf".into(),
                ..Mensaje::default()
            },
            Mensaje {
                ruta: Some("cuentas.xlsx".into()),
                nombre: "cuentas.xlsx".into(),
                ..Mensaje::default()
            },
            Mensaje {
                ruta: Some("no-esta.png".into()),
                ..Mensaje::default()
            },
        ];
        let v = guias_del_chat(&mensajes, |m| m.ruta.as_ref().map(|r| dir.join(r)));
        assert_eq!(v.len(), 2, "{v:?}");
        assert_eq!(v[0].titulo, "📄 lecturas.pdf");
        assert_eq!(v[0].fuente, Fuente::Fichero(pdf));
        assert_eq!(v[1].titulo, "✎ Frase para practicar");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn estado() -> Estado {
        Estado {
            guia: Guia::Ninguna,
            tam: 24.0,
            scroll: 0.0,
            idioma: 2,
            grabadora: None,
            abriendo: None,
            soltado: false,
            cerrando: None,
            desde: 0,
            toma: None,
            guardada: false,
            salida: None,
            botones: Botones::default(),
            aviso: None,
            cargando: None,
            lector: None,
            con_el_raton: false,
            lengua: "es",
        }
    }

    /// `cargo test -p pixpin --bin pixpinmax muestra_de_pronunciar --
    /// --ignored --nocapture`: la pantalla sin guia y con texto y toma.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_de_pronunciar() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let marco = Rect {
            x: 0,
            y: 0,
            ancho: 1000,
            alto: 780,
        };
        let mut e = estado();
        crate::ventanita::muestra("pronunciar-sin-guia", 1000, 780, |p, _| {
            pintar_todo(&mut e, p, marco, 1.0, &textos, 0)
        });
        let mut e = estado();
        e.guia = Guia::Texto(
            "Good morning. Could you tell me the way to the station, please?\n\nThe weather is lovely today.".into(),
        );
        e.toma = Some((PathBuf::from("x.m4a"), 4_200, vec![]));
        crate::ventanita::muestra("pronunciar-con-toma", 1000, 780, |p, _| {
            pintar_todo(&mut e, p, marco, 1.0, &textos, 0)
        });
    }

    /// **Lo que corre en el hilo de la ventana, medido** (tope: 50 ms).
    /// Abre el microfono de verdad (sin sonar nada): `cargo test -p pixpin
    /// --bin pixpinmax mide_lo_que_corre -- --ignored --nocapture`.
    #[test]
    #[ignore = "abre el microfono y la tarjeta de sonido"]
    fn mide_lo_que_corre_en_la_ventana_de_pronunciar() {
        let _com = pixpin_shell::ComDelHilo::iniciar();
        let ms = |t: std::time::Instant| t.elapsed().as_secs_f64() * 1000.0;
        let dir = std::env::temp_dir().join("pixpin-mide-pronunciar");
        let _ = std::fs::create_dir_all(&dir);
        let mut peor = 0f64;
        // Como la ventana: el motor de sonido se crea al abrirla.
        let listo = motor_de_sonido_listo().expect("sonido de Windows");
        for vuelta in 0..2 {
            // Lo que ve la ventana: pedir el microfono vuelve en el acto.
            let t = std::time::Instant::now();
            let r = crate::ventanita::abrir_microfono(dir.join(format!("t{vuelta}.m4a")));
            let pedir = ms(t);
            let t = std::time::Instant::now();
            let g = r.recv().unwrap().expect("microfono");
            let en_llegar = ms(t);
            std::thread::sleep(std::time::Duration::from_millis(1_500));
            let t = std::time::Instant::now();
            let r = crate::ventanita::cerrar_microfono(g);
            let parar = ms(t);
            let hecho = r.recv().unwrap().expect("parar");
            let t = std::time::Instant::now();
            listo.cambiar_fuente(&hecho.ruta).expect("abrir");
            let abrir = ms(t);
            let t = std::time::Instant::now();
            let l = pixpin_voz::sapi::Lector::nuevo("es");
            let lector = ms(t);
            drop(l);
            eprintln!(
                "vuelta {vuelta}: pedir el microfono {pedir:.1} ms (llega en otro hilo en {en_llegar:.0} ms) | soltar {parar:.1} ms | abrir la toma {abrir:.1} ms | voz de Windows {lector:.1} ms"
            );
            if vuelta == 1 {
                peor = pedir.max(parar).max(abrir).max(lector);
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
        // La primera vuelta arranca Media Foundation (una vez por proceso).
        assert!(peor < 50.0, "algo pasa de 50 ms: {peor:.1}");
    }

    #[test]
    fn los_rotulos_de_pronunciar_estan_en_los_dos_idiomas() {
        for idioma in [Idioma::Espanol, Idioma::Ingles] {
            let c = Catalogo::nuevo(idioma);
            for k in [
                "pronunciar-titulo",
                "pronunciar-manten",
                "pronunciar-suelta",
                "pronunciar-otra-vez",
                "pronunciar-guia",
                "pronunciar-guia-texto",
                "pronunciar-guia-fichero",
                "pronunciar-sin-guia",
                "pronunciar-repetir",
                "pronunciar-guardar",
                "pronunciar-ya-guardada",
                "pronunciar-guardado",
                "pronunciar-idioma",
                "pronunciar-idioma-ajustes",
                "pronunciar-oir-texto",
                "pronunciar-sin-voz",
                "pronunciar-no-se-pudo",
            ] {
                assert_ne!(c.t(k), k, "falta {k} en {idioma:?}");
            }
        }
    }
}
