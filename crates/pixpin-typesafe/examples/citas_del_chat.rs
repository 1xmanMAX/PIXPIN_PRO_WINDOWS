//! **Mas casos B7: citas de los mensajes de un chat al horario**
//! (`horario_de_chat`, fuera de las reuniones). El guion es un chat
//! (`[m:ss] Quien: mensaje`, `# chat:` y `# fecha:`); por mensaje con fechas
//! o con una propuesta de otro pendiente, una peticion, y la tabla de
//! `Horario::actualizar` (propuesta → pendiente; aceptada en otro mensaje o
//! acordada → evento).
//!
//! `cargo run -q -p pixpin-typesafe --example citas_del_chat -- chat_citas.txt --rapido`
//!
//! Hechos: «evento jue 2026-10-29 17:00 (aceptado)», «pendiente jue
//! 2026-10-29 17:00».

mod comun;

use comun::casos::p;
use pixpin_typesafe::horario_de_chat::{self, Horario, Mensaje, Resultado};

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar("citas_del_chat", "citas propuestas y aceptadas en un chat");
    let chat = s.guion.valor("chat").unwrap_or("").to_string();
    let hoy = s.guion.fecha();
    let mut h = Horario::default();
    let mut antes: Vec<Mensaje> = Vec::new();
    let frases = s.guion.frases.clone();
    for f in &frases {
        s.reloj.esperar_hasta(f.fin_ms);
        println!("[{}] {}: {}", comun::hora(f.ms), f.quien, f.dice);
        let m = Mensaje {
            de: f.quien.clone(),
            texto: f.dice.clone(),
        };
        let prep = horario_de_chat::preparar(&chat, &antes, &m, hoy, h.pendiente.as_ref());
        antes.push(m.clone());
        if prep.peticion.is_none() {
            continue;
        }
        if !prep.fechas.is_empty() {
            println!(
                "    fechas: {}",
                prep.fechas
                    .iter()
                    .map(|e| format!("«{}» = {}", e.texto, e.momento))
                    .collect::<Vec<_>>()
                    .join(" · ")
            );
        }
        let Some(Some(x)) = s.preguntar(|cl| horario_de_chat::preguntar(cl, &prep)) else {
            continue;
        };
        println!(
            "    cita {} · acordada {} · cual {} · acepta {}",
            x.cita.map_or("—".to_string(), p),
            p(x.acordada),
            x.cual,
            x.acepta.map_or("—".to_string(), p)
        );
        let ahora = s.reloj.ahora();
        match h.actualizar(&m, &x, &prep.fechas) {
            Resultado::Evento(e) => s.hecho(
                ahora,
                format!(
                    "evento {}{}: {}",
                    e.momento,
                    if e.aceptado { " (aceptado)" } else { "" },
                    e.titulo
                ),
            ),
            Resultado::Pendiente(x) => s.hecho(ahora, format!("pendiente {}", x.momento)),
            Resultado::Repetido(m) => println!("    (repetido: {m})"),
            Resultado::Dudosa(q) => println!("    (dudosa: {})", p(q)),
            Resultado::Nada => {}
        }
    }
    println!("\n   horario:");
    for e in &h.eventos {
        println!("     {} «{}»", e.momento, e.titulo);
    }
    s.terminar()
}
