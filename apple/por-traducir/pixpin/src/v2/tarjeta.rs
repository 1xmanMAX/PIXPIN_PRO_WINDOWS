//! **La tarjeta** (una tarea, un momento, una leccion) y el **estado vacio**
//! de una lista.

#![forbid(unsafe_code)]

use pixpin_render::icono::Icono;
use pixpin_render::{Color, Pintor, RectF};

use super::color::blanco;
use super::geom::encoger;
use crate::caja_dibujo::hex;

/// El fondo de una tarjeta en `r`: TARJETA con radio 12 y un borde de 1 px
/// de blanco al 6 % (sin el, dos tarjetas juntas se funden). Con el raton
/// encima, mas clara; con el foco del teclado, el borde en ACENTO de 1,5.
///
/// No hay trazo redondeado en `Pintor`: el borde es el mismo recuadro un
/// poco mayor por debajo.
#[allow(dead_code)] // lo adoptan tareas y timeline en la siguiente tanda
pub fn fondo(p: &Pintor, r: RectF, encima: bool, foco: bool, s: f32) {
    let radio = super::RADIO_TARJETA * s;
    let (borde, grosor) = if foco {
        (super::ACENTO, 1.5 * s)
    } else {
        (blanco(0.06), 1.0 * s)
    };
    p.rellenar_redondeado(r, radio, borde);
    let relleno = if encima {
        super::TARJETA_ENCIMA
    } else {
        super::TARJETA
    };
    p.rellenar_redondeado(encoger(r, grosor), (radio - grosor).max(0.0), relleno);
}

/// Los tonos de las tarjetas de debajo de una pila, de la de justo debajo a
/// la del fondo.
const TONOS_PILA: [Color; 2] = [hex(0x48484E), hex(0x3A3A3F)];

/// Los cantos de las tarjetas de debajo de una pila de `n` (un grupo de
/// momentos, varias tareas iguales): 1 o 2, cada una 6 px mas arriba y a la
/// derecha. Se pintan ANTES de la tarjeta de encima, que las tapa salvo el
/// canto. Con `n` de 1 o menos no pinta nada.
#[allow(dead_code)] // lo adoptan tareas y timeline en la siguiente tanda
pub fn pila(p: &Pintor, r: RectF, n: usize, s: f32) {
    let radio = super::RADIO_TARJETA * s;
    for k in (0..cantos_de_pila(n)).rev() {
        let d = 6.0 * s * (k + 1) as f32;
        let canto = RectF {
            x: r.x + d,
            y: r.y - d,
            ..r
        };
        p.rellenar_redondeado(canto, radio, blanco(0.3));
        p.rellenar_redondeado(encoger(canto, 1.0 * s), radio - 1.0 * s, TONOS_PILA[k]);
    }
}

/// Cuantos cantos asoman en una pila de `n`: uno por cada tarjeta de mas,
/// hasta dos (con mas, no se distinguen).
#[allow(dead_code)] // lo adoptan tareas y timeline en la siguiente tanda
pub fn cantos_de_pila(n: usize) -> usize {
    n.saturating_sub(1).min(TONOS_PILA.len())
}

/// **El estado vacio** de una lista: un icono de 48 en blanco al 25 % y un
/// texto de 15 en gris debajo, centrados en `zona`. Que una lista sin nada
/// no parezca una ventana rota.
pub fn vacio(p: &Pintor, zona: RectF, icono: &Icono, texto: &str, s: f32) {
    let lado = 48.0 * s;
    let tam = super::LETRA_TITULO_TARJETA * s;
    let ancho = (zona.ancho - 64.0 * s).clamp(0.0, 420.0 * s);
    let (tw, th) = p.medir_texto_ajustado(texto, tam, ancho);
    let hueco = super::HUECO_GRANDE * s;
    let y = zona.y + (zona.alto - lado - hueco - th) / 2.0;
    p.icono(
        icono,
        RectF {
            x: zona.x + (zona.ancho - lado) / 2.0,
            y,
            ancho: lado,
            alto: lado,
        },
        blanco(0.25),
    );
    p.texto_ajustado(
        texto,
        zona.x + (zona.ancho - tw) / 2.0,
        y + lado + hueco,
        tam,
        ancho,
        super::GRIS,
    );
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_pila_ensena_un_canto_por_tarjeta_de_mas_hasta_dos() {
        assert_eq!(cantos_de_pila(2), 1);
        assert_eq!(cantos_de_pila(3), 2);
        assert_eq!(cantos_de_pila(40), 2);
        // Caso negativo: una sola (o ninguna) no es pila.
        assert_eq!(cantos_de_pila(1), 0);
        assert_eq!(cantos_de_pila(0), 0);
    }
}
