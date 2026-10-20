//! **Lo grabado bajito, mas alto al oirlo** (20-oct-2026). El usuario: «se
//! graban con un volumen muy bajo, casi no se escucha: aumentalos con
//! software a la hora de reproducirlos».
//!
//! El motor de Media Foundation que reproduce ([`crate::Salida`]) no sube de
//! 1.0 (`SetVolume`), asi que el realce se hace antes: se descodifica el
//! audio, se mide su pico de verdad (el 99,9 % de las muestras, para que un
//! chasquido suelto no mande), y si esta bajo se escribe una copia con la
//! ganancia justa (hasta x16, +24 dB) y un limitador suave que no deja
//! saturar. Se oye esa copia, que dura exactamente lo mismo: la barra, saltar
//! a un trozo de la transcripcion y la velocidad siguen igual.
//!
//! La copia se guarda en la carpeta temporal por fichero (ruta, tamano y
//! fecha): la segunda vez sale al momento. Lo que ya suena fuerte no se toca
//! (se apunta, para no volver a medirlo). `PIXPIN_AUDIO_SIN_REALCE` lo apaga.

use std::hash::{Hash, Hasher};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use windows::Win32::Media::MediaFoundation::{
    IMFSample, MF_MT_AUDIO_BITS_PER_SAMPLE, MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_MAJOR_TYPE,
    MF_MT_SUBTYPE, MF_SOURCE_READER_ALL_STREAMS, MF_SOURCE_READER_FIRST_AUDIO_STREAM, MF_SOURCE_READERF_ENDOFSTREAM,
    MFAudioFormat_PCM, MFCreateMediaType, MFCreateSourceReaderFromURL, MFMediaType_Audio,
};
use windows::core::PCWSTR;

/// A donde se sube el pico: -1 dBFS.
const OBJETIVO: f32 = 0.89;
/// Lo maximo que se sube: x16 (+24 dB), lo mismo que el realce al grabar.
const GANANCIA_MAXIMA: f32 = 16.0;
/// Menos que esto no merece una copia: ya se oye bien.
const GANANCIA_MINIMA: f32 = 1.3;
/// Desde aqui el limitador redondea en vez de dejar saturar.
const UMBRAL: f32 = 0.85;

/// **El fichero que hay que oir**: la copia realzada si `ruta` suena bajo,
/// o `ruta` tal cual (ya fuerte, o sin poder leerlo: que lo intente el motor).
pub fn para_oir(ruta: &Path) -> PathBuf {
    if std::env::var_os("PIXPIN_AUDIO_SIN_REALCE").is_some() {
        return ruta.to_path_buf();
    }
    let Some(copia) = ruta_de_la_copia(ruta) else {
        return ruta.to_path_buf();
    };
    if copia.is_file() {
        return copia;
    }
    let ya_fuerte = copia.with_extension("fuerte");
    if ya_fuerte.is_file() {
        return ruta.to_path_buf();
    }
    let t = std::time::Instant::now();
    let Some((mut muestras, hercios, canales)) = descodificar(ruta) else {
        return ruta.to_path_buf();
    };
    let g = ganancia_para(&muestras);
    if g < GANANCIA_MINIMA {
        let _ = std::fs::create_dir_all(copia.parent().unwrap_or(Path::new(".")));
        let _ = std::fs::write(&ya_fuerte, b"");
        return ruta.to_path_buf();
    }
    aplicar(&mut muestras, g);
    let _ = std::fs::create_dir_all(copia.parent().unwrap_or(Path::new(".")));
    let parte = copia.with_extension("parte");
    if std::fs::write(&parte, wav(&muestras, hercios, canales)).is_err() || std::fs::rename(&parte, &copia).is_err() {
        let _ = std::fs::remove_file(&parte);
        return ruta.to_path_buf();
    }
    tracing::info!(ganancia = g, ms = t.elapsed().as_millis() as u64, "audio bajo: se oye realzado");
    copia
}

/// Donde va la copia de `ruta`: por su ruta, su tamano y su fecha (si el
/// fichero cambia, otra copia).
fn ruta_de_la_copia(ruta: &Path) -> Option<PathBuf> {
    let meta = std::fs::metadata(ruta).ok()?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    ruta.hash(&mut h);
    meta.len().hash(&mut h);
    meta.modified().ok()?.hash(&mut h);
    Some(std::env::temp_dir().join("pixpin-audio-alto").join(format!("{:016x}.wav", h.finish())))
}

/// **La ganancia para que el pico de verdad quede en el objetivo**: el pico
/// es el 99,9 % de las muestras (un golpe suelto no frena el realce), con
/// tope; 1 si es silencio.
pub fn ganancia_para(muestras: &[f32]) -> f32 {
    if muestras.is_empty() {
        return 1.0;
    }
    // Un histograma en vez de ordenar: un audio largo tiene millones.
    let mut cubos = [0u32; 1024];
    for m in muestras {
        let i = ((m.abs().min(1.0)) * 1023.0) as usize;
        cubos[i] += 1;
    }
    let corte = (muestras.len() as f64 * 0.999) as u64;
    let mut acumulado = 0u64;
    let mut pico = 1.0;
    for (i, c) in cubos.iter().enumerate() {
        acumulado += *c as u64;
        if acumulado >= corte {
            pico = (i as f32 + 1.0) / 1024.0;
            break;
        }
    }
    // Silencio (o casi): nada que subir.
    if pico < 0.002 {
        return 1.0;
    }
    (OBJETIVO / pico).clamp(1.0, GANANCIA_MAXIMA)
}

/// Sube cada muestra `g` veces; lo que pasa del umbral se redondea (tanh)
/// en vez de recortarse: no se oye roto.
pub fn aplicar(muestras: &mut [f32], g: f32) {
    for m in muestras.iter_mut() {
        let y = *m * g;
        let a = y.abs();
        *m = if a <= UMBRAL { y } else { y.signum() * (UMBRAL + (1.0 - UMBRAL) * ((a - UMBRAL) / (1.0 - UMBRAL)).tanh()) };
    }
}

/// Un WAV PCM de 16 bits con esas muestras (entrelazadas por canal).
fn wav(muestras: &[f32], hercios: u32, canales: u16) -> Vec<u8> {
    let datos = (muestras.len() * 2) as u32;
    let mut v = Vec::with_capacity(44 + datos as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + datos).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&canales.to_le_bytes());
    v.extend_from_slice(&hercios.to_le_bytes());
    v.extend_from_slice(&(hercios * canales as u32 * 2).to_le_bytes());
    v.extend_from_slice(&(canales * 2).to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&datos.to_le_bytes());
    for m in muestras {
        v.extend_from_slice(&((m.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
    }
    v
}

/// Descodifica `ruta` entera a muestras de -1 a 1 con sus hercios y canales
/// de siempre (solo se pide PCM de 16 bits; lo demas, el del fichero).
fn descodificar(ruta: &Path) -> Option<(Vec<f32>, u32, u16)> {
    crate::salida::arrancar_media_foundation();
    let ancha: Vec<u16> = ruta.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    // SAFETY: `ancha` es UTF-16 terminada en cero y vive durante la llamada.
    let lector = unsafe { MFCreateSourceReaderFromURL(PCWSTR(ancha.as_ptr()), None) }.ok()?;
    let todas = MF_SOURCE_READER_ALL_STREAMS.0 as u32;
    let audio = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
    // SAFETY: el lector esta vivo; el tipo se crea aqui y sus atributos son
    // constantes del sistema. Solo la pista de audio, en PCM de 16 bits.
    let (hercios, canales) = unsafe {
        let tipo = MFCreateMediaType().ok()?;
        tipo.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).ok()?;
        tipo.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM).ok()?;
        tipo.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16).ok()?;
        lector.SetStreamSelection(todas, false).ok()?;
        lector.SetStreamSelection(audio, true).ok()?;
        lector.SetCurrentMediaType(audio, None, &tipo).ok()?;
        let actual = lector.GetCurrentMediaType(audio).ok()?;
        (actual.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND).ok()?, actual.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS).ok()? as u16)
    };
    if hercios == 0 || canales == 0 {
        return None;
    }
    let mut muestras: Vec<f32> = Vec::new();
    loop {
        let mut banderas: u32 = 0;
        let mut trozo: Option<IMFSample> = None;
        // SAFETY: punteros a variables locales vivas; lectura sincrona.
        unsafe { lector.ReadSample(audio, 0, None, Some(&mut banderas), None, Some(&mut trozo)) }.ok()?;
        if banderas & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
            break;
        }
        let Some(trozo) = trozo else { continue };
        // SAFETY: la muestra esta viva; el buffer se lee solo entre `Lock` y
        // `Unlock`, que se llama siempre (no hay `?` entre medias).
        unsafe {
            let buffer = trozo.ConvertToContiguousBuffer().ok()?;
            let mut datos: *mut u8 = std::ptr::null_mut();
            let mut largo: u32 = 0;
            if buffer.Lock(&mut datos, None, Some(&mut largo)).is_err() {
                continue;
            }
            if !datos.is_null() {
                let cuantas = largo as usize / 2;
                muestras.reserve(cuantas);
                for i in 0..cuantas {
                    let m = i16::from_le_bytes([*datos.add(i * 2), *datos.add(i * 2 + 1)]);
                    muestras.push(m as f32 / 32768.0);
                }
            }
            let _ = buffer.Unlock();
        }
    }
    (!muestras.is_empty()).then_some((muestras, hercios, canales))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn seno(amplitud: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| amplitud * (i as f32 * 0.05).sin()).collect()
    }

    #[test]
    fn lo_bajito_sube_hasta_el_objetivo_y_lo_fuerte_no() {
        let bajo = seno(0.1, 48_000);
        let g = ganancia_para(&bajo);
        assert!((g - 8.9).abs() < 0.3, "{g}");
        let fuerte = seno(0.8, 48_000);
        assert!(ganancia_para(&fuerte) < GANANCIA_MINIMA);
    }

    #[test]
    fn un_chasquido_suelto_no_frena_el_realce() {
        let mut v = seno(0.05, 100_000);
        v[500] = 1.0;
        assert!(ganancia_para(&v) > 10.0, "el golpe suelto no cuenta como pico");
    }

    #[test]
    fn el_silencio_no_se_sube_y_hay_tope() {
        assert_eq!(ganancia_para(&vec![0.0; 1000]), 1.0);
        assert_eq!(ganancia_para(&seno(0.01, 48_000)), GANANCIA_MAXIMA);
    }

    #[test]
    fn el_limitador_no_deja_saturar() {
        let mut v = vec![0.5, -0.5, 0.05];
        aplicar(&mut v, 4.0);
        assert!(v[0] < 1.0 && v[0] > UMBRAL);
        assert!(v[1] > -1.0 && v[1] < -UMBRAL);
        assert!((v[2] - 0.2).abs() < 1e-6, "lo que no llega al umbral sube tal cual");
    }

    #[test]
    fn el_wav_dice_su_formato() {
        let w = wav(&[0.0, 0.5, -0.5, 1.0], 48_000, 2);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(u16::from_le_bytes([w[22], w[23]]), 2);
        assert_eq!(u32::from_le_bytes([w[24], w[25], w[26], w[27]]), 48_000);
        assert_eq!(w.len(), 44 + 8);
    }
}

#[cfg(test)]
mod a_mano {
    /// Con un audio de verdad: `PIXPIN_AUDIO_PRUEBA=<fichero> cargo test -p
    /// pixpin-audio realzar_uno_de_verdad -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita un audio de verdad"]
    fn realzar_uno_de_verdad() {
        let Some(f) = std::env::var_os("PIXPIN_AUDIO_PRUEBA") else { return };
        let ruta = std::path::PathBuf::from(f);
        let t = std::time::Instant::now();
        let oir = super::para_oir(&ruta);
        println!("{} -> {} en {:?}", ruta.display(), oir.display(), t.elapsed());
        if let Some((m, h, c)) = super::descodificar(&oir) {
            println!("ganancia que quedaria: {:.2} ({h} Hz, {c} canales, {} muestras)", super::ganancia_para(&m), m.len());
        }
    }
}
