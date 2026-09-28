//! Donde se guarda un proyecto, desde la lista: «Guardar en…» al crearlo,
//! «Cambiar ubicacion…», «Volver a la ubicacion habitual» y «Abrir carpeta
//! del proyecto». El trabajo de verdad (mover, la union, la papelera) esta en
//! `pixpin_proyecto::ubicacion`; aqui solo el dialogo, cerrar lo abierto
//! antes de mover y el aviso.
//!
//! Va aparte de `ventana_chat.rs` porque ese fichero lo tocan muchos a la vez
//! y esto no necesita mas que cuatro cosas suyas.

use pixpin_proyecto::{almacen, ubicacion};
use pixpin_shell::overlay::VentanaOverlay;
use pixpin_store::{Catalogo, Ubicacion};

use super::{Abierto, abrir_proyecto, apagar_lienzo, cerrar_panel};

/// Lo que dice el aviso cuando algo de esto falla.
pub(super) fn texto_de_error(textos: &Catalogo, e: &ubicacion::Error) -> String {
    match e {
        ubicacion::Error::NoDisponible(ruta) => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("ruta", ruta.display().to_string());
            textos.t_args("ubicacion-no-disponible-abrir", &args)
        }
        otro => textos.t(otro.clave()),
    }
}

/// Que se quiere hacer con la carpeta del proyecto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Cambio {
    /// Elegir una carpeta y moverlo alli.
    AOtra,
    /// Traerlo de vuelta a `proyectos/<id>`.
    AHabitual,
}

/// Mueve el proyecto `id` y devuelve el aviso que hay que ensenar, o `None`
/// si el usuario cancelo el dialogo.
///
/// Si el proyecto esta abierto se cierra antes —la hoja, la mini-aplicacion
/// y el lienzo vivo guardan lo suyo y sueltan sus ficheros— y se vuelve a
/// abrir despues: con un fichero abierto el traslado fallaria, y aunque no
/// se pierda nada, el usuario solo veria «no se pudo».
pub(super) fn cambiar(
    ventana: &VentanaOverlay,
    textos: &Catalogo,
    donde: &Ubicacion,
    id: &str,
    cambio: Cambio,
    abierto: &mut Option<Abierto>,
    borradores: &mut std::collections::HashMap<String, String>,
) -> Option<String> {
    let raiz = donde.raiz();
    let elegida = match cambio {
        Cambio::AOtra => Some(pixpin_shell::elegir_carpeta::pedir_carpeta(
            ventana.handle(),
            &textos.t("ubicacion-elegir-titulo"),
        )?),
        Cambio::AHabitual => None,
    };
    let estaba_abierto = abierto.as_ref().is_some_and(|a| a.ficha.id == id);
    if estaba_abierto {
        if let Some(a) = abierto.as_mut() {
            cerrar_panel(donde, a);
            apagar_lienzo(donde, a);
        }
        if let Some(a) = abierto.take() {
            borradores.insert(a.ficha.id.clone(), a.borrador);
        }
    }
    let hecho = match &elegida {
        Some(carpeta) => ubicacion::mover_a(raiz, id, carpeta).map(|d| {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("ruta", d.display().to_string());
            textos.t_args("ubicacion-movido", &args)
        }),
        None => ubicacion::volver_a_habitual(raiz, id).map(|_| textos.t("ubicacion-vuelto")),
    };
    let aviso = match hecho {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(?e, proyecto = id, "no se pudo cambiar la carpeta del proyecto");
            texto_de_error(textos, &e)
        }
    };
    if estaba_abierto
        && let Some(f) = almacen::Indice::leer(raiz).buscar(id).cloned()
    {
        let mut nuevo = abrir_proyecto(donde, &f);
        nuevo.borrador = borradores.remove(id).unwrap_or_default();
        *abierto = Some(nuevo);
    }
    Some(aviso)
}

/// Abre en el Explorador la carpeta de verdad del proyecto: la elegida si la
/// tiene, no la union de `AppData`, que al usuario no le dice nada.
pub(super) fn abrir_carpeta(textos: &Catalogo, donde: &Ubicacion, ficha: &almacen::Ficha) -> Option<String> {
    let raiz = donde.raiz();
    if let Err(e) = ubicacion::preparar(raiz, ficha) {
        return Some(texto_de_error(textos, &e));
    }
    let carpeta = ubicacion::carpeta_real(raiz, &ficha.id);
    // Un proyecto recien creado aun no tiene carpeta: se le hace, que el
    // usuario la ha pedido y abrir nada no le dice nada.
    if let Err(e) = std::fs::create_dir_all(&carpeta)
        .map_err(anyhow::Error::from)
        .and_then(|_| pixpin_shell::abrir(&carpeta).map_err(anyhow::Error::from))
    {
        tracing::warn!(?e, "no se pudo abrir la carpeta del proyecto");
        return Some(textos.t("ubicacion-error-abrir"));
    }
    None
}

/// «Proyecto nuevo en una carpeta…»: pide la carpeta y crea el proyecto
/// alli. `None` si se cancela el dialogo: no se crea nada.
///
/// Si moverlo falla, el proyecto se queda igual, en la zona habitual, que es
/// lo que pidio el usuario para cuando no se elige: se devuelve con el aviso.
pub(super) fn nuevo_en_carpeta(
    ventana: &VentanaOverlay,
    textos: &Catalogo,
    donde: &Ubicacion,
    identidad: &str,
) -> Option<(almacen::Ficha, Option<String>)> {
    let carpeta = pixpin_shell::elegir_carpeta::pedir_carpeta(
        ventana.handle(),
        &textos.t("ubicacion-elegir-titulo"),
    )?;
    let raiz = donde.raiz();
    let cuando = pixpin_shell::entorno::ahora_utc_ms();
    let mut ficha = almacen::Ficha::nueva("", cuando, identidad);
    // `leer_si_esta`: aqui se reescribe la lista entera, y una que no se pudo
    // leer no puede pasar por vacia.
    let mut indice = match almacen::Indice::leer_si_esta(raiz) {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!(?e, "no se pudo leer la lista para crear el proyecto");
            return None;
        }
    };
    indice.proyectos.push(ficha.clone());
    if let Err(e) = indice.guardar(raiz) {
        tracing::warn!(?e, "no se pudo crear el proyecto");
        return None;
    }
    let aviso = match ubicacion::mover_a(raiz, &ficha.id, &carpeta) {
        Ok(d) => {
            ficha.ubicacion = Some(d.display().to_string());
            None
        }
        Err(e) => {
            tracing::warn!(?e, "el proyecto nuevo se queda en la zona habitual");
            Some(texto_de_error(textos, &e))
        }
    };
    Some((ficha, aviso))
}
