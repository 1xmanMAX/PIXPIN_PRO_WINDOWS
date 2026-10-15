//! **Una herramienta del asistente pedida desde fuera** (pedido
//! `herramienta`, 15-oct-2026): lo que usa el servidor MCP
//! (`apps/pixpin-mcp`) para que Claude Desktop o Claude Code hagan cosas en
//! PixPin con las mismas herramientas que el asistente.
//!
//! El pedido trae el nombre de una fila de `pixpin_ia::acciones::todas()` y
//! sus argumentos. Aqui se valida con la misma tabla y se hace **por el
//! mismo camino que el asistente**: las de la app con
//! [`super::acciones::hacer`] (tareas por su nombre, pedidos a esta misma
//! PixPin, pregunta antes de lo que borra o envia) y las de documentos con
//! [`super::documentos_acciones::hacer`] (sin adjuntos: el archivo se nombra
//! o se da su ruta). La frase de «hecho» o el motivo del fallo se escribe en
//! el fichero `respuesta`, que es lo unico que el que pidio puede leer.
//!
//! - **Permiso**: lo que la fila manda preguntar y, ademas, la pregunta que
//!   trae el pedido (`preguntar`: el servidor MCP pregunta tambien antes de
//!   cambiar lo que ya existe) se preguntan en el chat «Asistente» y en el
//!   aviso flotante, con «Sí»/«No», como las automatizaciones
//!   ([`super::permiso::preguntar_en_el_chat`]). Sin respuesta, es un no.
//! - **El fichero de respuesta** solo puede estar en `%TEMP%\pixpin-mcp\`, ser
//!   un `.json` y no existir: un pedido no puede hacer que PixPin escriba
//!   donde quiera ni encima de nada. Se escribe al lado y se renombra, para
//!   que el otro lado nunca lea uno a medias.
//! - `hoja_a_lienzo` no: dibuja en el lienzo abierto del chat del asistente.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use pixpin_ia::acciones;
use serde_json::{Map, Value, json};

use super::acciones::{Confirmar, Mandar};
use crate::automatizaciones::encargo::Entorno;

/// La carpeta de las respuestas, dentro de `%TEMP%` (la misma que
/// `pixpin_mcp::escribir::CARPETA`).
pub const CARPETA: &str = "pixpin-mcp";

/// Lo que trae el pedido.
#[derive(Debug, Clone, Default)]
pub struct Encargo {
    pub nombre: String,
    /// Los argumentos, en texto JSON (un objeto).
    pub args: Option<String>,
    pub respuesta: Option<PathBuf>,
    pub preguntar: Option<String>,
    /// Quien lo pide («Claude»), para el aviso y la pregunta.
    pub origen: Option<String>,
}

/// Si PixPin puede escribir la respuesta ahi: dentro de `%TEMP%\pixpin-mcp\`
/// (sin subcarpetas ni `..`), un `.json` que aun no existe.
pub fn respuesta_valida(r: &Path, temp: &Path) -> bool {
    let carpeta = temp.join(CARPETA);
    let misma = |a: &Path, b: &Path| a.to_string_lossy().trim_end_matches(['\\', '/']).eq_ignore_ascii_case(b.to_string_lossy().trim_end_matches(['\\', '/']));
    r.is_absolute()
        && r.parent().is_some_and(|p| misma(p, &carpeta))
        && r.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"))
        && r.file_name().is_some_and(|n| !n.to_string_lossy().starts_with('.'))
        && !r.components().any(|c| matches!(c, std::path::Component::ParentDir))
        && !r.exists()
}

/// Escribe la respuesta de una vez (al lado y renombrando).
pub fn escribir_respuesta(fichero: &Path, r: &Result<String, String>) -> std::io::Result<()> {
    let v = match r {
        Ok(t) => json!({"ok": true, "texto": t}),
        Err(e) => json!({"ok": false, "texto": e}),
    };
    let parte = fichero.with_extension("parte");
    std::fs::write(&parte, v.to_string())?;
    std::fs::rename(&parte, fichero)
}

/// **Hace la herramienta** con quien pregunta y quien manda los pedidos
/// dados (las pruebas no tocan la PixPin del usuario).
pub fn hacer_con(
    e: &Encargo,
    raiz: &Path,
    idioma: pixpin_store::Idioma,
    confirmar: &Confirmar,
    mandar: &Mandar,
) -> Result<String, String> {
    let fila = acciones::por_nombre(e.nombre.trim()).ok_or_else(|| format!("no hay ninguna herramienta «{}»", e.nombre))?;
    if fila.herramienta == pixpin_ia::hoja::NOMBRE {
        return Err("«hoja_a_lienzo» solo se puede pedir desde el chat del asistente".into());
    }
    let args: Map<String, Value> = match e.args.as_deref().map(serde_json::from_str::<Value>) {
        None => Map::new(),
        Some(Ok(Value::Object(m))) => m,
        Some(Ok(Value::Null)) => Map::new(),
        _ => return Err("«args» tiene que ser un objeto JSON".into()),
    };
    let a = acciones::validar(fila, &args).map_err(|e| e.to_string())?;
    if let Some(q) = e.preguntar.as_deref().map(str::trim).filter(|q| !q.is_empty())
        && !confirmar(q)
    {
        return Err("el usuario dijo que no; no se hizo nada".into());
    }
    if pixpin_ia::acciones_documentos::es_de_documentos(a.herramienta) {
        let parar = AtomicBool::new(false);
        return super::documentos_acciones::hacer(
            &a,
            &super::documentos_acciones::Contexto {
                raiz,
                proyecto: "",
                adjuntos: &[],
                mandar,
                parar: &parar,
                idioma,
            },
        );
    }
    super::acciones::hacer(&a, Some(raiz), confirmar, mandar)
}

/// Si hara falta preguntar (para sacar el aviso flotante donde se ve la
/// pregunta aunque el chat este cerrado).
fn preguntara(e: &Encargo) -> bool {
    e.preguntar.as_deref().is_some_and(|q| !q.trim().is_empty())
        || acciones::por_nombre(e.nombre.trim()).is_some_and(|f| f.confirmar.is_some())
}

/// **Lo hace en su hilo** y escribe la respuesta. Una `respuesta` que no
/// vale se ignora (se hace igual y queda en el registro).
pub fn lanzar(entorno: Entorno, idioma: pixpin_store::Idioma, opciones: crate::ventana_chat::OpcionesLienzo, e: Encargo) {
    let temp = std::env::temp_dir();
    let respuesta = e.respuesta.clone().filter(|r| {
        let bien = respuesta_valida(r, &temp);
        if !bien {
            tracing::warn!(ruta = %r.display(), "herramienta: respuesta fuera de %TEMP%\\pixpin-mcp, no se escribe");
        }
        bien
    });
    let lanzado = std::thread::Builder::new().name("herramienta-externa".into()).spawn(move || {
        let _com = pixpin_shell::ComDelHilo::iniciar();
        let de = e.origen.clone().filter(|o| !o.trim().is_empty()).unwrap_or_else(|| "Otro programa".into());
        // Con pregunta, la tarjeta del aviso flotante la ensena aunque el
        // chat este cerrado (como las automatizaciones).
        let mando = preguntara(&e).then(|| {
            crate::ventana_chat::sin_ventana::avisar(crate::ventana_chat::sin_ventana::Aviso {
                titulo: format!("🤖 {de}"),
                texto: "⚙️ …".into(),
                abrir: None,
                acabado: false,
                chat: Some(crate::ventana_chat::sin_ventana::Chat {
                    idioma,
                    ubicacion: entorno.ubicacion.clone(),
                    opciones,
                }),
            })
        });
        let ent = entorno.clone();
        let quien = de.clone();
        let confirmar: Confirmar = Box::new(move |p: &str| {
            super::permiso::preguntar_en_el_chat(&ent.ubicacion, &ent.aparato, &quien, p)
        });
        let r = hacer_con(&e, entorno.ubicacion.raiz(), idioma, &confirmar, &super::acciones::mandar_a_pixpin());
        tracing::info!(herramienta = %e.nombre, %de, bien = r.is_ok(), "herramienta pedida desde fuera");
        if let Some(m) = mando {
            m.poner(
                match &r {
                    Ok(t) => t.clone(),
                    Err(e) => format!("⚠️ {e}"),
                },
                None,
            );
        }
        if let Some(f) = respuesta
            && let Err(err) = escribir_respuesta(&f, &r)
        {
            tracing::warn!(%err, ruta = %f.display(), "herramienta: no se pudo escribir la respuesta");
        }
    });
    if let Err(err) = lanzado {
        tracing::warn!(%err, "herramienta: no se pudo lanzar su hilo");
    }
}

#[cfg(test)]
mod pruebas {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[test]
    fn la_respuesta_solo_va_a_su_carpeta_y_nunca_encima() {
        let temp = std::env::temp_dir().join(format!("pixpin-externa-{}", std::process::id()));
        let carpeta = temp.join(CARPETA);
        std::fs::create_dir_all(&carpeta).unwrap();
        assert!(respuesta_valida(&carpeta.join("1-2.json"), &temp));
        // Casos negativos: fuera, en una subcarpeta, con `..`, otra extension,
        // relativa o ya existente.
        assert!(!respuesta_valida(&temp.join("1.json"), &temp));
        assert!(!respuesta_valida(&carpeta.join("sub").join("1.json"), &temp));
        assert!(!respuesta_valida(&carpeta.join("..").join("1.json"), &temp));
        assert!(!respuesta_valida(&carpeta.join("1.exe"), &temp));
        assert!(!respuesta_valida(Path::new(r"pixpin-mcp\1.json"), &temp));
        let ya = carpeta.join("ya.json");
        std::fs::write(&ya, "x").unwrap();
        assert!(!respuesta_valida(&ya, &temp));
        // Se escribe entera, sin dejar el `.parte`.
        let f = carpeta.join("r.json");
        escribir_respuesta(&f, &Ok("Apuntada «pan».".into())).unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(v, json!({"ok": true, "texto": "Apuntada «pan»."}));
        assert!(!f.with_extension("parte").exists());
        let _ = std::fs::remove_dir_all(&temp);
    }

    /// **De punta a punta con las tareas de verdad**: la herramienta pedida
    /// desde fuera se hace como el asistente (por su nombre, con su fecha),
    /// y lo que lleva `preguntar` no se hace si dice que no.
    #[test]
    fn la_herramienta_se_hace_como_en_el_asistente() {
        let raiz = std::env::temp_dir().join(format!("pixpin-externa-tareas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        crate::tareas::apuntar(&raiz, "PC01", "comprar pan").unwrap();
        let mandados = Arc::new(Mutex::new(Vec::<String>::new()));
        let (r2, m2) = (raiz.clone(), mandados.clone());
        let mandar: Mandar = Box::new(move |json| {
            m2.lock().unwrap().push(json.to_string());
            let u = pixpin_store::Ubicacion::Portable { raiz: r2.clone() };
            let textos = pixpin_store::Catalogo::nuevo(pixpin_store::Idioma::Espanol);
            let cx = crate::pedidos::Contexto {
                idioma: pixpin_store::Idioma::Espanol,
                ubicacion: &u,
                opciones: crate::ventana_chat::OpcionesLienzo {
                    enganche: Default::default(),
                    nivel: pixpin_nivel::Nivel::Ligero,
                    medir_fotogramas: false,
                },
                aparato: "PC01",
                textos: &textos,
            };
            crate::pedidos::atender(json, &cx);
            1
        });
        let si: Confirmar = Box::new(|_| true);
        let no: Confirmar = Box::new(|_| false);
        let es = pixpin_store::Idioma::Espanol;
        let encargo = |nombre: &str, args: &str, preguntar: Option<&str>| Encargo {
            nombre: nombre.into(),
            args: Some(args.into()),
            preguntar: preguntar.map(str::to_string),
            ..Default::default()
        };
        let r = hacer_con(&encargo("anadir_tarea", r#"{"texto":"revisar planos","fecha":"mañana a las 10"}"#, None), &raiz, es, &si, &mandar);
        assert_eq!(r.as_deref(), Ok("Apuntada «revisar planos» para mañana 10:00."));
        // Con `preguntar` y un no: nada sale.
        let antes = mandados.lock().unwrap().len();
        let r = hacer_con(&encargo("marcar_tarea", r#"{"tarea":"comprar pan"}"#, Some("¿Marco?")), &raiz, es, &no, &mandar);
        assert!(r.unwrap_err().contains("dijo que no"));
        assert_eq!(mandados.lock().unwrap().len(), antes, "salio sin permiso");
        let r = hacer_con(&encargo("marcar_tarea", r#"{"tarea":"comprar pan"}"#, Some("¿Marco?")), &raiz, es, &si, &mandar);
        assert_eq!(r.as_deref(), Ok("«comprar pan» marcada como hecha."));
        // La consulta tambien vale (contesta sin mandar nada).
        let r = hacer_con(&encargo("consultar_tareas", "{}", None), &raiz, es, &si, &mandar).unwrap();
        assert!(r.contains("revisar planos"), "{r}");
        // Negativos: una que no existe, la del lienzo, argumentos que no son
        // un objeto o que no valen.
        assert!(hacer_con(&encargo("borrar_todo", "{}", None), &raiz, es, &si, &mandar).unwrap_err().contains("borrar_todo"));
        assert!(hacer_con(&encargo("hoja_a_lienzo", "{}", None), &raiz, es, &si, &mandar).is_err());
        assert!(hacer_con(&encargo("anadir_tarea", "[1]", None), &raiz, es, &si, &mandar).unwrap_err().contains("objeto"));
        assert!(hacer_con(&encargo("anadir_tarea", "{}", None), &raiz, es, &si, &mandar).unwrap_err().contains("«texto»"));
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
