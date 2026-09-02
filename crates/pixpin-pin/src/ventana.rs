//! La ventana del pin: PixPinPin, autocontenida tras GWLP_USERDATA.
//!
//! El pin vive en el bucle principal de la app (VentanaMensajes::ejecutar
//! bombea todos los mensajes del hilo), asi que su WndProc ejecuta los
//! efectos ahi mismo: mover con SetWindowPos, redimensionar recreando la
//! superficie, cerrar destruyendo. El ejecutable se entera por el callback
//! CambioPin (unica via: pixpin-pin no puede tocar el almacen, misma capa).
//!
//! Este WndProc JAMAS llama a PostQuitMessage — tercera vez que la mina de
//! S1-A esta a punto de pisarse, tercera vez que el comentario lo impide.

use std::rc::Rc;
use std::sync::Once;

use pixpin_codec::ImagenRgba;
use pixpin_geom::{Punto, Rect};
use pixpin_render::{Color, MotorRender, RectF, Superficie};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Graphics::Gdi::ValidateRect;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::estado::{EfectoPin, EstadoPin, EventoPin, MINIMO_LOGICO};

/// Margen transparente alrededor del contenido: ahi vive la sombra (D30).
pub const MARGEN_SOMBRA_LOGICO: u32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CambioPin {
    Movido(Rect),
    Redimensionado(Rect),
    Cerrado,
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorPin {
    #[error("no se pudo crear la ventana del pin: {0}")]
    Creacion(#[source] windows::core::Error),
    #[error("no se pudo preparar el dibujo del pin: {0}")]
    Dibujo(#[from] pixpin_render::ErrorRender),
}

/// Rect de VENTANA para un contenido: el margen de sombra a cada lado.
pub fn rect_ventana(contenido: Rect, escala_por_cien: u32) -> Rect {
    let m = (MARGEN_SOMBRA_LOGICO * escala_por_cien / 100) as i32;
    Rect {
        x: contenido.x - m,
        y: contenido.y - m,
        ancho: contenido.ancho + 2 * m as u32,
        alto: contenido.alto + 2 * m as u32,
    }
}

/// La inversa exacta de `rect_ventana`.
pub fn contenido_desde_ventana(ventana: Rect, escala_por_cien: u32) -> Rect {
    let m = (MARGEN_SOMBRA_LOGICO * escala_por_cien / 100) as i32;
    Rect {
        x: ventana.x + m,
        y: ventana.y + m,
        ancho: ventana.ancho.saturating_sub(2 * m as u32),
        alto: ventana.alto.saturating_sub(2 * m as u32),
    }
}

/// Todo lo que el WndProc necesita, colgado de GWLP_USERDATA.
struct PinInterno {
    estado: EstadoPin,
    escala_por_cien: u32,
    motor: Rc<MotorRender>,
    d3d: ID3D11Device,
    superficie: Superficie,
    /// (ancho, alto) nativos de la imagen: el 100% del doble clic.
    imagen_nativa: (u32, u32),
    bitmap: ID2D1Bitmap1,
    al_cambiar: Box<dyn Fn(CambioPin)>,
}

pub struct Pin {
    hwnd: HWND,
}

static REGISTRO: Once = Once::new();

impl Pin {
    /// Crea el pin visible (sin robar el foco: spec 4.4) con la imagen ya
    /// pintada. `rect_contenido` en pixeles fisicos del escritorio virtual.
    pub fn nuevo(
        d3d: &ID3D11Device,
        motor: Rc<MotorRender>,
        imagen: &ImagenRgba,
        rect_contenido: Rect,
        escala_por_cien: u32,
        al_cambiar: Box<dyn Fn(CambioPin)>,
    ) -> Result<Pin, ErrorPin> {
        REGISTRO.call_once(registrar_clase);
        let ventana = rect_ventana(rect_contenido, escala_por_cien);
        // SAFETY: la clase quedo registrada en call_once; estilos constantes
        // documentados; modulo propio.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW,
                w!("PixPinPin"),
                w!(""),
                WS_POPUP,
                ventana.x,
                ventana.y,
                ventana.ancho as i32,
                ventana.alto as i32,
                None,
                None,
                Some(GetModuleHandleW(None).map_err(ErrorPin::Creacion)?.into()),
                None,
            )
            .map_err(ErrorPin::Creacion)?
        };

        let superficie = Superficie::nueva(&motor, d3d, hwnd, ventana.ancho, ventana.alto)?;
        let bitmap = motor.bitmap_desde_pixeles(imagen.ancho, imagen.alto, &imagen.pixeles)?;

        let interno = Box::new(PinInterno {
            estado: EstadoPin::nuevo(rect_contenido, escala_por_cien),
            escala_por_cien,
            motor,
            d3d: d3d.clone(),
            superficie,
            imagen_nativa: (imagen.ancho, imagen.alto),
            bitmap,
            al_cambiar,
        });
        // SAFETY: la ventana es propia y viva; el Box se cede al USERDATA y
        // se recupera exactamente una vez en WM_NCDESTROY.
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(interno) as isize);
        }

        let pin = Pin { hwnd };
        pin.repintar();
        // SAFETY: mostrar sin activar (spec 4.4: pinear no roba el foco).
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        Ok(pin)
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn rect_contenido(&self) -> Rect {
        match interno_de(self.hwnd) {
            Some(i) => i.estado.rect(),
            None => Rect {
                x: 0,
                y: 0,
                ancho: 0,
                alto: 0,
            },
        }
    }

    fn repintar(&self) {
        if let Some(i) = interno_de(self.hwnd) {
            pintar(i);
        }
    }
}

impl Drop for Pin {
    fn drop(&mut self) {
        // SAFETY: destruir una ventana propia desde su hilo; si el WndProc ya
        // la destruyo (Esc), DestroyWindow falla y se ignora.
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// El interno colgado del USERDATA, si la ventana sigue viva.
fn interno_de<'a>(hwnd: HWND) -> Option<&'a mut PinInterno> {
    // SAFETY: el puntero lo puso Pin::nuevo y solo WM_NCDESTROY lo retira;
    // entre ambos es un Box valido. Todo ocurre en el hilo de interfaz.
    unsafe {
        let crudo = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PinInterno;
        crudo.as_mut()
    }
}

fn registrar_clase() {
    // SAFETY: registro unico (Once); CS_DBLCLKS para recibir WM_LBUTTONDBLCLK.
    unsafe {
        let clase = WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(procedimiento_pin),
            hInstance: GetModuleHandleW(None).expect("modulo propio").into(),
            lpszClassName: w!("PixPinPin"),
            ..Default::default()
        };
        RegisterClassW(&clase);
    }
}

/// Dibuja el fotograma completo del pin: sombra + imagen. Cero cromo (D23).
fn pintar(i: &PinInterno) {
    let Ok(destino) = i.superficie.empezar(&i.motor) else {
        return;
    };
    let escala = i.escala_por_cien as f32 / 100.0;
    let m = MARGEN_SOMBRA_LOGICO as f32 * escala;
    let contenido = i.estado.rect();
    let (w, h) = (contenido.ancho as f32, contenido.alto as f32);
    let radio = 8.0 * escala;

    let _ = i.motor.dibujar(&destino, |p| {
        p.limpiar_transparente();
        // Sombra difusa: seis aros redondeados concentricos de alfa
        // decreciente, desplazados hacia abajo. Sin desenfoque real y
        // suficiente para el look de recorte elevado (D30). El cache por
        // bitmap de la spec queda para S2-B.
        let desplome = 2.0 * escala;
        for (paso, alfa) in [0.10f32, 0.08, 0.06, 0.045, 0.03, 0.02].iter().enumerate() {
            let crece = (paso as f32 + 1.0) * 2.0 * escala;
            p.rellenar_redondeado(
                RectF {
                    x: m - crece,
                    y: m - crece + desplome,
                    ancho: w + 2.0 * crece,
                    alto: h + 2.0 * crece,
                },
                radio + crece,
                Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: *alfa,
                },
            );
        }
        // La tarjeta y la imagen. La imagen va sin recorte redondeado en
        // S2-A (simplificacion consciente anotada en el plan): el redondeo
        // se aprecia en la sombra.
        p.rellenar_redondeado(
            RectF {
                x: m,
                y: m,
                ancho: w,
                alto: h,
            },
            radio,
            Color::BLANCO,
        );
        p.bitmap(
            &i.bitmap,
            RectF {
                x: m,
                y: m,
                ancho: w,
                alto: h,
            },
            None,
            false,
        );
    });
    let _ = i.superficie.presentar();
}

/// Aplica un efecto de la maquina pura sobre la ventana real.
fn aplicar(hwnd: HWND, efecto: EfectoPin) {
    let Some(i) = interno_de(hwnd) else { return };
    match efecto {
        EfectoPin::Nada => {}
        EfectoPin::Mover(contenido) => {
            let v = rect_ventana(contenido, i.escala_por_cien);
            // SAFETY: SetWindowPos sobre ventana propia; sin redibujar: la
            // composicion mueve el visual entero.
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    v.x,
                    v.y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }
        EfectoPin::Redimensionar(contenido) => {
            let v = rect_ventana(contenido, i.escala_por_cien);
            // SAFETY: igual que arriba, con tamano.
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    v.x,
                    v.y,
                    v.ancho as i32,
                    v.alto as i32,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            // La superficie es del tamano de la ventana: se recrea.
            if let Ok(s) = Superficie::nueva(&i.motor, &i.d3d, hwnd, v.ancho, v.alto) {
                i.superficie = s;
            }
            pintar(i);
        }
        EfectoPin::AlternarTamano => {
            let actual = i.estado.rect();
            let (nw, nh) = i.imagen_nativa;
            let al_natural = actual.ancho == nw && actual.alto == nh;
            let nuevo = if al_natural {
                // Ajustado: 80% del area del monitor bajo el pin no esta a
                // mano sin enumerar; media del nativo, con el minimo del
                // estado. Suficiente para S2-A y determinista.
                let minimo = MINIMO_LOGICO * i.escala_por_cien / 100;
                Rect {
                    x: actual.x,
                    y: actual.y,
                    ancho: (nw / 2).max(minimo),
                    alto: (nh / 2).max(minimo),
                }
            } else {
                Rect {
                    x: actual.x,
                    y: actual.y,
                    ancho: nw,
                    alto: nh,
                }
            };
            i.estado = EstadoPin::nuevo(nuevo, i.escala_por_cien);
            aplicar(hwnd, EfectoPin::Redimensionar(nuevo));
            if let Some(i2) = interno_de(hwnd) {
                (i2.al_cambiar)(CambioPin::Redimensionado(nuevo));
            }
        }
        EfectoPin::GestoTerminado(contenido) => {
            (i.al_cambiar)(CambioPin::Movido(contenido));
        }
        EfectoPin::Cerrar => {
            (i.al_cambiar)(CambioPin::Cerrado);
            // SAFETY: destruye la ventana propia; WM_NCDESTROY libera el Box.
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    }
}

extern "system" fn procedimiento_pin(
    hwnd: HWND,
    mensaje: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Cliente -> escritorio virtual, como en el overlay.
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
        WM_LBUTTONDOWN => {
            // SAFETY: captura para no perder el arrastre al salir del borde.
            unsafe { SetCapture(hwnd) };
            // Clic tambien enfoca: es lo que arma el Esc de D23.
            // SAFETY: foco sobre ventana propia.
            unsafe {
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(hwnd));
            }
            if let Some(i) = interno_de(hwnd) {
                let e = i.estado.procesar(EventoPin::BotonPulsado(punto(lparam)));
                aplicar(hwnd, e);
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if let Some(i) = interno_de(hwnd) {
                let e = i.estado.procesar(EventoPin::RatonMovido(punto(lparam)));
                aplicar(hwnd, e);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            // SAFETY: libera la captura tomada en el pulsado.
            unsafe {
                let _ = ReleaseCapture();
            }
            if let Some(i) = interno_de(hwnd) {
                let e = i.estado.procesar(EventoPin::BotonSoltado);
                aplicar(hwnd, e);
            }
            LRESULT(0)
        }
        WM_LBUTTONDBLCLK => {
            if let Some(i) = interno_de(hwnd) {
                let e = i.estado.procesar(EventoPin::DobleClic);
                aplicar(hwnd, e);
            }
            LRESULT(0)
        }
        WM_KEYDOWN if wparam.0 as u32 == VK_ESCAPE.0 as u32 => {
            if let Some(i) = interno_de(hwnd) {
                let e = i.estado.procesar(EventoPin::Escape);
                aplicar(hwnd, e);
            }
            LRESULT(0)
        }
        WM_SETCURSOR => {
            let diagonal = interno_de(hwnd)
                .map(|i| {
                    // La posicion del cursor en pantalla, preguntada aqui:
                    // WM_SETCURSOR no trae coordenadas utiles.
                    let mut p = windows::Win32::Foundation::POINT::default();
                    // SAFETY: GetCursorPos escribe en la variable local.
                    unsafe {
                        let _ = GetCursorPos(&mut p);
                    }
                    i.estado.sobre_esquina(Punto { x: p.x, y: p.y })
                })
                .unwrap_or(false);
            let id = if diagonal { IDC_SIZENWSE } else { IDC_SIZEALL };
            // SAFETY: cursores del sistema, llamadas sin precondiciones.
            unsafe {
                if let Ok(c) = LoadCursorW(None, id) {
                    SetCursor(Some(c));
                }
            }
            LRESULT(1)
        }
        WM_PAINT => {
            // SAFETY: ValidateRect marca pintado; el dibujo es del swapchain.
            unsafe {
                let _ = ValidateRect(Some(hwnd), None);
            }
            if let Some(i) = interno_de(hwnd) {
                pintar(i);
            }
            LRESULT(0)
        }
        WM_NCDESTROY => {
            // SAFETY: recupera el Box cedido en Pin::nuevo exactamente una
            // vez y deja el USERDATA a cero antes de soltarlo.
            unsafe {
                let crudo = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut PinInterno;
                if !crudo.is_null() {
                    drop(Box::from_raw(crudo));
                }
            }
            LRESULT(0)
        }
        // NUNCA PostQuitMessage: cerrar un pin no apaga la aplicacion.
        _ => {
            // SAFETY: delegacion estandar.
            unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_codec::ImagenRgba;
    use pixpin_geom::Rect;
    use pixpin_render::MotorRender;
    use std::cell::RefCell;
    use std::rc::Rc;
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
    };

    fn d3d() -> ID3D11Device {
        let mut d = None;
        // SAFETY: salidas locales, constantes documentadas (patron de
        // pixpin-render).
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                Default::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d),
                None,
                None,
            )
            .expect("GPU real");
        }
        d.unwrap()
    }

    fn imagen_2x2() -> ImagenRgba {
        ImagenRgba {
            ancho: 2,
            alto: 2,
            pixeles: vec![255; 16],
        }
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn el_pin_se_crea_visible_y_al_destruirse_no_mata_nada() {
        let d3d = d3d();
        let motor = Rc::new(MotorRender::nuevo(&d3d).unwrap());
        let cambios: Rc<RefCell<Vec<CambioPin>>> = Rc::new(RefCell::new(Vec::new()));
        let c = Rc::clone(&cambios);
        let pin = Pin::nuevo(
            &d3d,
            Rc::clone(&motor),
            &imagen_2x2(),
            Rect {
                x: 100,
                y: 100,
                ancho: 200,
                alto: 150,
            },
            100,
            Box::new(move |cambio| c.borrow_mut().push(cambio)),
        )
        .expect("el pin deberia crearse");

        // SAFETY: IsWindowVisible es consulta pura sobre handle vivo.
        let visible = unsafe { IsWindowVisible(pin.hwnd()).as_bool() };
        assert!(visible, "el pin nace visible (sin robar el foco)");
        assert_eq!(
            pin.rect_contenido(),
            Rect {
                x: 100,
                y: 100,
                ancho: 200,
                alto: 150
            }
        );

        // Dos pines: destruir uno no toca al otro (la mina de S1-A).
        let c2 = Rc::clone(&cambios);
        let pin2 = Pin::nuevo(
            &d3d,
            motor,
            &imagen_2x2(),
            Rect {
                x: 400,
                y: 100,
                ancho: 200,
                alto: 150,
            },
            100,
            Box::new(move |cambio| c2.borrow_mut().push(cambio)),
        )
        .unwrap();
        let hwnd2 = pin2.hwnd();
        drop(pin);
        // SAFETY: IsWindow consulta pura.
        let vivo2 = unsafe { IsWindow(Some(hwnd2)).as_bool() };
        assert!(vivo2, "destruir un pin no puede llevarse a los demas");
    }

    #[test]
    fn la_ventana_es_mayor_que_el_contenido_por_el_margen() {
        // Puro: la conversion contenido <-> ventana con margen de sombra.
        let contenido = Rect {
            x: 100,
            y: 100,
            ancho: 200,
            alto: 150,
        };
        let v = rect_ventana(contenido, 150);
        let margen = (MARGEN_SOMBRA_LOGICO * 150 / 100) as i32;
        assert_eq!(v.x, 100 - margen);
        assert_eq!(v.ancho, 200 + 2 * margen as u32);
        assert_eq!(
            contenido_desde_ventana(v, 150),
            contenido,
            "ida y vuelta exacta"
        );
    }
}
