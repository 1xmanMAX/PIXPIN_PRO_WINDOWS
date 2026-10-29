//! **Prototipo 7: las preguntas que nadie contesto.** Cada pregunta de
//! verdad queda abierta; en las frases siguientes va un Noul por pregunta
//! abierta («¿esta frase la contesta?», con la pregunta dentro) en la misma
//! peticion de la frase (`vivo`, partes `pregunta` y `abiertas`); la tabla,
//! en `pendientes`. Al final, las que siguen abiertas.
//!
//! `cargo run -q -p pixpin-typesafe --example preguntas_pendientes -- coordinacion.txt --rapido`
//!
//! Hechos: «abierta: …», «contestada <pregunta> por <quien>: …», «sin
//! respuesta <pregunta>».

mod comun;

use comun::partes::{self, Reunion};
use pixpin_typesafe::pendientes::Pendientes;
use pixpin_typesafe::vivo::{self, Partes};

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar(
        "preguntas_pendientes",
        "preguntas hechas que nadie contesto",
    );
    let r = Reunion::de(&s);
    let mut pend = Pendientes::default();
    s.por_frase(|s, _, antes, f| {
        let abiertas = pend.para_preguntar();
        let p = Partes {
            pregunta: true,
            abiertas: &abiertas,
            ..Default::default()
        };
        let ctx = r.contexto(antes, f);
        if let Some((sen, _)) = s.preguntar(|c| vivo::preguntar(c, &ctx, &p)) {
            partes::preguntas(s, &mut pend, f, &sen);
        }
    });
    println!();
    partes::preguntas_al_final(&mut s, &pend);
    s.terminar()
}
