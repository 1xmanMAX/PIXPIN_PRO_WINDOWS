//! Escribe cada figura de serie como SVG (y una pagina con todas) en la
//! carpeta que se le pase, para mirarlas sin abrir la app.
//!
//! `cargo run -p pixpin-motor2d --example figuras_de_serie_svg -- <carpeta>`

use pixpin_motor2d::elemento::ColorRgba;
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::exportar::{self, Alcance};
use pixpin_motor2d::exportar_svg::{self, OpcionesSvg};
use pixpin_motor2d::figuras_de_serie;

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("carpeta"));
    std::fs::create_dir_all(&dir).unwrap();
    let mut pagina = String::from("<html><body style='background:#2a2d34;font:12px sans-serif;color:#ddd;margin:0'><div style='display:flex;flex-wrap:wrap;gap:12px;padding:12px'>");
    for f in figuras_de_serie::catalogo() {
        let mut escena = Escena::nueva();
        for e in &f.elementos {
            escena.anadir(e.clone());
        }
        let hojas = exportar::hojas(&escena, Alcance::Todo, &[], None);
        let Some(hoja) = hojas.first() else { continue };
        let svg = exportar_svg::svg(hoja, OpcionesSvg { fondo: Some(ColorRgba::opaco(1.0, 1.0, 1.0)), marcos: false }, &|_| None);
        let nombre = format!("{}.svg", f.id);
        std::fs::write(dir.join(&nombre), &svg).unwrap();
        pagina.push_str(&format!(
            "<div style='width:260px'><div style='background:#fff;height:300px;display:flex;align-items:center;justify-content:center'><img src='{nombre}' style='max-width:250px;max-height:290px'></div><div>{} ({} piezas)</div></div>",
            f.nombre,
            f.elementos.len()
        ));
    }
    pagina.push_str("</div></body></html>");
    std::fs::write(dir.join("todas.html"), pagina).unwrap();
}
