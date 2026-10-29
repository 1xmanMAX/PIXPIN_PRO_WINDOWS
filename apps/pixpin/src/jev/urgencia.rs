//! **La prioridad de cada tarea segun su contexto** (`docs/jev.md`): por
//! cada tarea pendiente, Jev juzga lo que pasa si se hace tarde, si otro la
//! espera, si el plazo lo puso otro y si en su chat la piden ya
//! (`pixpin_typesafe::urgencia`); cuanto falta para su hora (`⏰`) lo cuenta
//! el codigo, y la tabla de alli da urgente, alta, normal o baja. Lo que el
//! usuario marco (⏫, 🔼) manda.
//!
//! Va en segundo plano: al leer las listas ([`aplicar`]) se pone lo ya
//! sabido y se pregunta por lo que falta en un hilo; al acabar, la ventana
//! de Tareas relee. Lo sabido se queda en memoria (por el texto de la tarea,
//! su lista y su chat) mientras la app este abierta; nada va al disco ni
//! viaja con la sincronizacion.
//!
//! Sin clave o con la funcion apagada, `prioridad` se queda en `None` y
//! Tareas ordena y ensena todo como siempre.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use pixpin_proyecto::{almacen, cuaderno};
use pixpin_store::Ubicacion;
use pixpin_typesafe::urgencia::{self as ju, Contexto, Marca, Senales};

use super::Funcion;
use crate::tareas::Lista;
use crate::tareas::frase::{self, Urgencia};

/// Lo sabido de cada tarea: lo que dijo Jev, o cuando fallo (no se vuelve a
/// preguntar hasta pasado un rato).
enum Sabido {
    Senales(Senales),
    Fallo(Instant),
}

static SABIDO: Mutex<Option<HashMap<String, Sabido>>> = Mutex::new(None);
static PREGUNTANDO: AtomicBool = AtomicBool::new(false);

/// Tras un fallo, cuanto se espera para volver a preguntar.
const TRAS_UN_FALLO: Duration = Duration::from_secs(10 * 60);
/// Cuantas tareas van a la vez.
const A_LA_VEZ: usize = 4;
/// Cuantas tareas se preguntan como mucho en una tanda (las demas, en la
/// siguiente lectura).
const POR_TANDA: usize = 60;

fn con_sabido<R>(f: impl FnOnce(&mut HashMap<String, Sabido>) -> R) -> R {
    let mut g = SABIDO.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(HashMap::new))
}

/// La clave de una tarea: su texto (sin la fecha de creada), su lista y su
/// chat.
fn clave_de(l: &Lista, texto: &str) -> String {
    format!("{}\u{1f}{}\u{1f}{}", l.proyecto, l.titulo, texto)
}

/// **Lo que dijo Jev de una tarea**, si ya se sabe: por su chat, el titulo
/// de su lista y lo que se ve de ella (sin la fecha de creada). Lo usa la
/// reunion en vivo (`jev::en_vivo`) para partir de la prioridad de Tareas.
pub fn sabidas(proyecto: &str, lista: &str, visible: &str) -> Option<Senales> {
    let clave = clave_por_texto(proyecto, lista, visible);
    con_sabido(|s| match s.get(&clave) {
        Some(Sabido::Senales(x)) => Some(x.clone()),
        _ => None,
    })
}

/// **Cambia lo sabido de una tarea** (lo dicho en la reunion la apura) o,
/// con `None`, lo olvida: Tareas lo vuelve a preguntar. Solo en memoria,
/// como todo lo de aqui.
pub fn poner_sabidas(proyecto: &str, lista: &str, visible: &str, s: Option<Senales>) {
    let clave = clave_por_texto(proyecto, lista, visible);
    con_sabido(|m| match s {
        Some(x) => {
            m.insert(clave, Sabido::Senales(x));
        }
        None => {
            m.remove(&clave);
        }
    });
}

fn clave_por_texto(proyecto: &str, lista: &str, visible: &str) -> String {
    let texto = texto_para_jev(&frase::marcas(visible));
    format!("{proyecto}\u{1f}{lista}\u{1f}{texto}")
}

/// La marca del usuario.
fn marca_de(u: Urgencia) -> Marca {
    match u {
        Urgencia::MuyUrgente => Marca::MuyUrgente,
        Urgencia::Urgente => Marca::Urgente,
        Urgencia::Normal => Marca::Ninguna,
    }
}

/// Lo que se le manda de la tarea: su titulo sin marcas y, si tiene, su nota.
fn texto_para_jev(f: &frase::Frase) -> String {
    match &f.nota {
        Some(n) if !n.trim().is_empty() => format!("{} ({})", f.titulo.trim(), n.trim()),
        _ => f.titulo.trim().to_string(),
    }
}

/// **Pone la prioridad de cada tarea pendiente** con lo ya sabido, y lanza
/// en su hilo las preguntas de lo que falta.
pub fn aplicar(u: &Ubicacion, listas: &mut [Lista]) {
    let Some(cliente) = super::para(u, Funcion::Urgencia) else {
        return;
    };
    let hoy = frase::ahora().dia;
    let mut faltan: Vec<(String, String, Contexto)> = Vec::new();
    for l in listas.iter_mut() {
        let lista = l.clone();
        for f in l.filas.iter_mut().filter(|f| !f.hecha) {
            let m = frase::marcas(&f.texto);
            let texto = texto_para_jev(&m);
            if texto.is_empty() {
                continue;
            }
            let dias = m.hora.map(|h| h.dia - hoy);
            let clave = clave_de(&lista, &texto);
            let sabido = con_sabido(|s| match s.get(&clave) {
                Some(Sabido::Senales(x)) => Some(Some(x.clone())),
                Some(Sabido::Fallo(t)) if t.elapsed() < TRAS_UN_FALLO => Some(None),
                _ => None,
            });
            match sabido {
                Some(Some(s)) => {
                    f.prioridad = Some(ju::decidir(marca_de(m.urgencia), Some(&s), dias));
                }
                // Fallo reciente: lo de siempre.
                Some(None) => {}
                None => faltan.push((
                    clave,
                    lista.proyecto.clone(),
                    Contexto {
                        tarea: texto,
                        lista: lista.titulo.clone(),
                        chat: lista.chat.clone(),
                        recientes: Vec::new(),
                    },
                )),
            }
        }
    }
    if faltan.is_empty() || PREGUNTANDO.swap(true, Ordering::SeqCst) {
        return;
    }
    faltan.truncate(POR_TANDA);
    let raiz = u.raiz().to_path_buf();
    let lanzado = std::thread::Builder::new()
        .name("jev-urgencia".into())
        .spawn(move || {
            preguntar(&cliente, &raiz, faltan);
            PREGUNTANDO.store(false, Ordering::SeqCst);
            crate::tareas::refrescar();
        });
    if lanzado.is_err() {
        PREGUNTANDO.store(false, Ordering::SeqCst);
    }
}

/// Lo de [`aplicar`], en el hilo: los mensajes recientes de cada chat y las
/// preguntas.
fn preguntar(
    cliente: &pixpin_typesafe::Cliente,
    raiz: &Path,
    mut faltan: Vec<(String, String, Contexto)>,
) {
    let mut recientes: HashMap<String, Vec<String>> = HashMap::new();
    for (_, proyecto, c) in &mut faltan {
        let r = recientes
            .entry(proyecto.clone())
            .or_insert_with(|| recientes_del_chat(raiz, proyecto));
        c.recientes = r.clone();
    }
    let contextos: Vec<Contexto> = faltan.iter().map(|(_, _, c)| c.clone()).collect();
    let empezo = Instant::now();
    let respuestas = ju::preguntar_varias(cliente, &contextos, A_LA_VEZ);
    let mut fallos = 0;
    con_sabido(|s| {
        for ((clave, _, _), r) in faltan.into_iter().zip(respuestas) {
            match r {
                Ok(x) => {
                    s.insert(clave, Sabido::Senales(x));
                }
                Err(e) => {
                    fallos += 1;
                    if fallos == 1 {
                        tracing::warn!("jev: sin prioridad para una tarea: {e}");
                    }
                    s.insert(clave, Sabido::Fallo(Instant::now()));
                }
            }
        }
    });
    tracing::info!(
        tareas = contextos.len(),
        fallos,
        ms = empezo.elapsed().as_millis() as u64,
        "jev: prioridades de las tareas"
    );
}

/// Los ultimos mensajes escritos del chat (notas con texto, no las listas
/// de tareas), del mas viejo al mas nuevo.
fn recientes_del_chat(raiz: &Path, proyecto: &str) -> Vec<String> {
    let Ok(c) = cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, proyecto)) else {
        return Vec::new();
    };
    let mut v: Vec<String> = c
        .mensajes
        .iter()
        .rev()
        .filter(|m| m.ruta.is_none() && !crate::pedidos::es_lista(m))
        .map(|m| m.texto.trim().to_string())
        .filter(|t| !t.is_empty())
        .take(ju::RECIENTES)
        .collect();
    v.reverse();
    v
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn sin_clave_no_se_toca_nada() {
        let u = Ubicacion::Instalado {
            raiz: std::env::temp_dir().join("pixpin-jev-urgencia-sin-clave"),
        };
        let mut listas = vec![Lista {
            proyecto: "p".into(),
            chat: "Mensajes guardados".into(),
            guardados: true,
            codigo: "c".into(),
            titulo: "Inbox".into(),
            cuando: 0,
            filas: crate::tareas::filas_de("- [ ] pagar la matrícula ⏫\n- [ ] pan"),
            aparato: None,
        }];
        aplicar(&u, &mut listas);
        assert!(listas[0].filas.iter().all(|f| f.prioridad.is_none()));
    }

    #[test]
    fn la_marca_y_el_texto_que_se_manda() {
        assert_eq!(marca_de(Urgencia::MuyUrgente), Marca::MuyUrgente);
        assert_eq!(marca_de(Urgencia::Normal), Marca::Ninguna);
        let m = frase::marcas("llamar al banco 🔼 #casa 💬 cierran a las 3");
        assert_eq!(texto_para_jev(&m), "llamar al banco (cierran a las 3)");
    }
}
