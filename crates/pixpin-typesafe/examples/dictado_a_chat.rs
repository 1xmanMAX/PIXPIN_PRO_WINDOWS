//! **Prototipo 10: dictar al chat.** «Anota esto en el chat de la obra» →
//! en que chat (Choice sobre los nombres de `# chat: …`, con un Noul de «se
//! nombra alguno») y que se anota (Choice entre los textos que saca el
//! codigo: lo que sigue a «:» o a «que», o la frase anterior para «esto»).
//! `vivo`, parte `chats`; la tabla, en `dictado`.
//!
//! `cargo run -q -p pixpin-typesafe --example dictado_a_chat -- dictado.txt --rapido`
//!
//! Hechos: «nota en <chat|¿qué chat?>: <texto>».

mod comun;

use comun::partes::{self, Reunion};
use pixpin_typesafe::vivo::{self, Partes};

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar(
        "dictado_a_chat",
        "«anota esto en el chat de…» → chat y texto",
    );
    let r = Reunion::de(&s);
    println!("   chats: {}\n", r.chats.join(" · "));
    let p = Partes {
        chats: &r.chats,
        ..Default::default()
    };
    s.por_frase(|s, _, antes, f| {
        let ctx = r.contexto(antes, f);
        if let Some((sen, _)) = s.preguntar(|c| vivo::preguntar(c, &ctx, &p)) {
            partes::dictado(s, f, &sen, &r.chats);
        }
    });
    s.terminar()
}
