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

#[cfg(test)]
mod volcar {
    /// Vuelca los textos de un plano que contengan algo (para investigar).
    #[test]
    #[ignore = "vuelca textos de una muestra"]
    fn volcar_textos() {
        use opencadcodec::entities::EntityType;
        let ruta = std::env::var("PIXPIN_CAD_PLANO").unwrap();
        let buscar = std::env::var("PIXPIN_CAD_BUSCAR").unwrap_or_default();
        let doc = super::leer(std::path::Path::new(&ruta)).unwrap();
        let mut n = 0;
        let coincide = |v: &str| match buscar.strip_prefix('=') { Some(e) => crate::texto::texto_plano(v).trim() == e, None => v.contains(&buscar) };
        let mut ver = |e: &EntityType, donde: &str| {
            match e {
                EntityType::MText(t) if coincide(&t.value) && n < 12 => {
                    n += 1;
                    println!("MTEXT[{donde}] en={:?} {:?} h={} w={} ls={} est={:?} ap={:?} rot={} dir={:?}", t.insertion_point, t.value, t.height, t.rectangle_width, t.line_spacing_factor, t.style, t.attachment_point, t.rotation, t.dwg_x_direction);
                }
                EntityType::Text(t) if coincide(&t.value) && n < 12 => {
                    n += 1;
                    println!("TEXT[{donde}] {:?} h={} wf={} est={:?} h_al={:?} v_al={:?}", t.value, t.height, t.width_factor, t.style, t.horizontal_alignment, t.vertical_alignment);
                }
                EntityType::Insert(ins) => {
                    for at in &ins.attributes {
                        if coincide(&at.value) && n < 12 {
                            n += 1;
                            println!("ATTRIB[{donde}] bloque={:?} esc=({},{}) rot={} en={:?} {:?} h={} rot={} ali=({:?},{:?}) mt={}", ins.block_name, ins.x_scale(), ins.y_scale(), ins.rotation, at.insertion_point, at.value, at.height, at.rotation, at.horizontal_alignment, at.vertical_alignment, at.embedded_mtext.is_some());
                        }
                    }
                }
                _ => {}
            }
        };
        for e in doc.model_space_entities() {
            ver(e, "modelo");
        }
        for br in doc.block_records.iter() {
            for e in doc.entities_in_block(&br.name) {
                ver(e, &br.name);
            }
        }
        for s in doc.text_styles.iter() {
            println!("ESTILO {:?} fuente={:?} big={:?} ttf={:?} alto={} ancho={}", s.name, s.font_file, s.big_font_file, s.true_type_font, s.height, s.width_factor);
        }
    }
}
