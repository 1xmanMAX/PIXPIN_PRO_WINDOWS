//! **La linea de ordenes de punta a punta**: el `pixpinmax.exe` de verdad,
//! lanzado como lo lanzaria un script, contra una ventana de mentira de
//! esta prueba que contesta como la de PixPin (`mensajero::leer_pedido`).
//!
//! Nunca toca la PixPin del usuario: `PIXPIN_PEDIDOS_VENTANA` hace que la
//! orden busque la ventana de esta prueba (y que nunca abra la app), y
//! `PIXPIN_ORDEN_SIN_CUADROS` que no salga ningun cuadro. Por si algo se
//! colara hasta el arranque normal, `PIXPIN_INSTANCIA` lo mandaria a una
//! PixPin de prueba aparte (`instancia_de_prueba`), no a la del usuario.
//!
//! `#[ignore]` porque lanza el ejecutable (que tiene que estar compilado):
//! `cargo test -p pixpin --test orden_de_punta_a_punta -- --ignored --test-threads=1 --nocapture`
//!
//! Es un test de integracion (binario aparte): el forbid(unsafe_code) de
//! main.rs no aplica aqui, pero cada bloque unsafe lleva su SAFETY.

use std::process::{Command, Output, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use pixpin_shell::mensajero::{PEDIDO_JSON, leer_pedido};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, HWND_MESSAGE, MSG,
    RegisterClassW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_COPYDATA, WNDCLASSW,
};
use windows::core::{HSTRING, PCWSTR};

static RECIBIDOS: Mutex<Vec<String>> = Mutex::new(Vec::new());

unsafe extern "system" fn procedimiento(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if m == WM_COPYDATA && l.0 != 0 {
        // SAFETY: es un WM_COPYDATA recien recibido: su COPYDATASTRUCT y sus
        // datos viven mientras dura el mensaje, y aqui se copian.
        let paquete = unsafe { &*(l.0 as *const COPYDATASTRUCT) };
        if paquete.dwData != PEDIDO_JSON || paquete.lpData.is_null() {
            return LRESULT(0);
        }
        // SAFETY: igual que arriba.
        let bytes = unsafe {
            std::slice::from_raw_parts(paquete.lpData as *const u8, paquete.cbData as usize)
        };
        return match leer_pedido(bytes) {
            Ok(t) => {
                RECIBIDOS.lock().unwrap().push(t);
                LRESULT(1)
            }
            Err(r) => LRESULT(r),
        };
    }
    // SAFETY: el procedimiento por defecto con los mismos argumentos.
    unsafe { DefWindowProcW(h, m, w, l) }
}

/// La clase de la ventana de mentira (una sola por proceso de pruebas).
fn clase() -> &'static str {
    static CLASE: OnceLock<String> = OnceLock::new();
    CLASE.get_or_init(|| {
        let clase = format!("PixPinPruebaOrden{}", std::process::id());
        let c = clase.clone();
        let (listo_tx, listo_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let nombre = HSTRING::from(c.as_str());
            // SAFETY: clase y ventana de solo mensajes de este hilo; los
            // nombres viven durante las llamadas; el bucle es el de siempre.
            unsafe {
                let instancia = GetModuleHandleW(None).unwrap();
                let wc = WNDCLASSW {
                    lpfnWndProc: Some(procedimiento),
                    hInstance: instancia.into(),
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
                    Some(instancia.into()),
                    None,
                )
                .unwrap();
                listo_tx.send(()).unwrap();
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    DispatchMessageW(&msg);
                }
            }
        });
        listo_rx.recv().unwrap();
        clase
    })
}

fn orden(clase_destino: &str, args: &[&str], entrada: Option<&str>) -> (Output, Duration) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_pixpinmax"));
    c.args(args)
        .env("PIXPIN_PEDIDOS_VENTANA", clase_destino)
        .env("PIXPIN_ORDEN_SIN_CUADROS", "1")
        .env("PIXPIN_INSTANCIA", "orden-prueba")
        .stdin(if entrada.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let inicio = Instant::now();
    let mut hijo = c.spawn().unwrap();
    if let Some(e) = entrada {
        use std::io::Write;
        hijo.stdin.take().unwrap().write_all(e.as_bytes()).unwrap();
    }
    let salida = hijo.wait_with_output().unwrap();
    (salida, inicio.elapsed())
}

fn a_la_prueba(args: &[&str]) -> (i32, Vec<String>, String) {
    RECIBIDOS.lock().unwrap().clear();
    let (s, _) = orden(clase(), args, None);
    let recibidos = std::mem::take(&mut *RECIBIDOS.lock().unwrap());
    (
        s.status.code().unwrap(),
        recibidos,
        String::from_utf8_lossy(&s.stderr).into_owned(),
    )
}

fn valor(t: &str) -> serde_json::Value {
    serde_json::from_str(t).unwrap()
}

#[test]
#[ignore = "lanza pixpinmax.exe; ver la cabecera"]
fn un_verbo_llega_como_pedido_y_sale_con_cero() {
    let (c, r, err) = a_la_prueba(&["tarea", "comprar", "yeso"]);
    assert_eq!(c, 0, "{err}");
    assert_eq!(r.len(), 1);
    assert_eq!(
        valor(&r[0]),
        serde_json::json!({"pixpin":1,"accion":"anadir_tarea","texto":"comprar yeso"})
    );
    let (c, r, _) = a_la_prueba(&["--pedido", r#"{"pixpin":1,"accion":"capturar"}"#]);
    assert_eq!((c, r.len()), (0, 1));
    // Un pedido por la entrada estandar (`--pedido-archivo -`).
    RECIBIDOS.lock().unwrap().clear();
    let (s, _) = orden(
        clase(),
        &["--pedido-archivo", "-"],
        Some("\u{feff}{\"pixpin\":1,\"accion\":\"ventana_principal\"}\n"),
    );
    assert_eq!(
        s.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&s.stderr)
    );
    assert_eq!(RECIBIDOS.lock().unwrap().len(), 1);
}

#[test]
#[ignore = "lanza pixpinmax.exe; ver la cabecera"]
fn los_enlaces_llegan_y_los_de_android_tambien() {
    for u in [
        "pixpin://tarea?texto=pan%20integral",
        "pixpin://atajo/capturar",
        "PIXPIN://atajo/proyecto/pr-1/",
    ] {
        let (c, r, err) = a_la_prueba(&["--url", u]);
        assert_eq!((c, r.len()), (0, 1), "{u}: {err}");
    }
    // Tal cual, sin --url (como lo pasaria un acceso directo).
    let (c, r, _) = a_la_prueba(&["pixpin://nota?texto=idea"]);
    assert_eq!((c, r.len()), (0, 1));
}

#[test]
#[ignore = "lanza pixpinmax.exe; ver la cabecera"]
fn caso_negativo_codigos_de_salida_y_nada_llega() {
    let casos: [(&[&str], i32); 8] = [
        (&["--pedido", r#"{"pixpin":2,"accion":"chat"}"#], 2),
        (&["--pedido", "no es json"], 3),
        (&["tarea", "x", "--proyeto", "P"], 3),
        (&["--url", "pixpin://borrar_todo"], 3),
        // Enlaces maliciosos: una ruta de red, y crear una automatizacion.
        (
            &[
                "--url",
                "pixpin://pinear?ruta=%5C%5Catacante.example%5Cc%5Cx.png",
            ],
            6,
        ),
        (
            &[
                "--url",
                "pixpin://pedido?json=%7B%22pixpin%22%3A1%2C%22accion%22%3A%22automatizacion%22%2C%22operacion%22%3A%22crear%22%7D",
            ],
            6,
        ),
        // Lo que pide confirmar, sin cuadros, se contesta «no».
        (
            &[
                "--url",
                "pixpin://pedido?json=%7B%22pixpin%22%3A1%2C%22accion%22%3A%22borrar_lista%22%2C%22codigo%22%3A%221%22%7D",
            ],
            6,
        ),
        (&["pinear", "C:\\no\\existe\\x.png"], 1),
    ];
    for (args, esperado) in casos {
        let (c, r, err) = a_la_prueba(args);
        assert_eq!(c, esperado, "{args:?}: {err}");
        assert!(r.is_empty(), "{args:?} no debia llegar: {r:?}");
        assert!(!err.trim().is_empty(), "{args:?}: deberia decir por que");
    }
}

#[test]
#[ignore = "lanza pixpinmax.exe; ver la cabecera"]
fn caso_negativo_sin_app_sale_con_cuatro_y_no_la_abre() {
    let inicio = Instant::now();
    let (s, _) = orden(
        &format!("PixPinNoExisteNunca{}", std::process::id()),
        &["capturar"],
        None,
    );
    assert_eq!(s.status.code(), Some(4));
    // Sin esperar los 8 s del arranque: con la ventana de prueba no se abre nada.
    assert!(
        inicio.elapsed() < Duration::from_secs(3),
        "{:?}",
        inicio.elapsed()
    );
}

#[test]
#[ignore = "lanza pixpinmax.exe; ver la cabecera"]
fn ayuda_y_ocr_escriben_en_la_salida() {
    let (s, _) = orden(clase(), &["ayuda"], None);
    assert_eq!(s.status.code(), Some(0));
    let ayuda = String::from_utf8_lossy(&s.stdout);
    assert!(
        ayuda.contains("Verbos:") && ayuda.contains("pinear"),
        "{ayuda}"
    );
    // Una captura de la documentacion, que tiene texto.
    let origen = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/img/ajustes-herramientas.png"
    );
    let dir = std::env::temp_dir().join(format!("pixpin-orden-ocr-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let copia = dir.join("captura.png");
    std::fs::copy(origen, &copia).unwrap();
    let (s, t) = orden(clase(), &["ocr", copia.to_str().unwrap()], None);
    let texto = String::from_utf8_lossy(&s.stdout);
    println!(
        "ocr en {t:?}: {} letras; empieza: {:?}",
        texto.chars().count(),
        texto.chars().take(60).collect::<String>()
    );
    assert_eq!(
        s.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&s.stderr)
    );
    assert!(texto.trim().chars().count() > 10, "{texto}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "lanza pixpinmax.exe; ver la cabecera"]
fn las_copias_del_explorador_se_juntan_en_un_pedido() {
    let dir = std::env::temp_dir().join(format!("pixpin-orden-juntar-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rutas: Vec<String> = (0..3)
        .map(|i| {
            let r = dir.join(format!("f{i}.png"));
            std::fs::write(&r, b"x").unwrap();
            r.to_string_lossy().into_owned()
        })
        .collect();
    RECIBIDOS.lock().unwrap().clear();
    let clase = clase().to_string();
    // Como el Explorador: una copia por fichero, casi a la vez.
    let hijos: Vec<_> = rutas
        .iter()
        .map(|r| {
            Command::new(env!("CARGO_BIN_EXE_pixpinmax"))
                .args(["--juntar", "chat", r])
                .env("PIXPIN_PEDIDOS_VENTANA", &clase)
                .env("PIXPIN_ORDEN_SIN_CUADROS", "1")
                .env("PIXPIN_INSTANCIA", "orden-prueba")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap()
        })
        .collect();
    for mut h in hijos {
        assert_eq!(h.wait().unwrap().code(), Some(0));
    }
    let r = std::mem::take(&mut *RECIBIDOS.lock().unwrap());
    assert_eq!(r.len(), 1, "{r:?}");
    let mut imagenes: Vec<String> = valor(&r[0])["imagenes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    imagenes.sort();
    assert_eq!(imagenes, rutas);
    let _ = std::fs::remove_dir_all(&dir);
}

/// **Cuanto tarda** una orden de punta a punta: de lanzar el proceso a que
/// sale con «aceptado». La mediana de varias, para que una vuelta lenta del
/// disco no cuente.
#[test]
#[ignore = "lanza pixpinmax.exe; ver la cabecera"]
fn medir_el_tiempo_hasta_aceptado() {
    let mut tiempos = Vec::new();
    let mut solo = Vec::new();
    for _ in 0..15 {
        let (s, t) = orden(clase(), &["capturar"], None);
        assert_eq!(s.status.code(), Some(0));
        tiempos.push(t);
        let (s, t) = orden(clase(), &["ayuda"], None);
        assert_eq!(s.status.code(), Some(0));
        solo.push(t);
    }
    tiempos.sort();
    solo.sort();
    println!(
        "pixpinmax capturar -> aceptado: min {:?}, mediana {:?}, max {:?}; pixpinmax ayuda (solo arrancar y salir): mediana {:?}",
        tiempos[0], tiempos[7], tiempos[14], solo[7]
    );
    assert!(tiempos[7] < Duration::from_millis(500), "{:?}", tiempos[7]);
}
