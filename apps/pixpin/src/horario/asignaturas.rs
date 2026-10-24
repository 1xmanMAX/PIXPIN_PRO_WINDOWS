//! **Cada asignatura es un grupo de tareas** (18-oct-2026).
//!
//! El usuario: «que cada asignatura se cree como un grupo de tareas, y
//! cuando cree una tarea pueda añadirla a una asignatura».
//!
//! POR QUE asi: en PixPin (y en el movil) un «grupo de tareas» ya es una
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

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use pixpin_ia::acciones::normalizar;
use pixpin_proyecto::{almacen, cuaderno};

use crate::pedidos::{self, Fallo};
use crate::tareas::{self, Lista};

/// Como se llama el chat de las asignaturas.
pub const NOMBRE_CHAT: &str = "Asignaturas";
/// Su marca en el indice.
const MARCA: &str = "asignaturas";

fn tiene_marca(f: &almacen::Ficha) -> bool {
    f.resto.get(MARCA).and_then(|v| v.as_bool()) == Some(true)
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
    if let Some(f) = indice.proyectos.iter().find(|f| tiene_marca(f)) {
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
            f.clone()
        }
        None => {
            let ahora = pixpin_shell::entorno::ahora_utc_ms();
            let mut f = almacen::Ficha::nueva(NOMBRE_CHAT, ahora, aparato);
            f.resto.insert(marca.0, marca.1);
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

/// La lista de tareas de la asignatura, creandola (y el chat) si no esta.
pub fn asegurar_lista(raiz: &Path, aparato: &str, nombre: &str) -> Result<Lista, Fallo> {
    if let Some(l) = de(&listas(raiz), nombre) {
        return Ok(l.clone());
    }
    let f = asegurar_ficha(raiz, aparato)?;
    let m = pedidos::nueva_lista(raiz, &f.id, aparato, nombre.trim())?;
    tareas::lista_de(&f, &m).ok_or(Fallo::NoEsLista)
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
    let lista = asegurar_lista(raiz, aparato, nombre)?;
    let m = tareas::apuntar_en(raiz, aparato, &lista, texto, imagenes)?;
    Ok((lista.proyecto, m))
}

/// Los nombres de las asignaturas del horario, para el desplegable de la
/// ventana de Tareas.
pub fn nombres(raiz: &Path) -> Vec<String> {
    let ahora = pixpin_shell::entorno::ahora_local_ms();
    super::de_disco(raiz, pixpin_ia::fechas::Momento::de_ms_locales(ahora))
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
