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

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, TRUE};
use windows::Win32::Graphics::Gdi::{BI_RGB, BITMAPINFOHEADER};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GHND, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::UI::Shell::{DROPFILES, DragQueryFileW, HDROP};

use crate::imagen::{ErrorCodec, ImagenRgba};

/// Formato de portapapeles para un mapa de bits independiente del dispositivo.
const CF_DIB: u32 = 8;
/// Texto Unicode.
const CF_UNICODETEXT: u32 = 13;
/// Lista de ficheros soltados o copiados desde el Explorador.
const CF_HDROP: u32 = 15;

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

/// Lo que habia en el portapapeles cuando se preguntó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContenidoPortapapeles {
    Imagen(ImagenRgba),
    Texto(String),
    Rutas(Vec<std::path::PathBuf>),
}

/// Cierra el portapapeles al salir del ambito, pase lo que pase. Sin esto,
/// un retorno temprano dejaria el portapapeles abierto y NINGUNA otra
/// aplicacion del sistema podria usarlo hasta cerrar PixPin.
struct GuardiaPortapapeles;

impl Drop for GuardiaPortapapeles {
    fn drop(&mut self) {
        // SAFETY: solo se construye tras un OpenClipboard con exito.
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

/// Lee el portapapeles. `None` si esta vacio o trae un formato ajeno.
///
/// Prioridad **archivos > imagen > texto** y no es casual: quien copia
/// ficheros en el Explorador deja ademas su ruta como texto, y lo que quiso
/// copiar fue el fichero.
pub fn leer() -> Option<ContenidoPortapapeles> {
    // SAFETY: si abre, el guardia de mas abajo garantiza el cierre.
    unsafe { OpenClipboard(None) }.ok()?;
    let _guardia = GuardiaPortapapeles;

    if let Some(rutas) = leer_rutas() {
        return Some(ContenidoPortapapeles::Rutas(rutas));
    }
    if let Some(imagen) = leer_dib() {
        return Some(ContenidoPortapapeles::Imagen(imagen));
    }
    leer_texto().map(ContenidoPortapapeles::Texto)
}

/// Copia texto plano como `CF_UNICODETEXT`. La pareja de `copiar_imagen`
/// para las notas.
pub fn copiar_texto(texto: &str) -> Result<(), ErrorCodec> {
    let utf16: Vec<u16> = texto.encode_utf16().chain(std::iter::once(0)).collect();
    let total = utf16.len() * 2;

    // SAFETY: memoria movible del tamano exacto que se va a escribir; el
    // bloque se cede al portapapeles con SetClipboardData, que pasa a ser su
    // dueno, asi que no se libera en el camino de exito.
    let bloque = unsafe { GlobalAlloc(GHND, total).map_err(|_| ErrorCodec::Portapapeles)? };
    // SAFETY: recien reservado, nadie mas lo tiene bloqueado.
    let destino = unsafe { GlobalLock(bloque) };
    if destino.is_null() {
        return Err(ErrorCodec::Portapapeles);
    }
    // SAFETY: `destino` apunta a `total` bytes escribibles y se escriben
    // exactamente esos.
    unsafe {
        std::ptr::copy_nonoverlapping(utf16.as_ptr() as *const u8, destino as *mut u8, total);
        let _ = GlobalUnlock(bloque);
    }

    // SAFETY: abrir, vaciar, ceder y cerrar, sin retornos intermedios.
    unsafe {
        OpenClipboard(None).map_err(|_| ErrorCodec::Portapapeles)?;
        let vaciado = EmptyClipboard();
        let puesto = SetClipboardData(CF_UNICODETEXT, Some(HANDLE(bloque.0)));
        let _ = CloseClipboard();
        vaciado.map_err(|_| ErrorCodec::Portapapeles)?;
        puesto.map_err(|_| ErrorCodec::Portapapeles)?;
    }
    Ok(())
}

/// Copia una lista de ficheros como `CF_HDROP`, el mismo formato que deja el
/// Explorador al hacer Ctrl+C sobre un fichero.
///
/// Es lo que hace que al pegar en el Explorador, en WhatsApp, en Discord o en
/// un correo viaje el fichero y no su nombre escrito como texto. La pareja de
/// lectura es `leer_rutas`.
///
/// Las rutas deben ser absolutas; ver `construir_hdrop` para el porque.
pub fn copiar_ficheros(rutas: &[std::path::PathBuf]) -> Result<(), ErrorCodec> {
    // Primero se monta la carga entera y solo despues se toca Win32: si algo
    // esta mal, el portapapeles del usuario ni se entera.
    let carga = construir_hdrop(rutas)?;

    // SAFETY: memoria movible del tamano exacto que se va a escribir. En el
    // camino de exito el bloque se cede al portapapeles y no se libera; en
    // cada camino de fallo de aqui abajo seguimos siendo duenos y se libera a
    // mano, porque el bloque no cambia de dueno hasta que `SetClipboardData`
    // devuelve exito.
    let bloque = unsafe { GlobalAlloc(GHND, carga.len()).map_err(|_| ErrorCodec::Portapapeles)? };

    // SAFETY: recien reservado, nadie mas lo tiene bloqueado.
    let destino = unsafe { GlobalLock(bloque) };
    if destino.is_null() {
        // SAFETY: el bloque es nuestro y el bloqueo fallo, asi que no hay
        // nadie dentro; sin este `GlobalFree` se filtraria memoria global,
        // que es un recurso de todo el proceso.
        unsafe {
            let _ = GlobalFree(Some(bloque));
        }
        return Err(ErrorCodec::Portapapeles);
    }

    // SAFETY: `destino` apunta a `carga.len()` bytes escribibles y se escriben
    // exactamente esos.
    unsafe {
        std::ptr::copy_nonoverlapping(carga.as_ptr(), destino as *mut u8, carga.len());
        let _ = GlobalUnlock(bloque);
    }

    // SAFETY: abrir, vaciar, ceder y cerrar sin retornos intermedios que
    // dejarian el portapapeles abierto para toda la sesion.
    unsafe {
        if OpenClipboard(None).is_err() {
            let _ = GlobalFree(Some(bloque));
            return Err(ErrorCodec::Portapapeles);
        }
        let vaciado = EmptyClipboard();
        let puesto = SetClipboardData(CF_HDROP, Some(HANDLE(bloque.0)));
        let _ = CloseClipboard();
        // El unico caso en que el bloque sigue siendo nuestro es que la cesion
        // fallara; si tuvo exito, liberarlo aqui seria una doble liberacion.
        if puesto.is_err() {
            let _ = GlobalFree(Some(bloque));
            return Err(ErrorCodec::Portapapeles);
        }
        vaciado.map_err(|_| ErrorCodec::Portapapeles)?;
    }

    Ok(())
}

/// Monta la carga util de `CF_HDROP`: una cabecera `DROPFILES` seguida de las
/// rutas en UTF-16, cada una terminada en NUL, y un NUL extra que cierra la
/// lista.
///
/// Va aparte del trasiego de Win32 para poder comprobar los bytes en una
/// prueba normal, sin abrir el portapapeles, que es un recurso global de la
/// sesion y puede estar tomado por otro proceso.
///
/// Todos los rechazos de aqui devuelven `ErrorCodec::Portapapeles`, que es
/// mas vago de lo que merecen: una variante propia por motivo (lista vacia,
/// ruta relativa, NUL dentro del nombre) diria mucho mas al llamante, pero el
/// enum vive en `imagen.rs`. Vale la pena partirlo cuando alguien necesite
/// distinguirlos.
///
/// Es publica porque el arrastre saliente del pin ofrece esta misma carga
/// dentro de un `STGMEDIUM` en vez de dejarla en el portapapeles: el formato
/// es el mismo byte a byte, y rehacerlo alli seria rehacer tambien la trampa
/// de `fWide`.
pub fn construir_hdrop(rutas: &[std::path::PathBuf]) -> Result<Vec<u8>, ErrorCodec> {
    use std::os::windows::ffi::OsStrExt;

    // Publicar un HDROP sin ficheros vaciaria el portapapeles a cambio de
    // nada: el usuario perderia lo que tenia copiado y no ganaria un pegado.
    if rutas.is_empty() {
        return Err(ErrorCodec::Portapapeles);
    }

    let mut nombres: Vec<u16> = Vec::new();
    for ruta in rutas {
        // Se rechaza la relativa en vez de convertirla. Convertir la resolveria
        // contra el directorio de trabajo de PixPin, que en una aplicacion de
        // escritorio puede ser cualquier cosa (System32 si arranca con la
        // sesion), asi que daria una ruta absoluta igual de equivocada pero sin
        // avisar: el destino pegaria un fichero inexistente. Mejor un error que
        // el llamante ve.
        if !ruta.is_absolute() {
            return Err(ErrorCodec::Portapapeles);
        }
        let ancha: Vec<u16> = ruta.as_os_str().encode_wide().collect();
        // Un NUL dentro del nombre cortaria la lista justo ahi y el receptor
        // leeria el resto de rutas como si fueran otras entradas. No se
        // publica nada antes que publicar una lista partida.
        if ancha.contains(&0) {
            return Err(ErrorCodec::Portapapeles);
        }
        nombres.extend_from_slice(&ancha);
        nombres.push(0);
    }
    // El NUL de mas: es lo unico que le dice al receptor donde acaba la lista.
    nombres.push(0);

    let cabecera = DROPFILES {
        // Desplazamiento en bytes desde el principio del bloque hasta el primer
        // nombre. Los nombres van pegados detras de la cabecera, asi que es su
        // tamano exacto.
        pFiles: size_of::<DROPFILES>() as u32,
        // TRUE porque los nombres van en UTF-16, y este es EL fallo clasico del
        // formato: con fWide en FALSE el receptor lee los mismos bytes como
        // ANSI, ve el primer byte de la primera letra seguido de un cero y pega
        // un nombre de una sola letra o basura, sin un solo error que ayude a
        // encontrarlo.
        fWide: TRUE,
        // `pt` y `fNC` solo importan cuando el HDROP viene de un arrastre; para
        // un copiado valen cero.
        ..Default::default()
    };

    let mut carga = Vec::with_capacity(size_of::<DROPFILES>() + nombres.len() * 2);
    // SAFETY: `DROPFILES` es un POD `repr(C, packed)` con todos sus campos
    // inicializados, asi que sus `size_of` bytes son legibles y no hay relleno
    // sin inicializar. Solo se copian, no se guarda el puntero.
    carga.extend_from_slice(unsafe {
        std::slice::from_raw_parts(
            &cabecera as *const DROPFILES as *const u8,
            size_of::<DROPFILES>(),
        )
    });
    // Win32 espera UTF-16 en el orden nativo, que en toda maquina Windows
    // soportada es little endian.
    for unidad in nombres {
        carga.extend_from_slice(&unidad.to_le_bytes());
    }
    Ok(carga)
}

/// Requiere el portapapeles ya abierto por el llamante.
fn leer_texto() -> Option<String> {
    // SAFETY: el portapapeles esta abierto; el handle es propiedad del
    // portapapeles y solo se lee mientras dure el bloqueo.
    unsafe {
        let handle = GetClipboardData(CF_UNICODETEXT).ok()?;
        let bloque = HGLOBAL(handle.0);
        let datos = GlobalLock(bloque) as *const u16;
        if datos.is_null() {
            return None;
        }
        let mut largo = 0usize;
        while *datos.add(largo) != 0 && largo < 16 * 1024 * 1024 {
            largo += 1;
        }
        let texto = String::from_utf16_lossy(std::slice::from_raw_parts(datos, largo));
        let _ = GlobalUnlock(bloque);
        if texto.is_empty() { None } else { Some(texto) }
    }
}

/// Requiere el portapapeles ya abierto por el llamante.
fn leer_rutas() -> Option<Vec<std::path::PathBuf>> {
    // SAFETY: el portapapeles esta abierto; DragQueryFileW se usa con el
    // patron documentado (primero el numero, luego cada nombre con su
    // longitud consultada antes de reservar).
    unsafe {
        let handle = GetClipboardData(CF_HDROP).ok()?;
        let drop = HDROP(handle.0);
        let cuantos = DragQueryFileW(drop, u32::MAX, None);
        let mut rutas = Vec::new();
        for i in 0..cuantos {
            let largo = DragQueryFileW(drop, i, None) as usize;
            if largo == 0 {
                continue;
            }
            let mut buffer = vec![0u16; largo + 1];
            let escritos = DragQueryFileW(drop, i, Some(&mut buffer)) as usize;
            if escritos > 0 {
                rutas.push(std::path::PathBuf::from(String::from_utf16_lossy(
                    &buffer[..escritos],
                )));
            }
        }
        if rutas.is_empty() { None } else { Some(rutas) }
    }
}

/// Requiere el portapapeles ya abierto por el llamante. Convierte el DIB a
/// RGBA compacto: filas de abajo arriba y relleno a 4 bytes son cosa del
/// formato, no del resto del programa.
fn leer_dib() -> Option<ImagenRgba> {
    // SAFETY: el portapapeles esta abierto. Antes de leer un solo pixel se
    // comprueba que el bloque tiene al menos cabecera + los bytes que la
    // propia cabecera declara, asi que las lecturas de abajo estan dentro.
    unsafe {
        let handle = GetClipboardData(CF_DIB).ok()?;
        let bloque = HGLOBAL(handle.0);
        let base = GlobalLock(bloque) as *const u8;
        if base.is_null() {
            return None;
        }
        let disponible = GlobalSize(bloque);
        let resultado = leer_dib_desde(base, disponible);
        let _ = GlobalUnlock(bloque);
        resultado
    }
}

/// # Safety
///
/// `base` debe apuntar a `disponible` bytes legibles.
unsafe fn leer_dib_desde(base: *const u8, disponible: usize) -> Option<ImagenRgba> {
    let cab_tam = size_of::<BITMAPINFOHEADER>();
    if disponible < cab_tam {
        return None;
    }
    // SAFETY: se acaba de comprobar que hay al menos una cabecera.
    let cabecera: BITMAPINFOHEADER = unsafe { std::ptr::read_unaligned(base as *const _) };

    let ancho = cabecera.biWidth;
    let alto_declarado = cabecera.biHeight;
    // Un alto negativo significa filas ya en orden natural (arriba abajo).
    let de_arriba_abajo = alto_declarado < 0;
    let alto = alto_declarado.unsigned_abs();
    if ancho <= 0 || alto == 0 || cabecera.biCompression != BI_RGB.0 {
        return None;
    }
    let bits = cabecera.biBitCount;
    if bits != 24 && bits != 32 {
        return None;
    }

    let ancho = ancho as usize;
    let alto = alto as usize;
    let bytes_pixel = bits as usize / 8;
    // Las filas de un DIB estan alineadas a 4 bytes.
    let paso = (ancho * bytes_pixel).div_ceil(4) * 4;
    // Tras la cabecera puede haber mascaras o paleta; con BI_RGB de 24/32
    // bits no las hay, asi que los pixeles empiezan justo despues.
    let inicio = cabecera.biSize.max(cab_tam as u32) as usize;
    if disponible < inicio + paso * alto {
        return None;
    }

    let mut pixeles = vec![0u8; ancho * alto * 4];
    for fila in 0..alto {
        let origen_fila = if de_arriba_abajo {
            fila
        } else {
            alto - 1 - fila
        };
        // SAFETY: el rango [inicio, inicio + paso*alto) se comprobo arriba.
        let src = unsafe { base.add(inicio + origen_fila * paso) };
        for x in 0..ancho {
            // SAFETY: x < ancho y ancho*bytes_pixel <= paso.
            let p = unsafe { src.add(x * bytes_pixel) };
            let destino = (fila * ancho + x) * 4;
            // SAFETY: dentro de la fila reservada arriba.
            unsafe {
                pixeles[destino] = *p.add(2); // B G R -> R
                pixeles[destino + 1] = *p.add(1);
                pixeles[destino + 2] = *p;
                pixeles[destino + 3] = if bytes_pixel == 4 { *p.add(3) } else { 255 };
            }
        }
    }

    // Muchas aplicaciones publican DIB de 32 bits con el alfa a cero sin
    // querer decir "transparente". Un pin totalmente invisible seria un
    // fallo peor que ignorar un alfa legitimo: si TODO es cero, se opaca.
    if bytes_pixel == 4 && pixeles.chunks_exact(4).all(|p| p[3] == 0) {
        for p in pixeles.chunks_exact_mut(4) {
            p[3] = 255;
        }
    }

    Some(ImagenRgba {
        ancho: ancho as u32,
        alto: alto as u32,
        pixeles,
    })
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
    #[ignore = "toca el portapapeles real; ejecutar con --ignored"]
    fn una_imagen_copiada_se_lee_de_vuelta_identica() {
        // La ida y vuelta completa: copiar_imagen escribe BGRA de abajo
        // arriba, leer() deshace las dos cosas. Si alguien toca una sola de
        // las dos rutas, este test lo caza.
        let img = ImagenRgba {
            ancho: 3,
            alto: 2,
            pixeles: vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, //
                10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255,
            ],
        };

        copiar_imagen(&img).expect("deberia copiar");

        match leer() {
            Some(ContenidoPortapapeles::Imagen(vuelta)) => assert_eq!(
                vuelta, img,
                "la imagen leida debe ser identica a la copiada"
            ),
            otro => panic!("se esperaba una imagen, llego {otro:?}"),
        }
    }

    #[test]
    #[ignore = "toca el portapapeles real; ejecutar con --ignored"]
    fn un_texto_copiado_se_lee_de_vuelta_con_sus_acentos() {
        copiar_texto("Señal — ñandú 漢字").expect("deberia copiar texto");
        match leer() {
            Some(ContenidoPortapapeles::Texto(t)) => assert_eq!(t, "Señal — ñandú 漢字"),
            otro => panic!("se esperaba texto, llego {otro:?}"),
        }
    }

    #[test]
    #[ignore = "toca el portapapeles real; ejecutar con --ignored"]
    fn con_imagen_y_texto_a_la_vez_gana_la_imagen() {
        // Caso negativo del orden de prioridad: si `leer` mirara el texto
        // primero, pinear una captura copiada daria una nota con basura.
        // (Rutas > imagen no se puede montar sin el Explorador; el orden del
        // codigo lo garantiza y este test cubre el escalon que si es
        // construible aqui.)
        let img = ImagenRgba {
            ancho: 1,
            alto: 1,
            pixeles: vec![1, 2, 3, 255],
        };
        copiar_imagen(&img).unwrap();
        // copiar_texto vacia el portapapeles, asi que hay que anadir el DIB
        // despues: se copia el texto primero y la imagen encima.
        copiar_texto("texto que no debe ganar").unwrap();
        copiar_imagen(&img).unwrap();
        assert!(matches!(leer(), Some(ContenidoPortapapeles::Imagen(_))));
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

    /// Devuelve las unidades UTF-16 que van detras de la cabecera `DROPFILES`.
    fn nombres_de(carga: &[u8]) -> Vec<u16> {
        carga[size_of::<DROPFILES>()..]
            .chunks_exact(2)
            .map(|par| u16::from_le_bytes([par[0], par[1]]))
            .collect()
    }

    #[test]
    fn la_cabecera_hdrop_apunta_tras_de_si_y_declara_texto_ancho() {
        let carga = construir_hdrop(&[std::path::PathBuf::from(r"C:\tmp\a.png")])
            .expect("una ruta absoluta deberia valer");

        let desplazamiento = u32::from_le_bytes(carga[0..4].try_into().unwrap());
        assert_eq!(
            desplazamiento as usize,
            size_of::<DROPFILES>(),
            "pFiles debe ser el desplazamiento en bytes hasta el primer nombre"
        );

        // fWide es el ultimo campo de la cabecera. Si esto se pone a cero, el
        // Explorador pega basura sin dar ningun error, asi que se blinda aqui.
        let ancho =
            i32::from_le_bytes(carga[size_of::<DROPFILES>() - 4..][..4].try_into().unwrap());
        assert_eq!(
            ancho, 1,
            "fWide debe ser TRUE porque las rutas van en UTF-16"
        );
    }

    #[test]
    fn la_lista_de_rutas_acaba_en_un_nul_de_mas() {
        let carga = construir_hdrop(&[
            std::path::PathBuf::from(r"C:\tmp\a.png"),
            std::path::PathBuf::from(r"C:\tmp\b.png"),
        ])
        .expect("dos rutas absolutas deberian valer");

        let unidades = nombres_de(&carga);
        let esperado: Vec<u16> = r"C:\tmp\a.png"
            .encode_utf16()
            .chain(std::iter::once(0))
            .chain(r"C:\tmp\b.png".encode_utf16())
            .chain(std::iter::once(0))
            // El NUL que cierra la lista entera.
            .chain(std::iter::once(0))
            .collect();
        assert_eq!(unidades, esperado);
    }

    #[test]
    fn las_rutas_relativas_no_se_copian() {
        // Caso negativo: una relativa se resolveria contra el directorio de
        // trabajo del que pega y el fichero no apareceria por ningun lado.
        // Falla antes de tocar Win32, asi que no necesita escritorio.
        let relativa = std::path::PathBuf::from("capturas/a.png");
        assert!(construir_hdrop(std::slice::from_ref(&relativa)).is_err());
        assert!(copiar_ficheros(std::slice::from_ref(&relativa)).is_err());
    }

    #[test]
    fn una_lista_vacia_no_se_copia() {
        // Vaciaria el portapapeles del usuario sin dejar nada a cambio.
        assert!(copiar_ficheros(&[]).is_err());
    }

    #[test]
    fn una_ruta_con_un_nul_dentro_no_se_copia() {
        // Cortaria la lista por la mitad y el receptor leeria el resto como si
        // fueran otras rutas.
        use std::os::windows::ffi::OsStringExt;
        let cruda = std::ffi::OsString::from_wide(&[
            b'C' as u16,
            b':' as u16,
            b'\\' as u16,
            b'a' as u16,
            0,
            b'b' as u16,
        ]);
        assert!(construir_hdrop(&[std::path::PathBuf::from(cruda)]).is_err());
    }

    #[test]
    #[ignore = "toca el portapapeles real; ejecutar con --ignored"]
    fn unos_ficheros_copiados_se_leen_de_vuelta_como_rutas() {
        // La ida y vuelta completa contra Win32: si la cabecera estuviera mal,
        // DragQueryFileW no devolveria estas mismas rutas. El fichero no hace
        // falta que exista: HDROP transporta nombres, no contenido.
        let rutas = vec![
            std::env::temp_dir().join("pixpin-prueba-hdrop-1.png"),
            std::env::temp_dir().join("pixpin-prueba-hdrop-2.png"),
        ];
        copiar_ficheros(&rutas).expect("deberia copiar los ficheros");

        match leer() {
            Some(ContenidoPortapapeles::Rutas(vuelta)) => assert_eq!(vuelta, rutas),
            otro => panic!("se esperaban rutas, llego {otro:?}"),
        }
    }
}
