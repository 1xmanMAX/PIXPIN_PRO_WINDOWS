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
pub mod envio;
pub mod mensajes;

/// La version del protocolo que se habla. Viaja en el saludo `hola`.
///
/// **La comprobacion es estricta y sin margen**: Android corta si el numero
/// no es exactamente el suyo. No hay negociacion ni rango, asi que una v3 y
/// una v4 no se hablan, y este numero hay que subirlo a la vez que alli.
///
/// Historia: la 2 mandaba el resumen detras de cada archivo; la 3 trajo los
/// codigos unicos, la fusion sin preguntar y los parches; la 4 (16-sep-2026)
/// hace viajar el borrado de un proyecto con las lapidas de chat.
pub const VERSION: u32 = 4;

/// El tipo de servicio que se anuncia y se busca por mDNS al sincronizar.
pub const SERVICIO: &str = "_pixpin._tcp.";
/// Mandar algo puntual a otro aparato: el que envia escucha y anuncia esto.
pub const SERVICIO_ENVIO: &str = "_pixpinenvio._tcp.";
/// Y al reves: el que RECIBE escucha y ensena su codigo, para que el otro se
/// lo mande. Es el caso de «pasarle esto al ordenador»: el PC ensena su QR y
/// el movil lo escanea.
pub const SERVICIO_RECIBIR: &str = "_pixpinrecibe._tcp.";
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
