//! pixpin-web — ver y editar un `.html` dentro de PixPin, con WebView2.
//!
//! Es el puerto de `ui/VisorHtmlActivity.kt` de PixPin Android. El chat genera
//! ficheros HTML que se editan solos (una tabla, una ficha, una lista) y que
//! traen su propio boton de guardar. Hasta ahora en el escritorio esos
//! ficheros se abrian en el navegador del sistema, y guardar significaba
//! descargar una copia a `Descargas` que el usuario tenia que mover a mano.
//! Aqui la pagina se abre dentro y el guardado cae **encima del mismo
//! fichero**.
//!
//! Se capturan los dos guardados posibles, porque las paginas vienen de dos
//! epocas distintas:
//!
//! 1. Las que conocen el puente llaman a `window.PixPinVisor.guardar(nombre,
//!    base64)`. Es el camino limpio: llega el HTML entero en un mensaje.
//! 2. Las que no lo conocen hacen lo de siempre, un `<a download>` o un
//!    `Blob`. Ese camino se intercepta en `DownloadStarting`: la descarga se
//!    desvia a un fichero temporal nuestro y el aviso del navegador se cancela.
//!
//! Los dos pasan por el mismo filtro de [`validacion`] antes de tocar nada.
//!
//! Este crate habla con COM y con el sistema operativo, asi que hay `unsafe`;
//! cada bloque lleva su `// SAFETY:`. No depende de ningun otro crate de
//! PixPin salvo `pixpin-geom`: es capa L1, un cimiento como `pixpin-audio`.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod base64;
pub mod pagina;
pub mod puente;
pub mod ruta;
pub mod validacion;

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use pixpin_geom::Rect;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED, COREWEBVIEW2_PREFERRED_COLOR_SCHEME_DARK,
    COREWEBVIEW2_PREFERRED_COLOR_SCHEME_LIGHT, CreateCoreWebView2EnvironmentWithOptions,
    GetAvailableCoreWebView2BrowserVersionString, ICoreWebView2, ICoreWebView2_4, ICoreWebView2_13,
    ICoreWebView2Controller, ICoreWebView2Environment, ICoreWebView2EnvironmentOptions,
};
use webview2_com::{
    AddScriptToExecuteOnDocumentCreatedCompletedHandler, CoTaskMemPWSTR,
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    DownloadStartingEventHandler, ExecuteScriptCompletedHandler, StateChangedEventHandler,
    WebMessageReceivedEventHandler, take_pwstr,
};
use windows::Win32::Foundation::{E_POINTER, HWND, RECT};
use windows::core::{Interface, PCWSTR, PWSTR};

/// Lo que puede salir mal al montar el visor.
#[derive(Debug, thiserror::Error)]
pub enum ErrorWeb {
    /// No esta el runtime de WebView2 en el equipo. Quien llama debe abrir el
    /// html en el navegador del sistema, como se hacia antes.
    #[error("no esta instalado el runtime de WebView2")]
    SinRuntime,
    /// Fallo creando el entorno o el controlador de WebView2.
    #[error("no se pudo crear el WebView2: {0}")]
    Creacion(String),
    /// El WebView2 existe pero no pudo abrir el fichero.
    #[error("no se pudo abrir el html: {0}")]
    Navegacion(String),
}

/// La funcion que recibe el HTML nuevo, compartida por los dos caminos de
/// guardado. Es `Rc` y no `Arc` a proposito: WebView2 llama siempre desde el
/// hilo de interfaz, asi que un contador atomico solo seria coste.
type AlGuardar = Rc<dyn Fn(Vec<u8>)>;

/// Un WebView2 empotrado en una ventana de PixPin, mostrando un `.html`.
pub struct VisorHtml {
    controlador: ICoreWebView2Controller,
    /// Si Ctrl+P es del visor (los marcos numerados) y no de la pagina.
    ctrl_p: Rc<std::cell::Cell<bool>>,
    /// Lo que la pagina mando por `chrome.webview.postMessage` y no es un
    /// guardado (el riel de los visores: el desplazamiento al momento y las
    /// teclas pulsadas dentro de la pagina). Ver [`VisorHtml::tomar_mensajes`].
    otros: Rc<std::cell::RefCell<Vec<String>>>,
    /// La carpeta propia donde aterrizan las descargas interceptadas y los
    /// datos de usuario del navegador. Se borra al cerrar.
    temporal: PathBuf,
}

impl VisorHtml {
    /// Crea el WebView2 dentro de `padre`, ocupando `area` (pixeles cliente),
    /// y abre `ruta`. `al_guardar` recibe los BYTES del html nuevo; lo llama
    /// desde el hilo de interfaz.
    /// `al_tecla` recibe Escape y F11 pulsados DENTRO de la pagina (su
    /// codigo de tecla virtual). Hace falta porque el WebView2 se queda con
    /// el teclado: sin esto no llegan a la ventana que lo contiene, y el
    /// visor no se podria cerrar ni pasar a pantalla completa mientras se
    /// escribe en la pagina.
    pub fn nuevo(
        padre: HWND,
        area: Rect,
        ruta: &Path,
        al_guardar: Box<dyn Fn(Vec<u8>) + 'static>,
        al_tecla: Box<dyn Fn(u32) + 'static>,
    ) -> Result<VisorHtml, ErrorWeb> {
        if !Self::hay_runtime() {
            return Err(ErrorWeb::SinRuntime);
        }
        let al_guardar: AlGuardar = Rc::from(al_guardar);

        let temporal = carpeta_temporal();
        std::fs::create_dir_all(&temporal)
            .map_err(|e| ErrorWeb::Creacion(format!("no se pudo crear {temporal:?}: {e}")))?;

        let t0 = std::time::Instant::now();
        let (_entorno, controlador) = entorno_y_controlador(padre)?;

        // SAFETY: `controlador` acaba de llegar de WebView2 y es valido; el
        // RECT se construye aqui y vive mas que la llamada.
        let vista = unsafe {
            controlador
                .SetBounds(a_rect(area))
                .and_then(|()| controlador.SetIsVisible(true))
                .and_then(|()| controlador.CoreWebView2())
        }
        .map_err(|e| ErrorWeb::Creacion(e.to_string()))?;

        medir_carga(&vista, "html", t0);
        let otros = Rc::new(std::cell::RefCell::new(Vec::new()));
        conectar_puente(&vista, al_guardar.clone(), Rc::clone(&otros))?;
        conectar_descargas(&vista, al_guardar, temporal.clone())?;
        let ctrl_p = Rc::new(std::cell::Cell::new(false));
        conectar_teclas(&controlador, Rc::from(al_tecla), Rc::clone(&ctrl_p))?;

        let url = CoTaskMemPWSTR::from(ruta::url_de_fichero(ruta).as_str());
        // SAFETY: `url` mantiene viva la cadena durante toda la llamada, que
        // es lo unico que `Navigate` necesita: WebView2 la copia.
        unsafe { vista.Navigate(*url.as_ref().as_pcwstr()) }
            .map_err(|e| ErrorWeb::Navegacion(e.to_string()))?;

        Ok(VisorHtml {
            controlador,
            ctrl_p,
            otros,
            temporal,
        })
    }

    /// **Modo oscuro** de la pagina (la luna/sol de los visores de PixPin):
    /// se le dice al WebView2 el esquema de color que se prefiere, el
    /// `prefers-color-scheme` de CSS. Las paginas con estilo oscuro (las de
    /// PixPin lo traen) lo usan al momento; las que no lo tienen se ven
    /// igual, sin invertir nada (una foto o un grafico no salen en negativo).
    /// Con un runtime de WebView2 viejo, sin `ICoreWebView2_13`, no hace nada.
    pub fn poner_oscuro(&self, oscuro: bool) {
        poner_esquema(&self.controlador, oscuro);
    }

    /// **Vuelve a cargar** la pagina desde el disco (el boton de recargar
    /// de su barra): lo que otro programa haya cambiado en el fichero sale.
    pub fn recargar(&self) {
        // SAFETY: `self.controlador` vive mientras `self`; la vista sale de
        // el y solo se usa en esta llamada.
        if let Err(e) = unsafe { self.controlador.CoreWebView2().and_then(|v| v.Reload()) } {
            tracing::warn!(error = %e, "no se pudo recargar la pagina");
        }
    }

    /// Le da el teclado a la pagina (la ventana que la lleva lo tenia).
    pub fn enfocar(&self) {
        enfocar(&self.controlador);
    }

    /// Recoloca el WebView2 dentro de la ventana padre.
    pub fn redimensionar(&self, area: Rect) {
        // SAFETY: `self.controlador` es valido mientras viva `self`, y se
        // libera en `Drop`.
        if let Err(e) = unsafe { self.controlador.SetBounds(a_rect(area)) } {
            // No se propaga: redimensionar se llama desde `WM_SIZE`, donde no
            // hay a quien devolverle el error, y un visor mal colocado es
            // mucho menos grave que tirar la ventana entera.
            tracing::warn!(error = %e, "no se pudo recolocar el WebView2");
        }
    }

    /// **El zoom de la pagina** (el de Ctrl+rueda, `ZoomFactor` del
    /// controlador): 1 es el 100 %.
    pub fn zoom(&self) -> f64 {
        zoom_de(&self.controlador)
    }

    /// Pone el zoom de la pagina (un marcador que vuelve a su vista).
    pub fn poner_zoom(&self, zoom: f64) {
        poner_zoom_en(&self.controlador, zoom);
    }

    /// Si Ctrl+P dentro de la pagina es del visor (llega a `al_tecla` como
    /// [`CTRL_P`]) en vez de la impresion de Edge: con marcos numerados.
    pub fn tomar_ctrl_p(&self, si: bool) {
        self.ctrl_p.set(si);
    }

    /// **Lo que la pagina mando** (`chrome.webview.postMessage`) desde la
    /// ultima vez, sin los guardados (esos van a `al_guardar`). Llega en este
    /// mismo hilo al bombear sus mensajes; quien lo lleva lo recoge en cada
    /// vuelta de su bucle.
    pub fn tomar_mensajes(&self) -> Vec<String> {
        std::mem::take(&mut *self.otros.borrow_mut())
    }

    /// **Corre `js` en la pagina** y le da a `al_volver` lo que devuelva,
    /// en JSON (`null` si no devuelve nada o fallo). Llega en este mismo
    /// hilo, al bombear sus mensajes. Es lo que usa el riel de los visores
    /// (19-oct-2026) para saber por donde va la pagina y llevarla a una
    /// marca.
    pub fn ejecutar(&self, js: &str, al_volver: Box<dyn FnOnce(String) + 'static>) {
        // SAFETY: `self.controlador` vive mientras `self`; la vista sale de
        // el y solo se usa en esta llamada.
        match unsafe { self.controlador.CoreWebView2() } {
            Ok(v) => ejecutar_en(&v, js, al_volver),
            Err(e) => tracing::warn!(error = %e, "sin vista para correr el guion"),
        }
    }

    /// Si el runtime de WebView2 esta instalado. Sin el, quien llama abre el
    /// html en el navegador como hasta ahora.
    pub fn hay_runtime() -> bool {
        let mut version = PWSTR::null();
        // SAFETY: se pasa una carpeta nula para que busque la instalacion del
        // sistema, y `version` es un puntero valido a escribir. Si devuelve
        // error no escribe nada, y si acierta la cadena es de `CoTaskMemAlloc`
        // y se libera justo debajo con `take_pwstr`.
        let encontrado =
            unsafe { GetAvailableCoreWebView2BrowserVersionString(PCWSTR::null(), &mut version) }
                .is_ok();
        if !version.is_null() {
            let _ = take_pwstr(version);
        }
        encontrado
    }
}

impl Drop for VisorHtml {
    fn drop(&mut self) {
        // SAFETY: `controlador` sigue siendo valido aqui; `Close` es lo que
        // apaga el proceso del navegador. Sin el, el WebView2 sobrevive a la
        // ventana y se queda un proceso huerfano por cada html abierto.
        if let Err(e) = unsafe { self.controlador.Close() } {
            tracing::warn!(error = %e, "el WebView2 no cerro limpiamente");
        }
        controlador_cerrado();
        // Las descargas interceptadas ya se entregaron en memoria: lo que
        // quede aqui es basura y no debe sobrevivir a la sesion.
        let _ = std::fs::remove_dir_all(&self.temporal);
    }
}

/// Lo que le llega a `al_tecla` de [`VisorHtml::nuevo`] con Ctrl+P, si el
/// visor lo pidio con [`VisorHtml::tomar_ctrl_p`].
pub const CTRL_P: u32 = 0x1_0050;

fn ctrl_pulsado() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL};
    // SAFETY: solo lee el estado de una tecla.
    unsafe { GetKeyState(VK_CONTROL.0 as i32) < 0 }
}

/// El `ZoomFactor` de un controlador (1 si no contesta).
pub(crate) fn zoom_de(controlador: &ICoreWebView2Controller) -> f64 {
    let mut z = 1.0f64;
    // SAFETY: controlador vivo de quien llama; `z` es local.
    match unsafe { controlador.ZoomFactor(&mut z) } {
        Ok(()) if z.is_finite() && z > 0.0 => z,
        _ => 1.0,
    }
}

/// Pone el `ZoomFactor` (acotado a lo que deja Edge: del 25 % al 500 %).
pub(crate) fn poner_zoom_en(controlador: &ICoreWebView2Controller, zoom: f64) {
    if !zoom.is_finite() {
        return;
    }
    // SAFETY: controlador vivo de quien llama.
    if let Err(e) = unsafe { controlador.SetZoomFactor(zoom.clamp(0.25, 5.0)) } {
        tracing::warn!(error = %e, "no se pudo poner el zoom de la pagina");
    }
}

/// **El esquema de color** que se le pide a la pagina, el
/// `prefers-color-scheme` de CSS: lo usan el visor de HTML y la pagina del
/// lector de notas para seguir la luna/sol de los visores de PixPin. Las
/// paginas que no tienen estilo oscuro se ven igual, sin invertir nada. Con
/// un runtime de WebView2 viejo, sin `ICoreWebView2_13`, no hace nada.
pub(crate) fn poner_esquema(controlador: &ICoreWebView2Controller, oscuro: bool) {
    let esquema = if oscuro {
        COREWEBVIEW2_PREFERRED_COLOR_SCHEME_DARK
    } else {
        COREWEBVIEW2_PREFERRED_COLOR_SCHEME_LIGHT
    };
    // SAFETY: `controlador` lo da quien llama, vivo; las interfaces que se
    // piden salen de el y solo se usan en esta llamada.
    let hecho = unsafe {
        controlador
            .CoreWebView2()
            .and_then(|v| v.cast::<ICoreWebView2_13>())
            .and_then(|v| v.Profile())
            .and_then(|p| p.SetPreferredColorScheme(esquema))
    };
    if let Err(e) = hecho {
        tracing::warn!(error = %e, "el WebView2 no deja elegir el esquema de color");
    }
}

/// Corre `js` en `vista` y le pasa el resultado (JSON) a `al_volver`; ver
/// [`VisorHtml::ejecutar`].
pub(crate) fn ejecutar_en(
    vista: &ICoreWebView2,
    js: &str,
    al_volver: Box<dyn FnOnce(String) + 'static>,
) {
    let guion = CoTaskMemPWSTR::from(js);
    let manejador = ExecuteScriptCompletedHandler::create(Box::new(move |hecho, resultado| {
        al_volver(if hecho.is_ok() {
            resultado
        } else {
            "null".into()
        });
        Ok(())
    }));
    // SAFETY: `guion` vive durante la llamada y WebView2 lo copia; el
    // manejador es un objeto COM propio que WebView2 retiene hasta llamarlo.
    if let Err(e) = unsafe { vista.ExecuteScript(*guion.as_ref().as_pcwstr(), &manejador) } {
        tracing::warn!(error = %e, "no se pudo correr el guion en la pagina");
    }
}

/// El teclado, a la pagina.
pub(crate) fn enfocar(controlador: &ICoreWebView2Controller) {
    use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC;
    // SAFETY: controlador vivo de quien llama.
    let _ = unsafe { controlador.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC) };
}

/// **Pinta el fondo** de la zona de cliente de `hwnd` de un color liso: lo
/// que se ve en la ventana de un visor mientras el WebView2 arranca (sin
/// el, la ventana nueva saldria vacia o con restos de lo de debajo).
pub fn pintar_fondo(hwnd: isize, (r, g, b): (u8, u8, u8)) {
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::Graphics::Gdi::{
        CreateSolidBrush, DCX_CACHE, DCX_CLIPCHILDREN, DeleteObject, FillRect, GetDCEx, HGDIOBJ,
        ReleaseDC,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
    let hwnd = HWND(hwnd as *mut _);
    // SAFETY: GetDC/ReleaseDC emparejados sobre la ventana de quien llama;
    // la brocha se crea y se borra aqui. Un handle muerto solo da error.
    unsafe {
        let mut rc = RECT::default();
        if GetClientRect(hwnd, &mut rc).is_err() {
            return;
        }
        // Sin pisar la pagina, que es una ventana hija: solo lo de alrededor
        // (el borde de estirar) y, mientras no la hay, todo.
        let dc = GetDCEx(Some(hwnd), None, DCX_CACHE | DCX_CLIPCHILDREN);
        if dc.is_invalid() {
            return;
        }
        let brocha = CreateSolidBrush(COLORREF(r as u32 | (g as u32) << 8 | (b as u32) << 16));
        FillRect(dc, &rc, brocha);
        let _ = DeleteObject(HGDIOBJ(brocha.0));
        ReleaseDC(Some(hwnd), dc);
    }
}

/// **Cuanto tarda una ventana en verse**, para las pruebas que miden la
/// latencia de los visores: espera hasta `tope` a que haya una ventana
/// visible con ese titulo exacto y dice cuanto tardo desde que se llamo.
pub fn esperar_visible(titulo: &str, tope: std::time::Duration) -> Option<std::time::Duration> {
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, IsWindowVisible};
    let t: Vec<u16> = titulo.encode_utf16().chain(std::iter::once(0)).collect();
    let desde = std::time::Instant::now();
    while desde.elapsed() < tope {
        // SAFETY: cadena terminada en cero que vive durante la llamada; solo
        // se consulta.
        if let Ok(h) = unsafe { FindWindowW(PCWSTR::null(), PCWSTR(t.as_ptr())) }
            && !h.is_invalid()
            // SAFETY: consulta de solo lectura sobre un handle cualquiera.
            && unsafe { IsWindowVisible(h) }.as_bool()
        {
            return Some(desde.elapsed());
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    None
}

/// Carpeta propia bajo el temporal del sistema, distinta por visor.
pub(crate) fn carpeta_temporal() -> PathBuf {
    static CONTADOR: AtomicU64 = AtomicU64::new(0);
    let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("pixpin-web-{}-{n}", std::process::id()))
}

/// `Rect` de PixPin a `RECT` de Windows.
///
/// Los dos usan media apertura, asi que la derecha y el abajo de `Rect` ya son
/// el primer pixel de fuera, que es justo lo que espera `SetBounds`.
pub(crate) fn a_rect(area: Rect) -> RECT {
    RECT {
        left: area.izquierda(),
        top: area.arriba(),
        right: area.derecha(),
        bottom: area.abajo(),
    }
}

/// **La carpeta de datos de WebView2, una y para siempre** (18-oct-2026).
/// Antes cada visor estrenaba la suya (`pixpin-web-<pid>-<n>\datos`): cada
/// apertura era un navegador arrancando en frio, sin cache, y se notaba lento
/// (el usuario: «se sienten lentos ambos visores»). Con una sola, los visores
/// comparten el proceso del navegador que ya este vivo y su cache.
/// `PIXPIN_WEB_DATOS` la cambia: las pruebas a mano con la app abierta (otro
/// proceso no puede usar la misma carpeta, 0x8007139F).
pub fn carpeta_de_datos() -> PathBuf {
    if let Some(d) = std::env::var_os("PIXPIN_WEB_DATOS") {
        return PathBuf::from(d);
    }
    std::env::temp_dir().join("pixpin-web-datos")
}

thread_local! {
    /// El entorno de WebView2 de este hilo, hecho una vez.
    static ENTORNO: std::cell::RefCell<Option<ICoreWebView2Environment>> = const { std::cell::RefCell::new(None) };
    /// Los controladores vivos de este hilo.
    static VIVOS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// **Un controlador de este hilo se cerro** (19-oct-2026): con el ultimo, el
/// entorno se suelta YA. Si se quedaba en el `thread_local`, se soltaba al
/// morir el hilo, despues de su `CoUninitialize`, y el WebView2 tiraba la
/// app entera (0xC000041D en `EmbeddedBrowserWebView.dll`) al cerrar un
/// pin de html o de notas. `examples/cerrar_hilos.rs` lo reproduce.
pub(crate) fn controlador_cerrado() {
    // `try_with`: un controlador que se suelta con el hilo ya muriendo no
    // debe tirar la app entera (un `with` ahi es un panico fatal).
    let Ok(quedan) = VIVOS.try_with(|v| {
        let n = v.get().saturating_sub(1);
        v.set(n);
        n
    }) else {
        return;
    };
    if quedan == 0 {
        let entorno = ENTORNO
            .try_with(|e| e.try_borrow_mut().ok().and_then(|mut e| e.take()))
            .ok()
            .flatten();
        drop(entorno);
        bombear_lo_pendiente();
    }
}

/// Lo que el WebView2 dejo en la cola al cerrar (sus ultimos avisos y
/// liberaciones), antes de que el hilo se vaya.
fn bombear_lo_pendiente() {
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage,
    };
    let mut msg = MSG::default();
    let t = std::time::Instant::now();
    // SAFETY: la cola de mensajes de este hilo; solo se despacha lo que hay.
    unsafe {
        while t.elapsed().as_millis() < 200
            && PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool()
        {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// **El entorno y un controlador dentro de `padre`**: el entorno de este hilo
/// si ya hay (y si el navegador murio, uno nuevo y se reintenta una vez).
pub(crate) fn entorno_y_controlador(
    padre: HWND,
) -> Result<(ICoreWebView2Environment, ICoreWebView2Controller), ErrorWeb> {
    let hecho = entorno_y_controlador_sin_contar(padre);
    if hecho.is_ok() {
        VIVOS.with(|v| v.set(v.get() + 1));
    }
    hecho
}

fn entorno_y_controlador_sin_contar(
    padre: HWND,
) -> Result<(ICoreWebView2Environment, ICoreWebView2Controller), ErrorWeb> {
    let entorno = match ENTORNO.with(|e| e.borrow().clone()) {
        Some(e) => e,
        None => crear_entorno(&carpeta_de_datos())?,
    };
    match crear_controlador(&entorno, padre) {
        Ok(c) => {
            ENTORNO.with(|e| *e.borrow_mut() = Some(entorno.clone()));
            Ok((entorno, c))
        }
        Err(_) => {
            ENTORNO.with(|e| *e.borrow_mut() = None);
            let entorno = crear_entorno(&carpeta_de_datos())?;
            let c = crear_controlador(&entorno, padre)?;
            ENTORNO.with(|e| *e.borrow_mut() = Some(entorno.clone()));
            Ok((entorno, c))
        }
    }
}

/// **Precalienta el navegador** (18-oct): en un hilo suyo, un WebView2 oculto
/// en una ventana de mensajes que se queda vivo mientras viva la app. El
/// primer visor que se abra encuentra el proceso del navegador ya en marcha.
/// Cuesta la memoria de ese proceso: quien llama decide (equipos con memoria).
pub fn precalentar() {
    let _ = std::thread::Builder::new()
        .name("webview2-caliente".into())
        .spawn(|| {
            use windows::Win32::UI::WindowsAndMessaging::{
                CreateWindowExW, DispatchMessageW, GetMessageW, HWND_MESSAGE, MSG,
                TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE,
            };
            // SAFETY: COM en este hilo nuevo, que es suyo hasta que acaba.
            let _com = unsafe {
                windows::Win32::System::Com::CoInitializeEx(
                    None,
                    windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
                )
            };
            if !VisorHtml::hay_runtime() {
                return;
            }
            let t = std::time::Instant::now();
            // SAFETY: una ventana de mensajes de la clase de sistema «Static», de
            // este hilo; solo hace de padre del controlador oculto.
            let hwnd = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    windows::core::w!("Static"),
                    windows::core::w!("pixpin-webview2"),
                    WINDOW_STYLE(0),
                    0,
                    0,
                    0,
                    0,
                    Some(HWND_MESSAGE),
                    None,
                    None,
                    None,
                )
            };
            let Ok(hwnd) = hwnd else { return };
            let Ok((_e, controlador)) = entorno_y_controlador(hwnd) else {
                return;
            };
            tracing::info!(
                ms = t.elapsed().as_millis() as u64,
                "webview2: navegador precalentado"
            );
            let _vivo = controlador;
            let mut msg = MSG::default();
            // SAFETY: el bucle de mensajes de este hilo (WebView2 lo necesita).
            unsafe {
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        });
}

/// Crea el entorno de WebView2, bombeando mensajes hasta que responde.
///
/// La carpeta de datos de usuario se fuerza a la nuestra ([`carpeta_de_datos`])
/// porque la de serie va al lado del ejecutable, y PixPin puede estar
/// instalado en `Program Files`, donde no se escribe.
pub(crate) fn crear_entorno(carpeta: &Path) -> Result<ICoreWebView2Environment, ErrorWeb> {
    let datos = CoTaskMemPWSTR::from(carpeta.to_string_lossy().as_ref());
    let (tx, rx) = std::sync::mpsc::channel();

    CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
        Box::new(move |manejador| {
            // SAFETY: `datos` vive hasta el final de este closure y WebView2
            // copia la cadena antes de volver; `manejador` lo crea el propio
            // `wait_for_async_operation`.
            unsafe {
                CreateCoreWebView2EnvironmentWithOptions(
                    PCWSTR::null(),
                    *datos.as_ref().as_pcwstr(),
                    None::<&ICoreWebView2EnvironmentOptions>,
                    &manejador,
                )
            }
            .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |codigo, entorno| {
            codigo?;
            tx.send(entorno.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                .expect("el canal del entorno sigue abierto");
            Ok(())
        }),
    )
    .map_err(|e| ErrorWeb::Creacion(format!("{e:?}")))?;

    rx.recv()
        .map_err(|_| ErrorWeb::Creacion("el entorno nunca contesto".into()))?
        .map_err(|e| ErrorWeb::Creacion(e.to_string()))
}

/// Crea el controlador (la ventana hija de verdad) dentro de `padre`.
pub(crate) fn crear_controlador(
    entorno: &ICoreWebView2Environment,
    padre: HWND,
) -> Result<ICoreWebView2Controller, ErrorWeb> {
    let entorno = entorno.clone();
    let (tx, rx) = std::sync::mpsc::channel();

    CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
        Box::new(move |manejador| {
            // SAFETY: `padre` lo da quien llama y debe ser una ventana viva;
            // `entorno` es el que acaba de crear `crear_entorno`.
            unsafe { entorno.CreateCoreWebView2Controller(padre, &manejador) }
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |codigo, controlador| {
            codigo?;
            tx.send(controlador.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                .expect("el canal del controlador sigue abierto");
            Ok(())
        }),
    )
    .map_err(|e| ErrorWeb::Creacion(format!("{e:?}")))?;

    rx.recv()
        .map_err(|_| ErrorWeb::Creacion("el controlador nunca contesto".into()))?
        .map_err(|e| ErrorWeb::Creacion(e.to_string()))
}

/// Escape y F11, tambien con el foco DENTRO de la pagina.
///
/// `add_AcceleratorKeyPressed` es del CONTROLADOR y no de la vista: llega
/// antes de que la pagina vea la tecla, que es lo unico que funciona aqui.
/// Mientras se escribe en un campo del html, el WebView2 se queda el teclado
/// y la ventana de PixPin no recibe nada; sin esto el visor solo se cerraria
/// con Alt+F4 y no pasaria a pantalla completa.
fn conectar_teclas(
    controlador: &ICoreWebView2Controller,
    al_tecla: Rc<dyn Fn(u32)>,
    ctrl_p: Rc<std::cell::Cell<bool>>,
) -> Result<(), ErrorWeb> {
    use webview2_com::AcceleratorKeyPressedEventHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN;

    const VK_ESCAPE: u32 = 0x1B;
    const VK_F11: u32 = 0x7A;
    let mut token = 0i64;
    let manejador = AcceleratorKeyPressedEventHandler::create(Box::new(move |_c, args| {
        let Some(args) = args else { return Ok(()) };
        let (mut clase, mut tecla) = (Default::default(), 0u32);
        // SAFETY: `args` lo da WebView2 y vive durante el evento; las dos
        // salidas son variables locales.
        unsafe {
            args.KeyEventKind(&mut clase)?;
            args.VirtualKey(&mut tecla)?;
        }
        if clase == COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN && (tecla == VK_ESCAPE || tecla == VK_F11)
        {
            // Tragada: la pagina no tiene por que enterarse de que se cierra
            // su visor, y si la usara para otra cosa haria las dos.
            // SAFETY: propiedad del propio evento.
            unsafe {
                args.SetHandled(true)?;
            }
            al_tecla(tecla);
        } else if clase == COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
            && tecla == 0x50
            && ctrl_p.get()
            && ctrl_pulsado()
        {
            // Ctrl+P con marcos: imprimirlos, no la pagina entera de Edge.
            // SAFETY: propiedad del propio evento.
            unsafe {
                args.SetHandled(true)?;
            }
            al_tecla(CTRL_P);
        }
        Ok(())
    }));
    // SAFETY: `token` es un i64 valido a escribir; el manejador queda
    // referenciado por el propio WebView2.
    unsafe { controlador.add_AcceleratorKeyPressed(&manejador, &mut token) }
        .map_err(|e| ErrorWeb::Creacion(e.to_string()))?;
    Ok(())
}

/// Camino 1: inyecta `window.PixPinVisor` y escucha sus mensajes.
fn conectar_puente(
    vista: &ICoreWebView2,
    al_guardar: AlGuardar,
    otros: Rc<std::cell::RefCell<Vec<String>>>,
) -> Result<(), ErrorWeb> {
    let script = CoTaskMemPWSTR::from(puente::SCRIPT);
    let mut token = 0i64;

    // SAFETY: `vista` acaba de salir del controlador y sigue viva. Sin
    // `IsWebMessageEnabled` el puente entero seria mudo: `postMessage` no
    // llega.
    unsafe {
        let ajustes = vista
            .Settings()
            .map_err(|e| ErrorWeb::Creacion(e.to_string()))?;
        ajustes
            .SetIsWebMessageEnabled(true)
            .map_err(|e| ErrorWeb::Creacion(e.to_string()))?;
    }

    let manejador_script =
        AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(|codigo, _id| {
            if let Err(e) = codigo {
                tracing::warn!(error = %e, "no se pudo inyectar PixPinVisor");
            }
            Ok(())
        }));
    // SAFETY: `script` vive hasta el final de la funcion y WebView2 copia la
    // cadena antes de volver.
    unsafe {
        vista.AddScriptToExecuteOnDocumentCreated(*script.as_ref().as_pcwstr(), &manejador_script)
    }
    .map_err(|e| ErrorWeb::Creacion(e.to_string()))?;

    let manejador_mensaje =
        WebMessageReceivedEventHandler::create(Box::new(move |_vista, args| {
            if let Some(args) = args {
                let mut mensaje = PWSTR::null();
                // SAFETY: `args` lo da WebView2 y vive durante el evento; la
                // cadena que escribe se libera con `take_pwstr`.
                if unsafe { args.TryGetWebMessageAsString(&mut mensaje) }.is_ok() {
                    let mensaje = take_pwstr(mensaje);
                    if puente::es_peticion_de_guardado(&mensaje) {
                        atender_mensaje(&mensaje, &al_guardar);
                    } else if otros.borrow().len() < MENSAJES_GUARDADOS {
                        otros.borrow_mut().push(mensaje);
                    }
                }
            }
            Ok(())
        }));
    // SAFETY: `token` es un i64 valido a escribir y el manejador queda
    // referenciado por el propio WebView2.
    unsafe { vista.add_WebMessageReceived(&manejador_mensaje, &mut token) }
        .map_err(|e| ErrorWeb::Creacion(e.to_string()))?;

    Ok(())
}

/// Cuantos mensajes que no son guardados se guardan sin recoger: una pagina
/// que mande sin parar y un visor que no los recoja no se comen la memoria.
const MENSAJES_GUARDADOS: usize = 512;

/// Descodifica y valida lo que llego por el puente.
fn atender_mensaje(mensaje: &str, al_guardar: &AlGuardar) {
    if !puente::es_peticion_de_guardado(mensaje) {
        return;
    }
    let Some(datos) = puente::campo_de_texto(mensaje, "datos") else {
        tracing::warn!("PixPinVisor.guardar llego sin campo `datos`");
        return;
    };
    let Some(bytes) = base64::descodificar(&datos) else {
        tracing::warn!("PixPinVisor.guardar mando un base64 que no se entiende");
        return;
    };
    entregar(bytes, al_guardar, "puente");
}

/// Camino 2: desvia las descargas a un fichero nuestro y las recoge al acabar.
fn conectar_descargas(
    vista: &ICoreWebView2,
    al_guardar: AlGuardar,
    temporal: PathBuf,
) -> Result<(), ErrorWeb> {
    // `DownloadStarting` solo existe a partir de `ICoreWebView2_4`. Si el
    // runtime es mas viejo no se cae el visor entero: se queda sin el segundo
    // camino, que es exactamente lo que se podia hacer antes.
    let Ok(vista4) = vista.cast::<ICoreWebView2_4>() else {
        tracing::warn!(
            "el runtime de WebView2 es anterior a ICoreWebView2_4: sin interceptar descargas"
        );
        return Ok(());
    };

    let mut token = 0i64;
    // SAFETY: `vista4` es la misma interfaz consultada arriba, y `token` es un
    // i64 valido a escribir. El manejador se queda vivo porque WebView2 le
    // sube el contador de referencias.
    unsafe {
        vista4.add_DownloadStarting(
            &DownloadStartingEventHandler::create(Box::new(move |_vista, args| {
                if let Some(args) = args {
                    desviar_descarga(&args, &al_guardar, &temporal);
                }
                Ok(())
            })),
            &mut token,
        )
    }
    .map_err(|e| ErrorWeb::Creacion(e.to_string()))?;
    Ok(())
}

/// Manda una descarga a un fichero temporal propio y se suscribe a su final.
fn desviar_descarga(
    args: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2DownloadStartingEventArgs,
    al_guardar: &AlGuardar,
    temporal: &Path,
) {
    static CONTADOR: AtomicU64 = AtomicU64::new(0);
    // Solo se desvia la propia pagina (su boton de guardar baja un `.html`).
    // Lo demas —el CSV de una tabla, «Bajar CSV»— es una descarga de verdad:
    // se deja a WebView2, que la pone en Descargas con su aviso. Antes se
    // desviaba todo y lo que no era la pagina se tiraba en silencio.
    let mut sugerida = PWSTR::null();
    // SAFETY: `args` lo da WebView2 y vive durante el evento; la cadena que
    // escribe se libera con `take_pwstr`.
    if unsafe { args.ResultFilePath(&mut sugerida) }.is_ok() {
        let sugerida = take_pwstr(sugerida);
        if !es_la_pagina(&sugerida) {
            tracing::info!(%sugerida, "descarga que no es la pagina: la hace WebView2");
            return;
        }
    }
    let destino = temporal.join(format!(
        "descarga-{}.html",
        CONTADOR.fetch_add(1, Ordering::Relaxed)
    ));
    let destino_pcwstr = CoTaskMemPWSTR::from(destino.to_string_lossy().as_ref());
    let al_guardar = al_guardar.clone();

    // SAFETY: `args` lo da WebView2 y es valido durante el evento; la cadena
    // del destino vive hasta el final de esta funcion y WebView2 la copia.
    let resultado = unsafe {
        args.SetResultFilePath(*destino_pcwstr.as_ref().as_pcwstr())
            // `Handled` a true quita la barra de descargas del navegador: para
            // el usuario esto no es una descarga, es su boton de guardar.
            .and_then(|()| args.SetHandled(true))
            .and_then(|()| args.DownloadOperation())
    };
    let operacion = match resultado {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!(error = %e, "no se pudo desviar la descarga");
            return;
        }
    };

    let mut token = 0i64;
    // SAFETY: `operacion` viene de `args` y WebView2 la mantiene viva mientras
    // la descarga exista; el manejador queda referenciado por ella.
    let alta = unsafe {
        operacion.add_StateChanged(
            &StateChangedEventHandler::create(Box::new(move |operacion, _| {
                if let Some(operacion) = operacion {
                    recoger_descarga(&operacion, &al_guardar);
                }
                Ok(())
            })),
            &mut token,
        )
    };
    if let Err(e) = alta {
        tracing::warn!(error = %e, "no se pudo seguir el estado de la descarga");
    }
}

/// Si lo que se baja es una pagina (`.html`/`.htm`, por el nombre que
/// sugiere el navegador): lo unico que puede ser el boton de guardar.
fn es_la_pagina(ruta_sugerida: &str) -> bool {
    Path::new(ruta_sugerida)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"))
}

/// Si la descarga termino, lee el fichero temporal y lo entrega.
fn recoger_descarga(
    operacion: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2DownloadOperation,
    al_guardar: &AlGuardar,
) {
    let mut estado = COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED;
    let mut ruta_pwstr = PWSTR::null();
    // SAFETY: `operacion` la da WebView2 y vive durante el evento; los dos
    // punteros son a variables locales validas y la cadena se libera abajo.
    let leido = unsafe {
        operacion
            .State(&mut estado)
            .and_then(|()| operacion.ResultFilePath(&mut ruta_pwstr))
    };
    if leido.is_err() {
        if !ruta_pwstr.is_null() {
            let _ = take_pwstr(ruta_pwstr);
        }
        return;
    }
    let ruta = PathBuf::from(take_pwstr(ruta_pwstr));

    // Los estados intermedios (en curso, pausada) llegan por el mismo evento.
    if estado != COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED {
        return;
    }

    match std::fs::read(&ruta) {
        Ok(bytes) => {
            entregar(bytes, al_guardar, "descarga");
            // El temporal ya cumplio: quien guarda de verdad es quien llama.
            let _ = std::fs::remove_file(&ruta);
        }
        Err(e) => tracing::warn!(error = %e, ?ruta, "no se pudo leer la descarga interceptada"),
    }
}

/// Unico sitio por el que se llama a `al_guardar`, con el filtro delante.
///
/// Que sea uno solo es la gracia: los dos caminos pueden traer cualquier cosa
/// y el fichero del usuario esta al otro lado.
fn entregar(bytes: Vec<u8>, al_guardar: &AlGuardar, origen: &str) {
    if !validacion::parece_html_completo(&bytes) {
        tracing::warn!(
            origen,
            bytes = bytes.len(),
            "no parece la pagina: no se guarda"
        );
        return;
    }
    al_guardar(bytes);
}

/// **Cuanto tarda en verse la pagina** (18-oct, «los visores se sienten
/// lentos»): desde `desde` hasta que WebView2 termina de cargarla, al
/// registro (`webview2: pagina lista`). Lo que mide la prueba de latencia.
pub(crate) fn medir_carga(vista: &ICoreWebView2, que: &'static str, desde: std::time::Instant) {
    use webview2_com::NavigationCompletedEventHandler;
    let mut token = 0i64;
    let hecho = std::cell::Cell::new(false);
    let manejador = NavigationCompletedEventHandler::create(Box::new(move |_v, _a| {
        if !hecho.replace(true) {
            tracing::info!(
                que,
                ms = desde.elapsed().as_millis() as u64,
                "webview2: pagina lista"
            );
        }
        Ok(())
    }));
    // SAFETY: `token` es un i64 local; el manejador lo guarda WebView2.
    let _ = unsafe { vista.add_NavigationCompleted(&manejador, &mut token) };
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::cell::RefCell;

    /// Recoge lo que se entregaria al usuario, sin ventana ni navegador.
    fn espia() -> (AlGuardar, Rc<RefCell<Vec<Vec<u8>>>>) {
        let caja = Rc::new(RefCell::new(Vec::new()));
        let copia = caja.clone();
        let f: AlGuardar = Rc::new(move |bytes| copia.borrow_mut().push(bytes));
        (f, caja)
    }

    fn html_bueno() -> String {
        format!("<html><body>{}</body></html>", "a".repeat(400))
    }

    fn en_base64(texto: &str) -> String {
        const ALFABETO: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let bytes = texto.as_bytes();
        let mut salida = String::new();
        for trozo in bytes.chunks(3) {
            let mut bloque = [0u8; 3];
            bloque[..trozo.len()].copy_from_slice(trozo);
            let n =
                (u32::from(bloque[0]) << 16) | (u32::from(bloque[1]) << 8) | u32::from(bloque[2]);
            for i in 0..4 {
                if i <= trozo.len() {
                    salida.push(ALFABETO[((n >> (18 - 6 * i)) & 0x3F) as usize] as char);
                } else {
                    salida.push('=');
                }
            }
        }
        salida
    }

    #[test]
    fn solo_se_desvia_la_descarga_de_la_propia_pagina() {
        assert!(es_la_pagina(r"C:\Users\x\Downloads\Obra.html"));
        assert!(es_la_pagina(r"C:\Users\x\Downloads\Obra (anotado).HTM"));
        // Casos negativos: el CSV de una tabla y lo que no tiene extension
        // son descargas de verdad y se dejan a WebView2.
        assert!(!es_la_pagina(r"C:\Users\x\Downloads\Gastos.csv"));
        assert!(!es_la_pagina(r"C:\Users\x\Downloads\descarga"));
        assert!(!es_la_pagina(""));
    }

    #[test]
    fn un_guardado_bueno_del_puente_llega_entero() {
        let (f, caja) = espia();
        let mensaje = format!(
            r#"{{"tipo":"guardar","nombre":"n.html","datos":"{}"}}"#,
            en_base64(&html_bueno())
        );
        atender_mensaje(&mensaje, &f);
        assert_eq!(caja.borrow().len(), 1);
        assert_eq!(caja.borrow()[0], html_bueno().into_bytes());
    }

    #[test]
    fn un_guardado_demasiado_corto_no_toca_el_fichero() {
        let (f, caja) = espia();
        let mensaje = format!(
            r#"{{"tipo":"guardar","datos":"{}"}}"#,
            en_base64("<html></html>")
        );
        atender_mensaje(&mensaje, &f);
        assert!(caja.borrow().is_empty());
    }

    #[test]
    fn un_guardado_sin_el_cierre_de_html_no_toca_el_fichero() {
        let (f, caja) = espia();
        let mensaje = format!(
            r#"{{"tipo":"guardar","datos":"{}"}}"#,
            en_base64(&"z".repeat(1000))
        );
        atender_mensaje(&mensaje, &f);
        assert!(caja.borrow().is_empty());
    }

    #[test]
    fn un_base64_roto_no_toca_el_fichero() {
        let (f, caja) = espia();
        atender_mensaje(r#"{"tipo":"guardar","datos":"no~es~base64"}"#, &f);
        assert!(caja.borrow().is_empty());
    }

    #[test]
    fn un_mensaje_que_no_es_de_guardar_se_ignora() {
        let (f, caja) = espia();
        let mensaje = format!(
            r#"{{"tipo":"otra cosa","datos":"{}"}}"#,
            en_base64(&html_bueno())
        );
        atender_mensaje(&mensaje, &f);
        assert!(caja.borrow().is_empty());
    }

    #[test]
    fn el_area_se_traduce_a_un_rect_de_windows() {
        let r = a_rect(Rect {
            x: 4,
            y: 8,
            ancho: 100,
            alto: 50,
        });
        assert_eq!((r.left, r.top, r.right, r.bottom), (4, 8, 104, 58));
    }

    #[test]
    fn cada_visor_pide_su_propia_carpeta_temporal() {
        assert_ne!(carpeta_temporal(), carpeta_temporal());
    }

    #[test]
    fn preguntar_por_el_runtime_no_revienta() {
        // No se afirma el valor: depende de si este equipo tiene WebView2.
        // Lo que prueba es que el enlace con `webview2loader` funciona.
        let _ = VisorHtml::hay_runtime();
    }

    #[test]
    #[ignore = "necesita una ventana de verdad y el runtime de WebView2"]
    fn abre_un_html_dentro_de_una_ventana() {
        // Pendiente: crear un HWND, llamar a `VisorHtml::nuevo` con un html de
        // prueba y comprobar que el boton de guardar de la pagina llega a
        // `al_guardar`. Sin escritorio no se puede.
    }
}
