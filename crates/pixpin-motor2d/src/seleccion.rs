//! Que elementos estan elegidos.
//!
//! # Por que un `Vec` y no un `HashSet` (D26)
//!
//! Una seleccion de trabajo son de uno a veinte elementos. Buscar
//! linealmente en veinte es mas rapido que calcular un hash, y sobre todo:
//! el `Vec` se reutiliza con `clear()`, que conserva la memoria ya pedida.
//! Arrastrar una marquesina reconstruye la seleccion en cada aviso del
//! raton —camino caliente— y ahi la regla es cero asignaciones.
//!
//! El orden de los ids no significa nada, pero es estable a proposito: un
//! `HashSet` lo cambiaria de un recorrido a otro, y con el cambiaria el
//! orden en que se pintan los marcos.

use crate::elemento::Elemento;
use crate::escena::Escena;
use crate::vector::Punto2;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Seleccion {
    ids: Vec<u64>,
}

impl Seleccion {
    pub fn nueva() -> Self {
        Self::default()
    }

    pub fn ids(&self) -> &[u64] {
        &self.ids
    }

    pub fn esta_vacia(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn cuantos(&self) -> usize {
        self.ids.len()
    }

    pub fn contiene(&self, id: u64) -> bool {
        self.ids.contains(&id)
    }

    /// Lo que hace un clic normal: sustituye la seleccion entera.
    pub fn poner(&mut self, id: u64) {
        self.ids.clear();
        self.ids.push(id);
    }

    /// Lo que hace `Shift` + clic: lo anade si no estaba, lo quita si estaba.
    pub fn alternar(&mut self, id: u64) {
        match self.ids.iter().position(|x| *x == id) {
            Some(i) => {
                self.ids.remove(i);
            }
            None => self.ids.push(id),
        }
    }

    /// Lo que hace la marquesina o `Ctrl+A`: sustituye por todos estos.
    pub fn poner_todos(&mut self, ids: impl IntoIterator<Item = u64>) {
        self.ids.clear();
        for id in ids {
            if !self.ids.contains(&id) {
                self.ids.push(id);
            }
        }
    }

    /// Vacia la seleccion **conservando la memoria pedida** (D26).
    pub fn limpiar(&mut self) {
        self.ids.clear();
    }

    /// Para la prueba de que `limpiar` no reasigna.
    pub fn capacidad(&self) -> usize {
        self.ids.capacity()
    }

    /// Los elementos elegidos que siguen vivos.
    ///
    /// Filtra los borrados porque deshacer es borrado logico: el elemento
    /// sigue en la lista, y si contara, el marco abarcaria cosas que el
    /// usuario no ve en la pantalla.
    fn vivos<'a>(&'a self, escena: &'a Escena) -> impl Iterator<Item = &'a Elemento> + 'a {
        self.ids
            .iter()
            .filter_map(move |id| escena.buscar(*id))
            .filter(|e| !e.borrado)
    }

    /// La caja que abarca todo lo elegido, paralela a los ejes.
    ///
    /// Paralela a los ejes aunque los elementos esten girados: es lo que
    /// hacen Excalidraw y el Android. Cada elemento conserva su propio
    /// angulo; la caja es solo el marco desde el que se tira.
    pub fn caja(&self, escena: &Escena) -> Option<(f32, f32, f32, f32)> {
        let mut caja: Option<(f32, f32, f32, f32)> = None;
        for e in self.vivos(escena) {
            let (x0, y0, x1, y1) = e.caja();
            caja = Some(match caja {
                None => (x0, y0, x1, y1),
                Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
            });
        }
        caja
    }

    /// El centro de la caja: alrededor de el se gira y se escala.
    pub fn centro(&self, escena: &Escena) -> Option<Punto2> {
        let (x0, y0, x1, y1) = self.caja(escena)?;
        Some(Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};

    fn rect(x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
        Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x,
            y,
            ancho,
            alto,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
        }
    }

    #[test]
    fn poner_sustituye_y_alternar_anade_o_quita() {
        let mut s = Seleccion::nueva();
        s.poner(1);
        assert_eq!(s.ids(), &[1]);

        // Clic normal sustituye.
        s.poner(2);
        assert_eq!(s.ids(), &[2]);

        // Shift+clic anade.
        s.alternar(3);
        assert_eq!(s.ids(), &[2, 3]);

        // Shift+clic sobre lo ya elegido lo quita.
        s.alternar(2);
        assert_eq!(s.ids(), &[3]);
    }

    #[test]
    fn poner_dos_veces_el_mismo_no_lo_duplica() {
        let mut s = Seleccion::nueva();
        s.alternar(7);
        s.poner(7);
        assert_eq!(s.ids(), &[7], "una sola vez");
    }

    #[test]
    fn poner_todos_elimina_duplicados_en_la_entrada() {
        // El metodo acepta cualquier iterador y no puede fiarse de que quien
        // lo llama le de ids unicos. La rama que evita duplicados es
        // justamente la que lo hace cuadratico, y sin esta prueba no se ejecuta.
        let mut s = Seleccion::nueva();
        s.poner_todos([1, 1, 2]);
        assert_eq!(s.ids(), &[1, 2]);
    }

    #[test]
    fn la_caja_de_varios_abarca_a_todos() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 10.0, 10.0));
        let b = escena.anadir(rect(100.0, 50.0, 20.0, 20.0));

        let mut s = Seleccion::nueva();
        s.poner_todos([a, b]);

        let (x0, y0, x1, y1) = s.caja(&escena).unwrap();
        assert_eq!((x0, y0), (0.0, 0.0));
        assert_eq!((x1, y1), (120.0, 70.0));
        assert_eq!(s.centro(&escena).unwrap(), Punto2::nuevo(60.0, 35.0));
    }

    #[test]
    fn un_elemento_borrado_no_cuenta_para_la_caja() {
        // Deshacer es borrado logico: el elemento sigue en la lista. Si la
        // caja lo contara, el marco abarcaria cosas que el usuario no ve.
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 10.0, 10.0));
        let b = escena.anadir(rect(500.0, 500.0, 10.0, 10.0));
        escena.borrar(b);

        let mut s = Seleccion::nueva();
        s.poner_todos([a, b]);

        let (_, _, x1, y1) = s.caja(&escena).unwrap();
        assert_eq!((x1, y1), (10.0, 10.0), "el borrado no estira la caja");
    }

    #[test]
    fn una_seleccion_de_solo_borrados_no_tiene_caja() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 10.0, 10.0));
        escena.borrar(a);

        let mut s = Seleccion::nueva();
        s.poner(a);
        assert!(s.caja(&escena).is_none());
    }

    #[test]
    fn limpiar_conserva_la_capacidad_para_no_reasignar() {
        // D26: arrastrar una marquesina es camino caliente. El Vec se
        // reutiliza con clear(), que no devuelve la memoria al sistema.
        let mut s = Seleccion::nueva();
        s.poner_todos(1..=50);
        let capacidad = s.capacidad();
        s.limpiar();
        assert!(s.esta_vacia());
        assert_eq!(s.capacidad(), capacidad, "clear() no reasigna");
    }
}
