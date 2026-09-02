//! Geometria del pin flotante: esquinas proporcionales y recolocacion.
//!
//! D23: el pin se agarra desde cualquier punto y SOLO las esquinas
//! redimensionan, siempre en proporcion, ancladas a la opuesta. La
//! recolocacion restaura pines cuyo monitor desaparecio sin cambiarles el
//! tamano ni dejarlos fuera de pantalla.

use crate::punto::Punto;
use crate::rect::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Esquina {
    Noroeste,
    Noreste,
    Sureste,
    Suroeste,
}

impl Esquina {
    /// El punto que NO se mueve al redimensionar por esta esquina.
    fn ancla(self, r: Rect) -> Punto {
        match self {
            Esquina::Noroeste => Punto {
                x: r.derecha(),
                y: r.abajo(),
            },
            Esquina::Noreste => Punto {
                x: r.izquierda(),
                y: r.abajo(),
            },
            Esquina::Sureste => Punto {
                x: r.izquierda(),
                y: r.arriba(),
            },
            Esquina::Suroeste => Punto {
                x: r.derecha(),
                y: r.arriba(),
            },
        }
    }
}

/// Que esquina hay bajo el punto local, con zona cuadrada de `zona` px.
pub fn esquina_en(rect_local: Rect, p: Punto, zona: u32) -> Option<Esquina> {
    let z = zona as i32;
    let cerca_izq = p.x < rect_local.izquierda() + z;
    let cerca_der = p.x >= rect_local.derecha() - z;
    let cerca_arr = p.y < rect_local.arriba() + z;
    let cerca_aba = p.y >= rect_local.abajo() - z;
    match (cerca_izq, cerca_der, cerca_arr, cerca_aba) {
        (true, false, true, false) => Some(Esquina::Noroeste),
        (false, true, true, false) => Some(Esquina::Noreste),
        (false, true, false, true) => Some(Esquina::Sureste),
        (true, false, false, true) => Some(Esquina::Suroeste),
        _ => None,
    }
}

/// Redimension proporcional: el rect nuevo tiene la proporcion de
/// `original`, la esquina opuesta clavada, y su tamano lo dicta la
/// distancia del cursor al ancla (domina el eje mayor relativo).
pub fn redimension_proporcional(
    original: Rect,
    esquina: Esquina,
    cursor: Punto,
    minimo: u32,
) -> Rect {
    if original.esta_vacio() {
        return original;
    }
    let ancla = esquina.ancla(original);
    let dx = (cursor.x - ancla.x).abs().max(1) as f64;
    let dy = (cursor.y - ancla.y).abs().max(1) as f64;
    let proporcion = original.ancho as f64 / original.alto as f64;

    // Domina el eje que pide mas tamano relativo: asi el rect siempre
    // "alcanza" al cursor por un lado y lo recorta por el otro.
    let (ancho, alto) = if dx / proporcion >= dy {
        (dx, dx / proporcion)
    } else {
        (dy * proporcion, dy)
    };
    let minimo_f = minimo.max(1) as f64;
    let (ancho, alto) = if ancho < minimo_f || alto < minimo_f {
        if proporcion >= 1.0 {
            (minimo_f * proporcion, minimo_f)
        } else {
            (minimo_f, minimo_f / proporcion)
        }
    } else {
        (ancho, alto)
    };
    let (ancho, alto) = (ancho.round() as u32, alto.round() as u32);

    // Reconstruir desde el ancla hacia el lado de la esquina activa.
    let (x, y) = match esquina {
        Esquina::Sureste => (ancla.x, ancla.y),
        Esquina::Noroeste => (ancla.x - ancho as i32, ancla.y - alto as i32),
        Esquina::Noreste => (ancla.x, ancla.y - alto as i32),
        Esquina::Suroeste => (ancla.x - ancho as i32, ancla.y),
    };
    Rect { x, y, ancho, alto }
}

/// Desliza el rect al interior del area sin cambiar su tamano. Un rect mas
/// grande que el area queda alineado a la esquina superior izquierda.
pub fn recolocar_en_area(rect: Rect, area_trabajo: Rect) -> Rect {
    let max_x = (area_trabajo.derecha() - rect.ancho as i32).max(area_trabajo.izquierda());
    let max_y = (area_trabajo.abajo() - rect.alto as i32).max(area_trabajo.arriba());
    Rect {
        x: rect.x.clamp(area_trabajo.izquierda(), max_x),
        y: rect.y.clamp(area_trabajo.arriba(), max_y),
        ancho: rect.ancho,
        alto: rect.alto,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::punto::Punto;
    use crate::rect::Rect;

    #[test]
    fn las_cuatro_esquinas_se_detectan_y_el_centro_no() {
        let r = Rect {
            x: 0,
            y: 0,
            ancho: 400,
            alto: 300,
        };
        assert_eq!(
            esquina_en(r, Punto { x: 5, y: 5 }, 12),
            Some(Esquina::Noroeste)
        );
        assert_eq!(
            esquina_en(r, Punto { x: 395, y: 5 }, 12),
            Some(Esquina::Noreste)
        );
        assert_eq!(
            esquina_en(r, Punto { x: 395, y: 295 }, 12),
            Some(Esquina::Sureste)
        );
        assert_eq!(
            esquina_en(r, Punto { x: 5, y: 295 }, 12),
            Some(Esquina::Suroeste)
        );
        // Caso negativo: el centro y los BORDES no son esquinas — en el pin,
        // el borde mueve como cualquier otro punto (D23: cero cromo).
        assert_eq!(esquina_en(r, Punto { x: 200, y: 150 }, 12), None);
        assert_eq!(esquina_en(r, Punto { x: 200, y: 5 }, 12), None);
        assert_eq!(esquina_en(r, Punto { x: 13, y: 13 }, 12), None);
    }

    #[test]
    fn redimensionar_conserva_la_proporcion_y_ancla_la_opuesta() {
        // 400x300 = 4:3. Arrastrar la sureste a un cursor "ancho" debe dar
        // un rect 4:3 con la noroeste clavada en (100, 100).
        let r = Rect {
            x: 100,
            y: 100,
            ancho: 400,
            alto: 300,
        };
        let nuevo = redimension_proporcional(r, Esquina::Sureste, Punto { x: 900, y: 400 }, 48);
        assert_eq!(
            (nuevo.x, nuevo.y),
            (100, 100),
            "la esquina opuesta no se mueve"
        );
        let prop_original = 400.0 / 300.0;
        let prop_nueva = nuevo.ancho as f64 / nuevo.alto as f64;
        assert!(
            (prop_nueva - prop_original).abs() < 0.02,
            "proporcion {prop_nueva} != {prop_original}"
        );
        assert!(nuevo.ancho > 400, "arrastrar hacia fuera agranda");
    }

    #[test]
    fn redimensionar_por_la_noroeste_ancla_la_sureste() {
        let r = Rect {
            x: 100,
            y: 100,
            ancho: 400,
            alto: 300,
        };
        let nuevo = redimension_proporcional(r, Esquina::Noroeste, Punto { x: 300, y: 250 }, 48);
        assert_eq!(
            (nuevo.derecha(), nuevo.abajo()),
            (500, 400),
            "la sureste queda clavada"
        );
        assert!(nuevo.ancho < 400, "arrastrar hacia dentro encoge");
    }

    #[test]
    fn el_minimo_impide_desaparecer() {
        // Caso negativo: cruzar el ancla no puede dar 0 ni voltear.
        let r = Rect {
            x: 100,
            y: 100,
            ancho: 400,
            alto: 300,
        };
        let nuevo = redimension_proporcional(r, Esquina::Sureste, Punto { x: 90, y: 90 }, 48);
        assert!(nuevo.ancho >= 48 && nuevo.alto >= 48, "quedo {nuevo:?}");
        assert_eq!((nuevo.x, nuevo.y), (100, 100));
    }

    #[test]
    fn recolocar_desliza_sin_cambiar_tamano() {
        let area = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1040,
        };
        let fuera = Rect {
            x: 1800,
            y: -50,
            ancho: 300,
            alto: 200,
        };
        let dentro = recolocar_en_area(fuera, area);
        assert_eq!(
            (dentro.ancho, dentro.alto),
            (300, 200),
            "el tamano es sagrado"
        );
        assert_eq!(
            area.interseccion(dentro),
            Some(dentro),
            "queda entero dentro"
        );
    }

    #[test]
    fn recolocar_un_gigante_lo_alinea_arriba_izquierda() {
        // Caso negativo del clamp: un pin mas grande que el area no puede
        // hacer entrar en panico a un clamp con min > max.
        let area = Rect {
            x: 0,
            y: 0,
            ancho: 800,
            alto: 600,
        };
        let gigante = Rect {
            x: 500,
            y: 500,
            ancho: 2000,
            alto: 1500,
        };
        let puesto = recolocar_en_area(gigante, area);
        assert_eq!((puesto.x, puesto.y), (0, 0));
        assert_eq!((puesto.ancho, puesto.alto), (2000, 1500));
    }
}
