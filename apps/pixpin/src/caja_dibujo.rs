//! Como se pinta la caja de herramientas (D53). La misma para la capa de
//! pantalla y para la paleta del pin: la geometria la da `pixpin-ui` y aqui
//! solo se traduce a rectangulos, colores y letras.
//!
//! `origen` es lo que se resta a cada rect: la capa pasa `(0, 0)` porque su
//! documento ya es local al monitor; la paleta pasa la esquina de su marco
//! para que la caja caiga en el `(0, 0)` de su propia ventana.

use pixpin_geom::Punto;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_ui::{BotonCaja, CajaHerramientas, Herramienta};

pub fn pintar_caja(
    p: &Pintor,
    caja: &CajaHerramientas,
    activa: Herramienta,
    escala_por_cien: u32,
    origen: Punto,
) {
    let e = escala_por_cien as f32 / 100.0;
    let m = caja.marco;
    p.rellenar_redondeado(
        RectF {
            x: (m.x - origen.x) as f32,
            y: (m.y - origen.y) as f32,
            ancho: m.ancho as f32,
            alto: m.alto as f32,
        },
        8.0 * e,
        Color {
            r: 0.12,
            g: 0.12,
            b: 0.14,
            a: 0.92,
        },
    );

    // La lista viene de la caja, no de una constante fija: cada superficie
    // tiene la suya (BOTONES del anotador, BOTONES_EDITOR del editor), y
    // este dibujo es el mismo para las dos.
    for (i, boton) in caja.botones().iter().enumerate() {
        let r = caja.rect_de(i);
        let caja_boton = RectF {
            x: (r.x - origen.x) as f32,
            y: (r.y - origen.y) as f32,
            ancho: r.ancho as f32,
            alto: r.alto as f32,
        };
        if matches!(boton, BotonCaja::Elegir(h) if *h == activa) {
            p.rellenar_redondeado(
                caja_boton,
                6.0 * e,
                Color {
                    r: 0.25,
                    g: 0.45,
                    b: 0.85,
                    a: 1.0,
                },
            );
        }
        // Los iconos de Excalidraw, a su tamano de boton: 20 px logicos
        // dentro de un boton de 40, como su barra de herramientas.
        let lado = 20.0 * e;
        p.icono(
            icono(*boton),
            RectF {
                x: caja_boton.x + (caja_boton.ancho - lado) / 2.0,
                y: caja_boton.y + (caja_boton.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            Color::BLANCO,
        );
    }
}

/// El icono de Excalidraw de cada boton. Donde Excalidraw no tiene la
/// herramienta (resaltador, foco, cotas), el suyo mas parecido.
fn icono(b: BotonCaja) -> &'static pixpin_render::icono::Icono {
    use pixpin_render::iconos_excalidraw as i;
    match b {
        BotonCaja::Elegir(Herramienta::Mano) => &i::HAND_ICON,
        BotonCaja::Elegir(Herramienta::Lapiz) => &i::FREEDRAW_ICON,
        BotonCaja::Elegir(Herramienta::Resaltador) => &i::PEN_MODE_ICON,
        BotonCaja::Elegir(Herramienta::Linea) => &i::LINE_ICON,
        BotonCaja::Elegir(Herramienta::Flecha) => &i::ARROW_ICON,
        BotonCaja::Elegir(Herramienta::Rectangulo) => &i::RECTANGLE_ICON,
        BotonCaja::Elegir(Herramienta::Elipse) => &i::ELLIPSE_ICON,
        BotonCaja::Elegir(Herramienta::Texto) => &i::TEXT_ICON,
        BotonCaja::Elegir(Herramienta::Foco) => &i::PRESENTATION_ICON,
        BotonCaja::Elegir(Herramienta::Lupa) => &i::SEARCH_ICON,
        BotonCaja::Elegir(Herramienta::Borrador) => &i::ERASER_ICON,
        BotonCaja::Elegir(Herramienta::Cota) => &i::LINE_EDITOR_ICON,
        BotonCaja::Elegir(Herramienta::Escalar) => &i::RESIZE_ICON,
        BotonCaja::Elegir(Herramienta::EscalaGrafica) => &i::GRID_ICON,
        BotonCaja::Deshacer => &i::UNDO_ICON,
        BotonCaja::Rehacer => &i::REDO_ICON,
        BotonCaja::Color => &i::PALETTE,
        BotonCaja::Salir => &i::CLOSE_ICON,
    }
}
