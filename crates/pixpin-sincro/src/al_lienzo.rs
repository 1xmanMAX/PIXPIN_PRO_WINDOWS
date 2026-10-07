//! **Una foto suelta al lienzo que el movil tiene abierto** (6-oct-2026).
//!
//! Lo pidio el usuario: «en el chat del plugin pueda yo pegar la imagen y
//! elegir el dispositivo [...] y que se envie rapidamente y se inserte en el
//! canvas que estoy trabajando en ese momento», sin abrir Sincronizar en el
//! movil. Va por **el mismo canal cifrado del grupo** y el mismo puerto que la
//! sincronizacion: el movil ya escucha ahi mientras PixPin esta a la vista
//! (`sincro/Presencia.kt`), asi que no hace falta ni un servicio nuevo ni un
//! codigo que teclear. La guia de Android esta en
//! `docs/investigacion/2026-10-06-foto-al-lienzo-android.md`.
//!
//! El dialogo, tras el `hola` de siempre (protocolo 4, sin subirlo):
//!
//! ```text
//! PC  → {"t":"suelto","nombre":"captura.png","mime":"image/png","bytes":N,"destino":"lienzo"}
//! mov → {}                                   (vale: mandala)
//! PC  → TROZO… TROZO (N bytes)  + {"resumen":"<sha256>"}   (la cola de siempre)
//! mov → {"t":"listo","donde":"lienzo"}       (o "chat" si no habia lienzo abierto)
//! ...otra foto, otra vez desde «suelto»...
//! PC  → {"t":"adios"}   mov → {}
//! ```
//!
//! **Por que se espera el «vale» antes de los trozos**, y no se mandan detras
//! de la peticion como en `pon`: un movil de antes no conoce «suelto» y
//! contesta `No sé qué es «suelto»`; si ya le hubieran salido los trozos
//! detras, su `leerPeticion` se encontraria un TROZO donde espera JSON, lanza
//! y corta la conexion a medias. Con el «vale» delante, un movil viejo solo ve
//! una peticion que no conoce, la contesta y la conversacion sigue sana para
//! despedirse. Cuesta una ida y vuelta por foto, nada a lado de los megas.

use std::io::{self, Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::canal::{Canal, ErrorCanal, Tipo};
use crate::mensajes::{Aparato, Hola, OCUPADO, Peticion, Respuesta};
use crate::protocolo::{ErrorSincro, Salida, recibir_trozos};

/// El `t` de la peticion.
pub const SUELTO: &str = "suelto";
/// El `t` de la respuesta final.
pub const LISTO: &str = "listo";
/// A donde se quiere que vaya. Hoy solo hay uno; va escrito para que otro
/// destino futuro («al chat de tal proyecto») no necesite otra peticion.
pub const DESTINO_LIENZO: &str = "lienzo";
/// Lo mas grande que acepta el lado que recibe de esta crate. Una foto de
/// movil son pocos megas; mas de esto es un video o basura.
pub const TOPE_DE_FOTO: i64 = 64 << 20;

/// La peticion `suelto`. Los campos que Android aun no tiene en su
/// `Peticion` (`nombre`, `mime`, `destino`) los ignora un movil viejo
/// (`ignoreUnknownKeys`), que es lo que deja contestar «no se que es».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Suelto {
    pub t: String,
    pub nombre: String,
    pub mime: String,
    pub bytes: i64,
    pub destino: String,
}

impl Default for Suelto {
    fn default() -> Self {
        Suelto {
            t: SUELTO.into(),
            nombre: String::new(),
            mime: String::new(),
            bytes: 0,
            destino: DESTINO_LIENZO.into(),
        }
    }
}

/// Lo que contesta el movil a cada paso. `error` lleno es que no se hizo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Contestacion {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub donde: Option<String>,
}

/// Donde quedo la foto en el movil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Donde {
    /// En el lienzo que tenia delante, centrada en lo que se miraba.
    Lienzo,
    /// No habia lienzo abierto: a la Conversacion general, como un envio.
    Chat,
}

impl Donde {
    pub fn texto(self) -> &'static str {
        match self {
            Donde::Lienzo => "lienzo",
            Donde::Chat => "chat",
        }
    }

    /// Lo que no sea «lienzo» cuenta como chat: si el movil dice otra cosa,
    /// lo seguro es buscarla en la conversacion, que es donde acaba todo lo
    /// que llega sin sitio.
    fn de_texto(t: Option<&str>) -> Donde {
        if t == Some("lienzo") {
            Donde::Lienzo
        } else {
            Donde::Chat
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorAlLienzo {
    /// Incluye `CodigoDistinto`: el otro tiene otro codigo de grupo.
    #[error(transparent)]
    Canal(#[from] ErrorCanal),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("JSON que no se entiende: {0}")]
    Json(#[from] serde_json::Error),
    /// Un PixPin de antes: no conoce «suelto». La conexion sigue sana.
    #[error("el otro aparato aun no recibe fotos al lienzo")]
    MovilSinSoporte,
    /// Esta sincronizando con otro aparato.
    #[error("{}", OCUPADO)]
    Ocupado,
    #[error("el otro aparato habla la version {0} del protocolo")]
    OtraVersion(u32),
    /// Lo que contesto el otro como error, tal cual.
    #[error("{0}")]
    Remoto(String),
    #[error("{0}")]
    Protocolo(String),
}

impl ErrorAlLienzo {
    pub fn es_codigo_distinto(&self) -> bool {
        matches!(self, ErrorAlLienzo::Canal(ErrorCanal::CodigoDistinto))
    }
}

impl From<ErrorSincro> for ErrorAlLienzo {
    fn from(e: ErrorSincro) -> Self {
        match e {
            ErrorSincro::Canal(c) => ErrorAlLienzo::Canal(c),
            ErrorSincro::Io(i) => ErrorAlLienzo::Io(i),
            ErrorSincro::Json(j) => ErrorAlLienzo::Json(j),
            ErrorSincro::Remoto(t) => ErrorAlLienzo::Remoto(t),
            ErrorSincro::Protocolo(t) => ErrorAlLienzo::Protocolo(t),
        }
    }
}

/// Si un error del otro es el «No sé qué es «suelto»» de un PixPin de antes
/// (`Protocolo.kt`, la rama `else` del `Respondedor`; el PC dice lo mismo).
pub fn es_sin_soporte(error: &str) -> bool {
    error.starts_with("No sé qué es") && error.contains(SUELTO)
}

/// El tipo de una foto por su extension. El movil lo mira para decidir si va
/// al lienzo (una imagen) o a la conversacion (cualquier otra cosa).
pub fn mime_de(nombre: &str) -> &'static str {
    let ext = nombre
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        _ => "application/octet-stream",
    }
}

fn enviar<F: Read + Write, T: Serialize>(c: &mut Canal<F>, cosa: &T) -> Result<(), ErrorAlLienzo> {
    c.mandar(Tipo::Json, &serde_json::to_vec(cosa)?)?;
    Ok(())
}

fn leer<F: Read + Write, T: for<'a> Deserialize<'a>>(c: &mut Canal<F>) -> Result<T, ErrorAlLienzo> {
    let (tipo, datos) = c.recibir()?;
    if tipo != Tipo::Json {
        return Err(ErrorAlLienzo::Protocolo("Se esperaba una respuesta".into()));
    }
    Ok(serde_json::from_slice(&datos)?)
}

/// Un error del movil, con su tipo cuando lo tiene.
fn error_remoto(e: String) -> ErrorAlLienzo {
    if e == OCUPADO {
        ErrorAlLienzo::Ocupado
    } else if es_sin_soporte(&e) {
        ErrorAlLienzo::MovilSinSoporte
    } else {
        ErrorAlLienzo::Remoto(e)
    }
}

// ------------------------------------------------------------ quien manda

/// Una conexion ya saludada con el movil, lista para mandarle fotos.
pub struct Conexion<F: Read + Write> {
    canal: Canal<F>,
    /// El movil, tal como se presento.
    pub otro: Aparato,
    /// En que puerto escucha el, para recordarlo.
    pub puerto_del_otro: u32,
    despedida: bool,
}

impl<F: Read + Write> Conexion<F> {
    /// El saludo de siempre (`Sesion.abrir`): `hola` con quien soy y que
    /// version hablo. **No ocupa** este aparato como una vuelta de
    /// sincronizar: mandar una foto no escribe nada de aqui, y no tiene
    /// sentido que espere a que acabe otra cosa.
    pub fn abrir(
        flujo: F,
        codigo: &str,
        nonce: [u8; 32],
        hola: Hola,
    ) -> Result<Conexion<F>, ErrorAlLienzo> {
        let clave = crate::codigo::clave_de_grupo(codigo);
        let mut canal = Canal::saludar(flujo, &clave, nonce, true)?;
        enviar(
            &mut canal,
            &Peticion {
                t: "hola".into(),
                hola: Some(hola),
                ..Default::default()
            },
        )?;
        let r: Respuesta = leer(&mut canal)?;
        if let Some(e) = r.error {
            return Err(error_remoto(e));
        }
        let hola = r
            .hola
            .ok_or_else(|| ErrorAlLienzo::Protocolo("El otro aparato no saludó".into()))?;
        if hola.version != crate::VERSION {
            return Err(ErrorAlLienzo::OtraVersion(hola.version));
        }
        Ok(Conexion {
            canal,
            otro: hola.yo,
            puerto_del_otro: hola.puerto,
            despedida: false,
        })
    }

    /// Manda una foto y dice donde quedo. Un [ErrorAlLienzo::MovilSinSoporte]
    /// deja la conexion sana: no salio ningun trozo.
    pub fn mandar(&mut self, ruta: &Path, nombre: &str) -> Result<Donde, ErrorAlLienzo> {
        // Antes de pedir nada: si el fichero no esta, no se le promete al
        // movil una foto que no llegara.
        let sale = Salida::de_fichero(ruta)?;
        enviar(
            &mut self.canal,
            &Suelto {
                nombre: nombre.to_string(),
                mime: mime_de(nombre).to_string(),
                bytes: sale.largo,
                ..Default::default()
            },
        )?;
        let vale: Contestacion = leer(&mut self.canal)?;
        if let Some(e) = vale.error {
            return Err(error_remoto(e));
        }
        if sale.mandar(&mut self.canal, &mut |_| {})?.is_none() {
            // La cola ya dijo «saltado»: el movil la tira y contesta.
            let _ = leer::<_, Contestacion>(&mut self.canal);
            return Err(ErrorAlLienzo::Protocolo(
                "La imagen cambió mientras se mandaba".into(),
            ));
        }
        let listo: Contestacion = leer(&mut self.canal)?;
        if let Some(e) = listo.error {
            return Err(error_remoto(e));
        }
        Ok(Donde::de_texto(listo.donde.as_deref()))
    }

    /// Se despide (`adios`). Si la conexion ya se rompio, no pasa nada.
    pub fn adios(mut self) {
        self.despedirse();
    }

    fn despedirse(&mut self) {
        if self.despedida {
            return;
        }
        self.despedida = true;
        if enviar(&mut self.canal, &Peticion::de("adios")).is_ok() {
            let _ = leer::<_, Respuesta>(&mut self.canal);
        }
    }
}

impl<F: Read + Write> Drop for Conexion<F> {
    fn drop(&mut self) {
        // Sin despedirse, el `Respondedor` del movil se quedaria esperando la
        // siguiente peticion hasta agotar su espera.
        self.despedirse();
    }
}

// ------------------------------------------------------------ quien recibe

/// **El lado del movil**, para las pruebas y como modelo de lo que hay que
/// escribir en Android: atiende una peticion `suelto` ya leida. `guardar`
/// recibe la peticion y los bytes y dice donde los puso, o por que no.
pub fn responder<F: Read + Write>(
    canal: &mut Canal<F>,
    p: &Suelto,
    guardar: &mut dyn FnMut(&Suelto, Vec<u8>) -> Result<Donde, String>,
) -> Result<(), ErrorAlLienzo> {
    if !(0..=TOPE_DE_FOTO).contains(&p.bytes) {
        // Sin «vale» no llega ningun trozo: basta con decirlo.
        return enviar(
            canal,
            &Contestacion {
                error: Some("La imagen es demasiado grande".into()),
                ..Default::default()
            },
        );
    }
    enviar(canal, &Contestacion::default())?;
    let mut bytes = Vec::with_capacity(p.bytes as usize);
    let entera = recibir_trozos(canal, p.bytes, &mut bytes, &mut |_| {})?;
    let r = match entera {
        None => Contestacion {
            error: Some("La imagen cambió mientras se mandaba".into()),
            ..Default::default()
        },
        Some(_) => match guardar(p, bytes) {
            Ok(d) => Contestacion {
                t: Some(LISTO.into()),
                donde: Some(d.texto().into()),
                ..Default::default()
            },
            Err(e) => Contestacion {
                error: Some(e),
                ..Default::default()
            },
        },
    };
    enviar(canal, &r)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::net::{TcpListener, TcpStream};
    use std::path::PathBuf;
    use std::thread::JoinHandle;

    const CODIGO: &str = "ABCD2345";

    /// Que clase de movil hay al otro lado.
    #[derive(Clone, Copy)]
    enum Movil {
        /// Con la guia hecha; `lienzo`: si tiene un lienzo delante.
        Nuevo { lienzo: bool },
        /// Un PixPin de antes del 6-oct: «No sé qué es».
        Viejo,
        /// Sincronizando con otro: «ocupado» a todo, como el `Respondedor`.
        Ocupado,
        /// Otra version del protocolo en el saludo.
        Version(u32),
    }

    #[derive(Debug, Default)]
    struct Recibido {
        fotos: Vec<(Suelto, Vec<u8>)>,
        despedido: bool,
        /// Un TROZO donde el movil viejo esperaba una peticion: se le habria
        /// cortado la conversacion.
        trozo_inesperado: bool,
    }

    fn aparato(id: &str, nombre: &str) -> Aparato {
        Aparato {
            id: id.into(),
            nombre: nombre.into(),
            letra: Some("B".into()),
            desde: 0,
        }
    }

    fn mi_hola() -> Hola {
        Hola {
            yo: aparato("pc", "Portatil"),
            reloj: 1,
            ..Default::default()
        }
    }

    /// Un movil de mentira en 127.0.0.1 que atiende UNA conexion como el
    /// `Respondedor` de Android, con la parte nueva de [responder].
    fn movil(codigo: &'static str, clase: Movil) -> (u16, JoinHandle<Recibido>) {
        let escucha = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let puerto = escucha.local_addr().unwrap().port();
        let hilo = std::thread::spawn(move || {
            let mut rec = Recibido::default();
            let (flujo, _) = escucha.accept().unwrap();
            let clave = crate::codigo::clave_de_grupo(codigo);
            let Ok(mut c) = Canal::saludar(flujo, &clave, [2; 32], false) else {
                return rec;
            };
            let Ok(hola) = leer::<_, Peticion>(&mut c) else {
                return rec;
            };
            assert!(hola.hola.is_some(), "lo primero es el saludo");
            let version = match clase {
                Movil::Version(v) => v,
                _ => crate::VERSION,
            };
            enviar(
                &mut c,
                &Respuesta {
                    hola: Some(Hola {
                        yo: aparato("tel", "Telefono"),
                        version,
                        puerto: 47474,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
            loop {
                let Ok((tipo, datos)) = c.recibir() else {
                    return rec;
                };
                if tipo != Tipo::Json {
                    // `leerPeticion` lanza IOException y corta.
                    rec.trozo_inesperado = true;
                    return rec;
                }
                let v: serde_json::Value = serde_json::from_slice(&datos).unwrap();
                let t = v["t"].as_str().unwrap_or_default().to_string();
                if t == "adios" {
                    enviar(&mut c, &Respuesta::default()).unwrap();
                    rec.despedido = true;
                    return rec;
                }
                match clase {
                    Movil::Ocupado => {
                        let r = Respuesta {
                            error: Some(OCUPADO.into()),
                            ..Default::default()
                        };
                        enviar(&mut c, &r).unwrap();
                    }
                    Movil::Nuevo { lienzo } if t == SUELTO => {
                        let p: Suelto = serde_json::from_value(v).unwrap();
                        let fotos = &mut rec.fotos;
                        responder(&mut c, &p, &mut |p, b| {
                            fotos.push((p.clone(), b));
                            Ok(if lienzo { Donde::Lienzo } else { Donde::Chat })
                        })
                        .unwrap();
                    }
                    _ => {
                        let r = Respuesta {
                            error: Some(format!("No sé qué es «{t}»")),
                            ..Default::default()
                        };
                        enviar(&mut c, &r).unwrap();
                    }
                }
            }
        });
        (puerto, hilo)
    }

    fn foto(nombre: &str, bytes: &[u8]) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-al-lienzo-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let r = d.join(nombre);
        std::fs::write(&r, bytes).unwrap();
        r
    }

    fn conectar(puerto: u16, codigo: &str) -> Result<Conexion<TcpStream>, ErrorAlLienzo> {
        let flujo = TcpStream::connect(("127.0.0.1", puerto)).unwrap();
        Conexion::abrir(flujo, codigo, [1; 32], mi_hola())
    }

    #[test]
    fn una_foto_llega_entera_al_lienzo_del_movil() {
        let (puerto, hilo) = movil(CODIGO, Movil::Nuevo { lienzo: true });
        // Mas de un tramo (1 MiB), para que vaya troceada de verdad.
        let bytes: Vec<u8> = (0..(1 << 20) + 777).map(|i| (i % 251) as u8).collect();
        let ruta = foto("grande.png", &bytes);
        let mut c = conectar(puerto, CODIGO).unwrap();
        assert_eq!(c.otro.nombre, "Telefono");
        assert_eq!(c.puerto_del_otro, 47474);
        assert_eq!(c.mandar(&ruta, "captura.png").unwrap(), Donde::Lienzo);
        c.adios();
        let rec = hilo.join().unwrap();
        assert!(rec.despedido);
        assert_eq!(rec.fotos.len(), 1);
        let (p, b) = &rec.fotos[0];
        assert_eq!(p.nombre, "captura.png");
        assert_eq!(p.mime, "image/png");
        assert_eq!(p.destino, "lienzo");
        assert_eq!(p.bytes as usize, bytes.len());
        assert_eq!(b, &bytes);
    }

    #[test]
    fn varias_fotos_van_una_tras_otra_en_la_misma_conexion() {
        // Sin lienzo delante: el movil las deja en el chat y lo dice.
        let (puerto, hilo) = movil(CODIGO, Movil::Nuevo { lienzo: false });
        let a = foto("a.jpg", b"jpg a");
        let b = foto("b.bmp", b"BM b");
        let mut c = conectar(puerto, CODIGO).unwrap();
        assert_eq!(c.mandar(&a, "a.jpg").unwrap(), Donde::Chat);
        assert_eq!(c.mandar(&b, "b.bmp").unwrap(), Donde::Chat);
        drop(c); // al soltarla tambien se despide
        let rec = hilo.join().unwrap();
        assert!(rec.despedido);
        let nombres: Vec<_> = rec.fotos.iter().map(|(p, _)| p.nombre.as_str()).collect();
        assert_eq!(nombres, ["a.jpg", "b.bmp"]);
        assert_eq!(rec.fotos[0].0.mime, "image/jpeg");
        assert_eq!(rec.fotos[1].1, b"BM b");
    }

    #[test]
    fn caso_negativo_un_movil_viejo_dice_que_no_sabe_y_no_le_llega_ningun_trozo() {
        let (puerto, hilo) = movil(CODIGO, Movil::Viejo);
        let ruta = foto("vieja.png", b"png");
        let mut c = conectar(puerto, CODIGO).unwrap();
        let e = c.mandar(&ruta, "vieja.png").unwrap_err();
        assert!(matches!(e, ErrorAlLienzo::MovilSinSoporte), "{e:?}");
        // La conexion sigue sana: se despide bien.
        c.adios();
        let rec = hilo.join().unwrap();
        assert!(
            !rec.trozo_inesperado,
            "el movil viejo cortaria con un TROZO"
        );
        assert!(rec.despedido);
        assert!(rec.fotos.is_empty());
    }

    #[test]
    fn caso_negativo_con_otro_codigo_de_grupo_el_movil_corta_y_no_recibe_nada() {
        // El primero que descifra es el movil (nuestro `hola`): no puede y
        // cierra, como `Respondedor.atender`. Aqui solo se ve el corte.
        let (puerto, hilo) = movil("ZZZZ9999", Movil::Nuevo { lienzo: true });
        let e = conectar(puerto, CODIGO).err().unwrap();
        assert!(
            matches!(e, ErrorAlLienzo::Canal(_) | ErrorAlLienzo::Io(_)),
            "{e:?}"
        );
        assert!(hilo.join().unwrap().fotos.is_empty());
    }

    #[test]
    fn caso_negativo_otra_version_u_ocupado_se_dicen_con_su_nombre() {
        let (puerto, hilo) = movil(CODIGO, Movil::Version(3));
        let e = conectar(puerto, CODIGO).err().unwrap();
        assert!(matches!(e, ErrorAlLienzo::OtraVersion(3)), "{e:?}");
        let _ = hilo.join();

        let (puerto, hilo) = movil(CODIGO, Movil::Ocupado);
        let ruta = foto("ocupado.png", b"png");
        let mut c = conectar(puerto, CODIGO).unwrap();
        let e = c.mandar(&ruta, "ocupado.png").unwrap_err();
        assert!(matches!(e, ErrorAlLienzo::Ocupado), "{e:?}");
        c.adios();
        assert!(hilo.join().unwrap().fotos.is_empty());
    }

    #[test]
    fn caso_negativo_un_fichero_que_no_esta_no_se_le_promete_al_movil() {
        let (puerto, hilo) = movil(CODIGO, Movil::Nuevo { lienzo: true });
        let mut c = conectar(puerto, CODIGO).unwrap();
        let e = c.mandar(Path::new("Z:\\no\\existe.png"), "existe.png");
        assert!(matches!(e, Err(ErrorAlLienzo::Io(_))), "{e:?}");
        // Y la siguiente sigue pudiendo ir.
        let ruta = foto("despues.png", b"png");
        assert_eq!(c.mandar(&ruta, "despues.png").unwrap(), Donde::Lienzo);
        c.adios();
        assert_eq!(hilo.join().unwrap().fotos.len(), 1);
    }

    #[test]
    fn la_peticion_y_las_respuestas_son_las_de_la_guia() {
        let p = Suelto {
            nombre: "c.png".into(),
            mime: "image/png".into(),
            bytes: 5,
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_string(&p).unwrap(),
            r#"{"t":"suelto","nombre":"c.png","mime":"image/png","bytes":5,"destino":"lienzo"}"#
        );
        assert_eq!(
            serde_json::to_string(&Contestacion::default()).unwrap(),
            "{}"
        );
        let listo: Contestacion =
            serde_json::from_str(r#"{"t":"listo","donde":"chat","otro":1}"#).unwrap();
        assert_eq!(Donde::de_texto(listo.donde.as_deref()), Donde::Chat);
        assert!(es_sin_soporte("No sé qué es «suelto»"));
        // Caso negativo: otro error cualquiera no es «sin soporte».
        assert!(!es_sin_soporte("No sé qué es «otra»"));
        assert!(!es_sin_soporte("La imagen es demasiado grande"));
        assert_eq!(mime_de("FOTO.JPEG"), "image/jpeg");
        assert_eq!(mime_de("sin extension"), "application/octet-stream");
    }
}
