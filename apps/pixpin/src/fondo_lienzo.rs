//! La imagen del pin como fondo fijo del lienzo (D132-D143).
//!
//! No es un elemento de la escena (D133): no pasa por `Escena` ni por la
//! rejilla, asi que el gesto no puede seleccionarla, moverla ni borrarla.
//! Vive en el mundo en (0,0)-(ancho, alto), las coordenadas en las que ya
//! estan las anotaciones del pin.

use pixpin_codec::ImagenRgba;
use pixpin_motor2d::camara::Camara;
use pixpin_render::{Interpolacion, MotorRender, Pintor, RectF};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

/// Margen alrededor de la imagen al abrir, en pixeles logicos (D135).
pub const MARGEN_ENCUADRE: f32 = 48.0;

/// La camara (logica) con la que se abre el lienzo: la imagen centrada, a
/// zoom 1 si cabe con margen y, si no, al zoom que la hace caber (D135).
pub fn encuadre_inicial(
    ancho: f32,
    alto: f32,
    area_ancho_px: f32,
    area_alto_px: f32,
    escala_por_cien: u32,
) -> Camara {
    let escala = crate::navegacion::escala_de(escala_por_cien);
    let (w, h) = (area_ancho_px / escala, area_alto_px / escala);
    let cabe = ancho + 2.0 * MARGEN_ENCUADRE <= w && alto + 2.0 * MARGEN_ENCUADRE <= h;
    if cabe {
        return Camara {
            x: ancho / 2.0 - w / 2.0,
            y: alto / 2.0 - h / 2.0,
            zoom: 1.0,
        };
    }
    Camara::encajar((0.0, 0.0, ancho, alto), w, h, MARGEN_ENCUADRE)
}

/// Como se muestrea la imagen segun cuantos pixeles fisicos ocupa cada
/// pixel suyo (D141): al 100 % exacto o muy ampliada, pixeles tal cual (una
/// captura se lee nitida); reducida, cubica (sin dientes); entre medias,
/// lineal.
pub fn modo_nitidez(zoom_efectivo: f32) -> Interpolacion {
    if zoom_efectivo == 1.0 || zoom_efectivo >= 3.0 {
        Interpolacion::Vecino
    } else if zoom_efectivo < 1.0 {
        Interpolacion::Cubica
    } else {
        Interpolacion::Lineal
    }
}

/// El tamano al que hay que subir una imagen para que su lado mayor no pase
/// de `maximo`, o `None` si ya cabe (D139).
pub fn lado_de_subida(ancho: u32, alto: u32, maximo: u32) -> Option<(u32, u32)> {
    let mayor = ancho.max(alto);
    if maximo == 0 || mayor <= maximo {
        return None;
    }
    let f = maximo as f64 / mayor as f64;
    Some((
        ((ancho as f64 * f).round() as u32).clamp(1, maximo),
        ((alto as f64 * f).round() as u32).clamp(1, maximo),
    ))
}

/// Si la imagen (0,0)-(ancho, alto) toca la caja del mundo que se ve (D142).
pub fn se_ve(vista: (f32, f32, f32, f32), ancho: f32, alto: f32) -> bool {
    vista.0 <= ancho && vista.2 >= 0.0 && vista.1 <= alto && vista.3 >= 0.0
}

/// El recuadro gris que sustituye a una imagen que no se pudo leer. Nunca de
/// cero pixeles: un bitmap vacio no se puede subir.
///
/// Sin llamador de produccion todavia: la Tarea 10 es quien decide el fondo
/// real de un pin y quien recurre a este recuadro si la lectura falla. Esta
/// tarea (8) solo cablea la bandeja pasando `None`, asi que aqui solo lo usan
/// las pruebas -las de este fichero y la de `ventana_editor` que comprueba
/// que el fondo no es seleccionable-.
#[allow(dead_code)]
pub fn recuadro_gris(ancho: u32, alto: u32) -> ImagenRgba {
    let (ancho, alto) = (ancho.max(1), alto.max(1));
    ImagenRgba {
        ancho,
        alto,
        pixeles: [200u8, 200, 200, 255].repeat(ancho as usize * alto as usize),
    }
}

/// La imagen de fondo mientras el lienzo esta abierto.
///
/// Guarda los pixeles (ya reducidos si hizo falta) ademas del bitmap: con el
/// dispositivo perdido hay que volver a subirlo, igual que la cache de tinta
/// se rehace. Todo se suelta al cerrar el lienzo (D143).
pub struct FondoLienzo {
    /// Tamano en el mundo: el de la imagen original.
    ancho: f32,
    alto: f32,
    imagen: ImagenRgba,
    bitmap: Option<ID2D1Bitmap1>,
    /// Subir fallo: no se reintenta en cada fotograma hasta `soltar`.
    fallo: bool,
}

impl FondoLienzo {
    pub fn nuevo(imagen: ImagenRgba, lado_maximo: u32) -> Self {
        let (ancho, alto) = (imagen.ancho as f32, imagen.alto as f32);
        let imagen = match lado_de_subida(imagen.ancho, imagen.alto, lado_maximo) {
            None => imagen,
            Some((w, h)) => {
                tracing::info!(
                    original_ancho = imagen.ancho,
                    original_alto = imagen.alto,
                    ancho = w,
                    alto = h,
                    "imagen del lienzo reducida para la GPU"
                );
                pixpin_codec::redimensionar(imagen, w, h).unwrap_or_else(|e| {
                    tracing::warn!(
                        ?e,
                        "no se pudo reducir la imagen del lienzo; sale sin fondo"
                    );
                    ImagenRgba {
                        ancho: 0,
                        alto: 0,
                        pixeles: Vec::new(),
                    }
                })
            }
        };
        Self {
            ancho,
            alto,
            imagen,
            bitmap: None,
            fallo: false,
        }
    }

    pub fn ancho(&self) -> f32 {
        self.ancho
    }

    pub fn alto(&self) -> f32 {
        self.alto
    }

    /// Cuantos pixeles se suben de verdad.
    ///
    /// Sin llamador de produccion todavia: lo usa la Tarea 10, que es quien
    /// de verdad decide el fondo real de un pin (esta tarea solo cablea la
    /// bandeja pasando `None`). Probado aqui mismo mientras tanto.
    #[allow(dead_code)]
    pub fn tamano_subido(&self) -> (u32, u32) {
        (self.imagen.ancho, self.imagen.alto)
    }

    /// Sube el bitmap si aun no esta (una vez por lienzo, o tras `soltar`).
    pub fn asegurar(&mut self, motor: &MotorRender) {
        if self.bitmap.is_some() || self.fallo || self.imagen.ancho == 0 || self.imagen.alto == 0 {
            return;
        }
        match motor.bitmap_desde_pixeles_premultiplicado(
            self.imagen.ancho,
            self.imagen.alto,
            &self.imagen.pixeles,
        ) {
            Ok(b) => self.bitmap = Some(b),
            Err(e) => {
                self.fallo = true;
                tracing::warn!(?e, "no se pudo subir la imagen del lienzo");
            }
        }
    }

    /// Pinta la imagen con la vista del mundo ya puesta. `vista` es la caja
    /// del mundo visible y `zoom_efectivo` el de la camara efectiva.
    pub fn pintar(&self, p: &Pintor<'_>, vista: (f32, f32, f32, f32), zoom_efectivo: f32) {
        let Some(b) = &self.bitmap else {
            return;
        };
        if !se_ve(vista, self.ancho, self.alto) {
            return;
        }
        p.bitmap_con(
            b,
            RectF {
                x: 0.0,
                y: 0.0,
                ancho: self.ancho,
                alto: self.alto,
            },
            None,
            modo_nitidez(zoom_efectivo),
        );
    }

    /// Olvida el bitmap (dispositivo perdido): el siguiente `asegurar` lo
    /// vuelve a subir desde los pixeles guardados.
    pub fn soltar(&mut self) {
        self.bitmap = None;
        self.fallo = false;
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::navegacion::vista_efectiva;
    use pixpin_motor2d::vector::Punto2;

    #[test]
    fn una_imagen_que_cabe_se_abre_a_zoom_uno_y_centrada() {
        for escala in [100, 150] {
            let c = encuadre_inicial(800.0, 600.0, 1920.0, 1080.0, escala);
            assert_eq!(c.zoom, 1.0, "escala {escala}");
            let centro = vista_efectiva(&c, escala).a_pantalla(Punto2::nuevo(400.0, 300.0));
            assert!(
                (centro.x - 960.0).abs() < 0.5 && (centro.y - 540.0).abs() < 0.5,
                "escala {escala}: {centro:?}"
            );
        }
    }

    #[test]
    fn una_imagen_que_no_cabe_se_abre_al_zoom_que_la_hace_caber_con_margen() {
        let c = encuadre_inicial(4000.0, 3000.0, 1920.0, 1080.0, 100);
        assert!(c.zoom < 1.0);
        assert!(4000.0 * c.zoom + 2.0 * MARGEN_ENCUADRE <= 1920.0 + 0.5);
        assert!(3000.0 * c.zoom + 2.0 * MARGEN_ENCUADRE <= 1080.0 + 0.5);
        let centro = vista_efectiva(&c, 100).a_pantalla(Punto2::nuevo(2000.0, 1500.0));
        assert!((centro.x - 960.0).abs() < 0.5 && (centro.y - 540.0).abs() < 0.5);
    }

    #[test]
    fn un_pixel_mas_que_el_hueco_con_margen_ya_no_va_a_zoom_uno() {
        // Caso negativo del borde: 1825 + 2 x 48 = 1921 > 1920.
        assert!(encuadre_inicial(1825.0, 600.0, 1920.0, 1080.0, 100).zoom < 1.0);
        assert_eq!(
            encuadre_inicial(1824.0, 600.0, 1920.0, 1080.0, 100).zoom,
            1.0
        );
    }

    #[test]
    fn la_nitidez_depende_del_zoom_efectivo() {
        assert_eq!(modo_nitidez(1.0), Interpolacion::Vecino);
        assert_eq!(modo_nitidez(2.0), Interpolacion::Lineal);
        assert_eq!(modo_nitidez(2.999), Interpolacion::Lineal);
        assert_eq!(modo_nitidez(3.0), Interpolacion::Vecino);
        assert_eq!(modo_nitidez(0.5), Interpolacion::Cubica);
        // Caso negativo: casi 1 no es 1; a 1,0001 ya se interpola.
        assert_eq!(modo_nitidez(1.0001), Interpolacion::Lineal);
    }

    #[test]
    fn solo_se_reduce_lo_que_pasa_del_maximo_y_se_conserva_la_proporcion() {
        assert_eq!(lado_de_subida(1920, 1080, 4096), None);
        assert_eq!(lado_de_subida(4096, 10, 4096), None);
        assert_eq!(lado_de_subida(5000, 10, 4096), Some((4096, 8)));
        assert_eq!(lado_de_subida(10, 9000, 4096), Some((5, 4096)));
        // Un maximo desconocido (0) no reduce a nada.
        assert_eq!(lado_de_subida(5000, 10, 0), None);
    }

    #[test]
    fn el_fondo_reducido_sigue_midiendo_lo_que_la_imagen_original() {
        let img = ImagenRgba {
            ancho: 5000,
            alto: 10,
            pixeles: vec![255; 5000 * 10 * 4],
        };
        let f = FondoLienzo::nuevo(img, 4096);
        assert_eq!(f.tamano_subido(), (4096, 8));
        assert_eq!((f.ancho(), f.alto()), (5000.0, 10.0));
        let pequena = ImagenRgba {
            ancho: 20,
            alto: 10,
            pixeles: vec![0; 800],
        };
        assert_eq!(FondoLienzo::nuevo(pequena, 4096).tamano_subido(), (20, 10));
    }

    #[test]
    fn fuera_de_la_vista_el_fondo_no_se_pinta() {
        assert!(se_ve((-10.0, -10.0, 100.0, 100.0), 800.0, 600.0));
        assert!(se_ve((799.0, 599.0, 900.0, 700.0), 800.0, 600.0));
        assert!(!se_ve((801.0, 0.0, 1000.0, 100.0), 800.0, 600.0));
        assert!(!se_ve((-500.0, -500.0, -1.0, -1.0), 800.0, 600.0));
    }

    #[test]
    fn el_recuadro_de_reserva_es_gris_opaco_y_nunca_de_cero() {
        let r = recuadro_gris(3, 2);
        assert_eq!((r.ancho, r.alto), (3, 2));
        assert_eq!(&r.pixeles[0..4], &[200, 200, 200, 255]);
        let minimo = recuadro_gris(0, 0);
        assert_eq!((minimo.ancho, minimo.alto), (1, 1));
    }
}
