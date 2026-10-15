//! Cuentas de recuadros que hacia cada ventana por su cuenta.

#![forbid(unsafe_code)]

use pixpin_render::RectF;

/// Si un punto cae en un recuadro (bordes incluidos). El mismo de
/// `ventanita`, que es el que usa `Botones`: dos versiones darian zonas que
/// no casan con lo pintado en el borde.
pub use crate::ventanita::dentro;

/// El recuadro `m` mas pequeno por cada lado (con `m` negativo, mas
/// grande). Nunca da medidas negativas.
pub fn encoger(r: RectF, m: f32) -> RectF {
    RectF {
        x: r.x + m,
        y: r.y + m,
        ancho: (r.ancho - 2.0 * m).max(0.0),
        alto: (r.alto - 2.0 * m).max(0.0),
    }
}

/// Lo comun de dos recuadros, si lo hay: la zona pulsable de algo que
/// asoma a medias por el borde de una lista desplazada.
pub fn cortar(a: RectF, b: RectF) -> Option<RectF> {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.ancho).min(b.x + b.ancho);
    let y1 = (a.y + a.alto).min(b.y + b.alto);
    (x1 > x0 && y1 > y0).then_some(RectF {
        x: x0,
        y: y0,
        ancho: x1 - x0,
        alto: y1 - y0,
    })
}

/// Un cuadrado de `lado` centrado en `c`: la zona de 40 de algo que se ve
/// mas pequeno.
pub fn centrado(c: (f32, f32), lado: f32) -> RectF {
    RectF {
        x: c.0 - lado / 2.0,
        y: c.1 - lado / 2.0,
        ancho: lado,
        alto: lado,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn r(x: f32, y: f32, w: f32, h: f32) -> RectF {
        RectF {
            x,
            y,
            ancho: w,
            alto: h,
        }
    }

    #[test]
    fn encoger_quita_por_los_cuatro_lados_y_en_negativo_agranda() {
        assert_eq!(encoger(r(0.0, 0.0, 100.0, 50.0), 10.0), r(10.0, 10.0, 80.0, 30.0));
        assert_eq!(encoger(r(10.0, 10.0, 20.0, 20.0), -3.0), r(7.0, 7.0, 26.0, 26.0));
        // Caso negativo: encoger de mas no da un ancho negativo.
        let e = encoger(r(0.0, 0.0, 10.0, 10.0), 20.0);
        assert_eq!((e.ancho, e.alto), (0.0, 0.0));
    }

    #[test]
    fn cortar_da_lo_comun_y_nada_si_no_se_tocan() {
        assert_eq!(
            cortar(r(0.0, 0.0, 100.0, 100.0), r(50.0, 80.0, 100.0, 100.0)),
            Some(r(50.0, 80.0, 50.0, 20.0))
        );
        // Caso negativo: dos que solo se rozan no tienen zona comun.
        assert_eq!(cortar(r(0.0, 0.0, 10.0, 10.0), r(10.0, 0.0, 10.0, 10.0)), None);
    }

    #[test]
    fn centrado_pone_el_cuadrado_alrededor_del_punto() {
        let c = centrado((50.0, 20.0), 40.0);
        assert_eq!(c, r(30.0, 0.0, 40.0, 40.0));
        assert!(dentro(c, (50.0, 20.0)));
        // Caso negativo: lo que queda a 21 del centro ya esta fuera.
        assert!(!dentro(c, (71.0, 20.0)));
    }
}
