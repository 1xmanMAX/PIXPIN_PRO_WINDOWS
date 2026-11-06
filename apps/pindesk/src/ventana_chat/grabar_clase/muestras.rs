//! **Muestras en PNG de la grabación** (2-nov-2026), para las capturas de la
//! Store: la barra grabando con los temas de la reunión abiertos debajo, y la
//! pantalla completa con los temas (dos ya tratados) y lo que se va oyendo.
//!
//! ```text
//! cargo test -p pindesk --bin pindesk muestras_de_la_grabacion -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Deja `grabar-clase-barra.png` y `grabar-clase-completa.png` en
//! `PINDESK_MUESTRAS` (o la temporal). Sin un dispositivo de sonido (el
//! runner de la Store no tiene) la barra sale «abriendo», con el reloj en
//! 0:00: lo demás es igual.

use std::sync::mpsc::channel;
use std::time::Duration;

use pindesk_geom::Rect;
use pindesk_proyecto::temas::Tema;
use pindesk_store::{Catalogo, Idioma, Ubicacion};

use super::{Estado, Fase, HUECO, Pedido, Rotulos, pintar, tamano_de};
use crate::ventanita::Botones;

/// La reunión de la muestra: los temas de la agenda, dos ya tratados (uno
/// por Jev, otro a mano), y las frases que ya se oyeron.
fn estado(pedido: &Pedido, carpeta: &std::path::Path) -> Estado {
    let mut e = Estado::vacio(pedido, format!("Clase {HUECO}"));
    // Con sonido de verdad, grabando; sin él, abriendo (el reloj en 0:00).
    let destino = carpeta.join("muestra.m4a");
    e.fase = match pindesk_audio::Grabadora::empezar_con(
        &destino,
        pindesk_audio::Fuentes {
            microfono: false,
            equipo: true,
        },
    ) {
        Ok(g) => Fase::Grabando(g),
        Err(_) => Fase::Abriendo(channel().1),
    };
    let temas = [
        "Repaso de la regla de la cadena",
        "Derivadas implícitas",
        "Ejercicios 4.2 a 4.6",
        "Dudas del examen parcial",
        "Tarea para el lunes",
    ];
    e.temas = temas
        .iter()
        .enumerate()
        .map(|(i, t)| Tema::nuevo(&format!("t{i}"), t))
        .collect();
    e.temas[0].hecho = true;
    e.temas[0].ms = Some(3 * 60_000 + 12_000);
    e.temas[0].por_jev = true;
    e.temas[0].p = Some(0.93);
    e.temas[1].hecho = true;
    e.temas[1].ms = Some(14 * 60_000 + 40_000);
    e.temas[1].a_mano = true;
    e.titulo_temas = format!("Temas · {HUECO}");
    e.panel_temas = true;
    e.frases = vec![
        (
            14 * 60_000 + 40_000,
            "Bien, pasamos a las derivadas implícitas.".into(),
        ),
        (
            15 * 60_000 + 5_000,
            "Cuando no podemos despejar y, derivamos los dos lados respecto de x.".into(),
        ),
        (
            15 * 60_000 + 31_000,
            "Por ejemplo, en x² + y² = 25 queda 2x + 2y·y′ = 0.".into(),
        ),
        (
            16 * 60_000 + 2_000,
            "Y de ahí despejamos y′, que es −x partido por y.".into(),
        ),
    ];
    e.parcial = "ahora probad vosotros con el ejercicio 4.2".into();
    e
}

#[test]
#[ignore = "necesita GPU; genera PNG para mirarlos"]
fn muestras_de_la_grabacion() {
    let carpeta =
        std::env::temp_dir().join(format!("pixpin-muestra-grabacion-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&carpeta);
    std::fs::create_dir_all(&carpeta).unwrap();
    let pedido = Pedido {
        ubicacion: Ubicacion::Portable {
            raiz: carpeta.clone(),
        },
        idioma: Idioma::Espanol,
        aparato: "PC".into(),
        proyectos: vec![("calculo".into(), "Cálculo II".into())],
        asignatura: None,
    };
    let textos = Catalogo::nuevo(Idioma::Espanol);
    let r = Rotulos::de(&textos);
    let mut e = estado(&pedido, &carpeta);
    let llevado = Duration::from_secs(16 * 60 + 10);
    let escala = 1.25f32;

    // La barra, con la lista de los temas abierta debajo.
    let (w, h) = tamano_de(&e);
    let (w, h) = (
        (w as f32 * escala).round() as u32,
        (h as f32 * escala).round() as u32,
    );
    let marco = Rect {
        x: 0,
        y: 0,
        ancho: w,
        alto: h,
    };
    let mut botones: Botones<super::Accion> = Botones::default();
    crate::ventanita::muestra("grabar-clase-barra", w, h, |p, _| {
        pintar(p, marco, escala, &e, &pedido, &r, llevado, &mut botones)
    });

    // La pantalla completa, del tamaño de una pantalla de portátil.
    e.completa = true;
    e.letra = 1.15;
    let (w, h) = (1600u32, 900u32);
    let marco = Rect {
        x: 0,
        y: 0,
        ancho: w,
        alto: h,
    };
    crate::ventanita::muestra("grabar-clase-completa", w, h, |p, _| {
        pintar(p, marco, 1.0, &e, &pedido, &r, llevado, &mut botones)
    });

    if let Fase::Grabando(g) = std::mem::replace(&mut e.fase, Fase::Preparar) {
        g.cancelar();
    }
    let _ = std::fs::remove_dir_all(&carpeta);
}
