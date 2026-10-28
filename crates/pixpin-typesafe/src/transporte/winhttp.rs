//! **El POST con WinHTTP**, lo que ya trae Windows (como
//! `pixpin_ia::nube::http`): el TLS y el proxy del sistema, sin cliente HTTP
//! propio en el ejecutable. Aqui todo es corto: Jev contesta en 70-500 ms y
//! quien llama da el tiempo (unos 2 s), que vale para conectar, mandar y
//! recibir.
//!
//! Solo `https://`, salvo hacia `127.0.0.1`/`localhost` (un servidor de
//! pruebas): la clave nunca sale sin cifrar a la red.
//!
//! Este modulo habla con el sistema operativo: cada `unsafe` lleva su
//! `// SAFETY:`.

use std::ffi::c_void;
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
        // llamada; cada asa se envuelve en `Asa` en cuanto existe, asi que
        // se cierra en todos los caminos y en orden inverso (las variables
        // se sueltan al reves de como se crearon). Los bufferes que se pasan
        // a WinHTTP viven mientras dura cada llamada, que es sincrona.
        unsafe {
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
            let _ = WinHttpSetTimeouts(sesion.0, ms, ms, ms, ms);
            let conexion = Asa(WinHttpConnect(
                sesion.0,
                &HSTRING::from(servidor.as_str()),
                puerto,
                0,
            ));
            if conexion.0.is_null() {
                return Err(error("no se pudo conectar"));
            }
            let peticion = Asa(WinHttpOpenRequest(
                conexion.0,
                &HSTRING::from(metodo),
                &HSTRING::from(ruta.as_str()),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                WINHTTP_OPEN_REQUEST_FLAGS(if segura { WINHTTP_FLAG_SECURE.0 } else { 0 }),
            ));
            if peticion.0.is_null() {
                return Err(error("no se pudo preparar la peticion"));
            }
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
