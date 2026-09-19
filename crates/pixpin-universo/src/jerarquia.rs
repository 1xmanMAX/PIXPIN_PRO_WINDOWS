//! Que puede ir dentro de que (D201–D205).
//!
//! La jerarquia es fija: cosmos > galaxia > planeta > luna. Se guarda el
//! padre de cada astro y se recalcula solo al soltar: el zoom semantico
//! pregunta «que hay dentro» en cada fotograma, y buscarlo por geometria
//! cada vez costaria lo que cuesta el fotograma entero.

use crate::astro::{Astro, Clase, IdAstro};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rechazo {
    /// Una luna dentro de la galaxia de otro proyecto (D202).
    OtraGalaxia,
    /// Una luna fuera de toda galaxia y de todo exoplaneta (D202).
    FueraDeGalaxia,
    /// Una galaxia con el centro dentro de otra.
    GalaxiaEncima,
    /// Un exoplaneta con lunas de otro proyecto no puede entrar en una
    /// galaxia: esas lunas acabarian en una galaxia que no es la suya.
    ExtranjerasEnGalaxia,
    /// La luna ya esta en el cielo (D201).
    YaColocada,
    NoExiste,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aterriza {
    pub padre: Option<IdAstro>,
    /// Se acepta pero se avisa: un planeta soltado encima de otro (D203).
    pub aviso: bool,
}

/// Donde cae `movido` si se suelta con el centro en `(x, y)`.
///
/// `ignorar` son los que se mueven con el (sus hijos, el resto de la
/// seleccion): un planeta no puede aterrizar dentro de si mismo.
pub fn aterrizaje(
    astros: &[Astro],
    movido: &Astro,
    x: f32,
    y: f32,
    ignorar: &[IdAstro],
) -> Result<Aterriza, Rechazo> {
    let mut debajo: Vec<&Astro> = astros
        .iter()
        .filter(|a| a.id != movido.id && !ignorar.contains(&a.id))
        .filter(|a| a.es_contenedor() && a.contiene(x, y))
        .collect();
    // El mas pequeno primero: es el que de verdad lo contiene.
    debajo.sort_by(|a, b| a.radio.total_cmp(&b.radio));
    let galaxia = debajo
        .iter()
        .find(|a| matches!(a.clase, Clase::Galaxia { .. }))
        .copied();
    let planeta = debajo
        .iter()
        .find(|a| matches!(a.clase, Clase::Planeta))
        .copied();

    match &movido.clase {
        Clase::Galaxia { .. } => match galaxia {
            Some(_) => Err(Rechazo::GalaxiaEncima),
            None => Ok(Aterriza {
                padre: None,
                aviso: false,
            }),
        },
        Clase::Planeta => {
            let aviso = planeta.is_some();
            let Some(g) = galaxia else {
                return Ok(Aterriza { padre: None, aviso });
            };
            let ajenas = astros
                .iter()
                .filter(|a| a.padre == Some(movido.id))
                .any(|l| l.proyecto() != g.proyecto());
            if ajenas {
                Err(Rechazo::ExtranjerasEnGalaxia)
            } else {
                Ok(Aterriza {
                    padre: Some(g.id),
                    aviso,
                })
            }
        }
        Clase::Luna { proyecto, .. } => {
            if let Some(p) = planeta {
                // El planeta manda si es exoplaneta; si esta en una galaxia,
                // tiene que ser la de la luna.
                let su_galaxia = p.padre.and_then(|g| astros.iter().find(|a| a.id == g));
                return match su_galaxia {
                    Some(g) if g.proyecto() != Some(proyecto.as_str()) => Err(Rechazo::OtraGalaxia),
                    _ => Ok(Aterriza {
                        padre: Some(p.id),
                        aviso: false,
                    }),
                };
            }
            match galaxia {
                Some(g) if g.proyecto() == Some(proyecto.as_str()) => Ok(Aterriza {
                    padre: Some(g.id),
                    aviso: false,
                }),
                Some(_) => Err(Rechazo::OtraGalaxia),
                None => Err(Rechazo::FueraDeGalaxia),
            }
        }
    }
}

/// Hijos y nietos, sin repetidos. La profundidad es 2 como mucho (D203),
/// pero se recorre con pila para no depender de ello.
pub fn descendientes(astros: &[Astro], id: IdAstro) -> Vec<IdAstro> {
    let mut salida = Vec::new();
    let mut pila = vec![id];
    while let Some(p) = pila.pop() {
        for a in astros.iter().filter(|a| a.padre == Some(p)) {
            if !salida.contains(&a.id) {
                salida.push(a.id);
                pila.push(a.id);
            }
        }
    }
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::astro::Astro;

    fn cielo() -> Vec<Astro> {
        let g1 = Astro::galaxia(IdAstro(1), "p1", 0.0, 0.0);
        let g2 = Astro::galaxia(IdAstro(2), "p2", 6000.0, 0.0);
        let mut pl = Astro::planeta(IdAstro(3), 500.0, 0.0, 400.0);
        pl.padre = Some(IdAstro(1));
        let exo = Astro::planeta(IdAstro(4), 3000.0, 3000.0, 400.0);
        vec![g1, g2, pl, exo]
    }

    #[test]
    fn una_luna_en_su_galaxia_aterriza_en_ella() {
        let c = cielo();
        let l = Astro::luna(IdAstro(9), "m:a", "p1", 0.0, 0.0);
        let r = aterrizaje(&c, &l, -1000.0, 0.0, &[]).unwrap();
        assert_eq!(r.padre, Some(IdAstro(1)));
    }

    #[test]
    fn una_luna_en_un_planeta_de_su_galaxia_es_del_planeta_el_contenedor_mas_pequeno() {
        let c = cielo();
        let l = Astro::luna(IdAstro(9), "m:a", "p1", 0.0, 0.0);
        let r = aterrizaje(&c, &l, 500.0, 100.0, &[]).unwrap();
        assert_eq!(r.padre, Some(IdAstro(3)));
    }

    #[test]
    fn una_luna_en_otra_galaxia_se_rechaza() {
        let c = cielo();
        let l = Astro::luna(IdAstro(9), "m:a", "p1", 0.0, 0.0);
        assert_eq!(
            aterrizaje(&c, &l, 6000.0, 0.0, &[]),
            Err(Rechazo::OtraGalaxia)
        );
    }

    #[test]
    fn una_luna_en_un_planeta_de_otra_galaxia_se_rechaza() {
        let c = cielo();
        let l = Astro::luna(IdAstro(9), "m:a", "p2", 0.0, 0.0);
        assert_eq!(
            aterrizaje(&c, &l, 500.0, 0.0, &[]),
            Err(Rechazo::OtraGalaxia)
        );
    }

    #[test]
    fn una_luna_de_cualquier_proyecto_entra_en_un_exoplaneta() {
        let c = cielo();
        let l = Astro::luna(IdAstro(9), "m:a", "p2", 0.0, 0.0);
        let r = aterrizaje(&c, &l, 3000.0, 3000.0, &[]).unwrap();
        assert_eq!(r.padre, Some(IdAstro(4)));
    }

    #[test]
    fn una_luna_en_el_vacio_se_rechaza() {
        let c = cielo();
        let l = Astro::luna(IdAstro(9), "m:a", "p1", 0.0, 0.0);
        assert_eq!(
            aterrizaje(&c, &l, -9000.0, 0.0, &[]),
            Err(Rechazo::FueraDeGalaxia)
        );
    }

    #[test]
    fn un_planeta_soltado_sobre_otro_es_de_la_galaxia_y_avisa() {
        let c = cielo();
        let p = Astro::planeta(IdAstro(9), 0.0, 0.0, 250.0);
        let r = aterrizaje(&c, &p, 500.0, 0.0, &[]).unwrap();
        assert_eq!(
            r,
            Aterriza {
                padre: Some(IdAstro(1)),
                aviso: true
            }
        );
    }

    #[test]
    fn un_planeta_en_el_vacio_es_un_exoplaneta() {
        let c = cielo();
        let p = Astro::planeta(IdAstro(9), 0.0, 0.0, 250.0);
        assert_eq!(
            aterrizaje(&c, &p, -9000.0, 0.0, &[]).unwrap(),
            Aterriza {
                padre: None,
                aviso: false
            }
        );
    }

    #[test]
    fn un_exoplaneta_con_lunas_de_otro_proyecto_no_entra_en_una_galaxia() {
        let mut c = cielo();
        let mut l = Astro::luna(IdAstro(9), "m:a", "p2", 3000.0, 3000.0);
        l.padre = Some(IdAstro(4));
        c.push(l);
        let exo = c[3].clone();
        assert_eq!(
            aterrizaje(&c, &exo, 0.0, 0.0, &[]),
            Err(Rechazo::ExtranjerasEnGalaxia)
        );
    }

    #[test]
    fn una_galaxia_no_se_suelta_encima_de_otra() {
        let c = cielo();
        let g = c[1].clone();
        assert_eq!(
            aterrizaje(&c, &g, 100.0, 0.0, &[]),
            Err(Rechazo::GalaxiaEncima)
        );
        assert!(aterrizaje(&c, &g, 20_000.0, 0.0, &[]).is_ok());
    }

    #[test]
    fn lo_que_se_ignora_no_cuenta_como_contenedor() {
        let c = cielo();
        let l = Astro::luna(IdAstro(9), "m:a", "p1", 0.0, 0.0);
        let r = aterrizaje(&c, &l, 500.0, 0.0, &[IdAstro(3)]).unwrap();
        assert_eq!(
            r.padre,
            Some(IdAstro(1)),
            "sin el planeta, cae en la galaxia"
        );
    }

    #[test]
    fn los_descendientes_incluyen_nietos_y_no_a_los_demas() {
        let mut c = cielo();
        let mut l = Astro::luna(IdAstro(9), "m:a", "p1", 500.0, 0.0);
        l.padre = Some(IdAstro(3));
        c.push(l);
        let mut d = descendientes(&c, IdAstro(1));
        d.sort();
        assert_eq!(d, vec![IdAstro(3), IdAstro(9)]);
        assert!(descendientes(&c, IdAstro(2)).is_empty());
    }
}
