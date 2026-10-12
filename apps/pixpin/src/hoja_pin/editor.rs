//! **La hoja editable del pin**, sin ventana: lo que se escribe, lo que se
//! elige, las pestanas de las hojas y la pila de deshacer. Lo pinta y le
//! pasa el raton y el teclado `hoja_pin`; aqui todo se prueba sin GPU.
//!
//! Es lo que hacia la hoja de calculo pintada dentro del chat
//! (`HojaAbierta` de `ventana_chat`, que ya no existe), con lo que le
//! faltaba y tiene la de Android (`tabla/TablaActivity.kt`): deshacer y
//! rehacer, elegir un rango, negrita y alineacion, y las hojas de un libro
//! en pestanas.
//!
//! **Deshacer es una pila de cambios de celda**: cada cambio guarda solo
//! las celdas que toco, como estaban y como quedaron. Barato aunque la tabla
//! tenga cuarenta mil celdas: escribir en una guarda una.

use std::cell::OnceCell;
use std::collections::BTreeMap;

use pixpin_proyecto::formula::{self, Valor};
use pixpin_proyecto::importar_hojas::{MAX_COLS, MAX_FILAS};
use pixpin_proyecto::tabla::{EstiloDeCelda, Ref, Tabla, ref_a, ref_de};
use pixpin_proyecto::tabla_xlsx::{ALTO_FILA, ANCHO_COLUMNA};

/// Cuantos cambios se pueden deshacer.
const MAXIMO_DESHACER: usize = 200;
/// Mas celdas que esto de una vez (negrita, borrar) no se tocan: un rango
/// de columnas enteras seria millones.
pub const MAXIMO_RANGO: u64 = 200_000;

/// Un rango de celdas elegido: donde se empezo (`ancla`) y la celda activa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seleccion {
    pub ancla: Ref,
    pub activa: Ref,
}

impl Seleccion {
    pub fn en(r: Ref) -> Seleccion {
        Seleccion { ancla: r, activa: r }
    }

    /// Las esquinas: (columna0, fila0, columna1, fila1), incluidas.
    pub fn esquinas(&self) -> (u32, u32, u32, u32) {
        let (a, b) = (self.ancla, self.activa);
        (a.columna.min(b.columna), a.fila.min(b.fila), a.columna.max(b.columna), a.fila.max(b.fila))
    }

    pub fn celdas(&self) -> u64 {
        let (c0, f0, c1, f1) = self.esquinas();
        (c1 - c0 + 1) as u64 * (f1 - f0 + 1) as u64
    }
}

/// Lo que habia y lo que hay en una celda: su contenido y su estilo.
type Foto = (Option<String>, Option<EstiloDeCelda>);

/// Un cambio que se puede deshacer: en que hoja, las celdas que toco (antes
/// y despues) y donde estaba la seleccion.
#[derive(Debug, Clone, PartialEq)]
struct Cambio {
    hoja: usize,
    celdas: Vec<(String, Foto, Foto)>,
    sel: Seleccion,
}

/// Una hoja del libro abierta para escribir.
pub struct HojaEditable {
    /// El nombre de la pestana.
    pub nombre: String,
    pub tabla: Tabla,
    /// El mensaje del chat que la guarda (la tabla ES su texto). `None`: un
    /// libro leido para editar que aun no se guardo en el chat.
    pub id_mensaje: Option<String>,
    /// Si cambio algo desde que se guardo.
    pub tocada: bool,
    pub sel: Seleccion,
    /// Lo desplazado, en pixeles de la hoja al 100 %.
    pub x: f32,
    pub y: f32,
    /// Donde empieza cada columna (px al 100 %), hasta la ultima posible.
    xs: Vec<f32>,
    /// Lo que vale cada celda, calculado de una vez al pintar y olvidado al
    /// cambiar una.
    valores: OnceCell<BTreeMap<String, Valor>>,
}

impl HojaEditable {
    pub fn nueva(nombre: String, tabla: Tabla, id_mensaje: Option<String>) -> HojaEditable {
        let xs = columnas_de(&tabla);
        HojaEditable {
            nombre,
            tabla,
            id_mensaje,
            tocada: false,
            sel: Seleccion::en(Ref { columna: 0, fila: 0 }),
            x: 0.0,
            y: 0.0,
            xs,
            valores: OnceCell::new(),
        }
    }

    /// Donde empieza la columna `c` (px al 100 %).
    pub fn x_de(&self, c: u32) -> f32 {
        let c = (c as usize).min(self.xs.len() - 1);
        self.xs[c]
    }

    pub fn ancho_de(&self, c: u32) -> f32 {
        self.x_de(c + 1) - self.x_de(c)
    }

    pub fn y_de(f: u32) -> f32 {
        f as f32 * ALTO_FILA as f32
    }

    /// La columna en `x` (px de hoja al 100 %).
    pub fn columna_en(&self, x: f32) -> u32 {
        if x <= 0.0 {
            return 0;
        }
        let i = self.xs.partition_point(|&c| c <= x);
        (i.saturating_sub(1) as u32).min(MAX_COLS - 1)
    }

    pub fn fila_en(y: f32) -> u32 {
        ((y.max(0.0) / ALTO_FILA as f32) as u32).min(MAX_FILAS - 1)
    }

    /// Lo que se ve en la celda: el resultado de su formula, con el
    /// separador decimal del usuario.
    pub fn mostrado(&self, r: Ref, decimal: char) -> String {
        let clave = ref_a(r);
        if !self.tabla.celdas.contains_key(&clave) {
            return String::new();
        }
        self.valores
            .get_or_init(|| formula::evaluar_todo(&self.tabla))
            .get(&clave)
            .map(|v| v.mostrar(decimal))
            .unwrap_or_default()
    }

    /// Si el valor de la celda es un numero (va a la derecha, como en Excel).
    pub fn es_numero(&self, r: Ref) -> bool {
        let clave = ref_a(r);
        self.tabla.celdas.contains_key(&clave)
            && matches!(self.valores.get_or_init(|| formula::evaluar_todo(&self.tabla)).get(&clave), Some(Valor::Numero(_)))
    }

    /// Mueve la vista lo justo para que se vea la celda activa, con
    /// `visible` (ancho, alto) en pixeles de hoja al 100 %.
    pub fn seguir(&mut self, (ancho, alto): (f32, f32)) {
        let a = self.sel.activa;
        let (x0, x1) = (self.x_de(a.columna), self.x_de(a.columna + 1));
        if x0 < self.x {
            self.x = x0;
        } else if x1 > self.x + ancho {
            self.x = (x1 - ancho).min(x0);
        }
        let (y0, y1) = (Self::y_de(a.fila), Self::y_de(a.fila + 1));
        if y0 < self.y {
            self.y = y0;
        } else if y1 > self.y + alto {
            self.y = (y1 - alto).min(y0);
        }
    }

    /// Los valores, si ya se calcularon (para no repetir la cuenta al pasar
    /// la hoja al visor).
    pub fn valores_hechos(&self) -> Option<&BTreeMap<String, Valor>> {
        self.valores.get()
    }

    fn olvidar_valores(&mut self) {
        self.valores = OnceCell::new();
    }
}

/// Donde empieza cada columna con los anchos de la tabla.
fn columnas_de(t: &Tabla) -> Vec<f32> {
    let mut anchos = vec![ANCHO_COLUMNA as f32; MAX_COLS as usize];
    for (letra, px) in &t.anchos {
        if let Some(r) = ref_de(&format!("{letra}1"))
            && *px > 0
            && let Some(a) = anchos.get_mut(r.columna as usize)
        {
            *a = *px as f32;
        }
    }
    let mut xs = Vec::with_capacity(anchos.len() + 1);
    let mut x = 0.0;
    xs.push(0.0);
    for a in anchos {
        x += a;
        xs.push(x);
    }
    xs
}

/// Las celdas de `t` dentro de las esquinas, con su contenido y su estilo.
fn fotografiar(t: &Tabla, (c0, f0, c1, f1): (u32, u32, u32, u32)) -> BTreeMap<String, Foto> {
    let dentro = |k: &String| ref_de(k).is_some_and(|r| (c0..=c1).contains(&r.columna) && (f0..=f1).contains(&r.fila));
    let mut m: BTreeMap<String, Foto> = BTreeMap::new();
    for (k, v) in t.celdas.iter().filter(|(k, _)| dentro(k)) {
        m.entry(k.clone()).or_default().0 = Some(v.clone());
    }
    for (k, e) in t.estilos.iter().filter(|(k, _)| dentro(k)) {
        m.entry(k.clone()).or_default().1 = Some(e.clone());
    }
    m
}

fn poner_foto(t: &mut Tabla, clave: &str, (celda, estilo): &Foto) {
    match celda {
        Some(c) => {
            t.celdas.insert(clave.to_string(), c.clone());
        }
        None => {
            t.celdas.remove(clave);
        }
    }
    match estilo {
        Some(e) => {
            t.estilos.insert(clave.to_string(), e.clone());
        }
        None => {
            t.estilos.remove(clave);
        }
    }
}

/// La alineacion de una celda (el `a` del movil).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alineacion {
    Izquierda,
    Centro,
    Derecha,
}

impl Alineacion {
    fn letra(self) -> &'static str {
        match self {
            Alineacion::Izquierda => "i",
            Alineacion::Centro => "c",
            Alineacion::Derecha => "d",
        }
    }
}

/// **El editor**: las hojas, la activa, lo que se esta tecleando y las
/// pilas de deshacer y rehacer.
pub struct Editor {
    pub hojas: Vec<HojaEditable>,
    pub activa: usize,
    /// Lo que se teclea en la celda activa. `None`: elegida, sin escribir
    /// (las flechas andan por la hoja).
    pub edicion: Option<String>,
    deshacer: Vec<Cambio>,
    rehacer: Vec<Cambio>,
}

impl Editor {
    /// Un editor con estas hojas. Sin ninguna no hay nada que editar.
    pub fn nuevo(hojas: Vec<HojaEditable>, activa: usize) -> Option<Editor> {
        if hojas.is_empty() {
            return None;
        }
        let activa = activa.min(hojas.len() - 1);
        Some(Editor { hojas, activa, edicion: None, deshacer: Vec::new(), rehacer: Vec::new() })
    }

    pub fn hoja(&self) -> &HojaEditable {
        &self.hojas[self.activa]
    }

    pub fn hoja_mut(&mut self) -> &mut HojaEditable {
        &mut self.hojas[self.activa]
    }

    #[cfg(test)]
    pub fn puede_deshacer(&self) -> bool {
        !self.deshacer.is_empty()
    }

    #[cfg(test)]
    pub fn puede_rehacer(&self) -> bool {
        !self.rehacer.is_empty()
    }

    /// Si alguna hoja cambio desde que se guardo.
    pub fn tocado(&self) -> bool {
        self.hojas.iter().any(|h| h.tocada)
    }

    /// Lo que se ve en la barra de formulas: lo que se teclea o lo escrito
    /// tal cual (la FORMULA, no su resultado).
    pub fn en_la_barra(&self) -> &str {
        match &self.edicion {
            Some(t) => t,
            None => self.hoja().tabla.celda(self.hoja().sel.activa),
        }
    }

    /// **Hace un cambio en las celdas de `esquinas`** de la hoja activa y lo
    /// apunta para deshacerlo. `false` si no cambio nada.
    fn cambiar(&mut self, esquinas: (u32, u32, u32, u32), f: impl FnOnce(&mut Tabla)) -> bool {
        let activa = self.activa;
        let h = &mut self.hojas[activa];
        let antes = fotografiar(&h.tabla, esquinas);
        let sel = h.sel;
        f(&mut h.tabla);
        let despues = fotografiar(&h.tabla, esquinas);
        let mut celdas = Vec::new();
        for k in antes.keys().chain(despues.keys().filter(|k| !antes.contains_key(*k))) {
            let a = antes.get(k).cloned().unwrap_or_default();
            let d = despues.get(k).cloned().unwrap_or_default();
            if a != d {
                celdas.push((k.clone(), a, d));
            }
        }
        if celdas.is_empty() {
            return false;
        }
        h.tocada = true;
        h.olvidar_valores();
        self.deshacer.push(Cambio { hoja: activa, celdas, sel });
        if self.deshacer.len() > MAXIMO_DESHACER {
            self.deshacer.remove(0);
        }
        self.rehacer.clear();
        true
    }

    /// Deja escrito lo que se tecleaba. Escribir lo mismo que habia no
    /// cuenta como cambio (no se guarda ni se apunta).
    pub fn confirmar(&mut self) -> bool {
        let Some(texto) = self.edicion.take() else { return false };
        let texto = texto.trim().to_string();
        let r = self.hoja().sel.activa;
        self.cambiar((r.columna, r.fila, r.columna, r.fila), |t| t.poner(r, &texto))
    }

    /// Escapar con algo tecleado deja la celda como estaba.
    pub fn cancelar(&mut self) {
        self.edicion = None;
    }

    /// Empieza a escribir en la celda activa: con lo que tiene (F2, doble
    /// clic) o de cero (teclear encima, como en cualquier hoja).
    pub fn empezar(&mut self, con_lo_que_hay: bool) {
        if self.edicion.is_none() {
            let r = self.hoja().sel.activa;
            self.edicion = Some(if con_lo_que_hay { self.hoja().tabla.celda(r).to_string() } else { String::new() });
        }
    }

    pub fn teclear(&mut self, c: char) {
        self.edicion.get_or_insert_with(String::new).push(c);
    }

    /// Retroceso: borra una letra de lo tecleado; sin nada tecleado, vacia
    /// la celda para escribir de cero (como el chat).
    pub fn retroceso(&mut self) {
        match self.edicion.as_mut() {
            Some(t) => {
                t.pop();
            }
            None => self.edicion = Some(String::new()),
        }
    }

    /// Suprimir: vacia lo elegido (contenido; el estilo se queda, como en
    /// Excel). `false` si no habia nada o es demasiado.
    pub fn borrar_seleccion(&mut self) -> bool {
        self.edicion = None;
        let sel = self.hoja().sel;
        if sel.celdas() > MAXIMO_RANGO {
            return false;
        }
        let esquinas = sel.esquinas();
        self.cambiar(esquinas, |t| {
            let (c0, f0, c1, f1) = esquinas;
            t.celdas.retain(|k, _| !ref_de(k).is_some_and(|r| (c0..=c1).contains(&r.columna) && (f0..=f1).contains(&r.fila)));
        })
    }

    /// Mueve la celda activa (confirmando lo tecleado); con `extender`,
    /// alarga el rango desde el ancla.
    pub fn mover(&mut self, dx: i32, dy: i32, extender: bool) {
        self.confirmar();
        let h = self.hoja_mut();
        let a = h.sel.activa;
        let r = Ref {
            columna: (a.columna as i64 + dx as i64).clamp(0, MAX_COLS as i64 - 1) as u32,
            fila: (a.fila as i64 + dy as i64).clamp(0, MAX_FILAS as i64 - 1) as u32,
        };
        h.sel = if extender { Seleccion { ancla: h.sel.ancla, activa: r } } else { Seleccion::en(r) };
    }

    /// Elige una celda (un clic); con `extender` (Mayus), alarga el rango.
    pub fn elegir(&mut self, r: Ref, extender: bool) {
        self.confirmar();
        let h = self.hoja_mut();
        h.sel = if extender { Seleccion { ancla: h.sel.ancla, activa: r } } else { Seleccion::en(r) };
    }

    /// Todo lo escrito de la hoja activa.
    pub fn elegir_todo(&mut self) {
        self.confirmar();
        let (c, f) = self.hoja().tabla.tamano();
        let h = self.hoja_mut();
        h.sel = Seleccion { ancla: Ref { columna: 0, fila: 0 }, activa: Ref { columna: c.saturating_sub(1), fila: f.saturating_sub(1) } };
    }

    /// Cambia de hoja (confirmando lo tecleado en la que se deja).
    pub fn cambiar_hoja(&mut self, i: usize) {
        if i < self.hojas.len() && i != self.activa {
            self.confirmar();
            self.activa = i;
        }
    }

    /// **Negrita** en lo elegido: si todo lo elegido ya la tiene, se quita;
    /// si no, se pone (como Excel y el movil).
    pub fn alternar_negrita(&mut self) -> bool {
        self.confirmar();
        let sel = self.hoja().sel;
        if sel.celdas() > MAXIMO_RANGO {
            return false;
        }
        let (c0, f0, c1, f1) = sel.esquinas();
        let t = &self.hoja().tabla;
        let todas = (f0..=f1).all(|f| (c0..=c1).all(|c| t.estilos.get(&ref_a(Ref { columna: c, fila: f })).is_some_and(|e| e.n)));
        self.estilo_en_rango(|e| e.n = !todas)
    }

    /// La alineacion de lo elegido; la misma otra vez la quita (vuelve a la
    /// que toque por el valor).
    pub fn alinear(&mut self, a: Alineacion) -> bool {
        self.confirmar();
        let sel = self.hoja().sel;
        if sel.celdas() > MAXIMO_RANGO {
            return false;
        }
        let ya = self.hoja().tabla.estilos.get(&ref_a(sel.activa)).and_then(|e| e.a.as_deref()) == Some(a.letra());
        self.estilo_en_rango(|e| e.a = if ya { None } else { Some(a.letra().to_string()) })
    }

    fn estilo_en_rango(&mut self, f: impl Fn(&mut EstiloDeCelda)) -> bool {
        let esquinas = self.hoja().sel.esquinas();
        self.cambiar(esquinas, |t| {
            let (c0, f0, c1, f1) = esquinas;
            for fila in f0..=f1 {
                for columna in c0..=c1 {
                    let k = ref_a(Ref { columna, fila });
                    let e = t.estilos.entry(k.clone()).or_default();
                    f(e);
                    if e.vacio() {
                        t.estilos.remove(&k);
                    }
                }
            }
        })
    }

    /// **Pega** unas filas de celdas con la esquina en la celda activa
    /// (`portapapeles_tabla::pegar`), como un solo cambio. Escribiendo, el
    /// texto va dentro de lo que se escribe.
    pub fn pegar(&mut self, filas: &[Vec<pixpin_proyecto::portapapeles_tabla::Pegada>]) -> bool {
        let en = self.hoja().sel.activa;
        let alto = filas.len() as u32;
        let ancho = filas.iter().map(|f| f.len()).max().unwrap_or(0) as u32;
        if alto == 0 || ancho == 0 {
            return false;
        }
        let esquinas = (en.columna, en.fila, (en.columna + ancho - 1).min(MAX_COLS - 1), (en.fila + alto - 1).min(MAX_FILAS - 1));
        let hecho = self.cambiar(esquinas, |t| {
            pixpin_proyecto::portapapeles_tabla::pegar(t, en, filas);
        });
        // Lo pegado queda elegido, como en Excel.
        let h = self.hoja_mut();
        h.sel = Seleccion { ancla: en, activa: Ref { columna: esquinas.2, fila: esquinas.3 } };
        hecho
    }

    /// **Deshace** el ultimo cambio: vuelve a su hoja y a su seleccion.
    pub fn deshacer(&mut self) -> bool {
        self.edicion = None;
        let Some(c) = self.deshacer.pop() else { return false };
        self.aplicar(&c, false);
        self.rehacer.push(c);
        true
    }

    pub fn rehacer(&mut self) -> bool {
        self.edicion = None;
        let Some(c) = self.rehacer.pop() else { return false };
        self.aplicar(&c, true);
        self.deshacer.push(c);
        true
    }

    fn aplicar(&mut self, c: &Cambio, adelante: bool) {
        let Some(h) = self.hojas.get_mut(c.hoja) else { return };
        for (k, antes, despues) in &c.celdas {
            poner_foto(&mut h.tabla, k, if adelante { despues } else { antes });
        }
        h.tocada = true;
        h.olvidar_valores();
        h.sel = c.sel;
        self.activa = c.hoja;
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn r(t: &str) -> Ref {
        ref_de(t).unwrap()
    }

    fn editor() -> Editor {
        let mut t = Tabla::default();
        t.poner(r("A1"), "Concepto");
        t.poner(r("B2"), "5");
        t.poner(r("B3"), "=B2*2");
        let otra = Tabla::default();
        Editor::nuevo(
            vec![HojaEditable::nueva("Gastos".into(), t, Some("1".into())), HojaEditable::nueva("Resumen".into(), otra, Some("2".into()))],
            0,
        )
        .unwrap()
    }

    #[test]
    fn escribir_deshacer_y_rehacer_celda_a_celda() {
        let mut e = editor();
        e.elegir(r("B2"), false);
        e.teclear('7');
        e.mover(0, 1, false);
        assert_eq!(e.hoja().tabla.celda(r("B2")), "7");
        assert_eq!(e.hoja().mostrado(r("B3"), '.'), "14", "la formula se recalcula");
        assert!(e.hoja().tocada);
        e.elegir(r("C1"), false);
        e.teclear('x');
        e.confirmar();
        assert!(e.deshacer());
        assert_eq!(e.hoja().tabla.celda(r("C1")), "", "deshacer quita lo ultimo");
        assert!(e.deshacer());
        assert_eq!(e.hoja().tabla.celda(r("B2")), "5");
        assert_eq!(e.hoja().mostrado(r("B3"), '.'), "10");
        assert_eq!(e.hoja().sel.activa, r("B2"), "deshacer vuelve a donde se escribio");
        // Caso negativo: no hay nada mas que deshacer.
        assert!(!e.deshacer());
        assert!(e.rehacer());
        assert_eq!(e.hoja().tabla.celda(r("B2")), "7");
        assert!(e.rehacer());
        assert_eq!(e.hoja().tabla.celda(r("C1")), "x");
        assert!(!e.rehacer());
    }

    #[test]
    fn escribir_lo_mismo_no_es_un_cambio_y_uno_nuevo_borra_lo_rehecho() {
        let mut e = editor();
        e.elegir(r("A1"), false);
        e.empezar(true);
        assert_eq!(e.en_la_barra(), "Concepto");
        assert!(!e.confirmar(), "lo mismo que habia no cuenta");
        assert!(!e.puede_deshacer());
        assert!(!e.hoja().tocada);
        e.teclear('a');
        e.confirmar();
        e.deshacer();
        assert!(e.puede_rehacer());
        e.teclear('b');
        e.confirmar();
        assert!(!e.puede_rehacer(), "un cambio nuevo tira lo que se podia rehacer");
        // Escapar no escribe nada.
        e.teclear('z');
        e.cancelar();
        assert_eq!(e.hoja().tabla.celda(r("A1")), "b");
    }

    #[test]
    fn deshacer_lleva_a_la_hoja_del_cambio() {
        let mut e = editor();
        e.cambiar_hoja(1);
        e.elegir(r("D4"), false);
        e.teclear('1');
        e.confirmar();
        e.cambiar_hoja(0);
        assert!(e.deshacer());
        assert_eq!(e.activa, 1);
        assert_eq!(e.hojas[1].tabla.celda(r("D4")), "");
        assert_eq!(e.hojas[0].tabla.celda(r("A1")), "Concepto", "la otra hoja no se toca");
    }

    #[test]
    fn negrita_alinear_y_borrar_un_rango_son_un_solo_cambio() {
        let mut e = editor();
        e.elegir(r("A1"), false);
        e.mover(1, 2, true);
        assert_eq!(e.hoja().sel.esquinas(), (0, 0, 1, 2));
        assert!(e.alternar_negrita());
        assert!(e.hoja().tabla.estilos["B3"].n);
        assert_eq!(e.hoja().tabla.estilos.len(), 6);
        assert!(e.alternar_negrita(), "todo en negrita: se quita");
        assert!(e.hoja().tabla.estilos.is_empty(), "un estilo vacio no se guarda");
        assert!(e.alinear(Alineacion::Centro));
        assert_eq!(e.hoja().tabla.estilos["A1"].a.as_deref(), Some("c"));
        assert!(e.borrar_seleccion());
        assert!(e.hoja().tabla.celdas.is_empty());
        assert_eq!(e.hoja().tabla.estilos.len(), 6, "borrar deja el formato");
        assert!(e.deshacer());
        assert_eq!(e.hoja().tabla.celdas.len(), 3, "un deshacer devuelve el rango entero");
        // Caso negativo: un rango gigante no se toca.
        e.elegir(r("A1"), false);
        e.elegir(r("ZZ100000"), true);
        assert!(!e.borrar_seleccion());
        assert!(!e.alternar_negrita());
    }

    #[test]
    fn pegar_se_deshace_de_una_vez() {
        use pixpin_proyecto::portapapeles_tabla::Pegada;
        let mut e = editor();
        e.elegir(r("B2"), false);
        let p = |t: &str| Pegada { texto: t.into(), ..Default::default() };
        assert!(e.pegar(&[vec![p("1"), p("2")], vec![p("3")]]));
        assert_eq!(e.hoja().tabla.celda(r("C2")), "2");
        assert_eq!(e.hoja().sel.esquinas(), (1, 1, 2, 2));
        assert!(e.deshacer());
        assert_eq!(e.hoja().tabla.celda(r("B2")), "5");
        assert_eq!(e.hoja().tabla.celda(r("C2")), "");
        assert!(!e.pegar(&[]), "nada que pegar");
    }

    #[test]
    fn las_columnas_siguen_los_anchos_de_la_tabla() {
        let mut t = Tabla::default();
        t.anchos.insert("B".into(), 180);
        let h = HojaEditable::nueva("x".into(), t, None);
        assert_eq!(h.x_de(1), ANCHO_COLUMNA as f32);
        assert_eq!(h.ancho_de(1), 180.0);
        assert_eq!(h.columna_en(ANCHO_COLUMNA as f32 + 179.0), 1);
        assert_eq!(h.columna_en(ANCHO_COLUMNA as f32 + 180.0), 2);
        assert_eq!(h.columna_en(-5.0), 0);
        assert_eq!(HojaEditable::fila_en(ALTO_FILA as f32 * 3.5), 3);
        // Caso negativo: no se sale de la hoja.
        assert_eq!(h.columna_en(1.0e9), MAX_COLS - 1);
        assert_eq!(HojaEditable::fila_en(1.0e12), MAX_FILAS - 1);
        assert!(Editor::nuevo(Vec::new(), 0).is_none(), "sin hojas no hay editor");
    }

    #[test]
    fn la_vista_sigue_a_la_celda_activa_lo_justo() {
        let mut h = HojaEditable::nueva("x".into(), Tabla::default(), None);
        let vis = (ANCHO_COLUMNA as f32 * 3.0, ALTO_FILA as f32 * 10.0);
        h.sel = Seleccion::en(r("B5"));
        h.seguir(vis);
        assert_eq!((h.x, h.y), (0.0, 0.0), "lo que ya se ve no mueve nada");
        h.sel = Seleccion::en(r("E20"));
        h.seguir(vis);
        assert_eq!(h.x, h.x_de(5) - vis.0, "la E queda en el borde derecho");
        assert_eq!(h.y, HojaEditable::y_de(20) - vis.1);
        h.sel = Seleccion::en(r("A1"));
        h.seguir(vis);
        assert_eq!((h.x, h.y), (0.0, 0.0), "y vuelve al principio");
    }
}
