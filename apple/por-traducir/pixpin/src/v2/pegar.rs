//! **Pegar imagenes del portapapeles** (Ctrl+V en una caja): un mapa de
//! bits copiado (una captura, una imagen del navegador) o ficheros copiados
//! en el Explorador.
//!
//! Lo hacian por separado `tareas/ventana.rs` (`Campo::pegar`, con huella
//! para no meter la misma imagen dos veces) y `timeline/ventana.rs`
//! (`pegar`, sin huella). Aqui esta lo de tareas, que es lo completo.
//!
//! NOTA: tareas y timeline deben pasarse a este modulo (sus `png_temporal`,
//! `es_imagen` y `EXTENSIONES_DE_IMAGEN` son copias de lo de aqui). No se
//! tocan ahora porque otro agente esta editando timeline.

#![forbid(unsafe_code)]
// Todo lo de este fichero: lo adoptan tareas y timeline en la siguiente tanda.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use pixpin_codec::ContenidoPortapapeles;

/// Las extensiones que se aceptan como imagen: las que el movil pinta como
/// imagen (`Markdown.claseDeMedio`) y el codec de aqui lee. Copia de
/// `tareas::EXTENSIONES_DE_IMAGEN`.
pub const EXTENSIONES_DE_IMAGEN: [&str; 6] = ["png", "jpg", "jpeg", "bmp", "gif", "webp"];

/// Si un fichero es una imagen de las que se pegan, por su extension.
pub fn es_imagen(ruta: &Path) -> bool {
    ruta.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONES_DE_IMAGEN.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

/// Una imagen pegada, esperando a que la caja la guarde de verdad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pegada {
    pub ruta: PathBuf,
    /// Un PNG escrito al pegar un mapa de bits: es nuestro y se borra al
    /// vaciar la caja. Un fichero copiado del Explorador no es nuestro y se
    /// deja.
    pub temporal: bool,
    /// La huella de su contenido (`pixpin_lanzador::imagenes::huella`): la
    /// misma imagen copiada dos veces es la misma huella, aunque el numero
    /// de secuencia del portapapeles cambie.
    pub huella: u64,
}

impl Pegada {
    /// Borra su PNG si es temporal. Lo de fuera no se toca.
    pub fn soltar(&self) {
        if self.temporal {
            let _ = std::fs::remove_file(&self.ruta);
        }
    }
}

/// La huella de un mapa de bits: sus medidas y sus pixeles (dos imagenes
/// con los mismos bytes y distinta forma no son la misma).
pub fn huella_de_mapa(img: &pixpin_codec::ImagenRgba) -> u64 {
    let mut bytes = Vec::with_capacity(img.pixeles.len() + 8);
    bytes.extend_from_slice(&img.ancho.to_le_bytes());
    bytes.extend_from_slice(&img.alto.to_le_bytes());
    bytes.extend_from_slice(&img.pixeles);
    pixpin_lanzador::imagenes::huella(&bytes)
}

/// La huella de un fichero: por su contenido; si no se puede leer, por su
/// ruta (sigue sirviendo para no meter dos veces el mismo).
pub fn huella_de_fichero(ruta: &Path) -> u64 {
    std::fs::read(ruta)
        .map(|b| pixpin_lanzador::imagenes::huella(&b))
        .unwrap_or_else(|_| pixpin_lanzador::imagenes::huella(ruta.to_string_lossy().as_bytes()))
}

/// Guarda un mapa de bits pegado en un PNG nuevo de `carpeta`. El nombre
/// lleva el proceso, la hora y un contador: dos pegadas en el mismo
/// milisegundo no se pisan.
pub fn png_temporal(img: &pixpin_codec::ImagenRgba, carpeta: &Path) -> anyhow::Result<PathBuf> {
    static CUENTA: AtomicU32 = AtomicU32::new(0);
    std::fs::create_dir_all(carpeta)?;
    let n = CUENTA.fetch_add(1, Ordering::Relaxed);
    let ruta = carpeta.join(format!(
        "pegada-{}-{}-{n}.png",
        std::process::id(),
        pixpin_shell::entorno::ahora_utc_ms()
    ));
    pixpin_codec::guardar(img, &ruta, pixpin_codec::FormatoImagen::Png)?;
    Ok(ruta)
}

/// Las imagenes de un contenido del portapapeles: el mapa de bits a un PNG
/// de `carpeta`; de los ficheros, solo las imagenes. El texto no es asunto
/// de esto: lo pega cada caja con su `ui::Campo`.
pub fn pegadas_de(contenido: Option<ContenidoPortapapeles>, carpeta: &Path) -> Vec<Pegada> {
    match contenido {
        Some(ContenidoPortapapeles::Imagen(img)) if img.ancho > 0 && img.alto > 0 => {
            let huella = huella_de_mapa(&img);
            match png_temporal(&img, carpeta) {
                Ok(ruta) => vec![Pegada {
                    ruta,
                    temporal: true,
                    huella,
                }],
                Err(e) => {
                    tracing::warn!(?e, "la imagen pegada no se pudo guardar");
                    Vec::new()
                }
            }
        }
        Some(ContenidoPortapapeles::Rutas(rutas)) => rutas
            .into_iter()
            .filter(|r| es_imagen(r))
            .map(|ruta| Pegada {
                huella: huella_de_fichero(&ruta),
                ruta,
                temporal: false,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// **Lee las imagenes del portapapeles** ahora. Los mapas de bits van a
/// `%TEMP%\pixpin-pegadas`. Vacio si no hay ninguna imagen (o solo texto).
pub fn leer_portapapeles() -> Vec<Pegada> {
    pegadas_de(
        pixpin_codec::portapapeles::leer(),
        &std::env::temp_dir().join("pixpin-pegadas"),
    )
}

/// **Una imagen, una vez**: quita de `nuevas` las que ya estan en `ya` (o
/// repetidas entre ellas) por su huella, y borra el PNG temporal de las que
/// sobran. Devuelve las que quedan y cuantas se quitaron (para avisar «ya
/// estaba»).
pub fn sin_repetidas(ya: &[Pegada], nuevas: Vec<Pegada>) -> (Vec<Pegada>, usize) {
    let mut vistas: std::collections::HashSet<u64> = ya.iter().map(|p| p.huella).collect();
    let mut quedan = Vec::new();
    let mut quitadas = 0;
    for p in nuevas {
        if vistas.insert(p.huella) {
            quedan.push(p);
        } else {
            p.soltar();
            quitadas += 1;
        }
    }
    (quedan, quitadas)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn carpeta(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-v2-pegar-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn mapa(color: u8) -> pixpin_codec::ImagenRgba {
        pixpin_codec::ImagenRgba {
            ancho: 4,
            alto: 2,
            pixeles: [color, 0, 0, 255].repeat(8),
        }
    }

    #[test]
    fn es_imagen_mira_la_extension_sin_mayusculas() {
        assert!(es_imagen(Path::new("a/foto.PNG")));
        assert!(es_imagen(Path::new("a/b.webp")));
        // Caso negativo: un video o un texto no se pegan como imagen.
        assert!(!es_imagen(Path::new("a/clip.mp4")));
        assert!(!es_imagen(Path::new("a/sin-extension")));
    }

    #[test]
    fn un_mapa_de_bits_pegado_va_a_un_png_temporal_con_su_huella() {
        let dir = carpeta("mapa");
        let v = pegadas_de(Some(ContenidoPortapapeles::Imagen(mapa(9))), &dir);
        assert_eq!(v.len(), 1);
        assert!(v[0].temporal && v[0].ruta.is_file());
        assert_eq!(v[0].huella, huella_de_mapa(&mapa(9)));
        // La misma forma con otros pixeles, otra huella.
        assert_ne!(huella_de_mapa(&mapa(9)), huella_de_mapa(&mapa(10)));
        // Caso negativo: un mapa vacio o un texto no dan nada.
        let vacio = pixpin_codec::ImagenRgba {
            ancho: 0,
            alto: 0,
            pixeles: vec![],
        };
        assert!(pegadas_de(Some(ContenidoPortapapeles::Imagen(vacio)), &dir).is_empty());
        assert!(pegadas_de(Some(ContenidoPortapapeles::Texto("hola".into())), &dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn de_los_ficheros_copiados_solo_entran_las_imagenes_y_no_son_temporales() {
        let dir = carpeta("rutas");
        let (a, b) = (dir.join("a.png"), dir.join("b.txt"));
        std::fs::write(&a, b"x").unwrap();
        std::fs::write(&b, b"x").unwrap();
        let v = pegadas_de(Some(ContenidoPortapapeles::Rutas(vec![a.clone(), b])), &dir);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].ruta, a);
        // Caso negativo: no es nuestro, asi que soltarlo no lo borra.
        assert!(!v[0].temporal);
        v[0].soltar();
        assert!(a.is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_misma_imagen_dos_veces_entra_una_y_su_png_de_sobra_se_borra() {
        let dir = carpeta("repetida");
        let primera = pegadas_de(Some(ContenidoPortapapeles::Imagen(mapa(3))), &dir);
        let otra_vez = pegadas_de(Some(ContenidoPortapapeles::Imagen(mapa(3))), &dir);
        let sobra = otra_vez[0].ruta.clone();
        let (quedan, quitadas) = sin_repetidas(&primera, otra_vez);
        assert!(quedan.is_empty());
        assert_eq!(quitadas, 1);
        assert!(!sobra.exists(), "el PNG de la repetida no se queda en el temporal");
        // Caso negativo: una distinta si entra.
        let distinta = pegadas_de(Some(ContenidoPortapapeles::Imagen(mapa(4))), &dir);
        let (quedan, quitadas) = sin_repetidas(&primera, distinta);
        assert_eq!((quedan.len(), quitadas), (1, 0));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
