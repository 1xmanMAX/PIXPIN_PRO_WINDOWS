//! Un destino de dibujo sin ventana, y esperar a que la GPU acabe.
//!
//! Para medir un fotograma entero desde una prueba: sin esto, quien no puede
//! escribir `unsafe` (la aplicacion) no tiene donde pintar sin abrir una
//! ventana, y sin la espera solo se mediria lo que tarda la CPU en encargar
//! el trabajo, no lo que tarda en hacerse.

use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_QUERY_DESC, D3D11_QUERY_EVENT,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, ID3D11Device, ID3D11Query, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};

use crate::{ErrorRender, MotorRender};

/// Una textura de `ancho` x `alto` envuelta como destino de dibujo.
pub struct FueraDePantalla {
    _textura: ID3D11Texture2D,
    pub destino: ID2D1Bitmap1,
    d3d: ID3D11Device,
}

impl FueraDePantalla {
    pub fn nuevo(
        motor: &MotorRender,
        d3d: &ID3D11Device,
        ancho: u32,
        alto: u32,
    ) -> Result<Self, ErrorRender> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: ancho,
            Height: alto,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
            ..Default::default()
        };
        let mut t = None;
        // SAFETY: `desc` esta inicializada y `t` es la salida local.
        unsafe { d3d.CreateTexture2D(&desc, None, Some(&mut t))? };
        let textura = t.ok_or(ErrorRender::SinDxgi)?;
        let destino = motor.destino_desde_textura(&textura)?;
        Ok(Self {
            _textura: textura,
            destino,
            d3d: d3d.clone(),
        })
    }

    /// Los pixeles pintados, RGBA sin premultiplicar y fila tras fila: para
    /// mirar a ojo lo que se midio (un PNG), no para medir.
    pub fn leer_rgba(&self) -> Result<(u32, u32, Vec<u8>), ErrorRender> {
        use windows::Win32::Graphics::Direct3D11::{
            D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_USAGE_STAGING,
        };
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: GetDesc rellena la estructura local.
        unsafe { self._textura.GetDesc(&mut desc) };
        let copia_desc = D3D11_TEXTURE2D_DESC {
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
            ..desc
        };
        let mut copia = None;
        // SAFETY: `copia_desc` inicializada; `copia` es la salida local.
        unsafe {
            self.d3d
                .CreateTexture2D(&copia_desc, None, Some(&mut copia))?
        };
        let copia = copia.ok_or(ErrorRender::SinDxgi)?;
        // SAFETY: contexto del dispositivo vivo; mismo formato y tamano.
        let ctx = unsafe { self.d3d.GetImmediateContext()? };
        // SAFETY: las dos texturas son de este dispositivo y del mismo tamano.
        unsafe { ctx.CopyResource(&copia, &self._textura) };
        let mut mapa = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: textura de lectura; se desmapea antes de salir.
        unsafe { ctx.Map(&copia, 0, D3D11_MAP_READ, 0, Some(&mut mapa))? };
        let (w, h) = (desc.Width, desc.Height);
        let mut v = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            // SAFETY: cada fila tiene `RowPitch` bytes y al menos `w * 4`
            // utiles; el mapa sigue abierto.
            let fila = unsafe {
                std::slice::from_raw_parts(
                    (mapa.pData as *const u8).add((y * mapa.RowPitch) as usize),
                    (w * 4) as usize,
                )
            };
            for p in fila.chunks_exact(4) {
                let a = p[3] as u32;
                let des = |c: u8| (c as u32 * 255).checked_div(a).unwrap_or(0).min(255) as u8;
                v.extend_from_slice(&[des(p[2]), des(p[1]), des(p[0]), p[3]]);
            }
        }
        // SAFETY: empareja el Map de arriba.
        unsafe { ctx.Unmap(&copia, 0) };
        Ok((w, h, v))
    }

    /// Bloquea hasta que la GPU haya terminado todo lo encargado hasta
    /// ahora. Una consulta de evento y no leer un pixel: leer obliga a una
    /// copia que tambien se mediria.
    pub fn esperar_gpu(&self) -> Result<(), ErrorRender> {
        let desc = D3D11_QUERY_DESC {
            Query: D3D11_QUERY_EVENT,
            MiscFlags: 0,
        };
        let mut q: Option<ID3D11Query> = None;
        // SAFETY: `desc` inicializada; `q` es la salida local.
        unsafe { self.d3d.CreateQuery(&desc, Some(&mut q))? };
        let q = q.ok_or(ErrorRender::SinDxgi)?;
        // SAFETY: el contexto inmediato es del dispositivo vivo; la consulta
        // es suya.
        let ctx = unsafe { self.d3d.GetImmediateContext()? };
        // SAFETY: `End` sobre una consulta de evento recien creada.
        unsafe { ctx.End(&q) };
        loop {
            let mut hecho: i32 = 0;
            // SAFETY: `hecho` es un BOOL local del tamano que se pasa; sin
            // la bandera DONOTFLUSH, GetData manda el trabajo pendiente.
            let r = unsafe {
                ctx.GetData(
                    &q,
                    Some(&mut hecho as *mut i32 as *mut _),
                    std::mem::size_of::<i32>() as u32,
                    0,
                )
            };
            if r.is_ok() && hecho != 0 {
                return Ok(());
            }
            std::thread::yield_now();
        }
    }
}
