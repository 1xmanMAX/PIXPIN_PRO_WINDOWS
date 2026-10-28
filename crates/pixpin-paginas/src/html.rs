//! **Partir HTML en piezas**: texto, etiqueta que abre (con sus atributos)
//! y etiqueta que cierra. No es un parser de HTML5 entero: lo justo para
//! leer paginas reales (etiquetas sin cerrar, atributos sin comillas,
//! comentarios, `<script>` con `<` dentro) sin perderse ni colgarse. Nunca
//! falla: lo que no entiende es texto.

/// Una pieza del HTML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pieza<'a> {
    Texto(&'a str),
    Abre {
        /// En minusculas.
        nombre: String,
        /// Nombres en minusculas; valores con las entidades ya leidas.
        atributos: Vec<(String, String)>,
    },
    Cierra(String),
}

impl Pieza<'_> {
    /// El valor de un atributo de una etiqueta que abre.
    pub fn atributo(&self, nombre: &str) -> Option<&str> {
        match self {
            Pieza::Abre { atributos, .. } => atributos
                .iter()
                .find(|(n, _)| n == nombre)
                .map(|(_, v)| v.as_str()),
            _ => None,
        }
    }
}

/// Las etiquetas cuyo contenido es texto crudo hasta su cierre (puede
/// llevar `<` y `>` que no son etiquetas).
const CRUDAS: &[&str] = &["script", "style", "textarea", "xmp", "noscript"];

/// Las etiquetas que nunca tienen contenido.
pub const VACIAS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// **Parte el HTML.**
pub fn piezas(html: &str) -> Vec<Pieza<'_>> {
    let b = html.as_bytes();
    let mut v = Vec::new();
    let mut i = 0;
    let mut texto_desde = 0;
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        let siguiente = b.get(i + 1).copied().unwrap_or(0);
        let fin = if html[i..].starts_with("<!--") {
            // Comentario.
            Some(html[i + 4..].find("-->").map_or(b.len(), |f| i + 4 + f + 3))
        } else if siguiente == b'!' || siguiente == b'?' {
            // <!DOCTYPE>, <![CDATA[...]]>, <?xml?>.
            Some(html[i..].find('>').map_or(b.len(), |f| i + f + 1))
        } else if siguiente == b'/' && b.get(i + 2).is_some_and(u8::is_ascii_alphabetic) {
            let (nombre, f) = nombre_en(html, i + 2);
            let f = html[f..].find('>').map_or(b.len(), |g| f + g + 1);
            if texto_desde < i {
                v.push(Pieza::Texto(&html[texto_desde..i]));
            }
            v.push(Pieza::Cierra(nombre));
            texto_desde = f;
            i = f;
            continue;
        } else if siguiente.is_ascii_alphabetic() {
            let (nombre, f) = nombre_en(html, i + 1);
            let (atributos, f) = atributos_en(html, f);
            if texto_desde < i {
                v.push(Pieza::Texto(&html[texto_desde..i]));
            }
            let cruda = CRUDAS.contains(&nombre.as_str());
            v.push(Pieza::Abre {
                nombre: nombre.clone(),
                atributos,
            });
            i = f;
            texto_desde = f;
            if cruda {
                // Hasta `</nombre`, sin mirar lo de dentro.
                let cierre = format!("</{nombre}");
                let resto = &html[f..];
                let donde = buscar_sin_mayusculas(resto, &cierre).unwrap_or(resto.len());
                if donde > 0 {
                    v.push(Pieza::Texto(&resto[..donde]));
                }
                let tras = f + donde;
                let fin = html[tras..].find('>').map_or(b.len(), |g| tras + g + 1);
                v.push(Pieza::Cierra(nombre));
                i = fin;
                texto_desde = fin;
            }
            continue;
        } else {
            None
        };
        match fin {
            Some(f) => {
                if texto_desde < i {
                    v.push(Pieza::Texto(&html[texto_desde..i]));
                }
                i = f;
                texto_desde = f;
            }
            None => i += 1,
        }
    }
    if texto_desde < b.len() {
        v.push(Pieza::Texto(&html[texto_desde..]));
    }
    v
}

fn buscar_sin_mayusculas(texto: &str, aguja: &str) -> Option<usize> {
    let t = texto.as_bytes();
    let a = aguja.as_bytes();
    if a.len() > t.len() {
        return None;
    }
    (0..=t.len() - a.len()).find(|&i| t[i..i + a.len()].eq_ignore_ascii_case(a))
}

/// El nombre de una etiqueta desde `i`, y donde acaba.
fn nombre_en(html: &str, i: usize) -> (String, usize) {
    let b = html.as_bytes();
    let mut f = i;
    while f < b.len() && (b[f].is_ascii_alphanumeric() || matches!(b[f], b'-' | b':' | b'_')) {
        f += 1;
    }
    (html[i..f].to_ascii_lowercase(), f)
}

/// Los atributos desde `i` hasta el `>`, y donde acaba la etiqueta.
fn atributos_en(html: &str, mut i: usize) -> (Vec<(String, String)>, usize) {
    let b = html.as_bytes();
    let mut v = Vec::new();
    loop {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        if i >= b.len() {
            return (v, b.len());
        }
        if b[i] == b'>' {
            return (v, i + 1);
        }
        let ini = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && !matches!(b[i], b'=' | b'>' | b'/') {
            i += 1;
        }
        let nombre = html[ini..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut valor = String::new();
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                let ini = i + 1;
                let fin = html[ini..].find(q as char).map_or(b.len(), |f| ini + f);
                valor = entidades(&html[ini..fin]);
                i = (fin + 1).min(b.len());
            } else {
                let ini = i;
                while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' {
                    i += 1;
                }
                valor = entidades(&html[ini..i]);
            }
        }
        if !nombre.is_empty() {
            v.push((nombre, valor));
        }
    }
}

/// Las entidades con nombre que salen en paginas en espanol e ingles.
const NOMBRADAS: &[(&str, &str)] = &[
    ("amp", "&"),
    ("lt", "<"),
    ("gt", ">"),
    ("quot", "\""),
    ("apos", "'"),
    ("nbsp", " "),
    ("aacute", "á"),
    ("eacute", "é"),
    ("iacute", "í"),
    ("oacute", "ó"),
    ("uacute", "ú"),
    ("Aacute", "Á"),
    ("Eacute", "É"),
    ("Iacute", "Í"),
    ("Oacute", "Ó"),
    ("Uacute", "Ú"),
    ("ntilde", "ñ"),
    ("Ntilde", "Ñ"),
    ("uuml", "ü"),
    ("Uuml", "Ü"),
    ("ccedil", "ç"),
    ("agrave", "à"),
    ("egrave", "è"),
    ("ograve", "ò"),
    ("iexcl", "¡"),
    ("iquest", "¿"),
    ("laquo", "«"),
    ("raquo", "»"),
    ("ldquo", "“"),
    ("rdquo", "”"),
    ("lsquo", "‘"),
    ("rsquo", "’"),
    ("ndash", "–"),
    ("mdash", "—"),
    ("hellip", "…"),
    ("middot", "·"),
    ("bull", "•"),
    ("copy", "©"),
    ("reg", "®"),
    ("trade", "™"),
    ("euro", "€"),
    ("pound", "£"),
    ("deg", "°"),
    ("ordm", "º"),
    ("ordf", "ª"),
    ("times", "×"),
    ("divide", "÷"),
    ("sect", "§"),
    ("para", "¶"),
    ("shy", ""),
    ("zwnj", ""),
    ("zwj", ""),
];

/// **Lee las entidades** (`&amp;`, `&#225;`, `&#xE1;`, `&aacute;`). Las que
/// no conoce quedan tal cual.
pub fn entidades(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut o = String::with_capacity(s.len());
    let mut resto = s;
    while let Some(i) = resto.find('&') {
        o.push_str(&resto[..i]);
        let tras = &resto[i + 1..];
        let fin = tras
            .char_indices()
            .take(12)
            .find(|(_, c)| *c == ';' || !(c.is_ascii_alphanumeric() || *c == '#'))
            .map(|(j, c)| (j, c == ';'))
            .or_else(|| (tras.len() <= 12).then_some((tras.len(), false)));
        let leida = fin.and_then(|(j, con_punto)| {
            let nombre = &tras[..j];
            let valor = if let Some(n) = nombre.strip_prefix('#') {
                let cp = if let Some(h) = n.strip_prefix(['x', 'X']) {
                    u32::from_str_radix(h, 16).ok()
                } else {
                    n.parse::<u32>().ok()
                }?;
                let c = match cp {
                    0 => '\u{FFFD}',
                    // Lo que las paginas viejas mandan como Windows-1252.
                    0x80..=0x9F => crate::cp1252(cp as u8),
                    _ => char::from_u32(cp).unwrap_or('\u{FFFD}'),
                };
                c.to_string()
            } else {
                NOMBRADAS
                    .iter()
                    .find(|(n, _)| *n == nombre)
                    .map(|(_, v)| v.to_string())?
            };
            Some((valor, j + usize::from(con_punto)))
        });
        match leida {
            Some((valor, largo)) => {
                o.push_str(&valor);
                resto = &tras[largo..];
            }
            None => {
                o.push('&');
                resto = tras;
            }
        }
    }
    o.push_str(resto);
    o
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn parte_etiquetas_atributos_y_texto() {
        let p = piezas(r#"<p class=a id="b">Hola <a href='/x?a=1&amp;b=2'>mundo</a><br/></p>"#);
        assert_eq!(p.len(), 7, "{p:?}");
        assert_eq!(p[0].atributo("class"), Some("a"));
        assert_eq!(p[0].atributo("id"), Some("b"));
        assert_eq!(p[1], Pieza::Texto("Hola "));
        assert_eq!(p[2].atributo("href"), Some("/x?a=1&b=2"));
        assert_eq!(p[4], Pieza::Cierra("a".into()));
        assert!(matches!(&p[5], Pieza::Abre { nombre, .. } if nombre == "br"));
    }

    #[test]
    fn el_script_es_crudo_y_los_comentarios_no_salen() {
        let p = piezas("<SCRIPT>if (a<b) { x='</p>' }</script><!-- <b>no</b> -->fin");
        assert!(matches!(&p[0], Pieza::Abre { nombre, .. } if nombre == "script"));
        assert_eq!(p[1], Pieza::Texto("if (a<b) { x='</p>' }"));
        // `</p>` dentro de una cadena no cierra el script, pero su `</script`
        // si (como los navegadores).
        assert!(p.contains(&Pieza::Texto("fin")));
        assert!(
            !p.iter()
                .any(|x| matches!(x, Pieza::Texto(t) if t.contains("no")))
        );
    }

    #[test]
    fn lo_roto_no_cuelga() {
        for raro in [
            "<",
            "<a",
            "<a href=",
            "<a href='x",
            "</",
            "<!--",
            "a < b",
            "<script>x",
            "&#xZZ;",
        ] {
            let _ = piezas(raro);
        }
        assert_eq!(piezas("a < b"), vec![Pieza::Texto("a < b")]);
    }

    #[test]
    fn lee_las_entidades() {
        assert_eq!(
            entidades("Matem&aacute;ticas &amp; F&#237;sica &#x2013; 2&ordm;"),
            "Matemáticas & Física – 2º"
        );
        assert_eq!(entidades("&#150;"), "–");
        assert_eq!(entidades("a &desconocida; b & c"), "a &desconocida; b & c");
        assert_eq!(entidades("&nbsp"), " ");
    }
}
