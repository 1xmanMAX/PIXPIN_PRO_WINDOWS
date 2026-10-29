//! **Lo que Jev saco de la reunion mientras se grababa** (29-oct-2026,
//! `docs/jev.md`, «En la reunión»): las decisiones, las preguntas que nadie
//! contesto, los minutos de cada tema, las tareas y eventos propuestos (con
//! lo que hizo la persona con ellos) y los cambios de urgencia de tareas
//! que ya existian. Va en el `resto` de la nota de voz, como los temas, para
//! que «Estudiar la clase», la pagina compartida y el movil lo lean igual.
//!
//! Lo puro, sin ventana ni red. Cada clave sale solo si tiene algo: una
//! reunion sin nada de esto deja la nota byte a byte como antes.
//!
//! ```json
//! "decisiones": [{"ms": 125000, "texto": "queda decidido: adelantamos el pedido"}],
//! "preguntasAbiertas": [{"ms": 300000, "texto": "¿quién paga el autobús?"}],
//! "tiempoPorTema": {"temas": [{"id": "t1", "ms": 360000}], "otro": 120000},
//! "propuestas": [{"id": "p1", "tipo": "tarea", "ms": 90000,
//!                 "texto": "yo mando la cotización el viernes",
//!                 "responsable": "Luis", "fecha": "2026-10-30", "hora": "15:00",
//!                 "prioridad": "alta", "estado": "aceptada"}],
//! "cambiosDeUrgencia": [{"ms": 400000, "tarea": "Pagar al proveedor",
//!                        "antes": "normal", "despues": "urgente",
//!                        "fechaAntes": "2026-11-05", "fechaDespues": "2026-10-29",
//!                        "deshecho": false}]
//! ```
//!
//! `ms` es siempre el milisegundo de la grabacion en que empezo la frase.
//! `tiempoPorTema.temas[].id` es el `id` de `resto["temas"]`. Una propuesta
//! es `tarea` o `evento`; `estado`, `pendiente` (nadie la toco), `aceptada`
//! (se apunto en Tareas) o `descartada`. `responsable`, `fecha`, `hora` y
//! `prioridad` solo si se saben. Un cambio de urgencia se aplica solo (es lo
//! unico que no espera a que se acepte) y `deshecho` dice si la persona lo
//! deshizo.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const DECISIONES: &str = "decisiones";
pub const PREGUNTAS_ABIERTAS: &str = "preguntasAbiertas";
pub const TIEMPO_POR_TEMA: &str = "tiempoPorTema";
pub const PROPUESTAS: &str = "propuestas";
pub const CAMBIOS_DE_URGENCIA: &str = "cambiosDeUrgencia";

/// Todas las claves de aqui (para copiarlas de una nota a otra).
pub const CLAVES: [&str; 5] = [
    DECISIONES,
    PREGUNTAS_ABIERTAS,
    TIEMPO_POR_TEMA,
    PROPUESTAS,
    CAMBIOS_DE_URGENCIA,
];

/// Algo dicho en un minuto: una decision o una pregunta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dicho {
    pub ms: i64,
    pub texto: String,
}

/// Lo que llevo un tema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TiempoDeTema {
    pub id: String,
    pub ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TiempoPorTema {
    pub temas: Vec<TiempoDeTema>,
    /// Lo que no fue de ningun tema de la agenda.
    #[serde(default)]
    pub otro: i64,
}

impl TiempoPorTema {
    /// Lo que llevo el tema `id`, si se conto.
    pub fn de(&self, id: &str) -> Option<i64> {
        self.temas.iter().find(|t| t.id == id).map(|t| t.ms)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tipo {
    Tarea,
    Evento,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EstadoPropuesta {
    #[default]
    Pendiente,
    Aceptada,
    Descartada,
}

/// **Una tarea o un evento propuesto** por lo dicho.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Propuesta {
    pub id: String,
    pub tipo: Tipo,
    pub ms: i64,
    pub texto: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub responsable: Option<String>,
    /// `AAAA-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fecha: Option<String>,
    /// `HH:MM`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hora: Option<String>,
    /// `urgente`, `alta`, `normal` o `baja` (los de `urgencia::Prioridad`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prioridad: Option<String>,
    #[serde(default)]
    pub estado: EstadoPropuesta,
}

/// **Un cambio de urgencia** de una tarea que ya existia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CambioDeUrgencia {
    pub ms: i64,
    pub tarea: String,
    pub antes: String,
    pub despues: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fecha_antes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fecha_despues: Option<String>,
    #[serde(default)]
    pub deshecho: bool,
}

/// **Todo lo de la reunion**, en el orden en que se dijo.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Acta {
    pub decisiones: Vec<Dicho>,
    pub preguntas_abiertas: Vec<Dicho>,
    pub tiempo_por_tema: Option<TiempoPorTema>,
    pub propuestas: Vec<Propuesta>,
    pub cambios: Vec<CambioDeUrgencia>,
}

impl Acta {
    pub fn vacia(&self) -> bool {
        self.decisiones.is_empty()
            && self.preguntas_abiertas.is_empty()
            && self.tiempo_por_tema.is_none()
            && self.propuestas.is_empty()
            && self.cambios.is_empty()
    }

    /// Las propuestas que nadie acepto ni descarto.
    pub fn pendientes(&self) -> impl Iterator<Item = &Propuesta> {
        self.propuestas
            .iter()
            .filter(|p| p.estado == EstadoPropuesta::Pendiente)
    }
}

/// Los elementos de una lista que se entienden (lo demas se salta).
fn lista<T: for<'de> Deserialize<'de>>(resto: &Map<String, Value>, k: &str) -> Vec<T> {
    resto
        .get(k)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| serde_json::from_value(x.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// **Lee el acta** de una nota (vacia si no tiene).
pub fn de(resto: &Map<String, Value>) -> Acta {
    let tiempo_por_tema = resto.get(TIEMPO_POR_TEMA).and_then(|v| {
        let o = v.as_object()?;
        let temas = o
            .get("temas")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| serde_json::from_value::<TiempoDeTema>(x.clone()).ok())
                    .map(|t| TiempoDeTema {
                        ms: t.ms.max(0),
                        ..t
                    })
                    .collect()
            })
            .unwrap_or_default();
        let otro = o.get("otro").and_then(Value::as_i64).unwrap_or(0).max(0);
        Some(TiempoPorTema { temas, otro })
    });
    Acta {
        decisiones: lista(resto, DECISIONES),
        preguntas_abiertas: lista(resto, PREGUNTAS_ABIERTAS),
        tiempo_por_tema,
        propuestas: lista(resto, PROPUESTAS),
        cambios: lista(resto, CAMBIOS_DE_URGENCIA),
    }
}

fn poner_lista<T: Serialize>(resto: &mut Map<String, Value>, k: &str, v: &[T]) {
    if v.is_empty() {
        resto.remove(k);
    } else if let Ok(x) = serde_json::to_value(v) {
        resto.insert(k.into(), x);
    }
}

/// **Escribe el acta** en la nota. Lo vacio se quita.
pub fn poner(resto: &mut Map<String, Value>, a: &Acta) {
    poner_lista(resto, DECISIONES, &a.decisiones);
    poner_lista(resto, PREGUNTAS_ABIERTAS, &a.preguntas_abiertas);
    poner_lista(resto, PROPUESTAS, &a.propuestas);
    poner_lista(resto, CAMBIOS_DE_URGENCIA, &a.cambios);
    match a
        .tiempo_por_tema
        .as_ref()
        .and_then(|t| serde_json::to_value(t).ok())
    {
        Some(v) => {
            resto.insert(TIEMPO_POR_TEMA.into(), v);
        }
        None => {
            resto.remove(TIEMPO_POR_TEMA);
        }
    }
}

/// «6 min», «45 s»: lo que llevo un tema, redondeado al minuto (por debajo
/// de uno, en segundos).
pub fn duracion_corta(ms: i64) -> String {
    let s = (ms.max(0) + 500) / 1000;
    if s < 60 {
        format!("{s} s")
    } else {
        format!("{} min", (s + 30) / 60)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn acta() -> Acta {
        Acta {
            decisiones: vec![Dicho {
                ms: 125_000,
                texto: "adelantamos el pedido".into(),
            }],
            preguntas_abiertas: vec![Dicho {
                ms: 300_000,
                texto: "¿quién paga el autobús?".into(),
            }],
            tiempo_por_tema: Some(TiempoPorTema {
                temas: vec![TiempoDeTema {
                    id: "t1".into(),
                    ms: 360_000,
                }],
                otro: 120_000,
            }),
            propuestas: vec![Propuesta {
                id: "p1".into(),
                tipo: Tipo::Tarea,
                ms: 90_000,
                texto: "yo mando la cotización el viernes".into(),
                responsable: None,
                fecha: Some("2026-10-30".into()),
                hora: None,
                prioridad: Some("normal".into()),
                estado: EstadoPropuesta::Aceptada,
            }],
            cambios: vec![CambioDeUrgencia {
                ms: 400_000,
                tarea: "Pagar al proveedor".into(),
                antes: "normal".into(),
                despues: "urgente".into(),
                fecha_antes: None,
                fecha_despues: Some("2026-10-29".into()),
                deshecho: true,
            }],
        }
    }

    #[test]
    fn el_acta_va_y_vuelve_con_los_nombres_del_movil() {
        let mut r = Map::new();
        poner(&mut r, &acta());
        let j = serde_json::to_string(&Value::Object(r.clone())).unwrap();
        for k in [
            "\"preguntasAbiertas\"",
            "\"tiempoPorTema\"",
            "\"fechaDespues\"",
            "\"estado\":\"aceptada\"",
            "\"tipo\":\"tarea\"",
            "\"deshecho\":true",
        ] {
            assert!(j.contains(k), "{k} en {j}");
        }
        // Lo que no se sabe no sale.
        assert!(!j.contains("responsable"), "{j}");
        assert_eq!(de(&r), acta());
        assert_eq!(acta().tiempo_por_tema.unwrap().de("t1"), Some(360_000));
    }

    #[test]
    fn sin_nada_la_nota_no_cambia_y_lo_raro_se_salta() {
        let mut r = Map::new();
        r.insert("otra".into(), Value::from(1));
        let antes = r.clone();
        poner(&mut r, &Acta::default());
        assert_eq!(r, antes);
        assert!(de(&r).vacia());
        // Lo que no se entiende se salta; lo demas se lee.
        let r: Map<String, Value> = serde_json::from_str(
            r#"{"decisiones": [{"ms": 1, "texto": "sí"}, {"texto": 3}],
                "propuestas": [{"id": "p1", "tipo": "evento", "ms": 5, "texto": "nos vemos"}],
                "tiempoPorTema": {"temas": [{"id": "t1", "ms": -4}]}}"#,
        )
        .unwrap();
        let a = de(&r);
        assert_eq!(a.decisiones.len(), 1);
        assert_eq!(a.propuestas[0].estado, EstadoPropuesta::Pendiente);
        assert_eq!(a.pendientes().count(), 1);
        assert_eq!(a.tiempo_por_tema.unwrap().temas[0].ms, 0);
    }

    #[test]
    fn la_duracion_corta() {
        assert_eq!(duracion_corta(44_000), "44 s");
        assert_eq!(duracion_corta(60_000), "1 min");
        assert_eq!(duracion_corta(359_000), "6 min");
        assert_eq!(duracion_corta(-3), "0 s");
    }
}
