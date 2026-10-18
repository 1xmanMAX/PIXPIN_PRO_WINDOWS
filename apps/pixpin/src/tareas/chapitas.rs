//! **Las chapitas de una tarea** (18-oct-2026): su urgencia («Urgente»,
//! «Muy urgente») y sus `#etiquetas`, en una fila de pildoras pequeñas. Las
//! pintan la tarjeta de la lista, el cuadrado, la columna del Kanban y el pin
//! de la tarea: una sola pieza para todos.

#![forbid(unsafe_code)]

use pixpin_render::{Color, Pintor, RectF};
use pixpin_store::Catalogo;

use super::frase::{Chapa, Urgencia};
use crate::caja_dibujo::hex;
use crate::v2::color::{blanco, con_alfa, negro};

/// El alto de una chapita, en pixeles logicos.
pub const ALTO: f32 = 20.0;
/// La letra.
const LETRA: f32 = 11.5;
/// El hueco entre dos.
const HUECO: f32 = 6.0;

/// El rojo de «muy urgente» y el ambar de «urgente» (los de la franja del
/// pin de la tarea).
pub const ROJO: Color = hex(0xe5534b);
pub const AMBAR: Color = hex(0xe8a33d);
/// La letra de una etiqueta: el azul claro de las fichas de la caja.
const AZUL_LETRA: Color = hex(0x9cc4f0);

/// Lo que dice una chapita.
pub fn rotulo(c: &Chapa, textos: &Catalogo) -> String {
    match c {
        Chapa::Urgencia(Urgencia::MuyUrgente) => textos.t("tareas-muy-urgente"),
        Chapa::Urgencia(_) => textos.t("tareas-urgente"),
        Chapa::Etiqueta(e) => format!("#{e}"),
    }
}

/// Sus colores (fondo y letra). `sobre_foto`: encima de una imagen, con un
/// fondo oscuro que se lea.
fn colores(c: &Chapa, sobre_foto: bool) -> (Color, Color) {
    match c {
        Chapa::Urgencia(Urgencia::MuyUrgente) => (ROJO, Color::BLANCO),
        Chapa::Urgencia(_) if sobre_foto => (negro(0.6), AMBAR),
        Chapa::Urgencia(_) => (con_alfa(AMBAR, 0.22), AMBAR),
        Chapa::Etiqueta(_) if sobre_foto => (negro(0.55), Color::BLANCO),
        Chapa::Etiqueta(_) => (blanco(0.08), AZUL_LETRA),
    }
}

/// **Pinta la fila** desde `(x, y)` (arriba a la izquierda) sin pasar de
/// `ancho`: las que no caben no salen. Devuelve la caja de cada una que se
/// pinto, con su posicion en `chapas` (para pulsarla).
#[allow(clippy::too_many_arguments)] // pintor, chapitas, sitio, ancho, foto, escala y textos
pub fn pintar(
    p: &Pintor,
    chapas: &[Chapa],
    x: f32,
    y: f32,
    ancho: f32,
    sobre_foto: bool,
    s: f32,
    textos: &Catalogo,
) -> Vec<(RectF, usize)> {
    let mut v = Vec::new();
    let mut cx = x;
    let tam = LETRA * s;
    let alto = ALTO * s;
    for (k, c) in chapas.iter().enumerate() {
        let t = rotulo(c, textos);
        let (tw, th) = p.medir_texto(&t, tam);
        let w = tw + 16.0 * s;
        if cx + w > x + ancho + 0.5 {
            break;
        }
        let caja = RectF {
            x: cx,
            y,
            ancho: w,
            alto,
        };
        let (fondo, letra) = colores(c, sobre_foto);
        p.rellenar_redondeado(caja, alto / 2.0, fondo);
        p.texto(&t, cx + 8.0 * s, y + (alto - th) / 2.0, tam, letra);
        v.push((caja, k));
        cx += w + HUECO * s;
    }
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cada_chapita_dice_lo_suyo() {
        let t = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        assert_eq!(rotulo(&Chapa::Urgencia(Urgencia::Urgente), &t), "Urgente");
        assert_eq!(
            rotulo(&Chapa::Urgencia(Urgencia::MuyUrgente), &t),
            "Muy urgente"
        );
        assert_eq!(rotulo(&Chapa::Etiqueta("casa".into()), &t), "#casa");
        let en = Catalogo::nuevo(pixpin_store::Idioma::Ingles);
        assert_eq!(
            rotulo(&Chapa::Urgencia(Urgencia::MuyUrgente), &en),
            "Very urgent"
        );
        // Muy urgente se distingue de urgente de lejos: fondo lleno.
        assert_ne!(
            colores(&Chapa::Urgencia(Urgencia::MuyUrgente), false).0,
            colores(&Chapa::Urgencia(Urgencia::Urgente), false).0
        );
    }
}
