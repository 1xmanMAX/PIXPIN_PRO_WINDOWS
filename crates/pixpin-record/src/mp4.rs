//! MP4 con video H.264 a partir de una lista de fotogramas RGBA.
//!
//! Se usa **Media Foundation** (`IMFSinkWriter`) y no una libreria externa
//! por la misma razon por la que el GIF se escribe a mano: meter FFmpeg
//! obligaria a decidir sobre la GPL y a arrastrar decenas de megas. Media
//! Foundation viene con Windows, el proyecto ya la usa para reproducir el
//! video del pin, y trae el codificador H.264 de Microsoft.
//!
//! Las decisiones que se tomaron, y por que:
//!
//! - **Se escribe a fichero, no a un `Vec<u8>`.** El sink writer sabe
//!   escribir a una ruta o a un `IMFByteStream`; envolver un buffer en un
//!   byte stream propio serian cien lineas de COM para nada, porque quien
//!   graba la pantalla quiere un fichero.
//! - **La entrada se declara RGB32 y la salida H.264.** El sink writer mete
//!   por su cuenta el conversor de color (RGB -> NV12). Escribir NV12 a mano
//!   obligaria a hacer la conversion de espacio de color aqui, y hacerla mal
//!   —confundir BT.601 con BT.709, o el rango limitado con el completo— es
//!   exactamente de donde salen las caras verdes y los grises lavados.
//! - **Codificador por software, no por hardware.** El sink writer solo usa
//!   MFT de hardware si se le pide con
//!   `MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS`; aqui se deja apagado a
//!   proposito. El codificador de Microsoft esta en todas las ediciones
//!   no-N de Windows y da el mismo resultado en cualquier maquina, mientras
//!   que los de Intel/NVIDIA/AMD varian y varios rechazan los tamanos
//!   pequenos con los que corren las pruebas.

use std::path::{Path, PathBuf};
use std::sync::Once;

use pixpin_codec::imagen::ImagenRgba;
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::Media::MediaFoundation::{
    IMFAttributes, IMFSample, IMFSinkWriter, MF_E_INVALIDMEDIATYPE, MF_E_TOPO_CODEC_NOT_FOUND,
    MF_MT_ALL_SAMPLES_INDEPENDENT, MF_MT_AVG_BITRATE, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_RATE,
    MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_PIXEL_ASPECT_RATIO,
    MF_MT_SUBTYPE, MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SINK_WRITER_DISABLE_THROTTLING,
    MF_TRANSCODE_CONTAINERTYPE, MF_VERSION, MFCreateAttributes, MFCreateMediaType,
    MFCreateMemoryBuffer, MFCreateSample, MFCreateSinkWriterFromURL, MFMediaType_Video,
    MFSTARTUP_LITE, MFStartup, MFTranscodeContainerType_MPEG4, MFVideoFormat_H264,
    MFVideoFormat_RGB32, MFVideoInterlace_Progressive,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows::core::PCWSTR;

/// Media Foundation cuenta el tiempo en unidades de 100 nanosegundos.
const UNIDADES_POR_SEGUNDO: i64 = 10_000_000;

/// Bytes por pixel de RGBA y de RGB32: los dos son cuatro canales de 8 bits.
const BYTES_POR_PIXEL: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcionesMp4 {
    /// Fotogramas por segundo a los que se grabo.
    pub por_segundo: u32,
    /// Bits por segundo del video. Si es None, se calcula del tamano.
    pub bitrate: Option<u32>,
}

impl Default for OpcionesMp4 {
    fn default() -> Self {
        // 30 es el ritmo al que graba la mayoria de capturadores y el que
        // reproduce cualquier cosa sin pensarlo.
        OpcionesMp4 {
            por_segundo: 30,
            bitrate: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorMp4 {
    #[error("no hay ningun fotograma que codificar")]
    SinFotogramas,
    #[error("los fotogramas por segundo han de ser mayores que cero")]
    SinRitmo,
    #[error(
        "el fotograma {indice} mide {ancho}x{alto} pero el primero mide \
         {espera_ancho}x{espera_alto}: un video no puede cambiar de tamano a mitad"
    )]
    TamanoDistinto {
        indice: usize,
        ancho: u32,
        alto: u32,
        espera_ancho: u32,
        espera_alto: u32,
    },
    #[error("el fotograma {indice} tiene {tiene} bytes pero {ancho}x{alto} necesita {espera}")]
    BufferIncoherente {
        indice: usize,
        ancho: u32,
        alto: u32,
        tiene: usize,
        espera: usize,
    },
    #[error(
        "el tamano {ancho}x{alto} no cabe en un H.264: al redondear a par queda \
         {par_ancho}x{par_alto} y hace falta al menos 2x2"
    )]
    DemasiadoPequeno {
        ancho: u32,
        alto: u32,
        par_ancho: u32,
        par_alto: u32,
    },
    #[error(
        "este Windows no trae el codificador H.264 (suele ser una edicion N sin el \
         paquete de caracteristicas multimedia)"
    )]
    SinCodificadorH264,
    #[error("no se pudo escribir {ruta}: {fuente}")]
    Escritura {
        ruta: PathBuf,
        #[source]
        fuente: windows::core::Error,
    },
    #[error("Media Foundation fallo al {paso}: {fuente}")]
    MediaFoundation {
        paso: &'static str,
        #[source]
        fuente: windows::core::Error,
    },
}

/// Envuelve un fallo de Media Foundation diciendo en que paso ocurrio. Sin
/// esto todos los errores serian el mismo HRESULT sin contexto, y depurar
/// una tuberia de seis pasos a base de `0x80004005` es perder la tarde.
fn en(paso: &'static str) -> impl Fn(windows::core::Error) -> ErrorMp4 {
    move |fuente| ErrorMp4::MediaFoundation { paso, fuente }
}

/// El error que devuelve Media Foundation cuando no encuentra un MFT que
/// haga H.264 es el mismo que cuando el tipo de salida no le gusta, asi que
/// los dos se traducen al mensaje sobre el que el usuario puede actuar:
/// falta el paquete multimedia.
fn falta_codificador(e: &windows::core::Error) -> bool {
    e.code() == MF_E_TOPO_CODEC_NOT_FOUND || e.code() == MF_E_INVALIDMEDIATYPE
}

/// Media Foundation se arranca una vez por proceso y no se apaga, igual que
/// en el reproductor del pin: `MFStartup`/`MFShutdown` llevan su propia
/// cuenta, asi que este arranque convive con el de aquel sin pisarlo.
static ARRANQUE: Once = Once::new();

fn arrancar_media_foundation() {
    ARRANQUE.call_once(|| {
        // SAFETY: `MFStartup` no tiene mas precondicion que recibir la
        // version de la cabecera con la que se compilo, que es lo que vale
        // `MF_VERSION`. Si fallara, el fallo reaparece luego como error al
        // crear el sink writer, que si sabemos contar.
        unsafe {
            let _ = MFStartup(MF_VERSION, MFSTARTUP_LITE);
        }
    });
}

/// COM inicializado para este hilo, y soltado al salir **solo si fuimos
/// nosotros quienes lo inicializamos**.
///
/// `codificar_mp4` es una funcion de libreria: puede llamarla un hilo que ya
/// tenga COM montado (el de la interfaz, en apartamento) o uno recien nacido
/// de un pool. `CoInitializeEx` devuelve `RPC_E_CHANGED_MODE` cuando el hilo
/// ya esta en otro apartamento; en ese caso no incrementa la cuenta, y
/// llamar a `CoUninitialize` cerraria el COM de otro.
struct ComDelHilo {
    nuestro: bool,
}

impl ComDelHilo {
    fn nuevo() -> ComDelHilo {
        // SAFETY: `CoInitializeEx` es la forma normal de entrar en COM y no
        // tiene precondiciones. Se guarda si la cuenta subio, para poder
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
            // objeto COM creado aqui: se declara el primero de la funcion,
            // luego se destruye el ultimo.
            unsafe { CoUninitialize() };
        }
    }
}

/// Empaqueta dos u32 en el u64 que espera Media Foundation para los pares
/// (ancho, alto), (numerador, denominador) y demas: el primero arriba.
fn empaquetar(primero: u32, segundo: u32) -> u64 {
    ((primero as u64) << 32) | (segundo as u64)
}

/// Bits por segundo cuando quien llama no opina.
///
/// La cuenta es 0,1 bits por pixel y segundo. Una captura de pantalla es
/// mucho mas facil de comprimir que el video de una camara —zonas planas,
/// nada de ruido de sensor—, asi que las tablas de bitrate para camara
/// sobran de largo. Sale 6,2 Mb/s para 1920x1080 a 30, holgado para que el
/// texto se lea nitido. Los topes evitan los dos extremos absurdos: un
/// video ilegible de 100 kb/s para una ventanita, y 80 Mb/s para una 4K.
fn bitrate_por_defecto(ancho: u32, alto: u32, por_segundo: u32) -> u32 {
    let pixeles_por_segundo = ancho as u64 * alto as u64 * por_segundo as u64;
    (pixeles_por_segundo / 10).clamp(500_000, 20_000_000) as u32
}

/// Comprueba la lista y devuelve el tamano **ya redondeado a par** con el
/// que se va a codificar.
///
/// H.264 submuestrea el color de dos en dos pixeles (4:2:0), asi que un
/// ancho o un alto impar no tiene donde ir. Y una zona de captura de 641
/// pixeles de ancho es de lo mas normal, no un caso de laboratorio.
///
/// Se **recorta** la ultima columna o fila en vez de rellenar: rellenar
/// mete una linea negra que se ve en el borde del video y ademas cambia lo
/// que el usuario encuadro, mientras que perder un pixel del borde de una
/// captura de pantalla no lo nota nadie.
fn medidas(fotogramas: &[ImagenRgba]) -> Result<(u32, u32), ErrorMp4> {
    let Some(primero) = fotogramas.first() else {
        return Err(ErrorMp4::SinFotogramas);
    };

    for (indice, fotograma) in fotogramas.iter().enumerate() {
        if (fotograma.ancho, fotograma.alto) != (primero.ancho, primero.alto) {
            return Err(ErrorMp4::TamanoDistinto {
                indice,
                ancho: fotograma.ancho,
                alto: fotograma.alto,
                espera_ancho: primero.ancho,
                espera_alto: primero.alto,
            });
        }
        let espera = fotograma.bytes_esperados();
        if fotograma.pixeles.len() != espera {
            return Err(ErrorMp4::BufferIncoherente {
                indice,
                ancho: fotograma.ancho,
                alto: fotograma.alto,
                tiene: fotograma.pixeles.len(),
                espera,
            });
        }
    }

    let par_ancho = primero.ancho & !1;
    let par_alto = primero.alto & !1;
    if par_ancho < 2 || par_alto < 2 {
        return Err(ErrorMp4::DemasiadoPequeno {
            ancho: primero.ancho,
            alto: primero.alto,
            par_ancho,
            par_alto,
        });
    }
    Ok((par_ancho, par_alto))
}

/// La ruta como cadena ancha terminada en cero, que es lo que come
/// `MFCreateSinkWriterFromURL`. Se pasa por `OsStr` para no romper las
/// rutas con caracteres que no sean UTF-8 valido.
fn a_utf16(ruta: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    ruta.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Escribe los fotogramas RGBA en `destino` como MP4 con video H.264.
///
/// Si algo falla a mitad, el fichero a medio escribir se borra: un MP4 sin
/// su caja `moov` no lo abre ningun reproductor, y dejarlo ahi solo sirve
/// para que el usuario crea que tiene una grabacion.
pub fn codificar_mp4(
    fotogramas: &[ImagenRgba],
    opciones: OpcionesMp4,
    destino: &Path,
) -> Result<(), ErrorMp4> {
    if opciones.por_segundo == 0 {
        return Err(ErrorMp4::SinRitmo);
    }
    // Las comprobaciones van antes de tocar COM para que los errores de
    // quien llama no dependan de que Media Foundation este disponible.
    let (ancho, alto) = medidas(fotogramas)?;

    arrancar_media_foundation();
    // Se declara el primero para que se destruya el ultimo, cuando ya no
    // queda vivo ningun objeto COM de `escribir`.
    let _com = ComDelHilo::nuevo();

    let bitrate = opciones
        .bitrate
        .unwrap_or_else(|| bitrate_por_defecto(ancho, alto, opciones.por_segundo));

    let resultado = escribir(
        fotogramas,
        ancho,
        alto,
        opciones.por_segundo,
        bitrate,
        destino,
    );
    if resultado.is_err() {
        let _ = std::fs::remove_file(destino);
    }
    resultado
}

fn escribir(
    fotogramas: &[ImagenRgba],
    ancho: u32,
    alto: u32,
    por_segundo: u32,
    bitrate: u32,
    destino: &Path,
) -> Result<(), ErrorMp4> {
    let ruta = a_utf16(destino);

    let atributos = atributos_del_escritor()?;
    // SAFETY: COM esta inicializado en este hilo (guardia de `codificar_mp4`)
    // y Media Foundation arrancada. `ruta` vive mas que la llamada y termina
    // en cero, que es lo que exige `PCWSTR`; `atributos` es propio.
    let escritor = unsafe { MFCreateSinkWriterFromURL(PCWSTR(ruta.as_ptr()), None, &atributos) }
        .map_err(|fuente| ErrorMp4::Escritura {
            ruta: destino.to_path_buf(),
            fuente,
        })?;

    let flujo = anadir_flujo_h264(&escritor, ancho, alto, por_segundo, bitrate)?;
    fijar_entrada_rgb32(&escritor, flujo, ancho, alto, por_segundo)?;

    // SAFETY: `escritor` es propio y ya tiene sus dos tipos configurados,
    // que es lo unico que `BeginWriting` exige.
    unsafe { escritor.BeginWriting() }.map_err(en("empezar a escribir el MP4"))?;

    for (indice, fotograma) in fotogramas.iter().enumerate() {
        // El tiempo de cada fotograma se calcula desde el principio y no
        // sumando duraciones: a 30 fps la duracion exacta (333333,3
        // unidades) no es entera, y arrastrar el redondeo fotograma a
        // fotograma atrasa el video un cuadro cada pocos segundos.
        let inicio = comienzo(indice, por_segundo);
        let duracion = comienzo(indice + 1, por_segundo) - inicio;
        let muestra = muestra_de(fotograma, ancho, alto, inicio, duracion)?;
        // SAFETY: la muestra se acaba de crear aqui y `flujo` es el indice
        // que devolvio `AddStream` sobre este mismo escritor.
        unsafe { escritor.WriteSample(flujo, &muestra) }.map_err(en("escribir un fotograma"))?;
    }

    // `Finalize` es quien escribe la caja `moov`. Sin el, el fichero tiene
    // los datos pero no el indice, y no lo abre nadie.
    // SAFETY: `escritor` es propio y esta escribiendo desde `BeginWriting`.
    unsafe { escritor.Finalize() }.map_err(en("cerrar el MP4 (Finalize)"))?;
    Ok(())
}

/// Instante en el que empieza el fotograma `indice`, en unidades de 100 ns.
fn comienzo(indice: usize, por_segundo: u32) -> i64 {
    (indice as i64 * UNIDADES_POR_SEGUNDO) / por_segundo as i64
}

fn atributos_del_escritor() -> Result<IMFAttributes, ErrorMp4> {
    let mut atributos: Option<IMFAttributes> = None;
    // SAFETY: `atributos` es una variable local valida donde la funcion
    // deposita el objeto; si falla no se usa.
    unsafe { MFCreateAttributes(&mut atributos, 3) }.map_err(en("crear los atributos"))?;
    let atributos = atributos.ok_or_else(|| ErrorMp4::MediaFoundation {
        paso: "crear los atributos",
        fuente: windows::core::Error::empty(),
    })?;

    // SAFETY: el objeto acaba de crearse aqui, nadie mas lo tiene, y las
    // claves y valores son constantes del propio Media Foundation.
    unsafe {
        // Sin esto el contenedor se deduce de la extension del fichero, y
        // quien llama puede querer guardar la grabacion con otra.
        atributos
            .SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_MPEG4)
            .map_err(en("elegir el contenedor MP4"))?;
        // Sin desactivar la regulacion, `WriteSample` duerme para no
        // adelantarse al reloj de presentacion. Aqui no se graba en vivo:
        // los fotogramas ya estan en memoria y hay que soltarlos tan rapido
        // como el codificador aguante.
        atributos
            .SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)
            .map_err(en("desactivar la regulacion"))?;
        // Explicito aunque coincida con el valor por defecto: ver la nota
        // de la cabecera del modulo sobre por que no se usa el codificador
        // del hardware.
        atributos
            .SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 0)
            .map_err(en("fijar el codificador por software"))?;
    }
    Ok(atributos)
}

/// Anade el flujo de salida H.264 y devuelve su indice.
fn anadir_flujo_h264(
    escritor: &IMFSinkWriter,
    ancho: u32,
    alto: u32,
    por_segundo: u32,
    bitrate: u32,
) -> Result<u32, ErrorMp4> {
    // SAFETY: `MFCreateMediaType` no tiene precondiciones y el tipo que
    // devuelve es propio; solo recibe claves y GUID constantes.
    let salida = unsafe { MFCreateMediaType() }.map_err(en("crear el tipo de salida"))?;
    // SAFETY: `salida` acaba de crearse en esta funcion.
    unsafe {
        salida
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .and_then(|()| salida.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264))
            .and_then(|()| salida.SetUINT32(&MF_MT_AVG_BITRATE, bitrate))
            .and_then(|()| {
                salida.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
            })
            .and_then(|()| salida.SetUINT64(&MF_MT_FRAME_SIZE, empaquetar(ancho, alto)))
            .and_then(|()| salida.SetUINT64(&MF_MT_FRAME_RATE, empaquetar(por_segundo, 1)))
            // Pixeles cuadrados. Sin esta clave hay reproductores que
            // asumen la relacion de aspecto del DVD y estiran la captura.
            .and_then(|()| salida.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, empaquetar(1, 1)))
            .map_err(en("describir el video H.264"))?;
    }

    // SAFETY: `escritor` y `salida` estan vivos. Esta es la llamada que
    // resuelve el MFT codificador, y por eso es aqui donde se detecta que
    // en esta maquina no hay ninguno.
    unsafe { escritor.AddStream(&salida) }.map_err(|fuente| {
        if falta_codificador(&fuente) {
            ErrorMp4::SinCodificadorH264
        } else {
            ErrorMp4::MediaFoundation {
                paso: "anadir el flujo de video",
                fuente,
            }
        }
    })
}

/// Describe lo que le vamos a dar al escritor: RGB32 de arriba abajo.
fn fijar_entrada_rgb32(
    escritor: &IMFSinkWriter,
    flujo: u32,
    ancho: u32,
    alto: u32,
    por_segundo: u32,
) -> Result<(), ErrorMp4> {
    // SAFETY: `MFCreateMediaType` no tiene precondiciones.
    let entrada = unsafe { MFCreateMediaType() }.map_err(en("crear el tipo de entrada"))?;
    let paso_de_fila = (ancho as usize * BYTES_POR_PIXEL) as u32;
    // SAFETY: `entrada` acaba de crearse en esta funcion.
    unsafe {
        entrada
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .and_then(|()| entrada.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32))
            .and_then(|()| {
                entrada.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
            })
            // **El orden de las filas.** Los formatos RGB de Media
            // Foundation son, por herencia de GDI, de abajo arriba: quien
            // los lee da la vuelta al fotograma salvo que se le diga otra
            // cosa. Decirselo es exactamente esta clave: el paso de fila es
            // un entero con signo, y positivo significa "las filas van de
            // arriba abajo", que es como viene una `ImagenRgba`. La
            // alternativa —copiar las filas al reves y declarar el paso
            // negativo— da el mismo video, pero deja la decision repartida
            // entre el bucle de copia y el tipo de medio; asi vive en un
            // solo sitio y se ve.
            .and_then(|()| entrada.SetUINT32(&MF_MT_DEFAULT_STRIDE, paso_de_fila))
            // Cada fotograma que entra es completo e independiente: no hay
            // predicciones que resolver antes de llegar al codificador.
            .and_then(|()| entrada.SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1))
            .and_then(|()| entrada.SetUINT64(&MF_MT_FRAME_SIZE, empaquetar(ancho, alto)))
            .and_then(|()| entrada.SetUINT64(&MF_MT_FRAME_RATE, empaquetar(por_segundo, 1)))
            .and_then(|()| entrada.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, empaquetar(1, 1)))
            .map_err(en("describir los fotogramas RGB32"))?;
    }

    // SAFETY: `escritor` y `entrada` siguen vivos; el tercer parametro
    // (parametros de codificacion) es opcional y no se usa.
    unsafe { escritor.SetInputMediaType(flujo, &entrada, None) }.map_err(|fuente| {
        if falta_codificador(&fuente) {
            ErrorMp4::SinCodificadorH264
        } else {
            ErrorMp4::MediaFoundation {
                paso: "conectar los fotogramas RGB32 con el codificador",
                fuente,
            }
        }
    })
}

/// Un fotograma RGBA convertido en la muestra RGB32 que espera el escritor.
fn muestra_de(
    imagen: &ImagenRgba,
    ancho: u32,
    alto: u32,
    inicio: i64,
    duracion: i64,
) -> Result<IMFSample, ErrorMp4> {
    let paso_destino = ancho as usize * BYTES_POR_PIXEL;
    let paso_origen = imagen.ancho as usize * BYTES_POR_PIXEL;
    let total = paso_destino * alto as usize;

    // SAFETY: `MFCreateMemoryBuffer` solo reserva memoria; `total` cabe en
    // u32 porque `medidas` ya comprobo que existe un buffer de origen de al
    // menos ese tamano.
    let buffer = unsafe { MFCreateMemoryBuffer(total as u32) }
        .map_err(en("reservar el buffer de un fotograma"))?;

    let mut destino: *mut u8 = std::ptr::null_mut();
    // SAFETY: `buffer` es propio y recien creado, asi que nadie mas lo
    // tiene bloqueado; `Lock` deja en `destino` un puntero a por lo menos
    // los `total` bytes con los que se creo.
    unsafe { buffer.Lock(&mut destino, None, None) }.map_err(en("bloquear un fotograma"))?;

    // SAFETY: el puntero viene de `Lock`, que garantiza `total` bytes
    // escribibles y validos para u8 (alineamiento 1), y esa region no la
    // toca nadie mas hasta el `Unlock` de mas abajo.
    let filas = unsafe { std::slice::from_raw_parts_mut(destino, total) };
    for y in 0..alto as usize {
        let origen = &imagen.pixeles[y * paso_origen..y * paso_origen + paso_destino];
        let fila = &mut filas[y * paso_destino..(y + 1) * paso_destino];
        for (entra, sale) in origen.chunks_exact(4).zip(fila.chunks_exact_mut(4)) {
            // **RGBA no es RGB32.** Lo que Media Foundation llama RGB32 es
            // BGRA en memoria: primero el azul. Copiar los bytes tal cual
            // es el fallo por el que la gente ve los videos con el rojo y
            // el azul cambiados —cielos naranjas, caras azules—, y como el
            // alfa cae en el mismo sitio en los dos formatos, leyendo el
            // codigo no se nota.
            sale[0] = entra[2];
            sale[1] = entra[1];
            sale[2] = entra[0];
            sale[3] = entra[3];
        }
    }

    // SAFETY: empareja el `Lock` de arriba; `filas` ya no se usa.
    unsafe { buffer.Unlock() }.map_err(en("desbloquear un fotograma"))?;
    // SAFETY: `buffer` es propio. Sin longitud actual el escritor lo lee
    // como vacio y el video sale en negro.
    unsafe { buffer.SetCurrentLength(total as u32) }
        .map_err(en("fijar el tamano de un fotograma"))?;

    // SAFETY: `MFCreateSample` no tiene precondiciones y lo demas opera
    // sobre la muestra y el buffer creados en esta funcion.
    unsafe {
        let muestra = MFCreateSample().map_err(en("crear la muestra"))?;
        muestra
            .AddBuffer(&buffer)
            .and_then(|()| muestra.SetSampleTime(inicio))
            .and_then(|()| muestra.SetSampleDuration(duracion))
            .map_err(en("montar la muestra"))?;
        Ok(muestra)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::fs;

    use windows::Win32::Media::MediaFoundation::{
        IMF2DBuffer, MF_SOURCE_READER_ALL_STREAMS,
        MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, MF_SOURCE_READER_FIRST_VIDEO_STREAM,
        MF_SOURCE_READERF_ENDOFSTREAM, MFCreateSourceReaderFromURL, MFVideoFormat_NV12,
    };
    use windows::core::{GUID, Interface};

    /// Un directorio propio por prueba: las pruebas corren en paralelo y dos
    /// que escriban el mismo `.mp4` se pisarian el fichero.
    fn temporal(etiqueta: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pixpin-record-{etiqueta}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Imagen de un solo color, opaca.
    fn lisa(ancho: u32, alto: u32, rojo: u8, verde: u8, azul: u8) -> ImagenRgba {
        ImagenRgba {
            ancho,
            alto,
            pixeles: [rojo, verde, azul, 255]
                .repeat(ancho as usize * alto as usize)
                .to_vec(),
        }
    }

    /// Mitad de arriba blanca, mitad de abajo negra. Es la imagen con la que
    /// se distingue un video del derecho de uno del reves: cualquier otra
    /// cosa (un degradado suave, un texto) sobrevive peor a la compresion.
    fn arriba_blanca(ancho: u32, alto: u32) -> ImagenRgba {
        let mut pixeles = Vec::with_capacity(ancho as usize * alto as usize * 4);
        for y in 0..alto {
            let tono = if y < alto / 2 { 255 } else { 0 };
            for _ in 0..ancho {
                pixeles.extend_from_slice(&[tono, tono, tono, 255]);
            }
        }
        ImagenRgba {
            ancho,
            alto,
            pixeles,
        }
    }

    /// Ruido reproducible, para que el codificador tenga algo que comprimir
    /// y el bitrate se note en el tamano del fichero.
    fn ruidosa(ancho: u32, alto: u32, semilla: u32) -> ImagenRgba {
        let mut estado = semilla | 1;
        let mut pixeles = Vec::with_capacity(ancho as usize * alto as usize * 4);
        for _ in 0..ancho as usize * alto as usize {
            // Xorshift: barato y siempre el mismo, que es lo que hace falta.
            estado ^= estado << 13;
            estado ^= estado >> 17;
            estado ^= estado << 5;
            pixeles.extend_from_slice(&[
                estado as u8,
                (estado >> 8) as u8,
                (estado >> 16) as u8,
                255,
            ]);
        }
        ImagenRgba {
            ancho,
            alto,
            pixeles,
        }
    }

    /// Lo que Media Foundation dice del video que acabamos de escribir.
    struct Leido {
        ancho: u32,
        alto: u32,
        fotogramas: u32,
        /// Fin del ultimo fotograma, en unidades de 100 ns.
        duracion: i64,
        /// El primer fotograma decodificado, fila a fila de arriba abajo y
        /// sin relleno.
        primero: Vec<u8>,
        /// Bytes por fila de `primero`.
        paso: usize,
    }

    /// Decodifica el fichero con `IMFSourceReader`, o sea con el mismo
    /// Windows que lo va a reproducir. Verificar que la funcion no devolvio
    /// error no prueba nada: un MP4 con los colores cambiados o boca abajo
    /// se escribe igual de bien.
    ///
    /// `bytes_por_pixel` es el ancho de una muestra en el plano que se lee:
    /// 4 para RGB32 y 1 para el plano de luminancia de NV12.
    fn leer(ruta: &Path, subtipo: GUID, bytes_por_pixel: usize) -> Leido {
        arrancar_media_foundation();
        let _com = ComDelHilo::nuevo();
        let ruta16 = a_utf16(ruta);

        // SAFETY: todo son objetos COM propios de esta funcion, con COM y
        // Media Foundation ya arrancados, y `ruta16` vive hasta el final.
        unsafe {
            let mut atributos: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut atributos, 1).unwrap();
            let atributos = atributos.unwrap();
            // Sin esto el lector solo ofrece los formatos nativos del
            // decodificador y no sabe darnos RGB32.
            atributos
                .SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)
                .unwrap();

            let lector = MFCreateSourceReaderFromURL(PCWSTR(ruta16.as_ptr()), &atributos).unwrap();
            let flujo = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
            lector
                .SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)
                .unwrap();
            lector.SetStreamSelection(flujo, true).unwrap();

            let quiero = MFCreateMediaType().unwrap();
            quiero
                .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
                .unwrap();
            quiero.SetGUID(&MF_MT_SUBTYPE, &subtipo).unwrap();
            lector.SetCurrentMediaType(flujo, None, &quiero).unwrap();

            let medida = lector
                .GetCurrentMediaType(flujo)
                .unwrap()
                .GetUINT64(&MF_MT_FRAME_SIZE)
                .unwrap();
            let ancho = (medida >> 32) as u32;
            let alto = medida as u32;

            let paso = ancho as usize * bytes_por_pixel;
            let mut leido = Leido {
                ancho,
                alto,
                fotogramas: 0,
                duracion: 0,
                primero: Vec::new(),
                paso,
            };

            loop {
                let mut banderas = 0u32;
                let mut tiempo = 0i64;
                let mut muestra: Option<IMFSample> = None;
                lector
                    .ReadSample(
                        flujo,
                        0,
                        None,
                        Some(&mut banderas),
                        Some(&mut tiempo),
                        Some(&mut muestra),
                    )
                    .unwrap();
                if banderas & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    break;
                }
                // Sin muestra pero sin fin de flujo es una marca de tiempo:
                // no es un fotograma y no se cuenta.
                let Some(muestra) = muestra else { continue };

                leido.fotogramas += 1;
                leido.duracion = leido
                    .duracion
                    .max(tiempo + muestra.GetSampleDuration().unwrap_or(0));

                if leido.primero.is_empty() {
                    leido.primero = filas_de(&muestra, alto, paso);
                }
            }
            leido
        }
    }

    /// Copia las `alto` primeras filas de la muestra a un buffer compacto.
    ///
    /// Se usa `IMF2DBuffer::Lock2D` y no `Lock` porque devuelve el puntero a
    /// la **fila de arriba** junto con el paso, que puede ser negativo si el
    /// buffer viene de abajo arriba. Leerlo con `Lock` a secas seria repetir
    /// aqui justo la ambiguedad de orden de filas que la prueba pretende
    /// medir, y la prueba pasaria con el video del reves.
    ///
    /// SAFETY: la muestra viene del lector y esta viva mientras dure la
    /// funcion; el buffer se bloquea y se desbloquea aqui mismo.
    unsafe fn filas_de(muestra: &IMFSample, alto: u32, paso: usize) -> Vec<u8> {
        // SAFETY: la muestra del lector siempre trae al menos un buffer.
        let buffer = unsafe { muestra.GetBufferByIndex(0) }.unwrap();
        let dos: IMF2DBuffer = buffer.cast().unwrap();

        let mut inicio: *mut u8 = std::ptr::null_mut();
        let mut avance: i32 = 0;
        // SAFETY: `dos` es la vista 2D del buffer que acabamos de sacar de
        // la muestra; nadie mas lo tiene bloqueado.
        unsafe { dos.Lock2D(&mut inicio, &mut avance) }.unwrap();

        let mut salida = Vec::with_capacity(paso * alto as usize);
        for y in 0..alto as isize {
            // SAFETY: `Lock2D` garantiza `alto` filas de `paso` bytes
            // validos a partir de `inicio` avanzando `avance` bytes por
            // fila, con el signo que haga falta.
            let fila =
                unsafe { std::slice::from_raw_parts(inicio.offset(y * avance as isize), paso) };
            salida.extend_from_slice(fila);
        }

        // SAFETY: empareja el `Lock2D` de arriba; ya no queda ningun slice
        // apuntando dentro.
        unsafe { dos.Unlock2D() }.unwrap();
        salida
    }

    /// Media de una fila, para no depender de un pixel suelto que la
    /// compresion haya podido mover.
    fn media(fila: &[u8]) -> u32 {
        fila.iter().map(|b| *b as u32).sum::<u32>() / fila.len() as u32
    }

    #[test]
    fn un_video_normal_tiene_las_medidas_los_fotogramas_y_la_duracion_esperados() {
        let dir = temporal("normal");
        let ruta = dir.join("normal.mp4");
        let fotogramas: Vec<ImagenRgba> = (0..6).map(|i| ruidosa(160, 120, i + 1)).collect();

        codificar_mp4(
            &fotogramas,
            OpcionesMp4 {
                por_segundo: 10,
                bitrate: None,
            },
            &ruta,
        )
        .unwrap();

        let leido = leer(&ruta, MFVideoFormat_NV12, 1);
        assert_eq!((leido.ancho, leido.alto), (160, 120));
        assert_eq!(leido.fotogramas, 6, "han de salir los seis que entraron");
        // Seis fotogramas a 10 por segundo son 0,6 s. Se deja holgura
        // porque el ultimo fotograma puede redondear su duracion.
        assert!(
            (5_500_000..=6_500_000).contains(&leido.duracion),
            "duracion rara: {} unidades de 100 ns",
            leido.duracion
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn el_video_conserva_el_orden_de_las_filas() {
        // Si el fotograma entrara boca abajo, la mitad blanca acabaria en la
        // parte de abajo del video y esta prueba fallaria. Se mira el plano
        // de luminancia de NV12 y no RGB32 porque NV12 es siempre de arriba
        // abajo: no hay convenio que interpretar.
        let dir = temporal("filas");
        let ruta = dir.join("filas.mp4");
        let fotogramas: Vec<ImagenRgba> = (0..4).map(|_| arriba_blanca(160, 120)).collect();

        codificar_mp4(&fotogramas, OpcionesMp4::default(), &ruta).unwrap();

        let leido = leer(&ruta, MFVideoFormat_NV12, 1);
        let arriba = media(&leido.primero[..leido.paso]);
        let abajo = media(&leido.primero[leido.paso * (leido.alto as usize - 1)..]);
        assert!(
            arriba > abajo + 100,
            "el video salio boca abajo: luminancia arriba {arriba}, abajo {abajo}"
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn el_rojo_del_fotograma_sigue_siendo_rojo_en_el_video() {
        // El fallo clasico: RGBA copiado tal cual a lo que Media Foundation
        // llama RGB32, que es BGRA. Sin el intercambio de canales el
        // fotograma rojo saldria azul. El fotograma es de un solo color
        // para que la prueba no dependa tambien del orden de las filas.
        let dir = temporal("rojo");
        let ruta = dir.join("rojo.mp4");
        let fotogramas: Vec<ImagenRgba> = (0..4).map(|_| lisa(160, 120, 255, 0, 0)).collect();

        codificar_mp4(&fotogramas, OpcionesMp4::default(), &ruta).unwrap();

        let leido = leer(&ruta, MFVideoFormat_RGB32, 4);
        // RGB32 es BGRA: el pixel del centro se lee azul, verde, rojo.
        let centro = (leido.alto as usize / 2) * leido.paso + (leido.ancho as usize / 2) * 4;
        let azul = leido.primero[centro] as i32;
        let verde = leido.primero[centro + 1] as i32;
        let rojo = leido.primero[centro + 2] as i32;
        assert!(
            rojo > azul + 60 && rojo > verde + 60,
            "los canales estan cambiados: rojo {rojo}, verde {verde}, azul {azul}"
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn un_ancho_impar_no_rompe_el_video() {
        // Una zona de captura de 161x121 es de lo mas normal. H.264 no sabe
        // de tamanos impares, asi que se recorta al par de abajo.
        let dir = temporal("impar");
        let ruta = dir.join("impar.mp4");
        let fotogramas: Vec<ImagenRgba> = (0..4).map(|_| arriba_blanca(161, 121)).collect();

        codificar_mp4(&fotogramas, OpcionesMp4::default(), &ruta).unwrap();

        let leido = leer(&ruta, MFVideoFormat_NV12, 1);
        assert_eq!(
            (leido.ancho, leido.alto),
            (160, 120),
            "el tamano impar debe recortarse al par de abajo"
        );
        assert_eq!(leido.fotogramas, 4);
        // Y recortar no puede volcar la imagen: la mitad blanca sigue
        // arriba aunque se haya perdido la ultima fila.
        let arriba = media(&leido.primero[..leido.paso]);
        let abajo = media(&leido.primero[leido.paso * (leido.alto as usize - 1)..]);
        assert!(arriba > abajo + 100);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn el_fichero_empieza_por_ftyp_y_trae_moov_y_mdat() {
        // Comprobacion de estructura, independiente de Media Foundation: un
        // MP4 empieza por la caja `ftyp` (tamano de 4 bytes y luego el
        // nombre), y sin `moov` no hay indice que reproducir.
        let dir = temporal("cajas");
        let ruta = dir.join("cajas.mp4");
        let fotogramas: Vec<ImagenRgba> = (0..4).map(|i| ruidosa(160, 120, i + 7)).collect();

        codificar_mp4(&fotogramas, OpcionesMp4::default(), &ruta).unwrap();

        let bytes = fs::read(&ruta).unwrap();
        assert_eq!(&bytes[4..8], b"ftyp", "un MP4 empieza por la caja ftyp");
        assert!(
            bytes.windows(4).any(|v| v == b"moov"),
            "falta el indice (moov): Finalize no llego a escribirlo"
        );
        assert!(
            bytes.windows(4).any(|v| v == b"mdat"),
            "falta el bloque de datos (mdat)"
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn el_bitrate_pedido_cambia_el_tamano_del_fichero() {
        // Verifica que la opcion llega de verdad al codificador y no se
        // queda en la estructura. Con ruido, que es lo que peor se comprime,
        // la diferencia es enorme.
        let dir = temporal("bitrate");
        let fotogramas: Vec<ImagenRgba> = (0..8).map(|i| ruidosa(320, 240, i + 3)).collect();

        let mut tamanos = Vec::new();
        for (etiqueta, bitrate) in [("bajo", 200_000u32), ("alto", 8_000_000u32)] {
            let ruta = dir.join(format!("{etiqueta}.mp4"));
            codificar_mp4(
                &fotogramas,
                OpcionesMp4 {
                    por_segundo: 10,
                    bitrate: Some(bitrate),
                },
                &ruta,
            )
            .unwrap();
            tamanos.push(fs::metadata(&ruta).unwrap().len());
        }
        assert!(
            tamanos[0] < tamanos[1],
            "el bitrate no llego al codificador: {} vs {} bytes",
            tamanos[0],
            tamanos[1]
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn una_lista_vacia_da_error_y_no_deja_fichero() {
        // Caso negativo: sin esto se crearia un MP4 de cero fotogramas, que
        // ningun reproductor abre, y el usuario creeria que grabo algo.
        let dir = temporal("vacia");
        let ruta = dir.join("vacia.mp4");

        let e = codificar_mp4(&[], OpcionesMp4::default(), &ruta).unwrap_err();
        assert!(matches!(e, ErrorMp4::SinFotogramas), "{e}");
        assert!(!ruta.exists(), "no debe quedar un fichero a medias");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn fotogramas_de_tamanos_distintos_dan_error() {
        // Un video tiene un tamano y solo uno. Si esto pasara, el segundo
        // fotograma se leeria con el paso de fila del primero y el video
        // saldria en diagonal.
        let dir = temporal("mezcla");
        let ruta = dir.join("mezcla.mp4");
        let fotogramas = vec![lisa(160, 120, 10, 20, 30), lisa(160, 60, 10, 20, 30)];

        let e = codificar_mp4(&fotogramas, OpcionesMp4::default(), &ruta).unwrap_err();
        assert!(
            matches!(e, ErrorMp4::TamanoDistinto { indice: 1, .. }),
            "{e}"
        );
        assert!(!ruta.exists());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn un_fotograma_con_menos_bytes_de_la_cuenta_da_error() {
        // Caso negativo: declara 160x120 pero trae cuatro bytes. Sin la
        // comprobacion, el bucle de copia entraria en panico al cortar la
        // primera fila.
        let dir = temporal("corto");
        let ruta = dir.join("corto.mp4");
        let fotogramas = vec![ImagenRgba {
            ancho: 160,
            alto: 120,
            pixeles: vec![0, 0, 0, 255],
        }];

        let e = codificar_mp4(&fotogramas, OpcionesMp4::default(), &ruta).unwrap_err();
        assert!(matches!(e, ErrorMp4::BufferIncoherente { .. }), "{e}");
        assert!(!ruta.exists());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn una_ruta_imposible_de_escribir_da_error_con_la_ruta() {
        let fotogramas: Vec<ImagenRgba> = (0..2).map(|_| lisa(160, 120, 0, 0, 0)).collect();
        let e = codificar_mp4(
            &fotogramas,
            OpcionesMp4::default(),
            Path::new("Z:/no/existe/grabacion.mp4"),
        )
        .unwrap_err();
        assert!(
            e.to_string().contains("grabacion.mp4"),
            "el error debe decir cual: {e}"
        );
    }

    #[test]
    fn cero_fotogramas_por_segundo_da_error() {
        // Caso negativo: con cero se dividiria por cero al repartir los
        // tiempos, y ademas no significa nada.
        let dir = temporal("ritmo");
        let ruta = dir.join("ritmo.mp4");
        let fotogramas: Vec<ImagenRgba> = (0..2).map(|_| lisa(160, 120, 0, 0, 0)).collect();

        let e = codificar_mp4(
            &fotogramas,
            OpcionesMp4 {
                por_segundo: 0,
                bitrate: None,
            },
            &ruta,
        )
        .unwrap_err();
        assert!(matches!(e, ErrorMp4::SinRitmo), "{e}");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn un_fotograma_de_un_pixel_de_ancho_da_error_en_vez_de_quedarse_en_cero() {
        // Caso negativo del redondeo a par: 1 recortado al par de abajo es
        // 0, y un video de ancho cero no existe.
        let dir = temporal("minimo");
        let ruta = dir.join("minimo.mp4");
        let fotogramas = vec![lisa(1, 120, 0, 0, 0)];

        let e = codificar_mp4(&fotogramas, OpcionesMp4::default(), &ruta).unwrap_err();
        assert!(
            matches!(e, ErrorMp4::DemasiadoPequeno { par_ancho: 0, .. }),
            "{e}"
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn las_medidas_se_redondean_al_par_de_abajo() {
        assert_eq!(medidas(&[lisa(160, 120, 0, 0, 0)]).unwrap(), (160, 120));
        assert_eq!(medidas(&[lisa(161, 121, 0, 0, 0)]).unwrap(), (160, 120));
        assert_eq!(medidas(&[lisa(3, 3, 0, 0, 0)]).unwrap(), (2, 2));
    }

    #[test]
    fn el_bitrate_por_defecto_crece_con_el_tamano_y_tiene_topes() {
        let pequeno = bitrate_por_defecto(320, 240, 30);
        let mediano = bitrate_por_defecto(1920, 1080, 30);
        let enorme = bitrate_por_defecto(7680, 4320, 60);
        assert!(
            pequeno < mediano,
            "{pequeno} deberia ser menor que {mediano}"
        );
        // Una ventanita a pocos fotogramas no puede caer por debajo del
        // suelo, o el texto se volveria ilegible.
        assert_eq!(bitrate_por_defecto(64, 64, 1), 500_000);
        // Y una pantalla 8K no puede pedir gigabits.
        assert_eq!(enorme, 20_000_000);
    }

    #[test]
    fn los_tiempos_de_los_fotogramas_no_acumulan_error_de_redondeo() {
        // A 30 fps la duracion exacta no es entera. Calculando cada
        // comienzo desde cero, el fotograma 30 cae en el segundo exacto; si
        // se fueran sumando duraciones redondeadas, llegaria diez unidades
        // antes y el video se adelantaria un cuadro cada pocos segundos.
        assert_eq!(comienzo(0, 30), 0);
        assert_eq!(comienzo(30, 30), UNIDADES_POR_SEGUNDO);
        assert_eq!(comienzo(300, 30), 10 * UNIDADES_POR_SEGUNDO);
    }
}
