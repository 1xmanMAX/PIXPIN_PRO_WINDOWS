//! **Que herramientas de dibujo salen** (`[herramientas]` del TOML).
//!
//! Las herramientas de dibujo son las mismas en el lienzo, el lector, los
//! pines y el anotador de pantalla (ver
//! `docs/superpowers/specs/2026-09-24-herramientas-unicas-design.md`), y el
//! usuario pidio poder apagar algunas desde los ajustes generales. Una
//! apagada desaparece de la barra y de su tecla en TODOS esos sitios.
//!
//! Se guarda la lista de las APAGADAS y no una casilla por herramienta:
//!
//! - una herramienta que se anada manana nace activa sin tocar el fichero de
//!   nadie (con casillas, un fichero viejo no la nombraria y habria que
//!   decidir que significa su ausencia);
//! - un nombre que no se conoce (de una version mas nueva, o mal escrito) se
//!   ignora en vez de romper la carga de los ajustes.
//!
//! Los nombres son los estables de [`NOMBRES`]; el titulo que ve el usuario
//! sale de `herramientas-<nombre>` en los `.ftl`.

use serde::{Deserialize, Serialize};

/// Los nombres estables de las herramientas que se pueden apagar. La ventana
/// de ajustes no sigue este orden: las ensena por los grupos de la barra.
///
/// La mano no esta: sin ella no se puede ni elegir lo dibujado para moverlo
/// o borrarlo, y apagarla dejaria cualquier anfitrion a medias. Deshacer,
/// rehacer y salir tampoco, que no son herramientas sino acciones.
pub const NOMBRES: &[&str] = &[
    "lazo",
    "bolita",
    "rectangulo",
    "rombo",
    "elipse",
    "flecha",
    "flecha-codos",
    "flecha-libre",
    "linea",
    "lapiz",
    "grafito",
    "texto",
    "imagen",
    "borrador",
    "laser",
    "resaltador",
    "foco",
    "lupa",
    "mosaico",
    "arco",
    "serie",
    "punto",
    "cota",
    "escalar",
    "escala-grafica",
    "marco",
    "zona",
    "cronograma",
    "relleno",
    "recortar",
    "extender",
    "nudo",
    "copiar-estilo",
    "figuras",
    // Las dos que sacan el dibujo del lienzo (el grupo «sacar» de la barra).
    "imprimir",
    "compartir",
];

/// Seccion `[herramientas]`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Herramientas {
    /// Los nombres de las apagadas. Vacia de fabrica: todas activas.
    pub apagadas: Vec<String>,
}

impl Herramientas {
    /// Si la herramienta de ese nombre sale. Sin distinguir mayusculas: el
    /// fichero lo escribe a veces una persona.
    pub fn activa(&self, nombre: &str) -> bool {
        !self
            .apagadas
            .iter()
            .any(|a| a.trim().eq_ignore_ascii_case(nombre))
    }

    /// Enciende o apaga una. Apagar dos veces no la repite en la lista, y
    /// encender una que no estaba apagada no hace nada.
    pub fn poner(&mut self, nombre: &str, activa: bool) {
        self.apagadas
            .retain(|a| !a.trim().eq_ignore_ascii_case(nombre));
        if !activa {
            self.apagadas.push(nombre.to_string());
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn de_fabrica_estan_todas_activas() {
        let h = Herramientas::default();
        for n in NOMBRES {
            assert!(h.activa(n), "{n}");
        }
    }

    #[test]
    fn apagar_una_no_apaga_las_demas_y_encenderla_la_devuelve() {
        let mut h = Herramientas::default();
        h.poner("lazo", false);
        assert!(!h.activa("lazo"));
        assert!(h.activa("lapiz"));
        h.poner("lazo", false);
        assert_eq!(h.apagadas.len(), 1, "apagar dos veces no la repite");
        h.poner("lazo", true);
        assert!(h.activa("lazo"));
        assert!(h.apagadas.is_empty());
    }

    #[test]
    fn un_nombre_escrito_a_mano_con_mayusculas_o_espacios_vale_igual() {
        let h = Herramientas {
            apagadas: vec![" Grafito ".into()],
        };
        assert!(!h.activa("grafito"));
    }

    #[test]
    fn un_nombre_desconocido_no_rompe_nada_ni_apaga_nada() {
        let a: crate::Ajustes =
            toml::from_str("[herramientas]\napagadas = [\"teletransporte\"]").unwrap();
        for n in NOMBRES {
            assert!(a.herramientas.activa(n), "{n}");
        }
    }

    #[test]
    fn la_seccion_va_y_vuelve_por_el_toml() {
        let mut a = crate::Ajustes::default();
        a.herramientas.poner("mosaico", false);
        a.herramientas.poner("cota", false);
        let texto = toml::to_string_pretty(&a).unwrap();
        let vuelta: crate::Ajustes = toml::from_str(&texto).unwrap();
        assert_eq!(vuelta.herramientas, a.herramientas);
        // Un fichero de antes, sin la seccion, sale con todas activas.
        let viejo: crate::Ajustes = toml::from_str("idioma = \"espanol\"").unwrap_or_default();
        assert!(viejo.herramientas.apagadas.is_empty());
    }

    #[test]
    fn los_nombres_no_se_repiten() {
        let mut v: Vec<&str> = NOMBRES.to_vec();
        v.sort_unstable();
        v.dedup();
        assert_eq!(v.len(), NOMBRES.len());
    }
}
