//! Sincronizar con PixPin Android por la red local.
//!
//! Es un puerto del protocolo del movil, no un diseño nuevo: cada numero,
//! cada rotulo y cada orden estan copiados de `sincro/` de PixPin Android
//! (repositorio `1xmanMAX/PIXPIN_PRO_ANDROID`, commit `05722d9`) para que los
//! dos aparatos se entiendan **byte a byte**. Si algo de aqui parece
//! mejorable, casi siempre la respuesta es que hay que cambiarlo tambien
//! alli, o no habra sincronizacion.
//!
//! Lo que ya esta:
//!
//! - `codigo`: del codigo que se teclea a la clave del grupo (PBKDF2) y a la
//!   etiqueta publica que se anuncia en la red.
//! - `canal`: el saludo, las dos claves de sesion y los tramos cifrados.
//!
//! Lo que falta, dicho en vez de disimulado: el descubrimiento por mDNS, los
//! mensajes del dialogo, la diferencia y la fusion.

#![forbid(unsafe_code)]

pub mod canal;
pub mod codigo;
pub mod mensajes;

/// La version del protocolo que se habla. Viaja en el saludo `hola`.
///
/// Solo la comprueba quien inicia (asi es en Android): si no coincide, se
/// corta ahi y se pide actualizar los dos aparatos. Historia: la 2 mandaba el
/// resumen detras de cada archivo; la 3 trae los codigos unicos, la fusion
/// sin preguntar y los parches.
pub const VERSION: u32 = 3;

/// El tipo de servicio que se anuncia y se busca por mDNS.
pub const SERVICIO: &str = "_pixpin._tcp.";
/// El puerto que se intenta primero. Si esta ocupado, Android cae a uno
/// efimero y lo dice en el saludo, asi que no se puede dar por fijo.
pub const PUERTO: u16 = 47474;
/// Los cuatro bytes de la sonda que manda quien solo quiere saber si hay
/// alguien escuchando, antes de hablar el protocolo.
pub const SONDA: &[u8; 4] = b"PING";
/// Lo que contesta quien esta vivo.
pub const SONDA_RESPUESTA: &[u8; 4] = b"PONG";
/// Cuantos mensajes se piden de una vez.
pub const MENSAJES_POR_TANDA: usize = 200;
/// Cuantos aparatos caben en un grupo: una letra cada uno, de la A a la Z.
pub const APARATOS_POR_GRUPO: usize = 26;
