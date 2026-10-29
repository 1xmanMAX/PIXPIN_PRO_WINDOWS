//! **Tareas dichas en voz alta** (29-oct-2026, prototipo `tareas_dichas`):
//! «yo me encargo de mandar la cotización el viernes» → una tarea
//! propuesta con su texto, responsable, fecha y prioridad. Jev juzga (¿es
//! un compromiso?, ¿quien?, ¿cual de las expresiones de fecha es el
//! plazo?, ¿lo dice con prisa?); el dia lo resuelve el codigo
//! ([`crate::fechas`]) contra la fecha de la reunion.
//!
//! **Tabla** ([`decidir`]), con SI = 0.7 y NO = 0.3:
//!
//! | compromiso | se hace |
//! |---|---|
//! | ≥ SI | tarea propuesta |
//! | entre NO y SI | nada, se apunta como dudosa (el usuario no ve nada) |
//! | ≤ NO | nada |
//!
//! | quien_dicho | `quien` | responsable |
//! |---|---|---|
//! | ≥ SI | el elegido con ≥ [`QUIEN_MINIMO`] | ese nombre (o «todos») |
//! | ≥ SI | el elegido con menos | sin decidir («¿quien?») |
//! | ≤ NO | — | nadie |
//! | entre los dos, o sin participantes | — | sin decidir |
//!
//! La fila del «con menos» (29-oct-2026, de punta a punta con audio): sin
//! saber quien habla (Whisper da el texto, no la voz), «mañana la llamo»
//! dice que alguien se lo queda (`quien_dicho` 0.93) pero no quien, y
//! `quien` reparte entre los participantes (0.37 el primero): es una
//! adivinanza, no un nombre. Cuando se nombra («Luis, tú hablas con el
//! tutor») sale 0.97.
//!
//! | plazo_dicho | fecha |
//! |---|---|
//! | ≥ SI | la expresion elegida (o la unica), resuelta en codigo |
//! | < SI | sin fecha |
//!
//! Prioridad: los puntos de `urgencia::puntos_del_plazo` por los dias que
//! faltan, +3 si `urgente` ≥ SI; ≥ 3 urgente, ≥ 1.5 alta (hoy o mañana),
//! si no normal. El texto es la frase tal cual (se elige, no se genera):
//! quien la acepta la puede corregir.

use crate::fechas::{Expresion, Fecha, Momento};
use crate::preguntas::{NO, SI};
use crate::urgencia::{Prioridad, puntos_del_plazo};
use crate::vivo::{Compromiso, Frase, Plazo};

/// Lo menos que tiene que tener el elegido de `quien` para ponerlo de
/// responsable (si no, «¿quien?»).
pub const QUIEN_MINIMO: f64 = 0.5;

/// Quien hace la tarea.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Responsable {
    Nombre(String),
    Nadie,
    SinDecidir,
}

impl std::fmt::Display for Responsable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Responsable::Nombre(n) => write!(f, "{n}"),
            Responsable::Nadie => write!(f, "nadie"),
            Responsable::SinDecidir => write!(f, "¿quién?"),
        }
    }
}

/// **Una tarea propuesta.**
#[derive(Debug, Clone, PartialEq)]
pub struct Propuesta {
    pub ms: i64,
    pub texto: String,
    pub responsable: Responsable,
    pub fecha: Option<Momento>,
    /// Las palabras de la fecha («el viernes»).
    pub expresion: Option<String>,
    pub prioridad: Prioridad,
    pub p: f64,
}

/// Lo que sale de una frase.
#[derive(Debug, Clone, PartialEq)]
pub enum Resultado {
    Propuesta(Propuesta),
    /// Entre NO y SI: no se propone.
    Dudosa(f64),
    Nada,
}

/// La prioridad de una tarea nueva por lo que falta y la prisa.
pub fn prioridad(dias: Option<i64>, urgente: f64) -> Prioridad {
    let n = puntos_del_plazo(dias) + if urgente >= SI { 3.0 } else { 0.0 };
    if n >= 3.0 {
        Prioridad::Urgente
    } else if n >= 1.5 {
        Prioridad::Alta
    } else {
        Prioridad::Normal
    }
}

/// El plazo que vale de una frase: el elegido, si `plazo_dicho` ≥ SI.
pub fn plazo_elegido<'a>(plazo: Option<&Plazo>, fechas: &'a [Expresion]) -> Option<&'a Expresion> {
    plazo.filter(|p| p.p >= SI).and_then(|p| fechas.get(p.cual))
}

/// **La tabla.**
pub fn decidir(
    frase: &Frase,
    c: &Compromiso,
    plazo: Option<&Plazo>,
    fechas: &[Expresion],
    hoy: Fecha,
) -> Resultado {
    if c.p <= NO {
        return Resultado::Nada;
    }
    if c.p < SI {
        return Resultado::Dudosa(c.p);
    }
    let responsable = match (c.quien_dicho, &c.quien) {
        (Some(p), Some((n, pn))) if p >= SI && *pn >= QUIEN_MINIMO => {
            Responsable::Nombre(n.clone())
        }
        (Some(p), _) if p <= NO => Responsable::Nadie,
        _ => Responsable::SinDecidir,
    };
    let e = plazo_elegido(plazo, fechas);
    let fecha = e.map(|e| e.momento);
    let dias = fecha.map(|m| hoy.hasta(m.fecha));
    Resultado::Propuesta(Propuesta {
        ms: frase.ms,
        texto: crate::recortar(&frase.dice, 300),
        responsable,
        fecha,
        expresion: e.map(|e| e.texto.clone()),
        prioridad: prioridad(dias, c.urgente),
        p: c.p,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::fechas;

    fn hoy() -> Fecha {
        Fecha::nueva(2026, 10, 26).unwrap()
    }

    fn frase(dice: &str) -> Frase {
        Frase {
            ms: 90_000,
            quien: "Ana".into(),
            dice: dice.into(),
        }
    }

    fn c(p: f64, quien_dicho: f64, urgente: f64) -> Compromiso {
        Compromiso {
            p,
            urgente,
            quien_dicho: Some(quien_dicho),
            quien: Some(("Ana".into(), 0.9)),
        }
    }

    #[test]
    fn la_tabla_de_las_tareas_dichas() {
        let f = frase("yo me encargo de mandar la cotización el viernes");
        let fe = fechas::buscar(&f.dice, hoy());
        let pl = Plazo { p: 0.9, cual: 0 };
        let Resultado::Propuesta(t) = decidir(&f, &c(0.92, 0.9, 0.1), Some(&pl), &fe, hoy()) else {
            panic!()
        };
        assert_eq!(t.responsable, Responsable::Nombre("Ana".into()));
        assert_eq!(t.fecha.unwrap().fecha.to_string(), "2026-10-30");
        assert_eq!(t.expresion.as_deref(), Some("el viernes"));
        assert_eq!(t.prioridad, Prioridad::Normal);
        // Nadie dicho; plazo dudoso: sin fecha.
        let pl = Plazo { p: 0.5, cual: 0 };
        let Resultado::Propuesta(t) = decidir(&f, &c(0.8, 0.1, 0.1), Some(&pl), &fe, hoy()) else {
            panic!()
        };
        assert_eq!(t.responsable, Responsable::Nadie);
        assert_eq!(t.fecha, None);
        // Quien a medias: sin decidir.
        let Resultado::Propuesta(t) = decidir(&f, &c(0.8, 0.5, 0.1), None, &fe, hoy()) else {
            panic!()
        };
        assert_eq!(t.responsable, Responsable::SinDecidir);
        // Alguien se lo queda, pero `quien` adivina (sin saber quien habla):
        // sin decidir.
        let mut adivina = c(0.95, 0.93, 0.1);
        adivina.quien = Some(("Luis".into(), 0.37));
        let Resultado::Propuesta(t) = decidir(&f, &adivina, None, &fe, hoy()) else {
            panic!()
        };
        assert_eq!(t.responsable, Responsable::SinDecidir);
        // Dudosa y no.
        assert_eq!(
            decidir(&f, &c(0.5, 0.9, 0.0), None, &fe, hoy()),
            Resultado::Dudosa(0.5)
        );
        assert_eq!(
            decidir(&f, &c(0.1, 0.9, 0.0), None, &fe, hoy()),
            Resultado::Nada
        );
    }

    #[test]
    fn la_prioridad_por_lo_que_falta() {
        assert_eq!(prioridad(Some(0), 0.0), Prioridad::Alta);
        assert_eq!(prioridad(Some(1), 0.0), Prioridad::Alta);
        assert_eq!(prioridad(Some(4), 0.0), Prioridad::Normal);
        assert_eq!(prioridad(None, 0.9), Prioridad::Urgente);
        assert_eq!(prioridad(None, 0.5), Prioridad::Normal);
    }
}
