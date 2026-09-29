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
pub const TERMINACIONES: [&str; 6] = [
    ".excalidraw.gz",
    ".marcas",
    ".espacios",
    ".maqueta",
    ".voz",
    ".sitio",
];

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
}
