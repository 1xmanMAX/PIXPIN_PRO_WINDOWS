//! **Mas casos B2: etiquetas solas para notas y capturas** (`etiquetas`).
//! Las etiquetas del usuario en la cabecera (`# etiqueta: Facturas`); cada
//! caso, una nota (titulo y texto). Un Noul por etiqueta y la tabla de
//! `etiquetas::decidir`.
//!
//! `cargo run -q -p pixpin-typesafe --example etiquetas_solas -- etiquetas.txt --rapido`
//!
//! Hechos: «n01 + Facturas» (puesta), «n01 ? Viajes» (sugerida), «n01 sin
//! etiquetas».

mod comun;

use comun::casos::{self, p};
use pixpin_typesafe::etiquetas::{self, Nota};

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar("etiquetas_solas", "las etiquetas de cada nota");
    let lista = s.guion.valores("etiqueta");
    println!("   etiquetas: {}\n", lista.join(", "));
    for c in casos::casos(&s.guion) {
        let n = Nota {
            titulo: c.texto("titulo"),
            texto: c.texto("texto"),
        };
        println!(
            "── {}: [{}] {}",
            c.id,
            n.titulo,
            n.texto.replace('\n', " / ")
        );
        let Some((e, probs)) = s.preguntar(|cl| etiquetas::elegir(cl, &n, &lista)) else {
            continue;
        };
        let mut orden: Vec<(usize, f64)> = probs.iter().copied().enumerate().collect();
        orden.sort_by(|a, b| b.1.total_cmp(&a.1));
        println!(
            "    {}",
            orden
                .iter()
                .take(4)
                .map(|(i, q)| format!("{} {}", lista[*i], p(*q)))
                .collect::<Vec<_>>()
                .join(" · ")
        );
        let ahora = s.reloj.ahora();
        if e.puestas.is_empty() && e.sugeridas.is_empty() {
            s.hecho(ahora, format!("{} sin etiquetas", c.id));
        }
        for (i, _) in &e.puestas {
            s.hecho(ahora, format!("{} + {}", c.id, lista[*i]));
        }
        for (i, _) in &e.sugeridas {
            s.hecho(ahora, format!("{} ? {}", c.id, lista[*i]));
        }
    }
    s.terminar()
}
