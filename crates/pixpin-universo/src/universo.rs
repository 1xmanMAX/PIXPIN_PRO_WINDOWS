//! El documento: todo lo que se guarda en `universo.json` (D240).

use pixpin_motor2d::Escena;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::astro::{Astro, Conexion, IdAstro};

pub const VERSION: u32 = 1;

/// Donde se quedo mirando el usuario. No es `Camara` porque esa no es serde
/// y es estado del motor; esto es del documento.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Encuadre {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl Default for Encuadre {
    fn default() -> Self {
        Self {
            x: -20_000.0,
            y: -12_000.0,
            zoom: 0.02,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Universo {
    pub version: u32,
    pub siguiente_id: u64,
    pub astros: Vec<Astro>,
    pub conexiones: Vec<Conexion>,
    /// Solo para el fichero: mientras el universo esta abierto la escena es
    /// del editor (ver el plan, «Ajustes»).
    pub anotaciones: Escena,
    pub encuadre: Encuadre,
    #[serde(flatten)]
    pub resto: Map<String, Value>,
    #[serde(skip)]
    pub(crate) historia: crate::historia::Historia,
    /// Sube con cada cambio: la rejilla y el indice de busqueda se rehacen
    /// cuando cambia, y no en cada fotograma.
    #[serde(skip)]
    pub(crate) cambios: u64,
    /// Los pasos de la escena ya apuntados en la pila (D234).
    #[serde(skip)]
    pub(crate) visto: u64,
}

impl Default for Universo {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Universo {
    pub fn nuevo() -> Self {
        Self {
            version: VERSION,
            siguiente_id: 1,
            astros: Vec::new(),
            conexiones: Vec::new(),
            anotaciones: Escena::nueva(),
            encuadre: Encuadre::default(),
            resto: Map::new(),
            historia: Default::default(),
            cambios: 0,
            visto: 0,
        }
    }

    /// Astros y conexiones comparten contador: un id no se repite nunca.
    pub fn nuevo_id(&mut self) -> IdAstro {
        let id = self.siguiente_id.max(1);
        self.siguiente_id = id + 1;
        IdAstro(id)
    }

    pub fn astro(&self, id: IdAstro) -> Option<&Astro> {
        self.astros.iter().find(|a| a.id == id)
    }

    pub fn luna_de(&self, codigo: &str) -> Option<&Astro> {
        self.astros.iter().find(|a| a.codigo() == Some(codigo))
    }

    pub fn hijos(&self, id: IdAstro) -> impl Iterator<Item = &Astro> {
        self.astros.iter().filter(move |a| a.padre == Some(id))
    }

    pub fn cambios(&self) -> u64 {
        self.cambios
    }

    /// Para quien cambia el universo desde fuera de sus operaciones (la
    /// sincronizacion con el indice).
    pub fn marcar_cambio(&mut self) {
        self.cambios += 1;
    }

    /// D208: una linea que no va a ninguna parte se quita. Devuelve cuantas.
    pub fn limpiar_conexiones(&mut self) -> usize {
        let antes = self.conexiones.len();
        let ids: std::collections::HashSet<IdAstro> = self.astros.iter().map(|a| a.id).collect();
        self.conexiones
            .retain(|c| ids.contains(&c.desde) && ids.contains(&c.hasta));
        let quitadas = antes - self.conexiones.len();
        if quitadas > 0 {
            self.cambios += 1;
        }
        quitadas
    }

    /// Abre un paso si no hay uno abierto. Devuelve si lo abrio este: solo
    /// quien lo abre lo cierra, asi una operacion puede usar otra dentro.
    pub(crate) fn empezar(&mut self) -> bool {
        if self.historia.en_curso.is_some() {
            return false;
        }
        self.historia.en_curso = Some(crate::historia::Paso::default());
        true
    }

    pub(crate) fn apuntar_astro(&mut self, id: IdAstro) {
        let actual = self.astro(id).cloned();
        if let Some(p) = &mut self.historia.en_curso
            && !p.astros.iter().any(|(i, _, _)| *i == id)
        {
            p.astros.push((id, actual, None));
        }
    }

    pub(crate) fn apuntar_conexion(&mut self, id: u64) {
        let actual = self.conexiones.iter().find(|c| c.id == id).cloned();
        if let Some(p) = &mut self.historia.en_curso
            && !p.conexiones.iter().any(|(i, _, _)| *i == id)
        {
            p.conexiones.push((id, actual, None));
        }
    }

    pub(crate) fn marcar_anotaciones(&mut self, escena: &Escena) {
        if let Some(p) = &mut self.historia.en_curso {
            p.anotaciones = true;
        }
        self.visto = escena.pasos_cerrados();
    }

    pub(crate) fn terminar(&mut self, propio: bool) {
        if !propio {
            return;
        }
        let Some(mut p) = self.historia.en_curso.take() else {
            return;
        };
        for (id, _, despues) in &mut p.astros {
            *despues = self.astros.iter().find(|a| a.id == *id).cloned();
        }
        for (id, _, despues) in &mut p.conexiones {
            *despues = self.conexiones.iter().find(|c| c.id == *id).cloned();
        }
        p.astros.retain(|(_, a, d)| a != d);
        p.conexiones.retain(|(_, a, d)| a != d);
        self.cambios += 1;
        if !p.vacio() {
            self.historia.empujar(p);
        }
    }

    /// Deshace lo hecho en el paso abierto, sin apuntarlo.
    pub(crate) fn cancelar(&mut self, propio: bool) {
        if !propio {
            return;
        }
        let Some(p) = self.historia.en_curso.take() else {
            return;
        };
        for (id, antes, _) in p.astros.iter().rev() {
            self.poner_astro(*id, antes.clone());
        }
        for (id, antes, _) in p.conexiones.iter().rev() {
            self.poner_conexion(*id, antes.clone());
        }
        self.cambios += 1;
    }

    fn poner_astro(&mut self, id: IdAstro, valor: Option<Astro>) {
        let pos = self.astros.iter().position(|a| a.id == id);
        match (pos, valor) {
            (Some(i), Some(v)) => self.astros[i] = v,
            (Some(i), None) => {
                self.astros.remove(i);
            }
            (None, Some(v)) => self.astros.push(v),
            (None, None) => {}
        }
    }

    fn poner_conexion(&mut self, id: u64, valor: Option<Conexion>) {
        let pos = self.conexiones.iter().position(|c| c.id == id);
        match (pos, valor) {
            (Some(i), Some(v)) => self.conexiones[i] = v,
            (Some(i), None) => {
                self.conexiones.remove(i);
            }
            (None, Some(v)) => self.conexiones.push(v),
            (None, None) => {}
        }
    }

    pub fn deshacer(&mut self, escena: &mut Escena) -> bool {
        let Some(p) = self.historia.atras.pop() else {
            return false;
        };
        for (id, antes, _) in &p.astros {
            self.poner_astro(*id, antes.clone());
        }
        for (id, antes, _) in &p.conexiones {
            self.poner_conexion(*id, antes.clone());
        }
        if p.anotaciones {
            escena.deshacer();
        }
        self.historia.adelante.push(p);
        self.cambios += 1;
        true
    }

    pub fn rehacer(&mut self, escena: &mut Escena) -> bool {
        let Some(p) = self.historia.adelante.pop() else {
            return false;
        };
        for (id, _, despues) in &p.astros {
            self.poner_astro(*id, despues.clone());
        }
        for (id, _, despues) in &p.conexiones {
            self.poner_conexion(*id, despues.clone());
        }
        if p.anotaciones {
            escena.rehacer();
        }
        self.historia.atras.push(p);
        self.cambios += 1;
        true
    }

    /// Apunta un paso «solo anotaciones» por cada paso que el editor cerro
    /// en la escena desde la ultima vez. Se llama tras cada evento.
    pub fn sincronizar_trazos(&mut self, escena: &Escena) {
        let ahora = escena.pasos_cerrados();
        while self.visto < ahora {
            self.historia.empujar(crate::historia::Paso {
                anotaciones: true,
                ..Default::default()
            });
            self.visto += 1;
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::astro::{Astro, Conexion, IdAstro};

    #[test]
    fn un_universo_nuevo_empieza_en_la_version_uno_y_el_id_uno() {
        let mut u = Universo::nuevo();
        assert_eq!(u.version, VERSION);
        assert_eq!(u.nuevo_id(), IdAstro(1));
        assert_eq!(u.nuevo_id(), IdAstro(2));
    }

    #[test]
    fn limpiar_quita_las_conexiones_que_apuntan_a_un_astro_que_no_existe() {
        let mut u = Universo::nuevo();
        u.astros.push(Astro::planeta(IdAstro(1), 0.0, 0.0, 250.0));
        u.astros.push(Astro::planeta(IdAstro(2), 900.0, 0.0, 250.0));
        u.conexiones
            .push(Conexion::nueva(10, IdAstro(1), IdAstro(2)));
        u.conexiones
            .push(Conexion::nueva(11, IdAstro(1), IdAstro(99)));
        assert_eq!(u.limpiar_conexiones(), 1);
        assert_eq!(u.conexiones.len(), 1);
        assert_eq!(u.conexiones[0].id, 10);
    }

    #[test]
    fn hijos_devuelve_solo_los_de_ese_padre() {
        let mut u = Universo::nuevo();
        let mut l = Astro::luna(IdAstro(2), "m:a", "p", 0.0, 0.0);
        l.padre = Some(IdAstro(1));
        u.astros.push(Astro::planeta(IdAstro(1), 0.0, 0.0, 250.0));
        u.astros.push(l);
        u.astros.push(Astro::luna(IdAstro(3), "m:b", "p", 0.0, 0.0));
        let ids: Vec<IdAstro> = u.hijos(IdAstro(1)).map(|a| a.id).collect();
        assert_eq!(ids, vec![IdAstro(2)]);
    }
}
