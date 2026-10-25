//! **Cambiar la foto del chat** (25-oct-2026, el usuario: «permitir poner un
//! fondo en las esferas de los chats para que parezca mas una app de chat:
//! dame la opcion de poner emojis como su perfil, o poner una imagen yo»).
//!
//! Una hoja pequena, como la caja de soltar: siempre encima, sin boton en la
//! barra de tareas y en su propio hilo. Se abre con el clic derecho sobre un
//! chat de la lista («Cambiar foto del chat…») y pulsando el avatar de la
//! isla del chat. De arriba abajo:
//!
//! - **El circulo grande**, con lo que quedara. Con una imagen se arrastra
//!   para moverla y la rueda acerca hacia el raton (`Recorte`): lo que se ve
//!   en el circulo es lo que se guarda.
//! - **Elegir imagen…**, **Pegar imagen** (tambien Ctrl+V) y **Como
//!   siempre** (las iniciales en su color de siempre).
//! - **El fondo**: doce degradados, detras del emoji o de las iniciales.
//! - **Los emojis**, en rejilla, con su buscador encima (se escribe sin
//!   pulsar nada: «casa», «obra», «book»…).
//! - **Cancelar** y **Guardar**. Escape cancela; Intro guarda.
//!
//! Guardar escribe la ficha (`pixpin_proyecto::avatar`) y avisa al chat, que
//! relee la lista; la foto viaja a los demas aparatos en la siguiente vuelta.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_codec::ImagenRgba;
use pixpin_geom::{Punto, Rect};
use pixpin_proyecto::almacen::Ficha;
use pixpin_proyecto::avatar::{self, Avatar};
use pixpin_render::{Color, Interpolacion, MotorRender, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::Catalogo;
use pixpin_ui::foto_del_chat::{self as ui, EMOJIS, FONDOS, Recorte};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use super::{CLARO, OSCURO, Tema, encoger, hex, logo, mi};
use crate::overlay::Recursos;
use crate::ventanita::{Botones, dentro};

const VK_ESCAPE: u32 = 0x1B;
const VK_RETURN: u32 = 0x0D;
const VK_BACK: u32 = 0x08;
const VK_V: u32 = 0x56;
/// El tamano logico de la hoja.
const ANCHO: u32 = 360;
const ALTO: u32 = 600;
const MARGEN: f32 = 14.0;
/// El circulo grande.
const VISTA: f32 = 132.0;
const COLUMNAS: usize = 8;
const FILAS_VISIBLES: usize = 4;
const CELDA_ALTO: f32 = 40.0;
/// Lo mas grande que se guarda en memoria de una foto elegida: de sobra
/// para acercar ocho veces sobre 256 y sin subir texturas enormes.
const LADO_DE_TRABAJO: u32 = 2048;
const AVISO_DURA: Duration = Duration::from_millis(2500);

/// Que chat cambia de foto y como esta ahora.
#[derive(Debug, Clone)]
pub(super) struct Pedido {
    pub raiz: PathBuf,
    pub id: String,
    pub nombre: String,
    pub codigo: String,
    pub avatar: Option<Avatar>,
}

/// Los rotulos, ya traducidos (el `Catalogo` no cruza hilos).
#[derive(Debug, Clone)]
pub(super) struct Rotulos {
    titulo: String,
    imagen: String,
    pegar: String,
    quitar: String,
    buscar: String,
    sin_emojis: String,
    fondo: String,
    recorte: String,
    guardar: String,
    cancelar: String,
    sin_imagen: String,
    error: String,
}

impl Rotulos {
    pub(super) fn de(t: &Catalogo) -> Rotulos {
        Rotulos {
            titulo: t.t("foto-chat-titulo"),
            imagen: t.t("foto-chat-imagen"),
            pegar: t.t("foto-chat-pegar"),
            quitar: t.t("foto-chat-quitar"),
            buscar: t.t("foto-chat-buscar"),
            sin_emojis: t.t("foto-chat-sin-emojis"),
            fondo: t.t("foto-chat-fondo"),
            recorte: t.t("foto-chat-recorte"),
            guardar: t.t("foto-chat-guardar"),
            cancelar: t.t("foto-chat-cancelar"),
            sin_imagen: t.t("foto-chat-sin-imagen"),
            error: t.t("foto-chat-error"),
        }
    }
}

/// La hoja abierta, si hay una: solo cabe una (`0` mientras nace).
static ABIERTA: Mutex<Option<isize>> = Mutex::new(None);

/// **Abre la hoja de la foto de `f`**, o trae delante la que ya esta.
pub(super) fn abrir(raiz: &Path, f: &Ficha, textos: &Catalogo) {
    {
        let mut g = ABIERTA.lock().unwrap_or_else(|e| e.into_inner());
        match *g {
            Some(0) => return,
            Some(h) => {
                VentanaOverlay::restaurar_de_hwnd(windows::Win32::Foundation::HWND(h as *mut _));
                return;
            }
            None => *g = Some(0),
        }
    }
    let pedido = Pedido {
        raiz: raiz.to_path_buf(),
        id: f.id.clone(),
        nombre: f.nombre.clone(),
        codigo: f.codigo_unico(),
        avatar: avatar::de_ficha_o_logo(raiz, f),
    };
    let rotulos = Rotulos::de(textos);
    let soltar = || *ABIERTA.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let hecho = std::thread::Builder::new()
        .name("foto-del-chat".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            if let Err(e) = crate::dispositivo_perdido::con_recursos("foto del chat", |r| {
                hoja(r, &pedido, &rotulos)
            }) {
                tracing::warn!(?e, "no se pudo abrir la foto del chat");
            }
            *ABIERTA.lock().unwrap_or_else(|e| e.into_inner()) = None;
        });
    if let Err(e) = hecho {
        soltar();
        tracing::warn!(?e, "no se pudo lanzar el hilo de la foto del chat");
    }
}

// --- Lo elegido -------------------------------------------------------------

/// Lo que quedara en el circulo.
enum Eleccion {
    /// Una imagen nueva: entera (reducida a `LADO_DE_TRABAJO`) y que
    /// cuadrado suyo entra.
    Nueva {
        imagen: ImagenRgba,
        recorte: Recorte,
        bitmap: Option<ID2D1Bitmap1>,
    },
    /// La imagen que ya tenia, sin tocar.
    LaDeAntes(Option<ID2D1Bitmap1>),
    Emoji(String),
    Iniciales,
}

struct Estado {
    eleccion: Eleccion,
    /// El degradado elegido. Sin el, el color de siempre del chat.
    fondo: Option<(u32, u32)>,
    /// El ultimo emoji elegido: elegir un fondo con una imagen puesta vuelve
    /// a el (o a las iniciales).
    ultimo_emoji: Option<String>,
    busqueda: String,
    /// La primera fila de emojis que se ve.
    scroll: usize,
    aviso: Option<(String, Instant)>,
    /// Si hay algo que guardar.
    tocado: bool,
}

impl Estado {
    fn de(p: &Pedido, motor: &MotorRender) -> Estado {
        let a = p.avatar.as_ref();
        let eleccion = match a {
            Some(a) if a.es_imagen() => Eleccion::LaDeAntes(
                avatar::ruta_de_la_imagen(&p.raiz, &p.id, a)
                    .and_then(|r| logo::leer(&r))
                    .and_then(|f| f.subir(motor)),
            ),
            Some(a) => match a.el_emoji() {
                Some(e) => Eleccion::Emoji(e.to_string()),
                None => Eleccion::Iniciales,
            },
            None => Eleccion::Iniciales,
        };
        Estado {
            ultimo_emoji: a.and_then(Avatar::el_emoji).map(str::to_string),
            fondo: a.and_then(Avatar::fondo),
            eleccion,
            busqueda: String::new(),
            scroll: 0,
            aviso: None,
            tocado: false,
        }
    }

    fn avisar(&mut self, t: &str) {
        self.aviso = Some((t.to_string(), Instant::now()));
    }

    /// Pone una imagen recien leida, entera y con su cuadrado del centro.
    fn poner_imagen(&mut self, imagen: ImagenRgba, motor: &MotorRender) {
        let imagen = reducida(imagen);
        let bitmap = motor
            .bitmap_desde_pixeles_premultiplicado(imagen.ancho, imagen.alto, &imagen.pixeles)
            .inspect_err(|e| tracing::warn!(?e, "no se pudo subir la foto elegida"))
            .ok();
        self.eleccion = Eleccion::Nueva {
            recorte: Recorte::centrado(imagen.ancho, imagen.alto),
            imagen,
            bitmap,
        };
        self.tocado = true;
    }

    fn poner_emoji(&mut self, e: &str, codigo: &str) {
        if self.fondo.is_none() {
            let f = FONDOS[ui::fondo_propuesto(codigo)];
            self.fondo = Some((f.arriba, f.abajo));
        }
        self.ultimo_emoji = Some(e.to_string());
        self.eleccion = Eleccion::Emoji(e.to_string());
        self.tocado = true;
    }

    fn poner_fondo(&mut self, n: usize) {
        let f = FONDOS[n];
        self.fondo = Some((f.arriba, f.abajo));
        if matches!(
            self.eleccion,
            Eleccion::Nueva { .. } | Eleccion::LaDeAntes(_)
        ) {
            self.eleccion = match &self.ultimo_emoji {
                Some(e) => Eleccion::Emoji(e.clone()),
                None => Eleccion::Iniciales,
            };
        }
        self.tocado = true;
    }

    fn como_siempre(&mut self) {
        self.eleccion = Eleccion::Iniciales;
        self.fondo = None;
        self.tocado = true;
    }
}

/// Una foto grande, reducida para trabajar con ella sin deformarla.
fn reducida(imagen: ImagenRgba) -> ImagenRgba {
    let mayor = imagen.ancho.max(imagen.alto);
    if mayor <= LADO_DE_TRABAJO {
        return imagen;
    }
    let k = LADO_DE_TRABAJO as f32 / mayor as f32;
    let (w, h) = (
        ((imagen.ancho as f32 * k).round() as u32).max(1),
        ((imagen.alto as f32 * k).round() as u32).max(1),
    );
    match pixpin_codec::redimensionar(imagen.clone(), w, h) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(?e, "no se pudo reducir la foto elegida");
            imagen
        }
    }
}

/// La imagen del portapapeles: copiada como imagen o como fichero.
fn imagen_del_portapapeles() -> Option<ImagenRgba> {
    match pixpin_codec::portapapeles::leer()? {
        pixpin_codec::ContenidoPortapapeles::Imagen(i) if i.ancho > 0 && i.alto > 0 => Some(i),
        pixpin_codec::ContenidoPortapapeles::Rutas(r) => {
            crate::imagenes_lienzo::imagen_de_rutas(&r)
        }
        _ => None,
    }
}

fn ahora_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// **Guarda lo elegido**. `false` si no habia nada que guardar.
fn guardar(p: &Pedido, e: &Estado) -> Result<bool> {
    if !e.tocado {
        return Ok(false);
    }
    let cuando = ahora_ms();
    let textos = e
        .fondo
        .map(|(a, b)| (avatar::texto_de_color(a), avatar::texto_de_color(b)));
    let fondo = textos.as_ref().map(|(a, b)| (a.as_str(), b.as_str()));
    match &e.eleccion {
        Eleccion::Nueva {
            imagen, recorte, ..
        } => {
            let cuadrado = recorte.en_pixeles(imagen.ancho, imagen.alto);
            let c = logo::recortar(imagen, cuadrado).context("recorte fuera de la foto")?;
            logo::guardar_imagen(&p.raiz, &p.id, c, cuando)?;
        }
        Eleccion::LaDeAntes(_) => return Ok(false),
        Eleccion::Emoji(em) => avatar::poner(&p.raiz, &p.id, &Avatar::emoji(em, fondo, cuando))?,
        Eleccion::Iniciales => avatar::poner(&p.raiz, &p.id, &Avatar::iniciales(fondo, cuando))?,
    }
    Ok(true)
}

// --- Donde va cada cosa -------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Cerrar,
    Guardar,
    Imagen,
    Pegar,
    Quitar,
    Fondo(usize),
    /// Por su lugar en `EMOJIS`.
    Emoji(usize),
    Vista,
    /// La barra de arriba: se arrastra la hoja.
    Barra,
}

/// Los sitios de la hoja, en pixeles de la ventana.
struct Sitios {
    cerrar: RectF,
    barra: RectF,
    vista: RectF,
    pista_y: f32,
    botones: [RectF; 3],
    rotulo_fondo_y: f32,
    fondos: RectF,
    buscador: RectF,
    rejilla: RectF,
    cancelar: RectF,
    guardar: RectF,
}

fn sitios(w: f32, h: f32, e: f32) -> Sitios {
    let m = MARGEN * e;
    let dentro_w = w - 2.0 * m;
    let vista = VISTA * e;
    let vy = 52.0 * e;
    let by = vy + vista + 30.0 * e;
    let alto_boton = 32.0 * e;
    let hueco = 8.0 * e;
    let bw = (dentro_w - 2.0 * hueco) / 3.0;
    let boton = |i: f32| RectF {
        x: m + i * (bw + hueco),
        y: by,
        ancho: bw,
        alto: alto_boton,
    };
    let rotulo_fondo_y = by + alto_boton + 14.0 * e;
    let fondos = RectF {
        x: m,
        y: rotulo_fondo_y + 20.0 * e,
        ancho: dentro_w,
        alto: 26.0 * e,
    };
    let buscador = RectF {
        x: m,
        y: fondos.y + fondos.alto + 14.0 * e,
        ancho: dentro_w,
        alto: 32.0 * e,
    };
    let rejilla = RectF {
        x: m,
        y: buscador.y + buscador.alto + 8.0 * e,
        ancho: dentro_w,
        alto: CELDA_ALTO * e * FILAS_VISIBLES as f32,
    };
    let pie = 34.0 * e;
    let guardar = RectF {
        x: w - m - 100.0 * e,
        y: h - m - pie,
        ancho: 100.0 * e,
        alto: pie,
    };
    Sitios {
        cerrar: RectF {
            x: w - 40.0 * e,
            y: 10.0 * e,
            ancho: 28.0 * e,
            alto: 28.0 * e,
        },
        barra: RectF {
            x: 0.0,
            y: 0.0,
            ancho: w,
            alto: 46.0 * e,
        },
        vista: RectF {
            x: (w - vista) / 2.0,
            y: vy,
            ancho: vista,
            alto: vista,
        },
        pista_y: vy + vista + 8.0 * e,
        botones: [boton(0.0), boton(1.0), boton(2.0)],
        rotulo_fondo_y,
        fondos,
        buscador,
        rejilla,
        cancelar: RectF {
            x: guardar.x - 8.0 * e - 100.0 * e,
            ..guardar
        },
        guardar,
    }
}

/// La celda `i` de la rejilla visible (por lugar desde la primera visible).
fn celda(rejilla: RectF, e: f32, i: usize) -> RectF {
    let ancho = rejilla.ancho / COLUMNAS as f32;
    RectF {
        x: rejilla.x + (i % COLUMNAS) as f32 * ancho,
        y: rejilla.y + (i / COLUMNAS) as f32 * CELDA_ALTO * e,
        ancho,
        alto: CELDA_ALTO * e,
    }
}

// --- La ventana -------------------------------------------------------------

fn hoja(recursos: &Recursos, pedido: &Pedido, r: &Rotulos) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .monitor_en(pixpin_shell::posicion_del_cursor())
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let e = monitor.escala_por_cien as f32 / 100.0;
    let mut marco =
        crate::ventanita::centrado(monitor.area_trabajo, ANCHO, ALTO, monitor.escala_por_cien);
    let mut ventana = VentanaOverlay::nueva(marco).context("no se pudo abrir la hoja")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para la hoja")?;
    ventana.mostrar();
    ventana.traer_encima();
    ventana.enfocar();
    *ABIERTA.lock().unwrap_or_else(|e| e.into_inner()) = Some(ventana.handle().0 as isize);
    let tema: &Tema = if pixpin_shell::entorno::tema_claro() && !crate::tema_cosmos::activo() {
        &CLARO
    } else {
        &OSCURO
    };

    let mut estado = Estado::de(pedido, &motor);
    let mut botones: Botones<Accion> = Botones::default();
    // Lo pulsado: donde, el marco de entonces, que habia debajo y el ultimo
    // punto (para arrastrar la foto o la hoja).
    let mut pulsado: Option<(Punto, Rect, Option<Accion>, Punto)> = None;
    let mut vivo = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        let mut accion = None;
        let s = sitios(marco.ancho as f32, marco.alto as f32, e);
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            let local = |p: Punto| ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
            match ev {
                EventoOverlay::Cerrar | EventoOverlay::Tecla { vk: VK_ESCAPE, .. } => vivo = false,
                EventoOverlay::Tecla { vk: VK_RETURN, .. } => accion = Some(Accion::Guardar),
                EventoOverlay::Tecla { vk: VK_BACK, .. } => {
                    estado.busqueda.pop();
                    estado.scroll = 0;
                }
                EventoOverlay::Tecla {
                    vk: VK_V,
                    ctrl: true,
                    ..
                } => accion = Some(Accion::Pegar),
                EventoOverlay::Caracter(c) if !c.is_control() => {
                    estado.busqueda.push(c);
                    estado.scroll = 0;
                }
                EventoOverlay::RatonMovido(p) => {
                    botones.raton = local(p);
                    if let Some((desde, origen, que, ultimo)) = pulsado.as_mut() {
                        match que {
                            Some(Accion::Vista) => {
                                if let Eleccion::Nueva {
                                    imagen, recorte, ..
                                } = &mut estado.eleccion
                                {
                                    recorte.mover(
                                        imagen.ancho,
                                        imagen.alto,
                                        s.vista.ancho,
                                        (p.x - ultimo.x) as f32,
                                        (p.y - ultimo.y) as f32,
                                    );
                                    estado.tocado = true;
                                }
                            }
                            Some(Accion::Barra) => {
                                marco = Rect {
                                    x: origen.x + p.x - desde.x,
                                    y: origen.y + p.y - desde.y,
                                    ..*origen
                                };
                                ventana.mover(marco);
                                botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                            }
                            _ => {}
                        }
                        *ultimo = p;
                    }
                }
                EventoOverlay::BotonPulsado(p) => {
                    botones.raton = local(p);
                    let que = botones.bajo_el_raton();
                    if que == Some(Accion::Vista) || que == Some(Accion::Barra) {
                        ventana.capturar_raton();
                    }
                    pulsado = Some((p, marco, que, p));
                }
                EventoOverlay::BotonSoltado(p) => {
                    botones.raton = local(p);
                    ventana.soltar_raton();
                    if let Some((_, _, Some(a), _)) = pulsado.take()
                        && botones.bajo_el_raton() == Some(a)
                        && !matches!(a, Accion::Vista | Accion::Barra)
                    {
                        accion = Some(a);
                    }
                }
                EventoOverlay::Rueda(giro) => {
                    if dentro(s.vista, botones.raton)
                        && let Eleccion::Nueva {
                            imagen, recorte, ..
                        } = &mut estado.eleccion
                    {
                        let punto = (
                            (botones.raton.0 - s.vista.x) / s.vista.ancho,
                            (botones.raton.1 - s.vista.y) / s.vista.alto,
                        );
                        recorte.rueda(imagen.ancho, imagen.alto, giro, punto);
                        estado.tocado = true;
                    } else if dentro(s.rejilla, botones.raton) {
                        let filas = ui::buscar(&estado.busqueda).len().div_ceil(COLUMNAS);
                        let tope = filas.saturating_sub(FILAS_VISIBLES);
                        estado.scroll = if giro > 0 {
                            estado.scroll.saturating_sub(1)
                        } else {
                            (estado.scroll + 1).min(tope)
                        };
                    }
                }
                _ => {}
            }
        }
        match accion {
            Some(Accion::Cerrar) => vivo = false,
            Some(Accion::Guardar) => match guardar(pedido, &estado) {
                Ok(_) => {
                    super::refrescar();
                    vivo = false;
                }
                Err(err) => {
                    tracing::warn!(?err, "no se pudo guardar la foto del chat");
                    estado.avisar(&r.error);
                }
            },
            Some(Accion::Imagen) => {
                if let Some(ruta) = pixpin_shell::elegir::pedir_una_imagen(ventana.handle()) {
                    match pixpin_codec::cargar(&ruta) {
                        Ok(i) if i.ancho > 0 && i.alto > 0 => estado.poner_imagen(i, &motor),
                        otra => {
                            tracing::info!(?otra, "foto del chat que no se lee");
                            estado.avisar(&r.error);
                        }
                    }
                }
                ventana.traer_encima();
                ventana.enfocar();
            }
            Some(Accion::Pegar) => match imagen_del_portapapeles() {
                Some(i) => estado.poner_imagen(i, &motor),
                None => estado.avisar(&r.sin_imagen),
            },
            Some(Accion::Quitar) => estado.como_siempre(),
            Some(Accion::Fondo(n)) => estado.poner_fondo(n),
            Some(Accion::Emoji(n)) => estado.poner_emoji(EMOJIS[n].0, &pedido.codigo),
            _ => {}
        }
        if !vivo {
            break;
        }
        if estado
            .aviso
            .as_ref()
            .is_some_and(|(_, d)| d.elapsed() >= AVISO_DURA)
        {
            estado.aviso = None;
        }

        if let Ok(d) = superficie.empezar(&motor) {
            let _ = motor.dibujar(&d, |p: &Pintor| {
                pintar(p, &s, e, tema, &estado, pedido, r, &mut botones)
            });
            let _ = superficie.presentar();
        }
        let latido = if estado.aviso.is_some() { 120 } else { 500 };
        pixpin_shell::overlay::esperar_eventos(Some(latido));
    }
    ventana.ocultar();
    Ok(())
}

// --- Pintar -----------------------------------------------------------------

/// Un boton redondeado con su rotulo centrado (y que no se sale).
fn boton(
    p: &Pintor,
    b: &mut Botones<Accion>,
    caja: RectF,
    que: Accion,
    rotulo: &str,
    tema: &Tema,
    fuerte: bool,
    e: f32,
) {
    let encima = dentro(caja, b.raton);
    let (fondo, texto) = match (fuerte, encima) {
        (true, _) => (tema.enviar, Color::BLANCO),
        (false, true) => (tema.boton_sobre, tema.texto),
        (false, false) => (tema.campo, tema.texto),
    };
    p.rellenar_redondeado(caja, caja.alto / 2.0, fondo);
    let tam = 12.5 * e;
    let hueco = 8.0 * e;
    let (w, h) = p.medir_texto(rotulo, tam);
    let w = w.min(caja.ancho - 2.0 * hueco);
    p.texto_linea(
        rotulo,
        caja.x + (caja.ancho - w) / 2.0,
        caja.y + (caja.alto - h) / 2.0,
        tam,
        caja.ancho - 2.0 * hueco,
        texto,
    );
    b.zona(caja, que);
}

#[allow(clippy::too_many_arguments)]
fn pintar(
    p: &Pintor,
    s: &Sitios,
    e: f32,
    tema: &Tema,
    est: &Estado,
    pedido: &Pedido,
    r: &Rotulos,
    b: &mut Botones<Accion>,
) {
    let (w, h) = (s.barra.ancho, s.guardar.y + s.guardar.alto + MARGEN * e);
    p.limpiar_transparente();
    let todo = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: h,
    };
    p.rellenar_redondeado(todo, 18.0 * e, tema.isla_borde);
    p.rellenar_redondeado(encoger(todo, 1.0), 17.0 * e, tema.isla);
    b.vaciar();
    b.zona(s.barra, Accion::Barra);

    // Arriba: el titulo con el nombre del chat, y el aspa.
    let tam = 15.0 * e;
    let titulo = format!("{} · {}", r.titulo, pedido.nombre);
    let (_, th) = p.medir_texto(&titulo, tam);
    p.texto_linea(
        &titulo,
        MARGEN * e + 4.0 * e,
        s.cerrar.y + (s.cerrar.alto - th) / 2.0,
        tam,
        (s.cerrar.x - MARGEN * e - 8.0 * e).max(0.0),
        tema.pildora_texto,
    );
    if dentro(s.cerrar, b.raton) {
        p.rellenar_redondeado(s.cerrar, s.cerrar.alto / 2.0, tema.boton_sobre);
    }
    p.icono(&mi::CLOSE, encoger(s.cerrar, 5.0 * e), tema.campo_apagado);
    b.zona(s.cerrar, Accion::Cerrar);

    // El circulo grande: lo que quedara.
    let v = s.vista;
    let fondo = est.fondo;
    match &est.eleccion {
        Eleccion::Nueva {
            imagen,
            recorte,
            bitmap,
        } => {
            if p.empujar_recorte_redondeado(v, v.ancho / 2.0) {
                p.rellenar(v, tema.campo);
                if let Some(bm) = bitmap {
                    let (x, y, dw, dh) =
                        recorte.destino((v.x, v.y, v.ancho), imagen.ancho, imagen.alto);
                    p.bitmap_con(
                        bm,
                        RectF {
                            x,
                            y,
                            ancho: dw,
                            alto: dh,
                        },
                        None,
                        Interpolacion::Lineal,
                    );
                }
                p.soltar_recorte_redondeado();
            }
            p.anillo(
                (v.x + v.ancho / 2.0, v.y + v.alto / 2.0),
                v.ancho / 2.0,
                1.0,
                tema.campo_apagado,
            );
            b.zona(v, Accion::Vista);
            let tam = 11.5 * e;
            let (tw, _) = p.medir_texto(&r.recorte, tam);
            p.texto(
                &r.recorte,
                (w - tw) / 2.0,
                s.pista_y,
                tam,
                tema.campo_apagado,
            );
        }
        Eleccion::LaDeAntes(bm) => {
            let a = Avatar::imagen("", 0);
            logo::pintar_avatar(p, v, Some(&a), bm.as_ref(), &pedido.nombre, &pedido.codigo);
        }
        Eleccion::Emoji(em) => {
            let a = Avatar {
                tipo: avatar::EMOJI.into(),
                emoji: Some(em.clone()),
                ..Default::default()
            };
            pintar_con_fondo(p, v, &a, fondo, pedido);
        }
        Eleccion::Iniciales => {
            pintar_con_fondo(p, v, &Avatar::iniciales(None, 0), fondo, pedido);
        }
    }

    // Los tres botones.
    let rotulos = [
        (Accion::Imagen, &r.imagen),
        (Accion::Pegar, &r.pegar),
        (Accion::Quitar, &r.quitar),
    ];
    for (caja, (que, rotulo)) in s.botones.iter().zip(rotulos) {
        boton(p, b, *caja, que, rotulo, tema, false, e);
    }

    // El fondo: los doce degradados, el elegido con su aro.
    p.texto(
        &r.fondo,
        s.fondos.x,
        s.rotulo_fondo_y,
        12.0 * e,
        tema.campo_apagado,
    );
    let lado = s.fondos.alto - 4.0 * e;
    let paso = (s.fondos.ancho - lado) / (FONDOS.len() - 1) as f32;
    let elegido = fondo.and_then(|(a, b)| ui::lugar_del_fondo(a, b));
    for (i, f) in FONDOS.iter().enumerate() {
        let caja = RectF {
            x: s.fondos.x + i as f32 * paso,
            y: s.fondos.y + 2.0 * e,
            ancho: lado,
            alto: lado,
        };
        if elegido == Some(i) || dentro(caja, b.raton) {
            p.anillo(
                (caja.x + lado / 2.0, caja.y + lado / 2.0),
                lado / 2.0 + 2.0 * e,
                (1.5 * e).max(1.0),
                if elegido == Some(i) {
                    tema.texto
                } else {
                    tema.campo_apagado
                },
            );
        }
        logo::pintar_fondo(p, caja, (f.arriba, f.abajo));
        b.zona(caja, Accion::Fondo(i));
    }

    // El buscador de emojis: se escribe sin pulsarlo.
    let bus = s.buscador;
    p.rellenar_redondeado(bus, bus.alto / 2.0, tema.buscador);
    let lupa = 16.0 * e;
    p.icono(
        &mi::SEARCH,
        RectF {
            x: bus.x + 10.0 * e,
            y: bus.y + (bus.alto - lupa) / 2.0,
            ancho: lupa,
            alto: lupa,
        },
        tema.campo_apagado,
    );
    let tam = 13.0 * e;
    let tx = bus.x + 10.0 * e + lupa + 8.0 * e;
    let (texto, color) = if est.busqueda.is_empty() {
        (r.buscar.clone(), tema.campo_apagado)
    } else {
        (format!("{}|", est.busqueda), tema.texto)
    };
    let (_, th) = p.medir_texto(&texto, tam);
    p.texto_linea(
        &texto,
        tx,
        bus.y + (bus.alto - th) / 2.0,
        tam,
        bus.x + bus.ancho - tx - 10.0 * e,
        color,
    );

    // La rejilla de emojis.
    let lista = ui::buscar(&est.busqueda);
    if lista.is_empty() {
        let tam = 12.5 * e;
        let (tw, _) = p.medir_texto(&r.sin_emojis, tam);
        p.texto(
            &r.sin_emojis,
            s.rejilla.x + (s.rejilla.ancho - tw) / 2.0,
            s.rejilla.y + 20.0 * e,
            tam,
            tema.campo_apagado,
        );
    }
    let elegido = match &est.eleccion {
        Eleccion::Emoji(x) => Some(x.as_str()),
        _ => None,
    };
    let desde = est.scroll * COLUMNAS;
    for (i, n) in lista
        .iter()
        .skip(desde)
        .take(COLUMNAS * FILAS_VISIBLES)
        .enumerate()
    {
        let c = celda(s.rejilla, e, i);
        let em = EMOJIS[*n].0;
        if elegido == Some(em) {
            p.rellenar_redondeado(encoger(c, 2.0 * e), 8.0 * e, tema.fila_elegida);
        } else if dentro(c, b.raton) {
            p.rellenar_redondeado(encoger(c, 2.0 * e), 8.0 * e, tema.boton_sobre);
        }
        let tam = 22.0 * e;
        let (ew, eh) = p.medir_texto(em, tam);
        p.texto_color(
            em,
            c.x + (c.ancho - ew) / 2.0,
            c.y + (c.alto - eh) / 2.0,
            tam,
            tema.texto,
        );
        b.zona(c, Accion::Emoji(*n));
    }

    // Abajo: lo que se dijo, Cancelar y Guardar.
    if let Some((t, _)) = &est.aviso {
        p.texto_linea(
            t,
            MARGEN * e,
            s.guardar.y - 22.0 * e,
            12.0 * e,
            w - 2.0 * MARGEN * e,
            hex(0xe05a4f),
        );
    }
    boton(
        p,
        b,
        s.cancelar,
        Accion::Cerrar,
        &r.cancelar,
        tema,
        false,
        e,
    );
    boton(p, b, s.guardar, Accion::Guardar, &r.guardar, tema, true, e);
}

/// El emoji o las iniciales sobre el fondo elegido (o el de siempre).
fn pintar_con_fondo(
    p: &Pintor,
    caja: RectF,
    a: &Avatar,
    fondo: Option<(u32, u32)>,
    pedido: &Pedido,
) {
    let mut a = a.clone();
    if let Some((arriba, abajo)) = fondo {
        a.color = Some(avatar::texto_de_color(arriba));
        a.color2 = Some(avatar::texto_de_color(abajo));
    }
    logo::pintar_avatar(p, caja, Some(&a), None, &pedido.nombre, &pedido.codigo);
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn todo_cabe_en_la_hoja_y_nada_se_pisa() {
        for e in [1.0, 1.5, 2.0] {
            let (w, h) = (ANCHO as f32 * e, ALTO as f32 * e);
            let s = sitios(w, h, e);
            assert!(s.vista.y > s.barra.y + s.barra.alto - 1.0, "{e}");
            assert!(s.botones[0].y > s.pista_y, "{e}: la pista bajo el circulo");
            assert!(s.fondos.y > s.botones[0].y + s.botones[0].alto, "{e}");
            assert!(s.buscador.y > s.fondos.y + s.fondos.alto, "{e}");
            assert!(s.rejilla.y > s.buscador.y + s.buscador.alto, "{e}");
            assert!(
                s.guardar.y > s.rejilla.y + s.rejilla.alto + 20.0 * e,
                "{e}: la rejilla no pisa el aviso ni los botones"
            );
            assert!(s.cancelar.x > MARGEN * e && s.guardar.x + s.guardar.ancho <= w);
            // La ultima celda de la rejilla, dentro de ella.
            let ultima = celda(s.rejilla, e, COLUMNAS * FILAS_VISIBLES - 1);
            assert!(ultima.x + ultima.ancho <= s.rejilla.x + s.rejilla.ancho + 0.01);
            assert!(ultima.y + ultima.alto <= s.rejilla.y + s.rejilla.alto + 0.01);
        }
    }

    #[test]
    fn una_foto_grande_se_reduce_sin_deformarse_y_una_pequena_no() {
        let grande = ImagenRgba {
            ancho: 4096,
            alto: 1024,
            pixeles: vec![255; 4096 * 1024 * 4],
        };
        let r = reducida(grande);
        assert_eq!((r.ancho, r.alto), (LADO_DE_TRABAJO, 512));
        let pequena = ImagenRgba {
            ancho: 30,
            alto: 20,
            pixeles: vec![0; 30 * 20 * 4],
        };
        assert_eq!(reducida(pequena).ancho, 30);
    }

    #[test]
    fn elegir_un_fondo_con_una_imagen_vuelve_al_ultimo_emoji_y_como_siempre_lo_quita_todo() {
        let mut est = Estado {
            eleccion: Eleccion::LaDeAntes(None),
            fondo: None,
            ultimo_emoji: Some("🏠".into()),
            busqueda: String::new(),
            scroll: 0,
            aviso: None,
            tocado: false,
        };
        est.poner_fondo(3);
        assert!(matches!(&est.eleccion, Eleccion::Emoji(x) if x == "🏠"));
        assert_eq!(est.fondo, Some((FONDOS[3].arriba, FONDOS[3].abajo)));
        assert!(est.tocado);
        est.como_siempre();
        assert!(matches!(est.eleccion, Eleccion::Iniciales));
        assert_eq!(est.fondo, None);
        // Un emoji sin fondo se lleva el propuesto del chat.
        est.poner_emoji("🍕", "AGJ36629KT");
        let f = FONDOS[ui::fondo_propuesto("AGJ36629KT")];
        assert_eq!(est.fondo, Some((f.arriba, f.abajo)));
    }

    #[test]
    fn guardar_sin_tocar_nada_no_escribe_y_un_emoji_queda_en_la_ficha() {
        let raiz = std::env::temp_dir().join(format!("pixpin-hoja-foto-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let f = Ficha::nueva("Casa", 1, "PC01");
        pixpin_proyecto::almacen::Indice {
            proyectos: vec![f.clone()],
            ..Default::default()
        }
        .guardar(&raiz)
        .unwrap();
        let pedido = Pedido {
            raiz: raiz.clone(),
            id: f.id.clone(),
            nombre: f.nombre.clone(),
            codigo: f.codigo_unico(),
            avatar: None,
        };
        let mut est = Estado {
            eleccion: Eleccion::Iniciales,
            fondo: None,
            ultimo_emoji: None,
            busqueda: String::new(),
            scroll: 0,
            aviso: None,
            tocado: false,
        };
        // Caso negativo: sin tocar nada no se escribe nada.
        assert!(!guardar(&pedido, &est).unwrap());
        est.poner_emoji("🏗️", &pedido.codigo);
        assert!(guardar(&pedido, &est).unwrap());
        let ficha = pixpin_proyecto::almacen::Indice::leer(&raiz)
            .buscar(&f.id)
            .cloned()
            .unwrap();
        let a = avatar::de_ficha(&ficha).unwrap();
        assert_eq!(a.el_emoji(), Some("🏗️"));
        assert!(a.fondo().is_some());
        // Y una imagen nueva se guarda recortada a su cuadrado.
        est.eleccion = Eleccion::Nueva {
            imagen: ImagenRgba {
                ancho: 40,
                alto: 20,
                pixeles: vec![200; 40 * 20 * 4],
            },
            recorte: Recorte::centrado(40, 20),
            bitmap: None,
        };
        assert!(guardar(&pedido, &est).unwrap());
        let ficha = pixpin_proyecto::almacen::Indice::leer(&raiz)
            .buscar(&f.id)
            .cloned()
            .unwrap();
        let a = avatar::de_ficha(&ficha).unwrap();
        let ruta = avatar::ruta_de_la_imagen(&raiz, &f.id, &a).unwrap();
        let guardada = pixpin_codec::cargar(&ruta).unwrap();
        assert_eq!((guardada.ancho, guardada.alto), (20, 20));
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
