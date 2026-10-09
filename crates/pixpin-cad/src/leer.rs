//! Abrir un DWG o un DXF con `opencadcodec`.
use std::path::Path;

pub use opencadcodec::CadDocument;

/// Si los primeros bytes son de un DWG (`AC10xx`).
pub fn es_dwg(cabeza: &[u8]) -> bool {
    cabeza.len() >= 6 && cabeza.starts_with(b"AC10")
}

/// Lee el plano: primero estricto y, si se salto algo, otra vez en modo
/// tolerante (lo que hace OpenCADStudio).
pub fn leer(ruta: &Path) -> Result<CadDocument, String> {
    let mut cabeza = [0u8; 6];
    {
        use std::io::Read;
        let mut f = std::fs::File::open(ruta).map_err(|e| e.to_string())?;
        let _ = f.read(&mut cabeza);
    }
    if es_dwg(&cabeza) {
        use opencadcodec::{DwgReadOptions, DwgReader};
        let estricto = DwgReader::from_file(ruta)
            .map_err(|e| e.to_string())?
            .read_with_stats();
        match estricto {
            Ok(r) if r.stats.skipped_source_records == 0 && r.stats.stream_completed => Ok(r.document),
            _ => DwgReader::from_file_with_options(ruta, DwgReadOptions::failsafe())
                .map_err(|e| e.to_string())?
                .read()
                .map_err(|e| e.to_string()),
        }
    } else {
        use opencadcodec::{DxfReader, DxfReaderConfiguration};
        let estricto = DxfReader::from_file(ruta).map_err(|e| e.to_string())?.read_with_stats();
        match estricto {
            Ok(r) if r.stats.skipped_source_records == 0 && r.stats.stream_completed => Ok(r.document),
            _ => DxfReader::from_file(ruta)
                .map_err(|e| e.to_string())?
                .with_configuration(DxfReaderConfiguration {
                    failsafe: true,
                    default_encoding: None,
                })
                .read()
                .map_err(|e| e.to_string()),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    #[ignore = "lee los planos de muestra de E:/pixpin-cad/muestras"]
    fn lee_las_muestras() {
        for e in std::fs::read_dir("E:/pixpin-cad/muestras").unwrap().flatten() {
            let t = std::time::Instant::now();
            match leer(&e.path()) {
                Ok(doc) => {
                    let mut tipos = std::collections::BTreeMap::<String, usize>::new();
                    let mut n = 0;
                    for ent in doc.model_space_entities() {
                        *tipos.entry(ent.as_entity().entity_type().to_string()).or_default() += 1;
                        n += 1;
                    }
                    println!("{:?}: {:?} {} entidades, {} bloques, {:?}\n  {:?}", e.file_name(), doc.version, n, doc.block_records.iter().count(), t.elapsed(), tipos);
                }
                Err(err) => println!("{:?}: ERROR {err}", e.file_name()),
            }
        }
    }
}
