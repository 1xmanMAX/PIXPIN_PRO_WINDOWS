//! **Prototipo sin interfaz de «Sincronizar sola»** (la letra pegada con el
//! minuto de cada linea, oyendo la cancion): el mismo camino que la app
//! (`ventana_chat/musica.rs`): `Whisper::oir_cancion_pcm` →
//! `alinear_letra::palabras_oidas` → `alinear_letra::alinear`, con el idioma
//! de la letra si se adivina (o `--idioma`).
//!
//! ```text
//! cargo run -p pixpin-voz --release --example alinear_cancion -- \
//!     cancion.mp3 letra.txt [--idioma es] [--raiz <datos>] [--ort <dir>] \
//!     [--oido] [--lrc salida.lrc]
//! ```
//!
//! - `<datos>` lleva `whisper/base/` con los tres ficheros de Whisper base
//!   (ver `docs/probar-ideas-sin-interfaz.md`); `--ort` es la carpeta de
//!   `libonnxruntime.so` (o `PIXPIN_VOZ_RAIZ` / `PIXPIN_ORT`).
//! - **Si la letra ya trae tiempos** (un LRC o `[m:ss]`), se le quitan para
//!   alinearla y se usan como verdad: sale el error de cada linea y la media
//!   y el maximo, en segundos.
//! - `--oido` imprime tambien lo que oyo Whisper, tanda a tanda.
//! - `--solo-tiempos` no imprime el texto de las lineas (solo su numero):
//!   para compartir lo medido sin copiar la letra.

mod comun;

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use comun::{Argumentos, leer_audio, mmss, raiz_y_ort};
use pixpin_voz::alinear_letra::{self as alinear, Alineado};
use pixpin_voz::letra_de_cancion as letra;
use pixpin_voz::whisper::Whisper;
use pixpin_voz::{Idiomas, pcm};

fn main() {
    if let Err(e) = correr() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn correr() -> Result<(), String> {
    let a = Argumentos::leer(&["oido", "solo-tiempos"]);
    let [audio, texto] = a.sueltos.as_slice() else {
        return Err(
            "uso: alinear_cancion <audio> <letra.txt> [--idioma es] [--raiz <datos>] \
             [--ort <dir>] [--oido] [--lrc salida.lrc]"
                .into(),
        );
    };
    let (raiz, ort) = raiz_y_ort(&a)?;
    let pegada = std::fs::read_to_string(texto).map_err(|e| format!("{texto}: {e}"))?;

    // Como la app: las lineas del `Sincronizador` (sin renglones en blanco).
    // Si la letra traia tiempos, son la verdad con que se mide.
    let leida = letra::leer(&pegada);
    let pares: Vec<(Option<i64>, String)> = leida
        .lineas
        .iter()
        .flat_map(|l| l.texto.lines().map(move |t| (l.ms, t.trim().to_string())))
        .filter(|(_, t)| !t.is_empty())
        .collect();
    let lineas: Vec<String> = letra::Sincronizador::de(&leida).textos().to_vec();
    debug_assert_eq!(lineas.len(), pares.len());
    let verdad: Option<Vec<Option<i64>>> = leida
        .con_tiempos()
        .then(|| pares.iter().map(|(ms, _)| *ms).collect());
    if lineas.is_empty() {
        return Err("la letra esta vacia".into());
    }

    let idioma = match (
        a.valor("idioma"),
        alinear::idioma_de_la_letra(&lineas).filter(|c| pixpin_voz::whisper::sabe(c)),
    ) {
        (Some(i), _) => i.to_string(),
        (None, Some(c)) => c.to_string(),
        (None, None) => "es".to_string(),
    };

    let t0 = Instant::now();
    let w = Whisper::cargar(&raiz, ort.as_deref()).map_err(|e| e.to_string())?;
    let carga = t0.elapsed().as_secs_f32();
    let t1 = Instant::now();
    let muestras = leer_audio(Path::new(audio))?;
    let decodificar = t1.elapsed().as_secs_f32();
    let dura = pcm::duracion_ms(&muestras);

    let t2 = Instant::now();
    let segmentos = w
        .oir_cancion_pcm(
            &muestras,
            &Idiomas::uno(&idioma),
            &mut |_| {},
            &AtomicBool::new(false),
        )
        .map_err(|e| e.to_string())?;
    let oir = t2.elapsed().as_secs_f32();
    let oidas = alinear::palabras_oidas(&segmentos);
    let t3 = Instant::now();
    let hecho = alinear::alinear(&lineas, &oidas, Some(dura));
    let alinear_s = t3.elapsed().as_secs_f32();

    println!(
        "# {audio} | {:.1} s de audio | idioma {idioma} | {} lineas | {} tandas | {} palabras oidas",
        dura as f32 / 1000.0,
        lineas.len(),
        segmentos.len(),
        oidas.len()
    );
    println!(
        "# carga {carga:.2} s | decodificar {decodificar:.2} s | oir {oir:.1} s (RTF {:.2}) | alinear {:.0} ms",
        oir / (dura as f32 / 1000.0),
        alinear_s * 1000.0
    );
    let solo_tiempos = a.hay("solo-tiempos");
    if a.hay("oido") && !solo_tiempos {
        for s in &segmentos {
            println!(
                "#   oido {}–{}  {}",
                mmss(s.desde_ms),
                mmss(s.hasta_ms),
                s.texto
            );
        }
    }
    let Some(al) = hecho else {
        println!("# ninguna linea se pudo anclar: no hay tiempos que dar");
        return Ok(());
    };
    imprimir(&lineas, &al, verdad.as_deref(), solo_tiempos);
    if let Some(p) = a.valor("lrc") {
        let lrc = letra::a_lrc(&al.letra(&lineas), None, None);
        std::fs::write(p, lrc).map_err(|e| format!("{p}: {e}"))?;
    }
    Ok(())
}

/// Cada linea con su `[m:ss]` (lo que guarda la app), su confianza, si
/// salio de lo oido o de repartir, y el error si hay verdad.
fn imprimir(lineas: &[String], al: &Alineado, verdad: Option<&[Option<i64>]>, solo_tiempos: bool) {
    let mut errores: Vec<(f64, bool)> = Vec::new();
    for (i, (t, l)) in lineas.iter().zip(&al.lineas).enumerate() {
        let como = if l.anclada { "ancla" } else { "repart" };
        let error = verdad.and_then(|v| v.get(i).copied().flatten()).map(|v| {
            let e = (l.ms - v) as f64 / 1000.0;
            errores.push((e.abs(), l.anclada));
            format!("  verdad {} err {e:+.1}", mmss(v))
        });
        println!(
            "[{}] {:<48} ({}, {:.2}, {como}){}",
            letra::marca(l.ms),
            if solo_tiempos {
                format!("linea {}", i + 1)
            } else {
                t.clone()
            },
            mmss(l.ms),
            l.confianza,
            error.unwrap_or_default()
        );
    }
    println!(
        "# ancladas {}/{} ({:.0} %)",
        al.ancladas(),
        lineas.len(),
        100.0 * al.ancladas() as f64 / lineas.len() as f64
    );
    if !errores.is_empty() {
        let resumen = |v: &[f64]| {
            if v.is_empty() {
                return "—".to_string();
            }
            let media = v.iter().sum::<f64>() / v.len() as f64;
            let max = v.iter().cloned().fold(0.0, f64::max);
            let dentro = |s: f64| v.iter().filter(|e| **e <= s).count() as f64 / v.len() as f64;
            format!(
                "media {media:.2} s | max {max:.1} s | ≤1 s {:.0} % | ≤2 s {:.0} % (n={})",
                100.0 * dentro(1.0),
                100.0 * dentro(2.0),
                v.len()
            )
        };
        let todas: Vec<f64> = errores.iter().map(|e| e.0).collect();
        let anclas: Vec<f64> = errores.iter().filter(|e| e.1).map(|e| e.0).collect();
        println!("# error todas:   {}", resumen(&todas));
        println!("# error anclas:  {}", resumen(&anclas));
    }
}
