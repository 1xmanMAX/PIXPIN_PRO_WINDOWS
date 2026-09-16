//! Lo que se dicen los dos aparatos, en JSON.
//!
//! Las mismas clases que `sincro/Protocolo.kt`. Dos reglas de kotlinx que hay
//! que respetar o el movil no entiende lo que llega:
//!
//! - `encodeDefaults = false`: **un campo con su valor por omision no se
//!   escribe**. Por eso aqui va `skip_serializing_if` en casi todo; escribir
//!   `"bytes": 0` donde Android no escribe nada cambia el JSON, y con el los
//!   resumenes que se calculan sobre texto.
//! - `ignoreUnknownKeys = true`: lo que no se conoce se ignora en vez de
//!   fallar, que es lo que permite que una version mas nueva del movil hable
//!   con una mas vieja de aqui.

use serde::{Deserialize, Serialize};

fn es_cero_i64(v: &i64) -> bool {
    *v == 0
}

fn es_cero_u32(v: &u32) -> bool {
    *v == 0
}

fn es_falso(v: &bool) -> bool {
    !*v
}

/// Un aparato del grupo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Aparato {
    pub id: String,
    pub nombre: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub letra: Option<String>,
    #[serde(skip_serializing_if = "es_cero_i64")]
    pub desde: i64,
}

/// El saludo: quien soy, quien mas hay, que hora tengo y que version hablo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Hola {
    pub yo: Aparato,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub miembros: Vec<Aparato>,
    /// El reloj del que saluda, en milisegundos. Con el se calcula cuanto va
    /// adelantado el otro, y esa correccion se usa en toda la fusion.
    #[serde(skip_serializing_if = "es_cero_i64")]
    pub reloj: i64,
    pub version: u32,
    /// Quien llega sin letra pide una.
    #[serde(skip_serializing_if = "es_falso")]
    pub unirme: bool,
    #[serde(skip_serializing_if = "es_cero_u32")]
    pub puerto: u32,
}

impl Default for Hola {
    fn default() -> Self {
        Hola {
            yo: Aparato::default(),
            miembros: Vec::new(),
            reloj: 0,
            // La version no lleva `skip`: se escribe siempre, porque es lo
            // unico que evita que dos versiones incompatibles se destrocen.
            version: crate::VERSION,
            unirme: false,
            puerto: 0,
        }
    }
}

/// Una conversacion del otro aparato, tal como la lista.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Chat {
    pub id: String,
    pub nombre: String,
    #[serde(skip_serializing_if = "es_cero_u32")]
    pub mensajes: u32,
    #[serde(skip_serializing_if = "es_cero_i64")]
    pub tocado: i64,
}

/// Una linea del inventario: lo justo para saber si algo cambio sin mandarlo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Apunte {
    /// La sena: el codigo unico del mensaje, o la ruta si es un archivo.
    pub sena: String,
    #[serde(skip_serializing_if = "es_cero_i64")]
    pub creado: i64,
    #[serde(skip_serializing_if = "es_cero_i64")]
    pub tocado: i64,
    /// SHA-256 de su forma canonica. Sin esto no se sabe si de verdad cambio,
    /// y se acabaria mandando todo cada vez.
    pub resumen: String,
    #[serde(skip_serializing_if = "es_falso")]
    pub borrado: bool,
    /// El codigo de chat (`47·K7Q2`), si lo tiene.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

/// Un archivo del otro lado.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ArchivoInfo {
    pub ruta: String,
    /// Resumen de su forma canonica (los de texto) o de sus bytes.
    pub resumen: String,
    #[serde(skip_serializing_if = "es_cero_i64")]
    pub bytes: i64,
    #[serde(skip_serializing_if = "es_cero_i64")]
    pub tocado: i64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub etiqueta: String,
    /// El resumen de los bytes tal cual, que es lo que se usaba antes de que
    /// los de texto se resumieran en forma canonica.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub crudo: String,
}

/// Lo acordado la ultima vez: que resumen tenia cada cosa cuando los dos
/// quedaron iguales.
///
/// Es lo que permite saber quien cambio que. Si los dos no recuerdan lo
/// mismo, se hace como la primera vez: juntar todo y no pisar nada.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Base {
    pub mensajes: std::collections::BTreeMap<String, String>,
    pub archivos: std::collections::BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proyecto: Option<String>,
}

/// Lo que pide quien conduce la sincronizacion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Peticion {
    /// Que se pide: `hola`, `catalogo`, `inventario`, `mensajes`, `aplicar`,
    /// `archivos`, `pon`, `parche`, `damecambios`, `dame`, `base`, `adios`.
    pub t: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hola: Option<Hola>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub senas: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub poner: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub borrar: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proyecto: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ruta: Option<String>,
    #[serde(skip_serializing_if = "es_cero_i64")]
    pub bytes: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<Base>,
    /// El parche viaja como CADENA con JSON dentro, no como objeto.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parche: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desde: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resumen: Option<String>,
}

impl Peticion {
    /// Una peticion de solo tipo, que son la mayoria.
    pub fn de(t: &str) -> Peticion {
        Peticion {
            t: t.to_string(),
            ..Default::default()
        }
    }

    /// Una peticion sobre una conversacion.
    pub fn sobre(t: &str, chat: &str) -> Peticion {
        Peticion {
            t: t.to_string(),
            chat: Some(chat.to_string()),
            ..Default::default()
        }
    }
}

/// Lo que contesta el otro. `error` lleno significa que no se hizo.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Respuesta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hola: Option<Hola>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub chats: Vec<Chat>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub apuntes: Vec<Apunte>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proyecto: Option<String>,
    #[serde(rename = "selloDeBase", skip_serializing_if = "Option::is_none")]
    pub sello_de_base: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mensajes: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub archivos: Vec<ArchivoInfo>,
    #[serde(skip_serializing_if = "es_cero_i64")]
    pub bytes: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resumen: Option<String>,
    #[serde(skip_serializing_if = "es_falso")]
    pub saltado: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parche: Option<String>,
    #[serde(rename = "faltaBase", skip_serializing_if = "es_falso")]
    pub falta_base: bool,
}

/// El texto que manda Android cuando ya esta sincronizando con otro.
pub const OCUPADO: &str = "Está sincronizando con otro aparato. Prueba otra vez en un momento.";

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_peticion_sencilla_no_escribe_campos_vacios() {
        // Android no escribe los campos con su valor por omision; si aqui se
        // escribieran, el JSON no seria el mismo y los resumenes que se
        // calculan sobre texto dejarian de cuadrar.
        let json = serde_json::to_string(&Peticion::de("catalogo")).unwrap();
        assert_eq!(json, r#"{"t":"catalogo"}"#);
        let json = serde_json::to_string(&Peticion::sobre("inventario", "general")).unwrap();
        assert_eq!(json, r#"{"t":"inventario","chat":"general"}"#);
    }

    #[test]
    fn una_respuesta_vacia_es_un_objeto_vacio() {
        assert_eq!(serde_json::to_string(&Respuesta::default()).unwrap(), "{}");
        // Y la que lleva algo, solo eso.
        let r = Respuesta {
            saltado: true,
            ..Default::default()
        };
        assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"saltado":true}"#);
    }

    #[test]
    fn los_nombres_de_android_se_respetan_tal_cual() {
        let r = Respuesta {
            sello_de_base: Some("abc".into()),
            falta_base: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains(r#""selloDeBase":"abc""#), "{json}");
        assert!(json.contains(r#""faltaBase":true"#), "{json}");
    }

    #[test]
    fn lo_que_no_se_conoce_se_ignora_en_vez_de_fallar() {
        // Una version mas nueva del movil manda campos que aqui no existen:
        // tiene que poder hablar con esta igualmente.
        let json = r#"{"t":"hola","cosaNueva":{"x":1},"chat":"general"}"#;
        let p: Peticion = serde_json::from_str(json).unwrap();
        assert_eq!(p.t, "hola");
        assert_eq!(p.chat.as_deref(), Some("general"));
    }

    #[test]
    fn el_saludo_lleva_siempre_la_version() {
        let h = Hola::default();
        assert_eq!(h.version, crate::VERSION);
        let json = serde_json::to_string(&h).unwrap();
        assert!(json.contains(r#""version":3"#), "{json}");
    }

    #[test]
    fn un_apunte_va_y_vuelve_igual() {
        let a = Apunte {
            sena: "RVK5YHKCX7".into(),
            creado: 1_725_500_000_000,
            tocado: 1_725_500_001_000,
            resumen: "abc123".into(),
            borrado: false,
            alias: Some("47·K7Q2".into()),
        };
        let json = serde_json::to_string(&a).unwrap();
        // `borrado: false` no se escribe.
        assert!(!json.contains("borrado"), "{json}");
        assert_eq!(serde_json::from_str::<Apunte>(&json).unwrap(), a);
    }
}
