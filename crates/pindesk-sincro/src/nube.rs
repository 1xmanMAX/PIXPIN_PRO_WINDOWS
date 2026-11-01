//! **PixPin nube** (protocolo 1, capacidad `deltas1`): los chats compartidos
//! por la nube, ademas de por la red local (1-nov-2026).
//!
//! La nube es la web de la obra (`src/herramientas/nube/` del repositorio
//! `agente-2.0-constrution`, su manual `docs/pixpin-nube.md`). Los chats
//! propios siguen por la red local, gratis y sin servidor; un chat
//! **compartido** se puede llevar por la nube: lo que uno sube lo bajan los
//! demas desde donde esten. Lo que viaja es lo mismo que por la red local
//! (mensajes portatiles, proyecto, ficheros con sus rutas de Android), y de
//! un lienzo o una tinta **solo los trazos nuevos**.
//!
//! - [tipos]: los cuerpos JSON del manual.
//! - [cliente]: un pedido por puerta de la API, sobre un [cliente::Transporte].
//! - [trazos]: el delta de una escena de Excalidraw y la regla para juntar.
//! - [estado]: lo que este aparato sabe que tiene la nube de cada chat.
//! - [vuelta]: bajar, juntar y subir un chat.
//! - [sin_conexion]: «Sin conexión» en un visor.
//! - [http]: un HTTP minimo hacia este equipo (pruebas, sin WinHTTP).
//!
//! Ver `docs/nube.md` de PinDesk.

pub mod cliente;
pub mod estado;
pub mod http;
pub mod sin_conexion;
pub mod tipos;
pub mod trazos;
pub mod vuelta;

#[cfg(test)]
mod de_verdad;
#[cfg(test)]
mod falsa;
#[cfg(test)]
mod pruebas;

use crate::compartidos::Llaves;

/// El nombre del protocolo (se queda `pixpin-nube`: ver `docs/formato-pindesk.md`).
pub const PROTOCOLO: &str = "pixpin-nube";
/// La version del protocolo.
pub const VERSION: u32 = 1;
/// La capacidad de subir y bajar solo los cambios.
pub const DELTAS: &str = "deltas1";
/// El prefijo de las llaves de aparato.
pub const PREFIJO_LLAVE: &str = "pxn_";

/// **El texto que firma un aparato para presentarse** (`PUT /yo`, §3.2):
/// `pixpin-nube|yo|<id del aparato en la nube>|<id>|<clave>`.
pub fn texto_de_yo(aparato_nube: &str, id: &str, clave: &str) -> String {
    format!("pixpin-nube|yo|{aparato_nube}|{id}|{clave}")
}

/// La firma de [texto_de_yo] con las llaves de este aparato.
pub fn firma_de_yo(llaves: &Llaves, aparato_nube: &str, id: &str) -> String {
    llaves.firmar(texto_de_yo(aparato_nube, id, &llaves.clave()).as_bytes())
}

/// Si una llave tiene la forma de las de la nube.
pub fn llave_valida(llave: &str) -> bool {
    let l = llave.trim();
    l.starts_with(PREFIJO_LLAVE)
        && l.len() > PREFIJO_LLAVE.len() + 8
        && l.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}
