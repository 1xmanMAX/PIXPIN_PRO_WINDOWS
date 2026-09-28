//! Las tablas de calculo del proyecto.
//!
//! El movil tiene su hoja desde el 10-sep-2026 (`motor/TablaDeCalculo.kt`,
//! el mismo JSON: `nombre`, `celdas`, `anchos`, `estilos`, `tocado`,
//! `protegida`); esta nota decia lo contrario porque se escribio mirando un
//! clon viejo. Aqui viaja por el hueco de las mini-apps: un mensaje de clase
//! `MINIAPP` lleva el documento entero en `texto`
//! y la palabra de su tipo en `miniapp` —aqui, `"tabla"`—, y ante una
//! palabra que no conoce el movil ensena ese texto tal cual en vez de
//! romperse. No hay fichero aparte: la tabla ES el texto del mensaje.
//!
//! La forma es deliberadamente sencilla: **un mapa de celda a texto**, no
//! una matriz.
//!
//! Que sea un mapa y no una matriz no es capricho: una tabla con una celda en
//! la `A1` y otra en la `Z900` ocupa dos entradas y no ochocientas mil. Y
//! ademas es lo que hace que la fusion funcione celda a celda —un mapa se
//! junta clave por clave— sin escribir nada especial para ello.
//!
//! Aqui no se calculan formulas todavia: se guarda el texto tal cual
//! («=SUMA(B1:B6)») y se ensena. Calcular es un paso aparte, y fingir un
//! resultado seria peor que ensenar la formula.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// La palabra que va en `Mensaje.miniapp` para decir que el texto es una
/// tabla. Se escribe una vez aqui porque es un dato guardado: cambiarla
/// dejaria de reconocer las tablas ya escritas.
pub const MINIAPP: &str = "tabla";

/// Una tabla, tal como viaja.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tabla {
    pub nombre: String,
    /// De referencia (`B7`) a contenido. El contenido puede ser un numero, un
    /// texto o una formula que empieza por `=`.
    pub celdas: BTreeMap<String, String>,
    /// Ancho de una columna (`B`) en pixeles, si se cambio a mano.
    pub anchos: BTreeMap<String, u32>,
    /// El estilo de una celda: el `EstiloDeCelda` del movil.
    pub estilos: BTreeMap<String, EstiloDeCelda>,
    /// Milisegundos desde 1970 de la ultima vez que se toco.
    pub tocado: i64,
    /// Lo que no se entiende, tal cual: guardar no puede perder lo que anada
    /// una version de Android.
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

/// **El poco formato de una celda**, el `EstiloDeCelda` del movil
/// (`motor/TablaDeCalculo.kt`): negrita, alineacion y fondo, con nombres de
/// una letra porque se repiten en cada celda con estilo. Lo que no vale lo de
/// por defecto no se escribe (`encodeDefaults = false` alli).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EstiloDeCelda {
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub n: bool,
    /// `i`, `c` o `d`; sin nada, la que toque por el valor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a: Option<String>,
    /// El fondo, `#rrggbb`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub f: Option<String>,
    /// Editable aunque la tabla este protegida (la pagina web lo mira).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub e: bool,
    #[serde(flatten)]
    pub resto: serde_json::Map<String, serde_json::Value>,
}

impl EstiloDeCelda {
    /// Si no cambia nada: no vale la pena guardarlo.
    pub fn vacio(&self) -> bool {
        !self.n && self.a.is_none() && self.f.is_none() && !self.e && self.resto.is_empty()
    }
}

/// Una celda por su sitio: columna y fila, las dos desde cero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ref {
    pub columna: u32,
    pub fila: u32,
}

/// De `B7` a (1, 6). `None` si no tiene esa forma.
///
/// Las columnas van como en cualquier hoja de calculo: tras la `Z` viene la
/// `AA`, que es la **27**, no la 26; por eso se suma uno a cada letra antes
/// de multiplicar, y se resta uno al final.
pub fn ref_de(texto: &str) -> Option<Ref> {
    let texto = texto.trim();
    if texto.is_empty() {
        return None;
    }
    let letras: String = texto
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    let numeros = &texto[letras.len()..];
    if letras.is_empty() || numeros.is_empty() {
        return None;
    }
    // Una referencia con mas de siete letras no cabe en un u32, y no existe
    // hoja con tantas columnas: es basura, no una celda lejana.
    if letras.len() > 7 || !numeros.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut columna: u32 = 0;
    for c in letras.chars() {
        let valor = c.to_ascii_uppercase() as u32 - 'A' as u32 + 1;
        columna = columna.checked_mul(26)?.checked_add(valor)?;
    }
    let fila: u32 = numeros.parse().ok()?;
    // La fila 0 no existe: las hojas empiezan en la 1.
    Some(Ref {
        columna: columna.checked_sub(1)?,
        fila: fila.checked_sub(1)?,
    })
}

/// De (1, 6) a `B7`.
pub fn ref_a(r: Ref) -> String {
    let mut letras = Vec::new();
    let mut n = r.columna as u64 + 1;
    while n > 0 {
        let resto = ((n - 1) % 26) as u8;
        letras.push((b'A' + resto) as char);
        n = (n - 1) / 26;
    }
    letras.reverse();
    let letras: String = letras.into_iter().collect();
    format!("{letras}{}", r.fila + 1)
}

impl Tabla {
    /// Lo que hay en una celda, o cadena vacia.
    pub fn celda(&self, r: Ref) -> &str {
        self.celdas.get(&ref_a(r)).map_or("", |s| s.as_str())
    }

    /// Escribe una celda. Escribir vacio la QUITA en vez de guardar una
    /// cadena vacia: si no, borrar una celda dejaria basura que viajaria por
    /// la red y contaria como diferencia sin serlo.
    pub fn poner(&mut self, r: Ref, contenido: &str) {
        let clave = ref_a(r);
        if contenido.is_empty() {
            self.celdas.remove(&clave);
        } else {
            self.celdas.insert(clave, contenido.to_string());
        }
    }

    /// Hasta donde llega la tabla: cuantas columnas y cuantas filas hay que
    /// ensenar para que se vea todo lo escrito.
    ///
    /// Se mira lo que hay, no un tamano guardado: una tabla es tan grande
    /// como su celda mas lejana.
    pub fn tamano(&self) -> (u32, u32) {
        let mut columnas = 0;
        let mut filas = 0;
        for clave in self.celdas.keys() {
            if let Some(r) = ref_de(clave) {
                columnas = columnas.max(r.columna + 1);
                filas = filas.max(r.fila + 1);
            }
        }
        (columnas, filas)
    }

    /// Si el contenido de una celda es una formula.
    pub fn es_formula(contenido: &str) -> bool {
        contenido.starts_with('=')
    }

    pub fn leer(texto: &str) -> Result<Tabla, serde_json::Error> {
        serde_json::from_str(texto)
    }

    pub fn escribir(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_referencias_van_y_vuelven() {
        for (texto, columna, fila) in [
            ("A1", 0, 0),
            ("B7", 1, 6),
            ("Z1", 25, 0),
            // Tras la Z viene la AA, que es la 27; el fallo clasico es que
            // salga la 26 por no sumar uno a cada letra.
            ("AA1", 26, 0),
            ("AB1", 27, 0),
            ("BA1", 52, 0),
            ("ZZ100", 701, 99),
            ("AAA1", 702, 0),
        ] {
            let r = ref_de(texto).unwrap_or_else(|| panic!("{texto}"));
            assert_eq!((r.columna, r.fila), (columna, fila), "{texto}");
            assert_eq!(ref_a(r), texto, "de vuelta");
        }
    }

    #[test]
    fn lo_que_no_es_una_celda_no_lo_parece() {
        // Casos negativos: sin numero, sin letra, del reves, la fila cero
        // (las hojas empiezan en la 1) y basura.
        for malo in ["", "A", "1", "1A", "A0", "-", "A-1", "A1.5", "ABCDEFGH1"] {
            assert!(ref_de(malo).is_none(), "{malo} no es una celda");
        }
        // Las minusculas si valen: se teclean asi a menudo.
        assert_eq!(ref_de("b7"), ref_de("B7"));
    }

    #[test]
    fn una_celda_vacia_se_quita_en_vez_de_guardarse_vacia() {
        let mut t = Tabla::default();
        let b7 = ref_de("B7").unwrap();
        t.poner(b7, "12");
        assert_eq!(t.celda(b7), "12");
        assert_eq!(t.celdas.len(), 1);
        // Borrarla la quita del mapa: una cadena vacia viajaria por la red y
        // contaria como diferencia sin serlo.
        t.poner(b7, "");
        assert_eq!(t.celda(b7), "");
        assert!(t.celdas.is_empty());
    }

    #[test]
    fn la_tabla_es_tan_grande_como_su_celda_mas_lejana() {
        let mut t = Tabla::default();
        assert_eq!(t.tamano(), (0, 0), "una tabla vacia no ocupa nada");
        t.poner(ref_de("A1").unwrap(), "1");
        assert_eq!(t.tamano(), (1, 1));
        t.poner(ref_de("C10").unwrap(), "2");
        assert_eq!(t.tamano(), (3, 10));
        // Una celda suelta lejos no cuesta nada: el mapa tiene dos entradas,
        // no treinta.
        assert_eq!(t.celdas.len(), 2);
    }

    #[test]
    fn una_tabla_de_android_se_lee_y_se_devuelve_sin_perder_nada() {
        let original = r##"{
            "nombre": "Gastos",
            "celdas": {"A1": "Concepto", "B1": "Importe", "B7": "=SUMA(B2:B6)"},
            "anchos": {"A": 180},
            "estilos": {"B1": {"n": true, "f": "#fff2cc"}, "C1": {"a": "d", "x": 7}},
            "tocado": 1789500000000,
            "congeladas": 1,
            "futuro": {"x": true}
        }"##;
        let t = Tabla::leer(original).unwrap();
        assert_eq!(t.nombre, "Gastos");
        assert_eq!(t.celda(ref_de("A1").unwrap()), "Concepto");
        assert_eq!(t.anchos.get("A"), Some(&180));
        // El estilo es el `EstiloDeCelda` del movil (`n`, `a`, `f`, `e`), no
        // una palabra: con una cadena aqui, una tabla con negritas del movil
        // no se podia ni abrir.
        let b1 = &t.estilos["B1"];
        assert!(b1.n);
        assert_eq!(b1.f.as_deref(), Some("#fff2cc"));
        assert_eq!(t.estilos["C1"].a.as_deref(), Some("d"));
        assert!(Tabla::es_formula(t.celda(ref_de("B7").unwrap())));
        assert!(!Tabla::es_formula(t.celda(ref_de("A1").unwrap())));

        // Y lo que aqui no se entiende vuelve tal cual: guardar desde Windows
        // no puede tirar campos de una version mas nueva del movil.
        let vuelta: serde_json::Value = serde_json::from_str(&t.escribir().unwrap()).unwrap();
        assert_eq!(vuelta["congeladas"], 1);
        assert_eq!(vuelta["futuro"]["x"], true);
        assert_eq!(vuelta["celdas"]["B7"], "=SUMA(B2:B6)");
        assert_eq!(vuelta["estilos"]["C1"]["x"], 7, "lo que no se entiende de un estilo tambien vuelve");
        // Y sin los valores por defecto, como `encodeDefaults = false` del
        // movil: mil celdas con estilo no repiten mil veces `"n":false`.
        assert!(vuelta["estilos"]["C1"].get("n").is_none());
    }
}
