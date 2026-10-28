//! **El video de la reunion** (25-oct-2026, el usuario: «si quiero grabar
//! toda la reunion, que tambien pueda grabar el VIDEO mas el audio del
//! equipo y el microfono»).
//!
//! Un hilo propio que le da al motor de video (`pixpin_motor_video`, ver
//! `docs/motor-de-video.md`) la pantalla donde esta la ventana y el **mismo
//! sonido** que la nota de voz: la grabadora le manda cada bloque mezclado
//! por una [`pixpin_audio::Copia`], y ese sonido es tambien el reloj del
//! video (el motor fecha los fotogramas contra el). Asi:
//!
//! - las dos pistas no se separan aunque el equipo se atasque;
//! - **la pausa para el video**: se le dice al motor ([`Video::pausar`]) y,
//!   de todas formas, en pausa no llega sonido y su reloj no corre;
//! - el video se acaba cuando se acaba el sonido: al parar la grabadora el
//!   canal se cierra, el hilo le dice al motor que termine y este cierra el
//!   MP4.
//!
//! El motor hace el trabajo en la GPU (captura, conversion y codificador de
//! la grafica) y solo escribe un fotograma cuando la pantalla cambio; en el
//! equipo de referencia (un Core i3 de tercera generacion con 4 GB) la CPU
//! casi no se entera. Si la grafica no puede, cae a software y luego a la
//! CPU, y dice cual quedo ([`Video::codificador`]). La barra no sale en el
//! video: esta excluida de las capturas.
//!
//! **A prueba de cortes** (28-oct-2026): el MP4 se escribe **fragmentado**
//! (`Opciones::fragmentado`), que se puede abrir aunque PixPin muera a mitad
//! de la reunion; al cerrarlo bien, si le falta el indice de fragmentos, se
//! rehace a MP4 normal sin volver a codificar
//! (`pixpin_motor_video::archivo::recuperar_video`), que es lo que salta
//! bien en el movil. Lo que quede de un corte lo recupera la siguiente vez
//! `grabar_clase::recuperar`; aqui ya no se barre nada. Si el disco se
//! queda sin sitio, [`Video::cortar`] cierra el video con lo grabado y la
//! nota sigue.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, Result};
use pixpin_motor_video::{
    Codificador, CopiaMono, Fuente, FuenteAudio, MotorVideo, Opciones, Perfil, Siguiente,
};

/// Un video grabandose.
pub(super) struct Video {
    tirar: Arc<AtomicBool>,
    /// Cerrar ya el MP4 con lo grabado (el disco se llena): la nota sigue.
    cortar: Arc<AtomicBool>,
    roto: Arc<AtomicBool>,
    /// La pausa que pide la barra; el hilo se la pasa al motor.
    pausa: Arc<AtomicBool>,
    /// El codificador que eligio el motor, en cuanto arranca.
    codificador: Arc<OnceLock<Codificador>>,
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
    /// Empieza a grabar el monitor `monitor` en `destino` con el `perfil`
    /// elegido, y con el sonido que llegue por `sonido` (mono, a
    /// `muestreo`).
    pub(super) fn empezar(
        destino: PathBuf,
        monitor: u32,
        muestreo: u32,
        perfil: Perfil,
        sonido: Receiver<Vec<i16>>,
    ) -> Video {
        let tirar = Arc::new(AtomicBool::new(false));
        let cortar = Arc::new(AtomicBool::new(false));
        let roto = Arc::new(AtomicBool::new(false));
        let pausa = Arc::new(AtomicBool::new(false));
        let codificador = Arc::new(OnceLock::new());
        let hilo = {
            let tirar = Arc::clone(&tirar);
            let cortar = Arc::clone(&cortar);
            let roto = Arc::clone(&roto);
            let pausa = Arc::clone(&pausa);
            let codificador = Arc::clone(&codificador);
            std::thread::Builder::new()
                .name("clase-video".into())
                .spawn(move || {
                    let hecho = grabar(
                        &destino,
                        monitor,
                        perfil,
                        CopiaMono::nueva(sonido, muestreo),
                        &Senales {
                            tirar: &tirar,
                            cortar: &cortar,
                            roto: &roto,
                            pausa: &pausa,
                            codificador: &codificador,
                        },
                    );
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
        Video {
            tirar,
            cortar,
            roto,
            pausa,
            codificador,
            hilo,
        }
    }

    /// Pausa o sigue el video, a la vez que la nota.
    pub(super) fn pausar(&self, en_pausa: bool) {
        self.pausa.store(en_pausa, Ordering::Release);
    }

    /// **Cierra ya el video** con lo grabado hasta ahora (el disco se esta
    /// llenando): el MP4 queda bueno y la nota sigue sola. Se recoge al
    /// terminar, como siempre.
    pub(super) fn cortar(&self) {
        self.cortar.store(true, Ordering::Release);
    }

    /// Con que se esta codificando (la grafica, el procesador...), en
    /// cuanto el motor arranco.
    pub(super) fn codificador(&self) -> Option<Codificador> {
        self.codificador.get().copied()
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
        // Soltarlo sin recogerlo (la barra se cayo a mitad) no es tirarlo:
        // se para el hilo, que no puede quedar grabando sin nadie que lo
        // pare, pero cerrando el MP4. Queda en su sitio y la recuperacion lo
        // encuentra por el diario. Tirar es [`Video::tirar`].
        if let Some(h) = self.hilo.take() {
            self.cortar.store(true, Ordering::Release);
            if let Ok(Ok(ruta)) = h.join() {
                tracing::warn!(ruta = %ruta.display(), "video soltado sin recoger; queda para recuperarlo");
            }
        }
    }
}

/// Lo que el hilo comparte con la barra.
struct Senales<'a> {
    tirar: &'a AtomicBool,
    cortar: &'a AtomicBool,
    roto: &'a AtomicBool,
    pausa: &'a AtomicBool,
    codificador: &'a OnceLock<Codificador>,
}

/// El cuerpo del hilo: arranca el motor y le va dando el sonido hasta que
/// la grabadora cierra el canal.
fn grabar(
    destino: &std::path::Path,
    monitor: u32,
    perfil: Perfil,
    mut sonido: impl FuenteAudio,
    s: &Senales,
) -> Result<PathBuf> {
    // La pantalla de la ventana; si ya no esta (se desconecto), la
    // principal.
    let fuente = pixpin_capture::handle_de_monitor(monitor)
        .map(|h| Fuente::Monitor(h.0 as isize))
        .unwrap_or_else(pixpin_motor_video::monitor_principal);
    let motor = MotorVideo::empezar(
        Opciones {
            perfil,
            audio: Some(sonido.formato()),
            // Se puede abrir aunque PixPin muera a mitad (ver arriba).
            fragmentado: true,
            ..Opciones::de(fuente)
        },
        destino.to_path_buf(),
    )
    .context("no se pudo empezar el video")?;
    let _ = s.codificador.set(motor.estado().codificador);
    let mut en_pausa = false;
    loop {
        // Soltar el motor sin terminarlo es tirarlo: borra el `.parte`.
        anyhow::ensure!(!s.tirar.load(Ordering::Acquire), "video tirado");
        if s.cortar.load(Ordering::Acquire) {
            tracing::warn!("el video de la clase se cierra antes: el disco se llena");
            s.roto.store(true, Ordering::Release);
            break;
        }
        let pausa = s.pausa.load(Ordering::Acquire);
        if pausa != en_pausa {
            en_pausa = pausa;
            if pausa {
                motor.pausar();
            } else {
                motor.seguir();
            }
        }
        match sonido.siguiente(Duration::from_millis(40)) {
            Siguiente::Bloque { muestras, posicion } => {
                // Si el motor ya paro (se corto), `terminar` dice por que y
                // guarda lo que habia.
                if motor.poner_audio(&muestras, posicion).is_err() {
                    break;
                }
            }
            Siguiente::Nada => {}
            Siguiente::Fin => break,
        }
        if motor.estado().cortado {
            s.roto.store(true, Ordering::Release);
        }
    }
    let r = motor.terminar().context("no se pudo cerrar el MP4")?;
    if let Some(motivo) = &r.cortado {
        // Se corto a mitad: lo grabado hasta ahi vale y se guarda.
        tracing::warn!(
            motivo,
            "el video de la clase se corto; se guarda lo grabado"
        );
        s.roto.store(true, Ordering::Release);
    }
    tracing::info!(
        fotogramas = r.fotogramas,
        caidos = r.caidos,
        bytes = r.bytes,
        codificador = r.codificador.clave(),
        ancho = r.medidas.0,
        alto = r.medidas.1,
        fragmentado = r.fragmentado,
        "video de la clase cerrado"
    );
    if r.fragmentado {
        a_normal(&r.ruta);
    }
    Ok(r.ruta)
}

/// **Un MP4 fragmentado ya cerrado, a MP4 normal**, si le falta el indice de
/// fragmentos (sin el, el movil salta mal por una hora de video). Se copia
/// sin volver a codificar; si no se puede, se queda fragmentado, que se abre
/// igual.
fn a_normal(ruta: &std::path::Path) {
    use pixpin_sistema::mp4::{EstadoMp4, mirar};
    if mirar(ruta) != (EstadoMp4::Fragmentado { con_indice: false }) {
        return;
    }
    let mut s = ruta.as_os_str().to_os_string();
    s.push(".fragmentado.parte");
    let fragmentado = PathBuf::from(s);
    if std::fs::rename(ruta, &fragmentado).is_err() {
        return;
    }
    let como = pixpin_motor_video::archivo::recuperar_video(
        &fragmentado,
        ruta,
        pixpin_motor_video::rehacer_mp4,
    );
    if como.is_none() {
        // No deberia (era fragmentado): que vuelva a su nombre tal cual.
        let _ = std::fs::rename(&fragmentado, ruta);
    }
    tracing::info!(?como, "video de la clase pasado a MP4 normal");
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
}
