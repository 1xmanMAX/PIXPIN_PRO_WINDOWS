//! **Mas casos B6: que aviso interrumpe** (`alertas`). Cada caso, un aviso
//! que llega (`# app:`, `# de:`, `# texto:`) y lo que hace el usuario en ese
//! momento (`# actividad: libre | reunion: Titulo | grabando |
//! presentando`, lo sabe la app). Una peticion por aviso y la tabla de
//! `alertas::decidir`.
//!
//! `cargo run -q -p pixpin-typesafe --example avisos_listos -- avisos.txt --rapido`
//!
//! Hechos: «a01 → interrumpir|mostrar|discreto|para despues|bandeja».

mod comun;

use comun::casos::{self, p};
use pixpin_typesafe::alertas::{self, Actividad, Aviso, Que};

fn actividad(t: &str) -> Actividad {
    let t = t.trim();
    match t.split_once(':') {
        Some((k, titulo)) if k.trim() == "reunion" => Actividad::EnReunion(titulo.trim().into()),
        _ => match t {
            "grabando" => Actividad::Grabando,
            "presentando" => Actividad::Presentando,
            _ => Actividad::Libre,
        },
    }
}

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar("avisos_listos", "que aviso interrumpe y cual espera");
    for c in casos::casos(&s.guion) {
        let a = Aviso {
            app: c.texto("app"),
            de: c.texto("de"),
            texto: c.texto("texto"),
        };
        let act = actividad(&c.texto("actividad"));
        println!("── {}: [{:?}] {} · {}: {}", c.id, act, a.app, a.de, a.texto);
        let Some((q, x)) = s.preguntar(|cl| alertas::decidir_aviso(cl, &a, &act)) else {
            continue;
        };
        println!(
            "    rutina {} · pide ya {} · grave {} · de la reunion {}",
            p(x.rutina),
            p(x.pide_ya),
            p(x.grave),
            x.de_la_reunion.map_or("—".to_string(), p)
        );
        let extra = match q {
            Que::Interrumpir { sonido: false } => " (sin sonido)",
            _ => "",
        };
        let ahora = s.reloj.ahora();
        s.hecho(ahora, format!("{} → {}{extra}", c.id, q.id()));
    }
    s.terminar()
}
