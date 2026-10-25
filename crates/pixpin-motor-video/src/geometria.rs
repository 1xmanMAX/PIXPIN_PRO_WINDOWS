//! Las medidas: a que tamano sale el video, y donde cae la pantalla dentro
//! de el cuando no tiene su misma forma (las bandas negras).
//!
//! El tamano del video se fija al empezar y **no cambia** aunque cambie la
//! resolucion o se grabe una ventana que se estira: un MP4 con el tamano
//! cambiando a mitad no lo reproduce bien casi nadie. Lo que cambia es el
//! rectangulo donde el procesador de video pinta la imagen ([`encaje`]).
//!
//! Todo va en pixeles de verdad (los de la textura de la captura), asi que
//! el escalado de la pantalla (DPI) no entra en ninguna cuenta.

/// El tope duro de «la pantalla tal cual»: 4K. Por encima ningun
/// codificador de la grafica de un equipo normal da abasto, y H.264 tampoco
/// lo admite en los niveles que lee un movil.
pub const TOPE_NATIVO: (u32, u32) = (3840, 2160);

/// Un lado par y de por lo menos dos: lo unico que admite el color 4:2:0
/// (NV12, H.264), que va de dos en dos pixeles.
pub fn par(v: u32) -> u32 {
    (v & !1).max(2)
}

/// **El tamano del video** para una fuente de `ancho`x`alto` sin pasar de
/// `tope` (o de [`TOPE_NATIVO`] si no hay): la misma forma, nunca mas
/// grande que la fuente y con los dos lados pares.
pub fn medida_de_salida(ancho: u32, alto: u32, tope: Option<(u32, u32)>) -> (u32, u32) {
    let (tw, th) = tope.unwrap_or(TOPE_NATIVO);
    pixpin_record::escala::medida_de_salida(ancho, alto, tw, th)
}

/// Un rectangulo en pixeles del video.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub ancho: u32,
    pub alto: u32,
}

/// **Donde cae una imagen de `ancho`x`alto` en un video de `sw`x`sh`**: lo
/// mas grande que quepa sin deformarla, centrada, con bandas negras donde
/// sobre. Esquina y lados pares, para que el color (que va de dos en dos)
/// no se corra medio pixel en el borde.
pub fn encaje(ancho: u32, alto: u32, sw: u32, sh: u32) -> Rect {
    let (ancho, alto) = (ancho.max(1), alto.max(1));
    let (sw, sh) = (par(sw), par(sh));
    // La escala como fraccion exacta: la menor de las dos.
    let (num, den) = if u64::from(sw) * u64::from(alto) <= u64::from(sh) * u64::from(ancho) {
        (u64::from(sw), u64::from(ancho))
    } else {
        (u64::from(sh), u64::from(alto))
    };
    let w = ((u64::from(ancho) * num / den) as u32).min(sw);
    let h = ((u64::from(alto) * num / den) as u32).min(sh);
    let (w, h) = (par(w).min(sw), par(h).min(sh));
    Rect {
        x: ((sw - w) / 2) & !1,
        y: ((sh - h) / 2) & !1,
        ancho: w,
        alto: h,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn par_redondea_abajo_y_nunca_da_cero() {
        assert_eq!(par(1921), 1920);
        assert_eq!(par(1920), 1920);
        // Caso negativo: lo diminuto no se queda en cero ni en uno.
        assert_eq!(par(1), 2);
        assert_eq!(par(0), 2);
    }

    #[test]
    fn la_medida_nativa_tiene_su_tope() {
        assert_eq!(medida_de_salida(2560, 1440, None), (2560, 1440));
        assert_eq!(medida_de_salida(7680, 4320, None), (3840, 2160));
        // Impares: pares.
        assert_eq!(medida_de_salida(1365, 767, None), (1364, 766));
    }

    #[test]
    fn la_misma_forma_ocupa_todo() {
        assert_eq!(
            encaje(1920, 1080, 1280, 720),
            Rect {
                x: 0,
                y: 0,
                ancho: 1280,
                alto: 720
            }
        );
    }

    #[test]
    fn una_pantalla_cuadrada_lleva_bandas_a_los_lados() {
        // 1280x1024 dentro de 1920x1080: manda el alto.
        let r = encaje(1280, 1024, 1920, 1080);
        assert_eq!(r.alto, 1080);
        assert_eq!(r.ancho, 1350);
        assert_eq!(r.y, 0);
        assert_eq!(r.x, 284);
        // Centrada: las dos bandas iguales (o a un par de pixeles).
        assert!((r.x as i64 - (1920 - r.x - r.ancho) as i64).abs() <= 2);
    }

    #[test]
    fn una_ventana_alargada_lleva_bandas_arriba_y_abajo() {
        let r = encaje(3000, 500, 1920, 1080);
        assert_eq!(r.ancho, 1920);
        assert_eq!(r.alto, 320);
        assert_eq!(r.x, 0);
        assert_eq!(r.y, 380);
    }

    #[test]
    fn el_encaje_es_par_y_nunca_se_sale() {
        for (w, h) in [(1, 1), (333, 777), (1366, 768), (5000, 3), (3, 5000)] {
            for (sw, sh) in [(1280, 720), (1920, 1080), (641, 481)] {
                let r = encaje(w, h, sw, sh);
                assert_eq!(r.x % 2, 0);
                assert_eq!(r.y % 2, 0);
                assert_eq!(r.ancho % 2, 0);
                assert_eq!(r.alto % 2, 0);
                // Caso negativo: nunca fuera del video, aunque el video
                // tenga medidas impares.
                assert!(r.x + r.ancho <= par(sw), "{w}x{h} en {sw}x{sh}: {r:?}");
                assert!(r.y + r.alto <= par(sh), "{w}x{h} en {sw}x{sh}: {r:?}");
                assert!(r.ancho >= 2 && r.alto >= 2);
            }
        }
    }
}
