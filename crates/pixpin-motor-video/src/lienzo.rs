//! **El encaje por la CPU**, para el ultimo recurso de la cadena: lo mismo
//! que hace el procesador de video en la GPU (encoger y poner bandas), con
//! el escalado bilineal de `pixpin-record`.
//!
//! Solo se usa cuando la grafica no tiene procesador de video; por eso el
//! camino por la CPU graba siempre a lo de «Ligero» (720p, 10 fps).

use crate::geometria;

/// Los buffers que se reutilizan entre fotogramas: una hora a diez por
/// segundo no puede reservar memoria en cada uno.
#[derive(Debug, Default)]
pub struct Lienzo {
    recorte: Vec<u8>,
    encogido: Vec<u8>,
    /// El fotograma listo, BGRA de `ancho`x`alto` sin relleno de fila.
    pub pixeles: Vec<u8>,
}

impl Lienzo {
    /// **Pinta `origen`** (BGRA de `w`x`h`, de la que vale la esquina de
    /// `contenido`) encajado y con bandas negras en un fotograma de
    /// `ancho`x`alto`. Un origen que no trae los bytes que dice da un
    /// fotograma negro, no un cierre.
    pub fn pintar(
        &mut self,
        origen: &[u8],
        (w, h): (u32, u32),
        contenido: (u32, u32),
        (ancho, alto): (u32, u32),
    ) {
        let total = ancho as usize * alto as usize * 4;
        self.pixeles.clear();
        self.pixeles
            .extend(std::iter::repeat_n([0u8, 0, 0, 255], total / 4).flatten());
        let (cw, ch) = (contenido.0.min(w), contenido.1.min(h));
        if cw == 0 || ch == 0 || origen.len() < w as usize * h as usize * 4 {
            return;
        }
        let fuente: &[u8] = if (cw, ch) == (w, h) {
            origen
        } else {
            self.recorte.clear();
            for fila in 0..ch as usize {
                let i = fila * w as usize * 4;
                self.recorte
                    .extend_from_slice(&origen[i..i + cw as usize * 4]);
            }
            &self.recorte
        };
        let r = geometria::encaje(cw, ch, ancho, alto);
        pixpin_record::escala::escalar_bgra(fuente, cw, ch, &mut self.encogido, r.ancho, r.alto);
        let fila_util = r.ancho as usize * 4;
        for fila in 0..r.alto as usize {
            let o = fila * fila_util;
            let d = ((r.y as usize + fila) * ancho as usize + r.x as usize) * 4;
            self.pixeles[d..d + fila_util].copy_from_slice(&self.encogido[o..o + fila_util]);
        }
    }

    /// Un fotograma negro de `ancho`x`alto`.
    pub fn negro(&mut self, (ancho, alto): (u32, u32)) {
        self.pintar(&[], (0, 0), (0, 0), (ancho, alto));
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn px(l: &Lienzo, ancho: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * ancho + x) * 4) as usize;
        l.pixeles[i..i + 4].try_into().unwrap()
    }

    #[test]
    fn la_misma_forma_ocupa_todo_el_fotograma() {
        let blanco = [255u8, 255, 255, 255].repeat(40 * 20);
        let mut l = Lienzo::default();
        l.pintar(&blanco, (40, 20), (40, 20), (20, 10));
        assert_eq!(l.pixeles.len(), 20 * 10 * 4);
        assert!(l.pixeles.chunks_exact(4).all(|p| p == [255, 255, 255, 255]));
    }

    #[test]
    fn una_fuente_cuadrada_lleva_bandas_negras() {
        let blanco = [255u8, 255, 255, 255].repeat(10 * 10);
        let mut l = Lienzo::default();
        l.pintar(&blanco, (10, 10), (10, 10), (20, 10));
        // El centro blanco y los lados negros (y opacos).
        assert_eq!(px(&l, 20, 10, 5), [255, 255, 255, 255]);
        assert_eq!(px(&l, 20, 0, 5), [0, 0, 0, 255]);
        assert_eq!(px(&l, 20, 19, 5), [0, 0, 0, 255]);
    }

    #[test]
    fn solo_cuenta_el_contenido_de_la_esquina() {
        // Una superficie de 8x8 con solo 4x8 de imagen (tras un cambio de
        // tamano): lo de la derecha es basura y no puede salir.
        let mut s = vec![0u8; 8 * 8 * 4];
        for y in 0..8 {
            for x in 0..8 {
                let v = if x < 4 { 200 } else { 9 };
                let i = (y * 8 + x) * 4;
                s[i..i + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        let mut l = Lienzo::default();
        l.pintar(&s, (8, 8), (4, 8), (4, 8));
        assert!(l.pixeles.chunks_exact(4).all(|p| p == [200, 200, 200, 255]));
    }

    #[test]
    fn un_origen_corto_da_negro_y_no_un_cierre() {
        // Caso negativo: dice 100x100 y trae cuatro bytes.
        let mut l = Lienzo::default();
        l.pintar(&[1, 2, 3, 4], (100, 100), (100, 100), (8, 4));
        assert_eq!(l.pixeles.len(), 8 * 4 * 4);
        assert!(l.pixeles.chunks_exact(4).all(|p| p == [0, 0, 0, 255]));
        l.negro((2, 2));
        assert_eq!(l.pixeles, [0, 0, 0, 255].repeat(4));
    }
}
