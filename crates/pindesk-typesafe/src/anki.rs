//! **Que tarjetas de una clase valen la pena** (Anki): menos y mejores. Por
//! cada tarjeta (una captura y lo dicho en su tramo), un Score de lo util que
//! es para estudiar y un Noul de si dice que entra en el examen.
//!
//! **Tabla de decision** ([`veredicto`]); `util` = probabilidad de que el
//! nivel sea 2 o 3 («explica parte del tema» o «una definicion o idea
//! clave»):
//!
//! | examen ≥ 0.7 | util ≥ 0.7 | util ≤ 0.3 | veredicto |
//! |---|---|---|---|
//! | si | * | * | queda |
//! | no | si | — | queda |
//! | no | — | si | sobra |
//! | no | entre 0.3 y 0.7 | | dudosa |
//!
//! Y [`elegir`]: quedan las que «quedan»; las dudosas, de mas a menos util,
//! solo hasta llegar a [`MINIMO`]; si aun asi no quedara ninguna, la mas
//! util (mandar a Anki y que no llegue nada no le sirve a nadie). Una
//! tarjeta sin nada dicho no se pregunta: la imagen es lo que se repasa, y
//! queda.

use serde_json::{Value, json};

use crate::preguntas::anki as q;
use crate::preguntas::{NO, SI};
use crate::{Cliente, Error, Escala, Peticion};

/// Cuantas tarjetas se procura dejar, como poco, si hay dudosas.
pub const MINIMO: usize = 3;

/// Una tarjeta para valorar: su minuto («12:30») y lo dicho en su tramo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tramo {
    pub minuto: String,
    pub dicho: String,
}

/// Lo que Jev dijo de una tarjeta.
#[derive(Debug, Clone, PartialEq)]
pub struct Senales {
    pub valor: Escala,
    pub examen: f64,
}

impl Senales {
    pub fn de(r: &crate::Respuestas) -> Result<Senales, Error> {
        Ok(Senales {
            valor: r.score(q::VALOR, q::NIVELES.len())?,
            examen: r.noul(q::EXAMEN)?,
        })
    }

    /// La probabilidad de que sirva para estudiar (nivel 2 o 3).
    pub fn util(&self) -> f64 {
        self.valor.al_menos(2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Veredicto {
    Queda,
    Dudosa,
    Sobra,
}

/// **La tabla** de una tarjeta.
pub fn veredicto(s: &Senales) -> Veredicto {
    if s.examen >= SI || s.util() >= SI {
        Veredicto::Queda
    } else if s.util() <= NO {
        Veredicto::Sobra
    } else {
        Veredicto::Dudosa
    }
}

/// **Cuales quedan**, en el orden de las tarjetas. `None`: no se pregunto
/// (sin nada dicho) y queda.
pub fn elegir(senales: &[Option<Senales>]) -> Vec<bool> {
    let mut quedan: Vec<bool> = senales
        .iter()
        .map(|s| s.as_ref().is_none_or(|s| veredicto(s) == Veredicto::Queda))
        .collect();
    let mut dudosas: Vec<(usize, f64)> = senales
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let s = s.as_ref()?;
            (veredicto(s) == Veredicto::Dudosa).then(|| (i, s.util()))
        })
        .collect();
    dudosas.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    for (i, _) in dudosas {
        if quedan.iter().filter(|q| **q).count() >= MINIMO {
            break;
        }
        quedan[i] = true;
    }
    if !quedan.iter().any(|q| *q)
        && let Some((i, _)) = senales
            .iter()
            .enumerate()
            .filter_map(|(i, s)| Some((i, s.as_ref()?.util())))
            .max_by(|a, b| a.1.total_cmp(&b.1))
    {
        quedan[i] = true;
    }
    quedan
}

/// El estado de una tarjeta.
pub fn estado(clase: &str, t: &Tramo) -> Value {
    let mut v = json!({
        "minuto": t.minuto,
        "dicho": crate::recortar(&t.dicho, 1500),
    });
    if !clase.trim().is_empty() {
        v["clase"] = json!(crate::recortar(clase, 120));
    }
    v
}

pub fn peticion(clase: &str, t: &Tramo) -> Peticion {
    q::preguntas()
        .into_iter()
        .fold(Peticion::nueva(estado(clase, t)), |p, (n, pr)| p.con(n, pr))
}

/// **Valora cada tarjeta**, `hilos` a la vez. Las que no tienen nada dicho
/// no se preguntan (`None`). Un fallo es un error del conjunto: quien llama
/// manda entonces todas, como siempre.
pub fn valorar(
    cliente: &Cliente,
    clase: &str,
    tramos: &[Tramo],
    hilos: usize,
) -> Result<Vec<Option<Senales>>, Error> {
    crate::en_paralelo(tramos, hilos, &|t| {
        if t.dicho.split_whitespace().count() < 3 {
            return Ok(None);
        }
        let r = cliente.preguntar(&peticion(clase, t))?;
        Senales::de(&r).map(Some)
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::grabadas;
    use crate::transporte::Grabado;
    use std::sync::Arc;

    fn s(niveles: [f64; 4], examen: f64) -> Option<Senales> {
        Some(Senales {
            valor: Escala {
                valor: niveles.iter().enumerate().map(|(i, p)| i as f64 * p).sum(),
                confianza: 0.6,
                niveles: niveles.to_vec(),
            },
            examen,
        })
    }

    #[test]
    fn la_tabla_de_cada_tarjeta() {
        let clave = s([0.0, 0.05, 0.15, 0.8], 0.1).unwrap();
        assert_eq!(veredicto(&clave), Veredicto::Queda);
        let charla = s([0.85, 0.1, 0.05, 0.0], 0.02).unwrap();
        assert_eq!(veredicto(&charla), Veredicto::Sobra);
        // Aunque parezca poco, si dice que entra en el examen, queda.
        let examen = s([0.6, 0.3, 0.1, 0.0], 0.9).unwrap();
        assert_eq!(veredicto(&examen), Veredicto::Queda);
        let media = s([0.1, 0.4, 0.4, 0.1], 0.1).unwrap();
        assert_eq!(veredicto(&media), Veredicto::Dudosa);
    }

    #[test]
    fn elegir_deja_menos_y_mejores() {
        let v = vec![
            s([0.0, 0.05, 0.15, 0.8], 0.1), // queda
            s([0.85, 0.1, 0.05, 0.0], 0.0), // sobra
            s([0.1, 0.4, 0.4, 0.1], 0.1),   // dudosa (0.5)
            None,                           // sin nada dicho: queda
            s([0.1, 0.3, 0.4, 0.2], 0.1),   // dudosa (0.6)
            s([0.1, 0.5, 0.3, 0.1], 0.1),   // dudosa (0.4)
        ];
        // Dos que quedan; las dudosas rellenan hasta 3, la mas util primero.
        assert_eq!(elegir(&v), vec![true, false, false, true, true, false]);
        // Con muchas buenas, las dudosas no entran.
        let buenas = vec![
            s([0.0, 0.0, 0.2, 0.8], 0.0),
            s([0.0, 0.0, 0.2, 0.8], 0.0),
            s([0.0, 0.0, 0.2, 0.8], 0.0),
            s([0.1, 0.4, 0.4, 0.1], 0.1),
        ];
        assert_eq!(elegir(&buenas), vec![true, true, true, false]);
        // Si todas sobran, la mas util (no se manda nada vacio).
        let malas = vec![s([0.9, 0.1, 0.0, 0.0], 0.0), s([0.7, 0.1, 0.2, 0.0], 0.0)];
        assert_eq!(elegir(&malas), vec![false, true]);
        assert!(elegir(&[]).is_empty());
    }

    #[test]
    fn valora_con_respuestas_grabadas() {
        let r = grabadas::respuesta(&[
            grabadas::score(q::VALOR, &[0.0, 0.1, 0.2, 0.7]),
            grabadas::noul(q::EXAMEN, 0.2),
        ]);
        let t = Arc::new(Grabado::nuevo(vec![Grabado::ok(&r)]));
        let c = Cliente::con_transporte("k", t.clone()).con_cache_propio();
        let tramos = vec![
            Tramo {
                minuto: "0:30".into(),
                dicho: "La derivada es el límite del cociente incremental.".into(),
            },
            Tramo {
                minuto: "1:10".into(),
                dicho: "".into(),
            },
        ];
        let v = valorar(&c, "Cálculo", &tramos, 1).unwrap();
        assert_eq!(t.llamadas(), 1);
        assert!((v[0].as_ref().unwrap().util() - 0.9).abs() < 1e-9);
        assert!(v[1].is_none());
        let p = t.pedidos.lock().unwrap()[0].cuerpo.clone();
        let e: Value = serde_json::from_slice(&p).unwrap();
        assert_eq!(e["state"]["minuto"], "0:30");
        assert_eq!(e["state"]["clase"], "Cálculo");
    }
}
