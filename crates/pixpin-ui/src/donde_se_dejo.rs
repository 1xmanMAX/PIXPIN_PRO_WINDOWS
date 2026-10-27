//! **Donde se dejo cada chat** (27-oct-2026, checklist del usuario, punto 4:
//! «Todos los chats abren en el último mensaje o donde se dejó; botón «↓»
//! como Telegram»; lo de Android es `PosicionDelChat.kt`, 00ac81c2).
//!
//! - **Al abrir**: si al salir se habia dejado subido, se vuelve a ese
//!   mensaje y a ese punto de el ([`al_abrir`], [`scroll_del_ancla`]); si no,
//!   o si aquel mensaje ya no esta, pegado al final, como un chat nuevo.
//! - **Lo que no se ha visto**: lo que hay por debajo del ultimo mensaje que
//!   llego a verse ([`sin_leer`]), para la chapa del «↓», como Telegram.
//!
//! Aqui solo las cuentas, sin fuente ni disco: quien pinta apunta el ancla
//! ([`ancla`]) y el ultimo visto ([`ultimo_visto`]) de cada fotograma, y la
//! aplicacion lo guarda por chat en este equipo (no viaja: donde se quedo
//! cada uno es de cada aparato).

use pixpin_geom::Rect;

use crate::historial::{self, Puesto};

/// Lo que se apunta de un chat al salir. `arriba`: el primer mensaje que se
/// veia, o nada si se estaba abajo; `desplazamiento`: cuanto de el quedaba
/// por encima del borde (logicos, a escala 100); `leido_hasta`: el ultimo
/// mensaje que llego a verse.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Guardada {
    pub arriba: Option<String>,
    pub desplazamiento: i32,
    pub leido_hasta: Option<String>,
}

/// Adonde ir al abrir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlAbrir {
    /// Pegado al final (el borde de abajo del ultimo mensaje).
    Abajo,
    /// Al mensaje `indice` de la lista, con `desplazamiento` logicos de el
    /// por encima del borde.
    En { indice: usize, desplazamiento: i32 },
}

/// **Adonde se abre** con lo guardado y los mensajes que se ven ahora (`ids`,
/// en el orden del chat). Abajo si no hay nada guardado, si se dejo abajo o
/// si aquel mensaje ya no esta (se borro, o una sincronizacion lo quito):
/// mejor el final que un sitio cualquiera.
pub fn al_abrir<S: AsRef<str>>(guardada: Option<&Guardada>, ids: &[S]) -> AlAbrir {
    let Some(g) = guardada else {
        return AlAbrir::Abajo;
    };
    let Some(id) = g.arriba.as_deref() else {
        return AlAbrir::Abajo;
    };
    match ids.iter().rposition(|x| x.as_ref() == id) {
        // Si el que estaba arriba es ya el ultimo, es lo mismo que abajo.
        Some(i) if i + 1 < ids.len() => AlAbrir::En {
            indice: i,
            desplazamiento: g.desplazamiento.max(0),
        },
        _ => AlAbrir::Abajo,
    }
}

/// **Lo que no se ha visto**: cuantos mensajes hay detras de `leido_hasta`.
/// Si no se sabe o ya no esta, ninguno: la chapa no puede inventarse un
/// numero.
pub fn sin_leer<S: AsRef<str>>(ids: &[S], leido_hasta: Option<&str>) -> usize {
    let Some(id) = leido_hasta else {
        return 0;
    };
    ids.iter()
        .rposition(|x| x.as_ref() == id)
        .map_or(0, |i| ids.len() - 1 - i)
}

/// El mas avanzado de dos «leido hasta»: el que va mas abajo en `ids`. Uno
/// que ya no esta no cuenta.
pub fn mas_avanzado<'a, S: AsRef<str>>(
    ids: &[S],
    a: Option<&'a str>,
    b: Option<&'a str>,
) -> Option<&'a str> {
    let donde = |x: Option<&str>| x.and_then(|x| ids.iter().rposition(|y| y.as_ref() == x));
    match (donde(a), donde(b)) {
        (None, None) => None,
        (Some(ia), Some(ib)) if ib > ia => b,
        (Some(_), _) => a,
        (None, Some(_)) => b,
    }
}

/// **El ancla de lo que se ve**: el primer mensaje que asoma por arriba y
/// cuanto de el queda por encima del borde (en pixeles del documento).
/// `None` pegado al final (`scroll` en `None` o en el tope): eso no se
/// apunta como sitio, se abre abajo y ya.
pub fn ancla(
    area: Rect,
    puestos: &[Puesto],
    alto_total: u32,
    scroll: Option<i32>,
) -> Option<(usize, i32)> {
    let s = scroll?;
    if s >= historial::scroll_maximo(area, alto_total) {
        return None;
    }
    let i = puestos.iter().position(|p| p.burbuja.abajo() > s)?;
    Some((i, (s - puestos[i].arriba()).max(0)))
}

/// **El desplazamiento que vuelve al ancla**: el de arriba del mensaje
/// `indice` mas `desplazamiento`, dentro de lo que existe. `None` si eso ya
/// es el final (se queda pegado, y lo nuevo que llegue se sigue viendo).
pub fn scroll_del_ancla(
    area: Rect,
    puestos: &[Puesto],
    alto_total: u32,
    indice: usize,
    desplazamiento: i32,
) -> Option<i32> {
    let p = puestos.get(indice)?;
    let s = historial::scroll_ajustado(area, alto_total, p.arriba() + desplazamiento);
    (s < historial::scroll_maximo(area, alto_total)).then_some(s)
}

/// **El ultimo mensaje que se ve** con este desplazamiento (`None`, pegado
/// al final): el de mas abajo que asoma dentro del area, aunque sea un poco.
pub fn ultimo_visto(
    area: Rect,
    puestos: &[Puesto],
    alto_total: u32,
    scroll: Option<i32>,
) -> Option<usize> {
    let s = scroll.unwrap_or_else(|| historial::scroll_maximo(area, alto_total));
    let abajo = s + area.alto as i32;
    puestos
        .iter()
        .rposition(|p| p.arriba() < abajo && p.burbuja.abajo() > s)
}

/// De pixeles a logicos (escala 100), para que un cambio de escala no mueva
/// el sitio guardado.
pub fn a_logicos(pixeles: i32, escala_por_cien: u32) -> i32 {
    pixeles * 100 / escala_por_cien.max(1) as i32
}

/// De logicos a pixeles de esta escala.
pub fn a_pixeles(logicos: i32, escala_por_cien: u32) -> i32 {
    logicos * escala_por_cien as i32 / 100
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn area() -> Rect {
        Rect {
            x: 300,
            y: 94,
            ancho: 700,
            alto: 600,
        }
    }

    /// Diez mensajes de 100 de alto, uno detras de otro: 1000 en total.
    fn puestos() -> Vec<Puesto> {
        (0..10)
            .map(|i| Puesto {
                burbuja: Rect {
                    x: 0,
                    y: i * 100,
                    ancho: 200,
                    alto: 90,
                },
                separador: None,
                nombre: None,
            })
            .collect()
    }

    fn ids() -> Vec<String> {
        (0..10).map(|i| format!("m{i}")).collect()
    }

    fn guardada(arriba: Option<&str>, desplazamiento: i32) -> Guardada {
        Guardada {
            arriba: arriba.map(str::to_owned),
            desplazamiento,
            leido_hasta: None,
        }
    }

    #[test]
    fn un_chat_nuevo_o_dejado_abajo_se_abre_abajo() {
        let ids = ids();
        assert_eq!(al_abrir(None, &ids), AlAbrir::Abajo);
        assert_eq!(al_abrir(Some(&guardada(None, 0)), &ids), AlAbrir::Abajo);
        // El que estaba arriba ya es el ultimo: lo mismo que abajo.
        assert_eq!(
            al_abrir(Some(&guardada(Some("m9"), 5)), &ids),
            AlAbrir::Abajo
        );
        // Sin mensajes no hay adonde volver.
        assert_eq!(
            al_abrir(Some(&guardada(Some("m3"), 5)), &Vec::<String>::new()),
            AlAbrir::Abajo
        );
    }

    #[test]
    fn se_vuelve_al_mensaje_y_si_ya_no_esta_abajo() {
        let ids = ids();
        assert_eq!(
            al_abrir(Some(&guardada(Some("m3"), 40)), &ids),
            AlAbrir::En {
                indice: 3,
                desplazamiento: 40
            }
        );
        // Un desplazamiento negativo (fichero tocado a mano) es cero.
        assert_eq!(
            al_abrir(Some(&guardada(Some("m3"), -7)), &ids),
            AlAbrir::En {
                indice: 3,
                desplazamiento: 0
            }
        );
        // Borrado (o quitado por una sincronizacion): abajo.
        assert_eq!(
            al_abrir(Some(&guardada(Some("zz"), 0)), &ids),
            AlAbrir::Abajo
        );
    }

    #[test]
    fn el_ancla_y_su_vuelta_dan_el_mismo_sitio() {
        let (a, p) = (area(), puestos());
        // Pegado al final: no hay ancla.
        assert_eq!(ancla(a, &p, 1000, None), None);
        assert_eq!(ancla(a, &p, 1000, Some(400)), None, "400 es el tope");
        // A 250: asoma el m2 (de 200 a 290), con 50 por encima del borde.
        assert_eq!(ancla(a, &p, 1000, Some(250)), Some((2, 50)));
        // En el hueco entre dos (295): el siguiente, sin nada por encima.
        assert_eq!(ancla(a, &p, 1000, Some(295)), Some((3, 0)));
        // Y volver: el mismo desplazamiento.
        assert_eq!(scroll_del_ancla(a, &p, 1000, 2, 50), Some(250));
        // Lo que ya cae en el final se queda pegado (None).
        assert_eq!(scroll_del_ancla(a, &p, 1000, 8, 0), None);
        // Un mensaje que no existe, tampoco se mueve.
        assert_eq!(scroll_del_ancla(a, &p, 1000, 99, 0), None);
    }

    #[test]
    fn el_ultimo_visto_es_el_de_mas_abajo_que_asoma() {
        let (a, p) = (area(), puestos());
        // Pegado al final se ve el ultimo.
        assert_eq!(ultimo_visto(a, &p, 1000, None), Some(9));
        // Desde 0 se ve de 0 a 600: el m5 (500..590) es el ultimo; el m6
        // empieza justo en el borde y no asoma.
        assert_eq!(ultimo_visto(a, &p, 1000, Some(0)), Some(5));
        assert_eq!(ultimo_visto(a, &p, 1000, Some(1)), Some(6));
        assert_eq!(ultimo_visto(a, &[], 0, None), None);
    }

    #[test]
    fn lo_no_visto_es_lo_de_detras_del_ultimo_leido() {
        let ids = ids();
        assert_eq!(sin_leer(&ids, None), 0);
        assert_eq!(sin_leer(&ids, Some("m9")), 0);
        assert_eq!(sin_leer(&ids, Some("m6")), 3);
        // Uno que ya no esta no inventa numero.
        assert_eq!(sin_leer(&ids, Some("zz")), 0);
    }

    #[test]
    fn el_leido_nunca_vuelve_atras() {
        let ids = ids();
        assert_eq!(mas_avanzado(&ids, Some("m6"), Some("m4")), Some("m6"));
        assert_eq!(mas_avanzado(&ids, Some("m4"), Some("m6")), Some("m6"));
        assert_eq!(mas_avanzado(&ids, None, Some("m4")), Some("m4"));
        assert_eq!(mas_avanzado(&ids, Some("zz"), Some("m4")), Some("m4"));
        assert_eq!(mas_avanzado(&ids, Some("m4"), Some("zz")), Some("m4"));
        assert_eq!(mas_avanzado(&ids, Some("zz"), None), None);
    }

    #[test]
    fn los_logicos_no_dependen_de_la_escala() {
        assert_eq!(a_logicos(150, 150), 100);
        assert_eq!(a_pixeles(100, 150), 150);
        assert_eq!(a_logicos(80, 0), 8000, "escala cero no divide por cero");
    }
}
