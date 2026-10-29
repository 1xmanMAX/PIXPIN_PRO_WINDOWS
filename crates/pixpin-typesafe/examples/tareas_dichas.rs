//! **Prototipo 2: tareas dichas en voz alta.** «Yo me encargo de mandar la
//! cotización el viernes» → tarea propuesta {texto, responsable, fecha,
//! prioridad}. Jev juzga (¿compromiso?, ¿quien?, ¿cual es el plazo?, ¿con
//! prisa?) en una peticion por frase (`vivo`, parte `compromiso`); la fecha
//! la resuelve el codigo contra la de la reunion (`fechas`); la tabla, en
//! `compromisos`.
//!
//! `cargo run -q -p pixpin-typesafe --example tareas_dichas -- tareas.txt --rapido`
//!
//! Hechos: «tarea <responsable> <fecha|sin fecha> <prioridad>: <frase>».

mod comun;

use comun::partes::{self, Reunion};
use pixpin_typesafe::vivo::{self, Partes};

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar("tareas_dichas", "compromisos dichos → tareas propuestas");
    let r = Reunion::de(&s);
    let p = Partes {
        compromiso: true,
        ..Default::default()
    };
    s.por_frase(|s, _, antes, f| {
        let ctx = r.contexto(antes, f);
        let Some((sen, fe)) = s.preguntar(|c| vivo::preguntar(c, &ctx, &p)) else {
            return;
        };
        partes::fechas(&fe);
        partes::tareas(s, f, &sen, &fe, r.hoy);
    });
    s.terminar()
}
