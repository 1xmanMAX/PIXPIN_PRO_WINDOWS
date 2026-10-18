//! **Lo que se reconoce al escribir una tarea** (17-oct-2026).
//!
//! El usuario: «añadir el reconocimiento de palabras en las tareas de forma
//! sencilla: que reconozca la prioridad baja/media/alta y recordatorio […];
//! ponerle tags con el hashtag». Al apuntar «llamar al banco prioridad alta
//! recordar a las 5 #casa» se guarda la tarea «llamar al banco» con
//! prioridad alta, la etiqueta `#casa` y su hora de recordar.
//!
//! **No hay campos nuevos**: como la fecha de creacion (`➕`) y la hora de
//! recordar (`⏰`, `tareas::RELOJ`), todo va DENTRO del texto de la tarea,
//! que es lo unico que viaja al movil. Asi el movil (que no sabe nada de
//! esto) lo ensena tal cual y nada se pierde:
//!
//! ```text
//! - [ ] llamar al banco ⏫ #casa 💬 no abren hasta el lunes ⏰ 2026-10-17 17:00 ➕ 2026-10-17
//! ```
//!
//! - La **prioridad** son los emoticonos de las tareas de Obsidian (el mismo
//!   Markdown de casillas que ya usamos): `⏫` alta, `🔼` media, `🔽` baja.
//!   Al ser emoticonos, la tarjeta los pinta como sus chapitas de siempre
//!   (`emoticonos_de`), sin otra pieza.
//! - Las **etiquetas** son `#palabra`, como en Obsidian y en cualquier
//!   buscador: buscar `#casa` en la ventana las encuentra.
//! - La **nota** de por que no se esta haciendo (la del pin de la tarea) va
//!   tras `💬`, delante de la hora y de la fecha, que tienen que seguir al
//!   final para que `mini::partir` y `tareas::hora_de_tarea` las lean.
//!
//! La fecha dicha en español la entiende `pixpin_ia::fechas` (el mismo que
//! usa el asistente), sin reloj propio: quien llama da el «ahora».

#![forbid(unsafe_code)]

use pixpin_ia::fechas::{self, Momento};
use pixpin_proyecto::mini;

/// Lo que marca la nota de una tarea.
pub const NOTA: char = '💬';

/// La prioridad de una tarea.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Prioridad {
    Baja,
    Media,
    Alta,
}

impl Prioridad {
    /// El emoticono que la guarda en el texto (los de Obsidian Tasks).
    pub fn marca(self) -> &'static str {
        match self {
            Prioridad::Alta => "⏫",
            Prioridad::Media => "🔼",
            Prioridad::Baja => "🔽",
        }
    }

    fn de_marca(c: char) -> Option<Prioridad> {
        match c {
            '⏫' => Some(Prioridad::Alta),
            '🔼' => Some(Prioridad::Media),
            '🔽' => Some(Prioridad::Baja),
            _ => None,
        }
    }

    fn de_palabra(p: &str) -> Option<Prioridad> {
        match p {
            "alta" | "alto" | "urgente" | "maxima" => Some(Prioridad::Alta),
            "media" | "medio" | "normal" => Some(Prioridad::Media),
            "baja" | "bajo" | "minima" => Some(Prioridad::Baja),
            _ => None,
        }
    }
}

/// Una tarea recien escrita, ya entendida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frase {
    /// Lo que hay que hacer, sin lo reconocido.
    pub titulo: String,
    pub prioridad: Option<Prioridad>,
    /// Sin la almohadilla, en el orden escrito y sin repetir.
    pub etiquetas: Vec<String>,
    /// Cuando recordarla, en hora local.
    pub hora: Option<Momento>,
}

impl Frase {
    /// El texto que se guarda: el titulo con sus marcas detras.
    pub fn texto(&self) -> String {
        let mut s = self.titulo.clone();
        if let Some(p) = self.prioridad {
            s.push(' ');
            s.push_str(p.marca());
        }
        for e in &self.etiquetas {
            s.push_str(" #");
            s.push_str(e);
        }
        if let Some(h) = self.hora {
            s = super::con_hora(&s, ms_de(h));
        }
        s.trim().to_string()
    }
}

/// El momento local de ahora.
pub fn ahora() -> Momento {
    Momento::de_ms_locales(pixpin_shell::entorno::ahora_local_ms())
}

/// Los milisegundos locales de un momento.
pub fn ms_de(m: Momento) -> i64 {
    (m.dia * 1440 + i64::from(m.minuto)) * 60_000
}

/// Lo que dice que lo que sigue es cuando recordarla.
fn es_aviso(p: &str) -> bool {
    matches!(
        p,
        "recordar"
            | "recuerdame"
            | "recordarme"
            | "recordatorio"
            | "recuerda"
            | "avisar"
            | "avisame"
            | "avisarme"
            | "aviso"
            | "alarma"
    )
}

/// Una palabra para compararla: minusculas, sin tildes ni puntuacion.
fn llana(p: &str) -> String {
    pixpin_ia::acciones::normalizar(p.trim_matches(|c: char| !c.is_alphanumeric() && c != '!'))
}

/// La etiqueta de una palabra `#algo`, sin la almohadilla ni la puntuacion
/// de detras. `None` si no lo es (`#` suelto, `#1` sigue siendo un numero).
fn etiqueta_de(p: &str) -> Option<String> {
    let resto = p.strip_prefix('#')?;
    let e: String = resto
        .trim_end_matches(|c: char| !c.is_alphanumeric())
        .to_string();
    let valida = !e.is_empty()
        && e.chars().any(char::is_alphabetic)
        && e.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-');
    valida.then_some(e)
}

/// **Entiende una tarea recien escrita.** `ahora`: el momento local.
///
/// Lo que no se entiende se queda en el titulo, nunca se adivina: una fecha
/// que no existe o que ya paso («recordar el 31 de febrero») no pone hora y
/// se lee tal cual, para que el usuario vea que no se entendio.
pub fn leer(texto: &str, ahora: Momento) -> Frase {
    let mut prioridad = None;
    let mut etiquetas: Vec<String> = Vec::new();
    // 1. Las etiquetas y la prioridad, esten donde esten.
    let crudas: Vec<&str> = texto.split_whitespace().collect();
    let mut quedan: Vec<&str> = Vec::with_capacity(crudas.len());
    let mut i = 0;
    while i < crudas.len() {
        let p = crudas[i];
        if let Some(e) = etiqueta_de(p) {
            if !etiquetas.iter().any(|x| x.eq_ignore_ascii_case(&e)) {
                etiquetas.push(e);
            }
            i += 1;
            continue;
        }
        let l = llana(p);
        let sig = crudas.get(i + 1).map(|s| llana(s));
        // «prioridad alta», «prio alta», «con prioridad alta».
        if matches!(l.as_str(), "prioridad" | "prio" | "prioritaria")
            && let Some(pr) = sig.as_deref().and_then(Prioridad::de_palabra)
        {
            prioridad = Some(pr);
            if quedan.last().is_some_and(|q| llana(q) == "con") {
                quedan.pop();
            }
            i += 2;
            continue;
        }
        // «alta prioridad».
        if let Some(pr) = Prioridad::de_palabra(&l)
            && sig.as_deref() == Some("prioridad")
        {
            prioridad = Some(pr);
            if quedan.last().is_some_and(|q| llana(q) == "con") {
                quedan.pop();
            }
            i += 2;
            continue;
        }
        // «!alta», «!!!» y «urgente» sueltos.
        if let Some(pr) = l.strip_prefix('!').and_then(Prioridad::de_palabra) {
            prioridad = Some(pr);
            i += 1;
            continue;
        }
        if p == "!!!" || l == "urgente" {
            prioridad = Some(Prioridad::Alta);
            i += 1;
            continue;
        }
        if let Some(pr) = p.chars().next().and_then(Prioridad::de_marca)
            && p.chars().count() == 1
        {
            prioridad = Some(pr);
            i += 1;
            continue;
        }
        quedan.push(p);
        i += 1;
    }
    // 2. La hora: tras «recordar» / «avisame»…, o una fecha al final.
    let mut hora = None;
    let mut palabras: Vec<String> = quedan.iter().map(|s| s.to_string()).collect();
    if let Some(k) = palabras.iter().position(|p| es_aviso(&llana(p))) {
        // La cola mas larga tras la palabra que sea una fecha entera; lo que
        // sobre detras vuelve al titulo («recordar mañana comprar pan»).
        let mut hallada = None;
        for j in (k + 2..=palabras.len()).rev() {
            if let Some(m) = fecha_de(&palabras[k + 1..j].join(" "), ahora) {
                hallada = Some((j, m));
                break;
            }
        }
        if let Some((j, m)) = hallada {
            hora = Some(m);
            palabras.drain(k..j);
        } else if k > 0 && k + 1 == palabras.len() {
            // «recordar» suelto al final: no dice cuando; se quita igual.
            palabras.remove(k);
        }
    }
    if hora.is_none() {
        let frase = palabras.join(" ");
        if let Ok((cabeza, Some(m))) = fechas::separar(&frase, ahora)
            // Un numero suelto al final («leer el capitulo 5») no es una hora:
            // sin «recordar» delante, la hora tiene que decirse como hora.
            && frase[cabeza.len()..].trim().parse::<u64>().is_err()
        {
            let m = por_la_tarde(&frase[cabeza.len()..], ahora).unwrap_or(m);
            hora = Some(m);
            palabras = cabeza.split_whitespace().map(str::to_string).collect();
        }
    }
    let titulo = palabras
        .join(" ")
        .trim_end_matches([',', ';', ':', ' ', '-'])
        .trim()
        .to_string();
    Frase {
        titulo,
        prioridad,
        etiquetas,
        hora,
    }
}

/// Una fecha dicha, con la hora de la tarde si es lo que se queria decir.
fn fecha_de(frase: &str, ahora: Momento) -> Option<Momento> {
    let m = fechas::interpretar(frase, ahora).ok()?;
    Some(por_la_tarde(frase, ahora).unwrap_or(m))
}

/// **«A las 5» es por la tarde.** Nadie apunta una tarea para las cinco de
/// la madrugada sin decirlo: una hora suelta de la 1 a las 7, sin «de la
/// mañana», «am» ni minutos, se lee como de la tarde (las 17:00). Devuelve
/// `None` si la frase no es de esas o ya dice la franja.
fn por_la_tarde(frase: &str, ahora: Momento) -> Option<Momento> {
    let q = pixpin_ia::acciones::normalizar(frase);
    let palabras: Vec<&str> = q.split_whitespace().collect();
    let franja = ["de la manana", "por la manana", "madrugada", "tarde", "noche", "mediodia"]
        .iter()
        .any(|f| q.contains(f))
        || palabras.iter().any(|p| matches!(*p, "am" | "pm"));
    if franja {
        return None;
    }
    let suelta = palabras.windows(2).any(|w| {
        matches!(w[0], "las" | "la") && w[1].parse::<u32>().is_ok_and(|n| (1..=7).contains(&n))
    });
    if !suelta {
        return None;
    }
    fechas::interpretar(&format!("{frase} de la tarde"), ahora).ok()
}

// ----------------------------------------------- leer lo guardado

/// Lo que lleva una tarea ya guardada, para ensenarla.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Marcas {
    /// Lo que se lee: sin prioridad, etiquetas, nota ni hora.
    pub titulo: String,
    pub prioridad: Option<Prioridad>,
    pub etiquetas: Vec<String>,
    pub nota: Option<String>,
    /// La hora de recordar, en ms locales.
    pub hora: Option<i64>,
}

/// Las marcas de un texto de tarea (lo que se ve, sin `➕` ni imagenes; o
/// el crudo entero: la fecha y las imagenes no estorban).
pub fn marcas(texto: &str) -> Marcas {
    let (visible, _) = mini::partir(texto);
    let (visible, _) = mini::imagenes_de(visible);
    let hora = super::hora_de_tarea(&visible);
    let mut base = super::sin_hora(&visible);
    let mut nota = None;
    if let Some(i) = base.find(NOTA) {
        let n = base[i + NOTA.len_utf8()..].trim().to_string();
        if !n.is_empty() {
            nota = Some(n);
        }
        base.truncate(i);
    }
    let mut prioridad = None;
    let mut etiquetas = Vec::new();
    let mut quedan = Vec::new();
    for p in base.split_whitespace() {
        let mut cs = p.chars();
        if let (Some(c), None) = (cs.next(), cs.next())
            && let Some(pr) = Prioridad::de_marca(c)
        {
            prioridad = Some(pr);
            continue;
        }
        if let Some(e) = etiqueta_de(p) {
            if !etiquetas.contains(&e) {
                etiquetas.push(e);
            }
            continue;
        }
        quedan.push(p);
    }
    Marcas {
        titulo: quedan.join(" "),
        prioridad,
        etiquetas,
        nota,
        hora,
    }
}

/// **Lo que ensena la tarjeta** de una tarea: el texto y sus chapitas. El
/// titulo con sus `#etiquetas` detras y, si tiene, la hora de recordar
/// legible («⏰ hoy 17:00») y la nota; las chapitas son sus emoticonos con la
/// prioridad la primera. Sustituye a `emoticonos_de` sobre el texto crudo,
/// que dejaba la fecha del reloj suelta en el renglon.
pub fn a_la_vista(texto: &str, ahora: Option<Momento>) -> (String, Vec<String>) {
    let m = marcas(texto);
    let (mut limpio, mut emos) = super::emoticonos::emoticonos_de(&m.titulo);
    if let Some(p) = m.prioridad {
        emos.retain(|e| e != p.marca());
        emos.insert(0, p.marca().to_string());
    }
    for e in &m.etiquetas {
        limpio.push_str(" #");
        limpio.push_str(e);
    }
    if let Some(h) = m.hora {
        let cuando = match ahora {
            Some(a) => Momento::de_ms_locales(h).legible(a),
            None => {
                let mm = Momento::de_ms_locales(h);
                let (y, mo, d) = fechas::fecha_de(mm.dia);
                format!("{d}/{mo}/{y} {:02}:{:02}", mm.minuto / 60, mm.minuto % 60)
            }
        };
        limpio.push_str(&format!(" · ⏰ {cuando}"));
    }
    if let Some(n) = &m.nota {
        limpio.push_str(&format!(" · {NOTA} {n}"));
    }
    (limpio.trim().to_string(), emos)
}

/// Una nota en un renglon y sin las marcas que confundirian al leerla.
fn nota_limpia(nota: &str) -> String {
    nota.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .filter(|c| *c != super::RELOJ && *c != mini::MARCA_DE_CREADA && *c != NOTA)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// **El texto crudo de una tarea con otra nota** (vacia: sin nota). La nota
/// va tras `💬`, delante de las imagenes, la hora y la fecha, que se quedan
/// como estaban.
pub fn con_nota(crudo: &str, nota: &str) -> String {
    let (visible, creada) = mini::partir(crudo);
    let (cabeza, cola) = match super::hora_de_tarea(visible) {
        Some(t) => (super::sin_hora(visible), Some(super::marca_de_hora(t))),
        None => (visible.to_string(), None),
    };
    let (texto, imagenes) = mini::imagenes_de(&cabeza);
    let sin_nota = match texto.find(NOTA) {
        Some(i) => texto[..i].trim_end().to_string(),
        None => texto,
    };
    let nota = nota_limpia(nota);
    let mut s = sin_nota;
    if !nota.is_empty() {
        s = format!("{} {NOTA} {nota}", s.trim_end());
    }
    let mut s = mini::con_imagenes(&s, &imagenes);
    if let Some(c) = cola {
        s = format!("{} {c}", s.trim_end());
    }
    match creada {
        Some(f) => mini::con_fecha(&s, f),
        None => s.trim().to_string(),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Miercoles 14 de octubre de 2026, 10:30 (el de las pruebas de fechas).
    fn ahora() -> Momento {
        Momento {
            dia: fechas::dias_de(2026, 10, 14),
            minuto: 10 * 60 + 30,
        }
    }

    fn a(dia: u32, h: u32, m: u32) -> Option<Momento> {
        Some(Momento {
            dia: fechas::dias_de(2026, 10, i64::from(dia)),
            minuto: h * 60 + m,
        })
    }

    #[test]
    fn la_frase_del_usuario_se_entiende_entera() {
        let f = leer("llamar al banco prioridad alta recordar a las 5 #casa", ahora());
        assert_eq!(f.titulo, "llamar al banco");
        assert_eq!(f.prioridad, Some(Prioridad::Alta));
        assert_eq!(f.etiquetas, ["casa"]);
        assert_eq!(f.hora, a(14, 17, 0), "las 5 son las 17:00 de hoy");
        assert_eq!(f.texto(), "llamar al banco ⏫ #casa ⏰ 2026-10-14 17:00");
    }

    #[test]
    fn prioridades_dichas_de_varias_formas() {
        let p = |t: &str| leer(t, ahora()).prioridad;
        assert_eq!(p("comprar pan prioridad baja"), Some(Prioridad::Baja));
        assert_eq!(p("comprar pan con prioridad media"), Some(Prioridad::Media));
        assert_eq!(p("revisar planos alta prioridad"), Some(Prioridad::Alta));
        assert_eq!(p("pagar luz !alta"), Some(Prioridad::Alta));
        assert_eq!(p("pagar luz urgente"), Some(Prioridad::Alta));
        assert_eq!(p("pagar luz 🔽"), Some(Prioridad::Baja));
        assert_eq!(leer("comprar pan con prioridad media", ahora()).titulo, "comprar pan");
        // Casos negativos: «alta» o «media» sueltas son palabras del titulo.
        assert_eq!(p("subir a la parte alta"), None);
        assert_eq!(leer("subir a la parte alta", ahora()).titulo, "subir a la parte alta");
        assert_eq!(p("media naranja"), None);
    }

    #[test]
    fn las_etiquetas_se_quitan_del_titulo_y_no_se_repiten() {
        let f = leer("#obra pedir yeso #Obra #urgente-ya, #1", ahora());
        assert_eq!(f.titulo, "pedir yeso #1", "#1 es un numero, no una etiqueta");
        assert_eq!(f.etiquetas, ["obra", "urgente-ya"]);
        assert_eq!(f.prioridad, None);
    }

    #[test]
    fn el_recordatorio_tras_su_palabra_o_al_final() {
        let h = |t: &str| leer(t, ahora()).hora;
        assert_eq!(h("llamar a Ana recuérdame mañana a las 10"), a(15, 10, 0));
        assert_eq!(h("entregar informe el viernes"), a(16, 9, 0));
        assert_eq!(h("pagar alquiler 20/10"), a(20, 9, 0));
        assert_eq!(h("cita médico avisame a las 18:30"), a(14, 18, 30));
        assert_eq!(h("recoger paquete a las 9 de la mañana"), a(15, 9, 0));
        // Lo que sigue a la fecha vuelve al titulo.
        let f = leer("recordar mañana comprar pan", ahora());
        assert_eq!((f.titulo.as_str(), f.hora), ("comprar pan", a(15, 9, 0)));
        assert_eq!(leer("llamar a Ana recuérdame mañana a las 10", ahora()).titulo, "llamar a Ana");
    }

    #[test]
    fn caso_negativo_lo_que_no_es_fecha_no_pone_hora() {
        for t in ["leer el capítulo 5", "recordar el 31 de febrero", "comprar 3 kilos", "recordar"] {
            let f = leer(t, ahora());
            assert_eq!(f.hora, None, "{t}");
        }
        assert_eq!(leer("recordar el 31 de febrero", ahora()).titulo, "recordar el 31 de febrero");
        assert_eq!(leer("leer el capítulo 5", ahora()).titulo, "leer el capítulo 5");
        // Sin nada que reconocer, el texto tal cual.
        let f = leer("  regar las plantas ", ahora());
        assert_eq!(f.texto(), "regar las plantas");
    }

    #[test]
    fn las_marcas_guardadas_se_leen_para_ensenarlas() {
        let crudo = "llamar al banco ⏫ #casa 💬 cierran a las 3 ⏰ 2026-10-14 17:00 ➕ 2026-10-14";
        let m = marcas(crudo);
        assert_eq!(m.titulo, "llamar al banco");
        assert_eq!(m.prioridad, Some(Prioridad::Alta));
        assert_eq!(m.etiquetas, ["casa"]);
        assert_eq!(m.nota.as_deref(), Some("cierran a las 3"));
        assert_eq!(m.hora, a(14, 17, 0).map(ms_de));
        let (texto, emos) = a_la_vista(crudo, Some(ahora()));
        assert_eq!(texto, "llamar al banco #casa · ⏰ hoy 17:00 · 💬 cierran a las 3");
        assert_eq!(emos, ["⏫"]);
        // Una tarea de antes, sin marcas: igual que con `emoticonos_de`.
        assert_eq!(a_la_vista("pan 🍞", None), ("pan".to_string(), vec!["🍞".to_string()]));
    }

    #[test]
    fn la_nota_entra_y_sale_sin_tocar_la_hora_la_fecha_ni_las_imagenes() {
        let crudo = "yeso ⏫ ![img 01](pixpin:files/a.png) ⏰ 2026-10-14 17:00 ➕ 2026-10-10";
        let con = con_nota(crudo, "falta\nel camión ⏰");
        assert_eq!(
            con,
            "yeso ⏫ 💬 falta el camión ![img 01](pixpin:files/a.png) ⏰ 2026-10-14 17:00 ➕ 2026-10-10"
        );
        assert_eq!(super::super::hora_de_tarea(&con), a(14, 17, 0).map(ms_de));
        assert_eq!(mini::partir(&con).1, mini::partir(crudo).1, "la fecha sigue");
        assert_eq!(mini::imagenes_de(mini::partir(&con).0).1.len(), 1);
        // Otra nota cambia la de antes; vacia la quita.
        let otra = con_nota(&con, "mañana");
        assert_eq!(marcas(&otra).nota.as_deref(), Some("mañana"));
        assert_eq!(otra.matches(NOTA).count(), 1);
        let sin = con_nota(&otra, "  ");
        assert_eq!(sin, crudo);
        // Sin hora ni fecha.
        assert_eq!(con_nota("pan", "no hay"), "pan 💬 no hay");
    }
}
