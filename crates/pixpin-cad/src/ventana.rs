//! **La ventana del plano, como un pin**: sin bordes, siempre encima (se
//! puede soltar), se estira por los bordes. Lo que el usuario pidio el
//! 8-oct-2026: «como un pin normal: una barra fuera del pin que aparece y
//! desaparece», con solo el tema (claro u oscuro) y «acotar plano» para
//! medir.
//!
//! - Rueda: acercar y alejar hacia el cursor, suave.
//! - Arrastrar (izquierdo o central): mover el plano. Ctrl + arrastrar, o el
//!   asa de la barra: mover la ventana.
//! - Doble clic: el plano entero (o, acotando, terminar la medida).
//! - Clic derecho: un menu (todo, tema, acotar, encima, cerrar).
//! - Teclas: Esc (acotando: deja la medida; si no, cierra), F o Inicio
//!   todo, B tema, M acotar, T encima, flechas, + y −.
//!
//! No se dibuja nada si nada cambia: en reposo no gasta.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, MonitorFromWindow, ScreenToClient};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_CONTROL};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

const WM_MOUSELEAVE: u32 = 0x02A3;
const INFINITE: u32 = u32::MAX;

use crate::convertir::rgba;
use crate::gpu::{Gpu, PlanoGpu, Vista};
use crate::modelo::{Constructor, Modelo};
use crate::texto::Textos;

/// De que ventana es un evento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum De {
    Plano,
    Barra,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Ev {
    Mover(i32, i32),
    Bajar(i32, i32, u8),
    Subir(i32, i32, u8),
    Doble(i32, i32),
    Rueda(i32, i32, i32),
    Tecla(u32),
    Tamano(u32, u32),
    Salir,
    Cerrar,
    Menu(u32),
}

#[derive(Default)]
struct Estado {
    eventos: VecDeque<(De, Ev)>,
}

thread_local! {
    static ESTADO: RefCell<Estado> = RefCell::new(Estado::default());
}

fn apuntar(de: De, e: Ev) {
    ESTADO.with(|s| s.borrow_mut().eventos.push_back((de, e)));
}

fn xy(l: LPARAM) -> (i32, i32) {
    ((l.0 & 0xffff) as i16 as i32, ((l.0 >> 16) & 0xffff) as i16 as i32)
}

unsafe extern "system" fn procedimiento_plano(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: lo mismo que `procedimiento`.
    unsafe { procedimiento(De::Plano, hwnd, msg, wp, lp) }
}

/// Cada ventana con el suyo: la barra recibe mensajes (su `WM_SIZE`) antes
/// de que se sepa su `HWND`, y con uno solo se tomaban por del plano.
unsafe extern "system" fn procedimiento_barra(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: lo mismo que `procedimiento`.
    unsafe { procedimiento(De::Barra, hwnd, msg, wp, lp) }
}

unsafe fn procedimiento(de: De, hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_NCCALCSIZE if wp.0 != 0 && de == De::Plano => return LRESULT(0),
        WM_NCHITTEST if de == De::Plano => {
            let (sx, sy) = xy(lp);
            let mut p = POINT { x: sx, y: sy };
            let mut r = RECT::default();
            // SAFETY: ventana propia; salidas locales.
            unsafe {
                let _ = ScreenToClient(hwnd, &mut p);
                let _ = GetClientRect(hwnd, &mut r);
            }
            let borde = 7;
            let (izq, der, arr, aba) = (p.x < borde, p.x >= r.right - borde, p.y < borde, p.y >= r.bottom - borde);
            let ht = match (izq, der, arr, aba) {
                (true, _, true, _) => HTTOPLEFT,
                (_, true, true, _) => HTTOPRIGHT,
                (true, _, _, true) => HTBOTTOMLEFT,
                (_, true, _, true) => HTBOTTOMRIGHT,
                (true, ..) => HTLEFT,
                (_, true, ..) => HTRIGHT,
                (_, _, true, _) => HTTOP,
                (.., true) => HTBOTTOM,
                _ => HTCLIENT,
            };
            return LRESULT(ht as isize);
        }
        WM_MOUSEACTIVATE if de == De::Barra => return LRESULT(MA_NOACTIVATE as isize),
        WM_SIZE => {
            let (w, h) = xy(lp);
            apuntar(de, Ev::Tamano(w.max(1) as u32, h.max(1) as u32));
            return LRESULT(0);
        }
        WM_MOUSEMOVE => {
            let (x, y) = xy(lp);
            apuntar(de, Ev::Mover(x, y));
            let mut t = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            // SAFETY: estructura local con su tamano.
            let _ = unsafe { TrackMouseEvent(&mut t) };
            return LRESULT(0);
        }
        WM_MOUSELEAVE => apuntar(de, Ev::Salir),
        WM_LBUTTONDOWN | WM_MBUTTONDOWN | WM_RBUTTONDOWN => {
            let (x, y) = xy(lp);
            if de == De::Plano {
                // SAFETY: ventana propia.
                unsafe { SetCapture(hwnd) };
            }
            apuntar(de, Ev::Bajar(x, y, if msg == WM_LBUTTONDOWN { 0 } else if msg == WM_MBUTTONDOWN { 1 } else { 2 }));
            return LRESULT(0);
        }
        WM_LBUTTONUP | WM_MBUTTONUP | WM_RBUTTONUP => {
            let (x, y) = xy(lp);
            // SAFETY: suelta la captura propia.
            let _ = unsafe { ReleaseCapture() };
            apuntar(de, Ev::Subir(x, y, if msg == WM_LBUTTONUP { 0 } else if msg == WM_MBUTTONUP { 1 } else { 2 }));
            return LRESULT(0);
        }
        WM_LBUTTONDBLCLK => {
            let (x, y) = xy(lp);
            apuntar(de, Ev::Doble(x, y));
            return LRESULT(0);
        }
        WM_MOUSEWHEEL => {
            let (sx, sy) = xy(lp);
            let mut p = POINT { x: sx, y: sy };
            // SAFETY: ventana propia; punto local.
            let _ = unsafe { ScreenToClient(hwnd, &mut p) };
            apuntar(de, Ev::Rueda(p.x, p.y, ((wp.0 >> 16) & 0xffff) as i16 as i32));
            return LRESULT(0);
        }
        WM_KEYDOWN => {
            apuntar(de, Ev::Tecla(wp.0 as u32));
            return LRESULT(0);
        }
        WM_COMMAND => {
            apuntar(de, Ev::Menu((wp.0 & 0xffff) as u32));
            return LRESULT(0);
        }
        WM_CLOSE => {
            apuntar(de, Ev::Cerrar);
            return LRESULT(0);
        }
        WM_ERASEBKGND => return LRESULT(1),
        WM_GETMINMAXINFO => {
            // SAFETY: en este mensaje `lp` es un MINMAXINFO valido.
            let mm = unsafe { &mut *(lp.0 as *mut MINMAXINFO) };
            mm.ptMinTrackSize = POINT { x: 160, y: 120 };
            return LRESULT(0);
        }
        _ => {}
    }
    // SAFETY: lo demas, a Windows.
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

fn registrar() -> windows::core::Result<windows::Win32::Foundation::HINSTANCE> {
    // SAFETY: clases propias; registrar dos veces falla sin efecto.
    unsafe {
        let inst: windows::Win32::Foundation::HINSTANCE = GetModuleHandleW(None)?.into();
        let icono = LoadIconW(Some(inst), PCWSTR(1 as _)).unwrap_or_default();
        let procs: [(PCWSTR, bool, WNDPROC); 2] = [
            (w!("PixPinPlanoCad"), true, Some(procedimiento_plano)),
            (w!("PixPinPlanoBarra"), false, Some(procedimiento_barra)),
        ];
        for (clase, dobles, proc_) in procs {
            let c = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: if dobles { CS_DBLCLKS } else { WNDCLASS_STYLES(0) },
                lpfnWndProc: proc_,
                hInstance: inst,
                hIcon: icono,
                hIconSm: icono,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: clase,
                ..Default::default()
            };
            RegisterClassExW(&c);
        }
        Ok(inst)
    }
}

fn crear_ventana(titulo: &str) -> windows::core::Result<(HWND, u32, u32)> {
    let inst = registrar()?;
    // SAFETY: ventana propia; cadenas vivas durante cada llamada.
    unsafe {
        // En el monitor del raton, el 60 % de su area de trabajo.
        let mut p = POINT::default();
        let _ = GetCursorPos(&mut p);
        let mon = MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(mon, &mut info);
        let t = info.rcWork;
        let (aw, ah) = (t.right - t.left, t.bottom - t.top);
        let (w, h) = ((aw as f32 * 0.6) as i32, (ah as f32 * 0.65) as i32);
        let titulo: Vec<u16> = titulo.encode_utf16().chain(std::iter::once(0)).collect();
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW | WS_EX_TOPMOST,
            w!("PixPinPlanoCad"),
            PCWSTR(titulo.as_ptr()),
            WS_POPUP | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU,
            t.left + (aw - w) / 2,
            t.top + (ah - h) / 2 + (ah as f32 * 0.03) as i32,
            w,
            h,
            None,
            None,
            Some(inst),
            None,
        )?;
        let si: i32 = 1;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &si as *const i32 as *const _, 4);
        // Un pixel de marco de DWM: la sombra de las ventanas de Windows.
        let _ = DwmExtendFrameIntoClientArea(hwnd, &MARGINS { cxLeftWidth: 1, cxRightWidth: 1, cyTopHeight: 1, cyBottomHeight: 1 });
        let _ = SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        Ok((hwnd, w as u32, h as u32))
    }
}

/// La barra: una ventanita sin activar, duena del plano (va siempre encima
/// de el), con las esquinas redondas de Windows 11.
fn crear_barra(dueno: HWND, w: i32, h: i32) -> windows::core::Result<HWND> {
    let inst = registrar()?;
    // SAFETY: ventana propia, nace oculta.
    unsafe {
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
            w!("PixPinPlanoBarra"),
            w!(""),
            WS_POPUP,
            0,
            0,
            w,
            h,
            Some(dueno),
            None,
            Some(inst),
            None,
        )?;
        let redondas: i32 = 2;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &redondas as *const i32 as *const _, 4);
        Ok(hwnd)
    }
}

/// La camara: que punto del plano (respecto a su origen) esta en el centro
/// y cuantas unidades del plano mide un pixel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camara {
    pub centro: [f64; 2],
    pub px: f64,
}

impl Camara {
    /// El plano entero en una ventana de `w` x `h`, con un margen.
    pub fn encuadrar(caja: [f32; 4], w: u32, h: u32) -> Camara {
        let (cw, ch) = ((caja[2] - caja[0]).max(1e-9) as f64, (caja[3] - caja[1]).max(1e-9) as f64);
        let px = (cw / (w.max(1) as f64 * 0.92)).max(ch / (h.max(1) as f64 * 0.92));
        Camara {
            centro: [(caja[0] + caja[2]) as f64 / 2.0, (caja[1] + caja[3]) as f64 / 2.0],
            px: if px.is_finite() && px > 0.0 { px } else { 1.0 },
        }
    }

    /// El punto del plano bajo el pixel (x, y) de una ventana `w` x `h`.
    pub fn plano(&self, x: f64, y: f64, w: u32, h: u32) -> [f64; 2] {
        [self.centro[0] + (x - w as f64 / 2.0) * self.px, self.centro[1] - (y - h as f64 / 2.0) * self.px]
    }

    /// El pixel de un punto del plano.
    pub fn pantalla(&self, p: [f64; 2], w: u32, h: u32) -> [f64; 2] {
        [(p[0] - self.centro[0]) / self.px + w as f64 / 2.0, h as f64 / 2.0 - (p[1] - self.centro[1]) / self.px]
    }

    /// Acercar `factor` (>1 aleja) dejando quieto el punto bajo (x, y).
    pub fn zoom(&self, factor: f64, x: f64, y: f64, w: u32, h: u32) -> Camara {
        let antes = self.plano(x, y, w, h);
        let px = (self.px * factor).clamp(1e-9, 1e9);
        let mut c = Camara { centro: self.centro, px };
        let despues = c.plano(x, y, w, h);
        c.centro = [c.centro[0] + antes[0] - despues[0], c.centro[1] + antes[1] - despues[1]];
        c
    }

    fn vista(&self, w: u32, h: u32, color7: [f32; 4]) -> Vista {
        Vista {
            escala: [(2.0 / (w as f64 * self.px)) as f32, (2.0 / (h as f64 * self.px)) as f32],
            centro: [self.centro[0] as f32, self.centro[1] as f32],
            color7,
            px: self.px as f32,
            relleno: [0.0; 3],
        }
    }
}

// ------------------------------------------------------------------ enganche

/// Los puntos donde se engancha la cota: los vertices de lo dibujado y los
/// centros y extremos de los circulos y arcos. Ordenados por celda para
/// buscar solo cerca del cursor.
pub struct Enganches {
    celda: f64,
    claves: Vec<(u64, [f32; 2])>,
}

fn clave(x: f64, y: f64, celda: f64) -> u64 {
    let cx = ((x / celda).floor() as i64 + (1 << 30)) as u64 & 0xffff_ffff;
    let cy = ((y / celda).floor() as i64 + (1 << 30)) as u64 & 0xffff_ffff;
    cx << 32 | cy
}

impl Enganches {
    pub fn de(m: &Modelo) -> Enganches {
        let lado = ((m.caja[2] - m.caja[0]).max(m.caja[3] - m.caja[1]) as f64).max(1e-6);
        let celda = lado / 256.0;
        let mut puntos: Vec<[f32; 2]> = m.vertices.iter().map(|v| [v.x, v.y]).collect();
        for a in &m.arcos {
            puntos.push(a.centro);
            if a.barrido < 6.28 {
                for ang in [a.inicio, a.inicio + a.barrido] {
                    puntos.push([a.centro[0] + a.radio * ang.cos(), a.centro[1] + a.radio * ang.sin()]);
                }
            } else {
                for k in 0..4 {
                    let ang = k as f32 * std::f32::consts::FRAC_PI_2;
                    puntos.push([a.centro[0] + a.radio * ang.cos(), a.centro[1] + a.radio * ang.sin()]);
                }
            }
        }
        let mut claves: Vec<(u64, [f32; 2])> = puntos.into_iter().map(|p| (clave(p[0] as f64, p[1] as f64, celda), p)).collect();
        claves.sort_unstable_by_key(|c| c.0);
        claves.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
        Enganches { celda, claves }
    }

    /// El punto mas cercano a `p` a menos de `radio` (unidades del plano).
    pub fn cerca(&self, p: [f64; 2], radio: f64) -> Option<[f64; 2]> {
        let n = (radio / self.celda).ceil() as i64;
        if n > 8 {
            return None; // muy de lejos: no vale la pena
        }
        let (cx, cy) = ((p[0] / self.celda).floor() as i64, (p[1] / self.celda).floor() as i64);
        let mut mejor: Option<([f64; 2], f64)> = None;
        for i in cx - n..=cx + n {
            for j in cy - n..=cy + n {
                let k = ((i + (1 << 30)) as u64 & 0xffff_ffff) << 32 | ((j + (1 << 30)) as u64 & 0xffff_ffff);
                let desde = self.claves.partition_point(|c| c.0 < k);
                for (kk, q) in &self.claves[desde..] {
                    if *kk != k {
                        break;
                    }
                    let d = (q[0] as f64 - p[0]).hypot(q[1] as f64 - p[1]);
                    if d <= radio && mejor.is_none_or(|(_, m)| d < m) {
                        mejor = Some(([q[0] as f64, q[1] as f64], d));
                    }
                }
            }
        }
        mejor.map(|(q, _)| q)
    }
}

/// Las unidades del plano para escribir una medida.
pub fn sufijo(unidades: u32) -> &'static str {
    match unidades {
        1 => " in",
        2 => " ft",
        4 => " mm",
        5 => " cm",
        6 => " m",
        7 => " km",
        14 => " dm",
        _ => "",
    }
}

/// Una medida con sus decimales justos y su unidad.
pub fn medida(d: f64, unidades: u32) -> String {
    let dec = if d >= 1000.0 {
        1
    } else if d >= 100.0 {
        2
    } else {
        3
    };
    format!("{:.*}{}", dec, d, sufijo(unidades))
}

// ------------------------------------------------------------------ pintar

const BOTONES: [&str; 3] = ["asa", "tema", "acotar"];
const ALTO_BARRA: f64 = 48.0;
const BOTON: f64 = 40.0;

fn ancho_barra(e: f64) -> f64 {
    (4.0 + 18.0 + 2.0 + BOTON + 2.0 + BOTON + 4.0) * e
}

/// Donde cae cada boton de la barra (x desde, x hasta).
fn botones_barra(e: f64) -> [(f64, f64); 3] {
    let x0 = 4.0 * e;
    let asa = (x0, x0 + 18.0 * e);
    let t = asa.1 + 2.0 * e;
    let tema = (t, t + BOTON * e);
    let a = tema.1 + 2.0 * e;
    [asa, tema, (a, a + BOTON * e)]
}

/// Lo que va encima se dibuja despues: el modelo ordena por tamano (mayor
/// primero) y por sitio, asi que en la barra y en las cotas el «tamano» es
/// la capa: fondo, rayas, pastillas y marcas, en ese orden.
const CAPA_FONDO: f64 = 1e9;
const CAPA_RAYAS: f64 = 1e7;
const CAPA_ENCIMA: f64 = 1e5;
const CAPA_ARRIBA: f64 = 1e3;

fn rect_en(c: &mut Constructor, x0: f64, y0: f64, x1: f64, y1: f64, color: u32, capa: f64) {
    c.triangulos(&[[x0, y0], [x1, y0], [x0, y1], [x1, y0], [x1, y1], [x0, y1]], color, Some(capa));
}

fn rect(c: &mut Constructor, x0: f64, y0: f64, x1: f64, y1: f64, color: u32) {
    rect_en(c, x0, y0, x1, y1, color, CAPA_ENCIMA);
}

/// Una raya gruesa (de `g` pixeles) en pantalla.
fn raya(c: &mut Constructor, a: [f64; 2], b: [f64; 2], g: f64, color: u32) {
    raya_en(c, a, b, g, color, CAPA_RAYAS);
}

fn raya_en(c: &mut Constructor, a: [f64; 2], b: [f64; 2], g: f64, color: u32, capa: f64) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l = dx.hypot(dy);
    if l < 1e-9 {
        return;
    }
    let (nx, ny) = (-dy / l * g / 2.0, dx / l * g / 2.0);
    c.triangulos(
        &[
            [a[0] + nx, a[1] + ny],
            [a[0] - nx, a[1] - ny],
            [b[0] + nx, b[1] + ny],
            [a[0] - nx, a[1] - ny],
            [b[0] - nx, b[1] - ny],
            [b[0] + nx, b[1] + ny],
        ],
        color,
        Some(capa),
    );
}

fn barra(e: f64, encima: Option<usize>, claro: bool, acotando: bool) -> Modelo {
    let mut c = Constructor::nuevo();
    let (fondo, tinta, hover, tenue) = if claro {
        (rgba(0xF9, 0xF9, 0xFB), rgba(0x1C, 0x1C, 0x1E), 0x14000000u32, rgba(0x6E, 0x6E, 0x73))
    } else {
        (rgba(0x1E, 0x1E, 0x20), rgba(0xF5, 0xF5, 0xF7), 0x24ffffffu32, rgba(0xC7, 0xC7, 0xCC))
    };
    let (w, h) = (ancho_barra(e), ALTO_BARRA * e);
    rect_en(&mut c, 0.0, 0.0, w, h, fondo, CAPA_FONDO);
    let b = botones_barra(e);
    let y0 = (h - BOTON * e) / 2.0;
    let azul = rgba(0x00, 0x60, 0xDF);
    for (i, (x0, x1)) in b.iter().enumerate() {
        let activo = BOTONES[i] == "acotar" && acotando;
        if activo {
            rect_en(&mut c, *x0, y0, *x1, y0 + BOTON * e, azul, CAPA_RAYAS * 10.0);
        } else if encima == Some(i) && i > 0 {
            rect_en(&mut c, *x0, y0, *x1, y0 + BOTON * e, hover, CAPA_RAYAS * 10.0);
        }
        let (cx, cy) = ((x0 + x1) / 2.0, h / 2.0);
        let r = 8.0 * e;
        let col = if activo { rgba(255, 255, 255) } else { tinta };
        match BOTONES[i] {
            "asa" => {
                // Seis puntos: de aqui se arrastra la ventana.
                for (dx, dy) in [(-3.0, -6.0), (3.0, -6.0), (-3.0, 0.0), (3.0, 0.0), (-3.0, 6.0), (3.0, 6.0)] {
                    let (px, py) = (cx + dx * e, cy + dy * e);
                    rect(&mut c, px - 1.2 * e, py - 1.2 * e, px + 1.2 * e, py + 1.2 * e, tenue);
                }
            }
            "tema" => {
                let circ: Vec<[f64; 2]> = (0..=40)
                    .map(|k| {
                        let a = k as f64 / 40.0 * std::f64::consts::TAU;
                        [cx + r * a.cos(), cy + r * a.sin()]
                    })
                    .collect();
                for v in circ.windows(2) {
                    raya(&mut c, v[0], v[1], 1.6 * e, col);
                }
                let mut tri = Vec::new();
                for k in 0..20 {
                    let a0 = -std::f64::consts::FRAC_PI_2 + k as f64 / 20.0 * std::f64::consts::PI;
                    let a1 = -std::f64::consts::FRAC_PI_2 + (k + 1) as f64 / 20.0 * std::f64::consts::PI;
                    tri.extend_from_slice(&[[cx, cy], [cx + r * a0.cos(), cy + r * a0.sin()], [cx + r * a1.cos(), cy + r * a1.sin()]]);
                }
                c.triangulos(&tri, col, Some(CAPA_RAYAS));
            }
            _ => {
                // Una regla en diagonal con sus marcas.
                let (a, bb) = ([cx - 9.0 * e, cy + 5.0 * e], [cx + 9.0 * e, cy - 5.0 * e]);
                let (dx, dy) = ((bb[0] - a[0]) / 1.0, (bb[1] - a[1]) / 1.0);
                let l = dx.hypot(dy);
                let (ux, uy) = (dx / l, dy / l);
                let (nx, ny) = (-uy, ux);
                let ancho = 4.5 * e;
                let p = |t: f64, s: f64| [a[0] + ux * t + nx * s, a[1] + uy * t + ny * s];
                for (t0, t1, s0, s1) in [(0.0, l, -ancho, -ancho), (0.0, l, ancho, ancho), (0.0, 0.0, -ancho, ancho), (l, l, -ancho, ancho)] {
                    raya(&mut c, p(t0, s0), p(t1, s1), 1.5 * e, col);
                }
                for k in 1..5 {
                    let t = l * k as f64 / 5.0;
                    let largo = if k % 2 == 0 { ancho * 0.9 } else { ancho * 0.4 };
                    raya(&mut c, p(t, -ancho), p(t, -ancho + largo), 1.3 * e, col);
                }
            }
        }
    }
    c.terminar()
}

/// Las cotas y lo demas que va encima del plano, en pixeles.
struct Acotar {
    activo: bool,
    puntos: Vec<[f64; 2]>,
    hechas: Vec<Vec<[f64; 2]>>,
    cursor: Option<[f64; 2]>,
    enganche: Option<[f64; 2]>,
}

#[allow(clippy::too_many_arguments)]
fn encima_del_plano(textos: &mut Textos, a: &Acotar, cam: &Camara, w: u32, h: u32, e: f64, unidades: u32, claro: bool, pista: &str) -> Modelo {
    let mut c = Constructor::nuevo();
    let naranja = rgba(0xFF, 0x8A, 0x00);
    let verde = rgba(0x30, 0xD1, 0x58);
    let (pildora, letra) = if claro { (0xEE1C1C1Eu32, rgba(255, 255, 255)) } else { (0xEEF5F5F7u32, rgba(0x1C, 0x1C, 0x1E)) };
    let a_px = |p: [f64; 2]| cam.pantalla(p, w, h);
    let tam = 12.5 * e;
    let mut etiqueta = |c: &mut Constructor, t: &str, x: f64, y: f64| {
        let ancho = textos.medir_pantalla(t, tam);
        let (x0, y0) = (x - ancho / 2.0 - 6.0 * e, y - tam * 0.95);
        rect(c, x0, y0, x0 + ancho + 12.0 * e, y0 + tam * 1.55, pildora);
        textos.en_pantalla(c, t, x - ancho / 2.0, y + tam * 0.25, tam, letra, x + ancho + 50.0);
    };
    let mut cadenas: Vec<(&Vec<[f64; 2]>, bool)> = a.hechas.iter().map(|v| (v, false)).collect();
    let mut viva = a.puntos.clone();
    if a.activo && !a.puntos.is_empty()
        && let Some(cur) = a.enganche.or(a.cursor)
    {
        viva.push(cur);
    }
    cadenas.push((&viva, true));
    for (cad, es_viva) in cadenas {
        if cad.is_empty() {
            continue;
        }
        let mut total = 0.0;
        for par in cad.windows(2) {
            let (p, q) = (a_px(par[0]), a_px(par[1]));
            raya(&mut c, p, q, 2.0 * e, naranja);
            let d = (par[1][0] - par[0][0]).hypot(par[1][1] - par[0][1]);
            total += d;
            let (mx, my) = ((p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0);
            let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
            let l = dx.hypot(dy).max(1e-9);
            let (nx, ny) = (-dy / l, dx / l);
            let lado = if ny > 0.0 { -1.0 } else { 1.0 };
            etiqueta(&mut c, &medida(d, unidades), mx + nx * lado * 14.0 * e, my + ny * lado * 14.0 * e);
        }
        for p in cad.iter() {
            let q = a_px(*p);
            rect(&mut c, q[0] - 3.0 * e, q[1] - 3.0 * e, q[0] + 3.0 * e, q[1] + 3.0 * e, naranja);
        }
        if cad.len() > 2 {
            let u = a_px(*cad.last().unwrap_or(&[0.0, 0.0]));
            etiqueta(&mut c, &format!("Σ {}", medida(total, unidades)), u[0] + 40.0 * e, u[1] + 22.0 * e);
        }
        let _ = es_viva;
    }
    if a.activo {
        if let Some(g) = a.enganche {
            let q = a_px(g);
            let s = 6.0 * e;
            for (p0, p1) in [([-s, -s], [s, -s]), ([s, -s], [s, s]), ([s, s], [-s, s]), ([-s, s], [-s, -s])] {
                raya_en(&mut c, [q[0] + p0[0], q[1] + p0[1]], [q[0] + p1[0], q[1] + p1[1]], 1.8 * e, verde, CAPA_ARRIBA);
            }
        }
        // La pista, abajo a la izquierda.
        let ancho = textos.medir_pantalla(pista, tam);
        let (x0, y0) = (12.0 * e, h as f64 - 34.0 * e);
        rect(&mut c, x0, y0, x0 + ancho + 20.0 * e, y0 + 24.0 * e, pildora);
        textos.en_pantalla(&mut c, pista, x0 + 10.0 * e, y0 + 16.5 * e, tam, letra, x0 + ancho + 40.0 * e);
    }
    c.terminar()
}

/// Un aviso en el centro (abriendo, error).
fn aviso(textos: &mut Textos, texto: &str, w: u32, h: u32, escala: f64, claro: bool) -> Modelo {
    let mut c = Constructor::nuevo();
    let tam = 15.0 * escala;
    let tinta = if claro { rgba(0x6b, 0x6a, 0x66) } else { rgba(0x9a, 0x98, 0x93) };
    let ancho = textos.medir_pantalla(texto, tam);
    textos.en_pantalla(&mut c, texto, (w as f64 - ancho) / 2.0, h as f64 / 2.0, tam, tinta, w as f64);
    c.terminar()
}

/// La vista de un modelo dibujado en pixeles de la ventana (y hacia abajo).
fn vista_pantalla(m: &Modelo, w: u32, h: u32) -> Vista {
    Vista {
        escala: [2.0 / w as f32, -2.0 / h as f32],
        centro: [(w as f64 / 2.0 - m.origen[0]) as f32, (h as f64 / 2.0 - m.origen[1]) as f32],
        color7: [1.0; 4],
        px: 1.0,
        relleno: [0.0; 3],
    }
}

/// Donde va la barra: encima del plano si cabe, si no debajo, si no dentro.
pub fn colocar_barra(plano: RECT, trabajo: RECT, ancho: i32, alto: i32, sep: i32) -> (i32, i32) {
    let x = plano.left.clamp(trabajo.left, (trabajo.right - ancho).max(trabajo.left));
    let arriba = plano.top - sep - alto;
    if arriba >= trabajo.top {
        return (x, arriba);
    }
    let abajo = plano.bottom + sep;
    if abajo + alto <= trabajo.bottom {
        return (x, abajo);
    }
    (x + sep, plano.top.max(trabajo.top) + sep)
}

const M_TODO: u32 = 1;
const M_TEMA: u32 = 2;
const M_ACOTAR: u32 = 3;
const M_BORRAR: u32 = 4;
const M_ENCIMA: u32 = 5;
const M_CERRAR: u32 = 6;

fn menu(hwnd: HWND, t: &TextosUi, acotando: bool, hay_cotas: bool, fijada: bool) {
    // SAFETY: menu propio que se destruye antes de salir; cadenas vivas.
    unsafe {
        let Ok(m) = CreatePopupMenu() else { return };
        let poner = |id: u32, texto: &str, marca: bool, gris: bool| {
            let v: Vec<u16> = texto.encode_utf16().chain(std::iter::once(0)).collect();
            let mut f = MF_STRING;
            if marca {
                f |= MF_CHECKED;
            }
            if gris {
                f |= MF_GRAYED;
            }
            let _ = AppendMenuW(m, f, id as usize, PCWSTR(v.as_ptr()));
        };
        poner(M_TODO, &t.todo, false, false);
        poner(M_TEMA, &t.tema, false, false);
        poner(M_ACOTAR, &t.acotar, acotando, false);
        poner(M_BORRAR, &t.borrar_cotas, false, !hay_cotas);
        let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
        poner(M_ENCIMA, &t.encima, fijada, false);
        poner(M_CERRAR, &t.cerrar, false, false);
        let mut p = POINT::default();
        let _ = GetCursorPos(&mut p);
        let _ = TrackPopupMenu(m, TPM_RIGHTBUTTON, p.x, p.y, Some(0), hwnd, None);
        let _ = DestroyMenu(m);
    }
}

/// **Abre la ventana del plano** y no vuelve hasta que se cierra. `cargando`
/// trae el plano cuando esta listo (o el porque no).
pub fn ver(titulo: &str, cargando: Receiver<Result<Modelo, String>>, textos_ui: TextosUi) -> Result<(), String> {
    let (hwnd, mut w, mut h) = crear_ventana(&format!("{titulo} — PixPin")).map_err(|e| e.to_string())?;
    let mut gpu = match Gpu::nueva(hwnd, w, h) {
        Ok(g) => g,
        Err(e) => {
            // SAFETY: ventana propia.
            let _ = unsafe { DestroyWindow(hwnd) };
            return Err(format!("sin Direct3D: {e}"));
        }
    };
    // SAFETY: ventana propia.
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let e = dpi as f64 / 96.0;
    let (bw, bh) = (ancho_barra(e).ceil() as i32, (ALTO_BARRA * e).ceil() as i32);
    let hbarra = crear_barra(hwnd, bw, bh).map_err(|e| e.to_string())?;
    let mut gpu_barra = Gpu::nueva(hbarra, bw as u32, bh as u32).map_err(|e| e.to_string())?;
    let mut textos = Textos::nuevo();
    let mut plano: Option<(Modelo, PlanoGpu)> = None;
    let mut enganches: Option<Enganches> = None;
    let mut mensaje = textos_ui.abriendo.clone();
    let mut camara = Camara { centro: [0.0, 0.0], px: 1.0 };
    let mut destino = camara;
    let mut claro = false;
    let mut fijada = true;
    let mut dentro_plano = false;
    let mut dentro_barra = false;
    let mut fuera_desde: Option<Instant> = None;
    let mut barra_visible = false;
    let mut arrastre: Option<(i32, i32, Camara, bool)> = None;
    let mut boton_encima: Option<usize> = None;
    let mut acotar = Acotar {
        activo: false,
        puntos: Vec::new(),
        hechas: Vec::new(),
        cursor: None,
        enganche: None,
    };
    let mut raton = (0i32, 0i32);
    let mut sucio = true;
    let mut barra_sucia = true;
    let mut aviso_ui: Option<(Modelo, PlanoGpu)> = None;
    'bucle: loop {
        let animando = camara != destino;
        // SAFETY: bucle de mensajes de este hilo.
        unsafe {
            if !animando && !sucio && !barra_sucia {
                let espera = if plano.is_none() || barra_visible || fuera_desde.is_some() { 50 } else { INFINITE };
                let _ = MsgWaitForMultipleObjectsEx(None, espera, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
            }
            let mut m = MSG::default();
            while PeekMessageW(&mut m, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&m);
                DispatchMessageW(&m);
            }
        }
        // El plano, cuando llega.
        if plano.is_none() && mensaje == textos_ui.abriendo {
            match cargando.try_recv() {
                Ok(Ok(m)) => {
                    if m.vacio() {
                        mensaje = textos_ui.vacio.clone();
                    } else {
                        match gpu.subir(&m) {
                            Ok(p) => {
                                camara = Camara::encuadrar(m.caja, w, h);
                                destino = camara;
                                plano = Some((m, p));
                                mensaje.clear();
                            }
                            Err(err) => mensaje = format!("{}: {err}", textos_ui.error),
                        }
                    }
                    aviso_ui = None;
                    sucio = true;
                }
                Ok(Err(err)) => {
                    mensaje = format!("{}: {err}", textos_ui.error);
                    aviso_ui = None;
                    sucio = true;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    mensaje = textos_ui.error.clone();
                    aviso_ui = None;
                    sucio = true;
                }
            }
        }
        let eventos: Vec<(De, Ev)> = ESTADO.with(|s| s.borrow_mut().eventos.drain(..).collect());
        let b_barra = botones_barra(e);
        let boton_en = |x: i32| -> Option<usize> { b_barra.iter().position(|(a, b)| (x as f64) >= *a && (x as f64) < *b) };
        let mut cambiar_tema = false;
        let mut cambiar_acotar = false;
        let mut cambiar_encima = false;
        let mut borrar_cotas = false;
        let mut encuadrar = false;
        for (de, ev) in eventos {
            match (de, ev) {
                (_, Ev::Cerrar) => break 'bucle,
                (De::Barra, Ev::Mover(x, _)) => {
                    dentro_barra = true;
                    let b = boton_en(x);
                    if b != boton_encima {
                        boton_encima = b;
                        barra_sucia = true;
                    }
                }
                (De::Barra, Ev::Salir) => {
                    dentro_barra = false;
                    boton_encima = None;
                    barra_sucia = true;
                }
                (De::Barra, Ev::Bajar(x, _, 0)) => match boton_en(x) {
                    Some(0) => {
                        // El asa: Windows arrastra la ventana del plano.
                        // SAFETY: ventanas propias.
                        unsafe {
                            let _ = ReleaseCapture();
                            SendMessageW(hwnd, WM_NCLBUTTONDOWN, Some(WPARAM(HTCAPTION as usize)), Some(LPARAM(0)));
                        }
                    }
                    Some(1) => cambiar_tema = true,
                    Some(2) => cambiar_acotar = true,
                    _ => {}
                },
                (De::Barra, _) => {}
                (De::Plano, Ev::Tamano(nw, nh)) => {
                    if (nw, nh) != (w, h) {
                        w = nw;
                        h = nh;
                        let _ = gpu.redimensionar(w, h);
                        aviso_ui = None;
                        sucio = true;
                    }
                }
                (De::Plano, Ev::Mover(x, y)) => {
                    raton = (x, y);
                    dentro_plano = true;
                    if let Some((x0, y0, c0, ventana)) = arrastre {
                        if ventana {
                            continue;
                        }
                        if (x - x0).abs() + (y - y0).abs() > 3 || !acotar.activo {
                            let c = Camara {
                                centro: [c0.centro[0] - (x - x0) as f64 * c0.px, c0.centro[1] + (y - y0) as f64 * c0.px],
                                px: c0.px,
                            };
                            camara = c;
                            destino = c;
                            sucio = true;
                        }
                    }
                    if acotar.activo {
                        let p = camara.plano(x as f64, y as f64, w, h);
                        acotar.cursor = Some(p);
                        acotar.enganche = enganches.as_ref().and_then(|g| g.cerca(p, 12.0 * e * camara.px));
                        sucio = true;
                    }
                }
                (De::Plano, Ev::Salir) => {
                    dentro_plano = false;
                    if acotar.activo {
                        acotar.cursor = None;
                        acotar.enganche = None;
                        sucio = true;
                    }
                }
                (De::Plano, Ev::Bajar(x, y, b)) => {
                    // SAFETY: lee el estado de la tecla Ctrl.
                    let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
                    if b == 0 && ctrl {
                        // Ctrl + arrastrar: mover la ventana.
                        // SAFETY: ventana propia.
                        unsafe {
                            let _ = ReleaseCapture();
                            SendMessageW(hwnd, WM_NCLBUTTONDOWN, Some(WPARAM(HTCAPTION as usize)), Some(LPARAM(0)));
                        }
                    } else if b <= 1 {
                        arrastre = Some((x, y, camara, false));
                    } else {
                        menu(hwnd, &textos_ui, acotar.activo, !acotar.hechas.is_empty() || !acotar.puntos.is_empty(), fijada);
                    }
                }
                (De::Plano, Ev::Subir(x, y, b)) => {
                    let clic = arrastre.is_some_and(|(x0, y0, _, _)| (x - x0).abs() + (y - y0).abs() <= 3);
                    arrastre = None;
                    if b == 0 && clic && acotar.activo && plano.is_some() {
                        let p = camara.plano(x as f64, y as f64, w, h);
                        let p = enganches.as_ref().and_then(|g| g.cerca(p, 12.0 * e * camara.px)).unwrap_or(p);
                        if acotar.puntos.last() != Some(&p) {
                            acotar.puntos.push(p);
                        }
                        sucio = true;
                    }
                }
                (De::Plano, Ev::Doble(..)) => {
                    if acotar.activo {
                        // El doble clic deja la medida (y su segundo clic ya
                        // puso el ultimo punto).
                        if acotar.puntos.len() >= 2 {
                            acotar.hechas.push(std::mem::take(&mut acotar.puntos));
                        }
                        acotar.puntos.clear();
                        sucio = true;
                    } else {
                        encuadrar = true;
                    }
                }
                (De::Plano, Ev::Rueda(x, y, d)) => {
                    let factor = 1.25f64.powf(-(d as f64) / 120.0);
                    destino = destino.zoom(factor, x as f64, y as f64, w, h);
                }
                (De::Plano, Ev::Menu(id)) => match id {
                    M_TODO => encuadrar = true,
                    M_TEMA => cambiar_tema = true,
                    M_ACOTAR => cambiar_acotar = true,
                    M_BORRAR => borrar_cotas = true,
                    M_ENCIMA => cambiar_encima = true,
                    M_CERRAR => break 'bucle,
                    _ => {}
                },
                (De::Plano, Ev::Tecla(vk)) => match vk {
                    0x1B => {
                        if !acotar.puntos.is_empty() {
                            acotar.puntos.clear();
                            sucio = true;
                        } else if acotar.activo {
                            cambiar_acotar = true;
                        } else {
                            break 'bucle;
                        }
                    }
                    0x0D => {
                        if acotar.puntos.len() >= 2 {
                            acotar.hechas.push(std::mem::take(&mut acotar.puntos));
                            sucio = true;
                        }
                    }
                    0x08 => {
                        acotar.puntos.pop();
                        sucio = true;
                    }
                    0x2E => borrar_cotas = true,
                    0x46 | 0x24 => encuadrar = true,
                    0x42 => cambiar_tema = true,
                    0x4D => cambiar_acotar = true,
                    0x54 => cambiar_encima = true,
                    0xBB | 0x6B => destino = destino.zoom(1.0 / 1.4, w as f64 / 2.0, h as f64 / 2.0, w, h),
                    0xBD | 0x6D => destino = destino.zoom(1.4, w as f64 / 2.0, h as f64 / 2.0, w, h),
                    0x25 => destino.centro[0] -= w as f64 * 0.15 * destino.px,
                    0x27 => destino.centro[0] += w as f64 * 0.15 * destino.px,
                    0x26 => destino.centro[1] += h as f64 * 0.15 * destino.px,
                    0x28 => destino.centro[1] -= h as f64 * 0.15 * destino.px,
                    _ => {}
                },
            }
        }
        if encuadrar && let Some((m, _)) = &plano {
            destino = Camara::encuadrar(m.caja, w, h);
        }
        if cambiar_tema {
            claro = !claro;
            barra_sucia = true;
            aviso_ui = None;
            sucio = true;
        }
        if cambiar_acotar {
            acotar.activo = !acotar.activo;
            acotar.puntos.clear();
            if acotar.activo && enganches.is_none()
                && let Some((m, _)) = &plano
            {
                enganches = Some(Enganches::de(m));
            }
            if acotar.activo {
                acotar.cursor = Some(camara.plano(raton.0 as f64, raton.1 as f64, w, h));
            }
            // SAFETY: cursor del sistema.
            unsafe {
                let _ = SetCursor(LoadCursorW(None, if acotar.activo { IDC_CROSS } else { IDC_ARROW }).ok());
            }
            barra_sucia = true;
            sucio = true;
        }
        if borrar_cotas {
            acotar.hechas.clear();
            acotar.puntos.clear();
            sucio = true;
        }
        if cambiar_encima {
            fijada = !fijada;
            poner_encima(hwnd, fijada);
        }
        if acotar.activo {
            // SAFETY: cursor del sistema, sobre la ventana propia.
            unsafe {
                if dentro_plano {
                    let _ = SetCursor(LoadCursorW(None, IDC_CROSS).ok());
                }
            }
        }
        // La barra: sale con el raton encima del plano o de ella, y se va
        // un poco despues de salir de los dos.
        let quiere = dentro_plano || dentro_barra || arrastre.is_some();
        if quiere {
            fuera_desde = None;
        } else if barra_visible && fuera_desde.is_none() {
            fuera_desde = Some(Instant::now());
        }
        let mostrar = quiere || fuera_desde.is_some_and(|t| t.elapsed() < Duration::from_millis(600));
        if !mostrar {
            fuera_desde = None;
        }
        if mostrar && plano.is_some() {
            // Donde va, segun donde este ahora el plano.
            let mut r = RECT::default();
            // SAFETY: ventanas propias; estructuras locales.
            unsafe {
                let _ = GetWindowRect(hwnd, &mut r);
                let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                let mut info = MONITORINFO {
                    cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                let _ = GetMonitorInfoW(mon, &mut info);
                let (x, y) = colocar_barra(r, info.rcWork, bw, bh, (8.0 * e) as i32);
                let _ = SetWindowPos(hbarra, Some(HWND_TOPMOST), x, y, bw, bh, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            }
            if !barra_visible {
                barra_visible = true;
                barra_sucia = true;
            }
        } else if barra_visible {
            barra_visible = false;
            // SAFETY: ventana propia.
            let _ = unsafe { ShowWindow(hbarra, SW_HIDE) };
        }
        // La camara va hacia su destino: suave, sin pasarse.
        if camara != destino {
            let k = 0.38;
            let px = (camara.px.ln() + (destino.px.ln() - camara.px.ln()) * k).exp();
            let centro = [
                camara.centro[0] + (destino.centro[0] - camara.centro[0]) * k,
                camara.centro[1] + (destino.centro[1] - camara.centro[1]) * k,
            ];
            camara = if (px - destino.px).abs() < destino.px * 1e-4
                && (centro[0] - destino.centro[0]).abs() < destino.px * 0.05
                && (centro[1] - destino.centro[1]).abs() < destino.px * 0.05
            {
                destino
            } else {
                Camara { centro, px }
            };
            sucio = true;
        }
        if barra_sucia && barra_visible {
            let m = barra(e, boton_encima, claro, acotar.activo);
            if let Ok(p) = gpu_barra.subir(&m) {
                let (gw, gh) = gpu_barra.tamano();
                let fondo = if claro { [0.976, 0.976, 0.984, 1.0] } else { [0.118, 0.118, 0.125, 1.0] };
                let _ = gpu_barra.dibujar(fondo, &[(&p, vista_pantalla(&m, gw, gh))]);
            }
            barra_sucia = false;
        }
        if aviso_ui.is_none() && !mensaje.is_empty() {
            let m = aviso(&mut textos, &mensaje, w, h, e, claro);
            aviso_ui = gpu.subir(&m).ok().map(|p| (m, p));
            sucio = true;
        }
        if sucio {
            let fondo = if claro { [0.99, 0.988, 0.976, 1.0] } else { [0.13, 0.15, 0.19, 1.0] };
            let color7 = if claro { [0.0, 0.0, 0.0, 1.0] } else { [1.0, 1.0, 1.0, 1.0] };
            let mut capas: Vec<(&PlanoGpu, Vista)> = Vec::new();
            if let Some((_, p)) = &plano {
                capas.push((p, camara.vista(w, h, color7)));
            }
            if mensaje.is_empty() {
                aviso_ui = None;
            } else if let Some((m, p)) = &aviso_ui {
                capas.push((p, vista_pantalla(m, w, h)));
            }
            // Las cotas, encima de todo.
            let encima = if acotar.activo || !acotar.hechas.is_empty() {
                let unidades = plano.as_ref().map_or(0, |(m, _)| m.unidades);
                let m = encima_del_plano(&mut textos, &acotar, &camara, w, h, e, unidades, claro, &textos_ui.pista_acotar);
                gpu.subir(&m).ok().map(|p| (m, p))
            } else {
                None
            };
            if let Some((m, p)) = &encima {
                capas.push((p, vista_pantalla(m, w, h)));
            }
            if let Err(err) = gpu.dibujar(fondo, &capas) {
                tracing::warn!(error = %err, "no se pudo dibujar el plano");
            }
            sucio = false;
        }
    }
    // SAFETY: ventanas propias.
    unsafe {
        let _ = DestroyWindow(hbarra);
        let _ = DestroyWindow(hwnd);
    }
    Ok(())
}

fn poner_encima(hwnd: HWND, si: bool) {
    // SAFETY: ventana propia.
    let _ = unsafe { SetWindowPos(hwnd, Some(if si { HWND_TOPMOST } else { HWND_NOTOPMOST }), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
}

/// Los textos de la ventana, en el idioma de la app.
#[derive(Debug, Clone)]
pub struct TextosUi {
    pub abriendo: String,
    pub vacio: String,
    pub error: String,
    pub pista_acotar: String,
    pub todo: String,
    pub tema: String,
    pub acotar: String,
    pub borrar_cotas: String,
    pub encima: String,
    pub cerrar: String,
}

impl Default for TextosUi {
    fn default() -> Self {
        Self {
            abriendo: "Abriendo el plano…".into(),
            vacio: "El plano no tiene nada que dibujar".into(),
            error: "No se pudo abrir el plano".into(),
            pista_acotar: "Clic en los puntos para medir · Doble clic o Intro: terminar · Esc: salir".into(),
            todo: "Ver todo el plano\tF".into(),
            tema: "Fondo claro u oscuro\tB".into(),
            acotar: "Acotar plano\tM".into(),
            borrar_cotas: "Borrar las cotas\tSupr".into(),
            encima: "Siempre encima\tT".into(),
            cerrar: "Cerrar\tEsc".into(),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_rueda_deja_quieto_el_punto_bajo_el_cursor() {
        let c = Camara { centro: [100.0, 50.0], px: 2.0 };
        let antes = c.plano(30.0, 400.0, 800, 600);
        let z = c.zoom(0.5, 30.0, 400.0, 800, 600);
        let despues = z.plano(30.0, 400.0, 800, 600);
        assert!((antes[0] - despues[0]).abs() < 1e-9 && (antes[1] - despues[1]).abs() < 1e-9);
        assert_eq!(z.px, 1.0);
        // Ida y vuelta entre pantalla y plano.
        let p = c.pantalla(antes, 800, 600);
        assert!((p[0] - 30.0).abs() < 1e-9 && (p[1] - 400.0).abs() < 1e-9);
    }

    #[test]
    fn encuadrar_mete_el_plano_entero_con_margen() {
        let c = Camara::encuadrar([-100.0, -10.0, 100.0, 10.0], 1000, 500);
        assert_eq!(c.centro, [0.0, 0.0]);
        assert!((c.px - 200.0 / 920.0).abs() < 1e-9);
        // Caso negativo: una caja vacia no da una escala absurda.
        assert!(Camara::encuadrar([0.0; 4], 100, 100).px > 0.0);
    }

    #[test]
    fn la_cota_se_engancha_al_vertice_cercano_y_no_al_lejano() {
        let mut c = Constructor::nuevo();
        c.polilinea(&[[0.0, 0.0], [100.0, 0.0], [100.0, 50.0]], 0, None);
        c.arco([50.0, 25.0], 10.0, 0.0, std::f64::consts::TAU, 0);
        let m = c.terminar();
        let g = Enganches::de(&m);
        let o = m.origen;
        // Cerca de (100,0) del plano (en coordenadas del modelo, relativas).
        let p = g.cerca([100.0 - o[0] + 0.3, 0.0 - o[1] - 0.2], 1.0).unwrap();
        assert!((p[0] - (100.0 - o[0])).abs() < 1e-4);
        // El centro del circulo tambien engancha.
        assert!(g.cerca([50.0 - o[0] + 0.5, 25.0 - o[1]], 1.0).is_some());
        // Caso negativo: lejos de todo, nada.
        assert!(g.cerca([30.0 - o[0], 40.0 - o[1]], 1.0).is_none());
    }

    #[test]
    fn las_medidas_llevan_sus_decimales_y_su_unidad() {
        assert_eq!(medida(12.34567, 6), "12.346 m");
        assert_eq!(medida(123.456, 6), "123.46 m");
        assert_eq!(medida(1234.5, 4), "1234.5 mm");
        assert_eq!(medida(2.0, 0), "2.000");
    }

    #[test]
    fn la_barra_va_encima_si_cabe_y_si_no_debajo() {
        let t = RECT { left: 0, top: 0, right: 1920, bottom: 1040 };
        let p = RECT { left: 100, top: 300, right: 900, bottom: 800 };
        assert_eq!(colocar_barra(p, t, 120, 48, 8), (100, 244));
        let p = RECT { left: 100, top: 20, right: 900, bottom: 800 };
        assert_eq!(colocar_barra(p, t, 120, 48, 8), (100, 808));
        // Caso negativo: sin sitio fuera, dentro y arriba.
        let p = RECT { left: 0, top: 0, right: 1920, bottom: 1040 };
        assert_eq!(colocar_barra(p, t, 120, 48, 8), (8, 8));
    }
}
