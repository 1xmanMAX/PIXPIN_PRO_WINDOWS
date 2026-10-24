//! **La Jump List del icono, al dia** (13-oct-2026): las cuatro entradas
//! fijas y los tres ultimos proyectos tocados, como los atajos del icono de
//! Android (`Atajos.ponerAlDia`). La lista y lo que pide cada entrada son de
//! `pixpin_shell::jump_list`; aqui se leen los proyectos y se pone.
//!
//! Un hilo propio mira el indice de proyectos cada minuto (un JSON pequeño)
//! y solo toca el sistema si la lista cambio: barato, y sin tener que
//! enganchar cada sitio que crea, renombra o abre un proyecto.

use std::time::Duration;

use pixpin_shell::jump_list::{self, Reciente, Textos};
use pixpin_store::{Idioma, Ubicacion};

/// Cada cuanto se mira si cambiaron los proyectos.
const CADA: Duration = Duration::from_secs(60);

/// El pedido de un atajo, si esta copia la lanzo uno (`--atajo ...`).
pub fn pedido_del_arranque() -> Option<String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    jump_list::pedido_de_atajo(&args)
}

fn textos(idioma: Idioma) -> Textos {
    match idioma {
        Idioma::Ingles => Textos {
            capturar: "Capture".into(),
            capturar_largo: "Capture an area of the screen".into(),
            mensajes: "Saved messages".into(),
            mensajes_largo: "Open Saved messages".into(),
            proyectos: "Projects".into(),
            proyectos_largo: "See your projects".into(),
            sincronizar: "Sync".into(),
            sincronizar_largo: "Sync with your devices".into(),
            recientes: "Recent projects".into(),
            proyecto_largo: "Open the project {}".into(),
        },
        _ => Textos::default(),
    }
}

/// Los proyectos que pueden ir: con nombre, sin archivar, y cual es el de
/// «Mensajes guardados» (que ya tiene su entrada fija).
fn recientes(raiz: &std::path::Path) -> (Vec<Reciente>, String) {
    let indice = pixpin_proyecto::almacen::Indice::leer(raiz);
    let guardados = indice
        .proyectos
        .iter()
        .find(|f| f.es_guardados())
        .map(|f| f.id.clone())
        .unwrap_or_default();
    let v = indice
        .proyectos
        .iter()
        .filter(|f| f.resto.get("archivado").and_then(|v| v.as_bool()) != Some(true))
        .map(|f| Reciente {
            id: f.id.clone(),
            nombre: f.nombre.clone(),
            tocado: f.tocado,
        })
        .collect();
    (v, guardados)
}

/// Pone la lista y la tiene al dia, en su hilo.
pub fn poner_al_dia(idioma: Idioma, ubicacion: Ubicacion) {
    let lanzado = std::thread::Builder::new()
        .name("lista-del-icono".into())
        .spawn(move || {
            let _com = pixpin_shell::ComDelHilo::iniciar();
            let Ok(exe) = std::env::current_exe() else {
                return;
            };
            let t = textos(idioma);
            let mut puesta = String::new();
            loop {
                let (proyectos, guardados) = recientes(ubicacion.raiz());
                let lista = jump_list::armar(&proyectos, &t, &guardados);
                let firma = lista.firma();
                if firma != puesta {
                    match jump_list::poner(&exe, &lista, &t.recientes) {
                        Ok(()) => puesta = firma,
                        Err(e) => tracing::info!(?e, "no se pudo poner la lista del icono"),
                    }
                }
                std::thread::sleep(CADA);
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo lanzar el hilo de la lista del icono");
    }
}
