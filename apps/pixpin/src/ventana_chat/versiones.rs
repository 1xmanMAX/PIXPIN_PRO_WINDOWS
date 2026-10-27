//! **Las versiones vinculadas, en la ventana del chat** (27-oct-2026): lo de
//! pantalla de `pixpin_proyecto::versiones`, que es donde esta la regla (y
//! `guardados/VersionesEnElChat.kt` del movil). Aqui: abrir lo de otra
//! persona como version mia, hacer la de un toque, cerrar los borradores al
//! volver de un editor, y los dos rotulos de la burbuja —«Version de Ana»
//! encima de una version, que lleva al original, y «2 versiones» en el
//! original, que las lista—.
//!
//! Solo en un chat compartido (`compartidos_ui::del_chat`) y solo con lo de
//! otra persona (`conversacion::Lados::quien` → `Otro`): lo mio se sigue
//! editando en su sitio, y en un chat propio todo es como siempre.

use std::collections::HashMap;
use std::sync::Mutex;

use pixpin_proyecto::cuaderno::{Clase, Mensaje};
use pixpin_proyecto::versiones::{self as vv, DeQuien};
use pixpin_store::{Catalogo, Ubicacion};

use super::{Abierto, OpcionesLienzo};

/// Lo que hay que decir en el aviso del chat, desde donde no se tiene a
/// mano (`tocar_la_burbuja`, el hilo de un editor): lo recoge el bucle.
/// Ya en palabras, o por su clave si se pide sin catalogo a mano.
static AVISO: Mutex<Option<Result<String, &'static str>>> = Mutex::new(None);

fn avisar(texto: String) {
    if let Ok(mut g) = AVISO.lock() {
        *g = Some(Ok(texto));
    }
}

fn avisar_clave(clave: &'static str) {
    if let Ok(mut g) = AVISO.lock() {
        *g = Some(Err(clave));
    }
}

/// El aviso pendiente, si hay (una vez).
pub(super) fn tomar_aviso(textos: &Catalogo) -> Option<String> {
    let a = AVISO.lock().ok().and_then(|mut g| g.take())?;
    Some(a.unwrap_or_else(|clave| textos.t(clave)))
}

/// **El nombre de la otra persona si `m` es suyo** en este chat
/// compartido; `None` si es mio o el chat no es compartido. El asistente no
/// es una persona de la lista: lo suyo es mio, como en la burbuja.
pub(crate) fn de_quien(raiz: &std::path::Path, ficha: &str, m: &Mensaje) -> Option<String> {
    if crate::asistente::chat::es_del_asistente(m) {
        return None;
    }
    let cc = crate::sincronizar::compartidos_ui::del_chat(raiz, ficha)?;
    cc.lados
        .quien(vv::aparato_corto(m).as_deref())
        .nombre()
        .map(str::to_string)
}

/// El codigo de este equipo, el que lleva lo que nace aqui.
fn mi_aparato(raiz: &std::path::Path) -> Option<String> {
    pixpin_proyecto::identidad::Identidad::leer_o_crear(raiz, "PC")
        .ok()
        .map(|i| i.yo.codigo())
        .filter(|c| !c.is_empty())
}

/// **Al volver al chat**: los borradores cambiados pasan a ser versiones
/// (`versiones::cerrar`). Devuelve si hubo alguna, y entonces lo dice.
pub(super) fn cerrar(raiz: &std::path::Path, textos: &Catalogo) -> bool {
    if !vv::hay_en_curso(raiz) {
        return false;
    }
    match vv::cerrar(raiz, pixpin_shell::entorno::ahora_utc_ms()) {
        Ok(hechas) if !hechas.is_empty() => {
            tracing::info!(cuantas = hechas.len(), "versiones vinculadas guardadas");
            avisar(textos.t("chat-version-guardada"));
            true
        }
        Ok(_) => false,
        Err(e) => {
            tracing::warn!(?e, "no se pudieron cerrar los borradores de versiones");
            false
        }
    }
}

/// **Abrir lo de otra persona como version mia**: si el mensaje `indice` es
/// ajeno y se edita en un borrador (un lienzo, una pagina, una foto, un
/// documento), se prepara la copia y el editor se abre sobre ella; lo que
/// se cambie queda como «Version de …» debajo del original. Devuelve si se
/// encargo; si no, se abre como siempre.
///
/// Las hojas de calculo no: su pin no toca nunca el archivo (lo editado
/// sale como una tabla nueva, que ya es mia), asi que no hay nada que
/// proteger.
pub(super) fn abrir_como_version(
    ubicacion: &Ubicacion,
    a: &Abierto,
    indice: usize,
    textos: &Catalogo,
    lienzo: OpcionesLienzo,
    idioma: pixpin_store::Idioma,
    identidad: &str,
) -> bool {
    let Some(m) = a.mensajes.get(indice) else {
        return false;
    };
    if vv::como(m) != Some(vv::Como::Borrador) || crate::hoja_pin::que_abrir(m).is_some() {
        return false;
    }
    let Some(quien) = de_quien(&a.raiz, &a.ficha.id, m) else {
        return false;
    };
    let raiz = ubicacion.raiz().to_path_buf();
    // Lo que se cambio antes y aun no era version, primero (como el movil).
    cerrar(&raiz, textos);
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let aparato = Some(identidad).filter(|c| !c.is_empty());
    let e = match vv::empezar(
        &raiz,
        &a.ficha.id,
        m,
        &a.mensajes,
        aparato,
        ahora,
        &vv::id_nuevo(ahora),
        &pixpin_proyecto::codigos::nuevo(),
    ) {
        Ok(e) => e,
        Err(err) => {
            tracing::warn!(?err, "no se pudo preparar la version");
            avisar(textos.t("chat-version-no"));
            return true;
        }
    };
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("quien", quien);
    avisar(textos.t_args("chat-version-es-de", &args));
    tracing::info!(original = %m.id, version = %e.version.id, "se abre como version mia");
    let proyecto = a.ficha.id.clone();
    // El editor trabaja sobre el chat con la copia al final: es la hoja que
    // abre, y la que se busca por su id.
    let mut mensajes = a.mensajes.clone();
    mensajes.push(e.version.clone());
    let j = mensajes.len() - 1;
    match m.clase {
        Some(Clase::Dibujo) | Some(Clase::Pagina) => {
            super::lienzo_aparte(super::clave_de_hoja(&proyecto, &mensajes, j), move || {
                super::abrir_hojas(&raiz, &proyecto, &mensajes, j, lienzo);
            });
        }
        Some(Clase::Imagen) => {
            // Su lienzo se estrena con el nombre de la copia (`foto-<id>`);
            // la foto que pone de fondo no cuenta como cambio.
            crate::foto_anotada::asegurar_lienzo(&raiz, &proyecto, &mut mensajes, j);
            let plana = super::ruta_del_mensaje(&raiz, &proyecto, &mensajes[j]);
            super::lienzo_aparte(super::clave_de_hoja(&proyecto, &mensajes, j), move || {
                if !crate::foto_anotada::abrir_si_tiene_lienzo(
                    &raiz, &proyecto, &mensajes, j, lienzo,
                ) && let Some(ruta) = plana
                {
                    super::abrir_foto_en_lienzo(&ruta, lienzo);
                }
            });
        }
        _ => super::abrir_el_mensaje(ubicacion, &proyecto, &e.version, idioma),
    }
    true
}

/// **Un cambio de un toque sobre lo de otra persona** (la descripcion de un
/// archivo, el documento de una mini-app): se hace en una version mia, al
/// final del chat, y no en el suyo. Devuelve el indice de la version; `None`
/// si no se encargo (no es ajeno, o no se versiona), y entonces quien llama
/// cambia el mensaje en su sitio, como siempre.
pub(super) fn al_momento(
    ubicacion: &Ubicacion,
    a: &mut Abierto,
    indice: usize,
    cambio: impl FnOnce(Mensaje) -> Mensaje,
) -> Option<std::io::Result<usize>> {
    let m = a.mensajes.get(indice)?;
    vv::como(m)?;
    de_quien(&a.raiz, &a.ficha.id, m)?;
    let raiz = ubicacion.raiz();
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let v = vv::al_momento(
        raiz,
        &a.ficha.id,
        m,
        &a.mensajes,
        mi_aparato(raiz).as_deref(),
        ahora,
        cambio,
        &vv::id_nuevo(ahora),
        &pixpin_proyecto::codigos::nuevo(),
    );
    let v = vv::con_numero(&a.mensajes, v);
    let carpeta = pixpin_proyecto::almacen::carpeta(raiz, &a.ficha.id);
    if let Err(e) = pixpin_proyecto::cuaderno::anadir(&carpeta, &v) {
        return Some(Err(e));
    }
    tracing::info!(original = %a.mensajes[indice].id, version = %v.id, "version de un toque");
    // La mas nueva: al final, que es donde la deja releer (por la hora).
    a.vistas.push(super::leer_vista(ubicacion, &a.ficha.id, &v));
    a.mensajes.push(v);
    a.colocado.borrow_mut().ancho = 0;
    Some(Ok(a.mensajes.len() - 1))
}

/// Dice como fue una version de un toque (lo recoge el bucle).
pub(super) fn avisar_de_un_toque(hecho: &std::io::Result<usize>) {
    match hecho {
        Ok(_) => avisar_clave("chat-version-guardada"),
        Err(e) => {
            tracing::warn!(?e, "no se pudo guardar la version");
            avisar_clave("chat-version-no");
        }
    }
}

// ------------------------------------------------------------------ los rotulos

/// Los rotulos de un chat, ya en palabras: el de encima de cada version
/// (con el indice de su original, adonde lleva) y el chip de cada original
/// con la lista de sus versiones (texto e indice de cada una).
#[derive(Debug, Default)]
pub(super) struct Rotulos {
    pub cabecera: HashMap<String, (String, Option<usize>)>,
    pub chip: HashMap<String, (String, Vec<(String, usize)>)>,
}

impl Rotulos {
    pub(super) fn de(&self, m: &Mensaje) -> (Option<&(String, Option<usize>)>, Option<&str>) {
        (
            self.cabecera.get(&m.id),
            self.chip.get(&m.id).map(|(t, _)| t.as_str()),
        )
    }
}

/// **Que rotulos lleva cada burbuja** del chat abierto. Sin ninguna version
/// en el chat, nada (y sin mirar de quien es nada).
pub(super) fn rotulos(a: &Abierto, textos: &Catalogo) -> Rotulos {
    if a.mensajes.iter().all(|m| m.version_de.is_none()) {
        return Rotulos::default();
    }
    let cc = crate::sincronizar::compartidos_ui::del_chat(&a.raiz, &a.ficha.id);
    let autor = |m: &Mensaje| -> Option<String> {
        if crate::asistente::chat::es_del_asistente(m) {
            return None;
        }
        cc.as_ref()?
            .lados
            .quien(vv::aparato_corto(m).as_deref())
            .nombre()
            .map(str::to_string)
    };
    let r = vv::rotulos(&a.mensajes, cc.is_some(), autor);
    let indice = |id: &str| a.mensajes.iter().position(|m| m.id == id);
    let mut salida = Rotulos::default();
    for (id, c) in &r.cabecera {
        let ir = c.original.as_deref().and_then(indice);
        salida
            .cabecera
            .insert(id.clone(), (texto_de_cabecera(c, textos), ir));
    }
    for (id, lista) in &r.versiones {
        let items: Vec<(String, usize)> = lista
            .iter()
            .filter_map(|e| Some((texto_en_la_lista(e, textos), indice(&e.id)?)))
            .collect();
        if items.is_empty() {
            continue;
        }
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("cuantas", items.len() as i64);
        salida
            .chip
            .insert(id.clone(), (textos.t_args("chat-versiones", &args), items));
    }
    salida
}

/// «Version de Ana», «Version 2 de Ana», «Version de lo tuyo»…
fn texto_de_cabecera(c: &vv::Cabecera, textos: &Catalogo) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    if let Some(n) = c.n {
        args.set("n", n);
    }
    let con_n = |clave: &str| {
        if c.n.is_some() {
            format!("{clave}-n")
        } else {
            clave.to_string()
        }
    };
    let clave = match &c.de {
        DeQuien::De(quien) => {
            args.set("quien", quien.clone());
            con_n("chat-version-de")
        }
        DeQuien::Tuyo => con_n("chat-version-de-lo-tuyo"),
        DeQuien::OtroMensaje => con_n("chat-version-de-otro"),
        DeQuien::SinNombrar => con_n("chat-version-sola"),
    };
    textos.t_args(&clave, &args)
}

/// «Version 1 · tuya», «Version 2 · Ana».
fn texto_en_la_lista(e: &vv::EnLaLista, textos: &Catalogo) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("n", e.n);
    match &e.quien {
        Some(Some(quien)) => {
            args.set("quien", quien.clone());
            textos.t_args("chat-version-de-la-lista", &args)
        }
        Some(None) => textos.t_args("chat-version-tuya", &args),
        None => textos.t_args("chat-version-sola-n", &args),
    }
}
