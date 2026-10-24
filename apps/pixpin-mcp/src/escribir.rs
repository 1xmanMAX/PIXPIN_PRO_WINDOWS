//! **Las herramientas de la app**, hechas por la PixPin que corre.
//!
//! El protocolo de pedidos solo contesta «aceptado» (`1`): el trabajo lo
//! hace luego el bucle de la app. Pero el modelo necesita saber que paso
//! («Apuntada «pan» para mañana 10:00», «No encuentro esa tarea»). Por eso
//! hay un pedido propio, `herramienta` (`docs/protocolo-pedidos.md`):
//!
//! ```json
//! {"pixpin":1, "accion":"herramienta", "nombre":"anadir_tarea",
//!  "args":{"texto":"pan","fecha":"mañana"},
//!  "respuesta":"C:\\Users\\…\\Temp\\pixpin-mcp\\1234-1.json",
//!  "preguntar":"¿…?", "origen":"Claude"}
//! ```
//!
//! La app valida la fila con la misma tabla, la hace como el asistente
//! (`asistente::acciones::hacer` o `documentos_acciones::hacer`, con el
//! permiso en su aviso flotante) y escribe `{"ok":bool,"texto":"…"}` en
//! `respuesta`, de una vez (escribe al lado y renombra). Aqui se espera a que
//! aparezca ese fichero.
//!
//! **Por que un fichero** y no una ventana o una tuberia de vuelta: no hace
//! falta que este exe tenga ventana ni bucle de mensajes, sobrevive a que
//! cualquiera de los dos procesos se caiga (lo peor es esperar hasta el
//! plazo) y se prueba sin la app con una ventana de mentira. La app solo
//! escribe dentro de `%TEMP%\pixpin-mcp\` y nunca encima de nada.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowExW, FindWindowW, HWND_MESSAGE};
use windows::core::HSTRING;

use pixpin_ia::acciones::{self, Accion};
use pixpin_shell::mensajero::{self, respuesta};

/// La clase de la ventana de mensajes de PixPin.
pub const CLASE: &str = "PixPinMaxVentanaMensajes";
/// Otra clase (pruebas: una ventana de mentira que hace de PixPin). Con
/// ella no se arranca nunca PixPin.
pub const VAR_VENTANA: &str = "PIXPIN_MCP_VENTANA";
/// La carpeta de las respuestas, dentro de `%TEMP%`.
pub const CARPETA: &str = "pixpin-mcp";
/// Cuanto se espera a la respuesta de algo que no pregunta.
pub const PLAZO: Duration = Duration::from_secs(150);
/// Y de algo que pregunta: lo que tarda la pregunta en caducar en la app
/// (`asistente::chat::CADUCA_LA_PREGUNTA`, 5 min) y un margen.
pub const PLAZO_CON_PREGUNTA: Duration = Duration::from_secs(5 * 60 + 30);
/// Lo que se espera a que aparezca la ventana tras arrancar PixPin (lo mismo
/// que el plugin de Flow Launcher).
const ARRANQUE: Duration = Duration::from_secs(8);

fn clase() -> String {
    std::env::var(VAR_VENTANA)
        .ok()
        .filter(|c| !c.trim().is_empty())
        .unwrap_or_else(|| CLASE.to_string())
}

/// La ventana de mensajes, arriba o entre las de solo mensajes (como
/// `mensajero::ventana_de_la_copia`, pero con la clase que toque).
fn ventana(clase: &str) -> Option<HWND> {
    let c = HSTRING::from(clase);
    // SAFETY: `c` vive durante la llamada; una ventana nula es «no hay».
    if let Ok(h) = unsafe { FindWindowW(&c, None) }
        && !h.0.is_null()
    {
        return Some(h);
    }
    // SAFETY: igual; HWND_MESSAGE es el padre valido para buscar entre las
    // ventanas de solo mensajes.
    let h = unsafe { FindWindowExW(Some(HWND_MESSAGE), None, &c, None) }.ok()?;
    (!h.0.is_null()).then_some(h)
}

/// **La ventana de PixPin**; si no esta abierta, la arranca (la que esta al
/// lado de este exe, que es donde la deja `instalar.ps1`) y la espera.
fn ventana_de_pixpin() -> Result<HWND, String> {
    let clase = clase();
    if let Some(h) = ventana(&clase) {
        return Ok(h);
    }
    let cerrada =
        || "PixPin no esta abierta y no se pudo arrancar; pide al usuario que la abra".to_string();
    if clase != CLASE {
        return Err(cerrada());
    }
    let exe = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("pixpinmax.exe")))
        .filter(|e| e.is_file())
        .ok_or_else(cerrada)?;
    std::process::Command::new(&exe)
        .spawn()
        .map_err(|_| cerrada())?;
    let hasta = Instant::now() + ARRANQUE;
    while Instant::now() < hasta {
        std::thread::sleep(Duration::from_millis(100));
        if let Some(h) = ventana(&clase) {
            return Ok(h);
        }
    }
    Err(cerrada())
}

static CONTADOR: AtomicU64 = AtomicU64::new(0);

/// Un fichero de respuesta nuevo (aun no existe).
fn fichero_de_respuesta() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(CARPETA);
    std::fs::create_dir_all(&dir).map_err(|e| format!("no se pudo preparar la respuesta: {e}"))?;
    let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
    let ms = pixpin_shell::entorno::ahora_utc_ms();
    Ok(dir.join(format!("{}-{ms}-{n}.json", std::process::id())))
}

/// La pregunta que hace PixPin para lo que cambia algo que ya existe
/// ([`crate::herramientas::MODIFICA`]); lo que ya pregunta en la tabla lleva
/// su propia pregunta (con los nombres de verdad) y no necesita esta.
pub fn pregunta_de(a: &Accion, args: &Map<String, Value>) -> Option<String> {
    if a.confirmar.is_some() || !crate::herramientas::MODIFICA.contains(&a.herramienta) {
        return None;
    }
    let mut datos: Vec<String> = a
        .params
        .iter()
        .filter_map(|p| {
            let v = args.get(p.nombre)?;
            let t = acciones::leible(v);
            (!t.trim().is_empty()).then(|| format!("{}: «{}»", p.nombre, recortar(&t, 80)))
        })
        .collect();
    datos.truncate(6);
    Some(format!(
        "Claude pide: {} ({}). ¿Lo hago?",
        a.descripcion.trim_end_matches('.'),
        datos.join(", ")
    ))
}

fn recortar(t: &str, n: usize) -> String {
    if t.chars().count() <= n {
        t.to_string()
    } else {
        format!("{}…", t.chars().take(n).collect::<String>())
    }
}

/// El pedido `herramienta` para esa fila.
pub fn pedido(a: &Accion, args: &Map<String, Value>, respuesta: &Path) -> Value {
    let mut p = json!({
        "pixpin": 1,
        "accion": "herramienta",
        "nombre": a.herramienta,
        "args": args,
        "respuesta": respuesta.display().to_string(),
        "origen": "Claude",
    });
    if let Some(q) = pregunta_de(a, args) {
        p["preguntar"] = json!(q);
    }
    p
}

/// Lee una respuesta ya escrita: `Ok` con su texto o `Err` con el motivo.
pub fn leer_respuesta(texto: &str) -> Result<String, String> {
    let v: Value = serde_json::from_str(texto)
        .map_err(|_| "PixPin contesto algo que no se entiende".to_string())?;
    let t = v
        .get("texto")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if v.get("ok").and_then(Value::as_bool) == Some(true) {
        Ok(if t.trim().is_empty() {
            "Hecho.".into()
        } else {
            t
        })
    } else {
        Err(if t.trim().is_empty() {
            "PixPin no pudo hacerlo".into()
        } else {
            t
        })
    }
}

/// **Hace una fila de la tabla en PixPin** y devuelve lo que contesto.
pub fn hacer(a: &'static Accion, args: &Map<String, Value>) -> Result<String, String> {
    // Lo mal puesto se dice ya, con el motivo de la tabla, sin ir a la app.
    acciones::validar(a, args).map_err(|e| e.to_string())?;
    let destino = ventana_de_pixpin()?;
    let fichero = fichero_de_respuesta()?;
    let json = pedido(a, args, &fichero).to_string();
    match mensajero::enviar_pedido_a(destino, &json) {
        respuesta::ACEPTADO => {}
        respuesta::NO_SE_ENTIENDE => {
            return Err("esta PixPin no entiende el pedido «herramienta»: hay que actualizarla (herramientas\\instalar.ps1)".into());
        }
        respuesta::VERSION_NO => {
            return Err("esta PixPin no conoce la version 1 de los pedidos".into());
        }
        _ => return Err("PixPin no contesto; puede que se este cerrando".into()),
    }
    let plazo = if crate::herramientas::pregunta(a) {
        PLAZO_CON_PREGUNTA
    } else {
        PLAZO
    };
    esperar(&fichero, plazo)
}

/// Espera a que aparezca el fichero de respuesta (la app lo crea de una vez,
/// renombrando), lo lee y lo borra.
pub fn esperar(fichero: &Path, plazo: Duration) -> Result<String, String> {
    let hasta = Instant::now() + plazo;
    let mut paso = Duration::from_millis(5);
    loop {
        if let Ok(texto) = std::fs::read_to_string(fichero) {
            let _ = std::fs::remove_file(fichero);
            return leer_respuesta(&texto);
        }
        if Instant::now() >= hasta {
            return Err(format!("PixPin no contesto en {} s", plazo.as_secs()));
        }
        std::thread::sleep(paso);
        // Rapido al principio (casi todo tarda milisegundos), luego sin
        // gastar: una pregunta al usuario puede tardar minutos.
        paso = (paso * 2).min(Duration::from_millis(100));
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn args(s: &str) -> Map<String, Value> {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn el_pedido_lleva_la_fila_los_argumentos_y_la_respuesta() {
        let a = acciones::por_nombre("anadir_tarea").unwrap();
        let p = pedido(
            a,
            &args(r#"{"texto":"pan","fecha":"mañana"}"#),
            Path::new(r"C:\t\r.json"),
        );
        assert_eq!(p["accion"], "herramienta");
        assert_eq!(p["nombre"], "anadir_tarea");
        assert_eq!(p["args"]["fecha"], "mañana");
        assert_eq!(p["respuesta"], r"C:\t\r.json");
        assert!(p.get("preguntar").is_none(), "crear no pregunta");
        // El vistazo de la ventana de PixPin lo da por bueno.
        assert_eq!(
            mensajero::validar_pedido(&p.to_string()),
            respuesta::ACEPTADO
        );
    }

    #[test]
    fn lo_que_cambia_algo_lleva_su_pregunta_y_lo_que_borra_la_suya() {
        let e = acciones::por_nombre("editar_tarea").unwrap();
        let q = pregunta_de(e, &args(r#"{"tarea":"pan","texto":"pan integral"}"#)).unwrap();
        assert!(
            q.starts_with("Claude pide: Cambia el texto") && q.contains("«pan integral»"),
            "{q}"
        );
        assert!(q.contains("tarea: «pan»"), "{q}");
        // La de borrar ya pregunta en la app con su frase: no se duplica.
        assert!(
            pregunta_de(
                acciones::por_nombre("borrar_lista").unwrap(),
                &args(r#"{"lista":"x"}"#)
            )
            .is_none()
        );
        assert!(pregunta_de(acciones::por_nombre("nota_nueva").unwrap(), &Map::new()).is_none());
    }

    #[test]
    fn la_respuesta_se_lee_bien_o_mal() {
        assert_eq!(
            leer_respuesta(r#"{"ok":true,"texto":"Apuntada «pan»."}"#),
            Ok("Apuntada «pan».".into())
        );
        assert_eq!(leer_respuesta(r#"{"ok":true}"#), Ok("Hecho.".into()));
        assert_eq!(
            leer_respuesta(r#"{"ok":false,"texto":"No encuentro «x»"}"#),
            Err("No encuentro «x»".into())
        );
        assert!(leer_respuesta("basura").is_err());
    }

    #[test]
    fn sin_respuesta_se_rinde_al_plazo() {
        let f = std::env::temp_dir().join(format!("pixpin-mcp-nada-{}.json", std::process::id()));
        let t0 = Instant::now();
        assert!(
            esperar(&f, Duration::from_millis(150))
                .unwrap_err()
                .contains("no contesto")
        );
        assert!(t0.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn lo_mal_puesto_no_sale_de_aqui() {
        // Sin «texto» no hay tarea: el error de la tabla, sin buscar PixPin.
        let e = hacer(acciones::por_nombre("anadir_tarea").unwrap(), &Map::new()).unwrap_err();
        assert!(e.contains("«texto»"), "{e}");
    }
}
