//! **Los pedidos a la nube**, uno por puerta de la API (§3 del manual y
//! `deltas1`), sin saber de redes: la peticion sale por un [Transporte]
//! que pone cada aparato (WinHTTP en Windows; [`super::http::HttpLocal`]
//! hacia este equipo en las pruebas; uno falso en memoria en las de aqui).

use std::time::Duration;

use super::tipos::{Cambios, ChatDeLaNube, Hola, Inventario, Novedades, Parche, Subido};

/// Lo que contesto el servidor, tambien si no es 2xx.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contestado {
    pub codigo: u16,
    pub cuerpo: Vec<u8>,
}

/// **Manda una peticion** y espera la respuesta entera, como mucho `tiempo`.
/// `Err`: no se llego a hablar con el servidor (sin red, DNS, TLS, tiempo).
/// Bloquea: se llama desde un hilo de fondo.
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

/// Por que no salio.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FalloNube {
    /// No hay red, o el servidor no contesta: se reintenta con espera.
    #[error("sin red: {0}")]
    Red(String),
    /// La llave no vale (401) o no deja (403): no se reintenta solo.
    #[error("{0}")]
    Llave(String),
    /// Otro subio entre medias: bajar, juntar y volver a subir.
    #[error("la nube va por la revision {0}")]
    Conflicto(i64),
    /// Lo que no esta (404).
    #[error("no esta en la nube")]
    NoEsta,
    /// Otra respuesta de error, con lo que dijo la nube.
    #[error("la nube dijo {0}: {1}")]
    Servidor(u16, String),
    /// Una respuesta que no se entiende, o un fichero que llego cambiado.
    #[error("respuesta rara: {0}")]
    Formato(String),
    /// No se pudo leer o escribir aqui.
    #[error("disco: {0}")]
    Disco(String),
}

impl From<std::io::Error> for FalloNube {
    fn from(e: std::io::Error) -> Self {
        FalloNube::Disco(e.to_string())
    }
}

impl FalloNube {
    /// Si vale la pena volver a intentarlo mas tarde (red o servidor caido).
    pub fn pasajero(&self) -> bool {
        matches!(self, FalloNube::Red(_)) || matches!(self, FalloNube::Servidor(c, _) if *c >= 500)
    }
}

/// Cuanto se espera una peticion normal.
pub const TIEMPO: Duration = Duration::from_secs(60);
/// Lo que se espera de mas en `novedades` sobre lo pedido con `espera`.
pub const MARGEN_DE_ESPERA: Duration = Duration::from_secs(15);

/// **La nube de este aparato**: su direccion, su llave y por donde sale.
pub struct Nube<'a> {
    /// `https://obra.ejemplo.com`, sin barra final.
    pub direccion: String,
    pub llave: String,
    pub t: &'a dyn Transporte,
}

/// El error que trae el JSON de la nube, o el principio del cuerpo.
fn error_de(cuerpo: &[u8]) -> String {
    serde_json::from_slice::<serde_json::Value>(cuerpo)
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
        .unwrap_or_else(|| String::from_utf8_lossy(&cuerpo[..cuerpo.len().min(200)]).into_owned())
}

/// Lo que va en `?espacio=` (minusculas, cifras y guiones: no hace falta mas).
fn con_espacio(espacio: &str) -> String {
    if espacio.is_empty() {
        String::new()
    } else {
        format!("espacio={}", codificar(espacio))
    }
}

/// `encodeURIComponent` de lo que puede llevar un id de chat o un espacio.
pub fn codificar(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

impl Nube<'_> {
    fn url(&self, ruta: &str) -> String {
        format!(
            "{}/nube/api/v1/{ruta}",
            self.direccion.trim_end_matches('/')
        )
    }

    fn pedir(
        &self,
        metodo: &str,
        ruta: &str,
        cuerpo: &[u8],
        tipo: Option<&str>,
        tiempo: Duration,
    ) -> Result<Contestado, FalloNube> {
        let mut cab = vec![("Authorization", format!("Bearer {}", self.llave))];
        if let Some(t) = tipo {
            cab.push(("Content-Type", t.to_string()));
        }
        let r = self
            .t
            .enviar(metodo, &self.url(ruta), &cab, cuerpo, tiempo)
            .map_err(FalloNube::Red)?;
        match r.codigo {
            200..=299 => Ok(r),
            401 | 403 => Err(FalloNube::Llave(error_de(&r.cuerpo))),
            404 => Err(FalloNube::NoEsta),
            409 => {
                let rev = serde_json::from_slice::<serde_json::Value>(&r.cuerpo)
                    .ok()
                    .and_then(|v| v.get("revision").and_then(|x| x.as_i64()))
                    .unwrap_or(0);
                Err(FalloNube::Conflicto(rev))
            }
            c => Err(FalloNube::Servidor(c, error_de(&r.cuerpo))),
        }
    }

    fn json<T: serde::de::DeserializeOwned>(r: Contestado) -> Result<T, FalloNube> {
        serde_json::from_slice(&r.cuerpo).map_err(|e| FalloNube::Formato(e.to_string()))
    }

    fn con_json<T: serde::de::DeserializeOwned>(
        &self,
        metodo: &str,
        ruta: &str,
        cuerpo: &impl serde::Serialize,
    ) -> Result<T, FalloNube> {
        let b = serde_json::to_vec(cuerpo).map_err(|e| FalloNube::Formato(e.to_string()))?;
        Self::json(self.pedir(metodo, ruta, &b, Some("application/json"), TIEMPO)?)
    }

    /// `GET hola`: comprueba la llave y dice que sabe la nube.
    pub fn hola(&self) -> Result<Hola, FalloNube> {
        let h: Hola = Self::json(self.pedir("GET", "hola", &[], None, TIEMPO)?)?;
        if h.protocolo != super::PROTOCOLO {
            return Err(FalloNube::Formato(format!(
                "no es una nube de PixPin ({})",
                h.protocolo
            )));
        }
        Ok(h)
    }

    /// `PUT yo`: la persona de PinDesk de este aparato, con su firma.
    pub fn presentar(&self, id: &str, clave: &str, firma: &str) -> Result<(), FalloNube> {
        let _: serde_json::Value = self.con_json(
            "PUT",
            "yo",
            &serde_json::json!({"id": id, "clave": clave, "firma": firma}),
        )?;
        Ok(())
    }

    /// `GET chats`: los suyos y los compartidos donde esta dentro.
    pub fn chats(&self) -> Result<Vec<ChatDeLaNube>, FalloNube> {
        #[derive(serde::Deserialize)]
        struct L {
            #[serde(default)]
            chats: Vec<ChatDeLaNube>,
        }
        let l: L = Self::json(self.pedir("GET", "chats", &[], None, TIEMPO)?)?;
        Ok(l.chats)
    }

    /// `GET chats/<chat>`: el inventario entero, con su revision.
    pub fn inventario(&self, espacio: &str, chat: &str) -> Result<Inventario, FalloNube> {
        let q = con_espacio(espacio);
        let ruta = format!(
            "chats/{}{}{q}",
            codificar(chat),
            if q.is_empty() { "" } else { "?" }
        );
        Self::json(self.pedir("GET", &ruta, &[], None, TIEMPO)?)
    }

    /// `PUT chats/<chat>`: el inventario entero (protocolo 1).
    pub fn subir_inventario(&self, chat: &str, inv: &Inventario) -> Result<Subido, FalloNube> {
        self.con_json("PUT", &format!("chats/{}", codificar(chat)), inv)
    }

    /// `PATCH chats/<chat>`: solo lo que cambio (`deltas1`).
    pub fn parche(&self, chat: &str, p: &Parche) -> Result<Subido, FalloNube> {
        self.con_json("PATCH", &format!("chats/{}", codificar(chat)), p)
    }

    /// `GET chats/<chat>/cambios?espacio=&desde=` (`deltas1`).
    pub fn cambios(&self, espacio: &str, chat: &str, desde: i64) -> Result<Cambios, FalloNube> {
        let q = con_espacio(espacio);
        let ruta = format!(
            "chats/{}/cambios?{q}{}desde={desde}",
            codificar(chat),
            if q.is_empty() { "" } else { "&" }
        );
        Self::json(self.pedir("GET", &ruta, &[], None, TIEMPO)?)
    }

    /// `HEAD objetos/<sha>`: si la nube ya lo tiene.
    pub fn hay_objeto(&self, sha: &str) -> Result<bool, FalloNube> {
        match self.pedir("HEAD", &format!("objetos/{sha}"), &[], None, TIEMPO) {
            Ok(_) => Ok(true),
            Err(FalloNube::NoEsta) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// `PUT objetos/<sha>`: un fichero crudo.
    pub fn subir_objeto(&self, sha: &str, bytes: &[u8]) -> Result<(), FalloNube> {
        self.pedir(
            "PUT",
            &format!("objetos/{sha}"),
            bytes,
            Some("application/octet-stream"),
            TIEMPO,
        )?;
        Ok(())
    }

    /// `GET objetos/<sha>`, comprobando que llego lo que se pidio.
    pub fn bajar_objeto(&self, sha: &str) -> Result<Vec<u8>, FalloNube> {
        let r = self.pedir("GET", &format!("objetos/{sha}"), &[], None, TIEMPO)?;
        if crate::canonico::sha256_hex(&r.cuerpo) != sha {
            return Err(FalloNube::Formato(format!(
                "el objeto {sha} llego cambiado"
            )));
        }
        Ok(r.cuerpo)
    }

    /// `GET novedades?desde=&espera=`: lo que cambio, esperando con la
    /// conexion abierta hasta `espera` segundos (maximo 30).
    pub fn novedades(&self, desde: i64, espera: u64) -> Result<Novedades, FalloNube> {
        let espera = espera.min(30);
        let r = self.pedir(
            "GET",
            &format!("novedades?desde={desde}&espera={espera}"),
            &[],
            None,
            Duration::from_secs(espera) + MARGEN_DE_ESPERA,
        )?;
        Self::json(r)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn codifica_como_encode_uri_component() {
        assert_eq!(codificar("p-obra_1.x"), "p-obra_1.x");
        assert_eq!(codificar("a b/ñ"), "a%20b%2F%C3%B1");
    }

    #[test]
    fn el_error_de_la_nube_se_lee_de_su_json() {
        assert_eq!(
            error_de(br#"{"error":"Llave revocada."}"#),
            "Llave revocada."
        );
        assert_eq!(error_de(b"caido"), "caido");
    }
}
