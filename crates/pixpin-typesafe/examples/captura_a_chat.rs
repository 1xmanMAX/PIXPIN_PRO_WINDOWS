//! **Mas casos B1: a que chat va una captura** (`captura_a_chat`). Los
//! chats del usuario en la cabecera (`# chat: Nombre | de que trata`); cada
//! caso, una captura: el titulo de su ventana y lo que leyo el OCR. Una
//! peticion por captura (¿es de algun chat? y ¿de cual?) y la tabla de
//! `captura_a_chat::decidir`.
//!
//! `cargo run -q -p pixpin-typesafe --example captura_a_chat -- capturas.txt --rapido`
//!
//! Hechos: «c01 → obra_calle_mayor · guardar», «c02 → x · sugerir (o y)»,
//! «c03 → ninguno».

mod comun;

use comun::casos::{self, p};
use pixpin_typesafe::captura_a_chat::{self, Captura, Chat, Destino};

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar("captura_a_chat", "a que chat va cada captura");
    let chats: Vec<Chat> = s
        .guion
        .valores("chat")
        .iter()
        .map(|c| match c.split_once('|') {
            Some((n, t)) => Chat {
                nombre: n.trim().to_string(),
                trata: t.trim().to_string(),
            },
            None => Chat {
                nombre: c.trim().to_string(),
                trata: String::new(),
            },
        })
        .collect();
    let (_, claves) = captura_a_chat::peticion(&Captura::default(), &chats);
    for (k, c) in claves.iter().zip(&chats) {
        println!("   chat {k}: {} ({})", c.nombre, c.trata);
    }
    println!();
    for c in casos::casos(&s.guion) {
        let cap = Captura {
            ventana: c.texto("ventana"),
            texto: c.texto("texto"),
        };
        println!(
            "── {}: [{}] {}",
            c.id,
            cap.ventana,
            cap.texto.replace('\n', " / ")
        );
        let Some((d, x)) = s.preguntar(|cl| captura_a_chat::elegir(cl, &cap, &chats)) else {
            continue;
        };
        println!(
            "    de algun chat {} · {}",
            p(x.de_alguno),
            x.chats
                .iter()
                .take(3)
                .map(|(i, q)| format!("{} {}", claves[*i], p(*q)))
                .collect::<Vec<_>>()
                .join(" · ")
        );
        let h = match d {
            Destino::Guardar(i) => format!("{} → {} · guardar", c.id, claves[i]),
            Destino::Sugerir(v) => format!(
                "{} → {} · sugerir{}",
                c.id,
                claves[v[0]],
                v.get(1)
                    .map(|i| format!(" (o {})", claves[*i]))
                    .unwrap_or_default()
            ),
            Destino::Ninguno => format!("{} → ninguno", c.id),
        };
        let ahora = s.reloj.ahora();
        s.hecho(ahora, h);
    }
    s.terminar()
}
