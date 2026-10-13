//! **Las herramientas del asistente: el protocolo comun PC/Android y la
//! validacion de lo que el modelo pide.**
//!
//! Cada herramienta se describe con nombre, descripcion y parametros en JSON
//! Schema —la forma de `tools` de la API de chat de OpenAI, que entienden
//! llama.cpp, LiteRT-LM y la plantilla de Qwen—. El movil usa **el mismo
//! JSON** ([`catalogo_json`]): asi un modelo que aprendio a llamar
//! `dibujar_en_lienzo` en el PC lo llama igual en el telefono, y el resultado
//! (elementos de Excalidraw) se abre igual en los dos.
//!
//! ## Por que una validacion estricta
//!
//! Un modelo de 2B parametros acierta casi siempre la forma de la llamada
//! (medido: 6 de 6 en este PC) pero no siempre el contenido: un texto sin
//! `texto`, una coordenada de 90 000, una grafica con mas etiquetas que
//! valores. Lo que entra al lienzo tiene que ser correcto, asi que:
//!
//! - **La estructura es estricta**: tipos, campos obligatorios, rangos y
//!   listas enumeradas. Si falla, la llamada NO se ejecuta y el modelo recibe
//!   el motivo en espanol para corregirse ([`ErrorHerramienta::para_el_modelo`]);
//!   el bucle del agente reintenta (ver `agente.rs`).
//! - **Lo cosmetico es tolerante**: un color que no se entiende pasa a negro
//!   y queda un aviso. El usuario acepta alucinaciones pequenas; no acepta un
//!   lienzo roto ni esperar otra vuelta por un color.
//! - **El JSON roto se intenta arreglar** una vez ([`reparar_json`]: vallas de
//!   Markdown, texto alrededor, comas sobrantes) antes de rendirse.

use serde_json::{Map, Value, json};

use crate::motor::LlamadaCruda;

/// Lo que mide el lienzo que se le describe al modelo. Las coordenadas que
/// devuelve son de este espacio; el lienzo real las coloca donde mira el
/// usuario (ver `lienzo.rs`).
pub const ANCHO_LIENZO: f64 = 800.0;
pub const ALTO_LIENZO: f64 = 600.0;

/// Lo mas lejos que se acepta una coordenada (un poco fuera del lienzo se
/// permite: el modelo a veces se pasa del borde).
pub const COORDENADA_MAXIMA: f64 = 2000.0;
pub const COORDENADA_MINIMA: f64 = -200.0;
pub const MAX_ELEMENTOS: usize = 60;
pub const MAX_DATOS_GRAFICA: usize = 40;
pub const MAX_TEXTO: usize = 500;

/// Una herramienta tal como se le describe al modelo.
#[derive(Debug, Clone, PartialEq)]
pub struct Herramienta {
    pub nombre: String,
    pub descripcion: String,
    /// JSON Schema de los argumentos.
    pub parametros: Value,
}

impl Herramienta {
    pub fn a_json(&self) -> Value {
        json!({"type": "function", "function": {
            "name": self.nombre, "description": self.descripcion, "parameters": self.parametros}})
    }
}

pub const DIBUJAR: &str = "dibujar_en_lienzo";
pub const GRAFICA: &str = "grafica";
pub const OCR: &str = "ocr";

/// Las herramientas de la primera tanda. Las descripciones son cortas a
/// proposito: cada token del catalogo se paga en cada vuelta (con las tres,
/// unos 680 tokens de entrada; medido).
pub fn catalogo() -> Vec<Herramienta> {
    vec![
        Herramienta {
            nombre: DIBUJAR.into(),
            descripcion: "Dibuja figuras en el lienzo de PixPin. Lienzo de 800x600 pixeles, origen arriba a la izquierda. Para copiar una imagen, describe sus formas principales con estas figuras.".into(),
            parametros: json!({
                "type": "object",
                "required": ["elementos"],
                "properties": {"elementos": {"type": "array", "minItems": 1, "maxItems": MAX_ELEMENTOS, "items": {
                    "type": "object",
                    "required": ["tipo", "x", "y"],
                    "properties": {
                        "tipo": {"type": "string", "enum": ["rectangulo", "elipse", "rombo", "linea", "flecha", "texto"]},
                        "x": {"type": "number", "description": "izquierda (o inicio de la linea)"},
                        "y": {"type": "number", "description": "arriba (o inicio de la linea)"},
                        "ancho": {"type": "number"},
                        "alto": {"type": "number"},
                        "x2": {"type": "number", "description": "fin de linea o flecha"},
                        "y2": {"type": "number", "description": "fin de linea o flecha"},
                        "color": {"type": "string", "description": "color del trazo, ej. #e03131 o rojo"},
                        "relleno": {"type": "string", "description": "color de relleno; omitir si no tiene"},
                        "texto": {"type": "string", "description": "obligatorio si tipo es texto"},
                        "tam_letra": {"type": "number"}
                    }
                }}}
            }),
        },
        Herramienta {
            nombre: GRAFICA.into(),
            descripcion: "Dibuja en el lienzo una grafica de barras, lineas o pastel a partir de datos.".into(),
            parametros: json!({
                "type": "object",
                "required": ["tipo", "etiquetas", "valores"],
                "properties": {
                    "tipo": {"type": "string", "enum": ["barras", "lineas", "pastel"]},
                    "titulo": {"type": "string"},
                    "etiquetas": {"type": "array", "items": {"type": "string"}, "maxItems": MAX_DATOS_GRAFICA},
                    "valores": {"type": "array", "items": {"type": "number"}, "maxItems": MAX_DATOS_GRAFICA}
                }
            }),
        },
        Herramienta {
            nombre: OCR.into(),
            descripcion: "Reconoce el texto de un PDF escaneado o de una imagen sin texto seleccionable, con el OCR de Windows. Usala cuando el usuario quiera el texto de un documento adjunto.".into(),
            parametros: json!({
                "type": "object",
                "properties": {
                    "archivo": {"type": "string", "description": "nombre del adjunto; omitir para el ultimo adjunto"},
                    "paginas": {"type": "string", "description": "ej. 1-3,5; omitir para todas"},
                    "idioma": {"type": "string", "enum": ["es", "en", "auto"]}
                }
            }),
        },
    ]
}

/// El catalogo como texto JSON (lista de `tools`), para copiarlo tal cual al
/// movil o a la documentacion.
pub fn catalogo_json() -> String {
    let lista: Vec<Value> = catalogo().iter().map(Herramienta::a_json).collect();
    serde_json::to_string_pretty(&lista).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Lo que pide el modelo, ya validado.

#[derive(Debug, Clone, PartialEq)]
pub enum Llamada {
    Dibujar(Vec<Figura>),
    Grafica(Grafica),
    Ocr(PeticionOcr),
}

impl Llamada {
    pub fn nombre(&self) -> &'static str {
        match self {
            Llamada::Dibujar(_) => DIBUJAR,
            Llamada::Grafica(_) => GRAFICA,
            Llamada::Ocr(_) => OCR,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoFigura {
    Rectangulo,
    Elipse,
    Rombo,
    Linea,
    Flecha,
    Texto,
}

/// Un color como `#rrggbb`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Color(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct Figura {
    pub tipo: TipoFigura,
    pub x: f64,
    pub y: f64,
    pub ancho: f64,
    pub alto: f64,
    /// Para lineas y flechas: el otro extremo.
    pub x2: f64,
    pub y2: f64,
    pub color: Color,
    pub relleno: Option<Color>,
    pub texto: String,
    pub tam_letra: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoGrafica {
    Barras,
    Lineas,
    Pastel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Grafica {
    pub tipo: TipoGrafica,
    pub titulo: String,
    pub etiquetas: Vec<String>,
    pub valores: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PeticionOcr {
    /// `None`: el ultimo adjunto de la conversacion.
    pub archivo: Option<String>,
    /// Rangos de paginas, de 1 en adelante, ambos incluidos. Vacio: todas.
    pub paginas: Vec<(u32, u32)>,
    /// `None`: el idioma del sistema.
    pub idioma: Option<String>,
}

/// Una llamada correcta, con los arreglos cosmeticos que se le hicieron.
#[derive(Debug, Clone, PartialEq)]
pub struct Validada {
    pub llamada: Llamada,
    pub avisos: Vec<String>,
}

/// Por que no se ejecuta una llamada.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ErrorHerramienta {
    #[error("la herramienta «{0}» no existe")]
    Desconocida(String),
    #[error("los argumentos no son JSON valido")]
    JsonRoto,
    #[error("{0}")]
    Parametro(String),
}

impl ErrorHerramienta {
    /// El mensaje que se le devuelve al modelo para que se corrija.
    pub fn para_el_modelo(&self, conocidas: &[Herramienta]) -> String {
        let nombres: Vec<&str> = conocidas.iter().map(|h| h.nombre.as_str()).collect();
        match self {
            ErrorHerramienta::Desconocida(n) => format!(
                "ERROR: la herramienta «{n}» no existe. Las que hay son: {}. Usa una de ellas o contesta con texto.",
                nombres.join(", ")
            ),
            ErrorHerramienta::JsonRoto => {
                "ERROR: los argumentos no son JSON valido. Vuelve a llamar con un objeto JSON correcto.".into()
            }
            ErrorHerramienta::Parametro(m) => {
                format!("ERROR: {m}. Corrige los argumentos y vuelve a llamar.")
            }
        }
    }
}

fn mal(m: impl Into<String>) -> ErrorHerramienta {
    ErrorHerramienta::Parametro(m.into())
}

/// **Intenta arreglar un JSON roto** de los que escriben los modelos
/// pequenos: entre vallas de Markdown, con texto delante o detras, o con una
/// coma antes de `}`/`]`. Solo arreglos que no cambian el sentido.
pub fn reparar_json(texto: &str) -> Option<Value> {
    let t = texto.trim();
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        return Some(v);
    }
    let desde = t.find('{')?;
    let hasta = t.rfind('}')?;
    if hasta <= desde {
        return None;
    }
    let trozo = &t[desde..=hasta];
    if let Ok(v) = serde_json::from_str::<Value>(trozo) {
        return Some(v);
    }
    // Comas colgantes, fuera de cadenas.
    let mut limpio = String::with_capacity(trozo.len());
    let mut en_cadena = false;
    let mut escapado = false;
    let car: Vec<char> = trozo.chars().collect();
    for (i, &c) in car.iter().enumerate() {
        if en_cadena {
            limpio.push(c);
            if escapado {
                escapado = false;
            } else if c == '\\' {
                escapado = true;
            } else if c == '"' {
                en_cadena = false;
            }
            continue;
        }
        if c == '"' {
            en_cadena = true;
        }
        if c == ',' {
            let siguiente = car[i + 1..].iter().find(|c| !c.is_whitespace());
            if matches!(siguiente, Some('}') | Some(']')) {
                continue;
            }
        }
        limpio.push(c);
    }
    serde_json::from_str::<Value>(&limpio).ok()
}

/// **Valida una llamada del modelo** contra las herramientas conocidas.
pub fn validar(cruda: &LlamadaCruda, conocidas: &[Herramienta]) -> Result<Validada, ErrorHerramienta> {
    let nombre = cruda.nombre.trim();
    if !conocidas.iter().any(|h| h.nombre == nombre) {
        return Err(ErrorHerramienta::Desconocida(nombre.to_string()));
    }
    let args = if cruda.argumentos.trim().is_empty() {
        Value::Object(Map::new())
    } else {
        reparar_json(&cruda.argumentos).ok_or(ErrorHerramienta::JsonRoto)?
    };
    let Value::Object(args) = args else {
        return Err(mal("los argumentos tienen que ser un objeto JSON"));
    };
    let mut avisos = Vec::new();
    let llamada = match nombre {
        DIBUJAR => Llamada::Dibujar(validar_dibujo(&args, &mut avisos)?),
        GRAFICA => Llamada::Grafica(validar_grafica(&args, &mut avisos)?),
        OCR => Llamada::Ocr(validar_ocr(&args)?),
        // Una herramienta del catalogo que este crate no sabe validar (la
        // anadio el chat): no se ejecuta a ciegas.
        otro => return Err(ErrorHerramienta::Desconocida(otro.to_string())),
    };
    Ok(Validada { llamada, avisos })
}

/// Un numero: acepta tambien `"120"` (los modelos pequenos lo hacen).
fn numero(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().trim_end_matches("px").trim().parse().ok(),
        _ => None,
    }
    .filter(|n: &f64| n.is_finite())
}

fn numero_en(
    o: &Map<String, Value>,
    campo: &str,
    donde: &str,
    rango: (f64, f64),
) -> Result<Option<f64>, ErrorHerramienta> {
    let Some(v) = o.get(campo) else { return Ok(None) };
    if v.is_null() {
        return Ok(None);
    }
    let n = numero(v).ok_or_else(|| mal(format!("{donde}: «{campo}» tiene que ser un numero")))?;
    if n < rango.0 || n > rango.1 {
        return Err(mal(format!(
            "{donde}: «{campo}» = {n} esta fuera de rango ({} a {})",
            rango.0, rango.1
        )));
    }
    Ok(Some(n))
}

fn texto_en(o: &Map<String, Value>, campo: &str) -> Option<String> {
    match o.get(campo)? {
        Value::String(s) => Some(s.trim().to_string()).filter(|s| !s.is_empty()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

const COORD: (f64, f64) = (COORDENADA_MINIMA, COORDENADA_MAXIMA);
const MEDIDA: (f64, f64) = (1.0, COORDENADA_MAXIMA);

fn validar_dibujo(args: &Map<String, Value>, avisos: &mut Vec<String>) -> Result<Vec<Figura>, ErrorHerramienta> {
    let lista = match args.get("elementos") {
        Some(Value::Array(l)) => l,
        // Un solo elemento suelto, sin la lista: se acepta.
        Some(Value::Object(_)) => std::slice::from_ref(&args["elementos"]),
        _ => return Err(mal("falta «elementos» (una lista de figuras)")),
    };
    if lista.is_empty() {
        return Err(mal("«elementos» esta vacia: pon al menos una figura"));
    }
    if lista.len() > MAX_ELEMENTOS {
        return Err(mal(format!(
            "demasiadas figuras ({}); el maximo es {MAX_ELEMENTOS}",
            lista.len()
        )));
    }
    lista
        .iter()
        .enumerate()
        .map(|(i, v)| validar_figura(i, v, avisos))
        .collect()
}

fn validar_figura(i: usize, v: &Value, avisos: &mut Vec<String>) -> Result<Figura, ErrorHerramienta> {
    let donde = format!("elemento {}", i + 1);
    let Value::Object(o) = v else {
        return Err(mal(format!("{donde}: tiene que ser un objeto")));
    };
    let tipo_txt = texto_en(o, "tipo").ok_or_else(|| mal(format!("{donde}: falta «tipo»")))?;
    let tipo = match tipo_txt.to_lowercase().as_str() {
        "rectangulo" | "rectángulo" | "rect" | "rectangle" | "cuadrado" | "square" => TipoFigura::Rectangulo,
        "elipse" | "ellipse" | "circulo" | "círculo" | "circle" | "ovalo" | "óvalo" => TipoFigura::Elipse,
        "rombo" | "diamond" => TipoFigura::Rombo,
        "linea" | "línea" | "line" => TipoFigura::Linea,
        "flecha" | "arrow" => TipoFigura::Flecha,
        "texto" | "text" => TipoFigura::Texto,
        otro => {
            return Err(mal(format!(
                "{donde}: «tipo» = «{otro}» no existe; usa rectangulo, elipse, rombo, linea, flecha o texto"
            )));
        }
    };
    let x = numero_en(o, "x", &donde, COORD)?.ok_or_else(|| mal(format!("{donde}: falta «x»")))?;
    let y = numero_en(o, "y", &donde, COORD)?.ok_or_else(|| mal(format!("{donde}: falta «y»")))?;
    let es_trazo = matches!(tipo, TipoFigura::Linea | TipoFigura::Flecha);
    // En una linea el ancho/alto son el desplazamiento y pueden ser negativos.
    let rango_medida = if es_trazo { (-COORDENADA_MAXIMA, COORDENADA_MAXIMA) } else { MEDIDA };
    let ancho = numero_en(o, "ancho", &donde, rango_medida)?;
    let alto = numero_en(o, "alto", &donde, rango_medida)?;
    let x2 = numero_en(o, "x2", &donde, COORD)?;
    let y2 = numero_en(o, "y2", &donde, COORD)?;
    let tam_letra = numero_en(o, "tam_letra", &donde, (6.0, 200.0))?.unwrap_or(24.0);
    let texto = texto_en(o, "texto").unwrap_or_default();
    if tipo == TipoFigura::Texto && texto.is_empty() {
        return Err(mal(format!("{donde}: un «texto» necesita el campo «texto» con lo que se lee")));
    }
    if texto.chars().count() > MAX_TEXTO {
        return Err(mal(format!("{donde}: el texto pasa de {MAX_TEXTO} caracteres")));
    }
    let (ancho, alto, x2, y2) = if es_trazo {
        let x2 = x2.or(ancho.map(|a| x + a)).unwrap_or(x + 100.0);
        let y2 = y2.or(alto.map(|a| y + a)).unwrap_or(y);
        if x2 == x && y2 == y {
            return Err(mal(format!("{donde}: la linea empieza y acaba en el mismo punto")));
        }
        (x2 - x, y2 - y, x2, y2)
    } else if tipo == TipoFigura::Texto {
        let a = ancho.unwrap_or(tam_letra * 0.6 * texto.chars().count() as f64);
        let h = alto.unwrap_or(tam_letra * 1.25);
        (a, h, x + a, y + h)
    } else {
        let a = ancho.unwrap_or(100.0);
        let h = alto.unwrap_or(a);
        (a, h, x + a, y + h)
    };
    let trazo = match texto_en(o, "color") {
        None => Color("#1e1e1e".into()),
        Some(c) => color(&c).unwrap_or_else(|| {
            avisos.push(format!("{donde}: color «{c}» desconocido, se usa negro"));
            Color("#1e1e1e".into())
        }),
    };
    let relleno = match texto_en(o, "relleno") {
        None => None,
        Some(c) if matches!(c.to_lowercase().as_str(), "ninguno" | "none" | "transparent" | "transparente") => None,
        Some(c) => match color(&c) {
            Some(col) => Some(col),
            None => {
                avisos.push(format!("{donde}: relleno «{c}» desconocido, se deja sin relleno"));
                None
            }
        },
    };
    Ok(Figura {
        tipo,
        x,
        y,
        ancho,
        alto,
        x2,
        y2,
        color: trazo,
        relleno,
        texto,
        tam_letra,
    })
}

/// `#rgb`, `#rrggbb` o un nombre en espanol o ingles. Los nombres caen en la
/// paleta de Excalidraw, la misma del lienzo.
pub fn color(texto: &str) -> Option<Color> {
    let t = texto.trim().to_lowercase();
    if let Some(h) = t.strip_prefix('#') {
        if h.chars().all(|c| c.is_ascii_hexdigit()) {
            return match h.len() {
                6 => Some(Color(format!("#{h}"))),
                8 => Some(Color(format!("#{}", &h[..6]))),
                3 => {
                    let mut s = String::from("#");
                    for c in h.chars() {
                        s.push(c);
                        s.push(c);
                    }
                    Some(Color(s))
                }
                _ => None,
            };
        }
        return None;
    }
    let hex = match t.as_str() {
        "negro" | "black" => "#1e1e1e",
        "blanco" | "white" => "#ffffff",
        "rojo" | "red" => "#e03131",
        "verde" | "green" => "#2f9e44",
        "azul" | "blue" => "#1971c2",
        "amarillo" | "yellow" => "#f08c00",
        "naranja" | "orange" => "#e8590c",
        "morado" | "violeta" | "purple" | "violet" => "#6741d9",
        "rosa" | "pink" => "#c2255c",
        "gris" | "gray" | "grey" => "#868e96",
        "marron" | "marrón" | "cafe" | "café" | "brown" => "#8b5a2b",
        "celeste" | "cyan" | "cian" => "#0c8599",
        _ => return None,
    };
    Some(Color(hex.into()))
}

fn validar_grafica(args: &Map<String, Value>, avisos: &mut Vec<String>) -> Result<Grafica, ErrorHerramienta> {
    let tipo = match texto_en(args, "tipo").map(|t| t.to_lowercase()).as_deref() {
        Some("barras" | "barra" | "bar" | "columnas") => TipoGrafica::Barras,
        Some("lineas" | "líneas" | "linea" | "línea" | "line") => TipoGrafica::Lineas,
        Some("pastel" | "torta" | "pie" | "circular") => TipoGrafica::Pastel,
        Some(otro) => return Err(mal(format!("«tipo» = «{otro}» no existe; usa barras, lineas o pastel"))),
        None => return Err(mal("falta «tipo» (barras, lineas o pastel)")),
    };
    let valores: Vec<f64> = match args.get("valores") {
        Some(Value::Array(l)) => l
            .iter()
            .enumerate()
            .map(|(i, v)| {
                numero(v)
                    .filter(|n| n.abs() < 1e12)
                    .ok_or_else(|| mal(format!("«valores»[{}] no es un numero valido", i + 1)))
            })
            .collect::<Result<_, _>>()?,
        _ => return Err(mal("falta «valores» (una lista de numeros)")),
    };
    if valores.is_empty() || valores.len() > MAX_DATOS_GRAFICA {
        return Err(mal(format!(
            "«valores» tiene {} datos; tiene que tener de 1 a {MAX_DATOS_GRAFICA}",
            valores.len()
        )));
    }
    let etiquetas: Vec<String> = match args.get("etiquetas") {
        Some(Value::Array(l)) => l
            .iter()
            .map(|v| match v {
                Value::String(s) => s.trim().chars().take(60).collect(),
                otro => otro.to_string(),
            })
            .collect(),
        None | Some(Value::Null) => {
            avisos.push("sin «etiquetas»: se numeran".into());
            (1..=valores.len()).map(|i| i.to_string()).collect()
        }
        _ => return Err(mal("«etiquetas» tiene que ser una lista de textos")),
    };
    if etiquetas.len() != valores.len() {
        return Err(mal(format!(
            "hay {} etiquetas y {} valores; tiene que haber el mismo numero",
            etiquetas.len(),
            valores.len()
        )));
    }
    if tipo == TipoGrafica::Pastel {
        if valores.iter().any(|v| *v < 0.0) {
            return Err(mal("un pastel no admite valores negativos"));
        }
        if valores.iter().sum::<f64>() <= 0.0 {
            return Err(mal("un pastel necesita algun valor mayor que cero"));
        }
    }
    let titulo = texto_en(args, "titulo").unwrap_or_default().chars().take(120).collect();
    Ok(Grafica {
        tipo,
        titulo,
        etiquetas,
        valores,
    })
}

fn validar_ocr(args: &Map<String, Value>) -> Result<PeticionOcr, ErrorHerramienta> {
    let archivo = texto_en(args, "archivo");
    let paginas = match texto_en(args, "paginas") {
        None => Vec::new(),
        Some(p) => leer_paginas(&p)?,
    };
    let idioma = match texto_en(args, "idioma").map(|i| i.to_lowercase()).as_deref() {
        None | Some("auto") => None,
        Some(i @ ("es" | "en")) => Some(i.to_string()),
        Some("espanol" | "español" | "spanish") => Some("es".into()),
        Some("ingles" | "inglés" | "english") => Some("en".into()),
        Some(otro) => return Err(mal(format!("«idioma» = «{otro}» no existe; usa es, en o auto"))),
    };
    Ok(PeticionOcr {
        archivo,
        paginas,
        idioma,
    })
}

/// `"1-3,5"` → `[(1,3),(5,5)]`. Paginas de 1 a 10 000.
pub fn leer_paginas(texto: &str) -> Result<Vec<(u32, u32)>, ErrorHerramienta> {
    let t = texto.trim().to_lowercase();
    if t.is_empty() || t == "todas" || t == "all" {
        return Ok(Vec::new());
    }
    let mut rangos = Vec::new();
    for trozo in t.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (a, b) = match trozo.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (trozo, trozo),
        };
        let a: u32 = a.parse().map_err(|_| mal(format!("«paginas»: «{trozo}» no se entiende")))?;
        let b: u32 = b.parse().map_err(|_| mal(format!("«paginas»: «{trozo}» no se entiende")))?;
        if a == 0 || b < a || b > 10_000 {
            return Err(mal(format!("«paginas»: «{trozo}» esta fuera de rango")));
        }
        rangos.push((a, b));
    }
    Ok(rangos)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn cruda(nombre: &str, args: &str) -> LlamadaCruda {
        LlamadaCruda {
            id: "1".into(),
            nombre: nombre.into(),
            argumentos: args.into(),
        }
    }

    fn validar_(nombre: &str, args: &str) -> Result<Validada, ErrorHerramienta> {
        validar(&cruda(nombre, args), &catalogo())
    }

    #[test]
    fn la_llamada_medida_del_modelo_2b_pasa() {
        // Copiada de la medida real (Qwen3.5-2B, ver el documento).
        let v = validar_(
            DIBUJAR,
            r#"{"elementos":[{"y": 200, "x": 100, "tipo": "elipse", "alto": 150, "ancho": 150}, {"y": 250, "x": 600, "tipo": "rectangulo", "alto": 100, "ancho": 150}]}"#,
        )
        .unwrap();
        let Llamada::Dibujar(f) = v.llamada else { panic!() };
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].tipo, TipoFigura::Elipse);
        assert_eq!(f[1].x2, 750.0);
        assert!(v.avisos.is_empty());
    }

    #[test]
    fn un_texto_sin_texto_se_rechaza_para_reintentar() {
        // Lo que hizo el 0.8B en la medida.
        let e = validar_(DIBUJAR, r##"{"elementos":[{"y":150,"x":300,"tipo":"texto","color":"#ffffff"}]}"##).unwrap_err();
        let m = e.para_el_modelo(&catalogo());
        assert!(m.contains("elemento 1"), "{m}");
        assert!(m.contains("«texto»"), "{m}");
    }

    #[test]
    fn herramienta_desconocida_dice_las_que_hay() {
        let e = validar_("borrar_disco", "{}").unwrap_err();
        assert_eq!(e, ErrorHerramienta::Desconocida("borrar_disco".into()));
        let m = e.para_el_modelo(&catalogo());
        assert!(m.contains("dibujar_en_lienzo") && m.contains("grafica") && m.contains("ocr"));
    }

    #[test]
    fn json_roto_sin_arreglo_se_rechaza() {
        assert_eq!(validar_(GRAFICA, r#"{"tipo": "barras", "valores": [1, 2"#).unwrap_err(), ErrorHerramienta::JsonRoto);
        assert_eq!(validar_(GRAFICA, "no es json").unwrap_err(), ErrorHerramienta::JsonRoto);
    }

    #[test]
    fn json_con_vallas_y_comas_colgantes_se_arregla() {
        let v = validar_(
            GRAFICA,
            "```json\n{\"tipo\": \"barras\", \"etiquetas\": [\"a\", \"b\",], \"valores\": [1, 2,],}\n```",
        )
        .unwrap();
        let Llamada::Grafica(g) = v.llamada else { panic!() };
        assert_eq!(g.valores, vec![1.0, 2.0]);
        // Una coma dentro de una cadena no se toca.
        let v = reparar_json(r#"{"t": "a, }", }"#).unwrap();
        assert_eq!(v["t"], "a, }");
    }

    #[test]
    fn coordenadas_fuera_de_rango_se_rechazan() {
        let e = validar_(DIBUJAR, r#"{"elementos":[{"tipo":"rectangulo","x":90000,"y":10}]}"#).unwrap_err();
        assert!(e.to_string().contains("fuera de rango"), "{e}");
        let e = validar_(DIBUJAR, r#"{"elementos":[{"tipo":"rectangulo","x":1,"y":1,"ancho":-5}]}"#).unwrap_err();
        assert!(e.to_string().contains("«ancho»"), "{e}");
        let e = validar_(DIBUJAR, r#"{"elementos":[{"tipo":"texto","x":1,"y":1,"texto":"a","tam_letra":900}]}"#).unwrap_err();
        assert!(e.to_string().contains("tam_letra"), "{e}");
    }

    #[test]
    fn demasiadas_figuras_o_ninguna_se_rechazan() {
        let una = r#"{"tipo":"elipse","x":1,"y":1}"#;
        let muchas = vec![una; MAX_ELEMENTOS + 1].join(",");
        assert!(validar_(DIBUJAR, &format!(r#"{{"elementos":[{muchas}]}}"#)).is_err());
        assert!(validar_(DIBUJAR, r#"{"elementos":[]}"#).is_err());
        assert!(validar_(DIBUJAR, r#"{}"#).is_err());
    }

    #[test]
    fn tipo_inventado_se_rechaza() {
        let e = validar_(DIBUJAR, r#"{"elementos":[{"tipo":"estrella","x":1,"y":1}]}"#).unwrap_err();
        assert!(e.to_string().contains("estrella"));
    }

    #[test]
    fn los_colores_raros_no_rompen_la_llamada() {
        let v = validar_(
            DIBUJAR,
            r##"{"elementos":[{"tipo":"rectangulo","x":"10","y":"20px","color":"azul","relleno":"#F00"},{"tipo":"elipse","x":1,"y":1,"color":"color-de-ilusion"}]}"##,
        )
        .unwrap();
        let Llamada::Dibujar(f) = v.llamada else { panic!() };
        assert_eq!(f[0].x, 10.0);
        assert_eq!(f[0].y, 20.0);
        assert_eq!(f[0].color, Color("#1971c2".into()));
        assert_eq!(f[0].relleno, Some(Color("#ff0000".into())));
        assert_eq!(f[1].color, Color("#1e1e1e".into()));
        assert_eq!(v.avisos.len(), 1);
    }

    #[test]
    fn una_linea_acepta_x2_y2_o_ancho_alto() {
        let v = validar_(DIBUJAR, r#"{"elementos":[{"tipo":"linea","x":10,"y":10,"x2":110,"y2":60},{"tipo":"flecha","x":0,"y":100,"ancho":-50,"alto":0}]}"#).unwrap();
        let Llamada::Dibujar(f) = v.llamada else { panic!() };
        assert_eq!((f[0].ancho, f[0].alto), (100.0, 50.0));
        assert_eq!((f[1].x2, f[1].y2), (-50.0, 100.0));
        assert!(validar_(DIBUJAR, r#"{"elementos":[{"tipo":"linea","x":5,"y":5,"x2":5,"y2":5}]}"#).is_err());
    }

    #[test]
    fn la_grafica_medida_pasa_y_la_descuadrada_no() {
        let v = validar_(GRAFICA, r#"{"tipo":"barras","etiquetas":["Enero", "Febrero", "Marzo"],"valores":[120, 95, 140]}"#).unwrap();
        let Llamada::Grafica(g) = v.llamada else { panic!() };
        assert_eq!(g.tipo, TipoGrafica::Barras);
        let e = validar_(GRAFICA, r#"{"tipo":"barras","etiquetas":["a"],"valores":[1,2]}"#).unwrap_err();
        assert!(e.to_string().contains("mismo numero"));
        assert!(validar_(GRAFICA, r#"{"tipo":"pastel","etiquetas":["a","b"],"valores":[-1,2]}"#).is_err());
        assert!(validar_(GRAFICA, r#"{"tipo":"radar","etiquetas":["a"],"valores":[1]}"#).is_err());
        assert!(validar_(GRAFICA, r#"{"tipo":"barras","etiquetas":["a"],"valores":["mucho"]}"#).is_err());
    }

    #[test]
    fn ocr_lee_paginas_e_idioma() {
        let v = validar_(OCR, r#"{"archivo":"tesis.pdf","paginas":"1-3, 5","idioma":"español"}"#).unwrap();
        assert_eq!(
            v.llamada,
            Llamada::Ocr(PeticionOcr {
                archivo: Some("tesis.pdf".into()),
                paginas: vec![(1, 3), (5, 5)],
                idioma: Some("es".into())
            })
        );
        // Sin argumentos: el ultimo adjunto, todas las paginas.
        assert!(validar_(OCR, "").is_ok());
        assert!(validar_(OCR, r#"{"paginas":"0-2"}"#).is_err());
        assert!(validar_(OCR, r#"{"paginas":"5-2"}"#).is_err());
        assert!(validar_(OCR, r#"{"paginas":"uno"}"#).is_err());
        assert!(validar_(OCR, r#"{"idioma":"klingon"}"#).is_err());
    }

    #[test]
    fn los_argumentos_que_no_son_objeto_se_rechazan() {
        assert!(matches!(validar_(GRAFICA, "[1,2,3]"), Err(ErrorHerramienta::Parametro(_)) | Err(ErrorHerramienta::JsonRoto)));
    }

    #[test]
    fn el_catalogo_es_json_de_herramientas_valido() {
        let v: Value = serde_json::from_str(&catalogo_json()).unwrap();
        let lista = v.as_array().unwrap();
        assert_eq!(lista.len(), 3);
        for h in lista {
            assert_eq!(h["type"], "function");
            assert_eq!(h["function"]["parameters"]["type"], "object");
        }
    }
}
