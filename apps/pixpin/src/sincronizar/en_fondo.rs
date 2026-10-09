//! **Sincronizar sin abrir la ventana**, pedido desde el plugin de Flow
//! Launcher (`p s`, pedido `sincronizar`).
//!
//! El usuario, 7-oct-2026: «dentro del plugin que haya la opcion de
//! sincronizar … me aparezca el listado de los celulares y su estado de
//! conexion y si le doy a uno se sincronice solo con ese, si le doy a todos
//! con todos». Lo de la red es lo mismo que el boton «Sincronizar con todos»
//! de la ventana ([`super::lanzar_con_todos`]): sin preguntar, con lo elegido
//! la ultima vez con cada aparato. Aqui solo cambia quien escucha lo que
//! cuenta: en vez de la ventana, un hilo que al acabar lo dice en el globo.
//!
//! Y lo que el plugin ensena como «conectado» sale de aqui tambien:
//! [`apuntar_estado`] deja en `sincro/estado.json` lo que dijo la sonda de
//! fondo, porque el plugin arranca una vez por tecla y no puede salir a la
//! red a preguntar.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use pixpin_store::Ubicacion;
use serde::{Deserialize, Serialize};

use super::{Aviso, Fase};

/// Una sola sincronizacion de fondo a la vez: dos vueltas con el mismo
/// aparato a la par se pisarian los ficheros.
static EN_MARCHA: AtomicBool = AtomicBool::new(false);

/// Lo que se pide desde el plugin: `todos` o el `id` de un aparato del grupo.
pub const TODOS: &str = "todos";

/// `sincro/estado.json`: si cada aparato contesto a la ultima sonda.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Estado {
    /// Cuando paso la sonda (ms desde 1970).
    pub cuando: i64,
    /// `id` → si contesto.
    pub responden: BTreeMap<String, bool>,
}

/// Lo apunta la sonda de fondo tras cada pasada.
pub(super) fn apuntar_estado(raiz: &Path, responden: BTreeMap<String, bool>) {
    let estado = Estado {
        cuando: super::ahora_ms(),
        responden,
    };
    let Ok(texto) = serde_json::to_string(&estado) else {
        return;
    };
    let carpeta = super::carpeta_sincro(raiz);
    let _ = std::fs::create_dir_all(&carpeta);
    // Con `.tmp` y renombrar: el plugin puede estar leyendolo justo ahora.
    let tmp = carpeta.join("estado.json.tmp");
    if std::fs::write(&tmp, texto).is_ok() {
        let _ = std::fs::rename(&tmp, carpeta.join("estado.json"));
    }
}

/// A quien se llama: `(nombre, host, puerto)` como lo espera
/// `lanzar_con_todos`. Sin direccion apuntada va vacia: `llegar` lo busca
/// por el anuncio de la red antes de rendirse. `None`: no hay grupo, o ese
/// aparato no es del grupo.
fn lista(raiz: &Path, aparato: &str) -> Option<Vec<(String, String, u16)>> {
    lista_de(raiz, |id| aparato == TODOS || id == aparato)
}

/// Como [`lista`], con los aparatos cuyo `id` cumple `quiero`.
fn lista_de(raiz: &Path, quiero: impl Fn(&str) -> bool) -> Option<Vec<(String, String, u16)>> {
    let id = super::leer_identidad(raiz).ok()?;
    if id.codigo.as_deref().is_none_or(|c| c.trim().is_empty()) {
        return None;
    }
    let dirs = super::leer_direcciones(raiz);
    let v: Vec<(String, String, u16)> = id
        .miembros
        .iter()
        .filter(|m| m.id != id.yo.id && !m.id.is_empty())
        .filter(|m| quiero(&m.id))
        .map(|m| {
            let (h, p) = dirs
                .iter()
                .find(|(i, _, _)| *i == m.id)
                .map(|(_, h, p)| (h.clone(), *p))
                .unwrap_or((String::new(), pixpin_sincro::PUERTO));
            (m.nombre.clone(), h, p)
        })
        .collect();
    (!v.is_empty()).then_some(v)
}

/// Sincroniza con `aparato` (o con todos) en un hilo y lo dice al acabar.
pub fn lanzar(ubicacion: &Ubicacion, aparato: &str) {
    let raiz = ubicacion.raiz().to_path_buf();
    let Some(lista) = lista(&raiz, aparato) else {
        super::al_movil::avisar(if aparato == TODOS {
            "Este PC no tiene a nadie más en el grupo: únelo en Sincronizar".into()
        } else {
            "Ese aparato ya no está en el grupo".into()
        });
        return;
    };
    let quien = if lista.len() == 1 {
        lista[0].0.clone()
    } else {
        format!("{} aparatos", lista.len())
    };
    let arrancada = arrancar(&raiz, lista, |fin| {
        if let Some(texto) = fin.and_then(|f| contar(&f)) {
            super::al_movil::avisar(texto);
        }
    });
    super::al_movil::avisar(if arrancada {
        format!("Sincronizando con {quien}…")
    } else {
        "Ya se está sincronizando: espera a que acabe".into()
    });
}

/// **La de la sincronizacion automatica**: con los aparatos `ids` (los que
/// contestan), sin decir nada al empezar. `al_acabar` recibe como acabo.
/// `false` si ya habia una en marcha o ninguno de `ids` es del grupo.
pub(super) fn lanzar_callada(
    raiz: &Path,
    ids: &[String],
    al_acabar: impl FnOnce(Option<Fase>) + Send + 'static,
) -> bool {
    let Some(lista) = lista_de(raiz, |id| ids.iter().any(|x| x == id)) else {
        return false;
    };
    arrancar(raiz, lista, al_acabar)
}

/// Pone en marcha la vuelta con `lista` en un hilo. `false` si ya habia una
/// de fondo en marcha: dos vueltas con el mismo aparato a la par se pisarian.
fn arrancar(
    raiz: &Path,
    lista: Vec<(String, String, u16)>,
    al_acabar: impl FnOnce(Option<Fase>) + Send + 'static,
) -> bool {
    if EN_MARCHA.swap(true, Ordering::SeqCst) {
        return false;
    }
    let (tx, rx) = mpsc::channel();
    super::lanzar_con_todos(raiz, &tx, lista, super::presencia::puerto());
    // El que escucha suelta su `tx`: si la vuelta no arranca, `recv` acaba.
    drop(tx);
    let lanzado = std::thread::Builder::new()
        .name("sincro-fondo".into())
        .spawn(move || {
            let mut fin = None;
            while let Ok(a) = rx.recv() {
                match a {
                    Aviso::Identidad => {
                        super::presencia::difundir(super::presencia::Novedad::Identidad)
                    }
                    Aviso::Fase(f @ (Fase::Terminado { .. } | Fase::Fallo(_) | Fase::Nada)) => {
                        fin = Some(f);
                        break;
                    }
                    _ => {}
                }
            }
            EN_MARCHA.store(false, Ordering::SeqCst);
            al_acabar(fin);
        });
    if let Err(e) = lanzado {
        EN_MARCHA.store(false, Ordering::SeqCst);
        tracing::warn!(?e, "no se pudo lanzar el hilo de sincronizar de fondo");
        return false;
    }
    true
}

/// Lo que se dice en el globo: el titulo y lo hecho; los avisos, solo el
/// primero (el globo no da para mas; el resto esta en la ventana).
pub(super) fn contar(f: &Fase) -> Option<String> {
    match f {
        Fase::Terminado {
            titulo,
            texto,
            aviso,
            ..
        } => {
            let mut t = titulo.clone();
            if let Some(l) = texto.lines().find(|l| !l.trim().is_empty()) {
                t.push('\n');
                t.push_str(l.trim());
            }
            if let Some(a) = aviso
                .as_deref()
                .and_then(|a| a.split("\n\n").next())
                .filter(|a| !a.trim().is_empty())
            {
                t.push('\n');
                t.push_str(a.trim());
            }
            Some(t)
        }
        Fase::Fallo(e) => Some(format!("No se pudo sincronizar: {e}")),
        _ => None,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::identidad::{Aparato, Identidad};
    use std::path::PathBuf;

    fn raiz(nombre: &str) -> PathBuf {
        let r =
            std::env::temp_dir().join(format!("pixpin-en-fondo-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(r.join("sincro")).unwrap();
        r
    }

    fn aparato(id: &str, nombre: &str) -> Aparato {
        Aparato {
            id: id.into(),
            nombre: nombre.into(),
            letra: None,
            ..Default::default()
        }
    }

    fn con_grupo(r: &Path) {
        let id = Identidad {
            yo: aparato("pc", "PC"),
            codigo: Some("ABCD2345".into()),
            miembros: vec![
                aparato("pc", "PC"),
                aparato("tel", "Redmi"),
                aparato("tab", "Tablet"),
            ],
            ..Default::default()
        };
        std::fs::write(
            r.join("sincro").join("identidad.json"),
            serde_json::to_string(&id).unwrap(),
        )
        .unwrap();
        std::fs::write(
            r.join("sincro").join("direcciones.txt"),
            "tel\t192.168.1.5\t47474\n",
        )
        .unwrap();
    }

    #[test]
    fn todos_son_los_del_grupo_menos_este_pc() {
        let r = raiz("todos");
        con_grupo(&r);
        let l = lista(&r, TODOS).unwrap();
        assert_eq!(l.len(), 2);
        assert!(l.contains(&("Redmi".into(), "192.168.1.5".into(), 47474)));
        // Sin direccion apuntada va vacia: `llegar` la busca por la red.
        assert!(l.contains(&("Tablet".into(), String::new(), pixpin_sincro::PUERTO)));
    }

    #[test]
    fn uno_solo_es_ese_y_nada_mas() {
        let r = raiz("uno");
        con_grupo(&r);
        let l = lista(&r, "tel").unwrap();
        assert_eq!(l, vec![("Redmi".into(), "192.168.1.5".into(), 47474)]);
    }

    #[test]
    fn caso_negativo_sin_grupo_o_un_desconocido_no_hay_a_quien_llamar() {
        let r = raiz("sin");
        assert_eq!(lista(&r, TODOS), None);
        con_grupo(&r);
        assert_eq!(lista(&r, "otro"), None);
        // Este PC no se sincroniza consigo mismo.
        assert_eq!(lista(&r, "pc"), None);
    }

    #[test]
    fn el_estado_se_apunta_y_se_vuelve_a_leer() {
        let r = raiz("estado");
        let mut m = BTreeMap::new();
        m.insert("tel".to_string(), true);
        m.insert("tab".to_string(), false);
        apuntar_estado(&r, m.clone());
        let leido: Estado = serde_json::from_str(
            &std::fs::read_to_string(r.join("sincro").join("estado.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(leido.responden, m);
        assert!(leido.cuando > 0);
    }

    #[test]
    fn el_globo_dice_el_titulo_lo_hecho_y_el_primer_aviso() {
        let f = Fase::Terminado {
            trajo: true,
            titulo: "Al día con Redmi".into(),
            texto: "3 mensajes nuevos\n1,2 MB en 2 s".into(),
            aviso: Some("Un archivo no llegó\n\nOtro aviso".into()),
        };
        assert_eq!(
            contar(&f).unwrap(),
            "Al día con Redmi\n3 mensajes nuevos\nUn archivo no llegó"
        );
        assert_eq!(
            contar(&Fase::Fallo("No contesta".into())).unwrap(),
            "No se pudo sincronizar: No contesta"
        );
        assert_eq!(contar(&Fase::Nada), None);
    }
}
