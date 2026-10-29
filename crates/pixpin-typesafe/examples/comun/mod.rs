//! **Lo comun de los prototipos sin interfaz** (29-oct-2026, ver
//! `docs/probar-ideas-sin-interfaz.md`): leer un guion de reunion inventado,
//! pasarlo frase a frase en tiempo real simulado, hablar con Jev (de
//! verdad, en seco o con respuestas grabadas), medir y comprobar lo que el
//! guion espera.
//!
//! La logica que se prueba **no esta aqui**: esta en la biblioteca
//! (`pixpin_typesafe::vivo`, `compromisos`, `temas`...), la misma que usara
//! la app. Esto solo es el banco de pruebas.
//!
//! - **De verdad**: la clave de `TYPESAFE_API_KEY` o del fichero de
//!   `TYPESAFE_API_KEY_FILE`. En Linux sale por `curl` con la cabecera de
//!   la clave por la entrada estandar (`-H @-`): nunca en la linea de
//!   ordenes ni en el registro. En Windows, por WinHTTP como la app.
//! - `--en-seco`: sin red ni clave; escribe el JSON exacto de cada
//!   peticion y contesta «no» a todo.
//! - `--grabadas F`: contesta lo grabado en `F` (JSONL; ver [`Repetido`]).
//! - `--grabar F`: de verdad, y guarda cada respuesta en `F` para repetirla.

#![allow(dead_code)]

pub mod partes;

use std::collections::{HashMap, VecDeque};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pixpin_typesafe::fechas::Fecha;
use pixpin_typesafe::transporte::{Contestado, Transporte};
use pixpin_typesafe::vivo::Frase;
use pixpin_typesafe::{Cliente, Error};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

/// Lo que cuesta Jev: dolares por millon de tokens de entrada (Jev 1.14,
/// `docs/jev.md`).
pub const DOLARES_POR_MILLON: f64 = 0.042;

// ---------------------------------------------------------------- guion

/// Una frase del guion.
#[derive(Debug, Clone)]
pub struct FraseGuion {
    pub ms: i64,
    /// Cuando llega el texto (y entra en la prueba).
    pub fin_ms: i64,
    /// Cuando se dejo de hablar. En un guion escrito a mano, lo mismo que
    /// `fin_ms`; en uno oido (`frases_en_vivo --salida`), antes: el corte
    /// y Whisper van en medio.
    pub fin_voz_ms: i64,
    pub quien: String,
    pub dice: String,
    pub linea: usize,
}

impl FraseGuion {
    pub fn vivo(&self) -> Frase {
        Frase {
            ms: self.ms,
            quien: self.quien.clone(),
            dice: self.dice.clone(),
        }
    }
}

/// `# espera: …` (tiene que pasar) o `# espera-no: …` (no puede pasar),
/// para todos los prototipos o solo para uno (`# espera[tareas_dichas]: …`).
#[derive(Debug, Clone)]
pub struct Espera {
    pub si: bool,
    pub texto: String,
    pub solo: Option<String>,
    pub linea: usize,
}

/// **Un guion**: la cabecera (`# clave: valor`), las frases
/// (`[m:ss] Quien: lo que dice`) y lo que se espera.
#[derive(Debug, Clone)]
pub struct Guion {
    pub ruta: PathBuf,
    pub cabecera: Vec<(String, String)>,
    pub frases: Vec<FraseGuion>,
    pub esperas: Vec<Espera>,
}

/// «1:05» o «1:05.3» (o «1:02:05»), en ms.
fn minuto(s: &str) -> Option<i64> {
    let p: Vec<&str> = s.split(':').map(str::trim).collect();
    let (seg, hm) = p.split_last()?;
    let seg: f64 = seg.parse().ok().filter(|x: &f64| *x >= 0.0)?;
    let hm: Vec<i64> = hm.iter().map(|x| x.parse().ok()).collect::<Option<_>>()?;
    let min = match hm.as_slice() {
        [m] => *m,
        [h, m] => h * 60 + m,
        _ => return None,
    };
    Some(min * 60_000 + (seg * 1000.0).round() as i64)
}

impl Guion {
    pub fn leer(ruta: &Path) -> Result<Guion, String> {
        let texto =
            std::fs::read_to_string(ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
        Guion::de_texto(ruta, &texto)
    }

    pub fn de_texto(ruta: &Path, texto: &str) -> Result<Guion, String> {
        let mut g = Guion {
            ruta: ruta.to_path_buf(),
            cabecera: Vec::new(),
            frases: Vec::new(),
            esperas: Vec::new(),
        };
        for (n, l) in texto.lines().enumerate() {
            let n = n + 1;
            let l = l.trim();
            if l.is_empty() {
                continue;
            }
            if let Some(c) = l.strip_prefix('#') {
                let Some((k, v)) = c.split_once(':') else {
                    continue;
                };
                let k = k.trim();
                let v = v.trim().to_string();
                let (base, solo) = match k.split_once('[') {
                    Some((b, s)) => (b.trim(), Some(s.trim_end_matches(']').trim().to_string())),
                    None => (k, None),
                };
                match base {
                    "espera" | "espera-no" => g.esperas.push(Espera {
                        si: base == "espera",
                        texto: v,
                        solo,
                        linea: n,
                    }),
                    _ if !base.contains(' ') => g.cabecera.push((base.to_lowercase(), v)),
                    _ => {}
                }
                continue;
            }
            let Some(resto) = l.strip_prefix('[') else {
                return Err(format!("{}:{n}: no se entiende «{l}»", ruta.display()));
            };
            let (t, resto) = resto
                .split_once(']')
                .ok_or_else(|| format!("{}:{n}: falta «]»", ruta.display()))?;
            // `[0:12]` (escrito a mano) o `[0:12.3 0:17.1 0:19.0]` (oido:
            // empieza, se deja de hablar, llega el texto).
            let tiempos: Vec<i64> = t
                .split_whitespace()
                .map(minuto)
                .collect::<Option<_>>()
                .filter(|v: &Vec<i64>| v.len() == 1 || v.len() == 3)
                .ok_or_else(|| format!("{}:{n}: minuto «{t}»", ruta.display()))?;
            let ms = tiempos[0];
            let oido = (tiempos.len() == 3).then(|| (tiempos[1], tiempos[2]));
            let (quien, dice) = resto
                .split_once(':')
                .ok_or_else(|| format!("{}:{n}: falta «Quien:»", ruta.display()))?;
            g.frases.push(FraseGuion {
                ms,
                fin_ms: oido.map_or(0, |o| o.1),
                fin_voz_ms: oido.map_or(0, |o| o.0),
                quien: quien.trim().to_string(),
                dice: dice.trim().to_string(),
                linea: n,
            });
        }
        // Escrito a mano, cuando se deja de hablar: ~350 ms por palabra,
        // sin pisar la siguiente (y el texto llega en el acto).
        for i in 0..g.frases.len() {
            let f = &g.frases[i];
            if f.fin_ms > 0 {
                continue;
            }
            let mut fin = f.ms + 350 * f.dice.split_whitespace().count() as i64;
            if let Some(s) = g.frases.get(i + 1) {
                fin = fin.min(s.ms);
            }
            g.frases[i].fin_ms = fin.max(f.ms + 200);
            g.frases[i].fin_voz_ms = g.frases[i].fin_ms;
        }
        Ok(g)
    }

    /// ¿Es oido (`frases_en_vivo --salida`: el texto llega despues de que
    /// se deja de hablar)?
    pub fn oido(&self) -> bool {
        self.frases.iter().any(|f| f.fin_voz_ms < f.fin_ms)
    }

    /// **Parte en oraciones** lo que junto la escucha en vivo
    /// (`vivo::partir`, lo que hara la app): cada trozo llega cuando el
    /// texto entero y se dejo de decir donde le toca por letras.
    pub fn partir(&mut self) {
        let mut v = Vec::new();
        for f in &self.frases {
            for (t, fin) in pixpin_typesafe::vivo::partir(&f.vivo(), f.fin_voz_ms) {
                v.push(FraseGuion {
                    ms: t.ms,
                    fin_ms: f.fin_ms,
                    fin_voz_ms: fin,
                    quien: t.quien,
                    dice: t.dice,
                    linea: f.linea,
                });
            }
        }
        self.frases = v;
    }

    pub fn valor(&self, k: &str) -> Option<&str> {
        self.cabecera
            .iter()
            .find(|(c, _)| c == k)
            .map(|(_, v)| v.as_str())
    }

    pub fn valores(&self, k: &str) -> Vec<String> {
        self.cabecera
            .iter()
            .filter(|(c, _)| c == k)
            .map(|(_, v)| v.clone())
            .collect()
    }

    /// `# participantes: Ana, Luis, Marta`.
    pub fn lista(&self, k: &str) -> Vec<String> {
        self.valor(k)
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `# fecha: 2026-10-26` (por omision, el lunes 26-oct-2026).
    pub fn fecha(&self) -> Fecha {
        self.valor("fecha")
            .and_then(Fecha::de_texto)
            .unwrap_or(Fecha {
                anio: 2026,
                mes: 10,
                dia: 26,
            })
    }
}

/// Para comparar: minusculas, sin tildes y solo letras y numeros.
pub fn normal(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'á' | 'à' => 'a',
            'é' | 'è' => 'e',
            'í' | 'ì' => 'i',
            'ó' | 'ò' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            'ñ' => 'n',
            otro => otro,
        };
        if c.is_alphanumeric() {
            o.push(c);
        } else if !o.ends_with(' ') {
            o.push(' ');
        }
    }
    o.trim().to_string()
}

/// «1:05.3».
pub fn hora(ms: i64) -> String {
    let ms = ms.max(0);
    format!(
        "{}:{:02}.{}",
        ms / 60_000,
        (ms / 1000) % 60,
        (ms % 1000) / 100
    )
}

// ---------------------------------------------------------------- opciones

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    DeVerdad,
    EnSeco,
    Grabadas,
}

#[derive(Debug, Clone)]
pub struct Opciones {
    pub guion: PathBuf,
    pub rapido: bool,
    pub modo: Modo,
    pub grabadas: Option<PathBuf>,
    pub grabar: Option<PathBuf>,
    pub tiempo: Duration,
    pub ver: bool,
    pub latencia_ms: i64,
    /// Con un guion oido, no partir en oraciones (`--sin-partir`).
    pub sin_partir: bool,
}

fn ayuda(nombre: &str) -> String {
    format!(
        "uso: cargo run -q -p pixpin-typesafe --example {nombre} -- GUION [opciones]\n\
         \n  GUION           un .txt de examples/guiones/ (basta el nombre)\n\
         \x20 --rapido        sin esperar el tiempo real entre frases\n\
         \x20 --en-seco       sin red ni clave: escribe cada peticion y contesta «no»\n\
         \x20 --grabadas F    contesta lo grabado en F (JSONL)\n\
         \x20 --grabar F      de verdad, y guarda las respuestas en F\n\
         \x20 --ver           de verdad, escribe tambien cada peticion\n\
         \x20 --tiempo S      lo que se espera por peticion (10 s)\n\
         \x20 --latencia MS   la latencia simulada en seco o grabado (300 ms)\n\
         \x20 --sin-partir    un guion oido, sin partir en oraciones\n\
         \nDe verdad: la clave en TYPESAFE_API_KEY o TYPESAFE_API_KEY_FILE."
    )
}

fn salir(msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(2)
}

pub fn opciones(nombre: &str) -> Opciones {
    let mut a = std::env::args().skip(1);
    let mut guion = None;
    let mut o = Opciones {
        guion: PathBuf::new(),
        rapido: false,
        modo: Modo::DeVerdad,
        grabadas: None,
        grabar: None,
        tiempo: Duration::from_secs(10),
        ver: false,
        latencia_ms: 300,
        sin_partir: false,
    };
    while let Some(x) = a.next() {
        let mut valor = |q: &str| a.next().unwrap_or_else(|| salir(&format!("{q} sin valor")));
        match x.as_str() {
            "--rapido" => o.rapido = true,
            "--en-seco" => o.modo = Modo::EnSeco,
            "--ver" => o.ver = true,
            "--sin-partir" => o.sin_partir = true,
            "--grabadas" => {
                o.modo = Modo::Grabadas;
                o.grabadas = Some(PathBuf::from(valor("--grabadas")));
            }
            "--grabar" => o.grabar = Some(PathBuf::from(valor("--grabar"))),
            "--tiempo" => {
                let s: f64 = valor("--tiempo")
                    .parse()
                    .unwrap_or_else(|_| salir("--tiempo: segundos"));
                o.tiempo = Duration::from_secs_f64(s);
            }
            "--latencia" => {
                o.latencia_ms = valor("--latencia")
                    .parse()
                    .unwrap_or_else(|_| salir("--latencia: ms"))
            }
            "-h" | "--ayuda" | "--help" => {
                println!("{}", ayuda(nombre));
                std::process::exit(0)
            }
            otro if otro.starts_with("--") => salir(&format!("{otro}?\n\n{}", ayuda(nombre))),
            otro => guion = Some(otro.to_string()),
        }
    }
    let g = guion.unwrap_or_else(|| salir(&ayuda(nombre)));
    o.guion = buscar(&g);
    if let Some(f) = &o.grabadas {
        o.grabadas = Some(buscar(&f.to_string_lossy()));
    }
    o
}

/// La ruta tal cual o dentro de `examples/guiones/`.
fn buscar(r: &str) -> PathBuf {
    let p = PathBuf::from(r);
    if p.exists() {
        return p;
    }
    let dentro = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("guiones")
        .join(r);
    if dentro.exists() { dentro } else { p }
}

/// La clave: de `TYPESAFE_API_KEY` o del fichero de
/// `TYPESAFE_API_KEY_FILE`. Nunca se escribe en ningun sitio.
fn clave() -> Option<String> {
    if let Ok(k) = std::env::var("TYPESAFE_API_KEY")
        && !k.trim().is_empty()
    {
        return Some(k.trim().to_string());
    }
    let f = std::env::var("TYPESAFE_API_KEY_FILE").ok()?;
    let k = std::fs::read_to_string(f.trim()).ok()?;
    Some(k.trim().to_string()).filter(|k| !k.is_empty())
}

// ---------------------------------------------------------------- transportes

/// **Por curl** (Linux y Mac): la clave va en las cabeceras por la entrada
/// estandar (`-H @-`), el cuerpo por un fichero temporal.
pub struct Curl;

static NUMERO: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl Transporte for Curl {
    fn enviar(
        &self,
        metodo: &str,
        url: &str,
        cabeceras: &[(&'static str, String)],
        cuerpo: &[u8],
        tiempo: Duration,
    ) -> Result<Contestado, String> {
        use std::process::{Command, Stdio};
        let n = NUMERO.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let base = std::env::temp_dir().join(format!("pixpin-jev-{}-{n}", std::process::id()));
        let entrada = base.with_extension("peticion.json");
        let salida = base.with_extension("respuesta.json");
        std::fs::write(&entrada, cuerpo).map_err(|e| e.to_string())?;
        let mut c = Command::new("curl");
        c.args(["-sS", "-X", metodo, "--max-time"])
            .arg(format!("{:.1}", tiempo.as_secs_f64().max(0.5)))
            .args(["-H", "@-", "-w", "%{http_code}", "-o"])
            .arg(&salida);
        if !cuerpo.is_empty() {
            let mut d = std::ffi::OsString::from("@");
            d.push(&entrada);
            c.arg("--data-binary").arg(d);
        }
        c.arg(url)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let r = (|| {
            let mut h = c.spawn().map_err(|e| format!("curl: {e}"))?;
            {
                let mut e = h.stdin.take().ok_or("curl sin entrada")?;
                for (k, v) in cabeceras {
                    writeln!(e, "{k}: {v}").map_err(|e| e.to_string())?;
                }
            }
            let o = h.wait_with_output().map_err(|e| e.to_string())?;
            if !o.status.success() {
                return Err(format!(
                    "curl: {}",
                    String::from_utf8_lossy(&o.stderr).trim()
                ));
            }
            let codigo: u16 = String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse()
                .map_err(|_| "curl: sin codigo".to_string())?;
            let cuerpo = std::fs::read(&salida).unwrap_or_default();
            Ok(Contestado { codigo, cuerpo })
        })();
        let _ = std::fs::remove_file(&entrada);
        let _ = std::fs::remove_file(&salida);
        r
    }
}

/// La huella de un cuerpo (para repetir exactamente lo grabado).
pub fn huella(cuerpo: &[u8]) -> String {
    Sha256::digest(cuerpo)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// **Una respuesta entera** para las preguntas de `cuerpo`, con lo `dado`
/// (atajos: un numero es un Noul; un texto, la opcion elegida de una
/// Choice con 0.9; en un Score, el nivel). Lo que no se da: Noul
/// `por_omision`, la primera opcion, el nivel 0.
pub fn contestar(
    cuerpo: &[u8],
    dado: &Map<String, Value>,
    por_omision: f64,
) -> Result<Vec<u8>, String> {
    let p: Value = serde_json::from_slice(cuerpo).map_err(|e| e.to_string())?;
    let preguntas = p["questions"].as_object().ok_or("peticion sin preguntas")?;
    for k in dado.keys() {
        if !preguntas.contains_key(k) {
            eprintln!("    aviso: «{k}» grabada pero no preguntada");
        }
    }
    let mut a = Map::new();
    for (k, q) in preguntas {
        let d = dado.get(k);
        let r = match (q["type"].as_str(), d) {
            (_, Some(v)) if v.get("type").is_some() => v.clone(),
            (Some("noul"), d) => {
                json!({"type": "noul", "noul": d.and_then(Value::as_f64).unwrap_or(por_omision)})
            }
            (Some("choice"), d) => {
                let ops: Vec<&String> = q["criteria"]
                    .as_object()
                    .map(|m| m.keys().collect())
                    .unwrap_or_default();
                let elegida = d
                    .and_then(Value::as_str)
                    .map(String::from)
                    .or_else(|| ops.first().map(|s| s.to_string()))
                    .unwrap_or_default();
                if !ops.iter().any(|o| **o == elegida) {
                    return Err(format!("«{k}»: la opcion «{elegida}» no se pidio"));
                }
                let n = ops.len() as f64;
                let probs: Map<String, Value> = ops
                    .iter()
                    .map(|o| {
                        let pr = if d.is_some() {
                            if **o == elegida {
                                0.9
                            } else {
                                0.1 / (n - 1.0).max(1.0)
                            }
                        } else {
                            1.0 / n
                        };
                        (o.to_string(), json!(pr))
                    })
                    .collect();
                json!({"type": "choice", "choice": elegida, "confidence": 0.5, "probabilities": probs})
            }
            (Some("score"), d) => {
                let n = q["criteria"].as_array().map_or(1, |v| v.len());
                let nivel = d.and_then(Value::as_u64).unwrap_or(0) as usize;
                let probs: Map<String, Value> = (0..n)
                    .map(|i| (i.to_string(), json!(if i == nivel { 1.0 } else { 0.0 })))
                    .collect();
                json!({"type": "score", "score": nivel, "confidence": 1.0, "legend": {}, "probabilities": probs})
            }
            _ => {
                json!({"type": "box", "box": {"top": 0.0, "bottom": 1.0, "left": 0.0, "right": 1.0}, "confidence": null})
            }
        };
        a.insert(k.clone(), r);
    }
    serde_json::to_vec(&json!({
        "model": "sin-red",
        "answers": a,
        "usage": {"input_tokens": cuerpo.len() / 4, "output_tokens": preguntas.len()},
    }))
    .map_err(|e| e.to_string())
}

/// **En seco**: escribe el cuerpo exacto y contesta «no» a todo.
pub struct EnSeco;

impl Transporte for EnSeco {
    fn enviar(
        &self,
        _: &str,
        _: &str,
        _: &[(&'static str, String)],
        cuerpo: &[u8],
        _: Duration,
    ) -> Result<Contestado, String> {
        println!(
            "    ⇢ peticion exacta ({} bytes): {}",
            cuerpo.len(),
            String::from_utf8_lossy(cuerpo)
        );
        Ok(Contestado {
            codigo: 200,
            cuerpo: contestar(cuerpo, &Map::new(), 0.03)?,
        })
    }
}

/// **Respuestas grabadas**: un JSONL con una respuesta por linea.
///
/// - `{"huella": "…", "answers": {…}}` (lo que escribe `--grabar`): para la
///   peticion con esa huella exacta, en cualquier orden.
/// - `{"pide": "yo me encargo", "answers": {"compromiso": 0.92, "quien": "ana"}}`:
///   para la siguiente peticion que contenga ese texto; las que no lo
///   contienen se contestan «no» a todo (asi solo se escriben las frases
///   que importan). Atajos en `answers`: ver [`contestar`].
/// - `{"answers": {…}}`: para la siguiente peticion, sea cual sea.
/// - `{"fallo": "sin red"}` o `{"http": 429}`: un fallo (para ver que es
///   un error y no un «no»). Con `pide`, solo para esa peticion.
pub struct Repetido {
    por_huella: Mutex<HashMap<String, Value>>,
    en_orden: Mutex<VecDeque<Value>>,
    pub sin_grabar: Mutex<usize>,
}

impl Repetido {
    pub fn leer(ruta: &Path) -> Result<Repetido, String> {
        let t = std::fs::read_to_string(ruta).map_err(|e| format!("{}: {e}", ruta.display()))?;
        let mut por_huella = HashMap::new();
        let mut en_orden = VecDeque::new();
        for (n, l) in t.lines().enumerate() {
            let l = l.trim();
            if l.is_empty() || l.starts_with("//") {
                continue;
            }
            let v: Value = serde_json::from_str(l)
                .map_err(|e| format!("{}:{}: {e}", ruta.display(), n + 1))?;
            match v.get("huella").and_then(Value::as_str) {
                Some(h) => {
                    por_huella.insert(h.to_string(), v);
                }
                None => en_orden.push_back(v),
            }
        }
        Ok(Repetido {
            por_huella: Mutex::new(por_huella),
            en_orden: Mutex::new(en_orden),
            sin_grabar: Mutex::new(0),
        })
    }

    pub fn sin_usar(&self) -> usize {
        self.en_orden.lock().map(|v| v.len()).unwrap_or(0)
            + self.por_huella.lock().map(|v| v.len()).unwrap_or(0)
    }
}

impl Transporte for Repetido {
    fn enviar(
        &self,
        _: &str,
        _: &str,
        _: &[(&'static str, String)],
        cuerpo: &[u8],
        _: Duration,
    ) -> Result<Contestado, String> {
        let h = huella(cuerpo);
        let texto = String::from_utf8_lossy(cuerpo);
        let linea = self
            .por_huella
            .lock()
            .ok()
            .and_then(|mut m| m.remove(&h))
            .or_else(|| {
                let mut v = self.en_orden.lock().ok()?;
                let sirve = match v.front()?.get("pide").and_then(Value::as_str) {
                    Some(p) => texto.contains(p),
                    None => true,
                };
                if sirve { v.pop_front() } else { None }
            });
        let Some(l) = linea else {
            if let Ok(mut n) = self.sin_grabar.lock() {
                *n += 1;
            }
            println!("    (sin grabar: «no» a todo)");
            return Ok(Contestado {
                codigo: 200,
                cuerpo: contestar(cuerpo, &Map::new(), 0.03)?,
            });
        };
        if let Some(f) = l.get("fallo").and_then(Value::as_str) {
            return Err(f.to_string());
        }
        if let Some(c) = l.get("http").and_then(Value::as_u64) {
            return Ok(Contestado {
                codigo: c as u16,
                cuerpo: br#"{"detail":"grabado"}"#.to_vec(),
            });
        }
        let dado = l
            .get("answers")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        Ok(Contestado {
            codigo: 200,
            cuerpo: con_uso(contestar(cuerpo, &dado, 0.03)?, l.get("usage")),
        })
    }
}

/// Los tokens grabados, si los hay (si no, los estimados).
fn con_uso(cuerpo: Vec<u8>, uso: Option<&Value>) -> Vec<u8> {
    let Some(u) = uso.filter(|u| u.is_object()) else {
        return cuerpo;
    };
    match serde_json::from_slice::<Value>(&cuerpo) {
        Ok(mut v) => {
            v["usage"] = u.clone();
            serde_json::to_vec(&v).unwrap_or(cuerpo)
        }
        Err(_) => cuerpo,
    }
}

/// **Graba** lo que contesta el de dentro (solo los 200).
pub struct Grabador {
    dentro: Arc<dyn Transporte>,
    fichero: Mutex<std::fs::File>,
}

impl Transporte for Grabador {
    fn enviar(
        &self,
        metodo: &str,
        url: &str,
        cabeceras: &[(&'static str, String)],
        cuerpo: &[u8],
        tiempo: Duration,
    ) -> Result<Contestado, String> {
        let c = self.dentro.enviar(metodo, url, cabeceras, cuerpo, tiempo)?;
        if (200..300).contains(&c.codigo)
            && let Ok(v) = serde_json::from_slice::<Value>(&c.cuerpo)
            && let Ok(mut f) = self.fichero.lock()
        {
            let l = json!({"huella": huella(cuerpo), "model": v["model"], "answers": v["answers"], "usage": v["usage"]});
            let _ = writeln!(f, "{l}");
        }
        Ok(c)
    }
}

/// Lo medido de una peticion.
#[derive(Debug, Clone)]
pub struct Medida {
    pub ms: f64,
    pub tokens: u64,
    pub codigo: Option<u16>,
}

/// **Mide** cada peticion que sale: lo que tarda y los tokens de entrada.
pub struct Medidor {
    dentro: Arc<dyn Transporte>,
    pub medidas: Mutex<Vec<Medida>>,
    ver: bool,
}

impl Transporte for Medidor {
    fn enviar(
        &self,
        metodo: &str,
        url: &str,
        cabeceras: &[(&'static str, String)],
        cuerpo: &[u8],
        tiempo: Duration,
    ) -> Result<Contestado, String> {
        if self.ver {
            println!(
                "    ⇢ peticion exacta ({} bytes): {}",
                cuerpo.len(),
                String::from_utf8_lossy(cuerpo)
            );
        }
        let t = Instant::now();
        let r = self.dentro.enviar(metodo, url, cabeceras, cuerpo, tiempo);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let tokens = r
            .as_ref()
            .ok()
            .and_then(|c| serde_json::from_slice::<Value>(&c.cuerpo).ok())
            .and_then(|v| v["usage"]["input_tokens"].as_u64())
            .unwrap_or(cuerpo.len() as u64 / 4);
        if let Ok(mut m) = self.medidas.lock() {
            m.push(Medida {
                ms,
                tokens,
                codigo: r.as_ref().ok().map(|c| c.codigo),
            });
        }
        r
    }
}

// ---------------------------------------------------------------- reloj

/// El reloj de la reunion, en ms desde el principio del guion: real, o
/// simulado con `--rapido`.
pub struct Reloj {
    rapido: bool,
    inicio: Instant,
    sim: i64,
}

impl Reloj {
    pub fn ahora(&self) -> i64 {
        if self.rapido {
            self.sim
        } else {
            self.inicio.elapsed().as_millis() as i64
        }
    }

    pub fn esperar_hasta(&mut self, ms: i64) {
        if self.rapido {
            self.sim = self.sim.max(ms);
        } else {
            let falta = ms - self.ahora();
            if falta > 0 {
                std::thread::sleep(Duration::from_millis(falta as u64));
            }
        }
    }

    /// Paso el tiempo de una peticion (en real ya paso solo).
    fn paso(&mut self, ms: i64, dormir: bool) {
        if self.rapido {
            self.sim += ms;
        } else if dormir && ms > 0 {
            std::thread::sleep(Duration::from_millis(ms as u64));
        }
    }
}

// ---------------------------------------------------------------- sesion

/// Algo que decidio Jev (con el codigo): lo que se compara con las esperas.
#[derive(Debug, Clone)]
pub struct Hecho {
    pub ms: i64,
    pub texto: String,
    /// El prototipo que lo decidio (en `reunion_completa`, una espera
    /// `[tareas_dichas]` solo se compara con lo de las tareas).
    pub de: &'static str,
}

/// **Una prueba**: el guion, el cliente, el reloj y lo que va saliendo.
pub struct Sesion {
    pub nombre: &'static str,
    pub op: Opciones,
    pub guion: Guion,
    pub cliente: Cliente,
    medidor: Arc<Medidor>,
    repetido: Option<Arc<Repetido>>,
    pub reloj: Reloj,
    pub hechos: Vec<Hecho>,
    pub errores: usize,
    pub del_cache: usize,
    /// Las esperas de otros prototipos que tambien valen aqui
    /// (`reunion_completa` comprueba las de todos).
    pub tambien: Vec<&'static str>,
    /// Con un guion oido: de que se dejo de hablar a lo decidido, en ms
    /// (corte + Whisper + Jev), una por frase preguntada o por ✓.
    pub desde_voz: Vec<i64>,
}

/// **Empieza**: lee las opciones y el guion y prepara el cliente.
pub fn empezar(nombre: &'static str, que_prueba: &str) -> Sesion {
    let op = opciones(nombre);
    let mut guion = Guion::leer(&op.guion).unwrap_or_else(|e| salir(&e));
    if guion.oido() && !op.sin_partir {
        guion.partir();
    }
    let mut repetido = None;
    let (clave, base): (String, Arc<dyn Transporte>) = match op.modo {
        Modo::EnSeco => ("en-seco".into(), Arc::new(EnSeco)),
        Modo::Grabadas => {
            let r = Arc::new(
                Repetido::leer(op.grabadas.as_deref().unwrap_or(Path::new("")))
                    .unwrap_or_else(|e| salir(&e)),
            );
            repetido = Some(r.clone());
            ("grabadas".into(), r)
        }
        Modo::DeVerdad => {
            let k = clave().unwrap_or_else(|| {
                salir(
                    "falta la clave: TYPESAFE_API_KEY o TYPESAFE_API_KEY_FILE.\n\
                     Para probar sin clave: --en-seco o --grabadas F.",
                )
            });
            #[cfg(windows)]
            let t: Arc<dyn Transporte> = pixpin_typesafe::transporte::del_sistema();
            #[cfg(not(windows))]
            let t: Arc<dyn Transporte> = Arc::new(Curl);
            (k, t)
        }
    };
    let base: Arc<dyn Transporte> = match &op.grabar {
        Some(f) if op.modo == Modo::DeVerdad => {
            let fichero = std::fs::File::create(f)
                .unwrap_or_else(|e| salir(&format!("{}: {e}", f.display())));
            Arc::new(Grabador {
                dentro: base,
                fichero: Mutex::new(fichero),
            })
        }
        _ => base,
    };
    let medidor = Arc::new(Medidor {
        dentro: base,
        medidas: Mutex::new(Vec::new()),
        ver: op.ver && op.modo == Modo::DeVerdad,
    });
    let cliente = Cliente::con_transporte(clave, medidor.clone())
        .con_cache_propio()
        .con_tiempo(op.tiempo);
    let modo = match op.modo {
        Modo::DeVerdad => "de verdad, con la API de TypeSafe",
        Modo::EnSeco => "en seco: sin red, «no» a todo",
        Modo::Grabadas => "con respuestas grabadas",
    };
    println!("━━ {nombre}: {que_prueba}");
    println!(
        "   guion {} · {} frases · {modo}{}",
        op.guion.display(),
        guion.frases.len(),
        if op.rapido {
            " · rapido"
        } else {
            " · tiempo real"
        }
    );
    if let Some(r) = guion.valor("reunion") {
        println!("   reunion: {r} ({})", guion.fecha());
    }
    println!();
    Sesion {
        nombre,
        reloj: Reloj {
            rapido: op.rapido,
            inicio: Instant::now(),
            sim: 0,
        },
        op,
        guion,
        cliente,
        medidor,
        repetido,
        hechos: Vec::new(),
        errores: 0,
        del_cache: 0,
        tambien: Vec::new(),
        desde_voz: Vec::new(),
    }
}

impl Sesion {
    /// Escribe una linea con la hora de la reunion.
    pub fn log(&self, texto: &str) {
        println!("[{}] {texto}", hora(self.reloj.ahora()));
    }

    /// **Pregunta a Jev** (con la funcion de la biblioteca que se prueba):
    /// mide, avanza el reloj y escribe lo que tardo. Un fallo se escribe y
    /// es `None` (nunca un «no»).
    pub fn preguntar<T>(&mut self, f: impl FnOnce(&Cliente) -> Result<T, Error>) -> Option<T> {
        let antes = self.medidor.medidas.lock().map(|m| m.len()).unwrap_or(0);
        let t = Instant::now();
        let r = f(&self.cliente);
        let real = t.elapsed().as_millis() as i64;
        let nuevas: Vec<Medida> = self
            .medidor
            .medidas
            .lock()
            .map(|m| m[antes..].to_vec())
            .unwrap_or_default();
        let ms = if self.op.modo == Modo::DeVerdad {
            self.reloj.paso(real, false);
            real
        } else {
            let s = self.op.latencia_ms * nuevas.len() as i64;
            self.reloj.paso(s, true);
            s
        };
        let tokens: u64 = nuevas.iter().map(|m| m.tokens).sum();
        let como = if nuevas.is_empty() {
            self.del_cache += 1;
            "del cache".to_string()
        } else if self.op.modo == Modo::DeVerdad {
            format!("{ms} ms, {tokens} tokens")
        } else {
            format!("{ms} ms simulados, ~{tokens} tokens")
        };
        match r {
            Ok(v) => {
                println!("    Jev: {como}");
                Some(v)
            }
            Err(e) => {
                self.errores += 1;
                println!("    Jev: ERROR ({como}): {e}  → no se decide nada");
                None
            }
        }
    }

    /// ¿El guion es oido (`frases_en_vivo --salida`, con la hora en que
    /// se dejo de hablar y la del texto)?
    pub fn oido(&self) -> bool {
        self.guion.oido()
    }

    /// Con un guion oido, apunta y escribe lo que tardo lo decidido desde
    /// que se dejo de hablar en `f`: el corte y Whisper (hasta el texto) y
    /// lo demas (Jev, y la espera si iba otra peticion).
    pub fn desde_la_voz(&mut self, f: &FraseGuion) {
        if !self.oido() {
            return;
        }
        let ahora = self.reloj.ahora();
        let total = ahora - f.fin_voz_ms;
        self.desde_voz.push(total);
        println!(
            "    ⏱ se dejo de hablar a las {} → decidido +{:.1} s (corte y texto +{:.1} s, Jev +{:.1} s)",
            hora(f.fin_voz_ms),
            total as f64 / 1000.0,
            (f.fin_ms - f.fin_voz_ms) as f64 / 1000.0,
            (ahora - f.fin_ms) as f64 / 1000.0
        );
    }

    /// Apunta algo decidido.
    pub fn hecho(&mut self, ms: i64, texto: String) {
        self.hecho_de(self.nombre, ms, texto);
    }

    /// Apunta algo decidido por la parte `de`.
    pub fn hecho_de(&mut self, de: &'static str, ms: i64, texto: String) {
        println!("    ⇒ {texto}");
        self.hechos.push(Hecho { ms, texto, de });
    }

    /// **Pasa el guion frase a frase**, cada una cuando se deja de decir:
    /// `f(sesion, numero, anteriores, frase)`. Las muy cortas (un «vale»)
    /// no se preguntan.
    pub fn por_frase(&mut self, mut f: impl FnMut(&mut Sesion, usize, &[Frase], &Frase)) {
        let frases = self.guion.frases.clone();
        let vivas: Vec<Frase> = frases.iter().map(FraseGuion::vivo).collect();
        for (i, fg) in frases.iter().enumerate() {
            self.reloj.esperar_hasta(fg.fin_ms);
            println!("[{}] {}: {}", hora(fg.ms), fg.quien, fg.dice);
            if !pixpin_typesafe::vivo::vale_la_pena(&vivas[i]) {
                println!("    (muy corta: no se pregunta)");
                continue;
            }
            f(self, i, &vivas[..i], &vivas[i]);
            self.desde_la_voz(fg);
        }
    }

    /// **El resumen y las esperas**: 0 si todo lo esperado paso, 1 si no.
    pub fn terminar(self) -> ExitCode {
        let m = self
            .medidor
            .medidas
            .lock()
            .map(|m| m.clone())
            .unwrap_or_default();
        let n = m.len();
        let tokens: u64 = m.iter().map(|x| x.tokens).sum();
        let mut lat: Vec<f64> = m.iter().map(|x| x.ms).collect();
        lat.sort_by(f64::total_cmp);
        let pct = |p: f64| -> f64 {
            if lat.is_empty() {
                0.0
            } else {
                lat[((lat.len() as f64 - 1.0) * p).round() as usize]
            }
        };
        println!();
        println!("━━ resumen de {}", self.nombre);
        println!(
            "   peticiones: {n} ({} errores, {} del cache)",
            self.errores, self.del_cache
        );
        let estimado = if self.op.modo == Modo::DeVerdad {
            ""
        } else {
            "~"
        };
        println!(
            "   tokens de entrada: {estimado}{tokens} ≈ {estimado}{:.5} $ ({DOLARES_POR_MILLON} $ por millon)",
            tokens as f64 * DOLARES_POR_MILLON / 1e6
        );
        if self.op.modo == Modo::DeVerdad && n > 0 {
            println!(
                "   latencia: p50 {:.0} ms · p95 {:.0} ms · max {:.0} ms",
                pct(0.5),
                pct(0.95),
                pct(1.0)
            );
        }
        let esperas: Vec<&Espera> = self
            .guion
            .esperas
            .iter()
            .filter(|e| {
                e.solo
                    .as_deref()
                    .is_none_or(|s| s == self.nombre || self.tambien.contains(&s))
            })
            .collect();
        let hechos: Vec<(String, &str)> = self
            .hechos
            .iter()
            .map(|h| (normal(&h.texto), h.de))
            .collect();
        let voz = if self.desde_voz.is_empty() {
            String::new()
        } else {
            let mut v = self.desde_voz.clone();
            v.sort_unstable();
            let p = |x: f64| v[((v.len() as f64 - 1.0) * x).round() as usize];
            println!(
                "   de que se deja de hablar a lo decidido: p50 {:.1} s · p95 {:.1} s · max {:.1} s ({} veces)",
                p(0.5) as f64 / 1000.0,
                p(0.95) as f64 / 1000.0,
                p(1.0) as f64 / 1000.0,
                v.len()
            );
            format!(" voz_p50_ms={} voz_max_ms={}", p(0.5), p(1.0))
        };
        let mut bien = 0;
        let mut fallos = 0;
        if self.op.modo == Modo::EnSeco {
            println!(
                "   esperas: {} (en seco no se comprueban: Jev no contesto)",
                esperas.len()
            );
        } else {
            for e in &esperas {
                let t = normal(&e.texto);
                // Con varias partes juntas, lo de una parte solo se compara
                // con lo suyo; en un prototipo suelto, con todo.
                let de_aqui = |de: &str| match e.solo.as_deref() {
                    Some(s) if !self.tambien.is_empty() => s == de,
                    _ => true,
                };
                let hay = hechos.iter().any(|(h, de)| de_aqui(de) && h.contains(&t));
                let ok = hay == e.si;
                if ok {
                    bien += 1;
                } else {
                    fallos += 1;
                }
                println!(
                    "   {} {} «{}» (linea {})",
                    if ok { "✓" } else { "✗" },
                    if e.si { "espera" } else { "espera-no" },
                    e.texto,
                    e.linea
                );
            }
        }
        if let Some(r) = &self.repetido {
            let sin = r.sin_usar();
            if sin > 0 {
                println!("   ✗ quedan {sin} respuestas grabadas sin usar (¿desalineadas?)");
                fallos += 1;
            }
        }
        println!(
            "RESUMEN {} esperas={bien}/{} peticiones={n} errores={} tokens={tokens} p50_ms={:.0} p95_ms={:.0}{voz}",
            self.nombre,
            esperas.len(),
            self.errores,
            pct(0.5),
            pct(0.95)
        );
        if fallos > 0 {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        }
    }
}
