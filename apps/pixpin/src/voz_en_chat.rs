//! **Lo que la voz tiene a medias dentro de una conversacion abierta**: la
//! caja de corregir la letra (B7), Pronunciar (B9) y la conversacion por
//! turnos (B10).
//!
//! Las tres corren en su propio hilo —una ventana propia o la caja de
//! texto de Windows— y devuelven lo suyo por un canal, como el telepronter.
//! El chat guarda aqui los tres extremos y, una vez por vuelta de su bucle,
//! pregunta si ha llegado algo ([`Pendiente::letra`],
//! [`Pendiente::toma`], [`Pendiente::conversacion`]). Nada de esto bloquea:
//! `try_recv` y vuelta.

use std::sync::mpsc::{Receiver, TryRecvError};

/// Los tres canales que puede tener abiertos una conversacion.
#[derive(Default)]
pub struct Pendiente {
    /// La caja de corregir la letra, con el id de la nota a la que vuelve.
    letra: Option<(String, Receiver<Option<String>>)>,
    pronunciar: Option<Receiver<crate::pronunciar::Toma>>,
    conversacion: Option<Receiver<crate::conversacion::Grabada>>,
}

/// Lo que da un canal al mirarlo: nada todavia, algo, o que se cerro.
enum Mirado<T> {
    Nada,
    Algo(T),
    Cerrado,
}

fn mirar<T>(r: &Receiver<T>) -> Mirado<T> {
    match r.try_recv() {
        Ok(v) => Mirado::Algo(v),
        Err(TryRecvError::Empty) => Mirado::Nada,
        Err(TryRecvError::Disconnected) => Mirado::Cerrado,
    }
}

impl Pendiente {
    /// Si hay algo que esperar: el chat vuelve a mirar cada poco aunque no
    /// entre ningun evento, porque lo que llega por estos canales no mueve
    /// el raton.
    pub fn espera(&self) -> bool {
        self.letra.is_some() || self.pronunciar.is_some() || self.conversacion.is_some()
    }

    /// Si la caja de la letra ya esta abierta (una a la vez).
    pub fn editando_letra(&self) -> bool {
        self.letra.is_some()
    }

    pub fn editar_letra(&mut self, id: String, r: Receiver<Option<String>>) {
        self.letra = Some((id, r));
    }

    /// La letra corregida y de que nota es, una sola vez. Cancelar la caja
    /// la cierra sin devolver nada.
    pub fn letra(&mut self) -> Option<(String, String)> {
        let (id, r) = self.letra.as_ref()?;
        match mirar(r) {
            Mirado::Nada => None,
            Mirado::Algo(texto) => {
                let id = id.clone();
                self.letra = None;
                texto.map(|t| (id, t))
            }
            Mirado::Cerrado => {
                self.letra = None;
                None
            }
        }
    }

    pub fn pronunciando(&self) -> bool {
        self.pronunciar.is_some()
    }

    pub fn pronunciar(&mut self, r: Receiver<crate::pronunciar::Toma>) {
        self.pronunciar = Some(r);
    }

    /// Una toma que el usuario guardo. Pronunciar puede guardar varias
    /// tomas antes de cerrarse, asi que el canal sigue abierto hasta que la
    /// ventana se cierra.
    pub fn toma(&mut self) -> Option<crate::pronunciar::Toma> {
        match mirar(self.pronunciar.as_ref()?) {
            Mirado::Nada => None,
            Mirado::Algo(t) => Some(t),
            Mirado::Cerrado => {
                self.pronunciar = None;
                None
            }
        }
    }

    pub fn conversando(&self) -> bool {
        self.conversacion.is_some()
    }

    pub fn conversar(&mut self, r: Receiver<crate::conversacion::Grabada>) {
        self.conversacion = Some(r);
    }

    /// La conversacion grabada, una sola vez.
    pub fn conversacion(&mut self) -> Option<crate::conversacion::Grabada> {
        match mirar(self.conversacion.as_ref()?) {
            Mirado::Nada => None,
            Mirado::Algo(g) => {
                self.conversacion = None;
                Some(g)
            }
            Mirado::Cerrado => {
                self.conversacion = None;
                None
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn la_letra_corregida_llega_una_vez_con_su_nota() {
        let mut p = Pendiente::default();
        assert!(!p.espera());
        let (tx, rx) = channel();
        p.editar_letra("m1".into(), rx);
        assert!(p.espera() && p.editando_letra());
        assert_eq!(p.letra(), None, "todavia escribiendo");
        tx.send(Some("nueva".into())).unwrap();
        assert_eq!(p.letra(), Some(("m1".into(), "nueva".into())));
        assert!(!p.espera(), "ya no hay nada que esperar");
        assert_eq!(p.letra(), None);
    }

    #[test]
    fn cancelar_o_cerrar_la_caja_no_devuelve_nada_y_la_suelta() {
        let mut p = Pendiente::default();
        let (tx, rx) = channel();
        p.editar_letra("m1".into(), rx);
        tx.send(None).unwrap();
        assert_eq!(p.letra(), None);
        assert!(!p.editando_letra());
        let (tx, rx) = channel::<Option<String>>();
        p.editar_letra("m2".into(), rx);
        drop(tx);
        assert_eq!(p.letra(), None);
        assert!(
            !p.editando_letra(),
            "un hilo muerto no deja la caja «abierta»"
        );
    }
}
