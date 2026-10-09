//! Sincronizar: tus aparatos, y pasar algo a otra persona.
//!
//! Copia de la pantalla del movil (`sincro/SincronizarActivity.kt` de PixPin
//! Android, v0.51.0): las mismas cajas en el mismo orden —«Este aparato»,
//! «Tus aparatos», «Anadir otro aparato», «Pasar algo a otra persona»—, los
//! mismos textos, los mismos iconos de Material y los colores de su tema de
//! noche. El usuario compara las dos pantallas una al lado de la otra: lo que
//! aqui se aparte de alli lo va a ver.
//!
//! Habla con el movil por el protocolo de verdad (`pixpin-sincro`): crear un
//! grupo, unirse con un codigo, aceptar a quien se une, la sonda `PING`/`PONG`
//! que dice quien esta disponible, encontrarse por la red con el mismo
//! anuncio mDNS que el movil (`_pixpin._tcp` con `g`, `id`, `l`, `n`) y
//! **juntar los chats y sus archivos en los dos sentidos**: cuando el movil
//! llama, este equipo responde con el `Respondedor`; cuando se pulsa
//! «Sincronizar» aqui, se elige que en «¿Que sincronizar?» (`elegir`) y la
//! `Sesion` lleva la vuelta. Lo de este equipo lo ve el protocolo a traves
//! de `pixpin_proyecto::vista::DiscoPc`, sin cambiar como se guarda.
//!
//! «Recibir» y «Enviar» de «Pasar algo a otra persona» abren sus pantallas
//! aqui mismo (`recibir_wifi`, `enviar_wifi`), con la flecha de volver
//! arriba: en el movil todo eso cuelga de Sincronizar, y aqui tambien.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use anyhow::{Context, Result};
use pixpin_geom::{Punto, Rect};
use pixpin_proyecto::identidad::{Aparato, Identidad};
use pixpin_proyecto::vista::DiscoPc;
use pixpin_render::icono::{Icono, Pintura, TrazoIcono};
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_sincro::canal::{Canal, Tipo};
use pixpin_sincro::disco::Disco;
use pixpin_sincro::mensajes::{self as m, Peticion, Respuesta};
use pixpin_sincro::protocolo::{ErrorSincro, Hecho, Respondedor, Sesion};
use pixpin_sincro::vuelta;
use pixpin_store::{Catalogo, Ubicacion};

use crate::caja_dibujo::hex;
use crate::overlay::Recursos;

mod al_dia;
pub(crate) mod automatica;
pub(crate) mod al_movil;
mod copias_ui;
mod elegir;
pub(crate) mod en_fondo;
mod enviar_wifi;
mod galeria;
pub(crate) mod presencia;
mod recibir_wifi;

const ANCHO_LOGICO: u32 = 480;
const ALTO_LOGICO: u32 = 760;
/// La barra de arriba: por donde se arrastra la ventana, como la del chat.
const BARRA: f32 = 56.0;

// El tema de noche del movil (`ui/theme/Theme.kt`, `DarkColors`), valor por
// valor. Los fondos no son grises sino del tono de la marca, y el texto no es
// blanco puro: alli se explica por que, y aqui se copia sin rehacerlo.
const FONDO: Color = hex(0x0e1416);
const CAJA: Color = hex(0x141b1d);
const BORDE: Color = hex(0x344145);
const TEXTO: Color = hex(0xe2e9eb);
const SUAVE: Color = hex(0xb7c3c7);
const PRIMARIO: Color = hex(0x5cd3f0);
const SOBRE_PRIMARIO: Color = hex(0x00363f);
const CONTENEDOR: Color = hex(0x0c4e5c);
const SOBRE_CONTENEDOR: Color = hex(0xc6f1fb);
const VARIANTE: Color = hex(0x212b2e);
const CONTORNO: Color = hex(0x7f8e93);
const ERROR: Color = hex(0xffb4ab);
/// `surfaceContainer`: el fondo de los dialogos.
const DIALOGO: Color = hex(0x192123);
/// Lo que se pone sobre la pantalla cuando hay un dialogo: se sigue viendo lo
/// de debajo, pero deja de tocarse.
const VELO: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.55,
};
/// `VERDE` de `SincronizarActivity.kt`.
const VERDE: Color = hex(0x2e9e4f);
/// `ROJO` de `SincronizarActivity.kt`: lo que solo esta en un aparato.
const ROJO: Color = hex(0xe0453a);

/// Solo una ventana de sincronizar a la vez: dos escuchando en el mismo
/// puerto y escribiendo la misma identidad se pisarian.
///
/// Es el `HWND` de la ventana abierta (> 0), -1 mientras nace y 0 si no hay,
/// como el del chat y el del universo. Un booleano bastaba para NO abrir dos,
/// pero no para despertar a la que ya esta: sin el handle, un pedido que
/// llegara con la ventana delante se quedaria en la cola hasta que el usuario
/// moviera el raton por encima.
static ABIERTA: AtomicIsize = AtomicIsize::new(0);

/// Lo que se le pide a la ventana de Sincronizar desde fuera (hoy, mandar
/// unos ficheros por Wi-Fi desde el menu de un mensaje del chat).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pedido {
    /// Entrar directo en «Enviar por Wi-Fi» con estos ficheros ya puestos,
    /// sin pasar por «¿Que mandar?».
    Enviar(Vec<PathBuf>),
    /// Entrar directo en «Copias de seguridad», que arriba tiene «Proyectos
    /// borrados»: la papelera que los recupera enteros.
    // La pide `abrir_papelera` (menu del clic derecho de un proyecto).
    Copias,
}

/// El pedido que espera a que la ventana lo recoja. Solo cabe uno: si se
/// piden dos seguidos gana el ultimo, que es el que el usuario acaba de
/// pulsar (mismo trato que `universo::PEDIDO`).
static PEDIDO: std::sync::Mutex<Option<Pedido>> = std::sync::Mutex::new(None);

fn dejar_pedido(p: Pedido) {
    if let Ok(mut g) = PEDIDO.lock() {
        *g = Some(p);
    }
}

fn tomar_pedido() -> Option<Pedido> {
    PEDIDO.lock().ok().and_then(|mut g| g.take())
}

/// Abre la ventana en su propio hilo, como el chat y recibir: el hilo
/// principal sigue atendiendo atajos y gestos.
pub fn lanzar(idioma: pixpin_store::Idioma, ubicacion: Ubicacion) {
    con_pedido(idioma, ubicacion, None)
}

/// Manda estos ficheros por Wi-Fi: abre Sincronizar YA en la pantalla de
/// enviar con ellos puestos, o se lo pide a la que ya esta abierta.
pub fn enviar_por_wifi(idioma: pixpin_store::Idioma, ubicacion: Ubicacion, rutas: Vec<PathBuf>) {
    if rutas.is_empty() {
        return;
    }
    con_pedido(idioma, ubicacion, Some(Pedido::Enviar(rutas)))
}

/// **La papelera de proyectos**: abre Sincronizar YA en «Copias de
/// seguridad», con «Proyectos borrados» arriba (H6, v0.79 del movil). No
/// hace falta tener ningun proyecto abierto: es para llamarla desde la lista
/// de proyectos del chat o desde la bandeja.
// Sin llamar todavia: el boton va en la lista de proyectos del chat
// (`ventana_chat.rs`), que es de otro frente. Hasta entonces se llega por
// Sincronizar → Copias de seguridad.
pub fn abrir_papelera(idioma: pixpin_store::Idioma, ubicacion: Ubicacion) {
    con_pedido(idioma, ubicacion, Some(Pedido::Copias))
}

fn con_pedido(idioma: pixpin_store::Idioma, ubicacion: Ubicacion, pedido: Option<Pedido>) {
    if let Some(p) = pedido {
        dejar_pedido(p);
    }
    let ya = ABIERTA.load(Ordering::SeqCst);
    if ya != 0 {
        if ya > 0 {
            VentanaOverlay::restaurar_de_hwnd(windows::Win32::Foundation::HWND(ya as *mut _));
            pixpin_shell::overlay::despertar(ya);
        }
        // Naciendo (-1): su bucle recoge el pedido en la primera vuelta.
        return;
    }
    // Se marca antes de que exista la ventana: dos peticiones seguidas no
    // pueden abrir dos Sincronizar. El hilo pone el HWND de verdad al crearla.
    ABIERTA.store(-1, Ordering::SeqCst);
    let lanzado = std::thread::Builder::new()
        .name("sincronizar".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let textos = Catalogo::nuevo(idioma);
            let hecho = crate::dispositivo_perdido::con_recursos("sincronizar", |r| {
                abrir(r, &textos, &ubicacion)
            });
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo abrir la ventana de sincronizar");
            }
            ABIERTA.store(0, Ordering::SeqCst);
        });
    if let Err(e) = lanzado {
        ABIERTA.store(0, Ordering::SeqCst);
        tracing::warn!(?e, "no se pudo lanzar el hilo de sincronizar");
    }
}

// ------------------------------------------------------------------ estado

/// Lo que se ensena encima de la pantalla, como los dialogos del movil.
#[derive(Debug, Clone, PartialEq)]
enum Fase {
    Nada,
    Trabajando(String),
    /// Sincronizando: lo que hace, y la barra si se pasan archivos.
    Progreso(pixpin_sincro::vuelta::Trabajo),
    Terminado {
        titulo: String,
        texto: String,
        /// Lo que merece leerse aparte, en el color de los avisos.
        aviso: Option<String>,
        /// Si llego algo del otro lado (traido o juntado): la automatica
        /// solo avisa entonces.
        trajo: bool,
    },
    Fallo(String),
}

/// Cuantas vueltas de sincronizar hay en marcha ahora, en los dos sentidos
/// (las que pide este equipo y las que pide el movil al llamar). La
/// sincronizacion automatica no arranca mientras haya alguna: dos vueltas a
/// la vez sobre los mismos chats se pisarian.
static EN_CURSO: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

struct EnCurso;

impl EnCurso {
    fn nueva() -> EnCurso {
        EN_CURSO.fetch_add(1, Ordering::SeqCst);
        EnCurso
    }
}

impl Drop for EnCurso {
    fn drop(&mut self) {
        EN_CURSO.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Si hay alguna vuelta en marcha.
pub(crate) fn hay_vueltas() -> bool {
    EN_CURSO.load(Ordering::SeqCst) > 0
}

/// Lo que mandan los hilos de red a la ventana.
enum Aviso {
    Fase(Fase),
    /// Si un aparato recordado contesto a la sonda.
    Responde(String, bool),
    /// Alguien cambio `identidad.json`: se vuelve a leer.
    Identidad,
    /// Una linea para «Actividad».
    Registro(String),
    /// Lo que cuentan los hilos de «Recibir por Wi-Fi», con el turno de la
    /// pantalla que los lanzo: lo de una pantalla ya cerrada no pisa la nueva.
    Recibir(u64, recibir_wifi::Estado),
    Enviar(u64, enviar_wifi::Novedad),
    /// La vuelta pregunta que chats: se abre «¿Que sincronizar?» y el hilo
    /// espera la respuesta por `responde`.
    Elegir {
        otro: String,
        pregunta: pixpin_sincro::vuelta::Pregunta,
        responde: mpsc::Sender<Option<pixpin_sincro::vuelta::Eleccion>>,
    },
}

/// Lo que necesitan los hilos de recibir y enviar: donde vive todo, quien
/// soy y por donde avisar a la ventana.
#[derive(Clone)]
struct Contexto {
    raiz: PathBuf,
    nombre: String,
    id: String,
    tx: mpsc::Sender<Aviso>,
}

/// Una tecla para el campo de una sub-pantalla.
enum Tecla {
    Letra(char),
    Borrar,
    Intro,
    Pegar(String),
}

/// Que se ve: la portada o una de las pantallas que cuelgan de ella, como en
/// el movil cuelgan de Sincronizar «Recibir» y «Enviar».
enum Sub {
    Portada,
    Recibir(Box<recibir_wifi::Recibir>),
    Enviar(Box<enviar_wifi::Enviar>),
    Elegir(Box<elegir::Eligiendo>),
    /// «Copias de seguridad»: volver a como estaba antes de un envio o de
    /// una sincronizacion (`sincro/CopiasActivity.kt`).
    Copias(Box<copias_ui::Copias>),
}

/// Que se esta escribiendo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueCampo {
    Nombre,
    /// Unirse: el codigo y, debajo, la direccion.
    Codigo,
    Direccion,
}

#[derive(Debug, Clone)]
struct Campo {
    que: QueCampo,
    texto: String,
    /// Solo al unirse: el segundo campo, la direccion.
    extra: String,
    /// En cual de los dos se escribe.
    en_extra: bool,
}

impl Campo {
    fn nuevo(que: QueCampo, texto: String) -> Campo {
        Campo {
            que,
            texto,
            extra: String::new(),
            en_extra: false,
        }
    }
}

/// Lo que hace un clic en cada sitio. Se apunta al pintar, asi el sitio y
/// lo que hace no pueden desacompasarse.
#[derive(Debug, Clone, PartialEq)]
enum Accion {
    Cerrar,
    Renombrar,
    CrearGrupo,
    Unirme,
    /// Nombre, host y puerto del otro.
    SincronizarCon(String, String, u16),
    /// «Sincronizar con todos»: nombre, host y puerto de cada uno.
    SincronizarConTodos(Vec<(String, String, u16)>),
    Elegir(elegir::Toque),
    ConectarDireccion,
    VerCodigo,
    SalirDelGrupo,
    ConfirmarSalir,
    /// Los dos botones de «Pasar algo a otra persona».
    AbrirRecibir,
    AbrirEnviar,
    /// «Copias de seguridad», desde la portada.
    AbrirCopias,
    Copias(copias_ui::Toque),
    Enviar(enviar_wifi::Toque),
    /// La flecha de una sub-pantalla: a la portada.
    Volver,
    Recibir(recibir_wifi::Toque),
    Aceptar,
    Cancelar,
    CampoPrincipal,
    CampoExtra,
    CerrarFase,
}

struct Pantalla {
    raiz: PathBuf,
    identidad: Identidad,
    direcciones: Vec<(String, String, u16)>,
    responden: std::collections::HashMap<String, bool>,
    fase: Fase,
    campo: Option<Campo>,
    ver_codigo: bool,
    saliendo: bool,
    registro: Vec<String>,
    mi_direccion: String,
    desplazamiento: f32,
    alto_contenido: f32,
    sub: Sub,
    /// Cuantas sub-pantallas se han abierto: el turno de la siguiente.
    turnos: u64,
    /// Donde estaba la portada al entrar en una sub-pantalla, para volver
    /// al mismo sitio.
    desplazamiento_portada: f32,
    tx: mpsc::Sender<Aviso>,
}

impl Pantalla {
    fn contexto(&self) -> Contexto {
        Contexto {
            raiz: self.raiz.clone(),
            nombre: self.identidad.yo.nombre.clone(),
            id: self.identidad.yo.id.clone(),
            tx: self.tx.clone(),
        }
    }

    fn entrar(&mut self, sub: impl FnOnce(u64) -> Sub) {
        self.turnos += 1;
        self.desplazamiento_portada = self.desplazamiento;
        self.desplazamiento = 0.0;
        self.sub = sub(self.turnos);
    }

    fn volver(&mut self) {
        // Soltar la sub-pantalla cierra su puerta (su `Drop`).
        self.sub = Sub::Portada;
        self.desplazamiento = self.desplazamiento_portada;
    }

    fn animando(&self) -> bool {
        // La barra sin total de «Sincronizando» se mueve sola.
        if matches!(&self.fase, Fase::Progreso(t) if t.total == 0) {
            return true;
        }
        match &self.sub {
            Sub::Portada | Sub::Elegir(_) | Sub::Copias(_) => false,
            Sub::Recibir(r) => r.animando(),
            Sub::Enviar(e) => e.animando(),
        }
    }

    fn tecla(&mut self, t: Tecla) -> bool {
        let cx = self.contexto();
        match &mut self.sub {
            Sub::Portada | Sub::Elegir(_) | Sub::Copias(_) => false,
            Sub::Recibir(r) => recibir_wifi::tecla(r, t, &cx),
            Sub::Enviar(e) => enviar_wifi::tecla(e, t, &cx),
        }
    }
}

// ------------------------------------------------------------------ ventana

pub fn abrir(recursos: &Recursos, textos: &Catalogo, ubicacion: &Ubicacion) -> Result<()> {
    let raiz = ubicacion.raiz().to_path_buf();
    let identidad = leer_identidad(&raiz).context("sin identidad de este equipo")?;

    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .principal()
        .context("sin monitor principal")?
        .to_owned();
    let escala = monitor.escala_por_cien;
    let e = |v: u32| v * escala / 100;
    let alto = e(ALTO_LOGICO).min(monitor.area.alto);
    let mut marco = Rect {
        x: monitor.area.x + (monitor.area.ancho as i32 - e(ANCHO_LOGICO) as i32) / 2,
        y: monitor.area.y + (monitor.area.alto as i32 - alto as i32) / 2,
        ancho: e(ANCHO_LOGICO),
        alto,
    };
    let mut ventana = VentanaOverlay::nueva_normal(marco, &textos.t("sinc-titulo"))
        .context("no se pudo abrir la ventana de sincronizar")?;
    ABIERTA.store(ventana.handle().0 as isize, Ordering::SeqCst);
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para sincronizar")?;
    ventana.mostrar();
    ventana.enfocar();

    // Dejarse encontrar y atender a quien llama **no es de esta ventana**:
    // vive con la aplicacion (`presencia`, puerto de `Presencia.kt` del
    // movil). Aqui solo se pregunta por donde se escucha y se pide que
    // cuenten lo que pase mientras la ventana este delante.
    let (tx, rx) = mpsc::channel::<Aviso>();
    let (tx_novedad, rx_novedad) = mpsc::channel::<presencia::Novedad>();
    let _mirando = presencia::suscribir(tx_novedad);
    let puerto = presencia::puerto();

    let mut pantalla = Pantalla {
        raiz: raiz.clone(),
        identidad,
        direcciones: leer_direcciones(&raiz),
        responden: Default::default(),
        fase: Fase::Nada,
        campo: None,
        ver_codigo: false,
        saliendo: false,
        registro: Vec::new(),
        mi_direccion: match (ip_local(), puerto) {
            (Some(ip), Some(p)) => format!("{ip}:{p}"),
            _ => String::new(),
        },
        desplazamiento: 0.0,
        alto_contenido: 0.0,
        sub: Sub::Portada,
        turnos: 0,
        desplazamiento_portada: 0.0,
        tx: tx.clone(),
    };

    let mut zonas: Vec<(RectF, Accion)> = Vec::new();
    let mut arrastre: Option<(Punto, Rect)> = None;
    let mut hay_que_pintar = true;
    'bucle: loop {
        // Lo que pidieron desde fuera (hoy: «Enviar por Wi-Fi» desde el menu
        // de un mensaje del chat). Se mira en CADA vuelta y no solo al nacer:
        // con la ventana ya abierta, quien lo deja la despierta y el pedido se
        // recoge aqui sin que el usuario tenga que tocar nada.
        if let Some(pedido) = tomar_pedido() {
            match pedido {
                Pedido::Enviar(rutas) => {
                    let cx = pantalla.contexto();
                    // Lo de encima estorba a lo que se acaba de pedir.
                    pantalla.fase = Fase::Nada;
                    pantalla.campo = None;
                    pantalla.saliendo = false;
                    pantalla.entrar(|t| {
                        Sub::Enviar(Box::new(enviar_wifi::Enviar::con_archivos(t, rutas, &cx)))
                    });
                }
                Pedido::Copias => {
                    pantalla.fase = Fase::Nada;
                    pantalla.campo = None;
                    pantalla.saliendo = false;
                    // Si ya estaba en copias se vuelve a leer: puede que se
                    // acabe de borrar algo desde el chat.
                    let raiz = pantalla.raiz.clone();
                    pantalla.entrar(|_| Sub::Copias(Box::new(copias_ui::Copias::nuevo(raiz))));
                }
            }
            hay_que_pintar = true;
        }
        pixpin_shell::overlay::bombear_pendientes();
        for (hwnd, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            match evento {
                EventoOverlay::Cerrar => break 'bucle,
                EventoOverlay::Pintar | EventoOverlay::CambioDpi => hay_que_pintar = true,
                EventoOverlay::Tecla { vk: 0x1B, .. } => {
                    // Escape cierra lo de encima antes que la ventana, como el
                    // «atras» del movil.
                    if pantalla.campo.is_some() || pantalla.saliendo {
                        pantalla.campo = None;
                        pantalla.saliendo = false;
                    } else if pantalla.fase != Fase::Nada {
                        pantalla.fase = Fase::Nada;
                    } else if !matches!(pantalla.sub, Sub::Portada) {
                        pantalla.volver();
                    } else {
                        break 'bucle;
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::Tecla { vk: 0x0D, .. } => {
                    if pantalla.campo.is_some() {
                        aceptar_campo(&mut pantalla, &tx, puerto);
                    } else {
                        pantalla.tecla(Tecla::Intro);
                    }
                    hay_que_pintar = true;
                }
                // Ctrl+V: en el ordenador no hay camara, y pegar el texto
                // del QR que alguien mando es lo que la sustituye.
                EventoOverlay::Tecla {
                    vk: 0x56,
                    ctrl: true,
                    ..
                } => {
                    if let Some(pixpin_codec::ContenidoPortapapeles::Texto(t)) =
                        pixpin_codec::leer()
                    {
                        if let Some(c) = pantalla.campo.as_mut() {
                            for ch in t.trim().chars().filter(|c| !c.is_control()) {
                                escribir(c, ch);
                            }
                        } else {
                            pantalla.tecla(Tecla::Pegar(t));
                        }
                        hay_que_pintar = true;
                    }
                }
                EventoOverlay::Tecla { vk: 0x09, .. } => {
                    if let Some(c) = pantalla.campo.as_mut()
                        && c.que == QueCampo::Codigo
                    {
                        c.en_extra = !c.en_extra;
                        hay_que_pintar = true;
                    }
                }
                EventoOverlay::Tecla { vk: 0x08, .. } => {
                    if let Some(c) = pantalla.campo.as_mut() {
                        if c.en_extra {
                            c.extra.pop();
                        } else {
                            c.texto.pop();
                        }
                        hay_que_pintar = true;
                    } else if pantalla.tecla(Tecla::Borrar) {
                        hay_que_pintar = true;
                    }
                }
                EventoOverlay::Caracter(ch) if !ch.is_control() => {
                    if let Some(c) = pantalla.campo.as_mut() {
                        escribir(c, ch);
                        hay_que_pintar = true;
                    } else if pantalla.tecla(Tecla::Letra(ch)) {
                        hay_que_pintar = true;
                    }
                }
                EventoOverlay::Rueda(d) => {
                    let visible = marco.alto as f32 - BARRA * escala as f32 / 100.0;
                    let tope = (pantalla.alto_contenido - visible).max(0.0);
                    pantalla.desplazamiento =
                        (pantalla.desplazamiento - d as f32 * 0.5).clamp(0.0, tope);
                    hay_que_pintar = true;
                }
                EventoOverlay::BotonPulsado(p) => {
                    let (x, y) = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    let accion = zonas
                        .iter()
                        .rev()
                        .find(|(r, _)| {
                            x >= r.x && x < r.x + r.ancho && y >= r.y && y < r.y + r.alto
                        })
                        .map(|(_, a)| a.clone());
                    match accion {
                        Some(Accion::Cerrar) => break 'bucle,
                        Some(a) => atender(a, &mut pantalla, &tx, puerto, &|| {
                            pixpin_shell::elegir::pedir_ficheros(ventana.handle())
                        }),
                        None if y < BARRA * escala as f32 / 100.0 => {
                            ventana.capturar_raton();
                            arrastre = Some((p, marco));
                        }
                        None => {}
                    }
                    hay_que_pintar = true;
                }
                EventoOverlay::RatonMovido(p) => {
                    if let Some((desde, antes)) = arrastre {
                        marco = Rect {
                            x: antes.x + p.x - desde.x,
                            y: antes.y + p.y - desde.y,
                            ..antes
                        };
                        ventana.mover(marco);
                    }
                }
                EventoOverlay::BotonSoltado(_) if arrastre.take().is_some() => {
                    ventana.soltar_raton();
                }
                _ => {}
            }
        }

        // Lo que cuenta el servicio de presencia, traducido a lo de aqui.
        while let Ok(n) = rx_novedad.try_recv() {
            let _ = tx.send(match n {
                presencia::Novedad::Identidad => Aviso::Identidad,
                presencia::Novedad::Responde(id, si) => Aviso::Responde(id, si),
                presencia::Novedad::Registro(t) => Aviso::Registro(t),
            });
        }
        while let Ok(aviso) = rx.try_recv() {
            match aviso {
                Aviso::Fase(f) => pantalla.fase = f,
                Aviso::Responde(id, si) => {
                    pantalla.responden.insert(id, si);
                }
                Aviso::Identidad => {
                    if let Ok(i) = leer_identidad(&raiz) {
                        pantalla.identidad = i;
                    }
                    pantalla.direcciones = leer_direcciones(&raiz);
                }
                Aviso::Registro(linea) => {
                    pantalla.registro.insert(0, linea);
                    pantalla.registro.truncate(5);
                }
                Aviso::Recibir(turno, estado) => {
                    if let Sub::Recibir(r) = &mut pantalla.sub
                        && r.turno == turno
                    {
                        r.estado = estado;
                    }
                }
                Aviso::Enviar(turno, n) => {
                    let cx = pantalla.contexto();
                    if let Sub::Enviar(s) = &mut pantalla.sub
                        && s.turno == turno
                    {
                        enviar_wifi::novedad(s, n, &cx);
                    }
                }
                Aviso::Elegir {
                    otro,
                    pregunta,
                    responde,
                } => {
                    // Mientras se elige no hay dialogo encima: la lista es la
                    // pantalla, como en el movil.
                    pantalla.fase = Fase::Nada;
                    pantalla.campo = None;
                    pantalla.entrar(|_| {
                        Sub::Elegir(Box::new(elegir::Eligiendo::nuevo(otro, pregunta, responde)))
                    });
                }
            }
            hay_que_pintar = true;
        }
        if pantalla.animando() {
            hay_que_pintar = true;
        }

        if hay_que_pintar {
            hay_que_pintar = false;
            if let Ok(destino) = superficie.empezar(&motor) {
                zonas.clear();
                let _ = motor.dibujar(&destino, |p: &Pintor| {
                    pintar(p, marco, escala, textos, &mut pantalla, &mut zonas);
                });
                let _ = superficie.presentar();
            }
        }
        // La red no tiene evento de ventana: lo que mandan los hilos se mira
        // diez veces por segundo, como en recibir.
        pixpin_shell::overlay::esperar_eventos(Some(100));
    }
    // La presencia sigue viva: cerrar esta ventana no es cerrar PixPin, y el
    // movil tiene que poder seguir encontrando este equipo. Lo unico que se
    // suelta es la suscripcion (`_mirando`), y con ella la sonda se espacia.
    Ok(())
}

fn escribir(c: &mut Campo, ch: char) {
    // Los topes son los del movil: 40 para nombre y direccion, 14 para el
    // codigo (diez signos con guion y algun espacio de mas).
    let (destino, tope) = if c.en_extra {
        (&mut c.extra, 40)
    } else {
        let tope = if c.que == QueCampo::Codigo { 14 } else { 40 };
        (&mut c.texto, tope)
    };
    if destino.chars().count() < tope {
        destino.push(ch);
    }
}

fn atender(
    accion: Accion,
    s: &mut Pantalla,
    tx: &mpsc::Sender<Aviso>,
    puerto: Option<u16>,
    elegir: &dyn Fn() -> Vec<PathBuf>,
) {
    match accion {
        Accion::Cerrar => {}
        Accion::Renombrar => {
            s.campo = Some(Campo::nuevo(
                QueCampo::Nombre,
                s.identidad.yo.nombre.clone(),
            ))
        }
        Accion::Unirme => s.campo = Some(Campo::nuevo(QueCampo::Codigo, String::new())),
        Accion::ConectarDireccion => {
            s.campo = Some(Campo::nuevo(QueCampo::Direccion, String::new()))
        }
        Accion::CampoPrincipal => {
            if let Some(c) = s.campo.as_mut() {
                c.en_extra = false;
            }
        }
        Accion::CampoExtra => {
            if let Some(c) = s.campo.as_mut() {
                c.en_extra = true;
            }
        }
        Accion::Aceptar => aceptar_campo(s, tx, puerto),
        Accion::Cancelar => {
            s.campo = None;
            s.saliendo = false;
        }
        Accion::CrearGrupo => match crear_grupo(&s.raiz) {
            Ok(i) => s.identidad = i,
            Err(e) => s.fase = Fase::Fallo(e.to_string()),
        },
        Accion::VerCodigo => s.ver_codigo = !s.ver_codigo,
        Accion::SalirDelGrupo => s.saliendo = true,
        Accion::ConfirmarSalir => {
            s.saliendo = false;
            match salir_del_grupo(&s.raiz) {
                Ok(i) => {
                    s.identidad = i;
                    s.responden.clear();
                }
                Err(e) => s.fase = Fase::Fallo(e.to_string()),
            }
        }
        Accion::SincronizarCon(nombre, host, p) => {
            lanzar_vuelta(&s.raiz, tx, host, p, nombre, puerto)
        }
        Accion::SincronizarConTodos(lista) => lanzar_con_todos(&s.raiz, tx, lista, puerto),
        Accion::Elegir(toque) => {
            if let Sub::Elegir(el) = &mut s.sub
                && elegir::tocar(el, toque)
            {
                s.volver();
            }
        }
        Accion::AbrirRecibir => {
            s.entrar(|t| Sub::Recibir(Box::new(recibir_wifi::Recibir::nuevo(t))))
        }
        Accion::Volver => s.volver(),
        Accion::Recibir(toque) => {
            let cx = s.contexto();
            if let Sub::Recibir(r) = &mut s.sub
                && recibir_wifi::tocar(r, toque, &cx)
            {
                s.volver();
            }
        }
        Accion::AbrirEnviar => s.entrar(|t| Sub::Enviar(Box::new(enviar_wifi::Enviar::nuevo(t)))),
        Accion::AbrirCopias => {
            let raiz = s.raiz.clone();
            s.entrar(|_| Sub::Copias(Box::new(copias_ui::Copias::nuevo(raiz))));
        }
        Accion::Copias(toque) => {
            if let Sub::Copias(c) = &mut s.sub
                && copias_ui::tocar(c, toque)
            {
                s.volver();
            }
        }
        Accion::Enviar(toque) => {
            let cx = s.contexto();
            if let Sub::Enviar(e) = &mut s.sub
                && enviar_wifi::tocar(e, toque, &cx, elegir)
            {
                s.volver();
            }
        }
        Accion::CerrarFase => s.fase = Fase::Nada,
    }
}

fn aceptar_campo(s: &mut Pantalla, tx: &mpsc::Sender<Aviso>, puerto: Option<u16>) {
    let Some(c) = s.campo.take() else {
        return;
    };
    match c.que {
        QueCampo::Nombre => {
            let nombre = c.texto.trim().to_string();
            if nombre.is_empty() {
                s.campo = Some(c);
                return;
            }
            match renombrar(&s.raiz, &nombre) {
                Ok(i) => s.identidad = i,
                Err(e) => s.fase = Fase::Fallo(e.to_string()),
            }
        }
        QueCampo::Codigo => {
            let codigo = pixpin_sincro::codigo::limpiar(&c.texto);
            if !codigo_valido(&codigo) {
                s.campo = Some(Campo {
                    en_extra: false,
                    ..c
                });
                return;
            }
            // La direccion es solo para cuando no lo encuentra: vacia, se
            // busca por la red; escrita y mal, se vuelve a ella.
            let direccion = partir_direccion(&c.extra);
            if direccion.is_none() && !c.extra.trim().is_empty() {
                s.campo = Some(Campo {
                    en_extra: true,
                    ..c
                });
                return;
            }
            lanzar_union(&s.raiz, tx, codigo, direccion, puerto);
        }
        QueCampo::Direccion => {
            let Some((host, p)) = partir_direccion(&c.texto) else {
                s.campo = Some(c);
                return;
            };
            lanzar_vuelta(&s.raiz, tx, host.clone(), p, host, puerto);
        }
    }
}

// ------------------------------------------------------------ identidad

fn nombre_del_equipo() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PixPin Max".into())
}

fn leer_identidad(raiz: &Path) -> std::io::Result<Identidad> {
    Identidad::leer_o_crear(raiz, &nombre_del_equipo())
}

/// La hora de los relojes de los dos aparatos: milisegundos desde 1970 en
/// UTC, como `System.currentTimeMillis()`. No la local: con ella el desfase
/// saldria de horas en cuanto el movil estuviera en otra zona.
/// La hora para nombres y sellos de fuera de este modulo (`al_frente`).
pub(crate) fn ahora_para_nombres() -> i64 {
    ahora_ms()
}

fn ahora_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// Un codigo de grupo nuevo con el azar del sistema. Se descartan los bytes
/// que sesgarian el reparto: 248 es el mayor multiplo de 31 que cabe en un
/// byte, y sin esto los primeros ocho signos saldrian mas que los demas.
fn codigo_nuevo() -> Option<String> {
    let signos: Vec<char> = pixpin_sincro::codigo::SIGNOS.chars().collect();
    let tope = 256 / signos.len() * signos.len();
    let mut salida = String::new();
    while salida.len() < pixpin_sincro::codigo::LARGO {
        let mut b = [0u8; 16];
        if !pixpin_shell::azar(&mut b) {
            return None;
        }
        for x in b {
            if (x as usize) < tope && salida.len() < pixpin_sincro::codigo::LARGO {
                salida.push(signos[x as usize % signos.len()]);
            }
        }
    }
    Some(salida)
}

/// `Disco.crearGrupo`: este aparato dentro, con la letra `a`.
fn crear_grupo(raiz: &Path) -> Result<Identidad> {
    let actual = leer_identidad(raiz)?;
    let codigo = codigo_nuevo().context("el sistema no dio azar para el codigo")?;
    let mut yo = actual.yo.clone();
    yo.letra = Some("a".into());
    yo.desde = ahora_ms();
    let nueva = Identidad {
        yo: yo.clone(),
        codigo: Some(codigo),
        miembros: vec![yo],
        resto: actual.resto,
    };
    nueva.guardar(raiz)?;
    Ok(nueva)
}

/// `Disco.salirDelGrupo`: lo acordado con los demas se olvida, y si vuelve
/// a entrar la primera vuelta se hace como la primera vez.
fn salir_del_grupo(raiz: &Path) -> Result<Identidad> {
    let actual = leer_identidad(raiz)?;
    let mut yo = actual.yo.clone();
    yo.letra = None;
    yo.desde = 0;
    let nueva = Identidad {
        yo,
        codigo: None,
        miembros: Vec::new(),
        resto: actual.resto,
    };
    nueva.guardar(raiz)?;
    let _ = std::fs::remove_dir_all(carpeta_sincro(raiz).join("base"));
    Ok(nueva)
}

/// `Disco.renombrar`: el nombre cambia tambien en su ficha de miembro.
fn renombrar(raiz: &Path, nombre: &str) -> Result<Identidad> {
    let mut actual = leer_identidad(raiz)?;
    actual.yo.nombre = nombre.to_string();
    let yo = actual.yo.clone();
    for m in actual.miembros.iter_mut().filter(|m| m.id == yo.id) {
        *m = yo.clone();
    }
    actual.guardar(raiz)?;
    Ok(actual)
}

fn al_cable(a: &Aparato) -> m::Aparato {
    m::Aparato {
        id: a.id.clone(),
        nombre: a.nombre.clone(),
        letra: a.letra.clone(),
        desde: a.desde,
    }
}

fn del_cable(a: &m::Aparato) -> Aparato {
    Aparato {
        id: a.id.clone(),
        nombre: a.nombre.clone(),
        letra: a.letra.clone(),
        desde: a.desde,
        resto: Default::default(),
    }
}

/// `Grupo.juntar`: por id; de uno repetido gana el que habla por si mismo.
fn juntar(
    mios: &[Aparato],
    suyos: &[m::Aparato],
    quien_habla: Option<&m::Aparato>,
) -> Vec<Aparato> {
    let mut salida: Vec<Aparato> = mios.to_vec();
    for a in suyos {
        if !salida.iter().any(|x| x.id == a.id) {
            salida.push(del_cable(a));
        }
    }
    if let Some(q) = quien_habla {
        match salida.iter_mut().find(|x| x.id == q.id) {
            Some(x) => *x = del_cable(q),
            None => salida.push(del_cable(q)),
        }
    }
    salida
}

/// Los miembros tal como se guardan, o yo solo si la lista esta vacia: asi
/// lo hace el movil (`miembros.ifEmpty { listOf(yo) }`).
fn miembros_o_yo(id: &Identidad) -> Vec<Aparato> {
    if id.miembros.is_empty() {
        vec![id.yo.clone()]
    } else {
        id.miembros.clone()
    }
}

#[cfg(test)]
use pixpin_sincro::grupo::sena::libre as letra_libre;
use pixpin_sincro::grupo::{legible, valido as codigo_valido};

/// `partirDireccion` del movil: `192.168.1.20:47474`, o sin puerto y va el
/// de siempre.
fn partir_direccion(texto: &str) -> Option<(String, u16)> {
    let t = texto.trim();
    if t.is_empty() {
        return None;
    }
    match t.split_once(':') {
        Some((h, p)) => Some((h.to_string(), p.trim().parse().ok()?)),
        None => Some((t.to_string(), pixpin_sincro::PUERTO)),
    }
}

/// `haceCuanto`: sin horas exactas, que no hacen falta para decidir.
fn hace_cuanto(cuando: i64, ahora: i64) -> String {
    let min = (ahora - cuando).max(0) / 60_000;
    match min {
        0 => "ahora mismo".into(),
        1..=59 => format!("hace {min} min"),
        60..=1439 => format!("hace {} h", min / 60),
        1440..=2879 => "ayer".into(),
        _ => format!("hace {} días", min / 1440),
    }
}

// ----------------------------------------------------- direcciones y fechas

fn carpeta_sincro(raiz: &Path) -> PathBuf {
    raiz.join("sincro")
}

/// `sincro/direcciones.txt`, el mismo formato que el movil: id, host y
/// puerto separados por tabuladores.
fn leer_direcciones(raiz: &Path) -> Vec<(String, String, u16)> {
    std::fs::read_to_string(carpeta_sincro(raiz).join("direcciones.txt"))
        .map(|t| leer_direcciones_de(&t))
        .unwrap_or_default()
}

fn leer_direcciones_de(texto: &str) -> Vec<(String, String, u16)> {
    texto
        .lines()
        .filter_map(|l| {
            let mut p = l.split('\t');
            let (id, host) = (p.next()?, p.next()?);
            let puerto = p.next()?.trim().parse().ok()?;
            Some((id.to_string(), host.to_string(), puerto))
        })
        .collect()
}

fn apuntar_direccion(raiz: &Path, id: &str, host: &str, puerto: u16) {
    if puerto == 0 || host.is_empty() {
        return;
    }
    let mut todas = leer_direcciones(raiz);
    todas.retain(|(i, _, _)| i != id);
    todas.push((id.to_string(), host.to_string(), puerto));
    let texto: String = todas
        .iter()
        .map(|(i, h, p)| format!("{i}\t{h}\t{p}\n"))
        .collect();
    let carpeta = carpeta_sincro(raiz);
    let _ = std::fs::create_dir_all(&carpeta);
    let _ = std::fs::write(carpeta.join("direcciones.txt"), texto);
}

/// `Disco.ultimaVez`: `sincro/elegidos/<id>.cuando`.
fn ultima_vez(raiz: &Path, otro: &str) -> i64 {
    // La misma que apunta la sincronizacion al acabar cada chat.
    DiscoPc::nuevo(raiz).ultima_vez(otro)
}

fn ip_local() -> Option<String> {
    let s = std::net::UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    s.connect(("8.8.8.8", 53)).ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}

// ------------------------------------------------------------------ la red

fn nonce() -> Option<[u8; pixpin_sincro::canal::NONCE]> {
    let mut n = [0u8; pixpin_sincro::canal::NONCE];
    pixpin_shell::azar(&mut n).then_some(n)
}

fn mandar<F: Read + Write, T: serde::Serialize>(c: &mut Canal<F>, cosa: &T) -> Result<()> {
    let json = serde_json::to_vec(cosa)?;
    c.mandar(Tipo::Json, &json)?;
    Ok(())
}

fn leer_respuesta<F: Read + Write>(c: &mut Canal<F>) -> Result<Respuesta> {
    let (tipo, datos) = c.recibir()?;
    anyhow::ensure!(tipo == Tipo::Json, "se esperaba una respuesta");
    let r: Respuesta = serde_json::from_slice(&datos)?;
    if let Some(e) = r.error {
        anyhow::bail!(e);
    }
    Ok(r)
}

/// `Red.conectar`: 8 s para llegar y 2 min de espera, que mientras el otro
/// decide no llega nada.
fn conectar(host: &str, puerto: u16) -> Result<TcpStream> {
    let dir = (host, puerto)
        .to_socket_addrs()?
        .next()
        .context("direccion sin resolver")?;
    let s = TcpStream::connect_timeout(&dir, Duration::from_secs(8))?;
    s.set_read_timeout(Some(Duration::from_secs(120)))?;
    s.set_nodelay(true)?;
    Ok(s)
}

/// `Red.sondear`: un «PING» y esperar el «PONG».
fn sondear_uno(host: &str, puerto: u16) -> bool {
    let Some(dir) = (host, puerto)
        .to_socket_addrs()
        .ok()
        .and_then(|mut d| d.next())
    else {
        return false;
    };
    let Ok(mut s) = TcpStream::connect_timeout(&dir, Duration::from_millis(1200)) else {
        return false;
    };
    let _ = s.set_read_timeout(Some(Duration::from_millis(1500)));
    if s.write_all(pixpin_sincro::SONDA).is_err() {
        return false;
    }
    let mut b = [0u8; 4];
    s.read_exact(&mut b).is_ok() && &b == pixpin_sincro::SONDA_RESPUESTA
}

/// K6: la direccion buena para llamar a `nombre`, sabiendo la que habia al
/// pintar. Si no contesta, se busca por la red y lo encontrado se apunta
/// (`AlDia.aparatos`, ver `al_dia`). Sin rastro de el, el mismo error que
/// daria la conexion, pero sin esperar los 8 s de agotarla.
/// Los del grupo que se anuncian ahora en la red (3 s de mDNS), menos este
/// equipo. Sin grupo, nadie: sin etiqueta no se sabe a quien buscar.
fn vistos_del_grupo(x: &Identidad) -> Vec<al_dia::Visto> {
    let Some(codigo) = x.codigo.as_deref() else {
        return Vec::new();
    };
    let etiqueta = etiqueta_de(codigo);
    pixpin_shell::mdns::buscar(TIPO_MDNS, Duration::from_secs(3))
        .unwrap_or_default()
        .iter()
        .filter(|v| es_del_grupo(v, &etiqueta, &x.yo.id))
        .map(|v| al_dia::Visto {
            id: v.datos.get("id").cloned().unwrap_or_default(),
            nombre: v.datos.get("n").cloned().unwrap_or_default(),
            host: v.host.to_string(),
            puerto: v.puerto,
        })
        .collect()
}

fn llegar(raiz: &Path, host: &str, puerto: u16, nombre: &str) -> Result<(String, u16)> {
    let identidad = leer_identidad(raiz).ok();
    // El id sale de la direccion apuntada; si no, del nombre en el grupo.
    let id = leer_direcciones(raiz)
        .into_iter()
        .find(|(_, h, p)| h == host && *p == puerto)
        .map(|(i, _, _)| i)
        .or_else(|| {
            identidad.as_ref().and_then(|x| {
                x.miembros
                    .iter()
                    .find(|m| m.nombre == nombre && m.id != x.yo.id)
                    .map(|m| m.id.clone())
            })
        });
    let buscar = || identidad.as_ref().map(vistos_del_grupo).unwrap_or_default();
    match al_dia::donde_esta(id.as_deref(), nombre, host, puerto, sondear_uno, buscar) {
        al_dia::Donde::LaDeSiempre => Ok((host.to_string(), puerto)),
        al_dia::Donde::Nueva { host, puerto } => {
            tracing::info!(%nombre, %host, puerto, "sincro: el otro cambio de direccion");
            if let Some(i) = &id {
                apuntar_direccion(raiz, i, &host, puerto);
            }
            Ok((host, puerto))
        }
        // Lo mismo que diria agotar la conexion («No se pudo llegar a…»),
        // sin los 8 s de esperarla.
        al_dia::Donde::NoEsta => {
            Err(std::io::Error::from(std::io::ErrorKind::HostUnreachable).into())
        }
    }
}

/// Los recordados, comprobados cada cuatro segundos mientras alguien mira la
/// ventana, como en el movil: asi un aparato conocido aparece al momento. Sin
/// nadie delante se espacia (ver `presencia::espera_de_la_sonda`): buscar por
/// la red cuesta, y el movil tampoco busca con la pantalla cerrada.
fn sondear(raiz: PathBuf, vivo: Arc<AtomicBool>) {
    let _ = std::thread::Builder::new()
        .name("sincro-sonda".into())
        .spawn(move || {
            let mut etiqueta = (String::new(), String::new());
            while vivo.load(Ordering::SeqCst) {
                if let Ok(id) = leer_identidad(&raiz)
                    && let Some(codigo) = id.codigo.clone()
                {
                    // Primero la red: lo que anuncia la etiqueta del grupo
                    // se apunta con su direccion, que es lo que hace que
                    // aparezca sin haberla tecleado nunca.
                    if etiqueta.0 != codigo {
                        etiqueta = (codigo.clone(), etiqueta_de(&codigo));
                    }
                    if let Ok(vistos) =
                        pixpin_shell::mdns::buscar(TIPO_MDNS, Duration::from_secs(3))
                    {
                        for v in vistos
                            .iter()
                            .filter(|v| es_del_grupo(v, &etiqueta.1, &id.yo.id))
                        {
                            if let Some(otro) = v.datos.get("id") {
                                apuntar_direccion(&raiz, otro, &v.host.to_string(), v.puerto);
                            }
                        }
                        presencia::difundir(presencia::Novedad::Identidad);
                    }
                    let dirs = leer_direcciones(&raiz);
                    let mut responden = std::collections::BTreeMap::new();
                    for miembro in id.miembros.iter().filter(|x| x.id != id.yo.id) {
                        if let Some((_, h, p)) = dirs.iter().find(|(i, _, _)| *i == miembro.id) {
                            let si = sondear_uno(h, *p);
                            responden.insert(miembro.id.clone(), si);
                            presencia::difundir(presencia::Novedad::Responde(
                                miembro.id.clone(),
                                si,
                            ));
                        }
                    }
                    // Para el plugin (`p s`), que no puede salir a preguntar.
                    en_fondo::apuntar_estado(&raiz, responden);
                }
                // A cachitos de 100 ms para que cerrar la aplicacion no tenga
                // que esperar a que pase la espera entera.
                let espera = presencia::espera_de_la_sonda();
                let trozos = (espera.as_millis() / 100).max(1);
                for _ in 0..trozos {
                    if !vivo.load(Ordering::SeqCst) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        });
}

/// Escucha en el puerto de siempre, o en cualquiera si esta tomado. Devuelve
/// el puerto, que es lo que se ensena en «La de este aparato».
fn escuchar(raiz: PathBuf, vivo: Arc<AtomicBool>) -> Option<u16> {
    let escucha = TcpListener::bind(("0.0.0.0", pixpin_sincro::PUERTO))
        .or_else(|_| TcpListener::bind(("0.0.0.0", 0)))
        .ok()?;
    escucha.set_nonblocking(true).ok()?;
    let puerto = escucha.local_addr().ok()?.port();
    let _ = std::thread::Builder::new()
        .name("sincro-escucha".into())
        .spawn(move || {
            while vivo.load(Ordering::SeqCst) {
                match escucha.accept() {
                    Ok((flujo, de)) => {
                        let _ = flujo.set_nonblocking(false);
                        // Diez segundos para la sonda; `responder` lo alarga si es
                        // una sincronizacion.
                        let _ = flujo.set_read_timeout(Some(Duration::from_secs(10)));
                        // Cada conexion en su hilo: mientras se junta un chat
                        // largo, la sonda del movil tiene que seguir teniendo
                        // respuesta, y un segundo que llame, oir «ocupado».
                        let raiz = raiz.clone();
                        let _ = std::thread::Builder::new()
                            .name("sincro-atiende".into())
                            .spawn(move || {
                                if let Err(e) = responder(flujo, &raiz, puerto) {
                                    tracing::info!(%de, ?e, "una conexion de sincronizar termino mal");
                                    presencia::difundir(presencia::Novedad::Registro(format!(
                                        "{de}: {e}"
                                    )));
                                }
                            });
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(150));
                    }
                    Err(e) => {
                        tracing::warn!(?e, "la escucha de sincronizar fallo");
                        return;
                    }
                }
            }
        });
    Some(puerto)
}

/// `Respondedor.atender` del movil, entero: la sonda, el saludo (dar letra a
/// quien se une) y despues todo lo que pida quien dirige —inventario,
/// mensajes, proyecto, archivos, parches, lo acordado, lapidas— sobre el
/// almacen de este equipo visto como el del movil (`vista::DiscoPc`).
fn responder(mut flujo: TcpStream, raiz: &Path, mi_puerto: u16) -> Result<()> {
    let mut cuatro = [0u8; 4];
    let n = flujo.peek(&mut cuatro)?;
    if n == 4 && &cuatro == pixpin_sincro::SONDA {
        flujo.read_exact(&mut cuatro)?;
        flujo.write_all(pixpin_sincro::SONDA_RESPUESTA)?;
        return Ok(());
    }
    // Mientras dura, la sincronizacion automatica no arranca otra.
    let _en_curso = EnCurso::nueva();
    // Largo, como `Red.escuchar` del movil: mientras el otro decide que
    // sincronizar, aqui no llega nada, y cortarle a los dos minutos le
    // tiraria la vuelta.
    flujo.set_read_timeout(Some(Duration::from_secs(30 * 60)))?;
    let de = flujo.peer_addr().ok();
    let raiz_suya = raiz.to_path_buf();
    let disco = DiscoPc::nuevo(raiz).con_avisos(move |c| {
        if c == pixpin_sincro::disco::Cambio::Identidad {
            presencia::difundir(presencia::Novedad::Identidad);
        } else {
            // Mensajes, proyectos o archivos: lo que el movil acaba de
            // escribir DEBAJO de una ventana de chat abierta. Sin avisarla,
            // lo recien llegado no sale hasta cerrarla y volver a abrirla.
            crate::ventana_chat::refrescar();
            // Y una hora puesta en el movil llega como un campo mas del
            // mensaje: sin releer la agenda no sonaria aqui hasta el
            // siguiente arranque. Solo con los mensajes, que es donde vive
            // la hora, y no con cada archivo que entra.
            if c == pixpin_sincro::disco::Cambio::Mensajes {
                crate::recordatorios::releer(&raiz_suya);
            }
        }
    });
    let r = Respondedor {
        disco: &disco,
        estado: &|t: &str| {
            presencia::difundir(presencia::Novedad::Registro(t.to_string()));
        },
        ahora: &ahora_ms,
        mi_puerto: u32::from(mi_puerto),
        al_saludar: &|otro: &m::Aparato, puerto: u32| {
            if let Some(d) = de {
                apuntar_direccion(raiz, &otro.id, &d.ip().to_string(), puerto as u16);
            }
        },
        // Un archivo suelto de otro aparato: al chat o lienzo abierto aqui.
        suelto: Some(&|otro: &m::Aparato| {
            let yo = leer_identidad(raiz)
                .map(|i| i.yo.nombre)
                .ok()
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(nombre_del_equipo);
            presencia::difundir(presencia::Novedad::Registro(format!(
                "Archivo recibido de {}",
                otro.nombre
            )));
            Box::new(crate::al_frente::AlFrente {
                raiz: raiz.to_path_buf(),
                yo,
                de: otro.id.clone(),
                carpeta_lienzo: carpeta_sincro(raiz).join("al-lienzo"),
            })
        }),
    };
    // Con la galeria de capturas: si el otro la pide, la tiene (Android
    // v0.107.0); uno de antes no la pide y no pasa nada.
    let capturas = galeria::CapturasDelPc::nuevas(raiz);
    let hecho = r.atender_con_galeria(flujo, nonce().context("sin azar")?, Some(&capturas));
    // Lo que llego sin marco de la tinta (el movil de hoy no lo escribe)
    // recibe el suyo, en otro hilo; viaja en la vuelta siguiente (K21).
    crate::marco_de_la_tinta::tras_sincronizar(raiz);
    // Lo que se escribio (la lista de proyectos, «sincronizado hace…»).
    presencia::difundir(presencia::Novedad::Identidad);
    hecho?;
    Ok(())
}

/// Lo que queda de un saludo como quien llama.
struct Saludo {
    canal: Canal<TcpStream>,
    otro: m::Aparato,
}

/// El saludo de quien llama (`Sesion.abrir`).
fn saludar_como_cliente(
    raiz: &Path,
    host: &str,
    puerto: u16,
    codigo: &str,
    unirme: bool,
    mi_puerto: Option<u16>,
) -> Result<Saludo> {
    let id = leer_identidad(raiz)?;
    let flujo = conectar(host, puerto)?;
    let clave = pixpin_sincro::codigo::clave_de_grupo(codigo);
    let mut canal = Canal::saludar(flujo, &clave, nonce().context("sin azar")?, true)?;
    let antes = ahora_ms();
    mandar(
        &mut canal,
        &Peticion {
            t: "hola".into(),
            hola: Some(m::Hola {
                yo: al_cable(&id.yo),
                miembros: id.miembros.iter().map(al_cable).collect(),
                reloj: antes,
                unirme,
                puerto: mi_puerto.unwrap_or(0) as u32,
                ..Default::default()
            }),
            ..Default::default()
        },
    )?;
    let r = leer_respuesta(&mut canal)?;
    let hola = r.hola.context("El otro aparato no saludó")?;
    anyhow::ensure!(
        hola.version == pixpin_sincro::VERSION,
        "El otro aparato tiene otra versión de PixPin: actualiza los dos"
    );
    if unirme {
        let yo = hola
            .miembros
            .iter()
            .find(|x| x.id == id.yo.id)
            .context("El otro aparato no me dio letra")?;
        Identidad {
            yo: Aparato {
                resto: id.yo.resto.clone(),
                ..del_cable(yo)
            },
            codigo: Some(codigo.to_string()),
            miembros: hola.miembros.iter().map(del_cable).collect(),
            resto: id.resto.clone(),
        }
        .guardar(raiz)?;
    } else {
        Identidad {
            miembros: juntar(&miembros_o_yo(&id), &hola.miembros, Some(&hola.yo)),
            ..id
        }
        .guardar(raiz)?;
    }
    let puerto_otro = if hola.puerto > 0 {
        hola.puerto as u16
    } else {
        puerto
    };
    apuntar_direccion(raiz, &hola.yo.id, host, puerto_otro);
    Ok(Saludo {
        canal,
        otro: hola.yo,
    })
}

fn despedirse(canal: &mut Canal<TcpStream>) {
    if mandar(canal, &Peticion::de("adios")).is_ok() {
        let _ = leer_respuesta(canal);
    }
}

/// `explicar` del movil: el fallo en palabras de quien lo sufre.
fn explicar(e: &anyhow::Error, nombre: &str) -> String {
    // Lo del protocolo, como lo cuenta el movil: «ocupado» con sus palabras;
    // lo que dijo el otro, como un corte con su motivo; lo de la red, abajo.
    let io = match e.downcast_ref::<ErrorSincro>() {
        Some(ErrorSincro::Remoto(t)) if t == m::OCUPADO => {
            return format!(
                "{nombre} está sincronizando con otro aparato. Prueba otra vez en un momento."
            );
        }
        Some(ErrorSincro::Remoto(t)) => {
            return format!(
                "Se cortó la conexión con {nombre}: {t}. Vuelve a sincronizar para terminar."
            );
        }
        Some(ErrorSincro::Protocolo(t)) => return t.clone(),
        Some(ErrorSincro::Canal(pixpin_sincro::canal::ErrorCanal::Io(io)))
        | Some(ErrorSincro::Io(io)) => Some(io),
        Some(ErrorSincro::Canal(_)) => {
            return format!("{nombre} no aceptó la conexión: ¿tiene el mismo código de grupo?");
        }
        _ => e.downcast_ref::<std::io::Error>(),
    };
    if let Some(io) = io {
        use std::io::ErrorKind as K;
        return match io.kind() {
            K::ConnectionRefused | K::TimedOut | K::HostUnreachable | K::NetworkUnreachable => {
                format!(
                    "No se pudo llegar a {nombre}. Comprueba que está en la misma Wi-Fi y con PixPin abierto."
                )
            }
            K::UnexpectedEof => {
                format!(
                    "{nombre} cortó la conexión a medias. Lo que ya se había pasado está bien; vuelve a sincronizar para terminar."
                )
            }
            _ => format!(
                "Se cortó la conexión con {nombre}: {io}. Vuelve a sincronizar para terminar."
            ),
        };
    }
    // El codigo del grupo no viaja: si el otro tiene otro, su primer tramo no
    // descifra, y eso hay que contarlo asi y no como un fallo de red.
    if e.downcast_ref::<pixpin_sincro::canal::ErrorCanal>()
        .is_some()
    {
        return format!("{nombre} no aceptó la conexión: ¿tiene el mismo código de grupo?");
    }
    e.to_string()
}

/// Busca por la red, hasta `tope`, un aparato que anuncie la etiqueta del
/// grupo y no sea este. Es `red.buscar(...) { it["g"] == etiqueta }` del
/// movil, que espera 25 s antes de rendirse.
fn buscar_del_grupo(
    etiqueta: &str,
    yo: &str,
    tope: Duration,
) -> Option<pixpin_shell::mdns::Vecino> {
    let empezo = std::time::Instant::now();
    while empezo.elapsed() < tope {
        let vistos = pixpin_shell::mdns::buscar(TIPO_MDNS, Duration::from_secs(3)).ok()?;
        if let Some(v) = vistos.into_iter().find(|v| es_del_grupo(v, etiqueta, yo)) {
            return Some(v);
        }
    }
    None
}

fn es_del_grupo(v: &pixpin_shell::mdns::Vecino, etiqueta: &str, yo: &str) -> bool {
    v.datos.get("g").map(String::as_str) == Some(etiqueta)
        && v.datos.get("id").map(String::as_str) != Some(yo)
}

/// El tipo que anuncia y busca el movil (`Red.TIPO`).
const TIPO_MDNS: &str = "_pixpin._tcp";

fn etiqueta_de(codigo: &str) -> String {
    pixpin_sincro::codigo::etiqueta(&pixpin_sincro::codigo::clave_de_grupo(codigo))
}

/// Mientras la ventana viva, este equipo se anuncia como lo hace el movil en
/// `Presencia`: `PixPin <nombre>` con `g`, `id`, `l` y `n`. Se rehace cuando
/// cambia algo de eso (crear grupo, salir, renombrar, recibir letra). En un
/// hilo aparte porque anunciar bloquea casi un segundo esperando a Windows.
fn anunciarse(raiz: PathBuf, puerto: u16, vivo: Arc<AtomicBool>) {
    let _ = std::thread::Builder::new()
        .name("sincro-anuncio".into())
        .spawn(move || {
            let mut anuncio: Option<pixpin_shell::mdns::Anuncio> = None;
            let mut con: Option<(String, String, String)> = None;
            let mut etiqueta = (String::new(), String::new());
            while vivo.load(Ordering::SeqCst) {
                if let Ok(id) = leer_identidad(&raiz) {
                    let clave = id.codigo.clone().map(|c| {
                        (
                            c,
                            id.yo.letra.clone().unwrap_or_default(),
                            id.yo.nombre.clone(),
                        )
                    });
                    if clave != con {
                        anuncio = None;
                        if let Some((codigo, letra, nombre)) = &clave {
                            // La etiqueta cuesta 60 000 vueltas de PBKDF2:
                            // se calcula una vez por codigo.
                            if etiqueta.0 != *codigo {
                                etiqueta = (codigo.clone(), etiqueta_de(codigo));
                            }
                            let corto: String = nombre.chars().take(40).collect();
                            match pixpin_shell::mdns::anunciar(
                                TIPO_MDNS,
                                &format!("PixPin {nombre}"),
                                puerto,
                                &[
                                    ("g", &etiqueta.1),
                                    ("id", &id.yo.id),
                                    ("l", letra),
                                    ("n", &corto),
                                ],
                            ) {
                                Ok(a) => anuncio = Some(a),
                                Err(e) => tracing::warn!(?e, "no se pudo anunciar en la red"),
                            }
                        }
                        con = clave;
                    }
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            drop(anuncio);
        });
}

fn lanzar_union(
    raiz: &Path,
    tx: &mpsc::Sender<Aviso>,
    codigo: String,
    direccion: Option<(String, u16)>,
    mi_puerto: Option<u16>,
) {
    let raiz = raiz.to_path_buf();
    let tx = tx.clone();
    let _ = tx.send(Aviso::Fase(Fase::Trabajando(
        "Buscando un aparato del grupo en esta Wi-Fi…\nEn el otro aparato, ten PixPin abierto."
            .into(),
    )));
    let _ = std::thread::Builder::new()
        .name("sincro-unirse".into())
        .spawn(move || {
            let yo = leer_identidad(&raiz).map(|i| i.yo.id).unwrap_or_default();
            let hallado = match direccion {
                Some(d) => Some(d),
                None => buscar_del_grupo(&etiqueta_de(&codigo), &yo, Duration::from_secs(25))
                    .map(|v| (v.host.to_string(), v.puerto)),
            };
            let Some((host, puerto)) = hallado else {
                let _ = tx.send(Aviso::Fase(Fase::Fallo(
                    "No apareció ningún aparato con ese código. Comprueba el código, que los dos \
                     estáis en la misma Wi-Fi y que el otro tiene PixPin abierto. También puedes \
                     escribir su dirección."
                        .into(),
                )));
                return;
            };
            let _ = tx.send(Aviso::Fase(Fase::Trabajando(format!(
                "Uniéndome al grupo con {host}…"
            ))));
            let fase = match saludar_como_cliente(&raiz, &host, puerto, &codigo, true, mi_puerto) {
                Ok(mut s) => {
                    despedirse(&mut s.canal);
                    let letra = leer_identidad(&raiz)
                        .ok()
                        .and_then(|i| i.yo.letra)
                        .unwrap_or_default();
                    Fase::Terminado {
                        aviso: None,
                        titulo: "Dentro del grupo".into(),
                        trajo: false,
                        texto: format!(
                            "Este aparato es la letra «{letra}»: sus mensajes se nombran #1{letra}, #2{letra}…\n\nYa puedes sincronizar con {}.",
                            s.otro.nombre
                        ),
                    }
                }
                Err(e) => Fase::Fallo(explicar(&e, "el otro aparato")),
            };
            let _ = tx.send(Aviso::Identidad);
            let _ = tx.send(Aviso::Fase(fase));
        });
}

/// Una vuelta con un aparato, preguntando que chats (`sincronizarCon`):
/// «¿Que sincronizar?», juntar cada chat elegido con su barra y, al acabar,
/// «Al dia con X» con lo hecho y los avisos.
fn lanzar_vuelta(
    raiz: &Path,
    tx: &mpsc::Sender<Aviso>,
    host: String,
    puerto: u16,
    nombre: String,
    mi_puerto: Option<u16>,
) {
    let raiz = raiz.to_path_buf();
    let tx = tx.clone();
    let _ = tx.send(Aviso::Fase(Fase::Trabajando(format!(
        "Conectando con {nombre}…"
    ))));
    let _ = std::thread::Builder::new()
        .name("sincro-vuelta".into())
        .spawn(move || {
            let mut hecho = Hecho::default();
            let r = una_vuelta(
                &raiz, &tx, &host, puerto, &nombre, mi_puerto, true, &mut hecho, "",
            );
            let fase = match r {
                Ok(v) if v.cancelada => Fase::Nada,
                Ok(v) => {
                    let mut avisos = vuelta::avisos_de_lo_hecho(&hecho);
                    avisos.extend(v.avisos);
                    Fase::Terminado {
                        titulo: format!("Al día con {}", v.nombre),
                        trajo: hecho.traidos > 0 || hecho.fusionados > 0,
                        texto: vuelta::contar_lo_hecho(&hecho, v.bytes, v.segundos),
                        aviso: (!avisos.is_empty()).then(|| avisos.join("\n\n")),
                    }
                }
                Err(e) => Fase::Fallo(explicar(&e, &nombre)),
            };
            let _ = tx.send(Aviso::Identidad);
            let _ = tx.send(Aviso::Fase(fase));
        });
}

/// `unaVuelta`: conecta, saluda, resuelve lo borrado y junta lo elegido
/// (`preguntar`) o lo de la ultima vez con ese aparato. `rotulo` va delante
/// de lo que se ensena, para saber por donde va una ronda.
#[allow(clippy::too_many_arguments)]
fn una_vuelta(
    raiz: &Path,
    tx: &mpsc::Sender<Aviso>,
    host: &str,
    puerto: u16,
    nombre: &str,
    mi_puerto: Option<u16>,
    preguntar: bool,
    hecho: &mut Hecho,
    rotulo: &str,
) -> Result<vuelta::Vuelta> {
    let _en_curso = EnCurso::nueva();
    let _ = tx.send(Aviso::Fase(Fase::Trabajando(format!(
        "{rotulo}Conectando con {nombre}…"
    ))));
    // K6: si la direccion de cuando se pinto ya no contesta, se busca por la
    // red antes de estrellarse contra ella.
    let (bueno, puerto) = llegar(raiz, host, puerto, nombre)?;
    let host = bueno.as_str();
    let flujo = conectar(host, puerto)?;
    // La vuelta que se pide desde aqui tambien escribe lo del otro aparato,
    // asi que el chat abierto tiene que enterarse igual que en `responder`.
    let raiz_suya = raiz.to_path_buf();
    let disco = DiscoPc::nuevo(raiz).con_avisos(move |c| {
        crate::ventana_chat::refrescar();
        if c == pixpin_sincro::disco::Cambio::Mensajes {
            crate::recordatorios::releer(&raiz_suya);
        }
    });
    // Las capturas de este equipo, para la galeria que viaja (antes que la
    // sesion: tienen que vivir mas que ella).
    let capturas = galeria::CapturasDelPc::nuevas(raiz);
    let mut s = Sesion::conectar(
        flujo,
        &disco,
        false,
        None,
        ahora_ms,
        u32::from(mi_puerto.unwrap_or(0)),
        nonce().context("sin azar")?,
    )
    .map_err(|e| {
        // Cortar en el saludo es lo que hace quien no descifra: otro codigo.
        if e.es_corte() {
            anyhow::anyhow!("{nombre} no aceptó la conexión: ¿tiene el mismo código de grupo?")
        } else {
            anyhow::Error::from(e)
        }
    })?;
    // **La galeria de capturas** viaja detras de los chats (Android
    // v0.107.0): las capturas de cada uno pasan al otro con su fecha y se
    // van el mismo dia en los dos. Ver `galeria`.
    s.con_galeria(&capturas);
    // «Lo mio manda» (v0.79 del movil): con el encendido, lo de este equipo
    // se impone y del otro lado solo llega lo que aqui no existe. Se lee del
    // TOML en cada vuelta y no se guarda en la sesion: asi cambiarlo tiene
    // efecto en la siguiente sincronizacion sin reiniciar nada.
    // Preguntando, es solo como empieza el interruptor «Juntar / Lo mio
    // manda» de «¿Que sincronizar?», y lo que se elija ahi es lo que vale;
    // sin preguntar («con todos») es lo que manda.
    // `Instalado` o `Portable` da igual para esto: el fichero de ajustes es
    // `raiz/pixpinmax.toml` en los dos casos (`Ubicacion::fichero_ajustes`).
    let donde = Ubicacion::Instalado {
        raiz: raiz.to_path_buf(),
    };
    if pixpin_store::ajustes::cargar(&donde)
        .map(|a| a.sincro.lo_mio_manda)
        .unwrap_or(false)
    {
        s.quien_manda(pixpin_sincro::mezcla::Mando::LoMioManda);
        tracing::info!("sincronizando con «lo mio manda»: este equipo se impone");
    }
    let suyo = if s.puerto_del_otro > 0 {
        s.puerto_del_otro as u16
    } else {
        puerto
    };
    apuntar_direccion(raiz, &s.otro.id, host, suyo);
    let _ = tx.send(Aviso::Identidad);
    let mut elegir = |otro: &m::Aparato, pregunta: vuelta::Pregunta| {
        let (responde, espera) = mpsc::channel();
        tx.send(Aviso::Elegir {
            otro: otro.nombre.clone(),
            pregunta,
            responde,
        })
        .ok()?;
        espera.recv().ok().flatten()
    };
    let mut progreso = |t: vuelta::Trabajo| {
        let _ = tx.send(Aviso::Fase(Fase::Progreso(t)));
    };
    let r = vuelta::una(
        &mut s,
        if preguntar { Some(&mut elegir) } else { None },
        hecho,
        rotulo,
        &ahora_ms,
        &mut progreso,
    );
    if r.is_err() {
        s.soltar();
    }
    // Lo que llego sin marco de la tinta recibe el suyo, en otro hilo (K21).
    crate::marco_de_la_tinta::tras_sincronizar(raiz);
    Ok(r?)
}

/// **Todos mis aparatos de una vez** (`sincronizarConTodos`): uno detras de
/// otro y sin preguntar, con lo elegido la ultima vez con cada uno. Con dos
/// o mas, dos rondas: en la primera este equipo queda con todo, pero los
/// primeros se quedaron con lo de antes de hablar con los ultimos; la
/// segunda lo reparte, y cuesta poco porque solo viajan los cambios. Si uno
/// falla se sigue con los demas y se dice al final cual quedo pendiente.
fn lanzar_con_todos(
    raiz: &Path,
    tx: &mpsc::Sender<Aviso>,
    lista: Vec<(String, String, u16)>,
    mi_puerto: Option<u16>,
) {
    if lista.is_empty() {
        return;
    }
    let raiz = raiz.to_path_buf();
    let tx = tx.clone();
    let _ = std::thread::Builder::new()
        .name("sincro-todos".into())
        .spawn(move || {
            // Toda la ronda cuenta como una: entre aparato y aparato la
            // automatica no puede colarse.
            let _en_curso = EnCurso::nueva();
            let mut hecho = Hecho::default();
            let mut avisos: Vec<String> = Vec::new();
            let mut fallaron: Vec<(String, String)> = Vec::new();
            let mut al_dia: Vec<String> = Vec::new();
            let mut bytes = 0u64;
            let empezo = std::time::Instant::now();
            let rondas = if lista.len() >= 2 { 2 } else { 1 };
            for ronda in 1..=rondas {
                for (nombre, host, puerto) in &lista {
                    // Uno que ya fallo no se reintenta: si estaba apagado lo
                    // sigue estando, y cada intento cuesta agotar la conexion.
                    if fallaron.iter().any(|(n, _)| n == nombre) {
                        continue;
                    }
                    let rotulo = if rondas > 1 {
                        format!("Ronda {ronda} de {rondas} · ")
                    } else {
                        String::new()
                    };
                    match una_vuelta(
                        &raiz, &tx, host, *puerto, nombre, mi_puerto, false, &mut hecho, &rotulo,
                    ) {
                        Ok(v) if v.cancelada => {
                            let _ = tx.send(Aviso::Fase(Fase::Nada));
                            return;
                        }
                        Ok(v) => {
                            bytes += v.bytes;
                            for a in v.avisos {
                                if !avisos.contains(&a) {
                                    avisos.push(a);
                                }
                            }
                            if !al_dia.contains(&v.nombre) {
                                al_dia.push(v.nombre);
                            }
                        }
                        Err(e) => fallaron.push((nombre.clone(), explicar(&e, nombre))),
                    }
                }
            }
            let segundos = empezo.elapsed().as_secs_f64();
            let fase = if al_dia.is_empty() {
                let t: Vec<String> = fallaron.iter().map(|(_, t)| t.clone()).collect();
                Fase::Fallo(if t.is_empty() {
                    "No se pudo con ninguno.".into()
                } else {
                    t.join("\n\n")
                })
            } else {
                let titulo = if al_dia.len() == 1 {
                    format!("Al día con {}", al_dia[0])
                } else {
                    format!("Al día con {} aparatos", al_dia.len())
                };
                let mut texto = vuelta::contar_lo_hecho(&hecho, bytes, segundos);
                if al_dia.len() > 1 {
                    texto.push('\n');
                    texto.push_str(&al_dia.join(" · "));
                }
                let mut todos = vuelta::avisos_de_lo_hecho(&hecho);
                todos.extend(avisos);
                if let Some((_, primero)) = fallaron.first() {
                    let pendientes: Vec<String> =
                        fallaron.iter().map(|(n, _)| format!("«{n}»")).collect();
                    todos.push(format!(
                        "Quedó pendiente {}: {primero}",
                        pendientes.join(", ")
                    ));
                }
                Fase::Terminado {
                    titulo,
                    texto,
                    aviso: (!todos.is_empty()).then(|| todos.join("\n\n")),
                    trajo: hecho.traidos > 0 || hecho.fusionados > 0,
                }
            };
            let _ = tx.send(Aviso::Identidad);
            let _ = tx.send(Aviso::Fase(fase));
        });
}

// ------------------------------------------------------------------ iconos

/// Un icono de Material Icons: un solo trazado relleno de 24 x 24.
macro_rules! material {
    ($d:expr) => {
        Icono {
            vista: (0.0, 0.0, 24.0, 24.0),
            trazos: &[TrazoIcono {
                d: $d,
                relleno: Pintura::Actual,
                trazo: Pintura::Nada,
                grosor: 0.0,
                extremo_redondo: false,
                union_redonda: false,
                opacidad: 1.0,
                par_impar: false,
                matriz: None,
                mascara: None,
            }],
        }
    };
}

// Los iconos de la pantalla del movil: los mismos `Icons.Filled.*` que usa
// `SincronizarActivity.kt`, sacados de Material Icons (Apache-2.0,
// `google/material-design-icons`, `src/<grupo>/<nombre>/materialicons/24px.svg`).
/// `ArrowBack`.
const VOLVER: Icono = material!("M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z");
/// `Edit`.
const EDITAR: Icono = material!(
    "M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34c-.39-.39-1.02-.39-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z"
);
/// `Sync`.
const SINCRO: Icono = material!(
    "M12 4V1L8 5l4 4V6c3.31 0 6 2.69 6 6 0 1.01-.25 1.97-.7 2.8l1.46 1.46C19.54 15.03 20 13.57 20 12c0-4.42-3.58-8-8-8zm0 14c-3.31 0-6-2.69-6-6 0-1.01.25-1.97.7-2.8L5.24 7.74C4.46 8.97 4 10.43 4 12c0 4.42 3.58 8 8 8v3l4-4-4-4v3z"
);
/// `Download`.
const BAJAR: Icono = material!("M5,20h14v-2H5V20z M19,9h-4V3H9v6H5l7,7L19,9z");
/// `Send` (el `AutoMirrored`, que en izquierda a derecha es el mismo).
const ENVIAR: Icono = material!("M2.01 21L23 12 2.01 3 2 10l15 2-15 2z");
/// `History`: el de «Copias de seguridad», volver a como estaba.
const HISTORIAL: Icono = material!(
    "M13 3c-4.97 0-9 4.03-9 9H1l3.89 3.89.07.14L9 12H6c0-3.87 3.13-7 7-7s7 3.13 7 7-3.13 7-7 7c-1.93 0-3.68-.79-4.94-2.06l-1.42 1.42C8.27 19.99 10.51 21 13 21c4.97 0 9-4.03 9-9s-4.03-9-9-9zm-1 5v5l4.28 2.54.72-1.21-3.5-2.08V8H12z"
);
/// `Close`.
const CERRAR: Icono = material!(
    "M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"
);

// ------------------------------------------------------------------ pintar

/// Un aparato del grupo en la lista.
struct Miembro {
    id: String,
    nombre: String,
    letra: Option<String>,
    host: Option<(String, u16)>,
    cerca: bool,
}

fn miembros(s: &Pantalla) -> Vec<Miembro> {
    let mut v: Vec<Miembro> = s
        .identidad
        .miembros
        .iter()
        .filter(|x| x.id != s.identidad.yo.id)
        .map(|x| Miembro {
            id: x.id.clone(),
            nombre: x.nombre.clone(),
            letra: x.letra.clone(),
            host: s
                .direcciones
                .iter()
                .find(|(i, _, _)| *i == x.id)
                .map(|(_, h, pu)| (h.clone(), *pu)),
            cerca: s.responden.get(&x.id).copied().unwrap_or(false),
        })
        .collect();
    // Los disponibles primero, como en el movil.
    v.sort_by(|a, b| b.cerca.cmp(&a.cerca).then_with(|| a.nombre.cmp(&b.nombre)));
    v
}

fn rect(x: f32, y: f32, ancho: f32, alto: f32) -> RectF {
    RectF { x, y, ancho, alto }
}

fn menos(r: RectF, d: f32) -> RectF {
    rect(r.x + d, r.y + d, r.ancho - 2.0 * d, r.alto - 2.0 * d)
}

/// La `Caja` del movil: borde, fondo propio y esquinas de 18.
fn caja(p: &Pintor, r: RectF, e: f32) {
    p.rellenar_redondeado(r, 18.0 * e, BORDE);
    p.rellenar_redondeado(menos(r, e), 17.0 * e, CAJA);
}

fn circulo(p: &Pintor, cx: f32, cy: f32, lado: f32, color: Color) {
    p.rellenar_redondeado(
        rect(cx - lado / 2.0, cy - lado / 2.0, lado, lado),
        lado / 2.0,
        color,
    );
}

/// `Letra` del movil: la letra del aparato en un circulo.
fn letra(p: &Pintor, letra: Option<&str>, cx: f32, cy: f32, lado: f32) {
    circulo(p, cx, cy, lado, CONTENEDOR);
    let t = letra
        .map(|l| l.to_uppercase())
        .unwrap_or_else(|| "·".into());
    let tam = lado * 0.46;
    let (w, h) = p.medir_texto(&t, tam);
    p.texto(&t, cx - w / 2.0, cy - h / 2.0, tam, SOBRE_CONTENEDOR);
}

/// Lo que se pinta y se puede pulsar: se apunta junto con su accion.
struct Lienzo<'a> {
    p: &'a Pintor<'a>,
    e: f32,
    zonas: Vec<(RectF, Accion)>,
}

impl Lienzo<'_> {
    /// Un boton de Material: lleno (`Button`) o con borde (`OutlinedButton`).
    fn boton(&mut self, r: RectF, texto: &str, icono: Option<&Icono>, lleno: bool, a: Accion) {
        let (p, e) = (self.p, self.e);
        let radio = r.alto / 2.0;
        if lleno {
            p.rellenar_redondeado(r, radio, PRIMARIO);
        } else {
            p.rellenar_redondeado(r, radio, CONTORNO);
            p.rellenar_redondeado(menos(r, e), radio - e, CAJA);
        }
        let color = if lleno { SOBRE_PRIMARIO } else { PRIMARIO };
        let tam = 14.0 * e;
        let (w, h) = p.medir_texto(texto, tam);
        let lado = 18.0 * e;
        let total = w + icono.map_or(0.0, |_| lado + 8.0 * e);
        let mut x = r.x + (r.ancho - total) / 2.0;
        if let Some(i) = icono {
            p.icono(i, rect(x, r.y + (r.alto - lado) / 2.0, lado, lado), color);
            x += lado + 8.0 * e;
        }
        p.texto(texto, x, r.y + (r.alto - h) / 2.0, tam, color);
        self.zonas.push((r, a));
    }

    /// Un `TextButton`: solo el texto. Devuelve lo que ocupa de ancho.
    fn boton_texto(&mut self, x: f32, y: f32, texto: &str, color: Color, a: Accion) -> f32 {
        let (p, e) = (self.p, self.e);
        let tam = 14.0 * e;
        let (w, h) = p.medir_texto(texto, tam);
        let r = rect(x, y, w + 24.0 * e, 40.0 * e);
        p.texto(texto, x + 12.0 * e, y + (r.alto - h) / 2.0, tam, color);
        self.zonas.push((r, a));
        r.ancho
    }

    /// Un `IconButton`: el icono de 24 y una zona de toque de 48.
    fn icono(&mut self, i: &Icono, cx: f32, cy: f32, color: Color, a: Accion) {
        let e = self.e;
        let lado = 24.0 * e;
        self.p
            .icono(i, rect(cx - lado / 2.0, cy - lado / 2.0, lado, lado), color);
        let toque = 48.0 * e;
        self.zonas
            .push((rect(cx - toque / 2.0, cy - toque / 2.0, toque, toque), a));
    }

    /// Un parrafo partido en lineas; devuelve el alto que ocupo.
    fn parrafo(&self, texto: &str, x: f32, y: f32, ancho: f32, tam: f32, color: Color) -> f32 {
        let (_, h) = self.p.medir_texto_ajustado(texto, tam, ancho);
        self.p.texto_ajustado(texto, x, y, tam, ancho, color);
        h
    }

    fn alto_de(&self, texto: &str, tam: f32, ancho: f32) -> f32 {
        self.p.medir_texto_ajustado(texto, tam, ancho).1
    }

    /// Una linea centrada en `[x, x + ancho]`.
    fn centrado(&self, texto: &str, x: f32, ancho: f32, y: f32, tam: f32, color: Color) {
        let (w, _) = self.p.medir_texto(texto, tam);
        if w <= ancho {
            self.p.texto(texto, x + (ancho - w) / 2.0, y, tam, color);
        } else {
            self.p.texto_linea(texto, x, y, tam, ancho, color);
        }
    }

    /// Un parrafo con cada linea centrada, como `TextAlign.Center`. DirectWrite
    /// lo haria solo, pero el pintor solo sabe alinear a la izquierda: se
    /// parte a mano por palabras. Devuelve el alto que ocupo.
    #[allow(clippy::too_many_arguments)]
    fn parrafo_centrado(
        &self,
        texto: &str,
        x: f32,
        y: f32,
        ancho: f32,
        tam: f32,
        color: Color,
    ) -> f32 {
        let (_, alto_linea) = self.p.medir_texto("Ag", tam);
        let mut yy = y;
        for renglon in texto.lines() {
            let mut linea = String::new();
            for palabra in renglon.split_whitespace() {
                let prueba = if linea.is_empty() {
                    palabra.to_string()
                } else {
                    format!("{linea} {palabra}")
                };
                if !linea.is_empty() && self.p.medir_texto(&prueba, tam).0 > ancho {
                    self.centrado(&linea, x, ancho, yy, tam, color);
                    yy += alto_linea;
                    linea = palabra.to_string();
                } else {
                    linea = prueba;
                }
            }
            self.centrado(&linea, x, ancho, yy, tam, color);
            yy += alto_linea;
        }
        yy - y
    }

    /// `CircularProgressIndicator`: tres cuartos de anillo que giran con el
    /// reloj. La ventana se repinta mientras hay uno (ver `animando`).
    fn girando(&self, cx: f32, cy: f32, radio: f32, grosor: f32) {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() % 1200)
            .unwrap_or(0) as f32
            / 1200.0;
        let inicio = t * std::f32::consts::TAU;
        let puntos: Vec<(f32, f32)> = (0..=36)
            .map(|i| {
                let a = inicio + i as f32 / 36.0 * std::f32::consts::TAU * 0.75;
                (cx + radio * a.cos(), cy + radio * a.sin())
            })
            .collect();
        self.p.polilinea(&puntos, grosor, PRIMARIO);
    }

    /// `LinearProgressIndicator`: la pista y lo hecho encima.
    fn barra(&self, r: RectF, parte: f32) {
        self.p.rellenar_redondeado(r, r.alto / 2.0, VARIANTE);
        let lleno = rect(r.x, r.y, r.ancho * parte.clamp(0.0, 1.0), r.alto);
        if lleno.ancho > 0.0 {
            self.p.rellenar_redondeado(lleno, r.alto / 2.0, PRIMARIO);
        }
    }

    /// `Titulo` del movil: el rotulo de cada seccion, en el color primario.
    fn titulo(&self, texto: &str, x: f32, y: &mut f32) {
        let e = self.e;
        *y += 22.0 * e;
        self.p.texto(texto, x + 4.0 * e, *y, 14.0 * e, PRIMARIO);
        *y += 28.0 * e;
    }
}

fn pintar(
    p: &Pintor,
    marco: Rect,
    escala: u32,
    textos: &Catalogo,
    s: &mut Pantalla,
    zonas: &mut Vec<(RectF, Accion)>,
) {
    let e = escala as f32 / 100.0;
    let ancho = marco.ancho as f32;
    let alto = marco.alto as f32;
    p.rellenar(rect(0.0, 0.0, ancho, alto), FONDO);

    let barra = BARRA * e;
    let mut l = Lienzo {
        p,
        e,
        zonas: Vec::new(),
    };

    // «¿Que sincronizar?» lleva abajo una barra fija con sus botones.
    let pie = if matches!(s.sub, Sub::Elegir(_)) {
        72.0 * e
    } else {
        0.0
    };

    // ---- el contenido, desplazable, por debajo de la barra
    p.empujar_recorte(rect(0.0, barra, ancho, alto - barra - pie));
    let mut y = barra - s.desplazamiento;
    match &s.sub {
        Sub::Recibir(r) => {
            // El movil deja 20 dp a los lados en estas pantallas.
            y = recibir_wifi::pintar(&mut l, textos, r, 20.0 * e, ancho - 40.0 * e, y);
        }
        Sub::Enviar(en) => {
            y = enviar_wifi::pintar(&mut l, textos, en, 20.0 * e, ancho - 40.0 * e, y);
        }
        Sub::Elegir(el) => {
            y = elegir::pintar(&mut l, textos, el, &s.identidad.yo.nombre, ancho, y);
        }
        Sub::Copias(c) => {
            y = copias_ui::pintar(&mut l, textos, c, 20.0 * e, ancho - 40.0 * e, y);
        }
        Sub::Portada => y = pintar_portada(&mut l, textos, s, ancho, y),
    }
    y += 40.0 * e + pie;
    s.alto_contenido = y + s.desplazamiento - barra;
    p.soltar_recorte();

    // Lo de dentro solo es pulsable donde se ve: bajo la barra no.
    zonas.extend(
        std::mem::take(&mut l.zonas)
            .into_iter()
            .filter(|(r, _)| r.y + r.alto > barra && r.y < alto - pie),
    );
    if let Sub::Elegir(el) = &s.sub {
        elegir::pintar_pie(&mut l, textos, el, ancho, alto - pie, pie);
    }

    // ---- la barra: volver, el titulo y cerrar
    p.rellenar(rect(0.0, 0.0, ancho, barra), FONDO);
    let (titulo, vuelta) = match &s.sub {
        Sub::Portada => (textos.t("sinc-titulo"), Accion::Cerrar),
        Sub::Recibir(_) => (textos.t("rw-titulo"), Accion::Volver),
        Sub::Enviar(_) => (textos.t("ew-titulo"), Accion::Volver),
        Sub::Elegir(_) => (
            textos.t("sinc-elegir-titulo"),
            Accion::Elegir(elegir::Toque::Cancelar),
        ),
        Sub::Copias(_) => (textos.t("cop-titulo"), Accion::Volver),
    };
    l.icono(&VOLVER, 28.0 * e, barra / 2.0, TEXTO, vuelta);
    let tam = 24.0 * e;
    let (_, h) = p.medir_texto(&titulo, tam);
    p.texto_linea(
        &titulo,
        56.0 * e,
        (barra - h) / 2.0,
        tam,
        ancho - 112.0 * e,
        TEXTO,
    );
    l.icono(
        &CERRAR,
        ancho - 28.0 * e,
        barra / 2.0,
        SUAVE,
        Accion::Cerrar,
    );

    // ---- los dialogos, encima de todo
    if let Sub::Copias(c) = &s.sub {
        // El de «¿Volver a esta copia?» es suyo y se pinta solo; si esta,
        // nada de debajo se toca.
        let mut suyo = Lienzo {
            p,
            e,
            zonas: Vec::new(),
        };
        if copias_ui::dialogo(&mut suyo, textos, c, ancho, alto) {
            l.zonas.clear();
            zonas.clear();
            zonas.append(&mut suyo.zonas);
            return;
        }
    }
    if s.campo.is_some() || s.saliendo || s.fase != Fase::Nada {
        // Lo de debajo deja de ser pulsable mientras hay un dialogo, como
        // en Android.
        l.zonas.clear();
        zonas.clear();
        p.rellenar(rect(0.0, 0.0, ancho, alto), VELO);
        dialogo(&mut l, ancho, alto, textos, s);
    }
    zonas.append(&mut l.zonas);
}

/// La portada: las cajas de la pantalla del movil. Devuelve donde acaba.
fn pintar_portada(
    l: &mut Lienzo<'_>,
    textos: &Catalogo,
    s: &Pantalla,
    ancho: f32,
    mut y: f32,
) -> f32 {
    let (p, e) = (l.p, l.e);
    let x0 = 16.0 * e;
    let w = ancho - 32.0 * e;
    let xi = x0 + 16.0 * e;
    let wi = w - 32.0 * e;
    p.texto(&textos.t("sinc-subtitulo"), xi, y, 14.0 * e, SUAVE);
    y += 36.0 * e;

    // Este aparato
    let r = rect(x0, y, w, 80.0 * e);
    caja(p, r, e);
    letra(
        p,
        s.identidad.yo.letra.as_deref(),
        xi + 24.0 * e,
        r.y + r.alto / 2.0,
        48.0 * e,
    );
    p.texto(
        &textos.t("sinc-este-aparato"),
        xi + 62.0 * e,
        r.y + 16.0 * e,
        12.0 * e,
        SUAVE,
    );
    p.texto_linea(
        &s.identidad.yo.nombre,
        xi + 62.0 * e,
        r.y + 34.0 * e,
        22.0 * e,
        r.ancho - 140.0 * e,
        TEXTO,
    );
    l.icono(
        &EDITAR,
        r.x + r.ancho - 36.0 * e,
        r.y + r.alto / 2.0,
        SUAVE,
        Accion::Renombrar,
    );
    y += r.alto;

    // Tus aparatos
    l.titulo(&textos.t("sinc-tus-aparatos"), x0, &mut y);
    if s.identidad.codigo.is_none() {
        let txt = textos.t("sinc-sin-grupo");
        let h = l.alto_de(&txt, 14.0 * e, wi);
        let alto_caja = 16.0 * e + h + 14.0 * e + 56.0 * e + 48.0 * e + 16.0 * e;
        caja(p, rect(x0, y, w, alto_caja), e);
        l.parrafo(&txt, xi, y + 16.0 * e, wi, 14.0 * e, TEXTO);
        let yb = y + 30.0 * e + h;
        l.boton(
            rect(xi, yb, wi, 48.0 * e),
            &textos.t("sinc-crear-grupo"),
            None,
            true,
            Accion::CrearGrupo,
        );
        l.boton(
            rect(xi, yb + 56.0 * e, wi, 48.0 * e),
            &textos.t("sinc-unirme"),
            None,
            false,
            Accion::Unirme,
        );
        y += alto_caja;
    } else {
        y = pintar_grupo(l, textos, s, x0, w, y);
    }

    // Pasar algo a otra persona
    l.titulo(&textos.t("sinc-pasar"), x0, &mut y);
    let txt = textos.t("sinc-pasar-como");
    let tambien = textos.t("sinc-pasar-tambien");
    let h1 = l.alto_de(&txt, 14.0 * e, wi);
    let h2 = l.alto_de(&tambien, 12.0 * e, wi);
    let hp = 16.0 * e + h1 + 12.0 * e + 48.0 * e + 8.0 * e + h2 + 16.0 * e;
    caja(p, rect(x0, y, w, hp), e);
    l.parrafo(&txt, xi, y + 16.0 * e, wi, 14.0 * e, TEXTO);
    let yb = y + 28.0 * e + h1;
    let medio = (wi - 10.0 * e) / 2.0;
    l.boton(
        rect(xi, yb, medio, 48.0 * e),
        &textos.t("sinc-recibir"),
        Some(&BAJAR),
        true,
        Accion::AbrirRecibir,
    );
    l.boton(
        rect(xi + medio + 10.0 * e, yb, medio, 48.0 * e),
        &textos.t("sinc-enviar"),
        Some(&ENVIAR),
        false,
        Accion::AbrirEnviar,
    );
    l.parrafo(&tambien, xi, yb + 56.0 * e, wi, 12.0 * e, SUAVE);
    y += hp;

    // Copias de seguridad (`CopiasActivity` del movil): el sitio al que ir
    // cuando un envio o una vuelta ha pisado algo.
    l.titulo(&textos.t("cop-titulo"), x0, &mut y);
    let cc = textos.t("cop-portada-como");
    let hcc = l.alto_de(&cc, 14.0 * e, wi);
    let hb = 16.0 * e + hcc + 12.0 * e + 48.0 * e + 16.0 * e;
    caja(p, rect(x0, y, w, hb), e);
    l.parrafo(&cc, xi, y + 16.0 * e, wi, 14.0 * e, TEXTO);
    l.boton(
        rect(xi, y + 28.0 * e + hcc, wi, 48.0 * e),
        &textos.t("cop-ver"),
        Some(&HISTORIAL),
        false,
        Accion::AbrirCopias,
    );
    y += hb;

    // Actividad
    let apagada = !presencia::encendida();
    if !s.registro.is_empty() || !s.mi_direccion.is_empty() || apagada {
        l.titulo(&textos.t("sinc-actividad"), x0, &mut y);
        let lineas = s.registro.len() + usize::from(!s.mi_direccion.is_empty());
        // Sin presencia, el movil no puede llamar a este equipo, y eso hay
        // que decirlo donde se dice por donde se escucha.
        let aviso = apagada.then(|| textos.t("sinc-presencia-apagada"));
        let ha = 16.0 * e
            + 20.0 * e * lineas as f32
            + aviso
                .as_ref()
                .map_or(0.0, |t| l.alto_de(t, 12.0 * e, wi) + 4.0 * e)
            + 12.0 * e;
        caja(p, rect(x0, y, w, ha), e);
        let mut yl = y + 16.0 * e;
        if let Some(t) = &aviso {
            yl += l.parrafo(t, xi, yl, wi, 12.0 * e, ERROR) + 4.0 * e;
        }
        if !s.mi_direccion.is_empty() {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("dir", s.mi_direccion.clone());
            p.texto_linea(
                &textos.t_args("sinc-escuchando", &args),
                xi,
                yl,
                12.0 * e,
                wi,
                SUAVE,
            );
            yl += 20.0 * e;
        }
        for linea in &s.registro {
            p.texto_linea(linea, xi, yl, 12.0 * e, wi, SUAVE);
            yl += 20.0 * e;
        }
        y += ha;
    }
    y
}

/// «Tus aparatos» con grupo, y la caja de «Anadir otro aparato». Devuelve
/// donde acaba.
fn pintar_grupo(
    l: &mut Lienzo<'_>,
    textos: &Catalogo,
    s: &Pantalla,
    x0: f32,
    w: f32,
    mut y: f32,
) -> f32 {
    let (p, e) = (l.p, l.e);
    let xi = x0 + 16.0 * e;
    let wi = w - 32.0 * e;
    let lista = miembros(s);
    let ahora = ahora_ms();
    let fila = 64.0 * e;
    let disponibles: Vec<&Miembro> = lista
        .iter()
        .filter(|m| m.cerca && m.host.is_some())
        .collect();
    let vacio = textos.t("sinc-grupo-vacio");
    let mut alto_caja = 12.0 * e + 40.0 * e + fila * lista.len() as f32;
    if lista.is_empty() {
        alto_caja += l.alto_de(&vacio, 14.0 * e, wi) + 24.0 * e;
    }
    if disponibles.len() > 1 {
        alto_caja += 88.0 * e;
    }
    if lista.iter().any(|m| !m.cerca) {
        alto_caja += 28.0 * e;
    }
    caja(p, rect(x0, y, w, alto_caja), e);
    let y0 = y;
    y += 6.0 * e;
    if lista.is_empty() {
        y += l.parrafo(&vacio, xi, y + 12.0 * e, wi, 14.0 * e, TEXTO) + 24.0 * e;
    }
    for (n, mi) in lista.iter().enumerate() {
        if n > 0 {
            p.rellenar(rect(xi, y, wi, e.max(1.0)), BORDE);
        }
        let cy = y + fila / 2.0;
        letra(p, mi.letra.as_deref(), xi + 18.0 * e, cy, 36.0 * e);
        let xt = xi + 48.0 * e;
        let ancho_boton = if mi.host.is_some() { 150.0 * e } else { 0.0 };
        p.texto_linea(
            &mi.nombre,
            xt,
            cy - 18.0 * e,
            15.0 * e,
            wi - 48.0 * e - ancho_boton,
            TEXTO,
        );
        circulo(
            p,
            xt + 4.0 * e,
            cy + 11.0 * e,
            8.0 * e,
            if mi.cerca { VERDE } else { CONTORNO },
        );
        let cuando = ultima_vez(&s.raiz, &mi.id);
        let mut estado = if mi.cerca {
            textos.t("sinc-disponible")
        } else {
            textos.t("sinc-no-encontrado")
        };
        if cuando > 0 {
            estado.push_str(&format!(" · sincronizado {}", hace_cuanto(cuando, ahora)));
        }
        p.texto_linea(
            &estado,
            xt + 14.0 * e,
            cy + 3.0 * e,
            12.0 * e,
            wi - 62.0 * e - ancho_boton,
            SUAVE,
        );
        if let Some((h, pu)) = &mi.host {
            let a = Accion::SincronizarCon(mi.nombre.clone(), h.clone(), *pu);
            let r = rect(
                xi + wi - ancho_boton + 10.0 * e,
                cy - 20.0 * e,
                ancho_boton - 10.0 * e,
                40.0 * e,
            );
            if mi.cerca {
                l.boton(r, &textos.t("sinc-sincronizar"), Some(&SINCRO), true, a);
            } else {
                l.boton(r, &textos.t("sinc-probar"), None, false, a);
            }
        }
        y += fila;
    }
    if disponibles.len() > 1 {
        // «Sincronizar con todos (n)»: uno detras de otro y sin preguntar
        // (`sincronizarConTodos`).
        let todos: Vec<(String, String, u16)> = disponibles
            .iter()
            .filter_map(|m| {
                let (h, pu) = m.host.as_ref()?;
                Some((m.nombre.clone(), h.clone(), *pu))
            })
            .collect();
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("n", disponibles.len().to_string());
        l.boton(
            rect(xi, y + 8.0 * e, wi, 48.0 * e),
            &textos.t_args("sinc-con-todos", &args),
            Some(&SINCRO),
            true,
            Accion::SincronizarConTodos(todos),
        );
        y += 64.0 * e;
        // Con tres o mas se dice que van en dos rondas, como en el movil.
        let como = if disponibles.len() > 2 {
            textos.t("sinc-con-todos-rondas")
        } else {
            textos.t("sinc-con-todos-como")
        };
        p.texto_linea(&como, xi, y, 12.0 * e, wi, SUAVE);
        y += 24.0 * e;
    }
    if lista.iter().any(|m| !m.cerca) {
        p.texto_linea(
            &textos.t("sinc-si-no-aparece"),
            xi,
            y + 6.0 * e,
            12.0 * e,
            wi,
            SUAVE,
        );
        y += 28.0 * e;
    }
    l.boton_texto(
        x0 + 6.0 * e,
        y,
        &textos.t("sinc-con-direccion"),
        PRIMARIO,
        Accion::ConectarDireccion,
    );
    y = y0 + alto_caja + 12.0 * e;

    // Anadir otro aparato
    let como = textos.t("sinc-anadir-como");
    let hc_como = l.alto_de(&como, 14.0 * e, wi);
    let hc = 16.0 * e + 26.0 * e + hc_como + 10.0 * e + 60.0 * e + 4.0 * e + 40.0 * e + 12.0 * e;
    caja(p, rect(x0, y, w, hc), e);
    p.texto(&textos.t("sinc-anadir"), xi, y + 16.0 * e, 16.0 * e, TEXTO);
    l.parrafo(&como, xi, y + 42.0 * e, wi, 14.0 * e, TEXTO);
    let rc = rect(xi, y + 52.0 * e + hc_como, wi, 60.0 * e);
    p.rellenar_redondeado(rc, 12.0 * e, VARIANTE);
    let (t, tam) = if s.ver_codigo {
        (
            legible(s.identidad.codigo.as_deref().unwrap_or_default()),
            26.0 * e,
        )
    } else {
        (textos.t("sinc-toca-codigo"), 15.0 * e)
    };
    let (wt, ht) = p.medir_texto(&t, tam);
    p.texto(
        &t,
        rc.x + (rc.ancho - wt) / 2.0,
        rc.y + (rc.alto - ht) / 2.0,
        tam,
        TEXTO,
    );
    l.zonas.push((rc, Accion::VerCodigo));
    l.boton_texto(
        xi - 12.0 * e,
        rc.y + rc.alto + 4.0 * e,
        &textos.t("sinc-salir"),
        ERROR,
        Accion::SalirDelGrupo,
    );
    y + hc
}

/// Un `OutlinedTextField`: el rotulo sobre el borde, la pista en gris y el
/// cursor al final.
#[allow(clippy::too_many_arguments)]
fn campo(
    l: &mut Lienzo<'_>,
    r: RectF,
    rotulo: &str,
    texto: &str,
    pista: &str,
    activo: bool,
    a: Accion,
) {
    let (p, e) = (l.p, l.e);
    let borde = if activo { 2.0 * e } else { e };
    p.rellenar_redondeado(r, 6.0 * e, if activo { PRIMARIO } else { CONTORNO });
    p.rellenar_redondeado(menos(r, borde), 5.0 * e, DIALOGO);
    if !rotulo.is_empty() {
        let (wr, _) = p.medir_texto(rotulo, 11.0 * e);
        p.rellenar(
            rect(r.x + 10.0 * e, r.y - 7.0 * e, wr + 8.0 * e, 14.0 * e),
            DIALOGO,
        );
        p.texto(
            rotulo,
            r.x + 14.0 * e,
            r.y - 8.0 * e,
            11.0 * e,
            if activo { PRIMARIO } else { SUAVE },
        );
    }
    let (mostrar, color) = if texto.is_empty() {
        (pista, SUAVE)
    } else {
        (texto, TEXTO)
    };
    let tam = 16.0 * e;
    let (wt, ht) = p.medir_texto(mostrar, tam);
    p.texto(
        mostrar,
        r.x + 16.0 * e,
        r.y + (r.alto - ht) / 2.0,
        tam,
        color,
    );
    if activo {
        let xc = r.x + 16.0 * e + if texto.is_empty() { 0.0 } else { wt } + e;
        p.rellenar(
            rect(xc, r.y + 14.0 * e, 2.0 * e, r.alto - 28.0 * e),
            PRIMARIO,
        );
    }
    l.zonas.push((r, a));
}

/// Los `AlertDialog` del movil: titulo, texto, campos y los botones de texto
/// abajo a la derecha.
/// La barra de «Sincronizando»: con total, lo hecho y «x de y · velocidad»
/// debajo (`LinearProgressIndicator(progress = …)`); sin total, un trozo que
/// va y viene, como el indeterminado del movil.
fn barra_de_progreso(l: &Lienzo<'_>, t: &vuelta::Trabajo, r: RectF) {
    let (p, e) = (l.p, l.e);
    if t.total > 0 {
        l.barra(r, t.hechos as f32 / t.total as f32);
        let hecho = vuelta::tamano_legible(t.hechos as i64);
        let texto = format!(
            "{} de {} · {}",
            if hecho.is_empty() {
                "0 B".into()
            } else {
                hecho
            },
            vuelta::tamano_legible(t.total as i64),
            t.velocidad
        );
        p.texto_linea(&texto, r.x, r.y + 12.0 * e, 12.0 * e, r.ancho, SUAVE);
    } else {
        p.rellenar_redondeado(r, r.alto / 2.0, VARIANTE);
        let fase = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() % 1600)
            .unwrap_or(0) as f32
            / 1600.0;
        let largo = r.ancho * 0.3;
        let x = r.x - largo + (r.ancho + largo) * fase;
        let desde = x.max(r.x);
        let hasta = (x + largo).min(r.x + r.ancho);
        if hasta > desde {
            p.rellenar_redondeado(
                rect(desde, r.y, hasta - desde, r.alto),
                r.alto / 2.0,
                PRIMARIO,
            );
        }
    }
}

fn dialogo(l: &mut Lienzo<'_>, ancho: f32, alto: f32, textos: &Catalogo, s: &Pantalla) {
    let (p, e) = (l.p, l.e);
    let w = (ancho - 48.0 * e).min(380.0 * e);
    let x = (ancho - w) / 2.0;
    let dentro = w - 48.0 * e;
    let (titulo, cuerpo): (String, String) = if let Some(c) = &s.campo {
        match c.que {
            QueCampo::Nombre => (textos.t("sinc-nombre-titulo"), textos.t("sinc-nombre-como")),
            QueCampo::Codigo => (textos.t("sinc-unirme-titulo"), textos.t("sinc-unirme-como")),
            QueCampo::Direccion => (
                textos.t("sinc-direccion-titulo"),
                textos.t("sinc-direccion-como"),
            ),
        }
    } else if s.saliendo {
        (textos.t("sinc-salir-titulo"), textos.t("sinc-salir-como"))
    } else {
        match &s.fase {
            Fase::Trabajando(t) => (textos.t("sinc-un-momento"), t.clone()),
            Fase::Progreso(t) => (textos.t("sinc-sincronizando"), t.texto.clone()),
            Fase::Terminado { titulo, texto, .. } => (titulo.clone(), texto.clone()),
            Fase::Fallo(t) => (textos.t("sinc-no-se-pudo"), t.clone()),
            Fase::Nada => return,
        }
    };
    // Primero se mide, luego se pinta: el alto depende del texto.
    let h_cuerpo = l.alto_de(&cuerpo, 14.0 * e, dentro);
    let campos = match &s.campo {
        Some(c) if c.que == QueCampo::Codigo => 2.0,
        Some(_) => 1.0,
        None => 0.0,
    };
    let con_la_mia =
        matches!(&s.campo, Some(c) if c.que == QueCampo::Direccion) && !s.mi_direccion.is_empty();
    // Lo de debajo del texto en una vuelta: el aviso del final, en el color
    // de los errores, o la barra mientras pasan archivos.
    let sin_campo = s.campo.is_none() && !s.saliendo;
    let aviso = match &s.fase {
        Fase::Terminado { aviso: Some(a), .. } if sin_campo => Some(a.as_str()),
        _ => None,
    };
    let progreso = match &s.fase {
        Fase::Progreso(t) if sin_campo => Some(t),
        _ => None,
    };
    let h_aviso = aviso.map_or(0.0, |a| l.alto_de(a, 14.0 * e, dentro) + 10.0 * e);
    let h_barra = if progreso.is_some() { 36.0 * e } else { 0.0 };
    let h = 24.0 * e
        + 36.0 * e
        + h_cuerpo
        + 12.0 * e
        + h_aviso
        + h_barra
        + campos * 70.0 * e
        + if con_la_mia { 48.0 * e } else { 0.0 }
        + 56.0 * e;
    let y = ((alto - h) / 2.0).max(8.0 * e);
    p.rellenar_redondeado(rect(x, y, w, h), 28.0 * e, DIALOGO);
    let xi = x + 24.0 * e;
    let mut yy = y + 24.0 * e;
    p.texto_linea(&titulo, xi, yy, 22.0 * e, dentro, TEXTO);
    yy += 36.0 * e;
    yy += l.parrafo(&cuerpo, xi, yy, dentro, 14.0 * e, TEXTO) + 12.0 * e;
    if let Some(a) = aviso {
        yy += l.parrafo(a, xi, yy, dentro, 14.0 * e, ERROR) + 10.0 * e;
    }
    if let Some(t) = progreso {
        barra_de_progreso(l, t, rect(xi, yy, dentro, 4.0 * e));
        yy += h_barra;
    }

    if let Some(c) = &s.campo {
        let r = |yy: f32| rect(xi, yy + 8.0 * e, dentro, 52.0 * e);
        match c.que {
            QueCampo::Nombre => {
                campo(l, r(yy), "", &c.texto, "", true, Accion::CampoPrincipal);
                yy += 70.0 * e;
            }
            QueCampo::Codigo => {
                campo(
                    l,
                    r(yy),
                    &textos.t("sinc-codigo"),
                    &c.texto,
                    "ABCDE-23456",
                    !c.en_extra,
                    Accion::CampoPrincipal,
                );
                yy += 70.0 * e;
                campo(
                    l,
                    r(yy),
                    &textos.t("sinc-direccion"),
                    &c.extra,
                    "192.168.1.20:47474",
                    c.en_extra,
                    Accion::CampoExtra,
                );
                yy += 70.0 * e;
            }
            QueCampo::Direccion => {
                campo(
                    l,
                    r(yy),
                    "",
                    &c.texto,
                    "192.168.1.20:47474",
                    true,
                    Accion::CampoPrincipal,
                );
                yy += 70.0 * e;
                if con_la_mia {
                    p.texto(&textos.t("sinc-la-de-este"), xi, yy, 12.0 * e, SUAVE);
                    p.texto(&s.mi_direccion, xi, yy + 18.0 * e, 18.0 * e, TEXTO);
                    yy += 48.0 * e;
                }
            }
        }
    }

    // Los botones de texto, a la derecha, como los de `AlertDialog`. El de
    // aceptar se apaga mientras lo escrito no vale, como `enabled = ...`.
    let (principal, listo, accion, cancelar) = if let Some(c) = &s.campo {
        let (t, listo) = match c.que {
            QueCampo::Nombre => (textos.t("sinc-guardar"), !c.texto.trim().is_empty()),
            QueCampo::Codigo => (
                textos.t("sinc-unirme-boton"),
                codigo_valido(&pixpin_sincro::codigo::limpiar(&c.texto)),
            ),
            QueCampo::Direccion => (
                textos.t("sinc-conectar"),
                partir_direccion(&c.texto).is_some(),
            ),
        };
        (t, listo, Accion::Aceptar, true)
    } else if s.saliendo {
        (
            textos.t("sinc-salir-boton"),
            true,
            Accion::ConfirmarSalir,
            true,
        )
    } else if matches!(s.fase, Fase::Trabajando(_) | Fase::Progreso(_)) {
        (textos.t("sinc-ocultar"), true, Accion::CerrarFase, false)
    } else {
        (textos.t("sinc-vale"), true, Accion::CerrarFase, false)
    };
    let (wp, _) = p.medir_texto(&principal, 14.0 * e);
    let xp = x + w - 24.0 * e - wp - 24.0 * e;
    let yb = yy + 4.0 * e;
    l.boton_texto(
        xp,
        yb,
        &principal,
        if listo { PRIMARIO } else { CONTORNO },
        accion,
    );
    if cancelar {
        let t = textos.t("sinc-cancelar");
        let (ws, _) = p.medir_texto(&t, 14.0 * e);
        l.boton_texto(xp - ws - 32.0 * e, yb, &t, PRIMARIO, Accion::Cancelar);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Los tres casos de la cola de pedidos, en una sola prueba porque
    /// comparten el estatico y correr a la vez se pisarian.
    #[test]
    fn el_pedido_se_recoge_una_sola_vez_y_gana_el_ultimo() {
        PEDIDO.lock().unwrap().take();
        // Caso negativo: sin nadie que pida nada, no hay que entrar en nada.
        assert_eq!(tomar_pedido(), None);
        dejar_pedido(Pedido::Enviar(vec![PathBuf::from("uno.png")]));
        dejar_pedido(Pedido::Enviar(vec![PathBuf::from("dos.png")]));
        assert_eq!(
            tomar_pedido(),
            Some(Pedido::Enviar(vec![PathBuf::from("dos.png")]))
        );
        // Y recogido no vuelve: si no, cada vuelta del bucle volveria a
        // entrar en «Enviar» y no se podria salir de esa pantalla.
        assert_eq!(tomar_pedido(), None);
    }

    /// Sin ficheros no se abre nada: un mensaje sin adjunto no tiene que
    /// sacar la ventana de Sincronizar a la cara.
    #[test]
    fn enviar_por_wifi_sin_rutas_no_deja_pedido() {
        PEDIDO.lock().unwrap().take();
        enviar_por_wifi(
            pixpin_store::Idioma::Espanol,
            Ubicacion::Portable {
                raiz: PathBuf::from("."),
            },
            Vec::new(),
        );
        assert_eq!(tomar_pedido(), None);
    }

    #[test]
    fn una_direccion_con_puerto_se_parte_y_sin_puerto_va_el_de_siempre() {
        assert_eq!(
            partir_direccion(" 192.168.1.20:5000 "),
            Some(("192.168.1.20".into(), 5000))
        );
        assert_eq!(
            partir_direccion("192.168.1.20"),
            Some(("192.168.1.20".into(), pixpin_sincro::PUERTO))
        );
        assert_eq!(partir_direccion(""), None);
        assert_eq!(partir_direccion("1.2.3.4:abc"), None);
    }

    #[test]
    fn el_codigo_se_lee_en_dos_mitades_como_en_el_movil() {
        assert_eq!(legible("K7Q2M9XMPA"), "K7Q2M-9XMPA");
        assert_eq!(legible("K7Q2"), "K7Q2");
    }

    #[test]
    fn un_codigo_tecleado_con_guion_y_minusculas_vale_y_uno_corto_no() {
        assert!(codigo_valido(&pixpin_sincro::codigo::limpiar(
            "k7q2m-9xmpa"
        )));
        assert!(!codigo_valido(&pixpin_sincro::codigo::limpiar("k7q2m")));
    }

    #[test]
    fn hace_cuanto_habla_como_el_movil() {
        let h = 60_000;
        assert_eq!(hace_cuanto(0, 30_000), "ahora mismo");
        assert_eq!(hace_cuanto(0, 5 * h), "hace 5 min");
        assert_eq!(hace_cuanto(0, 120 * h), "hace 2 h");
        assert_eq!(hace_cuanto(0, 30 * 60 * h), "ayer");
        assert_eq!(hace_cuanto(0, 3 * 24 * 60 * h), "hace 3 días");
        assert_eq!(
            hace_cuanto(10, 0),
            "ahora mismo",
            "un reloj que va atras no da negativos"
        );
    }

    #[test]
    fn las_direcciones_se_leen_con_el_formato_del_movil_y_lo_roto_se_salta() {
        let v = leer_direcciones_de("abc\t192.168.1.5\t47474\nroto\nx\t1.2.3.4\tnope\n");
        assert_eq!(v, vec![("abc".into(), "192.168.1.5".into(), 47474)]);
    }

    #[test]
    fn la_letra_libre_es_la_primera_que_nadie_tiene() {
        assert_eq!(letra_libre(&[]), Some('a'));
        assert_eq!(letra_libre(&['a', 'b', 'd']), Some('c'));
        let todas: Vec<char> = ('a'..='z').collect();
        assert_eq!(letra_libre(&todas), None);
    }

    #[test]
    fn al_juntar_gana_lo_que_dice_de_si_mismo_quien_habla() {
        let mio = Aparato {
            id: "t".into(),
            nombre: "Tableta vieja".into(),
            ..Default::default()
        };
        let suyo = m::Aparato {
            id: "t".into(),
            nombre: "Tableta".into(),
            ..Default::default()
        };
        let otro = m::Aparato {
            id: "p".into(),
            nombre: "Portatil".into(),
            ..Default::default()
        };
        let sin = juntar(
            std::slice::from_ref(&mio),
            &[suyo.clone(), otro.clone()],
            None,
        );
        assert_eq!(sin.len(), 2);
        assert_eq!(
            sin[0].nombre, "Tableta vieja",
            "sin quien hable, lo mio se queda"
        );
        let con = juntar(&[mio], &[otro], Some(&suyo));
        assert_eq!(
            con.iter().find(|a| a.id == "t").map(|a| a.nombre.as_str()),
            Some("Tableta")
        );
    }

    #[test]
    fn un_codigo_nuevo_tiene_diez_signos_del_alfabeto_del_grupo() {
        let c = codigo_nuevo().expect("el sistema da azar");
        assert!(codigo_valido(&c), "{c}");
    }
}
