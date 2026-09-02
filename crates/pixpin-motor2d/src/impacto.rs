//! Que elemento hay bajo el cursor.
//!
//! La regla que decide todo: **una figura sin relleno se toca por su borde, no
//! por su interior.** Un rectangulo vacio dibujado alrededor de un texto no
//! puede robar los clics de ese texto; si lo hiciera, encuadrar algo lo
//! volveria inalcanzable.
//!
//! La tolerancia existe porque nadie acierta un trazo de un pixel: se toca lo
//! que esta "cerca", y cerca significa la mitad del grosor mas un margen fijo
//! del tamaño de la punta de un dedo en raton.

use crate::elemento::{Elemento, Figura};
use crate::vector::{Punto2, distancia_a_segmento};

/// Margen extra de tolerancia, en pixeles del documento. Un trazo de 1 px se
/// sigue pudiendo tocar sin apuntar al pixel exacto.
pub const TOLERANCIA: f32 = 6.0;

/// Si el punto toca el elemento.
pub fn toca(e: &Elemento, p: Punto2) -> bool {
    if e.borrado {
        return false;
    }
    // El angulo se deshace sobre el punto, no sobre la figura: girar el punto
    // es una operacion; girar toda la geometria, cientos.
    let (x0, y0, x1, y1) = e.caja();
    let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let p = if e.angulo != 0.0 {
        p.girar(centro, -e.angulo)
    } else {
        p
    };

    let margen = e.grosor / 2.0 + TOLERANCIA;

    // Descarte rapido por caja: barato y evita el trabajo fino en la inmensa
    // mayoria de los elementos de una escena grande.
    if p.x < x0 - margen || p.x > x1 + margen || p.y < y0 - margen || p.y > y1 + margen {
        return false;
    }

    match &e.figura {
        Figura::Lapiz { puntos, .. }
        | Figura::Resaltador { puntos }
        | Figura::Linea { puntos }
        | Figura::Flecha { puntos, .. } => cerca_de_la_polilinea(puntos, p, margen),

        Figura::Rectangulo => {
            if e.tiene_relleno() {
                dentro_de_la_caja(p, e, margen)
            } else {
                cerca_del_borde_del_rectangulo(p, e, margen)
            }
        }

        // Lo que se ve del foco es el hueco: se agarra por dentro.
        Figura::Foco { .. } => dentro_de_la_caja(p, e, margen),

        Figura::Elipse => {
            let rx = (e.ancho / 2.0).max(0.001);
            let ry = (e.alto / 2.0).max(0.001);
            let cx = e.x + rx;
            let cy = e.y + ry;
            // Distancia normalizada al centro: 1 es justo el borde.
            let d = ((p.x - cx) / rx).powi(2) + ((p.y - cy) / ry).powi(2);
            if e.tiene_relleno() {
                d <= 1.0
            } else {
                // Sin relleno solo cuenta el anillo del borde. El margen se
                // normaliza con el radio menor, que es el caso mas estrecho.
                let holgura = margen / rx.min(ry);
                let dentro = (1.0 - holgura).max(0.0).powi(2);
                let fuera = (1.0 + holgura).powi(2);
                d >= dentro && d <= fuera
            }
        }

        // Texto e imagen son cajas solidas: su interior SI cuenta, porque es
        // donde esta el contenido.
        Figura::Texto { .. } | Figura::Imagen { .. } => dentro_de_la_caja(p, e, margen),
    }
}

fn dentro_de_la_caja(p: Punto2, e: &Elemento, margen: f32) -> bool {
    let (x0, y0, x1, y1) = e.caja();
    p.x >= x0 - margen && p.x <= x1 + margen && p.y >= y0 - margen && p.y <= y1 + margen
}

fn cerca_del_borde_del_rectangulo(p: Punto2, e: &Elemento, margen: f32) -> bool {
    let (x0, y0, x1, y1) = (e.x, e.y, e.x + e.ancho, e.y + e.alto);
    let esquinas = [
        Punto2::nuevo(x0, y0),
        Punto2::nuevo(x1, y0),
        Punto2::nuevo(x1, y1),
        Punto2::nuevo(x0, y1),
    ];
    (0..4).any(|i| distancia_a_segmento(p, esquinas[i], esquinas[(i + 1) % 4]) <= margen)
}

fn cerca_de_la_polilinea(puntos: &[Punto2], p: Punto2, margen: f32) -> bool {
    match puntos.len() {
        0 => false,
        // Un trazo de un punto es un circulo: se toca por su radio.
        1 => p.distancia(puntos[0]) <= margen,
        _ => puntos
            .windows(2)
            .any(|w| distancia_a_segmento(p, w[0], w[1]) <= margen),
    }
}

/// El elemento de mas arriba que toca el punto, o `None`.
///
/// Se recorre al reves porque el ultimo de la lista es el que esta encima: al
/// hacer clic donde se solapan dos, se selecciona el que se ve.
pub fn elemento_en(elementos: &[Elemento], p: Punto2) -> Option<u64> {
    elementos.iter().rev().find(|e| toca(e, p)).map(|e| e.id)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo};

    fn base() -> Elemento {
        Elemento {
            id: 1,
            figura: Figura::Rectangulo,
            x: 100.0,
            y: 100.0,
            ancho: 200.0,
            alto: 100.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
        }
    }

    #[test]
    fn una_linea_fina_se_toca_sin_apuntar_al_pixel_exacto() {
        let e = Elemento {
            figura: Figura::Linea {
                puntos: vec![Punto2::nuevo(0.0, 100.0), Punto2::nuevo(200.0, 100.0)],
            },
            grosor: 1.0,
            ..base()
        };
        assert!(toca(&e, Punto2::nuevo(100.0, 103.0)), "3 px deberia bastar");
        assert!(!toca(&e, Punto2::nuevo(100.0, 140.0)), "40 px es demasiado");
    }

    #[test]
    fn un_rectangulo_sin_relleno_se_toca_por_el_borde_y_no_por_dentro() {
        // La regla que sostiene todo: encuadrar algo no puede volverlo
        // inalcanzable.
        let e = base();
        assert!(toca(&e, Punto2::nuevo(100.0, 150.0)), "el borde izquierdo");
        assert!(toca(&e, Punto2::nuevo(200.0, 100.0)), "el borde superior");
        assert!(
            !toca(&e, Punto2::nuevo(200.0, 150.0)),
            "el centro de un rectangulo vacio NO se toca"
        );
    }

    #[test]
    fn un_rectangulo_relleno_si_se_toca_por_dentro() {
        // Caso complementario del anterior: con relleno, el interior es el
        // elemento.
        let e = Elemento {
            relleno: Some(ColorRgba::opaco(1.0, 1.0, 0.0)),
            ..base()
        };
        assert!(toca(&e, Punto2::nuevo(200.0, 150.0)));
    }

    #[test]
    fn una_elipse_sin_relleno_se_toca_por_el_anillo() {
        let e = Elemento {
            figura: Figura::Elipse,
            ..base()
        };
        // Extremo derecho del borde.
        assert!(toca(&e, Punto2::nuevo(300.0, 150.0)));
        // Centro: hueco.
        assert!(!toca(&e, Punto2::nuevo(200.0, 150.0)));
        // La esquina de la caja queda FUERA de la elipse: es el caso que
        // distingue una elipse de verdad de un rectangulo disfrazado.
        assert!(!toca(&e, Punto2::nuevo(100.0, 100.0)));
    }

    #[test]
    fn un_elemento_borrado_no_se_toca() {
        let e = Elemento {
            borrado: true,
            relleno: Some(ColorRgba::opaco(1.0, 0.0, 0.0)),
            ..base()
        };
        assert!(!toca(&e, Punto2::nuevo(200.0, 150.0)));
    }

    #[test]
    fn el_texto_y_la_imagen_se_tocan_por_dentro() {
        let texto = Elemento {
            figura: Figura::Texto {
                texto: "hola".into(),
                tam: 14.0,
                familia: "Segoe UI".into(),
            },
            ..base()
        };
        assert!(toca(&texto, Punto2::nuevo(200.0, 150.0)));
        let imagen = Elemento {
            figura: Figura::Imagen { id_objeto: 3 },
            ..base()
        };
        assert!(toca(&imagen, Punto2::nuevo(200.0, 150.0)));
    }

    #[test]
    fn gana_el_elemento_de_arriba() {
        // Dos rectangulos rellenos superpuestos: al hacer clic se selecciona
        // el que se VE, que es el ultimo de la lista.
        let abajo = Elemento {
            id: 1,
            relleno: Some(ColorRgba::opaco(1.0, 0.0, 0.0)),
            ..base()
        };
        let arriba = Elemento {
            id: 2,
            relleno: Some(ColorRgba::opaco(0.0, 1.0, 0.0)),
            ..base()
        };
        assert_eq!(
            elemento_en(&[abajo, arriba], Punto2::nuevo(200.0, 150.0)),
            Some(2)
        );
    }

    #[test]
    fn en_un_hueco_no_hay_ningun_elemento() {
        assert_eq!(elemento_en(&[base()], Punto2::nuevo(900.0, 900.0)), None);
    }

    #[test]
    fn un_elemento_girado_se_toca_donde_se_ve() {
        // Un rectangulo apaisado girado un cuarto de vuelta pasa a ser
        // vertical: un punto sobre su nuevo borde debe tocarlo, y el hueco de
        // donde estaba antes, no.
        let e = Elemento {
            angulo: std::f32::consts::FRAC_PI_2,
            relleno: Some(ColorRgba::opaco(0.0, 0.0, 1.0)),
            ..base()
        };
        // El centro sigue siendo el centro, gire lo que gire.
        assert!(toca(&e, Punto2::nuevo(200.0, 150.0)));
        // A 80 px del centro en vertical: fuera del original (alto 100, o sea
        // 50 a cada lado), dentro tras girar (ancho 200 = 100 a cada lado).
        assert!(
            toca(&e, Punto2::nuevo(200.0, 230.0)),
            "tras girar, el lado largo es el vertical"
        );
    }

    #[test]
    fn un_trazo_de_un_solo_punto_se_toca_por_su_radio() {
        let e = Elemento {
            figura: Figura::Lapiz {
                puntos: vec![Punto2::nuevo(50.0, 50.0)],
                presiones: vec![],
            },
            grosor: 10.0,
            ..base()
        };
        assert!(toca(&e, Punto2::nuevo(53.0, 53.0)));
        assert!(!toca(&e, Punto2::nuevo(150.0, 150.0)));
    }

    #[test]
    fn el_foco_se_agarra_por_dentro_del_hueco() {
        // Lo que se ve es el hueco: es lo que el usuario intenta mover.
        let e = Elemento {
            figura: Figura::Foco { elipse: false },
            ..base()
        };
        assert!(toca(&e, Punto2::nuevo(200.0, 150.0)));
        assert!(!toca(&e, Punto2::nuevo(500.0, 400.0)));
    }
}
