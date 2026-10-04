//! **Dictar una leccion**: lo que en el movil es `RecognizerIntent`, aqui es
//! grabar con el microfono y pasarlo a texto con Whisper, por los mismos
//! caminos que una nota de voz del chat (`ventanita::abrir_microfono`,
//! `ventanita::cerrar_microfono`, `voz::pasar_a_texto`). Un clic empieza, otro
//! para; lo dicho llega como texto y la ficha lo reparte (`Dictado`).
//!
//! Nada se queda en el chat: el audio es un temporal que se borra al
//! acabar. Si este equipo no puede pasar voz a texto (sin ONNX Runtime), se
//! dice, como el movil sin dictado: «escribe la leccion».

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use pixpin_store::{Idioma, Ubicacion};

pub enum Dictado {
    Nada,
    Abriendo(Receiver<Result<pixpin_audio::Grabadora, pixpin_audio::ErrorAudio>>),
    Grabando(pixpin_audio::Grabadora),
    Parando(Receiver<Result<pixpin_audio::Grabacion, pixpin_audio::ErrorAudio>>),
    Pasando(crate::voz::EnMarcha),
}

/// Lo que paso en esta vuelta.
pub enum Salida {
    Nada,
    Texto(String),
    Fallo(String),
}

impl Dictado {
    pub fn activo(&self) -> bool {
        !matches!(self, Dictado::Nada)
    }

    /// Lo que se ensena mientras dura.
    pub fn estado(&self) -> Option<String> {
        match self {
            Dictado::Nada => None,
            Dictado::Abriendo(_) => Some("Abriendo el micrófono…".into()),
            Dictado::Grabando(_) => Some("🎙️ Te escucho… pulsa el micrófono para terminar".into()),
            Dictado::Parando(_) => Some("Guardando lo dicho…".into()),
            Dictado::Pasando(m) if m.bajando() => {
                Some(format!("Bajando el modelo de voz (una vez)… {:.0} %", m.avance() * 100.0))
            }
            Dictado::Pasando(m) => Some(format!("Pasando a texto… {:.0} %", m.avance() * 100.0)),
        }
    }

    /// El clic en el microfono: empezar o parar.
    pub fn pulsar(&mut self, ubicacion: &Ubicacion, idioma: Idioma) -> Option<String> {
        match std::mem::replace(self, Dictado::Nada) {
            Dictado::Nada => {
                if !crate::voz::whisper_posible(ubicacion, idioma) {
                    return Some("No hay dictado en este equipo: escribe la lección".into());
                }
                *self = Dictado::Abriendo(crate::ventanita::abrir_microfono(temporal(ubicacion)));
                None
            }
            Dictado::Grabando(g) => {
                *self = Dictado::Parando(crate::ventanita::cerrar_microfono(g));
                None
            }
            // Abriendo, parando o pasando: se deja seguir.
            otro => {
                *self = otro;
                None
            }
        }
    }

    /// Mira los hilos. Llamar en cada vuelta del bucle.
    pub fn avanzar(&mut self, ubicacion: &Ubicacion, idioma: Idioma) -> Salida {
        let (siguiente, salida) = match std::mem::replace(self, Dictado::Nada) {
            Dictado::Nada => (Dictado::Nada, Salida::Nada),
            Dictado::Abriendo(rx) => match rx.try_recv() {
                Ok(Ok(g)) => (Dictado::Grabando(g), Salida::Nada),
                Ok(Err(e)) => (Dictado::Nada, Salida::Fallo(fallo(&e))),
                Err(TryRecvError::Empty) => (Dictado::Abriendo(rx), Salida::Nada),
                Err(TryRecvError::Disconnected) => (Dictado::Nada, Salida::Fallo("No se pudo abrir el micrófono".into())),
            },
            Dictado::Grabando(g) => (Dictado::Grabando(g), Salida::Nada),
            Dictado::Parando(rx) => match rx.try_recv() {
                Ok(Ok(grabacion)) => (
                    Dictado::Pasando(crate::voz::pasar_a_texto(
                        "leccion",
                        &grabacion.ruta,
                        ubicacion,
                        idioma,
                        Vec::new(),
                        None,
                    )),
                    Salida::Nada,
                ),
                Ok(Err(e)) => {
                    let _ = std::fs::remove_file(temporal(ubicacion));
                    (Dictado::Nada, Salida::Fallo(fallo(&e)))
                }
                Err(TryRecvError::Empty) => (Dictado::Parando(rx), Salida::Nada),
                Err(TryRecvError::Disconnected) => (Dictado::Nada, Salida::Fallo("No se pudo guardar lo dicho".into())),
            },
            Dictado::Pasando(mut m) => match m.recoger() {
                None => (Dictado::Pasando(m), Salida::Nada),
                Some(r) => {
                    let _ = std::fs::remove_file(temporal(ubicacion));
                    match r {
                        Ok(t) => {
                            let texto = crate::voz::sin_marcas(&t.texto).trim().to_string();
                            if texto.is_empty() {
                                (Dictado::Nada, Salida::Fallo("No se entendió nada".into()))
                            } else {
                                (Dictado::Nada, Salida::Texto(texto))
                            }
                        }
                        Err(e) => {
                            tracing::info!(?e, "el dictado de la leccion no se paso a texto");
                            (Dictado::Nada, Salida::Fallo("No se pudo pasar a texto".into()))
                        }
                    }
                }
            },
        };
        *self = siguiente;
        salida
    }
}

fn temporal(ubicacion: &Ubicacion) -> PathBuf {
    let dir = ubicacion.raiz().join("cache");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("leccion-dictado.m4a")
}

fn fallo(e: &pixpin_audio::ErrorAudio) -> String {
    match e {
        pixpin_audio::ErrorAudio::SinMicrofono => "No hay micrófono".into(),
        pixpin_audio::ErrorAudio::DemasiadoCorta { .. } => "Demasiado corto: mantén el micrófono un poco más".into(),
        otro => format!("No se pudo grabar: {otro}"),
    }
}
