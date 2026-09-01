//! El motor Direct2D: factorias, dispositivo y contexto sobre el D3D11 de
//! la captura.
//!
//! La decision que sostiene esta fase: D2D dibuja sobre el MISMO dispositivo
//! D3D11 que posee la textura capturada. Asi el fondo del overlay es la
//! textura misma envuelta en un bitmap, sin copia y sin bajar a la CPU. Si
//! fueran dispositivos distintos, harian falta texturas compartidas y
//! sincronizacion — complejidad que no compra nada.
//!
//! `pixpin-render` es L1: recibe `&ID3D11Device` y no sabe de donde sale.

use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_IGNORE, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS, D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_NONE,
    D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1CreateFactory, ID2D1Bitmap1, ID2D1Device,
    ID2D1DeviceContext, ID2D1Factory1,
};
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWriteCreateFactory, IDWriteFactory,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Dxgi::{IDXGIDevice, IDXGISurface};
use windows::core::Interface;

#[derive(Debug, thiserror::Error)]
pub enum ErrorRender {
    #[error("error de Windows al dibujar: {0}")]
    Windows(#[from] windows::core::Error),
    #[error("la textura no expone una superficie DXGI")]
    SinDxgi,
}

/// Color RGBA en coma flotante 0..1, el idioma nativo de Direct2D.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const NEGRO: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const BLANCO: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    /// El azul de acento de la seleccion.
    pub const ACENTO: Color = Color {
        r: 0.13,
        g: 0.55,
        b: 0.95,
        a: 1.0,
    };

    /// El velo que oscurece lo no seleccionado.
    pub fn oscurecido() -> Color {
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.55,
        }
    }

    pub fn a_d2d(self) -> D2D1_COLOR_F {
        D2D1_COLOR_F {
            r: self.r,
            g: self.g,
            b: self.b,
            a: self.a,
        }
    }
}

pub struct MotorRender {
    fabrica: ID2D1Factory1,
    _dispositivo: ID2D1Device,
    contexto: ID2D1DeviceContext,
    dwrite: IDWriteFactory,
}

impl MotorRender {
    pub fn nuevo(d3d: &ID3D11Device) -> Result<Self, ErrorRender> {
        // SAFETY: crear la factoria no tiene precondiciones; single-threaded
        // porque todo el dibujo ocurre en el hilo de interfaz.
        let fabrica: ID2D1Factory1 =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
        let dxgi: IDXGIDevice = d3d.cast().map_err(|_| ErrorRender::SinDxgi)?;
        // SAFETY: `dxgi` es una vista valida del dispositivo del llamante; el
        // conteo de referencias COM lo mantiene vivo mientras viva el motor.
        let dispositivo = unsafe { fabrica.CreateDevice(&dxgi)? };
        // SAFETY: el dispositivo D2D se acaba de crear y esta vivo.
        let contexto =
            unsafe { dispositivo.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)? };
        // SAFETY: crear la factoria compartida de DirectWrite no tiene
        // precondiciones.
        let dwrite: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        Ok(Self {
            fabrica,
            _dispositivo: dispositivo,
            contexto,
            dwrite,
        })
    }

    pub(crate) fn fabrica(&self) -> &ID2D1Factory1 {
        &self.fabrica
    }

    pub fn contexto(&self) -> &ID2D1DeviceContext {
        &self.contexto
    }

    pub fn dwrite(&self) -> &IDWriteFactory {
        &self.dwrite
    }

    /// Envuelve una textura como bitmap de SOLO LECTURA para dibujarla.
    /// No copia pixeles: el bitmap ES la textura.
    pub fn bitmap_desde_textura(&self, t: &ID3D11Texture2D) -> Result<ID2D1Bitmap1, ErrorRender> {
        self.envolver(t, D2D1_BITMAP_OPTIONS_NONE)
    }

    /// Envuelve una textura como DESTINO de dibujo (`SetTarget`).
    pub fn destino_desde_textura(&self, t: &ID3D11Texture2D) -> Result<ID2D1Bitmap1, ErrorRender> {
        self.envolver(t, D2D1_BITMAP_OPTIONS_TARGET)
    }

    /// Envuelve el backbuffer de un swapchain como destino.
    ///
    /// Los buferes de un swapchain exigen `TARGET | CANNOT_DRAW`: no pueden
    /// usarse como fuente de dibujo, y D2D rechaza con E_INVALIDARG el
    /// intento de envolverlos solo como TARGET — se descubrio ejecutando el
    /// test de la superficie, no leyendo documentacion.
    pub fn destino_backbuffer(&self, t: &ID3D11Texture2D) -> Result<ID2D1Bitmap1, ErrorRender> {
        self.envolver(
            t,
            D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        )
    }

    fn envolver(
        &self,
        t: &ID3D11Texture2D,
        opciones: D2D1_BITMAP_OPTIONS,
    ) -> Result<ID2D1Bitmap1, ErrorRender> {
        let superficie: IDXGISurface = t.cast().map_err(|_| ErrorRender::SinDxgi)?;
        let propiedades = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                // Todo destino de dibujo va premultiplicado (los swapchain de
                // composicion lo exigen); las fuentes ignoran el alfa porque
                // la captura no trae un canal alfa util y componer contra esa
                // basura mancharia el velo. La comprobacion es por BIT, no
                // por igualdad: el backbuffer lleva TARGET | CANNOT_DRAW.
                alphaMode: if (opciones.0 & D2D1_BITMAP_OPTIONS_TARGET.0) != 0 {
                    D2D1_ALPHA_MODE_PREMULTIPLIED
                } else {
                    D2D1_ALPHA_MODE_IGNORE
                },
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: opciones,
            colorContext: std::mem::ManuallyDrop::new(None),
        };
        // SAFETY: la superficie procede de la textura del llamante, viva
        // durante la llamada; D2D retiene su propia referencia al crearla.
        let bitmap = unsafe {
            self.contexto
                .CreateBitmapFromDxgiSurface(&superficie, Some(&propiedades))?
        };
        Ok(bitmap)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CPU_ACCESS_READ,
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
        D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING,
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    };
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};

    /// Un dispositivo D3D11 minimo, solo para el test. No se usa el de
    /// pixpin-capture porque render (L1) no puede depender de capture (L2)
    /// ni siquiera en dev-dependencies: el test de capas tambien las mira.
    fn dispositivo_de_prueba() -> (ID3D11Device, ID3D11DeviceContext) {
        let mut d3d = None;
        let mut ctx = None;
        // SAFETY: punteros de salida locales inicializados a None; constantes
        // documentadas en el resto de parametros.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                Default::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d),
                None,
                Some(&mut ctx),
            )
            .expect("el test necesita GPU real");
        }
        (d3d.unwrap(), ctx.unwrap())
    }

    fn textura(d3d: &ID3D11Device, ancho: u32, alto: u32) -> ID3D11Texture2D {
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
        // SAFETY: desc inicializada; t es la variable de salida local.
        unsafe { d3d.CreateTexture2D(&desc, None, Some(&mut t)) }.unwrap();
        t.unwrap()
    }

    /// Lee un pixel BGRA de una textura, via staging. El llamante garantiza
    /// que (x, y) cae dentro de la textura.
    fn pixel(
        d3d: &ID3D11Device,
        ctx: &ID3D11DeviceContext,
        t: &ID3D11Texture2D,
        x: u32,
        y: u32,
    ) -> [u8; 4] {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: GetDesc solo rellena la estructura local.
        unsafe { t.GetDesc(&mut desc) };
        let escenificada_desc = D3D11_TEXTURE2D_DESC {
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
            ..desc
        };
        let mut e = None;
        // SAFETY: desc inicializada; e es la salida local.
        unsafe { d3d.CreateTexture2D(&escenificada_desc, None, Some(&mut e)) }.unwrap();
        let e = e.unwrap();
        // SAFETY: mismo formato y tamano, usos compatibles con CopyResource.
        unsafe { ctx.CopyResource(&e, t) };
        let mut mapa = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: textura staging con lectura; se desmapea justo despues.
        unsafe { ctx.Map(&e, 0, D3D11_MAP_READ, 0, Some(&mut mapa)) }.unwrap();
        let paso = mapa.RowPitch as usize;
        // SAFETY: pData apunta a alto*paso bytes mientras este mapeada; el
        // offset queda dentro porque (x, y) esta dentro de la textura
        // (obligacion del llamante de este helper de test).
        let v = unsafe {
            let base = (mapa.pData as *const u8).add(y as usize * paso + x as usize * 4);
            [*base, *base.add(1), *base.add(2), *base.add(3)]
        };
        // SAFETY: se desmapea lo que se mapeo, una vez.
        unsafe { ctx.Unmap(&e, 0) };
        v
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn dibujar_un_rectangulo_rojo_deja_pixeles_rojos_en_la_textura() {
        let (d3d, ctx) = dispositivo_de_prueba();
        let motor = MotorRender::nuevo(&d3d).expect("deberia crearse el motor D2D");
        let destino = textura(&d3d, 64, 64);
        let objetivo = motor.destino_desde_textura(&destino).unwrap();

        let c = motor.contexto();
        // SAFETY: el contexto D2D esta vivo (lo mantiene `motor`) y las
        // llamadas siguen el protocolo SetTarget/BeginDraw/EndDraw.
        unsafe {
            c.SetTarget(&objetivo);
            c.BeginDraw();
            c.Clear(Some(
                &Color {
                    r: 0.0,
                    g: 0.0,
                    b: 1.0,
                    a: 1.0,
                }
                .a_d2d(),
            ));
            let pincel = c
                .CreateSolidColorBrush(
                    &Color {
                        r: 1.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }
                    .a_d2d(),
                    None,
                )
                .unwrap();
            c.FillRectangle(
                &windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                    left: 8.0,
                    top: 8.0,
                    right: 32.0,
                    bottom: 32.0,
                },
                &pincel,
            );
            c.EndDraw(None, None).unwrap();
            c.SetTarget(None);
        }

        // Dentro del rectangulo: rojo (BGRA = 0,0,255). Fuera: azul.
        assert_eq!(pixel(&d3d, &ctx, &destino, 16, 16), [0, 0, 255, 255]);
        // Caso negativo: si Clear no funcionara o el rect lo cubriera todo,
        // este pixel tambien seria rojo.
        assert_eq!(pixel(&d3d, &ctx, &destino, 50, 50), [255, 0, 0, 255]);
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn un_bitmap_de_solo_lectura_envuelve_la_textura_sin_copiarla() {
        let (d3d, _ctx) = dispositivo_de_prueba();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        let t = textura(&d3d, 32, 32);
        let bitmap = motor.bitmap_desde_textura(&t).expect("deberia envolverse");
        // SAFETY: GetSize es una lectura sin precondiciones sobre un bitmap vivo.
        let tam = unsafe { bitmap.GetSize() };
        assert_eq!((tam.width as u32, tam.height as u32), (32, 32));
    }
}
