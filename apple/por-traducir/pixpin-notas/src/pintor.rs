//! **Con que se pinta el marco y los menus**: Direct2D sobre el `HDC` de la
//! ventana (`ID2D1DCRenderTarget`) para las formas y los iconos, suaves y
//! finos como los de la captura, y GDI para las letras, con la misma Work
//! Sans que lleva el texto de la nota (registrada para el proceso en
//! `letras`). Primero las formas, luego las letras encima.
//!
//! Un objetivo de dibujo sobre un `HDC` sirve igual para la pantalla
//! (`WM_PAINT`) que para un mapa de bits en memoria: las muestras en PNG
//! salen por el mismo camino que lo que se ve.
//!
//! Los iconos estan dibujados en una rejilla de 16x16 y se escalan a su
//! caja; son de trazo, como los de la captura.

use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::*;
use windows_numerics::Vector2;

use crate::disposicion::Caja;
use crate::tema::{Rgb, bgr};

/// Los dibujos de la barra y de los menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icono {
    Tabla,
    Imagen,
    Casillas,
    Mas,
    Flecha,
    Compartir,
    PantallaCompleta,
    Minimizar,
    Cerrar,
    Lista,
    Numerada,
    Fecha,
    Cita,
    Codigo,
    Separador,
    Lapiz,
    Disco,
    Copia,
    /// Una hoja de un proyecto que se actualiza (pagina viva).
    Pagina,
    /// Un enlace a una hoja.
    Enlace,
    // Los comentarios (`panel_comentarios`).
    /// El globo de los comentarios (el boton del panel).
    Comentario,
    /// El globo con un «+» (comentar lo elegido).
    Comentar,
    /// Resolver un comentario.
    Hecho,
    /// Ver u ocultar los resueltos.
    Filtro,
    /// Mas cosas (editar, borrar).
    Puntos,
    // Los incrustados (`incrustados`).
    /// Un documento: la hoja con la esquina doblada.
    Documento,
    /// Un mensaje del chat: el globo.
    Chat,
    /// Un audio: la onda.
    Audio,
}

pub struct Pintor {
    objetivo: ID2D1DCRenderTarget,
    trazo: Option<ID2D1StrokeStyle>,
    pub escala: f32,
    /// La letra de los rotulos (Work Sans) a 13 px y a 12 px.
    pub letra: HFONT,
    pub letra_chica: HFONT,
    pub letra_negrita: HFONT,
}

fn color(c: Rgb) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((c >> 16) & 0xff) as f32 / 255.0,
        g: ((c >> 8) & 0xff) as f32 / 255.0,
        b: (c & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

fn punto(x: f32, y: f32) -> Vector2 {
    Vector2 { X: x, Y: y }
}

/// Una letra de GDI por su nombre y su alto en pixeles.
pub fn letra_gdi(nombre: &str, alto_px: i32, negrita: bool) -> HFONT {
    let mut lf = LOGFONTW {
        lfHeight: -alto_px,
        lfWeight: if negrita { 600 } else { 400 },
        lfQuality: CLEARTYPE_QUALITY,
        lfCharSet: DEFAULT_CHARSET,
        ..Default::default()
    };
    for (i, c) in nombre.encode_utf16().take(31).enumerate() {
        lf.lfFaceName[i] = c;
    }
    // SAFETY: la estructura es local y completa.
    unsafe { CreateFontIndirectW(&lf) }
}

impl Pintor {
    pub fn nuevo(escala: f32, letra_ui: &str) -> Option<Pintor> {
        // SAFETY: crear la fabrica y el objetivo no toma punteros ajenos; las
        // propiedades son locales.
        unsafe {
            let fabrica: ID2D1Factory =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).ok()?;
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_IGNORE,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                ..Default::default()
            };
            let objetivo = fabrica.CreateDCRenderTarget(&props).ok()?;
            let trazo = fabrica
                .CreateStrokeStyle(
                    &D2D1_STROKE_STYLE_PROPERTIES {
                        startCap: D2D1_CAP_STYLE_ROUND,
                        endCap: D2D1_CAP_STYLE_ROUND,
                        dashCap: D2D1_CAP_STYLE_ROUND,
                        lineJoin: D2D1_LINE_JOIN_ROUND,
                        miterLimit: 10.0,
                        ..Default::default()
                    },
                    None,
                )
                .ok();
            let px = |v: f32| (v * escala).round() as i32;
            Some(Pintor {
                objetivo,
                trazo,
                escala,
                letra: letra_gdi(letra_ui, px(13.5), false),
                letra_chica: letra_gdi(letra_ui, px(12.0), false),
                letra_negrita: letra_gdi(letra_ui, px(13.0), true),
            })
        }
    }

    /// Las formas: `dibujar` pinta con el pintor atado a `hdc` en `zona`.
    pub fn formas(&self, hdc: HDC, zona: RECT, dibujar: impl FnOnce(&Formas)) {
        // SAFETY: el HDC es el de quien pinta y vive durante la llamada; el
        // objetivo se ata a el solo entre BeginDraw y EndDraw.
        unsafe {
            if self.objetivo.BindDC(hdc, &zona).is_err() {
                return;
            }
            self.objetivo.BeginDraw();
            self.objetivo
                .SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
            let f = Formas {
                rt: &self.objetivo,
                trazo: self.trazo.as_ref(),
                escala: self.escala,
                dx: -(zona.left as f32),
                dy: -(zona.top as f32),
            };
            dibujar(&f);
            let _ = self.objetivo.EndDraw(None, None);
        }
    }

    /// Lo que mide a lo ancho un texto con una letra.
    pub fn medir(&self, hdc: HDC, letra: HFONT, texto: &str) -> i32 {
        let t: Vec<u16> = texto.encode_utf16().collect();
        let mut s = windows::Win32::Foundation::SIZE::default();
        // SAFETY: HDC y letra vivos; se devuelve la letra que habia.
        unsafe {
            let vieja = SelectObject(hdc, HGDIOBJ(letra.0));
            let _ = GetTextExtentPoint32W(hdc, &t, &mut s);
            SelectObject(hdc, vieja);
        }
        s.cx
    }

    /// Un texto en una caja: centrado en alto; `centrado` tambien a lo ancho.
    /// Lo que no cabe acaba en «…».
    pub fn texto(
        &self,
        hdc: HDC,
        letra: HFONT,
        texto: &str,
        caja: Caja,
        tinta: Rgb,
        centrado: bool,
    ) {
        // `DrawTextW` con `DT_END_ELLIPSIS` y un texto vacio se sale de la
        // memoria (visto el 30-sep con un rotulo sin traducir): nada que pintar.
        if texto.is_empty() {
            return;
        }
        let mut t: Vec<u16> = texto.encode_utf16().collect();
        let mut r = RECT {
            left: caja.x,
            top: caja.y,
            right: caja.derecha(),
            bottom: caja.abajo(),
        };
        let formato = DT_SINGLELINE
            | DT_VCENTER
            | DT_END_ELLIPSIS
            | DT_NOPREFIX
            | if centrado { DT_CENTER } else { DT_LEFT };
        // SAFETY: HDC y letra vivos; el texto es un bufer propio.
        unsafe {
            let vieja = SelectObject(hdc, HGDIOBJ(letra.0));
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, COLORREF(bgr(tinta)));
            DrawTextW(hdc, &mut t, &mut r, formato);
            SelectObject(hdc, vieja);
        }
    }
}

impl Drop for Pintor {
    fn drop(&mut self) {
        // SAFETY: letras propias, creadas en `nuevo`.
        unsafe {
            let _ = DeleteObject(HGDIOBJ(self.letra.0));
            let _ = DeleteObject(HGDIOBJ(self.letra_chica.0));
            let _ = DeleteObject(HGDIOBJ(self.letra_negrita.0));
        }
    }
}

/// El pincel de formas, valido dentro de [`Pintor::formas`].
pub struct Formas<'a> {
    rt: &'a ID2D1DCRenderTarget,
    trazo: Option<&'a ID2D1StrokeStyle>,
    escala: f32,
    dx: f32,
    dy: f32,
}

impl Formas<'_> {
    fn pincel(&self, c: Rgb) -> Option<ID2D1SolidColorBrush> {
        // SAFETY: dentro de BeginDraw/EndDraw; el color es local.
        unsafe { self.rt.CreateSolidColorBrush(&color(c), None).ok() }
    }

    pub fn rect(&self, caja: Caja, c: Rgb) {
        self.redondo(caja, 0.0, c);
    }

    pub fn redondo(&self, caja: Caja, radio: f32, c: Rgb) {
        let Some(p) = self.pincel(c) else { return };
        let rr = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: caja.x as f32 + self.dx,
                top: caja.y as f32 + self.dy,
                right: caja.derecha() as f32 + self.dx,
                bottom: caja.abajo() as f32 + self.dy,
            },
            radiusX: radio * self.escala,
            radiusY: radio * self.escala,
        };
        // SAFETY: dentro del dibujo; pincel vivo.
        unsafe { self.rt.FillRoundedRectangle(&rr, &p) };
    }

    pub fn borde(&self, caja: Caja, radio: f32, grosor: f32, c: Rgb) {
        let Some(p) = self.pincel(c) else { return };
        let m = grosor / 2.0;
        let rr = D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: caja.x as f32 + self.dx + m,
                top: caja.y as f32 + self.dy + m,
                right: caja.derecha() as f32 + self.dx - m,
                bottom: caja.abajo() as f32 + self.dy - m,
            },
            radiusX: radio * self.escala,
            radiusY: radio * self.escala,
        };
        // SAFETY: como arriba.
        unsafe { self.rt.DrawRoundedRectangle(&rr, &p, grosor, None) };
    }

    /// Un icono de trazo en el centro de `caja`, de `lado` pixeles (a 96 ppp).
    pub fn icono(&self, i: Icono, caja: Caja, lado: f32, c: Rgb) {
        let Some(p) = self.pincel(c) else { return };
        let l = lado * self.escala;
        let x0 = caja.x as f32 + (caja.an as f32 - l) / 2.0 + self.dx;
        let y0 = caja.y as f32 + (caja.al as f32 - l) / 2.0 + self.dy;
        let k = l / 16.0;
        let g = (1.35 * self.escala).max(1.0);
        let at = |x: f32, y: f32| punto(x0 + x * k, y0 + y * k);
        let linea = |a: (f32, f32), b: (f32, f32)| {
            // SAFETY: dentro del dibujo; pincel y trazo vivos.
            unsafe {
                self.rt
                    .DrawLine(at(a.0, a.1), at(b.0, b.1), &p, g, self.trazo)
            };
        };
        let quebrada = |ps: &[(f32, f32)]| {
            for w in ps.windows(2) {
                linea(w[0], w[1]);
            }
        };
        let marco = |x: f32, y: f32, an: f32, al: f32, r: f32| {
            let rr = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: x0 + x * k,
                    top: y0 + y * k,
                    right: x0 + (x + an) * k,
                    bottom: y0 + (y + al) * k,
                },
                radiusX: r * k,
                radiusY: r * k,
            };
            // SAFETY: como arriba.
            unsafe { self.rt.DrawRoundedRectangle(&rr, &p, g, self.trazo) };
        };
        let circulo = |x: f32, y: f32, r: f32, lleno: bool| {
            let e = D2D1_ELLIPSE {
                point: at(x, y),
                radiusX: r * k,
                radiusY: r * k,
            };
            // SAFETY: como arriba.
            unsafe {
                if lleno {
                    self.rt.FillEllipse(&e, &p);
                } else {
                    self.rt.DrawEllipse(&e, &p, g, self.trazo);
                }
            }
        };
        match i {
            Icono::Tabla => {
                marco(2.0, 3.0, 12.0, 10.0, 1.5);
                linea((2.0, 6.5), (14.0, 6.5));
                linea((2.0, 9.8), (14.0, 9.8));
                linea((6.0, 3.0), (6.0, 13.0));
                linea((10.0, 3.0), (10.0, 13.0));
            }
            Icono::Imagen => {
                marco(2.0, 3.0, 12.0, 10.0, 1.5);
                circulo(10.3, 6.2, 1.1, false);
                quebrada(&[
                    (2.5, 11.5),
                    (6.0, 8.0),
                    (9.0, 11.0),
                    (10.8, 9.4),
                    (13.5, 11.8),
                ]);
            }
            Icono::Casillas => {
                quebrada(&[(2.0, 5.0), (3.4, 6.4), (5.8, 3.6)]);
                linea((8.0, 5.0), (14.0, 5.0));
                quebrada(&[(2.0, 11.0), (3.4, 12.4), (5.8, 9.6)]);
                linea((8.0, 11.0), (14.0, 11.0));
            }
            Icono::Mas => {
                linea((8.0, 3.0), (8.0, 13.0));
                linea((3.0, 8.0), (13.0, 8.0));
            }
            Icono::Flecha => quebrada(&[(4.5, 6.5), (8.0, 10.0), (11.5, 6.5)]),
            Icono::Compartir => {
                quebrada(&[(4.0, 7.5), (4.0, 13.5), (12.0, 13.5), (12.0, 7.5)]);
                linea((8.0, 2.5), (8.0, 10.0));
                quebrada(&[(5.3, 5.0), (8.0, 2.5), (10.7, 5.0)]);
            }
            Icono::PantallaCompleta => {
                quebrada(&[(9.5, 2.5), (13.5, 2.5), (13.5, 6.5)]);
                linea((13.5, 2.5), (9.3, 6.7));
                quebrada(&[(2.5, 9.5), (2.5, 13.5), (6.5, 13.5)]);
                linea((2.5, 13.5), (6.7, 9.3));
            }
            Icono::Minimizar => linea((4.0, 8.5), (12.0, 8.5)),
            Icono::Cerrar => {
                linea((4.0, 4.0), (12.0, 12.0));
                linea((12.0, 4.0), (4.0, 12.0));
            }
            Icono::Lista => {
                for y in [4.0, 8.0, 12.0] {
                    circulo(3.0, y, 0.9, true);
                    linea((6.0, y), (14.0, y));
                }
            }
            Icono::Numerada => {
                linea((3.2, 2.6), (3.2, 5.6));
                quebrada(&[(2.2, 7.2), (4.0, 7.2), (4.0, 8.5), (2.2, 9.6), (4.2, 9.6)]);
                quebrada(&[
                    (2.2, 11.4),
                    (4.1, 11.4),
                    (3.0, 12.6),
                    (4.2, 13.2),
                    (2.2, 14.2),
                ]);
                for y in [4.0, 8.4, 12.8] {
                    linea((6.5, y), (14.0, y));
                }
            }
            Icono::Fecha => {
                marco(2.0, 3.5, 12.0, 10.5, 1.5);
                linea((2.0, 7.0), (14.0, 7.0));
                linea((5.5, 2.0), (5.5, 4.8));
                linea((10.5, 2.0), (10.5, 4.8));
            }
            Icono::Cita => {
                linea((3.0, 3.0), (3.0, 13.0));
                linea((6.5, 5.0), (14.0, 5.0));
                linea((6.5, 8.0), (14.0, 8.0));
                linea((6.5, 11.0), (11.0, 11.0));
            }
            Icono::Codigo => {
                quebrada(&[(5.0, 4.5), (2.0, 8.0), (5.0, 11.5)]);
                quebrada(&[(11.0, 4.5), (14.0, 8.0), (11.0, 11.5)]);
                linea((9.3, 3.5), (6.7, 12.5));
            }
            Icono::Separador => {
                linea((2.0, 8.0), (14.0, 8.0));
                linea((5.0, 4.5), (11.0, 4.5));
                linea((5.0, 11.5), (11.0, 11.5));
            }
            Icono::Lapiz => {
                quebrada(&[
                    (3.0, 13.0),
                    (3.6, 10.2),
                    (10.8, 3.0),
                    (13.0, 5.2),
                    (5.8, 12.4),
                    (3.0, 13.0),
                ]);
                linea((9.4, 4.4), (11.6, 6.6));
            }
            Icono::Disco => {
                marco(2.5, 2.5, 11.0, 11.0, 1.5);
                linea((5.0, 2.5), (5.0, 5.8));
                quebrada(&[(5.0, 5.8), (10.5, 5.8), (10.5, 2.5)]);
                marco(5.0, 9.0, 6.0, 4.5, 0.5);
            }
            Icono::Copia => {
                marco(5.0, 5.0, 9.0, 9.0, 1.5);
                quebrada(&[
                    (3.5, 10.5),
                    (2.0, 10.5),
                    (2.0, 2.0),
                    (10.5, 2.0),
                    (10.5, 3.5),
                ]);
            }
            Icono::Pagina => {
                marco(2.5, 2.0, 9.0, 12.0, 1.5);
                linea((4.8, 5.5), (9.2, 5.5));
                linea((4.8, 8.2), (8.0, 8.2));
                circulo(12.0, 12.0, 2.4, false);
                circulo(12.0, 12.0, 0.8, true);
            }
            Icono::Enlace => {
                marco(1.5, 5.5, 7.5, 5.0, 2.5);
                marco(7.0, 5.5, 7.5, 5.0, 2.5);
            }
            Icono::Comentario | Icono::Comentar => {
                // Un globo con su pico abajo a la izquierda.
                quebrada(&[
                    (4.0, 2.5),
                    (12.0, 2.5),
                    (14.0, 4.5),
                    (14.0, 9.5),
                    (12.0, 11.5),
                    (7.0, 11.5),
                    (4.0, 14.0),
                    (4.5, 11.5),
                    (4.0, 11.5),
                    (2.0, 9.5),
                    (2.0, 4.5),
                    (4.0, 2.5),
                ]);
                if i == Icono::Comentar {
                    linea((8.0, 4.8), (8.0, 9.2));
                    linea((5.8, 7.0), (10.2, 7.0));
                } else {
                    linea((5.0, 6.0), (11.0, 6.0));
                    linea((5.0, 8.5), (9.0, 8.5));
                }
            }
            Icono::Hecho => quebrada(&[(3.0, 8.5), (6.5, 12.0), (13.0, 4.5)]),
            Icono::Filtro => {
                linea((2.5, 4.5), (13.5, 4.5));
                linea((4.5, 8.0), (11.5, 8.0));
                linea((6.5, 11.5), (9.5, 11.5));
            }
            Icono::Puntos => {
                for x in [3.5, 8.0, 12.5] {
                    circulo(x, 8.0, 1.1, true);
                }
            }
            Icono::Documento => {
                quebrada(&[
                    (3.5, 2.0),
                    (9.5, 2.0),
                    (12.5, 5.0),
                    (12.5, 14.0),
                    (3.5, 14.0),
                    (3.5, 2.0),
                ]);
                quebrada(&[(9.5, 2.0), (9.5, 5.0), (12.5, 5.0)]);
                linea((5.8, 8.5), (10.2, 8.5));
                linea((5.8, 11.0), (9.0, 11.0));
            }
            Icono::Chat => {
                // El globo redondo del chat, con su pico abajo a la izquierda.
                circulo(8.0, 7.5, 5.5, false);
                quebrada(&[(4.2, 11.4), (3.0, 14.0), (6.4, 12.7)]);
            }
            Icono::Audio => {
                for (x, h) in [(3.0, 2.0), (5.5, 5.0), (8.0, 3.0), (10.5, 6.0), (13.0, 2.5)] {
                    linea((x, 8.0 - h), (x, 8.0 + h));
                }
            }
        }
    }

    /// Una raya fina de lado a lado.
    pub fn raya(&self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb) {
        let Some(p) = self.pincel(c) else { return };
        // SAFETY: dentro del dibujo; pincel vivo.
        unsafe {
            self.rt.DrawLine(
                punto(x0 as f32 + self.dx, y0 as f32 + 0.5 + self.dy),
                punto(x1 as f32 + self.dx, y1 as f32 + 0.5 + self.dy),
                &p,
                1.0,
                None,
            )
        };
    }
}
