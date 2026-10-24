//! **Una conversacion MCP de verdad por stdio** con `pixpin-mcp.exe`:
//! `initialize`, `tools/list`, herramientas de leer sobre datos de prueba en
//! una carpeta temporal, una herramienta desconocida (error bien formado) y
//! herramientas de la app contra una **ventana de mentira** que hace de
//! PixPin (`PIXPIN_MCP_VENTANA`): la PixPin del usuario no se toca nunca.
//!
//! Mide el arranque (hasta la respuesta a `initialize`) y `tools/list`.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, HWND_MESSAGE, MSG,
    RegisterClassW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_COPYDATA, WNDCLASSW,
};
use windows::core::{HSTRING, PCWSTR};

use pixpin_ia::documentos::{Abrir, Firma, Fuente, Indice, Tipo, Unidad};
use pixpin_proyecto::cuaderno::{self, Mensaje, Sello};
use pixpin_proyecto::{almacen, mini};

/// Los pedidos que le llegaron a la ventana de mentira.
static RECIBIDOS: Mutex<Vec<Value>> = Mutex::new(Vec::new());

/// La ventana de mentira: lee el pedido como PixPin (`pedido_de_copydata`),
/// lo apunta y contesta en su fichero de respuesta lo que hizo.
unsafe extern "system" fn procedimiento(h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_COPYDATA {
        // SAFETY: `l` es el de un WM_COPYDATA recien recibido.
        if let Some(r) = unsafe { pixpin_shell::mensajero::pedido_de_copydata(l) } {
            return match r {
                Ok(json) => {
                    let p: Value = serde_json::from_str(&json).unwrap();
                    if let Some(f) = p["respuesta"].as_str() {
                        let nombre = p["nombre"].as_str().unwrap_or("");
                        let resp = if nombre == "quitar_tarea" {
                            json!({"ok": false, "texto": "el usuario dijo que no; no se hizo nada"})
                        } else {
                            json!({"ok": true, "texto": format!("hecho {nombre} {}", p["args"])})
                        };
                        let parte = Path::new(f).with_extension("parte");
                        std::fs::write(&parte, resp.to_string()).unwrap();
                        std::fs::rename(&parte, f).unwrap();
                    }
                    RECIBIDOS.lock().unwrap().push(p);
                    LRESULT(1)
                }
                Err(c) => LRESULT(c),
            };
        }
    }
    // SAFETY: lo demas, a Windows.
    unsafe { DefWindowProcW(h, msg, w, l) }
}

/// Saca la ventana de mentira en su hilo, con su bucle.
fn ventana_de_mentira(clase: String) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let nombre = HSTRING::from(clase);
        // SAFETY: registro y creacion de una ventana de solo mensajes propia;
        // `nombre` vive todo el hilo.
        unsafe {
            let inst = GetModuleHandleW(None).unwrap();
            let wc = WNDCLASSW {
                lpfnWndProc: Some(procedimiento),
                hInstance: inst.into(),
                lpszClassName: PCWSTR(nombre.as_ptr()),
                ..Default::default()
            };
            RegisterClassW(&wc);
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                &nombre,
                &nombre,
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(inst.into()),
                None,
            )
            .unwrap();
            tx.send(()).unwrap();
            let mut m = MSG::default();
            while GetMessageW(&mut m, None, 0, 0).as_bool() {
                DispatchMessageW(&m);
            }
        }
    });
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
}

/// Una carpeta de datos de prueba: «Mensajes guardados» con su Inbox y el
/// indice de documentos con un PDF que habla de cunetas.
fn datos_de_prueba(raiz: &Path) -> PathBuf {
    let _ = std::fs::remove_dir_all(raiz);
    std::fs::create_dir_all(raiz).unwrap();
    let ahora = 1_760_000_000_000;
    let f = almacen::asegurar_guardados(raiz, ahora, "PC01").unwrap();
    let carpeta = almacen::carpeta(raiz, &f.id);
    let sello = Sello {
        cuando: ahora,
        numero: 1,
        aparato: "PC01".into(),
        proyecto: f.id.clone(),
    };
    cuaderno::anadir(
        &carpeta,
        &Mensaje::miniapp(
            mini::TAREAS,
            "Inbox",
            "# Inbox\n- [ ] comprar pan\n- [x] pagar la luz\n- [ ] llamar al ingeniero\n",
            &sello,
        ),
    )
    .unwrap();
    let md = raiz.join("acta.md");
    std::fs::write(
        &md,
        "# Acta de obra\n\nSe acordo revisar las cotas del puente.\n",
    )
    .unwrap();
    let pdf = raiz.join("Informe de cunetas.pdf");
    let mut ix = Indice::nuevo(Some(raiz.join("cache").join("documentos")));
    ix.aplicar(
        Fuente {
            clave: format!("f:{}", pdf.display()),
            proyecto: f.id.clone(),
            proyecto_nombre: "Mensajes guardados".into(),
            nombre: "Informe de cunetas.pdf".into(),
            tipo: Tipo::Pdf,
            ruta: pdf.clone(),
            firma: Firma { bytes: 10, modificado: ahora },
            abrir: Abrir::Fichero { ruta: pdf.clone(), pagina: None },
        },
        Ok(vec![Unidad {
            texto: "La pendiente de las cunetas del kilometro tres es del dos por ciento, revestidas de concreto.".into(),
            pagina: Some(4),
            ..Default::default()
        }]),
    );
    ix.guardar();
    md
}

struct Servidor {
    hijo: Child,
    entrada: ChildStdin,
    salida: BufReader<ChildStdout>,
    id: u64,
}

impl Servidor {
    fn arrancar(raiz: &Path, clase: &str) -> Servidor {
        let mut hijo = Command::new(env!("CARGO_BIN_EXE_pixpin-mcp"))
            .env("PIXPIN_RAIZ", raiz)
            .env("PIXPIN_MCP_VENTANA", clase)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        Servidor {
            entrada: hijo.stdin.take().unwrap(),
            salida: BufReader::new(hijo.stdout.take().unwrap()),
            hijo,
            id: 0,
        }
    }

    fn mandar(&mut self, v: &Value) {
        writeln!(self.entrada, "{v}").unwrap();
        self.entrada.flush().unwrap();
    }

    fn leer(&mut self) -> Value {
        let mut l = String::new();
        self.salida.read_line(&mut l).unwrap();
        assert!(!l.is_empty(), "el servidor cerro stdout");
        serde_json::from_str(&l).unwrap_or_else(|e| panic!("{e}: {l}"))
    }

    fn pedir(&mut self, metodo: &str, params: Value) -> Value {
        self.id += 1;
        let id = self.id;
        self.mandar(&json!({"jsonrpc": "2.0", "id": id, "method": metodo, "params": params}));
        let r = self.leer();
        assert_eq!(r["jsonrpc"], "2.0");
        assert_eq!(r["id"], id, "{r}");
        r
    }

    fn llamar(&mut self, nombre: &str, args: Value) -> Value {
        self.pedir("tools/call", json!({"name": nombre, "arguments": args}))
    }
}

fn texto(r: &Value) -> String {
    r["result"]["content"]
        .as_array()
        .unwrap_or_else(|| panic!("sin content: {r}"))
        .iter()
        .filter_map(|c| c["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn una_conversacion_de_ida_y_vuelta() {
    let raiz = std::env::temp_dir().join(format!("pixpin-mcp-prueba-{}", std::process::id()));
    let md = datos_de_prueba(&raiz);
    let clase = format!("PixPinMcpPrueba{}", std::process::id());
    ventana_de_mentira(clase.clone());

    // Arranque: hasta la respuesta a `initialize`.
    let t0 = Instant::now();
    let mut s = Servidor::arrancar(&raiz, &clase);
    let r = s.pedir(
        "initialize",
        json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "prueba", "version": "1"}}),
    );
    let arranque = t0.elapsed();
    assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(r["result"]["serverInfo"]["name"], "pixpin");
    s.mandar(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    assert!(s.pedir("ping", json!({}))["result"].is_object());

    // La lista, entera.
    let t0 = Instant::now();
    let r = s.pedir("tools/list", json!({}));
    let lista = t0.elapsed();
    let tools = r["result"]["tools"].as_array().unwrap();
    let n = pixpin_mcp::herramientas::lista().len();
    assert_eq!(tools.len(), n);
    assert!(n >= 40, "{n} herramientas");
    for nombre in [
        "anadir_tarea",
        "borrar_lista",
        "consultar_tareas",
        "buscar_en_mis_cosas",
        "ver_pantalla",
        "tabla_de_imagen",
    ] {
        assert!(tools.iter().any(|t| t["name"] == nombre), "falta {nombre}");
    }
    println!(
        "arranque hasta initialize: {arranque:?}; tools/list ({n} herramientas, {} KB): {lista:?}",
        r.to_string().len() / 1024
    );

    // Leer, sin PixPin: las tareas, buscar, un documento.
    let r = s.llamar("consultar_tareas", json!({}));
    assert_eq!(r["result"]["isError"], false, "{r}");
    let t = texto(&r);
    assert!(
        t.contains("comprar pan")
            && t.contains("llamar al ingeniero")
            && !t.contains("pagar la luz"),
        "{t}"
    );
    let r = s.llamar("consultar_tareas", json!({"estado": "hechas"}));
    assert!(texto(&r).contains("pagar la luz"), "{r}");
    let r = s.llamar(
        "buscar_en_mis_cosas",
        json!({"consulta": "pendiente cunetas"}),
    );
    let t = texto(&r);
    assert!(
        t.contains("Informe de cunetas.pdf")
            && t.contains("dos por ciento")
            && t.contains("abrir_fichero"),
        "{t}"
    );
    let r = s.llamar("leer_documento", json!({"ruta": md.display().to_string()}));
    assert!(texto(&r).contains("cotas del puente"), "{r}");
    let r = s.llamar("listar_chats", json!({}));
    assert!(texto(&r).contains("Mensajes guardados"), "{r}");
    // Un fallo de la herramienta es un resultado con isError (el modelo lo lee).
    let r = s.llamar("leer_documento", json!({"ruta": "C:\\no\\esta.pdf"}));
    assert_eq!(r["result"]["isError"], true, "{r}");

    // Herramienta desconocida: error JSON-RPC bien formado.
    let r = s.llamar("borrar_el_disco", json!({}));
    assert_eq!(r["error"]["code"], -32602, "{r}");
    assert!(
        r["error"]["message"]
            .as_str()
            .unwrap()
            .contains("borrar_el_disco")
    );
    assert!(r.get("result").is_none());
    // Metodo desconocido.
    assert_eq!(s.pedir("cosas/raras", json!({}))["error"]["code"], -32601);

    // Escribir: a la ventana de mentira, con su respuesta de vuelta.
    let t0 = Instant::now();
    let r = s.llamar(
        "anadir_tarea",
        json!({"texto": "comprar yeso", "fecha": "mañana"}),
    );
    println!("anadir_tarea de ida y vuelta: {:?}", t0.elapsed());
    assert_eq!(r["result"]["isError"], false, "{r}");
    assert!(texto(&r).starts_with("hecho anadir_tarea"), "{r}");
    // Mal puesta: el motivo de la tabla, sin llegar a PixPin.
    let antes = RECIBIDOS.lock().unwrap().len();
    let r = s.llamar("anadir_tarea", json!({}));
    assert_eq!(r["result"]["isError"], true);
    assert!(texto(&r).contains("«texto»"), "{r}");
    assert_eq!(
        RECIBIDOS.lock().unwrap().len(),
        antes,
        "lo mal puesto no sale"
    );
    // Lo que cambia algo lleva su pregunta; lo que borra, la de la tabla
    // (y si el usuario dice que no, isError con el motivo).
    let r = s.llamar(
        "editar_tarea",
        json!({"tarea": "comprar pan", "texto": "comprar pan integral"}),
    );
    assert_eq!(r["result"]["isError"], false, "{r}");
    let r = s.llamar("quitar_tarea", json!({"tarea": "comprar pan"}));
    assert_eq!(r["result"]["isError"], true);
    assert!(texto(&r).contains("dijo que no"));
    {
        let rec = RECIBIDOS.lock().unwrap();
        let de = |n: &str| {
            rec.iter()
                .find(|p| p["nombre"] == n)
                .unwrap_or_else(|| panic!("no llego {n}"))
        };
        assert_eq!(de("anadir_tarea")["accion"], "herramienta");
        assert_eq!(de("anadir_tarea")["args"]["fecha"], "mañana");
        assert!(de("anadir_tarea").get("preguntar").is_none());
        assert!(
            de("editar_tarea")["preguntar"]
                .as_str()
                .unwrap()
                .contains("comprar pan integral")
        );
        assert!(
            de("quitar_tarea").get("preguntar").is_none(),
            "la pregunta de borrar la pone la tabla en PixPin"
        );
        // Ninguna respuesta se queda en %TEMP%.
        for p in rec.iter() {
            assert!(!Path::new(p["respuesta"].as_str().unwrap()).exists());
        }
    }

    // Se cierra la entrada: el servidor se va solo.
    drop(s.entrada);
    let t0 = Instant::now();
    loop {
        if let Some(st) = s.hijo.try_wait().unwrap() {
            assert!(st.success());
            break;
        }
        assert!(
            t0.elapsed() < Duration::from_secs(5),
            "no se fue al cerrar stdin"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = std::fs::remove_dir_all(&raiz);
}

/// **Lo que tarda en arrancar** (hasta contestar `initialize`) y en dar la
/// lista, varias veces: la primera paga leer el exe del disco.
#[test]
fn arranque_y_lista_medidos() {
    let raiz = std::env::temp_dir().join(format!("pixpin-mcp-medir-{}", std::process::id()));
    std::fs::create_dir_all(&raiz).unwrap();
    let mut arranques = Vec::new();
    let mut listas = Vec::new();
    for _ in 0..8 {
        let t0 = Instant::now();
        let mut s = Servidor::arrancar(&raiz, "PixPinMcpNadieEscucha");
        s.pedir("initialize", json!({"protocolVersion": "2025-06-18"}));
        arranques.push(t0.elapsed());
        let t0 = Instant::now();
        let r = s.pedir("tools/list", json!({}));
        listas.push(t0.elapsed());
        assert!(r["result"]["tools"].as_array().unwrap().len() >= 40);
        drop(s.entrada);
        let _ = s.hijo.wait();
    }
    let mediana = |v: &mut Vec<Duration>| {
        v.sort();
        (v[0], v[v.len() / 2])
    };
    let (amin, amed) = mediana(&mut arranques);
    let (lmin, lmed) = mediana(&mut listas);
    println!(
        "arranque: minimo {amin:?}, mediana {amed:?}; tools/list: minimo {lmin:?}, mediana {lmed:?}"
    );
    // Holgado: es la compilacion de depuracion y el equipo compila a la vez.
    assert!(amin < Duration::from_secs(2), "{amin:?}");
    let _ = std::fs::remove_dir_all(&raiz);
}

/// Sin PixPin (ni la de mentira), lo de escribir dice que no esta abierta y
/// lo de leer sigue funcionando.
#[test]
fn sin_pixpin_se_dice_y_leer_sigue() {
    let raiz = std::env::temp_dir().join(format!("pixpin-mcp-sin-{}", std::process::id()));
    datos_de_prueba(&raiz);
    let mut s = Servidor::arrancar(&raiz, "PixPinMcpNadieEscucha");
    s.pedir("initialize", json!({"protocolVersion": "2025-11-25"}));
    let r = s.llamar("anadir_tarea", json!({"texto": "x"}));
    assert_eq!(r["result"]["isError"], true);
    assert!(texto(&r).contains("no esta abierta"), "{r}");
    let r = s.llamar("consultar_tareas", json!({}));
    assert!(texto(&r).contains("comprar pan"), "{r}");
    drop(s.entrada);
    let _ = s.hijo.wait();
    let _ = std::fs::remove_dir_all(&raiz);
}
