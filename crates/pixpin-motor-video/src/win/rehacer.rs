//! **Rehacer un MP4 sin volver a codificarlo** (28-oct-2026, grabar sin
//! perder nada): de un MP4 fragmentado cortado (la aplicacion murio a mitad
//! de la reunion) a un MP4 normal con su indice, copiando cada muestra tal
//! cual. Es un `IMFSourceReader` que lee las pistas en su formato de origen
//! (H.264/HEVC y AAC) y un `IMFSinkWriter` que las recibe en ese mismo
//! formato: sin codificador por medio, una hora de video se copia en lo que
//! tarda el disco.
//!
//! El lector se para en el ultimo fragmento entero; lo que hubiera a medio
//! escribir detras se pierde (como mucho un fragmento, unos segundos).

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Media::MediaFoundation::{
    IMFAttributes, IMFSample, MF_SOURCE_READER_ANY_STREAM, MF_SOURCE_READERF_ENDOFSTREAM,
    MF_SOURCE_READERF_ERROR, MF_SOURCE_READERF_STREAMTICK, MF_TRANSCODE_CONTAINERTYPE,
    MFCreateAttributes, MFCreateSinkWriterFromURL, MFCreateSourceReaderFromURL,
    MFTranscodeContainerType_MPEG4,
};
use windows::core::PCWSTR;

use super::com::{ComDelHilo, arrancar_media_foundation, en};
use crate::ErrorMotor;

fn ancha(ruta: &Path) -> Vec<u16> {
    ruta.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Copia `origen` a `destino` (MP4 normal). Devuelve las muestras copiadas.
pub(crate) fn rehacer_mp4(origen: &Path, destino: &Path) -> Result<u64, ErrorMotor> {
    let _com = ComDelHilo::nuevo();
    arrancar_media_foundation();
    let de = ancha(origen);
    let a = ancha(destino);
    // SAFETY: llamadas COM sobre objetos creados en este hilo; los unicos
    // punteros crudos son las rutas terminadas en cero (vivas hasta el final
    // de la funcion) y las salidas de `ReadSample`, variables locales.
    unsafe {
        let lector = MFCreateSourceReaderFromURL(PCWSTR(de.as_ptr()), None::<&IMFAttributes>)
            .map_err(en("abrir el video a rehacer"))?;
        let mut atributos: Option<IMFAttributes> = None;
        MFCreateAttributes(&mut atributos, 1).map_err(en("crear los atributos del MP4"))?;
        let atributos = atributos.ok_or(ErrorMotor::Sistema {
            paso: "crear los atributos del MP4",
            detalle: String::new(),
            codigo: 0x8000_4005_u32 as i32,
        })?;
        atributos
            .SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_MPEG4)
            .map_err(en("elegir el contenedor MPEG-4"))?;
        let escritor = MFCreateSinkWriterFromURL(PCWSTR(a.as_ptr()), None, &atributos)
            .map_err(en("crear el MP4 rehecho"))?;

        // Cada pista del origen, en su formato de origen, a una del destino.
        let mut pistas: Vec<(u32, u32, bool)> = Vec::new();
        for i in 0..16u32 {
            let Ok(tipo) = lector.GetNativeMediaType(i, 0) else {
                break;
            };
            lector
                .SetStreamSelection(i, true)
                .map_err(en("elegir una pista del video a rehacer"))?;
            let j = escritor
                .AddStream(&tipo)
                .map_err(en("anadir una pista al MP4 rehecho"))?;
            escritor
                .SetInputMediaType(j, &tipo, None)
                .map_err(en("copiar el formato de una pista"))?;
            pistas.push((i, j, false));
        }
        if pistas.is_empty() {
            return Err(ErrorMotor::SinFotogramas);
        }
        escritor
            .BeginWriting()
            .map_err(en("empezar a escribir el MP4 rehecho"))?;
        let mut copiadas = 0u64;
        loop {
            let mut indice = 0u32;
            let mut banderas = 0u32;
            let mut tiempo = 0i64;
            let mut muestra: Option<IMFSample> = None;
            let leido = lector.ReadSample(
                MF_SOURCE_READER_ANY_STREAM.0 as u32,
                0,
                Some(&mut indice),
                Some(&mut banderas),
                Some(&mut tiempo),
                Some(&mut muestra),
            );
            // Un fragmento roto al final (lo que se corto): hasta ahi.
            if leido.is_err() || banderas & MF_SOURCE_READERF_ERROR.0 as u32 != 0 {
                tracing::info!(copiadas, "el video a rehacer acaba en un fragmento roto");
                break;
            }
            let Some(p) = pistas.iter_mut().find(|p| p.0 == indice) else {
                continue;
            };
            if let Some(m) = muestra {
                escritor
                    .WriteSample(p.1, &m)
                    .map_err(en("copiar una muestra al MP4 rehecho"))?;
                copiadas += 1;
            } else if banderas & MF_SOURCE_READERF_STREAMTICK.0 as u32 != 0 {
                let _ = escritor.SendStreamTick(p.1, tiempo);
            }
            if banderas & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                p.2 = true;
                if pistas.iter().all(|p| p.2) {
                    break;
                }
            }
        }
        if copiadas == 0 {
            return Err(ErrorMotor::SinFotogramas);
        }
        escritor
            .Finalize()
            .map_err(en("cerrar el MP4 rehecho (Finalize)"))?;
        Ok(copiadas)
    }
}
