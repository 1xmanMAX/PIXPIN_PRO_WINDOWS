//! **Las ideas de la reunion juntas, frase a frase** (29-oct-2026, lo de
//! `examples/reunion_completa.rs` hecho biblioteca para la app): una
//! [`Mesa`] por reunion guarda el estado de todas (las tareas que ya
//! existen, las preguntas abiertas, los eventos propuestos, el reloj de los
//! temas y sus avisos) y, por cada frase, arma **una sola peticion**
//! ([`crate::vivo`]) con las partes encendidas ([`Ideas`]) y pasa las
//! respuestas por la tabla de cada idea. Lo que sale son [`Suceso`]s: lo
//! que la app propone, cambia o apunta.
//!
//! | idea | parte de `vivo` | tabla | suceso |
//! |---|---|---|---|
//! | tareas dichas | `compromiso` | `compromisos::decidir` | [`Suceso::Tarea`] |
//! | urgencia en vivo | `tareas` | `urgencia_viva::actualizar` | [`Suceso::Cambio`] |
//! | decisiones | `decision` | `vivo::es_decision` | [`Suceso::Decision`] |
//! | preguntas pendientes | `pregunta`, `abiertas` | `Pendientes::tras_frase` | [`Suceso::Abierta`], [`Suceso::Contestada`] |
//! | fechas al horario | `cita` | `Horario::proponer` | [`Suceso::Evento`] |
//! | tiempo por tema | `tema` | `tiempo_de_temas::asignar` | [`Suceso::Tiempos`] |
//! | tema olvidado | `tema`, `cierre` | `Avisador::avisar` | [`Suceso::Aviso`] |
//!
//! **Un fallo de Jev no cambia nada** de lo que decide Jev: sin respuesta no
//! hay propuestas, cambios, decisiones ni preguntas. El reloj de los temas
//! si sigue (la frase va al tema de la anterior, como una duda: es la tabla
//! de `tiempo_de_temas`) y los avisos que solo dependen de la hora tambien.
//!
//! Los umbrales son los de cada tabla (SI = 0.7, NO = 0.3): solo propone lo
//! que Jev da por seguro; lo dudoso no se ensena.

use crate::citas::{self, Horario};
use crate::compromisos::{self, Propuesta};
use crate::fechas::{Expresion, Fecha};
use crate::pendientes::{Abierta, Contestada, Evento, Pendientes};
use crate::tiempo_de_temas::{self, Avisador, Aviso, Cronometro, Destino};
use crate::urgencia_viva::{self, Cambio, TareaViva};
use crate::vivo::{self, Contexto, Frase, Partes, Preparada, Senales};
use crate::{Cliente, Error};

/// Quien habla, cuando no se sabe (la escucha en vivo no separa voces): va
/// en el estado como el que dice la frase. Sin participantes no se
/// pregunta quien hace una tarea.
pub const QUIEN_NO_SE: &str = "Voz";

/// **Que ideas estan encendidas** (los interruptores de `[jev]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ideas {
    pub tareas: bool,
    pub urgencia: bool,
    pub decisiones: bool,
    pub preguntas: bool,
    pub citas: bool,
    pub tiempo_por_tema: bool,
    pub tema_olvidado: bool,
}

impl Ideas {
    /// Todas encendidas.
    pub fn todas() -> Ideas {
        Ideas {
            tareas: true,
            urgencia: true,
            decisiones: true,
            preguntas: true,
            citas: true,
            tiempo_por_tema: true,
            tema_olvidado: true,
        }
    }

    /// Si hay alguna encendida.
    pub fn alguna(self) -> bool {
        self.tareas
            || self.urgencia
            || self.decisiones
            || self.preguntas
            || self.citas
            || self.tiempo_por_tema
            || self.tema_olvidado
    }
}

/// **Lo que sale de una frase.**
#[derive(Debug, Clone, PartialEq)]
pub enum Suceso {
    /// Una tarea propuesta (se acepta o se descarta: nunca se apunta sola).
    Tarea(Propuesta),
    /// Un evento propuesto para el horario.
    Evento(citas::Evento),
    /// Una tarea que ya existia cambia de prioridad o de fecha, con su
    /// texto.
    Cambio(Cambio, String),
    /// Se decidio algo: el minuto y la frase.
    Decision { ms: i64, texto: String },
    /// Una pregunta queda abierta.
    Abierta(Abierta),
    /// Una pregunta abierta se contesto.
    Contestada(Contestada),
    /// Lo que lleva cada tema de la agenda, y lo de fuera, hasta `hasta_ms`.
    Tiempos {
        por_tema: Vec<i64>,
        otro: i64,
        hasta_ms: i64,
    },
    /// Queda poco, se paso la hora o estan cerrando y hay temas sin tratar
    /// (los indices de la agenda).
    Aviso(Aviso),
}

/// **El estado de una reunion** para las ideas de frase a frase.
#[derive(Debug, Clone)]
pub struct Mesa {
    pub ideas: Ideas,
    pub reunion: String,
    pub agenda: Vec<String>,
    /// El dia de la reunion (para resolver «el viernes»).
    pub hoy: Fecha,
    antes: Vec<Frase>,
    tareas: Vec<TareaViva>,
    pendientes: Pendientes,
    horario: Horario,
    cronometro: Cronometro,
    /// Sin duracion prevista no hay avisos de tiempo.
    avisador: Option<Avisador>,
    /// Los temas que ya se dieron por tratados fuera de aqui (marcados a
    /// mano o por los temas en vivo): no se avisa de ellos.
    tratados: Vec<usize>,
}

impl Mesa {
    /// Una reunion nueva. `duracion_ms`: la prevista, si se dijo.
    /// `tareas`: las que ya existen y pueden cambiar de prioridad (como
    /// mucho `preguntas::vivo::POR_PETICION` van en cada peticion).
    pub fn nueva(
        ideas: Ideas,
        reunion: &str,
        agenda: Vec<String>,
        hoy: Fecha,
        duracion_ms: Option<i64>,
        tareas: Vec<TareaViva>,
    ) -> Mesa {
        let n = agenda.len();
        Mesa {
            ideas,
            reunion: reunion.to_string(),
            agenda,
            hoy,
            antes: Vec::new(),
            tareas,
            pendientes: Pendientes::default(),
            horario: Horario::default(),
            cronometro: Cronometro::nuevo(n),
            avisador: duracion_ms.filter(|d| *d > 0).map(Avisador::nuevo),
            tratados: Vec::new(),
        }
    }

    /// Una tarea mas (una propuesta aceptada) o la misma cambiada (un
    /// «Deshacer»): por su id.
    pub fn tarea(&mut self, t: TareaViva) {
        match self.tareas.iter_mut().find(|x| x.id == t.id) {
            Some(x) => *x = t,
            None => self.tareas.push(t),
        }
    }

    /// La tarea con este id, como esta ahora.
    pub fn tarea_de(&self, id: &str) -> Option<&TareaViva> {
        self.tareas.iter().find(|t| t.id == id)
    }

    /// Los temas de la agenda que ya estan tratados (por indice).
    pub fn tratados(&mut self, v: Vec<usize>) {
        self.tratados = v;
    }

    /// Las preguntas que siguen sin respuesta.
    pub fn abiertas(&self) -> &[Abierta] {
        self.pendientes.abiertas()
    }

    /// Lo contado de cada tema hasta `ms`.
    pub fn tiempos(&self, ms: i64) -> Cronometro {
        self.cronometro.hasta(ms)
    }

    fn con_tema(&self) -> bool {
        (self.ideas.tiempo_por_tema || self.ideas.tema_olvidado) && !self.agenda.is_empty()
    }

    fn con_cierre(&self) -> bool {
        self.ideas.tema_olvidado && self.avisador.is_some() && !self.agenda.is_empty()
    }

    /// Si hay algo que preguntar por esta frase (alguna idea encendida y
    /// la frase con palabras de sobra).
    pub fn vale_la_pena(&self, f: &Frase) -> bool {
        self.ideas.alguna() && vivo::vale_la_pena(f)
    }

    /// Las piezas de la peticion de `frase`, y con ellas se llama a `f`.
    fn con_peticion<R>(&self, frase: &Frase, f: impl FnOnce(&Contexto, &Partes) -> R) -> R {
        let ids: Vec<(String, String)> = if self.ideas.urgencia {
            self.tareas
                .iter()
                .map(|t| (t.id.clone(), t.texto.clone()))
                .collect()
        } else {
            Vec::new()
        };
        let abiertas = if self.ideas.preguntas {
            self.pendientes.para_preguntar()
        } else {
            Vec::new()
        };
        let partes = Partes {
            compromiso: self.ideas.tareas,
            decision: self.ideas.decisiones,
            pregunta: self.ideas.preguntas,
            cita: self.ideas.citas,
            cierre: self.con_cierre(),
            tema: self.con_tema(),
            tareas: &ids,
            abiertas: &abiertas,
            chats: &[],
        };
        let tema_anterior = match self.cronometro.anterior() {
            Some(Destino::Tema(i)) => self.agenda.get(i).map(String::as_str),
            _ => None,
        };
        let ctx = Contexto {
            reunion: &self.reunion,
            participantes: &[],
            agenda: if partes.tema { &self.agenda } else { &[] },
            antes: &self.antes,
            frase,
            hoy: self.hoy,
            tema_anterior,
        };
        f(&ctx, &partes)
    }

    /// **La peticion de una frase** (para verla en las pruebas o el
    /// registro).
    pub fn preparar(&self, frase: &Frase) -> Preparada {
        self.con_peticion(frase, |c, p| vivo::preparar(c, p))
    }

    /// **Pregunta por una frase**: una sola peticion con todas las partes
    /// encendidas. Un fallo es un error (nunca un «no»).
    pub fn preguntar(
        &self,
        cliente: &Cliente,
        frase: &Frase,
    ) -> Result<(Senales, Vec<Expresion>), Error> {
        self.con_peticion(frase, |c, p| vivo::preguntar(cliente, c, p))
    }

    /// **Lo que pasa con una frase**: lo que dijo Jev (`None` si fallo o
    /// no se pregunto) por las tablas. `fin_ms`: cuando se dejo de hablar
    /// en ella (hasta ahi cuenta el reloj de los temas y la hora de los
    /// avisos).
    pub fn tras_frase(
        &mut self,
        frase: Frase,
        respuesta: Option<&(Senales, Vec<Expresion>)>,
        fin_ms: i64,
    ) -> Vec<Suceso> {
        let mut v = Vec::new();
        if let Some((sen, fe)) = respuesta {
            self.lo_que_dijo_jev(&frase, sen, fe, &mut v);
        }
        let sen = respuesta.map(|x| &x.0);
        if self.con_tema() {
            let d = tiempo_de_temas::asignar(
                sen.and_then(|s| s.tema.as_ref()),
                self.cronometro.anterior(),
            );
            self.cronometro.frase(frase.ms, d);
            if self.ideas.tiempo_por_tema {
                let h = self.cronometro.hasta(fin_ms);
                v.push(Suceso::Tiempos {
                    por_tema: h.por_tema,
                    otro: h.otro,
                    hasta_ms: fin_ms,
                });
            }
            if self.ideas.tema_olvidado {
                self.avisos(sen.and_then(|s| s.cierre), fin_ms, &mut v);
            }
        }
        self.antes.push(frase);
        let sobran = self.antes.len().saturating_sub(vivo::ANTES);
        self.antes.drain(..sobran);
        v
    }

    fn lo_que_dijo_jev(&mut self, f: &Frase, sen: &Senales, fe: &[Expresion], v: &mut Vec<Suceso>) {
        if self.ideas.tareas
            && let Some(c) = &sen.compromiso
            && let compromisos::Resultado::Propuesta(t) =
                compromisos::decidir(f, c, sen.plazo.as_ref(), fe, self.hoy)
        {
            v.push(Suceso::Tarea(t));
        }
        if self.ideas.urgencia {
            let plazo = compromisos::plazo_elegido(sen.plazo.as_ref(), fe);
            for (id, st) in &sen.tareas {
                let Some(t) = self.tareas.iter_mut().find(|t| t.id == *id) else {
                    continue;
                };
                if let Some(c) = urgencia_viva::actualizar(t, st, plazo, self.hoy) {
                    v.push(Suceso::Cambio(c, t.texto.clone()));
                }
            }
        }
        if self.ideas.decisiones && vivo::es_decision(sen) {
            v.push(Suceso::Decision {
                ms: f.ms,
                texto: crate::recortar(&f.dice, 300),
            });
        }
        if self.ideas.preguntas {
            for ev in self
                .pendientes
                .tras_frase(f, sen.pregunta, &sen.respondidas)
            {
                match ev {
                    Evento::Abierta(a) => v.push(Suceso::Abierta(a)),
                    Evento::Contestada(c) => v.push(Suceso::Contestada(c)),
                    Evento::Quiza { .. } => {}
                }
            }
        }
        if self.ideas.citas
            && let citas::Resultado::Evento(e) = self.horario.proponer(f, sen.cita.as_ref(), fe)
        {
            v.push(Suceso::Evento(e));
        }
    }

    fn avisos(&mut self, cierre: Option<f64>, ms: i64, v: &mut Vec<Suceso>) {
        let actual = self.cronometro.anterior();
        let sin: Vec<usize> = self
            .cronometro
            .hasta(ms)
            .sin_tratar()
            .into_iter()
            .filter(|i| actual != Some(Destino::Tema(*i)) && !self.tratados.contains(i))
            .collect();
        if let Some(a) = self.avisador.as_mut() {
            v.extend(a.avisar(ms, cierre, &sin).into_iter().map(Suceso::Aviso));
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::grabadas;
    use crate::transporte::Grabado;
    use crate::urgencia::Prioridad;
    use std::sync::Arc;

    fn hoy() -> Fecha {
        Fecha::nueva(2026, 10, 26).unwrap()
    }

    fn frase(ms: i64, dice: &str) -> Frase {
        Frase {
            ms,
            quien: QUIEN_NO_SE.into(),
            dice: dice.into(),
        }
    }

    fn cliente(respuestas: &[String]) -> (Cliente, Arc<Grabado>) {
        let t = Arc::new(Grabado::nuevo(
            respuestas.iter().map(|r| Grabado::ok(r)).collect(),
        ));
        (
            Cliente::con_transporte("k", t.clone()).con_cache_propio(),
            t,
        )
    }

    #[test]
    fn las_partes_van_segun_las_ideas_encendidas() {
        let tareas = vec![TareaViva::nueva(
            "t1",
            "Pagar al proveedor",
            None,
            None,
            hoy(),
        )];
        let agenda = vec!["Paro".to_string(), "Excursión".to_string()];
        let m = Mesa::nueva(
            Ideas::todas(),
            "Coordinación",
            agenda.clone(),
            hoy(),
            None,
            tareas.clone(),
        );
        let f = frase(0, "yo me encargo de mandar la cotización el viernes");
        let p = m.preparar(&f);
        let nombres: Vec<&str> = p.peticion.questions.keys().map(String::as_str).collect();
        // Sin duracion prevista no se pregunta si se esta cerrando; sin
        // participantes, ni quien.
        assert_eq!(
            nombres,
            [
                "apura_t1",
                "cita",
                "compromiso",
                "de_la_agenda",
                "decision",
                "plazo_dicho",
                "pregunta",
                "sobre_t1",
                "tema",
                "urgente"
            ]
        );
        assert!(p.peticion.state.get("participantes").is_none());
        assert_eq!(p.peticion.state["frase"]["quien"], QUIEN_NO_SE);
        // Solo las decisiones: una pregunta, sin agenda en el estado.
        let solo = Ideas {
            decisiones: true,
            ..Default::default()
        };
        let m = Mesa::nueva(solo, "", agenda, hoy(), Some(1_800_000), tareas);
        let p = m.preparar(&f);
        assert_eq!(p.peticion.questions.len(), 1);
        assert!(p.peticion.state.get("agenda").is_none());
        assert!(!Ideas::default().alguna());
    }

    #[test]
    fn una_frase_da_sus_sucesos_y_un_fallo_no_propone_nada() {
        let tareas = vec![TareaViva::nueva(
            "t1",
            "Pagar al proveedor",
            Some(hoy().mas(10)),
            None,
            hoy(),
        )];
        let mut m = Mesa::nueva(
            Ideas::todas(),
            "Coordinación",
            vec!["Paro".into(), "Excursión".into()],
            hoy(),
            Some(20 * 60_000),
            tareas,
        );
        let r = grabadas::respuesta(&[
            grabadas::noul("compromiso", 0.92),
            grabadas::noul("urgente", 0.1),
            grabadas::noul("plazo_dicho", 0.9),
            grabadas::noul("decision", 0.85),
            grabadas::noul("pregunta", 0.05),
            grabadas::noul("cita", 0.1),
            grabadas::noul("cierre", 0.05),
            grabadas::noul("de_la_agenda", 0.9),
            grabadas::choice("tema", "excursion", &["paro"]),
            grabadas::noul("sobre_t1", 0.9),
            grabadas::noul("apura_t1", 0.2),
        ]);
        let (cl, t) = cliente(&[r]);
        let f = frase(60_000, "queda decidido: yo mando la cotización el viernes");
        let resp = m.preguntar(&cl, &f).unwrap();
        assert_eq!(t.llamadas(), 1, "una sola peticion por frase");
        let v = m.tras_frase(f, Some(&resp), 64_000);
        let Suceso::Tarea(p) = &v[0] else {
            panic!("{v:?}")
        };
        assert_eq!(p.fecha.unwrap().fecha.to_string(), "2026-10-30");
        // La tarea que ya existia: su fecha pasa al viernes (sin sus senales
        // de urgencia, la prioridad sigue normal).
        let Suceso::Cambio(c, texto) = &v[1] else {
            panic!("{v:?}")
        };
        assert_eq!(texto, "Pagar al proveedor");
        assert_eq!(c.fecha_despues.unwrap().to_string(), "2026-10-30");
        assert_eq!(c.antes, Prioridad::Normal);
        assert!(matches!(&v[2], Suceso::Decision { ms: 60_000, .. }));
        assert!(matches!(
            &v[3],
            Suceso::Tiempos {
                hasta_ms: 64_000,
                ..
            }
        ));
        // Un fallo: el reloj sigue al tema anterior, nada mas.
        let f = frase(70_000, "y ahora vamos con lo del autobús del viernes");
        let v = m.tras_frase(f, None, 75_000);
        assert_eq!(v.len(), 1);
        let Suceso::Tiempos { por_tema, .. } = &v[0] else {
            panic!("{v:?}")
        };
        assert_eq!(por_tema, &vec![0, 15_000]);
        // Se pasa la hora con el paro sin tratar.
        let v = m.tras_frase(
            frase(21 * 60_000, "bueno, se nos fue la hora"),
            None,
            21 * 60_000,
        );
        assert!(
            v.iter().any(
                |s| matches!(s, Suceso::Aviso(Aviso::TiempoCumplido { temas }) if temas == &[0])
            ),
            "{v:?}"
        );
    }

    #[test]
    fn lo_tratado_fuera_no_se_avisa() {
        let mut m = Mesa::nueva(
            Ideas::todas(),
            "",
            vec!["Paro".into()],
            hoy(),
            Some(60_000),
            Vec::new(),
        );
        m.tratados(vec![0]);
        let v = m.tras_frase(
            frase(120_000, "se acabó el tiempo de verdad"),
            None,
            120_000,
        );
        assert!(!v.iter().any(|s| matches!(s, Suceso::Aviso(_))), "{v:?}");
    }
}
