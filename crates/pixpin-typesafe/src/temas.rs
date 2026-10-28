//! **Que temas de la agenda ya se trataron**, mientras se graba la reunion
//! (28-oct-2026, `docs/jev.md`). Cada tramo de unos 15 s pasado a texto en
//! vivo va a Jev en **una sola peticion** (comparten el estado: el tramo, el
//! anterior, la agenda y el titulo) con dos Nouls por cada tema pendiente:
//! «¿se trato?» y «¿quedo cerrado?» (las preguntas, en
//! `preguntas::temas`). La decision la toma el codigo, con esta tabla.
//!
//! **Tabla de decision** ([`Seguimiento::decidir`]), por tema y por tramo,
//! con [`SI`] = 0.7 y [`CASI`] = 0.5:
//!
//! | tratado (este tramo) | cerrado (este tramo) | el tramo anterior (el numero justo antes) | se marca |
//! |---|---|---|---|
//! | ≥ SI | — | — | tratado: `ms` de este tramo, `p` = tratado |
//! | ≥ CASI y < SI | ≥ SI | — | tratado (se hablo y se cerro): `ms` de este tramo, `p` = cerrado |
//! | ≥ CASI y < SI | < SI | tratado ≥ CASI | tratado (dos tramos seguidos a medias): `ms` del anterior, `p` = la mayor de las dos |
//! | ≥ CASI y < SI | < SI | otro, o no se miro (saltado) | nada aun: se recuerda para el siguiente |
//! | < CASI | — | — | nada (y se olvida lo recordado) |
//!
//! Un tema «cerrado» que no se trato (tratado < CASI) no se marca: el cierre
//! seria de otra cosa. Jev solo marca, nunca desmarca, y solo los temas que
//! siguen pendientes y que nadie toco a mano (eso lo decide quien llama,
//! `pixpin_proyecto::temas`). Un tramo saltado (el equipo iba cargado)
//! rompe la racha: «dos seguidos» es por numero de tramo.
//!
//! Sin clave o si Jev falla, no se marca nada: el fallo es un error, nunca
//! un «no se trato».

use std::collections::HashMap;

use serde_json::{Map, Value, json};

use crate::preguntas::SI;
use crate::preguntas::temas as q;
pub use crate::preguntas::temas::CASI;
use crate::{Cliente, Error, Peticion};

/// Lo largo del tramo que va (unos 15 s hablados son 40-60 palabras).
const LARGO_TRAMO: usize = 1500;
/// Y del anterior.
const LARGO_ANTES: usize = 700;
/// Y de cada tema de la agenda.
const LARGO_TEMA: usize = 160;

/// Un tema que aun hay que mirar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pendiente {
    pub id: String,
    pub texto: String,
}

/// Un tramo pasado a texto: su numero (seguidos de uno en uno desde el
/// principio de la grabacion; un tramo saltado deja su hueco), el
/// milisegundo de la grabacion en que empieza y lo que se dijo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tramo {
    pub n: u64,
    pub ms: i64,
    pub texto: String,
}

/// Lo que Jev dijo de un tema en un tramo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Senal {
    pub tratado: f64,
    pub cerrado: f64,
}

/// Un tema que ya se trato: su `id`, el milisegundo en que empezo a
/// tratarse y la probabilidad.
#[derive(Debug, Clone, PartialEq)]
pub struct Tratado {
    pub id: String,
    pub ms: i64,
    pub p: f64,
}

/// **El estado de un tramo**: el titulo (si lo hay), la agenda entera, el
/// tramo anterior (si lo hay) y el tramo.
pub fn estado(reunion: &str, agenda: &[String], antes: Option<&str>, tramo: &str) -> Value {
    let mut m = Map::new();
    if !reunion.trim().is_empty() {
        m.insert("reunion".into(), json!(crate::recortar(reunion, 120)));
    }
    let agenda: Vec<Value> = agenda
        .iter()
        .filter(|t| !t.trim().is_empty())
        .map(|t| json!(crate::recortar(t, LARGO_TEMA)))
        .collect();
    if !agenda.is_empty() {
        m.insert("agenda".into(), Value::Array(agenda));
    }
    if let Some(a) = antes.filter(|a| !a.trim().is_empty()) {
        m.insert("antes".into(), json!(crate::recortar(a, LARGO_ANTES)));
    }
    m.insert("tramo".into(), json!(crate::recortar(tramo, LARGO_TRAMO)));
    Value::Object(m)
}

/// **La peticion de un tramo**: el estado y dos preguntas por cada tema
/// pendiente (como mucho [`q::POR_PETICION`] temas).
pub fn peticion(
    reunion: &str,
    agenda: &[String],
    antes: Option<&str>,
    tramo: &str,
    pendientes: &[Pendiente],
) -> Peticion {
    let temas: Vec<(&str, String)> = pendientes
        .iter()
        .take(q::POR_PETICION)
        .map(|p| (p.id.as_str(), crate::recortar(&p.texto, LARGO_TEMA)))
        .collect();
    let pares: Vec<(&str, &str)> = temas.iter().map(|(i, t)| (*i, t.as_str())).collect();
    q::preguntas(&pares).into_iter().fold(
        Peticion::nueva(estado(reunion, agenda, antes, tramo)),
        |p, (n, pr)| p.con(&n, pr),
    )
}

/// Si un tramo merece pregunta: algo dicho (tres palabras o mas; un «eh,
/// vale» suelto no) y algun tema pendiente.
pub fn se_mira(tramo: &str, pendientes: &[Pendiente]) -> bool {
    !pendientes.is_empty() && tramo.split_whitespace().count() >= 3
}

/// **Pregunta por un tramo**: la senal de cada tema que fue en la peticion
/// (los primeros [`q::POR_PETICION`]). Un fallo es un error del tramo
/// entero.
pub fn preguntar(
    cliente: &Cliente,
    reunion: &str,
    agenda: &[String],
    antes: Option<&str>,
    tramo: &str,
    pendientes: &[Pendiente],
) -> Result<(Vec<(String, Senal)>, String), Error> {
    let r = cliente.preguntar(&peticion(reunion, agenda, antes, tramo, pendientes))?;
    let mut v = Vec::new();
    for p in pendientes.iter().take(q::POR_PETICION) {
        v.push((
            p.id.clone(),
            Senal {
                tratado: r.noul(&q::tratado(&p.id))?,
                cerrado: r.noul(&q::cerrado(&p.id))?,
            },
        ));
    }
    Ok((v, r.model))
}

/// **Lo que se recuerda entre tramos** para la fila de «dos seguidos a
/// medias»: por tema, el ultimo tramo en que quedo entre [`CASI`] y [`SI`].
#[derive(Debug, Clone, Default)]
pub struct Seguimiento {
    a_medias: HashMap<String, (u64, i64, f64)>,
}

impl Seguimiento {
    /// **La tabla**: lo que se trato en el tramo `n` (que empieza en `ms`)
    /// segun las senales de Jev. Lo devuelto se marca; lo demas espera.
    pub fn decidir(&mut self, n: u64, ms: i64, senales: &[(String, Senal)]) -> Vec<Tratado> {
        let mut v = Vec::new();
        for (id, s) in senales {
            let antes = self.a_medias.remove(id);
            if s.tratado >= SI {
                v.push(Tratado {
                    id: id.clone(),
                    ms,
                    p: s.tratado,
                });
            } else if s.tratado >= CASI {
                if s.cerrado >= SI {
                    v.push(Tratado {
                        id: id.clone(),
                        ms,
                        p: s.cerrado,
                    });
                } else if let Some((_, ms0, p0)) = antes.filter(|(n0, _, _)| n0 + 1 == n) {
                    v.push(Tratado {
                        id: id.clone(),
                        ms: ms0,
                        p: p0.max(s.tratado),
                    });
                } else {
                    self.a_medias.insert(id.clone(), (n, ms, s.tratado));
                }
            }
        }
        v
    }

    /// Olvida un tema (se marco a mano, se quito).
    pub fn olvidar(&mut self, id: &str) {
        self.a_medias.remove(id);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::grabadas;
    use crate::transporte::Grabado;
    use std::sync::Arc;

    fn s(tratado: f64, cerrado: f64) -> Senal {
        Senal { tratado, cerrado }
    }

    fn una(id: &str, x: Senal) -> Vec<(String, Senal)> {
        vec![(id.to_string(), x)]
    }

    #[test]
    fn la_tabla_de_lo_tratado() {
        let mut g = Seguimiento::default();
        // Claro en un tramo.
        assert_eq!(
            g.decidir(3, 45_000, &una("t1", s(0.86, 0.2))),
            vec![Tratado {
                id: "t1".into(),
                ms: 45_000,
                p: 0.86
            }]
        );
        // El umbral es inclusivo.
        assert_eq!(g.decidir(3, 0, &una("t1", s(0.7, 0.0))).len(), 1);
        // A medias pero cerrado: se trato.
        let v = g.decidir(4, 60_000, &una("t2", s(0.55, 0.9)));
        assert_eq!((v[0].ms, v[0].p), (60_000, 0.9));
        // Cerrado sin tratar: no (el cierre seria de otra cosa).
        assert!(g.decidir(5, 0, &una("t3", s(0.2, 0.95))).is_empty());
        // Solo nombrado o dejado para luego: no.
        assert!(g.decidir(6, 0, &una("t3", s(0.1, 0.05))).is_empty());
    }

    #[test]
    fn dos_tramos_seguidos_a_medias_cuentan() {
        let mut g = Seguimiento::default();
        assert!(g.decidir(10, 150_000, &una("t1", s(0.6, 0.3))).is_empty());
        let v = g.decidir(11, 165_000, &una("t1", s(0.65, 0.4)));
        // El minuto es el del primero: ahi empezo a tratarse.
        assert_eq!(
            v,
            vec![Tratado {
                id: "t1".into(),
                ms: 150_000,
                p: 0.65
            }]
        );
        // No seguidos (el 13 se salto): la racha se rompe.
        assert!(g.decidir(12, 0, &una("t2", s(0.6, 0.0))).is_empty());
        assert!(g.decidir(14, 0, &una("t2", s(0.6, 0.0))).is_empty());
        // Un tramo claro en «no» entre medias tambien.
        assert!(g.decidir(20, 0, &una("t3", s(0.6, 0.0))).is_empty());
        assert!(g.decidir(21, 0, &una("t3", s(0.2, 0.0))).is_empty());
        assert!(g.decidir(22, 0, &una("t3", s(0.6, 0.0))).is_empty());
        // Olvidado (se marco a mano): no cuenta.
        assert!(g.decidir(30, 0, &una("t4", s(0.6, 0.0))).is_empty());
        g.olvidar("t4");
        assert!(g.decidir(31, 0, &una("t4", s(0.6, 0.0))).is_empty());
    }

    fn pendientes() -> Vec<Pendiente> {
        [("t1", "Presupuesto 2027"), ("t3", "Viaje a Lima")]
            .into_iter()
            .map(|(id, t)| Pendiente {
                id: id.into(),
                texto: t.into(),
            })
            .collect()
    }

    #[test]
    fn el_estado_y_la_peticion() {
        let agenda = vec![
            "Presupuesto 2027".to_string(),
            "Nuevo becario".to_string(),
            "Viaje a Lima".to_string(),
        ];
        let e = estado("Comité", &agenda, None, "bueno, el presupuesto…");
        assert_eq!(e["reunion"], "Comité");
        assert_eq!(e["agenda"].as_array().unwrap().len(), 3);
        // Sin tramo anterior el campo no va (vacio se leeria como senal).
        assert!(e.get("antes").is_none());
        let e = estado("", &agenda, Some("antes"), "ahora");
        assert!(e.get("reunion").is_none());
        assert_eq!(e["antes"], "antes");
        let p = peticion("Comité", &agenda, None, "x", &pendientes());
        assert_eq!(p.questions.len(), 4);
        assert!(p.questions.contains_key("tratado_t3"));
        // Como mucho POR_PETICION temas.
        let muchos: Vec<Pendiente> = (0..30)
            .map(|i| Pendiente {
                id: format!("t{i}"),
                texto: format!("tema {i}"),
            })
            .collect();
        assert_eq!(
            peticion("", &[], None, "x", &muchos).questions.len(),
            2 * q::POR_PETICION
        );
        assert!(!se_mira("eh vale", &pendientes()));
        assert!(!se_mira("hablemos del presupuesto ya", &[]));
        assert!(se_mira("hablemos del presupuesto ya", &pendientes()));
    }

    #[test]
    fn pregunta_con_respuestas_grabadas() {
        let t = Arc::new(Grabado::nuevo(vec![Grabado::ok(&grabadas::nouls(&[
            ("tratado_t1", 0.91),
            ("cerrado_t1", 0.8),
            ("tratado_t3", 0.12),
            ("cerrado_t3", 0.05),
        ]))]));
        let c = Cliente::con_transporte("k", t.clone()).con_cache_propio();
        let (v, modelo) = preguntar(
            &c,
            "Comité",
            &[],
            None,
            "El presupuesto queda en cuarenta mil, lo aprobamos. Del viaje hablamos luego.",
            &pendientes(),
        )
        .unwrap();
        assert_eq!(t.llamadas(), 1, "una sola peticion por tramo");
        assert_eq!(modelo, "jev-1.14.0");
        let mut g = Seguimiento::default();
        let hechos = g.decidir(0, 0, &v);
        assert_eq!(hechos.len(), 1);
        assert_eq!(hechos[0].id, "t1");
    }

    #[test]
    fn un_fallo_o_una_respuesta_que_falta_es_un_error() {
        let t = Arc::new(Grabado::nuevo(vec![Err("sin red".into())]));
        let c = Cliente::con_transporte("k", t).con_cache_propio();
        assert!(matches!(
            preguntar(&c, "", &[], None, "a b c", &pendientes()),
            Err(Error::Red(_))
        ));
        let t = Arc::new(Grabado::nuevo(vec![Grabado::ok(&grabadas::nouls(&[(
            "tratado_t1",
            0.9,
        )]))]));
        let c = Cliente::con_transporte("k", t).con_cache_propio();
        assert!(matches!(
            preguntar(&c, "", &[], None, "a b c", &pendientes()),
            Err(Error::FaltaRespuesta(_))
        ));
        // Sin clave no se llama a nadie.
        let c = Cliente::con_transporte("", Arc::new(Grabado::default()));
        assert_eq!(
            preguntar(&c, "", &[], None, "a b c", &pendientes()),
            Err(Error::SinClave)
        );
    }

    /// **La de verdad**, solo con `TYPESAFE_API_KEY`: un tramo que trata un
    /// tema y deja otro para luego. `cargo test -p pixpin-typesafe --
    /// --ignored`.
    #[test]
    #[ignore = "sale a la red: necesita TYPESAFE_API_KEY"]
    fn de_verdad_con_la_api() {
        let Ok(clave) = std::env::var("TYPESAFE_API_KEY") else {
            eprintln!("sin TYPESAFE_API_KEY: no se prueba");
            return;
        };
        if cfg!(not(windows)) {
            eprintln!("la red de TypeSafe solo esta en Windows: no se prueba");
            return;
        }
        let c = Cliente::nuevo(clave).con_tiempo(std::time::Duration::from_secs(10));
        let agenda = vec!["Presupuesto 2027".to_string(), "Viaje a Lima".to_string()];
        let (v, _) = preguntar(
            &c,
            "Comité de dirección",
            &agenda,
            None,
            "Vale, con el presupuesto: hemos repasado las partidas y queda en cuarenta mil euros, lo aprobamos así. Lo del viaje a Lima lo vemos la semana que viene.",
            &pendientes(),
        )
        .expect("la API contesta");
        let mut g = Seguimiento::default();
        let ids: Vec<String> = g.decidir(0, 0, &v).into_iter().map(|t| t.id).collect();
        assert_eq!(ids, vec!["t1".to_string()], "{v:?}");
    }
}
