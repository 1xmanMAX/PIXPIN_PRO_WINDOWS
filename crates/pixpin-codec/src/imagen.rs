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
    /// Si NINGUN pixel es translucido. Una captura de pantalla siempre lo
    /// es, y saberlo deja al pin ahorrarse pintar la tarjeta de debajo: es
    /// un relleno del tamano entero del pin en cada fotograma. Se responde
    /// una vez, al crear el pin, y se corta en cuanto encuentra un alfa
    /// distinto de 255.
    pub fn es_opaca(&self) -> bool {
        self.pixeles.len() >= 4 && self.pixeles.iter().skip(3).step_by(4).all(|a| *a == 255)
    }

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
    #[error("no se pudo leer {ruta}: {fuente}")]
    Lectura {
        ruta: std::path::PathBuf,
        #[source]
        fuente: image::ImageError,
    },
}

/// Lee una imagen del disco a RGBA. La pareja de `guardar`.
pub fn cargar(ruta: &Path) -> Result<ImagenRgba, ErrorCodec> {
    let dinamica = lector(ruta)
        .and_then(|l| l.decode())
        .map_err(|fuente| ErrorCodec::Lectura {
            ruta: ruta.to_path_buf(),
            fuente,
        })?;
    let rgba = dinamica.to_rgba8();
    Ok(ImagenRgba {
        ancho: rgba.width(),
        alto: rgba.height(),
        pixeles: rgba.into_raw(),
    })
}

/// Cuanto mide una imagen, SIN descomprimirla: solo su cabecera.
///
/// Lo quiere el chat para cada foto que ensena: los trazos que se dibujan
/// encima van en coordenadas de la foto, asi que hay que saber su tamano
/// para colocarlos. Cargarlas enteras para eso seria descomprimir doce
/// megapixeles por burbuja.
pub fn medidas(ruta: &Path) -> Result<(u32, u32), ErrorCodec> {
    lector(ruta)
        .and_then(|l| l.into_dimensions())
        .map_err(|fuente| ErrorCodec::Lectura {
            ruta: ruta.to_path_buf(),
            fuente,
        })
}

/// Abre el fichero y decide que formato es MIRANDO DENTRO, no por el nombre.
///
/// El movil guarda las fotos de un proyecto como `imagenes/<id>`, sin
/// extension. Por el nombre, todas eran «formato desconocido»: ni vista
/// previa en el chat, ni fondo en el lienzo, ni pin de imagen.
fn lector(
    ruta: &Path,
) -> Result<image::ImageReader<std::io::BufReader<std::fs::File>>, image::ImageError> {
    Ok(image::ImageReader::open(ruta)?.with_guessed_format()?)
}

/// PNG en memoria: para el almacen, que guarda bytes, no rutas.
pub fn codificar_png(imagen: &ImagenRgba) -> Result<Vec<u8>, ErrorCodec> {
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
    let mut salida = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(buffer)
        .write_to(&mut salida, image::ImageFormat::Png)
        .map_err(|fuente| ErrorCodec::Escritura {
            ruta: std::path::PathBuf::from("<memoria>"),
            fuente,
        })?;
    Ok(salida.into_inner())
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

/// Premultiplica RGB por alfa (en el mismo buffer straight-alpha que usa
/// `ImagenRgba`), SOLO para pasarselo al filtro de `image::imageops::resize`.
///
/// `resize` promedia cada canal por separado. Con alfa recto, un pixel
/// transparente NEGRO junto a uno opaco BLANCO promedia el negro y el blanco
/// del RGB sin pesar por cuanto pesa cada uno en el resultado -sale una
/// franja gris a medio camino, visible aunque el resultado sea semitraslucido
/// y el negro de detras "no deberia" contar. Premultiplicando antes, el RGB
/// de un pixel con alfa 0 ya es (0,0,0): un pixel completamente transparente
/// deja de tener voto en el promedio de color, que es lo que se espera de
/// "invisible". Se deshace con `despremultiplicar` tras `resize`.
fn premultiplicar_para_resize(pixeles: &[u8]) -> Vec<u8> {
    let mut v = pixeles.to_vec();
    for p in v.chunks_exact_mut(4) {
        let a = p[3] as u32;
        for c in &mut p[0..3] {
            *c = ((*c as u32 * a + 127) / 255) as u8;
        }
    }
    v
}

/// Deshace `premultiplicar_para_resize` sobre el resultado YA reducido de
/// `resize`, in-place: vuelve a alfa recto para que `ImagenRgba` siga
/// cumpliendo su contrato (straight alpha, ver el comentario del modulo).
fn despremultiplicar_de_resize(pixeles: &mut [u8]) {
    for p in pixeles.chunks_exact_mut(4) {
        let a = p[3] as u32;
        for c in &mut p[0..3] {
            // `checked_div` en vez de comprobar `a == 0` a mano (clippy pide
            // esto: `manual_checked_ops`). Sin alfa no hay color que
            // recuperar -el original se perdio al premultiplicar, 0 por
            // cualquier cosa da 0-, asi que se deja en negro en vez de
            // dividir entre cero.
            *c = (*c as u32 * 255 + a / 2)
                .checked_div(a)
                .map(|v| v.min(255))
                .unwrap_or(0) as u8;
        }
    }
}

/// La imagen a otro tamano, con filtro triangular (suave y rapido). Para el
/// fondo del lienzo cuando la GPU no admite su tamano (D139). Consume la
/// imagen para no duplicar sus bytes mientras se reduce.
pub fn redimensionar(imagen: ImagenRgba, ancho: u32, alto: u32) -> Result<ImagenRgba, ErrorCodec> {
    if imagen.ancho == 0 || imagen.alto == 0 || ancho == 0 || alto == 0 {
        return Err(ErrorCodec::Vacia { ancho, alto });
    }
    let (a0, h0) = (imagen.ancho, imagen.alto);
    let espera = imagen.bytes_esperados();
    let tiene = imagen.pixeles.len();
    let incoherente = ErrorCodec::TamanoIncoherente {
        ancho: a0,
        alto: h0,
        tiene,
        espera,
    };
    // `from_raw` acepta un buffer MAS largo de lo necesario: se exige el
    // tamano justo aqui, como `validar_tamano_rgba` en el render.
    if tiene != espera {
        return Err(incoherente);
    }
    // Premultiplicar ANTES de construir la imagen de `image`: es el unico
    // buffer que se le pasa a `resize` (D139/fix ronda 1: sin esto, un
    // recorte con transparencia sangraba el color de detras en el borde).
    let premultiplicada = premultiplicar_para_resize(&imagen.pixeles);
    let origen = image::RgbaImage::from_raw(a0, h0, premultiplicada).ok_or(incoherente)?;
    let r = image::imageops::resize(&origen, ancho, alto, image::imageops::FilterType::Triangle);
    let mut pixeles = r.into_raw();
    despremultiplicar_de_resize(&mut pixeles);
    Ok(ImagenRgba {
        ancho,
        alto,
        pixeles,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::fs;

    #[test]
    fn una_imagen_es_opaca_solo_si_todos_sus_alfas_estan_al_maximo() {
        let opaca = ImagenRgba {
            ancho: 2,
            alto: 1,
            pixeles: vec![1, 2, 3, 255, 4, 5, 6, 255],
        };
        assert!(opaca.es_opaca());
        // Caso negativo: un solo pixel translucido obliga a pintar la
        // tarjeta de debajo, o se veria el escritorio a traves.
        let con_alfa = ImagenRgba {
            ancho: 2,
            alto: 1,
            pixeles: vec![1, 2, 3, 255, 4, 5, 6, 254],
        };
        assert!(!con_alfa.es_opaca());
        // Y una imagen vacia no se considera opaca: no tapa nada.
        assert!(
            !ImagenRgba {
                ancho: 0,
                alto: 0,
                pixeles: Vec::new(),
            }
            .es_opaca()
        );
    }

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
    fn cargar_devuelve_lo_guardado() {
        let dir = temporal("cargar");
        let ruta = dir.join("ida-vuelta.png");
        let original = imagen_de_prueba();
        guardar(&original, &ruta, FormatoImagen::Png).unwrap();
        let leida = cargar(&ruta).unwrap();
        assert_eq!(leida, original, "PNG es sin perdida: la vuelta es identica");
    }

    #[test]
    fn cargar_una_ruta_inexistente_da_error_con_la_ruta() {
        let e = cargar(std::path::Path::new("Z:/no/existe.png")).unwrap_err();
        assert!(
            e.to_string().contains("existe.png"),
            "el error debe decir cual: {e}"
        );
    }

    #[test]
    fn codificar_png_en_memoria_equivale_a_guardar() {
        let original = imagen_de_prueba();
        let en_memoria = codificar_png(&original).unwrap();
        // No se exige igualdad byte a byte con el fichero (el encoder puede
        // variar entre rutas), sino la ida y vuelta: decodificar lo
        // codificado devuelve la imagen exacta.
        let vuelta = image::load_from_memory(&en_memoria).unwrap().to_rgba8();
        assert_eq!(vuelta.as_raw(), &original.pixeles);
    }

    #[test]
    fn codificar_una_imagen_vacia_da_error() {
        let vacia = ImagenRgba {
            ancho: 0,
            alto: 0,
            pixeles: vec![],
        };
        assert!(codificar_png(&vacia).is_err());
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

    #[test]
    fn redimensionar_da_el_tamano_pedido_con_sus_bytes() {
        let img = ImagenRgba {
            ancho: 4,
            alto: 2,
            pixeles: [10u8, 20, 30, 255].repeat(8),
        };
        let r = redimensionar(img, 2, 1).unwrap();
        assert_eq!((r.ancho, r.alto), (2, 1));
        assert_eq!(r.pixeles.len(), r.bytes_esperados());
        assert_eq!(
            &r.pixeles[0..4],
            &[10, 20, 30, 255],
            "un color liso sigue liso"
        );
    }

    #[test]
    fn redimensionar_a_cero_o_con_bytes_de_menos_falla() {
        let buena = || ImagenRgba {
            ancho: 2,
            alto: 2,
            pixeles: vec![0; 16],
        };
        assert!(redimensionar(buena(), 0, 5).is_err());
        let corta = ImagenRgba {
            ancho: 2,
            alto: 2,
            pixeles: vec![0; 15],
        };
        assert!(redimensionar(corta, 1, 1).is_err());
    }

    #[test]
    fn redimensionar_no_deja_que_lo_transparente_manche_el_color_del_vecino() {
        // Fix ronda 1 (D139): blanco opaco, blanco opaco, negro transparente,
        // negro transparente -> a 2x1 el filtro triangular de `image` no
        // hace una media de caja exacta de dos en dos (su soporte se
        // ensancha al reducir, asi que cada mitad se contamina un poco de
        // alfa de la otra: 219 y 36 de verdad, no 255 y 0 limpios). Eso es
        // normal en un downscale continuo y no es lo que este test vigila:
        // lo que vigila es que el COLOR nunca se contamine con el negro
        // invisible, que es justo lo que arregla premultiplicar antes de
        // llamar a `resize`.
        let img = ImagenRgba {
            ancho: 4,
            alto: 1,
            pixeles: vec![
                255, 255, 255, 255, //
                255, 255, 255, 255, //
                0, 0, 0, 0, //
                0, 0, 0, 0,
            ],
        };
        let r = redimensionar(img, 2, 1).unwrap();
        assert_eq!(&r.pixeles[0..3], &[255, 255, 255], "mitad blanca: color");
        assert!(r.pixeles[3] > 200, "mitad blanca: alfa alto, no 128 gris");
        assert_eq!(
            &r.pixeles[4..7],
            &[255, 255, 255],
            "mitad transparente: color sigue blanco, no gris"
        );
        assert!(r.pixeles[7] < 50, "mitad transparente: alfa bajo");

        // El caso que de verdad delata el fallo: blanco opaco junto a negro
        // TOTALMENTE transparente, reducidos a un solo pixel. El alfa final
        // (~128) es la media de 255 y 0, pero el color no puede irse a gris
        // (~128,128,128): el negro no pesa nada mientras es invisible, asi
        // que el color tiene que seguir siendo blanco.
        let mitad = ImagenRgba {
            ancho: 2,
            alto: 1,
            pixeles: vec![
                255, 255, 255, 255, //
                0, 0, 0, 0,
            ],
        };
        let r = redimensionar(mitad, 1, 1).unwrap();
        assert!(
            (r.pixeles[3] as i32 - 128).abs() <= 1,
            "alfa a medias: {:?}",
            r.pixeles
        );
        for c in &r.pixeles[0..3] {
            assert!(
                *c >= 250,
                "el color no puede haberse ido a gris: {:?}",
                r.pixeles
            );
        }
    }
}
