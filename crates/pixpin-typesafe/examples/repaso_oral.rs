//! **Prototipo 9: repaso oral.** Con los puntos clave de una clase
//! (`# punto: …`) y lo que el alumno va explicando frase a frase, que
//! puntos cubre, cuales dice mal y cuales faltan. Una peticion por frase
//! con lo explicado hasta ahora y dos Nouls por punto (`repaso`).
//!
//! `cargo run -q -p pixpin-typesafe --example repaso_oral -- repaso.txt --rapido`
//!
//! Hechos (al final): «cubierto|mal|falta|dudoso <punto>».

mod comun;

use pixpin_typesafe::repaso::{self, Repaso};

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar(
        "repaso_oral",
        "puntos clave cubiertos, mal dichos o que faltan",
    );
    let clase = s.guion.valor("clase").unwrap_or("").to_string();
    let mut r = Repaso::nuevo(&clase, &s.guion.valores("punto"));
    for (id, t) in &r.puntos {
        println!("   {id}: {t}");
    }
    println!();
    s.por_frase(|s, _, _, f| {
        r.oir(&f.dice);
        let puntos = r.por_preguntar();
        let Some(sen) = s.preguntar(|c| repaso::preguntar(c, &r.clase, &r.explicacion, &puntos))
        else {
            return;
        };
        for (id, x) in &sen {
            if x.cubre >= 0.3 || x.contradice >= 0.3 {
                println!(
                    "    {id}: cubre {:.2} · contradice {:.2}",
                    x.cubre, x.contradice
                );
            }
        }
        for (id, a, b) in r.decidir(&sen) {
            println!("    {id}: {} → {}", a.id(), b.id());
        }
    });
    println!();
    let fin = s.reloj.ahora();
    let lista: Vec<(String, &'static str)> = r
        .puntos
        .iter()
        .zip(&r.estados)
        .map(|((_, t), e)| (t.clone(), e.id()))
        .collect();
    for (t, e) in lista {
        s.hecho(fin, format!("{e} {t}"));
    }
    s.terminar()
}
