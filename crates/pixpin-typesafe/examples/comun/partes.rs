//! **Lo que hace cada prototipo con las senales de una frase**: escribirlo
//! y apuntar lo decidido. Las decisiones son de la biblioteca; aqui solo se
//! llaman y se ensenan. Lo usan los prototipos sueltos y `reunion_completa`
//! (todas las partes en una sola peticion por frase).

use pixpin_typesafe::citas::{self, Horario};
use pixpin_typesafe::compromisos;
use pixpin_typesafe::dictado;
use pixpin_typesafe::fechas::{Expresion, Fecha};
use pixpin_typesafe::pendientes::{Evento, Pendientes};
use pixpin_typesafe::tiempo_de_temas::{self, Avisador, Aviso, Cronometro, Destino};
use pixpin_typesafe::urgencia::{self, Contexto as CtxUrgencia};
use pixpin_typesafe::urgencia_viva::{self, TareaViva};
use pixpin_typesafe::vivo::{self, Contexto, Frase, Senales};

use super::{Sesion, hora};

/// Lo que se sabe de la reunion por la cabecera del guion.
pub struct Reunion {
    pub titulo: String,
    pub participantes: Vec<String>,
    pub agenda: Vec<String>,
    pub chats: Vec<String>,
    pub hoy: Fecha,
}

impl Reunion {
    pub fn de(s: &Sesion) -> Reunion {
        Reunion {
            titulo: s.guion.valor("reunion").unwrap_or("").to_string(),
            participantes: s.guion.lista("participantes"),
            agenda: s.guion.valores("agenda"),
            chats: s.guion.valores("chat"),
            hoy: s.guion.fecha(),
        }
    }

    pub fn contexto<'a>(&'a self, antes: &'a [Frase], frase: &'a Frase) -> Contexto<'a> {
        Contexto {
            reunion: &self.titulo,
            participantes: &self.participantes,
            agenda: &self.agenda,
            antes,
            frase,
            hoy: self.hoy,
        }
    }

    /// El nombre de un destino.
    pub fn tema(&self, d: Destino) -> String {
        match d {
            Destino::Tema(i) => self.agenda.get(i).cloned().unwrap_or_default(),
            Destino::Otro => "otro".into(),
        }
    }
}

/// Las expresiones de fecha que encontro el codigo.
pub fn fechas(f: &[Expresion]) {
    if !f.is_empty() {
        let v: Vec<String> = f
            .iter()
            .map(|e| format!("«{}» = {}", e.texto, e.momento))
            .collect();
        println!("    fechas en la frase (codigo): {}", v.join(" · "));
    }
}

/// **Tareas dichas.** Hecho: «tarea <responsable> <fecha|sin fecha>
/// <prioridad>: <frase>».
pub fn tareas(s: &mut Sesion, f: &Frase, sen: &Senales, fe: &[Expresion], hoy: Fecha) {
    let Some(c) = &sen.compromiso else { return };
    println!(
        "    compromiso {:.2} · quien_dicho {} · quien {} · plazo_dicho {} · urgente {:.2}",
        c.p,
        c.quien_dicho.map_or("-".into(), |p| format!("{p:.2}")),
        c.quien
            .as_ref()
            .map_or("-".into(), |(n, p)| format!("{n} ({p:.2})")),
        sen.plazo.map_or("-".into(), |p| format!(
            "{:.2} → «{}»",
            p.p,
            fe.get(p.cual).map_or("", |e| e.texto.as_str())
        )),
        c.urgente
    );
    match compromisos::decidir(f, c, sen.plazo.as_ref(), fe, hoy) {
        compromisos::Resultado::Propuesta(t) => {
            let fecha = t.fecha.map_or("sin fecha".into(), |m| m.fecha.to_string());
            let detalle = match (&t.fecha, &t.expresion) {
                (Some(m), Some(e)) => format!(" [{m}, de «{e}»]"),
                _ => String::new(),
            };
            s.hecho_de(
                "tareas_dichas",
                f.ms,
                format!(
                    "tarea {} {fecha} {}: {}{detalle}",
                    t.responsable,
                    t.prioridad.id(),
                    t.texto
                ),
            );
        }
        compromisos::Resultado::Dudosa(p) => println!("    (tarea dudosa {p:.2}: no se propone)"),
        compromisos::Resultado::Nada => {}
    }
}

/// Las tareas de la cabecera (`# tarea: texto | 2026-11-02`), con la
/// urgencia de Jev de cada una (una peticion por tarea, como la app).
pub fn tareas_del_guion(s: &mut Sesion, hoy: Fecha) -> Vec<TareaViva> {
    let titulo = s.guion.valor("reunion").unwrap_or("").to_string();
    let mut v = Vec::new();
    for (i, l) in s.guion.valores("tarea").iter().enumerate() {
        let (texto, fecha) = match l.split_once('|') {
            Some((t, f)) => (t.trim().to_string(), Fecha::de_texto(f)),
            None => (l.trim().to_string(), None),
        };
        let ctx = CtxUrgencia {
            tarea: texto.clone(),
            lista: titulo.clone(),
            ..Default::default()
        };
        println!(
            "   tarea t{}: {texto} ({})",
            i + 1,
            fecha.map_or("sin fecha".into(), |f| f.to_string())
        );
        let senales = s.preguntar(|c| urgencia::preguntar(c, &ctx));
        let t = TareaViva::nueva(&format!("t{}", i + 1), &texto, fecha, senales, hoy);
        println!("    prioridad de partida: {}", t.prioridad.id());
        v.push(t);
    }
    println!();
    v
}

/// **Urgencia en vivo.** Hechos: «prioridad <tarea>: <antes> → <despues>»
/// y «fecha <tarea>: <antes> → <despues>».
pub fn urgencias(
    s: &mut Sesion,
    tareas: &mut [TareaViva],
    sen: &Senales,
    fe: &[Expresion],
    hoy: Fecha,
) {
    let plazo = compromisos::plazo_elegido(sen.plazo.as_ref(), fe);
    for (id, st) in &sen.tareas {
        let Some(t) = tareas.iter_mut().find(|t| t.id == *id) else {
            continue;
        };
        if st.sobre >= 0.3 || st.apura >= 0.3 {
            println!(
                "    {id} «{}»: sobre {:.2} · apura {:.2}",
                t.texto, st.sobre, st.apura
            );
        }
        if let Some(c) = urgencia_viva::actualizar(t, st, plazo, hoy) {
            let ms = s.reloj.ahora();
            if c.fecha_antes != c.fecha_despues {
                let f = |x: Option<Fecha>| x.map_or("sin fecha".into(), |f| f.to_string());
                s.hecho_de(
                    "urgencia_en_vivo",
                    ms,
                    format!(
                        "fecha {}: {} → {}",
                        t.texto,
                        f(c.fecha_antes),
                        f(c.fecha_despues)
                    ),
                );
            }
            if c.antes != c.despues {
                s.hecho_de(
                    "urgencia_en_vivo",
                    ms,
                    format!(
                        "prioridad {}: {} → {}",
                        t.texto,
                        c.antes.id(),
                        c.despues.id()
                    ),
                );
            }
        }
    }
}

/// **Decisiones.** Hecho: «decisión [m:ss]: <frase>».
pub fn decision(s: &mut Sesion, f: &Frase, sen: &Senales) {
    let Some(p) = sen.decision else { return };
    if p >= 0.3 {
        println!("    decision {p:.2}");
    }
    if vivo::es_decision(sen) {
        s.hecho_de(
            "decisiones",
            f.ms,
            format!("decisión: {} [{}]", f.dice, hora(f.ms)),
        );
    }
}

/// **Preguntas pendientes.** Hechos: «abierta: …», «contestada <pregunta>
/// por <quien>: <frase>».
pub fn preguntas(s: &mut Sesion, p: &mut Pendientes, f: &Frase, sen: &Senales) {
    if let Some(x) = sen.pregunta.filter(|x| *x >= 0.3) {
        println!("    pregunta {x:.2}");
    }
    for ev in p.tras_frase(f, sen.pregunta, &sen.respondidas) {
        match ev {
            Evento::Abierta(a) => s.hecho_de(
                "preguntas_pendientes",
                f.ms,
                format!("abierta: {}", a.texto),
            ),
            Evento::Contestada(c) => s.hecho_de(
                "preguntas_pendientes",
                f.ms,
                format!(
                    "contestada {} por {}: {}",
                    c.pregunta.texto, c.quien, f.dice
                ),
            ),
            Evento::Quiza { id, p } => {
                println!("    {id} quiza contestada ({p:.2}): sigue abierta")
            }
        }
    }
}

/// Al final: las que nadie contesto. Hecho: «sin respuesta <pregunta>».
pub fn preguntas_al_final(s: &mut Sesion, p: &Pendientes) {
    for a in p.abiertas() {
        s.hecho_de(
            "preguntas_pendientes",
            a.ms,
            format!(
                "sin respuesta {} (de {}, a las {})",
                a.texto,
                a.quien,
                hora(a.ms)
            ),
        );
    }
}

/// **Al horario.** Hecho: «evento <momento>: <frase>».
pub fn cita(s: &mut Sesion, h: &mut Horario, f: &Frase, sen: &Senales, fe: &[Expresion]) {
    if let Some(c) = &sen.cita {
        println!(
            "    cita {:.2} → «{}»",
            c.p,
            fe.get(c.cual).map_or("", |e| e.texto.as_str())
        );
    }
    match h.proponer(f, sen.cita.as_ref(), fe) {
        citas::Resultado::Evento(e) => s.hecho_de(
            "fechas_al_horario",
            f.ms,
            format!("evento {}: {} (de «{}»)", e.momento, e.titulo, e.expresion),
        ),
        citas::Resultado::Repetido(m) => println!("    (cita repetida: {m})"),
        citas::Resultado::Dudosa(p) => println!("    (cita dudosa {p:.2})"),
        citas::Resultado::Nada => {}
    }
}

/// **Dictado.** Hecho: «nota en <chat|¿que chat?>: <texto>».
pub fn dictado(s: &mut Sesion, f: &Frase, sen: &Senales, chats: &[String]) {
    if let Some(d) = &sen.dictado
        && d.p >= 0.3
    {
        println!(
            "    dictado {:.2} · chat dicho {:.2} → «{}» · texto «{}»",
            d.p,
            d.chat_dicho,
            chats.get(d.chat).map_or("", |c| c.as_str()),
            d.texto.as_deref().unwrap_or("")
        );
    }
    match dictado::decidir(f, sen.dictado.as_ref(), chats) {
        dictado::Resultado::Nota(n) => s.hecho_de(
            "dictado_a_chat",
            f.ms,
            format!(
                "nota en {}: {}",
                n.chat.as_deref().unwrap_or("¿qué chat?"),
                n.texto
            ),
        ),
        dictado::Resultado::SinTexto => println!("    (dictado sin nada que anotar)"),
        dictado::Resultado::Dudoso(p) => println!("    (dictado dudoso {p:.2})"),
        dictado::Resultado::Nada => {}
    }
}

/// **Tiempo por tema**: a que tema va la frase. Hecho: «<tema>: <frase>».
pub fn tema(s: &mut Sesion, r: &Reunion, c: &mut Cronometro, f: &Frase, sen: Option<&Senales>) {
    let t = sen.and_then(|x| x.tema.as_ref());
    if let Some(t) = t {
        println!(
            "    de la agenda {:.2} · tema «{}» ({:.2})",
            t.de_la_agenda,
            r.agenda.get(t.cual).map_or("", |s| s.as_str()),
            t.p_cual
        );
    }
    let d = tiempo_de_temas::asignar(t, c.anterior());
    c.frase(f.ms, d);
    s.hecho_de(
        "tiempo_por_tema",
        f.ms,
        format!("{}: {}", r.tema(d), f.dice),
    );
}

/// El tiempo de cada tema, contado hasta `fin`.
pub fn tiempos(s: &mut Sesion, r: &Reunion, c: &Cronometro, fin: i64) {
    let h = c.hasta(fin);
    println!();
    println!("   tiempo por tema:");
    let mut mayor: Option<(usize, i64)> = None;
    for (i, ms) in h.por_tema.iter().enumerate() {
        println!("     {:>7}  {}", hora(*ms), r.agenda[i]);
        if mayor.is_none_or(|(_, m)| *ms > m) {
            mayor = Some((i, *ms));
        }
    }
    println!("     {:>7}  (otro)", hora(h.otro));
    if let Some((i, _)) = mayor {
        s.hecho_de(
            "tiempo_por_tema",
            fin,
            format!("más tiempo: {}", r.agenda[i]),
        );
    }
}

/// **Tema olvidado**: los avisos tras una frase. Hechos: «aviso queda
/// poco…», «aviso tiempo cumplido…», «aviso están cerrando…».
pub fn olvidados(
    s: &mut Sesion,
    r: &Reunion,
    c: &Cronometro,
    a: &mut Avisador,
    sen: Option<&Senales>,
) {
    let ahora = s.reloj.ahora();
    let actual = c.anterior();
    let sin: Vec<usize> = c
        .hasta(ahora)
        .sin_tratar()
        .into_iter()
        .filter(|i| actual != Some(Destino::Tema(*i)))
        .collect();
    let cierre = sen.and_then(|x| x.cierre);
    if let Some(p) = cierre.filter(|p| *p >= 0.3) {
        println!("    cierre {p:.2}");
    }
    let nombres = |v: &[usize]| {
        v.iter()
            .map(|i| r.agenda[*i].clone())
            .collect::<Vec<_>>()
            .join(", ")
    };
    for av in a.avisar(ahora, cierre, &sin) {
        let t = match av {
            Aviso::QuedaPoco { quedan_ms, temas } => {
                format!(
                    "aviso queda poco (quedan {}): {}",
                    hora(quedan_ms),
                    nombres(&temas)
                )
            }
            Aviso::TiempoCumplido { temas } => {
                format!("aviso tiempo cumplido: {}", nombres(&temas))
            }
            Aviso::Cierran { temas } => {
                format!("aviso están cerrando y quedan: {}", nombres(&temas))
            }
        };
        s.hecho_de("tema_olvidado", ahora, t);
    }
}
