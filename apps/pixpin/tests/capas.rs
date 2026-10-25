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
        // `pixpin-timeline` es el del timeline (5-oct): formato, fichero,
        // frases clave, dias y la pagina exportada; solo serde y el disco.
        // `pixpin-lecciones` es la logica de las lecciones aprendidas copiada
        // del movil (JSON, buscador, etiquetador, repaso): solo serde_json, y
        // la usa `pixpin-sincro` para juntar dos versiones de una leccion.
        // `pixpin-docs` (cuentas del texto), `pixpin-hojas` (las hojas de
        // calculo) y `pixpin-cad` (el visor de planos) no dependen de ningun
        // crate de PixPin: tambien son cimientos.
        // `pixpin-sistema` es lo poco que la logica le pregunta al sistema
        // (hora local, carpeta de datos, atajos, uniones), con version de
        // Windows y portable: asi `pixpin-store` y `pixpin-proyecto` no
        // arrastran `pixpin-shell` y compilan tambien en Mac.
        "pixpin-geom" | "pixpin-nivel" | "pixpin-lecciones" | "pixpin-timeline"
        // `pixpin-frase` (18-oct) entiende frases de tareas y fechas, sin nadie.
        | "pixpin-docs" | "pixpin-hojas" | "pixpin-cad" | "pixpin-frase" | "pixpin-sistema"
        // El motor de audio (25-oct-2026): WASAPI, la mezcla y el .m4a, sin
        // ningun crate de PixPin; `pixpin-audio` lo usa por debajo.
        | "pixpin-motor-audio" => 0,
        // `pixpin-sincro` es el cable de WiFi: solo criptografia y JSON, sin
        // depender de ningun crate de PixPin, asi que vive abajo del todo
        // con los demas cimientos.
        // `pixpin-audio` es otro cimiento del mismo estilo: Media
        // Foundation para oir y WASAPI para grabar, sin depender de ningun
        // crate de PixPin.
        // `pixpin-voz` es el tercero: pasar una nota de voz a texto (Media
        // Foundation para decodificar, Vosk cargado al vuelo) y la cuenta
        // del telepronter. Tampoco depende de ningun crate de PixPin.
        // `pixpin-web` es el visor de HTML sobre WebView2: habla con Windows
        // y con nadie de PixPin salvo la geometria, como los demas de aqui.
        // `pdfsqueeze-core` es el compresor de PDF del autor, copiado de
        // Thesis (ver su PROCEDENCIA.md): solo lopdf, image y compania, sin
        // ningun crate de PixPin ni de Windows. Cimiento puro.
        // `pixpin-office` (Word/PowerPoint/Excel sin Office) solo usa
        // `pixpin-docs`; `pixpin-bim` solo usa `pixpin-cad`.
        "pixpin-shell" | "pixpin-codec" | "pixpin-motor2d" | "pixpin-sincro"
        | "pixpin-tinta" | "pixpin-audio" | "pixpin-voz" | "pixpin-web" | "pdfsqueeze-core"
        // Las tarjetas de Anki (24-oct-2026): sin crates de PixPin, solo red.
        | "pixpin-anki"
        | "pixpin-office" | "pixpin-bim" => 1,
        // El plugin de Flow Launcher es un ejecutable aparte que no enlaza
        // ningun crate de PixPin: habla con la app por WM_COPYDATA. La app si
        // usa su libreria (el borrador de las imagenes pegadas en Flow).
        "pixpin-lanzador" => 2,
        "pixpin-capture" | "pixpin-pdf" | "pixpin-ocr" | "pixpin-record" | "pixpin-store"
        | "pixpin-proyecto" => 2,
        // El compresor de PDF en su propio ejecutable: solo pdfsqueeze y la
        // prioridad de pixpin-shell. `pixpin` solo lo usa en sus pruebas.
        "pixpin-aligerar" => 2,
        // El puente de la extension del navegador: un exe que solo usa
        // `pixpin-shell`.
        "pixpin-puente" => 2,
        // `pixpin-render` pinta las vistas previas de PDF, Office y hojas, asi
        // que va encima de `pixpin-pdf` (capa 2).
        // El motor de video (25-oct-2026) usa la captura y, como ultimo
        // recurso, el codificador de `pixpin-record` (los dos de la capa 2).
        "pixpin-ui" | "pixpin-render" | "pixpin-motor-video" => 3,
        // `pixpin-notas` es la ventana del editor de notas Markdown (H12): usa
        // los componentes de `pixpin-ui` y `pixpin-render`, asi que va encima.
        // Los pines y la pila tambien pintan con `pixpin-render`.
        "pixpin-notas" | "pixpin-pin" | "pixpin-pila" => 4,
        // El asistente usa el editor de notas, el OCR, los PDF y la voz.
        "pixpin-ia" => 5,
        // La app y el servidor MCP, que usan el asistente.
        "pixpin" | "pixpin-mcp" => 6,
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
fn estan_los_cuarenta_paquetes() {
    let encontrados = manifiestos();
    // 32 con `pixpin-sistema` (lo que la logica pregunta al sistema, con
    // version portable: la usa la copia del nucleo de Mac); 40 con los
    // motores de video y de audio.
    assert_eq!(
        encontrados.len(),
        40,
        "se esperan 35 crates de libreria y los cinco ejecutables, encontrados: {:?}",
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

/// **pixpinmax.exe no enlaza el compresor de PDF.** pdfsqueeze y sus codecs
/// son ~1,8 MB que viven en su propio ejecutable (`pixpin-aligerar`), al que
/// pixpinmax lanza como proceso. Basta una linea en `[dependencies]` para
/// volver a meterlos en el programa que se abre en cada arranque de Windows;
/// en `[dev-dependencies]` si pueden estar, que eso no llega al ejecutable.
#[test]
fn pixpinmax_no_enlaza_el_compresor_de_pdf() {
    let texto = fs::read_to_string(raiz().join("apps/pixpin/Cargo.toml")).unwrap();
    let doc: toml::Value = texto.parse().unwrap();
    let deps = doc["dependencies"].as_table().unwrap();
    for prohibida in ["pdfsqueeze-core", "pixpin-aligerar", "lopdf"] {
        assert!(
            !deps.iter().any(|(k, v)| nombre_real(k, v) == prohibida),
            "`{prohibida}` en [dependencies] de pixpin lo enlazaria en pixpinmax.exe"
        );
    }
    // Caso negativo de la propia prueba: el compresor si lo enlaza.
    let texto = fs::read_to_string(raiz().join("apps/pixpin-aligerar/Cargo.toml")).unwrap();
    let doc: toml::Value = texto.parse().unwrap();
    assert!(
        doc["dependencies"]
            .as_table()
            .unwrap()
            .contains_key("pdfsqueeze-core")
    );
}
