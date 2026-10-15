//! Bajada puntual de una textura a memoria de sistema.
//!
//! Se llama **una sola vez por captura**, cuando el usuario guarda o copia.
//! Todo lo demas ocurre en la GPU.
//!
//! El paso delicado es el relleno de fila: D3D11 alinea cada fila a un
//! `RowPitch` que casi nunca coincide con `ancho * 4`. Copiar el buffer de
//! golpe produce una imagen inclinada, que es un fallo clasico y muy visible.

use pixpin_codec::ImagenRgba;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_STAGING, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};

use crate::dispositivo::{Dispositivo, ErrorCaptura};
use crate::instantanea::Instantanea;

/// Copia la instantanea a memoria de sistema como RGBA sin relleno.
pub fn a_imagen(dispositivo: &Dispositivo, inst: &Instantanea) -> Result<ImagenRgba, ErrorCaptura> {
    textura_a_imagen(dispositivo.d3d(), dispositivo.contexto(), inst.textura())
}

/// Lo mismo sobre una textura suelta. Existe para el pin en vivo, que guarda
/// su ultimo fotograma en una textura propia y lo baja solo cuando el
/// usuario lo copia o lo congela, lejos de donde vive el `Dispositivo`.
pub fn textura_a_imagen(
    d3d: &ID3D11Device,
    contexto: &ID3D11DeviceContext,
    textura: &ID3D11Texture2D,
) -> Result<ImagenRgba, ErrorCaptura> {
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    // SAFETY: el llamante mantiene viva la textura durante toda la funcion;
    // `GetDesc` solo rellena la estructura que se le pasa.
    unsafe { textura.GetDesc(&mut desc) };

    // Una textura de escenificacion: la GPU no dibuja en ella, pero la CPU
    // puede leerla. Es el unico modo de sacar pixeles de la memoria de video.
    let desc_lectura = D3D11_TEXTURE2D_DESC {
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
        ..desc
    };

    let mut escenificada: Option<ID3D11Texture2D> = None;
    // SAFETY: `desc_lectura` esta completamente inicializada y `escenificada`
    // es una variable local que la API rellena en caso de exito.
    unsafe { d3d.CreateTexture2D(&desc_lectura, None, Some(&mut escenificada))? };
    let escenificada = escenificada.ok_or(ErrorCaptura::SinFotograma)?;

    // SAFETY: origen y destino tienen la misma descripcion salvo el uso y los
    // permisos de CPU, que es exactamente lo que `CopyResource` admite.
    unsafe { contexto.CopyResource(&escenificada, textura) };

    let mut mapa = D3D11_MAPPED_SUBRESOURCE::default();
    // SAFETY: la textura es de escenificacion con acceso de lectura, asi que
    // mapearla para lectura es valido. Se desmapea sin falta mas abajo, y
    // entre medias no hay ningun `?` que pueda saltarselo.
    unsafe {
        contexto.Map(&escenificada, 0, D3D11_MAP_READ, 0, Some(&mut mapa))?;
    }

    let ancho = desc.Width as usize;
    let alto = desc.Height as usize;
    let paso = mapa.RowPitch as usize;
    let mut pixeles = Vec::with_capacity(ancho * alto * 4);

    // SAFETY: `mapa.pData` apunta a `alto * paso` bytes legibles mientras la
    // textura este mapeada, y no se sale de ese rango: se lee fila a fila,
    // tomando solo `ancho * 4` bytes de cada una y saltando el relleno.
    unsafe {
        let base = mapa.pData as *const u8;
        for fila in 0..alto {
            let inicio = base.add(fila * paso);
            let franja = std::slice::from_raw_parts(inicio, ancho * 4);
            // La GPU entrega BGRA; `ImagenRgba` espera RGBA. Se intercambian
            // los canales rojo y azul al copiar.
            for pixel in franja.chunks_exact(4) {
                pixeles.push(pixel[2]);
                pixeles.push(pixel[1]);
                pixeles.push(pixel[0]);
                pixeles.push(pixel[3]);
            }
        }
    }

    // SAFETY: se desmapea exactamente el mismo subrecurso que se mapeo arriba,
    // y solo una vez.
    unsafe { contexto.Unmap(&escenificada, 0) };

    Ok(ImagenRgba {
        ancho: desc.Width,
        alto: desc.Height,
        pixeles,
    })
}
