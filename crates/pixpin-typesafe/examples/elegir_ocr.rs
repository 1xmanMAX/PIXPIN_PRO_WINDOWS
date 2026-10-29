//! **Mas casos B5: que OCR lee cada imagen** (`motor_ocr`, con imagenes).
//! Cada caso, una imagen (`# imagen:`) y, si se quiere comparar, lo que
//! leyo cada motor (`# windows: …`, `# indra: …`, escritos a mano con los
//! fallos tipicos de cada uno). Una peticion por imagen; la tabla de
//! `motor_ocr::motor` elige el motor (con Indra bajado) y
//! `motor_ocr::lectura` con que lectura se queda.
//!
//! `cargo run -q -p pixpin-typesafe --example elegir_ocr -- ocr.txt --rapido`
//!
//! Hechos: «o01 tipo captura_de_pantalla», «o01 motor windows», «o01
//! lectura indra», «o05 sin texto».

mod comun;

use comun::casos::{self, p};
use pixpin_typesafe::motor_ocr::{self, Eleccion, Motor};

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar("elegir_ocr", "que motor de OCR y que lectura");
    for c in casos::casos(&s.guion) {
        let ruta = c.texto("imagen");
        let img = casos::imagen(&s.guion, &ruta);
        let mut lecturas = Vec::new();
        if let Some(w) = c.valor("windows") {
            lecturas.push((Motor::Windows, w.to_string()));
        }
        if let Some(i) = c.valor("indra") {
            lecturas.push((Motor::Indra, i.to_string()));
        }
        println!("── {}: {ruta}", c.id);
        let Some(x) = s.preguntar(|cl| motor_ocr::preguntar(cl, &img, &lecturas)) else {
            continue;
        };
        println!(
            "    hay texto {} · tipo {} {} · mejor lectura {}",
            p(x.hay_texto),
            x.tipo.0.id(),
            p(x.tipo.1),
            x.mejor
                .map_or("—".to_string(), |(m, q)| format!("{} {}", m.id(), p(q)))
        );
        let ahora = s.reloj.ahora();
        match motor_ocr::motor(&x, true) {
            Eleccion::Ninguno => {
                s.hecho(ahora, format!("{} sin texto", c.id));
                continue;
            }
            Eleccion::Este { motor, .. } => {
                s.hecho(ahora, format!("{} tipo {}", c.id, x.tipo.0.id()));
                s.hecho(ahora, format!("{} motor {}", c.id, motor.id()));
            }
            Eleccion::ElDeSiempre => {
                s.hecho(ahora, format!("{} tipo dudoso ({})", c.id, x.tipo.0.id()));
                s.hecho(ahora, format!("{} motor de los ajustes", c.id));
            }
        }
        if lecturas.len() > 1 {
            let l = motor_ocr::lectura(&x, Motor::Windows);
            s.hecho(ahora, format!("{} lectura {}", c.id, l.id()));
        }
    }
    s.terminar()
}
