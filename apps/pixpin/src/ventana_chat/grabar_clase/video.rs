//! **El video de la reunion** (25-oct-2026, el usuario: «si quiero grabar
//! toda la reunion, que tambien pueda grabar el VIDEO mas el audio del
//! equipo y el microfono»).
//!
//! Un hilo propio que mira una pantalla con Windows.Graphics.Capture
//! (`SesionViva`, con el cursor) y escribe un MP4 H.264 con el **mismo
//! sonido** que la nota de voz: la grabadora le manda cada bloque mezclado
//! por una [`pixpin_audio::Copia`], y ese sonido es tambien el reloj del
//! video. Cada fotograma se fecha con las muestras recibidas, asi que:
//!
//! - las dos pistas no se separan aunque el equipo se atasque;
//! - **la pausa para el video sola**: en pausa no llega sonido, el reloj no
//!   corre y no se escribe ningun fotograma;
//! - el video se acaba cuando se acaba el sonido: al parar la grabadora el
//!   canal se cierra, el hilo escribe lo que quede y cierra el MP4.
//!
//! Pensado para el equipo de referencia (un Core i3 de tercera generacion con
//! 4 GB): diez fotogramas por segundo, como mucho 1280x720, una sola copia
//! de la pantalla en memoria y un fotograma nuevo solo cuando la pantalla
//! cambio. La barra no sale en el video: esta excluida de las capturas.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, Result};
use pixpin_record::escala;

/// Fotogramas por segundo del video de una reunion: las diapositivas y las
/// caras se siguen bien, y el codificador por software no ahoga al equipo.
pub(super) const POR_SEGUNDO: u32 = 10;
/// El tamano mas grande al que se graba.
const TOPE: (u32, u32) = (1280, 720);
/// Lo que se espera al primer fotograma de la pantalla antes de empezar con
/// uno negro.
const ESPERA_PRIMERO: i64 = 5_000_000;

/// Un video grabandose.
pub(super) struct Video {
    tirar: Arc<AtomicBool>,
    roto: Arc<AtomicBool>,
    hilo: Option<JoinHandle<Result<PathBuf>>>,
}

/// Donde se escribe el video mientras se graba, al lado del temporal de la
/// nota de voz y con su mismo nombre: si la aplicacion se cierra a mitad, lo
/// que quede esta al lado de su conversacion.
pub(super) fn temporal_del_video(temporal_de_la_voz: &std::path::Path) -> PathBuf {
    let mut s = temporal_de_la_voz.as_os_str().to_os_string();
    s.push(".video.mp4");
    PathBuf::from(s)
}

impl Video {
    /// Empieza a grabar el monitor `monitor` en `destino`, con el sonido que
    /// llegue por `sonido` (mono, a `muestreo`).
    pub(super) fn empezar(
        destino: PathBuf,
        monitor: u32,
        muestreo: u32,
        sonido: Receiver<Vec<i16>>,
    ) -> Video {
        let tirar = Arc::new(AtomicBool::new(false));
        let roto = Arc::new(AtomicBool::new(false));
        let hilo = {
            let tirar = Arc::clone(&tirar);
            let roto = Arc::clone(&roto);
            std::thread::Builder::new()
                .name("clase-video".into())
                .spawn(move || {
                    let _com = pixpin_shell::ComDelHilo::iniciar();
                    let hecho = grabar(&destino, monitor, muestreo, &sonido, &tirar, &roto);
                    if let Err(e) = &hecho
                        && !tirar.load(Ordering::Acquire)
                    {
                        tracing::warn!(?e, "el video de la clase se corto");
                        roto.store(true, Ordering::Release);
                    }
                    hecho
                })
        };
        let hilo = match hilo {
            Ok(h) => Some(h),
            Err(e) => {
                tracing::warn!(?e, "no se pudo lanzar el hilo del video de la clase");
                roto.store(true, Ordering::Release);
                None
            }
        };
        Video { tirar, roto, hilo }
    }

    /// Si el video dejo de grabarse (la nota sigue).
    pub(super) fn roto(&self) -> bool {
        self.roto.load(Ordering::Acquire)
    }

    /// Si el hilo ya cerro el MP4 (o lo dejo).
    pub(super) fn acabado(&self) -> bool {
        self.hilo.as_ref().is_none_or(|h| h.is_finished())
    }

    /// Espera al MP4 ya cerrado. Solo tiene sentido con el sonido parado: es
    /// lo que le dice al hilo que acabe.
    pub(super) fn recoger(mut self) -> Result<PathBuf> {
        let hilo = self.hilo.take().context("el video no llego a empezar")?;
        hilo.join()
            .map_err(|_| anyhow::anyhow!("el hilo del video se cayo"))?
    }

    /// Lo deja y borra lo escrito.
    pub(super) fn tirar(mut self) {
        self.parar_sin_guardar();
    }

    fn parar_sin_guardar(&mut self) {
        self.tirar.store(true, Ordering::Release);
        if let Some(h) = self.hilo.take() {
            // Sin `terminar`, el codificador borra su `.parte` al soltarse.
            if let Ok(Ok(ruta)) = h.join() {
                let _ = std::fs::remove_file(ruta);
            }
        }
    }
}

impl Drop for Video {
    fn drop(&mut self) {
        // Soltarlo sin recogerlo es tirarlo: no puede quedar un hilo
        // grabando la pantalla sin nadie que lo pare.
        if self.hilo.is_some() {
            self.parar_sin_guardar();
        }
    }
}

/// Lo que acaba asi es un video que se estaba escribiendo cuando la
/// aplicacion se cerro de golpe: sin su indice no lo abre nadie, y una hora
/// de reunion son cientos de megas en un disco pequeno.
const RESTO: &str = ".video.mp4.parte";

/// **Borra los videos a medias** de otra vez en `carpeta`. Solo se graba una
/// clase a la vez (`ABIERTA`), asi que cualquier `.parte` de video que haya
/// al empezar es de una grabacion que ya no va a terminar nunca.
fn barrer_restos(carpeta: &std::path::Path) {
    let Ok(entradas) = std::fs::read_dir(carpeta) else {
        return;
    };
    for e in entradas.flatten() {
        let nombre = e.file_name();
        if nombre.to_string_lossy().ends_with(RESTO) && e.path().is_file() {
            tracing::info!(ruta = %e.path().display(), "se borra un video a medias de otra vez");
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// El cuerpo del hilo.
fn grabar(
    destino: &std::path::Path,
    monitor: u32,
    muestreo: u32,
    sonido: &Receiver<Vec<i16>>,
    tirar: &AtomicBool,
    roto: &AtomicBool,
) -> Result<PathBuf> {
    let monitores = pixpin_capture::enumerar_monitores().context("sin monitores")?;
    let m = monitores
        .monitores()
        .iter()
        .find(|m| m.id == monitor)
        .or_else(|| monitores.principal())
        .context("sin monitor para el video")?
        .to_owned();
    let (ancho, alto) = escala::medida_de_salida(m.area.ancho, m.area.alto, TOPE.0, TOPE.1);
    if let Some(carpeta) = destino.parent() {
        barrer_restos(carpeta);
    }
    let mut cod = pixpin_record::Codificador::nuevo(
        destino,
        pixpin_record::OpcionesVivo {
            ancho,
            alto,
            por_segundo: POR_SEGUNDO,
            bitrate: None,
            muestreo_audio: Some(muestreo),
        },
    )
    .context("no se pudo abrir el MP4")?;
    let d = pixpin_capture::Dispositivo::nuevo().context("sin dispositivo de captura")?;
    let sesion = pixpin_capture::SesionViva::nueva_con_cursor(
        &d,
        m.id,
        // Algo menos que el ritmo del video: el tope solo ahorra copias.
        Duration::from_millis(1000 / u64::from(POR_SEGUNDO) * 2 / 3),
        None,
    )
    .context("no se pudo mirar la pantalla")?;
    let hecho = bucle(&mut cod, &d, &sesion, sonido, muestreo, tirar);
    sesion.cerrar();
    drop(d);
    if let Err(e) = hecho {
        anyhow::ensure!(!tirar.load(Ordering::Acquire), "video tirado");
        // Se corto a mitad (un fallo del codificador, la pantalla que se
        // fue): lo grabado hasta ahi vale, se intenta cerrar igual.
        tracing::warn!(?e, "el video de la clase se corto; se guarda lo grabado");
        roto.store(true, Ordering::Release);
    }
    let (escritos, saltados) = cod.cuenta();
    let ruta = cod.terminar().context("no se pudo cerrar el MP4")?;
    tracing::info!(escritos, saltados, ancho, alto, "video de la clase cerrado");
    Ok(ruta)
}

fn bucle(
    cod: &mut pixpin_record::Codificador,
    d: &pixpin_capture::Dispositivo,
    sesion: &pixpin_capture::SesionViva,
    sonido: &Receiver<Vec<i16>>,
    muestreo: u32,
    tirar: &AtomicBool,
) -> Result<()> {
    let (ancho, alto) = cod.medidas();
    let mut bajada = pixpin_capture::Bajada::default();
    let mut crudo: Vec<u8> = Vec::new();
    // Negro mientras no llegue la pantalla.
    let mut listo = vec![0u8; ancho as usize * alto as usize * 4];
    let mut muestras: u64 = 0;
    let mut hechos: u64 = 0;
    let mut visto: u64 = 0;
    let mut fin = false;
    while !fin {
        anyhow::ensure!(!tirar.load(Ordering::Acquire), "video tirado");
        match sonido.recv_timeout(Duration::from_millis(40)) {
            Ok(trozo) => {
                cod.poner_audio(&trozo)?;
                muestras += trozo.len() as u64;
                // Lo que se haya juntado mientras tanto, de una vez.
                while let Ok(trozo) = sonido.try_recv() {
                    cod.poner_audio(&trozo)?;
                    muestras += trozo.len() as u64;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => fin = true,
        }
        let ahora = escala::tiempo_de_muestras(muestras, muestreo);
        let debidos = escala::fotogramas_debidos(ahora, POR_SEGUNDO);
        if debidos <= hechos {
            continue;
        }
        let nuevos = sesion.aceptados();
        if hechos == 0 && nuevos == 0 && ahora < ESPERA_PRIMERO && !fin {
            continue;
        }
        // Solo se baja la pantalla si cambio: en una diapositiva quieta se
        // repite el ultimo fotograma sin tocar la GPU.
        if nuevos != visto
            && let Some(t) = sesion.ultimo()
        {
            match bajada.bajar_bgra(d, &t, &mut crudo) {
                Ok((w, h)) => {
                    escala::escalar_bgra(&crudo, w, h, &mut listo, ancho, alto);
                    visto = nuevos;
                }
                Err(e) => tracing::debug!(?e, "un fotograma de la clase no bajo"),
            }
        }
        // Si se fue atrasado, se escribe solo el ultimo que toca: el anterior
        // se queda en pantalla un poco mas, que es lo que se veria igual. El
        // primero cubre desde el cero (ver `escala::tramo_del_fotograma`).
        let (inicio, duracion) = escala::tramo_del_fotograma(hechos, debidos, POR_SEGUNDO);
        cod.poner_fotograma(&listo, inicio, duracion)?;
        hechos = debidos;
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_temporal_del_video_va_al_lado_del_de_la_voz() {
        let voz = std::path::Path::new("C:/p/voz-1.m4a.parcial");
        let v = temporal_del_video(voz);
        assert_eq!(v, std::path::Path::new("C:/p/voz-1.m4a.parcial.video.mp4"));
        // Caso negativo: no puede ser el mismo fichero que el de la voz.
        assert_ne!(v, voz);
    }

    #[test]
    fn barrer_restos_borra_solo_los_videos_a_medias() {
        let dir = std::env::temp_dir().join("pixpin-clase-barrer-restos");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // El nombre que de verdad deja el codificador a medias.
        let mut resto = temporal_del_video(&dir.join("voz-1.m4a.parcial")).into_os_string();
        resto.push(".parte");
        let resto = std::path::PathBuf::from(resto);
        assert!(resto.to_string_lossy().ends_with(RESTO));
        let hecho = dir.join("voz-1.m4a.parcial.video.mp4");
        let otro = dir.join("voz-1.m4a.parcial.parte");
        for f in [&resto, &hecho, &otro] {
            std::fs::write(f, b"x").unwrap();
        }
        barrer_restos(&dir);
        assert!(!resto.exists());
        // Caso negativo: un video ya cerrado vale, y lo de la voz no es suyo.
        assert!(hecho.exists());
        assert!(otro.exists());
        // Y una carpeta que no existe no es un cierre.
        barrer_restos(&dir.join("no-esta"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
