//! **Cuando volver a intentarlo con un aparato que fallo** (27-oct-2026).
//!
//! Puerto de `Reintentos` de `SincronizacionAutomatica.kt` de Android, puro
//! para probarlo sin red. Antes, una vuelta automatica que fallaba —o que
//! encontraba al otro ocupado— no se repetia hasta el siguiente periodo: diez
//! minutos. Y si los dos aparatos se llamaban a la vez, cada uno encontraba al
//! otro ocupado y los dos esperaban esos diez minutos.
//!
//! Ahora: **ocupado** se reintenta enseguida, entre [OCUPADO_MIN] y
//! [OCUPADO_MIN] + [OCUPADO_AZAR] (con azar, para que dos que chocaron no
//! vuelvan a chocar al mismo compas); **un fallo** de verdad, con espera que
//! se dobla —[BASE], el doble, el doble…— hasta [TOPE], con ±25 % de azar.
//! Salir bien lo borra todo.

use std::collections::HashMap;

pub const BASE: i64 = 5_000;
pub const TOPE: i64 = 5 * 60_000;
pub const OCUPADO_MIN: i64 = 1_000;
pub const OCUPADO_AZAR: i64 = 3_000;

/// Un numero al azar en `[0, 1)`, del sistema.
pub fn azar_del_sistema() -> f64 {
    use rand_core::RngCore;
    (rand_core::OsRng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

/// Lo que se recuerda de los fallos de cada aparato (o persona).
#[derive(Debug, Clone)]
pub struct Reintentos {
    fallos: HashMap<String, u32>,
    cuando: HashMap<String, i64>,
    azar: fn() -> f64,
}

impl Default for Reintentos {
    fn default() -> Self {
        Reintentos::con_azar(azar_del_sistema)
    }
}

impl Reintentos {
    /// Con el azar que se diga (las pruebas lo fijan).
    pub fn con_azar(azar: fn() -> f64) -> Reintentos {
        Reintentos {
            fallos: HashMap::new(),
            cuando: HashMap::new(),
            azar,
        }
    }

    /// Fallo con `otro` `ahora` (`ocupado`: el otro estaba en otra vuelta).
    /// Devuelve cuanto esperar antes de volver a intentarlo.
    pub fn fallo(&mut self, otro: &str, ahora: i64, ocupado: bool) -> i64 {
        let azar = (self.azar)();
        let espera = if ocupado {
            OCUPADO_MIN + (azar * OCUPADO_AZAR as f64) as i64
        } else {
            let n = self.fallos.get(otro).copied().unwrap_or(0) + 1;
            self.fallos.insert(otro.to_string(), n);
            let sin_azar = (BASE << (n - 1).min(16)).min(TOPE);
            (sin_azar as f64 * (0.75 + azar * 0.5)) as i64
        };
        self.cuando.insert(otro.to_string(), ahora + espera);
        espera
    }

    /// Salio bien con `otro`: se olvida todo.
    pub fn bien(&mut self, otro: &str) {
        self.fallos.remove(otro);
        self.cuando.remove(otro);
    }

    /// Los de `presentes` a los que ya toca volver a llamar.
    pub fn tocan(&self, ahora: i64, presentes: &[String]) -> Vec<String> {
        presentes
            .iter()
            .filter(|p| self.cuando.get(*p).is_some_and(|t| ahora >= *t))
            .cloned()
            .collect()
    }

    /// Cuando toca el siguiente reintento de alguno, si hay alguno.
    pub fn siguiente(&self) -> Option<i64> {
        self.cuando.values().min().copied()
    }

    /// Fallos seguidos con `otro` (no cuenta «ocupado»).
    pub fn seguidos(&self, otro: &str) -> u32 {
        self.fallos.get(otro).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn ocupado_enseguida_y_con_azar_un_fallo_cada_vez_mas_tarde_hasta_el_tope() {
        let mut r = Reintentos::con_azar(|| 0.5);
        let ocupado = r.fallo("pc", 0, true);
        assert!((OCUPADO_MIN..=OCUPADO_MIN + OCUPADO_AZAR).contains(&ocupado));
        assert_eq!(r.seguidos("pc"), 0, "ocupado no cuenta como fallo");
        let esperas: Vec<i64> = (0..10).map(|_| r.fallo("pc", 0, false)).collect();
        assert_eq!(esperas[0], BASE);
        assert_eq!(esperas[1], 2 * BASE);
        assert_eq!(*esperas.last().unwrap(), TOPE);
        let presentes = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(r.tocan(TOPE - 1, &presentes(&["pc"])).is_empty());
        assert_eq!(r.tocan(TOPE, &presentes(&["pc", "tableta"])), ["pc"]);
        // Dos que chocan no vuelven a chocar al mismo compas.
        let a = Reintentos::con_azar(|| 0.0).fallo("x", 0, true);
        let b = Reintentos::con_azar(|| 0.99).fallo("x", 0, true);
        assert!(b - a > 2_000);
        r.bien("pc");
        assert!(r.tocan(i64::MAX, &presentes(&["pc"])).is_empty());
        assert_eq!(r.siguiente(), None);
    }

    #[test]
    fn el_azar_del_sistema_esta_entre_cero_y_uno() {
        for _ in 0..100 {
            let x = azar_del_sistema();
            assert!((0.0..1.0).contains(&x));
        }
    }
}
