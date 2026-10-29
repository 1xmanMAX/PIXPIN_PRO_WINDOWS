//! **Mas casos B4: recortar la pizarra o la hoja de una foto** (`pizarra`,
//! con imagenes y Box). Cada caso, una foto (`# foto:`) y, si tiene
//! pizarra, su caja de verdad (`# verdad: top bottom left right`, en
//! fracciones, la que escribe `guiones/fotos/hacer_fotos.py`); se mide la
//! interseccion sobre union (IoU) de la caja de Jev y del recorte (con su
//! margen) contra la verdad.
//!
//! `cargo run -q -p pixpin-typesafe --example recortar_pizarra -- pizarras.txt --rapido`
//!
//! Hechos: «p01 recorte bien» (IoU del recorte ≥ [`BIEN`]), «p01 recorte
//! mal», «p01 falta trozo», «p01 sugerido», «p06 sin pizarra».

mod comun;

use comun::casos::{self, p};
use pixpin_typesafe::api::Caja;
use pixpin_typesafe::pizarra::{self, Recorte};

/// Lo que tiene que casar el recorte con la verdad para contar como bien.
const BIEN: f64 = 0.7;

fn caja(t: &str) -> Option<Caja> {
    let v: Vec<f64> = t
        .split_whitespace()
        .map(|x| x.parse().ok())
        .collect::<Option<_>>()?;
    match v.as_slice() {
        [top, bottom, left, right] => Some(Caja {
            top: *top,
            bottom: *bottom,
            left: *left,
            right: *right,
        }),
        _ => None,
    }
}

fn texto(c: &Caja) -> String {
    format!("{:.2} {:.2} {:.2} {:.2}", c.top, c.bottom, c.left, c.right)
}

fn main() -> std::process::ExitCode {
    let mut s = comun::empezar("recortar_pizarra", "el recorte de la pizarra o la hoja");
    let mut ious = Vec::new();
    for c in casos::casos(&s.guion) {
        let ruta = c.texto("foto");
        let foto = casos::imagen(&s.guion, &ruta);
        let verdad = c.valor("verdad").and_then(caja);
        println!("── {}: {ruta}", c.id);
        let Some((r, x)) = s.preguntar(|cl| pizarra::recortar(cl, &foto)) else {
            continue;
        };
        print!(
            "    hay {} · entera {} · caja {}",
            p(x.hay),
            p(x.entera),
            texto(&x.caja)
        );
        if let Some(v) = &verdad {
            print!(
                " · verdad {} · IoU caja {:.2}",
                texto(v),
                pizarra::iou(&x.caja, v)
            );
        }
        println!();
        let ahora = s.reloj.ahora();
        match r {
            Recorte::Hecho { caja, falta_trozo } => {
                if let Some(v) = &verdad {
                    let i = pizarra::iou(&caja, v);
                    ious.push(i);
                    let bien = if i >= BIEN { "bien" } else { "mal" };
                    s.hecho(ahora, format!("{} recorte {bien} (IoU {i:.2})", c.id));
                } else {
                    s.hecho(ahora, format!("{} recorte sin verdad", c.id));
                }
                if falta_trozo {
                    s.hecho(ahora, format!("{} falta trozo", c.id));
                }
            }
            Recorte::Sugerido(_) => s.hecho(ahora, format!("{} sugerido", c.id)),
            Recorte::Nada => s.hecho(ahora, format!("{} sin pizarra", c.id)),
        }
    }
    if !ious.is_empty() {
        println!(
            "\n   IoU medio de los recortes: {:.2} ({} recortes)",
            ious.iter().sum::<f64>() / ious.len() as f64,
            ious.len()
        );
    }
    s.terminar()
}
