//! **Los incrustados pintados como en el chat**: la burbuja con su hora y
//! su chapa, la fila de un archivo con el icono de su tipo y el
//! reproductor de un audio, en un mapa de bits que la nota pone encima del
//! texto como cualquier foto (`imagenes`).
//!
//! # Lo que se copia del chat
//!
//! El chat pinta con Direct2D sobre la GPU y la nota con GDI y Direct2D
//! sobre un `HDC`, asi que no se puede llamar a su `pintar_historial`
//! (que ademas pinta el historial entero). Se copian sus **medidas y
//! colores**, de donde los saca el chat: la burbuja de radio 17 con
//! relleno 11 x 8 y 430 de ancho como mucho (`pixpin_ui::historial`), los
//! colores de `ColoresDelChat.kt` del movil (los mismos que el `Tema` del
//! chat), la chapa del codigo y la hora abajo a la izquierda, la fila de un
//! archivo de 44 con el icono de su tipo (la hoja de su color con la
//! esquina doblada de `pixpin_render::icono::archivo` y el color de
//! `pixpin_ui::color_de_extension`) y la pastilla de abrir de 30 x 24.
//!
//! # Por que en un mapa de bits
//!
//! Una tarjeta se pinta una vez y se pone encima tantas veces como haga
//! falta (desplazar, escribir encima): asi va por el mismo camino que las
//! fotos (hueco del alto de la tarjeta, sin parpadeo, borde al elegirla).
//! Solo el reproductor que suena se vuelve a pintar, unas veces por
//! segundo, y mide unos 500 x 90.

use std::path::Path;

use windows::Win32::Foundation::{COLORREF, RECT, SIZE};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::*;
use windows_numerics::Vector2;

use super::{Archivo, Audio, Burbuja, Contenido, EstadoAudio, Ficha, RotulosIncrustados};
use crate::tema::{Rgb, bgr};

/// Las medidas del chat (`pixpin_ui::historial`), a 96 ppp.
const RADIO: f32 = 17.0;
const RELLENO_X: i32 = 11;
const RELLENO_Y: i32 = 8;
const ANCHO_MAXIMO: i32 = 430;
/// El reproductor es mas ancho que una burbuja de texto: la barra de
/// avance tiene que dejar apuntar a un minuto.
const ANCHO_AUDIO: i32 = 520;
const FILA: i32 = 44;
const TEXTO_TAM: i32 = 13;
const NOMBRE_TAM: i32 = 15;
const DETALLE_TAM: i32 = 13;
const HORA_TAM: i32 = 11;
const CHAPA_TAM: i32 = 10;
/// La vista de una foto o de una hoja dentro de la burbuja.
const VISTA_ANCHO: i32 = 260;
const VISTA_ALTO: i32 = 180;
/// Renglones de texto de un mensaje como mucho: es una cita del chat, no
/// el mensaje entero (ese se abre con un clic).
const RENGLONES: i32 = 8;
/// El aire de arriba y de abajo del mapa: el borde de elegido se ve.
const AIRE: i32 = 3;
const LETRA: &str = "Segoe UI";
const LETRA_NOMBRE: &str = "Segoe UI Semibold";

/// Los colores del chat del movil (`guardados/ColoresDelChat.kt`) que lleva
/// el `Tema` del chat del PC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Colores {
    pub papel: Rgb,
    pub burbuja: Rgb,
    pub texto: Rgb,
    pub hora: Rgb,
    pub filete: Rgb,
    pub circulo: Rgb,
    pub circulo_icono: Rgb,
    pub rojo: Rgb,
}

/// `a` sobre `b` con opacidad `t`.
pub(crate) fn mezcla(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let c = |d: u32| {
        let x = ((a >> d) & 0xff) as f32;
        let y = ((b >> d) & 0xff) as f32;
        ((x * t + y * (1.0 - t)).round() as u32).min(255) << d
    };
    c(16) | c(8) | c(0)
}

impl Colores {
    /// Los del chat claro u oscuro, sobre el papel de la nota.
    pub(crate) fn del_chat(oscuro: bool, papel: Rgb) -> Colores {
        if oscuro {
            // msgOutBg de noche `#3E618A`, su tinta `#FAFAFA` y su hora
            // `#A8CCE8`; la chapa, blanco al 20 % (`#6481A1` medido).
            Colores {
                papel,
                burbuja: 0x3e618a,
                texto: 0xfafafa,
                hora: 0xa8cce8,
                filete: mezcla(0xffffff, 0x3e618a, 0.20),
                circulo: 0x493a11,
                circulo_icono: 0xfee8b7,
                rojo: 0xff8a80,
            }
        } else {
            // `#EFFFDE`, `#101B24` y la hora verde honda `#3F7A30`.
            Colores {
                papel,
                burbuja: 0xefffde,
                texto: 0x101b24,
                hora: 0x3f7a30,
                filete: mezcla(0x101b24, 0xefffde, 0.10),
                circulo: 0xffdea6,
                circulo_icono: 0x3e2d00,
                rojo: 0xc62828,
            }
        }
    }
}

/// Un rectangulo en pixeles del mapa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Caja {
    pub x: i32,
    pub y: i32,
    pub an: i32,
    pub al: i32,
}

impl Caja {
    pub(crate) fn dentro(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.an && y < self.y + self.al
    }
    fn rect(&self) -> RECT {
        RECT {
            left: self.x,
            top: self.y,
            right: self.x + self.an,
            bottom: self.y + self.al,
        }
    }
}

/// Lo que se puede tocar en un incrustado pintado, en pixeles del mapa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Zonas {
    /// La burbuja o la tarjeta entera (fuera de ella, el clic elige el
    /// renglon sin abrir nada).
    pub tarjeta: Caja,
    pub tocar: Option<Caja>,
    pub barra: Option<Caja>,
    pub velocidad: Option<Caja>,
    /// «Pasar a texto».
    pub texto: Option<Caja>,
}

/// Lo que hace falta para pintar el reproductor ademas de su ficha.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct ComoSuena {
    /// El estado si este audio es el cargado.
    pub estado: Option<EstadoAudio>,
    /// La nota ya lleva su transcripcion debajo.
    pub con_letra: bool,
    /// Se esta pasando a texto: por donde va, de 0 a 1.
    pub pasando: Option<f32>,
}

/// Un incrustado pintado: el mapa (de este hilo, lo suelta quien lo
/// recibe) y lo que mide.
pub(crate) struct Pintado {
    pub mapa: HBITMAP,
    pub an: i32,
    pub al: i32,
    pub zonas: Zonas,
}

// ---------------------------------------------------------------------------
// El lienzo de memoria

// La fabrica y el objetivo de Direct2D van con cada lienzo y no en un
// `thread_local`: soltar objetos COM en el destructor de un hilo que acaba
// (con el cerrojo del cargador cogido) dejaba colgado el siguiente (visto
// en las pruebas, que abren y cierran una nota por hilo). Crearlos cuesta
// menos que pintar la tarjeta.

fn crear_d2d() -> Option<(ID2D1Factory, ID2D1DCRenderTarget)> {
    // SAFETY: crear la fabrica y el objetivo no toma punteros ajenos.
    unsafe {
        let fabrica: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).ok()?;
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
        Some((fabrica, objetivo))
    }
}

fn color(c: Rgb, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((c >> 16) & 0xff) as f32 / 255.0,
        g: ((c >> 8) & 0xff) as f32 / 255.0,
        b: (c & 0xff) as f32 / 255.0,
        a,
    }
}

/// Un mapa de bits de 32 bits en un DC de memoria, con el papel de fondo.
struct Lienzo {
    dc: HDC,
    mapa: HBITMAP,
    viejo: HGDIOBJ,
    bits: *mut u8,
    an: i32,
    al: i32,
    esc: f32,
    d2d: Option<(ID2D1Factory, ID2D1DCRenderTarget)>,
}

/// Una forma para Direct2D.
enum Forma<'a> {
    Redondo(Caja, f32, Rgb, f32),
    Circulo(f32, f32, f32, Rgb),
    Poligono(&'a [(f32, f32)], Rgb, f32),
    Linea((f32, f32), (f32, f32), f32, Rgb),
    Borde(Caja, f32, f32, Rgb),
}

impl Lienzo {
    fn nuevo(an: i32, al: i32, papel: Rgb, esc: f32) -> Option<Lienzo> {
        let (an, al) = (an.max(1), al.max(1));
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: an,
                biHeight: -al,
                biPlanes: 1,
                biBitCount: 32,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        // SAFETY: DC y mapa propios; los suelta `acabar` (o `Drop`).
        unsafe {
            let dc = CreateCompatibleDC(None);
            let mapa = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
            if bits.is_null() {
                let _ = DeleteObject(HGDIOBJ(mapa.0));
                let _ = DeleteDC(dc);
                return None;
            }
            let viejo = SelectObject(dc, HGDIOBJ(mapa.0));
            let l = Lienzo {
                dc,
                mapa,
                viejo,
                bits: bits as *mut u8,
                an,
                al,
                esc,
                d2d: crear_d2d(),
            };
            l.rellenar(Caja { x: 0, y: 0, an, al }, papel);
            Some(l)
        }
    }

    fn px(&self, v: i32) -> i32 {
        (v as f32 * self.esc).round() as i32
    }

    fn rellenar(&self, c: Caja, color: Rgb) {
        // SAFETY: pincel propio, creado y soltado aqui, sobre el DC propio.
        unsafe {
            let p = CreateSolidBrush(COLORREF(bgr(color)));
            FillRect(self.dc, &c.rect(), p);
            let _ = DeleteObject(HGDIOBJ(p.0));
        }
    }

    /// Unas formas de una vez, suaves (Direct2D atado al DC).
    fn formas(&self, fs: &[Forma]) {
        {
            let Some((fabrica, rt)) = &self.d2d else { return };
            let zona = RECT {
                left: 0,
                top: 0,
                right: self.an,
                bottom: self.al,
            };
            // SAFETY: el DC es el propio y vive durante el dibujo; el
            // objetivo se ata a el solo entre BeginDraw y EndDraw.
            unsafe {
                if rt.BindDC(self.dc, &zona).is_err() {
                    return;
                }
                rt.BeginDraw();
                rt.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
                for f in fs {
                    match f {
                        Forma::Redondo(c, r, col, a) => {
                            let Ok(p) = rt.CreateSolidColorBrush(&color(*col, *a), None) else { continue };
                            let rr = D2D1_ROUNDED_RECT {
                                rect: D2D_RECT_F {
                                    left: c.x as f32,
                                    top: c.y as f32,
                                    right: (c.x + c.an) as f32,
                                    bottom: (c.y + c.al) as f32,
                                },
                                radiusX: *r,
                                radiusY: *r,
                            };
                            rt.FillRoundedRectangle(&rr, &p);
                        }
                        Forma::Borde(c, r, g, col) => {
                            let Ok(p) = rt.CreateSolidColorBrush(&color(*col, 1.0), None) else { continue };
                            let m = g / 2.0;
                            let rr = D2D1_ROUNDED_RECT {
                                rect: D2D_RECT_F {
                                    left: c.x as f32 + m,
                                    top: c.y as f32 + m,
                                    right: (c.x + c.an) as f32 - m,
                                    bottom: (c.y + c.al) as f32 - m,
                                },
                                radiusX: *r,
                                radiusY: *r,
                            };
                            rt.DrawRoundedRectangle(&rr, &p, *g, None);
                        }
                        Forma::Circulo(x, y, r, col) => {
                            let Ok(p) = rt.CreateSolidColorBrush(&color(*col, 1.0), None) else { continue };
                            let e = D2D1_ELLIPSE {
                                point: Vector2 { X: *x, Y: *y },
                                radiusX: *r,
                                radiusY: *r,
                            };
                            rt.FillEllipse(&e, &p);
                        }
                        Forma::Linea(a, b, g, col) => {
                            let Ok(p) = rt.CreateSolidColorBrush(&color(*col, 1.0), None) else { continue };
                            rt.DrawLine(Vector2 { X: a.0, Y: a.1 }, Vector2 { X: b.0, Y: b.1 }, &p, *g, None);
                        }
                        Forma::Poligono(ps, col, a) => {
                            if ps.len() < 3 {
                                continue;
                            }
                            let Ok(p) = rt.CreateSolidColorBrush(&color(*col, *a), None) else { continue };
                            let Ok(geo) = fabrica.CreatePathGeometry() else { continue };
                            let Ok(s) = geo.Open() else { continue };
                            s.BeginFigure(Vector2 { X: ps[0].0, Y: ps[0].1 }, D2D1_FIGURE_BEGIN_FILLED);
                            for q in &ps[1..] {
                                s.AddLine(Vector2 { X: q.0, Y: q.1 });
                            }
                            s.EndFigure(D2D1_FIGURE_END_CLOSED);
                            if s.Close().is_ok() {
                                rt.FillGeometry(&geo, &p, None);
                            }
                        }
                    }
                }
                let _ = rt.EndDraw(None, None);
            }
        }
    }

    /// Una letra de GDI de `tam` pixeles (a 96 ppp).
    fn letra(&self, nombre: &str, tam: i32) -> HFONT {
        crate::pintor::letra_gdi(nombre, self.px(tam), false)
    }

    fn medir(&self, nombre: &str, tam: i32, texto: &str) -> SIZE {
        let t: Vec<u16> = texto.encode_utf16().collect();
        let mut s = SIZE::default();
        let f = self.letra(nombre, tam);
        // SAFETY: DC y letra propios; se devuelve la letra que habia.
        unsafe {
            let v = SelectObject(self.dc, HGDIOBJ(f.0));
            let _ = GetTextExtentPoint32W(self.dc, &t, &mut s);
            SelectObject(self.dc, v);
            let _ = DeleteObject(HGDIOBJ(f.0));
        }
        s
    }

    /// Lo alto que ocupa un texto partido en renglones en `ancho`.
    fn alto_partido(&self, tam: i32, texto: &str, ancho: i32) -> i32 {
        if texto.is_empty() {
            return 0;
        }
        let mut t: Vec<u16> = texto.encode_utf16().collect();
        let mut r = RECT {
            left: 0,
            top: 0,
            right: ancho.max(1),
            bottom: 0,
        };
        let f = self.letra(LETRA, tam);
        // SAFETY: como `medir`.
        unsafe {
            let v = SelectObject(self.dc, HGDIOBJ(f.0));
            DrawTextW(self.dc, &mut t, &mut r, DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX | DT_EDITCONTROL);
            SelectObject(self.dc, v);
            let _ = DeleteObject(HGDIOBJ(f.0));
        }
        r.bottom - r.top
    }

    /// Un texto: en un renglon con «…» si no cabe, o partido en renglones.
    fn texto(&self, nombre: &str, tam: i32, texto: &str, c: Caja, tinta: Rgb, partido: bool) {
        if texto.is_empty() || c.an <= 0 {
            return;
        }
        let mut t: Vec<u16> = texto.encode_utf16().collect();
        let mut r = c.rect();
        let formato = if partido {
            DT_WORDBREAK | DT_NOPREFIX | DT_EDITCONTROL
        } else {
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX
        };
        let f = self.letra(nombre, tam);
        // SAFETY: DC y letra propios; el texto es un bufer propio.
        unsafe {
            let v = SelectObject(self.dc, HGDIOBJ(f.0));
            SetBkMode(self.dc, TRANSPARENT);
            SetTextColor(self.dc, COLORREF(bgr(tinta)));
            DrawTextW(self.dc, &mut t, &mut r, formato);
            SelectObject(self.dc, v);
            let _ = DeleteObject(HGDIOBJ(f.0));
        }
    }

    /// Una foto centrada en `hueco`, encogida sin deformarla, con esquinas
    /// de radio `radio` (lo de fuera se queda como estaba) y su
    /// transparencia sobre `fondo`. `false` si no se pudo leer.
    fn foto_en(&self, ruta: &Path, hueco: Caja, radio: i32, fondo: Rgb) -> bool {
        let Some(img) = pixpin_codec::imagen::cargar(ruta).ok() else {
            return false;
        };
        let (an, al) = crate::imagenes::encajar_con(img.ancho, img.alto, (hueco.an, hueco.al), false);
        let img = if (an as u32, al as u32) != (img.ancho, img.alto) {
            match pixpin_codec::imagen::redimensionar(img, an as u32, al as u32) {
                Ok(i) => i,
                Err(_) => return false,
            }
        } else {
            img
        };
        let (x, y) = (hueco.x + (hueco.an - an) / 2, hueco.y + (hueco.al - al) / 2);
        // SAFETY: lo pintado por GDI y Direct2D tiene que estar en los bits
        // antes de escribirlos a mano.
        unsafe {
            let _ = GdiFlush();
        }
        let (fr, fg, fb) = ((fondo >> 16) & 0xff, (fondo >> 8) & 0xff, fondo & 0xff);
        for j in 0..al {
            for i in 0..an {
                let (px, py) = (x + i, y + j);
                if px < 0 || py < 0 || px >= self.an || py >= self.al || fuera_de_la_esquina(i, j, an, al, radio) {
                    continue;
                }
                let s = ((j * an + i) * 4) as usize;
                let a = img.pixeles[s + 3] as u32;
                let mez = |v: u8, f: u32| ((v as u32 * a + f * (255 - a)) / 255) as u8;
                let d = ((py * self.an + px) * 4) as usize;
                // SAFETY: `d` esta dentro del mapa (comprobado arriba).
                unsafe {
                    *self.bits.add(d) = mez(img.pixeles[s + 2], fb);
                    *self.bits.add(d + 1) = mez(img.pixeles[s + 1], fg);
                    *self.bits.add(d + 2) = mez(img.pixeles[s], fr);
                }
            }
        }
        true
    }

    /// Opaco del todo (GDI deja el alfa a cero donde escribe) y el mapa
    /// suelto del DC, para quien lo vaya a poner encima de la nota.
    fn acabar(mut self, zonas: Zonas) -> Pintado {
        // Direct2D se suelta aqui: el `forget` de abajo no lo haria.
        self.d2d = None;
        // SAFETY: los bits son del mapa propio, de `an * al` pixeles; luego
        // se suelta el DC y el mapa pasa a quien lo recibe.
        unsafe {
            let _ = GdiFlush();
            let n = (self.an * self.al) as usize;
            for k in 0..n {
                *self.bits.add(k * 4 + 3) = 255;
            }
            SelectObject(self.dc, self.viejo);
            let _ = DeleteDC(self.dc);
        }
        let p = Pintado {
            mapa: self.mapa,
            an: self.an,
            al: self.al,
            zonas,
        };
        std::mem::forget(self);
        p
    }
}

impl Drop for Lienzo {
    fn drop(&mut self) {
        // SAFETY: DC y mapa propios (si no se acabo).
        unsafe {
            SelectObject(self.dc, self.viejo);
            let _ = DeleteDC(self.dc);
            let _ = DeleteObject(HGDIOBJ(self.mapa.0));
        }
    }
}

/// Si el pixel `(i, j)` de una caja de `an x al` cae fuera de sus esquinas
/// redondeadas de radio `r`.
fn fuera_de_la_esquina(i: i32, j: i32, an: i32, al: i32, r: i32) -> bool {
    if r <= 0 {
        return false;
    }
    let cx = if i < r { r } else if i >= an - r { an - r - 1 } else { return false };
    let cy = if j < r { r } else if j >= al - r { al - r - 1 } else { return false };
    let (dx, dy) = ((i - cx) as f32, (j - cy) as f32);
    dx * dx + dy * dy > (r as f32) * (r as f32)
}

// ---------------------------------------------------------------------------
// Las piezas

/// El icono de un archivo: la hoja de su color con la esquina doblada y la
/// extension escrita (`IconoDeArchivo.kt`, `icono_de_tipo` del chat).
fn icono_de_archivo(l: &Lienzo, nombre: &str, c: Caja) {
    use pixpin_render::icono::archivo as ia;
    let f = pixpin_ui::color_de_extension::de(nombre);
    let caja = pixpin_render::RectF {
        x: c.x as f32,
        y: c.y as f32,
        ancho: c.an as f32,
        alto: c.al as f32,
    };
    let hoja = ia::contorno_hoja(caja);
    let doblez = ia::contorno_doblez(caja);
    l.formas(&[Forma::Poligono(&hoja, f.color, 1.0), Forma::Poligono(&doblez, 0xffffff, ia::ALFA_DEL_DOBLEZ)]);
    let rotulo = pixpin_ui::color_de_extension::rotulo(nombre);
    if rotulo.is_empty() {
        return;
    }
    let tam = (ia::tam_del_rotulo(c.al as f32, &rotulo) / l.esc).round() as i32;
    let (cx, cy) = ia::centro_del_rotulo(caja);
    let tinta = if f.texto_oscuro { 0x3a2b00 } else { 0xffffff };
    let s = l.medir(LETRA_NOMBRE, tam, &rotulo);
    l.texto(
        LETRA_NOMBRE,
        tam,
        &rotulo,
        Caja {
            x: cx as i32 - s.cx / 2,
            y: cy as i32 - s.cy / 2,
            an: s.cx + 2,
            al: s.cy,
        },
        tinta,
        false,
    );
}

/// La pastilla de «abrir fuera» del chat: el color de la hora al 15,6 %,
/// esquinas de 8 y la flecha que sale de un cuadro.
fn pastilla_de_abrir(l: &Lienzo, c: Caja, col: &Colores) {
    let e = l.esc;
    let (x, y) = (c.x as f32 + c.an as f32 / 2.0, c.y as f32 + c.al as f32 / 2.0);
    let k = 5.0 * e;
    let g = (1.4 * e).max(1.0);
    l.formas(&[
        Forma::Redondo(c, 8.0 * e, col.hora, 0.156),
        Forma::Linea((x - k, y - k + 2.0 * e), (x - k, y + k), g, col.hora),
        Forma::Linea((x - k, y + k), (x + k - 2.0 * e, y + k), g, col.hora),
        Forma::Linea((x - 1.0 * e, y + 1.0 * e), (x + k, y - k), g, col.hora),
        Forma::Linea((x + 1.0 * e, y - k), (x + k, y - k), g, col.hora),
        Forma::Linea((x + k, y - k), (x + k, y - 1.0 * e), g, col.hora),
    ]);
}

/// El triangulo de tocar o las dos barras de pausa, en el circulo.
fn boton_de_tocar(l: &Lienzo, c: Caja, sonando: bool, col: &Colores, apagado: bool) {
    let e = l.esc;
    let (cx, cy, r) = (c.x as f32 + c.an as f32 / 2.0, c.y as f32 + c.al as f32 / 2.0, c.an as f32 / 2.0);
    let tinta = if apagado { mezcla(col.circulo_icono, col.circulo, 0.4) } else { col.circulo_icono };
    l.formas(&[Forma::Circulo(cx, cy, r, col.circulo)]);
    if sonando {
        let (an, al) = (4.0 * e, 14.0 * e);
        l.formas(&[
            Forma::Redondo(
                Caja {
                    x: (cx - an - 2.0 * e) as i32,
                    y: (cy - al / 2.0) as i32,
                    an: an as i32,
                    al: al as i32,
                },
                1.0 * e,
                tinta,
                1.0,
            ),
            Forma::Redondo(
                Caja {
                    x: (cx + 2.0 * e) as i32,
                    y: (cy - al / 2.0) as i32,
                    an: an as i32,
                    al: al as i32,
                },
                1.0 * e,
                tinta,
                1.0,
            ),
        ]);
    } else {
        let k = 8.0 * e;
        let tri = [(cx - k * 0.6, cy - k), (cx + k, cy), (cx - k * 0.6, cy + k)];
        l.formas(&[Forma::Poligono(&tri, tinta, 1.0)]);
    }
}

/// La fila de un archivo (`FilaDeArchivo`): el icono de 44, el nombre y su
/// detalle, y la pastilla de abrir a la derecha.
fn fila_de_archivo(l: &Lienzo, a: &Archivo, x: i32, y: i32, ancho: i32, col: &Colores) {
    let fila = l.px(FILA);
    icono_de_archivo(l, &a.nombre, Caja { x, y, an: fila, al: fila });
    let abrir = Caja {
        x: x + ancho - l.px(30),
        y: y + (fila - l.px(24)) / 2,
        an: l.px(30),
        al: l.px(24),
    };
    if !a.falta {
        pastilla_de_abrir(l, abrir, col);
    }
    let tx = x + fila + l.px(10);
    let tw = (abrir.x - l.px(8) - tx).max(0);
    let (h1, h2) = (l.px(NOMBRE_TAM) * 4 / 3, l.px(DETALLE_TAM) * 4 / 3);
    let ty = y + (fila - h1 - h2) / 2;
    l.texto(LETRA_NOMBRE, NOMBRE_TAM, &a.nombre, Caja { x: tx, y: ty, an: tw, al: h1 }, col.texto, false);
    let tinta = if a.falta { col.rojo } else { mezcla(col.hora, col.burbuja, 0.8) };
    l.texto(LETRA, DETALLE_TAM, &a.detalle, Caja { x: tx, y: ty + h1, an: tw, al: h2 }, tinta, false);
}

/// Lo que mide el pie (chapa y hora) y lo pinta en `x, y` si `pintar`.
fn pie(l: &Lienzo, b: &Burbuja, x: i32, y: i32, col: &Colores, pintar: bool) -> (i32, i32) {
    let alto = l.px(HORA_TAM) * 4 / 3;
    let mut cx = x;
    if let Some(codigo) = &b.codigo {
        let s = l.medir(LETRA, CHAPA_TAM, codigo);
        let chapa = Caja {
            x: cx,
            y: y + (alto - s.cy - l.px(2)) / 2,
            an: s.cx + l.px(8),
            al: s.cy + l.px(2),
        };
        if pintar {
            l.formas(&[Forma::Redondo(chapa, 6.0 * l.esc, col.filete, 1.0)]);
            l.texto(LETRA, CHAPA_TAM, codigo, Caja { x: chapa.x + l.px(4), ..chapa }, col.hora, false);
        }
        cx += chapa.an + l.px(4);
    }
    let s = l.medir(LETRA, HORA_TAM, &b.hora);
    if pintar {
        l.texto(LETRA, HORA_TAM, &b.hora, Caja { x: cx, y, an: s.cx + 2, al: alto }, col.hora, false);
    }
    (cx + s.cx - x, alto)
}

/// Un texto recortado a `renglones` en `ancho`, con «…» si sobraba.
fn recortar(l: &Lienzo, tam: i32, texto: &str, ancho: i32, renglones: i32) -> String {
    let tope = l.px(tam) * 4 / 3 * renglones + l.px(2);
    if l.alto_partido(tam, texto, ancho) <= tope {
        return texto.to_string();
    }
    let letras: Vec<char> = texto.chars().collect();
    let (mut bajo, mut alto) = (0usize, letras.len());
    while bajo + 1 < alto {
        let medio = (bajo + alto) / 2;
        let prueba: String = letras[..medio].iter().collect::<String>() + "…";
        if l.alto_partido(tam, &prueba, ancho) <= tope {
            bajo = medio;
        } else {
            alto = medio;
        }
    }
    letras[..bajo].iter().collect::<String>().trim_end().to_string() + "…"
}

// ---------------------------------------------------------------------------
// Lo de cada ficha

/// **Pinta una ficha** en un mapa de `columna` de ancho (pixeles de
/// pantalla), con la burbuja pegada a la izquierda como el texto.
pub(crate) fn pintar(
    ficha: &Ficha,
    suena: &ComoSuena,
    columna: i32,
    col: &Colores,
    r: &RotulosIncrustados,
    esc: f32,
) -> Option<Pintado> {
    match ficha {
        Ficha::Archivo(a) => tarjeta_de_archivo(a, columna, col, esc),
        Ficha::Burbuja(b) => burbuja(b, columna, col, esc),
        Ficha::Audio(a) => reproductor(a, suena, columna, col, r, esc),
        Ficha::Borrado(nombre) => borrado(nombre, columna, col, r, esc),
    }
}

fn ancho_de(maximo: i32, columna: i32, esc: f32) -> i32 {
    ((maximo as f32 * esc) as i32).min(columna).max((120.0 * esc) as i32)
}

fn tarjeta_de_archivo(a: &Archivo, columna: i32, col: &Colores, esc: f32) -> Option<Pintado> {
    let px = |v: i32| (v as f32 * esc).round() as i32;
    let ancho = ancho_de(ANCHO_MAXIMO, columna, esc);
    let alto = px(FILA) + 2 * px(RELLENO_Y);
    let l = Lienzo::nuevo(columna.max(ancho), alto + 2 * px(AIRE), col.papel, esc)?;
    let caja = Caja {
        x: 0,
        y: px(AIRE),
        an: ancho,
        al: alto,
    };
    l.formas(&[Forma::Redondo(caja, RADIO * esc, col.burbuja, 1.0)]);
    fila_de_archivo(&l, a, px(RELLENO_X), caja.y + px(RELLENO_Y), ancho - 2 * px(RELLENO_X), col);
    Some(l.acabar(Zonas {
        tarjeta: caja,
        ..Default::default()
    }))
}

fn burbuja(b: &Burbuja, columna: i32, col: &Colores, esc: f32) -> Option<Pintado> {
    let px = |v: i32| (v as f32 * esc).round() as i32;
    let maximo = ancho_de(ANCHO_MAXIMO, columna, esc);
    let dentro_max = maximo - 2 * px(RELLENO_X);
    // Primero se mide en un lienzo de prueba, luego se pinta a su medida.
    let medida = Lienzo::nuevo(1, 1, col.papel, esc)?;
    let (pie_an, pie_al) = pie(&medida, b, 0, 0, col, false);
    let hueco = px(6);
    let (cuerpo_an, cuerpo_al, texto) = match &b.contenido {
        Contenido::Texto(t) => {
            let t = recortar(&medida, TEXTO_TAM, t, dentro_max, RENGLONES);
            let an = t.lines().map(|r| medida.medir(LETRA, TEXTO_TAM, r).cx).max().unwrap_or(0).min(dentro_max);
            (an, medida.alto_partido(TEXTO_TAM, &t, dentro_max.max(an)), t)
        }
        Contenido::Foto { pie, .. } => {
            let t = recortar(&medida, TEXTO_TAM, pie, px(VISTA_ANCHO), 3);
            let alto_t = if t.is_empty() { 0 } else { hueco + medida.alto_partido(TEXTO_TAM, &t, px(VISTA_ANCHO)) };
            (px(VISTA_ANCHO).min(dentro_max), px(VISTA_ALTO) + alto_t, t)
        }
        Contenido::Hoja { nombre, .. } => {
            (px(VISTA_ANCHO).min(dentro_max), px(VISTA_ALTO) + hueco + px(NOMBRE_TAM) * 4 / 3, nombre.clone())
        }
        Contenido::Archivo(_) => (dentro_max.min(px(320)), px(FILA), String::new()),
        Contenido::Voz { texto, .. } => {
            let t = recortar(&medida, TEXTO_TAM, texto, dentro_max, 3);
            let alto_t = if t.is_empty() { 0 } else { hueco + medida.alto_partido(TEXTO_TAM, &t, dentro_max) };
            (dentro_max.min(px(320)), px(FILA) + alto_t, t)
        }
    };
    drop(medida);
    let ancho = (cuerpo_an.max(pie_an) + 2 * px(RELLENO_X)).min(maximo);
    let dentro_an = ancho - 2 * px(RELLENO_X);
    let alto = px(RELLENO_Y) + cuerpo_al + px(4) + pie_al + px(RELLENO_Y) - px(2);
    let l = Lienzo::nuevo(columna.max(ancho), alto + 2 * px(AIRE), col.papel, esc)?;
    let caja = Caja {
        x: 0,
        y: px(AIRE),
        an: ancho,
        al: alto,
    };
    l.formas(&[Forma::Redondo(caja, RADIO * esc, col.burbuja, 1.0)]);
    let (x, y) = (px(RELLENO_X), caja.y + px(RELLENO_Y));
    match &b.contenido {
        Contenido::Texto(_) => {
            l.texto(LETRA, TEXTO_TAM, &texto, Caja { x, y, an: dentro_an, al: cuerpo_al }, col.texto, true);
        }
        Contenido::Foto { ruta, .. } => {
            let vista = Caja { x, y, an: dentro_an, al: px(VISTA_ALTO) };
            let fondo = mezcla(col.texto, col.burbuja, 0.08);
            l.formas(&[Forma::Redondo(vista, 8.0 * esc, fondo, 1.0)]);
            l.foto_en(ruta, vista, px(8), fondo);
            if !texto.is_empty() {
                let ty = y + px(VISTA_ALTO) + hueco;
                l.texto(LETRA, TEXTO_TAM, &texto, Caja { x, y: ty, an: dentro_an, al: cuerpo_al - px(VISTA_ALTO) - hueco }, col.texto, true);
            }
        }
        Contenido::Hoja { miniatura, clase, .. } => {
            let vista = Caja { x, y, an: dentro_an, al: px(VISTA_ALTO) };
            // La hoja sobre papel claro tambien de noche, como en el chat.
            let papel = if col.burbuja == 0x3e618a { 0xe8e8e8 } else { 0xffffff };
            l.formas(&[Forma::Redondo(vista, 8.0 * esc, papel, 1.0)]);
            let puesta = miniatura.as_ref().is_some_and(|m| l.foto_en(m, vista, px(8), papel));
            if !puesta {
                // Sin miniatura todavia: su clase en grande, en medio.
                let s = l.medir(LETRA_NOMBRE, 22, clase);
                l.texto(
                    LETRA_NOMBRE,
                    22,
                    clase,
                    Caja {
                        x: vista.x + (vista.an - s.cx) / 2,
                        an: s.cx + 2,
                        ..vista
                    },
                    0x5d5f71,
                    false,
                );
            }
            l.formas(&[Forma::Borde(vista, 8.0 * esc, 1.0, col.filete)]);
            let ty = y + px(VISTA_ALTO) + hueco;
            l.texto(LETRA_NOMBRE, NOMBRE_TAM, &texto, Caja { x, y: ty, an: dentro_an, al: px(NOMBRE_TAM) * 4 / 3 }, col.texto, false);
        }
        Contenido::Archivo(a) => fila_de_archivo(&l, a, x, y, dentro_an, col),
        Contenido::Voz { duracion, .. } => {
            boton_de_tocar(&l, Caja { x, y, an: px(FILA), al: px(FILA) }, false, col, false);
            let tx = x + px(FILA) + px(10);
            l.texto(LETRA, DETALLE_TAM, duracion, Caja { x: tx, y, an: dentro_an - (tx - x), al: px(FILA) }, col.hora, false);
            if !texto.is_empty() {
                let ty = y + px(FILA) + hueco;
                l.texto(LETRA, TEXTO_TAM, &texto, Caja { x, y: ty, an: dentro_an, al: cuerpo_al - px(FILA) - hueco }, col.texto, true);
            }
        }
    }
    pie(&l, b, x, y + cuerpo_al + px(4), col, true);
    Some(l.acabar(Zonas {
        tarjeta: caja,
        ..Default::default()
    }))
}

fn borrado(nombre: &str, columna: i32, col: &Colores, r: &RotulosIncrustados, esc: f32) -> Option<Pintado> {
    let px = |v: i32| (v as f32 * esc).round() as i32;
    let texto = if nombre.trim().is_empty() { r.borrado.clone() } else { format!("{} · {}", r.borrado, nombre.trim()) };
    let medida = Lienzo::nuevo(1, 1, col.papel, esc)?;
    let maximo = ancho_de(ANCHO_MAXIMO, columna, esc);
    let an = (medida.medir(LETRA, TEXTO_TAM, &texto).cx + 2 * px(RELLENO_X)).min(maximo);
    drop(medida);
    let alto = px(TEXTO_TAM) * 4 / 3 + 2 * px(RELLENO_Y);
    let l = Lienzo::nuevo(columna.max(an), alto + 2 * px(AIRE), col.papel, esc)?;
    let caja = Caja { x: 0, y: px(AIRE), an, al: alto };
    // La burbuja apagada y con su filete: esta, pero ya no dice nada.
    l.formas(&[
        Forma::Redondo(caja, RADIO * esc, mezcla(col.burbuja, col.papel, 0.45), 1.0),
        Forma::Borde(caja, RADIO * esc, 1.0, col.filete),
    ]);
    let tinta = mezcla(col.texto, col.papel, 0.6);
    l.texto(LETRA, TEXTO_TAM, &texto, Caja { x: px(RELLENO_X), y: caja.y, an: an - 2 * px(RELLENO_X), al: alto }, tinta, false);
    Some(l.acabar(Zonas {
        tarjeta: caja,
        ..Default::default()
    }))
}

/// **El reproductor**: el circulo de tocar de la nota de voz del chat, el
/// nombre, el tiempo «0:12 / 1:05», la velocidad, la barra de avance y,
/// si la nota aun no lleva la transcripcion, la pastilla de pasarla a texto.
fn reproductor(a: &Audio, suena: &ComoSuena, columna: i32, col: &Colores, r: &RotulosIncrustados, esc: f32) -> Option<Pintado> {
    let px = |v: i32| (v as f32 * esc).round() as i32;
    let ancho = ancho_de(ANCHO_AUDIO, columna, esc);
    let con_pastilla = !a.falta && !suena.con_letra;
    let barra_al = px(14);
    let pastilla_al = px(26);
    let alto = px(RELLENO_Y) + px(FILA) + px(6) + barra_al + if con_pastilla { px(6) + pastilla_al } else { 0 } + px(RELLENO_Y);
    let l = Lienzo::nuevo(columna.max(ancho), alto + 2 * px(AIRE), col.papel, esc)?;
    let caja = Caja { x: 0, y: px(AIRE), an: ancho, al: alto };
    l.formas(&[Forma::Redondo(caja, RADIO * esc, col.burbuja, 1.0)]);
    let (x, y) = (px(RELLENO_X), caja.y + px(RELLENO_Y));
    let dentro_an = ancho - 2 * px(RELLENO_X);
    let mut zonas = Zonas {
        tarjeta: caja,
        ..Default::default()
    };
    let estado = suena.estado.as_ref();
    let sonando = estado.is_some_and(|e| e.sonando);
    let tocar = Caja { x, y, an: px(FILA), al: px(FILA) };
    boton_de_tocar(&l, tocar, sonando, col, a.falta);
    if !a.falta {
        zonas.tocar = Some(tocar);
    }
    // La velocidad, escrita («1,5×»), en la pastilla de la derecha.
    let vel = super::velocidad_legible(estado.map_or(1.0, |e| e.velocidad));
    let vel_an = l.medir(LETRA, DETALLE_TAM, &vel).cx + px(14);
    let velocidad = Caja {
        x: x + dentro_an - vel_an,
        y: y + (px(FILA) - px(24)) / 2,
        an: vel_an,
        al: px(24),
    };
    if !a.falta {
        l.formas(&[Forma::Redondo(velocidad, 8.0 * esc, col.hora, 0.156)]);
        l.texto(LETRA, DETALLE_TAM, &vel, Caja { x: velocidad.x + px(7), ..velocidad }, col.hora, false);
        zonas.velocidad = Some(velocidad);
    }
    let tx = x + px(FILA) + px(10);
    let tw = (velocidad.x - px(8) - tx).max(0);
    let (h1, h2) = (px(NOMBRE_TAM) * 4 / 3, px(DETALLE_TAM) * 4 / 3);
    let ty = y + (px(FILA) - h1 - h2) / 2;
    l.texto(LETRA_NOMBRE, NOMBRE_TAM, &a.nombre, Caja { x: tx, y: ty, an: tw, al: h1 }, col.texto, false);
    let duracion = estado.map(|e| e.duracion_ms).filter(|d| *d > 0).unwrap_or(a.duracion_ms);
    let posicion = estado.map_or(0, |e| e.posicion_ms);
    let (detalle, tinta) = if a.falta {
        (r.falta.clone(), col.rojo)
    } else if duracion > 0 {
        (format!("{} / {}", super::marca_de_tiempo(posicion), super::marca_de_tiempo(duracion)), mezcla(col.hora, col.burbuja, 0.8))
    } else {
        (super::marca_de_tiempo(posicion), mezcla(col.hora, col.burbuja, 0.8))
    };
    l.texto(LETRA, DETALLE_TAM, &detalle, Caja { x: tx, y: ty + h1, an: tw, al: h2 }, tinta, false);
    // La barra: la pista en el color de la chapa y lo oido en el de la hora.
    let barra = Caja {
        x,
        y: y + px(FILA) + px(6),
        an: dentro_an,
        al: barra_al,
    };
    let linea = Caja {
        x: barra.x,
        y: barra.y + (barra.al - px(3).max(2)) / 2,
        an: barra.an,
        al: px(3).max(2),
    };
    let mut fs = vec![Forma::Redondo(linea, 1.5 * esc, col.filete, 1.0)];
    if duracion > 0 && posicion > 0 {
        let oido = ((linea.an as f32) * (posicion as f32 / duracion as f32).clamp(0.0, 1.0)) as i32;
        fs.push(Forma::Redondo(Caja { an: oido.max(1), ..linea }, 1.5 * esc, col.hora, 1.0));
        fs.push(Forma::Circulo((linea.x + oido) as f32, (linea.y as f32) + linea.al as f32 / 2.0, 5.0 * esc, col.hora));
    }
    l.formas(&fs);
    if !a.falta {
        zonas.barra = Some(barra);
    }
    if con_pastilla {
        let rotulo = match suena.pasando {
            Some(f) => r.pasando.replace("{pct}", &format!("{}", (f * 100.0).round() as i64)),
            None => r.pasar_a_texto.clone(),
        };
        let an = l.medir(LETRA, DETALLE_TAM, &rotulo).cx + px(34);
        let p = Caja {
            x,
            y: barra.y + barra.al + px(6),
            an,
            al: pastilla_al,
        };
        l.formas(&[Forma::Redondo(p, 8.0 * esc, col.hora, if suena.pasando.is_some() { 0.08 } else { 0.156 })]);
        // El icono de subtitulos del chat: un marco con dos rayas.
        let (ix, iy) = (p.x + px(9), p.y + (p.al - px(12)) / 2);
        let g = (1.3 * esc).max(1.0);
        let (fx, fy) = (ix as f32, iy as f32);
        l.formas(&[
            Forma::Borde(Caja { x: ix, y: iy, an: px(16), al: px(12) }, 2.0 * esc, g, col.hora),
            Forma::Linea((fx + 3.0 * esc, fy + 5.0 * esc), (fx + 9.0 * esc, fy + 5.0 * esc), g, col.hora),
            Forma::Linea((fx + 3.0 * esc, fy + 8.0 * esc), (fx + 12.0 * esc, fy + 8.0 * esc), g, col.hora),
        ]);
        l.texto(LETRA, DETALLE_TAM, &rotulo, Caja { x: p.x + px(28), an: p.an - px(28), ..p }, col.hora, false);
        if suena.pasando.is_none() {
            zonas.texto = Some(p);
        }
    }
    Some(l.acabar(zonas))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_mezcla_va_de_un_color_al_otro() {
        assert_eq!(mezcla(0xffffff, 0x000000, 1.0), 0xffffff);
        assert_eq!(mezcla(0xffffff, 0x000000, 0.0), 0x000000);
        // El blanco al 20 % sobre la burbuja de noche da la chapa medida.
        assert_eq!(mezcla(0xffffff, 0x3e618a, 0.20), 0x6581a1);
    }

    #[test]
    fn las_esquinas_de_una_foto_se_redondean_y_el_centro_no() {
        assert!(fuera_de_la_esquina(0, 0, 100, 50, 8));
        assert!(!fuera_de_la_esquina(50, 25, 100, 50, 8));
        assert!(!fuera_de_la_esquina(8, 0, 100, 50, 8));
        // Caso negativo: sin radio no se quita nada.
        assert!(!fuera_de_la_esquina(0, 0, 100, 50, 0));
    }
}
