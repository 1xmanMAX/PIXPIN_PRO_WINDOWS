//! **Mas casos A4: que modelo contesta al asistente** (`modelo`, lo que
//! hace `jev::modelo::segun_jev` antes de cada turno del chat). Cada caso
//! es un pedido (y, si hay, los mensajes de antes: `# reciente: usuario:
//! …`); Jev da la dificultad (Score) y tres Nouls, y la tabla de
//! `modelo::decidir` elige el rapido o el fuerte. Las ordenes del camino
//! rapido de la app no llegan aqui (no preguntan).
//!
//! `cargo run -q -p pixpin-typesafe --example elegir_modelo -- modelo.txt --rapido`
//!
//! Hechos: «m01 → rapido|fuerte (motivo)».

mod comun;

use comun::casos::{self, p};
use pixpin_typesafe::modelo::{self, Contexto, Motivo, Ruta};

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar(
        "elegir_modelo",
        "el modelo rapido o el fuerte para cada pedido",
    );
    for c in casos::casos(&s.guion) {
        let ctx = Contexto {
            pedido: c.texto("pedido"),
            recientes: c
                .valores("reciente")
                .iter()
                .map(|r| match r.split_once(':') {
                    Some((q, t)) => (q.trim().to_string(), t.trim().to_string()),
                    None => ("usuario".to_string(), r.clone()),
                })
                .collect(),
        };
        println!("── {}: «{}»", c.id, ctx.pedido);
        let Some(x) =
            s.preguntar(|cl| modelo::Senales::de(&cl.preguntar(&modelo::peticion(&ctx))?))
        else {
            continue;
        };
        println!(
            "    dificultad {} {:?} · orden {} · razonar {} · importa {}",
            p(x.dificultad.valor),
            x.dificultad
                .niveles
                .iter()
                .map(|v| p(*v))
                .collect::<Vec<_>>(),
            p(x.orden),
            p(x.razonar),
            p(x.importa)
        );
        let (ruta, motivo) = modelo::decidir(&x);
        let ruta = match ruta {
            Ruta::Rapido => "rapido",
            Ruta::Fuerte => "fuerte",
        };
        let motivo = match motivo {
            Motivo::Orden => "orden",
            Motivo::Razonar => "razonar",
            Motivo::Dificil => "dificil",
            Motivo::Importante => "importante",
            Motivo::Facil => "facil",
            Motivo::SinDecidir => "sin decidir",
        };
        let ahora = s.reloj.ahora();
        s.hecho(ahora, format!("{} → {ruta} ({motivo})", c.id));
    }
    s.terminar()
}
