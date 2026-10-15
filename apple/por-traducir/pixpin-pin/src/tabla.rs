//! Tablas copiadas de una hoja de calculo: puerto de `clipboard/TableData.kt`.
//!
//! Excel, Sheets y Numbers dejan en el portapapeles el mismo texto plano:
//! columnas separadas por TABULADOR y filas por salto de linea. Con eso se
//! tiene la tabla legible y alineada, que es lo que sirve dentro de un pin.
//! Tambien se prueban las barras de Markdown y las columnas alineadas con
//! espacios, que es como llega una tabla copiada de una web o de un PDF.

/// Minimo de filas para no confundir dos lineas cualesquiera con una tabla.
const MIN_FILAS: usize = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Separador {
    Tabulador,
    Barra,
    /// Dos espacios o mas.
    Espacios,
}

const SEPARADORES: [Separador; 3] = [Separador::Tabulador, Separador::Barra, Separador::Espacios];

fn partir(linea: &str, sep: Separador) -> Vec<String> {
    match sep {
        // **Con tabuladores no se recorta la linea.** `trim` se lleva los
        // tabuladores de los extremos, que es justo donde vive una celda
        // vacia: la fila llegaria con una columna menos y todos sus datos se
        // correrian un sitio. Solo se quitan espacios y el retorno de carro.
        Separador::Tabulador => linea
            .trim_matches(|c| c == ' ' || c == '\r')
            .split('\t')
            .map(|c| c.trim().to_string())
            .collect(),
        // Las de Markdown vienen con barra al principio y al final: sin
        // quitarlas saldria una celda vacia a cada lado que no existe.
        Separador::Barra => linea
            .trim()
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim().to_string())
            .collect(),
        Separador::Espacios => {
            let mut celdas = Vec::new();
            let mut resto = linea.trim().trim_matches('|');
            while let Some(i) = resto.find("  ") {
                celdas.push(resto[..i].trim().to_string());
                resto = resto[i..].trim_start_matches(' ');
            }
            celdas.push(resto.trim().to_string());
            celdas
        }
    }
}

/// La mejor rejilla que sale del texto, o `None`.
///
/// Se exige que la MAYORIA de las filas coincidan en numero de columnas: un
/// texto corriente puede llevar algun tabulador o una barra suelta, pero no
/// una rejilla regular. Gana el separador que da mas columnas coincidentes.
fn rejilla(texto: &str) -> Option<Vec<Vec<String>>> {
    let texto = texto.replace("\r\n", "\n");
    let lineas: Vec<&str> = texto.split('\n').filter(|l| !l.trim().is_empty()).collect();
    if lineas.len() < MIN_FILAS {
        return None;
    }
    let mut mejor: Option<Vec<Vec<String>>> = None;
    let mut mejor_nota = 0;
    for sep in SEPARADORES {
        let filas: Vec<Vec<String>> = lineas.iter().map(|l| partir(l, sep)).collect();
        // Fuera la linea de guiones de Markdown entre cabecera y cuerpo.
        let utiles: Vec<Vec<String>> = filas
            .into_iter()
            .filter(|f| {
                !f.iter()
                    .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
            })
            .collect();
        if utiles.len() < MIN_FILAS {
            continue;
        }
        // Cuantas filas hay de cada ancho, en el orden en que aparecen: a
        // igualdad gana el primero, como `maxByOrNull` sobre el
        // `groupingBy` del movil, que tambien guarda ese orden.
        let mut cuentas: Vec<(usize, usize)> = Vec::new();
        for f in &utiles {
            match cuentas.iter_mut().find(|(n, _)| *n == f.len()) {
                Some((_, v)) => *v += 1,
                None => cuentas.push((f.len(), 1)),
            }
        }
        let mut comun: Option<(usize, usize)> = None;
        for &(n, v) in &cuentas {
            if comun.is_none_or(|(_, mejor)| v > mejor) {
                comun = Some((n, v));
            }
        }
        let Some((columnas, veces)) = comun else {
            continue;
        };
        if columnas < 2 || veces < MIN_FILAS || veces * 2 < utiles.len() {
            continue;
        }
        let nota = veces * columnas;
        if nota > mejor_nota {
            mejor_nota = nota;
            mejor = Some(utiles);
        }
    }
    mejor
}

pub fn parece_tabla(texto: &str) -> bool {
    rejilla(texto).is_some()
}

/// Filas ya separadas, todas del mismo ancho: las cortas se rellenan con
/// celdas vacias para que la rejilla no se descuadre.
pub fn leer(texto: &str) -> Vec<Vec<String>> {
    let Some(filas) = rejilla(texto) else {
        return Vec::new();
    };
    let ancho = filas.iter().map(Vec::len).max().unwrap_or(0);
    filas
        .into_iter()
        .map(|mut f| {
            f.resize(ancho, String::new());
            f
        })
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lo_pegado_de_excel_es_una_tabla_con_sus_columnas() {
        let t = leer("Nombre\tEdad\nAna\t30\nLuis\t41\n");
        assert_eq!(t.len(), 3);
        assert_eq!(t[0], vec!["Nombre", "Edad"]);
        assert_eq!(t[2], vec!["Luis", "41"]);
    }

    #[test]
    fn una_celda_vacia_al_principio_no_corre_la_fila() {
        let t = leer("a\tb\tc\n\t2\t3\n1\t2\t3");
        assert_eq!(t[1], vec!["", "2", "3"]);
    }

    #[test]
    fn la_tabla_de_markdown_pierde_sus_bordes_y_la_raya_de_guiones() {
        let t = leer("| a | b |\n|---|:-:|\n| 1 | 2 |");
        assert_eq!(t, vec![vec!["a", "b"], vec!["1", "2"]]);
    }

    #[test]
    fn columnas_alineadas_con_espacios_tambien_son_tabla() {
        let t = leer("Mes    Total\nEnero  120\nMarzo  99");
        assert_eq!(t[1], vec!["Enero", "120"]);
    }

    #[test]
    fn un_texto_corriente_no_es_tabla() {
        assert!(!parece_tabla("hola\nque tal"));
        assert!(!parece_tabla("una sola\tlinea"));
        // Una barra suelta en una de dos lineas no hace rejilla.
        assert!(!parece_tabla("si | no\nnada mas aqui"));
        assert!(!parece_tabla(""));
    }

    #[test]
    fn las_filas_cortas_se_rellenan_hasta_el_ancho() {
        let t = leer("a\tb\tc\n1\t2\t3\n4\t5\t6\nx\ty");
        assert!(t.iter().all(|f| f.len() == 3), "{t:?}");
        assert_eq!(t[3], vec!["x", "y", ""]);
    }
}
