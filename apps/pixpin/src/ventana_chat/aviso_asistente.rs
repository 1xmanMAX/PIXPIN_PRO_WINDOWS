//! **El aviso flotante del asistente** (13-oct): lo que contesta el
//! asistente cuando se le pide desde Flow Launcher (`p ia …`) o lo que hizo
//! una automatizacion, sin abrir el chat.
//!
//! Una tarjeta pequena abajo a la derecha, siempre encima, que **no roba el
//! foco** (se esta trabajando en otra cosa): primero «💭 …» y, al acabar, la
//! respuesta corta con **«Abrir»** (el chat «Asistente» en esa respuesta) y
//! un aspa. Se va sola a los [`DURA`] de acabar, salvo con el raton encima.
//! No es el globo de Windows: ese lo silencian la concentracion y el modo
//! presentacion (ver `pixpin_shell::aviso`), y no tiene botones.
//!
//! El que lo saca se queda un [`Mando`] para poner la respuesta cuando la
//! tenga; la ventana lo mira en cada vuelta de su bucle.

#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicIsize, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_geom::{Punto, Rect};
use pixpin_render::{Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::Ubicacion;

use super::{OSCURO, OpcionesLienzo, mi};
use crate::overlay::Recursos;
use crate::ventanita::{Botones, dentro};

/// Lo que se queda en pantalla tras acabar.
pub const DURA: Duration = Duration::from_secs(15);
/// Lo mas que espera a la respuesta (un turno colgado no deja la tarjeta
/// para siempre).
const ESPERA_MAXIMA: Duration = Duration::from_secs(15 * 60);
const ANCHO: u32 = 340;
const ALTO: u32 = 178;
const VK_ESCAPE: u32 = 0x1B;
/// Lo que se ensena de la respuesta, como mucho.
const LETRAS: usize = 220;

/// Adonde lleva «Abrir»: el chat en esa respuesta.
#[derive(Clone)]
pub(crate) struct Abrir {
    pub idioma: pixpin_store::Idioma,
    pub ubicacion: Ubicacion,
    pub opciones: OpcionesLienzo,
    pub proyecto: String,
    /// El codigo del mensaje (`ir_a`).
    pub codigo: String,
}

/// Lo que dice la tarjeta.
#[derive(Clone)]
pub(crate) struct Aviso {
    pub titulo: String,
    pub texto: String,
    pub abrir: Option<Abrir>,
    /// Ya esta la respuesta: empieza a contar para irse.
    pub acabado: bool,
}

/// Para cambiar lo que dice una tarjeta ya abierta.
#[derive(Clone)]
pub(crate) struct Mando(Arc<Mutex<Aviso>>, Arc<AtomicIsize>);

impl Mando {
    /// Pone la respuesta (y «Abrir», si hay adonde).
    pub fn poner(
        &self,
        texto: String,
        abrir: Option<(pixpin_store::Idioma, Ubicacion, OpcionesLienzo, String, String)>,
    ) {
        if let Ok(mut a) = self.0.lock() {
            a.texto = texto;
            a.abrir = abrir.map(|(idioma, ubicacion, opciones, proyecto, codigo)| Abrir {
                idioma,
                ubicacion,
                opciones,
                proyecto,
                codigo,
            });
            a.acabado = true;
        }
        let h = self.1.load(Ordering::SeqCst);
        if h != 0 {
            pixpin_shell::overlay::despertar(h);
        }
    }
}

/// Cuantas hay abiertas (la nueva va encima de las otras).
static ABIERTAS: AtomicUsize = AtomicUsize::new(0);

/// **Saca una tarjeta**, en su propio hilo. Devuelve su mando.
pub(crate) fn lanzar(aviso: Aviso) -> Mando {
    let mando = Mando(Arc::new(Mutex::new(aviso)), Arc::new(AtomicIsize::new(0)));
    let m = mando.clone();
    let lanzado = std::thread::Builder::new().name("aviso-asistente".into()).spawn(move || {
        let _com = pixpin_shell::ComDelHilo::iniciar();
        let puesto = ABIERTAS.fetch_add(1, Ordering::SeqCst);
        if let Err(e) = crate::dispositivo_perdido::con_recursos("aviso del asistente", |r| flotar(r, &m, puesto)) {
            tracing::warn!(?e, "no se pudo sacar el aviso del asistente");
        }
        ABIERTAS.fetch_sub(1, Ordering::SeqCst);
    });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo del aviso del asistente");
    }
    mando
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Abrir,
    Cerrar,
}

/// Abajo a la derecha, encima de las que ya hubiera.
fn marco_inicial(area: Rect, escala_por_cien: u32, puesto: usize) -> Rect {
    let e = |v: u32| v * escala_por_cien / 100;
    let (w, h) = (e(ANCHO).min(area.ancho), e(ALTO).min(area.alto));
    let subir = (puesto.min(3) as u32) * (h + e(10));
    Rect {
        x: area.x + area.ancho as i32 - w as i32 - e(24) as i32,
        y: (area.y + area.alto as i32 - h as i32 - e(24) as i32 - subir as i32).max(area.y),
        ancho: w,
        alto: h,
    }
}

fn flotar(recursos: &Recursos, mando: &Mando, puesto: usize) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .monitor_en(pixpin_shell::posicion_del_cursor())
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien as f32 / 100.0;
    let marco = marco_inicial(monitor.area_trabajo, monitor.escala_por_cien, puesto);
    let ventana = VentanaOverlay::nueva(marco).context("no se pudo abrir el aviso")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(&motor, &recursos.d3d(), ventana.handle(), marco.ancho, marco.alto)
        .context("sin superficie para el aviso")?;
    // Sin robar el foco: se esta haciendo otra cosa.
    ventana.mostrar();
    ventana.traer_encima();
    mando.1.store(ventana.handle().0 as isize, Ordering::SeqCst);

    let nacio = Instant::now();
    let mut acabado_en: Option<Instant> = None;
    let mut botones: Botones<Accion> = Botones::default();
    let mut pulsado: Option<(Punto, Option<Accion>)> = None;
    let mut vivo = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        let mut accion = None;
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            match ev {
                EventoOverlay::Cerrar | EventoOverlay::Tecla { vk: VK_ESCAPE, .. } => vivo = false,
                EventoOverlay::RatonMovido(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                }
                EventoOverlay::BotonPulsado(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    pulsado = Some((p, botones.bajo_el_raton()));
                }
                EventoOverlay::BotonSoltado(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    if let Some((_, Some(a))) = pulsado.take()
                        && botones.bajo_el_raton() == Some(a)
                    {
                        accion = Some(a);
                    }
                }
                _ => {}
            }
        }
        let aviso = mando.0.lock().map(|a| a.clone()).ok();
        let Some(aviso) = aviso else { break };
        match accion {
            Some(Accion::Cerrar) => vivo = false,
            Some(Accion::Abrir) => {
                if let Some(a) = &aviso.abrir {
                    crate::ventana_chat::ir_a(
                        a.idioma,
                        a.ubicacion.clone(),
                        a.opciones,
                        a.proyecto.clone(),
                        Some(a.codigo.clone()),
                    );
                }
                vivo = false;
            }
            None => {}
        }
        // Con el raton encima no se va: se esta leyendo.
        let encima = botones.raton.0 >= 0.0
            && botones.raton.1 >= 0.0
            && botones.raton.0 <= marco.ancho as f32
            && botones.raton.1 <= marco.alto as f32;
        if aviso.acabado {
            let desde = *acabado_en.get_or_insert_with(Instant::now);
            if encima {
                acabado_en = Some(Instant::now());
            } else if desde.elapsed() >= DURA {
                vivo = false;
            }
        } else if nacio.elapsed() >= ESPERA_MAXIMA {
            vivo = false;
        }
        if !vivo {
            break;
        }
        if let Ok(d) = superficie.empezar(&motor) {
            let _ = motor.dibujar(&d, |p: &Pintor| pintar(p, marco, escala, &aviso, &mut botones));
            let _ = superficie.presentar();
        }
        pixpin_shell::overlay::esperar_eventos(Some(if aviso.acabado { 250 } else { 120 }));
    }
    ventana.ocultar();
    Ok(())
}

/// El texto que se ensena: corto, sin lineas en blanco de mas.
pub(crate) fn lo_que_se_ensena(texto: &str) -> String {
    let lineas: Vec<&str> = texto.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    crate::automatizaciones::recortar(&lineas.join("\n"), LETRAS)
}

fn pintar(p: &Pintor, marco: Rect, e: f32, aviso: &Aviso, botones: &mut Botones<Accion>) {
    let tema = &OSCURO;
    let (w, h) = (marco.ancho as f32, marco.alto as f32);
    p.limpiar_transparente();
    p.rellenar_redondeado(RectF { x: 0.0, y: 0.0, ancho: w, alto: h }, 16.0 * e, tema.isla_borde);
    p.rellenar_redondeado(RectF { x: 1.0, y: 1.0, ancho: w - 2.0, alto: h - 2.0 }, 15.0 * e, tema.isla);
    botones.vaciar();

    // Arriba: el titulo y el aspa.
    let aspa = RectF { x: w - 38.0 * e, y: 8.0 * e, ancho: 28.0 * e, alto: 28.0 * e };
    let tam = 14.0 * e;
    p.texto_linea(&aviso.titulo, 16.0 * e, 14.0 * e, tam, (aspa.x - 22.0 * e).max(0.0), tema.pildora_texto);
    if dentro(aspa, botones.raton) {
        p.rellenar_redondeado(aspa, aspa.alto / 2.0, tema.boton_sobre);
    }
    let l = 16.0 * e;
    p.icono(
        &mi::CLOSE,
        RectF { x: aspa.x + (aspa.ancho - l) / 2.0, y: aspa.y + (aspa.alto - l) / 2.0, ancho: l, alto: l },
        tema.campo_apagado,
    );
    botones.zona(aspa, Accion::Cerrar);

    // La respuesta.
    let texto = lo_que_se_ensena(&aviso.texto);
    let color = if aviso.acabado { tema.texto } else { tema.campo_apagado };
    p.texto_ajustado(&texto, 16.0 * e, 44.0 * e, 13.0 * e, w - 32.0 * e, color);

    // Abajo: «Abrir».
    if aviso.abrir.is_some() {
        let caja = RectF { x: w - 112.0 * e, y: h - 46.0 * e, ancho: 96.0 * e, alto: 32.0 * e };
        let fondo = if dentro(caja, botones.raton) { tema.boton_sobre } else { tema.campo };
        p.rellenar_redondeado(caja, 8.0 * e, fondo);
        let t = "Abrir";
        let (tw, th) = p.medir_texto(t, 14.0 * e);
        p.texto(t, caja.x + (caja.ancho - tw) / 2.0, caja.y + (caja.alto - th) / 2.0, 14.0 * e, tema.enviar);
        botones.zona(caja, Accion::Abrir);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_nueva_va_encima_de_las_que_hay_y_no_se_sale() {
        let area = Rect { x: 0, y: 0, ancho: 1920, alto: 1040 };
        let a = marco_inicial(area, 100, 0);
        let b = marco_inicial(area, 100, 1);
        assert_eq!(a.x, b.x);
        assert!(b.y < a.y);
        assert_eq!(a.y + a.alto as i32, 1040 - 24);
        // Caso negativo: muchas no se salen por arriba.
        let z = marco_inicial(Rect { x: 0, y: 0, ancho: 800, alto: 300 }, 100, 9);
        assert!(z.y >= 0);
    }

    #[test]
    fn lo_que_se_ensena_es_corto_y_sin_huecos() {
        assert_eq!(lo_que_se_ensena("  hola\n\n\n  que tal  "), "hola\nque tal");
        let largo = "a".repeat(1000);
        assert_eq!(lo_que_se_ensena(&largo).chars().count(), LETRAS + 1);
    }
}
