//! **El MP4**: un `IMFSinkWriter` con la pista de video (H.264 o HEVC, de
//! la grafica o por software) y la de sonido (AAC), las dos fechadas con el
//! mismo reloj (`crate::reloj`).
//!
//! - **Con la grafica**, el escritor recibe el `IMFDXGIDeviceManager` del
//!   dispositivo (`MF_SINK_WRITER_D3D_MANAGER`) y permiso para usar el
//!   hardware (`MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS`): asi elige el MFT
//!   de la grafica y le pasa las texturas NV12 sin copiarlas. Al acabar de
//!   montarlo **se comprueba** que lo elegido es de verdad de hardware
//!   ([`es_de_hardware`]); si no, se dice y se pasa al siguiente de la
//!   cadena, en vez de mandar texturas a un codificador de software que las
//!   bajaria por su cuenta sin que nadie lo sepa.
//! - **La grafica desde memoria** (`cadena::Camino::GraficaEnMemoria`): el
//!   mismo permiso de hardware pero sin gestor del dispositivo. El escritor
//!   carga el MFT de hardware que haya (aunque este en otra grafica) y le da
//!   el NV12 en memoria; tambien se comprueba que es de hardware. Los MFT de
//!   hardware son asincronos: el escritor los desbloquea
//!   (`MF_TRANSFORM_ASYNC_UNLOCK`) y lleva su bucle de eventos, que es lo que
//!   Cap tuvo que escribir a mano por usar el MFT sin escritor.
//! - **Sin latencia baja** (`MF_LOW_LATENCY` no se pone): lo que se quiere es
//!   el fichero mas pequeno, no el fotograma cuanto antes.
//! - **Calidad por bits con techo** (`PeakConstrainedVBR`): una media que
//!   fija lo que pesa una hora y un pico para que el texto de una
//!   diapositiva nueva salga nitido. Sin fotogramas B (los de la grafica de
//!   hace diez anos no siempre los hacen bien) y un fotograma clave cada dos
//!   segundos de ritmo maximo. Se pasan como parametros de codificacion de
//!   `SetInputMediaType`, que el escritor le da al codificador por su
//!   `ICodecAPI` antes de fijar los tipos; si el codificador no los admite,
//!   se prueba con menos y por ultimo sin ninguno ([`Parametros`]).
//! - **Sin la regulacion del escritor**: con ella `WriteSample` duerme hasta
//!   que el codificador se pone al dia, y como el sonido y el video se dan
//!   desde el mismo hilo una pista dormida esperando a la otra seria un
//!   cuelgue. El tope lo ponen las colas del motor (`crate::cola`).

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Media::MediaFoundation::{
    CODECAPI_AVEncCommonMaxBitRate, CODECAPI_AVEncCommonMeanBitRate,
    CODECAPI_AVEncCommonQualityVsSpeed, CODECAPI_AVEncCommonRateControlMode,
    CODECAPI_AVEncMPVDefaultBPictureCount, CODECAPI_AVEncMPVGOPSize, IMFAttributes,
    IMFDXGIDeviceManager, IMFMediaBuffer, IMFMediaType, IMFSample, IMFSinkWriter, IMFSinkWriterEx,
    MF_MT_AAC_PAYLOAD_TYPE, MF_MT_AUDIO_AVG_BYTES_PER_SECOND, MF_MT_AUDIO_BITS_PER_SAMPLE,
    MF_MT_AUDIO_BLOCK_ALIGNMENT, MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND,
    MF_MT_AVG_BITRATE, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE,
    MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_MPEG2_PROFILE, MF_MT_PIXEL_ASPECT_RATIO,
    MF_MT_SUBTYPE, MF_MT_TRANSFER_FUNCTION, MF_MT_VIDEO_NOMINAL_RANGE, MF_MT_VIDEO_PRIMARIES,
    MF_MT_YUV_MATRIX, MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SINK_WRITER_D3D_MANAGER,
    MF_SINK_WRITER_DISABLE_THROTTLING, MF_SINK_WRITER_STATISTICS, MF_TRANSCODE_CONTAINERTYPE,
    MFAudioFormat_AAC, MFAudioFormat_PCM, MFCreateAttributes, MFCreateMediaType,
    MFCreateMemoryBuffer, MFCreateSample, MFCreateSinkWriterFromURL, MFMediaType_Audio,
    MFMediaType_Video, MFNominalRange_16_235, MFT_CATEGORY_VIDEO_ENCODER,
    MFT_ENUM_HARDWARE_URL_Attribute, MFTranscodeContainerType_FMPEG4,
    MFTranscodeContainerType_MPEG4, MFVideoFormat_H264, MFVideoFormat_HEVC, MFVideoFormat_NV12,
    MFVideoInterlace_Progressive, MFVideoPrimaries_BT709, MFVideoTransFunc_709,
    MFVideoTransferMatrix_BT709, eAVEncCommonRateControlMode_PeakConstrainedVBR,
    eAVEncH264VProfile_High, eAVEncH265VProfile_Main_420_8,
};
use windows::core::{GUID, Interface, PCWSTR};

use super::com::en;
use crate::audio::{BYTES_POR_SEGUNDO_AAC, FormatoAudio};
use crate::perfil::{Ajustes, bits_por_segundo, grupo_de_imagenes};
use crate::reloj::tiempo_de_muestras;
use crate::{ErrorMotor, Formato};

/// Que se le pide al escritor.
pub(crate) struct Plan<'a> {
    /// Con el codificador de la grafica (con texturas si hay `gestor`, o
    /// leyendo el NV12 de memoria si no).
    pub hardware: bool,
    pub formato: Formato,
    /// El gestor del dispositivo, solo con la grafica.
    pub gestor: Option<&'a IMFDXGIDeviceManager>,
    pub medidas: (u32, u32),
    pub ajustes: Ajustes,
    pub audio: Option<FormatoAudio>,
    pub fragmentado: bool,
}

/// Cuanto se le dice al codificador, de mas a menos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Parametros {
    /// Bits con techo, grupo de imagenes, sin B, calidad contra velocidad.
    Completos,
    /// Solo el grupo de imagenes y sin B.
    Basicos,
    /// Nada: lo que el codificador haga con la media del tipo de salida.
    Ninguno,
}

pub(crate) struct Escritor {
    escritor: IMFSinkWriter,
    video: u32,
    audio: Option<(u32, FormatoAudio)>,
    /// Si salio fragmentado.
    pub fragmentado: bool,
}

impl Escritor {
    /// **Abre el MP4** en `parcial` con lo que pide el plan, probando de mas
    /// a menos (contenedor fragmentado o normal, parametros del codificador)
    /// hasta que uno arranque. Cada intento fallido borra lo que dejo.
    pub fn abrir(parcial: &Path, plan: &Plan) -> Result<Escritor, ErrorMotor> {
        let contenedores: &[(GUID, bool)] = if plan.fragmentado {
            &[
                (MFTranscodeContainerType_FMPEG4, true),
                (MFTranscodeContainerType_MPEG4, false),
            ]
        } else {
            &[(MFTranscodeContainerType_MPEG4, false)]
        };
        let mut ultimo = None;
        for &(contenedor, fragmentado) in contenedores {
            for parametros in [
                Parametros::Completos,
                Parametros::Basicos,
                Parametros::Ninguno,
            ] {
                match abrir_uno(parcial, plan, contenedor, parametros) {
                    Ok((escritor, video, audio)) => {
                        tracing::info!(
                            hardware = plan.hardware,
                            texturas = plan.gestor.is_some(),
                            ?parametros,
                            fragmentado,
                            "escritor del video abierto"
                        );
                        return Ok(Escritor {
                            escritor,
                            video,
                            audio: audio.zip(plan.audio),
                            fragmentado,
                        });
                    }
                    Err(e) => {
                        let _ = std::fs::remove_file(parcial);
                        tracing::debug!(?e, ?parametros, fragmentado, "un intento del escritor");
                        // Un codificador de software donde se pidio la
                        // grafica no mejora con otros parametros.
                        let sin_grafica = matches!(e, ErrorMotor::SinCodificador(_));
                        ultimo = Some(e);
                        if sin_grafica {
                            break;
                        }
                    }
                }
            }
        }
        Err(ultimo.unwrap_or(ErrorMotor::SinCodificador("sin intentos".into())))
    }

    /// Mete un fotograma ya fechado.
    pub fn video(&self, muestra: &IMFSample) -> Result<(), ErrorMotor> {
        // SAFETY: escritor vivo y escribiendo; `video` es el indice que dio
        // `AddStream`; la muestra esta montada y fechada.
        unsafe { self.escritor.WriteSample(self.video, muestra) }
            .map_err(en("escribir un fotograma"))
    }

    /// Mete un bloque de sonido intercalado que empieza en `posicion` (por
    /// canal). Sin pista de sonido no hace nada.
    pub fn audio(&self, muestras: &[i16], posicion: u64) -> Result<(), ErrorMotor> {
        let Some((flujo, formato)) = self.audio else {
            return Ok(());
        };
        let marcos = formato.por_canal(muestras.len());
        if marcos == 0 {
            return Ok(());
        }
        let utiles = marcos as usize * usize::from(formato.canales);
        let bytes = utiles * 2;
        let (buffer, destino) = buffer_de(bytes)?;
        // SAFETY: `destino` apunta a `bytes` bytes recien reservados y
        // bloqueados por `buffer_de`; las muestras son `utiles` i16 (2 bytes
        // cada una) y Windows es siempre little-endian, que es el orden del
        // PCM. Origen y destino no se solapan.
        unsafe {
            std::ptr::copy_nonoverlapping(muestras.as_ptr().cast::<u8>(), destino, bytes);
        }
        cerrar_buffer(&buffer, bytes)?;
        let inicio = tiempo_de_muestras(posicion, formato.muestreo);
        let fin = tiempo_de_muestras(posicion + marcos, formato.muestreo);
        let muestra = muestra_de(&buffer, inicio, (fin - inicio).max(1))?;
        // SAFETY: como en `video`, con el flujo del sonido.
        unsafe { self.escritor.WriteSample(flujo, &muestra) }
            .map_err(en("escribir un trozo de sonido"))
    }

    /// Fotogramas que ha recibido el escritor y aun no ha codificado. Si no
    /// lo sabe decir, cero: mejor no saltar nada que saltarlo todo.
    pub fn en_cola(&self) -> u64 {
        let mut s = MF_SINK_WRITER_STATISTICS {
            cb: std::mem::size_of::<MF_SINK_WRITER_STATISTICS>() as u32,
            ..Default::default()
        };
        // SAFETY: `s` es local, con su `cb` puesto como pide la API.
        match unsafe { self.escritor.GetStatistics(self.video, &mut s) } {
            Ok(()) => s.qwNumSamplesReceived.saturating_sub(s.qwNumSamplesEncoded),
            Err(_) => 0,
        }
    }

    /// **Cierra el MP4**: vacia los codificadores y escribe el indice.
    pub fn terminar(self) -> Result<(), ErrorMotor> {
        // SAFETY: escritor vivo y escribiendo desde `BeginWriting`.
        unsafe { self.escritor.Finalize() }.map_err(en("cerrar el MP4 (Finalize)"))
    }
}

type Abierto = (IMFSinkWriter, u32, Option<u32>);

fn abrir_uno(
    parcial: &Path,
    plan: &Plan,
    contenedor: GUID,
    parametros: Parametros,
) -> Result<Abierto, ErrorMotor> {
    let grafica = plan.hardware;
    let atributos = atributos(5)?;
    // SAFETY: objeto propio; claves y valores son constantes de Media
    // Foundation, y el gestor (si lo hay) esta vivo y ya tiene su
    // dispositivo (`ResetDevice`).
    unsafe {
        atributos
            .SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &contenedor)
            .map_err(en("elegir el contenedor"))?;
        atributos
            .SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)
            .map_err(en("desactivar la regulacion"))?;
        atributos
            .SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, u32::from(grafica))
            .map_err(en("elegir el codificador"))?;
        if let (true, Some(gestor)) = (grafica, plan.gestor) {
            atributos
                .SetUnknown(&MF_SINK_WRITER_D3D_MANAGER, gestor)
                .map_err(en("darle la GPU al escritor"))?;
        }
    }
    let ruta: Vec<u16> = parcial.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: COM y Media Foundation arrancados en este hilo (`motor`);
    // `ruta` termina en cero y vive mas que la llamada.
    let escritor = unsafe { MFCreateSinkWriterFromURL(PCWSTR(ruta.as_ptr()), None, &atributos) }
        .map_err(|e| ErrorMotor::Sistema {
            paso: "crear el fichero del video",
            detalle: format!("{}: {e}", parcial.display()),
            codigo: e.code().0,
        })?;

    let (ancho, alto) = plan.medidas;
    let (media, pico) = bits_por_segundo(&plan.ajustes, ancho, alto);
    let fps = plan.ajustes.por_segundo.max(1);
    let salida = tipo_de_video(plan.formato, ancho, alto, fps)?;
    // SAFETY: tipo recien creado; claves constantes y enteros.
    unsafe {
        salida
            .SetUINT32(&MF_MT_AVG_BITRATE, media)
            .and_then(|()| {
                salida.SetUINT32(
                    &MF_MT_MPEG2_PROFILE,
                    match plan.formato {
                        Formato::H264 => eAVEncH264VProfile_High.0 as u32,
                        Formato::Hevc => eAVEncH265VProfile_Main_420_8.0 as u32,
                    },
                )
            })
            .map_err(en("describir el video de salida"))?;
    }
    // SAFETY: escritor y tipo vivos. Es la llamada que busca el codificador.
    let video = unsafe { escritor.AddStream(&salida) }.map_err(en(
        "anadir la pista de video (sin codificador para ese tipo)",
    ))?;

    let entrada = tipo_de_video(Formato::H264, ancho, alto, fps)?;
    // SAFETY: tipo recien creado; claves constantes y enteros.
    unsafe {
        entrada
            .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)
            .map_err(en("describir el NV12"))?;
        if plan.gestor.is_none() {
            // Sin texturas (software, o la grafica leyendo de memoria) llega
            // en memoria, de arriba abajo y sin relleno.
            entrada
                .SetUINT32(&MF_MT_DEFAULT_STRIDE, ancho)
                .map_err(en("describir el paso del NV12"))?;
        }
    }
    let codificacion = parametros_de(parametros, &plan.ajustes, media, pico)?;
    // SAFETY: escritor, tipo y parametros vivos durante la llamada.
    unsafe {
        match &codificacion {
            Some(p) => escritor.SetInputMediaType(video, &entrada, p),
            None => escritor.SetInputMediaType(video, &entrada, None),
        }
    }
    .map_err(en("conectar el NV12 con el codificador"))?;

    let audio = match plan.audio {
        Some(f) => Some(anadir_aac(&escritor, f)?),
        None => None,
    };
    // SAFETY: escritor con todos sus tipos puestos.
    unsafe { escritor.BeginWriting() }.map_err(en("empezar a escribir el MP4"))?;
    if grafica && !es_de_hardware(&escritor, video) {
        return Err(ErrorMotor::SinCodificador(
            "el escritor no eligio el codificador de la grafica".into(),
        ));
    }
    Ok((escritor, video, audio))
}

/// Un tipo de video con lo que comparten la entrada y la salida: tamano,
/// ritmo, progresivo, pixel cuadrado y el color BT.709 de estudio que deja
/// el procesador de video (`conversor`). Declararlo en los dos lados es lo
/// que hace que el reproductor no lo lea como BT.601 (grises lavados).
fn tipo_de_video(
    formato: Formato,
    ancho: u32,
    alto: u32,
    fps: u32,
) -> Result<IMFMediaType, ErrorMotor> {
    let subtipo = match formato {
        Formato::H264 => MFVideoFormat_H264,
        Formato::Hevc => MFVideoFormat_HEVC,
    };
    // SAFETY: `MFCreateMediaType` no tiene precondiciones; el tipo es propio
    // y solo recibe claves y GUID constantes y enteros.
    unsafe {
        let t = MFCreateMediaType().map_err(en("crear un tipo de video"))?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .and_then(|()| t.SetGUID(&MF_MT_SUBTYPE, &subtipo))
            .and_then(|()| {
                t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
            })
            .and_then(|()| t.SetUINT64(&MF_MT_FRAME_SIZE, par_u64(ancho, alto)))
            .and_then(|()| t.SetUINT64(&MF_MT_FRAME_RATE, par_u64(fps, 1)))
            // Pixeles cuadrados: sin esto hay reproductores que estiran.
            .and_then(|()| t.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, par_u64(1, 1)))
            .and_then(|()| t.SetUINT32(&MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT709.0 as u32))
            .and_then(|()| t.SetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_16_235.0 as u32))
            .and_then(|()| t.SetUINT32(&MF_MT_VIDEO_PRIMARIES, MFVideoPrimaries_BT709.0 as u32))
            .and_then(|()| t.SetUINT32(&MF_MT_TRANSFER_FUNCTION, MFVideoTransFunc_709.0 as u32))
            .map_err(en("describir un tipo de video"))?;
        Ok(t)
    }
}

/// Los parametros de codificacion de cada nivel.
fn parametros_de(
    p: Parametros,
    a: &Ajustes,
    media: u32,
    pico: u32,
) -> Result<Option<IMFAttributes>, ErrorMotor> {
    if p == Parametros::Ninguno {
        return Ok(None);
    }
    let attr = atributos(6)?;
    // SAFETY: objeto propio; claves `CODECAPI_*` constantes con valores
    // UINT32, que es como las espera el escritor para pasarlas al
    // `ICodecAPI` del codificador.
    unsafe {
        attr.SetUINT32(&CODECAPI_AVEncMPVGOPSize, grupo_de_imagenes(a))
            .and_then(|()| attr.SetUINT32(&CODECAPI_AVEncMPVDefaultBPictureCount, 0))
            .map_err(en("poner el grupo de imagenes"))?;
        if p == Parametros::Completos {
            attr.SetUINT32(
                &CODECAPI_AVEncCommonRateControlMode,
                eAVEncCommonRateControlMode_PeakConstrainedVBR.0 as u32,
            )
            .and_then(|()| attr.SetUINT32(&CODECAPI_AVEncCommonMeanBitRate, media))
            .and_then(|()| attr.SetUINT32(&CODECAPI_AVEncCommonMaxBitRate, pico))
            .and_then(|()| {
                attr.SetUINT32(&CODECAPI_AVEncCommonQualityVsSpeed, a.calidad_vs_velocidad)
            })
            .map_err(en("poner los bits del codificador"))?;
        }
    }
    Ok(Some(attr))
}

/// **Si el codificador que eligio el escritor es de la grafica**: los MFT
/// de hardware llevan `MFT_ENUM_HARDWARE_URL_Attribute`. Si el escritor no
/// deja preguntar, se da por bueno (es lo que se le pidio).
fn es_de_hardware(escritor: &IMFSinkWriter, flujo: u32) -> bool {
    let Ok(ex) = escritor.cast::<IMFSinkWriterEx>() else {
        return true;
    };
    for i in 0..8 {
        let mut categoria = GUID::zeroed();
        let mut mft = None;
        // SAFETY: salidas locales; el indice que no existe da error y corta.
        if unsafe { ex.GetTransformForStream(flujo, i, Some(&mut categoria), &mut mft) }.is_err() {
            break;
        }
        if categoria == MFT_CATEGORY_VIDEO_ENCODER
            && let Some(mft) = mft
        {
            // SAFETY: MFT vivo; solo se consulta su almacen de atributos.
            return unsafe {
                mft.GetAttributes()
                    .and_then(|a| a.GetStringLength(&MFT_ENUM_HARDWARE_URL_Attribute))
                    .is_ok()
            };
        }
    }
    false
}

/// La pista de sonido: sale AAC crudo (lo que va dentro de un MP4) y entra
/// PCM de 16 bits.
fn anadir_aac(escritor: &IMFSinkWriter, f: FormatoAudio) -> Result<u32, ErrorMotor> {
    let canales = u32::from(f.canales);
    // SAFETY: tipos recien creados aqui; solo reciben claves y GUID
    // constantes y enteros; el escritor esta vivo.
    unsafe {
        let salida = MFCreateMediaType().map_err(en("crear el tipo del sonido"))?;
        salida
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
            .and_then(|()| salida.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC))
            .and_then(|()| salida.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16))
            .and_then(|()| salida.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, f.muestreo))
            .and_then(|()| salida.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, canales))
            .and_then(|()| {
                salida.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, BYTES_POR_SEGUNDO_AAC)
            })
            // Cero: AAC crudo. Uno seria ADTS, que el contenedor rechaza.
            .and_then(|()| salida.SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0))
            .map_err(en("describir el sonido AAC"))?;
        let flujo = escritor
            .AddStream(&salida)
            .map_err(en("anadir la pista de sonido (sin codificador AAC)"))?;
        let entrada = MFCreateMediaType().map_err(en("crear el tipo del PCM"))?;
        entrada
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
            .and_then(|()| entrada.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM))
            .and_then(|()| entrada.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16))
            .and_then(|()| entrada.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, f.muestreo))
            .and_then(|()| entrada.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, canales))
            .and_then(|()| entrada.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, f.bytes_por_marco()))
            .and_then(|()| {
                entrada.SetUINT32(
                    &MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
                    f.muestreo * f.bytes_por_marco(),
                )
            })
            .map_err(en("describir el PCM"))?;
        escritor
            .SetInputMediaType(flujo, &entrada, None)
            .map_err(en("conectar el PCM con el codificador AAC"))?;
        Ok(flujo)
    }
}

fn atributos(cuantos: u32) -> Result<IMFAttributes, ErrorMotor> {
    let mut a: Option<IMFAttributes> = None;
    // SAFETY: `a` es local; la funcion deposita ahi el objeto.
    unsafe { MFCreateAttributes(&mut a, cuantos) }.map_err(en("crear unos atributos"))?;
    a.ok_or(ErrorMotor::Sistema {
        paso: "crear unos atributos",
        detalle: "sin objeto".into(),
        codigo: 0,
    })
}

/// Empaqueta dos u32 en el u64 de Media Foundation, el primero arriba.
fn par_u64(primero: u32, segundo: u32) -> u64 {
    (u64::from(primero) << 32) | u64::from(segundo)
}

/// Un buffer en memoria de `bytes`, **ya bloqueado**: devuelve el puntero
/// donde escribir. Se cierra con [`cerrar_buffer`].
pub(crate) fn buffer_de(bytes: usize) -> Result<(IMFMediaBuffer, *mut u8), ErrorMotor> {
    // SAFETY: `MFCreateMemoryBuffer` solo reserva memoria.
    let buffer = unsafe { MFCreateMemoryBuffer(bytes as u32) }
        .map_err(en("reservar el buffer de una muestra"))?;
    let mut destino: *mut u8 = std::ptr::null_mut();
    // SAFETY: buffer propio y recien creado; `Lock` deja un puntero a por lo
    // menos `bytes` bytes, valido hasta el `Unlock` de `cerrar_buffer`.
    unsafe { buffer.Lock(&mut destino, None, None) }.map_err(en("bloquear una muestra"))?;
    Ok((buffer, destino))
}

/// Desbloquea un buffer de [`buffer_de`] con `bytes` utiles.
pub(crate) fn cerrar_buffer(buffer: &IMFMediaBuffer, bytes: usize) -> Result<(), ErrorMotor> {
    // SAFETY: empareja el `Lock` de `buffer_de`; la longitud es la escrita,
    // dentro de lo reservado.
    unsafe {
        buffer.Unlock().map_err(en("desbloquear una muestra"))?;
        buffer
            .SetCurrentLength(bytes as u32)
            .map_err(en("medir una muestra"))
    }
}

/// Una muestra con `buffer`, fechada.
pub(crate) fn muestra_de(
    buffer: &IMFMediaBuffer,
    inicio: i64,
    duracion: i64,
) -> Result<IMFSample, ErrorMotor> {
    // SAFETY: `MFCreateSample` no tiene precondiciones; lo demas opera sobre
    // la muestra recien creada y un buffer vivo.
    unsafe {
        let m = MFCreateSample().map_err(en("crear una muestra"))?;
        m.AddBuffer(buffer)
            .and_then(|()| m.SetSampleTime(inicio))
            .and_then(|()| m.SetSampleDuration(duracion))
            .map_err(en("montar una muestra"))?;
        Ok(m)
    }
}
