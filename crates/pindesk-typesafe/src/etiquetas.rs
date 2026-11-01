//! **Etiquetas solas para notas y capturas** (29-oct-2026, prototipo
//! `etiquetas_solas`, ver `docs/probar-ideas-sin-interfaz.md`, «Más
//! casos»): de las etiquetas que el usuario ya tiene, cuales le van a una
//! nota (o al texto de una captura). Un Noul por etiqueta (puede valer mas
//! de una), con la etiqueta dentro; nunca se inventa una etiqueta nueva.
//!
//! **Tabla** ([`decidir`]):
//!
//! | probabilidad | se hace |
//! |---|---|
//! | ≥ SI | se pone (las [`COMO_MUCHO`] mas probables) |
//! | entre [`SUGERIR`] y SI, si no se puso ninguna | se sugiere (dos como mucho) |
//! | lo demas | nada |
//!
//! Una nota sin texto (menos de [`PALABRAS`] palabras) no se pregunta.

use serde_json::{Map, Value, json};

use crate::preguntas::SI;
use crate::preguntas::etiquetas as q;
use crate::{Cliente, Error, Peticion};

/// Cuantas etiquetas se ponen como mucho.
pub const COMO_MUCHO: usize = 3;
/// Por encima, se sugiere (si no se puso ninguna).
pub const SUGERIR: f64 = 0.5;
/// Lo minimo para preguntar.
pub const PALABRAS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Nota {
    pub titulo: String,
    pub texto: String,
}

pub fn estado(n: &Nota) -> Value {
    let mut m = Map::new();
    if !n.titulo.trim().is_empty() {
        m.insert("titulo".into(), json!(crate::recortar(&n.titulo, 200)));
    }
    m.insert("texto".into(), json!(crate::recortar(&n.texto, 2000)));
    json!({ "nota": m })
}

/// La peticion y la clave de cada etiqueta (en su orden). Van las primeras
/// [`q::POR_PETICION`].
pub fn peticion(n: &Nota, etiquetas: &[String]) -> (Peticion, Vec<String>) {
    let claves = crate::claves_unicas(&etiquetas[..etiquetas.len().min(q::POR_PETICION)]);
    let p = claves.iter().fold(Peticion::nueva(estado(n)), |p, (k, t)| {
        p.con(&q::nombre(k), q::va(t))
    });
    (p, claves.into_iter().map(|(k, _)| k).collect())
}

/// Lo que se hace con cada etiqueta (por indice).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Eleccion {
    pub puestas: Vec<(usize, f64)>,
    pub sugeridas: Vec<(usize, f64)>,
}

/// **La tabla**, con la probabilidad de cada etiqueta en su orden.
pub fn decidir(probs: &[f64]) -> Eleccion {
    let mut orden: Vec<(usize, f64)> = probs.iter().copied().enumerate().collect();
    orden.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let puestas: Vec<(usize, f64)> = orden
        .iter()
        .filter(|(_, p)| *p >= SI)
        .take(COMO_MUCHO)
        .copied()
        .collect();
    let sugeridas = if puestas.is_empty() {
        orden
            .iter()
            .filter(|(_, p)| *p >= SUGERIR)
            .take(2)
            .copied()
            .collect()
    } else {
        Vec::new()
    };
    Eleccion { puestas, sugeridas }
}

/// **Pregunta y decide.** Devuelve tambien la probabilidad de cada una.
pub fn elegir(
    cliente: &Cliente,
    n: &Nota,
    etiquetas: &[String],
) -> Result<(Eleccion, Vec<f64>), Error> {
    if etiquetas.is_empty() || n.texto.split_whitespace().count() < PALABRAS {
        return Ok((Eleccion::default(), Vec::new()));
    }
    let (p, claves) = peticion(n, etiquetas);
    let r = cliente.preguntar(&p)?;
    let probs = claves
        .iter()
        .map(|k| r.noul(&q::nombre(k)))
        .collect::<Result<Vec<f64>, Error>>()?;
    Ok((decidir(&probs), probs))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::grabadas;
    use crate::transporte::Grabado;
    use std::sync::Arc;

    #[test]
    fn la_tabla_de_las_etiquetas() {
        let e = decidir(&[0.9, 0.2, 0.75, 0.95, 0.8]);
        assert_eq!(
            e.puestas.iter().map(|x| x.0).collect::<Vec<_>>(),
            vec![3, 0, 4]
        );
        assert!(e.sugeridas.is_empty());
        let e = decidir(&[0.55, 0.65, 0.6, 0.1]);
        assert!(e.puestas.is_empty());
        assert_eq!(
            e.sugeridas.iter().map(|x| x.0).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(decidir(&[0.2, 0.4]), Eleccion::default());
        assert_eq!(decidir(&[0.7]).puestas, vec![(0, 0.7)]);
    }

    #[test]
    fn una_pregunta_por_etiqueta_con_la_etiqueta_dentro() {
        let n = Nota {
            titulo: String::new(),
            texto: "factura de la luz de octubre".into(),
        };
        let (p, claves) = peticion(&n, &["Facturas".into(), "Viajes".into()]);
        assert_eq!(claves, vec!["facturas", "viajes"]);
        let v: Value = serde_json::from_slice(&p.cuerpo().unwrap()).unwrap();
        assert!(
            v["questions"]["etiqueta_facturas"]["instructions"]
                .as_str()
                .unwrap()
                .contains("«Facturas»")
        );
        assert!(v["state"]["nota"].get("titulo").is_none());
    }

    #[test]
    fn elige_con_respuestas_grabadas_y_sin_texto_no_pregunta() {
        let t = Arc::new(Grabado::nuevo(vec![Grabado::ok(&grabadas::nouls(&[
            ("etiqueta_facturas", 0.92),
            ("etiqueta_viajes", 0.05),
        ]))]));
        let c = Cliente::con_transporte("k", t.clone()).con_cache_propio();
        let n = Nota {
            titulo: "Luz".into(),
            texto: "factura de la luz de octubre, 63 euros".into(),
        };
        let (e, probs) = elegir(&c, &n, &["Facturas".into(), "Viajes".into()]).unwrap();
        assert_eq!(e.puestas, vec![(0, 0.92)]);
        assert_eq!(probs.len(), 2);
        let corta = Nota {
            titulo: String::new(),
            texto: "ok".into(),
        };
        assert!(
            elegir(&c, &corta, &["Facturas".into()])
                .unwrap()
                .1
                .is_empty()
        );
        assert_eq!(t.llamadas(), 1);
    }
}
