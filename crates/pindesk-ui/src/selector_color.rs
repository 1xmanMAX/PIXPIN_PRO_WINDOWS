//! **Las cuentas del selector de color avanzado**, sin nada de pantalla.
//!
//! El usuario (2-nov-2026): «un selector de colores mas avanzado, y
//! agregale opacidad». El desplegable de la paleta gana un cuadro de
//! saturacion y brillo, una tira de tonos, una de opacidad, el codigo de
//! seis u ocho digitos, el cuentagotas y los colores recientes. Lo que se
//! pinta y donde va lo decide `panel_lateral`; aqui va solo la aritmetica,
//! para probarla sin GPU:
//!
//! - HSV de ida y vuelta ([`Hsv`]), que es lo que mueve el cuadro y la tira.
//! - Leer y escribir el codigo ([`leer_codigo`], [`escribir_codigo`]): con
//!   o sin almohadilla, 3, 6 u 8 digitos.
//! - Que marca se lee encima de una muestra ([`marca_clara`]): blanca sobre
//!   lo oscuro y oscura sobre lo claro, por contraste WCAG y no a ojo.
//! - Los recientes ([`Recientes`]): el ultimo delante, sin repetidos.
//!
//! **La opacidad no va dentro del color.** Excalidraw admite `#rrggbbaa` en
//! `strokeColor`, pero el movil lee los colores con `Color.parseColor`, que
//! entiende ocho digitos como `#aarrggbb`: un rojo al 50 % saldria alli de
//! otro color. Por eso los dos digitos de mas que se escriben en el campo
//! van al campo `opacity` del elemento (0-100, el de siempre, que el movil
//! ya pinta), y el color se guarda siempre con seis.

use pindesk_motor2d::elemento::ColorRgba;

/// Un color en tono (0-360), saturacion (0-1) y brillo (0-1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsv {
    pub h: f32,
    pub s: f32,
    pub v: f32,
}

impl Hsv {
    /// El de un color. Un gris no tiene tono: sale 0, y quien recuerda el
    /// tono que se estaba eligiendo lo conserva con [`Hsv::con_tono_de`].
    pub fn de(c: ColorRgba) -> Hsv {
        let (r, g, b) = (
            c.r.clamp(0.0, 1.0),
            c.g.clamp(0.0, 1.0),
            c.b.clamp(0.0, 1.0),
        );
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let d = max - min;
        let h = if d <= f32::EPSILON {
            0.0
        } else if max == r {
            60.0 * ((g - b) / d).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / d + 2.0)
        } else {
            60.0 * ((r - g) / d + 4.0)
        };
        Hsv {
            h: h.rem_euclid(360.0),
            s: if max <= f32::EPSILON { 0.0 } else { d / max },
            v: max,
        }
    }

    /// El color opaco que es.
    pub fn color(self) -> ColorRgba {
        let h = self.h.rem_euclid(360.0) / 60.0;
        let s = self.s.clamp(0.0, 1.0);
        let v = self.v.clamp(0.0, 1.0);
        let c = v * s;
        let x = c * (1.0 - (h % 2.0 - 1.0).abs());
        let (r, g, b) = match h as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = v - c;
        ColorRgba::opaco(r + m, g + m, b + m)
    }

    /// **El tono que se estaba eligiendo, si este color no tiene uno.**
    /// Arrastrar el cuadro hasta el borde gris (o el negro) deja un color
    /// sin tono; sin esto la tira saltaria al rojo y el siguiente arrastre
    /// del cuadro pintaria rojo en vez del azul que se estaba aclarando.
    pub fn con_tono_de(self, recordado: Option<f32>) -> Hsv {
        match recordado {
            Some(h) if self.s <= 0.001 || self.v <= 0.001 => Hsv { h, ..self },
            _ => self,
        }
    }
}

/// El tono puro (saturacion y brillo enteros): el fondo del cuadro.
pub fn tono_puro(h: f32) -> ColorRgba {
    Hsv { h, s: 1.0, v: 1.0 }.color()
}

fn byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// **Un codigo escrito**: `#rgb`, `#rrggbb` o `#rrggbbaa`, con o sin la
/// almohadilla y sin distinguir mayusculas. Devuelve el color (opaco) y, si
/// traia los dos ultimos digitos, la opacidad de 0 a 1. `None` con
/// cualquier otra cosa.
pub fn leer_codigo(escrito: &str) -> Option<(ColorRgba, Option<f32>)> {
    let s = escrito.trim();
    let s = s.strip_prefix('#').unwrap_or(s);
    if !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let par = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
    let uno = |i: usize| Some(u8::from_str_radix(s.get(i..=i)?, 16).ok()? * 17);
    let (r, g, b, a) = match s.len() {
        3 => (uno(0)?, uno(1)?, uno(2)?, None),
        6 => (par(0)?, par(2)?, par(4)?, None),
        8 => (par(0)?, par(2)?, par(4)?, Some(par(6)?)),
        _ => return None,
    };
    let f = |v: u8| v as f32 / 255.0;
    Some((ColorRgba::opaco(f(r), f(g), f(b)), a.map(f)))
}

/// `rrggbb` en minusculas y sin almohadilla; con `aa` detras si la
/// opacidad no es entera.
pub fn escribir_codigo(c: ColorRgba, opacidad: f32) -> String {
    let mut s = format!("{:02x}{:02x}{:02x}", byte(c.r), byte(c.g), byte(c.b));
    if byte(opacidad) < 255 {
        s.push_str(&format!("{:02x}", byte(opacidad)));
    }
    s
}

/// Luminancia relativa de WCAG.
pub fn luminancia(c: ColorRgba) -> f32 {
    let lineal = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lineal(c.r) + 0.7152 * lineal(c.g) + 0.0722 * lineal(c.b)
}

/// El contraste WCAG entre dos colores (de 1 a 21).
pub fn contraste(a: ColorRgba, b: ColorRgba) -> f32 {
    let (la, lb) = (luminancia(a), luminancia(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// **Si la marca encima de esta muestra va blanca.** La que mas contraste
/// tenga: blanca sobre el azul de Excalidraw, oscura sobre el amarillo.
pub fn marca_clara(fondo: ColorRgba) -> bool {
    let blanco = ColorRgba::opaco(1.0, 1.0, 1.0);
    let oscuro = ColorRgba::opaco(0.118, 0.118, 0.118);
    contraste(fondo, blanco) >= contraste(fondo, oscuro)
}

/// Si dos colores se ven igual (a 8 bits por canal).
pub fn iguales(a: ColorRgba, b: ColorRgba) -> bool {
    byte(a.r) == byte(b.r) && byte(a.g) == byte(b.g) && byte(a.b) == byte(b.b)
}

/// **Los colores que se eligieron a mano**, el ultimo delante.
///
/// Solo los que no estaban ya a la vista: un color de la paleta se vuelve
/// a encontrar en la paleta, y repetirlo aqui gastaria un hueco. Lo decide
/// quien apunta; aqui solo se ordena y se recorta.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Recientes {
    colores: Vec<ColorRgba>,
}

impl Recientes {
    /// Cuantos se recuerdan: una fila del desplegable.
    pub const CUANTOS: usize = 5;

    /// Los recordados, leidos de sus codigos (lo que no se lee, fuera).
    pub fn de_codigos<S: AsRef<str>>(codigos: &[S]) -> Recientes {
        let mut r = Recientes::default();
        for c in codigos.iter().rev() {
            if let Some((color, _)) = leer_codigo(c.as_ref()) {
                r.apuntar(color);
            }
        }
        r
    }

    /// Para guardarlos: `#rrggbb`, el ultimo delante.
    pub fn codigos(&self) -> Vec<String> {
        self.colores
            .iter()
            .map(|c| format!("#{}", escribir_codigo(*c, 1.0)))
            .collect()
    }

    pub fn colores(&self) -> &[ColorRgba] {
        &self.colores
    }

    /// Pone `c` delante. Devuelve si cambio algo.
    pub fn apuntar(&mut self, c: ColorRgba) -> bool {
        let c = ColorRgba { a: 1.0, ..c };
        if self.colores.first().is_some_and(|p| iguales(*p, c)) {
            return false;
        }
        self.colores.retain(|p| !iguales(*p, c));
        self.colores.insert(0, c);
        self.colores.truncate(Self::CUANTOS);
        true
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn hex(v: u32) -> ColorRgba {
        let c = |s: u32| ((v >> s) & 0xff) as f32 / 255.0;
        ColorRgba::opaco(c(16), c(8), c(0))
    }

    #[test]
    fn los_primarios_tienen_su_tono() {
        let h = |v| Hsv::de(hex(v)).h.round();
        assert_eq!(h(0xff0000), 0.0);
        assert_eq!(h(0xffff00), 60.0);
        assert_eq!(h(0x00ff00), 120.0);
        assert_eq!(h(0x00ffff), 180.0);
        assert_eq!(h(0x0000ff), 240.0);
        assert_eq!(h(0xff00ff), 300.0);
    }

    #[test]
    fn de_rgb_a_hsv_y_vuelta_da_el_mismo_color() {
        // La paleta entera de Excalidraw y unos cuantos sueltos: ida y vuelta
        // a 8 bits por canal, sin perder ni un digito del codigo.
        for v in [
            0x1e1e1e, 0xe03131, 0x2f9e44, 0x1971c2, 0xf08c00, 0xffc9c9, 0xb2f2bb, 0xa5d8ff,
            0xffec99, 0xffffff, 0x000000, 0x123456, 0x7f7f7f, 0xc2255c, 0x099268,
        ] {
            let c = hex(v);
            let vuelta = Hsv::de(c).color();
            assert!(iguales(c, vuelta), "{v:06x} volvio como {vuelta:?}");
        }
    }

    #[test]
    fn un_gris_conserva_el_tono_que_se_estaba_eligiendo() {
        let gris = Hsv::de(hex(0x808080));
        assert_eq!(gris.s, 0.0);
        assert_eq!(gris.con_tono_de(Some(210.0)).h, 210.0);
        // Caso negativo: un color con tono propio no se lo deja quitar.
        let azul = Hsv::de(hex(0x1971c2));
        assert!((azul.con_tono_de(Some(10.0)).h - azul.h).abs() < 0.01);
        assert_eq!(gris.con_tono_de(None).h, 0.0);
    }

    #[test]
    fn el_codigo_se_lee_con_tres_seis_u_ocho_digitos() {
        assert_eq!(leer_codigo("#e03131"), Some((hex(0xe03131), None)));
        assert_eq!(leer_codigo("E03131"), Some((hex(0xe03131), None)));
        assert_eq!(leer_codigo("#f0a"), Some((hex(0xff00aa), None)));
        let (c, a) = leer_codigo(" #1971c280 ").unwrap();
        assert!(iguales(c, hex(0x1971c2)));
        assert!((a.unwrap() - 128.0 / 255.0).abs() < 1e-6);
        // Casos negativos: largos raros, letras que no son hex, vacio.
        for malo in [
            "",
            "#",
            "#ff",
            "#ffff",
            "1234567",
            "#12345g",
            "transparent",
            "#1234567890",
        ] {
            assert_eq!(leer_codigo(malo), None, "{malo}");
        }
    }

    #[test]
    fn el_codigo_lleva_la_opacidad_solo_si_no_es_entera() {
        assert_eq!(escribir_codigo(hex(0x1e1e1e), 1.0), "1e1e1e");
        assert_eq!(escribir_codigo(hex(0x1e1e1e), 0.5), "1e1e1e80");
        assert_eq!(escribir_codigo(hex(0xffffff), 0.0), "ffffff00");
        // Y lo escrito se vuelve a leer igual.
        let (c, a) = leer_codigo(&escribir_codigo(hex(0xabcdef), 0.4)).unwrap();
        assert!(iguales(c, hex(0xabcdef)));
        assert!((a.unwrap() - 0.4).abs() < 0.003);
    }

    #[test]
    fn la_marca_va_blanca_sobre_lo_oscuro_y_oscura_sobre_lo_claro() {
        assert!(marca_clara(hex(0x1e1e1e)));
        assert!(marca_clara(hex(0x1971c2)));
        assert!(marca_clara(hex(0xe03131)));
        assert!(!marca_clara(hex(0xffffff)));
        assert!(!marca_clara(hex(0xffec99)));
        assert!(!marca_clara(hex(0xa5d8ff)));
        assert!((contraste(hex(0x000000), hex(0xffffff)) - 21.0).abs() < 0.01);
    }

    #[test]
    fn los_recientes_ponen_el_ultimo_delante_sin_repetir_y_recortan() {
        let mut r = Recientes::default();
        assert!(r.apuntar(hex(0x111111)));
        assert!(r.apuntar(hex(0x222222)));
        assert!(
            !r.apuntar(hex(0x222222)),
            "el mismo otra vez no cambia nada"
        );
        assert!(r.apuntar(hex(0x111111)), "uno de atras pasa delante");
        assert_eq!(r.colores().len(), 2);
        assert!(iguales(r.colores()[0], hex(0x111111)));
        for v in 0..10 {
            r.apuntar(hex(0x100000 * v + 5));
        }
        assert_eq!(r.colores().len(), Recientes::CUANTOS);
        // Y vuelven igual de su fichero, en el mismo orden.
        let leidos = Recientes::de_codigos(&r.codigos());
        assert_eq!(leidos.codigos(), r.codigos());
        // Caso negativo: lo que no es un codigo no entra.
        assert!(Recientes::de_codigos(&["rojo", ""]).colores().is_empty());
    }
}
