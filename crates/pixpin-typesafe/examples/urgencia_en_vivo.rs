//! **Prototipo 3: la urgencia de una tarea que cambia al hablar.** Con las
//! tareas de la cabecera (`# tarea: texto | 2026-11-02`), su prioridad de
//! partida la da `urgencia` (una peticion por tarea, como la app); luego,
//! frase a frase, dos Nouls por tarea («¿es de esta tarea?», «¿la apura?»)
//! y el plazo dicho (`vivo`, parte `tareas`) cambian sus entradas
//! (`urgencia_viva`), y la tabla de `urgencia` vuelve a decidir.
//!
//! `cargo run -q -p pixpin-typesafe --example urgencia_en_vivo -- coordinacion.txt --rapido`
//!
//! Hechos: «fecha <tarea>: <antes> → <despues>», «prioridad <tarea>:
//! <antes> → <despues>».

mod comun;

use comun::partes::{self, Reunion};
use pixpin_typesafe::vivo::{self, Partes};

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar(
        "urgencia_en_vivo",
        "la prioridad de las tareas que ya existen, segun lo que se dice",
    );
    let r = Reunion::de(&s);
    let mut tareas = partes::tareas_del_guion(&mut s, r.hoy);
    s.por_frase(|s, _, antes, f| {
        let ids: Vec<(String, String)> = tareas
            .iter()
            .map(|t| (t.id.clone(), t.texto.clone()))
            .collect();
        let p = Partes {
            tareas: &ids,
            ..Default::default()
        };
        let ctx = r.contexto(antes, f);
        let Some((sen, fe)) = s.preguntar(|c| vivo::preguntar(c, &ctx, &p)) else {
            return;
        };
        partes::fechas(&fe);
        partes::urgencias(s, &mut tareas, &sen, &fe, r.hoy);
    });
    println!();
    for t in &tareas {
        println!(
            "   {} {}: {} ({})",
            t.id,
            t.texto,
            t.prioridad.id(),
            t.fecha.map_or("sin fecha".into(), |f| f.to_string())
        );
    }
    s.terminar()
}
