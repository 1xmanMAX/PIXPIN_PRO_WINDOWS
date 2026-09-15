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
        // En PixPin la mano ELIGE y mueve: es la Seleccion de Excalidraw,
        // no su mano de desplazar el lienzo.
        BotonCaja::Elegir(Herramienta::Mano) => &i::SELECTION_ICON,
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

/// Un color de la hoja de estilos de Excalidraw, `#rrggbb`.
pub(crate) const fn hex(rgb: u32) -> Color {
    Color {
        r: ((rgb >> 16) & 0xff) as f32 / 255.0,
        g: ((rgb >> 8) & 0xff) as f32 / 255.0,
        b: (rgb & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

/// Tema claro de Excalidraw (`docs/excalidraw/interfaz.md` §2.1 y §5.1).
const ISLA: Color = hex(0xffffff);
const ICONO: Color = hex(0x1b1b1f);
const HOVER: Color = hex(0xf1f0ff);
const ACTIVO_FONDO: Color = hex(0xe0dfff);
const ACTIVO_ICONO: Color = hex(0x030064);
const TECLA: Color = hex(0xb8b8b8);
const SEPARADOR: Color = hex(0xf1f0ff);

/// La sombra de isla de Excalidraw (`--shadow-island`) aproximada con capas
/// redondeadas: tres sombras de CSS no existen en Direct2D, y un desenfoque
/// de verdad costaria un efecto por fotograma para algo casi invisible.
pub(crate) fn sombra_isla(p: &Pintor, r: RectF, radio: f32, e: f32) {
    let capa = |crece: f32, baja: f32, alfa: f32| {
        p.rellenar_redondeado(
            RectF {
                x: r.x - crece,
                y: r.y - crece + baja,
                ancho: r.ancho + 2.0 * crece,
                alto: r.alto + 2.0 * crece,
            },
            radio + crece,
            Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: alfa,
            },
        )
    };
    // 0 7px 14px .05: tres anillos que se van apagando hacia abajo.
    capa(6.0 * e, 7.0 * e, 0.012);
    capa(4.0 * e, 6.0 * e, 0.016);
    capa(2.0 * e, 4.0 * e, 0.02);
    // 0 0 3px .08 y 0 0 1px .17: el contorno fino que marca la isla.
    capa(1.5 * e, 0.0, 0.035);
    capa(0.75 * e, 0.0, 0.12);
}

/// La barra de herramientas de arriba del editor, con el aspecto de
/// Excalidraw: isla blanca, iconos oscuros, fondo lila al pasar el raton y
/// en la herramienta activa, separadores entre grupos y la tecla del atajo
/// en la esquina de cada boton que la tiene.
pub fn pintar_barra(
    p: &Pintor,
    barra: &CajaHerramientas,
    activa: Herramienta,
    escala_por_cien: u32,
    raton: Option<Punto>,
    tecla: impl Fn(BotonCaja) -> Option<char>,
) {
    let e = escala_por_cien as f32 / 100.0;
    let m = barra.marco;
    let marco = RectF {
        x: m.x as f32,
        y: m.y as f32,
        ancho: m.ancho as f32,
        alto: m.alto as f32,
    };
    sombra_isla(p, marco, 8.0 * e, e);
    p.rellenar_redondeado(marco, 8.0 * e, ISLA);

    for s in barra.separadores() {
        p.rellenar(
            RectF {
                x: s.x as f32,
                y: s.y as f32,
                ancho: s.ancho as f32,
                alto: s.alto as f32,
            },
            SEPARADOR,
        );
    }

    let sobre = raton.and_then(|r| barra.boton_en(r));
    for (i, boton) in barra.botones().iter().enumerate() {
        let r = barra.rect_de(i);
        let caja = RectF {
            x: r.x as f32,
            y: r.y as f32,
            ancho: r.ancho as f32,
            alto: r.alto as f32,
        };
        let elegido = matches!(boton, BotonCaja::Elegir(h) if *h == activa);
        if elegido {
            p.rellenar_redondeado(caja, 8.0 * e, ACTIVO_FONDO);
        } else if sobre == Some(*boton) {
            p.rellenar_redondeado(caja, 8.0 * e, HOVER);
        }
        let color = if elegido { ACTIVO_ICONO } else { ICONO };
        let lado = 16.0 * e;
        p.icono(
            icono(*boton),
            RectF {
                x: caja.x + (caja.ancho - lado) / 2.0,
                y: caja.y + (caja.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            color,
        );
        if let Some(c) = tecla(*boton) {
            let texto = c.to_string();
            let tam = 10.0 * e;
            let (w, h) = p.medir_texto(&texto, tam);
            p.texto(
                &texto,
                caja.x + caja.ancho - 4.0 * e - w,
                caja.y + caja.alto - 2.0 * e - h,
                tam,
                if elegido { ACTIVO_ICONO } else { TECLA },
            );
        }
    }
}
