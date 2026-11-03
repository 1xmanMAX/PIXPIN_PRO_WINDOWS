//! **La figura en grande** en la ventana de las figuras del chat
//! (3-nov-2026). El usuario: «quiero una vista previa COMPLETA en el panel
//! lateral … si el espacio es pequeno, que haga como en los chats: al
//! hacer clic en una figura, toda la interfaz pasa a ver solo esa figura; si
//! la ventana no es pequena, que salga al lado y que pueda acercar y alejar
//! con el panel tactil, simple y facil».
//!
//! Lo puro de eso, sin pintar nada:
//!
//! - **[`reparto`]**: con la ventana ancha, la rejilla y la figura al lado
//!   (la mitad, mas o menos); estrecha —el panel saldria menor que
//!   [`PREVIA_MINIMA`]—, una cosa cada vez, como el chat estrecho
//!   (Telegram): la rejilla, o la figura sola con su «‹» para volver.
//! - **[`gesto_de_rueda`]**: lo que hace la rueda encima de la figura, como
//!   en un lienzo: dos dedos la llevan en cualquier direccion (la rueda
//!   vertical y la horizontal que manda el panel), pellizcar —que Windows
//!   manda como Ctrl + rueda— y Ctrl + rueda acercan hacia el raton.
//! - **[`tecla_de_zoom`]**: «+», «−» y «0» (encajar).
//!
//! El acercar y el mover en si son los de [`crate::encuadre`]: el aumento
//! cuenta desde «encajada» y la figura no se pierde de vista. La ventana
//! vuelve a pintar las ordenes del dibujo en el rectangulo que sale, asi
//! que se ve nitida a cualquier aumento (no se estira un mapa de bits).

/// La ventana ancha desde aqui (pixeles logicos).
pub const ANCHO_PARA_DOS: f32 = 900.0;
/// Lo menos que mide el panel de la figura al lado.
pub const PREVIA_MINIMA: f32 = 420.0;
/// Lo mas.
pub const PREVIA_MAXIMA: f32 = 960.0;
/// Lo menos que se le deja a la rejilla al lado (dos columnas).
pub const REJILLA_MINIMA: f32 = 380.0;
/// Lo que mueve una muesca de rueda (120) sin Ctrl, en pixeles logicos.
pub const PIXELES_POR_MUESCA: f32 = 60.0;

/// **Como se reparte la ventana.**
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reparto {
    /// La rejilla y, a su derecha, la figura en un panel de este ancho
    /// (logicos).
    AlLado { previa: f32 },
    /// Una cosa cada vez: la rejilla o, elegida una, la figura sola.
    UnaCadaVez,
}

/// **El reparto para una ventana** de `ancho` pixeles logicos.
pub fn reparto(ancho: f32) -> Reparto {
    if ancho < ANCHO_PARA_DOS {
        return Reparto::UnaCadaVez;
    }
    let previa = (ancho * 0.5)
        .clamp(PREVIA_MINIMA, PREVIA_MAXIMA)
        .min(ancho - REJILLA_MINIMA);
    if previa < PREVIA_MINIMA {
        Reparto::UnaCadaVez
    } else {
        Reparto::AlLado { previa }
    }
}

/// **Lo que hace un giro de rueda** encima de la figura.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Gesto {
    /// Acercar (`> 1`) o alejar hacia el raton.
    Acercar(f32),
    /// Llevar la figura (pixeles de la ventana): `dx` positivo, a la derecha.
    Mover { dx: f32, dy: f32 },
}

/// **La rueda encima de la figura.** `delta` de 120 en 120 por muesca
/// (positivo: arriba, o a la derecha si es `horizontal`), `ctrl` si Ctrl
/// estaba pulsado (el pellizco del panel lo trae puesto) y `escala` la de
/// la pantalla (1,0 al 100 %).
///
/// Como en un lienzo y en el visor de fotos del movil: sin Ctrl la rueda
/// lleva, y con Ctrl acerca. Lo que se ve sigue a los dedos: dos dedos
/// hacia arriba (la rueda «hacia abajo») suben la figura.
pub fn gesto_de_rueda(delta: i32, horizontal: bool, ctrl: bool, escala: f32) -> Gesto {
    let muescas = delta as f32 / 120.0;
    if ctrl && !horizontal {
        return Gesto::Acercar(crate::encuadre::PASO.powf(muescas));
    }
    let paso = muescas * PIXELES_POR_MUESCA * escala.max(0.25);
    if horizontal {
        // A la derecha en el panel: se ve lo de la derecha, la figura va a
        // la izquierda.
        Gesto::Mover { dx: -paso, dy: 0.0 }
    } else {
        Gesto::Mover { dx: 0.0, dy: paso }
    }
}

/// **Las teclas del aumento.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeclaZoom {
    Acercar,
    Alejar,
    Encajar,
}

/// La tecla de aumento de un codigo virtual de Windows, si lo es: «+» (de
/// la fila y del teclado numerico), «−» y «0».
pub fn tecla_de_zoom(vk: u32) -> Option<TeclaZoom> {
    match vk {
        // VK_OEM_PLUS, VK_ADD
        0xBB | 0x6B => Some(TeclaZoom::Acercar),
        // VK_OEM_MINUS, VK_SUBTRACT
        0xBD | 0x6D => Some(TeclaZoom::Alejar),
        // «0» y el 0 del teclado numerico
        0x30 | 0x60 => Some(TeclaZoom::Encajar),
        _ => None,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::encuadre::{Caja, Encuadre};

    #[test]
    fn ancha_al_lado_y_estrecha_una_cada_vez() {
        // La ventana de siempre (1320): la figura se lleva la mitad.
        assert_eq!(reparto(1320.0), Reparto::AlLado { previa: 660.0 });
        // Justo en el borde, la rejilla conserva sus dos columnas.
        match reparto(900.0) {
            Reparto::AlLado { previa } => {
                assert!(previa >= PREVIA_MINIMA && 900.0 - previa >= REJILLA_MINIMA)
            }
            otro => panic!("a 900 caben las dos: {otro:?}"),
        }
        // Muy ancha, la figura no pasa de su tope.
        assert_eq!(
            reparto(2600.0),
            Reparto::AlLado {
                previa: PREVIA_MAXIMA
            }
        );
        // Caso negativo: estrecha no se aprieta, se ve una cosa cada vez.
        assert_eq!(reparto(899.0), Reparto::UnaCadaVez);
        assert_eq!(reparto(500.0), Reparto::UnaCadaVez);
    }

    #[test]
    fn dos_dedos_llevan_y_ctrl_o_pellizco_acercan() {
        assert_eq!(
            gesto_de_rueda(-120, false, false, 1.0),
            Gesto::Mover { dx: 0.0, dy: -60.0 }
        );
        assert_eq!(
            gesto_de_rueda(120, true, false, 2.0),
            Gesto::Mover {
                dx: -120.0,
                dy: 0.0
            }
        );
        match gesto_de_rueda(120, false, true, 1.0) {
            Gesto::Acercar(f) => assert!(f > 1.0),
            otro => panic!("Ctrl acerca: {otro:?}"),
        }
        match gesto_de_rueda(-12, false, true, 1.0) {
            Gesto::Acercar(f) => assert!(f < 1.0 && f > 0.95, "el panel, en pasos finos"),
            otro => panic!("{otro:?}"),
        }
        // Caso negativo: Ctrl con la rueda de lado no acerca.
        assert!(matches!(
            gesto_de_rueda(120, true, true, 1.0),
            Gesto::Mover { .. }
        ));
    }

    /// Lo que ve el usuario: acercar con el pellizco hacia un punto y
    /// llevarla con dos dedos, sin perderla de vista, y el doble clic (o el
    /// «0») la vuelve a encajar.
    #[test]
    fn acercar_llevar_y_encajar_como_en_la_ventana() {
        const CAJA: Caja = (0.0, 0.0, 600.0, 400.0);
        let (w, h) = (800.0, 600.0);
        let mut e = Encuadre::default();
        let raton = (340.0, 210.0);
        let antes = e.a_la_imagen(CAJA, w, h, raton).unwrap();
        for _ in 0..4 {
            if let Gesto::Acercar(f) = gesto_de_rueda(120, false, true, 1.0) {
                e.acercar(CAJA, w, h, f, raton);
            }
        }
        assert!(e.aumento > 2.0);
        let despues = e.a_la_imagen(CAJA, w, h, raton).unwrap();
        assert!((antes.0 - despues.0).abs() < 0.5 && (antes.1 - despues.1).abs() < 0.5);
        if let Gesto::Mover { dx, dy } = gesto_de_rueda(-120, false, false, 1.0) {
            e.mover(CAJA, w, h, dx, dy);
        }
        // Caso negativo: por mucho que se lleve, no queda hueco en la caja.
        for _ in 0..100 {
            e.mover(CAJA, w, h, 0.0, -500.0);
        }
        let d = e.destino(CAJA, w, h);
        assert!(d.1 + d.3 >= CAJA.1 + CAJA.3 - 0.5);
        assert_eq!(tecla_de_zoom(0x30), Some(TeclaZoom::Encajar));
        e.encajar();
        assert!(e.encajado());
        assert_eq!(tecla_de_zoom(0xBB), Some(TeclaZoom::Acercar));
        assert_eq!(tecla_de_zoom(0x6D), Some(TeclaZoom::Alejar));
        assert_eq!(tecla_de_zoom(0x41), None, "la A busca, no acerca");
    }
}
