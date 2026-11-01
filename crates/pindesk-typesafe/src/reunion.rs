//! **Las partes importantes de una clase o reunion** grabada: por cada
//! renglon de la transcripcion (`[m:ss] texto`), si encarga una tarea, si
//! anuncia una decision, si alguien pregunta algo o si dice algo de examen o
//! un dato clave. Una peticion por renglon con todas sus preguntas juntas
//! (comparten el estado: el renglon, sus vecinos y el titulo), varias a la
//! vez en segundo plano.
//!
//! **Tabla de decision** ([`decidir`]), con [`SI`] = 0.7:
//!
//! | charla ≥ SI | tarea ≥ SI | decision ≥ SI | pregunta ≥ SI | examen o dato ≥ SI | se marca |
//! |---|---|---|---|---|---|
//! | no | si | — | — | — | tarea |
//! | no | — | si | — | — | decision |
//! | no | — | — | si | — | pregunta |
//! | no | — | — | — | si | examen |
//! | si | si | — | — | — | solo tarea («traigan la calculadora» es logistica y es tarea) |
//! | si | no | * | * | * | nada |
//!
//! Un renglon puede llevar varias marcas (son Nouls separados). Lo que queda
//! entre 0.3 y 0.7 no se marca: no esta claro, y una marca de mas en una
//! clase de una hora es ruido. `tarea_quien` y `tarea_cuando` solo se leen
//! si hay tarea (preguntas especulativas: van en la misma peticion).

use serde_json::{Map, Value, json};

use crate::preguntas::reunion as q;
use crate::preguntas::{NO, SI};
use crate::{Cliente, Error, Peticion};

/// Un renglon de la transcripcion: su minuto (si lo lleva) y lo que dice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renglon {
    pub ms: Option<i64>,
    pub texto: String,
}

/// Que clase de parte importante es.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tipo {
    Tarea,
    Decision,
    Pregunta,
    Examen,
}

impl Tipo {
    pub const TODOS: [Tipo; 4] = [Tipo::Tarea, Tipo::Decision, Tipo::Pregunta, Tipo::Examen];

    /// Como se guarda (`importantes[].tipo`).
    pub fn id(self) -> &'static str {
        match self {
            Tipo::Tarea => "tarea",
            Tipo::Decision => "decision",
            Tipo::Pregunta => "pregunta",
            Tipo::Examen => "examen",
        }
    }
}

/// Lo que Jev dijo de un renglon.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Senales {
    pub tarea: f64,
    pub quien: f64,
    pub cuando: f64,
    pub decision: f64,
    pub pregunta: f64,
    pub examen: f64,
    pub dato: f64,
    pub charla: f64,
}

impl Senales {
    pub fn de(r: &crate::Respuestas) -> Result<Senales, Error> {
        Ok(Senales {
            tarea: r.noul(q::TAREA)?,
            quien: r.noul(q::TAREA_QUIEN)?,
            cuando: r.noul(q::TAREA_CUANDO)?,
            decision: r.noul(q::DECISION)?,
            pregunta: r.noul(q::PREGUNTA)?,
            examen: r.noul(q::EXAMEN)?,
            dato: r.noul(q::DATO)?,
            charla: r.noul(q::CHARLA)?,
        })
    }
}

/// **La tabla**: que marcas lleva un renglon y con que probabilidad.
pub fn decidir(s: &Senales) -> Vec<(Tipo, f64)> {
    let mut v = Vec::new();
    if s.tarea >= SI {
        v.push((Tipo::Tarea, s.tarea));
    }
    if s.charla >= SI {
        return v;
    }
    if s.decision >= SI {
        v.push((Tipo::Decision, s.decision));
    }
    if s.pregunta >= SI {
        v.push((Tipo::Pregunta, s.pregunta));
    }
    let examen = s.examen.max(s.dato);
    if examen >= SI {
        v.push((Tipo::Examen, examen));
    }
    v
}

/// Una parte importante hallada: el minuto de su renglon, que es y con que
/// probabilidad. Las tareas dicen ademas si se sabe quien y cuando (`None`
/// si no se sabe: entre 0.3 y 0.7).
#[derive(Debug, Clone, PartialEq)]
pub struct Hallazgo {
    pub ms: i64,
    pub tipo: Tipo,
    pub p: f64,
    pub quien: Option<bool>,
    pub cuando: Option<bool>,
}

fn si_o_no(p: f64) -> Option<bool> {
    if p >= SI {
        Some(true)
    } else if p <= NO {
        Some(false)
    } else {
        None
    }
}

/// Lo largo que va un renglon (los de una clase son parrafos de 20-30 s).
const LARGO: usize = 700;
/// Y cada vecino.
const LARGO_VECINO: usize = 300;

/// **El estado de un renglon**: el titulo, el renglon y sus vecinos (los que
/// haya; sin vecinos el campo no va).
pub fn estado(clase: &str, renglones: &[Renglon], i: usize) -> Value {
    let mut m = Map::new();
    if !clase.trim().is_empty() {
        m.insert("clase".into(), json!(crate::recortar(clase, 120)));
    }
    let desde = i.saturating_sub(q::VECINOS);
    let antes: Vec<Value> = renglones[desde..i]
        .iter()
        .map(|r| json!(crate::recortar(&r.texto, LARGO_VECINO)))
        .collect();
    if !antes.is_empty() {
        m.insert("antes".into(), Value::Array(antes));
    }
    m.insert(
        "linea".into(),
        json!(crate::recortar(&renglones[i].texto, LARGO)),
    );
    let despues: Vec<Value> = renglones
        .iter()
        .skip(i + 1)
        .take(q::VECINOS)
        .map(|r| json!(crate::recortar(&r.texto, LARGO_VECINO)))
        .collect();
    if !despues.is_empty() {
        m.insert("despues".into(), Value::Array(despues));
    }
    Value::Object(m)
}

/// La peticion de un renglon.
pub fn peticion(clase: &str, renglones: &[Renglon], i: usize) -> Peticion {
    q::preguntas().into_iter().fold(
        Peticion::nueva(estado(clase, renglones, i)),
        |p, (n, pr)| p.con(n, pr),
    )
}

/// Si un renglon merece pregunta: con minuto y con algo dicho (dos palabras
/// o mas; un «eh» suelto no).
fn se_mira(r: &Renglon) -> bool {
    r.ms.is_some() && r.texto.split_whitespace().count() >= 2
}

/// **Busca las partes importantes** de toda la transcripcion, `hilos`
/// peticiones a la vez. Devuelve lo hallado en orden y el modelo que
/// contesto. Un fallo en cualquier renglon es un error del conjunto: nada a
/// medias se guarda como si estuviera entero.
pub fn buscar(
    cliente: &Cliente,
    clase: &str,
    renglones: &[Renglon],
    hilos: usize,
) -> Result<(Vec<Hallazgo>, String), Error> {
    let indices: Vec<usize> = (0..renglones.len())
        .filter(|i| se_mira(&renglones[*i]))
        .collect();
    let respuestas = crate::en_paralelo(&indices, hilos, &|i| {
        let r = cliente.preguntar(&peticion(clase, renglones, *i))?;
        Ok((*i, Senales::de(&r)?, r.model))
    })?;
    let mut modelo = String::new();
    let mut v = Vec::new();
    for (i, s, m) in respuestas {
        modelo = m;
        let Some(ms) = renglones[i].ms else { continue };
        for (tipo, p) in decidir(&s) {
            let tarea = tipo == Tipo::Tarea;
            v.push(Hallazgo {
                ms,
                tipo,
                p,
                quien: tarea.then(|| si_o_no(s.quien)).flatten(),
                cuando: tarea.then(|| si_o_no(s.cuando)).flatten(),
            });
        }
    }
    Ok((v, modelo))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::grabadas;
    use crate::transporte::Grabado;
    use std::sync::Arc;

    fn s(tarea: f64, decision: f64, pregunta: f64, examen: f64, dato: f64, charla: f64) -> Senales {
        Senales {
            tarea,
            quien: 0.5,
            cuando: 0.5,
            decision,
            pregunta,
            examen,
            dato,
            charla,
        }
    }

    #[test]
    fn la_tabla_de_las_marcas() {
        // Una tarea clara.
        assert_eq!(
            decidir(&s(0.92, 0.1, 0.1, 0.1, 0.1, 0.05)),
            vec![(Tipo::Tarea, 0.92)]
        );
        // Varias a la vez: son Nouls separados.
        let v = decidir(&s(0.8, 0.75, 0.1, 0.9, 0.2, 0.1));
        assert_eq!(
            v.iter().map(|(t, _)| *t).collect::<Vec<_>>(),
            vec![Tipo::Tarea, Tipo::Decision, Tipo::Examen]
        );
        // Un dato clave cuenta como «examen» (lo que hay que estudiar).
        assert_eq!(
            decidir(&s(0.1, 0.1, 0.1, 0.2, 0.85, 0.1)),
            vec![(Tipo::Examen, 0.85)]
        );
        // La charla apaga todo menos la tarea.
        assert_eq!(
            decidir(&s(0.8, 0.9, 0.9, 0.9, 0.9, 0.95)),
            vec![(Tipo::Tarea, 0.8)]
        );
        assert!(decidir(&s(0.1, 0.9, 0.9, 0.9, 0.9, 0.95)).is_empty());
        // Lo que no esta claro no se marca.
        assert!(decidir(&s(0.6, 0.5, 0.65, 0.4, 0.5, 0.2)).is_empty());
        // El umbral es inclusivo.
        assert_eq!(decidir(&s(0.7, 0.0, 0.0, 0.0, 0.0, 0.0)).len(), 1);
    }

    fn renglones() -> Vec<Renglon> {
        [
            (Some(0), "Buenos días, ¿me escuchan bien?"),
            (Some(20_000), "Hoy vemos la derivada como razón de cambio."),
            (
                Some(45_000),
                "Para el lunes resuelvan los ejercicios del 1 al 5.",
            ),
            (None, "texto pegado sin minuto"),
            (Some(70_000), "eh"),
        ]
        .into_iter()
        .map(|(ms, t)| Renglon {
            ms,
            texto: t.to_string(),
        })
        .collect()
    }

    #[test]
    fn el_estado_lleva_el_renglon_sus_vecinos_y_el_titulo() {
        let r = renglones();
        let e = estado("Cálculo I", &r, 2);
        assert_eq!(e["clase"], "Cálculo I");
        assert_eq!(e["linea"], r[2].texto);
        assert_eq!(e["antes"].as_array().unwrap().len(), 2);
        assert_eq!(e["despues"].as_array().unwrap().len(), 2);
        // El primero no tiene «antes»: el campo no va (una lista vacia se
        // leeria como senal).
        let e = estado("", &r, 0);
        assert!(e.get("antes").is_none());
        assert!(e.get("clase").is_none());
        let p = peticion("Cálculo I", &r, 2);
        assert_eq!(p.questions.len(), 8);
    }

    fn respuesta(tarea: f64, quien: f64, cuando: f64, charla: f64) -> String {
        grabadas::nouls(&[
            (q::TAREA, tarea),
            (q::TAREA_QUIEN, quien),
            (q::TAREA_CUANDO, cuando),
            (q::DECISION, 0.05),
            (q::PREGUNTA, 0.1),
            (q::EXAMEN, 0.1),
            (q::DATO, 0.2),
            (q::CHARLA, charla),
        ])
    }

    #[test]
    fn busca_con_respuestas_grabadas() {
        // Tres renglones se miran (el sin minuto y el «eh», no). Con un solo
        // hilo, las respuestas llegan en orden.
        let t = Arc::new(Grabado::nuevo(vec![
            Grabado::ok(&respuesta(0.05, 0.1, 0.1, 0.97)),
            Grabado::ok(&respuesta(0.1, 0.1, 0.1, 0.05)),
            Grabado::ok(&respuesta(0.94, 0.4, 0.9, 0.02)),
        ]));
        let c = Cliente::con_transporte("k", t.clone()).con_cache_propio();
        let (v, modelo) = buscar(&c, "Cálculo I", &renglones(), 1).unwrap();
        assert_eq!(t.llamadas(), 3);
        assert_eq!(modelo, "jev-1.14.0");
        assert_eq!(
            v,
            vec![Hallazgo {
                ms: 45_000,
                tipo: Tipo::Tarea,
                p: 0.94,
                quien: None,
                cuando: Some(true),
            }]
        );
    }

    #[test]
    fn un_fallo_no_deja_un_resultado_a_medias() {
        let t = Arc::new(Grabado::nuevo(vec![
            Grabado::ok(&respuesta(0.05, 0.1, 0.1, 0.97)),
            Err("se corto".into()),
        ]));
        let c = Cliente::con_transporte("k", t).con_cache_propio();
        assert!(matches!(
            buscar(&c, "x", &renglones(), 1),
            Err(Error::Red(_))
        ));
    }
}
