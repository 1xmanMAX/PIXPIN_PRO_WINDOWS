//! **Prototipo 5: cuanto tiempo se dedico a cada tema.** Cada frase va a
//! un tema de la agenda (Choice) o a «otro» (un Noul aparte: «¿es de la
//! agenda?»; la duda sigue el tema anterior), y el codigo suma los minutos
//! (`tiempo_de_temas`). Es la misma peticion que `tema_olvidado`.
//!
//! `cargo run -q -p pixpin-typesafe --example tiempo_por_tema -- coordinacion.txt --rapido`
//!
//! Hechos: «<tema|otro>: <frase>» y «más tiempo: <tema>».

mod comun;

use comun::partes::{self, Reunion};
use pixpin_typesafe::tiempo_de_temas::Cronometro;
use pixpin_typesafe::vivo::{self, Partes};

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar(
        "tiempo_por_tema",
        "cada frase a su tema y los minutos de cada uno",
    );
    let r = Reunion::de(&s);
    let mut c = Cronometro::nuevo(r.agenda.len());
    let p = Partes {
        tema: true,
        ..Default::default()
    };
    s.por_frase(|s, _, antes, f| {
        let anterior = r.tema_anterior(&c);
        let mut ctx = r.contexto(antes, f);
        ctx.tema_anterior = anterior.as_deref();
        let sen = s.preguntar(|cl| vivo::preguntar(cl, &ctx, &p)).map(|x| x.0);
        partes::tema(s, &r, &mut c, f, sen.as_ref());
    });
    let fin = s.guion.frases.last().map_or(0, |f| f.fin_ms);
    partes::tiempos(&mut s, &r, &c, fin);
    s.terminar()
}
