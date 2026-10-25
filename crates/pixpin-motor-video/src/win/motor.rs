//! **El hilo del motor**: el unico que toca la GPU, Media Foundation y el
//! fichero. Todo le llega por un canal con tope ([`Mensaje`]): el sonido,
//! los avisos de fotograma nuevo, la pausa y el final.
//!
//! Por que un solo hilo: el contexto inmediato de Direct3D y el escritor de
//! Media Foundation se usan siempre desde el mismo sitio, sin cerrojos
//! propios, y el orden entre el sonido y los fotogramas es el de llegada.
//! Los codificadores de la grafica son asincronos y Media Foundation los
//! mueve en sus hilos sobre el mismo dispositivo: por eso el dispositivo va
//! protegido para varios hilos (`SetMultithreadProtected`) y el hilo esta
//! en el apartamento multihilo de COM.
//!
//! Lo que hace en cada vuelta: atiende un mensaje (o se despierta a la hora
//! que dijo el ritmo) y decide si toca un fotograma ([`crate::ritmo`]). El
//! sonido se escribe siempre; los fotogramas solo si el codificador tiene
//! sitio.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, SyncSender, channel, sync_channel};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

use pixpin_capture::Dispositivo;
use windows::Win32::Graphics::Direct3D11::ID3D11Multithread;
use windows::core::Interface;

use super::captura::Captura;
use super::com::{self, ComDelHilo, arrancar_media_foundation};
use super::conversor::Conversor;
use super::tubo::{Cpu, Escrito, Grafica, Pedido, Software, Tubo};
use crate::cadena::{self, Capacidades};
use crate::cola::{Atasco, CAPACIDAD_MENSAJES};
use crate::perfil::{Ajustes, Perfil};
use crate::reloj::{Reloj, UNIDADES_POR_SEGUNDO};
use crate::ritmo::{self, Decision, Medidor};
use crate::{
    Codificador, ErrorMotor, Estadisticas, Formato, FormatoAudio, Opciones, Resumen, archivo,
};

/// Lo que se le dice al hilo del motor.
pub(crate) enum Mensaje {
    /// Hay un fotograma nuevo retenido (solo despierta).
    Fotograma,
    /// Sonido intercalado que empieza en `posicion` (por canal).
    Audio {
        muestras: Vec<i16>,
        posicion: u64,
    },
    /// Pausa o sigue, a la hora `hora` (100 ns del sistema).
    Pausa {
        pausar: bool,
        hora: i64,
    },
    Terminar,
    Tirar,
}

#[derive(Debug, Clone, Copy)]
struct Arranque {
    codificador: Codificador,
    formato: Formato,
    medidas: (u32, u32),
}

/// Lo que el hilo cuenta y cualquiera puede leer sin pararlo.
#[derive(Default)]
struct Contadores {
    fotogramas: AtomicU64,
    caidos: AtomicU64,
    bytes: AtomicU64,
    fps: AtomicU32,
    cortado: AtomicBool,
    arranque: OnceLock<Arranque>,
}

/// **Un video grabandose.** Se crea con [`MotorVideo::empezar`] y se cierra
/// con [`MotorVideo::terminar`]; soltarlo sin terminar lo tira (borra lo
/// escrito). Se puede mover entre hilos: lo que toca Windows vive en el
/// hilo del motor.
pub struct MotorVideo {
    tx: SyncSender<Mensaje>,
    hilo: Option<JoinHandle<Result<Resumen, ErrorMotor>>>,
    contadores: Arc<Contadores>,
}

impl MotorVideo {
    /// **Empieza a grabar** en `destino` (que no existe hasta terminar: se
    /// escribe en `<destino>.parte`). Vuelve cuando ya esta grabando, con
    /// el codificador elegido ([`MotorVideo::estado`]), o con por que no.
    pub fn empezar(opciones: Opciones, destino: PathBuf) -> Result<MotorVideo, ErrorMotor> {
        opciones.validar()?;
        let (tx, rx) = sync_channel(CAPACIDAD_MENSAJES);
        let (listo_tx, listo_rx) = channel();
        let contadores = Arc::new(Contadores::default());
        let hilo = {
            let tx = tx.clone();
            let contadores = Arc::clone(&contadores);
            std::thread::Builder::new()
                .name("motor-video".into())
                .spawn(move || hilo(opciones, destino, rx, tx, listo_tx, contadores))
                .map_err(|_| ErrorMotor::HiloCaido)?
        };
        match listo_rx.recv() {
            Ok(Ok(())) => Ok(MotorVideo {
                tx,
                hilo: Some(hilo),
                contadores,
            }),
            Ok(Err(e)) => {
                let _ = hilo.join();
                Err(e)
            }
            Err(_) => {
                let _ = hilo.join();
                Err(ErrorMotor::HiloCaido)
            }
        }
    }

    /// **Da un bloque de sonido** intercalado que empieza en `posicion`
    /// (muestras por canal desde el principio). El sonido nunca se tira: si
    /// la cola del motor esta llena, espera a que haya sitio. Un error es
    /// que el motor ya paro (se corto); [`MotorVideo::terminar`] dice por que.
    pub fn poner_audio(&self, muestras: &[i16], posicion: u64) -> Result<(), ErrorMotor> {
        if muestras.is_empty() {
            return Ok(());
        }
        self.tx
            .send(Mensaje::Audio {
                muestras: muestras.to_vec(),
                posicion,
            })
            .map_err(|_| ErrorMotor::HiloCaido)
    }

    /// Pausa: no se escriben fotogramas y, sin sonido, el reloj no corre.
    /// Con sonido basta con que no llegue (el reloj es el suyo), pero
    /// decirlo evita el ultimo cuarto de segundo de video de mas.
    pub fn pausar(&self) {
        let _ = self.tx.send(Mensaje::Pausa {
            pausar: true,
            hora: com::ahora(),
        });
    }

    pub fn seguir(&self) {
        let _ = self.tx.send(Mensaje::Pausa {
            pausar: false,
            hora: com::ahora(),
        });
    }

    /// Como va.
    pub fn estado(&self) -> Estadisticas {
        let c = &self.contadores;
        let a = c.arranque.get().copied().unwrap_or(Arranque {
            codificador: Codificador::Cpu,
            formato: Formato::H264,
            medidas: (0, 0),
        });
        Estadisticas {
            fps: f32::from_bits(c.fps.load(Ordering::Relaxed)),
            fotogramas: c.fotogramas.load(Ordering::Relaxed),
            caidos: c.caidos.load(Ordering::Relaxed),
            bytes: c.bytes.load(Ordering::Relaxed),
            codificador: a.codificador,
            formato: a.formato,
            medidas: a.medidas,
            cortado: c.cortado.load(Ordering::Acquire),
        }
    }

    /// **Cierra el MP4** (el ultimo fotograma, el indice, el nombre bueno)
    /// y dice como quedo.
    pub fn terminar(mut self) -> Result<Resumen, ErrorMotor> {
        let _ = self.tx.send(Mensaje::Terminar);
        let hilo = self.hilo.take().ok_or(ErrorMotor::HiloCaido)?;
        hilo.join().map_err(|_| ErrorMotor::HiloCaido)?
    }
}

impl Drop for MotorVideo {
    fn drop(&mut self) {
        // Soltarlo sin terminar es tirarlo: no puede quedar un hilo
        // grabando la pantalla sin nadie que lo pare.
        if let Some(h) = self.hilo.take() {
            let _ = self.tx.send(Mensaje::Tirar);
            let _ = h.join();
        }
    }
}

/// El cuerpo del hilo.
fn hilo(
    opciones: Opciones,
    destino: PathBuf,
    rx: Receiver<Mensaje>,
    tx: SyncSender<Mensaje>,
    listo: Sender<Result<(), ErrorMotor>>,
    contadores: Arc<Contadores>,
) -> Result<Resumen, ErrorMotor> {
    // El primero en crearse y el ultimo en soltarse.
    let _com = ComDelHilo::nuevo();
    arrancar_media_foundation();
    match Hilo::preparar(&opciones, &destino, tx, contadores) {
        Ok(h) => {
            let _ = listo.send(Ok(()));
            h.correr(&rx)
        }
        Err(e) => {
            let _ = listo.send(Err(e));
            Err(ErrorMotor::HiloCaido)
        }
    }
}

/// Protege el contexto inmediato para varios hilos: el codificador de la
/// grafica lo usa desde los de Media Foundation mientras el motor convierte.
fn proteger(d: &Dispositivo) {
    if let Ok(m) = d.d3d().cast::<ID3D11Multithread>() {
        // SAFETY: interfaz valida del dispositivo vivo; solo cambia una
        // bandera del contexto.
        unsafe {
            let _ = m.SetMultithreadProtected(true);
        }
    }
}

/// Lo que vive en el hilo del motor. Los campos se sueltan en el orden en
/// que estan: la captura (que no llegue nada mas), el tubo (codificador y
/// fichero) y el dispositivo.
struct Hilo {
    captura: Option<Captura>,
    tubo: Option<Tubo>,
    disp: Dispositivo,
    contadores: Arc<Contadores>,
    audio: Option<FormatoAudio>,
    reloj: Reloj,
    inicio: i64,
    minimo: i64,
    duracion: i64,
    ultimo_intento: Option<i64>,
    medidor: Medidor,
    atasco: Atasco,
    caidos: u64,
    cortado: Option<String>,
    avisada_cerrada: bool,
    proximo_recuento: i64,
}

impl Hilo {
    fn preparar(
        opciones: &Opciones,
        destino: &std::path::Path,
        tx: SyncSender<Mensaje>,
        contadores: Arc<Contadores>,
    ) -> Result<Hilo, ErrorMotor> {
        let disp = Dispositivo::nuevo().map_err(|e| ErrorMotor::Fuente(e.to_string()))?;
        proteger(&disp);
        let ajustes = opciones.perfil.ajustes();
        let captura = Captura::abrir(
            &disp,
            opciones.fuente,
            opciones.con_cursor,
            ritmo::minimo_entre(ajustes.por_segundo),
            tx,
        )?;
        let (cw, ch) = captura.tamano();
        let medidas = opciones.perfil.medidas(cw, ch);
        let conv = if disp.soporta_video() {
            match Conversor::nuevo(
                disp.d3d(),
                disp.contexto(),
                (cw, ch),
                medidas,
                ajustes.por_segundo,
            ) {
                Ok(c) => Some(c),
                Err(e) => {
                    tracing::warn!(?e, "la grafica no tiene procesador de video");
                    None
                }
            }
        } else {
            None
        };
        let con_gpu = conv.is_some();
        let caps = Capacidades {
            conversor_gpu: con_gpu,
            h264_hardware: con_gpu && super::hay_h264_por_hardware(),
            hevc_hardware: con_gpu && opciones.hevc && super::hay_hevc_por_hardware(),
        };
        let tubo = elegir(
            &disp,
            conv,
            caps,
            opciones,
            destino,
            medidas,
            ajustes,
            (cw, ch),
        )?;
        let por_segundo = match &tubo {
            Tubo::Cpu(_) => cpu_ajustes(ajustes).por_segundo,
            _ => ajustes.por_segundo,
        };
        let arranque = Arranque {
            codificador: tubo.codificador(),
            formato: tubo.formato(),
            medidas: tubo.medidas(),
        };
        let _ = contadores.arranque.set(arranque);
        tracing::info!(
            codificador = arranque.codificador.clave(),
            formato = ?arranque.formato,
            ancho = arranque.medidas.0,
            alto = arranque.medidas.1,
            por_segundo,
            fuente_ancho = cw,
            fuente_alto = ch,
            grafica = %disp.adaptador(),
            "motor de video en marcha"
        );
        let ahora = com::ahora();
        Ok(Hilo {
            captura: Some(captura),
            tubo: Some(tubo),
            disp,
            contadores,
            audio: opciones.audio,
            reloj: Reloj::nuevo(opciones.audio.map(|a| a.muestreo), ahora),
            inicio: ahora,
            minimo: ritmo::minimo_entre(por_segundo),
            duracion: UNIDADES_POR_SEGUNDO / i64::from(por_segundo.max(1)),
            ultimo_intento: None,
            medidor: Medidor::default(),
            atasco: Atasco::default(),
            caidos: 0,
            cortado: None,
            avisada_cerrada: false,
            proximo_recuento: ahora + UNIDADES_POR_SEGUNDO,
        })
    }

    fn correr(mut self, rx: &Receiver<Mensaje>) -> Result<Resumen, ErrorMotor> {
        let terminar = loop {
            let ahora = com::ahora();
            let hay = self
                .captura
                .as_ref()
                .is_some_and(|c| c.hora_retenida().is_some());
            // Entre 1 y 250 ms: lo que diga el ritmo, con un techo para que
            // los contadores se refresquen aunque no pase nada.
            let espera = if self.reloj.en_pausa() {
                // En pausa no hay nada que decidir: se espera a un mensaje.
                UNIDADES_POR_SEGUNDO / 4
            } else {
                ritmo::espera(ahora, self.inicio, self.ultimo_intento, hay, self.minimo)
                    .clamp(10_000, UNIDADES_POR_SEGUNDO / 4)
            };
            match rx.recv_timeout(Duration::from_micros((espera / 10) as u64)) {
                Ok(Mensaje::Audio { muestras, posicion }) => {
                    if let Err(e) = self.audio(&muestras, posicion) {
                        self.cortar(e);
                        break true;
                    }
                }
                Ok(Mensaje::Fotograma) | Err(RecvTimeoutError::Timeout) => {}
                Ok(Mensaje::Pausa { pausar, hora }) => self.pausa(pausar, hora),
                Ok(Mensaje::Terminar) => break true,
                Ok(Mensaje::Tirar) | Err(RecvTimeoutError::Disconnected) => break false,
            }
            if let Err(e) = self.paso() {
                self.cortar(e);
                break true;
            }
            self.recontar();
        };
        if terminar {
            self.cerrar()
        } else {
            // Sin cerrar el MP4: el `.parte` se borra al soltar el tubo.
            self.captura = None;
            self.tubo = None;
            tracing::info!("video tirado sin guardar");
            Err(ErrorMotor::Tirado)
        }
    }

    fn audio(&mut self, muestras: &[i16], posicion: u64) -> Result<(), ErrorMotor> {
        let Some(formato) = self.audio else {
            return Ok(());
        };
        if let Some(t) = self.tubo.as_mut() {
            t.audio(muestras, posicion)?;
        }
        self.reloj
            .sonido(posicion + formato.por_canal(muestras.len()), com::ahora());
        Ok(())
    }

    fn pausa(&mut self, pausar: bool, hora: i64) {
        if pausar {
            self.reloj.pausar(hora);
            if let Some(r) = self.captura.as_ref().and_then(Captura::tomar) {
                r.soltar();
            }
        } else {
            self.reloj.seguir(hora);
        }
    }

    /// **Un paso del video**: si toca, un fotograma nuevo o el repetido.
    fn paso(&mut self) -> Result<(), ErrorMotor> {
        let ahora = com::ahora();
        let Some(captura) = self.captura.as_ref() else {
            return Ok(());
        };
        if captura.cerrada() && !self.avisada_cerrada {
            self.avisada_cerrada = true;
            tracing::info!("la ventana grabada se cerro; el video sigue con su ultima imagen");
        }
        if self.reloj.en_pausa() {
            if let Some(r) = captura.tomar() {
                r.soltar();
            }
            return Ok(());
        }
        let hora = captura.hora_retenida();
        let escrito = match ritmo::decidir(
            ahora,
            self.inicio,
            self.ultimo_intento,
            hora.is_some(),
            self.minimo,
        ) {
            Decision::Esperar => return Ok(()),
            Decision::Nuevo => {
                self.ultimo_intento = Some(ahora);
                // Si el reloj aun no lo puede fechar (esperando al sonido
                // tras una pausa) se queda retenido: es la imagen mas nueva
                // y no se puede perder.
                let Some(t) = hora.and_then(|h| self.reloj.fotograma(h)) else {
                    return Ok(());
                };
                let Some(r) = captura.tomar() else {
                    return Ok(());
                };
                let Some(tubo) = self.tubo.as_mut() else {
                    r.soltar();
                    return Ok(());
                };
                let hecho = match r.textura() {
                    Ok(tex) => tubo.fotograma(&self.disp, &tex, r.contenido, t, self.duracion),
                    // Un fotograma sin textura (raro: la superficie de un
                    // pool recien recreado) se salta, no corta el video.
                    Err(e) => {
                        tracing::debug!(?e, "un fotograma sin textura");
                        Ok(Escrito::Caido)
                    }
                };
                // La GPU ya tiene en su cola todo lo que lo lee: se suelta.
                r.soltar();
                hecho?
            }
            Decision::Repetir => {
                self.ultimo_intento = Some(ahora);
                let Some(t) = self.reloj.fotograma(ahora) else {
                    return Ok(());
                };
                let Some(tubo) = self.tubo.as_mut() else {
                    return Ok(());
                };
                tubo.repetir(t, self.duracion)?
            }
        };
        self.contar(escrito, ahora)
    }

    fn contar(&mut self, e: Escrito, ahora: i64) -> Result<(), ErrorMotor> {
        match e {
            Escrito::Hecho => {
                self.atasco.entro();
                self.medidor.uno(ahora);
                if let Some(t) = &self.tubo {
                    self.contadores
                        .fotogramas
                        .store(t.escritos(), Ordering::Relaxed);
                }
            }
            Escrito::Caido => {
                self.caidos += 1;
                self.contadores.caidos.store(self.caidos, Ordering::Relaxed);
                if self.atasco.no_entro(ahora) {
                    return Err(ErrorMotor::SinCodificador(
                        "el codificador dejo de aceptar fotogramas".into(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Lo que ocupa el fichero y el ritmo, una vez por segundo.
    fn recontar(&mut self) {
        let ahora = com::ahora();
        if ahora < self.proximo_recuento {
            return;
        }
        self.proximo_recuento = ahora + UNIDADES_POR_SEGUNDO;
        if let Some(t) = &self.tubo {
            self.contadores
                .bytes
                .store(archivo::bytes(t.parcial()), Ordering::Relaxed);
        }
        let fps = self.medidor.por_segundo(ahora);
        self.contadores.fps.store(fps.to_bits(), Ordering::Relaxed);
    }

    /// El video se corta a mitad: se guarda lo que habia.
    fn cortar(&mut self, e: ErrorMotor) {
        tracing::warn!(?e, "el video se corto; se guarda lo grabado");
        self.cortado = Some(e.to_string());
        self.contadores.cortado.store(true, Ordering::Release);
    }

    /// **Cierra el MP4**: para la captura, pone el ultimo fotograma donde
    /// acaba el sonido (para que las dos pistas duren lo mismo), escribe el
    /// indice y lo pone en su nombre.
    fn cerrar(mut self) -> Result<Resumen, ErrorMotor> {
        self.captura = None;
        let ahora = com::ahora();
        let mut tubo = self.tubo.take().ok_or(ErrorMotor::HiloCaido)?;
        let fin = self.reloj.final_(ahora);
        if tubo.escritos() == 0 {
            // Ni un fotograma (una grabacion de medio segundo, una pantalla
            // que nunca dio imagen): uno negro que lo cubra todo, para que
            // el MP4 valga.
            let _ = tubo.repetir(0, fin.max(self.duracion));
        }
        if let Err(e) = tubo.vaciar(fin, self.duracion) {
            tracing::warn!(?e, "el ultimo fotograma del video no entro");
        }
        let (codificador, formato, medidas) = (tubo.codificador(), tubo.formato(), tubo.medidas());
        let fotogramas = tubo.escritos();
        let (ruta, fragmentado) = tubo.terminar()?;
        let bytes = archivo::bytes(&ruta);
        tracing::info!(
            fotogramas,
            caidos = self.caidos,
            bytes,
            segundos = fin / UNIDADES_POR_SEGUNDO,
            "video cerrado"
        );
        Ok(Resumen {
            ruta,
            duracion_ms: (fin.max(0) / 10_000) as u64,
            fotogramas,
            caidos: self.caidos,
            bytes,
            codificador,
            formato,
            medidas,
            fragmentado,
            cortado: self.cortado.take(),
        })
    }
}

/// Los ajustes del camino por la CPU: los de «Ligero», sin pasar del ritmo
/// del perfil pedido.
fn cpu_ajustes(pedido: Ajustes) -> Ajustes {
    let ligero = Perfil::Ligero.ajustes();
    Ajustes {
        por_segundo: ligero.por_segundo.min(pedido.por_segundo),
        ..ligero
    }
}

/// **Recorre la cadena de caidas** hasta que un camino arranca.
#[allow(clippy::too_many_arguments)]
fn elegir(
    disp: &Dispositivo,
    mut conv: Option<Conversor>,
    caps: Capacidades,
    opciones: &Opciones,
    destino: &std::path::Path,
    medidas: (u32, u32),
    ajustes: Ajustes,
    fuente: (u32, u32),
) -> Result<Tubo, ErrorMotor> {
    let mut motivos = Vec::new();
    for (codificador, formato) in cadena::intentos(caps, opciones.hevc) {
        let pedido = Pedido {
            destino,
            formato,
            medidas,
            ajustes,
            audio: opciones.audio,
            fragmentado: opciones.fragmentado,
        };
        let hecho = match codificador {
            Codificador::Hardware | Codificador::Software => {
                let Some(c) = conv.take() else {
                    continue;
                };
                let abierto = if codificador == Codificador::Hardware {
                    Grafica::abrir(disp, c, &pedido).map(Tubo::Grafica)
                } else {
                    Software::abrir(disp, c, &pedido).map(Tubo::Software)
                };
                abierto.map_err(|devuelto| {
                    let (c, e) = *devuelto;
                    conv = Some(c);
                    e
                })
            }
            Codificador::Cpu => {
                let a = cpu_ajustes(ajustes);
                Cpu::abrir(&Pedido {
                    medidas: Perfil::Ligero.medidas(fuente.0, fuente.1),
                    ajustes: a,
                    ..pedido
                })
                .map(Tubo::Cpu)
            }
        };
        match hecho {
            Ok(t) => return Ok(t),
            Err(e) => {
                tracing::warn!(
                    codificador = codificador.clave(),
                    ?formato,
                    ?e,
                    "un codificador no arranco; se prueba el siguiente"
                );
                motivos.push(format!("{}: {e}", codificador.clave()));
            }
        }
    }
    Err(ErrorMotor::SinCodificador(motivos.join("; ")))
}
