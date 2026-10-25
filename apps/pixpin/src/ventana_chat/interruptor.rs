//! **El interruptor Chat · Funciones** de la barra de titulo del chat.
//!
//! Antes tenia en medio «Proyectos», la pantalla de Proyectos del movil
//! dentro del chat; se quito el 25-oct-2026 (el usuario: «los proyectos no
//! sirven»). La segunda parte fue «Figuras», que abria su ventana aparte;
//! desde el 25-oct-2026 es «Funciones» y SI se queda elegida, como una
//! pestana: la columna de la lista ensena la rejilla de las funciones
//! (`funciones.rs`), con las figuras dentro. Las cuentas son de
//! `pixpin_ui::interruptor`; aqui se pinta y se apunta donde quedo, para
//! que el clic busque en lo mismo que se pinto.

use std::cell::Cell;

use pixpin_geom::Punto;
use pixpin_render::Pintor;
use pixpin_ui::interruptor as ui;

use super::{Tema, encoger, rf};

/// Las partes del interruptor de la barra de arriba.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Parte {
    Chat,
    /// La rejilla de las funciones (`funciones.rs`, 25-oct-2026).
    Funciones,
}

/// Donde quedo el interruptor en el ultimo fotograma (sin pintar, ninguno).
#[derive(Default)]
pub(super) struct Interruptor {
    pintado: Cell<Option<ui::Interruptor>>,
}

impl Interruptor {
    /// Que parte del interruptor hay bajo el punto. Pulsar una la elige,
    /// como un selector: pulsar «Chat» estando en el chat no cambia nada.
    pub(super) fn en(&self, l: Punto) -> Option<Parte> {
        let i = self.pintado.get()?;
        if i.funciones.contiene(l) {
            Some(Parte::Funciones)
        } else if i.caja.contiene(l) {
            Some(Parte::Chat)
        } else {
            None
        }
    }

    /// Lo pinta en la barra de titulo: un selector de dos con `elegida`
    /// puesta, como los de Windows 11.
    pub(super) fn pintar(
        &self,
        elegida: Parte,
        p: &Pintor,
        d: &pixpin_ui::chat::Disposicion,
        tema: &Tema,
        escala: u32,
        textos: &pixpin_store::Catalogo,
    ) {
        let e = escala as f32 / 100.0;
        let barra = d.barra;
        if barra.ancho == 0 {
            self.pintado.set(None);
            return;
        }
        // Desde donde acaba el nombre del programa hasta el primer boton de
        // la ventana (el de minimizar, el de mas a la izquierda).
        let (nombre, _) = p.medir_texto(&textos.t("app-nombre"), pixpin_ui::chat::TITULO_TAM * e);
        let desde = barra.x + (12.0 * e + nombre + 12.0 * e) as i32;
        let hasta = d
            .botones_barra(escala)
            .iter()
            .map(|(_, r)| r.x)
            .min()
            .unwrap_or(barra.derecha());
        let i = ui::interruptor(barra, desde, hasta, escala);
        self.pintado.set(Some(i));
        let radio = i.caja.alto as f32 / 2.0;
        p.rellenar_redondeado(rf(i.caja), radio, tema.boton_sobre);
        let puesta = match elegida {
            Parte::Chat => i.chat,
            Parte::Funciones => i.funciones,
        };
        p.rellenar_redondeado(
            encoger(rf(puesta), 2.0 * e),
            radio - 2.0 * e,
            tema.fila_elegida,
        );
        for (mitad, clave, elegida) in [
            (i.chat, "proyectos-interruptor-chat", elegida == Parte::Chat),
            (
                i.funciones,
                "proyectos-interruptor-funciones",
                elegida == Parte::Funciones,
            ),
        ] {
            let t = textos.t(clave);
            let tam = ui::INTERRUPTOR_TAM * e;
            let (w, h) = p.medir_texto(&t, tam);
            p.texto(
                &t,
                mitad.x as f32 + (mitad.ancho as f32 - w) / 2.0,
                mitad.y as f32 + (mitad.alto as f32 - h) / 2.0,
                tam,
                if elegida {
                    tema.texto_elegido
                } else {
                    tema.apagado
                },
            );
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_geom::Rect;

    #[test]
    fn el_interruptor_elige_la_mitad_que_se_pulsa() {
        let v = Interruptor::default();
        assert_eq!(
            v.en(Punto { x: 1, y: 1 }),
            None,
            "sin pintar no hay interruptor"
        );
        let i = ui::interruptor(
            Rect {
                x: 0,
                y: 0,
                ancho: 1000,
                alto: 24,
            },
            80,
            900,
            100,
        );
        v.pintado.set(Some(i));
        let centro = |r: Rect| Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        };
        assert_eq!(v.en(centro(i.chat)), Some(Parte::Chat));
        assert_eq!(v.en(centro(i.funciones)), Some(Parte::Funciones));
        assert_eq!(v.en(Punto { x: 5, y: 5 }), None);
    }
}
