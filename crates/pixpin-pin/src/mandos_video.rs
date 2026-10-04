//! Los mandos del pin de video, sin nada de pantalla: donde cae cada uno y
//! que hace la rueda o el clic. El pin es una ventana minima —solo la
//! imagen—, asi que los mandos solo aparecen con el raton encima: una franja
//! abajo con reproducir/pausar, el tiempo, la barra para saltar y el
//! altavoz.

use pixpin_render::RectF;

/// Alto de la franja de mandos, en pixeles logicos.
pub const FRANJA_LOGICA: f32 = 36.0;
/// Lado de los botones de la franja, en pixeles logicos.
const BOTON_LOGICO: f32 = 28.0;
/// Ancho reservado al tiempo («12:34 / 56:07»), en pixeles logicos.
const TIEMPO_LOGICO: f32 = 92.0;
/// Por debajo de este ancho el tiempo no cabe y se quita: manda la barra.
const ANCHO_CON_TIEMPO_LOGICO: f32 = 300.0;
/// Por debajo de esto no hay mandos: el pin es una miniatura y se maneja
/// con el teclado o el menu.
const ANCHO_MINIMO_LOGICO: f32 = 120.0;
const ALTO_MINIMO_LOGICO: f32 = 64.0;
/// Lo que sube o baja el volumen cada muesca de la rueda.
pub const PASO_VOLUMEN: f64 = 0.05;
/// Lo que salta cada flecha, en segundos.
pub const SALTO_FLECHA: f64 = 5.0;

/// Que hay bajo el raton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZonaMando {
    Reproducir,
    Barra,
    Sonido,
}

/// Donde va cada mando dentro de la caja del video.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mandos {
    pub franja: RectF,
    pub boton: RectF,
    /// La zona donde se pulsa para saltar (toda la altura de la franja); la
    /// linea que se dibuja va centrada en ella.
    pub barra: RectF,
    pub sonido: RectF,
    /// Donde va el tiempo, si cabe.
    pub tiempo: Option<RectF>,
}

fn dentro(r: RectF, x: f32, y: f32) -> bool {
    x >= r.x && x < r.x + r.ancho && y >= r.y && y < r.y + r.alto
}

/// Coloca los mandos en `caja` (la imagen del video). `None` si el pin es
/// demasiado pequeno para tenerlos.
pub fn mandos_en(caja: RectF, escala: f32) -> Option<Mandos> {
    let e = escala.max(0.5);
    if caja.ancho < ANCHO_MINIMO_LOGICO * e || caja.alto < ALTO_MINIMO_LOGICO * e {
        return None;
    }
    let alto = FRANJA_LOGICA * e;
    let lado = BOTON_LOGICO * e;
    let margen = 4.0 * e;
    let franja = RectF {
        x: caja.x,
        y: caja.y + caja.alto - alto,
        ancho: caja.ancho,
        alto,
    };
    let y_boton = franja.y + (alto - lado) / 2.0;
    let boton = RectF {
        x: franja.x + margen,
        y: y_boton,
        ancho: lado,
        alto: lado,
    };
    let sonido = RectF {
        x: franja.x + franja.ancho - margen - lado,
        y: y_boton,
        ancho: lado,
        alto: lado,
    };
    let con_tiempo = caja.ancho >= ANCHO_CON_TIEMPO_LOGICO * e;
    let tiempo = con_tiempo.then(|| RectF {
        x: boton.x + lado + margen,
        y: franja.y,
        ancho: TIEMPO_LOGICO * e,
        alto,
    });
    let x0 = match tiempo {
        Some(t) => t.x + t.ancho,
        None => boton.x + lado + 2.0 * margen,
    };
    let x1 = sonido.x - 2.0 * margen;
    let barra = RectF {
        x: x0,
        y: franja.y,
        ancho: (x1 - x0).max(1.0),
        alto,
    };
    Some(Mandos {
        franja,
        boton,
        barra,
        sonido,
        tiempo,
    })
}

impl Mandos {
    /// El mando bajo el punto, o `None` si cae fuera de la franja o en un
    /// hueco de ella (el hueco no hace nada: se puede agarrar y mover).
    pub fn zona_en(&self, x: f32, y: f32) -> Option<ZonaMando> {
        if !dentro(self.franja, x, y) {
            return None;
        }
        if dentro(self.boton, x, y) {
            Some(ZonaMando::Reproducir)
        } else if dentro(self.sonido, x, y) {
            Some(ZonaMando::Sonido)
        } else if dentro(self.barra, x, y) {
            Some(ZonaMando::Barra)
        } else {
            None
        }
    }

    /// Que fraccion del video corresponde a una `x` sobre la barra, de 0 a
    /// 1 (fuera de ella se pega al extremo: arrastrando se puede salir).
    pub fn fraccion(&self, x: f32) -> f64 {
        (((x - self.barra.x) / self.barra.ancho.max(1.0)) as f64).clamp(0.0, 1.0)
    }
}

/// El tiempo como lo pone un reproductor: «m:ss», o «h:mm:ss» pasada la
/// hora. Negativos y no numeros, a cero.
pub fn formato_tiempo(segundos: f64) -> String {
    let s = if segundos.is_finite() && segundos > 0.0 {
        segundos.floor() as u64
    } else {
        0
    };
    let (h, m, s) = (s / 3600, (s / 60) % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// El volumen tras girar la rueda `muescas` (positivo, hacia arriba).
pub fn volumen_tras_rueda(volumen: f64, muescas: i32) -> f64 {
    let v = (volumen + muescas as f64 * PASO_VOLUMEN).clamp(0.0, 1.0);
    // Sin restos de coma flotante: 0.1 + 0.05 * 2 tiene que ser 0.2.
    (v * 100.0).round() / 100.0
}

/// El rect que conserva la proporcion del video: mismo ancho y centro, alto
/// segun `nativo`. `None` si ya la conserva (menos de un 2 % de diferencia)
/// o si alguna medida es cero. Para cuando el pin nacio con un tamano
/// provisional y los metadatos traen el de verdad.
pub fn rect_con_proporcion(
    rect: pixpin_geom::Rect,
    nativo: (u32, u32),
) -> Option<pixpin_geom::Rect> {
    let (nw, nh) = nativo;
    if nw == 0 || nh == 0 || rect.ancho == 0 || rect.alto == 0 {
        return None;
    }
    let actual = rect.ancho as f64 / rect.alto as f64;
    let buena = nw as f64 / nh as f64;
    if (actual / buena - 1.0).abs() < 0.02 {
        return None;
    }
    let alto = ((rect.ancho as f64 / buena).round() as u32).max(1);
    Some(pixpin_geom::Rect {
        x: rect.x,
        y: rect.y + (rect.alto as i32 - alto as i32) / 2,
        ancho: rect.ancho,
        alto,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn caja(ancho: f32, alto: f32) -> RectF {
        RectF {
            x: 10.0,
            y: 20.0,
            ancho,
            alto,
        }
    }

    #[test]
    fn la_franja_va_abajo_con_cada_mando_en_su_sitio() {
        let m = mandos_en(caja(640.0, 360.0), 1.0).expect("cabe");
        assert_eq!(
            m.franja.y + m.franja.alto,
            380.0,
            "pegada al borde de abajo"
        );
        let medio_y = m.franja.y + m.franja.alto / 2.0;
        assert_eq!(
            m.zona_en(m.boton.x + 2.0, medio_y),
            Some(ZonaMando::Reproducir)
        );
        assert_eq!(
            m.zona_en(m.sonido.x + 2.0, medio_y),
            Some(ZonaMando::Sonido)
        );
        assert_eq!(
            m.zona_en(m.barra.x + m.barra.ancho / 2.0, medio_y),
            Some(ZonaMando::Barra)
        );
        assert!(m.tiempo.is_some(), "a 640 px el tiempo cabe");
        // Ningun mando se pisa con otro.
        assert!(m.boton.x + m.boton.ancho <= m.barra.x);
        assert!(m.barra.x + m.barra.ancho <= m.sonido.x);
    }

    #[test]
    fn caso_negativo_encima_de_la_imagen_no_hay_mando() {
        let m = mandos_en(caja(640.0, 360.0), 1.0).unwrap();
        // En medio del video: es para mover o pausar, no para un mando.
        assert_eq!(m.zona_en(330.0, 200.0), None);
        // Fuera de la caja tampoco.
        assert_eq!(m.zona_en(-5.0, 379.0), None);
    }

    #[test]
    fn caso_negativo_un_pin_diminuto_no_tiene_mandos() {
        assert!(mandos_en(caja(100.0, 60.0), 1.0).is_none());
        // A 200 % de escala, 200 px fisicos son 100 logicos: tampoco.
        assert!(mandos_en(caja(200.0, 300.0), 2.0).is_none());
    }

    #[test]
    fn estrecho_quita_el_tiempo_y_deja_la_barra() {
        let m = mandos_en(caja(200.0, 120.0), 1.0).expect("cabe");
        assert!(m.tiempo.is_none());
        assert!(m.barra.ancho > 50.0, "barra de {}", m.barra.ancho);
    }

    #[test]
    fn la_fraccion_va_de_cero_a_uno_y_se_pega_a_los_extremos() {
        let m = mandos_en(caja(640.0, 360.0), 1.0).unwrap();
        assert_eq!(m.fraccion(m.barra.x), 0.0);
        assert!((m.fraccion(m.barra.x + m.barra.ancho / 2.0) - 0.5).abs() < 1e-6);
        assert_eq!(
            m.fraccion(m.barra.x - 100.0),
            0.0,
            "caso negativo: antes de la barra"
        );
        assert_eq!(m.fraccion(m.barra.x + m.barra.ancho + 100.0), 1.0);
    }

    #[test]
    fn el_tiempo_se_escribe_como_en_un_reproductor() {
        assert_eq!(formato_tiempo(0.0), "0:00");
        assert_eq!(formato_tiempo(5.9), "0:05");
        assert_eq!(formato_tiempo(754.0), "12:34");
        assert_eq!(formato_tiempo(3_725.0), "1:02:05");
    }

    #[test]
    fn caso_negativo_un_tiempo_raro_es_cero() {
        assert_eq!(formato_tiempo(-3.0), "0:00");
        assert_eq!(formato_tiempo(f64::NAN), "0:00");
        assert_eq!(formato_tiempo(f64::INFINITY), "0:00");
    }

    #[test]
    fn la_rueda_sube_y_baja_el_volumen_sin_pasarse() {
        assert_eq!(volumen_tras_rueda(0.5, 2), 0.6);
        assert_eq!(volumen_tras_rueda(0.5, -3), 0.35);
        assert_eq!(
            volumen_tras_rueda(0.98, 3),
            1.0,
            "caso negativo: no pasa de 1"
        );
        assert_eq!(
            volumen_tras_rueda(0.02, -3),
            0.0,
            "caso negativo: no baja de 0"
        );
    }

    #[test]
    fn el_rect_toma_la_proporcion_del_video_conservando_ancho_y_centro() {
        let r = pixpin_geom::Rect {
            x: 100,
            y: 100,
            ancho: 480,
            alto: 270,
        };
        // Un video vertical de movil.
        let v = rect_con_proporcion(r, (1080, 1920)).expect("hay que cambiarla");
        assert_eq!(v.ancho, 480);
        assert_eq!(v.alto, 853);
        assert_eq!(
            v.y + v.alto as i32 / 2,
            r.y + r.alto as i32 / 2,
            "mismo centro"
        );
    }

    #[test]
    fn caso_negativo_si_la_proporcion_ya_vale_no_se_toca() {
        let r = pixpin_geom::Rect {
            x: 0,
            y: 0,
            ancho: 480,
            alto: 270,
        };
        assert_eq!(rect_con_proporcion(r, (1920, 1080)), None);
        assert_eq!(rect_con_proporcion(r, (1280, 722)), None, "menos de un 2 %");
        assert_eq!(rect_con_proporcion(r, (0, 1080)), None, "sin tamano nativo");
    }
}
