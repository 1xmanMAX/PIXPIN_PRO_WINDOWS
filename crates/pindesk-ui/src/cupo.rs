//! **Un cupo de memoria por bytes, soltando lo que hace mas que no se usa**
//! (LRU). Lo usan las vistas previas del chat: sin tope, recorrer un chat
//! de mil fotos dejaba mil fotos en memoria (y en la GPU).
//!
//! Solo lleva las cuentas: que hay, cuanto pesa y cuando se uso. Soltar de
//! verdad lo hace quien lo guarda, con las llaves que esto le devuelve.
//!
//! Lo usado en el fotograma en curso NUNCA se suelta, aunque se pase del
//! tope: soltar lo que se esta viendo para volver a leerlo en el siguiente
//! seria parpadear sin fin. El tope se cumple en cuanto deje de verse.

use std::collections::HashMap;
use std::hash::Hash;

/// Las cuentas del cupo.
#[derive(Debug, Clone)]
pub struct Cupo<K> {
    tope: usize,
    usado: usize,
    /// Lo que pesa cada cosa y cuando se uso por ultima vez.
    cosas: HashMap<K, (usize, u64)>,
    reloj: u64,
    /// Desde que vuelta del reloj empieza el fotograma en curso.
    fotograma: u64,
}

impl<K: Eq + Hash + Clone> Cupo<K> {
    /// Un cupo de `tope` bytes.
    pub fn nuevo(tope: usize) -> Self {
        Self {
            tope,
            usado: 0,
            cosas: HashMap::new(),
            reloj: 0,
            fotograma: 0,
        }
    }

    /// Los bytes que se llevan ahora.
    pub fn usado(&self) -> usize {
        self.usado
    }

    /// Cuantas cosas se llevan.
    pub fn cuantas(&self) -> usize {
        self.cosas.len()
    }

    /// Empieza otro fotograma: lo usado hasta ahora ya se puede soltar.
    pub fn nuevo_fotograma(&mut self) {
        self.reloj += 1;
        self.fotograma = self.reloj;
    }

    /// Se acaba de usar `llave` (se pinta o se va a pintar).
    pub fn usar(&mut self, llave: &K) {
        if let Some((_, cuando)) = self.cosas.get_mut(llave) {
            *cuando = self.reloj;
        }
    }

    /// Mete `llave` con su peso (o lo cambia, si ya estaba) y devuelve lo que
    /// hay que soltar para volver a caber, de lo mas viejo a lo mas nuevo.
    pub fn meter(&mut self, llave: K, bytes: usize) -> Vec<K> {
        if let Some((antes, _)) = self.cosas.insert(llave, (bytes, self.reloj)) {
            self.usado -= antes;
        }
        self.usado += bytes;
        self.sobrante()
    }

    /// Lo saca de las cuentas (quien lo guarda ya lo solto).
    pub fn quitar(&mut self, llave: &K) {
        if let Some((bytes, _)) = self.cosas.remove(llave) {
            self.usado -= bytes;
        }
    }

    /// Lo que sobra para volver a caber, ya fuera de las cuentas: lo mas
    /// viejo primero y nunca lo del fotograma en curso.
    pub fn sobrante(&mut self) -> Vec<K> {
        if self.usado <= self.tope {
            return Vec::new();
        }
        let mut viejas: Vec<(u64, K, usize)> = self
            .cosas
            .iter()
            .filter(|(_, (_, cuando))| *cuando < self.fotograma)
            .map(|(k, (bytes, cuando))| (*cuando, k.clone(), *bytes))
            .collect();
        viejas.sort_by_key(|(cuando, _, _)| *cuando);
        let mut fuera = Vec::new();
        for (_, k, bytes) in viejas {
            if self.usado <= self.tope {
                break;
            }
            self.cosas.remove(&k);
            self.usado -= bytes;
            fuera.push(k);
        }
        fuera
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn mientras_cabe_no_se_suelta_nada() {
        let mut c = Cupo::nuevo(100);
        assert!(c.meter("a", 40).is_empty());
        assert!(c.meter("b", 60).is_empty(), "justo en el tope cabe");
        assert_eq!(c.usado(), 100);
        assert_eq!(c.cuantas(), 2);
    }

    #[test]
    fn al_pasarse_se_suelta_lo_que_hace_mas_que_no_se_usa() {
        let mut c = Cupo::nuevo(100);
        c.meter("a", 40);
        c.nuevo_fotograma();
        c.meter("b", 40);
        c.nuevo_fotograma();
        // Se vuelve a usar la «a»: la mas vieja pasa a ser la «b».
        c.usar(&"a");
        c.nuevo_fotograma();
        assert_eq!(c.meter("c", 40), vec!["b"]);
        assert_eq!(c.usado(), 80);
    }

    #[test]
    fn lo_del_fotograma_en_curso_no_se_suelta_aunque_no_quepa() {
        // Caso negativo: soltar lo que se esta viendo seria parpadear.
        let mut c = Cupo::nuevo(100);
        c.nuevo_fotograma();
        c.meter("a", 80);
        assert!(c.meter("b", 80).is_empty());
        assert_eq!(c.usado(), 160, "se pasa del tope mientras se vea");
        // En cuanto deja de verse, se cumple el tope.
        c.nuevo_fotograma();
        c.usar(&"b");
        assert_eq!(c.sobrante(), vec!["a"]);
        assert_eq!(c.usado(), 80);
    }

    #[test]
    fn cambiar_el_peso_o_quitar_lleva_bien_la_cuenta() {
        let mut c = Cupo::nuevo(1000);
        c.meter("a", 10);
        c.meter("a", 30);
        assert_eq!(c.usado(), 30, "el peso nuevo sustituye al viejo");
        c.quitar(&"a");
        c.quitar(&"a");
        assert_eq!(c.usado(), 0);
        assert_eq!(c.cuantas(), 0);
        // Usar lo que no esta no hace nada (caso negativo).
        c.usar(&"z");
        assert_eq!(c.cuantas(), 0);
    }
}
