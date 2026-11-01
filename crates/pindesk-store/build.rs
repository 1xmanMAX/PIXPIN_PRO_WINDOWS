//! Mete en el binario los catalogos de idioma que existan.
//!
//! Cada idioma vive en `i18n/<etiqueta>/main.ftl`. Español e ingles tienen
//! que estar siempre (son la red de los demas); el resto puede faltar: un
//! idioma sin fichero sale entero en ingles, y uno a medias, con lo que
//! falte en ingles (ver `src/idioma.rs`). Asi un traductor deja su fichero
//! en la carpeta y basta con compilar: no hay que tocar codigo.
//!
//! La lista de etiquetas tiene que ir en el mismo orden que
//! `Idioma::TODOS`; un test lo comprueba.

use std::fmt::Write as _;
use std::path::Path;

const ETIQUETAS: [&str; 14] = [
    "es-ES", "en-US", "zh-CN", "hi-IN", "ar-SA", "pt-BR", "fr-FR", "de-DE", "ja-JP", "ru-RU",
    "ko-KR", "it-IT", "id-ID", "tr-TR",
];

fn main() {
    let raiz = std::env::var("CARGO_MANIFEST_DIR").expect("cargo da CARGO_MANIFEST_DIR");
    // Una carpeta: cargo mira todo lo que hay dentro, asi que un catalogo
    // nuevo vuelve a ejecutar este script.
    println!("cargo:rerun-if-changed=i18n");
    println!("cargo:rerun-if-changed=build.rs");

    let mut salida = String::from(
        "// Generado por build.rs: un catalogo por idioma, `None` si no hay fichero.\n\
         pub(crate) const CATALOGOS: [(&str, Option<&str>); 14] = [\n",
    );
    for etiqueta in ETIQUETAS {
        let fichero = Path::new(&raiz)
            .join("i18n")
            .join(etiqueta)
            .join("main.ftl");
        if fichero.is_file() {
            let ruta = fichero.to_str().expect("la ruta del crate es UTF-8");
            writeln!(salida, "    ({etiqueta:?}, Some(include_str!({ruta:?}))),").unwrap();
        } else {
            assert!(
                etiqueta != "es-ES" && etiqueta != "en-US",
                "falta {}: español e ingles son obligatorios",
                fichero.display()
            );
            writeln!(salida, "    ({etiqueta:?}, None),").unwrap();
        }
    }
    salida.push_str("];\n");

    let destino =
        Path::new(&std::env::var("OUT_DIR").expect("cargo da OUT_DIR")).join("catalogos.rs");
    std::fs::write(destino, salida).expect("se puede escribir en OUT_DIR");
}
