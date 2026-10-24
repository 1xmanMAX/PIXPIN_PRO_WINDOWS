//! **Escuchar un documento en voz alta** en los lectores (Word, libro,
//! Markdown y PDF). Es `ui/LectorEnVoz.kt`, `ui/LeyendoEnVozService.kt` y la
//! `BarraDeEscuchar` de `ui/VisorHtmlActivity.kt` del movil (22/23-sep-2026,
//! v0.89 `1e66199` y v0.90 `6e97552`).
//!
//! # Un solo lector de voz para la aplicacion
//!
//! Como en el movil (`LectorEnVoz.delApp`, «que el audio continue al salir y
//! se ponga en pantalla como si estuviera reproduciendo un audio»), **la voz
//! no es de la ventana del lector**: vive en un hilo propio de la aplicacion
//! (lo arranca `main`, [`iniciar`]) y **sigue sonando al cerrar el lector**.
//! El lector solo se engancha a ella ([`LeerEnVoz`]) para resaltar lo que
//! suena y mover el verde; al volver a abrir el mismo documento se engancha
//! otra vez y va al parrafo que suena ([`LeerEnVoz::enganchar`]).
//!
//! Lo que en el movil es la notificacion de reproductor aqui son **los
//! mandos de reproductor de Windows** (`pixpin_shell::reproductor_sistema`):
//! el titulo del documento y «Parrafo 3 de 40 · voz» en el recuadro del
//! volumen y la pantalla de bloqueo, con pausa, anterior, siguiente y parar,
//! y **las teclas multimedia del teclado**. Con el lector cerrado, el
//! **marcador verde se sigue apuntando** en cada parrafo (el `apuntar` que
//! deja cada lector, el `ExportarDocumentoAnotado.ponerVoz` del movil).
//!
//! # Dos voces
//!
//! - **La de Windows** (`ISpVoice`, `pixpin_voz::sapi::Lector`): ya esta en
//!   el equipo, no pesa y el texto no sale de el. En el movil es la de Google.
//! - **Las de Microsoft** (`pixpin_voz::edge`, las de «Leer en voz alta» de
//!   Edge), si se encienden en el engranaje: muy naturales, gratis, pero no
//!   es un servicio oficial y el texto va a Microsoft. Cada parrafo llega
//!   como un MP3 y suena con Media Foundation (`pixpin_audio::Salida`, que
//!   cambia la velocidad sin cambiar el tono, como el `MediaPlayer` del
//!   movil). Se piden tres por delante para que no haya silencios. Si
//!   Microsoft no responde, **sigue la de Windows desde ese parrafo**
//!   (`caerAGoogle`).
//!
//! Las cuentas (trozos, idioma, velocidad, el verde) estan en
//! `pixpin_docs::voz_alta`; lo de Microsoft, en `pixpin_voz::edge`.
//!
//! Diferencias con el movil, porque Windows es otro: no hay auricular de
//! llamadas ni foco de audio; tocar los mandos de Windows no abre el lector
//! (una app de escritorio no tiene ese enlace). La pausa de SAPI es mejor:
//! se para a media palabra y sigue ahi.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use pixpin_docs::voz_alta;
use pixpin_render::icono::material;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_shell::reproductor_sistema::{BotonDelSistema, MandosDelSistema};
use pixpin_voz::edge::{self, Voz};
use pixpin_voz::sapi::Lector;

use crate::lector::{APAGADO, CRISTAL, TEXTO, ahora_ms, con_alfa};

/// El ambar del parrafo que suena (`.pixpin-leyendo` del movil:
/// `rgba(255,196,64,.30)`).
pub const AMBAR_LEYENDO: Color = Color {
    r: 1.0,
    g: 196.0 / 255.0,
    b: 64.0 / 255.0,
    a: 0.30,
};

/// Lo que tarda como poco la voz en ponerse a hablar tras pedirselo: antes
/// de esto su «he acabado» puede ser el de lo de antes.
const MS_PARA_EMPEZAR: u64 = 150;

/// Cuantos parrafos de Microsoft se piden por delante del que suena
/// (`LectorEnVoz.POR_DELANTE`).
const POR_DELANTE: usize = 3;

/// Lo que paso desde la ultima vuelta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suceso {
    /// Suena (o se paro en) este parrafo.
    Parrafo(usize),
    /// Se acabo el documento.
    Acabado,
    /// La voz ya no es de este documento: se paro desde los mandos de
    /// Windows o se empezo a escuchar otro.
    Fuera,
    /// Se paro, siguio o cambio de voz sin cambiar de parrafo: la barra se
    /// repinta.
    Cambio,
}

/// Los botones de la barra de escuchar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotonVoz {
    Anterior,
    Alternar,
    Siguiente,
    Velocidad,
    Cerrar,
}

/// Los textos que el hilo de la voz ensena, ya traducidos (el hilo no tiene
/// el catalogo).
#[derive(Debug, Clone, Default)]
pub struct Rotulos {
    /// «Párrafo {n} de {total}».
    pub parrafo: String,
    pub preparando: String,
    pub no_respondio: String,
    pub sin_voz: String,
}

impl Rotulos {
    pub fn de(textos: &pixpin_store::Catalogo) -> Rotulos {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("n", "{n}");
        args.set("total", "{total}");
        Rotulos {
            parrafo: textos.t_args("lector-voz-parrafo", &args),
            preparando: textos.t("lector-voz-preparando"),
            no_respondio: textos.t("lector-voz-no-respondio"),
            sin_voz: textos.t("lector-voz-sin-voz"),
        }
    }

    fn donde(&self, parrafo: usize, cuantos: usize) -> String {
        self.parrafo
            .replace("{n}", &(parrafo + 1).to_string())
            .replace("{total}", &cuantos.to_string())
    }
}

/// **Apunta el verde** de un documento con su lector cerrado: `Some(i)`, el
/// parrafo que suena; `None`, se acabo (fuera el verde).
pub type Apuntar = Box<dyn Fn(Option<usize>) + Send>;

/// **Lo que hay que leer**, de que documento y como.
pub struct Documento {
    /// De que documento es (su ruta): el lector la compara con la suya.
    pub clave: PathBuf,
    /// Lo que se ensena en los mandos de Windows.
    pub titulo: String,
    pub parrafos: Vec<String>,
    /// El idioma con el que se lee, y el de reserva (el de la app).
    pub idioma: String,
    pub reserva: String,
    pub velocidad: f32,
    /// Leer con las voces de Microsoft (el engranaje), y la elegida para
    /// este idioma si la hay.
    pub microsoft: bool,
    pub preferida: Option<String>,
    /// Donde viven las caches (`cache/voz-edge`).
    pub raiz: PathBuf,
    pub rotulos: Rotulos,
    pub apuntar: Apuntar,
}

/// Lo que el hilo de la voz y los lectores comparten.
#[derive(Debug, Clone, Default)]
struct Comun {
    /// Cada documento nuevo es otra vuelta: un lector enganchado a la de
    /// antes ya no manda.
    vuelta: u64,
    /// Hay documento (no se solto).
    vivo: bool,
    clave: PathBuf,
    titulo: String,
    idioma: String,
    cuantos: usize,
    actual: usize,
    leyendo: bool,
    velocidad: f32,
    /// La voz que lee, para ensenarla: «Elvira · Microsoft · en línea».
    voz: String,
    /// Cuantos lectores tiene abiertos el documento: sin ninguno, el verde
    /// lo apunta el hilo.
    enganchados: usize,
    /// Cuantas veces se acabo el documento.
    acabados: u64,
}

enum Que {
    Leer(usize),
    Situar(usize),
    Alternar,
    Velocidad(f32),
    /// Leer con esta voz de Microsoft (nombre, etiqueta), o con la de
    /// Windows si `None`.
    Microsoft(Option<(String, String)>),
    /// Encender Microsoft a medias: hay que buscar la voz del idioma.
    BuscarMicrosoft(Option<String>),
    Soltar,
}

enum Orden {
    Documento(Box<Documento>, u64),
    De(u64, Que),
    /// La lista de voces de Microsoft, pedida para la vuelta `u64`.
    Voces(u64, Vec<Voz>, Option<String>),
    Apagar,
}

struct Lado {
    tx: Mutex<Sender<Orden>>,
    comun: Arc<Mutex<Comun>>,
}

static LADO: OnceLock<Lado> = OnceLock::new();

fn lado() -> &'static Lado {
    LADO.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        let comun = Arc::new(Mutex::new(Comun::default()));
        let c = Arc::clone(&comun);
        let tx_hilo = tx.clone();
        let lanzado = std::thread::Builder::new()
            .name("voz-de-la-app".into())
            .spawn(move || Hilo::nuevo(c, tx_hilo).correr(rx));
        if let Err(e) = lanzado {
            tracing::warn!(?e, "sin hilo para leer en voz alta");
        }
        Lado {
            tx: Mutex::new(tx),
            comun,
        }
    })
}

fn comun() -> MutexGuard<'static, Comun> {
    lado().comun.lock().unwrap_or_else(|e| e.into_inner())
}

fn mandar(o: Orden) {
    let l = lado();
    let _ = l.tx.lock().unwrap_or_else(|e| e.into_inner()).send(o);
}

/// **Arranca el lector de voz de la aplicacion** (desde `main`): un hilo que
/// espera sin gastar nada hasta que alguien escucha un documento.
pub fn iniciar() {
    let _ = lado();
}

/// Calla y suelta todo al salir de la aplicacion.
pub fn apagar() {
    if LADO.get().is_some() {
        mandar(Orden::Apagar);
    }
}

/// **Calla lo que suene, sea de quien sea** (15-oct, el asistente: «para
/// de leer»): como la X de la barra del lector que lo puso a leer.
pub fn callar_lo_que_suene() {
    if LADO.get().is_some() {
        let vuelta = comun().vuelta;
        mandar(Orden::De(vuelta, Que::Soltar));
    }
}

/// Si lo que suena (o esta en pausa) es este documento.
pub fn suena(clave: &Path) -> bool {
    LADO.get().is_some() && {
        let c = comun();
        c.vivo && c.clave == clave
    }
}

/// El idioma con el que se esta leyendo, si se lee algo.
pub fn idioma_que_suena() -> Option<String> {
    LADO.get()?;
    let c = comun();
    (c.vivo && !c.idioma.is_empty()).then(|| c.idioma.clone())
}

/// **Un lector enganchado a la voz de la aplicacion.** Soltarlo (cerrar el
/// lector) no calla la voz: para eso, [`LeerEnVoz::soltar`].
pub struct LeerEnVoz {
    vuelta: u64,
    visto: Option<usize>,
    acabados: u64,
    fuera: bool,
    /// Lo ultimo que se le conto al lector: si suena y con que voz (la barra
    /// cambia si se pausa desde los mandos de Windows).
    visto_leyendo: bool,
    visto_voz: String,
}

impl LeerEnVoz {
    /// **Empieza a escuchar `doc`** (se calla lo que sonara). No suena hasta
    /// [`LeerEnVoz::leer`]. `None` si no hay con que leer: sin Microsoft y
    /// sin voz de Windows para el idioma ni para el de reserva. COM tiene
    /// que estar iniciado en el hilo (lo esta en los lectores).
    pub fn arrancar(doc: Documento) -> Option<LeerEnVoz> {
        if !doc.microsoft
            && Lector::nuevo(&doc.idioma)
                .or_else(|| Lector::nuevo(&doc.reserva))
                .is_none()
        {
            return None;
        }
        let (vuelta, acabados) = {
            let mut c = comun();
            let vuelta = c.vuelta + 1;
            let acabados = c.acabados;
            *c = Comun {
                vuelta,
                vivo: true,
                clave: doc.clave.clone(),
                titulo: doc.titulo.clone(),
                idioma: doc.idioma.clone(),
                cuantos: doc.parrafos.len(),
                actual: 0,
                leyendo: false,
                velocidad: doc.velocidad,
                voz: doc.rotulos.preparando.clone(),
                enganchados: 1,
                acabados,
            };
            (vuelta, acabados)
        };
        mandar(Orden::Documento(Box::new(doc), vuelta));
        Some(LeerEnVoz {
            vuelta,
            visto: None,
            acabados,
            fuera: false,
            visto_leyendo: false,
            visto_voz: String::new(),
        })
    }

    /// **Se engancha a lo que suena** si es `clave` y tiene `cuantos`
    /// parrafos (el mismo documento contado igual): al volver a abrirlo.
    pub fn enganchar(clave: &Path, cuantos: usize) -> Option<LeerEnVoz> {
        LADO.get()?;
        let mut c = comun();
        if !c.vivo || c.clave != clave || c.cuantos != cuantos {
            return None;
        }
        c.enganchados += 1;
        Some(LeerEnVoz {
            vuelta: c.vuelta,
            visto: Some(c.actual),
            acabados: c.acabados,
            fuera: false,
            visto_leyendo: c.leyendo,
            visto_voz: c.voz.clone(),
        })
    }

    fn mio<T>(&self, f: impl FnOnce(&Comun) -> T, si_no: T) -> T {
        let c = comun();
        if c.vuelta == self.vuelta && c.vivo {
            f(&c)
        } else {
            si_no
        }
    }

    fn mandar(&self, q: Que) {
        mandar(Orden::De(self.vuelta, q));
    }

    pub fn cuantos(&self) -> usize {
        self.mio(|c| c.cuantos, 0)
    }

    /// El parrafo que suena (o donde se paro).
    pub fn actual(&self) -> usize {
        self.mio(|c| c.actual, 0)
    }

    pub fn leyendo(&self) -> bool {
        self.mio(|c| c.leyendo, false)
    }

    pub fn velocidad(&self) -> f32 {
        self.mio(|c| c.velocidad, 1.0)
    }

    /// «Microsoft Helena Desktop · Windows», «Elvira · Microsoft · en
    /// línea»: para la barra.
    pub fn nombre(&self) -> String {
        self.mio(|c| c.voz.clone(), String::new())
    }

    /// Empieza a leer en el parrafo `desde`, por su principio.
    pub fn leer(&mut self, desde: usize) {
        let a = {
            let mut c = comun();
            if c.vuelta != self.vuelta || c.cuantos == 0 {
                return;
            }
            c.actual = desde.min(c.cuantos - 1);
            c.leyendo = true;
            c.actual
        };
        self.visto = Some(a);
        self.mandar(Que::Leer(a));
    }

    /// Play/pausa.
    pub fn alternar(&mut self) {
        {
            let mut c = comun();
            if c.vuelta != self.vuelta {
                return;
            }
            c.leyendo = !c.leyendo;
        }
        self.mandar(Que::Alternar);
    }

    /// Un parrafo adelante (+1) o atras (−1), desde su principio. Devuelve
    /// el parrafo en el que queda.
    pub fn saltar(&mut self, cuantos: isize) -> usize {
        let (a, leyendo) = {
            let mut c = comun();
            if c.vuelta != self.vuelta || c.cuantos == 0 {
                return 0;
            }
            let a = (c.actual as isize + cuantos).clamp(0, c.cuantos as isize - 1) as usize;
            c.actual = a;
            (a, c.leyendo)
        };
        self.visto = Some(a);
        self.mandar(if leyendo {
            Que::Leer(a)
        } else {
            Que::Situar(a)
        });
        a
    }

    /// La velocidad siguiente; sonando, se sigue con ella.
    pub fn otra_velocidad(&mut self) -> f32 {
        let v = voz_alta::siguiente_velocidad(self.velocidad());
        {
            let mut c = comun();
            if c.vuelta == self.vuelta {
                c.velocidad = v;
            }
        }
        self.mandar(Que::Velocidad(v));
        v
    }

    /// **Deja de escuchar**: calla y olvida el documento (la X de la barra).
    pub fn soltar(mut self) {
        self.mandar(Que::Soltar);
        self.fuera = true;
    }

    /// **Lo que paso desde la ultima vuelta** del lector.
    pub fn vuelta(&mut self) -> Option<Suceso> {
        if self.fuera {
            return None;
        }
        let c = comun();
        if c.vuelta != self.vuelta || !c.vivo {
            self.fuera = true;
            return Some(Suceso::Fuera);
        }
        if c.acabados != self.acabados {
            self.acabados = c.acabados;
            self.visto = None;
            return Some(Suceso::Acabado);
        }
        if c.cuantos > 0 && self.visto != Some(c.actual) && (c.leyendo || self.visto.is_some()) {
            self.visto = Some(c.actual);
            return Some(Suceso::Parrafo(c.actual));
        }
        if c.leyendo != self.visto_leyendo || c.voz != self.visto_voz {
            self.visto_leyendo = c.leyendo;
            self.visto_voz = c.voz.clone();
            return Some(Suceso::Cambio);
        }
        None
    }
}

impl Drop for LeerEnVoz {
    /// Cerrar el lector: la voz sigue, y desde ahora el verde lo apunta ella.
    fn drop(&mut self) {
        let mut c = comun();
        if c.vuelta == self.vuelta && c.enganchados > 0 {
            c.enganchados -= 1;
        }
    }
}

// ---------------------------------------------------------------- el hilo

/// El MP3 de un parrafo de Microsoft: pedido, o ya aqui (`None` si fallo).
enum Audio {
    Pedido(Receiver<Option<PathBuf>>),
    Listo(Option<PathBuf>),
}

struct Hilo {
    comun: Arc<Mutex<Comun>>,
    tx: Sender<Orden>,
    vuelta: u64,
    doc: Option<Box<Documento>>,
    sapi: Option<Lector>,
    etiqueta_windows: String,
    trozos: Vec<String>,
    trozo: usize,
    en_pausa: bool,
    hablado_en: u64,
    /// La voz de Microsoft que lee, o `None`: la de Windows.
    edge: Option<String>,
    /// Se espera la lista de voces para empezar en este parrafo.
    pendiente: Option<usize>,
    esperando_voces: bool,
    audios: HashMap<usize, Audio>,
    salida: Option<pixpin_audio::Salida>,
    /// Sonando con Microsoft, aun sin el MP3 del parrafo.
    esperando_audio: bool,
    mandos: Option<MandosDelSistema>,
    ensenado: Option<(bool, String, String)>,
}

impl Hilo {
    fn nuevo(comun: Arc<Mutex<Comun>>, tx: Sender<Orden>) -> Hilo {
        Hilo {
            comun,
            tx,
            vuelta: 0,
            doc: None,
            sapi: None,
            etiqueta_windows: String::new(),
            trozos: Vec::new(),
            trozo: 0,
            en_pausa: false,
            hablado_en: 0,
            edge: None,
            pendiente: None,
            esperando_voces: false,
            audios: HashMap::new(),
            salida: None,
            esperando_audio: false,
            mandos: None,
            ensenado: None,
        }
    }

    fn c(&self) -> MutexGuard<'_, Comun> {
        self.comun.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn correr(mut self, rx: Receiver<Orden>) {
        let _com = pixpin_shell::ComDelHilo::iniciar();
        loop {
            // Sin documento se espera sin gastar nada; con el, una vuelta
            // cada 40 ms (el «acabado» de la voz y los mandos de Windows).
            let orden = if self.doc.is_none() {
                rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
            } else {
                rx.recv_timeout(std::time::Duration::from_millis(40))
            };
            match orden {
                Ok(Orden::Apagar) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(o) => self.atender(o),
                Err(RecvTimeoutError::Timeout) => {}
            }
            let mut apagar = false;
            while let Ok(o) = rx.try_recv() {
                if matches!(o, Orden::Apagar) {
                    apagar = true;
                    break;
                }
                self.atender(o);
            }
            if apagar {
                break;
            }
            let botones = self.mandos.as_ref().map(|m| {
                m.bombear();
                m.botones()
            });
            for b in botones.unwrap_or_default() {
                self.boton_del_sistema(b);
            }
            self.dar_la_vuelta();
            self.ensenar();
        }
        self.soltar();
    }

    fn atender(&mut self, o: Orden) {
        match o {
            Orden::Documento(doc, vuelta) => self.documento(doc, vuelta),
            Orden::De(v, q) if v == self.vuelta && self.doc.is_some() => self.hacer(q),
            Orden::Voces(v, voces, preferida) if v == self.vuelta => {
                self.con_las_voces(&voces, preferida)
            }
            Orden::De(..) | Orden::Voces(..) | Orden::Apagar => {}
        }
    }

    fn documento(&mut self, doc: Box<Documento>, vuelta: u64) {
        self.callar_todo();
        self.audios.clear();
        self.vuelta = vuelta;
        self.sapi = Lector::nuevo(&doc.idioma).or_else(|| Lector::nuevo(&doc.reserva));
        self.etiqueta_windows = match &self.sapi {
            Some(l) => {
                l.poner_tasa(voz_alta::tasa_de_windows(doc.velocidad));
                etiqueta_de_windows(l.nombre())
            }
            None => String::new(),
        };
        self.edge = None;
        self.pendiente = None;
        self.esperando_voces = false;
        self.en_pausa = false;
        if doc.microsoft {
            self.buscar_microsoft(&doc.raiz, doc.preferida.clone());
            self.c().voz = doc.rotulos.preparando.clone();
        } else {
            let e = self.etiqueta_windows.clone();
            self.c().voz = e;
        }
        // Las pruebas no ponen los mandos en el escritorio de quien las pasa.
        if self.mandos.is_none() && !cfg!(test) {
            self.mandos = MandosDelSistema::nuevos();
        }
        self.ensenado = None;
        self.doc = Some(doc);
    }

    /// Pide la lista de voces de Microsoft en otro hilo (la red puede tardar
    /// diez segundos; mientras, los mandos responden).
    fn buscar_microsoft(&mut self, raiz: &Path, preferida: Option<String>) {
        self.esperando_voces = true;
        let carpeta = carpeta_de_edge(raiz);
        let tx = self.tx.clone();
        let vuelta = self.vuelta;
        let _ = std::thread::Builder::new()
            .name("voces-de-microsoft".into())
            .spawn(move || {
                let _ = tx.send(Orden::Voces(vuelta, edge::voces(&carpeta), preferida));
            });
    }

    /// **La voz de Microsoft del idioma** (`ponerLaVozDeMicrosoft`): la
    /// elegida o la del pais; si no hay, la de Windows. Lo que esperaba a
    /// saberlo empieza ya, y lo que sonaba vuelve a empezar su parrafo.
    fn con_las_voces(&mut self, voces: &[Voz], preferida: Option<String>) {
        self.esperando_voces = false;
        let Some((idioma, no_respondio)) = self
            .doc
            .as_ref()
            .map(|d| (d.idioma.clone(), d.rotulos.no_respondio.clone()))
        else {
            return;
        };
        let pendiente = self.pendiente.take();
        let (sonaba, actual) = {
            let c = self.c();
            (c.leyendo, c.actual)
        };
        self.callar_todo();
        self.en_pausa = false;
        self.audios.clear();
        let etiqueta = match edge::elegir(voces, &idioma, preferida.as_deref()) {
            Some(v) => {
                self.edge = Some(v.nombre.clone());
                etiqueta_de_microsoft(&v)
            }
            None => {
                self.edge = None;
                format!("{} · {no_respondio}", self.etiqueta_o_sin_voz())
            }
        };
        self.c().voz = etiqueta;
        if let Some(p) = pendiente {
            self.leer(p);
        } else if sonaba {
            self.leer(actual);
        }
    }

    fn etiqueta_o_sin_voz(&self) -> String {
        if self.etiqueta_windows.is_empty() {
            self.doc
                .as_ref()
                .map(|d| d.rotulos.sin_voz.clone())
                .unwrap_or_default()
        } else {
            self.etiqueta_windows.clone()
        }
    }

    fn hacer(&mut self, q: Que) {
        match q {
            Que::Leer(p) => self.leer(p),
            Que::Situar(p) => {
                self.callar_todo();
                self.en_pausa = false;
                self.pendiente = None;
                self.c().actual = p;
                self.apuntar_si_no_hay_lector(Some(p));
            }
            Que::Alternar => self.alternar(),
            Que::Velocidad(v) => self.velocidad(v),
            Que::Microsoft(m) => self.usar_microsoft(m),
            Que::BuscarMicrosoft(preferida) => {
                if let Some(raiz) = self.doc.as_ref().map(|d| d.raiz.clone()) {
                    self.buscar_microsoft(&raiz, preferida);
                }
            }
            Que::Soltar => self.soltar(),
        }
    }

    /// Calla lo que suene, sin cambiar el estado.
    fn callar_todo(&mut self) {
        if let Some(l) = &self.sapi {
            l.callar();
        }
        self.salida = None;
        self.esperando_audio = false;
    }

    /// **Empieza a leer en el parrafo `p`**, por su principio.
    fn leer(&mut self, p: usize) {
        let Some(texto) = self.doc.as_ref().and_then(|d| {
            d.parrafos
                .get(p.min(d.parrafos.len().saturating_sub(1)))
                .cloned()
        }) else {
            return;
        };
        let p = p.min(self.doc.as_ref().map_or(1, |d| d.parrafos.len()) - 1);
        self.callar_todo();
        self.en_pausa = false;
        {
            let mut c = self.c();
            c.actual = p;
            c.leyendo = true;
        }
        if self.esperando_voces {
            // Aun no se sabe con que voz de Microsoft: se empieza al saberlo.
            self.pendiente = Some(p);
            return;
        }
        if self.edge.is_some() {
            self.pedir_audios(p);
            self.esperando_audio = true;
        } else if self.sapi.is_some() {
            self.trozos = voz_alta::trozos(&texto, voz_alta::TOPE);
            self.trozo = 0;
            self.decir();
        } else {
            self.c().leyendo = false;
        }
        self.apuntar_si_no_hay_lector(Some(p));
    }

    fn decir(&mut self) {
        if let Some(l) = &self.sapi {
            match self.trozos.get(self.trozo) {
                Some(t) => l.leer(t),
                // Un parrafo sin nada que decir: se pasa en la siguiente vuelta.
                None => l.callar(),
            }
        }
        self.hablado_en = ahora_ms();
    }

    /// **El verde se mueve al parrafo que suena** (`apuntarElVerde`), si el
    /// documento no tiene un lector abierto que ya lo haga.
    fn apuntar_si_no_hay_lector(&self, p: Option<usize>) {
        if self.c().enganchados > 0 {
            return;
        }
        if let Some(doc) = &self.doc {
            (doc.apuntar)(p);
        }
    }

    fn alternar(&mut self) {
        // El lector ya puso `leyendo` a lo que quiere.
        if self.c().leyendo {
            self.seguir();
        } else {
            self.pausar();
        }
    }

    fn pausar(&mut self) {
        self.c().leyendo = false;
        self.pendiente = None;
        if let Some(s) = &self.salida {
            s.pausar();
            return;
        }
        if self.esperando_audio {
            self.esperando_audio = false;
            return;
        }
        if let Some(l) = &self.sapi {
            l.pausar();
            self.en_pausa = true;
        }
    }

    fn seguir(&mut self) {
        let (actual, v) = {
            let mut c = self.c();
            c.leyendo = true;
            (c.actual, c.velocidad)
        };
        if let Some(r) = self.salida.as_ref().map(|s| s.tocar(v)) {
            if r.is_err() {
                self.caer(actual);
            }
            return;
        }
        if self.en_pausa
            && let Some(l) = &self.sapi
        {
            l.reanudar();
            self.en_pausa = false;
            return;
        }
        self.leer(actual);
    }

    fn velocidad(&mut self, v: f32) {
        self.c().velocidad = v;
        // Con Microsoft, el MP3 se acelera ahi mismo, sin pedirlo otra vez.
        if let Some(s) = &self.salida {
            s.poner_velocidad(v);
            return;
        }
        if let Some(l) = &self.sapi {
            l.poner_tasa(voz_alta::tasa_de_windows(v));
        }
        if self.c().leyendo && self.edge.is_none() {
            self.decir();
        } else if self.en_pausa {
            // Lo pausado seguiria a la velocidad de antes: se empieza el trozo.
            if let Some(l) = &self.sapi {
                l.callar();
            }
            self.en_pausa = false;
        }
    }

    /// **Leer con esta voz de Microsoft** (o con la de Windows): si sonaba,
    /// se vuelve a empezar el parrafo con la nueva (`usarEdge`).
    fn usar_microsoft(&mut self, m: Option<(String, String)>) {
        let nombre = m.as_ref().map(|(n, _)| n.clone());
        if nombre == self.edge && !self.esperando_voces {
            return;
        }
        self.esperando_voces = false;
        let pendiente = self.pendiente.take();
        let (sonaba, actual) = {
            let c = self.c();
            (c.leyendo, c.actual)
        };
        self.callar_todo();
        self.en_pausa = false;
        self.audios.clear();
        self.edge = nombre;
        let etiqueta = match m {
            Some((_, etiqueta)) => etiqueta,
            None => self.etiqueta_o_sin_voz(),
        };
        self.c().voz = etiqueta;
        if let Some(p) = pendiente {
            self.leer(p);
        } else if sonaba {
            self.leer(actual);
        }
    }

    /// Pide los MP3 de `p` y de los [`POR_DELANTE`] siguientes.
    fn pedir_audios(&mut self, p: usize) {
        let (Some(doc), Some(voz)) = (&self.doc, &self.edge) else {
            return;
        };
        let carpeta = carpeta_de_edge(&doc.raiz);
        for q in p..(p + 1 + POR_DELANTE).min(doc.parrafos.len()) {
            if self.audios.contains_key(&q) {
                continue;
            }
            let (tx, rx) = mpsc::channel();
            let texto = doc.parrafos[q].clone();
            let voz = voz.clone();
            let carpeta = carpeta.clone();
            let lanzado = std::thread::Builder::new()
                .name("voz-de-microsoft".into())
                .spawn(move || {
                    let r =
                        edge::archivo(&carpeta, &texto, &voz, |t| voz_alta::trozos(t, edge::TOPE));
                    if let Err(e) = &r {
                        tracing::info!(%e, "Microsoft no dio la voz de un parrafo");
                    }
                    let _ = tx.send(r.ok());
                });
            self.audios.insert(
                q,
                if lanzado.is_ok() {
                    Audio::Pedido(rx)
                } else {
                    Audio::Listo(None)
                },
            );
        }
    }

    /// El MP3 del parrafo `p` si ya llego: `Some(Some(ruta))`, `Some(None)`
    /// si fallo, `None` si aun no.
    fn audio_de(&mut self, p: usize) -> Option<Option<PathBuf>> {
        let a = self.audios.get_mut(&p)?;
        if let Audio::Pedido(rx) = a {
            match rx.try_recv() {
                Ok(r) => *a = Audio::Listo(r),
                Err(mpsc::TryRecvError::Empty) => return None,
                Err(mpsc::TryRecvError::Disconnected) => *a = Audio::Listo(None),
            }
        }
        match a {
            Audio::Listo(r) => Some(r.clone()),
            Audio::Pedido(_) => None,
        }
    }

    /// **Microsoft no respondio**: se sigue con la voz de Windows, desde ese
    /// mismo parrafo (`caerAGoogle`).
    fn caer(&mut self, p: usize) {
        self.callar_todo();
        self.audios.clear();
        self.edge = None;
        let aviso = self
            .doc
            .as_ref()
            .map(|d| d.rotulos.no_respondio.clone())
            .unwrap_or_default();
        let e = format!("{} · {aviso}", self.etiqueta_o_sin_voz());
        self.c().voz = e;
        if self.sapi.is_some() {
            self.leer(p);
        } else {
            self.c().leyendo = false;
        }
    }

    /// **Lo que paso desde la ultima vuelta**: el MP3 que llega, la voz que
    /// acaba su trozo, el parrafo siguiente o el final.
    fn dar_la_vuelta(&mut self) {
        let (leyendo, actual, v) = {
            let c = self.c();
            (c.leyendo, c.actual, c.velocidad)
        };
        if !leyendo || self.doc.is_none() || self.pendiente.is_some() {
            return;
        }
        if self.edge.is_some() {
            if self.esperando_audio {
                match self.audio_de(actual) {
                    None => {}
                    Some(Some(ruta)) => {
                        self.esperando_audio = false;
                        match pixpin_audio::Salida::abrir(&ruta) {
                            Ok(s) if s.tocar(v).is_ok() => self.salida = Some(s),
                            _ => self.caer(actual),
                        }
                    }
                    Some(None) => self.caer(actual),
                }
                return;
            }
            let Some((fallo, termino)) = self.salida.as_ref().map(|s| (s.fallo(), s.termino()))
            else {
                return;
            };
            if fallo {
                self.caer(actual);
            } else if termino {
                self.siguiente();
            }
            return;
        }
        let acabado = match &self.sapi {
            Some(l) => ahora_ms() >= self.hablado_en + MS_PARA_EMPEZAR && l.acabado(),
            None => false,
        };
        if !acabado {
            return;
        }
        if self.trozo + 1 < self.trozos.len() {
            self.trozo += 1;
            self.decir();
            return;
        }
        self.siguiente();
    }

    fn siguiente(&mut self) {
        let n = self.doc.as_ref().map_or(0, |d| d.parrafos.len());
        let actual = self.c().actual;
        if actual + 1 < n {
            self.leer(actual + 1);
        } else {
            self.acabar();
        }
    }

    /// **Se acabo el documento**: el verde se quita, y la proxima vez se
    /// empieza por donde se mire (`alAcabarElDocumento`).
    fn acabar(&mut self) {
        self.callar_todo();
        {
            let mut c = self.c();
            c.leyendo = false;
            c.actual = 0;
            c.acabados += 1;
        }
        self.apuntar_si_no_hay_lector(None);
    }

    /// Lo pulsado en los mandos de Windows o en las teclas multimedia (los
    /// de la notificacion del movil).
    fn boton_del_sistema(&mut self, b: BotonDelSistema) {
        let (leyendo, actual, n) = {
            let c = self.c();
            (c.leyendo, c.actual, c.cuantos)
        };
        match b {
            BotonDelSistema::Tocar if !leyendo => self.seguir(),
            BotonDelSistema::Pausar if leyendo => self.pausar(),
            BotonDelSistema::Siguiente | BotonDelSistema::Anterior if n > 0 => {
                let paso: isize = if b == BotonDelSistema::Siguiente {
                    1
                } else {
                    -1
                };
                let a = (actual as isize + paso).clamp(0, n as isize - 1) as usize;
                if leyendo {
                    self.leer(a);
                } else {
                    self.hacer(Que::Situar(a));
                }
            }
            BotonDelSistema::Parar => self.soltar(),
            _ => {}
        }
    }

    /// Los mandos de Windows, al dia.
    fn ensenar(&mut self) {
        if self.mandos.is_none() {
            return;
        }
        let ahora = {
            let c = self.c();
            let donde = self
                .doc
                .as_ref()
                .map(|d| d.rotulos.donde(c.actual, c.cuantos))
                .unwrap_or_default();
            let detalle = if c.voz.is_empty() {
                donde
            } else {
                format!("{donde} · {}", c.voz)
            };
            (c.leyendo, c.titulo.clone(), detalle)
        };
        if self.ensenado.as_ref() != Some(&ahora) {
            if let Some(m) = &self.mandos {
                m.poner(ahora.0, &ahora.1, &ahora.2);
            }
            self.ensenado = Some(ahora);
        }
    }

    /// **Suelta la voz y olvida el documento** (la X de la barra, parar en
    /// los mandos de Windows, salir de la app).
    fn soltar(&mut self) {
        self.callar_todo();
        self.audios.clear();
        self.doc = None;
        self.sapi = None;
        self.edge = None;
        self.pendiente = None;
        self.esperando_voces = false;
        self.mandos = None;
        self.ensenado = None;
        let mut c = self.c();
        c.vivo = false;
        c.leyendo = false;
    }
}

fn etiqueta_de_windows(nombre: &str) -> String {
    if nombre.is_empty() {
        "Windows".into()
    } else {
        format!("{nombre} · Windows")
    }
}

/// «Elvira · Microsoft · en línea», como el movil.
pub fn etiqueta_de_microsoft(v: &Voz) -> String {
    format!("{} · Microsoft · en línea", v.corto())
}

/// La carpeta de lo de Microsoft: los MP3 leidos y la lista de voces.
pub fn carpeta_de_edge(raiz: &Path) -> PathBuf {
    raiz.join("cache").join("voz-edge")
}

// ----------------------------------------------- lo que se recuerda de la voz

/// **Lo que se recuerda en este equipo** de la voz (las preferencias
/// `lectura` del movil: `voz:edge` y `voz:edge:<lengua>`): si se lee con
/// Microsoft y que voz se eligio para cada lengua. En `voz.json` de la
/// carpeta de la app.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Preferencias {
    pub microsoft: bool,
    pub voces: std::collections::BTreeMap<String, String>,
}

fn fichero_de_preferencias(raiz: &Path) -> PathBuf {
    raiz.join("voz.json")
}

impl Preferencias {
    /// Lo guardado; lo de fabrica si no hay o no se entiende.
    pub fn leer(raiz: &Path) -> Preferencias {
        let Ok(t) = std::fs::read_to_string(fichero_de_preferencias(raiz)) else {
            return Preferencias::default();
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) else {
            return Preferencias::default();
        };
        Preferencias {
            microsoft: v
                .get("microsoft")
                .and_then(|x| x.as_bool())
                .unwrap_or(false),
            voces: v
                .get("voces")
                .and_then(|x| x.as_object())
                .map(|o| {
                    o.iter()
                        .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    pub fn guardar(&self, raiz: &Path) {
        let v = serde_json::json!({ "microsoft": self.microsoft, "voces": self.voces });
        if let Err(e) = std::fs::write(fichero_de_preferencias(raiz), v.to_string()) {
            tracing::warn!(?e, "no se pudo guardar lo de la voz");
        }
    }

    /// La voz de Microsoft elegida para la lengua de `idioma`.
    pub fn voz_de(&self, idioma: &str) -> Option<String> {
        self.voces.get(&voz_alta::partes(idioma).0).cloned()
    }

    pub fn poner_voz(&mut self, idioma: &str, nombre: &str) {
        self.voces
            .insert(voz_alta::partes(idioma).0, nombre.to_string());
    }
}

// ------------------------------------------------- el engranaje de los lectores

/// Lo de la voz que hay en el engranaje de los lectores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AjusteVoz {
    /// Encender o apagar las voces de Microsoft.
    Microsoft,
    /// Elegir la voz de Microsoft (sale un menu).
    ElegirVoz,
}

/// **Las filas de la voz en el engranaje** («Voces de Microsoft» y, si estan
/// puestas, «Voz de Microsoft»), como el apartado «Voz» del movil. El
/// rotulo dice que el texto va a Microsoft.
pub fn filas(textos: &pixpin_store::Catalogo, raiz: &Path) -> Vec<(String, AjusteVoz)> {
    let p = Preferencias::leer(raiz);
    let mut v = vec![(
        textos.t(if p.microsoft {
            "lector-voces-microsoft-si"
        } else {
            "lector-voces-microsoft-no"
        }),
        AjusteVoz::Microsoft,
    )];
    if p.microsoft {
        let idioma = idioma_que_suena().unwrap_or_else(|| idioma_de_la_app(textos));
        let puesta = p
            .voz_de(&idioma)
            .map(|n| corto_del_nombre(&n))
            .unwrap_or_else(|| textos.t("lector-voz-del-pais"));
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("voz", puesta);
        v.push((
            textos.t_args("lector-voz-de-microsoft", &args),
            AjusteVoz::ElegirVoz,
        ));
    }
    v
}

/// «es-ES-ElviraNeural» → «Elvira» (lo del engranaje del movil).
pub fn corto_del_nombre(n: &str) -> String {
    let resto = n.splitn(3, '-').nth(2).unwrap_or(n);
    let r = resto.strip_suffix("Neural").unwrap_or(resto);
    r.replace("Multilingual", " multilingüe")
}

/// **Encender o apagar las voces de Microsoft**; sonando, vale ya.
pub fn alternar_microsoft(raiz: &Path) {
    let mut p = Preferencias::leer(raiz);
    p.microsoft = !p.microsoft;
    p.guardar(raiz);
    if LADO.get().is_none() {
        return;
    }
    let (vivo, vuelta, idioma) = {
        let c = comun();
        (c.vivo, c.vuelta, c.idioma.clone())
    };
    if !vivo {
        return;
    }
    mandar(Orden::De(
        vuelta,
        if p.microsoft {
            Que::BuscarMicrosoft(p.voz_de(&idioma))
        } else {
            Que::Microsoft(None)
        },
    ));
}

/// **Elegir la voz de Microsoft** (`ElegirVozDeMicrosoft`): un menu con las
/// mejores del idioma que se lee —las del pais primero— y las de IA, que leen
/// cualquiera. Se recuerda por lengua y, sonando, se cambia ya. La lista sale
/// de la copia guardada; si aun no hay, se pide y se avisa (`Some(aviso)`).
pub fn elegir_voz(
    hwnd: windows::Win32::Foundation::HWND,
    textos: &pixpin_store::Catalogo,
    raiz: &Path,
) -> Option<String> {
    let carpeta = carpeta_de_edge(raiz);
    let todas = edge::voces_guardadas(&carpeta);
    if todas.is_empty() {
        let _ = std::thread::Builder::new()
            .name("voces-de-microsoft".into())
            .spawn(move || {
                let _ = edge::voces(&carpeta);
            });
        return Some(textos.t("lector-voz-pidiendo"));
    }
    let idioma = idioma_que_suena().unwrap_or_else(|| idioma_de_la_app(textos));
    let lista = edge::las_mejores(&todas, &idioma);
    if lista.is_empty() {
        return Some(textos.t("lector-voz-sin-voces"));
    }
    let mut p = Preferencias::leer(raiz);
    let puesta = p.voz_de(&idioma);
    let entradas: Vec<(u32, String)> = lista
        .iter()
        .enumerate()
        .map(|(i, v)| (i as u32 + 1, rotulo_del_menu(textos, v, puesta.as_deref())))
        .collect();
    let elegida = pixpin_shell::menu_llano(hwnd, &entradas)?;
    let v = lista.get((elegida as usize).checked_sub(1)?)?;
    p.poner_voz(&idioma, &v.nombre);
    p.guardar(raiz);
    if LADO.get().is_some() && p.microsoft {
        let (vivo, vuelta) = {
            let c = comun();
            (c.vivo, c.vuelta)
        };
        if vivo {
            mandar(Orden::De(
                vuelta,
                Que::Microsoft(Some((v.nombre.clone(), etiqueta_de_microsoft(v)))),
            ));
        }
    }
    None
}

/// «✓ Elvira · es-ES · mujer», «Ava multilingüe · de IA, lee cualquier
/// idioma · mujer».
fn rotulo_del_menu(textos: &pixpin_store::Catalogo, v: &Voz, puesta: Option<&str>) -> String {
    let marca = if Some(v.nombre.as_str()) == puesta {
        "✓ "
    } else {
        ""
    };
    let genero = match v.genero.as_str() {
        "Female" => textos.t("lector-voz-mujer"),
        "Male" => textos.t("lector-voz-hombre"),
        g => g.to_string(),
    };
    let de_donde = if edge::es_multilingue(v) {
        textos.t("lector-voz-de-ia")
    } else {
        v.idioma.clone()
    };
    format!("{marca}{} · {de_donde} · {genero}", v.corto())
}

/// **El documento para la voz**, con lo comun ya puesto: velocidad 1, las
/// voces de Microsoft si estan encendidas (y la elegida para el idioma) y
/// los rotulos traducidos.
pub fn documento(
    textos: &pixpin_store::Catalogo,
    raiz: &Path,
    clave: &Path,
    titulo: String,
    parrafos: Vec<String>,
    idioma: String,
    apuntar: Apuntar,
) -> Documento {
    let p = Preferencias::leer(raiz);
    Documento {
        clave: clave.to_path_buf(),
        titulo,
        parrafos,
        preferida: p.voz_de(&idioma),
        reserva: idioma_de_la_app(textos),
        idioma,
        velocidad: 1.0,
        microsoft: p.microsoft,
        raiz: raiz.to_path_buf(),
        rotulos: Rotulos::de(textos),
        apuntar,
    }
}

// ------------------------------------------------------------- la barra

/// **La barra de escuchar**, abajo y de cristal como la del movil: atras un
/// parrafo, play/pausa, adelante uno, la velocidad (rota al pulsarla) y
/// cerrar. Debajo, que voz lee y por donde va. Devuelve donde cae cada boton.
pub fn pintar_barra(
    p: &Pintor,
    ancho: f32,
    alto: f32,
    e: f32,
    voz: &LeerEnVoz,
) -> Vec<(RectF, BotonVoz)> {
    let mut botones = Vec::new();
    let lado = 40.0 * e;
    let rotulo = voz_alta::rotulo(voz.velocidad());
    let tam = 14.0 * e;
    let (w_vel, _) = p.medir_texto(&rotulo, tam);
    let ancho_vel = w_vel + 20.0 * e;
    let fila = 4.0 * lado + ancho_vel;
    let debajo = format!(
        "{} · {} / {}",
        voz.nombre(),
        voz.actual() + 1,
        voz.cuantos().max(1)
    );
    let tam_debajo = 11.0 * e;
    let (w_debajo, h_debajo) = p.medir_texto(&debajo, tam_debajo);
    let total = fila.max(w_debajo + 24.0 * e) + 12.0 * e;
    let caja = RectF {
        x: (ancho - total) / 2.0,
        y: alto - 24.0 * e - lado - h_debajo - 10.0 * e,
        ancho: total,
        alto: lado + h_debajo + 10.0 * e,
    };
    p.rellenar_redondeado(caja, 20.0 * e, con_alfa(CRISTAL, 0.95));
    let mut x = caja.x + (total - fila) / 2.0;
    let y = caja.y;
    let icono = |p: &Pintor, i: &pixpin_render::icono::Icono, r: RectF, l: f32| {
        p.icono(
            i,
            RectF {
                x: r.x + (r.ancho - l) / 2.0,
                y: r.y + (r.alto - l) / 2.0,
                ancho: l,
                alto: l,
            },
            TEXTO,
        );
    };
    let texto = |p: &Pintor, t: &str, r: RectF, tam: f32| {
        let (w, h) = p.medir_texto(t, tam);
        p.texto(
            t,
            r.x + (r.ancho - w) / 2.0,
            r.y + (r.alto - h) / 2.0,
            tam,
            TEXTO,
        );
    };
    let r = RectF {
        x,
        y,
        ancho: lado,
        alto: lado,
    };
    texto(p, "⏮", r, 16.0 * e);
    botones.push((r, BotonVoz::Anterior));
    x += lado;
    let r = RectF {
        x,
        y,
        ancho: lado,
        alto: lado,
    };
    icono(
        p,
        if voz.leyendo() {
            &material::PAUSE
        } else {
            &material::PLAY_ARROW
        },
        r,
        28.0 * e,
    );
    botones.push((r, BotonVoz::Alternar));
    x += lado;
    let r = RectF {
        x,
        y,
        ancho: lado,
        alto: lado,
    };
    texto(p, "⏭", r, 16.0 * e);
    botones.push((r, BotonVoz::Siguiente));
    x += lado;
    let r = RectF {
        x,
        y,
        ancho: ancho_vel,
        alto: lado,
    };
    texto(p, &rotulo, r, tam);
    botones.push((r, BotonVoz::Velocidad));
    x += ancho_vel;
    let r = RectF {
        x,
        y,
        ancho: lado,
        alto: lado,
    };
    icono(p, &material::CLOSE, r, 20.0 * e);
    botones.push((r, BotonVoz::Cerrar));
    p.texto(
        &debajo,
        caja.x + (total - w_debajo) / 2.0,
        y + lado,
        tam_debajo,
        APAGADO,
    );
    botones
}

/// Lo que se le dice al usuario cuando Windows no tiene voz de un idioma.
pub fn sin_voz(textos: &pixpin_store::Catalogo, idioma: &str) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("idioma", voz_alta::partes(idioma).0);
    textos.t_args("lector-sin-voz", &args)
}

/// El idioma de reserva: el de la aplicacion (`es`/`en`).
pub fn idioma_de_la_app(textos: &pixpin_store::Catalogo) -> String {
    textos.t("lector-voz-idioma")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn raiz(nombre: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!("pixpin-voz-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    fn esperar(mut hasta: impl FnMut() -> bool) {
        for _ in 0..100 {
            if hasta() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    #[test]
    fn lo_de_la_voz_se_recuerda_por_lengua_y_un_fichero_roto_es_lo_de_fabrica() {
        let r = raiz("prefs");
        assert_eq!(Preferencias::leer(&r), Preferencias::default());
        let mut p = Preferencias::leer(&r);
        p.microsoft = true;
        p.poner_voz("es-PE", "es-ES-ElviraNeural");
        p.guardar(&r);
        let leido = Preferencias::leer(&r);
        assert!(leido.microsoft);
        // Por lengua: vale para el castellano de cualquier pais.
        assert_eq!(leido.voz_de("es-MX").as_deref(), Some("es-ES-ElviraNeural"));
        assert_eq!(leido.voz_de("en-US"), None);
        std::fs::write(r.join("voz.json"), "{no es json").unwrap();
        assert_eq!(Preferencias::leer(&r), Preferencias::default());
        std::fs::write(r.join("voz.json"), r#"{"microsoft":"si","voces":{"es":3}}"#).unwrap();
        assert_eq!(Preferencias::leer(&r), Preferencias::default());
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn el_nombre_corto_y_las_etiquetas_son_las_del_movil() {
        assert_eq!(corto_del_nombre("es-ES-ElviraNeural"), "Elvira");
        assert_eq!(
            corto_del_nombre("en-US-AvaMultilingualNeural"),
            "Ava multilingüe"
        );
        assert_eq!(corto_del_nombre("raro"), "raro");
        let v = Voz {
            nombre: "es-ES-ElviraNeural".into(),
            idioma: "es-ES".into(),
            genero: "Female".into(),
        };
        assert_eq!(etiqueta_de_microsoft(&v), "Elvira · Microsoft · en línea");
        assert_eq!(etiqueta_de_windows(""), "Windows");
        assert_eq!(
            etiqueta_de_windows("Microsoft Helena Desktop"),
            "Microsoft Helena Desktop · Windows"
        );
        let r = Rotulos {
            parrafo: "Párrafo {n} de {total}".into(),
            ..Rotulos::default()
        };
        assert_eq!(r.donde(2, 40), "Párrafo 3 de 40");
    }

    /// Un documento que no suena: Microsoft «encendido» con una lista de
    /// voces vacia y fresca (no se pide nada a la red) y una voz de Windows
    /// que no existe. Solo se cuentan los parrafos.
    fn mudo(r: &Path, nombre: &str, parrafos: usize, apuntar: Apuntar) -> Documento {
        let carpeta = carpeta_de_edge(r);
        std::fs::create_dir_all(&carpeta).unwrap();
        std::fs::write(carpeta.join("voces-edge.json"), "[]").unwrap();
        Documento {
            clave: r.join(nombre),
            titulo: nombre.into(),
            parrafos: (0..parrafos).map(|i| format!("Parrafo {i}.")).collect(),
            idioma: "xx-XX".into(),
            reserva: "xx-XX".into(),
            velocidad: 1.0,
            microsoft: true,
            preferida: None,
            raiz: r.to_path_buf(),
            rotulos: Rotulos {
                no_respondio: "Microsoft no respondió".into(),
                sin_voz: "Sin voz".into(),
                ..Rotulos::default()
            },
            apuntar,
        }
    }

    /// **La voz es de la app, no del lector**: cerrar el lector no la calla,
    /// el verde lo apunta ella sola, volver a abrir se engancha al parrafo
    /// que suena, y empezar otro documento deja fuera al de antes.
    #[test]
    fn cerrar_el_lector_no_calla_la_voz_y_volver_a_abrir_se_engancha() {
        let r = raiz("app");
        let apuntados = Arc::new(Mutex::new(Vec::<Option<usize>>::new()));
        let a = Arc::clone(&apuntados);
        let doc = mudo(
            &r,
            "libro.docx",
            3,
            Box::new(move |p| a.lock().unwrap().push(p)),
        );
        let clave = doc.clave.clone();
        let mut v = LeerEnVoz::arrancar(doc).expect("con Microsoft se puede empezar");
        assert!(suena(&clave));
        assert!(!suena(&r.join("otro.docx")));
        assert_eq!(v.cuantos(), 3);
        // Parado: saltar mueve el sitio.
        assert_eq!(v.saltar(1), 1);
        assert_eq!(v.actual(), 1);
        assert_eq!(v.saltar(5), 2, "no se pasa del ultimo");
        assert!(
            !matches!(v.vuelta(), Some(Suceso::Parrafo(_))),
            "lo movio el propio lector: no hay parrafo que contarle"
        );
        // Con el lector abierto, el verde lo apunta el lector, no la voz.
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert!(apuntados.lock().unwrap().is_empty());
        // Se cierra el lector: la voz sigue con el documento.
        drop(v);
        assert!(suena(&clave));
        assert!(
            LeerEnVoz::enganchar(&clave, 7).is_none(),
            "contado distinto: no es el mismo"
        );
        assert!(LeerEnVoz::enganchar(&r.join("otro.docx"), 3).is_none());
        let mut otra = LeerEnVoz::enganchar(&clave, 3).expect("el mismo documento se engancha");
        assert_eq!(otra.actual(), 2, "va al parrafo que suena");
        // Con el lector abierto de nuevo, un paso desde los mandos de Windows
        // se le cuenta.
        let vuelta = comun().vuelta;
        mandar(Orden::De(vuelta, Que::Situar(1)));
        esperar(|| otra.actual() == 1);
        assert_eq!(otra.vuelta(), Some(Suceso::Parrafo(1)));
        assert!(apuntados.lock().unwrap().is_empty());
        drop(otra);
        // Sin lector, el verde lo apunta la voz.
        mandar(Orden::De(vuelta, Que::Situar(0)));
        esperar(|| !apuntados.lock().unwrap().is_empty());
        assert_eq!(apuntados.lock().unwrap().as_slice(), &[Some(0)]);
        // Otro documento: el de antes queda fuera.
        let mut otra = LeerEnVoz::enganchar(&clave, 3).unwrap();
        let nueva = LeerEnVoz::arrancar(mudo(&r, "otro.docx", 1, Box::new(|_| {}))).unwrap();
        assert_eq!(otra.vuelta(), Some(Suceso::Fuera));
        assert_eq!(otra.vuelta(), None, "se dice una vez");
        assert!(!suena(&clave));
        // Sin voz de Microsoft para el idioma ni de Windows: lo dice.
        esperar(|| nueva.nombre() == "Sin voz · Microsoft no respondió");
        assert_eq!(nueva.nombre(), "Sin voz · Microsoft no respondió");
        nueva.soltar();
        esperar(|| !comun().vivo);
        assert!(!comun().vivo, "la X calla y olvida");
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn sin_microsoft_y_sin_voz_de_windows_no_se_empieza() {
        let _com = pixpin_shell::ComDelHilo::iniciar();
        let mut doc = mudo(&raiz("sin"), "x.docx", 1, Box::new(|_| {}));
        doc.microsoft = false;
        assert!(LeerEnVoz::arrancar(doc).is_none());
    }
}
