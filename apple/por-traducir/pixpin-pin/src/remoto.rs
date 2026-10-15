//! Manejar a distancia la zona de un pin en vivo: las cuentas, sin Windows.
//!
//! El usuario lo pidio asi: «que pueda hacer clic en algun elemento del pin
//! en vivo y que se mande el clic a la zona exacta de donde se toma el pin,
//! de forma que pueda manejar lo que se ve a la distancia».
//!
//! Aqui solo vive lo que se puede equivocar en silencio: llevar un punto del
//! pin —agrandado, encogido, con el zoom de `Ctrl + rueda` dentro— al pixel
//! de la zona que se esta viendo ahi, y decidir si una pulsacion fue un clic
//! o el principio de un arrastre. El clic de verdad lo sintetiza
//! `pixpin-shell::entrada`, y lo encarga el ejecutable, que es quien conoce
//! la zona.

/// Cuanto puede moverse el cursor entre pulsar y soltar, en pixeles fisicos,
/// para que siga siendo un clic. Por encima es un arrastre, que tambien se
/// reenvia a la zona pero como pulsar-mover-soltar. (Mover el PIN, con el
/// modo encendido, es `Ctrl + arrastrar`: lo pidio el usuario asi.)
pub const HOLGURA_CLIC: i32 = 5;

/// Si entre `pulsado` y `soltado` (pantalla) hubo un clic y no un arrastre.
pub fn fue_clic(pulsado: (i32, i32), soltado: (i32, i32)) -> bool {
    (soltado.0 - pulsado.0).abs() <= HOLGURA_CLIC && (soltado.1 - pulsado.1).abs() <= HOLGURA_CLIC
}

/// La vista interior del pin: el zoom de `Ctrl + rueda` y su desplazamiento.
/// Lo que se ve en `v` del contenido es el punto `(v - d) / escala`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vista {
    pub escala: f32,
    pub dx: f32,
    pub dy: f32,
}

impl Vista {
    pub const NEUTRA: Vista = Vista {
        escala: 1.0,
        dx: 0.0,
        dy: 0.0,
    };
}

/// El pixel de la ZONA que se ve en `p`.
///
/// - `p`: el punto dentro del contenido del pin (margen de sombra ya
///   descontado), en pixeles de pantalla.
/// - `contenido`: ancho y alto con que se ve el contenido ahora mismo.
/// - `zona`: ancho y alto de la zona capturada, que es el tamano nativo.
///
/// `None` si el punto cae fuera de la zona: un clic en la sombra del pin o
/// con medidas a cero no se reenvia a ningun sitio.
pub fn punto_en_la_zona(
    p: (f32, f32),
    contenido: (u32, u32),
    vista: Vista,
    zona: (u32, u32),
) -> Option<(i32, i32)> {
    if contenido.0 == 0 || contenido.1 == 0 || zona.0 == 0 || zona.1 == 0 {
        return None;
    }
    let escala = if vista.escala > 0.0 {
        vista.escala
    } else {
        1.0
    };
    // Primero se deshace la vista interior, luego el tamano del pin.
    let x = (p.0 - vista.dx) / escala * zona.0 as f32 / contenido.0 as f32;
    let y = (p.1 - vista.dy) / escala * zona.1 as f32 / contenido.1 as f32;
    // `floor` y no `round`: el pixel 0 ocupa de 0.0 a 0.999, y redondear
    // mandaria la mitad derecha de cada pixel al vecino.
    let (x, y) = (x.floor() as i32, y.floor() as i32);
    (x >= 0 && y >= 0 && x < zona.0 as i32 && y < zona.1 as i32).then_some((x, y))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn al_cien_por_cien_el_punto_del_pin_es_el_de_la_zona() {
        assert_eq!(
            punto_en_la_zona((120.0, 45.0), (400, 300), Vista::NEUTRA, (400, 300)),
            Some((120, 45))
        );
    }

    #[test]
    fn con_el_pin_al_doble_el_clic_cae_en_la_mitad() {
        // El pin agrandado con la rueda: 800x600 en pantalla, zona 400x300.
        assert_eq!(
            punto_en_la_zona((600.0, 300.0), (800, 600), Vista::NEUTRA, (400, 300)),
            Some((300, 150))
        );
    }

    #[test]
    fn con_el_pin_encogido_cada_pixel_suyo_son_varios_de_la_zona() {
        assert_eq!(
            punto_en_la_zona((100.0, 50.0), (200, 150), Vista::NEUTRA, (400, 300)),
            Some((200, 100))
        );
    }

    #[test]
    fn con_el_zoom_interior_se_deshace_la_vista_antes_que_el_tamano() {
        // Vista x2 anclada en el centro de un pin de 400x300: dx = -200,
        // dy = -150. El centro de la ventana sigue viendo el centro de la zona.
        let vista = Vista {
            escala: 2.0,
            dx: -200.0,
            dy: -150.0,
        };
        assert_eq!(
            punto_en_la_zona((200.0, 150.0), (400, 300), vista, (400, 300)),
            Some((200, 150))
        );
        // Y la esquina de arriba a la izquierda ve el punto (100, 75).
        assert_eq!(
            punto_en_la_zona((0.0, 0.0), (400, 300), vista, (400, 300)),
            Some((100, 75))
        );
    }

    #[test]
    fn un_punto_fuera_del_contenido_no_se_reenvia() {
        // Caso negativo: la sombra del pin y el borde de mas alla.
        let m = (400, 300);
        assert_eq!(punto_en_la_zona((-1.0, 10.0), m, Vista::NEUTRA, m), None);
        assert_eq!(punto_en_la_zona((10.0, 300.0), m, Vista::NEUTRA, m), None);
        assert_eq!(punto_en_la_zona((400.0, 10.0), m, Vista::NEUTRA, m), None);
        // El ultimo pixel de dentro si.
        assert_eq!(
            punto_en_la_zona((399.9, 299.9), m, Vista::NEUTRA, m),
            Some((399, 299))
        );
    }

    #[test]
    fn con_medidas_a_cero_no_hay_division_ni_clic() {
        assert_eq!(
            punto_en_la_zona((1.0, 1.0), (0, 300), Vista::NEUTRA, (400, 300)),
            None
        );
        assert_eq!(
            punto_en_la_zona((1.0, 1.0), (400, 300), Vista::NEUTRA, (400, 0)),
            None
        );
        // Una vista con escala cero (no deberia existir) se toma por neutra.
        let rota = Vista {
            escala: 0.0,
            dx: 0.0,
            dy: 0.0,
        };
        assert_eq!(
            punto_en_la_zona((10.0, 10.0), (400, 300), rota, (400, 300)),
            Some((10, 10))
        );
    }

    #[test]
    fn un_clic_tiembla_y_un_arrastre_no_es_un_clic() {
        assert!(fue_clic((500, 500), (500, 500)));
        assert!(fue_clic((500, 500), (504, 496)));
        // Caso negativo: mover el pin no puede acabar en un clic en la zona.
        assert!(!fue_clic((500, 500), (506, 500)));
        assert!(!fue_clic((500, 500), (500, 380)));
    }
}
