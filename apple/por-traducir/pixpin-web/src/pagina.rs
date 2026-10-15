//! **Una ventana de PixPin hecha con una pagina** (8-oct-2026): la del lector
//! y editor de notas Markdown.
//!
//! A diferencia de [`crate::VisorHtml`], que abre un `.html` del disco dentro
//! de una ventana que monta quien llama, aqui todo es de este modulo:
//!
//! - **La ventana** no tiene marco: su cabecera la dibuja la pagina (la del
//!   editor de notas de siempre). Pero es una ventana de Windows de verdad
//!   (`WS_THICKFRAME`): la pagina pide moverla ([`Pagina::arrastrar`]) o
//!   estirarla ([`Pagina::estirar`]) y Windows lo hace a su manera, con el
//!   ajuste a media pantalla y la sombra. El WebView2 la llena entera y se
//!   recoloca en cada `WM_SIZE`.
//! - **La pagina** no esta en el disco: se sirve desde memoria por
//!   `https://pixpin.nota/` (`WebResourceRequested`), y lo demas que pida
//!   bajo esa direccion (las fotos de la nota) lo contesta quien llama con
//!   [`OpcionesPagina::servir`]. Nada sale a internet por aqui.
//! - **Hablar con ella**: [`Pagina::mandar`] le pasa un texto (JSON) y lo
//!   que ella mande con `chrome.webview.postMessage` sale de
//!   [`Pagina::esperar`] como [`Suceso::Mensaje`].
//! - **Cerrar** con la X o Alt+F4 no cierra: llega [`Suceso::PideCerrar`] y
//!   quien llama decide (el lector guarda antes). Cerrar de verdad es soltar
//!   la [`Pagina`].
//!
//! Lo que el navegador haria por su cuenta y aqui estorba se apaga: las
//! herramientas de desarrollo, la barra de estado y las teclas del
//! navegador (F5 recargaria la pagina y se perderia lo escrito). Ctrl+rueda
//! sigue ampliando, y el menu del boton derecho (copiar, pegar, corrector)
//! sigue estando.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;

use pixpin_geom::Rect;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_COLOR, COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC,
    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL, ICoreWebView2, ICoreWebView2Controller,
    ICoreWebView2Controller2, ICoreWebView2Environment, ICoreWebView2Settings3,
};
use webview2_com::{
    CoTaskMemPWSTR, NavigationStartingEventHandler, NewWindowRequestedEventHandler,
    WebMessageReceivedEventHandler, WebResourceRequestedEventHandler, take_pwstr,
};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmExtendFrameIntoClientArea, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::SHCreateMemStream;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW,
    DestroyWindow, DispatchMessageW, GetClientRect,
    GetWindowPlacement, HICON, IDC_ARROW, LoadCursorW, LoadIconW, MINMAXINFO, MSG,
    MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE, PeekMessageW, QS_ALLINPUT,
    RegisterClassExW, SET_WINDOW_POS_FLAGS, SPI_GETWORKAREA, SW_SHOWMAXIMIZED, SW_SHOWNORMAL,
    SWP_NOACTIVATE, SWP_NOZORDER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SetForegroundWindow,
    SetWindowPos, SetWindowTextW, ShowWindow, SystemParametersInfoW,
    TranslateMessage, WINDOWPLACEMENT, WM_CLOSE, WM_DPICHANGED, WM_GETMINMAXINFO, WM_MOVE,
    WM_SETFOCUS, WM_SIZE, WNDCLASSEXW, WS_EX_APPWINDOW,
    GWL_STYLE, GetSystemMetrics, GetWindowLongPtrW, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT,
    HTCAPTION, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, HWND_TOP, IsZoomed,
    NCCALCSIZE_PARAMS, SM_CXPADDEDBORDER, SM_CXSIZEFRAME, SM_CYSIZEFRAME, SW_MAXIMIZE,
    SW_MINIMIZE, SW_RESTORE, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SendMessageW,
    SetWindowPlacement, WM_NCCALCSIZE, WM_NCLBUTTONDOWN, WS_MAXIMIZEBOX,
    WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
};
use windows::core::{Interface, PCWSTR, PWSTR, w};

use crate::ErrorWeb;

/// La direccion de la pagina y de lo que sirve quien llama.
pub const ORIGEN: &str = "https://pixpin.nota/";

/// Lo que se le sirve a la pagina: los bytes y su tipo (`image/png`).
pub struct Recurso {
    pub bytes: Vec<u8>,
    pub tipo: String,
}

/// Como se abre.
pub struct OpcionesPagina {
    pub titulo: String,
    /// Donde, en pixeles de pantalla (el marco entero). Sin ella, centrada y
    /// del 70 % del area de trabajo.
    pub area: Option<Rect>,
    pub maximizada: bool,
    /// El color de fondo mientras la pagina carga (sin el, un fogonazo
    /// blanco en una app oscura).
    pub fondo: (u8, u8, u8),
    /// El html de la pagina, servido como `https://pixpin.nota/index.html`.
    pub pagina: &'static [u8],
    /// Lo demas bajo `https://pixpin.nota/`: recibe lo que va detras
    /// (`archivo?r=…`). `None` es un 404.
    pub servir: Box<dyn Fn(&str) -> Option<Recurso>>,
}

/// Lo que paso desde la ultima vez.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Suceso {
    /// Lo que mando la pagina (`chrome.webview.postMessage`).
    Mensaje(String),
    /// La X, Alt+F4 o cerrar desde la barra de tareas.
    PideCerrar,
    /// La pagina quiso irse a otra direccion (un enlace que no paro ella):
    /// no se va; quien llama decide si abrirla en el navegador.
    Navegar(String),
    /// La ventana ya no existe.
    Destruida,
}

#[derive(Default)]
struct Estado {
    controlador: Option<ICoreWebView2Controller>,
    sucesos: VecDeque<Suceso>,
    /// En pantalla completa: donde estaba antes, para volver.
    antes_de_pantalla: Option<WINDOWPLACEMENT>,
}

/// El estilo de la ventana: sin titulo, con bordes que se estiran.
const ESTILO: u32 = WS_POPUP.0 | WS_THICKFRAME.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0 | WS_SYSMENU.0;

thread_local! {
    /// El estado de cada ventana de este hilo, por su `HWND`: el
    /// procedimiento de ventana es una funcion suelta y lo busca aqui.
    static ESTADOS: RefCell<HashMap<isize, Rc<RefCell<Estado>>>> = RefCell::new(HashMap::new());
}

fn estado_de(hwnd: HWND) -> Option<Rc<RefCell<Estado>>> {
    ESTADOS.with(|e| e.borrow().get(&(hwnd.0 as isize)).cloned())
}

/// Una ventana con su pagina. Soltarla la cierra.
pub struct Pagina {
    hwnd: HWND,
    controlador: ICoreWebView2Controller,
    vista: ICoreWebView2,
    estado: Rc<RefCell<Estado>>,
    temporal: PathBuf,
}

impl Pagina {
    pub fn nueva(o: OpcionesPagina) -> Result<Pagina, ErrorWeb> {
        if !crate::VisorHtml::hay_runtime() {
            return Err(ErrorWeb::SinRuntime);
        }
        let temporal = crate::carpeta_temporal();
        std::fs::create_dir_all(&temporal)
            .map_err(|e| ErrorWeb::Creacion(format!("no se pudo crear {temporal:?}: {e}")))?;

        let estado = Rc::new(RefCell::new(Estado::default()));
        let hwnd = crear_ventana(&o.titulo, o.area)?;
        ESTADOS.with(|e| e.borrow_mut().insert(hwnd.0 as isize, estado.clone()));

        let entorno = crate::crear_entorno(&temporal)?;
        let controlador = crate::crear_controlador(&entorno, hwnd)?;
        // SAFETY: `controlador` acaba de llegar de WebView2 y es valido.
        let vista = unsafe { controlador.CoreWebView2() }
            .map_err(|e| ErrorWeb::Creacion(e.to_string()))?;
        estado.borrow_mut().controlador = Some(controlador.clone());

        if let Ok(c2) = controlador.cast::<ICoreWebView2Controller2>() {
            let (r, g, b) = o.fondo;
            // SAFETY: interfaz recien consultada del controlador vivo.
            let _ = unsafe {
                c2.SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
                    A: 255,
                    R: r,
                    G: g,
                    B: b,
                })
            };
        }
        ajustes(&vista)?;
        servir(&vista, &entorno, o.pagina, o.servir)?;
        escuchar(&vista, estado.clone())?;

        recolocar(hwnd, &controlador);
        // SAFETY: ventana propia de este hilo, recien creada.
        unsafe {
            let _ = ShowWindow(
                hwnd,
                if o.maximizada {
                    SW_SHOWMAXIMIZED
                } else {
                    SW_SHOWNORMAL
                },
            );
            let _ = SetForegroundWindow(hwnd);
        }
        recolocar(hwnd, &controlador);
        // SAFETY: el controlador es valido y ya es visible.
        unsafe {
            let _ = controlador.SetIsVisible(true);
            let _ = controlador.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
        }

        let url = CoTaskMemPWSTR::from(format!("{ORIGEN}index.html").as_str());
        // SAFETY: `url` vive durante la llamada y WebView2 la copia.
        unsafe { vista.Navigate(*url.as_ref().as_pcwstr()) }
            .map_err(|e| ErrorWeb::Navegacion(e.to_string()))?;

        Ok(Pagina {
            hwnd,
            controlador,
            vista,
            estado,
            temporal,
        })
    }

    pub fn hwnd(&self) -> isize {
        self.hwnd.0 as isize
    }

    /// Le pasa `texto` a la pagina (le llega como `event.data`, una cadena).
    pub fn mandar(&self, texto: &str) {
        let t = CoTaskMemPWSTR::from(texto);
        // SAFETY: `t` vive durante la llamada y WebView2 la copia.
        if let Err(e) = unsafe { self.vista.PostWebMessageAsString(*t.as_ref().as_pcwstr()) } {
            tracing::warn!(error = %e, "no se pudo mandar a la pagina");
        }
    }

    pub fn poner_titulo(&self, titulo: &str) {
        let t: Vec<u16> = titulo.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: ventana propia; la cadena termina en cero y vive durante la
        // llamada.
        let _ = unsafe { SetWindowTextW(self.hwnd, PCWSTR(t.as_ptr())) };
    }

    /// Atiende la ventana hasta que pase algo o se cumpla `tope_ms`, y dice
    /// lo que paso. Es el bucle de mensajes de este hilo.
    pub fn esperar(&self, tope_ms: u32) -> Vec<Suceso> {
        // SAFETY: bucle de mensajes estandar de este hilo; `m` es local.
        unsafe {
            let _ = MsgWaitForMultipleObjectsEx(None, tope_ms, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
            let mut m = MSG::default();
            while PeekMessageW(&mut m, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&m);
                DispatchMessageW(&m);
            }
        }
        self.estado.borrow_mut().sucesos.drain(..).collect()
    }

    /// La pagina pide mover la ventana (raton abajo en su cabecera):
    /// Windows la arrastra, con el ajuste a los bordes de la pantalla.
    pub fn arrastrar(&self) {
        // SAFETY: ventana propia; el WebView2 tiene el raton y se le quita
        // para que Windows lo lleve.
        unsafe {
            let _ = ReleaseCapture();
            SendMessageW(self.hwnd, WM_NCLBUTTONDOWN, Some(WPARAM(HTCAPTION as usize)), Some(LPARAM(0)));
        }
    }

    /// Estirar por un borde: `n`, `s`, `e`, `o` o una esquina (`ne`…).
    pub fn estirar(&self, borde: &str) {
        let ht = match borde {
            "n" => HTTOP,
            "s" => HTBOTTOM,
            "e" => HTRIGHT,
            "o" => HTLEFT,
            "ne" => HTTOPRIGHT,
            "no" => HTTOPLEFT,
            "se" => HTBOTTOMRIGHT,
            "so" => HTBOTTOMLEFT,
            _ => return,
        };
        // SAFETY: como en `arrastrar`.
        unsafe {
            let _ = ReleaseCapture();
            SendMessageW(self.hwnd, WM_NCLBUTTONDOWN, Some(WPARAM(ht as usize)), Some(LPARAM(0)));
        }
    }

    pub fn minimizar(&self) {
        // SAFETY: ventana propia.
        let _ = unsafe { ShowWindow(self.hwnd, SW_MINIMIZE) };
    }

    /// Maximiza o devuelve a su tamano.
    pub fn alternar_maximizar(&self) {
        // SAFETY: ventana propia.
        unsafe {
            let max = IsZoomed(self.hwnd).as_bool();
            let _ = ShowWindow(self.hwnd, if max { SW_RESTORE } else { SW_MAXIMIZE });
        }
    }

    /// Pantalla completa (el monitor entero, sin la barra de tareas) o
    /// vuelta a donde estaba. Devuelve si quedo en pantalla completa.
    pub fn alternar_pantalla(&self) -> bool {
        let antes = self.estado.borrow_mut().antes_de_pantalla.take();
        // SAFETY: ventana propia; estructuras locales con su tamano.
        unsafe {
            if let Some(p) = antes {
                let _ = SetWindowPlacement(self.hwnd, &p);
                let _ = SetWindowPos(self.hwnd, None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED);
                return false;
            }
            let mut p = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            };
            let _ = GetWindowPlacement(self.hwnd, &mut p);
            self.estado.borrow_mut().antes_de_pantalla = Some(p);
            let mon = MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let _ = GetMonitorInfoW(mon, &mut info);
            let r = info.rcMonitor;
            if IsZoomed(self.hwnd).as_bool() {
                let _ = ShowWindow(self.hwnd, SW_RESTORE);
            }
            let _ = SetWindowPos(self.hwnd, Some(HWND_TOP), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_FRAMECHANGED);
            true
        }
    }

    /// Donde esta y si esta maximizada, para recordarlo.
    pub fn colocacion(&self) -> Option<(Rect, bool)> {
        let mut p = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        // SAFETY: ventana propia; `p` lleva su tamano puesto.
        unsafe { GetWindowPlacement(self.hwnd, &mut p) }.ok()?;
        let r = p.rcNormalPosition;
        Some((
            Rect {
                x: r.left,
                y: r.top,
                ancho: (r.right - r.left).max(1) as u32,
                alto: (r.bottom - r.top).max(1) as u32,
            },
            p.showCmd == SW_SHOWMAXIMIZED.0 as u32,
        ))
    }
}

impl Drop for Pagina {
    fn drop(&mut self) {
        // SAFETY: el controlador sigue vivo; `Close` apaga el navegador (sin
        // el queda un proceso huerfano). La ventana es de este hilo.
        unsafe {
            let _ = self.controlador.Close();
            let _ = DestroyWindow(self.hwnd);
        }
        ESTADOS.with(|e| e.borrow_mut().remove(&(self.hwnd.0 as isize)));
        let _ = std::fs::remove_dir_all(&self.temporal);
    }
}

// ------------------------------------------------------------------ ventana

fn crear_ventana(titulo: &str, area: Option<Rect>) -> Result<HWND, ErrorWeb> {
    let error = |e: windows::core::Error| ErrorWeb::Creacion(e.to_string());
    // SAFETY: registro de clase y creacion de ventana con cadenas literales o
    // vivas durante cada llamada. Registrar dos veces la misma clase falla
    // sin efecto (ya existe), que es lo que se quiere.
    unsafe {
        let instancia = GetModuleHandleW(None).map_err(error)?;
        // El icono de PixPin (recurso 1 del ejecutable).
        let icono = LoadIconW(Some(instancia.into()), PCWSTR(1 as _)).unwrap_or(HICON::default());
        let clase = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(procedimiento),
            hInstance: instancia.into(),
            hIcon: icono,
            hIconSm: icono,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: w!("PixPinPagina"),
            ..Default::default()
        };
        RegisterClassExW(&clase);

        let r = area.unwrap_or_else(area_por_defecto);
        let titulo: Vec<u16> = titulo.encode_utf16().chain(std::iter::once(0)).collect();
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW,
            w!("PixPinPagina"),
            PCWSTR(titulo.as_ptr()),
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(ESTILO),
            r.x,
            r.y,
            r.ancho as i32,
            r.alto as i32,
            None,
            None,
            Some(instancia.into()),
            None,
        )
        .map_err(error)?;
        // La barra de titulo oscura, como el resto de PixPin.
        let si: i32 = 1;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &si as *const i32 as *const _,
            std::mem::size_of::<i32>() as u32,
        );
        // Un pixel de marco de DWM: la sombra y las esquinas de Windows 11.
        let _ = DwmExtendFrameIntoClientArea(hwnd, &MARGINS { cxLeftWidth: 0, cxRightWidth: 0, cyTopHeight: 1, cyBottomHeight: 0 });
        let _ = SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED);
        Ok(hwnd)
    }
}

/// El 70 % del area de trabajo, centrado.
fn area_por_defecto() -> Rect {
    let mut t = RECT::default();
    // SAFETY: `t` es un RECT local valido a escribir.
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut t as *mut RECT as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .is_ok();
    if !ok || t.right <= t.left {
        return Rect {
            x: 100,
            y: 80,
            ancho: 1180,
            alto: 820,
        };
    }
    let (w, h) = (t.right - t.left, t.bottom - t.top);
    let (aw, ah) = ((w as f32 * 0.7) as i32, (h as f32 * 0.8) as i32);
    Rect {
        x: t.left + (w - aw) / 2,
        y: t.top + (h - ah) / 2,
        ancho: aw.max(600) as u32,
        alto: ah.max(420) as u32,
    }
}

fn recolocar(hwnd: HWND, controlador: &ICoreWebView2Controller) {
    let mut r = RECT::default();
    // SAFETY: ventana propia; `r` es local. El controlador es valido.
    unsafe {
        if GetClientRect(hwnd, &mut r).is_ok() {
            let _ = controlador.SetBounds(r);
        }
    }
}

/// El procedimiento de la ventana: lo justo para que el WebView2 la llene y
/// para que cerrar sea una pregunta.
unsafe extern "system" fn procedimiento(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        // Sin marco: todo es cliente. Maximizada, Windows la saca un borde
        // por cada lado; se le quita para que no se salga de la pantalla.
        WM_NCCALCSIZE if wparam.0 != 0 => {
            // SAFETY: con wparam != 0, lparam es un NCCALCSIZE_PARAMS valido.
            unsafe {
                let p = &mut *(lparam.0 as *mut NCCALCSIZE_PARAMS);
                let estilo = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
                if IsZoomed(hwnd).as_bool() && estilo & WS_THICKFRAME.0 != 0 {
                    let bx = GetSystemMetrics(SM_CXSIZEFRAME) + GetSystemMetrics(SM_CXPADDEDBORDER);
                    let by = GetSystemMetrics(SM_CYSIZEFRAME) + GetSystemMetrics(SM_CXPADDEDBORDER);
                    p.rgrc[0].left += bx;
                    p.rgrc[0].right -= bx;
                    p.rgrc[0].top += by;
                    p.rgrc[0].bottom -= by;
                }
            }
            return LRESULT(0);
        }
        WM_SIZE => {
            if let Some(e) = estado_de(hwnd)
                && let Some(c) = e.borrow().controlador.clone()
            {
                recolocar(hwnd, &c);
            }
            return LRESULT(0);
        }
        WM_MOVE => {
            if let Some(e) = estado_de(hwnd)
                && let Some(c) = e.borrow().controlador.clone()
            {
                // SAFETY: controlador vivo de esta ventana.
                let _ = unsafe { c.NotifyParentWindowPositionChanged() };
            }
        }
        WM_SETFOCUS => {
            if let Some(e) = estado_de(hwnd)
                && let Some(c) = e.borrow().controlador.clone()
            {
                // SAFETY: controlador vivo de esta ventana.
                let _ = unsafe { c.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC) };
            }
            return LRESULT(0);
        }
        WM_CLOSE => {
            match estado_de(hwnd) {
                Some(e) => e.borrow_mut().sucesos.push_back(Suceso::PideCerrar),
                // SAFETY: ventana propia sin estado: nadie la espera.
                None => unsafe {
                    let _ = DestroyWindow(hwnd);
                },
            }
            return LRESULT(0);
        }
        WM_GETMINMAXINFO => {
            // SAFETY: en WM_GETMINMAXINFO `lparam` es un MINMAXINFO valido.
            let mm = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
            mm.ptMinTrackSize.x = 480;
            mm.ptMinTrackSize.y = 320;
            return LRESULT(0);
        }
        WM_DPICHANGED => {
            // SAFETY: en WM_DPICHANGED `lparam` es el RECT sugerido.
            let r = unsafe { &*(lparam.0 as *const RECT) };
            // SAFETY: ventana propia.
            let _ = unsafe {
                SetWindowPos(
                    hwnd,
                    None,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SET_WINDOW_POS_FLAGS(SWP_NOZORDER.0 | SWP_NOACTIVATE.0),
                )
            };
            return LRESULT(0);
        }
        _ => {}
    }
    // SAFETY: el resto, a Windows.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

// ------------------------------------------------------------------ webview

fn ajustes(vista: &ICoreWebView2) -> Result<(), ErrorWeb> {
    let error = |e: windows::core::Error| ErrorWeb::Creacion(e.to_string());
    // SAFETY: `vista` es valida; los ajustes son propiedades simples.
    unsafe {
        let a = vista.Settings().map_err(error)?;
        a.SetIsWebMessageEnabled(true).map_err(error)?;
        let _ = a.SetAreDevToolsEnabled(false);
        let _ = a.SetIsStatusBarEnabled(false);
        let _ = a.SetIsZoomControlEnabled(true);
        let _ = a.SetAreDefaultContextMenusEnabled(true);
        if let Ok(a3) = a.cast::<ICoreWebView2Settings3>() {
            let _ = a3.SetAreBrowserAcceleratorKeysEnabled(false);
        }
    }
    Ok(())
}

/// La pagina y lo que pida bajo [`ORIGEN`], desde memoria.
fn servir(
    vista: &ICoreWebView2,
    entorno: &ICoreWebView2Environment,
    pagina: &'static [u8],
    servir: Box<dyn Fn(&str) -> Option<Recurso>>,
) -> Result<(), ErrorWeb> {
    let filtro = CoTaskMemPWSTR::from(format!("{ORIGEN}*").as_str());
    // SAFETY: `filtro` vive durante la llamada y WebView2 la copia.
    unsafe {
        vista.AddWebResourceRequestedFilter(
            *filtro.as_ref().as_pcwstr(),
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
        )
    }
    .map_err(|e| ErrorWeb::Creacion(e.to_string()))?;
    let entorno = entorno.clone();
    let manejador = WebResourceRequestedEventHandler::create(Box::new(move |_v, args| {
        let Some(args) = args else { return Ok(()) };
        let mut uri = PWSTR::null();
        // SAFETY: `args` lo da WebView2 durante el evento; la cadena se
        // libera con `take_pwstr`.
        unsafe { args.Request()?.Uri(&mut uri)? };
        let uri = take_pwstr(uri);
        let resto = uri.strip_prefix(ORIGEN).unwrap_or("");
        let recurso = if resto == "index.html" || resto.is_empty() {
            Some(Recurso {
                bytes: pagina.to_vec(),
                tipo: "text/html; charset=utf-8".into(),
            })
        } else {
            servir(resto)
        };
        let (codigo, motivo, bytes, cabeceras) = match recurso {
            Some(r) => (
                200,
                "OK",
                r.bytes,
                format!(
                    "Content-Type: {}\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *",
                    r.tipo
                ),
            ),
            None => (404, "Not Found", Vec::new(), String::new()),
        };
        // SAFETY: `SHCreateMemStream` copia los bytes; las cadenas viven
        // durante la llamada y WebView2 las copia.
        unsafe {
            let flujo = SHCreateMemStream(Some(&bytes));
            let motivo = CoTaskMemPWSTR::from(motivo);
            let cabeceras = CoTaskMemPWSTR::from(cabeceras.as_str());
            let respuesta = entorno.CreateWebResourceResponse(
                flujo.as_ref(),
                codigo,
                *motivo.as_ref().as_pcwstr(),
                *cabeceras.as_ref().as_pcwstr(),
            )?;
            args.SetResponse(&respuesta)?;
        }
        Ok(())
    }));
    let mut token = 0i64;
    // SAFETY: `token` es local; el manejador queda referenciado por WebView2.
    unsafe { vista.add_WebResourceRequested(&manejador, &mut token) }
        .map_err(|e| ErrorWeb::Creacion(e.to_string()))?;
    Ok(())
}

/// Los mensajes de la pagina, y que no se vaya a ningun otro sitio.
fn escuchar(vista: &ICoreWebView2, estado: Rc<RefCell<Estado>>) -> Result<(), ErrorWeb> {
    let error = |e: windows::core::Error| ErrorWeb::Creacion(e.to_string());
    let e1 = estado.clone();
    let mensajes = WebMessageReceivedEventHandler::create(Box::new(move |_v, args| {
        if let Some(args) = args {
            let mut m = PWSTR::null();
            // SAFETY: `args` vive durante el evento; la cadena se libera con
            // `take_pwstr`.
            if unsafe { args.TryGetWebMessageAsString(&mut m) }.is_ok() {
                e1.borrow_mut().sucesos.push_back(Suceso::Mensaje(take_pwstr(m)));
            }
        }
        Ok(())
    }));
    let e2 = estado.clone();
    let navegar = NavigationStartingEventHandler::create(Box::new(move |_v, args| {
        let Some(args) = args else { return Ok(()) };
        let mut uri = PWSTR::null();
        // SAFETY: `args` vive durante el evento; la cadena se libera abajo.
        unsafe { args.Uri(&mut uri)? };
        let uri = take_pwstr(uri);
        if !uri.starts_with(ORIGEN) {
            // SAFETY: propiedad del propio evento.
            unsafe { args.SetCancel(true)? };
            e2.borrow_mut().sucesos.push_back(Suceso::Navegar(uri));
        }
        Ok(())
    }));
    let e3 = estado;
    let ventana_nueva = NewWindowRequestedEventHandler::create(Box::new(move |_v, args| {
        let Some(args) = args else { return Ok(()) };
        let mut uri = PWSTR::null();
        // SAFETY: `args` vive durante el evento; la cadena se libera abajo.
        unsafe {
            args.Uri(&mut uri)?;
            args.SetHandled(true)?;
        }
        e3.borrow_mut().sucesos.push_back(Suceso::Navegar(take_pwstr(uri)));
        Ok(())
    }));
    let mut token = 0i64;
    // SAFETY: `token` es local; los manejadores quedan referenciados por
    // WebView2.
    unsafe {
        vista.add_WebMessageReceived(&mensajes, &mut token).map_err(error)?;
        vista.add_NavigationStarting(&navegar, &mut token).map_err(error)?;
        vista.add_NewWindowRequested(&ventana_nueva, &mut token).map_err(error)?;
    }
    Ok(())
}

/// Lo que va detras de `?r=` en una direccion, descodificado (`%20`, `+`).
pub fn parametro(consulta: &str, nombre: &str) -> Option<String> {
    let q = consulta.split_once('?').map_or(consulta, |(_, q)| q);
    q.split('&').find_map(|par| {
        let (k, v) = par.split_once('=')?;
        (k == nombre).then(|| descodificar(v))
    })
}

fn descodificar(s: &str) -> String {
    let b = s.as_bytes();
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => match (hex(b[i + 1]), hex(b[i + 2])) {
                (Some(a), Some(c)) => {
                    out.push(a * 16 + c);
                    i += 3;
                    continue;
                }
                _ => out.push(b'%'),
            },
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_parametro_sale_descodificado() {
        assert_eq!(
            parametro("archivo?r=pixpin%3Afiles%2Fnotas%2Fplano%20a.png", "r").as_deref(),
            Some("pixpin:files/notas/plano a.png")
        );
        assert_eq!(
            parametro("archivo?x=1&r=%C3%B1and%C3%BA.jpg", "r").as_deref(),
            Some("ñandú.jpg")
        );
        // Casos negativos: sin el parametro, o un % que no es un codigo.
        assert_eq!(parametro("archivo?x=1", "r"), None);
        assert_eq!(parametro("archivo?r=50%", "r").as_deref(), Some("50%"));
    }
}
