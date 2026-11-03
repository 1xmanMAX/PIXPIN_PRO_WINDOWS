//! **El minichat de una pegatina**: una ventanita junto a ella con su hilo
//! (quien, cuando, que dijo y lo adjuntado) y la caja de escribir.
//!
//! - **Intro** manda; Mayus+Intro parte el renglon. Escape (o un clic fuera)
//!   la cierra.
//! - **Adjuntar** (el clip): el cuadro de abrir de Windows; **Pegar** (o
//!   Ctrl+V): una imagen o ficheros copiados; soltar ficheros encima tambien.
//!   Cada uno va como una entrada.
//! - **Atajo** (el eslabon): el elemento elegido en el lienzo, o un mensaje
//!   del chat del documento (sale la lista de los ultimos).
//! - **Lienzo** (el lapiz): un lienzo propio en blanco, que se abre ya.
//! - Un clic en una **imagen** la abre en el lienzo para anotar encima (lo
//!   anotado se guarda y viaja, unido a esa entrada); en un **fichero**, lo
//!   abre Windows; en un **atajo**, va a el. Clic derecho en una entrada
//!   suya la quita.
//! - Arriba: resolver o reabrir, crear tarea (las de urgencia) y cerrar.
//!
//! Es modal: corre su propio bucle en el hilo del visor y vuelve al
//! cerrarse, con lo que el visor tenga que hacer ([`Pedido`]). Lo que llega
//! de otro aparato mientras esta abierta se ve al momento.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use pindesk_comentarios::{Adjunto, Comentario, Entrada, TipoAdjunto, lista};
use pindesk_geom::{Punto, Rect};
use pindesk_render::icono::{Icono, material};
use pindesk_render::{Color, MotorRender, Pintor, RectF, Superficie};
use pindesk_shell::overlay::{EventoOverlay, VentanaOverlay};
use pindesk_ui::pegatinas::{self as ui, BotonChat, Caja, MiniChat};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use super::pintar::{self, APAGADO, ENCIMA, ISLA, RAYA, TINTA, con_alfa, rf};
use super::{Pedido, Pegador, ahora, textos};
use crate::caja_dibujo::hex;
use crate::caja_texto::{CajaLinea, Colores, PortapapelesWindows};

const VK_ESCAPE: u32 = 0x1B;
const VK_ENTRAR: u32 = 0x0D;
const VK_V: u32 = b'V' as u32;

/// El aire alrededor de la tarjeta (donde cae su sombra).
const AIRE: f32 = 10.0;
/// Lo que mide de lado como mucho la miniatura de una imagen.
const MINIATURA: u32 = 360;

const CAJA_FONDO: Color = hex(0xf3f3f6);
const SELECCION: Color = hex(0xc9d7ff);

/// Lo que hay bajo el raton en el hilo.
#[derive(Debug, Clone, PartialEq)]
enum Accion {
    Boton(BotonChat),
    /// La imagen `k` de una entrada: al lienzo.
    Imagen(String, usize),
    /// El lienzo propio `k` de una entrada.
    Lienzo(String, usize),
    Archivo {
        ruta: String,
        nombre: String,
    },
    Mensaje {
        proyecto: String,
        mensaje: String,
    },
    Elemento {
        lienzo: String,
        punto: Option<[f64; 2]>,
    },
    /// Una fila de la lista de atajos.
    Elegir(usize),
    /// Una entrada entera (para quitarla con el clic derecho).
    Entrada(String),
}

/// Una fila de la lista de atajos.
#[derive(Debug, Clone)]
struct Opcion {
    rotulo: String,
    adjunto: Adjunto,
}

struct Estado {
    id: String,
    caja: CajaLinea,
    /// Cuanto se ha subido desde lo ultimo del hilo.
    subido: f32,
    tope: f32,
    zonas: Vec<(RectF, Accion)>,
    raton: (f32, f32),
    miniaturas: HashMap<PathBuf, Option<(ID2D1Bitmap1, u32, u32)>>,
    atajos: Option<Vec<Opcion>>,
    pedido: Option<Pedido>,
    aviso: Option<String>,
}

fn reducida(img: &pindesk_codec::ImagenRgba) -> (u32, u32, Vec<u8>) {
    let lado = img.ancho.max(img.alto).max(1);
    if lado <= MINIATURA {
        return (img.ancho, img.alto, img.pixeles.clone());
    }
    let (w, h) = (
        (img.ancho * MINIATURA / lado).max(1),
        (img.alto * MINIATURA / lado).max(1),
    );
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        let sy = (y * img.alto / h).min(img.alto - 1);
        for x in 0..w {
            let sx = (x * img.ancho / w).min(img.ancho - 1);
            let i = ((sy * img.ancho + sx) * 4) as usize;
            v.extend_from_slice(&img.pixeles[i..i + 4]);
        }
    }
    (w, h, v)
}

/// **Abre el minichat** de la pegatina `id`, junto a `cerca` (el centro del
/// redondel, en el escritorio). No vuelve hasta que se cierra.
pub(crate) fn abrir(
    peg: &mut Pegador,
    id: &str,
    cerca: (f32, f32),
    dueno: isize,
    atajo: Option<Adjunto>,
) -> Option<Pedido> {
    let punto = Punto {
        x: cerca.0 as i32,
        y: cerca.1 as i32,
    };
    let (_, trabajo) = pindesk_shell::overlay::monitor_de(punto);
    let e = escala_de_monitor(punto);
    let aire = AIRE * e;
    let tarjeta = ui::sitio_del_minichat(
        cerca,
        Caja {
            x: trabajo.x as f32 + aire,
            y: trabajo.y as f32 + aire,
            ancho: trabajo.ancho as f32 - 2.0 * aire,
            alto: trabajo.alto as f32 - 2.0 * aire,
        },
        e,
    );
    let marco = Rect {
        x: (tarjeta.x - aire) as i32,
        y: (tarjeta.y - aire) as i32,
        ancho: (tarjeta.ancho + 2.0 * aire) as u32,
        alto: (tarjeta.alto + 2.0 * aire) as u32,
    };
    let ventana = VentanaOverlay::nueva(marco).ok()?;
    if dueno != 0 {
        ventana.poner_dueno(HWND(dueno as *mut _));
    }
    ventana.aceptar_ficheros(true);
    let dispositivo = pindesk_capture::Dispositivo::nuevo().ok()?;
    let motor = MotorRender::nuevo(dispositivo.d3d()).ok()?;
    let superficie = Superficie::nueva(
        &motor,
        dispositivo.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .ok()?;
    ventana.mostrar();
    ventana.enfocar();
    let mut st = Estado {
        id: id.to_string(),
        caja: CajaLinea::nueva().en_varias_lineas().sin_parpadeo(),
        subido: 0.0,
        tope: 0.0,
        zonas: Vec::new(),
        raton: (-1.0, -1.0),
        miniaturas: HashMap::new(),
        atajos: None,
        pedido: None,
        aviso: None,
    };
    let mut area = marco;
    let mut sucio = true;
    'bucle: loop {
        pindesk_shell::overlay::bombear_pendientes();
        ventana.seguir_sitio(&mut area);
        for (hwnd, ev) in pindesk_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                // Un clic en el visor cierra el minichat, como un globo.
                if matches!(
                    ev,
                    EventoOverlay::BotonPulsado(_) | EventoOverlay::BotonDerechoPulsado(_)
                ) {
                    break 'bucle;
                }
                continue;
            }
            let local = |p: &Punto| ((p.x - area.x) as f32 - aire, (p.y - area.y) as f32 - aire);
            match ev {
                EventoOverlay::Cerrar => break 'bucle,
                EventoOverlay::Pintar | EventoOverlay::CambioDpi => sucio = true,
                EventoOverlay::Tecla { vk: VK_ESCAPE, .. } => {
                    if st.atajos.take().is_some() {
                        sucio = true;
                    } else {
                        break 'bucle;
                    }
                }
                EventoOverlay::Tecla {
                    vk: VK_ENTRAR,
                    shift: false,
                    ctrl: false,
                    ..
                } => {
                    enviar(peg, &mut st, Vec::new());
                    sucio = true;
                }
                EventoOverlay::Tecla {
                    vk: VK_V,
                    ctrl: true,
                    shift: false,
                    ..
                } => {
                    if !pegar(peg, &mut st, e) {
                        st.caja.tecla(VK_V, false, true, &mut PortapapelesWindows);
                    }
                    sucio = true;
                }
                EventoOverlay::Tecla {
                    vk, shift, ctrl, ..
                } => {
                    if st
                        .caja
                        .tecla(vk, shift, ctrl, &mut PortapapelesWindows)
                        .atendida()
                    {
                        sucio = true;
                    }
                }
                EventoOverlay::Caracter(c) => {
                    if pindesk_shell::overlay::es_letra_de_wm_char(c) && st.caja.caracter(c) {
                        sucio = true;
                    }
                }
                EventoOverlay::FicherosSoltados => {
                    for r in pindesk_shell::overlay::ficheros_soltados() {
                        adjuntar(peg, &mut st, &r);
                    }
                    sucio = true;
                }
                EventoOverlay::Rueda(d) => {
                    st.subido = (st.subido + d as f32 / 120.0 * 48.0 * e).clamp(0.0, st.tope);
                    sucio = true;
                }
                EventoOverlay::RatonMovido(p) => {
                    st.raton = local(&p);
                    st.caja.arrastrar(Punto {
                        x: st.raton.0 as i32,
                        y: st.raton.1 as i32,
                    });
                    // Lo que hay bajo el raton se ilumina: siempre se repinta.
                    sucio = true;
                }
                EventoOverlay::BotonSoltado(_) => {
                    st.caja.soltar_raton();
                }
                EventoOverlay::BotonDerechoPulsado(p) => {
                    let q = local(&p);
                    if let Some(Accion::Entrada(eid)) = accion_en(&st, q, true)
                        && peg
                            .pegatinas
                            .get(&st.id)
                            .and_then(|c| c.entrada(&eid))
                            .is_some_and(|x| x.aparato == peg.quien.aparato)
                    {
                        peg.pegatinas.quitar_entrada(&st.id, &eid, ahora());
                        peg.guardar();
                        sucio = true;
                    }
                }
                EventoOverlay::BotonPulsado(p) => {
                    let q = local(&p);
                    st.raton = q;
                    let l = Punto {
                        x: q.0 as i32,
                        y: q.1 as i32,
                    };
                    let shift = pindesk_shell::entrada::modificadores_pulsados().shift;
                    if st.caja.pulsar(l, shift, Instant::now()) {
                        sucio = true;
                        continue;
                    }
                    let Some(a) = accion_en(&st, q, false) else {
                        continue;
                    };
                    sucio = true;
                    if pulsar(peg, &mut st, a, &ventana, &atajo) {
                        break 'bucle;
                    }
                }
                _ => {}
            }
        }
        if peg.al_dia() {
            sucio = true;
        }
        // La pegatina se quito (aqui o en otro aparato): nada que ensenar.
        if peg.pegatinas.get(&st.id).is_none() {
            break;
        }
        if sucio {
            sucio = false;
            pintar(
                &motor,
                &superficie,
                peg,
                &mut st,
                marco.ancho,
                marco.alto,
                e,
            );
            ventana.validar();
        }
        pindesk_shell::overlay::esperar_eventos(Some(500));
    }
    ventana.ocultar();
    peg.guardar();
    st.pedido
}

fn escala_de_monitor(p: Punto) -> f32 {
    pindesk_capture::enumerar_monitores()
        .ok()
        .and_then(|d| d.monitor_en(p).map(|m| m.escala_por_cien as f32 / 100.0))
        .unwrap_or(1.0)
        .max(0.5)
}

fn accion_en(st: &Estado, q: (f32, f32), con_entradas: bool) -> Option<Accion> {
    st.zonas
        .iter()
        .rev()
        .filter(|(_, a)| con_entradas || !matches!(a, Accion::Entrada(_)))
        .find(|(r, _)| q.0 >= r.x && q.1 >= r.y && q.0 < r.x + r.ancho && q.1 < r.y + r.alto)
        .map(|(_, a)| a.clone())
}

/// Manda lo escrito (y `adjuntos`). Devuelve si mando algo.
fn enviar(peg: &mut Pegador, st: &mut Estado, adjuntos: Vec<Adjunto>) -> bool {
    let texto = st.caja.texto().to_string();
    let quien = peg.quien.clone();
    if peg
        .pegatinas
        .escribir(&st.id, &quien, ahora(), &texto, adjuntos)
        .is_none()
    {
        return false;
    }
    st.caja.vaciar();
    st.subido = 0.0;
    peg.guardar();
    true
}

/// Adjunta un fichero como entrada (con lo escrito, si habia algo).
fn adjuntar(peg: &mut Pegador, st: &mut Estado, ruta: &std::path::Path) {
    let Some(s) = &peg.sitio else {
        st.aviso = Some(textos().t("comentario-sin-sitio"));
        return;
    };
    match s.adjuntar(ruta) {
        Ok(a) => {
            enviar(peg, st, vec![a]);
        }
        Err(e) => tracing::warn!(?e, ruta = %ruta.display(), "no se pudo adjuntar al comentario"),
    }
}

/// **Ctrl+V**: una imagen o ficheros copiados van como entradas. `false`
/// si no habia nada de eso (el texto lo pega la caja).
fn pegar(peg: &mut Pegador, st: &mut Estado, _e: f32) -> bool {
    use pindesk_codec::portapapeles::ContenidoPortapapeles as C;
    match pindesk_codec::portapapeles::leer() {
        Some(C::Imagen(img)) => {
            let (Some(s), Ok(png)) = (&peg.sitio, pindesk_codec::imagen::codificar_png(&img))
            else {
                return true;
            };
            match s.adjuntar_bytes(&png, "pegada.png") {
                Ok(a) => {
                    enviar(peg, st, vec![a]);
                }
                Err(e) => tracing::warn!(?e, "no se pudo pegar la imagen en el comentario"),
            }
            true
        }
        Some(C::Rutas(rutas)) => {
            for r in rutas {
                adjuntar(peg, st, &r);
            }
            true
        }
        _ => false,
    }
}

/// Lo que hace un clic. Devuelve si hay que cerrar el minichat.
fn pulsar(
    peg: &mut Pegador,
    st: &mut Estado,
    a: Accion,
    ventana: &VentanaOverlay,
    atajo: &Option<Adjunto>,
) -> bool {
    let cuando = ahora();
    match a {
        Accion::Boton(BotonChat::Cerrar) => return true,
        Accion::Boton(BotonChat::Resolver) => {
            let si = !peg.pegatinas.get(&st.id).is_some_and(|c| c.resuelto);
            if peg.pegatinas.resolver(&st.id, si, cuando) {
                peg.guardar();
            }
        }
        Accion::Boton(BotonChat::Tarea) => {
            if peg.crear_tarea(&st.id) {
                peg.guardar();
                st.aviso = Some(textos().t("comentario-tarea-creada"));
            }
        }
        Accion::Boton(BotonChat::Enviar) => {
            enviar(peg, st, Vec::new());
        }
        Accion::Boton(BotonChat::Adjuntar) => {
            for r in pindesk_shell::elegir::pedir_ficheros(ventana.handle()) {
                adjuntar(peg, st, &r);
            }
        }
        Accion::Boton(BotonChat::Pegar) => {
            if !pegar(peg, st, 1.0) {
                st.caja.tecla(VK_V, false, true, &mut PortapapelesWindows);
            }
        }
        Accion::Boton(BotonChat::Atajo) => {
            st.atajos = if st.atajos.is_some() {
                None
            } else {
                Some(opciones_de_atajo(peg, atajo))
            };
        }
        Accion::Boton(BotonChat::Lienzo) => {
            let Some(s) = peg.sitio.clone() else {
                st.aviso = Some(textos().t("comentario-sin-sitio"));
                return false;
            };
            let quien = peg.quien.clone();
            let nueva = peg.pegatinas.id_nuevo(&quien.aparato, cuando);
            let tinta = s.lienzo_nuevo(&nueva);
            if peg
                .pegatinas
                .escribir(&st.id, &quien, cuando, "", vec![Adjunto::lienzo(&tinta)])
                .is_some()
            {
                peg.guardar();
                super::lienzo::abrir_tinta(None, s.ruta_real(&tinta));
            }
        }
        Accion::Elegir(i) => {
            if let Some(o) = st.atajos.take().and_then(|v| v.into_iter().nth(i)) {
                enviar(peg, st, vec![o.adjunto]);
            }
        }
        Accion::Imagen(eid, k) => {
            let Some(s) = peg.sitio.clone() else {
                return false;
            };
            let Some(a) = peg
                .pegatinas
                .get(&st.id)
                .and_then(|c| c.entrada(&eid))
                .and_then(|x| x.adjuntos.get(k))
                .cloned()
            else {
                return false;
            };
            let tinta = match a.tinta {
                Some(t) => t,
                None => {
                    let t = s.lienzo_nuevo(&format!("{eid}-{k}"));
                    peg.pegatinas.poner_tinta(&st.id, &eid, k, &t, cuando);
                    peg.guardar();
                    t
                }
            };
            let imagen = a.ruta.map(|r| s.ruta_real(&r));
            super::lienzo::abrir_tinta(imagen, s.ruta_real(&tinta));
        }
        Accion::Lienzo(eid, k) => {
            let Some(s) = peg.sitio.clone() else {
                return false;
            };
            if let Some(t) = peg
                .pegatinas
                .get(&st.id)
                .and_then(|c| c.entrada(&eid))
                .and_then(|x| x.adjuntos.get(k))
                .and_then(|a| a.tinta.clone())
            {
                super::lienzo::abrir_tinta(None, s.ruta_real(&t));
            }
        }
        Accion::Archivo { ruta, nombre } => {
            let Some(s) = &peg.sitio else {
                return false;
            };
            abrir_fichero(&s.ruta_real(&ruta), &nombre);
        }
        Accion::Mensaje { proyecto, mensaje } => {
            if !crate::ventana_chat::pedir_ir_a(proyecto, mensaje) {
                st.aviso = Some(textos().t("comentario-abre-el-chat"));
            }
        }
        Accion::Elemento { lienzo, punto } => {
            st.pedido = Some(Pedido::Elemento { lienzo, punto });
            return true;
        }
        Accion::Entrada(_) => {}
    }
    false
}

/// Abre un fichero adjunto con lo de Windows: una copia con su nombre de
/// verdad (el adjunto guardado no lleva extension).
fn abrir_fichero(ruta: &std::path::Path, nombre: &str) {
    let carpeta = std::env::temp_dir().join("pindesk-comentarios");
    let limpio: String = nombre
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    let copia = carpeta.join(if limpio.trim().is_empty() {
        "adjunto".to_string()
    } else {
        limpio
    });
    let hecho = std::fs::create_dir_all(&carpeta)
        .and_then(|()| std::fs::copy(ruta, &copia).map(|_| ()))
        .map_err(|e| e.to_string())
        .and_then(|()| pindesk_shell::abrir(&copia).map_err(|e| e.to_string()));
    if let Err(e) = hecho {
        tracing::warn!(%e, ruta = %ruta.display(), "no se pudo abrir el adjunto del comentario");
    }
}

/// Lo que se puede poner de atajo: el elemento elegido en el lienzo (si lo
/// hay) y los ultimos mensajes del chat del documento.
fn opciones_de_atajo(peg: &Pegador, atajo: &Option<Adjunto>) -> Vec<Opcion> {
    let mut v = Vec::new();
    if let Some(a) = atajo {
        v.push(Opcion {
            rotulo: format!(
                "{} \u{2014} {}",
                textos().t("comentario-elemento"),
                a.rotulo.clone().unwrap_or_default()
            ),
            adjunto: a.clone(),
        });
    }
    let Some(s) = &peg.sitio else {
        return v;
    };
    let (Some(raiz), Some(ficha)) = (s.raiz(), s.ficha()) else {
        return v;
    };
    let Ok(c) = pindesk_proyecto::cuaderno::Cuaderno::leer_de(&pindesk_proyecto::almacen::carpeta(
        raiz, &ficha,
    )) else {
        return v;
    };
    for m in c.mensajes.iter().rev().take(25) {
        let r = m.resumen();
        let r = if r.trim().is_empty() {
            m.nombre.clone()
        } else {
            r
        };
        if r.trim().is_empty() {
            continue;
        }
        let r: String = r
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .take(70)
            .collect();
        v.push(Opcion {
            adjunto: Adjunto::mensaje(&ficha, &m.codigo_unico(), &r),
            rotulo: r,
        });
    }
    v
}

fn icono_de(b: BotonChat) -> &'static Icono {
    match b {
        BotonChat::Resolver => &material::CHECK_CIRCLE,
        BotonChat::Tarea => &material::CHECKLIST,
        BotonChat::Cerrar => &material::CLOSE,
        BotonChat::Adjuntar => &material::ATTACH_FILE,
        BotonChat::Pegar => &material::CONTENT_PASTE,
        BotonChat::Atajo => &material::LINK,
        BotonChat::Lienzo => &material::DRAW,
        BotonChat::Enviar => &material::SEND,
    }
}

fn dentro(r: RectF, p: (f32, f32)) -> bool {
    p.0 >= r.x && p.1 >= r.y && p.0 < r.x + r.ancho && p.1 < r.y + r.alto
}

fn icono_en(p: &Pintor<'_>, i: &Icono, r: RectF, lado: f32, color: Color) {
    p.icono(
        i,
        RectF {
            x: r.x + (r.ancho - lado) / 2.0,
            y: r.y + (r.alto - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        color,
    );
}

#[allow(clippy::too_many_arguments)] // lo que se pinta y donde
fn pintar(
    motor: &MotorRender,
    superficie: &Superficie,
    peg: &Pegador,
    st: &mut Estado,
    ancho: u32,
    alto: u32,
    e: f32,
) {
    let Some(c) = peg.pegatinas.get(&st.id).cloned() else {
        return;
    };
    let Ok(destino) = superficie.empezar(motor) else {
        return;
    };
    // Las miniaturas que falten, antes de pintar.
    for x in c.entradas() {
        for a in &x.adjuntos {
            if a.tipo != TipoAdjunto::Imagen {
                continue;
            }
            let (Some(s), Some(r)) = (&peg.sitio, &a.ruta) else {
                continue;
            };
            let real = s.ruta_real(r);
            st.miniaturas.entry(real.clone()).or_insert_with(|| {
                let img = pindesk_codec::cargar(&real).ok()?;
                let (w, h, px) = reducida(&img);
                motor
                    .bitmap_desde_pixeles_premultiplicado(w, h, &px)
                    .ok()
                    .map(|b| (b, w, h))
            });
        }
    }
    let aire = AIRE * e;
    let (w, h) = (ancho as f32 - 2.0 * aire, alto as f32 - 2.0 * aire);
    let d = MiniChat::colocar(
        w,
        h,
        e,
        alto_de_la_caja(motor, &st.caja, w, e),
        lista::se_puede_crear_tarea(&c) || c.tarea.is_some(),
    );
    let mut zonas: Vec<(RectF, Accion)> = Vec::new();
    let _ = motor.dibujar(&destino, |p| {
        p.limpiar_transparente();
        p.desplazar(aire, aire);
        let tarjeta = Caja {
            x: 0.0,
            y: 0.0,
            ancho: w,
            alto: h,
        };
        pintar::isla(p, tarjeta, e);
        // La cabecera, con una franja del color del tipo.
        p.con_recorte(rf(d.cabecera), |p| {
            p.rellenar_redondeado(
                rf(d.cabecera),
                10.0 * e,
                con_alfa(pintar::color(c.tipo), 0.14),
            );
        });
        pintar::pegatina(
            p,
            c.tipo,
            (
                d.pegatina.x + d.pegatina.ancho / 2.0,
                d.pegatina.abajo() + 2.0 * e,
            ),
            e * 0.9,
            c.resuelto,
            0,
            false,
            1.0,
        );
        let mut titulo = textos().t(c.tipo.clave());
        if c.resuelto {
            titulo = format!("{titulo} \u{00b7} {}", textos().t("comentario-resuelto"));
        }
        let x0 = d.pegatina.derecha() + 10.0 * e;
        let tope = d
            .botones
            .iter()
            .filter(|(b, _)| b.y < d.cabecera.abajo())
            .map(|(b, _)| b.x)
            .fold(w, f32::min);
        p.texto_linea(&titulo, x0, 8.0 * e, 14.0 * e, tope - x0 - 6.0 * e, TINTA);
        let doc = peg.sitio.as_ref().map(|s| s.nombre.as_str()).unwrap_or("");
        p.texto_linea(doc, x0, 27.0 * e, 11.0 * e, tope - x0 - 6.0 * e, APAGADO);
        // El hilo.
        p.con_recorte(rf(d.hilo), |p| {
            hilo(p, peg, st, &c, &d, e, &mut zonas);
        });
        p.linea((0.0, d.barra.y), (w, d.barra.y), 1.0 * e, RAYA);
        // La caja de escribir.
        p.rellenar_redondeado(rf(d.caja_texto), 8.0 * e, CAJA_FONDO);
        let hueco = RectF {
            x: d.caja_texto.x + 8.0 * e,
            y: d.caja_texto.y + 4.0 * e,
            ancho: d.caja_texto.ancho - 16.0 * e,
            alto: d.caja_texto.alto - 8.0 * e,
        };
        st.caja.pintar(
            p,
            rf(d.caja_texto),
            hueco,
            13.0 * e,
            &textos().t("comentario-escribir"),
            &Colores {
                texto: TINTA,
                invitacion: APAGADO,
                seleccion: SELECCION,
                rayita: TINTA,
            },
            true,
            Instant::now(),
        );
        // Los botones.
        for (caja, b) in &d.botones {
            let r = rf(*caja);
            let encendido = match b {
                BotonChat::Resolver => c.resuelto,
                BotonChat::Tarea => c.tarea.is_some(),
                BotonChat::Atajo => st.atajos.is_some(),
                _ => false,
            };
            if encendido || dentro(r, st.raton) {
                p.rellenar_redondeado(r, 8.0 * e, if encendido { pintar::ACTIVO } else { ENCIMA });
            }
            let color = match b {
                BotonChat::Enviar if !st.caja.texto().trim().is_empty() => pintar::color(c.tipo),
                BotonChat::Resolver if c.resuelto => pintar::VERDE,
                BotonChat::Tarea if c.tarea.is_some() => pintar::VERDE,
                _ => TINTA,
            };
            icono_en(p, icono_de(*b), r, 20.0 * e, color);
            zonas.push((r, Accion::Boton(*b)));
        }
        // La lista de atajos, encima del hilo.
        if let Some(op) = &st.atajos {
            let panel = rf(d.hilo);
            p.rellenar(panel, ISLA);
            p.texto(
                &textos().t("comentario-elegir-atajo"),
                12.0 * e,
                panel.y + 8.0 * e,
                12.0 * e,
                APAGADO,
            );
            if op.is_empty() {
                p.texto_ajustado(
                    &textos().t("comentario-sin-atajos"),
                    12.0 * e,
                    panel.y + 32.0 * e,
                    12.5 * e,
                    w - 24.0 * e,
                    TINTA,
                );
            }
            let fila = 30.0 * e;
            for (i, o) in op.iter().enumerate() {
                let r = RectF {
                    x: 0.0,
                    y: panel.y + 28.0 * e + i as f32 * fila,
                    ancho: w,
                    alto: fila,
                };
                if r.y + r.alto > panel.y + panel.alto {
                    break;
                }
                if dentro(r, st.raton) {
                    p.rellenar(r, ENCIMA);
                }
                let icono = if o.adjunto.tipo == TipoAdjunto::Elemento {
                    &material::DRAW
                } else {
                    &material::FORUM
                };
                icono_en(
                    p,
                    icono,
                    RectF {
                        x: 10.0 * e,
                        y: r.y,
                        ancho: 20.0 * e,
                        alto: fila,
                    },
                    16.0 * e,
                    APAGADO,
                );
                p.texto_linea(
                    &o.rotulo,
                    38.0 * e,
                    r.y + 7.0 * e,
                    12.5 * e,
                    w - 50.0 * e,
                    TINTA,
                );
                zonas.push((r, Accion::Elegir(i)));
            }
        }
        // Un aviso corto, abajo del hilo.
        if let Some(a) = &st.aviso {
            let tam = 11.5 * e;
            let (tw, th) = p.medir_texto(a, tam);
            let r = RectF {
                x: (w - tw) / 2.0 - 10.0 * e,
                y: d.hilo.abajo() - th - 18.0 * e,
                ancho: tw + 20.0 * e,
                alto: th + 10.0 * e,
            };
            p.rellenar_redondeado(r, 6.0 * e, con_alfa(TINTA, 0.85));
            p.texto(a, r.x + 10.0 * e, r.y + 5.0 * e, tam, Color::BLANCO);
        }
    });
    let _ = superficie.presentar();
    st.zonas = zonas;
}

fn alto_de_la_caja(motor: &MotorRender, caja: &CajaLinea, ancho: f32, e: f32) -> f32 {
    let t = caja.texto();
    if t.is_empty() {
        return 30.0 * e;
    }
    let (_, h) = motor.medir_texto_ajustado(t, 13.0 * e, (ancho - 80.0 * e).max(40.0));
    h + 12.0 * e
}

fn hora(cuando: i64) -> String {
    crate::recordatorios::cuando_legible(
        pindesk_shell::entorno::a_local(cuando),
        pindesk_shell::entorno::ahora_local_ms(),
    )
}

/// **El hilo**: cada entrada con quien, cuando, lo escrito y lo adjuntado.
/// Se pinta de abajo arriba desde lo ultimo, como un chat.
#[allow(clippy::too_many_arguments)] // lo que se pinta y donde
fn hilo(
    p: &Pintor<'_>,
    peg: &Pegador,
    st: &mut Estado,
    c: &Comentario,
    d: &MiniChat,
    e: f32,
    zonas: &mut Vec<(RectF, Accion)>,
) {
    let entradas: Vec<&Entrada> = c.entradas().collect();
    let ancho = d.hilo.ancho - 28.0 * e;
    if entradas.is_empty() {
        p.texto_ajustado(
            &textos().t("comentario-sin-entradas"),
            14.0 * e,
            d.hilo.y + 16.0 * e,
            13.0 * e,
            ancho,
            APAGADO,
        );
        st.tope = 0.0;
        return;
    }
    // Lo que mide cada entrada.
    let medir = |x: &Entrada| -> f32 {
        let mut h = 20.0 * e;
        if !x.texto.is_empty() {
            h += p.medir_texto_ajustado(&x.texto, 13.0 * e, ancho).1 + 4.0 * e;
        }
        for a in &x.adjuntos {
            h += alto_de_adjunto(peg, st, a, ancho, e) + 6.0 * e;
        }
        h + 12.0 * e
    };
    let altos: Vec<f32> = entradas.iter().map(|x| medir(x)).collect();
    let total: f32 = altos.iter().sum::<f32>() + 8.0 * e;
    st.tope = ui::tope_del_hilo(total, d.hilo.alto);
    st.subido = st.subido.min(st.tope);
    let mut y = d.hilo.y + 8.0 * e - (st.tope - st.subido);
    for (x, alto) in entradas.iter().zip(altos) {
        if y + alto < d.hilo.y || y > d.hilo.abajo() {
            y += alto;
            continue;
        }
        let caja_entrada = RectF {
            x: 0.0,
            y,
            ancho: d.hilo.ancho,
            alto,
        };
        zonas.push((caja_entrada, Accion::Entrada(x.id.clone())));
        let mio = x.aparato == peg.quien.aparato;
        let quien = if x.autor.is_empty() {
            "?".to_string()
        } else {
            x.autor.clone()
        };
        p.texto_linea(
            &quien,
            14.0 * e,
            y,
            12.0 * e,
            ancho * 0.6,
            if mio { pintar::color(c.tipo) } else { TINTA },
        );
        let (qw, _) = p.medir_texto(&quien, 12.0 * e);
        p.texto_linea(
            &format!("\u{00b7} {}", hora(x.cuando)),
            14.0 * e + qw.min(ancho * 0.6) + 6.0 * e,
            y + 1.0 * e,
            11.0 * e,
            ancho * 0.4,
            APAGADO,
        );
        let mut yy = y + 20.0 * e;
        if !x.texto.is_empty() {
            p.texto_ajustado(&x.texto, 14.0 * e, yy, 13.0 * e, ancho, TINTA);
            yy += p.medir_texto_ajustado(&x.texto, 13.0 * e, ancho).1 + 4.0 * e;
        }
        for (k, a) in x.adjuntos.iter().enumerate() {
            let h = alto_de_adjunto(peg, st, a, ancho, e);
            let r = pintar_adjunto(p, peg, st, a, 14.0 * e, yy, ancho, e);
            if let Some(accion) = accion_de(&x.id, k, a) {
                zonas.push((r, accion));
            }
            yy += h + 6.0 * e;
        }
        y += alto;
    }
}

fn accion_de(entrada: &str, k: usize, a: &Adjunto) -> Option<Accion> {
    Some(match a.tipo {
        TipoAdjunto::Imagen => Accion::Imagen(entrada.to_string(), k),
        TipoAdjunto::Lienzo => Accion::Lienzo(entrada.to_string(), k),
        TipoAdjunto::Archivo => Accion::Archivo {
            ruta: a.ruta.clone()?,
            nombre: a.nombre.clone().unwrap_or_default(),
        },
        TipoAdjunto::Mensaje => Accion::Mensaje {
            proyecto: a.chat.clone()?,
            mensaje: a.mensaje.clone()?,
        },
        TipoAdjunto::Elemento => Accion::Elemento {
            lienzo: a.lienzo.clone().unwrap_or_default(),
            punto: a.punto,
        },
        TipoAdjunto::Otro(_) => return None,
    })
}

fn miniatura<'a>(
    peg: &Pegador,
    st: &'a Estado,
    a: &Adjunto,
) -> Option<&'a (ID2D1Bitmap1, u32, u32)> {
    let s = peg.sitio.as_ref()?;
    st.miniaturas
        .get(&s.ruta_real(a.ruta.as_deref()?))?
        .as_ref()
}

fn tamano_de_miniatura(w: u32, h: u32, ancho: f32, e: f32) -> (f32, f32) {
    let max_w = ancho.min(220.0 * e);
    let max_h = 150.0 * e;
    let k = (max_w / w.max(1) as f32).min(max_h / h.max(1) as f32);
    (w as f32 * k, h as f32 * k)
}

fn alto_de_adjunto(peg: &Pegador, st: &Estado, a: &Adjunto, ancho: f32, e: f32) -> f32 {
    match (a.tipo == TipoAdjunto::Imagen, miniatura(peg, st, a)) {
        (true, Some((_, w, h))) => tamano_de_miniatura(*w, *h, ancho, e).1,
        _ => 30.0 * e,
    }
}

/// Pinta un adjunto en (x, y) y devuelve su caja.
#[allow(clippy::too_many_arguments)] // lo que se pinta y donde
fn pintar_adjunto(
    p: &Pintor<'_>,
    peg: &Pegador,
    st: &Estado,
    a: &Adjunto,
    x: f32,
    y: f32,
    ancho: f32,
    e: f32,
) -> RectF {
    if a.tipo == TipoAdjunto::Imagen
        && let Some((b, w, h)) = miniatura(peg, st, a)
    {
        let (mw, mh) = tamano_de_miniatura(*w, *h, ancho, e);
        let r = RectF {
            x,
            y,
            ancho: mw,
            alto: mh,
        };
        p.bitmap(b, r, None, false);
        p.trazar(r, 1.0 * e, RAYA);
        // «Anotar», abajo a la derecha de la imagen.
        let rot = textos().t(if a.tinta.is_some() {
            "comentario-anotada"
        } else {
            "comentario-anotar"
        });
        let tam = 11.0 * e;
        let (tw, th) = p.medir_texto(&rot, tam);
        let chapa = RectF {
            x: r.x + r.ancho - tw - 34.0 * e,
            y: r.y + r.alto - th - 14.0 * e,
            ancho: tw + 28.0 * e,
            alto: th + 8.0 * e,
        };
        p.rellenar_redondeado(chapa, 6.0 * e, con_alfa(TINTA, 0.78));
        icono_en(
            p,
            &material::DRAW,
            RectF {
                x: chapa.x + 4.0 * e,
                y: chapa.y,
                ancho: 16.0 * e,
                alto: chapa.alto,
            },
            12.0 * e,
            Color::BLANCO,
        );
        p.texto(
            &rot,
            chapa.x + 22.0 * e,
            chapa.y + 4.0 * e,
            tam,
            Color::BLANCO,
        );
        return r;
    }
    let (icono, rotulo): (&Icono, String) = match &a.tipo {
        TipoAdjunto::Imagen => (&material::IMAGE, a.nombre.clone().unwrap_or_default()),
        TipoAdjunto::Archivo => (&material::DESCRIPTION, a.nombre.clone().unwrap_or_default()),
        TipoAdjunto::Mensaje => (
            &material::FORUM,
            a.rotulo
                .clone()
                .unwrap_or_else(|| textos().t("comentario-mensaje")),
        ),
        TipoAdjunto::Elemento => (
            &material::LINK,
            format!(
                "{} {}",
                textos().t("comentario-elemento"),
                a.rotulo.clone().unwrap_or_default()
            ),
        ),
        TipoAdjunto::Lienzo => (&material::DRAW, textos().t("comentario-lienzo-propio")),
        TipoAdjunto::Otro(s) => (&material::DESCRIPTION, s.clone()),
    };
    let tam = 12.0 * e;
    let (tw, _) = p.medir_texto(&rotulo, tam);
    let r = RectF {
        x,
        y,
        ancho: (tw + 44.0 * e).min(ancho),
        alto: 30.0 * e,
    };
    p.rellenar_redondeado(
        r,
        8.0 * e,
        if dentro(r, st.raton) {
            ENCIMA
        } else {
            CAJA_FONDO
        },
    );
    icono_en(
        p,
        icono,
        RectF {
            x: r.x + 6.0 * e,
            y: r.y,
            ancho: 20.0 * e,
            alto: r.alto,
        },
        16.0 * e,
        TINTA,
    );
    p.texto_linea(
        &rotulo,
        r.x + 32.0 * e,
        r.y + 7.0 * e,
        tam,
        r.ancho - 40.0 * e,
        TINTA,
    );
    r
}
