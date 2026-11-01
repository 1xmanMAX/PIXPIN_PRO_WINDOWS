//! **Repaso oral** (29-oct-2026, prototipo `repaso_oral`): un alumno
//! explica en voz alta lo de la clase y se ve que puntos clave cubre, cuales
//! dice mal y cuales faltan. Una peticion por frase con lo explicado hasta
//! ahora (`explicacion`) y dos Nouls por punto con el punto dentro
//! (`preguntas::repaso`): «¿lo explica?» y «¿dice algo que lo contradice?»
//! (dos juicios, dos preguntas).
//!
//! **Tabla de una peticion** ([`leer_punto`]):
//!
//! | `contradice` | `cubre` | estado |
//! |---|---|---|
//! | ≥ SI | — | mal |
//! | < SI | ≥ SI | cubierto |
//! | < SI | ≤ NO | falta |
//! | < SI | entre NO y SI | dudoso |
//!
//! **Entre peticiones** ([`Repaso::decidir`]): «mal» se queda (lo dicho mal
//! no se borra al seguir hablando); «cubierto» solo pasa a «mal»; «dudoso»
//! sube a «cubierto» o «mal»; «falta» puede pasar a cualquiera.

use serde_json::json;

use crate::preguntas::repaso as q;
use crate::preguntas::{NO, SI};
use crate::{Cliente, Error, Peticion};

/// Lo largo de la explicacion que se manda (lo ultimo).
const LARGO: usize = 4000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    Falta,
    Dudoso,
    Cubierto,
    Mal,
}

impl Estado {
    pub fn id(self) -> &'static str {
        match self {
            Estado::Falta => "falta",
            Estado::Dudoso => "dudoso",
            Estado::Cubierto => "cubierto",
            Estado::Mal => "mal",
        }
    }
}

/// Lo que dijo Jev de un punto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SenalPunto {
    pub cubre: f64,
    pub contradice: f64,
}

/// La tabla de una peticion.
pub fn leer_punto(s: &SenalPunto) -> Estado {
    if s.contradice >= SI {
        Estado::Mal
    } else if s.cubre >= SI {
        Estado::Cubierto
    } else if s.cubre <= NO {
        Estado::Falta
    } else {
        Estado::Dudoso
    }
}

/// La peticion: la clase, lo explicado y los puntos (id, texto).
pub fn peticion(clase: &str, explicacion: &str, puntos: &[(String, String)]) -> Peticion {
    let mut largo: Vec<char> = explicacion.trim().chars().collect();
    if largo.len() > LARGO {
        largo = largo.split_off(largo.len() - LARGO);
        largo.insert(0, '…');
    }
    let mut p = Peticion::nueva(json!({
        "clase": crate::recortar(clase, 120),
        "explicacion": largo.into_iter().collect::<String>(),
    }));
    for (id, t) in puntos.iter().take(q::POR_PETICION) {
        let t = crate::recortar(t, 200);
        p = p
            .con(&q::cubre(id), q::explica(&t))
            .con(&q::contradice(id), q::lo_contradice(&t));
    }
    p
}

/// **Pregunta**: la senal de cada punto que fue.
pub fn preguntar(
    cliente: &Cliente,
    clase: &str,
    explicacion: &str,
    puntos: &[(String, String)],
) -> Result<Vec<(String, SenalPunto)>, Error> {
    let r = cliente.preguntar(&peticion(clase, explicacion, puntos))?;
    puntos
        .iter()
        .take(q::POR_PETICION)
        .map(|(id, _)| {
            Ok((
                id.clone(),
                SenalPunto {
                    cubre: r.noul(&q::cubre(id))?,
                    contradice: r.noul(&q::contradice(id))?,
                },
            ))
        })
        .collect()
}

/// **El repaso de un alumno.**
#[derive(Debug, Clone)]
pub struct Repaso {
    pub clase: String,
    pub puntos: Vec<(String, String)>,
    pub estados: Vec<Estado>,
    pub explicacion: String,
}

impl Repaso {
    pub fn nuevo(clase: &str, puntos: &[String]) -> Repaso {
        Repaso {
            clase: clase.into(),
            puntos: puntos
                .iter()
                .enumerate()
                .map(|(i, t)| (format!("p{}", i + 1), t.clone()))
                .collect(),
            estados: vec![Estado::Falta; puntos.len()],
            explicacion: String::new(),
        }
    }

    /// Lo nuevo que dijo el alumno.
    pub fn oir(&mut self, texto: &str) {
        if !self.explicacion.is_empty() {
            self.explicacion.push(' ');
        }
        self.explicacion.push_str(texto.trim());
    }

    /// Los puntos que van en la siguiente peticion (los «mal» ya no).
    pub fn por_preguntar(&self) -> Vec<(String, String)> {
        self.puntos
            .iter()
            .zip(&self.estados)
            .filter(|(_, e)| **e != Estado::Mal)
            .map(|(p, _)| p.clone())
            .collect()
    }

    /// **La tabla entre peticiones**: lo que cambio (id, antes, ahora).
    pub fn decidir(&mut self, senales: &[(String, SenalPunto)]) -> Vec<(String, Estado, Estado)> {
        let mut v = Vec::new();
        for (id, s) in senales {
            let Some(i) = self.puntos.iter().position(|(p, _)| p == id) else {
                continue;
            };
            let antes = self.estados[i];
            let nuevo = leer_punto(s);
            let ahora = match (antes, nuevo) {
                (Estado::Mal, _) => Estado::Mal,
                (_, Estado::Mal) => Estado::Mal,
                (Estado::Cubierto, _) => Estado::Cubierto,
                (Estado::Dudoso, Estado::Cubierto) => Estado::Cubierto,
                (Estado::Dudoso, _) => Estado::Dudoso,
                (Estado::Falta, n) => n,
            };
            if ahora != antes {
                self.estados[i] = ahora;
                v.push((id.clone(), antes, ahora));
            }
        }
        v
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::grabadas;
    use crate::transporte::Grabado;
    use std::sync::Arc;

    fn s(cubre: f64, contradice: f64) -> SenalPunto {
        SenalPunto { cubre, contradice }
    }

    #[test]
    fn la_tabla_de_un_punto() {
        assert_eq!(leer_punto(&s(0.9, 0.8)), Estado::Mal);
        assert_eq!(leer_punto(&s(0.9, 0.1)), Estado::Cubierto);
        assert_eq!(leer_punto(&s(0.1, 0.1)), Estado::Falta);
        assert_eq!(leer_punto(&s(0.5, 0.5)), Estado::Dudoso);
    }

    #[test]
    fn entre_peticiones_lo_mal_se_queda() {
        let mut r = Repaso::nuevo("Fotosíntesis", &["a".into(), "b".into(), "c".into()]);
        let c = r.decidir(&[
            ("p1".into(), s(0.9, 0.0)),
            ("p2".into(), s(0.5, 0.0)),
            ("p3".into(), s(0.0, 0.0)),
        ]);
        assert_eq!(c.len(), 2);
        // Lo cubierto no vuelve a faltar; lo dudoso sube; lo mal se queda.
        r.decidir(&[
            ("p1".into(), s(0.1, 0.0)),
            ("p2".into(), s(0.9, 0.0)),
            ("p3".into(), s(0.9, 0.9)),
        ]);
        assert_eq!(
            r.estados,
            vec![Estado::Cubierto, Estado::Cubierto, Estado::Mal]
        );
        r.decidir(&[("p3".into(), s(0.9, 0.0))]);
        assert_eq!(r.estados[2], Estado::Mal);
        assert_eq!(r.por_preguntar().len(), 2);
    }

    #[test]
    fn pregunta_con_respuestas_grabadas() {
        let t = Arc::new(Grabado::nuevo(vec![Grabado::ok(&grabadas::nouls(&[
            ("cubre_p1", 0.9),
            ("contradice_p1", 0.05),
            ("cubre_p2", 0.2),
            ("contradice_p2", 0.8),
        ]))]));
        let c = Cliente::con_transporte("k", t.clone()).con_cache_propio();
        let mut r = Repaso::nuevo(
            "Fotosíntesis",
            &["Produce oxígeno".into(), "Usa luz".into()],
        );
        r.oir("La planta suelta oxígeno.");
        r.oir("Y lo hace de noche, sin luz.");
        let s = preguntar(&c, &r.clase, &r.explicacion, &r.por_preguntar()).unwrap();
        r.decidir(&s);
        assert_eq!(r.estados, vec![Estado::Cubierto, Estado::Mal]);
        let cuerpo = String::from_utf8(t.pedidos.lock().unwrap()[0].cuerpo.clone()).unwrap();
        assert!(cuerpo.contains("suelta oxígeno. Y lo hace de noche"));
        // Lo que falta: error.
        let t = Arc::new(Grabado::nuevo(vec![Grabado::ok(&grabadas::nouls(&[(
            "cubre_p1", 0.9,
        )]))]));
        let c = Cliente::con_transporte("k", t).con_cache_propio();
        assert!(preguntar(&c, "x", "y", &r.puntos).is_err());
    }
}
