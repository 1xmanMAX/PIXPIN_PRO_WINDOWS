//! **Mas casos A1: las partes importantes de una clase grabada**
//! (`reunion`, lo que ya usa la app al terminar de grabar: `importantes`).
//! Por renglon de la transcripcion (sin quien habla, como sale de Whisper),
//! una peticion con sus ocho Nouls; la tabla de `reunion::decidir` marca
//! tarea, decision, pregunta o examen, y en las tareas si se sabe quien y
//! cuando.
//!
//! `cargo run -q -p pixpin-typesafe --example partes_importantes -- importantes.txt --rapido`
//!
//! Hechos: «tarea en m:ss cuando=si quien=no», «decision en m:ss»...

mod comun;

use pixpin_typesafe::reunion::{self, Renglon, Senales};

fn si_no(p: f64) -> &'static str {
    match pixpin_typesafe::preguntas::leer(p) {
        pixpin_typesafe::preguntas::Lectura::Si => "si",
        pixpin_typesafe::preguntas::Lectura::No => "no",
        pixpin_typesafe::preguntas::Lectura::Dudoso => "?",
    }
}

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar(
        "partes_importantes",
        "tarea, decision, pregunta y examen en una clase grabada",
    );
    let clase = s.guion.valor("clase").unwrap_or("").to_string();
    let renglones: Vec<Renglon> = s
        .guion
        .frases
        .iter()
        .map(|f| Renglon {
            ms: Some(f.ms),
            texto: f.dice.clone(),
        })
        .collect();
    let frases = s.guion.frases.clone();
    for (i, f) in frases.iter().enumerate() {
        s.reloj.esperar_hasta(f.fin_ms);
        println!("[{}] {}", comun::hora(f.ms), f.dice);
        if f.dice.split_whitespace().count() < 2 {
            continue;
        }
        let r = &renglones;
        let c = &clase;
        let Some(sen) = s.preguntar(|cl| Senales::de(&cl.preguntar(&reunion::peticion(c, r, i))?))
        else {
            continue;
        };
        println!(
            "    tarea {} (quien {} cuando {}) · decision {} · pregunta {} · examen {} · dato {} · charla {}",
            comun::casos::p(sen.tarea),
            comun::casos::p(sen.quien),
            comun::casos::p(sen.cuando),
            comun::casos::p(sen.decision),
            comun::casos::p(sen.pregunta),
            comun::casos::p(sen.examen),
            comun::casos::p(sen.dato),
            comun::casos::p(sen.charla)
        );
        let minuto = format!("{}:{:02}", f.ms / 60_000, (f.ms / 1000) % 60);
        for (tipo, _) in reunion::decidir(&sen) {
            let extra = if tipo == reunion::Tipo::Tarea {
                format!(" cuando={} quien={}", si_no(sen.cuando), si_no(sen.quien))
            } else {
                String::new()
            };
            let ahora = s.reloj.ahora();
            s.hecho(ahora, format!("{} en {minuto}{extra}", tipo.id()));
        }
    }
    s.terminar()
}
