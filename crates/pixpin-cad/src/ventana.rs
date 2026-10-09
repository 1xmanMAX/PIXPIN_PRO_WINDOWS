//! **La ventana del plano, como un pin**: sin bordes y siempre encima (se
//! puede soltar), se estira por los bordes y se mueve por su barra.
//!
//! - Rueda: acercar y alejar hacia el cursor, suave.
//! - Arrastrar (izquierdo o central): mover el plano.
//! - Doble clic: el plano entero.
//! - Teclas: Esc cierra; F o Inicio, todo; B, fondo claro u oscuro; T,
//!   encima o no; flechas, mover; + y −, acercar y alejar.
//! - Al pasar el raton sale arriba una barra con el nombre y los botones
//!   (fondo, todo, encima, cerrar).
//!
//! No se dibuja nada si nada cambia: en reposo no gasta.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmExtendFrameIntoClientArea, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, ScreenToClient};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

const WM_MOUSELEAVE: u32 = 0x02A3;
const INFINITE: u32 = u32::MAX;

use crate::convertir::rgba;
use crate::gpu::{Gpu, PlanoGpu, Vista};
use crate::modelo::{Constructor, Modelo};
use crate::texto::Textos;

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
}

#[derive(Default)]
struct Estado {
    eventos: VecDeque<Ev>,
    /// El alto de la barra en pixeles (para decir a Windows que ahi se
    /// arrastra la ventana) y donde empiezan sus botones.
    barra: i32,
    botones_desde: i32,
    barra_visible: bool,
}

thread_local! {
    static ESTADO: RefCell<Estado> = RefCell::new(Estado::default());
}

fn apuntar(e: Ev) {
    ESTADO.with(|s| s.borrow_mut().eventos.push_back(e));
}

fn xy(l: LPARAM) -> (i32, i32) {
    ((l.0 & 0xffff) as i16 as i32, ((l.0 >> 16) & 0xffff) as i16 as i32)
}

unsafe extern "system" fn procedimiento(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_NCCALCSIZE if wp.0 != 0 => return LRESULT(0),
        WM_NCHITTEST => {
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
                _ => {
                    let (barra, botones, visible) = ESTADO.with(|s| {
                        let s = s.borrow();
                        (s.barra, s.botones_desde, s.barra_visible)
                    });
                    if visible && p.y < barra && p.x < botones { HTCAPTION } else { HTCLIENT }
                }
            };
            return LRESULT(ht as isize);
        }
        WM_SIZE => {
            let (w, h) = xy(lp);
            apuntar(Ev::Tamano(w.max(1) as u32, h.max(1) as u32));
            return LRESULT(0);
        }
        WM_MOUSEMOVE => {
            let (x, y) = xy(lp);
            apuntar(Ev::Mover(x, y));
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
        WM_MOUSELEAVE | WM_NCMOUSELEAVE => apuntar(Ev::Salir),
        WM_LBUTTONDOWN | WM_MBUTTONDOWN | WM_RBUTTONDOWN => {
            let (x, y) = xy(lp);
            // SAFETY: ventana propia.
            unsafe { SetCapture(hwnd) };
            apuntar(Ev::Bajar(x, y, if msg == WM_LBUTTONDOWN { 0 } else if msg == WM_MBUTTONDOWN { 1 } else { 2 }));
            return LRESULT(0);
        }
        WM_LBUTTONUP | WM_MBUTTONUP | WM_RBUTTONUP => {
            let (x, y) = xy(lp);
            // SAFETY: suelta la captura propia.
            let _ = unsafe { ReleaseCapture() };
            apuntar(Ev::Subir(x, y, if msg == WM_LBUTTONUP { 0 } else if msg == WM_MBUTTONUP { 1 } else { 2 }));
            return LRESULT(0);
        }
        WM_LBUTTONDBLCLK => {
            let (x, y) = xy(lp);
            apuntar(Ev::Doble(x, y));
            return LRESULT(0);
        }
        WM_MOUSEWHEEL => {
            let (sx, sy) = xy(lp);
            let mut p = POINT { x: sx, y: sy };
            // SAFETY: ventana propia; punto local.
            let _ = unsafe { ScreenToClient(hwnd, &mut p) };
            apuntar(Ev::Rueda(p.x, p.y, ((wp.0 >> 16) & 0xffff) as i16 as i32));
            return LRESULT(0);
        }
        WM_KEYDOWN => {
            apuntar(Ev::Tecla(wp.0 as u32));
            return LRESULT(0);
        }
        WM_CLOSE => {
            apuntar(Ev::Cerrar);
            return LRESULT(0);
        }
        WM_ERASEBKGND => return LRESULT(1),
        WM_GETMINMAXINFO => {
            // SAFETY: en este mensaje `lp` es un MINMAXINFO valido.
            let mm = unsafe { &mut *(lp.0 as *mut MINMAXINFO) };
            mm.ptMinTrackSize = POINT { x: 220, y: 160 };
            return LRESULT(0);
        }
        _ => {}
    }
    // SAFETY: lo demas, a Windows.
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

fn crear_ventana(titulo: &str) -> windows::core::Result<(HWND, u32, u32)> {
    // SAFETY: clase y ventana propias; cadenas vivas durante cada llamada.
    unsafe {
        let inst = GetModuleHandleW(None)?;
        let icono = LoadIconW(Some(inst.into()), PCWSTR(1 as _)).unwrap_or_default();
        let clase = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_DBLCLKS,
            lpfnWndProc: Some(procedimiento),
            hInstance: inst.into(),
            hIcon: icono,
            hIconSm: icono,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: w!("PixPinPlanoCad"),
            ..Default::default()
        };
        RegisterClassExW(&clase);
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
            t.top + (ah - h) / 2,
            w,
            h,
            None,
            None,
            Some(inst.into()),
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

const BOTONES: [&str; 4] = ["fondo", "todo", "encima", "cerrar"];

/// La barra de arriba, en pixeles de la ventana (y hacia abajo).
fn barra(textos: &mut Textos, titulo: &str, estado: &str, w: u32, escala: f64, encima_boton: Option<usize>, fijada: bool, claro: bool) -> Modelo {
    let mut c = Constructor::nuevo();
    let alto = 34.0 * escala;
    let w = w as f64;
    let (fondo, tinta, tenue) = if claro {
        (0xe6f2f0ea, rgba(0x1f, 0x1e, 0x1d), rgba(0x6b, 0x6a, 0x66))
    } else {
        (0xe61f1e1d, rgba(0xe6, 0xe4, 0xdf), rgba(0x9a, 0x98, 0x93))
    };
    let rect = |c: &mut Constructor, x0: f64, y0: f64, x1: f64, y1: f64, color: u32| {
        c.triangulos(&[[x0, y0], [x1, y0], [x0, y1], [x1, y0], [x1, y1], [x0, y1]], color, Some(1e6));
    };
    rect(&mut c, 0.0, 0.0, w, alto, fondo);
    let lado = 28.0 * escala;
    let hueco = 4.0 * escala;
    let x_botones = w - 8.0 * escala - (lado + hueco) * BOTONES.len() as f64;
    for (i, b) in BOTONES.iter().enumerate() {
        let x = x_botones + i as f64 * (lado + hueco);
        let y = (alto - lado) / 2.0;
        if encima_boton == Some(i) {
            let color = if *b == "cerrar" { rgba(0xc4, 0x2b, 0x1c) } else if claro { rgba(0xe4, 0xe2, 0xda) } else { rgba(0x31, 0x31, 0x30) };
            rect(&mut c, x, y, x + lado, y + lado, color);
        }
        let (cx, cy, r) = (x + lado / 2.0, y + lado / 2.0, 6.5 * escala);
        let col = if *b == "encima" && fijada { rgba(0x5b, 0x9b, 0xf0) } else { tinta };
        match *b {
            "fondo" => {
                let circ: Vec<[f64; 2]> = (0..=32).map(|k| {
                    let a = k as f64 / 32.0 * std::f64::consts::TAU;
                    [cx + r * a.cos(), cy + r * a.sin()]
                }).collect();
                c.polilinea(&circ, col, Some(1e6));
                let medio: Vec<[f64; 2]> = (0..=16).map(|k| {
                    let a = -std::f64::consts::FRAC_PI_2 + k as f64 / 16.0 * std::f64::consts::PI;
                    [cx + r * a.cos(), cy + r * a.sin()]
                }).collect();
                let mut tri = Vec::new();
                for v in medio.windows(2) {
                    tri.extend_from_slice(&[[cx, cy], v[0], v[1]]);
                }
                c.triangulos(&tri, col, Some(1e6));
            }
            "todo" => {
                let k = r * 0.45;
                for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                    let (px, py) = (cx + sx * r, cy + sy * r);
                    c.polilinea(&[[px - sx * k, py], [px, py], [px, py - sy * k]], col, Some(1e6));
                }
            }
            "encima" => {
                // Una chincheta.
                c.polilinea(&[[cx - r * 0.55, cy - r], [cx + r * 0.55, cy - r]], col, Some(1e6));
                c.polilinea(&[[cx - r * 0.35, cy - r], [cx - r * 0.45, cy + r * 0.1], [cx + r * 0.45, cy + r * 0.1], [cx + r * 0.35, cy - r]], col, Some(1e6));
                c.polilinea(&[[cx - r * 0.8, cy + r * 0.1], [cx + r * 0.8, cy + r * 0.1]], col, Some(1e6));
                c.polilinea(&[[cx, cy + r * 0.1], [cx, cy + r]], col, Some(1e6));
            }
            _ => {
                let k = r * 0.75;
                c.polilinea(&[[cx - k, cy - k], [cx + k, cy + k]], col, Some(1e6));
                c.polilinea(&[[cx - k, cy + k], [cx + k, cy - k]], col, Some(1e6));
            }
        }
    }
    let tam = 13.0 * escala;
    let base = alto / 2.0 + tam * 0.36;
    let ancho = textos.en_pantalla(&mut c, titulo, 12.0 * escala, base, tam, tinta, x_botones - 24.0 * escala);
    if !estado.is_empty() {
        textos.en_pantalla(&mut c, estado, 12.0 * escala + ancho + 14.0 * escala, base, tam * 0.92, tenue, x_botones - 12.0 * escala);
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

/// **Abre la ventana del plano** y no vuelve hasta que se cierra. `cargar`
/// llega por el canal cuando el plano esta listo (o el porque no).
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
    let escala = dpi as f64 / 96.0;
    ESTADO.with(|s| {
        let mut s = s.borrow_mut();
        *s = Estado::default();
        s.barra = (34.0 * escala) as i32;
    });
    let mut textos = Textos::nuevo();
    let mut plano: Option<(Modelo, PlanoGpu)> = None;
    let mut mensaje = textos_ui.abriendo.clone();
    let mut camara = Camara { centro: [0.0, 0.0], px: 1.0 };
    let mut destino = camara;
    let mut claro = false;
    let mut fijada = true;
    let mut raton = (0, 0);
    let mut dentro = false;
    let mut arrastre: Option<(i32, i32, Camara)> = None;
    let mut boton_encima: Option<usize> = None;
    let mut ultimo_mov = Instant::now();
    let mut sucio = true;
    let mut barra_ui: Option<(Modelo, PlanoGpu)> = None;
    let mut barra_sucia = true;
    let mut aviso_ui: Option<(Modelo, PlanoGpu)> = None;
    let mut detalle = String::new();
    'bucle: loop {
        // Mensajes: si hay animacion o carga, sin esperar; si no, a dormir.
        let animando = (camara.px - destino.px).abs() > destino.px * 1e-4
            || (camara.centro[0] - destino.centro[0]).abs() > destino.px * 0.05
            || (camara.centro[1] - destino.centro[1]).abs() > destino.px * 0.05;
        // SAFETY: bucle de mensajes de este hilo.
        unsafe {
            if !animando && !sucio && !barra_sucia {
                let espera = if plano.is_none() { 50 } else if dentro { 400 } else { INFINITE };
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
                                if !m.sin_dibujar.is_empty() {
                                    let n: u32 = m.sin_dibujar.iter().map(|(_, n)| n).sum();
                                    detalle = textos_ui.sin_dibujar.replace("{n}", &n.to_string());
                                }
                                plano = Some((m, p));
                                mensaje.clear();
                            }
                            Err(e) => mensaje = format!("{}: {e}", textos_ui.error),
                        }
                    }
                    aviso_ui = None;
                    sucio = true;
                    barra_sucia = true;
                }
                Ok(Err(e)) => {
                    mensaje = format!("{}: {e}", textos_ui.error);
                    aviso_ui = None;
                    sucio = true;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    if mensaje == textos_ui.abriendo {
                        mensaje = textos_ui.error.clone();
                        aviso_ui = None;
                        sucio = true;
                    }
                }
            }
        }
        let eventos: Vec<Ev> = ESTADO.with(|s| s.borrow_mut().eventos.drain(..).collect());
        let lado = 28.0 * escala;
        let hueco = 4.0 * escala;
        let x_botones = w as f64 - 8.0 * escala - (lado + hueco) * BOTONES.len() as f64;
        let alto_barra = 34.0 * escala;
        let boton_en = |x: i32, y: i32| -> Option<usize> {
            if (y as f64) >= alto_barra || (x as f64) < x_botones {
                return None;
            }
            let i = ((x as f64 - x_botones) / (lado + hueco)) as usize;
            (i < BOTONES.len()).then_some(i)
        };
        for e in eventos {
            match e {
                Ev::Cerrar => break 'bucle,
                Ev::Tamano(nw, nh) => {
                    if (nw, nh) != (w, h) {
                        // Que el centro siga en su sitio y la escala igual.
                        w = nw;
                        h = nh;
                        let _ = gpu.redimensionar(w, h);
                        barra_sucia = true;
                        aviso_ui = None;
                        sucio = true;
                    }
                }
                Ev::Mover(x, y) => {
                    raton = (x, y);
                    if !dentro {
                        dentro = true;
                        barra_sucia = true;
                    }
                    ultimo_mov = Instant::now();
                    if let Some((x0, y0, c0)) = arrastre {
                        let c = Camara {
                            centro: [c0.centro[0] - (x - x0) as f64 * c0.px, c0.centro[1] + (y - y0) as f64 * c0.px],
                            px: c0.px,
                        };
                        camara = c;
                        destino = c;
                        sucio = true;
                    }
                    let b = boton_en(x, y);
                    if b != boton_encima {
                        boton_encima = b;
                        barra_sucia = true;
                    }
                }
                Ev::Salir => {
                    dentro = false;
                    boton_encima = None;
                    barra_sucia = true;
                }
                Ev::Bajar(x, y, b) => {
                    if b == 0 && let Some(i) = boton_en(x, y) {
                        match BOTONES[i] {
                            "fondo" => {
                                claro = !claro;
                                barra_sucia = true;
                                aviso_ui = None;
                                sucio = true;
                            }
                            "todo" => {
                                if let Some((m, _)) = &plano {
                                    destino = Camara::encuadrar(m.caja, w, h);
                                }
                            }
                            "encima" => {
                                fijada = !fijada;
                                poner_encima(hwnd, fijada);
                                barra_sucia = true;
                            }
                            _ => break 'bucle,
                        }
                        continue;
                    }
                    if b <= 1 {
                        arrastre = Some((x, y, camara));
                    }
                }
                Ev::Subir(..) => arrastre = None,
                Ev::Doble(x, y) => {
                    if boton_en(x, y).is_none()
                        && let Some((m, _)) = &plano
                    {
                        destino = Camara::encuadrar(m.caja, w, h);
                    }
                }
                Ev::Rueda(x, y, d) => {
                    let factor = 1.25f64.powf(-(d as f64) / 120.0);
                    destino = destino.zoom(factor, x as f64, y as f64, w, h);
                }
                Ev::Tecla(vk) => match vk {
                    0x1B => break 'bucle,
                    0x46 | 0x24 => {
                        if let Some((m, _)) = &plano {
                            destino = Camara::encuadrar(m.caja, w, h);
                        }
                    }
                    0x42 => {
                        claro = !claro;
                        barra_sucia = true;
                        aviso_ui = None;
                        sucio = true;
                    }
                    0x54 => {
                        fijada = !fijada;
                        poner_encima(hwnd, fijada);
                        barra_sucia = true;
                    }
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
        // La barra se esconde a los 2 s sin mover el raton.
        let barra_visible = plano.is_none() || (dentro && (ultimo_mov.elapsed() < Duration::from_secs(2) || boton_encima.is_some())) || arrastre.is_none() && raton.1 < (alto_barra as i32) && dentro;
        let antes = ESTADO.with(|s| s.borrow().barra_visible);
        if antes != barra_visible {
            ESTADO.with(|s| {
                let mut s = s.borrow_mut();
                s.barra_visible = barra_visible;
                s.botones_desde = x_botones as i32;
            });
            sucio = true;
        } else {
            ESTADO.with(|s| s.borrow_mut().botones_desde = x_botones as i32);
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
        if barra_sucia {
            let estado = if plano.is_some() { detalle.as_str() } else { "" };
            let m = barra(&mut textos, titulo, estado, w, escala, boton_encima, fijada, claro);
            barra_ui = gpu.subir(&m).ok().map(|p| (m, p));
            barra_sucia = false;
            sucio = true;
        }
        if aviso_ui.is_none() && !mensaje.is_empty() {
            let m = aviso(&mut textos, &mensaje, w, h, escala, claro);
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
            if barra_visible && let Some((m, p)) = &barra_ui {
                capas.push((p, vista_pantalla(m, w, h)));
            }
            if let Err(e) = gpu.dibujar(fondo, &capas) {
                tracing::warn!(error = %e, "no se pudo dibujar el plano");
            }
            sucio = false;
        }
    }
    // SAFETY: ventana propia.
    let _ = unsafe { DestroyWindow(hwnd) };
    Ok(())
}

fn poner_encima(hwnd: HWND, si: bool) {
    // SAFETY: ventana propia.
    let _ = unsafe {
        SetWindowPos(hwnd, Some(if si { HWND_TOPMOST } else { HWND_NOTOPMOST }), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE)
    };
}

/// Los textos de la ventana, en el idioma de la app.
#[derive(Debug, Clone)]
pub struct TextosUi {
    pub abriendo: String,
    pub vacio: String,
    pub error: String,
    /// Con `{n}`: cuantas cosas no se pudieron dibujar.
    pub sin_dibujar: String,
}

impl Default for TextosUi {
    fn default() -> Self {
        Self {
            abriendo: "Abriendo el plano…".into(),
            vacio: "El plano no tiene nada que dibujar".into(),
            error: "No se pudo abrir el plano".into(),
            sin_dibujar: "{n} objetos 3D sin dibujar".into(),
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
    }

    #[test]
    fn encuadrar_mete_el_plano_entero_con_margen() {
        let c = Camara::encuadrar([-100.0, -10.0, 100.0, 10.0], 1000, 500);
        assert_eq!(c.centro, [0.0, 0.0]);
        // 200 de ancho en 920 pixeles.
        assert!((c.px - 200.0 / 920.0).abs() < 1e-9);
        // Caso negativo: una caja vacia no da una escala absurda.
        assert!(Camara::encuadrar([0.0; 4], 100, 100).px > 0.0);
    }
}
