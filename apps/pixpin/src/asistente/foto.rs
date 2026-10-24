//! **La foto que se le enseña al asistente, reducida.**
//!
//! El proyector de vision de Qwen3.5 trocea la imagen en parches: una foto
//! del movil de 4000 px son miles de tokens y decenas de segundos solo en
//! «mirarla». A ~768 px de lado se siguen leyendo las figuras, los colores y
//! un titulo grande (medido, ver el documento §5.4), y cuesta de 200 a 950
//! tokens. Va en JPEG: pesa poco en la peticion y el modelo no lo nota.

use std::path::Path;

use pixpin_codec::imagen::ImagenRgba;

/// El lado mayor de la foto que se manda.
pub const LADO: u32 = 768;

/// Las medidas reducidas para que el lado mayor no pase de `lado` (nunca se
/// amplia).
pub fn medidas_reducidas(ancho: u32, alto: u32, lado: u32) -> (u32, u32) {
    let mayor = ancho.max(alto);
    if mayor <= lado || mayor == 0 {
        return (ancho, alto);
    }
    let f = lado as f64 / mayor as f64;
    (
        ((ancho as f64 * f).round() as u32).max(1),
        ((alto as f64 * f).round() as u32).max(1),
    )
}

/// La imagen reducida y en JPEG, lista para `Mensaje::usuario_con_imagen`.
pub fn preparar(imagen: ImagenRgba) -> Result<(Vec<u8>, &'static str), String> {
    let (a, h) = medidas_reducidas(imagen.ancho, imagen.alto, LADO);
    let reducida = if (a, h) == (imagen.ancho, imagen.alto) {
        imagen
    } else {
        pixpin_codec::imagen::redimensionar(imagen, a, h).map_err(|e| e.to_string())?
    };
    let bytes = pixpin_codec::imagen::codificar_jpg(&reducida, 85).map_err(|e| e.to_string())?;
    Ok((bytes, "image/jpeg"))
}

/// Lo mismo desde un fichero.
pub fn preparar_fichero(ruta: &Path) -> Result<(Vec<u8>, &'static str), String> {
    let imagen =
        pixpin_codec::imagen::cargar(ruta).map_err(|e| format!("no se pudo abrir la foto: {e}"))?;
    preparar(imagen)
}
