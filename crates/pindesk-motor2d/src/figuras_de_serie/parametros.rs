//! **Los parametros de una plantilla, con botones** (3-nov-2026). El
//! usuario, en la ventana de las figuras del chat: «la opcion de poner
//! PARAMETROS ya no esta: numero de elementos, columnas, filas, dias,
//! actividades y cosas asi, que son utiles».
//!
//! Lo que se le pide a una plantilla que se alarga (`intervalo.rs`) dicho
//! como unos valores que se suben y se bajan con − y + (y se escriben): del
//! cuanto al cuanto, cuantos por hoja, las semanas del cronograma y el mes
//! del mensual y de los habitos. Aqui se decide que valores lleva cada
//! plantilla, entre que topes, como se corrigen unos a otros (el «del» nunca
//! pasa al «al») y que [`Intervalo`] sale de ellos para [`alargar`]. Puro:
//! la ventana solo pinta y llama.
//!
//! Los valores que no se tocaron no se piden: con todo como viene, la
//! plantilla es la de serie ([`Parametros::intervalo`] da `None`).
//!
//! [`alargar`]: super::alargar

use super::intervalo::{Intervalo, MAX_HOJAS};
use super::plantillas::AMPLIABLES;

/// Las semanas del cronograma de serie.
const SEMANAS_DE_SERIE: (u32, u32) = (1, 12);
/// Lo mas largo que puede ser el cronograma en semanas (`alargar` no deja
/// 260 o mas).
const MAX_SEMANAS: u32 = 259;
/// Los años que entiende `alargar` para un mes.
const ANOS: (u32, u32) = (1900, 2200);

/// **Un valor de los que se ajustan.**
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Parametro {
    /// El primero de la serie («del 1»).
    Desde,
    /// El ultimo («al 40»).
    Hasta,
    /// Cuantos por hoja («de 10 en 10»); 0, todo en una.
    PorHoja,
    /// La primera semana del cronograma.
    SemanasDesde,
    /// La ultima.
    SemanasHasta,
    /// El mes (1-12) del mensual y de los habitos; 0, sin mes (la de serie).
    Mes,
    /// El año de ese mes.
    Ano,
    /// Cuantos meses seguidos, una hoja cada uno.
    Meses,
}

impl Parametro {
    /// Todos, en el orden en que se ensenan.
    pub const TODOS: [Parametro; 8] = [
        Parametro::Desde,
        Parametro::Hasta,
        Parametro::PorHoja,
        Parametro::SemanasDesde,
        Parametro::SemanasHasta,
        Parametro::Mes,
        Parametro::Ano,
        Parametro::Meses,
    ];

    /// Su palabra para guardarlo ([`Parametros::a_texto`]).
    pub fn palabra(self) -> &'static str {
        match self {
            Parametro::Desde => "desde",
            Parametro::Hasta => "hasta",
            Parametro::PorHoja => "por-hoja",
            Parametro::SemanasDesde => "semanas-desde",
            Parametro::SemanasHasta => "semanas-hasta",
            Parametro::Mes => "mes",
            Parametro::Ano => "ano",
            Parametro::Meses => "meses",
        }
    }

    pub fn de_palabra(s: &str) -> Option<Parametro> {
        Parametro::TODOS
            .into_iter()
            .find(|p| p.palabra() == s.trim())
    }
}

/// Si la plantilla `clave` se ajusta con parametros (si se alarga).
pub fn tiene_parametros(clave: &str) -> bool {
    AMPLIABLES.iter().any(|a| a.clave == clave)
}

/// **Los valores de una plantilla.** Se crean con los de serie
/// ([`Parametros::de_serie`]) y se cambian con [`Parametros::poner`] o
/// [`Parametros::paso`], que los dejan siempre dentro de sus topes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parametros {
    clave: String,
    desde: u32,
    hasta: u32,
    por_hoja: u32,
    semanas: (u32, u32),
    mes: u32,
    ano: u32,
    meses: u32,
    /// El año de hoy: el de serie al elegir un mes.
    ano_de_hoy: u32,
}

impl Parametros {
    /// **Los de serie** de `clave`, o `None` si esa plantilla no se ajusta.
    /// `ano_de_hoy` es el año que sale al elegir un mes (quien llama sabe
    /// la fecha; esto es puro).
    pub fn de_serie(clave: &str, ano_de_hoy: i32) -> Option<Parametros> {
        let a = AMPLIABLES.iter().find(|a| a.clave == clave)?;
        let ano = (ano_de_hoy.max(0) as u32).clamp(ANOS.0, ANOS.1);
        Some(Parametros {
            clave: clave.to_string(),
            desde: a.defecto.0,
            hasta: a.defecto.1,
            por_hoja: 0,
            semanas: SEMANAS_DE_SERIE,
            mes: 0,
            ano,
            meses: 1,
            ano_de_hoy: ano,
        })
    }

    pub fn clave(&self) -> &str {
        &self.clave
    }

    /// Si va por meses: el mensual siempre que tenga mes, y los habitos
    /// cuando se les eligio uno (entonces los dias son los del mes).
    pub fn por_meses(&self) -> bool {
        self.mes > 0 && matches!(self.clave.as_str(), "planificador-mensual" | "habitos")
    }

    /// **Los que se ensenan ahora**, en su orden. El mes trae su año y sus
    /// meses solo cuando esta puesto; con mes, los habitos dejan el
    /// intervalo de dias (lo da el mes).
    pub fn visibles(&self) -> Vec<Parametro> {
        use Parametro::*;
        let mut v = Vec::new();
        let con_mes = matches!(self.clave.as_str(), "planificador-mensual" | "habitos");
        if self.clave != "planificador-mensual" && !self.por_meses() {
            v.extend([Desde, Hasta, PorHoja]);
        }
        if self.clave == "cronograma" {
            v.extend([SemanasDesde, SemanasHasta]);
        }
        if con_mes {
            v.push(Mes);
            if self.mes > 0 {
                v.extend([Ano, Meses]);
            }
        }
        v
    }

    /// El valor de `p`.
    pub fn valor(&self, p: Parametro) -> u32 {
        match p {
            Parametro::Desde => self.desde,
            Parametro::Hasta => self.hasta,
            Parametro::PorHoja => self.por_hoja,
            Parametro::SemanasDesde => self.semanas.0,
            Parametro::SemanasHasta => self.semanas.1,
            Parametro::Mes => self.mes,
            Parametro::Ano => self.ano,
            Parametro::Meses => self.meses,
        }
    }

    /// **Entre que topes va `p`** ahora (dependen de los demas: cuantos por
    /// hoja, de cuantos hay; y que no salgan mas de [`MAX_HOJAS`] hojas).
    pub fn rango(&self, p: Parametro) -> (u32, u32) {
        let a = AMPLIABLES
            .iter()
            .find(|a| a.clave == self.clave)
            .expect("solo se crean de la tabla");
        match p {
            Parametro::Desde | Parametro::Hasta => (a.minimo, a.maximo),
            Parametro::PorHoja => {
                // Con 0, todo en una hoja; si no, de 1 a uno menos que todos
                // (con todos en una hoja ya esta el 0), y que no pasen de
                // `MAX_HOJAS` hojas.
                let n = self.hasta - self.desde + 1;
                (0, n.saturating_sub(1))
            }
            Parametro::SemanasDesde => (1, 9999 - MAX_SEMANAS),
            Parametro::SemanasHasta => (1, 9999),
            Parametro::Mes => (0, 12),
            Parametro::Ano => ANOS,
            Parametro::Meses => (1, MAX_HOJAS as u32),
        }
    }

    /// Lo menos que puede ir por hoja sin pasar de [`MAX_HOJAS`] hojas.
    fn por_hoja_minimo(&self) -> u32 {
        let n = self.hasta - self.desde + 1;
        n.div_ceil(MAX_HOJAS as u32).max(1)
    }

    /// **Pone `p` a `v`**, acotado, y corrige los demas para que sigan
    /// teniendo sentido: el «del» que pasa al «al» se lo lleva, y al reves;
    /// cuantos por hoja no deja mas de [`MAX_HOJAS`] hojas.
    pub fn poner(&mut self, p: Parametro, v: u32) {
        let (lo, hi) = self.rango(p);
        let v = v.clamp(lo, hi);
        match p {
            Parametro::Desde => {
                self.desde = v;
                self.hasta = self.hasta.max(v);
            }
            Parametro::Hasta => {
                self.hasta = v;
                self.desde = self.desde.min(v);
            }
            Parametro::PorHoja => self.por_hoja = v,
            Parametro::SemanasDesde => {
                self.semanas.0 = v;
                self.semanas.1 = self.semanas.1.clamp(v, v + MAX_SEMANAS);
            }
            Parametro::SemanasHasta => {
                self.semanas.1 = v;
                self.semanas.0 = self
                    .semanas
                    .0
                    .clamp(v.saturating_sub(MAX_SEMANAS).max(1), v);
            }
            Parametro::Mes => self.mes = v,
            Parametro::Ano => self.ano = v,
            Parametro::Meses => self.meses = v,
        }
        self.corregir_por_hoja();
    }

    /// Cuantos por hoja, dentro de lo que dejan los demas.
    fn corregir_por_hoja(&mut self) {
        if self.por_hoja == 0 {
            return;
        }
        let (_, hi) = self.rango(Parametro::PorHoja);
        self.por_hoja = if hi == 0 {
            0
        } else {
            self.por_hoja.clamp(self.por_hoja_minimo().min(hi), hi)
        };
    }

    /// **Sube o baja `p`** `pasos` (el − y el +, la rueda encima). Cuantos
    /// por hoja salta del 0 («todo en una») a lo menos que se puede, y de
    /// ahi abajo vuelve al 0; el mes baja hasta el «sin mes» (0).
    pub fn paso(&mut self, p: Parametro, pasos: i32) {
        if pasos == 0 {
            return;
        }
        let v = self.valor(p) as i64;
        let (lo, hi) = self.rango(p);
        let nuevo = match p {
            Parametro::PorHoja => {
                let minimo = self.por_hoja_minimo() as i64;
                if v == 0 && pasos > 0 {
                    minimo.max(1) + (pasos as i64 - 1)
                } else if v + (pasos as i64) < minimo {
                    0
                } else {
                    v + pasos as i64
                }
            }
            _ => v + pasos as i64,
        };
        self.poner(p, nuevo.clamp(lo as i64, hi as i64) as u32);
    }

    /// Si esta todo como en la de serie.
    pub fn de_fabrica(&self) -> bool {
        Parametros::de_serie(&self.clave, self.ano_de_hoy as i32).is_some_and(|s| {
            s.desde == self.desde
                && s.hasta == self.hasta
                && s.por_hoja == self.por_hoja
                && s.semanas == self.semanas
                && s.mes == self.mes
                && (self.mes == 0 || (s.ano == self.ano && s.meses == self.meses))
        })
    }

    /// Vuelve a los de serie.
    pub fn restablecer(&mut self) {
        if let Some(s) = Parametros::de_serie(&self.clave, self.ano_de_hoy as i32) {
            *self = s;
        }
    }

    /// **Lo que se le pide a [`alargar`](super::alargar)**, o `None` si
    /// esta todo como viene (la de serie, tal cual).
    pub fn intervalo(&self) -> Option<Intervalo> {
        if self.de_fabrica() {
            return None;
        }
        if self.por_meses() {
            return Some(Intervalo {
                mes: Some((self.ano as i32, self.mes)),
                meses: Some(self.meses),
                ..Intervalo::default()
            });
        }
        let semanas = (self.clave == "cronograma" && self.semanas != SEMANAS_DE_SERIE)
            .then_some(self.semanas);
        Some(Intervalo {
            desde: Some(self.desde),
            hasta: Some(self.hasta),
            de_a: (self.por_hoja > 0).then_some(self.por_hoja),
            semanas,
            ..Intervalo::default()
        })
    }

    /// **Cuantas hojas salen**, sin hacerlas.
    pub fn hojas(&self) -> usize {
        if self.por_meses() {
            return self.meses as usize;
        }
        let n = self.hasta - self.desde + 1;
        if self.por_hoja == 0 || self.por_hoja >= n {
            1
        } else {
            n.div_ceil(self.por_hoja) as usize
        }
    }

    /// **Para recordarlos**: solo lo que no es de serie, «desde=1;hasta=40».
    pub fn a_texto(&self) -> String {
        let serie = Parametros::de_serie(&self.clave, self.ano_de_hoy as i32);
        Parametro::TODOS
            .into_iter()
            .filter(|p| serie.as_ref().is_none_or(|s| s.valor(*p) != self.valor(*p)))
            .map(|p| format!("{}={}", p.palabra(), self.valor(p)))
            .collect::<Vec<_>>()
            .join(";")
    }

    /// **Los recordados** de `clave` ([`Parametros::a_texto`]): lo que no
    /// se entienda se queda como el de serie, y todo se acota.
    pub fn de_texto(clave: &str, texto: &str, ano_de_hoy: i32) -> Option<Parametros> {
        let mut p = Parametros::de_serie(clave, ano_de_hoy)?;
        // Primero los extremos y despues lo que depende de ellos.
        let mut pares: Vec<(Parametro, u32)> = texto
            .split(';')
            .filter_map(|par| {
                let (k, v) = par.split_once('=')?;
                Some((Parametro::de_palabra(k)?, v.trim().parse().ok()?))
            })
            .collect();
        pares.sort_by_key(|(k, _)| matches!(k, Parametro::PorHoja));
        for (k, v) in pares {
            p.poner(k, v);
        }
        Some(p)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::figuras_de_serie::alargar;

    #[test]
    fn de_serie_no_pide_nada_y_la_que_no_se_alarga_no_tiene() {
        let p = Parametros::de_serie("lista-de-tareas", 2026).unwrap();
        assert!(p.de_fabrica());
        assert_eq!(p.intervalo(), None);
        assert_eq!(p.hojas(), 1);
        assert_eq!(p.a_texto(), "");
        assert_eq!(
            p.visibles(),
            [Parametro::Desde, Parametro::Hasta, Parametro::PorHoja]
        );
        // Caso negativo: un simbolo suelto no lleva parametros.
        assert!(Parametros::de_serie("diario", 2026).is_none());
        assert!(!tiene_parametros("diario"));
        assert!(tiene_parametros("cronograma"));
    }

    #[test]
    fn el_del_nunca_pasa_al_al_y_todo_queda_en_sus_topes() {
        let mut p = Parametros::de_serie("lista-de-tareas", 2026).unwrap();
        p.poner(Parametro::Hasta, 40);
        assert_eq!(
            (p.valor(Parametro::Desde), p.valor(Parametro::Hasta)),
            (1, 40)
        );
        p.poner(Parametro::Desde, 60);
        assert_eq!(p.valor(Parametro::Hasta), 60, "el «del» se lleva al «al»");
        p.poner(Parametro::Hasta, 5);
        assert_eq!(p.valor(Parametro::Desde), 5, "y al reves");
        // Caso negativo: fuera de los topes no se sale.
        p.poner(Parametro::Hasta, 100_000);
        assert_eq!(p.valor(Parametro::Hasta), 999);
        p.paso(Parametro::Desde, -50);
        assert_eq!(p.valor(Parametro::Desde), 1);
        let mut h = Parametros::de_serie("planificador-diario", 2026).unwrap();
        h.poner(Parametro::Hasta, 30);
        assert_eq!(h.valor(Parametro::Hasta), 24, "un dia tiene 24 horas");
    }

    #[test]
    fn por_hoja_salta_del_cero_y_nunca_pasa_de_sesenta_hojas() {
        let mut p = Parametros::de_serie("lista-numerada", 2026).unwrap();
        p.poner(Parametro::Desde, 1);
        p.poner(Parametro::Hasta, 40);
        p.paso(Parametro::PorHoja, 1);
        assert_eq!(p.valor(Parametro::PorHoja), 1);
        p.poner(Parametro::PorHoja, 10);
        assert_eq!(p.hojas(), 4);
        let hojas = alargar("lista-numerada", &p.intervalo().unwrap()).unwrap();
        assert_eq!(hojas.len(), p.hojas(), "lo que dice es lo que sale");
        // Con mil numeros, de 1 en 1 serian mil hojas: lo menos son 17.
        p.poner(Parametro::Hasta, 1000);
        p.poner(Parametro::PorHoja, 1);
        assert_eq!(p.valor(Parametro::PorHoja), 17);
        assert!(p.hojas() <= MAX_HOJAS);
        assert!(alargar("lista-numerada", &p.intervalo().unwrap()).is_ok());
        // Caso negativo: bajar de lo menos vuelve a «todo en una».
        p.paso(Parametro::PorHoja, -1);
        assert_eq!(p.valor(Parametro::PorHoja), 0);
        assert_eq!(p.hojas(), 1);
    }

    #[test]
    fn las_semanas_del_cronograma_y_el_mes_de_los_habitos() {
        let mut c = Parametros::de_serie("cronograma", 2026).unwrap();
        assert!(c.visibles().contains(&Parametro::SemanasHasta));
        c.poner(Parametro::SemanasHasta, 26);
        let iv = c.intervalo().unwrap();
        assert_eq!(iv.semanas, Some((1, 26)));
        assert!(alargar("cronograma", &iv).is_ok());
        c.poner(Parametro::SemanasHasta, 5000);
        assert_eq!(c.valor(Parametro::SemanasDesde), 5000 - MAX_SEMANAS);
        assert!(alargar("cronograma", &c.intervalo().unwrap()).is_ok());

        let mut h = Parametros::de_serie("habitos", 2026).unwrap();
        assert!(!h.visibles().contains(&Parametro::Ano), "sin mes, sin año");
        h.paso(Parametro::Mes, 1);
        assert!(h.por_meses());
        assert_eq!(
            h.visibles(),
            [Parametro::Mes, Parametro::Ano, Parametro::Meses]
        );
        h.poner(Parametro::Mes, 2);
        h.poner(Parametro::Meses, 3);
        let iv = h.intervalo().unwrap();
        assert_eq!((iv.mes, iv.meses), (Some((2026, 2)), Some(3)));
        assert_eq!(alargar("habitos", &iv).unwrap().len(), 3);
        // Caso negativo: el mensual sin mes es el de serie.
        let m = Parametros::de_serie("planificador-mensual", 2026).unwrap();
        assert_eq!(m.visibles(), [Parametro::Mes]);
        assert_eq!(m.intervalo(), None);
    }

    #[test]
    fn se_recuerdan_y_lo_roto_se_queda_de_serie() {
        let mut p = Parametros::de_serie("metas", 2026).unwrap();
        p.poner(Parametro::Hasta, 30);
        p.poner(Parametro::PorHoja, 10);
        let t = p.a_texto();
        assert_eq!(t, "hasta=30;por-hoja=10");
        assert_eq!(Parametros::de_texto("metas", &t, 2026), Some(p));
        // Caso negativo: basura y valores imposibles no rompen nada.
        let q = Parametros::de_texto("metas", "hasta=xx;nada=3;desde=0", 2026).unwrap();
        assert_eq!(q.valor(Parametro::Desde), 1);
        assert_eq!(q.valor(Parametro::Hasta), 12);
        assert!(Parametros::de_texto("diario", "hasta=3", 2026).is_none());
    }
}
