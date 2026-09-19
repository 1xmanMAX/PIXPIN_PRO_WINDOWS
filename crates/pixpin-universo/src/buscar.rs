//! Buscar (D226): en memoria, sin tocar el disco al teclear.

use crate::astro::{Astro, IdAstro};
use crate::universo::Universo;

pub const TOPE_RESULTADOS: usize = 20;

/// Minusculas y sin tildes: quien busca «camion» quiere «Camión».
pub fn normalizar(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            o => o,
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hallazgo {
    Astro(IdAstro),
    /// Un archivo que sigue en la nebulosa.
    Suelta {
        proyecto: String,
        codigo: String,
    },
}

#[derive(Debug, Default)]
pub struct IndiceBusqueda {
    entradas: Vec<(Hallazgo, String)>,
}

impl IndiceBusqueda {
    /// `texto_de` da lo que el universo no sabe de un astro (el nombre del
    /// proyecto de una galaxia, la `busqueda` de la ficha de una luna).
    /// `sueltas` son `(proyecto, codigo, texto)` de lo que esta en nebulosas.
    pub fn construir(
        u: &Universo,
        texto_de: impl Fn(&Astro) -> String,
        sueltas: impl IntoIterator<Item = (String, String, String)>,
    ) -> Self {
        let mut entradas: Vec<(Hallazgo, String)> = u
            .astros
            .iter()
            .map(|a| {
                let t = format!("{} {} {}", texto_de(a), a.nombre, a.nota);
                (Hallazgo::Astro(a.id), normalizar(&t))
            })
            .collect();
        entradas.extend(
            sueltas.into_iter().map(|(proyecto, codigo, t)| {
                (Hallazgo::Suelta { proyecto, codigo }, normalizar(&t))
            }),
        );
        Self { entradas }
    }

    pub fn buscar(&self, consulta: &str, tope: usize) -> Vec<Hallazgo> {
        let q = normalizar(consulta);
        let palabras: Vec<&str> = q.split_whitespace().collect();
        if palabras.is_empty() {
            return Vec::new();
        }
        self.entradas
            .iter()
            .filter(|(_, t)| palabras.iter().all(|p| t.contains(p)))
            .take(tope)
            .map(|(h, _)| h.clone())
            .collect()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::astro::{Astro, IdAstro};

    #[test]
    fn normalizar_quita_mayusculas_y_tildes() {
        assert_eq!(normalizar("Árbol CAMIÓN Ñandú"), "arbol camion nandu");
    }

    fn indice() -> IndiceBusqueda {
        let mut u = Universo::nuevo();
        let mut p = Astro::planeta(IdAstro(1), 0.0, 0.0, 250.0);
        p.nombre = "Planos de la cocina".into();
        p.nota = "revisar con el arquitecto".into();
        u.astros.push(p);
        u.astros.push(Astro::luna(IdAstro(2), "m:a", "p", 0.0, 0.0));
        IndiceBusqueda::construir(
            &u,
            |a| {
                if a.id == IdAstro(2) {
                    "presupuesto final.pdf".into()
                } else {
                    String::new()
                }
            },
            [(
                "p".to_string(),
                "m:z".to_string(),
                "fachada norte.jpg".to_string(),
            )],
        )
    }

    #[test]
    fn encuentra_por_nombre_por_nota_y_por_fichero_sin_tildes() {
        let i = indice();
        assert_eq!(i.buscar("cocina", 20), vec![Hallazgo::Astro(IdAstro(1))]);
        assert_eq!(
            i.buscar("ARQUITECTO", 20),
            vec![Hallazgo::Astro(IdAstro(1))]
        );
        assert_eq!(
            i.buscar("presupuesto", 20),
            vec![Hallazgo::Astro(IdAstro(2))]
        );
        assert_eq!(
            i.buscar("fachada", 20),
            vec![Hallazgo::Suelta {
                proyecto: "p".into(),
                codigo: "m:z".into()
            }]
        );
    }

    #[test]
    fn todas_las_palabras_tienen_que_estar_y_lo_vacio_no_encuentra_nada() {
        let i = indice();
        assert!(i.buscar("cocina fachada", 20).is_empty());
        assert!(i.buscar("   ", 20).is_empty());
    }
}
