//! **El movil como camara de la reunion que graba el PC** (27-oct-2026):
//! el lado del PC, sin ventanas, para probarlo de punta a punta.
//!
//! El cable es `pixpin_sincro::camara`; aqui esta lo que hace el PC con
//! el:
//!
//! - **Lleva el reloj de la grabacion** ([`Camaras::empezar`],
//!   [`Camaras::seguir_a`] con lo que dice la grabadora a cada vuelta de la
//!   barra, [`Camaras::acabar`]) y lo anuncia ([`Camaras::anuncio`]): el id,
//!   el titulo, el milisegundo en que va y si esta en pausa.
//! - **Recibe cada foto o clip** ([`Camaras::recibir`]): su minuto es el que
//!   calculo el movil (`ms`) o, si no lo trae, el de su hora con el desfase
//!   de los relojes (`pixpin_sincro::camara::ms_de_la_foto`).
//! - **Lo pone en la clase** ([`adjuntar`]) como una captura mas: una
//!   IMAGEN (o el ARCHIVO `.mp4` de un clip) con `resto.enGrabacion` (`{de,
//!   ms}`, y `duracionMs` el clip), `resto.fuente: "movil"` ([`FUENTE`]), en la lista de
//!   la nota (`capturas` o `clips`) y dentro de su carpeta, en su minuto.
//!   La foto entra tal cual llego: sus bytes, con su EXIF.
//!
//! **Cuando entra**: la clase se guarda al «Terminar» (la nota de voz no
//! existe antes). Lo que llega grabando espera en una cola, en disco, y
//! entra al guardarse ([`Camaras::guardada`]) con su minuto. Lo que llega
//! despues (el movil lo subio tarde, sin red) entra al momento en la clase
//! ya guardada. Las ultimas grabaciones acabadas se recuerdan un rato
//! ([`ACABADAS`]) para eso.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pixpin_sincro::al_lienzo::Suelto;
use pixpin_sincro::camara::{self, Anuncio, Reloj};

use crate::{almacen, carpeta_de_paginas, clase, cuaderno};

/// La clave de `resto` que dice que lo hizo la camara del movil, y su
/// valor ([`MOVIL`]). No es `origen`: ese nombre ya es un campo del mensaje
/// (el aparato donde nacio, `Mensaje::origen`) y escribirlo en `resto` lo
/// pisaria al guardar.
pub const FUENTE: &str = "fuente";
pub const MOVIL: &str = "movil";

/// Si un mensaje lo hizo la camara del movil en una grabacion.
pub fn es_del_movil(m: &cuaderno::Mensaje) -> bool {
    m.resto.get(FUENTE).and_then(|v| v.as_str()) == Some(MOVIL)
}

/// Cuantas grabaciones acabadas se recuerdan para lo que llegue tarde.
pub const ACABADAS: usize = 4;
/// Lo que se deja pasar entre la grabadora y el reloj antes de corregirlo.
const TOLERANCIA_MS: i64 = 250;

/// **Donde quedo una clase guardada**: el proyecto, su nota (si entro) y su
/// carpeta (si se hizo). Con esto entra lo que llega del movil.
#[derive(Clone)]
pub struct Destino {
    pub raiz: PathBuf,
    pub proyecto: String,
    pub voz: Option<String>,
    pub carpeta: Option<String>,
    /// El codigo de este equipo para el sello (`K7Q2`).
    pub aparato: String,
    /// La hora del mensaje nuevo, sin repetir milisegundo con otro de este
    /// equipo (el id de un mensaje es su hora).
    pub hora: fn() -> i64,
}

impl std::fmt::Debug for Destino {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Destino")
            .field("proyecto", &self.proyecto)
            .field("voz", &self.voz)
            .field("carpeta", &self.carpeta)
            .finish()
    }
}

/// Una foto o un clip llegado, esperando a que la clase se guarde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Llegada {
    pub ms: i64,
    /// Un clip: lo que dura. Una foto, 0.
    pub duracion_ms: i64,
    pub clip: bool,
    /// Donde espera (en la cola de [`Camaras`]).
    pub fichero: PathBuf,
    pub nombre: String,
    /// Del aparato que la hizo.
    pub de: String,
    pub tomada_en: i64,
}

/// Lo que acaba de llegar, para el aviso de la barra («Foto del móvil ·
/// 12:34»).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recibida {
    pub ms: i64,
    pub clip: bool,
    pub de: String,
}

/// Una grabacion, en curso o acabada.
#[derive(Debug)]
struct Grabacion {
    id: String,
    titulo: String,
    reloj: Reloj,
    /// La de verdad, la de la nota, cuando se sabe.
    duracion: Option<i64>,
    cola: Vec<Llegada>,
    destino: Option<Destino>,
    recibidas: usize,
}

impl Grabacion {
    fn tope(&self, ahora: i64) -> i64 {
        self.duracion
            .unwrap_or_else(|| self.reloj.duracion(ahora))
            .max(0)
    }
}

/// **Las grabaciones de este PC que el movil puede ver**: la que esta en
/// curso y las ultimas acabadas.
#[derive(Debug)]
pub struct Camaras {
    actual: Option<Grabacion>,
    acabadas: Vec<Grabacion>,
    /// Donde espera lo que llega antes de que se guarde la clase.
    cola: PathBuf,
    /// El nombre de este equipo («Cámara para la reunión del PC …»).
    pub aparato: String,
    /// Si se anuncia y se aceptan fotos (el interruptor «📱 Cámara del
    /// móvil» de la barra).
    pub encendida: bool,
    /// Los codecs de video que sabe ver este equipo (`h264`, `hevc`).
    pub codecs: Vec<String>,
    /// El desfase medido con cada aparato (lo que va adelantado su reloj).
    desfases: BTreeMap<String, i64>,
    nuevas: Vec<Recibida>,
}

impl Camaras {
    pub fn nuevas(cola: PathBuf, aparato: &str) -> Camaras {
        Camaras {
            actual: None,
            acabadas: Vec::new(),
            cola,
            aparato: aparato.to_string(),
            encendida: true,
            codecs: vec![camara::H264.into()],
            desfases: BTreeMap::new(),
            nuevas: Vec::new(),
        }
    }

    /// El id de la que esta en curso.
    pub fn en_curso(&self) -> Option<&str> {
        self.actual.as_ref().map(|g| g.id.as_str())
    }

    /// Lo llegado a la que esta en curso.
    pub fn recibidas(&self) -> usize {
        self.actual.as_ref().map_or(0, |g| g.recibidas)
    }

    /// **Empieza una grabacion** con id `id`, que ya va por `ms`. La que
    /// hubiera en curso se da por acabada.
    pub fn empezar(&mut self, id: &str, titulo: &str, ahora: i64, ms: i64) {
        self.acabar(ahora);
        self.actual = Some(Grabacion {
            id: id.to_string(),
            titulo: titulo.to_string(),
            reloj: Reloj::empezar(ahora, ms),
            duracion: None,
            cola: Vec::new(),
            destino: None,
            recibidas: 0,
        });
    }

    /// Con lo que dice la grabadora a cada vuelta de la barra. Devuelve si
    /// cambio la pausa (para avisar al movil).
    pub fn seguir_a(&mut self, ahora: i64, ms: i64, pausa: bool) -> bool {
        let Some(g) = self.actual.as_mut() else {
            return false;
        };
        let antes = g.reloj.en_pausa();
        g.reloj.seguir_a(ahora, ms, pausa, TOLERANCIA_MS);
        antes != g.reloj.en_pausa()
    }

    /// **Acaba la que esta en curso.** Lo suyo queda en la cola hasta
    /// [`Camaras::guardada`].
    pub fn acabar(&mut self, ahora: i64) {
        if let Some(mut g) = self.actual.take() {
            g.reloj.acabar(ahora);
            self.acabadas.insert(0, g);
            while self.acabadas.len() > ACABADAS {
                if let Some(vieja) = self.acabadas.pop() {
                    tirar_cola(&vieja);
                }
            }
        }
    }

    /// **La grabacion se tiro** sin guardarse: lo suyo tambien.
    pub fn tirar(&mut self) {
        if let Some(g) = self.actual.take() {
            tirar_cola(&g);
        } else if let Some(i) = self.acabadas.iter().position(|g| g.destino.is_none()) {
            tirar_cola(&self.acabadas.remove(i));
        }
    }

    /// **La clase se guardo** en `destino` y su nota dura `duracion` (si
    /// entro): lo que esperaba entra ya, en su minuto. Va a la ultima
    /// acabada sin guardar (o a la de ahora, si aun no se acabo). Devuelve
    /// cuantas entraron.
    pub fn guardada(&mut self, destino: Destino, duracion: Option<i64>, ahora: i64) -> usize {
        if self.acabadas.iter().all(|g| g.destino.is_some()) {
            self.acabar(ahora);
        }
        let Some(g) = self.acabadas.iter_mut().find(|g| g.destino.is_none()) else {
            return 0;
        };
        if let Some(d) = duracion.filter(|d| *d > 0) {
            g.duracion = Some(d);
        }
        let tope = g.tope(ahora);
        let mut hechas = 0;
        let mut quedan = Vec::new();
        for mut l in std::mem::take(&mut g.cola) {
            l.ms = l.ms.clamp(0, tope);
            l.duracion_ms = l.duracion_ms.min((tope - l.ms).max(0));
            match adjuntar(&destino, &g.titulo, &l) {
                Ok(_) => hechas += 1,
                // No se tira: sigue en la cola, en su carpeta.
                Err(_) => quedan.push(l),
            }
        }
        g.cola = quedan;
        g.destino = Some(destino);
        hechas
    }

    /// **Lo que se anuncia al movil**: la de ahora; si no hay, la ultima
    /// acabada (para que sepa que acabo y que aun puede subir lo suyo).
    /// `None` apagada o sin ninguna.
    pub fn anuncio(&self, ahora: i64) -> Option<Anuncio> {
        if !self.encendida {
            return None;
        }
        let (g, activa) = match &self.actual {
            Some(g) => (g, true),
            None => (self.acabadas.first()?, false),
        };
        Some(Anuncio {
            grabacion: g.id.clone(),
            titulo: g.titulo.clone(),
            aparato: self.aparato.clone(),
            activa,
            en_pausa: g.reloj.en_pausa(),
            ms_actual: g.reloj.ms_en(ahora),
            duracion: if activa { 0 } else { g.tope(ahora) },
            desde: g.reloj.desde,
            pausas: g.reloj.pausas_al_cable(),
            codecs: self.codecs.clone(),
            clip_maximo_ms: camara::CLIP_MAXIMO_MS,
            ..Anuncio::nada()
        })
    }

    /// Lo que va adelantado el reloj de `aparato`, medido al avisarle.
    pub fn medir(&mut self, aparato: &str, desfase: i64) {
        self.desfases.insert(aparato.to_string(), desfase);
    }

    fn buscar(&mut self, id: &str) -> Option<&mut Grabacion> {
        self.actual
            .iter_mut()
            .chain(self.acabadas.iter_mut())
            .find(|g| g.id == id)
    }

    /// **Antes del «vale»**: que la grabacion exista aqui y se acepten
    /// fotos.
    pub fn aceptar(&mut self, p: &Suelto) -> Result<(), String> {
        camara::aceptar(p)?;
        if !self.encendida {
            return Err("La cámara del móvil está apagada en el PC".into());
        }
        if self.buscar(&p.grabacion).is_none() {
            return Err("Esa grabación ya no está en el PC".into());
        }
        Ok(())
    }

    /// **Una foto o un clip llego** a `fichero` (temporal) de `de`
    /// (`de_id`, para su desfase). Entra ya si la clase esta guardada; si
    /// no, espera en la cola. Devuelve su minuto.
    pub fn recibir(
        &mut self,
        p: &Suelto,
        fichero: &Path,
        de_id: &str,
        de: &str,
        ahora: i64,
    ) -> Result<i64, String> {
        self.aceptar(p)?;
        let medido = self.desfases.get(de_id).copied();
        let cola = self.cola.clone();
        let g = self
            .buscar(&p.grabacion)
            .ok_or_else(|| "Esa grabación ya no está en el PC".to_string())?;
        let tope = g.tope(ahora);
        let ms = camara::ms_de_la_foto(&g.reloj, ahora, Some(tope), p, medido);
        let clip = camara::es_clip(p);
        let duracion_ms = if clip {
            camara::duracion_del_clip(p, ms, tope)
        } else {
            0
        };
        let nombre = nombre_de(p, clip, g.recibidas + 1);
        let mut l = Llegada {
            ms,
            duracion_ms,
            clip,
            fichero: fichero.to_path_buf(),
            nombre,
            de: de.to_string(),
            tomada_en: p.tomada_en,
        };
        match g.destino.clone() {
            Some(d) => {
                adjuntar(&d, &g.titulo, &l).map_err(|e| format!("No se pudo guardar: {e}"))?;
            }
            None => {
                let dir = cola.join(almacen::nombre_seguro(&g.id));
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let sitio = dir.join(format!("{}-{}", g.cola.len(), l.nombre));
                mover(fichero, &sitio).map_err(|e| format!("No se pudo guardar: {e}"))?;
                l.fichero = sitio;
                g.cola.push(l);
            }
        }
        g.recibidas += 1;
        self.nuevas.push(Recibida {
            ms,
            clip,
            de: de.to_string(),
        });
        Ok(ms)
    }

    /// Lo llegado desde la ultima vez que se pregunto.
    pub fn tomar_nuevas(&mut self) -> Vec<Recibida> {
        std::mem::take(&mut self.nuevas)
    }
}

fn tirar_cola(g: &Grabacion) {
    for l in &g.cola {
        let _ = std::fs::remove_file(&l.fichero);
    }
    if let Some(dir) = g.cola.first().and_then(|l| l.fichero.parent()) {
        let _ = std::fs::remove_dir(dir);
    }
}

fn mover(desde: &Path, hasta: &Path) -> std::io::Result<()> {
    if std::fs::rename(desde, hasta).is_err() {
        std::fs::copy(desde, hasta)?;
        let _ = std::fs::remove_file(desde);
    }
    Ok(())
}

/// El nombre con que entra: el suyo si lo trae con una extension que cuadra;
/// si no, `movil-N.jpg` / `.mp4`.
fn nombre_de(p: &Suelto, clip: bool, n: usize) -> String {
    let suyo = Path::new(&p.nombre)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let cuadra = pixpin_sincro::al_lienzo::mime_de(&suyo) == p.mime;
    if cuadra && !suyo.trim().is_empty() {
        return suyo;
    }
    let ext = match p.mime.as_str() {
        "image/png" => "png",
        "image/webp" => "webp",
        "video/mp4" => "mp4",
        _ if clip => "mp4",
        _ => "jpg",
    };
    format!("movil-{n}.{ext}")
}

/// El minuto como en el pie de las capturas: `12:34` o `1:02:03`.
pub fn minuto(ms: i64) -> String {
    let s = ms.max(0) / 1000;
    let (h, m, s) = (s / 3600, (s / 60) % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// **Pone una foto o un clip del movil en la clase guardada** `destino`:
/// el mensaje (con el fichero movido dentro del proyecto, tal cual llego),
/// su enlace a la nota, su sitio en la lista de la nota y en la carpeta.
/// Devuelve el id del mensaje.
pub fn adjuntar(destino: &Destino, titulo: &str, l: &Llegada) -> std::io::Result<String> {
    let raiz = &destino.raiz;
    let proyecto = &destino.proyecto;
    let carpeta = almacen::carpeta(raiz, proyecto);
    let numero = match cuaderno::Cuaderno::leer_de(&carpeta) {
        Ok(c) => c.siguiente_numero(Some(proyecto)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 1,
        Err(e) => return Err(e),
    };
    let (ruta, bytes) = almacen::mover_adjunto(raiz, proyecto, &l.nombre, &l.fichero)?;
    let cuando = (destino.hora)();
    let mut m = cuaderno::Mensaje::adjunto(
        cuaderno::clase_de_nombre(&l.nombre),
        &l.nombre,
        &ruta,
        bytes,
        &cuaderno::Sello {
            cuando,
            numero,
            aparato: destino.aparato.clone(),
            proyecto: proyecto.clone(),
        },
    );
    let rotulo = format!("📱 ⏱ {}", minuto(l.ms));
    m.texto = if titulo.trim().is_empty() {
        rotulo.clone()
    } else {
        format!("{rotulo} · {}", titulo.trim())
    };
    if let Some(voz) = &destino.voz {
        let mut enlace = serde_json::json!({ "de": voz, "ms": l.ms });
        if l.clip {
            enlace["duracionMs"] = serde_json::json!(l.duracion_ms);
        }
        m.resto.insert("enGrabacion".into(), enlace);
    }
    m.resto.insert(FUENTE.into(), serde_json::json!(MOVIL));
    if !l.de.is_empty() {
        m.resto
            .insert("deAparato".into(), serde_json::json!(l.de.clone()));
    }
    if l.tomada_en > 0 {
        m.resto
            .insert("tomadaEn".into(), serde_json::json!(l.tomada_en));
    }
    cuaderno::anadir(&carpeta, &m)?;
    let id = m.id.clone();
    if let Some(voz) = &destino.voz {
        cuaderno::cambiar(&carpeta, voz, |n| {
            if l.clip {
                clase::poner_clip(&mut n.resto, l.ms, l.duracion_ms, &id);
            } else {
                clase::poner_captura(&mut n.resto, l.ms, &id);
            }
            true
        })?;
    }
    if let Some(c) = &destino.carpeta {
        carpeta_de_paginas::cambiar_en_disco(&carpeta, c, |c| {
            c.anadir_en_su_minuto(&id, &rotulo, l.ms)
        })?;
    }
    let mut indice = almacen::Indice::leer(raiz);
    if let Some(f) = indice.proyectos.iter_mut().find(|f| &f.id == proyecto) {
        f.tocado = f.tocado.max(cuando);
        indice.guardar(raiz)?;
    }
    Ok(id)
}

// ------------------------------------------------------------ al cable

/// **Lo que atiende la camara** en una conexion con `de` (lo da quien
/// recibe los sueltos, ver `pixpin_sincro::al_lienzo::Recibe::camara`).
pub struct Recepcion {
    pub camaras: Arc<Mutex<Camaras>>,
    pub de_id: String,
    pub de_nombre: String,
    pub ahora: fn() -> i64,
}

impl Recepcion {
    fn con<T>(&self, f: impl FnOnce(&mut Camaras) -> T) -> T {
        let mut c = self.camaras.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut c)
    }
}

impl camara::Camara for Recepcion {
    fn anuncio(&mut self) -> Option<Anuncio> {
        let ahora = (self.ahora)();
        self.con(|c| c.anuncio(ahora))
    }

    fn aceptar_foto(&mut self, p: &Suelto) -> Result<(), String> {
        self.con(|c| c.aceptar(p))
    }

    fn guardar_foto(&mut self, p: &Suelto, fichero: &Path) -> Result<i64, String> {
        let ahora = (self.ahora)();
        let (id, nombre) = (self.de_id.clone(), self.de_nombre.clone());
        self.con(|c| c.recibir(p, fichero, &id, &nombre, ahora))
    }
}

/// Un `Recibe` que solo atiende la camara: lo demas («suelto» al chat
/// abierto) lo niega. Para quien no tiene otra cosa que hacer con los
/// sueltos (las pruebas, una herramienta).
impl pixpin_sincro::al_lienzo::Recibe for Recepcion {
    fn aceptar(&mut self, _: &Suelto) -> Result<(), String> {
        Err(format!(
            "No sé qué es «{}»",
            pixpin_sincro::al_lienzo::SUELTO
        ))
    }

    fn guardar(
        &mut self,
        _: &Suelto,
        _: &Path,
    ) -> Result<pixpin_sincro::al_lienzo::Llegada, String> {
        Err(format!(
            "No sé qué es «{}»",
            pixpin_sincro::al_lienzo::SUELTO
        ))
    }

    fn camara(&mut self) -> Option<&mut dyn camara::Camara> {
        Some(self)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const S: i64 = 1000;
    const PARED: i64 = 1_761_550_000_000;

    fn foto(grabacion: &str, ms: Option<i64>) -> Suelto {
        Suelto {
            nombre: "IMG_1.jpg".into(),
            mime: "image/jpeg".into(),
            bytes: 4,
            destino: camara::DESTINO.into(),
            grabacion: grabacion.into(),
            ms,
            ..Default::default()
        }
    }

    fn temporal(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-camara-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join(nombre);
        std::fs::write(&f, [0xFF, 0xD8, 0xFF, 0xE1]).unwrap();
        f
    }

    #[test]
    fn el_anuncio_dice_en_que_minuto_va_y_si_esta_en_pausa() {
        let dir = std::env::temp_dir().join(format!("pixpin-camara-a-{}", std::process::id()));
        let mut c = Camaras::nuevas(dir, "Portatil");
        assert_eq!(c.anuncio(PARED), None);
        c.empezar("g1", "Clase 27-10", PARED, 0);
        assert!(!c.seguir_a(PARED + 10 * S, 10 * S, false));
        assert!(c.seguir_a(PARED + 12 * S, 12 * S, true), "cambio la pausa");
        let a = c.anuncio(PARED + 20 * S).unwrap();
        assert!(a.grabando() && a.en_pausa);
        assert_eq!(a.ms_actual, 12 * S);
        assert_eq!(a.aparato, "Portatil");
        assert_eq!(a.codec_del_clip(), Some(camara::H264));
        c.acabar(PARED + 30 * S);
        let a = c.anuncio(PARED + 40 * S).unwrap();
        assert!(!a.activa, "acabo, pero se dice cual era");
        assert_eq!((a.grabacion.as_str(), a.duracion), ("g1", 12 * S));
        // Apagada, nada.
        c.encendida = false;
        assert_eq!(c.anuncio(PARED + 40 * S), None);
    }

    #[test]
    fn caso_negativo_lo_de_otra_grabacion_o_apagada_no_entra() {
        let dir = std::env::temp_dir().join(format!("pixpin-camara-b-{}", std::process::id()));
        let mut c = Camaras::nuevas(dir, "Portatil");
        c.empezar("g1", "", PARED, 0);
        assert!(c.aceptar(&foto("g1", Some(1))).is_ok());
        assert!(c.aceptar(&foto("otra", Some(1))).is_err());
        let f = temporal("x.jpg");
        assert!(
            c.recibir(&foto("otra", None), &f, "tel", "Tel", PARED)
                .is_err()
        );
        c.encendida = false;
        assert!(c.aceptar(&foto("g1", Some(1))).is_err());
        assert!(c.tomar_nuevas().is_empty());
        // Tirada, lo que esperaba se va.
        c.encendida = true;
        let ms = c.recibir(&foto("g1", Some(500)), &f, "tel", "Tel", PARED + S);
        assert_eq!(ms, Ok(500));
        assert_eq!(c.recibidas(), 1);
        c.tirar();
        assert_eq!(c.en_curso(), None);
        assert!(c.aceptar(&foto("g1", Some(1))).is_err());
    }

    #[test]
    fn el_nombre_es_el_suyo_si_cuadra_y_si_no_uno_de_serie() {
        let mut p = foto("g", None);
        assert_eq!(nombre_de(&p, false, 3), "IMG_1.jpg");
        p.nombre = "../../malo.exe".into();
        assert_eq!(nombre_de(&p, false, 3), "movil-3.jpg");
        p.nombre = "clip.mp4".into();
        p.mime = "video/mp4".into();
        assert_eq!(nombre_de(&p, true, 4), "clip.mp4");
        assert_eq!(minuto(754_000), "12:34");
        assert_eq!(minuto(3_723_000), "1:02:03");
    }
}
