//! **Grabar de verdad** la pantalla principal: necesita Windows con una
//! sesion de escritorio, GPU y Media Foundation (H.264 y AAC). No corre en
//! la CI; se ejecuta a mano con
//! `cargo test -p pixpin-motor-video -- --ignored --nocapture`
//! (ver docs/motor-de-video.md).
#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use pixpin_motor_video::archivo::a_medias;
use pixpin_motor_video::{
    FormatoAudio, MotorVideo, Opciones, Perfil, hay_h264_por_hardware, hay_hevc_por_hardware,
    monitor_principal,
};

fn carpeta(nombre: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pixpin-motor-video-{nombre}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Cuantas cajas `trak` (pistas) tiene el MP4.
fn pistas(ruta: &Path) -> usize {
    let bytes = std::fs::read(ruta).unwrap();
    assert!(bytes.windows(4).any(|v| v == b"moov"), "sin indice");
    bytes.windows(4).filter(|v| *v == b"trak").count()
}

#[test]
#[ignore = "necesita una sesion de escritorio con GPU; ejecutar con --ignored"]
fn tres_segundos_de_la_pantalla_con_sonido_dan_un_mp4_con_dos_pistas() {
    eprintln!(
        "H.264 de la grafica: {}; HEVC de la grafica: {}",
        hay_h264_por_hardware(),
        hay_hevc_por_hardware()
    );
    let dir = carpeta("tres-segundos");
    let destino = dir.join("prueba.mp4");
    let opciones = Opciones {
        audio: Some(FormatoAudio::mono(48_000)),
        ..Opciones::de(monitor_principal())
    };
    let motor = MotorVideo::empezar(opciones, destino.clone()).expect("el motor no arranco");
    // Un tono de 440 Hz en bloques de 10 ms, al ritmo de verdad, como los
    // daria la grabadora.
    let inicio = Instant::now();
    let mut posicion = 0u64;
    while inicio.elapsed() < Duration::from_secs(3) {
        let debidas = (inicio.elapsed().as_secs_f64() * 48_000.0) as u64;
        while posicion + 480 <= debidas {
            let bloque: Vec<i16> = (0..480u64)
                .map(|i| {
                    let t = (posicion + i) as f64 / 48_000.0;
                    ((t * 440.0 * std::f64::consts::TAU).sin() * 8_000.0) as i16
                })
                .collect();
            motor.poner_audio(&bloque, posicion).unwrap();
            posicion += 480;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let estado = motor.estado();
    eprintln!("{estado:?}");
    let r = motor.terminar().expect("el MP4 no se cerro");
    eprintln!("{r:?}");
    assert_eq!(r.ruta, destino);
    assert!(!a_medias(&destino).exists(), "quedo el .parte");
    assert!(r.cortado.is_none());
    assert_eq!(pistas(&destino), 2, "video y sonido");
    // Tres segundos: del orden de lo que dura el sonido dado.
    assert!(
        (2_700..=3_300).contains(&r.duracion_ms),
        "{}",
        r.duracion_ms
    );
    // Por lo menos un fotograma por segundo (el latido), y el primero.
    assert!(r.fotogramas >= 3, "{}", r.fotogramas);
    // Un tamano creible: ni vacio ni descontrolado (Normal: ~0,5 Mbit/s de
    // media y 3 de pico; tres segundos no pasan de 2 MB).
    assert!((2_000..=2_000_000).contains(&r.bytes), "{} bytes", r.bytes);
    // Lados pares, que es lo unico que admite H.264.
    assert_eq!((r.medidas.0 % 2, r.medidas.1 % 2), (0, 0));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
#[ignore = "necesita una sesion de escritorio con GPU; ejecutar con --ignored"]
fn sin_sonido_sale_un_mp4_de_una_pista_con_el_reloj_del_sistema() {
    let dir = carpeta("mudo");
    let destino = dir.join("mudo.mp4");
    let opciones = Opciones {
        perfil: Perfil::Ligero,
        ..Opciones::de(monitor_principal())
    };
    let motor = MotorVideo::empezar(opciones, destino.clone()).unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    motor.pausar();
    std::thread::sleep(Duration::from_millis(1000));
    motor.seguir();
    std::thread::sleep(Duration::from_millis(500));
    let r = motor.terminar().unwrap();
    eprintln!("{r:?}");
    assert_eq!(pistas(&destino), 1);
    // Caso negativo: el segundo de pausa no cuenta.
    assert!(
        (1_700..=2_300).contains(&r.duracion_ms),
        "{}",
        r.duracion_ms
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
#[ignore = "necesita una sesion de escritorio con GPU; ejecutar con --ignored"]
fn soltar_el_motor_sin_terminar_no_deja_nada() {
    let dir = carpeta("tirado");
    let destino = dir.join("tirado.mp4");
    let motor = MotorVideo::empezar(Opciones::de(monitor_principal()), destino.clone()).unwrap();
    std::thread::sleep(Duration::from_millis(800));
    drop(motor);
    assert!(!destino.exists());
    assert!(!a_medias(&destino).exists());
    std::fs::remove_dir_all(&dir).unwrap();
}
