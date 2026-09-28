//! El panel de propiedades de un pin que se anota: el color, el grosor y lo
//! demas de la herramienta o de lo elegido, **el mismo panel del lienzo**
//! (`panel_dibujo`, que monta `pixpin_ui::panel_lateral` con la tabla de
//! `pixpin_ui::propiedades`).
//!
//! Antes la barra del pin no tenia color: habia que abrir el pin en el
//! lienzo para cambiar de tinta. Ahora va en una ventanita al lado del pin,
//! una `Paleta` como la de la barra, que no se activa nunca para que el
//! teclado siga en el pin.
//!
//! Aqui solo vive donde se pone. Que sale y que hace cada clic es de
//! `panel_dibujo`, igual que en el lienzo, el lector y la hojita.

use pixpin_geom::Rect;
use pixpin_ui::panel_lateral::PanelLateral;

/// Lo que ocupa el panel de ancho y cuanto se separa del pin, logicos.
const ANCHO_PANEL: u32 = 200;
const HUECO: u32 = 8;
/// Donde empieza el panel dentro de su `area` (`panel_lateral`: 16 a la
/// izquierda y 76 por arriba, lo que deja sitio a la barra).
const X_EN_AREA: u32 = 16;
const Y_EN_AREA: u32 = 76;
/// Lo que se sale la sombra de la isla por los lados y por abajo.
const SOMBRA: u32 = 14;

/// **El `area` que se le da al panel** para que quede junto al pin: a su
/// izquierda si cabe, y si no a su derecha. `panel_lateral` coloca el panel
/// a 16 del borde izquierdo y 76 del de arriba de su area, asi que el area
/// se corre para que ese sitio caiga donde se quiere.
///
/// Alineado con lo alto del pin, pero nunca por encima de donde lo pondria
/// el lienzo: ahi arriba esta la barra de herramientas.
pub fn area_para(pin: Rect, trabajo: Rect, escala: u32) -> Rect {
    let e = |v: u32| (v * escala / 100) as i32;
    let izquierda = pin.x - e(HUECO) - e(ANCHO_PANEL);
    let x_panel = if izquierda >= trabajo.x {
        izquierda
    } else {
        pin.derecha() + e(HUECO)
    };
    let x = x_panel - e(X_EN_AREA);
    let y = (pin.y - e(Y_EN_AREA)).max(trabajo.y);
    Rect {
        x,
        y,
        ancho: (trabajo.derecha() - x).max(1) as u32,
        alto: (trabajo.abajo() - y).max(1) as u32,
    }
}

/// La ventana que lo muestra: el panel y su desplegable, si lo hay, con
/// sitio para la sombra. Un rect del escritorio.
pub fn marco_de(panel: &PanelLateral, escala: u32) -> Rect {
    let mut r = panel.marco;
    if let Some(em) = &panel.emergente {
        r = union(r, em.marco);
    }
    let s = (SOMBRA * escala / 100) as i32;
    Rect {
        x: r.x - s,
        y: r.y - s,
        ancho: r.ancho + 2 * s as u32,
        alto: r.alto + 2 * s as u32,
    }
}

fn union(a: Rect, b: Rect) -> Rect {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    Rect {
        x,
        y,
        ancho: (a.derecha().max(b.derecha()) - x) as u32,
        alto: (a.abajo().max(b.abajo()) - y) as u32,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::escena::Escena;
    use pixpin_motor2d::gesto::{Gesto, Herramienta};

    const TRABAJO: Rect = Rect {
        x: 0,
        y: 0,
        ancho: 1920,
        alto: 1040,
    };

    fn panel_de(area: Rect) -> PanelLateral {
        let mut gesto = Gesto::nuevo();
        gesto.herramienta = Herramienta::Lapiz;
        crate::panel_dibujo::panel_para(&gesto, &Escena::nueva(), area, 100)
            .expect("el lapiz tiene color y grosor")
    }

    #[test]
    fn con_sitio_el_panel_va_a_la_izquierda_del_pin_sin_tocarlo() {
        let pin = Rect {
            x: 800,
            y: 300,
            ancho: 400,
            alto: 300,
        };
        let panel = panel_de(area_para(pin, TRABAJO, 100));
        assert_eq!(panel.marco.derecha(), pin.x - 8, "{:?}", panel.marco);
        assert_eq!(panel.marco.y, pin.y, "alineado con lo alto del pin");
    }

    #[test]
    fn pegado_a_la_izquierda_el_panel_pasa_a_la_derecha() {
        let pin = Rect {
            x: 40,
            y: 300,
            ancho: 400,
            alto: 300,
        };
        let panel = panel_de(area_para(pin, TRABAJO, 100));
        assert_eq!(panel.marco.x, pin.derecha() + 8, "{:?}", panel.marco);
        assert!(panel.marco.x >= TRABAJO.x);
    }

    #[test]
    fn arriba_del_todo_el_panel_no_sube_hasta_la_barra() {
        let pin = Rect {
            x: 800,
            y: 10,
            ancho: 400,
            alto: 300,
        };
        let panel = panel_de(area_para(pin, TRABAJO, 100));
        assert_eq!(panel.marco.y, 76, "donde lo pondria el lienzo");
    }

    #[test]
    fn la_ventana_del_panel_lo_cubre_entero_con_su_sombra() {
        let panel = panel_de(area_para(
            Rect {
                x: 800,
                y: 300,
                ancho: 400,
                alto: 300,
            },
            TRABAJO,
            150,
        ));
        let v = marco_de(&panel, 150);
        assert!(v.x < panel.marco.x && v.y < panel.marco.y);
        assert!(v.derecha() > panel.marco.derecha() && v.abajo() > panel.marco.abajo());
    }

    #[test]
    fn con_la_mano_no_hay_panel_que_ensenar() {
        // Caso negativo: la mano no deja rastro y no tiene nada que ajustar.
        let mut gesto = Gesto::nuevo();
        gesto.herramienta = Herramienta::Mano;
        let area = area_para(
            Rect {
                x: 800,
                y: 300,
                ancho: 400,
                alto: 300,
            },
            TRABAJO,
            100,
        );
        assert!(crate::panel_dibujo::panel_para(&gesto, &Escena::nueva(), area, 100).is_none());
    }
}
