//! Grabar una nota de voz: WASAPI para coger el sonido, Media Foundation
//! para escribir el `.m4a`.
//!
//! ## Por que dos tecnologias y no una
//!
//! `IMFMediaEngine` —lo que usa [`crate::salida`]— solo reproduce. Para
//! grabar hace falta el PCM crudo, y no solo para escribir el fichero: es de
//! donde salen los **picos** de la onda. El movil los captura mientras entra
//! el sonido justamente por eso (`pin/Voz.kt:87-98`): el `.m4a` ya escrito
//! es AAC comprimido, y sacarle las amplitudes obligaria a descodificarlo
//! entero cada vez que se pinta una lista de notas.
//!
//! Asi que: **WASAPI** (`IAudioClient` compartido, avisado por evento) da el
//! PCM; de ahi salen a la vez los picos y las muestras que come el
//! **`IMFSinkWriter`** con el codificador AAC del sistema, que es el mismo
//! camino por el que `pixpin-record` escribe el MP4 del video.
//!
//! ## El hilo
//!
//! Todo lo de arriba vive en un hilo propio. Un `IAudioClient` compartido
//! avisa cada pocos milisegundos y hay que vaciarlo antes de que se llene:
//! hacerlo en el hilo de la interfaz llenaria la nota de huecos cada vez que
//! el usuario arrastrara una ventana. El hilo no toca nada de la interfaz;
//! habla por atomicos (el nivel y lo llevado, para pintar) y devuelve la
//! [`Grabacion`] entera al pararlo.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread::JoinHandle;

use windows::Win32::Foundation::{CloseHandle, HANDLE, RPC_E_CHANGED_MODE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::{
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, IAudioCaptureClient, IAudioClient,
    IMMDeviceEnumerator, MMDeviceEnumerator, WAVEFORMATEX, eCapture, eConsole,
};
use windows::Win32::Media::MediaFoundation::{
    IMFAttributes, IMFSample, IMFSinkWriter, MF_E_INVALIDMEDIATYPE, MF_E_TOPO_CODEC_NOT_FOUND,
    MF_MT_AAC_PAYLOAD_TYPE, MF_MT_AUDIO_AVG_BYTES_PER_SECOND, MF_MT_AUDIO_BITS_PER_SAMPLE,
    MF_MT_AUDIO_BLOCK_ALIGNMENT, MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND,
    MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_TRANSCODE_CONTAINERTYPE, MF_VERSION, MFAudioFormat_AAC,
    MFAudioFormat_PCM, MFCreateAttributes, MFCreateMediaType, MFCreateMemoryBuffer, MFCreateSample,
    MFCreateSinkWriterFromURL, MFMediaType_Audio, MFSTARTUP_LITE, MFStartup,
    MFTranscodeContainerType_MPEG4,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::core::PCWSTR;

use crate::mezcla::{self, Remuestreador};
use crate::picos::{MINIMO_MS, Reparto};
use crate::{ErrorAudio, en};

/// Cuanto sonido cabe en el anillo de la tarjeta antes de que se pise.
///
/// 200 ms en unidades de 100 ns. Con menos, un tiron del sistema —un
/// antivirus, una ventana que se abre— se come un trozo de la nota; con mas,
/// parar tarda en notarse. Es holgura, no retardo: se vacia en cuanto avisa.
const ANILLO_100NS: i64 = 2_000_000;

/// Bytes por segundo del AAC de salida.
///
/// El codificador de Windows solo admite **12 000, 16 000, 20 000 o 24 000**
/// bytes por segundo (96, 128, 160 y 192 kbps). Se coge el mas bajo, que es
/// el que mas se parece a los 32 000 **bits** por segundo del movil
/// (`Voz.kt:141`) —aunque sigue siendo tres veces mas— porque una nota de
/// voz no necesita mas y el fichero viaja por WiFi en cada sincronizacion.
const BYTES_POR_SEGUNDO_AAC: u32 = 12_000;

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

/// Una grabacion en marcha.
pub struct Grabadora {
    destino: PathBuf,
    parar: Arc<AtomicBool>,
    nivel: Arc<AtomicI32>,
    muestras: Arc<AtomicU64>,
    muestreo: u32,
    hilo: Option<JoinHandle<Result<Grabacion, ErrorAudio>>>,
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
        if let Some(padre) = destino.parent()
            && !padre.as_os_str().is_empty()
        {
            std::fs::create_dir_all(padre).map_err(|_| ErrorAudio::NoEsAudio {
                ruta: padre.display().to_string(),
            })?;
        }

        let parar = Arc::new(AtomicBool::new(false));
        let nivel = Arc::new(AtomicI32::new(0));
        let muestras = Arc::new(AtomicU64::new(0));
        let (avisar, arranque) = sync_channel::<Result<u32, ErrorAudio>>(1);

        let hilo = {
            let destino = destino.to_path_buf();
            let parar = Arc::clone(&parar);
            let nivel = Arc::clone(&nivel);
            let muestras = Arc::clone(&muestras);
            std::thread::Builder::new()
                .name("pixpin-grabar".into())
                .spawn(move || grabar(&destino, &parar, &nivel, &muestras, &avisar))
                .map_err(|_| ErrorAudio::HiloCaido)?
        };

        match arranque.recv() {
            Ok(Ok(muestreo)) => Ok(Grabadora {
                destino: destino.to_path_buf(),
                parar,
                nivel,
                muestras,
                muestreo,
                hilo: Some(hilo),
            }),
            Ok(Err(e)) => {
                let _ = hilo.join();
                Err(e)
            }
            // El canal cerrado sin decir nada significa que el hilo se cayo
            // antes de avisar.
            Err(_) => Err(ErrorAudio::HiloCaido),
        }
    }

    /// El ultimo pico visto, de 0 a 32767: para pintar la onda en vivo
    /// mientras se graba.
    pub fn nivel(&self) -> i32 {
        self.nivel.load(Ordering::Relaxed)
    }

    /// Cuanto lleva grabado, en milisegundos.
    ///
    /// Se cuenta en **muestras**, no con un reloj: asi el cronometro de la
    /// pantalla dice exactamente lo mismo que acabara diciendo
    /// `duracionMs`, aunque el ordenador se hubiera atascado a mitad.
    pub fn llevado_ms(&self) -> i64 {
        mezcla::duracion_ms(
            self.muestras.load(Ordering::Relaxed) as usize,
            self.muestreo,
        )
    }

    /// Para, cierra el `.m4a` y devuelve lo que hay que guardar.
    ///
    /// Una nota mas corta que [`MINIMO_MS`] se rechaza y **se borra el
    /// fichero**, igual que en el movil, donde `Voz.parar` devuelve `false`
    /// y quien llama borra (`GrabadoraActivity.kt:134-151`): por debajo de
    /// eso lo que hubo fue un resbalon en el boton, no una nota.
    pub fn parar(mut self) -> Result<Grabacion, ErrorAudio> {
        let grabacion = self.terminar()?;
        if grabacion.duracion_ms < MINIMO_MS {
            let _ = std::fs::remove_file(&grabacion.ruta);
            return Err(ErrorAudio::DemasiadoCorta {
                duro_ms: grabacion.duracion_ms,
                minimo_ms: MINIMO_MS,
            });
        }
        Ok(grabacion)
    }

    /// Para y tira lo grabado: el usuario se arrepintio.
    pub fn cancelar(mut self) {
        let _ = self.terminar();
        let _ = std::fs::remove_file(&self.destino);
    }

    fn terminar(&mut self) -> Result<Grabacion, ErrorAudio> {
        self.parar.store(true, Ordering::Release);
        let hilo = self.hilo.take().ok_or(ErrorAudio::YaParada)?;
        hilo.join().map_err(|_| ErrorAudio::HiloCaido)?
    }
}

impl Drop for Grabadora {
    fn drop(&mut self) {
        // Soltar la grabadora sin pararla dejaria el microfono cogido y un
        // hilo girando hasta que se cierre la aplicacion.
        if self.hilo.is_some() {
            let _ = self.terminar();
            let _ = std::fs::remove_file(&self.destino);
        }
    }
}

/// COM inicializado para este hilo y soltado al salir **solo si fuimos
/// nosotros quienes lo inicializamos**.
///
/// Misma pieza que en `pixpin-record::mp4`: `CoInitializeEx` devuelve
/// `RPC_E_CHANGED_MODE` cuando el hilo ya esta en otro apartamento, y en ese
/// caso no sube la cuenta; llamar a `CoUninitialize` cerraria el COM de
/// otro.
struct ComDelHilo {
    nuestro: bool,
}

impl ComDelHilo {
    fn nuevo() -> ComDelHilo {
        // SAFETY: `CoInitializeEx` es la forma normal de entrar en COM y no
        // tiene precondiciones. Se guarda si la cuenta subio para poder
        // deshacerlo con exactitud en `Drop`.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        ComDelHilo {
            nuestro: hr != RPC_E_CHANGED_MODE,
        }
    }
}

impl Drop for ComDelHilo {
    fn drop(&mut self) {
        if self.nuestro {
            // SAFETY: empareja exactamente el `CoInitializeEx` de arriba, y
            // para cuando este guardia se destruye ya no queda vivo ningun
            // objeto COM creado aqui: se declara el primero del hilo, luego
            // se destruye el ultimo.
            unsafe { CoUninitialize() };
        }
    }
}

/// Un `HANDLE` de evento que se cierra solo.
struct Evento(HANDLE);

impl Evento {
    fn nuevo() -> Result<Evento, ErrorAudio> {
        // SAFETY: evento sin nombre, sin descriptor de seguridad, de
        // reinicio automatico y empezando sin senalar. Devuelve un HANDLE
        // propio que se cierra en `Drop`.
        let h = unsafe { CreateEventW(None, false, false, PCWSTR::null()) }
            .map_err(en("crear el evento del microfono"))?;
        Ok(Evento(h))
    }
}

impl Drop for Evento {
    fn drop(&mut self) {
        // SAFETY: el HANDLE lo creamos nosotros y no se ha cerrado antes;
        // para cuando esto corre, el `IAudioClient` que lo usaba ya esta
        // parado.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Un `WAVEFORMATEX*` de `GetMixFormat`, que hay que soltar con
/// `CoTaskMemFree` y no con el asignador de Rust.
struct FormatoDeLaMesa(*mut WAVEFORMATEX);

impl Drop for FormatoDeLaMesa {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: el puntero salio de `GetMixFormat`, que reserva con
            // el asignador de tareas de COM; es el unico dueno y no se ha
            // liberado antes.
            unsafe { CoTaskMemFree(Some(self.0 as *const _)) };
        }
    }
}

/// El cuerpo del hilo que graba.
fn grabar(
    destino: &Path,
    parar: &AtomicBool,
    nivel: &AtomicI32,
    muestras_vistas: &AtomicU64,
    avisar: &SyncSender<Result<u32, ErrorAudio>>,
) -> Result<Grabacion, ErrorAudio> {
    let _com = ComDelHilo::nuevo();
    arrancar_media_foundation();

    let preparado = preparar(destino);
    let mut equipo = match preparado {
        Ok(e) => {
            let _ = avisar.send(Ok(e.muestreo));
            e
        }
        Err(e) => {
            // El error viaja al que llamo a `empezar`; el hilo no tiene a
            // quien contarselo de otra forma.
            let _ = avisar.send(Err(clonar_error(&e)));
            return Err(e);
        }
    };

    let resultado = bucle(&mut equipo, parar, nivel, muestras_vistas);
    let total = equipo.escritas;

    // SAFETY: el cliente esta vivo y arrancado; parar dos veces es legal.
    unsafe {
        let _ = equipo.cliente.Stop();
    }
    // SAFETY: el escritor esta vivo y en escritura; `Finalize` es lo que
    // cierra el indice del MPEG-4. Sin el, el `.m4a` queda ilegible.
    unsafe { equipo.escritor.Finalize() }.map_err(en("cerrar el .m4a (Finalize)"))?;

    resultado?;

    Ok(Grabacion {
        ruta: destino.to_path_buf(),
        duracion_ms: mezcla::duracion_ms(total as usize, equipo.muestreo),
        picos: equipo.reparto.terminar(),
    })
}

/// Lo que el hilo monta una vez y usa en cada vuelta.
struct Equipo {
    cliente: IAudioClient,
    captura: IAudioCaptureClient,
    _evento: Evento,
    espera: HANDLE,
    escritor: IMFSinkWriter,
    flujo: u32,
    /// Canales y bits con los que entrega la mesa de Windows.
    canales: u16,
    bits: u16,
    remuestreador: Remuestreador,
    reparto: Reparto,
    muestreo: u32,
    /// Muestras mono ya escritas: marca el tiempo de cada muestra del MP4.
    escritas: u64,
}

fn preparar(destino: &Path) -> Result<Equipo, ErrorAudio> {
    // SAFETY: todo el bloque son llamadas COM sobre objetos que se acaban
    // de crear y cuyo dueno es este hilo. El unico puntero crudo es el
    // `WAVEFORMATEX*` de `GetMixFormat`, que queda bajo `FormatoDeLaMesa` y
    // se lee solo mientras ese guardia vive.
    unsafe {
        let enumerador: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)
                .map_err(en("abrir la lista de aparatos de sonido"))?;
        let aparato = enumerador
            .GetDefaultAudioEndpoint(eCapture, eConsole)
            .map_err(|_| ErrorAudio::SinMicrofono)?;
        let cliente: IAudioClient = aparato
            .Activate(CLSCTX_ALL, None)
            .map_err(|_| ErrorAudio::SinMicrofono)?;

        let formato = FormatoDeLaMesa(cliente.GetMixFormat().map_err(en("preguntar el formato"))?);
        if formato.0.is_null() {
            return Err(ErrorAudio::SinMicrofono);
        }
        let canales = (*formato.0).nChannels.max(1);
        let bits = (*formato.0).wBitsPerSample;
        let de_la_tarjeta = (*formato.0).nSamplesPerSec;
        // La mesa de Windows entrega coma flotante de 32 bits casi siempre,
        // y enteros de 16 en algunos aparatos antiguos. Cualquier otra cosa
        // —24 bits empaquetados— se rechaza en vez de escribir ruido.
        if bits != 32 && bits != 16 {
            return Err(ErrorAudio::FormatoDesconocido { bits });
        }

        let muestreo = mezcla::muestreo_de_grabacion(de_la_tarjeta);

        let evento = Evento::nuevo()?;
        cliente
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                ANILLO_100NS,
                0,
                formato.0,
                None,
            )
            .map_err(en("abrir el microfono"))?;
        cliente
            .SetEventHandle(evento.0)
            .map_err(en("enganchar el aviso del microfono"))?;
        let captura: IAudioCaptureClient = cliente
            .GetService()
            .map_err(en("pedir el lector del microfono"))?;

        let (escritor, flujo) = abrir_escritor(destino, muestreo)?;
        cliente.Start().map_err(en("arrancar el microfono"))?;

        let espera = evento.0;
        Ok(Equipo {
            cliente,
            captura,
            _evento: evento,
            espera,
            escritor,
            flujo,
            canales,
            bits,
            remuestreador: Remuestreador::nuevo(de_la_tarjeta, muestreo),
            reparto: Reparto::nuevo(muestreo),
            muestreo,
            escritas: 0,
        })
    }
}

fn bucle(
    equipo: &mut Equipo,
    parar: &AtomicBool,
    nivel: &AtomicI32,
    muestras_vistas: &AtomicU64,
) -> Result<(), ErrorAudio> {
    while !parar.load(Ordering::Acquire) {
        // SAFETY: el evento vive dentro de `equipo` y el cliente lo tiene
        // enganchado; la espera es la forma documentada de sincronizarse
        // con un `IAudioClient` en modo evento. El tope de 200 ms evita
        // quedarse colgado si el aparato desaparece a mitad (un USB que se
        // desenchufa no vuelve a senalar nunca).
        let que_paso = unsafe { WaitForSingleObject(equipo.espera, 200) };
        if que_paso != WAIT_OBJECT_0 {
            continue;
        }
        vaciar(equipo, nivel, muestras_vistas)?;
    }
    // Una ultima pasada: lo que entro entre el ultimo aviso y el boton de
    // parar es medio segundo de nota que si no se perderia.
    vaciar(equipo, nivel, muestras_vistas)
}

/// Saca de la tarjeta todo lo que haya en el anillo.
fn vaciar(
    equipo: &mut Equipo,
    nivel: &AtomicI32,
    muestras_vistas: &AtomicU64,
) -> Result<(), ErrorAudio> {
    loop {
        // SAFETY: consulta sobre el lector vivo.
        let cuantos = match unsafe { equipo.captura.GetNextPacketSize() } {
            Ok(n) => n,
            // El aparato se fue (se desenchufo el microfono). Se deja de
            // vaciar y la nota se cierra con lo que haya.
            Err(_) => return Ok(()),
        };
        if cuantos == 0 {
            return Ok(());
        }

        let mut datos: *mut u8 = std::ptr::null_mut();
        let mut marcos = 0u32;
        let mut banderas = 0u32;
        // SAFETY: los tres punteros son a locales vivos; `GetBuffer` los
        // rellena y a partir de ahi `datos` apunta a `marcos * canales *
        // bits/8` bytes validos hasta el `ReleaseBuffer` de mas abajo, que
        // se hace siempre antes de la siguiente vuelta.
        if unsafe {
            equipo
                .captura
                .GetBuffer(&mut datos, &mut marcos, &mut banderas, None, None)
        }
        .is_err()
        {
            return Ok(());
        }

        let canales = equipo.canales as usize;
        let total = marcos as usize * canales;
        // Bandera de silencio: la tarjeta dice «aqui no hubo nada» y el
        // buffer puede traer basura. Se escriben ceros, que es lo que sono.
        let mono = if banderas & 2 != 0 || datos.is_null() {
            vec![0i16; marcos as usize]
        } else if equipo.bits == 32 {
            // SAFETY: `GetBuffer` garantiza `marcos * canales` muestras de
            // 32 bits alineadas en el puntero entregado; el corte se hace
            // con ese mismo tamano y no se guarda mas alla del
            // `ReleaseBuffer`.
            let crudo = unsafe { std::slice::from_raw_parts(datos as *const f32, total) };
            mezcla::a_mono_desde_f32(crudo, equipo.canales)
        } else {
            // SAFETY: igual que arriba, con muestras de 16 bits.
            let crudo = unsafe { std::slice::from_raw_parts(datos as *const i16, total) };
            mezcla::a_mono_desde_i16(crudo, equipo.canales)
        };

        // SAFETY: devuelve el mismo numero de marcos que entrego
        // `GetBuffer`, que es lo que pide el contrato de WASAPI. A partir
        // de aqui `datos` ya no vale, y no se vuelve a usar.
        unsafe {
            let _ = equipo.captura.ReleaseBuffer(marcos);
        }

        let muestras = equipo.remuestreador.empuja(&mono);
        if muestras.is_empty() {
            continue;
        }
        nivel.store(crate::picos::pico_de_bloque(&muestras), Ordering::Relaxed);
        equipo.reparto.empuja(&muestras);
        escribir(equipo, &muestras)?;
        muestras_vistas.store(equipo.escritas, Ordering::Relaxed);
    }
}

/// Mete un bloque de muestras mono en el `.m4a`.
fn escribir(equipo: &mut Equipo, muestras: &[i16]) -> Result<(), ErrorAudio> {
    let bytes = muestras.len() * 2;
    // SAFETY: `MFCreateMemoryBuffer` solo reserva memoria.
    let buffer = unsafe { MFCreateMemoryBuffer(bytes as u32) }
        .map_err(en("reservar memoria para un trozo de sonido"))?;

    let mut destino: *mut u8 = std::ptr::null_mut();
    // SAFETY: `Lock` entrega un puntero a un bloque de al menos `bytes`
    // bytes propiedad del buffer, valido hasta `Unlock`, que se hace justo
    // despues y en el mismo camino.
    unsafe { buffer.Lock(&mut destino, None, None) }.map_err(en("bloquear el trozo de sonido"))?;
    // SAFETY: `destino` apunta a `bytes` bytes recien reservados y sin
    // alias, y el origen son `bytes` bytes de un `Vec` vivo. Se copia byte
    // a byte porque el destino no esta alineado a `i16` por contrato.
    unsafe {
        std::ptr::copy_nonoverlapping(muestras.as_ptr() as *const u8, destino, bytes);
    }
    // SAFETY: empareja el `Lock` de arriba.
    unsafe { buffer.Unlock() }.map_err(en("desbloquear el trozo de sonido"))?;
    // SAFETY: `bytes` es exactamente lo que se acaba de escribir y no pasa
    // del tamano con el que se creo el buffer.
    unsafe { buffer.SetCurrentLength(bytes as u32) }.map_err(en("medir el trozo de sonido"))?;

    let cien_ns = |muestras: u64| (muestras as i64 * 10_000_000) / equipo.muestreo.max(1) as i64;
    // SAFETY: `MFCreateSample` no tiene precondiciones; el buffer esta
    // vivo, y los tiempos salen de una cuenta de muestras que solo crece.
    let muestra: IMFSample = unsafe {
        let m = MFCreateSample().map_err(en("crear la muestra de sonido"))?;
        m.AddBuffer(&buffer).map_err(en("montar la muestra"))?;
        m.SetSampleTime(cien_ns(equipo.escritas))
            .map_err(en("fechar la muestra"))?;
        m.SetSampleDuration(cien_ns(muestras.len() as u64))
            .map_err(en("medir la muestra"))?;
        m
    };

    // SAFETY: escritor y flujo vivos, muestra recien montada.
    unsafe { equipo.escritor.WriteSample(equipo.flujo, &muestra) }
        .map_err(en("escribir un trozo de sonido"))?;
    equipo.escritas += muestras.len() as u64;
    Ok(())
}

/// Monta el `IMFSinkWriter` que escribe el `.m4a`: contenedor MPEG-4, salida
/// AAC mono, entrada PCM de 16 bits.
fn abrir_escritor(destino: &Path, muestreo: u32) -> Result<(IMFSinkWriter, u32), ErrorAudio> {
    let ruta: Vec<u16> = destino
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // SAFETY: bloque de llamadas COM sobre objetos recien creados; el unico
    // puntero crudo es la ruta terminada en cero de arriba, que vive hasta
    // el final de la funcion.
    unsafe {
        let mut atributos: Option<IMFAttributes> = None;
        MFCreateAttributes(&mut atributos, 1).map_err(en("crear los atributos del .m4a"))?;
        let atributos = atributos.ok_or_else(|| ErrorAudio::Windows {
            paso: "crear los atributos del .m4a",
            fuente: windows::core::Error::empty(),
        })?;
        // MPEG-4: es lo que hay dentro de un `.m4a`, y lo mismo que pone
        // `MediaRecorder.OutputFormat.MPEG_4` en el movil (`Voz.kt:62`).
        atributos
            .SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_MPEG4)
            .map_err(en("elegir el contenedor MPEG-4"))?;

        let escritor = MFCreateSinkWriterFromURL(PCWSTR(ruta.as_ptr()), None, &atributos).map_err(
            |fuente| ErrorAudio::Windows {
                paso: "crear el fichero .m4a",
                fuente,
            },
        )?;

        let salida = MFCreateMediaType().map_err(en("crear el tipo de salida"))?;
        salida
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
            .map_err(en("marcar la salida como sonido"))?;
        salida
            .SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)
            .map_err(en("marcar la salida como AAC"))?;
        salida
            .SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)
            .map_err(en("poner los bits de la salida"))?;
        salida
            .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, muestreo)
            .map_err(en("poner el muestreo de la salida"))?;
        // Mono, como el movil, que nunca llama a `setAudioChannels` y se
        // queda con el defecto de un canal (`Voz.kt`, nota del inventario).
        salida
            .SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 1)
            .map_err(en("poner los canales de la salida"))?;
        salida
            .SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, BYTES_POR_SEGUNDO_AAC)
            .map_err(en("poner el bitrate"))?;
        // Cero es «AAC crudo», que es lo que va dentro de un MPEG-4; uno
        // seria ADTS, con su cabecera por trama, y el contenedor la
        // rechaza.
        salida
            .SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0)
            .map_err(en("poner el tipo de carga AAC"))?;

        let flujo = escritor.AddStream(&salida).map_err(|fuente| {
            if falta_codificador(&fuente) {
                ErrorAudio::SinCodificadorAac
            } else {
                ErrorAudio::Windows {
                    paso: "anadir el flujo de sonido",
                    fuente,
                }
            }
        })?;

        let entrada = MFCreateMediaType().map_err(en("crear el tipo de entrada"))?;
        entrada
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
            .map_err(en("marcar la entrada como sonido"))?;
        entrada
            .SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)
            .map_err(en("marcar la entrada como PCM"))?;
        entrada
            .SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)
            .map_err(en("poner los bits de la entrada"))?;
        entrada
            .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, muestreo)
            .map_err(en("poner el muestreo de la entrada"))?;
        entrada
            .SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 1)
            .map_err(en("poner los canales de la entrada"))?;
        // Mono de 16 bits: dos bytes por muestra, y por tanto por marco.
        entrada
            .SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 2)
            .map_err(en("poner el alineamiento de la entrada"))?;
        entrada
            .SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, muestreo * 2)
            .map_err(en("poner los bytes por segundo de la entrada"))?;

        escritor
            .SetInputMediaType(flujo, &entrada, None)
            .map_err(|fuente| {
                if falta_codificador(&fuente) {
                    ErrorAudio::SinCodificadorAac
                } else {
                    ErrorAudio::Windows {
                        paso: "encajar el PCM con el codificador AAC",
                        fuente,
                    }
                }
            })?;
        escritor
            .BeginWriting()
            .map_err(en("empezar a escribir el .m4a"))?;

        Ok((escritor, flujo))
    }
}

/// El error que da Media Foundation cuando no encuentra un MFT que haga AAC
/// es el mismo que cuando el tipo de salida no le gusta, asi que los dos se
/// traducen al mensaje sobre el que el usuario puede actuar: falta el
/// paquete multimedia (las ediciones N de Windows salen sin el).
fn falta_codificador(e: &windows::core::Error) -> bool {
    e.code() == MF_E_TOPO_CODEC_NOT_FOUND || e.code() == MF_E_INVALIDMEDIATYPE
}

/// Copia un error para poder mandarlo por el canal y devolverlo a la vez.
///
/// `ErrorAudio` no es `Clone` porque `windows::core::Error` no lo es del
/// todo barato; aqui se rehace a mano y el caso de Windows se degrada a su
/// codigo, que es lo unico que necesita quien lo lee.
fn clonar_error(e: &ErrorAudio) -> ErrorAudio {
    match e {
        ErrorAudio::SinMicrofono => ErrorAudio::SinMicrofono,
        ErrorAudio::SinCodificadorAac => ErrorAudio::SinCodificadorAac,
        ErrorAudio::FormatoDesconocido { bits } => ErrorAudio::FormatoDesconocido { bits: *bits },
        ErrorAudio::NoEsAudio { ruta } => ErrorAudio::NoEsAudio { ruta: ruta.clone() },
        ErrorAudio::DemasiadoCorta { duro_ms, minimo_ms } => ErrorAudio::DemasiadoCorta {
            duro_ms: *duro_ms,
            minimo_ms: *minimo_ms,
        },
        ErrorAudio::YaParada => ErrorAudio::YaParada,
        ErrorAudio::HiloCaido => ErrorAudio::HiloCaido,
        ErrorAudio::Windows { paso, fuente } => ErrorAudio::Windows {
            paso,
            fuente: windows::core::Error::from_hresult(fuente.code()),
        },
    }
}

/// Media Foundation se arranca una vez por proceso y no se apaga.
fn arrancar_media_foundation() {
    static ARRANQUE: std::sync::Once = std::sync::Once::new();
    ARRANQUE.call_once(|| {
        // SAFETY: `MFStartup` solo pide la version de la cabecera con la
        // que se compilo, que es lo que vale `MF_VERSION`. Un fallo aqui
        // reaparece como error al crear el escritor.
        unsafe {
            let _ = MFStartup(MF_VERSION, MFSTARTUP_LITE);
        }
    });
}

use std::os::windows::ffi::OsStrExt;

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
    }
}
