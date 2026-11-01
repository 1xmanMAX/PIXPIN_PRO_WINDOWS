//! El pin de color: copiar `#ff8800`, `rgb(255, 136, 0)` o `orange` y
//! pinearlo da una muestra del color, que se copia de un toque.
//!
//! Puerto regla a regla de `ContentClassifier.parseColor` del movil (y de sus
//! pruebas, `ContentClassifierTest.kt`): los mismos formatos, los mismos
//! nombres CSS con los mismos valores —que son los de Material, no los del
//! estandar: «red» es `#F44336`— y la misma salida en mayusculas. Puro: se
//! prueba sin escritorio.
//!
//! El color va en ARGB empaquetado en un `u32`, como el `Int` del movil.

/// El color como ARGB, o `None` si el texto no es un color.
///
/// Acepta `#rgb`, `#rrggbb`, `#aarrggbb`, `rgb(r, g, b)`, `rgba(r, g, b, a)`
/// (con `a` de 0 a 1 si lleva punto, de 0 a 255 si no), tres numeros sueltos
/// `r, g, b` y los nombres CSS de la lista. Sin espacios alrededor: quien
/// llama ya recorta.
pub fn leer(entrada: &str) -> Option<u32> {
    if let Some(hex) = entrada.strip_prefix('#') {
        return de_hex(hex);
    }
    if let Some(c) = de_funcion_rgb(entrada) {
        return c;
    }
    if let Some(c) = de_triple(entrada) {
        return c;
    }
    NOMBRES_CSS
        .iter()
        .find(|(n, _)| *n == entrada.to_lowercase())
        .map(|(_, c)| *c)
}

/// `#RRGGBB` en mayusculas, sin el alfa (`toHex`).
pub fn a_hex(argb: u32) -> String {
    format!("#{:06X}", argb & 0x00FF_FFFF)
}

/// `rgb(r, g, b)` (`toRgb`).
pub fn a_rgb(argb: u32) -> String {
    let (r, g, b) = componentes(argb);
    format!("rgb({r}, {g}, {b})")
}

/// Rojo, verde y azul, de 0 a 255.
pub fn componentes(argb: u32) -> (u8, u8, u8) {
    ((argb >> 16) as u8, (argb >> 8) as u8, argb as u8)
}

/// El alfa, de 0 a 255.
pub fn alfa(argb: u32) -> u8 {
    (argb >> 24) as u8
}

/// Si el color es claro: decide si lo que se escribe encima va en negro o
/// en blanco. Luminancia relativa aproximada, sin gamma: basta para elegir
/// entre dos tintas.
pub fn es_claro(argb: u32) -> bool {
    let (r, g, b) = componentes(argb);
    0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32 > 150.0
}

fn de_hex(hex: &str) -> Option<u32> {
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    match hex.len() {
        3 => {
            // Cada cifra se dobla: #28B es #2288BB.
            let mut v = 0u32;
            for c in hex.chars() {
                let d = c.to_digit(16)?;
                v = (v << 8) | (d * 17);
            }
            Some(0xFF00_0000 | v)
        }
        6 => u32::from_str_radix(hex, 16).ok().map(|v| 0xFF00_0000 | v),
        8 => u32::from_str_radix(hex, 16).ok(),
        _ => None,
    }
}

/// Un numero de una a tres cifras, como `\d{1,3}` del movil.
fn numero(s: &str) -> Option<u32> {
    let s = s.trim();
    if s.is_empty() || s.len() > 3 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// `rgb(...)` o `rgba(...)`. El `Option` de fuera dice si tenia esa forma;
/// el de dentro, si ademas era un color (un 300 la tiene y no lo es).
fn de_funcion_rgb(entrada: &str) -> Option<Option<u32>> {
    let dentro = entrada
        .strip_prefix("rgba")
        .or_else(|| entrada.strip_prefix("rgb"))?
        .strip_prefix('(')?
        .strip_suffix(')')?;
    let partes: Vec<&str> = dentro.split(',').collect();
    if !(3..=4).contains(&partes.len()) {
        return None;
    }
    let (r, g, b) = (
        numero(partes[0])?,
        numero(partes[1])?,
        numero(partes[2])?,
    );
    let a = match partes.get(3).map(|a| a.trim()) {
        None => 255,
        Some(a) => alfa_de(a)?,
    };
    if r > 255 || g > 255 || b > 255 {
        return Some(None);
    }
    Some(Some((a << 24) | (r << 16) | (g << 8) | b))
}

/// El alfa de `rgba`: `0.5` es la mitad (se trunca, como `toInt()`), `128`
/// es 128. Lo que no tenga la forma `\d?\.?\d+` no vale.
fn alfa_de(a: &str) -> Option<u32> {
    let (ent, dec) = match a.split_once('.') {
        Some((e, d)) => (e, Some(d)),
        None => (a, None),
    };
    let solo_cifras = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    match dec {
        Some(d) => {
            if ent.len() > 1 || !solo_cifras(ent) || d.is_empty() || !solo_cifras(d) {
                return None;
            }
            let f: f32 = a.parse().ok()?;
            Some((f.clamp(0.0, 1.0) * 255.0) as u32)
        }
        // Sin punto, cualquier tira de cifras; la que no cabe en un numero
        // (`toIntOrNull` nulo) se queda en opaco, como en el movil.
        None => {
            if ent.is_empty() || !solo_cifras(ent) {
                return None;
            }
            Some(ent.parse::<u32>().map_or(255, |n| n.min(255)))
        }
    }
}

/// `41, 184, 219` sueltos.
fn de_triple(entrada: &str) -> Option<Option<u32>> {
    let partes: Vec<&str> = entrada.split(',').collect();
    if partes.len() != 3 {
        return None;
    }
    // Sin blancos en los extremos de la cadena entera (`^...$`): por
    // dentro, alrededor de las comas, si.
    if partes[0].starts_with(char::is_whitespace) || partes[2].ends_with(char::is_whitespace) {
        return None;
    }
    let (r, g, b) = (numero(partes[0])?, numero(partes[1])?, numero(partes[2])?);
    if r > 255 || g > 255 || b > 255 {
        return Some(None);
    }
    Some(Some(0xFF00_0000 | (r << 16) | (g << 8) | b))
}

/// Los nombres CSS del movil, con SUS valores (`cssColorNames`).
const NOMBRES_CSS: [(&str, u32); 30] = [
    ("white", 0xFFFFFFFF),
    ("black", 0xFF000000),
    ("red", 0xFFF44336),
    ("orange", 0xFFFF9800),
    ("yellow", 0xFFFFEB3B),
    ("green", 0xFF4CAF50),
    ("teal", 0xFF009688),
    ("cyan", 0xFF00BCD4),
    ("blue", 0xFF2196F3),
    ("indigo", 0xFF3F51B5),
    ("purple", 0xFF9C27B0),
    ("pink", 0xFFE91E63),
    ("brown", 0xFF795548),
    ("gray", 0xFF9E9E9E),
    ("grey", 0xFF9E9E9E),
    ("magenta", 0xFFFF00FF),
    ("lime", 0xFFCDDC39),
    ("olive", 0xFF808000),
    ("navy", 0xFF000080),
    ("maroon", 0xFF800000),
    ("aqua", 0xFF00FFFF),
    ("silver", 0xFFC0C0C0),
    ("gold", 0xFFFFD700),
    ("coral", 0xFFFF7F50),
    ("salmon", 0xFFFA8072),
    ("khaki", 0xFFF0E68C),
    ("violet", 0xFFEE82EE),
    ("turquoise", 0xFF40E0D0),
    ("crimson", 0xFFDC143C),
    ("tomato", 0xFFFF6347),
];

#[cfg(test)]
mod pruebas {
    //! Los casos de `ContentClassifierTest.kt`, uno a uno.
    use super::*;

    #[test]
    fn hex_de_6_digitos() {
        assert_eq!(leer("#29B8DB"), Some(0xFF29B8DB));
    }

    #[test]
    fn hex_de_3_digitos_se_expande() {
        assert_eq!(leer("#28B"), Some(0xFF2288BB));
    }

    #[test]
    fn hex_de_8_digitos_con_alpha() {
        assert_eq!(leer("#8029B8DB"), Some(0x8029B8DB));
    }

    #[test]
    fn funcion_rgb() {
        assert_eq!(leer("rgb(41,184,219)"), Some(0xFF29B8DB));
        assert_eq!(leer("rgb(41, 184, 219)"), Some(0xFF29B8DB));
    }

    #[test]
    fn funcion_rgba_con_alpha_decimal() {
        assert_eq!(leer("rgba(17, 34, 51, 0.5)"), Some(0x7F112233));
    }

    #[test]
    fn triples_sueltos() {
        assert_eq!(leer("41, 184, 219"), Some(0xFF29B8DB));
    }

    #[test]
    fn nombres_css() {
        assert_eq!(leer("orange"), Some(0xFFFF9800));
        assert_eq!(leer("BLUE"), Some(0xFF2196F3));
    }

    #[test]
    fn no_es_color() {
        assert_eq!(leer("hola mundo"), None);
        assert_eq!(leer("#12345"), None);
        assert_eq!(leer("rgb(300, 0, 0)"), None);
        assert_eq!(leer("1, 2, 300"), None);
        assert_eq!(leer(""), None);
        // Y los nuestros: letras que no son cifras, parentesis sin cerrar,
        // cuatro numeros sueltos, un numero de cuatro cifras.
        assert_eq!(leer("#GGGGGG"), None);
        assert_eq!(leer("rgb(1, 2, 3"), None);
        assert_eq!(leer("1, 2, 3, 4"), None);
        assert_eq!(leer("1000, 2, 3"), None);
        assert_eq!(leer("#"), None);
    }

    #[test]
    fn formatos_de_salida() {
        assert_eq!(a_hex(0xFF29B8DB), "#29B8DB");
        assert_eq!(a_rgb(0xFF29B8DB), "rgb(41, 184, 219)");
        // El alfa no sale en el hex, como en el movil.
        assert_eq!(a_hex(0x8029B8DB), "#29B8DB");
    }

    #[test]
    fn rgba_con_alfa_entero_y_fuera_de_rango() {
        assert_eq!(leer("rgba(0, 0, 0, 128)"), Some(0x80000000));
        // `1.5` se queda en 1: opaco.
        assert_eq!(leer("rgba(0, 0, 0, 1.5)"), Some(0xFF000000));
        assert_eq!(leer("rgba(0, 0, 0, 1000)"), Some(0xFF000000));
        assert_eq!(leer("rgba(0, 0, 0, .5)"), Some(0x7F000000));
        assert_eq!(leer("rgba(0, 0, 0, 0.)"), None);
    }

    #[test]
    fn claro_y_oscuro() {
        assert!(es_claro(0xFFFFEB3B));
        assert!(!es_claro(0xFF000080));
    }
}
