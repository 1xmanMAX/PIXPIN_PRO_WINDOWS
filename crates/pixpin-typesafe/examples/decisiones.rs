//! **Prototipo 6: las decisiones de la reunion.** Distingue «se decidió X»
//! de lo que solo se discute o se propone (un Noul por frase, `vivo`, parte
//! `decision`; los criterios nombran el caso que confunde: «podríamos…» no
//! es una decision) y lista las decisiones con su hora.
//!
//! `cargo run -q -p pixpin-typesafe --example decisiones -- coordinacion.txt --rapido`
//!
//! Hechos: «decisión [m:ss]: <frase>».

mod comun;

use comun::partes::{self, Reunion};
use pixpin_typesafe::vivo::{self, Partes};

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar("decisiones", "lo decidido frente a lo discutido");
    let r = Reunion::de(&s);
    let p = Partes {
        decision: true,
        ..Default::default()
    };
    s.por_frase(|s, _, antes, f| {
        let ctx = r.contexto(antes, f);
        if let Some((sen, _)) = s.preguntar(|c| vivo::preguntar(c, &ctx, &p)) {
            partes::decision(s, f, &sen);
        }
    });
    println!("\n   decisiones:");
    for h in &s.hechos {
        println!("     {}", h.texto);
    }
    s.terminar()
}
