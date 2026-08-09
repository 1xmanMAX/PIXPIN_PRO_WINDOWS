//! Verifica la regla de capas del documento maestro: un crate solo puede
//! depender de crates de una capa estrictamente inferior.
//!
//! Se lee el Cargo.toml de cada miembro del workspace en vez de inspeccionar
//! el grafo de cargo porque asi el test no necesita compilar nada y corre en
//! milisegundos, tambien en CI sin Windows.

use std::fs;
use std::path::{Path, PathBuf};

/// Capa de cada crate. Numero menor = mas abajo en la arquitectura.
fn capa(nombre: &str) -> Option<u8> {
    Some(match nombre {
        "pixpin-geom" | "pixpin-model" => 0,
        "pixpin-shell" | "pixpin-render" | "pixpin-gpu" | "pixpin-codec" => 1,
        "pixpin-capture" | "pixpin-pin" | "pixpin-pdf" | "pixpin-ocr" | "pixpin-record"
        | "pixpin-store" => 2,
        "pixpin-ui" | "pixpin-flow" | "pixpin-plugin" => 3,
        "pixpin" => 4,
        _ => return None,
    })
}

/// Raiz del workspace, subiendo desde el directorio de este paquete.
fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("el paquete debe estar en <raiz>/apps/pixpin")
        .to_path_buf()
}

/// Devuelve (nombre, ruta del Cargo.toml) de cada crate de PixPin.
fn manifiestos() -> Vec<(String, PathBuf)> {
    let mut salida = Vec::new();
    for dir in [raiz().join("crates"), raiz().join("apps")] {
        for entrada in fs::read_dir(&dir).expect("falta el directorio del workspace") {
            let ruta = entrada.unwrap().path().join("Cargo.toml");
            if ruta.is_file() {
                let texto = fs::read_to_string(&ruta).unwrap();
                let doc: toml::Value = texto.parse().unwrap();
                let nombre = doc["package"]["name"].as_str().unwrap().to_string();
                salida.push((nombre, ruta));
            }
        }
    }
    salida
}

#[test]
fn estan_los_dieciseis_paquetes() {
    let encontrados = manifiestos();
    assert_eq!(
        encontrados.len(),
        16,
        "se esperan 15 crates de libreria mas el ejecutable, encontrados: {:?}",
        encontrados.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );
}

#[test]
fn todo_paquete_tiene_capa_asignada() {
    for (nombre, _) in manifiestos() {
        assert!(
            capa(&nombre).is_some(),
            "el paquete `{nombre}` no tiene capa asignada en el test; \
             añadelo a la funcion `capa` o revisa como se llama"
        );
    }
}

#[test]
fn ninguna_dependencia_sube_de_capa() {
    for (nombre, ruta) in manifiestos() {
        let mia = capa(&nombre).unwrap();
        let texto = fs::read_to_string(&ruta).unwrap();
        let doc: toml::Value = texto.parse().unwrap();

        for seccion in ["dependencies", "dev-dependencies", "build-dependencies"] {
            let Some(tabla) = doc.get(seccion).and_then(|v| v.as_table()) else {
                continue;
            };
            for dep in tabla.keys() {
                let Some(suya) = capa(dep) else { continue };
                assert!(
                    suya < mia,
                    "`{nombre}` (capa {mia}) depende de `{dep}` (capa {suya}) en [{seccion}]. \
                     Una capa solo puede depender de capas estrictamente inferiores."
                );
            }
        }
    }
}
