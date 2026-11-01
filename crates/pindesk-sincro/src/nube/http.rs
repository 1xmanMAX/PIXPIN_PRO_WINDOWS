//! **Un HTTP/1.1 minimo hacia este mismo equipo** (`http://127.0.0.1:…`):
//! el transporte de las pruebas de punta a punta contra la nube de
//! referencia, y el de un PinDesk sin WinHTTP. Solo `http://` y solo hacia
//! `127.0.0.1` o `localhost`: una llave nunca sale sin cifrar a la red. En
//! Windows la app usa WinHTTP (`pindesk_ia::nube::http`), con TLS.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use super::cliente::{Contestado, Transporte};

/// El transporte hacia este equipo.
pub struct HttpLocal;

/// `(servidor, puerto, ruta)` de una direccion `http://` local.
pub fn partir(url: &str) -> Option<(String, u16, String)> {
    let resto = url.strip_prefix("http://")?;
    let (anfitrion, ruta) = match resto.find('/') {
        Some(i) => (&resto[..i], &resto[i..]),
        None => (resto, "/"),
    };
    let (servidor, puerto) = match anfitrion.rsplit_once(':') {
        Some((s, p)) => (s, p.parse().ok()?),
        None => (anfitrion, 80),
    };
    matches!(servidor, "127.0.0.1" | "localhost")
        .then(|| (servidor.to_string(), puerto, ruta.to_string()))
}

fn leer_troceado(r: &mut impl BufRead) -> std::io::Result<Vec<u8>> {
    let mut salida = Vec::new();
    loop {
        let mut linea = String::new();
        r.read_line(&mut linea)?;
        let n = usize::from_str_radix(linea.trim().split(';').next().unwrap_or("0"), 16)
            .map_err(std::io::Error::other)?;
        if n == 0 {
            let mut fin = String::new();
            let _ = r.read_line(&mut fin);
            return Ok(salida);
        }
        let mut trozo = vec![0; n];
        r.read_exact(&mut trozo)?;
        salida.extend_from_slice(&trozo);
        let mut crlf = [0u8; 2];
        r.read_exact(&mut crlf)?;
    }
}

impl Transporte for HttpLocal {
    fn enviar(
        &self,
        metodo: &str,
        url: &str,
        cabeceras: &[(&'static str, String)],
        cuerpo: &[u8],
        tiempo: Duration,
    ) -> Result<Contestado, String> {
        let (servidor, puerto, ruta) =
            partir(url).ok_or_else(|| format!("solo http:// hacia este equipo: {url}"))?;
        let mut s = TcpStream::connect((servidor.as_str(), puerto)).map_err(|e| e.to_string())?;
        s.set_read_timeout(Some(tiempo))
            .map_err(|e| e.to_string())?;
        let mut cab = format!(
            "{metodo} {ruta} HTTP/1.1\r\nHost: {servidor}:{puerto}\r\nConnection: close\r\nContent-Length: {}\r\n",
            cuerpo.len()
        );
        for (k, v) in cabeceras {
            cab.push_str(&format!("{k}: {v}\r\n"));
        }
        cab.push_str("\r\n");
        s.write_all(cab.as_bytes()).map_err(|e| e.to_string())?;
        s.write_all(cuerpo).map_err(|e| e.to_string())?;
        let mut r = BufReader::new(s);
        let mut linea = String::new();
        r.read_line(&mut linea).map_err(|e| e.to_string())?;
        let codigo: u16 = linea
            .split_whitespace()
            .nth(1)
            .and_then(|c| c.parse().ok())
            .ok_or_else(|| format!("respuesta rara: {linea}"))?;
        let mut largo = None;
        let mut troceado = false;
        loop {
            let mut l = String::new();
            r.read_line(&mut l).map_err(|e| e.to_string())?;
            let l = l.trim_end();
            if l.is_empty() {
                break;
            }
            if let Some((k, v)) = l.split_once(':') {
                let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
                if k == "content-length" {
                    largo = v.parse::<usize>().ok();
                } else if k == "transfer-encoding" && v.eq_ignore_ascii_case("chunked") {
                    troceado = true;
                }
            }
        }
        let cuerpo = if metodo == "HEAD" {
            Vec::new()
        } else if troceado {
            leer_troceado(&mut r).map_err(|e| e.to_string())?
        } else if let Some(n) = largo {
            let mut b = vec![0; n];
            r.read_exact(&mut b).map_err(|e| e.to_string())?;
            b
        } else {
            let mut b = Vec::new();
            r.read_to_end(&mut b).map_err(|e| e.to_string())?;
            b
        };
        Ok(Contestado { codigo, cuerpo })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn solo_hacia_este_equipo() {
        assert_eq!(
            partir("http://127.0.0.1:3000/nube/api/v1/hola"),
            Some(("127.0.0.1".into(), 3000, "/nube/api/v1/hola".into()))
        );
        assert_eq!(partir("http://obra.ejemplo.com/x"), None);
        assert_eq!(partir("https://127.0.0.1/x"), None);
    }

    #[test]
    fn lee_una_respuesta_troceada() {
        let mut r = std::io::Cursor::new(b"4\r\nhola\r\n1\r\n!\r\n0\r\n\r\n".to_vec());
        assert_eq!(leer_troceado(&mut r).unwrap(), b"hola!");
    }
}
