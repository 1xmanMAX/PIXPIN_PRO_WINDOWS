//! **El aviso flotante del asistente** (13-oct): lo que contesta el
//! asistente cuando se le pide desde Flow Launcher (`p ia …`) o lo que hizo
//! una automatizacion, sin abrir el chat.
//!
//! Una tarjeta pequena abajo a la derecha, siempre encima, que **no roba el
//! foco** (se esta trabajando en otra cosa): primero «💭 …» y, al acabar, la
//! respuesta con **«Abrir»** (el chat «Asistente» en esa respuesta) y un
//! aspa. Se va sola a los [`DURA`] de acabar, salvo con el raton encima.
//! No es el globo de Windows: ese lo silencian la concentracion y el modo
//! presentacion (ver `pixpin_shell::aviso`), y no tiene botones.
//!
//! **14-oct-2026** (lo vio el usuario con «¿qué tengo pendiente?» y 31
//! tareas): una respuesta larga ya no pasa por debajo de «Abrir». El pie
//! de los botones tiene su sitio reservado ([`medidas`]); la tarjeta crece
//! con el texto hasta [`ALTO_MAXIMO`] y lo que no cabe se lee con la rueda
//! (con su barrita). Sigue el tema claro u oscuro de Windows, como el chat.
//!
//! **Y avisa de una pregunta pendiente**: si el turno que espera esta
//! tarjeta pide permiso para algo que borra o envia
//! (`asistente::permiso`), la tarjeta lo dice («Necesito tu permiso: …»)
//! con «Sí», «No» y «Abrir» (el chat en esa burbuja). Lo seguro manda: el
//! aspa o Esc con la pregunta a la vista es un «No», y sin respuesta caduca
//! en no, como en el chat.
//!
//! El que lo saca se queda un [`Mando`] para poner la respuesta cuando la
//! tenga; la ventana lo mira en cada vuelta de su bucle.

#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicIsize, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_geom::{Punto, Rect};
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::Ubicacion;

use super::{CLARO, OSCURO, OpcionesLienzo, Tema, mi};
use crate::asistente::permiso::{self, Pregunta};
use crate::caja_dibujo::hex;
use crate::overlay::Recursos;
use crate::ventanita::{Botones, dentro};

/// Lo que se queda en pantalla tras acabar.
pub const DURA: Duration = Duration::from_secs(15);
/// Lo mas que espera a la respuesta (un turno colgado no deja la tarjeta
/// para siempre).
const ESPERA_MAXIMA: Duration = Duration::from_secs(15 * 60);
const ANCHO: u32 = 340;
/// Lo que mide sin apenas texto, y lo mas que crece (logicos): con mas, se
/// desplaza dentro.
const ALTO_MINIMO: u32 = 120;
const ALTO_MAXIMO: u32 = 420;
/// La paso que sube cada tarjeta sobre la anterior (la de antes era fija).
const PASO_ENTRE_TARJETAS: u32 = 178;
/// La cabecera (titulo y aspa), el pie con los botones y el margen de abajo
/// sin botones, en logicos.
const CABECERA: f32 = 44.0;
const PIE: f32 = 56.0;
const ABAJO: f32 = 14.0;
const TAM_TEXTO: f32 = 13.0;
const VK_ESCAPE: u32 = 0x1B;
/// Lo que se ensena de la respuesta, como mucho (lo demas, en el chat).
const LETRAS: usize = 2000;

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

/// Con que abrir el chat antes de tener respuesta (para llevar a la burbuja
/// de una pregunta).
#[derive(Clone)]
pub(crate) struct Chat {
    pub idioma: pixpin_store::Idioma,
    pub ubicacion: Ubicacion,
    pub opciones: OpcionesLienzo,
}

/// Lo que dice la tarjeta.
#[derive(Clone)]
pub(crate) struct Aviso {
    pub titulo: String,
    pub texto: String,
    pub abrir: Option<Abrir>,
    /// Ya esta la respuesta: empieza a contar para irse.
    pub acabado: bool,
    /// El chat, para «Abrir» en una pregunta pendiente. Sin el, la pregunta
    /// sale igual con «Sí»/«No».
    pub chat: Option<Chat>,
}

/// Para cambiar lo que dice una tarjeta ya abierta.
#[derive(Clone)]
pub(crate) struct Mando(Arc<Mutex<Aviso>>, Arc<AtomicIsize>);

impl Mando {
    /// Pone la respuesta (y «Abrir», si hay adonde).
    pub fn poner(
        &self,
        texto: String,
        abrir: Option<(
            pixpin_store::Idioma,
            Ubicacion,
            OpcionesLienzo,
            String,
            String,
        )>,
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

    /// Cambia lo que dice **sin darla por acabada** (sigue trabajando y no
    /// cuenta para irse): un turno que sigue con el chat cerrado y pregunta
    /// algo (`ventana_chat::asistente::de_fondo`).
    pub fn cambiar_texto(&self, texto: String) {
        if let Ok(mut a) = self.0.lock() {
            a.texto = texto;
        }
        let h = self.1.load(Ordering::SeqCst);
        if h != 0 {
            pixpin_shell::overlay::despertar(h);
        }
    }
}

/// Cuantas hay abiertas (la nueva va encima de las otras).
static ABIERTAS: AtomicUsize = AtomicUsize::new(0);
/// El numero de cada tarjeta, para quedarse con una pregunta.
static SIGUIENTE: AtomicU64 = AtomicU64::new(1);

/// **Saca una tarjeta**, en su propio hilo. Devuelve su mando.
pub(crate) fn lanzar(aviso: Aviso) -> Mando {
    let mando = Mando(Arc::new(Mutex::new(aviso)), Arc::new(AtomicIsize::new(0)));
    let m = mando.clone();
    let lanzado = std::thread::Builder::new()
        .name("aviso-asistente".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let puesto = ABIERTAS.fetch_add(1, Ordering::SeqCst);
            let id = SIGUIENTE.fetch_add(1, Ordering::SeqCst);
            if let Err(e) = crate::dispositivo_perdido::con_recursos("aviso del asistente", |r| {
                flotar(r, &m, puesto, id)
            }) {
                tracing::warn!(?e, "no se pudo sacar el aviso del asistente");
            }
            // Lo que ensenaba sin contestar, libre para otra tarjeta o el chat.
            permiso::soltar_la_tarjeta(id);
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
    Si,
    No,
}

/// Abajo a la derecha, encima de las que ya hubiera, con el alto minimo.
fn marco_inicial(area: Rect, escala_por_cien: u32, puesto: usize) -> Rect {
    let e = |v: u32| v * escala_por_cien / 100;
    let (w, h) = (e(ANCHO).min(area.ancho), e(ALTO_MINIMO).min(area.alto));
    let subir = (puesto.min(3) as u32) * (e(PASO_ENTRE_TARJETAS) + e(10));
    Rect {
        x: area.x + area.ancho as i32 - w as i32 - e(24) as i32,
        y: (area.y + area.alto as i32 - h as i32 - e(24) as i32 - subir as i32).max(area.y),
        ancho: w,
        alto: h,
    }
}

/// El mismo marco con otro alto, **creciendo hacia arriba** (el pie se
/// queda donde estaba) sin salirse del area por arriba.
fn con_alto(marco: Rect, alto: u32, area: Rect) -> Rect {
    let abajo = marco.y + marco.alto as i32;
    let alto = alto.min(area.alto);
    Rect {
        y: (abajo - alto as i32).max(area.y),
        alto,
        ..marco
    }
}

/// **Donde va cada cosa** en una tarjeta de `ancho` (pixeles) con un texto
/// que, ajustado, mide `alto_texto`: el alto de la tarjeta (entre el
/// minimo y `alto_maximo`), la zona del texto, que **acaba antes del pie**
/// de los botones, y lo que sobra del texto (se desplaza con la rueda).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Medidas {
    alto: f32,
    texto: RectF,
    /// La y de los botones, si hay pie.
    pie: Option<f32>,
    /// Lo que no cabe del texto (0 si cabe entero).
    sobra: f32,
}

fn medidas(ancho: f32, alto_texto: f32, con_pie: bool, alto_maximo: f32, e: f32) -> Medidas {
    let cabecera = CABECERA * e;
    let abajo = if con_pie { PIE * e } else { ABAJO * e };
    let minimo = ALTO_MINIMO as f32 * e;
    let alto = (cabecera + alto_texto + abajo)
        .clamp(minimo, alto_maximo.max(minimo))
        .ceil();
    let texto = RectF {
        x: 16.0 * e,
        y: cabecera,
        ancho: (ancho - 32.0 * e).max(1.0),
        alto: (alto - cabecera - abajo).max(0.0),
    };
    Medidas {
        alto,
        texto,
        pie: con_pie.then_some(alto - 46.0 * e),
        sobra: (alto_texto - texto.alto).max(0.0),
    }
}

/// Lo que se pinta en un fotograma.
struct Vista<'a> {
    titulo: &'a str,
    texto: &'a str,
    /// Aun trabajando: el texto, apagado.
    apagado: bool,
    pie: Pie,
    medidas: Medidas,
    desplazado: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pie {
    Nada,
    Abrir,
    /// «Sí», «No» y, si se sabe el chat, «Abrir».
    Permiso {
        abrir: bool,
    },
}

/// Los colores de la tarjeta: los del chat, con un filete que se vea sobre
/// un escritorio claro (el del chat claro es casi transparente).
struct Colores {
    tema: &'static Tema,
    borde: Color,
    /// El rotulo de «Abrir»: el azul del tema que se lee sobre el boton.
    enlace: Color,
}

fn colores(claro: bool) -> Colores {
    if claro {
        Colores {
            tema: &CLARO,
            borde: hex(0xc4c8d4),
            enlace: CLARO.texto_pestana_activa,
        }
    } else {
        Colores {
            tema: &OSCURO,
            borde: OSCURO.isla_borde,
            enlace: OSCURO.texto_pestana_activa,
        }
    }
}

/// El texto de una pregunta pendiente.
fn texto_de_la_pregunta(p: &Pregunta) -> String {
    format!(
        "🔐 Necesito tu permiso: {}\n\nSi no contestas, no lo hago.",
        p.texto
    )
}

fn flotar(recursos: &Recursos, mando: &Mando, puesto: usize, id: u64) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .monitor_en(pixpin_shell::posicion_del_cursor())
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien as f32 / 100.0;
    let area = monitor.area_trabajo;
    let mut marco = marco_inicial(area, monitor.escala_por_cien, puesto);
    let alto_maximo = (ALTO_MAXIMO as f32 * escala).min(area.alto as f32 - 48.0 * escala);
    let mut ventana = VentanaOverlay::nueva(marco).context("no se pudo abrir el aviso")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para el aviso")?;
    // Sin robar el foco: se esta haciendo otra cosa.
    ventana.mostrar();
    ventana.traer_encima();
    mando.1.store(ventana.handle().0 as isize, Ordering::SeqCst);
    // El tema del chat: el de Windows (con Cosmos, siempre de noche).
    let claro = pixpin_shell::entorno::tema_claro() && !crate::tema_cosmos::activo();
    let colores = colores(claro);

    let nacio = Instant::now();
    let mut acabado_en: Option<Instant> = None;
    let mut botones: Botones<Accion> = Botones::default();
    let mut pulsado: Option<(Punto, Option<Accion>)> = None;
    // El texto medido la ultima vez (medir solo cuando cambia).
    let mut medido: (String, f32) = (String::new(), 0.0);
    let mut desplazado = 0.0f32;
    let mut vivo = true;
    // Lo que dijo al irse: el aspa con una pregunta delante es un no.
    let mut cerrada_a_mano = false;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        let mut accion = None;
        let mut rueda = 0;
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            match ev {
                EventoOverlay::Cerrar | EventoOverlay::Tecla { vk: VK_ESCAPE, .. } => {
                    vivo = false;
                    cerrada_a_mano = true;
                }
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
                EventoOverlay::Rueda(d) => rueda += d,
                _ => {}
            }
        }
        let aviso = mando.0.lock().map(|a| a.clone()).ok();
        let Some(aviso) = aviso else { break };
        // Una pregunta del turno que espera esta tarjeta (solo mientras
        // trabaja: acabado, ya no pregunta nada).
        let pregunta = if aviso.acabado {
            None
        } else {
            permiso::para_la_tarjeta(id)
        };
        match accion {
            Some(Accion::Cerrar) => {
                vivo = false;
                cerrada_a_mano = true;
            }
            Some(Accion::Si) | Some(Accion::No) => {
                if let Some(p) = &pregunta {
                    permiso::contestar(&p.proyecto, &p.nota, accion == Some(Accion::Si));
                }
            }
            Some(Accion::Abrir) => {
                if let Some(p) = &pregunta {
                    // A la burbuja de la pregunta, que tiene su «Sí»/«No».
                    if let Some(c) = &aviso.chat {
                        crate::ventana_chat::ir_a(
                            c.idioma,
                            c.ubicacion.clone(),
                            c.opciones,
                            p.proyecto.clone(),
                            Some(p.nota.clone()),
                        );
                    }
                } else if let Some(a) = &aviso.abrir {
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
        if !vivo {
            // El aspa o Esc con la pregunta delante: no (lo seguro).
            if cerrada_a_mano && let Some(p) = &pregunta {
                permiso::contestar(&p.proyecto, &p.nota, false);
            }
            break;
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
                break;
            }
        } else if nacio.elapsed() >= ESPERA_MAXIMA {
            break;
        }

        // Que se ensena, cuanto mide y, si cambio, el alto de la ventana.
        let (texto, pie) = match &pregunta {
            Some(p) => (
                texto_de_la_pregunta(p),
                Pie::Permiso {
                    abrir: aviso.chat.is_some(),
                },
            ),
            None => (
                lo_que_se_ensena(&aviso.texto),
                if aviso.abrir.is_some() {
                    Pie::Abrir
                } else {
                    Pie::Nada
                },
            ),
        };
        if medido.0 != texto {
            let ancho_texto = marco.ancho as f32 - 32.0 * escala;
            let alto = motor.medir_texto(&texto, TAM_TEXTO * escala, ancho_texto).1;
            medido = (texto, alto);
            desplazado = 0.0;
        }
        let m = medidas(
            marco.ancho as f32,
            medido.1,
            pie != Pie::Nada,
            alto_maximo,
            escala,
        );
        desplazado = (desplazado - rueda as f32 / 120.0 * 48.0 * escala).clamp(0.0, m.sobra);
        if m.alto as u32 != marco.alto {
            marco = con_alto(marco, m.alto as u32, area);
            ventana.mover(marco);
            let _ = superficie.redimensionar(marco.ancho, marco.alto);
        }
        let vista = Vista {
            titulo: &aviso.titulo,
            texto: &medido.0,
            apagado: !aviso.acabado && pregunta.is_none(),
            pie,
            medidas: m,
            desplazado,
        };
        if let Ok(d) = superficie.empezar(&motor) {
            let _ = motor.dibujar(&d, |p: &Pintor| {
                pintar(
                    p,
                    (marco.ancho as f32, marco.alto as f32),
                    escala,
                    &colores,
                    &vista,
                    &mut botones,
                )
            });
            let _ = superficie.presentar();
        }
        pixpin_shell::overlay::esperar_eventos(Some(if aviso.acabado { 250 } else { 120 }));
    }
    ventana.ocultar();
    Ok(())
}

/// El texto que se ensena: sin lineas en blanco de mas y sin pasar de
/// [`LETRAS`] (lo largo entero esta en el chat, tras «Abrir»).
pub(crate) fn lo_que_se_ensena(texto: &str) -> String {
    let lineas: Vec<&str> = texto
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    crate::automatizaciones::recortar(&lineas.join("\n"), LETRAS)
}

fn pintar(
    p: &Pintor,
    (w, h): (f32, f32),
    e: f32,
    c: &Colores,
    v: &Vista,
    botones: &mut Botones<Accion>,
) {
    let tema = c.tema;
    p.limpiar_transparente();
    p.rellenar_redondeado(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho: w,
            alto: h,
        },
        16.0 * e,
        c.borde,
    );
    p.rellenar_redondeado(
        RectF {
            x: 1.0,
            y: 1.0,
            ancho: w - 2.0,
            alto: h - 2.0,
        },
        15.0 * e,
        tema.isla,
    );
    botones.vaciar();

    // Arriba: el titulo y el aspa.
    let aspa = RectF {
        x: w - 38.0 * e,
        y: 8.0 * e,
        ancho: 28.0 * e,
        alto: 28.0 * e,
    };
    p.texto_linea(
        v.titulo,
        16.0 * e,
        14.0 * e,
        14.0 * e,
        (aspa.x - 22.0 * e).max(0.0),
        tema.pildora_texto,
    );
    if dentro(aspa, botones.raton) {
        p.rellenar_redondeado(aspa, aspa.alto / 2.0, tema.boton_sobre);
    }
    let l = 16.0 * e;
    p.icono(
        &mi::CLOSE,
        RectF {
            x: aspa.x + (aspa.ancho - l) / 2.0,
            y: aspa.y + (aspa.alto - l) / 2.0,
            ancho: l,
            alto: l,
        },
        tema.campo_apagado,
    );
    botones.zona(aspa, Accion::Cerrar);

    // El texto, recortado a su zona: nunca pasa por debajo de los botones.
    let zona = v.medidas.texto;
    let color = if v.apagado {
        tema.campo_apagado
    } else {
        tema.texto
    };
    p.con_recorte(zona, |p| {
        p.texto_ajustado(
            v.texto,
            zona.x,
            zona.y - v.desplazado,
            TAM_TEXTO * e,
            zona.ancho,
            color,
        );
    });
    if v.medidas.sobra > 0.0 {
        // La barrita de lo que queda, a la derecha de la zona.
        let total = zona.alto + v.medidas.sobra;
        let alto = (zona.alto * zona.alto / total).max(18.0 * e);
        let y = zona.y + (zona.alto - alto) * (v.desplazado / v.medidas.sobra);
        p.rellenar_redondeado(
            RectF {
                x: w - 9.0 * e,
                y,
                ancho: 3.0 * e,
                alto,
            },
            1.5 * e,
            tema.campo_apagado,
        );
    }

    // Abajo: los botones, en su sitio reservado.
    let Some(y) = v.medidas.pie else { return };
    let boton = |p: &Pintor,
                 botones: &mut Botones<Accion>,
                 caja: RectF,
                 rotulo: &str,
                 tinta: Color,
                 que: Accion| {
        let fondo = if dentro(caja, botones.raton) {
            tema.boton_sobre
        } else {
            tema.campo
        };
        p.rellenar_redondeado(caja, 8.0 * e, fondo);
        let (tw, th) = p.medir_texto(rotulo, 14.0 * e);
        p.texto(
            rotulo,
            caja.x + (caja.ancho - tw) / 2.0,
            caja.y + (caja.alto - th) / 2.0,
            14.0 * e,
            tinta,
        );
        botones.zona(caja, que);
    };
    let (bw, bh) = (88.0 * e, 32.0 * e);
    let abrir = RectF {
        x: w - 16.0 * e - bw,
        y,
        ancho: bw,
        alto: bh,
    };
    match v.pie {
        Pie::Nada => {}
        Pie::Abrir => boton(p, botones, abrir, "Abrir", c.enlace, Accion::Abrir),
        Pie::Permiso { abrir: con_abrir } => {
            let si = RectF {
                x: 16.0 * e,
                y,
                ancho: bw,
                alto: bh,
            };
            let no = RectF {
                x: si.x + bw + 8.0 * e,
                ..si
            };
            boton(p, botones, si, "Sí", tema.texto, Accion::Si);
            boton(p, botones, no, "No", tema.texto, Accion::No);
            if con_abrir {
                boton(p, botones, abrir, "Abrir", c.enlace, Accion::Abrir);
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_nueva_va_encima_de_las_que_hay_y_no_se_sale() {
        let area = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1040,
        };
        let a = marco_inicial(area, 100, 0);
        let b = marco_inicial(area, 100, 1);
        assert_eq!(a.x, b.x);
        assert!(b.y < a.y);
        assert_eq!(a.y + a.alto as i32, 1040 - 24);
        // Caso negativo: muchas no se salen por arriba.
        let z = marco_inicial(
            Rect {
                x: 0,
                y: 0,
                ancho: 800,
                alto: 300,
            },
            100,
            9,
        );
        assert!(z.y >= 0);
    }

    #[test]
    fn crece_hacia_arriba_con_el_pie_donde_estaba_y_sin_salirse() {
        let area = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1040,
        };
        let a = marco_inicial(area, 100, 0);
        let b = con_alto(a, 400, area);
        assert_eq!(
            b.y + b.alto as i32,
            a.y + a.alto as i32,
            "el pie no se mueve"
        );
        assert_eq!((b.x, b.ancho, b.alto), (a.x, a.ancho, 400));
        // Caso negativo: mas alto que la pantalla, se queda dentro.
        let c = con_alto(a, 5000, area);
        assert!(c.y >= 0 && c.alto <= area.alto);
    }

    #[test]
    fn un_texto_enorme_no_pasa_por_debajo_de_los_botones() {
        let (ancho, maximo) = (340.0, 420.0);
        // 31 tareas: mucho mas de lo que cabe.
        let m = medidas(ancho, 31.0 * 18.0, true, maximo, 1.0);
        assert_eq!(m.alto, maximo, "crece hasta el maximo y no mas");
        let pie = m.pie.expect("con botones");
        assert!(
            m.texto.y + m.texto.alto <= pie - 6.0,
            "la zona del texto acaba antes del pie: {m:?}"
        );
        assert!(m.sobra > 0.0, "lo que no cabe se desplaza");
        assert!((m.texto.alto + m.sobra - 31.0 * 18.0).abs() < 0.01);
        // Corto: el alto minimo y nada que desplazar.
        let corto = medidas(ancho, 18.0, true, maximo, 1.0);
        assert_eq!(corto.alto, ALTO_MINIMO as f32);
        assert_eq!(corto.sobra, 0.0);
        // A escala 150 %, lo mismo en proporcion.
        let grande = medidas(ancho * 1.5, 2000.0, true, maximo * 1.5, 1.5);
        assert!(grande.texto.y + grande.texto.alto <= grande.pie.unwrap());
        // Caso negativo: sin botones no se reserva pie (el texto llega mas abajo).
        let sin = medidas(ancho, 31.0 * 18.0, false, maximo, 1.0);
        assert_eq!(sin.pie, None);
        assert!(sin.texto.alto > m.texto.alto);
        // Ni un maximo absurdo (pantalla minuscula) baja del minimo.
        assert_eq!(
            medidas(ancho, 900.0, true, 10.0, 1.0).alto,
            ALTO_MINIMO as f32
        );
    }

    #[test]
    fn lo_que_se_ensena_es_sin_huecos_y_con_tope() {
        assert_eq!(lo_que_se_ensena("  hola\n\n\n  que tal  "), "hola\nque tal");
        let largo = "a".repeat(10_000);
        assert_eq!(lo_que_se_ensena(&largo).chars().count(), LETRAS + 1);
        // 31 tareas caben enteras (antes se cortaba a las 220 letras).
        let tareas: String = (1..=31)
            .map(|i| format!("• Tarea pendiente numero {i}\n"))
            .collect();
        assert!(!lo_que_se_ensena(&tareas).ends_with('…'));
    }

    #[test]
    fn la_pregunta_dice_que_sin_respuesta_es_que_no() {
        let p = Pregunta {
            proyecto: "p".into(),
            nota: "n".into(),
            texto: "¿Borro 3 tareas?".into(),
        };
        let t = texto_de_la_pregunta(&p);
        assert!(t.contains("Necesito tu permiso: ¿Borro 3 tareas?"));
        assert!(t.contains("no lo hago"));
    }

    /// **Las tarjetas pintadas de verdad** (necesita GPU): la respuesta
    /// larga y la pregunta, en claro y en oscuro. Deja los PNG en
    /// `PIXPIN_MUESTRAS`.
    #[test]
    #[ignore = "pinta PNG con la GPU"]
    fn muestra_del_aviso() {
        let tareas: String = (1..=31)
            .map(|i| format!("{i}. Revisar la partida {i} del presupuesto de la obra\n"))
            .collect();
        let largo = format!("Tienes 31 tareas pendientes:\n{tareas}");
        let pregunta = texto_de_la_pregunta(&Pregunta {
            proyecto: "p".into(),
            nota: "n".into(),
            texto: "¿Borro las 3 tareas hechas de «Obra Norte»? No se puede deshacer.".into(),
        });
        for (nombre, texto, pie, apagado) in [
            ("aviso-largo", largo.as_str(), Pie::Abrir, false),
            (
                "aviso-permiso",
                pregunta.as_str(),
                Pie::Permiso { abrir: true },
                false,
            ),
        ] {
            for claro in [true, false] {
                let e = 1.0;
                let ancho = ANCHO as f32 * e;
                let pintar_con =
                    |p: &Pintor, motor: &pixpin_render::MotorRender, alto: &mut f32| {
                        let t = lo_que_se_ensena(texto);
                        let medido = motor.medir_texto(&t, TAM_TEXTO * e, ancho - 32.0 * e).1;
                        let m = medidas(ancho, medido, pie != Pie::Nada, ALTO_MAXIMO as f32 * e, e);
                        *alto = m.alto;
                        let v = Vista {
                            titulo: "🤖 ¿Qué tengo pendiente?",
                            texto: &t,
                            apagado,
                            pie,
                            medidas: m,
                            desplazado: 0.0,
                        };
                        let mut b = Botones::default();
                        pintar(p, (ancho, m.alto), e, &colores(claro), &v, &mut b);
                    };
                // El alto sale de medir: primero se mide, luego se pinta a esa medida.
                let mut alto = 0.0;
                crate::ventanita::muestra("aviso-medir", ancho as u32, 10, |p, motor| {
                    pintar_con(p, motor, &mut alto)
                });
                let tema = if claro { "claro" } else { "oscuro" };
                crate::ventanita::muestra(
                    &format!("{nombre}-{tema}"),
                    ancho as u32,
                    alto as u32,
                    |p, motor| pintar_con(p, motor, &mut alto),
                );
            }
        }
    }
}
