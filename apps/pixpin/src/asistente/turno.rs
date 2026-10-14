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

use pixpin_ia::agente::{self, Evento, Final, Opciones};
use pixpin_ia::contexto::{self, Fuente};
use pixpin_ia::herramientas::Herramienta;
use pixpin_ia::hoja_trabajo::{HojaDeTrabajo, Registro, ResumenDelTurno};
use pixpin_ia::motor::{Mensaje, Motor, Rol, Seguir};
use pixpin_ia::pensar::Plan;

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
    /// **Una accion que borra o envia pide permiso**: la burbuja ensena la
    /// pregunta con «Sí» y «No» y contesta por `responder` (`true`: si).
    /// El turno espera sin bloquear la ventana; si el chat se cierra, el
    /// `Sender` muere con el y la pregunta cuenta como «no».
    Confirmar {
        pregunta: String,
        responder: std::sync::mpsc::Sender<bool>,
    },
    /// Lo citado de los documentos del usuario hasta ahora en el turno
    /// (`buscar_en_mis_cosas`): la respuesta lo guarda y el chat ensena
    /// la cita como tarjeta que abre el documento.
    Citas(Vec<pixpin_ia::documentos::Cita>),
    /// Lo que penso, la peticion mejorada y la hoja reescrita, para guardar
    /// con la respuesta (llega justo antes del `Fin`, si el turno tenia
    /// [`Memoria`]).
    Registro(Registro),
    /// Se acabo. (La conversacion siguiente se rehace del chat, que es lo
    /// que el usuario ve y puede borrar.)
    Fin { resultado: Result<Final, String> },
}

/// **La memoria de un turno** (13-oct): la hoja de trabajo del chat, donde
/// busca `buscar_historial` y como piensa. Sin ella, el turno es el de
/// siempre.
#[derive(Debug, Clone, Default)]
pub struct Memoria {
    pub hoja: HojaDeTrabajo,
    pub fuentes: Vec<Fuente>,
    pub pensar: bool,
    pub planificar: bool,
    /// Tokens de la ventana del motor (0: no se mide).
    pub ventana: u32,
    /// El historial ya llega reiniciado (lo decidio `pensado::preparar_turno`).
    pub reiniciado: bool,
    /// Que la vuelta de planear pueda anadir herramientas del catalogo entero
    /// a las de la seleccion (`Opciones::ampliar`).
    pub ampliar: bool,
    /// Las ordenes simples sin modelo (`pixpin_ia::rapido`, 14-oct).
    pub rapido: bool,
}

/// Lo que se va juntando de los eventos para el registro del turno.
#[derive(Default)]
struct Recogido {
    pensamiento: String,
    segundos: Option<f32>,
    plan: Option<Plan>,
    herramientas: Vec<String>,
    reinicio: bool,
}

impl Recogido {
    fn ver(&mut self, e: &Evento) {
        match e {
            Evento::Pensando(t) => self.pensamiento.push_str(t),
            Evento::PensoListo { segundos } => *self.segundos.get_or_insert(0.0) += segundos,
            Evento::Peticion(p) => self.plan = Some(p.clone()),
            Evento::Reinicio { .. } => self.reinicio = true,
            Evento::Resultado { nombre, ok: true, .. } => self.herramientas.push(nombre.clone()),
            _ => {}
        }
    }

    /// El registro, con la hoja reescrita.
    fn registro(self, mut hoja: HojaDeTrabajo, pedido: &str, r: &Result<Final, String>, reiniciado: bool) -> Registro {
        let plan = self.plan.unwrap_or_else(|| Plan::por_omision(pedido));
        let (respuesta, incompleto) = match r {
            Ok(f) => (
                f.texto.clone(),
                f.cortado || f.texto.starts_with("No he sabido") || f.texto.starts_with("Hecho en parte"),
            ),
            Err(e) => (format!("No pude contestar: {e}"), true),
        };
        hoja.reescribir(
            &plan,
            &ResumenDelTurno {
                herramientas: self.herramientas,
                respuesta,
                incompleto,
                pensamiento: self.pensamiento.clone(),
            },
        );
        Registro {
            segundos: self.segundos.filter(|_| !self.pensamiento.trim().is_empty()),
            pensamiento: self.pensamiento,
            plan: Some(plan),
            hoja,
            reinicio: reiniciado || self.reinicio,
        }
    }
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
        Turno::lanzar_hacia(motor, proyecto, historial, adjuntos, foto, None)
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
        // Claude y DeepSeek reciben el catalogo entero (eligen bien y el
        // prefijo se cachea); el modelo local, solo las que vienen a cuento.
        // «Copia/dibuja esto» con una imagen: solo calcar, en los dos.
        let herramientas = if motor.catalogo_completo()
            && !super::acciones::es_para_calcar(&historial, &adjuntos, foto.is_some())
        {
            pixpin_ia::herramientas::catalogo()
        } else {
            super::acciones::herramientas_para(&historial, &adjuntos, foto.is_some())
        };
        Turno::lanzar_con(motor, proyecto, historial, adjuntos, foto, destino, herramientas)
    }

    pub fn lanzar_con(
        motor: Arc<dyn Motor>,
        proyecto: String,
        historial: Vec<Mensaje>,
        adjuntos: Vec<Adjunto>,
        foto: Option<PathBuf>,
        destino: Option<String>,
        herramientas: Vec<Herramienta>,
    ) -> Turno {
        Turno::lanzar_con_memoria(motor, proyecto, historial, adjuntos, foto, destino, herramientas, None, None)
    }

    /// Lanza el turno con su [`Memoria`]: piensa, planea, reescribe la hoja
    /// y manda el [`Aviso::Registro`] antes del final.
    pub fn lanzar_con_memoria(
        motor: Arc<dyn Motor>,
        proyecto: String,
        mut historial: Vec<Mensaje>,
        adjuntos: Vec<Adjunto>,
        foto: Option<PathBuf>,
        destino: Option<String>,
        herramientas: Vec<Herramienta>,
        memoria: Option<Memoria>,
        // Donde viven los chats: con ella, el asistente lee las listas de
        // tareas y las pide por su nombre (14-oct).
        raiz: Option<PathBuf>,
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
                let pedido = historial
                    .iter()
                    .rev()
                    .find(|m| m.rol == Rol::Usuario)
                    .map(Mensaje::texto_entero)
                    .unwrap_or_default();
                let con_memoria = memoria.is_some();
                let memoria = memoria.unwrap_or_default();
                let mut ejecutor = EjecutorDelChat::nuevo(tx.clone(), proyecto, adjuntos, parar_hilo.clone())
                    .hacia(destino)
                    .en_la_nube(motor.catalogo_completo())
                    // Lo que borra o envia pregunta en la burbuja, no en un
                    // cuadro de Windows (13-oct-2026).
                    .con_acciones(
                        super::acciones::confirmar_en_la_burbuja(tx.clone(), parar_hilo.clone()),
                        super::acciones::mandar_a_pixpin(),
                    )
                    .con_raiz(raiz);
                let buscar = |c: &str| contexto::buscar(&memoria.fuentes, c, 3);
                let todas = pixpin_ia::herramientas::catalogo();
                let opciones = Opciones {
                    planificar: memoria.planificar,
                    pensar: memoria.pensar,
                    hoja: con_memoria.then_some(&memoria.hoja),
                    ventana: memoria.ventana,
                    // Buscar en lo viejo solo cuando hay algo viejo que no esta
                    // cargado (tras un reinicio): antes, todo esta delante, y
                    // con la herramienta a mano el 2B la llamaba en vez de
                    // contestar (medido, 13-oct).
                    buscar: (con_memoria && (memoria.reiniciado || memoria.hoja.reinicios > 0)).then_some(&buscar as &dyn Fn(&str) -> String),
                    ampliar: memoria.ampliar.then_some(&todas[..]),
                    rapido: memoria.rapido.then(super::rapido::ahora),
                };
                let mut recogido = Recogido::default();
                let resultado = agente::turno_con(
                    motor.as_ref(),
                    &mut historial,
                    &herramientas,
                    &mut ejecutor,
                    &opciones,
                    &mut |e| {
                        recogido.ver(&e);
                        let _ = tx.send(Aviso::Evento(e));
                        if parar_hilo.load(Ordering::Relaxed) {
                            Seguir::Parar
                        } else {
                            Seguir::Si
                        }
                    },
                )
                .map_err(|e| {
                    // Lo tecnico, al registro; a la burbuja, en palabras.
                    tracing::warn!(error = %e, "asistente: el turno no pudo acabar");
                    agente::para_el_usuario(&e)
                });
                if con_memoria {
                    let r = recogido.registro(memoria.hoja.clone(), &pedido, &resultado, memoria.reiniciado);
                    let _ = tx.send(Aviso::Registro(r));
                }
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
