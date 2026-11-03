//! **Las pegatinas de comentario en la hoja de calculo**
//! (`crate::comentarios`): el ancla es la hoja (`pagina`, desde 0) y un
//! punto en pixeles de la hoja al 100 % (96 ppp), las unidades de sus
//! marcos y de su tinta. Se ven las de la hoja abierta, sobre las celdas.

use pindesk_comentarios::{Ancla, Sitio};
use pindesk_geom::Rect;
use pindesk_render::{Pintor, RectF};
use pindesk_shell::overlay::EventoOverlay;

use super::Estado;
use super::marcos::Mapa;
use crate::comentarios::{Contexto, Pedido, Respuesta, Ubicar};

/// Las anclas de la hoja abierta con la vista de ahora.
pub(super) struct UbicarHoja {
    pub(super) mapa: Mapa,
    pub(super) celdas: RectF,
    pub(super) activa: u32,
}

fn dentro(r: &RectF, (x, y): (f32, f32)) -> bool {
    x >= r.x && x <= r.x + r.ancho && y >= r.y && y <= r.y + r.alto
}

impl Ubicar for UbicarHoja {
    fn a_pantalla(&self, a: &Ancla) -> Option<(f32, f32)> {
        let Sitio::Pagina { pagina, x, y } = a.sitio() else {
            return None;
        };
        if pagina != self.activa {
            return None;
        }
        let p = self.mapa.a_pantalla([x, y]);
        // Sobre los encabezados no se pinta: alli no hay celdas.
        dentro(&self.celdas, p).then_some(p)
    }

    fn de_pantalla(&self, p: (f32, f32)) -> Option<Ancla> {
        if !dentro(&self.celdas, p) {
            return None;
        }
        let [x, y] = self.mapa.a_hoja(p);
        Some(Ancla::pagina(self.activa, x, y))
    }
}

impl Estado<'_> {
    pub(super) fn ubicar_comentarios(&self) -> UbicarHoja {
        let (_, celdas) = self.vista();
        UbicarHoja {
            mapa: Mapa::de_abierta(self.abierta(), self.zonas().hoja),
            celdas,
            activa: self.activa as u32,
        }
    }

    fn contexto_de_comentarios<'a>(
        &self,
        ubicar: &'a UbicarHoja,
        area: Rect,
        dueno: isize,
    ) -> Contexto<'a> {
        let z = self.zonas();
        Contexto {
            area,
            e: self.e,
            // Encima de las pestanas de las hojas.
            reserva_abajo: (self.alto - z.pestanas.y).max(0.0),
            ubicar,
            ocupado: self.gesto.is_some(),
            dueno,
            atajo: None,
        }
    }

    /// **Las pegatinas, antes que la hoja**: devuelve si el evento era suyo.
    pub(super) fn comentar(&mut self, ev: &EventoOverlay, area: Rect, dueno: isize) -> Respuesta {
        if self.sin_interfaz || self.editando {
            return Respuesta::default();
        }
        let ubicar = self.ubicar_comentarios();
        let cx = self.contexto_de_comentarios(&ubicar, area, dueno);
        let Some(peg) = self.comentarios.as_mut() else {
            return Respuesta::default();
        };
        let r = peg.evento(ev, &cx);
        if let Some(Pedido::IrA(a)) = &r.pedido
            && let Sitio::Pagina { pagina, x, y } = a.sitio()
            && pagina as usize == self.activa
        {
            // Con el aumento de ahora, su punto en el centro de las celdas.
            let (_, celdas) = self.vista();
            let ab = self.abierta_mut();
            ab.x = x - celdas.ancho as f64 / 2.0 / ab.zoom;
            ab.y = y - celdas.alto as f64 / 2.0 / ab.zoom;
        }
        r
    }

    /// Lo que llego de otro aparato. `true` si hay que repintar.
    pub(super) fn comentarios_al_dia(&mut self) -> bool {
        self.comentarios.as_mut().is_some_and(|p| p.al_dia())
    }

    /// Las pegatinas de la hoja y el boton, la tira, el menu y la lista.
    pub(super) fn pintar_comentarios(&self, p: &Pintor) {
        let Some(peg) = &self.comentarios else {
            return;
        };
        let ubicar = self.ubicar_comentarios();
        p.con_recorte(ubicar.celdas, |p| peg.pintar_pegatinas(p, &ubicar, self.e));
        let area = Rect {
            x: 0,
            y: 0,
            ancho: self.ancho as u32,
            alto: self.alto as u32,
        };
        let cx = self.contexto_de_comentarios(&ubicar, area, 0);
        peg.pintar_interfaz(p, &cx, self.zonas().hoja.y + 8.0 * self.e);
    }
}
