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

use std::mem::size_of;

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
    historia: Vec<Paso>,
    #[serde(skip)]
    rehacer: Vec<Paso>,
    /// El paso que se esta construyendo, entre `abrir_paso` y `cerrar_paso`.
    #[serde(skip)]
    en_curso: Option<Paso>,
    /// Lo que ocupan `historia` y `rehacer`, para no tener que recorrerlos.
    #[serde(skip)]
    bytes_historial: usize,
}

/// Un cambio suelto. Cada uno sabe invertirse.
#[derive(Debug, Clone, PartialEq)]
enum Cambio {
    Anadido(u64),
    Borrado(u64),
    /// El elemento entero **antes** del cambio.
    ///
    /// Guardar el estado anterior y no la operacion inversa es D24, y no es
    /// preferencia de estilo: en coma flotante `(a * 1.5) / 1.5` no siempre
    /// devuelve `a`. Con la operacion inversa, treinta ciclos de deshacer y
    /// rehacer deforman el dibujo poco a poco — un fallo que aparece en
    /// casa del usuario y no en las pruebas.
    Editado {
        id: u64,
        antes: Box<Elemento>,
    },
}

/// Todo lo que hizo un gesto. Un arrastre que mueve cuarenta elementos es
/// **un** paso de deshacer, no cuarenta.
#[derive(Debug, Clone, PartialEq, Default)]
struct Paso {
    cambios: Vec<Cambio>,
}

impl Cambio {
    /// Lo que ocupa de cara al techo del historial.
    fn bytes(&self) -> usize {
        match self {
            Cambio::Anadido(_) | Cambio::Borrado(_) => size_of::<Cambio>(),
            Cambio::Editado { antes, .. } => size_of::<Cambio>() + antes.bytes(),
        }
    }
}

/// Techo del historial (D25). Ocho megas son quinientos arrastres de un
/// trazo de 492 puntos —mas de lo que nadie deshace de una sentada— y una
/// fraccion asumible de los ~1,5 GB que Windows 10 deja libres en la
/// maquina suelo de 4 GB.
const TECHO_HISTORIAL: usize = 8 * 1024 * 1024;

fn bytes_de(paso: &Paso) -> usize {
    paso.cambios.iter().map(Cambio::bytes).sum()
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
            en_curso: None,
            bytes_historial: 0,
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
        self.apuntar_anadido(id);
        id
    }

    /// Mete un cambio suelto en el paso en curso, o le crea uno propio si no
    /// hay ninguno abierto. Hacer algo nuevo fuera de un paso corta la rama
    /// de rehacer, como en cualquier editor.
    fn empujar_cambio(&mut self, cambio: Cambio) {
        match &mut self.en_curso {
            Some(paso) => paso.cambios.push(cambio),
            None => {
                let paso = Paso {
                    cambios: vec![cambio],
                };
                self.bytes_historial += bytes_de(&paso);
                self.historia.push(paso);
                self.rehacer.clear();
            }
        }
    }

    fn apuntar_anadido(&mut self, id: u64) {
        self.empujar_cambio(Cambio::Anadido(id));
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
    ///
    /// El elemento ya esta en su posicion final cuando se llama: el "antes"
    /// se reconstruye desplazandolo al reves, que para una traslacion (una
    /// suma) es exacto, a diferencia de una escala (D24).
    pub fn apuntar_movimiento(&mut self, id: u64, dx: f32, dy: f32) {
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        let Some(e) = self.buscar(id) else { return };
        let mut antes = e.clone();
        antes.mover(-dx, -dy);
        self.empujar_cambio(Cambio::Editado {
            id,
            antes: Box::new(antes),
        });
    }

    /// Borrado a peticion del usuario (el borrador, la tecla Suprimir): se
    /// apunta para poder deshacerlo.
    pub fn borrar_apuntando(&mut self, id: u64) -> bool {
        if self.borrar(id) {
            self.empujar_cambio(Cambio::Borrado(id));
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

    /// Empieza un gesto. Todo lo que pase hasta `cerrar_paso` sera un solo
    /// paso de deshacer.
    ///
    /// Abrir dos veces sin cerrar no es error: el segundo abrir no hace
    /// nada. La ventana puede recibir un `WM_LBUTTONDOWN` sin su
    /// `WM_LBUTTONUP` si el usuario suelta fuera, y perder el gesto entero
    /// por eso seria peor que ignorarlo.
    pub fn abrir_paso(&mut self) {
        if self.en_curso.is_none() {
            self.en_curso = Some(Paso::default());
        }
    }

    /// Guarda el estado actual de un elemento para poder volver a el.
    ///
    /// Apuntarlo dos veces dentro del mismo paso guarda **solo la primera**:
    /// mover el raton produce cien avisos por gesto, y cien instantaneas de
    /// un trazo largo se comerian el techo en un solo arrastre.
    pub fn apuntar_edicion(&mut self, id: u64) {
        let Some(paso) = &self.en_curso else { return };
        let ya_esta = paso
            .cambios
            .iter()
            .any(|c| matches!(c, Cambio::Editado { id: i, .. } if *i == id));
        if ya_esta {
            return;
        }
        let Some(e) = self.buscar(id) else { return };
        let cambio = Cambio::Editado {
            id,
            antes: Box::new(e.clone()),
        };
        if let Some(paso) = &mut self.en_curso {
            paso.cambios.push(cambio);
        }
    }

    /// Cierra el gesto. Un paso sin cambios no entra en el historial: hacer
    /// clic sin arrastrar no debe consumir un `Ctrl+Z`.
    pub fn cerrar_paso(&mut self) {
        let Some(paso) = self.en_curso.take() else {
            return;
        };
        if paso.cambios.is_empty() {
            return;
        }
        self.bytes_historial += bytes_de(&paso);
        self.historia.push(paso);
        for p in self.rehacer.drain(..) {
            self.bytes_historial -= bytes_de(&p);
        }
        self.podar();
    }

    /// Deshace el gesto en curso sin apuntarlo. Es lo que hace `Escape` a
    /// mitad de un arrastre, y sale gratis porque el paso ya guarda el
    /// estado anterior de lo que se estaba tocando.
    pub fn cancelar_paso(&mut self) {
        let Some(paso) = self.en_curso.take() else {
            return;
        };
        self.aplicar_inverso(&paso);
    }

    /// Deshace el ultimo gesto. Devuelve si habia algo que deshacer.
    pub fn deshacer(&mut self) -> bool {
        let Some(paso) = self.historia.pop() else {
            return false;
        };
        self.bytes_historial -= bytes_de(&paso);
        let inverso = self.aplicar_inverso(&paso);
        self.bytes_historial += bytes_de(&inverso);
        self.rehacer.push(inverso);
        true
    }

    /// Rehace el ultimo gesto deshecho.
    pub fn rehacer(&mut self) -> bool {
        let Some(paso) = self.rehacer.pop() else {
            return false;
        };
        self.bytes_historial -= bytes_de(&paso);
        let inverso = self.aplicar_inverso(&paso);
        self.bytes_historial += bytes_de(&inverso);
        self.historia.push(inverso);
        true
    }

    /// Aplica el paso al reves y devuelve el paso que lo desharia.
    ///
    /// Que devuelva su propio inverso es lo que hace que deshacer y rehacer
    /// sean la misma funcion. Con dos funciones distintas, acabarian
    /// discrepando en cuanto se anadiera una variante a `Cambio`.
    fn aplicar_inverso(&mut self, paso: &Paso) -> Paso {
        let mut inverso = Paso::default();
        // Al reves: si un gesto borro y luego anadio, deshacerlo tiene que
        // quitar lo anadido antes de restaurar lo borrado.
        for c in paso.cambios.iter().rev() {
            match c {
                Cambio::Anadido(id) => {
                    self.borrar(*id);
                    inverso.cambios.push(Cambio::Borrado(*id));
                }
                Cambio::Borrado(id) => {
                    self.restaurar(*id);
                    inverso.cambios.push(Cambio::Anadido(*id));
                }
                Cambio::Editado { id, antes } => {
                    if let Some(i) = self.elementos.iter().position(|e| e.id == *id) {
                        let ahora = self.elementos[i].clone();
                        self.elementos[i] = (**antes).clone();
                        inverso.cambios.push(Cambio::Editado {
                            id: *id,
                            antes: Box::new(ahora),
                        });
                    }
                }
            }
        }
        inverso
    }

    /// Tira los pasos mas antiguos hasta caber en el techo (D25).
    ///
    /// El techo va en memoria y no en numero de pasos porque quinientos
    /// pasos son ocho kilobytes o doscientos megas segun lo que se haya
    /// tocado, y en un equipo de 4 GB eso no se puede prometer.
    fn podar(&mut self) {
        while self.bytes_historial > TECHO_HISTORIAL && self.historia.len() > 1 {
            let viejo = self.historia.remove(0);
            self.bytes_historial -= bytes_de(&viejo);
        }
    }

    /// Lo que ocupa el historial ahora mismo.
    pub fn bytes_de_historial(&self) -> usize {
        self.bytes_historial
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

    fn base() -> Elemento {
        Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 50.0,
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
        }
    }

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

        assert!(e.deshacer());
        assert_eq!(
            (e.buscar(id).unwrap().x, e.buscar(id).unwrap().y),
            (10.0, 0.0),
            "el arrastre entero se deshace de una vez"
        );
        assert!(!e.buscar(id).unwrap().borrado, "deshacer mover no borra");

        assert!(e.rehacer());
        assert_eq!(
            (e.buscar(id).unwrap().x, e.buscar(id).unwrap().y),
            (40.0, 10.0)
        );

        // Y el siguiente deshacer se lleva el alta, que es el paso anterior.
        e.deshacer();
        assert!(e.deshacer());
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
        assert!(!e.rehacer(), "el rehacer viejo ya no vale");
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

        assert!(e.deshacer(), "deshace el ultimo");
        assert_eq!(e.cuantos_visibles(), 1);
        assert!(e.buscar(segundo).unwrap().borrado);
        assert!(e.rehacer());
        assert_eq!(e.cuantos_visibles(), 2);
        assert!(!e.buscar(segundo).unwrap().borrado);
    }

    #[test]
    fn deshacer_sobre_una_escena_vacia_no_entra_en_panico() {
        let mut e = Escena::nueva();
        assert!(!e.deshacer());
        assert!(!e.rehacer());
    }

    #[test]
    fn deshacer_dos_veces_deshace_dos_elementos_distintos() {
        // Caso negativo del anterior: una implementacion que solo mirase el
        // ultimo de la lista deshareria siempre el mismo.
        let mut e = Escena::nueva();
        let uno = e.anadir(rect(0.0));
        let dos = e.anadir(rect(100.0));
        assert!(e.deshacer());
        assert!(e.buscar(dos).unwrap().borrado);
        assert!(e.deshacer());
        assert!(e.buscar(uno).unwrap().borrado);
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

    #[test]
    fn un_paso_agrupa_todos_los_cambios_del_gesto() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(base());
        let b = escena.anadir(base());
        let c = escena.anadir(base());

        escena.abrir_paso();
        for id in [a, b, c] {
            escena.apuntar_edicion(id);
            escena.buscar_mut(id).unwrap().mover(10.0, 0.0);
        }
        escena.cerrar_paso();

        assert!(escena.deshacer(), "tiene que haber algo que deshacer");
        assert_eq!(escena.buscar(a).unwrap().x, 0.0, "el primero vuelve");
        assert_eq!(escena.buscar(c).unwrap().x, 0.0, "y el tercero tambien");
        assert!(escena.deshacer(), "queda deshacer el haber anadido");
    }

    #[test]
    fn treinta_ciclos_de_deshacer_y_rehacer_no_deforman_el_dibujo() {
        // El motivo de D24. Con la operacion inversa en vez del estado
        // anterior, escalar por 1,5 y dividir por 1,5 acumularia error en coma
        // flotante hasta que el dibujo se nota torcido.
        let mut escena = Escena::nueva();
        let id = escena.anadir(base());
        let partida = escena.buscar(id).unwrap().clone();

        for _ in 0..30 {
            escena.abrir_paso();
            escena.apuntar_edicion(id);
            let e = escena.buscar_mut(id).unwrap();
            e.ancho *= 1.5;
            e.alto *= 1.5;
            e.angulo += 0.37;
            e.tocar();
            escena.cerrar_paso();

            assert!(escena.deshacer());
        }

        assert_eq!(
            escena.buscar(id).unwrap(),
            &partida,
            "identico bit a bit, no aproximado"
        );
    }

    #[test]
    fn cancelar_un_paso_deja_todo_como_estaba_y_no_ensucia_el_historial() {
        let mut escena = Escena::nueva();
        let id = escena.anadir(base());

        escena.abrir_paso();
        escena.apuntar_edicion(id);
        escena.buscar_mut(id).unwrap().mover(500.0, 500.0);
        escena.cancelar_paso();

        assert_eq!(escena.buscar(id).unwrap().x, 0.0, "vuelve a su sitio");
        assert!(escena.deshacer(), "queda el paso de haberlo anadido");
        assert!(
            !escena.deshacer(),
            "y nada mas: el arrastre cancelado no cuenta"
        );
    }

    #[test]
    fn un_gesto_sin_cambios_no_consume_un_ctrl_zeta() {
        let mut escena = Escena::nueva();
        escena.anadir(base());
        escena.abrir_paso();
        escena.cerrar_paso();

        assert!(escena.deshacer(), "el anadido");
        assert!(!escena.deshacer(), "el clic sin arrastrar no dejo paso");
    }

    #[test]
    fn apuntar_dos_veces_el_mismo_elemento_guarda_solo_el_estado_original() {
        // Mover el raton produce cien avisos por gesto. Si cada uno guardara
        // una instantanea, arrastrar un trazo largo se comeria el techo de
        // memoria en un solo arrastre.
        let mut escena = Escena::nueva();
        let id = escena.anadir(base());

        escena.abrir_paso();
        for _ in 0..100 {
            escena.apuntar_edicion(id);
            escena.buscar_mut(id).unwrap().mover(1.0, 0.0);
        }
        escena.cerrar_paso();

        assert!(escena.deshacer());
        assert_eq!(escena.buscar(id).unwrap().x, 0.0, "vuelve al origen entero");
    }

    #[test]
    fn el_historial_no_pasa_de_ocho_megas() {
        // D25: el techo va en memoria, no en numero de pasos.
        let mut escena = Escena::nueva();
        let puntos: Vec<Punto2> = (0..492)
            .map(|i| Punto2::nuevo(i as f32, (i * 2) as f32))
            .collect();
        let id = escena.anadir(Elemento {
            figura: Figura::Lapiz {
                puntos,
                presiones: Vec::new(),
            },
            ..base()
        });

        for _ in 0..500 {
            escena.abrir_paso();
            escena.apuntar_edicion(id);
            escena.buscar_mut(id).unwrap().mover(1.0, 0.0);
            escena.cerrar_paso();
        }

        assert!(
            escena.bytes_de_historial() <= 8 * 1024 * 1024,
            "el historial ocupa {} bytes",
            escena.bytes_de_historial()
        );
        assert!(escena.deshacer(), "y aun asi se deshace lo reciente");
    }
}
