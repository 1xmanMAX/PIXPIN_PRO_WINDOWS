//! Markdown para las notas, puro (lo pidio el usuario: «si copio Markdown
//! que se represente»).
//!
//! Un subconjunto util y predecible: titulos `#`, listas `-`/`*`/`1.`,
//! citas `>`, bloques de codigo con vallas, reglas `---`, y en linea
//! `**negrita**`, `*cursiva*`, `` `codigo` `` y `[texto](enlace)`. Todo lo
//! demas es texto tal cual: una nota normal se ve como siempre. Las lineas
//! de un parrafo se conservan (una nota es texto pegado, no un documento):
//! `analizar` NO junta lineas como haria un renderizador estricto.
//!
//! Este modulo no dibuja: produce bloques con texto plano y tramos de
//! estilo, y `disponer` los coloca con un medidor que le presta la ventana.

use pixpin_render::{EstiloTexto, Tramo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tipo {
    Parrafo,
    /// Nivel 1..=6.
    Titulo(u8),
    Vineta,
    Numerada(u32),
    Cita,
    /// Codigo con vallas: monoespaciado, sin estilos en linea.
    Codigo,
    Regla,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bloque {
    pub tipo: Tipo,
    /// El texto ya sin marcas.
    pub texto: String,
    /// Tramos de estilo sobre `texto`, en unidades UTF-16.
    pub tramos: Vec<Tramo>,
}

/// Parte el texto en bloques.
pub fn analizar(texto: &str) -> Vec<Bloque> {
    let mut bloques: Vec<Bloque> = Vec::new();
    let mut parrafo: Vec<String> = Vec::new();
    let mut codigo: Option<Vec<String>> = None;

    let cerrar_parrafo = |parrafo: &mut Vec<String>, bloques: &mut Vec<Bloque>| {
        if parrafo.is_empty() {
            return;
        }
        let (texto, tramos) = en_linea(&parrafo.join("\n"));
        parrafo.clear();
        bloques.push(Bloque {
            tipo: Tipo::Parrafo,
            texto,
            tramos,
        });
    };

    for linea in texto.lines() {
        let linea = linea.trim_end_matches('\r');
        if let Some(lineas) = codigo.as_mut() {
            if linea.trim_start().starts_with("```") {
                let texto = lineas.join("\n");
                codigo = None;
                bloques.push(Bloque {
                    tipo: Tipo::Codigo,
                    texto,
                    tramos: Vec::new(),
                });
            } else {
                lineas.push(linea.to_string());
            }
            continue;
        }
        let recortada = linea.trim_start();
        if recortada.starts_with("```") {
            cerrar_parrafo(&mut parrafo, &mut bloques);
            codigo = Some(Vec::new());
            continue;
        }
        if recortada.trim().is_empty() {
            cerrar_parrafo(&mut parrafo, &mut bloques);
            continue;
        }
        if es_regla(recortada) {
            cerrar_parrafo(&mut parrafo, &mut bloques);
            bloques.push(Bloque {
                tipo: Tipo::Regla,
                texto: String::new(),
                tramos: Vec::new(),
            });
            continue;
        }
        if let Some((nivel, resto)) = titulo(recortada) {
            cerrar_parrafo(&mut parrafo, &mut bloques);
            let (texto, tramos) = en_linea(resto);
            bloques.push(Bloque {
                tipo: Tipo::Titulo(nivel),
                texto,
                tramos,
            });
            continue;
        }
        if let Some(resto) = recortada
            .strip_prefix("- ")
            .or_else(|| recortada.strip_prefix("* "))
            .or_else(|| recortada.strip_prefix("+ "))
        {
            cerrar_parrafo(&mut parrafo, &mut bloques);
            let (texto, tramos) = en_linea(resto);
            bloques.push(Bloque {
                tipo: Tipo::Vineta,
                texto,
                tramos,
            });
            continue;
        }
        if let Some((n, resto)) = numerada(recortada) {
            cerrar_parrafo(&mut parrafo, &mut bloques);
            let (texto, tramos) = en_linea(resto);
            bloques.push(Bloque {
                tipo: Tipo::Numerada(n),
                texto,
                tramos,
            });
            continue;
        }
        if let Some(resto) = recortada.strip_prefix('>') {
            cerrar_parrafo(&mut parrafo, &mut bloques);
            let (texto, tramos) = en_linea(resto.trim_start());
            // Citas seguidas se juntan en una.
            if let Some(ultimo) = bloques.last_mut().filter(|b| b.tipo == Tipo::Cita) {
                let base = ultimo.texto.encode_utf16().count() as u32 + 1;
                ultimo.texto.push('\n');
                ultimo.texto.push_str(&texto);
                ultimo.tramos.extend(tramos.into_iter().map(|t| Tramo {
                    inicio: t.inicio + base,
                    ..t
                }));
            } else {
                bloques.push(Bloque {
                    tipo: Tipo::Cita,
                    texto,
                    tramos,
                });
            }
            continue;
        }
        parrafo.push(linea.to_string());
    }
    if let Some(lineas) = codigo {
        // Valla sin cerrar: el codigo vale igual.
        bloques.push(Bloque {
            tipo: Tipo::Codigo,
            texto: lineas.join("\n"),
            tramos: Vec::new(),
        });
    }
    cerrar_parrafo(&mut parrafo, &mut bloques);
    bloques
}

fn es_regla(l: &str) -> bool {
    let l = l.trim();
    l.len() >= 3 && (l.chars().all(|c| c == '-') || l.chars().all(|c| c == '*'))
}

fn titulo(l: &str) -> Option<(u8, &str)> {
    let nivel = l.chars().take_while(|c| *c == '#').count();
    if nivel == 0 || nivel > 6 {
        return None;
    }
    let resto = &l[nivel..];
    resto
        .strip_prefix(' ')
        .map(|r| (nivel as u8, r.trim_end_matches(['#', ' '])))
}

fn numerada(l: &str) -> Option<(u32, &str)> {
    let digitos = l.chars().take_while(|c| c.is_ascii_digit()).count();
    if digitos == 0 || digitos > 4 {
        return None;
    }
    let n: u32 = l[..digitos].parse().ok()?;
    let resto = &l[digitos..];
    resto
        .strip_prefix(". ")
        .or_else(|| resto.strip_prefix(") "))
        .map(|r| (n, r))
}

/// Marcas en linea: `**`/`__` negrita, `*`/`_` cursiva, `` ` `` codigo,
/// `[texto](enlace)` deja el texto, `\` escapa la marca siguiente.
pub fn en_linea(entrada: &str) -> (String, Vec<Tramo>) {
    let cs: Vec<char> = entrada.chars().collect();
    let mut salida = String::new();
    let mut tramos: Vec<Tramo> = Vec::new();
    let mut pos_u16: u32 = 0;
    let mut i = 0;
    // Aperturas pendientes: (posicion utf16 de inicio, estilo).
    let mut negrita: Option<u32> = None;
    let mut cursiva: Option<u32> = None;

    let empujar = |salida: &mut String, pos: &mut u32, c: char| {
        salida.push(c);
        *pos += c.len_utf16() as u32;
    };

    while i < cs.len() {
        let c = cs[i];
        let siguiente = cs.get(i + 1).copied();
        if c == '\\' && siguiente.is_some_and(|s| "*_`[]\\#>-".contains(s)) {
            empujar(&mut salida, &mut pos_u16, siguiente.unwrap_or(' '));
            i += 2;
            continue;
        }
        if c == '`' {
            // Codigo en linea hasta la siguiente comilla; sin cierre, la
            // comilla es literal.
            if let Some(fin) = cs[i + 1..].iter().position(|x| *x == '`') {
                let inicio = pos_u16;
                for x in &cs[i + 1..i + 1 + fin] {
                    empujar(&mut salida, &mut pos_u16, *x);
                }
                tramos.push(Tramo {
                    inicio,
                    longitud: pos_u16 - inicio,
                    estilo: EstiloTexto {
                        mono: true,
                        ..Default::default()
                    },
                });
                i += fin + 2;
                continue;
            }
        }
        if c == '[' {
            // [texto](enlace): se queda el texto.
            if let Some(cierre) = cs[i..].iter().position(|x| *x == ']') {
                let j = i + cierre;
                if cs.get(j + 1) == Some(&'(') {
                    if let Some(fin) = cs[j + 1..].iter().position(|x| *x == ')') {
                        for x in &cs[i + 1..j] {
                            empujar(&mut salida, &mut pos_u16, *x);
                        }
                        i = j + 1 + fin + 1;
                        continue;
                    }
                }
            }
        }
        let doble = (c == '*' || c == '_') && siguiente == Some(c);
        if doble {
            match negrita.take() {
                Some(inicio) => tramos.push(Tramo {
                    inicio,
                    longitud: pos_u16 - inicio,
                    estilo: EstiloTexto {
                        negrita: true,
                        ..Default::default()
                    },
                }),
                None => negrita = Some(pos_u16),
            }
            i += 2;
            continue;
        }
        if c == '*' || (c == '_' && marca_valida(&cs, i)) {
            match cursiva.take() {
                Some(inicio) => tramos.push(Tramo {
                    inicio,
                    longitud: pos_u16 - inicio,
                    estilo: EstiloTexto {
                        cursiva: true,
                        ..Default::default()
                    },
                }),
                None => cursiva = Some(pos_u16),
            }
            i += 1;
            continue;
        }
        empujar(&mut salida, &mut pos_u16, c);
        i += 1;
    }
    // Marcas sin cierre: eran texto. Se reponen donde estaban... no: se
    // habrian quitado del texto. Mas simple y honesto: el texto ya salio
    // sin ellas; los tramos abiertos se descartan.
    tramos.retain(|t| t.longitud > 0);
    tramos.sort_by_key(|t| t.inicio);
    (salida, tramos)
}

/// Un `_` solo marca cursiva si esta pegado a una palabra por un lado y no
/// por los dos (`snake_case` no es cursiva).
fn marca_valida(cs: &[char], i: usize) -> bool {
    let antes = i.checked_sub(1).and_then(|k| cs.get(k)).copied();
    let despues = cs.get(i + 1).copied();
    let palabra = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric());
    !(palabra(antes) && palabra(despues))
}

/// Una linea colocada: que bloque, donde y a que tamano, con su prefijo
/// (vineta o numero) si lo tiene.
#[derive(Debug, Clone, PartialEq)]
pub struct Colocado {
    pub bloque: usize,
    pub x: f32,
    pub y: f32,
    pub tam: f32,
    pub ancho: f32,
    pub alto: f32,
    pub prefijo: Option<String>,
    pub prefijo_x: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Disposicion {
    pub colocados: Vec<Colocado>,
    pub ancho: f32,
    pub alto: f32,
}

/// Factor de tamano de cada tipo respecto al texto base.
pub fn factor_tam(tipo: &Tipo) -> f32 {
    match tipo {
        Tipo::Titulo(1) => 1.6,
        Tipo::Titulo(2) => 1.4,
        Tipo::Titulo(3) => 1.2,
        Tipo::Titulo(_) => 1.1,
        Tipo::Codigo => 0.92,
        _ => 1.0,
    }
}

/// Tramos efectivos de un bloque: los titulos van en negrita enteros y el
/// codigo en mono entero, ademas de lo que traigan.
pub fn tramos_de(b: &Bloque) -> Vec<Tramo> {
    let largo = b.texto.encode_utf16().count() as u32;
    let mut v = b.tramos.clone();
    match b.tipo {
        Tipo::Titulo(_) => v.push(Tramo {
            inicio: 0,
            longitud: largo,
            estilo: EstiloTexto {
                negrita: true,
                ..Default::default()
            },
        }),
        Tipo::Codigo => v.push(Tramo {
            inicio: 0,
            longitud: largo,
            estilo: EstiloTexto {
                mono: true,
                ..Default::default()
            },
        }),
        _ => {}
    }
    v
}

/// Quien mide un parrafo: (texto, cuerpo, ancho maximo, tramos) -> (ancho, alto).
pub type Medidor<'a> = dyn Fn(&str, f32, f32, &[Tramo]) -> (f32, f32) + 'a;

/// Coloca los bloques en una columna de `ancho_max` con texto base `tam`.
/// `medidor(texto, tam, ancho_max, tramos)` devuelve (ancho, alto).
pub fn disponer(bloques: &[Bloque], ancho_max: f32, tam: f32, medidor: &Medidor) -> Disposicion {
    let mut colocados = Vec::new();
    let mut y = 0.0f32;
    let mut ancho_total = 0.0f32;
    let hueco = tam * 0.5;
    for (i, b) in bloques.iter().enumerate() {
        if i > 0 {
            y += hueco;
        }
        let t = tam * factor_tam(&b.tipo);
        let (sangria, prefijo) = match &b.tipo {
            Tipo::Vineta => (tam * 1.4, Some("•".to_string())),
            Tipo::Numerada(n) => (tam * 1.8, Some(format!("{n}."))),
            Tipo::Cita => (tam * 1.0, None),
            Tipo::Codigo => (tam * 0.6, None),
            _ => (0.0, None),
        };
        if b.tipo == Tipo::Regla {
            colocados.push(Colocado {
                bloque: i,
                x: 0.0,
                y,
                tam: t,
                ancho: ancho_max,
                alto: tam,
                prefijo: None,
                prefijo_x: 0.0,
            });
            y += tam;
            ancho_total = ancho_total.max(ancho_max.min(tam * 6.0));
            continue;
        }
        let ancho_texto = (ancho_max - sangria).max(1.0);
        let (w, h) = medidor(&b.texto, t, ancho_texto, &tramos_de(b));
        let h = h.max(t * 1.2);
        let relleno = if b.tipo == Tipo::Codigo {
            tam * 0.4
        } else {
            0.0
        };
        colocados.push(Colocado {
            bloque: i,
            x: sangria,
            y: y + relleno,
            tam: t,
            ancho: w,
            alto: h,
            prefijo,
            prefijo_x: sangria * 0.25,
        });
        y += h + 2.0 * relleno;
        ancho_total = ancho_total.max(
            sangria
                + w
                + (if b.tipo == Tipo::Codigo {
                    tam * 0.6
                } else {
                    0.0
                }),
        );
    }
    Disposicion {
        colocados,
        ancho: ancho_total,
        alto: y,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn medidor_fijo(texto: &str, tam: f32, ancho_max: f32, _: &[Tramo]) -> (f32, f32) {
        // Cada caracter mide medio `tam`; las lineas se parten al ancho.
        let por_linea = ((ancho_max / (tam * 0.5)).floor() as usize).max(1);
        let mut lineas = 0usize;
        let mut ancho = 0.0f32;
        for l in texto.split('\n') {
            let n = l.chars().count().max(1);
            lineas += n.div_ceil(por_linea);
            ancho = ancho.max((n.min(por_linea) as f32) * tam * 0.5);
        }
        (ancho, lineas as f32 * tam * 1.2)
    }

    #[test]
    fn el_texto_plano_es_un_parrafo_con_sus_saltos() {
        let b = analizar("hola\nmundo");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].tipo, Tipo::Parrafo);
        assert_eq!(b[0].texto, "hola\nmundo");
        assert!(b[0].tramos.is_empty());
    }

    #[test]
    fn titulos_listas_citas_codigo_y_regla() {
        let b = analizar(
            "# Titulo\n\ntexto\n- uno\n- dos\n1. primero\n> cita\n> sigue\n---\n```\nlet x = 1;\n```",
        );
        let tipos: Vec<&Tipo> = b.iter().map(|x| &x.tipo).collect();
        assert_eq!(
            tipos,
            vec![
                &Tipo::Titulo(1),
                &Tipo::Parrafo,
                &Tipo::Vineta,
                &Tipo::Vineta,
                &Tipo::Numerada(1),
                &Tipo::Cita,
                &Tipo::Regla,
                &Tipo::Codigo
            ]
        );
        assert_eq!(b[0].texto, "Titulo");
        assert_eq!(b[5].texto, "cita\nsigue", "las citas seguidas se juntan");
        assert_eq!(b[7].texto, "let x = 1;");
    }

    #[test]
    fn negrita_cursiva_codigo_y_enlace_en_linea() {
        let (t, tramos) = en_linea("a **b** *c* `d` [e](http://x)");
        assert_eq!(t, "a b c d e");
        assert_eq!(tramos.len(), 3);
        assert_eq!((tramos[0].inicio, tramos[0].longitud), (2, 1));
        assert!(tramos[0].estilo.negrita);
        assert_eq!((tramos[1].inicio, tramos[1].longitud), (4, 1));
        assert!(tramos[1].estilo.cursiva);
        assert_eq!((tramos[2].inicio, tramos[2].longitud), (6, 1));
        assert!(tramos[2].estilo.mono);
    }

    #[test]
    fn las_posiciones_van_en_utf16() {
        // «😀» ocupa dos unidades UTF-16: la negrita que va detras empieza
        // en 3, no en 2.
        let (t, tramos) = en_linea("😀 **b**");
        assert_eq!(t, "😀 b");
        assert_eq!((tramos[0].inicio, tramos[0].longitud), (3, 1));
    }

    #[test]
    fn el_guion_bajo_dentro_de_una_palabra_no_es_cursiva() {
        let (t, tramos) = en_linea("snake_case_aqui");
        assert_eq!(t, "snake_case_aqui");
        assert!(tramos.is_empty());
    }

    #[test]
    fn una_marca_sin_cierre_no_deja_tramo() {
        let (t, tramos) = en_linea("a **b");
        assert_eq!(t, "a b");
        assert!(tramos.is_empty());
    }

    #[test]
    fn disponer_apila_los_bloques_con_hueco_y_sangria() {
        let b = analizar("# T\n\n- uno\n- dos");
        let d = disponer(&b, 200.0, 10.0, &medidor_fijo);
        assert_eq!(d.colocados.len(), 3);
        assert_eq!(d.colocados[0].tam, 16.0, "el titulo 1 va a 1,6x");
        assert_eq!(d.colocados[1].x, 14.0, "la vineta lleva sangria");
        assert_eq!(d.colocados[1].prefijo.as_deref(), Some("•"));
        assert!(d.colocados[1].y > d.colocados[0].y + d.colocados[0].alto);
        assert!(d.alto > 0.0 && d.ancho > 0.0);
    }
}
