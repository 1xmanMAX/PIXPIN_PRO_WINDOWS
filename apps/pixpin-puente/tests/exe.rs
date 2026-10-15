//! El `pixpin-puente.exe` de verdad, arrancado como lo arranca el navegador
//! (tuberias, un mensaje con su largo), en modo «en seco»: no busca la
//! PixPin del usuario, escribe en una carpeta de la prueba y devuelve el
//! pedido. Mide lo que tarda de punta a punta.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn correr(carpeta: &std::path::Path, mensaje: &[u8]) -> (serde_json::Value, Duration) {
    let inicio = Instant::now();
    let mut hijo = Command::new(env!("CARGO_BIN_EXE_pixpin-puente"))
        .arg("chrome-extension://abc/")
        .env("PIXPIN_PUENTE_EN_SECO", carpeta)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut e = hijo.stdin.take().unwrap();
        e.write_all(&(mensaje.len() as u32).to_ne_bytes()).unwrap();
        e.write_all(mensaje).unwrap();
    }
    let mut salida = Vec::new();
    hijo.stdout
        .take()
        .unwrap()
        .read_to_end(&mut salida)
        .unwrap();
    assert!(hijo.wait().unwrap().success());
    let tardo = inicio.elapsed();
    let largo = u32::from_ne_bytes(salida[..4].try_into().unwrap()) as usize;
    assert_eq!(largo, salida.len() - 4);
    (serde_json::from_slice(&salida[4..]).unwrap(), tardo)
}

#[test]
fn el_exe_contesta_por_la_tuberia_y_rapido() {
    let d = std::env::temp_dir().join(format!("pixpin-puente-exe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    // Una imagen de ~1,5 MB en base64 (lo de una captura de pagina mediana).
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.resize(1_100_000, 7);
    let b64 = base64(&png);
    let m = serde_json::json!({"accion":"captura_pagina","url":"https://a.pe","titulo":"A",
                               "imagen":{"tipo":"image/png","base64":b64}});
    let mut tiempos = Vec::new();
    for _ in 0..5 {
        let (r, t) = correr(&d, m.to_string().as_bytes());
        assert_eq!(r["ok"], true, "{r}");
        let pedido: serde_json::Value =
            serde_json::from_str(r["pedido"].as_str().unwrap()).unwrap();
        let ruta = pedido["imagenes"][0].as_str().unwrap().to_string();
        assert_eq!(std::fs::read(&ruta).unwrap().len(), png.len());
        tiempos.push(t);
    }
    let (r, t) = correr(&d, br#"{"accion":"tarea","texto":"comprar yeso"}"#);
    assert_eq!(r["ok"], true);
    tiempos.sort();
    // Lo que tarda Windows en arrancar un programa cualquiera, para comparar.
    let mut base: Vec<Duration> = (0..5)
        .map(|_| {
            let i = Instant::now();
            let _ = Command::new(r"C:\Windows\System32\whoami.exe")
                .stdout(Stdio::null())
                .status();
            i.elapsed()
        })
        .collect();
    base.sort();
    eprintln!(
        "whoami.exe (solo arrancar un proceso en este equipo): mediana {:?}",
        base[2]
    );
    eprintln!(
        "puente con imagen de 1,1 MB: mediana {:?}, peor {:?}; tarea: {t:?}",
        tiempos[2], tiempos[4]
    );
    // Con el arranque del proceso incluido, que es de Windows (lo de
    // `whoami.exe`) y aqui varia con la carga del equipo: solo se mira que
    // no se cuelgue. Lo que tarda el puente por dentro lo mide
    // `pruebas::lo_que_tarda_el_puente_por_dentro`, con su tope de 50 ms.
    assert!(tiempos[2] < Duration::from_secs(3), "{tiempos:?}");

    // JSON roto: contesta el error, no se cuelga.
    let (r, _) = correr(&d, b"{roto");
    assert_eq!(r["ok"], false);
    let _ = std::fs::remove_dir_all(&d);
}

fn base64(datos: &[u8]) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for t in datos.chunks(3) {
        let n = (u32::from(t[0]) << 16)
            | (u32::from(*t.get(1).unwrap_or(&0)) << 8)
            | u32::from(*t.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= t.len() {
                s.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}
