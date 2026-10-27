//! **El movil como camara de la reunion del PC** (27-oct-2026).
//!
//! El usuario: «ya que estamos en Windows, agregar una funcion especial
//! para conectar con Android, para que el MOVIL tome las fotos y se envien
//! directamente y se adjunten aqui» (a la reunion que se esta grabando en el
//! PC). Y despues: «primero el PC le pasa al movil el tiempo de la
//! grabacion, para que el movil marque cada foto con el minuto de la
//! grabacion en que se hizo; luego la manda y el PC la pone justo ahi».
//!
//! No es un cable nuevo: va por **el mismo canal cifrado del grupo** y el
//! mismo `suelto` de [`crate::al_lienzo`] (una foto, el «vale», los trozos de
//! siempre y el «listo»). Solo cambian tres cosas, y las tres solo si los dos
//! dicen [`CAPACIDAD`] (`camara1`) en el `hola`:
//!
//! 1. **`camara_remota`** ([`PETICION`]): el estado de la grabacion del PC
//!    ([`Anuncio`]): que se graba, su titulo, en que milisegundo va
//!    (`ms_actual`), si esta en pausa, y el reloj del PC (`reloj`) para medir
//!    el desfase de los relojes como NTP ([`desfase`]). Va en los dos
//!    sentidos con el mismo `t`:
//!    - el **movil pregunta** (dirige; el PC responde con su `Respondedor`):
//!      `{"t":"camara_remota","reloj":t0}` → el anuncio con `reloj: t1`.
//!    - el **PC avisa** (dirige; el movil responde) al empezar, pausar,
//!      seguir y acabar: el anuncio con `reloj: t0` → `{"reloj":t1}`.
//! 2. **Las fotos** son un `suelto` con `destino: "grabacion"` ([`DESTINO`])
//!    y el minuto **ya calculado en el movil** (`ms`), mas la hora de su
//!    reloj (`tomada_en`) y su desfase medido (`reloj`) para quien no mande
//!    `ms`. El «listo» dice donde quedo: `{"t":"listo","donde":"grabacion",
//!    "ms":754000}`.
//! 3. Lo que no es una foto, o es demasiado grande, se niega antes del
//!    «vale» ([`aceptar`]) y se vuelve a mirar por sus primeros bytes al
//!    llegar ([`es_imagen`]): no sale ningun trozo y la conexion sigue sana.
//!
//! Un aparato de antes contesta «No sé qué es «camara_remota»» y la
//! conversacion sigue: el movil no ensena la camara y ya esta. La guia
//! completa para Android esta en `docs/movil-como-camara.md`.
//!
//! El reloj de la grabacion ([`Reloj`]) y el minuto de cada foto
//! ([`ms_de_la_foto`]) son logica pura: los mismos numeros en el PC y en las
//! pruebas, y la regla que tiene que copiar el movil.

use std::io::{Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::al_lienzo::{Conexion, Contestacion, ErrorAlLienzo, LISTO, Suelto, mime_de};
use crate::canal::Tipo;

/// Lo que dicen en el `hola` los que saben hacer de camara y recibirla.
pub const CAPACIDAD: &str = "camara1";
/// El `t` de la peticion (en los dos sentidos) y del anuncio.
pub const PETICION: &str = "camara_remota";
/// El `destino` de un `suelto` que es una foto para la grabacion.
pub const DESTINO: &str = "grabacion";
/// Lo que dice el «listo» de una foto que entro en la grabacion.
pub const DONDE: &str = "grabacion";
/// Lo mas grande que se acepta: una foto de 200 megapixeles en JPEG cabe de
/// sobra; un video, no (no es para eso).
pub const TOPE_DE_FOTO: i64 = 64 << 20;
/// Lo mas grande de un clip de video: dos minutos a 6 Mbit/s son unos
/// 90 MB; con margen para un movil que se pase un poco.
pub const TOPE_DE_CLIP: i64 = 256 << 20;
/// Lo mas largo de un clip que se pide al movil.
pub const CLIP_MAXIMO_MS: i64 = 2 * 60 * 1000;
/// Lo que se acepta. HEIC no: el movil la pasa a JPEG antes (el PC no la
/// sabe ensenar sin el codec de pago de Windows). Los clips, en MP4.
pub const TIPOS: [&str; 4] = ["image/jpeg", "image/png", "image/webp", "video/mp4"];
/// Los codecs de video que puede decir el PC en [`Anuncio::codecs`].
pub const H264: &str = "h264";
pub const HEVC: &str = "hevc";

fn es_cero(v: &i64) -> bool {
    *v == 0
}

fn es_falso(v: &bool) -> bool {
    !*v
}

/// **El estado de la grabacion del PC**, como viaja en [`PETICION`]. Los
/// campos con su valor por omision no se escriben (`encodeDefaults =
/// false`, como el resto del protocolo).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Anuncio {
    pub t: String,
    /// El id de la grabacion (no el de la nota de voz, que aun no existe:
    /// la nota entra al «Terminar»). Vacio: el PC no esta grabando.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub grabacion: String,
    /// «Clase 27-10 10h05», lo que ensena el movil.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub titulo: String,
    /// El nombre del PC, para «Cámara para la reunión del PC».
    #[serde(skip_serializing_if = "String::is_empty")]
    pub aparato: String,
    /// Grabando ahora (aunque sea en pausa). `false` con `grabacion`: ya
    /// acabo, y las fotos que queden por subir de entonces aun entran.
    #[serde(skip_serializing_if = "es_falso")]
    pub activa: bool,
    #[serde(skip_serializing_if = "es_falso")]
    pub en_pausa: bool,
    /// **En que milisegundo de la grabacion va** al escribir este anuncio.
    /// Con el, el movil lleva su propio reloj de la grabacion
    /// ([`Reloj::seguir_a`]).
    #[serde(skip_serializing_if = "es_cero")]
    pub ms_actual: i64,
    /// Lo que duro, cuando ya acabo.
    #[serde(skip_serializing_if = "es_cero")]
    pub duracion: i64,
    /// Cuando empezo, en el reloj del PC.
    #[serde(skip_serializing_if = "es_cero")]
    pub desde: i64,
    /// Las pausas, `[desde, hasta]` en el reloj del PC (`hasta` 0: sigue).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pausas: Vec<[i64; 2]>,
    /// El reloj de quien escribe este JSON, al escribirlo: `t0` o `t1` de la
    /// medida del desfase.
    #[serde(skip_serializing_if = "es_cero")]
    pub reloj: i64,
    /// **Los codecs de video que el PC sabe ver** ([`H264`], [`HEVC`]): el
    /// movil graba los clips en HEVC solo si esta; si no, en H.264. Vacio
    /// (un PC sin clips): solo fotos.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub codecs: Vec<String>,
    /// Lo mas largo de un clip, en ms.
    #[serde(rename = "clipMaximoMs", skip_serializing_if = "es_cero")]
    pub clip_maximo_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Anuncio {
    /// El anuncio vacio: «no estoy grabando».
    pub fn nada() -> Anuncio {
        Anuncio {
            t: PETICION.into(),
            ..Default::default()
        }
    }

    /// Si hay una grabacion a la que mandar fotos ahora mismo.
    pub fn grabando(&self) -> bool {
        self.activa && !self.grabacion.is_empty()
    }

    /// Si el PC recibe clips de video.
    pub fn con_clips(&self) -> bool {
        !self.codecs.is_empty()
    }

    /// En que graba el movil un clip para este PC: HEVC si lo sabe ver,
    /// H.264 si no. `None`: no recibe clips.
    pub fn codec_del_clip(&self) -> Option<&'static str> {
        if self.codecs.iter().any(|c| c == HEVC) {
            Some(HEVC)
        } else if self.codecs.iter().any(|c| c == H264) {
            Some(H264)
        } else {
            None
        }
    }
}

/// **El desfase entre dos relojes, como NTP**: `t0` sale de aqui, `t1` es el
/// reloj del otro al contestar, `t2` vuelve aqui. Lo que va adelantado el
/// otro (negativo: atrasado), suponiendo que la ida y la vuelta tardan lo
/// mismo.
pub fn desfase(t0: i64, t1: i64, t2: i64) -> i64 {
    t1 - (t0 + t2.max(t0)) / 2
}

// ------------------------------------------------------------ el reloj

/// **El reloj de una grabacion**: cuando empezo y sus pausas, en el reloj
/// de quien lo lleva. Pasa una hora de pared a un milisegundo de la
/// grabacion ([`Reloj::ms_en`]): lo pausado no cuenta.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reloj {
    pub desde: i64,
    /// `(desde, hasta)`; `hasta` `None`: en pausa ahora.
    pub pausas: Vec<(i64, Option<i64>)>,
    /// Cuando acabo.
    pub fin: Option<i64>,
}

impl Reloj {
    /// Empieza a contar en `ahora`, que ya va por `ms_ya`.
    pub fn empezar(ahora: i64, ms_ya: i64) -> Reloj {
        Reloj {
            desde: ahora - ms_ya.max(0),
            ..Default::default()
        }
    }

    pub fn en_pausa(&self) -> bool {
        self.pausas.last().is_some_and(|(_, h)| h.is_none())
    }

    pub fn acabado(&self) -> bool {
        self.fin.is_some()
    }

    pub fn pausar(&mut self, ahora: i64) {
        if !self.en_pausa() && !self.acabado() {
            self.pausas.push((ahora, None));
        }
    }

    pub fn seguir(&mut self, ahora: i64) {
        if let Some((d, h @ None)) = self.pausas.last_mut() {
            *h = Some(ahora.max(*d));
        }
    }

    pub fn acabar(&mut self, ahora: i64) {
        if self.fin.is_none() {
            self.seguir(ahora);
            self.fin = Some(ahora.max(self.desde));
        }
    }

    /// Lo que estuvo en pausa antes de `t`.
    fn pausado_hasta(&self, t: i64) -> i64 {
        self.pausas
            .iter()
            .map(|(d, h)| {
                let fin = h.unwrap_or(t).min(t);
                (fin - *d).max(0)
            })
            .sum()
    }

    /// **El milisegundo de la grabacion en la hora de pared `t`**: lo que
    /// corrio desde el principio menos lo pausado. Antes de empezar, 0; tras
    /// acabar, lo que duro.
    pub fn ms_en(&self, t: i64) -> i64 {
        let t = self.fin.map_or(t, |f| t.min(f));
        if t <= self.desde {
            return 0;
        }
        (t - self.desde - self.pausado_hasta(t)).max(0)
    }

    /// Lo que lleva (o lo que duro).
    pub fn duracion(&self, ahora: i64) -> i64 {
        self.ms_en(ahora)
    }

    /// **Lo pone de acuerdo con quien manda** (la grabadora del PC, o el
    /// anuncio en el movil): en `ahora` va por `ms` y esta o no en pausa. Una
    /// diferencia de menos de `tolerancia` se deja: no hace falta mover el
    /// minuto a cada fotograma.
    pub fn seguir_a(&mut self, ahora: i64, ms: i64, pausa: bool, tolerancia: i64) {
        if self.acabado() {
            return;
        }
        if pausa {
            self.pausar(ahora);
        } else {
            self.seguir(ahora);
        }
        let mio = self.ms_en(ahora);
        if (mio - ms).abs() > tolerancia {
            self.desde += mio - ms.max(0);
        }
    }

    /// Las pausas como viajan en el [`Anuncio`].
    pub fn pausas_al_cable(&self) -> Vec<[i64; 2]> {
        self.pausas
            .iter()
            .map(|(d, h)| [*d, h.unwrap_or(0)])
            .collect()
    }
}

/// **El minuto de la grabacion de una foto** que llega a quien graba, con el
/// reloj de la grabacion y la hora de llegada (`ahora`):
///
/// 1. con `ms` (lo calculo el movil con el tiempo que le paso el PC): ese,
///    tal cual;
/// 2. sin `ms` y con `tomada_en` (un movil que solo sabe su hora): su hora
///    pasada al reloj del PC con el desfase (el que mando, `reloj`, o el que
///    se midio aqui, `medido`), y de ahi al minuto con las pausas;
/// 3. sin nada: el minuto en que llega.
///
/// Siempre dentro de `[0, lo que lleva o duro]`: una foto no puede ser de
/// despues del final, ni de antes del principio. `duracion`, si se sabe la
/// de verdad (la de la nota), manda sobre la del reloj.
pub fn ms_de_la_foto(
    reloj: &Reloj,
    ahora: i64,
    duracion: Option<i64>,
    foto: &Suelto,
    medido: Option<i64>,
) -> i64 {
    let tope = duracion.unwrap_or_else(|| reloj.duracion(ahora)).max(0);
    let ms = match foto.ms {
        Some(ms) => ms,
        None if foto.tomada_en > 0 => {
            let desfase = if foto.reloj != 0 {
                foto.reloj
            } else {
                medido.unwrap_or(0)
            };
            reloj.ms_en(foto.tomada_en - desfase)
        }
        None => reloj.ms_en(ahora),
    };
    ms.clamp(0, tope)
}

/// **Lo que dura un clip** en la grabacion: lo que dice el movil, sin pasar
/// de [`CLIP_MAXIMO_MS`] con margen ni del final de la grabacion (`tope`,
/// lo que lleva o duro) desde donde empezo (`ms`). Sin duracion, 0 (se
/// ensena como una foto).
pub fn duracion_del_clip(p: &Suelto, ms: i64, tope: i64) -> i64 {
    p.duracion_ms
        .clamp(0, 2 * CLIP_MAXIMO_MS)
        .min((tope - ms).max(0))
}

/// Si el `suelto` es un clip de video (y no una foto).
pub fn es_clip(p: &Suelto) -> bool {
    p.mime.starts_with("video/")
}

// ------------------------------------------------------------ comprobar

/// **Antes del «vale»**: que sea para una grabacion, una foto de un tipo
/// que se sabe ensenar y de un tamano razonable. `Err` con el motivo.
pub fn aceptar(p: &Suelto) -> Result<(), String> {
    if p.grabacion.trim().is_empty() {
        return Err("La foto no dice de qué grabación es".into());
    }
    if !TIPOS.contains(&p.mime.as_str()) {
        return Err(SOLO_FOTOS_Y_CLIPS.into());
    }
    let tope = if es_clip(p) {
        TOPE_DE_CLIP
    } else {
        TOPE_DE_FOTO
    };
    if !(1..=tope).contains(&p.bytes) {
        return Err("La foto o el clip es demasiado grande".into());
    }
    Ok(())
}

/// **Donde espera lo llegado de una toma** que se puede reanudar: por su
/// `clave` y su largo (otra toma con la misma clave y otro largo empieza de
/// cero).
pub fn parcial(carpeta: &Path, clave: &str, bytes: i64) -> std::path::PathBuf {
    let limpia: String = clave
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(80)
        .collect();
    carpeta
        .join("parciales")
        .join(format!("{limpia}-{bytes}.parcial"))
}

/// Lo que se contesta a lo que no es una foto ni un clip.
pub const SOLO_FOTOS_Y_CLIPS: &str = "Solo se aceptan fotos (JPEG, PNG o WebP) y clips MP4";

/// **Que de verdad es una foto**, por sus primeros bytes: el `mime` lo dice
/// quien manda, y esto no.
pub fn es_imagen(cabeza: &[u8]) -> bool {
    cabeza.starts_with(&[0xFF, 0xD8, 0xFF])
        || cabeza.starts_with(b"\x89PNG\r\n\x1a\n")
        || (cabeza.len() >= 12 && &cabeza[..4] == b"RIFF" && &cabeza[8..12] == b"WEBP")
}

/// **Que de verdad es un MP4** (`ftyp` en el byte 4) y no una foto HEIC o
/// AVIF, que usan la misma caja con otra marca.
pub fn es_mp4(cabeza: &[u8]) -> bool {
    if cabeza.len() < 12 || &cabeza[4..8] != b"ftyp" {
        return false;
    }
    !matches!(
        &cabeza[8..12],
        b"heic" | b"heix" | b"heim" | b"heis" | b"hevc" | b"mif1" | b"msf1" | b"avif" | b"avis"
    )
}

/// Que lo llegado a `fichero` es lo que dice ser: una foto o un clip.
pub fn fichero_es_valido(fichero: &Path, clip: bool) -> bool {
    let mut cabeza = [0u8; 16];
    let Ok(mut f) = std::fs::File::open(fichero) else {
        return false;
    };
    let n = f.read(&mut cabeza).unwrap_or(0);
    if clip {
        es_mp4(&cabeza[..n])
    } else {
        es_imagen(&cabeza[..n])
    }
}

// ------------------------------------------------------------ quien graba

/// **Lo que hace quien graba** (el PC) con lo de la camara: lo da quien
/// recibe los sueltos ([`crate::al_lienzo::Recibe::camara`]).
pub trait Camara {
    /// Como va la grabacion ahora (sin `reloj`: lo pone el protocolo).
    /// `None`: no se graba nada.
    fn anuncio(&mut self) -> Option<Anuncio>;
    /// Antes del «vale», tras [`aceptar`]: si esa grabacion la conoce y
    /// todavia admite fotos.
    fn aceptar_foto(&mut self, p: &Suelto) -> Result<(), String>;
    /// La foto llego entera a `fichero` (temporal: se copia o se mueve).
    /// Devuelve el milisegundo de la grabacion donde quedo.
    fn guardar_foto(&mut self, p: &Suelto, fichero: &Path) -> Result<i64, String>;
}

// ------------------------------------------------------------ el movil

/// Lo que lleva cada foto de la camara, ademas del fichero.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FotoDeCamara {
    pub grabacion: String,
    /// El minuto de la grabacion, calculado en el movil: lo primero.
    pub ms: Option<i64>,
    /// La hora del movil al hacerla.
    pub tomada_en: i64,
    /// Lo que va adelantado el reloj del movil sobre el del PC.
    pub reloj: i64,
    /// Un clip: lo que dura (su `ms` es donde empezo).
    pub duracion_ms: i64,
    /// El id de la toma (el mismo en cada intento): con el, un clip cortado
    /// sigue donde se quedo.
    pub clave: String,
}

fn enviar<F: Read + Write, T: Serialize>(
    c: &mut crate::canal::Canal<F>,
    cosa: &T,
) -> Result<(), ErrorAlLienzo> {
    c.mandar(Tipo::Json, &serde_json::to_vec(cosa)?)?;
    Ok(())
}

fn leer<F: Read + Write, T: for<'a> Deserialize<'a>>(
    c: &mut crate::canal::Canal<F>,
) -> Result<T, ErrorAlLienzo> {
    let (tipo, datos) = c.recibir()?;
    if tipo != Tipo::Json {
        return Err(ErrorAlLienzo::Protocolo("Se esperaba una respuesta".into()));
    }
    Ok(serde_json::from_slice(&datos)?)
}

impl<F: Read + Write> Conexion<F> {
    /// Si el otro sabe de la camara ([`CAPACIDAD`] en su `hola`).
    pub fn sabe_de_camara(&self) -> bool {
        self.capacidades.iter().any(|c| c == CAPACIDAD)
    }

    /// **El movil pregunta al PC como va su grabacion** y mide el desfase
    /// de los relojes de paso. Devuelve el anuncio y lo que va adelantado el
    /// reloj de aqui (el movil) sobre el del otro (el PC).
    pub fn preguntar_camara(
        &mut self,
        ahora: &dyn Fn() -> i64,
    ) -> Result<(Anuncio, i64), ErrorAlLienzo> {
        let t0 = ahora();
        enviar(
            &mut self.canal,
            &Anuncio {
                reloj: t0,
                ..Anuncio::nada()
            },
        )?;
        let r: Anuncio = leer(&mut self.canal)?;
        let t2 = ahora();
        if let Some(e) = r.error.clone() {
            return Err(crate::al_lienzo::error_remoto(e));
        }
        let mio_sobre_el_suyo = if r.reloj > 0 {
            -desfase(t0, r.reloj, t2)
        } else {
            0
        };
        Ok((r, mio_sobre_el_suyo))
    }

    /// **El PC avisa al movil** de como va su grabacion (al empezar, pausar,
    /// seguir o acabar). Devuelve lo que va adelantado el reloj del otro (el
    /// movil) sobre el de aqui, medido de paso.
    pub fn anunciar_camara(
        &mut self,
        anuncio: &Anuncio,
        ahora: &dyn Fn() -> i64,
    ) -> Result<i64, ErrorAlLienzo> {
        let t0 = ahora();
        enviar(
            &mut self.canal,
            &Anuncio {
                t: PETICION.into(),
                reloj: t0,
                ..anuncio.clone()
            },
        )?;
        let r: Anuncio = leer(&mut self.canal)?;
        let t2 = ahora();
        if let Some(e) = r.error {
            return Err(crate::al_lienzo::error_remoto(e));
        }
        Ok(if r.reloj > 0 {
            desfase(t0, r.reloj, t2)
        } else {
            0
        })
    }

    /// **El movil manda una foto (o un clip) a la grabacion del PC.** Devuelve el
    /// milisegundo donde quedo. Un «no» antes del «vale» deja la conexion
    /// sana.
    pub fn mandar_foto(
        &mut self,
        ruta: &Path,
        nombre: &str,
        foto: &FotoDeCamara,
    ) -> Result<i64, ErrorAlLienzo> {
        let listo = self.mandar_suelto(
            ruta,
            Suelto {
                nombre: nombre.to_string(),
                mime: mime_de(nombre).to_string(),
                destino: DESTINO.into(),
                grabacion: foto.grabacion.clone(),
                ms: foto.ms,
                tomada_en: foto.tomada_en,
                reloj: foto.reloj,
                duracion_ms: foto.duracion_ms,
                clave: foto.clave.clone(),
                ..Default::default()
            },
        )?;
        Ok(listo.ms.unwrap_or(0))
    }
}

/// **El «listo» de una foto**, o el motivo de que no entro.
pub(crate) fn listo_de_foto(ms: Result<i64, String>) -> Contestacion {
    match ms {
        Ok(ms) => Contestacion {
            t: Some(LISTO.into()),
            donde: Some(DONDE.into()),
            ms: Some(ms),
            ..Default::default()
        },
        Err(e) => Contestacion {
            error: Some(e),
            ..Default::default()
        },
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const S: i64 = 1000;
    const PARED: i64 = 1_761_550_000_000;

    #[test]
    fn el_minuto_no_cuenta_lo_pausado() {
        let mut r = Reloj::empezar(PARED, 0);
        assert_eq!(r.ms_en(PARED + 10 * S), 10 * S);
        r.pausar(PARED + 10 * S);
        assert!(r.en_pausa());
        // En pausa el minuto no corre.
        assert_eq!(r.ms_en(PARED + 25 * S), 10 * S);
        r.seguir(PARED + 30 * S);
        assert_eq!(r.ms_en(PARED + 40 * S), 20 * S);
        r.acabar(PARED + 50 * S);
        // Tras acabar, lo que duro.
        assert_eq!(r.ms_en(PARED + 999 * S), 30 * S);
        assert_eq!(r.duracion(PARED + 999 * S), 30 * S);
        // Antes de empezar, 0.
        assert_eq!(r.ms_en(PARED - S), 0);
        assert_eq!(r.pausas_al_cable(), vec![[PARED + 10 * S, PARED + 30 * S]]);
    }

    #[test]
    fn seguir_a_la_grabadora_corrige_la_deriva_y_las_pausas() {
        let mut r = Reloj::empezar(PARED, 2 * S);
        assert_eq!(r.ms_en(PARED), 2 * S);
        // La grabadora va 40 ms por detras: se deja.
        r.seguir_a(PARED + 10 * S, 12 * S - 40, false, 250);
        assert_eq!(r.ms_en(PARED + 10 * S), 12 * S);
        // Va un segundo por detras: se corrige.
        r.seguir_a(PARED + 20 * S, 21 * S, false, 250);
        assert_eq!(r.ms_en(PARED + 20 * S), 21 * S);
        r.seguir_a(PARED + 21 * S, 22 * S, true, 250);
        assert!(r.en_pausa());
        assert_eq!(r.ms_en(PARED + 30 * S), 22 * S);
        r.seguir_a(PARED + 31 * S, 22 * S, false, 250);
        assert!(!r.en_pausa());
        assert_eq!(r.ms_en(PARED + 32 * S), 23 * S);
    }

    #[test]
    fn el_desfase_se_mide_como_ntp() {
        // El otro va 5 s adelantado; la ida y la vuelta, 100 ms cada una.
        assert_eq!(desfase(PARED, PARED + 100 + 5 * S, PARED + 200), 5 * S);
        assert_eq!(desfase(PARED, PARED + 100 - 7 * S, PARED + 200), -7 * S);
        // Un t2 roto (antes que t0) no da un desfase absurdo.
        assert_eq!(desfase(PARED, PARED, PARED - 50), 0);
    }

    fn foto(ms: Option<i64>, tomada_en: i64, reloj: i64) -> Suelto {
        Suelto {
            destino: DESTINO.into(),
            grabacion: "g1".into(),
            ms,
            tomada_en,
            reloj,
            ..Default::default()
        }
    }

    #[test]
    fn el_minuto_de_la_foto_es_el_que_dice_el_movil() {
        let mut r = Reloj::empezar(PARED, 0);
        r.pausar(PARED + 60 * S);
        r.seguir(PARED + 90 * S);
        let ahora = PARED + 120 * S;
        // Con `ms`, ese, aunque su reloj diga otra cosa.
        assert_eq!(
            ms_de_la_foto(&r, ahora, None, &foto(Some(45 * S), 1, 999), None),
            45 * S
        );
        // Dentro de lo que lleva: ni negativo ni del futuro.
        assert_eq!(
            ms_de_la_foto(&r, ahora, None, &foto(Some(-3), 0, 0), None),
            0
        );
        assert_eq!(
            ms_de_la_foto(&r, ahora, None, &foto(Some(10_000 * S), 0, 0), None),
            90 * S
        );
    }

    #[test]
    fn sin_ms_se_calcula_con_su_hora_el_desfase_y_las_pausas() {
        let mut r = Reloj::empezar(PARED, 0);
        r.pausar(PARED + 60 * S);
        r.seguir(PARED + 90 * S);
        let ahora = PARED + 200 * S;
        // El movil va 8 s adelantado y lo dice el mismo (`reloj`): su foto
        // de las PARED+108 s (las +100 del PC) es del minuto 70 (30 s de
        // pausa fuera).
        let f = foto(None, PARED + 108 * S, 8 * S);
        assert_eq!(ms_de_la_foto(&r, ahora, None, &f, None), 70 * S);
        // Sin `reloj`, el que se midio aqui.
        let f = foto(None, PARED + 108 * S, 0);
        assert_eq!(ms_de_la_foto(&r, ahora, None, &f, Some(8 * S)), 70 * S);
        // Hecha en la pausa: el minuto en que se paro.
        let f = foto(None, PARED + 70 * S, 0);
        assert_eq!(ms_de_la_foto(&r, ahora, None, &f, None), 60 * S);
        // Sin nada: el minuto en que llega.
        assert_eq!(
            ms_de_la_foto(&r, ahora, None, &foto(None, 0, 0), None),
            170 * S
        );
    }

    #[test]
    fn la_que_llega_tarde_entra_en_lo_que_duro() {
        let mut r = Reloj::empezar(PARED, 0);
        r.acabar(PARED + 100 * S);
        let mucho_despues = PARED + 3600 * S;
        // Hecha en el segundo 95, subida una hora despues.
        assert_eq!(
            ms_de_la_foto(&r, mucho_despues, None, &foto(Some(95 * S), 0, 0), None),
            95 * S
        );
        // La nota duro un poco menos que el reloj: manda la nota.
        assert_eq!(
            ms_de_la_foto(
                &r,
                mucho_despues,
                Some(99 * S),
                &foto(Some(100 * S), 0, 0),
                None
            ),
            99 * S
        );
        // Hecha (segun su hora) despues del final: al final.
        let f = foto(None, PARED + 500 * S, 0);
        assert_eq!(ms_de_la_foto(&r, mucho_despues, None, &f, None), 100 * S);
    }

    #[test]
    fn solo_se_aceptan_fotos_de_una_grabacion() {
        let mut p = Suelto {
            nombre: "f.jpg".into(),
            mime: "image/jpeg".into(),
            bytes: 1234,
            destino: DESTINO.into(),
            grabacion: "g1".into(),
            ..Default::default()
        };
        assert_eq!(aceptar(&p), Ok(()));
        // Un clip MP4 cabe con su propio tope, mas grande.
        p.mime = "video/mp4".into();
        p.bytes = TOPE_DE_FOTO * 2;
        assert_eq!(aceptar(&p), Ok(()));
        p.bytes = TOPE_DE_CLIP + 1;
        assert!(aceptar(&p).is_err());
        p.mime = "image/jpeg".into();
        p.bytes = 1234;
        // Casos negativos: sin grabacion, un PDF, un HEIC, vacia, enorme.
        p.grabacion.clear();
        assert!(aceptar(&p).is_err());
        p.grabacion = "g1".into();
        for (mime, bytes) in [
            ("application/pdf", 1234),
            ("image/heic", 1234),
            ("image/jpeg", 0),
            ("image/jpeg", TOPE_DE_FOTO + 1),
        ] {
            p.mime = mime.into();
            p.bytes = bytes;
            assert!(aceptar(&p).is_err(), "{mime} {bytes}");
        }
        assert!(es_imagen(&[0xFF, 0xD8, 0xFF, 0xE1, 0, 0]));
        assert!(es_imagen(b"\x89PNG\r\n\x1a\n...."));
        assert!(es_imagen(b"RIFF\0\0\0\0WEBPVP8 "));
        assert!(!es_imagen(b"%PDF-1.7"));
        assert!(!es_imagen(b""));
        assert!(es_mp4(b"\0\0\0\x18ftypmp42\0\0\0\0"));
        assert!(es_mp4(b"\0\0\0\x18ftypisom\0\0\0\0"));
        // Una foto HEIC lleva la misma caja: no es un clip.
        assert!(!es_mp4(b"\0\0\0\x18ftypheic\0\0\0\0"));
        assert!(!es_mp4(&[0xFF, 0xD8, 0xFF, 0xE1, 0, 0, 0, 0, 0, 0, 0, 0]));
    }

    #[test]
    fn un_clip_empieza_en_su_minuto_y_dura_lo_suyo_sin_pasar_del_final() {
        let mut r = Reloj::empezar(PARED, 0);
        r.pausar(PARED + 30 * S);
        r.seguir(PARED + 40 * S);
        r.acabar(PARED + 100 * S);
        let mut clip = foto(Some(50 * S), 0, 0);
        clip.mime = "video/mp4".into();
        clip.duracion_ms = 20 * S;
        assert!(es_clip(&clip));
        let ahora = PARED + 500 * S;
        let ms = ms_de_la_foto(&r, ahora, None, &clip, None);
        assert_eq!(ms, 50 * S);
        assert_eq!(duracion_del_clip(&clip, ms, r.duracion(ahora)), 20 * S);
        // Empezado a 10 s del final (dura 90): no pasa del final.
        clip.ms = Some(80 * S);
        let ms = ms_de_la_foto(&r, ahora, None, &clip, None);
        assert_eq!(duracion_del_clip(&clip, ms, r.duracion(ahora)), 10 * S);
        // Casos negativos: sin duracion, o una negativa: 0.
        clip.duracion_ms = -5;
        assert_eq!(duracion_del_clip(&clip, ms, r.duracion(ahora)), 0);
        // Sin `ms`, el principio sale de su hora y su desfase, como una foto:
        // a las +60 s del PC (10 s de pausa fuera), el 50.
        let mut clip = foto(None, PARED + 63 * S, 3 * S);
        clip.mime = "video/mp4".into();
        clip.duracion_ms = 5 * S;
        let ms = ms_de_la_foto(&r, ahora, None, &clip, None);
        assert_eq!(ms, 50 * S);
        assert_eq!(duracion_del_clip(&clip, ms, r.duracion(ahora)), 5 * S);
    }

    #[test]
    fn el_movil_graba_en_hevc_solo_si_el_pc_lo_sabe_ver() {
        let mut a = Anuncio::nada();
        assert_eq!(a.codec_del_clip(), None);
        assert!(!a.con_clips());
        a.codecs = vec![H264.into()];
        assert_eq!(a.codec_del_clip(), Some(H264));
        a.codecs = vec![H264.into(), HEVC.into()];
        assert_eq!(a.codec_del_clip(), Some(HEVC));
    }

    #[test]
    fn el_anuncio_viaja_sin_lo_que_va_por_omision() {
        let a = Anuncio::nada();
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            r#"{"t":"camara_remota"}"#
        );
        assert!(!a.grabando());
        let a = Anuncio {
            grabacion: "g1".into(),
            titulo: "Clase".into(),
            activa: true,
            en_pausa: true,
            ms_actual: 754_000,
            pausas: vec![[5, 0]],
            ..Anuncio::nada()
        };
        let texto = serde_json::to_string(&a).unwrap();
        assert_eq!(
            texto,
            r#"{"t":"camara_remota","grabacion":"g1","titulo":"Clase","activa":true,"en_pausa":true,"ms_actual":754000,"pausas":[[5,0]]}"#
        );
        // Lo que no se conoce se ignora (un movil mas nuevo).
        let leido: Anuncio =
            serde_json::from_str(r#"{"t":"camara_remota","grabacion":"g1","nuevo":1}"#).unwrap();
        assert_eq!(leido.grabacion, "g1");
    }
}
