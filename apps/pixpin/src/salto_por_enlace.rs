//! **Adonde lleva una zona vinculada** (`DrawEditorActivity.abrirSublienzo`
//! del movil).
//!
//! La marca de una zona mandada al chat guarda en `enlace` el id del dibujo de
//! su sublienzo (`foto-<id del mensaje>`). En el movil tocar su icono abre ese
//! dibujo tal cual (`ExcalidrawStore.rutaDe(dibujo)`), sin preguntar al chat;
//! aqui el lienzo lo abre quien lo llamo (`ventana_chat::abrir_hojas`) y hace
//! falta, ademas del dibujo, un mensaje con el que abrirlo.
//!
//! # Por que no basta con buscar el mensaje con esa referencia
//!
//! El 28-sep el usuario pulso una zona y el lienzo se cerro sin abrir el otro
//! (registro: «el enlace no lleva a ninguna hoja de este proyecto»). Su
//! proyecto habia entrado al movil con un `.pixpin` **antes** de que el movil
//! renombrara los enlaces al importar (`PaquetePixpin.importar`, «La marca de
//! una zona senala a su sublienzo por su id, y el id cambia aqui»): los
//! dibujos se llamaban ya `foto-<id>-<hora>`, y las marcas seguian diciendo
//! `foto-<id>`. Ese proyecto esta asi en disco y asi llega al PC.
//!
//! Se resuelve como resolveria el movil de hoy si lo importara otra vez, y sin
//! inventar nada: el dibujo tal cual; si no esta, el de la hoja que salio de
//! ese mensaje (`deMensaje`) o el del mensaje con ese id; y si no, el mismo
//! nombre con el sufijo de importacion (`-<hora>`, la del proyecto `pr-<hora>`
//! o, si solo hay uno, cualquiera). Un enlace que no lleva a ningun lienzo que
//! exista da `None`: el editor avisa y **no se cierra**.

use pixpin_proyecto::{almacen, cuaderno, Proyecto};
use std::path::Path;

/// La hoja que hay que abrir para seguir un enlace.
#[derive(Debug, Clone)]
pub(crate) enum HojaDelEnlace {
    /// Ya esta en la lista que tenia quien abrio el lienzo, en este sitio.
    EnLaLista(usize),
    /// No estaba (una zona recien mandada, una hoja que solo esta en
    /// `proyecto.json`): su mensaje, leido del disco.
    Leida(cuaderno::Mensaje),
}

/// **El dibujo al que lleva `enlace`** en el proyecto, si existe su lienzo.
pub(crate) fn dibujo_del_enlace(raiz: &Path, proyecto: &str, enlace: &str) -> Option<String> {
    let e = normalizar(enlace)?;
    let existe = |d: &str| almacen::lienzo(raiz, proyecto, d).is_file();
    if existe(&e) {
        return Some(e);
    }
    let carpeta = almacen::carpeta(raiz, proyecto);
    // El sublienzo de una zona: `foto-<id del mensaje>`. Su hoja lo dice en
    // `deMensaje` y su mensaje lo lleva de `id`, con el nombre que sea que
    // tenga hoy su dibujo.
    if let Some(id) = e.strip_prefix("foto-").filter(|id| !id.is_empty()) {
        let p = leer_proyecto(&carpeta);
        let por_hoja = p.hojas.iter().find_map(|h| {
            (h.resto.get("deMensaje").and_then(|v| v.as_str()) == Some(id))
                .then(|| h.dibujo.clone())
                .flatten()
        });
        if let Some(d) = por_hoja.filter(|d| existe(d)) {
            return Some(d);
        }
        let por_mensaje = cuaderno::Cuaderno::leer_de(&carpeta)
            .ok()
            .and_then(|c| c.mensajes.into_iter().find(|m| m.id == id))
            .and_then(|m| m.referencia);
        if let Some(d) = por_mensaje.filter(|d| existe(d)) {
            return Some(d);
        }
    }
    // El sufijo que pone el movil al importar: la hora, que es tambien la del
    // proyecto (`pr-<hora>`).
    if let Some(sufijo) = proyecto.strip_prefix("pr").filter(|s| es_sufijo_de_hora(s)) {
        let d = format!("{e}{sufijo}");
        if existe(&d) {
            return Some(d);
        }
    }
    // Con otra hora (el proyecto se renombro, o vino de otro): solo si no hay
    // duda de cual es.
    let mut candidatos = std::fs::read_dir(carpeta.join("lienzos"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|f| {
            let nombre = f.file_name().to_string_lossy().to_string();
            let tronco = nombre.strip_suffix(".excalidraw")?.to_string();
            let resto = tronco.strip_prefix(e.as_str())?;
            es_sufijo_de_hora(resto).then_some(tronco)
        });
    match (candidatos.next(), candidatos.next()) {
        (Some(unico), None) => Some(unico),
        _ => None,
    }
}

/// **La hoja que abre `enlace`**: en la lista de quien llama si esta, y si no
/// leida del cuaderno o de las hojas del proyecto. `None` si el enlace no
/// lleva a ningun lienzo que exista.
pub(crate) fn hoja_del_enlace(
    raiz: &Path,
    proyecto: &str,
    mensajes: &[cuaderno::Mensaje],
    enlace: &str,
) -> Option<HojaDelEnlace> {
    let dibujo = dibujo_del_enlace(raiz, proyecto, enlace)?;
    let es_el = |m: &cuaderno::Mensaje| m.referencia.as_deref() == Some(dibujo.as_str());
    if let Some(i) = mensajes.iter().position(es_el) {
        return Some(HojaDelEnlace::EnLaLista(i));
    }
    if let Some(m) = crate::zona_al_chat::mensaje_con_dibujo(raiz, proyecto, &dibujo) {
        return Some(HojaDelEnlace::Leida(m));
    }
    // Una hoja que solo esta en `proyecto.json` (la del movil que el chat no
    // ensena): con su pagina, que es su fondo.
    if let Some(m) = almacen::hojas_para_ensenar(raiz, proyecto, "").into_iter().find(es_el) {
        return Some(HojaDelEnlace::Leida(m));
    }
    // Y un dibujo suelto del proyecto: el movil lo abre igual, por su fichero.
    Some(HojaDelEnlace::Leida(cuaderno::Mensaje {
        nombre: format!("{dibujo}.excalidraw"),
        referencia: Some(dibujo),
        ..Default::default()
    }))
}

/// Cuantas hojas se abren seguidas, yendo y volviendo, antes de parar. Cada
/// una espera al usuario, asi que no es un bucle que corra solo: es el tope
/// por si algo devolviera enlaces sin fin.
pub(crate) const SALTOS_MAXIMOS: usize = 256;

/// **Abre una hoja y sigue sus enlaces como el movil.**
///
/// `abrir(lista, i)` abre la hoja `i` de `lista` en el lienzo y devuelve si
/// se guardo algo y el enlace que se pulso al salir, si se pulso uno. Es lo
/// que hace `ventana_chat::abrir_una_hoja`; aqui va como parametro para que el
/// camino se pruebe sin ventanas.
///
/// - Pulsar una zona vinculada abre su lienzo (`abrirSublienzo`).
/// - **Cerrar ese lienzo vuelve al de antes**: en el movil el sublienzo se
///   abre encima y «atras» lleva a la pagina de la que salio. Antes aqui se
///   volvia al chat.
/// - Un enlace que no lleva a nada no cierra nada: se vuelve a abrir la misma
///   hoja (el editor ya avisa y no se cierra; esto es por si el lienzo
///   desaparecio entre medias).
///
/// Devuelve los sitios de `mensajes` cuyas hojas se guardaron, una vez cada
/// uno, para que quien llama rehaga sus burbujas.
pub(crate) fn recorrer_hojas(
    raiz: &Path,
    proyecto: &str,
    mensajes: &[cuaderno::Mensaje],
    indice: usize,
    mut abrir: impl FnMut(&[cuaderno::Mensaje], usize) -> (bool, Option<String>),
) -> Vec<usize> {
    let mut guardadas = Vec::new();
    // Las hojas que no estaban en la lista (una zona recien mandada, una hoja
    // solo del proyecto) se anaden aqui, sin tocar la de quien llama.
    let mut todos = std::borrow::Cow::Borrowed(mensajes);
    // De donde se vino: cerrar un sublienzo vuelve a su pagina.
    let mut pila: Vec<usize> = Vec::new();
    let mut indice = indice;
    for _ in 0..SALTOS_MAXIMOS {
        let (guardada, enlace) = abrir(&todos, indice);
        if guardada && indice < mensajes.len() && !guardadas.contains(&indice) {
            guardadas.push(indice);
        }
        let Some(enlace) = enlace else {
            match pila.pop() {
                Some(anterior) => {
                    indice = anterior;
                    continue;
                }
                None => break,
            }
        };
        match hoja_del_enlace(raiz, proyecto, &todos, &enlace) {
            Some(HojaDelEnlace::EnLaLista(s)) => {
                pila.push(indice);
                indice = s;
            }
            Some(HojaDelEnlace::Leida(m)) => {
                todos.to_mut().push(m);
                pila.push(indice);
                indice = todos.len() - 1;
            }
            None => {
                tracing::warn!(%enlace, "el enlace no lleva a ninguna hoja de este proyecto; sigue la misma");
            }
        }
    }
    guardadas
}

/// El id de dibujo que hay dentro de un enlace: tal cual, o como ruta
/// (`lienzos/<id>.excalidraw`). Nada que pueda salir de la carpeta.
fn normalizar(enlace: &str) -> Option<String> {
    let e = enlace.trim();
    let e = e.strip_prefix("lienzos/").unwrap_or(e);
    let e = e.strip_suffix(".excalidraw").unwrap_or(e);
    if e.is_empty() || e.contains(['/', '\\', ':']) || e.contains("..") {
        return None;
    }
    Some(e.to_string())
}

/// `-<cifras>`: la hora que pone el movil detras de lo que importa.
fn es_sufijo_de_hora(s: &str) -> bool {
    s.strip_prefix('-')
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

fn leer_proyecto(carpeta: &Path) -> Proyecto {
    std::fs::read_to_string(carpeta.join("proyecto.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    fn raiz(etiqueta: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!("pixpin-salto-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    fn lienzo(raiz: &Path, proyecto: &str, dibujo: &str) {
        let r = almacen::lienzo(raiz, proyecto, dibujo);
        std::fs::create_dir_all(r.parent().unwrap()).unwrap();
        std::fs::write(r, r#"{"type":"excalidraw","elements":[]}"#).unwrap();
    }

    fn proyecto_json(raiz: &Path, proyecto: &str, p: serde_json::Value) {
        let c = almacen::carpeta(raiz, proyecto);
        std::fs::create_dir_all(&c).unwrap();
        std::fs::write(c.join("proyecto.json"), p.to_string()).unwrap();
    }

    fn mensaje(id: &str, referencia: &str) -> cuaderno::Mensaje {
        cuaderno::Mensaje {
            id: id.into(),
            nombre: format!("{referencia}.excalidraw"),
            referencia: Some(referencia.into()),
            ..Default::default()
        }
    }

    #[test]
    fn un_enlace_a_un_dibujo_que_existe_lleva_a_el_tal_cual() {
        let r = raiz("tal-cual");
        lienzo(&r, "p1", "foto-abc");
        assert_eq!(dibujo_del_enlace(&r, "p1", "foto-abc").as_deref(), Some("foto-abc"));
        // Tambien escrito como ruta.
        assert_eq!(dibujo_del_enlace(&r, "p1", "lienzos/foto-abc.excalidraw").as_deref(), Some("foto-abc"));
        // Caso negativo: nada fuera de la carpeta de lienzos.
        assert_eq!(dibujo_del_enlace(&r, "p1", "../p2/foto-abc"), None);
        assert_eq!(dibujo_del_enlace(&r, "p1", "  "), None);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn el_enlace_viejo_de_un_proyecto_importado_encuentra_su_sublienzo_renombrado() {
        // Lo del usuario: `pr-1789352007247`, dibujos con `-1789352007247`
        // detras y marcas que dicen `foto-e1da…` sin el.
        let r = raiz("importado");
        let p = "pr-1789352007247";
        lienzo(&r, p, "dib-1-1789352007247");
        lienzo(&r, p, "foto-e1da-1789352007247");
        assert_eq!(dibujo_del_enlace(&r, p, "foto-e1da").as_deref(), Some("foto-e1da-1789352007247"));
        // Caso negativo: un prefijo que no es el nombre entero no vale
        // (`foto-e1` no es `foto-e1da`).
        assert_eq!(dibujo_del_enlace(&r, p, "foto-e1"), None);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn por_la_hoja_que_salio_del_mensaje_aunque_el_dibujo_se_llame_de_otro_modo() {
        let r = raiz("de-mensaje");
        proyecto_json(
            &r,
            "p1",
            json!({"id": "p1", "hojas": [
                {"id": "h-1", "nombre": "Planta", "dibujo": "dib-1"},
                {"id": "h-2", "nombre": "↳ Zona", "dibujo": "zona-renombrada", "deMensaje": "e1da", "padre": "h-1"}
            ]}),
        );
        lienzo(&r, "p1", "zona-renombrada");
        assert_eq!(dibujo_del_enlace(&r, "p1", "foto-e1da").as_deref(), Some("zona-renombrada"));
        // Caso negativo: el mensaje de otra zona no se confunde.
        assert_eq!(dibujo_del_enlace(&r, "p1", "foto-otro"), None);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn por_el_id_del_mensaje_del_cuaderno() {
        let r = raiz("por-id");
        let carpeta = almacen::carpeta(&r, "p1");
        std::fs::create_dir_all(&carpeta).unwrap();
        cuaderno::anadir(&carpeta, &mensaje("e1da", "otra-cosa")).unwrap();
        lienzo(&r, "p1", "otra-cosa");
        assert_eq!(dibujo_del_enlace(&r, "p1", "foto-e1da").as_deref(), Some("otra-cosa"));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn con_dos_candidatos_de_otra_hora_no_se_elige_a_ciegas() {
        let r = raiz("dudoso");
        lienzo(&r, "p1", "foto-x-100");
        lienzo(&r, "p1", "foto-x-200");
        assert_eq!(dibujo_del_enlace(&r, "p1", "foto-x"), None, "caso negativo: dos posibles");
        // Con uno solo, si.
        let _ = std::fs::remove_file(almacen::lienzo(&r, "p1", "foto-x-200"));
        assert_eq!(dibujo_del_enlace(&r, "p1", "foto-x").as_deref(), Some("foto-x-100"));
        // Y lo que sigue al nombre tiene que ser una hora, no otra palabra.
        lienzo(&r, "p1", "foto-y-copia");
        assert_eq!(dibujo_del_enlace(&r, "p1", "foto-y"), None);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn un_enlace_a_un_lienzo_que_no_existe_no_lleva_a_ninguna_hoja() {
        let r = raiz("nada");
        std::fs::create_dir_all(almacen::carpeta(&r, "p1")).unwrap();
        // Ni con un mensaje que lo nombre: sin su fichero no hay que abrir.
        cuaderno::anadir(&almacen::carpeta(&r, "p1"), &mensaje("m1", "foto-fantasma")).unwrap();
        assert_eq!(dibujo_del_enlace(&r, "p1", "foto-fantasma"), None);
        assert!(hoja_del_enlace(&r, "p1", &[mensaje("m1", "foto-fantasma")], "foto-fantasma").is_none());
        // Ni en un proyecto que no existe.
        assert!(hoja_del_enlace(&r, "p-no", &[], "foto-fantasma").is_none());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn la_hoja_del_enlace_se_toma_de_la_lista_si_esta_y_si_no_del_disco() {
        let r = raiz("hoja");
        let p = "pr-1789352007247";
        let carpeta = almacen::carpeta(&r, p);
        std::fs::create_dir_all(&carpeta).unwrap();
        lienzo(&r, p, "foto-e1da-1789352007247");
        let lista = vec![mensaje("a", "dib-1"), mensaje("e1da", "foto-e1da-1789352007247")];
        // En la lista: su sitio.
        assert!(matches!(hoja_del_enlace(&r, p, &lista, "foto-e1da"), Some(HojaDelEnlace::EnLaLista(1))));
        // Fuera de la lista (abierto desde Proyectos, o recien mandada): del
        // cuaderno.
        cuaderno::anadir(&carpeta, &mensaje("e1da", "foto-e1da-1789352007247")).unwrap();
        match hoja_del_enlace(&r, p, &lista[..1], "foto-e1da") {
            Some(HojaDelEnlace::Leida(m)) => assert_eq!(m.id, "e1da"),
            otra => panic!("{otra:?}"),
        }
        let _ = std::fs::remove_dir_all(&r);
    }

    /// Lo que hace el usuario en cada lienzo que se abre: el enlace que pulsa
    /// al salir, o `None` si lo cierra. Apunta que referencia se abrio.
    fn guion<'a>(
        pasos: &'a [Option<&'a str>],
        abiertas: &'a mut Vec<String>,
    ) -> impl FnMut(&[cuaderno::Mensaje], usize) -> (bool, Option<String>) + 'a {
        let mut n = 0;
        move |lista, i| {
            abiertas.push(lista[i].referencia.clone().unwrap_or_default());
            let paso = pasos.get(n).copied().flatten().map(str::to_string);
            n += 1;
            (true, paso)
        }
    }

    #[test]
    fn pulsar_la_zona_abre_su_sublienzo_y_cerrarlo_vuelve_a_la_pagina() {
        // El camino entero del usuario: pagina → zona (enlace viejo, sin el
        // sufijo) → cerrar el sublienzo → la pagina otra vez → cerrar.
        let r = raiz("camino");
        let p = "pr-1789352007247";
        lienzo(&r, p, "dib-1-1789352007247");
        lienzo(&r, p, "foto-e1da-1789352007247");
        let carpeta = almacen::carpeta(&r, p);
        cuaderno::anadir(&carpeta, &mensaje("e1da", "foto-e1da-1789352007247")).unwrap();
        // La lista de quien abrio NO trae el sublienzo (abierto desde la
        // tarjeta de Proyectos, o la zona es de hoy).
        let lista = vec![mensaje("a", "dib-1-1789352007247")];
        let mut abiertas = Vec::new();
        let pasos = [Some("foto-e1da"), None, None];
        let guardadas = recorrer_hojas(&r, p, &lista, 0, guion(&pasos, &mut abiertas));
        assert_eq!(
            abiertas,
            ["dib-1-1789352007247", "foto-e1da-1789352007247", "dib-1-1789352007247"]
        );
        // Solo los sitios de la lista de quien llama, y una vez.
        assert_eq!(guardadas, [0]);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn un_enlace_que_no_lleva_a_nada_no_saca_del_lienzo() {
        let r = raiz("sin-destino");
        lienzo(&r, "p1", "dib-1");
        let lista = vec![mensaje("a", "dib-1")];
        let mut abiertas = Vec::new();
        // Pulsa una zona rota y luego cierra.
        let pasos = [Some("foto-borrada"), None];
        recorrer_hojas(&r, "p1", &lista, 0, guion(&pasos, &mut abiertas));
        assert_eq!(abiertas, ["dib-1", "dib-1"], "sigue en la misma hoja");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn de_zona_en_zona_y_atras_hasta_el_principio() {
        let r = raiz("cadena");
        for d in ["dib-1", "foto-a", "foto-b"] {
            lienzo(&r, "p1", d);
        }
        let lista = vec![mensaje("1", "dib-1"), mensaje("a", "foto-a"), mensaje("b", "foto-b")];
        let mut abiertas = Vec::new();
        let pasos = [Some("foto-a"), Some("foto-b"), None, None, None, Some("no-deberia")];
        recorrer_hojas(&r, "p1", &lista, 0, guion(&pasos, &mut abiertas));
        assert_eq!(abiertas, ["dib-1", "foto-a", "foto-b", "foto-a", "dib-1"]);
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn una_hoja_que_solo_esta_en_el_proyecto_se_abre_con_su_pagina() {
        let r = raiz("solo-proyecto");
        proyecto_json(
            &r,
            "p1",
            json!({"id": "p1", "hojas": [{"id": "h-3", "nombre": "Pagina 3", "dibujo": "dib-p3", "pagina": 2, "uid": "U3"}]}),
        );
        lienzo(&r, "p1", "dib-p3");
        match hoja_del_enlace(&r, "p1", &[], "dib-p3") {
            Some(HojaDelEnlace::Leida(m)) => {
                assert_eq!(m.referencia.as_deref(), Some("dib-p3"));
                assert_eq!(m.pagina, Some(2), "su pagina es su fondo");
            }
            otra => panic!("{otra:?}"),
        }
        // Un dibujo suelto, en ninguna hoja ni mensaje: se abre por su fichero.
        lienzo(&r, "p1", "suelto");
        match hoja_del_enlace(&r, "p1", &[], "suelto") {
            Some(HojaDelEnlace::Leida(m)) => assert_eq!(m.referencia.as_deref(), Some("suelto")),
            otra => panic!("{otra:?}"),
        }
        let _ = std::fs::remove_dir_all(&r);
    }
}
