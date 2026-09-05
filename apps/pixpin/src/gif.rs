//! Grabar una region de la pantalla como GIF animado (P5).
//!
//! El original trae FFmpeg y x264 para esto, que ademas de pesar obligan a
//! decidir sobre la GPL. Un GIF es una paleta de 256 colores y compresion
//! LZW, y eso ya lo escribimos nosotros en `pixpin-codec::gif`. Aqui solo
//! esta el bucle: capturar la region una y otra vez con el overlay ya
//! oculto, exactamente como hace la captura con scroll.
//!
//! Los topes no son adorno. Un fotograma es RGBA sin comprimir: una region
//! de 800x600 ocupa 1,9 MB, asi que doce segundos a diez por segundo son
//! 230 MB. En el equipo suelo, con 4 GB, ese es el limite entre grabar
//! y tirar la sesion al disco de intercambio.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_codec::ImagenRgba;
use pixpin_geom::Rect;

use crate::overlay::Recursos;

/// Fotogramas por segundo. Diez es lo que se usa para ensenar una interfaz:
/// se ve fluido, el fichero no se dispara y el equipo suelo lo aguanta.
const POR_SEGUNDO: u32 = 10;
/// Lo que dura cada paso del bucle.
const PASO: Duration = Duration::from_millis(1000 / POR_SEGUNDO as u64);
/// Tope de reloj. Un GIF mas largo que esto no es un GIF, es un video.
///
/// Son doce y no veinte por el tope de memoria de abajo: con una region de
/// 800x600, veinte segundos pedirian 384 MB y la grabacion se cortaria sola
/// antes de tiempo. Vale mas prometer doce y cumplirlos. Lo caza la prueba
/// `los_topes_dejan_grabar_algo_util_en_el_equipo_suelo`.
const TIEMPO_MAXIMO: Duration = Duration::from_secs(12);
/// Tope de memoria de los fotogramas en crudo, antes de comprimir.
const MEMORIA_MAXIMA: usize = 256 * 1024 * 1024;

/// Por que termino la grabacion, para el registro y para el usuario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fin {
    /// El usuario pulso Escape: lo normal.
    Escape,
    Tiempo,
    Memoria,
}

pub struct Grabacion {
    pub fotogramas: Vec<ImagenRgba>,
    pub fin: Fin,
}

/// Graba la region hasta que el usuario pulse Escape o se llegue a un tope.
///
/// Devuelve `None` si no se llego a capturar nada aprovechable. El overlay
/// ya esta oculto cuando esto corre: lo que se graba es la pantalla de
/// verdad, no nuestra ventana.
pub fn ejecutar_grabacion(recursos: &mut Recursos, region: Rect) -> Result<Option<Grabacion>> {
    if region.ancho == 0 || region.alto == 0 {
        return Ok(None);
    }
    let disposicion = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let monitor = disposicion
        .monitores()
        .iter()
        .find(|m| m.area.interseccion(region).is_some())
        .or_else(|| disposicion.principal())
        .context("sin monitor para la region")?
        .to_owned();

    let bytes_por_fotograma = region.ancho as usize * region.alto as usize * 4;
    let inicio = Instant::now();
    let mut fotogramas: Vec<ImagenRgba> = Vec::new();
    let mut siguiente = inicio;

    // El aviso de que se esta grabando. Sin el, el usuario no distingue una
    // grabacion en marcha de un cuelgue: fue exactamente lo que paso la
    // primera vez que se probo esto.
    let aviso = Aviso::nuevo(recursos, region, monitor.escala_por_cien);
    let mut segundo_pintado = u64::MAX;

    let fin = loop {
        // Que la aplicacion siga viva mientras dura la grabacion: sin esto
        // Windows la marca como «no responde» y ni siquiera se pinta el
        // aviso de arriba.
        pixpin_shell::overlay::bombear_pendientes();
        let segundo = inicio.elapsed().as_secs();
        if segundo != segundo_pintado {
            segundo_pintado = segundo;
            if let Some(a) = &aviso {
                a.pintar(segundo, TIEMPO_MAXIMO.as_secs());
            }
        }
        // Escape se sondea, no llega por ningun WndProc: durante la
        // grabacion no hay ninguna ventana nuestra con foco, igual que en
        // la captura con scroll.
        if pixpin_shell::escape_pulsado() {
            break Fin::Escape;
        }
        if inicio.elapsed() > TIEMPO_MAXIMO {
            break Fin::Tiempo;
        }
        if (fotogramas.len() + 1) * bytes_por_fotograma > MEMORIA_MAXIMA {
            break Fin::Memoria;
        }

        let t = Instant::now();
        match crate::scroll::capturar(recursos, &monitor, region) {
            Ok(f) => fotogramas.push(f),
            // Un fotograma perdido no tira la grabacion entera: se anota y
            // se sigue, que es mejor que perder los veinte segundos.
            Err(e) => tracing::warn!(?e, "fotograma perdido durante la grabacion"),
        }
        // El ritmo va contra un instante ABSOLUTO, no durmiendo intervalos:
        // dormir «lo que falta» acumula el error de cada vuelta y el GIF
        // acaba yendo mas lento de lo que declara. Es lo que hace LICEcap.
        // Si una captura tardo mas que el paso, no se intenta recuperar el
        // tiempo perdido encadenando capturas: se salta al siguiente hueco,
        // que es lo unico que evita que la cola crezca sin fin.
        let _ = t;
        siguiente += PASO;
        let ahora = Instant::now();
        if siguiente <= ahora {
            siguiente = ahora + PASO;
        } else {
            std::thread::sleep(siguiente - ahora);
        }
    };

    tracing::info!(
        fotogramas = fotogramas.len(),
        ms = inicio.elapsed().as_millis() as u64,
        ?fin,
        "grabacion terminada"
    );
    // Con menos de dos no hay animacion que ensenar.
    if fotogramas.len() < 2 {
        return Ok(None);
    }
    Ok(Some(Grabacion { fotogramas, fin }))
}

/// El cartelito que dice que se esta grabando y como parar.
///
/// Es una ventana propia, pasante a los clics, encima de la zona que se
/// graba pero FUERA de ella: si cayera dentro saldria en el GIF.
struct Aviso {
    ventana: pixpin_shell::overlay::VentanaOverlay,
    superficie: pixpin_render::Superficie,
    motor: std::rc::Rc<pixpin_render::MotorRender>,
    escala: f32,
}

/// Alto del cartel en pixeles logicos.
const AVISO_ALTO: u32 = 34;
/// Ancho del cartel en pixeles logicos: lo justo para «● 12 s · Esc para
/// parar» sin que tape media pantalla.
const AVISO_ANCHO: u32 = 230;

impl Aviso {
    /// `None` si no se pudo crear: un GIF sin cartel es peor, pero mucho
    /// mejor que no poder grabar.
    fn nuevo(recursos: &Recursos, region: Rect, escala_por_cien: u32) -> Option<Aviso> {
        let escala = escala_por_cien as f32 / 100.0;
        let (ancho, alto) = (
            (AVISO_ANCHO as f32 * escala) as u32,
            (AVISO_ALTO as f32 * escala) as u32,
        );
        // Justo encima de la zona; si no cabe arriba, justo debajo. Nunca
        // dentro: se grabaria a si mismo.
        let y = if region.y >= alto as i32 + 4 {
            region.y - alto as i32 - 4
        } else {
            region.y + region.alto as i32 + 4
        };
        let marco = Rect {
            x: region.x,
            y,
            ancho,
            alto,
        };
        let ventana = pixpin_shell::overlay::VentanaOverlay::nueva(marco).ok()?;
        let motor = recursos.motor();
        let superficie = pixpin_render::Superficie::nueva(
            &motor,
            &recursos.d3d(),
            ventana.handle(),
            ancho,
            alto,
        )
        .ok()?;
        ventana.poner_pasante(true);
        ventana.mostrar();
        Some(Aviso {
            ventana,
            superficie,
            motor,
            escala,
        })
    }

    fn pintar(&self, segundo: u64, tope: u64) {
        let Ok(destino) = self.superficie.empezar(&self.motor) else {
            return;
        };
        let e = self.escala;
        let ancho = AVISO_ANCHO as f32 * e;
        let alto = AVISO_ALTO as f32 * e;
        let _ = self.motor.dibujar(&destino, |p| {
            p.limpiar_transparente();
            p.rellenar_redondeado(
                pixpin_render::RectF {
                    x: 0.0,
                    y: 0.0,
                    ancho,
                    alto,
                },
                8.0 * e,
                pixpin_render::Color {
                    r: 0.08,
                    g: 0.08,
                    b: 0.09,
                    a: 0.92,
                },
            );
            // El punto rojo parpadea cada segundo: se ve de reojo que
            // sigue vivo, sin tener que leer el numero.
            if segundo % 2 == 0 {
                p.rellenar_redondeado(
                    pixpin_render::RectF {
                        x: 11.0 * e,
                        y: (alto - 10.0 * e) / 2.0,
                        ancho: 10.0 * e,
                        alto: 10.0 * e,
                    },
                    5.0 * e,
                    pixpin_render::Color {
                        r: 0.90,
                        g: 0.20,
                        b: 0.18,
                        a: 1.0,
                    },
                );
            }
            p.texto(
                &format!("{segundo} s de {tope} · Esc para parar"),
                28.0 * e,
                (alto - 15.0 * e) / 2.0,
                13.0 * e,
                pixpin_render::Color {
                    r: 0.95,
                    g: 0.95,
                    b: 0.96,
                    a: 1.0,
                },
            );
        });
        let _ = self.superficie.presentar();
    }
}

impl Drop for Aviso {
    fn drop(&mut self) {
        // Fuera antes de codificar: si no, el cartel se quedaria en pantalla
        // durante el rato que cuesta comprimir el GIF.
        self.ventana.ocultar();
    }
}

/// Las centesimas de segundo que hay que declarar en el GIF para el ritmo
/// al que se grabo. Se calcula y no se escribe a mano para que cambiar
/// `POR_SEGUNDO` no deje el GIF yendo a otra velocidad.
pub fn centesimas_por_fotograma() -> u16 {
    (100 / POR_SEGUNDO).max(1) as u16
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_ritmo_declarado_cuadra_con_el_de_grabacion() {
        // Caso negativo del despiste clasico: si esto se escribiera a mano
        // y alguien cambiara los fotogramas por segundo, el GIF saldria a
        // otra velocidad y nadie lo notaria hasta verlo.
        assert_eq!(centesimas_por_fotograma() as u32 * POR_SEGUNDO, 100);
        assert_eq!(PASO.as_millis() as u32 * POR_SEGUNDO, 1000);
    }

    #[test]
    fn los_topes_dejan_grabar_algo_util_en_el_equipo_suelo() {
        // Una region de 800x600 tiene que caber entera en el tope de
        // memoria durante los veinte segundos completos; si no, la
        // grabacion se cortaria sola antes de tiempo y el tope de reloj
        // seria mentira.
        let bytes = 800 * 600 * 4;
        let cuantos = TIEMPO_MAXIMO.as_secs() as usize * POR_SEGUNDO as usize;
        assert!(
            cuantos * bytes <= MEMORIA_MAXIMA,
            "{} MB para {cuantos} fotogramas",
            cuantos * bytes / 1024 / 1024
        );
    }
}
