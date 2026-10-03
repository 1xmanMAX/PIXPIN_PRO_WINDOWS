//! **La barra que sale encima de lo elegido** (H12, 30-sep), como la del
//! editor de documentos de Claude (capturas del usuario): comentar, emoji,
//! «Aa ▾» (titulo 1 a 3 o texto normal), negrita, cursiva, tachado,
//! codigo, quitar formato, «lista ▾» (vinetas, numerada, casillas) y
//! enlace. **Sin subrayado**: Markdown no tiene marca para subrayar y el
//! `Markdown.kt` del movil no entiende `<u>` (su `Formato.kt` lo dice:
//! «meter `<u>` seria inventarse una marca que nadie escribe a mano»).
//!
//! Es una ventana aparte que **no se queda con el foco**
//! (`WS_EX_NOACTIVATE`, como los menus): se sigue escribiendo en la nota y
//! lo elegido sigue elegido. La pinta el mismo pintor que el marco, con los
//! colores del tema (oscuro o claro). La parte de cuentas ([`disponer`],
//! [`colocar`]) no toca ventanas y se prueba sola.

use std::cell::{Cell, RefCell};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::disposicion::Caja;
use crate::pintor::{Icono, Pintor};
use crate::tema::Tema;

/// Los textos de la barra y de lo que abre, ya traducidos.
#[derive(Debug, Clone, Default)]
pub struct RotulosBarra {
    pub texto_normal: String,
    pub quitar_formato: String,
    /// La pista del cuadro del enlace: «Pega o escribe un enlace».
    pub enlace_pista: String,
    /// Los nombres de los emojis del selector, separados por `|`.
    pub emojis: String,
    pub letra_texto: String,
    pub letra_titulos: String,
    pub tamano: String,
    /// «Pequena|Normal|Grande|Muy grande».
    pub tamanos: String,
    pub solo_esta_nota: String,
    /// La pista del menu del tamano: que con Ctrl+rueda hay mas.
    pub pista_tamano: String,
}

/// Lo que se puede pulsar en la barra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BotonBarra {
    Comentar,
    Emoji,
    /// «Aa ▾»: titulo o texto normal.
    Formato,
    Negrita,
    Cursiva,
    Tachado,
    Codigo,
    QuitarFormato,
    /// «lista ▾»: vinetas, numerada, casillas.
    Listas,
    Enlace,
}

/// Los botones en orden, con un `None` donde va una raya de separar (los
/// grupos de la captura).
pub const BOTONES: [Option<BotonBarra>; 13] = [
    Some(BotonBarra::Comentar),
    Some(BotonBarra::Emoji),
    None,
    Some(BotonBarra::Formato),
    None,
    Some(BotonBarra::Negrita),
    Some(BotonBarra::Cursiva),
    Some(BotonBarra::Tachado),
    Some(BotonBarra::Codigo),
    Some(BotonBarra::QuitarFormato),
    None,
    Some(BotonBarra::Listas),
    Some(BotonBarra::Enlace),
];

const ALTO: i32 = 38;
const AIRE: i32 = 4;
const BOTON: i32 = 30;
const SEPARADOR: i32 = 9;

/// La barra colocada: cada boton con su caja (dentro de la barra), las
/// rayas de separar y lo que mide entera.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Disposicion {
    pub botones: Vec<(BotonBarra, Caja)>,
    pub rayas: Vec<Caja>,
    pub an: i32,
    pub al: i32,
}

impl Disposicion {
    pub fn boton_en(&self, x: i32, y: i32) -> Option<BotonBarra> {
        self.botones.iter().find(|(_, c)| c.contiene(x, y)).map(|(b, _)| *b)
    }

    pub fn caja(&self, b: BotonBarra) -> Option<Caja> {
        self.botones.iter().find(|(x, _)| *x == b).map(|(_, c)| *c)
    }
}

/// Coloca los botones de la barra a la `escala` de la pantalla.
pub fn disponer(escala: f32) -> Disposicion {
    let e = |v: i32| (v as f32 * escala).round() as i32;
    let mut d = Disposicion {
        al: e(ALTO),
        ..Default::default()
    };
    let mut x = e(AIRE);
    for b in BOTONES {
        match b {
            Some(b) => {
                let an = match b {
                    BotonBarra::Formato => e(46),
                    BotonBarra::Listas => e(42),
                    _ => e(BOTON),
                };
                d.botones.push((
                    b,
                    Caja {
                        x,
                        y: e(AIRE),
                        an,
                        al: d.al - 2 * e(AIRE),
                    },
                ));
                x += an + e(2);
            }
            None => {
                let medio = x + e(SEPARADOR) / 2 - e(1);
                d.rayas.push(Caja {
                    x: medio,
                    y: e(AIRE + 6),
                    an: e(1).max(1),
                    al: d.al - 2 * e(AIRE + 6),
                });
                x += e(SEPARADOR);
            }
        }
    }
    d.an = x - e(2) + e(AIRE);
    d
}

/// Donde sale la barra (en la pantalla): empezando un poco antes del
/// principio de lo elegido (como en Claude) y por encima, con un poco de aire; si arriba no cabe (lo elegido
/// esta en lo alto de la nota), debajo de su ultimo renglon. Siempre dentro
/// de `zona` (lo que se ve de la nota) de lado.
pub fn colocar(
    principio: (i32, i32),
    abajo: i32,
    tamano: (i32, i32),
    zona: (i32, i32, i32, i32),
    aire: i32,
) -> (i32, i32) {
    let (zx, zy, zan, zal) = zona;
    let x = (principio.0 - 2 * aire).min(zx + zan - tamano.0).max(zx);
    let arriba = principio.1 - aire - tamano.1;
    let y = if arriba >= zy {
        arriba
    } else {
        (abajo + aire).min(zy + zal - tamano.1).max(zy)
    };
    (x, y)
}

// ---------------------------------------------------------------------------
// La ventana

/// Lo que la barra necesita para pintarse (lo lee su procedimiento).
pub struct Vista {
    pub disp: Disposicion,
    pub tema: Tema,
    pub pintor: std::rc::Rc<Pintor>,
    pub hover: Option<BotonBarra>,
    /// Los formatos que ya tiene lo elegido (el boton se ve pulsado).
    pub puestos: Vec<BotonBarra>,
}

thread_local! {
    pub static VISTA: RefCell<Option<Vista>> = const { RefCell::new(None) };
    /// Lo que se pulso, para el bucle del editor.
    static CLIC: Cell<Option<BotonBarra>> = const { Cell::new(None) };
}

/// Lo pulsado desde la ultima vez, si algo.
pub fn tomar_clic() -> Option<BotonBarra> {
    CLIC.with(|c| c.take())
}

/// Pinta la barra en `hdc` con su esquina en `origen` (la ventana, o una
/// muestra en PNG).
pub fn pintar(hdc: HDC, v: &Vista, origen: (i32, i32)) {
    let esc = v.pintor.escala;
    let e = |x: i32| (x as f32 * esc).round() as i32;
    let t = &v.tema;
    let en = |c: Caja| Caja {
        x: c.x + origen.0,
        y: c.y + origen.1,
        ..c
    };
    let caja = Caja {
        x: origen.0,
        y: origen.1,
        an: v.disp.an,
        al: v.disp.al,
    };
    let zona = RECT {
        left: caja.x,
        top: caja.y,
        right: caja.derecha(),
        bottom: caja.abajo(),
    };
    v.pintor.formas(hdc, zona, |f| {
        f.redondo(caja, 10.0, t.menu);
        f.borde(caja, 10.0, 1.0, t.raya);
        for r in &v.disp.rayas {
            f.rect(en(*r), t.raya);
        }
        for (b, c) in &v.disp.botones {
            let c = en(*c);
            if v.hover == Some(*b) || v.puestos.contains(b) {
                f.redondo(c, 6.0, if v.puestos.contains(b) { t.elegido } else { t.pastilla });
            }
            match b {
                BotonBarra::Comentar => f.icono(Icono::Comentar, c, 16.0, t.texto),
                BotonBarra::Codigo => f.icono(Icono::Codigo, c, 16.0, t.texto),
                BotonBarra::Enlace => f.icono(Icono::Enlace, c, 16.0, t.texto),
                BotonBarra::Listas => {
                    f.icono(Icono::Lista, Caja { an: c.an - e(12), ..c }, 16.0, t.texto);
                    f.icono(Icono::Flecha, Caja { x: c.derecha() - e(16), an: e(14), ..c }, 10.0, t.tenue);
                }
                BotonBarra::Formato => {
                    f.icono(Icono::Flecha, Caja { x: c.derecha() - e(16), an: e(14), ..c }, 10.0, t.tenue);
                }
                // La raya del tachado, encima de su «S» (que va con las letras).
                BotonBarra::Tachado => {
                    let y = c.y + c.al / 2;
                    f.raya(c.x + e(9), y, c.derecha() - e(9), y, t.texto);
                }
                // Quitar formato: una «T» tachada en diagonal.
                BotonBarra::QuitarFormato => {
                    f.raya(c.x + e(9), c.abajo() - e(9), c.derecha() - e(9), c.y + e(9), t.tenue);
                }
                _ => {}
            }
        }
    });
    let p = &v.pintor;
    for (b, c) in &v.disp.botones {
        let c = en(*c);
        match b {
            BotonBarra::Emoji => p.texto(hdc, p.letra, "\u{263A}", c, t.texto, true),
            BotonBarra::Formato => p.texto(hdc, p.letra, "Aa", Caja { an: c.an - e(12), ..c }, t.texto, true),
            BotonBarra::Negrita => p.texto(hdc, p.letra_negrita, "B", c, t.texto, true),
            BotonBarra::Cursiva => cursiva(hdc, p, c, t),
            BotonBarra::Tachado => p.texto(hdc, p.letra, "S", c, t.texto, true),
            BotonBarra::QuitarFormato => p.texto(hdc, p.letra, "T", c, t.texto, true),
            _ => {}
        }
    }
}

/// La «I» de la cursiva, en cursiva de verdad.
fn cursiva(hdc: HDC, p: &Pintor, c: Caja, t: &Tema) {
    let alto = (13.5 * p.escala).round() as i32;
    // SAFETY: letra propia, creada y soltada aqui.
    unsafe {
        let letra = CreateFontW(
            -alto,
            0,
            0,
            0,
            500,
            1,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            w!("Georgia"),
        );
        p.texto(hdc, letra, "I", c, t.texto, true);
        let _ = DeleteObject(HGDIOBJ(letra.0));
    }
}

unsafe extern "system" fn procedimiento(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    let punto = || ((l.0 & 0xffff) as i16 as i32, ((l.0 >> 16) & 0xffff) as i16 as i32);
    match m {
        // Que un clic no le quite el foco (ni lo elegido) a la nota.
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            // SAFETY: pintado de una ventana propia, entre Begin y End.
            unsafe {
                let dc = BeginPaint(h, &mut ps);
                VISTA.with(|v| {
                    if let Some(v) = v.borrow().as_ref() {
                        pintar(dc, v, (0, 0));
                    }
                });
                let _ = EndPaint(h, &ps);
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE | WM_MOUSELEAVE => {
            let (x, y) = punto();
            let cambio = VISTA.with(|v| {
                let mut v = v.borrow_mut();
                let v = v.as_mut()?;
                let b = if m == WM_MOUSELEAVE { None } else { v.disp.boton_en(x, y) };
                (v.hover != b).then(|| v.hover = b)
            });
            if m == WM_MOUSEMOVE {
                let mut t = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: h,
                    dwHoverTime: 0,
                };
                // SAFETY: estructura local; ventana propia.
                unsafe {
                    let _ = TrackMouseEvent(&mut t);
                }
            }
            if cambio.is_some() {
                // SAFETY: ventana propia.
                unsafe {
                    let _ = InvalidateRect(Some(h), None, false);
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let (x, y) = punto();
            if let Some(b) = VISTA.with(|v| v.borrow().as_ref().and_then(|v| v.disp.boton_en(x, y))) {
                CLIC.with(|c| c.set(Some(b)));
                // Que el bucle del editor se entere ya (no hay otro mensaje
                // en su cola hasta que se mueva algo).
                // SAFETY: mensaje a la ventana duena, de este hilo.
                unsafe {
                    if let Ok(dueno) = GetWindow(h, GW_OWNER) {
                        let _ = PostMessageW(Some(dueno), WM_NULL, WPARAM(0), LPARAM(0));
                    }
                }
            }
            LRESULT(0)
        }
        // SAFETY: lo demas, a Windows.
        _ => unsafe { DefWindowProcW(h, m, w, l) },
    }
}

const WM_MOUSELEAVE: u32 = 0x02A3;

/// Crea la ventana de la barra (oculta), flotante sobre `dueno`.
pub fn crear(dueno: HWND) -> windows::core::Result<HWND> {
    // SAFETY: clase y ventana de este hilo; registrar dos veces falla sin dano.
    unsafe {
        let instancia = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?;
        let clase = WNDCLASSW {
            style: CS_DROPSHADOW,
            lpfnWndProc: Some(procedimiento),
            hInstance: instancia.into(),
            lpszClassName: w!("PixPinNotaBarraFlotante"),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&clase);
        let h = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("PixPinNotaBarraFlotante"),
            w!(""),
            WS_POPUP,
            0,
            0,
            10,
            10,
            Some(dueno),
            None,
            Some(instancia.into()),
            None,
        )?;
        let redondo: i32 = 2;
        let _ = windows::Win32::Graphics::Dwm::DwmSetWindowAttribute(
            h,
            windows::Win32::Graphics::Dwm::DWMWA_WINDOW_CORNER_PREFERENCE,
            &redondo as *const _ as *const _,
            4,
        );
        Ok(h)
    }
}

/// La ensena en `(x, y)` de la pantalla, sin quitar el foco.
pub fn ensenar(h: HWND, x: i32, y: i32, an: i32, al: i32) {
    // SAFETY: ventana propia.
    unsafe {
        let _ = SetWindowPos(h, Some(HWND_TOP), x, y, an, al, SWP_NOACTIVATE | SWP_SHOWWINDOW);
        let _ = InvalidateRect(Some(h), None, false);
    }
}

pub fn esconder(h: HWND) {
    // SAFETY: ventana propia.
    unsafe {
        let _ = ShowWindow(h, SW_HIDE);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_barra_lleva_los_botones_de_la_captura_y_sin_subrayado() {
        let d = disponer(1.0);
        let orden: Vec<BotonBarra> = d.botones.iter().map(|(b, _)| *b).collect();
        assert_eq!(orden.first(), Some(&BotonBarra::Comentar));
        assert_eq!(orden.last(), Some(&BotonBarra::Enlace));
        assert_eq!(orden.len(), 10);
        assert_eq!(d.rayas.len(), 3);
        // Ningun boton se pisa con el de al lado.
        for w in d.botones.windows(2) {
            assert!(w[0].1.derecha() <= w[1].1.x);
        }
        assert!(d.botones.last().unwrap().1.derecha() <= d.an);
    }

    #[test]
    fn un_clic_cae_en_su_boton_y_entre_botones_en_ninguno() {
        let d = disponer(1.0);
        let c = d.caja(BotonBarra::Negrita).unwrap();
        assert_eq!(d.boton_en(c.x + 2, c.y + 2), Some(BotonBarra::Negrita));
        let raya = d.rayas[0];
        assert_eq!(d.boton_en(raya.x, raya.y + 2), None);
        assert_eq!(d.boton_en(-5, 10), None);
    }

    #[test]
    fn a_doble_escala_todo_mide_el_doble() {
        let (a, b) = (disponer(1.0), disponer(2.0));
        assert!((b.an - 2 * a.an).abs() <= 4);
        assert_eq!(b.al, 2 * a.al);
    }

    #[test]
    fn sale_encima_de_lo_elegido_y_debajo_si_arriba_no_cabe() {
        let zona = (0, 100, 1000, 700);
        // Encima, empezando un poco antes de lo elegido.
        assert_eq!(colocar((500, 400), 430, (300, 38), zona, 8), (484, 354));
        // Arriba del todo: debajo del ultimo renglon elegido.
        assert_eq!(colocar((500, 120), 150, (300, 38), zona, 8), (484, 158));
        // Pegada al borde: no se sale de lado.
        assert_eq!(colocar((10, 400), 430, (300, 38), zona, 8).0, 0);
        assert_eq!(colocar((990, 400), 430, (300, 38), zona, 8).0, 700);
    }
}
