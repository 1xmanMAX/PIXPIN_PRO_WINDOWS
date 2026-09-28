//! Lo que hay en el cielo: galaxias, planetas y lunas, y las lineas entre
//! ellos.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Radio en mundo de una galaxia nueva (D229).
pub const RADIO_GALAXIA: f32 = 2000.0;
/// Radios de planeta por tamano (D223).
pub const RADIO_PLANETA_S: f32 = 250.0;
pub const RADIO_PLANETA_M: f32 = 400.0;
pub const RADIO_PLANETA_L: f32 = 700.0;
/// Una luna es una ficha de 96 de lado: la mitad es su radio.
pub const RADIO_LUNA: f32 = 48.0;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
#[serde(transparent)]
pub struct IdAstro(pub u64);

/// De que es cada astro. El exoplaneta no es una clase: es un planeta sin
/// padre. Una clase mas seria una segunda verdad sobre lo mismo, y habria
/// que cambiarla cada vez que se saca un planeta de su galaxia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "tipo", rename_all = "lowercase")]
pub enum Clase {
    /// Un proyecto del chat. `proyecto` es el `id` de su ficha en el indice.
    Galaxia { proyecto: String },
    #[default]
    Planeta,
    /// Un archivo del chat. `codigo` es `Mensaje::codigo_unico` (D242): ni
    /// la ruta ni la posicion, que cambian.
    Luna { codigo: String, proyecto: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Astro {
    pub id: IdAstro,
    pub clase: Clase,
    /// Centro en mundo.
    pub x: f32,
    pub y: f32,
    pub radio: f32,
    /// Se recalcula solo al soltar (D205).
    pub padre: Option<IdAstro>,
    /// Solo de planetas: galaxias y lunas lo toman del chat al pintar.
    pub nombre: String,
    pub emoji: Option<String>,
    /// `0xRRGGBB`.
    pub color: Option<u32>,
    /// La nota del astro (D223).
    pub nota: String,
    /// Solo galaxias: si las notas del chat son lunas (D200).
    pub notas_del_chat: bool,
    /// Lo puso el armado del chat (H2) y cuelga del sol con su vinculo, como
    /// `alternarVinculo(Espacio.SOL, id)` del movil: se le pinta su orbita y
    /// su raya al sol. Lo colocado a mano no lo lleva.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub atado: bool,
    /// Solo planetas: el codigo de la luna que hace de sol dentro de el. Es
    /// el subespacio del movil (`conSubespacio`) llevado a la jerarquia fija
    /// del PC: el archivo del chat en el centro y lo que le contesta en
    /// orbita a su alrededor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sistema_de: Option<String>,
    #[serde(flatten)]
    pub resto: Map<String, Value>,
}

impl Default for Astro {
    fn default() -> Self {
        Self {
            id: IdAstro(0),
            clase: Clase::Planeta,
            x: 0.0,
            y: 0.0,
            radio: RADIO_PLANETA_M,
            padre: None,
            nombre: String::new(),
            emoji: None,
            color: None,
            nota: String::new(),
            notas_del_chat: false,
            atado: false,
            sistema_de: None,
            resto: Map::new(),
        }
    }
}

impl Astro {
    pub fn galaxia(id: IdAstro, proyecto: &str, x: f32, y: f32) -> Self {
        Self {
            id,
            clase: Clase::Galaxia {
                proyecto: proyecto.to_string(),
            },
            x,
            y,
            radio: RADIO_GALAXIA,
            ..Default::default()
        }
    }

    pub fn planeta(id: IdAstro, x: f32, y: f32, radio: f32) -> Self {
        Self {
            id,
            clase: Clase::Planeta,
            x,
            y,
            radio,
            ..Default::default()
        }
    }

    pub fn luna(id: IdAstro, codigo: &str, proyecto: &str, x: f32, y: f32) -> Self {
        Self {
            id,
            clase: Clase::Luna {
                codigo: codigo.to_string(),
                proyecto: proyecto.to_string(),
            },
            x,
            y,
            radio: RADIO_LUNA,
            ..Default::default()
        }
    }

    pub fn contiene(&self, x: f32, y: f32) -> bool {
        let (dx, dy) = (x - self.x, y - self.y);
        dx * dx + dy * dy <= self.radio * self.radio
    }

    /// Si una caja `(x0, y0, x1, y1)` cabe entera: sus cuatro esquinas
    /// dentro del circulo. Es la regla de las anotaciones (D204), la misma
    /// que la del marco con su rectangulo.
    pub fn contiene_caja(&self, c: (f32, f32, f32, f32)) -> bool {
        let (x0, y0, x1, y1) = c;
        [(x0, y0), (x1, y0), (x0, y1), (x1, y1)]
            .iter()
            .all(|(x, y)| self.contiene(*x, *y))
    }

    pub fn caja(&self) -> (f32, f32, f32, f32) {
        (
            self.x - self.radio,
            self.y - self.radio,
            self.x + self.radio,
            self.y + self.radio,
        )
    }

    pub fn es_contenedor(&self) -> bool {
        !matches!(self.clase, Clase::Luna { .. })
    }

    pub fn proyecto(&self) -> Option<&str> {
        match &self.clase {
            Clase::Galaxia { proyecto } | Clase::Luna { proyecto, .. } => Some(proyecto),
            Clase::Planeta => None,
        }
    }

    pub fn codigo(&self) -> Option<&str> {
        match &self.clase {
            Clase::Luna { codigo, .. } => Some(codigo),
            _ => None,
        }
    }
}

/// Que dice una linea (§2.3 de la spec).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "tipo", content = "n", rename_all = "lowercase")]
pub enum TipoConexion {
    #[default]
    Relacion,
    Depende,
    Referencia,
    /// El numero que se pinta en medio.
    Secuencia(u32),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Conexion {
    pub id: u64,
    pub desde: IdAstro,
    pub hasta: IdAstro,
    /// Anidado y no aplanado: un campo aplanado no admite `default`, y una
    /// conexion sin tipo tiene que leerse como relacion.
    pub tipo: TipoConexion,
    pub rotulo: String,
    pub color: Option<u32>,
}

impl Conexion {
    pub fn nueva(id: u64, desde: IdAstro, hasta: IdAstro) -> Self {
        Self {
            id,
            desde,
            hasta,
            ..Default::default()
        }
    }

    pub fn toca(&self, id: IdAstro) -> bool {
        self.desde == id || self.hasta == id
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_punto_dentro_del_circulo_esta_contenido_y_uno_fuera_no() {
        let p = Astro::planeta(IdAstro(1), 0.0, 0.0, 100.0);
        assert!(p.contiene(60.0, 60.0));
        assert!(p.contiene(100.0, 0.0), "el borde cuenta");
        assert!(!p.contiene(80.0, 80.0));
    }

    #[test]
    fn una_caja_esta_dentro_solo_si_caben_sus_cuatro_esquinas() {
        let p = Astro::planeta(IdAstro(1), 0.0, 0.0, 100.0);
        assert!(p.contiene_caja((-50.0, -50.0, 50.0, 50.0)));
        assert!(!p.contiene_caja((-50.0, -50.0, 90.0, 90.0)));
    }

    #[test]
    fn la_luna_no_contiene_y_la_galaxia_y_el_planeta_si() {
        assert!(!Astro::luna(IdAstro(1), "m:a", "p1", 0.0, 0.0).es_contenedor());
        assert!(Astro::galaxia(IdAstro(2), "p1", 0.0, 0.0).es_contenedor());
        assert!(Astro::planeta(IdAstro(3), 0.0, 0.0, 250.0).es_contenedor());
    }

    #[test]
    fn un_astro_va_y_vuelve_por_json_con_sus_campos_desconocidos() {
        let texto = r#"{"id":7,"clase":{"tipo":"luna","codigo":"m:x","proyecto":"p"},
            "x":1.5,"y":-2,"radio":48,"futuro":{"a":1}}"#;
        let a: Astro = serde_json::from_str(texto).unwrap();
        assert_eq!(a.codigo(), Some("m:x"));
        assert_eq!(a.proyecto(), Some("p"));
        let vuelta = serde_json::to_string(&a).unwrap();
        assert!(
            vuelta.contains("\"futuro\""),
            "no se pierde lo que no se entiende"
        );
    }

    #[test]
    fn una_conexion_sin_tipo_es_una_relacion() {
        let c: Conexion = serde_json::from_str(r#"{"id":1,"desde":1,"hasta":2}"#).unwrap();
        assert_eq!(c.tipo, TipoConexion::Relacion);
    }
}
