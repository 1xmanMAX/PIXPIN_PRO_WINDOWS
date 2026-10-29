//! **Los temas de la reunion en la app** (28-oct-2026, `docs/jev.md`): el
//! cliente de Jev para marcarlos (y por que no lo hay), la lista de tareas
//! del chat que los lleva, lo que se cambia a mano al estudiar la clase y lo
//! que se pasa a la proxima reunion o a Tareas.
//!
//! Lo puro (el formato `resto["temas"]`, lo hecho a mano manda) vive en
//! `pixpin_proyecto::temas`; las preguntas y su tabla, en
//! `pixpin_typesafe::temas`; la escucha en vivo mientras se graba, en
//! `ventana_chat::grabar_clase`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use pixpin_proyecto::temas::{self, Tema};
use pixpin_proyecto::{almacen, cuaderno, mini};
use pixpin_store::Ubicacion;
use pixpin_typesafe::Cliente;

use super::Funcion;
use crate::pedidos::{self, Fallo};

/// Lo que espera Jev a una peticion de la escucha en vivo: lleva tres
/// preguntas por tema (hasta 60) y va de fondo, pero mientras esta en vuelo
/// no sale la siguiente: una que se atasca no puede retrasar el ✓ mucho
/// mas que esto.
const TIEMPO: Duration = Duration::from_secs(4);

/// Por que no marca Jev.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinJev {
    /// No hay clave de TypeSafe.
    SinClave,
    /// La funcion esta apagada en Ajustes.
    Apagado,
}

/// **El cliente de Jev para los temas**, o por que no lo hay (entonces se
/// marcan a mano).
pub fn cliente(u: &Ubicacion) -> Result<Cliente, SinJev> {
    match super::para(u, Funcion::Temas) {
        Some(c) => Ok(c.con_tiempo(TIEMPO)),
        None if super::hay_clave() => Err(SinJev::Apagado),
        None => Err(SinJev::SinClave),
    }
}

/// Una lista de tareas de la que sacar los temas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListaDeTemas {
    pub proyecto: String,
    pub codigo: String,
    pub titulo: String,
    /// Lo pendiente de la lista, ya limpio.
    pub temas: Vec<String>,
}

/// **Las listas de tareas con algo pendiente**, primero las del chat
/// `proyecto` y luego las demas (en el orden de la ventana Tareas).
pub fn listas(raiz: &Path, proyecto: &str) -> Vec<ListaDeTemas> {
    let (todas, _) = crate::tareas::reunir(raiz);
    let mut v: Vec<ListaDeTemas> = todas
        .into_iter()
        .filter(|l| l.cuantas_pendientes() > 0)
        .map(|l| ListaDeTemas {
            temas: l
                .filas
                .iter()
                .filter(|f| !f.hecha)
                .map(|f| temas::limpio(&f.texto))
                .filter(|t| !t.is_empty())
                .collect(),
            proyecto: l.proyecto,
            codigo: l.codigo,
            titulo: l.titulo,
        })
        .filter(|l| !l.temas.is_empty())
        .collect();
    // Estable: dentro de cada grupo, el orden de Tareas.
    v.sort_by_key(|l| l.proyecto != proyecto);
    v
}

/// **Escribe la lista de tareas de la reunion**: en la lista elegida al
/// preparar (`enlazada`, si sigue estando: se le anaden los temas nuevos y
/// se marcan los tratados) o en una nueva en el chat `proyecto`, con
/// `titulo`. Devuelve la lista, `(proyecto, codigo)`.
pub fn guardar_lista(
    raiz: &Path,
    proyecto: &str,
    aparato: &str,
    titulo: &str,
    lista: &[Tema],
    enlazada: Option<&(String, String)>,
) -> Result<(String, String), Fallo> {
    let hoy = mini::Fecha::de_ms_locales(
        pixpin_shell::entorno::ahora_utc_ms() + pixpin_shell::entorno::desfase_local_ms(),
    );
    let enlazada = enlazada
        .and_then(|(p, c)| pedidos::mensaje_de(raiz, p, c).ok().map(|m| (p.clone(), m)))
        .filter(|(_, m)| pedidos::es_lista(m));
    let (proyecto, mut m) = match enlazada {
        Some(x) => x,
        None => (
            proyecto.to_string(),
            pedidos::nueva_lista(raiz, proyecto, aparato, titulo)?,
        ),
    };
    // Los temas que la lista aun no tiene, al final y con la fecha de hoy.
    let ya: Vec<String> = temas::de_documento(&m.texto)
        .into_iter()
        .map(|(t, _)| t.to_lowercase())
        .collect();
    for t in lista {
        if !ya.contains(&t.texto.to_lowercase()) {
            m.texto = mini::anadir_el(&m.texto, &t.texto, hoy);
        }
    }
    // Y lo tratado, marcado.
    for (i, _) in temas::marcas_para_la_lista(lista, &m.texto) {
        m.texto = mini::alternar(&m.texto, i);
    }
    pedidos::reescribir(raiz, &proyecto, &m)?;
    crate::tareas::refrescar();
    Ok((proyecto, m.id))
}

/// **Un tema cambiado a mano en la nota** `id` (al estudiar la clase): en
/// `resto["temas"]` y en su lista de tareas. Devuelve si cambio.
pub fn cambiar_a_mano(
    raiz: &Path,
    proyecto: &str,
    id: &str,
    tema: &str,
    hecho: bool,
) -> Result<bool, String> {
    let carpeta = almacen::carpeta(raiz, proyecto);
    let mut texto = None;
    let mut lista = None;
    let cambiada = cuaderno::cambiar(&carpeta, id, |m| {
        let mut v = temas::temas_de(&m.resto);
        if !temas::a_mano(&mut v, tema, hecho, None) {
            return false;
        }
        texto = v.iter().find(|t| t.id == tema).map(|t| t.texto.clone());
        lista = temas::lista_de(&m.resto);
        temas::poner_temas(&mut m.resto, &v);
        true
    })
    .map_err(|e| e.to_string())?;
    if cambiada.is_none() {
        return Ok(false);
    }
    if let (Some(texto), Some((p, c))) = (texto, lista) {
        // La casilla de la lista con el mismo texto.
        let indice = pedidos::mensaje_de(raiz, &p, &c).ok().and_then(|m| {
            mini::leer_tareas(&m.texto).iter().position(|t| {
                let (visible, _) = mini::partir(&t.texto);
                temas::limpio(visible).to_lowercase() == texto.to_lowercase()
            })
        });
        if let Some(i) = indice
            && let Err(e) = pedidos::marcar_tarea(raiz, &p, &c, i, hecho)
        {
            tracing::warn!(?e, "el tema no se marco en su lista de tareas");
        }
        crate::tareas::refrescar();
    }
    crate::ventana_chat::refrescar();
    Ok(true)
}

// --- Lo que pasa a la proxima reunion ---------------------------------------

/// Donde esperan los temas de la proxima reunion: al lado de las notas de
/// voz (`<datos>/voz`, `voz::carpeta_de_audios`), en este equipo, como el
/// modo de grabar. Uno por linea.
fn fichero_de_la_proxima(raiz: &Path) -> PathBuf {
    raiz.join("voz").join("temas-para-la-proxima.txt")
}

/// Los temas que esperan a la proxima reunion.
pub fn para_la_proxima(raiz: &Path) -> Vec<String> {
    std::fs::read_to_string(fichero_de_la_proxima(raiz))
        .map(|t| temas::de_texto(&t))
        .unwrap_or_default()
}

/// **Pasa estos temas a la proxima reunion**: se suman a los que ya
/// esperaban (sin repetir) y salen solos al preparar la siguiente.
/// Devuelve cuantos esperan ahora.
pub fn guardar_para_la_proxima(raiz: &Path, nuevos: &[String]) -> std::io::Result<usize> {
    let mut v = para_la_proxima(raiz);
    v.extend(nuevos.iter().cloned());
    let v = temas::de_texto(&v.join("\n"));
    let f = fichero_de_la_proxima(raiz);
    if let Some(d) = f.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(&f, v.join("\n"))?;
    Ok(v.len())
}

/// Ya se usaron (empezo la reunion con ellos).
pub fn olvidar_la_proxima(raiz: &Path) {
    let _ = std::fs::remove_file(fichero_de_la_proxima(raiz));
}

/// **Apunta en el Inbox de Tareas** los temas sin tratar, cada uno con su
/// nota (`💬`) de que reunion viene. Devuelve cuantos.
pub fn a_tareas(raiz: &Path, sin_tratar: &[String], reunion: &str) -> usize {
    let aparato = crate::letras_que_viajan::aparato(raiz);
    let mut n = 0;
    for t in sin_tratar {
        let texto = format!("{t} {} 🎙 {}", crate::tareas::frase::NOTA, reunion.trim());
        match crate::tareas::apuntar(raiz, &aparato, &texto) {
            Ok(_) => n += 1,
            Err(e) => tracing::warn!(?e, "un tema sin tratar no se apunto en Tareas"),
        }
    }
    if n > 0 {
        crate::tareas::refrescar();
    }
    n
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn sin_clave_no_hay_cliente_y_se_dice_por_que() {
        let u = Ubicacion::Instalado {
            raiz: std::env::temp_dir().join("pixpin-jev-temas-sin-clave"),
        };
        assert_eq!(cliente(&u).err(), Some(SinJev::SinClave));
    }

    #[test]
    fn los_de_la_proxima_se_suman_sin_repetir_y_se_olvidan() {
        let raiz =
            std::env::temp_dir().join(format!("pixpin-temas-proxima-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        let u = raiz.as_path();
        assert!(para_la_proxima(u).is_empty());
        assert_eq!(
            guardar_para_la_proxima(u, &["Viaje".into(), "Varios".into()]).unwrap(),
            2
        );
        assert_eq!(
            guardar_para_la_proxima(u, &["viaje".into(), "Becario".into()]).unwrap(),
            3
        );
        assert_eq!(para_la_proxima(u), vec!["Viaje", "Varios", "Becario"]);
        olvidar_la_proxima(u);
        assert!(para_la_proxima(u).is_empty());
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
