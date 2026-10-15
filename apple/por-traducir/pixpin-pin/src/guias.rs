//! Las guias para alinear un pin con los demas mientras se arrastra.
//!
//! Mientras se mueve un pin, si uno de sus bordes queda a tiro del de otro
//! pin (el mismo borde, o pegado a el con la separacion de 16 px) aparece
//! una linea azul que lo dice. Al SOLTAR se pega a esa guia. Durante el
//! arrastre no se pega: el iman de los bordes de la pantalla ya aprendio
//! que pegarse a media pasada pelea con el raton y se siente como un tiron.
//! `Alt` mientras se arrastra = sin guias ni iman.
//!
//! La cuenta es pura y se prueba aparte; las lineas son dos ventanitas
//! macizas de 2 px que no se activan ni recogen el raton.

use std::cell::Cell;
use std::sync::Once;

use pixpin_geom::Rect;

/// A cuanto se nota una guia, en pixeles logicos.
pub const UMBRAL_LOGICO: i32 = 8;
/// La separacion entre pines que se ofrece pegar, en pixeles logicos.
pub const SEPARACION_LOGICA: i32 = 16;

/// Una linea de guia, en pixeles fisicos del escritorio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Guia {
    /// Vertical (alinea en x) u horizontal (alinea en y).
    pub vertical: bool,
    /// La x de una vertical o la y de una horizontal.
    pub pos: i32,
    /// De donde a donde va, en el otro eje.
    pub desde: i32,
    pub hasta: i32,
}

/// Donde quedaria el pin pegado a sus guias, y las guias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ajuste {
    pub rect: Rect,
    pub guias: Vec<Guia>,
}

/// Un candidato en un eje: cuanto hay que mover y la guia que lo dice.
fn mejor(candidatos: impl Iterator<Item = (i32, Guia)>, umbral: i32) -> Option<(i32, Guia)> {
    candidatos
        .filter(|(d, _)| d.abs() <= umbral)
        .min_by_key(|(d, _)| d.abs())
}

/// Busca las guias de `rect` frente a `otros`: en cada eje, el borde mas
/// cercano a menos de `umbral`. Dos clases de guia: el mismo borde (izquierda
/// con izquierda, arriba con arriba...) o pegado al de al lado con
/// `separacion` en medio.
pub fn alinear(rect: Rect, otros: &[Rect], umbral: i32, separacion: i32) -> Ajuste {
    let (l, r, t, b) = (rect.x, rect.derecha(), rect.y, rect.abajo());
    let en_x = otros.iter().flat_map(|o| {
        let (ol, or) = (o.x, o.derecha());
        // La linea vertical cubre a los dos pines.
        let (desde, hasta) = (t.min(o.y), b.max(o.abajo()));
        let g = move |pos| Guia {
            vertical: true,
            pos,
            desde,
            hasta,
        };
        [
            (ol - l, g(ol)),
            (or - r, g(or)),
            (ol - r, g(ol)),
            (or - l, g(or)),
            // Separados: la guia marca el borde del otro.
            (or + separacion - l, g(or)),
            (ol - separacion - r, g(ol)),
        ]
    });
    let en_y = otros.iter().flat_map(|o| {
        let (ot, ob) = (o.y, o.abajo());
        let (desde, hasta) = (l.min(o.x), r.max(o.derecha()));
        let g = move |pos| Guia {
            vertical: false,
            pos,
            desde,
            hasta,
        };
        [
            (ot - t, g(ot)),
            (ob - b, g(ob)),
            (ot - b, g(ot)),
            (ob - t, g(ob)),
            (ob + separacion - t, g(ob)),
            (ot - separacion - b, g(ot)),
        ]
    });
    let mut ajuste = Ajuste {
        rect,
        guias: Vec::new(),
    };
    if let Some((dx, g)) = mejor(en_x, umbral) {
        ajuste.rect.x += dx;
        ajuste.guias.push(g);
    }
    if let Some((dy, g)) = mejor(en_y, umbral) {
        ajuste.rect.y += dy;
        ajuste.guias.push(g);
    }
    // La guia tiene que cubrir tambien el pin YA pegado.
    for g in &mut ajuste.guias {
        let r = ajuste.rect;
        if g.vertical {
            g.desde = g.desde.min(r.y);
            g.hasta = g.hasta.max(r.abajo());
        } else {
            g.desde = g.desde.min(r.x);
            g.hasta = g.hasta.max(r.derecha());
        }
    }
    ajuste
}

// ---------------------------------------------------------------------------
// Las lineas en pantalla.
// ---------------------------------------------------------------------------

use windows::Win32::Foundation::{COLORREF, HWND};
use windows::Win32::Graphics::Gdi::CreateSolidBrush;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

static REGISTRO: Once = Once::new();

thread_local! {
    /// Las dos lineas (vertical y horizontal). Se crean la primera vez y se
    /// esconden al acabar: un arrastre crearia y destruiria decenas.
    static LINEAS: Cell<[Option<HWND>; 2]> = const { Cell::new([None, None]) };
}

extern "system" fn procedimiento_guia(
    hwnd: HWND,
    mensaje: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    // SAFETY: delegacion estandar: la linea solo pinta su fondo.
    unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) }
}
fn registrar() {
    // SAFETY: registro unico de una clase sin WndProc propio (la de
    // DefWindowProc basta: no pinta mas que su fondo). El pincel se queda
    // con la clase toda la vida del proceso, que es lo que quiere Win32.
    unsafe {
        let clase = WNDCLASSW {
            lpfnWndProc: Some(procedimiento_guia),
            hInstance: GetModuleHandleW(None).expect("modulo propio").into(),
            lpszClassName: w!("PixPinGuia"),
            // El azul de acento (#0A84FF) en BGR.
            hbrBackground: CreateSolidBrush(COLORREF(0x00FF_840A)),
            ..Default::default()
        };
        RegisterClassW(&clase);
    }
}

fn linea(i: usize) -> Option<HWND> {
    if let Some(h) = LINEAS.with(|l| l.get()[i]) {
        return Some(h);
    }
    REGISTRO.call_once(registrar);
    // SAFETY: clase registrada arriba; una ventana que no se activa, no
    // recoge el raton (transparente + capas) y no sale en la barra de tareas.
    let h = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT,
            w!("PixPinGuia"),
            w!(""),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            GetModuleHandleW(None).ok().map(|m| m.into()),
            None,
        )
        .ok()?
    };
    // SAFETY: ventana propia recien creada, con WS_EX_LAYERED.
    unsafe {
        let _ = SetLayeredWindowAttributes(h, COLORREF(0), 235, LWA_ALPHA);
    }
    LINEAS.with(|l| {
        let mut v = l.get();
        v[i] = Some(h);
        l.set(v);
    });
    Some(h)
}

/// Ensena estas guias (y esconde las que sobren). `grosor` en pixeles.
pub fn ensenar(guias: &[Guia], grosor: i32) {
    for i in 0..2 {
        let g = guias.iter().find(|g| g.vertical == (i == 0));
        match g {
            Some(g) => {
                let Some(h) = linea(i) else { continue };
                let (x, y, ancho, alto) = if g.vertical {
                    (g.pos - grosor / 2, g.desde, grosor, g.hasta - g.desde)
                } else {
                    (g.desde, g.pos - grosor / 2, g.hasta - g.desde, grosor)
                };
                // SAFETY: mover y ensenar una ventana propia sin activarla.
                unsafe {
                    let _ = SetWindowPos(
                        h,
                        Some(HWND_TOPMOST),
                        x,
                        y,
                        ancho.max(1),
                        alto.max(1),
                        SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    );
                }
            }
            None => {
                if let Some(h) = LINEAS.with(|l| l.get()[i]) {
                    // SAFETY: esconder una ventana propia.
                    unsafe {
                        let _ = ShowWindow(h, SW_HIDE);
                    }
                }
            }
        }
    }
}

/// Esconde las guias.
pub fn esconder() {
    ensenar(&[], 1);
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn r(x: i32, y: i32, ancho: u32, alto: u32) -> Rect {
        Rect { x, y, ancho, alto }
    }

    #[test]
    fn a_tiro_del_borde_izquierdo_de_otro_sale_su_guia_y_se_pega() {
        let video = r(440, 128, 600, 300);
        let recorte = r(445, 460, 230, 86);
        let a = alinear(recorte, &[video], 8, 16);
        assert_eq!(a.rect.x, 440, "izquierda con izquierda");
        let g = a.guias.iter().find(|g| g.vertical).expect("guia vertical");
        assert_eq!(g.pos, 440);
        assert!(
            g.desde <= 128 && g.hasta >= a.rect.abajo(),
            "cubre a los dos"
        );
    }

    #[test]
    fn se_ofrece_la_separacion_de_16_px_debajo_de_otro() {
        let video = r(440, 128, 600, 300);
        // Su arriba a 19 px del abajo del video: a 3 de los 16.
        let recorte = r(700, 447, 230, 86);
        let a = alinear(recorte, &[video], 8, 16);
        assert_eq!(a.rect.y, 428 + 16);
        assert!(a.guias.iter().any(|g| !g.vertical && g.pos == 428));
    }

    #[test]
    fn caso_negativo_lejos_de_todo_no_hay_guia_ni_se_mueve() {
        let otro = r(0, 0, 100, 100);
        let lejos = r(500, 500, 100, 100);
        let a = alinear(lejos, &[otro], 8, 16);
        assert_eq!(a.rect, lejos);
        assert!(a.guias.is_empty());
        // Ni sin otros pines.
        assert!(alinear(lejos, &[], 8, 16).guias.is_empty());
    }

    #[test]
    fn gana_el_borde_mas_cercano() {
        let a_lado = r(100, 0, 100, 100);
        let otro = r(103, 300, 100, 100);
        // Izquierda a 5 de la de `a_lado` y a 2 de la de `otro`.
        let movido = r(105, 600, 50, 50);
        let a = alinear(movido, &[a_lado, otro], 8, 16);
        assert_eq!(a.rect.x, 103);
    }

    #[test]
    fn una_guia_por_eje_como_mucho() {
        let otro = r(100, 100, 200, 200);
        let movido = r(103, 104, 200, 200);
        let a = alinear(movido, &[otro], 8, 16);
        assert_eq!(a.rect, otro);
        assert_eq!(a.guias.len(), 2);
        assert_eq!(a.guias.iter().filter(|g| g.vertical).count(), 1);
    }
}
