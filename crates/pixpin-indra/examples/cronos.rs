//! Cuanto tarda cada paso de imagen (sin modelos) sobre un PGM binario.
//!
//! `cargo run --release -p pixpin-indra --example cronos -- pagina.pgm`
use pixpin_indra::{Gris, imagen, prep};

fn main() {
    let p = std::env::args().nth(1).expect("un .pgm");
    let b = std::fs::read(&p).unwrap();
    let s = String::from_utf8_lossy(&b[..20]).to_string();
    let c: Vec<&str> = s.split_whitespace().collect();
    let (w, h): (usize, usize) = (c[1].parse().unwrap(), c[2].parse().unwrap());
    let ini = b.len() - w * h;
    let g = Gris::de_buffer(w, h, b[ini..].to_vec()).unwrap();
    if let Ok(n) = std::env::var("HILOS") {
        pixpin_indra::gris::fijar_hilos(n.parse().unwrap());
    }
    let t = std::time::Instant::now();
    let pv = imagen::previa(&g, imagen::LADO_PREVIA);
    println!("previa {:?}", t.elapsed());
    let t = std::time::Instant::now();
    let a = imagen::estimar_inclinacion(&pv, 800);
    println!("inclinacion {a} {:?}", t.elapsed());
    let t = std::time::Instant::now();
    let _ = imagen::entrada_orientacion(&pv, 768);
    println!("entrada orientacion {:?}", t.elapsed());
    let t = std::time::Instant::now();
    let _ = imagen::entrada_detector(&g, 1024);
    println!("entrada detector {:?}", t.elapsed());
    let t = std::time::Instant::now();
    let _ = prep::normalizar_fondo(&g);
    println!("fondo {:?}", t.elapsed());
    let t = std::time::Instant::now();
    let _ = prep::enfocar(&g, 0.6, 1.0);
    println!("enfoque {:?}", t.elapsed());
}
