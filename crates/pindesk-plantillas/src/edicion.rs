//! **Editar**: insertar donde se suelta, mover, duplicar, borrar, cambiar
//! el tamaño y deshacer. Puro: la app solo traduce el raton a milimetros de
//! la pagina y pinta la guia que sale de [`destino_en`].
//!
//! Soltar tiene tres casos, como en cualquier editor por bloques:
//!
//! - **Entre** los hijos de una fila o columna: la guia es una raya entre
//!   ellos y el bloque entra en ese hueco.
//! - **Al lado** de un bloque (sus bordes): si su contenedor reparte en ese
//!   eje, entra delante o detras; si no, los dos se envuelven en una fila
//!   (izquierda/derecha) o una columna (arriba/abajo) nueva.
//! - **Dentro** de un contenedor vacio: la guia es la caja entera.

use crate::modelo::{Bloque, Documento, Eje, HUECO, Tam, Tipo};
use crate::prim::{Caja, PaginaHecha};

/// Lo que se recuerda para deshacer.
pub const MAX_DESHACER: usize = 200;
/// Lo que mide la franja del borde que cuenta como «al lado» (mm).
pub const BORDE: f32 = 3.0;

/// Un borde de un bloque.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lado {
    Arriba,
    Abajo,
    Izquierda,
    Derecha,
}

impl Lado {
    fn eje(self) -> Eje {
        match self {
            Lado::Arriba | Lado::Abajo => Eje::Vertical,
            Lado::Izquierda | Lado::Derecha => Eje::Horizontal,
        }
    }
    fn detras(self) -> bool {
        matches!(self, Lado::Abajo | Lado::Derecha)
    }
}

/// **Donde va lo que se suelta.**
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Destino {
    Dentro { padre: u64, indice: usize },
    Junto { id: u64, lado: Lado },
}

/// **La guia** que se pinta mientras se arrastra.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Guia {
    /// Una raya de insercion (de `a` a `b`).
    Raya { a: (f32, f32), b: (f32, f32) },
    /// Una caja entera (un contenedor vacio).
    Zona(Caja),
}

/// **Donde caeria** algo soltado en `(x, y)` (mm de la pagina). `arrastrado`
/// es el bloque que se mueve (no puede caer dentro de si mismo).
pub fn destino_en(
    p: &PaginaHecha,
    doc: &Documento,
    x: f32,
    y: f32,
    arrastrado: Option<u64>,
) -> Option<(Destino, Guia)> {
    let prohibidos: Vec<u64> = match arrastrado.and_then(|id| doc.buscar(id)) {
        Some(b) => {
            let mut v = Vec::new();
            b.recorrer(&mut |h, _| v.push(h.id));
            v
        }
        None => Vec::new(),
    };
    let c = p
        .cajas
        .iter()
        .filter(|c| c.caja.contiene(x, y) && !prohibidos.contains(&c.id))
        .max_by_key(|c| c.profundidad)?;
    let caja = c.caja;
    if let Some(eje) = c.eje {
        // Cerca de su propio borde (y no es la raiz): al lado del contenedor.
        let cerca = (x - caja.x)
            .min(caja.derecha() - x)
            .min(y - caja.y)
            .min(caja.abajo() - y)
            < BORDE * 0.5;
        if !(cerca && c.padre.is_some()) {
            let hijos: Vec<_> = p
                .cajas
                .iter()
                .filter(|h| h.padre == Some(c.id) && !prohibidos.contains(&h.id))
                .collect();
            if hijos.is_empty() {
                return Some((
                    Destino::Dentro {
                        padre: c.id,
                        indice: 0,
                    },
                    Guia::Zona(caja.dentro(0.8)),
                ));
            }
            // El hueco: cuantos hijos quedan antes del punto en su eje.
            let mut orden = hijos.clone();
            orden.sort_by_key(|h| h.indice);
            let antes = orden
                .iter()
                .filter(|h| match eje {
                    Eje::Vertical => h.caja.y + h.caja.h / 2.0 < y,
                    Eje::Horizontal => {
                        // En una rejilla, por renglones.
                        h.caja.abajo() < y || (h.caja.y <= y && h.caja.x + h.caja.w / 2.0 < x)
                    }
                })
                .count();
            let indice = orden.get(antes).map_or(orden.len(), |h| h.indice);
            let guia = match eje {
                Eje::Vertical => {
                    let gy = match (
                        antes.checked_sub(1).and_then(|i| orden.get(i)),
                        orden.get(antes),
                    ) {
                        (Some(a), Some(b)) => (a.caja.abajo() + b.caja.y) / 2.0,
                        (Some(a), None) => a.caja.abajo() + 1.0,
                        (None, Some(b)) => b.caja.y - 1.0,
                        (None, None) => caja.y,
                    };
                    Guia::Raya {
                        a: (caja.x, gy),
                        b: (caja.derecha(), gy),
                    }
                }
                Eje::Horizontal => {
                    let (gx, y0, y1) = match (
                        antes.checked_sub(1).and_then(|i| orden.get(i)),
                        orden.get(antes),
                    ) {
                        (_, Some(b)) => (b.caja.x - 1.0, b.caja.y, b.caja.abajo()),
                        (Some(a), None) => (a.caja.derecha() + 1.0, a.caja.y, a.caja.abajo()),
                        (None, None) => (caja.x, caja.y, caja.abajo()),
                    };
                    Guia::Raya {
                        a: (gx, y0),
                        b: (gx, y1),
                    }
                }
            };
            return Some((
                Destino::Dentro {
                    padre: c.id,
                    indice,
                },
                guia,
            ));
        }
    }
    // Una hoja (o el borde de un contenedor): al lado.
    let fx = (x - caja.x) / caja.w.max(0.01);
    let fy = (y - caja.y) / caja.h.max(0.01);
    let lado = if fx < 0.22 {
        Lado::Izquierda
    } else if fx > 0.78 {
        Lado::Derecha
    } else if fy < 0.5 {
        Lado::Arriba
    } else {
        Lado::Abajo
    };
    let guia = match lado {
        Lado::Arriba => Guia::Raya {
            a: (caja.x, caja.y),
            b: (caja.derecha(), caja.y),
        },
        Lado::Abajo => Guia::Raya {
            a: (caja.x, caja.abajo()),
            b: (caja.derecha(), caja.abajo()),
        },
        Lado::Izquierda => Guia::Raya {
            a: (caja.x, caja.y),
            b: (caja.x, caja.abajo()),
        },
        Lado::Derecha => Guia::Raya {
            a: (caja.derecha(), caja.y),
            b: (caja.derecha(), caja.abajo()),
        },
    };
    Some((Destino::Junto { id: c.id, lado }, guia))
}

/// **El asa de cambiar el tamaño** del bloque `id`: su borde de detras en
/// el eje de su contenedor, si `(x, y)` esta encima. Devuelve el eje.
pub fn asa_en(p: &PaginaHecha, id: u64, x: f32, y: f32) -> Option<Eje> {
    let c = p.caja_de(id)?;
    let padre = p.caja_de(c.padre?)?;
    let caja = c.caja;
    match padre.eje? {
        Eje::Vertical => ((y - caja.abajo()).abs() < 1.6 && x >= caja.x && x <= caja.derecha())
            .then_some(Eje::Vertical),
        Eje::Horizontal => ((x - caja.derecha()).abs() < 1.6 && y >= caja.y && y <= caja.abajo())
            .then_some(Eje::Horizontal),
    }
}

/// **El editor**: el documento, lo elegido y la historia.
#[derive(Debug, Clone)]
pub struct Editor {
    pub doc: Documento,
    pub seleccion: Option<u64>,
    atras: Vec<Documento>,
    adelante: Vec<Documento>,
    /// Si hay cambios sin guardar.
    pub cambiado: bool,
}

impl Editor {
    pub fn nuevo(mut doc: Documento) -> Editor {
        doc.numerar();
        Editor {
            doc,
            seleccion: None,
            atras: Vec::new(),
            adelante: Vec::new(),
            cambiado: false,
        }
    }

    /// Apunta el estado de ahora para poder volver.
    pub fn paso(&mut self) {
        self.atras.push(self.doc.clone());
        if self.atras.len() > MAX_DESHACER {
            self.atras.remove(0);
        }
        self.adelante.clear();
        self.cambiado = true;
    }

    /// Quita el ultimo paso apuntado (un cambio que al final no cambio nada).
    fn olvidar_paso(&mut self) {
        self.atras.pop();
    }

    pub fn se_puede_deshacer(&self) -> bool {
        !self.atras.is_empty()
    }
    pub fn se_puede_rehacer(&self) -> bool {
        !self.adelante.is_empty()
    }

    pub fn deshacer(&mut self) -> bool {
        let Some(d) = self.atras.pop() else {
            return false;
        };
        self.adelante.push(std::mem::replace(&mut self.doc, d));
        self.validar_seleccion();
        self.cambiado = true;
        true
    }

    pub fn rehacer(&mut self) -> bool {
        let Some(d) = self.adelante.pop() else {
            return false;
        };
        self.atras.push(std::mem::replace(&mut self.doc, d));
        self.validar_seleccion();
        self.cambiado = true;
        true
    }

    fn validar_seleccion(&mut self) {
        if let Some(s) = self.seleccion
            && self.doc.buscar(s).is_none()
        {
            self.seleccion = None;
        }
    }

    /// **Cambia el documento entero** (aplicar lo de la IA, otra plantilla
    /// de la galeria), con su paso de deshacer.
    pub fn poner_documento(&mut self, mut doc: Documento) {
        self.paso();
        doc.numerar();
        self.doc = doc;
        self.validar_seleccion();
    }

    /// **Inserta** un bloque nuevo y lo elige. Devuelve su id.
    pub fn insertar(&mut self, b: Bloque, d: Destino) -> Option<u64> {
        self.paso();
        let mut b = b;
        renumerar(&mut b, self.doc.id_maximo() + 1);
        let id = b.id;
        if poner_en(&mut self.doc, b, d) {
            self.doc.numerar();
            self.seleccion = Some(id);
            Some(id)
        } else {
            self.olvidar_paso();
            None
        }
    }

    /// **Mueve** un bloque a otro sitio.
    pub fn mover(&mut self, id: u64, d: Destino) -> bool {
        if id == self.doc.raiz.id {
            return false;
        }
        let objetivo = match d {
            Destino::Dentro { padre, .. } => padre,
            Destino::Junto { id: o, .. } => o,
        };
        // No dentro de si mismo.
        if self
            .doc
            .buscar(id)
            .is_some_and(|b| b.buscar(objetivo).is_some())
        {
            return false;
        }
        let antes = self.doc.clone();
        self.paso();
        let origen = self.doc.raiz.padre_de(id);
        let Some(b) = quitar(&mut self.doc.raiz, id) else {
            self.olvidar_paso();
            return false;
        };
        // Quitarlo corre los de detras en el mismo contenedor.
        let d = match (d, origen) {
            (Destino::Dentro { padre, indice }, Some((p, i))) if p == padre && i < indice => {
                Destino::Dentro {
                    padre,
                    indice: indice - 1,
                }
            }
            _ => d,
        };
        if poner_en(&mut self.doc, b, d) {
            self.seleccion = Some(id);
            if self.doc == antes {
                self.olvidar_paso();
                return false;
            }
            true
        } else {
            self.doc = antes;
            self.olvidar_paso();
            false
        }
    }

    /// **Duplica** un bloque justo detras de el.
    pub fn duplicar(&mut self, id: u64) -> Option<u64> {
        let (padre, i) = self.doc.raiz.padre_de(id)?;
        let mut copia = self.doc.buscar(id)?.clone();
        copia.recorrer_mut(&mut |b| b.id = 0);
        self.insertar(
            copia,
            Destino::Dentro {
                padre,
                indice: i + 1,
            },
        )
    }

    /// **Borra** un bloque (la raiz no).
    pub fn borrar(&mut self, id: u64) -> bool {
        if id == self.doc.raiz.id || self.doc.buscar(id).is_none() {
            return false;
        }
        self.paso();
        let ok = quitar(&mut self.doc.raiz, id).is_some();
        if self.seleccion == Some(id) {
            self.seleccion = None;
        }
        ok
    }

    /// **Cambia un bloque** con `f` (que dice si cambio algo).
    pub fn cambiar(&mut self, id: u64, f: impl FnOnce(&mut Bloque) -> bool) -> bool {
        self.paso();
        let ok = self.doc.buscar_mut(id).is_some_and(f);
        if !ok {
            self.olvidar_paso();
        }
        ok
    }

    /// **Cambia lo de fuera del arbol** (pagina, tema, repeticion…).
    pub fn cambiar_documento(&mut self, f: impl FnOnce(&mut Documento)) -> bool {
        let antes = self.doc.clone();
        self.paso();
        f(&mut self.doc);
        if self.doc == antes {
            self.olvidar_paso();
            return false;
        }
        true
    }

    /// **Sube o baja** un bloque entre sus hermanos.
    pub fn correr(&mut self, id: u64, delta: i32) -> bool {
        let Some((padre, i)) = self.doc.raiz.padre_de(id) else {
            return false;
        };
        let n = self
            .doc
            .buscar(padre)
            .and_then(|p| p.tipo.hijos())
            .map_or(0, Vec::len);
        let j = i as i64 + i64::from(delta);
        if j < 0 || j >= n as i64 {
            return false;
        }
        self.cambiar(padre, |p| {
            if let Some(h) = p.tipo.hijos_mut() {
                let b = h.remove(i);
                h.insert(j as usize, b);
            }
            true
        })
    }

    /// **Envuelve** un bloque en un contenedor nuevo (`fila`, `columna`,
    /// `seccion`). Devuelve el id del contenedor.
    pub fn envolver(&mut self, id: u64, en: &str) -> Option<u64> {
        let nuevo_id = self.doc.id_maximo() + 1;
        self.paso();
        let ok = envolver_en(&mut self.doc, id, |mut b| {
            let tam = std::mem::take(&mut b.tam);
            let hijos = vec![b];
            let tipo = match en {
                "fila" => Tipo::Fila {
                    hijos,
                    hueco: HUECO,
                },
                "seccion" => Tipo::Seccion {
                    titulo: String::new(),
                    hijos,
                    recuadro: false,
                },
                _ => Tipo::Columna {
                    hijos,
                    hueco: HUECO,
                },
            };
            Bloque {
                id: nuevo_id,
                tam,
                acento: None,
                tipo,
            }
        });
        if ok {
            self.seleccion = Some(nuevo_id);
            Some(nuevo_id)
        } else {
            self.olvidar_paso();
            None
        }
    }

    /// **Pone otro bloque en su sitio** (lo que devuelve «ajustar» de la
    /// IA), conservando su tamaño en el contenedor si el nuevo no trae.
    pub fn reemplazar(&mut self, id: u64, mut nuevo: Bloque) -> bool {
        let Some(viejo) = self.doc.buscar(id) else {
            return false;
        };
        if nuevo.tam == Tam::Auto {
            nuevo.tam = viejo.tam;
        }
        renumerar(&mut nuevo, self.doc.id_maximo() + 1);
        nuevo.id = id;
        self.cambiar(id, |b| {
            *b = nuevo;
            true
        })
    }

    /// Cambia el tamaño sin apuntar paso (mientras se arrastra el asa; el
    /// paso se apunta con [`Editor::paso`] al empezar).
    pub fn redimensionar_sin_paso(&mut self, id: u64, mm: f32) {
        if let Some(b) = self.doc.buscar_mut(id) {
            b.tam = Tam::Fijo(mm.clamp(4.0, 2000.0));
        }
    }
}

impl Bloque {
    /// Recorre el arbol cambiando cada uno.
    pub fn recorrer_mut(&mut self, f: &mut dyn FnMut(&mut Bloque)) {
        f(self);
        if let Some(h) = self.tipo.hijos_mut() {
            for c in h {
                c.recorrer_mut(f);
            }
        }
    }
}

/// Da ids nuevos desde `desde` a los que tienen 0.
fn renumerar(b: &mut Bloque, desde: u64) {
    let mut n = desde;
    b.recorrer_mut(&mut |x| {
        if x.id == 0 {
            x.id = n;
            n += 1;
        }
    });
}

/// Quita un bloque del arbol y lo devuelve.
fn quitar(raiz: &mut Bloque, id: u64) -> Option<Bloque> {
    let hijos = raiz.tipo.hijos_mut()?;
    if let Some(i) = hijos.iter().position(|h| h.id == id) {
        return Some(hijos.remove(i));
    }
    hijos.iter_mut().find_map(|h| quitar(h, id))
}

/// Cambia el bloque `id` por lo que haga `f` con el (envolverlo).
fn envolver_en(doc: &mut Documento, id: u64, f: impl FnOnce(Bloque) -> Bloque) -> bool {
    if doc.raiz.id == id {
        let raiz = std::mem::replace(&mut doc.raiz, Bloque::nuevo(Tipo::Espacio {}));
        doc.raiz = f(raiz);
        return true;
    }
    let Some((padre, i)) = doc.raiz.padre_de(id) else {
        return false;
    };
    let Some(h) = doc.buscar_mut(padre).and_then(|p| p.tipo.hijos_mut()) else {
        return false;
    };
    let b = h.remove(i);
    h.insert(i, f(b));
    true
}

/// Pone `b` en el destino.
fn poner_en(doc: &mut Documento, b: Bloque, d: Destino) -> bool {
    match d {
        Destino::Dentro { padre, indice } => {
            let Some(h) = doc.buscar_mut(padre).and_then(|p| p.tipo.hijos_mut()) else {
                return false;
            };
            let i = indice.min(h.len());
            h.insert(i, b);
            true
        }
        Destino::Junto { id, lado } => {
            // Si el contenedor reparte en ese eje, delante o detras.
            if let Some((padre, i)) = doc.raiz.padre_de(id) {
                let eje_padre = doc.buscar(padre).and_then(|p| match p.tipo {
                    Tipo::Fila { .. } => Some(Eje::Horizontal),
                    Tipo::Columna { .. } | Tipo::Seccion { .. } => Some(Eje::Vertical),
                    _ => None,
                });
                if eje_padre == Some(lado.eje()) {
                    let indice = if lado.detras() { i + 1 } else { i };
                    return poner_en(doc, b, Destino::Dentro { padre, indice });
                }
            }
            // Si no, los dos en una fila o columna nueva.
            let nuevo_id = doc.id_maximo().max(b.id) + 1;
            envolver_en(doc, id, |mut viejo| {
                let tam = std::mem::take(&mut viejo.tam);
                let hijos = if lado.detras() {
                    vec![viejo, b]
                } else {
                    vec![b, viejo]
                };
                Bloque {
                    id: nuevo_id,
                    tam,
                    acento: None,
                    tipo: match lado.eje() {
                        Eje::Horizontal => Tipo::Fila {
                            hijos,
                            hueco: HUECO,
                        },
                        Eje::Vertical => Tipo::Columna {
                            hijos,
                            hueco: HUECO,
                        },
                    },
                }
            })
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::campos::Contexto;
    use crate::diseno::maquetar;
    use crate::fecha::Fecha;
    use crate::modelo::hacer;

    fn doc() -> Documento {
        let mut d = Documento::nuevo("prueba");
        if let Some(h) = d.raiz.tipo.hijos_mut() {
            h.push(hacer::encabezado("Titulo", 1));
            h.push(
                hacer::fila(vec![hacer::notas("A"), hacer::notas("B")]).con_tam(Tam::Fijo(60.0)),
            );
            h.push(hacer::casillas(&["uno"], 3));
        }
        d
    }

    fn ids(d: &Documento) -> Vec<(u64, &'static str)> {
        let mut v = Vec::new();
        d.raiz.recorrer(&mut |b, _| v.push((b.id, b.tipo.clave())));
        v
    }

    #[test]
    fn insertar_mover_duplicar_borrar_y_deshacer_todo() {
        let mut e = Editor::nuevo(doc());
        let raiz = e.doc.raiz.id;
        let original = e.doc.clone();
        let id = e
            .insertar(
                hacer::divisor(),
                Destino::Dentro {
                    padre: raiz,
                    indice: 1,
                },
            )
            .unwrap();
        assert_eq!(e.seleccion, Some(id));
        assert_eq!(e.doc.raiz.tipo.hijos().unwrap()[1].id, id);
        // Moverlo al final.
        assert!(e.mover(
            id,
            Destino::Dentro {
                padre: raiz,
                indice: 4
            }
        ));
        assert_eq!(e.doc.raiz.tipo.hijos().unwrap().last().unwrap().id, id);
        let dup = e.duplicar(id).unwrap();
        assert_ne!(dup, id);
        assert_eq!(e.doc.raiz.tipo.hijos().unwrap().len(), 5);
        assert!(e.borrar(dup));
        assert!(!e.borrar(raiz), "la raiz no se borra");
        assert!(e.correr(id, -1));
        assert!(!e.correr(id, -10));
        // Deshacer hasta el principio deja lo de antes.
        while e.deshacer() {}
        assert_eq!(e.doc, original);
        assert!(e.rehacer());
        assert_eq!(e.doc.cuantos(), original.cuantos() + 1);
    }

    #[test]
    fn soltar_al_lado_envuelve_cuando_el_eje_no_cuadra() {
        let mut e = Editor::nuevo(doc());
        let casillas = ids(&e.doc).iter().find(|x| x.1 == "casillas").unwrap().0;
        // A la derecha de una lista que esta en una columna: fila nueva.
        let id = e
            .insertar(
                hacer::texto("al lado"),
                Destino::Junto {
                    id: casillas,
                    lado: Lado::Derecha,
                },
            )
            .unwrap();
        let (padre, i) = e.doc.raiz.padre_de(id).unwrap();
        assert_eq!(i, 1);
        assert!(matches!(
            e.doc.buscar(padre).unwrap().tipo,
            Tipo::Fila { .. }
        ));
        // Debajo de una nota que esta en una fila: columna nueva.
        let nota_a = ids(&e.doc).iter().find(|x| x.1 == "notas").unwrap().0;
        let id2 = e
            .insertar(
                hacer::texto("debajo"),
                Destino::Junto {
                    id: nota_a,
                    lado: Lado::Abajo,
                },
            )
            .unwrap();
        let (p2, _) = e.doc.raiz.padre_de(id2).unwrap();
        assert!(matches!(
            e.doc.buscar(p2).unwrap().tipo,
            Tipo::Columna { .. }
        ));
        // Debajo de algo que esta en una columna: solo se inserta.
        let antes = e.doc.raiz.tipo.hijos().unwrap().len();
        let enc = ids(&e.doc).iter().find(|x| x.1 == "encabezado").unwrap().0;
        e.insertar(
            hacer::divisor(),
            Destino::Junto {
                id: enc,
                lado: Lado::Abajo,
            },
        )
        .unwrap();
        assert_eq!(e.doc.raiz.tipo.hijos().unwrap().len(), antes + 1);
    }

    #[test]
    fn no_se_mueve_dentro_de_si_mismo() {
        let mut e = Editor::nuevo(doc());
        let fila = ids(&e.doc).iter().find(|x| x.1 == "fila").unwrap().0;
        let nota = ids(&e.doc).iter().find(|x| x.1 == "notas").unwrap().0;
        let antes = e.doc.clone();
        assert!(!e.mover(
            fila,
            Destino::Junto {
                id: nota,
                lado: Lado::Abajo
            }
        ));
        assert!(!e.mover(
            fila,
            Destino::Dentro {
                padre: fila,
                indice: 0
            }
        ));
        assert_eq!(e.doc, antes);
        assert!(!e.se_puede_deshacer(), "lo que no se hizo no deja paso");
    }

    #[test]
    fn la_guia_de_soltar_sale_donde_toca() {
        let e = Editor::nuevo(doc());
        let ctx = Contexto::nuevo(Fecha::nueva(2026, 11, 6).unwrap(), "es");
        let p = maquetar(&e.doc, &ctx);
        let fila = p
            .cajas
            .iter()
            .find(|c| c.eje == Some(Eje::Horizontal))
            .unwrap();
        let nota_a = p
            .cajas
            .iter()
            .find(|c| c.padre == Some(fila.id) && c.indice == 0)
            .unwrap();
        // En el borde izquierdo de la primera nota: delante de ella.
        let (d, g) =
            destino_en(&p, &e.doc, nota_a.caja.x + 1.0, nota_a.caja.y + 20.0, None).unwrap();
        assert_eq!(
            d,
            Destino::Junto {
                id: nota_a.id,
                lado: Lado::Izquierda
            }
        );
        assert!(matches!(g, Guia::Raya { a, b } if (a.0 - b.0).abs() < 1e-3));
        // Debajo del todo de la pagina (dentro de la raiz, tras el ultimo).
        let raiz = p.cajas.iter().find(|c| c.padre.is_none()).unwrap();
        let (d, _) = destino_en(
            &p,
            &e.doc,
            raiz.caja.x + 50.0,
            raiz.caja.abajo() - 2.0,
            None,
        )
        .unwrap();
        assert_eq!(
            d,
            Destino::Dentro {
                padre: raiz.id,
                indice: 3
            }
        );
        // Fuera de la pagina, nada.
        assert!(destino_en(&p, &e.doc, -5.0, -5.0, None).is_none());
        // El asa de la fila (fija a 60 mm) en su borde de abajo.
        assert_eq!(
            asa_en(&p, fila.id, fila.caja.x + 10.0, fila.caja.abajo()),
            Some(Eje::Vertical)
        );
        assert_eq!(asa_en(&p, fila.id, fila.caja.x + 10.0, fila.caja.y), None);
    }
}
