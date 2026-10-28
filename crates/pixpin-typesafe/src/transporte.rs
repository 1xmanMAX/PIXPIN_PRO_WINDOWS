//! **Por donde sale la peticion.** Un rasgo pequeno ([`Transporte`]) para
//! que el cliente no sepa de redes: en Windows, WinHTTP ([`winhttp`]), lo
//! mismo que usan el asistente (`pixpin_ia::nube::http`) y la subida de
//! ficheros; en las pruebas, un [`Grabado`] que contesta lo que se le dio y
//! apunta lo que se le pidio. Nunca sale nada a la red en las pruebas.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

#[cfg(windows)]
pub mod winhttp;

/// Lo que contesto el servidor: su codigo y su cuerpo (tambien si no es 2xx:
/// el cliente lee de el el detalle del error).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contestado {
    pub codigo: u16,
    pub cuerpo: Vec<u8>,
}

/// **Manda una peticion HTTPS** y espera la respuesta entera, como mucho
/// `tiempo`. `Err`: no se llego a hablar con el servidor (red, DNS, TLS,
/// tiempo agotado). Bloquea: se llama desde un hilo de fondo.
pub trait Transporte: Send + Sync {
    fn enviar(
        &self,
        metodo: &str,
        url: &str,
        cabeceras: &[(&'static str, String)],
        cuerpo: &[u8],
        tiempo: Duration,
    ) -> Result<Contestado, String>;
}

/// El transporte de este sistema: WinHTTP en Windows. En otro sistema no hay
/// red para Jev todavia (la copia de Mac pondra el suyo).
pub fn del_sistema() -> std::sync::Arc<dyn Transporte> {
    #[cfg(windows)]
    {
        std::sync::Arc::new(winhttp::WinHttp)
    }
    #[cfg(not(windows))]
    {
        std::sync::Arc::new(SinRed)
    }
}

/// Sin red en este sistema: todo es un error (nunca un «no»).
pub struct SinRed;

impl Transporte for SinRed {
    fn enviar(
        &self,
        _: &str,
        _: &str,
        _: &[(&'static str, String)],
        _: &[u8],
        _: Duration,
    ) -> Result<Contestado, String> {
        Err("sin red para TypeSafe en este sistema".into())
    }
}

/// Lo que se le pidio a un [`Grabado`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pedido {
    pub metodo: String,
    pub url: String,
    pub cabeceras: Vec<(String, String)>,
    pub cuerpo: Vec<u8>,
}

/// **Un transporte de mentira** que contesta, en orden, lo grabado. Se
/// queda con cada pedido para comprobarlo. Sin nada que contestar, falla
/// como una red caida.
#[derive(Default)]
pub struct Grabado {
    contestar: Mutex<VecDeque<Result<Contestado, String>>>,
    pub pedidos: Mutex<Vec<Pedido>>,
}

impl Grabado {
    pub fn nuevo(respuestas: Vec<Result<Contestado, String>>) -> Grabado {
        Grabado {
            contestar: Mutex::new(respuestas.into()),
            pedidos: Mutex::new(Vec::new()),
        }
    }

    /// Un 200 con este JSON.
    pub fn ok(json: &str) -> Result<Contestado, String> {
        Ok(Contestado {
            codigo: 200,
            cuerpo: json.as_bytes().to_vec(),
        })
    }

    /// Cuantas veces se llamo.
    pub fn llamadas(&self) -> usize {
        self.pedidos.lock().map(|p| p.len()).unwrap_or(0)
    }
}

impl Transporte for Grabado {
    fn enviar(
        &self,
        metodo: &str,
        url: &str,
        cabeceras: &[(&'static str, String)],
        cuerpo: &[u8],
        _: Duration,
    ) -> Result<Contestado, String> {
        if let Ok(mut p) = self.pedidos.lock() {
            p.push(Pedido {
                metodo: metodo.to_string(),
                url: url.to_string(),
                cabeceras: cabeceras
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.clone()))
                    .collect(),
                cuerpo: cuerpo.to_vec(),
            });
        }
        self.contestar
            .lock()
            .ok()
            .and_then(|mut c| c.pop_front())
            .unwrap_or_else(|| Err("sin respuesta grabada".into()))
    }
}
