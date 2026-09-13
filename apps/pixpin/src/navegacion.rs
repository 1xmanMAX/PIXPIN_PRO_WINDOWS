//! Por donde mira el editor.
//!
//! La camara del usuario (`camara`) trabaja en pixeles LOGICOS, los de
//! Excalidraw: zoom 1 es un pixel CSS, que Windows multiplica por su escala.
//! Con la pantalla al 150 % y la camara en pixeles fisicos, el mismo
//! `strokeWidth` salia un tercio mas fino que en excalidraw.com (D127).
//!
//! Puro: sin ventana ni GPU, para probarlo todo sin escritorio.

use pixpin_motor2d::camara::Camara;

/// La escala del monitor como factor. Un monitor que dijera 0 (no pasa, pero
/// un DPI mal leido no puede dejar la camara con zoom cero y dividir por el)
/// cuenta como 100 %.
pub fn escala_de(escala_por_cien: u32) -> f32 {
    if escala_por_cien == 0 {
        1.0
    } else {
        escala_por_cien as f32 / 100.0
    }
}

/// La camara con la que se pinta y se traduce el raton: la del usuario con
/// su zoom multiplicado por la escala del monitor.
///
/// Es la UNICA que ve el pintado y `a_evento`. Los ficheros no cambian: el
/// mundo sigue en las mismas unidades, solo cambia cuantos pixeles fisicos
/// ocupa cada una.
pub fn vista_efectiva(camara: &Camara, escala_por_cien: u32) -> Camara {
    Camara {
        x: camara.x,
        y: camara.y,
        zoom: camara.zoom * escala_de(escala_por_cien),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::vector::Punto2;

    #[test]
    fn a_escala_cien_la_vista_es_la_camara_del_usuario() {
        let c = Camara {
            x: 12.0,
            y: -7.0,
            zoom: 2.5,
        };
        assert_eq!(vista_efectiva(&c, 100), c);
    }

    #[test]
    fn ida_y_vuelta_de_pantalla_a_mundo_con_escala_150_y_zoom_arbitrario() {
        let c = Camara {
            x: -340.25,
            y: 118.5,
            zoom: 0.37,
        };
        let v = vista_efectiva(&c, 150);
        for p in [
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(123.5, -44.25),
            Punto2::nuevo(-900.0, 2048.0),
        ] {
            let vuelta = v.a_mundo(v.a_pantalla(p));
            assert!(
                (vuelta.x - p.x).abs() < 1e-2 && (vuelta.y - p.y).abs() < 1e-2,
                "{p:?} volvio como {vuelta:?}"
            );
        }
    }

    #[test]
    fn un_grosor_de_uno_a_escala_150_ocupa_uno_y_medio_lo_de_escala_100() {
        let c = Camara::nueva();
        let ancho = |escala| {
            let v = vista_efectiva(&c, escala);
            v.a_pantalla(Punto2::nuevo(1.0, 0.0)).x - v.a_pantalla(Punto2::nuevo(0.0, 0.0)).x
        };
        assert!((ancho(100) - 1.0).abs() < 1e-6);
        assert!((ancho(150) - 1.5).abs() < 1e-6);
        assert!((ancho(150) / ancho(100) - 1.5).abs() < 1e-6);
    }

    #[test]
    fn una_escala_de_cero_no_deja_la_camara_sin_zoom() {
        // Caso negativo: con zoom 0, `a_mundo` dividiria por cero y el raton
        // caeria en el infinito.
        let v = vista_efectiva(&Camara::nueva(), 0);
        assert_eq!(v.zoom, 1.0);
    }
}
