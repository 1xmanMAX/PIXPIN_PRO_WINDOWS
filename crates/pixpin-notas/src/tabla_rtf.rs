//! **Una tabla de la nota, en RTF**, para meterla en el `RichEdit` como
//! tabla de verdad (celdas con sus rayas, Tab de celda en celda) de un solo
//! golpe: `EM_SETTEXTEX` sobre lo elegido, con deshacer. Rehacer la tabla
//! entera asi es tambien como se ponen y se quitan filas y columnas, se
//! combinan celdas y se pintan: el modelo (`pixpin_docs::md_tabla::Tabla`)
//! cambia y la tabla se vuelve a escribir.
//!
//! # Los anchos
//!
//! Como en Claude: cada columna lo que pide su texto en una linea, hasta
//! 40 em, y nunca menos que su palabra mas larga (medidos con la letra de
//! verdad, ver `anchos_con`); si todas juntas no llegan al ancho de la
//! columna de texto, se estiran hasta el. La tabla va centrada (`\trqc`):
//! una estrecha casa con el texto, y una mas ancha que la ventana conserva
//! su ancho y se desplaza ella sola con su barra (ver `tabla_ancha`).
//!
//! Los anchos no dependen de la ventana: cambiar su tamano no rehace nada.
//!
//! # Combinadas y colores
//!
//! El `RichEdit` de Windows 8 en adelante sabe de celdas combinadas
//! (`\clmgf`/`\clmrg` a lo ancho, `\clvmgf`/`\clvmrg` a lo alto: medido, la
//! tapada sigue en el texto con un `U+FFFF` y su marca de celda) y de fondo
//! de celda (`\clcbpat`). El color de la letra va en las letras (`\cf`);
//! como el formato en vivo las repinta con cada cambio, el editor lo guarda
//! tambien en la galleta de la marca de fin de cada celda (no en la trama,
//! `\clcfpat`: medido, el control pierde algunos de esos colores).

use pixpin_docs::md_tabla::{Alineacion, Tabla, Vertical, letras_visibles};

use crate::tema::Rgb;

/// Lo ancho que se deja la columna de texto, en pixeles a 96 ppp: una linea
/// mas larga se lee mal (la de la captura mide unos 660).
pub const COLUMNA_PX: i32 = 720;
const MINIMO_PX: i32 = 96;
/// Lo mas ancha que se hace una columna por su texto: 40 em de la letra del
/// cuerpo (16 px), como el navegador en las tablas de Claude. Un texto mas
/// largo se parte en renglones.
pub const MAXIMO_PX: i32 = 640;
/// Lo que mide de media una letra del cuerpo, a 96 ppp (sin ventana).
const LETRA_PX: i32 = 8;
/// El aire de cada lado dentro de la celda.
pub const AIRE_PX: i32 = 10;
/// Lo que la celda pone alrededor de su texto: el aire y las rayas.
const RELLENO_PX: i32 = 2 * AIRE_PX + 4;
/// El alto minimo de una fila, a 96 ppp.
pub const ALTO_FILA_PX: i32 = 34;
/// Veinteavos de punto por pixel a 96 ppp.
const TWIPS: i32 = 15;

/// **La paleta de las celdas**, la del lienzo (los colores rapidos de
/// Excalidraw del panel lateral): los cuatro fondos claros de relleno y las
/// cuatro tintas de trazo. Claros a proposito: sobre ellos la letra va
/// oscura en los dos temas ([`letra_sobre`]).
pub const FONDOS: [Rgb; 4] = [0xffc9c9, 0xb2f2bb, 0xa5d8ff, 0xffec99];
pub const LETRAS: [Rgb; 4] = [0xe03131, 0x2f9e44, 0x1971c2, 0xf08c00];

pub struct EstiloTabla<'a> {
    pub raya: Rgb,
    pub cabecera: Rgb,
    /// La letra de siempre (la del tema).
    pub texto: Rgb,
    pub letra: &'a str,
}

/// La letra que se lee sobre un fondo de celda: oscura sobre los claros,
/// clara sobre los oscuros (luminancia simple).
pub fn letra_sobre(fondo: Rgb) -> Rgb {
    let luz = ((fondo >> 16) & 0xff) * 299 + ((fondo >> 8) & 0xff) * 587 + (fondo & 0xff) * 114;
    if luz > 140_000 { 0x1f1e1d } else { 0xf5f4ef }
}

/// Lo que se ve de una celda: sin las marcas del Markdown (el formato en
/// vivo las esconde).
fn sin_marcas(s: &str) -> String {
    s.chars().filter(|c| !matches!(c, '*' | '`' | '~' | '_' | '$')).collect()
}

/// Lo que mide un texto con la letra media del cuerpo (sin ventana).
fn estimar(s: &str) -> i32 {
    letras_visibles(s) as i32 * LETRA_PX
}

/// El ancho de cada columna, en pixeles a 96 ppp, con la letra estimada.
pub fn anchos(t: &Tabla, columna_px: i32) -> Vec<i32> {
    anchos_con(t, columna_px, &estimar)
}

/// **El ancho de cada columna**, en pixeles a 96 ppp, midiendo el texto
/// con `medir` (pixeles a 96 ppp de un texto en una linea), como lo hace el
/// navegador con las tablas de Claude: lo que pide su texto en una linea
/// hasta 40 em, y nunca menos que su palabra mas larga (una tabla ancha no
/// se apina: se desplaza, ver `tabla_ancha`). Una combinada a lo ancho no
/// cuenta: su texto se reparte entre sus columnas.
pub fn anchos_con(t: &Tabla, columna_px: i32, medir: &dyn Fn(&str) -> i32) -> Vec<i32> {
    let n = t.columnas();
    // (natural, minimo) de cada columna.
    let medidas: Vec<(i32, i32)> = (0..n)
        .map(|c| {
            let (mut linea, mut palabra) = (0, 0);
            for (f, fila) in t.filas.iter().enumerate() {
                let x = t.formato(f, c);
                if x.tapada || x.columnas != 1 {
                    continue;
                }
                let Some(celda) = fila.get(c) else { continue };
                let visto = sin_marcas(celda);
                for renglon in visto.split('\n') {
                    linea = linea.max(medir(renglon));
                    for p in renglon.split_whitespace() {
                        palabra = palabra.max(medir(p));
                    }
                }
            }
            let minimo = MINIMO_PX.max(palabra + RELLENO_PX);
            ((linea + RELLENO_PX).clamp(minimo, MAXIMO_PX.max(minimo)), minimo)
        })
        .collect();
    let mut natural: Vec<i32> = medidas.iter().map(|m| m.0).collect();
    // Mas ancha que el tope: lo que sobra se quita de lo que cada columna
    // tiene por encima de su palabra mas larga, a proporcion.
    let total: i32 = natural.iter().sum();
    let tope = crate::tabla_ancha::TOPE_TABLA_PX;
    if total > tope {
        let holgura: i32 = medidas.iter().map(|(a, m)| a - m).sum();
        let quitar = (total - tope).min(holgura);
        if holgura > 0 {
            for (a, (nat, min)) in natural.iter_mut().zip(&medidas) {
                let parte = (nat - min) as i64 * quitar as i64;
                *a = nat - ((parte + holgura as i64 - 1) / holgura as i64) as i32;
                *a = (*a).max(*min);
            }
        }
    }
    let total: i32 = natural.iter().sum();
    if total >= columna_px || total == 0 {
        return natural;
    }
    // Se reparte lo que falta a proporcion, y el redondeo a la ultima.
    let falta = columna_px - total;
    let mut sal: Vec<i32> = natural.iter().map(|a| a + falta * a / total).collect();
    let suma: i32 = sal.iter().sum();
    if let Some(u) = sal.last_mut() {
        *u += columna_px - suma;
    }
    sal
}

/// Un texto dentro de RTF: las tres letras especiales escapadas y todo lo
/// que no es ASCII como `\uN?` (unidades UTF-16 con signo).
pub fn escapar(s: &str) -> String {
    let mut sal = String::with_capacity(s.len());
    for u in s.encode_utf16() {
        match u {
            0x5C => sal.push_str("\\\\"),
            0x7B => sal.push_str("\\{"),
            0x7D => sal.push_str("\\}"),
            // Un salto dentro de una celda la partiria en dos parrafos; en
            // Markdown no cabe, asi que espacio (como `md_tabla::a_gfm`).
            0x0A | 0x0D | 0x07 => sal.push(' '),
            0x20..=0x7E => sal.push(u as u8 as char),
            _ => sal.push_str(&format!("\\u{}?", u as i16)),
        }
    }
    sal
}

fn color(c: Rgb) -> String {
    format!("\\red{}\\green{}\\blue{};", (c >> 16) & 0xff, (c >> 8) & 0xff, c & 0xff)
}

/// La tabla de colores: los tres del tema (1 raya, 2 cabecera, 3 letra) y
/// los de las celdas detras, cada uno una vez.
struct Colores(Vec<Rgb>);

impl Colores {
    fn indice(&mut self, c: Rgb) -> usize {
        match self.0.iter().position(|x| *x == c) {
            Some(i) => i + 1,
            None => {
                self.0.push(c);
                self.0.len()
            }
        }
    }
}

/// La tabla en RTF, lista para `EM_SETTEXTEX`.
pub fn rtf(t: &Tabla, estilo: &EstiloTabla) -> String {
    rtf_con(t, estilo, &estimar)
}

/// La tabla en RTF, con el texto medido por `medir` (ver [`anchos_con`]).
pub fn rtf_con(t: &Tabla, estilo: &EstiloTabla, medir: &dyn Fn(&str) -> i32) -> String {
    let n = t.columnas();
    let anchos = anchos_con(t, COLUMNA_PX, medir);
    let mut colores = Colores(vec![estilo.raya, estilo.cabecera, estilo.texto]);
    let raya = "\\brdrs\\brdrw15\\brdrcf1";
    // Cada fila con un alto minimo y el texto centrado en alto: el aire de
    // las celdas de la captura (el RichEdit no sabe de relleno por arriba).
    let alto = ALTO_FILA_PX * TWIPS;
    let mut cuerpo = String::new();
    for (f, fila) in t.filas.iter().enumerate() {
        cuerpo.push_str(&format!("\\trowd\\trqc\\trgaph{}\\trrh{alto}", AIRE_PX * TWIPS));
        let mut x = 0;
        for (c, a) in anchos.iter().enumerate().take(n) {
            let (af, ac) = t.ancla(f, c);
            let manda = t.formato(af, ac);
            if manda.columnas > 1 {
                cuerpo.push_str(if c == ac { "\\clmgf" } else { "\\clmrg" });
            }
            if manda.filas > 1 {
                cuerpo.push_str(if f == af { "\\clvmgf" } else { "\\clvmrg" });
            }
            cuerpo.push_str(if manda.vertical == Vertical::Abajo { "\\clvertalb" } else { "\\clvertalc" });
            cuerpo.push_str(&format!("\\clbrdrt{raya}\\clbrdrl{raya}\\clbrdrb{raya}\\clbrdrr{raya}"));
            match manda.fondo {
                Some(fondo) => cuerpo.push_str(&format!("\\clcbpat{}", colores.indice(fondo))),
                None if t.es_cabecera(af, ac) => cuerpo.push_str("\\clcbpat2"),
                None => {}
            }
            x += a * TWIPS;
            cuerpo.push_str(&format!("\\cellx{x}"));
        }
        for c in 0..n {
            let (af, ac) = t.ancla(f, c);
            let manda = t.formato(af, ac);
            let q = match t.alineacion_de(af, ac) {
                Alineacion::Izquierda => "\\ql",
                Alineacion::Centro => "\\qc",
                Alineacion::Derecha => "\\qr",
            };
            let letra = manda.letra.or(manda.fondo.map(letra_sobre)).unwrap_or(estilo.texto);
            let cf = colores.indice(letra);
            let texto = if (af, ac) == (f, c) {
                fila.get(c).map(String::as_str).unwrap_or("")
            } else {
                ""
            };
            // Un salto (el de una combinada, como las junta el movil) es un
            // salto de renglon dentro del mismo parrafo (`\line`): partir la
            // celda en dos parrafos le romperia el renglon de la fila.
            let texto: Vec<String> = texto.split('\n').map(escapar).collect();
            cuerpo.push_str(&format!("\\pard\\intbl{q}\\f0\\cf{cf} {}\\cell ", texto.join("\\line ")));
        }
        cuerpo.push_str("\\row ");
    }
    let mut s = String::new();
    s.push_str("{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0\\fnil ");
    s.push_str(&escapar(estilo.letra));
    s.push_str(";}}{\\colortbl;");
    for c in &colores.0 {
        s.push_str(&color(*c));
    }
    s.push('}');
    s.push_str(&cuerpo);
    s.push('}');
    s
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_docs::md_tabla::leer_gfm;

    fn estilo() -> EstiloTabla<'static> {
        EstiloTabla {
            raya: 0x2e2e2d,
            cabecera: 0x151515,
            texto: 0xe6e4df,
            letra: "Work Sans",
        }
    }

    #[test]
    fn una_tabla_estrecha_se_estira_hasta_la_columna_de_texto() {
        let t = leer_gfm("| a | b |\n|---|---|\n| 1 | 2 |").unwrap();
        let a = anchos(&t, COLUMNA_PX);
        assert_eq!(a.iter().sum::<i32>(), COLUMNA_PX);
        assert_eq!(a[0], a[1]);
    }

    #[test]
    fn una_tabla_ancha_se_sale_y_cada_columna_pide_lo_suyo() {
        let largo = "Diagrama de Pareto (tipo de proyecto y problemas en pavimentacion)";
        let t = leer_gfm(&format!(
            "| Objetivo | Tecnica | Fuente | Entregable | Notas |\n|---|---|---|---|---|\n| OE1 | {largo} | {largo} | corto | x |"
        ))
        .unwrap();
        let a = anchos(&t, COLUMNA_PX);
        assert!(a.iter().sum::<i32>() > COLUMNA_PX, "no se encoge a la columna");
        assert_eq!(a[1], largo.len() as i32 * LETRA_PX + RELLENO_PX, "su texto en una linea, sin apinar");
        assert!(a[0] < a[1]);
        assert!(a[4] >= MINIMO_PX);
    }

    /// Una medida falsa: cada letra 8 px, la `W` 20.
    fn medir(s: &str) -> i32 {
        s.chars().map(|c| if c == 'W' { 20 } else { 8 }).sum()
    }

    #[test]
    fn cada_columna_mide_su_texto_en_una_linea_hasta_cuarenta_em() {
        let corto = "Juicio de expertos";
        let largo = "palabra ".repeat(120);
        let t = leer_gfm(&format!("| a | b | c |\n|---|---|---|\n| {corto} | {largo} | WWWW |")).unwrap();
        let a = anchos_con(&t, COLUMNA_PX, &medir);
        assert_eq!(a[0], medir(corto) + RELLENO_PX);
        assert_eq!(a[1], MAXIMO_PX, "un parrafo entero se parte en renglones de 40 em");
        assert_eq!(a[2], MINIMO_PX.max(80 + RELLENO_PX), "se mide con la letra de verdad");
        assert_eq!(MAXIMO_PX, 40 * 16);
    }

    #[test]
    fn una_palabra_larga_nunca_se_parte_aunque_pase_del_maximo() {
        let palabra = "x".repeat(100);
        let t = leer_gfm(&format!("| a | b |\n|---|---|\n| {palabra} | y |")).unwrap();
        let a = anchos_con(&t, COLUMNA_PX, &medir);
        assert_eq!(a[0], 800 + RELLENO_PX);
        // Caso negativo: las marcas del Markdown no cuentan (no se ven).
        let t = leer_gfm("| a |\n|---|\n| **negrita** |").unwrap();
        let con_marcas = anchos_con(&t, 10, &medir);
        let t = leer_gfm("| a |\n|---|\n| negrita |").unwrap();
        assert_eq!(con_marcas, anchos_con(&t, 10, &medir));
    }

    #[test]
    fn una_tabla_enorme_se_queda_en_el_tope_sin_partir_palabras() {
        let celda = "uno dos tres cuatro cinco seis siete ocho nueve diez once doce trece catorce";
        let fila = vec![celda; 8].join(" | ");
        let t = leer_gfm(&format!("|{}\n|{}\n| {fila} |", " h |".repeat(8), "---|".repeat(8))).unwrap();
        let a = anchos_con(&t, COLUMNA_PX, &medir);
        let total: i32 = a.iter().sum();
        assert!(total <= crate::tabla_ancha::TOPE_TABLA_PX, "{total}");
        assert!(total > crate::tabla_ancha::TOPE_TABLA_PX - 8, "se usa el tope entero: {total}");
        assert!(a.iter().all(|x| *x >= medir("catorce") + RELLENO_PX), "{a:?}");
    }

    #[test]
    fn las_letras_raras_y_las_llaves_se_escapan() {
        assert_eq!(escapar("café {x}\\"), "caf\\u233? \\{x\\}\\\\");
        // Fuera del plano basico: dos unidades, cada una con su signo.
        assert_eq!(escapar("😀"), "\\u-10179?\\u-8704?");
        assert_eq!(escapar("a\nb"), "a b");
    }

    #[test]
    fn el_rtf_lleva_una_fila_por_fila_y_una_celda_por_celda() {
        let t = leer_gfm("| a | b |\n|:-:|--:|\n| 1 | 2 |\n| 3 | 4 |").unwrap();
        let r = rtf(&t, &estilo());
        assert_eq!(r.matches("\\row").count(), 3);
        assert_eq!(r.matches("\\cell ").count(), 6);
        assert_eq!(r.matches("\\clcbpat2").count(), 2, "solo la cabecera va sombreada");
        assert!(r.contains("\\qc\\f0\\cf3 a\\cell"));
        assert!(r.contains("\\qr\\f0\\cf3 2\\cell"));
        assert!(!r.contains("\\clmgf"), "sin combinadas");
        assert!(r.starts_with("{\\rtf1") && r.ends_with('}'));
        assert_eq!(r.matches('{').count(), r.matches('}').count());
    }

    #[test]
    fn una_combinada_lleva_sus_marcas_de_rtf_y_su_tapada_va_vacia() {
        let mut t = leer_gfm("| a | b | c |\n|---|---|---|\n| 1 | 2 | 3 |\n| 4 | 5 | 6 |").unwrap();
        t.combinar((1, 0), (2, 1));
        let r = rtf(&t, &estilo());
        // Fila 1: la que manda y su vecina; fila 2: las de debajo.
        assert!(r.contains("\\clmgf\\clvmgf"));
        assert!(r.contains("\\clmrg\\clvmgf"));
        assert!(r.contains("\\clmgf\\clvmrg"));
        assert!(r.contains("\\clmrg\\clvmrg"));
        assert_eq!(r.matches("\\cell ").count(), 9, "las tapadas siguen siendo celdas");
        assert!(r.contains("\\cf3 1\\line 2\\line 4\\line 5\\cell"), "el texto junto, en renglones");
    }

    #[test]
    fn los_colores_de_celda_van_al_fondo_y_a_la_letra() {
        let mut t = leer_gfm("| a | b |\n|---|---|\n| 1 | 2 |").unwrap();
        t.poner_fondo((1, 0), (1, 0), Some(0xffc9c9));
        t.poner_letra((1, 1), (1, 1), Some(0xe03131));
        t.poner_fondo((0, 1), (0, 1), Some(0x1971c2));
        let r = rtf(&t, &estilo());
        // Los del tema (1-3) y luego, en el orden en que salen: el azul de
        // la cabecera (4), su letra clara (5), el rosa (6), la letra oscura
        // sobre el rosa (7) y el rojo (8).
        assert!(r.contains("\\clcbpat4\\cellx"));
        assert!(r.contains("\\cf5 b\\cell"), "sobre un fondo oscuro, letra clara");
        assert!(r.contains("\\red255\\green201\\blue201;"));
        assert!(r.contains("\\clcbpat6"));
        assert!(r.contains("\\cf8 2\\cell"), "la suya");
        assert!(r.contains("\\cf7 1\\cell"), "sobre un fondo claro, letra oscura");
        assert_eq!(r.matches("\\clcbpat2").count(), 1, "la cabecera con color no lleva el gris");
    }

    #[test]
    fn la_letra_se_lee_sobre_cada_fondo_de_la_paleta() {
        for f in FONDOS {
            assert_eq!(letra_sobre(f), 0x1f1e1d, "{f:06x}");
        }
        assert_eq!(letra_sobre(0x000000), 0xf5f4ef);
        assert_eq!(letra_sobre(0x4a86e8), 0xf5f4ef);
    }
}
