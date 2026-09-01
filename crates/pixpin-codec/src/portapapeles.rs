//! Copiar una imagen al portapapeles de Windows.
//!
//! Se publica como `CF_DIB`, que es el formato que entienden practicamente
//! todas las aplicaciones de escritorio, desde Paint hasta Word.
//!
//! La parte engorrosa es que un DIB clasico guarda las filas **de abajo a
//! arriba**. Se puede pedir el orden natural poniendo un alto negativo en la
//! cabecera, pero varias aplicaciones antiguas lo ignoran y muestran la imagen
//! del reves, asi que aqui se invierten las filas al copiar. Es un coste
//! pequeno y se paga una sola vez.

use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Gdi::{BI_RGB, BITMAPINFOHEADER};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GHND, GlobalAlloc, GlobalLock, GlobalUnlock};

use crate::imagen::{ErrorCodec, ImagenRgba};

/// Formato de portapapeles para un mapa de bits independiente del dispositivo.
const CF_DIB: u32 = 8;

/// Copia la imagen al portapapeles como `CF_DIB`.
pub fn copiar_imagen(imagen: &ImagenRgba) -> Result<(), ErrorCodec> {
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

    let cabecera = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: imagen.ancho as i32,
        biHeight: imagen.alto as i32,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        biSizeImage: espera as u32,
        ..Default::default()
    };

    let total = size_of::<BITMAPINFOHEADER>() + espera;

    // SAFETY: se pide memoria movible del tamano exacto que se va a escribir.
    // El bloque se cede al portapapeles mas abajo con `SetClipboardData`, que
    // pasa a ser su dueno; por eso no se libera aqui en el camino de exito.
    let bloque = unsafe { GlobalAlloc(GHND, total).map_err(|_| ErrorCodec::Portapapeles)? };

    // SAFETY: `bloque` acaba de reservarse y no esta bloqueado por nadie mas.
    let destino = unsafe { GlobalLock(bloque) };
    if destino.is_null() {
        return Err(ErrorCodec::Portapapeles);
    }

    // SAFETY: `destino` apunta a `total` bytes escribibles, y se escriben
    // exactamente esos: la cabecera y despues `espera` bytes de pixeles.
    unsafe {
        std::ptr::copy_nonoverlapping(
            &cabecera as *const BITMAPINFOHEADER as *const u8,
            destino as *mut u8,
            size_of::<BITMAPINFOHEADER>(),
        );

        let pixeles_destino = (destino as *mut u8).add(size_of::<BITMAPINFOHEADER>());
        let paso = imagen.ancho as usize * 4;
        // Filas invertidas: un DIB clasico las guarda de abajo a arriba.
        for fila in 0..imagen.alto as usize {
            let origen = &imagen.pixeles[fila * paso..(fila + 1) * paso];
            let destino_fila = pixeles_destino.add((imagen.alto as usize - 1 - fila) * paso);
            // Y de RGBA a BGRA, que es lo que espera un DIB.
            for (i, pixel) in origen.chunks_exact(4).enumerate() {
                let p = destino_fila.add(i * 4);
                *p = pixel[2];
                *p.add(1) = pixel[1];
                *p.add(2) = pixel[0];
                *p.add(3) = pixel[3];
            }
        }

        let _ = GlobalUnlock(bloque);
    }

    // SAFETY: se abre el portapapeles, se vacia, se cede el bloque y se
    // cierra. `SetClipboardData` toma la propiedad del bloque en caso de
    // exito, asi que no se libera despues.
    unsafe {
        OpenClipboard(None).map_err(|_| ErrorCodec::Portapapeles)?;
        let vaciado = EmptyClipboard();
        let puesto = SetClipboardData(CF_DIB, Some(HANDLE(bloque.0)));
        let _ = CloseClipboard();
        vaciado.map_err(|_| ErrorCodec::Portapapeles)?;
        puesto.map_err(|_| ErrorCodec::Portapapeles)?;
    }

    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Toca el portapapeles del usuario, que es un recurso global de la
    /// sesion, asi que necesita escritorio. Ejecutar con `--ignored`.
    #[test]
    #[ignore = "toca el portapapeles real; ejecutar con --ignored"]
    fn copiar_una_imagen_deja_un_mapa_de_bits_en_el_portapapeles() {
        use windows::Win32::System::DataExchange::{
            CloseClipboard, IsClipboardFormatAvailable, OpenClipboard,
        };

        let img = ImagenRgba {
            ancho: 2,
            alto: 2,
            pixeles: vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
            ],
        };

        copiar_imagen(&img).expect("deberia copiar al portapapeles");

        // CF_DIB = 8. Se comprueba que el formato quedo disponible de verdad,
        // no solo que la funcion devolvio Ok.
        // SAFETY: se abre y se cierra el portapapeles en la misma funcion, sin
        // ningun retorno intermedio entre ambas llamadas.
        unsafe {
            OpenClipboard(None).expect("deberia poder abrirse");
            let disponible = IsClipboardFormatAvailable(8).is_ok();
            let _ = CloseClipboard();
            assert!(
                disponible,
                "no quedo ningun mapa de bits en el portapapeles"
            );
        }
    }

    #[test]
    fn copiar_una_imagen_vacia_da_error_sin_tocar_el_portapapeles() {
        // Caso negativo, y ademas no necesita escritorio porque falla antes de
        // llegar a Win32.
        let vacia = ImagenRgba {
            ancho: 0,
            alto: 0,
            pixeles: vec![],
        };
        assert!(copiar_imagen(&vacia).is_err());
    }

    #[test]
    fn copiar_con_buffer_incoherente_da_error() {
        let mala = ImagenRgba {
            ancho: 4,
            alto: 4,
            pixeles: vec![0; 3],
        };
        assert!(copiar_imagen(&mala).is_err());
    }
}
