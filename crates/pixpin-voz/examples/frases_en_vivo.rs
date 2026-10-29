//! **Prototipo sin interfaz de la escucha en vivo de una reunion** (los
//! temas de la reunion, `grabar_clase/escucha.rs`): una grabacion de voz
//! entra en `en_vivo::Frasero` en bloques de 10 ms, como los de la mezcla
//! (48 kHz estereo), y cada frase que corta se pasa a texto con Whisper
//! base cargado como en la app (`Whisper::cargar_ligero`, el reparto del
//! modo cuidadoso), con la misma `Cola` y el mismo `Freno`.
//!
//! ```text
//! cargo run -p pixpin-voz --release --example frases_en_vivo -- \
//!     reunion.wav [--rapido] [--idioma es] [--raiz <datos>] [--ort <dir>] \
//!     [--medida whisper|windows] [--salida frases.txt] [--quien Nombre]
//! ```
//!
//! - Sin `--rapido`, **en tiempo real**: un hilo mete los bloques al ritmo
//!   del reloj (el oido) y otro pasa a texto (como la app); lo medido es de
//!   verdad.
//! - Con `--rapido`, **tiempo real simulado**: se mete el audio sin esperar
//!   y cada frase se pasa a texto en el acto; el reloj de la grabacion
//!   cuenta cuando habria salido cada frase y cuando habria estado su texto
//!   con un solo reconocedor (la cola y el freno de la app incluidos).
//!
//! Por frase: donde empieza y donde se dejo de hablar, el **corte** (se
//! dejo de hablar → sale la frase), la espera en la cola, lo que tarda en
//! pasarse a texto y el **retraso** total (se dejo de hablar → texto). Al
//! final, medias y maximos y el RTF.
//!
//! `--salida` escribe `[m:ss] Quien: frase`, el guion que leen los
//! prototipos de Jev (`crates/pixpin-typesafe/examples/`); `Quien` es
//! `--quien` (por omision «Voz»: aqui no se sabe quien habla).

mod comun;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use comun::{Argumentos, leer_audio, mmss, raiz_y_ort};
use pixpin_voz::en_vivo::{
    COLA, Cola, Frase, Frasero, Freno, Medida, PARA_WHISPER, PARA_WINDOWS, Paso,
};
use pixpin_voz::whisper::Whisper;
use pixpin_voz::{HERCIOS, marca_de_tiempo};

/// Lo que da la mezcla de la grabacion: 48 kHz, dos canales, de 10 en 10 ms.
const HZ_MEZCLA: u32 = 48_000;
const CANALES: u16 = 2;
const BLOQUE_MS: u64 = 10;

fn main() {
    if let Err(e) = correr() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

/// Lo medido de una frase.
struct Medido {
    n: u64,
    ms: i64,
    fin_voz_ms: i64,
    dura_ms: i64,
    /// Se dejo de hablar → sale la frase (ms de grabacion).
    corte_ms: i64,
    /// Sale la frase → empieza a pasarse a texto.
    espera_ms: i64,
    /// Lo que tarda Whisper.
    texto_ms: i64,
    paso: Paso,
    texto: String,
}

impl Medido {
    /// Se dejo de hablar → hay texto.
    fn retraso_ms(&self) -> i64 {
        self.corte_ms + self.espera_ms + self.texto_ms
    }
}

fn correr() -> Result<(), String> {
    let a = Argumentos::leer(&["rapido"]);
    let [audio] = a.sueltos.as_slice() else {
        return Err(
            "uso: frases_en_vivo <audio.wav> [--rapido] [--idioma es] [--raiz <datos>] \
             [--ort <dir>] [--medida whisper|windows] [--salida frases.txt] [--quien Nombre]"
                .into(),
        );
    };
    let (raiz, ort) = raiz_y_ort(&a)?;
    let idioma = a.valor("idioma").unwrap_or("es").to_string();
    let medida = match a.valor("medida").unwrap_or("whisper") {
        "windows" => PARA_WINDOWS,
        _ => PARA_WHISPER,
    };
    let muestras = leer_audio(Path::new(audio))?;
    let dura_ms = muestras.len() as i64 * 1000 / i64::from(HERCIOS);
    let t0 = Instant::now();
    let w = Whisper::cargar_ligero(&raiz, ort.as_deref()).map_err(|e| e.to_string())?;
    let carga = t0.elapsed().as_secs_f32();
    let bloques = bloques_de_la_mezcla(&muestras);

    let t1 = Instant::now();
    let medidos = if a.hay("rapido") {
        simulado(&w, &idioma, medida, &bloques)
    } else {
        en_tiempo_real(&w, &idioma, medida, &bloques)
    };
    let total_s = t1.elapsed().as_secs_f32();

    println!(
        "# {audio} | {:.1} s | {} | medida {} | carga {carga:.2} s | {}",
        dura_ms as f32 / 1000.0,
        if a.hay("rapido") {
            "tiempo real simulado"
        } else {
            "tiempo real"
        },
        a.valor("medida").unwrap_or("whisper"),
        format_args!("{total_s:.1} s de reloj")
    );
    println!("#  n  inicio  fin voz   dura  corte  espera  texto  retraso   RTF  texto");
    for m in &medidos {
        let rtf = m.texto_ms as f32 / m.dura_ms.max(1) as f32;
        let texto = match m.paso {
            Paso::Pasar => m.texto.clone(),
            otro => format!("<{otro:?}>"),
        };
        println!(
            "{:>4} {:>7} {:>8} {:>6.1} {:>6.2} {:>7.2} {:>6.2} {:>8.2} {:>5.2}  {}",
            m.n,
            mmss(m.ms),
            mmss(m.fin_voz_ms),
            m.dura_ms as f32 / 1000.0,
            m.corte_ms as f32 / 1000.0,
            m.espera_ms as f32 / 1000.0,
            m.texto_ms as f32 / 1000.0,
            m.retraso_ms() as f32 / 1000.0,
            rtf,
            texto
        );
    }
    resumen(&medidos, dura_ms);
    if let Some(p) = a.valor("salida") {
        let quien = a.valor("quien").unwrap_or("Voz");
        let guion: String = medidos
            .iter()
            .filter(|m| m.paso == Paso::Pasar && !m.texto.trim().is_empty())
            .map(|m| format!("[{}] {quien}: {}\n", marca_de_tiempo(m.ms), m.texto.trim()))
            .collect();
        std::fs::write(p, guion).map_err(|e| format!("{p}: {e}"))?;
        println!("# guion escrito en {p}");
    }
    Ok(())
}

/// El PCM de 16 kHz mono como lo daria la mezcla: bloques de 10 ms a
/// 48 kHz estereo (cada muestra repetida; `a_16k` la deja como estaba).
fn bloques_de_la_mezcla(muestras: &[i16]) -> Vec<(u64, Vec<i16>)> {
    let por_bloque = (HERCIOS as u64 * BLOQUE_MS / 1000) as usize;
    let factor = (HZ_MEZCLA / HERCIOS) as usize;
    let mut posicion = 0u64;
    muestras
        .chunks(por_bloque)
        .map(|c| {
            let b: Vec<i16> = c
                .iter()
                .flat_map(|&m| std::iter::repeat_n(m, factor * usize::from(CANALES)))
                .collect();
            let p = posicion;
            posicion += (c.len() * factor) as u64;
            (p, b)
        })
        .collect()
}

/// Lo que dice Whisper de una frase (lo mismo que junta la app).
fn pasar(w: &Whisper, idioma: &str, f: &Frase, parar: &AtomicBool) -> String {
    match w.transcribir_pcm(&f.muestras, idioma, &mut |_| {}, parar) {
        Ok(t) => t
            .segmentos
            .iter()
            .map(|s| s.texto.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        Err(_) => String::new(),
    }
}

/// El fin del bloque `i` en ms de grabacion.
fn reloj_de(posicion_fin: u64) -> i64 {
    (posicion_fin * 1000 / u64::from(HZ_MEZCLA)) as i64
}

/// **Tiempo real simulado**: un solo reconocedor; la cola tira lo mas viejo
/// si se llena y el freno salta una frase tras una que tardo mucho.
fn simulado(w: &Whisper, idioma: &str, medida: Medida, bloques: &[(u64, Vec<i16>)]) -> Vec<Medido> {
    let parar = AtomicBool::new(false);
    let mut f = Frasero::nuevo(medida);
    // Las frases con el ms de grabacion en que salen.
    let mut salidas: Vec<(Frase, i64)> = Vec::new();
    for (p, b) in bloques {
        let ahora = reloj_de(p + (b.len() / usize::from(CANALES)) as u64);
        for x in f.meter(*p, HZ_MEZCLA, CANALES, b) {
            salidas.push((x, ahora));
        }
    }
    if let Some(x) = f.acabar() {
        let fin = bloques.last().map_or(0, |(p, b)| {
            reloj_de(p + (b.len() / usize::from(CANALES)) as u64)
        });
        salidas.push((x, fin));
    }
    // Un reconocedor: libre desde `libre` (ms de grabacion).
    let mut libre = 0i64;
    let mut freno = Freno::default();
    let mut medidos = Vec::new();
    let mut i = 0;
    while i < salidas.len() {
        // Cuando se pone a ello, lo que haya salido ya hace cola: si pasa de
        // COLA, se tiran las mas viejas.
        let empieza = libre.max(salidas[i].1);
        let en_cola = salidas[i..].iter().take_while(|s| s.1 <= empieza).count();
        if en_cola > COLA {
            for (x, sale) in &salidas[i..i + en_cola - COLA] {
                medidos.push(medido(
                    x,
                    *sale,
                    0,
                    0,
                    Paso::SaltarRetraso,
                    "<tirada: cola llena>",
                ));
            }
            i += en_cola - COLA;
            continue;
        }
        let (x, sale) = &salidas[i];
        let paso = freno.decidir(x, None);
        if paso != Paso::Pasar {
            medidos.push(medido(x, *sale, 0, 0, paso, ""));
            i += 1;
            continue;
        }
        let t = Instant::now();
        let texto = pasar(w, idioma, x, &parar);
        let tarda = t.elapsed();
        freno.tardo(x.duracion(), tarda);
        let tarda_ms = tarda.as_millis() as i64;
        medidos.push(medido(x, *sale, empieza - sale, tarda_ms, paso, &texto));
        libre = empieza + tarda_ms;
        i += 1;
    }
    medidos
}

fn medido(x: &Frase, sale: i64, espera: i64, texto_ms: i64, paso: Paso, texto: &str) -> Medido {
    Medido {
        n: x.n,
        ms: x.ms,
        fin_voz_ms: x.fin_voz_ms,
        dura_ms: x.duracion().as_millis() as i64,
        corte_ms: sale - x.fin_voz_ms,
        espera_ms: espera,
        texto_ms,
        paso,
        texto: texto.to_string(),
    }
}

/// **En tiempo real**, con dos hilos como la app: el oido mete los bloques
/// al ritmo del reloj y el del texto saca de la cola.
fn en_tiempo_real(
    w: &Whisper,
    idioma: &str,
    medida: Medida,
    bloques: &[(u64, Vec<i16>)],
) -> Vec<Medido> {
    type Comun = (Mutex<(Cola<(Frase, Instant)>, bool)>, Condvar);
    let comun: Arc<Comun> = Arc::new((Mutex::new((Cola::nueva(COLA), false)), Condvar::new()));
    let empiezo = Instant::now();
    let mut medidos = Vec::new();
    std::thread::scope(|s| {
        let c = comun.clone();
        s.spawn(move || {
            let mut f = Frasero::nuevo(medida);
            let meter = |x: Frase| {
                let mut g = c.0.lock().unwrap_or_else(|e| e.into_inner());
                if let Some((tirada, _)) = g.0.meter((x, Instant::now())) {
                    eprintln!("# la frase {} se tiro: el texto va tarde", tirada.n);
                }
                drop(g);
                c.1.notify_all();
            };
            for (p, b) in bloques {
                let fin = reloj_de(p + (b.len() / usize::from(CANALES)) as u64);
                let cuando = empiezo + Duration::from_millis(fin as u64);
                if let Some(falta) = cuando.checked_duration_since(Instant::now()) {
                    std::thread::sleep(falta);
                }
                for x in f.meter(*p, HZ_MEZCLA, CANALES, b) {
                    meter(x);
                }
            }
            if let Some(x) = f.acabar() {
                meter(x);
            }
            c.0.lock().unwrap_or_else(|e| e.into_inner()).1 = true;
            c.1.notify_all();
        });
        let parar = AtomicBool::new(false);
        let mut freno = Freno::default();
        loop {
            let siguiente = {
                let mut g = comun.0.lock().unwrap_or_else(|e| e.into_inner());
                loop {
                    if let Some(x) = g.0.sacar() {
                        break Some(x);
                    }
                    if g.1 {
                        break None;
                    }
                    g = comun.1.wait(g).unwrap_or_else(|e| e.into_inner());
                }
            };
            let Some((x, salio)) = siguiente else {
                break;
            };
            // Cuando salio, en ms de grabacion (el reloj empezo con ella).
            let sale = salio.duration_since(empiezo).as_millis() as i64;
            let espera = salio.elapsed().as_millis() as i64;
            let paso = freno.decidir(&x, None);
            if paso != Paso::Pasar {
                medidos.push(medido(&x, sale, espera, 0, paso, ""));
                continue;
            }
            let t = Instant::now();
            let texto = pasar(w, idioma, &x, &parar);
            freno.tardo(x.duracion(), t.elapsed());
            let tarda = t.elapsed().as_millis() as i64;
            medidos.push(medido(&x, sale, espera, tarda, paso, &texto));
        }
        parar.store(true, Ordering::Relaxed);
    });
    medidos
}

fn resumen(medidos: &[Medido], dura_ms: i64) {
    let pasadas: Vec<&Medido> = medidos.iter().filter(|m| m.paso == Paso::Pasar).collect();
    let estad = |v: Vec<i64>| -> String {
        if v.is_empty() {
            return "—".into();
        }
        let media = v.iter().sum::<i64>() as f64 / v.len() as f64 / 1000.0;
        let max = *v.iter().max().unwrap_or(&0) as f64 / 1000.0;
        format!("media {media:.2} s, max {max:.2} s")
    };
    println!(
        "# frases {} (a texto {}, saltadas {})",
        medidos.len(),
        pasadas.len(),
        medidos.len() - pasadas.len()
    );
    println!(
        "# duracion de frase: {}",
        estad(pasadas.iter().map(|m| m.dura_ms).collect())
    );
    println!(
        "# corte (se callo → sale): {}",
        estad(pasadas.iter().map(|m| m.corte_ms).collect())
    );
    println!(
        "# espera en cola: {}",
        estad(pasadas.iter().map(|m| m.espera_ms).collect())
    );
    println!(
        "# pasar a texto: {}",
        estad(pasadas.iter().map(|m| m.texto_ms).collect())
    );
    println!(
        "# retraso (se callo → texto): {}",
        estad(pasadas.iter().map(|m| m.retraso_ms()).collect())
    );
    let texto: i64 = pasadas.iter().map(|m| m.texto_ms).sum();
    let audio: i64 = pasadas.iter().map(|m| m.dura_ms).sum();
    println!(
        "# RTF: {:.3} sobre lo pasado a texto ({:.1} s), {:.3} sobre la grabacion entera ({:.1} s)",
        texto as f64 / audio.max(1) as f64,
        audio as f64 / 1000.0,
        texto as f64 / dura_ms.max(1) as f64,
        dura_ms as f64 / 1000.0
    );
}
