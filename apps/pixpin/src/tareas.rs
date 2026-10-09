//! **Tareas**: las listas de tareas de TODOS los chats en una sola ventana
//! (3-oct).
//!
//! Lo pidio el usuario asi: «una ventana Tareas que junta las listas de
//! todos los chats: lo pendiente arriba, lo hecho plegado, las que tienen
//! fecha ordenadas por fecha; marcar una la tacha tambien en su mensaje (y
//! asi se sincroniza con el movil); y un campo para apuntar una tarea rapida
//! en Mensajes guardados».
//!
//! **No hay una segunda copia de las tareas.** Cada lista es la mini-app
//! `tareas` de un mensaje del chat (`mini/Tareas.kt` del movil) y aqui solo
//! se lee: marcar, apuntar, quitar y borrar pasan por las mismas funciones que los pedidos
//! de otros programas (`pedidos::marcar_tarea`, `pedidos::anadir_tarea`),
//! que reescriben el mensaje con `cuaderno::reemplazar` igual que la casilla
//! del panel del chat. Lo que cambia aqui es un mensaje cambiado, y eso es
//! lo que viaja al movil.
//!
//! La «fecha» de una tarea es la de creacion (`➕ AAAA-MM-DD` al final del
//! texto, ver `mini::partir`): el formato no tiene fecha de vencimiento.
//! Como en el movil y en el panel del chat, se ensena cuantos dias lleva y
//! nunca la fecha (lo pidio el usuario). Lo pendiente con fecha va primero,
//! la mas vieja arriba —lo que mas lleva esperando—, y lo que no tiene
//! fecha (tareas de antes de las fechas) detras, en el orden de su lista.
//!
//! **Una tarea puede llevar imagenes** (3-oct), como pegar una imagen en una
//! terminal: van dentro de su texto como `![img 01](pixpin:files/…)`, delante
//! de la fecha (`mini::imagenes_de`), y el fichero se guarda en `archivos/`
//! del chat de la lista, sin mensaje propio: igual que una nota que nombra
//! un documento, el chat no se llena de fotos sueltas. Ver
//! `docs/investigacion/2026-10-03-tareas-con-imagenes-android.md`.
//!
//! - [`ventana`]: la ventana, en su propio hilo como la galeria de capturas.
//! - [`tarjetas`]: lo puro de su vista en tarjetas: buscar, agrupar, colocar
//!   y el foco.
//! - [`rejilla`]: lo puro de la vista en cuadrados (tareas-v4).
//! - [`emoticonos`]: los emoticonos de una tarea, que son sus etiquetas.

#![forbid(unsafe_code)]

mod emoticonos;
pub(crate) mod rejilla;
mod tarjetas;
mod ventana;

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use pixpin_proyecto::{almacen, cuaderno, mini};
use pixpin_store::{Idioma, Ubicacion};

use crate::pedidos::{self, Fallo};

/// Abre la ventana de tareas, o la trae delante si ya estaba. Vuelve
/// enseguida: la ventana vive en su propio hilo. `aparato` es el codigo de
/// este equipo (`K7Q2`), el del sello de lo que se apunte.
pub fn abrir(idioma: Idioma, ubicacion: Ubicacion, aparato: &str) {
    ventana::abrir(idioma, ubicacion, aparato.to_string());
}

/// Como [`abrir`], con la vista en la lista `clave` (`Lista::clave`): lo que
/// abre una lista de tareas pulsada en el chat. Una sola interfaz de tareas.
pub fn abrir_en(idioma: Idioma, ubicacion: Ubicacion, aparato: &str, clave: String) {
    ventana::abrir_en(idioma, ubicacion, aparato.to_string(), Some(clave));
}

// ------------------------------------------------------------- la lectura

/// Una tarea de una lista, ya leida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fila {
    /// Su numero en el documento (desde 0): lo que pide `marcar_tarea`.
    pub indice: usize,
    /// El texto tal cual esta guardado, con su fecha: para comprobar, antes
    /// de marcar, que el numero sigue siendo esta tarea.
    pub crudo: String,
    /// Lo que se ensena: el texto sin la fecha ni las imagenes.
    pub texto: String,
    pub hecha: bool,
    /// El dia en que se creo, si lo lleva.
    pub creada: Option<mini::Fecha>,
    /// Los enlaces de sus imagenes (`pixpin:files/…`), en orden. Se
    /// resuelven con [`ruta_de_imagen`].
    pub imagenes: Vec<String>,
}

/// Una lista de tareas de un chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lista {
    /// El `id` del proyecto (la carpeta del chat).
    pub proyecto: String,
    /// Como se llama el chat.
    pub chat: String,
    /// Si es «Mensajes guardados».
    pub guardados: bool,
    /// El `id` del mensaje en su cuaderno.
    pub codigo: String,
    /// Su titulo (o el nombre del mensaje, como `pedidos::nombre_de_lista`).
    pub titulo: String,
    /// Cuando se creo el mensaje: las listas mas nuevas van primero.
    pub cuando: i64,
    /// Todas sus tareas, en el orden del documento.
    pub filas: Vec<Fila>,
}

impl Lista {
    /// Lo que la distingue de todas las demas: el chat y el mensaje.
    pub fn clave(&self) -> String {
        format!("{}/{}", self.proyecto, self.codigo)
    }

    pub fn cuantas_pendientes(&self) -> usize {
        self.filas.iter().filter(|f| !f.hecha).count()
    }

    /// Lo que se ve de la lista, en dos montones: arriba lo pendiente
    /// (ordenado con [`orden_de_pendiente`]) y aparte, plegable, lo hecho en
    /// el orden del documento.
    ///
    /// `sigue_arriba` deja arriba, tachada y en su sitio, una tarea recien
    /// marcada: si se fuera al montón plegado en el mismo clic, un clic sin
    /// querer no se podria deshacer sin ir a buscarla.
    pub fn a_la_vista(&self, sigue_arriba: &dyn Fn(&Fila) -> bool) -> (Vec<&Fila>, Vec<&Fila>) {
        let (mut arriba, plegadas): (Vec<&Fila>, Vec<&Fila>) =
            self.filas.iter().partition(|f| !f.hecha || sigue_arriba(f));
        arriba.sort_by_key(|f| orden_de_pendiente(f));
        (arriba, plegadas)
    }
}

/// Como se ordena lo pendiente: primero lo que tiene fecha, de la mas vieja
/// a la mas nueva; detras lo que no la tiene. A igualdad, el orden del
/// documento, que es el que el usuario le dio en el chat.
pub fn orden_de_pendiente(f: &Fila) -> (bool, Option<mini::Fecha>, usize) {
    (f.creada.is_none(), f.creada, f.indice)
}

/// Las tareas de un documento, con su texto partido de su fecha.
pub fn filas_de(documento: &str) -> Vec<Fila> {
    mini::leer_tareas(documento)
        .into_iter()
        .enumerate()
        .map(|(indice, t)| {
            let (visible, creada) = mini::partir(&t.texto);
            let (texto, imagenes) = mini::imagenes_de(visible);
            Fila {
                indice,
                texto,
                imagenes,
                creada,
                hecha: t.hecha,
                crudo: t.texto,
            }
        })
        .collect()
}

/// La lista de un mensaje, si es una lista de tareas. Tambien vacia: no se
/// ensena (no hay nada que hacer en ella), pero es un sitio adonde mover
/// lo del Inbox.
pub fn lista_de(ficha: &almacen::Ficha, m: &cuaderno::Mensaje) -> Option<Lista> {
    if !pedidos::es_lista(m) {
        return None;
    }
    let filas = filas_de(&m.texto);
    Some(Lista {
        proyecto: ficha.id.clone(),
        chat: ficha.nombre.clone(),
        guardados: ficha.es_guardados(),
        codigo: m.id.clone(),
        titulo: pedidos::nombre_de_lista(m),
        cuando: m.cuando,
        filas,
    })
}

/// El orden de las listas: el Inbox el primero (es donde cae todo lo
/// apuntado y lo que hay que repartir); luego las que tienen algo pendiente; entre
/// ellas, la mas nueva arriba (es la que se esta usando). Las que ya estan
/// hechas enteras, al final.
pub fn ordenar_listas(v: &mut [Lista]) {
    v.sort_by(|a, b| {
        let pa = a.cuantas_pendientes() > 0;
        let pb = b.cuantas_pendientes() > 0;
        es_inbox(b)
            .cmp(&es_inbox(a))
            .then(pb.cmp(&pa))
            .then(b.cuando.cmp(&a.cuando))
            .then(a.clave().cmp(&b.clave()))
    });
}

/// Todas las listas de todos los chats, ya ordenadas, y los `id` de los
/// chats que se miraron (para [`firma`]). Un chat cuyo cuaderno no se puede
/// leer no deja sin las demas: se apunta en el registro y se sigue.
pub fn reunir(raiz: &Path) -> (Vec<Lista>, Vec<String>) {
    let indice = almacen::Indice::leer(raiz);
    let mut listas = Vec::new();
    let mut proyectos = Vec::new();
    for ficha in &indice.proyectos {
        proyectos.push(ficha.id.clone());
        let c = match cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, &ficha.id)) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                tracing::warn!(?e, proyecto = %ficha.id, "tareas: no se pudo leer el cuaderno");
                continue;
            }
        };
        listas.extend(c.mensajes.iter().filter_map(|m| lista_de(ficha, m)));
    }
    ordenar_listas(&mut listas);
    (listas, proyectos)
}

/// La huella de lo que se lee: la fecha y el tamano del indice de proyectos
/// y del cuaderno de cada chat. Mirarla cada segundo cuesta unas pocas
/// llamadas al disco y ninguna lectura; si cambia (algo escrito en el chat,
/// o llegado del movil), se vuelve a [`reunir`].
pub fn firma(raiz: &Path, proyectos: &[String]) -> Vec<Option<(SystemTime, u64)>> {
    let de = |ruta: &Path| {
        std::fs::metadata(ruta)
            .ok()
            .map(|m| (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len()))
    };
    std::iter::once(de(&almacen::ruta(raiz)))
        .chain(
            proyectos
                .iter()
                .map(|id| de(&almacen::carpeta(raiz, id).join("guardados.jsonl"))),
        )
        // Lo reescrito desde esta app, aunque el disco no mueva la fecha.
        .chain(std::iter::once(Some((
            SystemTime::UNIX_EPOCH,
            pedidos::reescritos(),
        ))))
        .collect()
}

// --------------------------------------------------------------- escribir

/// Pone una tarea como hecha o pendiente, en su mensaje.
///
/// Antes se comprueba que la tarea numero `fila.indice` sigue siendo la que
/// se ve: si la lista cambio en el chat o llego cambiada del movil desde la
/// ultima lectura, el numero puede ser ya otra tarea, y tacharla seria
/// tachar lo que no se pidio. Entonces no se toca nada y se devuelve
/// `Ok(false)`: hay que releer.
pub fn marcar(raiz: &Path, lista: &Lista, fila: &Fila, hecha: bool) -> Result<bool, Fallo> {
    let m = pedidos::mensaje_de(raiz, &lista.proyecto, &lista.codigo)?;
    let sigue = mini::leer_tareas(&m.texto)
        .get(fila.indice)
        .is_some_and(|t| t.texto == fila.crudo);
    if !sigue {
        return Ok(false);
    }
    pedidos::marcar_tarea(raiz, &lista.proyecto, &lista.codigo, fila.indice, hecha)?;
    Ok(true)
}

/// Quita una tarea de su lista (el aspa de su tarjeta, o Supr). Como al
/// marcar, antes se comprueba que el numero sigue siendo esta tarea: si la
/// lista cambio fuera, quitar «la de ese numero» seria quitar otra, asi que
/// no se toca nada y se devuelve `Ok(None)`. Si se quito, devuelve la tarea
/// tal cual estaba, para poder [`reponer`]la.
pub fn quitar(raiz: &Path, lista: &Lista, fila: &Fila) -> Result<Option<mini::Tarea>, Fallo> {
    let m = pedidos::mensaje_de(raiz, &lista.proyecto, &lista.codigo)?;
    let sigue = mini::leer_tareas(&m.texto)
        .get(fila.indice)
        .is_some_and(|t| t.texto == fila.crudo);
    if !sigue {
        return Ok(None);
    }
    pedidos::quitar_tarea(raiz, &lista.proyecto, &lista.codigo, fila.indice).map(Some)
}

/// «Deshacer» de una tarea quitada: vuelve a su lista en el sitio que tenia
/// (o al final, si la lista se acorto entre medias), con su fecha y su
/// estado. Si la lista ya no esta (la borraron), se dice.
pub fn reponer(
    raiz: &Path,
    proyecto: &str,
    codigo: &str,
    indice: usize,
    tarea: mini::Tarea,
) -> Result<(), Fallo> {
    let mut m = pedidos::mensaje_de(raiz, proyecto, codigo)?;
    if !pedidos::es_lista(&m) {
        return Err(Fallo::NoEsLista);
    }
    let mut tareas = mini::leer_tareas(&m.texto);
    tareas.insert(indice.min(tareas.len()), tarea);
    m.texto = mini::escribir_tareas(&mini::titulo(&m.texto), &tareas);
    pedidos::reescribir(raiz, proyecto, &m)
}

/// **Corrige el texto** de una tarea («Editar» de la ventana, F2): lo que
/// se ve pasa a ser `texto`, con sus imagenes de antes y las nuevas pegadas
/// en la caja (`imagenes`, sus fichas `[img NN]` cambiadas por la imagen ya
/// copiada al chat de la lista) y su fecha de creacion: corregir una falta
/// no la hace nueva (`mini::renombrar`, el `Tareas.renombrar` del movil).
///
/// Como al marcar, si el numero ya no es esta tarea no se toca nada y se
/// devuelve `Ok(false)`. Un texto vacio no borra la tarea: eso es el aspa.
pub fn corregir(
    raiz: &Path,
    lista: &Lista,
    fila: &Fila,
    texto: &str,
    imagenes: &[(u32, PathBuf)],
) -> Result<bool, Fallo> {
    if mini::saneado(texto).is_empty() && imagenes.is_empty() && fila.imagenes.is_empty() {
        return Err(Fallo::TareaVacia);
    }
    let mut m = pedidos::mensaje_de(raiz, &lista.proyecto, &lista.codigo)?;
    let sigue = mini::leer_tareas(&m.texto)
        .get(fila.indice)
        .is_some_and(|t| t.texto == fila.crudo);
    if !sigue {
        return Ok(false);
    }
    let mut visible = con_imagenes_guardadas(raiz, &lista.proyecto, texto, imagenes)?;
    // Las imagenes que ya tenia, detras: la caja solo corrige el texto. El
    // rotulo de cada una no importa (el lector no lo mira).
    let antes = mini::con_imagenes("", &fila.imagenes);
    if !antes.is_empty() {
        visible = format!("{} {antes}", visible.trim());
    }
    let nuevo = mini::renombrar(&m.texto, fila.indice, &visible);
    if nuevo != m.texto {
        m.texto = nuevo;
        pedidos::reescribir(raiz, &lista.proyecto, &m)?;
    }
    Ok(true)
}

/// Borra la lista entera, con todas sus tareas: su mensaje sale del chat
/// como al borrarlo alli (`pedidos::borrar_lista`), y asi tambien del movil.
pub fn borrar_lista(raiz: &Path, lista: &Lista) -> Result<(), Fallo> {
    pedidos::borrar_lista(raiz, &lista.proyecto, &lista.codigo).map(|_| ())
}

/// Como se llama la lista adonde va todo lo que se apunta desde la ventana
/// (el usuario, 3-oct: «que todo lo que se inserte asi nomas entre a un
/// inbox»). Vive en «Mensajes guardados» y de ahi se reparte con «Mover a…».
pub const INBOX: &str = "Inbox";

/// Si una lista es el Inbox: la de «Mensajes guardados» que se llama asi.
pub fn es_inbox(lista: &Lista) -> bool {
    lista.guardados && lista.titulo.trim().eq_ignore_ascii_case(INBOX)
}

/// Apunta una tarea en el **Inbox** de «Mensajes guardados» (los dos se
/// crean si aun no estan), con la fecha de hoy.
#[cfg_attr(not(test), allow(dead_code))] // la ventana y los pedidos usan `apuntar_con`
pub fn apuntar(raiz: &Path, aparato: &str, texto: &str) -> Result<cuaderno::Mensaje, Fallo> {
    apuntar_con(raiz, aparato, texto, &[])
}

/// Como [`apuntar`], con imagenes: cada `(numero, fichero)` se copia al chat
/// del Inbox y su ficha `[img NN]` del texto se cambia por la imagen
/// ([`mini::fichas_a_imagenes`]).
pub fn apuntar_con(
    raiz: &Path,
    aparato: &str,
    texto: &str,
    imagenes: &[(u32, PathBuf)],
) -> Result<cuaderno::Mensaje, Fallo> {
    if mini::saneado(texto).is_empty() && imagenes.is_empty() {
        return Err(Fallo::TareaVacia);
    }
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let f = pedidos::ficha_de(raiz, None, aparato, ahora)?;
    let texto = &con_imagenes_guardadas(raiz, &f.id, texto, imagenes)?;
    let inbox = match cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, &f.id)) {
        Ok(c) => c.mensajes.into_iter().find(|m| {
            pedidos::es_lista(m)
                && !m.en_buzon
                && pedidos::nombre_de_lista(m)
                    .trim()
                    .eq_ignore_ascii_case(INBOX)
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    let inbox = match inbox {
        Some(m) => m,
        None => pedidos::nueva_lista(raiz, &f.id, aparato, INBOX)?,
    };
    pedidos::anadir_tarea(raiz, &f.id, aparato, Some(&inbox.id), texto, INBOX)
}

/// Como [`apuntar_con`], pero en la lista `lista` y no en el Inbox: la caja
/// de la ventana de Tareas abierta desde una lista del chat.
pub fn apuntar_en(
    raiz: &Path,
    aparato: &str,
    lista: &Lista,
    texto: &str,
    imagenes: &[(u32, PathBuf)],
) -> Result<cuaderno::Mensaje, Fallo> {
    if mini::saneado(texto).is_empty() && imagenes.is_empty() {
        return Err(Fallo::TareaVacia);
    }
    let texto = &con_imagenes_guardadas(raiz, &lista.proyecto, texto, imagenes)?;
    pedidos::anadir_tarea(
        raiz,
        &lista.proyecto,
        aparato,
        Some(&lista.codigo),
        texto,
        &lista.titulo,
    )
}

// --------------------------------------------------- los recordatorios
//
// **Una tarea puede recordarse a una hora** (8-oct-2026, el usuario: «añade
// los recordatorios a las tareas, para que algunas tareas que son
// importantes las recuerde cuando llegue el momento»). Android no lo tiene,
// asi que la hora va DENTRO del texto de la tarea, al final:
// `Llamar al banco ⏰ 2026-10-09 10:00`, en hora local. Asi viaja al
// sincronizar sin inventar otro campo, y en el movil se lee como texto. Al
// sonar, la marca se quita.

/// Lo que marca la hora en el texto de una tarea.
pub const RELOJ: char = '⏰';

/// Los dias desde 1970-01-01 de una fecha (Howard Hinnant, `days_from_civil`).
fn dias_de(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Al reves: la fecha de unos dias desde 1970.
fn fecha_de(dias: i64) -> (i64, i64, i64) {
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// La marca de una hora local: `⏰ 2026-10-09 10:00`.
pub fn marca_de_hora(local_ms: i64) -> String {
    const DIA: i64 = 86_400_000;
    let (y, m, d) = fecha_de(local_ms.div_euclid(DIA));
    let min = local_ms.rem_euclid(DIA) / 60_000;
    format!("{RELOJ} {y:04}-{m:02}-{d:02} {:02}:{:02}", min / 60, min % 60)
}

/// La hora (local) de una tarea y donde empieza su marca, si la lleva.
fn marca_en(texto: &str) -> Option<(usize, i64)> {
    let i = texto.rfind(RELOJ)?;
    let resto = texto[i + RELOJ.len_utf8()..].trim_start();
    let num = |a: usize, b: usize| resto.get(a..b).and_then(|s| s.parse::<i64>().ok());
    let (y, mo, d, h, mi) = (num(0, 4)?, num(5, 7)?, num(8, 10)?, num(11, 13)?, num(14, 16)?);
    let sep = resto.as_bytes();
    if sep.get(4) != Some(&b'-') || sep.get(7) != Some(&b'-') || sep.get(13) != Some(&b':') {
        return None;
    }
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 {
        return None;
    }
    Some((i, (dias_de(y, mo, d) * 1440 + h * 60 + mi) * 60_000))
}

/// La hora local a la que se recuerda una tarea, si tiene.
pub fn hora_de_tarea(texto: &str) -> Option<i64> {
    marca_en(texto).map(|(_, t)| t)
}

/// El texto sin la marca de la hora.
pub fn sin_hora(texto: &str) -> String {
    match marca_en(texto) {
        Some((i, _)) => texto[..i].trim_end().to_string(),
        None => texto.to_string(),
    }
}

/// El texto con la hora `local_ms` (cambiando la que tuviera).
pub fn con_hora(texto: &str, local_ms: i64) -> String {
    format!("{} {}", sin_hora(texto).trim_end(), marca_de_hora(local_ms))
}

/// El id de agenda del recordatorio de una tarea: su lista y su hora.
pub fn id_de_recordatorio(mensaje: &str, local_ms: i64) -> String {
    format!("tarea:{mensaje}:{local_ms}")
}

/// Los recordatorios de las tareas pendientes de un cuaderno.
pub fn avisos_del_cuaderno(c: &cuaderno::Cuaderno) -> Vec<crate::recordatorios::Recordatorio> {
    c.mensajes
        .iter()
        .filter(|m| pedidos::es_lista(m))
        .flat_map(|m| {
            mini::leer_tareas(&m.texto)
                .into_iter()
                .filter(|t| !t.hecha)
                .filter_map(|t| {
                    let local = hora_de_tarea(&t.texto)?;
                    Some(crate::recordatorios::Recordatorio {
                        id: id_de_recordatorio(&m.id, local),
                        cuando_utc_ms: crate::recordatorios::de_local_a_utc(local),
                        texto: format!("☑ {}", mini::partir(&sin_hora(&t.texto)).0.trim()),
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Quita la marca de la hora que sono (`id` de [`id_de_recordatorio`]).
/// `Ok(false)` si no es de una tarea o ya no esta.
pub fn olvidar_hora(carpeta: &Path, id: &str) -> std::io::Result<bool> {
    let Some((mensaje, local)) = id
        .strip_prefix("tarea:")
        .and_then(|r| r.rsplit_once(':'))
        .and_then(|(m, t)| Some((m, t.parse::<i64>().ok()?)))
    else {
        return Ok(false);
    };
    let c = cuaderno::Cuaderno::leer_de(carpeta)?;
    let Some(m) = c.mensajes.iter().find(|m| m.id == mensaje) else {
        return Ok(false);
    };
    let marca = marca_de_hora(local);
    let Some(i) = m.texto.find(&marca) else {
        return Ok(false);
    };
    let mut m = m.clone();
    // Con el espacio de delante, que se puso al marcarla.
    let desde = if m.texto[..i].ends_with(' ') { i - 1 } else { i };
    m.texto.replace_range(desde..i + marca.len(), "");
    cuaderno::reemplazar(carpeta, &m)
}

/// Pasa una tarea de su lista a otra (el boton «Mover a…» del Inbox): se
/// anade al final de `hasta` con su fecha de creacion y se quita de `desde`.
/// Primero se anade y luego se quita: si algo falla entre medias, la tarea
/// queda repetida, nunca perdida.
///
/// Como al marcar, si la tarea ya no esta donde se vio (la lista cambio en el
/// chat o llego del movil) no se toca nada y se devuelve `Ok(false)`.
pub fn mover(raiz: &Path, desde: &Lista, fila: &Fila, hasta: &Lista) -> Result<bool, Fallo> {
    if desde.clave() == hasta.clave() {
        return Ok(true);
    }
    let mut origen = pedidos::mensaje_de(raiz, &desde.proyecto, &desde.codigo)?;
    let mut tareas = mini::leer_tareas(&origen.texto);
    if tareas
        .get(fila.indice)
        .is_none_or(|t| t.texto != fila.crudo)
    {
        return Ok(false);
    }
    let mut destino = pedidos::mensaje_de(raiz, &hasta.proyecto, &hasta.codigo)?;
    if !pedidos::es_lista(&destino) {
        return Err(Fallo::NoEsLista);
    }
    let mut suyas = mini::leer_tareas(&destino.texto);
    suyas.push(mini::Tarea {
        texto: imagenes_al_chat(raiz, &desde.proyecto, &hasta.proyecto, &fila.crudo)?,
        hecha: fila.hecha,
    });
    destino.texto = mini::escribir_tareas(&mini::titulo(&destino.texto), &suyas);
    pedidos::reescribir(raiz, &hasta.proyecto, &destino)?;
    tareas.remove(fila.indice);
    origen.texto = mini::escribir_tareas(&mini::titulo(&origen.texto), &tareas);
    pedidos::reescribir(raiz, &desde.proyecto, &origen)?;
    Ok(true)
}

// ------------------------------------------------------------- imagenes

/// Si un fichero es una imagen de las que puede llevar una tarea, por su
/// extension. Las extensiones viven en `v2::pegar` (las mismas que pega
/// cualquier caja v2); esta envoltura se queda porque los pedidos y el
/// timeline preguntan aqui.
pub fn es_imagen(ruta: &Path) -> bool {
    crate::v2::pegar::es_imagen(ruta)
}

/// Copia una imagen a `archivos/` del chat `proyecto` y devuelve su enlace
/// portatil (`pixpin:files/guardados/pc/<chat>/archivos/tarea-<ms>-<n>.png`).
///
/// Un nombre propio y no el del original: el enlace va dentro de un
/// `![](…)` y la sincronizacion corta la ruta en el primer `)` o blanco
/// (`disco::en_texto`), asi que «foto (1).png» no viajaria. Sin mensaje en
/// el chat: la imagen es de la tarea, como un documento lo es de su nota.
pub fn guardar_imagen(raiz: &Path, proyecto: &str, origen: &Path) -> Result<String, Fallo> {
    if !origen.is_file() {
        return Err(Fallo::SinImagen(origen.to_path_buf()));
    }
    if !es_imagen(origen) {
        return Err(Fallo::NoEsImagen(origen.to_path_buf()));
    }
    let ext = origen
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    // Un BMP (lo que deja un mapa de bits copiado y pegado en el lanzador)
    // se guarda como PNG: pesa mucho menos y es lo que cualquier movil pinta.
    let bmp = ext == "bmp";
    let ext = if bmp { "png".to_string() } else { ext };
    let carpeta = almacen::carpeta(raiz, proyecto).join("archivos");
    std::fs::create_dir_all(&carpeta)?;
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let nombre = (1..10_000)
        .map(|n| format!("tarea-{ahora}-{n:02}.{ext}"))
        .find(|n| !carpeta.join(n).exists())
        .ok_or_else(|| Fallo::Disco(std::io::Error::other("demasiadas imagenes a la vez")))?;
    // Siempre una copia: el original puede ser temporal (la cache del
    // lanzador se vacia sola a diario) y la tarea no puede depender de el.
    if bmp {
        pixpin_codec::imagen::cargar(origen)
            .and_then(|img| {
                pixpin_codec::guardar(
                    &img,
                    &carpeta.join(&nombre),
                    pixpin_codec::FormatoImagen::Png,
                )
            })
            .map_err(|e| Fallo::Imagen(e.to_string()))?;
    } else {
        std::fs::copy(origen, carpeta.join(&nombre))?;
    }
    let chat = pixpin_proyecto::vista::chat_de_ficha(raiz, proyecto)
        .unwrap_or_else(|| proyecto.to_string());
    let rel = format!("archivos/{nombre}");
    Ok(format!(
        "{}{}",
        pixpin_sincro::disco::PORTATIL,
        pixpin_proyecto::vista::virtual_de(&chat, &rel)
    ))
}

/// El texto de una tarea con sus imagenes ya copiadas al chat `proyecto`:
/// las fichas `[img NN]` cambiadas por su `![img NN](enlace)`. Primero se
/// comprueba que esten todas: una que falta no deja copiadas las demas.
pub fn con_imagenes_guardadas(
    raiz: &Path,
    proyecto: &str,
    texto: &str,
    imagenes: &[(u32, PathBuf)],
) -> Result<String, Fallo> {
    if imagenes.is_empty() {
        return Ok(texto.to_string());
    }
    for (_, r) in imagenes {
        if !r.is_file() {
            return Err(Fallo::SinImagen(r.clone()));
        }
        if !es_imagen(r) {
            return Err(Fallo::NoEsImagen(r.clone()));
        }
    }
    let mut enlaces = Vec::with_capacity(imagenes.len());
    for (n, r) in imagenes {
        enlaces.push((*n, guardar_imagen(raiz, proyecto, r)?));
    }
    Ok(mini::fichas_a_imagenes(
        mini::saneado(texto).as_str(),
        &enlaces,
    ))
}

/// Donde esta en este equipo la imagen `enlace` de una tarea de la lista
/// del chat `proyecto`. `None` si aun no llego (viene del movil y no se ha
/// sincronizado) o si el enlace no es de aqui.
pub fn ruta_de_imagen(raiz: &Path, proyecto: &str, enlace: &str) -> Option<PathBuf> {
    pixpin_proyecto::vista::ruta_real(raiz, proyecto, enlace).filter(|p| p.is_file())
}

/// El texto de una tarea que pasa del chat `desde` al chat `hasta`, con sus
/// imagenes **copiadas** al de destino y los enlaces cambiados.
///
/// Copiadas y no enlazadas al chat de origen: un enlace del movil
/// (`pixpin:files/<algo>`) se resuelve en la carpeta de CADA chat
/// (`android/<algo>`), asi que en el otro chat no se encontraria; y aunque
/// los nacidos aqui (`guardados/pc/<chat>/…`) si se encuentran desde
/// cualquiera, borrar la lista o el chat de origen dejaria la tarea sin su
/// foto. Lo que aun no esta en este equipo se deja como estaba: sin el
/// fichero no hay nada que copiar, y el enlace sigue siendo el bueno.
fn imagenes_al_chat(raiz: &Path, desde: &str, hasta: &str, crudo: &str) -> Result<String, Fallo> {
    if desde == hasta {
        return Ok(crudo.to_string());
    }
    let mut fallo = None;
    let texto = mini::cambiar_enlaces(crudo, |enlace| {
        let origen = ruta_de_imagen(raiz, desde, enlace)?;
        match guardar_imagen(raiz, hasta, &origen) {
            Ok(nuevo) => Some(nuevo),
            Err(e) => {
                fallo.get_or_insert(e);
                None
            }
        }
    });
    match fallo {
        Some(e) => Err(e),
        None => Ok(texto),
    }
}

/// El dia de hoy en el huso del usuario, para contar los dias de cada tarea
/// igual que el panel del chat.
pub fn hoy() -> mini::Fecha {
    mini::Fecha::de_ms_locales(
        pixpin_shell::entorno::ahora_utc_ms() + pixpin_shell::entorno::desfase_local_ms(),
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::path::PathBuf;

    fn dia(d: u8) -> mini::Fecha {
        mini::Fecha {
            anio: 2026,
            mes: 10,
            dia: d,
        }
    }

    fn lista(cuando: i64, documento: &str) -> Lista {
        Lista {
            proyecto: "p".into(),
            chat: "Casa".into(),
            guardados: false,
            codigo: format!("m{cuando}"),
            titulo: "Compra".into(),
            cuando,
            filas: filas_de(documento),
        }
    }

    fn textos(v: &[&Fila]) -> Vec<String> {
        v.iter().map(|f| f.texto.clone()).collect()
    }

    #[test]
    fn las_filas_separan_el_texto_de_su_fecha() {
        let f = filas_de("# Compra\n\n- [ ] pan ➕ 2026-10-01\n- [x] sal\nun parrafo suelto");
        assert_eq!(f.len(), 2, "el titulo y el parrafo no son tareas");
        assert_eq!(f[0].texto, "pan");
        assert_eq!(f[0].crudo, "pan ➕ 2026-10-01");
        assert_eq!(f[0].creada, Some(dia(1)));
        assert_eq!((f[1].indice, f[1].hecha, f[1].creada), (1, true, None));
    }

    #[test]
    fn lo_pendiente_con_fecha_va_primero_y_la_mas_vieja_arriba() {
        let l = lista(
            1,
            "- [ ] sin fecha a\n- [ ] nueva ➕ 2026-10-03\n- [x] hecha ➕ 2026-09-01\n- [ ] vieja ➕ 2026-10-01\n- [ ] sin fecha b",
        );
        let (arriba, plegadas) = l.a_la_vista(&|_| false);
        assert_eq!(
            textos(&arriba),
            ["vieja", "nueva", "sin fecha a", "sin fecha b"]
        );
        // Caso negativo: lo hecho no sale arriba aunque sea lo mas viejo.
        assert_eq!(textos(&plegadas), ["hecha"]);
    }

    #[test]
    fn la_recien_marcada_se_queda_arriba_en_su_sitio() {
        let l = lista(
            1,
            "- [ ] a ➕ 2026-10-01\n- [x] b ➕ 2026-10-02\n- [ ] c ➕ 2026-10-03\n- [x] d",
        );
        let (arriba, plegadas) = l.a_la_vista(&|f| f.crudo.starts_with('b'));
        assert_eq!(textos(&arriba), ["a", "b", "c"]);
        assert_eq!(textos(&plegadas), ["d"]);
    }

    #[test]
    fn las_listas_con_algo_pendiente_van_antes_y_la_mas_nueva_primero() {
        let mut v = vec![
            lista(10, "- [x] todo hecho"),
            lista(5, "- [ ] vieja"),
            lista(20, "- [ ] nueva"),
        ];
        ordenar_listas(&mut v);
        let orden: Vec<i64> = v.iter().map(|l| l.cuando).collect();
        assert_eq!(
            orden,
            [20, 5, 10],
            "la hecha entera va al final aunque sea mas nueva"
        );
    }

    fn almacen_de_prueba(nombre: &str) -> (PathBuf, almacen::Ficha) {
        let raiz =
            std::env::temp_dir().join(format!("pixpin-tareas-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let f = almacen::Ficha::nueva("Obra", 1, "PC01");
        almacen::Indice {
            proyectos: vec![f.clone()],
            ..Default::default()
        }
        .guardar(&raiz)
        .unwrap();
        (raiz, f)
    }

    #[test]
    fn reunir_junta_las_listas_de_todos_los_chats_y_nada_mas() {
        let (raiz, obra) = almacen_de_prueba("reunir");
        // Una en el chat «Obra» y otra apuntada en Mensajes guardados.
        pedidos::anadir_tarea(&raiz, &obra.id, "PC01", None, "yeso", "Tareas").unwrap();
        apuntar(&raiz, "PC01", "pan").unwrap();
        // Caso negativo: una nota no sale; una lista vacia sale sin tareas
        // (es adonde mover), y la ventana no la ensena.
        crate::ventana_chat::escribir_nota(
            &raiz,
            &obra.id,
            "PC01",
            "- [ ] no soy lista",
            3,
            90,
            None,
        )
        .unwrap();
        let vacia = cuaderno::Mensaje {
            id: "vacia".into(),
            clase: Some(cuaderno::Clase::MiniApp),
            miniapp: Some(mini::TAREAS.into()),
            texto: "# Nada\n\n".into(),
            ..Default::default()
        };
        cuaderno::anadir(&almacen::carpeta(&raiz, &obra.id), &vacia).unwrap();

        let (listas, proyectos) = reunir(&raiz);
        assert_eq!(proyectos.len(), 2, "Obra y Mensajes guardados");
        assert_eq!(listas.len(), 3, "{listas:#?}");
        assert!(
            listas
                .iter()
                .any(|l| l.codigo == "vacia" && l.filas.is_empty())
        );
        let de = |guardados: bool| {
            listas
                .iter()
                .find(|l| l.guardados == guardados && !l.filas.is_empty())
                .unwrap()
        };
        assert_eq!(de(false).chat, "Obra");
        assert_eq!(
            textos(&de(false).filas.iter().collect::<Vec<_>>()),
            ["yeso"]
        );
        assert_eq!(textos(&de(true).filas.iter().collect::<Vec<_>>()), ["pan"]);
        assert_eq!(de(true).titulo, INBOX, "lo apuntado va al Inbox");
        assert_eq!(listas[0].titulo, INBOX, "y el Inbox va el primero");
        // Apuntar lleva la fecha de hoy, como en el chat.
        assert_eq!(de(true).filas[0].creada, Some(hoy()));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn marcar_tacha_en_el_mensaje_y_cambia_la_firma() {
        let (raiz, obra) = almacen_de_prueba("marcar");
        pedidos::anadir_tarea(&raiz, &obra.id, "PC01", None, "yeso", "Tareas").unwrap();
        pedidos::anadir_tarea(&raiz, &obra.id, "PC01", None, "arena", "Tareas").unwrap();
        let (listas, proyectos) = reunir(&raiz);
        let antes = firma(&raiz, &proyectos);
        let l = &listas[0];
        assert!(marcar(&raiz, l, &l.filas[1], true).unwrap());
        let doc = pedidos::mensaje_de(&raiz, &obra.id, &l.codigo)
            .unwrap()
            .texto;
        let hechas: Vec<bool> = mini::leer_tareas(&doc).iter().map(|t| t.hecha).collect();
        assert_eq!(hechas, [false, true], "solo la pedida, en su mensaje");
        assert_ne!(firma(&raiz, &proyectos), antes, "y la ventana se entera");
        // Caso negativo: si la tarea cambio desde que se leyo, no se toca.
        let mut vieja = l.filas[0].clone();
        vieja.crudo = "otra cosa".into();
        assert!(!marcar(&raiz, l, &vieja, true).unwrap());
        let doc2 = pedidos::mensaje_de(&raiz, &obra.id, &l.codigo)
            .unwrap()
            .texto;
        assert_eq!(doc2, doc);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn quitar_y_deshacer_la_devuelven_a_su_sitio_con_su_fecha() {
        let (raiz, obra) = almacen_de_prueba("quitar");
        for t in ["yeso", "arena", "cal"] {
            pedidos::anadir_tarea(&raiz, &obra.id, "PC01", None, t, "Obra").unwrap();
        }
        let (listas, _) = reunir(&raiz);
        let l = &listas[0];
        let quitada = quitar(&raiz, l, &l.filas[1]).unwrap().unwrap();
        assert_eq!(quitada.texto, l.filas[1].crudo, "con su fecha");
        let (listas, _) = reunir(&raiz);
        assert_eq!(
            textos(&listas[0].filas.iter().collect::<Vec<_>>()),
            ["yeso", "cal"]
        );
        // Caso negativo: la que se vio ya no esta en ese numero; no se toca.
        let mut vieja = listas[0].filas[1].clone();
        vieja.crudo = "otra".into();
        assert!(quitar(&raiz, &listas[0], &vieja).unwrap().is_none());
        reponer(&raiz, &obra.id, &l.codigo, 1, quitada).unwrap();
        let (listas, _) = reunir(&raiz);
        assert_eq!(
            textos(&listas[0].filas.iter().collect::<Vec<_>>()),
            ["yeso", "arena", "cal"]
        );
        assert_eq!(listas[0].filas[1].creada, Some(hoy()));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn borrar_el_inbox_se_lleva_sus_tareas_y_apuntar_lo_vuelve_a_crear() {
        let (raiz, _) = almacen_de_prueba("borrar-inbox");
        apuntar(&raiz, "PC01", "pan").unwrap();
        let (listas, _) = reunir(&raiz);
        let inbox = listas.iter().find(|l| es_inbox(l)).unwrap();
        borrar_lista(&raiz, inbox).unwrap();
        assert!(reunir(&raiz).0.iter().all(|l| !es_inbox(l)));
        // Caso negativo: borrarla otra vez dice que ya no esta.
        assert!(matches!(
            borrar_lista(&raiz, inbox),
            Err(Fallo::SinMensaje(_))
        ));
        apuntar(&raiz, "PC01", "leche").unwrap();
        let (listas, _) = reunir(&raiz);
        let nuevo = listas.iter().find(|l| es_inbox(l)).unwrap();
        assert_eq!(textos(&nuevo.filas.iter().collect::<Vec<_>>()), ["leche"]);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn la_hora_de_una_tarea_va_y_vuelve_por_su_texto() {
        // 2026-10-09 10:30, hora local.
        let t = (dias_de(2026, 10, 9) * 1440 + 10 * 60 + 30) * 60_000;
        assert_eq!(fecha_de(dias_de(2026, 10, 9)), (2026, 10, 9));
        assert_eq!(fecha_de(dias_de(2024, 2, 29)), (2024, 2, 29), "bisiesto");
        assert_eq!(marca_de_hora(t), "⏰ 2026-10-09 10:30");
        let con = con_hora("Llamar al banco", t);
        assert_eq!(con, "Llamar al banco ⏰ 2026-10-09 10:30");
        assert_eq!(hora_de_tarea(&con), Some(t));
        assert_eq!(sin_hora(&con), "Llamar al banco");
        // Cambiarla no la repite.
        let otra = con_hora(&con, t + 60_000);
        assert_eq!(otra.matches(RELOJ).count(), 1);
        // Casos negativos: un reloj suelto o una fecha imposible no son hora.
        assert_eq!(hora_de_tarea("despertar ⏰ pronto"), None);
        assert_eq!(hora_de_tarea("x ⏰ 2026-13-01 10:00"), None);
        assert_eq!(sin_hora("x ⏰ pronto"), "x ⏰ pronto");
    }

    #[test]
    fn una_tarea_con_hora_entra_en_la_agenda_y_al_sonar_pierde_su_marca() {
        let (raiz, obra) = almacen_de_prueba("tarea-con-hora");
        let t = (dias_de(2030, 1, 2) * 1440 + 9 * 60) * 60_000;
        let texto = con_hora("yeso", t);
        let m = pedidos::anadir_tarea(&raiz, &obra.id, "PC01", None, &texto, "Obra").unwrap();
        pedidos::anadir_tarea(&raiz, &obra.id, "PC01", Some(&m.id), "arena", "Obra").unwrap();
        let carpeta = almacen::carpeta(&raiz, &obra.id);
        let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
        let avisos = avisos_del_cuaderno(&c);
        assert_eq!(avisos.len(), 1, "solo la que tiene hora");
        assert!(avisos[0].texto.contains("yeso") && !avisos[0].texto.contains(RELOJ));
        assert!(olvidar_hora(&carpeta, &avisos[0].id).unwrap());
        let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
        assert!(avisos_del_cuaderno(&c).is_empty());
        let (listas, _) = reunir(&raiz);
        let l = listas.iter().find(|l| !l.guardados).unwrap();
        assert_eq!(textos(&l.filas.iter().collect::<Vec<_>>()), ["yeso", "arena"]);
        // Caso negativo: un id que no es de tarea no toca nada.
        assert!(!olvidar_hora(&carpeta, "otra-cosa").unwrap());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn abierta_desde_una_lista_del_chat_apunta_en_esa_lista_y_no_en_el_inbox() {
        let (raiz, obra) = almacen_de_prueba("apuntar-en");
        pedidos::anadir_tarea(&raiz, &obra.id, "PC01", None, "yeso", "Obra").unwrap();
        let (listas, _) = reunir(&raiz);
        let grupo = listas.iter().find(|l| !l.guardados).unwrap().clone();
        apuntar_en(&raiz, "PC01", &grupo, "arena", &[]).unwrap();
        let (listas, _) = reunir(&raiz);
        let grupo2 = listas.iter().find(|l| l.clave() == grupo.clave()).unwrap();
        assert_eq!(
            textos(&grupo2.filas.iter().collect::<Vec<_>>()),
            ["yeso", "arena"]
        );
        assert!(listas.iter().all(|l| !es_inbox(l)), "el Inbox ni se crea");
        // Caso negativo: vacia no apunta nada.
        assert!(matches!(
            apuntar_en(&raiz, "PC01", &grupo, "  ", &[]),
            Err(Fallo::TareaVacia)
        ));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn mover_lleva_la_tarea_del_inbox_al_grupo_con_su_fecha() {
        let (raiz, obra) = almacen_de_prueba("mover");
        pedidos::anadir_tarea(&raiz, &obra.id, "PC01", None, "yeso", "Obra").unwrap();
        apuntar(&raiz, "PC01", "arena").unwrap();
        apuntar(&raiz, "PC01", "pan").unwrap();
        let (listas, _) = reunir(&raiz);
        let inbox = listas.iter().find(|l| es_inbox(l)).unwrap();
        let grupo = listas.iter().find(|l| !l.guardados).unwrap();
        assert!(mover(&raiz, inbox, &inbox.filas[0], grupo).unwrap());
        let (listas, _) = reunir(&raiz);
        let inbox2 = listas.iter().find(|l| es_inbox(l)).unwrap();
        let grupo2 = listas.iter().find(|l| !l.guardados).unwrap();
        assert_eq!(textos(&inbox2.filas.iter().collect::<Vec<_>>()), ["pan"]);
        assert_eq!(
            textos(&grupo2.filas.iter().collect::<Vec<_>>()),
            ["yeso", "arena"]
        );
        assert_eq!(grupo2.filas[1].creada, Some(hoy()), "conserva su fecha");
        assert_eq!(grupo2.titulo, "Obra", "y el titulo del grupo");
        // Caso negativo: una tarea que ya no esta donde se vio no se mueve.
        let mut vieja = inbox2.filas[0].clone();
        vieja.crudo = "otra".into();
        assert!(!mover(&raiz, inbox2, &vieja, grupo2).unwrap());
        assert_eq!(
            reunir(&raiz)
                .0
                .iter()
                .find(|l| es_inbox(l))
                .unwrap()
                .filas
                .len(),
            1
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// Una imagen de verdad (un PNG de 2x2) fuera del almacen, como la que
    /// se pega o se copia del Explorador.
    fn foto_de_fuera(raiz: &Path, nombre: &str) -> PathBuf {
        let r = raiz.join("fuera").join(nombre);
        std::fs::create_dir_all(r.parent().unwrap()).unwrap();
        let img = pixpin_codec::ImagenRgba {
            ancho: 2,
            alto: 2,
            pixeles: vec![200; 16],
        };
        pixpin_codec::guardar(&img, &r, pixpin_codec::FormatoImagen::Png).unwrap();
        r
    }

    #[test]
    fn apuntar_con_imagenes_las_guarda_en_el_chat_y_las_enlaza_antes_de_la_fecha() {
        let (raiz, _) = almacen_de_prueba("imagenes");
        let a = foto_de_fuera(&raiz, "mi foto (1).png");
        let b = foto_de_fuera(&raiz, "b.jpg");
        apuntar_con(
            &raiz,
            "PC01",
            "yeso [img 01] para el muro",
            &[(1, a.clone()), (2, b)],
        )
        .unwrap();
        let (listas, _) = reunir(&raiz);
        let inbox = listas.iter().find(|l| es_inbox(l)).unwrap();
        let f = &inbox.filas[0];
        assert_eq!(f.texto, "yeso para el muro", "lo que se lee, sin enlaces");
        assert_eq!(f.creada, Some(hoy()), "y la fecha se sigue leyendo");
        assert_eq!(f.imagenes.len(), 2);
        // La 1 en su sitio, la 2 (sin ficha) al final, y la fecha detras.
        assert!(
            f.crudo
                .starts_with("yeso ![img 01](pixpin:files/guardados/pc/general/archivos/tarea-"),
            "{}",
            f.crudo
        );
        assert!(f.crudo.contains(") para el muro ![img 02]("), "{}", f.crudo);
        // Cada enlace lleva a una copia de este equipo, sin blancos ni parentesis.
        for e in &f.imagenes {
            let r = ruta_de_imagen(&raiz, &inbox.proyecto, e).unwrap_or_else(|| panic!("{e}"));
            assert!(r.starts_with(almacen::carpeta(&raiz, &inbox.proyecto).join("archivos")));
            assert!(!e.contains(' ') && !e.contains('('));
        }
        // El original se queda donde estaba.
        assert!(a.is_file());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn un_bmp_se_guarda_como_png_y_siempre_es_una_copia() {
        let (raiz, _) = almacen_de_prueba("bmp");
        // Un BMP de 1x1, 24 bits, como el que deja un mapa pegado en el lanzador.
        let mut bmp = Vec::new();
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&58u32.to_le_bytes());
        bmp.extend_from_slice(&[0, 0, 0, 0]);
        bmp.extend_from_slice(&54u32.to_le_bytes());
        bmp.extend_from_slice(&40u32.to_le_bytes());
        bmp.extend_from_slice(&1i32.to_le_bytes());
        bmp.extend_from_slice(&1i32.to_le_bytes());
        bmp.extend_from_slice(&1u16.to_le_bytes());
        bmp.extend_from_slice(&24u16.to_le_bytes());
        bmp.extend_from_slice(&[0; 24]);
        bmp.extend_from_slice(&[255, 0, 0, 0]);
        let origen = raiz
            .join("cache")
            .join("lanzador-imagenes")
            .join("pegada.bmp");
        std::fs::create_dir_all(origen.parent().unwrap()).unwrap();
        std::fs::write(&origen, &bmp).unwrap();
        apuntar_con(&raiz, "PC01", "[img 01] boceto", &[(1, origen.clone())]).unwrap();
        // La cache del lanzador se vacia sola: la tarea no puede depender de ella.
        std::fs::remove_file(&origen).unwrap();
        let (listas, _) = reunir(&raiz);
        let inbox = listas.iter().find(|l| es_inbox(l)).unwrap();
        let e = &inbox.filas[0].imagenes[0];
        assert!(e.ends_with(".png"), "{e}");
        let r = ruta_de_imagen(&raiz, &inbox.proyecto, e).unwrap();
        let img = pixpin_codec::imagen::cargar(&r).unwrap();
        assert_eq!((img.ancho, img.alto), (1, 1));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn caso_negativo_una_imagen_que_falta_no_apunta_nada_ni_copia_las_demas() {
        let (raiz, _) = almacen_de_prueba("imagen-falta");
        let a = foto_de_fuera(&raiz, "a.png");
        let r = apuntar_con(
            &raiz,
            "PC01",
            "x [img 01]",
            &[(1, a), (2, raiz.join("no-esta.png"))],
        );
        assert!(matches!(r, Err(Fallo::SinImagen(_))), "{r:?}");
        let texto = raiz.join("fuera").join("nota.txt");
        std::fs::write(&texto, "hola").unwrap();
        assert!(matches!(
            apuntar_con(&raiz, "PC01", "x", &[(1, texto)]),
            Err(Fallo::NoEsImagen(_))
        ));
        let (listas, _) = reunir(&raiz);
        assert!(listas.iter().all(|l| l.filas.is_empty()), "nada apuntado");
        let archivos = almacen::Indice::leer(&raiz)
            .proyectos
            .iter()
            .map(|f| almacen::carpeta(&raiz, &f.id).join("archivos"))
            .filter(|c| c.is_dir())
            .count();
        assert_eq!(archivos, 0, "ni copiada la buena");
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn mover_lleva_las_imagenes_al_chat_de_destino() {
        let (raiz, obra) = almacen_de_prueba("mover-imagenes");
        pedidos::anadir_tarea(&raiz, &obra.id, "PC01", None, "yeso", "Obra").unwrap();
        let a = foto_de_fuera(&raiz, "a.png");
        apuntar_con(&raiz, "PC01", "foto del muro", &[(1, a)]).unwrap();
        let (listas, _) = reunir(&raiz);
        let inbox = listas.iter().find(|l| es_inbox(l)).unwrap();
        let grupo = listas.iter().find(|l| !l.guardados).unwrap();
        let vieja = inbox.filas[0].imagenes[0].clone();
        assert!(mover(&raiz, inbox, &inbox.filas[0], grupo).unwrap());
        let (listas, _) = reunir(&raiz);
        let grupo = listas.iter().find(|l| !l.guardados).unwrap();
        let movida = &grupo.filas[1];
        assert_eq!(movida.texto, "foto del muro");
        assert_eq!(movida.creada, Some(hoy()));
        let nueva = &movida.imagenes[0];
        assert_ne!(nueva, &vieja, "el enlace es ahora del otro chat");
        let r = ruta_de_imagen(&raiz, &obra.id, nueva).unwrap();
        assert!(
            r.starts_with(almacen::carpeta(&raiz, &obra.id).join("archivos")),
            "{}",
            r.display()
        );
        // Caso negativo: un enlace del movil que aun no llego se deja igual.
        let crudo = "x ![img 01](pixpin:files/guardados/otra/no-llego.png) ➕ 2026-10-03";
        assert_eq!(
            imagenes_al_chat(&raiz, "g", &obra.id, crudo).unwrap(),
            crudo
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn apuntar_algo_vacio_no_crea_nada() {
        let (raiz, _) = almacen_de_prueba("vacio");
        assert!(matches!(
            apuntar(&raiz, "PC01", "   "),
            Err(Fallo::TareaVacia)
        ));
        assert!(reunir(&raiz).0.is_empty());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn corregir_cambia_el_texto_y_conserva_la_fecha_y_las_imagenes() {
        let (raiz, obra) = almacen_de_prueba("corregir");
        let foto = foto_de_fuera(&raiz, "muro.png");
        pedidos::anadir_tarea(&raiz, &obra.id, "PC01", None, "yeso", "Obra").unwrap();
        let (listas, _) = reunir(&raiz);
        let l = &listas[0];
        // Primero con una imagen pegada en la caja: se guarda y se enlaza.
        assert!(corregir(&raiz, l, &l.filas[0], "yeso fino [img 01]", &[(1, foto)]).unwrap());
        let (listas, _) = reunir(&raiz);
        let f = &listas[0].filas[0];
        assert_eq!(f.texto, "yeso fino");
        assert_eq!(f.imagenes.len(), 1);
        assert_eq!(f.creada, Some(hoy()), "la fecha no cambia");
        // Despues solo el texto: la imagen de antes se queda.
        assert!(corregir(&raiz, &listas[0], f, "yeso grueso", &[]).unwrap());
        let (listas, _) = reunir(&raiz);
        let f2 = &listas[0].filas[0];
        assert_eq!(f2.texto, "yeso grueso");
        assert_eq!(f2.imagenes, f.imagenes);
        // Caso negativo: si la tarea cambio fuera no se toca, y vaciarla no
        // la borra.
        let mut vieja = f2.clone();
        vieja.crudo = "otra".into();
        assert!(!corregir(&raiz, &listas[0], &vieja, "nada", &[]).unwrap());
        let sin_imagen = Fila {
            imagenes: vec![],
            ..f2.clone()
        };
        assert!(matches!(
            corregir(&raiz, &listas[0], &sin_imagen, "  ", &[]),
            Err(Fallo::TareaVacia)
        ));
        assert_eq!(reunir(&raiz).0[0].filas[0].texto, "yeso grueso");
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
