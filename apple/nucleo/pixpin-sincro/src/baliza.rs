//! **Descubrir rapido y prioridad al grabar** (28-oct-2026).
//!
//! El usuario: «con las clases grabadas, el MOVIL tarda demasiado en saber
//! si el PC esta grabando, y no puedo hacer fotos rapido. Cuando el PC grabe,
//! que active un modo de sincronizacion PRIORITARIO para la grabacion y sus
//! fotos y todo lo relacionado, que localice cualquier aparato de la red
//! rapido y simple. Todo tiene que ser rapido y fiable mientras se graba:
//! perder algo seria perder una reunion».
//!
//! Por que tardaba: los aparatos se encuentran por mDNS (`_pixpin._tcp`), que
//! muchas Wi-Fi reparten con uno o dos minutos de retraso, y por la sonda TCP
//! a la **ultima direccion conocida**, que va cada 60 s sin nadie mirando y no
//! sirve si el DHCP le dio otra al movil. El aviso del PC al empezar a grabar
//! iba solo a esa ultima direccion.
//!
//! Lo nuevo es una **baliza**: un datagrama UDP pequeño, firmado con la clave
//! del grupo, que quien graba manda **cada segundo** (y al momento al
//! empezar, pausar, seguir o acabar) por difusion (`255.255.255.255` y la de
//! cada red), por multidifusion ([`GRUPO_MULTICAST`]) y directa a las
//! direcciones conocidas; sin grabar, cada [`CADA_EN_REPOSO_MS`]. Quien la
//! oye y es del grupo sabe al momento donde esta el otro (IP de origen y
//! puerto TCP de la baliza) y si graba, sin esperar a ninguna sonda.
//!
//! - **Formato** ([`Baliza::escribir`], [`Baliza::leer`]): binario, de
//!   longitud variable, con un HMAC-SHA256 truncado a 16 bytes con una clave
//!   derivada de la del grupo (`HMAC(clave, "baliza")`). Quien no es del grupo
//!   no puede fingir una, y una repetida (la misma llega por tres caminos, o
//!   alguien la graba y la reenvia) se tira por su reloj y su secuencia
//!   ([`Oidas`]). La baliza **no abre nada**: solo dice a donde llamar, y la
//!   llamada va por el canal cifrado de siempre.
//! - **Sockets** ([`Emisor`], [`Oyente`]): uno por cada red de este aparato
//!   (Wi-Fi, cable, VPN), para que la difusion salga por todas y no solo por
//!   la de la ruta por defecto.
//! - **Si la difusion no pasa** (Wi-Fi que filtra la difusion o la
//!   multidifusion): la baliza va tambien directa a las ultimas direcciones
//!   conocidas y, grabando sin oir a nadie, a las de la red entera
//!   ([`barrido`]), que son datagramas de 100 bytes.
//! - **Que hacer primero** ([`Trabajo`], [`que_toca`]): mientras alguien del
//!   grupo graba, solo la camara (el anuncio, las fotos y clips); al acabar,
//!   los ficheros de esa grabacion antes que nada; lo demas, despues.
//!
//! El formato y los tiempos estan en `docs/movil-como-camara.md` («Descubrir
//! rápido y prioridad al grabar»), que es lo que tiene que hacer Android.

use std::collections::HashMap;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::time::Duration;

use hmac::{Hmac, Mac};
use sha2::Sha256;

/// El puerto UDP de la baliza: el de la sincronizacion (`PUERTO`) mas uno.
/// Fijo, porque una difusion no sabe de puertos efimeros.
pub const PUERTO_BALIZA: u16 = 47475;
/// La multidifusion de la baliza (ambito local de la organizacion, RFC 2365).
pub const GRUPO_MULTICAST: Ipv4Addr = Ipv4Addr::new(239, 255, 74, 74);
/// Los cuatro primeros bytes.
pub const MAGIA: &[u8; 4] = b"PXB1";
/// La version del formato.
pub const VERSION: u8 = 1;
/// Lo que mide la firma.
pub const LARGO_FIRMA: usize = 16;
/// Lo mas largo que puede ser el id del aparato o el de la grabacion.
pub const TOPE_TEXTO: usize = 64;
/// Lo que mide la parte fija (antes de los dos textos y la firma).
const CABEZA: usize = 36;
/// Lo mas que puede medir una baliza.
pub const TOPE: usize = CABEZA + 2 + 2 * TOPE_TEXTO + LARGO_FIRMA;

/// Cada cuanto se manda grabando.
pub const CADA_GRABANDO_MS: u64 = 1_000;
/// Cada cuanto sin grabar: lo justo para que quien abra PixPin vea pronto a
/// los demas sin llenar la red.
pub const CADA_EN_REPOSO_MS: u64 = 15_000;
/// Cuanto vale una baliza de «grabando» sin otra detras: tres perdidas
/// seguidas y ya no se da por grabando (el otro se fue o se apago).
pub const VIGENCIA_GRABANDO_MS: i64 = 4_000;
/// Cuanto se espera grabando sin oir a nadie antes de barrer la red.
pub const BARRER_TRAS_MS: i64 = 5_000;
/// Y cada cuanto se repite el barrido mientras siga sin oirse a nadie.
pub const BARRER_CADA_MS: i64 = 10_000;
/// Cuantas direcciones se barren como mucho por red (una /24 entera).
pub const TOPE_BARRIDO: usize = 254;
/// Lo mas que puede ir desviado el reloj de quien manda la baliza: mas que
/// eso es una baliza vieja que alguien reenvia.
pub const DESFASE_MAXIMO_MS: i64 = 10 * 60_000;

/// Las banderas del byte 5.
pub const GRABANDO: u8 = 1;
pub const EN_PAUSA: u8 = 1 << 1;
/// «Quien me oiga, que me conteste con la suya, directa»: la manda quien
/// acaba de abrir PixPin (o de cambiar de red) para no esperar al siguiente
/// latido de los demas.
pub const PREGUNTA: u8 = 1 << 2;
/// Este aparato acepta fotos y clips en su grabacion (`camara1` encendida).
pub const CAMARA: u8 = 1 << 3;

/// Lo que dice una baliza.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Baliza {
    /// La etiqueta del grupo en crudo: los ocho primeros bytes de
    /// `HMAC(clave, "etiqueta")` (la `g` del anuncio mDNS, en hex).
    pub etiqueta: [u8; 8],
    /// El id del aparato que la manda (`Aparato::id`).
    pub id: String,
    /// El puerto TCP donde escucha la sincronizacion.
    pub puerto: u16,
    /// El reloj de quien la manda (ms desde 1970).
    pub reloj: i64,
    /// Sube con cada baliza que manda el aparato; con [`Baliza::reloj`],
    /// lo que tira las repetidas.
    pub secuencia: u32,
    /// [`GRABANDO`], [`EN_PAUSA`], [`PREGUNTA`], [`CAMARA`].
    pub banderas: u8,
    /// El id de la grabacion (`camara_remota.grabacion`), o vacio.
    pub grabacion: String,
    /// En que milisegundo de la grabacion va al mandarla.
    pub ms_actual: i64,
}

/// Por que no vale una baliza.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rechazo {
    /// No es una baliza (otra cosa por el mismo puerto).
    NoEs,
    /// De otra version.
    Version,
    /// Cortada o con largos que no cuadran.
    Rota,
    /// De otro grupo: la etiqueta no es la nuestra.
    OtroGrupo,
    /// La firma no cuadra: alguien la fingio o la toco.
    Firma,
}

type HmacSha256 = Hmac<Sha256>;

/// La clave con que se firman las balizas: derivada, para no usar la del
/// grupo tal cual en algo que va en claro.
pub fn clave_baliza(clave_grupo: &[u8; 32]) -> [u8; 32] {
    crate::codigo::hmac(clave_grupo, b"baliza")
}

/// La etiqueta del grupo en crudo (ocho bytes).
pub fn etiqueta_cruda(clave_grupo: &[u8; 32]) -> [u8; 8] {
    let marca = crate::codigo::hmac(clave_grupo, b"etiqueta");
    let mut e = [0u8; 8];
    e.copy_from_slice(&marca[..8]);
    e
}

fn recortar(t: &str) -> &str {
    if t.len() <= TOPE_TEXTO {
        return t;
    }
    let mut fin = TOPE_TEXTO;
    while !t.is_char_boundary(fin) {
        fin -= 1;
    }
    &t[..fin]
}

impl Baliza {
    pub fn graba(&self) -> bool {
        self.banderas & GRABANDO != 0
    }

    pub fn en_pausa(&self) -> bool {
        self.banderas & EN_PAUSA != 0
    }

    pub fn pregunta(&self) -> bool {
        self.banderas & PREGUNTA != 0
    }

    pub fn camara(&self) -> bool {
        self.banderas & CAMARA != 0
    }

    /// Los bytes, firmados con la clave del grupo. Los textos de mas de
    /// [`TOPE_TEXTO`] bytes se recortan (sin partir una letra).
    pub fn escribir(&self, clave_grupo: &[u8; 32]) -> Vec<u8> {
        let id = recortar(&self.id);
        let grabacion = recortar(&self.grabacion);
        let mut b = Vec::with_capacity(CABEZA + 2 + id.len() + grabacion.len() + LARGO_FIRMA);
        b.extend_from_slice(MAGIA);
        b.push(VERSION);
        b.push(self.banderas);
        b.extend_from_slice(&self.etiqueta);
        b.extend_from_slice(&self.puerto.to_be_bytes());
        b.extend_from_slice(&self.reloj.to_be_bytes());
        b.extend_from_slice(&self.secuencia.to_be_bytes());
        b.extend_from_slice(&self.ms_actual.to_be_bytes());
        b.push(id.len() as u8);
        b.extend_from_slice(id.as_bytes());
        b.push(grabacion.len() as u8);
        b.extend_from_slice(grabacion.as_bytes());
        let firma = crate::codigo::hmac(&clave_baliza(clave_grupo), &b);
        b.extend_from_slice(&firma[..LARGO_FIRMA]);
        b
    }

    /// Lee y comprueba una baliza de **este** grupo. La etiqueta se mira
    /// antes que la firma: lo de otros grupos se tira sin calcular nada.
    pub fn leer(bytes: &[u8], clave_grupo: &[u8; 32]) -> Result<Baliza, Rechazo> {
        if bytes.len() < 4 || &bytes[..4] != MAGIA {
            return Err(Rechazo::NoEs);
        }
        if bytes.len() < CABEZA + 2 + LARGO_FIRMA || bytes.len() > TOPE {
            return Err(Rechazo::Rota);
        }
        if bytes[4] != VERSION {
            return Err(Rechazo::Version);
        }
        let mut etiqueta = [0u8; 8];
        etiqueta.copy_from_slice(&bytes[6..14]);
        if etiqueta != etiqueta_cruda(clave_grupo) {
            return Err(Rechazo::OtroGrupo);
        }
        let (datos, firma) = bytes.split_at(bytes.len() - LARGO_FIRMA);
        let mut mac = <HmacSha256 as Mac>::new_from_slice(&clave_baliza(clave_grupo))
            .expect("HMAC admite claves de cualquier largo");
        mac.update(datos);
        // En tiempo constante: no se le dice a nadie cuantos bytes acerto.
        mac.verify_truncated_left(firma)
            .map_err(|_| Rechazo::Firma)?;
        let u16_en = |i: usize| u16::from_be_bytes([datos[i], datos[i + 1]]);
        let i64_en = |i: usize| {
            let mut x = [0u8; 8];
            x.copy_from_slice(&datos[i..i + 8]);
            i64::from_be_bytes(x)
        };
        let mut sec = [0u8; 4];
        sec.copy_from_slice(&datos[24..28]);
        let mut i = CABEZA;
        let mut texto = || -> Result<String, Rechazo> {
            let n = *datos.get(i).ok_or(Rechazo::Rota)? as usize;
            let t = datos.get(i + 1..i + 1 + n).ok_or(Rechazo::Rota)?;
            i += 1 + n;
            String::from_utf8(t.to_vec()).map_err(|_| Rechazo::Rota)
        };
        let id = texto()?;
        let grabacion = texto()?;
        if i != datos.len() || id.is_empty() {
            return Err(Rechazo::Rota);
        }
        Ok(Baliza {
            etiqueta,
            id,
            puerto: u16_en(14),
            reloj: i64_en(16),
            secuencia: u32::from_be_bytes(sec),
            banderas: datos[5],
            grabacion,
            ms_actual: i64_en(28),
        })
    }
}

// ------------------------------------------------------------ las oidas

/// **Lo que ya se oyo de cada aparato**, para tirar las repetidas: la misma
/// baliza llega por la difusion, por la multidifusion y directa, y alguien
/// podria grabarla y reenviarla. Vale si su `(reloj, secuencia)` es mayor
/// que el ultimo de ese aparato y su reloj no esta a mas de
/// [`DESFASE_MAXIMO_MS`] del nuestro.
#[derive(Debug, Default)]
pub struct Oidas {
    ultimas: HashMap<String, (i64, u32)>,
}

impl Oidas {
    pub fn admitir(&mut self, b: &Baliza, ahora: i64) -> bool {
        if (b.reloj - ahora).abs() > DESFASE_MAXIMO_MS {
            return false;
        }
        let nueva = (b.reloj, b.secuencia);
        match self.ultimas.get(&b.id) {
            // Un reloj que va hacia atras mas que el desfase (se cambio la
            // hora) tambien entra: si no, ese aparato quedaria mudo.
            Some(&antes) if nueva <= antes && antes.0 - b.reloj < DESFASE_MAXIMO_MS => false,
            _ => {
                self.ultimas.insert(b.id.clone(), nueva);
                true
            }
        }
    }
}

// ------------------------------------------------------------ las redes

/// Una red IPv4 de este aparato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interfaz {
    pub ip: Ipv4Addr,
    /// Largo del prefijo (24 en `255.255.255.0`).
    pub prefijo: u8,
}

impl Interfaz {
    pub fn mascara(&self) -> u32 {
        match self.prefijo {
            0 => 0,
            p if p >= 32 => u32::MAX,
            p => u32::MAX << (32 - p),
        }
    }

    /// La direccion de difusion de esta red (`192.168.1.255`).
    pub fn difusion(&self) -> Ipv4Addr {
        Ipv4Addr::from(u32::from(self.ip) | !self.mascara())
    }

    pub fn contiene(&self, otra: Ipv4Addr) -> bool {
        u32::from(self.ip) & self.mascara() == u32::from(otra) & self.mascara()
    }
}

/// **Las direcciones a las que barrer** en `red` si no se oye a nadie: la
/// red entera hasta [`TOPE_BARRIDO`], empezando por las conocidas y sus
/// vecinas (el DHCP suele dar la de al lado), sin la propia, la de la red ni
/// la de difusion. En una red de mas de 1022 aparatos, solo las conocidas y
/// sus vecinas: barrerla entera seria llenarla.
pub fn barrido(red: &Interfaz, conocidas: &[Ipv4Addr]) -> Vec<Ipv4Addr> {
    let base = u32::from(red.ip) & red.mascara();
    let ultima = base | !red.mascara();
    let valida = |x: u32| x > base && x < ultima && x != u32::from(red.ip);
    let mut salida: Vec<Ipv4Addr> = Vec::new();
    let poner = |x: u32, salida: &mut Vec<Ipv4Addr>| {
        let ip = Ipv4Addr::from(x);
        if valida(x) && !salida.contains(&ip) && salida.len() < TOPE_BARRIDO {
            salida.push(ip);
        }
    };
    let propias: Vec<u32> = conocidas
        .iter()
        .filter(|c| red.contiene(**c))
        .map(|c| u32::from(*c))
        .collect();
    for &c in &propias {
        poner(c, &mut salida);
    }
    for d in 1..=8u32 {
        for &c in &propias {
            poner(c.wrapping_add(d), &mut salida);
            poner(c.wrapping_sub(d), &mut salida);
        }
    }
    if red.prefijo >= 22 {
        for x in base + 1..ultima {
            if salida.len() >= TOPE_BARRIDO {
                break;
            }
            poner(x, &mut salida);
        }
    }
    salida
}

// ------------------------------------------------------------ los sockets

/// **Quien manda las balizas**: un socket por red, atado a su direccion,
/// para que la difusion y la multidifusion salgan por cada una (con una VPN
/// puesta, la ruta por defecto es la del tunel y la Wi-Fi no se enteraria),
/// y uno suelto para lo demas.
pub struct Emisor {
    puerto: u16,
    redes: Vec<(Interfaz, UdpSocket)>,
    suelto: UdpSocket,
}

impl Emisor {
    /// Con `puerto` = [`PUERTO_BALIZA`] (otro en las pruebas). Una red que no
    /// deja abrir su socket se salta.
    pub fn nuevo(puerto: u16, redes: &[Interfaz]) -> io::Result<Emisor> {
        let suelto = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        let _ = suelto.set_broadcast(true);
        let redes = redes
            .iter()
            .filter_map(|r| {
                let s = UdpSocket::bind((r.ip, 0)).ok()?;
                let _ = s.set_broadcast(true);
                let _ = s.set_multicast_ttl_v4(1);
                let _ = s.set_multicast_loop_v4(true);
                Some((*r, s))
            })
            .collect();
        Ok(Emisor {
            puerto,
            redes,
            suelto,
        })
    }

    pub fn redes(&self) -> Vec<Interfaz> {
        self.redes.iter().map(|(r, _)| *r).collect()
    }

    /// A todos: la difusion general y la de cada red, y la multidifusion.
    /// Devuelve cuantos envios salieron.
    pub fn difundir(&self, paquete: &[u8]) -> usize {
        let mut hechos = 0;
        for (red, s) in &self.redes {
            for destino in [Ipv4Addr::BROADCAST, red.difusion(), GRUPO_MULTICAST] {
                hechos += usize::from(s.send_to(paquete, (destino, self.puerto)).is_ok());
            }
        }
        if self.redes.is_empty() {
            hechos += usize::from(
                self.suelto
                    .send_to(paquete, (Ipv4Addr::BROADCAST, self.puerto))
                    .is_ok(),
            );
        }
        hechos
    }

    /// Directa a cada una de `ips`, por el socket de su red si es de alguna.
    pub fn a(&self, paquete: &[u8], ips: &[Ipv4Addr]) -> usize {
        ips.iter()
            .filter(|ip| {
                let s = self
                    .redes
                    .iter()
                    .find(|(r, _)| r.contiene(**ip))
                    .map_or(&self.suelto, |(_, s)| s);
                s.send_to(paquete, SocketAddrV4::new(**ip, self.puerto))
                    .is_ok()
            })
            .count()
    }
}

/// **Quien oye las balizas**: atado a `0.0.0.0:puerto` (la difusion solo
/// llega asi) y apuntado a la multidifusion en cada red.
pub struct Oyente {
    socket: UdpSocket,
}

impl Oyente {
    pub fn abrir(puerto: u16, redes: &[Interfaz]) -> io::Result<Oyente> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, puerto))?;
        let mut unido = false;
        for r in redes {
            unido |= socket.join_multicast_v4(&GRUPO_MULTICAST, &r.ip).is_ok();
        }
        if !unido {
            let _ = socket.join_multicast_v4(&GRUPO_MULTICAST, &Ipv4Addr::UNSPECIFIED);
        }
        socket.set_read_timeout(Some(Duration::from_millis(200)))?;
        Ok(Oyente { socket })
    }

    pub fn puerto(&self) -> io::Result<u16> {
        Ok(self.socket.local_addr()?.port())
    }

    /// Un datagrama, o `None` si no llego nada en 200 ms. Los errores de un
    /// envio anterior que Windows cuelga aqui (`WSAECONNRESET` tras un
    /// «puerto inalcanzable») se tragan: no son de este socket.
    pub fn oir(&self, buf: &mut [u8; TOPE]) -> io::Result<Option<(usize, SocketAddr)>> {
        match self.socket.recv_from(buf) {
            Ok((n, de)) => Ok(Some((n, de))),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::ConnectionReset
                        | io::ErrorKind::Interrupted
                ) =>
            {
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }
}

// ------------------------------------------------------------ el ritmo

/// **Cuando mandar la siguiente**: cada [`CADA_GRABANDO_MS`] grabando, cada
/// [`CADA_EN_REPOSO_MS`] sin grabar y al momento si cambio la grabacion
/// (empezar, pausar, seguir, acabar). Puro: el tiempo lo pone quien llama.
#[derive(Debug, Default)]
pub struct Ritmo {
    ultima: Option<i64>,
    /// Cuando empezo a grabar sin oir a nadie, y el ultimo barrido.
    a_solas_desde: Option<i64>,
    barrido: Option<i64>,
}

impl Ritmo {
    pub fn toca(&mut self, ahora: i64, grabando: bool, cambio: bool) -> bool {
        let cada = if grabando {
            CADA_GRABANDO_MS
        } else {
            CADA_EN_REPOSO_MS
        } as i64;
        let toca = cambio || self.ultima.is_none_or(|u| ahora - u >= cada || ahora < u);
        if toca {
            self.ultima = Some(ahora);
        }
        toca
    }

    /// Si toca barrer la red: grabando, sin haber oido a nadie del grupo en
    /// [`BARRER_TRAS_MS`], y cada [`BARRER_CADA_MS`] mientras siga asi.
    pub fn toca_barrer(&mut self, ahora: i64, grabando: bool, oido_hace: Option<i64>) -> bool {
        let a_solas = grabando && oido_hace.is_none_or(|h| h > BARRER_TRAS_MS);
        if !a_solas {
            self.a_solas_desde = None;
            return false;
        }
        let desde = *self.a_solas_desde.get_or_insert(ahora);
        if ahora - desde < BARRER_TRAS_MS {
            return false;
        }
        if self.barrido.is_some_and(|b| ahora - b < BARRER_CADA_MS) {
            return false;
        }
        self.barrido = Some(ahora);
        true
    }
}

// ------------------------------------------------------------ la prioridad

/// **Lo que puede querer hacer la sincronizacion**, de lo primero a lo
/// ultimo. El orden es el de `Ord`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Trabajo {
    /// El anuncio de la grabacion y su reloj, las fotos y clips del movil.
    Camara,
    /// Los ficheros de una grabacion recien acabada (la voz, el video, la
    /// nota y sus capturas): solo ese chat.
    ArchivosDeLaGrabacion,
    /// Las tareas tachadas, «al instante».
    Tareas,
    /// La vuelta entera con el grupo.
    VueltaEntera,
    /// Los chats compartidos con otras personas.
    Personas,
}

impl Trabajo {
    /// Si se puede hacer mientras alguien del grupo graba: solo la camara.
    /// Lo demas espera a que acabe (y entonces, la grabacion primero).
    pub fn mientras_se_graba(self) -> bool {
        self == Trabajo::Camara
    }
}

/// **Que toca ahora** de lo que espera: lo primero en el orden de
/// [`Trabajo`], y mientras alguien graba, solo lo de la camara.
pub fn que_toca(espera: &[Trabajo], grabando_alguien: bool) -> Option<Trabajo> {
    espera
        .iter()
        .copied()
        .filter(|t| !grabando_alguien || t.mientras_se_graba())
        .min()
}

// ------------------------------------------------------------ el diagnostico

/// Lo que se le dice al usuario si grabando no aparece ningun movil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Diagnostico {
    /// Se oye a alguien, o aun es pronto para decir nada.
    Bien,
    /// La ultima direccion del movil es de otra red que ninguna de las de
    /// este aparato: no estan en la misma Wi-Fi.
    OtraRed,
    /// Estan en la misma red y no se oye nada ni contesta: la Wi-Fi aisla a
    /// los aparatos (de invitados, de hotel, de empresa) o un cortafuegos
    /// lo para.
    Aislados,
}

/// Puro: `grabando_ms`, lo que lleva grabando; `oidos`, si se oyo alguna
/// baliza o contesto alguna sonda del grupo desde que empezo; `conocidas`,
/// las ultimas direcciones de los moviles del grupo. Se espera
/// [`ESPERA_DIAGNOSTICO_MS`] antes de decir nada.
pub fn diagnosticar(
    grabando_ms: i64,
    oidos: bool,
    redes: &[Interfaz],
    conocidas: &[Ipv4Addr],
) -> Diagnostico {
    if oidos || grabando_ms < ESPERA_DIAGNOSTICO_MS || conocidas.is_empty() {
        return Diagnostico::Bien;
    }
    if !redes.is_empty()
        && !conocidas
            .iter()
            .any(|c| redes.iter().any(|r| r.contiene(*c)))
    {
        Diagnostico::OtraRed
    } else {
        Diagnostico::Aislados
    }
}

/// Cuanto se graba sin oir a nadie antes de decirlo.
pub const ESPERA_DIAGNOSTICO_MS: i64 = 20_000;

#[cfg(test)]
mod pruebas {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    /// [`ejemplo`] con la clave de `K7Q2M9XMPA`. Calculado tambien aparte,
    /// con `hashlib`/`hmac` de Python: una prueba que use el mismo codigo que
    /// prueba no demuestra nada.
    const VECTOR: &str = "50584231010909df6bf6b40e77f0b9720000019a2520d4e0000000070000\
                          0000000b81500b70632d706f72746174696c0f672d31373631353539353030\
                          30303041e37ec235431670bb92572185a1c4e2";

    fn clave() -> [u8; 32] {
        crate::codigo::clave_de_grupo("K7Q2M9XMPA")
    }

    fn ejemplo() -> Baliza {
        Baliza {
            etiqueta: etiqueta_cruda(&clave()),
            id: "pc-portatil".into(),
            puerto: 47474,
            reloj: 1_761_559_500_000,
            secuencia: 7,
            banderas: GRABANDO | CAMARA,
            grabacion: "g-1761559500000".into(),
            ms_actual: 754_000,
        }
    }

    #[test]
    fn la_baliza_va_y_vuelve_igual() {
        let b = ejemplo();
        let bytes = b.escribir(&clave());
        assert_eq!(bytes.len(), CABEZA + 2 + 11 + 15 + LARGO_FIRMA);
        assert_eq!(Baliza::leer(&bytes, &clave()), Ok(b.clone()));
        assert!(b.graba() && b.camara() && !b.en_pausa() && !b.pregunta());
    }

    #[test]
    fn el_vector_para_android_no_cambia() {
        // Lo que tiene que sacar Android byte a byte (esta en la guia). Si
        // esto cambia, cambia el cable y hay que cambiarlo alli tambien.
        let bytes = ejemplo().escribir(&clave());
        assert_eq!(
            hex(&bytes[6..14]),
            "09df6bf6b40e77f0",
            "la etiqueta del mDNS"
        );
        assert_eq!(hex(&bytes), VECTOR);
    }

    #[test]
    fn quien_no_es_del_grupo_no_puede_fingirla() {
        let bytes = ejemplo().escribir(&clave());
        let otra = crate::codigo::clave_de_grupo("K7Q2M9XMPB");
        // Caso negativo: con otra clave ni siquiera es de este grupo.
        assert_eq!(Baliza::leer(&bytes, &otra), Err(Rechazo::OtroGrupo));
        // Alguien que copia nuestra etiqueta (es publica) y firma con otra
        // clave: la firma no cuadra.
        let falsa = Baliza {
            etiqueta: etiqueta_cruda(&clave()),
            ..ejemplo()
        };
        let mut b = falsa.escribir(&otra);
        assert_eq!(Baliza::leer(&b, &clave()), Err(Rechazo::Firma));
        // Tocar un solo byte (el puerto) tambien la rompe.
        b = bytes.clone();
        b[15] ^= 1;
        assert_eq!(Baliza::leer(&b, &clave()), Err(Rechazo::Firma));
        // Y cortada, o de otra cosa, o de otra version.
        assert_eq!(Baliza::leer(&bytes[..20], &clave()), Err(Rechazo::Rota));
        assert_eq!(Baliza::leer(b"PING", &clave()), Err(Rechazo::NoEs));
        let mut v = bytes.clone();
        v[4] = 2;
        assert_eq!(Baliza::leer(&v, &clave()), Err(Rechazo::Version));
    }

    #[test]
    fn los_textos_largos_se_recortan_sin_partir_letras() {
        let b = Baliza {
            id: "ñ".repeat(40),
            grabacion: "x".repeat(100),
            ..ejemplo()
        };
        let leida = Baliza::leer(&b.escribir(&clave()), &clave()).unwrap();
        assert_eq!(leida.id, "ñ".repeat(32));
        assert_eq!(leida.grabacion.len(), TOPE_TEXTO);
        assert!(b.escribir(&clave()).len() <= TOPE);
    }

    #[test]
    fn la_repetida_se_tira_y_la_vieja_tambien() {
        let mut o = Oidas::default();
        let b = ejemplo();
        let ahora = b.reloj + 7_000; // el otro va 7 s atrasado
        assert!(o.admitir(&b, ahora));
        assert!(!o.admitir(&b, ahora), "la misma por otro camino");
        let siguiente = Baliza {
            secuencia: 8,
            ..b.clone()
        };
        assert!(o.admitir(&siguiente, ahora));
        // Caso negativo: una grabada hace una hora y reenviada.
        let vieja = Baliza {
            id: "otro".into(),
            reloj: ahora - 3_600_000,
            ..b.clone()
        };
        assert!(!o.admitir(&vieja, ahora));
    }

    #[test]
    fn las_redes_y_su_difusion() {
        let r = Interfaz {
            ip: Ipv4Addr::new(192, 168, 1, 20),
            prefijo: 24,
        };
        assert_eq!(r.difusion(), Ipv4Addr::new(192, 168, 1, 255));
        assert!(r.contiene(Ipv4Addr::new(192, 168, 1, 99)));
        assert!(!r.contiene(Ipv4Addr::new(192, 168, 2, 99)));
        let vpn = Interfaz {
            ip: Ipv4Addr::new(10, 8, 0, 2),
            prefijo: 32,
        };
        assert_eq!(vpn.difusion(), vpn.ip);
    }

    #[test]
    fn el_barrido_empieza_por_las_conocidas_y_sus_vecinas() {
        let r = Interfaz {
            ip: Ipv4Addr::new(192, 168, 1, 20),
            prefijo: 24,
        };
        let v = barrido(
            &r,
            &[Ipv4Addr::new(192, 168, 1, 37), Ipv4Addr::new(10, 0, 0, 5)],
        );
        assert_eq!(v[0], Ipv4Addr::new(192, 168, 1, 37));
        assert_eq!(v[1], Ipv4Addr::new(192, 168, 1, 38));
        assert_eq!(v[2], Ipv4Addr::new(192, 168, 1, 36));
        assert_eq!(
            v.len(),
            253,
            "la /24 menos la propia, la de red y la de difusion"
        );
        assert!(!v.contains(&r.ip));
        assert!(!v.contains(&Ipv4Addr::new(192, 168, 1, 255)));
        assert!(!v.contains(&Ipv4Addr::new(192, 168, 1, 0)));
        assert!(!v.contains(&Ipv4Addr::new(10, 0, 0, 5)), "de otra red");
        // Una red enorme (una /16 de empresa): solo las vecinas.
        let grande = Interfaz {
            ip: Ipv4Addr::new(10, 1, 2, 3),
            prefijo: 16,
        };
        assert_eq!(barrido(&grande, &[Ipv4Addr::new(10, 1, 7, 7)]).len(), 17);
        assert!(barrido(&grande, &[]).is_empty());
    }

    #[test]
    fn el_ritmo_grabando_es_de_un_segundo_y_al_momento_si_cambia() {
        let mut r = Ritmo::default();
        assert!(r.toca(0, false, false), "la primera, ya");
        assert!(!r.toca(1_000, false, false));
        assert!(r.toca(CADA_EN_REPOSO_MS as i64, false, false));
        // Empieza a grabar: al momento, sin esperar al reposo.
        assert!(r.toca(15_100, true, true));
        assert!(!r.toca(15_600, true, false));
        assert!(r.toca(16_100, true, false));
        // Un reloj que va hacia atras no la deja muda.
        assert!(r.toca(100, true, false));
    }

    #[test]
    fn se_barre_solo_grabando_y_sin_oir_a_nadie() {
        let mut r = Ritmo::default();
        assert!(!r.toca_barrer(0, false, None));
        assert!(!r.toca_barrer(0, true, None), "aun es pronto");
        assert!(r.toca_barrer(BARRER_TRAS_MS, true, None));
        assert!(!r.toca_barrer(BARRER_TRAS_MS + 1_000, true, None));
        assert!(r.toca_barrer(BARRER_TRAS_MS + BARRER_CADA_MS, true, None));
        // Caso negativo: se oye al movil, no se barre.
        assert!(!r.toca_barrer(60_000, true, Some(800)));
    }

    #[test]
    fn grabando_solo_va_la_camara_y_al_acabar_la_grabacion_primero() {
        use Trabajo::*;
        let todo = [
            Personas,
            VueltaEntera,
            Tareas,
            ArchivosDeLaGrabacion,
            Camara,
        ];
        assert_eq!(que_toca(&todo, true), Some(Camara));
        assert_eq!(que_toca(&[Personas, VueltaEntera, Tareas], true), None);
        assert_eq!(
            que_toca(
                &[Personas, VueltaEntera, Tareas, ArchivosDeLaGrabacion],
                false
            ),
            Some(ArchivosDeLaGrabacion)
        );
        assert_eq!(
            que_toca(&[Personas, VueltaEntera, Tareas], false),
            Some(Tareas)
        );
        assert_eq!(que_toca(&[], false), None);
    }

    #[test]
    fn el_diagnostico_distingue_otra_red_de_una_wifi_que_aisla() {
        let wifi = [Interfaz {
            ip: Ipv4Addr::new(192, 168, 1, 20),
            prefijo: 24,
        }];
        let movil = [Ipv4Addr::new(192, 168, 1, 37)];
        let fuera = [Ipv4Addr::new(172, 20, 10, 2)];
        assert_eq!(diagnosticar(5_000, false, &wifi, &movil), Diagnostico::Bien);
        assert_eq!(diagnosticar(30_000, true, &wifi, &movil), Diagnostico::Bien);
        assert_eq!(
            diagnosticar(30_000, false, &wifi, &movil),
            Diagnostico::Aislados
        );
        assert_eq!(
            diagnosticar(30_000, false, &wifi, &fuera),
            Diagnostico::OtraRed
        );
        // Sin ningun movil conocido no hay nada que decir.
        assert_eq!(diagnosticar(30_000, false, &wifi, &[]), Diagnostico::Bien);
    }
}
