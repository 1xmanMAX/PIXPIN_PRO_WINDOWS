//! La ventana de overlay: una por monitor, clase y WndProc PROPIOS.
//!
//! La retrospectiva de S1-A lo exige: el WndProc de VentanaMensajes llama a
//! PostQuitMessage ante cualquier WM_DESTROY. Si los overlays lo
//! compartieran, cerrar uno mataria la aplicacion. Este WndProc no llama a
//! PostQuitMessage NUNCA: el bucle modal termina porque el callback lo dice,
//! no porque una ventana muera.
//!
//! WS_EX_NOREDIRECTIONBITMAP: la ventana no tiene superficie GDI; todo lo
//! visible lo presenta la Superficie de pixpin-render por DirectComposition.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Once;

use pixpin_geom::{Punto, Rect};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{InvalidateRect, ValidateRect};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
// En windows 0.62, AttachThreadInput vive en System::Threading.
use windows::Win32::System::Threading::AttachThreadInput;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

/// WM_APP+1: otros hilos lo PostMessage-an para despertar el bucle modal.
pub const MSG_DESPIERTA: u32 = WM_APP + 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventoOverlay {
    RatonMovido(Punto),
    BotonPulsado(Punto),
    BotonSoltado(Punto),
    Tecla {
        vk: u32,
        shift: bool,
    },
    Pintar,
    CambioDpi,
    /// MSG_DESPIERTA recibido: hay trabajo de otro hilo esperando.
    Despierta,
    /// WM_CLOSE (Alt+F4): el usuario quiere cerrar; tratar como cancelar.
    Cerrar,
    /// Rueda del raton, positivo hacia arriba. La capa viva la usa para el
    /// grosor del trazo y el aumento de la lupa (D55).
    Rueda(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormaCursorWin {
    Cruz,
    Mover,
    RedimNS,
    RedimEO,
    RedimNeSo,
    RedimNoSe,
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorOverlay {
    #[error("no se pudo crear la ventana de overlay: {0}")]
    Creacion(#[source] windows::core::Error),
}

thread_local! {
    /// Cola de eventos del hilo de interfaz. Solo la toca el WndProc (que
    /// corre en este hilo) y bucle_modal. Los hilos ajenos usan PostMessage.
    static PENDIENTES_OVERLAY: RefCell<VecDeque<(HWND, EventoOverlay)>> =
        const { RefCell::new(VecDeque::new()) };
    /// Cursor vigente por ventana; WM_SETCURSOR lo consulta.
    static CURSOR: RefCell<Vec<(HWND, FormaCursorWin)>> = const { RefCell::new(Vec::new()) };
}

static REGISTRO: Once = Once::new();

pub struct VentanaOverlay {
    hwnd: HWND,
    area: Rect,
}

impl VentanaOverlay {
    pub fn nueva(area: Rect) -> Result<Self, ErrorOverlay> {
        REGISTRO.call_once(registrar_clase);
        // SAFETY: la clase quedo registrada en call_once; los estilos son
        // constantes documentadas y el modulo es el propio.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW,
                w!("PixPinOverlay"),
                w!(""),
                WS_POPUP,
                area.x,
                area.y,
                area.ancho as i32,
                area.alto as i32,
                None,
                None,
                Some(
                    GetModuleHandleW(None)
                        .map_err(ErrorOverlay::Creacion)?
                        .into(),
                ),
                None,
            )
            .map_err(ErrorOverlay::Creacion)?
        };
        Ok(Self { hwnd, area })
    }

    pub fn handle(&self) -> HWND {
        self.hwnd
    }

    pub fn area(&self) -> Rect {
        self.area
    }

    pub fn mostrar(&self) {
        // SIN activar: la activacion de SW_SHOW sincroniza con el shell y
        // costaba 25-40 ms medidos. El foco lo toma enfocar() despues, una
        // sola vez y fuera del camino critico de "visible".
        // SAFETY: la ventana es propia y esta viva.
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
        }
    }

    /// Deja pasar los clics a la aplicacion de abajo, o vuelve a
    /// recogerlos (D50).
    ///
    /// Es lo que hace posible la capa viva: dibujas encima de lo que estas
    /// haciendo, y cuando quieres seguir trabajando la vuelves pasante — el
    /// dibujo se sigue viendo, pero el raton lo atraviesa como si no
    /// estuviera. Sin esto, una capa a pantalla completa secuestra el
    /// escritorio entero.
    ///
    /// `WS_EX_TRANSPARENT` afecta SOLO al raton. El teclado sigue llegando
    /// mientras la ventana tenga el foco, que es lo que permite volver a
    /// activar el dibujo con el mismo atajo.
    pub fn poner_pasante(&self, pasante: bool) {
        use windows::Win32::UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_EX_TRANSPARENT,
        };
        // SAFETY: lee y escribe el estilo extendido de una ventana propia.
        unsafe {
            let actual = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) as u32;
            let nuevo = if pasante {
                actual | WS_EX_TRANSPARENT.0
            } else {
                actual & !WS_EX_TRANSPARENT.0
            };
            SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, nuevo as isize);
        }
    }

    /// Si ahora mismo los clics la atraviesan.
    pub fn es_pasante(&self) -> bool {
        use windows::Win32::UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, WS_EX_TRANSPARENT,
        };
        // SAFETY: consulta de solo lectura sobre ventana propia.
        let actual = unsafe { GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) as u32 };
        actual & WS_EX_TRANSPARENT.0 != 0
    }

    /// Esconde la ventana sin destruirla. El overlay retiene sus ventanas
    /// entre capturas porque crearlas (con su DComp y su swapchain) costaba
    /// ~90 ms de los 50 permitidos; una ventana oculta no recibe entrada ni
    /// se dibuja, asi que retenerla no cuesta nada.
    pub fn ocultar(&self) {
        // SAFETY: la ventana es propia y esta viva.
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    /// Toma el foco de teclado. Sin esto, Esc/Enter/flechas van a la
    /// aplicacion que estaba activa y el overlay es sordo al teclado — el
    /// primer bug real que encontro la prueba de extremo a extremo.
    ///
    /// `SetForegroundWindow` a secas es NO DETERMINISTA: Windows deniega
    /// robar el primer plano segun quien tuviera la ultima entrada (funciono
    /// a las 21:49 y fallo a las 21:52 con el mismo binario). El adjunto
    /// temporal al hilo del primer plano es el remedio clasico: mientras
    /// dura, ambos hilos comparten el permiso de foco.
    pub fn enfocar(&self) {
        use windows::Win32::System::Threading::GetCurrentThreadId;
        // SAFETY: consultas de solo lectura sobre el estado global, adjunto
        // simetrico (true/false) entre hilos vivos, y foco sobre ventana
        // propia; si el primer plano cambia entre medias, lo peor es que el
        // foco no llegue, que es el estado del que partiamos.
        unsafe {
            let primer_plano = GetForegroundWindow();
            if primer_plano.is_invalid() {
                let _ = SetForegroundWindow(self.hwnd);
                return;
            }
            let hilo_fg = GetWindowThreadProcessId(primer_plano, None);
            let hilo_yo = GetCurrentThreadId();
            if hilo_fg != hilo_yo {
                let _ = AttachThreadInput(hilo_yo, hilo_fg, true);
            }
            let _ = SetForegroundWindow(self.hwnd);
            let _ = SetFocus(Some(self.hwnd));
            if hilo_fg != hilo_yo {
                let _ = AttachThreadInput(hilo_yo, hilo_fg, false);
            }
        }
    }

    pub fn invalidar(&self) {
        // SAFETY: InvalidateRect sobre ventana propia; None invalida entera.
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    pub fn poner_cursor(&self, f: FormaCursorWin) {
        CURSOR.with(|c| {
            let mut c = c.borrow_mut();
            match c.iter_mut().find(|(h, _)| *h == self.hwnd) {
                Some(par) => par.1 = f,
                None => c.push((self.hwnd, f)),
            }
        });
    }
}

impl Drop for VentanaOverlay {
    fn drop(&mut self) {
        CURSOR.with(|c| c.borrow_mut().retain(|(h, _)| *h != self.hwnd));
        PENDIENTES_OVERLAY.with(|p| p.borrow_mut().retain(|(h, _)| *h != self.hwnd));
        // SAFETY: destruir una ventana propia desde su hilo es valido; si ya
        // fue destruida por el sistema, DestroyWindow falla y se ignora.
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// Bombea mensajes hasta que el callback devuelva Continuar::No.
///
/// No filtra por ventana: el teclado y MSG_DESPIERTA pueden llegar a
/// cualquiera de los overlays. Las ventanas viven en el llamante; el
/// parametro existe para atar tiempos de vida y dejar claro el contrato.
pub fn bucle_modal(
    ventanas: &[VentanaOverlay],
    mut callback: impl FnMut(HWND, EventoOverlay) -> crate::ventana::Continuar,
) {
    let _ = ventanas;
    let mut msg = MSG::default();
    loop {
        // SAFETY: GetMessageW con punteros locales validos; <= 0 es salida.
        let r = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if r.0 <= 0 {
            return;
        }
        // SAFETY: protocolo estandar de bombeo.
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        // Drenar lo que el WndProc haya encolado durante el Dispatch.
        loop {
            let siguiente = PENDIENTES_OVERLAY.with(|p| p.borrow_mut().pop_front());
            let Some((hwnd, evento)) = siguiente else {
                break;
            };
            if callback(hwnd, evento) == crate::ventana::Continuar::No {
                return;
            }
        }
    }
}

fn registrar_clase() {
    // SAFETY: registro unico (Once) de una clase con WndProc propio; los
    // campos no usados quedan a cero, que es lo que la API espera.
    unsafe {
        let clase = WNDCLASSW {
            lpfnWndProc: Some(procedimiento_overlay),
            hInstance: GetModuleHandleW(None).expect("modulo propio").into(),
            lpszClassName: w!("PixPinOverlay"),
            ..Default::default()
        };
        RegisterClassW(&clase);
    }
}

extern "system" fn procedimiento_overlay(
    hwnd: HWND,
    mensaje: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let encolar = |e: EventoOverlay| {
        PENDIENTES_OVERLAY.with(|p| p.borrow_mut().push_back((hwnd, e)));
    };
    // Cliente -> escritorio virtual: la ventana vive en su esquina del
    // escritorio, asi que basta sumar su origen real.
    let punto = |lparam: LPARAM| {
        let mut r = RECT::default();
        // SAFETY: GetWindowRect sobre la ventana del propio WndProc.
        unsafe {
            let _ = GetWindowRect(hwnd, &mut r);
        }
        Punto {
            x: r.left + (lparam.0 & 0xFFFF) as i16 as i32,
            y: r.top + ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
        }
    };

    match mensaje {
        WM_MOUSEMOVE => {
            encolar(EventoOverlay::RatonMovido(punto(lparam)));
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            // La palabra alta del wparam trae el giro con signo.
            let delta = ((wparam.0 >> 16) & 0xFFFF) as i16 as i32;
            encolar(EventoOverlay::Rueda(delta));
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            // SAFETY: SetCapture sobre ventana propia: el arrastre no se
            // pierde al salir del borde.
            unsafe { SetCapture(hwnd) };
            encolar(EventoOverlay::BotonPulsado(punto(lparam)));
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            // SAFETY: libera la captura tomada arriba.
            unsafe {
                let _ = ReleaseCapture();
            }
            encolar(EventoOverlay::BotonSoltado(punto(lparam)));
            LRESULT(0)
        }
        WM_KEYDOWN => {
            // SAFETY: GetKeyState es una consulta sin precondiciones.
            let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
            encolar(EventoOverlay::Tecla {
                vk: wparam.0 as u32,
                shift,
            });
            LRESULT(0)
        }
        WM_PAINT => {
            // SAFETY: ValidateRect marca la ventana como pintada; el dibujo
            // real lo hace el swapchain, no GDI.
            unsafe {
                let _ = ValidateRect(Some(hwnd), None);
            }
            encolar(EventoOverlay::Pintar);
            LRESULT(0)
        }
        WM_DPICHANGED => {
            encolar(EventoOverlay::CambioDpi);
            LRESULT(0)
        }
        WM_SETCURSOR => {
            let forma =
                CURSOR.with(|c| c.borrow().iter().find(|(h, _)| *h == hwnd).map(|(_, f)| *f));
            let id = match forma.unwrap_or(FormaCursorWin::Cruz) {
                FormaCursorWin::Cruz => IDC_CROSS,
                FormaCursorWin::Mover => IDC_SIZEALL,
                FormaCursorWin::RedimNS => IDC_SIZENS,
                FormaCursorWin::RedimEO => IDC_SIZEWE,
                FormaCursorWin::RedimNeSo => IDC_SIZENESW,
                FormaCursorWin::RedimNoSe => IDC_SIZENWSE,
            };
            // SAFETY: LoadCursorW de un cursor del sistema y SetCursor son
            // llamadas sin precondiciones sobre recursos compartidos.
            unsafe {
                if let Ok(cursor) = LoadCursorW(None, id) {
                    SetCursor(Some(cursor));
                }
            }
            LRESULT(1)
        }
        WM_CLOSE => {
            // Alt+F4 sobre el overlay: cancelar limpiamente, NUNCA destruir
            // la ventana por debajo del bucle modal — dejaria un overlay
            // zombi invisible con el bucle vivo.
            encolar(EventoOverlay::Cerrar);
            LRESULT(0)
        }
        m if m == MSG_DESPIERTA => {
            encolar(EventoOverlay::Despierta);
            LRESULT(0)
        }
        // NUNCA PostQuitMessage aqui: ver el comentario de modulo.
        _ => {
            // SAFETY: delegacion estandar al procedimiento por defecto.
            unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_geom::Rect;

    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn el_modo_pasante_se_activa_y_se_quita() {
        // D50: es lo que permite dibujar encima de lo que estas haciendo y
        // luego seguir trabajando sin cerrar la capa. Se comprueba contra el
        // estilo REAL de la ventana, no contra una variable propia: si
        // Windows no aplicara el cambio, un booleano nuestro mentiria.
        let v = VentanaOverlay::nueva(Rect {
            x: 0,
            y: 0,
            ancho: 300,
            alto: 200,
        })
        .expect("la ventana deberia crearse");

        assert!(!v.es_pasante(), "una capa nace recogiendo el raton");
        v.poner_pasante(true);
        assert!(v.es_pasante(), "los clics deberian atravesarla");
        v.poner_pasante(false);
        assert!(!v.es_pasante(), "y volver a recogerse");
    }

    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn poner_pasante_no_borra_los_demas_estilos() {
        // Caso negativo de la escritura del estilo: un SetWindowLongPtrW que
        // asignara el estilo en vez de combinarlo dejaria la ventana sin
        // TOPMOST ni NOREDIRECTIONBITMAP, y la capa dejaria de componerse.
        use windows::Win32::UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, WS_EX_TOPMOST,
        };
        let v = VentanaOverlay::nueva(Rect {
            x: 0,
            y: 0,
            ancho: 300,
            alto: 200,
        })
        .unwrap();
        v.poner_pasante(true);
        // SAFETY: consulta de solo lectura sobre la ventana del test.
        let estilo = unsafe { GetWindowLongPtrW(v.handle(), GWL_EXSTYLE) as u32 };
        assert!(
            estilo & WS_EX_TOPMOST.0 != 0,
            "la capa perdio el TOPMOST al volverse pasante"
        );
    }

    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn crear_y_destruir_un_overlay_no_toca_al_resto() {
        // La mina de la retrospectiva: si el WndProc del overlay llamara a
        // PostQuitMessage en WM_DESTROY, destruir la primera ventana
        // contaminaria el bucle de la segunda. Se destruye una y se
        // comprueba que la otra sigue viva y funcional.
        let a = VentanaOverlay::nueva(Rect {
            x: 0,
            y: 0,
            ancho: 200,
            alto: 200,
        })
        .expect("primera ventana");
        let b = VentanaOverlay::nueva(Rect {
            x: 200,
            y: 0,
            ancho: 200,
            alto: 200,
        })
        .expect("segunda ventana");

        drop(a);

        // SAFETY: IsWindow es una consulta sin precondiciones.
        let viva = unsafe {
            windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(b.handle())).as_bool()
        };
        assert!(viva, "destruir un overlay no debe llevarse a los demas");
    }

    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn el_bucle_modal_termina_cuando_el_callback_dice_no() {
        let v = VentanaOverlay::nueva(Rect {
            x: 0,
            y: 0,
            ancho: 100,
            alto: 100,
        })
        .unwrap();
        // Se autoenvia el mensaje de despertar; el callback corta al verlo.
        // Si bucle_modal no drenara la cola ni tradujera MSG_DESPIERTA, este
        // test se quedaria colgado (el timeout del harness lo delata).
        // SAFETY: PostMessageW a una ventana propia y viva.
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                Some(v.handle()),
                MSG_DESPIERTA,
                Default::default(),
                Default::default(),
            )
            .unwrap();
        }
        let mut despertares = 0;
        bucle_modal(std::slice::from_ref(&v), |_hwnd, evento| {
            if matches!(evento, EventoOverlay::Pintar) {
                return crate::ventana::Continuar::Si;
            }
            despertares += 1;
            crate::ventana::Continuar::No
        });
        assert_eq!(despertares, 1);
    }
}
