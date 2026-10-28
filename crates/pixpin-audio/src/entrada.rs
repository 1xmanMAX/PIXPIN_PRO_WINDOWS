//! Grabar una nota de voz (o una clase, o una reunion): la cara de siempre
//! sobre el motor de audio nuevo (`pixpin-motor-audio`).
//!
//! ## Que hace cada uno
//!
//! El **motor** captura (WASAPI, un hilo por fuente con prioridad de
//! audio), sigue el reloj de cada tarjeta para que el microfono y el sonido
//! del equipo no se separen en una hora, rellena los silencios del equipo,
//! sobrevive a que se desenchufe un aparato, realza la voz, mezcla, limita y
//! escribe el `.m4a` (ver `docs/motor-de-audio.md`).
//!
//! Esta [`Grabadora`] conserva lo que ya usaban las notas de voz, el
//! microfono flotante y la grabadora de clases, para que ninguno cambie:
//!
//! - El **`.parte`**: el motor escribe en `<destino>.parte` y solo al parar
//!   bien se renombra al destino. Un corte a mitad no deja un `.m4a`
//!   ilegible con nombre bueno, ni trunca lo que hubiera en esa ruta.
//! - Los **picos** de la onda (`crate::picos`), el nivel para pintar, el
//!   cronometro en muestras y el minimo de 700 ms.
//! - La **copia** del sonido para el video de una reunion ([`Copia`]).
//!
//! Lo de los picos y la copia lo hace un hilo pequeno suscrito al PCM del
//! motor: nada de eso puede retrasar la captura.
//!
//! - El **respaldo** (28-oct-2026, «si se perdiera algo … se perderia una
//!   reunion»), con [`Grabadora::empezar_a_prueba_de_cortes`]: el mismo PCM
//!   a un WAV al lado (`pixpin_motor_audio::respaldo`), que vale aunque la
//!   aplicacion muera o se vaya la luz, y del que [`recuperar`] rehace la
//!   nota. Con respaldo, **nada se borra sin que lo pida el usuario**: ni al
//!   soltar la grabadora sin pararla ni cuando el cierre falla.
//!
//! El formato del fichero es el de antes: AAC-LC **mono** en MPEG-4, a
//! 48 000 Hz y 96 kbps. Lo leen igual el movil, el reproductor del chat y
//! `pixpin-voz` para pasarlo a texto (que lo descodifica a 16 kHz mono con
//! `IMFSourceReader`, sin mirar con que tasa se escribio).

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;

use pixpin_motor_audio::respaldo::{self, Respaldo};
use pixpin_motor_audio::{
    BloquePcm, Equipo, ErrorMotor, Estadisticas, Microfono, MotorAudio, Niveles, Opciones,
    Receptor, Salida, Suscripcion, suscripcion,
};

use crate::ErrorAudio;
use crate::picos::{MINIMO_MS, PICO_MAXIMO, Reparto};

/// Muestras por segundo de toda nota: la del motor, que remuestrea cada
/// tarjeta a esta.
pub const MUESTREO: u32 = 48_000;

/// Bloques en cola hacia el hilo de los picos (unos 30-45 s): no se pierde
/// nada aunque la maquina se atasque un rato.
const COLA_DE_PICOS: usize = 3_000;

/// **De donde se graba** (24-oct-2026, grabar clases y reuniones).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fuentes {
    /// El microfono por defecto de Windows.
    pub microfono: bool,
    /// Lo que suena en el equipo (los altavoces o cascos por defecto): la
    /// otra parte de una reunion de Meet o Zoom, el video de una clase.
    pub equipo: bool,
}

impl Fuentes {
    /// Una nota de voz: solo el microfono.
    pub const MICROFONO: Fuentes = Fuentes {
        microfono: true,
        equipo: false,
    };
    /// Una reunion: lo que se dice y lo que se oye.
    pub const LAS_DOS: Fuentes = Fuentes {
        microfono: true,
        equipo: true,
    };
}

/// Lo que queda de una grabacion: justo lo que hay que escribir en el
/// mensaje de voz del cuaderno.
#[derive(Debug, Clone)]
pub struct Grabacion {
    pub ruta: PathBuf,
    /// Para `Mensaje.duracion_ms`. Entero, porque en Kotlin el campo es
    /// `Int` y un valor fuera de su rango hace que kotlinx no lea la linea.
    pub duracion_ms: i64,
    /// Para `resto["picos"]`: enteros crudos de 0 a 32767, como mucho 256.
    pub picos: Vec<i32>,
}

/// **El fichero a medio escribir, mientras se graba.**
///
/// Media Foundation crea y **trunca** el fichero al abrirlo, y solo escribe
/// el indice del MPEG-4 en `Finalize`: grabando directamente sobre el destino
/// definitivo, un corte a mitad —la aplicacion muere, se va la luz— dejaba un
/// `.m4a` ILEGIBLE con nombre bueno, y si en esa ruta habia algo se truncaba
/// nada mas pulsar grabar. Es lo mismo que `pixpin_sincro::envio` resuelve con
/// su `.parte`.
fn a_medias(destino: &Path) -> PathBuf {
    let mut s = destino.as_os_str().to_os_string();
    s.push(".parte");
    PathBuf::from(s)
}

/// **Una copia del sonido ya mezclado** (25-oct-2026, grabar la reunion en
/// video): cada bloque mono de 16 bits, al muestreo de la grabacion
/// ([`Grabadora::muestreo`]). En pausa no llega nada, asi que contar lo
/// recibido es el mismo reloj que [`Grabadora::llevado_ms`]. Cuando la
/// grabacion termina —o se tira— el emisor se suelta y el canal se cierra:
/// ese cierre es la senal de fin para quien escucha.
///
/// Para algo mas que esto (bloques con su posicion, estereo, saber si se
/// perdio algo) esta [`Grabadora::empezar_con_pcm`].
pub type Copia = Sender<Vec<i16>>;

/// Lo que el motor dice que paso, traducido a lo que ya sabian decir las
/// ventanas.
fn de_motor(e: ErrorMotor) -> ErrorAudio {
    match e {
        // Sin ninguna fuente pedida es lo mismo que antes: no hay microfono.
        ErrorMotor::SinFuentes | ErrorMotor::SinMicrofono => ErrorAudio::SinMicrofono,
        ErrorMotor::SinSonidoDelEquipo => ErrorAudio::SinSonidoDelEquipo,
        ErrorMotor::SinCodificadorAac => ErrorAudio::SinCodificadorAac,
        ErrorMotor::FormatoDesconocido { bits, .. } => ErrorAudio::FormatoDesconocido { bits },
        ErrorMotor::SalidaNoValida { .. } => ErrorAudio::FormatoDesconocido { bits: 16 },
        ErrorMotor::Fichero { ruta } => ErrorAudio::NoEsAudio { ruta },
        ErrorMotor::HiloCaido => ErrorAudio::HiloCaido,
        ErrorMotor::Windows { paso, codigo } => ErrorAudio::Windows {
            paso,
            fuente: windows::core::Error::from_hresult(windows::core::HRESULT(codigo)),
        },
    }
}

/// Una grabacion en marcha.
pub struct Grabadora {
    /// Donde acabara la nota **si sale bien**. Hasta el renombrado no existe.
    destino: PathBuf,
    /// Donde se esta escribiendo de verdad: `<destino>.parte`.
    parcial: PathBuf,
    /// En caja: el motor lleva sus hilos, colas y cuentas (cerca de 300
    /// bytes), y la grabadora viaja por canales y vive dentro de los
    /// estados de varias ventanas, que no tienen por que crecer con el.
    motor: Option<Box<MotorAudio>>,
    /// El hilo de los picos, de la copia y del respaldo; devuelve los picos
    /// al cerrarse el PCM.
    oyente: Option<JoinHandle<Vec<i32>>>,
    /// El respaldo WAV, si se pidio ([`Grabadora::empezar_a_prueba_de_cortes`]).
    respaldo: Option<PathBuf>,
}

impl Grabadora {
    /// Abre el microfono y empieza a escribir en `destino`.
    ///
    /// Vuelve cuando el microfono ya esta cogido de verdad, no antes: que no
    /// se pueda grabar es un caso **normal** —otra aplicacion lo tiene, el
    /// permiso de microfono esta quitado en los ajustes de Windows— y quien
    /// llama tiene que poder decirselo al usuario en el acto en vez de
    /// ensenar un boton rojo que no graba nada.
    pub fn empezar(destino: &Path) -> Result<Grabadora, ErrorAudio> {
        Grabadora::empezar_con(destino, Fuentes::MICROFONO)
    }

    /// Lo mismo, de las `fuentes` dadas: el microfono, lo que suena en el
    /// equipo o las dos juntas en una sola nota.
    pub fn empezar_con(destino: &Path, fuentes: Fuentes) -> Result<Grabadora, ErrorAudio> {
        Grabadora::empezar_con_copia(destino, fuentes, None)
    }

    /// Lo mismo, y ademas manda por `copia` cada bloque del sonido mezclado
    /// (ver [`Copia`]): lo que necesita el video de una reunion para llevar
    /// el mismo sonido que la nota y al mismo compas.
    pub fn empezar_con_copia(
        destino: &Path,
        fuentes: Fuentes,
        copia: Option<Copia>,
    ) -> Result<Grabadora, ErrorAudio> {
        Grabadora::empezar_del_todo(destino, fuentes, copia, Vec::new(), false)
    }

    /// **Lo mismo, a prueba de cortes**: ademas del `.m4a`, el mismo sonido
    /// a `<destino>.respaldo.wav` (ver `pixpin_motor_audio::respaldo`). Si la
    /// aplicacion muere, se va la luz o el `.m4a` no se puede cerrar, la nota
    /// se rehace del respaldo ([`recuperar`]). Es lo que usan las clases y
    /// reuniones: horas que no se pueden perder.
    pub fn empezar_a_prueba_de_cortes(
        destino: &Path,
        fuentes: Fuentes,
        copia: Option<Copia>,
    ) -> Result<Grabadora, ErrorAudio> {
        Grabadora::empezar_del_todo(destino, fuentes, copia, Vec::new(), true)
    }

    /// Lo mismo, con suscripciones al PCM del motor que reciben **desde el
    /// primer marco**: bloques mono de 16 bits a [`MUESTREO`], cada uno con
    /// su posicion, y la cuenta de lo que se le perdio a cada una (ver
    /// `pixpin_motor_audio::pcm`). Es la entrada pensada para el video.
    pub fn empezar_con_pcm(
        destino: &Path,
        fuentes: Fuentes,
        pcm: Vec<Suscripcion>,
    ) -> Result<Grabadora, ErrorAudio> {
        Grabadora::empezar_del_todo(destino, fuentes, None, pcm, false)
    }

    fn empezar_del_todo(
        destino: &Path,
        fuentes: Fuentes,
        copia: Option<Copia>,
        mut pcm: Vec<Suscripcion>,
        con_respaldo: bool,
    ) -> Result<Grabadora, ErrorAudio> {
        if !fuentes.microfono && !fuentes.equipo {
            return Err(ErrorAudio::SinMicrofono);
        }
        if let Some(padre) = destino.parent()
            && !padre.as_os_str().is_empty()
        {
            std::fs::create_dir_all(padre).map_err(|_| ErrorAudio::NoEsAudio {
                ruta: padre.display().to_string(),
            })?;
        }
        let parcial = a_medias(destino);
        let (picos, receptor) = suscripcion(COLA_DE_PICOS);
        pcm.push(picos);
        let opciones = Opciones {
            microfono: fuentes.microfono.then_some(Microfono::Defecto),
            equipo: fuentes.equipo.then_some(Equipo::Todo),
            salida: Salida {
                m4a: Some(parcial.clone()),
                canales: 1,
                muestreo: MUESTREO,
            },
            // El realce y el paso alto de siempre, solo en el microfono.
            realce_voz: true,
            paso_alto: true,
            pcm,
            ..Opciones::default()
        };
        // El respaldo se abre antes que el motor: el primer bloque ya cae en
        // el. Si no se puede (sin permiso, disco lleno), se graba sin el.
        let respaldo = if con_respaldo {
            let ruta = respaldo::ruta_del_respaldo(destino);
            match Respaldo::crear(&ruta, MUESTREO, 1) {
                Ok(r) => Some(r),
                Err(e) => {
                    tracing::warn!(?e, ruta = %ruta.display(), "la grabacion va sin respaldo");
                    None
                }
            }
        } else {
            None
        };
        let ruta_respaldo = respaldo.as_ref().map(|r| r.ruta().to_path_buf());
        let quitar_respaldo = || {
            if let Some(r) = &ruta_respaldo {
                let _ = std::fs::remove_file(r);
            }
        };
        let motor = match MotorAudio::empezar(opciones) {
            Ok(m) => m,
            Err(e) => {
                // Si fallo despues de crear el `.m4a`, que no se quede
                // huerfano. Es el `.parte`, nunca el destino del usuario.
                let _ = std::fs::remove_file(&parcial);
                drop(respaldo);
                quitar_respaldo();
                return Err(de_motor(e));
            }
        };
        let oyente = std::thread::Builder::new()
            .name("pixpin-grabar".into())
            .spawn(move || escuchar(&receptor, copia, respaldo))
            .map_err(|_| ErrorAudio::HiloCaido);
        let oyente = match oyente {
            Ok(h) => h,
            Err(e) => {
                drop(motor);
                let _ = std::fs::remove_file(&parcial);
                quitar_respaldo();
                return Err(e);
            }
        };
        Ok(Grabadora {
            destino: destino.to_path_buf(),
            parcial,
            motor: Some(Box::new(motor)),
            oyente: Some(oyente),
            respaldo: ruta_respaldo,
        })
    }

    /// **Pausa** (`true`) o sigue (`false`). En pausa lo que suena se tira y
    /// el reloj de la nota no corre: al seguir, la nota sigue donde iba.
    pub fn pausar(&self, si: bool) {
        if let Some(m) = &self.motor {
            if si { m.pausar() } else { m.seguir() }
        }
    }

    /// Si esta en pausa.
    pub fn en_pausa(&self) -> bool {
        self.motor.as_ref().is_some_and(|m| m.en_pausa())
    }

    /// El pico de lo ultimo que se mezclo, de 0 a 32767: para pintar la onda
    /// en vivo mientras se graba.
    pub fn nivel(&self) -> i32 {
        self.motor.as_ref().map_or(0, |m| {
            (m.niveles().mezcla.pico.clamp(0.0, 1.0) * PICO_MAXIMO as f32).round() as i32
        })
    }

    /// Los niveles de cada fuente y de la mezcla (lineales, de 0 a 1).
    pub fn niveles(&self) -> Niveles {
        self.motor.as_ref().map(|m| m.niveles()).unwrap_or_default()
    }

    /// Las cuentas del motor: lo capturado, el silencio puesto, la deriva
    /// de cada tarjeta, lo perdido.
    pub fn estadisticas(&self) -> Estadisticas {
        self.motor
            .as_ref()
            .map(|m| m.estadisticas())
            .unwrap_or_default()
    }

    /// Una suscripcion al PCM desde ahora (ver [`Grabadora::empezar_con_pcm`]
    /// para recibirlo desde el principio).
    pub fn suscribir_pcm(&self, capacidad: usize) -> Option<Receptor> {
        self.motor.as_ref().map(|m| m.suscribir_pcm(capacidad))
    }

    /// Muestras por segundo de la grabacion: las de la [`Copia`].
    pub fn muestreo(&self) -> u32 {
        MUESTREO
    }

    /// Cuanto lleva grabado, en milisegundos.
    ///
    /// Se cuenta en **muestras**, no con un reloj: asi el cronometro de la
    /// pantalla dice exactamente lo mismo que acabara diciendo
    /// `duracionMs`.
    pub fn llevado_ms(&self) -> i64 {
        self.motor.as_ref().map_or(0, |m| m.llevado_ms())
    }

    /// Para, cierra el `.m4a` y devuelve lo que hay que guardar.
    ///
    /// Una nota mas corta que [`MINIMO_MS`] se rechaza y **se borra el
    /// fichero**, igual que en el movil, donde `Voz.parar` devuelve `false`
    /// y quien llama borra (`GrabadoraActivity.kt:134-151`): por debajo de
    /// eso lo que hubo fue un resbalon en el boton, no una nota.
    ///
    /// **Con respaldo** ([`Grabadora::empezar_a_prueba_de_cortes`]), si el
    /// `.m4a` no se pudo cerrar (el disco, el codificador) la nota se rehace
    /// del respaldo antes de rendirse; y si ni eso, el `.parte` y el respaldo
    /// **se quedan** para [`recuperar`] mas tarde.
    pub fn parar(mut self) -> Result<Grabacion, ErrorAudio> {
        let respaldo = self.respaldo.take();
        let terminado = self.terminar();
        let Some(respaldo) = respaldo else {
            return self.parar_sin_respaldo(terminado);
        };
        let picos = match &terminado {
            Ok(g) if g.duracion_ms < MINIMO_MS => {
                // Un resbalon en el boton: ni nota ni respaldo.
                let _ = std::fs::remove_file(&self.parcial);
                let _ = std::fs::remove_file(&respaldo);
                return Err(ErrorAudio::DemasiadoCorta {
                    duro_ms: g.duracion_ms,
                    minimo_ms: MINIMO_MS,
                });
            }
            Ok(g) => {
                if std::fs::rename(&self.parcial, &self.destino).is_ok() {
                    let _ = std::fs::remove_file(&respaldo);
                    return terminado;
                }
                g.picos.clone()
            }
            Err(e) => {
                tracing::warn!(?e, "el .m4a no se cerro; se rehace del respaldo");
                Vec::new()
            }
        };
        match recuperar(&self.destino) {
            Ok(mut g) => {
                if !picos.is_empty() {
                    g.picos = picos;
                }
                Ok(g)
            }
            Err(e) => {
                tracing::warn!(
                    ?e,
                    respaldo = %respaldo.display(),
                    "la nota no se pudo rehacer; el respaldo se queda para recuperarla"
                );
                terminado.and(Err(e))
            }
        }
    }

    fn parar_sin_respaldo(
        &mut self,
        terminado: Result<Grabacion, ErrorAudio>,
    ) -> Result<Grabacion, ErrorAudio> {
        let grabacion = match terminado {
            Ok(g) => g,
            Err(e) => {
                // El `.m4a` a medias no vale para nada y nadie lo va a
                // encontrar por su nombre: se barre aqui.
                let _ = std::fs::remove_file(&self.parcial);
                return Err(e);
            }
        };
        if grabacion.duracion_ms < MINIMO_MS {
            let _ = std::fs::remove_file(&self.parcial);
            return Err(ErrorAudio::DemasiadoCorta {
                duro_ms: grabacion.duracion_ms,
                minimo_ms: MINIMO_MS,
            });
        }
        // **El renombrado es lo que publica la nota.** Hasta aqui el destino
        // no existe, asi que no hay ningun momento en el que haya un `.m4a`
        // con nombre bueno y contenido ilegible.
        std::fs::rename(&self.parcial, &self.destino).map_err(|_| ErrorAudio::NoEsAudio {
            ruta: self.destino.display().to_string(),
        })?;
        Ok(grabacion)
    }

    /// Para y tira lo grabado: el usuario se arrepintio.
    ///
    /// Se borra el `.parte`, no el destino: si en esa ruta habia algo del
    /// usuario, cancelar una nota no se lo puede llevar por delante.
    pub fn cancelar(mut self) {
        let _ = self.terminar();
        let _ = std::fs::remove_file(&self.parcial);
        // Tirar es lo unico que se lleva el respaldo: lo pidio el usuario.
        if let Some(r) = self.respaldo.take() {
            let _ = std::fs::remove_file(r);
        }
    }

    /// El respaldo WAV, si se graba con el.
    pub fn respaldo(&self) -> Option<&Path> {
        self.respaldo.as_deref()
    }

    fn terminar(&mut self) -> Result<Grabacion, ErrorAudio> {
        let motor = self.motor.take().ok_or(ErrorAudio::YaParada)?;
        let resumen = motor.terminar().map_err(de_motor);
        // El motor ya cerro el PCM: el oyente acaba en cuanto lo vacia.
        let picos = match self.oyente.take() {
            Some(h) => h.join().map_err(|_| ErrorAudio::HiloCaido)?,
            None => Vec::new(),
        };
        let resumen = resumen?;
        Ok(Grabacion {
            ruta: self.destino.clone(),
            duracion_ms: resumen.duracion_ms,
            picos,
        })
    }
}

impl Drop for Grabadora {
    fn drop(&mut self) {
        // Soltar la grabadora sin pararla dejaria el microfono cogido y un
        // hilo girando hasta que se cierre la aplicacion.
        if self.motor.is_some() {
            let _ = self.terminar();
            if self.respaldo.is_some() {
                // A prueba de cortes: soltarla sin pararla (un panico, la
                // ventana que se cierra mal) no es tirarla. El `.parte` ya
                // cerrado y el respaldo se quedan para `recuperar`.
                tracing::warn!(
                    parte = %self.parcial.display(),
                    "grabadora soltada sin parar; queda para recuperarla"
                );
                return;
            }
            // El `.parte`, nunca el destino: este `Drop` corria sobre la ruta
            // que dio quien llama, asi que soltar una grabadora sin pararla
            // borraba un fichero del usuario si ya habia uno ahi.
            let _ = std::fs::remove_file(&self.parcial);
        }
    }
}

/// El hilo de los picos, de la copia y del respaldo: hasta que el motor
/// cierre el PCM.
fn escuchar(
    receptor: &Receptor,
    mut copia: Option<Copia>,
    mut respaldo: Option<Respaldo>,
) -> Vec<i32> {
    let mut reparto = Reparto::nuevo(MUESTREO);
    let mut fin = 0u64;
    let mut ceros: Vec<i16> = Vec::new();
    let al_respaldo = |r: &mut Option<Respaldo>, m: &[i16]| {
        if let Some(x) = r.as_mut()
            && let Err(e) = x.escribir(m)
        {
            // El disco se lleno o se fue: la nota sigue, sin respaldo.
            tracing::warn!(?e, "el respaldo de la grabacion dejo de escribirse");
        }
    };
    while let Some(bloque) = receptor.recibir() {
        // Si se perdio algo (no deberia, con una cola tan larga), el hueco es
        // silencio: la onda y la copia siguen al compas de la nota.
        if bloque.posicion > fin {
            ceros.clear();
            ceros.resize((bloque.posicion - fin) as usize, 0);
            reparto.empuja(&ceros);
            al_respaldo(&mut respaldo, &ceros);
            enviar(&mut copia, ceros.clone());
        }
        fin = bloque.fin();
        reparto.empuja(&bloque.muestras);
        al_respaldo(&mut respaldo, &bloque.muestras);
        if copia.is_some() {
            let BloquePcm { muestras, .. } = bloque;
            enviar(&mut copia, muestras);
        } else {
            receptor.devolver(bloque);
        }
    }
    if let Some(r) = respaldo
        && let Err(e) = r.cerrar()
    {
        tracing::warn!(?e, "el respaldo de la grabacion no se cerro (se lee igual)");
    }
    reparto.terminar()
}

/// **Rehace una nota cortada** que se grababa en `destino` con respaldo
/// ([`Grabadora::empezar_a_prueba_de_cortes`]): la aplicacion murio, se fue
/// la luz o el `.m4a` no se pudo cerrar.
///
/// - Si el `.parte` llego a cerrarse (tiene su indice), se pone en `destino`
///   tal cual.
/// - Si no, se vuelve a codificar el respaldo WAV a `destino`.
///
/// Los picos salen del respaldo. Si sale bien, el `.parte` y el respaldo se
/// borran (ya estan en `destino`); si no, **no se toca nada**.
pub fn recuperar(destino: &Path) -> Result<Grabacion, ErrorAudio> {
    use pixpin_sistema::mp4::{EstadoMp4, mirar};
    let parcial = a_medias(destino);
    let ruta_respaldo = respaldo::ruta_del_respaldo(destino);
    let wav = respaldo::leer(&ruta_respaldo).ok();
    let picos = wav
        .map(|w| picos_del_respaldo(&ruta_respaldo, &w))
        .unwrap_or_default();
    let duracion_ms =
        if mirar(&parcial) == EstadoMp4::Cerrado && std::fs::rename(&parcial, destino).is_ok() {
            // Sin respaldo no se sabe lo que dura sin descodificarlo: lo dice
            // quien lo guarda (el diario de la grabacion) o se queda en cero.
            wav.map_or(0, |w| w.duracion_ms())
        } else {
            let w = wav.ok_or_else(|| ErrorAudio::NoEsAudio {
                ruta: ruta_respaldo.display().to_string(),
            })?;
            if w.duracion_ms() < MINIMO_MS {
                return Err(ErrorAudio::DemasiadoCorta {
                    duro_ms: w.duracion_ms(),
                    minimo_ms: MINIMO_MS,
                });
            }
            // A un `.parte` propio: el destino solo aparece entero.
            let mut rehecho = destino.as_os_str().to_os_string();
            rehecho.push(".rehecho.parte");
            let rehecho = PathBuf::from(rehecho);
            if let Err(e) = pixpin_motor_audio::rehacer_m4a(&ruta_respaldo, &rehecho) {
                let _ = std::fs::remove_file(&rehecho);
                return Err(de_motor(e));
            }
            std::fs::rename(&rehecho, destino).map_err(|_| ErrorAudio::NoEsAudio {
                ruta: destino.display().to_string(),
            })?;
            let _ = std::fs::remove_file(&parcial);
            w.duracion_ms()
        };
    let _ = std::fs::remove_file(&ruta_respaldo);
    Ok(Grabacion {
        ruta: destino.to_path_buf(),
        duracion_ms,
        picos,
    })
}

/// Si queda algo de una grabacion con respaldo en `destino` que
/// [`recuperar`] pueda rehacer.
pub fn hay_algo_que_recuperar(destino: &Path) -> bool {
    let wav = respaldo::leer(&respaldo::ruta_del_respaldo(destino));
    wav.is_ok_and(|w| w.marcos > 0)
        || pixpin_sistema::mp4::mirar(&a_medias(destino)) == pixpin_sistema::mp4::EstadoMp4::Cerrado
}

/// Los picos de la onda, de un respaldo entero (como si se grabara otra
/// vez, pero sin sonido: solo cuentas).
fn picos_del_respaldo(ruta: &Path, w: &respaldo::Wav) -> Vec<i32> {
    let mut reparto = Reparto::nuevo(w.muestreo);
    let canales = w.canales.max(1) as usize;
    let mut mono: Vec<i16> = Vec::new();
    let hecho = respaldo::recorrer::<std::io::Error>(ruta, w, w.muestreo as usize, |m| {
        if canales == 1 {
            reparto.empuja(m);
        } else {
            mono.clear();
            mono.extend(m.chunks_exact(canales).map(|c| c[0]));
            reparto.empuja(&mono);
        }
        Ok(())
    });
    if let Err(e) = hecho {
        tracing::info!(?e, "los picos del respaldo salen hasta donde se leyo");
    }
    reparto.terminar()
}

/// Manda un trozo por la copia. Si quien escuchaba ya no esta (su video
/// fallo), se deja de copiar: la nota sigue igual.
fn enviar(copia: &mut Option<Copia>, muestras: Vec<i16>) {
    if let Some(c) = copia
        && c.send(muestras).is_err()
    {
        *copia = None;
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Necesita **microfono de verdad**. Se ejecuta a mano con:
    /// `cargo test -p pixpin-audio -- --ignored --nocapture`
    #[test]
    #[ignore = "necesita un microfono conectado y el permiso de Windows"]
    fn una_nota_grabada_de_verdad_sale_con_picos_y_duracion() {
        let ruta = std::env::temp_dir().join("pixpin-audio-prueba.m4a");
        let _ = std::fs::remove_file(&ruta);

        let g = Grabadora::empezar(&ruta).expect("no se pudo abrir el microfono");
        std::thread::sleep(std::time::Duration::from_millis(1_500));
        let grabacion = g.parar().expect("no se pudo cerrar la nota");

        assert!(grabacion.duracion_ms >= MINIMO_MS, "{grabacion:?}");
        assert!(!grabacion.picos.is_empty(), "no capturo ni un pico");
        assert!(grabacion.picos.len() <= crate::PICOS_GUARDADOS);
        assert!(
            grabacion
                .picos
                .iter()
                .all(|p| (0..=crate::PICO_MAXIMO).contains(p))
        );
        assert!(std::fs::metadata(&ruta).unwrap().len() > 0);
        let _ = std::fs::remove_file(&ruta);
    }

    /// Tambien necesita microfono: comprueba que una pulsacion de medio
    /// segundo se rechaza y **no deja fichero**.
    #[test]
    #[ignore = "necesita un microfono conectado y el permiso de Windows"]
    fn una_nota_de_medio_segundo_se_rechaza_y_no_deja_fichero() {
        let ruta = std::env::temp_dir().join("pixpin-audio-corta.m4a");
        let _ = std::fs::remove_file(&ruta);

        let g = Grabadora::empezar(&ruta).expect("no se pudo abrir el microfono");
        std::thread::sleep(std::time::Duration::from_millis(200));
        let e = g.parar().unwrap_err();

        assert!(
            matches!(e, ErrorAudio::DemasiadoCorta { .. }),
            "salio {e:?}"
        );
        assert!(!ruta.exists(), "se quedo un .m4a que no suena");
        assert!(!a_medias(&ruta).exists(), "se quedo el fichero a medias");
    }

    /// **La aplicacion muere a mitad de una reunion** (el proceso se aborta:
    /// ni `parar`, ni `Drop`, ni `Finalize`). La prueba se lanza a si misma
    /// como hijo, que graba tres segundos y se mata; el padre recupera la
    /// nota del respaldo. Necesita microfono:
    /// `cargo test -p pixpin-audio -- --ignored --nocapture cortada`
    #[test]
    #[ignore = "necesita un microfono conectado y el permiso de Windows"]
    fn una_grabacion_cortada_de_golpe_se_recupera_del_respaldo() {
        const HIJO: &str = "PIXPIN_PRUEBA_CORTE_HIJO";
        if let Some(destino) = std::env::var_os(HIJO) {
            let g = Grabadora::empezar_a_prueba_de_cortes(
                Path::new(&destino),
                Fuentes::MICROFONO,
                None,
            )
            .expect("no se pudo abrir el microfono");
            std::thread::sleep(std::time::Duration::from_millis(3_000));
            // Sin soltar nada: como un corte de luz para este proceso.
            let _ = &g;
            std::process::abort();
        }
        let ruta = std::env::temp_dir().join("pixpin-audio-corte.m4a.parcial");
        let limpiar = |r: &Path| {
            let _ = std::fs::remove_file(r);
            let _ = std::fs::remove_file(a_medias(r));
            let _ = std::fs::remove_file(respaldo::ruta_del_respaldo(r));
        };
        limpiar(&ruta);
        let estado = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "entrada::pruebas::una_grabacion_cortada_de_golpe_se_recupera_del_respaldo",
            ])
            .env(HIJO, &ruta)
            .status()
            .expect("lanzar el hijo");
        assert!(!estado.success(), "el hijo tenia que morir de golpe");
        // Lo que deja un corte: nada con el nombre bueno, un `.parte` sin
        // indice y el respaldo.
        assert!(!ruta.exists());
        assert!(!pixpin_sistema::mp4::mirar(&a_medias(&ruta)).se_abre());
        assert!(hay_algo_que_recuperar(&ruta));

        let g = recuperar(&ruta).expect("la nota tenia que rehacerse del respaldo");
        assert!(g.duracion_ms >= 2_000, "{g:?}");
        assert!(!g.picos.is_empty());
        assert_eq!(
            pixpin_sistema::mp4::mirar(&ruta),
            pixpin_sistema::mp4::EstadoMp4::Cerrado
        );
        // Y lo de la recuperacion ya no hace falta.
        assert!(!respaldo::ruta_del_respaldo(&ruta).exists());
        assert!(!a_medias(&ruta).exists());
        limpiar(&ruta);
    }

    /// Soltar la grabadora sin pararla (un panico, una ventana que se cierra
    /// mal) **no tira la nota**: el `.parte` se cierra y se queda.
    #[test]
    #[ignore = "necesita un microfono conectado y el permiso de Windows"]
    fn una_grabadora_soltada_sin_parar_se_recupera() {
        let ruta = std::env::temp_dir().join("pixpin-audio-soltada.m4a.parcial");
        let _ = std::fs::remove_file(&ruta);
        let g = Grabadora::empezar_a_prueba_de_cortes(&ruta, Fuentes::MICROFONO, None)
            .expect("no se pudo abrir el microfono");
        std::thread::sleep(std::time::Duration::from_millis(1_500));
        drop(g);
        assert!(!ruta.exists());
        assert_eq!(
            pixpin_sistema::mp4::mirar(&a_medias(&ruta)),
            pixpin_sistema::mp4::EstadoMp4::Cerrado
        );
        let g = recuperar(&ruta).expect("recuperar");
        assert!(g.duracion_ms >= 1_000, "{g:?}");
        assert!(ruta.is_file());
        let _ = std::fs::remove_file(&ruta);
    }

    #[test]
    fn el_fichero_a_medias_va_al_lado_del_destino_y_con_su_nombre_entero() {
        // Se anade el sufijo a la ruta ENTERA y no se cambia la extension:
        // con `with_extension("parte")` dos notas del mismo milisegundo en
        // formatos distintos chocarian, y sobre todo el `.parte` dejaria de
        // decir de quien es.
        let p = a_medias(Path::new("C:/n/voz_1700000000000.m4a"));
        assert_eq!(p, Path::new("C:/n/voz_1700000000000.m4a.parte"));
        assert_eq!(p.parent(), Path::new("C:/n/voz_1.m4a").parent());
        // Caso negativo: no puede coincidir con el destino, o no habria dos
        // ficheros y el renombrado no protegeria de nada.
        assert_ne!(p, PathBuf::from("C:/n/voz_1700000000000.m4a"));
    }

    #[test]
    fn un_microfono_que_no_abre_no_toca_el_fichero_que_ya_habia() {
        // **El fallo:** Media Foundation TRUNCA el destino al abrirlo, asi
        // que pulsar grabar en una ruta ocupada se llevaba por delante lo que
        // hubiera antes de saber siquiera si habia microfono. Ahora lo unico
        // que se abre es el `.parte`.
        //
        // No necesita microfono: si lo hay, se para en seguida; si no lo hay,
        // `empezar` falla. Las dos ramas prometen lo mismo.
        let ruta = std::env::temp_dir().join("pixpin-audio-no-truncar.m4a");
        std::fs::write(&ruta, b"esto es de otro").expect("preparar el fichero");

        if let Ok(g) = Grabadora::empezar(&ruta) {
            g.cancelar();
        }

        assert_eq!(
            std::fs::read(&ruta).expect("el fichero de antes"),
            b"esto es de otro",
            "abrir el microfono trunco un fichero que no era suyo"
        );
        assert!(
            !a_medias(&ruta).exists(),
            "el .parte tiene que barrerse al cancelar o al fallar"
        );
        let _ = std::fs::remove_file(&ruta);
    }
}
