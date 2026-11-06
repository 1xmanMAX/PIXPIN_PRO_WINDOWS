//! **Cada asignatura es un grupo de tareas** (18-oct-2026).
//!
//! El usuario: «que cada asignatura se cree como un grupo de tareas, y
//! cuando cree una tarea pueda añadirla a una asignatura».
//!
//! POR QUE asi: en PinDesk (y en el movil) un «grupo de tareas» ya es una
//! lista de tareas de un chat (`mini/Tareas.kt`, `tareas::Lista`), y la
//! ventana de Tareas, el Kanban, los recordatorios y la sincronizacion
//! trabajan sobre ellas. Asi que una asignatura no inventa otro almacen:
//! es **una lista de tareas con su nombre en el chat «Asignaturas»**, un
//! chat como cualquier otro (viaja al movil, se ve en el chat y en Tareas,
//! donde cada asignatura sale como un grupo o una columna del Kanban).
//! Un chat aparte y no «Mensajes guardados»: alli vive el Inbox y lo suelto,
//! y las asignaturas juntas en su chat se encuentran de un vistazo.
//!
//! El chat se reconoce por una marca en el indice (como «Mensajes
//! guardados» y el «Asistente»): si se renombra sigue siendo el. Si ya habia
//! un chat llamado «Asignaturas» sin marca, se adopta. La lista de cada
//! asignatura nace al crear la asignatura (o al apuntarle la primera tarea),
//! y la que se busca es la del mismo nombre sin mirar mayusculas ni tildes.
//! Borrar una asignatura del horario NO borra su lista: las tareas son del
//! usuario y se quitan en Tareas.
//!
//! **El enlace** (1-nov-2026): cada linea `Asignatura …` de `horario.md`
//! lleva `lista:<id>`, el `id` del mensaje de su lista. Asi renombrar la
//! asignatura (en su ficha de la vista Horario, o a mano) renombra su lista
//! y no la deja huerfana. [`enlazar`] (el vigia del horario, al arrancar y
//! a cada cambio) crea las listas que faltan, apunta los enlaces y deja las
//! asignaturas para Flow Launcher (`cache/asignaturas.json`). En la ventana
//! de Tareas las listas de las asignaturas salen con su color y aunque esten
//! vacias ([`marcar`]). La lista sigue siendo una lista como las demas: el
//! movil no necesita saber nada nuevo.
//!
//! **Un solo chat para todo lo de las asignaturas** (6-nov-2026, el
//! usuario: «que se cree un chat en la que se agrupen todas y ahi se vaya
//! almacenando todo de estas, solo 1 para todos»): con una asignatura en el
//! horario el chat esta siempre ([`enlazar`] lo asegura), fijo detras del
//! «Asistente» y con su libro 📘 de foto. Ademas de las listas guarda las
//! clases grabadas en su hora, lo que se le manda con «Enviar a
//! asignatura…» y lo que se pone en el con un chip elegido, cada cosa con
//! su asignatura (`pindesk_proyecto::asignatura`; renombrarla aqui renombra
//! la etiqueta, [`reetiquetar`]). Ver
//! `docs/novedades-2026-11-06-un-chat-para-las-asignaturas.md`.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pindesk_ia::acciones::normalizar;
use pindesk_ia::fechas::Momento;
use pindesk_proyecto::{almacen, cuaderno};

use super::Asignatura;
use crate::pedidos::{self, Fallo};
use crate::tareas::{self, Lista};

/// Como se llama el chat de las asignaturas.
pub const NOMBRE_CHAT: &str = pindesk_proyecto::asignatura::NOMBRE_CHAT;
/// Su marca en el indice.
const MARCA: &str = pindesk_proyecto::asignatura::MARCA_CHAT;

fn tiene_marca(f: &almacen::Ficha) -> bool {
    f.es_asignaturas()
}

/// **Su foto al nacer o al adoptarlo** (6-nov-2026): el libro azul, para
/// reconocerlo en la lista. Solo si aun no tiene una: la que elija el
/// usuario manda.
fn poner_su_foto(f: &mut almacen::Ficha) {
    use pindesk_proyecto::{asignatura, avatar};
    if avatar::valor_de(f).is_none() {
        let a = avatar::Avatar::emoji(
            asignatura::EMOJI,
            Some(asignatura::FONDO),
            pindesk_shell::entorno::ahora_utc_ms(),
        );
        f.resto.insert(avatar::CLAVE.into(), a.a_valor());
    }
}

/// El chat «Asignaturas», si ya esta.
pub fn ficha(raiz: &Path) -> Option<almacen::Ficha> {
    let indice = almacen::Indice::leer(raiz);
    indice
        .proyectos
        .iter()
        .find(|f| tiene_marca(f))
        .or_else(|| indice.proyectos.iter().find(|f| f.nombre == NOMBRE_CHAT))
        .cloned()
}

/// El chat «Asignaturas», creandolo (o adoptando uno con ese nombre) si no
/// esta.
pub fn asegurar_ficha(raiz: &Path, aparato: &str) -> Result<almacen::Ficha, Fallo> {
    let mut indice = almacen::Indice::leer_si_esta(raiz)?;
    if let Some(f) = indice.proyectos.iter_mut().find(|f| tiene_marca(f)) {
        // Uno de antes del 6-nov, sin foto: el libro, una vez.
        if pindesk_proyecto::avatar::valor_de(f).is_none() {
            poner_su_foto(f);
            let f = f.clone();
            indice.guardar(raiz)?;
            return Ok(f);
        }
        return Ok(f.clone());
    }
    let marca = (MARCA.to_string(), serde_json::Value::Bool(true));
    let f = match indice
        .proyectos
        .iter_mut()
        .find(|f| f.nombre == NOMBRE_CHAT)
    {
        Some(f) => {
            f.resto.insert(marca.0, marca.1);
            poner_su_foto(f);
            f.clone()
        }
        None => {
            let ahora = pindesk_shell::entorno::ahora_utc_ms();
            let mut f = almacen::Ficha::nueva(NOMBRE_CHAT, ahora, aparato);
            f.resto.insert(marca.0, marca.1);
            poner_su_foto(&mut f);
            indice.proyectos.push(f.clone());
            f
        }
    };
    indice.guardar(raiz)?;
    Ok(f)
}

/// Las listas de tareas del chat «Asignaturas» (vacio si aun no hay chat).
pub fn listas(raiz: &Path) -> Vec<Lista> {
    let Some(f) = ficha(raiz) else {
        return Vec::new();
    };
    match cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, &f.id)) {
        Ok(c) => c
            .mensajes
            .iter()
            .filter(|m| !m.en_buzon)
            .filter_map(|m| tareas::lista_de(&f, m))
            .collect(),
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(?e, "asignaturas: no se pudo leer el cuaderno");
            }
            Vec::new()
        }
    }
}

/// La lista de la asignatura `nombre` entre `listas`.
pub fn de<'a>(listas: &'a [Lista], nombre: &str) -> Option<&'a Lista> {
    let n = normalizar(nombre.trim());
    listas.iter().find(|l| normalizar(l.titulo.trim()) == n)
}

/// **La lista de la asignatura `a`** entre `listas`: la de su enlace
/// (`lista:<id>`, aunque la lista ya se llame de otra forma) y, si no lo
/// tiene o esa lista ya no esta, la de su nombre.
pub fn de_asignatura<'a>(listas: &'a [Lista], a: &Asignatura) -> Option<&'a Lista> {
    a.lista
        .as_deref()
        .and_then(|id| listas.iter().find(|l| l.codigo == id))
        .or_else(|| de(listas, &a.nombre))
}

/// Que no creen dos la misma lista a la vez: la vista Horario (al añadir
/// una asignatura) y el vigia del horario ([`enlazar`]) son hilos distintos.
static CERROJO: Mutex<()> = Mutex::new(());

/// La lista de tareas de la asignatura, creandola (y el chat) si no esta.
pub fn asegurar_lista(raiz: &Path, aparato: &str, nombre: &str) -> Result<Lista, Fallo> {
    let _c = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
    asegurar_lista_sin_cerrojo(raiz, aparato, nombre)
}

fn asegurar_lista_sin_cerrojo(raiz: &Path, aparato: &str, nombre: &str) -> Result<Lista, Fallo> {
    if let Some(l) = de(&listas(raiz), nombre) {
        return Ok(l.clone());
    }
    let f = asegurar_ficha(raiz, aparato)?;
    let m = pedidos::nueva_lista(raiz, &f.id, aparato, nombre.trim())?;
    tareas::lista_de(&f, &m).ok_or(Fallo::NoEsLista)
}

/// **La lista de la asignatura que se llama `nombre`** (sin mirar
/// mayusculas ni tildes): si esta en el horario, la de su enlace; si no, o
/// aun no tiene, la de su nombre, creandola si hace falta. Lo que usan la
/// caja de Tareas, los pedidos (`anadir_tarea` con `asignatura`) y Flow.
pub fn lista_de_la_asignatura(raiz: &Path, aparato: &str, nombre: &str) -> Result<Lista, Fallo> {
    let ahora = Momento::de_ms_locales(pindesk_shell::entorno::ahora_local_ms());
    let n = normalizar(nombre.trim());
    let h = super::de_disco(raiz, ahora);
    let a = h
        .todas_las_asignaturas()
        .into_iter()
        .find(|a| normalizar(&a.nombre) == n);
    if let Some(l) = a
        .as_ref()
        .and_then(|a| de_asignatura(&listas(raiz), a).cloned())
    {
        return Ok(l);
    }
    // Con el nombre del horario, si esta: asi la lista nace bien escrita.
    let nombre = a.map_or_else(|| nombre.trim().to_string(), |a| a.nombre);
    asegurar_lista(raiz, aparato, &nombre)
}

/// **Renombra la lista** de una asignatura renombrada: cambia su titulo y
/// deja sus tareas como estaban (hechas, fechas, imagenes). Es un mensaje
/// reescrito, asi que el nombre nuevo viaja al movil como cualquier cambio
/// de una lista.
pub fn renombrar(raiz: &Path, lista: &Lista, nuevo: &str) -> Result<(), Fallo> {
    let nuevo = nuevo.trim();
    let mut m = pedidos::mensaje_de(raiz, &lista.proyecto, &lista.codigo)?;
    if !pedidos::es_lista(&m) {
        return Err(Fallo::NoEsLista);
    }
    if nuevo.is_empty() || pedidos::nombre_de_lista(&m) == nuevo {
        return Ok(());
    }
    let tareas = pindesk_proyecto::mini::leer_tareas(&m.texto);
    m.texto = pindesk_proyecto::mini::escribir_tareas(nuevo, &tareas);
    m.nombre = nuevo.to_string();
    pedidos::reescribir(raiz, &lista.proyecto, &m)
}

/// **La etiqueta de lo de una asignatura renombrada** (6-nov-2026): lo que
/// llevaba `viejo` en el chat pasa a llevar `nuevo`
/// (`pindesk_proyecto::asignatura::renombrar`). Cada mensaje se reescribe
/// en su sitio, con lo demas tal cual. Devuelve cuantos cambiaron.
pub fn reetiquetar(raiz: &Path, chat: &str, viejo: &str, nuevo: &str) -> Result<usize, Fallo> {
    let carpeta = almacen::carpeta(raiz, chat);
    let mut mensajes = match cuaderno::Cuaderno::leer_de(&carpeta) {
        Ok(c) => c.mensajes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e.into()),
    };
    let cambiados = pindesk_proyecto::asignatura::renombrar(&mut mensajes, viejo, nuevo);
    for &i in &cambiados {
        cuaderno::reemplazar(&carpeta, &mensajes[i])?;
    }
    Ok(cambiados.len())
}

/// **Las asignaturas del horario con su color**, `(nombre, 0xRRGGBB)`, en
/// su orden: los chips de arriba del chat «Asignaturas».
pub fn con_su_color(raiz: &Path) -> Vec<(String, u32)> {
    let ahora = Momento::de_ms_locales(pindesk_shell::entorno::ahora_local_ms());
    super::de_disco(raiz, ahora)
        .todas_las_asignaturas()
        .into_iter()
        .map(|a| {
            let c = a
                .color
                .unwrap_or_else(|| super::semana::color_por_nombre(&a.nombre));
            (a.nombre, c)
        })
        .collect()
}

/// **La asignatura de la clase de ahora** (la que esta en marcha o empieza
/// en unos minutos, `pindesk_proyecto::asignatura::en_clase`): de quien es
/// una grabacion que se empieza ahora.
pub fn en_clase_ahora(raiz: &Path) -> Option<String> {
    let ms = pindesk_shell::entorno::ahora_local_ms();
    let h = super::de_disco(raiz, Momento::de_ms_locales(ms));
    let dia = pindesk_ia::fechas::dia_de_la_semana(ms.div_euclid(86_400_000)) as u8;
    let minuto = (ms.rem_euclid(86_400_000) / 60_000) as u32;
    let tramos: Vec<pindesk_proyecto::asignatura::Tramo> = h
        .clases
        .iter()
        .map(|c| pindesk_proyecto::asignatura::Tramo {
            dias: &c.dias,
            inicio: c.inicio,
            fin: c.fin,
            nombre: &c.nombre,
        })
        .collect();
    let n = pindesk_proyecto::asignatura::en_clase(&tramos, dia, minuto)?;
    // Con el nombre de la lista lateral, si esta escrita distinto.
    Some(
        h.todas_las_asignaturas()
            .into_iter()
            .find(|a| pindesk_proyecto::asignatura::mismo(&a.nombre, n))
            .map_or_else(|| n.to_string(), |a| a.nombre),
    )
}

/// Lo que hay que hacer para que cada asignatura tenga su lista, ya
/// decidido ([`plan`]): lo puro de [`enlazar`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Paso {
    /// Crear la lista de la asignatura (no hay ninguna con su enlace ni con
    /// su nombre).
    Crear(String),
    /// La lista `.0` (su `codigo`) es la de la asignatura `.1`, que no lo
    /// tenia apuntado: apuntar el enlace en su linea.
    Enlazar(String, String),
    /// La asignatura se renombro (a mano o en la ficha): la lista `.0` pasa
    /// a llamarse `.1`.
    Renombrar(String, String),
}

/// **Que falta** para que cada asignatura del horario tenga su lista y el
/// enlace apuntado. `con_linea`: las de la lista lateral (las que tienen
/// linea `Asignatura …` donde apuntar el enlace); `solo_en_clases`: las
/// que solo nombra alguna clase, que tienen lista pero no enlace (se
/// buscan por el nombre hasta que la ventana escriba su linea).
///
/// El nombre que manda es el del horario: si la lista enlazada se llama de
/// otra forma, se renombra (ver `docs/novedades-2026-11-01-asignaturas-con-tareas.md`).
pub fn plan(
    con_linea: &[Asignatura],
    solo_en_clases: &[Asignatura],
    listas: &[Lista],
) -> Vec<Paso> {
    let mut pasos = Vec::new();
    for a in con_linea {
        match de_asignatura(listas, a) {
            Some(l) if a.lista.as_deref() == Some(l.codigo.as_str()) => {
                if l.titulo.trim() != a.nombre.trim() {
                    pasos.push(Paso::Renombrar(l.codigo.clone(), a.nombre.clone()));
                }
            }
            Some(l) => pasos.push(Paso::Enlazar(l.codigo.clone(), a.nombre.clone())),
            None => pasos.push(Paso::Crear(a.nombre.clone())),
        }
    }
    for a in solo_en_clases {
        if de(listas, &a.nombre).is_none() {
            pasos.push(Paso::Crear(a.nombre.clone()));
        }
    }
    pasos
}

/// **Cada asignatura con su lista** (1-nov-2026, el usuario: «que cada
/// asignatura se cree a la vez como asignatura y como grupo de tareas»):
/// crea las que faltan (las de horarios de antes y las escritas a mano),
/// apunta su enlace `lista:<id>` en `horario.md`, renombra la lista de una
/// asignatura renombrada y deja a Flow Launcher lo que necesita para
/// buscarlas ([`escribir_para_flow`]). Lo llama el vigia del horario al
/// arrancar y cada vez que `horario.md` cambia.
///
/// No borra nada: una lista sin asignatura (se quito del horario) se queda
/// en el chat con sus tareas. Devuelve si cambio algo.
pub fn enlazar(raiz: &Path, aparato: &str) -> Result<bool, Fallo> {
    let ruta = super::ruta(raiz);
    let Ok(original) = std::fs::read_to_string(&ruta) else {
        escribir_para_flow(raiz, &super::Horario::default(), &[]);
        return Ok(false);
    };
    let _c = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
    let ahora = Momento::de_ms_locales(pindesk_shell::entorno::ahora_local_ms());
    let h = super::leer(&original, ahora);
    let todas = h.todas_las_asignaturas();
    // Con una asignatura, el chat esta siempre (6-nov-2026): es donde va
    // todo lo suyo, aunque sus listas ya existieran.
    let falta_el_chat = !todas.is_empty()
        && ficha(raiz)
            .is_none_or(|f| !tiene_marca(&f) || pindesk_proyecto::avatar::valor_de(&f).is_none());
    if falta_el_chat {
        asegurar_ficha(raiz, aparato)?;
    }
    let solo_en_clases = &todas[h.asignaturas.len()..];
    let mut ls = listas(raiz);
    let pasos = plan(&h.asignaturas, solo_en_clases, &ls);
    let mut cambio = falta_el_chat;
    let mut enlaces: Vec<(String, String)> = Vec::new();
    for p in &pasos {
        match p {
            Paso::Crear(nombre) => {
                let l = asegurar_lista_sin_cerrojo(raiz, aparato, nombre)?;
                enlaces.push((normalizar(nombre), l.codigo.clone()));
                cambio = true;
            }
            Paso::Enlazar(codigo, nombre) => enlaces.push((normalizar(nombre), codigo.clone())),
            Paso::Renombrar(codigo, nombre) => {
                if let Some(l) = ls.iter().find(|l| &l.codigo == codigo) {
                    renombrar(raiz, l, nombre)?;
                    // Y lo suyo del chat la sigue (6-nov-2026): la etiqueta
                    // de sus clases y de lo que se le mando.
                    let n = reetiquetar(raiz, &l.proyecto, &l.titulo, nombre)?;
                    tracing::info!(%nombre, n, "horario: lista de la asignatura renombrada");
                    cambio = true;
                }
            }
        }
    }
    if cambio {
        ls = listas(raiz);
    }
    let nuevo = super::con_enlaces(&original, &|a| {
        let n = normalizar(&a.nombre);
        enlaces
            .iter()
            .find(|(k, _)| *k == n)
            .map(|(_, id)| id.clone())
    });
    if nuevo != original {
        // Si entre medias se guardo desde la ventana, lo suyo manda: el
        // vigia vuelve a pasar con el horario nuevo.
        if std::fs::read_to_string(&ruta).ok().as_deref() == Some(original.as_str()) {
            std::fs::write(&ruta, &nuevo)?;
            cambio = true;
        }
    }
    let h = super::leer(&nuevo, ahora);
    escribir_para_flow(raiz, &h, &ls);
    Ok(cambio)
}

/// Donde deja la app las asignaturas para Flow Launcher.
pub fn ruta_para_flow(raiz: &Path) -> PathBuf {
    raiz.join("cache").join("asignaturas.json")
}

/// **Lo que lee Flow Launcher** de las asignaturas (`p geometría …`): el
/// plugin no entiende `horario.md` (ni debe: el formato es de la app), asi
/// que la app le deja cada asignatura con su color, su aula, su lista
/// (`proyecto`/`codigo`) y sus clases de la semana en
/// `<raiz>/cache/asignaturas.json`. Ver `docs/integraciones.md`.
pub fn para_flow(h: &super::Horario, listas: &[Lista]) -> serde_json::Value {
    let asignaturas: Vec<serde_json::Value> = h
        .todas_las_asignaturas()
        .iter()
        .map(|a| {
            let n = normalizar(&a.nombre);
            let clases: Vec<serde_json::Value> = h
                .clases
                .iter()
                .filter(|c| normalizar(&c.nombre) == n)
                .flat_map(|c| {
                    c.dias.iter().map(move |&dia| {
                        serde_json::json!({
                            "dia": dia,
                            "inicio": c.inicio,
                            "fin": c.fin,
                            "aula": c.aula.as_ref().or(a.aula.as_ref()),
                        })
                    })
                })
                .collect();
            let l = de_asignatura(listas, a);
            let color = a
                .color
                .unwrap_or_else(|| super::semana::color_por_nombre(&a.nombre));
            serde_json::json!({
                "nombre": a.nombre,
                "aula": a.aula,
                "color": format!("#{color:06X}"),
                "proyecto": l.map(|l| l.proyecto.clone()),
                "codigo": l.map(|l| l.codigo.clone()),
                "clases": clases,
            })
        })
        .collect();
    serde_json::json!({ "version": 1, "asignaturas": asignaturas })
}

fn escribir_para_flow(raiz: &Path, h: &super::Horario, listas: &[Lista]) {
    let ruta = ruta_para_flow(raiz);
    let texto = para_flow(h, listas).to_string();
    if std::fs::read_to_string(&ruta).ok().as_deref() == Some(texto.as_str()) {
        return;
    }
    let hecho = ruta
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|_| std::fs::write(&ruta, texto));
    if let Err(e) = hecho {
        tracing::warn!(?e, "horario: no se dejaron las asignaturas para Flow");
    }
}

/// **El color de las listas de las asignaturas** en la ventana de Tareas
/// (1-nov-2026): a cada lista del chat «Asignaturas» que es de una
/// asignatura del horario, el color de la asignatura (`Lista::asignatura`).
/// Asi salen con su color y, aunque esten vacias, se ven para apuntarles
/// tareas. Las demas, como estaban.
pub fn marcar(raiz: &Path, todas: &mut [Lista]) {
    let Some(f) = ficha(raiz) else {
        return;
    };
    let ahora = Momento::de_ms_locales(pindesk_shell::entorno::ahora_local_ms());
    let h = super::de_disco(raiz, ahora);
    colorear(&h, &f.id, todas);
}

/// Lo puro de [`marcar`]: `chat` es el `id` del chat «Asignaturas».
pub fn colorear(h: &super::Horario, chat: &str, todas: &mut [Lista]) {
    let suyas: Vec<Lista> = todas
        .iter()
        .filter(|l| l.proyecto == chat)
        .cloned()
        .collect();
    for a in h.todas_las_asignaturas() {
        let Some(codigo) = de_asignatura(&suyas, &a).map(|l| l.codigo.clone()) else {
            continue;
        };
        let color = a
            .color
            .unwrap_or_else(|| super::semana::color_por_nombre(&a.nombre));
        if let Some(l) = todas
            .iter_mut()
            .find(|l| l.proyecto == chat && l.codigo == codigo)
        {
            l.asignatura = Some(color);
        }
    }
}

/// **Apunta una tarea en la asignatura** `nombre` (la caja de la ventana de
/// Tareas con una asignatura elegida). Devuelve el chat y el mensaje de la
/// lista, como `tareas::apuntar_con`.
pub fn apuntar(
    raiz: &Path,
    aparato: &str,
    nombre: &str,
    texto: &str,
    imagenes: &[(u32, PathBuf)],
) -> Result<(String, cuaderno::Mensaje), Fallo> {
    let lista = lista_de_la_asignatura(raiz, aparato, nombre)?;
    let m = tareas::apuntar_en(raiz, aparato, &lista, texto, imagenes)?;
    Ok((lista.proyecto, m))
}

/// Los nombres de las asignaturas del horario, para el desplegable de la
/// ventana de Tareas.
pub fn nombres(raiz: &Path) -> Vec<String> {
    let ahora = pindesk_shell::entorno::ahora_local_ms();
    super::de_disco(raiz, pindesk_ia::fechas::Momento::de_ms_locales(ahora))
        .todas_las_asignaturas()
        .into_iter()
        .map(|a| a.nombre)
        .collect()
}

/// La huella del cuaderno del chat: si cambia, se releen las tareas.
pub fn firma(raiz: &Path, proyecto: Option<&str>) -> Option<(std::time::SystemTime, u64)> {
    let ruta = almacen::carpeta(raiz, proyecto?).join("guardados.jsonl");
    std::fs::metadata(ruta)
        .ok()
        .map(|m| (m.modified().unwrap_or(std::time::UNIX_EPOCH), m.len()))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn raiz_de_prueba(nombre: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!(
            "pixpin-asignaturas-{nombre}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    #[test]
    fn cada_asignatura_es_una_lista_del_chat_asignaturas() {
        let raiz = raiz_de_prueba("lista");
        assert!(listas(&raiz).is_empty(), "sin chat aun");
        let l = asegurar_lista(&raiz, "PC01", "Geometría").unwrap();
        assert_eq!(l.titulo, "Geometría");
        assert_eq!(l.chat, NOMBRE_CHAT);
        // La misma, aunque se escriba distinto: no se duplica.
        let l2 = asegurar_lista(&raiz, "PC01", "geometria").unwrap();
        assert_eq!(l.clave(), l2.clave());
        assert_eq!(listas(&raiz).len(), 1);
        // Apuntar le añade la tarea, pendiente.
        let (proyecto, _) = apuntar(&raiz, "PC01", "Física", "resolver la hoja 3", &[]).unwrap();
        assert_eq!(proyecto, l.proyecto, "en el mismo chat");
        let todas = listas(&raiz);
        let f = de(&todas, "física").unwrap();
        assert_eq!(f.filas.len(), 1);
        assert_eq!(f.filas[0].texto, "resolver la hoja 3");
        assert!(!f.filas[0].hecha);
        // Y la ventana de Tareas la ve como una lista mas, en su chat.
        let (en_tareas, _) = tareas::reunir(&raiz);
        assert!(
            en_tareas
                .iter()
                .any(|x| x.titulo == "Física" && x.chat == NOMBRE_CHAT)
        );
        // Un solo chat «Asignaturas», reconocido por su marca.
        let indice = almacen::Indice::leer(&raiz);
        assert_eq!(
            indice
                .proyectos
                .iter()
                .filter(|f| f.nombre == NOMBRE_CHAT)
                .count(),
            1
        );
        assert!(indice.proyectos.iter().any(tiene_marca));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    fn asig(nombre: &str, lista: Option<&str>) -> Asignatura {
        Asignatura {
            nombre: nombre.into(),
            aula: None,
            color: None,
            lista: lista.map(str::to_string),
        }
    }

    fn lista_suelta(codigo: &str, titulo: &str) -> Lista {
        Lista {
            proyecto: "asig".into(),
            chat: NOMBRE_CHAT.into(),
            guardados: false,
            codigo: codigo.into(),
            titulo: titulo.into(),
            cuando: 1,
            filas: Vec::new(),
            aparato: None,
            asignatura: None,
        }
    }

    #[test]
    fn el_plan_crea_enlaza_y_renombra_sin_borrar_nada() {
        let ls = vec![
            lista_suelta("m1", "Geometría"),
            lista_suelta("m2", "Fisica"),
            lista_suelta("m3", "Historia"),
        ];
        let con_linea = vec![
            // Ya enlazada y con el mismo nombre: nada.
            asig("Geometría", Some("m1")),
            // Renombrada en el horario: la lista la sigue por su enlace.
            asig("Física II", Some("m2")),
            // De antes, sin enlace: se enlaza con la de su nombre.
            asig("historia", None),
            // Nueva: se crea.
            asig("Química", None),
            // Su lista se borro en el chat: se crea otra (vacia).
            asig("Arte", Some("m9")),
        ];
        let solo = vec![asig("Inglés", None), asig("geometria", None)];
        assert_eq!(
            plan(&con_linea, &solo, &ls),
            vec![
                Paso::Renombrar("m2".into(), "Física II".into()),
                Paso::Enlazar("m3".into(), "historia".into()),
                Paso::Crear("Química".into()),
                Paso::Crear("Arte".into()),
                Paso::Crear("Inglés".into()),
            ]
        );
        // Caso negativo: todo en orden, ningun paso.
        let bien = vec![asig("Geometría", Some("m1"))];
        assert!(plan(&bien, &[], &ls).is_empty());
    }

    #[test]
    fn el_color_de_la_asignatura_pasa_a_su_lista_y_solo_a_ella() {
        let h = super::super::leer(
            "Asignatura Geometría #FF0000 lista:m1\nAsignatura Física\n",
            Momento::de_ms_locales(0),
        );
        let mut ls = vec![
            lista_suelta("m1", "Geometría analítica"),
            lista_suelta("m2", "física"),
            lista_suelta("m3", "Suelta"),
            Lista {
                proyecto: "otro".into(),
                ..lista_suelta("m4", "Física")
            },
        ];
        colorear(&h, "asig", &mut ls);
        assert_eq!(ls[0].asignatura, Some(0xFF0000), "por su enlace");
        assert_eq!(
            ls[1].asignatura,
            Some(super::super::semana::color_por_nombre("Física")),
            "por su nombre, con el color de su nombre"
        );
        assert_eq!(ls[2].asignatura, None, "sin asignatura");
        assert_eq!(ls[3].asignatura, None, "de otro chat no");
    }

    #[test]
    fn enlazar_crea_las_listas_apunta_el_enlace_y_renombra() {
        let raiz = raiz_de_prueba("enlazar");
        let ruta = super::super::ruta(&raiz);
        std::fs::write(
            &ruta,
            "# Horario\nAsignatura Geometría @A-12\nLun 8:00-9:00 Geometría\nMar 9:00-10:00 Inglés\n",
        )
        .unwrap();
        assert!(enlazar(&raiz, "PC01").unwrap());
        let texto = std::fs::read_to_string(&ruta).unwrap();
        let geo = de(&listas(&raiz), "Geometría").unwrap().clone();
        assert!(
            texto.contains(&format!(
                "Asignatura Geometría @A-12 lista:{}\n",
                geo.codigo
            )),
            "{texto}"
        );
        assert!(
            de(&listas(&raiz), "Inglés").is_some(),
            "la de la clase tambien"
        );
        assert!(texto.contains("Mar 9:00-10:00 Inglés\n"), "lo demas igual");
        // Lo que lee Flow.
        let flow: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(ruta_para_flow(&raiz)).unwrap()).unwrap();
        let a = &flow["asignaturas"][0];
        assert_eq!(a["nombre"], "Geometría");
        assert_eq!(a["codigo"], geo.codigo.as_str());
        assert_eq!(a["clases"][0]["inicio"], 480);
        assert_eq!(a["clases"][0]["aula"], "A-12");
        // Otra vez: nada que hacer.
        assert!(!enlazar(&raiz, "PC01").unwrap());
        // Renombrada a mano (su linea y su clase): la lista la sigue, con
        // sus tareas.
        apuntar(&raiz, "PC01", "Geometría", "compás", &[]).unwrap();
        std::fs::write(&ruta, texto.replace("Geometría", "Dibujo")).unwrap();
        assert!(enlazar(&raiz, "PC01").unwrap());
        let ls = listas(&raiz);
        let dibujo = de(&ls, "Dibujo").unwrap();
        assert_eq!(dibujo.codigo, geo.codigo, "la misma lista");
        assert_eq!(dibujo.filas[0].texto, "compás");
        assert!(
            de(&ls, "Geometría").is_none(),
            "no queda otra con el nombre viejo"
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_chat_nace_con_su_libro_y_lo_suyo_sigue_a_la_asignatura_renombrada() {
        use pindesk_proyecto::{asignatura, avatar};
        let raiz = raiz_de_prueba("chat-unico");
        let ruta = super::super::ruta(&raiz);
        std::fs::write(&ruta, "Asignatura Física\n").unwrap();
        assert!(enlazar(&raiz, "PC01").unwrap());
        let f = ficha(&raiz).unwrap();
        assert!(f.es_asignaturas());
        let a = avatar::de_ficha(&f).unwrap();
        assert_eq!(a.el_emoji(), Some(asignatura::EMOJI), "con su libro");
        // Una clase grabada de Física, en el chat.
        let carpeta = almacen::carpeta(&raiz, &f.id);
        let mut m = cuaderno::Mensaje::nota(
            "clase del lunes",
            &cuaderno::Sello {
                cuando: 5,
                numero: 9,
                aparato: "PC01".into(),
                proyecto: f.id.clone(),
            },
        );
        asignatura::poner(&mut m, "Física");
        cuaderno::anadir(&carpeta, &m).unwrap();
        // Renombrada en el horario: la etiqueta la sigue.
        let texto = std::fs::read_to_string(&ruta).unwrap();
        std::fs::write(&ruta, texto.replace("Física", "Física II")).unwrap();
        assert!(enlazar(&raiz, "PC01").unwrap());
        let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
        let clase = c.mensajes.iter().find(|x| x.id == m.id).unwrap();
        assert_eq!(asignatura::de(clase), Some("Física II"));
        // La foto que ponga el usuario manda: no se le vuelve a poner.
        avatar::poner(&raiz, &f.id, &avatar::Avatar::iniciales(None, 7)).unwrap();
        assert!(!enlazar(&raiz, "PC01").unwrap());
        let f2 = ficha(&raiz).unwrap();
        assert_eq!(avatar::de_ficha(&f2).unwrap().el_emoji(), None);
        // Caso negativo: un solo chat, por muchas vueltas que se den.
        let indice = almacen::Indice::leer(&raiz);
        assert_eq!(
            indice.proyectos.iter().filter(|f| tiene_marca(f)).count(),
            1
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn caso_negativo_quitar_una_asignatura_no_borra_su_lista() {
        let raiz = raiz_de_prueba("quitar");
        let ruta = super::super::ruta(&raiz);
        std::fs::write(&ruta, "Asignatura Arte\n").unwrap();
        enlazar(&raiz, "PC01").unwrap();
        apuntar(&raiz, "PC01", "Arte", "acuarelas", &[]).unwrap();
        std::fs::write(&ruta, "# sin asignaturas\n").unwrap();
        enlazar(&raiz, "PC01").unwrap();
        let ls = listas(&raiz);
        assert_eq!(
            de(&ls, "Arte").unwrap().filas.len(),
            1,
            "sigue con su tarea"
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn caso_negativo_una_tarea_vacia_no_crea_nada_en_otra_asignatura() {
        let raiz = raiz_de_prueba("vacia");
        assert!(apuntar(&raiz, "PC01", "Química", "   ", &[]).is_err());
        let todas = listas(&raiz);
        assert!(de(&todas, "Química").is_none_or(|l| l.filas.is_empty()));
        assert!(de(&todas, "Historia").is_none());
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
