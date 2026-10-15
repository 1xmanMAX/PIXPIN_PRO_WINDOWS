//! Sincronizar con otro aparato del grupo (Windows, Android u otro Apple).
//!
//! Es el mismo protocolo y el mismo codigo que Windows (`pixpin_sincro`:
//! `Sesion` dirige, `Respondedor` contesta) sobre el mismo almacen
//! (`pixpin_proyecto::vista::DiscoPc`). Aqui solo se pone el transporte: un
//! `TcpStream` de la biblioteca estandar, que en Mac y en iOS va sobre los
//! sockets BSD del sistema.
//!
//! # Lo que falta (fase 2)
//!
//! - **Descubrir a los demas** (Bonjour `_pixpin._tcp`, TXT `g`/`id`/`l`/`n`)
//!   lo hace Swift: busca con `NWBrowser` de Network.framework y anuncia con
//!   `DNSServiceRegister` sobre el puerto de [`Escucha`]. Aqui se recibe ya
//!   el `host` y el `puerto`. [`Nucleo::anuncio`] da el anuncio entero tal
//!   como lo hace Windows (`anunciarse`) y [`etiqueta_de_grupo`] el `g` que
//!   hay que comparar.
//! - TODO: un transporte que ponga Swift (un trait de UniFFI con
//!   `leer`/`escribir` sobre `NWConnection`), para redes donde el socket BSD
//!   no basta (IPv6 con alcance, interfaces que elige Network.framework).
//!   El protocolo ya es generico en `Read + Write`, asi que es un adaptador.
//! - TODO: el `suelto` (archivo al lienzo abierto), la galeria de capturas y
//!   los chats compartidos con personas (`PXC1`): hoy se contestan «No sé
//!   qué es», como un aparato de antes.
//!
//! Todo es **bloqueante y sin hilos propios**: Swift lo llama desde una
//! `Task` de prioridad `.utility` y, si quiere escuchar, deja una esperando
//! en [`Escucha::atender_una`].

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use pixpin_proyecto::vista::DiscoPc;
use pixpin_sincro::disco::{Disco, Identidad};
use pixpin_sincro::mensajes::Aparato;
use pixpin_sincro::protocolo::{Hecho, Respondedor, Sesion};
use pixpin_sincro::{codigo, grupo, vuelta};

use crate::{ErrorNucleo, Nucleo};

/// Lo que salio de una vuelta, para ensenarlo.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ResumenSincro {
    /// El otro aparato.
    pub otro_id: String,
    pub otro_nombre: String,
    /// Mensajes que llegaron y que se mandaron.
    pub traidos: u64,
    pub enviados: u64,
    pub archivos: u64,
    /// Bytes que pasaron por la red.
    pub bytes: u64,
    pub segundos: f64,
    /// La frase de lo hecho, como la ensena Windows.
    pub texto: String,
    /// Los avisos del final (reloj desfasado, lo que no se pudo...).
    pub avisos: Vec<String>,
}

/// `Red.conectar` de Windows: 8 s para llegar y 2 min de espera, que
/// mientras el otro decide no llega nada.
fn conectar(host: &str, puerto: u16) -> Result<TcpStream, ErrorNucleo> {
    let dir = (host, puerto)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| ErrorNucleo::Sincro {
            detalle: format!("no se encuentra {host}"),
        })?;
    let s = TcpStream::connect_timeout(&dir, Duration::from_secs(8))?;
    s.set_read_timeout(Some(Duration::from_secs(120)))?;
    s.set_nodelay(true)?;
    Ok(s)
}

/// El azar de cada conexion. Sin azar del sistema no se habla: un nonce fijo
/// repetiria claves.
fn nonce() -> Result<[u8; pixpin_sincro::canal::NONCE], ErrorNucleo> {
    let mut n = [0u8; pixpin_sincro::canal::NONCE];
    getrandom::getrandom(&mut n).map_err(|e| ErrorNucleo::Sincro {
        detalle: format!("sin azar del sistema: {e}"),
    })?;
    Ok(n)
}

/// Un numero uniforme por debajo de `n`, del azar del sistema (como
/// `SecureRandom.nextInt(n)` de Android), sin el sesgo del modulo.
fn azar_menor_que(n: usize) -> Result<usize, ErrorNucleo> {
    let n = n as u32;
    let limite = u32::MAX - u32::MAX % n;
    loop {
        let mut b = [0u8; 4];
        getrandom::getrandom(&mut b).map_err(|e| ErrorNucleo::Sincro {
            detalle: format!("sin azar del sistema: {e}"),
        })?;
        let v = u32::from_le_bytes(b);
        if v < limite {
            return Ok((v % n) as usize);
        }
    }
}

/// Un corte en el saludo es lo que hace quien no descifra: otro codigo.
fn explicar(e: pixpin_sincro::protocolo::ErrorSincro, host: &str) -> ErrorNucleo {
    if e.es_corte() {
        ErrorNucleo::Sincro {
            detalle: format!("{host} no aceptó la conexión: ¿tiene el mismo código de grupo?"),
        }
    } else {
        e.into()
    }
}

/// La etiqueta publica de un grupo: lo que se anuncia en el TXT `g` de
/// Bonjour para que los del mismo grupo se reconozcan **sin publicar el
/// codigo**. Son 60 000 vueltas de PBKDF2 (se recuerdan por codigo): no
/// llamarla en el hilo de la interfaz.
#[uniffi::export]
pub fn etiqueta_de_grupo(codigo: String) -> String {
    codigo::etiqueta(&codigo::clave_de_grupo(&codigo))
}

/// `K7Q2M-9XMPA`: el codigo como se lee y se dicta.
#[uniffi::export]
pub fn codigo_legible(codigo: String) -> String {
    grupo::legible(&codigo::limpiar(&codigo))
}

/// Si lo tecleado es un codigo de grupo valido (con o sin guion, en
/// minusculas o mayusculas).
#[uniffi::export]
pub fn codigo_valido(codigo: String) -> bool {
    grupo::valido(&codigo::limpiar(&codigo))
}

/// El tipo de servicio de Bonjour para sincronizar, sin el punto final:
/// `_pixpin._tcp` (como lo piden `NWBrowser` y `DNSServiceRegister`).
#[uniffi::export]
pub fn tipo_de_servicio() -> String {
    pixpin_sincro::SERVICIO.trim_end_matches('.').to_string()
}

/// El puerto que se intenta primero al escuchar (47474). Si esta tomado se
/// escucha en otro cualquiera ([`Nucleo::escuchar`] con 0) y se anuncia ese.
#[uniffi::export]
pub fn puerto_preferido() -> u16 {
    pixpin_sincro::PUERTO
}

/// Una entrada del TXT de Bonjour, en el orden en que se anuncia.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ParTxt {
    pub clave: String,
    pub valor: String,
}

/// **Lo que este aparato anuncia en la red**, igual que Windows
/// (`anunciarse`) y Android (`Presencia`): la instancia `PixPin <nombre>`
/// del tipo `_pixpin._tcp` con el TXT `g` (etiqueta del grupo), `id`, `l`
/// (letra) y `n` (nombre, 40 letras como mucho). El puerto lo pone quien
/// anuncia: el de su [`Escucha`].
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AnuncioBonjour {
    /// `PixPin Mac de Max`, recortado a 63 bytes (una etiqueta DNS) sin
    /// partir una letra.
    pub instancia: String,
    /// `_pixpin._tcp`.
    pub tipo: String,
    /// `g`, `id`, `l`, `n`, en ese orden.
    pub txt: Vec<ParTxt>,
    /// La etiqueta del grupo (el valor de `g`), para comparar con lo que
    /// anuncian los demas.
    pub etiqueta: String,
    /// El id de este aparato (el valor de `id`), para no verse a si mismo.
    pub id: String,
}

/// Una etiqueta DNS mide como mucho 63 bytes (RFC 1035).
const MAXIMO_ETIQUETA: usize = 63;
/// Las letras del nombre que van en el TXT `n` (como Windows).
const LETRAS_DEL_NOMBRE: usize = 40;

/// Recorta a 63 bytes sin partir una letra (`recortar_etiqueta` de Windows).
fn recortar_etiqueta(nombre: &str) -> String {
    if nombre.len() <= MAXIMO_ETIQUETA {
        return nombre.to_string();
    }
    let mut corte = MAXIMO_ETIQUETA;
    while !nombre.is_char_boundary(corte) {
        corte -= 1;
    }
    nombre[..corte].to_string()
}

/// El anuncio de una identidad con la etiqueta de su grupo ya calculada.
/// Logica pura, sin red ni PBKDF2.
fn anuncio_de(id: &str, nombre: &str, letra: Option<&str>, etiqueta: String) -> AnuncioBonjour {
    let corto: String = nombre.chars().take(LETRAS_DEL_NOMBRE).collect();
    let par = |clave: &str, valor: &str| ParTxt {
        clave: clave.into(),
        valor: valor.into(),
    };
    AnuncioBonjour {
        instancia: recortar_etiqueta(format!("PixPin {nombre}").trim()),
        tipo: tipo_de_servicio(),
        txt: vec![
            par("g", &etiqueta),
            par("id", id),
            par("l", letra.unwrap_or_default()),
            par("n", &corto),
        ],
        etiqueta,
        id: id.to_string(),
    }
}

#[uniffi::export]
impl Nucleo {
    /// **El anuncio de Bonjour de este aparato**, o `None` si no esta en
    /// ningun grupo (sin grupo no se anuncia: nadie sabria que hacer con el).
    /// Hay que volver a pedirlo al crear un grupo o unirse a uno, que cambian
    /// la etiqueta y la letra.
    ///
    /// La primera vez por codigo cuesta las 60 000 vueltas de PBKDF2 de la
    /// etiqueta (luego se recuerda): no llamarlo en el hilo de la interfaz.
    pub fn anuncio(&self) -> Result<Option<AnuncioBonjour>, ErrorNucleo> {
        let i = self.identidad()?;
        Ok(i.codigo.map(|codigo| {
            anuncio_de(
                &i.yo.id,
                &i.yo.nombre,
                i.yo.letra.as_deref(),
                etiqueta_de_grupo(codigo),
            )
        }))
    }

    /// La etiqueta del grupo de este aparato (el `g` que anuncian los
    /// suyos), o `None` sin grupo. Cuesta lo mismo que [`Nucleo::anuncio`].
    pub fn etiqueta(&self) -> Result<Option<String>, ErrorNucleo> {
        Ok(self.identidad()?.codigo.map(etiqueta_de_grupo))
    }
}

#[uniffi::export]
impl Nucleo {
    /// Crea un grupo nuevo con este aparato dentro (letra `a`) y devuelve su
    /// codigo, para ensenarlo y que los demas se unan. Como `crearGrupo` de
    /// Android.
    pub fn crear_grupo(&self) -> Result<String, ErrorNucleo> {
        let disco = DiscoPc::nuevo(self.ruta());
        let actual = disco.identidad()?;
        if actual.codigo.is_some() {
            return Err(ErrorNucleo::Argumento {
                detalle: "este aparato ya esta en un grupo".into(),
            });
        }
        let mut fallo = None;
        let codigo = grupo::nuevo_codigo(|n| {
            azar_menor_que(n).unwrap_or_else(|e| {
                fallo = Some(e);
                0
            })
        });
        if let Some(e) = fallo {
            return Err(e);
        }
        let yo = Aparato {
            letra: Some("a".into()),
            desde: crate::ahora_ms(),
            ..actual.yo
        };
        disco.guardar_identidad(&Identidad {
            yo: yo.clone(),
            codigo: Some(codigo.clone()),
            miembros: vec![yo],
        })?;
        disco.sellar()?;
        Ok(codigo)
    }

    /// **Se une al grupo** de otro aparato que escucha en `host:puerto`, con
    /// el `codigo` que ensena el. Sale con su letra; los chats se pasan
    /// despues con [`Nucleo::sincronizar_con`], como en Windows.
    pub fn unirse_a_grupo(
        &self,
        host: String,
        puerto: u16,
        codigo: String,
    ) -> Result<crate::InfoAparato, ErrorNucleo> {
        let limpio = codigo::limpiar(&codigo);
        if !grupo::valido(&limpio) {
            return Err(ErrorNucleo::Argumento {
                detalle: "el codigo de grupo son diez signos".into(),
            });
        }
        let flujo = conectar(&host, puerto)?;
        let disco = DiscoPc::nuevo(self.ruta());
        let mut s = Sesion::conectar(
            flujo,
            &disco,
            true,
            Some(&limpio),
            crate::ahora_ms,
            0,
            nonce()?,
        )
        .map_err(|e| explicar(e, &host))?;
        s.adios();
        drop(s);
        self.aparato()
    }

    /// **Una vuelta de sincronizar** con el aparato de `host:puerto`, del
    /// mismo grupo: sin preguntar, con lo elegido la ultima vez con el (o
    /// todo la primera), y sin borrar ningun proyecto, como «con todos» de
    /// Windows. Bloquea hasta terminar.
    pub fn sincronizar_con(&self, host: String, puerto: u16) -> Result<ResumenSincro, ErrorNucleo> {
        let inicio = Instant::now();
        let flujo = conectar(&host, puerto)?;
        let disco = DiscoPc::nuevo(self.ruta());
        let mut s = Sesion::conectar(flujo, &disco, false, None, crate::ahora_ms, 0, nonce()?)
            .map_err(|e| explicar(e, &host))?;
        let otro = s.otro.clone();
        let mut hecho = Hecho::default();
        let r = vuelta::una(&mut s, None, &mut hecho, "", &crate::ahora_ms, &mut |_| {});
        let bytes = s.enviados() + s.recibidos();
        if r.is_err() {
            s.soltar();
        }
        drop(s);
        // Lo que llego cambia los cuadernos: los indices de la cache, fuera.
        self.vaciar_caches();
        let v = r?;
        let segundos = inicio.elapsed().as_secs_f64();
        Ok(ResumenSincro {
            otro_id: otro.id,
            otro_nombre: otro.nombre,
            traidos: hecho.traidos as u64,
            enviados: hecho.enviados as u64,
            archivos: hecho.archivos as u64,
            bytes,
            segundos,
            texto: vuelta::contar_lo_hecho(&hecho, bytes, segundos),
            avisos: v.avisos,
        })
    }

    /// Escucha en `puerto` (0: el que de el sistema; el de siempre es
    /// 47474, [`puerto_preferido`]) para que otro aparato sincronice con
    /// este o se una a su grupo. Si el puerto pedido esta tomado, falla: quien
    /// llama vuelve a probar con 0, como Windows.
    pub fn escuchar(&self, puerto: u16) -> Result<Arc<Escucha>, ErrorNucleo> {
        // Por IPv6 con IPv4 incluido si se puede; si no, solo IPv4.
        let escucha = TcpListener::bind(("::", puerto))
            .or_else(|_| TcpListener::bind(("0.0.0.0", puerto)))?;
        let puerto = escucha.local_addr()?.port();
        Ok(Arc::new(Escucha {
            escucha,
            puerto,
            raiz: self.ruta().to_path_buf(),
            cerrada: AtomicBool::new(false),
        }))
    }
}

/// **Un puerto escuchando.** Swift deja una `Task` esperando en
/// [`Escucha::atender_una`] (bloquea sin gastar nada hasta que alguien
/// llama) y la vuelve a llamar al terminar. Lo publica en Bonjour con
/// [`Escucha::puerto`].
#[derive(uniffi::Object)]
pub struct Escucha {
    escucha: TcpListener,
    puerto: u16,
    raiz: std::path::PathBuf,
    cerrada: AtomicBool,
}

#[uniffi::export]
impl Escucha {
    /// El puerto donde escucha de verdad (el que dio el sistema si se pidio 0).
    pub fn puerto(&self) -> u16 {
        self.puerto
    }

    /// Espera a que llegue alguien y lo atiende entero: una sonda (`PING`),
    /// alguien que se une al grupo o una vuelta de sincronizar. Devuelve
    /// `false` si se cerro con [`Escucha::cerrar`] en vez de llegar nadie.
    pub fn atender_una(&self) -> Result<bool, ErrorNucleo> {
        Ok(self.atender()? != Atendido::Cerrada)
    }

    /// Como [`Escucha::atender_una`], pero dice **a quien** se atendio: una
    /// sonda (no cambia nada: no hay que releer los chats) o un aparato que
    /// saludo (sincronizo o se unio: puede haber chats nuevos). Asi Swift no
    /// relee la lista cada vez que la presencia de Windows pregunta.
    pub fn atender(&self) -> Result<Atendido, ErrorNucleo> {
        let (flujo, _) = self.escucha.accept()?;
        if self.cerrada.load(Ordering::SeqCst) {
            return Ok(Atendido::Cerrada);
        }
        atender(flujo, &self.raiz, self.puerto)
    }

    /// Deja de escuchar: la `atender_una` que este esperando vuelve con
    /// `false`. Se despierta conectandose a si mismo, que es lo unico que
    /// saca a un `accept` de su espera sin sondear.
    pub fn cerrar(&self) {
        self.cerrada.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 1], self.puerto)),
            Duration::from_millis(500),
        );
    }
}

/// Lo que hizo [`Escucha::atender`].
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Atendido {
    /// Se cerro con [`Escucha::cerrar`]: no llego nadie.
    Cerrada,
    /// Una sonda (`PING`): alguien mira si este aparato esta. No cambia nada.
    Sonda,
    /// Un aparato saludo y se hablo con el (una vuelta o una union).
    Aparato { id: String, nombre: String },
}

/// Atiende una conexion entrante (`responder` de Windows, sin la galeria ni
/// el `suelto`).
fn atender(
    mut flujo: TcpStream,
    raiz: &std::path::Path,
    mi_puerto: u16,
) -> Result<Atendido, ErrorNucleo> {
    // Los cuatro primeros dicen a quien se atiende. Se leen con un tope de
    // espera corto: quien llama los manda nada mas conectar.
    flujo.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut cuatro = [0u8; 4];
    let mut n = flujo.peek(&mut cuatro)?;
    let tope = Instant::now() + Duration::from_secs(1);
    while n > 0 && n < 4 && Instant::now() < tope {
        std::thread::sleep(Duration::from_millis(25));
        n = flujo.peek(&mut cuatro)?;
    }
    if n == 4 && &cuatro == pixpin_sincro::SONDA {
        flujo.read_exact(&mut cuatro)?;
        flujo.write_all(pixpin_sincro::SONDA_RESPUESTA)?;
        return Ok(Atendido::Sonda);
    }
    if n == 4 && &cuatro == pixpin_sincro::canal::MAGIA_DE_PERSONA {
        return Err(ErrorNucleo::Sincro {
            detalle: "los chats compartidos con personas aun no estan en Apple".into(),
        });
    }
    // Largo, como `Red.escuchar` del movil: mientras el otro decide que
    // sincronizar, aqui no llega nada.
    flujo.set_read_timeout(Some(Duration::from_secs(30 * 60)))?;
    let disco = DiscoPc::nuevo(raiz);
    let quien: std::cell::RefCell<Option<Aparato>> = std::cell::RefCell::new(None);
    let r = Respondedor {
        disco: &disco,
        estado: &|_| {},
        ahora: &crate::ahora_ms,
        mi_puerto: u32::from(mi_puerto),
        al_saludar: &|otro, _| *quien.borrow_mut() = Some(otro.clone()),
        suelto: None,
    };
    r.atender(flujo, nonce()?)?;
    let otro = quien.into_inner().unwrap_or_default();
    Ok(Atendido::Aparato {
        id: otro.id,
        nombre: otro.nombre,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_anuncio_es_el_de_windows() {
        let a = anuncio_de(
            "id-del-mac",
            "Mac de Max",
            Some("c"),
            "0123456789abcdef".into(),
        );
        assert_eq!(a.instancia, "PixPin Mac de Max");
        assert_eq!(a.tipo, "_pixpin._tcp");
        let txt: Vec<_> = a
            .txt
            .iter()
            .map(|p| (p.clave.as_str(), p.valor.as_str()))
            .collect();
        assert_eq!(
            txt,
            [
                ("g", "0123456789abcdef"),
                ("id", "id-del-mac"),
                ("l", "c"),
                ("n", "Mac de Max"),
            ]
        );
        assert_eq!(a.etiqueta, "0123456789abcdef");
        assert_eq!(a.id, "id-del-mac");
    }

    #[test]
    fn sin_letra_va_vacia_y_el_nombre_largo_se_recorta() {
        let largo = "ñ".repeat(50);
        let a = anuncio_de("id-del-mac", &largo, None, "e".into());
        // 63 bytes como mucho y sin partir la ñ (2 bytes): «PixPin » son 7.
        assert!(a.instancia.len() <= MAXIMO_ETIQUETA);
        assert_eq!(a.instancia, format!("PixPin {}", "ñ".repeat(28)));
        let valor = |c: &str| a.txt.iter().find(|p| p.clave == c).unwrap().valor.clone();
        assert_eq!(valor("l"), "");
        assert_eq!(valor("n").chars().count(), LETRAS_DEL_NOMBRE);
    }

    #[test]
    fn tipo_y_puerto_son_los_de_pixpin_sincro() {
        assert_eq!(tipo_de_servicio(), "_pixpin._tcp");
        assert_eq!(puerto_preferido(), 47474);
    }
}
