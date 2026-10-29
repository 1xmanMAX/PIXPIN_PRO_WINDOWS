//! **Prototipo 4: el tema que se olvida.** Con la duracion prevista
//! (`# duracion: 20`, en minutos), avisa cuando queda poco y aun hay temas
//! sin tratar, y cuando se esta cerrando la reunion con temas pendientes.
//! Casi todo es codigo (`tiempo_de_temas`: el reloj, lo tratado, los
//! avisos); Jev solo dice a que tema va cada frase y si se esta cerrando
//! (`vivo`, partes `tema` y `cierre`: la misma peticion que
//! `tiempo_por_tema`).
//!
//! `cargo run -q -p pixpin-typesafe --example tema_olvidado -- tema_olvidado.txt --rapido`
//!
//! Hechos: «aviso queda poco (quedan m:ss): <temas>», «aviso tiempo
//! cumplido: <temas>», «aviso están cerrando y quedan: <temas>».

mod comun;

use comun::partes::{self, Reunion};
use pixpin_typesafe::tiempo_de_temas::{Avisador, Cronometro};
use pixpin_typesafe::vivo::{self, Partes};

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar(
        "tema_olvidado",
        "avisar si se acaba el tiempo y quedan temas",
    );
    let r = Reunion::de(&s);
    let minutos: i64 = s
        .guion
        .valor("duracion")
        .and_then(|d| d.parse().ok())
        .unwrap_or(30);
    let mut a = Avisador::nuevo(minutos * 60_000);
    println!(
        "   duracion prevista: {minutos} min (aviso a {} del final)\n",
        comun::hora(a.margen_ms())
    );
    let mut c = Cronometro::nuevo(r.agenda.len());
    let p = Partes {
        tema: true,
        cierre: true,
        ..Default::default()
    };
    s.por_frase(|s, _, antes, f| {
        let anterior = r.tema_anterior(&c);
        let mut ctx = r.contexto(antes, f);
        ctx.tema_anterior = anterior.as_deref();
        let sen = s.preguntar(|cl| vivo::preguntar(cl, &ctx, &p)).map(|x| x.0);
        partes::tema(s, &r, &mut c, f, sen.as_ref());
        partes::olvidados(s, &r, &c, &mut a, sen.as_ref());
    });
    let fin = s.guion.frases.last().map_or(0, |f| f.fin_ms);
    partes::tiempos(&mut s, &r, &c, fin);
    s.terminar()
}
