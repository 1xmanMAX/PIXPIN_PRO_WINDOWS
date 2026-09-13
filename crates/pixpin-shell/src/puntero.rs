//! Entrada fina para dibujar: todos los puntos del raton y del lapiz (E1).
//!
//! Windows fusiona los `WM_MOUSEMOVE` pendientes en uno: con un raton de
//! 1000 Hz y un fotograma ocupado, la aplicacion ve 40-60 puntos por
//! segundo y un trazo rapido sale como un poligono de pocos lados. Los
//! puntos no se pierden: `GetMouseMovePointsEx` guarda los 64 ultimos. Y el
//! lapiz trae los suyos, con presion y subpixel, en su historial de puntero.
//!
//! Excalidraw no hace nada de esto (se queda con un punto por fotograma);
//! aqui se puede hacer mejor que el original.
//!
//! Por que no se llama a `EnableMouseInPointer` (D106): convertiria el raton
//! en punteros para todo el proceso, y el overlay, los pines y los gestos
//! se reescribirian sin pruebas.

use std::cell::Cell;

use windows::Win32::Foundation::{RECT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GMMP_USE_DISPLAY_POINTS, GetMouseMovePointsEx, MOUSEMOVEPOINT,
};
use windows::Win32::UI::Input::Pointer::{
    GetPointerDeviceRects, GetPointerPenInfoHistory, GetPointerType, POINTER_FLAG_INCONTACT,
    POINTER_PEN_INFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageExtraInfo, PEN_FLAG_ERASER, PEN_FLAG_INVERTED, PEN_MASK_PRESSURE, POINTER_INPUT_TYPE,
    PT_PEN,
};

/// Una muestra del puntero en el escritorio virtual.
///
/// En punto fijo (1/16 px y presion 0-1024) y no en `f32` porque viaja
/// dentro de `EventoOverlay`, que es `Copy + Eq`: un `f32` no es `Eq`.
/// 1/16 de pixel es mas fino que cualquier digitalizador de consumo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Muestra {
    x16: i32,
    y16: i32,
    presion: Option<u16>,
}

impl Muestra {
    pub fn nueva(x: f32, y: f32, presion: Option<f32>) -> Muestra {
        Muestra {
            x16: (x * 16.0).round() as i32,
            y16: (y * 16.0).round() as i32,
            presion: presion.map(|p| (p.clamp(0.0, 1.0) * 1024.0).round() as u16),
        }
    }
    pub fn x(&self) -> f32 {
        self.x16 as f32 / 16.0
    }
    pub fn y(&self) -> f32 {
        self.y16 as f32 / 16.0
    }
    pub fn presion(&self) -> Option<f32> {
        self.presion.map(|p| p as f32 / 1024.0)
    }
}

/// Los puntos que Windows fusiono entre el ultimo entregado y el actual.
///
/// `recientes` viene como lo devuelve `GetMouseMovePointsEx`: el mas nuevo
/// primero, y en `[0]` el punto del propio mensaje, que ya llega por su
/// cuenta y no se repite. Se para al llegar al anterior o a algo mas viejo
/// que el: si el anterior ya salio del historial, no se devuelve historia
/// de antes de pulsar. (El contador de tiempo da la vuelta cada 49 dias:
/// ese mensaje se queda sin recuperar, nada mas.)
pub fn perdidos_entre(recientes: &[(i32, i32, u32)], anterior: (i32, i32, u32)) -> Vec<(i32, i32)> {
    let mut v = Vec::new();
    for &(x, y, t) in recientes.iter().skip(1) {
        if (x, y, t) == anterior || t < anterior.2 {
            break;
        }
        v.push((x, y));
    }
    v.reverse();
    v
}

/// HIMETRIC del digitalizador a pixeles del escritorio, por proporcion
/// entre los dos rectangulos que da `GetPointerDeviceRects`.
pub fn himetric_a_pixel(
    him: (i32, i32),
    dispositivo: (i32, i32, i32, i32),
    pantalla: (i32, i32, i32, i32),
) -> (f32, f32) {
    let (dl, dt, dr, db) = dispositivo;
    let (pl, pt, pr, pb) = pantalla;
    let ancho_d = ((dr - dl) as f32).max(1.0);
    let alto_d = ((db - dt) as f32).max(1.0);
    (
        pl as f32 + (him.0 - dl) as f32 * (pr - pl) as f32 / ancho_d,
        pt as f32 + (him.1 - dt) as f32 * (pb - pt) as f32 / alto_d,
    )
}

/// Con varios monitores, `GMMP_USE_DISPLAY_POINTS` devuelve las
/// coordenadas negativas como valores de 16 bits sin signo.
fn con_signo(c: i32) -> i32 {
    if c > 32767 { c - 65536 } else { c }
}

thread_local! {
    static AVISADO: Cell<bool> = const { Cell::new(false) };
}

fn avisar_una_vez(que: &str) {
    if !AVISADO.with(|a| a.replace(true)) {
        tracing::warn!(
            que,
            "sin historial de puntero: se usa solo el punto del mensaje"
        );
    }
}

/// Lo que hace falta recordar entre dos `WM_MOUSEMOVE` de una ventana.
#[derive(Debug, Default)]
pub struct HistorialRaton {
    anterior: Option<(i32, i32, u32)>,
}

impl HistorialRaton {
    pub const fn nuevo() -> Self {
        Self { anterior: None }
    }

    /// Al soltar el boton: el siguiente trazo no recupera puntos de este.
    pub fn olvidar(&mut self) {
        self.anterior = None;
    }

    /// Los puntos fusionados antes de `(x, y)` (escritorio virtual) y
    /// `tiempo` (`GetMessageTime`). El primer movimiento de un trazo no
    /// recupera nada: lo anterior es de antes de pulsar.
    pub fn recuperar(&mut self, x: i32, y: i32, tiempo: u32) -> Vec<Muestra> {
        let Some(anterior) = self.anterior.replace((x, y, tiempo)) else {
            return Vec::new();
        };
        let entrada = MOUSEMOVEPOINT {
            x: x & 0xFFFF,
            y: y & 0xFFFF,
            time: tiempo,
            dwExtraInfo: 0,
        };
        let mut buf = [MOUSEMOVEPOINT::default(); 64];
        // SAFETY: estructura de entrada local y bufer local de 64, el maximo
        // que documenta la API; el tamano pasado es el de la estructura.
        let n = unsafe {
            GetMouseMovePointsEx(
                std::mem::size_of::<MOUSEMOVEPOINT>() as u32,
                &entrada,
                &mut buf,
                GMMP_USE_DISPLAY_POINTS,
            )
        };
        if n <= 0 {
            avisar_una_vez("GetMouseMovePointsEx");
            return Vec::new();
        }
        let recientes: Vec<(i32, i32, u32)> = buf[..n as usize]
            .iter()
            .map(|m| (con_signo(m.x), con_signo(m.y), m.time))
            .collect();
        perdidos_entre(&recientes, anterior)
            .into_iter()
            .map(|(x, y)| Muestra::nueva(x as f32, y as f32, None))
            .collect()
    }
}

/// De donde vino un `WM_MOUSEMOVE`, segun la firma que Windows deja en
/// `GetMessageExtraInfo` cuando lo sintetiza a partir de otro dispositivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrigenRaton {
    /// Un raton de verdad (o cualquier firma que no reconocemos).
    Raton,
    /// Sintetizado a partir de un lapiz. Sus muestras buenas llegan por
    /// `WM_POINTERUPDATE`.
    Lapiz,
    /// Sintetizado a partir del tacto. El tacto no trae historial de
    /// puntero propio en esta entrega: este movimiento es lo unico que hay,
    /// y no debe tirarse.
    Tacto,
}

/// Clasifica la firma de `GetMessageExtraInfo` de un `WM_MOUSEMOVE`.
///
/// La firma documentada de lapiz y tacto comparten los tres bytes altos
/// (`0xFF5157xx`); el bit `0x80` del byte bajo es el que distingue tacto
/// (puesto) de lapiz (a cero). Antes de esta funcion, `es_raton_de_lapiz`
/// trataba ambos como lapiz y tiraba el movimiento sintetizado del tacto:
/// como `muestras_de_lapiz` devuelve `None` para `PT_TOUCH`, un arrastre con
/// el dedo se quedaba sin `RatonMovido` y sin `Muestra`, es decir, sin
/// dibujar nada.
pub fn origen_del_raton(extra: usize) -> OrigenRaton {
    if extra & 0xFFFF_FF00 != 0xFF51_5700 {
        OrigenRaton::Raton
    } else if extra & 0x80 != 0 {
        OrigenRaton::Tacto
    } else {
        OrigenRaton::Lapiz
    }
}

/// Si el `WM_MOUSEMOVE` que se esta atendiendo lo sintetizo Windows a partir
/// de un lapiz (no del tacto: ver `origen_del_raton`).
///
/// Esto por si solo NO basta para tirar el movimiento: si el lapiz real no
/// entrego muestras por `WM_POINTERUPDATE` (API caida, o el propio lapiz
/// fallo la firma por algun motivo), el llamante debe quedarse con el punto
/// del mensaje en vez de perderlo.
pub fn es_raton_de_lapiz() -> bool {
    // SAFETY: lee un valor del mensaje actual del hilo; sin precondiciones.
    let extra = unsafe { GetMessageExtraInfo() }.0 as usize;
    origen_del_raton(extra) == OrigenRaton::Lapiz
}

/// Las muestras de un `WM_POINTERUPDATE` de lapiz en contacto, de la mas
/// vieja a la mas nueva. `None` si no es un lapiz o si falla la API.
pub fn muestras_de_lapiz(wparam: WPARAM) -> Option<Vec<Muestra>> {
    let id = (wparam.0 & 0xFFFF) as u32;
    let mut tipo = POINTER_INPUT_TYPE::default();
    // SAFETY: id del mensaje actual y salida local.
    unsafe { GetPointerType(id, &mut tipo).ok()? };
    if tipo != PT_PEN {
        return None;
    }
    let mut cuantos = 0u32;
    // SAFETY: primera llamada solo para saber cuantas entradas hay.
    if unsafe { GetPointerPenInfoHistory(id, &mut cuantos, None) }.is_err() {
        avisar_una_vez("GetPointerPenInfoHistory");
        return None;
    }
    let mut buf = vec![POINTER_PEN_INFO::default(); cuantos.max(1) as usize];
    // SAFETY: bufer local del tamano pedido; `cuantos` sale con lo escrito.
    unsafe { GetPointerPenInfoHistory(id, &mut cuantos, Some(buf.as_mut_ptr())).ok()? };
    buf.truncate(cuantos as usize);
    let dispositivo = buf.first()?.pointerInfo.sourceDevice;
    let (mut rd, mut rp) = (RECT::default(), RECT::default());
    // SAFETY: dispositivo del propio mensaje; salidas locales.
    let con_rects = unsafe { GetPointerDeviceRects(dispositivo, &mut rd, &mut rp) }.is_ok();
    Some(
        buf.iter()
            .rev()
            .filter(|i| i.pointerInfo.pointerFlags.contains(POINTER_FLAG_INCONTACT))
            // D107: el borrador del lapiz (o la punta usada al reves) se
            // detecta y se ignora hasta E4, que es donde se implementa
            // borrar con el. Sin este filtro, dar la vuelta al lapiz
            // dibujaria con el en vez de no hacer nada.
            .filter(|i| i.penFlags & (PEN_FLAG_ERASER | PEN_FLAG_INVERTED) == 0)
            .map(|i| {
                let (x, y) = if con_rects {
                    himetric_a_pixel(
                        (
                            i.pointerInfo.ptHimetricLocation.x,
                            i.pointerInfo.ptHimetricLocation.y,
                        ),
                        (rd.left, rd.top, rd.right, rd.bottom),
                        (rp.left, rp.top, rp.right, rp.bottom),
                    )
                } else {
                    (
                        i.pointerInfo.ptPixelLocation.x as f32,
                        i.pointerInfo.ptPixelLocation.y as f32,
                    )
                };
                // penMask es un u32 de bits crudo, no un tipo con .contains().
                let presion =
                    (i.penMask & PEN_MASK_PRESSURE != 0).then(|| i.pressure as f32 / 1024.0);
                Muestra::nueva(x, y, presion)
            })
            .collect(),
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_recuperan_los_puntos_fusionados_del_mas_viejo_al_mas_nuevo() {
        // Como lo da Windows: el mas nuevo primero, y el [0] es el del mensaje.
        let recientes = [
            (40, 0, 104),
            (30, 0, 103),
            (20, 0, 102),
            (10, 0, 101),
            (0, 0, 100),
        ];
        let anterior = (10, 0, 101);
        assert_eq!(perdidos_entre(&recientes, anterior), vec![(20, 0), (30, 0)]);
    }

    #[test]
    fn sin_nada_fusionado_no_se_recupera_nada() {
        let recientes = [(40, 0, 104), (30, 0, 103)];
        assert!(perdidos_entre(&recientes, (30, 0, 103)).is_empty());
    }

    #[test]
    fn no_se_cuela_nada_mas_viejo_que_el_ultimo_entregado() {
        // El anterior ya salio del historial (mas de 64 movimientos): se para
        // en el primero con tiempo menor, no se devuelve historia vieja.
        let recientes = [(40, 0, 204), (30, 0, 203), (20, 0, 150), (10, 0, 90)];
        assert_eq!(perdidos_entre(&recientes, (0, 0, 180)), vec![(30, 0)]);
    }

    #[test]
    fn himetric_se_reparte_proporcional_sobre_la_pantalla() {
        let (x, y) = himetric_a_pixel((5000, 2500), (0, 0, 10000, 5000), (100, 50, 2020, 1130));
        assert!(
            (x - 1060.0).abs() < 0.01 && (y - 590.0).abs() < 0.01,
            "{x},{y}"
        );
    }

    #[test]
    fn un_dispositivo_de_tamano_cero_no_divide_por_cero() {
        let (x, y) = himetric_a_pixel((5, 5), (0, 0, 0, 0), (0, 0, 100, 100));
        assert!(x.is_finite() && y.is_finite());
    }

    #[test]
    fn la_firma_distingue_raton_lapiz_y_tacto() {
        // Un raton de verdad no lleva ninguna firma.
        assert_eq!(origen_del_raton(0), OrigenRaton::Raton);
        // El lapiz: los tres bytes altos de la firma, con el bit 0x80 a cero.
        assert_eq!(origen_del_raton(0xFF51_5700), OrigenRaton::Lapiz);
        // El tacto: la misma firma, pero con el bit 0x80 puesto. Antes de
        // origen_del_raton, esto se confundia con un lapiz y el arrastre
        // con el dedo se tiraba sin dejar ni RatonMovido ni Muestra.
        assert_eq!(origen_del_raton(0xFF51_5780), OrigenRaton::Tacto);
    }

    #[test]
    fn la_muestra_guarda_subpixel_y_presion_redondeados() {
        let m = Muestra::nueva(10.53, -3.25, Some(0.5));
        // 10.53 * 16 = 168.48, que redondea a 168 (1/16 mas cercano): 10.5.
        assert!((m.x() - 10.5).abs() < 1e-4, "{}", m.x());
        assert!((m.y() + 3.25).abs() < 1e-4);
        assert_eq!(m.presion(), Some(512.0 / 1024.0));
        assert_eq!(Muestra::nueva(0.0, 0.0, None).presion(), None);
    }
}
