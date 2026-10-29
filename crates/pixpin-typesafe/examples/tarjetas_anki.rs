//! **Mas casos A2: que tarjetas de Anki valen la pena** (`anki`). Cada caso
//! es una tarjeta (su minuto y lo dicho en su tramo); un Score de lo util y
//! un Noul de examen por tarjeta, la tabla de `anki::veredicto` y, al
//! final, `anki::elegir` (cuales se mandan).
//!
//! `cargo run -q -p pixpin-typesafe --example tarjetas_anki -- anki.txt --rapido`
//!
//! Hechos: «t01 queda|dudosa|sobra» y, al final, «t01 se manda» o «t01 no
//! se manda».

mod comun;

use comun::casos::{self, p};
use pixpin_typesafe::anki::{self, Senales, Tramo};

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar("tarjetas_anki", "que tarjetas de una clase valen la pena");
    let clase = s.guion.valor("clase").unwrap_or("").to_string();
    let lista = casos::casos(&s.guion);
    let mut senales: Vec<Option<Senales>> = Vec::new();
    for c in &lista {
        let t = Tramo {
            minuto: c.texto("minuto"),
            dicho: c.texto("dicho"),
        };
        println!("── {} [{}] {}", c.id, t.minuto, t.dicho);
        if t.dicho.split_whitespace().count() < 3 {
            println!("    (sin nada dicho: no se pregunta, queda)");
            senales.push(None);
            continue;
        }
        let x = s.preguntar(|cl| Senales::de(&cl.preguntar(&anki::peticion(&clase, &t))?));
        if let Some(x) = &x {
            println!(
                "    util (nivel 2-3) {} · niveles {:?} · examen {}",
                p(x.util()),
                x.valor.niveles.iter().map(|v| p(*v)).collect::<Vec<_>>(),
                p(x.examen)
            );
            let v = match anki::veredicto(x) {
                anki::Veredicto::Queda => "queda",
                anki::Veredicto::Dudosa => "dudosa",
                anki::Veredicto::Sobra => "sobra",
            };
            let ahora = s.reloj.ahora();
            s.hecho(ahora, format!("{} {v}", c.id));
        }
        senales.push(x);
    }
    println!();
    let quedan = anki::elegir(&senales);
    for (c, q) in lista.iter().zip(quedan) {
        let ahora = s.reloj.ahora();
        s.hecho(
            ahora,
            format!("{} {}", c.id, if q { "se manda" } else { "no se manda" }),
        );
    }
    s.terminar()
}
