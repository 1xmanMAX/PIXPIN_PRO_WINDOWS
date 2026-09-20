//! Sacar un audio por la tarjeta: `IMFMediaEngine` en modo solo-sonido.
//!
//! Es el hermano pequeno de `pixpin-pin/src/video.rs`: el mismo motor, la
//! misma fabrica, el mismo callback que corre en hilos de Media Foundation
//! y solo escribe atomicos. La diferencia es que aqui no hay gestor DXGI ni
//! textura: se crea con `MF_MEDIA_ENGINE_AUDIOONLY` y el motor se encarga de
//! abrir la tarjeta.
//!
//! **La carga es asincrona.** `abrir` vuelve enseguida, antes de que se
//! sepa cuanto dura. Quien llama pregunta por [`Salida::lista`] en su
//! propio latido; hasta entonces la duracion es cero, y por eso
//! `reloj::destino_de_fraccion` devuelve `None` con duracion cero en vez de
//! mandar a ninguna parte.

use std::path::Path;
use std::sync::Arc;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Media::MediaFoundation::{
    CLSID_MFMediaEngineClassFactory, IMFAttributes, IMFMediaEngine, IMFMediaEngineClassFactory,
    IMFMediaEngineNotify, IMFMediaEngineNotify_Impl, MF_MEDIA_ENGINE_AUDIOONLY,
    MF_MEDIA_ENGINE_CALLBACK, MF_MEDIA_ENGINE_EVENT, MF_MEDIA_ENGINE_EVENT_ENDED,
    MF_MEDIA_ENGINE_EVENT_ERROR, MF_MEDIA_ENGINE_EVENT_LOADEDMETADATA, MF_VERSION,
    MFCreateAttributes, MFSTARTUP_LITE, MFStartup,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::core::BSTR;
use windows::core::implement;

use crate::{ErrorAudio, en};

/// Media Foundation se arranca una vez por proceso y no se apaga: el
/// reproductor puede nacer y morir muchas veces en una sesion.
/// `MFStartup`/`MFShutdown` llevan su cuenta, asi que este arranque convive
/// con el del pin y el de `pixpin-record` sin pisarlos.
static ARRANQUE: Once = Once::new();

fn arrancar_media_foundation() {
    ARRANQUE.call_once(|| {
        // SAFETY: `MFStartup` no tiene mas precondicion que recibir la
        // version de la cabecera con la que se compilo, que es lo que vale
        // `MF_VERSION`. Un fallo aqui reaparece luego como error al crear
        // el motor, que si se sabe contar.
        unsafe {
            let _ = MFStartup(MF_VERSION, MFSTARTUP_LITE);
        }
    });
}

/// Lo que el callback le cuenta al hilo de la interfaz.
#[derive(Default)]
struct Avisos {
    metadatos: AtomicBool,
    error: AtomicBool,
    termino: AtomicBool,
}

/// El callback de Media Foundation. Corre en SUS hilos: solo atomicos, nada
/// de tocar ventanas ni de coger cerrojos que pueda tener el hilo de la
/// interfaz.
#[implement(IMFMediaEngineNotify)]
struct Aviso {
    avisos: Arc<Avisos>,
}

impl IMFMediaEngineNotify_Impl for Aviso_Impl {
    fn EventNotify(&self, evento: u32, _p1: usize, _p2: u32) -> windows::core::Result<()> {
        match MF_MEDIA_ENGINE_EVENT(evento as i32) {
            MF_MEDIA_ENGINE_EVENT_LOADEDMETADATA => {
                self.avisos.metadatos.store(true, Ordering::Release);
            }
            MF_MEDIA_ENGINE_EVENT_ERROR => self.avisos.error.store(true, Ordering::Release),
            MF_MEDIA_ENGINE_EVENT_ENDED => self.avisos.termino.store(true, Ordering::Release),
            _ => {}
        }
        Ok(())
    }
}

/// Un audio cargado en la tarjeta. Uno solo a la vez: quien lo guarda
/// (`apps/pixpin/src/audio.rs`) suelta el anterior antes de abrir otro, que
/// es lo que hace `Reproductor.cargar` en el movil (`Reproductor.kt:64`).
pub struct Salida {
    motor: IMFMediaEngine,
    avisos: Arc<Avisos>,
}

/// A mano y no derivado: un `IMFMediaEngine` no tiene nada que imprimir, y
/// lo unico util al depurar es en que punto de la carga va.
impl std::fmt::Debug for Salida {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Salida")
            .field("lista", &self.lista())
            .field("fallo", &self.fallo())
            .finish()
    }
}

impl Salida {
    /// Abre el fichero. **No empieza a sonar**: hay que llamar a [`tocar`].
    ///
    /// Que el fichero no sea un audio no se sabe aqui —la carga es
    /// asincrona— sino despues, por [`fallo`]. Lo unico que se comprueba en
    /// el acto es que exista, porque una ruta que apunta a un hueco es el
    /// caso corriente: el `.m4a` vino en la sincronizacion pero el fichero
    /// se quedo en el movil.
    ///
    /// [`tocar`]: Salida::tocar
    /// [`fallo`]: Salida::fallo
    pub fn abrir(ruta: &Path) -> Result<Salida, ErrorAudio> {
        if !ruta.is_file() {
            return Err(ErrorAudio::NoEsAudio {
                ruta: ruta.display().to_string(),
            });
        }
        arrancar_media_foundation();

        // SAFETY: COM se inicializa (o se reutiliza) en el hilo que
        // reproduce y no se libera: es el hilo de la interfaz, que vive lo
        // que vive la aplicacion. Todo lo que se crea aqui es propio y se
        // suelta en `Drop` con `Shutdown`.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

            let mut atributos: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut atributos, 1).map_err(en("crear los atributos del motor"))?;
            let atributos = atributos.ok_or_else(|| ErrorAudio::Windows {
                paso: "crear los atributos del motor",
                fuente: windows::core::Error::empty(),
            })?;

            let avisos = Arc::new(Avisos::default());
            let aviso: IMFMediaEngineNotify = Aviso {
                avisos: Arc::clone(&avisos),
            }
            .into();
            atributos
                .SetUnknown(&MF_MEDIA_ENGINE_CALLBACK, &aviso)
                .map_err(en("enganchar el callback"))?;

            let fabrica: IMFMediaEngineClassFactory =
                CoCreateInstance(&CLSID_MFMediaEngineClassFactory, None, CLSCTX_INPROC_SERVER)
                    .map_err(en("crear la fabrica del motor"))?;
            // `MF_MEDIA_ENGINE_AUDIOONLY`: sin esto el motor monta la mitad
            // de video —y pide un gestor DXGI— para un fichero que no la
            // tiene.
            let motor = fabrica
                .CreateInstance(MF_MEDIA_ENGINE_AUDIOONLY.0 as u32, &atributos)
                .map_err(en("crear el motor de sonido"))?;

            motor.SetAutoPlay(false).map_err(en("quitar el autoplay"))?;
            motor.SetLoop(false).map_err(en("quitar el bucle"))?;

            // Media Foundation admite una ruta de Windows tal cual como
            // origen; se pasa sin convertir a `file:///` para no tener que
            // escapar los espacios ni los acentos del nombre.
            let origen = BSTR::from(ruta.to_string_lossy().as_ref());
            motor.SetSource(&origen).map_err(en("abrir el fichero"))?;

            Ok(Salida { motor, avisos })
        }
    }

    /// Si Media Foundation no pudo con el fichero: no era un audio, o era
    /// uno con un codec que este Windows no trae.
    pub fn fallo(&self) -> bool {
        self.avisos.error.load(Ordering::Acquire)
    }

    /// Si ya se sabe cuanto dura. Hasta entonces la duracion es cero y
    /// saltar no lleva a ninguna parte.
    pub fn lista(&self) -> bool {
        self.avisos.metadatos.load(Ordering::Acquire)
    }

    /// Si llego al final desde la ultima vez que se pregunto.
    ///
    /// Consume el aviso: quien lleva el estado lo lee una vez, se pone a
    /// cero y rebobina, igual que el `setOnCompletionListener` del movil
    /// (`Reproductor.kt:69-72`).
    pub fn termino(&self) -> bool {
        self.avisos.termino.swap(false, Ordering::AcqRel)
    }

    /// Arranca o sigue, a la velocidad dada.
    ///
    /// La velocidad se pone **justo antes** de arrancar, como en el movil
    /// (`Reproductor.kt:84-86`): puesta con el motor parado, algunos
    /// aparatos arrancan solos.
    pub fn tocar(&self, velocidad: f32) -> Result<(), ErrorAudio> {
        self.poner_velocidad(velocidad);
        // SAFETY: llamada sobre el motor vivo, que solo muere en `Drop`.
        unsafe { self.motor.Play() }.map_err(en("arrancar el sonido"))
    }

    pub fn pausar(&self) {
        // SAFETY: llamada sobre el motor vivo. Un motor en error ya avisa
        // por `fallo`, asi que el resultado no anade nada.
        unsafe {
            let _ = self.motor.Pause();
        }
    }

    pub fn pausada(&self) -> bool {
        // SAFETY: consulta sobre el motor vivo.
        unsafe { self.motor.IsPaused() }.as_bool()
    }

    /// Cuanto dura, en milisegundos. Cero mientras no haya metadatos.
    ///
    /// `GetDuration` devuelve segundos en coma flotante, y puede devolver
    /// NaN (aun no se sabe) o infinito (una emision sin final). Los dos se
    /// cuentan como cero: un `duracionMs` que no es un numero pinta una
    /// barra de longitud imposible.
    pub fn duracion_ms(&self) -> i64 {
        // SAFETY: consulta sobre el motor vivo.
        let segundos = unsafe { self.motor.GetDuration() };
        a_milisegundos(segundos)
    }

    /// Por donde va, en milisegundos.
    pub fn posicion_ms(&self) -> i64 {
        // SAFETY: consulta sobre el motor vivo.
        let segundos = unsafe { self.motor.GetCurrentTime() };
        a_milisegundos(segundos)
    }

    /// Salta a un punto. Sin metadatos todavia no hace nada: pedirle un
    /// punto a un motor que aun no sabe cuanto dura deja la posicion donde
    /// estaba y enciende su bandera de error.
    pub fn ir_a_ms(&self, ms: i64) {
        if !self.lista() {
            return;
        }
        // SAFETY: llamada sobre el motor vivo con un numero acotado por
        // quien llama (`reloj::destino_al_saltar`).
        unsafe {
            let _ = self.motor.SetCurrentTime(ms.max(0) as f64 / 1000.0);
        }
    }

    /// Cambia la velocidad **sin cambiar el tono**: de eso se encarga el
    /// motor, igual que `PlaybackParams.setSpeed` en el movil.
    pub fn poner_velocidad(&self, velocidad: f32) {
        // Cero pararia el sonido dejando el boton diciendo que suena, y un
        // negativo es lo que Media Foundation llama «reproduccion hacia
        // atras», que aqui no se ofrece.
        let v = if velocidad.is_finite() && velocidad > 0.0 {
            velocidad as f64
        } else {
            1.0
        };
        // SAFETY: llamada sobre el motor vivo con un numero finito y
        // positivo.
        unsafe {
            let _ = self.motor.SetPlaybackRate(v);
        }
    }
}

impl Drop for Salida {
    fn drop(&mut self) {
        // SAFETY: `Shutdown` es la forma documentada de soltar un
        // `IMFMediaEngine`; sin el, el motor deja vivos sus hilos y la
        // tarjeta cogida. Despues de esto no se vuelve a tocar `self`.
        unsafe {
            let _ = self.motor.Shutdown();
        }
    }
}

/// Segundos en coma flotante a milisegundos enteros.
///
/// Aparte y pura porque es donde se cuela lo imposible: Media Foundation
/// devuelve NaN mientras carga e infinito en una emision sin final, y los
/// dos hay que contarlos como «todavia no se sabe» y no como una duracion.
fn a_milisegundos(segundos: f64) -> i64 {
    if !segundos.is_finite() || segundos <= 0.0 {
        return 0;
    }
    (segundos * 1000.0) as i64
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_duracion_que_no_es_un_numero_cuenta_como_desconocida() {
        assert_eq!(a_milisegundos(f64::NAN), 0);
        assert_eq!(a_milisegundos(f64::INFINITY), 0);
        assert_eq!(a_milisegundos(-1.0), 0);
        assert_eq!(a_milisegundos(0.0), 0);
        assert_eq!(a_milisegundos(1.5), 1_500);
    }

    #[test]
    fn un_fichero_que_no_existe_no_llega_ni_a_abrir_el_motor() {
        let e = Salida::abrir(Path::new("C:/no/existe/nada.m4a")).unwrap_err();
        assert!(matches!(e, ErrorAudio::NoEsAudio { .. }), "salio {e:?}");
    }

    #[test]
    fn un_fichero_que_no_es_audio_acaba_marcando_fallo() {
        // Un fichero de verdad, con bytes de verdad, que no es un audio.
        let ruta = std::env::temp_dir().join("pixpin-audio-no-es-audio.m4a");
        std::fs::write(&ruta, b"esto no es un audio, es un texto").unwrap();

        // Abrir puede fallar en el acto (sin Media Foundation en la
        // maquina) o cargar y marcar el error desde su hilo. Las dos son
        // respuestas buenas; lo que no vale es que se de por bueno.
        if let Ok(s) = Salida::abrir(&ruta) {
            let hasta = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !s.fallo() && !s.lista() && std::time::Instant::now() < hasta {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            assert!(s.fallo(), "Media Foundation dio por bueno un texto");
        }
        let _ = std::fs::remove_file(&ruta);
    }
}
