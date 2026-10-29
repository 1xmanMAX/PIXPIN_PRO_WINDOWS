//! **Prototipo 8: de lo dicho al horario.** «Nos vemos el martes a las 3»
//! → evento propuesto. El codigo busca las expresiones de fecha y hora y
//! las resuelve (`fechas`); Jev dice si la frase pone un momento a una cita
//! o un evento y cual de las expresiones es (`vivo`, parte `cita`); la
//! tabla, en `citas`. Sin ninguna expresion de fecha no se pregunta.
//!
//! `cargo run -q -p pixpin-typesafe --example fechas_al_horario -- fechas.txt --rapido`
//!
//! Hechos: «evento <dia> <fecha> [hora]: <frase> (de «…»)».

mod comun;

use comun::partes::{self, Reunion};
use pixpin_typesafe::citas::Horario;
use pixpin_typesafe::vivo::{self, Partes};

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar("fechas_al_horario", "citas dichas → eventos del calendario");
    let r = Reunion::de(&s);
    let mut h = Horario::default();
    let p = Partes {
        cita: true,
        ..Default::default()
    };
    s.por_frase(|s, _, antes, f| {
        let ctx = r.contexto(antes, f);
        let prep = vivo::preparar(&ctx, &p);
        partes::fechas(&prep.fechas);
        if prep.peticion.questions.is_empty() {
            println!("    (sin fechas: no se pregunta)");
            return;
        }
        if let Some((sen, fe)) = s.preguntar(|c| vivo::preguntar(c, &ctx, &p)) {
            partes::cita(s, &mut h, f, &sen, &fe);
        }
    });
    s.terminar()
}
