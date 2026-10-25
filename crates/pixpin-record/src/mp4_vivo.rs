//! **MP4 escrito mientras se graba** (25-oct-2026, grabar la reunion
//! entera: «que tambien pueda grabar el VIDEO mas el audio del equipo y el
//! microfono»).
//!
//! [`crate::codificar_mp4`] recibe todos los fotogramas en memoria: una hora
//! de reunion a 720p serian cientos de gigas. Aqui cada fotograma se le da
//! al `IMFSinkWriter` en cuanto se captura y se olvida, y el sonido entra por
//! una segunda pista AAC al mismo tiempo. Lo que se queda en memoria es un
//! fotograma, no la grabacion.
//!
//! Lo mismo que en `mp4` (entrada RGB32 con el conversor de color del
//! sistema, codificador por software) y ademas:
//!
//! - **Se escribe en `<destino>.parte`** y se renombra en [`Codificador::terminar`],
//!   como la nota de voz (`pixpin-audio::entrada::a_medias`): un MP4 sin su
//!   `moov` no lo abre nadie, y no puede quedar con el nombre bueno.
//! - **Sin la regulacion del escritor**, pero con un tope propio: con la
//!   regulacion encendida `WriteSample` duerme hasta que el codificador se
//!   pone al dia, y como el sonido y el video se dan desde el mismo hilo, una
//!   pista dormida esperando a la otra seria un cuelgue. En su lugar se mira
//!   cuantos fotogramas esperan al codificador y, si son demasiados (un
//!   equipo lento), se salta el fotograma: el video baja de ritmo un rato en
//!   vez de llenar la memoria.
//! - **El tiempo lo pone quien llama** para el video, y para el sonido se
//!   cuenta en muestras. Quien graba fecha los fotogramas con el mismo reloj
//!   de muestras ([`crate::escala::tiempo_de_muestras`]) y asi las dos pistas
//!   no se separan, ni siquiera tras una pausa.

use std::path::{Path, PathBuf};

use windows::Win32::Media::MediaFoundation::{
    IMFAttributes, IMFSample, IMFSinkWriter, MF_MT_AAC_PAYLOAD_TYPE,
    MF_MT_AUDIO_AVG_BYTES_PER_SECOND, MF_MT_AUDIO_BITS_PER_SAMPLE, MF_MT_AUDIO_BLOCK_ALIGNMENT,
    MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
    MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SINK_WRITER_DISABLE_THROTTLING,
    MF_SINK_WRITER_STATISTICS, MF_TRANSCODE_CONTAINERTYPE, MFAudioFormat_AAC, MFAudioFormat_PCM,
    MFCreateAttributes, MFCreateMediaType, MFCreateMemoryBuffer, MFCreateSample,
    MFCreateSinkWriterFromURL, MFMediaType_Audio, MFTranscodeContainerType_MPEG4,
};
use windows::core::PCWSTR;

use crate::escala::{muestreo_aac_valido, saltar_fotograma, tiempo_de_muestras};
use crate::mp4::{
    ComDelHilo, ErrorMp4, a_utf16, anadir_flujo_h264, arrancar_media_foundation,
    bitrate_por_defecto, en, falta_codificador, fijar_entrada_rgb32,
};

/// Bytes por segundo del AAC: los mismos 96 kbps que la nota de voz
/// (`pixpin-audio::entrada::BYTES_POR_SEGUNDO_AAC`). El codificador de
/// Windows solo admite 12 000, 16 000, 20 000 o 24 000.
const BYTES_POR_SEGUNDO_AAC: u32 = 12_000;

/// Como se graba.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcionesVivo {
    /// Medidas del video, ya pares (ver [`crate::escala::medida_de_salida`]).
    pub ancho: u32,
    pub alto: u32,
    pub por_segundo: u32,
    /// Bits por segundo del video; `None`, la cuenta de siempre.
    pub bitrate: Option<u32>,
    /// Muestras por segundo del sonido mono de 16 bits (44 100 o 48 000), o
    /// `None` para un video mudo.
    pub muestreo_audio: Option<u32>,
}

/// Un MP4 abierto, al que se le van dando fotogramas y sonido.
///
/// No es `Send`: se crea, se usa y se termina en el mismo hilo, que es lo
/// que pide COM. Soltarlo sin [`Codificador::terminar`] tira lo escrito.
pub struct Codificador {
    escritor: Option<IMFSinkWriter>,
    video: u32,
    audio: Option<u32>,
    opciones: OpcionesVivo,
    destino: PathBuf,
    parcial: PathBuf,
    fotogramas: u64,
    saltados_seguidos: u32,
    saltados: u64,
    muestras_audio: u64,
    /// El ultimo: el COM del hilo se suelta cuando ya no queda nada de Media
    /// Foundation vivo (los campos se sueltan en el orden en que estan).
    _com: ComDelHilo,
}

/// `<destino>.parte`, al lado del destino y con su nombre entero.
fn a_medias(destino: &Path) -> PathBuf {
    let mut s = destino.as_os_str().to_os_string();
    s.push(".parte");
    PathBuf::from(s)
}

impl Codificador {
    /// Abre el MP4 (de momento en `<destino>.parte`) con su pista H.264 y,
    /// si se pide, la de sonido AAC.
    pub fn nuevo(destino: &Path, opciones: OpcionesVivo) -> Result<Codificador, ErrorMp4> {
        if opciones.por_segundo == 0 {
            return Err(ErrorMp4::SinRitmo);
        }
        let (ancho, alto) = (opciones.ancho & !1, opciones.alto & !1);
        if ancho < 2 || alto < 2 {
            return Err(ErrorMp4::DemasiadoPequeno {
                ancho: opciones.ancho,
                alto: opciones.alto,
                par_ancho: ancho,
                par_alto: alto,
            });
        }
        if let Some(m) = opciones.muestreo_audio
            && !muestreo_aac_valido(m)
        {
            return Err(ErrorMp4::MuestreoNoAdmitido { muestreo: m });
        }
        let opciones = OpcionesVivo {
            ancho,
            alto,
            ..opciones
        };
        let com = ComDelHilo::nuevo();
        arrancar_media_foundation();
        if let Some(padre) = destino.parent()
            && !padre.as_os_str().is_empty()
        {
            let _ = std::fs::create_dir_all(padre);
        }
        let parcial = a_medias(destino);
        match abrir(&parcial, &opciones) {
            Ok((escritor, video, audio)) => Ok(Codificador {
                escritor: Some(escritor),
                video,
                audio,
                opciones,
                destino: destino.to_path_buf(),
                parcial,
                fotogramas: 0,
                saltados_seguidos: 0,
                saltados: 0,
                muestras_audio: 0,
                _com: com,
            }),
            Err(e) => {
                let _ = std::fs::remove_file(&parcial);
                Err(e)
            }
        }
    }

    /// Medidas del video, ya pares.
    pub fn medidas(&self) -> (u32, u32) {
        (self.opciones.ancho, self.opciones.alto)
    }

    /// Fotogramas escritos y saltados (por ir el codificador atrasado).
    pub fn cuenta(&self) -> (u64, u64) {
        (self.fotogramas, self.saltados)
    }

    /// Mete un fotograma BGRA de [`Codificador::medidas`], de arriba abajo
    /// y sin relleno, que empieza en `inicio` y dura `duracion` (100 ns).
    ///
    /// Devuelve `false` si se salto por ir el codificador atrasado.
    pub fn poner_fotograma(
        &mut self,
        bgra: &[u8],
        inicio: i64,
        duracion: i64,
    ) -> Result<bool, ErrorMp4> {
        let (ancho, alto) = self.medidas();
        let total = ancho as usize * alto as usize * 4;
        if bgra.len() != total {
            return Err(ErrorMp4::BufferIncoherente {
                indice: self.fotogramas as usize,
                ancho,
                alto,
                tiene: bgra.len(),
                espera: total,
            });
        }
        let escritor = self.escritor.as_ref().ok_or(ErrorMp4::SinFotogramas)?;
        if self.fotogramas > 0
            && saltar_fotograma(
                self.fotogramas,
                en_cola(escritor, self.video),
                self.saltados_seguidos,
            )
        {
            self.saltados_seguidos += 1;
            self.saltados += 1;
            return Ok(false);
        }
        let muestra = muestra_de_bytes(bgra, inicio, duracion.max(1))?;
        // SAFETY: escritor vivo y en escritura; `video` es el indice que dio
        // `AddStream` en este escritor; la muestra se acaba de crear.
        unsafe { escritor.WriteSample(self.video, &muestra) }
            .map_err(en("escribir un fotograma"))?;
        self.fotogramas += 1;
        self.saltados_seguidos = 0;
        Ok(true)
    }

    /// Mete un bloque de sonido mono de 16 bits, seguido del anterior. Sin
    /// pista de sonido no hace nada.
    pub fn poner_audio(&mut self, muestras: &[i16]) -> Result<(), ErrorMp4> {
        let (Some(flujo), Some(muestreo)) = (self.audio, self.opciones.muestreo_audio) else {
            return Ok(());
        };
        if muestras.is_empty() {
            return Ok(());
        }
        let escritor = self.escritor.as_ref().ok_or(ErrorMp4::SinFotogramas)?;
        let bytes: Vec<u8> = muestras.iter().flat_map(|m| m.to_le_bytes()).collect();
        let inicio = tiempo_de_muestras(self.muestras_audio, muestreo);
        let fin = tiempo_de_muestras(self.muestras_audio + muestras.len() as u64, muestreo);
        let muestra = muestra_de_bytes(&bytes, inicio, (fin - inicio).max(1))?;
        // SAFETY: como en `poner_fotograma`, con el flujo del sonido.
        unsafe { escritor.WriteSample(flujo, &muestra) }
            .map_err(en("escribir un trozo de sonido"))?;
        self.muestras_audio += muestras.len() as u64;
        Ok(())
    }

    /// **Cierra el MP4** (escribe su indice) y lo pone en su nombre bueno.
    /// Sin ningun fotograma no hay video: se tira y se dice.
    pub fn terminar(mut self) -> Result<PathBuf, ErrorMp4> {
        let escritor = self.escritor.take().ok_or(ErrorMp4::SinFotogramas)?;
        if self.fotogramas == 0 {
            drop(escritor);
            let _ = std::fs::remove_file(&self.parcial);
            return Err(ErrorMp4::SinFotogramas);
        }
        // SAFETY: escritor vivo y en escritura desde `BeginWriting`;
        // `Finalize` es quien escribe la caja `moov`.
        let hecho = unsafe { escritor.Finalize() }.map_err(en("cerrar el MP4 (Finalize)"));
        drop(escritor);
        if let Err(e) = hecho {
            let _ = std::fs::remove_file(&self.parcial);
            return Err(e);
        }
        // El MP4 ya esta cerrado y vale: si no se deja renombrar (otro
        // volumen, un antivirus que lo tiene abierto), se copia; y si ni
        // eso, se queda en `.parte` con su contenido, no se borra.
        if std::fs::rename(&self.parcial, &self.destino).is_err() {
            let copiado = std::fs::copy(&self.parcial, &self.destino);
            if let Err(fuente) = copiado {
                let _ = std::fs::remove_file(&self.destino);
                return Err(ErrorMp4::Renombrar {
                    ruta: self.destino.clone(),
                    fuente,
                });
            }
            let _ = std::fs::remove_file(&self.parcial);
        }
        Ok(self.destino.clone())
    }
}

impl Drop for Codificador {
    fn drop(&mut self) {
        // Soltado sin terminar: lo escrito no tiene indice y no vale.
        if let Some(escritor) = self.escritor.take() {
            drop(escritor);
            let _ = std::fs::remove_file(&self.parcial);
        }
    }
}

/// Cuantos fotogramas ha recibido el escritor y aun no ha codificado. Si no
/// lo sabe decir, cero: mejor no saltar nada que saltarlo todo.
fn en_cola(escritor: &IMFSinkWriter, flujo: u32) -> u64 {
    let mut s = MF_SINK_WRITER_STATISTICS {
        cb: std::mem::size_of::<MF_SINK_WRITER_STATISTICS>() as u32,
        ..Default::default()
    };
    // SAFETY: `s` es local, con su `cb` puesto como pide la API, y vive
    // durante la llamada; el escritor esta vivo.
    match unsafe { escritor.GetStatistics(flujo, &mut s) } {
        Ok(()) => s.qwNumSamplesReceived.saturating_sub(s.qwNumSamplesEncoded),
        Err(_) => 0,
    }
}

/// Una muestra de Media Foundation con una copia de `bytes`.
fn muestra_de_bytes(bytes: &[u8], inicio: i64, duracion: i64) -> Result<IMFSample, ErrorMp4> {
    // SAFETY: `MFCreateMemoryBuffer` solo reserva memoria.
    let buffer = unsafe { MFCreateMemoryBuffer(bytes.len() as u32) }
        .map_err(en("reservar el buffer de una muestra"))?;
    let mut destino: *mut u8 = std::ptr::null_mut();
    // SAFETY: buffer propio y recien creado; `Lock` deja un puntero a por lo
    // menos `bytes.len()` bytes, valido hasta el `Unlock` de abajo.
    unsafe { buffer.Lock(&mut destino, None, None) }.map_err(en("bloquear una muestra"))?;
    // SAFETY: origen y destino no se solapan (uno es de Rust, el otro de
    // Media Foundation) y los dos tienen `bytes.len()` bytes; se copia como
    // bytes, sin exigir alineamiento.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), destino, bytes.len()) };
    // SAFETY: empareja el `Lock` de arriba.
    unsafe { buffer.Unlock() }.map_err(en("desbloquear una muestra"))?;
    // SAFETY: la longitud es justo lo copiado, dentro de lo reservado.
    unsafe { buffer.SetCurrentLength(bytes.len() as u32) }.map_err(en("medir una muestra"))?;
    // SAFETY: `MFCreateSample` no tiene precondiciones; lo demas opera sobre
    // la muestra y el buffer de esta funcion.
    unsafe {
        let muestra = MFCreateSample().map_err(en("crear una muestra"))?;
        muestra
            .AddBuffer(&buffer)
            .and_then(|()| muestra.SetSampleTime(inicio))
            .and_then(|()| muestra.SetSampleDuration(duracion))
            .map_err(en("montar una muestra"))?;
        Ok(muestra)
    }
}

/// Crea el escritor con sus pistas y empieza a escribir.
fn abrir(
    parcial: &Path,
    opciones: &OpcionesVivo,
) -> Result<(IMFSinkWriter, u32, Option<u32>), ErrorMp4> {
    let ruta = a_utf16(parcial);
    let mut atributos: Option<IMFAttributes> = None;
    // SAFETY: `atributos` es local; la funcion deposita ahi el objeto.
    unsafe { MFCreateAttributes(&mut atributos, 3) }.map_err(en("crear los atributos"))?;
    let atributos = atributos.ok_or_else(|| ErrorMp4::MediaFoundation {
        paso: "crear los atributos",
        fuente: windows::core::Error::empty(),
    })?;
    // SAFETY: objeto propio; claves y valores son constantes de Media
    // Foundation.
    unsafe {
        atributos
            .SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_MPEG4)
            .map_err(en("elegir el contenedor MP4"))?;
        // Ver la cabecera del modulo: sin regulacion, con tope propio.
        atributos
            .SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)
            .map_err(en("desactivar la regulacion"))?;
        atributos
            .SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 0)
            .map_err(en("fijar el codificador por software"))?;
    }
    // SAFETY: COM y Media Foundation arrancados (`Codificador::nuevo`);
    // `ruta` termina en cero y vive mas que la llamada.
    let escritor = unsafe { MFCreateSinkWriterFromURL(PCWSTR(ruta.as_ptr()), None, &atributos) }
        .map_err(|fuente| ErrorMp4::Escritura {
            ruta: parcial.to_path_buf(),
            fuente,
        })?;

    let (ancho, alto, fps) = (opciones.ancho, opciones.alto, opciones.por_segundo);
    let bitrate = opciones
        .bitrate
        .unwrap_or_else(|| bitrate_por_defecto(ancho, alto, fps));
    let video = anadir_flujo_h264(&escritor, ancho, alto, fps, bitrate)?;
    fijar_entrada_rgb32(&escritor, video, ancho, alto, fps)?;
    let audio = match opciones.muestreo_audio {
        Some(muestreo) => Some(anadir_flujo_aac(&escritor, muestreo)?),
        None => None,
    };
    // SAFETY: escritor propio con todos sus tipos puestos.
    unsafe { escritor.BeginWriting() }.map_err(en("empezar a escribir el MP4"))?;
    Ok((escritor, video, audio))
}

/// La pista de sonido: sale AAC mono crudo (lo que va dentro de un MP4) y
/// entra PCM mono de 16 bits, como en la nota de voz.
fn anadir_flujo_aac(escritor: &IMFSinkWriter, muestreo: u32) -> Result<u32, ErrorMp4> {
    let como_aac = |fuente: windows::core::Error, paso: &'static str| {
        if falta_codificador(&fuente) {
            ErrorMp4::SinCodificadorAac
        } else {
            ErrorMp4::MediaFoundation { paso, fuente }
        }
    };
    // SAFETY: tipos recien creados aqui; solo reciben claves y GUID
    // constantes y enteros; el escritor esta vivo.
    unsafe {
        let salida = MFCreateMediaType().map_err(en("crear el tipo del sonido"))?;
        salida
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
            .and_then(|()| salida.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC))
            .and_then(|()| salida.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16))
            .and_then(|()| salida.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, muestreo))
            .and_then(|()| salida.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 1))
            .and_then(|()| {
                salida.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, BYTES_POR_SEGUNDO_AAC)
            })
            // Cero: AAC crudo. Uno seria ADTS, que el contenedor rechaza.
            .and_then(|()| salida.SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0))
            .map_err(en("describir el sonido AAC"))?;
        let flujo = escritor
            .AddStream(&salida)
            .map_err(|f| como_aac(f, "anadir la pista de sonido"))?;

        let entrada = MFCreateMediaType().map_err(en("crear el tipo del PCM"))?;
        entrada
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
            .and_then(|()| entrada.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM))
            .and_then(|()| entrada.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16))
            .and_then(|()| entrada.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, muestreo))
            .and_then(|()| entrada.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 1))
            // Mono de 16 bits: dos bytes por marco.
            .and_then(|()| entrada.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 2))
            .and_then(|()| entrada.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, muestreo * 2))
            .map_err(en("describir el PCM"))?;
        escritor
            .SetInputMediaType(flujo, &entrada, None)
            .map_err(|f| como_aac(f, "encajar el PCM con el codificador AAC"))?;
        Ok(flujo)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::escala::comienzo;

    /// Necesita Windows con Media Foundation: se ejecuta con
    /// `cargo test -p pixpin-record -- --ignored`.
    #[test]
    #[ignore = "necesita Media Foundation con H.264 y AAC"]
    fn un_video_en_vivo_con_sonido_se_cierra_y_tiene_las_dos_pistas() {
        let dir = std::env::temp_dir().join("pixpin-record-vivo");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("vivo.mp4");
        let mut c = Codificador::nuevo(
            &ruta,
            OpcionesVivo {
                ancho: 160,
                alto: 120,
                por_segundo: 10,
                bitrate: None,
                muestreo_audio: Some(48_000),
            },
        )
        .unwrap();
        let fotograma = vec![128u8; 160 * 120 * 4];
        for i in 0..10u64 {
            c.poner_audio(&vec![0i16; 4_800]).unwrap();
            c.poner_fotograma(
                &fotograma,
                comienzo(i, 10),
                comienzo(i + 1, 10) - comienzo(i, 10),
            )
            .unwrap();
        }
        let hecho = c.terminar().unwrap();
        assert_eq!(hecho, ruta);
        assert!(!a_medias(&ruta).exists());
        let bytes = std::fs::read(&ruta).unwrap();
        assert!(bytes.windows(4).any(|v| v == b"moov"));
        assert_eq!(bytes.windows(4).filter(|v| *v == b"trak").count(), 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn el_fichero_a_medias_va_al_lado_del_destino() {
        assert_eq!(
            a_medias(Path::new("C:/p/clase.mp4")),
            Path::new("C:/p/clase.mp4.parte")
        );
    }
}
