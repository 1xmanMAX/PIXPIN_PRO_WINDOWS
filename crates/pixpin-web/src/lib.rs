//! pixpin-web — ver y editar un `.html` dentro de PixPin, con WebView2.
//!
//! Es el puerto de `ui/VisorHtmlActivity.kt` de PixPin Android. El chat genera
//! ficheros HTML que se editan solos (una tabla, una ficha, una lista) y que
//! traen su propio boton de guardar. Hasta ahora en el escritorio esos
//! ficheros se abrian en el navegador del sistema, y guardar significaba
//! descargar una copia a `Descargas` que el usuario tenia que mover a mano.
//! Aqui la pagina se abre dentro y el guardado cae **encima del mismo
//! fichero**.
//!
//! Se capturan los dos guardados posibles, porque las paginas vienen de dos
//! epocas distintas:
//!
//! 1. Las que conocen el puente llaman a `window.PixPinVisor.guardar(nombre,
//!    base64)`. Es el camino limpio: llega el HTML entero en un mensaje.
//! 2. Las que no lo conocen hacen lo de siempre, un `<a download>` o un
//!    `Blob`. Ese camino se intercepta en `DownloadStarting`: la descarga se
//!    desvia a un fichero temporal nuestro y el aviso del navegador se cancela.
//!
//! Los dos pasan por el mismo filtro de [`validacion`] antes de tocar nada.
//!
//! Este crate habla con COM y con el sistema operativo, asi que hay `unsafe`;
//! cada bloque lleva su `// SAFETY:`. No depende de ningun otro crate de
//! PixPin salvo `pixpin-geom`: es capa L1, un cimiento como `pixpin-audio`.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod base64;
pub mod puente;
pub mod ruta;
pub mod validacion;

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use pixpin_geom::Rect;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED, CreateCoreWebView2EnvironmentWithOptions,
    GetAvailableCoreWebView2BrowserVersionString, ICoreWebView2, ICoreWebView2_4,
    ICoreWebView2Controller, ICoreWebView2Environment, ICoreWebView2EnvironmentOptions,
};
use webview2_com::{
    AddScriptToExecuteOnDocumentCreatedCompletedHandler, CoTaskMemPWSTR,
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    DownloadStartingEventHandler, StateChangedEventHandler, WebMessageReceivedEventHandler,
    take_pwstr,
};
use windows::Win32::Foundation::{E_POINTER, HWND, RECT};
use windows::core::{Interface, PCWSTR, PWSTR};

/// Lo que puede salir mal al montar el visor.
#[derive(Debug, thiserror::Error)]
pub enum ErrorWeb {
    /// No esta el runtime de WebView2 en el equipo. Quien llama debe abrir el
    /// html en el navegador del sistema, como se hacia antes.
    #[error("no esta instalado el runtime de WebView2")]
    SinRuntime,
    /// Fallo creando el entorno o el controlador de WebView2.
    #[error("no se pudo crear el WebView2: {0}")]
    Creacion(String),
    /// El WebView2 existe pero no pudo abrir el fichero.
    #[error("no se pudo abrir el html: {0}")]
    Navegacion(String),
}

/// La funcion que recibe el HTML nuevo, compartida por los dos caminos de
/// guardado. Es `Rc` y no `Arc` a proposito: WebView2 llama siempre desde el
/// hilo de interfaz, asi que un contador atomico solo seria coste.
type AlGuardar = Rc<dyn Fn(Vec<u8>)>;

/// Un WebView2 empotrado en una ventana de PixPin, mostrando un `.html`.
pub struct VisorHtml {
    controlador: ICoreWebView2Controller,
    /// La carpeta propia donde aterrizan las descargas interceptadas y los
    /// datos de usuario del navegador. Se borra al cerrar.
    temporal: PathBuf,
}

impl VisorHtml {
    /// Crea el WebView2 dentro de `padre`, ocupando `area` (pixeles cliente),
    /// y abre `ruta`. `al_guardar` recibe los BYTES del html nuevo; lo llama
    /// desde el hilo de interfaz.
    /// `al_cerrar` se llama cuando el usuario pulsa Escape DENTRO de la
    /// pagina. Hace falta porque el WebView2 se queda con el teclado: sin
    /// esto, Escape no llega a la ventana que lo contiene y el visor no se
    /// puede cerrar mientras se escribe en la pagina.
    pub fn nuevo(
        padre: HWND,
        area: Rect,
        ruta: &Path,
        al_guardar: Box<dyn Fn(Vec<u8>) + 'static>,
        al_cerrar: Box<dyn Fn() + 'static>,
    ) -> Result<VisorHtml, ErrorWeb> {
        if !Self::hay_runtime() {
            return Err(ErrorWeb::SinRuntime);
        }
        let al_guardar: AlGuardar = Rc::from(al_guardar);

        let temporal = carpeta_temporal();
        std::fs::create_dir_all(&temporal)
            .map_err(|e| ErrorWeb::Creacion(format!("no se pudo crear {temporal:?}: {e}")))?;

        let entorno = crear_entorno(&temporal)?;
        let controlador = crear_controlador(&entorno, padre)?;

        // SAFETY: `controlador` acaba de llegar de WebView2 y es valido; el
        // RECT se construye aqui y vive mas que la llamada.
        let vista = unsafe {
            controlador
                .SetBounds(a_rect(area))
                .and_then(|()| controlador.SetIsVisible(true))
                .and_then(|()| controlador.CoreWebView2())
        }
        .map_err(|e| ErrorWeb::Creacion(e.to_string()))?;

        conectar_puente(&vista, al_guardar.clone())?;
        conectar_descargas(&vista, al_guardar, temporal.clone())?;
        conectar_escape(&controlador, Rc::from(al_cerrar))?;

        let url = CoTaskMemPWSTR::from(ruta::url_de_fichero(ruta).as_str());
        // SAFETY: `url` mantiene viva la cadena durante toda la llamada, que
        // es lo unico que `Navigate` necesita: WebView2 la copia.
        unsafe { vista.Navigate(*url.as_ref().as_pcwstr()) }
            .map_err(|e| ErrorWeb::Navegacion(e.to_string()))?;

        Ok(VisorHtml {
            controlador,
            temporal,
        })
    }

    /// Recoloca el WebView2 dentro de la ventana padre.
    pub fn redimensionar(&self, area: Rect) {
        // SAFETY: `self.controlador` es valido mientras viva `self`, y se
        // libera en `Drop`.
        if let Err(e) = unsafe { self.controlador.SetBounds(a_rect(area)) } {
            // No se propaga: redimensionar se llama desde `WM_SIZE`, donde no
            // hay a quien devolverle el error, y un visor mal colocado es
            // mucho menos grave que tirar la ventana entera.
            tracing::warn!(error = %e, "no se pudo recolocar el WebView2");
        }
    }

    /// Si el runtime de WebView2 esta instalado. Sin el, quien llama abre el
    /// html en el navegador como hasta ahora.
    pub fn hay_runtime() -> bool {
        let mut version = PWSTR::null();
        // SAFETY: se pasa una carpeta nula para que busque la instalacion del
        // sistema, y `version` es un puntero valido a escribir. Si devuelve
        // error no escribe nada, y si acierta la cadena es de `CoTaskMemAlloc`
        // y se libera justo debajo con `take_pwstr`.
        let encontrado =
            unsafe { GetAvailableCoreWebView2BrowserVersionString(PCWSTR::null(), &mut version) }
                .is_ok();
        if !version.is_null() {
            let _ = take_pwstr(version);
        }
        encontrado
    }
}

impl Drop for VisorHtml {
    fn drop(&mut self) {
        // SAFETY: `controlador` sigue siendo valido aqui; `Close` es lo que
        // apaga el proceso del navegador. Sin el, el WebView2 sobrevive a la
        // ventana y se queda un proceso huerfano por cada html abierto.
        if let Err(e) = unsafe { self.controlador.Close() } {
            tracing::warn!(error = %e, "el WebView2 no cerro limpiamente");
        }
        // Las descargas interceptadas ya se entregaron en memoria: lo que
        // quede aqui es basura y no debe sobrevivir a la sesion.
        let _ = std::fs::remove_dir_all(&self.temporal);
    }
}

/// Carpeta propia bajo el temporal del sistema, distinta por visor.
fn carpeta_temporal() -> PathBuf {
    static CONTADOR: AtomicU64 = AtomicU64::new(0);
    let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("pixpin-web-{}-{n}", std::process::id()))
}

/// `Rect` de PixPin a `RECT` de Windows.
///
/// Los dos usan media apertura, asi que la derecha y el abajo de `Rect` ya son
/// el primer pixel de fuera, que es justo lo que espera `SetBounds`.
fn a_rect(area: Rect) -> RECT {
    RECT {
        left: area.izquierda(),
        top: area.arriba(),
        right: area.derecha(),
        bottom: area.abajo(),
    }
}

/// Crea el entorno de WebView2, bombeando mensajes hasta que responde.
///
/// La carpeta de datos de usuario se fuerza al temporal nuestro porque la de
/// serie va al lado del ejecutable, y PixPin puede estar instalado en
/// `Program Files`, donde no se escribe.
fn crear_entorno(temporal: &Path) -> Result<ICoreWebView2Environment, ErrorWeb> {
    let datos = CoTaskMemPWSTR::from(temporal.join("datos").to_string_lossy().as_ref());
    let (tx, rx) = std::sync::mpsc::channel();

    CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
        Box::new(move |manejador| {
            // SAFETY: `datos` vive hasta el final de este closure y WebView2
            // copia la cadena antes de volver; `manejador` lo crea el propio
            // `wait_for_async_operation`.
            unsafe {
                CreateCoreWebView2EnvironmentWithOptions(
                    PCWSTR::null(),
                    *datos.as_ref().as_pcwstr(),
                    None::<&ICoreWebView2EnvironmentOptions>,
                    &manejador,
                )
            }
            .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |codigo, entorno| {
            codigo?;
            tx.send(entorno.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                .expect("el canal del entorno sigue abierto");
            Ok(())
        }),
    )
    .map_err(|e| ErrorWeb::Creacion(format!("{e:?}")))?;

    rx.recv()
        .map_err(|_| ErrorWeb::Creacion("el entorno nunca contesto".into()))?
        .map_err(|e| ErrorWeb::Creacion(e.to_string()))
}

/// Crea el controlador (la ventana hija de verdad) dentro de `padre`.
fn crear_controlador(
    entorno: &ICoreWebView2Environment,
    padre: HWND,
) -> Result<ICoreWebView2Controller, ErrorWeb> {
    let entorno = entorno.clone();
    let (tx, rx) = std::sync::mpsc::channel();

    CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
        Box::new(move |manejador| {
            // SAFETY: `padre` lo da quien llama y debe ser una ventana viva;
            // `entorno` es el que acaba de crear `crear_entorno`.
            unsafe { entorno.CreateCoreWebView2Controller(padre, &manejador) }
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |codigo, controlador| {
            codigo?;
            tx.send(controlador.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                .expect("el canal del controlador sigue abierto");
            Ok(())
        }),
    )
    .map_err(|e| ErrorWeb::Creacion(format!("{e:?}")))?;

    rx.recv()
        .map_err(|_| ErrorWeb::Creacion("el controlador nunca contesto".into()))?
        .map_err(|e| ErrorWeb::Creacion(e.to_string()))
}

/// Escape, tambien con el foco DENTRO de la pagina.
///
/// `add_AcceleratorKeyPressed` es del CONTROLADOR y no de la vista: llega
/// antes de que la pagina vea la tecla, que es lo unico que funciona aqui.
/// Mientras se escribe en un campo del html, el WebView2 se queda el teclado
/// y la ventana de PixPin no recibe nada; sin esto el visor solo se cerraria
/// con Alt+F4.
fn conectar_escape(
    controlador: &ICoreWebView2Controller,
    al_cerrar: Rc<dyn Fn()>,
) -> Result<(), ErrorWeb> {
    use webview2_com::AcceleratorKeyPressedEventHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN;

    const VK_ESCAPE: u32 = 0x1B;
    let mut token = 0i64;
    let manejador = AcceleratorKeyPressedEventHandler::create(Box::new(move |_c, args| {
        let Some(args) = args else { return Ok(()) };
        let (mut clase, mut tecla) = (Default::default(), 0u32);
        // SAFETY: `args` lo da WebView2 y vive durante el evento; las dos
        // salidas son variables locales.
        unsafe {
            args.KeyEventKind(&mut clase)?;
            args.VirtualKey(&mut tecla)?;
        }
        if clase == COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN && tecla == VK_ESCAPE {
            // Tragada: la pagina no tiene por que enterarse de que se cierra
            // su visor, y si la usara para otra cosa haria las dos.
            // SAFETY: propiedad del propio evento.
            unsafe {
                args.SetHandled(true)?;
            }
            al_cerrar();
        }
        Ok(())
    }));
    // SAFETY: `token` es un i64 valido a escribir; el manejador queda
    // referenciado por el propio WebView2.
    unsafe { controlador.add_AcceleratorKeyPressed(&manejador, &mut token) }
        .map_err(|e| ErrorWeb::Creacion(e.to_string()))?;
    Ok(())
}

/// Camino 1: inyecta `window.PixPinVisor` y escucha sus mensajes.
fn conectar_puente(vista: &ICoreWebView2, al_guardar: AlGuardar) -> Result<(), ErrorWeb> {
    let script = CoTaskMemPWSTR::from(puente::SCRIPT);
    let mut token = 0i64;

    // SAFETY: `vista` acaba de salir del controlador y sigue viva. Sin
    // `IsWebMessageEnabled` el puente entero seria mudo: `postMessage` no
    // llega.
    unsafe {
        let ajustes = vista
            .Settings()
            .map_err(|e| ErrorWeb::Creacion(e.to_string()))?;
        ajustes
            .SetIsWebMessageEnabled(true)
            .map_err(|e| ErrorWeb::Creacion(e.to_string()))?;
    }

    let manejador_script =
        AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(|codigo, _id| {
            if let Err(e) = codigo {
                tracing::warn!(error = %e, "no se pudo inyectar PixPinVisor");
            }
            Ok(())
        }));
    // SAFETY: `script` vive hasta el final de la funcion y WebView2 copia la
    // cadena antes de volver.
    unsafe {
        vista.AddScriptToExecuteOnDocumentCreated(*script.as_ref().as_pcwstr(), &manejador_script)
    }
    .map_err(|e| ErrorWeb::Creacion(e.to_string()))?;

    let manejador_mensaje =
        WebMessageReceivedEventHandler::create(Box::new(move |_vista, args| {
            if let Some(args) = args {
                let mut mensaje = PWSTR::null();
                // SAFETY: `args` lo da WebView2 y vive durante el evento; la
                // cadena que escribe se libera con `take_pwstr`.
                if unsafe { args.TryGetWebMessageAsString(&mut mensaje) }.is_ok() {
                    atender_mensaje(&take_pwstr(mensaje), &al_guardar);
                }
            }
            Ok(())
        }));
    // SAFETY: `token` es un i64 valido a escribir y el manejador queda
    // referenciado por el propio WebView2.
    unsafe { vista.add_WebMessageReceived(&manejador_mensaje, &mut token) }
        .map_err(|e| ErrorWeb::Creacion(e.to_string()))?;

    Ok(())
}

/// Descodifica y valida lo que llego por el puente.
fn atender_mensaje(mensaje: &str, al_guardar: &AlGuardar) {
    if !puente::es_peticion_de_guardado(mensaje) {
        return;
    }
    let Some(datos) = puente::campo_de_texto(mensaje, "datos") else {
        tracing::warn!("PixPinVisor.guardar llego sin campo `datos`");
        return;
    };
    let Some(bytes) = base64::descodificar(&datos) else {
        tracing::warn!("PixPinVisor.guardar mando un base64 que no se entiende");
        return;
    };
    entregar(bytes, al_guardar, "puente");
}

/// Camino 2: desvia las descargas a un fichero nuestro y las recoge al acabar.
fn conectar_descargas(
    vista: &ICoreWebView2,
    al_guardar: AlGuardar,
    temporal: PathBuf,
) -> Result<(), ErrorWeb> {
    // `DownloadStarting` solo existe a partir de `ICoreWebView2_4`. Si el
    // runtime es mas viejo no se cae el visor entero: se queda sin el segundo
    // camino, que es exactamente lo que se podia hacer antes.
    let Ok(vista4) = vista.cast::<ICoreWebView2_4>() else {
        tracing::warn!(
            "el runtime de WebView2 es anterior a ICoreWebView2_4: sin interceptar descargas"
        );
        return Ok(());
    };

    let mut token = 0i64;
    // SAFETY: `vista4` es la misma interfaz consultada arriba, y `token` es un
    // i64 valido a escribir. El manejador se queda vivo porque WebView2 le
    // sube el contador de referencias.
    unsafe {
        vista4.add_DownloadStarting(
            &DownloadStartingEventHandler::create(Box::new(move |_vista, args| {
                if let Some(args) = args {
                    desviar_descarga(&args, &al_guardar, &temporal);
                }
                Ok(())
            })),
            &mut token,
        )
    }
    .map_err(|e| ErrorWeb::Creacion(e.to_string()))?;
    Ok(())
}

/// Manda una descarga a un fichero temporal propio y se suscribe a su final.
fn desviar_descarga(
    args: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2DownloadStartingEventArgs,
    al_guardar: &AlGuardar,
    temporal: &Path,
) {
    static CONTADOR: AtomicU64 = AtomicU64::new(0);
    // Solo se desvia la propia pagina (su boton de guardar baja un `.html`).
    // Lo demas —el CSV de una tabla, «Bajar CSV»— es una descarga de verdad:
    // se deja a WebView2, que la pone en Descargas con su aviso. Antes se
    // desviaba todo y lo que no era la pagina se tiraba en silencio.
    let mut sugerida = PWSTR::null();
    // SAFETY: `args` lo da WebView2 y vive durante el evento; la cadena que
    // escribe se libera con `take_pwstr`.
    if unsafe { args.ResultFilePath(&mut sugerida) }.is_ok() {
        let sugerida = take_pwstr(sugerida);
        if !es_la_pagina(&sugerida) {
            tracing::info!(%sugerida, "descarga que no es la pagina: la hace WebView2");
            return;
        }
    }
    let destino = temporal.join(format!(
        "descarga-{}.html",
        CONTADOR.fetch_add(1, Ordering::Relaxed)
    ));
    let destino_pcwstr = CoTaskMemPWSTR::from(destino.to_string_lossy().as_ref());
    let al_guardar = al_guardar.clone();

    // SAFETY: `args` lo da WebView2 y es valido durante el evento; la cadena
    // del destino vive hasta el final de esta funcion y WebView2 la copia.
    let resultado = unsafe {
        args.SetResultFilePath(*destino_pcwstr.as_ref().as_pcwstr())
            // `Handled` a true quita la barra de descargas del navegador: para
            // el usuario esto no es una descarga, es su boton de guardar.
            .and_then(|()| args.SetHandled(true))
            .and_then(|()| args.DownloadOperation())
    };
    let operacion = match resultado {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!(error = %e, "no se pudo desviar la descarga");
            return;
        }
    };

    let mut token = 0i64;
    // SAFETY: `operacion` viene de `args` y WebView2 la mantiene viva mientras
    // la descarga exista; el manejador queda referenciado por ella.
    let alta = unsafe {
        operacion.add_StateChanged(
            &StateChangedEventHandler::create(Box::new(move |operacion, _| {
                if let Some(operacion) = operacion {
                    recoger_descarga(&operacion, &al_guardar);
                }
                Ok(())
            })),
            &mut token,
        )
    };
    if let Err(e) = alta {
        tracing::warn!(error = %e, "no se pudo seguir el estado de la descarga");
    }
}

/// Si lo que se baja es una pagina (`.html`/`.htm`, por el nombre que
/// sugiere el navegador): lo unico que puede ser el boton de guardar.
fn es_la_pagina(ruta_sugerida: &str) -> bool {
    Path::new(ruta_sugerida)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"))
}

/// Si la descarga termino, lee el fichero temporal y lo entrega.
fn recoger_descarga(
    operacion: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2DownloadOperation,
    al_guardar: &AlGuardar,
) {
    let mut estado = COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED;
    let mut ruta_pwstr = PWSTR::null();
    // SAFETY: `operacion` la da WebView2 y vive durante el evento; los dos
    // punteros son a variables locales validas y la cadena se libera abajo.
    let leido = unsafe {
        operacion
            .State(&mut estado)
            .and_then(|()| operacion.ResultFilePath(&mut ruta_pwstr))
    };
    if leido.is_err() {
        if !ruta_pwstr.is_null() {
            let _ = take_pwstr(ruta_pwstr);
        }
        return;
    }
    let ruta = PathBuf::from(take_pwstr(ruta_pwstr));

    // Los estados intermedios (en curso, pausada) llegan por el mismo evento.
    if estado != COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED {
        return;
    }

    match std::fs::read(&ruta) {
        Ok(bytes) => {
            entregar(bytes, al_guardar, "descarga");
            // El temporal ya cumplio: quien guarda de verdad es quien llama.
            let _ = std::fs::remove_file(&ruta);
        }
        Err(e) => tracing::warn!(error = %e, ?ruta, "no se pudo leer la descarga interceptada"),
    }
}

/// Unico sitio por el que se llama a `al_guardar`, con el filtro delante.
///
/// Que sea uno solo es la gracia: los dos caminos pueden traer cualquier cosa
/// y el fichero del usuario esta al otro lado.
fn entregar(bytes: Vec<u8>, al_guardar: &AlGuardar, origen: &str) {
    if !validacion::parece_html_completo(&bytes) {
        tracing::warn!(
            origen,
            bytes = bytes.len(),
            "no parece la pagina: no se guarda"
        );
        return;
    }
    al_guardar(bytes);
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::cell::RefCell;

    /// Recoge lo que se entregaria al usuario, sin ventana ni navegador.
    fn espia() -> (AlGuardar, Rc<RefCell<Vec<Vec<u8>>>>) {
        let caja = Rc::new(RefCell::new(Vec::new()));
        let copia = caja.clone();
        let f: AlGuardar = Rc::new(move |bytes| copia.borrow_mut().push(bytes));
        (f, caja)
    }

    fn html_bueno() -> String {
        format!("<html><body>{}</body></html>", "a".repeat(400))
    }

    fn en_base64(texto: &str) -> String {
        const ALFABETO: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let bytes = texto.as_bytes();
        let mut salida = String::new();
        for trozo in bytes.chunks(3) {
            let mut bloque = [0u8; 3];
            bloque[..trozo.len()].copy_from_slice(trozo);
            let n =
                (u32::from(bloque[0]) << 16) | (u32::from(bloque[1]) << 8) | u32::from(bloque[2]);
            for i in 0..4 {
                if i <= trozo.len() {
                    salida.push(ALFABETO[((n >> (18 - 6 * i)) & 0x3F) as usize] as char);
                } else {
                    salida.push('=');
                }
            }
        }
        salida
    }

    #[test]
    fn solo_se_desvia_la_descarga_de_la_propia_pagina() {
        assert!(es_la_pagina(r"C:\Users\x\Downloads\Obra.html"));
        assert!(es_la_pagina(r"C:\Users\x\Downloads\Obra (anotado).HTM"));
        // Casos negativos: el CSV de una tabla y lo que no tiene extension
        // son descargas de verdad y se dejan a WebView2.
        assert!(!es_la_pagina(r"C:\Users\x\Downloads\Gastos.csv"));
        assert!(!es_la_pagina(r"C:\Users\x\Downloads\descarga"));
        assert!(!es_la_pagina(""));
    }

    #[test]
    fn un_guardado_bueno_del_puente_llega_entero() {
        let (f, caja) = espia();
        let mensaje = format!(
            r#"{{"tipo":"guardar","nombre":"n.html","datos":"{}"}}"#,
            en_base64(&html_bueno())
        );
        atender_mensaje(&mensaje, &f);
        assert_eq!(caja.borrow().len(), 1);
        assert_eq!(caja.borrow()[0], html_bueno().into_bytes());
    }

    #[test]
    fn un_guardado_demasiado_corto_no_toca_el_fichero() {
        let (f, caja) = espia();
        let mensaje = format!(
            r#"{{"tipo":"guardar","datos":"{}"}}"#,
            en_base64("<html></html>")
        );
        atender_mensaje(&mensaje, &f);
        assert!(caja.borrow().is_empty());
    }

    #[test]
    fn un_guardado_sin_el_cierre_de_html_no_toca_el_fichero() {
        let (f, caja) = espia();
        let mensaje = format!(
            r#"{{"tipo":"guardar","datos":"{}"}}"#,
            en_base64(&"z".repeat(1000))
        );
        atender_mensaje(&mensaje, &f);
        assert!(caja.borrow().is_empty());
    }

    #[test]
    fn un_base64_roto_no_toca_el_fichero() {
        let (f, caja) = espia();
        atender_mensaje(r#"{"tipo":"guardar","datos":"no~es~base64"}"#, &f);
        assert!(caja.borrow().is_empty());
    }

    #[test]
    fn un_mensaje_que_no_es_de_guardar_se_ignora() {
        let (f, caja) = espia();
        let mensaje = format!(
            r#"{{"tipo":"otra cosa","datos":"{}"}}"#,
            en_base64(&html_bueno())
        );
        atender_mensaje(&mensaje, &f);
        assert!(caja.borrow().is_empty());
    }

    #[test]
    fn el_area_se_traduce_a_un_rect_de_windows() {
        let r = a_rect(Rect {
            x: 4,
            y: 8,
            ancho: 100,
            alto: 50,
        });
        assert_eq!((r.left, r.top, r.right, r.bottom), (4, 8, 104, 58));
    }

    #[test]
    fn cada_visor_pide_su_propia_carpeta_temporal() {
        assert_ne!(carpeta_temporal(), carpeta_temporal());
    }

    #[test]
    fn preguntar_por_el_runtime_no_revienta() {
        // No se afirma el valor: depende de si este equipo tiene WebView2.
        // Lo que prueba es que el enlace con `webview2loader` funciona.
        let _ = VisorHtml::hay_runtime();
    }

    #[test]
    #[ignore = "necesita una ventana de verdad y el runtime de WebView2"]
    fn abre_un_html_dentro_de_una_ventana() {
        // Pendiente: crear un HWND, llamar a `VisorHtml::nuevo` con un html de
        // prueba y comprobar que el boton de guardar de la pagina llega a
        // `al_guardar`. Sin escritorio no se puede.
    }
}
