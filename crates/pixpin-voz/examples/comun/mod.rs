//! Lo que comparten los prototipos sin interfaz de `pixpin-voz`: leer el
//! audio a PCM de 16 kHz mono y encontrar el modelo y el runtime.
//!
//! En Windows la app decodifica con Media Foundation (`pcm::decodificar`);
//! aqui no hay ventana ni sistema que valga: un WAV PCM de 16 bits se lee a
//! mano (cualquier muestreo y canales, a 16 kHz con el mismo
//! `en_vivo::a_16k` que la escucha en vivo) y lo demas (MP3, OGG, M4A…) se
//! le pide a `ffmpeg`, si esta en el `PATH`.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use pixpin_voz::HERCIOS;

/// Los argumentos: los sueltos en orden y los `--clave valor` o `--bandera`.
pub struct Argumentos {
    pub sueltos: Vec<String>,
    pares: Vec<(String, Option<String>)>,
}

impl Argumentos {
    /// `banderas`: las `--x` que no llevan valor detras.
    pub fn leer(banderas: &[&str]) -> Argumentos {
        let mut sueltos = Vec::new();
        let mut pares = Vec::new();
        let mut it = std::env::args().skip(1);
        while let Some(a) = it.next() {
            if let Some(clave) = a.strip_prefix("--") {
                if banderas.contains(&clave) {
                    pares.push((clave.to_string(), None));
                } else {
                    pares.push((clave.to_string(), it.next()));
                }
            } else {
                sueltos.push(a);
            }
        }
        Argumentos { sueltos, pares }
    }

    pub fn valor(&self, clave: &str) -> Option<&str> {
        self.pares
            .iter()
            .find(|(c, _)| c == clave)
            .and_then(|(_, v)| v.as_deref())
    }

    pub fn hay(&self, clave: &str) -> bool {
        self.pares.iter().any(|(c, _)| c == clave)
    }
}

/// **El audio entero a PCM de 16 bits, mono, 16 kHz.**
pub fn leer_audio(ruta: &Path) -> Result<Vec<i16>, String> {
    let es_wav = ruta
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("wav"));
    if es_wav && let Some(m) = leer_wav(ruta)? {
        return Ok(m);
    }
    con_ffmpeg(ruta)
}

/// Un WAV PCM de 16 bits (`fmt ` con formato 1): `None` si es otra cosa
/// (flotante, ADPCM…), que entonces va a `ffmpeg`.
fn leer_wav(ruta: &Path) -> Result<Option<Vec<i16>>, String> {
    let b = std::fs::read(ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
    if b.len() < 12 || &b[..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Ok(None);
    }
    let u16_en = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_en = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let (mut hz, mut canales, mut bits, mut formato) = (0u32, 0u16, 0u16, 0u16);
    let mut i = 12;
    while i + 8 <= b.len() {
        let largo = u32_en(i + 4) as usize;
        let cuerpo = i + 8;
        if &b[i..i + 4] == b"fmt " && cuerpo + 16 <= b.len() {
            formato = u16_en(cuerpo);
            canales = u16_en(cuerpo + 2);
            hz = u32_en(cuerpo + 4);
            bits = u16_en(cuerpo + 14);
        } else if &b[i..i + 4] == b"data" {
            if formato != 1 || bits != 16 || canales == 0 || hz == 0 {
                return Ok(None);
            }
            let datos = &b[cuerpo..(cuerpo + largo).min(b.len())];
            let c = usize::from(canales);
            let mono: Vec<i16> = datos
                .chunks_exact(2 * c)
                .map(|marco| {
                    let s: i32 = marco
                        .chunks_exact(2)
                        .map(|m| i32::from(i16::from_le_bytes([m[0], m[1]])))
                        .sum();
                    (s / c as i32) as i16
                })
                .collect();
            return Ok(Some(pixpin_voz::en_vivo::a_16k(&mono, hz)));
        }
        i = cuerpo + largo + (largo & 1);
    }
    Ok(None)
}

/// Lo que no es un WAV PCM: `ffmpeg -i x -ac 1 -ar 16000 -f s16le -`.
fn con_ffmpeg(ruta: &Path) -> Result<Vec<i16>, String> {
    let salida = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-i"])
        .arg(ruta)
        .args(["-ac", "1", "-ar", &HERCIOS.to_string(), "-f", "s16le", "-"])
        .output()
        .map_err(|e| {
            format!(
                "{} no es un WAV PCM de 16 bits y no se pudo lanzar ffmpeg ({e}); \
                 conviertelo antes: ffmpeg -i entrada -ac 1 -ar 16000 salida.wav",
                ruta.display()
            )
        })?;
    if !salida.status.success() {
        return Err(format!(
            "ffmpeg no pudo con {}: {}",
            ruta.display(),
            String::from_utf8_lossy(&salida.stderr).trim()
        ));
    }
    Ok(salida
        .stdout
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect())
}

/// `<raiz>` con `whisper/base/` dentro (`--raiz` o `PIXPIN_VOZ_RAIZ`) y la
/// carpeta de `libonnxruntime.so` (`--ort` o `PIXPIN_ORT`; si no se da,
/// tambien vale `<raiz>/whisper/`, como en la app).
pub fn raiz_y_ort(a: &Argumentos) -> Result<(PathBuf, Option<PathBuf>), String> {
    let raiz = a
        .valor("raiz")
        .map(String::from)
        .or_else(|| std::env::var("PIXPIN_VOZ_RAIZ").ok())
        .ok_or("falta --raiz <carpeta con whisper/base/> (o PIXPIN_VOZ_RAIZ)")?;
    let ort = a
        .valor("ort")
        .map(String::from)
        .or_else(|| std::env::var("PIXPIN_ORT").ok())
        .map(PathBuf::from);
    Ok((PathBuf::from(raiz), ort))
}

/// `m:ss.d`, para leer los tiempos con una decima.
pub fn mmss(ms: i64) -> String {
    let ms = ms.max(0);
    format!("{}:{:02}.{}", ms / 60_000, ms / 1000 % 60, ms / 100 % 10)
}
