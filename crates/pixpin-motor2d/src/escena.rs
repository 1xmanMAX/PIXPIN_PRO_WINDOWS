//! La escena: la lista de elementos y su historia.
//!
//! El orden de la lista ES el orden de dibujo: el ultimo se pinta encima. No
//! hay campo de "profundidad" a proposito — un indice fraccionario o un
//! z-order paralelo son dos verdades sobre lo mismo, y acaban discrepando.
//!
//! Deshacer es **logico**: el elemento se marca `borrado` y sigue en la lista.
//! Rehacer es quitar la marca. Nada toca el disco hasta que se guarda, y el
//! coste de guardar un elemento borrado (unas decenas de bytes) es mucho menor
//! que el de perder un trazo.

use serde::{Deserialize, Serialize};

use crate::elemento::Elemento;
use crate::vector::Punto2;

/// Lo que se guarda en el fichero: version, contador y elementos. La
/// herramienta activa, el zoom o la seleccion NO estan aqui: son estado de la
/// interfaz, no del documento.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Escena {
    #[serde(default = "version_uno")]
    pub version: u32,
    #[serde(default = "uno_u64")]
    pub siguiente_id: u64,
    #[serde(default)]
    pub elementos: Vec<Elemento>,
    /// Lo que se ha hecho en ESTA sesion, para deshacerlo. No se guarda: el
    /// historial es estado de la interfaz, no del documento, y deshacer al
    /// abrir un dibujo de ayer un trazo que no se ve hacer confunde mas de
    /// lo que ayuda.
    #[serde(skip)]
    historia: Vec<Cambio>,
    #[serde(skip)]
    rehacer: Vec<Cambio>,
}

/// Un paso deshacible. Cada uno sabe invertirse.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Cambio {
    Anadido(u64),
    Borrado(u64),
    /// Un arrastre entero, no cada pixel: el desplazamiento total.
    Movido {
        id: u64,
        dx: f32,
        dy: f32,
    },
}

fn version_uno() -> u32 {
    1
}

fn uno_u64() -> u64 {
    1
}

impl Default for Escena {
    fn default() -> Self {
        Self {
            version: 1,
            siguiente_id: 1,
            elementos: Vec::new(),
            historia: Vec::new(),
            rehacer: Vec::new(),
        }
    }
}

impl Escena {
    pub fn nueva() -> Self {
        Self::default()
    }

    /// Anade el elemento encima de todo y le asigna su id.
    pub fn anadir(&mut self, mut e: Elemento) -> u64 {
        let id = self.siguiente_id.max(1);
        self.siguiente_id = id + 1;
        e.id = id;
        self.elementos.push(e);
        self.apuntar(Cambio::Anadido(id));
        id
    }

    /// Apunta un paso deshacible. Hacer algo nuevo corta la rama de rehacer,
    /// como en cualquier editor.
    fn apuntar(&mut self, c: Cambio) {
        self.historia.push(c);
        self.rehacer.clear();
    }

    /// Desplaza un elemento SIN apuntarlo: es cada pixel de un arrastre en
    /// curso. Lo que se deshace es el arrastre entero, y ese se apunta al
    /// soltar con `apuntar_movimiento`.
    pub fn mover(&mut self, id: u64, dx: f32, dy: f32) -> bool {
        match self.buscar_mut(id) {
            Some(e) => {
                e.mover(dx, dy);
                e.tocar();
                true
            }
            None => false,
        }
    }

    /// Cierra un arrastre: apunta el desplazamiento total como UN paso. Un
    /// arrastre que acaba donde empezo no deja rastro en el historial.
    pub fn apuntar_movimiento(&mut self, id: u64, dx: f32, dy: f32) {
        if dx != 0.0 || dy != 0.0 {
            self.apuntar(Cambio::Movido { id, dx, dy });
        }
    }

    /// Borrado a peticion del usuario (el borrador, la tecla Suprimir): se
    /// apunta para poder deshacerlo.
    pub fn borrar_apuntando(&mut self, id: u64) -> bool {
        if self.borrar(id) {
            self.apuntar(Cambio::Borrado(id));
            true
        } else {
            false
        }
    }

    pub fn buscar(&self, id: u64) -> Option<&Elemento> {
        self.elementos.iter().find(|e| e.id == id)
    }

    pub fn buscar_mut(&mut self, id: u64) -> Option<&mut Elemento> {
        self.elementos.iter_mut().find(|e| e.id == id)
    }

    /// Los que se ven, en orden de dibujo.
    pub fn visibles(&self) -> impl Iterator<Item = &Elemento> {
        self.elementos.iter().filter(|e| !e.borrado)
    }

    pub fn cuantos_visibles(&self) -> usize {
        self.visibles().count()
    }

    /// Borrado logico. `true` si habia algo que borrar.
    pub fn borrar(&mut self, id: u64) -> bool {
        match self.buscar_mut(id) {
            Some(e) if !e.borrado => {
                e.borrado = true;
                e.tocar();
                true
            }
            _ => false,
        }
    }

    pub fn restaurar(&mut self, id: u64) -> bool {
        match self.buscar_mut(id) {
            Some(e) if e.borrado => {
                e.borrado = false;
                e.tocar();
                true
            }
            _ => false,
        }
    }

    /// Deshace el ultimo paso de esta sesion: un trazo anadido, un borrado o
    /// un arrastre. Devuelve el id afectado.
    pub fn deshacer(&mut self) -> Option<u64> {
        let c = self.historia.pop()?;
        let id = self.invertir(c);
        self.rehacer.push(c);
        Some(id)
    }

    /// Rehace el ultimo paso deshecho.
    pub fn rehacer(&mut self) -> Option<u64> {
        let c = self.rehacer.pop()?;
        // Rehacer es invertir la inversion; con el paso ya invertido en la
        // pila, volver a invertirlo lo devuelve a su sitio.
        let id = match c {
            Cambio::Anadido(id) => {
                self.restaurar(id);
                id
            }
            Cambio::Borrado(id) => {
                self.borrar(id);
                id
            }
            Cambio::Movido { id, dx, dy } => {
                self.mover(id, dx, dy);
                id
            }
        };
        self.historia.push(c);
        Some(id)
    }

    fn invertir(&mut self, c: Cambio) -> u64 {
        match c {
            Cambio::Anadido(id) => {
                self.borrar(id);
                id
            }
            Cambio::Borrado(id) => {
                self.restaurar(id);
                id
            }
            Cambio::Movido { id, dx, dy } => {
                self.mover(id, -dx, -dy);
                id
            }
        }
    }

    /// Si hay algo que deshacer en esta sesion.
    pub fn hay_que_deshacer(&self) -> bool {
        !self.historia.is_empty()
    }

    /// Sube el elemento al frente sin cambiar el resto del orden.
    pub fn traer_al_frente(&mut self, id: u64) {
        if let Some(i) = self.elementos.iter().position(|e| e.id == id) {
            let e = self.elementos.remove(i);
            self.elementos.push(e);
        }
    }

    /// Saca de verdad los elementos borrados. Se llama al guardar, no al
    /// deshacer: hasta entonces, deshacer tiene que poder revertirse.
    pub fn compactar(&mut self) {
        self.elementos.retain(|e| !e.borrado);
    }

    /// La caja que abarca todo lo visible, o `None` si no hay nada.
    pub fn caja(&self) -> Option<(f32, f32, f32, f32)> {
        let mut iter = self.visibles();
        let primera = iter.next()?.caja();
        Some(iter.fold(primera, |(x0, y0, x1, y1), e| {
            let (a, b, c, d) = e.caja();
            (x0.min(a), y0.min(b), x1.max(c), y1.max(d))
        }))
    }

    /// El elemento visible de mas arriba bajo el punto.
    pub fn elemento_en(&self, p: Punto2) -> Option<u64> {
        self.elementos
            .iter()
            .rev()
            .find(|e| crate::impacto::toca(e, p))
            .map(|e| e.id)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};

    fn rect(x: f32) -> Elemento {
        Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x,
            y: 0.0,
            ancho: 50.0,
            alto: 50.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: Some(ColorRgba::opaco(1.0, 1.0, 1.0)),
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
        }
    }

    #[test]
    fn deshacer_un_arrastre_devuelve_el_elemento_a_su_sitio() {
        // Antes solo se deshacian altas y bajas: mover algo no se podia
        // deshacer, que es justo lo que se nota al poder seleccionar.
        let mut e = Escena::nueva();
        let id = e.anadir(rect(10.0));
        // Un arrastre son muchos pasos y UN solo paso de historial.
        e.mover(id, 5.0, 0.0);
        e.mover(id, 25.0, 10.0);
        e.apuntar_movimiento(id, 30.0, 10.0);
        assert_eq!(
            (e.buscar(id).unwrap().x, e.buscar(id).unwrap().y),
            (40.0, 10.0)
        );

        assert_eq!(e.deshacer(), Some(id));
        assert_eq!(
            (e.buscar(id).unwrap().x, e.buscar(id).unwrap().y),
            (10.0, 0.0),
            "el arrastre entero se deshace de una vez"
        );
        assert!(!e.buscar(id).unwrap().borrado, "deshacer mover no borra");

        assert_eq!(e.rehacer(), Some(id));
        assert_eq!(
            (e.buscar(id).unwrap().x, e.buscar(id).unwrap().y),
            (40.0, 10.0)
        );

        // Y el siguiente deshacer se lleva el alta, que es el paso anterior.
        e.deshacer();
        assert_eq!(e.deshacer(), Some(id));
        assert!(e.buscar(id).unwrap().borrado);
    }

    #[test]
    fn un_arrastre_que_acaba_donde_empezo_no_deja_paso() {
        let mut e = Escena::nueva();
        let id = e.anadir(rect(10.0));
        e.apuntar_movimiento(id, 0.0, 0.0);
        // Solo esta el alta: deshacer una vez y ya no queda nada.
        assert!(e.hay_que_deshacer());
        e.deshacer();
        assert!(!e.hay_que_deshacer());
    }

    #[test]
    fn hacer_algo_nuevo_corta_la_rama_de_rehacer() {
        let mut e = Escena::nueva();
        let uno = e.anadir(rect(0.0));
        e.deshacer();
        e.anadir(rect(100.0));
        assert_eq!(e.rehacer(), None, "el rehacer viejo ya no vale");
        assert!(
            e.buscar(uno).unwrap().borrado,
            "y lo deshecho sigue deshecho"
        );
    }

    #[test]
    fn el_borrador_se_deshace() {
        let mut e = Escena::nueva();
        let id = e.anadir(rect(0.0));
        assert!(e.borrar_apuntando(id));
        assert_eq!(e.cuantos_visibles(), 0);
        e.deshacer();
        assert_eq!(e.cuantos_visibles(), 1, "vuelve lo borrado a mano");
    }

    #[test]
    fn el_historial_no_viaja_en_el_fichero() {
        // Caso negativo: al abrir un dibujo de ayer no hay nada que
        // deshacer, porque no se hizo nada en esta sesion.
        let mut e = Escena::nueva();
        e.anadir(rect(0.0));
        let texto = serde_json::to_string(&e).unwrap();
        let vuelta: Escena = serde_json::from_str(&texto).unwrap();
        assert_eq!(vuelta.cuantos_visibles(), 1);
        assert!(!vuelta.hay_que_deshacer());
        assert_eq!(vuelta.elementos, e.elementos);
    }

    #[test]
    fn los_ids_no_se_repiten_ni_empiezan_en_cero() {
        // El cero es un id valido en el mundo, pero aqui es la señal de
        // "sin asignar" del elemento recien construido.
        let mut e = Escena::nueva();
        let a = e.anadir(rect(0.0));
        let b = e.anadir(rect(100.0));
        assert_eq!((a, b), (1, 2));
        assert_ne!(a, b);
    }

    #[test]
    fn deshacer_y_rehacer_vuelven_al_mismo_sitio() {
        let mut e = Escena::nueva();
        e.anadir(rect(0.0));
        let segundo = e.anadir(rect(100.0));

        assert_eq!(e.deshacer(), Some(segundo), "deshace el ultimo");
        assert_eq!(e.cuantos_visibles(), 1);
        assert_eq!(e.rehacer(), Some(segundo));
        assert_eq!(e.cuantos_visibles(), 2);
    }

    #[test]
    fn deshacer_sobre_una_escena_vacia_no_entra_en_panico() {
        let mut e = Escena::nueva();
        assert_eq!(e.deshacer(), None);
        assert_eq!(e.rehacer(), None);
    }

    #[test]
    fn deshacer_dos_veces_deshace_dos_elementos_distintos() {
        // Caso negativo del anterior: una implementacion que solo mirase el
        // ultimo de la lista deshareria siempre el mismo.
        let mut e = Escena::nueva();
        let uno = e.anadir(rect(0.0));
        let dos = e.anadir(rect(100.0));
        assert_eq!(e.deshacer(), Some(dos));
        assert_eq!(e.deshacer(), Some(uno));
        assert_eq!(e.cuantos_visibles(), 0);
    }

    #[test]
    fn compactar_saca_los_borrados_y_deja_los_demas() {
        let mut e = Escena::nueva();
        e.anadir(rect(0.0));
        let dos = e.anadir(rect(100.0));
        e.borrar(dos);
        assert_eq!(e.elementos.len(), 2, "antes de compactar siguen los dos");
        e.compactar();
        assert_eq!(e.elementos.len(), 1);
        assert_eq!(e.elementos[0].x, 0.0);
    }

    #[test]
    fn traer_al_frente_cambia_quien_recibe_el_clic() {
        let mut e = Escena::nueva();
        let uno = e.anadir(rect(0.0));
        let dos = e.anadir(rect(0.0));
        let p = Punto2::nuevo(25.0, 25.0);
        assert_eq!(e.elemento_en(p), Some(dos));
        e.traer_al_frente(uno);
        assert_eq!(e.elemento_en(p), Some(uno));
    }

    #[test]
    fn la_caja_abarca_todo_lo_visible_y_solo_eso() {
        let mut e = Escena::nueva();
        e.anadir(rect(0.0));
        let lejos = e.anadir(rect(1000.0));
        assert_eq!(e.caja(), Some((0.0, 0.0, 1050.0, 50.0)));
        // Un elemento borrado no cuenta: si contara, la vista "ajustar a la
        // ventana" se alejaria hasta un trazo que ya no existe.
        e.borrar(lejos);
        assert_eq!(e.caja(), Some((0.0, 0.0, 50.0, 50.0)));
    }

    #[test]
    fn una_escena_vacia_no_tiene_caja() {
        assert_eq!(Escena::nueva().caja(), None);
    }

    #[test]
    fn borrar_dos_veces_lo_mismo_no_cuenta_dos_veces() {
        let mut e = Escena::nueva();
        let uno = e.anadir(rect(0.0));
        assert!(e.borrar(uno));
        assert!(!e.borrar(uno), "ya estaba borrado");
        assert!(!e.borrar(999), "no existe");
    }
}
