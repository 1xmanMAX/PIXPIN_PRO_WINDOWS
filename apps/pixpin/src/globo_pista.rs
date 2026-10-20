//! **El globo de los botones de las barras** (19-oct-2026): el nombre y el
//! atajo del boton bajo el raton, en una ventanita propia debajo de el. Las
//! barras de los pines de los visores son ventanas pequenas aparte: dentro
//! no cabe, asi que va fuera, pasante a los clics y sin quitar el foco.
//!
//! Una por hilo (cada visor vive en el suyo). `mostrar` la coloca y la pinta;
//! `ocultar` la esconde. Si algo falla al crearla, simplemente no sale.

use std::cell::RefCell;

use pixpin_geom::Rect;
use pixpin_render::{Color, MotorRender, RectF, Superficie};
use pixpin_shell::overlay::VentanaOverlay;

const FONDO: Color = Color { r: 0.106, g: 0.106, b: 0.122, a: 0.96 };
const TINTA: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };

struct Globo {
    ventana: VentanaOverlay,
    superficie: Superficie,
    motor: MotorRender,
    _d: pixpin_capture::Dispositivo,
    texto: String,
    area: Rect,
}

thread_local! {
    static GLOBO: RefCell<Option<Globo>> = const { RefCell::new(None) };
}

/// Lo que mide el globo para `texto` a escala `e`: un calculo por letras
/// (sin pintor aun); la pastilla deja aire de sobra.
fn medida(texto: &str, e: f32) -> (u32, u32) {
    let letras = texto.chars().count() as f32;
    (((letras * 6.9 + 18.0) * e).ceil() as u32, (24.0 * e).ceil() as u32)
}

/// Ensena `texto` centrado bajo `boton` (pixeles de pantalla), a escala `e`.
pub fn mostrar(texto: &str, boton: Rect, e: f32) {
    let (w, h) = medida(texto, e);
    let area = Rect {
        x: boton.x + boton.ancho as i32 / 2 - w as i32 / 2,
        y: boton.y + boton.alto as i32 + (6.0 * e) as i32,
        ancho: w,
        alto: h,
    };
    GLOBO.with(|g| {
        let mut g = g.borrow_mut();
        if g.as_ref().is_some_and(|x| x.texto == texto && x.area == area) {
            return;
        }
        if g.is_none() {
            *g = crear(area);
        }
        let Some(globo) = g.as_mut() else { return };
        if globo.area != area {
            globo.ventana.mover(area);
            let _ = globo.superficie.redimensionar(area.ancho, area.alto);
            globo.area = area;
        }
        globo.texto = texto.to_string();
        pintar(globo, e);
        globo.ventana.mostrar();
    });
}

/// Lo esconde (el raton salio del boton o de la barra).
pub fn ocultar() {
    GLOBO.with(|g| {
        if let Some(globo) = g.borrow_mut().as_mut()
            && !globo.texto.is_empty()
        {
            globo.texto.clear();
            globo.ventana.ocultar();
        }
    });
}

fn crear(area: Rect) -> Option<Globo> {
    let d = pixpin_capture::Dispositivo::nuevo().ok()?;
    let motor = MotorRender::nuevo(d.d3d()).ok()?;
    let ventana = VentanaOverlay::nueva(area).ok()?;
    ventana.poner_pasante(true);
    ventana.poner_sin_activar();
    let superficie = Superficie::nueva(&motor, d.d3d(), ventana.handle(), area.ancho, area.alto).ok()?;
    Some(Globo { ventana, superficie, motor, _d: d, texto: String::new(), area })
}

fn pintar(g: &Globo, e: f32) {
    let Ok(destino) = g.superficie.empezar(&g.motor) else { return };
    let (w, h) = (g.area.ancho as f32, g.area.alto as f32);
    let _ = g.motor.dibujar(&destino, |p| {
        p.limpiar_transparente();
        p.rellenar_redondeado(RectF { x: 0.0, y: 0.0, ancho: w, alto: h }, 6.0 * e, FONDO);
        let tam = 12.0 * e;
        let (tw, th) = p.medir_texto(&g.texto, tam);
        p.texto(&g.texto, (w - tw) / 2.0, (h - th) / 2.0, tam, TINTA);
    });
    let _ = g.superficie.presentar();
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_globo_crece_con_el_texto_y_con_la_escala() {
        let (a, _) = medida("Cerrar (Esc)", 1.0);
        let (b, _) = medida("Pantalla completa (F11)", 1.0);
        assert!(b > a);
        let (c, h) = medida("Cerrar (Esc)", 2.0);
        assert!(c >= 2 * a - 1 && h >= 48);
    }
}
