//! Cuentas de color: transparencias, mezclas y el color estable de un texto.

#![forbid(unsafe_code)]

use pixpin_render::Color;

/// El mismo color con otra opacidad.
pub const fn con_alfa(c: Color, a: f32) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a,
    }
}

/// Blanco con opacidad `a`: los realces sobre fondo oscuro (encima 9 %,
/// rayas 7 %).
pub const fn blanco(a: f32) -> Color {
    con_alfa(Color::BLANCO, a)
}

/// Negro con opacidad `a`: lo que va sobre una foto (el aviso, los botones
/// de una miniatura).
pub const fn negro(a: f32) -> Color {
    con_alfa(Color::NEGRO, a)
}

/// `a` con un `t` de `b` encima (0 = todo `a`, 1 = todo `b`). Conserva la
/// opacidad de `a`: mezclar un tono no debe volver opaco lo transparente.
#[allow(dead_code)] // lo adoptan tareas y timeline en la siguiente tanda
pub fn mezcla(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a,
    }
}

/// Mas claro en `d` (0..1) por canal: el hover de un boton con fondo.
pub fn aclarar(c: Color, d: f32) -> Color {
    Color {
        r: (c.r + d).min(1.0),
        g: (c.g + d).min(1.0),
        b: (c.b + d).min(1.0),
        a: c.a,
    }
}

/// Mas oscuro en `d` (0..1) por canal: un fondo para letra blanca sacado de
/// un color vivo (el verde de «conservada»).
pub fn oscurecer(c: Color, d: f32) -> Color {
    Color {
        r: (c.r - d).max(0.0),
        g: (c.g - d).max(0.0),
        b: (c.b - d).max(0.0),
        a: c.a,
    }
}

/// **El color fijo de un texto** (una etiqueta, un area, un chat): uno de
/// `paleta`, elegido por una huella del texto. El mismo texto da siempre el
/// mismo color, entre ventanas y entre arranques, sin guardarlo en ningun
/// sitio. FNV-1a y no el `Hash` de Rust: ese cambia entre versiones.
#[allow(dead_code)] // lo adoptan tareas y timeline en la siguiente tanda
pub fn de_clave(texto: &str, paleta: &[Color]) -> Color {
    if paleta.is_empty() {
        return super::GRIS;
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in texto.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    paleta[(h % paleta.len() as u64) as usize]
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const ROJO: Color = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    #[test]
    fn mezcla_va_de_un_color_al_otro_y_conserva_la_opacidad() {
        let m = mezcla(con_alfa(Color::NEGRO, 0.5), Color::BLANCO, 0.25);
        assert_eq!((m.r, m.g, m.b, m.a), (0.25, 0.25, 0.25, 0.5));
        assert_eq!(mezcla(ROJO, Color::BLANCO, 1.0).g, 1.0);
        // Caso negativo: un t fuera de 0..1 no se pasa del otro color.
        assert_eq!(mezcla(ROJO, Color::NEGRO, 3.0).r, 0.0);
    }

    #[test]
    fn aclarar_y_oscurecer_no_se_salen_de_0_a_1() {
        let c = aclarar(ROJO, 0.2);
        assert_eq!((c.r, c.g), (1.0, 0.2));
        let c = oscurecer(ROJO, 0.2);
        assert_eq!((c.r, c.g), (0.8, 0.0));
        // Caso negativo: la opacidad no se toca.
        assert_eq!(aclarar(blanco(0.3), 0.5).a, 0.3);
        assert_eq!(negro(0.45).a, 0.45);
    }

    #[test]
    fn el_mismo_texto_da_siempre_el_mismo_color() {
        let paleta = [super::super::ACENTO, super::super::VERDE, super::super::ROJO];
        let a = de_clave("Trabajo", &paleta);
        assert_eq!(a, de_clave("Trabajo", &paleta));
        // Textos distintos reparten por toda la paleta.
        let usados: std::collections::HashSet<String> = (0..30)
            .map(|k| format!("{:?}", de_clave(&format!("area {k}"), &paleta)))
            .collect();
        assert_eq!(usados.len(), 3);
        // Caso negativo: sin paleta, gris y sin panico.
        assert_eq!(de_clave("x", &[]), super::super::GRIS);
    }
}
