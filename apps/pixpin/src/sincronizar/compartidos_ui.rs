//! **Las personas de un chat compartido, y entrar en uno** (Android
//! 13-oct-2026, `sincro/ChatCompartidoActivity.kt`), dentro de Sincronizar.
//!
//! Con un chat (desde el clic derecho de un proyecto, «Compartir con
//! personas»): si aun no esta compartido, compartirlo; si lo esta, quien hay,
//! **Anadir** (un codigo grande y su QR, con una puerta propia abierta
//! mientras esta pantalla este delante), **Sincronizar** con quien este en la
//! Wi-Fi, **Quitar** a alguien (solo quien lo creo) y **Salir**. Sin chat
//! («Entrar en un chat compartido»): teclear el codigo que ensena quien
//! invita, o pegar el texto de su QR —el PC no tiene camara—, y traerse el
//! chat.
//!
//! Lo de dentro —las firmas, la valla de archivos, el protocolo— esta en
//! `pixpin_sincro::compartidos` e `invitacion`, sin ventanas y probado alli.

use std::collections::HashSet;
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use pixpin_proyecto::vista::DiscoPc;

use crate::caja_texto::CajaLinea;
use pixpin_render::icono::material;
use pixpin_sincro::compartidos::{self, ChatCompartido, CompartidosEnDisco, Persona};
use pixpin_sincro::invitacion::{self, AvisoDePuerta, Puerta};
use pixpin_sincro::protocolo::ErrorSincro;
use pixpin_store::Catalogo;

use super::recibir_wifi::{boton_apagado, hex_blanco};
use super::{
    Accion, Aviso, BORDE, CONTENEDOR, Contexto, ERROR, FONDO, Lienzo, PRIMARIO, SOBRE_CONTENEDOR,
    SUAVE, TEXTO, Tecla, VARIANTE, caja, menos, rect,
};

/// Un texto del catalogo con sus argumentos: los hilos no tienen el
/// catalogo, asi que dicen que clave y la ventana la traduce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Dicho {
    Clave(&'static str, Vec<(&'static str, String)>),
    /// Lo que cuenta la sincronizacion, tal cual (viene en castellano del
    /// protocolo, como en el movil).
    Libre(String),
}

impl Dicho {
    fn de(clave: &'static str) -> Dicho {
        Dicho::Clave(clave, Vec::new())
    }

    fn con(clave: &'static str, k: &'static str, v: impl Into<String>) -> Dicho {
        Dicho::Clave(clave, vec![(k, v.into())])
    }

    fn texto(&self, textos: &Catalogo) -> String {
        match self {
            Dicho::Libre(t) => t.clone(),
            Dicho::Clave(c, args) if args.is_empty() => textos.t(c),
            Dicho::Clave(c, args) => {
                let mut a = fluent_bundle::FluentArgs::new();
                for (k, v) in args {
                    a.set(*k, v.clone());
                }
                textos.t_args(c, &a)
            }
        }
    }
}

/// Lo que cuentan los hilos y la puerta.
#[derive(Debug, Clone)]
pub(super) enum Novedad {
    /// Una linea de lo que pasa; con `fin`, el trabajo termino.
    Aviso(Dicho, bool),
    /// Se entro con el codigo y ya se trajo el chat.
    Dentro(ChatCompartido),
    /// La puerta se cerro sola (demasiados codigos malos).
    PuertaCerrada,
}

/// Lo que se pulsa en esta pantalla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Toque {
    CampoNombre,
    GuardarNombre,
    Compartir,
    Invitar,
    CerrarInvitacion,
    Sincronizar,
    /// Pide confirmar quitar a esa persona (o salir, si es uno mismo).
    Quitar(String),
    ConfirmarQuitar,
    CancelarQuitar,
    CampoCodigo,
    Entrar,
    /// Ya dentro: llevar el chat de PixPin al chat recien traido.
    AbrirChat,
    Cerrar,
}

/// La invitacion abierta: su puerta, su anuncio en la red y su QR.
struct Invitando {
    puerta: Puerta,
    _anuncio: Option<pixpin_shell::mdns::Anuncio>,
    qr: Option<qrcodegen::QrCode>,
}

pub(super) struct Personas {
    pub(super) turno: u64,
    /// El chat (id y nombre) o nada, si es para entrar en uno.
    chat: Option<(String, String)>,
    lista: Option<ChatCompartido>,
    yo: String,
    mis_aparatos: HashSet<String>,
    /// Mi nombre en los chats compartidos, como esta guardado y como se
    /// esta escribiendo.
    nombre: String,
    nombre_escrito: CajaLinea,
    en_nombre: bool,
    tecleado: CajaLinea,
    invitando: Option<Invitando>,
    aviso: Option<Dicho>,
    trabajando: Arc<AtomicBool>,
    quitando: Option<String>,
    dentro_de: Option<ChatCompartido>,
}

impl Personas {
    /// Las personas del chat `chat`.
    pub(super) fn de_chat(turno: u64, cx: &Contexto, chat: String, nombre: String) -> Personas {
        let mut p = Personas::vacia(turno, Some((chat, nombre)));
        p.releer(cx);
        p
    }

    /// Entrar en un chat que otro comparte.
    pub(super) fn entrar(turno: u64, cx: &Contexto) -> Personas {
        let mut p = Personas::vacia(turno, None);
        p.releer(cx);
        p
    }

    fn vacia(turno: u64, chat: Option<(String, String)>) -> Personas {
        Personas {
            turno,
            chat,
            lista: None,
            yo: String::new(),
            mis_aparatos: HashSet::new(),
            nombre: String::new(),
            nombre_escrito: CajaLinea::nueva()
                .con_maximo(compartidos::NOMBRE_MAX)
                .sin_parpadeo(),
            en_nombre: false,
            tecleado: CajaLinea::nueva().con_maximo(80).sin_parpadeo(),
            invitando: None,
            aviso: None,
            trabajando: Arc::new(AtomicBool::new(false)),
            quitando: None,
            dentro_de: None,
        }
    }

    /// Vuelve a leer la lista y quien soy: tras cada cambio de la lista.
    pub(super) fn releer(&mut self, cx: &Contexto) {
        let cd = CompartidosEnDisco::nuevo(&cx.raiz);
        self.yo = cx.id.clone();
        if let Ok(id) = super::leer_identidad(&cx.raiz) {
            self.mis_aparatos = id.miembros.iter().map(|m| m.id.clone()).collect();
            self.mis_aparatos.insert(id.yo.id.clone());
        }
        let nombre = cd.nombre().unwrap_or_else(|| cx.nombre.clone());
        if !self.en_nombre || self.nombre_escrito.texto() == self.nombre {
            self.nombre_escrito.poner_texto(&nombre);
        }
        self.nombre = nombre;
        self.lista = self.chat.as_ref().and_then(|(c, _)| cd.de(c));
    }

    pub(super) fn titulo(&self, textos: &Catalogo) -> String {
        textos.t(if self.chat.is_some() {
            "cc-titulo-personas"
        } else {
            "cc-titulo-entrar"
        })
    }

    pub(super) fn animando(&self) -> bool {
        self.trabajando.load(Ordering::SeqCst)
    }

    fn ocupada(&self) -> bool {
        self.trabajando.load(Ordering::SeqCst)
    }
}

/// Lo que llega de los hilos y de la puerta.
pub(super) fn novedad(p: &mut Personas, n: Novedad, cx: &Contexto) {
    match n {
        Novedad::Aviso(d, fin) => {
            p.aviso = Some(d);
            if fin {
                p.trabajando.store(false, Ordering::SeqCst);
            }
        }
        Novedad::Dentro(c) => {
            p.trabajando.store(false, Ordering::SeqCst);
            p.aviso = None;
            p.dentro_de = Some(c);
        }
        Novedad::PuertaCerrada => {
            p.invitando = None;
            p.aviso = Some(Dicho::de("cc-demasiados"));
        }
    }
    p.releer(cx);
}

/// La caja donde se escribe ahora (el codigo o mi nombre), si se ve: la del
/// raton. La otra no se toca hasta que se pulsa (su zona la activa).
pub(super) fn caja_visible(p: &mut Personas) -> Option<&mut CajaLinea> {
    let en_codigo = p.chat.is_none() && !p.en_nombre && p.dentro_de.is_none();
    if en_codigo {
        Some(&mut p.tecleado)
    } else if p.en_nombre {
        Some(&mut p.nombre_escrito)
    } else {
        None
    }
}

/// Lo que se escribe. Devuelve si cambio algo. Los dos campos (el codigo y
/// mi nombre) son la caja de una linea comun de la app: su rayita donde se
/// escribe, Ctrl+Retroceso, Mayus+flechas, Ctrl+A/C/X/V/Z...
pub(super) fn tecla(p: &mut Personas, t: Tecla, cx: &Contexto) -> bool {
    let en_codigo = p.chat.is_none() && !p.en_nombre && p.dentro_de.is_none();
    if !p.en_nombre && !en_codigo {
        return false;
    }
    let campo = if en_codigo {
        &mut p.tecleado
    } else {
        &mut p.nombre_escrito
    };
    match t {
        Tecla::Letra(c) => {
            campo.caracter(c);
        }
        Tecla::Borrar => {
            campo.tecla(
                0x08,
                false,
                false,
                &mut crate::caja_texto::PortapapelesWindows,
            );
        }
        Tecla::Caja { vk, shift, ctrl } => {
            return campo
                .tecla(vk, shift, ctrl, &mut crate::caja_texto::PortapapelesWindows)
                .atendida();
        }
        // El texto de un QR pegado: donde esta la rayita (sobre lo elegido),
        // sin blancos a los lados.
        Tecla::Pegar(texto) => {
            campo.pegar_texto(texto.trim());
        }
        Tecla::Intro => {
            if en_codigo {
                tocar(p, Toque::Entrar, cx);
            } else {
                tocar(p, Toque::GuardarNombre, cx);
            }
        }
    }
    true
}

/// Un toque. Devuelve `true` si hay que volver a la portada.
pub(super) fn tocar(p: &mut Personas, t: Toque, cx: &Contexto) -> bool {
    match t {
        Toque::CampoNombre => p.en_nombre = true,
        Toque::CampoCodigo => p.en_nombre = false,
        Toque::GuardarNombre => {
            let nuevo = p.nombre_escrito.trim().to_string();
            if !nuevo.is_empty() {
                let _ = CompartidosEnDisco::nuevo(&cx.raiz).poner_nombre(&nuevo);
            }
            p.en_nombre = false;
            p.releer(cx);
        }
        Toque::Compartir => {
            if let Some((chat, nombre)) = p.chat.clone() {
                let cd = CompartidosEnDisco::nuevo(&cx.raiz);
                let hecho = cd.yo(&cx.id, &cx.nombre).and_then(|yo| {
                    cd.cambiar(|l| -> std::io::Result<_> {
                        let mut l = l;
                        if !l.iter().any(|c| c.chat == chat) {
                            l.push(compartidos::crear(&chat, &nombre, &yo, super::ahora_ms()));
                        }
                        Ok(l)
                    })
                });
                p.aviso = Some(match hecho {
                    Ok(_) => Dicho::de("cc-compartido"),
                    Err(e) => Dicho::con("cc-se-corto", "motivo", e.to_string()),
                });
                p.releer(cx);
            }
        }
        Toque::Invitar => {
            if p.invitando.is_none()
                && let Some((chat, _)) = p.chat.clone()
            {
                match abrir_invitacion(cx, p.turno, &chat, &p.nombre) {
                    Ok(i) => p.invitando = Some(i),
                    Err(e) => p.aviso = Some(Dicho::con("cc-se-corto", "motivo", e.to_string())),
                }
            }
        }
        Toque::CerrarInvitacion => p.invitando = None,
        Toque::Sincronizar => {
            if let Some((chat, _)) = p.chat.clone()
                && !p.trabajando.swap(true, Ordering::SeqCst)
            {
                p.aviso = Some(Dicho::de("cc-buscando"));
                sincronizar_en_fondo(cx, p.turno, chat, p.mis_aparatos.clone());
            }
        }
        Toque::Quitar(id) => p.quitando = Some(id),
        Toque::CancelarQuitar => p.quitando = None,
        Toque::ConfirmarQuitar => {
            if let (Some(id), Some((chat, _))) = (p.quitando.take(), p.chat.clone()) {
                let cd = CompartidosEnDisco::nuevo(&cx.raiz);
                let hecho = cd
                    .yo(&cx.id, &cx.nombre)
                    .map_err(|e| e.to_string())
                    .and_then(|yo| {
                        cd.cambiar(|l| -> Result<_, QuitarFallo> {
                            l.into_iter()
                                .map(|c| {
                                    if c.chat == chat {
                                        compartidos::quitar(&c, &yo, &id, super::ahora_ms())
                                            .map_err(QuitarFallo::Dicho)
                                    } else {
                                        Ok(c)
                                    }
                                })
                                .collect()
                        })
                        .map_err(|e| e.to_string())
                    });
                match hecho {
                    Ok(_) => {
                        if id == cx.id {
                            p.invitando = None;
                        }
                        p.aviso = Some(Dicho::de(if id == cx.id {
                            "cc-has-salido"
                        } else {
                            "cc-quitado-aviso"
                        }));
                        // Que se enteren ya los que esten cerca.
                        if !p.trabajando.swap(true, Ordering::SeqCst) {
                            sincronizar_en_fondo(cx, p.turno, chat, p.mis_aparatos.clone());
                        }
                    }
                    Err(e) => p.aviso = Some(Dicho::Libre(e)),
                }
                p.releer(cx);
            }
        }
        Toque::Entrar => {
            let texto = p.tecleado.trim().to_string();
            if let Some((codigo, donde)) = que_entrar(&texto)
                && !p.trabajando.swap(true, Ordering::SeqCst)
            {
                p.aviso = Some(Dicho::de("cc-buscando-invitacion"));
                entrar_en_fondo(cx, p.turno, codigo, donde);
            }
        }
        Toque::AbrirChat => {
            if let Some(c) = &p.dentro_de {
                if super::abrir_chat(&cx.raiz, &c.chat) {
                    return true;
                }
                // Aqui no esta (no llego el proyecto): se dice, no se
                // cierra.
                p.aviso = Some(Dicho::de("cc-no-se-abre"));
            }
        }
        Toque::Cerrar => return true,
    }
    false
}

/// El fallo de quitar: lo que dice `compartidos::quitar`, o el disco.
#[derive(Debug)]
enum QuitarFallo {
    Dicho(String),
    Io(std::io::Error),
}

impl From<std::io::Error> for QuitarFallo {
    fn from(e: std::io::Error) -> Self {
        QuitarFallo::Io(e)
    }
}

impl std::fmt::Display for QuitarFallo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuitarFallo::Dicho(t) => f.write_str(t),
            QuitarFallo::Io(e) => write!(f, "{e}"),
        }
    }
}

/// Del texto del campo: el codigo y, si es el de un QR, donde esta quien
/// invita. Nada si no es ni una cosa ni otra.
pub(super) fn que_entrar(texto: &str) -> Option<(String, Option<(String, u16)>)> {
    if let Some(q) = invitacion::leer_qr(texto) {
        let donde = q.host.filter(|_| q.puerto > 0).map(|h| (h, q.puerto));
        return Some((q.codigo, donde));
    }
    // Otro QR de PixPin (o cualquier texto con dos puntos) no es un codigo:
    // limpiandolo saldrian ocho signos que parecen uno.
    if texto.contains(':') {
        return None;
    }
    let c = invitacion::limpiar(texto);
    invitacion::valido(&c).then_some((c, None))
}

// ------------------------------------------------------------------ la red

/// El almacen de este equipo visto como el del movil, avisando a las
/// ventanas abiertas de lo que llega (como la escucha de siempre).
fn disco(raiz: &Path) -> DiscoPc {
    DiscoPc::nuevo(raiz).con_avisos(|c| {
        if c == pixpin_sincro::disco::Cambio::Identidad {
            super::presencia::difundir(super::presencia::Novedad::Identidad);
        } else {
            crate::ventana_chat::refrescar();
            crate::tareas::refrescar();
        }
    })
}

fn avisar(cx: &Contexto, turno: u64, n: Novedad) {
    let _ = cx.tx.send(Aviso::Compartidos(turno, n));
}

/// **Abre la puerta para invitar** (`abrirInvitacion`): un puerto propio
/// mientras esta pantalla esta delante, anunciado con la etiqueta del codigo,
/// y el QR con la direccion.
fn abrir_invitacion(
    cx: &Contexto,
    turno: u64,
    chat: &str,
    nombre: &str,
) -> std::io::Result<Invitando> {
    let cx2 = cx.clone();
    let puerta = Puerta::abrir(
        Arc::new(disco(&cx.raiz)),
        chat,
        super::presencia::puerto().map_or(0, u32::from),
        super::ahora_ms,
        move |a| {
            let n = match a {
                AvisoDePuerta::Entro(p) => {
                    Novedad::Aviso(Dicho::con("cc-entro", "nombre", p.nombre), false)
                }
                AvisoDePuerta::Junto(_) => Novedad::Aviso(Dicho::de("cc-listo"), false),
                AvisoDePuerta::Fallo(m) => {
                    Novedad::Aviso(Dicho::con("cc-se-corto", "motivo", m), false)
                }
                AvisoDePuerta::Cerrada => Novedad::PuertaCerrada,
                AvisoDePuerta::Estado(t) => Novedad::Aviso(Dicho::Libre(t), false),
            };
            avisar(&cx2, turno, n);
        },
    )?;
    let etiqueta = invitacion::etiqueta(&puerta.codigo);
    let anuncio = pixpin_shell::mdns::anunciar(
        invitacion::TIPO,
        &format!("PixPin {nombre}"),
        puerta.puerto,
        &[("g", etiqueta.as_str())],
    )
    .map_err(|e| tracing::warn!(?e, "no se pudo anunciar la invitacion"))
    .ok();
    let ip = super::ip_local();
    let qr = crate::recibir::qr_de(&invitacion::texto_del_qr(
        &puerta.codigo,
        ip.as_deref(),
        puerta.puerto,
    ));
    Ok(Invitando {
        puerta,
        _anuncio: anuncio,
        qr,
    })
}

fn nonce() -> [u8; 32] {
    super::nonce().unwrap_or_else(|| {
        // Sin azar del sistema no se habla: un nonce fijo repetiria claves.
        let mut n = [0u8; 32];
        let t = super::ahora_ms().to_le_bytes();
        n[..8].copy_from_slice(&t);
        n
    })
}

fn conectar(host: &str, puerto: u16) -> std::io::Result<TcpStream> {
    super::recibir_wifi::conectar(host, puerto)
}

/// **Sincroniza este chat con cada persona que este en la Wi-Fi**
/// (`sincronizarAhora`): por la ultima direccion donde se la vio, o
/// buscandola en la red por su id. Mis propios aparatos no: esos van por el
/// grupo. Tambien con quien ya no esta: asi se entera de que salio o de que
/// lo quitaron.
fn sincronizar_en_fondo(cx: &Contexto, turno: u64, chat: String, mios: HashSet<String>) {
    let cx = cx.clone();
    let lanzado = std::thread::Builder::new()
        .name("chat-compartido-sincro".into())
        .spawn(move || {
            let fin = sincronizar_ahora(&cx, turno, &chat, &mios);
            avisar(&cx, turno, Novedad::Aviso(fin, true));
        });
    if lanzado.is_err() {
        tracing::warn!("no se pudo lanzar la sincronizacion del chat compartido");
    }
}

fn sincronizar_ahora(cx: &Contexto, turno: u64, chat: &str, mios: &HashSet<String>) -> Dicho {
    let Some(l) = CompartidosEnDisco::nuevo(&cx.raiz).de(chat) else {
        return Dicho::de("cc-nadie");
    };
    let otros: Vec<Persona> = l
        .personas
        .iter()
        .filter(|p| p.id != cx.id && !mios.contains(&p.id))
        .cloned()
        .collect();
    if otros.is_empty() {
        return Dicho::de("cc-nadie");
    }
    let mut dirs: Vec<(String, String, u16)> = super::leer_direcciones(&cx.raiz)
        .into_iter()
        .filter(|(id, h, pu)| otros.iter().any(|p| p.id == *id) && super::sondear_uno(h, *pu))
        .collect();
    let faltan: Vec<&Persona> = otros
        .iter()
        .filter(|p| !dirs.iter().any(|(id, _, _)| *id == p.id))
        .collect();
    if !faltan.is_empty() {
        let empezo = Instant::now();
        while empezo.elapsed() < Duration::from_secs(8) {
            let Ok(vistos) = pixpin_shell::mdns::buscar(super::TIPO_MDNS, Duration::from_secs(3))
            else {
                break;
            };
            for v in vistos {
                if let Some(id) = v.datos.get("id")
                    && faltan.iter().any(|p| p.id == *id)
                    && !dirs.iter().any(|(x, _, _)| x == id)
                {
                    dirs.push((id.clone(), v.host.to_string(), v.puerto));
                }
            }
            if faltan
                .iter()
                .all(|p| dirs.iter().any(|(x, _, _)| *x == p.id))
            {
                break;
            }
        }
    }
    let d = disco(&cx.raiz);
    let mi_puerto = super::presencia::puerto().map_or(0, u32::from);
    let mut hechos: Vec<String> = Vec::new();
    for p in &otros {
        let Some((_, host, puerto)) = dirs.iter().find(|(id, _, _)| *id == p.id).cloned() else {
            continue;
        };
        avisar(
            cx,
            turno,
            Novedad::Aviso(
                Dicho::con("cc-sincronizando-con", "nombre", p.nombre.clone()),
                false,
            ),
        );
        let Ok(flujo) = conectar(&host, puerto) else {
            continue;
        };
        let r = invitacion::vuelta_con_persona(
            flujo,
            &d,
            Some(&p.id),
            None,
            &super::ahora_ms,
            mi_puerto,
            nonce(),
            &mut |t| {
                avisar(
                    cx,
                    turno,
                    Novedad::Aviso(Dicho::Libre(t.to_string()), false),
                )
            },
        );
        match r {
            Ok(r) => {
                if r.puerto > 0 {
                    super::apuntar_direccion(&cx.raiz, &p.id, &host, r.puerto as u16);
                }
                hechos.push(p.nombre.clone());
            }
            Err(ErrorSincro::Remoto(m)) if m == pixpin_sincro::mensajes::OCUPADO => {
                hechos.push(format!("{} (…)", p.nombre));
            }
            Err(e) => tracing::info!(?e, "no se pudo sincronizar con una persona"),
        }
    }
    hechos.dedup();
    if hechos.is_empty() {
        Dicho::de("cc-nadie-en-wifi")
    } else {
        Dicho::con("cc-sincronizado", "nombres", hechos.join(", "))
    }
}

/// **Entrar con un codigo** (`entrarCon`): con la direccion del QR, o
/// buscando a quien invita en la red.
fn entrar_en_fondo(cx: &Contexto, turno: u64, codigo: String, donde: Option<(String, u16)>) {
    let cx = cx.clone();
    let lanzado = std::thread::Builder::new()
        .name("chat-compartido-entrar".into())
        .spawn(move || {
            let n = match entrar(&cx, turno, &codigo, donde) {
                Ok(c) => Novedad::Dentro(c),
                Err(d) => Novedad::Aviso(d, true),
            };
            avisar(&cx, turno, n);
        });
    if lanzado.is_err() {
        tracing::warn!("no se pudo lanzar la entrada al chat compartido");
    }
}

fn buscar_invitacion(codigo: &str) -> Option<(String, u16)> {
    let etiqueta = invitacion::etiqueta(codigo);
    let empezo = Instant::now();
    while empezo.elapsed() < Duration::from_secs(20) {
        match pixpin_shell::mdns::buscar(invitacion::TIPO, Duration::from_secs(3)) {
            Ok(vistos) => {
                if let Some(v) = vistos
                    .into_iter()
                    .find(|v| v.datos.get("g") == Some(&etiqueta))
                {
                    return Some((v.host.to_string(), v.puerto));
                }
            }
            Err(_) => std::thread::sleep(Duration::from_millis(500)),
        }
    }
    None
}

fn entrar(
    cx: &Contexto,
    turno: u64,
    codigo: &str,
    donde: Option<(String, u16)>,
) -> Result<ChatCompartido, Dicho> {
    let (host, puerto) = donde
        .or_else(|| buscar_invitacion(codigo))
        .ok_or_else(|| Dicho::de("cc-no-encontrada"))?;
    avisar(cx, turno, Novedad::Aviso(Dicho::de("cc-entrando"), false));
    let corto = |e: &dyn std::fmt::Display| Dicho::con("cc-se-corto", "motivo", e.to_string());
    let d = disco(&cx.raiz);
    let flujo = conectar(&host, puerto).map_err(|e| corto(&e))?;
    let e =
        invitacion::entrar(flujo, &d, codigo, super::ahora_ms(), nonce()).map_err(|e| corto(&e))?;
    if e.puerto > 0 {
        super::apuntar_direccion(&cx.raiz, &e.quien.id, &host, e.puerto as u16);
    }
    avisar(
        cx,
        turno,
        Novedad::Aviso(
            Dicho::con("cc-trayendo", "nombre", e.chat.nombre.clone()),
            false,
        ),
    );
    let flujo = conectar(&host, puerto).map_err(|e| corto(&e))?;
    invitacion::vuelta_con_persona(
        flujo,
        &d,
        Some(&e.quien.id),
        None,
        &super::ahora_ms,
        super::presencia::puerto().map_or(0, u32::from),
        nonce(),
        &mut |t| {
            avisar(
                cx,
                turno,
                Novedad::Aviso(Dicho::Libre(t.to_string()), false),
            )
        },
    )
    .map_err(|e| corto(&e))?;
    Ok(e.chat)
}

// ------------------------------------------------------------------ autores

/// **Quien escribio cada mensaje** en un chat compartido: del codigo del
/// aparato (`aparato` del mensaje, `K7Q2`) al nombre de su persona, de todos
/// los chats compartidos donde estoy. Lo mio no sale: es lo de siempre.
/// Se recuerda hasta que la lista cambia (`compartidos::version`).
pub(crate) fn autor(raiz: &Path, aparato: &str) -> Option<String> {
    type Cache = Option<(
        std::path::PathBuf,
        u64,
        std::collections::HashMap<String, String>,
    )>;
    static CACHE: std::sync::Mutex<Cache> = std::sync::Mutex::new(None);
    let v = compartidos::version();
    let mut g = CACHE.lock().ok()?;
    let vale = g.as_ref().is_some_and(|(r, x, _)| r == raiz && *x == v);
    if !vale {
        let (yo, mios) = mis_aparatos(raiz);
        let mut mapa = std::collections::HashMap::new();
        for c in CompartidosEnDisco::nuevo(raiz).leer() {
            if c.persona(&yo).is_none() {
                continue;
            }
            mapa.extend(compartidos::autores(&c));
        }
        // Lo de cualquiera de mis aparatos (mi movil tambien) no lleva
        // nombre: es lo de siempre.
        for m in &mios {
            mapa.remove(&pixpin_sincro::grupo::codigo_de_aparato(m));
        }
        *g = Some((raiz.to_path_buf(), v, mapa));
    }
    g.as_ref().and_then(|(_, _, m)| m.get(aparato).cloned())
}

/// El id de este aparato y los de todos los mios: este y los del grupo de
/// sincronizar (mi movil, mi tableta).
fn mis_aparatos(raiz: &Path) -> (String, HashSet<String>) {
    match super::leer_identidad(raiz) {
        Ok(i) => {
            let mut mios: HashSet<String> = i.miembros.iter().map(|m| m.id.clone()).collect();
            mios.insert(i.yo.id.clone());
            (i.yo.id, mios)
        }
        Err(_) => (String::new(), HashSet::new()),
    }
}

// ------------------------------------------------------------------ en el chat

/// **Un chat compartido visto desde la ventana del chat** (25-oct-2026): la
/// chapa de la lista y de la cabecera, y de quien es cada burbuja.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EnElChat {
    /// El id del chat tal como viaja (el de `compartidos.json`).
    pub chat: String,
    /// Las personas que estan dentro, yo incluido, cada una una vez.
    pub personas: usize,
    /// Quien es quien, por el codigo de aparato de cada mensaje.
    pub lados: pixpin_ui::conversacion::Lados,
}

/// **De quien es cada cosa** en un chat compartido, sin disco: `yo` es este
/// aparato y `mios` todos los mios. Una persona de la lista cuyo aparato es
/// mio (mi movil, que entro solo al sincronizar el grupo) es mia: si no, lo
/// que escribo desde el movil saldria a la izquierda con mi propio nombre.
pub(crate) fn en_el_chat(c: &ChatCompartido, mios: &HashSet<String>) -> EnElChat {
    let codigos_mios = mios
        .iter()
        .map(|id| pixpin_sincro::grupo::codigo_de_aparato(id))
        .chain(
            c.personas
                .iter()
                .filter(|p| mios.contains(&p.id))
                .map(|p| p.codigo.clone()),
        );
    // Por orden de entrada, como `compartidos::autores`: si un codigo
    // saliera dos veces, gana el nombre mas reciente.
    let mut personas: Vec<&Persona> = c.personas.iter().collect();
    personas.sort_by_key(|p| p.desde);
    EnElChat {
        chat: c.chat.clone(),
        personas: compartidos::nombres_dentro(c).len(),
        lados: pixpin_ui::conversacion::Lados::nuevo(
            codigos_mios,
            personas
                .into_iter()
                .map(|p| (p.codigo.clone(), p.nombre.clone())),
        ),
    }
}

/// Cada cuanto se vuelve a mirar aunque la lista no cambie, si algun chat
/// compartido aun no tenia proyecto aqui: el de un chat en el que se acaba
/// de entrar llega DESPUES que su lista, y hasta entonces no hay ficha a la
/// que ponerle la chapa.
const RELEER_CADA: Duration = Duration::from_secs(3);

type PorFicha = std::collections::HashMap<String, EnElChat>;

struct Recuerdo {
    raiz: std::path::PathBuf,
    version: u64,
    cuando: Instant,
    /// Algun chat compartido donde estoy no tenia aun su proyecto.
    faltan: bool,
    generacion: u64,
    por_ficha: Arc<PorFicha>,
}

static RECUERDO: std::sync::Mutex<Option<Recuerdo>> = std::sync::Mutex::new(None);

/// Los chats compartidos en los que estoy dentro, por la ficha de su
/// proyecto, y una generacion que solo sube cuando cambia algo (la ventana
/// del chat la mira para volver a colocar las burbujas, no cada vez).
fn por_ficha(raiz: &Path) -> (u64, Arc<PorFicha>) {
    let v = compartidos::version();
    let Ok(mut g) = RECUERDO.lock() else {
        return (0, Arc::default());
    };
    if let Some(r) = g.as_ref()
        && r.raiz == raiz
        && r.version == v
        && (!r.faltan || r.cuando.elapsed() < RELEER_CADA)
    {
        return (r.generacion, r.por_ficha.clone());
    }
    // Sin carpeta (las muestras y pruebas de la ventana) o sin ningun chat
    // compartido, nada: ni se lee la identidad, que leerla la crea.
    let lista = if raiz.as_os_str().is_empty() {
        Vec::new()
    } else {
        CompartidosEnDisco::nuevo(raiz).leer()
    };
    let (yo, mios) = if lista.is_empty() {
        (String::new(), HashSet::new())
    } else {
        mis_aparatos(raiz)
    };
    let mut nuevo = PorFicha::new();
    let mut faltan = false;
    if lista.iter().any(|c| compartidos::dentro(c, &yo)) {
        let mapa = DiscoPc::nuevo(raiz).mapa();
        for c in lista.iter().filter(|c| compartidos::dentro(c, &yo)) {
            match mapa.iter().find(|(chat, _)| *chat == c.chat) {
                Some((_, f)) => {
                    nuevo.insert(f.id.clone(), en_el_chat(c, &mios));
                }
                None => faltan = true,
            }
        }
    }
    let generacion = match g.as_ref() {
        Some(r) if r.raiz == raiz && *r.por_ficha == nuevo => r.generacion,
        Some(r) => r.generacion + 1,
        None => 1,
    };
    let por_ficha = Arc::new(nuevo);
    *g = Some(Recuerdo {
        raiz: raiz.to_path_buf(),
        version: v,
        cuando: Instant::now(),
        faltan,
        generacion,
        por_ficha: por_ficha.clone(),
    });
    (generacion, por_ficha)
}

/// **El chat compartido de esta ficha**, si lo es y estoy dentro. `None` es
/// un chat normal: se ve como siempre.
pub(crate) fn del_chat(raiz: &Path, ficha: &str) -> Option<EnElChat> {
    por_ficha(raiz).1.get(ficha).cloned()
}

/// Cuantas personas hay en el chat compartido de esta ficha (para la chapa
/// de la lista), o `None` si es un chat normal.
pub(crate) fn personas_del_chat(raiz: &Path, ficha: &str) -> Option<usize> {
    por_ficha(raiz).1.get(ficha).map(|c| c.personas)
}

/// Sube cuando cambia algo de los chats compartidos que se ven.
pub(crate) fn generacion(raiz: &Path) -> u64 {
    por_ficha(raiz).0
}

/// **Nace un chat compartido** (el «+» de la lista, «Chat compartido»): el
/// chat `chat` con una sola persona, yo, que lo creo. A los demas se les
/// invita despues desde sus personas, como siempre.
pub(crate) fn compartir_chat_nuevo(raiz: &Path, chat: &str, nombre: &str) -> std::io::Result<()> {
    let id = super::leer_identidad(raiz)?;
    let cd = CompartidosEnDisco::nuevo(raiz);
    let yo = cd.yo(&id.yo.id, &id.yo.nombre)?;
    cd.cambiar(|mut l| -> std::io::Result<_> {
        if !l.iter().any(|c| c.chat == chat) {
            l.push(compartidos::crear(chat, nombre, &yo, super::ahora_ms()));
        }
        Ok(l)
    })?;
    Ok(())
}

/// El nombre del chat compartido, si nacio sin el (se le pone al proyecto
/// justo despues de crearlo). No va firmado: es solo el que ve quien entra
/// antes de que le llegue el proyecto.
pub(crate) fn nombrar_si_falta(raiz: &Path, chat: &str, nombre: &str) -> std::io::Result<()> {
    let nombre: String = nombre.trim().chars().take(80).collect();
    if nombre.is_empty() {
        return Ok(());
    }
    CompartidosEnDisco::nuevo(raiz).cambiar(|l| -> std::io::Result<_> {
        Ok(l.into_iter()
            .map(|mut c| {
                if c.chat == chat && c.nombre.trim().is_empty() {
                    c.nombre = nombre.clone();
                }
                c
            })
            .collect())
    })?;
    Ok(())
}

/// La carpeta de datos de esta aplicacion, para [autor_aqui]: la pone la
/// presencia al arrancar (`presencia::instalar`).
static RAIZ: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

pub(crate) fn fijar_raiz(raiz: &Path) {
    let _ = RAIZ.set(raiz.to_path_buf());
}

/// [autor] con la carpeta de esta aplicacion: lo que usa la chapa del
/// codigo de cada mensaje en el chat.
pub(crate) fn autor_aqui(aparato: &str) -> Option<String> {
    autor(RAIZ.get()?, aparato)
}
// ------------------------------------------------------------------ pintar

/// Un campo de una linea con su rotulo encima (`OutlinedTextField`).
fn campo_linea(
    l: &mut Lienzo<'_>,
    r: pixpin_render::RectF,
    caja: &CajaLinea,
    activo: bool,
    a: Accion,
) {
    let (p, e) = (l.p, l.e);
    p.rellenar_redondeado(r, 6.0 * e, if activo { PRIMARIO } else { super::CONTORNO });
    p.rellenar_redondeado(menos(r, if activo { 2.0 } else { 1.0 } * e), 5.0 * e, FONDO);
    pintar_caja(p, r, caja, "", 15.0 * e, activo, e);
    l.zonas.push((r, a));
}

/// Lo escrito en un campo: la caja de una linea comun de la app, con su
/// rayita donde se escribe (no siempre al final), lo elegido y el texto
/// corrido si no cabe.
pub(super) fn pintar_caja(
    p: &pixpin_render::Pintor,
    r: pixpin_render::RectF,
    caja: &CajaLinea,
    pista: &str,
    tam: f32,
    activo: bool,
    e: f32,
) {
    let hueco = pixpin_render::RectF {
        x: r.x + 14.0 * e,
        ancho: (r.ancho - 28.0 * e).max(1.0),
        ..r
    };
    caja.pintar(
        p,
        hueco,
        hueco,
        tam,
        pista,
        &crate::caja_texto::Colores {
            texto: TEXTO,
            invitacion: SUAVE,
            seleccion: pixpin_render::Color {
                a: 0.35,
                ..PRIMARIO
            },
            rayita: PRIMARIO,
        },
        activo,
        Instant::now(),
    );
}

/// Pinta la pantalla desde `y` y devuelve donde acaba.
pub(super) fn pintar(
    l: &mut Lienzo<'_>,
    textos: &Catalogo,
    s: &Personas,
    x: f32,
    w: f32,
    mut y: f32,
) -> f32 {
    let (p, e) = (l.p, l.e);
    let t = |a: Toque| Accion::Compartidos(a);
    y += 8.0 * e;
    if let Some((_, nombre)) = &s.chat
        && !nombre.trim().is_empty()
    {
        p.texto_linea(&format!("«{nombre}»"), x, y, 14.0 * e, w, SUAVE);
        y += 26.0 * e;
    }
    // Mi nombre, como me ven los demas.
    p.texto(&textos.t("cc-tu-nombre"), x, y, 12.0 * e, SUAVE);
    y += 20.0 * e;
    let cambia = s.nombre_escrito.trim() != s.nombre && !s.nombre_escrito.trim().is_empty();
    let wb = if cambia { 110.0 * e } else { 0.0 };
    campo_linea(
        l,
        rect(x, y, w - wb, 48.0 * e),
        &s.nombre_escrito,
        s.en_nombre,
        t(Toque::CampoNombre),
    );
    if cambia {
        l.boton(
            rect(x + w - wb + 10.0 * e, y + 4.0 * e, wb - 10.0 * e, 40.0 * e),
            &textos.t("cc-guardar"),
            None,
            true,
            t(Toque::GuardarNombre),
        );
    }
    y += 48.0 * e + 20.0 * e;

    y = if s.chat.is_some() {
        pintar_personas(l, textos, s, x, w, y)
    } else {
        pintar_entrar(l, textos, s, x, w, y)
    };

    if let Some(a) = &s.aviso {
        y += 16.0 * e;
        let texto = a.texto(textos);
        if s.ocupada() {
            l.girando(x + w / 2.0, y + 12.0 * e, 10.0 * e, 3.0 * e);
            y += 30.0 * e;
        }
        y += l.parrafo_centrado(&texto, x, y, w, 14.0 * e, TEXTO);
    }
    y
}

fn caja_texto(l: &Lienzo<'_>, x: f32, y: f32, w: f32, titulo: Option<&str>, texto: &str) -> f32 {
    let e = l.e;
    let wi = w - 32.0 * e;
    let ht = titulo.map_or(0.0, |t| l.alto_de(t, 15.0 * e, wi) + 6.0 * e);
    let h = 16.0 * e + ht + l.alto_de(texto, 14.0 * e, wi) + 16.0 * e;
    caja(l.p, rect(x, y, w, h), e);
    if let Some(t) = titulo {
        l.parrafo(t, x + 16.0 * e, y + 16.0 * e, wi, 15.0 * e, TEXTO);
    }
    l.parrafo(texto, x + 16.0 * e, y + 16.0 * e + ht, wi, 14.0 * e, TEXTO);
    h
}

fn pintar_personas(
    l: &mut Lienzo<'_>,
    textos: &Catalogo,
    s: &Personas,
    x: f32,
    w: f32,
    mut y: f32,
) -> f32 {
    let (p, e) = (l.p, l.e);
    let t = |a: Toque| Accion::Compartidos(a);
    let c = match &s.lista {
        Some(c) if c.persona(&s.yo).is_some() => c,
        _ => {
            y += caja_texto(
                l,
                x,
                y,
                w,
                Some(&textos.t("cc-compartir-explica")),
                &textos.t("cc-compartir-detalle"),
            );
            y += 16.0 * e;
            l.boton(
                rect(x, y, w, 48.0 * e),
                &textos.t("cc-compartir"),
                Some(&material::PERSON),
                true,
                t(Toque::Compartir),
            );
            return y + 48.0 * e;
        }
    };
    let estoy = compartidos::dentro(c, &s.yo);
    if !estoy {
        y += caja_texto(l, x, y, w, None, &textos.t("cc-ya-no-estas"));
        y += 16.0 * e;
    }
    // Una fila por aparato: cada uno firma sus mensajes con su codigo.
    let mut filas: Vec<&Persona> = c.personas.iter().collect();
    filas.sort_by_key(|p| (!p.dentro(), p.desde));
    for per in filas {
        let alto = 56.0 * e;
        let lado = 36.0 * e;
        let rc = rect(x, y + (alto - lado) / 2.0, lado, lado);
        p.rellenar_redondeado(
            rc,
            lado / 2.0,
            if per.dentro() { CONTENEDOR } else { VARIANTE },
        );
        let inicial: String = per
            .nombre
            .chars()
            .take(1)
            .flat_map(char::to_uppercase)
            .collect();
        let (wi, hi) = p.medir_texto(&inicial, 15.0 * e);
        p.texto(
            &inicial,
            rc.x + (lado - wi) / 2.0,
            rc.y + (lado - hi) / 2.0,
            15.0 * e,
            SOBRE_CONTENEDOR,
        );
        let quitar = per.id != s.yo && compartidos::puede_quitar(c, &s.yo, &per.id);
        let wq = if quitar { 90.0 * e } else { 0.0 };
        let xt = x + lado + 12.0 * e;
        let wt = w - lado - 12.0 * e - wq;
        let tu = if per.id == s.yo {
            format!(" {}", textos.t("cc-tu"))
        } else {
            String::new()
        };
        p.texto_linea(
            &format!("{}{tu}", per.nombre),
            xt,
            y + 8.0 * e,
            15.0 * e,
            wt,
            if per.dentro() { TEXTO } else { SUAVE },
        );
        let mut marcas = vec![per.codigo.clone()];
        if per.id == c.creador {
            marcas.push(textos.t("cc-creo"));
        }
        if per.id != s.yo && s.mis_aparatos.contains(&per.id) {
            marcas.push(textos.t("cc-tu-otro-aparato"));
        }
        if !per.dentro() {
            marcas.push(textos.t(if per.quito.as_deref() == Some(per.id.as_str()) {
                "cc-salio"
            } else {
                "cc-quitado"
            }));
        }
        p.texto_linea(&marcas.join(" · "), xt, y + 30.0 * e, 12.0 * e, wt, SUAVE);
        if quitar {
            l.boton_texto(
                x + w - wq,
                y + 8.0 * e,
                &textos.t("cc-quitar"),
                PRIMARIO,
                t(Toque::Quitar(per.id.clone())),
            );
        }
        y += alto;
        p.rellenar(rect(x, y, w, e.max(1.0)), BORDE);
    }
    // ¿Quitar? / ¿Salir?
    if let Some(id) = &s.quitando {
        y += 12.0 * e;
        let yo_mismo = *id == s.yo;
        let nombre = c.persona(id).map(|p| p.nombre.clone()).unwrap_or_default();
        let titulo = if yo_mismo {
            textos.t("cc-salir-titulo")
        } else {
            Dicho::con("cc-quitar-titulo", "nombre", nombre).texto(textos)
        };
        let aviso = textos.t(if yo_mismo {
            "cc-salir-aviso"
        } else {
            "cc-quitar-aviso"
        });
        let h = caja_texto(l, x, y, w, Some(&titulo), &aviso);
        y += h + 10.0 * e;
        let medio = (w - 10.0 * e) / 2.0;
        l.boton(
            rect(x, y, medio, 44.0 * e),
            &textos.t("cc-cancelar"),
            None,
            false,
            t(Toque::CancelarQuitar),
        );
        l.boton(
            rect(x + medio + 10.0 * e, y, medio, 44.0 * e),
            &textos.t(if yo_mismo { "cc-salir" } else { "cc-quitar" }),
            None,
            true,
            t(Toque::ConfirmarQuitar),
        );
        y += 44.0 * e;
    }
    if !estoy {
        return y;
    }
    y += 16.0 * e;
    let medio = (w - 10.0 * e) / 2.0;
    if s.invitando.is_none() {
        l.boton(
            rect(x, y, medio, 48.0 * e),
            &textos.t("cc-anadir"),
            Some(&material::ADD),
            false,
            t(Toque::Invitar),
        );
    } else {
        l.boton(
            rect(x, y, medio, 48.0 * e),
            &textos.t("cc-cerrar-invitacion"),
            Some(&material::CLOSE),
            false,
            t(Toque::CerrarInvitacion),
        );
    }
    let rs = rect(x + medio + 10.0 * e, y, medio, 48.0 * e);
    if s.ocupada() {
        boton_apagado(l, rs, &textos.t("cc-sincronizar"));
    } else {
        l.boton(
            rs,
            &textos.t("cc-sincronizar"),
            None,
            true,
            t(Toque::Sincronizar),
        );
    }
    y += 48.0 * e;
    if let Some(i) = &s.invitando {
        y += 16.0 * e;
        let como = textos.t("cc-invitacion-como");
        let vale = textos.t("cc-invitacion-vale");
        let wi = w - 32.0 * e;
        let lado = wi.min(260.0 * e);
        let h = 16.0 * e
            + l.alto_de(&como, 14.0 * e, wi)
            + 12.0 * e
            + lado
            + 12.0 * e
            + 40.0 * e
            + l.alto_de(&vale, 12.0 * e, wi)
            + 16.0 * e;
        caja(p, rect(x, y, w, h), e);
        let mut yy = y + 16.0 * e;
        yy += l.parrafo_centrado(&como, x + 16.0 * e, yy, wi, 14.0 * e, TEXTO) + 12.0 * e;
        let rq = rect(x + (w - lado) / 2.0, yy, lado, lado);
        p.rellenar_redondeado(rq, 12.0 * e, hex_blanco());
        if let Some(qr) = &i.qr {
            crate::recibir::pintar_qr(p, menos(rq, 8.0 * e), qr);
        }
        yy += lado + 12.0 * e;
        l.centrado(
            &invitacion::legible(&i.puerta.codigo),
            x,
            w,
            yy,
            30.0 * e,
            TEXTO,
        );
        yy += 40.0 * e;
        l.parrafo_centrado(&vale, x + 16.0 * e, yy, wi, 12.0 * e, SUAVE);
        y += h;
    }
    y += 24.0 * e;
    l.boton_texto(
        x,
        y,
        &textos.t("cc-salir-del-chat"),
        ERROR,
        t(Toque::Quitar(s.yo.clone())),
    );
    y + 40.0 * e
}

fn pintar_entrar(
    l: &mut Lienzo<'_>,
    textos: &Catalogo,
    s: &Personas,
    x: f32,
    w: f32,
    mut y: f32,
) -> f32 {
    let e = l.e;
    let t = |a: Toque| Accion::Compartidos(a);
    if let Some(c) = &s.dentro_de {
        let con = Dicho::con(
            "cc-con",
            "nombres",
            compartidos::nombres_dentro(c).join(", "),
        );
        y += caja_texto(
            l,
            x,
            y,
            w,
            Some(&Dicho::con("cc-ya-estas", "nombre", c.nombre.clone()).texto(textos)),
            &con.texto(textos),
        );
        y += 16.0 * e;
        let medio = (w - 10.0 * e) / 2.0;
        l.boton(
            rect(x, y, medio, 48.0 * e),
            &textos.t("cc-cerrar"),
            None,
            false,
            t(Toque::Cerrar),
        );
        l.boton(
            rect(x + medio + 10.0 * e, y, medio, 48.0 * e),
            &textos.t("cc-abrir-chat"),
            None,
            true,
            t(Toque::AbrirChat),
        );
        return y + 48.0 * e;
    }
    y += caja_texto(l, x, y, w, None, &textos.t("cc-entrar-como"));
    y += 18.0 * e;
    l.centrado(&textos.t("cc-escribe-codigo"), x, w, y, 14.0 * e, SUAVE);
    y += 26.0 * e;
    let wc = w.min(300.0 * e);
    // El codigo, en la caja de una linea comun: se ve tal como se escribe,
    // con la rayita donde va (antes se reformateaba y la rayita iba
    // siempre al final). Un QR pegado es largo: letra mas pequena.
    let p = l.p;
    let rc = rect(x + (w - wc) / 2.0, y, wc, 64.0 * e);
    p.rellenar_redondeado(rc, 6.0 * e, PRIMARIO);
    p.rellenar_redondeado(menos(rc, 2.0 * e), 5.0 * e, FONDO);
    let largo = s.tecleado.chars().count() > 14;
    pintar_caja(
        p,
        rc,
        &s.tecleado,
        "XXXX-XXXX",
        if largo { 13.0 } else { 26.0 } * e,
        true,
        e,
    );
    l.zonas.push((rc, t(Toque::CampoCodigo)));
    y += 76.0 * e;
    let rb = rect(x + (w - wc) / 2.0, y, wc, 48.0 * e);
    if que_entrar(&s.tecleado).is_some() && !s.ocupada() {
        l.boton(rb, &textos.t("cc-entrar"), None, true, t(Toque::Entrar));
    } else {
        boton_apagado(l, rb, &textos.t("cc-entrar"));
    }
    y + 48.0 * e
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn yo(id: &str, nombre: &str) -> compartidos::Yo {
        compartidos::Yo {
            id: id.into(),
            nombre: nombre.into(),
            codigo: pixpin_sincro::grupo::codigo_de_aparato(id),
            llaves: compartidos::Llaves::nuevas(),
        }
    }

    #[test]
    fn en_un_chat_compartido_lo_de_mi_movil_es_mio_y_lo_de_ana_de_ana() {
        let pc = yo("pc-de-max", "Max");
        let movil = yo("movil-de-max", "Max");
        let ana = yo("movil-de-ana", "Ana");
        let c = compartidos::crear("tesis", "Tesis", &pc, 1);
        let c = compartidos::invitar(&c, &pc, &movil.ficha(0), 2).expect("mi movil entra");
        let c = compartidos::invitar(&c, &pc, &ana.ficha(0), 3).expect("Ana entra");
        let mios: HashSet<String> = ["pc-de-max".to_string(), "movil-de-max".to_string()].into();
        let v = en_el_chat(&c, &mios);
        use pixpin_ui::conversacion::Quien;
        assert_eq!(v.lados.quien(Some(&pc.codigo)), Quien::Yo);
        assert_eq!(v.lados.quien(Some(&movil.codigo)), Quien::Yo);
        assert_eq!(v.lados.quien(Some(&ana.codigo)), Quien::Otro("Ana".into()));
        // Lo de antes de compartir (sin codigo) sigue siendo mio.
        assert_eq!(v.lados.quien(None), Quien::Yo);
        assert_eq!(v.personas, 2, "Max (dos aparatos) y Ana");
        assert_eq!(v.lados.otras_personas(), 1);
    }

    #[test]
    fn se_entra_con_el_codigo_tecleado_o_con_el_texto_del_qr() {
        assert_eq!(que_entrar("k7q2-m9xp"), Some(("K7Q2M9XP".into(), None)));
        assert_eq!(
            que_entrar("pixpin-chat:1:K7Q2M9XP:192.168.1.5:4000"),
            Some(("K7Q2M9XP".into(), Some(("192.168.1.5".into(), 4000))))
        );
        assert_eq!(
            que_entrar("pixpin-chat:1:K7Q2M9XP::0"),
            Some(("K7Q2M9XP".into(), None))
        );
        // Casos negativos: corto, con signos que no son del alfabeto, otro QR.
        assert_eq!(que_entrar("K7Q2"), None);
        assert_eq!(que_entrar("pixpin-envio:1:123456::0"), None);
        assert_eq!(que_entrar(""), None);
    }

    #[test]
    fn abrir_chat_lleva_a_la_ficha_del_chat_traido_y_si_no_esta_lo_dice() {
        let dir = std::env::temp_dir().join(format!("pixpin-abrir-cc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ficha = pixpin_proyecto::almacen::Ficha::nueva("Tesis", 1, "PCPC");
        let mut i = pixpin_proyecto::almacen::Indice::leer(&dir);
        i.proyectos.push(ficha.clone());
        i.guardar(&dir).unwrap();
        let chat = pixpin_proyecto::vista::chat_de_ficha(&dir, &ficha.id).unwrap();
        assert_eq!(
            super::super::ficha_del_chat(&dir, &chat),
            Some(ficha.id.clone())
        );
        // Caso negativo: un chat que aqui no hay no lleva a ninguna parte, y
        // la pantalla lo dice en vez de cerrarse.
        assert_eq!(super::super::ficha_del_chat(&dir, "chat-que-no-esta"), None);
        let (tx, _rx) = std::sync::mpsc::channel();
        let cx = Contexto {
            raiz: dir.clone(),
            nombre: "Max".into(),
            id: "id-pc".into(),
            tx,
        };
        let mut p = Personas::entrar(1, &cx);
        p.dentro_de = Some(ChatCompartido {
            chat: "chat-que-no-esta".into(),
            nombre: "Otro".into(),
            creador: "id-ana".into(),
            creado: 1,
            personas: Vec::new(),
        });
        assert!(!tocar(&mut p, Toque::AbrirChat, &cx));
        assert_eq!(p.aviso, Some(Dicho::de("cc-no-se-abre")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn los_avisos_se_traducen_con_sus_datos() {
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let d = Dicho::con("cc-sincronizado", "nombres", "Ana, Eva");
        assert!(d.texto(&textos).contains("Ana, Eva"));
        let en = Catalogo::nuevo(pixpin_store::Idioma::Ingles);
        assert!(d.texto(&en).contains("Ana, Eva"));
        assert_ne!(d.texto(&textos), d.texto(&en));
    }

    #[test]
    fn el_autor_de_un_mensaje_sale_de_la_lista_y_lo_mio_no() {
        let dir = std::env::temp_dir().join(format!("pixpin-autor-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let id = pixpin_proyecto::identidad::Identidad::leer_o_crear(&dir, "PC").unwrap();
        let cd = CompartidosEnDisco::nuevo(&dir);
        let yo = cd.yo(&id.yo.id, "Max").unwrap();
        let ana = compartidos::Yo {
            id: "id-ana".into(),
            nombre: "Ana".into(),
            codigo: "ANAA".into(),
            llaves: compartidos::Llaves::nuevas(),
        };
        let c = compartidos::crear("p1", "Tesis", &yo, 1);
        let c = compartidos::invitar(&c, &yo, &ana.ficha(0), 2).unwrap();
        cd.poner(&c).unwrap();
        assert_eq!(autor(&dir, "ANAA").as_deref(), Some("Ana"));
        assert_eq!(autor(&dir, &yo.codigo), None, "lo mio se ve como siempre");
        assert_eq!(autor(&dir, "ZZZZ"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Pinta la pantalla a un PNG para mirarla, con datos de EJEMPLO en una
    /// carpeta temporal. `PIXPIN_MUESTRAS` dice donde dejarla.
    /// `cargo test -p pixpin -- --ignored --test-threads=1 muestra_de_chats_compartidos`
    #[test]
    #[ignore]
    fn muestra_de_chats_compartidos() {
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        let dir = std::env::temp_dir().join(format!("pixpin-muestra-cc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let id = super::super::leer_identidad(&dir).unwrap();
        let cd = CompartidosEnDisco::nuevo(&dir);
        cd.poner_nombre("Max").unwrap();
        let yo = cd.yo(&id.yo.id, "Max").unwrap();
        let otro = |n: &str, c: &str| compartidos::Yo {
            id: format!("id-{n}"),
            nombre: n.into(),
            codigo: c.into(),
            llaves: compartidos::Llaves::nuevas(),
        };
        let mut c = compartidos::crear("p1", "Tesis", &yo, 1);
        c = compartidos::invitar(&c, &yo, &otro("Ana", "ANAA").ficha(0), 2).unwrap();
        c = compartidos::invitar(&c, &yo, &otro("Eva", "EVAA").ficha(0), 3).unwrap();
        c = compartidos::quitar(&c, &yo, "id-Eva", 4).unwrap();
        cd.poner(&c).unwrap();
        let (tx, _rx) = std::sync::mpsc::channel();
        let cx = Contexto {
            raiz: dir.clone(),
            nombre: "Max".into(),
            id: id.yo.id.clone(),
            tx,
        };
        let mut con_chat = Personas::de_chat(1, &cx, "p1".into(), "Tesis".into());
        let puerta = Puerta::abrir_con(
            Arc::new(DiscoPc::nuevo(&dir)),
            "p1",
            "K7Q2M9XP",
            ("127.0.0.1", 0),
            0,
            super::super::ahora_ms,
            |_| {},
        )
        .unwrap();
        let qr = crate::recibir::qr_de(&invitacion::texto_del_qr(
            "K7Q2M9XP",
            Some("192.168.1.5"),
            puerta.puerto,
        ));
        con_chat.invitando = Some(Invitando {
            puerta,
            _anuncio: None,
            qr,
        });
        con_chat.aviso = Some(Dicho::con("cc-entro", "nombre", "Ana"));
        let mut entrar = Personas::entrar(2, &cx);
        entrar.tecleado = "k7q2m9xp".into();
        let mut dentro = Personas::entrar(3, &cx);
        dentro.dentro_de = Some(c.clone());
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        for (nombre, s, alto) in [
            ("cc-personas", &con_chat, 1300u32),
            ("cc-entrar", &entrar, 760),
            ("cc-dentro", &dentro, 520),
        ] {
            let (w, h) = (480u32, alto);
            let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), w, h).expect("superficie");
            motor
                .dibujar(&fuera.destino, |p| {
                    p.rellenar(rect(0.0, 0.0, w as f32, h as f32), FONDO);
                    let mut l = Lienzo {
                        p,
                        e: 1.0,
                        zonas: Vec::new(),
                    };
                    p.texto(&s.titulo(&textos), 20.0, 14.0, 24.0, TEXTO);
                    pintar(&mut l, &textos, s, 20.0, w as f32 - 40.0, 50.0);
                })
                .expect("pintar");
            fuera.esperar_gpu().expect("esperar");
            let (_, _, pixeles) = fuera.leer_rgba().expect("leer");
            let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::ImagenRgba {
                ancho: w,
                alto: h,
                pixeles,
            })
            .unwrap();
            let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            std::fs::write(carpeta.join(format!("{nombre}.png")), png).unwrap();
        }
        drop(con_chat);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
