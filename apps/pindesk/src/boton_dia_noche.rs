//! **Pinta el boton ☀/☾ redondo** (2-nov-2026, como el del movil): un
//! circulo con filete y, dentro, el sol de dia o la luna de noche. Las
//! cuentas (donde va, el giro al cambiar) son de `pindesk_ui::dia_noche`;
//! aqui solo los colores y el dibujo, el mismo en la ventana de figuras del
//! chat, la del lienzo, la cabecera comun y los visores.

use std::sync::OnceLock;
use std::time::Instant;

use pindesk_render::icono::material as mi;
use pindesk_render::{Color, Pintor, RectF};
use pindesk_ui::dia_noche::{ICONO, Paso};

/// Los colores del boton en una ventana.
#[derive(Debug, Clone, Copy)]
pub struct Colores {
    pub fondo: Color,
    /// El fondo con el raton encima.
    pub sobre: Color,
    pub filete: Color,
    pub tinta: Color,
}

impl Colores {
    /// Sobre una cabecera clara (la del movil de dia).
    pub const CLARO: Colores = Colores {
        fondo: Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
        sobre: Color {
            r: 0.94,
            g: 0.94,
            b: 0.96,
            a: 1.0,
        },
        filete: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.12,
        },
        tinta: Color {
            r: 0.11,
            g: 0.11,
            b: 0.12,
            a: 1.0,
        },
    };
    /// Sobre una cabecera oscura.
    pub const OSCURO: Colores = Colores {
        fondo: Color {
            r: 0.17,
            g: 0.17,
            b: 0.18,
            a: 1.0,
        },
        sobre: Color {
            r: 0.23,
            g: 0.23,
            b: 0.24,
            a: 1.0,
        },
        filete: Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 0.16,
        },
        tinta: Color {
            r: 0.96,
            g: 0.96,
            b: 0.97,
            a: 1.0,
        },
    };

    /// Los de un tema, por si es claro u oscuro.
    pub fn de(claro: bool) -> Colores {
        if claro {
            Colores::CLARO
        } else {
            Colores::OSCURO
        }
    }
}

/// Los ms de un reloj que no salta hacia atras, para el giro del icono.
pub fn ahora_ms() -> u64 {
    static RELOJ: OnceLock<Instant> = OnceLock::new();
    RELOJ.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Pinta el boton en `caja` (cuadrada) con los iconos de `paso`.
pub fn pintar(p: &Pintor, caja: RectF, paso: Paso, sobre: bool, c: &Colores) {
    let radio = caja.ancho.min(caja.alto) / 2.0;
    let centro = (caja.x + caja.ancho / 2.0, caja.y + caja.alto / 2.0);
    let grosor = (radio / 18.0).max(1.0);
    p.circulo(centro, radio, if sobre { c.sobre } else { c.fondo });
    p.anillo(centro, radio - grosor / 2.0, grosor, c.filete);
    // El icono mide lo que en el movil frente al boton (20 de 36).
    let lado = 2.0 * radio * ICONO as f32 / pindesk_ui::dia_noche::LADO as f32;
    for t in paso.sale.iter().chain(std::iter::once(&paso.entra)) {
        if t.alfa <= 0.01 {
            continue;
        }
        let l = lado * t.tam;
        let r = RectF {
            x: centro.0 - l / 2.0,
            y: centro.1 - l / 2.0,
            ancho: l,
            alto: l,
        };
        let icono = if t.luna {
            &mi::DARK_MODE
        } else {
            &mi::LIGHT_MODE
        };
        let tinta = Color {
            a: c.tinta.a * t.alfa,
            ..c.tinta
        };
        p.girado(centro, t.giro, |p| p.icono(icono, r, tinta));
    }
}

/// Lo de siempre: quieto, con el modo puesto.
pub fn pintar_quieto(p: &Pintor, caja: RectF, noche: bool, sobre: bool, c: &Colores) {
    pintar(p, caja, pindesk_ui::dia_noche::paso(noche, 1.0), sobre, c);
}
