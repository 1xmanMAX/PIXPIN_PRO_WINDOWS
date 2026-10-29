//! **Recortar la pizarra o la hoja de una foto** (29-oct-2026, prototipo
//! `recortar_pizarra`, ver `docs/probar-ideas-sin-interfaz.md`, «Más
//! casos»): una foto de la pizarra de clase o de unos apuntes sobre la
//! mesa → el recorte de lo escrito. Jev dice si hay una (Noul), donde esta
//! (Box) y si se ve entera (Noul); el recorte y su margen los decide el
//! codigo.
//!
//! **Tabla** ([`decidir`]):
//!
//! | `hay_pizarra` | `entera` | se hace |
//! |---|---|---|
//! | ≥ SI | ≥ NO | recorte de la caja con [`MARGEN`] de cada lado |
//! | ≥ SI | < NO | recorte, avisando de que falta un trozo |
//! | entre NO y SI | — | se sugiere el recorte (sin hacerlo) |
//! | ≤ NO | — | nada: la foto queda entera |
//!
//! Una caja de menos del [`MINIMO`] de la foto no se recorta (sugerir).

use serde_json::json;

use crate::api::Caja;
use crate::preguntas::pizarra as q;
use crate::preguntas::{NO, SI};
use crate::{Cliente, Error, Imagen, Peticion};

/// Lo que se agranda la caja de cada lado (fraccion de la foto).
pub const MARGEN: f64 = 0.02;
/// La caja mas pequena que se recorta sola (fraccion del area).
pub const MINIMO: f64 = 0.04;

pub fn peticion(foto: &Imagen) -> Peticion {
    let r = crate::mejor_foto::referencia(foto);
    Peticion::nueva(json!({ "foto": r }))
        .con_imagen(&r, foto.clone())
        .con(q::HAY, q::hay())
        .con(q::DONDE, q::donde())
        .con(q::ENTERA, q::entera())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Senales {
    pub hay: f64,
    pub caja: Caja,
    pub entera: f64,
}

impl Senales {
    pub fn de(r: &crate::Respuestas) -> Result<Senales, Error> {
        Ok(Senales {
            hay: r.noul(q::HAY)?,
            caja: r.caja(q::DONDE)?,
            entera: r.noul(q::ENTERA)?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Recorte {
    /// Se recorta (`falta_trozo`: se avisa de que no se ve entera).
    Hecho {
        caja: Caja,
        falta_trozo: bool,
    },
    /// Se sugiere, sin hacerlo.
    Sugerido(Caja),
    Nada,
}

/// La caja con su margen, dentro de la foto.
pub fn con_margen(c: Caja, m: f64) -> Caja {
    Caja {
        top: (c.top - m).max(0.0),
        bottom: (c.bottom + m).min(1.0),
        left: (c.left - m).max(0.0),
        right: (c.right + m).min(1.0),
    }
}

pub fn area(c: &Caja) -> f64 {
    (c.right - c.left).max(0.0) * (c.bottom - c.top).max(0.0)
}

/// **Interseccion sobre union** de dos cajas (para medir el recorte contra
/// la verdad).
pub fn iou(a: &Caja, b: &Caja) -> f64 {
    let i = Caja {
        top: a.top.max(b.top),
        bottom: a.bottom.min(b.bottom),
        left: a.left.max(b.left),
        right: a.right.min(b.right),
    };
    let inter = area(&i);
    let union = area(a) + area(b) - inter;
    if union <= 0.0 { 0.0 } else { inter / union }
}

/// **La tabla.**
pub fn decidir(s: &Senales) -> Recorte {
    if s.hay <= NO {
        return Recorte::Nada;
    }
    let caja = con_margen(s.caja, MARGEN);
    if s.hay < SI || area(&s.caja) < MINIMO {
        return Recorte::Sugerido(caja);
    }
    Recorte::Hecho {
        caja,
        falta_trozo: s.entera < NO,
    }
}

/// **Pregunta y decide.**
pub fn recortar(cliente: &Cliente, foto: &Imagen) -> Result<(Recorte, Senales), Error> {
    let s = Senales::de(&cliente.preguntar(&peticion(foto))?)?;
    Ok((decidir(&s), s))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn c(top: f64, bottom: f64, left: f64, right: f64) -> Caja {
        Caja {
            top,
            bottom,
            left,
            right,
        }
    }

    #[test]
    fn la_tabla_del_recorte() {
        let caja = c(0.2, 0.8, 0.1, 0.9);
        let s = |hay, entera| Senales { hay, caja, entera };
        let Recorte::Hecho {
            caja: r,
            falta_trozo: false,
        } = decidir(&s(0.95, 0.9))
        else {
            panic!()
        };
        assert!(iou(&r, &c(0.18, 0.82, 0.08, 0.92)) > 0.999);
        assert!(matches!(
            decidir(&s(0.95, 0.1)),
            Recorte::Hecho {
                falta_trozo: true,
                ..
            }
        ));
        assert!(matches!(decidir(&s(0.5, 0.9)), Recorte::Sugerido(_)));
        assert_eq!(decidir(&s(0.1, 0.9)), Recorte::Nada);
        // Una caja diminuta no se recorta sola.
        let chica = Senales {
            hay: 0.9,
            caja: c(0.5, 0.6, 0.5, 0.6),
            entera: 0.9,
        };
        assert!(matches!(decidir(&chica), Recorte::Sugerido(_)));
    }

    #[test]
    fn el_margen_no_se_sale_y_el_iou() {
        assert_eq!(
            con_margen(c(0.01, 0.99, 0.0, 1.0), 0.02),
            c(0.0, 1.0, 0.0, 1.0)
        );
        let a = c(0.0, 0.5, 0.0, 1.0);
        assert!((iou(&a, &a) - 1.0).abs() < 1e-9);
        assert!((iou(&a, &c(0.25, 0.75, 0.0, 1.0)) - 1.0 / 3.0).abs() < 1e-9);
        assert_eq!(iou(&a, &c(0.6, 0.9, 0.0, 1.0)), 0.0);
    }

    #[test]
    fn la_peticion_lleva_la_foto_y_el_box() {
        let p = peticion(&Imagen {
            formato: "jpg".into(),
            bytes: vec![9; 10],
        });
        assert!(p.con_imagenes());
        let v: serde_json::Value = serde_json::from_slice(&p.cuerpo().unwrap()).unwrap();
        let r = v["state"]["foto"].as_str().unwrap();
        assert!(v["assets"].get(r).is_some());
        assert_eq!(v["questions"]["donde"]["type"], "box");
    }
}
