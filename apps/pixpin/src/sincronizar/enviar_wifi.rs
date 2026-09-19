//! «Enviar por Wi-Fi», dentro de Sincronizar.
//!
//! Copia de `sincro/EnviarActivity.kt` del movil. Se elige que mandar, se
//! abre la puerta con un codigo de seis cifras (en grande y en su QR
//! `pixpin-envio:1:...`), se anuncia en la red con la etiqueta del codigo y
//! cada aparato que llega sale en la lista esperando que se le apruebe. El
//! codigo vale mientras esta pantalla este abierta, y cinco intentos con uno
//! equivocado lo queman.
//!
//! Lo que cambia respecto al movil, y por que:
//!
//! - **Elegir que.** En el movil «Enviar» salta directo al selector de
//!   archivos y un proyecto se manda desde su menu. Aqui la pantalla pregunta
//!   primero —«Archivos» (el dialogo de abrir de Windows, varios a la vez) o
//!   «Un proyecto» de la lista—, porque el chat del PC no tiene aun ese menu.
//! - **Sin camara.** «Escanear a quien recibe» es aqui «Pegar el codigo de
//!   quien recibe»: sus seis cifras, que se buscan en la red por la etiqueta
//!   que anuncia (`_pixpinrecibe._tcp`), o el texto entero de su QR.

use std::collections::{HashMap, HashSet};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use pixpin_render::icono::material;
use pixpin_sincro::envio::{self, Elemento, Emisor, Final, Remitente};
use pixpin_store::Catalogo;

use super::recibir_wifi::{
    ESPERA_LARGA, boton_apagado, buscar_y_conectar, caja_wifi, campo_grande, conectar, hex_blanco,
    nombre_de, nonce, tamano,
};
use super::{
    Accion, Aviso, BORDE, Contexto, ERROR, Lienzo, PRIMARIO, SUAVE, TEXTO, Tecla, VERDE, caja,
    menos, rect,
};

/// Lo que se manda: cada elemento con el fichero de donde sale.
type Cosas = Arc<Vec<(Elemento, PathBuf)>>;

/// Lo preparado, y la carpeta temporal que haya que borrar al salir.
type Preparado = (Vec<(Elemento, PathBuf)>, Option<PathBuf>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Falla {
    NoPreparado,
    /// Cinco intentos con un codigo equivocado.
    Quemado,
    Cortado(String),
}

pub(super) enum Fase {
    Eligiendo,
    /// La lista de proyectos para elegir uno: id y nombre.
    Proyectos(Vec<(String, String)>),
    Preparando,
    Esperando {
        codigo: String,
        qr: Option<qrcodegen::QrCode>,
        malos: u32,
    },
    Fallo(Falla),
}

/// En que anda cada aparato de la lista (`Destino.Fase` del movil).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum EnDestino {
    Pidiendo,
    Enviando { hechos: u64, total: u64 },
    Hecho,
    Rechazado,
    SinAprobar,
    Fallo(String),
}

pub(super) struct Destino {
    id: u64,
    nombre: String,
    en: EnDestino,
    /// El codigo que se pego para llamarle, si se le llamo desde aqui.
    llamado: Option<String>,
}

/// Lo que cuentan los hilos.
pub(super) enum Novedad {
    Preparado(Result<Preparado, String>),
    /// Alguien se conecto con el codigo y espera que se le apruebe.
    Llega {
        id: u64,
        nombre: String,
        aprueba: mpsc::Sender<bool>,
    },
    Destino {
        id: u64,
        nombre: Option<String>,
        en: EnDestino,
    },
    Malo(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Toque {
    Archivos,
    UnProyecto,
    Proyecto(usize),
    Aprobar(u64),
    NoAprobar(u64),
    AprobarTodos,
    Pegar,
    DejarDePegar,
    Campo,
    Mandar,
    Cerrar,
}

pub(super) struct Enviar {
    pub(super) turno: u64,
    fase: Fase,
    cosas: Cosas,
    destinos: Vec<Destino>,
    aprobaciones: HashMap<u64, mpsc::Sender<bool>>,
    /// Con el campo de «pegar el codigo de quien recibe» abierto.
    pegando: bool,
    tecleado: String,
    /// Los codigos ya llamados: pegar dos veces el mismo no manda dos.
    llamados: HashSet<String>,
    /// La carpeta donde se empaqueto un proyecto: se borra al salir.
    temporal: Option<PathBuf>,
    vivo: Arc<AtomicBool>,
    siguiente: Arc<AtomicU64>,
}

impl Drop for Enviar {
    fn drop(&mut self) {
        // Al cerrar la pantalla el codigo deja de existir: se cierra la
        // puerta y lo que se empaqueto para mandar se borra.
        self.vivo.store(false, Ordering::SeqCst);
        if let Some(d) = self.temporal.take() {
            let _ = std::thread::Builder::new()
                .name("enviar-limpiar".into())
                .spawn(move || {
                    // Un hilo que aun manda puede tener el fichero abierto:
                    // se espera un poco antes de borrar.
                    std::thread::sleep(Duration::from_secs(2));
                    let _ = std::fs::remove_dir_all(d);
                });
        }
    }
}

impl Enviar {
    pub(super) fn nuevo(turno: u64) -> Enviar {
        Enviar {
            turno,
            fase: Fase::Eligiendo,
            cosas: Arc::new(Vec::new()),
            destinos: Vec::new(),
            aprobaciones: HashMap::new(),
            pegando: false,
            tecleado: String::new(),
            llamados: HashSet::new(),
            temporal: None,
            vivo: Arc::new(AtomicBool::new(true)),
            siguiente: Arc::new(AtomicU64::new(1)),
        }
    }

    pub(super) fn animando(&self) -> bool {
        matches!(self.fase, Fase::Preparando | Fase::Esperando { .. })
    }

    /// Lo pegado, si es de quien espera recibir: seis cifras o su QR
    /// `pixpin-recibe`. El QR de otro que ENVIA no vale aqui.
    fn a_quien(&self) -> Option<envio::DelQr> {
        envio::leer_tecleado(&self.tecleado)
            .filter(|q| q.espera_recibir || (q.host.is_none() && !self.tecleado.contains(':')))
    }
}

fn avisar(cx: &Contexto, turno: u64, n: Novedad) -> bool {
    cx.tx.send(Aviso::Enviar(turno, n)).is_ok()
}

/// Lo que llega de los hilos.
pub(super) fn novedad(s: &mut Enviar, n: Novedad, cx: &Contexto) {
    match n {
        Novedad::Preparado(Ok((cosas, temporal))) => {
            s.cosas = Arc::new(cosas);
            s.temporal = temporal;
            s.fase = match abrir_la_puerta(s, cx) {
                Ok(f) => f,
                Err(e) => Fase::Fallo(Falla::Cortado(e.to_string())),
            };
        }
        Novedad::Preparado(Err(e)) => {
            tracing::warn!(%e, "no se pudo preparar el envio");
            s.fase = Fase::Fallo(Falla::NoPreparado);
        }
        Novedad::Llega {
            id,
            nombre,
            aprueba,
        } => {
            s.aprobaciones.insert(id, aprueba);
            s.destinos.push(Destino {
                id,
                nombre,
                en: EnDestino::Pidiendo,
                llamado: None,
            });
        }
        Novedad::Destino { id, nombre, en } => {
            if let Some(d) = s.destinos.iter_mut().find(|d| d.id == id) {
                if let Some(n) = nombre.filter(|n| !n.trim().is_empty()) {
                    d.nombre = n;
                }
                // Si fallo una llamada, ese codigo se puede volver a pegar.
                if matches!(en, EnDestino::Fallo(_))
                    && let Some(c) = &d.llamado
                {
                    s.llamados.remove(c);
                }
                d.en = en;
            }
        }
        Novedad::Malo(n) => {
            if n >= envio::INTENTOS {
                s.fase = Fase::Fallo(Falla::Quemado);
                s.vivo.store(false, Ordering::SeqCst);
            } else if let Fase::Esperando { malos, .. } = &mut s.fase {
                *malos = n;
            }
        }
    }
}

pub(super) fn tecla(s: &mut Enviar, t: Tecla, cx: &Contexto) -> bool {
    if !s.pegando || !matches!(s.fase, Fase::Esperando { .. }) {
        return false;
    }
    match t {
        Tecla::Letra(c) => {
            if s.tecleado.chars().count() < 80 {
                s.tecleado.push(c);
            }
        }
        Tecla::Borrar => {
            s.tecleado.pop();
        }
        Tecla::Pegar(texto) => s.tecleado = texto.trim().chars().take(80).collect(),
        Tecla::Intro => {
            tocar(s, Toque::Mandar, cx, &|| Vec::new());
        }
    }
    true
}

/// Un toque. `elegir` abre el dialogo de archivos de Windows. Devuelve
/// `true` si hay que volver a la portada.
pub(super) fn tocar(
    s: &mut Enviar,
    t: Toque,
    cx: &Contexto,
    elegir: &dyn Fn() -> Vec<PathBuf>,
) -> bool {
    match t {
        Toque::Archivos => {
            let rutas = elegir();
            if !rutas.is_empty() {
                s.fase = Fase::Preparando;
                preparar(cx, s.turno, Que::Archivos(rutas));
            }
        }
        Toque::UnProyecto => {
            let indice = pixpin_proyecto::almacen::Indice::leer(&cx.raiz);
            s.fase = Fase::Proyectos(
                indice
                    .ordenadas()
                    .iter()
                    .map(|f| (f.id.clone(), f.nombre.clone()))
                    .collect(),
            );
        }
        Toque::Proyecto(i) => {
            if let Fase::Proyectos(lista) = &s.fase
                && let Some((id, _)) = lista.get(i)
            {
                let id = id.clone();
                s.fase = Fase::Preparando;
                preparar(cx, s.turno, Que::Proyecto(id));
            }
        }
        Toque::Aprobar(id) => responder(s, id, true),
        Toque::NoAprobar(id) => responder(s, id, false),
        Toque::AprobarTodos => {
            let esperan: Vec<u64> = s
                .destinos
                .iter()
                .filter(|d| d.en == EnDestino::Pidiendo)
                .map(|d| d.id)
                .collect();
            for id in esperan {
                responder(s, id, true);
            }
        }
        Toque::Pegar => s.pegando = true,
        Toque::DejarDePegar => s.pegando = false,
        Toque::Campo => {}
        Toque::Mandar => {
            if let Some(q) = s.a_quien()
                && s.llamados.insert(q.codigo.clone())
            {
                mandar_a(s, q, cx);
                s.tecleado.clear();
            }
        }
        Toque::Cerrar => return true,
    }
    false
}

/// La respuesta a un aparato que espera: suelta el hilo que lo atiende.
fn responder(s: &mut Enviar, id: u64, si: bool) {
    if let Some(a) = s.aprobaciones.remove(&id) {
        let _ = a.send(si);
    }
    if let Some(d) = s.destinos.iter_mut().find(|d| d.id == id) {
        d.en = if si {
            EnDestino::Enviando {
                hechos: 0,
                total: 0,
            }
        } else {
            EnDestino::SinAprobar
        };
    }
}

// --------------------------------------------------------------- que

enum Que {
    Archivos(Vec<PathBuf>),
    Proyecto(String),
}

/// El tipo de un archivo por su extension, para el `mime` de la oferta: el
/// movil lo usa para decidir como guardarlo. Lo que no se reconoce va sin.
pub(super) fn mime_de(nombre: &str) -> Option<&'static str> {
    let ext = nombre.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "md" => "text/markdown",
        "html" | "htm" => "text/html",
        "json" => "application/json",
        "zip" | "pixpin" => "application/zip",
        "excalidraw" => "application/vnd.excalidraw+json",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "wav" => "audio/wav",
        _ => return None,
    })
}

/// Un archivo del disco tal cual, como `deUris` del movil. No se copia: se
/// manda desde donde esta, y si cambia mientras sale, el emisor lo nota.
fn de_archivo(ruta: &Path, yo: &str) -> Option<(Elemento, PathBuf)> {
    let meta = std::fs::metadata(ruta).ok().filter(|m| m.is_file())?;
    let nombre = envio::nombre_sano(&ruta.file_name()?.to_string_lossy());
    Some((
        Elemento {
            tipo: envio::ARCHIVO.into(),
            mime: mime_de(&nombre).map(str::to_string),
            identidad: format!("archivo:{yo}:{nombre}"),
            nombre,
            bytes: meta.len() as i64,
            proyecto: None,
            proyecto_nombre: None,
            creado: 0,
            uid: None,
            codigo_de_chat: None,
            aparato: None,
        },
        ruta.to_path_buf(),
    ))
}

/// Un proyecto de la lista, empaquetado en `carpeta` (`deProyecto`).
fn de_proyecto(raiz: &Path, id: &str, carpeta: &Path) -> Result<(Elemento, PathBuf), String> {
    let bytes =
        pixpin_proyecto::almacen::empaquetar(raiz, id, pixpin_shell::entorno::ahora_local_ms())
            .map_err(|e| e.to_string())?;
    let p = pixpin_proyecto::Paquete::desde_bytes(&bytes).map_err(|e| e.to_string())?;
    let pr = &p.proyecto;
    // Su sena entre envios, la de `Recepcion.identidadDe`: el origen si vino
    // de otro sitio, su id si nacio aqui.
    let origen = pr
        .resto
        .get("origen")
        .and_then(|v| v.as_str())
        .unwrap_or(&pr.id);
    let destino = carpeta.join(format!("{}.pixpin", envio::nombre_sano(&pr.nombre)));
    std::fs::create_dir_all(carpeta).map_err(|e| e.to_string())?;
    std::fs::write(&destino, &bytes).map_err(|e| e.to_string())?;
    Ok((
        Elemento {
            tipo: envio::PROYECTO.into(),
            nombre: pr.nombre.clone(),
            bytes: bytes.len() as i64,
            mime: Some("application/zip".into()),
            identidad: format!("proyecto:{origen}"),
            proyecto: None,
            proyecto_nombre: None,
            creado: pr.creado,
            uid: pr.uid.clone(),
            codigo_de_chat: None,
            aparato: pr.aparato.clone(),
        },
        destino,
    ))
}

/// Prepara en un hilo: empaquetar un proyecto grande no puede congelar la
/// ventana.
fn preparar(cx: &Contexto, turno: u64, que: Que) {
    let cx = cx.clone();
    let _ = std::thread::Builder::new()
        .name("enviar-preparar".into())
        .spawn(move || {
            let hecho = match que {
                Que::Archivos(rutas) => {
                    let v: Vec<_> = rutas.iter().filter_map(|r| de_archivo(r, &cx.id)).collect();
                    if v.is_empty() {
                        Err("ningún archivo se pudo leer".to_string())
                    } else {
                        Ok((v, None))
                    }
                }
                Que::Proyecto(id) => {
                    let carpeta = cx
                        .raiz
                        .join("envio")
                        .join(pixpin_shell::entorno::ahora_local_ms().to_string());
                    de_proyecto(&cx.raiz, &id, &carpeta).map(|c| (vec![c], Some(carpeta)))
                }
            };
            avisar(&cx, turno, Novedad::Preparado(hecho));
        });
}

// ------------------------------------------------------------- la red

fn remitente(cx: &Contexto) -> Remitente {
    Remitente {
        nombre: cx.nombre.clone(),
        id: cx.id.clone(),
        codigo: pixpin_proyecto::codigos::de_aparato(&cx.id),
    }
}

/// Abre la puerta (`abrirLaPuerta`): el codigo, la escucha, el QR y, en un
/// hilo, el anuncio y la espera. Cada llamada se atiende en su hilo, en
/// paralelo, porque la de uno puede quedarse esperando a que se le apruebe.
fn abrir_la_puerta(s: &Enviar, cx: &Contexto) -> anyhow::Result<Fase> {
    use anyhow::Context as _;
    let codigo = crate::recibir::codigo_nuevo().context("el sistema no dio azar")?;
    let escucha = TcpListener::bind(("0.0.0.0", 0))?;
    escucha.set_nonblocking(true)?;
    let puerto = escucha.local_addr()?.port();
    let ip = crate::recibir::ip_local().unwrap_or_default();
    let qr = crate::recibir::qr_de(&envio::texto_del_qr(&codigo, &ip, puerto));
    let (cx, turno, vivo, cosas, siguiente) = (
        cx.clone(),
        s.turno,
        s.vivo.clone(),
        s.cosas.clone(),
        s.siguiente.clone(),
    );
    let para_hilo = codigo.clone();
    std::thread::Builder::new()
        .name("enviar-puerta".into())
        .spawn(move || {
            let codigo = para_hilo;
            let etiqueta = envio::etiqueta(&codigo);
            let corto: String = cx.nombre.chars().take(40).collect();
            let anuncio = pixpin_shell::mdns::anunciar(
                envio::SERVICIO,
                &format!("PixPin {}", cx.nombre),
                puerto,
                &[("g", &etiqueta), ("n", &corto)],
            );
            if let Err(e) = &anuncio {
                tracing::warn!(?e, "no se pudo anunciar el envio");
            }
            let malos = Arc::new(AtomicU32::new(0));
            while vivo.load(Ordering::SeqCst) {
                match escucha.accept() {
                    Ok((flujo, de)) => {
                        tracing::info!(%de, "alguien llama para recibir");
                        let _ = flujo.set_nonblocking(false);
                        let _ = flujo.set_read_timeout(Some(ESPERA_LARGA));
                        let (cx, codigo, cosas, siguiente, malos, vivo) = (
                            cx.clone(),
                            codigo.clone(),
                            cosas.clone(),
                            siguiente.clone(),
                            malos.clone(),
                            vivo.clone(),
                        );
                        let _ = std::thread::Builder::new()
                            .name("enviar-atender".into())
                            .spawn(move || {
                                atender(
                                    flujo, &codigo, &cx, turno, &cosas, &siguiente, &malos, &vivo,
                                )
                            });
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(150));
                    }
                    Err(e) => {
                        tracing::warn!(?e, "la puerta del envio fallo");
                        return;
                    }
                }
            }
            drop(anuncio);
        })?;
    Ok(Fase::Esperando {
        codigo,
        qr,
        malos: 0,
    })
}

/// Una llamada entera de quien quiere recibir, con la aprobacion en medio.
#[allow(clippy::too_many_arguments)]
fn atender(
    flujo: TcpStream,
    codigo: &str,
    cx: &Contexto,
    turno: u64,
    cosas: &[(Elemento, PathBuf)],
    siguiente: &AtomicU64,
    malos: &AtomicU32,
    vivo: &AtomicBool,
) {
    let Some(n) = nonce() else {
        return;
    };
    let yo = remitente(cx);
    // Una celda: lo apunta el primer cierre y lo leen el segundo y el final.
    let conocido = std::cell::Cell::new(None::<u64>);
    let mut ultimo = Instant::now();
    let final_ = Emisor::nuevo(&yo, cosas).atender(
        flujo,
        codigo,
        n,
        false,
        |nombre| {
            let id = siguiente.fetch_add(1, Ordering::SeqCst);
            conocido.set(Some(id));
            let (aprueba, respuesta) = mpsc::channel();
            let como = if nombre.trim().is_empty() {
                "Un aparato".to_string()
            } else {
                nombre.to_string()
            };
            if !avisar(
                cx,
                turno,
                Novedad::Llega {
                    id,
                    nombre: como,
                    aprueba,
                },
            ) {
                return false;
            }
            // Aqui se para este hilo hasta que se toque «Enviar» o «No»;
            // cerrar la pantalla suelta el canal y cuenta como «no».
            respuesta.recv_timeout(ESPERA_LARGA) == Ok(true)
        },
        |hechos, total| {
            if ultimo.elapsed() > Duration::from_millis(120) || hechos == total {
                ultimo = Instant::now();
                if let Some(id) = conocido.get() {
                    avisar(
                        cx,
                        turno,
                        Novedad::Destino {
                            id,
                            nombre: None,
                            en: EnDestino::Enviando { hechos, total },
                        },
                    );
                }
            }
        },
    );
    match (final_, conocido.get()) {
        (Ok(f), Some(id)) => {
            let en = match f {
                Final::Enviado => EnDestino::Hecho,
                Final::Rechazado => EnDestino::Rechazado,
                Final::SinAprobar => EnDestino::SinAprobar,
            };
            avisar(
                cx,
                turno,
                Novedad::Destino {
                    id,
                    nombre: None,
                    en,
                },
            );
        }
        (Err(e), _) if e.es_codigo_distinto() => {
            let n = malos.fetch_add(1, Ordering::SeqCst) + 1;
            if n >= envio::INTENTOS {
                // Quemado: la puerta se cierra ya, no cuando se entere la
                // ventana.
                vivo.store(false, Ordering::SeqCst);
            }
            avisar(cx, turno, Novedad::Malo(n));
        }
        (Err(e), Some(id)) => {
            avisar(
                cx,
                turno,
                Novedad::Destino {
                    id,
                    nombre: None,
                    en: EnDestino::Fallo(e.to_string()),
                },
            );
        }
        // Se corto antes de saber quien era: nadie que ensenar en la lista.
        (r, None) => tracing::info!(?r, "una llamada al envio acabo antes de presentarse"),
    }
}

/// Mandarle a quien espera con su codigo (`mandarA` del movil): aqui los
/// papeles se cambian y ESTE aparato llama. Pegar su codigo es la
/// aprobacion: no se vuelve a preguntar.
fn mandar_a(s: &mut Enviar, q: envio::DelQr, cx: &Contexto) {
    if s.cosas.is_empty() {
        return;
    }
    let id = s.siguiente.fetch_add(1, Ordering::SeqCst);
    s.destinos.push(Destino {
        id,
        nombre: format!("Aparato {}", envio::legible(&q.codigo)),
        en: EnDestino::Enviando {
            hechos: 0,
            total: 0,
        },
        llamado: Some(q.codigo.clone()),
    });
    let (cx, turno, cosas, vivo) = (cx.clone(), s.turno, s.cosas.clone(), s.vivo.clone());
    let _ = std::thread::Builder::new()
        .name("enviar-llamar".into())
        .spawn(move || {
            let cambiar = |nombre: Option<String>, en: EnDestino| {
                avisar(&cx, turno, Novedad::Destino { id, nombre, en });
            };
            let flujo = match (&q.host, q.puerto) {
                (Some(h), p) if p > 0 => conectar(h, p).ok(),
                _ => None,
            }
            .or_else(|| buscar_y_conectar(envio::SERVICIO_RECIBIR, &q.codigo, &vivo));
            let Some(flujo) = flujo else {
                cambiar(None, EnDestino::Fallo(NO_ESPERA.into()));
                return;
            };
            let Some(n) = nonce() else {
                return;
            };
            let yo = remitente(&cx);
            let mut ultimo = Instant::now();
            let hecho = Emisor::nuevo(&yo, &cosas).atender(
                flujo,
                &q.codigo,
                n,
                true,
                |nombre| {
                    cambiar(
                        Some(nombre.to_string()),
                        EnDestino::Enviando {
                            hechos: 0,
                            total: 0,
                        },
                    );
                    true
                },
                |hechos, total| {
                    if ultimo.elapsed() > Duration::from_millis(120) || hechos == total {
                        ultimo = Instant::now();
                        cambiar(None, EnDestino::Enviando { hechos, total });
                    }
                },
            );
            cambiar(
                None,
                match hecho {
                    Ok(Final::Enviado) => EnDestino::Hecho,
                    Ok(_) => EnDestino::Rechazado,
                    Err(e) => EnDestino::Fallo(e.to_string()),
                },
            );
        });
}

/// El motivo cuando nadie contesta al codigo pegado. Va sin traducir porque
/// sale del hilo, como el resto de motivos de «Se cortó: ...».
const NO_ESPERA: &str = "no apareció nadie esperando con ese código en esta Wi-Fi";

// ------------------------------------------------------------------ pintar

fn con(textos: &Catalogo, clave: &str, pares: &[(&str, String)]) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    for (k, v) in pares {
        args.set(*k, v.clone());
    }
    textos.t_args(clave, &args)
}

pub(super) fn pintar(
    l: &mut Lienzo<'_>,
    textos: &Catalogo,
    s: &Enviar,
    x: f32,
    w: f32,
    mut y: f32,
) -> f32 {
    let (p, e) = (l.p, l.e);
    let t = Accion::Enviar;
    y += 8.0 * e;
    match &s.fase {
        Fase::Eligiendo => {
            y += 12.0 * e;
            l.centrado(&textos.t("ew-que"), x, w, y, 18.0 * e, TEXTO);
            y += 40.0 * e;
            l.boton(
                rect(x, y, w, 48.0 * e),
                &textos.t("ew-archivos"),
                Some(&material::ATTACH_FILE),
                true,
                t(Toque::Archivos),
            );
            y += 56.0 * e;
            l.boton(
                rect(x, y, w, 48.0 * e),
                &textos.t("ew-un-proyecto"),
                Some(&material::FORUM),
                false,
                t(Toque::UnProyecto),
            );
            y += 64.0 * e;
            y += l.parrafo_centrado(&textos.t("ew-que-como"), x, y, w, 12.0 * e, SUAVE);
        }
        Fase::Proyectos(lista) => {
            l.centrado(&textos.t("ew-elige-proyecto"), x, w, y, 16.0 * e, TEXTO);
            y += 30.0 * e;
            if lista.is_empty() {
                y += l.parrafo_centrado(&textos.t("ew-sin-proyectos"), x, y, w, 14.0 * e, SUAVE);
            } else {
                let fila = 48.0 * e;
                let h = fila * lista.len() as f32;
                caja(p, rect(x, y, w, h), e);
                for (i, (_, nombre)) in lista.iter().enumerate() {
                    let r = rect(x, y + fila * i as f32, w, fila);
                    if i > 0 {
                        p.rellenar(rect(x + 16.0 * e, r.y, w - 32.0 * e, e.max(1.0)), BORDE);
                    }
                    p.icono(
                        &material::FORUM,
                        rect(x + 16.0 * e, r.y + 12.0 * e, 24.0 * e, 24.0 * e),
                        PRIMARIO,
                    );
                    p.texto_linea(
                        nombre,
                        x + 52.0 * e,
                        r.y + 14.0 * e,
                        15.0 * e,
                        w - 68.0 * e,
                        TEXTO,
                    );
                    l.zonas.push((r, t(Toque::Proyecto(i))));
                }
                y += h;
            }
        }
        Fase::Preparando => {
            y += 20.0 * e;
            l.girando(x + w / 2.0, y + 20.0 * e, 18.0 * e, 4.0 * e);
            y += 54.0 * e;
            l.centrado(&textos.t("ew-preparando"), x, w, y, 14.0 * e, TEXTO);
            y += 24.0 * e;
        }
        Fase::Esperando { codigo, qr, malos } => {
            y = lo_que_se_envia(l, textos, s, x, w, y);
            y += 16.0 * e;
            y = los_aparatos(l, textos, s, x, w, y);
            y = pegar(l, textos, s, x, w, y);
            l.centrado(&textos.t("ew-en-el-otro"), x, w, y, 16.0 * e, TEXTO);
            y += 34.0 * e;
            let lado = w.min(300.0 * e);
            let rq = rect(x + (w - lado) / 2.0, y, lado, lado);
            p.rellenar_redondeado(rq, 18.0 * e, hex_blanco());
            if let Some(qr) = qr {
                crate::recibir::pintar_qr(p, menos(rq, 14.0 * e), qr);
            }
            y += lado + 14.0 * e;
            l.centrado(&textos.t("ew-o-escribe"), x, w, y, 14.0 * e, SUAVE);
            y += 22.0 * e;
            l.centrado(&envio::legible(codigo), x, w, y, 40.0 * e, TEXTO);
            y += 60.0 * e;
            y += caja_wifi(
                l,
                x,
                y,
                w,
                Some(&textos.t("ew-misma-wifi")),
                &textos.t("ew-misma-wifi-como"),
            );
            if *malos > 0 {
                y += 8.0 * e;
                y += l.parrafo(
                    &con(
                        textos,
                        "ew-malos",
                        &[
                            ("n", malos.to_string()),
                            ("de", envio::INTENTOS.to_string()),
                        ],
                    ),
                    x,
                    y,
                    w,
                    12.0 * e,
                    ERROR,
                );
            }
            y += 12.0 * e;
            l.girando(x + 8.0 * e, y + 8.0 * e, 6.0 * e, 2.0 * e);
            y += l.parrafo(
                &textos.t("ew-esperando"),
                x + 24.0 * e,
                y,
                w - 24.0 * e,
                12.0 * e,
                TEXTO,
            );
            if s.destinos.iter().any(|d| d.en == EnDestino::Hecho) {
                y += 20.0 * e;
                l.boton(
                    rect(x, y, w, 48.0 * e),
                    &textos.t("ew-cerrar"),
                    None,
                    true,
                    t(Toque::Cerrar),
                );
                y += 48.0 * e;
            }
        }
        Fase::Fallo(f) => {
            y += 20.0 * e;
            l.centrado(&textos.t("rw-no-se-pudo"), x, w, y, 20.0 * e, TEXTO);
            y += 34.0 * e;
            let texto = match f {
                Falla::NoPreparado => textos.t("ew-no-preparado"),
                Falla::Quemado => textos.t("ew-quemado"),
                Falla::Cortado(m) => con(textos, "rw-se-corto", &[("motivo", m.clone())]),
            };
            y += l.parrafo_centrado(&texto, x, y, w, 14.0 * e, SUAVE);
            y += 20.0 * e;
            l.boton(
                rect(x, y, w, 48.0 * e),
                &textos.t("ew-cerrar"),
                None,
                true,
                t(Toque::Cerrar),
            );
            y += 48.0 * e;
        }
    }
    y
}

/// `LoQueSeEnvia`: la caja con lo que va a salir.
fn lo_que_se_envia(
    l: &mut Lienzo<'_>,
    textos: &Catalogo,
    s: &Enviar,
    x: f32,
    w: f32,
    y: f32,
) -> f32 {
    let (p, e) = (l.p, l.e);
    let n = s.cosas.len();
    if n == 0 {
        return y;
    }
    let fila = 24.0 * e;
    let visibles = n.min(6);
    let h = 16.0 * e
        + 18.0 * e
        + fila * visibles as f32
        + if n > 6 { 20.0 * e } else { 0.0 }
        + 12.0 * e;
    caja(p, rect(x, y, w, h), e);
    let rotulo = if n == 1 {
        textos.t("ew-vas-a-enviar")
    } else {
        con(textos, "ew-vas-a-enviar-n", &[("n", n.to_string())])
    };
    p.texto(&rotulo, x + 16.0 * e, y + 14.0 * e, 12.0 * e, SUAVE);
    let mut yf = y + 38.0 * e;
    for (el, _) in s.cosas.iter().take(6) {
        let peso = tamano(el.bytes.max(0) as u64);
        let (wp, _) = p.medir_texto(&peso, 12.0 * e);
        p.texto(&peso, x + w - 16.0 * e - wp, yf + 2.0 * e, 12.0 * e, TEXTO);
        p.texto_linea(
            &nombre_de(textos, &el.tipo, &el.nombre),
            x + 16.0 * e,
            yf,
            15.0 * e,
            w - 44.0 * e - wp,
            TEXTO,
        );
        yf += fila;
    }
    if n > 6 {
        p.texto(
            &con(textos, "ew-y-mas", &[("n", (n - 6).to_string())]),
            x + 16.0 * e,
            yf,
            12.0 * e,
            TEXTO,
        );
    }
    y + h
}

/// `LosAparatos`: los que se conectaron, y aprobar a cada uno.
fn los_aparatos(l: &mut Lienzo<'_>, textos: &Catalogo, s: &Enviar, x: f32, w: f32, y: f32) -> f32 {
    let (p, e) = (l.p, l.e);
    if s.destinos.is_empty() {
        return y;
    }
    let fila = 60.0 * e;
    let esperan = s
        .destinos
        .iter()
        .filter(|d| d.en == EnDestino::Pidiendo)
        .count();
    let h = 16.0 * e
        + 20.0 * e
        + fila * s.destinos.len() as f32
        + if esperan > 1 { 56.0 * e } else { 0.0 }
        + 10.0 * e;
    caja(p, rect(x, y, w, h), e);
    let rotulo = if s.destinos.len() == 1 {
        textos.t("ew-un-aparato")
    } else {
        con(
            textos,
            "ew-n-aparatos",
            &[("n", s.destinos.len().to_string())],
        )
    };
    p.texto(&rotulo, x + 16.0 * e, y + 14.0 * e, 12.0 * e, SUAVE);
    let mut yf = y + 40.0 * e;
    let t = Accion::Enviar;
    for d in &s.destinos {
        let botones = if d.en == EnDestino::Pidiendo {
            150.0 * e
        } else {
            0.0
        };
        let wt = w - 32.0 * e - botones;
        p.texto_linea(&d.nombre, x + 16.0 * e, yf, 15.0 * e, wt, TEXTO);
        let (estado, color) = match &d.en {
            EnDestino::Pidiendo => (textos.t("ew-pidiendo"), SUAVE),
            EnDestino::Enviando { hechos, total } if *total > 0 => (
                con(
                    textos,
                    "ew-enviando",
                    &[("hechos", tamano(*hechos)), ("total", tamano(*total))],
                ),
                SUAVE,
            ),
            EnDestino::Enviando { .. } => (textos.t("ew-esperando-acepte"), SUAVE),
            EnDestino::Hecho => (textos.t("ew-enviado"), VERDE),
            EnDestino::Rechazado => (textos.t("ew-no-acepto"), SUAVE),
            EnDestino::SinAprobar => (textos.t("ew-no-aprobado"), SUAVE),
            EnDestino::Fallo(m) => (con(textos, "rw-se-corto", &[("motivo", m.clone())]), SUAVE),
        };
        p.texto_linea(&estado, x + 16.0 * e, yf + 22.0 * e, 12.0 * e, wt, color);
        if let EnDestino::Enviando { hechos, total } = d.en
            && total > 0
        {
            l.barra(
                rect(x + 16.0 * e, yf + 42.0 * e, wt, 4.0 * e),
                hechos as f32 / total as f32,
            );
        }
        if d.en == EnDestino::Pidiendo {
            let xb = x + w - 16.0 * e - botones;
            l.boton_texto(
                xb,
                yf,
                &textos.t("ew-no"),
                PRIMARIO,
                t(Toque::NoAprobar(d.id)),
            );
            l.boton(
                rect(xb + 60.0 * e, yf, botones - 60.0 * e, 40.0 * e),
                &textos.t("ew-enviar"),
                None,
                true,
                t(Toque::Aprobar(d.id)),
            );
        }
        yf += fila;
    }
    if esperan > 1 {
        l.boton(
            rect(x + 16.0 * e, yf + 6.0 * e, w - 32.0 * e, 44.0 * e),
            &textos.t("ew-enviar-a-todos"),
            None,
            true,
            t(Toque::AprobarTodos),
        );
    }
    y + h + 16.0 * e
}

/// `ElEscaner`, sin camara: pegar el codigo de quien espera recibir.
fn pegar(l: &mut Lienzo<'_>, textos: &Catalogo, s: &Enviar, x: f32, w: f32, mut y: f32) -> f32 {
    let e = l.e;
    let t = Accion::Enviar;
    if !s.pegando {
        l.boton(
            rect(x, y, w, 48.0 * e),
            &textos.t("ew-pegar"),
            Some(&material::CONTENT_PASTE),
            false,
            t(Toque::Pegar),
        );
        return y + 64.0 * e;
    }
    l.centrado(&textos.t("ew-pegar"), x, w, y, 16.0 * e, TEXTO);
    y += 26.0 * e;
    y += l.parrafo_centrado(&textos.t("ew-pegar-como"), x, y, w, 12.0 * e, SUAVE);
    y += 10.0 * e;
    let wc = w.min(260.0 * e);
    campo_grande(
        l,
        rect(x + (w - wc) / 2.0, y, wc, 64.0 * e),
        &s.tecleado,
        "000 000",
        t(Toque::Campo),
    );
    y += 76.0 * e;
    let rb = rect(x, y, w, 48.0 * e);
    if s.a_quien().is_some() {
        l.boton(rb, &textos.t("ew-mandar"), None, true, t(Toque::Mandar));
    } else {
        boton_apagado(l, rb, &textos.t("ew-mandar"));
    }
    y += 52.0 * e;
    let wd = l.p.medir_texto(&textos.t("ew-dejar-de-pegar"), 14.0 * e).0 + 24.0 * e;
    l.boton_texto(
        x + (w - wd) / 2.0,
        y,
        &textos.t("ew-dejar-de-pegar"),
        PRIMARIO,
        t(Toque::DejarDePegar),
    );
    y + 56.0 * e
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_tipo_de_un_archivo_sale_de_su_extension_y_lo_raro_va_sin() {
        assert_eq!(mime_de("Plano.PDF"), Some("application/pdf"));
        assert_eq!(mime_de("foto.jpeg"), Some("image/jpeg"));
        assert_eq!(mime_de("casa.pixpin"), Some("application/zip"));
        assert_eq!(mime_de("sin-extension"), None);
        assert_eq!(mime_de("cosa.xyz"), None);
    }

    #[test]
    fn para_mandar_vale_el_codigo_de_quien_espera_y_no_el_de_quien_envia() {
        let mut s = Enviar::nuevo(1);
        s.tecleado = "482 913".into();
        assert!(s.a_quien().is_some());
        s.tecleado = "pixpin-recibe:1:482913:192.168.1.4:47474".into();
        assert!(s.a_quien().is_some_and(|q| q.espera_recibir));
        // Caso negativo: el QR de otro que envia no es a quien mandar.
        s.tecleado = "pixpin-envio:1:482913:192.168.1.4:47474".into();
        assert!(s.a_quien().is_none());
        s.tecleado = "12".into();
        assert!(s.a_quien().is_none());
    }

    #[test]
    fn un_archivo_se_ofrece_con_su_nombre_su_tamano_y_su_sena() {
        let d = std::env::temp_dir().join(format!("pixpin-enviar-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        let ruta = d.join("plano.pdf");
        std::fs::write(&ruta, b"12345").unwrap();
        let (el, r) = de_archivo(&ruta, "pc").expect("se lee");
        assert_eq!((el.nombre.as_str(), el.bytes), ("plano.pdf", 5));
        assert_eq!(el.identidad, "archivo:pc:plano.pdf");
        assert_eq!(el.mime.as_deref(), Some("application/pdf"));
        assert_eq!(r, ruta);
        // Caso negativo: una carpeta o algo que no esta no se ofrece.
        assert!(de_archivo(&d, "pc").is_none());
        assert!(de_archivo(&d.join("no-esta.pdf"), "pc").is_none());
    }

    #[test]
    fn cinco_codigos_malos_queman_el_codigo_y_cierran_la_puerta() {
        let (tx, _rx) = mpsc::channel();
        let cx = Contexto {
            raiz: std::env::temp_dir(),
            nombre: "PC".into(),
            id: "pc".into(),
            tx,
        };
        let mut s = Enviar::nuevo(1);
        s.fase = Fase::Esperando {
            codigo: "482913".into(),
            qr: None,
            malos: 0,
        };
        novedad(&mut s, Novedad::Malo(4), &cx);
        assert!(matches!(s.fase, Fase::Esperando { malos: 4, .. }));
        assert!(s.vivo.load(Ordering::SeqCst), "con cuatro sigue abierta");
        novedad(&mut s, Novedad::Malo(envio::INTENTOS), &cx);
        assert!(matches!(s.fase, Fase::Fallo(Falla::Quemado)));
        assert!(!s.vivo.load(Ordering::SeqCst));
    }

    #[test]
    fn decir_que_no_suelta_al_hilo_que_espera_con_un_no() {
        let mut s = Enviar::nuevo(1);
        let (aprueba, respuesta) = mpsc::channel();
        s.aprobaciones.insert(7, aprueba);
        s.destinos.push(Destino {
            id: 7,
            nombre: "Movil".into(),
            en: EnDestino::Pidiendo,
            llamado: None,
        });
        responder(&mut s, 7, false);
        assert_eq!(respuesta.recv().ok(), Some(false));
        assert_eq!(s.destinos[0].en, EnDestino::SinAprobar);
    }
}
