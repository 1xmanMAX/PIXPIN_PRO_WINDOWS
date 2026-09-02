//! El icono real que Windows le da a un fichero, en pixeles RGBA.
//!
//! La ficha de archivo (D30) muestra el icono de verdad — el del PDF, el de
//! la carpeta, el de la aplicacion asociada —, no un dibujo generico: es lo
//! que hace que la ficha se reconozca de un vistazo.
//!
//! Dos detalles que no son obvios:
//!
//! - Se pide con `SHGFI_USEFILEATTRIBUTES`, asi que **funciona con rutas que
//!   ya no existen**. Una referencia rota (D28) tiene que enseñar el icono
//!   generico de su extension, no desaparecer.
//! - El icono se dibuja sobre una DIB seccion de 32 bits y se convierte de
//!   BGRA premultiplicado a RGBA recto, que es lo que consume el resto del
//!   programa.

use std::path::Path;

use pixpin_codec::ImagenRgba;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, HBITMAP, SelectObject,
};
use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows::Win32::UI::Shell::{
    SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_USEFILEATTRIBUTES, SHGetFileInfoW,
};
use windows::Win32::UI::WindowsAndMessaging::{DI_NORMAL, DestroyIcon, DrawIconEx};
use windows::core::HSTRING;

/// Lado del icono pedido, en pixeles. `SHGFI_LARGEICON` da 32x32 en el DPI
/// estandar del shell; se escala al dibujar la ficha.
pub const LADO_ICONO: u32 = 32;

/// El icono del fichero (o de su extension si la ruta ya no existe) en RGBA.
/// `None` solo si Windows no da icono alguno, algo que no deberia pasar.
pub fn icono_de(ruta: &Path) -> Option<ImagenRgba> {
    let texto = HSTRING::from(ruta.as_os_str());
    let mut info = SHFILEINFOW::default();

    // SAFETY: `info` es propia y su tamano se pasa exacto. Con
    // USEFILEATTRIBUTES el shell no toca el disco, asi que vale para rutas
    // inexistentes. El HICON devuelto es nuestro y se destruye abajo.
    let ok = unsafe {
        SHGetFileInfoW(
            &texto,
            FILE_ATTRIBUTE_NORMAL,
            Some(&mut info),
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES,
        )
    };
    if ok == 0 || info.hIcon.is_invalid() {
        return None;
    }

    let resultado = rasterizar(info.hIcon);
    // SAFETY: el HICON lo creo SHGetFileInfoW para nosotros y nadie mas lo
    // usa ya; no destruirlo filtraria un icono por cada ficha creada.
    unsafe {
        let _ = DestroyIcon(info.hIcon);
    }
    resultado
}

/// Dibuja el icono en una DIB de 32 bits y devuelve sus pixeles en RGBA.
fn rasterizar(icono: windows::Win32::UI::WindowsAndMessaging::HICON) -> Option<ImagenRgba> {
    let lado = LADO_ICONO as i32;
    let cabecera = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: lado,
        // Negativo: filas de arriba abajo, el orden natural. Asi no hay que
        // invertir nada despues.
        biHeight: -lado,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let info = BITMAPINFO {
        bmiHeader: cabecera,
        ..Default::default()
    };

    let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
    // SAFETY: se crea un DC de memoria y una DIB seccion propios; los dos se
    // liberan antes de salir por cualquier camino de esta funcion.
    unsafe {
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            return None;
        }
        let bitmap: HBITMAP = match windows::Win32::Graphics::Gdi::CreateDIBSection(
            Some(dc),
            &info,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        ) {
            Ok(b) if !b.is_invalid() && !bits.is_null() => b,
            _ => {
                let _ = DeleteDC(dc);
                return None;
            }
        };

        let anterior = SelectObject(dc, bitmap.into());
        let dibujado = DrawIconEx(dc, 0, 0, icono, lado, lado, 0, None, DI_NORMAL).is_ok();

        let imagen = if dibujado {
            let total = (lado * lado) as usize;
            let origen = std::slice::from_raw_parts(bits as *const u8, total * 4);
            let mut pixeles = vec![0u8; total * 4];
            for (i, p) in origen.chunks_exact(4).enumerate() {
                // El shell entrega BGRA premultiplicado; se deshace la
                // premultiplicacion para que el pintor, que multiplica de
                // nuevo al componer, no oscurezca los bordes suaves.
                let a = p[3];
                let (b, g, r) = if a == 0 || a == 255 {
                    (p[0], p[1], p[2])
                } else {
                    let d = |c: u8| ((c as u32 * 255) / a as u32).min(255) as u8;
                    (d(p[0]), d(p[1]), d(p[2]))
                };
                pixeles[i * 4] = r;
                pixeles[i * 4 + 1] = g;
                pixeles[i * 4 + 2] = b;
                pixeles[i * 4 + 3] = a;
            }
            Some(ImagenRgba {
                ancho: LADO_ICONO,
                alto: LADO_ICONO,
                pixeles,
            })
        } else {
            None
        };

        SelectObject(dc, anterior);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(dc);
        imagen
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    #[ignore = "pide sesion de escritorio (shell de Windows); --ignored"]
    fn el_icono_de_un_ejecutable_real_trae_pixeles() {
        let img = icono_de(Path::new(r"C:\Windows\notepad.exe"))
            .expect("el shell deberia dar icono del Bloc de notas");
        assert_eq!((img.ancho, img.alto), (LADO_ICONO, LADO_ICONO));
        assert!(
            img.pixeles.chunks_exact(4).any(|p| p[3] != 0),
            "un icono entero transparente significa que no se dibujo nada"
        );
    }

    #[test]
    #[ignore = "pide sesion de escritorio (shell de Windows); --ignored"]
    fn una_ruta_que_no_existe_tiene_igualmente_icono() {
        // D28: la referencia rota se MUESTRA, no se oculta. Sin
        // USEFILEATTRIBUTES esto devolveria None y la ficha saldria pelada.
        let img = icono_de(Path::new(r"Z:\no\existe\informe.pdf"));
        assert!(
            img.is_some(),
            "el icono generico de la extension debe salir"
        );
    }
}
