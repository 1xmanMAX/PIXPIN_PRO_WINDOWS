//! **Todas las ideas de frase a frase en UNA peticion por frase** (la
//! guia de TypeSafe: lo que comparte el estado va junto). Tareas dichas,
//! urgencia en vivo, decisiones, preguntas pendientes, citas, dictado,
//! tiempo por tema y tema olvidado comparten la frase, quien la dice y las
//! de antes: van todas en la misma peticion de `vivo`. Los temas en vivo
//! (`temas_en_vivo`) van aparte: su estado es otro (la ventana de 40 s) y
//! su ritmo tambien. El repaso oral, tambien aparte (otro estado).
//!
//! Comprueba las esperas de todos esos prototipos sobre el mismo guion:
//! si juntar las preguntas empeora alguna respuesta, se ve aqui.
//!
//! `cargo run -q -p pixpin-typesafe --example reunion_completa -- coordinacion.txt --rapido`

mod comun;

use comun::partes::{self, Reunion};
use pixpin_typesafe::citas::Horario;
use pixpin_typesafe::pendientes::Pendientes;
use pixpin_typesafe::tiempo_de_temas::{Avisador, Cronometro};
use pixpin_typesafe::vivo::{self, Partes};

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar(
        "reunion_completa",
        "todas las ideas de frase a frase en una sola peticion por frase",
    );
    s.tambien = vec![
        "tareas_dichas",
        "urgencia_en_vivo",
        "decisiones",
        "preguntas_pendientes",
        "fechas_al_horario",
        "dictado_a_chat",
        "tiempo_por_tema",
        "tema_olvidado",
    ];
    let r = Reunion::de(&s);
    let mut tareas = partes::tareas_del_guion(&mut s, r.hoy);
    let mut pend = Pendientes::default();
    let mut horario = Horario::default();
    let mut c = Cronometro::nuevo(r.agenda.len());
    let minutos: i64 = s
        .guion
        .valor("duracion")
        .and_then(|d| d.parse().ok())
        .unwrap_or(30);
    let mut a = Avisador::nuevo(minutos * 60_000);
    s.por_frase(|s, _, antes, f| {
        let ids: Vec<(String, String)> = tareas
            .iter()
            .map(|t| (t.id.clone(), t.texto.clone()))
            .collect();
        let abiertas = pend.para_preguntar();
        let p = Partes {
            compromiso: true,
            decision: true,
            pregunta: true,
            cita: true,
            cierre: true,
            tema: true,
            tareas: &ids,
            abiertas: &abiertas,
            chats: &r.chats,
        };
        let anterior = r.tema_anterior(&c);
        let mut ctx = r.contexto(antes, f);
        ctx.tema_anterior = anterior.as_deref();
        let n = vivo::preparar(&ctx, &p).peticion.questions.len();
        println!("    → {n} preguntas en una peticion");
        let sen = s.preguntar(|cl| vivo::preguntar(cl, &ctx, &p));
        if let Some((sen, fe)) = &sen {
            partes::fechas(fe);
            partes::tareas(s, f, sen, fe, r.hoy);
            partes::urgencias(s, &mut tareas, sen, fe, r.hoy);
            partes::decision(s, f, sen);
            partes::preguntas(s, &mut pend, f, sen);
            partes::cita(s, &mut horario, f, sen, fe);
            partes::dictado(s, f, sen, &r.chats);
        }
        let solo = sen.as_ref().map(|x| &x.0);
        partes::tema(s, &r, &mut c, f, solo);
        partes::olvidados(s, &r, &c, &mut a, solo);
    });
    println!();
    partes::preguntas_al_final(&mut s, &pend);
    let fin = s.guion.frases.last().map_or(0, |f| f.fin_ms);
    partes::tiempos(&mut s, &r, &c, fin);
    s.terminar()
}
