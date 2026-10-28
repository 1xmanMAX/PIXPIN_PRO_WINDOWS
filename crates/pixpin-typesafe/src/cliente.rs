//! **El cliente de TypeSafe**: manda una [`Peticion`] y devuelve sus
//! [`Respuestas`], o un [`Error`] (un fallo nunca se lee como «no").
//!
//! - **Tiempo**: 2 s por omision ([`TIEMPO`]). Jev contesta en 70-500 ms; si
//!   tarda mas, la funcion de PixPin sigue como siempre.
//! - **Registro**: el cuerpo exacto de cada peticion y su respuesta van a
//!   `tracing` en `debug` (objetivo `jev`), las imagenes cambiadas por su
//!   tamano. La clave, nunca.
//! - **Cache**: una peticion identica (mismo modelo, estado y preguntas) no
//!   se vuelve a mandar mientras siga en el cache del proceso ([`Cache`]).
//! - **Reintento**: uno solo, si TypeSafe dice 429 (limite) o 529
//!   (sobrecargado) y queda tiempo.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::Error;
use crate::api::{ListaDeModelos, Modelo, Peticion, Respuestas};
use crate::transporte::{self, Transporte};

/// La API.
pub const BASE: &str = "https://api.typesafe.ai";

/// Lo que se espera por una peticion.
pub const TIEMPO: Duration = Duration::from_secs(2);

/// Cuantas respuestas guarda el cache del proceso.
const CABEN: usize = 2048;

/// **El cache de respuestas**, por la huella del cuerpo (SHA-256). Sale la
/// mas vieja cuando se llena.
#[derive(Default)]
pub struct Cache {
    mapa: Mutex<Guardadas>,
}

/// Las respuestas por su huella y el orden en que entraron.
type Guardadas = (HashMap<[u8; 32], Respuestas>, VecDeque<[u8; 32]>);

impl Cache {
    fn leer(&self, h: &[u8; 32]) -> Option<Respuestas> {
        self.mapa.lock().ok()?.0.get(h).cloned()
    }

    fn guardar(&self, h: [u8; 32], r: Respuestas) {
        let Ok(mut g) = self.mapa.lock() else { return };
        let (mapa, orden) = &mut *g;
        if mapa.insert(h, r).is_none() {
            orden.push_back(h);
        }
        while orden.len() > CABEN {
            if let Some(viejo) = orden.pop_front() {
                mapa.remove(&viejo);
            }
        }
    }

    /// Cuantas respuestas hay guardadas.
    pub fn cuantas(&self) -> usize {
        self.mapa.lock().map(|g| g.0.len()).unwrap_or(0)
    }
}

/// El cache que comparten todos los clientes del proceso.
fn cache_del_proceso() -> Arc<Cache> {
    static CACHE: std::sync::OnceLock<Arc<Cache>> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| Arc::new(Cache::default())).clone()
}

/// **El cliente.** Barato de clonar.
#[derive(Clone)]
pub struct Cliente {
    clave: String,
    base: String,
    tiempo: Duration,
    transporte: Arc<dyn Transporte>,
    cache: Arc<Cache>,
}

impl std::fmt::Debug for Cliente {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // La clave no se ensena nunca.
        f.debug_struct("Cliente")
            .field("base", &self.base)
            .field("tiempo", &self.tiempo)
            .finish_non_exhaustive()
    }
}

impl Cliente {
    /// Con la red del sistema y el cache del proceso.
    pub fn nuevo(clave: impl Into<String>) -> Cliente {
        Cliente::con_transporte(clave, transporte::del_sistema())
    }

    /// Con otro transporte (las pruebas, un grabado) y un cache propio.
    pub fn con_transporte(clave: impl Into<String>, t: Arc<dyn Transporte>) -> Cliente {
        Cliente {
            clave: clave.into().trim().to_string(),
            base: BASE.to_string(),
            tiempo: TIEMPO,
            transporte: t,
            cache: cache_del_proceso(),
        }
    }

    /// Con un cache solo suyo (las pruebas, para contar llamadas).
    pub fn con_cache_propio(mut self) -> Cliente {
        self.cache = Arc::new(Cache::default());
        self
    }

    pub fn con_tiempo(mut self, t: Duration) -> Cliente {
        self.tiempo = t;
        self
    }

    pub fn con_base(mut self, base: &str) -> Cliente {
        self.base = base.trim_end_matches('/').to_string();
        self
    }

    pub fn cache(&self) -> &Cache {
        &self.cache
    }

    fn cabeceras(&self, imagenes: bool) -> Vec<(&'static str, String)> {
        let mut v = vec![
            ("Authorization", format!("Bearer {}", self.clave)),
            ("Content-Type", "application/json".to_string()),
            ("Accept", "application/json".to_string()),
        ];
        if imagenes {
            v.push(("TypeSafe-Danger-Zone-Images", "true".to_string()));
        }
        v
    }

    /// **Pregunta.** Bloquea como mucho unos [`TIEMPO`]: llamarlo desde un
    /// hilo de fondo.
    pub fn preguntar(&self, p: &Peticion) -> Result<Respuestas, Error> {
        if self.clave.is_empty() {
            return Err(Error::SinClave);
        }
        let cuerpo = p.cuerpo()?;
        let huella: [u8; 32] = Sha256::digest(&cuerpo).into();
        if let Some(r) = self.cache.leer(&huella) {
            tracing::debug!(target: "jev", "del cache: {}", p.para_el_registro());
            return Ok(r);
        }
        tracing::debug!(target: "jev", "peticion: {}", p.para_el_registro());
        let url = format!("{}/v1/systemone", self.base);
        let cabeceras = self.cabeceras(p.con_imagenes());
        let empezo = Instant::now();
        let mut intento = 0;
        let contestado = loop {
            let queda = self.tiempo.saturating_sub(empezo.elapsed());
            if queda.is_zero() {
                return Err(Error::Red("se acabo el tiempo".into()));
            }
            let c = self
                .transporte
                .enviar("POST", &url, &cabeceras, &cuerpo, queda)
                .map_err(Error::Red)?;
            if matches!(c.codigo, 429 | 529) && intento == 0 && empezo.elapsed() < self.tiempo / 2 {
                intento += 1;
                std::thread::sleep(Duration::from_millis(250));
                continue;
            }
            break c;
        };
        let texto = String::from_utf8_lossy(&contestado.cuerpo);
        tracing::debug!(
            target: "jev",
            codigo = contestado.codigo,
            ms = empezo.elapsed().as_millis() as u64,
            "respuesta: {texto}"
        );
        if !(200..300).contains(&contestado.codigo) {
            return Err(Error::Http {
                codigo: contestado.codigo,
                detalle: detalle(&texto),
            });
        }
        let r = Respuestas::de_json(&contestado.cuerpo)?;
        // Cada pregunta tiene que tener su respuesta: lo que falte es un
        // error de la peticion entera, no un hueco que se lea como «no».
        for nombre in p.questions.keys() {
            if !r.answers.contains_key(nombre) {
                return Err(Error::FaltaRespuesta(nombre.clone()));
            }
        }
        self.cache.guardar(huella, r.clone());
        Ok(r)
    }

    /// **Los modelos de la cuenta** (`GET /v1/models`): la prueba de la
    /// clave de Ajustes, que no gasta tokens.
    pub fn modelos(&self) -> Result<Vec<Modelo>, Error> {
        if self.clave.is_empty() {
            return Err(Error::SinClave);
        }
        let url = format!("{}/v1/models", self.base);
        let c = self
            .transporte
            .enviar("GET", &url, &self.cabeceras(false), &[], self.tiempo * 2)
            .map_err(Error::Red)?;
        let texto = String::from_utf8_lossy(&c.cuerpo);
        if !(200..300).contains(&c.codigo) {
            return Err(Error::Http {
                codigo: c.codigo,
                detalle: detalle(&texto),
            });
        }
        serde_json::from_slice::<ListaDeModelos>(&c.cuerpo)
            .map(|l| l.models)
            .map_err(|e| Error::Formato(e.to_string()))
    }
}

/// El `detail` del JSON de un error, si lo trae; si no, el principio del
/// cuerpo.
fn detalle(texto: &str) -> String {
    let leido = serde_json::from_str::<serde_json::Value>(texto)
        .ok()
        .and_then(|v| v.get("detail").cloned())
        .map(|d| match d {
            serde_json::Value::String(s) => s,
            otro => otro.to_string(),
        });
    let s = leido.unwrap_or_else(|| texto.to_string());
    s.chars().take(300).collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::api::Pregunta;
    use crate::transporte::{Contestado, Grabado};
    use serde_json::json;

    const BIEN: &str = r#"{"model":"jev-1.14.0","answers":{"billing":{"type":"noul","noul":0.98}},"usage":{"input_tokens":120,"output_tokens":12}}"#;

    fn peticion(texto: &str) -> Peticion {
        Peticion::nueva(json!({ "message": texto })).con(
            "billing",
            Pregunta::noul("Is `message` about billing?", "yes", "no"),
        )
    }

    #[test]
    fn manda_la_clave_y_lee_la_respuesta() {
        let t = Arc::new(Grabado::nuevo(vec![Grabado::ok(BIEN)]));
        let c = Cliente::con_transporte("ts-clave-de-prueba", t.clone()).con_cache_propio();
        let r = c.preguntar(&peticion("cobro doble")).unwrap();
        assert_eq!(r.noul("billing").unwrap(), 0.98);
        let p = t.pedidos.lock().unwrap()[0].clone();
        assert_eq!(p.metodo, "POST");
        assert_eq!(p.url, "https://api.typesafe.ai/v1/systemone");
        assert!(p.cabeceras.contains(&(
            "Authorization".to_string(),
            "Bearer ts-clave-de-prueba".to_string()
        )));
        assert!(
            !p.cabeceras
                .iter()
                .any(|(k, _)| k.starts_with("TypeSafe-Danger"))
        );
        // Lo que viajo es exactamente la peticion.
        assert_eq!(p.cuerpo, peticion("cobro doble").cuerpo().unwrap());
    }

    #[test]
    fn la_misma_peticion_sale_del_cache() {
        let t = Arc::new(Grabado::nuevo(vec![Grabado::ok(BIEN), Grabado::ok(BIEN)]));
        let c = Cliente::con_transporte("k", t.clone()).con_cache_propio();
        c.preguntar(&peticion("a")).unwrap();
        c.preguntar(&peticion("a")).unwrap();
        assert_eq!(t.llamadas(), 1);
        c.preguntar(&peticion("b")).unwrap();
        assert_eq!(t.llamadas(), 2);
        assert_eq!(c.cache().cuantas(), 2);
    }

    #[test]
    fn los_fallos_son_errores_y_no_se_guardan() {
        let t = Arc::new(Grabado::nuevo(vec![
            Ok(Contestado {
                codigo: 401,
                cuerpo: br#"{"detail":"Invalid API key"}"#.to_vec(),
            }),
            Err("sin red".into()),
            Grabado::ok(
                r#"{"model":"jev","answers":{},"usage":{"input_tokens":1,"output_tokens":0}}"#,
            ),
            Grabado::ok("esto no es json"),
        ]));
        let c = Cliente::con_transporte("k", t.clone()).con_cache_propio();
        match c.preguntar(&peticion("x")) {
            Err(Error::Http { codigo, detalle }) => {
                assert_eq!(codigo, 401);
                assert_eq!(detalle, "Invalid API key");
            }
            otro => panic!("{otro:?}"),
        }
        assert!(matches!(c.preguntar(&peticion("x")), Err(Error::Red(_))));
        assert!(matches!(
            c.preguntar(&peticion("x")),
            Err(Error::FaltaRespuesta(_))
        ));
        assert!(matches!(
            c.preguntar(&peticion("x")),
            Err(Error::Formato(_))
        ));
        assert_eq!(c.cache().cuantas(), 0);
    }

    #[test]
    fn sin_clave_ni_se_intenta() {
        let t = Arc::new(Grabado::nuevo(vec![Grabado::ok(BIEN)]));
        let c = Cliente::con_transporte("  ", t.clone());
        assert!(matches!(c.preguntar(&peticion("x")), Err(Error::SinClave)));
        assert_eq!(t.llamadas(), 0);
    }

    #[test]
    fn un_529_se_reintenta_una_vez() {
        let t = Arc::new(Grabado::nuevo(vec![
            Ok(Contestado {
                codigo: 529,
                cuerpo: b"{}".to_vec(),
            }),
            Grabado::ok(BIEN),
        ]));
        let c = Cliente::con_transporte("k", t.clone()).con_cache_propio();
        assert!(c.preguntar(&peticion("y")).is_ok());
        assert_eq!(t.llamadas(), 2);
    }

    #[test]
    fn las_imagenes_llevan_su_cabecera() {
        let t = Arc::new(Grabado::nuevo(vec![Grabado::ok(
            r#"{"model":"jev","answers":{"d":{"type":"box","box":{"top":0,"bottom":1,"left":0,"right":1},"confidence":null}},"usage":{"input_tokens":1,"output_tokens":0}}"#,
        )]));
        let c = Cliente::con_transporte("k", t.clone()).con_cache_propio();
        let p = Peticion::nueva(json!({"foto": "f1"})).con("d", Pregunta::caja("Where?"));
        c.preguntar(&p).unwrap();
        let ped = t.pedidos.lock().unwrap()[0].clone();
        assert!(ped.cabeceras.contains(&(
            "TypeSafe-Danger-Zone-Images".to_string(),
            "true".to_string()
        )));
    }

    #[test]
    fn la_lista_de_modelos() {
        let t = Arc::new(Grabado::nuevo(vec![Grabado::ok(
            r#"{"models":[{"name":"jev-latest","description":"Jev","release_date":"2026-10-14"}]}"#,
        )]));
        let c = Cliente::con_transporte("k", t.clone());
        let m = c.modelos().unwrap();
        assert_eq!(m[0].name, "jev-latest");
        assert_eq!(t.pedidos.lock().unwrap()[0].metodo, "GET");
    }

    #[test]
    fn el_registro_no_ensena_la_clave() {
        let c = Cliente::con_transporte("ts-secreta-123", Arc::new(Grabado::default()));
        assert!(!format!("{c:?}").contains("secreta"));
    }

    /// **La de verdad**, solo con `TYPESAFE_API_KEY`: una pregunta minima a
    /// la API. `cargo test -p pixpin-typesafe -- --ignored`.
    #[test]
    #[ignore = "sale a la red: necesita TYPESAFE_API_KEY"]
    fn de_verdad_con_la_api() {
        let Ok(clave) = std::env::var("TYPESAFE_API_KEY") else {
            eprintln!("sin TYPESAFE_API_KEY: no se prueba");
            return;
        };
        if cfg!(not(windows)) {
            eprintln!("la red de TypeSafe solo esta en Windows: no se prueba");
            return;
        }
        let c = Cliente::nuevo(clave).con_tiempo(Duration::from_secs(10));
        let r = c
            .preguntar(&peticion(
                "I was charged twice for my subscription this month.",
            ))
            .expect("la API contesta");
        let p = r.noul("billing").unwrap();
        assert!(p > 0.7, "billing = {p}");
    }
}
