//! **El tema**: letras, colores y grosor de linea de una plantilla.
//!
//! Los de serie son los de las plantillas de pagina de PinDesk
//! (`pindesk_motor2d::figuras_de_serie::plantillas`): la tinta azul pizarra,
//! los rotulos grises, las rayas claras y un acento por categoria
//! (planificacion azul, academico verde, creativo naranja, diario rosa), con
//! el titulo en Fraunces (o Caveat en el diario) y el cuerpo en Work Sans.
//! Asi lo que sale de aqui se ve de la misma familia que las figuras.

use serde::{Deserialize, Serialize};

/// Un color opaco o con transparencia. En JSON, `#rrggbb` o `#rrggbbaa`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn hex(c: u32) -> Color {
        Color {
            r: ((c >> 16) & 0xff) as u8,
            g: ((c >> 8) & 0xff) as u8,
            b: (c & 0xff) as u8,
            a: 255,
        }
    }

    pub const fn con_alfa(self, a: u8) -> Color {
        Color { a, ..self }
    }

    /// Lee `#rgb`, `#rrggbb` o `#rrggbbaa` (con o sin `#`).
    pub fn leer(s: &str) -> Option<Color> {
        let s = s.trim().trim_start_matches('#');
        let n = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
        match s.len() {
            3 => {
                let c: Vec<u8> = s
                    .chars()
                    .map(|c| c.to_digit(16).map(|d| d as u8 * 17))
                    .collect::<Option<_>>()?;
                Some(Color {
                    r: c[0],
                    g: c[1],
                    b: c[2],
                    a: 255,
                })
            }
            6 => Some(Color {
                r: n(0)?,
                g: n(2)?,
                b: n(4)?,
                a: 255,
            }),
            8 => Some(Color {
                r: n(0)?,
                g: n(2)?,
                b: n(4)?,
                a: n(6)?,
            }),
            _ => None,
        }
    }

    pub fn a_texto(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    /// Mezcla con `otro` (0 = este, 1 = el otro).
    pub fn mezcla(self, otro: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let m = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
        Color {
            r: m(self.r, otro.r),
            g: m(self.g, otro.g),
            b: m(self.b, otro.b),
            a: m(self.a, otro.a),
        }
    }

    /// 0-1, para los que pintan en flotante.
    pub fn f32s(self) -> [f32; 4] {
        [
            f32::from(self.r) / 255.0,
            f32::from(self.g) / 255.0,
            f32::from(self.b) / 255.0,
            f32::from(self.a) / 255.0,
        ]
    }
}

impl Serialize for Color {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.a_texto())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Color, D::Error> {
        let s = String::deserialize(d)?;
        Color::leer(&s).ok_or_else(|| serde::de::Error::custom(format!("color «{s}»")))
    }
}

/// Las letras de una plantilla: el nombre de la familia (las tres son de
/// las que trae PinDesk, `pindesk_render::letras`).
pub const FRAUNCES: &str = "Fraunces";
pub const WORK_SANS: &str = "Work Sans";
pub const CAVEAT: &str = "Caveat";

/// Que letra lleva un texto: la del titulo, la del cuerpo o la de mano.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Letra {
    Titulo,
    Cuerpo,
    Mano,
}

/// **El tema** de una plantilla.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tema {
    /// De cual de serie salio (`planificacion`, `diario`…), o `propio`.
    #[serde(default = "propio")]
    pub base: String,
    pub acento: Color,
    /// El acento suave (cabeceras de tabla, fondos).
    pub suave: Color,
    /// El texto.
    pub tinta: Color,
    /// Rotulos y numeros pequeños.
    pub rotulo: Color,
    /// Renglones y puntos.
    pub raya: Color,
    /// Bordes de cajas y tablas.
    pub borde: Color,
    pub papel: Color,
    pub letra_titulo: String,
    pub letra_cuerpo: String,
    /// El grosor de las rayas finas (mm); las fuertes son el doble.
    pub grosor: f32,
    /// La banda de color de arriba de la pagina.
    #[serde(default = "si")]
    pub banda: bool,
}

fn propio() -> String {
    "propio".into()
}
fn si() -> bool {
    true
}

const TINTA: Color = Color::hex(0x22304a);
const ROTULO: Color = Color::hex(0x5d6b82);
const RAYA: Color = Color::hex(0xcfd7e3);
const BORDE: Color = Color::hex(0xb4c0d3);
const BLANCO: Color = Color::hex(0xffffff);

/// Los de serie: `(clave, acento, suave, letra del titulo)`.
const DE_SERIE: [(&str, u32, u32, &str); 6] = [
    ("planificacion", 0x3f6db3, 0xe8eff9, FRAUNCES),
    ("academico", 0x2c8a7f, 0xe3f3f0, FRAUNCES),
    ("creativo", 0xc0692b, 0xfbeee3, FRAUNCES),
    ("diario", 0xbf5878, 0xfbe9ef, CAVEAT),
    ("minimo", 0x22304a, 0xeef1f5, WORK_SANS),
    ("noche", 0x8fb3ff, 0x253047, FRAUNCES),
];

impl Tema {
    /// Las claves de los de serie, en el orden en que se ofrecen.
    pub fn claves() -> impl Iterator<Item = &'static str> {
        DE_SERIE.iter().map(|t| t.0)
    }

    /// **Uno de serie** por su clave (`None` si no existe).
    pub fn de_serie(clave: &str) -> Option<Tema> {
        let (base, acento, suave, titulo) = *DE_SERIE.iter().find(|t| t.0 == clave.trim())?;
        let mut t = Tema {
            base: base.into(),
            acento: Color::hex(acento),
            suave: Color::hex(suave),
            tinta: TINTA,
            rotulo: ROTULO,
            raya: RAYA,
            borde: BORDE,
            papel: BLANCO,
            letra_titulo: titulo.into(),
            letra_cuerpo: WORK_SANS.into(),
            grosor: 0.25,
            banda: true,
        };
        if base == "noche" {
            // Papel de noche (el de las plantillas «(noche)» de PinDesk).
            t.papel = Color::hex(0x161b26);
            t.tinta = Color::hex(0xe6ebf5);
            t.rotulo = Color::hex(0x9aa7bd);
            t.raya = Color::hex(0x323c50);
            t.borde = Color::hex(0x46526a);
        }
        if base == "minimo" {
            t.banda = false;
        }
        Some(t)
    }

    /// El nombre de la familia de una letra.
    pub fn familia(&self, l: Letra) -> &str {
        match l {
            Letra::Titulo => &self.letra_titulo,
            Letra::Cuerpo => &self.letra_cuerpo,
            Letra::Mano => CAVEAT,
        }
    }

    /// Si es de papel oscuro.
    pub fn oscuro(&self) -> bool {
        let [r, g, b, _] = self.papel.f32s();
        0.2126 * r + 0.7152 * g + 0.0722 * b < 0.5
    }
}

impl Default for Tema {
    fn default() -> Self {
        Tema::de_serie("planificacion").expect("el de serie existe")
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn colores_de_ida_y_vuelta() {
        assert_eq!(Color::leer("#3f6db3"), Some(Color::hex(0x3f6db3)));
        assert_eq!(Color::leer("fff"), Some(Color::hex(0xffffff)));
        assert_eq!(Color::leer("#11223344").unwrap().a, 0x44);
        assert_eq!(Color::leer("#12"), None);
        assert_eq!(Color::leer("#zzzzzz"), None);
        assert_eq!(Color::hex(0x0a0b0c).a_texto(), "#0a0b0c");
        let j = serde_json::to_string(&Tema::default()).unwrap();
        let t: Tema = serde_json::from_str(&j).unwrap();
        assert_eq!(t, Tema::default());
    }

    #[test]
    fn los_de_serie_y_la_noche() {
        assert_eq!(Tema::claves().count(), 6);
        assert!(Tema::de_serie("noche").unwrap().oscuro());
        assert!(!Tema::de_serie("diario").unwrap().oscuro());
        assert_eq!(Tema::de_serie("diario").unwrap().letra_titulo, CAVEAT);
        assert!(Tema::de_serie("rosa").is_none());
    }
}
