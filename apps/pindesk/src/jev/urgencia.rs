//! **La prioridad de cada tarea segun su contexto** (`docs/jev.md`): por
//! cada tarea pendiente, Jev juzga lo que pasa si se hace tarde, si otro la
//! espera, si el plazo lo puso otro y si en su chat la piden ya
//! (`pindesk_typesafe::urgencia`); cuanto falta para su hora (`⏰` o, si no
//! tiene, la fecha del titulo de su lista: «HOY SÁBADO») lo cuenta el
//! codigo, y la tabla de alli da urgente, alta, normal o baja. Lo que el
//! usuario marco (⏫, 🔼) manda.
//!
//! **Las listas de temas de una reunion** («Temas · Clase…», las que una
//! nota de voz enlaza en `temasLista`) no se preguntan: son puntos de una
//! agenda, no tareas con plazo.
//!
//! Va en segundo plano: al leer las listas ([`aplicar`]) se pone lo ya
//! sabido y se pregunta por lo que falta en un hilo; al acabar, la ventana
//! de Tareas relee.
//!
//! **Lo que dijo Jev se guarda y viaja** (4-nov-2026, Android v0.161.0; el
//! usuario: «si una tarea ya fue revisada, que no se vuelva a hacer; lo
//! revisado en el PC se sincroniza al celular y lo del celular al PC; sin
//! gastar tokens»): en `pins/draw/anot-<uid de la lista>.urgencia.json`
//! (`pindesk_sincro::urgencias`), que viaja con el mensaje de la lista y se
//! junta tarea a tarea al llegar. Antes de preguntar se mira ahi: lo que ya
//! esta, de este aparato o de otro, no se pregunta. Se guardan las senales,
//! no la prioridad: el plazo cambia cada dia y la prioridad se recalcula.
//! En memoria queda ademas lo leido y lo que fallo (no se vuelve a preguntar
//! en 10 min), y lo que apura una reunion en vivo ([`poner_sabidas`]).
//!
//! **Cuando se pregunta**: las listas nacidas en este equipo, al cambiar sus
//! mensajes, pasados 5 s, sin esperar a abrir Tareas ([`vigilar`]); las de
//! otro aparato, solo al abrir Tareas y si su revision aun no ha llegado
//! (quien la creo ya la hizo, y suele llegar en la misma vuelta).
//!
//! Sin clave o con la funcion apagada, `prioridad` se queda en `None` y
//! Tareas ordena y ensena todo como siempre.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use pindesk_proyecto::{almacen, cuaderno};
use pindesk_sincro::disco::Disco;
use pindesk_sincro::urgencias::{self as guardadas, Guardada};
use pindesk_store::Ubicacion;
use pindesk_typesafe::urgencia::{self as ju, Contexto, Marca, Senales};

use super::Funcion;
use crate::tareas::Lista;
use crate::tareas::frase::{self, Urgencia};

/// Lo sabido de cada tarea: lo que dijo Jev, o cuando fallo (no se vuelve a
/// preguntar hasta pasado un rato).
enum Sabido {
    Senales(Senales),
    Fallo(Instant),
    /// Un tema de una reunion: sin prioridad.
    Tema,
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

/// Una tarea por preguntar.
struct Falta {
    /// La de lo sabido en memoria ([`clave_de`]).
    clave: String,
    /// Su lista: el proyecto y el mensaje.
    proyecto: String,
    codigo: String,
    /// Lo que se manda (`tarea` es tambien la clave en el fichero).
    contexto: Contexto,
    /// El fichero de la revision de su lista, si la lista es un mensaje del
    /// cuaderno de un chat del almacen.
    fichero: Option<PathBuf>,
}

/// Las senales de una tarea guardada en el fichero de su lista.
fn senales_de(g: &Guardada) -> Senales {
    Senales::de_partes(
        g.consecuencia.clone(),
        g.confianza,
        g.bloquea,
        g.plazo_ajeno,
        g.piden_ya,
    )
}

/// Lo que se guarda de lo que dijo Jev: cuando y en que aparato.
fn guardada_de(s: &Senales, cuando: i64, aparato: &str) -> Guardada {
    Guardada {
        consecuencia: s.consecuencia.niveles.clone(),
        confianza: s.consecuencia.confianza,
        bloquea: s.bloquea,
        plazo_ajeno: s.plazo_ajeno,
        piden_ya: s.piden_ya,
        cuando,
        aparato: aparato.to_string(),
    }
}

/// **Lo que ya se reviso de las listas, leido del disco una vez por
/// tanda**: el fichero de cada lista y lo que tiene, por su codigo unico.
struct DelDisco<'a> {
    raiz: &'a Path,
    /// El chat (`DiscoPc`) de cada proyecto.
    chats: HashMap<String, Option<String>>,
    listas: HashMap<String, (Option<PathBuf>, guardadas::Tareas)>,
}

impl<'a> DelDisco<'a> {
    fn nuevo(raiz: &'a Path) -> DelDisco<'a> {
        DelDisco {
            raiz,
            chats: HashMap::new(),
            listas: HashMap::new(),
        }
    }

    fn de(&mut self, l: &Lista) -> Option<&(Option<PathBuf>, guardadas::Tareas)> {
        let uid = l.uid.as_deref()?;
        if !self.listas.contains_key(uid) {
            let raiz = self.raiz;
            let chat = self
                .chats
                .entry(l.proyecto.clone())
                .or_insert_with(|| pindesk_proyecto::vista::chat_de_ficha(raiz, &l.proyecto))
                .clone();
            let fichero = chat.map(|c| {
                pindesk_proyecto::vista::DiscoPc::nuevo(raiz)
                    .ruta(&c, &pindesk_proyecto::anotado::urgencia_rel(uid))
            });
            let dentro = fichero.as_deref().map(guardadas::leer).unwrap_or_default();
            self.listas.insert(uid.to_string(), (fichero, dentro));
        }
        self.listas.get(uid)
    }
}

/// El codigo de este equipo, el que va en `aparato` de lo que se pregunta
/// aqui y el de los mensajes nacidos aqui.
fn yo(raiz: &Path) -> String {
    crate::letras_que_viajan::aparato(raiz)
}

/// Si la lista nacio en este equipo (o no se sabe donde).
fn nacida_aqui(l: &Lista, yo: &str) -> bool {
    l.aparato.as_deref().is_none_or(|a| a.is_empty() || a == yo)
}

/// Los dias que faltan para la fecha del titulo de la lista (dicha el dia
/// en que se creo), contados desde `hoy` (dia local).
fn dias_de_la_lista(l: &Lista, hoy: i64) -> Option<i64> {
    use pindesk_typesafe::fechas::Fecha;
    let creada = pindesk_shell::entorno::a_local(l.cuando).div_euclid(86_400_000);
    ju::dias_de_la_lista(&l.titulo, Fecha::de_dias(creada), Fecha::de_dias(hoy))
}

/// **Pone la prioridad de cada tarea pendiente** con lo ya sabido (en
/// memoria o en el fichero de su lista, venga de este aparato o de otro), y
/// lanza en su hilo las preguntas de lo que falta. Con `solo_de_aqui` (lo de
/// fondo, [`vigilar`]) solo se pregunta por las listas nacidas en este
/// equipo; las demas ya las reviso quien las creo.
pub fn aplicar(u: &Ubicacion, listas: &mut [Lista], solo_de_aqui: bool) {
    let Some(cliente) = super::para(u, Funcion::Urgencia) else {
        return;
    };
    let raiz = u.raiz();
    let hoy = frase::ahora().dia;
    let yo = solo_de_aqui.then(|| yo(raiz));
    let mut disco = DelDisco::nuevo(raiz);
    let mut faltan: Vec<Falta> = Vec::new();
    for l in listas.iter_mut() {
        let lista = l.clone();
        let dias_lista = dias_de_la_lista(&lista, hoy);
        let mia = yo.as_deref().is_none_or(|y| nacida_aqui(&lista, y));
        for f in l.filas.iter_mut().filter(|f| !f.hecha) {
            let m = frase::marcas(&f.texto);
            let texto = texto_para_jev(&m);
            if texto.is_empty() {
                continue;
            }
            let dias = m.hora.map(|h| h.dia - hoy).or(dias_lista);
            let clave = clave_de(&lista, &texto);
            let mut sabido = con_sabido(|s| match s.get(&clave) {
                Some(Sabido::Senales(x)) => Some(Some(x.clone())),
                Some(Sabido::Fallo(t)) if t.elapsed() < TRAS_UN_FALLO => Some(None),
                Some(Sabido::Tema) => Some(None),
                _ => None,
            });
            // Lo que ya reviso este u otro aparato, del disco: no se
            // pregunta otra vez (aunque aqui hubiera fallado hace poco).
            if !matches!(sabido, Some(Some(_)))
                && let Some(g) = disco.de(&lista).and_then(|(_, d)| d.get(&texto))
            {
                let s = senales_de(g);
                con_sabido(|m| m.insert(clave.clone(), Sabido::Senales(s.clone())));
                sabido = Some(Some(s));
            }
            match sabido {
                Some(Some(s)) => {
                    let marca = marca_de(m.urgencia);
                    f.prioridad = Some(ju::decidir(marca, Some(&s), dias));
                    f.motivo = (marca == Marca::Ninguna)
                        .then(|| ju::Motivo::de(&s, dias, m.hora.is_none()));
                }
                // Fallo reciente o tema de reunion: lo de siempre.
                Some(None) => {}
                None if mia => faltan.push(Falta {
                    clave,
                    proyecto: lista.proyecto.clone(),
                    codigo: lista.codigo.clone(),
                    contexto: Contexto {
                        tarea: texto,
                        lista: lista.titulo.clone(),
                        chat: lista.chat.clone(),
                        recientes: Vec::new(),
                    },
                    fichero: disco.de(&lista).and_then(|(f, _)| f.clone()),
                }),
                // De otro aparato y de fondo: la revisa quien la creo.
                None => {}
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

/// Cada cuanto [`vigilar`] mira si cambiaron los mensajes (solo fechas y
/// tamanos: [`crate::tareas::firma`]).
const MIRAR_CADA: Duration = Duration::from_secs(2);
/// El respiro tras un cambio antes de preguntar: lo que se esta escribiendo
/// no se pregunta letra a letra.
const RESPIRO: Duration = Duration::from_secs(5);

/// **Revisar al momento lo apuntado aqui** (4-nov-2026, Android
/// `alCambiarLasTareas`): un hilo que, cada vez que cambian los mensajes y
/// pasado un respiro, pregunta por las tareas nuevas de las listas nacidas
/// en este equipo, sin esperar a abrir Tareas. Lo ya revisado no cuesta
/// nada: sale del disco. Las listas de otro aparato no se preguntan aqui.
/// Tambien una vez al arrancar, por lo apuntado mientras estaba cerrada.
pub fn vigilar(u: Ubicacion) {
    let lanzado = std::thread::Builder::new()
        .name("jev-urgencia-vigilar".into())
        .spawn(move || {
            let raiz = u.raiz().to_path_buf();
            let mut proyectos: Vec<String> = Vec::new();
            let mut vista = None;
            let mut cambio: Option<Instant> = None;
            loop {
                std::thread::sleep(MIRAR_CADA);
                let ahora = crate::tareas::firma(&raiz, &proyectos);
                if vista.as_ref() != Some(&ahora) {
                    vista = Some(ahora);
                    cambio = Some(Instant::now());
                }
                // Mientras pregunta otra tanda, se espera: lo cambiado no se
                // pierde.
                if !cambio.is_some_and(|t| t.elapsed() >= RESPIRO)
                    || PREGUNTANDO.load(Ordering::SeqCst)
                {
                    continue;
                }
                cambio = None;
                if super::para(&u, Funcion::Urgencia).is_none() {
                    continue;
                }
                let (mut listas, p) = crate::tareas::reunir(&raiz);
                proyectos = p;
                vista = Some(crate::tareas::firma(&raiz, &proyectos));
                aplicar(&u, &mut listas, true);
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(%e, "jev: no se pudo vigilar las tareas");
    }
}

/// Lo de [`aplicar`], en el hilo: los mensajes recientes de cada chat y las
/// preguntas.
fn preguntar(cliente: &pindesk_typesafe::Cliente, raiz: &Path, faltan: Vec<Falta>) {
    let mut chats: HashMap<String, Chat> = HashMap::new();
    let mut temas = Vec::new();
    let faltan: Vec<Falta> = faltan
        .into_iter()
        .filter_map(|mut f| {
            let chat = chats
                .entry(f.proyecto.clone())
                .or_insert_with(|| leer_chat(raiz, &f.proyecto));
            // Los temas de una reunion no se preguntan ni se guardan.
            if chat.temas.contains(&(f.proyecto.clone(), f.codigo.clone())) {
                temas.push(f.clave);
                return None;
            }
            f.contexto.recientes = chat.recientes.clone();
            Some(f)
        })
        .collect();
    con_sabido(|s| {
        for clave in temas {
            s.insert(clave, Sabido::Tema);
        }
    });
    let contextos: Vec<Contexto> = faltan.iter().map(|f| f.contexto.clone()).collect();
    let empezo = Instant::now();
    let respuestas = ju::preguntar_varias(cliente, &contextos, A_LA_VEZ);
    let mut fallos = 0;
    // Lo dicho, por lista: a su fichero, para que ningun aparato lo vuelva
    // a preguntar.
    let ahora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64);
    let aparato = yo(raiz);
    let mut a_guardar: HashMap<PathBuf, guardadas::Tareas> = HashMap::new();
    con_sabido(|s| {
        for (f, r) in faltan.into_iter().zip(respuestas) {
            match r {
                Ok(x) => {
                    if let Some(fichero) = f.fichero {
                        a_guardar
                            .entry(fichero)
                            .or_default()
                            .insert(f.contexto.tarea, guardada_de(&x, ahora, &aparato));
                    }
                    s.insert(f.clave, Sabido::Senales(x));
                }
                Err(e) => {
                    fallos += 1;
                    if fallos == 1 {
                        tracing::warn!("jev: sin prioridad para una tarea: {e}");
                    }
                    s.insert(f.clave, Sabido::Fallo(Instant::now()));
                }
            }
        }
    });
    for (fichero, nuevas) in a_guardar {
        if let Err(e) = guardadas::anadir(&fichero, &nuevas) {
            tracing::warn!(%e, fichero = %fichero.display(), "jev: no se pudo guardar la urgencia");
        }
    }
    tracing::info!(
        tareas = contextos.len(),
        fallos,
        ms = empezo.elapsed().as_millis() as u64,
        "jev: prioridades de las tareas"
    );
}

/// Lo que se lee de un chat para sus tareas.
#[derive(Default)]
struct Chat {
    /// Los ultimos mensajes escritos (notas con texto, no las listas de
    /// tareas), del mas viejo al mas nuevo.
    recientes: Vec<String>,
    /// Las listas de temas de sus reuniones, `(proyecto, mensaje)`.
    temas: Vec<(String, String)>,
}

fn leer_chat(raiz: &Path, proyecto: &str) -> Chat {
    let Ok(c) = cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, proyecto)) else {
        return Chat::default();
    };
    Chat {
        recientes: recientes_de(&c),
        temas: c
            .mensajes
            .iter()
            .filter_map(|m| pindesk_proyecto::temas::lista_de(&m.resto))
            .collect(),
    }
}

fn recientes_de(c: &cuaderno::Cuaderno) -> Vec<String> {
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
            asignatura: None,
            uid: None,
        }];
        aplicar(&u, &mut listas, false);
        assert!(listas[0].filas.iter().all(|f| f.prioridad.is_none()));
        aplicar(&u, &mut listas, true);
        assert!(listas[0].filas.iter().all(|f| f.prioridad.is_none()));
    }

    #[test]
    fn nacida_aqui_es_la_de_este_equipo_o_sin_aparato() {
        let con = |a: Option<&str>| Lista {
            aparato: a.map(str::to_string),
            ..lista_de_prueba("p", None)
        };
        assert!(nacida_aqui(&con(None), "K7Q2"));
        assert!(nacida_aqui(&con(Some("")), "K7Q2"));
        assert!(nacida_aqui(&con(Some("K7Q2")), "K7Q2"));
        // Caso negativo: la del movil la revisa el movil.
        assert!(!nacida_aqui(&con(Some("MOVI")), "K7Q2"));
    }

    #[test]
    fn lo_guardado_vuelve_con_las_mismas_senales() {
        let s = Senales::de_partes(vec![0.05, 0.15, 0.5, 0.3], 0.6, 0.8, 0.9, Some(0.2));
        let g = guardada_de(&s, 1_793_582_613_265, "K7Q2");
        assert_eq!((g.cuando, g.aparato.as_str()), (1_793_582_613_265, "K7Q2"));
        assert_eq!(senales_de(&g), s);
        // Sin mensajes del chat no hay `piden_ya`, y asi vuelve.
        let sin = Senales::de_partes(vec![1.0], 0.6, 0.0, 0.0, None);
        assert_eq!(senales_de(&guardada_de(&sin, 1, "")).piden_ya, None);
    }

    fn lista_de_prueba(proyecto: &str, uid: Option<&str>) -> Lista {
        Lista {
            proyecto: proyecto.into(),
            chat: "Mensajes guardados".into(),
            guardados: true,
            codigo: "c".into(),
            titulo: "Inbox".into(),
            cuando: 0,
            filas: crate::tareas::filas_de("- [ ] pan"),
            aparato: None,
            asignatura: None,
            uid: uid.map(str::to_string),
        }
    }

    /// **Lo que ya reviso otro aparato se lee del fichero de su lista**, el
    /// mismo que viaja: no se vuelve a preguntar.
    #[test]
    fn el_fichero_de_la_lista_es_el_que_viaja_y_se_lee() {
        let raiz =
            std::env::temp_dir().join(format!("pixpin-jev-urgencia-disco-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let g = almacen::asegurar_guardados(&raiz, 1, "K7Q2").unwrap();
        let f =
            pindesk_proyecto::anotado::urgencia_de_la_lista(&raiz, &g.id, "ABCDE23456").unwrap();
        let s = Senales::de_partes(vec![0.0, 0.0, 1.0], 0.6, 0.1, 0.0, None);
        let tareas = guardadas::Tareas::from([("pan".to_string(), guardada_de(&s, 5, "MOVI"))]);
        assert!(guardadas::anadir(&f, &tareas).unwrap());
        let mut disco = DelDisco::nuevo(&raiz);
        let (fichero, dentro) = disco
            .de(&lista_de_prueba(&g.id, Some("ABCDE23456")))
            .unwrap();
        assert_eq!(fichero.as_deref(), Some(f.as_path()));
        assert_eq!(dentro.get("pan").map(senales_de), Some(s));
        // Casos negativos: sin codigo no hay fichero, y de un chat que no
        // esta en el almacen tampoco.
        assert!(disco.de(&lista_de_prueba(&g.id, None)).is_none());
        let (fichero, dentro) = disco
            .de(&lista_de_prueba("no-esta", Some("ZZZZZ23456")))
            .unwrap();
        assert!(fichero.is_none() && dentro.is_empty());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn la_marca_y_el_texto_que_se_manda() {
        assert_eq!(marca_de(Urgencia::MuyUrgente), Marca::MuyUrgente);
        assert_eq!(marca_de(Urgencia::Normal), Marca::Ninguna);
        let m = frase::marcas("llamar al banco 🔼 #casa 💬 cierran a las 3");
        assert_eq!(texto_para_jev(&m), "llamar al banco (cierran a las 3)");
    }
}
