//! Lo que el ejecutable pone entre la captura y el pin en vivo.
//!
//! `pixpin-pin` y `pixpin-capture` son la misma capa y no se ven: el pin
//! pide una `FuenteViva` y la captura ofrece un `RecorteVivo`. El adaptador
//! vive aqui, que ve a los dos. Tambien la decision de DONDE nace el pin,
//! que es pura y se prueba sin pantalla.

use std::cell::RefCell;
use std::rc::Rc;

use pixpin_capture::RecorteVivo;
use pixpin_geom::Rect;
use pixpin_pin::FuenteViva;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;

/// La fuente la comparten el pin, que pide fotogramas, y el gestor, que baja
/// el ultimo a la CPU para copiarlo o congelarlo. Todo en el hilo de
/// interfaz y nunca a la vez: el prestamo no se cruza.
pub struct FuenteCompartida(pub Rc<RefCell<RecorteVivo>>);

impl FuenteViva for FuenteCompartida {
    fn tick(&mut self) -> Option<ID3D11Texture2D> {
        self.0.borrow_mut().tick().cloned()
    }
}

/// Separacion entre la zona y el pin, en pixeles fisicos: lo justo para que
/// la sombra del pin no pise el borde de lo que se esta mirando.
pub const HUECO: i32 = 32;

/// Donde nace el pin en vivo: AL LADO de su zona, no encima.
///
/// Encima taparia justo lo que se quiere seguir usando —el pin recoge los
/// clics—, y la gracia del pin en vivo es trabajar en la zona y verlo en
/// otro sitio. Se prueba a la derecha, a la izquierda, debajo y encima,
/// dentro del area de trabajo del monitor; si no cabe en ninguno, encima de
/// la zona, desde donde el usuario lo arrastra.
pub fn sitio_junto_a(zona: Rect, trabajo: Rect) -> Rect {
    let (w, h) = (zona.ancho as i32, zona.alto as i32);
    let candidatos = [
        (zona.x + w + HUECO, zona.y),
        (zona.x - w - HUECO, zona.y),
        (zona.x, zona.y + h + HUECO),
        (zona.x, zona.y - h - HUECO),
    ];
    for (x, y) in candidatos {
        // Se arrima al borde por el eje que NO separa de la zona: un pin a
        // la derecha de una zona pegada abajo sube lo justo para caber.
        let r = if x == zona.x {
            Rect {
                x: x.clamp(
                    trabajo.x,
                    (trabajo.x + trabajo.ancho as i32 - w).max(trabajo.x),
                ),
                y,
                ancho: zona.ancho,
                alto: zona.alto,
            }
        } else {
            Rect {
                x,
                y: y.clamp(
                    trabajo.y,
                    (trabajo.y + trabajo.alto as i32 - h).max(trabajo.y),
                ),
                ancho: zona.ancho,
                alto: zona.alto,
            }
        };
        if trabajo.interseccion(r) == Some(r) && r.interseccion(zona).is_none() {
            return r;
        }
    }
    zona
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn trabajo() -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1040,
        }
    }

    #[test]
    fn con_sitio_nace_a_la_derecha_de_la_zona_a_la_misma_altura() {
        let zona = Rect {
            x: 100,
            y: 200,
            ancho: 400,
            alto: 300,
        };
        let r = sitio_junto_a(zona, trabajo());
        assert_eq!((r.x, r.y), (100 + 400 + HUECO, 200));
        assert_eq!((r.ancho, r.alto), (400, 300), "1:1, del tamano de la zona");
    }

    #[test]
    fn una_zona_pegada_a_la_derecha_lo_pone_a_la_izquierda() {
        let zona = Rect {
            x: 1500,
            y: 200,
            ancho: 400,
            alto: 300,
        };
        let r = sitio_junto_a(zona, trabajo());
        assert_eq!(r.x, 1500 - 400 - HUECO);
    }

    #[test]
    fn nunca_nace_encima_de_su_zona_si_hay_otro_sitio() {
        // Zona ancha arriba del todo: ni a la derecha ni a la izquierda cabe.
        let zona = Rect {
            x: 200,
            y: 0,
            ancho: 1500,
            alto: 300,
        };
        let r = sitio_junto_a(zona, trabajo());
        assert!(r.interseccion(zona).is_none(), "{r:?} pisa {zona:?}");
        assert_eq!(r.y, 300 + HUECO, "debajo");
    }

    #[test]
    fn cerca_del_borde_de_abajo_se_arrima_sin_salirse() {
        let zona = Rect {
            x: 100,
            y: 900,
            ancho: 300,
            alto: 300,
        };
        let r = sitio_junto_a(zona, trabajo());
        assert_eq!(r.x, 100 + 300 + HUECO);
        assert_eq!(r.y, 1040 - 300, "subido lo justo para caber");
        assert_eq!(trabajo().interseccion(r), Some(r));
    }

    #[test]
    fn una_zona_que_llena_la_pantalla_se_queda_donde_esta() {
        // Caso negativo: no hay hueco en ningun lado.
        let zona = trabajo();
        assert_eq!(sitio_junto_a(zona, trabajo()), zona);
    }
}
