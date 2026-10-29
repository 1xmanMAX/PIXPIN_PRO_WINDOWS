//! **Lo que se dice en vivo, frase a frase** (29-oct-2026, ver
//! `docs/probar-ideas-sin-interfaz.md`): **una sola peticion por frase**
//! que sirve a varias ideas a la vez, porque todas comparten el estado (la
//! frase, quien la dice y las dos de antes). Cada idea pone sus preguntas
//! si se pide ([`Partes`]) y lee sus respuestas ([`Senales`]); lo que se
//! hace con ellas lo deciden las tablas de cada modulo:
//!
//! | parte | preguntas (`preguntas::vivo`) | decide |
//! |---|---|---|
//! | `compromiso` | `compromiso`, `quien_dicho`, `quien`, `urgente` | `crate::compromisos` |
//! | `plazo` (con `compromiso` o con tareas) | `plazo_dicho`, `plazo` | `compromisos`, `urgencia_viva` |
//! | `tareas` | `sobre_<id>`, `apura_<id>` por tarea | `crate::urgencia_viva` |
//! | `decision` | `decision` | [`es_decision`] |
//! | `pregunta` y `abiertas` | `pregunta`, `respondida_<id>` por pregunta | `crate::pendientes` |
//! | `cita` | `cita`, `cita_cuando` | `crate::citas` |
//! | `tema` y `cierre` | `de_la_agenda`, `tema`, `cierre` | `crate::tiempo_de_temas` |
//! | `chats` | `dictado`, `dictado_chat_dicho`, `dictado_chat`, `dictado_que` | `crate::dictado` |
//!
//! Las fechas las busca el codigo ([`crate::fechas::buscar`]) y Jev solo
//! elige cual es el plazo o la cita: sin ninguna expresion de fecha en la
//! frase no se pregunta por plazos ni citas (no hay nada que elegir). Con
//! una sola, no hace falta la Choice: basta el Noul. Las preguntas
//! especulativas (quien, cual plazo, que chat) van en la misma peticion y
//! solo se leen si su Noul dice «si».
//!
//! `participantes` y `agenda` solo van en el estado si van las preguntas
//! que los nombran (un campo de mas puede torcer otra respuesta).

use serde_json::{Map, Value, json};

use crate::fechas::{self, Expresion, Fecha};
use crate::preguntas::vivo as q;
use crate::{Cliente, Error, Peticion, Respuestas};

/// **Una frase dicha**: el milisegundo de la reunion en que empieza, quien
/// la dice y lo que dice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frase {
    pub ms: i64,
    pub quien: String,
    pub dice: String,
}

/// Cuantas frases de antes van de contexto.
pub const ANTES: usize = 2;
/// Lo largo de la frase y de cada una de antes.
const LARGO_FRASE: usize = 600;
const LARGO_ANTES: usize = 300;

/// Lo que se sabe de la reunion cuando llega una frase.
#[derive(Debug, Clone, Copy)]
pub struct Contexto<'a> {
    pub reunion: &'a str,
    pub participantes: &'a [String],
    pub agenda: &'a [String],
    /// Las frases anteriores, de la mas vieja a la mas nueva (van las
    /// ultimas [`ANTES`]).
    pub antes: &'a [Frase],
    pub frase: &'a Frase,
    /// El dia de la reunion (para resolver «el viernes»).
    pub hoy: Fecha,
}

/// **Que ideas van en la peticion.**
#[derive(Debug, Clone, Copy, Default)]
pub struct Partes<'a> {
    pub compromiso: bool,
    pub decision: bool,
    pub pregunta: bool,
    pub cita: bool,
    pub cierre: bool,
    pub tema: bool,
    /// Las tareas que ya existen: (id, texto).
    pub tareas: &'a [(String, String)],
    /// Las preguntas aun sin respuesta: (id, texto).
    pub abiertas: &'a [(String, String)],
    /// Los chats donde se puede anotar.
    pub chats: &'a [String],
}

/// Si una frase trae bastante para preguntar (un «vale» suelto no).
pub fn vale_la_pena(f: &Frase) -> bool {
    f.dice.split_whitespace().count() >= crate::temas::PALABRAS_MINIMAS
}

/// La clave de una opcion: su texto en minusculas, sin tildes y con `_`
/// (el nombre de la opcion se le manda a Jev: mejor que «c1»).
pub fn clave(texto: &str) -> String {
    let mut s = String::new();
    for c in texto.trim().chars().flat_map(char::to_lowercase) {
        let c = match c {
            'á' | 'à' => 'a',
            'é' | 'è' => 'e',
            'í' | 'ì' => 'i',
            'ó' | 'ò' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            'ñ' => 'n',
            otro => otro,
        };
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('_') && !s.is_empty() {
            s.push('_');
        }
    }
    let s = s.trim_end_matches('_').chars().take(60).collect::<String>();
    if s.is_empty() { "x".to_string() } else { s }
}

/// Claves unicas para estos textos, en el mismo orden.
fn claves(textos: &[String]) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    for t in textos {
        let base = clave(t);
        let mut k = base.clone();
        let mut n = 2;
        while v.iter().any(|(x, _)| *x == k) || k == q::TODOS {
            k = format!("{base}_{n}");
            n += 1;
        }
        v.push((k, t.clone()));
    }
    v
}

/// **La peticion preparada**: la peticion y lo que hace falta para leer
/// sus respuestas (las opciones de cada Choice, las fechas de la frase).
#[derive(Debug, Clone)]
pub struct Preparada {
    pub peticion: Peticion,
    /// Las expresiones de fecha de la frase, en orden.
    pub fechas: Vec<Expresion>,
    quien: Vec<(String, String)>,
    tema: Vec<(String, String)>,
    chats: Vec<(String, String)>,
    /// Lo que se podria anotar: (clave, texto).
    pub textos_dictado: Vec<(String, String)>,
    compromiso: bool,
    plazo: bool,
    decision: bool,
    pregunta: bool,
    cita: bool,
    cierre: bool,
    tareas: Vec<String>,
    abiertas: Vec<String>,
}

/// El estado (ver `preguntas::vivo`).
pub fn estado(c: &Contexto, partes: &Partes) -> Value {
    let mut m = Map::new();
    if !c.reunion.trim().is_empty() {
        m.insert("reunion".into(), json!(crate::recortar(c.reunion, 120)));
    }
    if partes.compromiso && !c.participantes.is_empty() {
        m.insert("participantes".into(), json!(c.participantes));
    }
    if partes.tema && !c.agenda.is_empty() {
        let a: Vec<String> = c.agenda.iter().map(|t| crate::recortar(t, 160)).collect();
        m.insert("agenda".into(), json!(a));
    }
    let antes: Vec<String> = c
        .antes
        .iter()
        .rev()
        .take(ANTES)
        .rev()
        .map(|f| format!("{}: {}", f.quien, crate::recortar(&f.dice, LARGO_ANTES)))
        .collect();
    if !antes.is_empty() {
        m.insert("antes".into(), json!(antes));
    }
    m.insert(
        "frase".into(),
        json!({
            "quien": c.frase.quien,
            "dice": crate::recortar(&c.frase.dice, LARGO_FRASE),
        }),
    );
    Value::Object(m)
}

/// **Prepara la peticion** de una frase con las partes pedidas.
pub fn preparar(c: &Contexto, partes: &Partes) -> Preparada {
    let fechas = fechas::buscar(&c.frase.dice, c.hoy);
    let opciones_fecha = claves(&fechas.iter().map(|e| e.texto.clone()).collect::<Vec<_>>());
    let mut p = Peticion::nueva(estado(c, partes));
    let plazo = !fechas.is_empty() && (partes.compromiso || !partes.tareas.is_empty());
    let cita = partes.cita && !fechas.is_empty();
    let quien = claves(c.participantes);
    if partes.compromiso {
        p = p
            .con(q::COMPROMISO, q::compromiso())
            .con(q::URGENTE, q::urgente());
        if !quien.is_empty() {
            p = p
                .con(q::QUIEN_DICHO, q::quien_dicho())
                .con(q::QUIEN, q::quien(&quien, &c.frase.quien));
        }
    }
    if plazo {
        p = p.con(q::PLAZO_DICHO, q::plazo_dicho());
        if fechas.len() > 1 {
            p = p.con(q::PLAZO, q::plazo(&opciones_fecha));
        }
    }
    if partes.decision {
        p = p.con(q::DECISION, q::decision());
    }
    if partes.pregunta {
        p = p.con(q::PREGUNTA, q::pregunta());
    }
    // Las mas recientes primero (las viejas que no caben esperan).
    let mut abiertas = Vec::new();
    for (id, texto) in partes.abiertas.iter().rev().take(q::POR_PETICION) {
        p = p.con(
            &q::respondida(id),
            q::contesta(&crate::recortar(texto, 200)),
        );
        abiertas.push(id.clone());
    }
    if cita {
        p = p.con(q::CITA, q::cita());
        if fechas.len() > 1 {
            p = p.con(q::CITA_CUANDO, q::cita_cuando(&opciones_fecha));
        }
    }
    if partes.cierre {
        p = p.con(q::CIERRE, q::cierre());
    }
    let tema = claves(c.agenda);
    let con_tema = partes.tema && !tema.is_empty();
    if con_tema {
        p = p
            .con(q::DE_LA_AGENDA, q::de_la_agenda())
            .con(q::TEMA, q::tema(&tema));
    }
    let mut tareas = Vec::new();
    for (id, texto) in partes.tareas.iter().take(q::POR_PETICION) {
        let t = crate::recortar(texto, 200);
        p = p
            .con(&q::sobre(id), q::es_de_la_tarea(&t))
            .con(&q::apura(id), q::la_apura(&t));
        tareas.push(id.clone());
    }
    let chats = claves(partes.chats);
    let mut textos_dictado = Vec::new();
    if !chats.is_empty() {
        textos_dictado = crate::dictado::textos(c.frase, c.antes);
        p = p
            .con(q::DICTADO, q::dictado())
            .con(q::DICTADO_CHAT_DICHO, q::dictado_chat_dicho())
            .con(q::DICTADO_CHAT, q::dictado_chat(&chats));
        if textos_dictado.len() > 1 {
            p = p.con(q::DICTADO_QUE, q::dictado_que(&textos_dictado));
        }
    }
    Preparada {
        peticion: p,
        fechas,
        quien,
        tema: if con_tema { tema } else { Vec::new() },
        chats,
        textos_dictado,
        compromiso: partes.compromiso,
        plazo,
        decision: partes.decision,
        pregunta: partes.pregunta,
        cita,
        cierre: partes.cierre,
        tareas,
        abiertas,
    }
}

/// Lo que dijo Jev de un compromiso.
#[derive(Debug, Clone, PartialEq)]
pub struct Compromiso {
    pub p: f64,
    pub urgente: f64,
    /// `None` sin participantes (no se pregunto).
    pub quien_dicho: Option<f64>,
    /// El nombre mas probable y su probabilidad.
    pub quien: Option<(String, f64)>,
}

/// Lo que dijo del plazo: si hay uno y cual de [`Preparada::fechas`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plazo {
    pub p: f64,
    pub cual: usize,
}

/// Lo que dijo de una cita.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cita {
    pub p: f64,
    pub cual: usize,
}

/// Lo que dijo del tema.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tema {
    pub de_la_agenda: f64,
    /// El indice en la agenda del tema mas probable y su probabilidad.
    pub cual: usize,
    pub p_cual: f64,
}

/// Lo que dijo de una tarea.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SenalTarea {
    pub sobre: f64,
    pub apura: f64,
}

/// Lo que dijo de un dictado.
#[derive(Debug, Clone, PartialEq)]
pub struct Dictado {
    pub p: f64,
    pub chat_dicho: f64,
    /// El indice del chat mas probable.
    pub chat: usize,
    /// Lo que se anotaria (de [`Preparada::textos_dictado`]); `None` si no
    /// hay nada que anotar.
    pub texto: Option<String>,
}

/// **Todo lo que dijo Jev de una frase**, por partes (`None`: no se
/// pregunto).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Senales {
    pub compromiso: Option<Compromiso>,
    pub plazo: Option<Plazo>,
    pub decision: Option<f64>,
    pub pregunta: Option<f64>,
    pub cita: Option<Cita>,
    pub cierre: Option<f64>,
    pub tema: Option<Tema>,
    pub tareas: Vec<(String, SenalTarea)>,
    pub respondidas: Vec<(String, f64)>,
    pub dictado: Option<Dictado>,
}

/// La opcion elegida de una Choice y su indice entre `opciones`.
fn elegida(
    r: &Respuestas,
    nombre: &str,
    opciones: &[(String, String)],
) -> Result<(usize, f64), Error> {
    let (c, probs) = r.choice(nombre)?;
    let i = opciones
        .iter()
        .position(|(k, _)| k == c)
        .ok_or_else(|| Error::Formato(format!("{nombre}: opcion «{c}» que no se pidio")))?;
    Ok((i, probs.get(c).copied().unwrap_or(0.0)))
}

impl Preparada {
    /// **Lee las respuestas.** Lo que falte es un error de toda la frase.
    pub fn leer(&self, r: &Respuestas) -> Result<Senales, Error> {
        let mut s = Senales::default();
        if self.compromiso {
            let quien = if self.quien.is_empty() {
                None
            } else {
                let mut con_todos = self.quien.clone();
                con_todos.push((q::TODOS.into(), "todos".into()));
                let (i, p) = elegida(r, q::QUIEN, &con_todos)?;
                Some((con_todos[i].1.clone(), p))
            };
            s.compromiso = Some(Compromiso {
                p: r.noul(q::COMPROMISO)?,
                urgente: r.noul(q::URGENTE)?,
                quien_dicho: if self.quien.is_empty() {
                    None
                } else {
                    Some(r.noul(q::QUIEN_DICHO)?)
                },
                quien,
            });
        }
        if self.plazo {
            let cual = if self.fechas.len() > 1 {
                let opciones: Vec<(String, String)> = claves(
                    &self
                        .fechas
                        .iter()
                        .map(|e| e.texto.clone())
                        .collect::<Vec<_>>(),
                );
                elegida(r, q::PLAZO, &opciones)?.0
            } else {
                0
            };
            s.plazo = Some(Plazo {
                p: r.noul(q::PLAZO_DICHO)?,
                cual,
            });
        }
        if self.decision {
            s.decision = Some(r.noul(q::DECISION)?);
        }
        if self.pregunta {
            s.pregunta = Some(r.noul(q::PREGUNTA)?);
        }
        for id in &self.abiertas {
            s.respondidas
                .push((id.clone(), r.noul(&q::respondida(id))?));
        }
        if self.cita {
            let cual = if self.fechas.len() > 1 {
                let opciones: Vec<(String, String)> = claves(
                    &self
                        .fechas
                        .iter()
                        .map(|e| e.texto.clone())
                        .collect::<Vec<_>>(),
                );
                elegida(r, q::CITA_CUANDO, &opciones)?.0
            } else {
                0
            };
            s.cita = Some(Cita {
                p: r.noul(q::CITA)?,
                cual,
            });
        }
        if self.cierre {
            s.cierre = Some(r.noul(q::CIERRE)?);
        }
        if !self.tema.is_empty() {
            let (cual, p_cual) = elegida(r, q::TEMA, &self.tema)?;
            s.tema = Some(Tema {
                de_la_agenda: r.noul(q::DE_LA_AGENDA)?,
                cual,
                p_cual,
            });
        }
        for id in &self.tareas {
            s.tareas.push((
                id.clone(),
                SenalTarea {
                    sobre: r.noul(&q::sobre(id))?,
                    apura: r.noul(&q::apura(id))?,
                },
            ));
        }
        if !self.chats.is_empty() {
            let texto = match self.textos_dictado.len() {
                0 => None,
                1 => Some(self.textos_dictado[0].1.clone()),
                _ => {
                    let (i, _) = elegida(r, q::DICTADO_QUE, &self.textos_dictado)?;
                    Some(self.textos_dictado[i].1.clone())
                }
            };
            s.dictado = Some(Dictado {
                p: r.noul(q::DICTADO)?,
                chat_dicho: r.noul(q::DICTADO_CHAT_DICHO)?,
                chat: elegida(r, q::DICTADO_CHAT, &self.chats)?.0,
                texto,
            });
        }
        Ok(s)
    }
}

/// **Pregunta por una frase**: las senales y las fechas de la frase. Un
/// fallo es un error de la frase entera (nunca un «no»).
pub fn preguntar(
    cliente: &Cliente,
    c: &Contexto,
    partes: &Partes,
) -> Result<(Senales, Vec<Expresion>), Error> {
    let prep = preparar(c, partes);
    let r = cliente.preguntar(&prep.peticion)?;
    Ok((prep.leer(&r)?, prep.fechas))
}

/// **¿Se decidio algo?** `decision` ≥ SI: si; entre NO y SI: no se apunta
/// (una decision de mas en el acta es peor que una de menos).
pub fn es_decision(s: &Senales) -> bool {
    s.decision.is_some_and(|p| p >= crate::preguntas::SI)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::grabadas;
    use crate::transporte::Grabado;
    use std::sync::Arc;

    fn frase(quien: &str, dice: &str) -> Frase {
        Frase {
            ms: 60_000,
            quien: quien.into(),
            dice: dice.into(),
        }
    }

    fn hoy() -> Fecha {
        Fecha::nueva(2026, 10, 26).unwrap()
    }

    #[test]
    fn las_claves_de_las_opciones() {
        assert_eq!(clave("Cotización del viernes"), "cotizacion_del_viernes");
        assert_eq!(clave("  ¿Ana?  "), "ana");
        assert_eq!(clave("¡!"), "x");
        let v = claves(&["Ana".into(), "ana".into(), "Todos".into()]);
        assert_eq!(v[0].0, "ana");
        assert_eq!(v[1].0, "ana_2");
        assert_eq!(v[2].0, "todos_2", "«todos» es la opcion de todos");
    }

    #[test]
    fn el_estado_solo_lleva_lo_que_se_pregunta() {
        let gente = vec!["Ana".to_string(), "Luis".to_string()];
        let agenda = vec!["Paro de transportistas".to_string()];
        let antes = vec![
            frase("Luis", "uno"),
            frase("Ana", "dos"),
            frase("Luis", "tres"),
        ];
        let f = frase("Ana", "yo me encargo de mandar la cotización el viernes");
        let c = Contexto {
            reunion: "Coordinación",
            participantes: &gente,
            agenda: &agenda,
            antes: &antes,
            frase: &f,
            hoy: hoy(),
        };
        let solo_decision = Partes {
            decision: true,
            ..Default::default()
        };
        let e = estado(&c, &solo_decision);
        assert!(e.get("participantes").is_none());
        assert!(e.get("agenda").is_none());
        assert_eq!(e["antes"], json!(["Ana: dos", "Luis: tres"]));
        assert_eq!(e["frase"]["quien"], "Ana");
        let p = preparar(&c, &solo_decision);
        assert_eq!(p.peticion.questions.len(), 1);
        // Con compromiso: participantes, quien y el plazo (una sola fecha:
        // sin Choice de plazo).
        let p = preparar(
            &c,
            &Partes {
                compromiso: true,
                tema: true,
                ..Default::default()
            },
        );
        assert_eq!(p.peticion.state["participantes"], json!(["Ana", "Luis"]));
        assert!(p.peticion.state.get("agenda").is_some());
        let nombres: Vec<&str> = p.peticion.questions.keys().map(|s| s.as_str()).collect();
        assert_eq!(
            nombres,
            [
                "compromiso",
                "de_la_agenda",
                "plazo_dicho",
                "quien",
                "quien_dicho",
                "tema",
                "urgente"
            ]
        );
        assert_eq!(p.fechas.len(), 1);
        // La de quien dice cual es el que habla.
        let quien = serde_json::to_string(&p.peticion.questions["quien"]).unwrap();
        assert!(quien.contains("«Ana», the person speaking"), "{quien}");
    }

    #[test]
    fn sin_fechas_no_se_pregunta_por_plazos_ni_citas() {
        let f = frase("Ana", "hay que mandar la cotización");
        let c = Contexto {
            reunion: "",
            participantes: &[],
            agenda: &[],
            antes: &[],
            frase: &f,
            hoy: hoy(),
        };
        let p = preparar(
            &c,
            &Partes {
                compromiso: true,
                cita: true,
                ..Default::default()
            },
        );
        let nombres: Vec<&str> = p.peticion.questions.keys().map(|s| s.as_str()).collect();
        assert_eq!(nombres, ["compromiso", "urgente"]);
    }

    #[test]
    fn pregunta_con_respuestas_grabadas() {
        let gente = vec!["Ana".to_string(), "Luis".to_string()];
        let tareas = vec![("t1".to_string(), "Pagar al proveedor".to_string())];
        let abiertas = vec![("q1".to_string(), "¿Cuándo llega el material?".to_string())];
        let f = frase(
            "Ana",
            "la cotización del viernes la mando yo mañana a las 10, y el material llega el jueves",
        );
        let c = Contexto {
            reunion: "Coordinación",
            participantes: &gente,
            agenda: &[],
            antes: &[],
            frase: &f,
            hoy: hoy(),
        };
        let partes = Partes {
            compromiso: true,
            decision: true,
            pregunta: true,
            cita: true,
            tareas: &tareas,
            abiertas: &abiertas,
            ..Default::default()
        };
        let p = preparar(&c, &partes);
        let textos: Vec<&str> = p.fechas.iter().map(|e| e.texto.as_str()).collect();
        assert_eq!(textos, ["viernes", "mañana a las 10", "el jueves"]);
        let r = grabadas::respuesta(&[
            grabadas::noul("compromiso", 0.93),
            grabadas::noul("urgente", 0.1),
            grabadas::noul("quien_dicho", 0.9),
            grabadas::choice("quien", "ana", &["luis", "todos"]),
            grabadas::noul("plazo_dicho", 0.88),
            grabadas::choice("plazo", "manana_a_las_10", &["viernes", "el_jueves"]),
            grabadas::noul("decision", 0.2),
            grabadas::noul("pregunta", 0.05),
            grabadas::noul("respondida_q1", 0.91),
            grabadas::noul("cita", 0.15),
            grabadas::choice("cita_cuando", "el_jueves", &["viernes", "manana_a_las_10"]),
            grabadas::noul("sobre_t1", 0.1),
            grabadas::noul("apura_t1", 0.05),
        ]);
        let t = Arc::new(Grabado::nuevo(vec![Grabado::ok(&r)]));
        let cl = Cliente::con_transporte("k", t.clone()).con_cache_propio();
        let (s, fechas) = preguntar(&cl, &c, &partes).unwrap();
        assert_eq!(t.llamadas(), 1, "una sola peticion por frase");
        let co = s.compromiso.unwrap();
        assert_eq!(co.quien.unwrap().0, "Ana");
        let pl = s.plazo.unwrap();
        assert_eq!(fechas[pl.cual].texto, "mañana a las 10");
        assert_eq!(s.respondidas, vec![("q1".to_string(), 0.91)]);
        assert_eq!(s.cita.unwrap().cual, 2);
        assert!(!es_decision(&Senales {
            decision: Some(0.5),
            ..Default::default()
        }));
        // Una respuesta que falta: error de la frase.
        let t = Arc::new(Grabado::nuevo(vec![Grabado::ok(&grabadas::respuesta(&[
            grabadas::noul("compromiso", 0.9),
        ]))]));
        let cl = Cliente::con_transporte("k", t).con_cache_propio();
        assert!(matches!(
            preguntar(&cl, &c, &partes),
            Err(Error::FaltaRespuesta(_))
        ));
    }
}
