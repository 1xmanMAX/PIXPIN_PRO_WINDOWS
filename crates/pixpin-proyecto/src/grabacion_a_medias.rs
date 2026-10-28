//! **Grabar sin perder nada** (28-oct-2026, el usuario: «todo tiene que ser
//! rapido y fiable al grabar: si se perdiera algo seria malo, se perderia una
//! reunion»).
//!
//! Mientras se graba una clase o una reunion, todo lo que se va haciendo se
//! apunta **en el disco en el acto**, en un diario por grabacion:
//!
//! ```text
//! <raiz>/grabaciones-a-medias/<marca>/
//!     estado.json        el Diario: chat, modo, nombre, reloj de pared,
//!                        latido, y la lista de lo capturado con su minuto
//!     captura-1.png      cada captura o foto, en cuanto se hace
//!     captura-2.jpg
//! ```
//!
//! El sonido y el video van donde siempre (al lado del chat, en `.parte` y
//! con el respaldo WAV de la nota, ver `pixpin_motor_audio::respaldo`); el
//! diario dice donde.
//!
//! - **Al terminar bien** la clase entra en el chat y el diario se borra.
//! - **Si algo se corta** (la aplicacion muere, se va la luz, Windows se
//!   reinicia), al arrancar quedan diarios: [`pendientes`] los encuentra con
//!   una sola lectura de carpeta, [`hallar`] mira que se puede salvar de cada
//!   uno y [`Plan`] lo dice. Al usuario se le pregunta «Recuperar /
//!   Descartar»; **nada se borra sin que elija Descartar**.
//!
//! El diario se escribe a un `.tmp`, se lleva al disco (`sync_all`) y se
//! renombra encima: un corte a mitad de escribirlo deja el anterior entero.
//!
//! **El reloj de pared** ([`Reloj`]): cuando empezo, sus pausas y los huecos
//! (el equipo se suspendio: la nota no los graba, pero la hora siguio). Con
//! el se pasa una hora de pared a su minuto en la nota ([`Diario::ms_en`]),
//! que es lo que necesita lo que llega con hora y no con minuto (las fotos
//! del movil, `resto.reloj`).
//!
//! Todo es puro (`std::fs` y `serde`): se prueba en Linux.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use pixpin_sistema::mp4::{EstadoMp4, mirar};

/// La carpeta de los diarios, dentro de la raiz de PixPin.
pub const CARPETA: &str = "grabaciones-a-medias";
/// El diario de cada grabacion.
pub const FICHERO: &str = "estado.json";
/// La version del formato del diario.
pub const VERSION: u32 = 1;

/// Entre dos latidos, lo que la hora puede adelantar a la nota sin que sea
/// un hueco (la barra se atasco, el disco tardo).
pub const UMBRAL_HUECO_MS: i64 = 10_000;

/// Lo menos que dura una nota (lo de `pixpin_audio::MINIMO_MS`): menos es un
/// resbalon en el boton, no una reunion.
pub const MINIMO_NOTA_MS: i64 = 700;

/// Por debajo de esto libre en el disco, se avisa.
pub const ESPACIO_POCO: u64 = 2 * 1024 * 1024 * 1024;
/// Por debajo de esto, se para el video y se sigue solo con el sonido.
pub const ESPACIO_CRITICO: u64 = 500 * 1024 * 1024;

/// Un tramo de la hora de pared (ms UTC). `hasta: None`, sin cerrar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tramo {
    pub desde: i64,
    pub hasta: Option<i64>,
}

/// **El reloj de pared** de la grabacion.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Reloj {
    /// Cuando empezo a grabar (ms UTC).
    pub inicio: i64,
    /// Cuando acabo, si acabo.
    pub fin: Option<i64>,
    /// Las pausas del usuario.
    pub pausas: Vec<Tramo>,
    /// Lo que la hora siguio sin que la nota grabara (el equipo suspendido,
    /// la tapa cerrada): sin estos, una foto del movil de despues caeria
    /// tarde en la nota.
    pub huecos: Vec<Tramo>,
}

/// El ultimo latido: a que hora la nota iba por que minuto.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Latido {
    pub utc: i64,
    pub ms: i64,
    pub pausa: bool,
}

/// Una captura (de la pantalla o foto de la camara), ya en el disco.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturaApuntada {
    pub ms: i64,
    /// El fichero, dentro de la carpeta del diario.
    pub fichero: String,
    pub de_camara: bool,
}

/// Un clip de la camara: donde se graba y, al cerrarse, lo que dura.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipApuntado {
    pub ms: i64,
    pub ruta: PathBuf,
    /// `None` mientras se graba (o si se corto).
    pub duracion_ms: Option<i64>,
}

/// Una marca puesta a mano en un minuto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marcador {
    pub ms: i64,
    pub texto: String,
}

/// **El diario de una grabacion.**
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Diario {
    pub version: u32,
    /// Su nombre de carpeta: la hora de empezar, en ms UTC.
    pub marca: String,
    /// El chat (proyecto) donde va, y su nombre para ensenarlo.
    pub proyecto: String,
    pub nombre_proyecto: String,
    /// El nombre de la nota: «Clase 28-10 10h05».
    pub nombre: String,
    /// `pantalla`, `capturas` o `presencial` (`presencial::Modo::palabra`).
    pub modo: String,
    pub microfono: bool,
    pub equipo: bool,
    /// Donde se graba la nota (sin el `.parte`; el respaldo va al lado).
    pub audio: PathBuf,
    /// Donde se graba el video (sin el `.parte`), si se graba.
    pub video: Option<PathBuf>,
    pub reloj: Reloj,
    pub latido: Latido,
    pub capturas: Vec<CapturaApuntada>,
    pub clips: Vec<ClipApuntado>,
    pub marcadores: Vec<Marcador>,
    /// La grabacion que se anuncio al movil (`camara_del_movil`), para
    /// recoger lo que llego de el.
    pub movil: Option<String>,
    /// Lo demas (capturas, video, clips) ya entro en el chat y solo falta
    /// la nota: lo que pasa si la nota no se pudo cerrar al terminar.
    pub solo_la_nota: bool,
    /// Los temas de la reunion tal cual iran a la nota (`resto["temas"]`,
    /// ver `crate::temas`), con lo ya tratado: la clase recuperada los
    /// lleva. Sin temas, no va.
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub temas: serde_json::Value,
    /// La lista de tareas elegida al preparar, `(proyecto, id)`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temas_lista: Option<(String, String)>,
}

/// La carpeta de todos los diarios.
pub fn carpeta_de_diarios(raiz: &Path) -> PathBuf {
    raiz.join(CARPETA)
}

/// Escribe `bytes` en `ruta` sin que un corte deje algo a medias: a un
/// `.tmp` al lado, al disco, y renombrado encima.
pub fn escribir_entero(ruta: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = ruta.as_os_str().to_os_string();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, ruta)
}

impl Diario {
    /// Un diario nuevo para una grabacion que empieza a la hora `inicio`.
    pub fn nuevo(inicio: i64, proyecto: &str, nombre: &str, audio: &Path) -> Diario {
        Diario {
            version: VERSION,
            marca: inicio.to_string(),
            proyecto: proyecto.to_string(),
            nombre: nombre.to_string(),
            audio: audio.to_path_buf(),
            reloj: Reloj {
                inicio,
                ..Reloj::default()
            },
            latido: Latido {
                utc: inicio,
                ms: 0,
                pausa: false,
            },
            ..Diario::default()
        }
    }

    /// Su carpeta, dentro de la de los diarios de `raiz`.
    pub fn carpeta(&self, raiz: &Path) -> PathBuf {
        carpeta_de_diarios(raiz).join(&self.marca)
    }

    /// **Lo apunta** en `dir` (que se crea si no esta).
    pub fn guardar(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        escribir_entero(&dir.join(FICHERO), &json)
    }

    /// Lo lee de `dir`. Si el bueno no se entiende (no deberia: se escribe
    /// entero), prueba con el `.tmp` que quedara.
    pub fn leer(dir: &Path) -> std::io::Result<Diario> {
        let ruta = dir.join(FICHERO);
        let de = |r: &Path| -> std::io::Result<Diario> {
            let bytes = std::fs::read(r)?;
            serde_json::from_slice(&bytes)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        };
        de(&ruta).or_else(|e| {
            let mut tmp = ruta.as_os_str().to_os_string();
            tmp.push(".tmp");
            de(Path::new(&tmp)).map_err(|_| e)
        })
    }

    /// **Guarda una captura** en `dir` en el acto (`captura-N.ext`) y la
    /// apunta. Hay que [`Diario::guardar`] despues.
    pub fn apuntar_captura(
        &mut self,
        dir: &Path,
        ms: i64,
        bytes: &[u8],
        de_camara: bool,
        extension: &str,
    ) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(dir)?;
        let fichero = format!("captura-{}.{extension}", self.capturas.len() + 1);
        let ruta = dir.join(&fichero);
        escribir_entero(&ruta, bytes)?;
        self.capturas.push(CapturaApuntada {
            ms,
            fichero,
            de_camara,
        });
        Ok(ruta)
    }

    /// Un clip empieza en `ms`, grabandose en `ruta`.
    pub fn empezar_clip(&mut self, ms: i64, ruta: &Path) {
        self.clips.push(ClipApuntado {
            ms,
            ruta: ruta.to_path_buf(),
            duracion_ms: None,
        });
    }

    /// El clip de `ruta` se cerro y dura `duracion_ms`.
    pub fn cerrar_clip(&mut self, ruta: &Path, duracion_ms: i64) {
        if let Some(c) = self.clips.iter_mut().rev().find(|c| c.ruta == ruta) {
            c.duracion_ms = Some(duracion_ms.max(0));
        }
    }

    /// **Un latido de la barra**: a la hora `utc` la nota va por `ms`. Lleva
    /// las pausas y apunta un hueco si la hora corrio mucho mas que la nota
    /// sin estar en pausa (el equipo se suspendio). Devuelve si cambio algo
    /// que merezca escribirse ya (una pausa, un hueco).
    pub fn latir(&mut self, utc: i64, ms: i64, pausa: bool) -> bool {
        let antes = self.latido;
        self.latido = Latido { utc, ms, pausa };
        let mut cambio = false;
        if pausa && !antes.pausa {
            self.reloj.pausas.push(Tramo {
                desde: utc,
                hasta: None,
            });
            cambio = true;
        } else if !pausa && antes.pausa {
            if let Some(t) = self.reloj.pausas.last_mut().filter(|t| t.hasta.is_none()) {
                t.hasta = Some(utc);
            }
            cambio = true;
        } else if !pausa {
            let sin_grabar = (utc - antes.utc) - (ms - antes.ms);
            if sin_grabar > UMBRAL_HUECO_MS {
                // La nota se paro donde iba y siguio despues: el hueco es
                // lo que la hora corrio de mas, justo antes de este latido.
                self.reloj.huecos.push(Tramo {
                    desde: utc - sin_grabar,
                    hasta: Some(utc),
                });
                cambio = true;
            }
        }
        cambio
    }

    /// Acaba el reloj a la hora `utc`.
    pub fn acabar(&mut self, utc: i64) {
        if let Some(t) = self.reloj.pausas.last_mut().filter(|t| t.hasta.is_none()) {
            t.hasta = Some(utc);
        }
        self.reloj.fin = Some(utc);
    }

    /// **El minuto de la nota** a la hora de pared `utc`: lo que paso desde
    /// el inicio menos las pausas y los huecos de antes. Nunca negativo. La
    /// cuenta es la de las fotos por su hora
    /// ([`crate::por_su_hora::RelojDeGrabacion::ms_hasta`]): un solo reloj
    /// de pared para los dos.
    pub fn ms_en(&self, utc: i64) -> i64 {
        crate::por_su_hora::RelojDeGrabacion::del_diario(&self.reloj, "", 0).ms_hasta(utc)
    }

    /// **Lo capturado en orden**, con su minuto dentro de la nota de
    /// `duracion_ms` (lo de despues del final cae al final).
    pub fn linea_de_tiempo(&self, duracion_ms: i64) -> Vec<(i64, Elemento)> {
        let tope = duracion_ms.max(0);
        let mut v: Vec<(i64, Elemento)> = self
            .capturas
            .iter()
            .enumerate()
            .map(|(i, c)| (c.ms.clamp(0, tope), Elemento::Captura(i)))
            .chain(
                self.clips
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (c.ms.clamp(0, tope), Elemento::Clip(i))),
            )
            .chain(
                self.marcadores
                    .iter()
                    .enumerate()
                    .map(|(i, m)| (m.ms.clamp(0, tope), Elemento::Marcador(i))),
            )
            .collect();
        v.sort_by_key(|(ms, e)| (*ms, *e));
        v
    }
}

/// Un elemento de la [`Diario::linea_de_tiempo`], por su posicion en su
/// lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Elemento {
    Captura(usize),
    Clip(usize),
    Marcador(usize),
}

/// **Los diarios que quedaron** en `raiz`: grabaciones que no llegaron a
/// guardarse. Una sola lectura de carpeta (y un fichero por diario): se
/// puede mirar al arrancar sin que se note. Las mas viejas primero.
pub fn pendientes(raiz: &Path) -> Vec<(PathBuf, Diario)> {
    let Ok(entradas) = std::fs::read_dir(carpeta_de_diarios(raiz)) else {
        return Vec::new();
    };
    let mut v: Vec<(PathBuf, Diario)> = entradas
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter_map(|p| Diario::leer(&p).ok().map(|d| (p, d)))
        .collect();
    v.sort_by_key(|(_, d)| d.reloj.inicio);
    v
}

/// **Descartar**: lo pidio el usuario. Se borra la carpeta del diario y lo
/// que dice que es suyo (la nota a medias, su respaldo, el video a medias y
/// los clips). Lo que ya entro en un chat no se toca.
pub fn descartar(dir: &Path, d: &Diario) {
    let mut rutas = vec![d.audio.clone(), con_sufijo(&d.audio, ".parte")];
    rutas.push(con_sufijo(&d.audio, ".respaldo.wav"));
    rutas.push(con_sufijo(&d.audio, ".rehecho.parte"));
    if let Some(v) = &d.video {
        rutas.push(v.clone());
        rutas.push(con_sufijo(v, ".parte"));
    }
    rutas.extend(d.clips.iter().map(|c| c.ruta.clone()));
    for r in rutas {
        let _ = std::fs::remove_file(r);
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// `ruta` con `sufijo` anadido al nombre entero.
pub fn con_sufijo(ruta: &Path, sufijo: &str) -> PathBuf {
    let mut s = ruta.as_os_str().to_os_string();
    s.push(sufijo);
    PathBuf::from(s)
}

/// Lo que se encontro en el disco de una grabacion cortada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hallado {
    /// La nota ya cerrada en su nombre (se corto al guardarla).
    pub audio_listo: bool,
    /// Como quedo el `.parte` de la nota.
    pub audio_parte: EstadoMp4,
    /// Lo que dura el respaldo WAV, si lo hay y tiene algo.
    pub respaldo_ms: Option<i64>,
    /// El video ya cerrado en su nombre.
    pub video_listo: bool,
    /// Como quedo el `.parte` del video.
    pub video_parte: EstadoMp4,
    /// Las capturas del diario cuyo fichero sigue ahi.
    pub capturas: Vec<usize>,
    /// Los clips cuyo fichero se puede abrir.
    pub clips: Vec<usize>,
}

/// **Mira el disco**: que queda de la grabacion del diario `d` (en `dir`).
/// `respaldo_ms` lee el respaldo WAV (lo da quien sabe leerlo:
/// `pixpin_motor_audio::respaldo`).
pub fn hallar(dir: &Path, d: &Diario, respaldo_ms: impl Fn(&Path) -> Option<i64>) -> Hallado {
    let video = d.video.as_deref();
    Hallado {
        audio_listo: !d.audio.as_os_str().is_empty() && mirar(&d.audio) == EstadoMp4::Cerrado,
        audio_parte: mirar(&con_sufijo(&d.audio, ".parte")),
        respaldo_ms: respaldo_ms(&con_sufijo(&d.audio, ".respaldo.wav")).filter(|ms| *ms > 0),
        video_listo: video.is_some_and(|v| mirar(v) == EstadoMp4::Cerrado),
        video_parte: video.map_or(EstadoMp4::Nada, |v| mirar(&con_sufijo(v, ".parte"))),
        capturas: d
            .capturas
            .iter()
            .enumerate()
            .filter(|(_, c)| dir.join(&c.fichero).is_file())
            .map(|(i, _)| i)
            .collect(),
        clips: d
            .clips
            .iter()
            .enumerate()
            .filter(|(_, c)| mirar(&c.ruta).se_abre())
            .map(|(i, _)| i)
            .collect(),
    }
}

/// De donde sale la nota recuperada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nota {
    /// Ya estaba cerrada en su nombre.
    Lista,
    /// Del `.parte`, que llego a cerrarse.
    DelParte,
    /// Rehecha del respaldo WAV.
    DelRespaldo,
    /// No hay nota que salvar.
    Ninguna,
}

/// De donde sale el video recuperado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Video {
    Listo,
    /// Del `.parte` (cerrado o fragmentado: se abre).
    DelParte,
    /// Hay un `.parte` pero sin indice: no se puede abrir. Se conserva
    /// hasta Descartar.
    Perdido,
    Ninguno,
}

/// **Que se va a recuperar** de una grabacion cortada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub nota: Nota,
    /// Lo que dura la nota (del respaldo, o del ultimo latido).
    pub duracion_ms: i64,
    pub video: Video,
    pub capturas: Vec<usize>,
    pub clips: Vec<usize>,
}

impl Plan {
    /// Si hay algo que salvar (si no, solo queda descartar).
    pub fn algo(&self) -> bool {
        self.nota != Nota::Ninguna
            || matches!(self.video, Video::Listo | Video::DelParte)
            || !self.capturas.is_empty()
            || !self.clips.is_empty()
    }

    /// Los minutos para el aviso, redondeados (al menos 1 si hay algo).
    pub fn minutos(&self) -> i64 {
        let m = (self.duracion_ms + 30_000) / 60_000;
        if self.duracion_ms > 0 { m.max(1) } else { 0 }
    }
}

/// **El plan** para el diario `d` con lo [`Hallado`] en el disco. La nota,
/// por orden: la ya cerrada, el `.parte` cerrado, el respaldo. Lo que dura
/// sale del respaldo (exacto) o del ultimo latido.
pub fn plan(d: &Diario, h: &Hallado) -> Plan {
    let nota = if h.audio_listo {
        Nota::Lista
    } else if h.audio_parte == EstadoMp4::Cerrado {
        Nota::DelParte
    } else if h.respaldo_ms.is_some_and(|ms| ms >= MINIMO_NOTA_MS) {
        Nota::DelRespaldo
    } else {
        Nota::Ninguna
    };
    let video = if d.solo_la_nota {
        Video::Ninguno
    } else if h.video_listo {
        Video::Listo
    } else if h.video_parte.se_abre() {
        Video::DelParte
    } else if h.video_parte == EstadoMp4::SinIndice {
        Video::Perdido
    } else {
        Video::Ninguno
    };
    let duracion_ms = h.respaldo_ms.unwrap_or(d.latido.ms).max(0);
    Plan {
        nota,
        duracion_ms,
        video,
        capturas: if d.solo_la_nota {
            Vec::new()
        } else {
            h.capturas.clone()
        },
        clips: if d.solo_la_nota {
            Vec::new()
        } else {
            h.clips.clone()
        },
    }
}

/// Lo que dice el disco de cuanto cabe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Espacio {
    Bien,
    /// Menos de [`ESPACIO_POCO`]: avisar.
    Poco,
    /// Menos de [`ESPACIO_CRITICO`]: parar el video, seguir con el sonido.
    Critico,
}

/// **Cuanto cabe**, con `libres` bytes libres (`None`: no se sabe, y se
/// sigue como si cupiera).
pub fn espacio(libres: Option<u64>) -> Espacio {
    match libres {
        Some(b) if b < ESPACIO_CRITICO => Espacio::Critico,
        Some(b) if b < ESPACIO_POCO => Espacio::Poco,
        _ => Espacio::Bien,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn carpeta(nombre: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("pixpin-a-medias-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn caja(tipo: &[u8; 4], cuerpo: usize) -> Vec<u8> {
        let mut c = ((cuerpo + 8) as u32).to_be_bytes().to_vec();
        c.extend_from_slice(tipo);
        c.resize(cuerpo + 8, 0);
        c
    }

    fn mp4_cerrado() -> Vec<u8> {
        [caja(b"ftyp", 8), caja(b"mdat", 64), caja(b"moov", 16)].concat()
    }

    fn mp4_cortado() -> Vec<u8> {
        let mut b = [caja(b"ftyp", 8), caja(b"mdat", 640)].concat();
        b.truncate(200);
        b
    }

    const T0: i64 = 1_761_645_900_000;

    #[test]
    fn el_diario_va_y_vuelve_del_disco_entero() {
        let raiz = carpeta("ida-y-vuelta");
        let mut d = Diario::nuevo(
            T0,
            "p1",
            "Clase 28-10 10h05",
            &raiz.join("voz-1.m4a.parcial"),
        );
        d.modo = "presencial".into();
        d.microfono = true;
        d.video = Some(raiz.join("voz-1.m4a.parcial.video.mp4"));
        d.movil = Some("g-123".into());
        d.marcadores.push(Marcador {
            ms: 5_000,
            texto: "importante".into(),
        });
        let dir = d.carpeta(&raiz);
        d.apuntar_captura(&dir, 61_000, b"png!", false, "png")
            .unwrap();
        d.apuntar_captura(&dir, 2_000, b"jpg!", true, "jpg")
            .unwrap();
        d.empezar_clip(90_000, &raiz.join("clip-1.mp4"));
        d.cerrar_clip(&raiz.join("clip-1.mp4"), 12_345);
        d.latir(T0 + 120_000, 120_000, false);
        d.guardar(&dir).unwrap();

        let leido = Diario::leer(&dir).unwrap();
        assert_eq!(leido, d);
        assert_eq!(leido.version, VERSION);
        assert_eq!(std::fs::read(dir.join("captura-1.png")).unwrap(), b"png!");
        assert_eq!(leido.clips[0].duracion_ms, Some(12_345));
        // Sin `.tmp` olvidados.
        assert!(!dir.join("estado.json.tmp").exists());

        let p = pendientes(&raiz);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].1.marca, T0.to_string());
        std::fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn un_diario_roto_no_se_entiende_pero_el_tmp_lo_salva() {
        let raiz = carpeta("roto");
        let d = Diario::nuevo(T0, "p", "Clase", Path::new("a.m4a.parcial"));
        let dir = d.carpeta(&raiz);
        d.guardar(&dir).unwrap();
        // El corte llego a mitad del renombrado: el bueno roto, el .tmp
        // entero.
        std::fs::copy(dir.join(FICHERO), dir.join("estado.json.tmp")).unwrap();
        std::fs::write(dir.join(FICHERO), b"{ \"marca\": ").unwrap();
        assert_eq!(Diario::leer(&dir).unwrap(), d);
        // Caso negativo: sin nada que se entienda, no hay diario (y
        // `pendientes` no se cae por ello).
        std::fs::remove_file(dir.join("estado.json.tmp")).unwrap();
        assert!(Diario::leer(&dir).is_err());
        assert!(pendientes(&raiz).is_empty());
        assert!(pendientes(&raiz.join("no-esta")).is_empty());
        std::fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn el_reloj_lleva_pausas_y_huecos_y_pasa_la_hora_a_minuto() {
        let mut d = Diario::nuevo(T0, "p", "Clase", Path::new("a"));
        // Diez minutos grabando.
        assert!(!d.latir(T0 + 600_000, 600_000, false));
        // Pausa de dos minutos.
        assert!(d.latir(T0 + 600_000, 600_000, true));
        assert!(!d.latir(T0 + 660_000, 600_000, true));
        assert!(d.latir(T0 + 720_000, 600_000, false));
        // Cinco minutos mas, y el equipo se suspende 30 minutos.
        d.latir(T0 + 1_020_000, 900_000, false);
        assert!(d.latir(T0 + 2_820_000 + 2_000, 902_000, false));
        assert_eq!(d.reloj.pausas.len(), 1);
        assert_eq!(d.reloj.huecos.len(), 1);
        assert_eq!(d.reloj.huecos[0].hasta, Some(T0 + 2_822_000));

        // Antes de la pausa, la hora es el minuto.
        assert_eq!(d.ms_en(T0 + 300_000), 300_000);
        // En la pausa, el minuto no corre.
        assert_eq!(d.ms_en(T0 + 690_000), 600_000);
        // Despues de la pausa y del hueco, se descuentan los dos.
        assert_eq!(d.ms_en(T0 + 2_822_000), 902_000);
        assert_eq!(d.ms_en(T0 + 2_882_000), 962_000);
        // Caso negativo: antes de empezar no hay minutos negativos.
        assert_eq!(d.ms_en(T0 - 50_000), 0);

        // Un atasco de la barra de pocos segundos no es un hueco.
        let mut e = Diario::nuevo(T0, "p", "Clase", Path::new("a"));
        assert!(!e.latir(T0 + 8_000, 1_000, false));
        assert!(e.reloj.huecos.is_empty());

        // Acabado, la hora de despues cae al final.
        d.acabar(T0 + 3_000_000);
        assert_eq!(d.ms_en(T0 + 9_000_000), d.ms_en(T0 + 3_000_000));
    }

    #[test]
    fn la_linea_de_tiempo_va_en_orden_y_dentro_de_la_nota() {
        let raiz = carpeta("linea");
        let mut d = Diario::nuevo(T0, "p", "Clase", Path::new("a"));
        let dir = raiz.join("d");
        d.apuntar_captura(&dir, 70_000, b"a", false, "png").unwrap();
        d.apuntar_captura(&dir, 10_000, b"b", true, "jpg").unwrap();
        d.empezar_clip(40_000, Path::new("c.mp4"));
        d.marcadores.push(Marcador {
            ms: 10_000,
            texto: "x".into(),
        });
        // Una captura de despues del final (la nota se corto antes que el
        // diario) cae al final, no fuera.
        d.apuntar_captura(&dir, 999_000, b"c", false, "png")
            .unwrap();
        let l = d.linea_de_tiempo(80_000);
        assert_eq!(
            l,
            vec![
                (10_000, Elemento::Captura(1)),
                (10_000, Elemento::Marcador(0)),
                (40_000, Elemento::Clip(0)),
                (70_000, Elemento::Captura(0)),
                (80_000, Elemento::Captura(2)),
            ]
        );
        std::fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn el_plan_sale_de_lo_que_quedo_en_el_disco() {
        let raiz = carpeta("plan");
        let audio = raiz.join("voz-1.m4a.parcial");
        let video = raiz.join("voz-1.m4a.parcial.video.mp4");
        let mut d = Diario::nuevo(T0, "p", "Clase", &audio);
        d.video = Some(video.clone());
        let dir = d.carpeta(&raiz);
        d.apuntar_captura(&dir, 1_000, b"a", false, "png").unwrap();
        d.apuntar_captura(&dir, 2_000, b"b", false, "png").unwrap();
        std::fs::remove_file(dir.join("captura-2.png")).unwrap();
        let clip_bueno = raiz.join("clip-1.mp4");
        let clip_roto = raiz.join("clip-2.mp4");
        std::fs::write(&clip_bueno, mp4_cerrado()).unwrap();
        std::fs::write(&clip_roto, mp4_cortado()).unwrap();
        d.empezar_clip(3_000, &clip_bueno);
        d.empezar_clip(4_000, &clip_roto);
        d.latir(T0 + 2_820_000, 2_820_000, false);
        // **Lo que deja un corte**: la nota a medias sin indice, su respaldo
        // de 47 minutos y el video sin indice.
        std::fs::write(con_sufijo(&audio, ".parte"), mp4_cortado()).unwrap();
        std::fs::write(con_sufijo(&video, ".parte"), mp4_cortado()).unwrap();
        let con_respaldo = |r: &Path| {
            r.ends_with("voz-1.m4a.parcial.respaldo.wav")
                .then_some(2_820_500)
        };

        let h = hallar(&dir, &d, con_respaldo);
        assert!(!h.audio_listo);
        assert_eq!(h.audio_parte, EstadoMp4::SinIndice);
        assert_eq!(h.capturas, vec![0]);
        assert_eq!(h.clips, vec![0]);
        let p = plan(&d, &h);
        assert_eq!(p.nota, Nota::DelRespaldo);
        assert_eq!(p.duracion_ms, 2_820_500);
        assert_eq!(p.minutos(), 47);
        assert_eq!(p.video, Video::Perdido);
        assert!(p.algo());

        // El `.parte` de la nota llego a cerrarse: se usa antes que el WAV.
        std::fs::write(con_sufijo(&audio, ".parte"), mp4_cerrado()).unwrap();
        let p = plan(&d, &hallar(&dir, &d, con_respaldo));
        assert_eq!(p.nota, Nota::DelParte);
        // Y el video fragmentado se abre.
        std::fs::write(
            con_sufijo(&video, ".parte"),
            [caja(b"ftyp", 8), caja(b"moov", 8), caja(b"moof", 8)].concat(),
        )
        .unwrap();
        assert_eq!(
            plan(&d, &hallar(&dir, &d, con_respaldo)).video,
            Video::DelParte
        );

        // Sin respaldo, lo que dura sale del ultimo latido.
        let p = plan(&d, &hallar(&dir, &d, |_| None));
        assert_eq!(p.duracion_ms, 2_820_000);

        // Caso negativo: no queda nada de nada.
        let vacio = Diario::nuevo(T0, "p", "Clase", &raiz.join("otra.m4a.parcial"));
        let p = plan(&vacio, &hallar(&raiz.join("x"), &vacio, |_| None));
        assert_eq!(p.nota, Nota::Ninguna);
        assert!(!p.algo());
        assert_eq!(p.minutos(), 0);
        std::fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn solo_la_nota_no_vuelve_a_meter_lo_que_ya_entro() {
        let raiz = carpeta("solo-nota");
        let audio = raiz.join("voz-1.m4a.parcial");
        let mut d = Diario::nuevo(T0, "p", "Clase", &audio);
        let dir = d.carpeta(&raiz);
        d.apuntar_captura(&dir, 1_000, b"a", false, "png").unwrap();
        d.solo_la_nota = true;
        let p = plan(&d, &hallar(&dir, &d, |_| Some(60_000)));
        assert_eq!(p.nota, Nota::DelRespaldo);
        assert!(p.capturas.is_empty());
        std::fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn descartar_se_lleva_lo_suyo_y_nada_mas() {
        let raiz = carpeta("descartar");
        let audio = raiz.join("voz-1.m4a.parcial");
        let video = raiz.join("voz-1.m4a.parcial.video.mp4");
        let mut d = Diario::nuevo(T0, "p", "Clase", &audio);
        d.video = Some(video.clone());
        let dir = d.carpeta(&raiz);
        d.apuntar_captura(&dir, 1_000, b"a", false, "png").unwrap();
        d.guardar(&dir).unwrap();
        let suyos = [
            con_sufijo(&audio, ".parte"),
            con_sufijo(&audio, ".respaldo.wav"),
            con_sufijo(&video, ".parte"),
        ];
        for f in &suyos {
            std::fs::write(f, b"x").unwrap();
        }
        let ajeno = raiz.join("voz-2.m4a");
        std::fs::write(&ajeno, b"de otra").unwrap();
        descartar(&dir, &d);
        assert!(suyos.iter().all(|f| !f.exists()));
        assert!(!dir.exists());
        assert!(ajeno.exists(), "descartar no se lleva lo que no es suyo");
        std::fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn el_espacio_avisa_a_los_dos_gigas_y_para_el_video_a_los_quinientos_megas() {
        const MB: u64 = 1024 * 1024;
        assert_eq!(espacio(Some(50_000 * MB)), Espacio::Bien);
        assert_eq!(espacio(Some(2_047 * MB)), Espacio::Poco);
        assert_eq!(espacio(Some(499 * MB)), Espacio::Critico);
        // Caso negativo: si no se sabe, se graba igual.
        assert_eq!(espacio(None), Espacio::Bien);
    }
}
