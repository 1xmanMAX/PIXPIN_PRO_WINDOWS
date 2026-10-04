//! **Los menus que se despliegan**: el del «+» de la barra, el de la barra
//! `/` que sale al escribir y el de la flecha del titulo. Como en la
//! captura: una caja redondeada con su sombra, cada entrada con su icono y
//! su nombre (y, en el de la barra, el atajo de Markdown a la derecha, en
//! gris), la elegida con fondo, y abajo una pista apagada.
//!
//! La ventana del menu **no se queda con el foco** (`WS_EX_NOACTIVATE`): en
//! el de la barra se sigue escribiendo en la nota y lo tecleado lo filtra;
//! las flechas, Intro y Esc las atiende el editor antes que el texto. La
//! parte de cuentas ([`Menu`]) no toca ventanas y se prueba sola.

use std::cell::RefCell;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::disposicion::Caja;
use crate::pintor::{Icono, Pintor};
use crate::tema::Tema;

/// El dibujo de una entrada: un icono, o unas letras (`H1`) donde no hay
/// icono que lo diga mejor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dibujo {
    Icono(Icono),
    Letras(&'static str),
    /// Un cuadrado del color de fondo de una celda (`None`: sin color).
    Fondo(Option<crate::tema::Rgb>),
    /// Una «A» del color de la letra (`None`: la del tema).
    Tinta(Option<crate::tema::Rgb>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    pub id: u16,
    pub dibujo: Dibujo,
    pub texto: String,
    /// Lo de la derecha, en gris: el atajo de Markdown o de teclado.
    pub atajo: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Menu {
    pub entradas: Vec<Entrada>,
    pub pista: Option<String>,
    pub elegida: usize,
}

pub const ALTO_ENTRADA: i32 = 30;
const AIRE: i32 = 5;
const ALTO_PISTA: i32 = 30;
const ANCHO_MINIMO: i32 = 200;

impl Menu {
    pub fn bajar(&mut self) {
        if !self.entradas.is_empty() {
            self.elegida = (self.elegida + 1) % self.entradas.len();
        }
    }

    pub fn subir(&mut self) {
        if !self.entradas.is_empty() {
            self.elegida = (self.elegida + self.entradas.len() - 1) % self.entradas.len();
        }
    }

    pub fn elegida(&self) -> Option<u16> {
        self.entradas.get(self.elegida).map(|e| e.id)
    }

    /// Cambia las entradas conservando la elegida si sigue ahi (al filtrar
    /// no salta a la primera en cada letra).
    pub fn cambiar(&mut self, entradas: Vec<Entrada>) {
        let antes = self.elegida();
        self.elegida = antes
            .and_then(|id| entradas.iter().position(|e| e.id == id))
            .unwrap_or(0);
        self.entradas = entradas;
    }

    /// Lo que mide la caja: ancho y alto, con `ancho_de` midiendo textos.
    pub fn tamano(&self, escala: f32, ancho_de: &dyn Fn(&str) -> i32) -> (i32, i32) {
        let e = |v: i32| (v as f32 * escala).round() as i32;
        let mut ancho = e(ANCHO_MINIMO);
        for en in &self.entradas {
            let atajo = if en.atajo.is_empty() {
                0
            } else {
                ancho_de(&en.atajo) + e(20)
            };
            ancho = ancho.max(e(44) + ancho_de(&en.texto) + atajo + e(16));
        }
        if let Some(p) = &self.pista {
            ancho = ancho.max(ancho_de(p) + e(28));
        }
        let pista = if self.pista.is_some() {
            e(ALTO_PISTA)
        } else {
            0
        };
        (
            ancho,
            2 * e(AIRE) + self.entradas.len() as i32 * e(ALTO_ENTRADA) + pista,
        )
    }

    /// La caja de la entrada `i` dentro del menu.
    pub fn caja(&self, i: usize, ancho: i32, escala: f32) -> Caja {
        let e = |v: i32| (v as f32 * escala).round() as i32;
        Caja {
            x: e(AIRE),
            y: e(AIRE) + i as i32 * e(ALTO_ENTRADA),
            an: ancho - 2 * e(AIRE),
            al: e(ALTO_ENTRADA),
        }
    }

    /// La entrada bajo un punto del menu.
    pub fn entrada_en(&self, x: i32, y: i32, ancho: i32, escala: f32) -> Option<usize> {
        (0..self.entradas.len()).find(|&i| self.caja(i, ancho, escala).contiene(x, y))
    }
}

/// Donde sale: debajo del punto pedido, o encima si abajo no cabe, y
/// dentro de la pantalla de lado.
pub fn colocar(
    ancla: (i32, i32),
    alto_ancla: i32,
    tamano: (i32, i32),
    pantalla: (i32, i32, i32, i32),
) -> (i32, i32) {
    let (px, py, pan, pal) = pantalla;
    let x = ancla.0.min(px + pan - tamano.0).max(px);
    let abajo = ancla.1 + alto_ancla;
    let y = if abajo + tamano.1 <= py + pal {
        abajo
    } else {
        (ancla.1 - tamano.1).max(py)
    };
    (x, y)
}

// ---------------------------------------------------------------------------
// La ventana

/// Lo que el menu abierto necesita para pintarse (lo lee su procedimiento).
pub struct Vista {
    pub menu: Menu,
    pub tema: Tema,
    pub pintor: std::rc::Rc<Pintor>,
    pub ancho: i32,
}

thread_local! {
    pub static VISTA: RefCell<Option<Vista>> = const { RefCell::new(None) };
    /// Lo que se pulso o por donde se paso, para el bucle del editor.
    pub static CLIC: RefCell<Option<usize>> = const { RefCell::new(None) };
}

/// Pinta el menu en `hdc` con su esquina en `origen` (la ventana, o una
/// muestra en PNG).
pub fn pintar(hdc: HDC, v: &Vista, origen: (i32, i32), alto: i32) {
    let esc = v.pintor.escala;
    let e = |x: i32| (x as f32 * esc).round() as i32;
    let caja = Caja {
        x: origen.0,
        y: origen.1,
        an: v.ancho,
        al: alto,
    };
    let zona = RECT {
        left: caja.x,
        top: caja.y,
        right: caja.derecha(),
        bottom: caja.abajo(),
    };
    let en = |c: Caja| Caja {
        x: c.x + origen.0,
        y: c.y + origen.1,
        ..c
    };
    let pista_y = caja.abajo() - e(ALTO_PISTA);
    v.pintor.formas(hdc, zona, |f| {
        f.redondo(caja, 9.0, v.tema.menu);
        f.borde(caja, 9.0, 1.0, v.tema.raya);
        for (i, en_) in v.menu.entradas.iter().enumerate() {
            let c = en(v.menu.caja(i, v.ancho, esc));
            if i == v.menu.elegida {
                f.redondo(c, 5.0, v.tema.elegido);
            }
            if let Dibujo::Icono(icono) = en_.dibujo {
                let ic = Caja {
                    x: c.x + e(6),
                    y: c.y,
                    an: e(22),
                    al: c.al,
                };
                f.icono(icono, ic, 15.0, v.tema.texto);
            }
            if let Dibujo::Fondo(color) = en_.dibujo {
                let lado = e(16);
                let m = Caja {
                    x: c.x + e(9),
                    y: c.y + (c.al - lado) / 2,
                    an: lado,
                    al: lado,
                };
                match color {
                    Some(x) => f.redondo(m, 4.0, x),
                    // Sin color: el hueco con su raya cruzada.
                    None => f.raya(
                        m.x + 2,
                        m.abajo() - 2,
                        m.derecha() - 2,
                        m.y + 2,
                        v.tema.tenue,
                    ),
                }
                f.borde(m, 4.0, 1.0, v.tema.raya);
            }
        }
        if v.menu.pista.is_some() {
            f.raya(
                caja.x + 1,
                pista_y,
                caja.derecha() - 1,
                pista_y,
                v.tema.raya,
            );
        }
    });
    for (i, en_) in v.menu.entradas.iter().enumerate() {
        let c = en(v.menu.caja(i, v.ancho, esc));
        if let Dibujo::Letras(t) = en_.dibujo {
            let ic = Caja {
                x: c.x + e(4),
                y: c.y,
                an: e(26),
                al: c.al,
            };
            v.pintor
                .texto(hdc, v.pintor.letra_chica, t, ic, v.tema.texto, true);
        }
        if let Dibujo::Tinta(color) = en_.dibujo {
            let ic = Caja {
                x: c.x + e(4),
                y: c.y,
                an: e(26),
                al: c.al,
            };
            v.pintor.texto(
                hdc,
                v.pintor.letra_negrita,
                "A",
                ic,
                color.unwrap_or(v.tema.texto),
                true,
            );
        }
        let texto = Caja {
            x: c.x + e(34),
            y: c.y,
            an: c.an - e(40),
            al: c.al,
        };
        v.pintor
            .texto(hdc, v.pintor.letra, &en_.texto, texto, v.tema.texto, false);
        if !en_.atajo.is_empty() {
            let an = v.pintor.medir(hdc, v.pintor.letra_chica, &en_.atajo);
            let a = Caja {
                x: c.derecha() - e(10) - an,
                y: c.y,
                an,
                al: c.al,
            };
            v.pintor.texto(
                hdc,
                v.pintor.letra_chica,
                &en_.atajo,
                a,
                v.tema.tenue,
                false,
            );
        }
    }
    if let Some(p) = &v.menu.pista {
        let c = Caja {
            x: caja.x + e(14),
            y: pista_y,
            an: caja.an - e(20),
            al: e(ALTO_PISTA),
        };
        v.pintor
            .texto(hdc, v.pintor.letra_chica, p, c, v.tema.tenue, false);
    }
}

unsafe extern "system" fn procedimiento(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        // Que un clic no le quite el foco a la nota.
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            // SAFETY: pintado de una ventana propia, entre Begin y End.
            unsafe {
                let dc = BeginPaint(h, &mut ps);
                let mut r = RECT::default();
                let _ = GetClientRect(h, &mut r);
                VISTA.with(|v| {
                    if let Some(v) = v.borrow().as_ref() {
                        pintar(dc, v, (0, 0), r.bottom);
                    }
                });
                let _ = EndPaint(h, &ps);
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE | WM_LBUTTONUP => {
            let (x, y) = (
                (l.0 & 0xffff) as i16 as i32,
                ((l.0 >> 16) & 0xffff) as i16 as i32,
            );
            let i = VISTA.with(|v| {
                let mut v = v.borrow_mut();
                let v = v.as_mut()?;
                let i = v.menu.entrada_en(x, y, v.ancho, v.pintor.escala)?;
                let cambio = v.menu.elegida != i;
                v.menu.elegida = i;
                Some((i, cambio))
            });
            if let Some((i, cambio)) = i {
                if m == WM_LBUTTONUP {
                    CLIC.with(|c| *c.borrow_mut() = Some(i));
                } else if cambio {
                    // SAFETY: ventana propia.
                    unsafe {
                        let _ = InvalidateRect(Some(h), None, false);
                    }
                }
            }
            LRESULT(0)
        }
        // SAFETY: lo demas, a Windows.
        _ => unsafe { DefWindowProcW(h, m, w, l) },
    }
}

/// Crea la ventana del menu (oculta), hija flotante de `dueno`.
pub fn crear(dueno: HWND) -> windows::core::Result<HWND> {
    // SAFETY: clase y ventana de este hilo; registrar dos veces falla sin dano.
    unsafe {
        let instancia = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?;
        let clase = WNDCLASSW {
            style: CS_DROPSHADOW,
            lpfnWndProc: Some(procedimiento),
            hInstance: instancia.into(),
            lpszClassName: w!("PixPinNotaMenu"),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&clase);
        let h = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            w!("PixPinNotaMenu"),
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
        // Esquinas redondas de Windows 11 (en el 10 no hace nada).
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

/// Lo ensena en `(x, y)` de la pantalla con su tamano, sin quitar el foco.
pub fn ensenar(h: HWND, x: i32, y: i32, an: i32, al: i32) {
    // SAFETY: ventana propia.
    unsafe {
        let _ = SetWindowPos(
            h,
            Some(HWND_TOPMOST),
            x,
            y,
            an,
            al,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        let _ = InvalidateRect(Some(h), None, false);
    }
}

pub fn esconder(h: HWND) {
    // SAFETY: ventana propia.
    unsafe {
        let _ = ShowWindow(h, SW_HIDE);
    }
}

/// El rectangulo de trabajo de la pantalla donde cae un punto.
pub fn pantalla_de(p: POINT) -> (i32, i32, i32, i32) {
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
    };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: consulta con estructura local.
    unsafe {
        let m = MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST);
        let _ = GetMonitorInfoW(m, &mut info);
    }
    let r = info.rcWork;
    (r.left, r.top, r.right - r.left, r.bottom - r.top)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn entrada(id: u16, t: &str) -> Entrada {
        Entrada {
            id,
            dibujo: Dibujo::Icono(Icono::Lista),
            texto: t.into(),
            atajo: String::new(),
        }
    }

    fn menu() -> Menu {
        Menu {
            entradas: vec![
                entrada(1, "Lista"),
                entrada(2, "Casillas"),
                entrada(3, "Fecha"),
            ],
            pista: Some("Escribe / para mas".into()),
            elegida: 0,
        }
    }

    #[test]
    fn las_flechas_dan_la_vuelta() {
        let mut m = menu();
        m.subir();
        assert_eq!(m.elegida(), Some(3));
        m.bajar();
        assert_eq!(m.elegida(), Some(1));
        m.bajar();
        assert_eq!(m.elegida(), Some(2));
    }

    #[test]
    fn un_menu_vacio_no_elige_nada_ni_se_rompe() {
        let mut m = Menu::default();
        m.bajar();
        m.subir();
        assert_eq!(m.elegida(), None);
    }

    #[test]
    fn al_filtrar_se_queda_la_elegida_si_sigue() {
        let mut m = menu();
        m.bajar();
        m.cambiar(vec![entrada(2, "Casillas"), entrada(3, "Fecha")]);
        assert_eq!(m.elegida(), Some(2));
        m.cambiar(vec![entrada(3, "Fecha")]);
        assert_eq!(m.elegida(), Some(3));
    }

    #[test]
    fn la_caja_crece_con_sus_entradas_y_su_pista() {
        let m = menu();
        let (an, al) = m.tamano(1.0, &|s| s.len() as i32 * 7);
        assert_eq!(an, ANCHO_MINIMO);
        assert_eq!(al, 2 * AIRE + 3 * ALTO_ENTRADA + ALTO_PISTA);
        assert_eq!(m.entrada_en(20, AIRE + ALTO_ENTRADA + 3, an, 1.0), Some(1));
        assert_eq!(
            m.entrada_en(20, al - 5, an, 1.0),
            None,
            "la pista no es una entrada"
        );
    }

    #[test]
    fn el_menu_sale_debajo_o_encima_si_no_cabe() {
        let pantalla = (0, 0, 1000, 800);
        assert_eq!(colocar((100, 100), 20, (200, 300), pantalla), (100, 120));
        assert_eq!(colocar((100, 700), 20, (200, 300), pantalla), (100, 400));
        assert_eq!(colocar((950, 100), 20, (200, 300), pantalla), (800, 120));
    }
}
