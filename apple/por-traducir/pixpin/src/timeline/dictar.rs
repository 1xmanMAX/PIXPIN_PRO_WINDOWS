//! **Dictar un momento**: grabar con el microfono y pasarlo a texto con
//! Whisper, por los mismos caminos que el dictado de las lecciones
//! (`lecciones::dictar`) y la nota de voz del chat. Un clic empieza, otro
//! para.
//!
//! La diferencia con las lecciones: aqui **el audio se queda**. Lo dicho es
//! el momento, y oirlo otra vez dice mas que la transcripcion (y la salva si
//! Whisper entendio mal). Por eso la salida lleva el fichero y su duracion,
//! y quien la recoge lo mueve a los medios del timeline.
//!
//! Tambien recuerda **cuando se empezo a hablar**: el momento es ese, no el
//! de despues de pasar a texto, que puede tardar unos segundos.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use pixpin_store::{Catalogo, Idioma, Ubicacion};

pub enum Dictado {
    Nada,
    Abriendo {
        rx: Receiver<Result<pixpin_audio::Grabadora, pixpin_audio::ErrorAudio>>,
        desde: i64,
    },
    Grabando {
        g: pixpin_audio::Grabadora,
        desde: i64,
    },
    Parando {
        rx: Receiver<Result<pixpin_audio::Grabacion, pixpin_audio::ErrorAudio>>,
        desde: i64,
    },
    Pasando {
        m: crate::voz::EnMarcha,
        grabacion: pixpin_audio::Grabacion,
        desde: i64,
    },
}

/// Lo dicho, ya en texto, con su audio.
pub struct Dicho {
    pub texto: String,
    pub audio: PathBuf,
    pub duracion_ms: i64,
    /// Cuando se pulso el microfono (UTC).
    pub desde: i64,
}

/// Lo que paso en esta vuelta.
pub enum Salida {
    Nada,
    Listo(Dicho),
    /// Se grabo pero no se pudo pasar a texto: el audio se guarda igual, sin
    /// texto, para no perder lo dicho.
    SinTexto(Dicho),
    Fallo(String),
}

impl Dictado {
    pub fn activo(&self) -> bool {
        !matches!(self, Dictado::Nada)
    }

    pub fn grabando(&self) -> bool {
        matches!(self, Dictado::Grabando { .. })
    }

    /// Lo que se ensena mientras dura.
    pub fn estado(&self, textos: &Catalogo) -> Option<String> {
        let con_por_cien = |clave: &str, v: f32| {
            let mut a = fluent_bundle::FluentArgs::new();
            a.set("pc", (v * 100.0).round() as i64);
            textos.t_args(clave, &a)
        };
        match self {
            Dictado::Nada => None,
            Dictado::Abriendo { .. } => Some(textos.t("timeline-mic-abriendo")),
            Dictado::Grabando { .. } => Some(textos.t("timeline-mic-escuchando")),
            Dictado::Parando { .. } => Some(textos.t("timeline-mic-guardando")),
            Dictado::Pasando { m, .. } if m.bajando() => {
                Some(con_por_cien("timeline-mic-bajando", m.avance()))
            }
            Dictado::Pasando { m, .. } => Some(con_por_cien("timeline-mic-pasando", m.avance())),
        }
    }

    /// El clic en el microfono: empezar o parar. Devuelve un aviso si no se
    /// puede.
    pub fn pulsar(
        &mut self,
        ubicacion: &Ubicacion,
        idioma: Idioma,
        textos: &Catalogo,
    ) -> Option<String> {
        match std::mem::replace(self, Dictado::Nada) {
            Dictado::Nada => {
                if !crate::voz::whisper_posible(ubicacion, idioma) {
                    return Some(textos.t("timeline-mic-sin-dictado"));
                }
                let desde = pixpin_shell::entorno::ahora_utc_ms();
                *self = Dictado::Abriendo {
                    rx: crate::ventanita::abrir_microfono(temporal(ubicacion)),
                    desde,
                };
                None
            }
            Dictado::Grabando { g, desde } => {
                *self = Dictado::Parando {
                    rx: crate::ventanita::cerrar_microfono(g),
                    desde,
                };
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
    pub fn avanzar(&mut self, ubicacion: &Ubicacion, idioma: Idioma, textos: &Catalogo) -> Salida {
        let (siguiente, salida) = match std::mem::replace(self, Dictado::Nada) {
            Dictado::Nada => (Dictado::Nada, Salida::Nada),
            Dictado::Abriendo { rx, desde } => match rx.try_recv() {
                Ok(Ok(g)) => (Dictado::Grabando { g, desde }, Salida::Nada),
                Ok(Err(e)) => (Dictado::Nada, Salida::Fallo(fallo(&e, textos))),
                Err(TryRecvError::Empty) => (Dictado::Abriendo { rx, desde }, Salida::Nada),
                Err(TryRecvError::Disconnected) => (
                    Dictado::Nada,
                    Salida::Fallo(textos.t("timeline-mic-no-abre")),
                ),
            },
            g @ Dictado::Grabando { .. } => (g, Salida::Nada),
            Dictado::Parando { rx, desde } => match rx.try_recv() {
                Ok(Ok(grabacion)) => (
                    Dictado::Pasando {
                        m: crate::voz::pasar_a_texto(
                            "timeline",
                            &grabacion.ruta,
                            ubicacion,
                            idioma,
                            Vec::new(),
                            None,
                        ),
                        grabacion,
                        desde,
                    },
                    Salida::Nada,
                ),
                Ok(Err(e)) => {
                    let _ = std::fs::remove_file(temporal(ubicacion));
                    (Dictado::Nada, Salida::Fallo(fallo(&e, textos)))
                }
                Err(TryRecvError::Empty) => (Dictado::Parando { rx, desde }, Salida::Nada),
                Err(TryRecvError::Disconnected) => (
                    Dictado::Nada,
                    Salida::Fallo(textos.t("timeline-mic-no-guarda")),
                ),
            },
            Dictado::Pasando {
                mut m,
                grabacion,
                desde,
            } => match m.recoger() {
                None => (
                    Dictado::Pasando {
                        m,
                        grabacion,
                        desde,
                    },
                    Salida::Nada,
                ),
                Some(r) => {
                    let texto = match r {
                        Ok(t) => crate::voz::sin_marcas(&t.texto).trim().to_string(),
                        Err(e) => {
                            tracing::info!(?e, "el dictado del timeline no se paso a texto");
                            String::new()
                        }
                    };
                    let dicho = Dicho {
                        texto,
                        audio: grabacion.ruta,
                        duracion_ms: grabacion.duracion_ms,
                        desde,
                    };
                    if dicho.texto.is_empty() {
                        (Dictado::Nada, Salida::SinTexto(dicho))
                    } else {
                        (Dictado::Nada, Salida::Listo(dicho))
                    }
                }
            },
        };
        *self = siguiente;
        salida
    }
}

/// El fichero donde se graba. Uno solo basta: no se graba otro hasta que
/// este se haya movido a los medios.
fn temporal(ubicacion: &Ubicacion) -> PathBuf {
    let dir = ubicacion.raiz().join("cache");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("timeline-dictado.m4a")
}

fn fallo(e: &pixpin_audio::ErrorAudio, textos: &Catalogo) -> String {
    match e {
        pixpin_audio::ErrorAudio::SinMicrofono => textos.t("timeline-mic-sin-microfono"),
        pixpin_audio::ErrorAudio::DemasiadoCorta { .. } => textos.t("timeline-mic-corto"),
        otro => {
            let mut a = fluent_bundle::FluentArgs::new();
            a.set("error", otro.to_string());
            textos.t_args("timeline-mic-error", &a)
        }
    }
}
