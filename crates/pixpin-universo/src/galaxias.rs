//! Una galaxia por proyecto del chat, en espiral (D229), y al dia con el
//! indice (D231).

use crate::astro::{Astro, Clase, IdAstro, RADIO_GALAXIA};
use crate::jerarquia::descendientes;
use crate::universo::Universo;

pub const SEPARACION: f32 = 6000.0;
/// Dos radios y un margen de mil: dos galaxias no se tocan nunca.
pub const DISTANCIA_MINIMA: f32 = 2.0 * RADIO_GALAXIA + 1000.0;
/// El angulo aureo en radianes: reparte sin alinear, como las pipas de un
/// girasol, y cada vuelta no pisa la anterior.
const ANGULO_AUREO: f32 = 2.399_963_1;

fn punto_espiral(n: usize) -> (f32, f32) {
    if n == 0 {
        return (0.0, 0.0);
    }
    let r = SEPARACION * (n as f32).sqrt();
    let a = n as f32 * ANGULO_AUREO;
    (r * a.cos(), r * a.sin())
}

/// El primer punto de la espiral que no choca con ninguna galaxia.
pub fn siguiente_hueco(ocupados: &[(f32, f32)]) -> (f32, f32) {
    (0..)
        .map(punto_espiral)
        .find(|(x, y)| {
            ocupados
                .iter()
                .all(|(ox, oy)| ((x - ox).powi(2) + (y - oy).powi(2)).sqrt() >= DISTANCIA_MINIMA)
        })
        .expect("la espiral es infinita")
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Informe {
    pub nuevas: usize,
    pub quitadas: usize,
    pub lunas_quitadas: usize,
    pub conexiones_quitadas: usize,
}

/// Deja una galaxia por cada proyecto de `proyectos` (ids del indice, en su
/// orden) y ninguna mas.
///
/// No pasa por el deshacer: es la verdad de fuera, no algo que hizo el
/// usuario aqui. Por eso, si quita algo, vacia la pila: deshacer un paso
/// viejo podria resucitar la galaxia de un proyecto borrado.
pub fn sincronizar(u: &mut Universo, proyectos: &[String]) -> Informe {
    let mut inf = Informe::default();

    // Quitar lo que ya no existe.
    let muertas: Vec<IdAstro> = u
        .astros
        .iter()
        .filter(
            |a| matches!(&a.clase, Clase::Galaxia { proyecto } if !proyectos.contains(proyecto)),
        )
        .map(|a| a.id)
        .collect();
    if !muertas.is_empty() {
        let mut quitar: Vec<IdAstro> = Vec::new();
        for g in &muertas {
            quitar.push(*g);
            quitar.extend(descendientes(&u.astros, *g));
        }
        // Las lunas de ese proyecto que vivian en exoplanetas.
        for a in &u.astros {
            if let Clase::Luna { proyecto, .. } = &a.clase
                && !proyectos.contains(proyecto)
                && !quitar.contains(&a.id)
            {
                quitar.push(a.id);
            }
        }
        inf.quitadas = muertas.len();
        inf.lunas_quitadas = u
            .astros
            .iter()
            .filter(|a| quitar.contains(&a.id) && matches!(a.clase, Clase::Luna { .. }))
            .count();
        u.astros.retain(|a| !quitar.contains(&a.id));
        inf.conexiones_quitadas = u.limpiar_conexiones();
        u.historia = Default::default();
    }

    // Crear lo que falta.
    for p in proyectos {
        let ya = u
            .astros
            .iter()
            .any(|a| matches!(&a.clase, Clase::Galaxia { proyecto } if proyecto == p));
        if ya {
            continue;
        }
        let ocupados: Vec<(f32, f32)> = u
            .astros
            .iter()
            .filter(|a| matches!(a.clase, Clase::Galaxia { .. }))
            .map(|a| (a.x, a.y))
            .collect();
        let (x, y) = siguiente_hueco(&ocupados);
        let id = u.nuevo_id();
        u.astros.push(Astro::galaxia(id, p, x, y));
        inf.nuevas += 1;
    }

    if inf != Informe::default() {
        u.marcar_cambio();
    }
    inf
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::astro::{Astro, Clase, Conexion};

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn la_primera_galaxia_va_en_el_origen() {
        assert_eq!(siguiente_hueco(&[]), (0.0, 0.0));
    }

    #[test]
    fn un_hueco_nuevo_nunca_choca_con_una_galaxia_movida_a_mano() {
        // Una galaxia puesta a mano justo donde caeria la segunda.
        let segunda = siguiente_hueco(&[(0.0, 0.0)]);
        let hueco = siguiente_hueco(&[(0.0, 0.0), segunda]);
        for (x, y) in [(0.0, 0.0), segunda] {
            let d = ((hueco.0 - x).powi(2) + (hueco.1 - y).powi(2)).sqrt();
            assert!(d >= DISTANCIA_MINIMA, "choca: {d}");
        }
    }

    #[test]
    fn sincronizar_crea_una_galaxia_por_proyecto_nuevo_en_el_orden_del_indice() {
        let mut u = Universo::nuevo();
        let inf = sincronizar(&mut u, &ids(&["a", "b", "c"]));
        assert_eq!(inf.nuevas, 3);
        let primera = &u.astros[0];
        assert_eq!(primera.proyecto(), Some("a"));
        assert_eq!((primera.x, primera.y), (0.0, 0.0));
        // Dos veces no duplica.
        assert_eq!(sincronizar(&mut u, &ids(&["a", "b", "c"])).nuevas, 0);
        assert_eq!(u.astros.len(), 3);
    }

    #[test]
    fn un_proyecto_borrado_se_lleva_su_galaxia_sus_lunas_de_exoplanetas_y_sus_lineas() {
        let mut u = Universo::nuevo();
        sincronizar(&mut u, &ids(&["a", "b"]));
        let gb = u
            .astros
            .iter()
            .find(|a| a.proyecto() == Some("b"))
            .unwrap()
            .id;
        let exo = u.nuevo_id();
        u.astros.push(Astro::planeta(exo, 50_000.0, 0.0, 400.0));
        let lb = u.nuevo_id();
        let mut luna = Astro::luna(lb, "m:1", "b", 50_000.0, 0.0);
        luna.padre = Some(exo);
        u.astros.push(luna);
        u.conexiones.push(Conexion::nueva(99, exo, gb));
        let inf = sincronizar(&mut u, &ids(&["a"]));
        assert_eq!(inf.quitadas, 1);
        assert_eq!(inf.lunas_quitadas, 1);
        assert_eq!(inf.conexiones_quitadas, 1);
        assert!(
            u.astro(exo).is_some(),
            "el exoplaneta no es del proyecto: se queda"
        );
        assert!(
            u.astros
                .iter()
                .all(|a| !matches!(&a.clase, Clase::Luna { proyecto, .. } if proyecto == "b"))
        );
    }
}
