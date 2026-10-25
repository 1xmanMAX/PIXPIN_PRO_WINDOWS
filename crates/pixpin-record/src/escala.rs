//! Las cuentas del video en vivo, sin Windows: a que tamano se graba, como
//! se encoge cada fotograma y en que instante va cada cosa.
//!
//! Viven aparte de [`crate::mp4_vivo`] para poder probarlas en cualquier
//! maquina: son justo las que, hechas mal, dan un video estirado, inclinado
//! o desacompasado del sonido, y nada de eso se ve en un error.

/// Media Foundation cuenta el tiempo en unidades de 100 nanosegundos.
pub const UNIDADES_POR_SEGUNDO: i64 = 10_000_000;

/// **El tamano al que se graba** una pantalla de `ancho`x`alto` sin pasar de
/// `tope_ancho`x`tope_alto`: la misma forma (nunca se estira), nunca mas
/// grande que la pantalla, y con los dos lados pares, que es lo unico que
/// admite H.264 (ver `mp4::medidas`).
///
/// Encoger importa en un equipo modesto: codificar 1920x1080 por software
/// cuesta mas del doble que 1280x720, y en una reunion lo que se mira —las
/// diapositivas, las caras— se lee igual.
pub fn medida_de_salida(ancho: u32, alto: u32, tope_ancho: u32, tope_alto: u32) -> (u32, u32) {
    let (ancho, alto) = (ancho.max(2), alto.max(2));
    let (tope_ancho, tope_alto) = (tope_ancho.max(2), tope_alto.max(2));
    // La escala como fraccion exacta (num/den) para no arrastrar redondeos
    // de coma flotante: la menor de las dos, y nunca mas de uno.
    let (num, den) =
        if u64::from(tope_ancho) * u64::from(alto) <= u64::from(tope_alto) * u64::from(ancho) {
            (u64::from(tope_ancho), u64::from(ancho))
        } else {
            (u64::from(tope_alto), u64::from(alto))
        };
    let (num, den) = if num >= den { (1, 1) } else { (num, den) };
    let w = (u64::from(ancho) * num / den) as u32;
    let h = (u64::from(alto) * num / den) as u32;
    ((w & !1).max(2), (h & !1).max(2))
}

/// **Encoge (o copia) un fotograma BGRA** de `ancho`x`alto` a
/// `dw`x`dh`, bilineal, en `destino` (que se reutiliza entre fotogramas).
///
/// Bilineal y no el vecino mas cercano: el texto de una diapositiva
/// encogida a dos tercios con el vecino pierde trazos enteros y no se lee.
/// Cuentas en coma fija (16 bits de fraccion) porque esto corre diez veces
/// por segundo durante una hora.
///
/// Si `origen` no trae los bytes que dice su tamano, el fotograma sale
/// negro en vez de leer fuera: mejor un cuadro negro que un cierre.
pub fn escalar_bgra(origen: &[u8], ancho: u32, alto: u32, destino: &mut Vec<u8>, dw: u32, dh: u32) {
    let total = dw as usize * dh as usize * 4;
    destino.clear();
    destino.resize(total, 0);
    let (ow, oh) = (ancho as usize, alto as usize);
    if ow == 0 || oh == 0 || origen.len() < ow * oh * 4 || total == 0 {
        return;
    }
    if (ow, oh) == (dw as usize, dh as usize) {
        destino.copy_from_slice(&origen[..total]);
        return;
    }
    // Para cada columna de salida: la de origen de la izquierda y el peso de
    // la de la derecha. Se calcula una vez por fotograma, no por pixel.
    let columnas: Vec<(usize, u32)> = (0..dw as usize)
        .map(|x| posicion(x, dw as usize, ow))
        .collect();
    for y in 0..dh as usize {
        let (y0, fy) = posicion(y, dh as usize, oh);
        let y1 = (y0 + 1).min(oh - 1);
        let fila0 = &origen[y0 * ow * 4..(y0 + 1) * ow * 4];
        let fila1 = &origen[y1 * ow * 4..(y1 + 1) * ow * 4];
        let salida = &mut destino[y * dw as usize * 4..(y + 1) * dw as usize * 4];
        for (x, &(x0, fx)) in columnas.iter().enumerate() {
            let x1 = (x0 + 1).min(ow - 1);
            for c in 0..4 {
                let a = u32::from(fila0[x0 * 4 + c]);
                let b = u32::from(fila0[x1 * 4 + c]);
                let d = u32::from(fila1[x0 * 4 + c]);
                let e = u32::from(fila1[x1 * 4 + c]);
                let arriba = a * (UNO - fx) + b * fx;
                let abajo = d * (UNO - fx) + e * fx;
                // `arriba` y `abajo` llevan 16 bits de fraccion; la mezcla
                // vertical anade otros 16: se redondea y se quitan los 32.
                let v = (u64::from(arriba) * u64::from(UNO - fy)
                    + u64::from(abajo) * u64::from(fy)
                    + (1u64 << 31))
                    >> 32;
                salida[x * 4 + c] = v.min(255) as u8;
            }
        }
    }
}

/// Uno en coma fija de 16 bits.
const UNO: u32 = 1 << 16;

/// El pixel de origen (izquierda/arriba) y el peso del siguiente para la
/// posicion `i` de `n` en la salida, sobre `m` de entrada: centros con
/// centros, que es lo que no desplaza la imagen medio pixel.
fn posicion(i: usize, n: usize, m: usize) -> (usize, u32) {
    // centro = (i + 0,5) * m / n - 0,5, en coma fija.
    let centro = ((2 * i as u64 + 1) * m as u64 * u64::from(UNO)) / (2 * n as u64);
    let centro = centro.saturating_sub(u64::from(UNO / 2));
    let entero = (centro >> 16) as usize;
    if entero >= m - 1 {
        return (m - 1, 0);
    }
    (entero, (centro & u64::from(UNO - 1)) as u32)
}

/// Instante en el que empieza el fotograma `indice`, en unidades de 100 ns.
/// Desde el principio y no sumando duraciones: a ritmos que no dividen el
/// segundo, sumar redondeos atrasa el video un cuadro cada pocos segundos.
pub fn comienzo(indice: u64, por_segundo: u32) -> i64 {
    (indice as i64 * UNIDADES_POR_SEGUNDO) / i64::from(por_segundo.max(1))
}

/// El instante, en 100 ns, al que se llega tras `muestras` muestras de
/// sonido a `muestreo` por segundo. Es el reloj del video: el sonido no se
/// para nunca salvo en pausa, asi que el video no puede desacompasarse.
pub fn tiempo_de_muestras(muestras: u64, muestreo: u32) -> i64 {
    if muestreo == 0 {
        return 0;
    }
    (u128::from(muestras) * UNIDADES_POR_SEGUNDO as u128 / u128::from(muestreo)) as i64
}

/// Los unicos muestreos que admite el codificador AAC de Windows. La
/// grabadora ya remuestrea a uno de ellos (`pixpin-audio::mezcla`); aqui se
/// comprueba para que otro se diga claro y no como un fallo de Media
/// Foundation sin contexto.
pub const MUESTREOS_AAC: [u32; 2] = [44_100, 48_000];

/// Si el codificador AAC acepta `muestreo`.
pub fn muestreo_aac_valido(muestreo: u32) -> bool {
    MUESTREOS_AAC.contains(&muestreo)
}

/// Cuantos fotogramas pueden esperar al codificador antes de saltarse uno.
/// Veinte a 720p son unos 70 MB: holgura para un tiron, no para una hora.
pub const TOPE_EN_COLA: u64 = 20;

/// Por encima de esto se salta **siempre**, aunque se lleven muchos
/// saltados: cuarenta a 720p son unos 150 MB, y en el equipo de referencia
/// (4 GB) dejar crecer la cola es acabar sin memoria a mitad de reunion.
pub const TOPE_DURO: u64 = 40;

/// Cuantos fotogramas seguidos se pueden saltar como mucho por debajo de
/// [`TOPE_DURO`]. Si el codificador retiene fotogramas por diseno (mira
/// hacia delante), saltar siempre lo dejaria sin alimento y el video
/// congelado; asi sigue avanzando aunque sea a menos ritmo.
pub const SALTOS_SEGUIDOS: u32 = 4;

/// **Si se salta el fotograma** que toca, con `escritos` ya dados al
/// codificador, `en_cola` esperandole y `saltados_seguidos` justo antes.
/// El primero nunca se salta: sin el no hay video.
pub fn saltar_fotograma(escritos: u64, en_cola: u64, saltados_seguidos: u32) -> bool {
    escritos > 0
        && (en_cola > TOPE_DURO || (en_cola > TOPE_EN_COLA && saltados_seguidos < SALTOS_SEGUIDOS))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_medida_de_salida_conserva_la_forma_y_es_par() {
        // La pantalla de siempre cabe justa en 720p.
        assert_eq!(medida_de_salida(1920, 1080, 1280, 720), (1280, 720));
        // La de un portatil viejo: 1366x768 no es 16:9 exacto; sale par.
        let (w, h) = medida_de_salida(1366, 768, 1280, 720);
        assert_eq!((w % 2, h % 2), (0, 0));
        assert_eq!(w, 1280);
        assert!((718..=720).contains(&h), "{h}");
        // Una pantalla mas pequena que el tope no se agranda.
        assert_eq!(medida_de_salida(1024, 768, 1280, 720), (960, 720));
        assert_eq!(medida_de_salida(800, 600, 1280, 720), (800, 600));
        // Vertical: manda el alto.
        assert_eq!(medida_de_salida(1080, 1920, 1280, 720), (404, 720));
        // Caso negativo: impares y diminutas no dan un lado impar ni cero.
        assert_eq!(medida_de_salida(801, 601, 1280, 720), (800, 600));
        assert_eq!(medida_de_salida(1, 1, 1280, 720), (2, 2));
    }

    #[test]
    fn escalar_al_mismo_tamano_copia_tal_cual() {
        let origen: Vec<u8> = (0..4 * 4 * 4).map(|i| i as u8).collect();
        let mut d = Vec::new();
        escalar_bgra(&origen, 4, 4, &mut d, 4, 4);
        assert_eq!(d, origen);
    }

    #[test]
    fn escalar_un_color_liso_da_el_mismo_color() {
        let origen = [10u8, 20, 30, 255].repeat(30 * 20);
        let mut d = Vec::new();
        escalar_bgra(&origen, 30, 20, &mut d, 20, 12);
        assert_eq!(d.len(), 20 * 12 * 4);
        assert!(d.chunks_exact(4).all(|p| p == [10, 20, 30, 255]), "{d:?}");
    }

    #[test]
    fn escalar_conserva_que_va_arriba_y_que_a_la_izquierda() {
        // Arriba a la izquierda blanco, lo demas negro: si las filas o las
        // columnas se cruzaran, el blanco acabaria en otra esquina.
        let (w, h) = (40u32, 40u32);
        let mut origen = vec![0u8; (w * h * 4) as usize];
        for y in 0..20 {
            for x in 0..20 {
                let i = ((y * w + x) * 4) as usize;
                origen[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        let mut d = Vec::new();
        escalar_bgra(&origen, w, h, &mut d, 20, 20);
        let px = |x: usize, y: usize| d[(y * 20 + x) * 4];
        assert_eq!(px(2, 2), 255);
        assert_eq!(px(17, 17), 0);
        assert_eq!(px(17, 2), 0);
        assert_eq!(px(2, 17), 0);
    }

    #[test]
    fn un_origen_corto_da_un_fotograma_negro_y_no_un_cierre() {
        // Caso negativo: dice 100x100 y trae cuatro bytes.
        let mut d = vec![7u8; 3];
        escalar_bgra(&[1, 2, 3, 4], 100, 100, &mut d, 10, 10);
        assert_eq!(d.len(), 400);
        assert!(d.iter().all(|b| *b == 0));
    }

    #[test]
    fn el_tiempo_de_las_muestras_es_exacto() {
        assert_eq!(tiempo_de_muestras(48_000, 48_000), UNIDADES_POR_SEGUNDO);
        assert_eq!(
            tiempo_de_muestras(44_100 * 3600, 44_100),
            3600 * UNIDADES_POR_SEGUNDO
        );
        assert_eq!(tiempo_de_muestras(480, 48_000), 100_000);
        // Caso negativo: sin muestreo no hay reloj, y no se divide por cero.
        assert_eq!(tiempo_de_muestras(100, 0), 0);
    }

    #[test]
    fn el_comienzo_de_los_fotogramas_no_acumula_redondeo() {
        assert_eq!(comienzo(0, 30), 0);
        assert_eq!(comienzo(30, 30), UNIDADES_POR_SEGUNDO);
        assert_eq!(comienzo(10, 10), UNIDADES_POR_SEGUNDO);
    }

    #[test]
    fn solo_valen_los_muestreos_del_aac() {
        assert!(muestreo_aac_valido(44_100));
        assert!(muestreo_aac_valido(48_000));
        // Caso negativo: los de algunas tarjetas, sin remuestrear.
        assert!(!muestreo_aac_valido(96_000));
        assert!(!muestreo_aac_valido(16_000));
        assert!(!muestreo_aac_valido(0));
    }

    #[test]
    fn saltar_fotogramas_tiene_un_tope_que_no_se_pasa() {
        // El primero nunca, aunque la cola diga lo que diga.
        assert!(!saltar_fotograma(0, 1_000, 0));
        // Con poca cola, nunca.
        assert!(!saltar_fotograma(10, TOPE_EN_COLA, 0));
        // Atrasado: se salta, pero no mas de `SALTOS_SEGUIDOS` seguidos.
        assert!(saltar_fotograma(10, TOPE_EN_COLA + 1, 0));
        assert!(!saltar_fotograma(10, TOPE_EN_COLA + 1, SALTOS_SEGUIDOS));
        // Caso negativo: muy atrasado, se salta siempre; si no, la cola
        // crece sin fin con un codificador que no da abasto.
        assert!(saltar_fotograma(10, TOPE_DURO + 1, SALTOS_SEGUIDOS));
        assert!(saltar_fotograma(10, TOPE_DURO + 1, 1_000));
    }
}
