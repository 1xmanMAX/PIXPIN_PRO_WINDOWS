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
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
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

/// **Bajadas seguidas de una textura**, para grabar video (25-oct-2026,
/// grabar la reunion entera).
///
/// [`textura_a_imagen`] crea su textura de escenificacion en cada llamada y
/// cambia los canales a RGBA: bien para una captura suelta, caro diez veces
/// por segundo durante una hora en un portatil con 4 GB. Aqui la textura de
/// escenificacion se crea una vez y se reutiliza mientras no cambie el
/// tamano, y los pixeles salen **BGRA tal cual**, que es lo que come el
/// codificador de video (lo que Media Foundation llama RGB32).
#[derive(Default)]
pub struct Bajada {
    escenificada: Option<(ID3D11Texture2D, u32, u32)>,
}

impl Bajada {
    /// Copia `textura` a `destino` como BGRA sin relleno de fila y devuelve
    /// su `(ancho, alto)`. `destino` se reutiliza: solo crece si hace falta.
    pub fn bajar_bgra(
        &mut self,
        dispositivo: &Dispositivo,
        textura: &ID3D11Texture2D,
        destino: &mut Vec<u8>,
    ) -> Result<(u32, u32), ErrorCaptura> {
        let d3d = dispositivo.d3d();
        let contexto = dispositivo.contexto();
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: el llamante mantiene viva la textura durante la funcion;
        // `GetDesc` solo rellena la estructura que se le pasa.
        unsafe { textura.GetDesc(&mut desc) };
        // Cuatro bytes por pixel en orden BGRA: es lo que se copia abajo.
        // Otro formato (el de 16 bits por canal de una pantalla HDR) daria
        // un fotograma revuelto; mejor decir que no hay fotograma.
        if desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM
            && desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM_SRGB
        {
            return Err(ErrorCaptura::SinFotograma);
        }
        let vale = self
            .escenificada
            .as_ref()
            .is_some_and(|(_, w, h)| (*w, *h) == (desc.Width, desc.Height));
        if !vale {
            let desc_lectura = D3D11_TEXTURE2D_DESC {
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
                ..desc
            };
            let mut nueva: Option<ID3D11Texture2D> = None;
            // SAFETY: descripcion completa; `nueva` es local y la API la
            // rellena si sale bien.
            unsafe { d3d.CreateTexture2D(&desc_lectura, None, Some(&mut nueva))? };
            let nueva = nueva.ok_or(ErrorCaptura::SinFotograma)?;
            self.escenificada = Some((nueva, desc.Width, desc.Height));
        }
        let Some((escenificada, _, _)) = self.escenificada.as_ref() else {
            return Err(ErrorCaptura::SinFotograma);
        };

        // SAFETY: misma descripcion salvo uso y permisos de CPU, que es lo
        // que `CopyResource` admite (comprobado arriba por el tamano; el
        // formato de una sesion no cambia).
        unsafe { contexto.CopyResource(escenificada, textura) };
        let mut mapa = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: textura de escenificacion con lectura de CPU; se desmapea
        // mas abajo sin ningun `?` entre medias.
        unsafe { contexto.Map(escenificada, 0, D3D11_MAP_READ, 0, Some(&mut mapa))? };

        let ancho = desc.Width as usize;
        let alto = desc.Height as usize;
        let paso = mapa.RowPitch as usize;
        let fila_util = ancho * 4;
        destino.clear();
        destino.reserve(fila_util * alto);
        // SAFETY: `mapa.pData` apunta a `alto * paso` bytes legibles mientras
        // dure el mapeo; se lee fila a fila solo lo util de cada una.
        unsafe {
            let base = mapa.pData as *const u8;
            for fila in 0..alto {
                let franja = std::slice::from_raw_parts(base.add(fila * paso), fila_util);
                destino.extend_from_slice(franja);
            }
        }
        // SAFETY: el mismo subrecurso que se mapeo arriba, una sola vez.
        unsafe { contexto.Unmap(escenificada, 0) };
        Ok((desc.Width, desc.Height))
    }
}
