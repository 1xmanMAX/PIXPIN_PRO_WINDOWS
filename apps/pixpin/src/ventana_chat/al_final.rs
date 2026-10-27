//! **El «↓» de Telegram** (27-oct-2026, checklist del usuario, punto 4:
//! «Todos los chats abren en el último mensaje o donde se dejó; botón «↓»
//! como Telegram»). El chat ya abre pegado al ultimo mensaje
//! (`Abierto::scroll` en `None`); al subir mas de un tercio de lo que se ve,
//! sale abajo a la derecha un redondel con la flecha que vuelve al final y
//! lo deja pegado otra vez (lo nuevo que llegue se sigue viendo).
//!
//! **Con su chapa** (27-oct-2026, Android 00ac81c2): cuantos mensajes hay
//! por debajo del ultimo que llego a verse (`donde_se_dejo`), en la pildora
//! azul de los no leidos de la lista. Con algo sin ver, el redondel sale
//! aunque se este poco subido, como en Telegram.

use pixpin_geom::Rect;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_ui::chat;

use super::{Tema, mi};

/// Lo que mide el redondel, y su aire con el borde del historial (logicos).
const LADO: f32 = 40.0;
const AIRE: f32 = 16.0;

/// **Si sale el «↓»**: subido (`scroll` no es `None`, que es pegado al
/// final) mas de un tercio del alto del historial por encima del final, o
/// subido algo con mensajes sin ver debajo (`sin_ver`).
pub(super) fn se_ve(scroll: Option<i32>, tope: i32, alto_area: u32, sin_ver: usize) -> bool {
    scroll.is_some_and(|s| tope - s > (alto_area / 3) as i32 || (sin_ver > 0 && s < tope))
}

/// Donde va: abajo a la derecha del historial.
pub(super) fn caja(area: Rect, e: f32) -> Rect {
    let lado = (LADO * e).round() as u32;
    let aire = (AIRE * e).round() as i32;
    Rect {
        x: area.x + area.ancho as i32 - lado as i32 - aire,
        y: area.y + area.alto as i32 - lado as i32 - aire,
        ancho: lado,
        alto: lado,
    }
}

/// El redondel del color de las pastillas de la cabecera, con su flecha y,
/// si hay algo sin ver, la chapa con cuantos encima.
pub(super) fn pintar(p: &Pintor, tema: &Tema, caja: Rect, e: f32, sin_ver: usize) {
    let r = RectF {
        x: caja.x as f32,
        y: caja.y as f32,
        ancho: caja.ancho as f32,
        alto: caja.alto as f32,
    };
    p.rellenar_redondeado(r, r.ancho / 2.0, tema.pildora);
    let i = 24.0 * e;
    p.icono(
        &mi::KEYBOARD_ARROW_DOWN,
        RectF {
            x: r.x + (r.ancho - i) / 2.0,
            y: r.y + (r.alto - i) / 2.0,
            ancho: i,
            alto: i,
        },
        tema.pildora_texto,
    );
    if sin_ver > 0 {
        pintar_chapa(p, tema, r, e, sin_ver);
    }
}

/// La chapa de lo no visto: centrada sobre el borde de arriba del redondel,
/// la misma pildora que el contador de la lista (el numero entero, sin
/// «99+»: Telegram ensancha la pildora).
fn pintar_chapa(p: &Pintor, tema: &Tema, redondel: RectF, e: f32, sin_ver: usize) {
    let texto = sin_ver.to_string();
    let alto = chat::CONTADOR_ALTO as f32 * e;
    let (w, h) = p.medir_texto(&texto, chat::CONTADOR_TAM * e);
    let ancho = (w + 2.0 * chat::CONTADOR_RELLENO as f32 * e).max(alto);
    let caja = RectF {
        x: redondel.x + (redondel.ancho - ancho) / 2.0,
        y: redondel.y - alto / 2.0,
        ancho,
        alto,
    };
    p.rellenar_redondeado(caja, alto / 2.0, tema.contador);
    p.texto(
        &texto,
        caja.x + (ancho - w) / 2.0,
        caja.y + (alto - h) / 2.0,
        chat::CONTADOR_TAM * e,
        Color::BLANCO,
    );
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn sale_al_subir_y_no_pegado_al_final() {
        // Pegado al final (como abre el chat): no sale.
        assert!(!se_ve(None, 2000, 600, 0));
        assert!(!se_ve(None, 2000, 600, 3), "pegado no hay nada sin ver");
        // Un poco por encima del final, tampoco...
        assert!(!se_ve(Some(1900), 2000, 600, 0));
        // ...salvo que haya llegado algo que no se ha visto.
        assert!(se_ve(Some(1900), 2000, 600, 2));
        // Mas de un tercio de lo que se ve, si.
        assert!(se_ve(Some(1000), 2000, 600, 0));
        // Caso limite: un chat que cabe entero (sin nada que subir).
        assert!(!se_ve(Some(0), -50, 600, 0));
        assert!(!se_ve(Some(2000), 2000, 600, 1), "en el tope es abajo");
    }

    #[test]
    fn va_abajo_a_la_derecha_dentro_del_historial() {
        let area = Rect {
            x: 300,
            y: 60,
            ancho: 700,
            alto: 600,
        };
        let c = caja(area, 1.5);
        assert!(c.x + c.ancho as i32 <= area.x + area.ancho as i32);
        assert!(c.y + c.alto as i32 <= area.y + area.alto as i32);
        assert!(c.x > area.x + area.ancho as i32 / 2 && c.y > area.y + area.alto as i32 / 2);
        assert_eq!(c.ancho, 60);
    }
}
