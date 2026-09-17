//! Los marcos: un recuadro con nombre que se lleva consigo lo que encierra.
//!
//! Es el `frame` de Excalidraw, y aqui va **por contencion**: pertenece al
//! marco lo que cae entero dentro de su caja, y se mira cada vez. Excalidraw
//! guarda ademas un `frameId` en cada hijo; no hacerlo asi es deliberado,
//! porque una lista de hijos es una segunda verdad sobre lo mismo y habria
//! que mantenerla al mover, al borrar, al deshacer y al pegar. Lo que se
//! pierde es poco: un elemento que asoma medio fuera no viaja con el marco,
//! que es justo lo que se ve en la pantalla.

use crate::elemento::{Elemento, Figura};

/// Si un elemento es un marco.
pub fn es_marco(e: &Elemento) -> bool {
    matches!(e.figura, Figura::Marco { .. })
}

/// Lo que cae ENTERO dentro del marco. Nunca otro marco: dos marcos que se
/// solapan se arrastrarian el uno al otro y no habria forma de separarlos.
pub fn contenidos(elementos: &[Elemento], marco: &Elemento) -> Vec<u64> {
    let (mx0, my0, mx1, my1) = marco.caja();
    elementos
        .iter()
        .filter(|e| e.id != marco.id && !e.borrado && !es_marco(e))
        .filter(|e| {
            let (x0, y0, x1, y1) = e.caja();
            x0 >= mx0 && x1 <= mx1 && y0 >= my0 && y1 <= my1
        })
        .map(|e| e.id)
        .collect()
}

/// Los identificadores que hay que mover de verdad: los elegidos, mas lo que
/// encierre cada marco elegido.
///
/// Se devuelve sin repetidos y en orden: mover dos veces el mismo elemento lo
/// desplazaria el doble, que es el fallo tipico de esta funcion.
pub fn con_contenidos(elementos: &[Elemento], elegidos: &[u64]) -> Vec<u64> {
    let mut salida: Vec<u64> = elegidos.to_vec();
    for id in elegidos {
        let Some(marco) = elementos
            .iter()
            .find(|e| e.id == *id)
            .filter(|e| es_marco(e))
        else {
            continue;
        };
        for hijo in contenidos(elementos, marco) {
            if !salida.contains(&hijo) {
                salida.push(hijo);
            }
        }
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo};
    use crate::relleno::EstiloRelleno;

    fn caja(id: u64, figura: Figura, x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
        Elemento {
            id,
            figura,
            x,
            y,
            ancho,
            alto,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: None,
            estilo_relleno: EstiloRelleno::Solido,
            grosor: 1.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
        }
    }

    fn escena() -> Vec<Elemento> {
        vec![
            caja(
                1,
                Figura::Marco {
                    nombre: "Lamina 1".into(),
                },
                0.0,
                0.0,
                200.0,
                200.0,
            ),
            // Dentro del todo.
            caja(2, Figura::Rectangulo, 10.0, 10.0, 50.0, 50.0),
            // Asomando por la derecha: no es suyo.
            caja(3, Figura::Rectangulo, 180.0, 10.0, 50.0, 50.0),
            // Lejos.
            caja(4, Figura::Rectangulo, 400.0, 400.0, 10.0, 10.0),
        ]
    }

    #[test]
    fn el_marco_se_queda_con_lo_que_cabe_entero() {
        let es = escena();
        let dentro = contenidos(&es, &es[0]);
        assert_eq!(dentro, vec![2], "el que asoma y el de fuera no son suyos");
    }

    #[test]
    fn dos_marcos_solapados_no_se_arrastran_el_uno_al_otro() {
        // Si un marco contara como hijo de otro, moverlos seria imposible:
        // cada uno se llevaria al otro y se separarian solos.
        let mut es = escena();
        es.push(caja(
            5,
            Figura::Marco {
                nombre: "Dentro".into(),
            },
            20.0,
            20.0,
            40.0,
            40.0,
        ));
        assert_eq!(contenidos(&es, &es[0]), vec![2]);
    }

    #[test]
    fn mover_un_marco_mueve_lo_de_dentro_una_sola_vez() {
        let es = escena();
        // El hijo va elegido TAMBIEN a mano: sin quitar repetidos se moveria
        // el doble que el marco, y se saldria de el.
        let ids = con_contenidos(&es, &[1, 2]);
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn lo_elegido_que_no_es_marco_se_queda_como_esta() {
        let es = escena();
        assert_eq!(con_contenidos(&es, &[4]), vec![4]);
    }
}
