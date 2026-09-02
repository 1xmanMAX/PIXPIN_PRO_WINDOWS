//! La paleta flotante del pin (D58): la caja de herramientas junto al pin
//! mientras se anota.
//!
//! El pin es L2 y la caja (`pixpin-ui`) es L3, asi que el pin no puede
//! pintarla ni conocer sus botones. Esta ventana solo sabe dos cosas: DONDE
//! se pulsa y pintar lo que le digan. Quien la coloca, la pinta y decide que
//! hace cada boton es el gestor, que vive en la capa que si ve a los dos.
//!
//! No se activa nunca (`WS_EX_NOACTIVATE` y `WM_MOUSEACTIVATE`): el teclado
//! tiene que seguir en el pin, que es donde se escribe y donde Escape sale.
//! Una paleta que robara el foco rompería el texto in situ al primer clic.

use std::rc::Rc;
use std::sync::Once;

use pixpin_geom::{Punto, Rect};
use pixpin_render::{MotorRender, Pintor, Superficie};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Graphics::Gdi::ValidateRect;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::ventana::ErrorPin;

/// Lo que pinta la paleta. Lo pone el gestor y se vuelve a llamar en cada
/// `WM_PAINT`, asi que tiene que ser autocontenido (copias, no prestamos).
pub type PintorPaleta = Box<dyn Fn(&Pintor)>;

struct PaletaInterno {
    motor: Rc<MotorRender>,
    superficie: Superficie,
    pintor: Option<PintorPaleta>,
    al_pulsar: Box<dyn Fn(Punto)>,
}

/// Una ventana pequena, siempre encima, que no se activa. Se destruye al
/// soltarla: la paleta vive exactamente lo que dura el modo anotacion.
pub struct Paleta {
    hwnd: HWND,
}

static REGISTRO: Once = Once::new();

impl Paleta {
    /// `rect` en pixeles fisicos del escritorio virtual. `al_pulsar` recibe
    /// el punto LOCAL a la paleta (esquina superior izquierda = 0,0).
    pub fn nueva(
        d3d: &ID3D11Device,
        motor: Rc<MotorRender>,
        rect: Rect,
        al_pulsar: Box<dyn Fn(Punto)>,
    ) -> Result<Paleta, ErrorPin> {
        REGISTRO.call_once(registrar_clase);
        // SAFETY: la clase quedo registrada en call_once; estilos constantes
        // documentados; modulo propio.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("PixPinPaleta"),
                w!(""),
                WS_POPUP,
                rect.x,
                rect.y,
                rect.ancho as i32,
                rect.alto as i32,
                None,
                None,
                Some(GetModuleHandleW(None).map_err(ErrorPin::Creacion)?.into()),
                None,
            )
            .map_err(ErrorPin::Creacion)?
        };
        let superficie = Superficie::nueva(&motor, d3d, hwnd, rect.ancho, rect.alto)?;
        let interno = Box::new(PaletaInterno {
            motor,
            superficie,
            pintor: None,
            al_pulsar,
        });
        // SAFETY: la ventana es propia y viva; el Box se cede al USERDATA y
        // se recupera exactamente una vez en WM_NCDESTROY.
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(interno) as isize);
        }
        // SAFETY: mostrar sin activar: el foco se queda en el pin.
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        Ok(Paleta { hwnd })
    }

    /// Cambia como se pinta y repinta ya. El mismo pintor sirve para los
    /// `WM_PAINT` que vengan despues.
    pub fn poner_pintor(&self, pintor: PintorPaleta) {
        if let Some(i) = interno_de(self.hwnd) {
            i.pintor = Some(pintor);
            pintar(i);
        }
    }
}

impl Drop for Paleta {
    fn drop(&mut self) {
        // SAFETY: destruir una ventana propia desde su hilo; WM_NCDESTROY
        // libera el Box.
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// El interno colgado del USERDATA, si la ventana sigue viva.
fn interno_de<'a>(hwnd: HWND) -> Option<&'a mut PaletaInterno> {
    // SAFETY: el puntero lo puso Paleta::nueva y solo WM_NCDESTROY lo
    // retira; entre ambos es un Box valido. Todo ocurre en el hilo de
    // interfaz.
    unsafe {
        let crudo = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PaletaInterno;
        crudo.as_mut()
    }
}

fn registrar_clase() {
    // SAFETY: registro unico (Once) de una clase con WndProc propio; los
    // campos no usados quedan a cero, que es lo que la API espera.
    unsafe {
        let clase = WNDCLASSW {
            lpfnWndProc: Some(procedimiento_paleta),
            hInstance: GetModuleHandleW(None).expect("modulo propio").into(),
            lpszClassName: w!("PixPinPaleta"),
            ..Default::default()
        };
        RegisterClassW(&clase);
    }
}

fn pintar(i: &PaletaInterno) {
    let Ok(destino) = i.superficie.empezar(&i.motor) else {
        return;
    };
    let _ = i.motor.dibujar(&destino, |p| {
        p.limpiar_transparente();
        if let Some(f) = &i.pintor {
            f(p);
        }
    });
    let _ = i.superficie.presentar();
}

extern "system" fn procedimiento_paleta(
    hwnd: HWND,
    mensaje: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match mensaje {
        // Ni el clic la activa: el teclado sigue en el pin.
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_LBUTTONUP => {
            if let Some(i) = interno_de(hwnd) {
                let p = Punto {
                    x: (lparam.0 & 0xFFFF) as i16 as i32,
                    y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
                };
                (i.al_pulsar)(p);
            }
            LRESULT(0)
        }
        WM_PAINT => {
            // SAFETY: ValidateRect marca la ventana como pintada; el dibujo
            // real lo hace el swapchain, no GDI.
            unsafe {
                let _ = ValidateRect(Some(hwnd), None);
            }
            if let Some(i) = interno_de(hwnd) {
                pintar(i);
            }
            LRESULT(0)
        }
        WM_NCDESTROY => {
            // SAFETY: recupera el Box cedido en Paleta::nueva exactamente
            // una vez y deja el USERDATA a cero antes de soltarlo.
            unsafe {
                let crudo = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut PaletaInterno;
                if !crudo.is_null() {
                    drop(Box::from_raw(crudo));
                }
            }
            LRESULT(0)
        }
        // NUNCA PostQuitMessage: cerrar la paleta no apaga la aplicacion.
        // SAFETY: delegar al procedimiento por defecto es el protocolo.
        _ => unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) },
    }
}
