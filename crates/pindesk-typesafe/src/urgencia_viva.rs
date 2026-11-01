//! **La urgencia de una tarea que cambia mientras se habla** (29-oct-2026,
//! prototipo `urgencia_en_vivo`): «esto es para hoy» sube la prioridad de
//! una tarea que ya existe. La prioridad la sigue calculando la tabla de
//! [`crate::urgencia`] (consecuencia, quien espera, plazo ajeno, piden ya y
//! los dias que faltan); lo dicho en la reunion solo cambia dos de sus
//! entradas:
//!
//! | `sobre_<id>` | `apura_<id>` | plazo en la frase (`plazo_dicho` ≥ SI) | cambia |
//! |---|---|---|---|
//! | ≥ SI | — | si | la fecha: la expresion elegida, resuelta en codigo |
//! | ≥ SI | ≥ SI | — | `piden_ya` = `apura` (alguien la pide ya, en voz alta) |
//! | < SI | — | — | nada |
//!
//! y luego `urgencia::decidir` con las senales y los dias nuevos. Sin las
//! senales de la tarea (Jev fallo al empezar), la prioridad es la de
//! siempre («normal») aunque cambie la fecha.

use crate::fechas::{Expresion, Fecha};
use crate::preguntas::SI;
use crate::urgencia::{self, Marca, Prioridad, Senales};
use crate::vivo::SenalTarea;

/// Una tarea que ya existe.
#[derive(Debug, Clone, PartialEq)]
pub struct TareaViva {
    pub id: String,
    pub texto: String,
    pub fecha: Option<Fecha>,
    pub senales: Option<Senales>,
    pub prioridad: Prioridad,
}

impl TareaViva {
    pub fn nueva(
        id: &str,
        texto: &str,
        fecha: Option<Fecha>,
        senales: Option<Senales>,
        hoy: Fecha,
    ) -> TareaViva {
        let mut t = TareaViva {
            id: id.into(),
            texto: texto.into(),
            fecha,
            senales,
            prioridad: Prioridad::Normal,
        };
        t.prioridad = t.calcular(hoy);
        t
    }

    fn calcular(&self, hoy: Fecha) -> Prioridad {
        urgencia::decidir(
            Marca::Ninguna,
            self.senales.as_ref(),
            self.fecha.map(|f| hoy.hasta(f)),
        )
    }
}

/// Lo que cambio de una tarea.
#[derive(Debug, Clone, PartialEq)]
pub struct Cambio {
    pub id: String,
    pub antes: Prioridad,
    pub despues: Prioridad,
    pub fecha_antes: Option<Fecha>,
    pub fecha_despues: Option<Fecha>,
}

/// **La tabla**: lo que dijo Jev de la tarea en una frase, y el plazo
/// elegido de la frase (si lo hay). `Some` si cambio la prioridad o la
/// fecha.
pub fn actualizar(
    t: &mut TareaViva,
    s: &SenalTarea,
    plazo: Option<&Expresion>,
    hoy: Fecha,
) -> Option<Cambio> {
    if s.sobre < SI {
        return None;
    }
    let antes = t.prioridad;
    let fecha_antes = t.fecha;
    if let Some(e) = plazo {
        t.fecha = Some(e.momento.fecha);
    }
    if s.apura >= SI
        && let Some(se) = t.senales.as_mut()
    {
        se.piden_ya = Some(se.piden_ya.unwrap_or(0.0).max(s.apura));
    }
    t.prioridad = t.calcular(hoy);
    (t.prioridad != antes || t.fecha != fecha_antes).then(|| Cambio {
        id: t.id.clone(),
        antes,
        despues: t.prioridad,
        fecha_antes,
        fecha_despues: t.fecha,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::Escala;
    use crate::fechas;

    fn hoy() -> Fecha {
        Fecha::nueva(2026, 10, 26).unwrap()
    }

    /// Consecuencia media (1.8), que otro espera: 2.8 sin fecha.
    fn senales() -> Senales {
        Senales {
            consecuencia: Escala {
                valor: 1.8,
                confianza: 0.6,
                niveles: vec![0.05, 0.2, 0.65, 0.1],
            },
            bloquea: 0.9,
            plazo_ajeno: 0.1,
            piden_ya: None,
        }
    }

    #[test]
    fn para_hoy_sube_la_prioridad() {
        let viernes = Fecha::nueva(2026, 10, 30);
        let mut t = TareaViva::nueva(
            "t1",
            "Mandar la cotización",
            viernes,
            Some(senales()),
            hoy(),
        );
        // 2.8 + 0.5 (en 4 dias) = 3.3: alta.
        assert_eq!(t.prioridad, Prioridad::Alta);
        let e = fechas::buscar("esto es para hoy", hoy());
        let c = actualizar(
            &mut t,
            &SenalTarea {
                sobre: 0.9,
                apura: 0.9,
            },
            e.first(),
            hoy(),
        )
        .unwrap();
        // 2.8 + 1 (piden ya) + 2 (hoy): urgente.
        assert_eq!((c.antes, c.despues), (Prioridad::Alta, Prioridad::Urgente));
        assert_eq!(c.fecha_despues, Some(hoy()));
        // Otra frase que no es de la tarea: nada.
        assert!(
            actualizar(
                &mut t,
                &SenalTarea {
                    sobre: 0.2,
                    apura: 0.9
                },
                None,
                hoy()
            )
            .is_none()
        );
        // De la tarea, sin prisa ni fecha: nada cambia.
        assert!(
            actualizar(
                &mut t,
                &SenalTarea {
                    sobre: 0.9,
                    apura: 0.1
                },
                None,
                hoy()
            )
            .is_none()
        );
    }

    #[test]
    fn mas_tiempo_la_baja_y_sin_senales_es_normal() {
        let mut t = TareaViva::nueva("t1", "x", Some(hoy()), Some(senales()), hoy());
        // 2.8 + 2 (hoy) = 4.8: alta.
        assert_eq!(t.prioridad, Prioridad::Alta);
        let e = fechas::buscar("eso puede esperar al 20 de noviembre", hoy());
        let c = actualizar(
            &mut t,
            &SenalTarea {
                sobre: 0.9,
                apura: 0.05,
            },
            e.first(),
            hoy(),
        )
        .unwrap();
        assert_eq!(c.despues, Prioridad::Normal);
        let mut sin = TareaViva::nueva("t2", "y", None, None, hoy());
        let c = actualizar(
            &mut sin,
            &SenalTarea {
                sobre: 0.9,
                apura: 0.9,
            },
            fechas::buscar("para hoy", hoy()).first(),
            hoy(),
        )
        .unwrap();
        assert_eq!(c.despues, Prioridad::Normal);
        assert_eq!(c.fecha_despues, Some(hoy()));
    }
}
