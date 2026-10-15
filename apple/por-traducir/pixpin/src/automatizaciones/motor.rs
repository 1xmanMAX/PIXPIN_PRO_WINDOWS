//! **Lo que hace saltar a las automatizaciones y las corre**, en un hilo
//! propio («automatizaciones») para que un paso del asistente que tarda un
//! minuto no pare nada.
//!
//! - Los disparadores llegan por un canal: [`al_capturar`] (la pila de
//!   capturas, al escribir el PNG), [`al_sincronizar`] (una vuelta de fondo
//!   que trajo algo) y [`a_mano`]. La hora la mira el propio hilo cada
//!   [`LATIDO`] con [`toca_a_la_hora`], sobre la hora local y lo que ya
//!   corrio hoy (el estado de este equipo, que no viaja).
//! - [`elegir`] es lo puro (que corre para cada disparo): se prueba con un
//!   reloj de mentira.
//! - [`Guardia`] pone el tope: una misma automatizacion no corre mas de
//!   [`POR_HORA`] veces en una hora ni dos veces en [`HUECO_MINIMO`], por si
//!   dos se disparan la una a la otra por un camino que no se vio al
//!   crearlas.
//! - Al acabar cada una sale el aviso flotante del asistente con lo que
//!   paso (y «Abrir» si hablo el asistente).

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use pixpin_ia::acciones::AccionApp;

use super::encargo::{self, Encargo, Entorno};
use super::{Automatizacion, Disparador, Hacedor, Lista, Tipo, Vars};

/// Cada cuanto se mira la hora.
pub const LATIDO: Duration = Duration::from_secs(30);
/// Cuanto despues de su hora se recupera una que no corrio (el PC estaba
/// apagado a las 8 y se enciende a las 9: corre; a las 20, ya no).
pub const RECUPERAR_HASTA_MS: i64 = 6 * 3_600_000;
/// Veces por hora como mucho, cada una.
pub const POR_HORA: usize = 30;
/// Lo minimo entre dos vueltas de la misma.
pub const HUECO_MINIMO_MS: i64 = 3_000;
const HUECO_MINIMO: Duration = Duration::from_millis(HUECO_MINIMO_MS as u64);

/// Lo que hace saltar.
#[derive(Debug, Clone, PartialEq)]
pub enum Disparo {
    Captura(PathBuf),
    Sincro(String),
    AMano(String),
    /// Mirar la hora.
    Reloj,
}

// ---------------------------------------------------------------------------
// Lo puro

/// **Si a una `a_la_hora` le toca ahora.** `ahora` y `ultima` son horas
/// locales en ms. Le toca si hoy es uno de sus dias, ya paso su hora (hace
/// menos de [`RECUPERAR_HASTA_MS`]) y no corrio desde entonces.
pub fn toca_a_la_hora(d: &Disparador, ahora: i64, ultima: Option<i64>) -> bool {
    let Disparador::ALaHora { hora, dias } = d else {
        return false;
    };
    let Some(minuto) = super::minuto_del_dia(hora) else {
        return false;
    };
    if !dias.is_empty() && !dias.contains(&super::dia_de_la_semana(ahora)) {
        return false;
    }
    let hoy = ahora.div_euclid(86_400_000) * 86_400_000;
    let su_hora = hoy + i64::from(minuto) * 60_000;
    ahora >= su_hora && ahora - su_hora < RECUPERAR_HASTA_MS && ultima.is_none_or(|u| u < su_hora)
}

/// **Lo que corre por un disparo**: las activas de este aparato con ese
/// disparador, cada una con sus variables. A mano, la pedida aunque este
/// apagada, sea de otro disparador o de otro aparato (se pidio a proposito,
/// y aqui).
pub fn elegir(
    lista: &Lista,
    disparo: &Disparo,
    aparato: &str,
    ahora_local: i64,
    ultimas: &BTreeMap<String, i64>,
) -> Vec<(Automatizacion, Vars)> {
    let base = Vars::default().con_hora(ahora_local);
    lista
        .buenas()
        .filter_map(|a| {
            let vars = match (disparo, &a.disparador) {
                // A mano corre aqui aunque sea de otro aparato: se pidio aqui.
                (Disparo::AMano(id), _) if *id == a.id => base.clone(),
                (Disparo::AMano(_), _) => return None,
                _ if !a.activa || !a.corre_aqui(aparato) => return None,
                (Disparo::Captura(r), Disparador::AlCapturar) => Vars {
                    captura: Some(r.clone()),
                    ..base.clone()
                },
                (Disparo::Sincro(t), Disparador::AlSincronizar) => Vars {
                    sincro: t.clone(),
                    ..base.clone()
                },
                (Disparo::Reloj, d @ Disparador::ALaHora { .. })
                    if toca_a_la_hora(d, ahora_local, ultimas.get(&a.id).copied()) =>
                {
                    base.clone()
                }
                _ => return None,
            };
            Some((a.clone(), vars))
        })
        .collect()
}

/// **El tope de vueltas.**
#[derive(Debug, Default)]
pub struct Guardia {
    veces: HashMap<String, VecDeque<i64>>,
}

impl Guardia {
    /// Si puede correr ahora (`ahora` en ms) y, si si, lo apunta.
    pub fn puede(&mut self, id: &str, ahora: i64) -> Result<(), String> {
        let v = self.veces.entry(id.to_string()).or_default();
        while v.front().is_some_and(|t| ahora - t >= 3_600_000) {
            v.pop_front();
        }
        if v.back().is_some_and(|t| ahora - t < HUECO_MINIMO_MS) {
            return Err("acaba de correr: se salta (¿un bucle?)".into());
        }
        if v.len() >= POR_HORA {
            return Err(format!("ya corrió {POR_HORA} veces en la última hora: se para (¿un bucle?)"));
        }
        v.push_back(ahora);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// El estado de este equipo (no viaja)

fn fichero_de_estado(raiz: &Path) -> PathBuf {
    raiz.join("asistente").join("automatizaciones-estado.json")
}

/// La ultima vez que corrio cada una (hora local en ms).
pub fn leer_estado(raiz: &Path) -> BTreeMap<String, i64> {
    std::fs::read_to_string(fichero_de_estado(raiz))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| serde_json::from_value(v.get("ultimas")?.clone()).ok())
        .unwrap_or_default()
}

pub fn guardar_estado(raiz: &Path, ultimas: &BTreeMap<String, i64>) {
    let ruta = fichero_de_estado(raiz);
    if let Some(d) = ruta.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let texto = serde_json::json!({ "ultimas": ultimas }).to_string();
    if let Err(e) = std::fs::write(&ruta, texto) {
        tracing::warn!(?e, "automatizaciones: no se pudo guardar el estado");
    }
}

// ---------------------------------------------------------------------------
// El hilo

static CANAL: OnceLock<Mutex<Sender<Disparo>>> = OnceLock::new();

fn mandar(d: Disparo) -> bool {
    CANAL
        .get()
        .and_then(|c| c.lock().ok().map(|tx| tx.send(d).is_ok()))
        .unwrap_or(false)
}

/// Se guardo una captura nueva.
pub fn al_capturar(ruta: &Path) {
    mandar(Disparo::Captura(ruta.to_path_buf()));
}

/// Una vuelta de la sincronizacion trajo algo (`titulo`: lo que dijo).
pub fn al_sincronizar(titulo: &str) {
    mandar(Disparo::Sincro(titulo.to_string()));
}

/// Correr esa ya. `false` si el hilo no esta.
pub fn a_mano(id: &str) -> bool {
    mandar(Disparo::AMano(id.to_string()))
}

/// Lo que el hilo necesita de la app.
#[derive(Clone)]
pub struct DeLaApp {
    pub entorno: Entorno,
    pub idioma: pixpin_store::Idioma,
    pub opciones: crate::ventana_chat::OpcionesLienzo,
}

/// **Arranca el hilo** (una vez, al empezar la app).
pub fn arrancar(app: DeLaApp) {
    let (tx, rx) = channel();
    if CANAL.set(Mutex::new(tx)).is_err() {
        return;
    }
    let lanzado = std::thread::Builder::new()
        .name("automatizaciones".into())
        .spawn(move || bucle(app, rx));
    if let Err(e) = lanzado {
        tracing::warn!(?e, "automatizaciones: no se pudo lanzar su hilo");
    }
}

fn bucle(app: DeLaApp, rx: Receiver<Disparo>) {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let raiz = app.entorno.ubicacion.raiz().to_path_buf();
    let mut guardia = Guardia::default();
    let mut ultimas = leer_estado(&raiz);
    loop {
        let disparo = match rx.recv_timeout(LATIDO) {
            Ok(d) => d,
            Err(RecvTimeoutError::Timeout) => Disparo::Reloj,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        // Sin automatizaciones no se crea el chat «Asistente» solo por mirar.
        let lista = match leer_si_hay(&raiz) {
            Some(l) => l,
            None => continue,
        };
        let ahora_local = pixpin_shell::entorno::ahora_local_ms();
        let elegidas = elegir(&lista, &disparo, &app.entorno.aparato, ahora_local, &ultimas);
        for (a, vars) in elegidas {
            if let Err(m) = guardia.puede(&a.id, pixpin_shell::entorno::ahora_utc_ms()) {
                tracing::warn!(id = %a.id, %m, "automatizacion saltada");
                continue;
            }
            if matches!(a.disparador, Disparador::ALaHora { .. }) || matches!(disparo, Disparo::AMano(_)) {
                ultimas.insert(a.id.clone(), ahora_local);
                guardar_estado(&raiz, &ultimas);
            }
            correr(&app, &a, vars);
            std::thread::sleep(HUECO_MINIMO.min(Duration::from_millis(50)));
        }
    }
}

/// La lista, si el chat «Asistente» ya existe y tiene automatizaciones.
fn leer_si_hay(raiz: &Path) -> Option<Lista> {
    let indice = pixpin_proyecto::almacen::Indice::leer(raiz);
    let f = indice.proyectos.iter().find(|f| f.es_asistente())?;
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &f.id);
    match super::leer_de(&carpeta) {
        Ok(l) if l.buenas().next().is_some() => Some(l),
        Ok(_) => None,
        Err(e) => {
            tracing::warn!(%e, "automatizaciones: el fichero no se entiende");
            None
        }
    }
}

/// Corre una y lo dice en el aviso.
fn correr(app: &DeLaApp, a: &Automatizacion, vars: Vars) {
    tracing::info!(id = %a.id, nombre = %a.nombre, "automatizacion: empieza");
    let mut h = DeVerdad {
        app: app.clone(),
        abrir: None,
    };
    // El aviso sale ya, no al acabar (14-oct-2026): si un paso pide permiso
    // para borrar o enviar, la pregunta se ve en el, con «Sí»/«No».
    let mando = crate::ventana_chat::sin_ventana::avisar(crate::ventana_chat::sin_ventana::Aviso {
        titulo: format!("⚙️ {}", super::recortar(&a.nombre, 44)),
        texto: "⚙️ …".into(),
        abrir: None,
        acabado: false,
        chat: Some(crate::ventana_chat::sin_ventana::Chat {
            idioma: app.idioma,
            ubicacion: app.entorno.ubicacion.clone(),
            opciones: app.opciones,
        }),
    });
    let informe = super::ejecutar(a, vars, &mut h);
    tracing::info!(id = %a.id, bien = informe.bien(), pasos = ?informe.pasos, "automatizacion: acabo");
    mando.poner(
        informe.resumen(a.pasos.len()),
        h.abrir
            .map(|(p, c)| (app.idioma, app.entorno.ubicacion.clone(), app.opciones, p, c)),
    );
}

/// Los pasos de verdad: las acciones a esta PixPin (preguntando antes de lo
/// que borra) y el asistente con el motor de Ajustes.
struct DeVerdad {
    app: DeLaApp,
    /// La ultima respuesta del asistente (chat y codigo), para «Abrir».
    abrir: Option<(String, String)>,
}

impl Hacedor for DeVerdad {
    fn accion(&mut self, a: &AccionApp) -> Result<String, String> {
        crate::asistente::acciones::hacer(
            a,
            Some(self.app.entorno.ubicacion.raiz()),
            &{
                // Lo que borra o envia pregunta en una burbuja del chat
                // «Asistente» (13-oct-2026; antes, un cuadro de Windows).
                let ent = self.app.entorno.clone();
                let confirmar: crate::asistente::acciones::Confirmar = Box::new(move |p: &str| {
                    crate::asistente::permiso::preguntar_en_el_chat(&ent.ubicacion, &ent.aparato, "Automatización", p)
                });
                confirmar
            },
            &crate::asistente::acciones::mandar_a_pixpin(),
        )
    }

    fn asistente(&mut self, pedido: &str, proyecto: Option<&str>, imagenes: &[PathBuf], bloquear: Tipo) -> Result<String, String> {
        let textos = pixpin_store::Catalogo::nuevo(self.app.idioma);
        // Sin modelo, una orden rapida se hace igual (`asistente::rapido`).
        let motor = crate::asistente::rapido::motor_o_sin(
            crate::ventana_chat::sin_ventana::motor(&self.app.entorno.ubicacion, &textos),
            pedido,
        )?;
        let r = encargo::hacer(
            &self.app.entorno,
            motor,
            &Encargo {
                texto: pedido.to_string(),
                proyecto: proyecto.map(str::to_string),
                imagenes: imagenes.to_vec(),
                origen: Some("Automatización".into()),
                bloquear: Some(bloquear),
            },
            encargo::Permiso::EnLaBurbuja,
            crate::asistente::acciones::mandar_a_pixpin(),
        )?;
        self.abrir = Some((r.proyecto.clone(), r.codigo.clone()));
        if r.bien { Ok(r.texto) } else { Err(encargo::resumen_de(&r.texto)) }
    }

    fn proyecto(&self, nombre: &str) -> Option<String> {
        proyecto_por_nombre(self.app.entorno.ubicacion.raiz(), nombre)
    }
}

/// El id de un chat por su id o su nombre (sin mayusculas ni tildes; si no
/// hay uno igual, el unico que lo contenga).
pub fn proyecto_por_nombre(raiz: &Path, nombre: &str) -> Option<String> {
    let indice = pixpin_proyecto::almacen::Indice::leer(raiz);
    let q = pixpin_ia::acciones::normalizar(nombre.trim());
    if q.is_empty() {
        return None;
    }
    if let Some(f) = indice.proyectos.iter().find(|f| f.id == nombre.trim()) {
        return Some(f.id.clone());
    }
    let nombre_de = |f: &pixpin_proyecto::almacen::Ficha| {
        if f.es_guardados() {
            "mensajes guardados".to_string()
        } else {
            pixpin_ia::acciones::normalizar(&f.nombre)
        }
    };
    if let Some(f) = indice.proyectos.iter().find(|f| nombre_de(f) == q) {
        return Some(f.id.clone());
    }
    let parecidos: Vec<_> = indice.proyectos.iter().filter(|f| nombre_de(f).contains(&q)).collect();
    match parecidos.as_slice() {
        [uno] => Some(uno.id.clone()),
        _ => None,
    }
}
