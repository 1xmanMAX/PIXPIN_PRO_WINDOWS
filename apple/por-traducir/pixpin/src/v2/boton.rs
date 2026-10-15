//! **El boton de solo icono** del v2 (la ✕ de una barra, el lapiz de una
//! fila). Los de icono y rotulo son `lecciones::ui::boton_v2`.

#![forbid(unsafe_code)]

use pixpin_render::icono::Icono;
use pixpin_render::{Pintor, RectF};

use super::color::{blanco, negro};
use super::geom::{dentro, encoger};
use crate::ventanita::Botones;

/// Pinta un boton de icono en `caja` (40x40 en logicos) y lo apunta.
///
/// - En reposo, sin fondo: una fila de iconos con caja cada uno pesaba
///   demasiado en las maquetas.
/// - Con el raton encima, blanco al 9 %; encendido (un modo activo), el
///   azul de relleno con el icono en blanco.
/// - `sobre_foto`: va encima de una miniatura, donde sin fondo no se veria;
///   lleva un circulo negro al 45 %.
#[allow(clippy::too_many_arguments)] // pintor, botones, caja, accion, icono, estados y escala
pub fn boton_icono<A: Copy>(
    p: &Pintor,
    botones: &mut Botones<A>,
    caja: RectF,
    accion: A,
    icono: &Icono,
    encendido: bool,
    sobre_foto: bool,
    s: f32,
) {
    let encima = dentro(caja, botones.raton);
    let radio = super::RADIO_BOTON * s;
    if sobre_foto {
        let c = (caja.x + caja.ancho / 2.0, caja.y + caja.alto / 2.0);
        let fondo = if encendido {
            super::AZUL_LLENO
        } else {
            negro(if encima { 0.6 } else { 0.45 })
        };
        p.circulo(c, caja.ancho.min(caja.alto) / 2.0, fondo);
    } else if encendido {
        let fondo = if encima {
            super::color::aclarar(super::AZUL_LLENO, 0.07)
        } else {
            super::AZUL_LLENO
        };
        p.rellenar_redondeado(caja, radio, fondo);
    } else if encima {
        p.rellenar_redondeado(caja, radio, blanco(0.09));
    }
    let tinta = if encendido || sobre_foto {
        pixpin_render::Color::BLANCO
    } else if encima {
        super::TEXTO
    } else {
        super::CUERPO
    };
    p.icono(icono, encoger(caja, 10.0 * s), tinta);
    botones.zona(caja, accion);
}
