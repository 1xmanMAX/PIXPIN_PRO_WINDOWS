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

/// La miniatura que la Shell tiene de un archivo (D62): la misma que ensena
/// el Explorador. Vale para PDF, Office, imagenes y video, y para todo lo
/// que tenga un proveedor de miniaturas instalado. `None` si no la hay o
/// si el archivo no existe: la miniatura, al contrario que el icono, SI
/// necesita leer el archivo.
///
/// `lado` es el maximo pedido; la Shell devuelve lo que sepa extraer, con
/// la proporcion del original (`SIIGBF_BIGGERSIZEOK` permite que venga mas
/// grande antes que reescalada a peor).
pub fn miniatura_de(ruta: &Path, lado: u32) -> Option<ImagenRgba> {
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
    use windows::Win32::UI::Shell::{
        IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK,
        SIIGBF_THUMBNAILONLY,
    };

    let texto = HSTRING::from(ruta.as_os_str());
    // SAFETY: COM se inicializa (o se reutiliza, S_FALSE) y se libera en
    // pareja; el item y el HBITMAP son propios y se sueltan antes de salir.
    unsafe {
        let com = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let resultado = (|| {
            let item: IShellItemImageFactory = SHCreateItemFromParsingName(&texto, None).ok()?;
            let hbitmap = item
                .GetImage(
                    SIZE {
                        cx: lado as i32,
                        cy: lado as i32,
                    },
                    SIIGBF_THUMBNAILONLY | SIIGBF_BIGGERSIZEOK,
                )
                .ok()?;
            let imagen = hbitmap_a_rgba(hbitmap);
            let _ = DeleteObject(hbitmap.into());
            imagen
        })();
        if com.is_ok() {
            CoUninitialize();
        }
        resultado
    }
}

/// Lee los pixeles de un HBITMAP como RGBA recto. La Shell entrega 32 bits
/// BGRA; si el alfa viene todo a cero (origen de 24 bits: PDF, JPEG) la
/// imagen es opaca, y si viene premultiplicado (PNG) se deshace como en el
/// icono.
///
/// # Safety
/// `hb` debe ser un HBITMAP valido; no se destruye aqui.
unsafe fn hbitmap_a_rgba(hb: HBITMAP) -> Option<ImagenRgba> {
    use windows::Win32::Graphics::Gdi::{BITMAP, GetDIBits, GetObjectW};

    let mut bm = BITMAP::default();
    // SAFETY: el llamante garantiza un HBITMAP valido; `bm` es propia.
    let leido = unsafe {
        GetObjectW(
            hb.into(),
            size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut BITMAP as *mut core::ffi::c_void),
        )
    };
    if leido == 0 || bm.bmWidth <= 0 || bm.bmHeight <= 0 {
        return None;
    }
    let (ancho, alto) = (bm.bmWidth, bm.bmHeight);
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: ancho,
            biHeight: -alto,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixeles = vec![0u8; (ancho * alto * 4) as usize];
    // SAFETY: DC de memoria propio, liberado siempre; el bufer tiene
    // exactamente ancho*alto*4 bytes, que es lo que describe la cabecera.
    let filas = unsafe {
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            return None;
        }
        let filas = GetDIBits(
            dc,
            hb,
            0,
            alto as u32,
            Some(pixeles.as_mut_ptr() as *mut core::ffi::c_void),
            &mut info,
            DIB_RGB_COLORS,
        );
        let _ = DeleteDC(dc);
        filas
    };
    if filas == 0 {
        return None;
    }

    let opaca = pixeles.chunks_exact(4).all(|p| p[3] == 0);
    for p in pixeles.chunks_exact_mut(4) {
        let a = if opaca { 255 } else { p[3] };
        let (b, g, r) = if a == 0 || a == 255 {
            (p[0], p[1], p[2])
        } else {
            let d = |c: u8| ((c as u32 * 255) / a as u32).min(255) as u8;
            (d(p[0]), d(p[1]), d(p[2]))
        };
        p[0] = r;
        p[1] = g;
        p[2] = b;
        p[3] = a;
    }
    Some(ImagenRgba {
        ancho: ancho as u32,
        alto: alto as u32,
        pixeles,
    })
}

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
    fn la_miniatura_de_un_png_conserva_su_proporcion_y_su_color() {
        // La Shell siempre sabe hacer miniatura de un PNG: es el caso
        // controlado con el que se prueba la conversion HBITMAP -> RGBA.
        let ruta = std::env::temp_dir().join("pixpin-miniatura-prueba.png");
        let mut pixeles = Vec::with_capacity(64 * 48 * 4);
        for _ in 0..64 * 48 {
            pixeles.extend_from_slice(&[220, 30, 30, 255]);
        }
        let img = ImagenRgba {
            ancho: 64,
            alto: 48,
            pixeles,
        };
        pixpin_codec::guardar(&img, &ruta, pixpin_codec::FormatoImagen::Png).unwrap();

        let m = miniatura_de(&ruta, 256).expect("la Shell deberia dar miniatura de un PNG");
        assert!(m.ancho <= 256 && m.alto <= 256, "{}x{}", m.ancho, m.alto);
        let proporcion = m.ancho as f32 / m.alto as f32;
        assert!(
            (proporcion - 64.0 / 48.0).abs() < 0.05,
            "proporcion {proporcion}, esperada 1.333"
        );
        let centro = ((m.alto / 2) * m.ancho + m.ancho / 2) as usize * 4;
        let p = &m.pixeles[centro..centro + 4];
        assert!(
            p[0] > 150 && p[1] < 90 && p[2] < 90 && p[3] == 255,
            "el centro deberia ser rojo opaco, es {p:?}"
        );
        let _ = std::fs::remove_file(&ruta);
    }

    #[test]
    #[ignore = "pide sesion de escritorio (shell de Windows); --ignored"]
    fn sin_archivo_no_hay_miniatura() {
        // Caso negativo: al contrario que el icono, la miniatura necesita
        // leer el archivo; una ruta inexistente da None y el pin sera ficha.
        assert!(miniatura_de(Path::new(r"Z:\no\existe\informe.xyz"), 256).is_none());
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
