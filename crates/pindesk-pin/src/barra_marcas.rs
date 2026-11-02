//! **Marcar el texto de una imagen** (8-oct-2026): lo comun a la barra del
//! pin y a quien guarda las marcas.
//!
//! El usuario: «una vez hago que se reconozca el texto puedo seleccionar
//! textos; ahora con esto seleccionado pueda resaltar el texto, interlinearlo
//! o rayarlo o taparlo con un resaltado solido». Primero fue una barra aparte
//! encima del texto; despues pidio que no saliera ahi, que era grande y
//! estorbaba al mover el pin, sino en la barra de arriba del pin, la que
//! aparece y desaparece. Asi que aqui solo quedan los colores, las marcas y
//! sus dibujitos: la disposicion es la de `barra` y la ventana la de
//! `barra_flotante`. Android no tiene nada parecido en sus pines.
//!
//! Los dibujitos y los colores son tambien los de la barra de marcar de los
//! lectores (`lector_pdf::marcar`, 1-nov-2026).

use pindesk_render::{Color, Pintor, RectF};

// El modelo (las marcas, sus colores, la onda) es el componente comun de
// todos los visores, en el motor: aqui solo se pinta.
pub use pindesk_motor2d::marcas_texto::{COLOR_INICIAL, COLORES, MARCAS, Marca, onda};

/// El color `k` de [`COLORES`], para pintar.
pub fn color(k: u8) -> Color {
    let (r, g, b) = COLORES[(k as usize).min(COLORES.len() - 1)];
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

/// El dibujito de una marca en su boton: una «A» con la marca puesta, del
/// color elegido. `fondo` es el de la barra (la «A» tapada se recorta con el).
pub fn pintar_icono(
    p: &Pintor,
    marca: Option<Marca>,
    r: RectF,
    elegido: Color,
    tinta: Color,
    e: f32,
) {
    let tam = 16.0 * e;
    let letra = |p: &Pintor| {
        let (w, h) = p.medir_texto("A", tam);
        p.texto_linea(
            "A",
            r.x + (r.ancho - w) / 2.0,
            r.y + (r.alto - h) / 2.0 - 1.0 * e,
            tam,
            r.ancho,
            tinta,
        );
    };
    let (x0, x1) = (r.x + r.ancho / 2.0 - 9.0 * e, r.x + r.ancho / 2.0 + 9.0 * e);
    let abajo = r.y + r.alto / 2.0 + 10.0 * e;
    match marca {
        Some(Marca::Resaltar) => {
            p.rellenar_redondeado(
                RectF {
                    x: x0 - 1.0 * e,
                    y: r.y + r.alto / 2.0 - 10.0 * e,
                    ancho: x1 - x0 + 2.0 * e,
                    alto: 20.0 * e,
                },
                3.0 * e,
                Color { a: 0.55, ..elegido },
            );
            letra(p);
        }
        Some(Marca::Ondulada) => {
            letra(p);
            p.polilinea(&onda(x0, x1, abajo, 1.6 * e, 1.2 * e), 1.6 * e, elegido);
        }
        Some(Marca::Subrayar) => {
            letra(p);
            p.linea((x0, abajo), (x1, abajo), 1.8 * e, elegido);
        }
        Some(Marca::Tachar) => {
            letra(p);
            let y = r.y + r.alto / 2.0 + 1.0 * e;
            p.linea((x0, y), (x1, y), 2.0 * e, elegido);
        }
        Some(Marca::Tapar) => {
            letra(p);
            p.rellenar_redondeado(
                RectF {
                    x: x0,
                    y: r.y + r.alto / 2.0,
                    ancho: x1 - x0,
                    alto: 9.0 * e,
                },
                2.0 * e,
                elegido,
            );
        }
        // Quitar la marca: un circulo tachado.
        None => {
            let (cx, cy) = (r.x + r.ancho / 2.0, r.y + r.alto / 2.0);
            let radio = 8.0 * e;
            p.circulo((cx, cy), radio, Color { a: 0.35, ..tinta });
            let k = radio * 0.55;
            p.linea((cx - k, cy + k), (cx + k, cy - k), 1.8 * e, tinta);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn caso_negativo_un_color_fuera_de_rango_no_revienta() {
        assert_eq!(color(99), color(COLORES.len() as u8 - 1));
    }
}
