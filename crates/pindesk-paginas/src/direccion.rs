//! **Que direcciones se pueden abrir** (la guarda contra SSRF).
//!
//! El asistente abre lo que le pide un modelo, y un modelo puede haber leido
//! una pagina maliciosa. Asi que una direccion solo se abre si:
//!
//! - es `http://` o `https://` (nada de `file:`, `javascript:`, `data:`...);
//! - no lleva usuario ni contrasena (`https://yo:clave@...`);
//! - el servidor no es de este PC ni de la red de casa: ni `localhost`, ni
//!   nombres sin punto (`router`, `nas`), ni `.local`/`.lan`/`.internal`/
//!   `.home.arpa`, ni una IP privada, de enlace local, de documentacion,
//!   multicast o reservada ([`ip_publica`]), escrita como sea (`127.1`,
//!   `0x7f000001` y `2130706433` se rechazan: no son la forma normal);
//! - y, ya resuelto el nombre, **todas** sus IP son publicas
//!   ([`comprobar_ips`]): un dominio publico que apunta a `192.168.1.1`
//!   tampoco pasa. Lo mismo en cada redireccion, que se siguen a mano.
//!
//! Todo es texto y numeros: se prueba sin red.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Una direccion ya comprobada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    pub segura: bool,
    /// En minusculas; una IPv6 sin corchetes.
    pub servidor: String,
    pub puerto: u16,
    /// Ruta y consulta (`/a/b?c=1`), sin el `#fragmento`. Siempre empieza
    /// por `/` y va en ASCII (lo demas, con `%XX`).
    pub ruta: String,
}

impl Url {
    /// La direccion entera, tal como se pide.
    pub fn texto(&self) -> String {
        let esquema = if self.segura { "https" } else { "http" };
        let servidor = if self.servidor.contains(':') {
            format!("[{}]", self.servidor)
        } else {
            self.servidor.clone()
        };
        let defecto = if self.segura { 443 } else { 80 };
        if self.puerto == defecto {
            format!("{esquema}://{servidor}{}", self.ruta)
        } else {
            format!("{esquema}://{servidor}:{}{}", self.puerto, self.ruta)
        }
    }

    /// El dominio que se le ensena al usuario («🌐 leyendo ugr.es»).
    pub fn dominio(&self) -> &str {
        self.servidor.strip_prefix("www.").unwrap_or(&self.servidor)
    }

    /// La ruta sin la consulta.
    fn camino(&self) -> &str {
        self.ruta.split('?').next().unwrap_or("/")
    }

    /// La IP, si el servidor se escribio como IP.
    pub fn ip(&self) -> Option<IpAddr> {
        self.servidor.parse().ok()
    }
}

/// Los finales de nombre que son de la red de casa o de una empresa.
const FINALES_LOCALES: &[&str] = &[
    ".localhost",
    ".local",
    ".lan",
    ".home",
    ".internal",
    ".intranet",
    ".corp",
    ".home.arpa",
    ".localdomain",
];

/// **Lee y comprueba una direccion.** Sin esquema y con pinta de dominio
/// (`ugr.es/horarios`) se toma `https://`. El error va en palabras para el
/// modelo.
pub fn leer(texto: &str) -> Result<Url, String> {
    let t = texto
        .trim()
        .trim_matches(|c| matches!(c, '<' | '>' | '"' | '\'' | '«' | '»'));
    if t.is_empty() {
        return Err("falta la direccion".into());
    }
    if t.chars().any(|c| c.is_control()) {
        return Err("la direccion lleva caracteres de control".into());
    }
    let minus = t.to_ascii_lowercase();
    let (segura, resto) = if minus.starts_with("https://") {
        (true, &t[8..])
    } else if minus.starts_with("http://") {
        (false, &t[7..])
    } else if let Some(i) = t.find(':')
        && t[..i]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        && !t[i + 1..].starts_with(|c: char| c.is_ascii_digit())
    {
        return Err(format!(
            "solo se abren direcciones http:// o https:// (no «{}:»)",
            &t[..i]
        ));
    } else if t.contains('.') && !t.contains(' ') {
        (true, t)
    } else {
        return Err(format!("«{t}» no es una direccion web"));
    };
    let fin_autoridad = resto.find(['/', '?', '#']).unwrap_or(resto.len());
    let (autoridad, ruta) = resto.split_at(fin_autoridad);
    if autoridad.contains('@') {
        return Err("no se abren direcciones con usuario o contrasena".into());
    }
    let (servidor, puerto) = partir_autoridad(autoridad, segura)?;
    comprobar_servidor(&servidor)?;
    let ruta = ruta.split('#').next().unwrap_or("");
    let ruta = if ruta.is_empty() {
        "/".to_string()
    } else if ruta.starts_with('?') {
        format!("/{ruta}")
    } else {
        ruta.to_string()
    };
    Ok(Url {
        segura,
        servidor,
        puerto,
        ruta: a_ascii(&ruta),
    })
}

fn partir_autoridad(autoridad: &str, segura: bool) -> Result<(String, u16), String> {
    let defecto = if segura { 443 } else { 80 };
    let (servidor, puerto) = if let Some(r) = autoridad.strip_prefix('[') {
        let (ip, resto) = r.split_once(']').ok_or("una IPv6 sin cerrar el corchete")?;
        let puerto = match resto.strip_prefix(':') {
            Some(p) => p.parse::<u16>().map_err(|_| "el puerto no es valido")?,
            None if resto.is_empty() => defecto,
            None => return Err("la direccion no es valida".into()),
        };
        (ip.to_string(), puerto)
    } else {
        match autoridad.rsplit_once(':') {
            Some((s, "")) => (s.to_string(), defecto),
            Some((s, p)) => (
                s.to_string(),
                p.parse::<u16>().map_err(|_| "el puerto no es valido")?,
            ),
            None => (autoridad.to_string(), defecto),
        }
    };
    let servidor = servidor.trim_end_matches('.').to_ascii_lowercase();
    if servidor.is_empty() {
        return Err("falta el servidor en la direccion".into());
    }
    if puerto == 0 {
        return Err("el puerto no es valido".into());
    }
    Ok((servidor, puerto))
}

/// Lo que se comprueba del nombre o la IP del servidor, antes de resolver.
fn comprobar_servidor(servidor: &str) -> Result<(), String> {
    let no = || {
        Err(format!(
            "«{servidor}» es de este PC o de la red local: el asistente solo abre paginas publicas de internet"
        ))
    };
    if let Ok(ip) = servidor.parse::<IpAddr>() {
        return if ip_publica(ip) { Ok(()) } else { no() };
    }
    if !servidor.is_ascii() {
        return Err(format!(
            "«{servidor}» lleva letras fuera de ASCII: escribe el dominio en punycode (xn--...)"
        ));
    }
    if !servidor
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'))
        || servidor.split('.').any(str::is_empty)
    {
        return Err(format!("«{servidor}» no es un nombre de servidor valido"));
    }
    // Una IP escrita de otra forma (`127.1`, `0x7f.0.0.1`, `2130706433`):
    // el sistema la entenderia como IP; aqui no se acepta.
    let ultima = servidor.rsplit('.').next().unwrap_or("");
    if ultima.chars().all(|c| c.is_ascii_digit())
        || (ultima.starts_with("0x") && ultima[2..].chars().all(|c| c.is_ascii_hexdigit()))
    {
        return Err(format!(
            "«{servidor}» parece una IP escrita de forma rara: usa el nombre del sitio"
        ));
    }
    if servidor == "localhost"
        || !servidor.contains('.')
        || FINALES_LOCALES.iter().any(|f| servidor.ends_with(f))
    {
        return no();
    }
    Ok(())
}

/// **Si una IP es de internet** (y no de este PC, de la red de casa, de
/// documentacion, multicast o reservada).
pub fn ip_publica(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => v4_publica(v),
        IpAddr::V6(v) => v6_publica(v),
    }
}

fn v4_publica(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(a == 0
        || a == 10
        || a == 127
        || (a == 100 && (64..128).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..32).contains(&b))
        || (a == 192 && b == 0 && (c == 0 || c == 2))
        || (a == 192 && b == 88 && c == 99)
        || (a == 192 && b == 168)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 224)
}

fn v6_publica(ip: Ipv6Addr) -> bool {
    let s = ip.segments();
    // Las que llevan una IPv4 dentro: lo que diga esa IPv4.
    if let Some(v4) = ip.to_ipv4_mapped() {
        return v4_publica(v4);
    }
    if s[..6] == [0, 0, 0, 0, 0, 0] {
        // `::`, `::1` y las «compatibles» `::a.b.c.d` (obsoletas).
        return false;
    }
    let embebida =
        |hi: u16, lo: u16| Ipv4Addr::new((hi >> 8) as u8, hi as u8, (lo >> 8) as u8, lo as u8);
    if s[0] == 0x64 && s[1] == 0xff9b && s[2..6] == [0, 0, 0, 0] {
        // NAT64.
        return v4_publica(embebida(s[6], s[7]));
    }
    if s[0] == 0x2002 {
        // 6to4.
        return v4_publica(embebida(s[1], s[2]));
    }
    !((s[0] & 0xfe00) == 0xfc00 // unicas locales
        || (s[0] & 0xffc0) == 0xfe80 // de enlace local
        || (s[0] & 0xffc0) == 0xfec0 // de sitio (obsoletas)
        || (s[0] & 0xff00) == 0xff00 // multicast
        || (s[0] == 0x2001 && s[1] == 0x0db8) // documentacion
        || (s[0] == 0x2001 && s[1] == 0) // Teredo
        || s[0] == 0x100) // descarte
}

/// **Las IP a las que resolvio el nombre**: todas tienen que ser publicas
/// (si una no lo es, alguien podria hacer que se use esa).
pub fn comprobar_ips(servidor: &str, ips: &[IpAddr]) -> Result<(), String> {
    if ips.is_empty() {
        return Err(format!("no se encontro el servidor «{servidor}»"));
    }
    if ips.iter().all(|ip| ip_publica(*ip)) {
        Ok(())
    } else {
        Err(format!(
            "«{servidor}» apunta a una direccion de este PC o de la red local: no se abre"
        ))
    }
}

/// Pasa a `%XX` lo que no es ASCII imprimible (espacios, tildes), sin tocar
/// los `%XX` que ya vengan.
fn a_ascii(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_graphic() {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

/// Codifica un texto para ponerlo en la consulta de una URL (`?q=...`).
pub fn codificar(texto: &str) -> String {
    let mut o = String::with_capacity(texto.len() * 3);
    for b in texto.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                o.push(b as char)
            }
            b' ' => o.push('+'),
            _ => o.push_str(&format!("%{b:02X}")),
        }
    }
    o
}

/// Deshace `%XX` (y `+` como espacio si `mas_es_espacio`).
pub fn decodificar(texto: &str, mas_es_espacio: bool) -> String {
    let b = texto.as_bytes();
    let mut o = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                let hex = |c: u8| (c as char).to_digit(16);
                match (hex(b[i + 1]), hex(b[i + 2])) {
                    (Some(h), Some(l)) => {
                        o.push((h * 16 + l) as u8);
                        i += 3;
                        continue;
                    }
                    _ => o.push(b'%'),
                }
            }
            b'+' if mas_es_espacio => o.push(b' '),
            c => o.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&o).into_owned()
}

/// **Un enlace de la pagina hecho direccion entera**: absoluto, `//servidor`,
/// `/ruta`, `?consulta` o relativo (con `./` y `../`). `None` para lo que
/// no es una pagina (`#ancla`, `mailto:`, `javascript:`, `tel:`).
pub fn resolver(base: &Url, enlace: &str) -> Option<String> {
    let e = enlace.trim();
    if e.is_empty() || e.starts_with('#') {
        return None;
    }
    let minus = e.to_ascii_lowercase();
    if minus.starts_with("http://") || minus.starts_with("https://") {
        return Some(e.to_string());
    }
    if let Some(r) = e.strip_prefix("//") {
        return Some(format!(
            "{}://{r}",
            if base.segura { "https" } else { "http" }
        ));
    }
    if let Some(i) = e.find(':')
        && !e[..i].contains('/')
        && !e[..i].contains('?')
    {
        // Otro esquema (mailto:, javascript:, tel:, data:...).
        return None;
    }
    let origen = {
        let mut u = base.clone();
        u.ruta = String::new();
        u.texto()
    };
    let e = e.split('#').next().unwrap_or("");
    if e.starts_with('/') {
        return Some(format!("{origen}{}", limpiar_puntos(e)));
    }
    if e.starts_with('?') {
        return Some(format!("{origen}{}{e}", base.camino()));
    }
    let camino = base.camino();
    let carpeta = &camino[..camino.rfind('/').map_or(0, |i| i + 1)];
    Some(format!(
        "{origen}{}",
        limpiar_puntos(&format!("{carpeta}{e}"))
    ))
}

/// Quita los `.` y `..` de una ruta (la consulta, tal cual).
fn limpiar_puntos(ruta: &str) -> String {
    let (camino, consulta) = match ruta.find('?') {
        Some(i) => ruta.split_at(i),
        None => (ruta, ""),
    };
    let mut partes: Vec<&str> = Vec::new();
    let trozos: Vec<&str> = camino.split('/').collect();
    for (i, t) in trozos.iter().enumerate() {
        match *t {
            "." => {
                if i + 1 == trozos.len() {
                    partes.push("");
                }
            }
            ".." => {
                if partes.len() > 1 {
                    partes.pop();
                }
                if i + 1 == trozos.len() {
                    partes.push("");
                }
            }
            t => partes.push(t),
        }
    }
    let mut s = partes.join("/");
    if !s.starts_with('/') {
        s.insert(0, '/');
    }
    format!("{s}{consulta}")
}

/// **La forma de comparar dos direcciones**: sin esquema, sin `www.`, sin
/// `#`, sin la barra final y con el servidor en minusculas. Para saber si
/// una direccion ya salio en la conversacion ([`crate::vista`]).
pub fn para_comparar(texto: &str) -> Option<String> {
    let u = leer(texto).ok()?;
    let mut s = format!(
        "{}{}",
        u.dominio(),
        if u.puerto == 443 || u.puerto == 80 {
            String::new()
        } else {
            format!(":{}", u.puerto)
        }
    );
    let ruta = u.ruta.trim_end_matches('/');
    s.push_str(ruta);
    Some(s)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lee_las_direcciones_normales() {
        let u = leer("https://www.ugr.es/estudiantes/horarios?curso=2026#lunes").unwrap();
        assert!(u.segura);
        assert_eq!(u.servidor, "www.ugr.es");
        assert_eq!(u.puerto, 443);
        assert_eq!(u.ruta, "/estudiantes/horarios?curso=2026");
        assert_eq!(u.dominio(), "ugr.es");
        assert_eq!(
            u.texto(),
            "https://www.ugr.es/estudiantes/horarios?curso=2026"
        );
        let u = leer("HTTP://Example.COM:8080").unwrap();
        assert!(!u.segura);
        assert_eq!(
            (u.servidor.as_str(), u.puerto, u.ruta.as_str()),
            ("example.com", 8080, "/")
        );
        // Sin esquema: https.
        let u = leer("ugr.es/grados").unwrap();
        assert_eq!(u.texto(), "https://ugr.es/grados");
        // Tildes y espacios, en %XX.
        let u = leer("https://es.wikipedia.org/wiki/Física cuántica").unwrap();
        assert_eq!(u.ruta, "/wiki/F%C3%ADsica%20cu%C3%A1ntica");
        // Una IP publica, si.
        assert!(leer("http://93.184.215.14/").is_ok());
        assert!(leer("https://[2606:4700::1111]/").is_ok());
    }

    #[test]
    fn solo_http_y_https() {
        for mala in [
            "file:///C:/Windows/win.ini",
            "javascript:alert(1)",
            "data:text/html,<b>x</b>",
            "ftp://ftp.ejemplo.com/a",
            "mailto:yo@ejemplo.com",
            "ms-settings:privacy",
            "",
            "hola que tal",
        ] {
            assert!(leer(mala).is_err(), "{mala}");
        }
    }

    #[test]
    fn nada_de_este_pc_ni_de_la_red_de_casa() {
        for mala in [
            "http://localhost/",
            "http://localhost:8080/admin",
            "http://app.localhost/",
            "http://127.0.0.1/",
            "http://127.1/",
            "http://2130706433/",
            "http://0x7f000001/",
            "http://0x7f.0.0.1/",
            "http://017700000001/",
            "http://0.0.0.0/",
            "http://10.0.0.5/",
            "http://172.16.3.4/",
            "http://172.31.255.255/",
            "http://192.168.1.1/",
            "http://169.254.169.254/latest/meta-data/",
            "http://100.64.0.1/",
            "http://224.0.0.1/",
            "http://255.255.255.255/",
            "http://[::1]/",
            "http://[::]/",
            "http://[fe80::1]/",
            "http://[fd00::1]/",
            "http://[::ffff:127.0.0.1]/",
            "http://[::ffff:192.168.0.1]/",
            "http://[64:ff9b::a00:1]/",
            "http://[2002:c0a8:101::]/",
            "http://router/",
            "http://nas.local/",
            "http://impresora.lan/",
            "http://srv.corp/",
            "http://casa.home.arpa/",
            "http://yo:clave@ejemplo.com/",
            "https://ejemplo.com@192.168.1.1/",
        ] {
            assert!(leer(mala).is_err(), "{mala}");
        }
        // Lo publico que se parece, si.
        assert!(leer("http://172.32.0.1/").is_ok());
        assert!(leer("http://localhost.ejemplo.com/").is_ok());
        assert!(leer("https://www.local-news.com/").is_ok());
    }

    #[test]
    fn las_ip_resueltas_tambien_cuentan() {
        let publica: IpAddr = "93.184.215.14".parse().unwrap();
        let casa: IpAddr = "192.168.0.10".parse().unwrap();
        let v6: IpAddr = "2606:4700::1111".parse().unwrap();
        assert!(comprobar_ips("ejemplo.com", &[publica, v6]).is_ok());
        assert!(comprobar_ips("trampa.com", &[publica, casa]).is_err());
        assert!(comprobar_ips("trampa.com", &["::1".parse().unwrap()]).is_err());
        assert!(comprobar_ips("nada.com", &[]).is_err());
    }

    #[test]
    fn resuelve_los_enlaces_de_la_pagina() {
        let base = leer("https://ugr.es/estudiantes/grados/index.html?x=1").unwrap();
        let r = |e: &str| resolver(&base, e);
        assert_eq!(
            r("https://otra.com/a").as_deref(),
            Some("https://otra.com/a")
        );
        assert_eq!(
            r("//cdn.ugr.es/h.pdf").as_deref(),
            Some("https://cdn.ugr.es/h.pdf")
        );
        assert_eq!(r("/horarios").as_deref(), Some("https://ugr.es/horarios"));
        assert_eq!(
            r("?x=2").as_deref(),
            Some("https://ugr.es/estudiantes/grados/index.html?x=2")
        );
        assert_eq!(
            r("horario.pdf").as_deref(),
            Some("https://ugr.es/estudiantes/grados/horario.pdf")
        );
        assert_eq!(
            r("../becas/").as_deref(),
            Some("https://ugr.es/estudiantes/becas/")
        );
        assert_eq!(
            r("./a/../b#c").as_deref(),
            Some("https://ugr.es/estudiantes/grados/b")
        );
        assert_eq!(r("../../../../x").as_deref(), Some("https://ugr.es/x"));
        for nada in [
            "#arriba",
            "mailto:a@b.c",
            "javascript:void(0)",
            "tel:+34",
            "",
        ] {
            assert_eq!(r(nada), None, "{nada}");
        }
    }

    #[test]
    fn codifica_y_decodifica() {
        assert_eq!(codificar("horario ugr 2º"), "horario+ugr+2%C2%BA");
        assert_eq!(
            decodificar("https%3A%2F%2Fa.com%2Fb%3Fc%3D1", false),
            "https://a.com/b?c=1"
        );
        assert_eq!(decodificar("a+b%2", true), "a b%2");
        assert_eq!(decodificar("%zz", false), "%zz");
    }

    #[test]
    fn compara_sin_esquema_ni_www_ni_barra() {
        assert_eq!(
            para_comparar("https://www.UGR.es/horarios/#x"),
            para_comparar("http://ugr.es/horarios")
        );
        assert_ne!(
            para_comparar("https://ugr.es/a"),
            para_comparar("https://ugr.es/b")
        );
        assert_eq!(para_comparar("http://localhost/"), None);
    }
}
