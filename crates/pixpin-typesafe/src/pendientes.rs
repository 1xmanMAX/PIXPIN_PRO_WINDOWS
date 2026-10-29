//! **Preguntas que nadie contesto** (29-oct-2026, prototipo
//! `preguntas_pendientes`): cada pregunta de verdad que se hace en la
//! reunion queda abierta; en cada frase siguiente va un Noul por pregunta
//! abierta («¿esta frase contesta «…»?», con la pregunta dentro) en la
//! misma peticion de la frase (`crate::vivo`). Al final, las que siguen
//! abiertas son las pendientes.
//!
//! **Tabla** ([`Pendientes::tras_frase`]), en este orden:
//!
//! | señal | se hace |
//! |---|---|
//! | `respondida_<id>` ≥ SI | se cierra (contestada por quien habla) |
//! | `respondida_<id>` entre NO y SI | sigue abierta, marcada «quiza» |
//! | `pregunta` ≥ SI | se abre una nueva con la frase |
//!
//! Primero se cierran y luego se abre: una pregunta no se contesta a si
//! misma. Van como mucho `preguntas::vivo::POR_PETICION` abiertas por
//! peticion, las mas recientes.

use crate::preguntas::{NO, SI};
use crate::vivo::Frase;

/// Una pregunta abierta.
#[derive(Debug, Clone, PartialEq)]
pub struct Abierta {
    pub id: String,
    pub ms: i64,
    pub quien: String,
    pub texto: String,
    /// Alguna frase la contesto a medias (entre NO y SI).
    pub quiza: bool,
}

/// Una contestada: la pregunta, cuando y quien.
#[derive(Debug, Clone, PartialEq)]
pub struct Contestada {
    pub pregunta: Abierta,
    pub ms: i64,
    pub quien: String,
    pub p: f64,
}

/// Lo que paso con una frase.
#[derive(Debug, Clone, PartialEq)]
pub enum Evento {
    Abierta(Abierta),
    Contestada(Contestada),
    Quiza { id: String, p: f64 },
}

/// **Las preguntas de la reunion.**
#[derive(Debug, Clone, Default)]
pub struct Pendientes {
    abiertas: Vec<Abierta>,
    contestadas: Vec<Contestada>,
    n: usize,
}

impl Pendientes {
    /// Las que van en la siguiente peticion: (id, texto).
    pub fn para_preguntar(&self) -> Vec<(String, String)> {
        self.abiertas
            .iter()
            .map(|a| (a.id.clone(), a.texto.clone()))
            .collect()
    }

    /// **La tabla** para una frase: `pregunta` (si se pregunto) y las
    /// `respondidas` de las abiertas que fueron en su peticion.
    pub fn tras_frase(
        &mut self,
        frase: &Frase,
        pregunta: Option<f64>,
        respondidas: &[(String, f64)],
    ) -> Vec<Evento> {
        let mut ev = Vec::new();
        for (id, p) in respondidas {
            let Some(i) = self.abiertas.iter().position(|a| a.id == *id) else {
                continue;
            };
            if *p >= SI {
                let a = self.abiertas.remove(i);
                let c = Contestada {
                    pregunta: a,
                    ms: frase.ms,
                    quien: frase.quien.clone(),
                    p: *p,
                };
                self.contestadas.push(c.clone());
                ev.push(Evento::Contestada(c));
            } else if *p > NO {
                self.abiertas[i].quiza = true;
                ev.push(Evento::Quiza {
                    id: id.clone(),
                    p: *p,
                });
            }
        }
        if pregunta.is_some_and(|p| p >= SI) {
            self.n += 1;
            let a = Abierta {
                id: format!("q{}", self.n),
                ms: frase.ms,
                quien: frase.quien.clone(),
                texto: crate::recortar(&frase.dice, 200),
                quiza: false,
            };
            self.abiertas.push(a.clone());
            ev.push(Evento::Abierta(a));
        }
        ev
    }

    /// Las que siguen sin contestar.
    pub fn abiertas(&self) -> &[Abierta] {
        &self.abiertas
    }

    pub fn contestadas(&self) -> &[Contestada] {
        &self.contestadas
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn f(ms: i64, quien: &str, dice: &str) -> Frase {
        Frase {
            ms,
            quien: quien.into(),
            dice: dice.into(),
        }
    }

    #[test]
    fn se_abren_se_cierran_y_quedan_las_pendientes() {
        let mut p = Pendientes::default();
        let e = p.tras_frase(&f(0, "Ana", "¿cuándo llega el material?"), Some(0.9), &[]);
        assert!(matches!(&e[0], Evento::Abierta(a) if a.id == "q1"));
        // Una pregunta no se contesta a si misma: la nueva no va en la suya.
        let e = p.tras_frase(
            &f(5_000, "Luis", "¿y quién paga los proyectores?"),
            Some(0.85),
            &[("q1".into(), 0.1)],
        );
        assert_eq!(e.len(), 1);
        assert_eq!(p.para_preguntar().len(), 2);
        // A medias: sigue abierta, marcada.
        let e = p.tras_frase(
            &f(9_000, "Marta", "creo que pronto"),
            Some(0.1),
            &[("q1".into(), 0.5), ("q2".into(), 0.05)],
        );
        assert!(matches!(e[0], Evento::Quiza { .. }));
        assert!(p.abiertas()[0].quiza);
        // Contestada.
        let e = p.tras_frase(
            &f(12_000, "Marta", "llega el jueves"),
            Some(0.05),
            &[("q1".into(), 0.92), ("q2".into(), 0.1)],
        );
        assert!(matches!(&e[0], Evento::Contestada(c) if c.quien == "Marta"));
        assert_eq!(p.contestadas().len(), 1);
        let quedan: Vec<&str> = p.abiertas().iter().map(|a| a.id.as_str()).collect();
        assert_eq!(quedan, ["q2"]);
        // Una pregunta dudosa no se abre.
        assert!(
            p.tras_frase(&f(15_000, "Ana", "¿no?"), Some(0.5), &[])
                .is_empty()
        );
    }
}
