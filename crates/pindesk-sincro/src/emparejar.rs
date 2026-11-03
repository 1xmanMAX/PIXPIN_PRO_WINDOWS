//! **Emparejar con el codigo sin teclear direcciones** (3-nov-2026).
//!
//! El usuario: «cuando escribo el codigo de sincronizacion NO aparece el
//! aparato; tengo que escribir su direccion. Antes no pasaba: en la misma red
//! todo se conectaba solo con el codigo, y rapido». Y: «que esa pantalla
//! ensene un QR que se pueda leer con CUALQUIER camara y lleve directo a la
//! app y empareje, sin abrirla antes».
//!
//! Por que pasaba (docs/emparejar.md, «La causa»): unirse con un codigo solo
//! buscaba por **mDNS**, y desde el cambio de nombre cada anuncio, cada
//! busqueda y cada resolucion se hacen **dos veces** (`_pindesk._tcp` y
//! `_pixpin._tcp`) en los dos aparatos, mientras que `pindesk.exe` estrena
//! ruta y el cortafuegos de Windows no tiene regla para el. Ademas el PC
//! dejaba de buscar en cuanto una busqueda mDNS fallaba una vez
//! ([`buscar_mientras`] es la que no se rinde). Ahora, a la vez que mDNS:
//!
//! - **La baliza** ([`pregunta`], [`respuesta`]): quien teclea el codigo saca
//!   de el la clave del grupo y manda una baliza con «pregunta»
//!   (`baliza::PREGUNTA`) por difusion y multidifusion. Los aparatos del
//!   grupo **ya contestan** a cualquier pregunta firmada con la clave del
//!   grupo, aunque quien pregunta no este aun en la lista (el PC y Android
//!   lo hacen desde el 28-oct-2026), con su baliza directa, que trae su puerto
//!   TCP. Es un datagrama: no depende del mDNS de nadie.
//! - **El enlace del QR** ([`Enlace`]): `pindesk://emparejar?…` con el
//!   codigo, las direcciones y el puerto del PC. Quien lo lee no tiene que
//!   buscar nada: llama directamente.
//!
//! Todo lo de aqui es puro (sin sockets) salvo lo que ya hace
//! `baliza::{Emisor, Oyente}`, y se prueba en este equipo.

use std::net::Ipv4Addr;

use crate::baliza::{self, Baliza};

/// Cuanto se busca, como el movil (`withTimeoutOrNull(25_000)`).
pub const TOPE_BUSQUEDA_MS: i64 = 25_000;
/// Cada cuanto se repite la pregunta mientras se busca: la difusion se
/// pierde a veces y quien contesta puede estar abriendo PinDesk.
pub const CADA_PREGUNTA_MS: i64 = 1_000;
/// A los cuantos ms sin respuesta se pregunta a cada direccion de la red,
/// una a una (Wi-Fi que filtra la difusion): como `baliza::BARRER_TRAS_MS`.
pub const BARRER_TRAS_MS: i64 = 4_000;
/// Cuanto se ensena el QR de emparejar: el codigo es la llave del grupo, asi
/// que no se deja a la vista mas de lo que dura emparejar.
pub const VIGENCIA_QR_MS: i64 = 10 * 60_000;
/// Cuantas direcciones caben en el enlace: con mas, el QR se hace denso y
/// las camaras baratas no lo leen.
pub const TOPE_DIRECCIONES: usize = 4;

// ---------------------------------------------------------------- la baliza

/// La baliza con pregunta de quien se une: firmada con la clave que sale del
/// codigo tecleado, con su id y su puerto TCP (0 si no escucha).
pub fn pregunta(
    clave_grupo: &[u8; 32],
    yo: &str,
    puerto: u16,
    reloj: i64,
    secuencia: u32,
) -> Vec<u8> {
    Baliza {
        etiqueta: baliza::etiqueta_cruda(clave_grupo),
        id: yo.to_string(),
        puerto,
        reloj,
        secuencia,
        banderas: baliza::PREGUNTA,
        grabacion: String::new(),
        ms_actual: 0,
    }
    .escribir(clave_grupo)
}

/// Un aparato del grupo que contesto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encontrado {
    /// Su id (`Aparato::id`).
    pub id: String,
    /// Donde llamarle: la IP de origen del datagrama (la de verdad) y el
    /// puerto TCP de su baliza.
    pub host: Ipv4Addr,
    pub puerto: u16,
}

/// Si `bytes`, llegados de `de`, son la baliza de un aparato del grupo de
/// `clave_grupo` que no es `yo`: donde llamarle. Lo de otros grupos, lo mal
/// firmado y lo propio (la difusion vuelve a quien la manda) no vale.
pub fn respuesta(
    bytes: &[u8],
    de: Ipv4Addr,
    clave_grupo: &[u8; 32],
    yo: &str,
) -> Option<Encontrado> {
    let b = Baliza::leer(bytes, clave_grupo).ok()?;
    if b.id.is_empty() || b.id == yo {
        return None;
    }
    Some(Encontrado {
        id: b.id,
        host: de,
        puerto: if b.puerto == 0 {
            crate::PUERTO
        } else {
            b.puerto
        },
    })
}

/// A donde va la pregunta en la vuelta `n` (0, 1, 2…, una por
/// [`CADA_PREGUNTA_MS`]): siempre la difusion general, la de cada red y la
/// multidifusion; y, pasado [`BARRER_TRAS_MS`] sin respuesta, una vez, cada
/// direccion de cada red (`baliza::barrido`).
pub fn destinos(redes: &[baliza::Interfaz], vuelta: u32) -> Vec<Ipv4Addr> {
    let mut v = vec![Ipv4Addr::BROADCAST, baliza::GRUPO_MULTICAST];
    for r in redes {
        let d = r.difusion();
        if !v.contains(&d) {
            v.push(d);
        }
    }
    let barrer = (BARRER_TRAS_MS / CADA_PREGUNTA_MS) as u32;
    if vuelta == barrer {
        for r in redes {
            v.extend(baliza::barrido(r, &[]));
        }
    }
    v
}

// -------------------------------------------------------------- buscar

/// **Buscar sin rendirse** mientras `seguir` diga que si: cada vuelta llama a
/// `buscar` y devuelve el primero que `vale`. Un error de una vuelta (mDNS
/// que no contesto, la red que aun no esta) **no** acaba la busqueda: antes,
/// el PC hacia `buscar(...).ok()?` y el primer fallo tiraba los 25 s.
/// Quien llama decide cuanto esperar tras un error (dentro de `buscar`).
pub fn buscar_mientras<V, E>(
    mut seguir: impl FnMut() -> bool,
    mut buscar: impl FnMut() -> Result<Vec<V>, E>,
    vale: impl Fn(&V) -> bool,
) -> Option<V> {
    while seguir() {
        if let Ok(vistos) = buscar()
            && let Some(v) = vistos.into_iter().find(|v| vale(v))
        {
            return Some(v);
        }
    }
    None
}

// -------------------------------------------------------------- el enlace

/// El esquema del enlace. Se lee tambien `pixpin://` (el de antes).
pub const ESQUEMA: &str = "pindesk";
/// El anfitrion del enlace: `pindesk://emparejar?…`.
pub const ANFITRION: &str = "emparejar";
/// La version del formato del enlace (`v=1`).
pub const VERSION_ENLACE: u32 = 1;

/// **El enlace del QR de emparejar** (docs/emparejar.md, «El enlace»):
///
/// `pindesk://emparejar?v=1&c=K7Q2M9XMPA&h=192.168.1.20,10.0.0.5&p=47474&n=PinDesk%20Casa&e=1793180000000`
///
/// - `c`: el codigo del grupo, limpio (sin guion).
/// - `h`: las direcciones IPv4 de quien lo ensena, la de la ruta preferida
///   primero (como mucho [`TOPE_DIRECCIONES`]); puede faltar.
/// - `p`: su puerto TCP (si falta, [`crate::PUERTO`]).
/// - `n`: su nombre, para decir «Uniendome con…» (puede faltar).
/// - `e`: hasta cuando vale (ms desde 1970). Quien lo lee lo rechaza pasado
///   ese momento (con unos minutos de margen por los relojes); puede faltar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enlace {
    pub codigo: String,
    pub hosts: Vec<Ipv4Addr>,
    pub puerto: u16,
    pub nombre: String,
    pub expira: Option<i64>,
}

/// Margen para los relojes de los dos aparatos al mirar `e`.
pub const MARGEN_RELOJ_MS: i64 = 5 * 60_000;

impl Enlace {
    /// El texto del enlace (lo que va en el QR).
    pub fn texto(&self) -> String {
        let hosts: Vec<String> = self
            .hosts
            .iter()
            .take(TOPE_DIRECCIONES)
            .map(Ipv4Addr::to_string)
            .collect();
        let mut t = format!(
            "{ESQUEMA}://{ANFITRION}?v={VERSION_ENLACE}&c={}",
            crate::codigo::limpiar(&self.codigo)
        );
        if !hosts.is_empty() {
            t.push_str(&format!("&h={}", hosts.join(",")));
        }
        t.push_str(&format!("&p={}", self.puerto));
        if !self.nombre.trim().is_empty() {
            t.push_str(&format!("&n={}", cifrar(self.nombre.trim())));
        }
        if let Some(e) = self.expira {
            t.push_str(&format!("&e={e}"));
        }
        t
    }

    /// Lee un enlace de emparejar (`pindesk://` o `pixpin://`, anfitrion
    /// `emparejar`, sin distinguir mayusculas en esas dos partes). `None` si
    /// no es uno, si el codigo no tiene el largo del grupo o si es de una
    /// version que no se entiende. Las direcciones que no son IPv4 se saltan.
    pub fn leer(texto: &str) -> Option<Enlace> {
        let t = texto.trim();
        let (esquema, resto) = t.split_once("://")?;
        if !esquema.eq_ignore_ascii_case(ESQUEMA) && !esquema.eq_ignore_ascii_case("pixpin") {
            return None;
        }
        let (anfitrion, consulta) = resto.split_once('?').unwrap_or((resto, ""));
        if !anfitrion
            .trim_end_matches('/')
            .eq_ignore_ascii_case(ANFITRION)
        {
            return None;
        }
        let consulta = consulta.split('#').next().unwrap_or_default();
        let mut codigo = None;
        let mut hosts = Vec::new();
        let mut puerto = crate::PUERTO;
        let mut nombre = String::new();
        let mut expira = None;
        for par in consulta.split('&') {
            let (k, v) = par.split_once('=').unwrap_or((par, ""));
            let v = descifrar(v);
            match k {
                "v" => {
                    if v.parse::<u32>().ok()? > VERSION_ENLACE {
                        return None;
                    }
                }
                "c" => codigo = Some(crate::codigo::limpiar(&v)),
                "h" => {
                    hosts = v
                        .split(',')
                        .filter_map(|h| h.trim().parse::<Ipv4Addr>().ok())
                        .take(TOPE_DIRECCIONES)
                        .collect();
                }
                "p" => puerto = v.parse().ok().filter(|p| *p != 0)?,
                "n" => nombre = v,
                "e" => expira = v.parse().ok(),
                _ => {}
            }
        }
        let codigo = codigo.filter(|c| c.chars().count() == crate::codigo::LARGO)?;
        Some(Enlace {
            codigo,
            hosts,
            puerto,
            nombre,
            expira,
        })
    }

    /// Si sigue valiendo `ahora` (con [`MARGEN_RELOJ_MS`] de margen).
    pub fn vigente(&self, ahora: i64) -> bool {
        self.expira.is_none_or(|e| ahora <= e + MARGEN_RELOJ_MS)
    }
}

/// Las direcciones que van en el enlace: privadas o de enlace de una red
/// local (las que alcanza el movil en la misma Wi-Fi), la primera la de la
/// ruta preferida, sin repetir y como mucho [`TOPE_DIRECCIONES`]. Lo demas
/// (publicas, loopback) no sirve para llamar desde la misma red.
pub fn direcciones_para_el_enlace(redes: &[Ipv4Addr]) -> Vec<Ipv4Addr> {
    let mut v: Vec<Ipv4Addr> = Vec::new();
    for ip in redes {
        if (ip.is_private() || is_cgnat(*ip)) && !v.contains(ip) {
            v.push(*ip);
        }
    }
    v.truncate(TOPE_DIRECCIONES);
    v
}

/// 100.64.0.0/10: la red de muchos puntos de acceso de movil y de algunas
/// VPN de malla (Tailscale), por donde tambien se llega.
fn is_cgnat(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 100 && (o[1] & 0xc0) == 64
}

/// Cifrado de URL de lo que no es letra ASCII, cifra o `-._~` (RFC 3986).
fn cifrar(t: &str) -> String {
    let mut s = String::with_capacity(t.len());
    for b in t.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}

/// Lo contrario de [`cifrar`]; tambien `+` como espacio, que lo escriben
/// algunos lectores de QR al copiar.
fn descifrar(t: &str) -> String {
    let b = t.as_bytes();
    let mut v = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok();
                match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(x) => {
                        v.push(x);
                        i += 3;
                    }
                    None => {
                        v.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                v.push(b' ');
                i += 1;
            }
            x => {
                v.push(x);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&v).into_owned()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn clave() -> [u8; 32] {
        crate::codigo::clave_de_grupo("K7Q2M9XMPA")
    }

    #[test]
    fn la_pregunta_del_que_se_une_la_entiende_el_grupo_y_su_respuesta_dice_donde_llamar() {
        // Quien se une solo sabe el codigo: con el firma su pregunta.
        let p = pregunta(&clave(), "pc-nuevo", 0, 1_761_559_500_000, 1);
        let leida = Baliza::leer(&p, &clave()).expect("la entiende el grupo");
        assert!(leida.pregunta(), "lleva la pregunta: el grupo contesta");
        assert_eq!(leida.id, "pc-nuevo");
        // El movil del grupo contesta con su baliza normal (sin pregunta),
        // directa, con su puerto TCP.
        let suya = Baliza {
            etiqueta: baliza::etiqueta_cruda(&clave()),
            id: "movil".into(),
            puerto: 47480,
            reloj: 1_761_559_500_100,
            secuencia: 9,
            ..Default::default()
        }
        .escribir(&clave());
        let de = Ipv4Addr::new(192, 168, 1, 37);
        assert_eq!(
            respuesta(&suya, de, &clave(), "pc-nuevo"),
            Some(Encontrado {
                id: "movil".into(),
                host: de,
                puerto: 47480
            })
        );
        // Sin puerto en la baliza (un aparato de antes), el de siempre.
        let sin = Baliza {
            etiqueta: baliza::etiqueta_cruda(&clave()),
            id: "movil".into(),
            ..Default::default()
        }
        .escribir(&clave());
        assert_eq!(
            respuesta(&sin, de, &clave(), "x").map(|e| e.puerto),
            Some(crate::PUERTO)
        );
    }

    #[test]
    fn caso_negativo_ni_la_propia_ni_la_de_otro_grupo_ni_una_tocada_valen() {
        let de = Ipv4Addr::new(192, 168, 1, 37);
        // La propia vuelve por la difusion: no es nadie.
        let propia = pregunta(&clave(), "pc-nuevo", 0, 1, 1);
        assert_eq!(respuesta(&propia, de, &clave(), "pc-nuevo"), None);
        // De otro grupo (otro codigo): ni se mira la firma.
        let otra = crate::codigo::clave_de_grupo("ABCDEFGHJK");
        let ajena = pregunta(&otra, "movil-ajeno", 47474, 1, 1);
        assert_eq!(respuesta(&ajena, de, &clave(), "pc-nuevo"), None);
        // Tocada por el camino: la firma no cuadra.
        let mut tocada = pregunta(&clave(), "movil", 47474, 1, 1);
        tocada[20] ^= 1;
        assert_eq!(respuesta(&tocada, de, &clave(), "pc-nuevo"), None);
        assert_eq!(respuesta(b"PING", de, &clave(), "pc-nuevo"), None);
    }

    #[test]
    fn la_pregunta_va_a_la_difusion_y_la_multidifusion_y_una_vez_a_toda_la_red() {
        let redes = [baliza::Interfaz {
            ip: Ipv4Addr::new(192, 168, 1, 20),
            prefijo: 24,
        }];
        let primera = destinos(&redes, 0);
        assert!(primera.contains(&Ipv4Addr::BROADCAST));
        assert!(primera.contains(&baliza::GRUPO_MULTICAST));
        assert!(primera.contains(&Ipv4Addr::new(192, 168, 1, 255)));
        assert_eq!(primera.len(), 3, "sin barrer al principio");
        let barre = destinos(&redes, (BARRER_TRAS_MS / CADA_PREGUNTA_MS) as u32);
        assert!(
            barre.contains(&Ipv4Addr::new(192, 168, 1, 37)),
            "barre la red"
        );
        assert!(barre.len() > 200);
        // Caso negativo: el barrido es una sola vez.
        let despues = destinos(&redes, (BARRER_TRAS_MS / CADA_PREGUNTA_MS) as u32 + 1);
        assert_eq!(despues.len(), 3);
    }

    /// **La causa, reproducida**: el PC buscaba con `buscar(...).ok()?`, asi
    /// que si la primera busqueda mDNS fallaba (Windows ocupado, la red aun
    /// sin asentar) se rendia al momento y decia «No aparecio ningun
    /// aparato», sin haber buscado los 25 s. [`buscar_mientras`] sigue.
    #[test]
    fn un_fallo_de_una_busqueda_no_acaba_la_busqueda() {
        let mut vueltas = 0;
        let mut resultados: Vec<Result<Vec<&str>, ()>> = vec![
            Err(()),
            Ok(vec!["otro-grupo"]),
            Ok(vec!["otro-grupo", "movil"]),
        ];
        resultados.reverse();
        let hallado = buscar_mientras(
            || {
                vueltas += 1;
                vueltas <= 10
            },
            || resultados.pop().unwrap_or(Ok(vec![])),
            |v| *v == "movil",
        );
        assert_eq!(hallado, Some("movil"));
        // Lo de antes: el primer error lo acababa todo.
        let antes = (|| -> Option<&str> {
            let r: Result<Vec<&str>, ()> = Err(());
            r.ok()?.into_iter().find(|v| *v == "movil")
        })();
        assert_eq!(antes, None, "asi se perdia el aparato");
    }

    #[test]
    fn caso_negativo_sin_nadie_la_busqueda_acaba_cuando_se_le_dice() {
        let mut vueltas = 0;
        let hallado = buscar_mientras(
            || {
                vueltas += 1;
                vueltas <= 3
            },
            || Ok::<Vec<&str>, ()>(vec!["otro"]),
            |v| *v == "movil",
        );
        assert_eq!(hallado, None);
        assert_eq!(vueltas, 4);
    }

    fn ejemplo() -> Enlace {
        Enlace {
            codigo: "K7Q2M9XMPA".into(),
            hosts: vec![Ipv4Addr::new(192, 168, 1, 20), Ipv4Addr::new(10, 0, 0, 5)],
            puerto: 47474,
            nombre: "PinDesk Casa & Ñu".into(),
            expira: Some(1_793_180_000_000),
        }
    }

    #[test]
    fn el_enlace_del_qr_tiene_su_forma_y_se_lee_igual() {
        let t = ejemplo().texto();
        assert_eq!(
            t,
            "pindesk://emparejar?v=1&c=K7Q2M9XMPA&h=192.168.1.20,10.0.0.5&p=47474\
             &n=PinDesk%20Casa%20%26%20%C3%91u&e=1793180000000"
        );
        assert_eq!(Enlace::leer(&t), Some(ejemplo()));
        // El de antes y en mayusculas, con el codigo dictado con guion.
        let viejo = "PIXPIN://Emparejar?c=k7q2m-9xmpa&p=47474";
        let e = Enlace::leer(viejo).expect("pixpin:// tambien");
        assert_eq!(e.codigo, "K7Q2M9XMPA");
        assert!(e.hosts.is_empty() && e.nombre.is_empty() && e.expira.is_none());
        // Sin puerto, el de siempre; los «+» de algun lector, espacios.
        let e = Enlace::leer("pindesk://emparejar?c=K7Q2M9XMPA&n=Mi+PC").unwrap();
        assert_eq!((e.puerto, e.nombre.as_str()), (crate::PUERTO, "Mi PC"));
    }

    #[test]
    fn caso_negativo_lo_que_no_es_un_enlace_de_emparejar_no_se_lee() {
        for malo in [
            "",
            "https://emparejar?c=K7Q2M9XMPA",
            "pindesk://atajo/tareas?c=K7Q2M9XMPA",
            "pindesk://emparejar?c=K7Q2",
            "pindesk://emparejar?p=47474",
            "pindesk://emparejar?v=2&c=K7Q2M9XMPA",
            "pindesk://emparejar?c=K7Q2M9XMPA&p=0",
            "pindesk://emparejar?c=K7Q2M9XMPA&p=99999",
            "pixpin-envio:1:482913:1.2.3.4:5",
        ] {
            assert_eq!(Enlace::leer(malo), None, "{malo:?}");
        }
        // Una direccion rota se salta; las buenas se quedan.
        let e = Enlace::leer("pindesk://emparejar?c=K7Q2M9XMPA&h=nada,192.168.1.9,::1").unwrap();
        assert_eq!(e.hosts, [Ipv4Addr::new(192, 168, 1, 9)]);
    }

    #[test]
    fn el_enlace_caduca_con_margen_para_los_relojes() {
        let e = ejemplo();
        let fin = e.expira.unwrap();
        assert!(e.vigente(fin));
        assert!(e.vigente(fin + MARGEN_RELOJ_MS));
        assert!(!e.vigente(fin + MARGEN_RELOJ_MS + 1));
        assert!(Enlace { expira: None, ..e }.vigente(i64::MAX));
    }

    #[test]
    fn en_el_enlace_van_las_direcciones_de_la_red_local_y_no_otras() {
        let redes = [
            Ipv4Addr::new(192, 168, 1, 20),
            Ipv4Addr::new(8, 8, 8, 8),
            Ipv4Addr::new(192, 168, 1, 20),
            Ipv4Addr::new(100, 101, 7, 2),
            Ipv4Addr::new(127, 0, 0, 1),
            Ipv4Addr::new(10, 1, 1, 1),
            Ipv4Addr::new(172, 16, 0, 9),
            Ipv4Addr::new(172, 17, 0, 9),
        ];
        assert_eq!(
            direcciones_para_el_enlace(&redes),
            [
                Ipv4Addr::new(192, 168, 1, 20),
                Ipv4Addr::new(100, 101, 7, 2),
                Ipv4Addr::new(10, 1, 1, 1),
                Ipv4Addr::new(172, 16, 0, 9),
            ]
        );
        assert!(direcciones_para_el_enlace(&[Ipv4Addr::new(8, 8, 8, 8)]).is_empty());
    }
}
