//! **El POST con WinHTTP**, lo que ya trae Windows (como
//! `pixpin_ia::nube::http`): el TLS y el proxy del sistema, sin cliente HTTP
//! propio en el ejecutable. Aqui todo es corto: Jev contesta en 70-500 ms y
//! quien llama da el tiempo (unos 2 s), que vale para conectar, mandar y
//! recibir.
//!
//! **La sesion con cada servidor se guarda abierta** para la siguiente
//! peticion (29-oct-2026, los temas en vivo: una peticion cada pocos
//! segundos): con la misma sesion WinHTTP reusa la conexion TLS ya hecha
//! (keep-alive), sin volver a resolver, conectar ni negociar el TLS. Tras
//! un fallo de red se olvida y la siguiente abre otra.
//!
//! Solo `https://`, salvo hacia `127.0.0.1`/`localhost` (un servidor de
//! pruebas): la clave nunca sale sin cifrar a la red.
//!
//! Este modulo habla con el sistema operativo: cada `unsafe` lleva su
//! `// SAFETY:`.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use windows::Win32::Networking::WinHttp::{
    WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
    WINHTTP_ACCESS_TYPE_NO_PROXY, WINHTTP_FLAG_SECURE, WINHTTP_OPEN_REQUEST_FLAGS,
    WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE, WinHttpCloseHandle, WinHttpConnect,
    WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse,
    WinHttpSendRequest, WinHttpSetTimeouts,
};
use windows::core::{HSTRING, PCWSTR, w};

use super::{Contestado, Transporte};

/// Lo mas que se lee de una respuesta (las de Jev son de unos KB).
const LIMITE: usize = 4 * 1024 * 1024;

/// El transporte de Windows.
pub struct WinHttp;

/// Un asa de WinHTTP que se cierra sola.
struct Asa(*mut c_void);

impl Drop for Asa {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: el asa la devolvio WinHTTP, es de este `Asa` y no se
            // ha cerrado antes.
            let _ = unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

// SAFETY: un asa de sesion o de conexion de WinHTTP se puede usar desde
// cualquier hilo (WinHTTP las protege por dentro); solo se cierra al soltar
// el ultimo `Arc` que la guarda, y nadie cambia el puntero despues.
unsafe impl Send for Asa {}
// SAFETY: lo mismo.
unsafe impl Sync for Asa {}

/// La sesion y la conexion abiertas con un servidor.
struct Abierta {
    // El orden importa: la conexion se cierra antes que la sesion.
    conexion: Asa,
    _sesion: Asa,
}

/// (segura, servidor, puerto).
type Clave = (bool, String, u16);

fn abiertas() -> &'static Mutex<HashMap<Clave, Arc<Abierta>>> {
    static A: OnceLock<Mutex<HashMap<Clave, Arc<Abierta>>>> = OnceLock::new();
    A.get_or_init(|| Mutex::new(HashMap::new()))
}

/// La sesion y conexion con `servidor`: la guardada o una nueva.
fn abrir(segura: bool, servidor: &str, puerto: u16) -> Result<Arc<Abierta>, String> {
    let clave = (segura, servidor.to_string(), puerto);
    let mut g = abiertas().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(a) = g.get(&clave) {
        return Ok(Arc::clone(a));
    }
    let error = |que: &str| format!("{que}: {}", windows::core::Error::from_thread().message());
    // SAFETY: las cadenas son literales o `HSTRING` vivos durante cada
    // llamada; cada asa se envuelve en `Asa` en cuanto existe, asi que se
    // cierra en todos los caminos.
    let a = unsafe {
        let mut sesion = Asa(WinHttpOpen(
            w!("PixPin"),
            if segura {
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY
            } else {
                WINHTTP_ACCESS_TYPE_NO_PROXY
            },
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ));
        if sesion.0.is_null() && segura {
            sesion = Asa(WinHttpOpen(
                w!("PixPin"),
                WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            ));
        }
        if sesion.0.is_null() {
            return Err(error("sin conexion"));
        }
        let conexion = Asa(WinHttpConnect(
            sesion.0,
            &HSTRING::from(servidor),
            puerto,
            0,
        ));
        if conexion.0.is_null() {
            return Err(error("no se pudo conectar"));
        }
        Arc::new(Abierta {
            conexion,
            _sesion: sesion,
        })
    };
    g.insert(clave, Arc::clone(&a));
    Ok(a)
}

/// Tras un fallo de red, la sesion guardada no se vuelve a usar.
fn olvidar(segura: bool, servidor: &str, puerto: u16) {
    abiertas()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&(segura, servidor.to_string(), puerto));
}

/// `https://servidor[:puerto]/ruta` partido: (segura, servidor, puerto, ruta).
fn partir(url: &str) -> Option<(bool, String, u16, String)> {
    let (segura, resto) = if let Some(r) = url.strip_prefix("https://") {
        (true, r)
    } else {
        (false, url.strip_prefix("http://")?)
    };
    let (anfitrion, ruta) = match resto.find('/') {
        Some(i) => (&resto[..i], &resto[i..]),
        None => (resto, "/"),
    };
    let (servidor, puerto) = match anfitrion.rsplit_once(':') {
        Some((s, p)) => (s, p.parse::<u16>().ok()?),
        None => (anfitrion, if segura { 443 } else { 80 }),
    };
    if servidor.is_empty() || (!segura && !matches!(servidor, "127.0.0.1" | "localhost")) {
        return None;
    }
    Some((segura, servidor.to_string(), puerto, ruta.to_string()))
}

impl Transporte for WinHttp {
    fn enviar(
        &self,
        metodo: &str,
        url: &str,
        cabeceras: &[(&'static str, String)],
        cuerpo: &[u8],
        tiempo: Duration,
    ) -> Result<Contestado, String> {
        let (segura, servidor, puerto, ruta) =
            partir(url).ok_or_else(|| format!("direccion no valida: {url}"))?;
        let abierta = abrir(segura, &servidor, puerto)?;
        let r = enviar_por(&abierta, segura, &ruta, metodo, cabeceras, cuerpo, tiempo);
        if r.is_err() {
            olvidar(segura, &servidor, puerto);
        }
        r
    }
}

/// Una peticion por una conexion ya abierta.
fn enviar_por(
    abierta: &Abierta,
    segura: bool,
    ruta: &str,
    metodo: &str,
    cabeceras: &[(&'static str, String)],
    cuerpo: &[u8],
    tiempo: Duration,
) -> Result<Contestado, String> {
    let mut extra = String::new();
    for (k, v) in cabeceras {
        extra.push_str(k);
        extra.push_str(": ");
        extra.push_str(v);
        extra.push_str("\r\n");
    }
    let extra = HSTRING::from(extra);
    let ms = tiempo.as_millis().clamp(200, 60_000) as i32;
    let error = |que: &str| format!("{que}: {}", windows::core::Error::from_thread().message());
    if cuerpo.len() > u32::MAX as usize {
        return Err("la peticion es demasiado grande".into());
    }
    // SAFETY: las cadenas son literales o `HSTRING` vivos durante cada
    // llamada; la conexion vive en `abierta` mas que esta funcion, y el asa
    // de la peticion se envuelve en `Asa` en cuanto existe, asi que se
    // cierra en todos los caminos. Los bufferes que se pasan a WinHTTP
    // viven mientras dura cada llamada, que es sincrona.
    unsafe {
        let peticion = Asa(WinHttpOpenRequest(
            abierta.conexion.0,
            &HSTRING::from(metodo),
            &HSTRING::from(ruta),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_OPEN_REQUEST_FLAGS(if segura { WINHTTP_FLAG_SECURE.0 } else { 0 }),
        ));
        if peticion.0.is_null() {
            return Err(error("no se pudo preparar la peticion"));
        }
        // El tiempo es de esta peticion (la sesion es de todas).
        let _ = WinHttpSetTimeouts(peticion.0, ms, ms, ms, ms);
        WinHttpSendRequest(
            peticion.0,
            Some(&extra),
            (!cuerpo.is_empty()).then(|| cuerpo.as_ptr().cast()),
            cuerpo.len() as u32,
            cuerpo.len() as u32,
            0,
        )
        .map_err(|e| format!("no se pudo mandar: {}", e.message()))?;
        WinHttpReceiveResponse(peticion.0, std::ptr::null_mut())
            .map_err(|e| format!("el servidor no contesto: {}", e.message()))?;
        let mut codigo: u32 = 0;
        let mut largo = std::mem::size_of::<u32>() as u32;
        let _ = WinHttpQueryHeaders(
            peticion.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&mut codigo as *mut u32).cast()),
            &mut largo,
            std::ptr::null_mut(),
        );
        let mut buf = vec![0u8; 16 * 1024];
        let mut todo = Vec::new();
        loop {
            let mut n: u32 = 0;
            WinHttpReadData(
                peticion.0,
                buf.as_mut_ptr().cast(),
                buf.len() as u32,
                &mut n,
            )
            .map_err(|e| format!("se corto la respuesta: {}", e.message()))?;
            if n == 0 {
                break;
            }
            todo.extend_from_slice(&buf[..n as usize]);
            if todo.len() > LIMITE {
                return Err("la respuesta es demasiado grande".into());
            }
        }
        Ok(Contestado {
            codigo: codigo.min(u16::MAX as u32) as u16,
            cuerpo: todo,
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::partir;

    #[test]
    fn solo_https_salvo_este_equipo() {
        assert_eq!(
            partir("https://api.typesafe.ai/v1/systemone"),
            Some((true, "api.typesafe.ai".into(), 443, "/v1/systemone".into()))
        );
        assert!(partir("http://127.0.0.1:5555/v1/systemone").is_some());
        assert!(partir("http://api.typesafe.ai/v1/systemone").is_none());
    }
}
