//! **Mas casos A3: la urgencia de una tarea por su contexto** (`urgencia`,
//! fuera de la reunion: la lista de tareas). Cada caso es una tarea con su
//! lista, su chat, los mensajes recientes, los dias que faltan para su hora
//! (los cuenta el codigo) y la marca del usuario; Jev da la consecuencia
//! (Score) y tres Nouls, y la tabla de puntos de `urgencia::decidir` la
//! prioridad.
//!
//! `cargo run -q -p pixpin-typesafe --example urgencia_suelta -- urgencia.txt --rapido`
//!
//! Hechos: «u01 → urgente|alta|normal|baja».

mod comun;

use comun::casos::{self, p};
use pixpin_typesafe::urgencia::{self, Contexto, Marca};

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar(
        "urgencia_suelta",
        "la prioridad de una tarea por su contexto",
    );
    for c in casos::casos(&s.guion) {
        let ctx = Contexto {
            tarea: c.texto("tarea"),
            lista: c.texto("lista"),
            chat: c.texto("chat"),
            recientes: c.valores("mensaje"),
        };
        let dias = c.numero("dias").map(|d| d as i64);
        let marca = match c.valor("marca") {
            Some("urgente") => Marca::Urgente,
            Some("muy urgente") => Marca::MuyUrgente,
            _ => Marca::Ninguna,
        };
        println!(
            "── {}: «{}» (lista {}, chat {}, {} mensajes, dias {:?})",
            c.id,
            ctx.tarea,
            ctx.lista,
            ctx.chat,
            ctx.recientes.len(),
            dias
        );
        let Some(x) = s.preguntar(|cl| urgencia::preguntar(cl, &ctx)) else {
            continue;
        };
        println!(
            "    consecuencia {} {:?} · bloquea {} · plazo ajeno {} · piden ya {} · puntos {:.1}",
            p(x.consecuencia.valor),
            x.consecuencia
                .niveles
                .iter()
                .map(|v| p(*v))
                .collect::<Vec<_>>(),
            p(x.bloquea),
            p(x.plazo_ajeno),
            x.piden_ya.map_or("—".to_string(), p),
            urgencia::puntos(&x, dias)
        );
        let pr = urgencia::decidir(marca, Some(&x), dias);
        let ahora = s.reloj.ahora();
        s.hecho(ahora, format!("{} → {}", c.id, pr.id()));
    }
    s.terminar()
}
