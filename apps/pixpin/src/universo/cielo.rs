//! El cielo del movil (`FondoCosmico`, `Cosmos.kt:90-112`), horneado en un
//! bitmap opaco: el degradado radial `#151B3F -> #05060F` y las dos
//! nebulosas, violeta y cian.
//!
//! **Por que horneado y no pintado.** Son tres degradados radiales que tapan
//! la pantalla entera:
//!
//! - Con pinceles radiales de Direct2D serian tres pinceles nuevos por
//!   fotograma, a ~0,5 ms de CPU cada uno (`lienzo.rs:479-483`).
//! - Con `Pintor::brillo` son tres bitmaps **con alfa** estirados; medido a
//!   3000 x 2000, el fondo paso de 0,5 ms a 4,7 ms y el paneo de 1,9 ms a
//!   7,1 ms: casi el presupuesto entero en adorno.
//!
//! Horneados juntos en un bitmap chico y **opaco** (`bitmap_desde_pixeles`,
//! que ignora el alfa) y estirado a la pantalla **en lugar** de `limpiar`,
//! los tres cuestan una sola pasada sin mezcla. Se rehace solo al cambiar de
//! tamano la ventana.
//!
//! El movil tambien las tiene quietas en la pantalla en `FondoCosmico`; en
//! la pantalla de la galaxia las mueve con la camara a 0,6 de su velocidad
//! (`Galaxia.kt:805-812`). Aqui se quedan quietas: el paralaje de las
//! nebulosas costaba esos milisegundos y no se nota al lado del de las
//! estrellas, que si lo tienen.

use pixpin_render::{ErrorRender, MotorRender, Pintor, RectF};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

/// Lo ancho que se hornea. Un degradado tan suave no necesita mas: estirado
/// a 3000 px con interpolacion lineal no se le ven escalones, y generarlo
/// son 192 x 128 pixeles de CPU en vez de seis millones.
const ANCHO: u32 = 192;

/// `#151B3F` en el centro y `#05060F` en el borde, como alli.
const DENTRO: (f32, f32, f32) = (0x15 as f32, 0x1B as f32, 0x3F as f32);
const FUERA: (f32, f32, f32) = (0x05 as f32, 0x06 as f32, 0x0F as f32);
/// Donde cae el centro del degradado y hasta donde llega, en partes de la
/// pantalla (`Cosmos.kt:95`).
const CENTRO: (f32, f32) = (0.5, 0.35);
const RADIO: f32 = 0.95;

/// Las dos nebulosas de `FondoCosmico`: `(r, g, b, alfa, cx, cy, radio)`,
/// los centros en partes de la pantalla y el radio en partes del ANCHO.
const NEBULOSAS: [(f32, f32, f32, f32, f32, f32, f32); 2] = [
    (
        0x5B as f32,
        0x3F as f32,
        0xD9 as f32,
        0x33 as f32 / 255.0,
        0.2,
        0.22,
        0.8,
    ),
    (
        0x1F as f32,
        0xA2 as f32,
        0xC9 as f32,
        0x2A as f32 / 255.0,
        0.9,
        0.78,
        0.7,
    ),
];

/// Lo que mide el bitmap del cielo para una pantalla de `ancho` x `alto`.
///
/// Se hace con la MISMA proporcion que la pantalla, asi que al estirarlo los
/// degradados siguen siendo circulos y no elipses.
///
/// Es publico porque A3 fase 2 le da al cielo su propio visual de
/// composicion, y la superficie de ese visual se crea con este tamano -chico-
/// y la estira la composicion: 192 px de ancho en vez de 3000 son unos
/// cientos de kilobytes en vez de 24 MB.
pub fn tamano(ancho: u32, alto: u32) -> (u32, u32) {
    let w = ANCHO.max(1);
    let h = ((w as f32 * alto.max(1) as f32 / ancho.max(1) as f32).round() as u32).max(1);
    (w, h)
}

/// Los pixeles RGBA del cielo para una pantalla de `ancho` x `alto`.
pub fn pixeles(ancho: u32, alto: u32) -> (u32, u32, Vec<u8>) {
    let (w, h) = tamano(ancho, alto);
    let (cx, cy) = (w as f32 * CENTRO.0, h as f32 * CENTRO.1);
    // El radio va con el lado mayor **en pixeles de pantalla**, llevado a la
    // rejilla chica por el mismo factor en los dos ejes.
    let radio = (w.max(h) as f32 * RADIO).max(1.0);
    let mut v = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let t = (((px - cx).powi(2) + (py - cy).powi(2)).sqrt() / radio).clamp(0.0, 1.0);
            let mut c = [
                DENTRO.0 + (FUERA.0 - DENTRO.0) * t,
                DENTRO.1 + (FUERA.1 - DENTRO.1) * t,
                DENTRO.2 + (FUERA.2 - DENTRO.2) * t,
            ];
            // Las nebulosas encima, como las pinta Compose: mezcla normal,
            // con el alfa cayendo a cero en el borde de cada una.
            for (nr, ng, nb, alfa, ncx, ncy, nradio) in NEBULOSAS {
                let (dx, dy) = (px - w as f32 * ncx, py - h as f32 * ncy);
                let r = (w as f32 * nradio).max(1.0);
                let d = (dx * dx + dy * dy).sqrt();
                if d >= r {
                    continue;
                }
                let a = alfa * (1.0 - d / r);
                for (k, n) in [nr, ng, nb].into_iter().enumerate() {
                    c[k] += (n - c[k]) * a;
                }
            }
            let i = ((y * w + x) * 4) as usize;
            for k in 0..3 {
                v[i + k] = c[k].round().clamp(0.0, 255.0) as u8;
            }
            v[i + 3] = 255;
        }
    }
    (w, h, v)
}

/// El degradado ya subido, con el tamano de pantalla para el que se hizo.
#[derive(Default)]
pub struct Cielo {
    hecho: Option<((u32, u32), ID2D1Bitmap1)>,
}

impl Cielo {
    /// Lo hornea si falta o si la ventana cambio de tamano. Fuera del
    /// fotograma, como todo lo que crea recursos.
    pub fn preparar(
        &mut self,
        motor: &MotorRender,
        ancho: f32,
        alto: f32,
    ) -> Result<(), ErrorRender> {
        let tam = (ancho.max(1.0) as u32, alto.max(1.0) as u32);
        if self.hecho.as_ref().is_some_and(|(t, _)| *t == tam) {
            return Ok(());
        }
        let (w, h, px) = pixeles(tam.0, tam.1);
        self.hecho = Some((tam, motor.bitmap_desde_pixeles(w, h, &px)?));
        Ok(())
    }

    /// Se perdio el dispositivo: el bitmap era del viejo.
    pub fn soltar(&mut self) {
        self.hecho = None;
    }

    /// Pinta el cielo tapando la pantalla. Devuelve si lo hizo: si no, quien
    /// llama tiene que limpiar con un color liso.
    pub fn pintar(&self, p: &Pintor, ancho: f32, alto: f32) -> bool {
        let Some((_, b)) = &self.hecho else {
            return false;
        };
        p.bitmap(
            b,
            RectF {
                x: 0.0,
                y: 0.0,
                ancho,
                alto,
            },
            None,
            false,
        );
        true
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn pixel(px: &[u8], w: u32, x: u32, y: u32) -> (u8, u8, u8, u8) {
        let i = ((y * w + x) * 4) as usize;
        (px[i], px[i + 1], px[i + 2], px[i + 3])
    }

    #[test]
    fn el_cielo_guarda_la_proporcion_de_la_pantalla_y_es_opaco() {
        let (w, h, px) = pixeles(3000, 2000);
        assert_eq!((w, h), (192, 128));
        assert_eq!(px.len(), (w * h * 4) as usize);
        assert!(px.chunks(4).all(|c| c[3] == 255), "sin alfa que mezclar");
        // Una pantalla vertical da un bitmap vertical.
        assert_eq!(pixeles(1000, 2000).1, 384);
    }

    #[test]
    fn va_del_azul_del_centro_al_casi_negro_del_borde() {
        let (w, h, px) = pixeles(3000, 2000);
        // El centro del degradado es lo mas claro de la columna.
        let centro = pixel(
            &px,
            w,
            (w as f32 * CENTRO.0) as u32,
            (h as f32 * CENTRO.1) as u32,
        );
        let abajo = pixel(&px, w, (w as f32 * CENTRO.0) as u32, h - 1);
        assert!(abajo.2 < centro.2, "{abajo:?} vs {centro:?}");
        // Caso negativo: el centro NO es el del lienzo, esta mas arriba, asi
        // que bajando desde el ya oscurece.
        let medio = pixel(&px, w, (w as f32 * CENTRO.0) as u32, h / 2);
        assert!(medio.2 < centro.2, "{medio:?} vs {centro:?}");
    }

    #[test]
    fn las_dos_nebulosas_tinen_su_rincon_y_no_el_contrario() {
        let (w, h, px) = pixeles(3000, 2000);
        // La violeta arriba a la izquierda: mas rojo que el cielo de al lado
        // a la misma altura, que es lo que hace un violeta sobre azul.
        let violeta = pixel(&px, w, (w as f32 * 0.2) as u32, (h as f32 * 0.22) as u32);
        let cian = pixel(&px, w, (w as f32 * 0.9) as u32, (h as f32 * 0.78) as u32);
        assert!(violeta.0 > cian.0, "violeta {violeta:?} vs cian {cian:?}");
        // La cian: mas verde que rojo, que es lo que la distingue.
        assert!(cian.1 > cian.0, "{cian:?}");
        // Caso negativo: en la violeta no, alli el rojo gana al verde.
        assert!(violeta.0 > violeta.1, "{violeta:?}");
        // Y las dos tienen mas azul que nada: siguen siendo cielo.
        assert!(violeta.2 > violeta.0 && cian.2 > cian.1);
    }

    #[test]
    fn una_pantalla_sin_tamano_no_revienta_y_da_al_menos_un_pixel() {
        let (w, h, px) = pixeles(0, 0);
        assert!(w >= 1 && h >= 1);
        assert_eq!(px.len(), (w * h * 4) as usize);
    }
}
