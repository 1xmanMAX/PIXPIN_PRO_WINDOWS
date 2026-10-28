//! **Jev de TypeSafe en la app** (28-oct-2026, `docs/jev.md`): la clave de
//! Ajustes → Asistente → «TypeSafe (Jev)», el interruptor de cada funcion
//! (`[jev]`) y las cuatro funciones que le preguntan:
//!
//! - [`reunion`]: lo importante de una clase o reunion al acabar de pasarla a
//!   texto (tareas, decisiones, preguntas, examen), en `resto["importantes"]`.
//! - Anki (`ventana_chat::anki`): solo las tarjetas de una clase que valen
//!   la pena.
//! - [`urgencia`]: la prioridad de cada tarea pendiente, en Tareas.
//! - [`modelo`]: el modelo del asistente segun lo dificil del pedido.
//! - [`temas`]: los temas de la reunion que ya se trataron, mientras se
//!   graba (la escucha en vivo, en `ventana_chat::grabar_clase`).
//!
//! Las preguntas y las tablas de decision viven en `pixpin_typesafe`. Sin
//! clave (o con la funcion apagada) [`para`] da `None` y cada funcion hace
//! lo de siempre. Las pruebas nunca salen a la red.
//!
//! La clave pasa por aqui en claro solo de camino a DPAPI o a la cabecera
//! HTTP: nunca a `tracing` ni a los ajustes.

pub mod modelo;
pub mod reunion;
pub mod temas;
pub mod urgencia;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use pixpin_store::Ubicacion;
use pixpin_typesafe::{Cliente, claves};

/// Una funcion que usa Jev (su interruptor de `[jev]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Funcion {
    Reunion,
    Anki,
    Urgencia,
    Modelo,
    /// Los temas de la reunion tratados, mientras se graba.
    Temas,
}

impl Funcion {
    pub fn encendida_en(self, a: &pixpin_store::ajustes::Jev) -> bool {
        match self {
            Funcion::Reunion => a.reunion,
            Funcion::Anki => a.anki,
            Funcion::Urgencia => a.urgencia,
            Funcion::Modelo => a.modelo,
            Funcion::Temas => a.temas,
        }
    }
}

/// **El cliente para esta funcion**: con la clave guardada y la funcion
/// encendida. `None`: lo de siempre.
pub fn para(u: &Ubicacion, f: Funcion) -> Option<Cliente> {
    if cfg!(test) {
        return None;
    }
    let clave = clave()?;
    let a = pixpin_store::cargar(u).unwrap_or_default();
    f.encendida_en(&a.jev).then(|| Cliente::nuevo(clave))
}

// ---------------------------------------------------------------------------
// La clave (Ajustes → Asistente)

/// Lo que se sabe de la clave.
#[derive(Debug, Clone, Default)]
struct Ficha {
    /// La clave descifrada (se descifra una vez por proceso).
    clave: Option<String>,
    /// La ultima prueba: en curso (`None`) o su resultado.
    prueba: Option<Option<Result<String, String>>>,
}

static FICHA: Mutex<Option<Ficha>> = Mutex::new(None);
static NOVEDADES: AtomicU64 = AtomicU64::new(0);

fn con_ficha<R>(f: impl FnOnce(&mut Ficha) -> R) -> R {
    let mut g = FICHA.lock().unwrap_or_else(|e| e.into_inner());
    let ficha = g.get_or_insert_with(|| Ficha {
        clave: if cfg!(test) { None } else { claves::leer() },
        prueba: None,
    });
    f(ficha)
}

fn clave() -> Option<String> {
    con_ficha(|f| f.clave.clone())
}

/// Si hay clave.
pub fn hay_clave() -> bool {
    clave().is_some()
}

/// Sube cuando acaba una prueba: la ventana de ajustes repinta.
pub fn novedades() -> u64 {
    NOVEDADES.load(Ordering::SeqCst)
}

fn avisar(despertar: &(dyn Fn() + Send + Sync)) {
    NOVEDADES.fetch_add(1, Ordering::SeqCst);
    despertar();
}

/// La pagina donde se saca la clave.
pub const PAGINA_DE_CLAVES: &str = "console.typesafe.ai";

/// La linea de estado de la clave, para la ayuda de su fila.
pub fn estado(t: &pixpin_store::Catalogo) -> String {
    con_ficha(|f| {
        let base = match &f.clave {
            Some(c) => format!(
                "{} {}",
                t.t("ajustes2-clave-guardada"),
                claves::enmascarar(c)
            ),
            None => format!("{} {PAGINA_DE_CLAVES}", t.t("ajustes2-clave-falta")),
        };
        match &f.prueba {
            None => base,
            Some(None) => format!("{base} · {}", t.t("ajustes2-clave-probando")),
            Some(Some(Ok(modelo))) => {
                let mut args = fluent_bundle::FluentArgs::new();
                args.set("modelo", modelo.clone());
                format!("{base} · ✓ {}", t.t_args("ajustes2-jev-probada", &args))
            }
            Some(Some(Err(m))) => format!("{base} · ⚠️ {m}"),
        }
    })
}

/// **Guardar y probar**: lo escrito (o, vacio, la guardada) se cifra y se
/// guarda ya; la prueba (`GET /v1/models`, no gasta tokens) va en su hilo.
pub fn guardar_y_probar(escrito: &str, despertar: Arc<dyn Fn() + Send + Sync>) {
    let clave = match escrito.trim() {
        "" => match clave() {
            Some(c) => c,
            None => return,
        },
        c => c.to_string(),
    };
    if !claves::parece_clave(&clave) {
        con_ficha(|f| f.prueba = Some(Some(Err("eso no parece una clave".into()))));
        avisar(despertar.as_ref());
        return;
    }
    if let Err(e) = claves::guardar(&clave) {
        con_ficha(|f| f.prueba = Some(Some(Err(e))));
        avisar(despertar.as_ref());
        return;
    }
    con_ficha(|f| {
        f.clave = Some(clave.clone());
        f.prueba = Some(None);
    });
    avisar(despertar.as_ref());
    let lanzado = std::thread::Builder::new()
        .name("jev-probar-clave".into())
        .spawn(move || {
            let r = Cliente::nuevo(clave)
                .modelos()
                .map(|m| {
                    m.into_iter()
                        .map(|m| m.name)
                        .find(|n| n == pixpin_typesafe::api::MODELO)
                        .unwrap_or_else(|| pixpin_typesafe::api::MODELO.to_string())
                })
                .map_err(|e| e.to_string());
            if let Err(e) = &r {
                tracing::warn!("jev: la prueba de la clave fallo: {e}");
            }
            con_ficha(|f| f.prueba = Some(Some(r)));
            avisar(despertar.as_ref());
        });
    if lanzado.is_err() {
        con_ficha(|f| f.prueba = Some(Some(Err("no se pudo lanzar la prueba".into()))));
    }
}

/// Borra la clave guardada: todo vuelve a lo de siempre.
pub fn borrar() {
    let r = claves::borrar();
    con_ficha(|f| {
        f.clave = None;
        f.prueba = r.err().map(|e| Some(Err(e)));
    });
    NOVEDADES.fetch_add(1, Ordering::SeqCst);
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn en_las_pruebas_no_hay_cliente_ni_clave() {
        let u = Ubicacion::Instalado {
            raiz: std::env::temp_dir().join("pixpin-jev-sin-red"),
        };
        for f in [
            Funcion::Reunion,
            Funcion::Anki,
            Funcion::Urgencia,
            Funcion::Modelo,
            Funcion::Temas,
        ] {
            assert!(para(&u, f).is_none());
        }
        assert!(!hay_clave());
    }

    #[test]
    fn cada_funcion_lee_su_interruptor() {
        let mut a = pixpin_store::ajustes::Jev::default();
        assert!(Funcion::Reunion.encendida_en(&a));
        a.urgencia = false;
        assert!(!Funcion::Urgencia.encendida_en(&a));
        assert!(Funcion::Modelo.encendida_en(&a));
    }
}
