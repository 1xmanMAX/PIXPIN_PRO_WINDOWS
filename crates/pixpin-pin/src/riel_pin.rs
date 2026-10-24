//! **El riel del pin** (19-oct-2026): el lapiz y el marco para imprimir en
//! una columna al lado del pin, con la misma cara que el riel del lector de
//! PDF (`lector::pintar_riel` de la app): la isla de cristal oscuro, los
//! mismos iconos y lo pulsado en el azul `#0060DF`.
//!
//! El usuario: «el lapiz, el marcador y el frame sean universales, que
//! funcionen en todos los formatos y salgan en un riel lateral como en Word
//! y PDF; los mismos iconos en todos los visores para no confundirse».
//!
//! Sin marcador: un pin es una sola imagen que se ve entera, no hay sitio al
//! que volver. Solo el lapiz (si el pin se anota) y el marco.
//!
//! Va FUERA del pin, a su derecha (a la izquierda si no cabe), como la
//! barra va encima: los pines son pequenos y sin marco, y por dentro taparia
//! lo que ensenan. Aparece y se esconde con la barra; mientras se anota se
//! queda, con el lapiz encendido, y pulsarlo otra vez sale de anotar.
//!
//! Una sola ventana para todos los pines, como la barra: no se activa nunca
//! y cada clic vuelve al pin dueno por `ventana::accion_de_riel`.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Once;

use pixpin_geom::Rect;
use pixpin_render::{Color, MotorRender, Pintor, RectF, Superficie};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Graphics::Gdi::ValidateRect;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

/// Lado de un boton, en pixeles logicos: el del riel del lector.
const LADO_BOTON: f32 = 36.0;
/// Relleno de la isla alrededor de sus botones.
const RELLENO: f32 = 4.0;
/// Lo que queda entre el pin y el riel: lo mismo que con la barra.
const SEPARACION: f32 = crate::barra::SEPARACION_LOGICA;

/// Los colores del riel del lector (`lector::CRISTAL`, `TEXTO`,
/// `ENCENDIDO`): el mismo riel en todos los visores.
const CRISTAL: Color = rgba(0x26, 0x26, 0x2c, 0.92);
const TEXTO: Color = rgba(0xe4, 0xe2, 0xe6, 1.0);
const ENCENDIDO: Color = rgba(0x00, 0x60, 0xdf, 1.0);
const ENCIMA: Color = rgba(255, 255, 255, 0.12);

/// Un boton del riel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotonRiel {
    /// Anotar encima (lo que hace el doble clic); anotando, salir.
    Lapiz,
    /// El marco para imprimir una zona del pin.
    Marco,
}

/// El riel ya dispuesto, en pixeles de su ventana.
#[derive(Debug, Clone, PartialEq)]
pub struct Disposicion {
    pub ancho: f32,
    pub alto: f32,
    pub botones: Vec<(BotonRiel, RectF)>,
}

/// Dispone el riel: el lapiz arriba (si el pin se anota) y el marco debajo,
/// en el orden del riel del lector.
pub fn disponer(escala: f32, anotable: bool) -> Disposicion {
    let e = escala.max(0.5);
    let lado = LADO_BOTON * e;
    let relleno = RELLENO * e;
    let mut botones = Vec::new();
    let mut y = relleno;
    for b in [BotonRiel::Lapiz, BotonRiel::Marco] {
        if b == BotonRiel::Lapiz && !anotable {
            continue;
        }
        botones.push((
            b,
            RectF {
                x: relleno,
                y,
                ancho: lado,
                alto: lado,
            },
        ));
        y += lado;
    }
    Disposicion {
        ancho: lado + 2.0 * relleno,
        alto: y + relleno,
        botones,
    }
}

impl Disposicion {
    /// El boton bajo un punto de la ventana del riel.
    pub fn boton_en(&self, x: f32, y: f32) -> Option<BotonRiel> {
        self.botones.iter().find_map(|(b, r)| {
            (x >= r.x && x < r.x + r.ancho && y >= r.y && y < r.y + r.alto).then_some(*b)
        })
    }
}

/// Donde va el riel (su esquina, pixeles fisicos) para un pin cuyo
/// contenido ocupa `pin`: a su derecha y alineado arriba; a la izquierda si
/// a la derecha se sale del monitor; y si tampoco, por dentro del borde
/// derecho. Nunca fuera del area de trabajo.
pub fn colocar(pin: Rect, trabajo: Rect, ancho: u32, alto: u32, separacion: i32) -> (i32, i32) {
    let derecha = pin.derecha() + separacion;
    let izquierda = pin.x - separacion - ancho as i32;
    let x = if derecha + ancho as i32 <= trabajo.derecha() {
        derecha
    } else if izquierda >= trabajo.x {
        izquierda
    } else {
        pin.derecha() - separacion - ancho as i32
    };
    let max_x = (trabajo.derecha() - ancho as i32).max(trabajo.x);
    let max_y = (trabajo.abajo() - alto as i32).max(trabajo.y);
    (x.clamp(trabajo.x, max_x), pin.y.clamp(trabajo.y, max_y))
}

struct Interno {
    motor: Rc<MotorRender>,
    superficie: Superficie,
    dueno: HWND,
    escala: f32,
    disposicion: Disposicion,
    /// Se esta anotando el pin dueno: el lapiz, encendido.
    anotando: bool,
    /// Se ponen marcos en el pin dueno: el marco, encendido.
    marcando: bool,
    encima: Option<BotonRiel>,
    /// La esquina de la isla, en pantalla (la ventana empieza `hueco.0`
    /// antes, si el globo va a su izquierda).
    origen: (i32, i32),
    /// El nombre y la tecla de cada boton, para su globo.
    pistas: Vec<(BotonRiel, String)>,
    /// Lo que crece la ventana a la izquierda y a la derecha de la isla
    /// mientras ensena un globo (0 sin el).
    hueco: (f32, f32),
    trabajo: Rect,
}

/// Lo que queda entre el globo y su boton, en pixeles logicos.
const AIRE_GLOBO: f32 = 8.0;
const TAM_GLOBO: f32 = 12.0;

/// **Donde va el globo** del boton bajo el raton (19-oct-2026, el usuario:
/// «que al pasar por un icono me aparezca su atajo»): a la izquierda de la
/// isla, como en el riel del lector; si ahi se sale del area de trabajo, a
/// su derecha. Devuelve lo que la ventana crece por cada lado para `ancho`
/// de globo (con su aire), con la isla en `x` y de `isla` de ancho.
pub fn hueco_del_globo(x: i32, isla: f32, ancho: f32, trabajo: Rect) -> (f32, f32) {
    if ancho <= 0.0 {
        (0.0, 0.0)
    } else if x as f32 - ancho >= trabajo.x as f32 {
        (ancho, 0.0)
    } else if x as f32 + isla + ancho <= trabajo.derecha() as f32 {
        (0.0, ancho)
    } else {
        (ancho, 0.0)
    }
}

impl Interno {
    fn texto_globo(&self) -> Option<&str> {
        let b = self.encima?;
        self.pistas
            .iter()
            .find(|(x, _)| *x == b)
            .map(|(_, t)| t.as_str())
            .filter(|t| !t.is_empty())
    }

    /// Lo que debe crecer la ventana para el globo de ahora.
    fn hueco_ahora(&self) -> (f32, f32) {
        let e = self.escala;
        let ancho = self.texto_globo().map_or(0.0, |t| {
            self.motor.medir_texto(t, TAM_GLOBO * e, f32::MAX).0 + (16.0 + AIRE_GLOBO + 2.0) * e
        });
        hueco_del_globo(
            self.origen.0,
            self.disposicion.ancho,
            ancho.ceil(),
            self.trabajo,
        )
    }
}

static REGISTRO: Once = Once::new();

thread_local! {
    static VENTANA: Cell<Option<HWND>> = const { Cell::new(None) };
}

fn interno_de<'a>(hwnd: HWND) -> Option<&'a mut Interno> {
    // SAFETY: el puntero lo pone `mostrar` y solo WM_NCDESTROY lo retira;
    // entre ambos es un Box valido. Todo ocurre en el hilo de interfaz.
    unsafe {
        let crudo = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Interno;
        crudo.as_mut()
    }
}

fn ventana() -> Option<HWND> {
    VENTANA.with(|v| v.get())
}

/// La ventana del riel, para las pruebas que la miran.
#[cfg(test)]
pub(crate) fn ventana_de_prueba() -> Option<HWND> {
    ventana()
}

/// Como si el raton estuviera sobre `boton` (sin el raton de verdad: las
/// pruebas no mueven el cursor del usuario).
#[cfg(test)]
pub(crate) fn encima_de_prueba(boton: Option<BotonRiel>) {
    let Some(h) = ventana() else { return };
    if let Some(i) = interno_de(h) {
        i.encima = boton;
        mirar_globo(h, i);
    }
}

fn registrar_clase() {
    // SAFETY: registro unico (Once) de una clase con WndProc propio.
    unsafe {
        let clase = WNDCLASSW {
            lpfnWndProc: Some(procedimiento),
            hInstance: GetModuleHandleW(None).expect("modulo propio").into(),
            lpszClassName: w!("PixPinRielPin"),
            ..Default::default()
        };
        RegisterClassW(&clase);
    }
}

fn tamano(d: &Disposicion) -> (u32, u32) {
    (d.ancho.ceil() as u32, d.alto.ceil() as u32)
}

fn recolocar_ventana(hwnd: HWND, i: &Interno) {
    let (ancho, alto) = tamano(&i.disposicion);
    let ancho = ancho + (i.hueco.0 + i.hueco.1).ceil() as u32;
    if let Err(e) = i.superficie.redimensionar(ancho, alto) {
        tracing::warn!(?e, "no se pudo redimensionar el riel del pin");
        return;
    }
    // SAFETY: mover y ensenar una ventana propia sin activarla.
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            i.origen.0 - i.hueco.0.ceil() as i32,
            i.origen.1,
            ancho as i32,
            alto as i32,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
    pintar(i);
}

/// Ensena el riel del pin `dueno`, cuyo contenido ocupa `contenido`.
#[allow(clippy::too_many_arguments)] // todo lo que el riel necesita al aparecer
pub(crate) fn mostrar(
    d3d: &ID3D11Device,
    motor: &Rc<MotorRender>,
    dueno: HWND,
    contenido: Rect,
    trabajo: Rect,
    escala_por_cien: u32,
    anotable: bool,
    anotando: bool,
    marcando: bool,
    pistas: Vec<(BotonRiel, String)>,
) {
    // Mientras se anota un pin el riel es suyo: pasar el raton por otro pin
    // no se lo lleva (perderia el lapiz con el que se sale).
    if !anotando
        && let Some(i) = ventana().and_then(interno_de)
        && i.anotando
        && i.dueno != dueno
        && ventana().is_some_and(visible)
    {
        return;
    }
    let escala = escala_por_cien as f32 / 100.0;
    let disposicion = disponer(escala, anotable);
    let (ancho, alto) = tamano(&disposicion);
    let (x, y) = colocar(
        contenido,
        trabajo,
        ancho,
        alto,
        (SEPARACION * escala) as i32,
    );
    let hwnd = match ventana() {
        Some(h) => h,
        None => {
            REGISTRO.call_once(registrar_clase);
            // SAFETY: clase registrada; estilos documentados; se crea una
            // vez por hilo y no se destruye: se esconde.
            let h = unsafe {
                CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    w!("PixPinRielPin"),
                    w!(""),
                    WS_POPUP,
                    x,
                    y,
                    ancho as i32,
                    alto as i32,
                    None,
                    None,
                    GetModuleHandleW(None).ok().map(|m| m.into()),
                    None,
                )
            };
            let Ok(h) = h else {
                tracing::warn!("no se pudo crear el riel del pin");
                return;
            };
            let superficie = match Superficie::nueva(motor, d3d, h, ancho, alto) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(?e, "sin superficie para el riel del pin");
                    // SAFETY: ventana propia recien creada.
                    unsafe {
                        let _ = DestroyWindow(h);
                    }
                    return;
                }
            };
            let interno = Box::new(Interno {
                motor: Rc::clone(motor),
                superficie,
                dueno,
                escala,
                disposicion: disposicion.clone(),
                anotando,
                marcando,
                encima: None,
                origen: (x, y),
                pistas: Vec::new(),
                hueco: (0.0, 0.0),
                trabajo,
            });
            // SAFETY: el Box se cede al USERDATA y se recupera una vez en
            // WM_NCDESTROY.
            unsafe {
                SetWindowLongPtrW(h, GWLP_USERDATA, Box::into_raw(interno) as isize);
            }
            VENTANA.with(|v| v.set(Some(h)));
            h
        }
    };
    let Some(i) = interno_de(hwnd) else { return };
    if i.dueno != dueno {
        i.encima = None;
    }
    i.dueno = dueno;
    i.escala = escala;
    i.disposicion = disposicion;
    i.anotando = anotando;
    i.marcando = marcando;
    i.origen = (x, y);
    i.pistas = pistas;
    i.trabajo = trabajo;
    i.hueco = i.hueco_ahora();
    recolocar_ventana(hwnd, i);
}

/// Lo vuelve a poner junto al pin, que se ha movido o cambiado de tamano.
pub(crate) fn recolocar(dueno: HWND, contenido: Rect, trabajo: Rect) {
    let Some(h) = ventana() else { return };
    let Some(i) = interno_de(h) else { return };
    if i.dueno != dueno || !visible(h) {
        return;
    }
    let (ancho, alto) = tamano(&i.disposicion);
    let xy = colocar(
        contenido,
        trabajo,
        ancho,
        alto,
        (SEPARACION * i.escala) as i32,
    );
    if xy != i.origen {
        i.origen = xy;
        i.trabajo = trabajo;
        i.hueco = i.hueco_ahora();
        recolocar_ventana(h, i);
    }
}

fn visible(h: HWND) -> bool {
    // SAFETY: consulta pura sobre una ventana propia.
    unsafe { IsWindowVisible(h) }.as_bool()
}

/// La destruye (si existe): su superficie es del dispositivo grafico.
pub(crate) fn soltar() {
    if let Some(h) = ventana() {
        // SAFETY: ventana propia de este hilo; WM_NCDESTROY suelta su
        // estado y borra `VENTANA`.
        unsafe {
            let _ = DestroyWindow(h);
        }
        VENTANA.with(|v| v.set(None));
    }
}

/// Enciende o apaga el marco del riel (el modo de los marcos del pin
/// `dueno`), si el riel es suyo.
pub(crate) fn poner_marcando(dueno: HWND, marcando: bool) {
    let Some(h) = ventana() else { return };
    if let Some(i) = interno_de(h).filter(|i| i.dueno == dueno)
        && i.marcando != marcando
    {
        i.marcando = marcando;
        pintar(i);
    }
}

/// Lo esconde, si es de `dueno`.
pub(crate) fn esconder(dueno: HWND) {
    let Some(h) = ventana() else { return };
    if let Some(i) = interno_de(h).filter(|i| i.dueno == dueno) {
        i.encima = None;
        i.anotando = false;
        i.marcando = false;
        i.hueco = (0.0, 0.0);
        // SAFETY: esconder una ventana propia.
        unsafe {
            let _ = ShowWindow(h, SW_HIDE);
        }
    }
}

fn rect_ventana(h: HWND) -> RECT {
    let mut r = RECT::default();
    // SAFETY: GetWindowRect sobre una ventana propia.
    unsafe {
        let _ = GetWindowRect(h, &mut r);
    }
    r
}

/// Si el cursor (pantalla) esta encima del riel.
pub(crate) fn contiene(x: i32, y: i32) -> bool {
    let Some(h) = ventana().filter(|h| visible(*h)) else {
        return false;
    };
    let r = rect_ventana(h);
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

/// Si el cursor esta en el hueco entre el pin y su riel: pasar del uno al
/// otro lo cruza, y el riel no puede esconderse a medio camino.
pub(crate) fn en_el_paso(pin: Rect, x: i32, y: i32) -> bool {
    let Some(h) = ventana().filter(|h| visible(*h)) else {
        return false;
    };
    let r = rect_ventana(h);
    // Solo la franja a la altura del riel: el hueco de al lado, no todo el
    // rectangulo que abarcan los dos.
    let (l, d) = (r.left.min(pin.x), r.right.max(pin.derecha()));
    x >= l && x < d && y >= r.top && y < r.bottom
}

const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a,
    }
}

fn pintar(i: &Interno) {
    let Ok(destino) = i.superficie.empezar(&i.motor) else {
        return;
    };
    let _ = i.motor.dibujar(&destino, |p| {
        p.limpiar_transparente();
        pintar_riel(p, i);
    });
    let _ = i.superficie.presentar();
}

/// Lo mismo que pinta `lector::pintar_riel` para su isla.
fn pintar_riel(p: &Pintor, i: &Interno) {
    use pixpin_render::icono::material::{CROP_FREE, EDIT};
    let e = i.escala;
    let d = &i.disposicion;
    // Con el globo a la izquierda la isla empieza mas a la derecha.
    p.desplazar(i.hueco.0.ceil(), 0.0);
    p.rellenar_redondeado(
        RectF {
            x: 0.0,
            y: 0.0,
            ancho: d.ancho,
            alto: d.alto,
        },
        8.0 * e,
        CRISTAL,
    );
    for (b, r) in &d.botones {
        let activo =
            (*b == BotonRiel::Lapiz && i.anotando) || (*b == BotonRiel::Marco && i.marcando);
        if activo {
            p.rellenar_redondeado(*r, 6.0 * e, ENCENDIDO);
        } else if i.encima == Some(*b) {
            p.rellenar_redondeado(*r, 6.0 * e, ENCIMA);
        }
        let lado = 20.0 * e;
        let icono = match b {
            BotonRiel::Lapiz => &EDIT,
            BotonRiel::Marco => &CROP_FREE,
        };
        p.icono(
            icono,
            RectF {
                x: r.x + (r.ancho - lado) / 2.0,
                y: r.y + (r.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            if activo { Color::BLANCO } else { TEXTO },
        );
    }
    // El globo del boton bajo el raton: su nombre y su tecla, como el de
    // `lector::globo_a_la_izquierda` (a la derecha si a la izquierda no
    // cabe en la pantalla).
    if let (Some(texto), Some(b)) = (i.texto_globo(), i.encima)
        && let Some((_, r)) = d.botones.iter().find(|(x, _)| *x == b)
        && i.hueco != (0.0, 0.0)
    {
        let tam = TAM_GLOBO * e;
        let (w, h) = p.medir_texto(texto, tam);
        let (px, py) = (8.0 * e, 5.0 * e);
        let ancho = w + 2.0 * px;
        let x = if i.hueco.0 > 0.0 {
            -AIRE_GLOBO * e - ancho
        } else {
            d.ancho + AIRE_GLOBO * e
        };
        let caja = RectF {
            x,
            y: r.y + (r.alto - h - 2.0 * py) / 2.0,
            ancho,
            alto: h + 2.0 * py,
        };
        p.rellenar_redondeado(caja, 6.0 * e, GLOBO);
        p.texto(texto, caja.x + px, caja.y + py, tam, Color::BLANCO);
    }
}

/// El fondo del globo (el de `lector::globo_a_la_izquierda`).
const GLOBO: Color = Color {
    r: 0.106,
    g: 0.106,
    b: 0.122,
    a: 0.96,
};

/// Lo que tiene el raton encima cambio: el globo nuevo (la ventana crece o
/// encoge para el) y se repinta.
fn mirar_globo(hwnd: HWND, i: &mut Interno) {
    let hueco = i.hueco_ahora();
    if hueco != i.hueco {
        i.hueco = hueco;
        recolocar_ventana(hwnd, i);
    } else {
        pintar(i);
    }
}

/// El punto del raton en la isla (sin lo que la ventana crecio a la
/// izquierda para el globo).
fn local(lparam: LPARAM, izquierda: f32) -> (f32, f32) {
    (
        (lparam.0 & 0xFFFF) as i16 as f32 - izquierda.ceil(),
        ((lparam.0 >> 16) & 0xFFFF) as i16 as f32,
    )
}

extern "system" fn procedimiento(
    hwnd: HWND,
    mensaje: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    const WM_RATON_FUERA: u32 = 0x02A3;
    match mensaje {
        // Ni el clic lo activa: el teclado sigue en el pin.
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_MOUSEMOVE => {
            let Some(i) = interno_de(hwnd) else {
                return LRESULT(0);
            };
            let (x, y) = local(lparam, i.hueco.0);
            let encima = i.disposicion.boton_en(x, y);
            if encima != i.encima {
                i.encima = encima;
                mirar_globo(hwnd, i);
                let mut seguir = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                // SAFETY: estructura completa sobre la ventana propia.
                unsafe {
                    let _ = TrackMouseEvent(&mut seguir);
                }
            }
            LRESULT(0)
        }
        m if m == WM_RATON_FUERA => {
            if let Some(i) = interno_de(hwnd)
                && i.encima.is_some()
            {
                i.encima = None;
                mirar_globo(hwnd, i);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let Some(i) = interno_de(hwnd) else {
                return LRESULT(0);
            };
            let (x, y) = local(lparam, i.hueco.0);
            if let Some(b) = i.disposicion.boton_en(x, y) {
                let (dueno, anotando) = (i.dueno, i.anotando);
                // Sin prestamos vivos: el pin puede abrir algo modal (el
                // marco), que bombea mensajes y vuelve a entrar aqui.
                crate::ventana::accion_de_riel(dueno, b, anotando);
            }
            LRESULT(0)
        }
        WM_PAINT => {
            // SAFETY: ValidateRect marca la ventana como pintada; el dibujo
            // real lo hace el swapchain.
            unsafe {
                let _ = ValidateRect(Some(hwnd), None);
            }
            if let Some(i) = interno_de(hwnd) {
                pintar(i);
            }
            LRESULT(0)
        }
        WM_NCDESTROY => {
            VENTANA.with(|v| v.set(None));
            // SAFETY: recupera el Box cedido en `mostrar` una sola vez.
            unsafe {
                let crudo = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut Interno;
                if !crudo.is_null() {
                    drop(Box::from_raw(crudo));
                }
            }
            LRESULT(0)
        }
        // NUNCA PostQuitMessage.
        // SAFETY: delegacion estandar.
        _ => unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) },
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const TRABAJO: Rect = Rect {
        x: 0,
        y: 0,
        ancho: 1920,
        alto: 1040,
    };

    #[test]
    fn el_lapiz_va_encima_del_marco_como_en_el_lector() {
        let d = disponer(1.0, true);
        let orden: Vec<_> = d.botones.iter().map(|(b, _)| *b).collect();
        assert_eq!(orden, vec![BotonRiel::Lapiz, BotonRiel::Marco]);
        assert_eq!(d.ancho, 44.0);
        assert_eq!(d.alto, 80.0);
        assert_eq!(d.boton_en(22.0, 20.0), Some(BotonRiel::Lapiz));
        assert_eq!(d.boton_en(22.0, 60.0), Some(BotonRiel::Marco));
        assert_eq!(d.boton_en(1.0, 1.0), None, "el relleno no es de nadie");
    }

    #[test]
    fn caso_negativo_lo_que_no_se_anota_solo_lleva_el_marco() {
        let d = disponer(1.5, false);
        assert_eq!(d.botones.len(), 1);
        assert_eq!(d.botones[0].0, BotonRiel::Marco);
    }

    #[test]
    fn el_globo_va_a_la_izquierda_y_si_no_cabe_a_la_derecha() {
        // La isla en x = 408 (a la derecha de un pin): el globo, a su izquierda.
        assert_eq!(hueco_del_globo(408, 44.0, 150.0, TRABAJO), (150.0, 0.0));
        // Pegada al borde izquierdo de la pantalla: a su derecha.
        assert_eq!(hueco_del_globo(20, 44.0, 150.0, TRABAJO), (0.0, 150.0));
        // Caso negativo: sin globo (ningun boton bajo el raton), no crece.
        assert_eq!(hueco_del_globo(408, 44.0, 0.0, TRABAJO), (0.0, 0.0));
    }

    #[test]
    fn va_a_la_derecha_y_si_no_cabe_a_la_izquierda() {
        let pin = Rect {
            x: 100,
            y: 200,
            ancho: 300,
            alto: 200,
        };
        assert_eq!(colocar(pin, TRABAJO, 44, 80, 8), (408, 200));
        let al_borde = Rect {
            x: 1700,
            y: 200,
            ancho: 220,
            alto: 200,
        };
        assert_eq!(colocar(al_borde, TRABAJO, 44, 80, 8), (1700 - 8 - 44, 200));
        // Un pin que llena el monitor: por dentro, contra su borde derecho.
        let entero = Rect {
            x: 0,
            y: -10,
            ancho: 1920,
            alto: 1040,
        };
        let (x, y) = colocar(entero, TRABAJO, 44, 80, 8);
        assert_eq!(x, 1920 - 8 - 44);
        assert_eq!(y, 0, "nunca por encima del area de trabajo");
    }
}
