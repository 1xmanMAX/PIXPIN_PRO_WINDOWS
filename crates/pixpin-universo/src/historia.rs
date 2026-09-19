//! La pila de deshacer del universo (D234).
//!
//! Se guarda cada astro tocado ANTES y DESPUES, entero. Es D24 del motor: la
//! operacion inversa en coma flotante deforma con los ciclos; el estado no.

use crate::astro::{Astro, Conexion, IdAstro};

pub const TOPE: usize = 200;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Paso {
    pub astros: Vec<(IdAstro, Option<Astro>, Option<Astro>)>,
    pub conexiones: Vec<(u64, Option<Conexion>, Option<Conexion>)>,
    /// Este paso tambien deshace uno de la escena.
    pub anotaciones: bool,
}

impl Paso {
    pub fn vacio(&self) -> bool {
        self.astros.is_empty() && self.conexiones.is_empty() && !self.anotaciones
    }
}

#[derive(Debug, Clone, Default)]
pub struct Historia {
    pub(crate) atras: Vec<Paso>,
    pub(crate) adelante: Vec<Paso>,
    pub(crate) en_curso: Option<Paso>,
}

impl Historia {
    pub fn empujar(&mut self, p: Paso) {
        self.atras.push(p);
        if self.atras.len() > TOPE {
            self.atras.remove(0);
        }
        self.adelante.clear();
    }
}
