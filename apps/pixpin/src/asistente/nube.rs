//! **El asistente con Claude o DeepSeek** (13-oct-2026): lo que la app
//! necesita del motor de la nube de `pixpin_ia::nube`.
//!
//! - [`eleccion`]: quien contesta segun `[asistente] motor`.
//! - [`motor`]: el motor de la nube con la clave guardada y, si el usuario
//!   no lo apago, el local detras como respaldo.
//! - [`guardar_y_probar`] / [`borrar`] / [`estado`]: lo de Ajustes →
//!   Asistente. Probar es una peticion de red: va en su hilo y avisa a la
//!   ventana al acabar ([`novedades`] cambia y se la despierta).
//!
//! La clave pasa por aqui en claro solo de camino a DPAPI o a la cabecera
//! HTTP: nunca a `tracing` ni a los ajustes.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pixpin_ia::motor::Motor;
use pixpin_ia::nube::{self, MotorConRespaldo, MotorNube, Proveedor, claves};
use pixpin_store::Ubicacion;

/// Quien contesta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eleccion {
    Local,
    Nube(Proveedor),
}

/// Lo elegido en los ajustes. Algo que no se entienda es el local.
pub fn eleccion(a: &pixpin_store::ajustes::Asistente) -> Eleccion {
    Proveedor::por_id(&a.motor).map_or(Eleccion::Local, Eleccion::Nube)
}

/// Las opciones de «Motor», en el orden de los botones de Ajustes.
pub const OPCIONES: [Eleccion; 3] = [
    Eleccion::Local,
    Eleccion::Nube(Proveedor::Claude),
    Eleccion::Nube(Proveedor::DeepSeek),
];

pub fn id_de(e: Eleccion) -> &'static str {
    match e {
        Eleccion::Local => "",
        Eleccion::Nube(p) => p.id(),
    }
}

/// Lo ultimo leido de los ajustes: cuando, de donde y que.
static CACHE: Mutex<Option<(Instant, std::path::PathBuf, Eleccion)>> = Mutex::new(None);

/// Lo elegido, mirado como mucho cada 5 s (la tarjeta del chat pregunta en
/// cada fotograma y el TOML no se lee tanto).
pub fn eleccion_reciente(raiz_datos: &Path) -> Eleccion {
    let Ok(mut g) = CACHE.lock() else {
        return Eleccion::Local;
    };
    if let Some((t, r, e)) = g.as_ref()
        && r == raiz_datos
        && t.elapsed() < Duration::from_secs(5)
    {
        return *e;
    }
    let u = Ubicacion::Instalado {
        raiz: raiz_datos.to_path_buf(),
    };
    let e = eleccion(&pixpin_store::cargar(&u).unwrap_or_default().asistente);
    *g = Some((Instant::now(), raiz_datos.to_path_buf(), e));
    e
}

/// **Cambia quien contesta** desde el propio chat (17-oct-2026, la pastilla
/// del modelo en la cabecera del chat «Asistente»). Es el MISMO ajuste que
/// Ajustes → Asistente → Motor (`[asistente] motor`): se lee el TOML, se
/// cambia solo eso y se guarda; y lo recordado por [`eleccion_reciente`]
/// pasa a ser lo nuevo al instante (la pastilla no espera 5 s).
pub fn elegir(u: &Ubicacion, e: Eleccion) -> Result<(), String> {
    let mut a = pixpin_store::cargar(u).map_err(|e| e.to_string())?;
    a.asistente.motor = id_de(e).to_string();
    pixpin_store::guardar(u, &a).map_err(|e| e.to_string())?;
    if let Ok(mut g) = CACHE.lock() {
        *g = Some((Instant::now(), u.raiz().to_path_buf(), e));
    }
    Ok(())
}

/// El nombre corto para la pastilla del chat: «Claude Haiku», «DeepSeek»,
/// «Local».
pub fn nombre_corto(e: Eleccion) -> &'static str {
    match e {
        Eleccion::Local => "Local",
        Eleccion::Nube(Proveedor::Claude) => "Claude Haiku",
        Eleccion::Nube(Proveedor::DeepSeek) => "DeepSeek",
    }
}

/// **El motor de la nube** con la clave guardada. `local`: el respaldo si
/// la nube falla (ya filtrado por el ajuste y por estar bajado). `Err`: el
/// aviso para el usuario (falta la clave). `pensar`: si piensa este turno
/// (`pixpin_ia::complejidad`); el respaldo local lo recibe en la peticion.
pub fn motor(
    p: Proveedor,
    local: Option<Arc<dyn Motor>>,
    pensar: bool,
) -> Result<Arc<dyn Motor>, String> {
    let clave = claves::leer(p).ok_or_else(|| {
        format!(
            "Falta la clave de {}: pégala en Ajustes → Asistente (se saca en {}).",
            p.nombre(),
            p.pagina_de_claves()
        )
    })?;
    let nube: Arc<dyn Motor> = Arc::new(MotorNube::nuevo(p, clave).pensando(pensar));
    Ok(Arc::new(MotorConRespaldo::nuevo(nube, local, p.nombre())))
}

/// **El critico de respaldo** de la revision con rubrica (23-oct): con Claude
/// elegido y su clave guardada, Sonnet con la misma clave, sin pensar (solo
/// puntua una vista). Entra solo si Haiku no da una nota que se lea o ya
/// suspendio dos veces (`pixpin_ia::agente::Opciones::critico`). Con
/// DeepSeek o el local, ninguno: la revision se queda con su propio juez.
pub fn critico(raiz_datos: &Path) -> Option<Arc<dyn Motor>> {
    // Las pruebas nunca salen a la red.
    if cfg!(test) {
        return None;
    }
    if eleccion_reciente(raiz_datos) != Eleccion::Nube(Proveedor::Claude) {
        return None;
    }
    let clave = claves::leer(Proveedor::Claude)?;
    Some(Arc::new(
        MotorNube::nuevo(Proveedor::Claude, clave)
            .con_modelo(nube::claude::MODELO_CRITICO)
            .pensando(false),
    ))
}

/// **El jefe** (23-oct, `pixpin_ia::orquesta`): con la nube elegida y su
/// clave, el modelo fuerte del mismo proveedor y con la misma clave: Sonnet
/// con Claude, DeepSeek V4 Pro con DeepSeek. Piensa (planear y supervisar es
/// donde mas rinde). Con el local, ninguno.
pub fn jefe(raiz_datos: &Path) -> Option<Arc<dyn Motor>> {
    if cfg!(test) {
        return None;
    }
    let Eleccion::Nube(p) = eleccion_reciente(raiz_datos) else {
        return None;
    };
    let clave = claves::leer(p)?;
    let modelo = match p {
        Proveedor::Claude => nube::claude::MODELO_CRITICO,
        Proveedor::DeepSeek => nube::deepseek::MODELO_PRO,
    };
    Some(Arc::new(MotorNube::nuevo(p, clave).con_modelo(modelo)))
}

// ---------------------------------------------------------------------------
// Ajustes → Asistente

/// Lo que se sabe de cada clave para ensenarlo.
#[derive(Debug, Clone, Default)]
struct Ficha {
    /// `sk-ant-…AbCd` si hay una guardada.
    guardada: Option<String>,
    /// La ultima prueba: en curso (`None`) o su resultado.
    prueba: Option<Option<Result<String, String>>>,
}

static FICHAS: Mutex<Option<HashMap<Proveedor, Ficha>>> = Mutex::new(None);
static NOVEDADES: AtomicU64 = AtomicU64::new(0);

/// Sube cuando cambia algo de las claves (una prueba que acabo): la ventana
/// de ajustes repinta si lo ve distinto.
pub fn novedades() -> u64 {
    NOVEDADES.load(Ordering::SeqCst)
}

fn con_fichas<R>(f: impl FnOnce(&mut HashMap<Proveedor, Ficha>) -> R) -> R {
    let mut g = FICHAS.lock().unwrap_or_else(|e| e.into_inner());
    let mapa = g.get_or_insert_with(|| {
        // La primera vez se mira el disco: enmascarar pide descifrar.
        Proveedor::TODOS
            .into_iter()
            .map(|p| {
                (
                    p,
                    Ficha {
                        guardada: claves::leer(p).map(|c| claves::enmascarar(&c)),
                        prueba: None,
                    },
                )
            })
            .collect()
    });
    f(mapa)
}

/// Si hay clave guardada de este proveedor.
pub fn hay_clave(p: Proveedor) -> bool {
    con_fichas(|m| m.get(&p).is_some_and(|f| f.guardada.is_some()))
}

/// La linea de estado de la clave, para la ayuda de su fila.
pub fn estado(p: Proveedor, t: &pixpin_store::Catalogo) -> String {
    con_fichas(|m| {
        let f = m.get(&p).cloned().unwrap_or_default();
        let base = match &f.guardada {
            Some(mascara) => format!("{} {mascara}", t.t("ajustes2-clave-guardada")),
            None => format!("{} {}", t.t("ajustes2-clave-falta"), p.pagina_de_claves()),
        };
        match f.prueba {
            None => base,
            Some(None) => format!("{base} · {}", t.t("ajustes2-clave-probando")),
            Some(Some(Ok(m))) => format!("{base} · ✓ {m}"),
            Some(Some(Err(m))) => format!("{base} · ⚠️ {m}"),
        }
    })
}

fn avisar(despertar: &(dyn Fn() + Send + Sync)) {
    NOVEDADES.fetch_add(1, Ordering::SeqCst);
    despertar();
}

/// **Guardar y probar** lo escrito: se cifra y guarda ya, y la prueba (una
/// llamada minima) va en su hilo. Si no parece una clave, ni se guarda.
pub fn guardar_y_probar(p: Proveedor, escrito: &str, despertar: Arc<dyn Fn() + Send + Sync>) {
    let clave = escrito.trim().to_string();
    if !claves::parece_clave(&clave) {
        con_fichas(|m| {
            m.entry(p).or_default().prueba = Some(Some(Err("eso no parece una clave".into())))
        });
        avisar(despertar.as_ref());
        return;
    }
    if let Err(e) = claves::guardar(p, &clave) {
        con_fichas(|m| m.entry(p).or_default().prueba = Some(Some(Err(e))));
        avisar(despertar.as_ref());
        return;
    }
    con_fichas(|m| {
        let f = m.entry(p).or_default();
        f.guardada = Some(claves::enmascarar(&clave));
        f.prueba = Some(None);
    });
    avisar(despertar.as_ref());
    let lanzado = std::thread::Builder::new()
        .name("asistente-probar-clave".into())
        .spawn(move || {
            let r = nube::probar(p, &clave);
            if let Err(e) = &r {
                tracing::warn!(
                    "asistente: la prueba de la clave de {} fallo: {e}",
                    p.nombre()
                );
            }
            con_fichas(|m| m.entry(p).or_default().prueba = Some(Some(r)));
            avisar(despertar.as_ref());
        });
    if lanzado.is_err() {
        con_fichas(|m| {
            m.entry(p).or_default().prueba = Some(Some(Err("no se pudo lanzar la prueba".into())))
        });
    }
}

/// Borra la clave guardada.
pub fn borrar(p: Proveedor) {
    let r = claves::borrar_en(&claves::carpeta().unwrap_or_default(), p);
    con_fichas(|m| {
        let f = m.entry(p).or_default();
        f.guardada = None;
        f.prueba = r.err().map(|e| Some(Err(e)));
    });
    NOVEDADES.fetch_add(1, Ordering::SeqCst);
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_eleccion_sale_de_los_ajustes() {
        let mut a = pixpin_store::ajustes::Asistente::default();
        assert_eq!(eleccion(&a), Eleccion::Local);
        assert!(a.respaldo_local, "el respaldo viene encendido");
        a.motor = "claude".into();
        assert_eq!(eleccion(&a), Eleccion::Nube(Proveedor::Claude));
        a.motor = "deepseek".into();
        assert_eq!(eleccion(&a), Eleccion::Nube(Proveedor::DeepSeek));
        a.motor = "otro".into();
        assert_eq!(eleccion(&a), Eleccion::Local);
        for e in OPCIONES {
            let a = pixpin_store::ajustes::Asistente {
                motor: id_de(e).into(),
                ..Default::default()
            };
            assert_eq!(eleccion(&a), e);
        }
    }

    /// La pastilla del chat cambia el mismo ajuste que Ajustes → Asistente,
    /// sin tocar lo demas del TOML, y la eleccion recordada cambia ya.
    #[test]
    fn elegir_desde_el_chat_cambia_el_mismo_ajuste() {
        let raiz =
            std::env::temp_dir().join(format!("pixpin-elegir-modelo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let u = Ubicacion::Instalado { raiz: raiz.clone() };
        let mut a = pixpin_store::ajustes::Ajustes::default();
        a.asistente.revisar_lo_dicho = true;
        pixpin_store::guardar(&u, &a).unwrap();
        assert_eq!(eleccion_reciente(&raiz), Eleccion::Local);
        elegir(&u, Eleccion::Nube(Proveedor::DeepSeek)).unwrap();
        // Al instante, sin esperar los 5 s de lo recordado.
        assert_eq!(
            eleccion_reciente(&raiz),
            Eleccion::Nube(Proveedor::DeepSeek)
        );
        let leido = pixpin_store::cargar(&u).unwrap().asistente;
        assert_eq!(eleccion(&leido), Eleccion::Nube(Proveedor::DeepSeek));
        assert!(leido.revisar_lo_dicho, "lo demas de los ajustes no se toca");
        elegir(&u, Eleccion::Local).unwrap();
        assert_eq!(pixpin_store::cargar(&u).unwrap().asistente.motor, "");
        assert_eq!(
            nombre_corto(Eleccion::Nube(Proveedor::Claude)),
            "Claude Haiku"
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn lo_que_no_parece_una_clave_ni_se_guarda() {
        let despertado = Arc::new(AtomicU64::new(0));
        let d = despertado.clone();
        guardar_y_probar(
            Proveedor::DeepSeek,
            "hola",
            Arc::new(move || {
                d.fetch_add(1, Ordering::SeqCst);
            }),
        );
        assert_eq!(despertado.load(Ordering::SeqCst), 1);
        let t = pixpin_store::Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        assert!(estado(Proveedor::DeepSeek, &t).contains("no parece una clave"));
    }

    #[test]
    fn la_clave_no_va_al_toml() {
        let a = pixpin_store::ajustes::Ajustes {
            asistente: pixpin_store::ajustes::Asistente {
                motor: "claude".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let toml = toml::to_string(&a).unwrap();
        assert!(toml.contains("motor = \"claude\""));
        assert!(!toml.to_lowercase().contains("clave") && !toml.contains("sk-"));
    }
}
