//! La aritmetica de la lupa: que region amplia, donde se coloca y como
//! escribe el color. Esta pieza es tambien el cuentagotas del catalogo
//! (S1-B3 la reutiliza tal cual): mismo codigo, dos funciones cubiertas.

use pixpin_geom::{Punto, Rect};

/// Separacion entre el cursor y la esquina de la lupa, en pixeles fisicos.
const MARGEN_CURSOR: i32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lupa {
    /// Aumento: 8 pixeles dibujados por cada pixel real.
    pub factor: u32,
    /// Lado del cuadrado de la lupa, en pixeles fisicos del monitor.
    pub diametro: u32,
}

impl Lupa {
    /// Lupa de la spec: 8x, 176 px logicos escalados al DPI del monitor.
    pub fn por_defecto(escala_por_cien: u32) -> Lupa {
        Lupa {
            factor: 8,
            diametro: 176 * escala_por_cien / 100,
        }
    }

    /// Region del monitor que la lupa amplia, centrada en el cursor y
    /// desplazada (no encogida) para no salirse del monitor: encogerla
    /// cambiaria el aumento en los bordes.
    pub fn region_fuente(&self, cursor: Punto, monitor: Rect) -> Rect {
        let lado = (self.diametro / self.factor) as i32;
        let x = (cursor.x - lado / 2).clamp(monitor.izquierda(), monitor.derecha() - lado);
        let y = (cursor.y - lado / 2).clamp(monitor.arriba(), monitor.abajo() - lado);
        Rect {
            x,
            y,
            ancho: lado as u32,
            alto: lado as u32,
        }
    }

    /// Esquina superior izquierda donde dibujar la lupa: en el cuadrante
    /// opuesto al borde mas cercano, para no tapar lo que se esta mirando.
    pub fn colocar(&self, cursor: Punto, monitor: Rect) -> Punto {
        let d = self.diametro as i32;
        let mut x = cursor.x + MARGEN_CURSOR;
        let mut y = cursor.y + MARGEN_CURSOR;
        if x + d > monitor.derecha() {
            x = cursor.x - MARGEN_CURSOR - d;
        }
        if y + d > monitor.abajo() {
            y = cursor.y - MARGEN_CURSOR - d;
        }
        Punto {
            x: x.clamp(monitor.izquierda(), monitor.derecha() - d),
            y: y.clamp(monitor.arriba(), monitor.abajo() - d),
        }
    }
}

/// Copia local del formato de `pixpin-store` para no depender hacia arriba:
/// `pixpin-ui` es L3 y `pixpin-store` L2, pero la conversion la decide quien
/// cablea (el ejecutable), no este crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatoColorLupa {
    Hex,
    Rgb,
    Hsl,
}

/// El color bajo el cursor, como texto listo para ensenar y copiar.
pub fn texto_color(formato: FormatoColorLupa, rgba: [u8; 4]) -> String {
    let [r, g, b, _] = rgba;
    match formato {
        FormatoColorLupa::Hex => format!("#{r:02X}{g:02X}{b:02X}"),
        FormatoColorLupa::Rgb => format!("rgb({r}, {g}, {b})"),
        FormatoColorLupa::Hsl => {
            // Conversion clasica en aritmetica entera de milesimas, para que
            // el resultado sea determinista y sin sorpresas de coma flotante.
            let (r, g, b) = (r as i32, g as i32, b as i32);
            let max = r.max(g).max(b);
            let min = r.min(g).min(b);
            let l_mil = (max + min) * 1000 / 510; // luminosidad en milesimas
            let delta = max - min;
            let (h, s_mil) = if delta == 0 {
                (0, 0)
            } else {
                let s_mil = if l_mil > 500 {
                    delta * 1000 / (510 - max - min)
                } else {
                    delta * 1000 / (max + min)
                };
                let h = if max == r {
                    (60 * (g - b) / delta).rem_euclid(360)
                } else if max == g {
                    60 * (b - r) / delta + 120
                } else {
                    60 * (r - g) / delta + 240
                };
                (h, s_mil)
            };
            format!("hsl({h}, {}%, {}%)", (s_mil + 5) / 10, (l_mil + 5) / 10)
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_geom::{Punto, Rect};

    fn monitor() -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        }
    }

    #[test]
    fn el_diametro_escala_con_el_dpi_del_monitor() {
        assert_eq!(Lupa::por_defecto(100).diametro, 176);
        assert_eq!(Lupa::por_defecto(150).diametro, 264);
        assert_eq!(Lupa::por_defecto(200).diametro, 352);
        assert_eq!(Lupa::por_defecto(100).factor, 8);
    }

    #[test]
    fn la_region_fuente_esta_centrada_y_mide_diametro_entre_factor() {
        let lupa = Lupa {
            factor: 8,
            diametro: 176,
        };
        let r = lupa.region_fuente(Punto { x: 500, y: 400 }, monitor());
        assert_eq!(r.ancho, 22); // 176 / 8
        assert_eq!(r.alto, 22);
        // Centrada: el cursor cae en el pixel central.
        assert_eq!(r.x, 500 - 11);
        assert_eq!(r.y, 400 - 11);
    }

    #[test]
    fn la_region_fuente_no_se_sale_del_monitor_en_las_esquinas() {
        // Caso negativo: sin el ajuste, en la esquina la region tendria
        // coordenadas negativas y el recorte de textura fallaria.
        let lupa = Lupa {
            factor: 8,
            diametro: 176,
        };
        let r = lupa.region_fuente(Punto { x: 2, y: 2 }, monitor());
        assert_eq!((r.x, r.y), (0, 0));
        assert_eq!((r.ancho, r.alto), (22, 22));
        let r2 = lupa.region_fuente(Punto { x: 1919, y: 1079 }, monitor());
        assert_eq!(r2.derecha(), 1920);
        assert_eq!(r2.abajo(), 1080);
    }

    #[test]
    fn la_lupa_huye_del_cursor_al_acercarse_al_borde() {
        let lupa = Lupa {
            factor: 8,
            diametro: 176,
        };
        // Lejos de los bordes: abajo a la derecha del cursor.
        let p = lupa.colocar(Punto { x: 500, y: 400 }, monitor());
        assert!(p.x > 500 && p.y > 400);
        // Cerca del borde derecho: se va a la izquierda.
        let p = lupa.colocar(Punto { x: 1900, y: 400 }, monitor());
        assert!(p.x + 176 <= 1920, "se saldria por la derecha");
        assert!(p.x < 1900);
        // Esquina inferior derecha: arriba a la izquierda, y entera dentro.
        let p = lupa.colocar(Punto { x: 1910, y: 1070 }, monitor());
        assert!(p.x + 176 <= 1920 && p.y + 176 <= 1080);
    }

    #[test]
    fn el_texto_de_color_sale_en_los_tres_formatos() {
        assert_eq!(
            texto_color(FormatoColorLupa::Hex, [255, 87, 51, 255]),
            "#FF5733"
        );
        assert_eq!(
            texto_color(FormatoColorLupa::Rgb, [255, 87, 51, 255]),
            "rgb(255, 87, 51)"
        );
        // HSL de rojo puro: matiz 0, saturacion 100%, luminosidad 50%.
        assert_eq!(
            texto_color(FormatoColorLupa::Hsl, [255, 0, 0, 255]),
            "hsl(0, 100%, 50%)"
        );
        // Gris puro: saturacion 0; una division por cero ingenua daria NaN.
        assert_eq!(
            texto_color(FormatoColorLupa::Hsl, [128, 128, 128, 255]),
            "hsl(0, 0%, 50%)"
        );
        // Verde puro: matiz 120.
        assert_eq!(
            texto_color(FormatoColorLupa::Hsl, [0, 255, 0, 255]),
            "hsl(120, 100%, 50%)"
        );
    }
}
