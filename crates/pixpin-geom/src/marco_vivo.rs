//! **Donde esta la ventana de verdad**, para repartir los clics.
//!
//! Los eventos de raton del overlay llegan en coordenadas del escritorio
//! virtual: el WndProc suma al punto del cliente el origen que la ventana
//! tiene EN ESE MOMENTO. Cada ventana los pasa a las suyas restando su
//! `marco`. Si ese `marco` es el de cuando se abrio, en cuanto otro mueve la
//! ventana (AltSnap, Win+flecha, el arrastre del propio Windows por la
//! barra de titulo) los botones se quedan donde estaba: «mover la ventana
//! mueve los sitios de los clics» (1-nov-2026). La regla: el origen se
//! pregunta a Windows en cada vuelta, nunca se guarda del dia que nacio.
//!
//! Aqui, la aritmetica pura de esa regla; la llamada al sistema esta en
//! `pixpin-shell` (`VentanaOverlay::seguir_sitio`).

use crate::punto::Punto;
use crate::rect::Rect;

/// El `marco` que cree la ventana, llevado al sitio `real` que le da
/// Windows. Conserva el tamano del `marco`: la superficie y la disposicion
/// se hicieron para ese, y estirar es cosa de cada ventana. `None` si ya
/// estaba ahi.
pub fn al_sitio(marco: Rect, real: Rect) -> Option<Rect> {
    ((marco.x, marco.y) != (real.x, real.y)).then_some(Rect {
        x: real.x,
        y: real.y,
        ..marco
    })
}

/// De un punto del escritorio a uno de la ventana cuyo origen real es
/// `origen`.
pub fn a_cliente(p: Punto, origen: Punto) -> Punto {
    Punto {
        x: p.x - origen.x,
        y: p.y - origen.y,
    }
}

/// Si un punto del cliente cae en la zona por la que se arrastra la ventana
/// (su cabecera vacia): dentro de alguna `zona` y fuera de todos los
/// `botones`. Es lo que decide si `WM_NCHITTEST` contesta `HTCAPTION`.
pub fn es_arrastre(p: Punto, zonas: &[Rect], botones: &[Rect]) -> bool {
    zonas.iter().any(|z| z.contiene(p)) && !botones.iter().any(|b| b.contiene(p))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn r(x: i32, y: i32, ancho: u32, alto: u32) -> Rect {
        Rect { x, y, ancho, alto }
    }

    #[test]
    fn movida_por_otro_toma_el_sitio_nuevo_y_conserva_su_tamano() {
        let marco = r(400, 200, 900, 600);
        // AltSnap la llevo a la izquierda y, de paso, Windows dice otro
        // tamano: manda el sitio, el tamano es el de la ventana.
        let real = r(-1500, 30, 905, 610);
        assert_eq!(al_sitio(marco, real), Some(r(-1500, 30, 900, 600)));
        assert_eq!(al_sitio(marco, marco), None);
        // Solo el tamano distinto no la mueve.
        assert_eq!(al_sitio(marco, r(400, 200, 1, 1)), None);
    }

    #[test]
    fn el_clic_se_reparte_con_el_origen_de_ahora_no_con_el_de_al_abrir() {
        let al_abrir = Punto { x: 400, y: 200 };
        let ahora = Punto { x: 1200, y: 500 };
        // El boton esta en el (50, 120) de la ventana: tras moverla, el
        // clic en pantalla cae en ahora + (50, 120).
        let clic = Punto { x: 1250, y: 620 };
        assert_eq!(a_cliente(clic, ahora), Punto { x: 50, y: 120 });
        // Con el origen viejo se iria a otro sitio: el fallo de antes.
        assert_ne!(a_cliente(clic, al_abrir), Punto { x: 50, y: 120 });
        // Origen negativo (un monitor a la izquierda del principal).
        assert_eq!(
            a_cliente(Punto { x: -1900, y: -10 }, Punto { x: -1920, y: -40 }),
            Punto { x: 20, y: 30 }
        );
    }

    #[test]
    fn se_arrastra_por_la_cabecera_vacia_y_no_por_sus_botones() {
        let cabecera = [r(0, 0, 800, 90)];
        let botones = [r(600, 20, 150, 40)];
        assert!(es_arrastre(Punto { x: 300, y: 40 }, &cabecera, &botones));
        assert!(!es_arrastre(Punto { x: 650, y: 40 }, &cabecera, &botones));
        // Debajo de la cabecera, el contenido: clic normal.
        assert!(!es_arrastre(Punto { x: 300, y: 90 }, &cabecera, &botones));
        assert!(!es_arrastre(Punto { x: 300, y: 40 }, &[], &botones));
    }
}
