//! **La lista de chats de las muestras** (2-nov-2026): lo que se ve a la
//! izquierda en las capturas de la Store. Con la ficha tal cual del indice
//! de la prueba salia «01/01/70 · 0 hojas» (tocado en 1 ms y sin ultima
//! linea); aqui el chat abierto va arriba, tocado hace un momento y con su
//! ultima linea, y debajo unos proyectos como los de alguien que la usa.

use pindesk_proyecto::almacen::Ficha;

use super::Abierto;

const MINUTO: i64 = 60_000;
const HORA: i64 = 60 * MINUTO;
const DIA: i64 = 24 * HORA;

/// Las fichas de la lista: `abierto` primero (la elegida, `orden` 0) y
/// otras cuatro con fechas de hoy, ayer y de la semana.
pub(crate) fn fichas(abierto: &Ficha, a: &Abierto, ahora: i64) -> Vec<Ficha> {
    let ultimo = a.mensajes.iter().rev().find_map(|m| {
        let r = m.resumen();
        (!r.trim().is_empty()).then_some((r, m.cuando))
    });
    let mut primera = abierto.clone();
    match ultimo {
        Some((resumen, cuando)) => {
            primera.resumen = resumen;
            primera.tocado = cuando.max(ahora - 2 * MINUTO);
        }
        None => primera.tocado = ahora - 2 * MINUTO,
    }
    primera.hojas = primera.hojas.max(3);
    let otra =
        |id: &str, nombre: &str, hace: i64, hojas: u32, resumen: &str, sin_leer: u32| Ficha {
            id: id.into(),
            nombre: nombre.into(),
            tocado: ahora - hace,
            hojas,
            resumen: resumen.into(),
            sin_leer,
            ..Default::default()
        };
    vec![
        primera,
        otra(
            "tesis",
            "Tesis · capítulo 3",
            47 * MINUTO,
            12,
            "Revisar las citas de la sección 3.2",
            2,
        ),
        otra(
            "obra",
            "Obra Miraflores",
            5 * HORA,
            8,
            "Foto del encofrado del segundo piso",
            0,
        ),
        otra(
            "compras",
            "Compras de la casa",
            DIA + 3 * HORA,
            1,
            "Leche, pan, café y pilas AA",
            0,
        ),
        otra(
            "viaje",
            "Viaje a Cusco",
            4 * DIA,
            5,
            "Reserva del tren confirmada",
            0,
        ),
    ]
}

/// El orden de la lista: tal cual, la abierta primero.
pub(crate) fn orden(fichas: &[Ficha]) -> Vec<usize> {
    (0..fichas.len()).collect()
}
