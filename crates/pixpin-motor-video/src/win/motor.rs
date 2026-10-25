//! **El hilo del motor**: el unico que toca la GPU, Media Foundation y el
//! fichero. Todo le llega por un canal con tope ([`Mensaje`]): el sonido,
//! los avisos de fotograma nuevo, la pausa y el final.
//!
//! Por que un solo hilo: el contexto inmediato de Direct3D y el escritor de
//! Media Foundation se usan siempre desde el mismo sitio, y el orden entre
//! el sonido y los fotogramas es el de llegada. Los codificadores de la
//! grafica son asincronos y Media Foundation los mueve en sus hilos sobre
//! el mismo dispositivo: por eso el dispositivo va protegido para varios
//! hilos (`SetMultithreadProtected`), cada tanda de trabajo en la GPU va con
//! su cerrojo (`com::EnLaGpu`) y el hilo esta en el apartamento multihilo
//! de COM.
//!
//! Lo que decide (el ritmo, el reloj, el atasco, el cierre) es el nucleo
//! comun ([`crate::nucleo`]); aqui va lo que es de Windows:
//!
//! - **la grafica** donde se graba (`grafica::dispositivo_para`: la del
//!   monitor, nunca una virtual) y la cadena de caminos, con el tope de
//!   Quick Sync en las Intel antiguas y el reintento mas pequeno;
//! - **la grafica que se pierde** (TDR, controlador actualizado): se ve por
//!   el codigo de un fallo o mirando `GetDeviceRemovedReason` una vez por
//!   segundo, y se vuelve a montar todo en un dispositivo nuevo sin cerrar
//!   el MP4 ([`Hilo::recuperar`], como `RebuildDevice` de OBS), hasta tres
//!   veces cada diez minutos (`crate::recuperacion`);
//! - **el monitor que se desconecta**: se busca otra vez por su nombre cada
//!   tres segundos y, si no vuelve, el principal (`crate::reapertura`).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, SyncSender, channel, sync_channel};
use std::thread::JoinHandle;
use std::time::Duration;

use pixpin_capture::Dispositivo;
use windows::Win32::Graphics::Direct3D11::ID3D11Multithread;
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::Win32::Media::MediaFoundation::{MFVideoFormat_H264, MFVideoFormat_HEVC};
use windows::core::Interface;

use super::captura::{Captura, Retenido};
use super::com::{self, ComDelHilo, arrancar_media_foundation};
use super::conversor::Conversor;
use super::grafica;
use super::tubo::{Cpu, EnLaGrafica, Grafica, Pedido, Software, Tubo};
use crate::adaptador::Fabricante;
use crate::cadena::{self, Capacidades, Peculiaridades};
use crate::cola::CAPACIDAD_MENSAJES;
use crate::nucleo::{Arranque, Contadores, Nucleo};
use crate::perfil::{Ajustes, Perfil};
use crate::reapertura::{self, Reintento};
use crate::recuperacion::Presupuesto;
use crate::reloj::UNIDADES_POR_SEGUNDO;
use crate::{Camino, ErrorMotor, Estadisticas, Fuente, Opciones, Resumen, archivo, geometria};

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
        self.contadores.estadisticas()
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
    match Hilo::preparar(opciones, &destino, tx, contadores) {
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

/// Una captura que no hay (se perdio y aun no se pudo volver a abrir): el
/// nucleo sigue repitiendo la ultima imagen con ella.
struct Ninguna;

impl crate::plataforma::Captura for Ninguna {
    type Retenido = Retenido;

    fn hora_retenida(&self) -> Option<i64> {
        None
    }

    fn tomar(&self) -> Option<Retenido> {
        None
    }

    fn cerrada(&self) -> bool {
        true
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
    nucleo: Nucleo,
    opciones: Opciones,
    /// Lo que se graba ahora (un monitor puede cambiar al reabrirse).
    fuente: Fuente,
    nombre_monitor: String,
    despertar: SyncSender<Mensaje>,
    reapertura: Reintento,
    presupuesto: Presupuesto,
    vueltas: u32,
    cortado: Option<String>,
    avisada_cerrada: bool,
    proximo_recuento: i64,
}

impl Hilo {
    fn preparar(
        opciones: Opciones,
        destino: &std::path::Path,
        tx: SyncSender<Mensaje>,
        contadores: Arc<Contadores>,
    ) -> Result<Hilo, ErrorMotor> {
        let disp = grafica::dispositivo_para(opciones.fuente)?;
        proteger(&disp);
        let ajustes = opciones.perfil.ajustes();
        let captura = Captura::abrir(
            &disp,
            opciones.fuente,
            opciones.con_cursor,
            crate::ritmo::minimo_entre(ajustes.por_segundo),
            tx.clone(),
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
        let id = grafica::identidad(disp.d3d());
        let h264 = grafica::codificadores_de_hardware(MFVideoFormat_H264);
        let hevc = if opciones.hevc {
            grafica::codificadores_de_hardware(MFVideoFormat_HEVC)
        } else {
            Vec::new()
        };
        for c in &h264 {
            tracing::info!(nombre = %c.nombre, fabricante = ?c.fabricante, "codificador H.264 de hardware");
        }
        // Solo vale darle las texturas a un codificador de la misma grafica
        // (lo que no se sabe, se da por bueno: lo comprueba el escritor).
        let de_la_grafica = id.map(|i| Fabricante::de_id(i.fabricante));
        let caps = Capacidades {
            conversor_gpu: con_gpu,
            h264_hardware: con_gpu && !h264.is_empty(),
            hevc_hardware: con_gpu && !hevc.is_empty(),
            hardware_en_la_grafica: de_la_grafica.is_none_or(|f| {
                h264.iter()
                    .chain(hevc.iter())
                    .any(|c| c.fabricante.is_none_or(|x| x == f))
            }),
        };
        let pec = id
            .map(|i| cadena::peculiaridades(i.fabricante, i.dispositivo))
            .unwrap_or_default();
        let tubo = elegir(
            &disp,
            conv,
            caps,
            pec,
            &opciones,
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
        contadores.arrancar(arranque);
        tracing::info!(
            camino = tubo.camino().clave(),
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
        let nombre_monitor = match opciones.fuente {
            Fuente::Monitor(h) => grafica::nombre_del_monitor(HMONITOR(h as *mut _)),
            Fuente::Ventana(_) => String::new(),
        };
        Ok(Hilo {
            captura: Some(captura),
            tubo: Some(tubo),
            disp,
            contadores,
            nucleo: Nucleo::nuevo(opciones.audio, por_segundo, ahora),
            opciones,
            fuente: opciones.fuente,
            nombre_monitor,
            despertar: tx,
            reapertura: Reintento::default(),
            presupuesto: Presupuesto::default(),
            vueltas: 0,
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
                .is_some_and(|c| crate::plataforma::Captura::hora_retenida(c).is_some());
            let espera = self.nucleo.espera(ahora, hay);
            match rx.recv_timeout(Duration::from_micros((espera / 10) as u64)) {
                Ok(Mensaje::Audio { muestras, posicion }) => {
                    if let Err(e) = self.audio(&muestras, posicion)
                        && !self.tratar(e)
                    {
                        break true;
                    }
                }
                Ok(Mensaje::Fotograma) | Err(RecvTimeoutError::Timeout) => {}
                Ok(Mensaje::Pausa { pausar, hora }) => {
                    self.nucleo.pausa(self.captura.as_ref(), pausar, hora)
                }
                Ok(Mensaje::Terminar) => break true,
                Ok(Mensaje::Tirar) | Err(RecvTimeoutError::Disconnected) => break false,
            }
            if let Err(e) = self.paso()
                && !self.tratar(e)
            {
                break true;
            }
            if !self.recontar() {
                break true;
            }
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
        let Some(tubo) = self.tubo.as_mut() else {
            return Ok(());
        };
        let mut t = EnLaGrafica {
            tubo,
            disp: &self.disp,
        };
        self.nucleo.audio(&mut t, muestras, posicion, com::ahora())
    }

    /// **Un paso del video**: la fuente que se fue, y lo que diga el nucleo.
    fn paso(&mut self) -> Result<(), ErrorMotor> {
        let ahora = com::ahora();
        let sin_fuente = self
            .captura
            .as_ref()
            .is_none_or(crate::plataforma::Captura::cerrada);
        if sin_fuente {
            if !self.avisada_cerrada {
                self.avisada_cerrada = true;
                tracing::info!("la fuente del video se fue; el video sigue con su ultima imagen");
            }
            self.reabrir(ahora);
        }
        let Some(tubo) = self.tubo.as_mut() else {
            return Ok(());
        };
        let mut t = EnLaGrafica {
            tubo,
            disp: &self.disp,
        };
        match &self.captura {
            Some(c) => self.nucleo.paso(ahora, c, &mut t, &self.contadores)?,
            None => self
                .nucleo
                .paso(ahora, &Ninguna, &mut t, &self.contadores)?,
        };
        Ok(())
    }

    /// **Vuelve a abrir la fuente** si se puede: un monitor que se
    /// desconecto (por su nombre, o el principal), o la captura que no se
    /// pudo abrir tras una vuelta de la grafica. Una ventana cerrada no
    /// vuelve.
    fn reabrir(&mut self, ahora: i64) {
        if matches!(self.fuente, Fuente::Ventana(_)) && self.captura.is_some() {
            return;
        }
        if !self.reapertura.toca(ahora) {
            return;
        }
        let fuente = match self.fuente {
            Fuente::Monitor(_) => Fuente::Monitor(reapertura::elegir_monitor(
                &self.nombre_monitor,
                &grafica::monitores(),
                grafica::principal().0 as isize,
            )),
            v => v,
        };
        // La vieja fuera antes de abrir la nueva: su manejador ya no tiene
        // nada que dar.
        self.captura = None;
        match self.abrir_captura(fuente) {
            Ok(c) => {
                tracing::info!(?fuente, "la fuente del video volvio");
                if let Fuente::Monitor(h) = fuente {
                    self.nombre_monitor = grafica::nombre_del_monitor(HMONITOR(h as *mut _));
                }
                self.captura = Some(c);
                self.fuente = fuente;
                self.reapertura.olvidar();
                self.avisada_cerrada = false;
            }
            Err(e) => tracing::debug!(?e, "la fuente del video aun no vuelve"),
        }
    }

    fn abrir_captura(&self, fuente: Fuente) -> Result<Captura, ErrorMotor> {
        Captura::abrir(
            &self.disp,
            fuente,
            self.opciones.con_cursor,
            self.nucleo.minimo(),
            self.despertar.clone(),
        )
    }

    /// **Que hacer con un fallo**: si es la grafica que se fue, volver a
    /// montarlo todo; si no (o si no se pudo), cortar el video y guardar lo
    /// que habia. Devuelve si se sigue grabando.
    fn tratar(&mut self, e: ErrorMotor) -> bool {
        let perdida = e.es_perdida_de_la_grafica() || grafica::perdido(self.disp.d3d()).is_some();
        if !perdida {
            self.cortar(e);
            return false;
        }
        match self.recuperar(&e) {
            Ok(()) => true,
            Err(otro) => {
                self.cortar(otro);
                false
            }
        }
    }

    /// **La grafica se perdio**: un dispositivo nuevo (en la grafica de la
    /// fuente, como al empezar), lo de la GPU del tubo otra vez encima de el
    /// sin cerrar el MP4, y la captura otra vez. Como OBS
    /// (`gs_device::RebuildDevice` y `winrt_capture_device_loss_rebuild`).
    fn recuperar(&mut self, motivo: &ErrorMotor) -> Result<(), ErrorMotor> {
        let ahora = com::ahora();
        if !self.presupuesto.intentar(ahora) {
            return Err(ErrorMotor::SinCodificador(format!(
                "la grafica se perdio demasiadas veces: {motivo}"
            )));
        }
        tracing::warn!(
            %motivo,
            razon = ?grafica::perdido(self.disp.d3d()),
            vuelta = self.presupuesto.usadas(),
            "se perdio la grafica; se vuelve a montar sin cerrar el video"
        );
        self.captura = None;
        let nuevo = grafica::dispositivo_para(self.fuente)?;
        proteger(&nuevo);
        let Some(tubo) = self.tubo.as_mut() else {
            return Err(ErrorMotor::HiloCaido);
        };
        if tubo.camino() == Camino::Grafica {
            // El codificador de la grafica sigue siendo el de la de antes:
            // solo vale si el dispositivo nuevo es la misma grafica.
            let ids = |d: &Dispositivo| {
                grafica::identidad(d.d3d()).map(|i| (i.fabricante, i.dispositivo))
            };
            if ids(&nuevo) != ids(&self.disp) {
                return Err(ErrorMotor::SinCodificador(
                    "la grafica volvio, pero es otra: su codificador no sirve".into(),
                ));
            }
        }
        tubo.recrear(&nuevo)?;
        self.disp = nuevo;
        match self.abrir_captura(self.fuente) {
            Ok(c) => self.captura = Some(c),
            // Sin captura el video sigue con la ultima imagen y se
            // reintenta cada tres segundos (`reabrir`).
            Err(e) => tracing::warn!(?e, "la captura no volvio con la grafica"),
        }
        self.nucleo.tras_una_vuelta();
        self.vueltas += 1;
        self.contadores.vuelta();
        tracing::info!(vueltas = self.vueltas, "la grafica volvio y el video sigue");
        Ok(())
    }

    /// Lo que ocupa el fichero, el ritmo y si la grafica sigue ahi, una vez
    /// por segundo. Devuelve si se sigue grabando.
    fn recontar(&mut self) -> bool {
        let ahora = com::ahora();
        if ahora < self.proximo_recuento {
            return true;
        }
        self.proximo_recuento = ahora + UNIDADES_POR_SEGUNDO;
        if let Some(t) = &self.tubo {
            self.contadores.bytes(archivo::bytes(t.parcial()));
        }
        self.nucleo.recontar(ahora, &self.contadores);
        // Una grafica perdida no siempre da error: la captura deja de dar
        // fotogramas y el repetido se copia en una GPU que ya no esta.
        if let Some(codigo) = grafica::perdido(self.disp.d3d()) {
            let e = ErrorMotor::Sistema {
                paso: "mirar si la grafica sigue ahi",
                detalle: format!("GetDeviceRemovedReason = 0x{:08X}", codigo as u32),
                codigo,
            };
            return self.tratar(e);
        }
        true
    }

    /// El video se corta a mitad: se guarda lo que habia.
    fn cortar(&mut self, e: ErrorMotor) {
        tracing::warn!(?e, "el video se corto; se guarda lo grabado");
        self.cortado = Some(e.to_string());
        self.contadores.cortar();
    }

    /// **Cierra el MP4**: para la captura, pone el ultimo fotograma donde
    /// acaba el sonido (para que las dos pistas duren lo mismo), escribe el
    /// indice y lo pone en su nombre.
    fn cerrar(mut self) -> Result<Resumen, ErrorMotor> {
        self.captura = None;
        let ahora = com::ahora();
        let mut tubo = self.tubo.take().ok_or(ErrorMotor::HiloCaido)?;
        let fin = {
            let mut t = EnLaGrafica {
                tubo: &mut tubo,
                disp: &self.disp,
            };
            self.nucleo.cerrar(&mut t, ahora)
        };
        let (codificador, formato, medidas) = (tubo.codificador(), tubo.formato(), tubo.medidas());
        let fotogramas = tubo.escritos();
        let cerrado = tubo.terminar()?;
        if let Some(f) = cerrado.fallo {
            self.cortado
                .get_or_insert_with(|| format!("el cierre del MP4 fallo: {f}"));
        }
        let bytes = archivo::bytes(&cerrado.ruta);
        let caidos = self.nucleo.caidos();
        tracing::info!(
            fotogramas,
            caidos,
            bytes,
            vueltas = self.vueltas,
            segundos = fin / UNIDADES_POR_SEGUNDO,
            "video cerrado"
        );
        Ok(Resumen {
            ruta: cerrado.ruta,
            duracion_ms: (fin.max(0) / 10_000) as u64,
            fotogramas,
            caidos,
            bytes,
            codificador,
            formato,
            medidas,
            fragmentado: cerrado.fragmentado,
            cortado: self.cortado.take(),
            vueltas: self.vueltas,
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

/// **Recorre la cadena de caidas** hasta que un camino arranca. Los de la
/// grafica van con el tope de su codificador (`peculiaridades`) y, si no
/// arrancan a mas de 1080p por algo que no sea que no hay codificador, se
/// reintentan mas pequenos antes de bajar de camino.
#[allow(clippy::too_many_arguments)]
fn elegir(
    disp: &Dispositivo,
    mut conv: Option<Conversor>,
    caps: Capacidades,
    pec: Peculiaridades,
    opciones: &Opciones,
    destino: &std::path::Path,
    medidas: (u32, u32),
    ajustes: Ajustes,
    fuente: (u32, u32),
) -> Result<Tubo, ErrorMotor> {
    let mut motivos = Vec::new();
    let minimo = crate::ritmo::minimo_entre(ajustes.por_segundo);
    let pedido = |formato, medidas| Pedido {
        destino,
        formato,
        medidas,
        ajustes,
        audio: opciones.audio,
        fragmentado: opciones.fragmentado,
        minimo,
    };
    let con_tope = match pec.tope {
        Some(t) => geometria::medida_de_salida(medidas.0, medidas.1, Some(t)),
        None => medidas,
    };
    for (camino, formato) in cadena::intentos(caps, opciones.hevc) {
        let hecho = match camino {
            Camino::Grafica | Camino::GraficaEnMemoria | Camino::Software => {
                let Some(mut c) = conv.take() else {
                    continue;
                };
                let mut tam = if camino == Camino::Software {
                    medidas
                } else {
                    con_tope
                };
                loop {
                    if let Err(e) = c.con_salida(tam) {
                        conv = Some(c);
                        break Err(e);
                    }
                    let p = pedido(formato, tam);
                    let abierto = match camino {
                        Camino::Grafica => Grafica::abrir(disp, c, &p).map(Tubo::Grafica),
                        Camino::GraficaEnMemoria => {
                            Software::abrir(disp, c, &p, true).map(Tubo::Software)
                        }
                        _ => Software::abrir(disp, c, &p, false).map(Tubo::Software),
                    };
                    match abierto {
                        Ok(t) => break Ok(t),
                        Err(devuelto) => {
                            let (cc, e) = *devuelto;
                            c = cc;
                            let menor = cadena::reintento_mas_pequeno(tam);
                            let reintentar = camino != Camino::Software
                                && !matches!(e, ErrorMotor::SinCodificador(_));
                            match menor {
                                Some(m) if reintentar => {
                                    tracing::info!(
                                        camino = camino.clave(),
                                        ?tam,
                                        ?m,
                                        ?e,
                                        "la grafica no arranco a ese tamano; se reintenta mas pequeno"
                                    );
                                    tam = m;
                                }
                                _ => {
                                    conv = Some(c);
                                    break Err(e);
                                }
                            }
                        }
                    }
                }
            }
            Camino::Cpu => {
                let a = cpu_ajustes(ajustes);
                Cpu::abrir(&Pedido {
                    medidas: Perfil::Ligero.medidas(fuente.0, fuente.1),
                    ajustes: a,
                    minimo: crate::ritmo::minimo_entre(a.por_segundo),
                    ..pedido(formato, medidas)
                })
                .map(Tubo::Cpu)
            }
        };
        match hecho {
            Ok(t) => return Ok(t),
            Err(e) => {
                tracing::warn!(
                    camino = camino.clave(),
                    ?formato,
                    ?e,
                    "un camino no arranco; se prueba el siguiente"
                );
                motivos.push(format!("{}: {e}", camino.clave()));
            }
        }
    }
    Err(ErrorMotor::SinCodificador(motivos.join("; ")))
}
