//! El motor de siempre: `Windows.Media.Ocr`, que viene con Windows 10 y 11.
//!
//! Funciona sin conexion, cubre los idiomas que el usuario tenga instalados
//! y no anade un solo byte al ejecutable. Para texto de pantalla —limpio,
//! renderizado y recto— basta de sobra. Es el motor de fabrica y el
//! respaldo de Indra.

use pixpin_geom::Rect;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Security::Cryptography::CryptographicBuffer;
use windows_future::AsyncStatus;

use crate::{ErrorOcr, Linea, Palabra, envolvente, medidas_validas};

/// Si el equipo puede reconocer texto. Se pregunta antes de ofrecerlo, para
/// no abrir una captura que luego no va a servir de nada.
pub fn disponible() -> bool {
    OcrEngine::TryCreateFromUserProfileLanguages().is_ok()
}

/// Reconoce el texto de una imagen RGBA de `ancho` x `alto`.
///
/// Devuelve una linea por cada renglon que encuentre, con su caja. El orden
/// es el que da el sistema; ponerlo en orden de lectura es cosa de
/// `pixpin_geom::parrafos::agrupar`.
pub fn reconocer(ancho: u32, alto: u32, pixeles_rgba: &[u8]) -> Result<Vec<Linea>, ErrorOcr> {
    if !medidas_validas(ancho, alto, pixeles_rgba) {
        return Err(ErrorOcr::ImagenInvalida);
    }

    // El motor se crea con los idiomas del perfil del usuario: si escribe en
    // español e ingles, reconoce los dos sin preguntar nada.
    let motor = OcrEngine::TryCreateFromUserProfileLanguages().map_err(|_| ErrorOcr::SinIdiomas)?;

    // Windows quiere BGRA y nuestras imagenes son RGBA: se intercambian los
    // canales rojo y azul. Se copia porque `SoftwareBitmap` se queda con su
    // propio buffer de todas formas.
    let mut bgra = pixeles_rgba[..ancho as usize * alto as usize * 4].to_vec();
    for p in bgra.chunks_exact_mut(4) {
        p.swap(0, 2);
    }

    let buffer = CryptographicBuffer::CreateFromByteArray(&bgra)?;
    let mapa = SoftwareBitmap::CreateCopyFromBuffer(
        &buffer,
        BitmapPixelFormat::Bgra8,
        ancho as i32,
        alto as i32,
    )?;

    // Se espera a que termine y se recoge el resultado. Es una llamada local
    // sobre un recorte de pantalla, del orden de decenas de milisegundos;
    // sacarla a un hilo complicaria el bucle de mensajes sin ganar nada
    // perceptible. Se duerme un milisegundo entre vueltas para no quemar un
    // nucleo en el equipo suelo, que solo tiene cuatro.
    let operacion = motor.RecognizeAsync(&mapa)?;
    while operacion.Status()? == AsyncStatus::Started {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let resultado = operacion.GetResults()?;

    let mut lineas = Vec::new();
    for linea in resultado.Lines()? {
        let texto = linea.Text()?.to_string();
        if texto.trim().is_empty() {
            continue;
        }
        // `OcrLine` no da su caja: la unica geometria esta en las palabras,
        // asi que la linea es la envolvente de las suyas.
        let mut caja: Option<Rect> = None;
        let mut palabras = Vec::new();
        for palabra in linea.Words()? {
            let r = palabra.BoundingRect()?;
            let suya = Rect {
                x: r.X.round() as i32,
                y: r.Y.round() as i32,
                ancho: r.Width.round().max(0.0) as u32,
                alto: r.Height.round().max(0.0) as u32,
            };
            caja = Some(match caja {
                None => suya,
                Some(a) => envolvente(a, suya),
            });
            palabras.push(Palabra {
                caja: suya,
                texto: palabra.Text()?.to_string(),
            });
        }
        if let Some(caja) = caja {
            lineas.push(Linea {
                caja,
                texto,
                palabras,
            });
        }
    }
    Ok(lineas)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_imagen_incoherente_se_rechaza_antes_de_tocar_windows() {
        // Caso negativo: sin esta comprobacion un buffer corto llegaria a
        // `CreateCopyFromBuffer`, y el fallo saldria como un error del
        // sistema que no dice nada de lo que pasa de verdad.
        assert!(matches!(
            reconocer(10, 10, &[0u8; 8]),
            Err(ErrorOcr::ImagenInvalida)
        ));
        assert!(matches!(
            reconocer(0, 0, &[]),
            Err(ErrorOcr::ImagenInvalida)
        ));
        // Y unas medidas imposibles no desbordan al multiplicar.
        assert!(matches!(
            reconocer(u32::MAX, u32::MAX, &[0u8; 16]),
            Err(ErrorOcr::ImagenInvalida)
        ));
    }

    #[test]
    #[ignore = "necesita un Windows con idiomas de reconocimiento; ejecutar con --ignored"]
    fn el_equipo_dice_si_puede_reconocer() {
        // No se afirma el resultado: un Windows sin paquetes de idioma es
        // legitimo, y entonces la funcion tiene que decir que no, no fallar.
        let _ = disponible();
    }
}
