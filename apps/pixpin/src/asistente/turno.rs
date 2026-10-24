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
    /// **Una duda con opciones** (`preguntar_usuario`, 22-oct): la burbuja
    /// ensena la tarjeta con sus botones y «Otro…», y contesta por
    /// `responder` lo elegido o lo escrito. Si el chat se cierra, el
    /// `Sender` muere con el y cuenta como sin respuesta.
    Preguntar {
        pregunta: pixpin_ia::preguntar::Pregunta,
        responder: std::sync::mpsc::Sender<pixpin_ia::preguntar::Contestada>,
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
    /// **La memoria de largo plazo** (22-oct, `asistente::memoria`): con la
    /// nube, el perfil y el indice en el sistema y la herramienta `memoria`.
    pub largo_plazo: Option<pixpin_ia::memoria::Memoria>,
    /// Lo que el reinicio dejo de mandar: se guarda lo duradero en segundo
    /// plano mientras el turno contesta.
    pub a_guardar: Vec<(bool, String)>,
}

/// El modelo de vision de los turnos de las pruebas (en la app sale de
/// Ajustes: `hoja_a_lienzo::motor_de_vision`).
#[cfg(test)]
pub static VISION_EN_PRUEBAS: std::sync::Mutex<Option<Arc<dyn Motor>>> =
    std::sync::Mutex::new(None);

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
            Evento::Resultado {
                nombre, ok: true, ..
            } => self.herramientas.push(nombre.clone()),
            _ => {}
        }
    }

    /// El registro, con la hoja reescrita.
    fn registro(
        self,
        mut hoja: HojaDeTrabajo,
        pedido: &str,
        r: &Result<Final, String>,
        reiniciado: bool,
    ) -> Registro {
        let plan = self.plan.unwrap_or_else(|| Plan::por_omision(pedido));
        let (respuesta, incompleto) = match r {
            Ok(f) => (
                f.texto.clone(),
                f.cortado
                    || f.texto.starts_with("No he sabido")
                    || f.texto.starts_with("Hecho en parte"),
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
            segundos: self
                .segundos
                .filter(|_| !self.pensamiento.trim().is_empty()),
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
    /// Lo que el usuario escribe mientras trabaja (25-oct-2026): lo recoge
    /// el turno antes de cada vuelta (`agente::Opciones::entrantes`).
    entrantes: Arc<std::sync::Mutex<Vec<String>>>,
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
    #[cfg_attr(not(test), allow(dead_code))]
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
            // El catalogo entero, sin quitar herramientas por las palabras
            // del pedido (24-oct-2026: «…y abajo una zona gris» se quedaba
            // sin dibujar). Lo que evita duplicar es entender el pedido
            // (`agente::instrucciones`) y la guarda de `guardas.rs`.
            pixpin_ia::herramientas::catalogo()
        } else {
            super::acciones::herramientas_para(&historial, &adjuntos, foto.is_some())
        };
        Turno::lanzar_con(
            motor,
            proyecto,
            historial,
            adjuntos,
            foto,
            destino,
            herramientas,
        )
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn lanzar_con(
        motor: Arc<dyn Motor>,
        proyecto: String,
        historial: Vec<Mensaje>,
        adjuntos: Vec<Adjunto>,
        foto: Option<PathBuf>,
        destino: Option<String>,
        herramientas: Vec<Herramienta>,
    ) -> Turno {
        Turno::lanzar_con_memoria(
            motor,
            proyecto,
            historial,
            adjuntos,
            foto,
            destino,
            herramientas,
            None,
            None,
        )
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
        let entrantes: Arc<std::sync::Mutex<Vec<String>>> = Arc::default();
        let entrantes_hilo = entrantes.clone();
        let lanzado = std::thread::Builder::new().name("asistente-turno".into()).spawn({
            let tx = tx.clone();
            move || {
                // El OCR y el PDF son WinRT: este hilo necesita su COM. Va
                // primero para soltarse el ultimo, cuando ya no vive nada.
                let _com = pixpin_shell::ComDelHilo::iniciar();
                let con_foto = foto.is_some();
                let adjuntos_con_imagen = adjuntos.iter().any(|a| super::ocr::es_imagen(&a.ruta));
                if let Some(ruta) = foto {
                    poner_foto(&mut historial, &ruta);
                }
                let pedido = historial
                    .iter()
                    .rev()
                    .find(|m| m.rol == Rol::Usuario)
                    .map(Mensaje::texto_entero)
                    .unwrap_or_default();
                // Con la nube, que archivos tiene este chat (banco de
                // documentos, 15-oct: sin esto pedia que se los pegaran).
                // Despues de leer el pedido: lo que decide herramientas mira
                // lo que escribio el usuario, no esta nota.
                // Y la hora de ahora, al minuto (17-oct-2026): con ella y la
                // de cada archivo, «lo de esta mañana» se entiende solo.
                if motor.catalogo_completo()
                    && let Some(m) = historial.iter_mut().rev().find(|m| m.rol == Rol::Usuario)
                {
                    let desfase = super::desfase_local_ms();
                    if let Some(nota) = raiz.as_deref().and_then(|r| super::en_el_chat::nota_de_archivos(r, &proyecto, desfase)) {
                        m.partes.push(pixpin_ia::motor::Parte::Texto(nota));
                    }
                    m.partes.push(pixpin_ia::motor::Parte::Texto(super::nota_de_ahora(pixpin_shell::entorno::ahora_utc_ms(), desfase)));
                }
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
                    // Las dudas que cambian el resultado, con opciones en la
                    // burbuja (`preguntar_usuario`, 22-oct).
                    .con_preguntar(super::acciones::preguntar_en_la_burbuja(tx.clone(), parar_hilo.clone()))
                    .con_raiz(raiz.clone());
                // Las pruebas no tienen Ajustes: el banco dice con que modelo
                // de vision trabajan hoja_a_lienzo y anotar_inteligente.
                #[cfg(test)]
                if let Some(v) = VISION_EN_PRUEBAS.lock().unwrap_or_else(|e| e.into_inner()).clone() {
                    ejecutor = ejecutor.con_vision(v);
                }
                // «Calca esto» con la imagen pegada EN EL LIENZO y nada adjunto
                // (15-oct: contestaba «no hay imagen adjunta»): la del lienzo
                // (la elegida o la ultima) entra como adjunto y el modelo la ve.
                // Solo con la nube (el modelo local no se toca).
                if motor.catalogo_completo()
                    && !con_foto
                    && !adjuntos_con_imagen
                    && pide_imagen_del_lienzo(&pedido)
                    && super::al_lienzo::hay_quien_conteste()
                    && let Some(ruta) = ejecutor.imagen_del_lienzo()
                {
                    poner_foto(&mut historial, &ruta);
                    if let Some(m) = historial.iter_mut().rev().find(|m| m.rol == Rol::Usuario) {
                        m.partes.push(pixpin_ia::motor::Parte::Texto(
                            "

(La imagen adjunta es la ultima imagen pegada en el lienzo: «imagen del lienzo». Para calcarla o pasarla al lienzo usa calcar_imagen o hoja_a_lienzo con ella.)".into(),
                        ));
                    }
                }
                // El lienzo destino, resumido (tipos, #id, cajas, colores,
                // textos) al final del pedido: solo si el pedido es de dibujar
                // o editar y hay quien conteste (`pixpin_ia::edicion`).
                let ojos = con_ojos() && motor.ve_imagenes();
                let mirar = pixpin_ia::ojos::pide_mirar(&pedido);
                let de_lienzo = pixpin_ia::edicion::viene_a_cuento(&pedido) || mirar;
                if ojos && de_lienzo {
                    // La GPU de los ojos, lista antes de la primera vista.
                    super::ojos::precalentar();
                }
                if de_lienzo
                    && super::al_lienzo::hay_quien_conteste()
                    && let Some(r) = ejecutor.resumen_del_lienzo()
                {
                    pixpin_ia::edicion::anadir_resumen(&mut historial, &r);
                    // «Corrige», «alinea», «¿que falta?»: depende de lo que
                    // se ve; ademas del resumen, la vista (15-oct).
                    if ojos && mirar {
                        use pixpin_ia::agente::Ejecutor as _;
                        match ejecutor.ver(&pixpin_ia::ojos::Mirada::default()) {
                            Ok(v) => poner_vista(&mut historial, &v),
                            Err(e) => tracing::info!(%e, "asistente: sin vista al empezar"),
                        }
                    }
                }
                let buscar = |c: &str| contexto::buscar(&memoria.fuentes, c, 3);
                // La memoria de largo plazo, solo con la nube (el local no
                // se toca). Lo que el reinicio dejo atras, a la memoria sin
                // esperar.
                let largo_plazo = memoria.largo_plazo.clone().filter(|_| motor.catalogo_completo());
                if let Some(m) = &largo_plazo
                    && !memoria.a_guardar.is_empty()
                {
                    super::memoria::guardar_en_segundo_plano(motor.clone(), m.clone(), memoria.a_guardar.clone());
                }
                // Lo que la vuelta de planear puede anadir: al local, sin las de
                // solo la nube (`acciones::SOLO_NUBE`, «la IA local, no la toques»).
                let nube = motor.catalogo_completo();
                let todas: Vec<_> = pixpin_ia::herramientas::catalogo()
                    .into_iter()
                    .filter(|h| nube || !pixpin_ia::acciones::es_solo_nube(&h.nombre))
                    .collect();
                // El critico de respaldo (Sonnet), solo si hay revision.
                let critico = raiz.as_deref().filter(|_| ojos && de_lienzo).and_then(super::nube::critico);
                // El jefe (23-oct): con la nube, para lo complejo o de varias
                // partes (lo decide `orquesta::conviene` en el turno).
                let jefe = raiz.as_deref().filter(|_| motor.catalogo_completo()).and_then(super::nube::jefe);
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
                    ojos,
                    revisar: if ojos { revisiones(motor.as_ref()) } else { 0 },
                    memoria: largo_plazo.as_ref(),
                    critico: critico.as_deref(),
                    // Comprobar antes de contestar (rendirse, varias partes,
                    // algo que fallo): con la nube; el local no se toca.
                    repasar: nube,
                    jefe: jefe.as_deref(),
                    entrantes: None,
                };
                let recoger = || std::mem::take(&mut *entrantes_hilo.lock().unwrap_or_else(|e| e.into_inner()));
                let opciones = Opciones {
                    entrantes: Some(&recoger),
                    ..opciones
                };
                let mut recogido = Recogido::default();
                // El diario del asistente (fase 1: medir).
                let empezo = std::time::Instant::now();
                let mut apunte = pixpin_ia::diario::Entrada {
                    cuando: pixpin_shell::entorno::ahora_utc_ms(),
                    motor: raiz.as_deref().map(|r| super::nube::nombre_corto(super::nube::eleccion_reciente(r)).to_string()).unwrap_or_default(),
                    pedido: pixpin_ia::diario::recortar(&pedido),
                    ..Default::default()
                };
                let mut dicho = String::new();
                let resultado = agente::turno_con(
                    motor.as_ref(),
                    &mut historial,
                    &herramientas,
                    &mut ejecutor,
                    &opciones,
                    &mut |e| {
                        recogido.ver(&e);
                        apuntar_evento(&mut apunte, &mut dicho, &e);
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
                apunte.segundos = empezo.elapsed().as_secs_f32();
                apunte.se_rindio = agente::parece_rendirse(&dicho);
                apunte.fallo = resultado.is_err();
                apunte.cortado = resultado.as_ref().is_ok_and(|f| f.cortado);
                if let Some(r) = raiz.as_deref()
                    && let Err(e) = pixpin_ia::diario::apuntar(&r.join(DIARIO), &apunte)
                {
                    tracing::info!(%e, "asistente: no se pudo apuntar en el diario");
                }
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
            entrantes,
        }
    }

    /// **Le pasa al turno en marcha lo que el usuario acaba de escribir**:
    /// lo lee antes de su siguiente paso (o de acabar) y lo tiene en cuenta;
    /// «para» a secas lo detiene.
    pub fn anadir(&self, texto: String) {
        self.entrantes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(texto);
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

/// **Los ojos encendidos** (15-oct, `ojos.rs`). Siempre, salvo el banco,
/// que los apaga para medir sin ellos ([`apagar_ojos`]).
static SIN_OJOS: AtomicBool = AtomicBool::new(false);

pub fn con_ojos() -> bool {
    !SIN_OJOS.load(Ordering::Relaxed)
}

/// Apaga o enciende los ojos de todos los turnos (el banco).
#[cfg_attr(not(test), allow(dead_code))]
pub fn apagar_ojos(apagar: bool) {
    SIN_OJOS.store(apagar, Ordering::Relaxed);
}

/// **Cuantas rondas de correccion**: con la nube, las de la revision con
/// rubrica ([`agente::REVISIONES_DE_SERIE`], 22-oct: dos; de la segunda a
/// la tercera la mejora ya es minima y cuesta casi el doble). El local ni
/// mira ni revisa: el usuario pidio el 15-oct desarrollar primero la nube y
/// no tocar el local.
pub fn revisiones(motor: &dyn Motor) -> usize {
    if motor.catalogo_completo() {
        agente::REVISIONES_DE_SERIE
    } else {
        0
    }
}

/// **El diario del asistente** en la carpeta de datos (`pixpin_ia::diario`):
/// solo en este equipo, para medir.
pub const DIARIO: &str = "diario-del-asistente.jsonl";

/// Lo que un evento cuenta para el diario.
fn apuntar_evento(a: &mut pixpin_ia::diario::Entrada, dicho: &mut String, e: &agente::Evento) {
    use agente::Evento as E;
    use pixpin_ia::orquesta::EstadoTarea;
    match e {
        E::Texto(t) => dicho.push_str(t),
        E::Resultado { ok: true, .. } => a.herramientas_ok += 1,
        E::Resultado { ok: false, .. } => a.herramientas_error += 1,
        E::Llamando(n) if n == "repasar" => a.repasos += 1,
        E::Llamando(n) if n == "jefe:planear" => a.jefe = true,
        // La lista del jefe o la de pasos del turno (25-oct, `pasos.rs`): que
        // hubo jefe lo dice «jefe:planear», no la lista.
        E::Tareas(t) => {
            a.tareas = t.len();
            a.tareas_hechas = t.iter().filter(|x| x.estado == EstadoTarea::Hecha).count();
            // Cada vuelta a corregir avisa una vez con su tarea «corrigiendo».
            a.correcciones += usize::from(t.iter().any(|x| x.estado == EstadoTarea::Corrigiendo));
        }
        _ => {}
    }
}

/// Pega la vista del lienzo al pedido del usuario (la imagen delante, como
/// una foto adjunta, y su leyenda detras del texto).
fn poner_vista(historial: &mut [Mensaje], v: &pixpin_ia::ojos::Vista) {
    let Some(m) = historial.iter_mut().rev().find(|m| m.rol == Rol::Usuario) else {
        return;
    };
    m.partes.insert(
        0,
        pixpin_ia::motor::Parte::Imagen {
            bytes: v.png.clone(),
            mime: "image/png".into(),
        },
    );
    m.partes.push(pixpin_ia::motor::Parte::Texto(format!(
        "\n\n(La imagen adjunta es la vista del lienzo ahora; {})",
        v.texto.trim_start_matches(pixpin_ia::ojos::MARCA).trim()
    )));
}

/// Pega la foto, reducida, al ultimo mensaje (el del usuario). Si no se
/// puede leer, la pregunta va sin ella: mejor contestar sin ver que no
/// contestar.
fn poner_foto(historial: &mut [Mensaje], ruta: &std::path::Path) {
    let Some(ultimo) = historial.last_mut() else {
        return;
    };
    let mut texto = ultimo.texto_entero();
    // La foto de un problema (14-oct): el OCR de Windows, a toda resolucion,
    // pegado al mensaje; el modelo lee las formulas de la imagen y el texto
    // menudo del OCR (`problema::pista_de_ocr`).
    if pixpin_ia::problema::pide_problema(&texto) {
        texto.push_str(&ocr_de_la_foto(ruta));
    }
    match super::foto::preparar_fichero(ruta) {
        Ok((bytes, mime)) => *ultimo = Mensaje::usuario_con_imagen(texto, bytes, mime),
        Err(e) => tracing::warn!(%e, "asistente: la foto no se pudo preparar"),
    }
}

/// Lo que el OCR de Windows lee en la foto, ya dicho para el modelo; vacio
/// si no hay OCR en este Windows o la foto no se abre (el modelo la mira
/// igual).
fn ocr_de_la_foto(ruta: &std::path::Path) -> String {
    if !pixpin_ocr::disponible() {
        return String::new();
    }
    let Ok(img) = pixpin_codec::imagen::cargar(ruta) else {
        return String::new();
    };
    match pixpin_ocr::reconocer(img.ancho, img.alto, &img.pixeles) {
        Ok(lineas) => pixpin_ia::problema::pista_de_ocr(
            &lineas.into_iter().map(|l| l.texto).collect::<Vec<_>>(),
        ),
        Err(e) => {
            tracing::warn!(?e, "asistente: el OCR de la foto del problema fallo");
            String::new()
        }
    }
}

/// Si el pedido es calcar, dibujar o pasar «esto» (una imagen): «calca
/// esto», «dibújalo con las herramientas», «pásalo al lienzo».
pub fn pide_imagen_del_lienzo(pedido: &str) -> bool {
    let q = pixpin_ia::acciones::normalizar(pedido);
    let palabras: Vec<&str> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .collect();
    pixpin_ia::seleccion::pide_la_imagen(&palabras)
        && !pixpin_ia::seleccion::pide_pegar_o_recortar(&palabras)
}
