//! La captura con scroll (S1 §4, D75/D76): capturar la region, mandar la
//! rueda a la ventana de debajo, esperar a que asiente, capturar otra vez y
//! coser. La matematica vive en `pixpin_codec::cosido`; aqui solo esta el
//! bucle que la alimenta y sabe cuando parar.
//!
//! El overlay ya esta oculto cuando esto corre: lo que se captura es la
//! ventana real, no la foto congelada.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use pixpin_capture::a_imagen;
use pixpin_codec::{Cosedor, ImagenRgba, Resultado};
use pixpin_geom::{Punto, Rect};

use crate::overlay::Recursos;

/// Muescas de rueda por paso: tres son ~ un tercio de pantalla en la
/// mayoria de aplicaciones, bastante solape para encajar con confianza.
const MUESCAS_POR_PASO: i32 = 3;
/// Alto maximo de la imagen cosida (D76).
const ALTO_MAXIMO: usize = 20_000;
/// Pasos seguidos sin contenido nuevo que dan la captura por terminada.
const PASOS_QUIETOS_PARA_PARAR: u32 = 3;
/// Tope de reloj (D76): una pagina que cambia sola nunca daria "quieto".
const TIEMPO_MAXIMO: Duration = Duration::from_secs(30);
/// Esperar a que el scroll animado asiente: dos capturas iguales seguidas.
const ESPERA_ASENTAR: Duration = Duration::from_millis(60);
const ASENTAR_MAXIMO: Duration = Duration::from_millis(1000);

/// Por que termino la captura, para el log y para el usuario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fin {
    FinalDePagina,
    AltoMaximo,
    Escape,
    Tiempo,
}

/// Recorre la region haciendo scroll y devuelve la pagina cosida, o `None`
/// si no llego a capturar ni un fotograma valido.
pub fn ejecutar_scroll(recursos: &mut Recursos, region: Rect) -> Result<Option<ImagenRgba>> {
    let disposicion =
        pixpin_capture::enumerar_monitores().context("no se pudieron enumerar los monitores")?;
    let centro = Punto {
        x: region.x + region.ancho as i32 / 2,
        y: region.y + region.alto as i32 / 2,
    };
    let monitor = *disposicion
        .monitores()
        .iter()
        .find(|m| m.area.contiene(centro))
        .or_else(|| disposicion.principal())
        .context("sin monitor bajo la region")?;
    // La region se recorta al monitor: `recortar` exige contencion exacta.
    let region = region
        .interseccion(monitor.area)
        .context("la region no toca el monitor")?;

    let t0 = Instant::now();
    let mut cosedor = Cosedor::nuevo(region.ancho, ALTO_MAXIMO);
    let mut quietos = 0u32;
    let mut pasos = 0u32;
    let mut anterior: Option<ImagenRgba> = None;
    let fin;

    loop {
        if pixpin_shell::escape_pulsado() {
            fin = Fin::Escape;
            break;
        }
        if t0.elapsed() > TIEMPO_MAXIMO {
            fin = Fin::Tiempo;
            break;
        }

        let marco = capturar_asentado(recursos, &monitor, region)?;
        // Si la pantalla no cambio desde el paso anterior, la pagina se
        // acabo: el cosedor puede no saberlo (una banda lisa al final es
        // "incierta", no "sin movimiento"), pero los pixeles no mienten.
        let identico = anterior
            .as_ref()
            .is_some_and(|a: &ImagenRgba| a.pixeles == marco.pixeles);
        if identico {
            quietos += 1;
            if quietos >= PASOS_QUIETOS_PARA_PARAR {
                fin = Fin::FinalDePagina;
                break;
            }
        } else {
            match cosedor.anadir(&marco) {
                Resultado::Primero | Resultado::Anadido => quietos = 0,
                Resultado::SinMovimiento => {
                    quietos += 1;
                    if quietos >= PASOS_QUIETOS_PARA_PARAR {
                        fin = Fin::FinalDePagina;
                        break;
                    }
                }
                // Un fotograma dudoso se descarta y se sigue: el siguiente
                // ya viene de camino.
                Resultado::Incierto => {}
                Resultado::Lleno => {
                    fin = Fin::AltoMaximo;
                    break;
                }
            }
        }
        anterior = Some(marco);

        pixpin_shell::rueda_en(centro, MUESCAS_POR_PASO);
        pasos += 1;
    }

    let alto = cosedor.alto();
    let imagen = cosedor.terminar();
    tracing::info!(
        pasos,
        alto,
        ?fin,
        ms = t0.elapsed().as_millis() as u64,
        "captura con scroll terminada"
    );
    Ok(imagen)
}

/// La region tal como esta AHORA, esperando a que el scroll animado termine:
/// dos capturas seguidas identicas. Si en un segundo no asienta (un video,
/// un reloj) se toma la ultima y se sigue: el cosido ya rechaza lo dudoso.
fn capturar_asentado(
    recursos: &mut Recursos,
    monitor: &pixpin_geom::Monitor,
    region: Rect,
) -> Result<ImagenRgba> {
    let inicio = Instant::now();
    let mut anterior = capturar(recursos, monitor, region)?;
    loop {
        std::thread::sleep(ESPERA_ASENTAR);
        let actual = capturar(recursos, monitor, region)?;
        if actual.pixeles == anterior.pixeles || inicio.elapsed() > ASENTAR_MAXIMO {
            return Ok(actual);
        }
        anterior = actual;
    }
}

fn capturar(
    recursos: &mut Recursos,
    monitor: &pixpin_geom::Monitor,
    region: Rect,
) -> Result<ImagenRgba> {
    let instantanea = recursos
        .congelar_monitor(monitor)
        .context("no se pudo capturar el monitor")?;
    let recorte = instantanea
        .recortar(recursos.dispositivo(), region)
        .context("no se pudo recortar la region")?;
    a_imagen(recursos.dispositivo(), &recorte).context("no se pudo bajar el recorte a memoria")
}
