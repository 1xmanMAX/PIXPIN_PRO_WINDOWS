//! Que se pinta de cada astro segun lo grande que se vea (D218).
//!
//! No depende del zoom sino del radio en pantalla: a un mismo zoom una
//! galaxia enorme y una luna diminuta no pueden pintarse igual.

use crate::astro::{Astro, Clase};

/// Por debajo de esto no se distingue una galaxia de otra: el universo deja
/// alejarse mas que el resto de lienzos (ver el plan, «Ajustes»).
pub const ZOOM_MINIMO_UNIVERSO: f32 = 0.002;
/// Se baja de nivel al quedar un 15 % por debajo del umbral por el que se
/// subio: un zoom que roza el umbral no hace parpadear ni recargar.
pub const HISTERESIS: f32 = 0.85;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Nivel {
    Oculto,
    Punto,
    Disco,
    Icono,
    Ficha,
    Vista,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Galaxia,
    Planeta,
    Luna,
}

const GALAXIA: [(f32, Nivel); 3] = [
    (0.0, Nivel::Punto),
    (6.0, Nivel::Disco),
    (120.0, Nivel::Vista),
];
const PLANETA: [(f32, Nivel); 3] = [
    (0.0, Nivel::Oculto),
    (6.0, Nivel::Disco),
    (40.0, Nivel::Icono),
];
const LUNA: [(f32, Nivel); 5] = [
    (0.0, Nivel::Oculto),
    (4.0, Nivel::Punto),
    (16.0, Nivel::Icono),
    (48.0, Nivel::Ficha),
    (120.0, Nivel::Vista),
];

fn tabla(t: Tipo) -> &'static [(f32, Nivel)] {
    match t {
        Tipo::Galaxia => &GALAXIA,
        Tipo::Planeta => &PLANETA,
        Tipo::Luna => &LUNA,
    }
}

pub fn tipo_de(a: &Astro) -> Tipo {
    match a.clase {
        Clase::Galaxia { .. } => Tipo::Galaxia,
        Clase::Planeta => Tipo::Planeta,
        Clase::Luna { .. } => Tipo::Luna,
    }
}

pub fn nivel(t: Tipo, radio_px: f32) -> Nivel {
    tabla(t)
        .iter()
        .rev()
        .find(|(u, _)| radio_px >= *u)
        .map(|(_, n)| *n)
        .unwrap_or(tabla(t)[0].1)
}

fn umbral_de(t: Tipo, n: Nivel) -> Option<f32> {
    tabla(t).iter().find(|(_, m)| *m == n).map(|(u, _)| *u)
}

/// El nivel recordando el del fotograma anterior: subir es inmediato, bajar
/// espera a quedar un 15 % por debajo.
pub fn nivel_con_memoria(t: Tipo, radio_px: f32, antes: Option<Nivel>) -> Nivel {
    let crudo = nivel(t, radio_px);
    match antes {
        Some(a) if a > crudo => match umbral_de(t, a) {
            Some(u) if radio_px >= u * HISTERESIS => a,
            _ => crudo,
        },
        _ => crudo,
    }
}

/// El radio en pantalla por debajo del cual un astro de este tipo sale
/// `Oculto` seguro, recuerde lo que recuerde: el umbral del primer nivel
/// visible con la histeresis ya descontada. `0` si el tipo nunca se oculta.
pub fn radio_siempre_oculto(t: Tipo) -> f32 {
    tabla(t)
        .iter()
        .find(|(_, n)| *n != Nivel::Oculto)
        .filter(|_| tabla(t)[0].1 == Nivel::Oculto)
        .map(|(u, _)| u * HISTERESIS)
        .unwrap_or(0.0)
}

/// Si a este nivel se ve lo de dentro. Una galaxia cerrada es un disco,
/// aunque tenga tres mil lunas (D238.2).
pub fn abre_hijos(a: &Astro, n: Nivel) -> bool {
    match a.clase {
        Clase::Galaxia { .. } => n >= Nivel::Vista,
        Clase::Planeta => n >= Nivel::Icono,
        Clase::Luna { .. } => false,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn por_debajo_del_radio_siempre_oculto_no_se_ve_ni_con_memoria() {
        let r = radio_siempre_oculto(Tipo::Luna);
        assert!((r - 3.4).abs() < 1e-4, "{r}");
        for antes in [None, Some(Nivel::Punto), Some(Nivel::Vista)] {
            assert_eq!(
                nivel_con_memoria(Tipo::Luna, r - 0.01, antes),
                Nivel::Oculto
            );
        }
        // Justo en el umbral, recordando el punto, todavia se ve.
        assert_eq!(
            nivel_con_memoria(Tipo::Luna, r, Some(Nivel::Punto)),
            Nivel::Punto
        );
        // Caso negativo: una galaxia no se oculta nunca.
        assert_eq!(radio_siempre_oculto(Tipo::Galaxia), 0.0);
    }

    #[test]
    fn cada_tipo_cambia_de_nivel_en_sus_umbrales() {
        assert_eq!(nivel(Tipo::Galaxia, 5.9), Nivel::Punto);
        assert_eq!(nivel(Tipo::Galaxia, 6.0), Nivel::Disco);
        assert_eq!(nivel(Tipo::Galaxia, 120.0), Nivel::Vista);
        assert_eq!(nivel(Tipo::Planeta, 5.0), Nivel::Oculto);
        assert_eq!(nivel(Tipo::Planeta, 39.0), Nivel::Disco);
        assert_eq!(nivel(Tipo::Planeta, 40.0), Nivel::Icono);
        assert_eq!(nivel(Tipo::Luna, 3.9), Nivel::Oculto);
        assert_eq!(nivel(Tipo::Luna, 4.0), Nivel::Punto);
        assert_eq!(nivel(Tipo::Luna, 16.0), Nivel::Icono);
        assert_eq!(nivel(Tipo::Luna, 48.0), Nivel::Ficha);
        assert_eq!(nivel(Tipo::Luna, 120.0), Nivel::Vista);
    }

    #[test]
    fn se_sube_en_el_umbral_y_no_se_baja_hasta_quedar_un_quince_por_ciento_por_debajo() {
        // Sube en 120.
        assert_eq!(
            nivel_con_memoria(Tipo::Luna, 120.0, Some(Nivel::Ficha)),
            Nivel::Vista
        );
        // A 110 sigue en Vista: 120 * 0,85 = 102.
        assert_eq!(
            nivel_con_memoria(Tipo::Luna, 110.0, Some(Nivel::Vista)),
            Nivel::Vista
        );
        assert_eq!(
            nivel_con_memoria(Tipo::Luna, 102.0, Some(Nivel::Vista)),
            Nivel::Vista
        );
        // Por debajo de 102 baja.
        assert_eq!(
            nivel_con_memoria(Tipo::Luna, 101.0, Some(Nivel::Vista)),
            Nivel::Ficha
        );
    }

    #[test]
    fn sin_memoria_manda_el_nivel_crudo() {
        assert_eq!(nivel_con_memoria(Tipo::Luna, 110.0, None), Nivel::Ficha);
    }

    #[test]
    fn una_galaxia_abre_sus_hijos_solo_en_vista_y_un_planeta_desde_icono() {
        let g = crate::astro::Astro::galaxia(crate::astro::IdAstro(1), "p", 0.0, 0.0);
        let p = crate::astro::Astro::planeta(crate::astro::IdAstro(2), 0.0, 0.0, 250.0);
        assert!(!abre_hijos(&g, Nivel::Disco));
        assert!(abre_hijos(&g, Nivel::Vista));
        assert!(!abre_hijos(&p, Nivel::Disco));
        assert!(abre_hijos(&p, Nivel::Icono));
    }
}
