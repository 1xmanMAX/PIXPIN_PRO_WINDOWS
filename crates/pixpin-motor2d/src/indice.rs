//! Que elementos pueden estar en una zona, sin recorrerlos todos.
//!
//! `elemento_en` y `recortar` recorren la lista entera. Con ocho mil
//! elementos, mover el raton por encima cuesta ocho mil pruebas de impacto
//! por cada aviso del raton.
//!
//! # Por que una rejilla y no un arbol (D27)
//!
//! Un quadtree o un R-tree serian mas elegantes y escalarian mejor a
//! millones. Pero aqui hablamos de ocho mil, y a esa escala la rejilla ya
//! gana por goleada. Ademas:
//!
//! - **No reasigna al consultar.** Un arbol recorre nodos y va acumulando;
//!   la rejilla lee celdas contiguas.
//! - **Se demuestra correcta contra la fuerza bruta**, que es justo lo que
//!   hace la prueba principal. Un arbol mal equilibrado da resultados casi
//!   correctos, que es la peor clase de fallo.
//!
//! # La regla que no se puede romper
//!
//! La rejilla puede devolver **de mas** —quien la usa filtra despues— pero
//! jamas **de menos**. Un elemento que no salga de aqui es un elemento que
//! desaparece de la pantalla sin ningun error visible.

use std::collections::{HashMap, HashSet};

#[cfg(test)]
use crate::elemento::Elemento;
use crate::escena::Escena;

/// Lado de la celda, en unidades del mundo.
///
/// 256 sale de que un elemento tipico de trabajo mide entre 20 y 300: con
/// celdas mucho mas pequenas, cada elemento se apunta en decenas de ellas;
/// con celdas mucho mas grandes, cada consulta devuelve medio dibujo.
pub const CELDA: f32 = 256.0;

#[derive(Debug, Clone)]
pub struct Rejilla {
    lado: f32,
    celdas: HashMap<(i32, i32), Vec<u64>>,
    /// La `version` con la que se apunto cada elemento, para saber cual ha
    /// cambiado sin comparar elementos enteros.
    versiones: HashMap<u64, u32>,
}

impl Default for Rejilla {
    fn default() -> Self {
        Self::con_celda(CELDA)
    }
}

impl Rejilla {
    pub fn nueva() -> Self {
        Self::default()
    }

    pub fn con_celda(lado: f32) -> Self {
        Self {
            lado: lado.max(1.0),
            celdas: HashMap::new(),
            versiones: HashMap::new(),
        }
    }

    fn celda_de(&self, x: f32, y: f32) -> (i32, i32) {
        (
            (x / self.lado).floor() as i32,
            (y / self.lado).floor() as i32,
        )
    }

    /// Pone la rejilla al dia con la escena.
    ///
    /// Se llama en cada fotograma, asi que **no reconstruye**: mira la
    /// `version` de cada elemento y solo reapunta los que han cambiado. Si
    /// reconstruyera, seria mas cara que la fuerza bruta que viene a
    /// evitar.
    pub fn sincronizar(&mut self, escena: &Escena) {
        let mut vistos: HashSet<u64> = HashSet::with_capacity(escena.elementos.len());

        for e in &escena.elementos {
            vistos.insert(e.id);
            let cambio = match self.versiones.get(&e.id) {
                Some(v) => *v != e.version,
                None => true,
            };
            if !cambio {
                continue;
            }
            self.quitar(e.id);
            if !e.borrado {
                self.meter(e.id, e.caja());
            }
            self.versiones.insert(e.id, e.version);
        }

        // Los que ya no estan en la escena (compactar los saco de verdad).
        let sobrantes: Vec<u64> = self
            .versiones
            .keys()
            .copied()
            .filter(|id| !vistos.contains(id))
            .collect();
        for id in sobrantes {
            self.quitar(id);
            self.versiones.remove(&id);
        }
    }

    fn meter(&mut self, id: u64, caja: (f32, f32, f32, f32)) {
        let (x0, y0, x1, y1) = caja;
        let (cx0, cy0) = self.celda_de(x0, y0);
        let (cx1, cy1) = self.celda_de(x1, y1);
        for cx in cx0..=cx1 {
            for cy in cy0..=cy1 {
                self.celdas.entry((cx, cy)).or_default().push(id);
            }
        }
    }

    fn quitar(&mut self, id: u64) {
        // Recorrer las celdas es aceptable porque solo pasa cuando un
        // elemento cambia, y entonces esta en unas pocas.
        self.celdas.retain(|_, ids| {
            ids.retain(|x| *x != id);
            !ids.is_empty()
        });
    }

    /// Los que **pueden** estar en la caja. Puede devolver de mas, nunca de
    /// menos: quien lo use filtra con `toca` o con `dentro_de`.
    pub fn candidatos(&self, caja: (f32, f32, f32, f32)) -> Vec<u64> {
        let (x0, y0, x1, y1) = caja;
        let (x0, x1) = (x0.min(x1), x0.max(x1));
        let (y0, y1) = (y0.min(y1), y0.max(y1));
        let (cx0, cy0) = self.celda_de(x0, y0);
        let (cx1, cy1) = self.celda_de(x1, y1);

        let mut fuera: Vec<u64> = Vec::new();
        for cx in cx0..=cx1 {
            for cy in cy0..=cy1 {
                let Some(ids) = self.celdas.get(&(cx, cy)) else {
                    continue;
                };
                for id in ids {
                    if !fuera.contains(id) {
                        fuera.push(*id);
                    }
                }
            }
        }
        fuera
    }

    /// Cuantas celdas ocupadas hay. Para las pruebas.
    pub fn cuantas_celdas(&self) -> usize {
        self.celdas.len()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::azar::Azar;
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};

    fn rect(id: u64, x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
        Elemento {
            id,
            figura: Figura::Rectangulo,
            x,
            y,
            ancho,
            alto,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
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

    /// Lo mismo que hace la rejilla, pero recorriendo todo. Es la verdad
    /// contra la que se compara.
    fn a_lo_bruto(escena: &Escena, caja: (f32, f32, f32, f32)) -> Vec<u64> {
        let (bx0, by0, bx1, by1) = caja;
        let mut fuera: Vec<u64> = escena
            .elementos
            .iter()
            .filter(|e| !e.borrado)
            .filter(|e| {
                let (x0, y0, x1, y1) = e.caja();
                x1 >= bx0 && x0 <= bx1 && y1 >= by0 && y0 <= by1
            })
            .map(|e| e.id)
            .collect();
        fuera.sort_unstable();
        fuera
    }

    #[test]
    fn la_rejilla_dice_lo_mismo_que_la_fuerza_bruta() {
        // La prueba que justifica la rejilla entera. Escenas al azar pero
        // reproducibles: Azar es el Lehmer que ya usa el motor para que un
        // dibujo tenga el mismo garabato en cada apertura.
        let mut azar = Azar::nuevo(20260906);
        let mut escena = Escena::nueva();
        for i in 0..800 {
            let x = azar.siguiente() * 4000.0 - 2000.0;
            let y = azar.siguiente() * 4000.0 - 2000.0;
            let ancho = 1.0 + azar.siguiente() * 300.0;
            let alto = 1.0 + azar.siguiente() * 300.0;
            escena.anadir(rect(i + 1, x, y, ancho, alto));
        }

        let mut rejilla = Rejilla::nueva();
        rejilla.sincronizar(&escena);

        for _ in 0..200 {
            let x = azar.siguiente() * 4000.0 - 2000.0;
            let y = azar.siguiente() * 4000.0 - 2000.0;
            let caja = (
                x,
                y,
                x + azar.siguiente() * 900.0,
                y + azar.siguiente() * 900.0,
            );

            let mut de_la_rejilla = rejilla.candidatos(caja);
            de_la_rejilla.sort_unstable();
            // La rejilla puede devolver de mas —quien la usa filtra— pero
            // JAMAS de menos: un elemento que no salga aqui es un elemento
            // que desaparece de la pantalla.
            for id in a_lo_bruto(&escena, caja) {
                assert!(
                    de_la_rejilla.contains(&id),
                    "la rejilla se dejo el {id} en la caja {caja:?}"
                );
            }
        }
    }

    #[test]
    fn un_elemento_que_cambia_de_sitio_cambia_de_celda() {
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(1, 0.0, 0.0, 10.0, 10.0));
        let mut rejilla = Rejilla::nueva();
        rejilla.sincronizar(&escena);
        assert_eq!(rejilla.candidatos((0.0, 0.0, 20.0, 20.0)), vec![id]);

        escena.buscar_mut(id).unwrap().mover(5000.0, 5000.0);
        rejilla.sincronizar(&escena);

        assert!(
            rejilla.candidatos((0.0, 0.0, 20.0, 20.0)).is_empty(),
            "ya no esta donde estaba"
        );
        assert_eq!(
            rejilla.candidatos((4990.0, 4990.0, 5020.0, 5020.0)),
            vec![id]
        );
    }

    #[test]
    fn sincronizar_sin_cambios_no_hace_nada() {
        // Se llama en cada fotograma. Si reconstruyera la rejilla entera
        // cada vez, seria mas cara que la fuerza bruta que viene a evitar.
        let mut escena = Escena::nueva();
        for i in 0..100 {
            escena.anadir(rect(i + 1, i as f32 * 10.0, 0.0, 5.0, 5.0));
        }
        let mut rejilla = Rejilla::nueva();
        rejilla.sincronizar(&escena);
        let celdas = rejilla.cuantas_celdas();

        rejilla.sincronizar(&escena);
        assert_eq!(rejilla.cuantas_celdas(), celdas, "ni una celda de mas");
    }

    #[test]
    fn un_elemento_borrado_sale_de_la_rejilla() {
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(1, 0.0, 0.0, 10.0, 10.0));
        let mut rejilla = Rejilla::nueva();
        rejilla.sincronizar(&escena);

        escena.borrar(id);
        rejilla.sincronizar(&escena);

        assert!(
            rejilla
                .candidatos((-100.0, -100.0, 100.0, 100.0))
                .is_empty()
        );
    }

    #[test]
    fn un_elemento_restaurado_vuelve_a_la_rejilla() {
        // Deshacer un borrado es exactamente esto: restaurar sube la
        // version igual que borrar, y la rejilla tiene que volver a
        // apuntar el elemento o desaparece de la pantalla sin explicacion.
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(1, 0.0, 0.0, 10.0, 10.0));
        let mut rejilla = Rejilla::nueva();
        rejilla.sincronizar(&escena);

        escena.borrar(id);
        rejilla.sincronizar(&escena);
        assert!(
            rejilla
                .candidatos((-100.0, -100.0, 100.0, 100.0))
                .is_empty(),
            "borrado, no deberia estar"
        );

        escena.restaurar(id);
        rejilla.sincronizar(&escena);
        assert_eq!(
            rejilla.candidatos((-100.0, -100.0, 100.0, 100.0)),
            vec![id],
            "restaurado, deberia volver a salir"
        );
    }

    #[test]
    fn un_elemento_enorme_ocupa_todas_las_celdas_que_cruza() {
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(1, 0.0, 0.0, 1000.0, 1000.0));
        let mut rejilla = Rejilla::con_celda(100.0);
        rejilla.sincronizar(&escena);

        // Se encuentra tocando cualquiera de sus rincones.
        assert_eq!(rejilla.candidatos((5.0, 5.0, 6.0, 6.0)), vec![id]);
        assert_eq!(rejilla.candidatos((995.0, 995.0, 996.0, 996.0)), vec![id]);
    }

    #[test]
    fn no_devuelve_el_mismo_id_dos_veces() {
        // Un elemento que cruza cuatro celdas y una consulta que abarca las
        // cuatro: si no se deduplicara, se pintaria cuatro veces.
        let mut escena = Escena::nueva();
        escena.anadir(rect(1, 90.0, 90.0, 20.0, 20.0));
        let mut rejilla = Rejilla::con_celda(100.0);
        rejilla.sincronizar(&escena);

        assert_eq!(rejilla.candidatos((0.0, 0.0, 300.0, 300.0)).len(), 1);
    }
}
