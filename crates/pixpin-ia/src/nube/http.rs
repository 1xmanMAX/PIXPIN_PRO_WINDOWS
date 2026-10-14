//! **Un POST por HTTPS que se lee a trozos**, con WinHTTP (lo que ya trae
//! Windows).
//!
//! Por que WinHTTP y no un cliente de crates.io: PixPin ya lo usa para subir
//! (`pixpin_shell::subir`) y para bajar modelos (`pixpin-voz`); trae el TLS
//! del sistema (con sus certificados y su proxy) y no anade ni un byte de
//! criptografia propia al ejecutable.
//!
//! Lo unico especial aqui es que la respuesta se entrega **a medida que
//! llega** (las API de la nube contestan con Server-Sent Events) y que
//! `al_trozo` puede pedir parar entre trozo y trozo.
//!
//! `http://` solo se acepta hacia `127.0.0.1`/`localhost`: es para el
//! servidor falso de las pruebas. Una clave nunca sale sin cifrar a la red.
//!
//! Este modulo habla con el sistema operativo: cada `unsafe` lleva su
//! `// SAFETY:`.

use std::ffi::c_void;
use std::time::Duration;

use windows::Win32::Networking::WinHttp::{
    WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
    WINHTTP_ACCESS_TYPE_NO_PROXY, WINHTTP_FLAG_SECURE, WINHTTP_OPEN_REQUEST_FLAGS,
    WINHTTP_QUERY_CUSTOM, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders,
    WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetTimeouts,
};
use windows::core::{HSTRING, PCWSTR, w};

use crate::motor::Seguir;

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

/// Por que no salio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fallo {
    /// No se llego a hablar con el servidor (sin red, DNS, TLS, tiempo).
    Red(String),
    /// El servidor contesto con un codigo que no es 2xx. `cuerpo` es el
    /// principio de lo que dijo (su JSON de error).
    Http {
        codigo: u32,
        cuerpo: String,
        /// Lo que pidio esperar con `retry-after`, si lo dijo.
        esperar: Option<Duration>,
    },
}

/// Lo que se manda.
pub struct Pedido<'a> {
    pub url: &'a str,
    /// Cabeceras extra (la clave va aqui: nunca se escribe en un registro).
    pub cabeceras: Vec<(&'static str, String)>,
    pub cuerpo: &'a [u8],
}

/// Una direccion partida: si va cifrada, servidor, puerto y ruta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Direccion {
    pub segura: bool,
    pub servidor: String,
    pub puerto: u16,
    pub ruta: String,
}

/// Parte `https://servidor[:puerto]/ruta`. `http://` solo hacia este mismo
/// equipo.
pub fn partir(url: &str) -> Option<Direccion> {
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
    if servidor.is_empty() {
        return None;
    }
    if !segura && !matches!(servidor, "127.0.0.1" | "localhost") {
        return None;
    }
    Some(Direccion {
        segura,
        servidor: servidor.to_string(),
        puerto,
        ruta: ruta.to_string(),
    })
}

/// **Manda el POST** y entrega la respuesta a trozos. Bloquea: llamarlo
/// desde un hilo de fondo. Devuelve `Ok(true)` si `al_trozo` pidio parar.
pub fn enviar(p: &Pedido, al_trozo: &mut dyn FnMut(&[u8]) -> Seguir) -> Result<bool, Fallo> {
    let d = partir(p.url).ok_or_else(|| Fallo::Red(format!("direccion no valida: {}", p.url)))?;
    let mut cabeceras = String::from("Content-Type: application/json\r\nAccept: text/event-stream\r\n");
    for (k, v) in &p.cabeceras {
        cabeceras.push_str(k);
        cabeceras.push_str(": ");
        cabeceras.push_str(v);
        cabeceras.push_str("\r\n");
    }
    let cabeceras = HSTRING::from(cabeceras);
    let error = |que: &str| Fallo::Red(format!("{que}: {}", windows::core::Error::from_thread().message()));
    if p.cuerpo.len() > u32::MAX as usize {
        return Err(Fallo::Red("la peticion es demasiado grande".into()));
    }
    // SAFETY: las cadenas son literales o `HSTRING` vivos durante cada
    // llamada; cada asa se envuelve en `Asa` en cuanto existe, asi que se
    // cierra en todos los caminos y en orden inverso. Los bufferes que se
    // pasan a WinHTTP viven mientras dura cada llamada, que es sincrona.
    unsafe {
        let local = !d.segura;
        let mut sesion = Asa(WinHttpOpen(
            w!("PixPin"),
            if local { WINHTTP_ACCESS_TYPE_NO_PROXY } else { WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY },
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ));
        if sesion.0.is_null() && !local {
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
        // 20 s para conectar y mandar; hasta 3 min sin recibir nada (un
        // modelo que piensa mucho antes de escribir manda latidos, pero
        // mejor holgado).
        let _ = WinHttpSetTimeouts(sesion.0, 20_000, 20_000, 30_000, 180_000);
        let conexion = Asa(WinHttpConnect(sesion.0, &HSTRING::from(d.servidor.as_str()), d.puerto, 0));
        if conexion.0.is_null() {
            return Err(error("no se pudo conectar"));
        }
        let peticion = Asa(WinHttpOpenRequest(
            conexion.0,
            w!("POST"),
            &HSTRING::from(d.ruta.as_str()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_OPEN_REQUEST_FLAGS(if d.segura { WINHTTP_FLAG_SECURE.0 } else { 0 }),
        ));
        if peticion.0.is_null() {
            return Err(error("no se pudo preparar la peticion"));
        }
        WinHttpSendRequest(
            peticion.0,
            Some(&cabeceras),
            Some(p.cuerpo.as_ptr().cast()),
            p.cuerpo.len() as u32,
            p.cuerpo.len() as u32,
            0,
        )
        .map_err(|e| Fallo::Red(format!("no se pudo mandar: {}", e.message())))?;
        WinHttpReceiveResponse(peticion.0, std::ptr::null_mut())
            .map_err(|e| Fallo::Red(format!("el servidor no contesto: {}", e.message())))?;
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
        if !(200..300).contains(&codigo) {
            let esperar = cabecera(peticion.0, "retry-after")
                .and_then(|s| s.trim().parse::<f64>().ok())
                .filter(|s| s.is_finite() && *s >= 0.0)
                .map(|s| Duration::from_secs_f64(s.min(120.0)));
            let mut cuerpo = Vec::new();
            loop {
                let mut n: u32 = 0;
                if WinHttpReadData(peticion.0, buf.as_mut_ptr().cast(), buf.len() as u32, &mut n).is_err() || n == 0 {
                    break;
                }
                cuerpo.extend_from_slice(&buf[..n as usize]);
                if cuerpo.len() > 64 * 1024 {
                    break;
                }
            }
            return Err(Fallo::Http {
                codigo,
                cuerpo: String::from_utf8_lossy(&cuerpo).into_owned(),
                esperar,
            });
        }
        loop {
            let mut n: u32 = 0;
            WinHttpReadData(peticion.0, buf.as_mut_ptr().cast(), buf.len() as u32, &mut n)
                .map_err(|e| Fallo::Red(format!("se corto la respuesta: {}", e.message())))?;
            if n == 0 {
                return Ok(false);
            }
            if al_trozo(&buf[..n as usize]) == Seguir::Parar {
                return Ok(true);
            }
        }
    }
}

/// El valor de una cabecera de la respuesta, si esta.
///
/// # Safety
/// `peticion` tiene que ser un asa de peticion de WinHTTP viva que ya
/// recibio la respuesta.
unsafe fn cabecera(peticion: *mut c_void, nombre: &str) -> Option<String> {
    let nombre = HSTRING::from(nombre);
    let mut buf = vec![0u16; 256];
    let mut largo = (buf.len() * 2) as u32;
    // SAFETY: el bufer vive durante la llamada y su tamano en bytes es el
    // que se pasa; el nombre es un `HSTRING` vivo.
    unsafe {
        WinHttpQueryHeaders(
            peticion,
            WINHTTP_QUERY_CUSTOM,
            &nombre,
            Some(buf.as_mut_ptr().cast()),
            &mut largo,
            std::ptr::null_mut(),
        )
        .ok()?;
    }
    let n = (largo as usize / 2).min(buf.len());
    Some(String::from_utf16_lossy(&buf[..n]))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn https_con_y_sin_puerto() {
        let d = partir("https://api.anthropic.com/v1/messages").unwrap();
        assert!(d.segura);
        assert_eq!((d.servidor.as_str(), d.puerto, d.ruta.as_str()), ("api.anthropic.com", 443, "/v1/messages"));
        let d = partir("https://x.y:8443").unwrap();
        assert_eq!((d.puerto, d.ruta.as_str()), (8443, "/"));
    }

    #[test]
    fn http_sin_cifrar_solo_hacia_este_equipo() {
        assert!(partir("http://127.0.0.1:5555/v1/messages").is_some());
        assert!(partir("http://localhost:1/a").is_some());
        assert!(partir("http://api.deepseek.com/chat/completions").is_none());
        assert!(partir("ftp://x").is_none());
        assert!(partir("https://:44/").is_none());
        assert!(partir("https://x:noesnumero/").is_none());
    }
}
