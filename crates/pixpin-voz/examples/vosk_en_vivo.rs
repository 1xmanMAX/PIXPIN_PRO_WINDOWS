//! **Prototipo sin interfaz de Vosk en vivo** (`vosk_vivo::Corriente`, la
//! escucha de la reunion con Vosk): una grabacion entra en bloques de 10 ms
//! como los de la mezcla (48 kHz estereo) y sale lo que se va oyendo y cada
//! frase cerrada, como en la app. El hermano de `frases_en_vivo` (Whisper).
//!
//! ```text
//! cargo run -p pixpin-voz --release --example vosk_en_vivo -- \
//!     reunion.wav --vosk <libvosk.so|libvosk.dll> --modelo <vosk-model-small-es-0.42> \
//!     [--tiempo-real] [--idioma es] [--salida frases.txt] [--quien Nombre]
//! ```
//!
//! - Por omision, **tiempo real simulado**: se mete el audio sin esperar y
//!   se mide lo que tarda cada bloque; el reloj cuenta cuando habria salido
//!   cada cosa con un solo hilo que oye y reconoce (como el de la app).
//! - Con `--tiempo-real`, los bloques entran al ritmo del reloj.
//!
//! Por frase: donde empieza y donde se dejo de hablar, cuando salio el
//! primer parcial (desde que empezo a hablar) y el **retraso** (se dejo de
//! hablar → frase cerrada). Al final, medias, p50 y maximos y el RTF.
//! `--salida` escribe el guion oido de los prototipos de Jev
//! (`[inicio fin_voz texto] Quien: frase`), como `frases_en_vivo`.

mod comun;

use std::path::Path;
use std::time::{Duration, Instant};

use comun::{Argumentos, leer_audio, mmss};
use pixpin_voz::HERCIOS;
use pixpin_voz::vosk_vivo::{Corriente, ModeloVosk, Oido, Vosk};

const HZ_MEZCLA: u32 = 48_000;
const CANALES: u16 = 2;
const BLOQUE_MS: u64 = 10;

fn main() {
    if let Err(e) = correr() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

struct Medida {
    n: u64,
    ms: i64,
    fin_voz_ms: i64,
    /// Cuando salio el primer parcial de esta frase, si empezo tras un
    /// silencio (ms de grabacion).
    primer_parcial: Option<i64>,
    /// Cuando salio la frase (ms de grabacion).
    sale: i64,
    texto: String,
}

fn correr() -> Result<(), String> {
    let a = Argumentos::leer(&["tiempo-real"]);
    let [audio] = a.sueltos.as_slice() else {
        return Err(
            "uso: vosk_en_vivo <audio.wav> --vosk <libvosk> --modelo <carpeta> \
                    [--tiempo-real] [--idioma es] [--salida frases.txt] [--quien Nombre]"
                .into(),
        );
    };
    let dll = a.valor("vosk").ok_or("falta --vosk <libvosk.so|dll>")?;
    let carpeta = a.valor("modelo").ok_or("falta --modelo <carpeta>")?;
    let idioma = a.valor("idioma").unwrap_or("es").to_string();
    let muestras = leer_audio(Path::new(audio))?;
    let dura_ms = muestras.len() as i64 * 1000 / i64::from(HERCIOS);

    let t0 = Instant::now();
    let vosk = Vosk::cargar(Path::new(dll)).map_err(|e| e.to_string())?;
    let modelo = ModeloVosk::cargar(&vosk, Path::new(carpeta)).map_err(|e| e.to_string())?;
    let rec = modelo.reconocedor().map_err(|e| e.to_string())?;
    let carga = t0.elapsed().as_secs_f32();
    let mut c = Corriente::nueva(rec, &idioma);

    let tiempo_real = a.hay("tiempo-real");
    let por_bloque = (HERCIOS as u64 * BLOQUE_MS / 1000) as usize;
    let factor = (HZ_MEZCLA / HERCIOS) as usize;
    let mut posicion = 0u64;
    // El reloj simulado: ms de grabacion en que el hilo queda libre.
    let mut libre = 0i64;
    let mut ocupado = Duration::ZERO;
    let mut medidas: Vec<Medida> = Vec::new();
    // El primer parcial tras un silencio (lo de una frase partida por larga
    // ya venia a la vista).
    let mut primer_parcial: Option<i64> = None;
    let mut tras_silencio = true;
    let mut parciales = 0u64;
    let empiezo = Instant::now();
    let mut apuntar = |oido: Vec<Oido>, ahora: i64, medidas: &mut Vec<Medida>| {
        for o in oido {
            match o {
                Oido::Parcial(p) if !p.is_empty() => {
                    parciales += 1;
                    if std::mem::take(&mut tras_silencio) {
                        primer_parcial = Some(ahora);
                    }
                }
                Oido::Parcial(_) => tras_silencio = true,
                Oido::Frase(f) => medidas.push(Medida {
                    n: f.n,
                    ms: f.ms,
                    fin_voz_ms: f.fin_voz_ms,
                    primer_parcial: primer_parcial.take(),
                    sale: ahora,
                    texto: f.texto,
                }),
            }
        }
    };
    for trozo in muestras.chunks(por_bloque) {
        let b: Vec<i16> = trozo
            .iter()
            .flat_map(|&m| std::iter::repeat_n(m, factor * usize::from(CANALES)))
            .collect();
        let fin = ((posicion + (trozo.len() * factor) as u64) * 1000 / u64::from(HZ_MEZCLA)) as i64;
        if tiempo_real {
            let cuando = empiezo + Duration::from_millis(fin as u64);
            if let Some(falta) = cuando.checked_duration_since(Instant::now()) {
                std::thread::sleep(falta);
            }
        }
        let t = Instant::now();
        let oido = c.meter(posicion, HZ_MEZCLA, CANALES, &b);
        let tarda = t.elapsed();
        ocupado += tarda;
        let ahora = if tiempo_real {
            empiezo.elapsed().as_millis() as i64
        } else {
            libre = libre.max(fin) + tarda.as_millis() as i64;
            libre
        };
        apuntar(oido, ahora, &mut medidas);
        posicion += (trozo.len() * factor) as u64;
    }
    let ahora = libre.max(dura_ms);
    apuntar(c.acabar(), ahora, &mut medidas);

    println!(
        "# {audio} | {:.1} s | {} | carga {carga:.2} s | ocupado {:.1} s (RTF {:.3}) | parciales {parciales} | descartadas {}",
        dura_ms as f32 / 1000.0,
        if tiempo_real {
            "tiempo real"
        } else {
            "tiempo real simulado"
        },
        ocupado.as_secs_f32(),
        ocupado.as_millis() as f64 / dura_ms.max(1) as f64,
        c.descartadas
    );
    println!("#  n  inicio  fin voz  1er parcial  retraso  texto");
    for m in &medidas {
        println!(
            "{:>4} {:>7} {:>8} {:>12} {:>8.2}  {}",
            m.n,
            mmss(m.ms),
            mmss(m.fin_voz_ms),
            m.primer_parcial.map_or("—".into(), |p| format!(
                "{:.2}",
                (p - m.ms) as f32 / 1000.0
            )),
            (m.sale - m.fin_voz_ms) as f32 / 1000.0,
            m.texto
        );
    }
    let estad = |mut v: Vec<i64>| -> String {
        if v.is_empty() {
            return "—".into();
        }
        v.sort_unstable();
        let media = v.iter().sum::<i64>() as f64 / v.len() as f64 / 1000.0;
        let p50 = v[v.len() / 2] as f64 / 1000.0;
        let max = v[v.len() - 1] as f64 / 1000.0;
        format!("media {media:.2} s, p50 {p50:.2} s, max {max:.2} s")
    };
    println!("# frases {}", medidas.len());
    println!(
        "# primer parcial (empieza a hablar → se ve algo): {}",
        estad(
            medidas
                .iter()
                .filter_map(|m| m.primer_parcial.map(|p| p - m.ms))
                .collect()
        )
    );
    println!(
        "# retraso (se callo → frase cerrada): {}",
        estad(medidas.iter().map(|m| m.sale - m.fin_voz_ms).collect())
    );
    println!(
        "# duracion de frase: {}",
        estad(medidas.iter().map(|m| m.fin_voz_ms - m.ms).collect())
    );
    if let Some(p) = a.valor("salida") {
        let quien = a.valor("quien").unwrap_or("Voz");
        let guion: String = medidas
            .iter()
            .map(|m| {
                format!(
                    "[{} {} {}] {quien}: {}\n",
                    mmss(m.ms),
                    mmss(m.fin_voz_ms),
                    mmss(m.sale),
                    m.texto.trim()
                )
            })
            .collect();
        std::fs::write(p, guion).map_err(|e| format!("{p}: {e}"))?;
        println!("# guion escrito en {p}");
    }
    Ok(())
}
