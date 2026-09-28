//! La pizarra y la hoja: dos pines de dibujo que salen de una palabra (C4).
//!
//! **La pizarra no es un tipo de pin nuevo**, igual que en el movil
//! (`ImageStore.saveBlankBoard`, `PinWindowController.regenerateBoard`): es
//! un pin de imagen con el fondo generado aqui. Asi hereda entero el dibujo
//! encima (`.pixpin2d`), el lienzo, copiar y guardar, sin una linea de
//! dibujo nueva. Cambiar el fondo rehace la imagen del mismo tamano y lo
//! dibujado no se pierde, porque son vectores que viven encima.
//!
//! **La hoja** es el lienzo acotado del movil (`MiniApp.SHEET`,
//! `OverlayManager.escenaConHoja`): un marco A4 con papel puesto desde el
//! primer momento, que en el lienzo es una hoja de verdad
//! (`marco::ordenes_del_papel`) y al exportar sale con sus bordes.

use pixpin_codec::ImagenRgba;
use pixpin_motor2d::elemento::{PautaHoja, TamanoPapel};
use pixpin_motor2d::{Elemento, Escena, Figura};

/// El tamano de la pizarra, fijo y en vertical como una cuartilla: donde se
/// escribe a gusto sin que el pin ocupe la pantalla (`BOARD_W`, `BOARD_H`).
pub const ANCHO: u32 = 900;
pub const ALTO: u32 = 1200;

/// La separacion de la pauta, en pixeles del lienzo (`GRID_STEP`).
pub const PASO: u32 = 50;

/// Los cuatro fondos del movil (`BOARD_COLORS`): blanco, negro, azul
/// pizarra y verde pizarra. En RGB.
pub const COLORES: [u32; 4] = [0xFFFFFF, 0x1B1B1B, 0x102A43, 0x14312A];

/// Las cinco pautas (`BoardGrid`), en el orden del movil. El indice es lo
/// que viaja en el menu y en el fichero de al lado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pauta {
    Ninguna,
    Cuadros,
    Horizontal,
    Vertical,
    Puntos,
}

impl Pauta {
    pub const TODAS: [Pauta; 5] = [
        Pauta::Ninguna,
        Pauta::Cuadros,
        Pauta::Horizontal,
        Pauta::Vertical,
        Pauta::Puntos,
    ];

    pub fn por_indice(i: u8) -> Pauta {
        Pauta::TODAS.get(i as usize).copied().unwrap_or(Pauta::Ninguna)
    }
}

/// Si un fondo es claro (`isLight` del movil, la luminancia de siempre).
pub fn es_claro(rgb: u32) -> bool {
    let r = ((rgb >> 16) & 0xFF) as f32;
    let g = ((rgb >> 8) & 0xFF) as f32;
    let b = (rgb & 0xFF) as f32;
    0.299 * r + 0.587 * g + 0.114 * b > 140.0
}

/// Mezcla `tinta` (RGB y alfa 0-255) encima del pixel `i`.
fn mezclar(px: &mut [u8], i: usize, tinta: (u8, u8, u8, u8)) {
    let a = tinta.3 as u32;
    for (k, c) in [tinta.0, tinta.1, tinta.2].into_iter().enumerate() {
        let fondo = px[i + k] as u32;
        px[i + k] = ((c as u32 * a + fondo * (255 - a) + 127) / 255) as u8;
    }
}

/// **El fondo de una pizarra**: el color y su pauta, como `saveBlankBoard`.
///
/// El color de la pauta sale del propio fondo —oscura sobre claro y al
/// reves—: fijarla a un gris la haria invisible en la pizarra negra, que es
/// justo donde mas falta hace (`0x22000000` / `0x33FFFFFF` del movil). Las
/// rayas son de dos pixeles, centradas en su sitio, y los puntos de radio
/// tres, como alli.
pub fn fondo(color: u8, pauta: u8) -> ImagenRgba {
    let rgb = COLORES[(color as usize).min(COLORES.len() - 1)];
    let (r, g, b) = ((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
    let mut px = [r, g, b, 255].repeat((ANCHO * ALTO) as usize);
    let tinta = if es_claro(rgb) {
        (0, 0, 0, 0x22)
    } else {
        (255, 255, 255, 0x33)
    };
    let pauta = Pauta::por_indice(pauta);
    let indice = |x: u32, y: u32| ((y * ANCHO + x) * 4) as usize;
    match pauta {
        Pauta::Ninguna => {}
        Pauta::Puntos => {
            let mut y = PASO;
            while y < ALTO {
                let mut x = PASO;
                while x < ANCHO {
                    for dy in -3i32..=3 {
                        for dx in -3i32..=3 {
                            // Dentro del circulo de radio 3.
                            if dx * dx + dy * dy <= 9 {
                                let (px_x, px_y) = (x as i32 + dx, y as i32 + dy);
                                if px_x >= 0
                                    && px_y >= 0
                                    && (px_x as u32) < ANCHO
                                    && (px_y as u32) < ALTO
                                {
                                    mezclar(&mut px, indice(px_x as u32, px_y as u32), tinta);
                                }
                            }
                        }
                    }
                    x += PASO;
                }
                y += PASO;
            }
        }
        _ => {
            // Rayas y columnas se cruzan en los cuadros: el cruce se pinta
            // una sola vez, o saldria un punto mas oscuro en cada esquina.
            let raya_h = |y: u32| pauta != Pauta::Vertical && y >= PASO - 1 && (y + 1) % PASO <= 1;
            let raya_v = |x: u32| pauta != Pauta::Horizontal && x >= PASO - 1 && (x + 1) % PASO <= 1;
            for y in 0..ALTO {
                let en_h = raya_h(y);
                for x in 0..ANCHO {
                    if en_h || raya_v(x) {
                        mezclar(&mut px, indice(x, y), tinta);
                    }
                }
            }
        }
    }
    ImagenRgba {
        ancho: ANCHO,
        alto: ALTO,
        pixeles: px,
    }
}

/// Lo que se guarda junto a la imagen de una pizarra para saber que lo es y
/// con que fondo: `<color> <pauta>`. Aparte y no en el nombre, porque el
/// almacen pone el nombre de sus objetos.
pub fn escribir_estado(color: u8, pauta: u8) -> String {
    format!("{color} {pauta}")
}

/// El contrario; lo que no se entiende vuelve al fondo de fabrica, que es
/// mejor que perder la pizarra.
pub fn leer_estado(texto: &str) -> (u8, u8) {
    let mut partes = texto.split_whitespace().map(|p| p.parse::<u8>().ok());
    let color = partes.next().flatten().filter(|c| (*c as usize) < COLORES.len());
    let pauta = partes.next().flatten().filter(|p| (*p as usize) < Pauta::TODAS.len());
    (color.unwrap_or(0), pauta.unwrap_or(0))
}

/// El ancho de la hoja del pin en pixeles de escena. 800 como el movil: con
/// la letra a 20 caben unos sesenta caracteres por linea.
pub const ANCHO_HOJA: u32 = 800;

/// El alto de una hoja A4 de ese ancho.
pub fn alto_hoja() -> u32 {
    (ANCHO_HOJA as f32 * TamanoPapel::A4.proporcion()).round() as u32
}

/// El papel de la hoja: blanco y del tamano del A4. Es el fondo del pin; la
/// hoja de verdad (el marco con papel) va en la escena.
pub fn papel_de_la_hoja() -> ImagenRgba {
    let (w, h) = (ANCHO_HOJA, alto_hoja());
    ImagenRgba {
        ancho: w,
        alto: h,
        pixeles: [255u8, 255, 255, 255].repeat((w * h) as usize),
    }
}

/// La escena con la que nace la hoja: un marco A4 con papel liso, justo
/// encima del fondo del pin (`escenaConHoja` del movil).
pub fn escena_de_la_hoja(nombre: &str) -> Escena {
    let mut escena = Escena::nueva();
    let mut marco = Elemento {
        figura: Figura::Marco {
            nombre: nombre.to_string(),
        },
        x: 0.0,
        y: 0.0,
        ancho: ANCHO_HOJA as f32,
        alto: alto_hoja() as f32,
        ..Elemento::default()
    };
    marco.extras.papel = Some(TamanoPapel::A4);
    marco.extras.pauta = PautaHoja::Lisa;
    escena.anadir(marco);
    escena
}

/// El lienzo con el que nace un «lienzo» pineado: blanco y apaisado, del
/// tamano de reserva del lienzo del pin. El infinito esta al abrirlo.
pub fn papel_del_lienzo() -> ImagenRgba {
    let (w, h) = (800u32, 600u32);
    ImagenRgba {
        ancho: w,
        alto: h,
        pixeles: [255u8, 255, 255, 255].repeat((w * h) as usize),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn pixel(img: &ImagenRgba, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * img.ancho + x) * 4) as usize;
        [
            img.pixeles[i],
            img.pixeles[i + 1],
            img.pixeles[i + 2],
            img.pixeles[i + 3],
        ]
    }

    #[test]
    fn la_pizarra_lisa_es_de_su_color_de_punta_a_punta() {
        let img = fondo(2, 0);
        assert_eq!((img.ancho, img.alto), (900, 1200));
        assert_eq!(pixel(&img, 0, 0), [0x10, 0x2A, 0x43, 255]);
        assert_eq!(pixel(&img, 450, 600), [0x10, 0x2A, 0x43, 255]);
        assert!(img.es_opaca());
    }

    #[test]
    fn en_la_pizarra_negra_la_pauta_es_clara_y_en_la_blanca_oscura() {
        let negra = fondo(1, 1);
        let raya = pixel(&negra, 10, PASO);
        assert!(raya[0] > 0x1B, "sobre negro la raya aclara: {raya:?}");
        let blanca = fondo(0, 1);
        let raya = pixel(&blanca, 10, PASO);
        assert!(raya[0] < 0xFF, "sobre blanco la raya oscurece: {raya:?}");
        // Caso negativo: entre rayas, el fondo tal cual.
        assert_eq!(pixel(&blanca, 10, PASO + 10), [255, 255, 255, 255]);
    }

    #[test]
    fn las_rayas_solo_van_a_lo_ancho_y_las_columnas_solo_a_lo_alto() {
        let rayas = fondo(0, 2);
        assert_ne!(pixel(&rayas, 10, PASO), [255, 255, 255, 255]);
        assert_eq!(pixel(&rayas, PASO, 10), [255, 255, 255, 255], "sin columnas");
        let columnas = fondo(0, 3);
        assert_ne!(pixel(&columnas, PASO, 10), [255, 255, 255, 255]);
        assert_eq!(pixel(&columnas, 10, PASO), [255, 255, 255, 255], "sin rayas");
    }

    #[test]
    fn el_cruce_de_los_cuadros_no_sale_mas_oscuro_que_la_raya() {
        let c = fondo(0, 1);
        assert_eq!(pixel(&c, PASO, PASO), pixel(&c, 10, PASO));
    }

    #[test]
    fn los_puntos_caen_en_la_rejilla_y_no_entre_medias() {
        let p = fondo(0, 4);
        assert_ne!(pixel(&p, PASO, PASO), [255, 255, 255, 255]);
        assert_eq!(pixel(&p, PASO + 10, PASO), [255, 255, 255, 255]);
        assert_eq!(pixel(&p, 10, 10), [255, 255, 255, 255], "ni en el borde");
    }

    /// Las cuatro pizarras con una pauta cada una, para mirarlas a ojo.
    #[test]
    #[ignore = "genera PNG de muestra"]
    fn muestras_de_la_pizarra() {
        let dir = std::env::var_os("PIXPIN_MUESTRAS_PINES")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        for (color, pauta) in [(0, 1), (1, 4), (2, 2), (3, 3)] {
            let img = fondo(color, pauta);
            std::fs::write(
                dir.join(format!("pizarra-{color}-{pauta}.png")),
                pixpin_codec::codificar_png(&img).unwrap(),
            )
            .unwrap();
        }
    }

    #[test]
    fn un_indice_fuera_de_la_paleta_no_rompe_nada() {
        let img = fondo(99, 99);
        assert_eq!(pixel(&img, 0, 0), [0x14, 0x31, 0x2A, 255], "el ultimo color");
    }

    #[test]
    fn el_estado_de_la_pizarra_vuelve_igual_y_lo_roto_es_de_fabrica() {
        assert_eq!(leer_estado(&escribir_estado(3, 4)), (3, 4));
        assert_eq!(leer_estado("basura"), (0, 0));
        assert_eq!(leer_estado("9 9"), (0, 0), "fuera de la paleta");
        assert_eq!(leer_estado("2"), (2, 0));
    }

    #[test]
    fn la_hoja_es_un_a4_con_su_marco_de_papel_encima_del_fondo() {
        let papel = papel_de_la_hoja();
        assert_eq!((papel.ancho, papel.alto), (800, 1131));
        let escena = escena_de_la_hoja("Hoja");
        let hojas = pixpin_motor2d::marco::hojas_en_orden(&escena.elementos);
        assert_eq!(hojas.len(), 1);
        let h = hojas[0];
        assert_eq!(h.extras.papel, Some(TamanoPapel::A4));
        assert_eq!((h.x, h.y, h.ancho, h.alto), (0.0, 0.0, 800.0, 1131.0));
        // Y en el lienzo se pinta su papel: es una hoja, no un marco suelto.
        assert!(!pixpin_motor2d::marco::ordenes_del_papel(h).is_empty());
    }
}
