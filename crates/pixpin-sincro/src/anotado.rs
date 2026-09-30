//! **Lo anotado sobre un adjunto del chat, nombrado por su mensaje**
//! (`sincro/AnotacionesDelAdjunto.kt` de PixPin Android v0.96, 28-sep-2026).
//!
//! La tinta de un PDF suelto, la de un Word o un libro, sus marcadores y los
//! espacios para anotar se guardaban con la ruta del documento, que es de un
//! aparato: en el otro el mismo PDF vive en otra. Ahora van en `pins/draw/`
//! con el **codigo unico del mensaje** (`Codigos.unico`, [`crate::kotlin::unico`]),
//! que es el mismo en los dos aparatos, y `alcance` los manda con su mensaje:
//!
//! - `anot-<uid>-p<n>.excalidraw.gz`: la tinta de la pagina `n` (desde 0) de un PDF.
//! - `anot-<uid>.excalidraw.gz`: la tinta de un Word o un libro.
//! - `anot-<uid>-texto…`: lo de un PDF leido como texto (otro dibujo, otra columna).
//! - `anot-<uid>.marcas`: los marcadores, el mismo texto de siempre.
//! - `anot-<uid>.espacios`: los espacios del PDF, `0`..`3`.
//! - `anot-<uid>.maqueta`: la columna fijada de un Word o un libro
//!   (`columna,izq,der,tamano,grosor,tipo`).
//! - `anot-<uid>.voz`: el marcador verde de la voz (`parrafo:fraccion`).
//! - `anot-<uid>.sitio`: por donde se iba leyendo (fraccion del alto).
//! - `anot-<uid>-p<n>.hoja`, `anot-<uid>.hoja`: **el marco de la tinta**
//!   (29-sep), donde esta la hoja en las unidades en que se escribieron los
//!   puntos de la tinta hermana. Ver [`MarcoDeLaHoja`].
//!
//! El PDF de un proyecto lleva su tinta en las hojas (ya viajan); sus
//! marcadores y espacios van aqui con el codigo **del proyecto**. Y los
//! marcadores de un lienzo van junto a su dibujo, `<dibujo>.marcas`.
//!
//! Aqui solo va lo que es igual en los dos aparatos (los nombres y el texto
//! de la maqueta); donde cae cada fichero en el disco lo dice cada `Disco`.

use std::collections::BTreeMap;

/// Donde viven, en rutas portatiles.
pub const CARPETA: &str = "pins/draw";
pub const PREFIJO: &str = "anot-";

/// Las terminaciones de lo anotado (`TERMINACIONES`): ni temporales ni otra cosa.
pub const TERMINACIONES: [&str; 7] = [
    ".excalidraw.gz",
    ".marcas",
    ".espacios",
    ".maqueta",
    ".voz",
    ".sitio",
    ".hoja",
];

/// La terminacion del marco de la tinta ([`MarcoDeLaHoja`]).
pub const HOJA: &str = ".hoja";

/// La base de la tinta de la pagina `pagina` (desde 0) de un PDF.
pub fn de_pagina(uid: &str, pagina: u32) -> String {
    format!("{PREFIJO}{uid}-p{pagina}")
}

/// La base de lo de un PDF (marcadores y espacios).
pub fn del_pdf(uid: &str) -> String {
    format!("{PREFIJO}{uid}")
}

/// La base de un Word, un libro, o un PDF leido como texto (`delDocumento`).
pub fn del_documento(uid: &str, nombre_del_original: &str) -> String {
    if nombre_del_original.to_ascii_lowercase().ends_with(".pdf") {
        format!("{PREFIJO}{uid}-texto")
    } else {
        format!("{PREFIJO}{uid}")
    }
}

/// La ruta portatil de un fichero de lo anotado: `pins/draw/<base><terminacion>`.
pub fn rel(base: &str, terminacion: &str) -> String {
    format!("{CARPETA}/{base}{terminacion}")
}

/// `marcasJuntoA`: `pins/draw/X.excalidraw.gz` → `pins/draw/X.marcas`;
/// `None` si no es un dibujo de esa carpeta.
pub fn marcas_junto_a(rel: &str) -> Option<String> {
    let resto = rel.strip_prefix(CARPETA)?.strip_prefix('/')?;
    let base = resto.strip_suffix(".excalidraw.gz")?;
    (!resto.contains('/')).then(|| format!("{CARPETA}/{base}.marcas"))
}

/// El codigo de mensaje de un nombre de fichero (`anot-<uid>…`), si lo es:
/// empieza por el prefijo, acaba en una terminacion conocida y el codigo
/// tiene su largo entero (`porUid`).
pub fn uid_del_nombre(nombre: &str) -> Option<&str> {
    let resto = nombre.strip_prefix(PREFIJO)?;
    if !TERMINACIONES.iter().any(|t| nombre.ends_with(t)) {
        return None;
    }
    let fin = resto.find(['-', '.']).unwrap_or(resto.len());
    let uid = &resto[..fin];
    (uid.chars().count() == crate::codigo::LARGO).then_some(uid)
}

/// `porUid`: lo anotado de una lista de rutas portatiles, por codigo de
/// mensaje. Lo que no es de un mensaje se deja fuera.
pub fn por_uid<'a>(rels: impl IntoIterator<Item = &'a str>) -> BTreeMap<String, Vec<String>> {
    let mut salida: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in rels {
        let Some(nombre) = r.strip_prefix(CARPETA).and_then(|x| x.strip_prefix('/')) else {
            continue;
        };
        if nombre.contains('/') {
            continue;
        }
        if let Some(uid) = uid_del_nombre(nombre) {
            salida.entry(uid.to_string()).or_default().push(r.to_string());
        }
    }
    salida
}

/// **La columna fijada de un Word o un libro** y la letra con la que se
/// fijo (`AnotacionesDelAdjunto.Maqueta`). Los numeros son los del movil:
/// `grosor` es el indice de sus cuatro pesos (300, 400, 600, 800) y `tipo`
/// el de sus cuatro letras (serif, sans, mono, cursiva).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Maqueta {
    pub columna: i32,
    pub izq: i32,
    pub der: i32,
    pub tamano: i32,
    pub grosor: i32,
    pub tipo: i32,
}

impl Maqueta {
    pub fn a_texto(&self) -> String {
        format!(
            "{},{},{},{},{},{}",
            self.columna, self.izq, self.der, self.tamano, self.grosor, self.tipo
        )
    }

    /// `deTexto`: seis enteros con coma; una columna de cero o menos, o
    /// cualquier cosa que no sea un entero, no es maqueta.
    pub fn de_texto(t: &str) -> Option<Maqueta> {
        let n: Vec<i32> = t
            .trim()
            .split(',')
            .map(|x| x.trim().parse::<i32>().ok())
            .collect::<Option<Vec<_>>>()?;
        if n.len() != 6 || n[0] <= 0 {
            return None;
        }
        Some(Maqueta {
            columna: n[0],
            izq: n[1],
            der: n[2],
            tamano: n[3],
            grosor: n[4],
            tipo: n[5],
        })
    }
}

/// **El marco de la tinta** (idea del usuario, 29-sep-2026: «un marco
/// invisible [...] como puntos de referencia que siempre deben ir en algun
/// lugar, que la tinta se ajuste con respecto al marco»).
///
/// Cada aparato escribe la tinta de un documento en las unidades de su capa,
/// y esas unidades dependian de cosas que el otro tenia que adivinar (los
/// espacios del PDF, el margen izquierdo de un Word). El marco las dice: es
/// **donde esta la hoja** —la pagina de un PDF, la columna de un Word o un
/// libro— en las mismas unidades en que estan los puntos de su fichero de
/// tinta. Quien lee lleva ese rectangulo al de su hoja y la tinta cae encima
/// sin adivinar nada.
///
/// Va en un fichero hermano de la tinta (`anot-<uid>-p<n>.hoja`,
/// `anot-<uid>.hoja`), no dentro de la escena: la `Scene` del movil tira la
/// escena entera si trae un tipo que no conoce, y un elemento marco se
/// pintaria como un recuadro en un aparato viejo.
///
/// El texto es una linea `x0,y0,x1,y1` con punto decimal y, opcional, una
/// segunda con la version (`v1`). Para un Word o un libro el marco es la
/// banda de la columna desde lo alto del documento, **tan alta como ancha**
/// (`izq,0,izq+columna,columna`): el alto del documento depende de quien lo
/// maqueta (el `WebView` suma su relleno), y el texto crece a la escala de
/// su columna, asi que la escala vertical tiene que ser la misma.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarcoDeLaHoja {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

/// La unica version que se entiende. Otra es un formato que aun no se sabe
/// leer: mejor la regla vieja que encajar con numeros que dicen otra cosa.
const VERSION_DEL_MARCO: &str = "v1";

/// Un numero del marco: hasta tres decimales, sin ceros de cola ni `-0`.
/// Tres bastan (un milesimo de unidad) y un numero corto se lee igual en
/// Kotlin (`toFloatOrNull`) y en Rust.
fn numero(v: f32) -> String {
    let t = format!("{v:.3}");
    let t = t.trim_end_matches('0').trim_end_matches('.');
    if t == "-0" { "0".into() } else { t.to_string() }
}

impl MarcoDeLaHoja {
    pub fn nuevo(x0: f32, y0: f32, x1: f32, y1: f32) -> MarcoDeLaHoja {
        MarcoDeLaHoja { x0, y0, x1, y1 }
    }

    pub fn ancho(&self) -> f32 {
        self.x1 - self.x0
    }

    pub fn alto(&self) -> f32 {
        self.y1 - self.y0
    }

    /// Un marco con que encajar: numeros de verdad y con area.
    pub fn valido(&self) -> bool {
        [self.x0, self.y0, self.x1, self.y1].iter().all(|v| v.is_finite())
            && self.ancho() > 1e-3
            && self.alto() > 1e-3
    }

    pub fn a_texto(&self) -> String {
        format!(
            "{},{},{},{}\n{VERSION_DEL_MARCO}\n",
            numero(self.x0),
            numero(self.y0),
            numero(self.x1),
            numero(self.y1)
        )
    }

    /// `deTexto`: cuatro numeros con coma en la primera linea y, si hay
    /// segunda, que sea `v1`. Lo que no, `None`: se ignora y se lee con la
    /// regla de antes.
    pub fn de_texto(t: &str) -> Option<MarcoDeLaHoja> {
        let mut lineas = t.lines().map(str::trim).filter(|l| !l.is_empty());
        let n: Vec<f32> = lineas
            .next()?
            .split(',')
            .map(|x| x.trim().parse::<f32>().ok())
            .collect::<Option<Vec<_>>>()?;
        if let Some(v) = lineas.next()
            && v != VERSION_DEL_MARCO
        {
            return None;
        }
        if n.len() != 4 || lineas.next().is_some() {
            return None;
        }
        let m = MarcoDeLaHoja::nuevo(n[0], n[1], n[2], n[3]);
        m.valido().then_some(m)
    }

    /// **Como se lleva un punto de `propio` a este marco**, eje por eje:
    /// `(ex, ey, dx, dy)` con `aqui = propio * e + d`. Si las proporciones no
    /// coinciden, cada eje con la suya (se encaja al marco, que es lo que
    /// manda); quien la use lo dice en el registro.
    pub fn desde(&self, propio: &MarcoDeLaHoja) -> (f32, f32, f32, f32) {
        let ex = self.ancho() / propio.ancho();
        let ey = self.alto() / propio.alto();
        (ex, ey, self.x0 - propio.x0 * ex, self.y0 - propio.y0 * ey)
    }
}

/// `Lectura.margenDe` en enteros, como lo guarda el movil: dos tercios de la columna.
pub fn margen_de(columna: i32) -> i32 {
    columna * 2 / 3
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_nombres_son_los_del_movil() {
        assert_eq!(de_pagina("ABCDE23456", 2), "anot-ABCDE23456-p2");
        assert_eq!(del_pdf("ABCDE23456"), "anot-ABCDE23456");
        assert_eq!(del_documento("ABCDE23456", "2_informe.docx"), "anot-ABCDE23456");
        assert_eq!(del_documento("ABCDE23456", "Plano.PDF"), "anot-ABCDE23456-texto");
        assert_eq!(rel("anot-ABCDE23456", ".marcas"), "pins/draw/anot-ABCDE23456.marcas");
    }

    #[test]
    fn solo_lo_de_un_mensaje_con_su_codigo_entero_y_sin_temporales() {
        let rels = [
            "pins/draw/anot-ABCDE23456-p0.excalidraw.gz",
            "pins/draw/anot-ABCDE23456.marcas",
            "pins/draw/anot-ABCDE23456.marcas.tmp",
            "pins/draw/anot-CORTO.marcas",
            "pins/draw/d1.excalidraw.gz",
            "pins/draw/files/anot-ABCDE23456.marcas",
            "guardados/anot-ABCDE23456.marcas",
        ];
        let m = por_uid(rels);
        assert_eq!(m.len(), 1);
        assert_eq!(
            m["ABCDE23456"],
            vec![
                "pins/draw/anot-ABCDE23456-p0.excalidraw.gz".to_string(),
                "pins/draw/anot-ABCDE23456.marcas".into()
            ]
        );
        assert_eq!(uid_del_nombre("anot-ABCDE23456-texto.maqueta"), Some("ABCDE23456"));
        assert_eq!(uid_del_nombre("anot-ABCDE23456.pdf"), None);
    }

    #[test]
    fn las_marcas_de_un_lienzo_van_junto_a_su_dibujo() {
        assert_eq!(
            marcas_junto_a("pins/draw/d1.excalidraw.gz").as_deref(),
            Some("pins/draw/d1.marcas")
        );
        // Caso negativo: ni lo de otra carpeta ni lo que no es un dibujo.
        assert_eq!(marcas_junto_a("pins/draw/files/x.excalidraw.gz"), None);
        assert_eq!(marcas_junto_a("croquis3d/c.croquis.gz"), None);
        assert_eq!(marcas_junto_a("pins/drawx/d.excalidraw.gz"), None);
    }

    #[test]
    fn la_maqueta_se_lee_y_se_escribe_como_en_el_movil() {
        let m = Maqueta::de_texto("420,280,280,100,1,0").unwrap();
        assert_eq!(
            m,
            Maqueta { columna: 420, izq: 280, der: 280, tamano: 100, grosor: 1, tipo: 0 }
        );
        assert_eq!(m.a_texto(), "420,280,280,100,1,0");
        assert_eq!(Maqueta::de_texto(" 384, 256,256,100,1,0\n").map(|m| m.columna), Some(384));
        // Casos negativos del movil.
        assert_eq!(Maqueta::de_texto("0,1,2,3,4,5"), None);
        assert_eq!(Maqueta::de_texto("420,x,2,3,4,5"), None);
        assert_eq!(Maqueta::de_texto("420,1,2"), None);
        assert_eq!(Maqueta::de_texto(""), None);
        assert_eq!(margen_de(384), 256);
    }

    #[test]
    fn el_marco_de_la_hoja_viaja_con_su_tinta_y_se_borra_con_su_mensaje() {
        let rels = [
            "pins/draw/anot-ABCDE23456-p0.excalidraw.gz",
            "pins/draw/anot-ABCDE23456-p0.hoja",
            "pins/draw/anot-ABCDE23456.hoja",
            "pins/draw/anot-ABCDE23456-p0.hoja.tmp",
            "pins/draw/pdf-1a2b-p0.hoja",
        ];
        let m = por_uid(rels);
        assert_eq!(m["ABCDE23456"].len(), 3, "{m:?}");
        assert_eq!(uid_del_nombre("anot-ABCDE23456-p12.hoja"), Some("ABCDE23456"));
        // Caso negativo: el temporal no viaja.
        assert_eq!(uid_del_nombre("anot-ABCDE23456-p0.hoja.tmp"), None);
        assert_eq!(rel(&de_pagina("ABCDE23456", 3), HOJA), "pins/draw/anot-ABCDE23456-p3.hoja");
    }

    #[test]
    fn el_marco_se_escribe_corto_y_se_lee_igual() {
        let m = MarcoDeLaHoja::nuevo(-1050.0, 0.0, 2450.0, 4950.0);
        assert_eq!(m.a_texto(), "-1050,0,2450,4950\nv1\n");
        assert_eq!(MarcoDeLaHoja::de_texto(&m.a_texto()), Some(m));
        let raro = MarcoDeLaHoja::nuevo(0.12345, -0.0001, 1400.5, 1979.99);
        assert_eq!(raro.a_texto(), "0.123,0,1400.5,1979.99\nv1\n");
        // Sin version, con espacios y con el `1400.0` de Kotlin tambien vale.
        assert_eq!(
            MarcoDeLaHoja::de_texto(" 0.0, 0.0 ,1400.0,1980.0 \r\n"),
            Some(MarcoDeLaHoja::nuevo(0.0, 0.0, 1400.0, 1980.0))
        );
    }

    #[test]
    fn un_marco_mal_formado_no_es_marco() {
        for t in [
            "",
            "1,2,3",
            "1,2,3,4,5",
            "0,0,x,10",
            "0,0,1400,1980\nv2",
            "0,0,1400,1980\nv1\nmas",
            "10,0,10,1980",
            "0,50,1400,20",
            "0,0,NaN,1980",
            "0,0,inf,1980",
            "0;0;1400;1980",
        ] {
            assert_eq!(MarcoDeLaHoja::de_texto(t), None, "{t:?}");
        }
    }

    #[test]
    fn el_encaje_lleva_la_hoja_propia_exactamente_al_marco() {
        let propio = MarcoDeLaHoja::nuevo(0.0, 0.0, 1400.0, 1980.0);
        let fichero = MarcoDeLaHoja::nuevo(-1050.0, 0.0, 2450.0, 4950.0);
        let (ex, ey, dx, dy) = fichero.desde(&propio);
        assert_eq!((ex, ey, dx, dy), (2.5, 2.5, -1050.0, 0.0));
        // Proporciones distintas: cada eje con la suya.
        let otro = MarcoDeLaHoja::nuevo(100.0, 50.0, 800.0, 1050.0);
        let (ex, ey, dx, dy) = otro.desde(&propio);
        assert!((ex - 0.5).abs() < 1e-6 && (ey - 1000.0 / 1980.0).abs() < 1e-6);
        assert_eq!((dx, dy), (100.0, 50.0));
    }
}
