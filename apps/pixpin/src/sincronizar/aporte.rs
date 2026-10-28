//! **«Captura para la grabación del móvil»** (28-oct-2026, el usuario: «las
//! fotos tienen que ser BIDIRECCIONALES con la PC: la PC no puede tomar
//! fotos, pero si CAPTURAS, asi que normalmente la PC tambien puede aportar
//! capturas»).
//!
//! Cuando el movil graba y se lo dice a este equipo (`camara_remota` con su
//! grabacion, ver `camara::RecepcionPc::anuncio_ajeno`), sale un boton
//! flotante pequeno, siempre encima, arriba a la derecha: un clic captura el
//! monitor donde esta el boton, a resolucion completa y en PNG, y se la
//! manda al movil con el minuto de SU grabacion (su reloj pasado al de aqui)
//! y la hora de aqui, por el mismo `suelto` de la camara. El boton no sale
//! en la captura (se excluye y se esconde un instante). Si el movil no
//! contesta, la captura espera en disco y se reintenta al oirlo de nuevo.
//! El boton se va solo cuando el movil acaba o deja de oirse
//! (`camara_del_movil::AJENA_CADUCA_MS`); la × lo cierra hasta la grabacion
//! siguiente.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_geom::Rect;
use pixpin_proyecto::camara_del_movil::Ajena;
use pixpin_render::{Color, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_sincro::camara::FotoDeCamara;
use pixpin_store::Catalogo;
use serde::{Deserialize, Serialize};

use crate::overlay::Recursos;
use crate::ventanita::{APAGADO, Botones, FONDO, ROJO, TEXTO, VERDE};

static ABIERTO: AtomicBool = AtomicBool::new(false);
/// La × de esta grabacion: no vuelve a salir hasta la siguiente.
static CERRADO_PARA: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

const ANCHO: u32 = 300;
const ALTO: u32 = 56;
const MARGEN: i32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Capturar,
    Cerrar,
}

/// **Saca el boton** si no esta ya (en su propio hilo).
pub(crate) fn lanzar(raiz: &Path) {
    let ahora = super::ahora_ms();
    let grabacion = super::camara::camaras(raiz)
        .lock()
        .ok()
        .and_then(|c| c.ajena(ahora).map(|a| a.anuncio.grabacion.clone()));
    let Some(grabacion) = grabacion else {
        return;
    };
    if CERRADO_PARA.lock().is_ok_and(|g| *g == grabacion) {
        return;
    }
    reintentar(raiz);
    if ABIERTO.swap(true, Ordering::SeqCst) {
        return;
    }
    let raiz = raiz.to_path_buf();
    let hecho = std::thread::Builder::new()
        .name("captura-al-movil".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            if let Err(e) =
                crate::dispositivo_perdido::con_recursos("captura al movil", |r| flotar(r, &raiz))
            {
                tracing::warn!(?e, "no se pudo sacar el boton de captura al movil");
            }
            ABIERTO.store(false, Ordering::SeqCst);
        });
    if hecho.is_err() {
        ABIERTO.store(false, Ordering::SeqCst);
    }
}

fn ajena(raiz: &Path) -> Option<Ajena> {
    let ahora = super::ahora_ms();
    super::camara::camaras(raiz)
        .lock()
        .ok()
        .and_then(|c| c.ajena(ahora).cloned())
}

/// Lo que dice el boton debajo del rotulo un rato tras un clic.
enum Dicho {
    Enviada(String),
    Pendiente,
    Fallo,
}

fn flotar(recursos: &Recursos, raiz: &Path) -> Result<()> {
    let textos = Catalogo::nuevo(crate::salida::idioma_actual());
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores.principal().context("sin monitor")?.to_owned();
    let escala = monitor.escala_por_cien as f32 / 100.0;
    let (w, h) = (
        (ANCHO as f32 * escala) as u32,
        (ALTO as f32 * escala) as u32,
    );
    let margen = (MARGEN as f32 * escala) as i32;
    let marco = Rect {
        x: monitor.area_trabajo.derecha() - w as i32 - margen,
        y: monitor.area_trabajo.y + margen,
        ancho: w,
        alto: h,
    };
    let ventana = VentanaOverlay::nueva(marco).context("sin ventana")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(&motor, &recursos.d3d(), ventana.handle(), w, h)
        .context("sin superficie")?;
    ventana.poner_sin_activar();
    ventana.excluir_de_capturas();
    ventana.mostrar();
    ventana.traer_encima();

    let (tx, rx) = std::sync::mpsc::channel::<Dicho>();
    let mut botones: Botones<Accion> = Botones::default();
    let mut dicho: Option<(Dicho, Instant)> = None;
    let mut mirado = Instant::now() - Duration::from_secs(5);
    let mut actual = ajena(raiz);
    loop {
        pixpin_shell::overlay::bombear_pendientes();
        if mirado.elapsed() >= Duration::from_millis(500) {
            mirado = Instant::now();
            actual = ajena(raiz);
        }
        let Some(a) = actual.clone() else { break };
        let mut accion = None;
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            match ev {
                EventoOverlay::Cerrar => accion = Some(Accion::Cerrar),
                EventoOverlay::RatonMovido(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                }
                EventoOverlay::BotonSoltado(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    accion = botones.bajo_el_raton().or(Some(Accion::Capturar));
                }
                _ => {}
            }
        }
        match accion {
            Some(Accion::Cerrar) => {
                if let Ok(mut g) = CERRADO_PARA.lock() {
                    *g = a.anuncio.grabacion.clone();
                }
                break;
            }
            Some(Accion::Capturar) => {
                // Fuera de la foto: excluido y, por si el sistema no sabe
                // excluir, escondido el instante de la captura.
                ventana.ocultar();
                pixpin_shell::overlay::bombear_pendientes();
                pixpin_shell::overlay::esperar_composicion();
                let ahora = super::ahora_ms();
                let png = capturar(&monitores, marco);
                ventana.mostrar();
                match png {
                    Ok(png) => {
                        let raiz = raiz.to_path_buf();
                        let tx = tx.clone();
                        let suya = a.clone();
                        let _ = std::thread::Builder::new()
                            .name("captura-al-movil-envio".into())
                            .spawn(move || {
                                let _ = tx.send(enviar_nueva(&raiz, &suya, ahora, png));
                            });
                    }
                    Err(e) => {
                        tracing::warn!(?e, "no se pudo capturar para el movil");
                        dicho = Some((Dicho::Fallo, Instant::now()));
                    }
                }
            }
            None => {}
        }
        if let Ok(d) = rx.try_recv() {
            dicho = Some((d, Instant::now()));
        }
        if dicho
            .as_ref()
            .is_some_and(|(_, t)| t.elapsed() > Duration::from_secs(3))
        {
            dicho = None;
        }
        pintar(
            &superficie,
            &motor,
            &mut botones,
            &textos,
            &a,
            dicho.as_ref().map(|(d, _)| d),
            escala,
        );
        std::thread::sleep(Duration::from_millis(33));
    }
    ventana.ocultar();
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn pintar(
    superficie: &Superficie,
    motor: &pixpin_render::MotorRender,
    botones: &mut Botones<Accion>,
    textos: &Catalogo,
    a: &Ajena,
    dicho: Option<&Dicho>,
    e: f32,
) {
    let Ok(destino) = superficie.empezar(motor) else {
        return;
    };
    botones.vaciar();
    let (w, h) = (ANCHO as f32 * e, ALTO as f32 * e);
    let _ = motor.dibujar(&destino, |p| {
        p.limpiar_transparente();
        let todo = RectF {
            x: 0.0,
            y: 0.0,
            ancho: w,
            alto: h,
        };
        p.rellenar_redondeado(todo, 14.0 * e, Color { a: 0.94, ..FONDO });
        // El punto rojo de «grabando» del movil.
        p.circulo((18.0 * e, h / 2.0), 6.0 * e, ROJO);
        let titulo = textos.t("aporte-captura-al-movil");
        p.texto(&titulo, 32.0 * e, 9.0 * e, 14.0 * e, TEXTO);
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("movil", a.de_nombre.clone());
        args.set(
            "minuto",
            pixpin_proyecto::camara_del_movil::minuto(a.reloj.ms_en(super::ahora_ms())),
        );
        let (pie, color) = match dicho {
            Some(Dicho::Enviada(m)) => {
                let mut x = fluent_bundle::FluentArgs::new();
                x.set("minuto", m.clone());
                (textos.t_args("aporte-enviada", &x), VERDE)
            }
            Some(Dicho::Pendiente) => (textos.t("aporte-pendiente"), APAGADO),
            Some(Dicho::Fallo) => (textos.t("aporte-fallo"), ROJO),
            None => (textos.t_args("aporte-de-la-grabacion", &args), APAGADO),
        };
        p.texto(&pie, 32.0 * e, 30.0 * e, 12.0 * e, color);
        // La ×, a la derecha.
        let x = RectF {
            x: w - 34.0 * e,
            y: (h - 26.0 * e) / 2.0,
            ancho: 26.0 * e,
            alto: 26.0 * e,
        };
        p.texto("×", x.x + 8.0 * e, x.y + 3.0 * e, 16.0 * e, APAGADO);
        botones.zona(x, Accion::Cerrar);
    });
    let _ = superficie.presentar();
}

/// La foto del monitor donde esta el boton, entera, en PNG.
fn capturar(
    monitores: &pixpin_geom::monitores::DisposicionMonitores,
    marco: Rect,
) -> Result<Vec<u8>> {
    let centro = pixpin_geom::Punto {
        x: marco.x + marco.ancho as i32 / 2,
        y: marco.y + marco.alto as i32 / 2,
    };
    let m = monitores
        .monitor_en(centro)
        .or_else(|| monitores.principal())
        .context("sin monitor")?;
    let d = pixpin_capture::Dispositivo::nuevo()?;
    let inst = pixpin_capture::capturar_monitor(&d, m.id, m.area)?;
    let imagen = pixpin_capture::a_imagen(&d, &inst)?;
    Ok(pixpin_codec::imagen::codificar_png(&imagen)?)
}

// ------------------------------------------------------------ el envio

/// Lo que espera en disco junto a cada captura sin mandar.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Pendiente {
    de_id: String,
    grabacion: String,
    ms: i64,
    tomada_en: i64,
    reloj: i64,
}

fn carpeta(raiz: &Path) -> PathBuf {
    super::carpeta_sincro(raiz)
        .join("camara-movil")
        .join("al-movil")
}

fn enviar_nueva(raiz: &Path, a: &Ajena, ahora: i64, png: Vec<u8>) -> Dicho {
    let clave = format!("pc-{}-{ahora}", crate::letras_que_viajan::aparato(raiz));
    let foto = a.foto(ahora, &clave);
    let dir = carpeta(raiz);
    let fichero = dir.join(format!("{clave}.png"));
    let guardado = std::fs::create_dir_all(&dir)
        .and_then(|_| std::fs::write(&fichero, &png))
        .and_then(|_| {
            let p = Pendiente {
                de_id: a.de_id.clone(),
                grabacion: foto.grabacion.clone(),
                ms: foto.ms.unwrap_or(0),
                tomada_en: foto.tomada_en,
                reloj: foto.reloj,
            };
            std::fs::write(
                fichero.with_extension("json"),
                serde_json::to_vec(&p).unwrap_or_default(),
            )
        });
    if let Err(e) = guardado {
        tracing::warn!(?e, "la captura para el movil no se pudo guardar");
        return Dicho::Fallo;
    }
    match mandar(raiz, &a.de_id, &fichero, &foto) {
        Ok(ms) => {
            olvidar(&fichero);
            Dicho::Enviada(pixpin_proyecto::camara_del_movil::minuto(ms))
        }
        Err(e) => {
            tracing::info!(%e, "la captura para el movil espera");
            Dicho::Pendiente
        }
    }
}

fn olvidar(fichero: &Path) {
    let _ = std::fs::remove_file(fichero);
    let _ = std::fs::remove_file(fichero.with_extension("json"));
}

/// **Manda lo que quedo pendiente** (el movil no contesto entonces), en
/// un hilo. Se llama al oir de nuevo al movil.
pub(crate) fn reintentar(raiz: &Path) {
    let Ok(d) = std::fs::read_dir(carpeta(raiz)) else {
        return;
    };
    let pendientes: Vec<(PathBuf, Pendiente)> = d
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("png"))
        .filter_map(|p| {
            let j: Pendiente =
                serde_json::from_slice(&std::fs::read(p.with_extension("json")).ok()?).ok()?;
            Some((p, j))
        })
        .collect();
    if pendientes.is_empty() {
        return;
    }
    let raiz = raiz.to_path_buf();
    let _ = std::thread::Builder::new()
        .name("captura-al-movil-reintento".into())
        .spawn(move || {
            for (f, p) in pendientes {
                let clave = f
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let foto = FotoDeCamara {
                    grabacion: p.grabacion,
                    ms: Some(p.ms),
                    tomada_en: p.tomada_en,
                    reloj: p.reloj,
                    clave,
                    ..Default::default()
                };
                match mandar(&raiz, &p.de_id, &f, &foto) {
                    Ok(_) => olvidar(&f),
                    // Lo que el movil niega (ya no es de una grabacion suya)
                    // no se reintenta para siempre.
                    Err(e)
                        if e.to_string().contains("no cae") || e.to_string().contains("ya no") =>
                    {
                        olvidar(&f)
                    }
                    Err(_) => {}
                }
            }
        });
}

/// La captura `fichero` al movil `de_id`, por el canal de la camara.
fn mandar(raiz: &Path, de_id: &str, fichero: &Path, foto: &FotoDeCamara) -> Result<i64> {
    let otro = super::al_movil::aparatos_de(raiz)
        .into_iter()
        .find(|o| o.id == de_id)
        .context("ese movil ya no esta en el grupo")?;
    let Some(mut c) = super::camara::abrir_con(raiz, &otro)? else {
        anyhow::bail!("el movil no sabe de la camara");
    };
    let nombre = fichero
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "captura.png".into());
    let ms = c.mandar_foto(fichero, &nombre, foto)?;
    c.adios();
    Ok(ms)
}
