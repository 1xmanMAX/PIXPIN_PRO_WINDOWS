//! **La llamada secreta** (B11, v0.72 del movil): una nota de voz con hora
//! suena como si llamaran, y al contestar se oye **por el aparato de las
//! llamadas**.
//!
//! Es `pin/LlamadaSecretaActivity.kt` y el trozo de `RecordatorioReceiver`
//! que la dispara: **no hay campo nuevo**, es `clase = VOZ` con `recuerdaEn`.
//! Al llegar la hora:
//!
//! - **Sonando**: una ventanita «Llamada entrante» encima de todo, con el
//!   nombre de la nota, Contestar y Colgar, y el tono de llamada de Windows
//!   (`C:\Windows\Media\Ring01.wav`) por los altavoces, en bucle. Si nadie
//!   contesta en [`SEGUNDOS_SONANDO`], se corta y queda dicho como «llamada
//!   perdida»: un globo (`pixpin_shell::aviso`) y un pin, que es lo que de
//!   verdad avisa.
//! - **Contestada**: la nota por **la salida de comunicaciones** de Windows
//!   (`eCommunications`: los cascos con microfono, el auricular), que es el
//!   `MODE_IN_COMMUNICATION` del movil: nadie alrededor oye el recado. Hay
//!   altavoz, como en el movil, y el reloj de la llamada. Al acabar la nota,
//!   cuelga sola.
//!
//! No toca el audio del sistema: pedir la salida de llamadas a Media
//! Foundation no cambia nada de lo que el usuario tiene puesto.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use pixpin_audio::{Rol, Salida};
use pixpin_geom::Rect;
use pixpin_proyecto::cuaderno::{Clase, Cuaderno};
use pixpin_render::{Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};

use crate::caja_dibujo::hex;
use crate::overlay::Recursos;
use crate::ventanita::{Botones, ROJO, TEXTO, VERDE, centrado_en};

/// Cuanto suena antes de darla por perdida (`SEGUNDOS_SONANDO = 45`).
pub const SEGUNDOS_SONANDO: u64 = 45;

/// Lo que tarda en colgar sola tras acabar la nota (`postDelayed(..., 900)`).
const COLGAR_TRAS_MS: u64 = 900;

/// Los tonos de llamada que trae Windows, por orden de preferencia.
const TONOS: [&str; 3] = ["Ring01.wav", "Windows Ringin.wav", "Ring02.wav"];

/// Una llamada que sonar: el audio y quien «llama».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Llamada {
    pub audio: PathBuf,
    pub nombre: String,
}

/// **Si un recordatorio vencido es una llamada secreta**: el mensaje `id`
/// de la conversacion en `carpeta` es una nota de voz y su audio esta en
/// este equipo. Si no, `None`, y el recordatorio sale como siempre (un pin).
pub fn de_un_recordatorio(carpeta: &Path, id: &str) -> Option<Llamada> {
    let c = Cuaderno::leer_de(carpeta).ok()?;
    de_un_cuaderno(&c, carpeta, id)
}

/// Lo mismo sobre un cuaderno ya leido (para las pruebas).
pub fn de_un_cuaderno(c: &Cuaderno, carpeta: &Path, id: &str) -> Option<Llamada> {
    let m = c.mensajes.iter().find(|m| m.id == id)?;
    if m.clase != Some(Clase::Voz) {
        return None;
    }
    // `carpeta` es `<raiz>/proyectos/<id>`: de ahi salen los dos para
    // resolver la ruta como el chat (tambien las `pixpin:files/…`).
    let proyecto = carpeta.file_name()?.to_string_lossy().into_owned();
    let raiz = carpeta.parent()?.parent()?;
    let audio = pixpin_proyecto::vista::ruta_real(raiz, &proyecto, m.ruta.as_deref()?)?;
    if !audio.is_file() {
        return None;
    }
    Some(Llamada {
        audio,
        nombre: nombre_de_la_llamada(&m.nombre),
    })
}

/// «Quien llama»: el nombre del fichero sin extension
/// (`nombre.substringBeforeLast('.')`), o vacio si no tiene; quien pinta
/// pone entonces «Llamada».
pub fn nombre_de_la_llamada(nombre: &str) -> String {
    match nombre.rsplit_once('.') {
        Some((antes, _)) => antes.to_string(),
        None => nombre.to_string(),
    }
}

/// Los rotulos, ya traducidos en el hilo principal (el `Catalogo` no cruza
/// hilos).
#[derive(Debug, Clone)]
pub struct Rotulos {
    pub entrante: String,
    pub contestar: String,
    pub colgar: String,
    pub altavoz: String,
    pub llamada: String,
    pub perdida: String,
}

/// Las llamadas que nadie contesto, esperando a que el bucle principal saque
/// su pin (es el que tiene los pines).
static PERDIDAS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Las perdidas desde la ultima vez, y deja la lista vacia.
pub fn tomar_perdidas() -> Vec<String> {
    PERDIDAS
        .lock()
        .map(|mut v| std::mem::take(&mut *v))
        .unwrap_or_default()
}

/// **Hace sonar la llamada**, en su propio hilo. `hwnd_app` es la ventana
/// principal como entero (`HWND` no cruza hilos): a ella se le da el toque
/// si se pierde, y su icono de la bandeja lleva el globo.
pub fn lanzar(llamada: Llamada, rotulos: Rotulos, hwnd_app: isize) {
    let hecho = std::thread::Builder::new()
        .name("llamada-secreta".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let r = Recursos::nuevos().and_then(|r| sonar(&r, &llamada, &rotulos, hwnd_app));
            if let Err(e) = r {
                tracing::warn!(?e, "no se pudo abrir la llamada secreta");
            }
        });
    if let Err(e) = hecho {
        tracing::warn!(?e, "no se pudo lanzar el hilo de la llamada");
    }
}

/// El tono de llamada de Windows que haya en este equipo.
pub fn tono() -> Option<PathBuf> {
    let windir = std::env::var_os("WINDIR").map(PathBuf::from)?;
    TONOS
        .iter()
        .map(|t| windir.join("Media").join(t))
        .find(|p| p.is_file())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Contestar,
    Colgar,
    Altavoz,
}

/// En que punto esta la llamada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fase {
    Sonando,
    Contestada,
}

/// Que hacer con el tiempo que pasa (pura, para probarla): colgar sola si
/// suena demasiado o si la nota ya acabo hace un rato.
pub fn toca_colgar(fase: Fase, desde_ms: u64, ahora_ms: u64, acabo_en: Option<u64>) -> bool {
    match fase {
        Fase::Sonando => ahora_ms.saturating_sub(desde_ms) >= SEGUNDOS_SONANDO * 1000,
        Fase::Contestada => acabo_en.is_some_and(|t| ahora_ms.saturating_sub(t) >= COLGAR_TRAS_MS),
    }
}

/// La ventanita: abajo a la derecha del monitor del raton, encima de todo.
fn marco_de_la_llamada(area: Rect, escala_por_cien: u32) -> Rect {
    let e = |v: u32| (v * escala_por_cien / 100) as i32;
    let (w, h) = (e(340), e(460));
    Rect {
        x: area.x + area.ancho as i32 - w - e(24),
        y: area.y + area.alto as i32 - h - e(24),
        ancho: w as u32,
        alto: h as u32,
    }
}

fn sonar(recursos: &Recursos, llamada: &Llamada, rotulos: &Rotulos, hwnd_app: isize) -> Result<()> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = monitores
        .monitor_en(pixpin_shell::posicion_del_cursor())
        .or_else(|| monitores.principal())
        .context("sin monitor")?
        .to_owned();
    let escala = monitor.escala_por_cien as f32 / 100.0;
    let marco = marco_de_la_llamada(monitor.area_trabajo, monitor.escala_por_cien);
    let ventana = VentanaOverlay::nueva(marco).context("no se pudo abrir la llamada")?;
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        marco.ancho,
        marco.alto,
    )
    .context("sin superficie para la llamada")?;
    ventana.mostrar();
    ventana.traer_encima();

    // El tono, por la salida normal y en bucle: una llamada se oye en el
    // cuarto; lo secreto es lo que se dice despues.
    let mut timbre = tono().and_then(|t| Salida::abrir_por(&t, Rol::Normal, true).ok());
    if let Some(t) = &timbre {
        let _ = t.tocar(1.0);
    }
    let nombre = if llamada.nombre.trim().is_empty() {
        rotulos.llamada.clone()
    } else {
        llamada.nombre.clone()
    };
    let mut fase = Fase::Sonando;
    let mut desde = pixpin_shell::entorno::ahora_utc_ms() as u64;
    let mut voz: Option<Salida> = None;
    let mut en_altavoz = false;
    let mut acabo_en: Option<u64> = None;
    // Al pasar al altavoz: por donde seguir en cuanto la otra salida cargue.
    let mut salto: Option<i64> = None;
    let mut botones: Botones<Accion> = Botones::default();
    let mut vivo = true;
    while vivo {
        pixpin_shell::overlay::bombear_pendientes();
        let ahora = pixpin_shell::entorno::ahora_utc_ms() as u64;
        let mut pulsado = None;
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            match ev {
                EventoOverlay::Cerrar => vivo = false,
                EventoOverlay::RatonMovido(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                }
                EventoOverlay::BotonPulsado(p) => {
                    botones.raton = ((p.x - marco.x) as f32, (p.y - marco.y) as f32);
                    pulsado = botones.bajo_el_raton();
                }
                EventoOverlay::Tecla { vk: 0x0D, .. } if fase == Fase::Sonando => {
                    pulsado = Some(Accion::Contestar)
                }
                EventoOverlay::Tecla { vk: 0x1B, .. } => pulsado = Some(Accion::Colgar),
                _ => {}
            }
        }
        match pulsado {
            Some(Accion::Colgar) => vivo = false,
            Some(Accion::Contestar) if fase == Fase::Sonando => {
                timbre = None;
                match Salida::abrir_por(&llamada.audio, Rol::Comunicaciones, false) {
                    Ok(s) => {
                        let _ = s.tocar(1.0);
                        voz = Some(s);
                        fase = Fase::Contestada;
                        desde = ahora;
                    }
                    Err(e) => {
                        tracing::warn!(?e, "la llamada no pudo abrir su nota");
                        vivo = false;
                    }
                }
            }
            Some(Accion::Altavoz) if fase == Fase::Contestada => {
                // Otra salida es otro motor: se abre en la otra y se sigue
                // por donde iba.
                let por = voz.as_ref().map(|v| v.posicion_ms()).unwrap_or(0);
                en_altavoz = !en_altavoz;
                let rol = if en_altavoz {
                    Rol::Normal
                } else {
                    Rol::Comunicaciones
                };
                voz = None;
                if let Ok(s) = Salida::abrir_por(&llamada.audio, rol, false) {
                    // El salto espera a que Media Foundation sepa cuanto
                    // dura (ver abajo): esperarlo aqui pararia la ventana.
                    salto = Some(por);
                    voz = Some(s);
                }
            }
            _ => {}
        }
        if let Some(ms) = salto
            && let Some(v) = &voz
            && (v.lista() || v.fallo())
        {
            v.ir_a_ms(ms);
            let _ = v.tocar(1.0);
            salto = None;
        }
        if let Some(v) = &voz
            && acabo_en.is_none()
            && (v.termino() || v.fallo())
        {
            acabo_en = Some(ahora);
        }
        if toca_colgar(fase, desde, ahora, acabo_en) {
            if fase == Fase::Sonando {
                perdida(&nombre, rotulos, hwnd_app);
            }
            vivo = false;
        }
        if !vivo {
            break;
        }
        if let Ok(d) = superficie.empezar(&motor) {
            let _ = motor.dibujar(&d, |p: &Pintor| {
                pintar(
                    p,
                    marco,
                    escala,
                    &nombre,
                    rotulos,
                    fase,
                    ahora.saturating_sub(desde),
                    en_altavoz,
                    &mut botones,
                )
            });
            let _ = superficie.presentar();
        }
        pixpin_shell::overlay::esperar_eventos(Some(if salto.is_some() { 20 } else { 250 }));
    }
    drop(timbre);
    drop(voz);
    ventana.ocultar();
    Ok(())
}

/// Nadie contesto: el globo ahora y el pin cuando lo saque el bucle
/// principal (`pinTexto("Llamada perdida · $nombre")`).
fn perdida(nombre: &str, rotulos: &Rotulos, hwnd_app: isize) {
    let texto = format!("{} · {nombre}", rotulos.perdida);
    tracing::info!(%nombre, "llamada secreta perdida");
    let hwnd = windows::Win32::Foundation::HWND(hwnd_app as *mut _);
    let mut aviso = pixpin_shell::aviso::Aviso::sobre_la_bandeja(hwnd);
    if let Err(e) = aviso.mostrar(&rotulos.perdida, &texto) {
        tracing::warn!(?e, "no se pudo avisar de la llamada perdida");
    }
    if let Ok(mut v) = PERDIDAS.lock() {
        v.push(texto);
    }
    pixpin_shell::despertar(hwnd);
}

#[allow(clippy::too_many_arguments)]
fn pintar(
    p: &Pintor,
    marco: Rect,
    escala: f32,
    nombre: &str,
    rotulos: &Rotulos,
    fase: Fase,
    llevado_ms: u64,
    en_altavoz: bool,
    botones: &mut Botones<Accion>,
) {
    use pixpin_render::icono::material as mi;
    let (w, h) = (marco.ancho as f32, marco.alto as f32);
    let todo = RectF {
        x: 0.0,
        y: 0.0,
        ancho: w,
        alto: h,
    };
    // El degradado del movil (#101A33 a #05070F), en dos bandas.
    p.limpiar(hex(0x05070F));
    p.rellenar_redondeado(
        RectF {
            alto: h * 0.6,
            ..todo
        },
        0.0,
        hex(0x101A33),
    );
    botones.vaciar();
    let arriba = if fase == Fase::Contestada {
        let s = llevado_ms / 1000;
        format!("{}:{:02}", s / 60, s % 60)
    } else {
        rotulos.entrante.clone()
    };
    let mut blanco_suave = TEXTO;
    blanco_suave.a = 0.7;
    centrado_en(p, &arriba, todo, 30.0 * escala, 15.0 * escala, blanco_suave);
    let centro = (w / 2.0, 130.0 * escala);
    let mut aro = TEXTO;
    aro.a = 0.12;
    p.circulo(centro, 56.0 * escala, aro);
    let lado = 64.0 * escala;
    p.icono(
        &mi::PERSON,
        RectF {
            x: centro.0 - lado / 2.0,
            y: centro.1 - lado / 2.0,
            ancho: lado,
            alto: lado,
        },
        TEXTO,
    );
    centrado_en(p, nombre, todo, 206.0 * escala, 24.0 * escala, TEXTO);

    // Los dos redondos de abajo: colgar a la izquierda y contestar (o el
    // altavoz, contestada) a la derecha, como el movil.
    let radio = 36.0 * escala;
    let y = h - 90.0 * escala;
    let mut redondo =
        |x: f32, icono: &pixpin_render::icono::Icono, fondo, tinta, rotulo: &str, que| {
            p.circulo((x, y), radio, fondo);
            let l = 32.0 * escala;
            p.icono(
                icono,
                RectF {
                    x: x - l / 2.0,
                    y: y - l / 2.0,
                    ancho: l,
                    alto: l,
                },
                tinta,
            );
            let caja = RectF {
                x: x - radio * 1.6,
                y: y + radio + 6.0 * escala,
                ancho: radio * 3.2,
                alto: 20.0 * escala,
            };
            centrado_en(p, rotulo, caja, caja.y, 13.0 * escala, blanco_suave);
            botones.zona(
                RectF {
                    x: x - radio,
                    y: y - radio,
                    ancho: radio * 2.0,
                    alto: radio * 2.0,
                },
                que,
            );
        };
    match fase {
        Fase::Sonando => {
            redondo(
                w * 0.28,
                &mi::CALL_END,
                ROJO,
                TEXTO,
                &rotulos.colgar,
                Accion::Colgar,
            );
            redondo(
                w * 0.72,
                &mi::CALL,
                VERDE,
                TEXTO,
                &rotulos.contestar,
                Accion::Contestar,
            );
        }
        Fase::Contestada => {
            let (fondo, tinta) = if en_altavoz {
                (TEXTO, hex(0x000000))
            } else {
                let mut f = TEXTO;
                f.a = 0.16;
                (f, TEXTO)
            };
            redondo(
                w * 0.28,
                &mi::VOLUME_UP,
                fondo,
                tinta,
                &rotulos.altavoz,
                Accion::Altavoz,
            );
            redondo(
                w * 0.72,
                &mi::CALL_END,
                ROJO,
                TEXTO,
                &rotulos.colgar,
                Accion::Colgar,
            );
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::cuaderno::Mensaje;

    fn carpeta() -> PathBuf {
        let raiz = std::env::temp_dir().join(format!("pixpin-llamada-{}", std::process::id()));
        let c = raiz.join("proyectos").join("p1");
        std::fs::create_dir_all(c.join("archivos")).unwrap();
        c
    }

    #[test]
    fn una_nota_de_voz_con_su_audio_es_una_llamada_con_el_nombre_sin_extension() {
        let c = carpeta();
        std::fs::write(c.join("archivos/recado.m4a"), b"x").unwrap();
        let cu = Cuaderno {
            mensajes: vec![Mensaje {
                id: "m1".into(),
                clase: Some(Clase::Voz),
                ruta: Some("archivos/recado.m4a".into()),
                nombre: "recado.m4a".into(),
                ..Mensaje::default()
            }],
            ..Cuaderno::default()
        };
        let l = de_un_cuaderno(&cu, &c, "m1").expect("es una llamada");
        assert_eq!(l.nombre, "recado");
        assert_eq!(l.audio, c.join("archivos/recado.m4a"));
    }

    #[test]
    fn una_nota_escrita_o_una_voz_sin_su_audio_no_son_llamadas() {
        let c = carpeta();
        let cu = Cuaderno {
            mensajes: vec![
                Mensaje {
                    id: "texto".into(),
                    clase: Some(Clase::Nota),
                    texto: "a las diez".into(),
                    ..Mensaje::default()
                },
                Mensaje {
                    id: "sin-audio".into(),
                    clase: Some(Clase::Voz),
                    ruta: Some("archivos/no-esta.m4a".into()),
                    ..Mensaje::default()
                },
            ],
            ..Cuaderno::default()
        };
        assert!(de_un_cuaderno(&cu, &c, "texto").is_none());
        assert!(
            de_un_cuaderno(&cu, &c, "sin-audio").is_none(),
            "el audio no llego a este equipo"
        );
        assert!(de_un_cuaderno(&cu, &c, "no-existe").is_none());
    }

    #[test]
    fn sin_contestar_se_cuelga_a_los_cuarenta_y_cinco_segundos() {
        assert!(!toca_colgar(Fase::Sonando, 1_000, 45_999, None));
        assert!(toca_colgar(Fase::Sonando, 1_000, 46_000, None));
    }

    #[test]
    fn contestada_cuelga_sola_un_poco_despues_de_acabar_la_nota() {
        assert!(
            !toca_colgar(Fase::Contestada, 0, 999_999, None),
            "mientras suena, no"
        );
        assert!(!toca_colgar(Fase::Contestada, 0, 10_500, Some(10_000)));
        assert!(toca_colgar(Fase::Contestada, 0, 10_900, Some(10_000)));
    }

    #[test]
    fn el_nombre_de_quien_llama_es_el_del_fichero_sin_extension() {
        assert_eq!(nombre_de_la_llamada("recado.largo.m4a"), "recado.largo");
        assert_eq!(nombre_de_la_llamada("sin_extension"), "sin_extension");
        assert_eq!(nombre_de_la_llamada(""), "");
    }

    #[test]
    fn la_ventanita_va_abajo_a_la_derecha_dentro_del_monitor() {
        let area = Rect {
            x: 1920,
            y: 0,
            ancho: 1920,
            alto: 1040,
        };
        let m = marco_de_la_llamada(area, 100);
        assert!(m.x >= area.x && m.x + m.ancho as i32 <= area.x + area.ancho as i32);
        assert!(m.y + m.alto as i32 <= area.alto as i32);
        assert!(m.x > area.x + area.ancho as i32 / 2, "a la derecha");
    }

    #[test]
    fn windows_trae_un_tono_de_llamada() {
        // En un Windows de verdad esta; si no, `tono` dice que no y la
        // llamada suena sin timbre en vez de fallar.
        if let Some(t) = tono() {
            assert!(t.is_file());
        }
    }

    /// `cargo test -p pixpin --bin pixpinmax muestra_de_la_llamada --
    /// --ignored --nocapture`: sonando y contestada.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_de_la_llamada() {
        let r = Rotulos {
            entrante: "Llamada entrante".into(),
            contestar: "Contestar".into(),
            colgar: "Colgar".into(),
            altavoz: "Altavoz".into(),
            llamada: "Llamada".into(),
            perdida: "Llamada perdida".into(),
        };
        let marco = Rect {
            x: 0,
            y: 0,
            ancho: 340,
            alto: 460,
        };
        let mut b = Botones::default();
        crate::ventanita::muestra("llamada-sonando", 340, 460, |p, _| {
            pintar(p, marco, 1.0, "recado", &r, Fase::Sonando, 0, false, &mut b)
        });
        crate::ventanita::muestra("llamada-contestada", 340, 460, |p, _| {
            pintar(
                p,
                marco,
                1.0,
                "recado",
                &r,
                Fase::Contestada,
                83_000,
                true,
                &mut b,
            )
        });
    }

    #[test]
    fn las_perdidas_se_toman_una_vez() {
        PERDIDAS
            .lock()
            .unwrap()
            .push("Llamada perdida · recado".into());
        assert_eq!(
            tomar_perdidas(),
            vec!["Llamada perdida · recado".to_string()]
        );
        assert!(tomar_perdidas().is_empty());
    }
}
