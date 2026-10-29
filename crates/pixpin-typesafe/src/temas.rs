//! **Que temas de la agenda ya se trataron**, mientras se graba la reunion
//! y **en vivo** (28-oct-2026; en vivo de verdad el 29-oct-2026, el
//! usuario: «en cuanto termino de decir un tema, que se marque»,
//! `docs/jev.md`).
//!
//! Lo que se dice llega frase a frase, ya pasado a texto ([`Dicho`]), a un
//! [`Rollo`]: los ultimos [`VENTANA_MS`] de la conversacion. Cada vez que
//! hay texto nuevo, [`Ritmo`] decide si ya toca preguntar (una peticion en
//! vuelo como mucho, [`HUECO_MS`] entre una y otra; lo que llega mientras
//! tanto se junta para la siguiente). La peticion es **una sola** (todo
//! comparte el estado: la conversacion, la agenda y el titulo) con tres
//! Nouls por cada tema pendiente: «¿se trato en `conversacion`?», «¿quedo
//! cerrado?» y «¿lo que se acaba de decir (`conversacion.ahora`) es de
//! ese tema?» (las preguntas, en `preguntas::temas`). La decision la toma
//! el codigo, con esta tabla.
//!
//! **Tabla de decision** ([`Seguimiento::decidir`]), por tema y por
//! peticion, con [`SI`] = 0.7 y [`CASI`] = 0.5. «Recordado» es una
//! peticion anterior, de hace como mucho [`VIGENCIA_MS`], en que el tema
//! quedo a medias:
//!
//! | tratado | cerrado | ahora | recordado | se marca |
//! |---|---|---|---|---|
//! | ≥ SI | — | — | — | tratado, `p` = tratado |
//! | ≥ CASI y < SI | ≥ SI | — | — | tratado (se hablo y se cerro), `p` = cerrado |
//! | ≥ CASI y < SI | < SI | ≥ CASI | si | tratado (dos peticiones a medias, la segunda con algo nuevo del tema), `p` = la mayor |
//! | ≥ CASI y < SI | < SI | < CASI, o sin recordado | — | nada aun: se recuerda (o se mantiene lo recordado) |
//! | < CASI | — | — | — | nada (y se olvida lo recordado) |
//!
//! El milisegundo de lo marcado es el del principio de `conversacion.ahora`
//! (la primera frase nueva) si lo nuevo es del tema, o el del principio de
//! la conversacion si no; y si habia algo recordado, el de entonces:
//! ahi empezo a tratarse.
//!
//! **Por que «ahora»**: las ventanas se solapan (cada peticion vuelve a
//! llevar los ~40 s de antes), asi que «dos a medias seguidas» podria ser
//! la misma frase leida dos veces. La segunda solo cuenta si trae algo
//! nuevo del tema. Las dos de arriba no lo piden: con mas conversacion
//! delante Jev puede ver ya claro lo que antes no.
//!
//! Un tema «cerrado» que no se trato (tratado < CASI) no se marca: el cierre
//! seria de otra cosa. Uno que solo se anuncia para luego («eso lo vemos al
//! final») no se marca: lo dicen los criterios. Jev solo marca, nunca
//! desmarca, y solo los temas que siguen pendientes y que nadie toco a mano
//! (eso lo decide quien llama, `pixpin_proyecto::temas`).
//!
//! Sin clave o si Jev falla, no se marca nada: el fallo es un error, nunca
//! un «no se trato», y lo nuevo no se da por mirado (va en la siguiente,
//! que espera [`TRAS_FALLO_MS`]). Sin texto nuevo no se pregunta.

use std::collections::{HashMap, VecDeque};

use serde_json::{Map, Value, json};

use crate::preguntas::SI;
use crate::preguntas::temas as q;
pub use crate::preguntas::temas::CASI;
use crate::{Cliente, Error, Peticion};

/// Lo que se le ensena a Jev de la conversacion: los ultimos 40 s (lo que
/// tarda en tratarse un tema corto; lo de antes ya se miro).
pub const VENTANA_MS: i64 = 40_000;
/// Lo que vale un «a medias» recordado: si la siguiente peticion a medias
/// llega mas tarde, se empieza de cero.
pub const VIGENCIA_MS: i64 = 60_000;
/// Lo menos entre dos peticiones.
pub const HUECO_MS: u64 = 1_500;
/// Lo que se espera despues de un fallo.
pub const TRAS_FALLO_MS: u64 = 5_000;
/// Las palabras nuevas que hacen falta para preguntar (un «vale» suelto no
/// trae nada; espera a juntarse con lo siguiente).
pub const PALABRAS_MINIMAS: usize = 3;
/// Lo largo de `conversacion.ahora` (unos 20 s hablados).
const LARGO_AHORA: usize = 1200;
/// Y de `conversacion.antes`.
const LARGO_ANTES: usize = 2400;
/// Y de cada tema de la agenda.
const LARGO_TEMA: usize = 160;

/// Un tema que aun hay que mirar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pendiente {
    pub id: String,
    pub texto: String,
}

/// **Una frase pasada a texto**: el milisegundo de la grabacion en que
/// empieza y en que se dejo de hablar, lo que se dijo y, en el reloj de
/// quien llama (monotono, en ms), cuando se dejo de hablar: para medir lo
/// que tarda el ✓.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dicho {
    pub ms: i64,
    pub fin_ms: i64,
    pub texto: String,
    pub reloj_fin: u64,
}

/// **Lo que se le ensena a Jev en una peticion.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ventana {
    /// Lo ya mirado de los ultimos [`VENTANA_MS`], si hay.
    pub antes: Option<String>,
    /// Lo nuevo.
    pub ahora: String,
    /// Donde empieza lo nuevo, en la grabacion.
    pub ms_ahora: i64,
    /// Donde empieza la conversacion entera.
    pub ms_desde: i64,
    /// Donde acaba (el fin de la ultima frase).
    pub fin_ms: i64,
    /// Cuando se dejo de hablar en la primera y en la ultima frase nueva
    /// (el reloj de [`Dicho::reloj_fin`]).
    pub reloj_primera: u64,
    pub reloj_ultima: u64,
    /// Hasta donde se mira (para [`Rollo::mirado`]).
    hasta: u64,
}

/// **Los ultimos segundos de la conversacion**, frase a frase, y hasta
/// donde los miro ya Jev.
#[derive(Debug, Clone, Default)]
pub struct Rollo {
    dichos: VecDeque<Dicho>,
    /// El numero (desde el principio) de `dichos[0]`.
    base: u64,
    /// Los dichos con numero menor ya se miraron.
    mirados: u64,
}

impl Rollo {
    /// Una frase mas (las vacias no cuentan).
    pub fn meter(&mut self, d: Dicho) {
        if d.texto.trim().is_empty() {
            return;
        }
        self.dichos.push_back(d);
    }

    fn nuevos(&self) -> impl Iterator<Item = &Dicho> {
        let saltar = self.mirados.saturating_sub(self.base) as usize;
        self.dichos.iter().skip(saltar)
    }

    /// Las palabras que aun no miro nadie.
    pub fn palabras_nuevas(&self) -> usize {
        self.nuevos()
            .map(|d| d.texto.split_whitespace().count())
            .sum()
    }

    /// Si hay bastante texto nuevo para preguntar.
    pub fn hay_nuevo(&self) -> bool {
        self.palabras_nuevas() >= PALABRAS_MINIMAS
    }

    /// **La ventana de ahora**: lo nuevo (de lo mas nuevo hacia atras,
    /// hasta [`LARGO_AHORA`]) y lo de antes que cabe en los ultimos
    /// [`VENTANA_MS`]. `None` sin nada nuevo.
    pub fn ventana(&self) -> Option<Ventana> {
        let ultimo = self.dichos.back()?;
        let primero_nuevo = self.mirados.max(self.base);
        if primero_nuevo >= self.base + self.dichos.len() as u64 {
            return None;
        }
        // Lo nuevo, de atras hacia delante, mientras quepa (siempre la
        // ultima frase).
        let mut desde = self.dichos.len();
        let mut largo = 0usize;
        while desde > (primero_nuevo - self.base) as usize {
            let n = self.dichos[desde - 1].texto.chars().count() + 1;
            if desde < self.dichos.len() && largo + n > LARGO_AHORA {
                break;
            }
            largo += n;
            desde -= 1;
        }
        let ahora: Vec<&str> = self.dichos.range(desde..).map(|d| d.texto.trim()).collect();
        let primera = &self.dichos[desde];
        // Lo de antes: lo que acaba dentro de la ventana, hasta lo nuevo.
        let corte = ultimo.fin_ms - VENTANA_MS;
        let antes: Vec<&Dicho> = self
            .dichos
            .range(..desde)
            .filter(|d| d.fin_ms >= corte)
            .collect();
        let ms_desde = antes.first().map_or(primera.ms, |d| d.ms);
        let antes = cola(
            &antes.iter().map(|d| d.texto.trim()).collect::<Vec<_>>(),
            LARGO_ANTES,
        );
        Some(Ventana {
            antes: Some(antes).filter(|a| !a.is_empty()),
            ahora: crate::recortar(&ahora.join(" "), LARGO_AHORA),
            ms_ahora: primera.ms,
            ms_desde,
            fin_ms: ultimo.fin_ms,
            reloj_primera: primera.reloj_fin,
            reloj_ultima: ultimo.reloj_fin,
            hasta: self.base + self.dichos.len() as u64,
        })
    }

    /// **Jev ya miro esta ventana**: lo suyo deja de ser nuevo, y lo que
    /// ya no cabe en la ventana se tira.
    pub fn mirado(&mut self, v: &Ventana) {
        self.mirados = self.mirados.max(v.hasta);
        let Some(fin) = self.dichos.back().map(|d| d.fin_ms) else {
            return;
        };
        while let Some(d) = self.dichos.front() {
            if self.base < self.mirados && d.fin_ms < fin - VENTANA_MS {
                self.dichos.pop_front();
                self.base += 1;
            } else {
                break;
            }
        }
    }
}

/// Las frases juntas, quedandose con el final si no caben (lo mas cerca de
/// lo nuevo), cortando por una palabra.
fn cola(frases: &[&str], n: usize) -> String {
    let todo = frases.join(" ");
    let total = todo.chars().count();
    if total <= n {
        return todo;
    }
    let resto: String = todo.chars().skip(total - n).collect();
    let resto = match resto.find(' ') {
        Some(i) => &resto[i + 1..],
        None => resto.as_str(),
    };
    format!("…{resto}")
}

/// Que hacer ahora con Jev.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cuando {
    /// Nada: sin texto nuevo, o con una peticion en vuelo (lo nuevo se
    /// junta para la siguiente).
    Nada,
    /// Preguntar ya.
    Ya,
    /// Preguntar dentro de tantos ms.
    Esperar(u64),
}

/// **El ritmo de las peticiones**, con un reloj de ms que da quien llama
/// (en las pruebas, uno de mentira).
#[derive(Debug, Clone, Default)]
pub struct Ritmo {
    en_vuelo: bool,
    ultima: Option<u64>,
    no_antes_de: u64,
}

impl Ritmo {
    /// Si toca preguntar a las `ahora`, con o sin texto nuevo.
    pub fn cuando(&self, ahora: u64, hay_nuevo: bool) -> Cuando {
        if self.en_vuelo || !hay_nuevo {
            return Cuando::Nada;
        }
        let listo = self
            .ultima
            .map_or(0, |u| u + HUECO_MS)
            .max(self.no_antes_de);
        if ahora >= listo {
            Cuando::Ya
        } else {
            Cuando::Esperar(listo - ahora)
        }
    }

    /// Sale una peticion.
    pub fn sale(&mut self, ahora: u64) {
        self.en_vuelo = true;
        self.ultima = Some(ahora);
    }

    /// Volvio (bien o con un fallo).
    pub fn vuelve(&mut self, ahora: u64, bien: bool) {
        self.en_vuelo = false;
        if !bien {
            self.no_antes_de = ahora + TRAS_FALLO_MS;
        }
    }
}

/// Lo que Jev dijo de un tema en una peticion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Senal {
    pub tratado: f64,
    pub cerrado: f64,
    pub ahora: f64,
}

/// Un tema que ya se trato: su `id`, el milisegundo en que empezo a
/// tratarse y la probabilidad.
#[derive(Debug, Clone, PartialEq)]
pub struct Tratado {
    pub id: String,
    pub ms: i64,
    pub p: f64,
}

/// **El estado de una peticion**: el titulo (si lo hay), la agenda entera
/// y la conversacion (lo de antes, si lo hay, y lo nuevo).
pub fn estado(reunion: &str, agenda: &[String], v: &Ventana) -> Value {
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
    let mut c = Map::new();
    if let Some(a) = v.antes.as_ref().filter(|a| !a.trim().is_empty()) {
        c.insert("antes".into(), json!(a));
    }
    c.insert("ahora".into(), json!(v.ahora));
    m.insert("conversacion".into(), Value::Object(c));
    Value::Object(m)
}

/// **La peticion**: el estado y tres preguntas por cada tema pendiente
/// (como mucho [`q::POR_PETICION`] temas).
pub fn peticion(
    reunion: &str,
    agenda: &[String],
    v: &Ventana,
    pendientes: &[Pendiente],
) -> Peticion {
    let temas: Vec<(&str, String)> = pendientes
        .iter()
        .take(q::POR_PETICION)
        .map(|p| (p.id.as_str(), crate::recortar(&p.texto, LARGO_TEMA)))
        .collect();
    let pares: Vec<(&str, &str)> = temas.iter().map(|(i, t)| (*i, t.as_str())).collect();
    q::preguntas(&pares)
        .into_iter()
        .fold(Peticion::nueva(estado(reunion, agenda, v)), |p, (n, pr)| {
            p.con(&n, pr)
        })
}

/// **Pregunta por una ventana**: la senal de cada tema que fue en la
/// peticion (los primeros [`q::POR_PETICION`]). Un fallo es un error de la
/// peticion entera.
pub fn preguntar(
    cliente: &Cliente,
    reunion: &str,
    agenda: &[String],
    v: &Ventana,
    pendientes: &[Pendiente],
) -> Result<(Vec<(String, Senal)>, String), Error> {
    let r = cliente.preguntar(&peticion(reunion, agenda, v, pendientes))?;
    let mut s = Vec::new();
    for p in pendientes.iter().take(q::POR_PETICION) {
        s.push((
            p.id.clone(),
            Senal {
                tratado: r.noul(&q::tratado(&p.id))?,
                cerrado: r.noul(&q::cerrado(&p.id))?,
                ahora: r.noul(&q::ahora(&p.id))?,
            },
        ));
    }
    Ok((s, r.model))
}

/// Un «a medias» recordado: el fin de la ventana en que se vio, el
/// milisegundo desde el que se trata y la probabilidad.
#[derive(Debug, Clone, Copy)]
struct AMedias {
    fin_ms: i64,
    ms: i64,
    p: f64,
}

/// **Lo que se recuerda entre peticiones** para la fila de «dos a
/// medias».
#[derive(Debug, Clone, Default)]
pub struct Seguimiento {
    a_medias: HashMap<String, AMedias>,
}

impl Seguimiento {
    /// **La tabla** (ver la cabecera): lo que se trato segun lo que dijo
    /// Jev de la ventana `v`. Lo devuelto se marca; lo demas espera.
    pub fn decidir(&mut self, v: &Ventana, senales: &[(String, Senal)]) -> Vec<Tratado> {
        let mut hechos = Vec::new();
        for (id, s) in senales {
            let antes = self
                .a_medias
                .remove(id)
                .filter(|a| v.fin_ms - a.fin_ms <= VIGENCIA_MS);
            let aqui = if s.ahora >= CASI {
                v.ms_ahora
            } else {
                v.ms_desde
            };
            let ms = antes.map_or(aqui, |a| a.ms);
            let marcar = |p: f64| Tratado {
                id: id.clone(),
                ms,
                p,
            };
            if s.tratado >= SI {
                hechos.push(marcar(s.tratado));
            } else if s.tratado >= CASI {
                if s.cerrado >= SI {
                    hechos.push(marcar(s.cerrado));
                } else if let Some(a) = antes.filter(|_| s.ahora >= CASI) {
                    hechos.push(marcar(a.p.max(s.tratado)));
                } else {
                    self.a_medias.insert(
                        id.clone(),
                        AMedias {
                            fin_ms: v.fin_ms,
                            ms,
                            p: antes.map_or(s.tratado, |a| a.p.max(s.tratado)),
                        },
                    );
                }
            }
        }
        hechos
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

    fn s(tratado: f64, cerrado: f64, ahora: f64) -> Senal {
        Senal {
            tratado,
            cerrado,
            ahora,
        }
    }

    fn una(id: &str, x: Senal) -> Vec<(String, Senal)> {
        vec![(id.to_string(), x)]
    }

    /// Una ventana de mentira: lo nuevo empieza en `ms_ahora`, la
    /// conversacion en `ms_ahora - 10 s`, y acaba 3 s despues.
    fn v(ms_ahora: i64) -> Ventana {
        Ventana {
            antes: None,
            ahora: "x".into(),
            ms_ahora,
            ms_desde: ms_ahora - 10_000,
            fin_ms: ms_ahora + 3_000,
            reloj_primera: 0,
            reloj_ultima: 0,
            hasta: 0,
        }
    }

    fn dicho(ms: i64, texto: &str) -> Dicho {
        let palabras = texto.split_whitespace().count() as i64;
        Dicho {
            ms,
            fin_ms: ms + 400 * palabras,
            texto: texto.into(),
            reloj_fin: (ms + 400 * palabras) as u64,
        }
    }

    #[test]
    fn la_tabla_de_lo_tratado() {
        let mut g = Seguimiento::default();
        // Claro: en el minuto de lo nuevo, si lo nuevo es del tema.
        assert_eq!(
            g.decidir(&v(45_000), &una("t1", s(0.86, 0.2, 0.9))),
            vec![Tratado {
                id: "t1".into(),
                ms: 45_000,
                p: 0.86
            }]
        );
        // Si lo nuevo es de otra cosa, en el principio de la conversacion.
        assert_eq!(
            g.decidir(&v(45_000), &una("t1", s(0.8, 0.2, 0.1)))[0].ms,
            35_000
        );
        // El umbral es inclusivo.
        assert_eq!(g.decidir(&v(0), &una("t1", s(0.7, 0.0, 0.0))).len(), 1);
        // A medias pero cerrado: se trato.
        let h = g.decidir(&v(60_000), &una("t2", s(0.55, 0.9, 0.6)));
        assert_eq!((h[0].ms, h[0].p), (60_000, 0.9));
        // Cerrado sin tratar: no (el cierre seria de otra cosa).
        assert!(g.decidir(&v(0), &una("t3", s(0.2, 0.95, 0.4))).is_empty());
        // Solo nombrado o dejado para luego: no.
        assert!(g.decidir(&v(0), &una("t3", s(0.1, 0.05, 0.1))).is_empty());
    }

    #[test]
    fn dos_a_medias_cuentan_solo_si_la_segunda_trae_algo_nuevo() {
        let mut g = Seguimiento::default();
        assert!(
            g.decidir(&v(150_000), &una("t1", s(0.6, 0.3, 0.7)))
                .is_empty()
        );
        // La misma frase leida otra vez (lo nuevo es de otra cosa): no.
        assert!(
            g.decidir(&v(153_000), &una("t1", s(0.62, 0.3, 0.1)))
                .is_empty()
        );
        // Algo nuevo del tema: si, en el minuto de la primera.
        assert_eq!(
            g.decidir(&v(156_000), &una("t1", s(0.65, 0.4, 0.6))),
            vec![Tratado {
                id: "t1".into(),
                ms: 150_000,
                p: 0.65
            }]
        );
        // Un «no» entre medias rompe la racha.
        assert!(g.decidir(&v(0), &una("t3", s(0.6, 0.0, 0.6))).is_empty());
        assert!(
            g.decidir(&v(3_000), &una("t3", s(0.2, 0.0, 0.0)))
                .is_empty()
        );
        assert!(
            g.decidir(&v(6_000), &una("t3", s(0.6, 0.0, 0.6)))
                .is_empty()
        );
        // Lo recordado caduca.
        assert!(g.decidir(&v(0), &una("t5", s(0.6, 0.0, 0.6))).is_empty());
        assert!(
            g.decidir(&v(VIGENCIA_MS + 1), &una("t5", s(0.6, 0.0, 0.6)))
                .is_empty()
        );
        // Olvidado (se marco a mano): no cuenta.
        assert!(g.decidir(&v(0), &una("t4", s(0.6, 0.0, 0.6))).is_empty());
        g.olvidar("t4");
        assert!(
            g.decidir(&v(3_000), &una("t4", s(0.6, 0.0, 0.6)))
                .is_empty()
        );
        // Recordado y luego claro: el minuto de cuando empezo.
        assert!(
            g.decidir(&v(10_000), &una("t6", s(0.6, 0.0, 0.6)))
                .is_empty()
        );
        assert_eq!(
            g.decidir(&v(14_000), &una("t6", s(0.9, 0.0, 0.9)))[0].ms,
            10_000
        );
    }

    #[test]
    fn el_rollo_ensena_lo_nuevo_y_los_ultimos_cuarenta_segundos() {
        let mut r = Rollo::default();
        assert!(r.ventana().is_none());
        r.meter(dicho(0, "buenos días a todos"));
        r.meter(dicho(2_000, ""));
        r.meter(dicho(3_000, "empezamos con la cotización del viernes"));
        assert!(r.hay_nuevo());
        let w = r.ventana().unwrap();
        assert_eq!(w.antes, None);
        assert_eq!(
            w.ahora,
            "buenos días a todos empezamos con la cotización del viernes"
        );
        assert_eq!((w.ms_ahora, w.ms_desde), (0, 0));
        r.mirado(&w);
        assert!(!r.hay_nuevo());
        assert!(r.ventana().is_none());
        // Lo que llega despues: nuevo, con lo de antes de contexto.
        r.meter(dicho(6_000, "eh"));
        assert!(!r.hay_nuevo(), "una palabra sola espera");
        r.meter(dicho(7_000, "el proveedor sube un tres por ciento"));
        let w = r.ventana().unwrap();
        assert_eq!(w.ahora, "eh el proveedor sube un tres por ciento");
        assert_eq!(w.ms_ahora, 6_000);
        assert_eq!(w.ms_desde, 0);
        assert_eq!(
            w.antes.as_deref(),
            Some("buenos días a todos empezamos con la cotización del viernes")
        );
        assert_eq!(w.reloj_primera, 6_400);
        assert_eq!(w.reloj_ultima, 9_800);
        // Lo que llega con la peticion en vuelo no se da por mirado.
        r.meter(dicho(10_000, "y lo aceptamos"));
        r.mirado(&w);
        assert_eq!(r.ventana().unwrap().ahora, "y lo aceptamos");
        // Lo de hace mas de 40 s ya no va.
        r.mirado(&r.ventana().unwrap());
        r.meter(dicho(60_000, "pasamos al paro de transportistas"));
        let w = r.ventana().unwrap();
        assert_eq!(w.antes, None);
        assert_eq!(w.ms_desde, 60_000);
        r.mirado(&w);
        assert_eq!(r.dichos.len(), 1, "lo viejo y mirado se tira");
    }

    #[test]
    fn el_rollo_recorta_por_delante() {
        let mut r = Rollo::default();
        let largo = "palabra ".repeat(200);
        for i in 0..5 {
            r.meter(dicho(i * 1_000, &largo));
        }
        let w = r.ventana().unwrap();
        // Lo nuevo no cabe entero: va lo mas reciente, y lo demas de antes.
        assert!(w.ahora.chars().count() <= LARGO_AHORA + 1);
        assert_eq!(w.ms_ahora, 4_000);
        let antes = w.antes.unwrap();
        assert!(antes.starts_with('…'));
        assert!(antes.chars().count() <= LARGO_ANTES + 1);
    }

    #[test]
    fn el_ritmo_junta_lo_que_llega_y_espera_entre_peticiones() {
        let mut t = Ritmo::default();
        // Sin texto nuevo, nada.
        assert_eq!(t.cuando(0, false), Cuando::Nada);
        assert_eq!(t.cuando(0, true), Cuando::Ya);
        t.sale(0);
        // En vuelo: lo nuevo espera (se junta).
        assert_eq!(t.cuando(400, true), Cuando::Nada);
        t.vuelve(600, true);
        // Volvio pronto: hasta 1,5 s despues de la anterior.
        assert_eq!(t.cuando(600, true), Cuando::Esperar(900));
        assert_eq!(t.cuando(1_500, true), Cuando::Ya);
        t.sale(1_500);
        // Volvio tarde: ya mismo.
        t.vuelve(4_000, true);
        assert_eq!(t.cuando(4_000, true), Cuando::Ya);
        t.sale(4_000);
        // Un fallo: 5 s.
        t.vuelve(4_500, false);
        assert_eq!(t.cuando(4_500, true), Cuando::Esperar(5_000));
        assert_eq!(t.cuando(9_500, true), Cuando::Ya);
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
        let mut w = v(0);
        w.ahora = "bueno, el presupuesto…".into();
        let e = estado("Comité", &agenda, &w);
        assert_eq!(e["reunion"], "Comité");
        assert_eq!(e["agenda"].as_array().unwrap().len(), 3);
        assert_eq!(e["conversacion"]["ahora"], "bueno, el presupuesto…");
        // Sin nada de antes el campo no va (vacio se leeria como senal).
        assert!(e["conversacion"].get("antes").is_none());
        w.antes = Some("antes".into());
        let e = estado("", &agenda, &w);
        assert!(e.get("reunion").is_none());
        assert_eq!(e["conversacion"]["antes"], "antes");
        let p = peticion("Comité", &agenda, &w, &pendientes());
        assert_eq!(p.questions.len(), 6);
        assert!(p.questions.contains_key("ahora_t3"));
        // Como mucho POR_PETICION temas.
        let muchos: Vec<Pendiente> = (0..30)
            .map(|i| Pendiente {
                id: format!("t{i}"),
                texto: format!("tema {i}"),
            })
            .collect();
        assert_eq!(
            peticion("", &[], &w, &muchos).questions.len(),
            3 * q::POR_PETICION
        );
    }

    #[test]
    fn pregunta_con_respuestas_grabadas() {
        let t = Arc::new(Grabado::nuevo(vec![Grabado::ok(&grabadas::nouls(&[
            ("tratado_t1", 0.91),
            ("cerrado_t1", 0.8),
            ("ahora_t1", 0.9),
            ("tratado_t3", 0.12),
            ("cerrado_t3", 0.05),
            ("ahora_t3", 0.2),
        ]))]));
        let c = Cliente::con_transporte("k", t.clone()).con_cache_propio();
        let mut r = Rollo::default();
        r.meter(dicho(
            12_000,
            "El presupuesto queda en cuarenta mil, lo aprobamos. Del viaje hablamos luego.",
        ));
        let w = r.ventana().unwrap();
        let (senales, modelo) = preguntar(&c, "Comité", &[], &w, &pendientes()).unwrap();
        assert_eq!(t.llamadas(), 1, "una sola peticion por ventana");
        assert_eq!(modelo, "jev-1.14.0");
        // Lo que salio: la conversacion con su «ahora».
        let cuerpo: Value = serde_json::from_slice(&t.pedidos.lock().unwrap()[0].cuerpo).unwrap();
        assert!(cuerpo.to_string().contains("cuarenta mil"), "{cuerpo}");
        let mut g = Seguimiento::default();
        let hechos = g.decidir(&w, &senales);
        assert_eq!(hechos.len(), 1);
        assert_eq!((hechos[0].id.as_str(), hechos[0].ms), ("t1", 12_000));
    }

    #[test]
    fn un_fallo_o_una_respuesta_que_falta_es_un_error() {
        let w = v(0);
        let t = Arc::new(Grabado::nuevo(vec![Err("sin red".into())]));
        let c = Cliente::con_transporte("k", t).con_cache_propio();
        assert!(matches!(
            preguntar(&c, "", &[], &w, &pendientes()),
            Err(Error::Red(_))
        ));
        let t = Arc::new(Grabado::nuevo(vec![Grabado::ok(&grabadas::nouls(&[
            ("tratado_t1", 0.9),
            ("cerrado_t1", 0.9),
        ]))]));
        let c = Cliente::con_transporte("k", t).con_cache_propio();
        assert!(matches!(
            preguntar(&c, "", &[], &w, &pendientes()),
            Err(Error::FaltaRespuesta(_))
        ));
        // Sin clave no se llama a nadie.
        let c = Cliente::con_transporte("", Arc::new(Grabado::default()));
        assert_eq!(
            preguntar(&c, "", &[], &w, &pendientes()),
            Err(Error::SinClave)
        );
    }

    /// **La de verdad**, solo con `TYPESAFE_API_KEY`: una reunion de
    /// mentira (escrita para la prueba, sin datos de nadie) con tres temas
    /// tratados en otro orden que el de la agenda y uno que se deja para
    /// luego, frase a frase como llegaria en vivo. Dice cuanto tarda cada
    /// peticion. `cargo test -p pixpin-typesafe -- --ignored --nocapture`.
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
        let agenda: Vec<String> = [
            "Cotización del viernes",
            "Paro de transportistas",
            "Notas de los estudiantes",
            "Compra de proyectores",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let mut pend: Vec<Pendiente> = agenda
            .iter()
            .enumerate()
            .map(|(i, t)| Pendiente {
                id: format!("t{}", i + 1),
                texto: t.clone(),
            })
            .collect();
        let frases = [
            "Bueno, empezamos, gracias por venir.",
            "Lo de los proyectores lo vemos al final si da tiempo.",
            "Primero las notas de los estudiantes del segundo curso.",
            "Han subido bastante, la media está en catorce y solo suspenden tres.",
            "Vale, entonces las publicamos el lunes en la plataforma.",
            "Ahora, el paro de transportistas de la semana que viene.",
            "Si paran el martes no llega el material, así que adelantamos el pedido al lunes.",
            "De acuerdo, Marta llama al proveedor hoy.",
            "Y la cotización del viernes, la que mandó la imprenta.",
            "Pide dos mil cuatrocientos por los mil folletos, me parece caro.",
            "Le pedimos que la rebaje a dos mil y si no, buscamos otra.",
        ];
        let mut r = Rollo::default();
        let mut g = Seguimiento::default();
        let mut marcados: Vec<(String, usize)> = Vec::new();
        for (i, f) in frases.iter().enumerate() {
            r.meter(dicho(i as i64 * 4_000, f));
            let Some(w) = r.ventana().filter(|_| r.hay_nuevo()) else {
                continue;
            };
            let empezo = std::time::Instant::now();
            let (senales, _) =
                preguntar(&c, "Reunión de coordinación", &agenda, &w, &pend).expect("contesta");
            eprintln!(
                "frase {i}: {} ms; {:?}",
                empezo.elapsed().as_millis(),
                senales
            );
            r.mirado(&w);
            for t in g.decidir(&w, &senales) {
                pend.retain(|p| p.id != t.id);
                marcados.push((t.id, i));
            }
        }
        eprintln!("marcados (tema, frase): {marcados:?}");
        let ids: Vec<&str> = marcados.iter().map(|(t, _)| t.as_str()).collect();
        assert!(
            !ids.contains(&"t4"),
            "lo dejado para luego no: {marcados:?}"
        );
        for t in ["t1", "t2", "t3"] {
            assert!(ids.contains(&t), "{t} sin marcar: {marcados:?}");
        }
    }
}
