//! **«Pantalla» de la barra: el video de la pantalla, encendido y apagado
//! mientras el audio sigue** (29-oct-2026, el usuario en el movil: «que en
//! la misma barra me de la opcion de grabar la pantalla o pausarla pero se
//! siga grabando el audio»; ver `docs/grabacion-unica.md`).
//!
//! Ya no hay modos: la grabacion es una sola, con el sonido siempre. El
//! video de la pantalla es un **trozo** cada vez que se enciende:
//!
//! - **El reparto del sonido** ([`Reparto`]): la grabadora le manda su
//!   copia del sonido mezclado desde el principio (la copia se pide al
//!   abrirla y no se puede pedir despues) y un hilo la reparte al video que
//!   este en marcha, o la tira si no hay ninguno. Asi cada trozo lleva el
//!   mismo sonido que la nota y su reloj (el motor fecha los fotogramas
//!   contra el sonido; en pausa no llega nada y no corre). Apagar la
//!   pantalla es soltar el canal del trozo: el hilo del video ve el fin y
//!   cierra su MP4, como al terminar.
//! - **Los trozos** ([`Trozo`]): el que empieza con la grabacion («Empezar
//!   grabando la pantalla» de Preparar) es **el video de la nota**, como
//!   siempre (`video: {id}`, `enGrabacion.ms = 0`, y su diario lo apunta
//!   como video). Los demas entran como **clips** en su minuto, «Clase … ·
//!   pantalla N.mp4», sin `fuente` (no son de la camara) y apuntados en el
//!   diario como trozos de la pantalla. Es el mismo formato que el movil.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use super::{Estado, Rotulos, camara, video};

/// **El reparto del sonido** entre los trozos del video.
pub(super) struct Reparto {
    destino: Arc<Mutex<Option<Sender<Vec<i16>>>>>,
}

impl Reparto {
    /// El reparto y la copia que se le pasa a la grabadora al abrirla.
    pub(super) fn nuevo() -> (Reparto, pixpin_audio::Copia) {
        let (tx, rx) = channel::<Vec<i16>>();
        let destino: Arc<Mutex<Option<Sender<Vec<i16>>>>> = Arc::new(Mutex::new(None));
        let suyo = Arc::clone(&destino);
        let lanzado = std::thread::Builder::new()
            .name("clase-reparto".into())
            .spawn(move || {
                // Hasta que la grabadora suelte la copia (acabo o se tiro).
                for bloque in rx {
                    let mut d = suyo.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(t) = d.as_ref()
                        && t.send(bloque).is_err()
                    {
                        *d = None;
                    }
                }
                // El fin del sonido es el fin del trozo que quedara abierto.
                *suyo.lock().unwrap_or_else(|e| e.into_inner()) = None;
            });
        if let Err(e) = lanzado {
            tracing::warn!(?e, "no se pudo lanzar el reparto del sonido de la clase");
        }
        (Reparto { destino }, tx)
    }

    /// El sonido para un trozo nuevo (el que hubiera se cierra).
    fn abrir(&self) -> Receiver<Vec<i16>> {
        let (tx, rx) = channel();
        *self.destino.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
        rx
    }

    /// Sin trozo: el sonido se tira y el video que iba cierra su MP4.
    fn cerrar(&self) {
        *self.destino.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// Un trozo del video de la pantalla.
pub(super) struct Trozo {
    /// `None` una vez recogido o tirado.
    pub video: Option<video::Video>,
    /// Cuando empezo y acabo, en la nota (`fin_ms` es `None` mientras va).
    pub ms: i64,
    pub fin_ms: Option<i64>,
    /// El que empezo con la grabacion: el video de la nota.
    pub principal: bool,
    pub ruta: PathBuf,
}

impl Estado {
    /// El video de la pantalla que se esta grabando ahora.
    pub(super) fn video_en_marcha(&self) -> Option<&video::Video> {
        self.trozos
            .last()
            .filter(|t| t.fin_ms.is_none())
            .and_then(|t| t.video.as_ref())
    }

    /// Si «Pantalla» esta encendida.
    pub(super) fn pantalla_encendida(&self) -> bool {
        self.video_en_marcha().is_some()
    }

    /// **Enciende el video de la pantalla** en el minuto `ms` de la nota,
    /// que va a `muestreo` y quiza en pausa. Con `principal`, es el que
    /// empieza con la grabacion.
    pub(super) fn encender_pantalla(
        &mut self,
        (ms, muestreo, en_pausa): (i64, u32, bool),
        principal: bool,
        r: &Rotulos,
    ) {
        if self.pantalla_encendida() {
            return;
        }
        // Sin sitio en el disco, el video ni empieza: el sonido pesa cien
        // veces menos y es lo que no se puede perder.
        if self.sin_video_por_disco {
            self.avisar(r.disco_lleno.clone());
            return;
        }
        let Some(reparto) = &self.reparto else {
            return;
        };
        let ms = if principal { 0 } else { ms };
        let ruta = if principal {
            video::temporal_del_video(&self.temporal)
        } else {
            let mut s = self.temporal.as_os_str().to_os_string();
            s.push(format!(".pantalla-{}.mp4", self.trozos.len() + 1));
            PathBuf::from(s)
        };
        let v = video::Video::empezar(
            ruta.clone(),
            self.monitor_video,
            muestreo,
            self.perfil_video,
            reparto.abrir(),
        );
        if en_pausa {
            v.pausar(true);
        }
        if !principal {
            self.apuntar_trozo_empezado(ms, &ruta);
        }
        self.video_avisado = false;
        self.trozos.push(Trozo {
            video: Some(v),
            ms,
            fin_ms: None,
            principal,
            ruta,
        });
    }

    /// **Apaga el video de la pantalla** en el minuto `ms`: el trozo cierra
    /// su MP4 en su hilo (se recoge al terminar).
    pub(super) fn apagar_pantalla(&mut self, ms: i64) {
        if let Some(r) = &self.reparto {
            r.cerrar();
        }
        if let Some(t) = self.trozos.last_mut().filter(|t| t.fin_ms.is_none()) {
            t.fin_ms = Some(ms.max(t.ms));
            if !t.principal {
                let (ruta, dura) = (t.ruta.clone(), ms - t.ms);
                self.apuntar_trozo_cerrado(&ruta, dura);
            }
        }
    }

    /// Si todos los trozos ya cerraron su MP4 (se espera al terminar sin
    /// congelar la barra).
    pub(super) fn trozos_acabados(&self) -> bool {
        self.trozos
            .iter()
            .all(|t| t.video.as_ref().is_none_or(video::Video::acabado))
    }

    /// **Recoge los trozos al guardar**: devuelve el video de la nota (si
    /// lo hay) y deja los demas en los clips, en su minuto. `fin_ms`: donde
    /// acabo la nota, para el trozo que siguiera abierto.
    pub(super) fn recoger_trozos(&mut self, fin_ms: i64) -> Option<PathBuf> {
        let mut principal = None;
        for mut t in std::mem::take(&mut self.trozos) {
            let Some(v) = t.video.take() else {
                continue;
            };
            let hecho = match v.recoger() {
                Ok(ruta) => ruta,
                Err(e) => {
                    tracing::warn!(?e, ms = t.ms, "un trozo de la pantalla no se pudo cerrar");
                    continue;
                }
            };
            if t.principal {
                principal = Some(hecho);
                continue;
            }
            let fin = t.fin_ms.unwrap_or(fin_ms).max(t.ms);
            if t.fin_ms.is_none() {
                self.apuntar_trozo_cerrado(&hecho, fin - t.ms);
            }
            self.clips.push(camara::ClipHecho {
                ms: t.ms,
                duracion_ms: fin - t.ms,
                ruta: hecho,
                de_pantalla: true,
            });
        }
        principal
    }

    /// **Tira los trozos** (la grabacion se tira, o no llego a grabarse).
    pub(super) fn tirar_trozos(&mut self) {
        if let Some(r) = &self.reparto {
            r.cerrar();
        }
        for mut t in self.trozos.drain(..) {
            if let Some(v) = t.video.take() {
                v.tirar();
            }
        }
    }
}
