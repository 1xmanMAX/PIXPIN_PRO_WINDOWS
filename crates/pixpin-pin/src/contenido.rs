//! Que muestra un pin: una imagen, una nota o la ficha de un archivo.
//!
//! El pin no sabe de almacenes ni de tipos de entrada (`pixpin-store` es su
//! misma capa y no puede verlo): recibe el contenido ya cargado y avisa por
//! callback de lo que el usuario pide. Aqui vive ademas el calculo del
//! tamano con el que nace cada tipo (D32), puro salvo la medicion del texto,
//! que llega inyectada para poder probarlo sin DirectWrite.

use std::path::{Path, PathBuf};

use pixpin_codec::ImagenRgba;

/// Ficha de archivo: alto fijo, ancho por defecto (px logicos, D32).
pub const FICHA_ANCHO_LOGICO: u32 = 280;
pub const FICHA_ALTO_LOGICO: u32 = 72;
/// La franja con el nombre bajo la miniatura de un documento (D71).
pub const DOCUMENTO_FRANJA_LOGICA: u32 = 28;
/// Tamano de un video mientras no se conocen sus metadatos (px logicos).
pub const VIDEO_ANCHO_PROVISIONAL_LOGICO: u32 = 480;
pub const VIDEO_ALTO_PROVISIONAL_LOGICO: u32 = 270;
/// Tope de la nota al nacer (px logicos): mas alla se recorta y se
/// redimensiona a mano.
pub const NOTA_ANCHO_MAX_LOGICO: u32 = 480;
pub const NOTA_ALTO_MAX_LOGICO: u32 = 640;
/// Margen interior de la nota y cuerpo del texto, en px logicos.
pub const NOTA_MARGEN_LOGICO: f32 = 12.0;
pub const NOTA_TEXTO_LOGICO: f32 = 14.0;

/// Lo que el pin dibuja dentro de su tarjeta.
pub enum Contenido {
    Imagen(ImagenRgba),
    /// Solo lectura en v1: se lee, se mueve, se agrupa y se copia. La
    /// edicion con cursor llega con el canvas (spec 3.2).
    Nota {
        texto: String,
    },
    Archivo {
        nombre: String,
        /// Tamano formateado o el aviso de que la ruta ya no existe; llega
        /// traducido, el pin no conoce el catalogo.
        detalle: String,
        icono: Option<ImagenRgba>,
        existe: bool,
    },
    /// Un video por referencia (D63). El reproductor vive en la ventana, no
    /// aqui: este contenido es `Clone` y un reproductor no. `ancho`/`alto`
    /// son los nativos si ya se conocen; si no, la proporcion de la
    /// miniatura o el tamano provisional.
    Video {
        nombre: String,
        ruta: PathBuf,
        ancho: u32,
        alto: u32,
    },
    /// La miniatura que la Shell da de un archivo, con el nombre debajo
    /// (D62). Por referencia, como la ficha.
    Documento {
        nombre: String,
        vista: ImagenRgba,
    },
}

/// Como se ensena un archivo por referencia, decidido SOLO por su extension
/// (D65): sin tocar disco, puro y probado. Lo que no es video es candidato a
/// documento, y quien llama comprueba si la Shell tiene miniatura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presentacion {
    Video,
    DocumentoSiHayMiniatura,
}

/// Extensiones que Media Foundation abre como video en un Windows 10 sin
/// codecs anadidos. Lo que falte aqui se ensena como documento (miniatura),
/// que es una degradacion visible y no un fallo.
const EXTENSIONES_VIDEO: [&str; 10] = [
    "mp4", "mkv", "webm", "mov", "avi", "m4v", "ts", "wmv", "mpg", "mpeg",
];

pub fn presentacion_de(ruta: &Path) -> Presentacion {
    let ext = ruta
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if EXTENSIONES_VIDEO.contains(&ext.as_str()) {
        Presentacion::Video
    } else {
        Presentacion::DocumentoSiHayMiniatura
    }
}

impl Contenido {
    /// Solo la ficha se redimensiona nada mas que a lo ancho (spec 4.1): su
    /// alto lo manda el contenido, no el raton.
    pub fn solo_ancho(&self) -> bool {
        matches!(self, Contenido::Archivo { .. })
    }

    /// La imagen nativa, para el 100 % del doble clic. La nota y la ficha no
    /// tienen "tamano original" de pixeles.
    pub fn imagen(&self) -> Option<&ImagenRgba> {
        match self {
            Contenido::Imagen(i) => Some(i),
            _ => None,
        }
    }
}

/// Tamano con el que nace el contenido, en pixeles FISICOS (D32).
///
/// `medidor` recibe (texto, cuerpo de la fuente, ancho maximo de linea) y
/// devuelve el (ancho, alto) que ocupa YA AJUSTADO a ese ancho, en pixeles
/// fisicos. El ancho maximo es parte de la pregunta y no un recorte
/// posterior: sin el, una nota de una sola linea larguisima daria un alto de
/// una linea y un ancho recortado, y el texto saldria cortado en la tarjeta.
/// En produccion lo aporta DirectWrite; en los tests, una funcion de mentira.
pub fn tamano_natural(
    contenido: &Contenido,
    escala_por_cien: u32,
    medidor: &dyn Fn(&str, f32, f32) -> (f32, f32),
) -> (u32, u32) {
    let escala = escala_por_cien as f32 / 100.0;
    let fis = |logico: u32| ((logico as f32) * escala).round() as u32;

    match contenido {
        // 1:1 en pixeles fisicos: la captura se ve exactamente como estaba
        // en pantalla (D26/3.2).
        Contenido::Imagen(img) => (img.ancho.max(1), img.alto.max(1)),

        Contenido::Nota { texto } => {
            let margen = NOTA_MARGEN_LOGICO * escala;
            let ancho_texto = fis(NOTA_ANCHO_MAX_LOGICO) as f32 - 2.0 * margen;
            let (tw, th) = medidor(texto, NOTA_TEXTO_LOGICO * escala, ancho_texto);
            let ancho = (tw + 2.0 * margen).round() as u32;
            let alto = (th + 2.0 * margen).round() as u32;
            (
                ancho.clamp(fis(80), fis(NOTA_ANCHO_MAX_LOGICO)),
                alto.clamp(fis(40), fis(NOTA_ALTO_MAX_LOGICO)),
            )
        }

        Contenido::Archivo { .. } => (fis(FICHA_ANCHO_LOGICO), fis(FICHA_ALTO_LOGICO)),

        // Como una imagen: el video es una imagen en movimiento (D71). Sin
        // tamano conocido, el provisional.
        Contenido::Video { ancho, alto, .. } => {
            if *ancho == 0 || *alto == 0 {
                (
                    fis(VIDEO_ANCHO_PROVISIONAL_LOGICO),
                    fis(VIDEO_ALTO_PROVISIONAL_LOGICO),
                )
            } else {
                (*ancho, *alto)
            }
        }

        // La miniatura tal cual la dio la Shell mas la franja del nombre.
        Contenido::Documento { vista, .. } => (
            vista.ancho.max(1),
            vista.alto.max(1) + fis(DOCUMENTO_FRANJA_LOGICA),
        ),
    }
}

/// Tamano de un fichero en unidades humanas. Vive aqui porque la ficha es
/// quien lo enseña, y asi se prueba sin tocar disco.
pub fn tamano_humano(bytes: u64) -> String {
    const UNIDADES: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut valor = bytes as f64;
    let mut unidad = 0;
    while valor >= 1024.0 && unidad + 1 < UNIDADES.len() {
        valor /= 1024.0;
        unidad += 1;
    }
    if unidad == 0 {
        format!("{bytes} B")
    } else if valor >= 10.0 {
        format!("{valor:.0} {}", UNIDADES[unidad])
    } else {
        format!("{valor:.1} {}", UNIDADES[unidad])
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Medidor de mentira: cada caracter ocupa medio cuerpo de ancho y el
    /// texto se parte al llegar al ancho maximo. Determinista, que es lo
    /// unico que se le pide.
    fn medidor(texto: &str, tam: f32, ancho_max: f32) -> (f32, f32) {
        let ancho_char = tam * 0.5;
        let por_linea = (ancho_max / ancho_char).floor().max(1.0);
        let n = texto.chars().count() as f32;
        let lineas = (n / por_linea).ceil().max(1.0);
        (n.min(por_linea) * ancho_char, lineas * tam * 1.3)
    }

    fn ficha() -> Contenido {
        Contenido::Archivo {
            nombre: "informe.pdf".into(),
            detalle: "1,2 MB".into(),
            icono: None,
            existe: true,
        }
    }

    #[test]
    fn la_ficha_nace_con_su_tamano_fijo_y_escala_con_el_dpi() {
        assert_eq!(tamano_natural(&ficha(), 100, &medidor), (280, 72));
        assert_eq!(
            tamano_natural(&ficha(), 150, &medidor),
            (420, 108),
            "a 150 % el mismo tamano logico son mas pixeles fisicos"
        );
    }

    #[test]
    fn la_ficha_solo_se_redimensiona_a_lo_ancho() {
        assert!(ficha().solo_ancho());
        // Caso negativo: la imagen y la nota si escalan en los dos ejes.
        assert!(!Contenido::Nota { texto: "x".into() }.solo_ancho());
    }

    #[test]
    fn una_imagen_nace_1_a_1_sin_que_el_dpi_la_toque() {
        // D26: el recorte queda EXACTAMENTE como estaba en pantalla. Aplicar
        // la escala aqui lo agrandaria y romperia el gesto insignia.
        let img = Contenido::Imagen(ImagenRgba {
            ancho: 600,
            alto: 450,
            pixeles: vec![0; 600 * 450 * 4],
        });
        assert_eq!(tamano_natural(&img, 150, &medidor), (600, 450));
    }

    #[test]
    fn una_nota_corta_se_ajusta_a_su_texto() {
        let nota = Contenido::Nota {
            texto: "hola".into(),
        };
        let (w, h) = tamano_natural(&nota, 100, &medidor);
        assert!(
            w < 480 && h < 640,
            "una nota corta no ocupa el maximo: {w}x{h}"
        );
        assert!(w >= 80 && h >= 40, "ni baja del minimo legible: {w}x{h}");
    }

    #[test]
    fn una_nota_kilometrica_se_queda_en_el_tope() {
        // Sin el tope, pegar un log de 3 MB crearia una ventana mas alta que
        // el escritorio y el pin naceria inmanejable.
        let nota = Contenido::Nota {
            texto: "palabra ".repeat(5000),
        };
        let (w, h) = tamano_natural(&nota, 100, &medidor);
        assert_eq!(h, 640, "el alto se corta en el tope, pase lo que pase");
        assert!(
            (470..=480).contains(&w),
            "el ancho llega al tope salvo el resto de un caracter: {w}"
        );
    }

    #[test]
    fn un_mp4_y_un_mkv_son_video_sin_importar_las_mayusculas() {
        for r in ["a.mp4", "b.MKV", "c.webm", "d.ts", r"C:\v\e.Mov"] {
            assert_eq!(presentacion_de(Path::new(r)), Presentacion::Video, "{r}");
        }
    }

    #[test]
    fn un_pdf_una_carpeta_y_sin_extension_no_son_video() {
        // Caso negativo: todo lo demas es candidato a documento; la Shell
        // decide si hay miniatura.
        for r in [
            "informe.pdf",
            r"C:\carpeta",
            "sinextension",
            "foto.png",
            "notas.txt",
        ] {
            assert_eq!(
                presentacion_de(Path::new(r)),
                Presentacion::DocumentoSiHayMiniatura,
                "{r}"
            );
        }
    }

    #[test]
    fn el_documento_nace_con_su_miniatura_mas_la_franja() {
        let c = Contenido::Documento {
            nombre: "informe.pdf".into(),
            vista: ImagenRgba {
                ancho: 400,
                alto: 300,
                pixeles: vec![0; 400 * 300 * 4],
            },
        };
        // Al 150 %, la franja de 28 px logicos son 42 fisicos.
        assert_eq!(tamano_natural(&c, 150, &medidor), (400, 342));
    }

    #[test]
    fn el_video_nace_con_su_tamano_nativo_o_el_provisional() {
        let con = Contenido::Video {
            nombre: "v.mp4".into(),
            ruta: PathBuf::from("v.mp4"),
            ancho: 1920,
            alto: 1080,
        };
        assert_eq!(tamano_natural(&con, 150, &medidor), (1920, 1080));
        let sin = Contenido::Video {
            nombre: "v.mp4".into(),
            ruta: PathBuf::from("v.mp4"),
            ancho: 0,
            alto: 0,
        };
        assert_eq!(tamano_natural(&sin, 100, &medidor), (480, 270));
    }

    #[test]
    fn el_tamano_humano_no_miente_en_los_bordes() {
        assert_eq!(tamano_humano(0), "0 B");
        assert_eq!(tamano_humano(999), "999 B");
        assert_eq!(tamano_humano(1024), "1.0 KB");
        assert_eq!(tamano_humano(1536), "1.5 KB");
        assert_eq!(tamano_humano(10 * 1024), "10 KB");
        assert_eq!(tamano_humano(1024 * 1024), "1.0 MB");
        assert_eq!(tamano_humano(3 * 1024 * 1024 * 1024), "3.0 GB");
    }
}
