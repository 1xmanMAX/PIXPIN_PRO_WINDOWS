//! **Vosk en vivo** (1-nov-2026, lo que el movil ya hacia con Vosk y el PC
//! aun no: oir de corrido, con lo que se va oyendo mientras se habla).
//!
//! La escucha de la reunion (`en_vivo`) corta frases y las pasa a texto
//! enteras: con el reconocedor de Windows o con Whisper *base* (este, unos
//! 2-3 s por trozo de 5-12 s, porque su encoder oye siempre 30 s). Vosk
//! (Kaldi, Apache-2.0, `vosk-model-small-*` de unos 40-50 MB) es **un
//! reconocedor de corriente**: se le da el audio a trozos de 100 ms y
//! devuelve **lo que va oyendo** (el parcial, decimas despues de decirlo) y,
//! en cuanto se calla medio segundo, **la frase cerrada** con el tiempo de
//! cada palabra (su propio detector de final,
//! `endpoint.rule2.min-trailing-silence=0.5` del modelo pequeno). Quien
//! habla de corrido no se calla medio segundo: pasados [`LARGO_MS`], la
//! frase se cierra en la primera pausa de una coma ([`PAUSA_MS`]).
//!
//! Como `motor` (las notas), **se carga al vuelo**: `LoadLibraryExW` en
//! Windows (con `LOAD_WITH_ALTERED_SEARCH_PATH`, para que las tres DLL de
//! MinGW que trae `libvosk.dll` se busquen a su lado) y `dlopen` fuera (solo
//! para medirlo en Linux con `libvosk.so`, `examples/vosk_en_vivo.rs`). El
//! repositorio no gana ninguna dependencia de compilacion.
//!
//! Reparto:
//!
//! - [`Corriente`]: lo puro, probado sin Vosk. Junta los bloques de la
//!   grabacion (i16 intercalado, con su posicion, como el `Frasero`), los
//!   pasa a mono y 16 kHz, se los da al reconocedor de 100 en 100 ms, mira
//!   el parcial cada [`CADA_PARCIAL_MS`] y traduce lo que diga Vosk a
//!   [`Oido`]: un parcial nuevo o una frase con el milisegundo de la
//!   grabacion en que empieza y en que se dejo de hablar. Un hueco corto se
//!   rellena con silencio; uno largo cierra la frase y lo que sigue va en
//!   su minuto.
//! - [`Vosk`] y [`ModeloVosk`]: la biblioteca y el modelo cargados;
//!   [`ReconocedorVosk`] es el reconocedor de verdad de una [`Corriente`].
//!
//! Cada bloque `unsafe` lleva su `// SAFETY:`.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::Path;
use std::sync::Arc;

use crate::credibilidad::creible;
use crate::en_vivo::RMS_VOZ;
use crate::{ErrorVoz, HERCIOS};

/// Lo que se le da al reconocedor de una vez: 100 ms.
pub const TROZO_MS: i64 = 100;
/// Cada cuanto audio se le pide el parcial.
pub const CADA_PARCIAL_MS: i64 = 200;
/// Un hueco de hasta esto se rellena con silencio (como en `en_vivo`).
pub const HUECO_MAXIMO_MS: i64 = 1_000;
/// **Quien habla de corrido**: Vosk solo cierra la frase al callarse medio
/// segundo, o a los 20 s. Con una frase de mas de esto a medias, se cierra
/// en la primera pausa de [`PAUSA_MS`] (la de una coma); asi a Jev le
/// llegan frases de unos segundos, como con el `Frasero`, y no de veinte
/// (sin puntuacion, `vivo::partir` no las parte).
pub const LARGO_MS: i64 = 4_000;
/// Con una frase de mas de esto a medias, se cierra aunque no haya pausa.
pub const MAXIMO_MS: i64 = 10_000;
/// Lo callado que ya vale para cerrar una frase larga.
pub const PAUSA_MS: i64 = 200;

/// **Lo que se oye**, por orden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Oido {
    /// Lo que va oyendo de la frase de ahora (cambia en cada parcial; vacio:
    /// ya no hay nada a medias). Solo se ensena.
    Parcial(String),
    /// Una frase cerrada.
    Frase(FraseOida),
}

/// Una frase cerrada por Vosk (o por [`LARGO_MS`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FraseOida {
    /// Su numero, desde 0 y seguidos.
    pub n: u64,
    /// El milisegundo de la grabacion en que empieza su primera palabra.
    pub ms: i64,
    /// El milisegundo de la grabacion en que acaba su ultima palabra.
    pub fin_voz_ms: i64,
    /// Lo oido despues de [`FraseOida::fin_voz_ms`] cuando se cerro: restado
    /// al momento en que sale, da cuando se dejo de hablar.
    pub callado_al_final_ms: i64,
    pub texto: String,
}

/// **Un reconocedor de corriente**: lo que hace falta de Vosk (asi la
/// [`Corriente`] se prueba con uno de mentira). Los tiempos de las palabras
/// cuentan desde que se creo, **tambien despues de [`Reconocedor::acabar`]**
/// (como Vosk 0.3.45, medido el 1-nov-2026).
pub trait Reconocedor {
    /// Le da audio (mono, 16 kHz). `true`: cerro una frase (pedir
    /// [`Reconocedor::resultado`]).
    fn comer(&mut self, muestras: &[i16]) -> bool;
    /// La frase cerrada, en el JSON de Vosk.
    fn resultado(&mut self) -> String;
    /// Lo que va oyendo, en el JSON de Vosk (`{"partial": "…"}`).
    fn parcial(&mut self) -> String;
    /// Cierra lo que haya (y sigue oyendo).
    fn acabar(&mut self) -> String;
}

/// **De bloques de la grabacion a lo oido** (ver la cabecera).
#[derive(Debug)]
pub struct Corriente<R: Reconocedor> {
    rec: R,
    idioma: String,
    /// El muestreo de los bloques (0: aun nada).
    hz: u32,
    /// Donde deberia empezar el siguiente bloque (marcos).
    esperado: Option<u64>,
    /// Lo que hay que sumar a un tiempo de Vosk para tener el milisegundo de
    /// la grabacion (el principio y los huecos largos, que no se le dan).
    desfase_ms: i64,
    /// Lo dado a Vosk desde que se creo (muestras a 16 kHz).
    dado: u64,
    /// Lo que espera a completar un trozo.
    pendiente: Vec<i16>,
    /// Lo dado desde el ultimo parcial.
    desde_parcial: u64,
    ultimo_parcial: String,
    /// Desde cuando (ms de Vosk) hay algo a medias: el primer parcial con
    /// texto tras la ultima frase.
    a_medias_desde: Option<i64>,
    /// Lo callado seguido al final de lo dado (ms).
    callado_ms: i64,
    /// Se le dio audio desde la ultima frase cerrada (si no, no hay nada
    /// que cerrar).
    sin_cerrar: bool,
    n: u64,
    /// Lo tirado por no ser creible (`credibilidad`).
    pub descartadas: u64,
}

fn ms_de(marcos: u64, hz: u32) -> i64 {
    (marcos * 1000 / u64::from(hz.max(1))) as i64
}

fn muestras_de(ms: i64) -> usize {
    (ms.max(0) as u64 * u64::from(HERCIOS) / 1000) as usize
}

/// Si un trozo (16 kHz) tiene voz: algun pedazo de 20 ms por encima del
/// umbral de `en_vivo`.
fn con_voz(t: &[i16]) -> bool {
    t.chunks(HERCIOS as usize / 50).any(|c| {
        let s: f64 = c.iter().map(|x| f64::from(*x) * f64::from(*x)).sum();
        (s / c.len().max(1) as f64).sqrt() >= RMS_VOZ
    })
}

impl<R: Reconocedor> Corriente<R> {
    /// `idioma`: el de Ajustes («es-ES»), para lo creible.
    pub fn nueva(rec: R, idioma: &str) -> Corriente<R> {
        Corriente {
            rec,
            idioma: idioma.to_string(),
            hz: 0,
            esperado: None,
            desfase_ms: 0,
            dado: 0,
            pendiente: Vec::new(),
            desde_parcial: 0,
            ultimo_parcial: String::new(),
            a_medias_desde: None,
            callado_ms: 0,
            sin_cerrar: false,
            n: 0,
            descartadas: 0,
        }
    }

    /// Lo dado a Vosk, en ms de Vosk.
    fn oido_vosk_ms(&self) -> i64 {
        ms_de(self.dado, HERCIOS)
    }

    /// El milisegundo de la grabacion hasta donde se ha oido.
    pub fn oido_hasta_ms(&self) -> i64 {
        self.desfase_ms + ms_de(self.dado + self.pendiente.len() as u64, HERCIOS)
    }

    /// **Mete un bloque** (`posicion` en marcos, i16 intercalado con
    /// `canales`). Devuelve lo que salio (casi siempre nada).
    pub fn meter(
        &mut self,
        posicion: u64,
        muestreo: u32,
        canales: u16,
        muestras: &[i16],
    ) -> Vec<Oido> {
        let mut salida = Vec::new();
        if muestreo == 0 || muestras.is_empty() {
            return salida;
        }
        let c = usize::from(canales.max(1));
        if self.hz != muestreo {
            self.cerrar(&mut salida);
            self.hz = muestreo;
            self.esperado = None;
        }
        match self.esperado {
            None => {
                self.cerrar(&mut salida);
                self.desfase_ms = ms_de(posicion, muestreo) - self.oido_vosk_ms();
            }
            Some(e) if posicion > e => {
                let hueco = ms_de(posicion - e, muestreo);
                if hueco <= HUECO_MAXIMO_MS {
                    let ceros = vec![0i16; muestras_de(hueco)];
                    self.dar(&ceros, &mut salida);
                } else {
                    // Lo dicho se cierra ahi; lo que venga va en su minuto.
                    self.cerrar(&mut salida);
                    self.desfase_ms = ms_de(posicion, muestreo) - self.oido_vosk_ms();
                }
            }
            Some(e) if posicion < e => {
                let saltar = ((e - posicion) as usize) * c;
                if saltar >= muestras.len() {
                    return salida;
                }
                return self.meter(e, muestreo, canales, &muestras[saltar..]);
            }
            Some(_) => {}
        }
        let mono: Vec<i16> = muestras
            .chunks(c)
            .map(|m| {
                let s: i32 = m.iter().map(|x| i32::from(*x)).sum();
                (s / m.len() as i32) as i16
            })
            .collect();
        self.esperado = Some(posicion + mono.len() as u64);
        let a16 = crate::en_vivo::a_16k(&mono, muestreo);
        self.dar(&a16, &mut salida);
        salida
    }

    /// **Al acabar la grabacion**: lo que quedara a medias.
    pub fn acabar(&mut self) -> Vec<Oido> {
        let mut salida = Vec::new();
        self.cerrar(&mut salida);
        self.esperado = None;
        salida
    }

    /// Da audio a 16 kHz, de trozo en trozo.
    fn dar(&mut self, a16: &[i16], salida: &mut Vec<Oido>) {
        self.pendiente.extend_from_slice(a16);
        let trozo = muestras_de(TROZO_MS);
        while self.pendiente.len() >= trozo {
            let t: Vec<i16> = self.pendiente.drain(..trozo).collect();
            self.comer(&t, salida);
        }
    }

    fn comer(&mut self, t: &[i16], salida: &mut Vec<Oido>) {
        let cerrada = self.rec.comer(t);
        self.sin_cerrar = true;
        self.dado += t.len() as u64;
        self.desde_parcial += t.len() as u64;
        self.callado_ms = if con_voz(t) {
            0
        } else {
            self.callado_ms + ms_de(t.len() as u64, HERCIOS)
        };
        if cerrada {
            let json = self.rec.resultado();
            self.frase(&json, salida);
            return;
        }
        // Una frase larga se cierra en la primera pausa (o pasado el maximo).
        if let Some(desde) = self.a_medias_desde {
            let largo = self.oido_vosk_ms() - desde;
            if (largo >= LARGO_MS && self.callado_ms >= PAUSA_MS) || largo >= MAXIMO_MS {
                let json = self.rec.acabar();
                self.frase(&json, salida);
                return;
            }
        }
        if self.desde_parcial >= muestras_de(CADA_PARCIAL_MS) as u64 {
            self.desde_parcial = 0;
            let p = texto_del_parcial(&self.rec.parcial());
            if !p.is_empty() && self.a_medias_desde.is_none() {
                self.a_medias_desde = Some(self.oido_vosk_ms());
            }
            self.parcial(p, salida);
        }
    }

    fn parcial(&mut self, p: String, salida: &mut Vec<Oido>) {
        if p != self.ultimo_parcial {
            self.ultimo_parcial = p.clone();
            salida.push(Oido::Parcial(p));
        }
    }

    /// Lo que quede a medias, cerrado.
    fn cerrar(&mut self, salida: &mut Vec<Oido>) {
        if self.hz == 0 {
            return;
        }
        if !self.pendiente.is_empty() {
            let t = std::mem::take(&mut self.pendiente);
            self.comer(&t, salida);
        }
        if self.sin_cerrar {
            let json = self.rec.acabar();
            self.frase(&json, salida);
        }
        self.parcial(String::new(), salida);
        self.desde_parcial = 0;
    }

    /// Una frase cerrada, si dice algo creible.
    fn frase(&mut self, json: &str, salida: &mut Vec<Oido>) {
        self.sin_cerrar = false;
        self.a_medias_desde = None;
        // Ya no hay nada a medias.
        self.parcial(String::new(), salida);
        let Some(l) = leer_resultado(json) else {
            return;
        };
        if !creible(&l.texto, &self.idioma) {
            self.descartadas += 1;
            return;
        }
        let oido = self.oido_vosk_ms();
        let inicio = l.inicio_ms.unwrap_or(oido).min(oido);
        let fin = l.fin_ms.unwrap_or(oido).clamp(inicio, oido);
        salida.push(Oido::Frase(FraseOida {
            n: self.n,
            ms: self.desfase_ms + inicio,
            fin_voz_ms: self.desfase_ms + fin,
            callado_al_final_ms: oido - fin,
            texto: l.texto,
        }));
        self.n += 1;
    }
}

/// Lo leido de un resultado de Vosk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leido {
    pub texto: String,
    /// Donde empieza la primera palabra y acaba la ultima (ms desde que se
    /// creo el reconocedor), si vienen.
    pub inicio_ms: Option<i64>,
    pub fin_ms: Option<i64>,
}

/// `{"text": "…", "result": [{"word", "start", "end", "conf"}]}` → lo leido;
/// `None` si no dice nada (un silencio) o no se entiende.
pub fn leer_resultado(json: &str) -> Option<Leido> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let texto = v.get("text")?.as_str()?.trim();
    if texto.is_empty() {
        return None;
    }
    let palabras = v
        .get("result")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    let ms = |p: &serde_json::Value, campo: &str| {
        p.get(campo)
            .and_then(|s| s.as_f64())
            .map(|s| (s * 1000.0).round() as i64)
    };
    Some(Leido {
        texto: texto.to_string(),
        inicio_ms: palabras.first().and_then(|p| ms(p, "start")),
        fin_ms: palabras.last().and_then(|p| ms(p, "end")),
    })
}

/// `{"partial": "…"}` → el texto (vacio si no hay o no se entiende).
pub fn texto_del_parcial(json: &str) -> String {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get("partial")?.as_str().map(|s| s.trim().to_string()))
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// La biblioteca de verdad.

type FnNivel = unsafe extern "C" fn(c_int);
type FnModeloNuevo = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type FnModeloLibre = unsafe extern "C" fn(*mut c_void);
type FnRecNuevo = unsafe extern "C" fn(*mut c_void, f32) -> *mut c_void;
type FnRecPalabras = unsafe extern "C" fn(*mut c_void, c_int);
type FnRecComer = unsafe extern "C" fn(*mut c_void, *const i16, c_int) -> c_int;
type FnRecTexto = unsafe extern "C" fn(*mut c_void) -> *const c_char;
type FnRecNada = unsafe extern "C" fn(*mut c_void);

/// **`libvosk` cargada**, con sus funciones. No se descarga nunca (como
/// `motor` y `ort`: descargar una biblioteca con estado propio a mitad de
/// la vida del proceso no gana nada).
pub struct Vosk {
    nivel: FnNivel,
    modelo_nuevo: FnModeloNuevo,
    modelo_libre: FnModeloLibre,
    rec_nuevo: FnRecNuevo,
    rec_palabras: FnRecPalabras,
    rec_comer: FnRecComer,
    rec_resultado: FnRecTexto,
    rec_parcial: FnRecTexto,
    rec_final: FnRecTexto,
    rec_libre: FnRecNada,
}

// SAFETY: `Vosk` solo guarda punteros a funciones de una biblioteca que no
// se descarga; llamarlas desde cualquier hilo es lo que la API permite (la
// documentacion de Vosk: el modelo se comparte entre hilos, cada
// reconocedor es de uno).
unsafe impl Send for Vosk {}
// SAFETY: ver arriba; ninguna funcion guarda estado en `Vosk`.
unsafe impl Sync for Vosk {}

#[cfg(unix)]
mod dl {
    use std::ffi::{c_char, c_int, c_void};
    pub const RTLD_NOW: c_int = 2;
    #[cfg_attr(target_os = "linux", link(name = "dl"))]
    unsafe extern "C" {
        pub fn dlopen(fichero: *const c_char, modo: c_int) -> *mut c_void;
        pub fn dlsym(modulo: *mut c_void, nombre: *const c_char) -> *mut c_void;
    }
}

/// Un simbolo de la biblioteca cargada, o nulo.
#[cfg(windows)]
type Modulo = windows::Win32::Foundation::HMODULE;
#[cfg(unix)]
type Modulo = *mut c_void;

#[cfg(windows)]
fn abrir(dll: &Path) -> Option<Modulo> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::System::LibraryLoader::{LOAD_WITH_ALTERED_SEARCH_PATH, LoadLibraryExW};
    let ancha: Vec<u16> = dll
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: `ancha` es UTF-16 terminada en cero y viva durante la llamada.
    // Con `LOAD_WITH_ALTERED_SEARCH_PATH` las DLL de las que depende
    // (`libstdc++-6.dll`, `libgcc_s_seh-1.dll`, `libwinpthread-1.dll`) se
    // buscan junto a ella. Cargarla ejecuta su `DllMain`, que es lo que se
    // quiere; la puso la app al bajarla.
    unsafe {
        LoadLibraryExW(
            windows::core::PCWSTR(ancha.as_ptr()),
            None,
            LOAD_WITH_ALTERED_SEARCH_PATH,
        )
    }
    .ok()
}

#[cfg(unix)]
fn abrir(dll: &Path) -> Option<Modulo> {
    use std::os::unix::ffi::OsStrExt;
    let ruta = CString::new(dll.as_os_str().as_bytes()).ok()?;
    // SAFETY: `ruta` termina en cero y vive durante la llamada; `dlopen`
    // ejecuta los constructores de la biblioteca, que es lo que se quiere.
    let m = unsafe { dl::dlopen(ruta.as_ptr(), dl::RTLD_NOW) };
    (!m.is_null()).then_some(m)
}

/// La direccion de `nombre` (terminado en cero) en el modulo, o nula.
///
/// # Safety
///
/// `modulo` es un modulo cargado.
#[cfg(windows)]
unsafe fn simbolo(modulo: Modulo, nombre: &CStr) -> *const c_void {
    // SAFETY: lo promete el llamante; el nombre termina en cero.
    unsafe {
        windows::Win32::System::LibraryLoader::GetProcAddress(
            modulo,
            windows::core::PCSTR(nombre.as_ptr().cast()),
        )
    }
    .map_or(std::ptr::null(), |f| f as *const c_void)
}

/// La direccion de `nombre` en el modulo, o nula.
///
/// # Safety
///
/// `modulo` es un modulo cargado.
#[cfg(unix)]
unsafe fn simbolo(modulo: Modulo, nombre: &CStr) -> *const c_void {
    // SAFETY: lo promete el llamante; el nombre termina en cero.
    unsafe { dl::dlsym(modulo, nombre.as_ptr()) }.cast_const()
}

impl Vosk {
    /// **Carga la biblioteca** (`libvosk.dll` o `libvosk.so`).
    pub fn cargar(dll: &Path) -> Result<Arc<Vosk>, ErrorVoz> {
        let sin = || ErrorVoz::SinMotor {
            donde: dll.display().to_string(),
        };
        let modulo = abrir(dll).ok_or_else(sin)?;
        macro_rules! pedir {
            ($nombre:literal, $tipo:ty) => {{
                // SAFETY: el modulo esta cargado y el nombre es una
                // constante terminada en cero.
                let f = unsafe { simbolo(modulo, $nombre) };
                if f.is_null() {
                    return Err(sin());
                }
                // SAFETY: la firma es la de `vosk_api.h` (0.3.45), todas
                // `extern "C"`; el puntero no es nulo.
                unsafe { std::mem::transmute::<*const c_void, $tipo>(f) }
            }};
        }
        let v = Vosk {
            nivel: pedir!(c"vosk_set_log_level", FnNivel),
            modelo_nuevo: pedir!(c"vosk_model_new", FnModeloNuevo),
            modelo_libre: pedir!(c"vosk_model_free", FnModeloLibre),
            rec_nuevo: pedir!(c"vosk_recognizer_new", FnRecNuevo),
            rec_palabras: pedir!(c"vosk_recognizer_set_words", FnRecPalabras),
            rec_comer: pedir!(c"vosk_recognizer_accept_waveform_s", FnRecComer),
            rec_resultado: pedir!(c"vosk_recognizer_result", FnRecTexto),
            rec_parcial: pedir!(c"vosk_recognizer_partial_result", FnRecTexto),
            rec_final: pedir!(c"vosk_recognizer_final_result", FnRecTexto),
            rec_libre: pedir!(c"vosk_recognizer_free", FnRecNada),
        };
        // SAFETY: recien resuelta; solo toma un entero. -1: solo avisos.
        unsafe { (v.nivel)(-1) };
        Ok(Arc::new(v))
    }
}

/// **Un modelo de Vosk cargado** (tarda uno o dos segundos; se reutiliza).
pub struct ModeloVosk {
    vosk: Arc<Vosk>,
    modelo: *mut c_void,
}

// SAFETY: Vosk documenta que un modelo se comparte entre hilos (solo se lee
// tras cargarlo); lo libera `Drop`, una vez.
unsafe impl Send for ModeloVosk {}
// SAFETY: ver arriba.
unsafe impl Sync for ModeloVosk {}

impl Drop for ModeloVosk {
    fn drop(&mut self) {
        // SAFETY: lo devolvio `vosk_model_new` y solo se libera aqui. Vosk
        // cuenta referencias: un reconocedor vivo lo sigue teniendo.
        unsafe { (self.vosk.modelo_libre)(self.modelo) };
    }
}

impl ModeloVosk {
    /// Carga la carpeta del modelo (`vosk-model-small-es-0.42`).
    pub fn cargar(vosk: &Arc<Vosk>, carpeta: &Path) -> Result<ModeloVosk, ErrorVoz> {
        let ruta = ruta_de(carpeta)?;
        // SAFETY: `ruta` termina en cero y vive durante la llamada; Vosk se
        // queda con una copia. Nulo si la carpeta no es un modelo.
        let modelo = unsafe { (vosk.modelo_nuevo)(ruta.as_ptr()) };
        if modelo.is_null() {
            return Err(ErrorVoz::ModeloIlegible {
                donde: carpeta.display().to_string(),
            });
        }
        Ok(ModeloVosk {
            vosk: Arc::clone(vosk),
            modelo,
        })
    }

    /// Un reconocedor nuevo de este modelo, a 16 kHz y con las palabras.
    pub fn reconocedor(&self) -> Result<ReconocedorVosk, ErrorVoz> {
        // SAFETY: `self.modelo` sigue vivo (`&self`).
        let rec = unsafe { (self.vosk.rec_nuevo)(self.modelo, HERCIOS as f32) };
        if rec.is_null() {
            return Err(ErrorVoz::ModeloIlegible {
                donde: "vosk".into(),
            });
        }
        // SAFETY: `rec` recien creado. Las palabras con su tiempo en la
        // frase cerrada; en el parcial no (con ellas Vosk lo da cada segundo
        // y con 1,5-2 s de retraso en vez de 0,4-0,8 s, medido el 1-nov-2026).
        unsafe { (self.vosk.rec_palabras)(rec, 1) };
        Ok(ReconocedorVosk {
            vosk: Arc::clone(&self.vosk),
            rec,
        })
    }
}

#[cfg(windows)]
fn ruta_de(carpeta: &Path) -> Result<CString, ErrorVoz> {
    crate::motor::ruta_para_kaldi(carpeta)
}

#[cfg(unix)]
fn ruta_de(carpeta: &Path) -> Result<CString, ErrorVoz> {
    use std::os::unix::ffi::OsStrExt;
    CString::new(carpeta.as_os_str().as_bytes()).map_err(|_| ErrorVoz::RutaImposible {
        donde: carpeta.display().to_string(),
    })
}

/// **El reconocedor de Vosk** de una [`Corriente`]. De un hilo (no es
/// `Sync`), pero se puede llevar a otro.
pub struct ReconocedorVosk {
    vosk: Arc<Vosk>,
    rec: *mut c_void,
}

// SAFETY: un reconocedor se usa desde un hilo cada vez (`&mut self`); Vosk
// no lo ata a ningun hilo concreto.
unsafe impl Send for ReconocedorVosk {}

impl Drop for ReconocedorVosk {
    fn drop(&mut self) {
        // SAFETY: lo devolvio `vosk_recognizer_new` y solo se libera aqui.
        unsafe { (self.vosk.rec_libre)(self.rec) };
    }
}

impl ReconocedorVosk {
    /// Copia una cadena devuelta por el reconocedor (suya, valida hasta la
    /// siguiente llamada).
    fn copiar(p: *const c_char) -> String {
        if p.is_null() {
            return String::new();
        }
        // SAFETY: Vosk devuelve una cadena UTF-8 terminada en cero, valida
        // hasta la siguiente llamada al reconocedor; se copia en el acto.
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

impl Reconocedor for ReconocedorVosk {
    fn comer(&mut self, muestras: &[i16]) -> bool {
        // SAFETY: `muestras` vive durante la llamada y se pasa su longitud en
        // muestras (la version `_s`); Vosk no guarda el puntero.
        let r =
            unsafe { (self.vosk.rec_comer)(self.rec, muestras.as_ptr(), muestras.len() as c_int) };
        r == 1
    }

    fn resultado(&mut self) -> String {
        // SAFETY: `self.rec` vivo; ver `copiar`.
        Self::copiar(unsafe { (self.vosk.rec_resultado)(self.rec) })
    }

    fn parcial(&mut self) -> String {
        // SAFETY: igual.
        Self::copiar(unsafe { (self.vosk.rec_parcial)(self.rec) })
    }

    fn acabar(&mut self) -> String {
        // SAFETY: igual; el final cierra lo que quede (sus tiempos siguen
        // contando desde que se creo, medido el 1-nov-2026 con 0.3.45).
        Self::copiar(unsafe { (self.vosk.rec_final)(self.rec) })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un reconocedor de mentira: lo que tiene voz es una palabra; cierra
    /// la frase en el primer trozo callado si `cierra_solo`, y si no solo
    /// cuando se le pide (`acabar`). Los tiempos cuentan desde que se creo.
    #[derive(Default)]
    struct DeMentira {
        cierra_solo: bool,
        /// Muestras dadas desde que se creo.
        dado: usize,
        /// Donde empezo y acabo la voz de la frase de ahora (muestras).
        voz: Option<(usize, usize)>,
        cerrada: Option<(usize, usize)>,
        parciales: usize,
    }

    impl DeMentira {
        fn json(v: Option<(usize, usize)>) -> String {
            match v {
                Some((a, b)) => {
                    let s = |m: usize| m as f64 / 16_000.0;
                    let t = (b - a) / 3;
                    format!(
                        r#"{{"result":[{{"word":"hola","start":{},"end":{}}},{{"word":"que","start":{},"end":{}}},{{"word":"tal","start":{},"end":{}}}],"text":"hola que tal"}}"#,
                        s(a),
                        s(a + t),
                        s(a + t),
                        s(a + 2 * t),
                        s(a + 2 * t),
                        s(b)
                    )
                }
                None => r#"{"text":""}"#.into(),
            }
        }
    }

    impl Reconocedor for DeMentira {
        fn comer(&mut self, m: &[i16]) -> bool {
            let voz = m.iter().any(|x| x.abs() > 1000);
            let desde = self.dado;
            self.dado += m.len();
            if voz {
                let (a, _) = self.voz.unwrap_or((desde, desde));
                self.voz = Some((a, self.dado));
                false
            } else if self.cierra_solo && self.voz.is_some() {
                self.cerrada = self.voz.take();
                true
            } else {
                false
            }
        }
        fn resultado(&mut self) -> String {
            Self::json(self.cerrada.take())
        }
        fn parcial(&mut self) -> String {
            self.parciales += 1;
            if self.voz.is_some() {
                r#"{"partial":"hola que"}"#.into()
            } else {
                r#"{"partial":""}"#.into()
            }
        }
        fn acabar(&mut self) -> String {
            Self::json(self.voz.take())
        }
    }

    fn que_cierra_solo() -> Corriente<DeMentira> {
        Corriente::nueva(
            DeMentira {
                cierra_solo: true,
                ..Default::default()
            },
            "es-ES",
        )
    }

    /// `ms` de 48 kHz estereo: voz (un seno fuerte) o silencio.
    fn bloque(ms: i64, voz: bool) -> Vec<i16> {
        let n = (ms * 48) as usize;
        (0..n)
            .flat_map(|i| {
                let s = if voz {
                    (8_000.0 * (i as f64 * 0.06).sin()) as i16
                } else {
                    0
                };
                [s, s]
            })
            .collect()
    }

    /// Mete `pcm` en bloques de 10 ms desde `desde` (marcos a 48 kHz).
    fn en_bloques(c: &mut Corriente<DeMentira>, pcm: &[i16], desde: u64) -> (Vec<Oido>, u64) {
        let mut pos = desde;
        let mut v = Vec::new();
        for b in pcm.chunks(480 * 2) {
            v.extend(c.meter(pos, 48_000, 2, b));
            pos += (b.len() / 2) as u64;
        }
        (v, pos)
    }

    fn frases(v: &[Oido]) -> Vec<&FraseOida> {
        v.iter()
            .filter_map(|o| match o {
                Oido::Frase(f) => Some(f),
                Oido::Parcial(_) => None,
            })
            .collect()
    }

    #[test]
    fn la_frase_sale_con_su_minuto_de_la_grabacion_y_antes_el_parcial() {
        let mut c = que_cierra_solo();
        // La grabacion ya iba por el segundo 10 cuando empezo a oirse.
        let mut pcm = bloque(1_000, false);
        pcm.extend(bloque(1_500, true));
        pcm.extend(bloque(600, false));
        let (v, _) = en_bloques(&mut c, &pcm, 480_000);
        let f = frases(&v);
        assert_eq!(f.len(), 1, "{v:?}");
        assert_eq!(f[0].n, 0);
        assert!((10_990..=11_010).contains(&f[0].ms), "{}", f[0].ms);
        assert!(
            (12_490..=12_510).contains(&f[0].fin_voz_ms),
            "{}",
            f[0].fin_voz_ms
        );
        assert!(
            f[0].callado_al_final_ms <= 110,
            "{}",
            f[0].callado_al_final_ms
        );
        assert_eq!(f[0].texto, "hola que tal");
        // Mientras hablaba, el parcial; y al cerrar, vacio.
        let i = v.iter().position(|o| matches!(o, Oido::Frase(_))).unwrap();
        assert!(v[..i].contains(&Oido::Parcial("hola que".into())));
        assert_eq!(v[i - 1], Oido::Parcial(String::new()));
    }

    #[test]
    fn el_mismo_parcial_no_se_repite() {
        let mut c = que_cierra_solo();
        let (v, _) = en_bloques(&mut c, &bloque(3_000, true), 0);
        let parciales = v
            .iter()
            .filter(|o| matches!(o, Oido::Parcial(p) if !p.is_empty()))
            .count();
        assert_eq!(parciales, 1, "{v:?}");
        // Se pidio cada 200 ms, no en cada bloque de 10 ms.
        assert!((13..=16).contains(&c.rec.parciales), "{}", c.rec.parciales);
    }

    #[test]
    fn quien_habla_de_corrido_se_corta_en_la_primera_pausa_tras_el_largo() {
        // Vosk no cierra solo: lo cierra la corriente.
        let mut c = Corriente::nueva(DeMentira::default(), "es-ES");
        // 1 s y una pausa corta (aun no es largo), 4,5 s y otra pausa (ya
        // si), 2 s y silencio.
        let mut pcm = bloque(1_000, true);
        pcm.extend(bloque(300, false));
        pcm.extend(bloque(4_500, true));
        pcm.extend(bloque(300, false));
        pcm.extend(bloque(2_000, true));
        pcm.extend(bloque(1_000, false));
        let (mut v, _) = en_bloques(&mut c, &pcm, 0);
        let antes = frases(&v).len();
        assert_eq!(antes, 1, "{v:?}");
        v.extend(c.acabar());
        let f = frases(&v);
        assert_eq!(f.len(), 2);
        // La primera acaba en la segunda pausa, no en la primera.
        assert!(
            (5_790..=5_810).contains(&f[0].fin_voz_ms),
            "{}",
            f[0].fin_voz_ms
        );
        assert!((6_090..=6_110).contains(&f[1].ms), "{}", f[1].ms);
        // Caso negativo: sin pausa, se corta igual pasado el maximo.
        let mut c = Corriente::nueva(DeMentira::default(), "es-ES");
        let (v, _) = en_bloques(&mut c, &bloque(12_000, true), 0);
        let f = frases(&v);
        assert_eq!(f.len(), 1, "{v:?}");
        assert!(f[0].fin_voz_ms - f[0].ms <= MAXIMO_MS + 500);
    }

    #[test]
    fn un_hueco_largo_cierra_la_frase_y_la_cuenta_sigue_en_su_minuto() {
        let mut c = que_cierra_solo();
        let (v, pos) = en_bloques(&mut c, &bloque(1_000, true), 0);
        assert!(frases(&v).is_empty());
        // Se pierden 3 s (el oido iba tarde): lo dicho se cierra ahi.
        let mut pcm = bloque(800, true);
        pcm.extend(bloque(500, false));
        let (v, _) = en_bloques(&mut c, &pcm, pos + 3 * 48_000);
        let f = frases(&v);
        assert_eq!(f.len(), 2, "{v:?}");
        assert!(f[0].ms <= 10 && (990..=1_010).contains(&f[0].fin_voz_ms));
        // La segunda empieza en el segundo 4 de la grabacion, no en el 1.
        assert!((3_990..=4_010).contains(&f[1].ms), "{}", f[1].ms);
        assert_eq!(f[1].n, 1);
    }

    #[test]
    fn un_hueco_corto_se_rellena_y_no_mueve_el_reloj() {
        let mut c = que_cierra_solo();
        let (_, pos) = en_bloques(&mut c, &bloque(1_000, false), 0);
        // 300 ms perdidos, luego voz y silencio.
        let mut pcm = bloque(1_000, true);
        pcm.extend(bloque(500, false));
        let (v, _) = en_bloques(&mut c, &pcm, pos + 300 * 48);
        let f = frases(&v);
        assert_eq!(f.len(), 1);
        assert!((1_290..=1_310).contains(&f[0].ms), "{}", f[0].ms);
    }

    #[test]
    fn al_acabar_sale_lo_que_quedaba_a_medias() {
        let mut c = que_cierra_solo();
        let (v, _) = en_bloques(&mut c, &bloque(1_250, true), 0);
        assert!(frases(&v).is_empty());
        let v = c.acabar();
        assert_eq!(frases(&v).len(), 1, "{v:?}");
        // Caso negativo: acabar otra vez no repite nada.
        assert!(frases(&c.acabar()).is_empty());
    }

    #[test]
    fn lo_que_vosk_devuelve_raro_no_tumba_nada() {
        assert_eq!(leer_resultado("no es json"), None);
        assert_eq!(leer_resultado(r#"{"text":""}"#), None);
        assert_eq!(leer_resultado(r#"{"text":"  "}"#), None);
        assert_eq!(
            leer_resultado(r#"{"text":"sin tiempos"}"#),
            Some(Leido {
                texto: "sin tiempos".into(),
                inicio_ms: None,
                fin_ms: None,
            })
        );
        let l = leer_resultado(
            r#"{"result":[{"conf":1.0,"end":1.2,"start":0.9,"word":"a"},{"conf":0.8,"end":2.05,"start":1.5,"word":"b"}],"text":"a b"}"#,
        )
        .unwrap();
        assert_eq!((l.inicio_ms, l.fin_ms), (Some(900), Some(2_050)));
        assert_eq!(
            texto_del_parcial(r#"{"partial" : "va oyendo "}"#),
            "va oyendo"
        );
        assert_eq!(texto_del_parcial("{}"), "");
        assert_eq!(texto_del_parcial("roto"), "");
    }
}
