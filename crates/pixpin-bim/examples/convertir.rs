//! cargo run --release -p pixpin-bim --example convertir -- modelo.ifc [salida.px3d]
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let t = std::time::Instant::now();
    match pixpin_bim::convertir_fichero(std::path::Path::new(&a[1])) {
        Ok(m) => {
            println!(
                "{} ms · {} vertices · {} triangulos opacos · {} transparentes · {} aristas · {} elementos · caja {:?} · origen {:?}",
                t.elapsed().as_millis(),
                m.vertices.len(),
                m.opacos.len() / 3,
                m.transparentes.len() / 3,
                m.aristas.len() / 2,
                m.elementos.len(),
                m.caja,
                m.origen
            );
            let mut tipos: std::collections::BTreeMap<&str, usize> = Default::default();
            for e in &m.elementos {
                *tipos.entry(&e.tipo).or_default() += 1;
            }
            println!("{tipos:?}");
            if let Some(s) = a.get(2) {
                std::fs::write(s, m.a_bytes()).unwrap();
            }
        }
        Err(e) => println!("error: {e}"),
    }
}
