//! **Mas casos A3: la urgencia de una tarea por su contexto** (`urgencia`,
//! fuera de la reunion: la lista de tareas). Cada caso es una tarea con su
//! lista, su chat, los mensajes recientes, los dias que faltan para su hora
//! (los cuenta el codigo) y la marca del usuario; Jev da la consecuencia
//! (Score) y tres Nouls, y la tabla de puntos de `urgencia::decidir` la
//! prioridad. Sin `dias`, los de la fecha del titulo de su lista
//! (`urgencia::dias_de_la_lista`), con la lista creada el dia `creada` y
//! contados desde `hoy` (del caso o de la cabecera del guion), como Tareas.
//!
//! `cargo run -q -p pindesk-typesafe --example urgencia_suelta -- urgencia.txt --rapido`
//!
//! Hechos: «u01 → urgente|alta|normal|baja».

mod comun;

use comun::casos::{self, p};
use pindesk_typesafe::fechas::Fecha;
use pindesk_typesafe::urgencia::{self, Contexto, Marca};

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
        let dia = |k: &str| c.valor(k).or(s.guion.valor(k)).and_then(Fecha::de_texto);
        let de_la_lista = match (dia("creada"), dia("hoy")) {
            (Some(creada), Some(hoy)) => urgencia::dias_de_la_lista(&ctx.lista, creada, hoy),
            _ => None,
        };
        let dias = c.numero("dias").map(|d| d as i64).or(de_la_lista);
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
