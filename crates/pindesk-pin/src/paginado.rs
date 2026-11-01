//! La nota pineada en paginas, como las de un PDF (`motormd/Paginado.kt` y
//! `motormd/Trozos.kt` del movil).
//!
//! Una nota larga en un pin es un rollo: no se sabe por donde vas ni cuanto
//! queda, y no hay forma de saltar a un sitio. Con paginas se maneja como un
//! PDF. Es algo que se ENCIENDE (menu «Páginas»), no el modo unico: leer
//! seguido sigue siendo lo de siempre.
//!
//! El corte se cuenta en **renglones**, no en pixeles: medir de verdad
//! exigiria pintar la nota, y asi el corte es el mismo se mire donde se mire
//! (en el movil y aqui). Cada bloque dice cuantos renglones ocupa mas o menos
//! y se van sumando. Un bloque **no se parte**: si no cabe, empieza la pagina
//! siguiente, salvo que el solo sea mas grande que una pagina entera, que
//! entonces se queda solo en la suya. Y una tabla grande empieza pagina.
//!
//! Las cuentas son las del movil; los tipos de bloque, los del Markdown de
//! las notas del PC (`crate::markdown`), que no tiene cajas ni casillas: una
//! tabla y una imagen se reconocen en el propio trozo de texto.

use crate::markdown::{self, Tipo};

/// Cuantos renglones caben en una pagina: una hoja A4 con margen.
pub const RENGLONES_POR_PAGINA: usize = 46;

/// Cuantos caracteres caben en un renglon, para estimar lo que dobla.
const ANCHO_EN_LETRAS: usize = 78;

/// Un bloque del documento, dicho en posiciones (bytes) del texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trozo {
    pub desde: usize,
    pub hasta: usize,
}

/// Una pagina con su sitio en el texto: de `desde` a `hasta` (bytes del
/// texto normalizado, ver [`normalizar`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pagina {
    pub desde: usize,
    pub hasta: usize,
}

/// El texto con los saltos `\r\n` hechos `\n`: las posiciones de las
/// paginas son sobre este texto, el mismo que se pinta.
pub fn normalizar(texto: &str) -> std::borrow::Cow<'_, str> {
    if texto.contains('\r') {
        std::borrow::Cow::Owned(texto.replace("\r\n", "\n"))
    } else {
        std::borrow::Cow::Borrowed(texto)
    }
}

/// Parte el documento en bloques **sin interpretarlos** (`trozosDe`): los
/// trozos cubren el texto entero, no se pisan, y una sola linea en blanco se
/// queda con el bloque de arriba.
pub fn trozos_de(texto: &str) -> Vec<Trozo> {
    if texto.is_empty() {
        return Vec::new();
    }
    let lineas: Vec<&str> = texto.split('\n').collect();
    let mut inicios = Vec::with_capacity(lineas.len());
    let mut acumulado = 0;
    for l in &lineas {
        inicios.push(acumulado);
        acumulado += l.len() + 1;
    }
    let fin_de_linea = |i: usize| inicios[i] + lineas[i].len();
    let mut salida = Vec::new();
    let mut i = 0;
    while i < lineas.len() {
        let desde = inicios[i];
        let t = lineas[i].trim();
        let ultima;
        if t.starts_with("```") {
            i += 1;
            while i < lineas.len() && !lineas[i].trim().starts_with("```") {
                i += 1;
            }
            if i < lineas.len() {
                i += 1;
            }
            ultima = i - 1;
        } else if abre_caja(t) {
            let mut nivel = 1;
            i += 1;
            while i < lineas.len() {
                let l = lineas[i].trim();
                if abre_caja(l) {
                    nivel += 1;
                }
                if l == ":::" {
                    nivel -= 1;
                    if nivel == 0 {
                        break;
                    }
                }
                i += 1;
            }
            if i < lineas.len() {
                i += 1;
            }
            ultima = i - 1;
        } else if t == "$$" {
            i += 1;
            while i < lineas.len() && lineas[i].trim() != "$$" {
                i += 1;
            }
            if i < lineas.len() {
                i += 1;
            }
            ultima = i - 1;
        } else if t.to_lowercase().starts_with("<table") {
            while i < lineas.len() && !lineas[i].to_lowercase().contains("</table>") {
                i += 1;
            }
            if i < lineas.len() {
                i += 1;
            }
            ultima = i - 1;
        } else if hay_tabla(&lineas, i) {
            while i < lineas.len() && lineas[i].trim().starts_with('|') {
                i += 1;
            }
            ultima = i - 1;
        } else if t.starts_with('>') {
            while i < lineas.len() && lineas[i].trim().starts_with('>') {
                i += 1;
            }
            ultima = i - 1;
        } else if t.is_empty() {
            i += 1;
            ultima = i - 1;
        } else if !arranca_bloque_suelto(t) {
            // Un parrafo se come las lineas que le siguen mientras no
            // arranquen otra cosa.
            i += 1;
            while i < lineas.len()
                && !lineas[i].trim().is_empty()
                && !arranca_bloque_suelto(lineas[i].trim())
                && !arranca_bloque_largo(&lineas, i)
            {
                i += 1;
            }
            ultima = i - 1;
        } else {
            i += 1;
            ultima = i - 1;
        }
        // Una sola linea en blanco se queda con este bloque.
        let mut ultima = ultima;
        if i < lineas.len() && lineas[i].trim().is_empty() && !t.is_empty() {
            i += 1;
            ultima = i - 1;
        }
        let hasta = if i < lineas.len() {
            inicios[i]
        } else {
            fin_de_linea(ultima)
        };
        salida.push(Trozo {
            desde,
            hasta: hasta.min(texto.len()),
        });
    }
    salida
}

fn abre_caja(l: &str) -> bool {
    l.starts_with(":::") && l != ":::"
}

/// Titulo, casilla, imagen sola, regla, vineta o numerada: bloques de una
/// linea que cortan un parrafo.
fn arranca_bloque_suelto(t: &str) -> bool {
    let b = t.as_bytes();
    let almohadillas = t.bytes().take_while(|c| *c == b'#').count();
    let titulo = (1..=6).contains(&almohadillas)
        && t[almohadillas..]
            .chars()
            .next()
            .is_none_or(char::is_whitespace);
    let vineta = b.len() > 2 && matches!(b[0], b'-' | b'*' | b'+') && b[1] == b' ';
    let imagen = t.starts_with("![") && t.ends_with(')') && t.contains("](");
    let cifras = t.bytes().take_while(u8::is_ascii_digit).count();
    let numerada = (1..=3).contains(&cifras)
        && matches!(b.get(cifras), Some(b'.') | Some(b')'))
        && b.get(cifras + 1).is_some_and(u8::is_ascii_whitespace);
    titulo || vineta || imagen || es_regla(t) || numerada
}

/// `---`, `***`, `___` (tres o mas del mismo).
fn es_regla(t: &str) -> bool {
    let t = t.trim();
    let Some(c) = t.chars().next() else {
        return false;
    };
    matches!(c, '-' | '*' | '_') && t.chars().count() >= 3 && t.chars().all(|x| x == c)
}

fn arranca_bloque_largo(lineas: &[&str], i: usize) -> bool {
    let t = lineas[i].trim();
    t.starts_with("```")
        || abre_caja(t)
        || t == "$$"
        || t.starts_with('>')
        || t.to_lowercase().starts_with("<table")
        || hay_tabla(lineas, i)
}

/// Una tabla de Markdown: una linea con barras y debajo la de los guiones.
fn hay_tabla(lineas: &[&str], i: usize) -> bool {
    if !lineas[i].trim().starts_with('|') {
        return false;
    }
    let Some(siguiente) = lineas.get(i + 1).map(|l| l.trim()) else {
        return false;
    };
    if !siguiente.starts_with('|') {
        return false;
    }
    let celdas: Vec<&str> = siguiente
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(str::trim)
        .collect();
    !celdas.is_empty()
        && celdas.iter().all(|c| {
            let c = c.strip_prefix(':').unwrap_or(c);
            let c = c.strip_suffix(':').unwrap_or(c);
            !c.is_empty() && c.bytes().all(|b| b == b'-')
        })
}

/// Cuantos renglones de mas ocupa un texto al doblar por el ancho.
fn dobla(texto: &str) -> usize {
    texto.chars().count() / ANCHO_EN_LETRAS
}

/// Cuantos renglones ocupa un bloque ya interpretado, mas o menos
/// (`Paginado.renglones`).
pub fn renglones(b: &markdown::Bloque) -> usize {
    match b.tipo {
        Tipo::Titulo(1) => 4 + dobla(&b.texto),
        Tipo::Titulo(2) => 3 + dobla(&b.texto),
        Tipo::Titulo(_) => 2 + dobla(&b.texto),
        Tipo::Parrafo | Tipo::Vineta | Tipo::Numerada(_) | Tipo::Cita => 1 + dobla(&b.texto),
        // El codigo no dobla: cada linea es una linea, mas las vallas.
        Tipo::Codigo => b.texto.matches('\n').count() + 3,
        Tipo::Regla => 1,
    }
}

/// Lo que ocupa un trozo de texto y si es una tabla (que baja entera).
fn medir_trozo(trozo: &str) -> (usize, bool) {
    let lineas: Vec<&str> = trozo.split('\n').collect();
    let primera = lineas
        .iter()
        .position(|l| !l.trim().is_empty())
        .unwrap_or(0);
    if hay_tabla(&lineas, primera) {
        // Una fila por fila (sin la de guiones), mas el marco.
        let filas = lineas
            .iter()
            .filter(|l| l.trim().starts_with('|'))
            .count()
            .saturating_sub(1);
        return (filas + 2, true);
    }
    let t = lineas[primera].trim();
    // Una imagen se lleva medio folio (`Medio` IMAGEN).
    if t.starts_with("![") && t.ends_with(')') {
        return (RENGLONES_POR_PAGINA / 2, false);
    }
    // La formula va compuesta y con aire; una fraccion abulta el doble.
    if t == "$$" {
        return (3 + trozo.matches('/').count(), false);
    }
    (markdown::analizar(trozo).iter().map(renglones).sum(), false)
}

/// Las paginas de una nota (`deTexto`). Nunca vacia: una nota sin nada
/// tiene una pagina en blanco. `texto` ya normalizado ([`normalizar`]).
pub fn paginas(texto: &str, por_pagina: usize) -> Vec<Pagina> {
    let trozos = trozos_de(texto);
    let Some(primero) = trozos.first() else {
        return vec![Pagina {
            desde: 0,
            hasta: texto.len(),
        }];
    };
    let tope = por_pagina.max(4);
    let mut salida = Vec::new();
    let mut desde = primero.desde;
    let mut hasta = desde;
    let mut altura = 0;
    let mut con_algo = false;
    for t in &trozos {
        let (suyo, es_tabla) = medir_trozo(&texto[t.desde..t.hasta]);
        let manda_sola = es_tabla && suyo > tope / 2;
        if (manda_sola || altura + suyo > tope) && con_algo {
            salida.push(Pagina { desde, hasta });
            desde = t.desde;
            altura = 0;
        }
        altura += suyo;
        hasta = t.hasta;
        con_algo = true;
    }
    salida.push(Pagina { desde, hasta });
    salida
}

/// Cuantas paginas tiene una nota.
pub fn cuantas(texto: &str) -> usize {
    paginas(&normalizar(texto), RENGLONES_POR_PAGINA).len()
}

#[cfg(test)]
mod pruebas {
    //! Los casos de `PaginadoTest.kt`, sobre el texto (en el PC no hay
    //! bloques interpretados con tablas: se paginan los trozos).
    use super::*;

    fn texto_largo(cuantos: usize) -> String {
        (1..=cuantos)
            .map(|n| format!("Párrafo número {n} de la nota."))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn tabla(filas: usize, columnas: usize) -> String {
        let fila = |c: &str| format!("|{}|", vec![c; columnas].join("|"));
        let mut v = vec![fila(" a "), fila("---")];
        for _ in 0..filas {
            v.push(fila(" 1 "));
        }
        v.join("\n")
    }

    #[test]
    fn una_nota_corta_cabe_en_una_pagina() {
        assert_eq!(cuantas(&texto_largo(5)), 1);
    }

    #[test]
    fn una_nota_vacia_sigue_teniendo_una_pagina() {
        assert_eq!(cuantas(""), 1);
        assert_eq!(paginas("", RENGLONES_POR_PAGINA).len(), 1);
    }

    #[test]
    fn contar_paginas_de_un_texto_de_verdad() {
        assert!(cuantas(&texto_largo(120)) >= 2);
        assert!(cuantas(&texto_largo(200)) > 3);
    }

    /// La invariante que importa: pegadas, las paginas son la nota entera,
    /// sin huecos ni solapes.
    #[test]
    fn las_paginas_cubren_el_texto_entero_sin_pisarse() {
        let texto = texto_largo(120);
        let p = paginas(&texto, RENGLONES_POR_PAGINA);
        assert!(p.len() > 1);
        assert_eq!(p.first().unwrap().desde, 0);
        assert_eq!(p.last().unwrap().hasta, texto.len());
        for par in p.windows(2) {
            assert_eq!(par[0].hasta, par[1].desde);
        }
        let pegado: String = p.iter().map(|x| &texto[x.desde..x.hasta]).collect();
        assert_eq!(pegado, texto);
    }

    #[test]
    fn los_trozos_cubren_el_texto_y_cada_pagina_empieza_en_un_trozo() {
        let texto = format!(
            "# Titulo\n\nUn parrafo\nque sigue\n- uno\n- dos\n\n```\ncodigo\n```\n> cita\n> dos\n\n{}\n\nfin",
            tabla(3, 2)
        );
        let t = trozos_de(&texto);
        assert_eq!(t.first().unwrap().desde, 0);
        assert_eq!(t.last().unwrap().hasta, texto.len());
        for par in t.windows(2) {
            assert_eq!(par[0].hasta, par[1].desde, "{t:?}");
        }
        let inicios: Vec<usize> = t.iter().map(|x| x.desde).collect();
        for p in paginas(&texto, 4) {
            assert!(inicios.contains(&p.desde));
        }
        // El parrafo de dos lineas es UN trozo, y la vineta corta el parrafo.
        let parrafo = t
            .iter()
            .find(|x| texto[x.desde..].starts_with("Un parrafo"))
            .unwrap();
        assert_eq!(
            &texto[parrafo.desde..parrafo.hasta],
            "Un parrafo\nque sigue\n"
        );
    }

    #[test]
    fn ninguna_pagina_se_pasa_del_tope_salvo_un_bloque_enorme() {
        let texto = texto_largo(200);
        for p in paginas(&texto, 20) {
            let alto: usize = trozos_de(&texto[p.desde..p.hasta])
                .iter()
                .map(|t| medir_trozo(&texto[p.desde..p.hasta][t.desde..t.hasta]).0)
                .sum();
            assert!(alto <= 20, "una pagina de {alto} renglones");
        }
    }

    /// Lo que se pidio expresamente: una tabla grande se lleva su hoja.
    #[test]
    fn una_tabla_grande_se_lleva_su_propia_pagina() {
        let texto = format!("{}\n\n{}\n\ncola", texto_largo(6), tabla(14, 3));
        let p = paginas(&texto, 20);
        let suya = p
            .iter()
            .find(|x| texto[x.desde..x.hasta].contains('|'))
            .unwrap();
        assert!(
            texto[suya.desde..].starts_with('|'),
            "la tabla no empezo pagina"
        );
    }

    #[test]
    fn un_bloque_mas_grande_que_la_pagina_se_queda_solo_en_la_suya() {
        let enorme = format!("```\n{}```", "línea\n".repeat(60));
        let texto = format!("{}\n\n{enorme}\n\n{}", texto_largo(3), texto_largo(3));
        let p = paginas(&texto, 20);
        let suya = p
            .iter()
            .find(|x| texto[x.desde..x.hasta].contains("```"))
            .unwrap();
        let dentro = &texto[suya.desde..suya.hasta];
        assert!(dentro.starts_with("```"));
        // Solo el codigo: ni el parrafo de antes ni el de despues.
        assert!(!dentro.contains("Párrafo"), "{dentro:?}");
        assert!(dentro.trim_end().ends_with("```"));
    }

    // ---- Lo que ocupa cada cosa ----

    #[test]
    fn un_titulo_ocupa_mas_que_un_parrafo() {
        assert!(medir_trozo("# Título").0 > medir_trozo("Título").0);
    }

    #[test]
    fn un_parrafo_largo_ocupa_mas_de_un_renglon() {
        assert!(medir_trozo(&"hola ".repeat(60)).0 > medir_trozo("hola").0);
    }

    #[test]
    fn una_tabla_ocupa_por_filas() {
        assert!(medir_trozo(&tabla(12, 2)).0 > medir_trozo(&tabla(2, 2)).0);
        // Caso negativo: barras sin la linea de guiones no son tabla.
        assert!(!medir_trozo("|a|b|\n|1|2|").1);
    }

    #[test]
    fn con_retornos_de_carro_cuenta_igual() {
        let unix = texto_largo(120);
        let windows = unix.replace('\n', "\r\n");
        assert_eq!(cuantas(&unix), cuantas(&windows));
    }
}
