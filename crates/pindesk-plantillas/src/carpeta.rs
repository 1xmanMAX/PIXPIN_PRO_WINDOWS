//! **La carpeta compartida con PinDesk**: `<datos de PinDesk>/plantillas/`
//! (en Windows, `%APPDATA%\PinDesk\plantillas`, o junto al `.exe` en la
//! copia portable). Plantillas guarda ahi lo que se «manda a PinDesk», y la
//! ventana de Figuras de PinDesk la lee y enseña cada plantilla como una
//! figura mas, en la categoria «Plantillas» (ver `docs/plantillas.md`).

use std::path::{Path, PathBuf};

use pindesk_motor2d::biblioteca::FiguraGuardada;

use crate::exportar;
use crate::fecha::Fecha;
use crate::formato::{self, EXTENSION};
use crate::modelo::Documento;

/// El nombre de la carpeta dentro de la de datos de PinDesk.
pub const CARPETA: &str = "plantillas";
/// Cuantas se leen como mucho (una carpeta llena de otra cosa no cuelga
/// Figuras).
pub const MAX_FICHEROS: usize = 200;
/// Cuantas paginas de cada una van a la figura.
pub const PAGINAS_EN_FIGURA: usize = 1;

/// La carpeta en una raiz de datos de PinDesk.
pub fn en(raiz: &Path) -> PathBuf {
    raiz.join(CARPETA)
}

/// Los `.pdplantilla` de una carpeta, por nombre.
pub fn listar(carpeta: &Path) -> Vec<PathBuf> {
    let Ok(dir) = std::fs::read_dir(carpeta) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = dir
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION))
        })
        .take(MAX_FICHEROS)
        .collect();
    v.sort();
    v
}

/// **La clave de un fichero** (su nombre sin extension, solo letras,
/// numeros y guiones): el id de su figura es `plantilla-<clave>`.
pub fn clave_de(ruta: &Path) -> String {
    let nombre = ruta
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("plantilla");
    limpiar(nombre)
}

fn limpiar(s: &str) -> String {
    let mut c: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_lowercase().next().unwrap_or(c)
            } else {
                '-'
            }
        })
        .collect();
    while c.contains("--") {
        c = c.replace("--", "-");
    }
    let c = c.trim_matches('-').chars().take(60).collect::<String>();
    if c.is_empty() { "plantilla".into() } else { c }
}

/// **Las figuras para PinDesk**: cada plantilla de la carpeta con su primera
/// pagina dibujada para `hoy`. Las que no se leen se saltan.
pub fn figuras_de(carpeta: &Path, hoy: Fecha) -> Vec<FiguraGuardada> {
    listar(carpeta)
        .iter()
        .filter_map(|r| {
            let texto = std::fs::read_to_string(r).ok()?;
            let (doc, _) = formato::leer(&texto).ok()?;
            Some(figura_de(&doc, &clave_de(r), hoy))
        })
        .collect()
}

/// La figura de un documento.
pub fn figura_de(doc: &Documento, clave: &str, hoy: Fecha) -> FiguraGuardada {
    let paginas: Vec<_> = exportar::paginas(doc, hoy)
        .into_iter()
        .take(PAGINAS_EN_FIGURA)
        .collect();
    exportar::figura::figura(clave, &doc.nombre, &paginas, &doc.tema)
}

/// **El fichero de una figura de Plantillas** (`plantilla-<clave>`), para
/// «Abrir en Plantillas» desde PinDesk.
pub fn ruta_de_figura(carpeta: &Path, id: &str) -> Option<PathBuf> {
    let clave = id.strip_prefix(exportar::figura::PREFIJO_ID)?;
    listar(carpeta).into_iter().find(|r| clave_de(r) == clave)
}

/// Si una figura de PinDesk viene de Plantillas.
pub fn es_de_plantillas(id: &str) -> bool {
    id.starts_with(exportar::figura::PREFIJO_ID)
}

/// **Guarda un documento en la carpeta** con un nombre que sale del suyo.
/// Si ya hay uno de ese nombre, lo sustituye (mandar otra vez la misma
/// plantilla la pone al dia en Figuras).
pub fn guardar_en(carpeta: &Path, doc: &Documento) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(carpeta)?;
    let ruta = carpeta.join(format!("{}.{EXTENSION}", limpiar(&doc.nombre)));
    let tmp = ruta.with_extension("tmp");
    std::fs::write(&tmp, formato::escribir(doc))?;
    std::fs::rename(&tmp, &ruta)?;
    Ok(ruta)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::textos::Textos;

    fn temporal(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pindesk-plantillas-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn mandar_a_pindesk_y_volver_a_abrir() {
        let raiz = temporal("carpeta");
        let c = en(&raiz);
        let hoy = Fecha::nueva(2026, 11, 6).unwrap();
        let doc = crate::galeria::por_clave("semanal", &Textos::nuevos("es"), hoy).unwrap();
        let ruta = guardar_en(&c, &doc).unwrap();
        assert!(
            ruta.ends_with("planificador-semanal.pdplantilla"),
            "{ruta:?}"
        );
        // Otra vez: la misma, no una segunda.
        guardar_en(&c, &doc).unwrap();
        std::fs::write(c.join("rota.pdplantilla"), "no es json").unwrap();
        std::fs::write(c.join("otra.txt"), "{}").unwrap();
        assert_eq!(listar(&c).len(), 2);
        let figuras = figuras_de(&c, hoy);
        assert_eq!(figuras.len(), 1, "la rota se salta");
        let f = &figuras[0];
        assert_eq!(f.id, "plantilla-planificador-semanal");
        assert!(f.nombre.starts_with("Plantillas · "));
        assert!(f.elementos.len() > 50);
        let (w, h) = f.tamano();
        assert!(
            (w - 794.0).abs() < 2.0 && (h - 1123.0).abs() < 2.0,
            "{w} × {h}: un A4"
        );
        assert!(es_de_plantillas(&f.id) && !es_de_plantillas("serie-semanal"));
        assert_eq!(ruta_de_figura(&c, &f.id), Some(ruta));
        assert_eq!(ruta_de_figura(&c, "plantilla-no-esta"), None);
        assert_eq!(
            clave_de(Path::new("Mi Diario 2027!.pdplantilla")),
            "mi-diario-2027"
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
