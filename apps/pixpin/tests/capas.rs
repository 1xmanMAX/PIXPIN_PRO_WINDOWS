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
        "pixpin-geom" | "pixpin-model" | "pixpin-nivel" => 0,
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
fn estan_los_diecisiete_paquetes() {
    let encontrados = manifiestos();
    assert_eq!(
        encontrados.len(),
        17,
        "se esperan 16 crates de libreria mas el ejecutable, encontrados: {:?}",
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

/// El nombre real del paquete que resuelve una entrada de dependencia.
///
/// Cargo permite renombrar una dependencia con la clave de la tabla y dejar
/// el nombre real del paquete en el campo `package`, por ejemplo:
/// `disfrazado = { package = "pixpin-shell", path = "../pixpin-shell" }`.
/// Si se compara solo contra la clave (`disfrazado`), la regla de capas se
/// puede burlar en silencio. Aqui se usa `package` cuando esta presente.
fn nombre_real<'a>(clave: &'a str, valor: &'a toml::Value) -> &'a str {
    valor
        .as_table()
        .and_then(|t| t.get("package"))
        .and_then(|p| p.as_str())
        .unwrap_or(clave)
}

/// Revisa una unica tabla de dependencias (p. ej. el contenido de
/// `[dependencies]` o de `[target.'cfg(windows)'.dev-dependencies]`) y hace
/// fallar el test si alguna entrada sube de capa.
fn revisar_tabla_dependencias(tabla: &toml::value::Table, nombre: &str, mia: u8, contexto: &str) {
    for (clave, valor) in tabla {
        let dep = nombre_real(clave, valor);
        let Some(suya) = capa(dep) else { continue };
        assert!(
            suya < mia,
            "`{nombre}` (capa {mia}) depende de `{dep}` (capa {suya}) en [{contexto}]. \
             Una capa solo puede depender de capas estrictamente inferiores."
        );
    }
}

#[test]
fn ninguna_dependencia_sube_de_capa() {
    const SECCIONES: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

    for (nombre, ruta) in manifiestos() {
        let mia = capa(&nombre).unwrap();
        let texto = fs::read_to_string(&ruta).unwrap();
        let doc: toml::Value = texto.parse().unwrap();

        for seccion in SECCIONES {
            if let Some(tabla) = doc.get(seccion).and_then(|v| v.as_table()) {
                revisar_tabla_dependencias(tabla, &nombre, mia, seccion);
            }
        }

        // Las dependencias condicionadas a una plataforma viven bajo
        // [target.<cfg-o-triple>.<seccion>]. En un proyecto solo-Windows,
        // por ejemplo [target.'cfg(windows)'.dependencies], es cuestion de
        // tiempo que se usen, y una violacion de capas ahi seria tan real
        // como una en [dependencies].
        if let Some(objetivos) = doc.get("target").and_then(|v| v.as_table()) {
            for (plataforma, tabla_plataforma) in objetivos {
                let Some(tabla_plataforma) = tabla_plataforma.as_table() else {
                    continue;
                };
                for seccion in SECCIONES {
                    if let Some(tabla) = tabla_plataforma.get(seccion).and_then(|v| v.as_table()) {
                        let contexto = format!("target.{plataforma}.{seccion}");
                        revisar_tabla_dependencias(tabla, &nombre, mia, &contexto);
                    }
                }
            }
        }
    }
}
