//! Como se pinta la caja de herramientas (D53). La misma para la capa de
//! pantalla y para la paleta del pin: la geometria la da `pixpin-ui` y aqui
//! solo se traduce a rectangulos, colores y letras.
//!
//! `origen` es lo que se resta a cada rect: la capa pasa `(0, 0)` porque su
//! documento ya es local al monitor; la paleta pasa la esquina de su marco
//! para que la caja caiga en el `(0, 0)` de su propia ventana.

use pixpin_geom::Punto;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_ui::{BOTONES, BotonCaja, CajaHerramientas, Herramienta};

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

    for (i, boton) in BOTONES.iter().enumerate() {
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
        // Sin iconos todavia: una letra por herramienta, que es legible y
        // no bloquea el resto de la fase. Los iconos vectoriales llegan
        // cuando el motor dibuje sus propios simbolos.
        p.texto(
            etiqueta(*boton),
            caja_boton.x + 13.0 * e,
            caja_boton.y + 8.0 * e,
            16.0 * e,
            Color::BLANCO,
        );
    }
}

/// La letra que representa cada boton mientras no haya iconos.
fn etiqueta(b: BotonCaja) -> &'static str {
    match b {
        BotonCaja::Elegir(Herramienta::Mano) => "M",
        BotonCaja::Elegir(Herramienta::Lapiz) => "L",
        BotonCaja::Elegir(Herramienta::Resaltador) => "R",
        BotonCaja::Elegir(Herramienta::Linea) => "/",
        BotonCaja::Elegir(Herramienta::Flecha) => ">",
        BotonCaja::Elegir(Herramienta::Rectangulo) => "□",
        BotonCaja::Elegir(Herramienta::Elipse) => "○",
        BotonCaja::Elegir(Herramienta::Texto) => "T",
        BotonCaja::Elegir(Herramienta::Foco) => "F",
        BotonCaja::Elegir(Herramienta::Lupa) => "Q",
        BotonCaja::Elegir(Herramienta::Borrador) => "B",
        BotonCaja::Deshacer => "↶",
        BotonCaja::Rehacer => "↷",
        BotonCaja::Color => "C",
        BotonCaja::Salir => "X",
    }
}
