//! **El diario de cambios de este aparato** (27-oct-2026).
//!
//! Puerto de `sincro/Diario.kt` de Android (commit `261b67a5`), con el
//! formato de `docs/sincronizacion-por-diario.md` de alli. El usuario: «sería
//! mejor guardar todos los cambios que se hacen en el dispositivo en un solo
//! archivo (qué se añadió, cambió o quitó y cuándo) y que al sincronizar la
//! app lea solo ese archivo y sincronice solo eso».
//!
//! Hasta hoy cada vuelta, en **cada** chat y en los dos aparatos, hacia la
//! copia de seguridad, leia el chat entero, recorria sus lienzos y volvia a
//! resumir cada archivo pequeno; y otra vez al cerrar. Aqui se apunta, **una
//! linea por cambio** (`sincro/diario/diario.jsonl`), todo lo que se escribe
//! en lo que viaja, con un **numero de orden** que solo crece. Con eso «¿que
//! cambio en este chat desde que quedo igual con el movil?» es mirar un
//! numero, no el disco: ver [Vista::cambio] y los [Cursores] por aparato,
//! que usa `Sesion::preparar`.
//!
//! **Nunca es la unica verdad, y esto es lo importante.** Si falta, esta
//! roto o no lo explica todo, la vuelta hace lo de siempre (mirarlo todo):
//! un fallo aqui cuesta tiempo, nunca datos.
//!
//! - **El barrido** ([Diario::barrer]): al empezar cada vuelta se mira el
//!   tamano, la fecha en nanosegundos y el nodo de cada archivo de lo que
//!   viaja —sin leerlo— y lo cambiado sin que nadie lo apuntara se apunta
//!   entonces. Lo escrito hace menos de [RECIENTE_MS] lleva ademas su CRC32
//!   (los «racily clean» de git).
//! - **Lo que no se entiende es «todo»** ([TODO]): una linea rota, el indice
//!   perdido, un indice de muchos chats cambiado sin apunte.
//! - **La verificacion entera** cada [VUELTAS_POR_VERIFICACION] vueltas o
//!   cada dia con cada aparato ([Cursores::toca_verificar]).
//!
//! En el PC las rutas del diario son las de este equipo relativas a su
//! carpeta de datos (`proyectos/<ficha>/lienzos/d1.excalidraw`), igual que en
//! Android son las de `files`: el diario es de este aparato y no viaja. Lo
//! que dice de que chat es cada cosa lo pone el `Disco` (`Disco::barrido`,
//! `Disco::chats_que_nombran`).
//!
//! Una instancia por carpeta y proceso ([Diario::de]); entre procesos se
//! escribe con un cerrojo de archivo y cada uno relee lo que anadieron los
//! otros.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::canonico;
use crate::disco::Marca;
use crate::mensajes::ArchivoInfo;

/// Un mensaje.
pub const MENSAJE: &str = "m";
/// Un proyecto.
pub const PROYECTO: &str = "p";
/// Un archivo (`o` es su ruta).
pub const ARCHIVO: &str = "f";
/// La lapida de un proyecto.
pub const LAPIDA: &str = "l";
/// **No se sabe que cambio: todos los chats.**
pub const TODO: &str = "x";
pub const ALTA: &str = "alta";
pub const CAMBIO: &str = "cambio";
pub const BAJA: &str = "baja";
/// La huella de un archivo que ya no esta.
pub const NO_ESTA: &str = "-";

/// Lo escrito hace menos de esto puede no haber movido la fecha: se mira
/// tambien su CRC.
pub const RECIENTE_MS: i64 = 3_000;
/// Por encima, ni el CRC de lo reciente (un video recien llegado no se
/// relee).
pub const TOPE_DE_CRC: u64 = 8 << 20;
/// Se compacta cuando pasa de esto y hay el triple de lineas que de cosas.
pub const COMPACTAR_DESDE: usize = 4_000;
/// Si aun compactado quedan mas cosas, se olvida la mitad mas vieja.
pub const TOPE_DE_COSAS: usize = 200_000;
/// Cada tantas vueltas con un aparato se mira todo.
pub const VUELTAS_POR_VERIFICACION: u32 = 25;
/// Y cada dia.
pub const VERIFICAR_CADA_MS: i64 = 24 * 3_600_000;

/// Las marcas de borrado: un indice de muchos chats.
pub const RUTA_DE_MARCAS: &str = "sincro/borrados.jsonl";
/// Las lapidas: otro.
pub const RUTA_DE_LAPIDAS: &str = "sincro/chatsborrados.jsonl";

/// Un chat de mentira para «todos» al resolver archivos.
const TODOS: &str = "\u{0}todos";

fn es_cero(v: &i64) -> bool {
    *v == 0
}

fn es_cambio(a: &String) -> bool {
    a == CAMBIO
}

fn cambio_por_omision() -> String {
    CAMBIO.into()
}

/// Una linea del diario. Nombres de una letra porque hay miles: `n` el
/// numero de orden, `t` la hora de este aparato (solo para leerla: el orden
/// lo da `n`), `k` que ([MENSAJE], [PROYECTO], [ARCHIVO], [LAPIDA] o
/// [TODO]), `c` de que chat (nulo en un archivo), `o` cual, `a` [ALTA],
/// [CAMBIO] o [BAJA], `r` el archivo del disco que se escribio y `h` su
/// huella tras escribirlo, que es lo que deja al barrido reconocer lo ya
/// apuntado. Los campos con su valor por omision no se escriben.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Apunte {
    #[serde(default, skip_serializing_if = "es_cero")]
    pub n: i64,
    #[serde(default, skip_serializing_if = "es_cero")]
    pub t: i64,
    pub k: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub o: String,
    #[serde(default = "cambio_por_omision", skip_serializing_if = "es_cambio")]
    pub a: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<String>,
}

impl Apunte {
    pub fn nuevo(k: &str, c: Option<&str>, o: &str, a: &str) -> Apunte {
        Apunte {
            n: 0,
            t: 0,
            k: k.into(),
            c: c.map(str::to_string),
            o: o.into(),
            a: a.into(),
            r: None,
            h: None,
        }
    }

    /// «No se sabe que cambio»: la vuelta siguiente lo mira todo.
    pub fn todo(motivo: &str) -> Apunte {
        Apunte::nuevo(TODO, None, motivo, CAMBIO)
    }

    /// Con el archivo que se escribio y su huella de ahora.
    pub fn con_huella(mut self, r: &str, h: String) -> Apunte {
        self.r = Some(r.into());
        self.h = Some(h);
        self
    }

    /// De cada cosa basta su ultimo apunte (ver [Diario] y la compactacion).
    fn clave(&self) -> String {
        if self.k == ARCHIVO {
            format!("{}|{}", self.k, self.o)
        } else {
            format!(
                "{}|{}|{}",
                self.k,
                self.c.as_deref().unwrap_or("null"),
                self.o
            )
        }
    }
}

/// La primera linea: el libro (cambia si el diario vuelve a nacer) y el
/// piso (lo olvidado al compactar).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cabecera {
    pub libro: String,
    #[serde(default)]
    pub piso: i64,
    #[serde(default = "uno")]
    pub v: u32,
}

fn uno() -> u32 {
    1
}

/// **Hasta donde quedo igual un chat con un aparato**: el numero del diario
/// de este aparato cuando, al cerrar, los dos lados estaban identicos.
/// `alli` es la huella de lo que el otro dijo tener entonces
/// ([huella_de_inventario]) y `sello` el de lo acordado: con un aparato que
/// no lleva diario (un PixPin de antes) basta pedirle su inventario y ver
/// que sigue siendo ese.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cursor {
    pub n: i64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub alli: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub sello: String,
}

/// Los cursores con un aparato (`sincro/diario/cursores/<id>.json`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cursores {
    pub libro: String,
    pub chats: BTreeMap<String, Cursor>,
    /// Cuando se hizo la ultima vuelta mirandolo todo con este aparato.
    pub verificado: i64,
    /// Las vueltas desde entonces.
    pub vueltas: u32,
}

impl Cursores {
    /// **Toca mirarlo todo**: cada [VUELTAS_POR_VERIFICACION] vueltas o cada
    /// [VERIFICAR_CADA_MS], o si el reloj fue hacia atras. Sin fecha aun (los
    /// cursores los puso el otro al dirigir el), cuenta desde la primera.
    pub fn toca_verificar(&self, ahora: i64) -> bool {
        self.vueltas >= VUELTAS_POR_VERIFICACION
            || (self.verificado > 0
                && (ahora - self.verificado >= VERIFICAR_CADA_MS || ahora < self.verificado))
    }
}

/// Lo que dice el diario, ya por chats: el ultimo numero que toco cada uno y
/// el ultimo [TODO].
#[derive(Debug, Clone, Default)]
pub struct Vista {
    pub libro: String,
    pub piso: i64,
    pub ultimo: i64,
    pub todo: i64,
    pub por_chat: HashMap<String, i64>,
}

impl Vista {
    /// Si `chat` cambio despues de `n`.
    pub fn cambio(&self, chat: &str, n: i64) -> bool {
        self.todo > n || self.por_chat.get(chat).copied().unwrap_or(0) > n
    }

    pub fn vale_libro(&self, c: &Cursores) -> bool {
        c.libro == self.libro
    }

    pub fn vale(&self, c: &Cursor) -> bool {
        c.n >= self.piso && c.n <= self.ultimo
    }

    /// El chat sigue como quedo con ese cursor.
    pub fn quieto(&self, cs: &Cursores, chat: &str) -> bool {
        let Some(c) = cs.chats.get(chat) else {
            return false;
        };
        self.vale_libro(cs) && self.vale(c) && !self.cambio(chat, c.n)
    }
}

/// **El interruptor**: apagado, ninguna vuelta usa el diario (se apunta
/// igual). Para medir y como salida de emergencia.
static ACTIVO: AtomicBool = AtomicBool::new(true);

pub fn activo() -> bool {
    ACTIVO.load(Ordering::SeqCst)
}

pub fn poner_activo(si: bool) {
    ACTIVO.store(si, Ordering::SeqCst);
}

#[derive(Default)]
struct Estado {
    cargado: bool,
    libro: String,
    piso: i64,
    ultimo: i64,
    leido_hasta: u64,
    nodo: Option<String>,
    lineas: usize,
    todo: i64,
    por_chat: HashMap<String, i64>,
    por_archivo: HashMap<String, i64>,
    claves: HashSet<String>,
    /// La huella con que se apunto por ultima vez cada archivo del disco.
    huella_apuntada: HashMap<String, String>,
    /// Lo que fallo al apuntar: lo primero que se apunte despues sera un
    /// [TODO].
    perdido: bool,
    /// Archivo → chats que lo nombran, sacado de cada alcance calculado.
    indice: HashMap<String, BTreeSet<String>>,
    inverso: HashMap<String, BTreeSet<String>>,
    indice_sucio: bool,
    indice_leido: bool,
    ultimo_motivo: Option<String>,
}

impl Estado {
    fn vaciar(&mut self) {
        self.libro.clear();
        self.piso = 0;
        self.ultimo = 0;
        self.leido_hasta = 0;
        self.lineas = 0;
        self.todo = 0;
        self.por_chat.clear();
        self.por_archivo.clear();
        self.claves.clear();
        self.huella_apuntada.clear();
    }

    fn anotar(&mut self, a: &Apunte) {
        self.lineas += 1;
        self.ultimo = self.ultimo.max(a.n);
        self.claves.insert(a.clave());
        match a.k.as_str() {
            TODO => self.todo = self.todo.max(a.n),
            ARCHIVO => {
                let v = self.por_archivo.entry(a.o.clone()).or_insert(0);
                *v = (*v).max(a.n);
            }
            _ => {
                if let Some(c) = &a.c {
                    let v = self.por_chat.entry(c.clone()).or_insert(0);
                    *v = (*v).max(a.n);
                }
            }
        }
        if let (Some(r), Some(h)) = (&a.r, &a.h) {
            self.huella_apuntada.insert(r.clone(), h.clone());
        }
    }
}

/// Lo que el barrido recorre de un aparato y como entiende lo que encuentra.
pub struct Barrido<'a> {
    /// Carpetas relativas a la raiz, con la barra del final, que se recorren
    /// enteras.
    pub carpetas: Vec<String>,
    /// Archivos sueltos relativos a la raiz (los indices de muchos chats).
    pub sueltos: Vec<String>,
    /// Cosas que no son un archivo pero cambian como uno: su clave y su
    /// valor de ahora (en el PC, la ficha de cada chat en el indice).
    pub virtuales: Vec<(String, String)>,
    /// Lo que se apunta cuando `rel` cambio (`accion`) sin que nadie lo
    /// apuntara.
    pub detectado: &'a dyn Fn(&str, &str) -> Vec<Apunte>,
}

/// Lo que resuelve de que chats son unos archivos que el indice no conoce.
/// `None` si no se pudo saber: entonces cuentan para todos.
pub type Resolver<'a> = &'a dyn Fn(&BTreeSet<String>) -> Option<HashMap<String, BTreeSet<String>>>;

/// El diario de una carpeta de datos. Barato de clonar: lo de dentro es uno
/// por carpeta y proceso.
#[derive(Clone)]
pub struct Diario {
    raiz: PathBuf,
    estado: Arc<Mutex<Estado>>,
}

static DIARIOS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<Estado>>>>> = OnceLock::new();

/// Un archivo de cerrojo: mientras existe, otro proceso no escribe. Se quita
/// al soltarlo.
struct Cerrojo(PathBuf);

impl Drop for Cerrojo {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

impl Diario {
    /// **El diario de la carpeta de datos `raiz`**: uno por carpeta y
    /// proceso. Vive en `raiz/sincro/diario/`.
    pub fn de(raiz: &Path) -> Diario {
        let clave = std::fs::canonicalize(raiz).unwrap_or_else(|_| raiz.to_path_buf());
        let mapa = DIARIOS.get_or_init(|| Mutex::new(HashMap::new()));
        let estado = mapa
            .lock()
            .map(|mut m| m.entry(clave.clone()).or_default().clone())
            .unwrap_or_default();
        Diario {
            raiz: raiz.to_path_buf(),
            estado,
        }
    }

    fn carpeta(&self) -> PathBuf {
        self.raiz.join("sincro").join("diario")
    }

    fn archivo_del_diario(&self) -> PathBuf {
        self.carpeta().join("diario.jsonl")
    }

    fn archivo_de_huellas(&self) -> PathBuf {
        self.carpeta().join("huellas.tsv")
    }

    fn archivo_del_indice(&self) -> PathBuf {
        self.carpeta().join("indice.tsv")
    }

    fn carpeta_de_cursores(&self) -> PathBuf {
        self.carpeta().join("cursores")
    }

    fn bloquear(&self) -> std::sync::MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// **El cerrojo entre procesos**: un archivo que se crea solo si no
    /// existe. Uno que lleva mas de diez segundos es de un proceso que murio
    /// con el puesto. Si en dos segundos no se consigue, se sigue sin el: el
    /// diario nunca es la unica verdad.
    fn cerrojo(&self) -> Option<Cerrojo> {
        let f = self.carpeta().join("cerrojo");
        std::fs::create_dir_all(self.carpeta()).ok()?;
        for _ in 0..400 {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&f)
            {
                Ok(mut a) => {
                    let _ = write!(a, "{}", std::process::id());
                    return Some(Cerrojo(f));
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    let viejo = std::fs::metadata(&f)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.elapsed().ok())
                        .is_some_and(|d| d.as_secs() >= 10);
                    if viejo {
                        let _ = std::fs::remove_file(&f);
                        continue;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(_) => return None,
            }
        }
        None
    }

    // ------------------------------------------------------------ leer

    /// Pone al dia lo de memoria con lo del archivo (lo que anadieron otros
    /// procesos).
    fn al_dia(&self, e: &mut Estado) {
        let f = self.archivo_del_diario();
        let Some(meta) = std::fs::metadata(&f).ok().filter(|m| m.is_file()) else {
            let motivo = (e.cargado || e.ultimo > 0).then_some("el diario desapareció");
            self.nacer(e, motivo);
            return;
        };
        let su_nodo = pixpin_sistema::archivos::nodo_de(&f);
        if !e.cargado || su_nodo != e.nodo || meta.len() < e.leido_hasta {
            e.vaciar();
            self.leer_desde(e, 0);
            e.nodo = su_nodo;
            e.cargado = true;
        } else if meta.len() > e.leido_hasta {
            let desde = e.leido_hasta;
            self.leer_desde(e, desde);
        }
    }

    fn leer_desde(&self, e: &mut Estado, desde: u64) {
        let f = self.archivo_del_diario();
        let bytes = (|| -> io::Result<Vec<u8>> {
            let mut a = std::fs::File::open(&f)?;
            a.seek(SeekFrom::Start(desde))?;
            let mut b = Vec::new();
            a.read_to_end(&mut b)?;
            Ok(b)
        })()
        .unwrap_or_default();
        let mut inicio = 0usize;
        let mut roto = false;
        for (i, b) in bytes.iter().enumerate() {
            if *b != b'\n' {
                continue;
            }
            let linea = String::from_utf8_lossy(&bytes[inicio..i]).to_string();
            inicio = i + 1;
            if linea.trim().is_empty() {
                continue;
            }
            if e.libro.is_empty() {
                match serde_json::from_str::<Cabecera>(&linea) {
                    Ok(c) if !c.libro.trim().is_empty() => {
                        e.libro = c.libro;
                        e.piso = c.piso;
                        continue;
                    }
                    _ => {
                        roto = true;
                        break;
                    }
                }
            }
            match serde_json::from_str::<Apunte>(&linea) {
                Ok(a) => e.anotar(&a),
                Err(_) => roto = true,
            }
        }
        e.leido_hasta = desde + inicio as u64;
        // **Lo que quedo a medias al final** (la aplicacion muerta mientras
        // escribia una linea): bajo el cerrojo nadie esta escribiendo, asi
        // que es basura. Se corta y se apunta «todo».
        if inicio < bytes.len() {
            roto = true;
        }
        if e.libro.is_empty() {
            self.nacer(e, Some("el diario no tenía cabecera"));
            return;
        }
        if roto {
            if let Ok(a) = std::fs::OpenOptions::new().write(true).open(&f) {
                let _ = a.set_len(e.leido_hasta);
            }
            e.perdido = true;
        }
    }

    /// Un diario nuevo, con otro libro: los cursores de antes ya no valen y
    /// la vuelta lo mira todo.
    fn nacer(&self, e: &mut Estado, motivo: Option<&str>) {
        e.vaciar();
        let cab = Cabecera {
            libro: uuid(),
            piso: 0,
            v: 1,
        };
        let texto = serde_json::to_string(&cab).unwrap_or_default() + "\n";
        let _ = crate::disco::escribir_atomico(&self.archivo_del_diario(), texto.as_bytes());
        e.libro = cab.libro.clone();
        e.leido_hasta = std::fs::metadata(self.archivo_del_diario()).map_or(0, |m| m.len());
        e.nodo = pixpin_sistema::archivos::nodo_de(&self.archivo_del_diario());
        e.cargado = true;
        e.perdido = false;
        // El indice era del diario de antes: nace vacio y del mismo libro,
        // asi un indice que falte despues es uno perdido.
        e.indice.clear();
        e.inverso.clear();
        e.indice_leido = true;
        e.indice_sucio = crate::disco::escribir_atomico(
            &self.archivo_del_indice(),
            format!("#{}\n", cab.libro).as_bytes(),
        )
        .is_err();
        if let Some(m) = motivo {
            e.ultimo_motivo = Some(m.to_string());
        }
    }

    /// Por que nacio el diario la ultima vez (para las pruebas y el
    /// registro).
    pub fn ultimo_motivo(&self) -> Option<String> {
        self.bloquear().ultimo_motivo.clone()
    }

    // ------------------------------------------------------------ escribir

    /// Anade `nuevos` con sus numeros. **Nunca falla**: apuntar no puede
    /// tumbar a quien guarda el chat. Si falla, lo primero que se consiga
    /// apuntar despues sera un [TODO].
    pub fn apuntar(&self, nuevos: Vec<Apunte>) {
        if nuevos.is_empty() {
            return;
        }
        let mut e = self.bloquear();
        let hecho = (|| -> io::Result<()> {
            let _cerrojo = self.cerrojo();
            self.al_dia(&mut e);
            let ahora = ahora_ms();
            let mut lista = Vec::with_capacity(nuevos.len() + 1);
            if e.perdido {
                lista.push(Apunte {
                    t: ahora,
                    ..Apunte::todo("perdido")
                });
            }
            lista.extend(nuevos);
            let mut texto = String::new();
            let mut numerados = Vec::with_capacity(lista.len());
            let mut n = e.ultimo;
            for mut a in lista {
                n += 1;
                a.n = n;
                if a.t == 0 {
                    a.t = ahora;
                }
                texto.push_str(&serde_json::to_string(&a).map_err(io::Error::other)?);
                texto.push('\n');
                numerados.push(a);
            }
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.archivo_del_diario())?
                .write_all(texto.as_bytes())?;
            e.leido_hasta = std::fs::metadata(self.archivo_del_diario())?.len();
            for a in &numerados {
                e.anotar(a);
            }
            e.perdido = false;
            if e.lineas > COMPACTAR_DESDE && e.lineas > 3 * e.claves.len() {
                self.compactar(&mut e)?;
            }
            Ok(())
        })();
        if hecho.is_err() {
            e.perdido = true;
        }
    }

    /// **Deja de cada cosa solo su ultimo apunte.** «¿Cambio el chat X
    /// despues de N?» sale igual: si alguno de sus apuntes pasaba de N, el
    /// ultimo tambien. Si aun asi queda grande, se olvida la mitad mas vieja
    /// y sube el piso: los cursores por debajo dejan de valer.
    fn compactar(&self, e: &mut Estado) -> io::Result<()> {
        let texto = std::fs::read_to_string(self.archivo_del_diario())?;
        let mut ultimos: HashMap<String, Apunte> = HashMap::new();
        for l in texto.lines().skip(1) {
            if let Ok(a) = serde_json::from_str::<Apunte>(l) {
                ultimos.insert(a.clave(), a);
            }
        }
        let mut quedan: Vec<Apunte> = ultimos.into_values().collect();
        quedan.sort_by_key(|a| a.n);
        let mut piso = e.piso;
        if quedan.len() > TOPE_DE_COSAS {
            let fuera = quedan.len() - TOPE_DE_COSAS / 2;
            piso = piso.max(quedan[fuera - 1].n);
            quedan.drain(..fuera);
        }
        let mut salida = serde_json::to_string(&Cabecera {
            libro: e.libro.clone(),
            piso,
            v: 1,
        })
        .map_err(io::Error::other)?;
        salida.push('\n');
        for a in &quedan {
            salida.push_str(&serde_json::to_string(a).map_err(io::Error::other)?);
            salida.push('\n');
        }
        let tmp = self.carpeta().join("diario.jsonl.tmp");
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(salida.as_bytes())?;
            f.sync_all()?;
        }
        crate::disco::reemplazar(&tmp, &self.archivo_del_diario())?;
        let antes = e.ultimo;
        e.vaciar();
        self.leer_desde(e, 0);
        // El ultimo numero no baja nunca aunque su linea se fuera con la poda.
        e.ultimo = e.ultimo.max(antes);
        e.nodo = pixpin_sistema::archivos::nodo_de(&self.archivo_del_diario());
        e.cargado = true;
        Ok(())
    }

    // ------------------------------------------------------------ los ganchos

    /// **Unos mensajes se reescribieron** (`r`, relativo a la raiz): lo que
    /// entro, cambio o se fue, por su chat y su id. `antes` y `despues` son
    /// los mensajes portatiles (los de un chat, o los de todos).
    pub fn mensajes(&self, antes: &[canonico::Json], despues: &[canonico::Json], r: &str) {
        use crate::kotlin;
        let clave = |m: &canonico::Json| {
            (
                kotlin::chat_de(m),
                kotlin::cadena(m, "id").unwrap_or_default().to_string(),
            )
        };
        let a: HashMap<(String, String), &canonico::Json> =
            antes.iter().map(|m| (clave(m), m)).collect();
        let mut nuevos = Vec::new();
        let mut vistos = HashSet::new();
        for m in despues {
            let k = clave(m);
            if !vistos.insert(k.clone()) {
                continue;
            }
            match a.get(&k) {
                None => nuevos.push(Apunte::nuevo(MENSAJE, Some(&k.0), &kotlin::unico(m), ALTA)),
                Some(v) if *v != m => nuevos.push(Apunte::nuevo(
                    MENSAJE,
                    Some(&k.0),
                    &kotlin::unico(m),
                    CAMBIO,
                )),
                _ => {}
            }
        }
        for (k, m) in &a {
            if !vistos.contains(k) {
                nuevos.push(Apunte::nuevo(MENSAJE, Some(&k.0), &kotlin::unico(m), BAJA));
            }
        }
        // Nada distinto, pero el archivo si se escribio: que el barrido no lo
        // tome por ajeno.
        if nuevos.is_empty() {
            nuevos.push(Apunte::nuevo(MENSAJE, None, "", CAMBIO));
        }
        let h = self.huella(r);
        if let Some(u) = nuevos.last_mut() {
            u.r = Some(r.into());
            u.h = Some(h);
        }
        self.apuntar(nuevos);
    }

    /// Las marcas de borrado se escribieron (`sincro/borrados.jsonl`).
    pub fn marcas(&self, nuevas: &[Marca]) {
        let mut l: Vec<Apunte> = nuevas
            .iter()
            .map(|m| {
                Apunte::nuevo(
                    MENSAJE,
                    Some(&m.chat),
                    m.uid.as_deref().unwrap_or(&m.sena),
                    BAJA,
                )
            })
            .collect();
        if l.is_empty() {
            l.push(Apunte::nuevo(MENSAJE, None, "", CAMBIO));
        }
        let h = self.huella(RUTA_DE_MARCAS);
        if let Some(u) = l.last_mut() {
            u.r = Some(RUTA_DE_MARCAS.into());
            u.h = Some(h);
        }
        self.apuntar(l);
    }

    /// La lista de proyectos se reescribio (`r`): los que entraron,
    /// cambiaron o se fueron, por su id.
    pub fn proyectos(&self, antes: &[canonico::Json], despues: &[canonico::Json], r: &str) {
        use crate::kotlin;
        let id = |p: &canonico::Json| kotlin::cadena(p, "id").unwrap_or_default().to_string();
        let a: HashMap<String, &canonico::Json> = antes.iter().map(|p| (id(p), p)).collect();
        let d: HashMap<String, &canonico::Json> = despues.iter().map(|p| (id(p), p)).collect();
        let mut nuevos = Vec::new();
        for (i, p) in &d {
            match a.get(i) {
                None => nuevos.push(Apunte::nuevo(PROYECTO, Some(i), i, ALTA)),
                Some(v) if v != p => nuevos.push(Apunte::nuevo(PROYECTO, Some(i), i, CAMBIO)),
                _ => {}
            }
        }
        for i in a.keys() {
            if !d.contains_key(i) {
                nuevos.push(Apunte::nuevo(PROYECTO, Some(i), i, BAJA));
            }
        }
        if nuevos.is_empty() {
            nuevos.push(Apunte::nuevo(PROYECTO, None, "", CAMBIO));
        }
        let h = self.huella(r);
        if let Some(u) = nuevos.last_mut() {
            u.r = Some(r.into());
            u.h = Some(h);
        }
        self.apuntar(nuevos);
    }

    /// Un proyecto entero borrado o recuperado (lapida puesta o quitada).
    pub fn lapida(&self, chat: &str, accion: &str) {
        let h = self.huella(RUTA_DE_LAPIDAS);
        self.apuntar(vec![
            Apunte::nuevo(LAPIDA, Some(chat), chat, accion).con_huella(RUTA_DE_LAPIDAS, h),
        ]);
    }

    /// Se escribio o se quito `rel` (relativa a la raiz).
    pub fn archivo(&self, rel: &str) {
        self.archivos(&[rel.to_string()]);
    }

    /// Lo mismo para varios a la vez.
    pub fn archivos(&self, rels: &[String]) {
        let l: Vec<Apunte> = rels
            .iter()
            .map(|rel| {
                let h = self.huella(rel);
                let a = if h == NO_ESTA { BAJA } else { CAMBIO };
                Apunte::nuevo(ARCHIVO, None, rel, a).con_huella(rel, h)
            })
            .collect();
        self.apuntar(l);
    }

    /// «No se que cambio»: la proxima vuelta lo mira todo.
    pub fn todo(&self, motivo: &str) {
        self.apuntar(vec![Apunte::todo(motivo)]);
    }

    // ------------------------------------------------------------ preguntar

    /// El ultimo numero apuntado (tras releer lo de otros procesos).
    pub fn ultimo(&self) -> i64 {
        let mut e = self.bloquear();
        let _c = self.cerrojo();
        self.al_dia(&mut e);
        e.ultimo
    }

    /// El libro de ahora.
    pub fn libro(&self) -> String {
        let mut e = self.bloquear();
        let _c = self.cerrojo();
        self.al_dia(&mut e);
        e.libro.clone()
    }

    /// **Lo que dice el diario por chats.** `resolver` dice de que chats son
    /// los archivos que el indice no conoce; los que no nombra nadie no son
    /// de ningun chat y no cuentan. Solo se resuelven los apuntados despues
    /// de `desde`.
    pub fn vista(&self, desde: i64, resolver: Resolver<'_>) -> Vista {
        let (libro, piso, ultimo, todo, mut salida, archivos, inverso) = {
            let mut e = self.bloquear();
            self.leer_indice(&mut e);
            {
                let _c = self.cerrojo();
                self.al_dia(&mut e);
            }
            (
                e.libro.clone(),
                e.piso,
                e.ultimo,
                if e.perdido { i64::MAX } else { e.todo },
                e.por_chat.clone(),
                e.por_archivo.clone(),
                e.inverso.clone(),
            )
        };
        let mut desconocidos = BTreeSet::new();
        for (rel, n) in &archivos {
            if *n <= desde {
                continue;
            }
            match inverso.get(rel) {
                Some(cs) => {
                    for c in cs {
                        let v = salida.entry(c.clone()).or_insert(0);
                        *v = (*v).max(*n);
                    }
                }
                None => {
                    desconocidos.insert(rel.clone());
                }
            }
        }
        if !desconocidos.is_empty() {
            let resueltos = resolver(&desconocidos).unwrap_or_else(|| {
                desconocidos
                    .iter()
                    .map(|r| (r.clone(), BTreeSet::from([TODOS.to_string()])))
                    .collect()
            });
            for (rel, cs) in resueltos {
                let Some(n) = archivos.get(&rel) else {
                    continue;
                };
                for c in cs {
                    let v = salida.entry(c).or_insert(0);
                    *v = (*v).max(*n);
                }
            }
        }
        let todo = todo.max(salida.remove(TODOS).unwrap_or(0));
        Vista {
            libro,
            piso,
            ultimo,
            todo,
            por_chat: salida,
        }
    }

    // ------------------------------------------------------------ el indice

    /// **Los archivos de `chat` son estos** (rutas relativas a la raiz): lo
    /// dice quien acaba de calcular su alcance.
    pub fn indexar(&self, chat: &str, rels: &BTreeSet<String>) {
        let mut e = self.bloquear();
        self.leer_indice(&mut e);
        let viejos = e.indice.get(chat).cloned().unwrap_or_default();
        if viejos == *rels {
            return;
        }
        for r in viejos.difference(rels) {
            if let Some(s) = e.inverso.get_mut(r) {
                s.remove(chat);
                if s.is_empty() {
                    e.inverso.remove(r);
                }
            }
        }
        for r in rels.difference(&viejos) {
            e.inverso
                .entry(r.clone())
                .or_default()
                .insert(chat.to_string());
        }
        e.indice.insert(chat.to_string(), rels.clone());
        e.indice_sucio = true;
    }

    fn leer_indice(&self, e: &mut Estado) {
        if e.indice_leido {
            return;
        }
        {
            let _c = self.cerrojo();
            self.al_dia(e);
        }
        if e.indice_leido {
            // `al_dia` hizo nacer el diario, y con el su indice.
            return;
        }
        e.indice_leido = true;
        let texto = match std::fs::read_to_string(self.archivo_del_indice()) {
            Ok(t) => t,
            Err(err) => {
                // Sin indice y con un diario que ya tiene historia: un archivo
                // cambiado no sabria de que chat es. Que se mire todo una vez.
                if err.kind() != io::ErrorKind::NotFound || e.ultimo > 0 {
                    e.perdido = true;
                }
                e.indice_sucio = true;
                return;
            }
        };
        let mut lineas = texto.lines();
        if lineas.next() != Some(&format!("#{}", e.libro)) {
            e.perdido = true;
            e.indice_sucio = true;
            return;
        }
        for l in lineas {
            let (chat, rels) = l.split_once('\t').unwrap_or((l, ""));
            if chat.is_empty() {
                continue;
            }
            let rels: BTreeSet<String> = rels
                .split('\u{1}')
                .filter(|r| !r.is_empty())
                .map(str::to_string)
                .collect();
            for r in &rels {
                e.inverso
                    .entry(r.clone())
                    .or_default()
                    .insert(chat.to_string());
            }
            e.indice.insert(chat.to_string(), rels);
        }
    }

    /// Escribe el indice si cambio. Va antes que los cursores: un cursor sin
    /// su indice no vale.
    pub fn guardar_indice(&self) {
        let mut e = self.bloquear();
        self.guardar_indice_con(&mut e);
    }

    fn guardar_indice_con(&self, e: &mut Estado) {
        self.leer_indice(e);
        if !e.indice_sucio {
            return;
        }
        let mut texto = format!("#{}\n", e.libro);
        let mut chats: Vec<&String> = e.indice.keys().collect();
        chats.sort();
        for c in chats {
            texto.push_str(c);
            texto.push('\t');
            let rels: Vec<&str> = e.indice[c].iter().map(String::as_str).collect();
            texto.push_str(&rels.join("\u{1}"));
            texto.push('\n');
        }
        if crate::disco::escribir_atomico(&self.archivo_del_indice(), texto.as_bytes()).is_ok() {
            e.indice_sucio = false;
        }
    }

    // ------------------------------------------------------------ los cursores

    fn archivo_de_cursores(&self, otro: &str) -> PathBuf {
        self.carpeta_de_cursores()
            .join(format!("{}.json", crate::disco::limpio(otro)))
    }

    /// Los cursores con `otro` tal como estan guardados.
    pub fn cursores(&self, otro: &str) -> Cursores {
        std::fs::read_to_string(self.archivo_de_cursores(otro))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Cambia los cursores de `otro` con `cambio`, sobre el libro de ahora:
    /// si los guardados eran de otro libro se tiran antes (ya no valen).
    pub fn cambiar_cursores(&self, otro: &str, cambio: impl FnOnce(Cursores) -> Cursores) {
        let mut e = self.bloquear();
        {
            let _c = self.cerrojo();
            self.al_dia(&mut e);
        }
        self.guardar_indice_con(&mut e);
        let mut antes = self.cursores(otro);
        if antes.libro != e.libro {
            antes = Cursores {
                libro: e.libro.clone(),
                verificado: antes.verificado,
                vueltas: antes.vueltas,
                ..Default::default()
            };
        }
        let mut despues = cambio(antes);
        despues.libro = e.libro.clone();
        if let Ok(t) = serde_json::to_string(&despues) {
            let _ = crate::disco::escribir_atomico(&self.archivo_de_cursores(otro), t.as_bytes());
        }
    }

    /// Pone (o quita, con `None`) el cursor de `chat` con `otro`.
    pub fn poner_cursor(&self, otro: &str, chat: &str, c: Option<Cursor>) {
        self.cambiar_cursores(otro, |mut cs| {
            match c {
                Some(c) => {
                    cs.chats.insert(chat.to_string(), c);
                }
                None => {
                    cs.chats.remove(chat);
                }
            }
            cs
        });
    }

    /// Fuera todo lo acordado con los demas (salir del grupo).
    pub fn olvidar_cursores(&self) {
        let _e = self.bloquear();
        let _ = std::fs::remove_dir_all(self.carpeta_de_cursores());
    }

    // ------------------------------------------------------------ el barrido

    /// La huella de `rel` (relativa a la raiz) ahora: tamano, fecha en
    /// nanosegundos y nodo; [NO_ESTA] si no existe.
    pub fn huella(&self, rel: &str) -> String {
        let f = self.raiz.join(rel);
        match std::fs::metadata(&f) {
            Ok(m) if m.is_file() => Huella::de(&f, &m).texto(),
            _ => NO_ESTA.into(),
        }
    }

    /// La huella de una cosa virtual de [Barrido::virtuales] con `valor`.
    pub fn huella_virtual(valor: &str) -> String {
        format!("0:0:{valor}")
    }

    /// **Lo que cambio en lo que viaja sin que nadie lo apuntara**, mirando
    /// solo tamano, fecha y nodo de cada archivo (y el CRC de los recien
    /// escritos). Lo encontrado se apunta como diga `b.detectado`. Va al
    /// empezar cada vuelta, en los dos lados. Devuelve cuantos apunto.
    pub fn barrer(&self, b: &Barrido<'_>) -> usize {
        let antes = self.leer_huellas();
        let ahora = self.recorrer(b);
        let tomada = ahora_ms();
        let (huella_apuntada, ultimo, perdido) = {
            let mut e = self.bloquear();
            {
                let _c = self.cerrojo();
                self.al_dia(&mut e);
            }
            (e.huella_apuntada.clone(), e.ultimo, e.perdido)
        };
        let mut nuevos = Vec::new();
        let mut guardar: BTreeMap<String, Huella> = BTreeMap::new();
        for (rel, h) in ahora {
            let previa = antes.as_ref().and_then(|a| a.get(&rel));
            let reciente = !h.virtual_
                && tomada - ((h.fecha / 1_000_000) as i64) < RECIENTE_MS
                && h.bytes <= TOPE_DE_CRC;
            let iguales = previa.is_some_and(|p| p.texto() == h.texto());
            let con_crc = iguales && previa.is_some_and(|p| p.crc.is_some());
            let mut cambio = !iguales;
            let mut crc = None;
            if con_crc {
                crc = crc_de(&self.raiz.join(&rel));
                if crc != previa.and_then(|p| p.crc) {
                    cambio = true;
                }
            }
            if reciente && crc.is_none() {
                crc = crc_de(&self.raiz.join(&rel));
            }
            let texto = h.texto();
            guardar.insert(rel.clone(), Huella { crc, ..h });
            if antes.is_none() || !cambio {
                continue;
            }
            // Lo apunto quien lo escribio, con esta misma huella: ya esta.
            if huella_apuntada.get(&rel) == Some(&texto) && !con_crc {
                continue;
            }
            let accion = if previa.is_none() { ALTA } else { CAMBIO };
            nuevos.extend((b.detectado)(&rel, accion));
        }
        if let Some(a) = &antes {
            for rel in a.keys() {
                if guardar.contains_key(rel) {
                    continue;
                }
                if huella_apuntada.get(rel).map(String::as_str) == Some(NO_ESTA) {
                    continue;
                }
                nuevos.extend((b.detectado)(rel, BAJA));
            }
        }
        if antes.is_none() && ultimo > 0 {
            nuevos.push(Apunte::todo("sin huellas"));
        }
        // Lo que se perdio (una linea rota, el indice) se apunta ya: asi el
        // «todo» tiene su numero y el cursor de esta vuelta queda por encima.
        if nuevos.is_empty() && perdido {
            nuevos.push(Apunte::todo("perdido"));
        }
        let n = nuevos.len();
        self.apuntar(nuevos);
        self.escribir_huellas(&guardar, tomada);
        n
    }

    fn leer_huellas(&self) -> Option<BTreeMap<String, Huella>> {
        let texto = std::fs::read_to_string(self.archivo_de_huellas()).ok()?;
        let mut salida = BTreeMap::new();
        for l in texto.lines() {
            if l.starts_with('#') {
                continue;
            }
            let p: Vec<&str> = l.split('\t').collect();
            if p.len() < 5 {
                continue;
            }
            let virtual_ = p[0].starts_with('@');
            let (Ok(bytes), Ok(fecha)) = (p[1].parse(), p[2].parse()) else {
                return None;
            };
            salida.insert(
                p[0].to_string(),
                Huella {
                    bytes,
                    fecha,
                    nodo: p[3].to_string(),
                    crc: (p[4] != "-").then(|| p[4].parse().ok()).flatten(),
                    virtual_,
                },
            );
        }
        Some(salida)
    }

    fn escribir_huellas(&self, m: &BTreeMap<String, Huella>, tomada: i64) {
        let mut s = format!("#{tomada}\n");
        for (rel, h) in m {
            s.push_str(&format!(
                "{rel}\t{}\t{}\t{}\t{}\n",
                h.bytes,
                h.fecha,
                h.nodo,
                h.crc.map_or("-".to_string(), |c| c.to_string())
            ));
        }
        let _ = crate::disco::escribir_atomico(&self.archivo_de_huellas(), s.as_bytes());
    }

    /// Todos los archivos de lo que viaja, con su tamano, fecha y nodo.
    fn recorrer(&self, b: &Barrido<'_>) -> BTreeMap<String, Huella> {
        let mut salida = BTreeMap::new();
        for c in &b.carpetas {
            let mut pendientes = vec![(self.raiz.join(c), c.trim_end_matches('/').to_string(), 0)];
            while let Some((dir, rel, hondo)) = pendientes.pop() {
                let Ok(l) = std::fs::read_dir(&dir) else {
                    continue;
                };
                for e in l.flatten() {
                    let nombre = e.file_name().to_string_lossy().to_string();
                    let ruta = e.path();
                    let r = format!("{rel}/{nombre}");
                    // Una union de directorio (un proyecto en otra carpeta)
                    // se sigue: es lo de ese chat.
                    let Ok(meta) = std::fs::metadata(&ruta) else {
                        continue;
                    };
                    if meta.is_dir() {
                        if hondo < 24 {
                            pendientes.push((ruta, r, hondo + 1));
                        }
                    } else if meta.is_file() && !temporal(&r) {
                        salida.insert(r, Huella::de(&ruta, &meta));
                    }
                }
            }
        }
        for r in &b.sueltos {
            let f = self.raiz.join(r);
            if salida.contains_key(r) {
                continue;
            }
            if let Ok(m) = std::fs::metadata(&f)
                && m.is_file()
            {
                salida.insert(r.clone(), Huella::de(&f, &m));
            }
        }
        for (clave, valor) in &b.virtuales {
            salida.insert(
                clave.clone(),
                Huella {
                    bytes: 0,
                    fecha: 0,
                    nodo: valor.clone(),
                    crc: None,
                    virtual_: true,
                },
            );
        }
        salida
    }
}

/// Lo que se recuerda de un archivo del disco.
#[derive(Debug, Clone)]
struct Huella {
    bytes: u64,
    fecha: u128,
    nodo: String,
    crc: Option<u32>,
    virtual_: bool,
}

impl Huella {
    fn de(f: &Path, m: &std::fs::Metadata) -> Huella {
        Huella {
            bytes: m.len(),
            fecha: m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos()),
            nodo: pixpin_sistema::archivos::nodo_de(f).unwrap_or_default(),
            crc: None,
            virtual_: false,
        }
    }

    fn texto(&self) -> String {
        format!("{}:{}:{}", self.bytes, self.fecha, self.nodo)
    }
}

fn crc_de(f: &Path) -> Option<u32> {
    let mut e = std::fs::File::open(f).ok()?;
    let mut h = crc32fast::Hasher::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = e.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Some(h.finalize())
}

/// Los temporales no son de lo que viaja.
pub fn temporal(rel: &str) -> bool {
    rel.ends_with(".tmp")
        || rel.ends_with(".sincro")
        || rel.ends_with(".nuevo")
        || rel.ends_with(crate::disco::PARCIAL)
        || rel.ends_with(&format!("{}.json", crate::disco::PARCIAL))
}

fn ahora_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Un UUID al azar (version 4), como `UUID.randomUUID()`.
fn uuid() -> String {
    use rand_core::RngCore;
    let mut b = [0u8; 16];
    rand_core::OsRng.fill_bytes(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// **La huella de lo que el otro dijo tener** de un chat: sus mensajes y
/// marcas por codigo y resumen, su proyecto canonico y sus archivos por ruta
/// y resumen (sin fechas: un archivo vuelto a escribir igual no es un
/// cambio). Ver [Cursor::alli] y el vector de la guia del PC (§7.1).
pub fn huella_de_inventario(
    apuntes: &[crate::mensajes::Apunte],
    proyecto: Option<&str>,
    archivos: &[ArchivoInfo],
) -> String {
    let mut a: Vec<&crate::mensajes::Apunte> = apuntes.iter().collect();
    a.sort_by(|x, y| canonico::comparar_como_kotlin(&x.sena, &y.sena));
    let mut s = String::new();
    for x in a {
        s.push_str(&x.sena);
        s.push('=');
        s.push_str(&x.resumen);
        if x.borrado {
            s.push('!');
        }
        s.push('\n');
    }
    s.push_str("--\n");
    if let Some(p) = proyecto {
        s.push_str(&canonico::de(p));
    }
    s.push_str("\n--\n");
    let mut f: Vec<&ArchivoInfo> = archivos.iter().collect();
    f.sort_by(|x, y| canonico::comparar_como_kotlin(&x.ruta, &y.ruta));
    for x in f {
        s.push_str(&x.ruta);
        s.push('=');
        s.push_str(&x.resumen);
        s.push('\n');
    }
    canonico::sha256_hex(s.as_bytes())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn carpeta(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "pixpin-diario-{nombre}-{}-{}",
            std::process::id(),
            ahora_ms()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn m(c: &str, o: &str) -> Apunte {
        Apunte::nuevo(MENSAJE, Some(c), o, CAMBIO)
    }

    #[test]
    fn la_huella_de_un_inventario_es_la_de_la_guia_para_el_pc() {
        let ap = |sena: &str, resumen: &str, borrado: bool| crate::mensajes::Apunte {
            sena: sena.into(),
            creado: 5,
            tocado: 5,
            resumen: resumen.into(),
            borrado,
            alias: None,
        };
        let h = huella_de_inventario(
            &[ap("b", "2", false), ap("a", "1", true)],
            None,
            &[ArchivoInfo {
                ruta: "x.pdf".into(),
                resumen: "ff".into(),
                bytes: 10,
                tocado: 123,
                ..Default::default()
            }],
        );
        assert_eq!(
            h,
            "17850c345c3cb2825327a7bef787cab20e302089178cd80d61079c06ea5bf9d7"
        );
    }

    #[test]
    fn un_apunte_se_escribe_como_en_android() {
        let a = Apunte {
            n: 1,
            t: 1_761_550_000_123,
            ..Apunte::nuevo(MENSAJE, Some("general"), "U", ALTA)
                .con_huella("guardados.jsonl", "1:2:(n)".into())
        };
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            r#"{"n":1,"t":1761550000123,"k":"m","c":"general","o":"U","a":"alta","r":"guardados.jsonl","h":"1:2:(n)"}"#
        );
        // `cambio` es lo de por omision: no se escribe, y se lee igual.
        let b = Apunte {
            n: 2,
            ..Apunte::todo("x")
        };
        let t = serde_json::to_string(&b).unwrap();
        assert_eq!(t, r#"{"n":2,"k":"x","o":"x"}"#);
        assert_eq!(serde_json::from_str::<Apunte>(&t).unwrap().a, CAMBIO);
    }

    #[test]
    fn el_diario_apunta_con_numeros_que_solo_crecen_y_por_chats() {
        let raiz = carpeta("numeros");
        let d = Diario::de(&raiz);
        let antes = d.ultimo();
        d.apuntar(vec![m("p1", "a")]);
        d.apuntar(vec![m("p1", "b"), m("p2", "c")]);
        let v = d.vista(0, &|_| None);
        assert_eq!(v.ultimo, antes + 3);
        assert!(v.cambio("p1", antes));
        assert!(v.cambio("p2", antes + 2));
        assert!(!v.cambio("p2", antes + 3));
        assert!(!v.cambio("general", 0));
        let lineas = std::fs::read_to_string(raiz.join("sincro/diario/diario.jsonl")).unwrap();
        assert!(lineas.lines().next().unwrap().contains("\"libro\""));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn se_compacta_solo_y_sigue_diciendo_lo_mismo() {
        let raiz = carpeta("compactar");
        let d = Diario::de(&raiz);
        for i in 0..4_500 {
            d.apuntar(vec![m(&format!("c{}", i % 3), &format!("m{}", i % 5))]);
        }
        let lineas = std::fs::read_to_string(raiz.join("sincro/diario/diario.jsonl"))
            .unwrap()
            .lines()
            .count();
        assert!(lineas < 1_000, "compactado: {lineas} lineas");
        let v = d.vista(0, &|_| None);
        assert!(v.cambio("c0", v.ultimo - 3));
        assert!(!v.cambio("c9", 0));
        assert_eq!(v.ultimo, 4_500);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn una_linea_rota_o_un_diario_borrado_no_se_fian_se_mira_todo() {
        let raiz = carpeta("roto");
        let d = Diario::de(&raiz);
        d.apuntar(vec![m("a", "x")]);
        let n = d.ultimo();
        let f = raiz.join("sincro/diario/diario.jsonl");
        let mut a = std::fs::OpenOptions::new().append(true).open(&f).unwrap();
        a.write_all(b"{\"n\":99,\"k\":").unwrap();
        drop(a);
        // Otro proceso: lo de memoria se pone al dia con lo del archivo.
        d.apuntar(vec![m("b", "y")]);
        assert!(
            d.vista(0, &|_| None).cambio("cualquiera", n),
            "lo roto es «todo»"
        );
        let libro = d.libro();
        std::fs::remove_file(&f).unwrap();
        assert_ne!(d.libro(), libro, "diario nuevo, libro nuevo");
        assert_eq!(d.ultimo_motivo().as_deref(), Some("el diario desapareció"));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    fn barrido_simple(d: &Diario) -> usize {
        let detectado = |rel: &str, a: &str| vec![Apunte::nuevo(ARCHIVO, None, rel, a)];
        d.barrer(&Barrido {
            carpetas: vec!["guardados/".into()],
            sueltos: vec![],
            virtuales: vec![],
            detectado: &detectado,
        })
    }

    #[test]
    fn el_barrido_ve_lo_escrito_sin_avisar_aunque_sea_del_mismo_tamano_y_al_instante() {
        let raiz = carpeta("barrido");
        std::fs::create_dir_all(raiz.join("guardados")).unwrap();
        let a = raiz.join("guardados/a.bin");
        std::fs::write(&a, [1u8; 1000]).unwrap();
        let d = Diario::de(&raiz);
        assert_eq!(barrido_simple(&d), 0, "la primera vez solo se toma la foto");
        let n = d.ultimo();
        // Mismo tamano, en su sitio y enseguida: la fecha puede no moverse;
        // el CRC de lo reciente si.
        std::fs::write(&a, [2u8; 1000]).unwrap();
        assert!(barrido_simple(&d) >= 1);
        let texto = std::fs::read_to_string(raiz.join("sincro/diario/diario.jsonl")).unwrap();
        assert!(
            texto
                .lines()
                .any(|l| l.contains("guardados/a.bin") && l.contains(&format!("\"n\":{}", n + 1))),
            "{texto}"
        );
        // Y lo apuntado por quien lo escribio no se apunta otra vez.
        std::fs::write(raiz.join("guardados/b.bin"), [1, 2, 3]).unwrap();
        d.archivo("guardados/b.bin");
        assert_eq!(barrido_simple(&d), 0);
        // Ni lo que no cambio.
        assert_eq!(barrido_simple(&d), 0);
        // Lo que se va, tambien se ve.
        std::fs::remove_file(&a).unwrap();
        assert_eq!(barrido_simple(&d), 1);
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn el_indice_dice_de_que_chat_es_cada_archivo_y_lo_desconocido_se_resuelve() {
        let raiz = carpeta("indice");
        let d = Diario::de(&raiz);
        d.indexar(
            "p1",
            &BTreeSet::from(["pins/draw/a.excalidraw.gz".to_string()]),
        );
        d.guardar_indice();
        let n = d.ultimo();
        d.apuntar(vec![Apunte::nuevo(
            ARCHIVO,
            None,
            "pins/draw/a.excalidraw.gz",
            CAMBIO,
        )]);
        d.apuntar(vec![Apunte::nuevo(
            ARCHIVO,
            None,
            "guardados/nuevo.pdf",
            ALTA,
        )]);
        d.apuntar(vec![Apunte::nuevo(
            ARCHIVO,
            None,
            "guardados/de-nadie.pdf",
            ALTA,
        )]);
        let v = d.vista(n, &|rels| {
            Some(
                rels.iter()
                    .filter(|r| r.contains("nuevo"))
                    .map(|r| (r.clone(), BTreeSet::from(["p2".to_string()])))
                    .collect(),
            )
        });
        assert!(v.cambio("p1", n));
        assert!(v.cambio("p2", n));
        assert!(
            !v.cambio("p3", n),
            "lo que no nombra nadie no es de ningun chat"
        );
        // Sin poder resolver, todos.
        assert!(d.vista(n, &|_| None).cambio("p3", n));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn un_cursor_vale_con_su_libro_y_dentro_del_diario() {
        let raiz = carpeta("cursores");
        let d = Diario::de(&raiz);
        d.apuntar(vec![m("general", "a")]);
        let n = d.ultimo();
        d.poner_cursor(
            "id-tel",
            "general",
            Some(Cursor {
                n,
                ..Default::default()
            }),
        );
        let cs = d.cursores("id-tel");
        assert!(d.vista(0, &|_| None).quieto(&cs, "general"));
        d.apuntar(vec![m("p1", "b")]);
        assert!(d.vista(0, &|_| None).quieto(&cs, "general"), "otro chat");
        d.apuntar(vec![m("general", "c")]);
        assert!(!d.vista(0, &|_| None).quieto(&cs, "general"));
        assert!(!d.vista(0, &|_| None).quieto(&cs, "p9"), "sin cursor");
        let mut otro = cs.clone();
        otro.libro = "otro".into();
        assert!(!d.vista(0, &|_| None).quieto(&otro, "general"));
        d.olvidar_cursores();
        assert!(d.cursores("id-tel").chats.is_empty());
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn la_verificacion_toca_cada_tantas_vueltas_o_cada_dia_o_si_el_reloj_va_atras() {
        let mut c = Cursores {
            verificado: 1_000,
            ..Default::default()
        };
        assert!(!c.toca_verificar(2_000));
        assert!(c.toca_verificar(1_000 + VERIFICAR_CADA_MS));
        assert!(c.toca_verificar(500), "el reloj fue hacia atras");
        c.vueltas = VUELTAS_POR_VERIFICACION;
        assert!(c.toca_verificar(2_000));
        assert!(!Cursores::default().toca_verificar(5));
    }
}
