//! **El riel se va con las barras** (27-oct-2026, `RielALaVista` del movil,
//! 57c1441a). El usuario: «al mover el contenido se esconden la barra de
//! arriba y la de abajo, pero la lateral no: que se esconda con ellas y
//! vuelva con ellas».
//!
//! Cada visor ya tenia su «a la vista»: la pastilla de arriba (y los mandos
//! de abajo) salen y se van solos a los pocos segundos, o la barra de los
//! pines sale con el raton encima y se va al salir. El riel (la isla de
//! marcar, anotar y enmarcar, y los puntos de los marcadores) pasa a seguir
//! ese mismo estado, en todos los visores, con esta regla:
//!
//! - **Con las barras a la vista**, se ve.
//! - **Mientras se usa** no se va: anotando, con los marcos, con la tira de
//!   emoticonos de un marcador abierta. Ahi el riel es la salida de lo que
//!   se esta haciendo, y esconderlo dejaria a uno sin poder dejar de anotar.
//! - **Con el raton cerca de su borde** (lo propio del PC: no hay dedo que
//!   toque la pantalla) vuelve aunque las barras sigan escondidas, y se
//!   queda mientras el raton este alli.
//!
//! Puro, para probarlo sin ventanas. Pixeles de la ventana del visor.

/// Lo que se mira, en pixeles logicos, desde el borde derecho: el riel mide
/// unos 44 y sus puntos crecen hacia dentro al pasar el raton.
pub const CERCA_DEL_BORDE: f32 = 96.0;

/// Si se esta usando el riel: lo que lo deja quieto.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EnUso {
    pub anotando: bool,
    pub marcos: bool,
    pub poniendo_marca: bool,
}

impl EnUso {
    pub fn alguno(self) -> bool {
        self.anotando || self.marcos || self.poniendo_marca
    }
}

/// **Si el raton esta cerca del borde del riel** (el derecho) de una vista
/// de `ancho` x `alto`, con la escala `e` (1.0 = 96 ppp).
pub fn raton_cerca(raton: (f32, f32), ancho: f32, alto: f32, e: f32) -> bool {
    let (x, y) = raton;
    x >= ancho - CERCA_DEL_BORDE * e && x <= ancho && y >= 0.0 && y <= alto
}

/// **Si el riel (y sus puntos) se ve**: con las barras, mientras se usa o
/// con el raton cerca de su borde.
pub fn visible(barras: bool, uso: EnUso, cerca: bool) -> bool {
    barras || uso.alguno() || cerca
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_va_con_las_barras_y_vuelve_con_ellas() {
        assert!(visible(true, EnUso::default(), false));
        assert!(!visible(false, EnUso::default(), false));
    }

    #[test]
    fn usandolo_se_queda_aunque_las_barras_se_vayan() {
        for uso in [
            EnUso {
                anotando: true,
                ..EnUso::default()
            },
            EnUso {
                marcos: true,
                ..EnUso::default()
            },
            EnUso {
                poniendo_marca: true,
                ..EnUso::default()
            },
        ] {
            assert!(visible(false, uso, false), "{uso:?}");
        }
    }

    #[test]
    fn el_raton_cerca_del_borde_lo_trae_y_lejos_no() {
        let (w, h) = (1000.0, 700.0);
        assert!(raton_cerca((990.0, 300.0), w, h, 1.0));
        assert!(raton_cerca((w - CERCA_DEL_BORDE, 10.0), w, h, 1.0));
        assert!(!raton_cerca((w - CERCA_DEL_BORDE - 1.0, 10.0), w, h, 1.0));
        // Con escala, la franja crece con ella.
        assert!(raton_cerca((w - 150.0, 10.0), w, h, 2.0));
        // Caso limite: fuera de la ventana (el raton se fue) no cuenta.
        assert!(!raton_cerca((1001.0, 300.0), w, h, 1.0));
        assert!(!raton_cerca((990.0, -1.0), w, h, 1.0));
        assert!(visible(false, EnUso::default(), true));
    }
}
