//! **Las marcas sobre el texto reconocido de un pin** (8-oct-2026): resaltar,
//! raya ondulada, subrayar, tachar y tapar lo que se selecciono con el raton.
//!
//! Cada marca es dibujo normal del pin (`.pixpin2d`), no pixeles pintados en
//! la foto: se ve en el lienzo, viaja al copiar o arrastrar el pin (que
//! hornea lo anotado) y se puede borrar. Lo unico que la distingue de un
//! rectangulo o una linea dibujados a mano es su grupo, [`GRUPO`]: asi el
//! circulo tachado de la barra quita las marcas y no lo dibujado.
//!
//! Todo en pixeles de la imagen original, que es el sistema del dibujo del
//! pin y el de las cajas del texto reconocido.

use pixpin_geom::Rect;
use pixpin_motor2d::{ColorRgba, Elemento, Figura, Punto2};
use pixpin_pin::Marca;

/// Lo que llevan delante los grupos de las marcas: `marca-texto-<n>`.
pub const GRUPO: &str = "marca-texto";

/// Cuanto se agranda el resaltado alrededor de las letras: las cajas del
/// reconocimiento van justas y un fondo ceñido parece cortado.
const AIRE: f32 = 0.12;

/// Opacidad del resaltado: deja leer lo de debajo.
const OPACIDAD_RESALTADO: f32 = 0.40;

/// Los elementos de una marca sobre `cajas` (una por renglon), del color
/// `rgb`, todos en el grupo `grupo`.
pub fn elementos(marca: Marca, (r, g, b): (u8, u8, u8), cajas: &[Rect], grupo: &str) -> Vec<Elemento> {
    let color = ColorRgba::opaco(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let base = Elemento {
        trazo: color,
        rugosidad: 0.0,
        grupos: vec![grupo.to_string()],
        ..Default::default()
    };
    cajas
        .iter()
        .filter(|c| c.ancho > 0 && c.alto > 0)
        .map(|c| {
            let (x, y, w, h) = (c.x as f32, c.y as f32, c.ancho as f32, c.alto as f32);
            let grosor = (h * 0.09).max(1.5);
            match marca {
                Marca::Resaltar | Marca::Tapar => {
                    let aire = h * AIRE;
                    Elemento {
                        figura: Figura::Rectangulo,
                        x: x - aire,
                        y: y - aire,
                        ancho: w + 2.0 * aire,
                        alto: h + 2.0 * aire,
                        relleno: Some(color),
                        estilo_relleno: pixpin_motor2d::relleno::EstiloRelleno::Solido,
                        // Sin borde: un contorno encima del relleno translucido
                        // saldria mas oscuro que el resto.
                        trazo: ColorRgba { a: 0.0, ..color },
                        grosor: 1.0,
                        opacidad: if marca == Marca::Tapar {
                            1.0
                        } else {
                            OPACIDAD_RESALTADO
                        },
                        ..base.clone()
                    }
                }
                Marca::Subrayar | Marca::Tachar | Marca::Ondulada => {
                    let altura = match marca {
                        Marca::Tachar => y + h * 0.55,
                        _ => y + h + grosor,
                    };
                    let puntos: Vec<Punto2> = if marca == Marca::Ondulada {
                        let paso = (h * 0.09).max(1.5);
                        pixpin_pin::barra_marcas::onda(x, x + w, altura, paso, paso)
                            .into_iter()
                            .map(|(px, py)| Punto2::nuevo(px, py))
                            .collect()
                    } else {
                        vec![Punto2::nuevo(x, altura), Punto2::nuevo(x + w, altura)]
                    };
                    let (alto_min, alto_max) = puntos
                        .iter()
                        .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
                    Elemento {
                        figura: Figura::Linea { puntos },
                        x,
                        y: alto_min,
                        ancho: w,
                        alto: alto_max - alto_min,
                        grosor: if marca == Marca::Ondulada {
                            grosor * 0.8
                        } else {
                            grosor
                        },
                        ..base.clone()
                    }
                }
            }
        })
        .collect()
}

/// Si `e` es una marca de texto.
pub fn es_marca(e: &Elemento) -> bool {
    e.grupos.iter().any(|g| g.starts_with(GRUPO))
}

/// Si la marca `e` cae sobre alguna de `cajas`.
pub fn toca(e: &Elemento, cajas: &[Rect]) -> bool {
    let (x0, y0, x1, y1) = e.caja();
    cajas.iter().any(|c| {
        let (cx0, cy0) = (c.x as f32, c.y as f32);
        let (cx1, cy1) = (cx0 + c.ancho as f32, cy0 + c.alto as f32);
        x0 < cx1 && cx0 < x1 && y0 < cy1 && cy0 < y1
    })
}

/// **Lo que la lupa del pin tiene que volver a tapar**: los mosaicos, como
/// siempre, y ademas lo cubierto con «Tapar». La lupa amplia la foto SIN lo
/// dibujado, y sin esto bastaba pasarla por encima para leer lo tapado.
pub fn tapadas(elementos: &[Elemento]) -> Vec<(f32, f32, f32, f32)> {
    let mut v = pixpin_motor2d::mosaico::cajas_tapadas(elementos);
    v.extend(
        elementos
            .iter()
            .filter(|e| {
                !e.borrado
                    && es_marca(e)
                    && matches!(e.figura, Figura::Rectangulo)
                    && e.opacidad >= 1.0
            })
            .map(pixpin_motor2d::mosaico::caja_girada),
    );
    v
}

/// Un paso de las marcas, para `Ctrl+Z`.
#[derive(Debug, Clone)]
pub enum Paso {
    /// Se anadieron estos elementos (sus ids en el dibujo).
    Anadidos(Vec<u64>),
    /// Se quitaron estos, tal como estaban.
    Quitados(Vec<Elemento>),
}

/// Lo que hizo un cambio en el dibujo: nada (y no se guarda), o algo, con
/// el paso que lo deshace si se apila.
#[derive(Debug)]
pub enum Cambio {
    Nada,
    Hecho(Option<Paso>),
}

/// Cuantos pasos se recuerdan por pin.
pub const PASOS: usize = 50;

#[cfg(test)]
mod pruebas {
    use super::*;

    fn renglon(y: i32) -> Rect {
        Rect {
            x: 10,
            y,
            ancho: 200,
            alto: 20,
        }
    }

    #[test]
    fn una_marca_por_renglon_con_su_grupo() {
        let cajas = [renglon(10), renglon(40)];
        for m in [
            Marca::Resaltar,
            Marca::Ondulada,
            Marca::Subrayar,
            Marca::Tachar,
            Marca::Tapar,
        ] {
            let v = elementos(m, (255, 200, 0), &cajas, "marca-texto-1");
            assert_eq!(v.len(), 2, "{m:?}");
            assert!(v.iter().all(es_marca));
        }
        // Una caja vacia no deja marca.
        let vacia = Rect {
            ancho: 0,
            ..renglon(0)
        };
        assert!(elementos(Marca::Tachar, (0, 0, 0), &[vacia], "marca-texto-1").is_empty());
    }

    #[test]
    fn resaltar_deja_leer_y_tapar_no() {
        let r = &elementos(Marca::Resaltar, (255, 200, 0), &[renglon(10)], "g")[0];
        let t = &elementos(Marca::Tapar, (255, 200, 0), &[renglon(10)], "g")[0];
        assert!(r.opacidad < 0.6);
        assert_eq!(t.opacidad, 1.0);
        // Los dos cubren las letras enteras, con un poco de aire.
        for e in [r, t] {
            assert!(e.x < 10.0 && e.y < 10.0);
            assert!(e.x + e.ancho > 210.0 && e.y + e.alto > 30.0);
            assert!(e.relleno.is_some());
            assert_eq!(e.trazo.a, 0.0);
        }
    }

    #[test]
    fn subrayar_va_debajo_tachar_por_el_medio_y_la_onda_sube_y_baja() {
        let caja = renglon(10);
        let y_de = |m| match &elementos(m, (0, 0, 0), &[caja], "g")[0].figura {
            Figura::Linea { puntos } => puntos.iter().map(|p| p.y).collect::<Vec<_>>(),
            otra => panic!("{otra:?}"),
        };
        let sub = y_de(Marca::Subrayar);
        let tac = y_de(Marca::Tachar);
        assert!(sub.iter().all(|y| *y > 30.0), "debajo de las letras");
        assert!(tac.iter().all(|y| *y > 15.0 && *y < 25.0), "por el medio");
        let onda = y_de(Marca::Ondulada);
        let (min, max) = onda
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), y| (a.min(*y), b.max(*y)));
        assert!(max - min > 2.0 && min > 25.0);
    }

    #[test]
    fn quitar_solo_toca_las_marcas_que_caen_en_la_seleccion() {
        let v = elementos(Marca::Resaltar, (0, 0, 0), &[renglon(10), renglon(100)], "g");
        assert!(toca(&v[0], &[renglon(12)]));
        assert!(!toca(&v[1], &[renglon(12)]));
        // Un rectangulo dibujado a mano no es una marca.
        let a_mano = Elemento::default();
        assert!(!es_marca(&a_mano));
    }

    #[test]
    fn la_lupa_vuelve_a_tapar_lo_tapado_pero_no_lo_resaltado() {
        let mut v = elementos(Marca::Tapar, (0, 0, 0), &[renglon(10)], "marca-texto-1");
        v.extend(elementos(Marca::Resaltar, (0, 0, 0), &[renglon(100)], "marca-texto-1"));
        // Caso negativo: un tapado a mano (sin grupo de marca) no es asunto
        // de este modulo.
        assert!(tapadas(&elementos(Marca::Tapar, (0, 0, 0), &[renglon(10)], "otro")).is_empty());
        let t = tapadas(&v);
        assert_eq!(t.len(), 1);
        assert!(t[0].1 < 10.0 && t[0].3 > 30.0);
    }
}
