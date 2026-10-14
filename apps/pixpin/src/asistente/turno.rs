//! **Un turno del asistente en un hilo de fondo.**
//!
//! `pixpin_ia::agente::turno` bloquea (segundos, o minutos con solo CPU):
//! aqui se lanza en su hilo y todo lo que pasa llega por un canal como
//! [`Aviso`]. El chat lo mira una vez por vuelta de su bucle con
//! [`Turno::recibir`] (sin esperar nunca) y repinta la burbuja.
//!
//! «Parar» pone una bandera que el turno lee en cada trozo del modelo: el
//! motor deja de escuchar y el turno acaba como `cortado` con lo que hubiera
//! escrito hasta ahi.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use pixpin_ia::agente::{self, Evento, Final};
use pixpin_ia::herramientas::{Herramienta, catalogo};
use pixpin_ia::motor::{Mensaje, Motor, Seguir};

use super::ejecutor::EjecutorDelChat;

/// Un fichero que el usuario adjunto en esta conversacion (lo que el OCR
/// puede leer).
#[derive(Debug, Clone, PartialEq)]
pub struct Adjunto {
    /// Como lo ve el usuario (y como lo nombra el modelo).
    pub nombre: String,
    pub ruta: PathBuf,
}

/// Lo que manda el hilo del turno a la UI.
pub enum Aviso {
    /// Lo que va pasando: texto nuevo, «dibujando…», resultados.
    Evento(Evento),
    /// El texto entero que leyo el OCR (al modelo le llega recortado), para
    /// ensenarlo en el chat y ofrecer guardarlo.
    TextoLeido { de: Adjunto, texto: String },
    /// Se acabo. (La conversacion siguiente se rehace del chat, que es lo
    /// que el usuario ve y puede borrar.)
    Fin { resultado: Result<Final, String> },
}

/// Un turno en marcha.
pub struct Turno {
    parar: Arc<AtomicBool>,
    rx: Receiver<Aviso>,
    acabado: bool,
}

impl Turno {
    /// Lanza el turno. `historial` ya trae las instrucciones de sistema y el
    /// mensaje nuevo del usuario al final.
    /// `foto`: la imagen que tiene que ver con la pregunta (se reduce y se
    /// pega al ultimo mensaje en el hilo, no en el de la ventana).
    #[cfg(test)]
    pub fn lanzar(
        motor: Arc<dyn Motor>,
        proyecto: String,
        historial: Vec<Mensaje>,
        adjuntos: Vec<Adjunto>,
        foto: Option<PathBuf>,
    ) -> Turno {
        Turno::lanzar_con(motor, proyecto, historial, adjuntos, foto, None, catalogo())
    }

    /// Lo mismo con el lienzo donde dibujar si no hay uno abierto (la
    /// mencion `@lienzo:` de la pregunta): su referencia en el chat.
    pub fn lanzar_hacia(
        motor: Arc<dyn Motor>,
        proyecto: String,
        historial: Vec<Mensaje>,
        adjuntos: Vec<Adjunto>,
        foto: Option<PathBuf>,
        destino: Option<String>,
    ) -> Turno {
        Turno::lanzar_con(motor, proyecto, historial, adjuntos, foto, destino, catalogo())
    }

    pub fn lanzar_con(
        motor: Arc<dyn Motor>,
        proyecto: String,
        mut historial: Vec<Mensaje>,
        adjuntos: Vec<Adjunto>,
        foto: Option<PathBuf>,
        destino: Option<String>,
        herramientas: Vec<Herramienta>,
    ) -> Turno {
        let (tx, rx) = channel();
        let parar = Arc::new(AtomicBool::new(false));
        let parar_hilo = parar.clone();
        let lanzado = std::thread::Builder::new().name("asistente-turno".into()).spawn({
            let tx = tx.clone();
            move || {
                // El OCR y el PDF son WinRT: este hilo necesita su COM. Va
                // primero para soltarse el ultimo, cuando ya no vive nada.
                let _com = pixpin_shell::ComDelHilo::iniciar();
                if let Some(ruta) = foto {
                    poner_foto(&mut historial, &ruta);
                }
                let mut ejecutor =
                    EjecutorDelChat::nuevo(tx.clone(), proyecto, adjuntos, parar_hilo.clone()).hacia(destino);
                let resultado = agente::turno(motor.as_ref(), &mut historial, &herramientas, &mut ejecutor, &mut |e| {
                    let _ = tx.send(Aviso::Evento(e));
                    if parar_hilo.load(Ordering::Relaxed) {
                        Seguir::Parar
                    } else {
                        Seguir::Si
                    }
                })
                .map_err(|e| {
                    // Lo tecnico, al registro; a la burbuja, en palabras.
                    tracing::warn!(error = %e, "asistente: el turno no pudo acabar");
                    agente::para_el_usuario(&e)
                });
                let _ = tx.send(Aviso::Fin { resultado });
            }
        });
        if let Err(e) = lanzado {
            tracing::warn!(?e, "asistente: no se pudo lanzar el hilo del turno");
            let _ = tx.send(Aviso::Fin {
                resultado: Err("No pude empezar a contestar. Prueba otra vez.".into()),
            });
        }
        Turno {
            parar,
            rx,
            acabado: false,
        }
    }

    /// Pide que pare. Llega en el siguiente trozo del modelo.
    pub fn parar(&self) {
        self.parar.store(true, Ordering::Relaxed);
    }

    /// Lo que haya llegado, sin esperar. Si el hilo murio sin decir nada
    /// (un panico), se inventa un final para no dejar la burbuja colgada.
    pub fn recibir(&mut self) -> Vec<Aviso> {
        let mut v = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(a) => {
                    if matches!(a, Aviso::Fin { .. }) {
                        self.acabado = true;
                    }
                    v.push(a);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !self.acabado {
                        self.acabado = true;
                        v.push(Aviso::Fin {
                            resultado: Err("el asistente se detuvo de golpe".into()),
                        });
                    }
                    break;
                }
            }
        }
        v
    }

    pub fn acabado(&self) -> bool {
        self.acabado
    }
}

/// La conversacion para el modelo a partir de lo que se ve en el chat:
/// instrucciones, los ultimos mensajes (solo texto: las llamadas viejas no
/// hacen falta y gastan contexto) y el nuevo, con su foto si la hay.
pub fn historial_para_el_modelo(
    anteriores: &[(bool, String)],
    nuevo: &str,
    foto: Option<(Vec<u8>, &str)>,
) -> Vec<Mensaje> {
    /// Cuantos mensajes de antes se mandan: el contexto es de 8192 tokens y
    /// el catalogo y una foto ya se comen ~1600.
    const ULTIMOS: usize = 12;
    /// Y cuanto de cada uno, como mucho (un OCR largo pegado en el chat).
    const LARGO: usize = 1500;
    let mut h = vec![Mensaje::sistema(agente::instrucciones())];
    let desde = anteriores.len().saturating_sub(ULTIMOS);
    for (del_usuario, texto) in &anteriores[desde..] {
        let texto: String = texto.chars().take(LARGO).collect();
        if texto.trim().is_empty() {
            continue;
        }
        h.push(if *del_usuario {
            Mensaje::usuario(texto)
        } else {
            Mensaje::texto(pixpin_ia::motor::Rol::Asistente, texto)
        });
    }
    h.push(match foto {
        Some((bytes, mime)) => Mensaje::usuario_con_imagen(nuevo, bytes, mime),
        None => Mensaje::usuario(nuevo),
    });
    h
}

/// Pega la foto, reducida, al ultimo mensaje (el del usuario). Si no se
/// puede leer, la pregunta va sin ella: mejor contestar sin ver que no
/// contestar.
fn poner_foto(historial: &mut [Mensaje], ruta: &std::path::Path) {
    let Some(ultimo) = historial.last_mut() else { return };
    match super::foto::preparar_fichero(ruta) {
        Ok((bytes, mime)) => *ultimo = Mensaje::usuario_con_imagen(ultimo.texto_entero(), bytes, mime),
        Err(e) => tracing::warn!(%e, "asistente: la foto no se pudo preparar"),
    }
}
