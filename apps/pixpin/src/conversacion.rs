//! **Una conversacion de varias personas, grabada por turnos y pasada a
//! texto con nombres** (B10).
//!
//! Es `guardados/ConversacionActivity.kt`. Se dice cuantos hablan y como se
//! llaman, y cada uno dice cuando le toca: asi se sabe quien dijo cada cosa
//! sin adivinarlo por la voz, que es lo que ningun reconocedor gratuito hace
//! bien. Al terminar, la nota entra en el chat con **`turnos`** (quien, desde
//! y hasta, los mismos campos del movil) y se pasa a texto turno a turno,
//! `[0:12] **Pepe:** …`.
//!
//! **Lo que cambia en el PC**: en el movil cada uno mantiene pulsado su
//! microfono en la pantalla, que esta en la mesa entre todos. Aqui se graba
//! **sin parar** (WASAPI, un solo `.m4a`, nada de pegar trozos) y las teclas
//! **1..9** dicen quien habla desde ese momento; **0** es «no habla nadie»
//! (lo de en medio no sale en el texto). Tambien vale pulsar la ficha de la
//! persona con el raton. Una sola grabacion tiene otra ventaja: no se pierde
//! el principio de cada frase mientras se busca el boton.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use anyhow::{Context, Result};
use pixpin_audio::picos::MINIMO_MS;
use pixpin_audio::{Grabadora, reloj};
use pixpin_geom::Rect;
use pixpin_proyecto::cuaderno::{Clase, Mensaje};
use pixpin_render::{Color, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::{Catalogo, Idioma};
use pixpin_voz::turnos::{self, Turno};

use crate::caja_dibujo::hex;
use crate::overlay::Recursos;
use crate::ventanita::{
    APAGADO, Botones, CRISTAL, DORADO, FONDO, ROJO, TEXTO, centrado, centrado_en,
};

/// Cuantos pueden hablar: del 2 al 9, una tecla por persona.
pub const MAXIMO: usize = 9;
/// Un turno mas corto que esto es un dedo que se equivoco de tecla.
const TURNO_MINIMO_MS: i64 = 400;

/// Los colores del movil (`COLORES`), y tres mas para llegar a nueve.
const COLORES: [Color; MAXIMO] = [
    hex(0x1E88E5),
    hex(0xE53935),
    hex(0x43A047),
    hex(0xFB8C00),
    hex(0x8E24AA),
    hex(0x00897B),
    hex(0x6D4C41),
    hex(0xD81B60),
    hex(0x3949AB),
];

/// Lo grabado: el audio entero y quien hablo cuando.
#[derive(Debug, Clone, PartialEq)]
pub struct Grabada {
    pub ruta: PathBuf,
    pub duracion_ms: i64,
    pub picos: Vec<i32>,
    pub turnos: Vec<Turno>,
}

/// **Escribe la conversacion en un mensaje de voz**: clase, duracion, onda y
/// `turnos` en el formato del movil (`Mensaje.turnos`), que es lo que hace
/// que pasarla a texto —ahora o «otra vez»— salga con los nombres.
pub fn aplicar(m: &mut Mensaje, g: &Grabada) {
    m.clase = Some(Clase::Voz);
    m.duracion_ms = g.duracion_ms;
    m.resto.insert(
        "picos".into(),
        serde_json::Value::Array(
            g.picos
                .iter()
                .map(|p| serde_json::Value::from(*p))
                .collect(),
        ),
    );
    m.resto.insert("turnos".into(), turnos::a_json(&g.turnos));
}

/// Como se llama la persona `i`: lo escrito, o «Persona 2».
pub fn nombre_de(nombres: &[String], i: usize, textos: &Catalogo) -> String {
    match nombres.get(i).map(|n| n.trim()) {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => {
            let mut a = fluent_bundle::FluentArgs::new();
            a.set("n", i as i64 + 1);
            textos.t_args("conversacion-persona", &a)
        }
    }
}

/// Los turnos de la grabacion a partir de las teclas, sin los ratos en que
/// no hablaba nadie (la tecla 0).
pub fn turnos_de(cambios: &[(i64, String)], fin_ms: i64) -> Vec<Turno> {
    turnos::de_los_cambios(cambios, fin_ms, TURNO_MINIMO_MS)
        .into_iter()
        .filter(|t| !t.quien.is_empty())
        .collect()
}

/// **Abre la conversacion en su hilo.** Por el canal llega la grabacion al
/// terminar; cerrar sin terminar cierra el canal sin mandar nada.
pub fn lanzar(idioma: Idioma, destino: PathBuf) -> Receiver<Grabada> {
    let (enviar, recibir) = channel();
    let lanzado = std::thread::Builder::new()
        .name("conversacion".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let hecho = crate::dispositivo_perdido::con_recursos("conversacion", |r| {
                abrir(r, idioma, &destino, &enviar)
            });
            if let Err(e) = hecho {
                tracing::warn!(?e, "no se pudo abrir la conversacion");
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de la conversacion");
    }
    recibir
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Cerrar,
    Menos,
    Mas,
    Nombre(usize),
    Empezar,
    Persona(usize),
    Nadie,
    Terminar,
    Tirar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Paso {
    Preparar,
    Grabar,
}

struct Estado {
    paso: Paso,
    cuantos: usize,
    nombres: Vec<String>,
    /// La caja de nombre en la que se escribe.
    escribiendo: Option<usize>,
    grabadora: Option<Grabadora>,
    /// El microfono abriendose en otro hilo.
    abriendo: Option<Receiver<Result<Grabadora, pixpin_audio::ErrorAudio>>>,
    /// `(milisegundo, quien)` de cada tecla; `quien` vacio = nadie.
    cambios: Vec<(i64, String)>,
    hablando: Option<usize>,
    botones: Botones<Accion>,
    aviso: Option<(String, u64)>,
}

fn abrir(
    recursos: &Recursos,
    idioma: Idioma,
    destino: &Path,
    enviar: &Sender<Grabada>,
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
    let marco = centrado(monitor.area_trabajo, 920, 660, monitor.escala_por_cien);
    let ventana = VentanaOverlay::nueva_normal(marco, &textos.t("conversacion-titulo"))
        .context("no se pudo abrir la conversacion")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para la conversacion")?;
    ventana.mostrar();
    ventana.enfocar();
    let _ = std::fs::create_dir_all(destino);

    let mut e = Estado {
        paso: Paso::Preparar,
        cuantos: 2,
        nombres: vec![String::new(); MAXIMO],
        escribiendo: Some(0),
        grabadora: None,
        abriendo: None,
        cambios: Vec::new(),
        hablando: None,
        botones: Botones::default(),
        aviso: None,
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
            let accion = match ev {
                EventoOverlay::Cerrar => Some(Accion::Cerrar),
                EventoOverlay::RatonMovido(p) => {
                    e.botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    None
                }
                EventoOverlay::BotonPulsado(p) => {
                    e.botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    e.botones.bajo_el_raton()
                }
                EventoOverlay::Caracter(c) if e.paso == Paso::Preparar => {
                    if let Some(i) = e.escribiendo
                        && !c.is_control()
                        && e.nombres[i].chars().count() < 30
                    {
                        e.nombres[i].push(c);
                    }
                    None
                }
                EventoOverlay::Tecla { vk, shift, .. } => tecla(&mut e, vk, shift),
                _ => None,
            };
            if let Some(a) = accion {
                hacer(&mut e, a, textos, destino, enviar, &mut vivo, ahora);
            }
        }
        if !vivo {
            break;
        }
        // El microfono, cuando el otro hilo lo tenga abierto.
        if let Some(r) = &e.abriendo
            && let Ok(llegado) = r.try_recv()
        {
            e.abriendo = None;
            pintar = true;
            match llegado {
                Ok(g) => {
                    e.grabadora = Some(g);
                    e.paso = Paso::Grabar;
                    e.escribiendo = None;
                }
                Err(err) => {
                    tracing::info!(?err, "no se pudo grabar la conversacion");
                    e.aviso = Some((textos.t("telepronter-sin-micro"), ahora + 4_000));
                }
            }
        }
        if e.aviso.as_ref().is_some_and(|(_, hasta)| *hasta < ahora) {
            e.aviso = None;
            pintar = true;
        }
        if pintar || e.grabadora.is_some() {
            if let Ok(d) = superficie.empezar(&motor) {
                let _ = motor.dibujar(&d, |p: &Pintor| {
                    pintar_todo(&mut e, p, marco, escala, textos)
                });
                let _ = superficie.presentar();
            }
            pintar = false;
        }
        let espera = if e.grabadora.is_some() || e.abriendo.is_some() {
            100
        } else {
            500
        };
        pixpin_shell::overlay::esperar_eventos(Some(espera));
    }
    // Cerrar sin terminar tira lo grabado: no se guarda a escondidas.
    if let Some(g) = e.grabadora.take() {
        g.cancelar();
    }
    Ok(())
}

/// Lo que hace cada tecla segun el paso.
fn tecla(e: &mut Estado, vk: u32, shift: bool) -> Option<Accion> {
    const VK_ESCAPE: u32 = 0x1B;
    const VK_INTRO: u32 = 0x0D;
    const VK_TAB: u32 = 0x09;
    const VK_BORRAR: u32 = 0x08;
    match e.paso {
        Paso::Preparar => match vk {
            VK_ESCAPE => Some(Accion::Cerrar),
            VK_INTRO => Some(Accion::Empezar),
            VK_TAB => {
                let n = e.cuantos;
                e.escribiendo = Some(match (e.escribiendo, shift) {
                    (Some(i), false) => (i + 1) % n,
                    (Some(i), true) => (i + n - 1) % n,
                    (None, _) => 0,
                });
                None
            }
            VK_BORRAR => {
                if let Some(i) = e.escribiendo {
                    e.nombres[i].pop();
                }
                None
            }
            _ => None,
        },
        // Grabando: 1..9 (fila de arriba o teclado numerico) y 0.
        Paso::Grabar => match vk {
            0x30 | 0x60 => Some(Accion::Nadie),
            0x31..=0x39 => Some(Accion::Persona((vk - 0x31) as usize)),
            0x61..=0x69 => Some(Accion::Persona((vk - 0x61) as usize)),
            VK_INTRO => Some(Accion::Terminar),
            _ => None,
        },
    }
}

fn hacer(
    e: &mut Estado,
    a: Accion,
    textos: &Catalogo,
    destino: &Path,
    enviar: &Sender<Grabada>,
    vivo: &mut bool,
    ahora: u64,
) {
    match a {
        Accion::Cerrar | Accion::Tirar => *vivo = false,
        Accion::Menos => {
            e.cuantos = (e.cuantos - 1).max(2);
            e.escribiendo = e.escribiendo.map(|i| i.min(e.cuantos - 1));
        }
        Accion::Mas => e.cuantos = (e.cuantos + 1).min(MAXIMO),
        Accion::Nombre(i) => e.escribiendo = Some(i),
        Accion::Empezar => {
            if e.paso != Paso::Preparar || e.abriendo.is_some() {
                return;
            }
            // En otro hilo: abrir el microfono tarda medio segundo la primera
            // vez y la ventana no puede quedarse parada.
            let fichero = destino.join(format!("conversacion-{ahora}.m4a"));
            e.abriendo = Some(crate::ventanita::abrir_microfono(fichero));
        }
        Accion::Persona(i) if i < e.cuantos => cambiar(e, Some(i), textos),
        Accion::Persona(_) => {}
        Accion::Nadie => cambiar(e, None, textos),
        Accion::Terminar => terminar(e, textos, enviar, vivo, ahora),
    }
}

/// Desde ahora habla `quien` (o nadie).
fn cambiar(e: &mut Estado, quien: Option<usize>, textos: &Catalogo) {
    let Some(g) = &e.grabadora else {
        return;
    };
    if e.hablando == quien {
        return;
    }
    let nombre = quien
        .map(|i| nombre_de(&e.nombres, i, textos))
        .unwrap_or_default();
    e.cambios.push((g.llevado_ms(), nombre));
    e.hablando = quien;
}

fn terminar(
    e: &mut Estado,
    textos: &Catalogo,
    enviar: &Sender<Grabada>,
    vivo: &mut bool,
    ahora: u64,
) {
    let Some(g) = e.grabadora.take() else {
        return;
    };
    let grabacion = match g.parar() {
        Ok(g) => g,
        Err(err) => {
            tracing::info!(?err, "la conversacion no se pudo cerrar");
            e.aviso = Some((textos.t("chat-no-se-pudo"), ahora + 4_000));
            e.paso = Paso::Preparar;
            return;
        }
    };
    if grabacion.duracion_ms < MINIMO_MS {
        let _ = std::fs::remove_file(&grabacion.ruta);
        e.aviso = Some((textos.t("chat-voz-muy-corta"), ahora + 3_000));
        e.paso = Paso::Preparar;
        return;
    }
    let turnos = turnos_de(&e.cambios, grabacion.duracion_ms);
    let _ = enviar.send(Grabada {
        ruta: grabacion.ruta,
        duracion_ms: grabacion.duracion_ms,
        picos: grabacion.picos,
        turnos,
    });
    *vivo = false;
}

// `i` da a la vez la columna, la fila y su color de `COLORES`.
#[allow(clippy::needless_range_loop)]
fn pintar_todo(e: &mut Estado, p: &Pintor, marco: Rect, escala: f32, textos: &Catalogo) {
    let (w, h) = (marco.ancho as f32, marco.alto as f32);
    p.limpiar(FONDO);
    e.botones.vaciar();
    let todo = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: h,
    };
    match e.paso {
        Paso::Preparar => {
            let x = 40.0 * escala;
            let mut y = 36.0 * escala;
            p.texto(&textos.t("conversacion-titulo"), x, y, 24.0 * escala, TEXTO);
            y += 40.0 * escala;
            p.texto_ajustado(
                &textos.t("conversacion-explicacion"),
                x,
                y,
                14.0 * escala,
                w - 2.0 * x,
                APAGADO,
            );
            y += 60.0 * escala;
            p.texto(
                &textos.t("conversacion-cuantos"),
                x,
                y + 8.0 * escala,
                16.0 * escala,
                TEXTO,
            );
            let lado = 36.0 * escala;
            let xb = w - x - 3.0 * lado;
            e.botones.boton(
                p,
                RectF {
                    x: xb,
                    y,
                    ancho: lado,
                    alto: lado,
                },
                Accion::Menos,
                "−",
                None,
                escala,
            );
            centrado_en(
                p,
                &e.cuantos.to_string(),
                RectF {
                    x: xb + lado,
                    y,
                    ancho: lado,
                    alto: lado,
                },
                y + 6.0 * escala,
                18.0 * escala,
                TEXTO,
            );
            e.botones.boton(
                p,
                RectF {
                    x: xb + 2.0 * lado,
                    y,
                    ancho: lado,
                    alto: lado,
                },
                Accion::Mas,
                "+",
                None,
                escala,
            );
            y += 50.0 * escala;
            // Los nombres, en dos columnas si son muchos.
            let columnas = if e.cuantos > 5 { 2 } else { 1 };
            let ancho = (w - 2.0 * x - 12.0 * escala * (columnas - 1) as f32) / columnas as f32;
            for i in 0..e.cuantos {
                let (col, fila) = (i % columnas, i / columnas);
                let c = RectF {
                    x: x + col as f32 * (ancho + 12.0 * escala),
                    y: y + fila as f32 * 46.0 * escala,
                    ancho,
                    alto: 38.0 * escala,
                };
                let activa = e.escribiendo == Some(i);
                p.rellenar_redondeado(
                    c,
                    8.0 * escala,
                    if activa { hex(0x2e3440) } else { CRISTAL },
                );
                p.rellenar_redondeado(
                    RectF {
                        ancho: 6.0 * escala,
                        ..c
                    },
                    3.0 * escala,
                    COLORES[i],
                );
                let (rotulo, color) = if e.nombres[i].is_empty() {
                    (nombre_de(&e.nombres, i, textos), APAGADO)
                } else {
                    (e.nombres[i].clone(), TEXTO)
                };
                let cursor = if activa { "|" } else { "" };
                p.texto(
                    &format!("{}  {rotulo}{cursor}", i + 1),
                    c.x + 16.0 * escala,
                    c.y + 9.0 * escala,
                    15.0 * escala,
                    color,
                );
                e.botones.zona(c, Accion::Nombre(i));
            }
            let ancho_b = 220.0 * escala;
            let c = RectF {
                x: w - x - ancho_b,
                y: h - 70.0 * escala,
                ancho: ancho_b,
                alto: 44.0 * escala,
            };
            e.botones.boton(
                p,
                c,
                Accion::Empezar,
                &textos.t("conversacion-empezar"),
                Some(crate::ventanita::AZUL),
                escala,
            );
            e.botones.boton(
                p,
                RectF {
                    x,
                    ancho: 140.0 * escala,
                    ..c
                },
                Accion::Cerrar,
                &textos.t("chat-cancelar-caja"),
                None,
                escala,
            );
        }
        Paso::Grabar => {
            // Un recuadro por persona, como los microfonos del movil.
            let arriba = 56.0 * escala;
            let pie = 80.0 * escala;
            let columnas = if e.cuantos <= 2 { e.cuantos } else { 3 };
            let filas = e.cuantos.div_ceil(columnas);
            let (cw, ch) = (
                (w - 24.0 * escala) / columnas as f32,
                (h - arriba - pie) / filas as f32,
            );
            let llevado = e.grabadora.as_ref().map(|g| g.llevado_ms()).unwrap_or(0);
            p.texto(
                &format!("● {}", reloj::duracion_legible(llevado)),
                20.0 * escala,
                18.0 * escala,
                18.0 * escala,
                ROJO,
            );
            centrado_en(
                p,
                &textos.t("conversacion-teclas"),
                todo,
                20.0 * escala,
                13.0 * escala,
                APAGADO,
            );
            for i in 0..e.cuantos {
                let c = RectF {
                    x: 12.0 * escala + (i % columnas) as f32 * cw + 6.0 * escala,
                    y: arriba + (i / columnas) as f32 * ch + 6.0 * escala,
                    ancho: cw - 12.0 * escala,
                    alto: ch - 12.0 * escala,
                };
                let activo = e.hablando == Some(i);
                let mut color = COLORES[i];
                color.a = if activo {
                    1.0
                } else if e.hablando.is_some() {
                    0.25
                } else {
                    0.55
                };
                p.rellenar_redondeado(c, 18.0 * escala, color);
                let medio = c.y + c.alto / 2.0;
                centrado_en(
                    p,
                    &(i + 1).to_string(),
                    c,
                    medio - 46.0 * escala,
                    30.0 * escala,
                    TEXTO,
                );
                centrado_en(
                    p,
                    &nombre_de(&e.nombres, i, textos),
                    c,
                    medio - 4.0 * escala,
                    20.0 * escala,
                    TEXTO,
                );
                let estado = if activo {
                    textos.t("conversacion-hablando")
                } else {
                    String::new()
                };
                centrado_en(p, &estado, c, medio + 26.0 * escala, 13.0 * escala, TEXTO);
                e.botones.zona(c, Accion::Persona(i));
            }
            let turnos = turnos_de(&e.cambios, llevado).len();
            let mut a = fluent_bundle::FluentArgs::new();
            a.set("n", turnos as i64);
            p.texto(
                &textos.t_args("conversacion-turnos", &a),
                20.0 * escala,
                h - pie + 30.0 * escala,
                14.0 * escala,
                APAGADO,
            );
            let ancho_b = 240.0 * escala;
            let c = RectF {
                x: w - 20.0 * escala - ancho_b,
                y: h - pie + 18.0 * escala,
                ancho: ancho_b,
                alto: 44.0 * escala,
            };
            e.botones.boton(
                p,
                c,
                Accion::Terminar,
                &textos.t("conversacion-terminar"),
                Some(ROJO),
                escala,
            );
            e.botones.boton(
                p,
                RectF {
                    x: c.x - 130.0 * escala,
                    ancho: 120.0 * escala,
                    ..c
                },
                Accion::Tirar,
                &textos.t("telepronter-tirar"),
                None,
                escala,
            );
        }
    }
    if let Some((aviso, _)) = &e.aviso {
        p.texto(
            aviso,
            20.0 * escala,
            h - 110.0 * escala,
            14.0 * escala,
            DORADO,
        );
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_conversacion_se_escribe_con_sus_turnos_en_el_campo_del_movil() {
        let mut m = Mensaje::default();
        aplicar(
            &mut m,
            &Grabada {
                ruta: PathBuf::from("C:/x/conversacion-1.m4a"),
                duracion_ms: 9_000,
                picos: vec![],
                turnos: vec![
                    Turno::nuevo("Ana", 0, 4_000),
                    Turno::nuevo("Luis", 4_000, 9_000),
                ],
            },
        );
        assert_eq!(m.clase, Some(Clase::Voz));
        assert_eq!(m.duracion_ms, 9_000);
        assert_eq!(
            m.resto.get("turnos"),
            Some(&serde_json::json!([
                {"quien": "Ana", "desdeMs": 0, "hastaMs": 4_000},
                {"quien": "Luis", "desdeMs": 4_000, "hastaMs": 9_000}
            ]))
        );
        // Y la transcripcion lo lee de vuelta con los nombres.
        assert_eq!(crate::voz::turnos_de(&m).len(), 2);
    }

    #[test]
    fn lo_que_se_graba_con_el_cero_no_es_de_nadie_y_no_sale() {
        let c = vec![
            (0, "Ana".to_string()),
            (3_000, String::new()),
            (6_000, "Luis".to_string()),
        ];
        assert_eq!(
            turnos_de(&c, 9_000),
            vec![
                Turno::nuevo("Ana", 0, 3_000),
                Turno::nuevo("Luis", 6_000, 9_000)
            ]
        );
    }

    #[test]
    fn sin_nombre_la_persona_se_llama_por_su_numero() {
        let t = Catalogo::nuevo(Idioma::Espanol);
        let nombres = vec!["  Pepe ".to_string(), String::new()];
        assert_eq!(nombre_de(&nombres, 0, &t), "Pepe");
        assert_eq!(nombre_de(&nombres, 1, &t), "Persona 2");
        assert_eq!(nombre_de(&nombres, 7, &t), "Persona 8");
    }

    #[test]
    fn las_teclas_uno_a_nueve_eligen_persona_y_el_cero_a_nadie() {
        let mut e = Estado {
            paso: Paso::Grabar,
            cuantos: 3,
            nombres: vec![String::new(); MAXIMO],
            escribiendo: None,
            grabadora: None,
            abriendo: None,
            cambios: vec![],
            hablando: None,
            botones: Botones::default(),
            aviso: None,
        };
        assert_eq!(tecla(&mut e, 0x31, false), Some(Accion::Persona(0)));
        assert_eq!(tecla(&mut e, 0x69, false), Some(Accion::Persona(8)));
        assert_eq!(tecla(&mut e, 0x30, false), Some(Accion::Nadie));
        assert_eq!(tecla(&mut e, 0x41, false), None, "una letra no hace nada");
        e.paso = Paso::Preparar;
        assert_eq!(
            tecla(&mut e, 0x31, false),
            None,
            "preparando, 1 es escribir"
        );
    }

    /// `cargo test -p pixpin --bin pixpinmax muestra_de_la_conversacion --
    /// --ignored --nocapture`: preparar y grabar.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_de_la_conversacion() {
        let textos = Catalogo::nuevo(Idioma::Espanol);
        let marco = Rect {
            x: 0,
            y: 0,
            ancho: 920,
            alto: 660,
        };
        let mut nombres = vec![String::new(); MAXIMO];
        nombres[0] = "Ana".into();
        nombres[1] = "Luis".into();
        let mut e = Estado {
            paso: Paso::Preparar,
            cuantos: 3,
            nombres,
            escribiendo: Some(1),
            grabadora: None,
            abriendo: None,
            cambios: vec![],
            hablando: None,
            botones: Botones::default(),
            aviso: None,
        };
        crate::ventanita::muestra("conversacion-preparar", 920, 660, |p, _| {
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
        e.paso = Paso::Grabar;
        e.hablando = Some(1);
        crate::ventanita::muestra("conversacion-grabar", 920, 660, |p, _| {
            pintar_todo(&mut e, p, marco, 1.0, &textos)
        });
    }

    #[test]
    fn los_rotulos_de_la_conversacion_estan_en_los_dos_idiomas() {
        for idioma in [Idioma::Espanol, Idioma::Ingles] {
            let c = Catalogo::nuevo(idioma);
            for k in [
                "conversacion-titulo",
                "conversacion-explicacion",
                "conversacion-cuantos",
                "conversacion-empezar",
                "conversacion-terminar",
                "conversacion-hablando",
                "conversacion-teclas",
                "conversacion-transcribiendo",
            ] {
                assert_ne!(c.t(k), k, "falta {k} en {idioma:?}");
            }
        }
    }
}
