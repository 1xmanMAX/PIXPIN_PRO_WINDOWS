//! **Cuanto tiempo se dedico a cada tema y que tema se olvida**
//! (29-oct-2026, prototipos `tiempo_por_tema` y `tema_olvidado`). Los dos
//! comparten una peticion por frase (`crate::vivo`, partes `tema` y
//! `cierre`): «¿es de la agenda?» (Noul), «¿de que tema?» (Choice sobre la
//! agenda) y «¿se esta cerrando la reunion?» (Noul). Lo demas es codigo:
//! sumar, comparar con la hora y avisar.
//!
//! **A que tema va una frase** ([`asignar`]). La guia de TypeSafe: una
//! Choice siempre elige algo, asi que el «otro» lo dice un Noul aparte:
//!
//! | `de_la_agenda` | va a |
//! |---|---|
//! | ≥ SI | el tema que elige `tema` |
//! | ≤ NO | «otro» |
//! | entre NO y SI | el tema de la frase anterior (un «vale, sí» sigue el tema); si no hay, «otro» |
//!
//! **El tiempo** ([`Cronometro`]): cada frase cuenta desde que empieza
//! hasta que empieza la siguiente, como mucho [`LARGO_MAXIMO_MS`] (un
//! silencio largo no es de nadie). Un tema con al menos
//! [`MINIMO_TRATADO_MS`] cuenta como tratado.
//!
//! **Los avisos** ([`Avisador`]), con la duracion prevista de la reunion y
//! los temas aun sin tratar:
//!
//! | situacion | aviso (una vez cada uno) |
//! |---|---|
//! | queda ≤ max(5 min, 15 % de la duracion) y hay temas sin tratar | «queda poco» con los temas |
//! | se paso la hora y hay temas sin tratar | «tiempo cumplido» |
//! | `cierre` ≥ SI y hay temas sin tratar | «estan cerrando» |
//! | `cierre` entre NO y SI | nada |

use crate::preguntas::{NO, SI};
use crate::vivo::Tema;

/// Lo que cuenta como mucho una frase.
pub const LARGO_MAXIMO_MS: i64 = 60_000;
/// Lo que hace falta para que un tema cuente como tratado.
pub const MINIMO_TRATADO_MS: i64 = 30_000;
/// Lo que se avisa antes del final como poco.
pub const AVISO_MINIMO_MS: i64 = 5 * 60_000;

/// A donde va una frase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destino {
    /// El tema de la agenda con este indice.
    Tema(usize),
    Otro,
}

/// **La tabla de a que tema va.** `None` en `t`: Jev no contesto (sigue el
/// anterior, como la duda).
pub fn asignar(t: Option<&Tema>, anterior: Option<Destino>) -> Destino {
    match t {
        Some(t) if t.de_la_agenda >= SI => Destino::Tema(t.cual),
        Some(t) if t.de_la_agenda <= NO => Destino::Otro,
        _ => anterior.unwrap_or(Destino::Otro),
    }
}

/// **El tiempo de cada tema.**
#[derive(Debug, Clone)]
pub struct Cronometro {
    pub por_tema: Vec<i64>,
    pub otro: i64,
    ultima: Option<(i64, Destino)>,
}

impl Cronometro {
    pub fn nuevo(temas: usize) -> Cronometro {
        Cronometro {
            por_tema: vec![0; temas],
            otro: 0,
            ultima: None,
        }
    }

    fn sumar(&mut self, d: Destino, ms: i64) {
        let ms = ms.clamp(0, LARGO_MAXIMO_MS);
        match d {
            Destino::Tema(i) => {
                if let Some(x) = self.por_tema.get_mut(i) {
                    *x += ms;
                }
            }
            Destino::Otro => self.otro += ms,
        }
    }

    /// Una frase que empieza en `ms` va a `d`: lo de la anterior se cuenta
    /// hasta aqui.
    pub fn frase(&mut self, ms: i64, d: Destino) {
        if let Some((desde, prev)) = self.ultima {
            self.sumar(prev, ms - desde);
        }
        self.ultima = Some((ms, d));
    }

    /// El destino de la ultima frase.
    pub fn anterior(&self) -> Option<Destino> {
        self.ultima.map(|(_, d)| d)
    }

    /// Lo contado hasta `ms` (sin cerrar: la ultima frase sigue).
    pub fn hasta(&self, ms: i64) -> Cronometro {
        let mut c = self.clone();
        if let Some((desde, prev)) = c.ultima.take() {
            c.sumar(prev, ms - desde);
        }
        c
    }

    /// Los temas aun sin tratar (menos de [`MINIMO_TRATADO_MS`]).
    pub fn sin_tratar(&self) -> Vec<usize> {
        (0..self.por_tema.len())
            .filter(|i| self.por_tema[*i] < MINIMO_TRATADO_MS)
            .collect()
    }
}

/// Un aviso de tema olvidado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aviso {
    QuedaPoco { quedan_ms: i64, temas: Vec<usize> },
    TiempoCumplido { temas: Vec<usize> },
    Cierran { temas: Vec<usize> },
}

/// **Los avisos de la reunion.**
#[derive(Debug, Clone)]
pub struct Avisador {
    pub duracion_ms: i64,
    poco: bool,
    cumplido: bool,
    cierran: bool,
}

impl Avisador {
    pub fn nuevo(duracion_ms: i64) -> Avisador {
        Avisador {
            duracion_ms,
            poco: false,
            cumplido: false,
            cierran: false,
        }
    }

    /// Lo que queda para avisar «queda poco».
    pub fn margen_ms(&self) -> i64 {
        AVISO_MINIMO_MS.max(self.duracion_ms * 15 / 100)
    }

    /// **La tabla**, a los `ms` de empezar, con lo que dijo Jev del cierre
    /// en la ultima frase y los temas sin tratar.
    pub fn avisar(&mut self, ms: i64, cierre: Option<f64>, sin_tratar: &[usize]) -> Vec<Aviso> {
        let mut v = Vec::new();
        if sin_tratar.is_empty() {
            return v;
        }
        let quedan = self.duracion_ms - ms;
        if quedan <= 0 && !self.cumplido {
            self.cumplido = true;
            self.poco = true;
            v.push(Aviso::TiempoCumplido {
                temas: sin_tratar.to_vec(),
            });
        } else if quedan > 0 && quedan <= self.margen_ms() && !self.poco {
            self.poco = true;
            v.push(Aviso::QuedaPoco {
                quedan_ms: quedan,
                temas: sin_tratar.to_vec(),
            });
        }
        if cierre.is_some_and(|p| p >= SI) && !self.cierran {
            self.cierran = true;
            v.push(Aviso::Cierran {
                temas: sin_tratar.to_vec(),
            });
        }
        v
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn t(de_la_agenda: f64, cual: usize) -> Tema {
        Tema {
            de_la_agenda,
            cual,
            p_cual: 0.8,
        }
    }

    #[test]
    fn a_que_tema_va_cada_frase() {
        assert_eq!(asignar(Some(&t(0.9, 2)), None), Destino::Tema(2));
        assert_eq!(
            asignar(Some(&t(0.1, 2)), Some(Destino::Tema(1))),
            Destino::Otro
        );
        // La duda sigue el tema anterior.
        assert_eq!(
            asignar(Some(&t(0.5, 2)), Some(Destino::Tema(1))),
            Destino::Tema(1)
        );
        assert_eq!(asignar(Some(&t(0.5, 2)), None), Destino::Otro);
        assert_eq!(asignar(None, Some(Destino::Tema(0))), Destino::Tema(0));
    }

    #[test]
    fn el_cronometro_cuenta_hasta_la_siguiente_frase() {
        let mut c = Cronometro::nuevo(3);
        c.frase(0, Destino::Otro);
        c.frase(10_000, Destino::Tema(0));
        c.frase(25_000, Destino::Tema(0));
        // Un silencio de 5 minutos cuenta 60 s.
        c.frase(325_000, Destino::Tema(1));
        let h = c.hasta(345_000);
        assert_eq!(h.otro, 10_000);
        assert_eq!(h.por_tema, vec![15_000 + 60_000, 20_000, 0]);
        assert_eq!(h.sin_tratar(), vec![1, 2]);
        assert_eq!(c.anterior(), Some(Destino::Tema(1)));
    }

    #[test]
    fn los_avisos_salen_una_vez() {
        // 20 minutos: avisa a 5 min del final.
        let mut a = Avisador::nuevo(20 * 60_000);
        assert_eq!(a.margen_ms(), 5 * 60_000);
        assert!(a.avisar(10 * 60_000, Some(0.1), &[2]).is_empty());
        let v = a.avisar(16 * 60_000, Some(0.1), &[2]);
        assert_eq!(
            v,
            vec![Aviso::QuedaPoco {
                quedan_ms: 4 * 60_000,
                temas: vec![2]
            }]
        );
        assert!(a.avisar(17 * 60_000, Some(0.5), &[2]).is_empty());
        assert_eq!(
            a.avisar(18 * 60_000, Some(0.9), &[2]),
            vec![Aviso::Cierran { temas: vec![2] }]
        );
        assert_eq!(
            a.avisar(21 * 60_000, None, &[2]),
            vec![Aviso::TiempoCumplido { temas: vec![2] }]
        );
        // Sin temas pendientes, nada.
        let mut b = Avisador::nuevo(60 * 60_000);
        assert_eq!(b.margen_ms(), 9 * 60_000);
        assert!(b.avisar(70 * 60_000, Some(0.95), &[]).is_empty());
    }
}
