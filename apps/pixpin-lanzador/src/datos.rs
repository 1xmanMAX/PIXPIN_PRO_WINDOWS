//! Leer el disco de PixPin, solo leer (docs/protocolo-pedidos.md, «Donde
//! leer»), con una cache en memoria que se invalida por la fecha de los
//! ficheros.
//!
//! Los structs son propios y de solo lectura: solo los campos que hacen falta
//! para buscar y abrir, todos con `#[serde(default)]`, de modo que un campo
//! nuevo de la app o del movil no rompe nada. Nunca se escribe aqui: para
//! cambiar algo se le pide a la app.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// El fichero de ajustes cuya presencia junto al exe activa el modo portable
/// (`pixpin_store::rutas`).
const NOMBRE_AJUSTES: &str = "pixpinmax.toml";
/// Lo que ensena la app para el proyecto de `guardados: true`
/// (`almacen::NOMBRE_GUARDADOS`), se llame como se llame en el indice.
pub const NOMBRE_GUARDADOS: &str = "Mensajes guardados";
/// Prefijo de las rutas que llegan del movil.
const PORTATIL: &str = "pixpin:files/";

/// Donde esta `pixpinmax.exe` instalado (`herramientas/instalar.ps1`).
pub fn exe_instalado() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(local).join("Programs").join("PixPinMax").join("pixpinmax.exe"))
}

/// La raiz de datos: la carpeta del exe instalado si a su lado hay un
/// `pixpinmax.toml` (portable), si no `%APPDATA%\PixPinMax`. Es la regla de
/// `pixpin_store::rutas::resolver`.
pub fn raiz_de_datos() -> Option<PathBuf> {
    if let Some(dir) = exe_instalado().as_deref().and_then(Path::parent) {
        if dir.join(NOMBRE_AJUSTES).is_file() {
            return Some(dir.to_path_buf());
        }
    }
    let appdata = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(appdata).join("PixPinMax"))
}

// --- Lo que se lee -------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct IndiceCrudo {
    proyectos: Vec<ProyectoCrudo>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ProyectoCrudo {
    id: String,
    nombre: String,
    tocado: i64,
    hojas: u32,
    guardados: bool,
    papelera: bool,
    #[serde(rename = "enPapelera")]
    en_papelera: bool,
}

/// Un mensaje de `guardados.jsonl`, lo justo para buscar y abrir.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Mensaje {
    pub id: String,
    pub cuando: i64,
    pub clase: String,
    pub texto: Option<String>,
    pub nombre: Option<String>,
    pub ruta: Option<String>,
    pub referencia: Option<String>,
    pub miniapp: Option<String>,
    pub transcripcion: Option<String>,
    /// Lo que dura un audio (el movil lo escribe como numero; `null` vale).
    #[serde(rename = "duracionMs")]
    pub duracion_ms: Option<f64>,
    #[serde(rename = "enBuzon")]
    pub en_buzon: bool,
}

impl Mensaje {
    pub fn texto(&self) -> &str {
        self.texto.as_deref().unwrap_or("")
    }
    pub fn nombre(&self) -> &str {
        self.nombre.as_deref().unwrap_or("").trim()
    }
    pub fn es_lista_de_tareas(&self) -> bool {
        self.clase == "MINIAPP" && self.miniapp.as_deref() == Some("tareas")
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ProyectoJson {
    hojas: Vec<Hoja>,
}

/// Una hoja de `proyecto.json`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Hoja {
    pub id: String,
    pub nombre: String,
    pub dibujo: Option<String>,
}

/// Una nota `.md` de `notas/`.
#[derive(Debug, Clone)]
pub struct NotaMd {
    pub ruta: PathBuf,
    pub titulo: String,
    pub cuando: i64,
}

/// Lo que se sabe de un proyecto.
#[derive(Debug, Clone, Default)]
pub struct Proyecto {
    pub id: String,
    /// El que se ensena (`Mensajes guardados` para el de guardados).
    pub nombre: String,
    pub tocado: i64,
    pub hojas: u32,
    pub guardados: bool,
    pub carpeta: PathBuf,
    pub mensajes: Vec<Mensaje>,
    pub hojas_json: Vec<Hoja>,
    pub notas_md: Vec<NotaMd>,
}

impl Proyecto {
    /// El `proyecto` que va en un pedido: `null` es «Mensajes guardados».
    pub fn id_para_pedido(&self) -> serde_json::Value {
        if self.guardados {
            serde_json::Value::Null
        } else {
            serde_json::Value::String(self.id.clone())
        }
    }

    /// Donde esta en este equipo lo que senala la `ruta` de un mensaje:
    /// relativa a la carpeta del proyecto, o `pixpin:files/…` (lo que llego
    /// del movil, que vive en `android/…`). `None` si es de otro aparato o
    /// intenta salirse de la carpeta.
    pub fn ruta_real(&self, ruta: &str) -> Option<PathBuf> {
        let ruta = ruta.trim();
        if ruta.is_empty() {
            return None;
        }
        let (base, rel) = match ruta.strip_prefix(PORTATIL) {
            Some(rel) => (self.carpeta.join("android"), rel),
            None => {
                if Path::new(ruta).is_absolute() || ruta.starts_with('/') || ruta.contains(':') {
                    return None;
                }
                (self.carpeta.clone(), ruta)
            }
        };
        let mut p = base;
        for trozo in rel.split(['/', '\\']) {
            match trozo {
                "" | "." => {}
                ".." => return None,
                t => p.push(t),
            }
        }
        Some(p)
    }
}

// --- Las tareas ----------------------------------------------------------

/// Una casilla de una lista de tareas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tarea {
    /// Su numero entre las casillas, desde 0, en el orden del documento: el
    /// `indice` de `marcar_tarea`.
    pub indice: usize,
    pub texto: String,
    pub hecha: bool,
    /// El dia en que se creo (`➕ AAAA-MM-DD` al final del texto, como
    /// `pixpin_proyecto::mini::partir`), en dias desde 1970.
    pub creada: Option<i64>,
}

/// El titulo del documento: `^#{1,6}\s+(.*)$` en la primera linea, como
/// `pixpin_proyecto::mini::titulo` (y `MiniApps.kt`).
pub fn titulo_del_documento(documento: &str) -> String {
    let primera = documento.split('\n').next().unwrap_or("").trim();
    let almohadillas = primera.bytes().take_while(|b| *b == b'#').count();
    if !(1..=6).contains(&almohadillas) {
        return String::new();
    }
    let resto = &primera[almohadillas..];
    if !resto.starts_with([' ', '\t']) {
        return String::new();
    }
    resto.trim().to_string()
}

/// Las casillas de un documento, como `pixpin_proyecto::mini::leer_tareas`.
pub fn leer_tareas(documento: &str) -> Vec<Tarea> {
    documento
        .lines()
        .filter_map(casilla)
        .enumerate()
        .map(|(indice, (texto, hecha))| {
            let (texto, creada) = partir_fecha(&texto);
            Tarea { indice, texto, hecha, creada }
        })
        .collect()
}

/// El texto de una tarea sin su fecha de creacion, y esa fecha en dias desde
/// 1970 (la regla de `pixpin_proyecto::mini::partir`: `➕ AAAA-MM-DD` al
/// final, separada por un blanco; si no, todo es texto).
pub fn partir_fecha(texto: &str) -> (String, Option<i64>) {
    let t = texto.trim_end();
    let entero = || (texto.trim().to_string(), None);
    if t.len() < 10 || !t.is_char_boundary(t.len() - 10) {
        return entero();
    }
    let (antes, fecha) = t.split_at(t.len() - 10);
    let b = fecha.as_bytes();
    let num = |r: &[u8]| -> Option<i64> { r.iter().try_fold(0i64, |a, c| c.is_ascii_digit().then(|| a * 10 + i64::from(c - b'0'))) };
    if b[4] != b'-' || b[7] != b'-' {
        return entero();
    }
    let (Some(a), Some(m), Some(d)) = (num(&b[0..4]), num(&b[5..7]), num(&b[8..10])) else { return entero() };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return entero();
    }
    let Some(resto) = antes.trim_end().strip_suffix('\u{2795}') else { return entero() };
    if !resto.is_empty() && !resto.ends_with(char::is_whitespace) {
        return entero();
    }
    (resto.trim().to_string(), Some(dias_civiles(a, m, d)))
}

/// Dias desde 1970-01-01 de una fecha del calendario (algoritmo de Hinnant).
pub fn dias_civiles(a: i64, m: i64, d: i64) -> i64 {
    let a = if m <= 2 { a - 1 } else { a };
    let era = a.div_euclid(400);
    let ya = a - era * 400;
    let mp = (m + 9) % 12;
    let dy = (153 * mp + 2) / 5 + d - 1;
    let de = ya * 365 + ya / 4 - ya / 100 + dy;
    era * 146_097 + de - 719_468
}

/// «hoy», «hace 1 dia», «hace N dias» (como el chat, `mini::edad`).
pub fn hace_dias(dias: i64) -> String {
    match dias {
        i64::MIN..=0 => "hoy".into(),
        1 => "hace 1 día".into(),
        n => format!("hace {n} días"),
    }
}

/// `- [ ] texto` / `* [x] texto`: el texto y si esta hecha.
fn casilla(linea: &str) -> Option<(String, bool)> {
    let resto = linea.trim_start().strip_prefix(['-', '*', '+'])?;
    if !resto.starts_with([' ', '\t']) {
        return None;
    }
    let resto = resto.trim_start().strip_prefix('[')?;
    let marca = resto.chars().next()?;
    let resto = resto.get(marca.len_utf8()..)?.strip_prefix(']')?;
    let hecha = match marca {
        'x' | 'X' => true,
        ' ' => false,
        _ => return None,
    };
    Some((resto.trim().to_string(), hecha))
}

/// El documento con la casilla `indice` marcada o desmarcada (para ensenar
/// el cambio sin esperar a que la app lo escriba).
pub fn con_tarea_marcada(documento: &str, indice: usize, hecha: bool) -> String {
    let mut n = 0;
    let mut lineas: Vec<String> = Vec::new();
    for linea in documento.split('\n') {
        if casilla(linea).is_some() {
            if n == indice {
                if let Some(p) = linea.find('[') {
                    let mut l = linea.to_string();
                    l.replace_range(p + 1..p + 2, if hecha { "x" } else { " " });
                    lineas.push(l);
                    n += 1;
                    continue;
                }
            }
            n += 1;
        }
        lineas.push(linea.to_string());
    }
    lineas.join("\n")
}

/// El documento con una tarea pendiente mas al final.
pub fn con_tarea_anadida(documento: &str, texto: &str) -> String {
    let mut d = documento.trim_end_matches('\n').to_string();
    if !d.is_empty() {
        d.push('\n');
    }
    d.push_str("- [ ] ");
    d.push_str(texto.trim());
    d
}

// --- La cache ------------------------------------------------------------

fn fecha(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn ms(t: SystemTime) -> i64 {
    t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Las fechas de los ficheros de los que sale un proyecto: si alguna cambia,
/// se relee.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Fechas {
    guardados: Option<SystemTime>,
    proyecto: Option<SystemTime>,
    notas: Option<SystemTime>,
}

#[derive(Debug, Default)]
struct Entrada {
    fechas: Fechas,
    mensajes: Vec<Mensaje>,
    hojas: Vec<Hoja>,
    notas_md: Vec<NotaMd>,
}

/// Lo leido del disco, que se pone al dia solo con lo que cambio.
#[derive(Debug)]
pub struct Datos {
    raiz: PathBuf,
    fecha_indice: Option<SystemTime>,
    indice: Vec<ProyectoCrudo>,
    cache: HashMap<String, Entrada>,
}

impl Datos {
    pub fn nuevo(raiz: PathBuf) -> Self {
        Datos { raiz, fecha_indice: None, indice: Vec::new(), cache: HashMap::new() }
    }

    pub fn raiz(&self) -> &Path {
        &self.raiz
    }

    /// Olvidar todo (el `reload_data` de Flow).
    pub fn olvidar(&mut self) {
        self.fecha_indice = None;
        self.indice.clear();
        self.cache.clear();
    }

    fn carpeta_proyectos(&self) -> PathBuf {
        self.raiz.join("proyectos")
    }

    /// Los proyectos, al dia con el disco.
    pub fn proyectos(&mut self) -> Vec<Proyecto> {
        let ruta_indice = self.carpeta_proyectos().join("indice.json");
        let f = fecha(&ruta_indice);
        if f.is_none() || f != self.fecha_indice {
            self.indice = std::fs::read_to_string(&ruta_indice)
                .ok()
                .and_then(|t| serde_json::from_str::<IndiceCrudo>(t.trim_start_matches('\u{feff}')).ok())
                .map(|i| i.proyectos)
                .unwrap_or_default();
            self.fecha_indice = f;
        }
        let base = self.carpeta_proyectos();
        let mut vistos = Vec::new();
        let mut salida = Vec::new();
        for p in &self.indice {
            if p.id.is_empty() || p.papelera || p.en_papelera || p.id.contains(['/', '\\', '.']) {
                continue;
            }
            let carpeta = base.join(&p.id);
            if !carpeta.is_dir() {
                continue;
            }
            vistos.push(p.id.clone());
            let fechas = Fechas {
                guardados: fecha(&carpeta.join("guardados.jsonl")),
                proyecto: fecha(&carpeta.join("proyecto.json")),
                notas: fecha(&carpeta.join("notas")),
            };
            let entrada = self.cache.entry(p.id.clone()).or_default();
            if entrada.fechas != fechas || fechas == Fechas::default() {
                *entrada = leer_proyecto(&carpeta);
                entrada.fechas = fechas;
            }
            salida.push(Proyecto {
                id: p.id.clone(),
                nombre: if p.guardados { NOMBRE_GUARDADOS.to_string() } else { p.nombre.trim().to_string() },
                tocado: p.tocado,
                hojas: p.hojas,
                guardados: p.guardados,
                carpeta,
                mensajes: entrada.mensajes.clone(),
                hojas_json: entrada.hojas.clone(),
                notas_md: entrada.notas_md.clone(),
            });
        }
        self.cache.retain(|id, _| vistos.contains(id));
        salida
    }

    /// Cambia en la cache el texto de un mensaje (lo que se espera que la
    /// app escriba). Se pierde en cuanto el fichero cambie de fecha: entonces
    /// manda lo que haya en el disco.
    pub fn retocar(&mut self, proyecto: &str, codigo: &str, cambio: impl FnOnce(&str) -> String) -> bool {
        let Some(e) = self.cache.get_mut(proyecto) else {
            return false;
        };
        let Some(m) = e.mensajes.iter_mut().find(|m| m.id == codigo) else {
            return false;
        };
        let nuevo = cambio(m.texto());
        m.texto = Some(nuevo);
        true
    }

    /// El id del proyecto de «Mensajes guardados», si lo hay.
    pub fn id_de_guardados(&mut self) -> Option<String> {
        self.proyectos().into_iter().find(|p| p.guardados).map(|p| p.id)
    }
}

fn leer_proyecto(carpeta: &Path) -> Entrada {
    let mensajes = std::fs::read_to_string(carpeta.join("guardados.jsonl"))
        .map(|t| {
            t.lines()
                .map(|l| l.trim().trim_start_matches('\u{feff}'))
                .filter(|l| !l.is_empty())
                // Una linea rota se salta, como en la app.
                .filter_map(|l| serde_json::from_str::<Mensaje>(l).ok())
                .filter(|m| !m.id.is_empty() && !m.en_buzon)
                .collect()
        })
        .unwrap_or_default();
    let hojas = std::fs::read_to_string(carpeta.join("proyecto.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<ProyectoJson>(t.trim_start_matches('\u{feff}')).ok())
        .map(|p| p.hojas)
        .unwrap_or_default();
    let mut notas_md = Vec::new();
    if let Ok(dir) = std::fs::read_dir(carpeta.join("notas")) {
        for e in dir.flatten() {
            let ruta = e.path();
            if ruta.extension().and_then(|x| x.to_str()).is_none_or(|x| !x.eq_ignore_ascii_case("md")) {
                continue;
            }
            let titulo = titulo_de_md(&ruta);
            let cuando = fecha(&ruta).map(ms).unwrap_or(0);
            notas_md.push(NotaMd { ruta, titulo, cuando });
        }
    }
    Entrada { fechas: Fechas::default(), mensajes, hojas, notas_md }
}

/// La primera linea con texto de una nota `.md` (sin las almohadillas), o el
/// nombre del fichero.
fn titulo_de_md(ruta: &Path) -> String {
    use std::io::Read;
    let mut trozo = Vec::new();
    if let Ok(f) = std::fs::File::open(ruta) {
        let _ = f.take(4096).read_to_end(&mut trozo);
    }
    let texto = String::from_utf8_lossy(&trozo);
    primera_linea(&texto)
        .unwrap_or_else(|| ruta.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default())
}

/// La primera linea con texto, sin `#` delante ni enlaces de imagen.
pub fn primera_linea(texto: &str) -> Option<String> {
    texto
        .lines()
        .map(|l| l.trim().trim_start_matches('\u{feff}').trim_start_matches('#').trim())
        .find(|l| !l.is_empty() && !l.starts_with("!["))
        .map(|l| {
            let l: String = l.chars().take(90).collect();
            l
        })
}

/// Milisegundos de ahora.
pub fn ahora_ms() -> i64 {
    ms(SystemTime::now())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_casillas_se_numeran_en_el_orden_del_documento() {
        let d = "# Compra\n\nlo que sea\n- [ ] pan\n* [x] vino\n+ [X] sal\n-[ ] no\n- [?] tampoco";
        assert_eq!(titulo_del_documento(d), "Compra");
        let t = leer_tareas(d);
        assert_eq!(t.len(), 3);
        assert_eq!(t[0], Tarea { indice: 0, texto: "pan".into(), hecha: false, creada: None });
        assert_eq!(t[2], Tarea { indice: 2, texto: "sal".into(), hecha: true, creada: None });
    }

    #[test]
    fn titulo_solo_con_blanco_detras_de_las_almohadillas() {
        assert_eq!(titulo_del_documento("#pegado\n- [ ] a"), "");
        assert_eq!(titulo_del_documento("####### siete"), "");
        assert_eq!(titulo_del_documento("## Dos"), "Dos");
    }

    #[test]
    fn marcar_y_anadir_en_la_cache() {
        let d = "# L\n\n- [x] a\n- [ ] b";
        assert_eq!(con_tarea_marcada(d, 1, true), "# L\n\n- [x] a\n- [x] b");
        assert_eq!(con_tarea_marcada(d, 0, false), "# L\n\n- [ ] a\n- [ ] b");
        assert_eq!(con_tarea_anadida(d, " c "), "# L\n\n- [x] a\n- [ ] b\n- [ ] c");
        assert_eq!(con_tarea_anadida("", "c"), "- [ ] c");
    }

    #[test]
    fn rutas_del_movil_y_relativas() {
        let p = Proyecto { carpeta: PathBuf::from(r"C:\d\proyectos\X"), ..Default::default() };
        assert_eq!(
            p.ruta_real("pixpin:files/guardados/voz_1.m4a"),
            Some(PathBuf::from(r"C:\d\proyectos\X\android\guardados\voz_1.m4a"))
        );
        assert_eq!(p.ruta_real("archivos/a b.pdf"), Some(PathBuf::from(r"C:\d\proyectos\X\archivos\a b.pdf")));
        assert_eq!(p.ruta_real("/storage/emulated/0/x.jpg"), None);
        assert_eq!(p.ruta_real(r"C:\x.jpg"), None);
        assert_eq!(p.ruta_real("archivos/../../fuera"), None);
        assert_eq!(p.ruta_real(""), None);
    }

    #[test]
    fn la_primera_linea_salta_titulos_vacios_e_imagenes() {
        assert_eq!(primera_linea("\n# \n![a](b)\n## Hola\nmas").as_deref(), Some("Hola"));
        assert_eq!(primera_linea("  \n"), None);
    }
}

#[cfg(test)]
mod pruebas_de_fecha {
    use super::*;

    #[test]
    fn la_fecha_de_creada_se_separa_del_texto_y_se_cuenta_en_dias() {
        let (t, c) = partir_fecha("pan ➕ 2026-10-02");
        assert_eq!(t, "pan");
        assert_eq!(c, Some(dias_civiles(2026, 10, 2)));
        assert_eq!(dias_civiles(1970, 1, 1), 0);
        assert_eq!(dias_civiles(2026, 10, 2) - dias_civiles(2026, 9, 30), 2);
        // Sin la marca, o pegada a una palabra, todo es texto.
        assert_eq!(partir_fecha("pan 2026-10-02"), ("pan 2026-10-02".into(), None));
        assert_eq!(partir_fecha("pan➕ 2026-10-02").1, None);
        assert_eq!(leer_tareas("- [ ] pan ➕ 2026-10-02")[0].texto, "pan");
        assert_eq!((hace_dias(0), hace_dias(1), hace_dias(5)), ("hoy".into(), "hace 1 día".into(), "hace 5 días".into()));
    }
}
