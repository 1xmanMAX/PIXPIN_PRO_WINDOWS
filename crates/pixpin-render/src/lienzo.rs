//! El pintor seguro: la unica cara de Direct2D que ven las capas sin unsafe.
//!
//! `apps/pixpin` es `forbid(unsafe_code)` y aun asi orquesta todo el dibujo
//! del overlay. Este modulo le da primitivas seguras —rellenar, trazar,
//! bitmap, texto— y encierra el protocolo SetTarget/BeginDraw/EndDraw en
//! una unica funcion con clausura, donde no se puede olvidar ningun paso.

use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    D2D1_CAP_STYLE_FLAT, D2D1_DASH_STYLE_DASH, D2D1_INTERPOLATION_MODE_LINEAR,
    D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR, D2D1_LINE_JOIN_MITER, D2D1_ROUNDED_RECT,
    D2D1_STROKE_STYLE_PROPERTIES1, ID2D1Bitmap1, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_TEXT_METRICS, IDWriteTextFormat, IDWriteTextLayout,
};
use windows::core::Interface;
use windows::core::w;
use windows_numerics::Vector2;

use crate::motor::{Color, ErrorRender, MotorRender};

/// Rectangulo en coordenadas de dibujo (pixeles del destino, coma flotante).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectF {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
}

impl RectF {
    pub fn desde(r: pixpin_geom_compat::Rect) -> RectF {
        RectF {
            x: r.0 as f32,
            y: r.1 as f32,
            ancho: r.2 as f32,
            alto: r.3 as f32,
        }
    }

    fn a_d2d(self) -> D2D_RECT_F {
        D2D_RECT_F {
            left: self.x,
            top: self.y,
            right: self.x + self.ancho,
            bottom: self.y + self.alto,
        }
    }
}

/// Compatibilidad sin dependencia: (x, y, ancho, alto). `pixpin-render` no
/// depende de `pixpin-geom` a proposito (son de la misma capa L0/L1 en
/// espiritu distinto); quien tiene un Rect de geom lo convierte en tupla.
pub mod pixpin_geom_compat {
    pub type Rect = (i32, i32, u32, u32);
}

impl MotorRender {
    /// Dibuja sobre `destino` con el protocolo completo encerrado: SetTarget,
    /// BeginDraw, la clausura, EndDraw y SetTarget(None). El error de EndDraw
    /// se devuelve; el destino queda siempre desligado.
    pub fn dibujar(
        &self,
        destino: &ID2D1Bitmap1,
        pintar: impl FnOnce(&Pintor),
    ) -> Result<(), ErrorRender> {
        let c = self.contexto();
        // SAFETY: protocolo documentado de D2D sobre un contexto vivo; el
        // SetTarget(None) final corre tanto en exito como en error de
        // EndDraw, porque va antes del `?`.
        unsafe {
            c.SetTarget(destino);
            c.BeginDraw();
        }
        let pintor = Pintor { motor: self };
        pintar(&pintor);
        // SAFETY: cierra el BeginDraw de arriba; None desliga el destino.
        let fin = unsafe { c.EndDraw(None, None) };
        // SAFETY: desligar siempre, tambien si EndDraw fallo.
        unsafe { c.SetTarget(None) };
        fin?;
        Ok(())
    }
}

/// Primitivas de dibujo validas SOLO dentro de `MotorRender::dibujar`.
pub struct Pintor<'a> {
    motor: &'a MotorRender,
}

impl Pintor<'_> {
    fn pincel(&self, color: Color) -> Option<ID2D1SolidColorBrush> {
        // SAFETY: crear un pincel sobre el contexto vivo no tiene mas
        // precondiciones; se usa y se suelta dentro del fotograma.
        unsafe {
            self.motor
                .contexto()
                .CreateSolidColorBrush(&color.a_d2d(), None)
                .ok()
        }
    }

    pub fn limpiar(&self, color: Color) {
        // SAFETY: Clear dentro de BeginDraw/EndDraw (lo garantiza `dibujar`).
        unsafe { self.motor.contexto().Clear(Some(&color.a_d2d())) };
    }

    /// Limpia a transparente total: el fondo de una ventana de composicion.
    pub fn limpiar_transparente(&self) {
        self.limpiar(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        });
    }

    pub fn rellenar(&self, r: RectF, color: Color) {
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; pincel y contexto vivos.
            unsafe { self.motor.contexto().FillRectangle(&r.a_d2d(), &p) };
        }
    }

    pub fn rellenar_redondeado(&self, r: RectF, radio: f32, color: Color) {
        if let Some(p) = self.pincel(color) {
            let rr = D2D1_ROUNDED_RECT {
                rect: r.a_d2d(),
                radiusX: radio,
                radiusY: radio,
            };
            // SAFETY: dentro del fotograma; pincel y contexto vivos.
            unsafe { self.motor.contexto().FillRoundedRectangle(&rr, &p) };
        }
    }

    pub fn trazar(&self, r: RectF, grosor: f32, color: Color) {
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; pincel y contexto vivos.
            unsafe {
                self.motor
                    .contexto()
                    .DrawRectangle(&r.a_d2d(), &p, grosor, None)
            };
        }
    }

    pub fn trazar_discontinuo(&self, r: RectF, grosor: f32, color: Color) {
        let estilo: Option<ID2D1StrokeStyle> = {
            let propiedades = D2D1_STROKE_STYLE_PROPERTIES1 {
                startCap: D2D1_CAP_STYLE_FLAT,
                endCap: D2D1_CAP_STYLE_FLAT,
                dashCap: D2D1_CAP_STYLE_FLAT,
                lineJoin: D2D1_LINE_JOIN_MITER,
                miterLimit: 10.0,
                dashStyle: D2D1_DASH_STYLE_DASH,
                dashOffset: 0.0,
                ..Default::default()
            };
            // SAFETY: la factoria vive en el motor; crear un estilo de trazo
            // no tiene precondiciones. El cast es el upcast StrokeStyle1 ->
            // StrokeStyle, que DrawRectangle espera.
            unsafe {
                self.motor
                    .fabrica()
                    .CreateStrokeStyle(&propiedades, None)
                    .ok()
                    .and_then(|e| e.cast::<ID2D1StrokeStyle>().ok())
            }
        };
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; objetos vivos. StrokeStyle1
            // hereda de StrokeStyle, que es lo que DrawRectangle espera.
            unsafe {
                self.motor
                    .contexto()
                    .DrawRectangle(&r.a_d2d(), &p, grosor, estilo.as_ref())
            };
        }
    }

    pub fn linea(&self, desde: (f32, f32), hasta: (f32, f32), grosor: f32, color: Color) {
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; pincel y contexto vivos.
            unsafe {
                self.motor.contexto().DrawLine(
                    Vector2 {
                        X: desde.0,
                        Y: desde.1,
                    },
                    Vector2 {
                        X: hasta.0,
                        Y: hasta.1,
                    },
                    &p,
                    grosor,
                    None,
                )
            };
        }
    }

    /// Dibuja un bitmap. `nitido` usa vecino mas cercano (la lupa: pixeles
    /// reales); si no, interpolacion lineal (reescalados suaves).
    pub fn bitmap(&self, b: &ID2D1Bitmap1, destino: RectF, fuente: Option<RectF>, nitido: bool) {
        let modo = if nitido {
            D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR
        } else {
            D2D1_INTERPOLATION_MODE_LINEAR
        };
        let fuente_d2d = fuente.map(|f| f.a_d2d());
        // SAFETY: dentro del fotograma; bitmap del mismo dispositivo D2D
        // (obligacion del llamante: todos los bitmaps salen de este motor).
        unsafe {
            self.motor.contexto().DrawBitmap(
                b,
                Some(&destino.a_d2d()),
                1.0,
                modo,
                fuente_d2d.as_ref().map(|f| f as *const _),
                None,
            )
        };
    }

    fn formato(&self, tam: f32) -> Option<IDWriteTextFormat> {
        // SAFETY: la factoria DirectWrite vive en el motor; Segoe UI existe
        // en todo Windows soportado y si faltara, el Err corta el texto.
        unsafe {
            self.motor
                .dwrite()
                .CreateTextFormat(
                    w!("Segoe UI"),
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    tam,
                    w!("es-ES"),
                )
                .ok()
        }
    }

    fn disposicion(&self, texto: &str, tam: f32) -> Option<(IDWriteTextLayout, f32, f32)> {
        let formato = self.formato(tam)?;
        let contenido: Vec<u16> = texto.encode_utf16().collect();
        // SAFETY: la disposicion copia el texto; medir no tiene mas
        // precondiciones que punteros validos durante la llamada.
        unsafe {
            let disposicion = self
                .motor
                .dwrite()
                .CreateTextLayout(&contenido, &formato, f32::MAX, f32::MAX)
                .ok()?;
            let mut metricas = DWRITE_TEXT_METRICS::default();
            disposicion.GetMetrics(&mut metricas).ok()?;
            Some((disposicion, metricas.width, metricas.height))
        }
    }

    /// Mide el texto sin dibujarlo: para colocar cajas.
    pub fn medir_texto(&self, texto: &str, tam: f32) -> (f32, f32) {
        self.disposicion(texto, tam)
            .map(|(_, w, h)| (w, h))
            .unwrap_or((0.0, 0.0))
    }

    pub fn texto(&self, texto: &str, x: f32, y: f32, tam: f32, color: Color) {
        let Some((disposicion, _, _)) = self.disposicion(texto, tam) else {
            return;
        };
        if let Some(p) = self.pincel(color) {
            // SAFETY: dentro del fotograma; objetos vivos.
            unsafe {
                self.motor.contexto().DrawTextLayout(
                    Vector2 { X: x, Y: y },
                    &disposicion,
                    &p,
                    Default::default(),
                )
            };
        }
    }

    /// Texto sobre una caja redondeada semitransparente: las etiquetas del
    /// overlay (dimensiones, color de la lupa).
    pub fn texto_con_fondo(
        &self,
        texto: &str,
        x: f32,
        y: f32,
        tam: f32,
        color_texto: Color,
        color_fondo: Color,
    ) {
        let (ancho, alto) = self.medir_texto(texto, tam);
        let relleno = tam * 0.4;
        self.rellenar_redondeado(
            RectF {
                x: x - relleno,
                y: y - relleno * 0.5,
                ancho: ancho + relleno * 2.0,
                alto: alto + relleno,
            },
            tam * 0.25,
            color_fondo,
        );
        self.texto(texto, x, y, tam, color_texto);
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

    fn dispositivo() -> (ID3D11Device, ID3D11DeviceContext) {
        let mut d3d = None;
        let mut ctx = None;
        // SAFETY: salidas locales, constantes documentadas.
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
            .expect("GPU real");
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
        // SAFETY: desc inicializada; t es la salida local.
        unsafe { d3d.CreateTexture2D(&desc, None, Some(&mut t)) }.unwrap();
        t.unwrap()
    }

    fn pixel(
        d3d: &ID3D11Device,
        ctx: &ID3D11DeviceContext,
        t: &ID3D11Texture2D,
        x: u32,
        y: u32,
    ) -> [u8; 4] {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: GetDesc rellena la estructura local.
        unsafe { t.GetDesc(&mut desc) };
        let e_desc = D3D11_TEXTURE2D_DESC {
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
            ..desc
        };
        let mut e = None;
        // SAFETY: desc inicializada; e es la salida local.
        unsafe { d3d.CreateTexture2D(&e_desc, None, Some(&mut e)) }.unwrap();
        let e = e.unwrap();
        // SAFETY: mismo formato/tamano; staging legible tras CopyResource.
        unsafe { ctx.CopyResource(&e, t) };
        let mut mapa = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: staging con lectura; se desmapea justo despues.
        unsafe { ctx.Map(&e, 0, D3D11_MAP_READ, 0, Some(&mut mapa)) }.unwrap();
        // SAFETY: (x, y) dentro de la textura, obligacion del llamante.
        let v = unsafe {
            let base =
                (mapa.pData as *const u8).add(y as usize * mapa.RowPitch as usize + x as usize * 4);
            [*base, *base.add(1), *base.add(2), *base.add(3)]
        };
        // SAFETY: empareja el Map de arriba.
        unsafe { ctx.Unmap(&e, 0) };
        v
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn el_pintor_seguro_dibuja_sin_una_linea_de_unsafe_en_el_llamante() {
        let (d3d, ctx) = dispositivo();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        let destino_tex = textura(&d3d, 128, 128);
        let destino = motor.destino_desde_textura(&destino_tex).unwrap();

        // Este bloque es exactamente lo que hara apps/pixpin: cero unsafe.
        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color {
                    r: 0.0,
                    g: 0.0,
                    b: 1.0,
                    a: 1.0,
                });
                p.rellenar(
                    RectF {
                        x: 8.0,
                        y: 8.0,
                        ancho: 24.0,
                        alto: 24.0,
                    },
                    Color {
                        r: 1.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                );
                p.rellenar_redondeado(
                    RectF {
                        x: 60.0,
                        y: 60.0,
                        ancho: 40.0,
                        alto: 20.0,
                    },
                    4.0,
                    Color::BLANCO,
                );
                p.trazar_discontinuo(
                    RectF {
                        x: 40.0,
                        y: 8.0,
                        ancho: 30.0,
                        alto: 30.0,
                    },
                    2.0,
                    Color::ACENTO,
                );
                p.linea((0.0, 120.0), (128.0, 120.0), 2.0, Color::BLANCO);
                p.texto("42", 100.0, 8.0, 12.0, Color::BLANCO);
                let (w, h) = p.medir_texto("970x542", 14.0);
                assert!(w > 10.0 && h > 5.0, "medir_texto devolvio ({w}, {h})");
            })
            .expect("el fotograma completo deberia dibujarse");

        // Rojo dentro del rectangulo, azul fuera: la tuberia entera funciona.
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 16, 16), [0, 0, 255, 255]);
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 110, 110), [255, 0, 0, 255]);
        // La caja blanca redondeada dejo su centro blanco.
        assert_eq!(
            pixel(&d3d, &ctx, &destino_tex, 80, 70),
            [255, 255, 255, 255]
        );
    }
}
