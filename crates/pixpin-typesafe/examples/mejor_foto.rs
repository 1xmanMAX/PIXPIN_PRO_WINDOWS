//! **Mas casos B3: la mejor foto de una rafaga** (`mejor_foto`, con
//! imagenes). Cada caso es una rafaga (`# foto: fotos/r01_1.jpg`, varias);
//! una peticion con todas las fotos en el estado, cinco Nouls por foto y
//! una Choice de cual se quedaria una persona; la tabla de
//! `mejor_foto::decidir` elige. Las fotos son inventadas
//! (`guiones/fotos/hacer_fotos.py`).
//!
//! `cargo run -q -p pixpin-typesafe --example mejor_foto -- rafagas.txt --rapido`
//!
//! Hechos: «r01 elige foto_2», «r01 descarta foto_1 (movida)», «r05
//! ninguna buena».

mod comun;

use comun::casos::{self, p};
use pixpin_typesafe::mejor_foto;

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar("mejor_foto", "la mejor foto de cada rafaga");
    for c in casos::casos(&s.guion) {
        let rutas = c.valores("foto");
        let fotos: Vec<_> = rutas.iter().map(|r| casos::imagen(&s.guion, r)).collect();
        println!("── {}: {}", c.id, rutas.join(", "));
        let Some((e, x)) = s.preguntar(|cl| mejor_foto::elegir(cl, &fotos)) else {
            continue;
        };
        for (i, u) in x.iter().enumerate() {
            println!(
                "    foto_{}: nitida {} · ojos cerrados {} · oscura {} · quemada {} · torcida {} · mejor {}",
                i + 1,
                p(u.nitida),
                p(u.ojos_cerrados),
                p(u.oscura),
                p(u.quemada),
                p(u.torcida),
                p(u.mejor)
            );
        }
        let Some(e) = e else { continue };
        let ahora = s.reloj.ahora();
        for (i, m) in &e.descartadas {
            let m: Vec<&str> = m.iter().map(|m| m.id()).collect();
            s.hecho(
                ahora,
                format!("{} descarta foto_{} ({})", c.id, i + 1, m.join(", ")),
            );
        }
        s.hecho(ahora, format!("{} elige foto_{}", c.id, e.elegida + 1));
        if !e.buena {
            s.hecho(ahora, format!("{} ninguna buena", c.id));
        }
    }
    s.terminar()
}
