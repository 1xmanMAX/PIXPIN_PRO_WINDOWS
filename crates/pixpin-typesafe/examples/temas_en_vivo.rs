//! **Prototipo 1: los temas de la agenda que se marcan solos, en vivo.**
//! Corre la logica de verdad de `pixpin_typesafe::temas` (el `Rollo` de los
//! ultimos 40 s, el `Ritmo` de las peticiones y la tabla de `Seguimiento`)
//! sobre un guion, frase a frase, y dice a que hora se marca cada tema
//! frente a la hora de la frase.
//!
//! `cargo run -q -p pixpin-typesafe --example temas_en_vivo -- coordinacion.txt --rapido`
//!
//! Hechos para las esperas: «tratado <tema>».

mod comun;

use pixpin_typesafe::temas::{self, Cuando, Dicho, Pendiente, Ritmo, Rollo, Seguimiento};

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar(
        "temas_en_vivo",
        "que temas de la agenda se tratan, marcados mientras se habla",
    );
    let reunion = s.guion.valor("reunion").unwrap_or("").to_string();
    let agenda = s.guion.valores("agenda");
    let mut pendientes: Vec<Pendiente> = agenda
        .iter()
        .enumerate()
        .map(|(i, t)| Pendiente {
            id: format!("t{}", i + 1),
            texto: t.clone(),
        })
        .collect();
    for p in &pendientes {
        println!("   agenda {}: {}", p.id, p.texto);
    }
    println!();
    let frases = s.guion.frases.clone();
    let mut rollo = Rollo::default();
    let mut ritmo = Ritmo::default();
    let mut seguimiento = Seguimiento::default();
    let mut i = 0;
    loop {
        let ahora = s.reloj.ahora();
        // Lo que ya se dijo entra al rollo.
        while i < frases.len() && frases[i].fin_ms <= ahora {
            let f = &frases[i];
            println!("[{}] {}: {}", comun::hora(f.ms), f.quien, f.dice);
            rollo.meter(Dicho {
                ms: f.ms,
                fin_ms: f.fin_ms,
                // Como en la app: el texto de Whisper, sin quien lo dice.
                texto: f.dice.clone(),
                reloj_fin: f.fin_ms as u64,
            });
            i += 1;
        }
        let siguiente = frases.get(i).map(|f| f.fin_ms);
        let preguntar = !pendientes.is_empty()
            && match ritmo.cuando(ahora as u64, rollo.hay_nuevo()) {
                Cuando::Ya => true,
                Cuando::Esperar(ms) => {
                    let cuando = ahora + ms as i64;
                    if siguiente.is_none_or(|n| cuando < n) {
                        s.reloj.esperar_hasta(cuando);
                    } else if let Some(n) = siguiente {
                        s.reloj.esperar_hasta(n);
                    }
                    continue;
                }
                Cuando::Nada => false,
            };
        if !preguntar {
            match siguiente {
                Some(n) => {
                    s.reloj.esperar_hasta(n);
                    continue;
                }
                None => break,
            }
        }
        let Some(v) = rollo.ventana() else { continue };
        ritmo.sale(ahora as u64);
        println!(
            "    → pregunta por {} temas pendientes (lo nuevo desde {})",
            pendientes.len(),
            comun::hora(v.ms_ahora)
        );
        let r = s.preguntar(|c| temas::preguntar(c, &reunion, &agenda, &v, &pendientes));
        let vuelta = s.reloj.ahora();
        ritmo.vuelve(vuelta as u64, r.is_some());
        let Some((senales, _modelo)) = r else {
            continue;
        };
        rollo.mirado(&v);
        for (id, x) in &senales {
            if x.tratado >= 0.3 || x.ahora >= 0.3 {
                println!(
                    "      {id}: tratado {:.2} · cerrado {:.2} · ahora {:.2}",
                    x.tratado, x.cerrado, x.ahora
                );
            }
        }
        for t in seguimiento.decidir(&v, &senales) {
            let texto = pendientes
                .iter()
                .find(|p| p.id == t.id)
                .map(|p| p.texto.clone())
                .unwrap_or_default();
            pendientes.retain(|p| p.id != t.id);
            println!(
                "    ✓ «{texto}» marcado a las {} · empezo a tratarse a las {} · la ultima frase mirada acabo a las {} (+{:.1} s)",
                comun::hora(vuelta),
                comun::hora(t.ms),
                comun::hora(v.reloj_ultima as i64),
                (vuelta - v.reloj_ultima as i64) as f64 / 1000.0
            );
            s.hecho(vuelta, format!("tratado {texto}"));
        }
    }
    println!();
    for p in &pendientes {
        println!("   sin marcar: {}", p.texto);
    }
    s.terminar()
}
