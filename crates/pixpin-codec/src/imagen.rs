//! Imagen en memoria de sistema y su codificacion a disco.
//!
//! Este es el **unico** punto donde la imagen existe fuera de la GPU. Todo lo
//! demas —captura, recorte, y en S1-B2 el dibujo y los efectos— ocurre en
//! texturas. Una captura 4K son 33 MB: bajarla en cada operacion es lo que
//! hace lentas y glotonas a las herramientas de captura corrientes.

use std::path::Path;

/// Pixeles RGBA de 8 bits por canal, en filas contiguas y **sin relleno**.
///
/// El relleno se quita al bajar de la GPU, en `pixpin_capture::a_imagen`, no
/// aqui: quien construya una `ImagenRgba` ya entrega filas compactas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagenRgba {
    pub ancho: u32,
    pub alto: u32,
    pub pixeles: Vec<u8>,
}

impl ImagenRgba {
    /// Bytes que deberia tener `pixeles` para ser coherente con las medidas.
    pub fn bytes_esperados(&self) -> usize {
        self.ancho as usize * self.alto as usize * 4
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatoImagen {
    Png,
    Jpg,
    Webp,
}

impl FormatoImagen {
    /// Formato a partir de una extension, sin distinguir mayusculas.
    pub fn por_extension(ext: &str) -> Option<FormatoImagen> {
        match ext.to_ascii_lowercase().as_str() {
            "png" => Some(FormatoImagen::Png),
            "jpg" | "jpeg" => Some(FormatoImagen::Jpg),
            "webp" => Some(FormatoImagen::Webp),
            _ => None,
        }
    }

    fn a_image(self) -> image::ImageFormat {
        match self {
            FormatoImagen::Png => image::ImageFormat::Png,
            FormatoImagen::Jpg => image::ImageFormat::Jpeg,
            FormatoImagen::Webp => image::ImageFormat::WebP,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorCodec {
    #[error("la imagen esta vacia ({ancho}x{alto})")]
    Vacia { ancho: u32, alto: u32 },
    #[error("el buffer tiene {tiene} bytes pero {ancho}x{alto} necesita {espera}")]
    TamanoIncoherente {
        ancho: u32,
        alto: u32,
        tiene: usize,
        espera: usize,
    },
    #[error("no se pudo escribir {ruta}: {fuente}")]
    Escritura {
        ruta: std::path::PathBuf,
        #[source]
        fuente: image::ImageError,
    },
    #[error("no se pudo escribir en el portapapeles de Windows")]
    Portapapeles,
}

/// Escribe la imagen a disco en el formato indicado.
pub fn guardar(imagen: &ImagenRgba, ruta: &Path, formato: FormatoImagen) -> Result<(), ErrorCodec> {
    // Las dos comprobaciones existen para no escribir un fichero corrupto que
    // el usuario creeria que es su captura.
    if imagen.ancho == 0 || imagen.alto == 0 {
        return Err(ErrorCodec::Vacia {
            ancho: imagen.ancho,
            alto: imagen.alto,
        });
    }
    let espera = imagen.bytes_esperados();
    if imagen.pixeles.len() != espera {
        return Err(ErrorCodec::TamanoIncoherente {
            ancho: imagen.ancho,
            alto: imagen.alto,
            tiene: imagen.pixeles.len(),
            espera,
        });
    }

    let buffer: image::RgbaImage =
        image::ImageBuffer::from_raw(imagen.ancho, imagen.alto, imagen.pixeles.clone())
            .expect("el tamano se acaba de comprobar");

    let dinamica = image::DynamicImage::ImageRgba8(buffer);
    // JPEG no tiene canal alfa; convertir aqui evita que `image` decida por
    // su cuenta y produzca un resultado distinto segun la version.
    let dinamica = match formato {
        FormatoImagen::Jpg => image::DynamicImage::ImageRgb8(dinamica.to_rgb8()),
        _ => dinamica,
    };

    dinamica
        .save_with_format(ruta, formato.a_image())
        .map_err(|fuente| ErrorCodec::Escritura {
            ruta: ruta.to_path_buf(),
            fuente,
        })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::fs;

    fn temporal(etiqueta: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("pixpin-codec-{etiqueta}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Cuadro 2x2: rojo, verde, azul, blanco. Opacos.
    fn imagen_de_prueba() -> ImagenRgba {
        ImagenRgba {
            ancho: 2,
            alto: 2,
            pixeles: vec![
                255, 0, 0, 255, //
                0, 255, 0, 255, //
                0, 0, 255, 255, //
                255, 255, 255, 255,
            ],
        }
    }

    #[test]
    fn png_conserva_los_pixeles_exactos() {
        // PNG es sin perdida: la ida y vuelta debe ser identica byte a byte.
        // Si alguien cambiara el orden de canales, este test lo cazaria.
        let dir = temporal("png");
        let ruta = dir.join("prueba.png");
        let original = imagen_de_prueba();

        guardar(&original, &ruta, FormatoImagen::Png).unwrap();

        let leida = image::open(&ruta).unwrap().to_rgba8();
        assert_eq!(leida.width(), 2);
        assert_eq!(leida.height(), 2);
        assert_eq!(leida.as_raw(), &original.pixeles);
    }

    #[test]
    fn jpg_y_webp_producen_ficheros_legibles_del_tamano_correcto() {
        // Con perdida, asi que no se comparan pixeles: solo que el fichero
        // existe, no esta vacio y se puede volver a abrir con las medidas
        // correctas.
        let dir = temporal("perdida");
        for (formato, nombre) in [
            (FormatoImagen::Jpg, "prueba.jpg"),
            (FormatoImagen::Webp, "prueba.webp"),
        ] {
            let ruta = dir.join(nombre);
            guardar(&imagen_de_prueba(), &ruta, formato).unwrap();

            assert!(
                fs::metadata(&ruta).unwrap().len() > 0,
                "{nombre} quedo vacio"
            );
            let leida = image::open(&ruta).unwrap();
            assert_eq!(
                (leida.width(), leida.height()),
                (2, 2),
                "{nombre} con medidas raras"
            );
        }
    }

    #[test]
    fn una_imagen_vacia_da_error_en_vez_de_escribir_basura() {
        // Caso negativo: sin esta comprobacion se escribiria un fichero
        // corrupto de 0x0 que ningun visor abre, y el usuario creeria que su
        // captura se guardo.
        let dir = temporal("vacia");
        let vacia = ImagenRgba {
            ancho: 0,
            alto: 0,
            pixeles: vec![],
        };
        assert!(guardar(&vacia, &dir.join("x.png"), FormatoImagen::Png).is_err());
    }

    #[test]
    fn un_buffer_con_tamano_incoherente_da_error() {
        // Otro caso negativo: declara 2x2 (16 bytes) pero trae 4. Sin la
        // comprobacion, `image` entraria en panico o leeria fuera de rango.
        let dir = temporal("incoherente");
        let mala = ImagenRgba {
            ancho: 2,
            alto: 2,
            pixeles: vec![0, 0, 0, 0],
        };
        assert!(guardar(&mala, &dir.join("x.png"), FormatoImagen::Png).is_err());
    }

    #[test]
    fn el_formato_se_deduce_de_la_extension() {
        assert_eq!(
            FormatoImagen::por_extension("PNG"),
            Some(FormatoImagen::Png)
        );
        assert_eq!(
            FormatoImagen::por_extension("jpg"),
            Some(FormatoImagen::Jpg)
        );
        assert_eq!(
            FormatoImagen::por_extension("jpeg"),
            Some(FormatoImagen::Jpg)
        );
        assert_eq!(
            FormatoImagen::por_extension("webp"),
            Some(FormatoImagen::Webp)
        );
        assert_eq!(FormatoImagen::por_extension("bmp"), None);
        assert_eq!(FormatoImagen::por_extension(""), None);
    }
}
