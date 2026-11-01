//! **Mayus+clic marca un tramo** de burbujas, con la seleccion ya abierta.
//!
//! En el movil se marcan varias manteniendo y jalando el dedo
//! (`detectDragGesturesAfterLongPress`, `MensajesActivity.kt:2842-2870`), y
//! aqui eso ya esta: con algo marcado, pulsar y barrer suma lo que pasa por
//! debajo. Lo que un escritorio anade es Mayus+clic: en una lista larga,
//! barrer obliga a arrastrar mientras la lista se desplaza, y cualquier
//! lista de Windows marca el tramo entero con un clic.
//!
//! **Desde donde**: desde la marcada mas cercana a la pulsada, en el orden en
//! que se ven. Asi no hace falta recordar cual se marco la ultima (que el
//! movil tampoco guarda), y con una sola marcada es exactamente lo de
//! Windows: de esa hasta aqui.

/// Los mensajes (por posicion en el cuaderno) que marca Mayus+clic sobre
/// `pulsado`. `visibles` es el orden en que se ven; `marcado` dice si uno ya
/// lo esta. Vacio si no hay ninguna marcada o la pulsada no se ve.
pub fn tramo(visibles: &[usize], marcado: impl Fn(usize) -> bool, pulsado: usize) -> Vec<usize> {
    let Some(hasta) = visibles.iter().position(|&i| i == pulsado) else {
        return Vec::new();
    };
    let Some(desde) = visibles
        .iter()
        .enumerate()
        .filter(|(_, i)| marcado(**i))
        .map(|(n, _)| n)
        .min_by_key(|n| n.abs_diff(hasta))
    else {
        return Vec::new();
    };
    let (a, b) = (desde.min(hasta), desde.max(hasta));
    visibles[a..=b].to_vec()
}

#[cfg(test)]
mod pruebas {
    use super::tramo;

    #[test]
    fn mayus_clic_marca_desde_la_marcada_mas_cercana_hasta_la_pulsada() {
        // Se ven los mensajes 0, 2, 3, 5, 8 (los otros los esconde la lupa).
        let visibles = [0, 2, 3, 5, 8];
        assert_eq!(tramo(&visibles, |i| i == 2, 8), vec![2, 3, 5, 8]);
        // Hacia arriba igual.
        assert_eq!(tramo(&visibles, |i| i == 5, 0), vec![0, 2, 3, 5]);
        // Con dos marcadas manda la mas cercana a la pulsada...
        assert_eq!(tramo(&visibles, |i| i == 0 || i == 8, 5), vec![5, 8]);
        // ...y a igual distancia, la de arriba: la primera que se encuentra.
        assert_eq!(tramo(&visibles, |i| i == 0 || i == 8, 3), vec![0, 2, 3]);
    }

    #[test]
    fn sin_ninguna_marcada_o_pulsando_una_que_no_se_ve_no_marca_nada() {
        let visibles = [0, 2, 3];
        assert!(tramo(&visibles, |_| false, 2).is_empty());
        assert!(tramo(&visibles, |i| i == 0, 7).is_empty());
        // Pulsar la misma marcada es un tramo de una.
        assert_eq!(tramo(&visibles, |i| i == 2, 2), vec![2]);
    }
}
