//! **Fechas dichas al horario** (29-oct-2026, prototipo
//! `fechas_al_horario`): «nos vemos el martes a las 3» → un evento
//! propuesto para el calendario. Jev dice si la frase pone un momento a una
//! reunion o un evento (`cita`) y cual de las expresiones de fecha es
//! (`cita_cuando`); el dia y la hora los resuelve el codigo
//! ([`crate::fechas`]).
//!
//! **Tabla** ([`Horario::proponer`]):
//!
//! | `cita` | expresiones de fecha | se hace |
//! |---|---|---|
//! | ≥ SI | al menos una | evento en la elegida (con hora, o de dia entero) |
//! | ≥ SI | el mismo momento ya propuesto | nada (repetido) |
//! | entre NO y SI | — | nada, se apunta como dudosa |
//! | ≤ NO | — | nada |
//!
//! Sin ninguna expresion de fecha no se pregunta (`crate::vivo`).

use crate::fechas::{Expresion, Momento};
use crate::preguntas::{NO, SI};
use crate::vivo::{Cita, Frase};

/// Un evento propuesto.
#[derive(Debug, Clone, PartialEq)]
pub struct Evento {
    pub ms: i64,
    pub titulo: String,
    pub momento: Momento,
    pub expresion: String,
    pub p: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Resultado {
    Evento(Evento),
    Repetido(Momento),
    Dudosa(f64),
    Nada,
}

/// **Los eventos propuestos en la reunion.**
#[derive(Debug, Clone, Default)]
pub struct Horario {
    pub eventos: Vec<Evento>,
}

impl Horario {
    /// **La tabla.**
    pub fn proponer(&mut self, frase: &Frase, c: Option<&Cita>, fechas: &[Expresion]) -> Resultado {
        let Some(c) = c else {
            return Resultado::Nada;
        };
        if c.p <= NO {
            return Resultado::Nada;
        }
        if c.p < SI {
            return Resultado::Dudosa(c.p);
        }
        let Some(e) = fechas.get(c.cual) else {
            return Resultado::Nada;
        };
        if self.eventos.iter().any(|x| x.momento == e.momento) {
            return Resultado::Repetido(e.momento);
        }
        let ev = Evento {
            ms: frase.ms,
            titulo: crate::recortar(&frase.dice, 80),
            momento: e.momento,
            expresion: e.texto.clone(),
            p: c.p,
        };
        self.eventos.push(ev.clone());
        Resultado::Evento(ev)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::fechas::{self, Fecha};

    #[test]
    fn la_tabla_de_las_citas() {
        let hoy = Fecha::nueva(2026, 10, 26).unwrap();
        let f = Frase {
            ms: 1_000,
            quien: "Ana".into(),
            dice: "nos vemos el martes a las 3".into(),
        };
        let fe = fechas::buscar(&f.dice, hoy);
        let mut h = Horario::default();
        let c = Cita { p: 0.9, cual: 0 };
        let Resultado::Evento(e) = h.proponer(&f, Some(&c), &fe) else {
            panic!()
        };
        assert_eq!(e.momento.to_string(), "mar 2026-10-27 15:00");
        assert!(matches!(
            h.proponer(&f, Some(&c), &fe),
            Resultado::Repetido(_)
        ));
        assert_eq!(
            h.proponer(&f, Some(&Cita { p: 0.5, cual: 0 }), &fe),
            Resultado::Dudosa(0.5)
        );
        assert_eq!(
            h.proponer(&f, Some(&Cita { p: 0.1, cual: 0 }), &fe),
            Resultado::Nada
        );
        assert_eq!(h.proponer(&f, None, &fe), Resultado::Nada);
        assert_eq!(h.eventos.len(), 1);
    }
}
