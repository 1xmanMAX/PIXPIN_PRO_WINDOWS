//! **La pista al pasar el raton** (tooltip): una linea que explica algo
//! que se ve pequeno (la chapita de los dias de una captura).

#![forbid(unsafe_code)]

use pixpin_render::{Pintor, RectF};

use super::color::{blanco, negro};
use super::geom::encoger;

/// Donde cae la caja de una pista de `ancho` x `alto` para lo senalado en
/// `ancla`: debajo, alineada a su borde derecho, sin salirse de `limite`.
/// Si debajo no cabe, encima. Aparte para probarlo sin pintar.
pub fn colocar(ancla: RectF, ancho: f32, alto: f32, limite: RectF, hueco: f32) -> RectF {
    let mut x = ancla.x + ancla.ancho - ancho;
    x = x.min(limite.x + limite.ancho - ancho).max(limite.x);
    let mut y = ancla.y + ancla.alto + hueco;
    if y + alto > limite.y + limite.alto {
        y = (ancla.y - hueco - alto).max(limite.y);
    }
    RectF {
        x,
        y,
        ancho,
        alto,
    }
}

/// Pinta la pista de `texto` para `ancla`, dentro de `limite`.
pub fn pintar(p: &Pintor, ancla: RectF, texto: &str, limite: RectF, s: f32) {
    let tam = super::LETRA_SECUNDARIO * s;
    let (tw, th) = p.medir_texto(texto, tam);
    let pad = 10.0 * s;
    let ancho = (tw + 2.0 * pad).min(limite.ancho);
    let alto = th + 12.0 * s;
    // Con aire contra el borde de la ventana: pegada al canto parece cortada.
    let caja = colocar(ancla, ancho, alto, encoger(limite, 8.0 * s), 6.0 * s);
    let radio = 8.0 * s;
    p.rellenar_redondeado(encoger(caja, -1.0 * s), radio + 1.0 * s, blanco(0.12));
    p.rellenar_redondeado(caja, radio, negro(0.92));
    p.texto_linea(
        texto,
        caja.x + pad,
        caja.y + (alto - th) / 2.0,
        tam,
        ancho - 2.0 * pad,
        super::TEXTO,
    );
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
    fn la_pista_va_debajo_alineada_a_la_derecha_y_sin_salirse() {
        let limite = r(0.0, 0.0, 1000.0, 800.0);
        let c = colocar(r(500.0, 100.0, 30.0, 24.0), 200.0, 30.0, limite, 6.0);
        assert_eq!(c, r(330.0, 130.0, 200.0, 30.0));
        // Pegada al borde izquierdo no se sale por la izquierda.
        let c = colocar(r(10.0, 100.0, 30.0, 24.0), 200.0, 30.0, limite, 6.0);
        assert_eq!(c.x, 0.0);
        // Caso negativo: abajo del todo no se sale; sube encima del ancla.
        let c = colocar(r(500.0, 780.0, 30.0, 20.0), 200.0, 30.0, limite, 6.0);
        assert_eq!(c.y, 780.0 - 6.0 - 30.0);
    }
}
