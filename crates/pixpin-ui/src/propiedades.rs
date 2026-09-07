//! Que se puede ajustar de cada cosa.
//!
//! El panel ensena **solo lo que tiene sentido para lo seleccionado**: con
//! un trazo a mano no aparece «relleno», con un texto aparece la fuente y no
//! la rugosidad. Ensenar el control en gris es ruido, y en la pantalla de un
//! portatil viejo es ademas espacio robado al lienzo.
//!
//! El Android resuelve esto en `PanelLateral.kt` con 2.185 lineas; la tabla
//! en si esta en `DrawProperties.kt` (332 lineas, puras) y es lo que se
//! porta. Todo lo demas de aquellas 2.185 es Compose.

use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::gesto::Herramienta;

/// El orden de este enum es el orden en que salen los controles. No es
/// casual: si dependiera del orden de la seleccion, los controles bailarian
/// de sitio al elegir en distinto orden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Propiedad {
    ColorTrazo,
    Relleno,
    Grosor,
    Estilo,
    Rugosidad,
    Opacidad,
    Fuente,
    TamanoTexto,
    PuntaFlecha,
}

use Propiedad::*;

pub fn de_figura(f: &Figura) -> &'static [Propiedad] {
    match f {
        Figura::Lapiz { .. } => &[ColorTrazo, Grosor, Opacidad],
        // Grueso, translucido y liso a proposito (D45): resaltar sobre
        // texto tiene que dejarlo legible.
        Figura::Resaltador { .. } => &[ColorTrazo, Grosor, Opacidad],
        Figura::Linea { .. } => &[ColorTrazo, Grosor, Estilo, Rugosidad, Opacidad],
        Figura::Flecha { .. } => &[ColorTrazo, Grosor, Estilo, Rugosidad, Opacidad, PuntaFlecha],
        Figura::Rectangulo | Figura::Elipse => {
            &[ColorTrazo, Relleno, Grosor, Estilo, Rugosidad, Opacidad]
        }
        Figura::Texto { .. } => &[ColorTrazo, Opacidad, Fuente, TamanoTexto],
        // El foco oscurece lo de alrededor: su color es el del velo.
        Figura::Foco { .. } => &[Opacidad],
        Figura::Imagen { .. } => &[Opacidad],
    }
}

/// Lo que se puede ajustar antes de dibujar, cuando no hay nada elegido.
pub fn de_herramienta(h: Herramienta) -> &'static [Propiedad] {
    match h {
        // No dejan rastro: no hay nada que ajustar.
        Herramienta::Mano | Herramienta::Lupa | Herramienta::Borrador => &[],
        Herramienta::Lapiz | Herramienta::Resaltador => &[ColorTrazo, Grosor, Opacidad],
        Herramienta::Linea => &[ColorTrazo, Grosor, Estilo, Rugosidad, Opacidad],
        Herramienta::Flecha => &[ColorTrazo, Grosor, Estilo, Rugosidad, Opacidad, PuntaFlecha],
        Herramienta::Rectangulo | Herramienta::Elipse => {
            &[ColorTrazo, Relleno, Grosor, Estilo, Rugosidad, Opacidad]
        }
        Herramienta::Texto => &[ColorTrazo, Opacidad, Fuente, TamanoTexto],
        Herramienta::Foco => &[Opacidad],
    }
}

/// Lo que se puede ajustar de todos a la vez.
///
/// Solo lo comun: ensenar «rugosidad» con un texto elegido cambiaria algo
/// que el usuario no ve cambiar.
pub fn comunes(elementos: &[&Elemento]) -> Vec<Propiedad> {
    let Some((primero, resto)) = elementos.split_first() else {
        return Vec::new();
    };
    let mut fuera: Vec<Propiedad> = de_figura(&primero.figura)
        .iter()
        .copied()
        .filter(|p| resto.iter().all(|e| de_figura(&e.figura).contains(p)))
        .collect();
    // El orden del enum, no el de la seleccion.
    fuera.sort_unstable();
    fuera
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn elem(figura: Figura) -> Elemento {
        Elemento {
            id: 1,
            figura,
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 100.0,
            angulo: 0.0,
            trazo: pixpin_motor2d::elemento::ColorRgba::opaco(0.0, 0.0, 0.0),
            relleno: None,
            grosor: 2.0,
            estilo: pixpin_motor2d::elemento::EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
        }
    }

    #[test]
    fn un_trazo_a_mano_no_tiene_relleno() {
        // Una mancha de tinta no tiene interior que rellenar. Ensenar el
        // control en gris es ruido, y en un portatil viejo es ademas
        // espacio robado al lienzo.
        let p = de_figura(&Figura::Lapiz {
            puntos: Vec::new(),
            presiones: Vec::new(),
        });
        assert!(!p.contains(&Propiedad::Relleno));
        assert!(p.contains(&Propiedad::Grosor));
    }

    #[test]
    fn un_texto_tiene_fuente_y_no_rugosidad() {
        let p = de_figura(&Figura::Texto {
            texto: String::new(),
            tam: 16.0,
            familia: "Segoe UI".to_string(),
        });
        assert!(p.contains(&Propiedad::Fuente));
        assert!(p.contains(&Propiedad::TamanoTexto));
        assert!(
            !p.contains(&Propiedad::Rugosidad),
            "las letras no son a mano"
        );
    }

    #[test]
    fn un_rectangulo_si_tiene_relleno_y_rugosidad() {
        let p = de_figura(&Figura::Rectangulo);
        assert!(p.contains(&Propiedad::Relleno));
        assert!(p.contains(&Propiedad::Rugosidad));
    }

    #[test]
    fn una_flecha_tiene_puntas() {
        let p = de_figura(&Figura::Flecha {
            puntos: Vec::new(),
            punta_inicio: false,
            punta_fin: true,
        });
        assert!(p.contains(&Propiedad::PuntaFlecha));
    }

    #[test]
    fn el_resaltador_no_tiene_rugosidad_ni_estilo() {
        // Es grueso, translucido y liso a proposito: resaltar sobre texto
        // tiene que dejarlo legible.
        let p = de_figura(&Figura::Resaltador { puntos: Vec::new() });
        assert!(!p.contains(&Propiedad::Rugosidad));
        assert!(!p.contains(&Propiedad::Estilo));
    }

    #[test]
    fn con_varios_elegidos_solo_salen_las_propiedades_comunes() {
        // Un texto y un rectangulo comparten color y opacidad, nada mas.
        // Ensenar «rugosidad» con un texto elegido cambiaria algo que el
        // usuario no ve.
        let texto = elem(Figura::Texto {
            texto: String::new(),
            tam: 16.0,
            familia: "Segoe UI".to_string(),
        });
        let rect = elem(Figura::Rectangulo);
        let p = comunes(&[&texto, &rect]);

        assert!(p.contains(&Propiedad::ColorTrazo));
        assert!(p.contains(&Propiedad::Opacidad));
        assert!(!p.contains(&Propiedad::Rugosidad));
        assert!(!p.contains(&Propiedad::Fuente));
    }

    #[test]
    fn con_un_solo_elemento_comunes_da_lo_mismo_que_de_figura() {
        // El caso del 90 % del tiempo: elegir una sola cosa. Hoy funciona
        // por coincidencia porque las listas de `de_figura` estan escritas
        // en el mismo orden que declara el enum; si alguien reordena una,
        // esta prueba lo nota.
        let rect = elem(Figura::Rectangulo);
        assert_eq!(comunes(&[&rect]), de_figura(&rect.figura));
    }

    #[test]
    fn sin_nada_elegido_no_hay_panel() {
        assert!(comunes(&[]).is_empty());
    }

    #[test]
    fn las_comunes_salen_en_el_mismo_orden_siempre() {
        // El orden es el del enum. Si dependiera del orden de la
        // seleccion, los controles bailarian de sitio al elegir en
        // distinto orden, que es de las cosas mas molestas que puede hacer
        // una interfaz.
        let a = elem(Figura::Rectangulo);
        let b = elem(Figura::Elipse);
        assert_eq!(comunes(&[&a, &b]), comunes(&[&b, &a]));
    }

    #[test]
    fn la_herramienta_manda_cuando_no_hay_nada_elegido() {
        // Antes de dibujar tambien se eligen color y grosor.
        let p = de_herramienta(Herramienta::Lapiz);
        assert!(p.contains(&Propiedad::Grosor));
        assert!(!p.contains(&Propiedad::Relleno));
    }

    #[test]
    fn la_mano_y_la_lupa_no_ajustan_nada() {
        assert!(de_herramienta(Herramienta::Mano).is_empty());
        assert!(de_herramienta(Herramienta::Lupa).is_empty());
    }
}
